use std::path::{Path, PathBuf};
use crate::core::lock::InstalledFile;
use crate::drivers::{CliDriver, FetchedPackage, Scope};
use crate::error::{AgixError, Result};

pub struct CodexDriver;

fn copy_dir_all(src: &Path, dst: &Path, installed: &mut Vec<InstalledFile>) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path, installed)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
            installed.push(InstalledFile { dest: dst_path.to_string_lossy().into_owned() });
        }
    }
    Ok(())
}

impl CodexDriver {
    pub fn install_with_base(
        &self,
        pkg_name: &str,
        fetched: &FetchedPackage,
        _scope: &Scope,
        base: &Path,
    ) -> Result<Vec<InstalledFile>> {
        let pkg_dst = base.join(pkg_name);
        let mut installed: Vec<InstalledFile> = Vec::new();
        copy_dir_all(&fetched.path, &pkg_dst, &mut installed)?;
        Ok(installed)
    }
}

impl CliDriver for CodexDriver {
    fn name(&self) -> &str {
        "codex"
    }

    fn detect(&self) -> bool {
        which::which("codex").is_ok()
            || dirs::home_dir()
                .map(|h| h.join(".codex").exists())
                .unwrap_or(false)
    }

    fn install(&self, pkg_name: &str, fetched: &FetchedPackage, scope: &Scope) -> Result<Vec<InstalledFile>> {
        let home = dirs::home_dir()
            .ok_or_else(|| AgixError::Other("cannot determine home directory".to_string()))?;
        let base = home.join(".codex").join("agix");
        self.install_with_base(pkg_name, fetched, scope, &base)
    }

    fn uninstall(&self, files: &[InstalledFile]) -> Result<()> {
        for f in files {
            let path = Path::new(&f.dest);
            if path.exists() {
                if path.is_dir() {
                    std::fs::remove_dir_all(path)?;
                } else {
                    std::fs::remove_file(path)?;
                }
            }
        }
        Ok(())
    }

    fn install_from_marketplace(&self, _identifier: &str, _scope: &Scope) -> Result<Vec<InstalledFile>> {
        Err(AgixError::Other("codex has no native marketplace".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn installs_to_managed_directory() {
        let pkg_dir = tempdir().unwrap();
        std::fs::write(pkg_dir.path().join("plugin.md"), "# plugin").unwrap();

        let install_base = tempdir().unwrap();
        let driver = CodexDriver;
        let fetched = crate::drivers::FetchedPackage {
            path: pkg_dir.path().to_path_buf(),
            sha: Some("abc".to_string()),
            content_hash: None,
        };
        let files = driver.install_with_base("my-plugin", &fetched, &crate::drivers::Scope::Global, install_base.path()).unwrap();
        assert!(!files.is_empty());
        assert!(install_base.path().join("my-plugin").exists());
    }
}
