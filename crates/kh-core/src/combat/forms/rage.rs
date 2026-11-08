//! (weapons F-RAGE) RAGE FORM (`form.AN`, CHANGE_AN 0x8B, wear form 7 "Anti"): its form package (documentation/
//! KH3_WEAPONS_PLAN.md 6e; build/sora_tmp/weapons/F-RAGE/STATE.md). KH3's own formchange, tied to no Keyblade: the gauge
//! never offers it, a LOSS OF HP does. The data is `sheets/port/sora-form-rage.json` (tools/sora_form_rage.js): every
//! number below that is not an address comes from it.
//!
//! WHAT KH3 DOES (decompiled C in re/decomp; addresses in the sheet and the files):
//! - LEARNED: never: `MyAntiFormGenerate` (pawn+0xCBC) is TRUE in the ATresPlayerPawnSora constructor (0x1413E7580) and
//!   nothing else writes it. What gates it is pawn vt+0x1FE0 (0x141436580): the save's style-change bit (the formchanges
//!   as a whole: BP_SetAbilityStyleChange of the Olympus game flow) and the scripts' BP_SetRestrictAntiForm. The mod has
//!   the formchanges from the start, so Rage Form is there from the start too (no story point to map).
//! - THE OFFER (0x14140CE00, the HP setter's tail): on every loss of HP to 25 % of MaxHP or less (vt+0x2538), outside
//!   Rage Form, alive, not swimming, not dying, no cooldown, not in one of the excluded states: a roll of
//!   `RoundToInt(f x RoundToInt((MaxHP x 35 - HP x 100) / MaxHP) / 100)` %, f = 100 for the first Rage of the battle, 30
//!   for the second, 0 after (x 0.5 in Critical Mode); a hit puts the situation command CHANGE_AN in the slot.
//! - THE COUNT (pawn+0xE5C) goes up when the form begins (StyleChange 0x1413CE760 case 0x8B, which also sets HP to MaxHP
//!   and the Riskcharges to 0) and back to 0 when the battle ends (SetBattleMode(false) 0x141446CE0). After the form a
//!   COOLDOWN of 90 s (EndStyle 0x14141CA20, pawn+0xC74 -> pawn+0xE40) blocks the offer.
//! - IN THE FORM: the attack class COMBO_ANTI (id 86, `states/combo_anti.rs`; Decide 0x141360000, kinds 0xBF .. 0xC5,
//!   MaxCombo 5, one finisher), the replacement anim set p_ex001_AnimSet13 (`replaced_clip`), no Keyblade switch (P2:
//!   `can_weapon_change`), the form's abilities (wear form 7: COMBO_MASTER), the aura ead_p_ant_com.
//! - A FULL GAUGE gives RISKCHARGE (0xB3) while fewer than three were used (SetStyleFinishCommand case 0x8B), then
//!   FINISH_AN (0xA5). RISKCHARGE (StartStyle case 0x46): HP = max(HP / 2, 1), the Riskcharges + 1, M_D419; the form goes
//!   on (its end event 0x14150C440 skips the form's end for 0xB3). FINISH_AN: M_D418, the form ends with it.
//! - THE END (EndStyle case 0x8B): RISKCHARGE / FINISH_AN / FINISH_AN2 deleted, the aura ended, the Riskcharges 0, the
//!   cooldown.
//!
//! Not in the core (hosts / INT; STATE.md): the replacement anim set for the states that do not play through the form
//! (locomotion, damage: `replaced_clip`), the face clip ANTI_01 / ANTI_02 of EndStyle (not in the pack). The attack rows
//! AN_* are in sora-combat-attack-data (tools/sora_combat_sheet.js, after all other rows). FINISH_AN2 (0x6D, the scripted Rage of the story) is never given.

use super::FormMoveset;
use crate::combat::form::KeybladeFormRow;
use crate::combat::situation;
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn};
use crate::notify::notify_asset_id;
use crate::pawn::Ctx;
use crate::states::{StateData, StateId};
use glam::Vec3;

// The rows (sheets/port/sora-form-rage.json): `FormRageRow`, `FORM_RAGE`, `FRG_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_rage.rs"));

/// CHANGE_AN: Rage Form.
pub const CHANGE_AN: i32 = crate::combat::form::CHANGE_AN;
/// FINISH_AN: the finish command after the third Riskcharge (M_D418).
pub const FINISH_AN: i32 = 0xA5;
/// RISKCHARGE: the finish command while fewer than three were used (M_D419).
pub const RISKCHARGE: i32 = 0xB3;
/// FINISH_AN2: Discension (only in the scripted Rage of the story: never given here).
pub const FINISH_AN2: i32 = 0x6D;
/// The XS situation command Riskcharge outside a form (abilities/xs/rage.rs) is the same kind as the form's.
const XS_RISKCHARGE: i32 = crate::abilities::xs::rage::RISKCHARGE_COMMAND;

