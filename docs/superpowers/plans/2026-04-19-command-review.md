# Agix Command Review & Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land five architectural refactors required by the latest design (source drivers with self-naming, CLI-driver-based marketplace installation, interactive CLI selection at init, per-driver local config detection, self-contained export archives), then run a command-by-command behavioral review against the updated design, fixing any remaining deviations.

**Architecture:** Two-phase plan. **Phase A (Tasks 2–7)** lands the architectural refactors upfront, because the review scenarios depend on the new design. Each refactor is TDD — failing test first, minimal impl, passing test, commit. **Phase B (Tasks 8–17)** executes a scenario matrix per command (golden path + sources × scopes × CLI filters + error cases), logs findings, fixes bugs with regression tests.

**Tech Stack:** Rust 2021, `cargo run`, `cargo test`, `assert_cmd` (integration tests), `tempfile` (isolation), `predicates` (output), `dialoguer` (new — interactive menu), `zip` (already a dep — for export), `which` (already a dep — for CLI detection).

---

## Review Workflow

For each review task in Phase B (Tasks 8–17), follow:

1. **Read** the command source (`src/commands/<command>.rs`) and existing tests (`tests/<command>_test.rs`).
2. **Run** every scenario listed (golden path + edge cases + error cases) via `cargo run --`.
3. **Observe** actual output, compare to the "Expected" block under each scenario.
4. **Log** every deviation in `docs/superpowers/plans/2026-04-19-findings.md` following the format in Task 1.
5. **Fix** inline: failing test → minimal impl → passing test → manual re-verify.
6. **Commit** after each task with prefix `review(<command>):` or `refactor(<area>):`.

Scenarios always use isolated temp dirs (`mktemp -d`) and override `HOME` for global-scope tests — never touch the real `~/.agix` or `~/.claude`.

---

## Task 1: Review infrastructure

**Files:**
- Create: `docs/superpowers/plans/2026-04-19-findings.md`
- Create: `tests/helpers/mod.rs`
- Create: `tests/fixtures/mock-skill-pkg/skill.md`
- Create: `tests/fixtures/mock-full-pkg/{skills/a.md,agents/b.md,hooks/c.md,README.md}`
- Create: `tests/fixtures/mock-marketplace/roundtable/skill.md`

- [ ] **Step 1: Create the findings log**

Write `docs/superpowers/plans/2026-04-19-findings.md`:

```markdown
# Agix Command Review — Findings Log

Log every deviation between expected and actual behavior. Format:

## `<command>` — <short scenario name>

**Scenario:** brief description
**Command:** `agix <subcommand> ...`
**Expected:** what should happen
**Actual:** what happens
**Severity:** blocker | major | minor | cosmetic
**Fix commit:** `<sha>` (filled after fix, or `Deferred: <reason>`)
```

- [ ] **Step 2: Create test fixtures**

```bash
mkdir -p tests/fixtures/mock-skill-pkg \
         tests/fixtures/mock-full-pkg/{skills,agents,hooks} \
         tests/fixtures/mock-marketplace/roundtable
```

Write:
- `tests/fixtures/mock-skill-pkg/skill.md` → `# mock skill`
- `tests/fixtures/mock-full-pkg/skills/a.md` → `# skill a`
- `tests/fixtures/mock-full-pkg/agents/b.md` → `# agent b`
- `tests/fixtures/mock-full-pkg/hooks/c.md` → `# hook c`
- `tests/fixtures/mock-full-pkg/README.md` → `# mock full pkg`
- `tests/fixtures/mock-marketplace/roundtable/skill.md` → `# roundtable plugin`

- [ ] **Step 3: Add a shared test helper module**

Create `tests/helpers/mod.rs`:

```rust
#![allow(dead_code)]
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct TestEnv {
    pub cwd: TempDir,
    pub home: TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        Self {
            cwd: tempfile::tempdir().unwrap(),
            home: tempfile::tempdir().unwrap(),
        }
    }

    pub fn write_agentfile(&self, content: &str) {
        std::fs::write(self.cwd.path().join("Agentfile"), content).unwrap();
    }

    pub fn agentfile_content(&self) -> String {
        std::fs::read_to_string(self.cwd.path().join("Agentfile")).unwrap()
    }

    pub fn fixture(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(relative)
    }
}
```

- [ ] **Step 4: Verify build**

Run: `cargo build --tests`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/2026-04-19-findings.md tests/helpers tests/fixtures
git commit -m "chore: review infrastructure (findings log, fixtures, helpers)"
```

---

# Phase A — Architectural Refactors

## Task 2: Source driver trait with self-naming

**Rationale:** Currently package-name extraction for the Agentfile lives in `add.rs` as a fragile rsplit chain. Move naming into each source driver — every driver knows best how to name a package from its own source value.

**Files:**
- Create: `src/sources/driver.rs`
- Modify: `src/sources/mod.rs`
- Modify: `src/sources/local.rs`
- Modify: `src/sources/github.rs`
- Modify: `src/sources/git.rs`
- Test: `src/sources/mod.rs` (tests module) or new `tests/source_driver_test.rs`

- [ ] **Step 1: Write failing tests for suggested_name**

Add to `src/sources/mod.rs` test module:

```rust
#[test]
fn local_suggested_name_is_last_path_component() {
    let spec = SourceSpec::Local { path: "/tmp/foo/my-pkg".into() };
    assert_eq!(spec.suggested_name().unwrap(), "my-pkg");
}

#[test]
fn local_suggested_name_strips_trailing_slash() {
    let spec = SourceSpec::Local { path: "/tmp/foo/my-pkg/".into() };
    assert_eq!(spec.suggested_name().unwrap(), "my-pkg");
}

#[test]
fn github_suggested_name_is_repo() {
    let spec = SourceSpec::GitHub {
        org: "fantoine".into(),
        repo: "claude-later".into(),
        ref_str: None,
    };
    assert_eq!(spec.suggested_name().unwrap(), "claude-later");
}

#[test]
fn git_suggested_name_strips_dot_git() {
    let spec = SourceSpec::Git {
        url: "https://example.com/foo.git".into(),
        ref_str: None,
    };
    assert_eq!(spec.suggested_name().unwrap(), "foo");
}

