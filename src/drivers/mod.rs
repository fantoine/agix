pub mod claude;
pub mod codex;

use crate::core::lock::InstalledFile;
use crate::error::Result;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Scope {
    Global,
    Local,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Local => "local",
        }
    }

    pub fn is_global(&self) -> bool {
        matches!(self, Scope::Global)
    }
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
    fn supports_marketplace(&self) -> bool;
    fn install(
        &self,
        pkg_name: &str,
        fetched: &FetchedPackage,
        scope: &Scope,
    ) -> Result<Vec<InstalledFile>>;
    fn uninstall(&self, files: &[InstalledFile]) -> Result<()>;

    /// Install a plugin via the CLI's native marketplace mechanism.
    /// Drivers that don't support marketplaces return `AgixError::Unsupported`.
    fn install_marketplace_plugin(
        &self,
        marketplace: &str,
        plugin: &str,
        scope: &Scope,
    ) -> Result<Vec<InstalledFile>>;

    /// Uninstall a marketplace-installed plugin via the CLI.
    fn uninstall_marketplace_plugin(&self, marketplace: &str, plugin: &str) -> Result<()>;

    /// Return Some(path) if this CLI has project-level config in the given cwd.
    fn detect_local_config(&self, cwd: &std::path::Path) -> Option<std::path::PathBuf>;
}

pub fn all_drivers() -> Vec<Box<dyn CliDriver>> {
    vec![Box::new(claude::ClaudeDriver), Box::new(codex::CodexDriver)]
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
