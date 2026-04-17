pub async fn run(scope: &str) -> anyhow::Result<()> {
    let (path, _lock_path, _scope) = super::agentfile_paths(scope)?;
    if path.exists() {
        crate::output::warn(&format!("Already initialized ({})", path.display()));
        std::process::exit(1);
    }
    crate::manifest::agentfile::ProjectManifest::empty(vec![]).to_file(&path)?;
    crate::output::success(&format!("Created {}", path.display()));
    Ok(())
}
