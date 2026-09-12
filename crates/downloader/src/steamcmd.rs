use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

/// Run steamcmd with the given script args (each `+something`), always
/// terminated with `+quit`. Streams steamcmd's own output through so the
/// maintainer sees real progress; errors clearly if steamcmd isn't on PATH.
fn run_steamcmd(login_args: &[String], script_args: &[String]) -> Result<()> {
    let mut cmd = Command::new("steamcmd");
    cmd.args(login_args);
    cmd.args(script_args);
    cmd.arg("+quit");
    let status = cmd.status().context(
        "running steamcmd (is it installed and on PATH? \
         https://developer.valvesoftware.com/wiki/SteamCMD)",
    )?;
    if !status.success() {
        bail!("steamcmd exited with status {status}");
    }
    Ok(())
}

fn login_args(username: Option<&str>, password: Option<&str>) -> Vec<String> {
    match (username, password) {
        (Some(u), Some(p)) => vec!["+login".to_string(), u.to_string(), p.to_string()],
        _ => vec!["+login".to_string(), "anonymous".to_string()],
    }
}

/// Install/update a dedicated server's own game files via steamcmd
/// (`+app_update <appid> validate`). This tool doesn't reimplement Steam's
/// download protocol — it drives Valve's own tool.
pub fn update_server(appid: u32, install_dir: &Path, username: Option<&str>, password: Option<&str>) -> Result<()> {
    std::fs::create_dir_all(install_dir)?;
    let install_dir_str = install_dir
        .to_str()
        .context("server install dir path is not valid UTF-8")?
        .to_string();
    run_steamcmd(
        &login_args(username, password),
        &[
            "+force_install_dir".to_string(),
            install_dir_str,
            "+app_update".to_string(),
            appid.to_string(),
            "validate".to_string(),
        ],
    )
}

/// Download a Steam Workshop item's content via steamcmd into `dest_dir`
/// (copied out of steamcmd's own scratch content cache afterward, so
/// callers get a stable, caller-chosen path).
pub fn download_workshop_item(appid: u32, workshop_id: u64, dest_dir: &Path) -> Result<()> {
    let scratch = std::env::temp_dir().join(format!("steamcmd-workshop-{appid}-{workshop_id}"));
    std::fs::create_dir_all(&scratch)?;
    let scratch_str = scratch.to_str().context("scratch path is not valid UTF-8")?.to_string();
    run_steamcmd(
        &login_args(None, None),
        &[
            "+force_install_dir".to_string(),
            scratch_str,
            "+workshop_download_item".to_string(),
            appid.to_string(),
            workshop_id.to_string(),
        ],
    )?;

    let content_dir = scratch
        .join("steamapps")
        .join("workshop")
        .join("content")
        .join(appid.to_string())
        .join(workshop_id.to_string());
    if !content_dir.is_dir() {
        anyhow::bail!("steamcmd reported success but {} wasn't created", content_dir.display());
    }
    if dest_dir.exists() {
        std::fs::remove_dir_all(dest_dir)?;
    }
    copy_dir_recursive(&content_dir, dest_dir)?;
    std::fs::remove_dir_all(&scratch).ok();
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
