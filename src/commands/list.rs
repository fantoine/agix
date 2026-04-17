pub async fn run(scope: &str) -> anyhow::Result<()> {
    let (_, lock_path, _) = super::agentfile_paths(scope)?;

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
