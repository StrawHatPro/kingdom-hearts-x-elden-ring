//! (IMPL_ER48) ELDEN RING'S STATUS AILMENTS ON THE HIDDEN BODY, AND WHAT CLEARS THEM (the rules: `status_rules.rs`).
//!
//! HOW THE AILMENTS REACH SORA (read from the code, not seen in a log yet): the body keeps Elden Ring's meters and
//! effects. Whatever HP the game takes off the body (an ailment's damage over time, a hemorrhage or frostbite burst)
//! reaches Sora through `damage_in` (sora.rs) as a share of the Tarnished's own maximum. This file tells `damage_in`
//! which losses are STATUS TICKS (no hit reaction) and clears the ailments on request.
//!
//! THE CLEARING (`cure.er_status`, default on): the body is given the special effects of Elden Ring's own cure items,
//! the Boluses (their `EquipParamGoods.refId_default`, read from the params when first needed: the goods rows 900 - 999
//! whose name has "Bolus"; without names the rows 900, 910 ... 960), through the game's own "apply special effect to a
//! character" function (the SDK's `ChrIns::apply_speffect`, RVA 0x3e8dc0 in 2.7.1.0, found by pattern and the RVA
//! checked here). Then every meter whose rest value has been learnt (status_rules `GaugeRest`) is written back to it
//! (`cure.er_gauges`, default on). The log's `cure:` lines show what was on him before and 0.5 s after.
//! NOT TESTED IN THE GAME.

use std::sync::{Mutex, OnceLock};

use eldenring::cs::{EquipParamGoods, PlayerIns, SoloParamRepository, SpEffectParam};
use fromsoftware_shared::FromStatic;
use kh_core::host::{Event, EventKind};
use kh_core::status::{ItemKind, Status};

use crate::control::Control;
use crate::erlog;
use crate::havok::{locate, Located};
use crate::lootmap::Cat;
use crate::status_rules::{self as rules, Dot, GaugeRest, Loss, AILMENTS};

/// `ChrIns::ApplySpEffect(chr, id, dont_sync)` (the SDK's `chr_ins_apply_speffect`).
pub(crate) const APPLY_FN: Located = Located {
    name: "ChrIns apply special effect",
    pattern: "48 8b c4 48 89 58 08 48 89 70 10 57 48 81 ec c0 00 00 00 0f 28 05 ? ? ? ? 48 8b f1 0f 28 0d ? ? ? ? 48 8d 48 88 0f 29 40 a8 41 0f b6 d8",
    capture: 0,
    rva_2_7_1_0: 0x003e_8dc0, rva_2_7_2_0: 0x003e_8dc0 /* (PORT) */,
};
type ApplyFn = unsafe extern "C" fn(*mut u8, i32, bool) -> u64;

/// The Boluses' goods ids when the names cannot be read (900 - 940 seen with their names in the user's log; 950, 960
/// public data, A GUESS: a row that does not exist is skipped).
const BOLUS_IDS: [u32; 7] = [900, 910, 920, 930, 940, 950, 960];

/// One cure item of the game's: goods id, name, its special effect.
#[derive(Clone, Debug)]
struct CureItem {
    goods: u32,
    name: String,
    sp_effect: i32,
}

#[derive(Default)]
struct State {
    pending: Vec<String>,
    cures: Option<Vec<CureItem>>,
    rest: GaugeRest,
    timers: [f32; 7],
    gauges: [u32; 7],
    gauge_max: [u32; 7],
    seen_data: bool,
    active: Option<usize>,
    dots: Vec<Dot>,
    dots_told: Vec<i32>,
    verify: Option<(f32, String)>,
    items_seen: Option<[u32; 12]>,
    clearing: Option<Vec<(ItemKind, String)>>,
    pack: Option<std::path::PathBuf>,
    at_save_point: bool,
    save_point_wait: f32,
    ticks: u32,
    built_told: [bool; 7],
    /// The ailment timer has been seen at 0 (a timer never seen at rest is not trusted as "running")
    timer_rested: [bool; 7],
    /// (IMPL_ER50) damage effects that came onto the body in the last 0.3 s (seconds since), the burst ailment whose
    /// meter just emptied or timer just started (seconds since), each meter's build-up last frame, procs counted
    fresh: Vec<(Dot, f32)>,
    burst: Option<(usize, f32)>,
    prev_built: [f32; 7],
    procs: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static APPLY: OnceLock<Option<usize>> = OnceLock::new();

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(State::default))
}

