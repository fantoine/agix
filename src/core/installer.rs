use std::path::Path;

use crate::core::lock::{InstalledFile, LockFile, LockedPackage};
use crate::core::resolver::Resolver;
use crate::drivers::{all_drivers, driver_for, FetchedPackage, Scope};
use crate::error::Result;
use crate::manifest::agentfile::ProjectManifest;
use crate::sources::{parse_source, FetchOutcome};

pub struct Installer;

impl Installer {
    /// Install all dependencies from `manifest`, updating the lock file at `lock_path`.
    pub async fn install_manifest(
        manifest: &ProjectManifest,
        lock_path: &Path,
        scope: &Scope,
    ) -> Result<()> {
        let deps = Resolver::resolve(manifest, &manifest.agix.cli);
        let mut lock = LockFile::from_file_or_default(lock_path);
        let tmp = tempfile::tempdir()?;

        for dep in &deps {
            let fetch_dir = tmp.path().join(&dep.name);
            std::fs::create_dir_all(&fetch_dir)?;

            let source = parse_source(&dep.source)?;
            let outcome = source.fetch(&fetch_dir).await?;

            match outcome {
                FetchOutcome::DelegateToDriver {
                    marketplace,
                    plugin,
                } => {
                    let target_clis: Vec<String> = if dep.cli.is_empty() {
                        all_drivers()
                            .into_iter()
                            .filter(|d| d.detect())
                            .map(|d| d.name().to_string())
                            .collect()
                    } else {
                        dep.cli.clone()
                    };

                    let mut all_files: Vec<InstalledFile> = Vec::new();
                    let mut success_count = 0usize;
                    for cli_name in &target_clis {
                        let driver = match driver_for(cli_name) {
                            Some(d) => d,
                            None => {
                                crate::output::warn(&format!(
                                    "no driver for '{cli_name}', skipping"
                                ));
                                continue;
                            }
                        };
                        if !driver.supports_marketplace() {
                            crate::output::warn(&format!(
                                "marketplace not supported for '{cli_name}', skipping"
                            ));
                            continue;
                        }
                        if !driver.detect() {
                            crate::output::warn(&format!("'{cli_name}' not detected, skipping"));
                            continue;
                        }
                        crate::output::info(&format!(
                            "Installing {plugin} from marketplace {marketplace} via {cli_name}..."
                        ));
                        match driver.install_marketplace_plugin(&marketplace, &plugin, scope) {
                            Ok(files) => {
                                crate::output::success(&format!(
                                    "Plugin '{plugin}' installed for {cli_name}"
                                ));
                                all_files.extend(files);
                                success_count += 1;
                            }
                            Err(e) => {
                                crate::output::warn(&format!("install failed for {cli_name}: {e}"));
                            }
                        }
                    }

                    // If targets were requested but none succeeded, fail the
                    // command so CI pipelines can detect it. If target_clis is
                    // empty (nothing detected), that's already surfaced as
                    // warnings and we preserve the historical Ok path.
                    if !target_clis.is_empty() && success_count == 0 {
                        return Err(crate::error::AgixError::Other(format!(
                            "marketplace plugin '{plugin}' from '{marketplace}' failed to install for all target CLIs"
                        )));
                    }

                    lock.upsert(LockedPackage {
                        name: dep.name.clone(),
                        source: dep.source.clone(),
                        sha: None,
                        content_hash: None,
                        version: None,
                        cli: dep.cli.clone(),
                        scope: scope.as_str().to_owned(),
                        files: all_files,
                    });
                    lock.to_file(lock_path)?;
                }
                FetchOutcome::Fetched {
                    path,
                    sha,
                    content_hash,
                } => {
                    let fetched = FetchedPackage {
                        path,
                        sha,
                        content_hash,
                    };

                    // Install via each target driver.
                    let mut all_files: Vec<InstalledFile> = Vec::new();
                    for cli_name in &dep.cli {
                        let driver = match driver_for(cli_name) {
                            Some(d) => d,
                            None => {
                                crate::output::warn(&format!(
                                    "no driver found for CLI '{}', skipping install of '{}'",
                                    cli_name, dep.name
                                ));
                                continue;
                            }
                        };
                        if !driver.detect() {
                            crate::output::warn(&format!(
                                "CLI '{}' not detected, skipping install of '{}'",
                                cli_name, dep.name
                            ));
                            continue;
                        }
                        let files = driver.install(&dep.name, &fetched, scope)?;
                        all_files.extend(files);
                    }

                    let sha = fetched.sha.clone();
                    let content_hash = fetched.content_hash.clone();

                    lock.upsert(LockedPackage {
                        name: dep.name.clone(),
                        source: dep.source.clone(),
                        sha,
                        content_hash,
                        version: None,
                        cli: dep.cli.clone(),
                        scope: scope.as_str().to_owned(),
                        files: all_files,
                    });
                    lock.to_file(lock_path)?;
                }
            }
        }

        Ok(())
    }

    /// Uninstall a package by name, removing its files and updating the lock file.
    pub fn uninstall(name: &str, lock_path: &Path) -> Result<()> {
        let mut lock = LockFile::from_file_or_default(lock_path);

        let pkg = match lock.find(name) {
            Some(p) => p.clone(),
            None => {
                return Err(crate::error::AgixError::PackageNotFound(name.to_string()));
            }
        };

        let cli_names = pkg.cli.clone();
        let files = pkg.files.clone();

        // Parse the lock's `source` string to decide marketplace vs file-based
        // uninstall. If the source is mangled (hand-edited, older format, …)
        // fall back to file-based uninstall so the user can still clean up.
        // Marketplace-managed plugins in that degraded path may need manual
        // cleanup via the CLI itself — we warn explicitly.
        let marketplace_route = match parse_source(&pkg.source) {
            Ok(source) => source
                .as_marketplace()
                .map(|(m, p)| (m.to_string(), p.to_string())),
            Err(e) => {
                crate::output::warn(&format!(
                    "lock source unparseable for '{name}': {e}; falling back to file-based uninstall"
                ));
                if files.is_empty() {
                    crate::output::warn(&format!(
                        "no tracked files for '{name}' — if this was a marketplace plugin, \
                         uninstall it manually via the relevant CLI"
                    ));
                }
                None
            }
        };

        for cli_name in &cli_names {
            let driver = match driver_for(cli_name) {
                Some(d) => d,
                None => {
                    crate::output::warn(&format!(
                        "no driver found for CLI '{}', skipping uninstall of '{}'",
                        cli_name, name
                    ));
                    continue;
                }
            };
            if let Some((marketplace, plugin)) = &marketplace_route {
                if let Err(e) = driver.uninstall_marketplace_plugin(marketplace, plugin) {
                    crate::output::warn(&format!(
                        "marketplace uninstall failed for {cli_name}: {e}"
                    ));
                }
            } else {
                driver.uninstall(&files)?;
            }
        }

        lock.remove(name);
        lock.to_file(lock_path)?;

        Ok(())
    }
}
