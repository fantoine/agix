use agix::core::installer::Installer;
use agix::core::lock::LockFile;
use agix::drivers::Scope;
use agix::manifest::agentfile::ProjectManifest;
use tempfile::tempdir;

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

    let mut cmd = assert_cmd::Command::cargo_bin("agix").unwrap();
    cmd.current_dir(dir.path()).arg("install");
    cmd.assert().success();
    assert!(dir.path().join("Agentfile.lock").exists());
}
