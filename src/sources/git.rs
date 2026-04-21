use async_trait::async_trait;
use std::path::Path;

use crate::error::{AgixError, Result};
use crate::sources::{FetchOutcome, Source, SourceScheme};

/// Snapshot of git-related capabilities available in the running agix binary,
/// used by `agix doctor` to surface the diagnostic to users.
///
/// `libgit2` is statically linked so we can always use it; the `git` CLI on
/// `PATH` is purely informational — agix itself doesn't shell out to it, but
/// users often expect the line in diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSupport {
    pub libgit2_version: String,
    pub cli_available: bool,
}

pub fn detect_git_support() -> GitSupport {
    let v = git2::Version::get().libgit2_version();
    GitSupport {
        libgit2_version: format!("{}.{}.{}", v.0, v.1, v.2),
        cli_available: which::which("git").is_ok(),
    }
}

#[derive(Debug, Clone)]
pub struct GitSource {
    url: String,
    ref_str: Option<String>,
}

impl GitSource {
    pub fn new(url: &str, ref_str: Option<&str>) -> Self {
        Self {
            url: url.to_owned(),
            ref_str: ref_str.map(str::to_owned),
        }
    }

    /// Resolve the declared ref (tag / branch / short SHA / HEAD) to a full
    /// commit SHA by opening a lightweight remote connection — no clone.
    /// Mirrors [`crate::sources::github::GitHubSource::resolve_ref`] so
    /// `outdated` can treat plain `git:` URLs the same way.
    ///
    /// Priority: exact 40-char hex SHA → `refs/tags/<r>` → `refs/heads/<r>` →
    /// HEAD (when `ref_str` is `None`).
    pub async fn resolve_ref(&self) -> Result<String> {
        let url = self.url.clone();
        let ref_str = self.ref_str.clone();

        tokio::task::spawn_blocking(move || -> Result<String> {
            if let Some(r) = &ref_str {
                if r.len() == 40 && r.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Ok(r.clone());
                }
            }

            let mut remote = git2::Remote::create_detached(url.as_str())?;
            remote.connect(git2::Direction::Fetch)?;
            let list = remote.list()?;

            match ref_str.as_deref() {
                Some(r) => {
                    let tag = format!("refs/tags/{r}");
                    let tag_peel = format!("refs/tags/{r}^{{}}");
                    let branch = format!("refs/heads/{r}");
                    // Prefer the peeled tag oid when present (annotated tags),
                    // then the branch, then the raw tag object id.
                    let mut tag_oid: Option<String> = None;
                    for head in list {
                        let name = head.name();
                        if name == tag_peel || name == branch {
                            return Ok(head.oid().to_string());
                        }
                        if name == tag {
                            tag_oid = Some(head.oid().to_string());
                        }
                    }
                    if let Some(oid) = tag_oid {
                        return Ok(oid);
                    }
                    Err(AgixError::Other(format!(
                        "ref '{r}' not found on remote {url}"
                    )))
                }
                None => {
                    for head in list {
                        if head.name() == "HEAD" {
                            return Ok(head.oid().to_string());
                        }
                    }
                    Err(AgixError::Other(format!(
                        "could not resolve HEAD on remote {url}"
                    )))
                }
            }
        })
        .await
        .map_err(|e| AgixError::Other(format!("git resolve_ref join error: {e}")))?
    }
}

#[async_trait]
impl Source for GitSource {
    fn scheme(&self) -> &'static str {
        "git"
    }

    fn canonical(&self) -> String {
        let base = format!("git:{}", self.url);
        if let Some(r) = &self.ref_str {
            format!("{base}@{r}")
        } else {
            base
        }
    }

    fn suggested_name(&self) -> Result<String> {
        let last = self
            .url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&self.url);
        Ok(last.trim_end_matches(".git").to_owned())
    }

    fn clone_box(&self) -> Box<dyn Source> {
        Box::new(self.clone())
    }

    async fn fetch(&self, dest: &Path) -> Result<FetchOutcome> {
        let repo = git2::Repository::clone(&self.url, dest)?;

        if let Some(ref ref_str) = self.ref_str {
            let obj = repo.revparse_single(ref_str)?;
            repo.checkout_tree(&obj, None)?;
            repo.set_head_detached(obj.id())?;
        }

        let sha = repo.head()?.peel_to_commit()?.id().to_string();

        Ok(FetchOutcome::Fetched {
            path: dest.to_path_buf(),
            sha: Some(sha),
            content_hash: None,
        })
    }
}

