use std::fs;
use tempfile::{tempdir, TempDir};

mod helpers;

/// Shared setup: tempdirs for the shim binary, its state, its log file, plus
/// a cwd with a minimal Agentfile and a tempdir HOME.
///
/// Returning every `TempDir` by value is load-bearing: a dropped `TempDir`
/// cleans up its directory immediately, so the shim would find its `STATE`
/// path missing between calls.
struct Fixtures {
    _bin_dir: TempDir,
    _state_dir: TempDir,
    home: TempDir,
    cwd: TempDir,
    log_path: std::path::PathBuf,
    path_env: String,
}

fn setup() -> Fixtures {
    let bin_dir = tempdir().unwrap();
    let state_dir = tempdir().unwrap();
    let log_path = bin_dir.path().join("claude-invocations.log");
    helpers::install_claude_shim(bin_dir.path(), &log_path, state_dir.path());

    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    fs::write(cwd.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    let path_env = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Fixtures {
        _bin_dir: bin_dir,
        _state_dir: state_dir,
        home,
        cwd,
        log_path,
        path_env,
    }
}

/// Fresh system: marketplace not yet registered, plugin not yet installed.
/// We expect agix to: list marketplaces → add → re-list → list plugins → install,
/// using the marketplace's *alias* (from `list --json`) in `<plugin>@<alias>`.
#[test]
fn add_marketplace_resolves_alias_and_installs_when_absent() {
    let f = setup();

    helpers::cmd_non_interactive(f.home.path())
        .env("PATH", &f.path_env)
        .current_dir(f.cwd.path())
        .args(["add", "marketplace", "fantoine/claude-plugins@roundtable"])
        .assert()
        .success();

    let log = fs::read_to_string(&f.log_path).unwrap();
    // Marketplace is probed *then* added (fresh state).
    assert!(
        log.contains("plugin marketplace list --json"),
        "missing marketplace list probe; log:\n{log}"
    );
    assert!(
        log.contains("plugin marketplace add fantoine/claude-plugins"),
        "missing marketplace add invocation; log:\n{log}"
    );
    // Plugin install uses the alias (basename `claude-plugins`), not the
    // full `fantoine/claude-plugins` repo path — that's the entire point
    // of the alias lookup.
    assert!(
        log.contains("plugin install roundtable@claude-plugins"),
        "expected install with alias-based ref; log:\n{log}"
    );
    assert!(
        !log.contains("plugin install roundtable@fantoine/claude-plugins"),
        "regression: the old org/repo form leaked into the install call; log:\n{log}"
    );
}

/// Pre-populated system: the marketplace is already registered and the plugin
/// is already installed. agix must detect this and emit neither `add` nor
/// `install` — repeat runs must be a fast no-op.
#[test]
fn add_marketplace_is_a_noop_when_already_present() {
    let bin_dir = tempdir().unwrap();
    let state_dir = tempdir().unwrap();
    let log_path = bin_dir.path().join("claude-invocations.log");
    helpers::install_claude_shim(bin_dir.path(), &log_path, state_dir.path());

    // Pre-populate the shim state as if the user had already registered
    // and installed everything manually.
    fs::write(
        state_dir.path().join("mkts.json"),
        r#"[{"name":"claude-plugins","source":"github","repo":"fantoine/claude-plugins"}]"#,
    )
    .unwrap();
    fs::write(
        state_dir.path().join("plugins.json"),
        r#"[{"id":"roundtable@claude-plugins","scope":"user"}]"#,
    )
    .unwrap();

    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();
    fs::write(cwd.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    let path_env = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    helpers::cmd_non_interactive(home.path())
        .env("PATH", &path_env)
        .current_dir(cwd.path())
        .args(["add", "marketplace", "fantoine/claude-plugins@roundtable"])
        .assert()
        .success();

    let log = fs::read_to_string(&log_path).unwrap();
    // Still probes — that's how it decides to skip.
    assert!(log.contains("plugin marketplace list --json"));
    assert!(log.contains("plugin list --json"));
    // But nothing mutating.
    assert!(
        !log.contains("plugin marketplace add"),
        "marketplace was already present; add must not be invoked; log:\n{log}"
    );
    assert!(
        !log.contains("plugin install"),
        "plugin was already installed; install must not be invoked; log:\n{log}"
    );
}
