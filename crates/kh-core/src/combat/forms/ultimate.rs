//! (weapons F-ULTIMATE) Ultimate Form (`form.SW`, CHANGE_SW 0x87, Ultima Weapon): its form package (documentation/
//! KH3_WEAPONS_PLAN.md 1.3 / 4; build/sora_tmp/weapons/F-ULTIMATE/STATE.md). A single-stage form of 30 s, wear form 6
//! ("Final", at the weapon change: the framework sets it from the row), with FINISH_SW (0xA1). The data is
//! `sheets/port/sora-form-ultimate.json` (tools/sora_form_ultimate.js): every number below that is not an address comes
//! from it.
//!
//! WHAT KH3 DOES (decompiled C in re/decomp; addresses here, in the sheet and in states/combo_sw.rs):
//! - THE ATTACK CLASS (pawn slot 1337 0x141432F00: 0x87 -> 0x52): TresPlayerState_ComboSW (id 82, `states/combo_sw.rs`), a
//!   subclass of the Attack state that overrides only Decide (slot 122 0x141363810, [`decide`]): the kinds 0xC7 .. 0xCC in
//!   a fixed chain (the floating blades' flurry M_9411, the stabs M_9412, the raid M_9413, the dive M_9416, the finishers
//!   M_9417 / M_9418), MaxCombo 6, Finishers 3 (0x141433570 / 0x141433A60). Its TELEPORT: a step toward a target farther
//!   than 400 cm within the soft-target range (or high, or below him in the air) vanishes (JumpNormalStart, invincible),
//!   reappears 150 cm short of the target 0.2 s later (the chase warp, slot 117 0x14134AA80 -> slot 136) and stays
//!   invincible 0.5 s more (+0x25C, Attack Tick 0x141341340). The air attacks are the same kinds in air mode.
//! - THE GUARD: weapon form 9 guards with the Reflec guard (Guard slot 119 0x14139F770: `states/guard.rs` (F-ULTIMATE)),
//!   and an attack out of it with the counter window open is the forced kind 0x25 (TryAttack 0x141437E1A): BarrierCracker /
//!   AirBarrierCracker (9341 / 9351 of the set), invincible, whose Shot notify fires the projectile BARRIERCRACKER of
//!   w_so190_ProjSet = TresProjSW_BladeCounter ([`counter_shot`]: SPHERE 550, row SCSW_COUNTER, 0.1 s).
//! - FINISH_SW: StyleFinish slot 151 0x1413E06B0 (M_9419: the Ultima blades' storm, SW_Myriad / SW_Myriad2):
//!   `states/combo_sw.rs` `finish_tick`.
//! - THE START: StartStyle 0x14141A9D0 case 0x87: the soft-target range (controller+0x970) x 2 ([`on_start`]); EndStyle's
//!   reset puts it back.
//! - THE HIT SHAPES: the windows name the groups of the weapon in his hand (w_so192: SW_StickL, SW_Crack, SW_Rengeki ...)
//!   and of the transform unit (w_so191: ChangeSW of the transformation StyleChange9): the generator's FK places them per
//!   frame (the rows `track.*`, with their growth); [`hit_shape`] reads them for the generic hit windows of combat/hit.rs (the
//!   KH hit path: every Elden Ring enemy the host's lock-on accepts, its reactions, launches and juggles).
//! - THE ANIM SET: m_ReplaceAnimSets AnimSet9 ([`replaced_clip`]); the anim player resolves every play through it.
//!
//! (AUDIT-C SW, build/sora_tmp/weapons/AUDIT-C/SW/PART.md) KH3 spawns no SW_RENGEKI projectile (no Shot notify in 9411 ..
//! 9418, no FName "SW_RENGEKI" in the exe, only the projectile set names it) and reads +0x234 bit 16 nowhere (byte scans of
//! the exe): nothing to port for either. +0x22C is no mesh fade: it is the teleport's AURA GROUP (15 / 16 of
//! m_CmnAuraEffect, [`warp_aura`]). The weapons' own clips ([`on_notifies`]) and the clips' attach data asset
//! ead_so192_aura0 ([`attach_rows`], kh-host vfx.rs) are played.

use super::FormMoveset;
use crate::combat::form::KeybladeFormRow;
use crate::combat::hit::{spawn_form_shot, FormShot};
use crate::host::TargetId;
use crate::pawn::Ctx;
use crate::states::StateData;
use glam::Vec3;

// The rows (sheets/port/sora-form-ultimate.json): `FormUltimateRow`, `FORM_ULTIMATE`, `FSW_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_ultimate.rs"));

/// CHANGE_SW: Ultimate Form.
pub const CHANGE_SW: i32 = 0x87;
/// FINISH_SW (SetStyleFinishCommand 0x1413EE030 in form 0x87).
pub const FINISH_SW: i32 = 0xA1;
/// The guard counter of the form (TryAttack 0x141437E1A: forced kind 0x25 out of GUARD).
pub const KIND_COUNTER: u8 = 0x25;
/// The window key of the counter's projectile (combat/hit.rs `FormShot::key`).
const COUNTER_SHOT_KEY: u16 = 0x8725;

