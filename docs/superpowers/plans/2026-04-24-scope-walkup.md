# Scope Walk-Up Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the explicit `--scope local|global` flag with context-aware walk-up resolution: find the nearest `Agentfile` walking up from cwd to `$HOME`, fall back to global; add `-g/--global` as the only override.

**Architecture:** Add `ResolvedScope` + `find_project_root` in `src/commands/mod.rs`. Update `agentfile_paths` to drive walk-up. Change every command `run()` from `scope: Scope` to `global: bool`. When project scope resolved, `set_current_dir(root)` so the driver's relative-path install (`./` + `Scope::Local`) targets the project root.

**Tech Stack:** Rust 2021, `dirs` crate (home dir), `assert_cmd` + `tempfile` (integration tests).

---

## File Map

| File | Change |
|---|---|
| `src/commands/mod.rs` | Add `ResolvedScope`, `find_project_root`, update `agentfile_paths` / `agentfile_paths_no_autoinit` |
| `src/output.rs` | Add `stderr_is_tty`, `scope_header` |
| `src/main.rs` | Replace `scope: Scope` → `global: bool` on all subcommands |
| `src/commands/add.rs` | `run(global: bool, ...)`, cwd walk-up, set_current_dir |
| `src/commands/install.rs` | Same pattern |
| `src/commands/remove.rs` | Same pattern |
| `src/commands/update.rs` | Same pattern |
| `src/commands/list.rs` | Same pattern + scope_header |
| `src/commands/outdated.rs` | Same pattern + scope_header |
| `src/commands/export.rs` | Same pattern + scope_header |
| `src/commands/init.rs` | `global: bool`, uses `agentfile_paths_no_autoinit` (no walk-up) |
| `src/drivers/mod.rs` | Keep `Scope` enum, add `ResolvedScope::to_scope()` helper |
| `tests/scope_test.rs` | New: walk-up integration tests |
| `tests/add_test.rs` `tests/install_test.rs` `tests/remove_test.rs` `tests/update_test.rs` `tests/list_test.rs` `tests/outdated_test.rs` `tests/init_test.rs` | Replace `--scope local/global` → remove/`-g` |
| `Cargo.toml` | Bump 0.1.4 → 0.2.0 |

---

### Task 1: ResolvedScope + find_project_root + agentfile_paths

**Files:**
- Modify: `src/commands/mod.rs`

- [ ] **Step 1: Write failing unit tests for `find_project_root`**

Add at the bottom of `src/commands/mod.rs`:

```rust
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
        assert_eq!(find_project_root(&sub, root.path()), Some(root.path().to_path_buf()));
    }

    #[test]
    fn find_project_root_finds_self() {
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("Agentfile"), "[agix]\ncli=[]\n").unwrap();
        assert_eq!(find_project_root(root.path(), root.path()), Some(root.path().to_path_buf()));
    }

    #[test]
    fn find_project_root_returns_none_when_missing() {
        let root = tempdir().unwrap();
        let sub = root.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        // no Agentfile anywhere; root acts as $HOME boundary
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
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test --lib commands::tests 2>&1 | tail -5
```

Expected: FAIL — `find_project_root` not found.

- [ ] **Step 3: Replace `src/commands/mod.rs` with the new implementation**

