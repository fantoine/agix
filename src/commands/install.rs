pub async fn run(global: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;
    if let super::ResolvedScope::Project(ref root) = resolved {
        std::env::set_current_dir(root)?;
    }
    let manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    for cli in &manifest.agix.cli {
        if crate::drivers::driver_for(cli)
            .map(|d| d.detect())
            .unwrap_or(false)
        {
            // detected, fine
        } else {
            crate::output::warn(&format!("CLI '{}' not detected on this system", cli));
        }
    }

    let scope = resolved.to_scope();
    crate::core::installer::Installer::install_manifest(&manifest, &lock_path, &scope).await?;
    crate::output::success("Installed");
    Ok(())
}
