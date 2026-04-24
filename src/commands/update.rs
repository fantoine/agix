use crate::error::AgixError;

pub async fn run(name: Option<String>, global: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;
    if let super::ResolvedScope::Project(ref root) = resolved {
        std::env::set_current_dir(root)?;
    }
    if !agentfile_path.exists() {
        anyhow::bail!("No Agentfile found.");
    }
    let manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;
    let scope = resolved.to_scope();

    if let Some(pkg_name) = name {
        // Update-specific: confirm the package is declared in the manifest
        // before doing anything. If not, fail with an actionable message that
        // lists known packages so the user can spot a typo.
        if !manifest_has_dep(&manifest, &pkg_name) {
            anyhow::bail!(
                "package '{pkg_name}' is not declared in the Agentfile — known packages: [{known}]",
                known = list_known(&manifest).join(", ")
            );
        }

        // Uninstall the current lock entry (if any) so the reinstall is a
        // clean refresh. A missing lock entry is surfaced as a clear error
        // mentioning the package and the known set.
        match crate::core::installer::Installer::uninstall(&pkg_name, &lock_path) {
            Ok(()) => {}
            Err(AgixError::PackageNotFound(pkg)) => {
                // Not in the lock — possible when the dep was added to the
                // Agentfile but never installed, or when the lock is missing
                // entirely. Report clearly with known packages for discoverability.
                let lock = crate::core::lock::LockFile::from_file_or_default(&lock_path);
                let locked: Vec<String> = lock.packages.iter().map(|p| p.name.clone()).collect();
                anyhow::bail!(
                    "package '{pkg}' is not in the lock file — nothing to refresh. \
                     Declared in Agentfile: [{decl}]. In lock: [{in_lock}]",
                    decl = list_known(&manifest).join(", "),
                    in_lock = locked.join(", "),
                );
            }
            Err(e) => return Err(e.into()),
        }

        // Re-install ONLY this dep by filtering the manifest. Otherwise
        // `install_manifest` would re-resolve every declared dep and silently
        // bump things the user didn't ask to update.
        let scoped = filter_manifest_to(&manifest, &pkg_name);
        crate::core::installer::Installer::install_manifest(&scoped, &lock_path, &scope).await?;
    } else {
        // Update-all: uninstall every locked package, then reinstall every
        // manifest dep from scratch. If the lock is missing, this degrades to
        // a plain install-from-manifest (no uninstall work to do).
        let lock = crate::core::lock::LockFile::from_file_or_default(&lock_path);
        let names: Vec<String> = lock.packages.iter().map(|p| p.name.clone()).collect();
        for pkg_name in names {
            // Ignore PackageNotFound here — we just listed them ourselves, but
            // be defensive in case the lock changes under us.
            if let Err(e) = crate::core::installer::Installer::uninstall(&pkg_name, &lock_path) {
                if !matches!(&e, AgixError::PackageNotFound(_)) {
                    return Err(e.into());
                }
            }
        }
        crate::core::installer::Installer::install_manifest(&manifest, &lock_path, &scope).await?;
    }

    crate::output::success("Updated");
    Ok(())
}

/// Return every dep name declared in the manifest across shared and
/// CLI-specific sections (deduped, sorted for stable output).
fn list_known(manifest: &crate::manifest::agentfile::ProjectManifest) -> Vec<String> {
    let mut names: Vec<String> = manifest.dependencies.keys().cloned().collect();
    for cli_deps in manifest.cli_dependencies.values() {
        for name in cli_deps.keys() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }
    names.sort();
    names
}

fn manifest_has_dep(manifest: &crate::manifest::agentfile::ProjectManifest, name: &str) -> bool {
    if manifest.dependencies.contains_key(name) {
        return true;
    }
    manifest
        .cli_dependencies
        .values()
        .any(|deps| deps.contains_key(name))
}

/// Build a manifest clone that contains only the dep whose name matches `name`,
/// preserving its original section(s). Used to scope `install_manifest` to a
/// single dep on `update <name>`.
fn filter_manifest_to(
    manifest: &crate::manifest::agentfile::ProjectManifest,
    name: &str,
) -> crate::manifest::agentfile::ProjectManifest {
    let mut out = manifest.clone();
    out.dependencies.retain(|k, _| k == name);
    for cli_deps in out.cli_dependencies.values_mut() {
        cli_deps.retain(|k, _| k == name);
    }
    // Drop empty CLI sections so the resolver doesn't iterate them needlessly.
    out.cli_dependencies.retain(|_, deps| !deps.is_empty());
    out
}
