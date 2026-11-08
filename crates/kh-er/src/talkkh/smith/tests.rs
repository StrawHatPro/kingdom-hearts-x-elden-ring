//! (P7 SMITHS) The rules of smith.rs against made-up commands, then OFFLINE AGAINST THE GAME'S OWN TALK SCRIPTS: every .esd
//! of build/sora_tmp/er/conv/game/esd (read-only copies; skipped when that folder is not there) is read here, the scripts
//! that run command 24 are found by the command, and their own talk-list commands, their 24 and the query-59 wait after it
//! are fed through the hooks' rules (ShopTalk with its SmithTalk, then the CV2 adapter's Talks) as the game would run them.

use super::super::rules::{cmd, env, Decision, Talks, Texts, Val};
use super::super::shop::ShopTalk;
use super::*;

const HEWG: i32 = 213_006_000;
const HEWG_HOLD: i32 = 213_001_110;
const IJI: i32 = 224_006_000;
const TABLE: i32 = 602_006_000;
const KALE: i32 = 800_006_000;
const LEAVE: i32 = super::super::rules::MSG_LEAVE;

fn sheet() -> SmithRules {
    let r = SmithRules::parse(kh_host::shop::SHEET);
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    r
}

fn on() -> SmithTalk {
    SmithTalk { on: true, rules: sheet(), ..SmithTalk::default() }
}

/// The hooks' order for a command: ShopTalk (it asks SmithTalk first), then the CV2 adapter.
fn shop() -> ShopTalk {
    ShopTalk { menu_on: true, smith_block: true, smith_ids: sheet().talk_ids, smith: on(), ..ShopTalk::default() }
}

/// The compiled sheet's rows are the scan's (make_shop_sheet.js).
#[test]
fn the_sheet_has_the_scanned_smith_rows() {
    let r = sheet();
    assert_eq!(r.open_enhance, 24);
    assert_eq!(r.menu_types, vec![9, 25], "Hewg / Iji c1_24(0) -> type 9, the Smithing Table c1_24(1) -> type 25");
    assert_eq!(r.swallow, vec![22, 46, 48, 143]);
    assert_eq!(r.trim, vec![20_000_011, 22_130_003, 22_130_018, 22_240_020], "Sell, Ashes of War, Ash of War duplication, Purchase");
    assert_eq!(r.talk_ids, vec![HEWG_HOLD, HEWG, IJI, TABLE]);
    assert!(SmithRules::parse("{}").problems.len() == 2 && SmithRules::parse("nope").problems.len() == 1);
}

/// (THE USER 2026-11-03) COMMAND 24 OPENS THE FORGE in any character's script: not run, a request; query 59 reads the menu
/// open (its types, "any") for that talk until the Forge is done, then the game's own answer; a script the sheet does not
/// list becomes a smith; system scripts and `forge.kh = 0` are left alone.
#[test]
fn command_24_is_the_keyblade_forge_in_any_character_script() {
    for (talk, kind, wait) in [(HEWG, 0, 9), (HEWG_HOLD, 0, 9), (IJI, 0, 9), (TABLE, 1, 25)] {
        let mut t = on();
        assert_eq!(t.on_event(talk, 7, 24, &[Val::Int(kind)]), Some(Decision::Swallow), "{talk}: Elden Ring's menu is not opened");
        let r = t.request.clone().expect("a Forge request");
        assert_eq!((r.talk_id, r.ins, r.kind, r.opened), (talk, 7, kind, false));
        assert_eq!(t.on_env(talk, env::MENU_OPEN, &[Val::Int(wait), Val::Int(0)]), Some(1), "{talk}: its wait reads the menu open");
        assert_eq!(t.on_env(talk, env::MENU_OPEN, &[Val::Int(-1)]), Some(1), "any");
        assert_eq!(t.on_env(talk, env::MENU_OPEN, &[Val::Int(1), Val::Int(0)]), None, "its talk list's own query is the game's");
        assert_eq!(t.notes.iter().filter(|n| n.contains("menu type 1 ")).count(), 1, "told");
        t.on_env(talk, env::MENU_OPEN, &[Val::Int(1)]);
        assert_eq!(t.notes.iter().filter(|n| n.contains("menu type 1 ")).count(), 1, "told once");
        assert_eq!(t.on_env(KALE, env::MENU_OPEN, &[Val::Int(9)]), None, "another talk");
        assert_eq!(t.on_env(talk, env::LIST_RESULT, &[]), None, "only query 59");
        assert!(t.done("closed").is_some());
        assert_eq!(t.on_env(talk, env::MENU_OPEN, &[Val::Int(wait), Val::Int(0)]), None, "closed: the game's answer sends the script back to its list");
        assert!(t.done("again").is_none(), "done once");
    }
    // a smith no scan knows (a DLC / later script): recognised by the command, its options trimmed from then on
    let other = 999_006_000;
    let mut t = on();
    assert_eq!(t.on_event(other, 1, cmd::ADD_LIST, &[Val::Int(4), Val::Int(20_000_011), Val::Int(-1)]), None, "not known as a smith yet");
    assert_eq!(t.on_event(other, 1, 24, &[Val::Int(0)]), Some(Decision::Swallow));
    assert!(t.is_smith(other) && t.notes.iter().any(|n| n.contains("a smith from now on")));
    assert_eq!(t.on_event(other, 1, cmd::ADD_LIST, &[Val::Int(4), Val::Int(20_000_011), Val::Int(-1)]), Some(Decision::Swallow), "trimmed now");
    // the grace's menu (a system script) and the key off
    assert_eq!(on().on_event(1000, 1, 24, &[Val::Int(0)]), None);
    let mut off = SmithTalk { on: false, ..on() };
    assert_eq!(off.on_event(HEWG, 1, 24, &[Val::Int(0)]), None, "forge.kh = 0");
    assert_eq!(off.on_event(HEWG, 1, cmd::ADD_LIST, &[Val::Int(2), Val::Int(22_130_003), Val::Int(-1)]), None);
}

