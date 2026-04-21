pub mod git;
pub mod github;
pub mod local;
pub mod marketplace;

use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::{Path, PathBuf};

use crate::error::{AgixError, Result};

/// What a fetch yielded. Marketplace sources don't fetch files — they delegate
/// to a CLI driver that owns its own plugin directory.
#[derive(Debug)]
pub enum FetchOutcome {
    Fetched {
        path: PathBuf,
        sha: Option<String>,
        content_hash: Option<String>,
    },
    DelegateToDriver {
        marketplace: String,
        plugin: String,
    },
}

#[async_trait]
pub trait Source: Send + Sync + std::fmt::Debug {
    /// Scheme prefix (e.g. "github", "git", "local", "marketplace"). Matches
    /// the left side of `<scheme>:<value>` in Agentfile source strings.
    fn scheme(&self) -> &'static str;

    /// Canonical `<scheme>:<value>` form. Must roundtrip through
    /// `sources::parse_source`.
    fn canonical(&self) -> String;

    /// Name to suggest when adding this source without an explicit name.
    fn suggested_name(&self) -> Result<String>;

    /// Fetch source files into `dest`, or signal that a CLI driver owns this.
    async fn fetch(&self, dest: &Path) -> Result<FetchOutcome>;

    /// Filesystem path this source refers to, if any. Used by `export` to
    /// vendor local directories into the archive. Default: None.
    fn local_path(&self) -> Option<&Path> {
        None
    }

    /// If this source delegates to a CLI marketplace, return `(marketplace, plugin)`.
    /// Used by uninstall routing to avoid fetching just to learn the source kind.
    /// Default: None.
    fn as_marketplace(&self) -> Option<(&str, &str)> {
        None
    }

    /// Deep clone into a new trait object. Required because `Box<dyn Source>`
    /// can't derive `Clone`; [`SourceBox`] forwards its `Clone` impl here.
    fn clone_box(&self) -> Box<dyn Source>;
}

// ---------------------------------------------------------------------------
// SourceBox — a typed, serde-aware wrapper around `Box<dyn Source>`.
// ---------------------------------------------------------------------------

/// Newtype around `Box<dyn Source>` that adds `Debug`, `Clone`, `PartialEq`,
/// `Eq`, `Serialize` and `Deserialize`.
///
/// Equality and hashing (not implemented — not needed yet) are based on the
/// canonical `<scheme>:<value>` form, so two sources that parse-and-canonicalise
/// to the same string are considered equal even if their internal reps differ.
///
/// Serde roundtrip: `SourceBox` (de)serialises from/to a single TOML string,
/// which is what you want inside `Dependency` and `LockedPackage`.
pub struct SourceBox(Box<dyn Source>);

impl SourceBox {
    pub fn parse(s: &str) -> Result<Self> {
        parse_source(s).map(SourceBox)
    }

    pub fn as_source(&self) -> &dyn Source {
        &*self.0
    }

    pub fn into_inner(self) -> Box<dyn Source> {
        self.0
    }
}

impl From<Box<dyn Source>> for SourceBox {
    fn from(b: Box<dyn Source>) -> Self {
        SourceBox(b)
    }
}

impl std::fmt::Debug for SourceBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SourceBox")
            .field(&self.0.canonical())
            .finish()
    }
}

impl std::ops::Deref for SourceBox {
    type Target = dyn Source;
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

impl Clone for SourceBox {
    fn clone(&self) -> Self {
        SourceBox(self.0.clone_box())
    }
}

impl PartialEq for SourceBox {
    fn eq(&self, other: &Self) -> bool {
        self.0.canonical() == other.0.canonical()
    }
}

impl Eq for SourceBox {}

impl Serialize for SourceBox {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.canonical())
    }
}

impl<'de> Deserialize<'de> for SourceBox {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        parse_source(&s)
            .map(SourceBox)
            .map_err(serde::de::Error::custom)
    }
}

/// A registered scheme that knows how to parse its own `<scheme>:<value>` form.
pub trait SourceScheme: Send + Sync {
    fn scheme(&self) -> &'static str;
    fn parse(&self, value: &str) -> Result<Box<dyn Source>>;
}

pub fn all_schemes() -> Vec<Box<dyn SourceScheme>> {
    vec![
        Box::new(local::LocalScheme),
        Box::new(github::GitHubScheme),
        Box::new(git::GitScheme),
        Box::new(marketplace::MarketplaceScheme),
    ]
}

pub fn scheme_names() -> Vec<&'static str> {
    all_schemes().iter().map(|s| s.scheme()).collect()
}

