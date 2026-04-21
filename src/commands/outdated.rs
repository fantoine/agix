use crate::core::lock::LockFile;
use crate::drivers::Scope;
use crate::manifest::agentfile::ProjectManifest;

/// Status of a single package as computed by [`check_outdated`].
///
/// `UpToDate` and `Outdated` cover the remote-resolve happy paths; the rest
/// are rendered to the user as informational rows so nothing is silently
/// dropped from the report.
#[derive(Debug, Clone, PartialEq)]
pub enum OutdatedStatus {
    /// Remote ref resolved to the same SHA as the lock file.
    UpToDate { name: String, sha: String },
    /// Remote ref resolved to a different SHA than the lock file.
    Outdated {
        name: String,
        current_sha: String,
        available_sha: String,
    },
    /// Local source — nothing to check remotely.
    Local { name: String },
    /// Marketplace source — update path is owned by the CLI driver.
    Marketplace { name: String, driver: String },
    /// Git (non-github) source — kept for rendering backwards-compat with
    /// pre-A2 locks. No longer emitted by [`check_outdated`] (git sources now
    /// go through [`crate::sources::git::GitSource::resolve_ref`]).
    GitNotCheckable { name: String },
    /// Lock entry has no SHA to compare against (pre-Phase-A lock, broken
    /// install, etc.).
    UnknownCurrent { name: String },
    /// Remote resolution failed (network error, private repo, etc.). Recorded
    /// rather than aborting the whole report so one flaky dep doesn't mask the
    /// others.
    ResolveFailed { name: String, error: String },
    /// Source string did not parse. Mirrors the `remove`/`update` graceful
    /// fallback for hand-edited locks.
    UnparseableSource { name: String, error: String },
}

pub async fn run(scope: Scope) -> anyhow::Result<()> {
    let (agentfile_path, lock_path, _) = super::agentfile_paths(scope, false)?;

    // Step 5: no Agentfile — exit non-zero with an actionable message.
    // `agentfile_paths` auto-creates the file for `--scope global`, so this
    // check only fires for local scope.
    if !agentfile_path.exists() {
        anyhow::bail!(
            "no Agentfile at {} — run `agix init` first",
            agentfile_path.display()
        );
    }

    // Step 6: no lock file — `outdated` needs a baseline of resolved SHAs to
    // compare against, so this is a hard error pointing at `agix install`.
    // The decision is logged in the Task 15 findings entry.
    if !lock_path.exists() {
        anyhow::bail!(
            "no lock file at {} — run `agix install` first to establish a baseline",
            lock_path.display()
        );
    }

    let manifest = ProjectManifest::from_file(&agentfile_path)?;
    let lock = LockFile::from_file(&lock_path)?;

    if lock.packages.is_empty() {
        crate::output::info("No packages installed.");
        return Ok(());
    }

    let statuses = check_outdated(&manifest, &lock, None).await?;
    render(&statuses);
    Ok(())
}

/// Pure check routine for `outdated`. Factored out of [`run`] so integration
/// tests can stub the GitHub API via mockito: pass `Some(&server.url())` as
/// `github_api_base` to redirect all `github:` ref resolutions.
///
/// `github_api_base = None` uses the real GitHub API (production path).
pub async fn check_outdated(
    manifest: &ProjectManifest,
    lock: &LockFile,
    github_api_base: Option<&str>,
) -> anyhow::Result<Vec<OutdatedStatus>> {
    let mut out = Vec::with_capacity(lock.packages.len());
    for pkg in &lock.packages {
        let requested_ref = dep_version_for(manifest, &pkg.name);
        out.push(resolve_one(pkg, requested_ref.as_deref(), github_api_base).await);
    }
    Ok(out)
}

/// Find the manifest-declared `version` (floating ref / tag / branch) for a
/// package by name, searching shared then per-CLI sections. Returns `None` if
/// the dep isn't declared (pre-Phase-A lock with a dropped manifest entry) or
/// if no explicit version was set — in which case we fall back to the lock's
/// own `version` field, and finally to "default branch" semantics inside
/// `GitHubSource::resolve_ref`.
fn dep_version_for(manifest: &ProjectManifest, name: &str) -> Option<String> {
    if let Some(dep) = manifest.dependencies.get(name) {
        if dep.version.is_some() {
            return dep.version.clone();
        }
    }
    for cli_deps in manifest.cli_dependencies.values() {
        if let Some(dep) = cli_deps.get(name) {
            if dep.version.is_some() {
                return dep.version.clone();
            }
        }
    }
    None
}

