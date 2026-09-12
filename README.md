# Valheim Mod Installer

A small Rust toolchain that turns a list of Thunderstore/GitHub mods into a single,
shareable, double-click installer for your Valheim modpack — no Steam Workshop, no
manual DLL-juggling for your players.

## What it does

- **Resolves and downloads** the latest BepInEx and mod versions from Thunderstore or
  GitHub Releases, based on a plain `modpack.toml` you maintain.
- **Packages** everything into a single self-extracting binary per OS
  (`installer-windows.exe`, `installer-linux`) — a player just runs it and it finds their
  Valheim install (or asks) and drops the mods in the right place. No dependencies needed
  on their machine.
- **Auto-versions** each release by diffing against the last one (mod added/removed →
  minor bump, version change → patch bump) and writes a changelog summarizing exactly what
  changed, mod by mod.
- **Backs up before touching anything** — every folder an update is about to overwrite
  (core, config, plugins) is snapshotted first, symlinks and all (so a Vortex-style
  symlinked mod setup survives a backup/restore round-trip intact). The installer can list
  and restore any previous backup on a later run. Nothing is ever deleted automatically.
- **Exports a plain `server-plugins/` folder** alongside each release, for copy-pasting
  onto a dedicated server (e.g. AMP-managed) without needing the client installer at all.
- **Checks for mod updates** on demand (no download/package cycle) so you know at a glance
  whether a new release is worth cutting.
- Optimized for size: mod payloads are zstd-compressed and embedded directly in the
  installer binary, so the whole thing stays small enough to send over Discord/email.

## Quick start

**Maintaining a modpack?** See [`docs/MAINTAINER.md`](docs/MAINTAINER.md) — one-time setup,
cutting a release, repo layout.

**Got a release from someone?** See the `INSTRUCTIONS.md` (players) or `SERVER.md`
(dedicated server hosts) inside whatever `releases/vX.Y.Z/` folder you were handed.

## How it's put together

| Crate | Role |
|---|---|
| `common` | Shared config/lockfile types, semver diff logic. |
| `downloader` (`mod-downloader`) | Resolves + caches the latest BepInEx/mod versions. |
| `packager` (`mod-packager`) | Builds the compressed payload, embeds it into the installer shells, handles versioning/changelog/server export. |
| `installer-runtime` | Extraction, Valheim path detection, backup/restore — shared logic linked into both installer binaries. |
| `installer-shell` | The trivial per-target binary `mod-packager` embeds payloads into. |

See [`docs/MAINTAINER.md`](docs/MAINTAINER.md) for the full breakdown, build commands, and
backup/restore behavior in detail.
