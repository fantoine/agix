use assert_cmd::Command;
use tempfile::tempdir;

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
