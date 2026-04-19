use std::collections::HashMap;
use std::path::Path;

use serde::{de, Deserialize, Deserializer, Serialize};

// ---------------------------------------------------------------------------
// AgixSection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgixSection {
    /// CLI tools this project / package targets (e.g. ["claude-code", "codex"])
    #[serde(default)]
    pub cli: Vec<String>,

    /// Package name (only meaningful in a package manifest)
    pub name: Option<String>,

    /// Semver string (only meaningful in a package manifest)
    pub version: Option<String>,

    /// Human-readable description (only meaningful in a package manifest)
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Dependency  (supports both string and table forms)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Dependency {
    pub source: String,
    pub version: Option<String>,
    pub exclude: Option<Vec<String>>,
}

impl<'de> Deserialize<'de> for Dependency {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct DependencyVisitor;

        impl<'de> de::Visitor<'de> for DependencyVisitor {
            type Value = Dependency;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "a string source or a table with a `source` key")
            }

            // String form: dep = "github:org/repo"
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Dependency {
                    source: v.to_string(),
                    version: None,
                    exclude: None,
                })
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(Dependency {
                    source: v,
                    version: None,
                    exclude: None,
                })
            }

            // Table form: dep = { source = "...", version = "...", exclude = [...] }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut source: Option<String> = None;
                let mut version: Option<String> = None;
                let mut exclude: Option<Vec<String>> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "source" => source = Some(map.next_value()?),
                        "version" => version = Some(map.next_value()?),
                        "exclude" => exclude = Some(map.next_value()?),
                        other => {
                            return Err(de::Error::unknown_field(
                                other,
                                &["source", "version", "exclude"],
                            ))
                        }
                    }
                }

                let source = source.ok_or_else(|| de::Error::missing_field("source"))?;
                Ok(Dependency {
                    source,
                    version,
                    exclude,
                })
            }
        }

        deserializer.deserialize_any(DependencyVisitor)
    }
}

// ---------------------------------------------------------------------------
// Hooks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hooks {
    #[serde(rename = "post-install")]
    pub post_install: Option<String>,

    #[serde(rename = "pre-uninstall")]
    pub pre_uninstall: Option<String>,
}

// ---------------------------------------------------------------------------
// CliSection  (the value under a per-CLI key like [claude-code])
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CliSection {
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
}

// ---------------------------------------------------------------------------
// ProjectManifest  (Agentfile at the root of a project)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProjectManifest {
    pub agix: AgixSection,

    /// Top-level dependencies shared across all CLIs
    pub dependencies: HashMap<String, Dependency>,

    /// Per-CLI dependency sections, e.g. `[claude-code.dependencies]`
    /// Keys are the CLI names declared in `agix.cli`.
    pub cli_dependencies: HashMap<String, HashMap<String, Dependency>>,
}

impl<'de> Deserialize<'de> for ProjectManifest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Deserialise the whole document into a raw Value first.
        let raw = toml::Value::deserialize(deserializer)?;
        let table = raw
            .as_table()
            .ok_or_else(|| de::Error::custom("expected a TOML table at the document root"))?;

        // --- [agix] section ---
        let agix: AgixSection = table
            .get("agix")
            .ok_or_else(|| de::Error::missing_field("agix"))?
            .clone()
            .try_into()
            .map_err(de::Error::custom)?;

        // --- [dependencies] section (optional) ---
        let dependencies: HashMap<String, Dependency> = match table.get("dependencies") {
            Some(v) => v.clone().try_into().map_err(de::Error::custom)?,
            None => HashMap::new(),
        };

        // --- per-CLI sections (everything that is a table AND has a `dependencies` sub-key) ---
        // We collect all top-level keys that look like CLI sections.
        let reserved = ["agix", "dependencies", "hooks"];
        let mut cli_dependencies: HashMap<String, HashMap<String, Dependency>> = HashMap::new();

        for (key, value) in table {
            if reserved.contains(&key.as_str()) {
                continue;
            }
            if let Some(sub_table) = value.as_table() {
                if let Some(deps_value) = sub_table.get("dependencies") {
                    let deps: HashMap<String, Dependency> =
                        deps_value.clone().try_into().map_err(de::Error::custom)?;
                    cli_dependencies.insert(key.clone(), deps);
                }
            }
        }

        Ok(ProjectManifest {
            agix,
            dependencies,
            cli_dependencies,
        })
    }
}