pub fn parse_source(s: &str) -> Result<Box<dyn Source>> {
    let (scheme, value) = s
        .split_once(':')
        .ok_or_else(|| AgixError::InvalidSource(format!("missing scheme in source: {s}")))?;
    for sch in all_schemes() {
        if sch.scheme() == scheme {
            return sch.parse(value);
        }
    }
    Err(AgixError::InvalidSource(format!(
        "unknown source scheme: {scheme}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_routes_to_correct_scheme() {
        let s = parse_source("local:/tmp/foo").unwrap();
        assert_eq!(s.scheme(), "local");
        assert_eq!(s.canonical(), "local:/tmp/foo");
    }

    #[test]
    fn parse_github_source() {
        let s = parse_source("github:org/repo@main").unwrap();
        assert_eq!(s.scheme(), "github");
        assert_eq!(s.canonical(), "github:org/repo@main");
        assert_eq!(s.suggested_name().unwrap(), "repo");
    }

    #[test]
    fn parse_github_source_no_ref() {
        let s = parse_source("github:org/repo").unwrap();
        assert_eq!(s.scheme(), "github");
        assert_eq!(s.canonical(), "github:org/repo");
    }

    #[test]
    fn parse_git_source() {
        let s = parse_source("git:https://example.com/repo.git").unwrap();
        assert_eq!(s.scheme(), "git");
        assert_eq!(s.suggested_name().unwrap(), "repo");
    }

    #[test]
    fn parse_local_source() {
        let s = parse_source("local:../my-tool").unwrap();
        assert_eq!(s.scheme(), "local");
        assert_eq!(s.suggested_name().unwrap(), "my-tool");
    }

    #[test]
    fn parse_local_suggested_name_strips_trailing_slash() {
        let s = parse_source("local:/tmp/foo/my-pkg/").unwrap();
        assert_eq!(s.suggested_name().unwrap(), "my-pkg");
    }

    #[test]
    fn parse_marketplace_source() {
        let s = parse_source("marketplace:fantoine/claude-plugins@roundtable").unwrap();
        assert_eq!(s.scheme(), "marketplace");
        assert_eq!(
            s.canonical(),
            "marketplace:fantoine/claude-plugins@roundtable"
        );
        assert_eq!(s.suggested_name().unwrap(), "roundtable");
        assert_eq!(
            s.as_marketplace(),
            Some(("fantoine/claude-plugins", "roundtable"))
        );
    }

    #[test]
    fn parse_unknown_scheme_fails() {
        assert!(parse_source("nope:xxx").is_err());
    }

    #[test]
    fn parse_missing_scheme_fails() {
        assert!(parse_source("no-colon").is_err());
    }

    #[test]
    fn scheme_names_lists_all_registered() {
        let names = scheme_names();
        assert!(names.contains(&"local"));
        assert!(names.contains(&"github"));
        assert!(names.contains(&"git"));
        assert!(names.contains(&"marketplace"));
    }

    #[test]
    fn local_path_exposes_path_only_for_local() {
        let local = parse_source("local:/tmp/foo").unwrap();
        assert_eq!(
            local.local_path().map(|p| p.display().to_string()),
            Some("/tmp/foo".into())
        );
        let gh = parse_source("github:org/repo").unwrap();
        assert!(gh.local_path().is_none());
        let git = parse_source("git:https://example.com/repo.git").unwrap();
        assert!(git.local_path().is_none());
        let mk = parse_source("marketplace:org/repo@plug").unwrap();
        assert!(mk.local_path().is_none());
    }

    #[test]
    fn as_marketplace_is_some_only_for_marketplace() {
        let local = parse_source("local:/tmp/foo").unwrap();
        assert!(local.as_marketplace().is_none());
        let gh = parse_source("github:org/repo").unwrap();
        assert!(gh.as_marketplace().is_none());
        let git = parse_source("git:https://example.com/repo.git").unwrap();
        assert!(git.as_marketplace().is_none());
    }

    #[test]
    fn canonical_roundtrips_through_parse() {
        for input in [
            "local:/tmp/foo",
            "github:org/repo",
            "github:org/repo@v1.0",
            "git:https://example.com/repo.git",
            "git:https://example.com/repo.git@abc",
            "marketplace:org/repo@plugin",
        ] {
            let parsed = parse_source(input).unwrap();
            assert_eq!(parsed.canonical(), input, "roundtrip failed for {input}");
        }
    }

    #[test]
    fn parse_marketplace_without_plugin_fails() {
        assert!(parse_source("marketplace:org/repo").is_err());
    }
}
