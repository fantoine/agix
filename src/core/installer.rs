use std::path::{Path, PathBuf};

use crate::core::lock::{InstalledFile, LockFile, LockedPackage};
use crate::core::resolver::Resolver;
use crate::drivers::{all_drivers, driver_for, FetchedPackage, Scope};
use crate::error::{AgixError, Result};
use crate::manifest::agentfile::ProjectManifest;
use crate::sources::{git::GitSource, github::GitHubSource, local::LocalSource, SourceSpec};

fn marketplace_base_dir(scope: &Scope, marketplace: &str) -> Result<PathBuf> {
    let (org, repo) = marketplace.split_once('/').ok_or_else(|| {
        AgixError::Other(format!("invalid marketplace identifier: '{marketplace}'"))
    })?;
    let base = match scope {
        Scope::Global => dirs::home_dir()
            .ok_or_else(|| AgixError::Other("cannot determine home directory".to_string()))?
            .join(".agix"),
        Scope::Local => std::env::current_dir()?.join(".agix"),
    };
    Ok(base.join("marketplaces").join(org).join(repo))
}

pub struct Installer;

impl Installer {
    /// Install all dependencies from `manifest`, updating the lock file at `lock_path`.
    pub async fn install_manifest(
        manifest: &ProjectManifest,
        lock_path: &Path,
        scope_str: &str,
    ) -> Result<()> {
        let scope = if scope_str == "global" {
            Scope::Global
        } else {
            Scope::Local
        };

        let deps = Resolver::resolve(manifest, &manifest.agix.cli);
        let mut lock = LockFile::from_file_or_default(lock_path);
        let tmp = tempfile::tempdir()?;

        for dep in &deps {
            let fetch_dir = tmp.path().join(&dep.name);
            std::fs::create_dir_all(&fetch_dir)?;

            let spec = SourceSpec::parse(&dep.source)?;

            // Marketplace sources: fetch the marketplace repo once, then install the plugin.
            if let SourceSpec::Marketplace {
                marketplace,
                plugin,
            } = &spec
            {
                // 1. Resolve and (if needed) fetch the marketplace repo to a permanent directory.
                let marketplace_dir = marketplace_base_dir(&scope, marketplace)?;
                if marketplace_dir.exists() {
                    crate::output::success(&format!(
                        "Marketplace {} already installed",
                        marketplace
                    ));
                } else {
                    crate::output::info(&format!("Installing marketplace {}...", marketplace));
                    let (org, repo) = marketplace.split_once('/').unwrap();
                    GitHubSource::new(org, repo, None)
                        .fetch(&marketplace_dir)
                        .await?;
                    crate::output::success(&format!("Marketplace {} installed", marketplace));
                }

                // 2. Locate the plugin subdirectory inside the marketplace.
                let plugin_dir = marketplace_dir.join(plugin);
                if !plugin_dir.exists() {
                    return Err(AgixError::Other(format!(
                        "plugin '{}' not found in marketplace '{}'",
                        plugin, marketplace
                    )));
                }

                // 3. Determine which CLIs to target.
                //    When dep.cli is empty (e.g. shared dep in a manifest with cli = []),
                //    fall back to every installed CLI driver.
                let target_clis: Vec<String> = if dep.cli.is_empty() {
                    all_drivers()
                        .into_iter()
                        .filter(|d| d.detect())
                        .map(|d| d.name().to_string())
                        .collect()
                } else {
                    dep.cli.clone()
                };

                // 4. Install the plugin for each supported CLI.
                let mut all_files: Vec<InstalledFile> = Vec::new();
                for cli_name in &target_clis {
                    let driver = match driver_for(cli_name) {
                        Some(d) => d,
                        None => {
                            crate::output::warn(&format!(
                                "no driver found for '{}', skipping",
                                cli_name
                            ));
                            continue;
                        }
                    };
                    if !driver.supports_marketplace() {
                        crate::output::warn(&format!(
                            "marketplace not supported for '{}', skipping",
                            cli_name
                        ));
                        continue;
                    }
                    if !driver.detect() {
                        crate::output::warn(&format!(
                            "'{}' not detected, skipping install of plugin '{}'",
                            cli_name, plugin
                        ));
                        continue;
                    }
                    let fetched_plugin = FetchedPackage {
                        path: plugin_dir.clone(),
                        sha: None,
                        content_hash: None,
                    };
                    let files = driver.install(plugin, &fetched_plugin, &scope)?;
                    if !files.is_empty() {
                        crate::output::success(&format!(
                            "Plugin '{}' installed for {}",
                            plugin, cli_name
                        ));
                    }
                    all_files.extend(files);
                }

                lock.upsert(LockedPackage {
                    name: dep.name.clone(),
                    source: dep.source.clone(),
                    sha: None,
                    content_hash: None,
                    version: None,
                    cli: dep.cli.clone(),
                    scope: scope_str.to_owned(),
                    files: all_files,
                });
                lock.to_file(lock_path)?;
                continue;
            }

            // Fetch the package into the temp directory.
            let fetched: FetchedPackage = match &spec {
                SourceSpec::Local { path } => {
                    let f = LocalSource::new(path.clone()).fetch(&fetch_dir)?;
                    FetchedPackage {
                        path: f.path,
                        sha: None,
                        content_hash: Some(f.content_hash),
                    }
                }
                SourceSpec::GitHub { org, repo, ref_str } => {
                    let f = GitHubSource::new(org, repo, ref_str.as_deref())
                        .fetch(&fetch_dir)
                        .await?;
                    FetchedPackage {
                        path: f.path,
                        sha: Some(f.sha),
                        content_hash: None,
                    }
                }
                SourceSpec::Git { url, ref_str } => {
                    let f = GitSource::new(url, ref_str.as_deref()).fetch(&fetch_dir)?;
                    FetchedPackage {
                        path: f.path,
                        sha: Some(f.sha),
                        content_hash: None,
                    }
                }
                SourceSpec::Marketplace { .. } => unreachable!("handled above"),
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
                let files = driver.install(&dep.name, &fetched, &scope)?;
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
                scope: scope_str.to_owned(),
                files: all_files,
            });
            lock.to_file(lock_path)?;
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
            driver.uninstall(&files)?;
        }

        lock.remove(name);
        lock.to_file(lock_path)?;

        Ok(())
    }
}
