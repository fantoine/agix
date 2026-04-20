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

use crate::drivers::Scope;
use std::path::PathBuf;

/// Return agentfile/lock paths WITHOUT auto-creating missing files.
/// `init` uses this so it can check-and-create rather than being surprised
/// by an auto-created file.
pub fn agentfile_paths_no_autoinit(scope: Scope) -> anyhow::Result<(PathBuf, PathBuf, Scope)> {
    if scope.is_global() {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
        let dir = home.join(".agix");
        std::fs::create_dir_all(&dir)?;
        Ok((dir.join("Agentfile"), dir.join("Agentfile.lock"), scope))
    } else {
        let dir = std::env::current_dir()?;
        Ok((dir.join("Agentfile"), dir.join("Agentfile.lock"), scope))
    }
}

/// Return agentfile/lock paths, auto-creating the global Agentfile if missing
/// (with an interactive CLI pick). Used by `add`, `install`, etc.
///
/// `non_interactive` is forwarded to the CLI picker so callers running in
/// non-interactive contexts don't block on a TTY prompt during the first-time
/// global setup. `AGIX_NO_INTERACTIVE=1` also still forces non-interactive
/// (checked inside `pick_clis`).
pub fn agentfile_paths(
    scope: Scope,
    non_interactive: bool,
) -> anyhow::Result<(PathBuf, PathBuf, Scope)> {
    let (agentfile, lock, scope) = agentfile_paths_no_autoinit(scope)?;
    if scope.is_global() && !agentfile.exists() {
        crate::output::info("No global Agentfile — running first-time setup");
        let picks = crate::ui::prompt::pick_clis(&[], non_interactive)?;
        crate::manifest::agentfile::ProjectManifest::empty(picks).to_file(&agentfile)?;
    }
    Ok((agentfile, lock, scope))
}
