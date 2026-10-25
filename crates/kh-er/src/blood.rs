//! (IMPL_ER60) BLOOD ON SORA AND THE KEYBLADE, by Elden Ring's logic (the user, 2026-10-14: "Allow Sora and his weapon to
//! get covered in blood following Elden Ring's logic, removes blood when going in save point").
//!
//! WHAT ELDEN RING DOES (as played; the game's own per-character blood value was NOT found: the SDK names no such field,
//! see IMPL_ER60_STATE.md): the player's armour and weapon take blood from hits on enemies that bleed and from the hits
//! he takes; it stays, darkens as it dries, is washed off by water and rain, and is gone after resting at a grace.
//!
//! THE MODEL HERE (every number a row `blood.*` of sheets/port/sora-er-host.json, host choices; the control key of the
//! same name overrides each while the game runs):
//! - each of Sora's hits that LANDS (not guarded) on a body that bleeds adds `blood.hit_blade` to the Keyblade and
//!   `blood.hit_body` to him. A body bleeds unless its NpcParam `resistBlood` is at least `blood.immune_resist` (Elden
//!   Ring's "immune" value: skeletons, golems, spirits and the like have it);
//! - HP he loses in one frame of at least `blood.hurt_min` of his maximum adds that share x `blood.hurt_body` to him;
//!   (FINAL4) only a LOSS OF HIS SHARE of HP counts: a maximum that changes with his share kept (a friend made the
//!   leader or AI again, BAL's leader maximum, a level or boost) adds none, and a frame whose hit his guard took
//!   (guarded, absorbed by Goofy's GUARD, a Magic Block, a dodge: `note_guarded`) adds none, for every character;
//! - new blood is fresh (wet red); it dries over `blood.dry_s` (dark brown, dull) and fades by itself over `blood.decay_s`;
//! - WASHING, the game's own wetness: while the hidden Elden Ring body carries one of the game's WET effects (the
//!   SpEffect ids of HitMtrlParam `spEffectIdForWet00..04`, read from the game's params: the effects Elden Ring puts on a
//!   character that is wet from rain or water, by level 0..4), the blood washes off, a full coat in `blood.wash_s` at
//!   level 4 (level k: (k + 1) / 5 of that speed);
//! - THE SAVE POINT: when he enters a save point's heal area (the area that heals him and cures his status: status.rs)
//!   the blood fades out over `blood.save_fade_s` (1 s); the next blood gets a new splatter pattern.
//!
//! The renderer (kh_er_render::blood) draws it with the `*_bl` pixel shaders. Keys: `blood.kh` (default 1; 0 = none at
//! all), `blood.force = BODY[,BLADE]` (testing aid: shows these amounts), `blood.probe = 0` (no discovery lines).

use std::collections::HashMap;
use std::sync::Mutex;

use eldenring::cs::{HitMtrlParam, NpcParam, PlayerIns, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use crate::control::Control;
use crate::erlog;
use crate::targets::ErTargets;

/// (FINAL4) THE CHARACTERS WHOSE HIT WAS GUARDED OR ABSORBED since their last blood step (the friend leader's
/// `hit_now`, the AI members' hits in the party glue): their next step adds no blood from HP.
static GUARDED: Mutex<Vec<kh_host::party::PartyChara>> = Mutex::new(Vec::new());

/// (FINAL4) A hit on `who` was guarded / absorbed / dodged (it cost him nothing): no blood from it.
#[allow(dead_code)] // (called by the party code only)
pub fn note_guarded(who: kh_host::party::PartyChara) {
    if let Ok(mut g) = GUARDED.lock() {
        if !g.contains(&who) {
            g.push(who);
        }
    }
}

/// (FINAL4) Whether a guarded hit on `who` was noted since the last call (the note is taken).
pub fn take_guarded(who: kh_host::party::PartyChara) -> bool {
    let Ok(mut g) = GUARDED.lock() else { return false };
    let had = g.contains(&who);
    g.retain(|c| *c != who);
    had
}

/// The sheet's rows `blood.*` (sora-er-host).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BloodRules {
    pub hit_blade: f32,
    pub hit_body: f32,
    pub hurt_body: f32,
    pub hurt_min: f32,
    pub dry_s: f32,
    pub decay_s: f32,
    pub wash_s: f32,
    pub save_fade_s: f32,
    pub immune_resist: u16,
}

impl BloodRules {
    pub const DEFAULT: BloodRules = BloodRules { hit_blade: 0.06, hit_body: 0.02, hurt_body: 0.8, hurt_min: 0.01, dry_s: 90.0, decay_s: 900.0, wash_s: 20.0, save_fade_s: 1.0, immune_resist: 999 };

    /// The rows of the sheet text (missing or malformed rows keep the defaults); the names of rows not understood.
    pub fn from_sheet(text: &str) -> (BloodRules, Vec<String>) {
        let mut r = BloodRules::DEFAULT;
        let mut problems = Vec::new();
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return (r, vec!["the sheet is not JSON".to_string()]) };
        for row in json.get("rows").and_then(|v| v.as_array()).into_iter().flatten() {
            let Some(id) = row.get(0).and_then(|v| v.as_str()).filter(|id| id.starts_with("blood.")) else { continue };
            let Some(v) = row.get(1).and_then(|v| v.as_f64()).map(|v| v as f32).filter(|v| v.is_finite() && *v >= 0.0) else {
                problems.push(id.to_string());
                continue;
            };
            if !r.set(id, v) {
                problems.push(id.to_string());
            }
        }
        (r, problems)
    }

    fn set(&mut self, id: &str, v: f32) -> bool {
        match id {
            "blood.hit_blade" => self.hit_blade = v,
            "blood.hit_body" => self.hit_body = v,
            "blood.hurt_body" => self.hurt_body = v,
            "blood.hurt_min" => self.hurt_min = v,
            "blood.dry_s" => self.dry_s = v.max(0.1),
            "blood.decay_s" => self.decay_s = v,
            "blood.wash_s" => self.wash_s = v,
            "blood.save_fade_s" => self.save_fade_s = v,
            "blood.immune_resist" => self.immune_resist = v.min(u16::MAX as f32) as u16,
            _ => return false,
        }
        true
    }

    /// The control file's keys of the same names over these.
    fn with_keys(mut self, control: &Control) -> BloodRules {
        for id in ["blood.hit_blade", "blood.hit_body", "blood.hurt_body", "blood.hurt_min", "blood.dry_s", "blood.decay_s", "blood.wash_s", "blood.save_fade_s", "blood.immune_resist"] {
            if let Some(v) = control.f32(id).filter(|v| v.is_finite() && *v >= 0.0) {
                self.set(id, v);
            }
        }
        self
    }
}

