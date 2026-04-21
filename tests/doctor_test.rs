use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

mod helpers;

// ---------------------------------------------------------------------------
// Step 2: clean system — every registered driver appears with a
// detection + local-config status. No Agentfile → warn and stop early.
// ---------------------------------------------------------------------------

#[test]
fn step2_doctor_without_agentfile_warns_and_exits_zero() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("doctor")
        .assert()
        .success()
        .stderr(predicate::str::contains("No Agentfile"));
}

#[test]
fn step2_doctor_lists_every_registered_driver_with_detection_state() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]
"#,
    )
    .unwrap();

    // With $HOME pointing at an empty tempdir and no `.claude` / `.codex` in
    // cwd, both drivers must still be listed — doctor reports *every* driver,
    // not just the ones the manifest targets.
    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("claude"))
        .stdout(predicate::str::contains("codex"))
        .stdout(predicate::str::contains("no local config"));
}

// ---------------------------------------------------------------------------
// Step 3: `.claude/` in cwd → local-config path surfaced for the claude driver.
// Driver name is "claude" post Phase A rename (was claude-code).
// ---------------------------------------------------------------------------

#[test]
fn step3_doctor_reports_local_claude_config_when_dot_claude_exists() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]
"#,
    )
    .unwrap();
    fs::create_dir(cwd.path().join(".claude")).unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("claude"))
        .stdout(predicate::str::contains("local config at"));
}

// ---------------------------------------------------------------------------
// Step 4: `.codex/` in cwd → same contract, codex driver.
// ---------------------------------------------------------------------------

#[test]
fn step4_doctor_reports_local_codex_config_when_dot_codex_exists() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["codex"]
"#,
    )
    .unwrap();
    fs::create_dir(cwd.path().join(".codex")).unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("codex"))
        .stdout(predicate::str::contains("local config at"));
}

// ---------------------------------------------------------------------------
// Step 5: a valid Agentfile is parsed and reported as such.
// ---------------------------------------------------------------------------

#[test]
fn step5_doctor_reports_agentfile_status_when_valid() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]
"#,
    )
    .unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("Agentfile: valid"));
}

// ---------------------------------------------------------------------------
// Step 6: broken Agentfile → surface the parse error and exit non-zero.
// Doctor is a diagnostic command; if the manifest won't parse the user needs
// to know loudly (non-zero exit for CI) rather than see a half-report.
// ---------------------------------------------------------------------------

#[test]
fn step6_doctor_with_broken_agentfile_reports_parse_error_and_exits_nonzero() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        // Invalid TOML: unterminated table header.
        r#"[agix
cli = ["claude"]
"#,
    )
    .unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Agentfile: invalid"));
}

// ---------------------------------------------------------------------------
// Step 7: Agentfile but no lock file → report the absence, don't claim
// "all files present" (which we can't know — we have no baseline).
// ---------------------------------------------------------------------------

#[test]
fn step7_doctor_without_lock_reports_no_lock_file() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]

[dependencies]
some-dep = { source = "local:/tmp/x" }
"#,
    )
    .unwrap();
    // Deliberately no Agentfile.lock.

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("no lock file"))
        .stdout(predicate::str::contains("agix install"))
        // Must NOT claim everything is fine when we have no baseline.
        .stdout(predicate::str::contains("All installed files present").not());
}

// ---------------------------------------------------------------------------
// Step 8: marketplace packages are labeled distinctly so the user knows
// doctor can't verify them itself (files are managed by the CLI).
// Addresses the deferred "marketplace silently passes" finding.
// ---------------------------------------------------------------------------

#[test]
fn step8_doctor_labels_marketplace_packages_as_cli_managed() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]

[dependencies]
roundtable = { source = "marketplace:fantoine/claude-plugins@roundtable" }
"#,
    )
    .unwrap();
    fs::write(
        cwd.path().join("Agentfile.lock"),
        r#"
[[package]]
name = "roundtable"
source = "marketplace:fantoine/claude-plugins@roundtable"
cli = ["claude"]
scope = "local"
files = []
"#,
    )
    .unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("roundtable"))
        .stdout(predicate::str::contains("marketplace"))
        .stdout(predicate::str::contains("managed by claude"));
}

// ---------------------------------------------------------------------------
// Regression (existing behaviour preserved): tracked-file package with a
// missing file is reported to stderr. Unchanged from pre-review doctor.
// ---------------------------------------------------------------------------

#[test]
fn regression_doctor_warns_on_missing_installed_file() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]
"#,
    )
    .unwrap();
    fs::write(
        cwd.path().join("Agentfile.lock"),
        r#"
[[package]]
name = "claude-later"
source = "github:fantoine/claude-later"
sha = "a1b2c3d"
cli = ["claude"]
scope = "local"

[[package.files]]
dest = "/nonexistent/path/that/does/not/exist.md"
"#,
    )
    .unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("doctor")
        .assert()
        .success()
        .stderr(predicate::str::contains("missing"));
}

// ---------------------------------------------------------------------------
// Regression: when the lock lists a tracked-file package and every file is
// present on disk, doctor emits the green success line.
// ---------------------------------------------------------------------------

#[test]
fn regression_doctor_reports_all_present_when_tracked_files_exist() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    let tracked = cwd.path().join("tracked.md");
    fs::write(&tracked, "# tracked").unwrap();

    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]
"#,
    )
    .unwrap();
    fs::write(
        cwd.path().join("Agentfile.lock"),
        format!(
            r#"
[[package]]
name = "local-pkg"
source = "local:/tmp/fake"
content_hash = "deadbeef"
cli = ["claude"]
scope = "local"

[[package.files]]
dest = {dest:?}
"#,
            dest = tracked.to_str().unwrap(),
        ),
    )
    .unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("All installed files present"));
}
