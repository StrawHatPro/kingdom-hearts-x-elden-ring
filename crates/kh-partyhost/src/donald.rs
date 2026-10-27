//! (DON1) THE DONALD OVERHAUL IN THE PARTY HOST (documentation/DONALD_OVERHAUL.md 9; build/sora_tmp/party/DON_PLAN.md):
//!
//! - THE ROSTER'S LEARNED MAGIC ([`PartyRuntime::don_learned`]): one [`DonLearned`] for the roster, whether Donald has
//!   joined or not (9: "He gets it even if he isn't in the party yet, and joins with every tier the party has earned").
//!   A boss step goes through [`PartyRuntime::don_step`]; every step (and a spawn) copies it onto Donald's status
//!   (`FriendStatus::donald.learned`).
//! - THE SAVE: the party record's ADDITIVE key `donald_magic` (format 2 / friends_version 5 readers ignore it). A save
//!   without it (older than this) loads the start's magic and sets `don_unrecorded`: the Elden Ring host rebuilds the
//!   steps once from the beaten-boss records (stream DON-PROG). His abilities (`DON_*`) ride in the v5 friend record's
//!   ability list (save.rs).
//! - THE SHEETS AT RUN TIME ([`sheet_data`]): sheets/port/party-don-core.json and party-don-spells.json are read again
//!   from the repo when present (the route of kh-er bossreward.rs), so a changed number needs no rebuild; else the
//!   compiled values. [`apply_ctl`] lays `don.*` control keys on top. The host installs the result
//!   (`kh_party::donald::install`).

use kh_party::donald::{self, DonData, DonLearned, DonStep, Grant};
use kh_party::donald::ability_info::{install_prog, ProgData}; // (DON-PROG)
use kh_party::donald::{aggro, DonAbility}; // (DON-PROG)
use serde_json::{json, Value};

use crate::runtime::PartyRuntime;
use crate::say;

/// The party record's key.
pub const RECORD_KEY: &str = "donald_magic";

/// The record: `{"v": 1, "tier": [6], "line2": [6], "arcane": [comet, gravity, meteor, ultima, zettaflare], "mp_steps": n}`.
pub fn to_json(l: &DonLearned) -> Value {
    let (tier, line2, arcane, mp) = l.parts();
    json!({ "v": 1, "tier": tier, "line2": line2, "arcane": arcane, "mp_steps": mp })
}

/// The record's value (None: not an object, i.e. a save older than the overhaul). Odd values are clamped.
pub fn from_json(v: &Value) -> Option<DonLearned> {
    if !v.is_object() {
        return None;
    }
    let list = |k: &str| v[k].as_array().map(|a| a.iter().map(|x| x.as_u64().unwrap_or(0)).collect::<Vec<u64>>()).unwrap_or_default();
    Some(DonLearned::from_parts(&list("tier"), &list("line2"), &list("arcane"), v["mp_steps"].as_u64().unwrap_or(0)))
}

impl PartyRuntime {
    /// The roster's learned magic.
    pub fn don_learned(&self) -> DonLearned {
        self.don_learned
    }

    /// ONE BOSS STEP for Donald (joined or not): what it gave him (the BONUS! panel's lines), now on his status too.
    pub fn don_step(&mut self, step: DonStep) -> Vec<Grant> {
        let got = self.don_learned.apply(step);
        let d = donald::data();
        say!("party: Donald's magic step {}: {}", step.token(), got.iter().map(|g| g.label(&d)).collect::<Vec<_>>().join(", "));
        self.sync_donald();
        got
    }

    /// Sets the roster's learned magic (a load, a catch-up) and copies it onto Donald.
    pub fn set_don_learned(&mut self, l: DonLearned) {
        self.don_learned = l;
        self.sync_donald();
    }

    /// The roster's learned magic onto Donald's status (every step; a spawn / load may have rebuilt it); (DON-PROG) an
    /// old save's X3D kit migrated onto his overhaul abilities (once), the abilities his level has reached given in level
    /// order. The test level (no real progression) shows every spell (`DonLearned::all()`, his loadout is the level-99
    /// one: `FriendStatus::don_test_loadout`).
    pub fn sync_donald(&mut self) {
        let l = if self.progression { self.don_learned } else { DonLearned::all() };
        for (c, id) in self.members.clone() {
            let Some(s) = self.party.status_mut(id) else { continue };
            if s.donald.is_none() {
                continue;
            }
            let migrated = s.kit_migrate();
            let dropped = s.don_drop_kh3_spells(); // an old save's DONALD_FIRE .. CURE, once
            let learned = s.don_sync_level();
            if let Some(d) = s.donald.as_mut() {
                if d.learned != l {
                    d.learned = l;
                }
            }
            if !migrated.is_empty() {
                say!("party: {}'s X3D kit migrated onto the overhaul: {}", c.name(), migrated.iter().map(|k| k.name()).collect::<Vec<_>>().join(", "));
            }
            if !dropped.is_empty() {
                say!("party: {}'s KH3 spell abilities of an older save dropped (the overhaul's magic replaced them): {}", c.name(), dropped.join(", "));
            }
            if !learned.is_empty() {
                say!("party: {} has learned by level: {}", c.name(), learned.iter().map(|k| k.label()).collect::<Vec<_>>().join(", "));
            }
        }
    }

    /// The record of a loaded party record (none in an older save: the start's magic, `don_unrecorded`).
    pub fn load_don_record(&mut self, v: &Value) {
        match from_json(v) {
            Some(l) => {
                self.don_unrecorded = false;
                self.don_learned = l;
            }
            None => {
                self.don_unrecorded = true;
                self.don_learned = DonLearned::start();
            }
        }
        self.sync_donald();
    }
}

