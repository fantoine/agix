use crate::manifest::agentfile::Dependency;

const VALID_SOURCE_TYPES: &[&str] = &["local", "github", "git", "marketplace"];

pub async fn run(
    source_type: String,
    source_value: String,
    scope: &str,
    cli_filter: Vec<String>,
    version: Option<String>,
) -> anyhow::Result<()> {
    if !VALID_SOURCE_TYPES.contains(&source_type.as_str()) {
        anyhow::bail!(
            "unknown source type '{}' — expected one of: {}",
            source_type,
            VALID_SOURCE_TYPES.join(", ")
        );
    }
    let source = format!("{}:{}", source_type, source_value);

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
