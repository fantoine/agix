pub mod claude_code;
pub mod codex;

use std::path::PathBuf;
use crate::core::lock::InstalledFile;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Scope { Global, Local }

impl Scope {
    pub fn as_str(&self) -> &str {
        match self { Scope::Global => "global", Scope::Local => "local" }
    }
}

pub struct FetchedPackage {
    pub path: PathBuf,
    pub sha: Option<String>,
    pub content_hash: Option<String>,
}

pub trait CliDriver: Send + Sync {
    fn name(&self) -> &str;
    fn detect(&self) -> bool;
    fn install(&self, pkg_name: &str, fetched: &FetchedPackage, scope: &Scope) -> Result<Vec<InstalledFile>>;
    fn uninstall(&self, files: &[InstalledFile]) -> Result<()>;
    fn install_from_marketplace(&self, identifier: &str, scope: &Scope) -> Result<Vec<InstalledFile>>;
}

pub fn all_drivers() -> Vec<Box<dyn CliDriver>> {
    vec![
        Box::new(claude_code::ClaudeCodeDriver),
        Box::new(codex::CodexDriver),
    ]
}

pub fn driver_for(cli_name: &str) -> Option<Box<dyn CliDriver>> {
    all_drivers().into_iter().find(|d| d.name() == cli_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_display() {
        assert_eq!(Scope::Global.as_str(), "global");
        assert_eq!(Scope::Local.as_str(), "local");
    }
}
