mod ansi;
pub mod backup;
pub mod detect;
pub mod extract;
pub mod install;
pub mod marker;
pub mod timestamp;

use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::PathBuf;

const FOOTER_MAGIC: &[u8] = b"VMODPACK1";
const LINE_WIDTH: usize = 80;

const DISCLAIMER: &str = "Before changing anything, every folder about to be overwritten (core, config, \
plugins — whatever this release touches) is backed up first, symlinks and all (so a \
Vortex-style symlinked mod setup restores exactly as it was). Backups are never \
deleted automatically — that's on you to clean up once you're confident you don't \
need them.";

enum MainAction {
    Update,
    Restore,
    Exit,
}

/// Entry point shared by both `installer-shell` binaries. Shows what's in
/// this version, then asks the user to pick update/restore/exit before
/// touching anything. Update backs up the current plugins folder and
/// installs the payload embedded after this executable's own bytes; restore
/// lists previous backups at the detected install and lets the user pick
/// one to roll back to.
pub fn run() -> Result<()> {
    print_banner();

    let own_payload = extract::read_own_payload().context("reading embedded mod payload")?;
    print_changelog(&own_payload.changelog);

    let root = detect_or_prompt_root()?;
    let backups = backup::list_backups(&root);

    match prompt_main_menu(&backups)? {
        MainAction::Update => run_update(&root, &own_payload)?,
        MainAction::Restore => run_restore(&root, &backups)?,
        MainAction::Exit => print_wrapped_colored("Exiting — nothing was changed.", ansi::red_bold),
    }

    pause_before_exit();
    Ok(())
}

fn run_update(root: &std::path::Path, own_payload: &extract::OwnPayload) -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("valheim-mod-installer-{}", std::process::id()));
    extract::unpack_payload(&own_payload.payload, &temp_dir).context("unpacking mod payload")?;

    let backup_outcome =
        backup::snapshot_before_overwrite(&root, &temp_dir).context("backing up files before install")?;
    print_backup_outcome(&backup_outcome, "Your previous install");

    install::install_mods(&temp_dir, &root).context("copying mods into Valheim install")?;
    let _ = std::fs::remove_dir_all(&temp_dir);

    println!();
    println!("{}", ansi::green_bold("Done! Mods installed to:"));
    println!("  {}", root.join("BepInEx").display());
    println!("Launch Valheim normally through Steam.");
    println!();
    println!("{}", ansi::dim("Reminder:"));
    print_wrapped_colored(
        "Backups live under the path below and are kept forever — this tool never \
         deletes them. Clean them up yourself whenever you're confident you don't need them.",
        ansi::dim,
    );
    println!("  {}", root.join("BepInEx").join("_installer_backups").display());
    Ok(())
}

fn run_restore(root: &std::path::Path, backups: &[(PathBuf, Option<backup::BackupInfo>)]) -> Result<()> {
    if backups.is_empty() {
        println!("No backups found under:");
        println!("  {}", root.join("BepInEx").join("_installer_backups").display());
        return Ok(());
    }

    match offer_restore_menu(backups)? {
        Some(chosen) => {
            let (path, _) = &backups[chosen];
            let outcome = backup::restore_backup(root, path)?;
            println!();
            println!("Restored backup:");
            println!("  {}", path.display());
            print_backup_outcome(&outcome, "What was in place just before this restore");
        }
        None => print_wrapped_colored("Cancelled — nothing was changed.", ansi::red_bold),
    }
    Ok(())
}

fn detect_or_prompt_root() -> Result<PathBuf> {
    match detect::find_valheim_install() {
        Some(root) => {
            println!("Found Valheim install at:");
            println!("  {}", ansi::bold(&root.display().to_string()));
            Ok(root)
        }
        None => {
            println!("Could not auto-detect your Valheim install.");
            prompt_for_path()
        }
    }
}

fn rule() -> String {
    "─".repeat(LINE_WIDTH)
}

