# Maintainer Guide

## One-time setup

```sh
rustup target add x86_64-pc-windows-gnu
# Arch/CachyOS:
sudo pacman -S mingw-w64-gcc
# Debian/Ubuntu:
sudo apt install mingw-w64
```

## Building the installer shells (only needed when installer-runtime/installer-shell code changes)

```sh
cargo build -p installer-shell --release
cargo build -p installer-shell --release --target x86_64-pc-windows-gnu
```

This produces the two reusable installer binaries that `mod-packager` embeds mod payloads
into:
- `target/release/installer-shell` (Linux)
- `target/x86_64-pc-windows-gnu/release/installer-shell.exe` (Windows)

## Checking for mod updates without cutting a release

```sh
cargo run -p downloader --bin mod-downloader -- --check
```

Resolves the latest version of every configured mod/BepInEx (no downloads, no cache
writes, doesn't touch `modpack.lock.toml`) and diffs against the current lockfile. Prints
"up to date" and exits 0 if nothing's changed, or lists what's newer and exits 1 — useful
for a quick look, or wiring into a cron/CI check, before deciding whether `./deploy.sh` is
worth running.

## Cutting a release (every time mods change)

1. Edit `modpack.toml` to add/remove/change mods.
2. From the repo root, run:
   ```sh
   ./deploy.sh
   ```
   This builds the installer shells (Linux always; Windows too if the mingw cross-compile
   target is set up, otherwise it's skipped with a message), downloads the latest mod
   versions, and packages a release — printing the release notes and file listing at the
   end, or telling you there's nothing new to release if nothing changed.
3. Share `releases/vX.Y.Z/installer-windows.exe` and `installer-linux` (and point players
   at that version's `INSTRUCTIONS.md`). For a dedicated server (e.g. AMP-managed), hand the
   host `releases/vX.Y.Z/server-plugins/` and `SERVER.md` — it's a plain, uncompressed copy of
   the mod plugin files to copy-paste over the server's `BepInEx/plugins/` folder.

The individual steps `deploy.sh` runs (`cargo build -p installer-shell --release`,
`cargo run -p downloader --bin mod-downloader`, `cargo run -p packager --bin mod-packager`)
still work standalone if you want more control over any one of them.

`mod-packager` auto-bumps the version by diffing against the previous release's lockfile:
adding/removing a mod bumps minor, a version-only change bumps patch, no changes means
nothing to release. See `releases/CHANGELOG.md` for the running history.

## Repo layout

- `crates/common` — shared config/lockfile types, semver diff logic.
- `crates/downloader` (`mod-downloader`) — resolves + caches latest BepInEx/mod versions.
- `crates/packager` (`mod-packager`) — builds the compressed payload, embeds it into the
  installer shells, handles versioning/changelog, exports `server-plugins/`.
- `crates/installer-runtime` — extraction, Valheim path detection, backup/restore logic
  shared by both installer binaries.
- `crates/installer-shell` — the trivial per-target binary `mod-packager` embeds payloads
  into.
- `crates/server-deploy` (`deploy-amp`) — pushes `server-plugins/` to an AMP-managed
  dedicated server over SFTP and triggers a restart via AMP's JSON API.
- `cache/` — downloaded mod/BepInEx files, gitignored.
- `releases/` — per-version lockfile, changelog, instructions, installer binaries, and
  `server-plugins/`.

## AMP server deploy automation

For a dedicated server managed through AMP (CubeCoders), `deploy-amp` pushes a release's
`server-plugins/` folder over SFTP and then calls AMP's JSON API to restart the instance —
automating the manual copy-paste step described in `SERVER.md`.

**Setup**: copy `amp.toml.example` to `amp.toml` (gitignored — it holds credentials) and
fill in your SFTP host/port/credentials, the instance's absolute `remote_plugins_path`, and
AMP panel API credentials. If 2FA is enabled on the SFTP account, set `totp_secret` to its
base32 secret (must decode to ≥128 bits — a normal authenticator-app secret is fine) and
the current code is computed and appended to the password automatically each run.

**Running it**: once `amp.toml` exists, `./deploy.sh` runs `deploy-amp` automatically as
its last step (skipped with a message if `amp.toml` is absent). To run it standalone:
```sh
cargo run -p server-deploy --bin deploy-amp
# or target a specific release instead of releases/latest.txt:
cargo run -p server-deploy --bin deploy-amp -- --release-dir releases/v0.2.0
```

**Important**: `restart_method` in `amp.toml` (default `Core/Restart`) is a best guess —
the exact API method/params for restarting an instance can vary by AMP version and how the
instance is fronted (standalone vs behind ADS). Every running AMP installation
self-documents its exact available API calls at `<base_url>/API` — check that against your
own instance and adjust `restart_method` if the default doesn't work. If the restart call
fails, the file upload has already succeeded by that point; `deploy-amp` says so and you
can restart manually from the AMP panel.

## Backups and restore (what end users see)

Before an install or update touches anything, the installer snapshots **every top-level
`BepInEx/` subfolder the release is about to overwrite** — typically `core/`, `config/`,
and `plugins/`, not just plugins — into
`BepInEx/_installer_backups/<timestamp>/` before writing anything new. If a subfolder being
backed up contains symlinks (e.g. a Vortex-style symlinked mod deployment), the backup
preserves the symlinks themselves rather than copying resolved file contents, so restoring
puts the exact previous setup back, links included.

Every packaged release bakes a small marker (`BepInEx/.valheim-mod-installer.marker`) into
the payload recording its modpack/BepInEx version, so a later backup can say whether what
it's backing up was installed by this tool (and which version) or came from somewhere else
(manual install, another mod manager) — shown to the user at backup time.

On startup, if backups exist, the installer lists them (origin, folders captured, file/
symlink counts) and lets the user pick one to restore instead of installing — pressing
Enter with no input still does the normal single-click install/update. Restoring itself
snapshots whatever's currently in place first, so it's symmetric with install and can never
lose data. **Backups are never deleted automatically** — cleanup under
`BepInEx/_installer_backups/` is left entirely to the user, and the installer says so every
time it runs.
