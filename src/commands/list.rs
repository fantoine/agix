use std::collections::BTreeMap;

use crate::core::lock::{LockFile, LockedPackage};
use crate::drivers::Scope;
use crate::manifest::agentfile::{Dependency, ProjectManifest};

pub async fn run(scope: Scope) -> anyhow::Result<()> {
    let (agentfile_path, lock_path, _) = super::agentfile_paths(scope, false)?;

    // Step 6: no Agentfile — exit non-zero with an actionable message.
    // `agentfile_paths` auto-creates the file for `--scope global` (first-time
    // setup), so this check only fires for local scope.
    if !agentfile_path.exists() {
        anyhow::bail!(
            "no Agentfile at {} — run `agix init` first",
            agentfile_path.display()
        );
    }

    let manifest = ProjectManifest::from_file(&agentfile_path)?;
    // Step 5: missing lock is not an error — `list` is read-only and should
    // still show declared deps so users can see what's in the manifest. Deps
    // that aren't in the lock are rendered with a `(not installed)` marker.
    let lock = LockFile::from_file_or_default(&lock_path);

    // Step 2: empty manifest (and thus empty lock) — friendly empty-state.
    if manifest.dependencies.is_empty() && manifest.cli_dependencies.is_empty() {
        crate::output::info("No dependencies declared.");
        return Ok(());
    }

    // Step 3 / 4: group by shared vs per-CLI section for a clear visual split.
    // Deterministic ordering on both groups and dep names.
    let shared: BTreeMap<&String, &Dependency> = manifest.dependencies.iter().collect();
    if !shared.is_empty() {
        println!("Shared:");
        for (name, dep) in &shared {
            print_dep(name, dep, &lock);
        }
    }

    let cli_deps: BTreeMap<&String, &std::collections::HashMap<String, Dependency>> =
        manifest.cli_dependencies.iter().collect();
    for (cli, deps) in &cli_deps {
        if deps.is_empty() {
            continue;
        }
        if !shared.is_empty() {
            println!();
        }
        println!("[{cli}]:");
        let sorted: BTreeMap<&String, &Dependency> = deps.iter().collect();
        for (name, dep) in &sorted {
            print_dep(name, dep, &lock);
        }
    }

    Ok(())
}

/// Render one dependency line. Prefers the locked version (short sha or
/// `content_hash` prefix) when available; falls back to the manifest-declared
/// `version` string; otherwise `(not installed)`.
fn print_dep(name: &str, dep: &Dependency, lock: &LockFile) {
    let locked = lock.find(name);
    let version = render_version(dep, locked);
    let clis = render_clis(locked);
    println!("  {name} @ {version} — {source}{clis}", source = dep.source);
}

fn render_version(dep: &Dependency, locked: Option<&LockedPackage>) -> String {
    if let Some(pkg) = locked {
        if let Some(sha) = pkg.sha.as_deref() {
            return sha[..sha.len().min(7)].to_string();
        }
        if let Some(ver) = pkg.version.as_deref() {
            return ver.to_string();
        }
        if let Some(hash) = pkg.content_hash.as_deref() {
            return format!("local:{}", &hash[..hash.len().min(7)]);
        }
        return "local".to_string();
    }
    // Not in lock — show manifest-declared version if any, else flag it.
    match &dep.version {
        Some(v) => format!("{v} (not installed)"),
        None => "(not installed)".to_string(),
    }
}

fn render_clis(locked: Option<&LockedPackage>) -> String {
    match locked {
        Some(pkg) if !pkg.cli.is_empty() => format!(" ({})", pkg.cli.join(", ")),
        _ => String::new(),
    }
}