/// One sheet file's rows as (key, value) pairs for [`DonData::set`]: a params sheet's `id` / numeric `value`, a spell
/// sheet's numeric columns as `spell.<id>.<column>`.
pub fn sheet_pairs(text: &str) -> Vec<(String, f32)> {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    let cols: Vec<&str> = v["columns"].as_array().map(|c| c.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let mut out = Vec::new();
    for row in v["rows"].as_array().into_iter().flatten() {
        let Some(id) = row[0].as_str() else { continue };
        if cols.get(1) == Some(&"value") {
            if let Some(x) = row[1].as_f64() {
                out.push((id.to_string(), x as f32));
            }
            continue;
        }
        for (i, c) in cols.iter().enumerate().skip(1).filter(|(_, c)| **c != "level") /* the level names the row */ {
            let cell = &row[i];
            let x = cell.as_f64().or_else(|| cell.as_bool().map(|b| b as u8 as f64));
            if let Some(x) = x {
                out.push((format!("spell.{id}.{c}"), x as f32));
            }
        }
    }
    out
}

/// THE SHEETS AT RUN TIME: the compiled data with the repo's sheet files laid on top when they can be read (a changed
/// number needs no rebuild). Returns the data and the keys a sheet named that the build does not know.
pub fn sheet_data() -> (DonData, Vec<String>) {
    let mut d = DonData::sheet();
    let mut unknown = Vec::new();
    for name in ["party-don-core", "party-don-spells"] {
        let path = kh_gendata::project_file(&format!("sheets/port/{name}.json")).to_string();
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for (k, v) in sheet_pairs(&text) {
            if !d.set(&k, v) {
                unknown.push(k);
            }
        }
    }
    (d, unknown)
}

/// `don.<key> = value` control keys on top (`pairs`: every key / value of the host's control file); the keys applied.
pub fn apply_ctl<'a>(d: &mut DonData, pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<String> {
    let mut done = Vec::new();
    for (k, v) in pairs {
        if !k.trim().starts_with("don.") {
            continue;
        }
        if let Ok(x) = v.trim().parse::<f32>() {
            if d.set(k, x) {
                done.push(k.trim().to_string());
            }
        }
    }
    done
}

// =================================================================================================== (DON-PROG)

/// (DON-PROG) THE LEVEL UP! WINDOW'S NEW ABILITIES of a character from level `from` to `to` (Donald's level tree,
/// `DonAbility::level`): their names, in level order. Empty for Goofy and Sora (their own rules).
pub fn level_abilities(c: kh_host::party::PartyChara, from: u32, to: u32) -> Vec<String> {
    if c != kh_host::party::PartyChara::Donald {
        return Vec::new();
    }
    DonAbility::learned_between(from, to).into_iter().map(|k| k.label().to_string()).collect()
}

/// (DON-PROG) The progression sheet's rows as (key, value) pairs for [`ProgData::set`] (`ability.<name>.ap` / `.level` /
/// `.priority`, the rules' `value`).
pub fn prog_pairs(text: &str) -> Vec<(String, f32)> {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    let cols: Vec<&str> = v["columns"].as_array().map(|c| c.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let mut out = Vec::new();
    for row in v["rows"].as_array().into_iter().flatten() {
        let Some(id) = row[0].as_str() else { continue };
        for (i, c) in cols.iter().enumerate().skip(1) {
            if !matches!(*c, "ap" | "level" | "priority" | "value") {
                continue;
            }
            if let Some(x) = row[i].as_f64() {
                out.push(if *c == "value" && !id.starts_with("ability.") { (id.to_string(), x as f32) } else { (format!("{id}.{c}"), x as f32) });
            }
        }
    }
    out
}

/// (DON-PROG) THE PROGRESSION SHEET AT RUN TIME (sheets/port/party-don-progression.json read again when present, else
/// the compiled rows) and the keys it named that the build does not know.
pub fn prog_sheet_data() -> (ProgData, Vec<String>) {
    let mut d = ProgData::sheet();
    let mut unknown = Vec::new();
    let path = kh_gendata::project_file("sheets/port/party-don-progression.json").to_string();
    if let Ok(text) = std::fs::read_to_string(&path) {
        for (k, v) in prog_pairs(&text) {
            if !d.set(&k, v) {
                unknown.push(k);
            }
        }
    }
    (d, unknown)
}

/// (DON-PROG) What [`install_all`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Installed {
    /// The `don.*` ctl keys applied.
    pub applied: Vec<String>,
    /// Keys no Donald sheet knows (a sheet's row the build lacks, a `don.*` key with a typo or no number).
    pub unknown: Vec<String>,
}

/// (DON-PROG) The last tuning's inputs: the host's `don.*` ctl pairs and Sora's MP cost factor in force.
static TUNING: std::sync::Mutex<Option<(Vec<(String, String)>, Option<f32>)>> = std::sync::Mutex::new(None);

/// (DON-PROG) TUNING WITHOUT A REBUILD, the whole route (the host calls it at the party's start and at every ctl
/// reload): the two foundation sheets ([`sheet_data`]) and the progression sheet read again; Donald's MP costs scaled
/// by SORA'S DIFFICULTY FACTOR (`tune.cost_scale` = Sora's `Status::mp_cost_scale`: Standard x1.5, Critical x1.0, the
/// user: "donald should work like Sora where spells cost MP"; the factor last seen by [`follow_cost_scale`]); then the
/// `don.<key> = value` ctl keys on top (a key of either sheet; `don.cost.scale` overrides the factor); both installed
/// (`donald::install`, `donald::ability_info::install_prog`).
pub fn install_all<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Installed {
    let pairs: Vec<(String, String)> = pairs.into_iter().filter(|(k, _)| k.trim().starts_with("don.")).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let scale = TUNING.lock().ok().and_then(|t| t.as_ref().and_then(|t| t.1));
    install_with(pairs, scale)
}

/// (DON-PROG) THE TUNING ITSELF (pure: nothing installed): the sheets read again, Sora's factor, the ctl keys on top.
pub fn tuned(pairs: &[(String, String)], scale: Option<f32>) -> (DonData, ProgData, Installed) {
    let (mut d, mut unknown) = sheet_data();
    let (mut p, u2) = prog_sheet_data();
    unknown.extend(u2);
    if let Some(s) = scale.filter(|s| *s > 0.0) {
        d.tune.cost_scale = s;
    }
    let mut applied = Vec::new();
    for (k, v) in pairs {
        match v.parse::<f32>() {
            Ok(x) if d.set(k, x) || p.set(k, x) => applied.push(k.clone()),
            _ => unknown.push(k.clone()),
        }
    }
    (d, p, Installed { applied, unknown })
}

