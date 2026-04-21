use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

// Shared hermetic-run helper matching the other command suites: tempdir cwd +
// tempdir HOME + AGIX_NO_INTERACTIVE=1. Without this the command can read or
// write into the developer's real $HOME during auto-init flows.
fn outdated_cmd(cwd: &std::path::Path, home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("agix").unwrap();
    cmd.env("AGIX_NO_INTERACTIVE", "1")
        .env("HOME", home)
        .current_dir(cwd);
    cmd
}

// ---------------------------------------------------------------------------
// Step 5: no Agentfile — exit non-zero with an actionable message.
// ---------------------------------------------------------------------------

#[test]
fn step5_outdated_without_agentfile_exits_non_zero() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    outdated_cmd(cwd.path(), home.path())
        .arg("outdated")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no Agentfile"))
        .stderr(predicate::str::contains("agix init"));
}

// ---------------------------------------------------------------------------
// Step 6: no lock file — `outdated` needs a baseline, so this is a hard error
// with a pointer at `agix install`. Decision captured in the findings log.
// ---------------------------------------------------------------------------

#[test]
fn step6_outdated_without_lock_errors_clearly() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]

[dependencies]
some-dep = { source = "local:/tmp/x" }
"#,
    )
    .unwrap();
    // Deliberately no Agentfile.lock.

    outdated_cmd(cwd.path(), home.path())
        .arg("outdated")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no lock file"))
        .stderr(predicate::str::contains("agix install"));
}

// ---------------------------------------------------------------------------
// Step 2: no updates available (all deps are local or unchangeable) —
// command succeeds with a friendly "all up to date" message.
//
// A local-only lock is the cheapest way to exercise the "nothing to update"
// path without touching the network.
// ---------------------------------------------------------------------------

#[test]
fn step2_outdated_with_only_local_deps_reports_all_up_to_date() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]

[dependencies]
my-tool = { source = "local:../my-tool" }
"#,
    )
    .unwrap();
    fs::write(
        cwd.path().join("Agentfile.lock"),
        r#"
[[package]]
name = "my-tool"
source = "local:../my-tool"
cli = ["claude"]
scope = "local"
files = []
"#,
    )
    .unwrap();

    outdated_cmd(cwd.path(), home.path())
        .arg("outdated")
        .assert()
        .success()
        .stdout(predicate::str::contains("All dependencies are up to date"));
}

// ---------------------------------------------------------------------------
// Step 4: local-only deps show up as "local (not checkable)" in the body, in
// addition to the aggregate "all up to date" summary.
// ---------------------------------------------------------------------------

#[test]
fn step4_outdated_labels_local_deps_as_not_checkable() {
    let cwd = tempdir().unwrap();
    let home = tempdir().unwrap();

    fs::write(
        cwd.path().join("Agentfile"),
        r#"[agix]
cli = ["claude"]

[dependencies]
my-tool = { source = "local:../my-tool" }
"#,
    )
    .unwrap();
    fs::write(
        cwd.path().join("Agentfile.lock"),
        r#"
[[package]]
name = "my-tool"
source = "local:../my-tool"
cli = ["claude"]
scope = "local"
files = []
"#,
    )
    .unwrap();

    outdated_cmd(cwd.path(), home.path())
        .arg("outdated")
        .assert()
        .success()
        .stdout(predicate::str::contains("my-tool"))
        .stdout(predicate::str::contains("local (not checkable)"));
}

// ---------------------------------------------------------------------------
// Step 3 + Step 7: remote-resolve via mockito.
//
// We can't hit the real github.com from the CLI path (the API base is
// hard-coded in `GitHubSource::new`, and wiring an env-var seam through the
// whole CLI is the deferred `add github` finding). So these tests call the
// lib-level `check_outdated` directly with a mockito-backed API base.
// This is the pragmatic test path the task brief (finding #1) explicitly
// steers us to.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn step3_outdated_reports_package_when_remote_sha_differs() {
    use agix::commands::outdated::{check_outdated, OutdatedStatus};
    use agix::core::lock::{LockFile, LockedPackage};
    use agix::manifest::agentfile::{AgixSection, Dependency, ProjectManifest};
    use std::collections::HashMap;

    let mut server = mockito::Server::new_async().await;
    // Floating ref "main" resolved against the mock server returns a new SHA
    // that doesn't match the one currently in the lock.
    let _mock = server
        .mock("GET", "/repos/fantoine/claude-later/git/ref/tags/main")
        .with_status(404)
        .create_async()
        .await;
    let _mock_branch = server
        .mock("GET", "/repos/fantoine/claude-later/git/ref/heads/main")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"object":{"sha":"ffffffffffffffffffffffffffffffffffffffff"}}"#)
        .create_async()
        .await;

    // Manifest requests the floating ref `main`.
    let mut deps = HashMap::new();
    deps.insert(
        "claude-later".to_string(),
        Dependency {
            source: agix::sources::SourceBox::parse("github:fantoine/claude-later").unwrap(),
            version: Some("main".to_string()),
            exclude: None,
        },
    );
    let manifest = ProjectManifest {
        agix: AgixSection {
            cli: vec!["claude".to_string()],
            name: None,
            version: None,
            description: None,
        },
        dependencies: deps,
        cli_dependencies: HashMap::new(),
    };

    // Lock pins an older SHA — the one mockito returns is different, so the
    // package must be reported as outdated.
    let lock = LockFile {
        packages: vec![LockedPackage {
            name: "claude-later".to_string(),
            source: agix::sources::SourceBox::parse("github:fantoine/claude-later").unwrap(),
            sha: Some("0000000000000000000000000000000000000000".to_string()),
            content_hash: None,
            version: Some("main".to_string()),
            cli: vec!["claude".to_string()],
            scope: "local".to_string(),
            files: vec![],
        }],
    };

    let statuses = check_outdated(&manifest, &lock, Some(&server.url()))
        .await
        .unwrap();

    assert_eq!(statuses.len(), 1);
    match &statuses[0] {
        OutdatedStatus::Outdated {
            name,
            current_sha,
            available_sha,
        } => {
            assert_eq!(name, "claude-later");
            assert_eq!(current_sha, "0000000000000000000000000000000000000000");
            assert_eq!(available_sha, "ffffffffffffffffffffffffffffffffffffffff");
        }
        other => panic!("expected Outdated, got {other:?}"),
    }
}