/// The package: the form, its finish command.
pub(crate) const MOVESET: FormMoveset = FormMoveset {
    name: "ultimate",
    forms: &["form.SW"],
    finishes: &[FINISH_SW],
    on_start,
    on_end,
    attack,
    finish_begin: crate::states::combo_sw::finish_begin,
    finish_tick: crate::states::combo_sw::finish_tick,
    replace_clip,
    hit_shape,
    hit_volumes, // (F-SHAPES)
    ..FormMoveset::NONE
};

/// The held form is Ultimate Form.
pub fn is_ultimate(cmd: i32) -> bool {
    cmd == CHANGE_SW
}

fn rows(what: &'static str) -> impl Iterator<Item = &'static FormUltimateRow> {
    FORM_ULTIMATE.iter().filter(move |r| r.what == what)
}

fn value(id: &str, k: usize) -> Option<f32> {
    form_ultimate_by_id(id).and_then(|r| r.values.get(k).copied())
}

/// The row of an attack kind (PlayAttack 0x1413378B0's case): 0xC7 .. 0xCC, and 0x25 on the ground or in the air.
pub fn attack_row(kind: u8, air: bool) -> Option<&'static FormUltimateRow> {
    let mut found = rows("attack").filter(|r| r.code == i32::from(kind));
    let first = found.next()?;
    if kind == KIND_COUNTER && air {
        return found.find(|r| r.flags.contains(&"air_variant")).or(Some(first));
    }
    Some(first)
}

/// A kind of COMBO_SW's chain (0xC7 .. 0xCC).
pub fn is_combo_kind(kind: u8) -> bool {
    (0xC7..=0xCC).contains(&kind)
}

/// m_ReplaceAnimSets AnimSet9: the clip (animation-table row) a logical name plays as in the form; None = the base set's.
pub fn replaced_clip(logical: &str) -> Option<&'static str> {
    rows("anim").find(|r| r.logical == logical && r.clip != "-").map(|r| r.clip)
}

/// The package's `replace_clip` hook (every play of the anim player in the form: anim.rs `start`).
fn replace_clip(logical: &str) -> Option<&'static str> {
    replaced_clip(logical)
}

/// MaxCombo (pawn slot 928 0x141433570) and Finishers (slot 1253 0x141433A60) in form 0x87: 6 / 3.
pub fn max_combo_and_finishers() -> (i32, i32) {
    (value("decide.SW", 10).map_or(6, |v| v as i32), value("decide.SW", 11).map_or(3, |v| v as i32))
}

// ================================================================================================ Decide

/// What ComboSW's Decide reads (PreDecide / TargetMetrics: state+0x128 the combo, +0x154 the reach, pawn+0xE10 the previous
/// kind, +0x1C4 the enemies around, +0x1CC / +0x1D0 the distance / height to the target, +0x1E0 target high, +0x142 the air
/// (+0x1EC bit 3 makes it 1), +0x1E8 the forced kind, the controller's soft-target range (vt+0x1700 = +0x970), pawn+0xE14).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwDecideIn {
    pub combo: i32,
    pub reach: u8,
    pub last_kind: u8,
    pub num_enemies: i32,
    pub dist: f32,
    pub dz: f32,
    pub target_high: bool,
    pub air: bool,
    pub forced_kind: u8,
    pub soft_range: f32,
    pub finish_count: i32,
}

/// What it leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwDecision {
    pub kind: u8,
    pub reach: u8,
    /// +0x1E1 (the teleport is to be made) with +0x25C = 0.5 and +0x22C = 15.
    pub warp: bool,
    pub finish: bool,
    pub finish_end: bool,
    pub finish_count: i32,
    /// combo 0: pawn vt+0x1D40(0) (pawn+0xE10 = 0).
    pub last_reset: bool,
}

/// `TresPlayerState_ComboSW::Decide` (slot 122 0x141363810, decompiled C):
///
/// ```text
/// PreDecide; last = pawn+0xE10
/// combo == 0: reach = dist > 400 ? 1 : (enemies > 1 ? 2 : 0); last = 0; pawn+0xE10 = 0
/// +0x1EC & 8: air = 1
/// forced == 0 && dist <= ctrl.range && (dist > 400 || target high || (air && dz < -100)): +0x1E1 = 1, +0x25C = 0.5, +0x22C = 15
/// kind = { 199 -> 200, 200 -> 201, 201 -> 202, 202 -> 203 (+0x1E1 = 0), 203 -> 204 (+0x1E1 = 0), else 199 + (combo != 0) }
/// combo >= MaxCombo - Finishers: finish (+0x140); kind 204: finish end (pawn vt+0x2740(1), +0x141); finish count + 1
/// ```
pub fn decide(i: &SwDecideIn) -> SwDecision {
    let mut d = SwDecision { kind: 0, reach: i.reach, warp: false, finish: false, finish_end: false, finish_count: i.finish_count, last_reset: false };
    let mut last = i.last_kind;
    if i.combo == 0 {
        d.reach = if i.dist > 400.0 { 1 } else if i.num_enemies > 1 { 2 } else { 0 };
        last = 0;
        d.last_reset = true;
    }
    let far = value("decide.SW", 6).unwrap_or(400.0);
    let below = value("decide.SW", 7).unwrap_or(-100.0);
    if i.forced_kind == 0 && i.dist <= i.soft_range && (i.dist > far || i.target_high || (i.air && i.dz < below)) {
        d.warp = true;
    }
    d.kind = match last {
        0xC7 => 0xC8,
        0xC8 => 0xC9,
        0xC9 => 0xCA,
        0xCA => {
            d.warp = false;
            0xCB
        }
        0xCB => {
            d.warp = false;
            0xCC
        }
        _ => 0xC7 + u8::from(i.combo != 0),
    };
    let (max, fin) = max_combo_and_finishers();
    if max - fin <= i.combo {
        d.finish = true;
        if d.kind == 0xCC {
            d.finish_end = true;
        }
        d.finish_count = i.finish_count + 1;
    }
    d
}

