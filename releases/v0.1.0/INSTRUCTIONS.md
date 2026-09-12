# Modpack v0.1.0 — Install Instructions

1. Download the file matching your OS:
   - Windows: `installer-windows.exe`
   - Linux (native, not Proton): `installer-linux`

2. Make sure Valheim and Steam are already installed, and you've run the game at least once.

3. Run the file:
   - Windows: double-click `installer-windows.exe`.
   - Linux: open a terminal in the download folder and run:
     `chmod +x installer-linux && ./installer-linux`

4. If it can't find your Valheim folder automatically, it will ask you to paste the path (the folder containing `valheim.exe` or `valheim.x86_64`).

This installs modpack v0.1.0 (BepInEx v5.4.2350). See CHANGELOG.md for what changed.

Linux note: BepInEx on Linux requires a Steam launch option. In Steam, right-click Valheim -> Properties -> Launch Options, and set it to run `start_game_bepinex.sh` from the game folder (see BepInEx's own README included in this install for the exact command). Windows needs no extra setup — BepInEx loads automatically.
