pub async fn run() -> anyhow::Result<()> {
    let path = std::env::current_dir()?.join("Agentfile");
    if !path.exists() {
        anyhow::bail!("No Agentfile found in current directory.");
    }

    let manifest = crate::manifest::agentfile::ProjectManifest::from_file(&path)?;

    if manifest.agix.cli.is_empty() {
        anyhow::bail!("Missing [agix] cli — specify at least one target CLI.");
    }

    // Package manifest: name is present → also require version.
    if let Some(name) = &manifest.agix.name {
        if manifest.agix.version.is_none() {
            anyhow::bail!("Missing [agix] version — required when [agix] name is set.");
        }
        let version = manifest.agix.version.as_deref().unwrap();
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