fn install_with(pairs: Vec<(String, String)>, scale: Option<f32>) -> Installed {
    let (d, p, done) = tuned(&pairs, scale);
    donald::install(d);
    install_prog(p);
    if let Ok(mut t) = TUNING.lock() {
        *t = Some((pairs, scale));
    }
    done
}

/// (DON-PROG) SORA'S MP COST FACTOR FOLLOWED (every party step, `PartyRuntime::don_after_step`): when it differs from
/// the one in force, the tuning is installed again with it (the last ctl keys kept). The installed result when it changed.
pub fn follow_cost_scale(scale: f32) -> Option<Installed> {
    let (pairs, now) = TUNING.lock().ok().map(|t| t.clone().unwrap_or_default()).unwrap_or_default();
    if now.is_some_and(|n| (n - scale).abs() < 1e-6) || !(scale > 0.0) {
        return None;
    }
    let done = install_with(pairs, Some(scale));
    say!("party: Donald's MP costs follow Sora's difficulty factor x{scale}{}", if done.applied.iter().any(|k| k == "don.cost.scale") { " (overridden by the ctl key don.cost.scale)" } else { "" });
    Some(done)
}

/// (DON-PROG) A record of Donald's steps for a boss row in the character's records (`don:<row id>`, beside Sora's
/// `boss:<row id>`): once per boss.
pub fn don_record(id: &str) -> String {
    format!("don:{id}")
}

/// (DON-PROG) The steps of a boss's `don.<row>` tokens; the tokens not understood.
pub fn steps_of(tokens: &[String]) -> (Vec<DonStep>, Vec<String>) {
    let mut steps = Vec::new();
    let mut bad = Vec::new();
    for t in tokens {
        match DonStep::parse(t) {
            Some(s) => steps.push(s),
            None => bad.push(t.clone()),
        }
    }
    (steps, bad)
}

/// (DON-PROG) THE RULE (pure): the boss rows whose steps Donald is due: the boss is beaten (`boss:<id>`, Sora's record)
/// and its steps are not recorded (`don:<id>`). This is both the live grant and THE CATCH-UP of an older save (no
/// `don:` record at all: every beaten boss's steps once).
pub fn don_due<'a>(rows: &'a [kh_host::bossrewards::DonRow], records: &[String]) -> Vec<&'a kh_host::bossrewards::DonRow> {
    rows.iter().filter(|r| kh_host::bossrewards::recorded(records, &r.boss) && !records.iter().any(|k| k.eq_ignore_ascii_case(&don_record(&r.boss)))).collect()
}

impl PartyRuntime {
    /// (DON-PROG) A BOSS'S STEPS FOR DONALD (joined or not): every step through [`PartyRuntime::don_step`]; the grants in
    /// panel order.
    pub fn don_boss(&mut self, steps: &[DonStep]) -> Vec<Grant> {
        let mut out = Vec::new();
        for s in steps {
            out.extend(self.don_step(*s));
        }
        out
    }

    /// (DON-PROG) THE DUE BOSS ROWS GIVEN (pure over `records`, which gets the `don:` records): (row, grants) per row, in
    /// the table's order. The catch-up and the live grants alike.
    pub fn don_claim_due(&mut self, rows: &[kh_host::bossrewards::DonRow], records: &mut Vec<String>) -> Vec<(String, Vec<Grant>)> {
        let due: Vec<kh_host::bossrewards::DonRow> = don_due(rows, records).into_iter().cloned().collect();
        let mut out = Vec::new();
        for row in due {
            let (steps, _) = steps_of(&row.steps);
            let grants = self.don_boss(&steps);
            records.push(don_record(&row.boss));
            out.push((row.boss.clone(), grants));
        }
        if !out.is_empty() {
            self.don_unrecorded = false;
        }
        out
    }

    /// (DON-PROG) THE TEST LEVEL'S ENEMY CHOICE WITH DONALD'S AGGRO (5.4; `victim` of a fixture dummy, the nearest rule):
    /// Taunt Pass first (an ally guarding: the enemies near Donald take him), then the nearest with Donald's weight and
    /// Distraction. `None`: no Donald in the party, the host's own rule stands. The rolls are stable per enemy for
    /// `aggro.er_period` s (an enemy keeps its choice that long). Never moves a hit: this is whom it attacks next.
    pub fn don_victim(&self, dummy: u32, from: kh_core::Vec3, sora: kh_core::Vec3) -> Option<crate::runtime::Victim> {
        use crate::runtime::{party_chara, Victim};
        let dv = self.current.iter().find(|v| v.chara == kh_party::Chara::Donald && !v.ko)?;
        let abilities = self.party.status(dv.id).and_then(|s| s.donald.as_ref()).map(|d| d.abilities)?;
        let ctx = self.party.don_prog.aggro_of(dv.id);
        let sora_in = self.lead.sora_active;
        let who = |m: kh_party::MemberId| -> Option<Victim> {
            if m == kh_party::donald::cast::SORA_MEMBER {
                return sora_in.then_some(Victim::Sora);
            }
            self.current.iter().find(|v| v.id == m && !v.ko).map(|v| Victim::Friend(party_chara(v.chara)))
        };
        // Taunt Pass
        if let Some(ally) = self.party.don_prog.taunt_now() {
            if ally != dv.id && aggro::taunt_pass(&abilities, &ctx, from.distance(dv.position)) {
                if let Some(v) = who(ally) {
                    return Some(v);
                }
            }
        }
        // the nearest rule with his weight and Distraction
        let mut cands: Vec<(aggro::Cand, Victim)> = Vec::new();
        if sora_in {
            cands.push((aggro::Cand { dist: from.distance(sora), donald: false, to_donald: sora.distance(dv.position) }, Victim::Sora));
        }
        for v in self.current.iter().filter(|v| !v.ko) {
            cands.push((aggro::Cand { dist: from.distance(v.position), donald: v.id == dv.id, to_donald: v.position.distance(dv.position) }, Victim::Friend(party_chara(v.chara))));
        }
        let list: Vec<aggro::Cand> = cands.iter().map(|c| c.0).collect();
        let bucket = (self.party.don_prog.clock / donald::ability_info::prog().value("aggro.er_period").max(0.1)).floor() as u64;
        let pick = aggro::choose(&list, &abilities, &ctx, aggro::roll(dummy as u64, bucket, 1), aggro::roll(dummy as u64, bucket, 2))?;
        Some(cands[pick].1)
    }