/// THE TRIM: only in smiths' scripts, only the sheet's texts, 19 and 149 (and a 19 still carrying its bank-5 condition);
/// the safety net; Kale's own "Sell" stays.
#[test]
fn the_smiths_keep_only_upgrades_and_talk() {
    let mut t = on();
    let add = |t: &mut SmithTalk, talk, idx, msg| t.on_event(talk, 1, cmd::ADD_LIST, &[Val::Int(idx), Val::Int(msg), Val::Int(-1)]);
    assert_eq!(add(&mut t, HEWG, 1, 22_130_001), None, "Strengthen armament");
    assert_eq!(add(&mut t, HEWG, 2, 22_130_003), Some(Decision::Swallow), "Ashes of War");
    assert_eq!(add(&mut t, HEWG, 20, 22_130_018), Some(Decision::Swallow), "Ash of War duplication");
    assert_eq!(add(&mut t, HEWG, 4, 20_000_011), Some(Decision::Swallow), "Sell");
    assert_eq!(add(&mut t, HEWG, 5, 22_130_004), None, "About prayer");
    assert_eq!(add(&mut t, HEWG, 99, LEAVE), None, "Leave");
    assert_eq!(add(&mut t, IJI, 20, 22_240_020), Some(Decision::Swallow), "Purchase");
    assert_eq!(add(&mut t, IJI, 1, 22_240_000), None, "Strengthen armaments");
    assert_eq!(add(&mut t, KALE, 2, 20_000_011), None, "Kale's Sell is not a smith's");
    assert_eq!(t.on_event(HEWG, 1, cmd::ADD_LIST_ALT, &[Val::Int(4), Val::Int(20_000_011), Val::Int(-1), Val::Int(0), Val::Int(0)]), Some(Decision::Swallow), "149");
    assert_eq!(t.on_event(HEWG, 1, cmd::ADD_LIST, &[Val::Int(1), Val::Int(20), Val::Int(22_130_018), Val::Int(-1)]), Some(Decision::Swallow), "a 19 with its condition first");
    for id in [22, 46, 48, 143] {
        assert_eq!(t.on_event(HEWG, 1, id, &[Val::Int(-1), Val::Int(-1)]), Some(Decision::Swallow), "the safety net: {id}");
        assert_eq!(t.on_event(KALE, 1, id, &[Val::Int(-1), Val::Int(-1)]), None, "not in a merchant's script: {id}");
    }
    assert_eq!(t.on_event(HEWG, 1, cmd::SHOW_SHOP_MESSAGE, &[Val::Int(1)]), None, "his talk list is the adapter's");
    assert_eq!(t.on_event(HEWG, 1, 141, &[Val::Int(9)]), None);
    assert!(t.request.is_none(), "no Forge without 24");
}