/// The package: the form, its two finish commands (STYLEFINISH bodies: `states/combo_anti.rs`).
pub(crate) const MOVESET: FormMoveset = FormMoveset {
    name: "rage",
    forms: &["form.AN"],
    finishes: &[FINISH_AN, RISKCHARGE],
    on_start,
    on_end,
    tick,
    attack: crate::states::combo_anti::try_attack,
    finish_cmd,
    finish_begin: crate::states::combo_anti::finish_begin,
    finish_tick: crate::states::combo_anti::finish_tick,
    replace_clip,
    shot_motion, // (AUDIT-C) the Crimson Blast shots fly (TresProjAN_CrimsonBlast*: InitialSpeed 50 .. 200)
    hit_shape,   // (COMBAT-FIX) AnimSet13's KB_* windows and FINISH_AN's AN_FinishEnd
    hit_volumes, // (COMBAT-FIX) FINISH_AN's AN_Finish hull
    ..FormMoveset::NONE
};

/// The pawn's Rage Form fields (pawn+0xE40 / +0xE58 / +0xE5C and what the port watches), on `Form::rage`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RageForm {
    /// pawn+0xE5C: Rage Forms begun in this battle (the offer's factor; 0 when the battle ends).
    pub uses: u32,
    /// pawn+0xE58: Riskcharges used in this form (3 -> FINISH_AN; 0 at the form's begin and end).
    pub riskcharges: u32,
    /// pawn+0xE40: the cooldown after the form, s (> 0 blocks the offer).
    pub cooldown: f32,
    /// The HP of the last frame (the setter's old value) and the battle flag of the last frame (SetBattleMode's edge).
    pub last_hp: i32,
    pub last_battle: bool,
    /// The first form tick after the transformation has run (StyleChange 0x1413CE760 case 0x8B).
    pub entered: bool,
    /// The aura of the form (group 100) and of the Riskcharges (groups 10 .. 9 + n and 13; F-POLISH, traced).
    aura: [Option<EffectHandle>; 6],
    /// (AUDIT-C) the eyes (group 13) apart from the charges: Rage's AirSlide ends and restarts them alone.
    eyes: [Option<EffectHandle>; 3],
    /// For tests and the debug text: rolls made, offers given.
    pub rolls: u32,
    pub offers: u32,
    /// (AUDIT-C) The flying shots of COMBO_ANTI (key, origin, velocity) and the next key (`shot`).
    moving: [Option<(u16, Vec3, Vec3)>; 8],
    next_key: u8,
}

// ------------------------------------------------------------------------------------------------ the data

fn rows(what: &'static str) -> impl Iterator<Item = &'static FormRageRow> {
    FORM_RAGE.iter().filter(move |r| r.what == what)
}

/// A rule's numbers (`rule.<key>` values).
pub(crate) fn rule(key: &str) -> &'static [f32] {
    rows("rule").find(|r| r.logical == key).map_or(&[], |r| r.values)
}

fn rule_at(key: &str, i: usize, default: f32) -> f32 {
    rule(key).get(i).copied().unwrap_or(default)
}

/// The row of an attack kind of COMBO_ANTI (`attack.<kind>`).
pub fn attack_row(kind: u8) -> Option<&'static FormRageRow> {
    rows("attack").find(|r| r.code == i32::from(kind))
}

/// The row of a finish command's body (`finish.<name>`).
pub fn finish_row(cmd: i32) -> Option<&'static FormRageRow> {
    rows("finish").find(|r| r.code == cmd)
}

/// M_D414's return clip (`chain.M_D421`).
pub(crate) fn chain_row() -> Option<&'static FormRageRow> {
    rows("chain").next()
}

/// m_ReplaceAnimSets: the clip (animation-table row) a logical name plays as in Rage Form (p_ex001_AnimSet13); None = the
/// base set's row. For the pose hosts too (locomotion, damage: STATE.md).
pub fn replaced_clip(cmd: i32, logical: &str) -> Option<&'static str> {
    if cmd != CHANGE_AN {
        return None;
    }
    super::replaced_clip(cmd, logical) // (F-LOCO) the one contract: AnimSet13 through the form's chain (RunLoopB13 too)
}

fn replace_clip(logical: &str) -> Option<&'static str> {
    replaced_clip(CHANGE_AN, logical)
}

/// MaxCombo (pawn slot 928 0x141433570) and Finishers (slot 1253 0x141433A60) in form 0x8B.
pub(crate) fn combo_limits() -> (i32, i32) {
    (rule_at("combo", 0, 5.0) as i32, rule_at("combo", 1, 1.0) as i32)
}

/// (F-RAGE fix) EVERY EFFECT RAGE FORM SPAWNS: the aura rows (ead_p_ant_com), the effects its clips' notifies name (the
/// sheet's `effects`: `time@group:asset`) and FINISH_AN's AN_FINISH particle. For the effect hosts' preload (kh-host
/// vfx.rs `load`): their materials are made with the pack, so the aura shows from its first frame.
pub fn effect_names() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    let clips = FORM_RAGE.iter().filter(|r| matches!(r.what, "attack" | "chain" | "finish")).flat_map(|r| r.effects.iter()).filter_map(|e| e.split_once(':').map(|(_, asset)| asset));
    for name in rows("aura").map(|r| r.logical).chain(clips).chain(std::iter::once(crate::states::combo_anti::FINISH_BLAST_PARTICLE)) {
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// The attack rows Rage Form hits with (sora-combat-attack-data carries them).
pub fn attack_rows() -> impl Iterator<Item = &'static str> {
    rows("row").map(|r| r.logical)
}

// ------------------------------------------------------------------------------------------------ the offer

/// UE's `RoundToInt` (floor(x + 0.5)): the `-((int)ROUND(-0.5 - 2x) >> 1)` idiom of 0x14140CE00.
fn round_to_int(x: f32) -> i32 {
    (x + 0.5).floor() as i32
}

