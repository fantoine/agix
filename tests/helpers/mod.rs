#![allow(dead_code)]
use assert_cmd::Command;
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
