use agix::core::lock::{InstalledFile, LockFile, LockedPackage};
use tempfile::tempdir;

#[test]
fn roundtrip_lock_file() {
    let dir = tempdir().unwrap();
    let lock_path = dir.path().join("Agentfile.lock");

    let mut lock = LockFile::default();
    lock.packages.push(LockedPackage {
        name: "superpowers".to_string(),
        source: "github:claude-plugins-official/superpowers".to_string(),
        sha: Some("a3f8c12d".to_string()),
        content_hash: None,
        version: None,
        cli: vec!["claude".to_string()],
        scope: "global".to_string(),
        files: vec![InstalledFile {
            dest: "~/.claude/skills/brainstorming".to_string(),
        }],
    });

    lock.to_file(&lock_path).unwrap();
    let loaded = LockFile::from_file(&lock_path).unwrap();

    assert_eq!(loaded.packages.len(), 1);
    assert_eq!(loaded.packages[0].name, "superpowers");
    assert_eq!(loaded.packages[0].sha.as_deref(), Some("a3f8c12d"));
    assert_eq!(loaded.packages[0].files.len(), 1);
}

#[test]
fn lock_file_find_package() {
    let mut lock = LockFile::default();
    lock.packages.push(LockedPackage {
        name: "rtk".to_string(),
        source: "github:org/rtk".to_string(),
        sha: Some("abc123".to_string()),
        content_hash: None,
        version: None,
        cli: vec!["claude".to_string()],
        scope: "local".to_string(),
        files: vec![],
    });

    assert!(lock.find("rtk").is_some());
    assert!(lock.find("missing").is_none());
}

#[test]
fn lock_file_missing_returns_default() {
    let dir = tempdir().unwrap();
    let lock_path = dir.path().join("Agentfile.lock");
    let lock = LockFile::from_file_or_default(&lock_path);
    assert!(lock.packages.is_empty());
}