/// The offer's chance in % (0x14140CE00): `RoundToInt(f x RoundToInt((MaxHP x 35 - HP x 100) / MaxHP) x 0.01)`, f by the
/// Rage count (100, 30, then 0), x 0.5 in Critical Mode (`CriticalModeAntiFormGenerateRate`); RageFever (a Re Mind code)
/// forces 1000.
pub fn offer_chance(hp: i32, max_hp: i32, uses: u32, critical: bool, fever: bool) -> i32 {
    if max_hp <= 0 {
        return 0;
    }
    let base = rule_at("offer_base", 0, 35.0);
    let share = (max_hp as f32 * base - hp as f32 * 100.0) / max_hp as f32;
    let factor = if uses == 0 { rule_at("offer_factor", 0, 100.0) } else { rule_at("offer_factor", 1, 30.0) };
    let mut f = factor as i32;
    if critical {
        f = (f as f32 * rule_at("offer_critical_rate", 0, 0.5)) as i32;
    }
    if uses >= 2 {
        f = rule_at("offer_factor", 2, 0.0) as i32;
    }
    if fever {
        f = rule_at("offer_fever", 0, 1000.0) as i32;
    }
    round_to_int((f * round_to_int(share)) as f32 * 0.01)
}

/// pawn vt+0x2538 (0x1405AD600): HP at or below 25 % of MaxHP (integer: `HP <= MaxHP * 25 / 100`).
pub fn low_hp(hp: i32, max_hp: i32) -> bool {
    hp <= max_hp * (rule_at("offer_hp_share", 0, 0.25) * 100.0).round() as i32 / 100
}

/// A uniform 0 .. 99 from the pawn's stream (KH3: `rand() % 100`).
fn roll100(cx: &mut Ctx) -> i32 {
    let rng = &mut cx.pawn.combat.rng;
    if *rng == 0 {
        *rng = 0x9E37_79B9;
    }
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    ((*rng >> 8) % 100) as i32
}

/// The states the offer is not made in (the switch of 0x14140CE00).
fn excluded_state(id: StateId) -> bool {
    rule("excluded_states").iter().any(|v| *v as i32 == id.value())
}

/// EVERY FRAME, form or not (combat/style.rs `tick`, first line: the HP setter's tail, the pawn tick's cooldown and
/// SetBattleMode's reset, which KH3 runs outside any form too).
pub(crate) fn watch(state: &StateData, cx: &mut Ctx, dt: f32) {
    let battle = cx.pawn.actions.battle;
    let hp = cx.pawn.status.hp;
    let r = &mut cx.pawn.combat.form.rage;
    // the pawn tick 0x141449CD0: pawn+0xE40 counts down.
    if r.cooldown > 0.0 {
        r.cooldown = (r.cooldown - dt).max(0.0);
    }
    // SetBattleMode(false) 0x141446CE0: vt+0x27D8(0): the count of the battle is over.
    if r.last_battle && !battle {
        r.uses = 0;
    }
    r.last_battle = battle;
    let old = std::mem::replace(&mut r.last_hp, hp);
    if hp < old {
        on_hp_set(state, cx, old, hp);
    }
}

/// The HP setter's tail (0x14140CE00) for a change `old` -> `new`.
fn on_hp_set(state: &StateData, cx: &mut Ctx, old: i32, new: i32) {
    let s = &*cx.pawn;
    let f = &s.combat.form;
    // the held form (ctrl vt+0x1A70) or the scripted one (ctrl+0x888) is Rage Form: nothing.
    if f.cmd == CHANGE_AN {
        return;
    }
    // !vt+0x1FE0 (the formchanges are on in the mod; no script restricts Rage Form), !vt+0x2540 (dying), new HP > 0,
    // !vt+0x1E40 (the cooldown), the state, MyAntiFormGenerate (Sora: true), the loss, vt+0x2538 (25 %).
    let dying = state.id() == StateId::Die || s.queue.queued_id() == Some(StateId::Die);
    if dying || new <= 0 || f.rage.cooldown > 0.0 || excluded_state(state.id()) || new >= old || !low_hp(new, s.status.max_hp()) {
        return;
    }
    let chance = offer_chance(new, s.status.max_hp(), f.rage.uses, s.status.game_level.is_critical(), false);
    cx.pawn.combat.form.rage.rolls += 1;
    if roll100(cx) < chance {
        // controller vt+0x1578(0x8B, -1.0, 0) = NotifySetActionCommand: the situation command.
        situation::enqueue(cx, CHANGE_AN, -1.0);
        cx.pawn.combat.form.rage.offers += 1;
    }
}

// ------------------------------------------------------------------------------------------------ the aura

/// The aura rows of an effect group (ead_p_ant_com).
fn aura_rows(group: i32) -> impl Iterator<Item = &'static FormRageRow> {
    rows("aura").filter(move |r| r.code == group)
}

