use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn outdated_empty_when_no_lock() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["outdated"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("No packages installed"));
}

#[test]
fn outdated_shows_local_source_notice() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile.lock"),
        r#"
[[package]]
name = "my-tool"
source = "local:../my-tool"
cli = ["claude"]
scope = "local"
files = []
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["outdated"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("local source"));
}

#[test]
fn outdated_shows_remote_not_implemented_notice() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile.lock"),
        r#"
[[package]]
name = "claude-later"
source = "github:fantoine/claude-later"
sha = "a1b2c3d"
cli = ["claude"]
scope = "local"
files = []
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["outdated"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "checking remote not yet implemented",
        ));
}
