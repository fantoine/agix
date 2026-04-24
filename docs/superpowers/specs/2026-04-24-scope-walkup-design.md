# Scope Walk-Up Resolution — Design Spec

**Date**: 2026-04-24
**Status**: Approved

## Goal

Replace the explicit `--scope local|global` flag with context-aware walk-up resolution: agix finds the nearest `Agentfile` by walking up from the current directory to `$HOME`, falling back to the global scope (`~/.agix/`) when none is found. A `-g/--global` flag provides an explicit override when needed.

## Motivation

- Skills installed locally are committed files — git is already the lock. The lockfile adds value mainly for remote sources (github:/git:) and marketplace plugins, not for committed files.
- The global scope (`~/.agix/Agentfile`) is where the lockfile truly earns its keep: machine portability, backup, restore.
- Both main competitors (`npx skills`, `gh skill`) are project-default. Walk-up differentiates agix by being context-aware (like git/cargo) rather than opinionated.
- Removes cognitive overhead: no flag needed in 95% of uses.

## Competitive Context

| Tool | Scope model |
|---|---|
| `npx skills` | project-default, `-g` for global |
| `gh skill` | project-default, `--scope user` for global |
| agix (before) | explicit `--scope local` (default) or `--scope global` |
| agix (after) | walk-up (context-aware), `-g/--global` override |

## Algorithm

### Walk-up

```
start = cwd
loop:
  if Agentfile exists in current dir → use it (project scope)
  if current dir == $HOME → stop
  current = parent(current)
→ fallback: ~/.agix/Agentfile (global scope)
```

Boundary: never walks above `$HOME`. On systems without a home directory (unusual CI), falls back to global immediately.

### Resolved scope

```rust
enum ResolvedScope {
    Global,            // ~/.agix/Agentfile
    Project(PathBuf),  // directory where Agentfile was found
}
```

### Scope resolution table

| Situation | Result |
|---|---|
| `-g` / `--global` flag | `~/.agix/Agentfile` (hard override) |
| No flag, Agentfile found in walk-up | that directory's Agentfile |
| No flag, nothing found before `$HOME` | `~/.agix/Agentfile` (silent fallback) |

## CLI Changes

### Flag replacement

`--scope local|global` removed from all subcommands. Replaced by:

```
-g, --global    Use ~/.agix/Agentfile regardless of context
```

### Per-command impact

| Command | Change |
|---|---|
| `init` | `--scope` → `-g/--global`. Still creates in cwd; `-g` creates in `~/.agix/`. No walk-up (init is always intentional). |
| `install` | Walk-up replaces `--scope local` default |
| `add` | Walk-up + prints resolved scope header before acting |
| `remove` | Walk-up + prints resolved scope header |
| `update` | Walk-up + prints resolved scope header |
| `list` | Walk-up + prints resolved scope header |
| `outdated` | Walk-up + prints resolved scope header |
| `doctor` | Walk-up + prints resolved scope header |
| `check` | Unchanged — reads package Agentfile, not project Agentfile |
| `export` | Walk-up + prints resolved scope header |

### Scope header (stderr, dim)

All commands that resolve scope print one line to stderr before output:

```
Using ~/projects/myapp/Agentfile   (project)
```
or
```
Using ~/.agix/Agentfile   (global)
```

Suppressed when stderr is not a TTY (CI/piped). Suppressed with `--json` if that flag is present.

## Implementation

### New function: `find_project_root`

Located in `src/commands/mod.rs`:

```rust
fn find_project_root(start: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let mut current = start.to_path_buf();
    loop {
        if current.join("Agentfile").exists() {
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
```

### Updated function: `agentfile_paths`

Signature change:

```rust
// Before
pub fn agentfile_paths(scope: Scope, create_global: bool) -> Result<(PathBuf, PathBuf, Scope)>

// After
pub fn agentfile_paths(global: bool, cwd: &Path) -> Result<(PathBuf, PathBuf, ResolvedScope)>
```

Logic:

```rust
pub fn agentfile_paths(global: bool, cwd: &Path) -> Result<(PathBuf, PathBuf, ResolvedScope)> {
    if global {
        let (af, lock) = global_paths()?;
        return Ok((af, lock, ResolvedScope::Global));
    }
    if let Some(root) = find_project_root(cwd) {
        let af = root.join("Agentfile");
        let lock = root.join("Agentfile.lock");
        return Ok((af, lock, ResolvedScope::Project(root)));
    }
    // Silent fallback to global
    let (af, lock) = global_paths()?;
    Ok((af, lock, ResolvedScope::Global))
}
```

### Output helper

New function in `src/output.rs`:

```rust
pub fn scope_header(scope: &ResolvedScope, agentfile: &Path) {
    if !stderr_is_tty() { return; }
    let label = match scope {
        ResolvedScope::Global => "global",
        ResolvedScope::Project(_) => "project",
    };
    eprintln!("  Using {}   ({})", agentfile.display(), label);
}
```