    /// (DON-PROG) AFTER THE PARTY'S STEP, THE AURAS ON SORA AND SORA'S SIDE OF THE RULES: the Water aura speeds his MP
    /// Charge, Light's Regen heals him (when he is in the party, within reach of an attuned Donald); his Thunder hits
    /// are noted (AI Donald's Water, doc 11) and his guards are Taunt Pass's (5.4).
    pub fn don_after_step(&mut self, sora: &mut kh_core::Sora, events: &[kh_core::Event], dt: f32) {
        let _ = follow_cost_scale(sora.status().mp_cost_scale); // Donald's MP costs scale with Sora's difficulty
        for e in events {
            match e.kind {
                kh_core::host::EventKind::Hit { record, .. } if record.attribute == kh_core::enums::ETresDamageAttribute::Thunder && record.damage > 0.0 => donald::ai::note_sora_thunder(&mut self.party),
                kh_core::host::EventKind::PlayerGuarded { .. } => donald::ai::note_guard(&mut self.party, donald::cast::SORA_MEMBER),
                _ => {}
            }
        }
        if !self.lead.sora_active || self.party.don_prog.aura.is_empty() {
            return;
        }
        let at = sora.output().position;
        let fx = self.party.don_prog.aura.at(at);
        let s = sora.status_mut();
        if s.mp_charging && fx.mp_regen_mult > 1.0 {
            s.mp_charge_timer = (s.mp_charge_timer - dt.max(0.0) * (fx.mp_regen_mult - 1.0) * s.mp_charge_rate()).max(0.0);
        }
        let (hp, max) = (s.hp, s.max_hp());
        if hp > 0 && hp < max {
            let n = self.party.don_prog.aura.regen_points(donald::cast::SORA_MEMBER, max, fx.regen_pct_per_s, dt);
            if n > 0 {
                sora.status_mut().set_hp(hp + n);
            }
        }
    }
}

/// (DON-PROG) A HIT OF THE PARTY WITH THE FIRE AURA: its damage x the aura's attack share at the attacker's place (the
/// hits of the members, their spells and missiles, and Sora's through [`crate::SoraTargets`]).
pub fn aura_hit(aura: &donald::aura::PartyAura, hit: &kh_core::HitRecord) -> kh_core::HitRecord {
    let mut h = *hit;
    if hit.attacker.valid && !aura.is_empty() {
        let m = aura.at(hit.attacker.position).attack_mult;
        if m > 1.0 {
            h.damage *= m;
        }
    }
    h
}

/// (DON-PROG) THE PARTY'S TARGETS WITH THE FIRE AURA: every call goes to the host's targets (EVERY method of
/// `kh_core::Targets` is forwarded: a method added to the trait must be added here, `aura_targets_forward_every_method`); a hit's damage is raised by
/// [`aura_hit`] (the aura of the step before: the party's step makes the new one).
pub struct AuraTargets<'a> {
    pub inner: &'a mut dyn kh_core::Targets,
    pub aura: donald::aura::PartyAura,
}

