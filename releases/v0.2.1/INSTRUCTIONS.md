# Modpack v0.2.1 — Install Instructions

1. Download the file matching your OS:
   - Windows: `installer-windows-v0.2.1.exe`
   - Linux (native, not Proton): `installer-linux-v0.2.1`

2. Make sure Valheim and Steam are already installed, and you've run the game at least once.

3. Run the file:
   - Windows: double-click `installer-windows-v0.2.1.exe`.
   - Linux: open a terminal in the download folder and run:
     `chmod +x installer-linux-v0.2.1 && ./installer-linux-v0.2.1`

4. If it can't find your Valheim folder automatically, it will ask you to paste the path (the folder containing `valheim.exe` or `valheim.x86_64`).

This installs modpack v0.2.1 (BepInEx v5.4.2350). See CHANGELOG.md for what changed.

Linux note: BepInEx on Linux requires a Steam launch option. In Steam, right-click Valheim -> Properties -> Launch Options, and set it to run `start_game_bepinex.sh` from the game folder (see BepInEx's own README included in this install for the exact command). Windows needs no extra setup — BepInEx loads automatically.
