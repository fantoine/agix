pub async fn run(global: bool, cli: Vec<String>, no_interactive: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let (path, _lock_path, _resolved) = super::agentfile_paths_no_autoinit(global, &cwd)?;
    if path.exists() {
        anyhow::bail!("Already initialized ({})", path.display());
    }
    let selected_clis = crate::ui::prompt::pick_clis(&cli, no_interactive)?;
    crate::manifest::agentfile::ProjectManifest::empty(selected_clis).to_file(&path)?;
    crate::output::success(&format!("Created {}", path.display()));
    Ok(())
}