fn apply_fn() -> Option<usize> {
    *APPLY.get_or_init(|| {
        let found = locate(&APPLY_FN);
        if found.is_some() {
            erlog!("cure: the game's \"apply special effect\" function located by pattern (PORT: {})", crate::gamever::current_name());
        }
        found
    })
}

/// Where the pack is (sora.rs, when Sora starts): the KH3 item texts are read from it.
pub fn set_pack(dir: &std::path::Path) {
    with(|s| s.pack = Some(dir.to_path_buf()));
}

/// A request to clear every Elden Ring ailment on the body (carried out in the next `frame`).
pub fn request(reason: &str) {
    with(|s| {
        if !s.pending.iter().any(|r| r == reason) {
            s.pending.push(reason.to_string());
        }
    });
}

/// (savepoint.rs) Whether he stands in a KH3 save point's recovery area that may heal him now. Entering it clears the
/// ailments; while he stays, an ailment that strikes or builds is cleared again (at most once a second).
pub fn save_point(in_recover: bool, dt: f32) {
    let ask = with(|s| {
        let entered = in_recover && !s.at_save_point;
        s.at_save_point = in_recover;
        s.save_point_wait = (s.save_point_wait - dt).max(0.0);
        let again = in_recover && s.save_point_wait <= 0.0 && something_on(s);
        if entered || again {
            s.save_point_wait = 1.0;
        }
        entered || again
    });
    if ask {
        request("a save point heals him");
    }
}

/// (IMPL_ER60) He stands in the recovery area of a save point that may heal him (as of the latest `save_point` call):
/// blood.rs washes his blood off there.
pub fn at_save_point() -> bool {
    with(|s| s.at_save_point)
}

fn something_on(s: &State) -> bool {
    s.timers.iter().any(|t| *t > 0.0) || (0..7).any(|i| s.rest.built(i, s.gauges[i], s.gauge_max[i]).is_some_and(|b| b > 0.05))
}

/// The core's events of a step and his status after it: Cure landing on him, and the effects of the KH3 items whose
/// own text says they remove status effects, ask for the clearing.
#[cfg_attr(feature = "party", allow(dead_code))]
pub fn on_events(events: &[Event], status: &Status) {
    on_events_of(events, status, kh_host::party::PartyChara::Sora, kh_host::party::PartyChara::Sora);
}

/// (ER party S2) `on_events` for Sora as a member: his Cure / items clear the body of the member they are his (the player
/// body while he leads; AI Sora's ally body while a friend leads: S3).
pub fn on_events_of(events: &[Event], status: &Status, member: kh_host::party::PartyChara, leader: kh_host::party::PartyChara) {
    let mut asks: Vec<String> = Vec::new();
    for event in events {
        if let EventKind::EffectSpawn { spawn, .. } = event.kind {
            if let Some(tier) = rules::cure_tier(spawn.name) {
                asks.push(format!("{} landed on Sora", ["Cure", "Cura", "Curaga"][tier]));
            }
        }
    }
    with(|s| {
        let now = status.item_effects;
        let before = s.items_seen.replace(now).unwrap_or(now);
        let landed = rules::items_landed(&before, &now);
        if landed.is_empty() {
            return;
        }
        let clearing = s.clearing.get_or_insert_with(|| load_clearing(s.pack.as_deref()));
        for item in landed {
            match clearing.iter().find(|(i, _)| *i == item) {
                Some((_, text)) => asks.push(format!("the item {} was used (its KH3 text: \"{text}\")", item.name())),
                None => erlog!("cure: the item {} was used: its KH3 text does not say it removes status effects: Elden Ring's ailments stay", item.name()),
            }
        }
    });
    for reason in asks {
        request_member(member, leader, &reason); // (ER party S2) the player body when he leads (as before)
    }
}

/// The KH3 items whose help text says they remove status effects (from the pack), said once in the log.
fn load_clearing(pack: Option<&std::path::Path>) -> Vec<(ItemKind, String)> {
    let Some(pack) = pack else {
        erlog!("cure: the pack's place is not known: no KH3 item clears Elden Ring's ailments");
        return Vec::new();
    };
    let read = |rel: &str| std::fs::read_to_string(pack.join(rel)).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    let (Some(icons), Some(strings)) = (read("ui/data/item_icons.json"), read("ui/strings/en/kh3_item.json")) else {
        erlog!("cure: the pack's item texts (ui/data/item_icons.json, ui/strings/en/kh3_item.json) are not readable: no KH3 item clears Elden Ring's ailments");
        return Vec::new();
    };
    let items = rules::clearing_items(&icons, &strings);
    for (item, text) in &items {
        erlog!("cure: the KH3 item {} clears Elden Ring's ailments: its text says \"{text}\"", item.name());
    }
    if items.is_empty() {
        erlog!("cure: no KH3 item's text says it removes status effects");
    }
    items
}

