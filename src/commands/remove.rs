pub async fn run(name: String, scope: &str, cli_filter: Vec<String>) -> anyhow::Result<()> {
    let (agentfile_path, lock_path, _scope) = super::agentfile_paths(scope, false)?;
    let mut manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

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
    crate::core::installer::Installer::uninstall(&name, &lock_path)?;
    crate::output::success(&format!("Removed {name}"));
    Ok(())
}