fn spawn_group(cx: &mut Ctx, group: i32, into: &mut [Option<EffectHandle>; 6]) {
    for (slot, row) in into.iter_mut().zip(aura_rows(group)) {
        let bone = (row.clip != "None").then_some(row.clip);
        let name = row.logical;
        // (F-RAGE fix) the row's own LocationOffset and ScaleOffset (the eyes' group 13: -10 / +4 cm, x 0.5 .. 0.65); its
        // ParticleColor / ParticleAlpha (values 4..7): (AUDIT-C) through `Effects::color`, as KH3's post-spawn 0x141772CD0
        // writes them to the component (+0xB78 / +0xB84; docs/sora-vfx-spec.md 4.3): the eyes' rows 1 / 2 are grey 0.65 / 0.4.
        let v = |i: usize, d: f32| row.values.get(i).copied().unwrap_or(d);
        let offset = Vec3::new(v(0, 0.0), v(1, 0.0), v(2, 0.0));
        let h = cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(name)), name, at: Attach::Sora { bone, offset }, rotation: Vec3::ZERO, scale: v(3, 1.0), persistent: true });
        let rgba = [v(4, 1.0), v(5, 1.0), v(6, 1.0), v(7, 1.0)];
        if rgba != [1.0; 4] {
            cx.host.effects.color(h, rgba); // (AUDIT-C)
        }
        *slot = Some(h);
    }
}

fn stop_all(cx: &mut Ctx, handles: [Option<EffectHandle>; 6]) {
    for h in handles.into_iter().flatten() {
        cx.stop_effect(h, false);
    }
}

/// The form's aura (F-POLISH, traced): group 100 only (vp_ant_bdy_ora00 on the hands and feet), FUN_1405B9370(pawn+0x440,
/// m_AntiFormEffect, 100, 0) in StyleChange Begin 0x1413CBA70 (0x1413CBD38), its tick 0x1413D3AC0 (0x1413D3DFB) and
/// 0x1413CE760 (0x1413CE9B9). Group 101 is passed by no code (every m_AntiFormEffect load of 0x1405 .. 0x1415 read).
fn aura_on(cx: &mut Ctx) {
    let mut handles = [None; 6];
    spawn_group(cx, 100, &mut handles);
    cx.pawn.combat.form.rage.aura = handles;
}

/// (AUDIT-C) A Riskcharge's lasting effects, from KH3's data: its charge (vp_ant_rsk_crg_00 / 01 at vfx_center) is M_D419's
/// OWN notify rows of effect group n + 9 (RISKCHARGE 0x1413E2A10 plays M_D419 with that group: combo_anti `riskcharge`), so
/// the core spawns no second copy; the eyes (group 13) start in StyleFinish End 0x1413CEBD0 at 0x1413CF044 when n == 1. The
/// restore ctrl vt+0x1B00 0x14140E7F0 (groups 10 .. 9 + n and 13 again) runs only from the form resume 0x14141DD00 (case
/// 0x8B, with RailSlide's), never at a Riskcharge (F-POLISH's note had it at the Riskcharge: the cause of the two live copies
/// ANLOOK saw). Groups 21 .. 23 / 31 .. 33 are passed by no code and named by no M_D419 notify (sheet attack.0xBE effects).
pub(crate) fn riskcharge_ended(cx: &mut Ctx) {
    if cx.pawn.combat.form.rage.riskcharges == 1 {
        eyes_on(cx);
    }
}

// ------------------------------------------------------------------------------------------------ (AUDIT-C) the guard

/// The reaction command of Rage Form's back slide (AirSlide Begin 0x141332930: +0xAC = 0xD) and its attack kind (+0xB0 =
/// 0xC6, M_D420 Rage Raid: PlayAttack 0x1413378B0 case 0xC6).
pub const RAGE_RAID_COMMAND: i32 = 0xD;
pub const RAGE_RAID_KIND: u8 = 0xC6;

/// OnGuardedHit (interface slot 142 0x14143CAF0) case 0x8B: a guarded hit in Rage Form is no Guard hit entry: the queue is
/// cleared (FUN_1405A8010) and an AIR_SLIDE is queued (FUN_1405B4F80(0x14)) with the attacker as its target pawn (+0x98;
/// a weapon's owner, param_4+0x588) and flag 0x10, its yaw his root yaw - 180 (a back slide away from it). Its Begin then
/// arms Rage Raid (flag 0x100). Returns pawn vt+0x14E0(0x4A) (HasAbility 0x4A) as KH3 does (the caller's guard result).
pub(crate) fn guarded(cx: &mut Ctx, attacker: Option<Vec3>) -> bool {
    let yaw = cx.pawn.yaw;
    cx.pawn.queue.clear();
    cx.pawn.queue.push_state(crate::states::air_slide::AirSlide::rage_back(yaw, attacker));
    crate::actions::has(cx.pawn, crate::enums::ETresAbilityKind::GuardRegene)
}

/// AirSlide Begin 0x141332930 in Rage Form: pawn+0x440 vt+0x1A8(100, 1) and (0xD, 1) (0x1405A85F0 -> FUN_141765520 with
/// "only this group"): the aura and the eyes end for the slide.
pub(crate) fn slide_hide(cx: &mut Ctx) {
    let r = cx.pawn.combat.form.rage;
    stop_all(cx, r.aura);
    stop_all(cx, [r.eyes[0], r.eyes[1], r.eyes[2], None, None, None]);
    let r = &mut cx.pawn.combat.form.rage;
    (r.aura, r.eyes) = ([None; 6], [None; 3]);
}

/// (AUDIT-C) Group 13 (the eyes) into its own handles.
fn eyes_on(cx: &mut Ctx) {
    let mut one = [None; 6];
    spawn_group(cx, 13, &mut one);
    let r = &mut cx.pawn.combat.form.rage;
    for (slot, h) in r.eyes.iter_mut().zip(one) {
        *slot = h;
    }
}

