use anyhow::{bail, Result};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::constants::manifest::{AGENTFILE, AGENTFILE_LOCK};
use crate::drivers::Scope;
use crate::manifest::agentfile::{Dependency, ProjectManifest};
use crate::sources::SourceBox;

/// Export the current project into a self-contained zip archive.
///
/// The resulting zip contains:
///   - A rewritten `Agentfile` where every `local:<abs-path>` dependency is
///     rewritten to `local:./local-sources/<name>`.
///   - A rewritten `Agentfile.lock` with the same source rewrites applied.
///   - A `local-sources/<name>/` tree per local dependency containing the
///     vendored files.
///
/// Unzipping the archive and running `agix install` in the extracted directory
/// must succeed — nothing should point to the source machine's filesystem.
pub async fn run(scope: Scope, all: bool, output: Option<String>) -> Result<()> {
    if all {
        // Deferred for v0.1.0: combining local + global scopes into a single
        // zip needs a cross-scope layout (e.g. `local/` + `global/` subtrees)
        // and a matching re-install flow. See Task 17 findings log.
        bail!("--all is not yet implemented — export one scope at a time");
    }

    let (agentfile_path, lock_path, _scope) = super::agentfile_paths_no_autoinit(scope)?;
    if !agentfile_path.exists() {
        bail!(
            "no Agentfile at {} — run `agix init` first",
            agentfile_path.display()
        );
    }

    let output_path = output.unwrap_or_else(|| "agix-export.zip".to_string());

    // Parse the manifest, then produce a rewritten clone whose local sources
    // are relocated to `local:./local-sources/<name>`.
    let manifest = ProjectManifest::from_file(&agentfile_path)?;
    let mut rewritten = manifest.clone();

    // Map of package-name -> original absolute source path, so we can locate
    // the files on disk and vendor them into the zip. We don't rely on the
    // lock alone because top-level deps may not all be installed yet.
    let mut local_sources: HashMap<String, PathBuf> = HashMap::new();

    rewrite_local_deps(&mut rewritten.dependencies, &mut local_sources);
    for deps in rewritten.cli_dependencies.values_mut() {
        rewrite_local_deps(deps, &mut local_sources);
    }

    // Open the zip.
    let file = std::fs::File::create(&output_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Rewritten Agentfile.
    let rewritten_agentfile = rewritten.to_toml_string()?;
    zip.start_file(AGENTFILE, options)?;
    zip.write_all(rewritten_agentfile.as_bytes())?;

    // Rewritten lock (if one exists).
    if lock_path.exists() {
        let lock_text = std::fs::read_to_string(&lock_path)?;
        let rewritten_lock = rewrite_local_sources_in_lock(&lock_text, &local_sources)?;
        zip.start_file(AGENTFILE_LOCK, options)?;
        zip.write_all(rewritten_lock.as_bytes())?;
    }

    // Vendor each local source under local-sources/<name>/.
    for (name, abs_path) in &local_sources {
        let source_dir = if abs_path.is_absolute() {
            abs_path.clone()
        } else {
            // Best effort: resolve relative to the project dir.
            agentfile_path
                .parent()
                .map(|p| p.join(abs_path))
                .unwrap_or_else(|| abs_path.clone())
        };
        if source_dir.exists() {
            copy_dir_into_zip(&mut zip, &source_dir, name, options)?;
        } else {
            crate::output::warn(&format!(
                "local source for '{}' not found at {} — skipping",
                name,
                source_dir.display()
            ));
        }
    }

    zip.finish()?;
    crate::output::success(&format!("Exported to {output_path}"));
    Ok(())
}

/// Mutate a `HashMap<String, Dependency>` in place: any dependency whose source
/// parses as `local:` is rewritten to `local:./local-sources/<name>`, and its
/// original path is recorded in `local_sources`.
fn rewrite_local_deps(
    deps: &mut HashMap<String, Dependency>,
    local_sources: &mut HashMap<String, PathBuf>,
) {
    for (name, dep) in deps.iter_mut() {
        if let Some(path) = dep.source.local_path() {
            local_sources.insert(name.clone(), path.to_path_buf());
            // Unwrap: `local:./…` is a syntactically valid local source, so
            // parsing cannot fail here.
            dep.source = SourceBox::parse(&format!("local:./local-sources/{name}"))
                .expect("hard-coded local: source is always parseable");
        }
    }
}

/// Rewrite every `source = "local:..."` entry in the lock file to point at
/// `local:./local-sources/<name>`, matching the rewritten manifest.
fn rewrite_local_sources_in_lock(
    lock_text: &str,
    local_sources: &HashMap<String, PathBuf>,
) -> Result<String> {
    if lock_text.trim().is_empty() {
        return Ok(String::new());
    }

    let mut value: toml::Value = toml::from_str(lock_text)?;

    if let Some(packages) = value
        .as_table_mut()
        .and_then(|t| t.get_mut("package"))
        .and_then(|v| v.as_array_mut())
    {
        for pkg in packages.iter_mut() {
            let Some(pkg_table) = pkg.as_table_mut() else {
                continue;
            };
            let Some(name) = pkg_table.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            if !local_sources.contains_key(name) {
                continue;
            }
            let new_source = format!("local:./local-sources/{name}");
            pkg_table.insert("source".to_string(), toml::Value::String(new_source));
            // Clear files: they recorded absolute destinations on the source
            // machine. The target `install` will repopulate them.
            if let Some(files) = pkg_table.get_mut("files") {
                *files = toml::Value::Array(vec![]);
            }
        }
    }

    Ok(toml::to_string_pretty(&value)?)
}

/// Walk `src_dir` and add every file under `local-sources/<name>/...` inside the
/// zip archive. Directories don't need explicit entries — extraction tools
/// recreate them from the file paths.
fn copy_dir_into_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    src_dir: &Path,
    name: &str,
    options: zip::write::FileOptions<()>,
) -> Result<()> {
    for entry in walkdir::WalkDir::new(src_dir) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(src_dir).unwrap_or(entry.path());
        // Force forward slashes inside the archive for portability.
        let rel_str = rel
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join("/");
        let zip_name = format!("local-sources/{name}/{rel_str}");
        zip.start_file(&zip_name, options)?;
        let content = std::fs::read(entry.path())?;
        zip.write_all(&content)?;
    }
    Ok(())
}
