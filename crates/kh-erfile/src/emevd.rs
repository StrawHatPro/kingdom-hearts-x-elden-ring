//! (ZETTA 2026-10-24) ELDEN RING'S MAP EVENTS (EMEVD, 64-bit layout), READ-ONLY: the events, their instructions, every
//! initialized instance of a parameterized event with its arguments put in, and THE TWO-PHASE BOSS PLAN Zettaflare's whole
//! kill follows (the user: "killing a boss with a second phase during it's first phase with Zettaflare should just kill the
//! boss entirely skipping the second phase. The cutscene should not play ... there can be no progression blockers"). No
//! per-boss table: everything is read from the boss's own map events.
//!
//! The instructions this reads (bank[index], arguments as 32-bit words; `cond` is a condition group):
//! - 2003[12] HandleBossDefeat(entity, banner): THE DEFEAT EVENT of a boss (its flags 2003[66] / 2003[69] after it);
//! - 4[14] IfCharacterHPValue(cond, entity, comparison, value, ...): comparison 5 = "at or below"; value 0 = its death,
//!   value > 0 = a PHASE THRESHOLD (the boss is immortal there: 2004[12] SetCharacterImmortality(entity, 1));
//! - 4[2] IfCharacterHPRatio(cond, entity, comparison, ratio as a float, ...): the same, as a share of its HP;
//! - 4[0] IfCharacterDeadAlive(cond, entity, 1 = dead, ...);
//! - 3[0] IfEventFlag(cond, state, type, flag), 2003[66] SetEventFlag(type, flag, state), 2003[69] (network) the same;
//! - 2002[*] the cutscene instructions (2002[3], [11], [13] PlayCutscene...: the first argument is the cutscene id);
//! - 2004[5] ChangeCharacterEnableState(entity, 1 = enabled);
//! - 2003[14] / 2003[76]: the player warped / the map's place changed (a transition the story needs).
//! - 2000[0] InitializeEvent(slot, event, args...).

/// One instruction: bank, index, its arguments (raw bytes and 32-bit words).
#[derive(Clone, Debug, PartialEq)]
pub struct Instr {
    pub bank: i32,
    pub index: i32,
    pub args: Vec<u8>,
    pub w: Vec<i32>,
}

impl Instr {
    pub fn is(&self, bank: i32, index: i32) -> bool {
        self.bank == bank && self.index == index
    }
    pub fn word(&self, k: usize) -> i32 {
        self.w.get(k).copied().unwrap_or(0)
    }
}

/// One event (or one initialized instance of a parameterized event): its id, restart behaviour, instructions.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub id: u32,
    pub slot: i32,
    pub rest: i32,
    pub ins: Vec<Instr>,
    params: Vec<(usize, usize, usize, usize)>,
}

