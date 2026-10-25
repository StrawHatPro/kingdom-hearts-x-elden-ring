//! The settings file (`kh-er.ini` next to the DLL; (INI) until 2026-10-24 it was `kh-er.ctl`, which is still read while
//! no `kh-er.ini` exists): `key = value` lines, read again whenever the file changes (checked about once a second on the
//! game thread). It lets an experiment be changed while the game runs: flags to set on the player, the mode, offsets.
//! Unknown keys are simply not asked for.
//!
//! (INI) THE RULES OF THE FILE ([`parse`]), the same for `kh-er.ini`, the old `kh-er.ctl` and `kh-er.cfg`:
//! - a line `key = value` (spaces around `=` and at either end do not matter; the value is everything after the FIRST `=`);
//! - a line starting with `#` or `;` is a comment; an INLINE comment starts at a `#` or `;` that follows a space or a
//!   tab (`hud.kh = 1   ; on`), so `a;b` or `key=#x` keep their sign. A value in double quotes (`key = "a ; b"`) is
//!   taken as it is between the quotes;
//! - `[Section]` lines only lay the file out: the keys stay global (`[Sora]` + `mode = sora` is the key `mode`);
//! - blank lines, a UTF-8 BOM, UTF-16 with its BOM (Notepad's "Unicode") and CRLF or LF line ends are all fine;
//! - a key given twice: the last one wins.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// (INI) THE SETTINGS FILE'S NAME beside the DLL.
pub const FILE: &str = "kh-er.ini";
/// (INI) Its old name: read only while no [`FILE`] exists (tools/ctl_to_ini.js converts it and keeps it as `kh-er.ctl.bak`).
pub const OLD_FILE: &str = "kh-er.ctl";

/// (INI) The file that is really read for `path`: an `.ini` that does not exist falls back to the `.ctl` of the same name
/// when that exists; for a `.ctl` path (the old name) the `.ini` of the same name wins when it exists. Any other path (`kh-er.cfg`) is itself.
pub fn resolve(path: &Path) -> PathBuf {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    let other = |ext: &str| Some(path.with_extension(ext)).filter(|p| p.is_file());
    match ext.as_deref() {
        Some("ini") if !path.is_file() => other("ctl").unwrap_or_else(|| path.to_path_buf()),
        Some("ctl") => other("ini").unwrap_or_else(|| path.to_path_buf()),
        _ => path.to_path_buf(),
    }
}

/// (INI) A settings file's text: UTF-8 (a BOM dropped) or UTF-16 LE / BE with its BOM. None: not readable, or not text.
pub fn read_text(path: &Path) -> Option<String> {
    decode(&std::fs::read(path).ok()?)
}

/// (INI) [`read_text`] of bytes.
pub fn decode(bytes: &[u8]) -> Option<String> {
    let utf16 = |b: &[u8], le: bool| {
        let units: Vec<u16> = b.chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        String::from_utf16(&units).ok()
    };
    if let Some(b) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8(b.to_vec()).ok()
    } else if let Some(b) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        utf16(b, true)
    } else if let Some(b) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        utf16(b, false)
    } else {
        String::from_utf8(bytes.to_vec()).ok()
    }
}

/// (INI) One file's `key = value` pairs in file order (the rules in the module's head). tools/ctl_to_ini.js has the same
/// rules in JavaScript (its dry run checks a converted file with them); `ini_tests` below hold both to the same answers.
pub fn parse(text: &str) -> Vec<(String, String)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && strip_inline_comment(line).ends_with(']') {
            continue; // a [Section]: layout only
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim();
            if !k.is_empty() {
                out.push((k.to_string(), value(v)));
            }
        }
    }
    out
}

/// (INI) The text before an inline comment (a `#` or `;` after a space or tab), trimmed.
fn strip_inline_comment(s: &str) -> &str {
    let mut prev_space = false;
    for (i, c) in s.char_indices() {
        if (c == '#' || c == ';') && prev_space {
            return s[..i].trim();
        }
        prev_space = c == ' ' || c == '\t';
    }
    s.trim()
}