/// AirSlide End 0x1413347F0 in Rage Form: FUN_1405B9370(pawn+0x440, m_AntiFormEffect, 100, 0), and group 0xD while the
/// Riskcharges (pawn+0x460 vt+0xA8) are not 0.
pub(crate) fn slide_restore(cx: &mut Ctx) {
    aura_on(cx);
    if cx.pawn.combat.form.rage.riskcharges > 0 {
        eyes_on(cx);
    }
}

// ------------------------------------------------------------------------------------------------ (AUDIT-C) the warp's aura

/// COMBO_ANTI's +0x22C (Decide 0x141360000: 0x11 with the warp): ead_pc_aura_com0 group 17 at the vanish, + 1 (18) at the
/// reappearance.
pub(crate) const WARP_AURA_GROUP: i32 = 0x11;

/// The held Keyblade's socket as a weapon anchor (`kb:<model>:R_wep_skt:<socket>`, kh-host's kbweapon route), interned: a row
/// on SQEX_ATTACH_OBJECT_ATTACH_TO_RIGHT_HAND_WEAPON is on whatever Keyblade is in his hand (Rage Form keeps it).
fn hand_anchor(model: &'static str, socket: &str) -> &'static str {
    static NAMES: std::sync::Mutex<Vec<(String, &'static str)>> = std::sync::Mutex::new(Vec::new());
    let key = format!("kb:{model}:R_wep_skt:{socket}");
    let mut names = NAMES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, s)) = names.iter().find(|(k, _)| *k == key) {
        return s;
    }
    let leaked: &'static str = Box::leak(key.clone().into_boxed_str());
    names.push((key, leaked));
    leaked
}

/// vt+0x1A0(m_CmnAuraEffect, group, 0): every row of ead_pc_aura_com0's `group` (sheet `aura.<group>.wrp.*`) once, at his
/// socket (`:free` = ToAttach false, `:abs` = absolute rotation) or on the held Keyblade's (`kbhand:<socket>`).
pub(crate) fn warp_aura(cx: &mut Ctx, group: i32) {
    for row in aura_rows(group).filter(|r| r.id.contains(".wrp.")) {
        let bone = match row.clip.strip_prefix("kbhand:") {
            Some(socket) => hand_anchor(cx.pawn.status.attack_model, socket),
            None => row.clip,
        };
        let v = |i: usize| row.values.get(i).copied().unwrap_or(0.0);
        cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(row.logical)), name: row.logical, at: Attach::Sora { bone: Some(bone), offset: Vec3::new(v(0), v(1), v(2)) }, rotation: Vec3::ZERO, scale: 1.0, persistent: false });
    }
}

// ------------------------------------------------------------------------------------------------ (AUDIT-C) the shots

/// The window keys of Rage Form's shots (FORM_SHOT_SEQUENCE, 0x4E00 | n): lightdark has 0x4C00.., nano 0x8000..
const SHOT_KEY: u16 = 0x4E00;

/// (AUDIT-C) COMBO_ANTI'S SHOT HANDLER (anim interface slot 69 = 0x141357AF0; COMBO_ANTI's vtable 0x14690B350, its +0x40
/// sub-vtable 0x14690B7C8 read from the exe: F-RAGE's "no spawner" looked at the Attack class's 0x141344580 only) for one
/// TresAnimNotify_Shot of kind `kind` (LocationIndex `index`). By the attack kind (pawn vt+0x1DC8):
/// - 0x37: the Attack's handler (false: attack.rs's).
/// - 0xBE "AN_RISKCHARGE": p_ex001_ProjectileSet has no such row: pawn vt+0x1450 spawns nothing.
/// - 0xBF (Crimson Upper, M_D411's three Shot notifies, LocationIndex 1 .. 3): only while the Riskcharges (pawn+0x460
///   vt+0xA8) >= LocationIndex: "AN_UPPER" at his root (+0x2B0) + forward x LocationIndex x 100.
/// - 0xC5 (Crimson Blast): "AN_CRIMSONBLAST1" / "2" / "3" by the Riskcharges, "AN_CRIMSONBLAST" with none, at the state's
///   slot 134 (0x141336940: his root at the lock-on height, vt+0x6E0) + forward x 100.
/// - any other kind: nothing.
///
/// Forward is his root's rotation (+0x2E0) as a vector (FUN_142D93270). The actor is the projectile set's class
/// (sheets/port/sora-combat-shots.json, tools/sora_combat_shots_sheet.js): its shapes, life, particle and speed.
pub(crate) fn shot(cx: &mut Ctx, kind: u8, index: i32) -> bool {
    let n = cx.pawn.combat.form.rage.riskcharges as i32;
    let a = cx.pawn.yaw.to_radians();
    let forward = Vec3::new(a.cos(), a.sin(), 0.0);
    let (name, at) = match kind {
        0x37 => return false,
        0xBF if n >= index => ("AN_UPPER", cx.pawn.cmc.location + forward * (index as f32 * 100.0)),
        0xC5 => {
            let name = match n {
                1 => "AN_CRIMSONBLAST1",
                2 => "AN_CRIMSONBLAST2",
                3 => "AN_CRIMSONBLAST3",
                _ => "AN_CRIMSONBLAST",
            };
            let centre = cx.pawn.cmc.location + Vec3::Z * cx.pawn.params().capsule_center_z;
            (name, centre + forward * 100.0)
        }
        _ => return true,
    };
    spawn_shot(cx, name, at)
}

