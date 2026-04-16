pub async fn run(global: bool) -> anyhow::Result<()> {
    let (agentfile_path, lock_path, scope) = super::agentfile_paths(global)?;
    let manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    for cli in &manifest.agix.cli {
        if crate::drivers::driver_for(cli).map(|d| d.detect()).unwrap_or(false) {
            // detected, fine
        } else {
            crate::output::warn(&format!("CLI '{}' not detected on this system", cli));
        }
    }

    crate::core::installer::Installer::install_manifest(&manifest, &lock_path, scope).await?;
    crate::output::success("Installed");
    Ok(())
}
