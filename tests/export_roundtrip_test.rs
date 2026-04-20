use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn export_zip_is_self_contained_and_installable() {
    // Setup: project with one local-source dep
    let src_dir = tempdir().unwrap();
    std::fs::create_dir(src_dir.path().join("skills")).unwrap();
    std::fs::write(src_dir.path().join("skills/s.md"), "# s").unwrap();

    let proj = tempdir().unwrap();
    std::fs::write(
        proj.path().join("Agentfile"),
        format!(
            "[agix]\ncli = [\"claude\"]\n\n[dependencies]\nmy-pkg = {{ source = \"local:{}\" }}\n",
            src_dir.path().display()
        ),
    )
    .unwrap();

    // 1. Install
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(proj.path())
        .args(["install"])
        .env("AGIX_NO_INTERACTIVE", "1")
        .assert()
        .success();

    // 2. Export
    let out = proj.path().join("state.zip");
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(proj.path())
        .args(["export", "--output"])
        .arg(&out)
        .env("AGIX_NO_INTERACTIVE", "1")
        .assert()
        .success();
    assert!(out.exists(), "zip should be created");

    // 3. Unzip to a fresh dir
    let target = tempdir().unwrap();
    let file = std::fs::File::open(&out).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    archive.extract(target.path()).unwrap();

    // 4. Agentfile inside the unzipped dir should reference local-sources/my-pkg (relative)
    let exported_agentfile = std::fs::read_to_string(target.path().join("Agentfile")).unwrap();
    assert!(
        exported_agentfile.contains("local:./local-sources/my-pkg")
            || exported_agentfile.contains("local:local-sources/my-pkg"),
        "Agentfile should reference local-sources/, got: {exported_agentfile}"
    );
    assert!(target
        .path()
        .join("local-sources/my-pkg/skills/s.md")
        .exists());

    // 5. Install in the unzipped dir should succeed
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(target.path())
        .args(["install"])
        .env("AGIX_NO_INTERACTIVE", "1")
        .assert()
        .success();
}
