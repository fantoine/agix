use crate::drivers::Scope;
use crate::error::AgixError;

pub async fn run(name: String, scope: Scope, cli_filter: Vec<String>) -> anyhow::Result<()> {
    let (agentfile_path, lock_path, _scope) = super::agentfile_paths(scope, false)?;
    let mut manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    // Design note: `remove` is intentionally lenient about `--cli` values
    // (unlike `add`, which errors on unknown CLIs). `remove` is a cleanup
    // operation — users may be deleting legacy sections whose CLI is no
    // longer registered, or typo-correcting a manifest. Mirroring `add`'s
    // strictness would block that. If the section doesn't exist, the removal
    // is simply a no-op for that filter entry. Reviewed: Task 11, Phase B.
    if cli_filter.is_empty() {
        // No filter — remove from shared deps and all CLI sections.
        manifest.dependencies.remove(&name);
        for cli_deps in manifest.cli_dependencies.values_mut() {
            cli_deps.remove(&name);
        }
    } else {
        // Filter — remove only from the specified CLI sections.
        for cli in &cli_filter {
            if let Some(cli_deps) = manifest.cli_dependencies.get_mut(cli) {
                cli_deps.remove(&name);
            }
        }
    }

    manifest.to_file(&agentfile_path)?;

    // Step 4/5 decision (Task 11):
    //   * lock file missing entirely  → warn + no-op (Step 5). The manifest
    //     edit already captures the user's intent; nothing was ever installed.
    //   * lock file present, no entry → error (Step 4). The user asked to
    //     uninstall a specific package and we'd be silently lying if we said
    //     "removed" when nothing in the lock matches.
    if !lock_path.exists() {
        crate::output::warn(&format!(
            "no lock file at {} — manifest updated, nothing to uninstall",
            lock_path.display()
        ));
    } else {
        match crate::core::installer::Installer::uninstall(&name, &lock_path) {
            Ok(()) => {}
            Err(AgixError::PackageNotFound(pkg)) => {
                return Err(anyhow::anyhow!(
                    "package '{pkg}' is not in the lock file — nothing to remove"
                ));
            }
            Err(e) => return Err(e.into()),
        }
    }

    crate::output::success(&format!("Removed {name}"));
    Ok(())
}
