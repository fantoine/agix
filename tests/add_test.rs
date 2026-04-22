use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::{tempdir, TempDir};

mod helpers;

/// Create a cwd with a minimal Agentfile plus a tempdir HOME. Returns both
/// tempdirs (keep alive for the test duration) and a local package dir.
fn setup_with_agentfile(cli_line: &str) -> (TempDir, TempDir, TempDir) {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        format!("[agix]\ncli = [{cli_line}]\n"),
    )
    .unwrap();
    fs::write(pkg.path().join("skill.md"), "# skill").unwrap();
    (cwd, home, pkg)
}

// ---------- Step 2: add local shared dep (no --cli) ----------

#[test]
fn step2_add_local_shared_dep_writes_to_top_level_dependencies() {
    let (cwd, home, pkg) = setup_with_agentfile("\"claude\"");
    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("add")
        .arg("local")
        .arg(pkg.path())
        .assert()
        .success();

    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    assert!(content.contains("[dependencies]"));
    assert!(!content.contains("[claude.dependencies"));
    assert!(content.contains("local:"));
}

// ---------- Step 3: add local --cli claude ----------

#[test]
fn step3_add_local_single_cli_writes_per_cli_section() {
    let (cwd, home, pkg) = setup_with_agentfile("\"claude\"");
    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["add", "local"])
        .arg(pkg.path())
        .args(["--cli", "claude"])
        .assert()
        .success();

    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    assert!(content.contains("[claude.dependencies]"));
}

// ---------- Step 4: add local --cli claude --cli codex ----------

#[test]
fn step4_add_local_multi_cli_writes_under_each_section() {
    let (cwd, home, pkg) = setup_with_agentfile("\"claude\", \"codex\"");
    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["add", "local"])
        .arg(pkg.path())
        .args(["--cli", "claude", "--cli", "codex"])
        .assert()
        .success();

    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    assert!(content.contains("[claude.dependencies]"));
    assert!(content.contains("[codex.dependencies]"));
}

// ---------- Step 5: --cli <unknown-driver> → error listing known drivers ----------

#[test]
fn step5_add_local_unknown_cli_errors_with_known_drivers_listed() {
    let (cwd, home, pkg) = setup_with_agentfile("\"claude\"");
    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["add", "local"])
        .arg(pkg.path())
        .args(["--cli", "not-in-agix-cli"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown CLI 'not-in-agix-cli'"))
        .stderr(predicates::str::contains("claude"))
        .stderr(predicates::str::contains("codex"));

    // And nothing was persisted — Agentfile must not gain a bogus section.
    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    assert!(!content.contains("not-in-agix-cli"));
}

// ---------- Step 6 / 7: GitHub source — currently NOT runnable hermetically ----------
//
// The `GitHubSource` wires its API base through a `#[cfg(test)]` constructor
// only; the CLI path does not honor an `AGIX_GITHUB_BASE_URL`-style env var.
// Wiring that override is out of scope for Task 10 (a Phase-B source-review
// task would own it). Unit tests in `src/sources/github.rs` already exercise
// `resolve_ref` via mockito, so we don't duplicate them here.

// ---------- Step 8: add git from a local bare repo ----------

#[test]
fn step8_add_git_from_local_bare_repo_succeeds() {
    // Build a bare repo served over the filesystem so git2 can clone without
    // touching the internet.
    let src = tempdir().unwrap();
    let repo = git2::Repository::init(src.path()).unwrap();
    let sig = git2::Signature::now("test", "test@test.com").unwrap();
    fs::write(src.path().join("skill.md"), "# skill").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("skill.md")).unwrap();
    index.write().unwrap();
    let oid = index.write_tree().unwrap();
    let tree = repo.find_tree(oid).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap();

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(cwd.path().join("Agentfile"), "[agix]\ncli = []\n").unwrap();

    let url = format!("file://{}", src.path().display());
    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["add", "git", &url])
        .assert()
        .success();

    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    assert!(content.contains("git:"));
}

// ---------- Step 9: add marketplace → invokes claude CLI shim ----------

/// Install a stateful `claude` shim inside `dir` and return the log path.
///
/// State files live in a `state/` subdir of `dir` so they share `dir`'s
/// lifetime (tests pass a `TempDir`-owned path; dropping the TempDir
/// cleans both the shim and its state together).
fn write_claude_shim(dir: &Path) -> std::path::PathBuf {
    let log = dir.join("claude-invocations.log");
    let state = dir.join("state");
    helpers::install_claude_shim(dir, &log, &state);
    log
}

