use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

mod helpers;

const MINIMAL_AGENTFILE: &str = "[agix]\ncli = []\n\n[dependencies]\n";
const MINIMAL_LOCK: &str = "";

// ---------------------------------------------------------------------------
// Walk-up: Agentfile in parent, cwd is a subdirectory
// ---------------------------------------------------------------------------

#[test]
fn walkup_finds_agentfile_in_parent_directory() {
    let home = tempdir().unwrap();
    // project is a subdirectory of home so walk-up stops at home boundary
    let project = home.path().join("project");
    fs::create_dir_all(&project).unwrap();
    let sub = project.join("src");
    fs::create_dir_all(&sub).unwrap();

    // Write a dep that's unique to this Agentfile so we can verify it was read
    fs::write(
        project.join("Agentfile"),
        "[agix]\ncli = []\n\n[dependencies]\nwalkup-test-dep = { source = \"local:./x\" }\n",
    )
    .unwrap();
    fs::write(project.join("Agentfile.lock"), MINIMAL_LOCK).unwrap();

    // list from sub/ should find project/Agentfile via walk-up
    helpers::cmd_non_interactive(home.path())
        .current_dir(&sub)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("walkup-test-dep"));
}

// ---------------------------------------------------------------------------
// Walk-up: no Agentfile anywhere → fallback to global
// ---------------------------------------------------------------------------

#[test]
fn walkup_falls_back_to_global_when_no_agentfile() {
    let home = tempdir().unwrap();
    let agix_dir = home.path().join(".agix");
    fs::create_dir_all(&agix_dir).unwrap();
    fs::write(
        agix_dir.join("Agentfile"),
        "[agix]\ncli = []\n\n[dependencies]\nglobal-only-dep = { source = \"local:./g\" }\n",
    )
    .unwrap();
    fs::write(agix_dir.join("Agentfile.lock"), MINIMAL_LOCK).unwrap();

    // cwd is a separate tempdir with no Agentfile → falls back to global
    let cwd = tempdir().unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("global-only-dep"));
}

// ---------------------------------------------------------------------------
// Walk-up: -g forces global even when project Agentfile exists
// ---------------------------------------------------------------------------

#[test]
fn global_flag_overrides_walkup_inside_project() {
    let home = tempdir().unwrap();
    let agix_dir = home.path().join(".agix");
    fs::create_dir_all(&agix_dir).unwrap();
    fs::write(
        agix_dir.join("Agentfile"),
        "[agix]\ncli = []\n\n[dependencies]\nglobal-dep = { source = \"local:./g\" }\n",
    )
    .unwrap();
    fs::write(agix_dir.join("Agentfile.lock"), MINIMAL_LOCK).unwrap();

    let project = home.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join("Agentfile"),
        "[agix]\ncli = []\n\n[dependencies]\nlocal-dep = { source = \"local:./l\" }\n",
    )
    .unwrap();
    fs::write(project.join("Agentfile.lock"), MINIMAL_LOCK).unwrap();

    // -g should use ~/.agix/Agentfile, not project/Agentfile
    helpers::cmd_non_interactive(home.path())
        .current_dir(&project)
        .args(["list", "-g"])
        .assert()
        .success()
        .stdout(predicate::str::contains("global-dep"))
        .stdout(predicate::str::contains("local-dep").not());
}

// ---------------------------------------------------------------------------
// Nested projects: inner Agentfile wins over outer
// ---------------------------------------------------------------------------

#[test]
fn nested_project_inner_agentfile_wins() {
    let home = tempdir().unwrap();
    let outer = home.path().join("outer");
    let inner = outer.join("inner");
    fs::create_dir_all(&inner).unwrap();

    fs::write(
        outer.join("Agentfile"),
        "[agix]\ncli = []\n\n[dependencies]\nouter-dep = { source = \"local:./x\" }\n",
    )
    .unwrap();
    fs::write(outer.join("Agentfile.lock"), MINIMAL_LOCK).unwrap();
    fs::write(
        inner.join("Agentfile"),
        "[agix]\ncli = []\n\n[dependencies]\ninner-dep = { source = \"local:./y\" }\n",
    )
    .unwrap();
    fs::write(inner.join("Agentfile.lock"), MINIMAL_LOCK).unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(&inner)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("inner-dep"))
        .stdout(predicate::str::contains("outer-dep").not());
}

// ---------------------------------------------------------------------------
// init creates in cwd, not in parent (no walk-up for init)
// ---------------------------------------------------------------------------

#[test]
fn init_creates_in_cwd_not_parent() {
    let home = tempdir().unwrap();
    let project = home.path().join("project");
    fs::create_dir_all(&project).unwrap();
    // Agentfile exists in project root
    fs::write(project.join("Agentfile"), MINIMAL_AGENTFILE).unwrap();
    let sub = project.join("sub");
    fs::create_dir_all(&sub).unwrap();
    // No Agentfile in sub

    helpers::cmd_non_interactive(home.path())
        .current_dir(&sub)
        .args(["init", "--no-interactive"])
        .assert()
        .success();

    assert!(sub.join("Agentfile").exists(), "init must create in cwd");
}

// ---------------------------------------------------------------------------
// init -g creates in ~/.agix/
// ---------------------------------------------------------------------------

#[test]
fn init_global_creates_in_home_agix() {
    let home = tempdir().unwrap();
    let cwd = tempdir().unwrap();

    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["init", "-g", "--no-interactive"])
        .assert()
        .success();

    assert!(
        home.path().join(".agix").join("Agentfile").exists(),
        "init -g must create ~/.agix/Agentfile"
    );
    assert!(
        !cwd.path().join("Agentfile").exists(),
        "init -g must not create Agentfile in cwd"
    );
}

// ---------------------------------------------------------------------------
// --scope flag is no longer accepted (removed in v0.2.0)
// ---------------------------------------------------------------------------

#[test]
fn scope_flag_is_removed() {
    let home = tempdir().unwrap();
    helpers::cmd_non_interactive(home.path())
        .args(["list", "--scope", "global"])
        .assert()
        .failure();
}
