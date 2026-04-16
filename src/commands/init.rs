pub async fn run(global: bool) -> anyhow::Result<()> {
    let (path, _lock_path, _scope) = super::agentfile_paths(global)?;
    if path.exists() {
        anyhow::bail!("Agentfile already exists at {}", path.display());
    }
    crate::manifest::agentfile::ProjectManifest::empty(vec![]).to_file(&path)?;
    crate::output::success(&format!("Created {}", path.display()));
    Ok(())
}