#[test]
fn step9_add_marketplace_local_scope_invokes_claude_shim() {
    let bin_dir = tempdir().unwrap();
    let log_path = write_claude_shim(bin_dir.path());

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(cwd.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    let path_env = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", &path_env)
        .args(["add", "marketplace", "fantoine/claude-plugins@roundtable"])
        .assert()
        .success();

    let log = fs::read_to_string(&log_path).unwrap();
    assert!(log.contains("plugin marketplace list --json"));
    assert!(log.contains("plugin marketplace add fantoine/claude-plugins"));
    // Install uses the alias (basename `claude-plugins`), not the org/repo path.
    assert!(log.contains("plugin install roundtable@claude-plugins"));
    assert!(!log.contains("plugin install roundtable@fantoine/claude-plugins"));
}

// ---------- Step 10: add marketplace --scope global with fresh HOME ----------

#[test]
fn step10_add_marketplace_global_scope_auto_inits_non_interactively() {
    let bin_dir = tempdir().unwrap();
    let _log_path = write_claude_shim(bin_dir.path());

    // Fresh HOME: no ~/.agix/Agentfile yet. With AGIX_NO_INTERACTIVE=1, the
    // auto-init inside `agentfile_paths` must not block on a TTY prompt.
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    let path_env = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", &path_env)
        .args([
            "add",
            "marketplace",
            "fantoine/claude-plugins@roundtable",
            "--scope",
            "global",
        ])
        .assert()
        .success();

    // Global Agentfile was auto-created.
    let global_agentfile = home.path().join(".agix").join("Agentfile");
    assert!(
        global_agentfile.exists(),
        "expected {:?} to exist",
        global_agentfile
    );
    let content = fs::read_to_string(&global_agentfile).unwrap();
    assert!(content.contains("[agix]"));
}

// ---------- Step 11: add without Agentfile → actionable error mentioning `agix init` ----------

#[test]
fn step11_add_without_agentfile_errors_mentioning_init() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    let pkg = tempdir().unwrap();
    fs::write(pkg.path().join("skill.md"), "# s").unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("add")
        .arg("local")
        .arg(pkg.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("Agentfile"))
        .stderr(predicates::str::contains("agix init"));
}

// ---------- Step 12: add ftp nope → error listing known schemes ----------

#[test]
fn step12_add_unknown_source_type_errors_with_known_schemes_listed() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(cwd.path().join("Agentfile"), "[agix]\ncli = []\n").unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["add", "ftp", "nope"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown source type 'ftp'"))
        .stderr(predicates::str::contains("local"))
        .stderr(predicates::str::contains("github"))
        .stderr(predicates::str::contains("git"))
        .stderr(predicates::str::contains("marketplace"));
}

// ---------- Step 13: add local twice → warn + overwrite ----------

#[test]
fn step13_add_same_local_twice_warns_and_overwrites() {
    let (cwd, home, pkg) = setup_with_agentfile("\"claude\"");

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("add")
        .arg("local")
        .arg(pkg.path())
        .assert()
        .success();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("add")
        .arg("local")
        .arg(pkg.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("already in [dependencies]"))
        .stderr(predicates::str::contains("overwriting"));

    // Manifest still valid: exactly one entry for the package. When the dep
    // has no version/exclude, it serialises as a bare-string key
    // (`name = "…"`); when it has extra fields, as a `[dependencies.name]`
    // subtable — accept either by counting both forms.
    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    let name = pkg.path().file_name().unwrap().to_str().unwrap();
    let plain_subtable = format!("[dependencies.{name}]");
    let quoted_subtable = format!("[dependencies.\"{name}\"]");
    let plain_inline = format!("\n{name} = ");
    let quoted_inline = format!("\n\"{name}\" = ");
    let occurrences = content.matches(&plain_subtable).count()
        + content.matches(&quoted_subtable).count()
        + content.matches(&plain_inline).count()
        + content.matches(&quoted_inline).count();
    assert_eq!(
        occurrences, 1,
        "expected 1 entry, got {occurrences}; content: {content}"
    );
}

#[test]
fn step13_add_same_cli_scoped_twice_warns_and_overwrites() {
    let (cwd, home, pkg) = setup_with_agentfile("\"claude\"");

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["add", "local"])
        .arg(pkg.path())
        .args(["--cli", "claude"])
        .assert()
        .success();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["add", "local"])
        .arg(pkg.path())
        .args(["--cli", "claude"])
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "already in [claude.dependencies]",
        ))
        .stderr(predicates::str::contains("overwriting"));
}

// ---------- Step 14: package name inference matches suggested_name() per source type ----------

#[test]
fn step14_suggested_name_local_strips_extension_and_uses_basename() {
    // LocalSource::suggested_name uses the last Normal component — no extension strip.
    // Plan text "strips extension" refers to archive-like sources, not local dirs;
    // keep this test honest: a dir path yields the dir name verbatim.
    let (cwd, home, pkg) = setup_with_agentfile("\"claude\"");
    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("add")
        .arg("local")
        .arg(pkg.path())
        .assert()
        .success();
    let name = pkg.path().file_name().unwrap().to_str().unwrap();
    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    let plain_subtable = format!("[dependencies.{name}]");
    let quoted_subtable = format!("[dependencies.\"{name}\"]");
    let plain_inline = format!("\n{name} = ");
    let quoted_inline = format!("\n\"{name}\" = ");
    assert!(
        content.contains(&plain_subtable)
            || content.contains(&quoted_subtable)
            || content.contains(&plain_inline)
            || content.contains(&quoted_inline),
        "expected entry for {name} (subtable or bare-string form), got: {content}"
    );
}

