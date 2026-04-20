use crate::drivers::Scope;

pub async fn run(scope: Scope, cli: Vec<String>, no_interactive: bool) -> anyhow::Result<()> {
    let (path, _lock_path, _scope) = super::agentfile_paths_no_autoinit(scope)?;
    if path.exists() {
        crate::output::warn(&format!("Already initialized ({})", path.display()));
        std::process::exit(1);
    }
    let selected_clis = crate::ui::prompt::pick_clis(&cli, no_interactive)?;
    crate::manifest::agentfile::ProjectManifest::empty(selected_clis).to_file(&path)?;
    crate::output::success(&format!("Created {}", path.display()));
    Ok(())
}