### `src/main.rs` — subcommand structs

Every subcommand that had `scope: Scope` gets:

```rust
#[arg(short = 'g', long)]
global: bool,
```

`init` is the exception: it keeps `--global` as a boolean flag (no walk-up; always creates in cwd or `~/.agix/` depending on the flag).

## Migration & Compatibility

### Breaking changes (UX only)

- `--scope local` removed — walk-up is the new default
- `--scope global` removed — replaced by `-g/--global`

Users scripting `--scope global` must update to `-g` or `--global`.

### Non-breaking

- `~/.agix/Agentfile` path unchanged
- Agentfile/lock file format unchanged
- Users who never used `--scope`: behavior is nearly identical (walk-up finds the local Agentfile when inside a project)

### Version bump

Breaking UX change → `0.2.0`.

## Testing

All integration tests use `helpers::cmd_non_interactive(home)` with `tempdir` for cwd and home isolation.

### New tests (`tests/scope_test.rs`)

```rust
// Walk-up: Agentfile in parent, cwd is a subdirectory
fn walkup_finds_agentfile_in_parent() {
    let project = tempdir();
    let sub = project.path().join("src");
    fs::create_dir(&sub);
    fs::write(project.path().join("Agentfile"), minimal_agentfile());
    fs::write(project.path().join("Agentfile.lock"), minimal_lock());

    cmd(home).current_dir(&sub).args(["list"]).assert().success()
        .stderr(contains(project.path().to_str().unwrap())); // scope header
}

// Walk-up: no Agentfile anywhere → fallback global (operates on ~/.agix/)
fn walkup_falls_back_to_global_when_no_agentfile() {
    let cwd = tempdir(); // empty, no Agentfile
    let home = tempdir();
    fs::write(home.path().join("Agentfile"), minimal_agentfile()); // global exists
    fs::write(home.path().join("Agentfile.lock"), minimal_lock());

    cmd(home.path()).current_dir(cwd.path()).args(["list"])
        .assert().success()
        .stderr(contains(".agix/Agentfile"));
}

// Walk-up stops at $HOME — no Agentfile found above home boundary
fn walkup_stops_at_home_boundary() {
    // home has no Agentfile, cwd is inside home, no project Agentfile exists
    // → fallback global creates ~/.agix/Agentfile automatically
    let home = tempdir();
    let cwd = tempdir_inside(home.path()); // cwd is a subdir of home
    cmd(home.path()).current_dir(cwd.path()).args(["list"])
        .assert().success()
        .stderr(contains(".agix/Agentfile"));
}

// -g overrides walk-up even inside a project
fn global_flag_overrides_walkup() {
    let project = tempdir();
    let home = tempdir();
    fs::write(project.path().join("Agentfile"), minimal_agentfile());
    init_global(home.path()); // creates ~/.agix/Agentfile

    cmd(home.path()).current_dir(project.path()).args(["list", "-g"])
        .assert().success()
        .stderr(contains(".agix/Agentfile")); // global used, not project
}

// Nested projects: inner Agentfile wins
fn nested_project_inner_agentfile_wins() {
    let outer = tempdir();
    let inner = outer.path().join("inner");
    fs::create_dir(&inner);
    fs::write(outer.path().join("Agentfile"), agentfile_with_dep("outer-dep"));
    fs::write(inner.join("Agentfile"), agentfile_with_dep("inner-dep"));

    cmd(home).current_dir(&inner).args(["list"])
        .assert().success()
        .stdout(contains("inner-dep"))
        .stdout(not(contains("outer-dep")));
}

// init creates in cwd, no walk-up
fn init_creates_in_cwd_not_parent() {
    let project = tempdir();
    let sub = project.path().join("sub");
    fs::create_dir(&sub);
    // No Agentfile in project root

    cmd(home).current_dir(&sub).args(["init", "--no-interactive"])
        .assert().success();

    assert!(sub.join("Agentfile").exists());           // created here
    assert!(!project.path().join("Agentfile").exists()); // not in parent
}

// init -g creates in ~/.agix/
fn init_global_creates_in_home_agix() {
    let home = tempdir();
    let cwd = tempdir();
    cmd(home.path()).current_dir(cwd.path()).args(["init", "-g", "--no-interactive"])
        .assert().success();
    assert!(home.path().join(".agix").join("Agentfile").exists());
}
```

### Existing tests to update

All integration tests passing `--scope local` or `--scope global` must be migrated:
- Remove `--scope local` (now default via walk-up)
- Replace `--scope global` with `-g`
- Ensure test setup places cwd at the directory containing the test Agentfile (so walk-up resolves correctly)

Affected files: `add_test.rs`, `install_test.rs`, `remove_test.rs`, `update_test.rs`, `list_test.rs`, `outdated_test.rs`, `export_roundtrip_test.rs`.