/// The invincibility after the teleport (+0x25C), s.
pub(crate) fn warp_invincible_time() -> f32 {
    value("decide.SW", 8).unwrap_or(0.5)
}

// ================================================================================================ the hit shapes

/// A track's capsule at a clip time (actor space, his yaw 0): [ax, ay, az, bx, by, bz, r] per frame, linear between frames.
pub fn track_at(row: &FormUltimateRow, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let frames = usize::try_from(row.code).ok().filter(|n| *n > 0)?;
    if row.values.len() < frames * 7 {
        return None;
    }
    let last = frames - 1;
    let f = if row.length > 0.0 { (time / row.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let at = |k: usize| row.values[i * 7 + k] + (row.values[j * 7 + k] - row.values[i * 7 + k]) * t;
    Some((Vec3::new(at(0), at(1), at(2)), Vec3::new(at(3), at(4), at(5)), at(6)))
}

/// The world shape of a group of the form's weapons in a clip (its sequence) at a clip time, for an actor at `location`
/// with `yaw`: (end 1, end 2, radius). None = not one of the form's tracked windows.
pub fn track_shape(group: &str, sequence: &str, time: f32, location: Vec3, yaw: f32) -> Option<(Vec3, Vec3, f32)> {
    let row = rows("track").find(|r| r.clip == sequence && r.logical == group)?;
    let (a, b, r) = track_at(row, time)?;
    let (sin, cos) = yaw.to_radians().sin_cos();
    let world = |v: Vec3| location + Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z);
    Some((world(a), world(b), r))
}

/// The package's `hit_shape` hook (combat/hit.rs `shape`): the form's tracked groups.
fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let name = crate::anim::SEQUENCES.get(sequence)?.name;
    track_shape(group, name, time, cx.pawn.cmc.location, cx.pawn.yaw)
}

/// (F-SHAPES) The world shapes of a group that is a CONVEX hull or several shapes (`vol.SW.<sequence>.<group>.<k>` rows:
/// SW_Finish2, SW_Myriad2, ChangeSW are tres_colltest01 hulls, SW_StabCross's two crossing blades two capsules) in a clip
/// (its sequence) at a clip time, for an actor at `location` with `yaw`. None = not such a group (its `track` row).
pub fn volume_shapes(group: &str, sequence: &str, time: f32, location: Vec3, yaw: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let out: Vec<_> = rows("vol").filter(|r| r.clip == sequence && r.logical == group).filter_map(|r| crate::combat::hull::VolTrack::of_row(r.flags, r.code, r.length, r.values)).map(|t| t.placed(time, location, yaw)).collect();
    (!out.is_empty()).then_some(out)
}

/// (F-SHAPES) The package's `hit_volumes` hook (combat/hit.rs `sweep_volumes`).
fn hit_volumes(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let name = crate::anim::SEQUENCES.get(sequence)?.name;
    volume_shapes(group, name, time, cx.pawn.cmc.location, cx.pawn.yaw)
}

// ================================================================================================ the guard counter

/// The forms whose guard counter is the forced kind 0x25 (TryAttack 0x141437E1A): 0x86, 0x87, 0x8E, 0x8F, 0x92, 0x93,
/// 0xDD .. 0xE0 (the others: 0x21 / 0x23 / 0x24).
pub fn counter_is_barrier_cracker(cmd: i32) -> bool {
    matches!(cmd, 0x86 | 0x87 | 0x8E | 0x8F | 0x92 | 0x93 | 0xDD..=0xE0)
}

