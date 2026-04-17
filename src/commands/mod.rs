pub mod add;
pub mod check;
pub mod doctor;
pub mod export;
pub mod init;
pub mod install;
pub mod list;
pub mod outdated;
pub mod remove;
pub mod update;

use std::path::PathBuf;

pub fn agentfile_paths(scope: &str) -> anyhow::Result<(PathBuf, PathBuf, &'static str)> {
    if scope == "global" {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
        let dir = home.join(".agix");
        std::fs::create_dir_all(&dir)?;
        Ok((dir.join("Agentfile"), dir.join("Agentfile.lock"), "global"))
    } else {
        let dir = std::env::current_dir()?;
        Ok((dir.join("Agentfile"), dir.join("Agentfile.lock"), "local"))
    }
}
