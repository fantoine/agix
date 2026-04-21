//! Canonical names for Agentfile / Agentfile.lock entries.
//!
//! Serde `#[serde(rename = "…")]` attributes require string literals and cannot
//! reference these constants. The contract is: any manual parsing via
//! `toml::Value` (inside `ProjectManifest`/`PackageManifest` custom `Deserialize`
//! impls, or inside `to_toml_string`) must go through these constants to avoid
//! drift against the serde-driven surface.

// --- File names ---------------------------------------------------------------

pub const AGENTFILE: &str = "Agentfile";
pub const AGENTFILE_LOCK: &str = "Agentfile.lock";

// --- Top-level Agentfile sections --------------------------------------------

pub const KEY_AGIX: &str = "agix";
pub const KEY_DEPENDENCIES: &str = "dependencies";
pub const KEY_HOOKS: &str = "hooks";

/// Top-level keys that belong to fixed Agentfile sections — anything else under
/// the document root is treated as a per-CLI section.
pub const RESERVED_TOP_LEVEL_KEYS: &[&str] = &[KEY_AGIX, KEY_DEPENDENCIES, KEY_HOOKS];

// --- Dependency table fields --------------------------------------------------

pub const KEY_SOURCE: &str = "source";
pub const KEY_VERSION: &str = "version";
pub const KEY_EXCLUDE: &str = "exclude";
