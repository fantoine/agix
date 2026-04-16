pub async fn run(global: bool) -> anyhow::Result<()> {
    let lock_path = if global {
        dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("no home dir"))?
            .join(".agix")
            .join("Agentfile.lock")
    } else {
        std::env::current_dir()?.join("Agentfile.lock")
    };

    let lock = crate::core::lock::LockFile::from_file_or_default(&lock_path);
    if lock.packages.is_empty() {
        crate::output::info("No packages installed.");
        return Ok(());
    }
    for pkg in &lock.packages {
        let version = pkg
            .sha
            .as_deref()
            .map(|s| &s[..s.len().min(7)])
            .unwrap_or("local");
        println!("  {} @ {} ({})", pkg.name, version, pkg.cli.join(", "));
    }
    Ok(())
}
