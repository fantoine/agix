use assert_cmd::Command;
use tempfile::tempdir;

/// Integration-level test: delegate to the real `claude` CLI to add a marketplace plugin.
/// Skipped when `claude` is not in PATH (CI).
#[test]
fn claude_install_marketplace_plugin_invokes_claude_cli() {
    if which::which("claude").is_err() {
        eprintln!("skipping: `claude` CLI not in PATH");
        return;
    }

    let home = tempdir().unwrap();
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .env("HOME", home.path())
        .current_dir(dir.path())
        .args(["add", "marketplace", "fantoine/claude-plugins@roundtable"])
        .assert()
        .success();
}
