pub mod backup;
pub mod detect;
pub mod extract;
pub mod game_metadata;
pub mod install;
pub mod marker;
pub mod timestamp;

use anyhow::{Context, Result};
use game_metadata::GameMetadata;
use std::io::{self, Write};
use std::path::PathBuf;

const FOOTER_MAGIC: &[u8] = b"VMODPACK1";

/// Entry point shared by both `installer-shell` binaries. Extracts the
/// payload embedded after this executable's own bytes (which also carries
/// the per-game metadata this run needs — this binary itself is built once
/// and reused across every game/release), finds (or asks for) the game's
/// install directory, offers to restore a previous backup if any exist, and
/// otherwise backs up whatever's about to be overwritten and installs.
pub fn run() -> Result<()> {
    let payload = extract::read_own_payload().context("reading embedded mod payload")?;
    let temp_dir = std::env::temp_dir().join(format!("mod-installer-{}", std::process::id()));
    extract::unpack_payload(&payload, &temp_dir).context("unpacking mod payload")?;
    let game = game_metadata::read_game_metadata(&temp_dir).context("reading game metadata from payload")?;
    let loader_root = game.loader_root();

    print_banner(&game);

    let root = match detect::find_game_install(&game) {
        Some(root) => {
            println!("Found {} install at: {}", game.game_name, root.display());
            root
        }
        None => {
            println!("Could not auto-detect your {} install.", game.game_name);
            prompt_for_path(&game)?
        }
    };

    let backups = backup::list_backups(&root, loader_root);
    if !backups.is_empty() {
        if let Some(chosen) = offer_restore_menu(&backups)? {
            let (path, _) = &backups[chosen];
            let outcome = backup::restore_backup(&root, path, loader_root)?;
            println!("\nRestored backup: {}", path.display());
            print_backup_outcome(&outcome, "What was in place just before this restore");
            pause_before_exit();
            return Ok(());
        }
    }

    let backup_outcome = backup::snapshot_before_overwrite(&root, &temp_dir, loader_root)
        .context("backing up files before install")?;
    print_backup_outcome(&backup_outcome, "Your previous install");

    install::install_mods(&temp_dir, &root).context("copying mods into the game install")?;
    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("\nDone! Mods installed to: {}", root.join(loader_root).display());
    println!("Launch {} normally through Steam.", game.game_name);
    println!(
        "\nReminder: backups live under {} and are kept forever — this tool never deletes them. \
         Clean them up yourself whenever you're confident you don't need them.",
        root.join(loader_root).join("_installer_backups").display()
    );
    pause_before_exit();
    Ok(())
}

fn print_banner(game: &GameMetadata) {
    println!("{} Mod Installer", game.game_name);
    println!("{}", "=".repeat(game.game_name.len() + 14));
    println!("This installs the loader and configured mods into your {} install.", game.game_name);
    println!("Make sure {} is already installed via Steam and has been run at least once.", game.game_name);
    println!(
        "Before changing anything, every folder about to be overwritten (core, config, plugins — \
         whatever this release touches) is backed up first, symlinks and all (so a Vortex-style \
         symlinked mod setup restores exactly as it was). Backups are never deleted automatically \
         — that's on you to clean up once you're confident you don't need them.\n"
    );
}

fn print_backup_outcome(outcome: &backup::BackupOutcome, subject: &str) {
    let (Some(path), Some(info)) = (&outcome.backup_path, &outcome.info) else {
        println!("No previous install found — nothing to back up.");
        return;
    };
    match (&info.previous_modpack_version, &info.previous_loader_version) {
        (Some(modpack_version), Some(loader_version)) => {
            println!("{subject} was installed by this tool: modpack v{modpack_version} (loader v{loader_version}).");
        }
        _ => {
            println!("{subject} wasn't installed by this tool (manual install or another mod manager?).");
        }
    }
    let folder_list = info
        .subfolders
        .iter()
        .map(|s| format!("{} ({} file(s))", s.name, s.file_count))
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
fn offer_restore_menu(backups: &[(PathBuf, Option<backup::BackupInfo>)]) -> Result<Option<usize>> {
    println!("Existing backups found:");
    for (i, (path, info)) in backups.iter().enumerate() {
        let label = match info {
            Some(info) => {
                let origin = match (&info.previous_modpack_version, &info.previous_loader_version) {
                    (Some(mv), Some(lv)) => format!("modpack v{mv} (loader v{lv})"),
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
    println!("Press Enter to install/update normally, or type a number above to restore that backup instead.");
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

fn prompt_for_path(game: &GameMetadata) -> Result<PathBuf> {
    loop {
        print!(
            "Please paste the full path to your {} install \
             (the folder containing {} / {}): ",
            game.game_name, game.windows_exe, game.linux_exe
        );
        io::stdout().flush().ok();
        let mut line = String::new();
        let bytes_read = io::stdin().read_line(&mut line).context("reading input path")?;
        if bytes_read == 0 {
            anyhow::bail!("no input received (stdin closed) while waiting for a game install path");
        }
        let candidate = PathBuf::from(line.trim());
        if detect::is_valid_game_root(&candidate, &game.windows_exe, &game.linux_exe, |p| p.exists()) {
            return Ok(candidate);
        }
        println!(
            "'{}' doesn't look like a {} install (no {} / {} found). Try again.",
            candidate.display(),
            game.game_name,
            game.windows_exe,
            game.linux_exe
        );
    }
}

fn pause_before_exit() {
    print!("Press Enter to exit...");
    io::stdout().flush().ok();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}
