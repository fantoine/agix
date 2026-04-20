use assert_cmd::Command;
use std::io::Read;
use tempfile::tempdir;

/// Read the `Agentfile` entry out of an exported zip as a UTF-8 string.
fn read_agentfile_from_zip(zip_path: &std::path::Path) -> String {
    let file = std::fs::File::open(zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut entry = archive.by_name("Agentfile").expect("Agentfile in zip");
    let mut buf = String::new();
    entry.read_to_string(&mut buf).unwrap();
    buf
}

#[test]
fn export_creates_zip_with_agentfile() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();
    std::fs::write(dir.path().join("Agentfile.lock"), "").unwrap();

    let output = dir.path().join("backup.zip");
    let mut cmd = Command::cargo_bin("agix").unwrap();
    cmd.current_dir(dir.path())
        .arg("export")
        .arg("--output")
        .arg(output.to_str().unwrap());
    cmd.assert().success();
    assert!(output.exists());
}

/// Step 2: with no `--output`, the zip must land at `agix-export.zip` in CWD.
#[test]
fn step2_export_default_filename_writes_agix_export_zip_in_cwd() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .arg("export")
        .assert()
        .success();

    let expected = dir.path().join("agix-export.zip");
    assert!(
        expected.exists(),
        "default filename agix-export.zip should be created in cwd, got no file at {}",
        expected.display()
    );
}

/// Step 4: `--all` is deferred for v0.1.0; must error non-zero with an
/// actionable message so the behaviour is obvious to callers and surfaces
/// clearly in any future regression if we ever start pretending to support it.
#[test]
fn step4_export_all_flag_is_not_yet_implemented_and_errors() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    let assert = Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .args(["export", "--all"])
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("--all is not yet implemented"),
        "expected --all deferral message on stderr, got: {stderr}"
    );
}

/// Step 5: exporting without an Agentfile must exit non-zero and point the
/// user at `agix init` (consistent with `list`, `outdated`, `add`).
#[test]
fn step5_export_without_agentfile_exits_nonzero_and_suggests_init() {
    let dir = tempdir().unwrap();
    // Deliberately no Agentfile.

    let assert = Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .arg("export")
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("no Agentfile"),
        "expected 'no Agentfile' on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("agix init"),
        "expected 'agix init' hint on stderr, got: {stderr}"
    );

    // And no zip should have been created.
    assert!(!dir.path().join("agix-export.zip").exists());
}

/// Step 7: github sources are remote refs — the exported Agentfile must keep
/// the canonical `source = "github:org/repo@ref"` verbatim. No vendoring,
/// no path rewriting, no local-sources/ entry.
#[test]
fn step7_export_preserves_github_source_verbatim() {
    let dir = tempdir().unwrap();
    let agentfile = "[agix]\ncli = [\"claude\"]\n\n\
         [dependencies]\n\
         gh-pkg = { source = \"github:fantoine/claude-later@main\" }\n";
    std::fs::write(dir.path().join("Agentfile"), agentfile).unwrap();

    let out = dir.path().join("state.zip");
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .args(["export", "--output"])
        .arg(&out)
        .assert()
        .success();

    let exported = read_agentfile_from_zip(&out);
    assert!(
        exported.contains("github:fantoine/claude-later@main"),
        "github source must roundtrip verbatim, got:\n{exported}"
    );
    assert!(
        !exported.contains("local-sources"),
        "github deps must not trigger local-sources vendoring, got:\n{exported}"
    );

    // And the zip must not contain a local-sources/ tree for the github dep.
    let file = std::fs::File::open(&out).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).unwrap();
        assert!(
            !entry.name().starts_with("local-sources/"),
            "zip should not contain local-sources/ for github deps, saw: {}",
            entry.name()
        );
    }
}

/// Step 8: marketplace sources are CLI-managed — the exported Agentfile must
/// keep the canonical `source = "marketplace:org/repo@plugin"` verbatim so
/// that installing from the unzipped directory re-invokes the CLI's plugin
/// install path (which is idempotent per the Task 12 fix).
#[test]
fn step8_export_preserves_marketplace_source_verbatim() {
    let dir = tempdir().unwrap();
    let agentfile = "[agix]\ncli = [\"claude\"]\n\n\
         [dependencies]\n\
         mkt-pkg = { source = \"marketplace:fantoine/claude-plugins@roundtable\" }\n";
    std::fs::write(dir.path().join("Agentfile"), agentfile).unwrap();

    let out = dir.path().join("state.zip");
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .args(["export", "--output"])
        .arg(&out)
        .assert()
        .success();

    let exported = read_agentfile_from_zip(&out);
    assert!(
        exported.contains("marketplace:fantoine/claude-plugins@roundtable"),
        "marketplace source must roundtrip verbatim, got:\n{exported}"
    );
    assert!(
        !exported.contains("local-sources"),
        "marketplace deps must not trigger local-sources vendoring, got:\n{exported}"
    );
}
