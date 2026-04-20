pub async fn run() -> anyhow::Result<()> {
    let path = std::env::current_dir()?.join("Agentfile");
    if !path.exists() {
        anyhow::bail!("No Agentfile found in current directory.");
    }

    let manifest = crate::manifest::agentfile::ProjectManifest::from_file(&path)?;

    if manifest.agix.cli.is_empty() {
        anyhow::bail!("Missing [agix] cli — specify at least one target CLI.");
    }

    // Warn (not error) for CLIs we don't have a driver for: the user may be
    // preparing a manifest for a CLI they'll install later. `check` validates
    // manifest structure, not installed-toolchain state.
    let known: Vec<String> = crate::drivers::all_drivers()
        .iter()
        .map(|d| d.name().to_string())
        .collect();
    for cli in &manifest.agix.cli {
        if crate::drivers::driver_for(cli).is_none() {
            crate::output::warn(&format!(
                "Unknown CLI '{}' in [agix] cli — no driver registered (known: {})",
                cli,
                known.join(", ")
            ));
        }
    }

    // Validate that every declared dependency source parses. Any parse error
    // must mention the dep name so the user can find it in the Agentfile.
    for (name, dep) in &manifest.dependencies {
        if let Err(e) = crate::sources::parse_source(&dep.source) {
            anyhow::bail!("Invalid source for dependency '{}': {}", name, e);
        }
    }
    for (cli, deps) in &manifest.cli_dependencies {
        for (name, dep) in deps {
            if let Err(e) = crate::sources::parse_source(&dep.source) {
                anyhow::bail!(
                    "Invalid source for dependency '{}' under [{}]: {}",
                    name,
                    cli,
                    e
                );
            }
        }
    }

    // Package manifest: name is present → also require version.
    if let Some(name) = &manifest.agix.name {
        let Some(version) = manifest.agix.version.as_deref() else {
            anyhow::bail!("Missing [agix] version — required when [agix] name is set.");
        };
        crate::output::success(&format!(
            "Agentfile valid — package {} v{} for {}",
            name,
            version,
            manifest.agix.cli.join(", ")
        ));
    } else {
        // Project manifest: name is absent → valid as-is.
        crate::output::success(&format!(
            "Agentfile valid — project for {}",
            manifest.agix.cli.join(", ")
        ));
    }

    Ok(())
}
