use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

mod helpers;

// Thin wrapper around the shared `cmd_non_interactive` helper that also binds
// `current_dir` (every `list` test needs to run in its own tempdir cwd).
fn list_cmd(cwd: &std::path::Path, home: &std::path::Path) -> Command {
    let mut cmd = helpers::cmd_non_interactive(home);
    cmd.current_dir(cwd);
    cmd
}

// ---------------------------------------------------------------------------
// Step 6: no Agentfile — exit non-zero with an actionable message.
// ---------------------------------------------------------------------------

#[test]
fn step6_list_without_local_agentfile_falls_back_to_global() {
    // Walk-up finds no Agentfile → fallback to ~/.agix/ (auto-created, no deps).
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    list_cmd(cwd.path(), home.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No dependencies declared"));
}

// ---------------------------------------------------------------------------
// Step 2: empty manifest — friendly empty-state line, not a crash / empty stdout.
// ---------------------------------------------------------------------------

#[test]
fn step2_list_with_empty_manifest_prints_friendly_empty_state() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]
"#,
    )
    .unwrap();

    list_cmd(cwd.path(), home.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No dependencies declared"));
}

// ---------------------------------------------------------------------------
// Step 3: shared dep — output shows source, version, target CLIs.
// ---------------------------------------------------------------------------

#[test]
fn step3_list_with_shared_dep_shows_source_version_clis() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    // Agentfile declares a shared local dep.
    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]

[dependencies]
my-shared = { source = "local:/tmp/fake" }
"#,
    )
    .unwrap();

    // Forge a matching lock entry so the installed path is exercised.
    fs::write(
        cwd.path().join("Agentfile.lock"),
        r#"
[[package]]
name = "my-shared"
source = "local:/tmp/fake"
content_hash = "deadbeefcafef00d"
cli = ["claude"]
scope = "local"
files = []
"#,
    )
    .unwrap();

    list_cmd(cwd.path(), home.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Shared:"))
        .stdout(predicate::str::contains("my-shared"))
        .stdout(predicate::str::contains("local:/tmp/fake"))
        .stdout(predicate::str::contains("(claude)"));
}

#[test]
fn step3_list_shows_short_sha_for_github_dep() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]

[dependencies]
gh-dep = { source = "github:org/repo" }
"#,
    )
    .unwrap();
    fs::write(
        cwd.path().join("Agentfile.lock"),
        r#"
[[package]]
name = "gh-dep"
source = "github:org/repo"
sha = "abcdef1234567890"
cli = ["claude"]
scope = "local"
files = []
"#,
    )
    .unwrap();

    list_cmd(cwd.path(), home.path())
        .arg("list")
        .assert()
        .success()
        // 7-char truncation applied.
        .stdout(predicate::str::contains("abcdef1"))
        .stdout(predicate::str::contains("github:org/repo"));
}

// ---------------------------------------------------------------------------
// Step 4: CLI-specific deps — grouped clearly under a `[<cli>]:` heading,
// separate from the shared block.
// ---------------------------------------------------------------------------

#[test]
fn step4_list_groups_cli_specific_deps_under_cli_heading() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude", "codex"]

[dependencies]
shared-a = { source = "local:/tmp/a" }

[claude.dependencies]
claude-only = { source = "local:/tmp/c" }

[codex.dependencies]
codex-only = { source = "local:/tmp/x" }
"#,
    )
    .unwrap();

    let output = list_cmd(cwd.path(), home.path())
        .arg("list")
        .output()
        .unwrap();
    assert!(output.status.success(), "command failed: {output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();

    // All three deps present.
    assert!(stdout.contains("shared-a"), "stdout: {stdout}");
    assert!(stdout.contains("claude-only"), "stdout: {stdout}");
    assert!(stdout.contains("codex-only"), "stdout: {stdout}");
    // Headings present and ordered: Shared first, then per-CLI sections.
    let shared_idx = stdout.find("Shared:").expect("missing Shared: heading");
    let claude_idx = stdout.find("[claude]:").expect("missing [claude]: heading");
    let codex_idx = stdout.find("[codex]:").expect("missing [codex]: heading");
    assert!(
        shared_idx < claude_idx,
        "Shared should come before [claude]"
    );
    assert!(
        claude_idx < codex_idx,
        "[claude] should come before [codex]"
    );

    // Grouping: `claude-only` appears after the `[claude]:` heading, before `[codex]:`.
    let claude_only_idx = stdout.find("claude-only").unwrap();
    assert!(claude_only_idx > claude_idx && claude_only_idx < codex_idx);
}

// ---------------------------------------------------------------------------
// Step 5: manifest present, no lock — manifest-only fallback. Declared deps
// still show up with a `(not installed)` marker so the user sees they exist
// but haven't been resolved yet.
// ---------------------------------------------------------------------------

#[test]
fn step5_list_without_lock_falls_back_to_manifest() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]

[dependencies]
never-installed = { source = "local:/tmp/x" }
"#,
    )
    .unwrap();
    // Deliberately no Agentfile.lock.

    list_cmd(cwd.path(), home.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("never-installed"))
        .stdout(predicate::str::contains("(not installed)"));
}

// ---------------------------------------------------------------------------
// Step 7: global scope — point HOME at a tempdir, create ~/.agix/Agentfile,
// and list against it.
// ---------------------------------------------------------------------------

#[test]
fn step7_list_global_scope_reads_home_agentfile() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    let global_dir = home.path().join(".agix");
    fs::create_dir_all(&global_dir).unwrap();
    fs::write(
        global_dir.join("Agentfile"),
        r#"[agix]
cli = ["claude"]

[dependencies]
global-dep = { source = "local:/tmp/g" }
"#,
    )
    .unwrap();
    fs::write(
        global_dir.join("Agentfile.lock"),
        r#"
[[package]]
name = "global-dep"
source = "local:/tmp/g"
content_hash = "aabbccddeeff0011"
cli = ["claude"]
scope = "global"
files = []
"#,
    )
    .unwrap();

    list_cmd(cwd.path(), home.path())
        .args(["list", "-g"])
        .assert()
        .success()
        .stdout(predicate::str::contains("global-dep"))
        .stdout(predicate::str::contains("local:/tmp/g"));
}

#[test]
fn step7_list_global_scope_auto_inits_then_reports_empty() {
    // Fresh HOME with no global Agentfile — `agentfile_paths` auto-creates one
    // (non-interactively, because AGIX_NO_INTERACTIVE=1). The resulting
    // manifest has no deps, so list should report the friendly empty state.
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    list_cmd(cwd.path(), home.path())
        .args(["list", "-g"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No dependencies declared"));

    // Confirms the auto-init landed the file where we expect.
    assert!(home.path().join(".agix").join("Agentfile").exists());
}
