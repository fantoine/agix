use std::path::Path;

use crate::core::lock::{InstalledFile, LockFile, LockedPackage};
use crate::core::resolver::Resolver;
use crate::drivers::{driver_for, FetchedPackage, Scope};
use crate::error::Result;
use crate::manifest::agentfile::ProjectManifest;
use crate::sources::{
    github::GitHubSource, git::GitSource, local::LocalSource, SourceSpec,
};

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

            // Marketplace sources: delegate to each target driver (determined by dep.cli).
            if let SourceSpec::Marketplace { marketplace, plugin } = &spec {
                let mut all_files: Vec<InstalledFile> = Vec::new();
                let mut resolved_version: Option<String> = None;
                for cli_name in &dep.cli {
                    match driver_for(cli_name) {
                        None => {
                            crate::output::warn(&format!(
                                "no driver for '{}', skipping marketplace install of '{}'",
                                cli_name, dep.name
                            ));
                        }
                        Some(driver) => {
                            if !driver.detect() {
                                crate::output::warn(&format!(
                                    "'{}' not detected, skipping marketplace install of '{}'",
                                    cli_name, dep.name
                                ));
                                continue;
                            }
                            let (files, version) = driver.install_from_marketplace(marketplace, plugin, &scope)?;
                            if version.is_some() {
                                resolved_version = version;
                            }
                            all_files.extend(files);
                        }
                    }
                }
                lock.upsert(LockedPackage {
                    name: dep.name.clone(),
                    source: dep.source.clone(),
                    sha: None,
                    content_hash: None,
                    version: resolved_version,
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
