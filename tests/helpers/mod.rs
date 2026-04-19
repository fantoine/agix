#![allow(dead_code)]
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
