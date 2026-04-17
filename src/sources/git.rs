use std::path::Path;

pub struct FetchedGit {
    pub sha: String,
    pub path: std::path::PathBuf,
}

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

    pub fn fetch(&self, dest: &Path) -> crate::error::Result<FetchedGit> {
        let repo = git2::Repository::clone(&self.url, dest)?;

        if let Some(ref ref_str) = self.ref_str {
            let obj = repo.revparse_single(ref_str)?;
            repo.checkout_tree(&obj, None)?;
            repo.set_head_detached(obj.id())?;
        }

        let sha = repo.head()?.peel_to_commit()?.id().to_string();

        Ok(FetchedGit {
            sha,
            path: dest.to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fetch_local_git_repo() {
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
        let fetched = source.fetch(dest.path()).unwrap();

        assert!(!fetched.sha.is_empty());
        assert!(dest.path().join("skill.md").exists());
    }
}
