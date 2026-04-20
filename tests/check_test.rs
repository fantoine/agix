use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

mod helpers;

// `check` does not prompt, but we still route through the shared helper so
// integration tests stay hermetic (tempdir HOME) and uniform across the suite.
// A throwaway HOME tempdir is created per call — `check` itself does not touch
// HOME, but future TTY/driver probes might, and centralising the override
// prevents regressions.
fn check_cmd() -> Command {
    let tmp = tempdir().unwrap();
    // Leak the tempdir for the command's lifetime. `check` doesn't write to
    // HOME, and each test only uses the returned Command once, so the leak is
    // bounded to the test binary's runtime. Alternative (passing HOME in per
    // test) would be noisier than the bug this helper prevents.
    let home = tmp.keep();
    helpers::cmd_non_interactive(&home)
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
        .stdout(predicates::str::contains("project for not-a-real-cli"))
        .stderr(predicates::str::contains("Unknown CLI"))
        .stderr(predicates::str::contains("not-a-real-cli"));
}