fn words(b: &[u8]) -> Vec<i32> {
    b.chunks_exact(4).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

/// The events of an EMEVD file (decompressed).
pub fn parse(b: &[u8]) -> Result<Vec<Event>, String> {
    if b.len() < 0x90 || &b[0..4] != b"EVD\0" {
        return Err("not an EMEVD file".to_string());
    }
    let q = |o: usize| -> Result<i64, String> { b.get(o..o + 8).map(|s| i64::from_le_bytes(s.try_into().unwrap())).ok_or(format!("EMEVD: read past its end at {o:#x}")) };
    let d = |o: usize| -> Result<i32, String> { b.get(o..o + 4).map(|s| i32::from_le_bytes(s.try_into().unwrap())).ok_or(format!("EMEVD: read past its end at {o:#x}")) };
    let h: Vec<i64> = (0..14).map(|k| q(0x10 + k * 8)).collect::<Result<_, _>>()?;
    let (ev_n, ev_o, ins_o, par_o, arg_o) = (h[0] as usize, h[1] as usize, h[3] as usize, h[9] as usize, h[13] as usize);
    if ev_n > 100_000 {
        return Err("EMEVD: implausible event count".to_string());
    }
    let mut out = Vec::with_capacity(ev_n);
    for e in 0..ev_n {
        let eo = ev_o + e * 0x30;
        let (id, n, io, pn, po, rest) = (q(eo)?, q(eo + 8)? as usize, q(eo + 16)? as usize, q(eo + 24)? as usize, q(eo + 32)? as usize, d(eo + 40)?);
        if n > 10_000 || pn > 10_000 {
            return Err(format!("EMEVD: implausible event {id}"));
        }
        let mut ins = Vec::with_capacity(n);
        for k in 0..n {
            let x = ins_o + io + k * 0x20;
            let (bank, index, alen, aoff) = (d(x)?, d(x + 4)?, q(x + 8)?, q(x + 16)?);
            let args = if alen > 0 && aoff >= 0 { b.get(arg_o + aoff as usize..arg_o + aoff as usize + alen as usize).ok_or("EMEVD: arguments past its end")?.to_vec() } else { Vec::new() };
            ins.push(Instr { bank, index, w: words(&args), args });
        }
        let mut params = Vec::with_capacity(pn);
        for p in 0..pn {
            let x = par_o + po + p * 0x20;
            params.push((q(x)? as usize, q(x + 8)? as usize, q(x + 16)? as usize, d(x + 24)?.max(0) as usize));
        }
        out.push(Event { id: id as u32, slot: 0, rest, ins, params });
    }
    Ok(out)
}

/// Every event as it runs: the plain ones, and each 2000[0] InitializeEvent(slot, event, args) of a parameterized one with
/// its arguments put in.
pub fn instances(events: &[Event]) -> Vec<Event> {
    let mut out: Vec<Event> = events.iter().filter(|e| e.params.is_empty()).cloned().collect();
    for e in events {
        for i in e.ins.iter().filter(|i| i.is(2000, 0)) {
            let Some(t) = events.iter().find(|x| x.id as i32 == i.word(1) && !x.params.is_empty()) else { continue };
            let init = i.args.get(8..).unwrap_or(&[]);
            let ins = t
                .ins
                .iter()
                .enumerate()
                .map(|(k, x)| {
                    let mut args = x.args.clone();
                    for &(pi, tgt, src, len) in &t.params {
                        if pi == k && tgt + len <= args.len() && src + len <= init.len() {
                            args[tgt..tgt + len].copy_from_slice(&init[src..src + len]);
                        }
                    }
                    Instr { bank: x.bank, index: x.index, w: words(&args), args }
                })
                .collect();
            out.push(Event { id: t.id, slot: i.word(0), rest: t.rest, ins, params: Vec::new() });
        }
    }
    out
}

/// How a two-phase boss's final form comes (read from its events).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanShape {
    /// The same character goes on (its phase threshold, a cutscene, its state changed): Malenia.
    SameCharacter,
    /// Another character is enabled by the phase change: Godfrey -> Hoarah Loux.
    NextCharacter,
}

/// WHAT ZETTAFLARE'S WHOLE KILL OF A BOSS DOES, read from its map's events.
#[derive(Clone, Debug, PartialEq)]
pub struct BossPlan {
    /// The entity Zettaflare hit (the first form).
    pub entity: u32,
    pub shape: PlanShape,
    /// The phase-change events: never run (their cutscene never starts, the second form is never made).
    pub block: Vec<u32>,
    /// Their cutscenes (what never plays).
    pub cutscenes: Vec<i32>,
    /// The entity whose death the defeat event waits on (the final form; the entity itself when SameCharacter).
    pub final_entity: u32,
    /// (2026-10-24) EVERY character the defeat event waits on (its HP at 0 / its death; the player left out): all are killed
    /// (the Valiant Gargoyles' defeat waits on both).
    pub finals: Vec<u32>,
    /// The defeat event and the flags it sets (the game's own event runs: banner, flags, rewards).
    pub defeat_event: u32,
    pub defeat_flags: Vec<i32>,
    /// STORY-CRITICAL: the defeat leads into a cutscene, or the change of phase moves the player or the map (an ending, a
    /// transition): the game's own phase change is left to play (the user: "Never block the ending sequence or a story
    /// transition").
    pub story_critical: bool,
    /// Why (for the log).
    pub why: String,
}

/// (2026-10-24) A PHASE THRESHOLD on `entity`: 4[14] IfCharacterHPValue or 4[2] IfCharacterHPRatio (its value a float:
/// the serpent before Rykard at 0.01, the Promised Consort at 0.65), "at or below" (5) a value above 0.
pub fn threshold(i: &Instr, entity: i32) -> bool {
    (i.is(4, 14) || i.is(4, 2)) && i.word(1) == entity && entity != 10000 && i.word(2) == 5 && i.word(3) > 0
}