pub struct GitScheme;

impl SourceScheme for GitScheme {
    fn scheme(&self) -> &'static str {
        "git"
    }

    fn parse(&self, value: &str) -> Result<Box<dyn Source>> {
        let (url, ref_str) = split_ref(value);
        Ok(Box::new(GitSource::new(url, ref_str)))
    }
}

fn split_ref(s: &str) -> (&str, Option<&str>) {
    match s.find('@') {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Build a local git repo with a commit tagged `v1.0.0`, returning its
    /// `file://` URL and the commit SHA.
    fn make_local_repo_with_tag() -> (tempfile::TempDir, String, String) {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let sig = git2::Signature::now("test", "test@test.com").unwrap();
        std::fs::write(dir.path().join("README.md"), "# hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("README.md")).unwrap();
        index.write().unwrap();
        let oid = index.write_tree().unwrap();
        let tree = repo.find_tree(oid).unwrap();
        let commit_oid = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        let commit = repo.find_commit(commit_oid).unwrap();
        repo.tag_lightweight("v1.0.0", commit.as_object(), false)
            .unwrap();
        let url = format!("file://{}", dir.path().display());
        (dir, url, commit_oid.to_string())
    }

    #[tokio::test]
    async fn resolve_ref_finds_tag() {
        let (_guard, url, sha) = make_local_repo_with_tag();
        let source = GitSource::new(&url, Some("v1.0.0"));
        let resolved = source.resolve_ref().await.unwrap();
        assert_eq!(resolved, sha);
    }

    #[tokio::test]
    async fn resolve_ref_falls_back_to_head() {
        let (_guard, url, sha) = make_local_repo_with_tag();
        let source = GitSource::new(&url, None);
        let resolved = source.resolve_ref().await.unwrap();
        assert_eq!(resolved, sha);
    }

    #[tokio::test]
    async fn resolve_ref_exact_sha_shortcircuits() {
        // 40-char hex SHA that doesn't exist anywhere — resolve_ref must
        // return it unchanged without contacting any remote.
        let sha = "abc123def456abc123def456abc123def456abc1";
        let source = GitSource::new("file:///nonexistent/repo", Some(sha));
        let resolved = source.resolve_ref().await.unwrap();
        assert_eq!(resolved, sha);
    }

    #[test]
    fn detect_git_support_reports_libgit2_version() {
        let support = detect_git_support();
        // Version string follows "maj.min.rev" shape, always has at least
        // two dots and is made of digits.
        assert_eq!(support.libgit2_version.matches('.').count(), 2);
        assert!(support
            .libgit2_version
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.'));
    }

    #[tokio::test]
    async fn fetch_local_git_repo() {
        let src = tempdir().unwrap();
        let repo = git2::Repository::init(src.path()).unwrap();
        let sig = git2::Signature::now("test", "test@test.com").unwrap();
        std::fs::write(src.path().join("skill.md"), "# skill").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("skill.md")).unwrap();
        index.write().unwrap();
        let oid = index.write_tree().unwrap();
        let tree = repo.find_tree(oid).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();

        let dest = tempdir().unwrap();
        let source = GitSource::new(src.path().to_str().unwrap(), None);
        let outcome = source.fetch(dest.path()).await.unwrap();

        match outcome {
            FetchOutcome::Fetched { sha: Some(sha), .. } => {
                assert!(!sha.is_empty());
            }
            other => panic!("expected Fetched with sha, got {other:?}"),
        }
        assert!(dest.path().join("skill.md").exists());
    }
}
