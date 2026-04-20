use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn update_fails_without_agentfile() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["update"])
        .current_dir(&dir)
        .assert()
        .failure();
}

#[test]
fn update_succeeds_with_empty_manifest() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["claude"]
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["update"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("Updated"));
}

#[test]
fn update_single_package_fails_without_agentfile() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["update", "claude-later"])
        .current_dir(&dir)
        .assert()
        .failure();
}