#[test]
fn marketplace_suggested_name_is_plugin() {
    let spec = SourceSpec::Marketplace {
        marketplace: "fantoine/claude-plugins".into(),
        plugin: "roundtable".into(),
    };
    assert_eq!(spec.suggested_name().unwrap(), "roundtable");
}
```

- [ ] **Step 2: Run the tests — expect compile error (no method)**

Run: `cargo test --lib source`
Expected: `error[E0599]: no method named 'suggested_name'`.

- [ ] **Step 3: Implement `suggested_name` on `SourceSpec`**

Add to `src/sources/mod.rs`:

```rust
impl SourceSpec {
    pub fn suggested_name(&self) -> Result<String> {
        match self {
            SourceSpec::Local { path } => path
                .components()
                .filter_map(|c| match c {
                    std::path::Component::Normal(s) => s.to_str(),
                    _ => None,
                })
                .last()
                .map(str::to_owned)
                .ok_or_else(|| AgixError::InvalidSource(
                    format!("cannot derive name from path {}", path.display())
                )),
            SourceSpec::GitHub { repo, .. } => Ok(repo.clone()),
            SourceSpec::Git { url, .. } => {
                let last = url
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or(url);
                Ok(last.trim_end_matches(".git").to_owned())
            }
            SourceSpec::Marketplace { plugin, .. } => Ok(plugin.clone()),
        }
    }
}
```

- [ ] **Step 4: Run tests, expect pass**

Run: `cargo test --lib source`
Expected: all 5 new tests pass.

- [ ] **Step 5: Replace the hacky extraction in `add.rs`**

In `src/commands/add.rs`, replace the name extraction block with:

```rust
let spec = crate::sources::SourceSpec::parse(&source)?;
let name = spec.suggested_name()?;
```

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: all tests pass (57+).

- [ ] **Step 7: Commit**

```bash
git add src/sources src/commands/add.rs
git commit -m "refactor(sources): add suggested_name on SourceSpec, remove ad-hoc name extraction"
```

---

## Task 3: CLI syntax `add <type> <value>` instead of `add <type>:<value>`

**Rationale:** The current `agix add local:<path>` conflates source type and value in one arg. Users expect them separate. Internal `SourceSpec` string format (`"local:/tmp/foo"`) stays unchanged so Agentfile storage is backward-compatible.

**Files:**
- Modify: `src/main.rs` (clap subcommand for `add`)
- Modify: `src/commands/add.rs`
- Modify: `tests/add_test.rs`
- Modify: `examples/**/Agentfile` (update any doc references, not storage format)
- Modify: `docs/superpowers/specs/2026-04-16-agix-design.md` (optional — if examples need refreshing)

- [ ] **Step 1: Write failing integration tests for the new syntax**

Add to `tests/add_test.rs`:

```rust
#[test]
fn add_local_with_separate_type_and_value() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = [\"claude-code\"]\n").unwrap();
    let pkg_dir = tempdir().unwrap();
    std::fs::write(pkg_dir.path().join("skill.md"), "# s").unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .arg("add")
        .arg("local")
        .arg(pkg_dir.path())
        .assert()
        .success();

    let content = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    assert!(content.contains("local:"), "source should be stored as local:<path>");
}

#[test]
fn add_github_with_separate_type_and_value() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = [\"claude-code\"]\n").unwrap();

    // NOTE: skip network on this unit test; assert the Agentfile gets the entry even if
    // installation fails for lack of the CLI/driver.
    let _ = Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .args(["add", "github", "fantoine/claude-later"])
        .output();
    let content = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    assert!(content.contains("github:fantoine/claude-later"));
}