/// (INI) The value after `=`: a quoted value as it is between its quotes, else the text before an inline comment.
fn value(raw: &str) -> String {
    let v = raw.trim();
    if let Some(rest) = v.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            let after = rest[end + 1..].trim();
            if after.is_empty() || after.starts_with('#') || after.starts_with(';') {
                return rest[..end].to_string();
            }
        }
    }
    // the raw value keeps its leading space: `key = ; note` is an empty value with a comment, `key=#x` the value `#x`
    strip_inline_comment(raw).to_string()
}

/// THE SHIPPED DEFAULTS of keys that are absent from the file (the user's decisions of 2026-10-12, each switched on
/// after it was seen working in the game, IMPL_ER7_STATE.md; `key = 0` in the file switches one off):
/// Sora's HUD and menus, Elden Ring's own HUD hidden part by part while ours is up (IMPL_ER8: its compass and prompts
/// stay; `hud.er = off` is the old all-off route through its option, `hud.er = 1` leaves it whole), Start = the Kingdom
/// Hearts menu (LB + Start = Elden Ring's) with the world paused, the KH3 game-over menu.
/// IMPL_ER9 (seen working in the game 2026-10-12): KH3's prize orbs from killed enemies, munny as the currency (the rune
/// counter mirrors Sora's munny), KH3 save points at the sites of grace.
/// `save.kh` (the user, 2026-10-12: "make Sora start at level 1 by default"): Sora's own save data per Elden Ring character;
/// a character without a file starts at level 1. `save.kh = 0` gives the level-99 test loadout again, `sora.level = N` a test level.
/// IMPL_ER10 (2026-10-12): NOT in the defaults, each for the reason in IMPL_ER10_STATE.md: `save.points_only = 1` (saving only at save
/// points + Continue = the game's own load of the last save; armed per character by its first Save at a save point:
/// SEEN WORKING on the test save, but Elden Ring's own Quit Game with the saves refused and a relaunch were not run),
/// `cutscene.menu = 1` (KH3's cutscene pause menu on Start in a cutscene: wired, never run in the game).
/// IMPL_ER11 (2026-10-12): `menu.pause = 1` is now THE GAME'S OWN PAUSE (the tutorial popup's; pause.rs); `cutscene.menu = 1`
/// (KH3's cutscene pause menu on Start in a cutscene: SEEN in the opening movie on the test save: Start opens, B does
/// nothing, Continue resumes, Skip Scene skips).
/// `save.points_only = 1` (the user's decision; default since IMPL_ER11: Elden Ring's own Quit Game / Return to Desktop
/// leave WITHOUT saving for an armed character, the title's system save is never refused, proofs in IMPL_ER11_STATE.md):
/// a character is ARMED by its first Save at a save point; from then only that Save writes, and Continue / a death load
/// the last save. `newgame.auto = 1`: the Create Character screen is filled in behind a cover (name "Sora").
/// IMPL_ER12 (2026-10-12): `newgame.auto = 1` IS A DEFAULT NOW: from New Game the screen is black (the cover goes up inside the
/// call that makes "Select Character Base"), the creation is pressed under it (name "Sora"), KH3's two choices are asked on
/// that black BEFORE the intro (the game's new-game step is held meanwhile), then the game's intro. Proven by whole-window
/// 60 fps captures on three runs (IMPL_ER12_STATE.md). `newgame.auto = 0`: Elden Ring's own creation screens.
/// (user decision 2026-10-13: "when gaining EXP it should say 'EXP'"): `hud.exp_unit` = EXP by default in the Elden Ring mod (`hud.exp_unit = P` gives the game's own text).
/// (user decision 2026-10-13: "the boss finish must be on by default"): `finish.kh` = 1; it has not been run in Elden Ring (IMPL_ER22_STATE.md): `finish.kh = 0` switches it off, `finish.speed = 0` keeps Elden Ring's own clock untouched.
/// (user decision 2026-10-13: "I want all the functions I requested to be online"): `loot.kh`, `loot.gate`, `shop.kh`, `items.er_keys` = 1
/// (the kill drop and the gate ran in the user's game on 4B6C2CFEF72E; the rest of IMPL_ER28 has not run in Elden Ring). `loot.purge` stays
/// off: it cannot be undone on the game's save.
/// (IMPL_ER31; the user, 2026-10-13: "also add magic unlocks to bosses. Make Margit give you thunder, for example, and Godrick the grafted
/// gives you heal"; standing instruction "I want all the functions I requested to be online"): `boss.rewards` = 1: a beaten boss of
/// sheets/port/sora-er-boss-rewards.json gives its reward once per character. NOT RUN IN ELDEN RING. `boss.rewards = 0` switches it off.
/// (IMPL_ER32; the user, 2026-10-13: "also Sora's attacks should break breakable objects and fire should explode explosives"; standing
/// instruction as above): `break.kh` = 1: Sora's hits reach breakable map objects as bullets with object damage, KH3 fire / blizzard /
/// thunder are Elden Ring fire / magic / lightning (breakables.rs). NOT RUN IN ELDEN RING. `break.kh = 0`: the attack rows exactly as before.
pub const DEFAULTS: &[(&str, &str)] = &[("hud.kh", "1"), ("hud.er", "0"), ("in.kh_menu", "1"), ("menu.pause", "1"), ("gameover.kh", "1"), ("prize.kh", "1"), ("munny.mirror", "1"), ("savepoint.kh", "1"), ("save.kh", "1"), ("cutscene.menu", "1"), ("save.points_only", "1"), ("newgame.auto", "1"), ("saveload.kh", "1"), ("hud.exp_unit", "EXP"), ("finish.kh", "1"), ("loot.kh", "1"), ("loot.gate", "1"), ("shop.kh", "1"), ("items.er_keys", "1"), ("boss.rewards", "1"), ("break.kh", "1"), ("cure.er_status", "1"), ("blood.kh", "1")];
// (IMPL_ER78b) `r.hybrid` is NOT a default: the first build with it on (B9CECFC41912) was reported broken in the user's game
// (2026-10-15); it stays opt-in (`r.hybrid = 1`) until a `r.hybrid_dump` from the game confirms it (kh-er-render hybrid.rs).

