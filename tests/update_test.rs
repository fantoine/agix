use agix::core::lock::LockFile;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

mod helpers;

// ---------------------------------------------------------------------------
// Existing smoke coverage (kept from earlier versions)
// ---------------------------------------------------------------------------

#[test]
fn update_fails_without_agentfile() {
    let dir = tempdir().unwrap();
    let home = tempdir().unwrap();

    helpers::cmd_non_interactive(home.path())
        .args(["update"])
        .current_dir(&dir)
        .assert()
        .failure();
}

#[test]
fn update_succeeds_with_empty_manifest() {
    let dir = tempdir().unwrap();
    let home = tempdir().unwrap();
    std::fs::write(
        dir.path().join("Agentfile"),
        r#"
[agix]
cli = ["claude"]
"#,
    )
    .unwrap();

    helpers::cmd_non_interactive(home.path())
        .args(["update"])
        .current_dir(&dir)
        .assert()
        .success()
        .stdout(predicates::str::contains("Updated"));
}

#[test]
fn update_single_package_fails_without_agentfile() {
    let dir = tempdir().unwrap();
    let home = tempdir().unwrap();

    helpers::cmd_non_interactive(home.path())
        .args(["update", "claude-later"])
        .current_dir(&dir)
        .assert()
        .failure();
}

fn write_agentfile(cwd: &Path, content: &str) {
    fs::write(cwd.join("Agentfile"), content).unwrap();
}

/// Build a local package with a skills/ subdir.
fn build_local_pkg(marker: &str) -> TempDir {
    let pkg = tempdir().unwrap();
    fs::create_dir(pkg.path().join("skills")).unwrap();
    fs::write(
        pkg.path().join("skills").join("s.md"),
        format!("# skill {marker}"),
    )
    .unwrap();
    pkg
}

/// Claude shim that logs every invocation and mimics the marketplace/plugin
/// JSON list commands via a state dir colocated with the shim.
fn write_claude_shim(dir: &Path) -> PathBuf {
    let log = dir.join("claude-invocations.log");
    let state = dir.join("state");
    helpers::install_claude_shim(dir, &log, &state);
    log
}

fn path_with(bin_dir: &Path) -> String {
    format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

// ---------------------------------------------------------------------------
// Step 2 — Update all: all remote (local here, for hermetic testing) deps re-resolved
// ---------------------------------------------------------------------------
//
// After install, mutate the local source of TWO deps; `agix update` with no
// name must re-resolve and re-copy both, updating both content_hashes.

#[test]
fn step2_update_all_reresolves_every_dep() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg_a = build_local_pkg("a-v1");
    let pkg_b = build_local_pkg("b-v1");

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
dep-a = {{ source = "local:{}" }}
dep-b = {{ source = "local:{}" }}
"#,
            pkg_a.path().display(),
            pkg_b.path().display()
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();

    let lock1 = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    let hash_a1 = lock1
        .packages
        .iter()
        .find(|p| p.name == "dep-a")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();
    let hash_b1 = lock1
        .packages
        .iter()
        .find(|p| p.name == "dep-b")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();

    // Mutate both sources.
    fs::write(pkg_a.path().join("skills").join("s.md"), "# skill a-v2").unwrap();
    fs::write(pkg_b.path().join("skills").join("s.md"), "# skill b-v2").unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("update")
        .assert()
        .success()
        .stdout(predicates::str::contains("Updated"));

    let lock2 = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    let hash_a2 = lock2
        .packages
        .iter()
        .find(|p| p.name == "dep-a")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();
    let hash_b2 = lock2
        .packages
        .iter()
        .find(|p| p.name == "dep-b")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();

    assert_ne!(hash_a1, hash_a2, "dep-a content_hash must change");
    assert_ne!(hash_b1, hash_b2, "dep-b content_hash must change");

    // And the installed files reflect the v2 content.
    let content_a =
        fs::read_to_string(cwd.path().join(".claude").join("skills").join("s.md")).unwrap();
    // Both deps land their `skills/s.md` at the same spot (.claude/skills/s.md);
    // the second dep wins lexically or by insertion order. Just ensure "v2" is
    // reflected somehow — in practice the last-installed dep wins the file.
    assert!(
        content_a.contains("v2"),
        "expected installed skill to show v2 content after update; got: {content_a}"
    );
}

// ---------------------------------------------------------------------------
// Step 3 — Update specific package: named dep re-resolved, others untouched
// ---------------------------------------------------------------------------

#[test]
fn step3_update_specific_package_refreshes_only_that_dep() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg_a = build_local_pkg("a-v1");
    let pkg_b = build_local_pkg("b-v1");

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
dep-a = {{ source = "local:{}" }}
dep-b = {{ source = "local:{}" }}
"#,
            pkg_a.path().display(),
            pkg_b.path().display()
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();

    let lock1 = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    let hash_a1 = lock1
        .packages
        .iter()
        .find(|p| p.name == "dep-a")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();
    let hash_b1 = lock1
        .packages
        .iter()
        .find(|p| p.name == "dep-b")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();

    // Mutate only dep-a.
    fs::write(pkg_a.path().join("skills").join("s.md"), "# skill a-v2").unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["update", "dep-a"])
        .assert()
        .success();

    let lock2 = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    let hash_a2 = lock2
        .packages
        .iter()
        .find(|p| p.name == "dep-a")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();
    let hash_b2 = lock2
        .packages
        .iter()
        .find(|p| p.name == "dep-b")
        .unwrap()
        .content_hash
        .clone()
        .unwrap();

    assert_ne!(hash_a1, hash_a2, "dep-a content_hash must change");
    assert_eq!(
        hash_b1, hash_b2,
        "dep-b content_hash must NOT change (source unchanged)"
    );
}

