pub mod backup;
pub mod detect;
pub mod extract;
pub mod install;
pub mod marker;
pub mod timestamp;

use anyhow::{Context, Result};
use std::io::{self, Write};

const FOOTER_MAGIC: &[u8] = b"VMODPACK1";

/// Entry point shared by both `installer-shell` binaries. Finds (or asks
/// for) the Valheim install directory, offers to restore a previous backup
/// if any exist, and otherwise backs up the current plugins folder and
/// installs the payload embedded after this executable's own bytes.
pub fn run() -> Result<()> {
    print_banner();

    let root = match detect::find_valheim_install() {
        Some(root) => {
            println!("Found Valheim install at: {}", root.display());
            root
        }
        None => {
            println!("Could not auto-detect your Valheim install.");
            prompt_for_path()?
        }
    };

    let backups = backup::list_backups(&root);
    if !backups.is_empty() {
        if let Some(chosen) = offer_restore_menu(&backups)? {
            let (path, _) = &backups[chosen];
            let outcome = backup::restore_backup(&root, path)?;
            println!("\nRestored backup: {}", path.display());
            print_backup_outcome(&outcome, "What was in place just before this restore");
            pause_before_exit();
            return Ok(());
        }
    }

    let own_payload = extract::read_own_payload().context("reading embedded mod payload")?;
    print_changelog(&own_payload.changelog);
    if !confirm_install()? {
        println!("\nCancelled — nothing was changed.");
        pause_before_exit();
        return Ok(());
    }

    let temp_dir = std::env::temp_dir().join(format!("valheim-mod-installer-{}", std::process::id()));
    extract::unpack_payload(&own_payload.payload, &temp_dir).context("unpacking mod payload")?;

    let backup_outcome =
        backup::snapshot_before_overwrite(&root, &temp_dir).context("backing up files before install")?;
    print_backup_outcome(&backup_outcome, "Your previous install");

    install::install_mods(&temp_dir, &root).context("copying mods into Valheim install")?;
    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("\nDone! Mods installed to: {}", root.join("BepInEx").display());
    println!("Launch Valheim normally through Steam.");
    println!(
        "\nReminder: backups live under {} and are kept forever — this tool never deletes them. \
         Clean them up yourself whenever you're confident you don't need them.",
        root.join("BepInEx").join("_installer_backups").display()
    );
    pause_before_exit();
    Ok(())
}

fn print_banner() {
    println!("Valheim Mod Installer");
    println!("======================");
    println!("This installs BepInEx and the configured mods into your Valheim install.");
    println!("Make sure Valheim is already installed via Steam and has been run at least once.");
    println!(
        "Before changing anything, every folder about to be overwritten (core, config, plugins — \
         whatever this release touches) is backed up first, symlinks and all (so a Vortex-style \
         symlinked mod setup restores exactly as it was). Backups are never deleted automatically \
         — that's on you to clean up once you're confident you don't need them.\n"
    );
}

/// Show the changelog entry for the version embedded in this installer, so
/// the user knows what they're about to install before confirming.
fn print_changelog(changelog: &str) {
    let changelog = changelog.trim();
    if changelog.is_empty() {
        return;
    }
    println!("--- What's in this version ---");
    println!("{changelog}");
    println!("-------------------------------");
}

/// Ask the user to confirm before making any changes. Pressing Enter with no
/// input confirms (keeps the common case single-click); anything else
/// starting with 'n' cancels. Stdin closing (0 bytes read, EOF) is treated
/// as a cancel, not a confirm — an empty line from a real Enter keypress
/// still reads as "\n" (1 byte), so this only catches a genuinely absent
/// answer.
fn confirm_install() -> Result<bool> {
    print!("Continue with install? [Y/n] ");
    io::stdout().flush().ok();
    let mut line = String::new();
    let bytes_read = io::stdin().read_line(&mut line).context("reading install confirmation")?;
    if bytes_read == 0 {
        return Ok(false);
    }
    let trimmed = line.trim().to_lowercase();
    Ok(trimmed.is_empty() || trimmed == "y" || trimmed == "yes")
}

