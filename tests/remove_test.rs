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
cli = ["claude"]

[claude.dependencies]
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

#[tokio::test]
async fn remove_with_cli_filter_only_removes_from_that_section() {
    let dir = tempdir().unwrap();
    let pkg_dir = tempdir().unwrap();
    let skills = pkg_dir.path().join("skills");
    std::fs::create_dir(&skills).unwrap();
    std::fs::write(skills.join("s.md"), "# s").unwrap();

    let manifest = format!(
        r#"
[agix]
cli = ["claude", "codex"]

[claude.dependencies]
my-pkg = {{ source = "local:{0}" }}

[codex.dependencies]
my-pkg = {{ source = "local:{0}" }}
"#,
        pkg_dir.path().display()
    );
    std::fs::write(dir.path().join("Agentfile"), &manifest).unwrap();

    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .arg("install")
        .assert()
        .success();

    // Remove only from claude section
    Command::cargo_bin("agix")
        .unwrap()
        .current_dir(dir.path())
        .arg("remove")
        .arg("my-pkg")
        .arg("--cli")
        .arg("claude")
        .assert()
        .success();

    let content = std::fs::read_to_string(dir.path().join("Agentfile")).unwrap();
    // Should still be present in codex section
    assert!(content.contains("codex"));
}
