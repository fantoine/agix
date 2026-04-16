pub mod github;
pub mod git;
pub mod local;

use crate::error::{AgixError, Result};

#[derive(Debug, Clone)]
pub enum SourceSpec {
    GitHub { org: String, repo: String, ref_str: Option<String> },
    Git    { url: String, ref_str: Option<String> },
    Local  { path: std::path::PathBuf },
    Marketplace { cli: String, identifier: String },
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
        // Marketplace: "<cli>:marketplace@<identifier>"
        if let Some(colon) = s.find(':') {
            let cli = &s[..colon];
            let rest = &s[colon + 1..];
            if rest.starts_with("marketplace@") {
                return Ok(SourceSpec::Marketplace {
                    cli: cli.to_owned(),
                    identifier: rest["marketplace@".len()..].to_owned(),
                });
            }
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
            SourceSpec::Marketplace { cli, identifier } => {
                format!("{cli}:marketplace@{identifier}")
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
        let spec = SourceSpec::parse("claude:marketplace@org/plugin").unwrap();
        assert!(matches!(spec, SourceSpec::Marketplace { ref cli, .. } if cli == "claude"));
    }

    #[test]
    fn parse_invalid_source() {
        assert!(SourceSpec::parse("invalid").is_err());
    }
}
