/// Project packager — creates an in-memory tar.gz of the current project
/// for upload to the Zenith cloud service.

use anyhow::{Context, Result};
use std::path::Path;

/// Package the project directory into a gzip-compressed tar archive.
/// Excludes: .git, target, .zenith, node_modules, .venv, __pycache__
pub fn package_project(dir: &Path) -> Result<Vec<u8>> {
    use flate2::{write::GzEncoder, Compression};
    use tar::Builder;

    let skip = [".git", "target", ".zenith", "node_modules", ".venv", "__pycache__"];

    let enc = GzEncoder::new(Vec::new(), Compression::default());
    let mut ar = Builder::new(enc);

    for entry in collect_files(dir, &skip)? {
        let rel = entry.strip_prefix(dir)
            .with_context(|| format!("Failed to strip prefix from {:?}", entry))?;
        ar.append_path_with_name(&entry, rel)
            .with_context(|| format!("Failed to add {:?} to archive", entry))?;
    }

    let gz = ar.into_inner()
        .context("Failed to finalise tar archive")?;
    gz.finish().context("Failed to finalise gzip compression")
}

fn collect_files(dir: &Path, skip: &[&str]) -> Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if skip.iter().any(|s| *s == name) { continue; }
        let path = entry.path();
        if path.is_dir() {
            out.extend(collect_files(&path, skip)?);
        } else {
            out.push(path);
        }
    }
    Ok(out)
}
