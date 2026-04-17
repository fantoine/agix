pub mod github;
pub mod git;
pub mod local;

use crate::error::{AgixError, Result};

#[derive(Debug, Clone)]
pub enum SourceSpec {
    GitHub { org: String, repo: String, ref_str: Option<String> },
    Git    { url: String, ref_str: Option<String> },
    Local  { path: std::path::PathBuf },
    // Format: "marketplace@<org/marketplace-repo>@<plugin>"
    Marketplace { marketplace: String, plugin: String },
}

impl SourceSpec {
    pub fn parse(s: &str) -> Result<Self> {
        if let Some(rest) = s.strip_prefix("github:") {
            let (path, ref_str) = split_ref(rest);
            let (org, repo) = path.split_once('/').ok_or_else(|| {
                AgixError::InvalidSource(format!("github source must be 'github:org/repo', got: {s}"))
            })?;
            return Ok(SourceSpec::GitHub {
                org: org.to_owned(),
                repo: repo.to_owned(),
                ref_str: ref_str.map(str::to_owned),
            });
        }
        if let Some(rest) = s.strip_prefix("git:") {
            let (url, ref_str) = split_ref(rest);
            return Ok(SourceSpec::Git {
                url: url.to_owned(),
                ref_str: ref_str.map(str::to_owned),
            });
        }
        if let Some(path) = s.strip_prefix("local:") {
            return Ok(SourceSpec::Local {
                path: std::path::PathBuf::from(path),
            });
        }
        // Marketplace: "marketplace:<org/repo>@<plugin>"
        if let Some(after) = s.strip_prefix("marketplace:") {
            let (marketplace, plugin) = after.split_once('@').ok_or_else(|| {
                AgixError::InvalidSource(format!(
                    "marketplace source must be 'marketplace:<org/repo>@<plugin>', got: {s}"
                ))
            })?;
            return Ok(SourceSpec::Marketplace {
                marketplace: marketplace.to_owned(),
                plugin: plugin.to_owned(),
            });
        }
        Err(AgixError::InvalidSource(format!("unknown source scheme: {s}")))
    }

    pub fn canonical(&self) -> String {
        match self {
            SourceSpec::GitHub { org, repo, ref_str } => {
                let base = format!("github:{org}/{repo}");
                if let Some(r) = ref_str { format!("{base}@{r}") } else { base }
            }
            SourceSpec::Git { url, ref_str } => {
                let base = format!("git:{url}");
                if let Some(r) = ref_str { format!("{base}@{r}") } else { base }
            }
            SourceSpec::Local { path } => format!("local:{}", path.display()),
            SourceSpec::Marketplace { marketplace, plugin } => {
                format!("marketplace:{marketplace}@{plugin}")
            }
        }
    }
}

fn split_ref(s: &str) -> (&str, Option<&str>) {
    match s.find('@') {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None    => (s, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_source() {
        let spec = SourceSpec::parse("github:org/repo").unwrap();
        assert!(matches!(spec, SourceSpec::GitHub { ref org, ref repo, ref_str: None } if org == "org" && repo == "repo"));
    }

    #[test]
    fn parse_github_with_version() {
        let spec = SourceSpec::parse("github:org/repo@main").unwrap();
        assert!(matches!(spec, SourceSpec::GitHub { ref_str: Some(ref r), .. } if r == "main"));
    }

    #[test]
    fn parse_git_source() {
        let spec = SourceSpec::parse("git:https://example.com/repo.git").unwrap();
        assert!(matches!(spec, SourceSpec::Git { .. }));
    }

    #[test]
    fn parse_local_source() {
        let spec = SourceSpec::parse("local:../my-tool").unwrap();
        assert!(matches!(spec, SourceSpec::Local { .. }));
    }

    #[test]
    fn parse_marketplace_source() {
        let spec = SourceSpec::parse("marketplace:fantoine/claude-plugins@roundtable").unwrap();
        assert!(matches!(spec, SourceSpec::Marketplace { ref marketplace, ref plugin }
            if marketplace == "fantoine/claude-plugins" && plugin == "roundtable"));
    }

    #[test]
    fn parse_invalid_source() {
        assert!(SourceSpec::parse("invalid").is_err());
    }
}
