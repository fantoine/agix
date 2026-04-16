pub async fn run() -> anyhow::Result<()> {
    let dir = std::env::current_dir()?;
    let agentfile_path = dir.join("Agentfile");
    let lock_path = dir.join("Agentfile.lock");

    println!("Agix Doctor\n");

    if !agentfile_path.exists() {
        crate::output::warn("No Agentfile in current directory");
        return Ok(());
    }

    let manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;
    for cli in &manifest.agix.cli {
        match crate::drivers::driver_for(cli) {
            Some(driver) if driver.detect() => crate::output::success(&format!("{cli} detected")),
            Some(_) => crate::output::warn(&format!("{cli} declared but not detected")),
            None    => crate::output::warn(&format!("{cli} — no driver available")),
        }
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