/// DEVELOPMENT AIDS ARE NOT PART OF THE RELEASED MOD: test one-shots, debug probes, scripted input and cheats
/// (invulnerability, unlocks, rewards or levels given, the party host's debug words). Their keys are dropped when the file
/// is read, so no line of kh-er.ini (or the old kh-er.ctl) can switch one on. Only a build with the cargo feature
/// `dev-aids` (off by default, never used by setup/setup.js) reads them.
const DEV_PREFIXES: &[&str] = &["test.", "dbg.", "p.dbg_"];
const DEV_KEYS: &[&str] = &[
    "party.word", "party.recover", "party.body_test", "party.body_test.show", "party.team_invuln", "party.climb_capture",
    "dmg.invuln", "dmg.off", "unlock.give", "unlock.goto", "unlock.jump", "unlock.reset", "unlock.starter", "unlock.state",
    "boss.grant", "prize.test", "finish.test", "sora.level", "hud.cmd", "r.hybrid_kh_test", "r.hybrid_dump", "blt.dump",
    "in.script", "in.script_menus", "in.vpad", "in.x", "in.y", "in.rx", "in.ry", "in.dir", "in.jump", "in.jump_hold",
];

/// True for a key of a development aid (see [`DEV_KEYS`]).
pub fn is_dev_key(key: &str) -> bool {
    DEV_PREFIXES.iter().any(|p| key.starts_with(p)) || DEV_KEYS.contains(&key)
}

#[derive(Default)]
pub struct Control {
    path: PathBuf,
    /// (INI) the files read at the last load and their times (None: none of them existed)
    stamp: Option<Vec<(PathBuf, SystemTime)>>,
    /// (INI) the main file really read ([`resolve`] of `path`), and the one last told in the log ([`Control::source_notice`])
    source: PathBuf,
    told: Option<PathBuf>,
    values: HashMap<String, String>,
    /// Counts reloads: an experiment that acts once per change compares it.
    pub generation: u64,
}

impl Control {
    pub fn new(path: PathBuf) -> Self {
        Self { source: path.clone(), path, ..Self::default() }
    }

