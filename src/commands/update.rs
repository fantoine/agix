pub async fn run(name: Option<String>, global: bool) -> anyhow::Result<()> {
    let (agentfile_path, lock_path, scope) = super::agentfile_paths(global)?;
    if !agentfile_path.exists() {
        anyhow::bail!("No Agentfile found.");
    }
    let manifest = crate::manifest::agentfile::ProjectManifest::from_file(&agentfile_path)?;

    if let Some(pkg_name) = name {
        crate::core::installer::Installer::uninstall(&pkg_name, &lock_path)?;
    } else {
        let lock = crate::core::lock::LockFile::from_file_or_default(&lock_path);
        let names: Vec<String> = lock.packages.iter().map(|p| p.name.clone()).collect();
        for pkg_name in names {
            crate::core::installer::Installer::uninstall(&pkg_name, &lock_path)?;
        }
    }

    crate::core::installer::Installer::install_manifest(&manifest, &lock_path, scope).await?;
    crate::output::success("Updated");
    Ok(())
}