/// (SHOP retired for the smiths) Through ShopTalk, as the hooks call it: with the Forge on the smiths' prompt (query 56) is
/// the game's (they can be talked to), 24 is the Forge's request (not the old refusal), a smith's 22 / 46 never opens the
/// KH shop; Kale's windows are still the KH shop's; the Forge off: the older rule exactly.
#[test]
fn the_shop_talk_hands_the_smiths_to_the_forge() {
    let mut s = shop();
    assert_eq!(s.before_env(HEWG, 56), None, "the smiths can be talked to");
    assert_eq!(s.on_event(HEWG, 3, 24, &[Val::Int(0)]), Some(Decision::Swallow));
    assert!(s.smith.request.is_some() && s.smith_seen.is_empty(), "the Forge's request, not the refusal");
    assert_eq!(s.before_env(HEWG, 56), None, "still not refused");
    assert_eq!(s.on_env(HEWG, env::MENU_OPEN, &[Val::Int(9), Val::Int(0)]), Some(1), "the menu reads open through ShopTalk");
    assert_eq!(s.on_event(IJI, 3, 22, &[Val::Int(100_225), Val::Int(100_249)]), Some(Decision::Swallow));
    assert!(s.request.is_none(), "a smith's buy window is not the KH shop's");
    assert_eq!(s.on_event(KALE, 3, 22, &[Val::Int(100_500), Val::Int(100_524)]), Some(Decision::Swallow));
    assert!(s.request.as_ref().is_some_and(|r| r.talk_id == KALE), "Kale's is");
    let mut old = ShopTalk { smith: SmithTalk { on: false, ..on() }, ..shop() };
    assert_eq!(old.before_env(HEWG, 56), Some(0), "forge.kh = 0: npc.smith_talk decides as before");
    assert_eq!(old.on_event(HEWG, 3, 24, &[Val::Int(0)]), Some(Decision::Swallow));
    assert!(old.smith.request.is_none());
}

// ------------------------------------------------------------------------------------------- the game's own talk scripts

/// A token of an ESD expression (esdparse.js's reading: bytes 0x00 .. 0x7F the integer byte - 64, 0x82 an int32, 0x84 ..
/// 0x8A a call, 0x8D negate).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tok {
    N(i64),
    F,
    Call(u8),
    Op(u8),
}

#[derive(Clone, Debug)]
struct Cmd {
    bank: i32,
    id: i32,
    args: Vec<Vec<Tok>>,
}

impl Cmd {
    /// The command's own arguments (bank 5: the condition first, not the command's).
    fn own(&self) -> &[Vec<Tok>] {
        if self.bank == 5 { self.args.get(1..).unwrap_or(&[]) } else { &self.args }
    }
    fn is(&self, id: i32) -> bool {
        (self.bank == 1 || self.bank == 5) && self.id == id
    }
    /// Its constant integer arguments as the talk-command function would read them.
    fn vals(&self) -> Vec<Val> {
        self.own().iter().map(|a| const_int(a).map_or(Val::Other, |v| Val::Int(v as i32))).collect()
    }
}

#[derive(Clone, Debug)]
struct Cond {
    expr: Vec<Tok>,
    pass: Vec<Cmd>,
    subs: Vec<Cond>,
}

#[derive(Clone, Debug)]
struct State {
    enter: Vec<Cmd>,
    conds: Vec<Cond>,
    other: Vec<Cmd>,
}

impl State {
    fn all(&self) -> Vec<&Cmd> {
        fn walk<'a>(c: &'a Cond, out: &mut Vec<&'a Cmd>) {
            out.extend(c.pass.iter());
            for s in &c.subs {
                walk(s, out);
            }
        }
        let mut out: Vec<&Cmd> = self.enter.iter().chain(self.other.iter()).collect();
        for c in &self.conds {
            walk(c, &mut out);
        }
        out
    }
}

fn const_int(t: &[Tok]) -> Option<i64> {
    match t {
        [Tok::N(v)] => Some(*v),
        [Tok::N(v), Tok::Op(0x8D)] => Some(-*v),
        _ => None,
    }
}

/// THE ESD READER (64-bit talk scripts; the layout of build/sora_tmp/er/conv/tools/esdparse.js, ported).
struct Esd<'a>(&'a [u8]);

const D: usize = 0x6C;