/// (AUDIT-C) pawn vt+0x1450(name, location, ...): the projectile set's actor `name` at `at` with his yaw: every attack
/// shape of its TresAtkColl0 (the sheet's rows: SPHERE / CAPSULE1 `m_Size`, `m_IncSize`, `m_IncMaxSize`,
/// `m_RelativeLocation1` in the actor's frame, the default row, m_MainLife), its particle at its root, flying forward at
/// its InitialSpeed. False = no such actor in the sheet.
pub(crate) fn spawn_shot(cx: &mut Ctx, name: &str, at: Vec3) -> bool {
    use crate::combat::hit::{spawn_form_shot, FormShot, FORM_SHOT_SEQUENCE};
    let rows: Vec<&'static crate::combat::tables::ShotShapeRow> = crate::combat::tables::SHOT_SHAPES.iter().filter(|r| r.shot == name).collect();
    let Some(first) = rows.first().copied() else { return false };
    let yaw = cx.pawn.yaw;
    let (s, c) = yaw.to_radians().sin_cos();
    let turn = |v: &[f32]| Vec3::new(v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2]);
    if !first.particle.is_empty() {
        cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(first.particle)), name: first.particle, at: Attach::World(at + turn(first.particle_loc)), rotation: Vec3::new(0.0, yaw, 0.0), scale: 1.0, persistent: false });
    }
    let info = cx.pawn.combat.attack_ctx;
    let velocity = Vec3::new(c, s, 0.0) * first.speed;
    for row in rows {
        let r = &mut cx.pawn.combat.form.rage;
        let key = SHOT_KEY | u16::from(r.next_key);
        r.next_key = r.next_key.wrapping_add(1);
        let opt = |v: &'static str| (!v.is_empty()).then_some(v);
        let group = if row.group == "None" { row.shot } else { row.group };
        let size = (row.size[0], row.inc_size[0], if row.max_size[0] > 0.0 { row.max_size[0] } else { row.size[0] });
        spawn_form_shot(cx, FormShot { key, group, row: row.row, position: at, yaw, size, life: row.life, hit_effect: opt(row.hit_effect), hit_se: opt(row.hit_se), info, hit_list: Vec::new() });
        if let Some(shot) = cx.pawn.combat.shots.iter_mut().rev().find(|x| x.window.key == (FORM_SHOT_SEQUENCE, key)) {
            shot.offset = Vec3::new(row.loc[0], row.loc[1], row.loc[2]);
            // CAPSULE1: an upright capsule of radius m_Size.X, half length m_Size.Z (the actor is not pitched).
            if row.shape == "CAPSULE1" {
                shot.axis = Vec3::Z * row.size[2];
            }
        }
        let moving = &mut cx.pawn.combat.form.rage.moving;
        if let Some(slot) = moving.iter_mut().find(|m| m.is_none_or(|(k, _, _)| k == key)) {
            *slot = (first.speed > 0.0).then_some((key, at, velocity));
        }
    }
    true
}

/// (AUDIT-C) The package's `shot_motion`: a Crimson Blast shot flies on at its speed (UProjectileMovementComponent: velocity
/// = forward x InitialSpeed, MaxSpeed the same, no gravity) for its life.
fn shot_motion(cx: &mut Ctx, shot: &mut crate::combat::hit::Shot) {
    let (seq, key) = shot.window.key;
    if seq != crate::combat::hit::FORM_SHOT_SEQUENCE || key & 0xFF00 != SHOT_KEY {
        return;
    }
    let moving = &mut cx.pawn.combat.form.rage.moving;
    if let Some((_, origin, velocity)) = moving.iter().flatten().find(|m| m.0 == key).copied() {
        shot.position = origin + velocity * shot.window.time;
        if shot.life <= 0.0 {
            moving.iter_mut().filter(|m| m.is_some_and(|(k, _, _)| k == key)).for_each(|m| *m = None);
        }
    }
}

// ------------------------------------------------------------------------------------------------ the hooks

/// StartStyle's tail for 0x8B: the held form ended first (the framework), the XS Riskcharge offer of outside the form
/// withdrawn (the form's own RISKCHARGE is the gauge's: no double offer), the aura on.
fn on_start(cx: &mut Ctx, _row: &'static KeybladeFormRow) {
    // (F-POLISH) StartStyle 0x14141A9D0 case 0x8B: the soft-target range controller+0x970 = MyAutoLockonLimit x 2.0.
    cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT * 2.0;
    situation::delete(cx, XS_RISKCHARGE);
    let r = &mut cx.pawn.combat.form.rage;
    r.entered = false;
    aura_on(cx);
}

