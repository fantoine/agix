use anyhow::{bail, Result};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::constants::manifest::{AGENTFILE, AGENTFILE_LOCK};
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
/// With `--all`, both local and global scopes are exported into the same zip
/// under `local/` and `global/` prefixes respectively.
///
/// Unzipping the archive and running `agix install` in the extracted directory
/// (or inside `local/` / `global/` for `--all`) must succeed — nothing should
/// point to the source machine's filesystem.
pub async fn run(global: bool, all: bool, output: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let output_path = output.unwrap_or_else(|| "agix-export.zip".to_string());
    let options: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    if all {
        let file = std::fs::File::create(&output_path)?;
        let mut zip = zip::ZipWriter::new(file);

        // Local scope — walk-up only, no auto-create.
        match super::agentfile_path_walk_up_only(&cwd) {
            Some(af) => {
                let lock = af.parent().unwrap_or(&cwd).join(AGENTFILE_LOCK);
                export_scope(&af, &lock, "local", &mut zip, options)?;
            }
            None => crate::output::warn("no local Agentfile found — skipping local scope"),
        }

        // Global scope — read only, no auto-create.
        let (global_af, global_lock) = super::global_paths()?;
        if global_af.exists() {
            export_scope(&global_af, &global_lock, "global", &mut zip, options)?;
        } else {
            crate::output::warn("no global Agentfile found — skipping global scope");
        }

        zip.finish()?;
        crate::output::success(&format!("Exported to {output_path}"));
        return Ok(());
    }

    let (agentfile_path, lock_path, resolved) = super::agentfile_paths(global, &cwd, false)?;
    if let super::ResolvedScope::Project(ref root) = resolved {
        std::env::set_current_dir(root)?;
    }
    crate::output::scope_header(
        &agentfile_path,
        matches!(resolved, super::ResolvedScope::Global),
    );

    if !agentfile_path.exists() {
        bail!(
            "no Agentfile at {} — run `agix init` first",
            agentfile_path.display()
        );
    }

    let file = std::fs::File::create(&output_path)?;
    let mut zip = zip::ZipWriter::new(file);
    export_scope(&agentfile_path, &lock_path, "", &mut zip, options)?;
    zip.finish()?;
    crate::output::success(&format!("Exported to {output_path}"));
    Ok(())
}

/// Write one scope (local or global) into `zip` under `prefix/`.
///
/// `prefix` is `""` for a single-scope export (files at zip root), or
/// `"local"` / `"global"` for a combined `--all` export.
fn export_scope<W: std::io::Write + std::io::Seek>(
    agentfile_path: &Path,
    lock_path: &Path,
    prefix: &str,
    zip: &mut zip::ZipWriter<W>,
    options: zip::write::FileOptions<()>,
) -> Result<()> {
    let zp = |name: &str| -> String {
        if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        }
    };

    let manifest = ProjectManifest::from_file(agentfile_path)?;
    let mut rewritten = manifest.clone();
    let mut local_sources: HashMap<String, PathBuf> = HashMap::new();

    rewrite_local_deps(&mut rewritten.dependencies, &mut local_sources);
    for deps in rewritten.cli_dependencies.values_mut() {
        rewrite_local_deps(deps, &mut local_sources);
    }

    let rewritten_agentfile = rewritten.to_toml_string()?;
    zip.start_file(zp(AGENTFILE), options)?;
    zip.write_all(rewritten_agentfile.as_bytes())?;

    if lock_path.exists() {
        let lock_text = std::fs::read_to_string(lock_path)?;
        let rewritten_lock = rewrite_local_sources_in_lock(&lock_text, &local_sources)?;
        zip.start_file(zp(AGENTFILE_LOCK), options)?;
        zip.write_all(rewritten_lock.as_bytes())?;
    }

    let ls_prefix = zp("local-sources");
    for (name, abs_path) in &local_sources {
        let source_dir = if abs_path.is_absolute() {
            abs_path.clone()
        } else {
            agentfile_path
                .parent()
                .map(|p| p.join(abs_path))
                .unwrap_or_else(|| abs_path.clone())
        };
        if source_dir.exists() {
            copy_dir_into_zip(zip, &source_dir, name, &ls_prefix, options)?;
        } else {
            crate::output::warn(&format!(
                "local source for '{}' not found at {} — skipping",
                name,
                source_dir.display()
            ));
        }
    }

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
            if let Some(files) = pkg_table.get_mut("files") {
                *files = toml::Value::Array(vec![]);
            }
        }
    }

    Ok(toml::to_string_pretty(&value)?)
}

/// Walk `src_dir` and add every file under `<ls_prefix>/<name>/...` inside the
/// zip archive.
fn copy_dir_into_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    src_dir: &Path,
    name: &str,
    ls_prefix: &str,
    options: zip::write::FileOptions<()>,
) -> Result<()> {
    for entry in walkdir::WalkDir::new(src_dir) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(src_dir).unwrap_or(entry.path());
        let rel_str = rel
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join("/");
        let zip_name = format!("{ls_prefix}/{name}/{rel_str}");
        zip.start_file(&zip_name, options)?;
        let content = std::fs::read(entry.path())?;
        zip.write_all(&content)?;
    }
    Ok(())
}
