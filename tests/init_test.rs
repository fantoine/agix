use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn global_scope_auto_inits_agentfile_if_missing() {
    let fake_home = tempdir().unwrap();
    let pkg_dir = tempdir().unwrap();
    std::fs::write(pkg_dir.path().join("skill.md"), "# skill").unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .env("HOME", fake_home.path())
        .env("AGIX_NO_INTERACTIVE", "1")
        .arg("add")
        .arg("local")
        .arg(pkg_dir.path())
        .arg("--scope")
        .arg("global")
        .assert()
        .success();

    assert!(fake_home.path().join(".agix").join("Agentfile").exists());
}

#[test]
fn init_creates_agentfile() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["init", "--no-interactive"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("Created"));

    assert!(dir.path().join("Agentfile").exists());
}

#[test]
fn init_fails_if_already_initialized() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = []\n").unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["init", "--no-interactive"])
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("Already initialized"));
}

#[test]
fn init_with_preselected_clis_writes_them_to_agentfile() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args([
            "init",
            "--no-interactive",
            "--cli",
            "claude",
            "--cli",
            "codex",
        ])
        .current_dir(&dir)
        .assert()
        .success();

    let contents = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    assert!(contents.contains("\"claude\""), "got: {contents}");
    assert!(contents.contains("\"codex\""), "got: {contents}");
}

#[test]
fn init_deduplicates_repeated_cli_flags() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args([
            "init",
            "--no-interactive",
            "--cli",
            "claude",
            "--cli",
            "claude",
        ])
        .current_dir(&dir)
        .assert()
        .success();

    let contents = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    // Exactly one occurrence of "claude" as a list entry.
    assert_eq!(contents.matches("\"claude\"").count(), 1, "got: {contents}");
}

#[test]
fn init_global_scope_creates_home_agentfile() {
    let fake_home = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .env("HOME", fake_home.path())
        .args([
            "init",
            "--scope",
            "global",
            "--no-interactive",
            "--cli",
            "claude",
        ])
        .assert()
        .success();

    let path = fake_home.path().join(".agix").join("Agentfile");
    assert!(path.exists(), "expected {} to exist", path.display());
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("\"claude\""), "got: {contents}");
}

#[test]
fn init_global_scope_fails_if_already_initialized() {
    let fake_home = tempdir().unwrap();
    let agix_dir = fake_home.path().join(".agix");
    std::fs::create_dir_all(&agix_dir).unwrap();
    std::fs::write(agix_dir.join("Agentfile"), "[agix]\ncli = []\n").unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .env("HOME", fake_home.path())
        .args(["init", "--scope", "global", "--no-interactive"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("Already initialized"));
}

#[test]
fn init_rejects_unknown_cli() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["init", "--no-interactive", "--cli", "unknown"])
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown CLI 'unknown'"));

    assert!(
        !dir.path().join("Agentfile").exists(),
        "Agentfile should not be created on validation error"
    );
}

#[test]
fn init_rejects_invalid_scope_at_clap_level() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["init", "--scope", "bogus", "--no-interactive"])
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("invalid value 'bogus'"));
}