/// The shot handler 0x141344580 case 0x25 in Ultimate Form: the projectile BARRIERCRACKER of w_so190_ProjSet
/// (TresProjSW_BladeCounter: a static SPHERE of m_Size 550, row SCSW_COUNTER, m_MainLife 0.1 s, not destroyed by a hit, its
/// m_HitEffects vw_so192_bld_hit1) at the weapon's ATK_01 (pawn vt+0x1410; the sheet's `shot.SW.<clip>` at the Shot notify)
/// with his yaw, carrying the attack's counters (shot vt+0x1130 ...).
pub(crate) fn counter_shot(cx: &mut Ctx, sequence: &str) {
    let Some(proj) = form_ultimate_by_id("proj.SW.BARRIERCRACKER") else { return };
    let at = rows("shot").find(|r| r.clip == sequence).map_or(Vec3::new(0.0, 0.0, 91.0), |r| Vec3::new(r.values[1], r.values[2], r.values[3]));
    let pawn = &*cx.pawn;
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    let position = pawn.cmc.location + Vec3::new(at.x * cos - at.y * sin, at.x * sin + at.y * cos, at.z);
    let v = |k: usize| proj.values.get(k).copied().unwrap_or(0.0);
    let hit_effect = proj.flags.get(3).copied().filter(|e| *e != "-");
    let row = proj.rows.first().copied().unwrap_or("SCSW_COUNTER");
    let info = cx.pawn.combat.attack_ctx;
    let yaw = cx.pawn.yaw;
    spawn_form_shot(cx, FormShot { key: COUNTER_SHOT_KEY, group: "BARRIERCRACKER", row, position, yaw, size: (v(1), v(2), v(3).max(v(1))), life: v(0), hit_effect, hit_se: None, info, hit_list: Vec::new() });
}

// ================================================================================================ the Reflec guard

/// Guard Begin 0x1413851D0, table 0x141385898: the weapon forms whose guard is the Reflec guard (slot 119 0x14139F770):
/// 8, 9 (Ultimate Form), 16, 17, 20 .. 23.
pub fn reflec_guard(weapon_form: i32) -> bool {
    matches!(weapon_form, 8 | 9 | 16 | 17 | 20..=23)
}

/// Slot 119: the effect group of the Reflec play by the weapon form (pawn vt+0x12E0(-1)): 22 -> 0x834, 23 -> 0x898, else
/// the sheet's 0xD48 (24 with Guard flag 0x2000: 0xD52; 24 never reaches slot 119).
pub fn reflec_group(weapon_form: i32) -> i32 {
    match weapon_form {
        22 => 0x834,
        23 => 0x898,
        _ => form_ultimate_by_id("guard.SW.Reflec").map_or(0xD48, |r| r.code),
    }
}

/// Slot 119's numbers: (the hit entry's start position in the clip s, the player guard's turn rate, the turn rate / timer
/// of a hit entry without success).
pub fn reflec_numbers() -> (f32, f32, f32) {
    (value("guard.SW.Reflec", 2).unwrap_or(0.166_666_6), value("guard.SW.Reflec", 3).unwrap_or(1.0), value("guard.SW.Reflec", 4).unwrap_or(0.05))
}

// ================================================================================================ the hooks

/// StartStyle 0x14141A9D0 case 0x87: ctrl+0x970 (the soft-target range) = pawn MyAutoLockonLimit x 2.
fn on_start(cx: &mut Ctx, _row: &'static KeybladeFormRow) {
    cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT * value("start.SW", 0).unwrap_or(2.0);
}

// (F-POLISH) The slot resume 0x14141E2B0 (a Keyblade switch back to Ultima Weapon's slot), case 0x87: the soft-target range
// x 2 is now states/weapon_change.rs `resume_switch`, the resume's switch for every form.

/// EndStyle 0x14141CA20 (its reset 0x141410FD0): the soft-target range back to MyAutoLockonLimit.
fn on_end(cx: &mut Ctx, _row: &'static KeybladeFormRow) {
    cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT;
}

/// The attack request in the form: COMBO_SW (pawn slot 1337 0x141432F00: 0x87 -> 0x52).
fn attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    crate::states::combo_sw::try_attack(state, cx, target)
}

// ------------------------------------------------------------------------------------------------ (AUDIT-C SW) the clips' extras

/// (AUDIT-C SW) The key range of the weapons' clip states in `Pawn::notify_handles`: (0xB000 | the play's id mod 0x1000, the
/// row's index). Outside the other passes' ranges (AUDIT-B F-SHIP 0xF000, F-CLAWS 0xD000 / 0xC000, AUDIT-A SHIELD 0xE000) and
/// above every sequence index of SEQUENCES.
const WNOTIFY_KEY: u16 = 0xB000;

