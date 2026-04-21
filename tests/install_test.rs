use agix::core::installer::Installer;
use agix::core::lock::LockFile;
use agix::drivers::Scope;
use agix::manifest::agentfile::ProjectManifest;
use predicates::prelude::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

mod helpers;

// ---------------------------------------------------------------------------
// Existing smoke coverage (kept from earlier versions)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn install_local_package_for_claude() {
    let pkg_dir = tempdir().unwrap();
    let skills_dir = pkg_dir.path().join("skills");
    std::fs::create_dir(&skills_dir).unwrap();
    std::fs::write(skills_dir.join("my-skill.md"), "# My Skill").unwrap();

    let manifest_str = format!(
        r#"
[agix]
cli = ["claude"]

[claude.dependencies]
my-pkg = {{ source = "local:{}" }}
"#,
        pkg_dir.path().display()
    );
    let manifest: ProjectManifest = toml::from_str(&manifest_str).unwrap();

    let install_dir = tempdir().unwrap();
    let lock_path = install_dir.path().join("Agentfile.lock");

    Installer::install_manifest(&manifest, &lock_path, &Scope::Local)
        .await
        .unwrap();

    let lock = LockFile::from_file(&lock_path).unwrap();
    assert_eq!(lock.packages.len(), 1);
    assert_eq!(lock.packages[0].name, "my-pkg");
    assert!(!lock.packages[0].files.is_empty());
}

#[tokio::test]
async fn install_command_from_agentfile() {
    let dir = tempdir().unwrap();
    let pkg_dir = tempdir().unwrap();
    let skills = pkg_dir.path().join("skills");
    std::fs::create_dir(&skills).unwrap();
    std::fs::write(skills.join("skill.md"), "# skill").unwrap();

    let manifest_str = format!(
        r#"
[agix]
cli = ["claude"]

[claude.dependencies]
my-pkg = {{ source = "local:{}" }}
"#,
        pkg_dir.path().display()
    );
    std::fs::write(dir.path().join("Agentfile"), &manifest_str).unwrap();

    let home = tempdir().unwrap();
    helpers::cmd_non_interactive(home.path())
        .current_dir(dir.path())
        .arg("install")
        .assert()
        .success();
    assert!(dir.path().join("Agentfile.lock").exists());
}

// ---------------------------------------------------------------------------
// Task 12 step helpers
// ---------------------------------------------------------------------------

fn write_agentfile(cwd: &Path, content: &str) {
    fs::write(cwd.join("Agentfile"), content).unwrap();
}

/// Build a local package fixture mirroring `tests/fixtures/mock-full-pkg/`
/// in a throwaway tempdir so tests are independent of the repo copy.
fn build_full_pkg() -> TempDir {
    let pkg = tempdir().unwrap();
    fs::create_dir(pkg.path().join("skills")).unwrap();
    fs::write(pkg.path().join("skills").join("a.md"), "# skill a").unwrap();
    fs::create_dir(pkg.path().join("agents")).unwrap();
    fs::write(pkg.path().join("agents").join("b.md"), "# agent b").unwrap();
    fs::create_dir(pkg.path().join("hooks")).unwrap();
    fs::write(pkg.path().join("hooks").join("c.md"), "# hook c").unwrap();
    fs::create_dir(pkg.path().join("mcp-servers")).unwrap();
    fs::write(pkg.path().join("mcp-servers").join("d.md"), "# mcp d").unwrap();
    fs::write(pkg.path().join("README.md"), "# mock full pkg").unwrap();
    pkg
}

