use crate::manifest::agentfile::Dependency;

pub async fn run(
    source: String,
    global: bool,
    cli_filter: Option<String>,
    version: Option<String>,
) -> anyhow::Result<()> {
    let (agentfile_path, lock_path, scope) = super::agentfile_paths(global)?;
    let mut manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    let name = source
        .trim_end_matches('/')
        .rsplit(&['/', ':'][..])
        .next()
        .unwrap_or(&source)
        .split('@')
        .next()
        .unwrap_or(&source)
        .to_owned();

    let dep = Dependency {
        source: source.clone(),
        version,
        exclude: None,
    };

    if let Some(ref cli) = cli_filter {
        manifest
            .cli_dependencies
            .entry(cli.clone())
            .or_default()
            .insert(name.clone(), dep);
    } else {
        manifest.dependencies.insert(name.clone(), dep);
    }

    manifest.to_file(&agentfile_path)?;
    crate::core::installer::Installer::install_manifest(&manifest, &lock_path, scope).await?;
    crate::output::success(&format!("Added {name}"));
    Ok(())
}
