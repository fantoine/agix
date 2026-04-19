use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn doctor_warns_without_agentfile() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["doctor"])
        .current_dir(&dir)
        .assert()
        .success()
        .stderr(predicates::str::contains("No Agentfile"));
}

#[test]
fn doctor_reports_all_files_present_when_lock_empty() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["claude-code"]
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["doctor"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("All installed files present"));
}

#[test]
fn doctor_reports_local_claude_config_when_dot_claude_exists() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["claude-code"]
"#,
    )
    .unwrap();
    std::fs::create_dir(dir.path().join(".claude")).unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["doctor"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("local config at"));
}

#[test]
fn doctor_reports_no_local_config_when_absent() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["claude-code"]
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["doctor"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("claude-code"))
        .stdout(predicates::str::contains("codex"))
        .stdout(predicates::str::contains("no local config"));
}

#[test]
fn doctor_warns_on_missing_installed_file() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["claude-code"]
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Agentfile.lock"),
        r#"
[[package]]
name = "claude-later"
source = "github:fantoine/claude-later"
sha = "a1b2c3d"
cli = ["claude-code"]
scope = "local"

[[package.files]]
dest = "/nonexistent/path/that/does/not/exist.md"
"#,
    )
    .unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .args(["doctor"])
        .current_dir(&dir)
        .assert()
        .success()
        .stderr(predicates::str::contains("missing"));
}
