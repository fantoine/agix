use async_trait::async_trait;
use std::io::Read as _;
use std::path::Path;

use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};

use crate::error::{AgixError, Result};
use crate::sources::{FetchOutcome, Source, SourceScheme};

const DEFAULT_API_BASE: &str = "https://api.github.com";
const DEFAULT_WEB_BASE: &str = "https://github.com";
const BASE_URL_ENV_VAR: &str = "AGIX_GITHUB_BASE_URL";

pub struct GitHubSource {
    org: String,
    repo: String,
    ref_str: Option<String>,
    api_base: String,
    web_base: String,
}

/// Pick the `(api_base, web_base)` pair honouring, in order:
/// 1. an explicit caller override (both bases collapse to it — suitable for a
///    single mockito server that answers both the API and the archive URL);
/// 2. the `AGIX_GITHUB_BASE_URL` env var (same collapsing semantics);
/// 3. the production defaults (`api.github.com` + `github.com`).
fn resolve_bases(override_base: Option<&str>) -> (String, String) {
    if let Some(base) = override_base {
        return (base.to_owned(), base.to_owned());
    }
    if let Ok(env_base) = std::env::var(BASE_URL_ENV_VAR) {
        if !env_base.is_empty() {
            return (env_base.clone(), env_base);
        }
    }
    (DEFAULT_API_BASE.to_owned(), DEFAULT_WEB_BASE.to_owned())
}

impl GitHubSource {
    pub fn new(org: &str, repo: &str, ref_str: Option<&str>) -> Self {
        let (api_base, web_base) = resolve_bases(None);
        Self {
            org: org.to_owned(),
            repo: repo.to_owned(),
            ref_str: ref_str.map(str::to_owned),
            api_base,
            web_base,
        }
    }

    /// Constructor with configurable base URL — intended for tests (e.g. mockito).
    /// Sets both `api_base` and `web_base` to `base`, so a single mock server
    /// can answer the API lookup and the archive download.
    #[cfg(test)]
    pub fn new_with_base(org: &str, repo: &str, ref_str: Option<&str>, base: &str) -> Self {
        Self {
            org: org.to_owned(),
            repo: repo.to_owned(),
            ref_str: ref_str.map(str::to_owned),
            api_base: base.to_owned(),
            web_base: base.to_owned(),
        }
    }

