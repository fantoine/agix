use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;

/// A single file installed by a package.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstalledFile {
    pub dest: String,
}

/// A resolved and installed package entry in the lock file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockedPackage {
    pub name: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub cli: Vec<String>,
    pub scope: String,
    pub files: Vec<InstalledFile>,
}

/// The lock file (`Agentfile.lock`) tracking all installed packages and their
/// exact resolved state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LockFile {
    #[serde(rename = "package", default)]
    pub packages: Vec<LockedPackage>,
}

impl LockFile {
    /// Read the lock file from disk.
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let lock: LockFile = toml::from_str(&content)?;
        Ok(lock)
    }

    /// Read the lock file from disk, or return an empty default if the file
    /// does not exist.
    pub fn from_file_or_default(path: &Path) -> Self {
        if path.exists() {
            Self::from_file(path).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Write the lock file to disk.
    pub fn to_file(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Find a package by name (immutable).
    pub fn find(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|p| p.name == name)
    }

    /// Find a package by name (mutable).
    pub fn find_mut(&mut self, name: &str) -> Option<&mut LockedPackage> {
        self.packages.iter_mut().find(|p| p.name == name)
    }

    /// Remove a package by name.
    pub fn remove(&mut self, name: &str) {
        self.packages.retain(|p| p.name != name);
    }

    /// Insert or replace a package (remove existing entry, then push).
    pub fn upsert(&mut self, pkg: LockedPackage) {
        self.remove(&pkg.name.clone());
        self.packages.push(pkg);
    }
}
