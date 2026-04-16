// Error strategy:
// - AgixError (this file): typed errors in library code (core/, sources/, drivers/, manifest/)
// - anyhow::Result: used at the CLI surface (commands/*.rs and main.rs) for human-readable context

pub type Result<T> = std::result::Result<T, AgixError>;

#[derive(Debug, thiserror::Error)]
pub enum AgixError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("Version conflict: package '{name}' required at '{a}' and '{b}'")]
    VersionConflict { name: String, a: String, b: String },
    #[error("Package not found: {0}")]
    PackageNotFound(String),
    #[error("Invalid source spec: {0}")]
    InvalidSource(String),
    #[error("{0}")]
    Other(String),
}