async fn resolve_one(
    pkg: &crate::core::lock::LockedPackage,
    manifest_version: Option<&str>,
    github_api_base: Option<&str>,
) -> OutdatedStatus {
    // Fast paths that don't need the Source trait at all. We match on the
    // lock's `source` prefix rather than parsing, so a mangled source still
    // lets us render a useful row rather than aborting the whole report.
    if pkg.source.starts_with("local:") {
        return OutdatedStatus::Local {
            name: pkg.name.clone(),
        };
    }
    if pkg.source.starts_with("marketplace:") {
        // The driver that manages marketplace plugins is the CLI in the
        // package's `cli` list (typically "claude"). Fall back to a generic
        // label if `cli` is empty — e.g. a hand-edited lock.
        let driver = pkg
            .cli
            .first()
            .cloned()
            .unwrap_or_else(|| "cli".to_string());
        return OutdatedStatus::Marketplace {
            name: pkg.name.clone(),
            driver,
        };
    }
    if let Some(payload) = pkg.source.strip_prefix("git:") {
        // Plain git sources: use `GitSource::resolve_ref` (libgit2 ls-remote)
        // to get the target SHA without cloning. Errors become
        // `ResolveFailed` so one unreachable remote doesn't poison the rest
        // of the report.
        let (url, embedded_ref) = match payload.split_once('@') {
            Some((u, r)) => (u, Some(r)),
            None => (payload, None),
        };
        let ref_str = manifest_version.or(embedded_ref);
        let source = crate::sources::git::GitSource::new(url, ref_str);

        let current = match pkg.sha.as_deref() {
            Some(s) => s.to_string(),
            None => {
                return OutdatedStatus::UnknownCurrent {
                    name: pkg.name.clone(),
                };
            }
        };

        return match source.resolve_ref().await {
            Ok(available) if available == current => OutdatedStatus::UpToDate {
                name: pkg.name.clone(),
                sha: current,
            },
            Ok(available) => OutdatedStatus::Outdated {
                name: pkg.name.clone(),
                current_sha: current,
                available_sha: available,
            },
            Err(e) => OutdatedStatus::ResolveFailed {
                name: pkg.name.clone(),
                error: e.to_string(),
            },
        };
    }
    if !pkg.source.starts_with("github:") {
        // Unknown scheme in the lock — label as unparseable for visibility
        // rather than silently skipping.
        return OutdatedStatus::UnparseableSource {
            name: pkg.name.clone(),
            error: format!("unknown source scheme: {}", pkg.source),
        };
    }

    // github: — extract org/repo (and the lock's embedded @ref if present),
    // then prefer the manifest-declared `version` over the lock's embedded ref
    // when resolving. This matches the semantics the user wrote in Agentfile.
    let payload = pkg.source.trim_start_matches("github:");
    let (path, embedded_ref) = match payload.split_once('@') {
        Some((p, r)) => (p, Some(r)),
        None => (payload, None),
    };
    let (org, repo) = match path.split_once('/') {
        Some(x) => x,
        None => {
            return OutdatedStatus::UnparseableSource {
                name: pkg.name.clone(),
                error: format!(
                    "github source must be 'github:org/repo', got: {}",
                    pkg.source
                ),
            };
        }
    };

    // Priority: manifest's `version` (what the user asked for) > lock's
    // embedded @ref (what `add` captured) > None (resolve HEAD of default).
    let ref_str = manifest_version.or(embedded_ref);

    let source = crate::sources::github::GitHubSource::new_with_optional_base(
        org,
        repo,
        ref_str,
        github_api_base,
    );

    let current = match pkg.sha.as_deref() {
        Some(s) => s.to_string(),
        None => {
            return OutdatedStatus::UnknownCurrent {
                name: pkg.name.clone(),
            };
        }
    };

    match source.resolve_ref().await {
        Ok(available) if available == current => OutdatedStatus::UpToDate {
            name: pkg.name.clone(),
            sha: current,
        },
        Ok(available) => OutdatedStatus::Outdated {
            name: pkg.name.clone(),
            current_sha: current,
            available_sha: available,
        },
        Err(e) => OutdatedStatus::ResolveFailed {
            name: pkg.name.clone(),
            error: e.to_string(),
        },
    }
}

/// Truncate to the classic 7-char short SHA. Strings that are shorter pass
/// through unchanged so we don't panic on pre-Phase-A entries with weird data.
fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

fn render(statuses: &[OutdatedStatus]) {
    let mut any_outdated = false;
    for s in statuses {
        match s {
            OutdatedStatus::UpToDate { name, sha } => {
                println!("  {name} @ {} — up to date", short(sha));
            }
            OutdatedStatus::Outdated {
                name,
                current_sha,
                available_sha,
            } => {
                any_outdated = true;
                println!(
                    "  {name} @ {} — update available: {}",
                    short(current_sha),
                    short(available_sha)
                );
            }
            OutdatedStatus::Local { name } => {
                println!("  {name} — local (not checkable)");
            }
            OutdatedStatus::Marketplace { name, driver } => {
                println!("  {name} — marketplace (managed by {driver})");
            }
            OutdatedStatus::GitNotCheckable { name } => {
                println!("  {name} — git (not checkable)");
            }
            OutdatedStatus::UnknownCurrent { name } => {
                println!("  {name} — no locked SHA to compare against");
            }
            OutdatedStatus::ResolveFailed { name, error } => {
                crate::output::warn(&format!("{name}: could not resolve remote ref: {error}"));
            }
            OutdatedStatus::UnparseableSource { name, error } => {
                crate::output::warn(&format!("{name}: unparseable lock source: {error}"));
            }
        }
    }
    if !any_outdated {
        crate::output::info("All dependencies are up to date.");
    }
}