// Shared-case naming verification for `git` is unit-covered in `sources/git.rs`;
// here we assert the integration produces the expected key.
#[test]
fn step14_suggested_name_git_uses_last_path_minus_dotgit() {
    let src = tempdir().unwrap();
    let repo = git2::Repository::init(src.path()).unwrap();
    let sig = git2::Signature::now("t", "t@t.t").unwrap();
    fs::write(src.path().join("x.md"), "x").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(Path::new("x.md")).unwrap();
    idx.write().unwrap();
    let oid = idx.write_tree().unwrap();
    let tree = repo.find_tree(oid).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap();

    // Use a filesystem path ending in `.git` to prove the `.git` trim works.
    let fake_git_path = src.path().with_extension("git");
    fs::rename(src.path(), &fake_git_path).unwrap();

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(cwd.path().join("Agentfile"), "[agix]\ncli = []\n").unwrap();

    let url = format!("file://{}", fake_git_path.display());
    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["add", "git", &url])
        .assert()
        .success();

    let expected_name = fake_git_path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .trim_end_matches(".git")
        .to_string();
    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    let plain_subtable = format!("[dependencies.{expected_name}]");
    let quoted_subtable = format!("[dependencies.\"{expected_name}\"]");
    let plain_inline = format!("\n{expected_name} = ");
    let quoted_inline = format!("\n\"{expected_name}\" = ");
    assert!(
        content.contains(&plain_subtable)
            || content.contains(&quoted_subtable)
            || content.contains(&plain_inline)
            || content.contains(&quoted_inline),
        "expected entry for {expected_name} (subtable or bare-string form); got: {content}"
    );
}

#[test]
fn step14_suggested_name_marketplace_uses_plugin_name() {
    let bin_dir = tempdir().unwrap();
    let _log = write_claude_shim(bin_dir.path());

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(cwd.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    let path_env = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", &path_env)
        .args(["add", "marketplace", "fantoine/claude-plugins@roundtable"])
        .assert()
        .success();

    let content = fs::read_to_string(cwd.path().join("Agentfile")).unwrap();
    assert!(
        content.contains("[dependencies.roundtable]") || content.contains("\nroundtable = "),
        "expected roundtable entry (subtable or bare-string form); got: {content}"
    );
}

// ---------- Regression: add installs only the new dep, not the whole manifest ----------

#[test]
fn regression_add_marketplace_does_not_install_sibling_deps() {
    // Agentfile starts with a *different* marketplace dep already declared.
    // Running `add marketplace <new>` must process only the new dep — the
    // sibling should be left alone (its install is the job of `agix install`).
    let bin_dir = tempdir().unwrap();
    let log_path = write_claude_shim(bin_dir.path());

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(
        cwd.path().join("Agentfile"),
        "[agix]\ncli = [\"claude\"]\n\n[dependencies]\ncaveman = { source = \"marketplace:JuliusBrussee/caveman@caveman\" }\n",
    )
    .unwrap();

    let path_env = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", &path_env)
        .args(["add", "marketplace", "fantoine/claude-plugins@later"])
        .assert()
        .success();

    let log = fs::read_to_string(&log_path).unwrap();
    assert!(
        log.contains("plugin install later@claude-plugins"),
        "expected the newly-added dep to be installed; got: {log}"
    );
    assert!(
        !log.contains("caveman"),
        "sibling dep must not be touched by `add`; got: {log}"
    );
}

// ---------- Regression: marketplace total-failure returns non-zero ----------

#[test]
fn regression_marketplace_total_failure_returns_nonzero() {
    // claude shim exits non-zero on every invocation.
    let bin_dir = tempdir().unwrap();
    let shim = bin_dir.path().join("claude");
    fs::write(&shim, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();
    fs::write(cwd.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    let path_env = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", &path_env)
        .args(["add", "marketplace", "org/repo@plugin"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "failed to install for all target CLIs",
        ));
}

// ---------- Regression: auto-init non-interactive seam ----------

#[test]
fn regression_global_auto_init_honors_non_interactive_env_var() {
    // Without claude on PATH, detect() is false → target_clis=[], success_count=0,
    // but target_clis is empty so the marketplace branch returns Ok. That's not
    // quite what we want to assert; use a shim so the install actually succeeds.
    let bin_dir = tempdir().unwrap();
    let _log = write_claude_shim(bin_dir.path());

    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    let path_env = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    // Fresh HOME with no global Agentfile. The auto-init inside
    // `agentfile_paths` would historically call `pick_clis(&[], false)` and
    // block; now it forwards the non-interactive state and
    // `AGIX_NO_INTERACTIVE=1` still applies. The command MUST NOT hang.
    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .env("PATH", &path_env)
        .args([
            "add",
            "marketplace",
            "fantoine/claude-plugins@roundtable",
            "--scope",
            "global",
        ])
        .assert()
        .success();

    assert!(home.path().join(".agix").join("Agentfile").exists());
}