#[test]
fn add_rejects_unknown_source_type() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = []\n").unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .args(["add", "ftp", "nope"])
        .assert()
        .failure();
}
```

- [ ] **Step 2: Run tests — expect failure**

Run: `cargo test --test add_test add_local_with_separate_type_and_value`
Expected: failure (clap still expects a single `source` positional).

- [ ] **Step 3: Update the clap subcommand**

In `src/main.rs`, change the `Add` variant:

```rust
Add {
    /// Source type: local | github | git | marketplace
    source_type: String,
    /// Source value (path, org/repo, url, marketplace@plugin, ...)
    source_value: String,
    #[arg(long, default_value = "local")]
    scope: Scope,
    #[arg(long, num_args = 1..)]
    cli: Vec<String>,
    #[arg(long)]
    version: Option<String>,
},
```

And the match arm:

```rust
Commands::Add {
    source_type,
    source_value,
    scope,
    cli,
    version,
} => agix::commands::add::run(source_type, source_value, scope.as_str(), cli, version).await,
```

- [ ] **Step 4: Update `add::run` signature**

In `src/commands/add.rs`:

```rust
pub async fn run(
    source_type: String,
    source_value: String,
    scope: &str,
    cli_filter: Vec<String>,
    version: Option<String>,
) -> anyhow::Result<()> {
    const VALID_TYPES: &[&str] = &["local", "github", "git", "marketplace"];
    if !VALID_TYPES.contains(&source_type.as_str()) {
        anyhow::bail!(
            "unknown source type '{}' — expected one of: {}",
            source_type,
            VALID_TYPES.join(", ")
        );
    }
    let source = format!("{}:{}", source_type, source_value);

    let (agentfile_path, lock_path, scope) = super::agentfile_paths(scope)?;
    let mut manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    let spec = crate::sources::SourceSpec::parse(&source)?;
    let name = spec.suggested_name()?;

    let dep = crate::manifest::agentfile::Dependency {
        source: source.clone(),
        version,
        exclude: None,
    };

    if cli_filter.is_empty() {
        manifest.dependencies.insert(name.clone(), dep);
    } else {
        for cli in &cli_filter {
            manifest
                .cli_dependencies
                .entry(cli.clone())
                .or_default()
                .insert(name.clone(), dep.clone());
        }
    }

    manifest.to_file(&agentfile_path)?;
    crate::core::installer::Installer::install_manifest(&manifest, &lock_path, scope).await?;
    crate::output::success(&format!("Added {name}"));
    Ok(())
}
```

- [ ] **Step 5: Update existing tests in `tests/add_test.rs`**

All `.arg(&source)` or `.arg("local:...")` calls must become `.arg("local").arg(path)` etc. Rewrite each existing test accordingly. Keep `add_writes_dependency_to_agentfile`, `add_shared_dependency_without_cli_flag`, `add_multi_cli_dependency` — just fix their arg chains.

- [ ] **Step 6: Update `tests/init_test.rs` global auto-init test**

The test `global_scope_auto_inits_agentfile_if_missing` uses the old `local:<path>` form. Update to `["add", "local", <path>]`.

- [ ] **Step 7: Update `tests/remove_test.rs` setup Agentfiles**

The `remove` tests embed Agentfiles with `source = "local:..."` strings — the on-disk format is unchanged, these tests still work. Verify once.

- [ ] **Step 8: Update `examples/*/Agentfile`**

Inline Agentfile storage still uses `local:<path>` / `github:<org/repo>` strings — no change needed. But example READMEs/comments that show CLI usage must be updated to the new two-arg form. Grep for old form:

Run: `grep -rn 'agix add local:' examples/ docs/ README.md 2>/dev/null`
Replace each `agix add local:<value>` with `agix add local <value>` (and same for github/git/marketplace).

- [ ] **Step 9: Run all tests**

Run: `cargo test`
Expected: all tests pass (updated + new).

- [ ] **Step 10: Manual smoke test**

```bash
cd "$(mktemp -d)"
cargo run --manifest-path <repo>/Cargo.toml -- init
cargo run --manifest-path <repo>/Cargo.toml -- add local <fixture-path>
```

Expected: Agentfile updated, installation runs (local source = no network).

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "refactor(cli): split 'add' into '<type> <value>' positional args"
```

---

## Task 4: Marketplace via CLI drivers (drop GitHub-based impl)

**Rationale:** The current marketplace fetches the marketplace repo from GitHub and mimics install. That's wrong — CLIs like Claude Code manage their own plugin marketplaces via their own tooling. Delegate entirely to the CLI driver.

**Files:**
- Modify: `src/drivers/mod.rs`
- Modify: `src/drivers/claude_code.rs`
- Modify: `src/drivers/codex.rs`
- Modify: `src/core/installer.rs`
- Create: `tests/marketplace_test.rs`

- [ ] **Step 1: Add `install_marketplace_plugin` to the `CliDriver` trait**

In `src/drivers/mod.rs`, add to the trait:

```rust
/// Install a plugin via the CLI's native marketplace mechanism.
/// Drivers that don't support marketplaces should return an `Unsupported` error.
fn install_marketplace_plugin(
    &self,
    marketplace: &str,
    plugin: &str,
    scope: &Scope,
) -> Result<Vec<InstalledFile>>;

/// Uninstall a marketplace-installed plugin via the CLI.
fn uninstall_marketplace_plugin(&self, marketplace: &str, plugin: &str) -> Result<()>;
```

Also add an `Unsupported` variant to `AgixError` if not present:

```rust
// src/error.rs
#[error("{0}")]
Unsupported(String),
```

- [ ] **Step 2: Implement for `CodexDriver` — always Unsupported**

In `src/drivers/codex.rs`:

```rust
fn install_marketplace_plugin(
    &self,
    _marketplace: &str,
    _plugin: &str,
    _scope: &Scope,
) -> Result<Vec<InstalledFile>> {
    Err(AgixError::Unsupported(
        "codex does not support marketplaces".to_string(),
    ))
}

fn uninstall_marketplace_plugin(&self, _marketplace: &str, _plugin: &str) -> Result<()> {
    Err(AgixError::Unsupported(
        "codex does not support marketplaces".to_string(),
    ))
}
```

- [ ] **Step 3: Write failing test for `ClaudeCodeDriver::install_marketplace_plugin`**

Add to `tests/marketplace_test.rs`:

```rust
use assert_cmd::Command;
use tempfile::tempdir;

/// Integration-level test: add a marketplace plugin via the real `claude` CLI.
/// Skipped if `claude` is not available in PATH.
#[test]
fn claude_install_marketplace_plugin_invokes_claude_cli() {
    if which::which("claude").is_err() {
        eprintln!("skipping: `claude` CLI not in PATH");
        return;
    }

    let home = tempdir().unwrap();
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        "[agix]\ncli = [\"claude-code\"]\n",
    ).unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(dir.path())
        .args([
            "add",
            "marketplace",
            "fantoine/claude-plugins@roundtable",
        ])
        .assert()
        .success();
}
```

- [ ] **Step 4: Implement `ClaudeCodeDriver::install_marketplace_plugin`**

In `src/drivers/claude_code.rs`:

```rust
fn install_marketplace_plugin(
    &self,
    marketplace: &str,
    plugin: &str,
    _scope: &Scope,
) -> Result<Vec<InstalledFile>> {
    use std::process::Command;

    if which::which("claude").is_err() {
        return Err(AgixError::Other(
            "`claude` CLI not found in PATH — install Claude Code first".to_string(),
        ));
    }

    // 1. Ensure the marketplace is registered. `claude plugin marketplace add` is
    //    idempotent — re-adding an already-registered marketplace is a no-op.
    let status = Command::new("claude")
        .args(["plugin", "marketplace", "add", marketplace])
        .status()
        .map_err(|e| AgixError::Other(format!("claude plugin marketplace add failed: {e}")))?;
    if !status.success() {
        return Err(AgixError::Other(format!(
            "claude plugin marketplace add {} exited with {}",
            marketplace, status
        )));
    }

    // 2. Install the plugin.
    let plugin_ref = format!("{}@{}", plugin, marketplace);
    let status = Command::new("claude")
        .args(["plugin", "install", &plugin_ref])
        .status()
        .map_err(|e| AgixError::Other(format!("claude plugin install failed: {e}")))?;
    if !status.success() {
        return Err(AgixError::Other(format!(
            "claude plugin install {} exited with {}",
            plugin_ref, status
        )));
    }

    // The CLI manages its own filesystem — we don't track individual files here.
    // Return an empty list; the lock file records only the plugin identity.
    Ok(vec![])
}

fn uninstall_marketplace_plugin(&self, marketplace: &str, plugin: &str) -> Result<()> {
    use std::process::Command;
    let plugin_ref = format!("{}@{}", plugin, marketplace);
    let status = Command::new("claude")
        .args(["plugin", "uninstall", &plugin_ref])
        .status()
        .map_err(|e| AgixError::Other(format!("claude plugin uninstall failed: {e}")))?;
    if !status.success() {
        return Err(AgixError::Other(format!(
            "claude plugin uninstall {} exited with {}",
            plugin_ref, status
        )));
    }
    Ok(())
}
```

- [ ] **Step 5: Rewrite the marketplace block in `installer.rs`**

Delete the fetch-via-GitHub logic and the `marketplace_base_dir` helper. Replace with delegation:

```rust
if let SourceSpec::Marketplace { marketplace, plugin } = &spec {
    let target_clis: Vec<String> = if dep.cli.is_empty() {
        all_drivers()
            .into_iter()
            .filter(|d| d.detect())
            .map(|d| d.name().to_string())
            .collect()
    } else {
        dep.cli.clone()
    };

    let mut all_files: Vec<InstalledFile> = Vec::new();
    for cli_name in &target_clis {
        let driver = match driver_for(cli_name) {
            Some(d) => d,
            None => {
                crate::output::warn(&format!("no driver for '{cli_name}', skipping"));
                continue;
            }
        };
        if !driver.supports_marketplace() {
            crate::output::warn(&format!(
                "marketplace not supported for '{cli_name}', skipping"
            ));
            continue;
        }
        if !driver.detect() {
            crate::output::warn(&format!("'{cli_name}' not detected, skipping"));
            continue;
        }
        crate::output::info(&format!(
            "Installing {plugin} from marketplace {marketplace} via {cli_name}..."
        ));
        match driver.install_marketplace_plugin(marketplace, plugin, &scope) {
            Ok(files) => {
                crate::output::success(&format!(
                    "Plugin '{plugin}' installed for {cli_name}"
                ));
                all_files.extend(files);
            }
            Err(e) => {
                crate::output::warn(&format!(
                    "install failed for {cli_name}: {e}"
                ));
            }
        }
    }

    lock.upsert(LockedPackage {
        name: dep.name.clone(),
        source: dep.source.clone(),
        sha: None,
        content_hash: None,
        version: None,
        cli: dep.cli.clone(),
        scope: scope_str.to_owned(),
        files: all_files,
    });
    lock.to_file(lock_path)?;
    continue;
}
```

- [ ] **Step 6: Remove now-dead `marketplace_base_dir` + related imports**

Delete the helper and its `PathBuf` import if no longer used. `cargo build` will tell you.

- [ ] **Step 7: Run tests**

Run: `cargo test`
Expected: all pass. The new `claude_install_marketplace_plugin_invokes_claude_cli` is skipped in CI (where `claude` isn't present).

- [ ] **Step 8: Manual verification**

```bash
# Clean state
rm -rf "$HOME/.agix/marketplaces"
cd "$(mktemp -d)"
cargo run --manifest-path <repo>/Cargo.toml -- init
cargo run --manifest-path <repo>/Cargo.toml -- add marketplace fantoine/claude-plugins@roundtable
```

Expected:
- stdout shows `claude plugin marketplace add ...` being invoked
- stdout shows `claude plugin install roundtable@fantoine/claude-plugins`
- Final `✓ Plugin 'roundtable' installed for claude-code`
- No `.agix/marketplaces/` directory created anywhere (old behavior gone)

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "refactor(marketplace): delegate install to CLI drivers via claude CLI"
```

---

## Task 5: Interactive CLI selection at `init`

**Rationale:** Right now `init` creates an empty `cli = []` array — the user has to hand-edit the Agentfile. Use a TUI checkbox menu to let them pick among detected CLIs at init time. Force the menu on the first global auto-init (triggered by `add --scope global` when no global Agentfile exists yet).

**Files:**
- Modify: `Cargo.toml` (add `dialoguer`)
- Create: `src/ui/prompt.rs`
- Create: `src/ui/mod.rs`
- Modify: `src/lib.rs` (expose `ui`)
- Modify: `src/commands/init.rs`
- Modify: `src/commands/mod.rs` (auto-init path in `agentfile_paths`)
- Modify: `src/main.rs` (pass `--cli` as init flag to skip prompt)
- Modify: `tests/init_test.rs`

- [ ] **Step 1: Add `dialoguer` dependency**

In `Cargo.toml` under `[dependencies]`:

```toml
dialoguer = { version = "0.11", default-features = false }
```

- [ ] **Step 2: Add an `--cli` flag + `--no-interactive` to `init`**

In `src/main.rs`:

```rust
Init {
    #[arg(long, default_value = "local")]
    scope: Scope,
    /// Pre-select CLIs (skips the interactive menu). Repeatable.
    #[arg(long, num_args = 1..)]
    cli: Vec<String>,
    /// Skip the interactive menu entirely (use with --cli or get cli = []).
    #[arg(long)]
    no_interactive: bool,
},
```

Match arm:

```rust
Commands::Init {
    scope,
    cli,
    no_interactive,
} => agix::commands::init::run(scope.as_str(), cli, no_interactive).await,
```

- [ ] **Step 3: Create `src/ui/prompt.rs` with the CLI selection menu**

```rust
use crate::drivers::all_drivers;
use crate::error::{AgixError, Result};

pub fn pick_clis(preselected: &[String], non_interactive: bool) -> Result<Vec<String>> {
    let drivers = all_drivers();
    let all_names: Vec<String> = drivers.iter().map(|d| d.name().to_string()).collect();

    // Non-interactive path: use preselected; empty is allowed.
    if non_interactive {
        for cli in preselected {
            if !all_names.contains(cli) {
                return Err(AgixError::Other(format!(
                    "unknown CLI '{}' (known: {})",
                    cli,
                    all_names.join(", ")
                )));
            }
        }
        return Ok(preselected.to_vec());
    }

    // Interactive path: show all drivers, pre-check detected ones (or those explicitly
    // passed via --cli).
    let default_selected: Vec<bool> = drivers
        .iter()
        .map(|d| preselected.contains(&d.name().to_string()) || d.detect())
        .collect();

    let labels: Vec<String> = drivers
        .iter()
        .map(|d| {
            let tag = if d.detect() { " (detected)" } else { "" };
            format!("{}{}", d.name(), tag)
        })
        .collect();

    let picked = dialoguer::MultiSelect::new()
        .with_prompt("Select CLIs to manage with agix")
        .items(&labels)
        .defaults(&default_selected)
        .interact()
        .map_err(|e| AgixError::Other(format!("prompt failed: {e}")))?;

    Ok(picked.iter().map(|&i| all_names[i].clone()).collect())
}
```

- [ ] **Step 4: Create `src/ui/mod.rs`**

```rust
pub mod prompt;
```

- [ ] **Step 5: Expose `ui` from `src/lib.rs`**

Add: `pub mod ui;`

- [ ] **Step 6: Rewrite `src/commands/init.rs`**

```rust
pub async fn run(scope: &str, cli: Vec<String>, no_interactive: bool) -> anyhow::Result<()> {
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
```

- [ ] **Step 7: Split `agentfile_paths` to avoid double-init collision**

In `src/commands/mod.rs`:

```rust
/// Return paths without auto-creating the Agentfile (used by `init`).
pub fn agentfile_paths_no_autoinit(scope: &str) -> anyhow::Result<(PathBuf, PathBuf, &'static str)> {
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

/// Return paths, auto-creating the global Agentfile if missing (interactive CLI pick).
/// This is the default for add/install/etc.
pub fn agentfile_paths(scope: &str) -> anyhow::Result<(PathBuf, PathBuf, &'static str)> {
    let (agentfile, lock, scope_s) = agentfile_paths_no_autoinit(scope)?;
    if scope == "global" && !agentfile.exists() {
        crate::output::info("No global Agentfile — running first-time setup");
        let picks = crate::ui::prompt::pick_clis(&[], false)?;
        crate::manifest::agentfile::ProjectManifest::empty(picks).to_file(&agentfile)?;
    }
    Ok((agentfile, lock, scope_s))
}
```

- [ ] **Step 8: Update existing init tests**

`tests/init_test.rs`:
- `init_creates_agentfile` → add `--no-interactive` to args.
- `init_fails_if_already_initialized` → add `--no-interactive`.
- `global_scope_auto_inits_agentfile_if_missing` → this test triggers auto-init via `add`. Since auto-init is now interactive, change it to pass `--cli claude-code` to `add` (which doesn't trigger the menu), OR make this test set an env var that bypasses the menu. Simplest: add an env variable override `AGIX_NO_INTERACTIVE=1` that `pick_clis` respects in non-interactive mode even without the flag. Implement that check in `prompt.rs`:

```rust
if non_interactive || std::env::var("AGIX_NO_INTERACTIVE").is_ok() {
    ...
}
```

Then set `.env("AGIX_NO_INTERACTIVE", "1")` on the test command.

- [ ] **Step 9: Run all tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 10: Manual smoke test**

```bash
cd "$(mktemp -d)"
cargo run --manifest-path <repo>/Cargo.toml -- init
# Menu appears; press space to toggle, enter to confirm.
cat Agentfile
# cli = ["claude-code"] (or whatever you picked)
```

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat(init): interactive CLI selection menu + auto-prompt on first global auto-init"
```

---

## Task 6: Per-driver local config detection for `doctor`

**Rationale:** `doctor` today only shows if a driver is detected globally. It should also tell the user whether a driver has local configuration in the current project.

**Files:**
- Modify: `src/drivers/mod.rs`
- Modify: `src/drivers/claude_code.rs`
- Modify: `src/drivers/codex.rs`
- Modify: `src/commands/doctor.rs`
- Modify: `tests/doctor_test.rs`

- [ ] **Step 1: Add `detect_local_config` to the `CliDriver` trait**

In `src/drivers/mod.rs`:

```rust
/// Return Some(path) if this CLI has project-level config in the given cwd.
fn detect_local_config(&self, cwd: &std::path::Path) -> Option<std::path::PathBuf>;
```

- [ ] **Step 2: Implement for `ClaudeCodeDriver`**

```rust
fn detect_local_config(&self, cwd: &Path) -> Option<PathBuf> {
    let candidate = cwd.join(".claude");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}
```

- [ ] **Step 3: Implement for `CodexDriver`**

```rust
fn detect_local_config(&self, cwd: &Path) -> Option<PathBuf> {
    let candidate = cwd.join(".codex");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}
```

- [ ] **Step 4: Update `doctor.rs` to use the new method**

Read current `src/commands/doctor.rs` first. Then augment the output:

```rust
pub async fn run() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    crate::output::info("CLI drivers:");
    for driver in crate::drivers::all_drivers() {
        let global = if driver.detect() { "detected" } else { "not detected" };
        let local = match driver.detect_local_config(&cwd) {
            Some(p) => format!("local config at {}", p.display()),
            None => "no local config".to_string(),
        };
        println!("  - {}: {} | {}", driver.name(), global, local);
    }
    // ...existing Agentfile validation block
    Ok(())
}
```

- [ ] **Step 5: Add a test**

In `tests/doctor_test.rs`, add a test that runs `doctor` in a dir with a `.claude/` directory and verifies the stdout includes `local config at`.

- [ ] **Step 6: Run tests, commit**

```bash
cargo test
git add -A
git commit -m "feat(doctor): per-driver local config detection"
```

---

## Task 7: Self-contained `export` with local-sources relocation

**Rationale:** Today `export` (if implemented at all) produces an Agentfile/lock but nothing portable. Users expect a zip they can unzip anywhere and run `agix install`. That requires copying local-source directories into the archive and rewriting their paths.

**Files:**
- Modify: `Cargo.toml` (confirm `zip` dep is present with write feature)
- Modify: `src/commands/export.rs`
- Create: `tests/export_roundtrip_test.rs`

- [ ] **Step 1: Ensure `zip` has write feature**

In `Cargo.toml`:

```toml
zip = { version = "0.6", features = ["deflate"] }
```

- [ ] **Step 2: Write a failing roundtrip test**

Create `tests/export_roundtrip_test.rs`:

```rust
use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn export_zip_is_self_contained_and_installable() {
    // Setup: project with one local-source dep
    let src_dir = tempdir().unwrap();
    std::fs::create_dir(src_dir.path().join("skills")).unwrap();
    std::fs::write(
        src_dir.path().join("skills/s.md"),
        "# s",
    ).unwrap();

    let proj = tempdir().unwrap();
    std::fs::write(
        proj.path().join("Agentfile"),
        format!(
            "[agix]\ncli = [\"claude-code\"]\n\n[dependencies]\nmy-pkg = {{ source = \"local:{}\" }}\n",
            src_dir.path().display()
        ),
    ).unwrap();

    // 1. Install
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(proj.path())
        .args(["install"])
        .env("AGIX_NO_INTERACTIVE", "1")
        .assert()
        .success();

    // 2. Export
    let out = proj.path().join("state.zip");
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(proj.path())
        .args(["export", "--output"])
        .arg(&out)
        .env("AGIX_NO_INTERACTIVE", "1")
        .assert()
        .success();
    assert!(out.exists(), "zip should be created");

    // 3. Unzip to a fresh dir
    let target = tempdir().unwrap();
    let file = std::fs::File::open(&out).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    archive.extract(target.path()).unwrap();

    // 4. Agentfile inside the unzipped dir should reference local-sources/my-pkg (relative)
    let exported_agentfile =
        std::fs::read_to_string(target.path().join("Agentfile")).unwrap();
    assert!(
        exported_agentfile.contains("local:./local-sources/my-pkg")
            || exported_agentfile.contains("local:local-sources/my-pkg"),
        "Agentfile should reference local-sources/, got: {exported_agentfile}"
    );
    assert!(target.path().join("local-sources/my-pkg/skills/s.md").exists());

    // 5. Install in the unzipped dir should succeed
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(target.path())
        .args(["install"])
        .env("AGIX_NO_INTERACTIVE", "1")
        .assert()
        .success();
}
```

- [ ] **Step 3: Run the test — expect failure**

Run: `cargo test --test export_roundtrip_test`
Expected: failure (export doesn't zip yet).

- [ ] **Step 4: Implement `export.rs`**

Write `src/commands/export.rs` (replacing any existing skeleton):

```rust
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::FileOptions;
use zip::ZipWriter;

use crate::manifest::agentfile::ProjectManifest;
use crate::sources::SourceSpec;

pub async fn run(scope: &str, all: bool, output: Option<String>) -> anyhow::Result<()> {
    if all {
        anyhow::bail!("--all is not yet implemented"); // deferred
    }
    let (agentfile_path, lock_path, _scope) = super::agentfile_paths_no_autoinit(scope)?;
    if !agentfile_path.exists() {
        anyhow::bail!("No Agentfile found");
    }
    let manifest = ProjectManifest::from_file(&agentfile_path)?;

    let out_path = output.clone().unwrap_or_else(|| "agix-export.zip".into());
    let out_file = File::create(&out_path)?;
    let mut zip = ZipWriter::new(out_file);
    let opts: FileOptions = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 1. Walk manifest, rewrite local sources, collect file copy operations
    let mut rewritten = manifest.clone();
    let mut shared_rewrite = |deps: &mut std::collections::BTreeMap<String, crate::manifest::agentfile::Dependency>,
                              zip: &mut ZipWriter<File>|
     -> anyhow::Result<()> {
        for (name, dep) in deps.iter_mut() {
            let spec = SourceSpec::parse(&dep.source)?;
            if let SourceSpec::Local { path } = spec {
                let relative = format!("local-sources/{name}");
                copy_dir_into_zip(&path, &relative, zip, &opts)?;
                dep.source = format!("local:./{relative}");
            }
        }
        Ok(())
    };
    shared_rewrite(&mut rewritten.dependencies, &mut zip)?;
    for (_, cli_deps) in rewritten.cli_dependencies.iter_mut() {
        shared_rewrite(cli_deps, &mut zip)?;
    }

    // 2. Write rewritten Agentfile to zip
    let toml_s = toml::to_string_pretty(&rewritten)?;
    zip.start_file("Agentfile", opts)?;
    zip.write_all(toml_s.as_bytes())?;

    // 3. Copy lock (if exists) — rewrite local lock paths too
    if lock_path.exists() {
        let lock_content = std::fs::read_to_string(&lock_path)?;
        // Simple path rewrite in lock — the lock stores absolute source strings too.
        let rewritten_lock = rewrite_local_sources_in_lock(&lock_content, &rewritten)?;
        zip.start_file("Agentfile.lock", opts)?;
        zip.write_all(rewritten_lock.as_bytes())?;
    }

    zip.finish()?;
    crate::output::success(&format!("Exported to {out_path}"));
    Ok(())
}

fn copy_dir_into_zip(
    src: &Path,
    zip_prefix: &str,
    zip: &mut ZipWriter<File>,
    opts: &FileOptions,
) -> anyhow::Result<()> {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let zip_path = format!("{zip_prefix}/{}", rel.display());
        if entry.file_type().is_dir() {
            zip.add_directory(&zip_path, *opts)?;
        } else {
            zip.start_file(&zip_path, *opts)?;
            let mut f = File::open(entry.path())?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            zip.write_all(&buf)?;
        }
    }
    Ok(())
}

fn rewrite_local_sources_in_lock(
    lock_content: &str,
    rewritten_manifest: &ProjectManifest,
) -> anyhow::Result<String> {
    // Parse lock as toml::Value, rewrite any `source = "local:<abs>"` entries
    // to the relocated path.
    let mut doc: toml::Value = lock_content.parse()?;
    if let Some(pkgs) = doc.get_mut("package").and_then(|v| v.as_array_mut()) {
        for pkg in pkgs.iter_mut() {
            if let Some(name) = pkg.get("name").and_then(|v| v.as_str()).map(str::to_owned) {
                // Look up the new source string from the manifest.
                let new_source = rewritten_manifest
                    .dependencies
                    .get(&name)
                    .or_else(|| {
                        rewritten_manifest
                            .cli_dependencies
                            .values()
                            .find_map(|m| m.get(&name))
                    })
                    .map(|d| d.source.clone());
                if let Some(s) = new_source {
                    if s.starts_with("local:") {
                        if let Some(src) = pkg.get_mut("source") {
                            *src = toml::Value::String(s);
                        }
                    }
                }
            }
        }
    }
    Ok(toml::to_string_pretty(&doc)?)
}
```

Add `walkdir = "2"` to `Cargo.toml` if not already present.

- [ ] **Step 5: Run the test, iterate until it passes**

Run: `cargo test --test export_roundtrip_test`
Expected: passes.

- [ ] **Step 6: Run all tests, commit**

```bash
cargo test
git add -A
git commit -m "feat(export): self-contained zip with local-sources relocation"
```

---

# Phase B — Per-Command Review

Each task below follows the review workflow at the top of this document: read code → run every scenario → log deviation → fix with regression test → commit.

## Task 8: Review `init`

**Files:** `src/commands/init.rs`, `tests/init_test.rs`, `src/ui/prompt.rs`

- [ ] **Step 1: Read source**
- [ ] **Step 2: Scenario — local init with `--no-interactive`**

```bash
cd "$(mktemp -d)"
cargo run ... -- init --no-interactive
```

Expected: exit 0, Agentfile with `cli = []`, message `Created ...`.

- [ ] **Step 3: Scenario — local init with `--cli claude-code --cli codex --no-interactive`**

Expected: Agentfile with `cli = ["claude-code", "codex"]`.

- [ ] **Step 4: Scenario — local init interactive (manual)**

```bash
cargo run ... -- init
```

Expected: menu renders, space toggles, enter confirms, Agentfile has checked CLIs.

- [ ] **Step 5: Scenario — local init when Agentfile already exists**

Expected: exit non-zero, stderr `Already initialized`.

- [ ] **Step 6: Scenario — global init fresh**

```bash
tmp_home=$(mktemp -d)
HOME="$tmp_home" cargo run ... -- init --scope global --no-interactive --cli claude-code
```

Expected: `$tmp_home/.agix/Agentfile` with `cli = ["claude-code"]`.

- [ ] **Step 7: Scenario — global init when already initialized**

Expected: exit non-zero, `Already initialized`.

- [ ] **Step 8: Scenario — `--cli unknown`**

Expected: exit non-zero, stderr mentions `unknown CLI 'unknown'`.

- [ ] **Step 9: Scenario — invalid scope value (clap-level)**

Expected: clap error.

- [ ] **Step 10: Log findings, fix bugs, commit**

Commit: `review(init): ...`

---

## Task 9: Review `check`

**Files:** `src/commands/check.rs`, `tests/check_test.rs`

- [ ] **Step 1: Read source**
- [ ] **Step 2: Scenario — no Agentfile**

Expected: exit non-zero, stderr `No Agentfile`.

- [ ] **Step 3: Scenario — valid project manifest**

```toml
[agix]
cli = ["claude-code"]
```

Expected: exit 0, stdout `Agentfile valid — project for claude-code`.

- [ ] **Step 4: Scenario — valid package manifest (name + version)**

```toml
[agix]
name = "my-pkg"
version = "1.0.0"
cli = ["claude-code"]
```

Expected: exit 0, stdout mentions `my-pkg v1.0.0`.

- [ ] **Step 5: Scenario — package manifest missing version**

Expected: exit non-zero.

- [ ] **Step 6: Scenario — missing `[agix].cli`**

Expected: exit non-zero, stderr mentions `cli`.

- [ ] **Step 7: Scenario — invalid TOML**

Expected: exit non-zero with line/col info.

- [ ] **Step 8: Scenario — dep with unparseable source**

Expected: exit non-zero, error mentions the dep name.

- [ ] **Step 9: Scenario — CLI not in known drivers (unknown CLI name in `cli` array)**

Decide: warn vs error. Enforce. Log decision.

- [ ] **Step 10: Log findings, fix bugs, commit**

Commit: `review(check): ...`

---

## Task 10: Review `add`

**Files:** `src/commands/add.rs`, `tests/add_test.rs`

- [ ] **Step 1: Read source**

### Matrix

- [ ] **Step 2: `add local <path>` shared dep, local scope**
- [ ] **Step 3: `add local <path> --cli claude-code`**
- [ ] **Step 4: `add local <path> --cli claude-code --cli codex`**
- [ ] **Step 5: `add local <path> --cli not-in-agix-cli` — decision: error or auto-append? Enforce.**
- [ ] **Step 6: `add github fantoine/claude-later`**
- [ ] **Step 7: `add github fantoine/claude-later --version main`**
- [ ] **Step 8: `add git https://github.com/fantoine/claude-later.git`**
- [ ] **Step 9: `add marketplace fantoine/claude-plugins@roundtable` — verify Claude CLI invocation**
- [ ] **Step 10: `add marketplace fantoine/claude-plugins@roundtable --scope global` — verify auto-init menu triggers**
- [ ] **Step 11: `add local <path>` without Agentfile** — Expected: exit non-zero, mentions `agix init`.
- [ ] **Step 12: `add ftp something` — unknown source type** — Expected: exit non-zero.
- [ ] **Step 13: `add local <path>` twice** — decision: overwrite vs warn. Enforce.
- [ ] **Step 14: Package name inference** — verify `suggested_name()` outputs match Task 2 expectations for each source type.
- [ ] **Step 15: Log findings, fix bugs, commit**

Commit: `review(add): ...`

---

## Task 11: Review `remove`

**Files:** `src/commands/remove.rs`, `tests/remove_test.rs`

- [ ] **Step 1: Read source**
- [ ] **Step 2: Remove shared dep**
- [ ] **Step 3: Remove with `--cli` filter (dep still in other CLI's section)**
- [ ] **Step 4: Remove non-existent package**
- [ ] **Step 5: Remove when no lock exists** — decide and enforce.
- [ ] **Step 6: Remove shared dep installed across multiple CLIs** — files removed from all CLI dirs, lock entry dropped.
- [ ] **Step 7: Remove in global scope**
- [ ] **Step 8: Remove a marketplace-installed plugin** — must call `uninstall_marketplace_plugin` on the driver (Task 4 trait method).
- [ ] **Step 9: Log findings, fix bugs, commit**

Commit: `review(remove): ...`

---

## Task 12: Review `install`

**Files:** `src/commands/install.rs`, `src/core/installer.rs`, `tests/install_test.rs`

- [ ] **Step 1: Read source**
- [ ] **Step 2: Fresh install of local dep (full-pkg fixture) — all subdirs copied**
- [ ] **Step 3: Install twice — idempotent / no-op on second run**
- [ ] **Step 4: Install after local source file changes — content_hash detects diff, re-copies**
- [ ] **Step 5: Install github dep with version = "main"**
- [ ] **Step 6: Install mixed sources (local + github + marketplace)**
- [ ] **Step 7: Install with CLI-specific deps (no cross-contamination)**
- [ ] **Step 8: Install with `exclude = ["codex"]`**
- [ ] **Step 9: Install without Agentfile — exit non-zero**
- [ ] **Step 10: Install with invalid source — exit non-zero, names the dep**
- [ ] **Step 11: Install when driver not detected — warn, skip, exit 0**
- [ ] **Step 12: Install with post-install hook — script executes**
- [ ] **Step 13: Log findings, fix bugs, commit**

Commit: `review(install): ...`

---

## Task 13: Review `update`

**Files:** `src/commands/update.rs`, `tests/update_test.rs`

- [ ] **Step 1: Read source**
- [ ] **Step 2: Update all (no name) — all remote deps re-resolved**
- [ ] **Step 3: Update specific package**
- [ ] **Step 4: Update non-existent package — exit non-zero**
- [ ] **Step 5: Update local dep — re-copies, content_hash updated**
- [ ] **Step 6: Update marketplace plugin — driver's install_marketplace_plugin re-invoked**
- [ ] **Step 7: Update without Agentfile — exit non-zero**
- [ ] **Step 8: Update without lock — decide (treat as install or error)**
- [ ] **Step 9: Log findings, fix bugs, commit**

Commit: `review(update): ...`

---

## Task 14: Review `list`

**Files:** `src/commands/list.rs`, `tests/list_test.rs`

- [ ] **Step 1: Read source**
- [ ] **Step 2: List empty manifest — stdout empty-state message (no crash)**
- [ ] **Step 3: List with shared dep — shows source, version, target CLIs**
- [ ] **Step 4: List with CLI-specific deps — grouped clearly**
- [ ] **Step 5: List with no lock — either manifest-only or errors. Decide.**
- [ ] **Step 6: List without Agentfile — exit non-zero**
- [ ] **Step 7: List global scope**
- [ ] **Step 8: Log findings, fix bugs, commit**

Commit: `review(list): ...`

---

## Task 15: Review `outdated`

**Files:** `src/commands/outdated.rs`, `tests/outdated_test.rs`

- [ ] **Step 1: Read source** — Audit noted remote checking is not implemented; verify and implement.
- [ ] **Step 2: Outdated with no updates — `all up to date`**
- [ ] **Step 3: Outdated with a remote update (floating ref) — lists the package with current+available SHA**
- [ ] **Step 4: Outdated with only local deps — skipped / labeled `local (not checkable)`**
- [ ] **Step 5: Outdated without Agentfile — exit non-zero**
- [ ] **Step 6: Outdated without lock — decide: all-outdated or error**
- [ ] **Step 7: Implement remote-resolve if missing**

In `src/commands/outdated.rs`, for each github/git dep with a floating `version`, call `GitHubSource::resolve_ref()` and compare to the lock's SHA. Add a test using `mockito` to serve a different SHA than in the lock; assert the command reports the package.

- [ ] **Step 8: Log findings, fix bugs, commit**

Commit: `review(outdated): ...`

---

## Task 16: Review `doctor`

**Files:** `src/commands/doctor.rs`, `tests/doctor_test.rs`

- [ ] **Step 1: Read source (post-refactor from Task 6)**
- [ ] **Step 2: Doctor on clean system — lists each driver, global detection + local config state**
- [ ] **Step 3: Doctor inside a dir with `.claude/` — shows `local config at ...` for claude-code**
- [ ] **Step 4: Doctor inside a dir with `.codex/` — same for codex**
- [ ] **Step 5: Doctor with valid Agentfile — reports Agentfile status**
- [ ] **Step 6: Doctor with broken Agentfile — reports parse error**
- [ ] **Step 7: Doctor with Agentfile but no lock — warns `run agix install`**
- [ ] **Step 8: Doctor with marketplaces previously installed via `claude` — optionally lists them (if Claude CLI reports them via `claude plugin list`). Decide scope.**
- [ ] **Step 9: Log findings, fix bugs, commit**

Commit: `review(doctor): ...`

---

## Task 17: Review `export`

**Files:** `src/commands/export.rs`, `tests/export_test.rs`, `tests/export_roundtrip_test.rs`

- [ ] **Step 1: Read source (post-refactor from Task 7)**
- [ ] **Step 2: Export to default filename (no `--output`)**

Expected: creates `agix-export.zip` in cwd.

- [ ] **Step 3: Export with `--output <path>` — writes to given path**
- [ ] **Step 4: Export with `--all` — includes both scopes (or errors if deferred)**
- [ ] **Step 5: Export without Agentfile — exit non-zero**
- [ ] **Step 6: Export roundtrip — unzip + install works (covered by Task 7 test, re-run)**
- [ ] **Step 7: Export with github dep — zip contains Agentfile referencing the same github source (no relocation needed)**
- [ ] **Step 8: Export with marketplace dep — zip stores the marketplace reference; install from zip invokes `claude plugin install` again**
- [ ] **Step 9: Log findings, fix bugs, commit**

Commit: `review(export): ...`

---

## Task 18: Cross-cutting review

**Files:** `src/output.rs`, `src/error.rs`, `src/manifest/agentfile.rs`, all commands

- [ ] **Step 1: Exit codes** — success=0, user error=1, internal error=non-zero. Fix deviations.
- [ ] **Step 2: Error messages** — actionable phrasing, no raw `anyhow` debug spew, written to stderr. Fix deviations.
- [ ] **Step 3: Output formatting consistency** — search for stray `println!`/`eprintln!` in `src/commands`, `src/core`, `src/drivers`:

```bash
grep -rn 'println!\|eprintln!' src/commands src/core src/drivers
```

Replace user-visible messages with `crate::output::{success, warn, info}`. Exception: data output (list, export stdout) is fine.

- [ ] **Step 4: `--scope` consistency** — every command that takes `--scope` defaults to `local`, global auto-init (except `init` itself) triggers the interactive menu.
- [ ] **Step 5: `--cli` consistency** — multi-value `--cli a --cli b` works on `add`, `remove`, `init`.
- [ ] **Step 6: Commit cross-cutting fixes**

```bash
git commit -m "review(core): cross-cutting consistency fixes"
```

---

## Task 19: Final sweep

- [ ] **Step 1: `cargo test`** — all pass (57 baseline + new)
- [ ] **Step 2: `cargo fmt -- --check`** — no diff
- [ ] **Step 3: `cargo clippy -- -D warnings`** — no warnings
- [ ] **Step 4: Re-read findings log** — every entry has a Fix commit or Deferred annotation
- [ ] **Step 5: Write review summary at the top of the findings log**

```markdown
## Review Summary

- **Total scenarios executed:** N
- **Issues found:** N (Blocker: N, Major: N, Minor: N, Cosmetic: N)
- **Issues fixed:** N
- **Issues deferred:** N
- **Regression tests added:** N
- **Tests before review:** 57
- **Tests after review:** N
- **Architectural refactors landed:** 6 (source-self-naming, add-syntax, marketplace-via-drivers, init-menu, doctor-local-config, export-zip)
```

- [ ] **Step 6: Commit the summary**

```bash
git add docs/superpowers/plans/2026-04-19-findings.md
git commit -m "review: summary and close-out"
```

---

## Notes for executors

- **Phase A is mandatory first.** Phase B scenarios assume the new CLI syntax and driver-based marketplace. Running them before the refactors will produce false positives.
- **Use tempdirs and `HOME` overrides.** Never touch real `~/.agix`/`~/.claude` unless a scenario explicitly requires it.
- **One bug → one test → one fix → one commit.** Don't batch.
- **Design decisions surface as scenarios.** When a scenario has "Decide and enforce", log the decision in findings and surface it to the user before committing the behavior. Don't silently pick.
- **Skip network-dependent tests gracefully** when the target CLI (`claude`) or network isn't available (use `if which::which(...).is_err() { return; }` pattern).