```rust
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

/// Walk up from `start` to `home` (inclusive) looking for an Agentfile.
/// Returns the directory containing it, or None if not found.
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

fn global_agentfile_paths() -> anyhow::Result<(PathBuf, PathBuf)> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    let dir = home.join(AGIX_DIR);
    std::fs::create_dir_all(&dir)?;
    Ok((dir.join(AGENTFILE), dir.join(AGENTFILE_LOCK)))
}

/// Used by `init` only — creates in cwd (global=false) or ~/.agix/ (global=true).
/// No walk-up: init is always intentional about where it creates the file.
pub fn agentfile_paths_no_autoinit(
    global: bool,
    cwd: &Path,
) -> anyhow::Result<(PathBuf, PathBuf, ResolvedScope)> {
    if global {
        let (af, lock) = global_agentfile_paths()?;
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
/// - `global=true` → force ~/.agix/
/// - `global=false` → walk up from cwd to $HOME; fallback to ~/.agix/
/// Auto-creates the global Agentfile on first use (interactive CLI pick).
pub fn agentfile_paths(
    global: bool,
    cwd: &Path,
    non_interactive: bool,
) -> anyhow::Result<(PathBuf, PathBuf, ResolvedScope)> {
    if global {
        let (af, lock) = global_agentfile_paths()?;
        if !af.exists() {
            crate::output::info("No global Agentfile — running first-time setup");
            let picks = crate::ui::prompt::pick_clis(&[], non_interactive)?;
            crate::manifest::agentfile::ProjectManifest::empty(picks).to_file(&af)?;
        }
        return Ok((af, lock, ResolvedScope::Global));
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    if let Some(root) = find_project_root(cwd, &home) {
        let af = root.join(AGENTFILE);
        let lock = root.join(AGENTFILE_LOCK);
        return Ok((af, lock, ResolvedScope::Project(root)));
    }

    // Fallback: global
    let (af, lock) = global_agentfile_paths()?;
    if !af.exists() {
        crate::output::info("No global Agentfile — running first-time setup");
        let picks = crate::ui::prompt::pick_clis(&[], non_interactive)?;
        crate::manifest::agentfile::ProjectManifest::empty(picks).to_file(&af)?;
    }
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
        assert_eq!(find_project_root(&sub, root.path()), Some(root.path().to_path_buf()));
    }

    #[test]
    fn find_project_root_finds_self() {
        let root = tempdir().unwrap();
        std::fs::write(root.path().join("Agentfile"), "[agix]\ncli=[]\n").unwrap();
        assert_eq!(find_project_root(root.path(), root.path()), Some(root.path().to_path_buf()));
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
```

- [ ] **Step 4: Run unit tests**

```bash
cargo test --lib commands::tests 2>&1 | tail -5
```

Expected: 4 passed.

- [ ] **Step 5: Build to catch compile errors before proceeding**

```bash
cargo build 2>&1 | grep "^error" | head -20
```

Expected: errors on callers of the old `agentfile_paths` — that's OK, we fix them in Task 3+.

---

### Task 2: scope_header + TTY detection in output.rs

**Files:**
- Modify: `src/output.rs`

- [ ] **Step 1: Add the new functions**

Replace the entire file:

```rust
pub fn success(msg: &str) {
    println!("  \u{2713} {}", msg);
}
pub fn warn(msg: &str) {
    eprintln!("  \u{26a0} {}", msg);
}
pub fn info(msg: &str) {
    println!("  {}", msg);
}

pub fn stderr_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stderr().is_terminal()
}

/// Print the resolved scope as a dim header line to stderr.
/// Suppressed when stderr is not a TTY (CI, pipes).
pub fn scope_header(agentfile: &std::path::Path, global: bool) {
    if !stderr_is_tty() {
        return;
    }
    let label = if global { "global" } else { "project" };
    eprintln!("  \x1b[2mUsing {}   ({})\x1b[0m", agentfile.display(), label);
}
```

- [ ] **Step 2: Build to check**

```bash
cargo build 2>&1 | grep "^error" | head -5
```

Expected: no errors in output.rs.

---

### Task 3: Update main.rs — replace scope with --global flag

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Write a failing integration test that verifies `--scope` is rejected**

