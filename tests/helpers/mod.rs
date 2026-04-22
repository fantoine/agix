#![allow(dead_code)]
use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct TestEnv {
    pub cwd: TempDir,
    pub home: TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        Self {
            cwd: tempfile::tempdir().unwrap(),
            home: tempfile::tempdir().unwrap(),
        }
    }

    pub fn write_agentfile(&self, content: &str) {
        std::fs::write(self.cwd.path().join("Agentfile"), content).unwrap();
    }

    pub fn agentfile_content(&self) -> String {
        std::fs::read_to_string(self.cwd.path().join("Agentfile")).unwrap()
    }

    pub fn fixture(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(relative)
    }
}

/// Pre-configured `assert_cmd::Command` for the `agix` binary in non-interactive
/// mode, with `HOME` overridden to a caller-provided tempdir so driver installs
/// never leak into the developer's real `~/.claude/` or `~/.codex/`.
///
/// This is the standard entry point for every integration test that executes
/// the CLI. Build the command and chain further `.env()` / `.arg()` / `.current_dir()`
/// calls as needed.
///
/// Declaration pattern — because integration tests are separate crates, each
/// test file that wants this helper must declare `mod helpers;` at the top so
/// the compiler actually pulls `tests/helpers/mod.rs` into the test binary.
pub fn cmd_non_interactive(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("agix").unwrap();
    cmd.env("AGIX_NO_INTERACTIVE", "1").env("HOME", home);
    cmd
}

/// Install a stateful `claude` shim on PATH.
///
/// The shim logs every invocation to `log_path` and mimics the four claude
/// subcommands agix exercises:
///
/// - `plugin marketplace list --json` → reads state file or returns `[]`
/// - `plugin marketplace add <source>` → writes state with alias = basename(source)
/// - `plugin list --json` → reads state file or returns `[]`
/// - `plugin install <plugin>@<alias>` → writes state entry for that id
/// - `plugin uninstall <plugin>@<alias>` → clears the plugin state
///
/// The `basename` alias rule deliberately makes the alias *differ* from the
/// `org/repo` passed to `add`, which is exactly the mismatch agix must handle
/// in real Claude Code installs (e.g. `fantoine/claude-plugins` → alias
/// `claude-plugins`).
pub fn install_claude_shim(bin_dir: &Path, log_path: &Path, state_dir: &Path) -> PathBuf {
    let shim = bin_dir.join("claude");
    fs::create_dir_all(state_dir).unwrap();
    let script = format!(
        r##"#!/bin/sh
LOG="{log}"
STATE="{state}"
echo "$*" >> "$LOG"

case "$*" in
  "plugin marketplace list"*)
    if [ -f "$STATE/mkts.json" ]; then cat "$STATE/mkts.json"; else printf '[]'; fi
    exit 0 ;;
  "plugin marketplace add "*)
    REPO="$4"
    ALIAS=$(basename "$REPO")
    printf '[{{"name":"%s","source":"github","repo":"%s"}}]' "$ALIAS" "$REPO" > "$STATE/mkts.json"
    exit 0 ;;
  "plugin list"*)
    if [ -f "$STATE/plugins.json" ]; then cat "$STATE/plugins.json"; else printf '[]'; fi
    exit 0 ;;
  "plugin install "*)
    REF="$3"
    printf '[{{"id":"%s","scope":"user"}}]' "$REF" > "$STATE/plugins.json"
    exit 0 ;;
  "plugin uninstall "*)
    rm -f "$STATE/plugins.json"
    exit 0 ;;
esac
exit 0
"##,
        log = log_path.display(),
        state = state_dir.display(),
    );
    fs::write(&shim, script).unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
    shim
}
