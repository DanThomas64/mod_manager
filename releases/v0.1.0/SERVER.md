# Modpack v0.1.0 — Dedicated Server Instructions

The `server-plugins/` folder in this release is a plain, uncompressed copy of every configured mod's files — exactly what needs to be present under your server's `BepInEx/plugins/` folder.

## Manual update (copy-paste)

1. Stop the server.
2. Make sure the loader (v5.4.2350) is already installed on the server, same as the client installers use.
3. Copy the contents of `server-plugins/` into the server's `BepInEx/plugins/` folder, overwriting existing files.
4. Start the server back up.

This is modpack v0.1.0 for Valheim — see CHANGELOG.md for what changed. Server-side mods should match the version installed on clients; mismatched loader/mod versions between server and clients can cause connection or desync issues.