/// What happened to him this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BloodInputs {
    /// His hits that landed on bodies that bleed.
    pub bleeding_hits: u32,
    pub hp: i32,
    pub max_hp: i32,
    /// The highest of Elden Ring's wet levels (0..4) on the hidden body, `None`: dry.
    pub wet_level: Option<u8>,
    /// He stands in a save point's heal area.
    pub at_save_point: bool,
    /// (FINAL4) A hit on him this frame was GUARDED or ABSORBED (his guard / GUARD meter, a Magic Block, a dodge, his
    /// invincibility): this frame's HP change adds no blood.
    pub guarded: bool,
}

/// The blood on him (engine-free: unit-tested).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Blood {
    pub body: f32,
    pub blade: f32,
    /// 1 = fresh .. 0 = dried.
    pub fresh: f32,
    /// The splatter pattern's seed: a new one after each wash to nothing.
    pub seed: f32,
    /// The save point's fade: seconds into it and the amounts it started from.
    clearing: Option<(f32, f32, f32)>,
    /// (FINAL4) his HP and maximum of the step before
    last_hp: Option<(i32, i32)>,
    at_save_point: bool,
}

/// What a step did worth a log line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BloodEvent {
    SavePointClears,
    Clean,
}

impl Blood {
    pub fn step(&mut self, r: &BloodRules, i: &BloodInputs, dt: f32) -> Option<BloodEvent> {
        let dt = dt.clamp(0.0, 0.25);
        let had = self.body > 0.0 || self.blade > 0.0;
        let mut event = None;
        // ---- the save point: entering its heal area starts the fade (it runs to its end)
        let entered = i.at_save_point && !self.at_save_point;
        self.at_save_point = i.at_save_point;
        if entered && had && self.clearing.is_none() {
            self.clearing = Some((0.0, self.body, self.blade));
            event = Some(BloodEvent::SavePointClears);
        }
        // ---- new blood: his hits on bleeding bodies, the HP he loses
        let mut added_body = r.hit_body * i.bleeding_hits as f32;
        let added_blade = r.hit_blade * i.bleeding_hits as f32;
        if let Some((before, before_max)) = self.last_hp {
            if i.max_hp > 0 && i.hp < before && !i.guarded {
                // (FINAL4) the HP lost, but never more than the share of his bar he lost: a maximum rescaled with his
                // share kept (160 / 160 -> 138 / 138 when Goofy becomes the leader) is no hit
                let lost = (before - i.hp) as f32 / i.max_hp as f32;
                let share = if before_max > 0 { lost.min(before as f32 / before_max as f32 - i.hp as f32 / i.max_hp as f32) } else { lost };
                if share >= r.hurt_min {
                    added_body += share * r.hurt_body;
                }
            }
        }
        self.last_hp = Some((i.hp, i.max_hp));
        if self.clearing.is_none() && !i.at_save_point {
            let old = self.body + self.blade;
            self.body = (self.body + added_body).min(1.0);
            self.blade = (self.blade + added_blade).min(1.0);
            let added = self.body + self.blade - old;
            if added > 0.0 {
                // (fresh blood over dried: the share of the new)
                self.fresh = ((self.fresh * old + added) / (old + added)).clamp(0.0, 1.0);
            }
        }
        // ---- drying, fading, washing
        self.fresh = (self.fresh - dt / r.dry_s.max(0.1)).max(0.0);
        let mut loss = if r.decay_s > 0.0 { dt / r.decay_s } else { 0.0 };
        if let Some(level) = i.wet_level {
            if r.wash_s > 0.0 {
                loss += dt * (level.min(4) as f32 + 1.0) / 5.0 / r.wash_s;
            }
            // (wet blood shines again)
            self.fresh = self.fresh.max(0.6);
        }
        self.body = (self.body - loss).max(0.0);
        self.blade = (self.blade - loss).max(0.0);
        if let Some((t, body, blade)) = self.clearing {
            let t = t + dt;
            let k = if r.save_fade_s > 0.0 { (1.0 - t / r.save_fade_s).max(0.0) } else { 0.0 };
            self.body = self.body.min(body * k);
            self.blade = self.blade.min(blade * k);
            self.clearing = if k > 0.0 { Some((t, body, blade)) } else { None };
        }
        if had && self.body <= 0.0 && self.blade <= 0.0 {
            self.body = 0.0;
            self.blade = 0.0;
            self.fresh = 0.0;
            self.seed = (self.seed + 1.0) % 64.0;
            self.clearing = None;
            event = event.or(Some(BloodEvent::Clean));
        }
        event
    }
}