/// THE PLAN for a boss whose first form is `entity`, from its map's event instances; None: not a boss with a second phase
/// in these events (the kill is a plain kill).
pub fn boss_plan(inst: &[Event], entity: u32) -> Option<BossPlan> {
    let e = entity as i32;
    // the defeat events: HandleBossDefeat(X)
    let defeats: Vec<(&Event, i32)> = inst.iter().filter_map(|ev| ev.ins.iter().find(|i| i.is(2003, 12)).map(|i| (ev, i.word(0)))).collect();
    let flags_set = |ev: &Event| -> Vec<i32> { ev.ins.iter().filter(|i| (i.is(2003, 66) || i.is(2003, 69)) && i.word(2) == 1).map(|i| i.word(1)).collect() };
    let cut_ids = |ev: &Event| -> Vec<i32> { ev.ins.iter().filter(|i| i.bank == 2002).map(|i| i.word(0)).collect() };
    let moves = |ev: &Event| ev.ins.iter().any(|i| i.is(2003, 14) || i.is(2003, 76));
    // 1. a PHASE THRESHOLD on it: an HP at or below a value above 0, in an event that is not a defeat event
    //    (2026-10-24) ONLY WITH A CHANGE IN IT, or in an event waiting a flag it sets: a cutscene, a character enabled, the
    //    immortality taken off, a new boss bar; a threshold that only sets a flag nothing changes on (the second music of
    //    a catacomb's boss at 60 %), or a special effect, is no phase change
    //    (the change AFTER its condition: a fight's start that shows the bar and only later waits a threshold is no change)
    let change = |i: &Instr| i.bank == 2002 || (i.is(2004, 5) && i.word(1) == 1) || (i.is(2004, 12) && i.word(1) == 0) || (i.is(2003, 11) && i.word(0) == 1);
    let changes_after = |ev: &Event, cond: &dyn Fn(&Instr) -> bool| ev.ins.iter().position(cond).is_some_and(|k| ev.ins[k + 1..].iter().any(change));
    let mut phase: Vec<&Event> = Vec::new();
    for ev in inst.iter().filter(|ev| !ev.ins.iter().any(|i| i.is(2003, 12))).filter(|ev| ev.ins.iter().any(|i| threshold(i, e))) {
        if changes_after(ev, &|i| threshold(i, e)) {
            phase.push(ev);
            continue;
        }
        let set = flags_set(ev);
        let next: Vec<&Event> = inst.iter().filter(|n| !n.ins.iter().any(|i| i.is(2003, 12)) && changes_after(n, &|i| i.is(3, 0) && set.contains(&i.word(1)) && (i.word(0) >> 8) & 0xff == 1)).collect();
        if !next.is_empty() {
            phase.push(ev);
            phase.extend(next);
        }
    }
    // 2. its death starting a change by a flag (Radagon -> the Elden Beast): an event waiting its death that sets flags,
    //    and the events waiting those flags that play a cutscene or enable a character
    let mut via_death = false;
    if phase.is_empty() && !defeats.iter().any(|(_, x)| *x == e) {
        for ev in inst.iter().filter(|ev| !ev.ins.iter().any(|i| i.is(2003, 12)) && ev.ins.iter().any(|i| i.is(4, 0) && i.word(1) == e && i.word(2) == 1)) {
            // (it changes the boss itself: Rennala)
            if ev.ins.iter().any(|i| i.bank == 2002 || (i.is(2004, 5) && i.word(1) == 1)) {
                phase.push(ev);
                via_death = true;
                continue;
            }
            let set = flags_set(ev);
            for next in inst.iter().filter(|n| n.ins.iter().any(|i| i.is(3, 0) && set.contains(&i.word(1)) && (i.word(0) >> 8) & 0xff == 1)) {
                if next.ins.iter().any(|i| i.bank == 2002) {
                    phase.push(ev);
                    phase.push(next);
                    via_death = true;
                }
            }
        }
    }
    if phase.is_empty() {
        return None;
    }
    // the final form: a defeat of this very entity, else of a character a phase event enables
    let enabled: Vec<i32> = phase.iter().flat_map(|ev| ev.ins.iter().filter(|i| i.is(2004, 5) && i.word(1) == 1).map(|i| i.word(0))).collect();
    let (defeat, shape) = if let Some(d) = defeats.iter().find(|(_, x)| *x == e) {
        (d, PlanShape::SameCharacter)
    } else if let Some(d) = defeats.iter().find(|(_, x)| enabled.contains(x)) {
        (d, PlanShape::NextCharacter)
    } else {
        return None;
    };
    let cutscenes: Vec<i32> = phase.iter().flat_map(|ev| cut_ids(ev)).collect();
    let post_cut = cut_ids(defeat.0);
    let moved = phase.iter().any(|ev| moves(ev));
    let story_critical = !post_cut.is_empty() || moved;
    let why = if !post_cut.is_empty() {
        format!("its defeat event plays cutscene(s) {post_cut:?} (an ending / a transition)")
    } else if moved {
        "its phase change moves the player or the map".to_string()
    } else if via_death {
        "its second form comes on its death".to_string()
    } else {
        String::new()
    };
    let mut block: Vec<u32> = phase.iter().map(|ev| ev.id).collect();
    block.sort();
    block.dedup();
    // (2026-10-24) a threshold that turns the character's team (2004[2]: Patches' surrender at half his HP) is no boss's
    // phase: the game's own (the old route), never killed whole
    let surrender = phase.iter().any(|ev| ev.ins.iter().any(|i| i.is(2004, 2)));
    let (story_critical, why) = if surrender { (true, "a SURRENDER, not a phase (its team changes: 2004[2]): the game's own".to_string()) } else { (story_critical, why) };
    let mut finals: Vec<u32> = defeat.0.ins.iter().filter(|i| (i.is(4, 14) && i.word(2) == 5 && i.word(3) == 0) || (i.is(4, 0) && i.word(2) == 1)).map(|i| i.word(1)).filter(|x| *x > 0 && *x != 10000).map(|x| x as u32).collect();
    finals.sort();
    finals.dedup();
    if finals.is_empty() {
        finals.push(defeat.1 as u32);
    }
    Some(BossPlan { entity, shape, block, cutscenes, final_entity: defeat.1 as u32, finals, defeat_event: defeat.0.id, defeat_flags: flags_set(defeat.0), story_critical, why })
}