impl<'a> AuraTargets<'a> {
    pub fn new(inner: &'a mut dyn kh_core::Targets, party: &kh_party::Party) -> AuraTargets<'a> {
        AuraTargets { inner, aura: party.don_prog.aura.clone() }
    }
}

impl kh_core::Targets for AuraTargets<'_> {
    fn enumerate(&self, out: &mut Vec<kh_core::host::TargetInfo>) {
        self.inner.enumerate(out)
    }
    fn script_hold(&mut self, id: kh_core::TargetId, hold: Option<kh_core::host::ScriptHold>) {
        self.inner.script_hold(id, hold) // (ZETTA) Zettaflare's held target
    }
    fn held_animated(&self, id: kh_core::TargetId) -> bool {
        self.inner.held_animated(id) // (ZETTA)
    }
    fn script_finale(&mut self, id: kh_core::TargetId, finale: Option<kh_core::host::ScriptFinale>) {
        self.inner.script_finale(id, finale) // (ZETTA)
    }
    fn drawn_extent(&self, id: kh_core::TargetId) -> Option<(f32, f32)> {
        self.inner.drawn_extent(id) // (ZETTA 2026-10-23) what the host draws of it (the framing and the beam's cover)
    }
    fn info(&self, id: kh_core::TargetId) -> Option<kh_core::host::TargetInfo> {
        self.inner.info(id)
    }
    fn hurt_shapes(&self, id: kh_core::TargetId, out: &mut Vec<kh_core::host::HurtShape>) {
        self.inner.hurt_shapes(id, out)
    }
    fn apply_hit(&mut self, hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
        let h = aura_hit(&self.aura, hit);
        self.inner.apply_hit(&h)
    }
    fn defense(&self, id: kh_core::TargetId) -> Option<kh_core::host::TargetDefense> {
        self.inner.defense(id)
    }
    fn allies(&self, out: &mut Vec<kh_core::host::Ally>) {
        self.inner.allies(out)
    }
    fn heal_ally(&mut self, id: kh_core::TargetId, amount: i32, effect: u8) {
        self.inner.heal_ally(id, amount, effect)
    }
    fn lock_on_range(&self, id: kh_core::TargetId) -> f32 {
        self.inner.lock_on_range(id)
    }
    fn engaged_count(&self) -> u32 {
        self.inner.engaged_count()
    }
    fn body(&self, id: kh_core::TargetId, part: u32) -> kh_core::host::TargetBody {
        self.inner.body(id, part)
    }
    fn take_armor(&mut self, id: kh_core::TargetId, power: i32, weak: bool) -> i32 {
        self.inner.take_armor(id, power, weak)
    }
    fn take_reaction_power(&mut self, id: kh_core::TargetId, power: i32) -> i32 {
        self.inner.take_reaction_power(id, power)
    }
    fn rush_suppression(&self, id: kh_core::TargetId) -> bool {
        self.inner.rush_suppression(id)
    }
    fn terrain_dz(&self, id: kh_core::TargetId) -> f32 {
        self.inner.terrain_dz(id)
    }
    fn push_body(&self, id: kh_core::TargetId) -> Option<kh_core::host::PushBody> {
        self.inner.push_body(id)
    }
    fn reaction_state(&self, id: kh_core::TargetId) -> u8 {
        self.inner.reaction_state(id)
    }
    fn attack_shapes(&self, out: &mut Vec<kh_core::host::AttackShape>) {
        self.inner.attack_shapes(out)
    }
    fn touched(&mut self, hit: &kh_core::HitRecord) {
        self.inner.touched(hit)
    }
    fn finish_kind(&self, id: kh_core::TargetId) -> kh_core::host::FinishKind {
        self.inner.finish_kind(id)
    }
    fn foe_level(&self, id: kh_core::TargetId) -> Option<u8> {
        self.inner.foe_level(id)
    }
    fn damage_to_hp(&self) -> f32 {
        self.inner.damage_to_hp()
    }
    fn in_revenge(&self, id: kh_core::TargetId) -> bool {
        self.inner.in_revenge(id)
    }
    fn end_revenge(&mut self, id: kh_core::TargetId) {
        self.inner.end_revenge(id)
    }
    fn friends(&self, out: &mut Vec<kh_core::host::Friend>) {
        self.inner.friends(out)
    }
    fn friend_link(&mut self, req: kh_core::host::FriendLinkRequest) {
        self.inner.friend_link(req)
    }
    fn size_class(&self, id: kh_core::TargetId) -> u8 {
        self.inner.size_class(id)
    }
    fn element_affinity(&self, id: kh_core::TargetId) -> Option<[i32; 8]> {
        self.inner.element_affinity(id)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use kh_party::donald::{ArcaneLine, DonAbility, Element};

    #[test]
    fn the_record_round_trips_and_an_old_save_reads_as_the_start() {
        let mut l = DonLearned::start();
        l.apply(DonStep::Element(Element::Thunder));
        l.apply(DonStep::Line2(Element::Light)); // (DONLINE2) Banish
        l.apply(DonStep::Line2(Element::Water));
        l.apply(DonStep::Arcane(ArcaneLine::Comet));
        assert_eq!(from_json(&to_json(&l)), Some(l));
        // (DONLINE2) a record of a save made before the line-2 change (Flare Lv 1 / Flood Lv 1 from the old start) keeps them:
        // a load never strips a learned spell
        let old = json!({ "v": 1, "tier": [1, 0, 0, 1, 0, 0], "line2": [1, 0, 0, 1, 0, 0], "arcane": [0, 1, 0, 0, 0], "mp_steps": 0 });
        let o = from_json(&old).unwrap();
        assert_eq!((o.line2(Element::Fire), o.line2(Element::Water)), (1, 1));
        assert_eq!(from_json(&Value::Null), None);
        let odd = json!({ "tier": [9, "x"], "arcane": [5] });
        let o = from_json(&odd).unwrap();
        assert_eq!((o.tier[0], o.tier[1], o.arcane(ArcaneLine::Comet), o.arcane(ArcaneLine::Gravity)), (4, 0, 2, 1));
    }

    #[test]
    fn the_runtime_sheets_match_the_compiled_ones() {
        let (d, unknown) = sheet_data();
        assert!(unknown.is_empty(), "keys the build does not know: {unknown:?}");
        assert_eq!(d, DonData::sheet(), "the repo's sheets are the compiled ones (regenerate with node tools/gen_port.js after a change)");
        let mut d = DonData::sheet();
        let done = apply_ctl(&mut d, [("don.attune.cast", "0.4"), ("don.spell.l1.fire.1.cost_att", "2"), ("hud.kh", "1"), ("don.nope", "1")]);
        assert_eq!(done, ["don.attune.cast", "don.spell.l1.fire.1.cost_att"]);
        assert_eq!((d.tune.attune_cast, d.row("l1.fire.1").unwrap().cost_att), (0.4, 2.0));
    }

    fn new() -> PartyRuntime {
        let mut rt = PartyRuntime::new(crate::RuntimeConfig { wanted: true, test: crate::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        rt.progression = true;
        rt
    }

    #[test]
    fn steps_reach_donald_joined_or_not_and_the_party_record_keeps_them() {
        let mut rt = new();
        // not joined: the roster keeps it
        assert_eq!(rt.don_step(DonStep::Element(Element::Aero)), vec![Grant::Tier(Element::Aero, 1), Grant::Attunement(Element::Aero)]); // (DONLINE2) no line 2
        rt.spawn_one(kh_host::party::PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let id = rt.id(kh_host::party::PartyChara::Donald).unwrap();
        let st = rt.party.status(id).unwrap().donald.as_ref().unwrap();
        assert_eq!(st.learned.tier(Element::Aero), 1, "he joins with every tier the party has earned");
        rt.don_step(DonStep::Element(Element::Aero));
        assert_eq!(rt.party.status(id).unwrap().donald.as_ref().unwrap().learned.tier(Element::Aero), 2);
        // his abilities ride in the friend record
        rt.party.status_mut(id).unwrap().donald.as_mut().unwrap().abilities.give(DonAbility::SwiftAttune);
        let record = rt.party_record();
        assert!(record[RECORD_KEY].is_object());
        let mut back = new();
        back.spawn_one(kh_host::party::PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        back.reset_friends_for_load(0);
        back.load_party_record(&record);
        assert!(!back.don_unrecorded);
        assert_eq!(back.don_learned().tier(Element::Aero), 2);
        let bid = back.id(kh_host::party::PartyChara::Donald).unwrap();
        let bst = back.party.status(bid).unwrap().donald.as_ref().unwrap();
        assert_eq!(bst.learned.tier(Element::Aero), 2);
        assert!(bst.abilities.owned(DonAbility::SwiftAttune));
        // an old record (no key): the start's magic, flagged for the host's catch-up
        let mut old = record.clone();
        old.as_object_mut().unwrap().remove(RECORD_KEY);
        let mut o = new();
        o.load_party_record(&old);
        assert!(o.don_unrecorded && o.don_learned() == DonLearned::start());
    }
}

/// (DON-PROG) the host's side: the progression sheet at run time, the boss steps and the catch-up, the test level's
/// enemy choice with Donald's aggro, the auras on Sora.
#[cfg(test)]
mod prog_host_tests {
    use super::*;
    use kh_host::party::PartyChara;
    use kh_party::donald::{ArcaneLine, Element};

    fn rt(progression: bool) -> PartyRuntime {
        let mut rt = PartyRuntime::new(crate::RuntimeConfig { wanted: true, test: crate::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        rt.progression = progression;
        rt
    }

    fn table() -> kh_host::bossrewards::Table {
        kh_host::bossrewards::Table::parse(kh_host::bossrewards::SHEET)
    }

    #[test]
    fn the_progression_sheet_at_run_time_is_the_compiled_one_and_takes_ctl_keys() {
        let (p, unknown) = prog_sheet_data();
        assert!(unknown.is_empty(), "{unknown:?}");
        assert_eq!(p, ProgData::sheet(), "regenerate with node tools/gen_port.js after a sheet change");
        // (the installed data is the process's: every runtime test's step re-installs it with Sora's factor, so the values are
        // checked on the pure tuning; the install route only for its report)
        let keys = |list: &[(&str, &str)]| list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
        let done = install_all([("don.aggro.low_profile", "0.2"), ("don.ability.scan.ap", "1"), ("don.attune.cast", "0.6"), ("don.nope", "1"), ("don.aggro.hide_behind", "x"), ("hud.kh", "1")]);
        assert_eq!(done.applied, ["don.aggro.low_profile", "don.ability.scan.ap", "don.attune.cast"]);
        assert_eq!(done.unknown, ["don.nope", "don.aggro.hide_behind"]);
        // DONALD'S MP COSTS FOLLOW SORA'S DIFFICULTY FACTOR (Standard x1.5, Critical x1.0); `don.cost.scale` overrides it
        assert_eq!(tuned(&[], Some(1.5)).0.tune.cost_scale, 1.5);
        assert_eq!(tuned(&keys(&[("don.attune.cast", "0.6")]), Some(1.5)).0.tune.cost_scale, 1.5, "a ctl reload keeps Sora's factor");
        assert_eq!(tuned(&keys(&[("don.cost.scale", "2")]), Some(1.0)).0.tune.cost_scale, 2.0, "the ctl key overrides it");
        assert_eq!(tuned(&[], Some(1.0)).0.tune.cost_scale, 1.0, "no key: Sora's factor (Critical, KH3's x1.0)");
        assert_eq!(tuned(&[], None).0, DonData::sheet(), "no factor known yet: the sheet");
        // the price of a spell scales with it (the core's cast rule)
        let core = kh_party::donald::DonCore::default();
        let price = |scale: f32| core.check_cast(kh_party::donald::SpellRef::Line1(Element::Fire, 1), &DonLearned::start(), 999, 999, false, &tuned(&[], Some(scale)).0).map(|p| p.mp);
        let (std_mp, crit_mp) = (price(1.5), price(1.0));
        assert!(matches!((std_mp, crit_mp), (Ok(a), Ok(b)) if a > b), "{std_mp:?} {crit_mp:?}");
        // follow_cost_scale installs a changed factor (its value is the shared one, read back at once is racy: not asserted)
        assert!(follow_cost_scale(0.0).is_none(), "no factor: nothing");
        let mut p = ProgData::sheet();
        for (k, v) in prog_pairs(r#"{"columns":["id","ap","level","priority","value"],"rows":[["ability.scan",2,3,4,0],["aggro.low_profile",0,0,0,0.5]]}"#) {
            assert!(p.set(&k, v), "{k}");
        }
        assert_eq!((p.row("ability.scan").map(|r| (r.ap, r.level, r.priority)), p.value("aggro.low_profile")), (Some((2, 3, 4)), 0.5));
    }

    #[test]
    fn every_boss_row_gives_donald_his_step_in_any_order_joined_or_not() {
        let t = table();
        assert!(t.problems.is_empty(), "{:?}", t.problems);
        for d in &t.don {
            let (steps, bad) = steps_of(&d.steps);
            assert!(bad.is_empty() && !steps.is_empty(), "don.{}: {bad:?}", d.boss);
        }
        // every boss beaten, in the table's order then backwards: every line at its top, every Banish level. (the user,
        // 2026-10-22: -za in the DLC) Forward the DLC boss comes last (no MP step); backward it comes first and gives every
        // missing tier, so each base boss after it gives Max MP +3 (Fire / Water: 2 base bosses, the others 3; 16 in all).
        // (DONLINE2) line 2 from its own three bosses per element (Light: the Tree Sentinel + two): exactly Lv 3, no MP step
        for reverse in [false, true] {
            let mut r = rt(true);
            let mut rows = t.don.clone();
            if reverse {
                rows.reverse();
            }
            let mut records: Vec<String> = Vec::new();
            for row in &rows {
                records.push(format!("boss:{}", row.boss));
                let got = r.don_claim_due(&t.don, &mut records);
                assert_eq!(got.len(), 1, "{}: once", row.boss);
            }
            let l = r.don_learned();
            assert_eq!(l.tier, [4; 6], "reverse {reverse}");
            assert_eq!(l.line2, [3; 6]);
            assert_eq!(l.arcane, [2, 3, 3, 1, 1]);
            assert_eq!(l.mp_steps, if reverse { 16 } else { 0 });
            assert!(r.don_claim_due(&t.don, &mut records).is_empty(), "nothing twice");
        }
        // the start: Fire and Water at tier 1; the Tree Sentinel first gives Banish Lv 1 and Light Attunement
        let mut r = rt(true);
        let mut records = vec!["boss:tree_sentinel".to_string()];
        let got = r.don_claim_due(&t.don, &mut records);
        assert_eq!(got, vec![("tree_sentinel".to_string(), vec![Grant::Line2(Element::Light, 1), Grant::Attunement(Element::Light)])]);
        assert!(records.contains(&"don:tree_sentinel".to_string()));
        // no base-game boss gives -za: three Thunder bosses stop at Thundaga, the DLC boss (Bayle) gives Thundaza + its ultimate
        let mut th = rt(true);
        let mut rec: Vec<String> = ["boss:margit", "boss:draconic_tree_sentinel", "boss:fortissax"].map(String::from).to_vec();
        th.don_claim_due(&table().don, &mut rec);
        assert_eq!(th.don_learned().tier(Element::Thunder), 3);
        rec.push("boss:bayle".into());
        let got = th.don_claim_due(&table().don, &mut rec);
        assert_eq!(got[0].1, vec![Grant::Tier(Element::Thunder, 4), Grant::Ultimate(Element::Thunder)]);
        // Margit: Thunder 1 (+ its Attunement; DONLINE2: no Ball Lightning); Messmer: Zettaflare
        records.push("boss:margit".into());
        records.push("boss:messmer".into());
        let got = r.don_claim_due(&t.don, &mut records);
        assert_eq!(got[0].1, vec![Grant::Tier(Element::Thunder, 1), Grant::Attunement(Element::Thunder)]);
        assert_eq!(got[1].1, vec![Grant::Arcane(ArcaneLine::Zettaflare, 1)]);
        // a step before Donald joins reaches him at his join
        r.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let id = r.id(PartyChara::Donald).unwrap();
        assert_eq!(r.party.status(id).unwrap().donald.as_ref().unwrap().learned.tier(Element::Thunder), 1);
    }

    #[test]
    fn an_old_save_catches_up_once_from_its_boss_records() {
        let t = table();
        // an older save: no donald_magic in the party record, Sora's records name bosses beaten before the overhaul
        let mut r = rt(true);
        r.load_party_record(&json!({ "format": 2, "leader": "sora" }));
        assert!(r.don_unrecorded && r.don_learned() == DonLearned::start());
        let old: Vec<String> = ["boss:margit", "boss:godrick", "boss:rykard", "boss:smarag", "small:1@2", "xs:smarag"].map(String::from).to_vec();
        let mut records = old.clone();
        let got = r.don_claim_due(&t.don, &mut records);
        assert_eq!(got.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["margit", "godrick", "smarag", "rykard"]);
        assert!(!r.don_unrecorded);
        let l = r.don_learned();
        assert_eq!((l.tier(Element::Thunder), l.tier(Element::Light), l.tier(Element::Fire), l.arcane(ArcaneLine::Comet)), (1, 1, 2, 1));
        // saved and loaded again (the new key now): nothing more from the same records
        let record = r.party_record();
        let mut back = rt(true);
        back.load_party_record(&record);
        assert!(!back.don_unrecorded);
        assert!(back.don_claim_due(&t.don, &mut records).is_empty());
        assert_eq!(back.don_learned(), l);
        // the old save's records are only added to (don:<id>), never changed
        assert!(records.starts_with(&old));
        assert_eq!(records.len(), old.len() + 4);
    }

    /// (DONLINE2, the user 2026-10-27: "Tier 2 magic should be given by different bosses") THE NEW LINE-2 ROWS AND AN EXISTING
    /// SAVE: a save whose Donald already had his steps (don: records, Flare from the old start and the old tier rule) gets
    /// each new line-2 row of a boss its Sora has beaten once, one level more (Max MP +3 at the top), through the same
    /// catch-up; it is not stripped of anything.
    #[test]
    fn an_existing_save_gets_the_new_line_2_rows_of_its_beaten_bosses_once() {
        let t = table();
        let mut r = rt(true);
        // an older Donald: Fire tier 3 with Flare Lv 3 (start + Rykard + Fire Giant under the old rule), Water tier 1 + Flood Lv 1
        r.load_party_record(&json!({ "format": 2, "leader": "sora", "donald_magic": { "v": 1, "tier": [3, 0, 0, 1, 0, 0], "line2": [3, 0, 0, 1, 0, 0], "arcane": [0, 1, 0, 0, 0], "mp_steps": 0 } }));
        assert!(!r.don_unrecorded);
        let mut records: Vec<String> = ["boss:rykard", "don:rykard", "boss:fire_giant", "don:fire_giant", "boss:agheel", "boss:leonine", "boss:tree_sentinel", "don:tree_sentinel"].map(String::from).to_vec();
        let got = r.don_claim_due(&t.don, &mut records);
        assert_eq!(got, vec![("agheel".to_string(), vec![Grant::MaxMp(3)]), ("leonine".to_string(), vec![Grant::Line2(Element::Water, 2)])]);
        let l = r.don_learned();
        assert_eq!((l.tier(Element::Fire), l.line2(Element::Fire), l.line2(Element::Water), l.mp_steps), (3, 3, 2, 1));
        assert!(r.don_claim_due(&t.don, &mut records).is_empty(), "once");
        // a new game: the line-2 bosses give Flare / Flood from Lv 1
        let mut n = rt(true);
        n.load_party_record(&json!({ "format": 2, "leader": "sora" }));
        let mut rec: Vec<String> = ["boss:agheel", "boss:mimic_tear", "boss:leonine"].map(String::from).to_vec();
        let got = n.don_claim_due(&t.don, &mut rec);
        assert_eq!(got, vec![("agheel".to_string(), vec![Grant::Line2(Element::Fire, 1)]), ("leonine".to_string(), vec![Grant::Line2(Element::Water, 1)]), ("mimic_tear".to_string(), vec![Grant::Line2(Element::Blizzard, 1), Grant::Attunement(Element::Blizzard)])]);
    }

    #[test]
    fn the_x3d_kit_of_an_old_save_becomes_the_overhauls_at_the_next_step() {
        let mut r = rt(true);
        r.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let id = r.id(PartyChara::Donald).unwrap();
        let s = r.party.status_mut(id).unwrap();
        s.kit.apply_record("KIT_AERO_FLOAT", 1, 1);
        s.kit.apply_record("KIT_QUICK_CHANT", 1, 1);
        r.sync_donald();
        let s = r.party.status(id).unwrap();
        assert!(s.donald.as_ref().unwrap().abilities.owned(DonAbility::AeroFloat));
        assert_eq!(s.kit, Default::default());
        // the reward tokens: don: and the old kit: alike
        assert_eq!(crate::FriendReward::parse("don:AERO_FLOAT"), Some(crate::FriendReward::Don(DonAbility::AeroFloat)));
        assert_eq!(crate::FriendReward::parse("kit:AERO_FLOAT"), Some(crate::FriendReward::Don(DonAbility::AeroFloat)));
        assert!(matches!(crate::FriendReward::parse("kit:TORNADO_COPTER"), Some(crate::FriendReward::Kit(_))), "Goofy's stay");
        assert_eq!(level_abilities(PartyChara::Donald, 9, 10), vec!["Low Profile".to_string()]);
        assert!(level_abilities(PartyChara::Goofy, 1, 99).is_empty());
    }

    /// An older save that lists his KH3 spell abilities (`["DONALD_FIRE", 1, 1]` ...) loads cleanly: they are dropped at
    /// the next step, his AP is the same, the next record no longer lists them; his tree's requests still pass the gate.
    #[test]
    fn an_old_saves_kh3_spell_abilities_are_dropped_once() {
        let mut r = rt(true);
        r.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let mut record = r.party_record();
        let friends = record["friends"].as_array_mut().unwrap();
        let don = friends.iter_mut().find(|f| f["chara"].as_str().is_some_and(|c| c.eq_ignore_ascii_case("donald"))).unwrap();
        let abilities = don["abilities"].as_array_mut().unwrap();
        for n in ["DONALD_FIRE", "DONALD_CURE"] {
            abilities.push(json!([n, 1, 1]));
        }
        let mut back = rt(true);
        back.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let id = back.id(PartyChara::Donald).unwrap();
        let ap = back.party.status(id).unwrap().ap_in_use();
        back.reset_friends_for_load(0);
        back.load_party_record(&record);
        back.sync_donald();
        let s = back.party.status(id).unwrap();
        let fire = kh_core::enums::ETresAbilityKind::from_game_name("DONALD_FIRE").unwrap();
        assert_eq!((s.abilities.owned(fire), s.ap_in_use()), (0, ap));
        assert_eq!(s.don_tree_gate("DONALD_FIRE"), Some(true), "the tree's requests pass on his state");
        let again = back.party_record().to_string();
        assert!(!again.contains("DONALD_FIRE") && !again.contains("DONALD_CURE"), "{again}");
    }

    #[test]
    fn the_test_level_shows_every_spell_and_the_real_progression_the_rosters() {
        let mut t = rt(false);
        t.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let id = t.id(PartyChara::Donald).unwrap();
        assert_eq!(t.party.status(id).unwrap().donald.as_ref().unwrap().learned, DonLearned::all());
        let mut p = rt(true);
        p.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let id = p.id(PartyChara::Donald).unwrap();
        assert_eq!(p.party.status(id).unwrap().donald.as_ref().unwrap().learned, DonLearned::start());
    }

    /// AuraTargets wraps the host's targets for the party's whole step: a trait method it did not forward would fall back to
    /// the trait's default for every friend (a silent change of behaviour). The sources are compared by name.
    #[test]
    fn aura_targets_forward_every_method() {
        let names = |text: &str, start: &str| -> std::collections::BTreeSet<String> {
            let body = &text[text.find(start).expect(start)..];
            let body = &body[..body.find("
}").unwrap()];
            body.lines().filter_map(|l| l.trim_start().strip_prefix("fn ")).map(|l| l.split(|c: char| c == '(' || c == '<').next().unwrap().to_string()).collect()
        };
        let core = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../kh-core/src/host/targets.rs")).unwrap();
        let mine = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/donald.rs")).unwrap();
        assert_eq!(names(&mine, "impl kh_core::Targets for AuraTargets"), names(&core, "pub trait Targets"));
    }

    #[test]
    fn the_aura_hit_raises_only_hits_inside_a_fire_aura() {
        let mut d = kh_party::donald::DonaldState::new();
        d.core.attune.slots[0] = Some(kh_party::donald::Slot { element: Element::Fire, res: 0.0, period: 1, ult_used: false, full_at: None });
        let mut aura = kh_party::donald::aura::PartyAura::default();
        aura.make(&[(kh_party::MemberId(0), kh_core::Vec3::ZERO, &d)], 2);
        let mut hit = kh_core::HitRecord { damage: 10.0, ..kh_core::HitRecord::empty(kh_core::TargetId(1)) };
        assert_eq!(aura_hit(&aura, &hit).damage, 10.0, "no attacker known");
        hit.attacker = kh_core::host::HitAttacker { position: kh_core::Vec3::new(500.0, 0.0, 0.0), yaw: 0.0, radius: 40.0, valid: true };
        assert!((aura_hit(&aura, &hit).damage - 11.0).abs() < 1e-4);
        hit.attacker.position.x = 5000.0;
        assert_eq!(aura_hit(&aura, &hit).damage, 10.0, "out of reach");
    }
}
