use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn list_empty_when_no_lock() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["list"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("No packages installed"));
}

#[test]
fn list_shows_installed_packages() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile.lock"),
        r#"
[[package]]
name = "claude-later"
source = "github:fantoine/claude-later"
sha = "a1b2c3d4e5f6"
cli = ["claude"]
scope = "local"
files = []
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["list"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("claude-later"));
}