fn print_backup_outcome(outcome: &backup::BackupOutcome, subject: &str) {
    let (Some(path), Some(info)) = (&outcome.backup_path, &outcome.info) else {
        println!("No previous install found — nothing to back up.");
        return;
    };
    match (&info.previous_modpack_version, &info.previous_bepinex_version) {
        (Some(modpack_version), Some(bepinex_version)) => {
            println!(
                "{subject} was installed by this tool: modpack v{modpack_version} (BepInEx v{bepinex_version})."
            );
        }
        _ => {
            println!("{subject} wasn't installed by this tool (manual install or another mod manager?).");
        }
    }
    let folder_list = info
        .subfolders
        .iter()
        .map(|s| format!("BepInEx/{} ({} file(s))", s.name, s.file_count))
        .collect::<Vec<_>>()
        .join(", ");
    println!("Backed up: {folder_list}");
    if info.total_symlinks() > 0 {
        println!(
            "{} of those were symlinks (e.g. Vortex-style deployment) — preserved as symlinks in the backup.",
            info.total_symlinks()
        );
    }
    println!("Saved to:\n  {}", path.display());
}

/// If backups exist, list them and let the user pick one to restore instead
/// of installing. Pressing Enter with no input proceeds to the normal
/// install/update path (keeps the common case single-click).
fn offer_restore_menu(backups: &[(std::path::PathBuf, Option<backup::BackupInfo>)]) -> Result<Option<usize>> {
    println!("Existing backups found:");
    for (i, (path, info)) in backups.iter().enumerate() {
        let label = match info {
            Some(info) => {
                let origin = match (&info.previous_modpack_version, &info.previous_bepinex_version) {
                    (Some(mv), Some(bv)) => format!("modpack v{mv} (BepInEx v{bv})"),
                    _ => "not installed by this tool".to_string(),
                };
                let folders = info
                    .subfolders
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let symlink_note = if info.total_symlinks() > 0 {
                    format!(", {} symlinked", info.total_symlinks())
                } else {
                    String::new()
                };
                format!("{origin}, includes [{folders}], {} file(s){symlink_note}", info.total_files())
            }
            None => "no info recorded".to_string(),
        };
        let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
        println!("  {}) {name} — {label}", i + 1);
    }
    println!(
        "Press Enter to install/update normally, or type a number above to restore that backup instead."
    );
    print!("> ");
    io::stdout().flush().ok();

    let mut line = String::new();
    io::stdin().read_line(&mut line).context("reading restore choice")?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    match trimmed.parse::<usize>() {
        Ok(n) if n >= 1 && n <= backups.len() => Ok(Some(n - 1)),
        _ => {
            println!("Unrecognized choice — proceeding with normal install.");
            Ok(None)
        }
    }
}

fn prompt_for_path() -> Result<std::path::PathBuf> {
    loop {
        print!(
            "Please paste the full path to your Valheim install \
             (the folder containing valheim.exe / valheim.x86_64): "
        );
        io::stdout().flush().ok();
        let mut line = String::new();
        let bytes_read = io::stdin().read_line(&mut line).context("reading input path")?;
        if bytes_read == 0 {
            anyhow::bail!("no input received (stdin closed) while waiting for a Valheim install path");
        }
        let candidate = std::path::PathBuf::from(line.trim());
        if detect::is_valid_valheim_root(&candidate, |p| p.exists()) {
            return Ok(candidate);
        }
        println!(
            "'{}' doesn't look like a Valheim install (no valheim.exe / valheim.x86_64 found). Try again.",
            candidate.display()
        );
    }
}

fn pause_before_exit() {
    print!("Press Enter to exit...");
    io::stdout().flush().ok();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}
