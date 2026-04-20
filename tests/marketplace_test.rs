use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

/// Smoke test: `agix add marketplace ...` invokes the `claude` CLI correctly.
/// Uses a shim `claude` on PATH that records arguments — no network, no real
/// Claude Code dependency.
#[test]
fn add_marketplace_invokes_claude_cli_for_install() {
    let bin_dir = tempdir().unwrap();
    let log_path = bin_dir.path().join("claude-invocations.log");
    let shim = bin_dir.path().join("claude");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\necho \"$@\" >> {}\nexit 0\n",
            log_path.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();

    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    fs::write(cwd.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    let path_env = format!("{}:{}", bin_dir.path().display(), std::env::var("PATH").unwrap_or_default());

    Command::cargo_bin("agix")
        .unwrap()
        .env("AGIX_NO_INTERACTIVE", "1")
        .env("HOME", home.path())
        .env("PATH", &path_env)
        .current_dir(cwd.path())
        .args(["add", "marketplace", "fantoine/claude-plugins@roundtable"])
        .assert()
        .success();

    let log = fs::read_to_string(&log_path).unwrap();
    // Expect both `plugin marketplace add` and `plugin install` calls.
    assert!(
        log.contains("plugin marketplace add fantoine/claude-plugins"),
        "missing marketplace add invocation; log: {log}"
    );
    assert!(
        log.contains("plugin install roundtable@fantoine/claude-plugins"),
        "missing plugin install invocation; log: {log}"
    );
}