/// EndStyle case 0x8B: the aura ended (pawn vt+0x23C8), the Riskcharges 0 (vt+0x27D0(0)), the cooldown (vt+0x1E38(90)).
fn on_end(cx: &mut Ctx, _row: &'static KeybladeFormRow) {
    // (F-POLISH) EndStyle case 0x8B: controller slot 738 (vt+0x1710): the soft-target range back to MyAutoLockonLimit.
    cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT;
    let r = cx.pawn.combat.form.rage;
    stop_all(cx, r.aura);
    stop_all(cx, [r.eyes[0], r.eyes[1], r.eyes[2], None, None, None]); // (AUDIT-C)
    let r = &mut cx.pawn.combat.form.rage;
    (r.aura, r.eyes) = ([None; 6], [None; 3]);
    r.riskcharges = 0;
    r.cooldown = rule_at("cooldown", 0, 90.0);
    r.entered = false;
    // (AUDIT-C) EndStyle 0x14141CA20 case 0x8B (ll. 412-420, with the form's end not skipped): the face row "ANTI_01", or
    // "ANTI_02" while in battle (pawn vt+0x1740), on pawn+0x440 (FUN_1405A8AC0 then FUN_1405A8980(row, -1, 0, 1, 0)).
    let row = if cx.pawn.actions.battle { "ANTI_02" } else { "ANTI_01" };
    cx.pawn.event(crate::host::EventKind::FaceAnim { row });
}

/// The form's frame: the first one after the transformation is StyleChange's 0x8B part (0x1413CE760: the Riskcharges 0,
/// the count + 1, HP = MaxHP).
fn tick(_: &StateData, cx: &mut Ctx, _row: &'static KeybladeFormRow, _dt: f32) {
    if cx.pawn.combat.form.rage.entered {
        return;
    }
    let max = cx.pawn.status.max_hp();
    cx.pawn.status.set_hp(max);
    let r = &mut cx.pawn.combat.form.rage;
    r.entered = true;
    r.riskcharges = 0;
    r.uses += 1;
    r.last_hp = max;
}

/// SetStyleFinishCommand case 0x8B: RISKCHARGE while fewer than three were used, then FINISH_AN.
fn finish_cmd(cx: &Ctx, _row: &'static KeybladeFormRow) -> i32 {
    if (cx.pawn.combat.form.rage.riskcharges as f32) < rule_at("riskcharge_max", 0, 3.0) {
        RISKCHARGE
    } else {
        FINISH_AN
    }
}

/// StartStyle case 0x46 (0x14141A9D0), after its TryStyleFinish (the framework's finish branch, combat/form.rs): HP =
/// max(HP / 2, 1), the Riskcharges + 1. Unlike the other finish kinds the form's time and the gauge stay.
pub(crate) fn riskcharge_taken(cx: &mut Ctx) {
    let hp = (cx.pawn.status.hp / 2).max(rule_at("riskcharge_hp", 1, 1.0) as i32);
    cx.pawn.status.set_hp(hp);
    let r = &mut cx.pawn.combat.form.rage;
    r.riskcharges += 1;
    r.last_hp = hp;
}

// ------------------------------------------------------------------------------------------------ (COMBAT-FIX) the hit shapes
// THE USER, 2026-11-06: "rage form's normal attacks do no damage, fix this". AnimSet13's own clips (D411 .. D416, D418) have no
// socket track in the pack, so their KB_Stick* / KB_Sphere* windows had no shape (hit.rs `shape`: None) and never hit; the
// pawn's AN_LHAND (L_wep_skt), AN_Finish (a CONVEX hull) and AN_FinishEnd (kh_sk) had no place on any clip. Only Crimson
// Blast's shots hit. The sheet's `sock.R.<seq>` / `sock.L.<seq>` tracks (from the clips, tools/sora_form_rage.js) and `shape.*`
// rows (p_ex001_Pawn TresAtkColl0) place them as KH3 does; damage then follows the normal hit path with the AN_* rows.

/// A per-frame track row (`sock.R.<seq>` / `sock.L.<seq>`) at clip time `time`: its two points, actor space.
fn track_at(side: &str, seq: &str, time: f32) -> Option<(Vec3, Vec3)> {
    let r = rows("sock").find(|r| r.logical == side && r.clip == seq)?;
    let frames = usize::try_from(r.code).ok().filter(|n| *n > 0 && r.values.len() >= n * 6)?;
    let last = frames - 1;
    let f = if r.length > 0.0 { (time / r.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let at = |k: usize| r.values[i * 6 + k] + (r.values[j * 6 + k] - r.values[i * 6 + k]) * t;
    Some((Vec3::new(at(0), at(1), at(2)), Vec3::new(at(3), at(4), at(5))))
}

/// The body socket L_wep_skt (p_ex001_Pawn's AN_LHAND) at clip time `time` of sequence `seq`, actor space; None = no track
/// of that clip (combat/hit.rs: the pawn's own sphere on it, in or out of the form: the XS Crimson Blast's D417 too).
pub(crate) fn left_hand(seq: &str, time: f32) -> Option<Vec3> {
    track_at("L", seq, time).map(|(p, _)| p)
}

/// The `shape.<group>` row of a p_ex001_Pawn group.
fn shape_row(group: &str) -> Option<&'static FormRageRow> {
    rows("shape").find(|r| r.logical == group)
}

/// m_bDisableLocation1Attach of the pawn groups the sheet carries (AN_Finish, AN_FinishEnd: placed once, at activation).
pub(crate) fn fixed_group(cmd: i32, group: &str) -> bool {
    cmd == CHANGE_AN && shape_row(group).is_some_and(|r| r.flags.get(4) == Some(&"yes"))
}

/// Actor space -> the world (Sora's location and yaw), as hit.rs.
fn world(cx: &Ctx, v: Vec3) -> Vec3 {
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    cx.pawn.cmc.location + Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z)
}

