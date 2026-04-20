use crate::manifest::agentfile::Dependency;

pub async fn run(
    source_type: String,
    source_value: String,
    scope: &str,
    cli_filter: Vec<String>,
    version: Option<String>,
) -> anyhow::Result<()> {
    let valid_source_types = crate::sources::scheme_names();
    if !valid_source_types.contains(&source_type.as_str()) {
        anyhow::bail!(
            "unknown source type '{}' — expected one of: {}",
            source_type,
            valid_source_types.join(", ")
        );
    }
    let source = format!("{}:{}", source_type, source_value);

    let (agentfile_path, lock_path, scope) = super::agentfile_paths(scope)?;
    let mut manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    let src = crate::sources::parse_source(&source)?;
    let name = src.suggested_name()?;

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
