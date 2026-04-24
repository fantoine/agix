pub async fn run() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let path = super::agentfile_path_walk_up_only(&cwd)
        .ok_or_else(|| anyhow::anyhow!("No Agentfile found — run `agix init` first."))?;
    if !path.exists() {
        anyhow::bail!("No Agentfile found — run `agix init` first.");
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

    // Sources are parsed eagerly by `Dependency::deserialize`, so reaching
    // this point already proves every dep's source is well-formed.

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
