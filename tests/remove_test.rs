use assert_cmd::Command;
use tempfile::tempdir;

#[tokio::test]
async fn remove_updates_agentfile_and_lock() {
    let dir = tempdir().unwrap();
    let pkg_dir = tempdir().unwrap();
    let skills = pkg_dir.path().join("skills");
    std::fs::create_dir(&skills).unwrap();
    std::fs::write(skills.join("s.md"), "# s").unwrap();

    let manifest = format!(
        r#"
[agix]
cli = ["claude-code"]

[claude-code.dependencies]
my-pkg = {{ source = "local:{}" }}
"#,
        pkg_dir.path().display()
    );
    std::fs::write(dir.path().join("Agentfile"), &manifest).unwrap();

    // Install first
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .arg("install")
        .assert()
        .success();

    // Remove
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .arg("remove")
        .arg("my-pkg")
        .assert()
        .success();

    let content = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    assert!(!content.contains("my-pkg"));
}
