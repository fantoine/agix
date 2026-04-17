use crate::manifest::agentfile::ProjectManifest;

// ---------------------------------------------------------------------------
// ResolvedDep
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ResolvedDep {
    pub name: String,
    pub source: String,
    pub version: Option<String>,
    /// CLIs for which this package must be installed.
    pub cli: Vec<String>,
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

pub struct Resolver;

impl Resolver {
    /// Resolve all dependencies from `manifest` for the given `active_clis`.
    ///
    /// Resolution rules:
    /// 1. Shared deps (`manifest.dependencies`): target CLIs = `active_clis` minus
    ///    those listed in `dep.exclude`. Skip if the resulting list is empty.
    /// 2. CLI-specific deps (`manifest.cli_dependencies`): for each active CLI,
    ///    for each dep in its section, merge into an existing `ResolvedDep` with
    ///    the same name (adding the CLI without duplicates) or create a new entry.
    pub fn resolve(manifest: &ProjectManifest, active_clis: &[String]) -> Vec<ResolvedDep> {
        // Use an ordered vec + a name→index map to preserve a stable ordering.
        let mut resolved: Vec<ResolvedDep> = Vec::new();
        let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        // ------------------------------------------------------------------
        // Step 1 – shared dependencies
        // ------------------------------------------------------------------
        for (name, dep) in &manifest.dependencies {
            let excluded: Vec<&str> = dep
                .exclude
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|s| s.as_str())
                .collect();

            let target_clis: Vec<String> = active_clis
                .iter()
                .filter(|cli| !excluded.contains(&cli.as_str()))
                .cloned()
                .collect();

            let idx = resolved.len();
            resolved.push(ResolvedDep {
                name: name.clone(),
                source: dep.source.clone(),
                version: dep.version.clone(),
                cli: target_clis,
            });
            index.insert(name.clone(), idx);
        }

        // ------------------------------------------------------------------
        // Step 2 – CLI-specific dependencies
        // ------------------------------------------------------------------
        for cli in active_clis {
            let cli_deps = match manifest.cli_dependencies.get(cli) {
                Some(deps) => deps,
                None => continue,
            };

            for (name, dep) in cli_deps {
                if let Some(&idx) = index.get(name) {
                    // Merge: add this CLI to the existing entry if not already present.
                    if !resolved[idx].cli.contains(cli) {
                        resolved[idx].cli.push(cli.clone());
                    }
                } else {
                    // New entry.
                    let idx = resolved.len();
                    resolved.push(ResolvedDep {
                        name: name.clone(),
                        source: dep.source.clone(),
                        version: dep.version.clone(),
                        cli: vec![cli.clone()],
                    });
                    index.insert(name.clone(), idx);
                }
            }
        }

        resolved
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manifest(toml: &str) -> crate::manifest::agentfile::ProjectManifest {
        toml::from_str(toml).unwrap()
    }

    #[test]
    fn resolves_cli_specific_deps() {
        let manifest = make_manifest(
            r#"
[agix]
cli = ["claude-code"]

[claude-code.dependencies]
superpowers = { source = "github:org/superpowers" }
"#,
        );
        let resolved = Resolver::resolve(&manifest, &["claude-code".to_string()]);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "superpowers");
        assert_eq!(resolved[0].cli, vec!["claude-code".to_string()]);
    }

    #[test]
    fn excludes_filtered_cli() {
        let manifest = make_manifest(
            r#"
[agix]
cli = ["claude-code", "codex"]

[dependencies]
rtk = { source = "github:org/rtk", exclude = ["codex"] }
"#,
        );
        let resolved =
            Resolver::resolve(&manifest, &["claude-code".to_string(), "codex".to_string()]);
        let rtk = resolved.iter().find(|r| r.name == "rtk").unwrap();
        assert!(rtk.cli.contains(&"claude-code".to_string()));
        assert!(!rtk.cli.contains(&"codex".to_string()));
    }

    #[test]
    fn shared_dep_goes_to_all_active_clis() {
        let manifest = make_manifest(
            r#"
[agix]
cli = ["claude-code", "codex"]

[dependencies]
shared = { source = "github:org/shared" }
"#,
        );
        let resolved =
            Resolver::resolve(&manifest, &["claude-code".to_string(), "codex".to_string()]);
        let shared = resolved.iter().find(|r| r.name == "shared").unwrap();
        assert!(shared.cli.contains(&"claude-code".to_string()));
        assert!(shared.cli.contains(&"codex".to_string()));
    }
}
