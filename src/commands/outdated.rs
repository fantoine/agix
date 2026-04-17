pub async fn run(scope: &str) -> anyhow::Result<()> {
    let (_, lock_path, _) = super::agentfile_paths(scope)?;
    let lock = crate::core::lock::LockFile::from_file_or_default(&lock_path);
    if lock.packages.is_empty() {
        crate::output::info("No packages installed.");
        return Ok(());
    }
    for pkg in &lock.packages {
        if pkg.source.starts_with("local:") {
            println!(
                "  {} — local source, run `agix update {}` to refresh",
                pkg.name, pkg.name
            );
        } else {
            let version = pkg
                .sha
                .as_deref()
                .map(|s| &s[..s.len().min(7)])
                .unwrap_or("unknown");
            println!(
                "  {} @ {} — checking remote not yet implemented",
                pkg.name, version
            );
        }
    }
    Ok(())
}
