# Modpack v0.1.0 — Dedicated Server Instructions

The `server-plugins/` folder in this release is a plain, uncompressed copy of every configured mod's plugin files — exactly what needs to be present under your server's `BepInEx/plugins/` folder.

## Manual update (copy-paste)

1. Stop the server.
2. Make sure BepInEx v5.4.2350 is already installed on the server (same `denikson-BepInExPack_Valheim` pack the client installers use).
3. Copy the contents of `server-plugins/` into the server's `BepInEx/plugins/` folder, overwriting existing files.
4. Start the server back up.

This is modpack v0.1.0 — see CHANGELOG.md for what changed. Server-side mods should match the version installed on clients; mismatched BepInEx/mod versions between server and clients can cause connection or desync issues.

## AMP

No direct AMP automation yet — for now this is a manual copy-paste step via AMP's file manager (or SFTP) onto the server's `BepInEx/plugins/` folder. Wiring this into AMP's update/deployment flow is a planned improvement.