#[derive(Default)]
struct State {
    blood: Blood,
    rules: Option<BloodRules>,
    clock: Option<f32>,
    /// Elden Ring's wet effects: (SpEffect id, level 0..4), read once from HitMtrlParam.
    wet_ids: Option<Vec<(i32, u8)>>,
    /// NpcParam row -> it bleeds.
    bleeds: HashMap<i32, bool>,
    told_on: Option<bool>,
    wet_told: Option<Option<u8>>,
    /// This second: hits on bleeding / non-bleeding bodies.
    hits: (u32, u32),
    probe_told: Option<String>,
    /// The latest of Sora's steps whose hits were counted (the event list stays while no step runs: a paused frame).
    last_step: Option<u64>,
    /// (ER party S5) the hidden body's wet level of the latest frame (the friends wash with it), the friends' blood, the
    /// latest step whose hits the friends' blood counted, their clock
    #[cfg(feature = "party")]
    wet_level: Option<u8>,
    #[cfg(feature = "party")]
    party: party::PartyBlood,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// The rows of the host sheet (sora.rs reads it at Sora's start: `sora-er-host.json` beside the loadout).
pub fn load(sheet: &std::path::Path) {
    let rules = match std::fs::read_to_string(sheet) {
        Ok(text) => {
            let (rules, problems) = BloodRules::from_sheet(&text);
            erlog!("blood: rows of {} ({} not understood{}): {rules:?}", sheet.display(), problems.len(), if problems.is_empty() { String::new() } else { format!(": {}", problems.join(", ")) });
            rules
        }
        Err(e) => {
            erlog!("blood: {} not read ({e}): the built-in rows", sheet.display());
            BloodRules::DEFAULT
        }
    };
    if let Ok(mut guard) = STATE.lock() {
        let state = guard.get_or_insert_with(State::default);
        state.rules = Some(rules);
    }
}

/// Elden Ring's wet effects from its params: every HitMtrlParam row's `spEffectIdForWet00..04` (id, level).
fn read_wet_ids() -> Vec<(i32, u8)> {
    // SAFETY: the game's singleton; rows are read through the SDK's param tables.
    let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return Vec::new() };
    let mut out: Vec<(i32, u8)> = Vec::new();
    for (_, row) in repo.rows::<HitMtrlParam>().take(4096) {
        for (level, id) in [row.sp_effect_id_for_wet00(), row.sp_effect_id_for_wet01(), row.sp_effect_id_for_wet02(), row.sp_effect_id_for_wet03(), row.sp_effect_id_for_wet04()].into_iter().enumerate() {
            if id > 0 && !out.iter().any(|(i, l)| *i == id && *l == level as u8) {
                out.push((id, level as u8));
            }
        }
    }
    out
}

/// Whether the body of this NpcParam row bleeds (`resistBlood` under the immune value); unknown rows bleed.
fn bleeds(state: &mut State, npc_param: i32, immune: u16, npc_id: i32) -> bool {
    if let Some(b) = state.bleeds.get(&npc_param) {
        return *b;
    }
    // SAFETY: the game's singleton; one row by id.
    let row = unsafe { SoloParamRepository::instance() }.ok().and_then(|repo| repo.get::<NpcParam>(npc_param as u32).map(|r| (r.resist_blood(), r.material_sfx1())));
    let b = row.is_none_or(|(resist, _)| resist < immune);
    match row {
        Some((resist, sfx)) => erlog!("blood: c{npc_id:04} (NpcParam {npc_param}) {} (resistBlood {resist}, its hit material sfx {sfx})", if b { "BLEEDS: his hits on it bloody him" } else { "does NOT bleed (immune): no blood from it" }),
        None => erlog!("blood: c{npc_id:04} (NpcParam {npc_param}): no row read: taken as bleeding"),
    }
    state.bleeds.insert(npc_param, b);
    b
}

/// A wide string of the game, read with checked reads (at most 64 characters).
fn wide_at(addr: usize) -> Option<String> {
    let mut b = [0u8; 128];
    if !crate::probe::read(addr, &mut b) {
        return None;
    }
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
    (units.len() < 64).then(|| String::from_utf16_lossy(&units))
}

/// (discovery, `blood.probe`, default on: a line only when it changes) The hidden body's model parameter modifiers by
/// name and first values: if Elden Ring keeps the player's blood in one of them, its name shows here.
fn probe(state: &mut State, player: &PlayerIns) {
    let module = &player.chr_ins.modules.model_param_modifier;
    let list: &[eldenring::cs::CSChrModelParamModifierModuleEntry] = &module.modifiers;
    if list.len() > 64 {
        return;
    }
    let base = list.as_ptr() as usize;
    let size = std::mem::size_of::<eldenring::cs::CSChrModelParamModifierModuleEntry>();
    let mut parts = Vec::new();
    for k in 0..list.len() {
        let entry = base + k * size;
        let name = crate::probe::word(entry + 0x20).and_then(wide_at).unwrap_or_else(|| "?".to_string());
        let values = crate::probe::floats::<4>(entry + 0x2C).map_or("?".to_string(), |v| format!("{:.2}/{:.2}/{:.2}/{:.2}", v[0], v[1], v[2], v[3]));
        parts.push(format!("{name} {values}"));
    }
    let line = parts.join(", ");
    if state.probe_told.as_deref() != Some(line.as_str()) {
        erlog!("blood: (discovery) the Tarnished's model parameter modifiers: {}", if line.is_empty() { "none" } else { line.as_str() });
        state.probe_told = Some(line);
    }
}

