use agix::manifest::agentfile::{PackageManifest, ProjectManifest};

#[test]
fn parse_project_manifest() {
    let toml = r#"
[agix]
cli = ["claude-code", "codex"]

[dependencies]
rtk = { source = "github:org/rtk", exclude = ["codex"] }

[claude-code.dependencies]
superpowers = { source = "github:claude-plugins-official/superpowers", version = "^2.0" }
"#;
    let manifest: ProjectManifest = toml::from_str(toml).unwrap();
    assert_eq!(manifest.agix.cli, vec!["claude-code", "codex"]);
    assert!(manifest.dependencies.contains_key("rtk"));
    let rtk = &manifest.dependencies["rtk"];
    assert_eq!(rtk.source, "github:org/rtk");
    assert_eq!(rtk.exclude, Some(vec!["codex".to_string()]));
    let claude_deps = manifest.cli_dependencies.get("claude-code").unwrap();
    assert!(claude_deps.contains_key("superpowers"));
}

#[test]
fn parse_package_manifest() {
    let toml = r#"
[agix]
name = "superpowers"
version = "2.1.0"
description = "Supercharge your Claude Code workflow"
cli = ["claude-code"]

[hooks]
post-install = "scripts/setup.sh"
pre-uninstall = "scripts/cleanup.sh"

[dependencies]
dep-a = { source = "github:org/dep-a" }
"#;
    let manifest: PackageManifest = toml::from_str(toml).unwrap();
    assert_eq!(manifest.agix.name.unwrap(), "superpowers");
    assert_eq!(manifest.agix.version.unwrap(), "2.1.0");
    assert_eq!(
        manifest.hooks.unwrap().post_install.unwrap(),
        "scripts/setup.sh"
    );
}

#[test]
fn parse_minimal_project_manifest() {
    let toml = r#"
[agix]
cli = ["claude-code"]
"#;
    let manifest: ProjectManifest = toml::from_str(toml).unwrap();
    assert_eq!(manifest.agix.cli, vec!["claude-code"]);
    assert!(manifest.dependencies.is_empty());
}