impl Esd<'_> {
    fn q(&self, o: usize) -> Option<usize> {
        let v = i64::from_le_bytes(self.0.get(o..o + 8)?.try_into().ok()?);
        usize::try_from(v).ok()
    }
    /// (offset, count) at o / o + 8: an empty list's offset may be -1 (read as nothing).
    fn span(&self, o: usize) -> Option<(usize, usize)> {
        let n = i64::from_le_bytes(self.0.get(o + 8..o + 16)?.try_into().ok()?);
        if n <= 0 {
            return Some((0, 0));
        }
        Some((self.q(o)?, usize::try_from(n).ok()?))
    }
    fn i32(&self, o: usize) -> Option<i32> {
        Some(i32::from_le_bytes(self.0.get(o..o + 4)?.try_into().ok()?))
    }
    fn expr(&self, off: usize, len: usize) -> Option<Vec<Tok>> {
        let mut out = Vec::new();
        let (mut o, end) = (D + off, D + off + len);
        while o < end {
            let c = *self.0.get(o)?;
            o += 1;
            match c {
                0x00..=0x7F => out.push(Tok::N(c as i64 - 64)),
                0x80 => {
                    out.push(Tok::F);
                    o += 4;
                }
                0x81 => {
                    out.push(Tok::F);
                    o += 8;
                }
                0x82 => {
                    out.push(Tok::N(self.i32(o)? as i64));
                    o += 4;
                }
                0x84..=0x8A => out.push(Tok::Call(c - 0x84)),
                0xA1 => break,
                0xA5 => {
                    while u16::from_le_bytes(self.0.get(o..o + 2)?.try_into().ok()?) != 0 {
                        o += 2;
                    }
                    o += 2;
                    out.push(Tok::F);
                }
                _ => out.push(Tok::Op(c)),
            }
        }
        Some(out)
    }
    fn commands(&self, (off, n): (usize, usize)) -> Option<Vec<Cmd>> {
        (0..n)
            .map(|k| {
                let o = D + off + k * 0x18;
                let (bank, id, (ao, an)) = (self.i32(o)?, self.i32(o + 4)?, self.span(o + 8)?);
                let args = (0..an).map(|a| self.expr(self.q(D + ao + a * 0x10)?, self.i32(D + ao + a * 0x10 + 8)? as usize)).collect::<Option<Vec<_>>>()?;
                Some(Cmd { bank, id, args })
            })
            .collect()
    }
    fn condition(&self, o: usize, depth: u8) -> Option<Cond> {
        let (pass, (so, sn), (eo, el)) = (self.commands(self.span(o + 8)?)?, self.span(o + 24)?, self.span(o + 40)?);
        let subs = if depth < 8 { (0..sn).map(|s| self.condition(D + self.q(D + so + s * 8)?, depth + 1)).collect::<Option<Vec<_>>>()? } else { Vec::new() };
        Some(Cond { expr: self.expr(eo, el)?, pass, subs })
    }
    /// The groups' states (group order, state order).
    fn groups(&self) -> Option<Vec<Vec<State>>> {
        if self.0.get(..4)? != b"fsSL" {
            return None;
        }
        let (go, gn) = self.span(D + 0x18)?;
        (0..gn)
            .map(|g| {
                let o = D + go + g * 0x20;
                let (so, sn) = self.span(o + 8)?;
                (0..sn)
                    .map(|s| {
                        let o = D + so + s * 0x48;
                        let (co, cn) = self.span(o + 8)?;
                        let enter = self.commands(self.span(o + 24)?)?;
                        let mut other = self.commands(self.span(o + 40)?)?;
                        other.extend(self.commands(self.span(o + 56)?)?);
                        let conds = (0..cn).map(|c| self.condition(D + self.q(D + co + c * 8)?, 0)).collect::<Option<Vec<_>>>()?;
                        Some(State { enter, conds, other })
                    })
                    .collect()
            })
            .collect()
    }
}