/// ONCE A FRAME after Sora's simulation (sora.rs `post_physics`): this frame's hits and HP, the hidden body's wetness,
/// the save point; hands the look to the renderer. `clock`: Sora's clock (s; it stands still while the game is paused).
pub fn frame(control: &Control, player: &PlayerIns, targets: &ErTargets, hp: i32, max_hp: i32, clock: f32, tick: bool) {
    let Ok(mut guard) = STATE.lock() else { return };
    let state = guard.get_or_insert_with(State::default);
    let on = control.flag("blood.kh") != Some(false);
    // (ER85, the user: "Sora's face should be able to get bloody as well") `blood.face = 0`: the head rule as before (droplets only)
    kh_er_render::blood::set_face(control.flag("blood.face") != Some(false));
    if state.told_on != Some(on) {
        state.told_on = Some(on);
        erlog!("blood: {}", if on { "Sora and the Keyblade take blood by Elden Ring's rules (blood.kh = 0: none); a save point washes it off" } else { "OFF (blood.kh = 0)" });
    }
    if !on {
        state.blood = Blood::default();
        state.clock = None;
        state.last_step = None;
        kh_er_render::blood::set(kh_er_render::blood::BloodLook::default());
        return;
    }
    let rules = state.rules.unwrap_or(BloodRules::DEFAULT).with_keys(control);
    // (a new Sora starts his clock at 0: his blood starts clean)
    let dt = match state.clock {
        Some(before) if clock >= before => clock - before,
        Some(_) => {
            state.blood = Blood::default();
            state.last_step = None;
            0.0
        }
        None => 0.0,
    };
    state.clock = Some(clock);
    // ---- his hits on bodies that bleed
    let mut bleeding_hits = 0u32;
    let counted = state.last_step;
    for e in targets.events.iter().filter(|e| !e.guarded && counted.is_none_or(|s| e.step > s)) {
        // (ER party S5) a hit of Donald's or Goofy's bloodies them, not Sora (party_frame)
        #[cfg(feature = "party")]
        if party::is_party_attack(e.attack_name) {
            continue;
        }
        let Some(body) = targets.list.iter().find(|b| b.id == e.target) else { continue };
        if bleeds(state, body.npc_param, rules.immune_resist, body.npc_id) {
            bleeding_hits += 1;
            state.hits.0 += 1;
        } else {
            state.hits.1 += 1;
        }
    }
    if let Some(newest) = targets.events.iter().map(|e| e.step).max() {
        state.last_step = Some(counted.map_or(newest, |s| s.max(newest)));
    }
    // ---- Elden Ring's wetness on the hidden body
    if state.wet_ids.as_ref().is_none_or(|ids| ids.is_empty()) && (tick || state.wet_ids.is_none()) {
        let ids = read_wet_ids();
        erlog!("blood: Elden Ring's wet effects (HitMtrlParam spEffectIdForWet00..04): {} id(s){}", ids.len(), if ids.is_empty() { " (none read yet: no washing until they are)".to_string() } else { format!(": {}", ids.iter().take(24).map(|(id, l)| format!("{id} (level {l})")).collect::<Vec<_>>().join(", ")) });
        state.wet_ids = Some(ids);
    }
    let wet_level = state.wet_ids.as_ref().and_then(|ids| player.chr_ins.special_effect.entries().take(128).filter_map(|e| ids.iter().find(|(id, _)| *id == e.param_id).map(|(_, l)| *l)).max());
    if state.wet_told != Some(wet_level) {
        state.wet_told = Some(wet_level);
        erlog!("blood: the Tarnished is {}", wet_level.map_or("dry".to_string(), |l| format!("WET (Elden Ring's level {l}): the blood washes off")));
    }
    #[cfg(feature = "party")]
    {
        state.wet_level = wet_level;
    }
    let guarded = take_guarded(kh_host::party::PartyChara::Sora); // (FINAL4)
    let inputs = BloodInputs { bleeding_hits, hp, max_hp, wet_level, at_save_point: crate::status::at_save_point(), guarded };
    let before = state.blood;
    match state.blood.step(&rules, &inputs, dt) {
        Some(BloodEvent::SavePointClears) => erlog!("blood: the save point washes it off (body {:.2}, Keyblade {:.2}, over {:.1} s)", before.body, before.blade, rules.save_fade_s),
        Some(BloodEvent::Clean) => erlog!("blood: clean again"),
        None => {}
    }
    let b = state.blood;
    // (testing aid) `blood.force = BODY[,BLADE]`
    let forced: Option<(f32, f32)> = control.str("blood.force").and_then(|v| {
        let n: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        n.first().map(|body| (*body, n.get(1).copied().unwrap_or(*body)))
    });
    let (body, blade, fresh) = forced.map_or((b.body, b.blade, b.fresh), |(body, blade)| (body, blade, 1.0));
    kh_er_render::blood::set(kh_er_render::blood::BloodLook { body, blade, fresh, seed: b.seed });
    if tick {
        if control.flag("blood.probe") != Some(false) {
            probe(state, player);
        }
        if b.body > 0.0 || b.blade > 0.0 || state.hits != (0, 0) || forced.is_some() {
            erlog!("blood: body {:.2} Keyblade {:.2} fresh {:.2}{} | hits this second on bleeding bodies {}, on others {} | {}", b.body, b.blade, b.fresh, forced.map_or(String::new(), |f| format!(" (blood.force shows {:.2} / {:.2})", f.0, f.1)), state.hits.0, state.hits.1, kh_er_render::blood::status());
        }
        state.hits = (0, 0);
    }
}