    /// Like [`GitHubSource::new`], but lets a caller override the GitHub base
    /// URL. When `base` is `Some`, both API and archive calls go there;
    /// when `None`, the env-var + production defaults apply (see
    /// [`resolve_bases`]).
    ///
    /// Used by `outdated` so integration tests can redirect HTTP calls to a
    /// mockito server.
    pub fn new_with_optional_base(
        org: &str,
        repo: &str,
        ref_str: Option<&str>,
        base: Option<&str>,
    ) -> Self {
        let (api_base, web_base) = resolve_bases(base);
        Self {
            org: org.to_owned(),
            repo: repo.to_owned(),
            ref_str: ref_str.map(str::to_owned),
            api_base,
            web_base,
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
}

#[async_trait]
impl Source for GitHubSource {
    fn scheme(&self) -> &'static str {
        "github"
    }

    fn canonical(&self) -> String {
        let base = format!("github:{}/{}", self.org, self.repo);
        if let Some(r) = &self.ref_str {
            format!("{base}@{r}")
        } else {
            base
        }
    }

    fn suggested_name(&self) -> Result<String> {
        Ok(self.repo.clone())
    }

    async fn fetch(&self, dest: &Path) -> Result<FetchOutcome> {
        let sha = self.resolve_ref().await?;

        let zip_url = format!(
            "{}/{}/{}/archive/{}.zip",
            self.web_base, self.org, self.repo, sha
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
                .split_once('/')
                .map(|x| x.1)
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

        Ok(FetchOutcome::Fetched {
            path: dest.to_path_buf(),
            sha: Some(sha),
            content_hash: None,
        })
    }
}

pub struct GitHubScheme;

impl SourceScheme for GitHubScheme {
    fn scheme(&self) -> &'static str {
        "github"
    }

    fn parse(&self, value: &str) -> Result<Box<dyn Source>> {
        let (path, ref_str) = split_ref(value);
        let (org, repo) = path.split_once('/').ok_or_else(|| {
            AgixError::InvalidSource(format!(
                "github source must be 'github:org/repo', got: github:{value}"
            ))
        })?;
        Ok(Box::new(GitHubSource::new(org, repo, ref_str)))
    }
}

fn split_ref(s: &str) -> (&str, Option<&str>) {
    match s.find('@') {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
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

    /// Build a minimal zip whose entries mimic the prefix GitHub adds to
    /// `/archive/<sha>.zip` downloads (`<repo>-<sha>/...`).
    fn build_github_style_zip() -> Vec<u8> {
        use std::io::Write as _;
        let mut buf = Vec::new();
        {
            let cursor = std::io::Cursor::new(&mut buf);
            let mut writer = zip::ZipWriter::new(cursor);
            let options: zip::write::FileOptions<()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            writer
                .start_file("repo-abc1234/README.md", options)
                .unwrap();
            writer.write_all(b"# hello").unwrap();
            writer.finish().unwrap();
        }
        buf
    }

    #[tokio::test]
    async fn fetch_uses_web_base_for_archive() {
        let mut server = mockito::Server::new_async().await;
        let sha = "abc123def456abc123def456abc123def456abc1";

        let mock_tag = server
            .mock("GET", "/repos/org/repo/git/ref/tags/v1.0.0")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{"object":{{"sha":"{sha}"}}}}"#))
            .create_async()
            .await;

        let mock_archive = server
            .mock("GET", format!("/org/repo/archive/{sha}.zip").as_str())
            .with_status(200)
            .with_header("content-type", "application/zip")
            .with_body(build_github_style_zip())
            .create_async()
            .await;

        let source = GitHubSource::new_with_base("org", "repo", Some("v1.0.0"), &server.url());
        let dest = tempdir().unwrap();
        let outcome = source.fetch(dest.path()).await.unwrap();

        mock_tag.assert_async().await;
        mock_archive.assert_async().await;

        match outcome {
            FetchOutcome::Fetched {
                sha: fetched_sha, ..
            } => {
                assert_eq!(fetched_sha.as_deref(), Some(sha));
            }
            other => panic!("unexpected FetchOutcome: {other:?}"),
        }
        assert!(dest.path().join("README.md").exists());
    }

    #[test]
    fn env_var_overrides_default_bases() {
        // Guard: serialise env-var mutation in case other tests in this binary
        // also touch it. `resolve_bases` reads the var eagerly.
        let _lock = ENV_GUARD.lock().unwrap();
        std::env::set_var(BASE_URL_ENV_VAR, "http://127.0.0.1:9999");
        let (api, web) = resolve_bases(None);
        std::env::remove_var(BASE_URL_ENV_VAR);
        assert_eq!(api, "http://127.0.0.1:9999");
        assert_eq!(web, "http://127.0.0.1:9999");
    }

    #[test]
    fn explicit_override_wins_over_env_var() {
        let _lock = ENV_GUARD.lock().unwrap();
        std::env::set_var(BASE_URL_ENV_VAR, "http://env-base.invalid");
        let (api, web) = resolve_bases(Some("http://explicit.invalid"));
        std::env::remove_var(BASE_URL_ENV_VAR);
        assert_eq!(api, "http://explicit.invalid");
        assert_eq!(web, "http://explicit.invalid");
    }

    #[test]
    fn defaults_when_env_var_unset() {
        let _lock = ENV_GUARD.lock().unwrap();
        std::env::remove_var(BASE_URL_ENV_VAR);
        let (api, web) = resolve_bases(None);
        assert_eq!(api, DEFAULT_API_BASE);
        assert_eq!(web, DEFAULT_WEB_BASE);
    }

    // Serialise tests that mutate AGIX_GITHUB_BASE_URL — cargo runs tests in
    // the same binary on multiple threads, and env vars are process-wide.
    static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
