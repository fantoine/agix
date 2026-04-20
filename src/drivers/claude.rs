use crate::core::lock::InstalledFile;
use crate::drivers::{CliDriver, FetchedPackage, Scope};
use crate::error::{AgixError, Result};
use std::path::{Path, PathBuf};

pub struct ClaudeDriver;

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
            installed.push(InstalledFile {
                dest: dst_path.to_string_lossy().into_owned(),
            });
        }
    }
    Ok(())
}

impl ClaudeDriver {
    pub fn install_with_base(
        &self,
        _pkg_name: &str,
        fetched: &FetchedPackage,
        _scope: &Scope,
        base: &Path,
    ) -> Result<Vec<InstalledFile>> {
        std::fs::create_dir_all(base)?;
        let pkg_path = &fetched.path;
        let mut installed: Vec<InstalledFile> = Vec::new();

        // Map known subdirectories
        let dir_mappings: &[(&str, &str)] = &[
            ("skills", "skills"),
            ("agents", "agents"),
            ("hooks", "hooks"),
            ("mcp-servers", "mcp"),
        ];

        for (src_name, dst_name) in dir_mappings {
            let src_dir = pkg_path.join(src_name);
            if src_dir.is_dir() {
                let dst_dir = base.join(dst_name);
                copy_dir_all(&src_dir, &dst_dir, &mut installed)?;
            }
        }

        // Copy *.md files at the root of the package
        for entry in std::fs::read_dir(pkg_path)? {
            let entry = entry?;
            let src_path = entry.path();
            if src_path.is_file() {
                if let Some(ext) = src_path.extension() {
                    if ext == "md" {
                        let file_name = entry.file_name();
                        let dst_path = base.join(&file_name);
                        std::fs::copy(&src_path, &dst_path)?;
                        installed.push(InstalledFile {
                            dest: dst_path.to_string_lossy().into_owned(),
                        });
                    }
                }
            }
        }

        // Run post-install hook if an Agentfile is present
        let agentfile_path = pkg_path.join("Agentfile");
        if let Some(manifest) =
            crate::manifest::agentfile::PackageManifest::from_file(&agentfile_path)?
        {
            if let Some(hooks) = &manifest.hooks {
                if let Some(post_install_script) = &hooks.post_install {
                    let script_path = pkg_path.join(post_install_script);
                    std::process::Command::new("sh")
                        .arg(&script_path)
                        .status()?;
                }
            }
        }

        Ok(installed)
    }
}

impl CliDriver for ClaudeDriver {
    fn name(&self) -> &str {
        "claude"
    }

    fn detect(&self) -> bool {
        let home_claude = dirs::home_dir()
            .map(|h| h.join(".claude").exists())
            .unwrap_or(false);
        home_claude || which::which("claude").is_ok()
    }

    fn install(
        &self,
        pkg_name: &str,
        fetched: &FetchedPackage,
        scope: &Scope,
    ) -> Result<Vec<InstalledFile>> {
        let base: PathBuf = match scope {
            Scope::Global => {
                let home = dirs::home_dir().ok_or_else(|| {
                    AgixError::Other("cannot determine home directory".to_string())
                })?;
                home.join(".claude")
            }
            Scope::Local => PathBuf::from(".claude"),
        };
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

    fn supports_marketplace(&self) -> bool {
        true
    }

    fn install_marketplace_plugin(
        &self,
        marketplace: &str,
        plugin: &str,
        _scope: &Scope,
    ) -> Result<Vec<InstalledFile>> {
        use std::process::Command;

        if which::which("claude").is_err() {
            return Err(AgixError::Other(
                "`claude` CLI not found in PATH — install Claude Code first".to_string(),
            ));
        }

        // 1. Register the marketplace (idempotent per Claude Code docs).
        let status = Command::new("claude")
            .args(["plugin", "marketplace", "add", marketplace])
            .status()
            .map_err(|e| AgixError::Other(format!("claude plugin marketplace add failed: {e}")))?;
        if !status.success() {
            return Err(AgixError::Other(format!(
                "claude plugin marketplace add {marketplace} exited with {status}"
            )));
        }

        // 2. Install the plugin (Claude's `<plugin>@<marketplace>` syntax).
        let plugin_ref = format!("{plugin}@{marketplace}");
        let status = Command::new("claude")
            .args(["plugin", "install", &plugin_ref])
            .status()
            .map_err(|e| AgixError::Other(format!("claude plugin install failed: {e}")))?;
        if !status.success() {
            return Err(AgixError::Other(format!(
                "claude plugin install {plugin_ref} exited with {status}"
            )));
        }

        // Claude Code manages its own files; we track only plugin identity in the lock.
        Ok(vec![])
    }

    fn uninstall_marketplace_plugin(&self, marketplace: &str, plugin: &str) -> Result<()> {
        use std::process::Command;
        let plugin_ref = format!("{plugin}@{marketplace}");
        let status = Command::new("claude")
            .args(["plugin", "uninstall", &plugin_ref])
            .status()
            .map_err(|e| AgixError::Other(format!("claude plugin uninstall failed: {e}")))?;
        if !status.success() {
            return Err(AgixError::Other(format!(
                "claude plugin uninstall {plugin_ref} exited with {status}"
            )));
        }
        Ok(())
    }

    fn detect_local_config(&self, cwd: &Path) -> Option<PathBuf> {
        let candidate = cwd.join(".claude");
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn installs_skills_to_correct_path() {
        let pkg_dir = tempdir().unwrap();
        let skills_dir = pkg_dir.path().join("skills");
        std::fs::create_dir(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("my-skill.md"), "# skill").unwrap();

        let install_base = tempdir().unwrap();
        let driver = ClaudeDriver;
        let fetched = crate::drivers::FetchedPackage {
            path: pkg_dir.path().to_path_buf(),
            sha: Some("abc".to_string()),
            content_hash: None,
        };

        let files = driver
            .install_with_base(
                "test-pkg",
                &fetched,
                &crate::drivers::Scope::Global,
                install_base.path(),
            )
            .unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].dest.contains("skills"));
        assert!(std::path::Path::new(&files[0].dest).exists());
    }

    #[test]
    fn uninstall_removes_files() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("to_remove.md");
        std::fs::write(&file, "content").unwrap();
        let driver = ClaudeDriver;
        driver
            .uninstall(&[crate::core::lock::InstalledFile {
                dest: file.to_str().unwrap().to_owned(),
            }])
            .unwrap();
        assert!(!file.exists());
    }
}
