use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn global_scope_auto_inits_agentfile_if_missing() {
    let fake_home = tempdir().unwrap();
    let pkg_dir = tempdir().unwrap();
    std::fs::write(pkg_dir.path().join("skill.md"), "# skill").unwrap();
    let source = format!("local:{}", pkg_dir.path().display());

    Command::cargo_bin("agix")
        .unwrap()
        .env("HOME", fake_home.path())
        .arg("add")
        .arg(&source)
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
        .args(["init"])
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
        .args(["init"])
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("Already initialized"));
}
