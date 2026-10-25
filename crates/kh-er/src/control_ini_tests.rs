//! (INI) The settings file's rules (control.rs `parse`, `resolve`, `decode`): kh-er.ini read in place of kh-er.ctl.

use super::*;
use std::collections::HashMap;

/// The rules before 2026-10-24 (the old `Control::poll` body), to hold the new ones to the same answers.
fn old_parse(text: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if !k.trim().is_empty() {
                values.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    values
}

fn map(text: &str) -> HashMap<String, String> {
    parse(text).into_iter().collect()
}

/// The user's er\mods\kh-er.ctl of 2026-10-24 (CRLF, no newline at the end).
const USER_CTL: &str = "mode=sora\r\ndraw.axes=0\r\ndraw.sora=0\r\nw.clamp=0\r\nw.hits=1\r\nride=1\r\nloot.kh=1\r\nloot.gate=1\r\nloot.log=1\r\nsavepoint.rest_events=0\r\nboc.gate=0\r\nr.world_lights=0\r\nr.hybrid=1\r\nprompt.er_hide=1";
/// The test profile's er\mods_test\kh-er.ctl of 2026-10-24 (CRLF and LF mixed, a blank line at the end).
const TEST_CTL: &str = "mode=sora\r\ndraw.axes=0\r\ndraw.sora=0\r\nw.clamp=0\r\nw.hits=1\r\nride=1\r\nloot.kh=1\r\nloot.gate=1\r\nloot.log=1\r\nsavepoint.rest_events=0\r\nboc.gate=0\nr.world_lights=0\nsave.dir=saves_test\nsave.file=KHTest.sl2\n\n";

/// What tools/ctl_to_ini.js makes of USER_CTL (its dry run on er\mods\kh-er.ctl, 2026-10-24).
const USER_INI: &str = r#"# Kingdom Hearts x Elden Ring: the mod's settings (kh-er.ini). Read again while the game runs.
# One "key = value" per line. Lines starting with # or ; are comments, and so is the rest of a line after " #" or
# " ;". [Sections] are only for layout: every key works in any section. A key that is not here has its shipped
# default; "key = 0" switches a default off.
# Converted from kh-er.ctl by tools/ctl_to_ini.js (the old file is kept as kh-er.ctl.bak).

[Mod]
# The mod itself.
# mode: sora = Sora plays in place of the Tarnished
mode = sora
# draw.axes: debug drawing of the axes (0 = off)
draw.axes = 0
# draw.sora: debug drawing of Sora's shapes (0 = off)
draw.sora = 0
# w.clamp: collision: 1 = a push out of an overlap is cut short where it would go into something else (0 = off, the shipped default)
w.clamp = 0
# w.hits: 1 = the walls Sora's collision meets are written to the log
w.hits = 1
# ride: 1 = Sora rides lifts and moving floors (0 = off)
ride = 1

[Hud]
# Sora's HUD and menus; Elden Ring's own HUD, prompts and banners.
# prompt.er_hide: 1 = Elden Ring's own action prompt box is hidden under KH3's reaction command
prompt.er_hide = 1

[Save]
# Saving, save points, a new game, the game-over menu, cutscenes.
# savepoint.rest_events: 0 = a save point never runs Elden Ring's rest; sit (or 1) = when Sora sits; always
savepoint.rest_events = 0

[Items]
# Loot, munny, shops, rewards, outfits.
# loot.kh: 1 = KH3 items drop from killed enemies
loot.kh = 1
# loot.gate: 1 = the gate: every other way an item reaches the player is turned into KH3's
loot.gate = 1
# loot.log: 1 = the gate writes what it sees to the log
loot.log = 1
# boc.gate: 0 = Boc's outfits without having talked to Boc (testing); 1 = only after talking to him
boc.gate = 0

[Render]
# Drawing Sora (the r.* switches of the renderer), light and effects.
# r.world_lights: 0 = no light of the world's on Sora
r.world_lights = 0
# r.hybrid: 1 = the hybrid: KH3's toon shading lit by Elden Ring's world light
r.hybrid = 1
"#;

/// The same content as a .ctl and as a sectioned .ini (LF or CRLF) gives the same settings, and the same as the old rules.
#[test]
fn the_ctl_and_the_sectioned_ini_give_the_same_settings() {
    let ctl = map(USER_CTL);
    assert_eq!(ctl.len(), 14);
    assert_eq!(ctl, old_parse(USER_CTL), "the old file means what it meant");
    assert_eq!(map(USER_INI), ctl, "the converted .ini (LF)");
    assert_eq!(map(&USER_INI.replace('\n', "\r\n")), ctl, "the converted .ini (CRLF, as the tool writes it)");
    // the converted file is also read the same by the OLD rules (a DLL from before the .ini): no inline comments in it
    assert_eq!(old_parse(USER_INI), ctl, "the .ini under the old rules");
    assert_eq!(ctl.get("mode").map(String::as_str), Some("sora"));
    assert_eq!(ctl.get("r.hybrid").map(String::as_str), Some("1"));
}

/// Every existing file reads as before: the user's, the test profile's, the release template's and the tools' lines.
#[test]
fn every_existing_file_reads_as_before() {
    let profile = "# a profile's OWN lines, read after er\\mods\\kh-er.ctl\n# and only ...:\nsave.dir=saves_other\nsave.file=KHOther.sl2\n# a test aid\ntest.aid=1\ntest.party_level=99\ntest.don_all_magic=1\n";
    let template = "# Kingdom Hearts x Elden Ring: the mod's options (er/mods/kh-er.ctl). One \"key=value\" per line; # starts a comment.\n# The file is read again while the game runs. The setup writes it once; after that it is yours.\nmode=sora\nr.hybrid=1\nprompt.er_hide=1\n";
    // the er\tools experiment lines (split at ';' into lines) and the tests' own files
    let tools = "mode=sora\ndraw.axes=0\ntime.set=12:30\nsora.warp=-10,2,30\nr.flat=1\ndbg.launch=c4300,up\ndbg.n=123456\nparty.word = party don-ready\ntest.goto = c4630,8\ntest.foe_hp = 10%\nboss.grant = margit\nstory.lift_blockers=on\nparty.er=0\nhud.exp_unit = EXP\nsave.dir=saves_test\n  spaced.key   =   spaced value  \nempty.value=\nkey.with.equals = a=b=c\npath = C:\\Games\\ELDEN RING\\Game\npack=D:/KH pack/build/sora_pack\n";
    for (name, text) in [("user", USER_CTL), ("test", TEST_CTL), ("profile", profile), ("template", template), ("tools", tools)] {
        assert_eq!(map(text), old_parse(text), "{name}");
    }
    assert_eq!(map(tools).get("key.with.equals").map(String::as_str), Some("a=b=c"));
    assert_eq!(map(tools).get("empty.value").map(String::as_str), Some(""));
    assert_eq!(map(TEST_CTL).get("save.dir").map(String::as_str), Some("saves_test"), "the test profile keeps its own saves");
}

/// `;` and `#` comments, whole-line and inline; signs inside a value are kept; quotes keep everything.
#[test]
fn comments_and_inline_comments() {
    let m = map(
        "# a comment\n; another\n   ; indented\nhud.kh = 1 ; on\nhud.er = 0   # off\nr.hybrid = 1\t#tab\na = x;y\nb = x#y\nc=#ff0000\nd = ; nothing\ne = \"a ; b # c\"  ; quoted\nf = \"unclosed ; x\ng = \"two\" words\n",
    );
    assert_eq!(m.get("hud.kh").map(String::as_str), Some("1"));
    assert_eq!(m.get("hud.er").map(String::as_str), Some("0"));
    assert_eq!(m.get("r.hybrid").map(String::as_str), Some("1"));
    assert_eq!(m.get("a").map(String::as_str), Some("x;y"), "a ';' with no space before it is part of the value");
    assert_eq!(m.get("b").map(String::as_str), Some("x#y"));
    assert_eq!(m.get("c").map(String::as_str), Some("#ff0000"), "a value starting with '#' right after '='");
    assert_eq!(m.get("d").map(String::as_str), Some(""), "a comment after '= '");
    assert_eq!(m.get("e").map(String::as_str), Some("a ; b # c"), "a quoted value is taken as it is");
    assert_eq!(m.get("f").map(String::as_str), Some("\"unclosed"), "an unclosed quote: the plain rule");
    assert_eq!(m.get("g").map(String::as_str), Some("\"two\" words"), "text after the quotes: the plain rule");
    assert_eq!(m.len(), 10, "no comment became a key: {m:?}");
}

/// `[Section]` lines only lay the file out: the keys stay global, and the last of a key given twice wins.
#[test]
fn sections_are_layout_only() {
    let m = map("[Sora]\nmode = sora\n  [ Hud ]  ; the HUD\nhud.kh = 0\n[Render]\nr.hybrid = 1\n[Again]\nhud.kh = 1\n[not a section\n[x] = y\n");
    assert_eq!(m.get("mode").map(String::as_str), Some("sora"));
    assert_eq!(m.get("hud.kh").map(String::as_str), Some("1"), "the last one wins, whatever its section");
    assert_eq!(m.get("r.hybrid").map(String::as_str), Some("1"));
    assert_eq!(m.get("[x]").map(String::as_str), Some("y"), "a line with '=' is a key as before");
    assert!(!m.keys().any(|k| k.contains("Sora") || k.contains("Hud") || k.starts_with("Render")), "{m:?}");
    assert_eq!(m.len(), 4);
}

/// A BOM (UTF-8, UTF-16 LE / BE), CRLF, blank lines and whitespace.
#[test]
fn bom_crlf_and_whitespace() {
    let want = map(USER_CTL);
    let mut utf8 = vec![0xEF, 0xBB, 0xBF];
    utf8.extend_from_slice(USER_INI.replace('\n', "\r\n").as_bytes());
    assert_eq!(map(&decode(&utf8).unwrap()), want, "UTF-8 with a BOM");
    assert_eq!(map(&format!("\u{feff}{USER_CTL}")), want, "a BOM left in the text");
    let mut le = vec![0xFF, 0xFE];
    let mut be = vec![0xFE, 0xFF];
    for u in USER_INI.encode_utf16() {
        le.extend_from_slice(&u.to_le_bytes());
        be.extend_from_slice(&u.to_be_bytes());
    }
    assert_eq!(map(&decode(&le).unwrap()), want, "UTF-16 LE (Notepad's Unicode)");
    assert_eq!(map(&decode(&be).unwrap()), want, "UTF-16 BE");
    assert_eq!(map("\r\n\r\n   \t\r\n  mode\t=\tsora  \r\n\r\n"), HashMap::from([("mode".to_string(), "sora".to_string())]));
    assert_eq!(decode(&[0xC3, 0x28]), None, "not text: nothing read (as before)");
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kh-er-ini-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// kh-er.ini is THE file; kh-er.ctl is read only while no kh-er.ini exists, and the log is told so once.
#[test]
fn the_ini_wins_and_the_old_name_is_the_fallback() {
    let dir = temp_dir("fallback");
    let ini = dir.join(FILE);
    let ctl = dir.join(OLD_FILE);
    std::fs::write(&ctl, "mode=sora\r\nhud.kh=0\r\n").unwrap();
    let mut c = Control::new(ini.clone());
    assert!(c.poll(), "the old file is read");
    assert_eq!(c.str("hud.kh"), Some("0"));
    assert!(c.old_name() && c.source() == ctl.as_path());
    let notice = c.source_notice().expect("told once");
    assert!(notice.contains("OLD") && notice.contains("kh-er.ctl") && notice.contains("kh-er.ini"), "{notice}");
    assert_eq!(c.source_notice(), None, "only once");
    assert!(!c.poll(), "nothing changed");
    // kh-er.ini appears (even with an older time than the .ctl): it is read from now on, the .ctl is not
    std::fs::write(&ini, "[Sora]\nmode = sora\n[Hud]\nhud.kh = 1 ; back on\n").unwrap();
    assert!(c.poll(), "the new file is a change");
    assert_eq!(c.str("hud.kh"), Some("1"));
    assert_eq!(c.str("mode"), Some("sora"));
    assert!(!c.old_name() && c.source() == ini.as_path());
    assert!(c.source_notice().is_some_and(|n| n.starts_with("control: settings file") && n.contains("kh-er.ini")));
    std::fs::write(&ctl, "hud.kh=0\r\nmode=other\r\n").unwrap();
    assert_eq!(c.str("hud.kh"), Some("1"), "the .ctl beside a .ini is never read");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Neither file: nothing is read, the shipped defaults answer (as before: the first poll reports no load).
#[test]
fn no_file_gives_the_defaults() {
    let dir = temp_dir("none");
    let mut c = Control::new(dir.join(FILE));
    assert!(!c.poll());
    assert_eq!(c.generation, 0);
    assert_eq!(c.str("hud.kh"), Some("1"), "a shipped default");
    assert!(!c.old_name());
    assert!(c.source_notice().is_some_and(|n| n.contains("not there")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A profile's extra file may be an .ini or a .ctl: a .ctl path reads the .ini of the same name when
/// that exists; kh-er.cfg (and any other name) is itself.
#[test]
fn the_extra_file_takes_either_name() {
    let dir = temp_dir("extra");
    let ctl = dir.join("kh-profile.ctl");
    let ini = dir.join("kh-profile.ini");
    std::fs::write(&ctl, "save.dir=saves_other\n").unwrap();
    assert_eq!(resolve(&ctl), ctl, "only the .ctl");
    assert_eq!(resolve(&ini), ctl, "an .ini path falls back to the .ctl");
    std::fs::write(&ini, "[Save]\nsave.dir = saves_other\n").unwrap();
    assert_eq!(resolve(&ctl), ini, "the converted .ini wins over the old path");
    assert_eq!(resolve(&ini), ini);
    std::fs::remove_file(&ctl).unwrap();
    assert_eq!(resolve(&ctl), ini, "the .ctl renamed away: the .ini");
    let cfg = dir.join("kh-er.cfg");
    assert_eq!(resolve(&cfg), cfg);
    let _ = std::fs::remove_dir_all(&dir);
}
