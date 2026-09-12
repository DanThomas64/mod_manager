use crate::{marker, timestamp};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const BACKUPS_DIRNAME: &str = "_installer_backups";
const BACKUP_INFO_FILENAME: &str = ".backup-info";

pub struct SubfolderStat {
    pub name: String,
    pub file_count: usize,
    pub symlink_count: usize,
}

pub struct BackupInfo {
    pub backed_up_at: String,
    pub previous_modpack_version: Option<String>,
    pub previous_bepinex_version: Option<String>,
    pub subfolders: Vec<SubfolderStat>,
}

impl BackupInfo {
    pub fn total_files(&self) -> usize {
        self.subfolders.iter().map(|s| s.file_count).sum()
    }

    pub fn total_symlinks(&self) -> usize {
        self.subfolders.iter().map(|s| s.symlink_count).sum()
    }
}

pub struct BackupOutcome {
    pub backed_up: bool,
    pub backup_path: Option<PathBuf>,
    pub info: Option<BackupInfo>,
}

fn backups_dir(bepinex_dir: &Path) -> PathBuf {
    bepinex_dir.join(BACKUPS_DIRNAME)
}

/// Before overwriting anything, move aside every top-level `BepInEx/<name>`
/// folder that both (a) exists in `dest_root`'s current install and (b) is
/// about to be written to by `incoming_root` (an extracted mod payload, or
/// another backup's snapshot being restored — both are shaped as
/// `<root>/BepInEx/<name>/...`). This generically covers core/, config/,
/// plugins/, and anything else a release happens to touch — not just
/// plugins — so any config the installer changes gets captured too.
///
/// Moves (not copies) preserve symlinks exactly as they were, so a
/// Vortex-style symlinked deployment backs up and restores intact instead
/// of being flattened into plain file copies.
pub fn snapshot_before_overwrite(dest_root: &Path, incoming_root: &Path) -> Result<BackupOutcome> {
    let dest_bepinex = dest_root.join("BepInEx");
    let incoming_bepinex = incoming_root.join("BepInEx");

    let names = incoming_subfolder_names(&incoming_bepinex)
        .into_iter()
        .filter(|name| dest_bepinex.join(name).is_dir())
        .collect::<Vec<_>>();
    if names.is_empty() {
        return Ok(BackupOutcome {
            backed_up: false,
            backup_path: None,
            info: None,
        });
    }

    let previous_marker = marker::read_marker(&dest_bepinex);
    let container = backups_dir(&dest_bepinex).join(timestamp::now_stamp());
    std::fs::create_dir_all(container.join("BepInEx")).context("creating backup directory")?;

    let mut subfolders = Vec::new();
    for name in names {
        let src = dest_bepinex.join(&name);
        let (file_count, symlink_count) = count_files_and_symlinks(&src);
        let dst = container.join("BepInEx").join(&name);
        move_dir_preserving_symlinks(&src, &dst)
            .with_context(|| format!("backing up BepInEx/{name}"))?;
        subfolders.push(SubfolderStat {
            name,
            file_count,
            symlink_count,
        });
    }

    let info = BackupInfo {
        backed_up_at: timestamp::now_stamp(),
        previous_modpack_version: previous_marker.as_ref().map(|m| m.modpack_version.clone()),
        previous_bepinex_version: previous_marker.as_ref().map(|m| m.bepinex_version.clone()),
        subfolders,
    };
    write_backup_info(&container, &info)?;

    Ok(BackupOutcome {
        backed_up: true,
        backup_path: Some(container),
        info: Some(info),
    })
}

