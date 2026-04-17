pub async fn run(name: String, scope: &str) -> anyhow::Result<()> {
    let (agentfile_path, lock_path, _scope) = super::agentfile_paths(scope)?;
    let mut manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    manifest.dependencies.remove(&name);
    for cli_deps in manifest.cli_dependencies.values_mut() {
        cli_deps.remove(&name);
    }

    manifest.to_file(&agentfile_path)?;
    crate::core::installer::Installer::uninstall(&name, &lock_path)?;
    crate::output::success(&format!("Removed {name}"));
    Ok(())
}