fn write_claude_shim(dir: &Path) -> PathBuf {
    let log = dir.join("claude-invocations.log");
    let shim = dir.join("claude");
    fs::write(
        &shim,
        format!("#!/bin/sh\necho \"$@\" >> {}\nexit 0\n", log.display()),
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
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
// Step 2 — Fresh install of local dep (full-pkg fixture): all subdirs copied
// ---------------------------------------------------------------------------

#[test]
fn step2_fresh_install_of_full_pkg_copies_all_subdirs() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = build_full_pkg();

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
full = {{ source = "local:{}" }}
"#,
            pkg.path().display()
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();

    let claude_dir = cwd.path().join(".claude");
    assert!(claude_dir.join("skills").join("a.md").exists());
    assert!(claude_dir.join("agents").join("b.md").exists());
    assert!(claude_dir.join("hooks").join("c.md").exists());
    // mcp-servers/ maps to mcp/ per ClaudeDriver::install_with_base.
    assert!(claude_dir.join("mcp").join("d.md").exists());
    assert!(claude_dir.join("README.md").exists());
}

// ---------------------------------------------------------------------------
// Step 3 — Install twice: idempotent / no-op on second run
// ---------------------------------------------------------------------------

#[test]
fn step3_install_twice_is_idempotent() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = build_full_pkg();

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
full = {{ source = "local:{}" }}
"#,
            pkg.path().display()
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();
    let lock1 = fs::read_to_string(cwd.path().join("Agentfile.lock")).unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();
    let lock2 = fs::read_to_string(cwd.path().join("Agentfile.lock")).unwrap();

    assert_eq!(
        lock1, lock2,
        "second install should not rewrite lock when nothing changed"
    );
    // Files still in place.
    assert!(cwd
        .path()
        .join(".claude")
        .join("skills")
        .join("a.md")
        .exists());
}

// ---------------------------------------------------------------------------
// Step 4 — Install after local source file changes: content_hash detects diff
// ---------------------------------------------------------------------------

#[test]
fn step4_reinstall_after_source_change_updates_content_hash_and_rewrites_file() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = build_full_pkg();

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
full = {{ source = "local:{}" }}
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

    // Mutate a source file and reinstall.
    fs::write(pkg.path().join("skills").join("a.md"), "# skill a v2").unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();

    let lock2 = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    let hash2 = lock2.packages[0].content_hash.clone().unwrap();

    assert_ne!(hash1, hash2, "content_hash must change when sources change");
    // And the installed file now reflects the new content.
    let installed =
        fs::read_to_string(cwd.path().join(".claude").join("skills").join("a.md")).unwrap();
    assert!(
        installed.contains("v2"),
        "expected installed file to be re-copied, got: {installed}"
    );
}

// ---------------------------------------------------------------------------
// Step 5 — Install github dep with `version = "main"`
// ---------------------------------------------------------------------------
//
// Per deferred finding "add github — no wired test seam for hermetic GitHub
// integration tests", the CLI path does not honour an `AGIX_GITHUB_BASE_URL`
// override, so we cannot exercise a real GitHubSource end-to-end here. `git:`
// sources (with `?ref=…`) cover the same code path up to the fetch itself,
// so we exercise `version = "main"` via a local bare git repo and document
// the gap inline.

#[test]
fn step5_install_git_dep_with_version_branch_checks_out_that_ref() {
    let src = tempdir().unwrap();
    let repo = git2::Repository::init(src.path()).unwrap();
    let sig = git2::Signature::now("t", "t@t.t").unwrap();
    fs::write(src.path().join("skill.md"), "# main tip").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(Path::new("skill.md")).unwrap();
    idx.write().unwrap();
    let oid = idx.write_tree().unwrap();
    let tree = repo.find_tree(oid).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap();

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let url = format!("file://{}", src.path().display());
    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
from-git = {{ source = "git:{url}", version = "main" }}
"#
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();

    // File from the tip was copied into the Claude scope.
    assert!(cwd.path().join(".claude").join("skill.md").exists());
    let lock = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    assert_eq!(lock.packages.len(), 1);
    assert_eq!(lock.packages[0].name, "from-git");
}

// ---------------------------------------------------------------------------
// Step 6 — Install mixed sources (local + git + marketplace)
// ---------------------------------------------------------------------------