/// List existing backups, newest first, each with whatever info was
/// recorded about it at backup time.
pub fn list_backups(dest_root: &Path) -> Vec<(PathBuf, Option<BackupInfo>)> {
    let dir = backups_dir(&dest_root.join("BepInEx"));
    let mut out: Vec<(PathBuf, Option<BackupInfo>)> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| {
            let path = e.path();
            let info = read_backup_info(&path);
            (path, info)
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}

/// Restore every subfolder captured in `backup_container` (e.g. core/,
/// config/, plugins/ — whatever that snapshot holds) as the active one.
/// Whatever is in place at restore time is itself snapshotted first via
/// `snapshot_before_overwrite`, so restoring can never lose data either —
/// it's symmetric with a normal install.
pub fn restore_backup(dest_root: &Path, backup_container: &Path) -> Result<BackupOutcome> {
    let pre_restore = snapshot_before_overwrite(dest_root, backup_container)?;
    let dest_bepinex = dest_root.join("BepInEx");
    for name in incoming_subfolder_names(&backup_container.join("BepInEx")) {
        move_dir_preserving_symlinks(
            &backup_container.join("BepInEx").join(&name),
            &dest_bepinex.join(&name),
        )
        .with_context(|| format!("restoring BepInEx/{name}"))?;
    }
    Ok(pre_restore)
}

fn incoming_subfolder_names(bepinex_dir: &Path) -> Vec<String> {
    std::fs::read_dir(bepinex_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect()
}

fn move_dir_preserving_symlinks(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    // Cross-filesystem rename isn't allowed; fall back to a symlink-aware copy.
    copy_dir_recursive_preserving_symlinks(src, dst)?;
    std::fs::remove_dir_all(src)?;
    Ok(())
}

fn copy_dir_recursive_preserving_symlinks(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_symlink() {
            let target = std::fs::read_link(&src_path)?;
            create_symlink(&target, &dst_path)
                .with_context(|| format!("recreating symlink {}", dst_path.display()))?;
        } else if file_type.is_dir() {
            copy_dir_recursive_preserving_symlinks(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(windows)]
fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    let target_is_dir = std::fs::metadata(target).map(|m| m.is_dir()).unwrap_or(false);
    if target_is_dir {
        std::os::windows::fs::symlink_dir(target, link)?;
    } else {
        std::os::windows::fs::symlink_file(target, link)?;
    }
    Ok(())
}

/// Count files under `dir`, without following symlinks (a symlink counts as
/// one entry, not as whatever it points to) — used purely for the summary
/// shown to the user, so they can see e.g. "87 of these were symlinks
/// (Vortex-style deployment)".
fn count_files_and_symlinks(dir: &Path) -> (usize, usize) {
    let mut files = 0;
    let mut symlinks = 0;
    for entry in std::fs::read_dir(dir).into_iter().flatten().filter_map(|e| e.ok()) {
        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_symlink() {
            files += 1;
            symlinks += 1;
        } else if file_type.is_dir() {
            let (f, s) = count_files_and_symlinks(&entry.path());
            files += f;
            symlinks += s;
        } else {
            files += 1;
        }
    }
    (files, symlinks)
}

fn write_backup_info(container: &Path, info: &BackupInfo) -> Result<()> {
    let mut text = format!(
        "backed_up_at={}\nprevious_modpack_version={}\nprevious_bepinex_version={}\nsubfolders={}\n",
        info.backed_up_at,
        info.previous_modpack_version.as_deref().unwrap_or("unknown"),
        info.previous_bepinex_version.as_deref().unwrap_or("unknown"),
        info.subfolders
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(","),
    );
    for s in &info.subfolders {
        text.push_str(&format!("{}.file_count={}\n", s.name, s.file_count));
        text.push_str(&format!("{}.symlink_count={}\n", s.name, s.symlink_count));
    }
    std::fs::write(container.join(BACKUP_INFO_FILENAME), text)?;
    Ok(())
}

fn read_backup_info(container: &Path) -> Option<BackupInfo> {
    let text = std::fs::read_to_string(container.join(BACKUP_INFO_FILENAME)).ok()?;
    let mut backed_up_at = String::new();
    let mut previous_modpack_version = None;
    let mut previous_bepinex_version = None;
    let mut subfolder_names: Vec<String> = Vec::new();
    let mut file_counts = std::collections::HashMap::new();
    let mut symlink_counts = std::collections::HashMap::new();

    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        match key.trim() {
            "backed_up_at" => backed_up_at = value.to_string(),
            "previous_modpack_version" if value != "unknown" => {
                previous_modpack_version = Some(value.to_string())
            }
            "previous_bepinex_version" if value != "unknown" => {
                previous_bepinex_version = Some(value.to_string())
            }
            "subfolders" => {
                subfolder_names = value.split(',').filter(|s| !s.is_empty()).map(String::from).collect()
            }
            key if key.ends_with(".file_count") => {
                let name = key.trim_end_matches(".file_count").to_string();
                file_counts.insert(name, value.parse().unwrap_or(0));
            }
            key if key.ends_with(".symlink_count") => {
                let name = key.trim_end_matches(".symlink_count").to_string();
                symlink_counts.insert(name, value.parse().unwrap_or(0));
            }
            _ => {}
        }
    }

    let subfolders = subfolder_names
        .into_iter()
        .map(|name| SubfolderStat {
            file_count: file_counts.get(&name).copied().unwrap_or(0),
            symlink_count: symlink_counts.get(&name).copied().unwrap_or(0),
            name,
        })
        .collect();

    Some(BackupInfo {
        backed_up_at,
        previous_modpack_version,
        previous_bepinex_version,
        subfolders,
    })
}
