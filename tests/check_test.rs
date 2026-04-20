use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn check_cmd() -> Command {
    let mut cmd = Command::cargo_bin("agix").unwrap();
    // `check` does not prompt, but we follow the project convention to keep
    // integration tests hermetic regardless of any future TTY touches.
    cmd.env("AGIX_NO_INTERACTIVE", "1");
    cmd
}

#[test]
fn check_valid_project_manifest() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["claude"]

[dependencies]
claude-later = "github:fantoine/claude-later"
"#,
    )
    .unwrap();

    check_cmd()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("project for claude"));
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
cli = ["claude"]
"#,
    )
    .unwrap();

    check_cmd()
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
cli = ["claude"]
"#,
    )
    .unwrap();

    check_cmd()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("version"));
}

#[test]
fn check_missing_agentfile_fails() {
    let dir = tempdir().unwrap();

    check_cmd()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("No Agentfile"));
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

    check_cmd()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("cli"));
}

#[test]
fn check_invalid_toml_reports_line_col() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix
cli = ["claude"]
"#,
    )
    .unwrap();

    check_cmd()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("line"))
        .stderr(predicates::str::contains("column"));
}

#[test]
fn check_unparseable_top_level_dep_source_fails() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["claude"]

[dependencies]
my-broken-dep = "nope:whatever"
"#,
    )
    .unwrap();

    check_cmd()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("my-broken-dep"));
}

#[test]
fn check_unparseable_per_cli_dep_source_fails() {
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["claude"]

[claude.dependencies]
cli-broken-dep = "nope:whatever"
"#,
    )
    .unwrap();

    check_cmd()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .failure()
        .stderr(predicates::str::contains("cli-broken-dep"))
        .stderr(predicates::str::contains("claude"));
}

#[test]
fn check_unknown_cli_warns_but_succeeds() {
    // Decision: unknown CLI names in [agix].cli are a warning (not an error).
    // `check` validates manifest structure; the user may be preparing a
    // manifest for a CLI they'll install / register later.
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["not-a-real-cli"]
"#,
    )
    .unwrap();

    check_cmd()
        .arg("check")
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "project for not-a-real-cli",
        ))
        .stderr(predicates::str::contains("Unknown CLI"))
        .stderr(predicates::str::contains("not-a-real-cli"));
}
