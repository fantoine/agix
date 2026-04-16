pub async fn run() -> anyhow::Result<()> {
    let path = std::env::current_dir()?.join("Agentfile");
    if !path.exists() {
        anyhow::bail!("No Agentfile found in current directory.");
    }
    let manifest = crate::manifest::agentfile::PackageManifest::from_file(&path)?
        .ok_or_else(|| anyhow::anyhow!("Agentfile is empty"))?;

    if manifest.agix.name.is_none() {
        anyhow::bail!("Missing [agix] name — required for a package manifest.");
    }
    if manifest.agix.version.is_none() {
        anyhow::bail!("Missing [agix] version — required for a package manifest.");
    }
    if manifest.agix.cli.is_empty() {
        anyhow::bail!("Missing [agix] cli — specify at least one target CLI.");
    }

    crate::output::success(&format!(
        "Agentfile valid — {} v{} for {}",
        manifest.agix.name.unwrap(),
        manifest.agix.version.unwrap(),
        manifest.agix.cli.join(", ")
    ));
    Ok(())
}