/// Every script of the read-only copies: talk id -> its groups (the first copy of a talk id).
fn game_scripts() -> Option<Vec<(i32, Vec<Vec<State>>)>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/conv/game/esd");
    let mut out: Vec<(i32, Vec<Vec<State>>)> = Vec::new();
    for map in std::fs::read_dir(&dir).ok()?.flatten() {
        for f in std::fs::read_dir(map.path()).ok()?.flatten() {
            let name = f.file_name().to_string_lossy().to_string();
            let Some(id) = name.strip_prefix('t').and_then(|n| n.strip_suffix(".esd")).and_then(|n| n.parse::<i32>().ok()) else { continue };
            if out.iter().any(|(t, _)| *t == id) {
                continue;
            }
            let bytes = std::fs::read(f.path()).ok()?;
            let groups = Esd(&bytes).groups().unwrap_or_else(|| panic!("{name}: not read"));
            out.push((id, groups));
        }
    }
    out.sort_by_key(|(id, _)| *id);
    Some(out)
}

struct NoTexts;
impl Texts for NoTexts {
    fn flag(&self, _: u32) -> Option<bool> {
        None
    }
    fn option_text(&self, msg: i32) -> Option<String> {
        Some(format!("text {msg}"))
    }
    fn dialog_text(&self, _: i32) -> Option<String> {
        None
    }
}

/// One command through the hooks' rules in the hooks' order (ShopTalk with SmithTalk first, then the CV2 adapter).
fn run(s: &mut ShopTalk, talks: &mut Talks, talk: i32, c: &Cmd) -> Decision {
    let args = c.vals();
    s.on_event(talk, 7, c.id, &args).unwrap_or_else(|| talks.on_event(talk, 7, 8, c.id, &args, &NoTexts))
}

