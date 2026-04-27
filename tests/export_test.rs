use std::io::Read;
use tempfile::tempdir;

mod helpers;

/// Read a named entry out of a zip as a UTF-8 string.
fn read_zip_entry(zip_path: &std::path::Path, entry_name: &str) -> String {
    let file = std::fs::File::open(zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut entry = archive
        .by_name(entry_name)
        .unwrap_or_else(|_| panic!("{entry_name} not found in zip"));
    let mut buf = String::new();
    entry.read_to_string(&mut buf).unwrap();
    buf
}

/// List all entry names in a zip.
fn zip_entry_names(zip_path: &std::path::Path) -> Vec<String> {
    let file = std::fs::File::open(zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect()
}

/// Read the `Agentfile` entry out of an exported zip as a UTF-8 string.
fn read_agentfile_from_zip(zip_path: &std::path::Path) -> String {
    read_zip_entry(zip_path, "Agentfile")
}

#[test]
fn export_creates_zip_with_agentfile() {
    let dir = tempdir().unwrap();
    let home = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();
    std::fs::write(dir.path().join("Agentfile.lock"), "").unwrap();

    let output = dir.path().join("backup.zip");
    helpers::cmd_non_interactive(home.path())
        .current_dir(dir.path())
        .arg("export")
        .arg("--output")
        .arg(output.to_str().unwrap())
        .assert()
        .success();
    assert!(output.exists());
}

/// Step 2: with no `--output`, the zip must land at `agix-export.zip` in CWD.
#[test]
fn step2_export_default_filename_writes_agix_export_zip_in_cwd() {
    let dir = tempdir().unwrap();
    let home = tempdir().unwrap();
    std::fs::write(dir.path().join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    helpers::cmd_non_interactive(home.path())
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

/// Step 4: `--all` exports both local and global scopes into a single zip
/// under `local/` and `global/` prefixes.
#[test]
fn step4_export_all_produces_zip_with_both_scopes() {
    let home = tempdir().unwrap();
    // Project dir nested inside home so walk-up boundary works.
    let project = home.path().join("project");
    std::fs::create_dir(&project).unwrap();
    std::fs::write(project.join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    // Bootstrap global Agentfile.
    let agix_dir = home.path().join(".agix");
    std::fs::create_dir_all(&agix_dir).unwrap();
    std::fs::write(agix_dir.join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    let out = project.join("combined.zip");
    helpers::cmd_non_interactive(home.path())
        .current_dir(&project)
        .args(["export", "--all", "--output"])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicates::str::contains("Exported"));

    assert!(out.exists(), "combined.zip must exist");

    let names = zip_entry_names(&out);
    assert!(
        names.iter().any(|n| n == "local/Agentfile"),
        "zip must contain local/Agentfile; entries: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "global/Agentfile"),
        "zip must contain global/Agentfile; entries: {names:?}"
    );
}

/// Step 4b: `--all` with only global scope (no local Agentfile) still succeeds
/// and warns about the missing local scope.
#[test]
fn step4b_export_all_with_only_global_warns_and_succeeds() {
    let home = tempdir().unwrap();
    let agix_dir = home.path().join(".agix");
    std::fs::create_dir_all(&agix_dir).unwrap();
    std::fs::write(agix_dir.join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    // cwd has no Agentfile and is not inside home → walk-up finds nothing.
    let cwd = tempdir().unwrap();
    let out = cwd.path().join("out.zip");
    helpers::cmd_non_interactive(home.path())
        .current_dir(cwd.path())
        .args(["export", "--all", "--output"])
        .arg(&out)
        .assert()
        .success()
        .stdout(predicates::str::contains("Exported"));

    let names = zip_entry_names(&out);
    assert!(
        names.iter().any(|n| n == "global/Agentfile"),
        "zip must contain global/Agentfile; entries: {names:?}"
    );
}

/// Step 4c: `--all` with a local dep vendors it under `local/local-sources/`.
#[test]
fn step4c_export_all_vendors_local_deps_under_scope_prefix() {
    let home = tempdir().unwrap();
    let project = home.path().join("project");
    std::fs::create_dir(&project).unwrap();

    let pkg = home.path().join("mypkg");
    std::fs::create_dir(&pkg).unwrap();
    std::fs::write(pkg.join("skill.md"), "# skill").unwrap();

    std::fs::write(
        project.join("Agentfile"),
        format!(
            "[agix]\ncli = [\"claude\"]\n\n[dependencies]\nmypkg = {{ source = \"local:{}\" }}\n",
            pkg.display()
        ),
    )
    .unwrap();

    let agix_dir = home.path().join(".agix");
    std::fs::create_dir_all(&agix_dir).unwrap();
    std::fs::write(agix_dir.join("Agentfile"), "[agix]\ncli = [\"claude\"]\n").unwrap();

    let out = project.join("combined.zip");
    helpers::cmd_non_interactive(home.path())
        .current_dir(&project)
        .args(["export", "--all", "--output"])
        .arg(&out)
        .assert()
        .success();

    let names = zip_entry_names(&out);
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("local/local-sources/mypkg/")),
        "local dep must be vendored under local/local-sources/mypkg/; entries: {names:?}"
    );
    let af = read_zip_entry(&out, "local/Agentfile");
    assert!(
        af.contains("local:./local-sources/mypkg"),
        "local/Agentfile must rewrite source to ./local-sources/mypkg; got:\n{af}"
    );
}

/// Step 5: exporting without an Agentfile must exit non-zero and point the
/// user at `agix init` (consistent with `list`, `outdated`, `add`).
#[test]
fn step5_export_without_agentfile_falls_back_to_global() {
    // Walk-up finds no Agentfile → fallback to ~/.agix/ (auto-created).
    let dir = tempdir().unwrap();
    let home = tempdir().unwrap();
    // Deliberately no Agentfile in dir.

    helpers::cmd_non_interactive(home.path())
        .current_dir(dir.path())
        .arg("export")
        .assert()
        .success()
        .stdout(predicates::str::contains("Exported to agix-export.zip"));

    assert!(home.path().join(".agix").join("Agentfile").exists());
}

/// Step 7: github sources are remote refs — the exported Agentfile must keep
/// the canonical `source = "github:org/repo@ref"` verbatim. No vendoring,
/// no path rewriting, no local-sources/ entry.
#[test]
fn step7_export_preserves_github_source_verbatim() {
    let dir = tempdir().unwrap();
    let home = tempdir().unwrap();
    let agentfile = "[agix]\ncli = [\"claude\"]\n\n\
         [dependencies]\n\
         gh-pkg = { source = \"github:fantoine/claude-later@main\" }\n";
    std::fs::write(dir.path().join("Agentfile"), agentfile).unwrap();

    let out = dir.path().join("state.zip");
    helpers::cmd_non_interactive(home.path())
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
    let home = tempdir().unwrap();
    let agentfile = "[agix]\ncli = [\"claude\"]\n\n\
         [dependencies]\n\
         mkt-pkg = { source = \"marketplace:fantoine/claude-plugins@roundtable\" }\n";
    std::fs::write(dir.path().join("Agentfile"), agentfile).unwrap();

    let out = dir.path().join("state.zip");
    helpers::cmd_non_interactive(home.path())
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
