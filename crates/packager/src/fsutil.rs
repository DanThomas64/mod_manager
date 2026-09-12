use anyhow::{Context, Result};
use std::fs::File;
use std::path::{Path, PathBuf};

/// Files commonly bundled in a mod package zip that aren't part of the
/// actual mod payload and shouldn't be copied into a loader's mods folder.
pub const NON_MOD_FILES: &[&str] = &["manifest.json", "readme.md", "icon.png", "changelog.md"];

pub fn unzip_to(zip_path: &Path, dest: &Path) -> Result<()> {
    let file = File::open(zip_path).with_context(|| format!("opening {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).with_context(|| format!("reading zip {}", zip_path.display()))?;
    std::fs::create_dir_all(dest)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let out_path = dest.join(relative);
        // Capture the zip's stored Unix permissions (e.g. the executable bit
        // on a launcher script) before `entry` is consumed by the copy below
        // — lost here, they'd stay lost through tar and into the installed
        // game folder.
        #[cfg(unix)]
        let unix_mode = entry.unix_mode();
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
            #[cfg(unix)]
            if let Some(mode) = unix_mode {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(mode))?;
            }
        }
    }
    Ok(())
}

/// Find the first directory named `name` (case-insensitive) anywhere under `root`.
pub fn find_dir_named(root: &Path, name: &str) -> Option<PathBuf> {
    if root.file_name().map(|n| n.eq_ignore_ascii_case(name)).unwrap_or(false) {
        return Some(root.to_path_buf());
    }
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.filter_map(|e| e.ok()) {
        if entry.file_type().ok()?.is_dir() {
            if let Some(found) = find_dir_named(&entry.path(), name) {
                return Some(found);
            }
        }
    }
    None
}

pub fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

pub fn copy_dir_contents_filtered(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_lossy = name.to_string_lossy().to_lowercase();
        if entry.file_type()?.is_file() && NON_MOD_FILES.contains(&name_lossy.as_str()) {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        if entry.file_type()?.is_dir() {
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Find the single cached entry for `id`@`version` under `cache_dir` — a
/// file for most sources, or a directory for a Steam Workshop item's raw
/// downloaded content.
pub fn find_cached_file(cache_dir: &Path, id: &str, version: &str) -> Result<PathBuf> {
    let dir = cache_dir.join(id).join(version);
    let entry = std::fs::read_dir(&dir)
        .with_context(|| format!("reading cache dir {}", dir.display()))?
        .filter_map(|e| e.ok())
        .next()
        .with_context(|| format!("no cached entry found in {}", dir.display()))?;
    Ok(entry.path())
}