/// The game's cure items (once): the Boluses and their special effects.
fn cure_items() -> Vec<CureItem> {
    // SAFETY: the singleton lives as long as the game; the SDK checks its holder table.
    let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return Vec::new() };
    let mut named: Vec<CureItem> = Vec::new();
    for goods in 900..1000u32 {
        let Some(row) = repo.get::<EquipParamGoods>(goods) else { continue };
        let Some(name) = crate::loot::item_name(Cat::Goods, goods) else { continue };
        if name.contains("Bolus") && row.ref_id_default() > 0 {
            named.push(CureItem { goods, name, sp_effect: row.ref_id_default() });
        }
    }
    let items = if named.is_empty() {
        erlog!("cure: no goods row 900 - 999 is named \"...Bolus...\": the rows {BOLUS_IDS:?} are taken (A GUESS for 950, 960)");
        BOLUS_IDS
            .iter()
            .filter_map(|g| repo.get::<EquipParamGoods>(*g).filter(|r| r.ref_id_default() > 0).map(|r| CureItem { goods: *g, name: format!("goods {g}"), sp_effect: r.ref_id_default() }))
            .collect()
    } else {
        named
    };
    for item in &items {
        let effect = repo.get::<SpEffectParam>(item.sp_effect as u32).map_or("ITS ROW IS MISSING".to_string(), |sp| {
            format!(
                "state info {}, resist points poison {} rot {} bleed {} death {} frost {} sleep {} madness {}, duration {} s",
                sp.state_info(),
                sp.change_poison_resist_point(),
                sp.change_disease_resist_point(),
                sp.change_blood_resist_point(),
                sp.change_curse_resist_point(),
                sp.change_freeze_resist_point(),
                sp.change_sleep_resist_point(),
                sp.change_madness_resist_point(),
                sp.effect_endurance()
            )
        });
        let text = crate::loot::item_text(Cat::Goods, item.goods).map_or(String::new(), |(t, _)| format!(" | the game's text: \"{}\"", t.chars().take(200).collect::<String>()));
        erlog!("cure: Elden Ring's cure item {} ({}) applies special effect {}: {effect}{text}", item.goods, item.name, item.sp_effect);
    }
    items
}