/// (ER party S5) THE FRIENDS' BLOOD, once a frame after `frame` (sora.rs post_physics): every party member (Donald,
/// Goofy) takes blood by Sora's rules (U11): his own hits that land on bodies that bleed (his attack rows: kh-party's
/// tables), the HP he loses; it dries, fades, washes with the hidden body's wetness; THE SAVE POINT washes EVERY active
/// member and his weapon (the user's rule: blood comes off at save points, as Sora's; S6's request). `members`: (who,
/// his kh-party character, HP, maximum, feet). The look per member is `party_look` (S4 draws it).
#[cfg(feature = "party")]
pub fn party_frame(control: &Control, targets: &ErTargets, members: &[(kh_host::party::PartyChara, kh_party::Chara, i32, i32, glam::Vec3)], clock: f32, tick: bool) {
    let Ok(mut guard) = STATE.lock() else { return };
    let state = guard.get_or_insert_with(State::default);
    if control.flag("blood.kh") == Some(false) {
        state.party = party::PartyBlood::default();
        party::publish(&[]);
        return;
    }
    let rules = state.rules.unwrap_or(BloodRules::DEFAULT).with_keys(control);
    let dt = state.party.advance(clock); // (ERT4) a new Sora host (load, Continue, respawn) starts the friends clean too
    // their hits on bodies that bleed (each event once)
    let counted = state.party.last_step;
    let mut hits: Vec<(kh_party::Chara, u32)> = Vec::new();
    let events: Vec<(u64, TargetIdOf, &'static str, glam::Vec3)> = targets.events.iter().filter(|e| !e.guarded && counted.is_none_or(|s| e.step > s)).map(|e| (e.step, e.target, e.attack_name, e.position)).collect();
    for (_, target, name, at) in &events {
        let Some(who) = party::attacker(name, *at, &members.iter().map(|m| (m.1, m.4)).collect::<Vec<_>>()) else { continue };
        let Some(body) = targets.list.iter().find(|b| b.id == *target) else { continue };
        if bleeds(state, body.npc_param, rules.immune_resist, body.npc_id) {
            match hits.iter_mut().find(|h| h.0 == who) {
                Some(h) => h.1 += 1,
                None => hits.push((who, 1)),
            }
        }
    }
    if let Some(newest) = events.iter().map(|e| e.0).max() {
        state.party.last_step = Some(counted.map_or(newest, |s| s.max(newest)));
    }
    let at_save_point = crate::status::at_save_point();
    let inputs: Vec<(kh_host::party::PartyChara, BloodInputs)> = members.iter().map(|(c, k, hp, max, _)| (*c, BloodInputs { bleeding_hits: hits.iter().find(|h| h.0 == *k).map_or(0, |h| h.1), hp: *hp, max_hp: *max, wet_level: state.wet_level, at_save_point, guarded: take_guarded(*c) /* (FINAL4) */ })).collect();
    for (c, event) in state.party.step(&rules, &inputs, dt) {
        match event {
            BloodEvent::SavePointClears => erlog!("blood: (party) the save point washes {} and his weapon", c.name()),
            BloodEvent::Clean => erlog!("blood: (party) {} is clean again", c.name()),
        }
    }
    party::publish(&state.party.looks());
    if tick {
        for (c, b) in state.party.looks() {
            if b.body > 0.0 || b.blade > 0.0 {
                erlog!("blood: (party) {} body {:.2} weapon {:.2} fresh {:.2}", c.name(), b.body, b.blade, b.fresh);
            }
        }
    }
}

#[cfg(feature = "party")]
type TargetIdOf = kh_core::TargetId;

/// (ER party S5) The friends' blood, engine-free (unit-tested).
#[cfg(feature = "party")]
pub mod party {
    use super::{Blood, BloodEvent, BloodInputs, BloodRules};
    use kh_host::party::PartyChara;
    use kh_er_render::blood::BloodLook;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct PartyBlood {
        pub members: Vec<(PartyChara, Blood)>,
        pub clock: Option<f32>,
        pub last_step: Option<u64>,
    }

    impl PartyBlood {
        /// (ERT4) The frame's time step from the host clock. The clock is the Sora instance's: a NEW one (a load, a
        /// KH3 Continue, a death's respawn) starts again at 0, and Sora's own blood starts clean then (`frame`). Before,
        /// the friends kept theirs (ERT4's run: Goofy 0.75 before and after a game over -> Continue). Now the same rule.
        pub fn advance(&mut self, clock: f32) -> f32 {
            let dt = match self.clock {
                Some(before) if clock >= before => clock - before,
                Some(_) => {
                    self.members.clear();
                    self.last_step = None;
                    0.0
                }
                None => 0.0,
            };
            self.clock = Some(clock);
            dt
        }

        /// One frame for every member present (a member who left keeps his blood until he is back).
        pub fn step(&mut self, rules: &BloodRules, inputs: &[(PartyChara, BloodInputs)], dt: f32) -> Vec<(PartyChara, BloodEvent)> {
            let mut out = Vec::new();
            for (c, i) in inputs {
                let k = match self.members.iter().position(|m| m.0 == *c) {
                    Some(k) => k,
                    None => {
                        self.members.push((*c, Blood::default()));
                        self.members.len() - 1
                    }
                };
                if let Some(e) = self.members[k].1.step(rules, i, dt) {
                    out.push((*c, e));
                }
            }
            out
        }

        pub fn looks(&self) -> Vec<(PartyChara, BloodLook)> {
            self.members.iter().map(|(c, b)| (*c, BloodLook { body: b.body, blade: b.blade, fresh: b.fresh, seed: b.seed + 17.0 * (1 + *c as u8) as f32 })).collect()
        }
    }

    static LOOKS: Mutex<Vec<(PartyChara, BloodLook)>> = Mutex::new(Vec::new());

    pub fn publish(looks: &[(PartyChara, BloodLook)]) {
        if let Ok(mut g) = LOOKS.lock() {
            *g = looks.to_vec();
        }
    }

    /// (for S4) THE BLOOD ON A FRIEND AND HIS WEAPON this frame (body = him, blade = his staff / shield), as Sora's
    /// `kh_er_render::blood::BloodLook`; None: none yet.
    #[allow(dead_code)] // (S4 reads it when it draws the friends)
    pub fn party_look(c: PartyChara) -> Option<BloodLook> {
        LOOKS.lock().ok()?.iter().find(|l| l.0 == c).map(|l| l.1)
    }

    /// Whether a hit's attack name is one of the friends' attack rows (the very row: kh-party hands its table's own
    /// `&'static str` as `HitRecord::attack_name`).
    pub fn is_party_attack(name: &'static str) -> bool {
        [kh_party::Chara::Donald, kh_party::Chara::Goofy].into_iter().any(|c| row_is(c, name))
    }

    fn row_is(c: kh_party::Chara, name: &'static str) -> bool {
        kh_party::tables::attack_row(c, name).is_some_and(|r| std::ptr::eq(r.name.as_ptr(), name.as_ptr()))
    }

    /// Whose hit it is: the friend whose attack row it is (both have a row of that name, e.g. `Attack3_Jump`, and the
    /// strings are one: the friend nearer the hit). `members`: (character, feet).
    pub fn attacker(name: &'static str, at: glam::Vec3, members: &[(kh_party::Chara, glam::Vec3)]) -> Option<kh_party::Chara> {
        let mut best: Option<(kh_party::Chara, f32)> = None;
        for (c, feet) in members {
            if row_is(*c, name) {
                let d = feet.distance(at);
                if best.is_none_or(|b| d < b.1) {
                    best = Some((*c, d));
                }
            }
        }
        best.map(|b| b.0)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const R: BloodRules = BloodRules::DEFAULT;

        /// (ERT4) A new Sora host (its clock back at 0: a load, a KH3 Continue, a respawn) starts the friends clean, as
        /// Sora; time going on keeps their blood.
        #[test]
        fn ert4_a_new_host_clock_starts_the_friends_clean() {
            let r = BloodRules::DEFAULT;
            let mut p = PartyBlood::default();
            assert_eq!(p.advance(10.0), 0.0);
            let hurt = BloodInputs { hp: 30, max_hp: 100, ..BloodInputs::default() };
            p.step(&r, &[(PartyChara::Goofy, BloodInputs { hp: 100, max_hp: 100, ..BloodInputs::default() })], 0.0);
            p.step(&r, &[(PartyChara::Goofy, hurt)], 0.1);
            assert!(p.looks()[0].1.body > 0.4, "{:?}", p.looks());
            let dt = p.advance(12.5);
            assert!((dt - 2.5).abs() < 1e-6);
            assert!(p.looks()[0].1.body > 0.4, "time going on keeps it");
            assert_eq!(p.advance(0.5), 0.0, "the new host's clock");
            assert!(p.looks().is_empty(), "clean: {:?}", p.looks());
            assert_eq!(p.last_step, None);
        }

        /// S6's request (the user's rule): the save point washes every member and his weapon, as Sora's.
        #[test]
        fn the_save_point_washes_every_member_and_his_weapon() {
            let mut p = PartyBlood::default();
            let full = |c, hits| (c, BloodInputs { bleeding_hits: hits, hp: 100, max_hp: 100, ..BloodInputs::default() });
            p.step(&R, &[full(PartyChara::Donald, 0), full(PartyChara::Goofy, 0)], 0.0);
            for _ in 0..8 {
                p.step(&R, &[full(PartyChara::Donald, 1), full(PartyChara::Goofy, 1)], 1.0 / 60.0);
            }
            assert!(p.looks().iter().all(|(_, l)| l.body > 0.1 && l.blade > 0.4), "{:?}", p.looks());
            // into the save point's heal area: every member's blood fades out within blood.save_fade_s (1 s)
            let at = |c| (c, BloodInputs { hp: 100, max_hp: 100, at_save_point: true, ..BloodInputs::default() });
            let first = p.step(&R, &[at(PartyChara::Donald), at(PartyChara::Goofy)], 1.0 / 60.0);
            assert_eq!(first.iter().filter(|e| e.1 == BloodEvent::SavePointClears).count(), 2);
            for _ in 0..61 {
                p.step(&R, &[at(PartyChara::Donald), at(PartyChara::Goofy)], 1.0 / 60.0);
            }
            assert!(p.looks().iter().all(|(_, l)| l.body == 0.0 && l.blade == 0.0), "{:?}", p.looks());
            // the looks are told apart (their own splatter seeds)
            let looks = p.looks();
            assert_ne!(looks[0].1.seed, looks[1].1.seed);
        }

        #[test]
        fn a_hit_is_the_friend_whose_attack_row_it_is() {
            let fire = kh_party::tables::attack_row(kh_party::Chara::Donald, "MAG_FIRE").map(|r| r.name);
            let Some(fire) = fire else {
                eprintln!("SKIPPED: no MAG_FIRE row");
                return;
            };
            let members = [(kh_party::Chara::Donald, glam::Vec3::ZERO), (kh_party::Chara::Goofy, glam::Vec3::X * 100.0)];
            assert!(is_party_attack(fire));
            assert_eq!(attacker(fire, glam::Vec3::X * 90.0, &members), Some(kh_party::Chara::Donald));
            // a string of the same text that is not the table's (Sora's own names are other strings)
            let other: &'static str = Box::leak(String::from("MAG_FIRE").into_boxed_str());
            assert!(!is_party_attack(other));
            // a name both friends have: the nearer one
            if let (Some(d), Some(g)) = (kh_party::tables::attack_row(kh_party::Chara::Donald, "Attack3_Jump"), kh_party::tables::attack_row(kh_party::Chara::Goofy, "Attack3_Jump")) {
                if std::ptr::eq(d.name.as_ptr(), g.name.as_ptr()) {
                    assert_eq!(attacker(d.name, glam::Vec3::X * 90.0, &members), Some(kh_party::Chara::Goofy));
                } else {
                    assert_eq!(attacker(g.name, glam::Vec3::ZERO, &members), Some(kh_party::Chara::Goofy));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: BloodRules = BloodRules::DEFAULT;

    fn run(b: &mut Blood, i: BloodInputs, seconds: f32) -> Vec<BloodEvent> {
        let mut out = Vec::new();
        let steps = (seconds * 60.0).round() as usize;
        for k in 0..steps {
            let now = if k == 0 { i } else { BloodInputs { bleeding_hits: 0, ..i } };
            out.extend(b.step(&R, &now, 1.0 / 60.0));
        }
        out
    }

    #[test]
    fn hits_on_bleeding_bodies_bloody_the_blade_first_and_him_less() {
        let mut b = Blood::default();
        let hp = BloodInputs { hp: 265, max_hp: 265, ..BloodInputs::default() };
        b.step(&R, &hp, 0.0);
        for _ in 0..10 {
            b.step(&R, &BloodInputs { bleeding_hits: 1, ..hp }, 1.0 / 60.0);
        }
        assert!((b.blade - 0.6).abs() < 0.01 && (b.body - 0.2).abs() < 0.01, "{b:?}");
        assert!(b.fresh > 0.99);
        // capped at full
        for _ in 0..40 {
            b.step(&R, &BloodInputs { bleeding_hits: 1, ..hp }, 1.0 / 60.0);
        }
        assert!(b.blade > 0.999 && b.blade <= 1.0 && b.body > 0.999 && b.body <= 1.0, "{b:?}");
    }

    #[test]
    fn damage_taken_bloodies_him_small_ticks_do_not() {
        let mut b = Blood::default();
        b.step(&R, &BloodInputs { hp: 200, max_hp: 200, ..BloodInputs::default() }, 0.0);
        // a hit of a quarter of his HP
        b.step(&R, &BloodInputs { hp: 150, max_hp: 200, ..BloodInputs::default() }, 1.0 / 60.0);
        assert!((b.body - 0.2).abs() < 0.01 && b.blade == 0.0, "{b:?}");
        // a poison tick of 1 HP (0.7 %): nothing
        let before = b.body;
        b.step(&R, &BloodInputs { hp: 149, max_hp: 200, ..BloodInputs::default() }, 1.0 / 60.0);
        assert!(b.body <= before);
        // healing adds nothing
        b.step(&R, &BloodInputs { hp: 200, max_hp: 200, ..BloodInputs::default() }, 1.0 / 60.0);
        assert!(b.body <= before);
    }

    #[test]
    fn it_dries_and_fades_slowly_and_water_washes_it() {
        let mut b = Blood { body: 1.0, blade: 1.0, fresh: 1.0, ..Blood::default() };
        let dry = BloodInputs { hp: 100, max_hp: 100, ..BloodInputs::default() };
        run(&mut b, dry, 90.0);
        assert!(b.fresh < 0.01, "dried after 90 s: {b:?}");
        assert!(b.body > 0.85, "a minute and a half takes only a little: {b:?}");
        // the heaviest wetness: a full coat goes in 20 s (with the slow fade: 19.6 s)
        let mut w = Blood { body: 1.0, blade: 1.0, fresh: 0.0, ..Blood::default() };
        let events = run(&mut w, BloodInputs { wet_level: Some(4), ..dry }, 19.3);
        assert!(w.body > 0.0 && w.body < 0.05 && w.fresh >= 0.6, "{w:?}");
        let events2 = run(&mut w, BloodInputs { wet_level: Some(4), ..dry }, 1.0);
        assert!(events.is_empty() && events2 == vec![BloodEvent::Clean] && w.body == 0.0 && w.seed == 1.0);
        // light rain (level 0) washes five times slower
        let mut l = Blood { body: 1.0, blade: 1.0, ..Blood::default() };
        run(&mut l, BloodInputs { wet_level: Some(0), ..dry }, 20.0);
        assert!((l.body - (1.0 - 0.2 - 20.0 / 900.0)).abs() < 0.01, "{l:?}");
    }

    #[test]
    fn the_save_point_clears_it_in_one_second_and_new_blood_waits_until_he_leaves() {
        let mut b = Blood { body: 0.8, blade: 0.5, fresh: 1.0, ..Blood::default() };
        let base = BloodInputs { hp: 100, max_hp: 100, ..BloodInputs::default() };
        let events = run(&mut b, BloodInputs { at_save_point: true, ..base }, 0.5);
        assert_eq!(events, vec![BloodEvent::SavePointClears]);
        assert!((b.body - 0.4).abs() < 0.02 && (b.blade - 0.25).abs() < 0.02, "half way: {b:?}");
        // he steps out at once: the fade still runs to its end
        let events = run(&mut b, base, 0.6);
        assert_eq!(events, vec![BloodEvent::Clean]);
        assert_eq!((b.body, b.blade, b.seed), (0.0, 0.0, 1.0));
        // a hit inside the heal area adds nothing
        b.step(&R, &BloodInputs { at_save_point: true, ..base }, 1.0 / 60.0);
        b.step(&R, &BloodInputs { at_save_point: true, bleeding_hits: 3, ..base }, 1.0 / 60.0);
        assert_eq!(b.body, 0.0);
        // entering with no blood: nothing happens
        let mut c = Blood::default();
        assert!(run(&mut c, BloodInputs { at_save_point: true, ..base }, 0.2).is_empty());
    }

    #[test]
    fn fresh_blood_over_dried_counts_by_its_share() {
        let mut b = Blood { body: 0.6, blade: 0.0, fresh: 0.0, ..Blood::default() };
        let i = BloodInputs { hp: 100, max_hp: 100, ..BloodInputs::default() };
        b.step(&R, &i, 0.0);
        b.step(&R, &BloodInputs { bleeding_hits: 1, ..i }, 0.0);
        // 0.06 + 0.02 new over 0.6 old
        assert!((b.fresh - 0.08 / 0.68).abs() < 0.01, "{b:?}");
    }

    /// (FINAL4) RSF2's finding (GG's run: `blood: (party) Goofy body 0.13` with no hit taken, at the frame
    /// `party.word leader goofy` made him lead): a maximum rescaled with the share kept (AI Goofy 160 / 160 made the leader:
    /// 138 / 138) and a guarded / absorbed hit add no blood, for any character; a real hit still does, and a hit that
    /// lands in the same frame as a rescale counts its own share.
    #[test]
    fn final4_a_guarded_hit_or_a_rescaled_maximum_adds_no_blood() {
        let mut b = Blood::default();
        b.step(&R, &BloodInputs { hp: 160, max_hp: 160, ..BloodInputs::default() }, 0.0);
        // made the leader: the bar is rescaled, full stays full
        b.step(&R, &BloodInputs { hp: 138, max_hp: 138, ..BloodInputs::default() }, 1.0 / 60.0);
        assert_eq!(b.body, 0.0, "a rescale is no hit: {b:?}");
        // a raised maximum with the HP kept (a boost) adds none either
        b.step(&R, &BloodInputs { hp: 138, max_hp: 150, ..BloodInputs::default() }, 1.0 / 60.0);
        assert_eq!(b.body, 0.0, "{b:?}");
        // a rescale with the share kept at half (80 / 160 -> 69 / 138)
        let mut h = Blood::default();
        h.step(&R, &BloodInputs { hp: 80, max_hp: 160, ..BloodInputs::default() }, 0.0);
        h.step(&R, &BloodInputs { hp: 69, max_hp: 138, ..BloodInputs::default() }, 1.0 / 60.0);
        assert_eq!(h.body, 0.0, "{h:?}");
        // a GUARDED hit: whatever the HP did in that frame (chip damage, a rescale), no blood
        b.step(&R, &BloodInputs { hp: 150, max_hp: 150, ..BloodInputs::default() }, 1.0 / 60.0);
        b.step(&R, &BloodInputs { hp: 120, max_hp: 150, guarded: true, ..BloodInputs::default() }, 1.0 / 60.0);
        assert_eq!(b.body, 0.0, "a guarded hit bleeds nothing: {b:?}");
        // the next real hit of a fifth of his bar still bloodies him (0.2 x 0.8)
        b.step(&R, &BloodInputs { hp: 90, max_hp: 150, ..BloodInputs::default() }, 1.0 / 60.0);
        assert!((b.body - 0.16).abs() < 0.01, "{b:?}");
        // a hit in the same frame as a rescale counts only the share he lost (100 / 100 -> 40 / 80: half the bar)
        let mut c = Blood::default();
        c.step(&R, &BloodInputs { hp: 100, max_hp: 100, ..BloodInputs::default() }, 0.0);
        c.step(&R, &BloodInputs { hp: 40, max_hp: 80, ..BloodInputs::default() }, 0.0);
        assert!((c.body - 0.4).abs() < 0.01, "{c:?}");
    }

    /// (FINAL4) The party's blood takes the same rule per member: Goofy's guarded frame bleeds nothing, Donald's real
    /// hit in the same frame does.
    #[cfg(feature = "party")]
    #[test]
    fn final4_a_members_guarded_hit_bleeds_nothing_and_the_others_hit_does() {
        use kh_host::party::PartyChara;
        let mut p = party::PartyBlood::default();
        let i = |hp, guarded| BloodInputs { hp, max_hp: 100, guarded, ..BloodInputs::default() };
        p.step(&R, &[(PartyChara::Donald, i(100, false)), (PartyChara::Goofy, i(100, false))], 0.0);
        p.step(&R, &[(PartyChara::Donald, i(75, false)), (PartyChara::Goofy, i(75, true))], 1.0 / 60.0);
        let looks = p.looks();
        let body = |c| looks.iter().find(|l| l.0 == c).map(|l| l.1.body).unwrap();
        assert!((body(PartyChara::Donald) - 0.2).abs() < 0.01 && body(PartyChara::Goofy) == 0.0, "{looks:?}");
    }

    /// (FINAL4) The note of a guarded hit reaches that character's next step only, and only once.
    #[test]
    fn final4_the_guarded_note_is_per_character_and_taken_once() {
        use kh_host::party::PartyChara;
        let _ = take_guarded(PartyChara::Goofy);
        let _ = take_guarded(PartyChara::Donald);
        note_guarded(PartyChara::Goofy);
        note_guarded(PartyChara::Goofy);
        assert!(!take_guarded(PartyChara::Donald));
        assert!(take_guarded(PartyChara::Goofy));
        assert!(!take_guarded(PartyChara::Goofy), "taken once");
    }

    #[test]
    fn the_sheet_rows_are_the_defaults() {
        let text = include_str!("../../../sheets/port/sora-er-host.json");
        let (rules, problems) = BloodRules::from_sheet(text);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(rules, BloodRules::DEFAULT);
        let n = serde_json::from_str::<serde_json::Value>(text).unwrap()["rows"].as_array().unwrap().iter().filter(|r| r[0].as_str().is_some_and(|id| id.starts_with("blood."))).count();
        assert_eq!(n, 9, "every rule has its row");
    }
}
