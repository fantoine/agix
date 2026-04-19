use crate::manifest::agentfile::Dependency;

pub async fn run(
    source: String,
    scope: &str,
    cli_filter: Vec<String>,
    version: Option<String>,
) -> anyhow::Result<()> {
    let (agentfile_path, lock_path, scope) = super::agentfile_paths(scope)?;
    let mut manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    let spec = crate::sources::SourceSpec::parse(&source)?;
    let name = spec.suggested_name()?;

    let dep = Dependency {
        source: source.clone(),
        version,
        exclude: None,
    };

    if cli_filter.is_empty() {
        manifest.dependencies.insert(name.clone(), dep);
    } else {
        for cli in &cli_filter {
            manifest
                .cli_dependencies
                .entry(cli.clone())
                .or_default()
                .insert(name.clone(), dep.clone());
        }
    }

    manifest.to_file(&agentfile_path)?;
    crate::core::installer::Installer::install_manifest(&manifest, &lock_path, scope).await?;
    crate::output::success(&format!("Added {name}"));
    Ok(())
}
