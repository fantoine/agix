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

use crate::constants::manifest::{AGENTFILE, AGENTFILE_LOCK};
use crate::constants::paths::AGIX_DIR;
use crate::drivers::Scope;
use std::path::{Path, PathBuf};

/// Where the resolved Agentfile lives.
#[derive(Debug, Clone)]
pub enum ResolvedScope {
    /// ~/.agix/Agentfile
    Global,
    /// Directory where the Agentfile was found by walk-up
    Project(PathBuf),
}

impl ResolvedScope {
    pub fn to_scope(&self) -> Scope {
        match self {
            ResolvedScope::Global => Scope::Global,
            ResolvedScope::Project(_) => Scope::Local,
        }
    }
}

/// Walk up from `start` toward `home` (inclusive) looking for an Agentfile.
/// Returns the directory containing it, or None if not found before reaching `home`.
///
/// If `start` is outside the `home` subtree the walk continues toward the
/// filesystem root; it will find nothing and return None (callers then fall
/// back to the global scope), which is the correct behaviour.
fn find_project_root(start: &Path, home: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(AGENTFILE).exists() {
            return Some(current);
        }
        if current == home {
            break;
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => break,
        }
    }
    None
}

/// Walk-up only, no global fallback. Returns the Agentfile path if found, or
/// None. Used by commands that must validate a specific project Agentfile
/// (e.g. `check`) and must not silently switch to the global scope.
pub fn agentfile_path_walk_up_only(cwd: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    find_project_root(cwd, &home).map(|root| root.join(AGENTFILE))
}

/// Return the `~/.agix/` paths without creating the directory.
pub fn global_paths() -> anyhow::Result<(PathBuf, PathBuf)> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let dir = home.join(AGIX_DIR);
    Ok((dir.join(AGENTFILE), dir.join(AGENTFILE_LOCK)))
}

/// Ensure `~/.agix/` exists (creates it if needed). Called only from write paths.
fn ensure_global_dir(af: &Path) -> anyhow::Result<()> {
    if let Some(dir) = af.parent() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Auto-create the global Agentfile with a CLI picker when it is missing.
/// Does nothing if the file already exists.
fn ensure_global_agentfile(af: &Path, non_interactive: bool) -> anyhow::Result<()> {
    if !af.exists() {
        ensure_global_dir(af)?;
        crate::output::info("No global Agentfile — running first-time setup");
        let picks = crate::ui::prompt::pick_clis(&[], non_interactive)?;
        crate::manifest::agentfile::ProjectManifest::empty(picks).to_file(af)?;
    }
    Ok(())
}

/// Used by `init` only — creates in cwd (global=false) or ~/.agix/ (global=true).
/// No walk-up: init is always intentional about where it creates the file.
pub fn agentfile_paths_no_autoinit(
    global: bool,
    cwd: &Path,
) -> anyhow::Result<(PathBuf, PathBuf, ResolvedScope)> {
    if global {
        let (af, lock) = global_paths()?;
        ensure_global_dir(&af)?;
        Ok((af, lock, ResolvedScope::Global))
    } else {
        Ok((
            cwd.join(AGENTFILE),
            cwd.join(AGENTFILE_LOCK),
            ResolvedScope::Project(cwd.to_path_buf()),
        ))
    }
}

/// Walk-up resolution for all commands except `init`.
///
/// - `global=true` → force ~/.agix/
/// - `global=false` → walk up from cwd to $HOME; fallback to ~/.agix/
///
/// Auto-creates the global Agentfile on first use (interactive CLI pick).
pub fn agentfile_paths(
    global: bool,
    cwd: &Path,
    non_interactive: bool,
) -> anyhow::Result<(PathBuf, PathBuf, ResolvedScope)> {
    if global {
        let (af, lock) = global_paths()?;
        ensure_global_agentfile(&af, non_interactive)?;
        return Ok((af, lock, ResolvedScope::Global));
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    if let Some(root) = find_project_root(cwd, &home) {
        let af = root.join(AGENTFILE);
        let lock = root.join(AGENTFILE_LOCK);
        return Ok((af, lock, ResolvedScope::Project(root)));
    }

    // Fallback: global
    let (af, lock) = global_paths()?;
    ensure_global_agentfile(&af, non_interactive)?;
    Ok((af, lock, ResolvedScope::Global))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn find_project_root_finds_parent() {
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("Agentfile"), "[agix]\ncli=[]\n").unwrap();
        let sub = root.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        assert_eq!(
            find_project_root(&sub, root.path()),
            Some(root.path().to_path_buf())
        );
    }

    #[test]
    fn find_project_root_finds_self() {
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("Agentfile"), "[agix]\ncli=[]\n").unwrap();
        assert_eq!(
            find_project_root(root.path(), root.path()),
            Some(root.path().to_path_buf())
        );
    }

    #[test]
    fn find_project_root_returns_none_when_missing() {
        let root = tempdir().unwrap();
        let sub = root.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        assert_eq!(find_project_root(&sub, root.path()), None);
    }

    #[test]
    fn find_project_root_inner_wins_over_outer() {
        let outer = tempdir().unwrap();
        let inner = outer.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        std::fs::write(outer.path().join("Agentfile"), "[agix]\ncli=[]\n").unwrap();
        std::fs::write(inner.join("Agentfile"), "[agix]\ncli=[]\n").unwrap();
        assert_eq!(find_project_root(&inner, outer.path()), Some(inner.clone()));
    }
}