#[tokio::test]
async fn step3_outdated_up_to_date_when_remote_sha_matches() {
    use agix::commands::outdated::{check_outdated, OutdatedStatus};
    use agix::core::lock::{LockFile, LockedPackage};
    use agix::manifest::agentfile::{AgixSection, Dependency, ProjectManifest};
    use std::collections::HashMap;

    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/repos/fantoine/claude-later/git/ref/tags/main")
        .with_status(404)
        .create_async()
        .await;
    let _mock_branch = server
        .mock("GET", "/repos/fantoine/claude-later/git/ref/heads/main")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"object":{"sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#)
        .create_async()
        .await;

    let mut deps = HashMap::new();
    deps.insert(
        "claude-later".to_string(),
        Dependency {
            source: agix::sources::SourceBox::parse("github:fantoine/claude-later").unwrap(),
            version: Some("main".to_string()),
            exclude: None,
        },
    );
    let manifest = ProjectManifest {
        agix: AgixSection {
            cli: vec!["claude".to_string()],
            name: None,
            version: None,
            description: None,
        },
        dependencies: deps,
        cli_dependencies: HashMap::new(),
    };

    let lock = LockFile {
        packages: vec![LockedPackage {
            name: "claude-later".to_string(),
            source: agix::sources::SourceBox::parse("github:fantoine/claude-later").unwrap(),
            sha: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
            content_hash: None,
            version: Some("main".to_string()),
            cli: vec!["claude".to_string()],
            scope: "local".to_string(),
            files: vec![],
        }],
    };

    let statuses = check_outdated(&manifest, &lock, Some(&server.url()))
        .await
        .unwrap();

    assert_eq!(statuses.len(), 1);
    match &statuses[0] {
        OutdatedStatus::UpToDate { name, sha } => {
            assert_eq!(name, "claude-later");
            assert_eq!(sha, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        }
        other => panic!("expected UpToDate, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Marketplace deps show up labeled and do not attempt any network I/O.
// This is the companion to Step 4 for marketplace sources.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn marketplace_deps_are_labeled_not_remotely_checked() {
    use agix::commands::outdated::{check_outdated, OutdatedStatus};
    use agix::core::lock::{LockFile, LockedPackage};
    use agix::manifest::agentfile::{AgixSection, ProjectManifest};
    use std::collections::HashMap;

    let manifest = ProjectManifest {
        agix: AgixSection {
            cli: vec!["claude".to_string()],
            name: None,
            version: None,
            description: None,
        },
        dependencies: HashMap::new(),
        cli_dependencies: HashMap::new(),
    };

    let lock = LockFile {
        packages: vec![LockedPackage {
            name: "roundtable".to_string(),
            source: agix::sources::SourceBox::parse(
                "marketplace:fantoine/claude-plugins@roundtable",
            )
            .unwrap(),
            sha: None,
            content_hash: None,
            version: None,
            cli: vec!["claude".to_string()],
            scope: "local".to_string(),
            files: vec![],
        }],
    };

    // `None` for api_base — the marketplace path must not reach the network,
    // so this is safe even without mockito.
    let statuses = check_outdated(&manifest, &lock, None).await.unwrap();
    assert_eq!(statuses.len(), 1);
    match &statuses[0] {
        OutdatedStatus::Marketplace { name, driver } => {
            assert_eq!(name, "roundtable");
            assert_eq!(driver, "claude");
        }
        other => panic!("expected Marketplace, got {other:?}"),
    }
}
