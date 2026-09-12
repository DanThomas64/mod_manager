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