/// (AUDIT-C SW) After the generic notify handlers of a pose tick (sora.rs), in every state: THE WEAPONS' OWN CLIPS (`wnotify`
/// rows, from build/sora_pack/keyblades/<model>/model.json; AUDIT-B F-SHIP's pass and row format). KH3's weapon actors play
/// the row of Sora's logical name on their own anim instance, in step with his clip: Ultima's transform unit w_so191 plays its
/// own 9401 while he plays StyleChange9, and its notifies fire on ITS mesh: the AttachSound se02019_011 (0 .. 4.7 s, fade
/// 0.7 s) at the unit, vw_so191_chg_ora0 / chg_trf0 on its kh_sk, chg_kaz0 / chg_lst0 on its bone w_so192 (anchor
/// `kb:<model>:<mount>:<bone>`, resolved by the host on the weapon's skeleton under that clip). A state notify ends at its end
/// time or when Sora's play changes (`WhenChangeAnimationType` END: the weapon's play changes with his).
pub(crate) fn on_notifies(cx: &mut Ctx) {
    use crate::host::{Attach, EffectId, EffectSpawn, SoundId, SoundPlay};
    use crate::notify::{notify_asset_id, NotifyHandle};
    let req = cx.pawn.anim_ticked;
    let seq = req.body_samples()[0].0;
    let play = WNOTIFY_KEY | (req.play_id & 0x0FFF) as u16;
    let time = req.time;
    let mut k = 0;
    while k < cx.pawn.notify_handles.len() {
        let (key, index, handle) = cx.pawn.notify_handles[k];
        if key & 0xF000 != WNOTIFY_KEY {
            k += 1;
            continue;
        }
        let end = FORM_ULTIMATE.get(usize::from(index)).and_then(|r| r.values.get(1).copied()).unwrap_or(0.0);
        if key == play && time < end {
            k += 1;
            continue;
        }
        cx.pawn.notify_handles.remove(k);
        match handle {
            NotifyHandle::Sound(h, fade) => cx.stop_sound(h, fade),
            NotifyHandle::Effect(h) => cx.stop_effect(h, false),
        }
    }
    let (from, to) = req.notify_window;
    if (from, to) == (0.0, 0.0) {
        return;
    }
    let group = cx.pawn.effect_group;
    for (index, r) in FORM_ULTIMATE.iter().enumerate().filter(|(_, r)| r.what == "wnotify" && r.clip == seq) {
        let v = |i: usize| r.values.get(i).copied().unwrap_or(0.0);
        let t = v(0);
        if !(t > from && t <= to || (from < 0.0 && t == 0.0)) {
            continue;
        }
        let state = r.flags.get(2) == Some(&"state");
        let (Some(anchor), Ok(index)) = (r.flags.first().copied(), u16::try_from(index)) else { continue };
        if r.code == 0 {
            let row_group = r.flags.get(3).and_then(|g| g.strip_prefix('g')).and_then(|g| g.parse::<i32>().ok()).unwrap_or(0);
            let Some(name) = r.effects.first().copied().filter(|_| crate::notify::effect_group_passes(row_group, true, group)) else { continue };
            let spawn = EffectSpawn { id: EffectId(notify_asset_id(name)), name, at: Attach::Sora { bone: Some(anchor), offset: Vec3::new(v(2), v(3), v(4)) }, rotation: Vec3::new(v(5), v(6), v(7)), scale: 1.0, persistent: state };
            let handle = cx.spawn_effect(spawn);
            if state {
                cx.pawn.notify_handles.push((play, index, NotifyHandle::Effect(handle)));
            }
        } else {
            let Some(name) = r.sounds.first().copied() else { continue };
            // the unit's sounds sit on its mesh: (AUDIT-A SOUND) at the row's own anchor (its AttachPointName, empty = the
            // mesh's root), placed by the hosts as its effects (kh-host audio/anchors.rs; without that mesh: its mount socket)
            let handle = cx.play_sound(SoundPlay { id: SoundId(notify_asset_id(name)), name, at: Attach::Sora { bone: Some(anchor), offset: Vec3::ZERO }, volume: 1.0, pitch: 1.0, looping: state, spatial: true });
            if state {
                cx.pawn.notify_handles.push((play, index, NotifyHandle::Sound(handle, v(8))));
            }
        }
    }
}

/// (AUDIT-C SW) The teleport's aura group `group` (15 at the vanish, 16 at the reappearance): the anim interface's
/// vt+0x1A0(m_CmnAuraEffect, +0x22C [+ 1], 0) with Sora's ead_pc_aura_com0 (rows `aura.SW.<group>.*`: ws_so192_wrp_in01 / in03
/// / out01 / out03 on vfx_center, ToAttach false, absolute rotation; ws_so192_wrp_in01 / in04 / out01 / out04 on the form
/// blade's vfx_bottom). ComboSW's Decide 0x141363810 sets +0x22C = 0xF with the teleport.
pub(crate) fn warp_aura(cx: &mut Ctx, group: i32) {
    use crate::host::{Attach, EffectId, EffectSpawn};
    use crate::notify::notify_asset_id;
    for r in rows("aura").filter(|r| r.code == group) {
        let (Some(name), Some(anchor)) = (r.effects.first().copied(), r.flags.first().copied()) else { continue };
        let v = |i: usize| r.values.get(i).copied().unwrap_or(0.0);
        cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(name)), name, at: Attach::Sora { bone: Some(anchor), offset: Vec3::new(v(0), v(1), v(2)) }, rotation: Vec3::new(v(3), v(4), v(5)), scale: 1.0, persistent: false });
    }
}

/// (AUDIT-C SW) The teleport's +0x22C (ComboSW's Decide 0x141363810: 0xF): the vanish's aura group; +1 the reappearance's.
pub const WARP_AURA_GROUP: i32 = 15;

