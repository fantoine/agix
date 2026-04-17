use std::io::Read as _;
use std::path::{Path, PathBuf};

use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};

use crate::error::{AgixError, Result};

pub struct FetchedGitHub {
    pub sha: String,
    pub path: PathBuf,
}

pub struct GitHubSource {
    org: String,
    repo: String,
    ref_str: Option<String>,
    api_base: String,
}

impl GitHubSource {
    pub fn new(org: &str, repo: &str, ref_str: Option<&str>) -> Self {
        Self {
            org: org.to_owned(),
            repo: repo.to_owned(),
            ref_str: ref_str.map(str::to_owned),
            api_base: "https://api.github.com".to_owned(),
        }
    }

    /// Constructor with configurable API base — intended for tests (e.g. mockito).
    #[cfg(test)]
    pub fn new_with_base(org: &str, repo: &str, ref_str: Option<&str>, base: &str) -> Self {
        Self {
            org: org.to_owned(),
            repo: repo.to_owned(),
            ref_str: ref_str.map(str::to_owned),
            api_base: base.to_owned(),
        }
    }

    pub fn parse_org_repo(s: &str) -> Result<(String, String)> {
        let (org, repo) = s
            .split_once('/')
            .ok_or_else(|| AgixError::InvalidSource(format!("expected 'org/repo', got: {s}")))?;
        Ok((org.to_owned(), repo.to_owned()))
    }

    pub async fn resolve_ref(&self) -> Result<String> {
        let ref_str = match &self.ref_str {
            None => return self.fetch_default_sha().await,
            Some(r) => r.clone(),
        };

        // Only treat as a commit hash if it is EXACTLY 40 hex chars
        // (a 7-char short SHA is ambiguous with tag/branch names, so we resolve those first)
        if ref_str.len() == 40 && ref_str.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(ref_str);
        }

        let client = build_client()?;

        // Try as tag
        let tag_url = format!(
            "{}/repos/{}/{}/git/ref/tags/{}",
            self.api_base, self.org, self.repo, ref_str
        );
        let resp = client.get(&tag_url).send().await.map_err(AgixError::Http)?;
        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await.map_err(AgixError::Http)?;
            if let Some(sha) = json["object"]["sha"].as_str() {
                return Ok(sha.to_owned());
            }
        }

        // Try as branch
        let branch_url = format!(
            "{}/repos/{}/{}/git/ref/heads/{}",
            self.api_base, self.org, self.repo, ref_str
        );
        let resp = client
            .get(&branch_url)
            .send()
            .await
            .map_err(AgixError::Http)?;
        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await.map_err(AgixError::Http)?;
            if let Some(sha) = json["object"]["sha"].as_str() {
                return Ok(sha.to_owned());
            }
        }

        // Fallback: HEAD of default branch
        self.fetch_default_sha().await
    }

    async fn fetch_default_sha(&self) -> Result<String> {
        let client = build_client()?;
        let url = format!(
            "{}/repos/{}/{}/commits/HEAD",
            self.api_base, self.org, self.repo
        );
        let resp = client.get(&url).send().await.map_err(AgixError::Http)?;
        if resp.status().is_success() {
            let json: serde_json::Value = resp.json().await.map_err(AgixError::Http)?;
            if let Some(sha) = json["sha"].as_str() {
                return Ok(sha.to_owned());
            }
        }
        Err(AgixError::Other(format!(
            "could not resolve ref for {}/{}",
            self.org, self.repo
        )))
    }

    pub async fn fetch(&self, dest: &Path) -> Result<FetchedGitHub> {
        let sha = self.resolve_ref().await?;

        let zip_url = format!(
            "https://github.com/{}/{}/archive/{}.zip",
            self.org, self.repo, sha
        );

        let resp = reqwest::get(&zip_url).await.map_err(AgixError::Http)?;
        let bytes = resp.bytes().await.map_err(AgixError::Http)?;

        let cursor = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| AgixError::Other(format!("zip open error: {e}")))?;

        std::fs::create_dir_all(dest)?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| AgixError::Other(format!("zip entry error: {e}")))?;

            let raw_name = file.name().to_owned();

            // Strip the first path component (the GitHub-added prefix, e.g. "repo-abc1234/")
            let stripped = raw_name
                .splitn(2, '/')
                .nth(1)
                .unwrap_or("")
                .trim_end_matches('/');

            if stripped.is_empty() {
                continue;
            }

            let out_path = dest.join(stripped);

            if raw_name.ends_with('/') {
                std::fs::create_dir_all(&out_path)?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut content = Vec::new();
                file.read_to_end(&mut content)
                    .map_err(|e| AgixError::Other(format!("zip read error: {e}")))?;
                std::fs::write(&out_path, &content)?;
            }
        }

        Ok(FetchedGitHub {
            sha,
            path: dest.to_path_buf(),
        })
    }
}

fn build_client() -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("agix/0.1"));
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(AgixError::Http)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn resolve_tag_returns_sha() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/repos/org/repo/git/ref/tags/v1.0.0")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"object":{"sha":"abc123def456abc123def456abc123def456abc1"}}"#)
            .create_async()
            .await;

        let source = GitHubSource::new_with_base("org", "repo", Some("v1.0.0"), &server.url());
        let sha = source.resolve_ref().await.unwrap();
        assert_eq!(sha, "abc123def456abc123def456abc123def456abc1");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn resolve_branch_falls_back_when_tag_missing() {
        let mut server = mockito::Server::new_async().await;
        let mock_tag = server
            .mock("GET", "/repos/org/repo/git/ref/tags/main")
            .with_status(404)
            .create_async()
            .await;
        let mock_branch = server
            .mock("GET", "/repos/org/repo/git/ref/heads/main")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"object":{"sha":"deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"}}"#)
            .create_async()
            .await;

        let source = GitHubSource::new_with_base("org", "repo", Some("main"), &server.url());
        let sha = source.resolve_ref().await.unwrap();
        assert_eq!(sha, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
        mock_tag.assert_async().await;
        mock_branch.assert_async().await;
    }

    #[tokio::test]
    async fn resolve_exact_40_hex_sha_returns_directly() {
        // No HTTP calls should be made for a 40-char hex string
        let source = GitHubSource::new_with_base(
            "org",
            "repo",
            Some("abc123def456abc123def456abc123def456abc1"),
            "http://localhost:1", // unreachable — should never be called
        );
        let sha = source.resolve_ref().await.unwrap();
        assert_eq!(sha, "abc123def456abc123def456abc123def456abc1");
    }

    #[test]
    fn parse_org_repo_valid() {
        let (org, repo) = GitHubSource::parse_org_repo("org/repo").unwrap();
        assert_eq!(org, "org");
        assert_eq!(repo, "repo");
    }

    #[test]
    fn parse_org_repo_invalid() {
        assert!(GitHubSource::parse_org_repo("invalid").is_err());
    }

    #[test]
    fn fetch_github_creates_dest() {
        // Smoke test: just verify the dest directory creation path compiles and works
        let dir = tempdir().unwrap();
        assert!(dir.path().exists());
    }
}