/// The package's `hit_shape` (combat/hit.rs `shape`, before the Kingdom Key's tracks): a KB_* window of an AnimSet13 clip
/// without a pack track rides the clip's `sock.R` track (the held Keyblade's ATK_00 / ATK_01; w_so_KeybladeBase's shape:
/// CAPSULE2 ATK_00 - ATK_01 or a SPHERE on one of them); AN_FinishEnd is its sphere at kh_sk (Sora's root) + m_RelativeLocation1.
/// None = not this package's (the generic path: the pack's tracks, `left_hand`).
fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> crate::combat::forms::HitShape {
    let seq = crate::anim::SEQUENCES.get(sequence)?;
    if let Some(r) = shape_row(group).filter(|r| r.flags.first() == Some(&"SPHERE") && r.clip == "kh_sk") {
        let v = |k: usize| r.values.get(k).copied().unwrap_or(0.0);
        let p = world(cx, Vec3::new(v(1), v(2), v(3)));
        return Some((p, p, v(0)));
    }
    if !group.starts_with("KB_") || !seq.sockets.is_empty() {
        return None;
    }
    let (a, b) = track_at("R", seq.name, time)?;
    let s = crate::combat::tables::group_shapes(group).next()?;
    let (a, b) = (world(cx, a), world(cx, b));
    match (s.shape, s.socket1, s.socket2) {
        ("CAPSULE2", "ATK_00", "ATK_01") => Some((a, b, s.size_x)),
        ("SPHERE", "ATK_00", _) => Some((a, a, s.size_x)),
        ("SPHERE", "ATK_01", _) => Some((b, b, s.size_x)),
        _ => None,
    }
}

/// The package's `hit_volumes` (combat/hit.rs `sweep_volumes`): FINISH_AN's AN_Finish, `tres_colltest01` x m_Scale at kh_sk
/// (Sora's root) + m_RelativeLocation1, rotation LOCAL 0 (upright in the world).
fn hit_volumes(cx: &Ctx, group: &str, _sequence: usize, _time: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let r = shape_row(group).filter(|r| r.flags.first() == Some(&"CONVEX"))?;
    let mesh = crate::combat::hull::mesh(r.flags.get(2).copied().unwrap_or("None"))?;
    let v = |k: usize| r.values.get(k).copied().unwrap_or(0.0);
    let centre = world(cx, Vec3::new(v(1), v(2), v(3)));
    Some(vec![crate::combat::hull::Volume::Hull(crate::combat::hull::Hull { mesh, centre, rotation: glam::Quat::IDENTITY, scale: Vec3::new(v(4), v(5), v(6)) })])
}

#[cfg(test)]
mod tests {
    use super::*;

    // THE TRIGGER TABLE (0x14140CE00): MaxHP 100; the chance by HP and the count, Critical Mode, the 25 % gate.
    #[test]
    fn the_offer_chance_table_is_kh3s() {
        let c = |hp, uses, crit| offer_chance(hp, 100, uses, crit, false);
        assert_eq!([c(25, 0, false), c(20, 0, false), c(10, 0, false), c(1, 0, false)], [10, 15, 25, 34]);
        assert_eq!([c(25, 1, false), c(10, 1, false), c(1, 1, false)], [3, 8, 10], "the second Rage of a battle: f = 30");
        assert_eq!([c(10, 2, false), c(1, 5, false)], [0, 0], "none from the third");
        assert_eq!([c(10, 0, true), c(10, 1, true)], [13, 4], "Critical Mode: f x 0.5 (12.5 -> 13, 15 x 25 % = 3.75 -> 4)");
        assert_eq!(offer_chance(10, 100, 7, false, true), 250, "RageFever: f = 1000");
        // MaxHP 437: share = (437 x 35 - 60 x 100) / 437 = 21.27 -> 21; 100 x 21 / 100 = 21.
        assert_eq!(offer_chance(60, 437, 0, false, false), 21);
        assert!(low_hp(25, 100) && !low_hp(26, 100) && low_hp(109, 437) && !low_hp(110, 437), "HP <= MaxHP x 25 / 100");
    }

    #[test]
    fn the_sheet_carries_the_moveset() {
        assert_eq!(combo_limits(), (5, 1));
        assert_eq!(attack_row(0xBF).map(|r| (r.clip, r.root_len, r.air_mode)), Some(("D411", 490.0, true)));
        assert_eq!(attack_row(0xC5).map(|r| r.clip), Some("M_D417"), "Crimson Blast: the XS row of D417");
        assert_eq!((finish_row(FINISH_AN).map(|r| r.clip), finish_row(RISKCHARGE).map(|r| r.clip)), (Some("D418"), Some("M_D419")));
        assert_eq!(replaced_clip(CHANGE_AN, "IdleB"), Some("D015"));
        assert_eq!(replaced_clip(0x94, "IdleB"), None, "only in Rage Form");
        assert_eq!(rule("cooldown"), &[90.0, 9.0]);
        assert_eq!(aura_rows(100).count(), 4, "vp_ant_bdy_ora00 on both hands and feet");
        assert!(attack_rows().any(|r| r == "AN_WARPTHRUST"));
        assert!(excluded_state(StateId::BadstatStun) && !excluded_state(StateId::Idle) && !excluded_state(StateId::Damage));
    }
}
