use anyhow::Result;
use std::io::Write;
use std::path::Path;

pub async fn run(scope: &str, all: bool, output: Option<String>) -> Result<()> {
    let output_path = output.as_deref().unwrap_or("agix-export.zip");
    let file = std::fs::File::create(output_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::FileOptions<()> = zip::write::FileOptions::default();

    if all || scope == "local" {
        let dir = std::env::current_dir()?;
        if dir.join("Agentfile").exists() {
            add_file_to_zip(&mut zip, &dir.join("Agentfile"), "Agentfile", options)?;
        }
        if dir.join("Agentfile.lock").exists() {
            add_file_to_zip(
                &mut zip,
                &dir.join("Agentfile.lock"),
                "Agentfile.lock",
                options,
            )?;
        }
        add_local_sources_to_zip(&mut zip, &dir, options)?;
    }

    if all || scope == "global" {
        if let Some(home) = dirs::home_dir() {
            let agix_dir = home.join(".agix");
            if agix_dir.join("Agentfile").exists() {
                add_file_to_zip(
                    &mut zip,
                    &agix_dir.join("Agentfile"),
                    "global/Agentfile",
                    options,
                )?;
            }
        }
    }

    zip.finish()?;
    crate::output::success(&format!("Exported to {output_path}"));
    Ok(())
}

fn add_file_to_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    path: &Path,
    name: &str,
    options: zip::write::FileOptions<()>,
) -> anyhow::Result<()> {
    zip.start_file(name, options)?;
    let content = std::fs::read(path)?;
    zip.write_all(&content)?;
    Ok(())
}

fn add_local_sources_to_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    project_dir: &Path,
    options: zip::write::FileOptions<()>,
) -> anyhow::Result<()> {
    let lock =
        crate::core::lock::LockFile::from_file_or_default(&project_dir.join("Agentfile.lock"));
    for pkg in &lock.packages {
        if let Ok(crate::sources::SourceSpec::Local { path }) =
            crate::sources::SourceSpec::parse(&pkg.source)
        {
            let resolved = if path.is_absolute() {
                path
            } else {
                project_dir.join(&path)
            };
            if resolved.exists() {
                for entry in walkdir::WalkDir::new(&resolved) {
                    let entry = entry?;
                    if entry.file_type().is_file() {
                        let rel = entry.path().strip_prefix(&resolved).unwrap();
                        let zip_name = format!("local-sources/{}/{}", pkg.name, rel.display());
                        zip.start_file(&zip_name, options)?;
                        let content = std::fs::read(entry.path())?;
                        zip.write_all(&content)?;
                    }
                }
            }
        }
    }
    Ok(())
}