// ---------------------------------------------------------------------------
// Step 4 — Update non-existent package: non-zero exit, error names the package
// and lists known packages for discoverability.
// ---------------------------------------------------------------------------

#[test]
fn step4_update_non_existent_package_fails_and_lists_known() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = build_local_pkg("v1");

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
real-dep = {{ source = "local:{}" }}
"#,
            pkg.path().display()
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["update", "nope-not-there"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("nope-not-there"))
        .stderr(predicates::str::contains("real-dep"));
}

// ---------------------------------------------------------------------------
// Step 5 — Update local dep: re-copies content, updates content_hash
// ---------------------------------------------------------------------------

#[test]
fn step5_update_local_dep_recopies_and_updates_content_hash() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = build_local_pkg("v1");

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
local-dep = {{ source = "local:{}" }}
"#,
            pkg.path().display()
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();

    let lock1 = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    let hash1 = lock1.packages[0].content_hash.clone().unwrap();

    // Mutate source.
    fs::write(pkg.path().join("skills").join("s.md"), "# skill v2 updated").unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["update", "local-dep"])
        .assert()
        .success();

    let lock2 = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    let hash2 = lock2.packages[0].content_hash.clone().unwrap();

    assert_ne!(hash1, hash2, "content_hash must change");

    // Installed file reflects new content.
    let installed =
        fs::read_to_string(cwd.path().join(".claude").join("skills").join("s.md")).unwrap();
    assert!(
        installed.contains("v2 updated"),
        "expected re-copied file to have v2 content, got: {installed}"
    );
}

// ---------------------------------------------------------------------------
// Step 6 — Update marketplace plugin: driver's install_marketplace_plugin
// re-invoked; uninstall_marketplace_plugin also called in between.
// ---------------------------------------------------------------------------

#[test]
fn step6_update_marketplace_plugin_reinvokes_install() {
    let bin_dir = tempdir().unwrap();
    let log = write_claude_shim(bin_dir.path());

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    write_agentfile(
        cwd.path(),
        r#"[agix]
cli = ["claude"]

[claude.dependencies]
mkt = { source = "marketplace:fantoine/claude-plugins@roundtable" }
"#,
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", path_with(bin_dir.path()))
        .arg("install")
        .assert()
        .success();

    // Reset the log so we can assert exactly what `update` invokes.
    fs::write(&log, "").unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", path_with(bin_dir.path()))
        .args(["update", "mkt"])
        .assert()
        .success();

    let claude_log = fs::read_to_string(&log).unwrap();
    // uninstall then install (both keyed by marketplace alias) must be visible.
    assert!(
        claude_log.contains("plugin uninstall roundtable@claude-plugins"),
        "expected update to call alias-keyed `plugin uninstall`; got: {claude_log}"
    );
    assert!(
        claude_log.contains("plugin install roundtable@claude-plugins"),
        "expected update to re-invoke alias-keyed `plugin install`; got: {claude_log}"
    );
}

// ---------------------------------------------------------------------------
// Step 7 — Update without Agentfile: exit non-zero
// ---------------------------------------------------------------------------
//
// (Already covered by the legacy smoke tests `update_fails_without_agentfile`
// and `update_single_package_fails_without_agentfile` above.)

// ---------------------------------------------------------------------------
// Step 8 — Update without lock file: behaves as install-all (update-all) or
// errors (update-specific) with a clear, package-named message.
// ---------------------------------------------------------------------------
//
// Decision (logged in findings): `update` with no lock file is treated as a
// fresh install for the update-all path (consistent with "re-resolve every
// dep"), and as a non-zero error for update-specific (there is no entry to
// refresh). Rationale: update-all is ambiguous-but-useful, update-specific
// is unambiguously wrong on an empty lock.

#[test]
fn step8a_update_all_without_lock_behaves_as_install() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = build_local_pkg("v1");

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
fresh = {{ source = "local:{}" }}
"#,
            pkg.path().display()
        ),
    );

    // No prior install → no Agentfile.lock.
    assert!(!cwd.path().join("Agentfile.lock").exists());

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("update")
        .assert()
        .success();

    assert!(cwd.path().join("Agentfile.lock").exists());
    let lock = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    assert_eq!(lock.packages.len(), 1);
    assert_eq!(lock.packages[0].name, "fresh");
}

#[test]
fn step8b_update_specific_without_lock_errors_clearly() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = build_local_pkg("v1");

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
fresh = {{ source = "local:{}" }}
"#,
            pkg.path().display()
        ),
    );

    assert!(!cwd.path().join("Agentfile.lock").exists());

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["update", "fresh"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("fresh"));
}

// Note: a previous regression verified `update` falling back to file-based
// uninstall when the lock had an unparseable source. The typed-source
// refactor removed that fallback — sources are parsed eagerly at lock-load
// time. `update` now relies on `LockFile::from_file_or_default`, which
// silently treats a malformed lock as empty and reinstalls from the
// manifest (the stranded legacy files are the user's to clean up). The
// parse-rejection invariant itself is unit-tested at the SourceBox layer.