/// (AUDIT-C SW) One row of an attach data asset the form's clips name (`ead` rows of the sheet; SQEX_StructParticleAttachData).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttachRow {
    /// The particle system the row plays.
    pub system: &'static str,
    /// The row's socket (`SocketName`).
    pub socket: &'static str,
    /// On the right-hand weapon (`SQEX_ATTACH_OBJECT_ATTACH_TO_RIGHT_HAND_WEAPON`), else on the owner (Sora).
    pub right_hand_weapon: bool,
    /// `ToAttach`.
    pub follow: bool,
    pub absolute_rotation: bool,
    /// LocationOffset, RotationOffset (pitch, yaw, roll), ScaleOffset.
    pub offset: Vec3,
    pub rotation: Vec3,
    pub scale: Vec3,
    /// ParticleColor rgb + ParticleAlpha.
    pub color: [f32; 4],
    pub fade_in: f32,
    pub start_delay: f32,
}

/// (AUDIT-C SW) The rows of the attach data asset `asset` (its name, any case) of the request's `group`: what KH3's
/// TresAnimNotifyState_AttachEffect plays for an `ead_*` entry (each row where its own row attaches it). Empty: not such an
/// asset of this package (ead_so192_aura0: vw_so192_foot_aura0 on Sora's L_ashi2 / R_ashi2, vw_so192_bld_aur0 on the right-hand
/// weapon's vfx_bottom, LOOPEND at the window's end).
pub fn attach_rows(asset: &str, group: i32) -> Vec<AttachRow> {
    attach_rows_where(|r| r.logical.eq_ignore_ascii_case(asset) && r.code == group)
}

/// (AUDIT-C SW) Every row of every attach data asset of the package (the host's placement table).
pub fn all_attach_rows() -> Vec<AttachRow> {
    attach_rows_where(|_| true)
}