impl ProjectManifest {
    /// Read an Agentfile from disk.
    pub fn from_file(path: &Path) -> crate::error::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let manifest = toml::from_str(&text)?;
        Ok(manifest)
    }

    /// Serialise the manifest to its canonical Agentfile TOML representation.
    ///
    /// Produces the canonical Agentfile format:
    ///   [agix]
    ///   [dependencies]
    ///   [claude-code.dependencies]   ← per-CLI sections
    ///
    /// This is the string that [`ProjectManifest::to_file`] writes to disk, and
    /// is also the shape that the custom [`Deserialize`] impl expects when
    /// reading the file back — so the output is guaranteed to roundtrip.
    pub fn to_toml_string(&self) -> crate::error::Result<String> {
        use toml::Value;

        let mut root = toml::map::Map::new();

        // [agix]
        root.insert("agix".to_string(), Value::try_from(&self.agix)?);

        // [dependencies]
        if !self.dependencies.is_empty() {
            root.insert(
                "dependencies".to_string(),
                Value::try_from(&self.dependencies)?,
            );
        }

        // [<cli>.dependencies]
        for (cli, deps) in &self.cli_dependencies {
            if deps.is_empty() {
                continue;
            }
            let mut cli_table = toml::map::Map::new();
            cli_table.insert("dependencies".to_string(), Value::try_from(deps)?);
            root.insert(cli.clone(), Value::Table(cli_table));
        }

        Ok(toml::to_string_pretty(&Value::Table(root))?)
    }

    /// Serialise and write the manifest back to disk using the canonical
    /// Agentfile format (see [`ProjectManifest::to_toml_string`]).
    pub fn to_file(&self, path: &Path) -> crate::error::Result<()> {
        let text = self.to_toml_string()?;
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Create a minimal empty manifest for a new project.
    pub fn empty(cli: Vec<String>) -> Self {
        ProjectManifest {
            agix: AgixSection {
                cli,
                name: None,
                version: None,
                description: None,
            },
            dependencies: HashMap::new(),
            cli_dependencies: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// PackageManifest  (Agentfile at the root of a package / plugin)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PackageManifest {
    pub agix: AgixSection,
    pub hooks: Option<Hooks>,
    pub dependencies: HashMap<String, Dependency>,
    pub cli_dependencies: HashMap<String, HashMap<String, Dependency>>,
}

impl<'de> Deserialize<'de> for PackageManifest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = toml::Value::deserialize(deserializer)?;
        let table = raw
            .as_table()
            .ok_or_else(|| de::Error::custom("expected a TOML table at the document root"))?;

        let agix: AgixSection = table
            .get("agix")
            .ok_or_else(|| de::Error::missing_field("agix"))?
            .clone()
            .try_into()
            .map_err(de::Error::custom)?;

        let hooks: Option<Hooks> = match table.get("hooks") {
            Some(v) => Some(v.clone().try_into().map_err(de::Error::custom)?),
            None => None,
        };

        let dependencies: HashMap<String, Dependency> = match table.get("dependencies") {
            Some(v) => v.clone().try_into().map_err(de::Error::custom)?,
            None => HashMap::new(),
        };

        let reserved = ["agix", "dependencies", "hooks"];
        let mut cli_dependencies: HashMap<String, HashMap<String, Dependency>> = HashMap::new();

        for (key, value) in table {
            if reserved.contains(&key.as_str()) {
                continue;
            }
            if let Some(sub_table) = value.as_table() {
                if let Some(deps_value) = sub_table.get("dependencies") {
                    let deps: HashMap<String, Dependency> =
                        deps_value.clone().try_into().map_err(de::Error::custom)?;
                    cli_dependencies.insert(key.clone(), deps);
                }
            }
        }

        Ok(PackageManifest {
            agix,
            hooks,
            dependencies,
            cli_dependencies,
        })
    }
}

impl PackageManifest {
    /// Read a package manifest from disk, returning `None` if the file does not exist.
    pub fn from_file(path: &Path) -> crate::error::Result<Option<Self>> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let manifest = toml::from_str(&text)?;
                Ok(Some(manifest))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
