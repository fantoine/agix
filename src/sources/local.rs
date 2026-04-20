use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::error::{AgixError, Result};
use crate::sources::{FetchOutcome, Source, SourceScheme};

pub struct LocalSource {
    pub path: PathBuf,
}

impl LocalSource {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn hash_dir(dir: &Path) -> Result<String> {
        let mut entries: Vec<_> = walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .collect();

        // Sort deterministically by relative path
        entries.sort_by(|a, b| a.path().cmp(b.path()));

        let mut hasher = blake3::Hasher::new();
        for entry in entries {
            let rel = entry.path().strip_prefix(dir).unwrap_or(entry.path());
            hasher.update(rel.to_string_lossy().as_bytes());
            let content = std::fs::read(entry.path())?;
            hasher.update(&content);
        }

        Ok(hasher.finalize().to_hex().to_string())
    }
}

#[async_trait]
impl Source for LocalSource {
    fn scheme(&self) -> &'static str {
        "local"
    }

    fn canonical(&self) -> String {
        format!("local:{}", self.path.display())
    }

    fn suggested_name(&self) -> Result<String> {
        self.path
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .next_back()
            .map(str::to_owned)
            .ok_or_else(|| {
                AgixError::InvalidSource(format!(
                    "cannot derive name from path {}",
                    self.path.display()
                ))
            })
    }

    async fn fetch(&self, dest: &Path) -> Result<FetchOutcome> {
        copy_dir_all(&self.path, dest)?;
        let content_hash = Self::hash_dir(dest)?;
        Ok(FetchOutcome::Fetched {
            path: dest.to_path_buf(),
            sha: None,
            content_hash: Some(content_hash),
        })
    }

    fn local_path(&self) -> Option<&Path> {
        Some(&self.path)
    }
}

pub struct LocalScheme;

impl SourceScheme for LocalScheme {
    fn scheme(&self) -> &'static str {
        "local"
    }

    fn parse(&self, value: &str) -> Result<Box<dyn Source>> {
        Ok(Box::new(LocalSource::new(PathBuf::from(value))))
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn fetch_local_source_copies_files() {
        let src_dir = tempdir().unwrap();
        std::fs::write(src_dir.path().join("skill.md"), "# skill").unwrap();

        let dest_dir = tempdir().unwrap();
        let source = LocalSource::new(src_dir.path().to_path_buf());
        let outcome = source.fetch(dest_dir.path()).await.unwrap();

        assert!(dest_dir.path().join("skill.md").exists());
        match outcome {
            FetchOutcome::Fetched {
                content_hash: Some(h),
                ..
            } => assert!(!h.is_empty()),
            other => panic!("expected Fetched with content_hash, got {other:?}"),
        }
    }

    #[test]
    fn content_hash_changes_on_modification() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("file.md");
        std::fs::write(&file, "original").unwrap();
        let h1 = LocalSource::hash_dir(dir.path()).unwrap();
        std::fs::write(&file, "modified").unwrap();
        let h2 = LocalSource::hash_dir(dir.path()).unwrap();
        assert_ne!(h1, h2);
    }
}