fn gauges_text(s: &State) -> String {
    (0..7)
        .filter(|i| s.gauge_max[*i] > 0 || s.timers[*i] > 0.0)
        .map(|i| {
            let built = s.rest.built(i, s.gauges[i], s.gauge_max[i]).map_or("direction not learnt".to_string(), |b| format!("{:.0} % built", b * 100.0));
            let timer = if s.timers[i] > 0.0 { format!(", STRUCK {:.1} s left", s.timers[i]) } else { String::new() };
            format!("{} {}/{} ({built}{timer})", AILMENTS[i], s.gauges[i], s.gauge_max[i])
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn dots_text(dots: &[Dot]) -> String {
    if dots.is_empty() {
        return "none".to_string();
    }
    dots.iter().map(|d| format!("{} ({} % + {} per tick)", d.id, d.rate, d.point)).collect::<Vec<_>>().join(", ")
}

/// Once a frame while Sora runs (sora.rs, before `damage_in`): reads the body's meters, ailment timers and
/// damage-over-time effects; carries out the clearings asked for.
pub fn frame(player: &mut PlayerIns, control: &Control, dt: f32) {
    // ---- what is on the body
    let data_ok = crate::game::plausible(crate::word!(player.player_game_data));
    let mut dots = Vec::new();
    // SAFETY: the singleton lives as long as the game; the SDK checks its holder table.
    if let Ok(repo) = unsafe { SoloParamRepository::instance() } {
        for entry in player.chr_ins.special_effect.entries().take(128) {
            if let Some(row) = repo.get::<SpEffectParam>(entry.param_id as u32) {
                if row.change_hp_rate() > 0.0 || row.change_hp_point() > 0 {
                    dots.push(Dot { id: entry.param_id, rate: row.change_hp_rate(), point: row.change_hp_point() });
                }
            }
        }
    }
    let pending = with(|s| {
        if data_ok {
            // SAFETY: the player's game data, alive with the player (the pointer was checked); game thread.
            let pgd = unsafe { player.player_game_data.as_ref() };
            let (gauges, max, timers) = (pgd.resistance_gauges, pgd.resistance_gauge_max, pgd.proc_status_timers);
            for i in 0..7 {
                // (IMPL_ER50) a burst ailment's proc: its timer starts, or its meter falls from half built or more to empty
                let built_now = s.rest.built(i, gauges[i], max[i]);
                if rules::BURSTS.contains(&i) && s.seen_data && ((timers[i] > 0.0 && s.timers[i] <= 0.0) || (s.prev_built[i] >= 0.5 && built_now.is_some_and(|b| b <= 0.1))) {
                    s.burst = Some((i, 0.0));
                }
                s.prev_built[i] = built_now.unwrap_or(0.0);
                if timers[i] > 0.0 && s.timers[i] <= 0.0 && s.seen_data {
                    erlog!("status: {} STRUCK the body ({:.1} s): {}", AILMENTS[i], timers[i], gauges_text(&State { timers, gauges, gauge_max: max, rest: s.rest, ..State::default() }));
                } else if timers[i] <= 0.0 && s.timers[i] > 0.0 {
                    erlog!("status: {} on the body ENDED", AILMENTS[i]);
                }
                let built = s.rest.built(i, gauges[i], max[i]).unwrap_or(0.0);
                if built >= 0.25 && !s.built_told[i] {
                    s.built_told[i] = true;
                    erlog!("status: {} builds up on the body: {}/{} ({:.0} %)", AILMENTS[i], gauges[i], max[i], built * 100.0);
                } else if built <= 0.0 {
                    s.built_told[i] = false;
                }
            }
            s.rest.observe(&gauges, &max);
            (s.gauges, s.gauge_max, s.timers, s.seen_data) = (gauges, max, timers, true);
        }
        let mut trusted = s.timers;
        for i in 0..7 {
            s.timer_rested[i] |= s.seen_data && s.timers[i] <= 0.0;
            if !s.timer_rested[i] {
                trusted[i] = 0.0;
            }
        }
        s.active = rules::over_time_active(&trusted);
        for d in &dots {
            if !s.dots_told.contains(&d.id) {
                s.dots_told.push(d.id);
                erlog!("status: damage-over-time effect {} on the body: {} % of the maximum + {} HP per tick (over-time ailment running: {})", d.id, d.rate, d.point, s.active.map_or("none", |i| AILMENTS[i]));
            }
        }
        s.dots_told.retain(|id| dots.iter().any(|d| d.id == *id));
        // (IMPL_ER50) the effects new this frame, kept 0.3 s as signs of a proc
        for (_, age) in s.fresh.iter_mut() {
            *age += dt;
        }
        let new: Vec<Dot> = dots.iter().filter(|d| !s.dots.iter().any(|o| o.id == d.id)).copied().collect();
        s.fresh.extend(new.into_iter().map(|d| (d, 0.0)));
        s.fresh.retain(|(_, age)| *age <= 0.3);
        s.burst = s.burst.map(|(i, age)| (i, age + dt)).filter(|(_, age)| *age <= 0.3);
        s.dots = dots;
        if let Some((left, before)) = s.verify.take() {
            let left = left - dt;
            if left <= 0.0 {
                erlog!("cure: 0.5 s later: meters {} | damage-over-time effects {} (before: {before})", gauges_text(s), dots_text(&s.dots));
            } else {
                s.verify = Some((left, before));
            }
        }
        std::mem::take(&mut s.pending)
    });
    if pending.is_empty() {
        return;
    }
    if control.flag("cure.er_status") == Some(false) {
        erlog!("cure: {}: cure.er_status = 0: Elden Ring's ailments are left on the body", pending.join("; "));
        return;
    }
    clear(player, control, data_ok, &pending.join("; "));
}

/// The clearing itself.
fn clear(player: &mut PlayerIns, control: &Control, data_ok: bool, reason: &str) {
    let cures = with(|s| s.cures.get_or_insert_with(cure_items).clone());
    let before = with(|s| format!("meters {} | damage-over-time effects {}", gauges_text(s), dots_text(&s.dots)));
    let mut applied = Vec::new();
    match apply_fn() {
        Some(f) => {
            // SAFETY: the game's own function at its checked address; the player object is complete (game.rs); game
            // thread (a frame task of the game's).
            let call: ApplyFn = unsafe { std::mem::transmute::<usize, ApplyFn>(f) };
            for c in &cures {
                unsafe { call(player as *mut PlayerIns as *mut u8, c.sp_effect, false) };
                applied.push(format!("{} ({})", c.sp_effect, c.name));
            }
        }
        None => erlog!("cure: the game's apply function was not found: no cure effect given"),
    }
    // ---- the meters back to their learnt rest value
    let mut reset = Vec::new();
    if data_ok && control.flag("cure.er_gauges") != Some(false) {
        // SAFETY: as in `frame`; plain u32 fields of the player's game data.
        let pgd = unsafe { player.player_game_data.as_mut() };
        let rest = with(|s| s.rest);
        for i in 0..7 {
            if let Some(v) = rest.reset_value(i, pgd.resistance_gauges[i], pgd.resistance_gauge_max[i]) {
                reset.push(format!("{} {} -> {v}", AILMENTS[i], pgd.resistance_gauges[i]));
                pgd.resistance_gauges[i] = v;
            }
        }
    }
    erlog!(
        "cure: {reason}: BEFORE {before} | applied the game's cure effects [{}] | meters written back to their rest value: [{}]",
        applied.join(", "),
        if reset.is_empty() { "none (none built up, or their direction is not learnt yet)".to_string() } else { reset.join(", ") }
    );
    with(|s| s.verify = Some((0.5, before)));
}

// ---------------------------------------------------------------------------------------------- (ER party S2) the party

/// (ER party S2) Cures asked for the AI allies' bodies (S3's pool takes them: `take_ally_cures`, then `cure_chr` on each
/// body): Donald's Cure on Goofy clears Goofy's body; the save point clears every member's (ER_PHASE_NOTES).
#[allow(dead_code)] // (ER party S2) called by S3 / S5 / S6
static ALLY_CURES: Mutex<Vec<(kh_host::party::PartyChara, String)>> = Mutex::new(Vec::new());

/// (ER party S2) A CURE FOR ONE MEMBER: the leader's is the player body's (`request`), an AI ally's goes to his body
/// (S3). `leader`: who leads now (Sora when the party is off).
#[allow(dead_code)] // (ER party S2) called by S3 / S5 / S6
pub fn request_member(member: kh_host::party::PartyChara, leader: kh_host::party::PartyChara, reason: &str) {
    if member == leader {
        request(reason);
        return;
    }
    let mut cures = ALLY_CURES.lock().unwrap_or_else(|e| e.into_inner());
    if !cures.iter().any(|(m, r)| *m == member && r == reason) {
        cures.push((member, reason.to_string()));
    }
}

/// (ER party S2) EVERY ACTIVE MEMBER'S AILMENTS (the save point's rule in ER: the user's "save points should also cure all
/// status effects", for the whole party): the player body's and each ally body's.
#[allow(dead_code)] // (ER party S2) called by S3 / S5 / S6
pub fn request_all(members: &[kh_host::party::PartyChara], leader: kh_host::party::PartyChara, reason: &str) {
    request(reason);
    for m in members.iter().filter(|m| **m != leader) {
        request_member(*m, leader, reason);
    }
}

/// (ER party S2) A party effect spawned on a member (`Attach::Party`; S5 / S1 call it with the effect's name): a Cure
/// landing on him clears his body's ailments (the leader's: the player body).
#[allow(dead_code)] // (ER party S2) called by S3 / S5 / S6
pub fn on_party_effect(name: &str, member: kh_host::party::PartyChara, leader: kh_host::party::PartyChara) {
    if let Some(tier) = rules::cure_tier(name) {
        request_member(member, leader, &format!("{} landed on {}", ["Cure", "Cura", "Curaga"][tier], member.name()));
    }
}

/// (ER party S2) A KH3 item a member used on a member (kh-party `PartyEventKind::ItemUsed`): its own text says whether it
/// clears status effects (as Sora's items: `on_events`).
#[allow(dead_code)] // (ER party S2) called by S3 / S5 / S6
pub fn on_party_item(item: ItemKind, member: kh_host::party::PartyChara, leader: kh_host::party::PartyChara) {
    let clears = with(|s| {
        let clearing = s.clearing.get_or_insert_with(|| load_clearing(s.pack.as_deref()));
        clearing.iter().find(|(i, _)| *i == item).map(|(_, text)| text.clone())
    });
    if let Some(text) = clears {
        request_member(member, leader, &format!("the item {} was used on {} (its KH3 text: \"{text}\")", item.name(), member.name()));
    }
}

/// (ER party S2, for S3) The ally cures asked since the last call.
#[allow(dead_code)] // (ER party S2) called by S3 / S5 / S6
pub fn take_ally_cures() -> Vec<(kh_host::party::PartyChara, String)> {
    std::mem::take(&mut *ALLY_CURES.lock().unwrap_or_else(|e| e.into_inner()))
}

/// (ER party S2, for S3) THE GAME'S CURE ON ANY CHARACTER (an AI ally's body): the Boluses' special effects through the
/// game's own "apply special effect" (as `clear` does for the player). An NPC body keeps its build-up meters elsewhere
/// (not in player game data): only the effects are given. Returns how many were applied. NOT TESTED IN THE GAME.
#[allow(dead_code)] // (ER party S2) called by S3 / S5 / S6
pub fn cure_chr(chr: &mut eldenring::cs::ChrIns, reason: &str) -> usize {
    let cures = with(|s| s.cures.get_or_insert_with(cure_items).clone());
    let Some(f) = apply_fn() else {
        erlog!("cure: {reason}: the game's apply function was not found: no cure effect given to the body");
        return 0;
    };
    // SAFETY: the game's own function at its checked address; `chr` is a live character of the game's (the caller holds it
    // by its handle this frame); game thread.
    let call: ApplyFn = unsafe { std::mem::transmute::<usize, ApplyFn>(f) };
    for c in &cures {
        unsafe { call(chr as *mut eldenring::cs::ChrIns as *mut u8, c.sp_effect, false) };
    }
    erlog!("cure: {reason}: the game's cure effects [{}] given to an ally's body", cures.iter().map(|c| format!("{} ({})", c.sp_effect, c.name)).collect::<Vec<_>>().join(", "));
    cures.len()
}

/// (sora.rs `damage_in`) What a loss of `lost` HP of the body is (see `status_rules::classify`); logs status ticks.
pub fn loss(lost: i32, own_max: i32, body_max: i32, control: &Control) -> Loss {
    if control.flag("status.ticks") == Some(false) {
        return Loss::Hit;
    }
    let cap = control.f32("status.tick_cap").unwrap_or(0.05);
    with(|s| {
        // (IMPL_ER50) the signs of a proc; the fallback percent per burst ailment (A GUESS from public data: hemorrhage
        // 15 %, frostbite 10 % of the maximum; madness none) only when no effect of its own is seen
        let fresh: Vec<Dot> = if control.flag("status.procs") == Some(false) { Vec::new() } else { s.fresh.iter().map(|(d, _)| *d).collect() };
        let ailment = s.burst.map(|(i, _)| i).filter(|_| control.flag("status.procs") != Some(false));
        let guess_pct = ailment.and_then(|i| control.f32(&format!("status.proc_pct.{}", ["", "", "hemorrhage", "", "frostbite", "", "madness"][i])).or(match i { 2 => Some(15.0), 4 => Some(10.0), _ => None }));
        let class = rules::classify_with(lost, own_max, body_max, s.active, &s.dots, cap, &rules::ProcSign { fresh: &fresh, ailment, guess_pct });
        if let Loss::Proc { ailment, pct, flat, dot, guessed } = class {
            s.procs += 1;
            s.burst = None;
            if let Some(d) = dot {
                s.fresh.retain(|(f, _)| f.id != d.id);
            }
            erlog!(
                "status: {} PROC #{}: the body lost {lost} of {body_max} HP ({}): {pct:.2} % of the maximum is taken of SORA'S maximum, the other {flat} HP (its flat part and the attack with it) as a share of his own {own_max} (damage.in); a hit, he reacts",
                ailment.map_or("a", |i| AILMENTS[i]),
                s.procs,
                match dot { Some(d) => format!("its effect {}: {} % + {} HP", d.id, d.rate, d.point), None if guessed => "NO EFFECT OF ITS OWN SEEN: the percent is the fallback GUESS status.proc_pct".to_string(), None => String::new() }
            );
        }
        if let Loss::Tick { ailment, own_loss, dot } = class {
            s.ticks += 1;
            if s.ticks <= 20 || s.ticks % 50 == 0 {
                erlog!(
                    "status: {} tick #{}: the body lost {lost} of {body_max} HP -> taken as {own_loss} of his own {own_max}{}: no hit reaction",
                    AILMENTS[ailment],
                    s.ticks,
                    match dot { Some(d) => format!(" (effect {}: {} % + {} per tick)", d.id, d.rate, d.point), None => " (no damage-over-time effect matched: the loss as it is)".to_string() }
                );
            }
        }
        class
    })
}