#[test]
fn step6_install_mixed_local_git_and_marketplace_sources() {
    // Local dep.
    let local_pkg = build_full_pkg();

    // Git dep from a local bare repo.
    let git_src = tempdir().unwrap();
    let repo = git2::Repository::init(git_src.path()).unwrap();
    let sig = git2::Signature::now("t", "t@t.t").unwrap();
    fs::write(git_src.path().join("git-skill.md"), "# from git").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(Path::new("git-skill.md")).unwrap();
    idx.write().unwrap();
    let oid = idx.write_tree().unwrap();
    let tree = repo.find_tree(oid).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap();

    // claude shim for marketplace.
    let bin_dir = tempdir().unwrap();
    let log = write_claude_shim(bin_dir.path());

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let git_url = format!("file://{}", git_src.path().display());

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
local-dep = {{ source = "local:{}" }}
git-dep = {{ source = "git:{git_url}" }}
market-dep = {{ source = "marketplace:fantoine/claude-plugins@roundtable" }}
"#,
            local_pkg.path().display()
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", path_with(bin_dir.path()))
        .arg("install")
        .assert()
        .success();

    // Local files landed.
    assert!(cwd
        .path()
        .join(".claude")
        .join("skills")
        .join("a.md")
        .exists());
    // Git file landed.
    assert!(cwd.path().join(".claude").join("git-skill.md").exists());
    // Marketplace shim was invoked.
    let claude_log = fs::read_to_string(&log).unwrap();
    assert!(
        claude_log.contains("plugin install roundtable@fantoine/claude-plugins"),
        "expected marketplace install invocation; got: {claude_log}"
    );

    let lock = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    assert_eq!(lock.packages.len(), 3);
}

// ---------------------------------------------------------------------------
// Step 7 — CLI-specific deps: no cross-contamination between [claude] and [codex]
// ---------------------------------------------------------------------------

#[test]
fn step7_cli_specific_deps_do_not_cross_contaminate() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let claude_pkg = build_full_pkg();
    let codex_pkg = tempdir().unwrap();
    fs::write(codex_pkg.path().join("codex-only.md"), "# codex").unwrap();

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude", "codex"]

[claude.dependencies]
claude-only = {{ source = "local:{}" }}

[codex.dependencies]
codex-only = {{ source = "local:{}" }}
"#,
            claude_pkg.path().display(),
            codex_pkg.path().display()
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();

    let lock = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();

    let claude_pkg_entry = lock
        .packages
        .iter()
        .find(|p| p.name == "claude-only")
        .expect("claude-only should be in lock");
    assert_eq!(claude_pkg_entry.cli, vec!["claude".to_string()]);

    let codex_pkg_entry = lock
        .packages
        .iter()
        .find(|p| p.name == "codex-only")
        .expect("codex-only should be in lock");
    assert_eq!(codex_pkg_entry.cli, vec!["codex".to_string()]);

    // And: claude's files landed under .claude/, codex's under .codex/agix/.
    assert!(cwd
        .path()
        .join(".claude")
        .join("skills")
        .join("a.md")
        .exists());
    // codex driver uses `.codex/agix/` layout; cross-check we didn't mirror
    // into the claude tree.
    assert!(!cwd.path().join(".claude").join("codex-only.md").exists());
}

// ---------------------------------------------------------------------------
// Step 8 — Install with exclude = ["codex"]: shared dep skips codex
// ---------------------------------------------------------------------------

#[test]
fn step8_shared_dep_with_exclude_codex_only_installs_for_claude() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = build_full_pkg();

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude", "codex"]

[dependencies]
shared = {{ source = "local:{}", exclude = ["codex"] }}
"#,
            pkg.path().display()
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();

    let lock = LockFile::from_file(&cwd.path().join("Agentfile.lock")).unwrap();
    let shared = lock
        .packages
        .iter()
        .find(|p| p.name == "shared")
        .expect("shared dep should be in lock");
    assert!(shared.cli.contains(&"claude".to_string()));
    assert!(
        !shared.cli.contains(&"codex".to_string()),
        "codex should be excluded; got: {:?}",
        shared.cli
    );
    // Files land under .claude/ only.
    assert!(cwd
        .path()
        .join(".claude")
        .join("skills")
        .join("a.md")
        .exists());
}

// ---------------------------------------------------------------------------
// Step 9 — Install without Agentfile: non-zero exit
// ---------------------------------------------------------------------------

#[test]
fn step9_install_without_agentfile_exits_nonzero() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .failure();
}

// ---------------------------------------------------------------------------
// Step 10 — Install with invalid source: non-zero, names the dep
// ---------------------------------------------------------------------------

