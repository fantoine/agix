use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn init_creates_agentfile() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("agix").unwrap();
    cmd.current_dir(dir.path()).arg("init");
    cmd.assert().success();
    assert!(dir.path().join("Agentfile").exists());
    let content = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    assert!(content.contains("[agix]"));
}

#[test]
fn init_fails_if_agentfile_exists() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = []\n").unwrap();
    let mut cmd = Command::cargo_bin("agix").unwrap();
    cmd.current_dir(dir.path()).arg("init");
    cmd.assert().failure();
}

#[test]
fn add_writes_dependency_to_agentfile() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        "[agix]\ncli = [\"claude-code\"]\n",
    )
    .unwrap();

    let pkg_dir = tempdir().unwrap();
    std::fs::write(pkg_dir.path().join("skill.md"), "# skill").unwrap();

    let mut cmd = Command::cargo_bin("agix").unwrap();
    cmd.current_dir(dir.path())
        .arg("add")
        .arg("local")
        .arg(pkg_dir.path())
        .arg("--cli")
        .arg("claude-code");
    cmd.assert().success();

    let content = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    assert!(content.contains("local:"));
}

#[test]
fn add_shared_dependency_without_cli_flag() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        "[agix]\ncli = [\"claude-code\"]\n",
    )
    .unwrap();

    let pkg_dir = tempdir().unwrap();
    std::fs::write(pkg_dir.path().join("skill.md"), "# skill").unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .arg("add")
        .arg("local")
        .arg(pkg_dir.path())
        .assert()
        .success();

    let content = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    // Serialized as [dependencies.<name>] — check the section key appears
    assert!(content.contains("dependencies"));
    // Should NOT be under a CLI-specific section
    assert!(!content.contains("claude-code.dependencies"));
}

#[test]
fn add_multi_cli_dependency() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        "[agix]\ncli = [\"claude-code\", \"codex\"]\n",
    )
    .unwrap();

    let pkg_dir = tempdir().unwrap();
    std::fs::write(pkg_dir.path().join("skill.md"), "# skill").unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .arg("add")
        .arg("local")
        .arg(pkg_dir.path())
        .arg("--cli")
        .arg("claude-code")
        .arg("--cli")
        .arg("codex")
        .assert()
        .success();

    let content = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    assert!(content.contains("claude-code"));
    assert!(content.contains("codex"));
}

#[test]
fn add_local_with_separate_type_and_value() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        "[agix]\ncli = [\"claude-code\"]\n",
    )
    .unwrap();
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
