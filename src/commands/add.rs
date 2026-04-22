use crate::drivers::Scope;
use crate::manifest::agentfile::Dependency;

pub async fn run(
    source_type: String,
    source_value: String,
    scope: Scope,
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

    // Reject unknown CLI names in --cli. Consistent with Task 9's decision for
    // `check` (unknown CLI in [agix].cli is a warning), but stricter here:
    // `add` is writing to the manifest, so a typo would silently persist a
    // bogus `[<typo>.dependencies]` section that no driver would ever act on.
    if !cli_filter.is_empty() {
        let known_names: Vec<String> = crate::drivers::all_drivers()
            .iter()
            .map(|d| d.name().to_string())
            .collect();
        for cli in &cli_filter {
            if !known_names.iter().any(|k| k == cli) {
                anyhow::bail!(
                    "unknown CLI '{}' — expected one of: {}",
                    cli,
                    known_names.join(", ")
                );
            }
        }
    }

    let source = format!("{}:{}", source_type, source_value);

    let (agentfile_path, lock_path, scope) = super::agentfile_paths(scope, false)?;

    // Actionable error if the Agentfile is missing (local scope). The auto-init
    // path in `agentfile_paths` covers global scope only.
    if !agentfile_path.exists() {
        anyhow::bail!(
            "No Agentfile at {}. Run `agix init` first.",
            agentfile_path.display()
        );
    }

    let mut manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    let src = crate::sources::parse_source(&source)?;
    let name = src.suggested_name()?;

    let dep = Dependency {
        source: crate::sources::SourceBox::from(src),
        version,
        exclude: None,
    };

    if cli_filter.is_empty() {
        if manifest.dependencies.contains_key(&name) {
            crate::output::warn(&format!(
                "dependency '{name}' already in [dependencies] — overwriting"
            ));
        }
        manifest.dependencies.insert(name.clone(), dep.clone());
    } else {
        for cli in &cli_filter {
            let entry = manifest.cli_dependencies.entry(cli.clone()).or_default();
            if entry.contains_key(&name) {
                crate::output::warn(&format!(
                    "dependency '{name}' already in [{cli}.dependencies] — overwriting"
                ));
            }
            entry.insert(name.clone(), dep.clone());
        }
    }

    manifest.to_file(&agentfile_path)?;

    // Only install the freshly-added dep. Running the whole manifest here
    // would surprise users by installing every sibling dep declared in the
    // Agentfile — that's the job of `agix install`. The scoped manifest
    // preserves `agix.cli` so the resolver still targets the correct drivers;
    // the lock is merged via `upsert`, so existing lock entries are untouched.
    let scoped_manifest = manifest.single_dep_scoped(&name, dep, &cli_filter);
    crate::core::installer::Installer::install_manifest(&scoped_manifest, &lock_path, &scope)
        .await?;
    crate::output::success(&format!("Added {name}"));
    Ok(())
}
