use anyhow::{Context, Result};
use std::path::Path;

/// Merge the extracted payload root (BepInEx/ plus the loader files that
/// sit alongside it — winhttp.dll, doorstop_config.ini,
/// start_game_bepinex.sh, etc.) directly into the Valheim install directory,
/// overwriting same-named files in place. This is an install-or-upgrade
/// operation; no rollback is kept here (see `backup` for that).
pub fn install_mods(source: &Path, dest_root: &Path) -> Result<()> {
    copy_dir_recursive(source, dest_root)
        .with_context(|| format!("copying {} into {}", source.display(), dest_root.display()))?;
    ensure_launcher_scripts_executable(dest_root)?;
    Ok(())
}

/// Belt-and-suspenders: explicitly mark BepInEx's Linux launcher scripts
/// executable after install, regardless of what permission bits made it
/// through packaging/extraction — Valheim on Linux won't start under
/// BepInEx if `start_game_bepinex.sh` isn't runnable.
#[cfg(unix)]
fn ensure_launcher_scripts_executable(dest_root: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for name in ["start_game_bepinex.sh", "start_server_bepinex.sh"] {
        let path = dest_root.join(name);
        if !path.exists() {
            continue;
        }
        let mut perms = std::fs::metadata(&path)
            .with_context(|| format!("reading permissions of {name}"))?
            .permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(&path, perms).with_context(|| format!("marking {name} executable"))?;
    }
    Ok(())
}

#[cfg(windows)]
fn ensure_launcher_scripts_executable(_dest_root: &Path) -> Result<()> {
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .with_context(|| format!("copying {} -> {}", src_path.display(), dst_path.display()))?;
        }
    }
    Ok(())
}