Add to `tests/init_test.rs` temporarily (we'll move it to scope_test.rs in Task 7):

```rust
#[test]
fn scope_flag_removed_global_flag_accepted() {
    let home = tempfile::tempdir().unwrap();
    // --scope global should now be rejected
    helpers::cmd_non_interactive(home.path())
        .args(["list", "--scope", "global"])
        .assert()
        .failure();
    // -g should work (falls back to global auto-init)
    helpers::cmd_non_interactive(home.path())
        .args(["list", "-g"])
        .assert()
        .success();
}
```

- [ ] **Step 2: Run to confirm it fails**

```bash
cargo test --test init_test scope_flag_removed 2>&1 | tail -5
```

Expected: FAIL (currently `--scope global` succeeds).

- [ ] **Step 3: Replace src/main.rs**

```rust
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "agix",
    about = "Agent Graph IndeX \u{2014} package manager for AI CLI tools",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        #[arg(short = 'g', long)]
        global: bool,
        /// Pre-select CLIs (skips the interactive menu). Repeatable.
        #[arg(long, num_args = 1..)]
        cli: Vec<String>,
        #[arg(long)]
        no_interactive: bool,
    },
    Install {
        #[arg(short = 'g', long)]
        global: bool,
    },
    Add {
        /// Source type: local | github | git | marketplace
        source_type: String,
        /// Source value (path, org/repo, URL, <org/repo>@<plugin>, ...)
        source_value: String,
        #[arg(short = 'g', long)]
        global: bool,
        #[arg(long, num_args = 1..)]
        cli: Vec<String>,
        #[arg(long)]
        version: Option<String>,
    },
    Remove {
        name: String,
        #[arg(short = 'g', long)]
        global: bool,
        #[arg(long, num_args = 1..)]
        cli: Vec<String>,
    },
    Update {
        name: Option<String>,
        #[arg(short = 'g', long)]
        global: bool,
    },
    List {
        #[arg(short = 'g', long)]
        global: bool,
    },
    Outdated {
        #[arg(short = 'g', long)]
        global: bool,
    },
    Check,
    Doctor,
    Export {
        #[arg(short = 'g', long)]
        global: bool,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        output: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init {
            global,
            cli,
            no_interactive,
        } => agix::commands::init::run(global, cli, no_interactive).await,
        Commands::Install { global } => agix::commands::install::run(global).await,
        Commands::Add {
            source_type,
            source_value,
            global,
            cli,
            version,
        } => agix::commands::add::run(source_type, source_value, global, cli, version).await,
        Commands::Remove { name, global, cli } => {
            agix::commands::remove::run(name, global, cli).await
        }
        Commands::Update { name, global } => agix::commands::update::run(name, global).await,
        Commands::List { global } => agix::commands::list::run(global).await,
        Commands::Outdated { global } => agix::commands::outdated::run(global).await,
        Commands::Check => agix::commands::check::run().await,
        Commands::Doctor => agix::commands::doctor::run().await,
        Commands::Export { global, all, output } => {
            agix::commands::export::run(global, all, output).await
        }
    }
}
```

- [ ] **Step 4: Build to see remaining errors**

```bash
cargo build 2>&1 | grep "^error" | head -30
```

Expected: errors on each command's `run()` signature — expected, fixed in Tasks 4-6.

---

### Task 4: Update write commands (add, install, remove, update)

**Files:**
- Modify: `src/commands/add.rs`
- Modify: `src/commands/install.rs`
- Modify: `src/commands/remove.rs`
- Modify: `src/commands/update.rs`

The pattern for all write commands:
1. `scope: Scope` → `global: bool`
2. Remove `use crate::drivers::Scope;` (no longer needed directly)
3. `let cwd = std::env::current_dir()?;`
4. `let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;`
5. `if let super::ResolvedScope::Project(ref root) = resolved { std::env::set_current_dir(root)?; }`
6. Replace `scope` with `resolved.to_scope()` where passed to installer

- [ ] **Step 1: Update src/commands/install.rs**

```rust
pub async fn run(global: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;
    if let super::ResolvedScope::Project(ref root) = resolved {
        std::env::set_current_dir(root)?;
    }
    if !agentfile_path.exists() {
        anyhow::bail!(
            "no Agentfile at {} — run `agix init` first",
            agentfile_path.display()
        );
    }
    let manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;
    let scope = resolved.to_scope();
    crate::core::installer::Installer::install_manifest(&manifest, &lock_path, &scope).await?;
    Ok(())
}
```

- [ ] **Step 2: Replace src/commands/remove.rs**

```rust
use crate::error::AgixError;

pub async fn run(name: String, global: bool, cli_filter: Vec<String>) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;
    if let super::ResolvedScope::Project(ref root) = resolved {
        std::env::set_current_dir(root)?;
    }
    let mut manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    if cli_filter.is_empty() {
        manifest.dependencies.remove(&name);
        for cli_deps in manifest.cli_dependencies.values_mut() {
            cli_deps.remove(&name);
        }
    } else {
        for cli in &cli_filter {
            if let Some(cli_deps) = manifest.cli_dependencies.get_mut(cli) {
                cli_deps.remove(&name);
            }
        }
    }

    manifest.to_file(&agentfile_path)?;

    if !lock_path.exists() {
        crate::output::warn(&format!(
            "no lock file at {} — manifest updated, nothing to uninstall",
            lock_path.display()
        ));
    } else {
        match crate::core::installer::Installer::uninstall(&name, &lock_path) {
            Ok(()) => {}
            Err(AgixError::PackageNotFound(pkg)) => {
                return Err(anyhow::anyhow!(
                    "package '{pkg}' is not in the lock file — nothing to remove"
                ));
            }
            Err(e) => return Err(e.into()),
        }
    }

    crate::output::success(&format!("Removed {name}"));
    Ok(())
}
```

- [ ] **Step 3: Replace src/commands/update.rs**

```rust
use crate::error::AgixError;

pub async fn run(name: Option<String>, global: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;
    if let super::ResolvedScope::Project(ref root) = resolved {
        std::env::set_current_dir(root)?;
    }
    if !agentfile_path.exists() {
        anyhow::bail!("No Agentfile found.");
    }
    let manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;
    let scope = resolved.to_scope();

    if let Some(pkg_name) = name {
        if !manifest_has_dep(&manifest, &pkg_name) {
            anyhow::bail!(
                "package '{pkg_name}' is not declared in the Agentfile — known packages: [{known}]",
                known = list_known(&manifest).join(", ")
            );
        }

        match crate::core::installer::Installer::uninstall(&pkg_name, &lock_path) {
            Ok(()) => {}
            Err(AgixError::PackageNotFound(pkg)) => {
                let lock = crate::core::lock::LockFile::from_file_or_default(&lock_path);
                let locked: Vec<String> = lock.packages.iter().map(|p| p.name.clone()).collect();
                anyhow::bail!(
                    "package '{pkg}' is not in the lock file — nothing to refresh. \
                     Declared in Agentfile: [{decl}]. In lock: [{in_lock}]",
                    decl = list_known(&manifest).join(", "),
                    in_lock = locked.join(", "),
                );
            }
            Err(e) => return Err(e.into()),
        }

        let scoped = filter_manifest_to(&manifest, &pkg_name);
        crate::core::installer::Installer::install_manifest(&scoped, &lock_path, &scope).await?;
    } else {
        let lock = crate::core::lock::LockFile::from_file_or_default(&lock_path);
        let names: Vec<String> = lock.packages.iter().map(|p| p.name.clone()).collect();
        for pkg_name in names {
            if let Err(e) = crate::core::installer::Installer::uninstall(&pkg_name, &lock_path) {
                if !matches!(&e, AgixError::PackageNotFound(_)) {
                    return Err(e.into());
                }
            }
        }
        crate::core::installer::Installer::install_manifest(&manifest, &lock_path, &scope).await?;
    }

    crate::output::success("Updated");
    Ok(())
}

fn list_known(manifest: &crate::manifest::agentfile::ProjectManifest) -> Vec<String> {
    let mut names: Vec<String> = manifest.dependencies.keys().cloned().collect();
    for cli_deps in manifest.cli_dependencies.values() {
        for name in cli_deps.keys() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }
    names.sort();
    names
}

fn manifest_has_dep(manifest: &crate::manifest::agentfile::ProjectManifest, name: &str) -> bool {
    if manifest.dependencies.contains_key(name) {
        return true;
    }
    manifest
        .cli_dependencies
        .values()
        .any(|deps| deps.contains_key(name))
}

fn filter_manifest_to(
    manifest: &crate::manifest::agentfile::ProjectManifest,
    name: &str,
) -> crate::manifest::agentfile::ProjectManifest {
    let mut out = manifest.clone();
    out.dependencies.retain(|k, _| k == name);
    for cli_deps in out.cli_dependencies.values_mut() {
        cli_deps.retain(|k, _| k == name);
    }
    out.cli_dependencies.retain(|_, deps| !deps.is_empty());
    out
}
```

- [ ] **Step 4: Replace src/commands/add.rs**

```rust
use crate::manifest::agentfile::Dependency;

pub async fn run(
    source_type: String,
    source_value: String,
    global: bool,
    cli_filter: Vec<String>,
    version: Option<String>,
) -> anyhow::Result<()> {
    let valid_source_types = crate::sources::scheme_names();
    if !valid_source_types.contains(&source_type.as_str()) {
        anyhow::bail!(
            "unknown source type '{}' — expected one of: {}",
            source_type,
            valid_source_types.join(", ")
        );
    }

    if !cli_filter.is_empty() {
        let known_names: Vec<String> = crate::drivers::all_drivers()
            .iter()
            .map(|d| d.name().to_string())
            .collect();
        for cli in &cli_filter {
            if !known_names.iter().any(|k| k == cli) {
                anyhow::bail!(
                    "unknown CLI '{}' — expected one of: {}",
                    cli,
                    known_names.join(", ")
                );
            }
        }
    }

    let source = format!("{}:{}", source_type, source_value);

    let cwd = std::env::current_dir()?;
    let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;
    if let super::ResolvedScope::Project(ref root) = resolved {
        std::env::set_current_dir(root)?;
    }
    let scope = resolved.to_scope();

    if !agentfile_path.exists() {
        anyhow::bail!(
            "No Agentfile at {}. Run `agix init` first.",
            agentfile_path.display()
        );
    }

    let mut manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    let src = crate::sources::parse_source(&source)?;
    let name = src.suggested_name()?;

    let dep = Dependency {
        source: crate::sources::SourceBox::from(src),
        version,
        exclude: None,
    };

    if cli_filter.is_empty() {
        if manifest.dependencies.contains_key(&name) {
            crate::output::warn(&format!(
                "dependency '{name}' already in [dependencies] — overwriting"
            ));
        }
        manifest.dependencies.insert(name.clone(), dep.clone());
    } else {
        for cli in &cli_filter {
            let entry = manifest.cli_dependencies.entry(cli.clone()).or_default();
            if entry.contains_key(&name) {
                crate::output::warn(&format!(
                    "dependency '{name}' already in [{cli}.dependencies] — overwriting"
                ));
            }
            entry.insert(name.clone(), dep.clone());
        }
    }

    manifest.to_file(&agentfile_path)?;

    let scoped_manifest = manifest.single_dep_scoped(&name, dep, &cli_filter);
    crate::core::installer::Installer::install_manifest(&scoped_manifest, &lock_path, &scope)
        .await?;
    crate::output::success(&format!("Added {name}"));
    Ok(())
}
```

- [ ] **Step 5: Build and run existing tests to check no regressions**

```bash
cargo build 2>&1 | grep "^error" | head -20
cargo test --test add_test 2>&1 | tail -5
cargo test --test install_test 2>&1 | tail -5
cargo test --test remove_test 2>&1 | tail -5
cargo test --test update_test 2>&1 | tail -5
```

Expected: compile errors only from list/outdated/export/init (not yet updated). The above test files will fail because they still use `--scope` — that's fixed in Task 8.

---

### Task 5: Update read/utility commands (list, outdated, export)

**Files:**
- Modify: `src/commands/list.rs`
- Modify: `src/commands/outdated.rs`
- Modify: `src/commands/export.rs`

Read commands additionally call `scope_header`. No `set_current_dir` needed (they don't install files).

- [ ] **Step 1: Update src/commands/list.rs**

Replace the signature and opening lines:

```rust
use std::collections::BTreeMap;

use crate::core::lock::{LockFile, LockedPackage};
use crate::manifest::agentfile::{Dependency, ProjectManifest};

pub async fn run(global: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;
    crate::output::scope_header(&agentfile_path, matches!(resolved, super::ResolvedScope::Global));

    if !agentfile_path.exists() {
        anyhow::bail!(
            "no Agentfile at {} — run `agix init` first",
            agentfile_path.display()
        );
    }
    // ... rest unchanged ...
```

- [ ] **Step 2: Update src/commands/outdated.rs**

Replace only the `run` function signature and its first 4 lines. The `check_outdated`, `render`, and all other functions are unchanged.

Find `pub async fn run(scope: Scope)` and replace with:

```rust
pub async fn run(global: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;
    let is_global = matches!(resolved, super::ResolvedScope::Global);
    crate::output::scope_header(&agentfile_path, is_global);

    if !agentfile_path.exists() {
        anyhow::bail!(
            "no Agentfile at {} — run `agix init` first",
            agentfile_path.display()
        );
    }
    // rest of function unchanged from here
```

Also remove `use crate::drivers::Scope;` from the top.

- [ ] **Step 3: Replace src/commands/export.rs `run` function only**

Replace the `run` function (lines 22–99 of the current file). All helper functions (`rewrite_local_deps`, `rewrite_local_sources_in_lock`, `copy_dir_into_zip`) are unchanged.

```rust
pub async fn run(global: bool, all: bool, output: Option<String>) -> Result<()> {
    if all {
        bail!("--all is not yet implemented — export one scope at a time");
    }

    let cwd = std::env::current_dir()?;
    let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;
    if let super::ResolvedScope::Project(ref root) = resolved {
        std::env::set_current_dir(root)?;
    }
    let is_global = matches!(resolved, super::ResolvedScope::Global);
    crate::output::scope_header(&agentfile_path, is_global);

    if !agentfile_path.exists() {
        bail!(
            "no Agentfile at {} — run `agix init` first",
            agentfile_path.display()
        );
    }

    let output_path = output.unwrap_or_else(|| "agix-export.zip".to_string());

    let manifest = ProjectManifest::from_file(&agentfile_path)?;
    let mut rewritten = manifest.clone();

    let mut local_sources: HashMap<String, PathBuf> = HashMap::new();

    rewrite_local_deps(&mut rewritten.dependencies, &mut local_sources);
    for deps in rewritten.cli_dependencies.values_mut() {
        rewrite_local_deps(deps, &mut local_sources);
    }

    let file = std::fs::File::create(&output_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let rewritten_agentfile = rewritten.to_toml_string()?;
    zip.start_file(AGENTFILE, options)?;
    zip.write_all(rewritten_agentfile.as_bytes())?;

    if lock_path.exists() {
        let lock_text = std::fs::read_to_string(&lock_path)?;
        let rewritten_lock = rewrite_local_sources_in_lock(&lock_text, &local_sources)?;
        zip.start_file(AGENTFILE_LOCK, options)?;
        zip.write_all(rewritten_lock.as_bytes())?;
    }

    for (name, abs_path) in &local_sources {
        let source_dir = if abs_path.is_absolute() {
            abs_path.clone()
        } else {
            agentfile_path
                .parent()
                .map(|p| p.join(abs_path))
                .unwrap_or_else(|| abs_path.clone())
        };
        if source_dir.exists() {
            copy_dir_into_zip(&mut zip, &source_dir, name, options)?;
        } else {
            crate::output::warn(&format!(
                "local source for '{}' not found at {} — skipping",
                name,
                source_dir.display()
            ));
        }
    }

    zip.finish()?;
    crate::output::success(&format!("Exported to {output_path}"));
    Ok(())
}
```

Also remove `use crate::drivers::Scope;` from the top of the file.

- [ ] **Step 4: Build**

```bash
cargo build 2>&1 | grep "^error" | head -20
```

Expected: only `init.rs` errors remain.

---

### Task 6: Update init command

**Files:**
- Modify: `src/commands/init.rs`

`init` uses `agentfile_paths_no_autoinit` (no walk-up — always creates in cwd or global).

- [ ] **Step 1: Replace src/commands/init.rs**

```rust
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
```

- [ ] **Step 2: Full build — must be clean**

```bash
cargo build 2>&1 | grep "^error"
```

Expected: no errors.

- [ ] **Step 3: Run full test suite (many will fail — that's expected)**

```bash
cargo test 2>&1 | tail -10
```

Note which tests fail and confirm they all relate to `--scope` in test args (not compile errors or logic bugs).

---

### Task 7: New integration tests — tests/scope_test.rs

**Files:**
- Create: `tests/scope_test.rs`

These tests exercise the walk-up logic end-to-end via the CLI binary.

- [ ] **Step 1: Create tests/scope_test.rs**

```rust
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

mod helpers;

const MINIMAL_AGENTFILE: &str = "[agix]\ncli = []\n\n[dependencies]\n";
const MINIMAL_LOCK: &str = "";

// ---------------------------------------------------------------------------
// Walk-up: Agentfile in parent, cwd is a subdirectory
// ---------------------------------------------------------------------------

#[test]
fn walkup_finds_agentfile_in_parent_directory() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    let sub = project.path().join("src");
    fs::create_dir(&sub).unwrap();
    fs::write(project.path().join("Agentfile"), MINIMAL_AGENTFILE).unwrap();
    fs::write(project.path().join("Agentfile.lock"), MINIMAL_LOCK).unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(&sub)
        .arg("list")
        .assert()
        .success()
        // scope_header prints the project Agentfile path to stderr
        .stderr(predicate::str::contains(
            project.path().to_str().unwrap(),
        ));
}

// ---------------------------------------------------------------------------
// Walk-up: no Agentfile anywhere → fallback to global
// ---------------------------------------------------------------------------

#[test]
fn walkup_falls_back_to_global_when_no_agentfile() {
    let home = tempdir().unwrap();
    let agix_dir = home.path().join(".agix");
    fs::create_dir_all(&agix_dir).unwrap();
    fs::write(agix_dir.join("Agentfile"), MINIMAL_AGENTFILE).unwrap();
    fs::write(agix_dir.join("Agentfile.lock"), MINIMAL_LOCK).unwrap();

    let cwd = tempdir().unwrap(); // no Agentfile here or in parents (different tmpdir)

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("list")
        .assert()
        .success()
        .stderr(predicate::str::contains(".agix/Agentfile"));
}

// ---------------------------------------------------------------------------
// Walk-up: -g forces global even when project Agentfile exists
// ---------------------------------------------------------------------------

#[test]
fn global_flag_overrides_walkup_inside_project() {
    let home = tempdir().unwrap();
    let agix_dir = home.path().join(".agix");
    fs::create_dir_all(&agix_dir).unwrap();
    fs::write(agix_dir.join("Agentfile"), MINIMAL_AGENTFILE).unwrap();
    fs::write(agix_dir.join("Agentfile.lock"), MINIMAL_LOCK).unwrap();

    let project = tempdir().unwrap();
    fs::write(project.path().join("Agentfile"), MINIMAL_AGENTFILE).unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(project.path())
        .args(["list", "-g"])
        .assert()
        .success()
        .stderr(predicate::str::contains(".agix/Agentfile"));
}

// ---------------------------------------------------------------------------
// Nested projects: inner Agentfile wins over outer
// ---------------------------------------------------------------------------

#[test]
fn nested_project_inner_agentfile_wins() {
    let home = tempdir().unwrap();
    let outer = tempdir().unwrap();
    let inner = outer.path().join("inner");
    fs::create_dir_all(&inner).unwrap();

    fs::write(
        outer.path().join("Agentfile"),
        "[agix]\ncli = []\n\n[dependencies]\nouter-dep = { source = \"local:./x\" }\n",
    )
    .unwrap();
    fs::write(outer.path().join("Agentfile.lock"), MINIMAL_LOCK).unwrap();
    fs::write(
        inner.join("Agentfile"),
        "[agix]\ncli = []\n\n[dependencies]\ninner-dep = { source = \"local:./y\" }\n",
    )
    .unwrap();
    fs::write(inner.join("Agentfile.lock"), MINIMAL_LOCK).unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(&inner)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("inner-dep"))
        .stdout(predicate::str::contains("outer-dep").not());
}

// ---------------------------------------------------------------------------
// init creates in cwd, not in parent (no walk-up for init)
// ---------------------------------------------------------------------------

#[test]
fn init_creates_in_cwd_not_parent() {
    let home = tempdir().unwrap();
    let project = tempdir().unwrap();
    // Agentfile exists in project root
    fs::write(project.path().join("Agentfile"), MINIMAL_AGENTFILE).unwrap();
    let sub = project.path().join("sub");
    fs::create_dir(&sub).unwrap();
    // No Agentfile in sub

    helpers::cmd_non_interactive(home.path())
        .current_dir(&sub)
        .args(["init", "--no-interactive"])
        .assert()
        .success();

    assert!(sub.join("Agentfile").exists(), "init must create in cwd");
    // project root's Agentfile should still exist (unchanged)
    assert!(project.path().join("Agentfile").exists());
}

// ---------------------------------------------------------------------------
// init -g creates in ~/.agix/
// ---------------------------------------------------------------------------

#[test]
fn init_global_creates_in_home_agix() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["init", "-g", "--no-interactive"])
        .assert()
        .success();

    assert!(
        home.path().join(".agix").join("Agentfile").exists(),
        "init -g must create ~/.agix/Agentfile"
    );
    assert!(
        !cwd.path().join("Agentfile").exists(),
        "init -g must not create Agentfile in cwd"
    );
}

// ---------------------------------------------------------------------------
// --scope flag is no longer accepted (removed in v0.2.0)
// ---------------------------------------------------------------------------

#[test]
fn scope_flag_is_removed() {
    let home = tempdir().unwrap();
    helpers::cmd_non_interactive(home.path())
        .args(["list", "--scope", "global"])
        .assert()
        .failure();
}
```

- [ ] **Step 2: Run new tests**

```bash
cargo test --test scope_test 2>&1 | tail -10
```

Expected: some pass, some fail (especially walk-up tests that depend on TTY stderr output). Fix any unexpected failures before continuing.

Note: `scope_header` only prints when stderr is a TTY. In CI and `assert_cmd`, stderr is not a TTY — so `.stderr(contains(...))` assertions on the header will fail. Fix: either skip those assertions in the test (use `.success()` only) or make scope_header also print when `AGIX_NO_INTERACTIVE` is set (simpler for testing).

If the TTY assertions fail: remove the `.stderr(contains(...))` lines from `walkup_finds_agentfile_in_parent_directory` and `walkup_falls_back_to_global_when_no_agentfile` and `global_flag_overrides_walkup_inside_project`. The walk-up behavior is verified by which Agentfile content is read (stdout content differs).

- [ ] **Step 3: All scope tests pass**

```bash
cargo test --test scope_test 2>&1 | tail -5
```

Expected: all pass.

---

### Task 8: Migrate existing tests

**Files:**
- Modify: `tests/init_test.rs`
- Modify: `tests/add_test.rs`
- Modify: `tests/install_test.rs`
- Modify: `tests/remove_test.rs`
- Modify: `tests/update_test.rs`
- Modify: `tests/list_test.rs`
- Modify: `tests/outdated_test.rs`
- Modify: `tests/export_roundtrip_test.rs`

Migration rules:
- `"--scope", "local"` → remove both args (walk-up finds it)
- `"--scope", "global"` → replace with `"-g"`
- `["--scope", "local"]` in `.args([...])` → remove
- `["--scope", "global"]` → `["-g"]`

- [ ] **Step 1: Fix init_test.rs**

Lines with `--scope`:
- Line 15: `"--scope"` → remove the `--scope` arg and its value (keep `"local"` behaviour as default)
- Line 105: same
- Line 128: `["init", "--scope", "global", "--no-interactive"]` → `["init", "-g", "--no-interactive"]`
- Line 158: `["init", "--scope", "bogus", "--no-interactive"]` — this tests invalid scope; remove the test entirely or update to test an invalid flag (e.g. `["init", "--scope"]` which fails for a different reason). Simplest: delete this test case since `--scope` no longer exists.

Also remove the temporary test added in Task 3 Step 1 (it's now in scope_test.rs).

- [ ] **Step 2: Fix add_test.rs**

```bash
grep -n "\-\-scope" tests/add_test.rs
```

Lines ~305 and ~645: replace `"--scope", "global"` with `"-g"`. Remove `"--scope", "local"` pairs.

- [ ] **Step 3: Fix remove_test.rs**

```bash
grep -n "\-\-scope" tests/remove_test.rs
```

Lines ~271, ~284: `["install", "--scope", "global"]` → `["install", "-g"]`, `["remove", "my-pkg", "--scope", "global"]` → `["remove", "my-pkg", "-g"]`.

- [ ] **Step 4: Fix list_test.rs**

```bash
grep -n "\-\-scope" tests/list_test.rs
```

Lines ~264, ~280: `["list", "--scope", "global"]` → `["list", "-g"]`.

- [ ] **Step 5: Fix outdated_test.rs, install_test.rs, update_test.rs, export_roundtrip_test.rs**

```bash
grep -rn "\-\-scope" tests/
```

Apply same rule: `"--scope", "local"` → remove; `"--scope", "global"` → `"-g"`.

- [ ] **Step 6: Run full test suite**

```bash
cargo test 2>&1 | tail -10
```

Expected: all tests pass (same count as before Task 4, plus new scope tests).

---

### Task 9: Version bump + clippy + fmt

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Bump version**

In `Cargo.toml`, change:
```toml
version = "0.1.4"
```
to:
```toml
version = "0.2.0"
```

- [ ] **Step 2: Lint and format**

```bash
cargo fmt
cargo clippy 2>&1 | grep "^error"
```

Expected: no errors.

- [ ] **Step 3: Full test suite — final gate**

```bash
cargo test 2>&1 | tail -5
```

Expected: all tests pass.
