use async_trait::async_trait;
use std::path::Path;

use crate::error::Result;
use crate::sources::{FetchOutcome, Source, SourceScheme};

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
