<p align="center">
  <img src="docs/images/logo-640.png" alt="Kingdom Hearts x Elden Ring" width="420">
</p>

# Kingdom Hearts x Elden Ring

Play as **Sora, Donald and Goofy** from KINGDOM HEARTS III in **ELDEN RING**. They fight with their own KH3 moves,
magic and combos, they have KH3's menus and HUD, and they can launch Elden Ring's enemies into the air and juggle them.

This is a **developer preview**. It's playable, but expect rough edges.

**No game files are included.** You need your own PC copies of both games. A setup program builds everything the mod
needs from them, on your PC.

## What's in it

- **Three playable characters.** Start your journey as Sora, Donald or Goofy. The others join you along the way,
  follow you as party members, and can take the lead.
- **KH3 combat.** Combos, finishers, blocking, dodging, air combos and team attacks. Enemies, bosses included, can be
  launched and juggled.
- **Magic.** Sora's spells, and Donald as a real mage, all the way up to his ultimate spell, Zettaflare.
- **KH3's look and menus.** The HUD, the command menu, the camp menu, the save screens and the cameras.
- **Progression.** Levels and abilities, KH3 rewards from Elden Ring bosses, munny, a Moogle shop, and KH3 save
  points at the sites of grace.
- **Sora's outfits** from KH1, KH2 and the KH3 worlds.
- **Ladders:** press Y at a ladder and the whole party jumps to the top in one leap. To go down, just drop off.
- **Made for a controller**, with KH3's Xbox layout.

## What you need

- **ELDEN RING** on PC (Steam), version **2.7.2.0** and DLC.
- **KINGDOM HEARTS III + Re Mind** on PC (Steam or Epic).
- **me3 (Mod Engine 3)**, version **0.13.0 or newer**: the tool that loads the mod. Get it from
  [its releases page](https://github.com/garyttierney/me3/releases).
- About **80 GB of free disk space** while the setup runs.
- A **controller** is strongly recommended.

You don't need to install anything else. The setup has everything it needs built in.

## Install

1. **Download** the latest `kingdom-hearts-x-elden-ring-...-win64.zip` from the
   [Releases page](https://github.com/StrawHatPro/kingdom-hearts-x-elden-ring/releases), and unzip it into a folder
   you'll keep. The mod runs from that folder.
2. **Install me3.**
3. **Double-click `setup.exe`** in the mod's folder.
   - Windows may say "Windows protected your PC", because `setup.exe` isn't signed. Click **More info**, then
     **Run anyway**. Your antivirus may also ask about it.
   - The setup finds your games by itself. If it can't, it opens a window so you can pick the game's folder.
4. **Wait.** The first time, the setup builds the mod's files from your KH3 install. This takes about 30 minutes, and
   longer on a slower PC. When it says **Ready**, you can play.

If something goes wrong, the setup stops and says why. Fix it and run `setup.exe` again: it carries on from where it
stopped.

## Play

The mod is **offline only**. me3 starts Elden Ring without Easy Anti-Cheat, so never take the mod online. Steam must be
running.

**The easy way:** double-click `er\play.bat` in the mod's folder.
It finds me3 by itself. If it can't, it asks you to pick `me3.exe` once and remembers it. It doesn't need to be run
as administrator.

**With Me3 Manager:**
1. Run the setup first (see Install).
2. In Me3 Manager, make a new Elden Ring profile just for this mod.
3. Add `er\mods\kh_er_loader.dll` from the mod's folder as an **external** native mod. Don't let Me3 Manager copy it:
   it has to stay in the mod's folder.
4. In **Profile settings**, turn on a **custom save file** and call it something like `KHSora.sl2`. Never use
   `ER0000.sl2`.
5. Launch the profile.

To check that the mod loaded, look for `er\logs\kh-er.log` in the mod's folder.

## Your saves

Your mod game is kept in two places:
- **The mod's save file**, `KHSora.sl2` (when you use `er\play.bat`), in Elden Ring's save folder
  (`%APPDATA%\EldenRing\<your id>\`), next to your normal save.
- **Your KH progress** (Sora's and the party's levels, abilities and items), in the mod's folder, in `er\saves`.

To back up your mod game, or move it to another PC, copy both.

- **Your normal Elden Ring save is never used.** The mod plays on its own save file.
- **Any characters from your normal Elden Ring save are copied over** into the mod's save file the first time you
  play. They're only copies: you can safely ignore them, and overwrite them with a **New Game** in the mod. Your real
  characters stay untouched in your normal save.

## Updating

1. Close Elden Ring.
2. Unzip the new version to the same place as before, so its `kingdom-hearts-x-elden-ring` folder lands on top of
   your mod folder. Say yes when Windows asks to replace files. Your saves (`er\saves`), your settings and the files the
   setup built are kept, because they aren't in the zip.
3. Double-click `setup.exe` again.

The setup reuses the files it built before, unless the new version needs them rebuilt.

## Uninstalling

1. If you might come back, copy `er\saves` and `KHSora.sl2` somewhere safe. **Deleting the mod's folder deletes your
   KH progress.**
2. Delete the mod's folder. The mod never writes into the game folders.
3. If you like, delete the mod's save file `KHSora.sl2` (and `KHSora.sl2.bak`) from
   `%APPDATA%\EldenRing\<your id>\`. Leave `ER0000.sl2` alone: that's your normal game.
4. Uninstall me3 if you don't use it for anything else.

## Building from source

Want to build it yourself, or need the setup's extra options? See [docs/BUILDING.md](docs/BUILDING.md).

## Credits and legal

- The Elden Ring SDK [fromsoftware-rs](https://github.com/Dasaav-dsv/fromsoftware-rs) by Dasaav and contributors.
- [me3](https://github.com/garyttierney/me3) by Gary Tierney and contributors.
- [Node.js](https://nodejs.org), built into `setup.exe`, and the open-source packages listed in
  [LEGAL.md](LEGAL.md). Their licences are in `THIRD-PARTY-NOTICES.txt` in the download.

This is an unofficial fan project. It is not affiliated with, endorsed by or sponsored by Square Enix, Disney,
FromSoftware or Bandai Namco Entertainment. KINGDOM HEARTS III is © Disney and Square Enix. ELDEN RING is © Bandai
Namco Entertainment Inc. and FromSoftware, Inc.

You must own both games. No game files are shared: everything the mod needs from the games is built on your PC from
your own copies. [LEGAL.md](LEGAL.md) explains what is and isn't included, and why.

No licence has been chosen for this project yet.