#[test]
fn step10_install_with_invalid_source_names_the_dep() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    write_agentfile(
        cwd.path(),
        r#"[agix]
cli = ["claude"]

[claude.dependencies]
broken = { source = "nope:whatever" }
"#,
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .failure()
        .stderr(predicates::str::contains("broken"));
}

// ---------------------------------------------------------------------------
// Step 11 — Install when driver not detected: warn, install anyway, exit 0
// ---------------------------------------------------------------------------
//
// Behavioural change from e7ab663: the Fetched branch of install_manifest no
// longer skips when driver.detect() is false. Files land in the scope's base
// dir so a later `claude` install picks them up; a warning is emitted.

#[test]
fn step11_install_when_claude_not_detected_warns_but_installs() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = build_full_pkg();

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
orphan = {{ source = "local:{}" }}
"#,
            pkg.path().display()
        ),
    );

    // Hide `claude` from PATH so which::which("claude") fails, and don't create
    // ~/.claude in the scratch HOME. Both branches of ClaudeDriver::detect()
    // must be false for this test to validate the "not detected" codepath.
    // The local-source install path does not spawn subprocesses, so an empty
    // PATH is fine here (and keeps the test hermetic on machines where
    // `claude` happens to live in /usr/bin).
    let empty_bin = tempdir().unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", empty_bin.path())
        .arg("install")
        .assert()
        .success()
        .stderr(predicates::str::contains("not detected"));

    // Files still land under the local scope base.
    assert!(cwd
        .path()
        .join(".claude")
        .join("skills")
        .join("a.md")
        .exists());
}

// ---------------------------------------------------------------------------
// Step 12 — Install with post-install hook: script executes
// ---------------------------------------------------------------------------

#[test]
fn step12_post_install_hook_executes() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();

    // Package: has a skills/ dir (so something copies), an Agentfile declaring
    // a post-install hook, and the hook script itself.
    fs::create_dir(pkg.path().join("skills")).unwrap();
    fs::write(pkg.path().join("skills").join("s.md"), "# s").unwrap();

    let marker = pkg.path().join("post-install-ran.marker");
    fs::write(
        pkg.path().join("install.sh"),
        format!("#!/bin/sh\ntouch {}\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(
        pkg.path().join("install.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();

    // Package-level Agentfile declaring the hook.
    fs::write(
        pkg.path().join("Agentfile"),
        r#"[agix]
name = "hooked"
version = "0.1.0"

[hooks]
post-install = "install.sh"
"#,
    )
    .unwrap();

    write_agentfile(
        cwd.path(),
        &format!(
            r#"[agix]
cli = ["claude"]

[claude.dependencies]
hooked = {{ source = "local:{}" }}
"#,
            pkg.path().display()
        ),
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("install")
        .assert()
        .success();

    assert!(
        marker.exists(),
        "expected post-install script to create marker {marker:?}"
    );
}

// ---------------------------------------------------------------------------
// Regression: idempotent marketplace reinstall stays silent on "already installed"
// ---------------------------------------------------------------------------
//
// Task 4 finding: `claude plugin install <x>@<y>` exits non-zero when the
// plugin is already installed, which ClaudeDriver::install_marketplace_plugin
// used to surface as AgixError::Other → the installer warned. Fix: the driver
// now captures output and treats "already installed" / "already exists" /
// "already added" on non-zero exit as success.

#[test]
fn regression_marketplace_reinstall_already_installed_is_silent_success() {
    // Shim emits "Plugin already installed" on stderr and exits 1, mimicking
    // Claude Code's behaviour when re-installing a registered plugin.
    let bin_dir = tempdir().unwrap();
    let shim = bin_dir.path().join("claude");
    fs::write(
        &shim,
        "#!/bin/sh\necho 'Plugin already installed' 1>&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    write_agentfile(
        cwd.path(),
        r#"[agix]
cli = ["claude"]

[claude.dependencies]
repeat = { source = "marketplace:fantoine/claude-plugins@roundtable" }
"#,
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", path_with(bin_dir.path()))
        .arg("install")
        .assert()
        .success()
        // No warn about install-failed should reach the user on this path.
        .stderr(predicates::str::contains("install failed for").not());
}