fn attach_rows_where(keep: impl Fn(&FormUltimateRow) -> bool) -> Vec<AttachRow> {
    rows("ead")
        .filter(|r| keep(r))
        .filter_map(|r| {
            let v = |i: usize| r.values.get(i).copied().unwrap_or(0.0);
            Some(AttachRow {
                system: r.effects.first().copied()?,
                socket: r.clip,
                right_hand_weapon: r.flags.first() == Some(&"ATTACH_TO_RIGHT_HAND_WEAPON"),
                follow: r.flags.get(2) != Some(&"free"),
                absolute_rotation: r.flags.get(3) == Some(&"abs"),
                offset: Vec3::new(v(0), v(1), v(2)),
                rotation: Vec3::new(v(3), v(4), v(5)),
                scale: Vec3::new(v(6), v(7), v(8)),
                color: [v(9), v(10), v(11), v(12)],
                fade_in: v(13),
                start_delay: v(15),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn din(combo: i32, last: u8, dist: f32) -> SwDecideIn {
        SwDecideIn { combo, reach: 0, last_kind: last, num_enemies: 1, dist, dz: 0.0, target_high: false, air: false, forced_kind: 0, soft_range: 1800.0, finish_count: 0 }
    }

    /// (AUDIT-C SW) The data the audit added: ead_so192_aura0's three rows (both feet on Sora, the blade on the right-hand
    /// weapon), the transform unit w_so191's own 9401 notifies (one AttachSound, four effects), the teleport's aura groups 15 /
    /// 16 of ead_pc_aura_com0 (body rows free + absolute on vfx_center, weapon rows on w_so192's vfx_bottom).
    #[test]
    fn the_audit_rows() {
        let aura = attach_rows("EAD_so192_aura0", 0);
        assert_eq!(aura.iter().map(|r| (r.system, r.socket, r.right_hand_weapon)).collect::<Vec<_>>(), [("vw_so192_foot_aura0", "L_ashi2", false), ("vw_so192_foot_aura0", "R_ashi2", false), ("vw_so192_bld_aur0", "vfx_bottom", true)]);
        assert!(aura.iter().all(|r| r.follow && r.scale == Vec3::ONE && r.color == [1.0; 4]));
        assert!(attach_rows("ead_so192_aura0", 7).is_empty() && attach_rows("ead_key_trail0", 0).is_empty());
        let w: Vec<_> = rows("wnotify").collect();
        assert_eq!(w.iter().filter(|r| r.code == 1).flat_map(|r| r.sounds.iter().copied()).collect::<Vec<_>>(), ["se02019_011"]);
        assert_eq!(w.iter().filter(|r| r.code == 0).flat_map(|r| r.effects.iter().copied()).collect::<Vec<_>>(), ["vw_so191_chg_ora0", "vw_so191_chg_trf0", "vw_so191_chg_kaz0", "vw_so191_chg_lst0"]);
        assert!(w.iter().all(|r| r.clip == "9401" && r.flags[0].starts_with("kb:w_so191:R_wep_skt:")));
        let group = |g: i32| rows("aura").filter(|r| r.code == g).map(|r| (r.effects[0], r.flags[0])).collect::<Vec<_>>();
        assert_eq!(group(WARP_AURA_GROUP), [("ws_so192_wrp_in03", "vfx_center:free:abs"), ("ws_so192_wrp_in01", "vfx_center:free:abs"), ("ws_so192_wrp_in04", "kb:w_so192:R_wep_skt:vfx_bottom"), ("ws_so192_wrp_in01", "kb:w_so192:R_wep_skt:vfx_bottom")]);
        assert_eq!(group(WARP_AURA_GROUP + 1).len(), 4);
    }

    /// The chain 0xC7 .. 0xCC, the finishers from combo 3, the finish end on 0xCC.
    #[test]
    fn the_combo_of_combo_sw() {
        let (mut last, mut n, mut kinds) = (0u8, 0, Vec::new());
        for combo in 0..6 {
            let d = decide(&SwDecideIn { finish_count: n, ..din(combo, last, 150.0) });
            kinds.push((d.kind, d.finish, d.finish_end));
            (last, n) = (d.kind, d.finish_count);
        }
        assert_eq!(kinds, [(0xC7, false, false), (0xC8, false, false), (0xC9, false, false), (0xCA, true, false), (0xCB, true, false), (0xCC, true, true)]);
        assert_eq!(n, 3);
        assert_eq!(max_combo_and_finishers(), (6, 3));
        // An unknown previous kind: 0xC7 at combo 0, 0xC8 after.
        assert_eq!((decide(&din(0, 0x50, 100.0)).kind, decide(&din(2, 0x50, 100.0)).kind), (0xC7, 0xC8));
    }

    /// The teleport: farther than 400 within the soft range; never on 0xCB / 0xCC's predecessors' steps 0xCA / 0xCB.
    #[test]
    fn the_teleport_rule() {
        assert!(decide(&din(0, 0, 900.0)).warp);
        assert!(!decide(&din(0, 0, 300.0)).warp);
        assert!(!decide(&din(0, 0, 1900.0)).warp, "beyond the soft range (900 x 2)");
        assert!(!decide(&SwDecideIn { soft_range: 900.0, ..din(0, 0, 1000.0) }).warp);
        assert!(decide(&SwDecideIn { target_high: true, ..din(0, 0, 100.0) }).warp);
        assert!(decide(&SwDecideIn { air: true, dz: -150.0, ..din(1, 0xC7, 100.0) }).warp);
        assert!(!decide(&SwDecideIn { forced_kind: KIND_COUNTER, ..din(0, 0, 900.0) }).warp);
        assert!(!decide(&din(3, 0xCA, 900.0)).warp && !decide(&din(4, 0xCB, 900.0)).warp);
        assert!(decide(&din(5, 0xCC, 900.0)).warp, "0xCC itself keeps it (KH3 clears it only for 0xCB / 0xCC's choices)");
        assert_eq!(decide(&din(0, 0, 900.0)).reach, 1);
        assert_eq!(decide(&SwDecideIn { num_enemies: 3, ..din(0, 0, 100.0) }).reach, 2);
        assert_eq!(warp_invincible_time(), 0.5);
    }

    /// The sheet: every kind's clip is a row of the animation table, the anim set maps the guard, the counter and the
    /// locomotion, every window group has a track.
    #[test]
    fn the_sheet_carries_the_form() {
        for kind in 0xC7..=0xCCu8 {
            let r = attack_row(kind, false).unwrap_or_else(|| panic!("{kind:#x}"));
            assert!(crate::anim::AnimId::from_name(r.clip).is_some(), "{}: {} is not in the animation table", r.id, r.clip);
        }
        assert_eq!((attack_row(KIND_COUNTER, false).map(|r| r.clip), attack_row(KIND_COUNTER, true).map(|r| r.clip)), (Some("9341"), Some("9351")));
        assert_eq!((replaced_clip("Reflec"), replaced_clip("AirReflec"), replaced_clip("IdleB"), replaced_clip("M_9419")), (Some("9279"), Some("9280"), Some("9015"), None));
        assert!(crate::anim::AnimId::from_name("M_9419").is_some(), "FINISH_SW's clip (the XS overlay's row)");
        for r in rows("attack").chain(rows("finish")).chain(rows("change")) {
            for w in r.windows {
                let group = w.split('@').nth(1).unwrap_or("-");
                let seq = crate::anim::AnimId::from_name(r.clip).map(|a| a.data().samples[0].1).map(|s| crate::anim::SEQUENCES[s].name);
                let seq = seq.unwrap_or("-");
                // (F-SHAPES) or its shapes' `vol` rows
                assert!(rows("track").chain(rows("vol")).any(|t| t.clip == seq && t.logical == group), "{}: no track of {group} in {seq}", r.id);
            }
        }
        assert!(counter_is_barrier_cracker(CHANGE_SW) && !counter_is_barrier_cracker(0x89));
        assert!(reflec_guard(9) && !reflec_guard(1) && !reflec_guard(11));
        assert_eq!(reflec_group(9), 0xD48);
    }

    /// The shapes: the crack ahead of him, the blade at his hand, the storm's sphere around the blade.
    #[test]
    fn the_tracked_shapes() {
        let (a, b, r) = track_shape("SW_Crack", "9411", 0.7, Vec3::ZERO, 0.0).unwrap();
        assert!(a.x.max(b.x) > 700.0 && a.x.min(b.x) >= -1.0 && r == 100.0, "{a} {b} {r}");
        let (a, b, _) = track_shape("SW_Crack", "9411", 0.7, Vec3::ZERO, 90.0).unwrap();
        assert!(a.y.max(b.y) > 700.0, "turned with his yaw: {a} {b}");
        let (a, b, r) = track_shape("SW_StickL", "9412", 0.3, Vec3::ZERO, 0.0).unwrap();
        assert!(a.distance(b) > 100.0 && r == 120.0, "the blade's capsule ATK_00 -> ATK_01: {a} {b}");
        assert_eq!(track_shape("SW_Myriad", "9419", 1.5, Vec3::ZERO, 0.0).map(|s| s.2), Some(1400.0));
        assert!(track_shape("SW_Finish", "9417", 1.1, Vec3::ZERO, 0.0).is_some_and(|s| s.2 == 600.0), "grown to m_IncMaxSize");
        // (F-SHAPES) ChangeSW is a hull now (`vol` rows), no capsule track.
        assert!(track_shape("ChangeSW", "9401", 2.5, Vec3::ZERO, 0.0).is_none() && volume_shapes("ChangeSW", "9401", 2.5, Vec3::ZERO, 0.0).is_some());
        assert_eq!(track_shape("SW_Crack", "1301", 0.2, Vec3::ZERO, 0.0), None);
    }

    /// (F-SHAPES) KH3's real shapes: the CONVEX groups are tres_colltest01 hulls under their transform and growth, the crossing
    /// blades two capsules.
    #[test]
    fn the_hulls_and_the_crossing_blades() {
        use crate::combat::hull::Volume;
        let hull = |g: &str, seq: &str, t: f32, yaw: f32| match volume_shapes(g, seq, t, Vec3::ZERO, yaw).as_deref() {
            Some([Volume::Hull(h)]) => *h,
            other => panic!("{g}: {other:?}"),
        };
        // SW_Finish2 (vfx_cure + 100 up, LOCAL, (1, 1, 2) + (8, 8, 20)/s up to (8, 8, 10)): at the window's start radius 100 and
        // half height 100; grown, radius 800 and half height 500 - the old upright capsule stood a sphere of 800 there.
        let start = hull("SW_Finish2", "9418", 0.8, 0.0);
        assert!((start.scale - Vec3::new(1.0, 1.0, 2.0)).length() < 1.0e-3, "{start:?}");
        let grown = hull("SW_Finish2", "9418", 1.7, 0.0);
        assert!((grown.scale - Vec3::new(8.0, 8.0, 10.0)).length() < 1.0e-3, "{grown:?}");
        let c = grown.centre;
        assert!(grown.distance(c + Vec3::new(790.0, 0.0, 0.0)) == 0.0 && grown.distance(c + Vec3::Z * 490.0) == 0.0);
        assert!((grown.distance(c + Vec3::Z * 700.0) - 200.0).abs() < 0.1, "the top is 500 above its centre, not 800");
        // LOCAL: the world rotation is relRot (0) whatever his yaw.
        assert_eq!(hull("SW_Finish2", "9418", 1.7, 90.0).rotation, grown.rotation);
        // SW_Myriad2: radius 3000 at last, half height 1500.
        let storm = hull("SW_Myriad2", "9419", 4.8, 0.0);
        assert!((storm.scale - Vec3::splat(30.0)).length() < 1.0e-3, "{storm:?}");
        // ChangeSW: radius 500, half height 1000 (scale 5, 5, 20) on kh_sk + (0, -900, 0).
        let change = hull("ChangeSW", "9401", 2.5, 0.0);
        assert!((change.scale - Vec3::new(5.0, 5.0, 20.0)).length() < 1.0e-3 && (change.reach() - (500.0f32.hypot(1000.0))).abs() < 1.0, "{change:?}");
        // SW_StabCross: the two blades, each its own capsule, crossing (their ends apart, mirror images in y).
        let blades = volume_shapes("SW_StabCross", "9412", 1.2, Vec3::ZERO, 0.0).expect("the blades");
        let [Volume::Capsule { a: a0, b: b0, radius: r0 }, Volume::Capsule { a: a1, b: b1, radius: r1 }] = blades.as_slice() else { panic!("two capsules: {blades:?}") };
        assert!(*r0 == 100.0 && *r1 == 100.0);
        assert!((a0.y + a1.y).abs() < 1.0 && (b0.y + b1.y).abs() < 1.0 && a0.distance(*a1) > 100.0, "{a0} {b0} / {a1} {b1}");
        assert!((b0.y - a0.y).signum() != (b1.y - a1.y).signum(), "crossing: the two axes turn opposite ways");
    }
}
