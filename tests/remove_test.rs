use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::{tempdir, TempDir};

/// Pre-seeded `agix` command for `remove` tests: always non-interactive and
/// bound to a scratch `HOME` so driver uninstalls never touch the host.
fn remove_cmd(cwd: &Path, home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("agix").unwrap();
    cmd.env("AGIX_NO_INTERACTIVE", "1")
        .env("HOME", home)
        .current_dir(cwd);
    cmd
}

/// Same seeding for the `install` preamble used by most remove scenarios.
fn install_cmd(cwd: &Path, home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("agix").unwrap();
    cmd.env("AGIX_NO_INTERACTIVE", "1")
        .env("HOME", home)
        .current_dir(cwd);
    cmd
}

/// Create a scratch workspace + HOME + local package dir containing a skill.
fn setup_workspace() -> (TempDir, TempDir, TempDir) {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();
    fs::create_dir(pkg.path().join("skills")).unwrap();
    fs::write(pkg.path().join("skills").join("s.md"), "# s").unwrap();
    (cwd, home, pkg)
}

fn write_claude_shim(dir: &Path) -> std::path::PathBuf {
    let log = dir.join("claude-invocations.log");
    let shim = dir.join("claude");
    fs::write(
        &shim,
        format!("#!/bin/sh\necho \"$@\" >> {}\nexit 0\n", log.display()),
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
    log
}

// ---------- Step 1: base sanity — remove shared dep via no-filter path ----------

#[tokio::test]
async fn step1_remove_shared_dep_updates_agentfile_and_lock() {
    let (cwd, home, pkg) = setup_workspace();

    let manifest = format!(
        r#"
[agix]
cli = ["claude"]

[dependencies]
my-pkg = {{ source = "local:{}" }}
"#,
        pkg.path().display()
    );
    fs::write(cwd.path().join("Agentfile"), &manifest).unwrap();

    install_cmd(cwd.path(), home.path())
        .arg("install")
        .assert()
        .success();

    // Lock contains my-pkg after install.
    let lock_before = fs::read_to_string(cwd.path().join("Agentfile.lock")).unwrap();
    assert!(lock_before.contains("my-pkg"), "expected my-pkg in lock");

    remove_cmd(cwd.path(), home.path())
        .args(["remove", "my-pkg"])
        .assert()
        .success();

    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    assert!(!content.contains("my-pkg"), "Agentfile should not contain my-pkg");
    let lock_after = fs::read_to_string(cwd.path().join("Agentfile.lock")).unwrap();
    assert!(!lock_after.contains("my-pkg"), "lock should not contain my-pkg");
}

// ---------- Step 3: remove --cli claude leaves dep in codex section ----------

#[tokio::test]
async fn step3_remove_with_cli_filter_only_removes_from_that_section() {
    let (cwd, home, pkg) = setup_workspace();

    let manifest = format!(
        r#"
[agix]
cli = ["claude", "codex"]

[claude.dependencies]
my-pkg = {{ source = "local:{0}" }}

[codex.dependencies]
my-pkg = {{ source = "local:{0}" }}
"#,
        pkg.path().display()
    );
    fs::write(cwd.path().join("Agentfile"), &manifest).unwrap();

    install_cmd(cwd.path(), home.path())
        .arg("install")
        .assert()
        .success();

    remove_cmd(cwd.path(), home.path())
        .args(["remove", "my-pkg", "--cli", "claude"])
        .assert()
        .success();

    // Parse back to assert structurally instead of string-matching TOML layout.
    let manifest =
        agix::manifest::agentfile::ProjectManifest::from_file(&cwd.path().join("Agentfile"))
            .unwrap();
    let claude_deps = manifest.cli_dependencies.get("claude");
    let codex_deps = manifest.cli_dependencies.get("codex");
    assert!(
        claude_deps.map(|m| !m.contains_key("my-pkg")).unwrap_or(true),
        "my-pkg should be gone from claude section"
    );
    assert!(
        codex_deps.is_some_and(|m| m.contains_key("my-pkg")),
        "my-pkg should remain under codex section"
    );
}

// ---------- Step 4: remove a non-existent package → clear error ----------

#[tokio::test]
async fn step4_remove_nonexistent_package_fails_with_clear_error() {
    let (cwd, home, _pkg) = setup_workspace();
    fs::write(
        cwd.path().join("Agentfile"),
        "[agix]\ncli = [\"claude\"]\n",
    )
    .unwrap();
    // Seed an empty lock so we hit "package not found" rather than "no lock".
    fs::write(
        cwd.path().join("Agentfile.lock"),
        "[[package]]\nname = \"other\"\nsource = \"local:/tmp/x\"\ncli = []\nscope = \"local\"\nfiles = []\n",
    )
    .unwrap();

    remove_cmd(cwd.path(), home.path())
        .args(["remove", "ghost-pkg"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("ghost-pkg"));
}

// ---------- Step 5: remove when no lock exists → warn + no-op (decision) ----------

#[tokio::test]
async fn step5_remove_when_no_lock_warns_and_updates_manifest() {
    // Decision (Task 11): if the package isn't in the lock (e.g. no lock file
    // yet), the manifest edit is still valuable. Warn instead of failing so
    // the user's intent ("drop this dep from my manifest") is honoured.
    let (cwd, home, pkg) = setup_workspace();

    let manifest = format!(
        r#"
[agix]
cli = ["claude"]

[dependencies]
my-pkg = {{ source = "local:{}" }}
"#,
        pkg.path().display()
    );
    fs::write(cwd.path().join("Agentfile"), &manifest).unwrap();

    // No install → no lock file present.
    assert!(!cwd.path().join("Agentfile.lock").exists());

    remove_cmd(cwd.path(), home.path())
        .args(["remove", "my-pkg"])
        .assert()
        .success()
        .stderr(predicates::str::contains("no lock file"));

    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    assert!(!content.contains("my-pkg"), "manifest entry should still be removed");
}

// ---------- Step 6: remove shared dep installed across multiple CLIs ----------

#[tokio::test]
async fn step6_remove_shared_dep_cleans_all_cli_dirs_and_lock() {
    let (cwd, home, pkg) = setup_workspace();

    let manifest = format!(
        r#"
[agix]
cli = ["claude", "codex"]

[dependencies]
my-pkg = {{ source = "local:{}" }}
"#,
        pkg.path().display()
    );
    fs::write(cwd.path().join("Agentfile"), &manifest).unwrap();

    install_cmd(cwd.path(), home.path())
        .arg("install")
        .assert()
        .success();

    // After install, claude + codex both have files under HOME.
    let claude_skill = home.path().join(".claude").join("skills").join("s.md");
    let codex_skill = home
        .path()
        .join(".codex")
        .join("agix")
        .join("my-pkg")
        .join("skills")
        .join("s.md");
    // Local-scope claude goes under cwd/.claude, global claude under HOME.
    // The default scope is local, so we look at cwd for claude.
    let claude_local = cwd.path().join(".claude").join("skills").join("s.md");
    assert!(
        claude_skill.exists() || claude_local.exists(),
        "expected claude to have installed a file somewhere"
    );
    // Codex always writes under ~/.codex/agix regardless of scope today.
    assert!(codex_skill.exists(), "expected codex file at {codex_skill:?}");

    remove_cmd(cwd.path(), home.path())
        .args(["remove", "my-pkg"])
        .assert()
        .success();

    // Files gone from every CLI dir.
    assert!(!claude_skill.exists(), "claude file should be gone");
    assert!(!claude_local.exists(), "claude local file should be gone");
    assert!(!codex_skill.exists(), "codex file should be gone");

    // Lock has no my-pkg entry.
    let lock = fs::read_to_string(cwd.path().join("Agentfile.lock")).unwrap();
    assert!(!lock.contains("my-pkg"), "lock should not contain my-pkg: {lock}");

    // Agentfile has no my-pkg entry.
    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    assert!(!content.contains("my-pkg"));
}

// ---------- Step 7: remove in global scope ----------

#[tokio::test]
async fn step7_remove_in_global_scope_touches_global_files() {
    let (cwd, home, pkg) = setup_workspace();

    let global_dir = home.path().join(".agix");
    fs::create_dir_all(&global_dir).unwrap();
    let manifest = format!(
        r#"
[agix]
cli = ["claude"]

[dependencies]
my-pkg = {{ source = "local:{}" }}
"#,
        pkg.path().display()
    );
    fs::write(global_dir.join("Agentfile"), &manifest).unwrap();

    install_cmd(cwd.path(), home.path())
        .args(["install", "--scope", "global"])
        .assert()
        .success();

    // Claude's global scope writes under HOME/.claude.
    let claude_global_skill = home.path().join(".claude").join("skills").join("s.md");
    assert!(
        claude_global_skill.exists(),
        "expected installed file at {claude_global_skill:?}"
    );

    remove_cmd(cwd.path(), home.path())
        .args(["remove", "my-pkg", "--scope", "global"])
        .assert()
        .success();

    assert!(!claude_global_skill.exists(), "global file should be removed");

    let global_lock = global_dir.join("Agentfile.lock");
    let lock = fs::read_to_string(&global_lock).unwrap();
    assert!(!lock.contains("my-pkg"), "global lock should not contain my-pkg");

    let content = fs::read_to_string(global_dir.join("Agentfile")).unwrap();
    assert!(!content.contains("my-pkg"));
}

// ---------- Step 8: remove a marketplace-installed plugin invokes driver ----------

#[tokio::test]
async fn step8_remove_marketplace_plugin_invokes_claude_uninstall() {
    let bin_dir = tempdir().unwrap();
    let log_path = write_claude_shim(bin_dir.path());

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        "[agix]\ncli = [\"claude\"]\n\n[dependencies]\nroundtable = { source = \"marketplace:fantoine/claude-plugins@roundtable\" }\n",
    )
    .unwrap();

    let path_env = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    install_cmd(cwd.path(), home.path())
        .env("PATH", &path_env)
        .arg("install")
        .assert()
        .success();

    // Install logged marketplace add + install.
    let log = fs::read_to_string(&log_path).unwrap();
    assert!(log.contains("plugin install roundtable@fantoine/claude-plugins"));

    remove_cmd(cwd.path(), home.path())
        .env("PATH", &path_env)
        .args(["remove", "roundtable"])
        .assert()
        .success();

    let log = fs::read_to_string(&log_path).unwrap();
    assert!(
        log.contains("plugin uninstall roundtable@fantoine/claude-plugins"),
        "expected claude plugin uninstall invocation; got: {log}"
    );

    // Lock entry gone.
    let lock = fs::read_to_string(cwd.path().join("Agentfile.lock")).unwrap();
    assert!(!lock.contains("roundtable"), "lock should not contain roundtable: {lock}");
}

// ---------- Regression: mangled lock source falls back to file-based uninstall ----------

#[tokio::test]
async fn regression_mangled_lock_source_falls_back_and_warns() {
    // Lock contains a `source` string that doesn't parse (no scheme). Legacy
    // fix: abort with parse error. New behavior: warn + fall back to
    // file-based uninstall using `pkg.files`, then remove the lock entry.
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    // Create a file that our "mangled" lock entry claims to own.
    let owned_dir = cwd.path().join(".claude").join("skills");
    fs::create_dir_all(&owned_dir).unwrap();
    let owned_file = owned_dir.join("s.md");
    fs::write(&owned_file, "# s").unwrap();

    fs::write(
        cwd.path().join("Agentfile"),
        "[agix]\ncli = [\"claude\"]\n\n[dependencies]\nmangled-pkg = { source = \"local:/tmp/nope\" }\n",
    )
    .unwrap();

    // Hand-crafted lock with an unparseable source (no colon/scheme).
    let lock_contents = format!(
        r#"[[package]]
name = "mangled-pkg"
source = "this-has-no-scheme"
cli = ["claude"]
scope = "local"

[[package.files]]
dest = "{}"
"#,
        owned_file.display()
    );
    fs::write(cwd.path().join("Agentfile.lock"), &lock_contents).unwrap();

    remove_cmd(cwd.path(), home.path())
        .args(["remove", "mangled-pkg"])
        .assert()
        .success()
        .stderr(predicates::str::contains("lock source unparseable"));

    // File was deleted via fallback.
    assert!(!owned_file.exists(), "tracked file should be removed via fallback");

    // Lock entry was dropped.
    let lock_after = fs::read_to_string(cwd.path().join("Agentfile.lock")).unwrap();
    assert!(
        !lock_after.contains("mangled-pkg"),
        "lock entry should be gone: {lock_after}"
    );
}

// ---------- Regression: lenient --cli filter accepts unknown driver (no-op) ----------

#[tokio::test]
async fn regression_remove_unknown_cli_filter_is_lenient() {
    // `remove --cli <unknown>` does not error (unlike `add`). Users may be
    // cleaning up legacy CLI sections. If the section does not exist, the
    // remove is a no-op for that filter entry.
    let (cwd, home, pkg) = setup_workspace();

    let manifest = format!(
        r#"
[agix]
cli = ["claude"]

[dependencies]
my-pkg = {{ source = "local:{}" }}
"#,
        pkg.path().display()
    );
    fs::write(cwd.path().join("Agentfile"), &manifest).unwrap();

    install_cmd(cwd.path(), home.path())
        .arg("install")
        .assert()
        .success();

    // --cli legacy-cli: no section exists → no-op, but no error.
    remove_cmd(cwd.path(), home.path())
        .args(["remove", "my-pkg", "--cli", "legacy-cli"])
        .assert()
        .success();

    // Shared dep unchanged (the filter targets the legacy-cli section only).
    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    assert!(content.contains("my-pkg"), "shared dep should remain under [dependencies]");
}
