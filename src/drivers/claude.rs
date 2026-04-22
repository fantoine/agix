use crate::constants::paths::CLAUDE_DIR;
use crate::core::lock::InstalledFile;
use crate::drivers::{CliDriver, FetchedPackage, Scope};
use crate::error::{AgixError, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ClaudeDriver;

/// Entry as emitted by `claude plugin marketplace list --json`. We only
/// capture the two fields we need: the marketplace's declared `name` (the
/// alias used in `<plugin>@<alias>` when installing) and its `repo`
/// (`<org>/<repo>` for github-sourced marketplaces, absent otherwise).
#[derive(Debug, Deserialize)]
struct MarketplaceEntry {
    name: String,
    #[serde(default)]
    repo: Option<String>,
}

/// Entry as emitted by `claude plugin list --json`. The `id` has the form
/// `<plugin>@<marketplace-alias>` — same string we'd pass to
/// `claude plugin install`.
#[derive(Debug, Deserialize)]
struct PluginEntry {
    id: String,
}

fn list_marketplaces() -> Result<Vec<MarketplaceEntry>> {
    let output = Command::new("claude")
        .args(["plugin", "marketplace", "list", "--json"])
        .output()
        .map_err(|e| AgixError::Other(format!("claude plugin marketplace list failed: {e}")))?;
    if !output.status.success() {
        return Err(AgixError::Other(format!(
            "claude plugin marketplace list exited with {}",
            output.status
        )));
    }
    parse_marketplace_list(&output.stdout)
}

fn parse_marketplace_list(bytes: &[u8]) -> Result<Vec<MarketplaceEntry>> {
    serde_json::from_slice(bytes)
        .map_err(|e| AgixError::Other(format!("failed to parse marketplace list JSON: {e}")))
}

fn list_plugins() -> Result<Vec<PluginEntry>> {
    let output = Command::new("claude")
        .args(["plugin", "list", "--json"])
        .output()
        .map_err(|e| AgixError::Other(format!("claude plugin list failed: {e}")))?;
    if !output.status.success() {
        return Err(AgixError::Other(format!(
            "claude plugin list exited with {}",
            output.status
        )));
    }
    parse_plugin_list(&output.stdout)
}

fn parse_plugin_list(bytes: &[u8]) -> Result<Vec<PluginEntry>> {
    serde_json::from_slice(bytes)
        .map_err(|e| AgixError::Other(format!("failed to parse plugin list JSON: {e}")))
}

/// Find the marketplace alias (`name`) registered with `repo == marketplace`.
/// `None` when no such marketplace is registered yet.
fn find_alias<'a>(entries: &'a [MarketplaceEntry], marketplace: &str) -> Option<&'a str> {
    entries
        .iter()
        .find(|e| e.repo.as_deref() == Some(marketplace))
        .map(|e| e.name.as_str())
}

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
        let agentfile_path = pkg_path.join(crate::constants::manifest::AGENTFILE);
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
            .map(|h| h.join(CLAUDE_DIR).exists())
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
                home.join(CLAUDE_DIR)
            }
            Scope::Local => PathBuf::from(CLAUDE_DIR),
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
        if which::which("claude").is_err() {
            return Err(AgixError::Other(
                "`claude` CLI not found in PATH — install Claude Code first".to_string(),
            ));
        }

        // 1. Resolve the marketplace alias. Claude identifies marketplaces by
        //    a `name` declared in their `marketplace.json` (e.g. `fantoine-plugins`
        //    for repo `fantoine/claude-plugins`), which is what `<plugin>@<alias>`
        //    expects. That alias is not derivable from `org/repo`, so we must
        //    read it back from `plugin marketplace list --json`.
        let alias = {
            let entries = list_marketplaces()?;
            if let Some(alias) = find_alias(&entries, marketplace) {
                println!("Marketplace {marketplace} already registered as '{alias}'");
                alias.to_string()
            } else {
                let output = Command::new("claude")
                    .args(["plugin", "marketplace", "add", marketplace])
                    .output()
                    .map_err(|e| {
                        AgixError::Other(format!("claude plugin marketplace add failed: {e}"))
                    })?;
                if !output.status.success() {
                    return Err(AgixError::Other(format!(
                        "claude plugin marketplace add {marketplace} exited with {}",
                        output.status
                    )));
                }
                let entries = list_marketplaces()?;
                let alias = find_alias(&entries, marketplace)
                    .ok_or_else(|| {
                        AgixError::Other(format!(
                            "marketplace {marketplace} not found after add (claude's marketplace list did not pick it up)"
                        ))
                    })?
                    .to_string();
                println!("Registered marketplace {marketplace} as '{alias}'");
                alias
            }
        };

        // 2. Install the plugin, keyed by alias. Again check first so repeat
        //    runs are a fast no-op instead of relying on string-matching the
        //    CLI's error output.
        let plugin_ref = format!("{plugin}@{alias}");
        let plugins = list_plugins()?;
        if plugins.iter().any(|p| p.id == plugin_ref) {
            println!("Plugin {plugin_ref} already installed");
        } else {
            let output = Command::new("claude")
                .args(["plugin", "install", &plugin_ref])
                .output()
                .map_err(|e| AgixError::Other(format!("claude plugin install failed: {e}")))?;
            if !output.status.success() {
                return Err(AgixError::Other(format!(
                    "claude plugin install {plugin_ref} exited with {}",
                    output.status
                )));
            }
            println!("Installed plugin {plugin_ref}");
        }

        // Claude Code manages its own files; we track only plugin identity in the lock.
        Ok(vec![])
    }

    fn uninstall_marketplace_plugin(&self, marketplace: &str, plugin: &str) -> Result<()> {
        // Resolve alias; if the marketplace is gone, the plugin cannot still
        // be installed through it, so treat it as a silent no-op.
        let entries = list_marketplaces()?;
        let alias = match find_alias(&entries, marketplace) {
            Some(alias) => alias.to_string(),
            None => return Ok(()),
        };
        let plugin_ref = format!("{plugin}@{alias}");

        // Idempotency: if claude doesn't list it, nothing to do.
        let plugins = list_plugins()?;
        if !plugins.iter().any(|p| p.id == plugin_ref) {
            return Ok(());
        }

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
        let candidate = cwd.join(CLAUDE_DIR);
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
    fn find_alias_matches_by_repo_not_by_name() {
        let json = br#"[
            {"name":"fantoine-plugins","source":"github","repo":"fantoine/claude-plugins"},
            {"name":"caveman","source":"github","repo":"JuliusBrussee/caveman"}
        ]"#;
        let entries = parse_marketplace_list(json).unwrap();
        assert_eq!(
            find_alias(&entries, "fantoine/claude-plugins"),
            Some("fantoine-plugins")
        );
        assert_eq!(
            find_alias(&entries, "JuliusBrussee/caveman"),
            Some("caveman")
        );
        assert_eq!(find_alias(&entries, "unknown/repo"), None);
        // Confirms we don't accidentally match on `name`.
        assert_eq!(find_alias(&entries, "fantoine-plugins"), None);
    }

    #[test]
    fn parse_marketplace_list_tolerates_extra_fields() {
        let json = br#"[{"name":"x","source":"github","repo":"a/b","installLocation":"/tmp/x"}]"#;
        let entries = parse_marketplace_list(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "x");
        assert_eq!(entries[0].repo.as_deref(), Some("a/b"));
    }

    #[test]
    fn parse_plugin_list_extracts_id() {
        let json = br#"[
            {"id":"later@fantoine-plugins","version":"1.0","scope":"user","enabled":true},
            {"id":"figma@claude-plugins-official","scope":"user"}
        ]"#;
        let entries = parse_plugin_list(json).unwrap();
        let ids: Vec<_> = entries.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["later@fantoine-plugins", "figma@claude-plugins-official"]
        );
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
