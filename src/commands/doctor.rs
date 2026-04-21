use crate::constants::manifest::{AGENTFILE, AGENTFILE_LOCK};

pub async fn run() -> anyhow::Result<()> {
    let dir = std::env::current_dir()?;
    let agentfile_path = dir.join(AGENTFILE);
    let lock_path = dir.join(AGENTFILE_LOCK);

    println!("Agix Doctor\n");

    if !agentfile_path.exists() {
        crate::output::warn("No Agentfile in current directory");
        return Ok(());
    }

    // Parse the Agentfile explicitly so we can surface a diagnostic-shaped
    // error line before bubbling up. Doctor is the place the user goes to
    // figure out what's broken — erroring with a clear `Agentfile: invalid
    // — <toml error>` is friendlier than a bare `Error: TOML parse error
    // at ...`, and we still exit non-zero so CI notices.
    if let Err(e) = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path) {
        crate::output::warn(&format!("Agentfile: invalid — {e}"));
        return Err(e.into());
    }
    crate::output::success("Agentfile: valid");

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

    // Git support: libgit2 is statically linked (always available); the git
    // CLI is optional and purely informational.
    let git = crate::sources::git::detect_git_support();
    crate::output::info("Git support:");
    println!("  - libgit2: {}", git.libgit2_version);
    println!(
        "  - git CLI: {}",
        if git.cli_available {
            "detected"
        } else {
            "not detected (optional)"
        }
    );

    // Lock-file state. Without a baseline we can't check installed-file
    // presence, so we report the absence and point at `agix install` rather
    // than silently running the (no-op) loop and printing a false
    // "all present" line.
    if !lock_path.exists() {
        crate::output::info(&format!(
            "no lock file at {} — run `agix install`",
            lock_path.display()
        ));
        return Ok(());
    }

    let lock = crate::core::lock::LockFile::from_file_or_default(&lock_path);
    let mut missing = 0usize;
    let mut tracked_pkgs = 0usize;
    for pkg in &lock.packages {
        // Marketplace packages don't track files (the CLI owns its plugin
        // dir). Label them distinctly so the user knows doctor can't verify
        // their on-disk state — claiming "all files present" for a package
        // whose files we never tracked would be misleading. Active
        // cross-checks against `claude plugin list` are deferred (see
        // findings log).
        if pkg.source.as_marketplace().is_some() {
            let driver = pkg
                .cli
                .first()
                .cloned()
                .unwrap_or_else(|| "cli".to_string());
            println!(
                "  - {}: marketplace (managed by {}) — not tracking files",
                pkg.name, driver
            );
            continue;
        }

        tracked_pkgs += 1;
        for file in &pkg.files {
            if !std::path::Path::new(&file.dest).exists() {
                crate::output::warn(&format!("missing: {} (from {})", file.dest, pkg.name));
                missing += 1;
            }
        }
    }

    if tracked_pkgs == 0 {
        // Either the lock is empty or all entries were marketplace — either
        // way there's nothing we can verify on disk. Stay quiet rather than
        // print a green "all present" that doesn't reflect any real check.
        return Ok(());
    }

    if missing == 0 {
        crate::output::success("All installed files present");
    } else {
        println!("\n  Run `agix install` to restore missing files.");
    }
    Ok(())
}
