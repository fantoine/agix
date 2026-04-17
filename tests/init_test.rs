use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn init_creates_agentfile() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix").unwrap()
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

    Command::cargo_bin("agix").unwrap()
        .args(["init"])
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("Already initialized"));
}