/// (P7, the user 2026-11-03) THE FOUR SMITHS' OWN SCRIPTS, OFFLINE: found by their command 24 among all 365; each group's
/// talk list run through the rules (every conditional option taken as shown): the trimmed texts never reach the list,
/// everything else does with its own index; B is "Leave"; "Strengthen" reads back its index 1; their 24 is the Forge's
/// request and the wait after it (its query 59 type) reads the menu open until the Forge is done; every other window
/// their scripts can open is swallowed; Kale's "Sell" and sell window are untouched.
#[test]
fn the_smiths_own_scripts_trim_and_open_the_forge() {
    let Some(scripts) = game_scripts() else {
        eprintln!("SKIPPED: no build/sora_tmp/er/conv/game/esd");
        return;
    };
    assert!(scripts.len() >= 359, "{} scripts", scripts.len());
    let r = sheet();
    let runs_24 = |groups: &Vec<Vec<State>>| groups.iter().flatten().any(|st| st.all().iter().any(|c| c.is(24)));
    let smiths: Vec<i32> = scripts.iter().filter(|(_, g)| runs_24(g)).map(|(id, _)| *id).collect();
    assert_eq!(smiths, r.talk_ids, "the scripts that run 24 are the sheet's");
    let strengthen = [(HEWG, 22_130_001), (HEWG_HOLD, 22_130_001), (IJI, 22_240_000), (TABLE, 26_020_000)];
    let mut trimmed_seen: Vec<i32> = Vec::new();
    for (talk, groups) in scripts.iter().filter(|(id, _)| smiths.contains(id)) {
        let talk = *talk;
        let mut kept_all: Vec<(i32, i32)> = Vec::new();
        for (g, states) in groups.iter().enumerate() {
            let adds: Vec<&Cmd> = states.iter().flat_map(|st| st.enter.iter()).filter(|c| c.is(cmd::ADD_LIST) || c.is(cmd::ADD_LIST_ALT)).collect();
            if adds.is_empty() {
                continue;
            }
            let (mut s, mut talks) = (shop(), Talks::new());
            talks.ready = true;
            talks.on_event(talk, 7, 8, cmd::CLEAR_LIST, &[], &NoTexts);
            let mut expect: Vec<(i32, i32)> = Vec::new();
            for c in &adds {
                let v = c.vals();
                let (idx, msg) = (v[0].int().expect("index"), v[1].int().expect("text"));
                let d = run(&mut s, &mut talks, talk, c);
                if r.trim.contains(&msg) {
                    assert_eq!(d, Decision::Swallow, "t{talk} x{g}: option {idx} text {msg} is trimmed");
                    trimmed_seen.push(msg);
                } else {
                    assert_eq!(d, Decision::Pass, "t{talk} x{g}: option {idx} text {msg} is the game's to add");
                    expect.retain(|o| o.0 != idx);
                    expect.push((idx, msg));
                }
            }
            assert_eq!(talks.on_event(talk, 7, 8, cmd::SHOW_SHOP_MESSAGE, &[Val::Int(1)], &NoTexts), Decision::Swallow, "t{talk} x{g}: the list in the bubble");
            let ask = talks.ask.clone().expect("asked");
            let got: Vec<(i32, i32)> = ask.options.iter().map(|o| (o.index, o.msg)).collect();
            assert_eq!(got, expect, "t{talk} x{g}: the options left, each with its own index");
            assert!(got.iter().all(|(_, m)| !r.trim.contains(m)));
            // B = Leave, read back as Leave's own index; "Strengthen" as its own (1), whatever was removed before it
            if let Some(k) = ask.cancel {
                assert_eq!(got[k].1, LEAVE);
                let mut t2 = talks.clone();
                assert_eq!(t2.answer(ask.serial, k), Some(got[k].0), "t{talk} x{g}: B reads Leave's index");
            }
            if let Some(k) = got.iter().position(|o| strengthen.contains(&(talk, o.1))) {
                assert_eq!(talks.answer(ask.serial, k), Some(1), "t{talk} x{g}: Strengthen reads back 1");
                assert_eq!(talks.on_env(talk, env::LIST_RESULT, &[]), Some(1));
            }
            kept_all.extend(got);
        }
        let (_, s_msg) = strengthen.iter().find(|(t, _)| *t == talk).copied().expect("a known smith");
        assert!(kept_all.contains(&(1, s_msg)), "t{talk}: Strengthen stays, index 1");
        assert!(kept_all.iter().any(|(_, m)| *m == LEAVE), "t{talk}: Leave stays");
        // its 24 and the wait right after it
        let mut waits = 0;
        for st in groups.iter().flatten() {
            let Some(c24) = st.all().into_iter().find(|c| c.is(24)).cloned() else { continue };
            let (mut s, mut talks) = (shop(), Talks::new());
            assert_eq!(run(&mut s, &mut talks, talk, &c24), Decision::Swallow, "t{talk}: Elden Ring's reinforcement menu is not opened");
            assert!(s.smith.request.is_some());
            for cond in &st.conds {
                for w in cond.expr.windows(4) {
                    if let [Tok::N(59), Tok::N(kind), Tok::N(_), Tok::Call(2)] = *w {
                        waits += 1;
                        let q = [Val::Int(kind as i32), Val::Int(0)];
                        assert!(r.menu_types.contains(&(kind as i32)), "t{talk}: its wait's type {kind} is in the sheet");
                        assert_eq!(s.on_env(talk, env::MENU_OPEN, &q), Some(1), "t{talk}: it waits while the Forge is up");
                        s.smith.done("closed");
                        assert_eq!(s.on_env(talk, env::MENU_OPEN, &q), None, "t{talk}: then back to its talk list");
                    }
                }
            }
        }
        assert!(waits >= 1, "t{talk}: a wait after 24");
        // every other window its script can open is swallowed (bank 1 or 5, wherever it is)
        for c in groups.iter().flatten().flat_map(|st| st.all()) {
            if [22, 46, 48, 142, 143, 144, 146, 148, 111].iter().any(|w| c.is(*w)) {
                let (mut s, mut talks) = (shop(), Talks::new());
                assert_eq!(run(&mut s, &mut talks, talk, c), Decision::Swallow, "t{talk}: command {} is not run", c.id);
                assert!(s.request.is_none(), "t{talk}: no KH shop for a smith");
            }
        }
    }
    trimmed_seen.sort();
    trimmed_seen.dedup();
    assert_eq!(trimmed_seen, r.trim, "every trimmed text is one the smiths' scripts add");
    // Kale (a merchant): his "Sell" stays and his sell window is the KH shop's
    if let Some((_, groups)) = scripts.iter().find(|(id, _)| *id == KALE) {
        let (mut s, mut talks) = (shop(), Talks::new());
        for c in groups.iter().flatten().flat_map(|st| st.enter.iter()).filter(|c| c.is(cmd::ADD_LIST)) {
            assert_eq!(run(&mut s, &mut talks, KALE, c), Decision::Pass, "Kale's options are his");
        }
        let sell = groups.iter().flatten().flat_map(|st| st.all()).find(|c| c.is(46)).cloned().expect("Kale sells");
        assert_eq!(run(&mut s, &mut talks, KALE, &sell), Decision::Swallow);
        assert!(s.request.as_ref().is_some_and(|q| q.sell), "the KH shop's sell tab");
    }
}