/// (2026-10-24) The cutscenes the defeat event of `entity` plays itself (an ending, a story beat: the Elden Beast's
/// 19000050, Fortissax's 12030021). Empty: none (or no defeat event of it here).
pub fn defeat_cutscenes(inst: &[Event], entity: u32) -> Vec<i32> {
    inst.iter()
        .filter(|ev| ev.ins.iter().any(|i| i.is(2003, 12) && i.word(0) == entity as i32))
        .flat_map(|ev| ev.ins.iter().filter(|i| i.bank == 2002).map(|i| i.word(0)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ins(bank: i32, index: i32, w: &[i32]) -> Instr {
        let args: Vec<u8> = w.iter().flat_map(|x| x.to_le_bytes()).collect();
        Instr { bank, index, w: w.to_vec(), args }
    }
    fn ev(id: u32, ins: Vec<Instr>) -> Event {
        Event { id, slot: 0, rest: 1, ins, params: Vec::new() }
    }

    /// (2026-10-24) the shapes, modelled on the install's events (m15, m11_05, m19): Malenia (the same character: her phase
    /// event blocked, her own death is the defeat), Godfrey (the next character: Hoarah Loux's death is the defeat),
    /// Radagon -> the Elden Beast (a flag chain into the ending: story-critical, left to the game), a plain boss (no plan)
    #[test]
    fn emevd_the_boss_plans_of_the_shapes() {
        // Malenia, m15_00_00_00: 15002811 waits HP(15000800) <= 1 -> cutscene 15000010; 15002800 the defeat
        let malenia = vec![
            ev(15002800, vec![ins(4, 14, &[0, 15000800, 5, 0, 0, 0]), ins(4, 0, &[2, 15000800, 1, 0]), ins(2003, 12, &[15000800, 16]), ins(2003, 69, &[0, 15000800, 1]), ins(2003, 66, &[0, 9120, 1])]),
            ev(15002810, vec![ins(2004, 12, &[15000800, 1])]),
            ev(15002811, vec![ins(4, 14, &[1, 15000800, 5, 1, 0, 0]), ins(2002, 11, &[15000010, 0]), ins(2004, 12, &[15000800, 0])]),
        ];
        let p = boss_plan(&malenia, 15000800).expect("a plan");
        assert_eq!((p.shape, p.block.clone(), p.cutscenes.clone(), p.final_entity, p.defeat_event, p.story_critical), (PlanShape::SameCharacter, vec![15002811], vec![15000010], 15000800, 15002800, false));
        assert_eq!(p.defeat_flags, vec![15000800, 9120]);
        // Godfrey, m11_05_00_00: 11052811 waits HP(11050801) <= 1 -> cutscene 11050020, enables 11050800
        let godfrey = vec![
            ev(11052800, vec![ins(4, 14, &[0, 11050800, 5, 0, 0, 0]), ins(2003, 12, &[11050800, 15]), ins(2003, 69, &[0, 11050800, 1])]),
            ev(11052811, vec![ins(4, 14, &[1, 11050801, 5, 1, 0, 0]), ins(2002, 11, &[11050020, 0]), ins(2004, 5, &[11050800, 1]), ins(2004, 5, &[11050801, 0])]),
        ];
        let p = boss_plan(&godfrey, 11050801).expect("a plan");
        assert_eq!((p.shape, p.block.clone(), p.final_entity, p.story_critical), (PlanShape::NextCharacter, vec![11052811], 11050800, false));
        // Radagon, m19_00_00_00: his death -> flag 19002802 -> 19002812 (cutscene, enables 19000800); the defeat plays the ending
        let radagon = vec![
            ev(19002800, vec![ins(4, 14, &[0, 19000800, 5, 0, 0, 0]), ins(2003, 12, &[19000800, 27]), ins(2002, 11, &[19000050, 0])]),
            ev(19002811, vec![ins(4, 0, &[1, 19000810, 1, 0]), ins(2003, 66, &[0, 19002802, 1])]),
            ev(19002812, vec![ins(3, 0, &[257, 19002802]), ins(2002, 11, &[19000000, 0]), ins(2004, 5, &[19000800, 1])]),
        ];
        let p = boss_plan(&radagon, 19000810).expect("a plan");
        assert!(p.story_critical, "into the ending: left to the game ({})", p.why);
        assert_eq!(p.final_entity, 19000800);
        // a catacomb boss's second music at 60 %: no phase
        let music = vec![
            ev(30002800, vec![ins(4, 14, &[0, 30000800, 5, 0, 0, 0]), ins(2003, 12, &[30000800, 18])]),
            ev(30002811, vec![ins(4, 2, &[1, 30000800, 5, 0x3f19999a, 0, 0x3f800000]), ins(2003, 66, &[0, 30002802, 1])]),
        ];
        assert!(boss_plan(&music, 30000800).is_none(), "a flag nothing changes on is no phase change");
        // a plain boss: no phase
        let plain = vec![ev(12052800, vec![ins(4, 14, &[0, 12050800, 5, 0, 0, 0]), ins(2003, 12, &[12050800, 16])])];
        assert!(boss_plan(&plain, 12050800).is_none());
    }

    /// (2026-10-24) EVERY MAP of the install (the scratch extracts): every boss with a phase change or a defeat cutscene, as
    /// the whole kill would take it (printed as TABLE lines; nothing asserted but that each plan is consistent)
    #[test]
    fn emevd_every_map_table() {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let Some(dir) = [base.join("../../build/sora_tmp/party/ZETTA/emevd/event"), base.join("../../../../../../../build/sora_tmp/party/ZETTA/emevd/event")].into_iter().find(|d| d.exists()) else {
            println!("TABLE: no extracts here: skipped");
            return;
        };
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "emevd")).collect();
        files.sort();
        for f in files {
            let map = f.file_stem().unwrap().to_string_lossy().to_string();
            let Ok(b) = std::fs::read(&f) else { continue };
            let Ok(ev) = parse(&b) else {
                println!("TABLE\t{map}\tPARSE FAILED");
                continue;
            };
            let inst = instances(&ev);
            // the candidates: every entity an HP / death condition names
            let mut cands: Vec<u32> = inst.iter().flat_map(|e| e.ins.iter().filter(|i| i.is(4, 14) || i.is(4, 2) || i.is(4, 0)).map(|i| i.word(1))).filter(|x| *x > 0 && *x != 10000).map(|x| x as u32).collect();
            cands.sort();
            cands.dedup();
            let mut finals = Vec::new();
            for c in cands {
                if let Some(pl) = boss_plan(&inst, c) {
                    assert!(!pl.block.is_empty() && pl.final_entity != 0);
                    finals.push(pl.final_entity);
                    println!("TABLE\t{map}\t{} -> {} (kills {:?})\t{:?}\tblock {:?}\tcutscenes {:?}\tdefeat {} flags {:?}\t{}\t{}", pl.entity, pl.final_entity, pl.finals, pl.shape, pl.block, pl.cutscenes, pl.defeat_event, pl.defeat_flags, if pl.story_critical { "STORY-CRITICAL" } else { "WHOLE KILL" }, pl.why);
                }
            }
            // one-phase bosses whose defeat plays a cutscene
            for d in inst.iter() {
                let Some(h) = d.ins.iter().find(|i| i.is(2003, 12)) else { continue };
                let cuts: Vec<i32> = d.ins.iter().filter(|i| i.bank == 2002).map(|i| i.word(0)).collect();
                if !cuts.is_empty() && !finals.contains(&(h.word(0) as u32)) {
                    println!("TABLE\t{map}\t{}\tone phase\t-\t-\tdefeat {} plays {:?}\tDEFEAT CUTSCENE (the silent-boss skip)\t", h.word(0), d.id, cuts);
                }
            }
        }
    }

    /// the install's own events, where the scratch extracts are (build/sora_tmp/party/ZETTA/emevd/event): what the whole
    /// kill would block and trigger for each two-phase boss (printed; asserted where the decode above was read by hand)
    #[test]
    fn emevd_the_install_plans() {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = [base.join("../../build/sora_tmp/party/ZETTA/emevd/event"), base.join("../../../../../../../build/sora_tmp/party/ZETTA/emevd/event")].into_iter().find(|d| d.exists()).unwrap_or_default();
        let cases = [("m16_00_00_00", 16000801u32, "Rykard (the serpent)"), ("m20_01_00_00", 20010801, "Promised Consort Radahn (first form)"), ("m15_00_00_00", 15000800u32, "Malenia"), ("m11_05_00_00", 11050801, "Godfrey (first form)"), ("m19_00_00_00", 19000810, "Radagon"), ("m14_00_00_00", 14000801, "Rennala (first form)"), ("m60_52_38_00", 1052380800, "Radahn"), ("m12_05_00_00", 12050800, "Mohg")];
        let mut any = false;
        for (map, entity, who) in cases {
            let Ok(b) = std::fs::read(dir.join(format!("{map}.emevd"))) else { continue };
            any = true;
            let inst = instances(&parse(&b).unwrap());
            let plan = boss_plan(&inst, entity);
            println!("PLAN {who} ({map}, entity {entity}): {plan:?}");
            match map {
                "m15_00_00_00" => assert!(plan.as_ref().is_some_and(|p| p.shape == PlanShape::SameCharacter && p.block == vec![15002811] && !p.story_critical)),
                "m11_05_00_00" => assert!(plan.as_ref().is_some_and(|p| p.shape == PlanShape::NextCharacter && p.final_entity == 11050800 && p.block == vec![11052811])),
                "m19_00_00_00" => assert!(plan.as_ref().is_some_and(|p| p.story_critical)),
                "m12_05_00_00" => assert!(plan.is_none(), "Mohg: one phase in the events"),
                "m16_00_00_00" => assert!(plan.as_ref().is_some_and(|p| p.final_entity == 16000800), "Rykard: the serpent's HP ratio 0.01 enables Rykard"),
                "m20_01_00_00" => assert!(plan.as_ref().is_some_and(|p| p.final_entity == 20010800), "the consort's HP ratio 0.65 enables his second form"),
                _ => {}
            }
        }
        if !any {
            println!("PLAN: no extracts here: skipped");
        }
    }
}
