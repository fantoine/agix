pub mod add;
pub mod check;
pub mod doctor;
pub mod export;
pub mod init;
pub mod install;
pub mod list;
pub mod outdated;
pub mod remove;
pub mod update;

use std::path::PathBuf;

/// Return agentfile/lock paths WITHOUT auto-creating missing files.
/// `init` uses this so it can check-and-create rather than being surprised
/// by an auto-created file.
pub fn agentfile_paths_no_autoinit(
    scope: &str,
) -> anyhow::Result<(PathBuf, PathBuf, &'static str)> {
    if scope == "global" {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
        let dir = home.join(".agix");
        std::fs::create_dir_all(&dir)?;
        Ok((dir.join("Agentfile"), dir.join("Agentfile.lock"), "global"))
    } else {
        let dir = std::env::current_dir()?;
        Ok((dir.join("Agentfile"), dir.join("Agentfile.lock"), "local"))
    }
}

/// Return agentfile/lock paths, auto-creating the global Agentfile if missing
/// (with an interactive CLI pick). Used by `add`, `install`, etc.
pub fn agentfile_paths(scope: &str) -> anyhow::Result<(PathBuf, PathBuf, &'static str)> {
    let (agentfile, lock, scope_s) = agentfile_paths_no_autoinit(scope)?;
    if scope == "global" && !agentfile.exists() {
        crate::output::info("No global Agentfile — running first-time setup");
        let picks = crate::ui::prompt::pick_clis(&[], false)?;
        crate::manifest::agentfile::ProjectManifest::empty(picks).to_file(&agentfile)?;
    }
    Ok((agentfile, lock, scope_s))
}
