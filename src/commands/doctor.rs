pub async fn run() -> anyhow::Result<()> {
    let dir = std::env::current_dir()?;
    let agentfile_path = dir.join("Agentfile");
    let lock_path = dir.join("Agentfile.lock");

    println!("Agix Doctor\n");

    if !agentfile_path.exists() {
        crate::output::warn("No Agentfile in current directory");
        return Ok(());
    }

    // Parse the Agentfile to surface any syntax errors early, even though we
    // no longer restrict driver reporting to declared CLIs.
    let _ = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    crate::output::info("CLI drivers:");
    for driver in crate::drivers::all_drivers() {
        let global = if driver.detect() {
            "detected"
        } else {
            "not detected"
        };
        let local = match driver.detect_local_config(&dir) {
            Some(p) => format!("local config at {}", p.display()),
            None => "no local config".to_string(),
        };
        println!("  - {}: {} | {}", driver.name(), global, local);
    }

    let lock = crate::core::lock::LockFile::from_file_or_default(&lock_path);
    let mut missing = 0usize;
    for pkg in &lock.packages {
        for file in &pkg.files {
            if !std::path::Path::new(&file.dest).exists() {
                crate::output::warn(&format!("missing: {} (from {})", file.dest, pkg.name));
                missing += 1;
            }
        }
    }
    if missing == 0 {
        crate::output::success("All installed files present");
    } else {
        println!("\n  Run `agix install` to restore missing files.");
    }
    Ok(())
}
