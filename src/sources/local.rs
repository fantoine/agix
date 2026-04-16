use std::path::{Path, PathBuf};

use crate::error::Result;

pub struct FetchedLocal {
    pub content_hash: String,
    pub path: PathBuf,
}

pub struct LocalSource {
    pub path: PathBuf,
}

impl LocalSource {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn fetch(&self, dest: &Path) -> Result<FetchedLocal> {
        copy_dir_all(&self.path, dest)?;
        let content_hash = Self::hash_dir(dest)?;
        Ok(FetchedLocal {
            content_hash,
            path: dest.to_path_buf(),
        })
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

    #[test]
    fn fetch_local_source_copies_files() {
        let src_dir = tempdir().unwrap();
        std::fs::write(src_dir.path().join("skill.md"), "# skill").unwrap();

        let dest_dir = tempdir().unwrap();
        let fetched = LocalSource::new(src_dir.path().to_path_buf())
            .fetch(dest_dir.path())
            .unwrap();

        assert!(dest_dir.path().join("skill.md").exists());
        assert!(!fetched.content_hash.is_empty());
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