    /// True when the file was (re)read.
    pub fn poll(&mut self) -> bool {
        // (INI) the main file: kh-er.ini, or the old kh-er.ctl while no kh-er.ini exists (resolve)
        let main = resolve(&self.path);
        let files: Vec<PathBuf> = vec![main.clone()];
        let stamp: Vec<(PathBuf, SystemTime)> = files.iter().filter_map(|p| Some((p.clone(), std::fs::metadata(p).and_then(|m| m.modified()).ok()?))).collect();
        let stamp = Some(stamp).filter(|s| !s.is_empty());
        if stamp == self.stamp {
            return false;
        }
        self.stamp = stamp;
        self.source = main;
        self.values.clear();
        for path in &files {
            if let Some(text) = read_text(path) {
                for (k, v) in parse(&text) {
                    if cfg!(not(feature = "dev-aids")) && is_dev_key(&k) {
                        continue; // a development aid: not part of the released mod
                    }
                    self.values.insert(k, v);
                }
            }
        }
        self.generation += 1;
        true
    }

    /// (INI) The main file read at the last load (kh-er.ini, or the old kh-er.ctl).
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// (INI) True when the main file read is the OLD name (`kh-er.ctl` read because no `kh-er.ini` exists).
    pub fn old_name(&self) -> bool {
        self.source != self.path && self.source.extension().is_some_and(|e| e.eq_ignore_ascii_case("ctl"))
    }

    /// (INI) A one-line notice for the log when the main file read is another one than last told (None: unchanged).
    pub fn source_notice(&mut self) -> Option<String> {
        if self.told.as_ref() == Some(&self.source) {
            return None;
        }
        self.told = Some(self.source.clone());
        Some(if self.old_name() {
            format!("control: NOTICE: the OLD settings file name was used: {} (no {} beside the DLL; the old name {OLD_FILE} is read only until node tools/ctl_to_ini.js converts it)", self.source.display(), self.path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned()))
        } else {
            format!("control: settings file {}{}", self.source.display(), if self.source.is_file() { "" } else { " (not there: the shipped defaults)" })
        })
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str()).or_else(|| DEFAULTS.iter().find(|(k, _)| *k == key).map(|(_, v)| *v))
    }

    pub fn f32(&self, key: &str) -> Option<f32> {
        self.str(key)?.parse().ok()
    }

    pub fn i64(&self, key: &str) -> Option<i64> {
        self.str(key)?.parse().ok()
    }

    /// `None` when the key is absent: "leave the game's value alone".
    pub fn flag(&self, key: &str) -> Option<bool> {
        match self.str(key)? {
            "1" | "true" | "on" => Some(true),
            "0" | "false" | "off" => Some(false),
            _ => None,
        }
    }

    pub fn on(&self, key: &str) -> bool {
        self.flag(key) == Some(true)
    }

    /// (DON-PROG) Every key / value of the file (the `don.*` keys of kh_partyhost::donald::install_all).
    pub fn pairs(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn dump(&self) -> String {
        let mut keys: Vec<_> = self.values.iter().collect();
        keys.sort();
        keys.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")
    }
}

/// A ONE-SHOT KEY: something is carried out once per CHANGE of the key's value. A value that is
/// already in the file when the watcher is made (a payload starting, Sora starting after a death)
/// is NOT carried out: a hot reload under a player once jumped the clock and ran a scan that way
/// (IMPL_ER3_STATE.md). An absent or emptied key never fires.
pub struct OneShot {
    key: &'static str,
    seen: Option<String>,
}

impl OneShot {
    pub fn new(key: &'static str, control: &Control) -> OneShot {
        OneShot { key, seen: control.str(key).map(str::to_string) }
    }

    /// The new value, when it changed since the last look and is not empty.
    pub fn fired(&mut self, control: &Control) -> Option<String> {
        let now = control.str(self.key).map(str::to_string);
        if now == self.seen {
            return None;
        }
        self.seen = now.clone();
        now.filter(|v| !v.is_empty())
    }
}

// (INI) the settings file's rules: the .ctl and the sectioned .ini read the same, the fallback, comments, a BOM
#[cfg(test)]
#[path = "control_ini_tests.rs"]
mod ini_tests;
