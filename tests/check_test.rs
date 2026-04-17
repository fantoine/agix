use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn check_valid_project_manifest() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["claude-code"]

[dependencies]
claude-later = "github:fantoine/claude-later"
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("project for claude-code"));
}

#[test]
fn check_valid_package_manifest() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
name = "my-pkg"
version = "1.0.0"
cli = ["claude-code"]
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("package my-pkg v1.0.0"));
}

#[test]
fn check_package_missing_version_fails() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
name = "my-pkg"
cli = ["claude-code"]
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .failure();
}

#[test]
fn check_missing_agentfile_fails() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .failure();
}

#[test]
fn check_missing_cli_fails() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .failure();
}
