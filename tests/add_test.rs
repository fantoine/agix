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
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = [\"claude-code\"]\n").unwrap();

    let pkg_dir = tempdir().unwrap();
    std::fs::write(pkg_dir.path().join("skill.md"), "# skill").unwrap();
    let source = format!("local:{}", pkg_dir.path().display());

    let mut cmd = Command::cargo_bin("agix").unwrap();
    cmd.current_dir(dir.path())
        .arg("add")
        .arg(&source)
        .arg("--cli")
        .arg("claude-code");
    cmd.assert().success();

    let content = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    assert!(content.contains("local:"));
}