/// Greedy word-wrap to a fixed column width, measured in chars (not bytes)
/// so multi-byte characters like "—" don't throw off the count.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_len = 0;
    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if current.is_empty() {
            current.push_str(word);
            current_len = word_len;
        } else if current_len + 1 + word_len <= width {
            current.push(' ');
            current.push_str(word);
            current_len += 1 + word_len;
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
            current_len = word_len;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Print `text`, greedy word-wrapped to `LINE_WIDTH` columns.
fn print_wrapped(text: &str) {
    for line in wrap_text(text, LINE_WIDTH) {
        println!("{line}");
    }
}

/// Like `print_wrapped`, but applies an ANSI color/style to each wrapped
/// line individually (wrapping first keeps every visible line ≤`LINE_WIDTH`
/// regardless of how many invisible escape-code bytes `color` adds).
fn print_wrapped_colored(text: &str, color: fn(&str) -> String) {
    for line in wrap_text(text, LINE_WIDTH) {
        println!("{}", color(&line));
    }
}

fn print_banner() {
    let border = "═".repeat(LINE_WIDTH);
    println!("{}", ansi::cyan_bold(&border));
    println!("{}", ansi::cyan_bold(&format!("{:^LINE_WIDTH$}", "Valheim Mod Installer")));
    println!("{}", ansi::cyan_bold(&border));
    println!();
    print_wrapped(
        "This installs BepInEx and the configured mods into your Valheim install. Make \
         sure Valheim is already installed via Steam and has been run at least once.",
    );
    println!();
    print_wrapped_colored(DISCLAIMER, ansi::dim);
    println!();
}

/// Show the changelog entry for the version embedded in this installer, so
/// the user knows what they're about to install before choosing an action —
/// printed right after the banner, ahead of the update/restore/exit menu.
fn print_changelog(changelog: &str) {
    let changelog = changelog.trim();
    if changelog.is_empty() {
        return;
    }
    println!("{}", ansi::yellow_bold(&rule()));
    println!("{}", ansi::yellow_bold("  What's in this version"));
    println!("{}", ansi::yellow_bold(&rule()));
    println!("{changelog}");
    println!("{}", ansi::yellow_bold(&rule()));
    println!();
}

/// Ask the user to pick update/restore/exit. Pressing Enter with no input
/// picks update (keeps the common case single-click). Stdin closing (0
/// bytes read, EOF) is treated as exit, not update — a genuinely absent
/// answer should never fall through to making changes. 'q' also quits —
/// not listed in the menu text, but a quiet convenience for anyone who
/// reaches for it out of habit.
fn prompt_main_menu(backups: &[(PathBuf, Option<backup::BackupInfo>)]) -> Result<MainAction> {
    println!("  {} Update / install mods", ansi::bold("1)"));
    println!("  {} {}", ansi::bold("2)"), restore_menu_label(backups));
    println!("  {} Exit", ansi::bold("3)"));
    println!();
    loop {
        print!("{} ", ansi::green_bold("Choose an option [1/2/3] (default 1):"));
        io::stdout().flush().ok();
        let mut line = String::new();
        let bytes_read = io::stdin().read_line(&mut line).context("reading menu choice")?;
        if bytes_read == 0 {
            return Ok(MainAction::Exit);
        }
        match line.trim().to_lowercase().as_str() {
            "" | "1" => return Ok(MainAction::Update),
            "2" => return Ok(MainAction::Restore),
            "3" | "q" => return Ok(MainAction::Exit),
            _ => println!("Unrecognized choice — enter 1, 2, or 3."),
        }
    }
}

/// "Restore from a backup" menu line, with a count and the most recent
/// backup's timestamp when any exist — `backups` is already sorted newest
/// first by `backup::list_backups`, and its folder name *is* the timestamp
/// (`timestamp::now_stamp`), so no extra info needs to have been recorded.
fn restore_menu_label(backups: &[(PathBuf, Option<backup::BackupInfo>)]) -> String {
    let Some((latest_path, _)) = backups.first() else {
        return "Restore from a backup (none available)".to_string();
    };
    let stamp = latest_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    format!(
        "Restore from a backup ({} available, latest {})",
        backups.len(),
        timestamp::to_display(&stamp)
    )
}

fn print_backup_outcome(outcome: &backup::BackupOutcome, subject: &str) {
    let (Some(path), Some(info)) = (&outcome.backup_path, &outcome.info) else {
        println!("No previous install found — nothing to back up.");
        return;
    };
    match (&info.previous_modpack_version, &info.previous_bepinex_version) {
        (Some(modpack_version), Some(bepinex_version)) => {
            print_wrapped(&format!(
                "{subject} was installed by this tool: modpack v{modpack_version} (BepInEx v{bepinex_version})."
            ));
        }
        _ => {
            print_wrapped(&format!(
                "{subject} wasn't installed by this tool (manual install or another mod manager?)."
            ));
        }
    }
    let folder_list = info
        .subfolders
        .iter()
        .map(|s| format!("BepInEx/{} ({} file(s))", s.name, s.file_count))
        .collect::<Vec<_>>()
        .join(", ");
    print_wrapped(&format!("Backed up: {folder_list}"));
    if info.total_symlinks() > 0 {
        print_wrapped(&format!(
            "{} of those were symlinks (e.g. Vortex-style deployment) — preserved as symlinks in the backup.",
            info.total_symlinks()
        ));
    }
    println!("Saved to:");
    println!("  {}", path.display());
}

/// List backups and let the user pick one to restore. Pressing Enter with no
/// input cancels the restore (returns to the top-level menu's "nothing was
/// changed" outcome) rather than doing anything by default.
fn offer_restore_menu(backups: &[(std::path::PathBuf, Option<backup::BackupInfo>)]) -> Result<Option<usize>> {
    println!("{}", ansi::bold("Existing backups found:"));
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
        println!("  {} {name}", ansi::bold(&format!("{})", i + 1)));
        for line in wrap_text(&label, LINE_WIDTH - 6) {
            println!("      {line}");
        }
    }
    print_wrapped("Press Enter to cancel, or type a number above to restore that backup.");
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
            println!("Unrecognized choice — cancelling restore.");
            Ok(None)
        }
    }
}

fn prompt_for_path() -> Result<PathBuf> {
    loop {
        print_wrapped(
            "Please paste the full path to your Valheim install (the folder \
             containing valheim.exe / valheim.x86_64):",
        );
        print!("> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        let bytes_read = io::stdin().read_line(&mut line).context("reading input path")?;
        if bytes_read == 0 {
            anyhow::bail!("no input received (stdin closed) while waiting for a Valheim install path");
        }
        let candidate = PathBuf::from(line.trim());
        if detect::is_valid_valheim_root(&candidate, |p| p.exists()) {
            return Ok(candidate);
        }
        println!("'{}' doesn't look like a Valheim install:", candidate.display());
        print_wrapped("(no valheim.exe / valheim.x86_64 found there). Try again.");
    }
}

fn pause_before_exit() {
    print!("Press Enter to exit...");
    io::stdout().flush().ok();
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}
