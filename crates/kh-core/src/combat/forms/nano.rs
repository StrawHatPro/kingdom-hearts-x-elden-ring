//! (weapons F-NANO) Nano Arms (`form.GM`, CHANGE_GM 0x86, Nano Gear): its form package (documentation/KH3_WEAPONS_PLAN.md
//! 1.3 / 4; build/sora_tmp/weapons/F-NANO/STATE.md). A single-stage form of 30 s with FINISH_GM (0xA0), wear form 4
//! (Grace), one weapon part `w_so172_Wep` that changes its shape by its own clips (blade, fists, drill, giant hammer,
//! yo-yos, flail, the hook) and the attack class COMBO_GM (id 81, `states/combo_gm.rs`). The data is
//! `sheets/port/sora-form-nano.json` (tools/sora_form_nano.js): every number below that is not an address comes from it.
//!
//! WHAT KH3 DOES (decompiled C in re/decomp):
//! - THE ATTACK CLASS (pawn slot 1337 0x141432F00: 0x86 -> TresPlayerState_ComboGM, vtable 0x14690EBF0): the Attack class
//!   with Reset 0x141352E20 (+0x250 = 2000), Begin / Tick / End = the Attack's (thunks 0x141333480 / 0x1413417E0 /
//!   0x141335DF0), slot 136 0x141337890 (the warp point: the Attack's 0x141337320 with 100) and its own Decide 0x141361870
//!   ([`decide`]). MaxCombo 0x141433570 case 0x86: 6, Finishers 0x141433A60: 3.
//! - THE COMBO: the base kinds 1 / 2 / 3 / 8 / 0x10 .. 0x12 / 0x17 with AnimSet8's clips (the blade: 830x), the finishers
//!   0xB5 / 0xB6 (the fist crash / the drill), 0xB7 / 0xB8 (the giant stamp / the yo-yos), 0xB9 (the punch with the Rocket
//!   Fist), the rise 0xBA (the flail), the fall 0xBB, the far hookshot 0xBC -> 0xBD (PlayAttack 0x1413378B0's cases: the
//!   sheet's `attack.*` rows).
//! - THE ROCKET FIST: kind 0xB9's TresShot -> the shot handler 0x141344580 case 0xB9 -> BX_ROCKET (w_so170_ProjSet:
//!   TresProjGM_RocketPunch0, a 0.05 s blast that respawns as TresProjGM_RocketPunch, the flying fist; [`NanoState`]).
//! - NANO COUNTER: the guard counter in 0x86 is the forced kind 0x25 (TryAttack 0x141437E28), PlayAttack's GM branch of
//!   case 0x25 (BarrierCracker / AirBarrierCracker = sequences 1341 / 1351, invincible) and its shot BARRIERCRACKER
//!   (TresProj_BarrierCrackerBX: a 550 cm blast, SCGM_COUNTER). NANO SHIELD is the guard itself (no replacement of it).
//! - OFFENSIVE DODGE: AnimSet8's DodgeRollB / AirDodge (8051 / 8054, the claws: SCGM_DODGE windows), played by the dodge and
//!   the air dodge through `forms::anim`.
//! - FINISH_GM: TresPlayerState_StyleFinishGM (id 36): the gate w_so174 (TresStylePawnGMGate) and its windows SCGM_PORTAL /
//!   SCGM_PORTAL2 (`states/combo_gm.rs` `finish_tick`).
//!
//! THE HIT SHAPES: the weapon's TresAtkColl0 groups (CL_RStickL, StampL, YO_*, GM_Punch ...) and the Kingdom Key groups of
//! the 830x clips, per clip frame from the FK of the weapon's own clips (the sheet's `track.*`): [`hit_shape`].

use super::FormMoveset;
use crate::combat::form::KeybladeFormRow;
use crate::combat::hit::{spawn_form_shot, FormShot, FORM_SHOT_SEQUENCE};
use crate::combat::AttackCtx;
use crate::host::{TargetFlags, TargetId};
use crate::pawn::Ctx;
use crate::states::StateData;
use glam::Vec3;

// The rows (sheets/port/sora-form-nano.json): `FormNanoRow`, `FORM_NANO`, `FNA_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_nano.rs"));

/// CHANGE_GM: Nano Arms.
pub const CHANGE_GM: i32 = 0x86;
/// FINISH_GM (SetStyleFinishCommand 0x1413EE030).
pub const FINISH_GM: i32 = 0xA0;
/// The kinds of COMBO_GM's own cases.
pub mod kind {
    pub const CRASH: u8 = 0xB5;
    pub const DRILL: u8 = 0xB6;
    pub const STAMP: u8 = 0xB7;
    pub const YOYO: u8 = 0xB8;
    pub const ROCKET: u8 = 0xB9;
    pub const RISE: u8 = 0xBA;
    pub const FALL: u8 = 0xBB;
    pub const HOOK: u8 = 0xBC;
    pub const HOOK_END: u8 = 0xBD;
    /// The Nano Counter (the guard counter of 0x86: BARRIERCRACKER).
    pub const COUNTER: u8 = 0x25;
}
/// The key range of this package's shots (`Window::key` = (FORM_SHOT_SEQUENCE, key)): the high half, apart from the gun
/// forms' counter.
const KEY_BASE: u16 = 0x8000;
/// A shot this package removes itself.
const NO_LIFE: f32 = 1.0e9;

/// The package: Nano Arms and FINISH_GM.
pub(crate) const MOVESET: FormMoveset = FormMoveset {
    name: "nano",
    forms: &["form.GM"],
    finishes: &[FINISH_GM],
    on_end,
    attack,
    finish_begin: crate::states::combo_gm::finish_begin,
    finish_tick: crate::states::combo_gm::finish_tick,
    replace_clip,
    hit_shape,
    hit_volumes, // (F-SHAPES)
    ..FormMoveset::NONE
};

/// (F-SHAPES) The world shapes of a group of w_so172 that is a CONVEX (`vol.sora.<sequence>.<group>.<k>` rows: StampL
/// tres_colltest01 x (8, 4, 5) under ham_flg_head (roll 90), SwingM x (5.5, 5.5, 4) at Sora's `center`; before, spheres of their
/// m_Size 50 / 550) at a clip time, for an actor at `location` with `yaw`.
pub fn volume_shapes(group: &str, sequence: &str, time: f32, location: Vec3, yaw: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let out: Vec<_> = rows("vol").filter(|r| r.clip == sequence && r.logical == group).filter_map(|r| crate::combat::hull::VolTrack::of_row(r.flags, r.code, r.length, r.values)).map(|t| t.placed(time, location, yaw)).collect();
    (!out.is_empty()).then_some(out)
}

/// (F-SHAPES) The package's `hit_volumes` hook (combat/hit.rs `sweep_volumes`).
fn hit_volumes(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let name = crate::anim::SEQUENCES.get(sequence)?.name;
    volume_shapes(group, name, time, cx.pawn.cmc.location, cx.pawn.yaw)
}

/// Nano Arms is held.
pub fn is_nano(cmd: i32) -> bool {
    cmd == CHANGE_GM
}

fn rows(what: &'static str) -> impl Iterator<Item = &'static FormNanoRow> {
    FORM_NANO.iter().filter(move |r| r.what == what)
}

fn v(row: &FormNanoRow, i: usize) -> f32 {
    row.values.get(i).copied().unwrap_or(0.0)
}

/// The row of an attack kind (`attack.<kind>`; the Nano Counter by ground / air).
pub fn attack_row(kind: u8, air: bool) -> Option<&'static FormNanoRow> {
    let mut it = rows("attack").filter(move |r| r.code == i32::from(kind));
    let first = it.next()?;
    if kind != self::kind::COUNTER {
        return Some(first);
    }
    std::iter::once(first).chain(it).find(|r| r.air_mode == air)
}

/// m_ReplaceAnimSets (AnimSet8): the animation-table row a logical name plays as in Nano Arms; None = the base set's.
pub fn replaced_clip(logical: &str) -> Option<&'static str> {
    rows("anim").find(|r| r.logical == logical && r.clip != "-").map(|r| r.clip)
}

fn replace_clip(logical: &str) -> Option<&'static str> {
    replaced_clip(logical)
}

/// A value of a `code.*` row.
pub(crate) fn code(id: &str, i: usize) -> f32 {
    rows("code").find(|r| r.id == id).map_or(0.0, |r| v(r, i))
}

/// MaxCombo (pawn slot 928 0x141433570) / Finishers (slot 1253 0x141433A60) in 0x86.
pub fn max_combo() -> i32 {
    code("code.combo", 0) as i32
}

pub fn finishers() -> i32 {
    code("code.combo", 1) as i32
}

// ------------------------------------------------------------------------------------------------ the decision

/// What ComboGM's Decide reads (state+0x128 the combo, +0x154 the reach, pawn+0xE10 the previous kind, +0x1CC / +0x1D0
/// the distance / height to the target, +0x1E0 target high, +0x142 the air, +0x1C4 the targets within 400 cm, pawn+0xE14
/// the finishers played).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GmDecideIn {
    pub combo: i32,
    pub reach: u8,
    pub last_kind: u8,
    pub dist: f32,
    pub dz: f32,
    pub target_high: bool,
    pub air: bool,
    pub num_enemies: i32,
    pub finish_count: i32,
}

/// What it leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GmDecision {
    pub kind: u8,
    pub reach: u8,
    pub finish: bool,
    pub finish_end: bool,
    pub finish_count: i32,
    /// +0x1DD with +0x1D4: the rise's fixed DirZ.
    pub fixed_dir_z: Option<f32>,
    /// combo 0: pawn vt+0x1D40(0) (pawn+0xE10 = 0).
    pub last_reset: bool,
}

/// `TresPlayerState_ComboGM::Decide` (0x141361870, decompiled C):
///
/// ```text
/// PreDecide; last = pawn+0xE10
/// combo 0: dist <= 400 ? (targets within 400 > 1 ? reach = 2) : reach = 1;  last = 0; pawn+0xE10 = 0
/// max = 6, fin = 3
/// combo >= max - fin:  combo < max: finish; n = pawn+0xE14; n >= fin - 1 || combo >= max - 1: finish end (pawn vt+0x2740(1))
///                      n 0: 0xB5 + air, n 1: 0xB7 + air, n 2: 0xB9, else 1; pawn+0xE14 = n + 1        (combo >= max: 1)
/// target high && dz > 150 && last != 0xBA && dist <= 400: 0xBA, DirZ = pawn rise speed x 200 (fixed)
/// last not 0xBC / 0xBD && dist > 400: 0xBC
/// dz < -100 && last != 0xBB && dist <= 400: 0xBB
/// else by last: 1 -> reach 1 ? 3 : 2;  2 / 0x11 -> reach 1 ? 1 : 3;  3 / 0x12 -> reach 1 ? 1 : 2;  8 / 0xBA / 0xBB -> 1;
///      0x0F / 0x10 -> 8;  0x17 -> 3;  0xBC / 0xBD -> 2;
///      other: first (combo 0) -> reach 1: 2, reach 2: 3, else 1; else by combo % 3: 0 -> reach + 1, 1 -> reach != 0 ? 1 : 2,
///      2 -> reach != 2 ? 3 : 2;
///      then + 0x0F when air or (combo > 0 and the target high)
/// ```
pub fn decide(i: &GmDecideIn) -> GmDecision {
    let mut d = GmDecision { kind: 1, reach: i.reach, finish: false, finish_end: false, finish_count: i.finish_count, fixed_dir_z: None, last_reset: false };
    let mut last = i.last_kind;
    let mut first = false;
    if i.combo == 0 {
        if i.dist <= 400.0 {
            if i.num_enemies > 1 {
                d.reach = 2;
            }
        } else {
            d.reach = 1;
        }
        last = 0;
        first = true;
        d.last_reset = true;
    }
    let (max, fin) = (max_combo(), finishers());
    if max - fin <= i.combo {
        if i.combo < max {
            let n = i.finish_count;
            d.finish = true;
            if fin - 1 <= n || max - 1 <= i.combo {
                d.finish_end = true;
            }
            d.kind = match n {
                0 => kind::CRASH + u8::from(i.air),
                1 => kind::STAMP + u8::from(i.air),
                2 => kind::ROCKET,
                _ => 1,
            };
            d.finish_count = n + 1;
        }
        return d;
    }
    if i.target_high && i.dz > 150.0 && last != kind::RISE && i.dist <= 400.0 {
        d.kind = kind::RISE;
        // DirZ = the pawn's rise speed x 200 (Attack's rule 8: pawn+0xBAC 3.5 x 200).
        d.fixed_dir_z = Some(crate::combat::tables::P.decide_rise_speed);
        return d;
    }
    if !matches!(last, kind::HOOK | kind::HOOK_END) && i.dist > 400.0 {
        d.kind = kind::HOOK;
        return d;
    }
    if i.dz < -100.0 && last != kind::FALL && i.dist <= 400.0 {
        d.kind = kind::FALL;
        return d;
    }
    let reach = d.reach;
    let mut k: u8 = match last {
        1 => if reach == 1 { 3 } else { 2 },
        2 | 0x11 => if reach == 1 { 1 } else { 3 },
        3 | 0x12 => if reach != 1 { 2 } else { 1 },
        8 | kind::RISE | kind::FALL => 1,
        0x0F | 0x10 => 8,
        0x17 => 3,
        kind::HOOK | kind::HOOK_END => 2,
        _ if first => match reach {
            1 => 2,
            2 => 3,
            _ => 1,
        },
        _ => match i.combo.rem_euclid(3) {
            0 => reach + 1,
            1 => if reach != 0 { 1 } else { 2 },
            _ => if reach != 2 { 3 } else { 2 },
        },
    };
    if i.air || (i.combo > 0 && i.target_high) {
        k += 0x0F;
    }
    d.kind = k;
    d
}

// ------------------------------------------------------------------------------------------------ the hooks

/// The attack request in Nano Arms: COMBO_GM (pawn slot 1337 0x141432F00: 0x86 -> ComboGM).
fn attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    crate::states::combo_gm::try_attack(state, cx, target)
}

/// EndStyle's per-form part: nothing of 0x86 in EndStyle 0x14141CA20; the gate of a finish cut short goes with the form.
fn on_end(cx: &mut Ctx, _: &'static KeybladeFormRow) {
    let n = &mut cx.pawn.combat.nano;
    n.gate = None;
    n.hide_sora = false;
}

/// Sora's actor space (x forward, y right, z up: the sheet's tracks) -> the world.
fn actor_to_world(cx: &Ctx, p: Vec3) -> Vec3 {
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    cx.pawn.cmc.location + Vec3::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos, p.z)
}

/// The frame of a per-frame track at `time` (linear between frames): `stride` floats a frame.
fn sample(row: &FormNanoRow, time: f32, stride: usize) -> Option<Vec<f32>> {
    let frames = row.values.len() / stride;
    if frames == 0 {
        return None;
    }
    let f = if row.length > 0.0 { (time / row.length).clamp(0.0, 1.0) * (frames - 1) as f32 } else { 0.0 };
    let i = (f.floor() as usize).min(frames - 1);
    let j = (i + 1).min(frames - 1);
    let t = f - i as f32;
    let a = &row.values[i * stride..(i + 1) * stride];
    let b = &row.values[j * stride..(j + 1) * stride];
    Some(a.iter().zip(b).map(|(x, y)| x + (y - x) * t).collect())
}

/// The shape row of a group (w_so172_Wep / w_so174 TresAtkColl0).
fn shape_row(owner: &str, group: &str) -> Option<&'static FormNanoRow> {
    rows("shape").find(|r| r.clip == owner && r.logical == group)
}

/// The radius of a shape `t` s after its activation (size + inc x t, capped by inc max when > 0: 0x140649E40).
fn radius(shape: &FormNanoRow, t: f32) -> f32 {
    let (size, inc, max) = (v(shape, 0), v(shape, 1), v(shape, 2));
    let r = (size + inc * t).max(0.0);
    if max > 0.0 { r.min(max) } else { r }
}

/// `m_bDisableLocation1Attach` of a weapon group (combat/hit.rs freezes it after the first tick).
pub(crate) fn fixed_group(cmd: i32, group: &str) -> bool {
    is_nano(cmd) && shape_row("w_so172_Wep", group).is_some_and(|r| r.flags.get(4) == Some(&"fixed"))
}

/// The package's `hit_shape` hook: a window's shape in Nano Arms from the clip's track (`track.sora.<sequence>.<group>`)
/// at clip time `time`: (end 1, end 2, radius), world. The radius grows from the window's begin (the clip's notify).
fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let seq = crate::anim::SEQUENCES.get(sequence)?;
    let track = rows("track").find(|r| r.clip == seq.name && r.logical == group && r.flags.first() == Some(&"sora"))?;
    let shape = shape_row("w_so172_Wep", group)?;
    let s = sample(track, time, 6)?;
    // The window's begin: the latest Attack notify of this group at or before `time`.
    let begin = seq
        .notifies
        .iter()
        .filter(|n| matches!(n.data, crate::anim::NotifyData::Attack { group: g, .. } if g == group) && n.time <= time + 1.0e-4)
        .map(|n| n.time)
        .fold(0.0f32, f32::max);
    let r = radius(shape, (time - begin).max(0.0));
    Some((actor_to_world(cx, Vec3::new(s[0], s[1], s[2])), actor_to_world(cx, Vec3::new(s[3], s[4], s[5])), r))
}

// ------------------------------------------------------------------------------------------------ the shots

/// One live Rocket Fist (TresProjGM_RocketPunch0 -> TresProjGM_RocketPunch).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rocket {
    pub key: u16,
    /// 0 = the blast at the fist (RocketPunch0), 1 = the flying fist (RocketPunch).
    pub stage: u8,
    /// The actor's location (the attack sphere and the root sphere sit 150 cm ahead of it).
    pub position: Vec3,
    pub velocity: Vec3,
    pub forward: Vec3,
    pub age: f32,
    pub target: TargetId,
    homing_on: bool,
    homing_delay: f32,
    info: AttackCtx,
    pub dead: bool,
}

/// What the host draws of the gate (FINISH_GM): w_so174 at `position` / `yaw` playing `clip` at `time`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    pub model: &'static str,
    pub position: Vec3,
    pub yaw: f32,
    /// The logical clip (M_8416 / M_8417 / M_8418) of w_so174's own set.
    pub clip: &'static str,
    pub time: f32,
    /// The next activation time of each of its windows (the interval re-opens SCGM_PORTAL).
    pub(crate) next: [f32; 2],
}

/// Nano Arms' run-time state on the pawn (PawnCombat::nano): the Rocket Fists in flight (they outlive the form), the gate.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct NanoState {
    pub rockets: Vec<Rocket>,
    next: u16,
    pub gate: Option<Gate>,
    /// FINISH_GM's sub-step 2 -> 5: Sora hidden (pawn vt+0xD98(0)); the host hides his mesh.
    pub hide_sora: bool,
    /// Counts (tests, the debug text).
    pub rockets_fired: u32,
    pub counters_fired: u32,
    pub portal_shots: u32,
    /// (AUDIT-B F-NANO) The live effects / sounds of the weapon's and the gate's own clips (`wfx` rows) and of the flying
    /// fists (keyed by the rocket's key); the clocks the weapon's / the gate's notify windows are read on.
    clip_fx: Vec<ClipFx>,
    weapon_clock: Option<(u32, f32)>,
    gate_clock: Option<(&'static str, f32)>,
    /// (AUDIT-B F-NANO) Counts of what the clips' rows started (tests, the debug text).
    pub clip_fx_started: u32,
}

impl NanoState {
    fn key(&mut self) -> u16 {
        self.next = self.next.wrapping_add(1) & 0x7FFF;
        KEY_BASE | self.next
    }
}

fn proj(name: &str, stage: i32) -> Option<&'static FormNanoRow> {
    rows("proj").find(|r| r.logical == name && r.code == stage)
}

// The proj rows' values: [life, speed, max speed, homing delay, max angle, turn max, root radius, root x, shape radius,
// shape x, no destroy on hit, map normal check].
const LIFE: usize = 0;
const SPEED: usize = 1;
const MAX_SPEED: usize = 2;
const HOMING_DELAY: usize = 3;
const MAX_ANGLE: usize = 4;
const TURN_MAX: usize = 5;
const ROOT_RADIUS: usize = 6;
const ROOT_X: usize = 7;
const SHAPE_RADIUS: usize = 8;
const SHAPE_X: usize = 9;
/// (F-POLISH) m_CheckMapHitNormal in degrees when m_bEnableMapHitNormalCheck, else 0.
const MAP_NORMAL: usize = 11;

fn hit_effect(row: &FormNanoRow) -> Option<&'static str> {
    row.flags.get(5).copied().filter(|x| *x != "-")
}

/// The muzzle of weapon 0 (pawn vt+0x1410(0) 0x1405AA790: w_so172's ATK_01) of the playing clip, world.
fn muzzle(cx: &Ctx) -> Vec3 {
    let seq = cx.pawn.anim.name.data().samples.first().map(|&(_, i)| crate::anim::SEQUENCES[i].name).unwrap_or("-");
    match rows("muzzle").find(|r| r.clip == seq).and_then(|r| sample(r, cx.pawn.anim.time, 3)) {
        Some(m) => actor_to_world(cx, Vec3::new(m[0], m[1], m[2])),
        None => cx.pawn.cmc.location,
    }
}

/// The shot handler 0x141344580 for Nano Arms' kinds (the Attack class's own cases are attack.rs's): 0xB9 = the notify's
/// projectile BX_ROCKET with the homing target (the state's +0x118), 0x25 = BARRIERCRACKER; at weapon 0's muzzle, along his
/// root rotation, with the attack's counters and finish flags (shot vt+0x1130 / +0x1140 / +0x1120 / +0x1180).
pub(crate) fn shot(cx: &mut Ctx, kind: u8, target: TargetId) {
    let at = muzzle(cx);
    let yaw = cx.pawn.yaw;
    let dir = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
    let info = cx.pawn.combat.attack_ctx;
    match kind {
        kind::ROCKET => {
            let Some(row) = proj("BX_ROCKET", 0) else { return };
            let key = cx.pawn.combat.nano.key();
            let size = (v(row, SHAPE_RADIUS), 0.0, 0.0);
            spawn_form_shot(cx, FormShot { key, group: "BX_ROCKET", row: row.rows.first().copied().unwrap_or("None"), position: at + dir * v(row, SHAPE_X), yaw, size, life: NO_LIFE, hit_effect: hit_effect(row), hit_se: None, info, hit_list: Vec::new() });
            let n = &mut cx.pawn.combat.nano;
            n.rockets.push(Rocket { key, stage: 0, position: at, velocity: Vec3::ZERO, forward: dir, age: 0.0, target, homing_on: false, homing_delay: 0.0, info, dead: false });
            n.rockets_fired += 1;
        }
        kind::COUNTER => {
            let Some(row) = proj("BARRIERCRACKER", 0) else { return };
            let key = cx.pawn.combat.nano.key();
            let size = (v(row, SHAPE_RADIUS), 0.0, 0.0);
            spawn_form_shot(cx, FormShot { key, group: "BARRIERCRACKER", row: row.rows.first().copied().unwrap_or("None"), position: at + dir * v(row, SHAPE_X), yaw, size, life: v(row, LIFE), hit_effect: hit_effect(row), hit_se: None, info, hit_list: Vec::new() });
            cx.pawn.combat.nano.counters_fired += 1;
        }
        _ => {}
    }
}

/// The Rocket Fists' step (real time: projectiles are object type 5; called from the pawn tick as the gun forms' bullets):
/// RocketPunch0's life (0.05 s) -> its respawn as RocketPunch (m_bReSpawnProjectileOnTimeOut: the target, the hit list and
/// the finish flag taken over, the rotation inherited), the flying fist's homing (UTresProjectileMovementComponent
/// 0x1414751E0: delay, turn max, max angle; speed 2000 from the ctor 0x141470A90), its root sweep against the level, its
/// life (5 s; not destroyed by a hit).
pub(crate) fn tick(cx: &mut Ctx, dt: f32) {
    if cx.pawn.combat.nano.rockets.is_empty() {
        return;
    }
    let mut list = std::mem::take(&mut cx.pawn.combat.nano.rockets);
    let mut respawn: Vec<(Rocket, Vec<TargetId>)> = Vec::new();
    for r in list.iter_mut() {
        let Some(row) = proj("BX_ROCKET", i32::from(r.stage)) else {
            r.dead = true;
            continue;
        };
        r.age += dt;
        if r.age >= v(row, LIFE) {
            r.dead = true;
            if r.stage == 0 && row.flags.first() == Some(&"respawn") {
                let taken = if row.flags.get(3) == Some(&"take_hit_list") { shot_hit_list(cx, r.key) } else { Vec::new() };
                respawn.push((*r, taken));
            }
            continue;
        }
        if r.stage == 1 {
            step(cx, r, row, dt);
        }
    }
    for r in list.iter().filter(|r| r.dead) {
        let key = r.key;
        cx.pawn.combat.shots.retain(|s| s.window.key != (FORM_SHOT_SEQUENCE, key));
    }
    list.retain(|r| !r.dead);
    list.append(&mut cx.pawn.combat.nano.rockets);
    cx.pawn.combat.nano.rockets = list;
    for (old, hit_list) in respawn {
        let Some(row) = proj("BX_ROCKET", 1) else { continue };
        let key = cx.pawn.combat.nano.key();
        let size = (v(row, SHAPE_RADIUS), 0.0, 0.0);
        let at = old.position + old.forward * v(row, SHAPE_X);
        let yaw = old.forward.y.atan2(old.forward.x).to_degrees();
        spawn_form_shot(cx, FormShot { key, group: "BX_ROCKET", row: row.rows.first().copied().unwrap_or("None"), position: at, yaw, size, life: NO_LIFE, hit_effect: hit_effect(row), hit_se: None, info: old.info, hit_list });
        let homing = row.flags.get(1) == Some(&"homing");
        cx.pawn.combat.nano.rockets.push(Rocket { key, stage: 1, position: old.position, velocity: old.forward * v(row, SPEED), forward: old.forward, age: 0.0, target: old.target, homing_on: homing, homing_delay: v(row, HOMING_DELAY), info: old.info, dead: false });
        // (AUDIT-B F-NANO) its TresEffectAttach0 (ead_so170_com0 group 1620)
        fist_fx(cx, key);
    }
    // (AUDIT-B F-NANO) the fists' effects ride with them (their place after this step)
    let fists: Vec<(crate::host::EffectHandle, &'static FormNanoRow, u16)> = cx.pawn.combat.nano.clip_fx.iter().filter_map(|f| match (f.owner, f.handle) {
        (FxOwner::Fist(k), FxHandle::Effect(h)) => Some((h, f.row, k)),
        _ => None,
    }).collect();
    for (h, row, key) in fists {
        if let Some(actor) = owner_actor(cx, FxOwner::Fist(key)) {
            let (at, rotation) = fx_world(row, 0.0, actor);
            cx.host.effects.transform(h, at, rotation, Vec3::new(v(row, 1), v(row, 2), v(row, 3)));
        }
    }
}

fn shot_hit_list(cx: &Ctx, key: u16) -> Vec<TargetId> {
    cx.pawn.combat.shots.iter().find(|s| s.window.key == (FORM_SHOT_SEQUENCE, key)).map(|s| s.window.hit_list().to_vec()).unwrap_or_default()
}

/// (F-POLISH) HandleImpact 0x14147AD80's normal check for a level hit (not started inside): `angle` = m_CheckMapHitNormal
/// (0 = no check). Some(the velocity along the surface, speed kept) when the hit is shallower than the angle (the graze),
/// None when it stops the projectile.
pub(crate) fn map_impact(velocity: Vec3, impact_normal: Vec3, angle: f32) -> Option<Vec3> {
    let dir = velocity.normalize_or_zero();
    if angle <= 0.0 || -dir.dot(impact_normal) >= angle.to_radians().cos() {
        return None;
    }
    let slid = velocity - impact_normal * velocity.dot(impact_normal);
    Some(if slid.length_squared() > 1.0e-8 { slid.normalize() * velocity.length() } else { velocity })
}

fn step(cx: &mut Ctx, r: &mut Rocket, row: &FormNanoRow, dt: f32) {
    // The homing (0x1414751E0 / 0x141481160, as magic/projectile.rs ports it): no turn acceleration, no cap (the ctor's 0).
    if r.homing_on {
        if r.homing_delay > 0.0 {
            r.homing_delay = (r.homing_delay - dt).max(0.0);
        }
        let point = cx.target(r.target).filter(|t| t.flags.has(TargetFlags::ALIVE)).map(|t| t.center);
        if let (true, Some(point)) = (r.homing_delay <= 0.0, point) {
            let turn = (v(row, TURN_MAX).to_radians() * dt).min(std::f32::consts::PI);
            let cos_max = if v(row, MAX_ANGLE) <= 0.0 { -1.0 } else { v(row, MAX_ANGLE).to_radians().cos() };
            r.velocity = crate::magic::projectile::steer(r.velocity, point - (r.position + r.forward * v(row, SHAPE_X)), turn, cos_max);
        }
    }
    let max = v(row, MAX_SPEED);
    if max > 0.0 && r.velocity.length() > max {
        r.velocity = r.velocity.normalize() * max;
    }
    if r.velocity.length_squared() > 1.0e-8 {
        r.forward = r.velocity.normalize();
    }
    let start = r.position;
    let delta = r.velocity * dt;
    // The root sphere (TresRoot: radius 50, 150 cm ahead) against the level (the host's World sweep: its impact normal).
    let off = r.forward * v(row, ROOT_X);
    let rr = v(row, ROOT_RADIUS);
    if rr > 0.0 {
        if let Some(h) = cx.world.sweep_capsule(start + off, start + off + delta, rr, rr).filter(|h| !h.start_penetrating) {
            let t = h.time.clamp(0.0, 1.0);
            // (F-POLISH) HandleImpact 0x14147AD80 (no bShouldBounce in the blueprint): with m_bEnableMapHitNormalCheck
            // (+0x2D8 bit 1) and a hit not started inside, a hit whose head-on part -(dir . ImpactNormal) is below
            // cos(m_CheckMapHitNormal 60) only sets the graze flag (+0x388 |= 0x10) and the fist goes on along the
            // surface, speed kept (as magic/projectile.rs ports the same component); a steeper hit stops it
            // (vt+0x500 StopSimulating): the fist ends.
            if let Some(slid) = map_impact(r.velocity, h.impact_normal, v(row, MAP_NORMAL)) {
                let contact = start + delta * t;
                if slid.length_squared() > 1.0e-8 {
                    r.velocity = slid;
                    r.forward = slid.normalize();
                }
                // The rest of the step along the surface, lifted off it by a hair (the next sweep must not start in it).
                let from = contact + h.impact_normal * 0.1;
                let rest = r.velocity * dt * (1.0 - t);
                let off = r.forward * v(row, ROOT_X);
                r.position = match cx.world.sweep_capsule(from + off, from + off + rest, rr, rr) {
                    Some(second) => from + rest * second.time.clamp(0.0, 1.0),
                    None => from + rest,
                };
            } else {
                r.position = start + delta * t;
                r.dead = true;
            }
        } else {
            r.position = start + delta;
        }
    } else {
        r.position = start + delta;
    }
    let (key, centre, yaw) = (r.key, r.position + r.forward * v(row, SHAPE_X), r.forward.y.atan2(r.forward.x).to_degrees());
    if let Some(shot) = cx.pawn.combat.shots.iter_mut().find(|s| s.window.key == (FORM_SHOT_SEQUENCE, key)) {
        shot.position = centre;
        shot.yaw = yaw;
    }
}

// ------------------------------------------------------------------------------------------------ the hookshot

/// The hookshot's hit (the on-contact handler 0x141355780 of kind 0xBC): the yo-yo hit the attack's own target. Returns
/// (the warp point, the start time into M_8424): slot 136 (ComboGM 0x141337890 = the Attack's 0x141337320 with 150 / 100:
/// 150 cm short of the target's lock point on the line from him, 100 cm below it, each leg swept against the level), the
/// start (1000 - |warp - him|xy) x 0.000333, at least 0.
pub(crate) fn hook_warp(cx: &Ctx, target: TargetId) -> Option<(Vec3, f32)> {
    let t = cx.target(target).filter(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE))?;
    let (distance, drop, from, rate) = (code("code.hook", 0), code("code.hook", 1), code("code.hook", 2), code("code.hook", 3));
    let own = cx.pawn.cmc.location;
    // (F-CLAWS for F-NANO) ComboGM slot 136 0x141337890 = the Attack's 0x141337320 with 100, straight mode (kind 0xBC): the
    // shared slot-136 line tests from HIM (states/attack.rs `warp_point`): never beyond a wall; placed swept (`swept_warp`)
    let end = crate::states::attack::warp_point(cx, &t, distance, drop);
    let dxy = (end - own).truncate().length();
    Some((end, ((from - dxy) * rate).max(0.0)))
}

/// The hookshot's values (`code.hook`): (stop distance, z gain, gravity scale, +0x18C).
pub(crate) fn hook_values() -> (f32, f32, f32, f32) {
    (code("code.hook", 4), code("code.hook", 5), code("code.hook", 6), code("code.hook", 7))
}

// ------------------------------------------------------------------------------------------------ FINISH_GM

/// The gate's hit windows of its M_8417 (the sheet's `gate.8417`: FinishGM SCGM_PORTAL with interval 0.1, FinishGM2
/// SCGM_PORTAL2) on its wepA_skt (`track.gate.8417.*`, the gate's actor space), growing from their begin
/// (`m_IncSize`). The interval re-opens the window with an empty hit list (KH3: the record's hit list is emptied every
/// interval): one shot per interval.
pub(crate) fn portal(cx: &mut Ctx) {
    let Some(mut g) = cx.pawn.combat.nano.gate else { return };
    let Some(row) = rows("gate").find(|r| r.clip == "8417") else { return };
    let t = cx.pawn.anim.time;
    for (k, (window, attack)) in row.windows.iter().zip(row.rows.iter()).enumerate().take(2) {
        let Some((a, b)) = window.split_once('-').and_then(|(a, b)| Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))) else { continue };
        let Some((attack_row, group)) = attack.split_once('@') else { continue };
        if t < a || t >= b || t < g.next[k] {
            continue;
        }
        let interval = v(row, k);
        let Some(shape) = shape_row("w_so174", group) else { continue };
        let Some(track) = rows("track").find(|r| r.clip == "8417" && r.logical == group && r.flags.first() == Some(&"gate")) else { continue };
        let Some(s) = sample(track, t, 6) else { continue };
        let (sin, cos) = g.yaw.to_radians().sin_cos();
        let at = g.position + Vec3::new(s[0] * cos - s[1] * sin, s[0] * sin + s[1] * cos, s[2]);
        let life = if interval > 0.0 { interval } else { b - a };
        let size = (radius(shape, t - a), v(shape, 1), v(shape, 2));
        let key = cx.pawn.combat.nano.key();
        let info = AttackCtx { kind: 0, ..cx.pawn.combat.attack_ctx };
        // (AUDIT-B F-NANO) the gate's hit effect (w_so174 TresAtkColl0 m_HitEffects: vw_so174_hit00, every group)
        let hit_effect = row.flags.first().and_then(|f| f.strip_prefix("hit:"));
        spawn_form_shot(cx, FormShot { key, group: if k == 0 { "FinishGM" } else { "FinishGM2" }, row: attack_row, position: at, yaw: g.yaw, size, life: life.min(b - t), hit_effect, hit_se: None, info, hit_list: Vec::new() });
        g.next[k] = if interval > 0.0 { t + interval } else { f32::MAX };
        cx.pawn.combat.nano.portal_shots += 1;
    }
    if let Some(live) = cx.pawn.combat.nano.gate.as_mut() {
        live.next = g.next;
    }
}

// ------------------------------------------------------------------------------------------------ (AUDIT-B F-NANO) the clips' extras

/// (AUDIT-B F-NANO) The sequences of Nano Gear's own clips (AnimSet8: 8401 StyleChange8, 8411 .. 8424, the dodges 8051 /
/// 8054, the Zone Connector 848x): only Nano Arms / Nano Gear play them.
pub fn is_nano_sequence(name: &str) -> bool {
    name.len() == 4 && (name.starts_with("84") || name == "8051" || name == "8054")
}

/// (AUDIT-B F-NANO) After the generic notify handlers of a pose tick (sora.rs): `TresAnimNotifyState_BpEvent` of Nano Gear's
/// clips -> `EventKind::AnimBpEvent` (the BpEvent notify 0x1405632A0 / 0x140565420 calls the owner's TresAnimInterface
/// vt+0x360 / +0x368 with the sequence name, m_EventID, m_Param -> the state-event blueprints' `ReceiveAnimNotifyStartBpEvent`
/// / `...EndBpEvent`: BP_TresPlayerStateEvent_camera_bx_key, the Nano Gear camera: kh_camera::forms_nano). As friend.rs does
/// for the team attacks; only Nano Gear's own sequences (no other form's events change).
pub(crate) fn on_notifies(cx: &mut Ctx, notifies: &crate::anim::Notifies) {
    use crate::anim::{NotifyClass, NotifyData, NotifyPhase};
    for e in notifies.events.iter().filter(|e| e.class() == NotifyClass::TresAnimNotifyStateBpEvent) {
        let seq = e.sequence_name();
        if !is_nano_sequence(seq) {
            continue;
        }
        let param = match e.def().data {
            NotifyData::Param { param } => param.unwrap_or(0),
            _ => 0,
        };
        let begin = match e.phase {
            NotifyPhase::Begin => true,
            NotifyPhase::End => false,
            _ => continue,
        };
        cx.pawn.event(crate::host::EventKind::AnimBpEvent { seq, param, begin });
    }
    clip_notifies(cx);
}

/// (AUDIT-B F-NANO) One live effect / sound of a `wfx` row.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ClipFx {
    row: &'static FormNanoRow,
    /// Who owns it: the weapon's play (Sora's play id), the gate's clip, or a fist (its key).
    owner: FxOwner,
    handle: FxHandle,
    /// It ends at its row's end time (a state notify); else it lives on its own (a one-shot) and is only moved.
    state: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FxOwner {
    Weapon(u32),
    Gate(&'static str),
    Fist(u16),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FxHandle {
    Effect(crate::host::EffectHandle),
    Sound(crate::host::SoundHandle, f32),
}

// The `wfx` rows' values: [GroupID, ScaleOffset xyz, FadeIn, FadeOut, StartDelayTime, then per frame x y z pitch yaw roll];
// a sound row: [FadeOutDuration, then per frame x y z 0 0 0].
const FX_HEAD: usize = 7;
const SOUND_HEAD: usize = 1;

fn fx_window(row: &FormNanoRow) -> (f32, f32) {
    row.windows.first().and_then(|w| w.split_once('-')).and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).unwrap_or((0.0, 0.0))
}

/// (AUDIT-B W-VFX2) A TypeDataSQEX_Thunder row's host-resolved anchor (`kb:w_so172:<unit socket>:<socket>[:to<EndSocketName>]`,
/// flags[8], tools/sora_form_nano.js via tools/lib/wnotify_rows.js `thunderAnchor`): KH3's thunder tick (0x144250690) reads
/// the beam's ends (the bone list R_wep / L_wep, R_y_himo01 ..) from the mesh the component hangs on, so the row names it.
/// None: an ordinary row (the FK track).
pub fn fx_anchor(row: &FormNanoRow) -> Option<&'static str> {
    row.flags.iter().skip(8).copied().find(|f| f.starts_with("kb:"))
}

fn is_sound(row: &FormNanoRow) -> bool {
    row.flags.get(1) == Some(&"sound")
}

/// The row's transform at clip time `t` in its owner's actor space: (location, (pitch, yaw, roll)).
fn fx_place(row: &FormNanoRow, t: f32) -> (Vec3, Vec3) {
    let head = if is_sound(row) { SOUND_HEAD } else { FX_HEAD };
    let frames = (row.values.len().saturating_sub(head)) / 6;
    if frames == 0 {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    let track = FormNanoRow { values: &row.values[head..], ..*row };
    let s = sample(&track, t, 6).unwrap_or_else(|| vec![0.0; 6]);
    // the rotator: the nearest frame's (no angle blend across +-180)
    let f = if row.length > 0.0 { ((t / row.length).clamp(0.0, 1.0) * (frames - 1) as f32).round() as usize } else { 0 };
    let r = &row.values[head + f.min(frames - 1) * 6 + 3..head + f.min(frames - 1) * 6 + 6];
    (Vec3::new(s[0], s[1], s[2]), Vec3::new(r[0], r[1], r[2]))
}

/// An owner's actor (location, yaw) now; None = gone.
fn owner_actor(cx: &Ctx, owner: FxOwner) -> Option<(Vec3, f32)> {
    match owner {
        FxOwner::Weapon(_) => Some((cx.pawn.cmc.location, cx.pawn.yaw)),
        FxOwner::Gate(_) => cx.pawn.combat.nano.gate.map(|g| (g.position, g.yaw)),
        FxOwner::Fist(key) => cx.pawn.combat.nano.rockets.iter().find(|r| r.key == key && !r.dead).map(|r| (r.position, r.forward.y.atan2(r.forward.x).to_degrees())),
    }
}

/// The world transform of a row at `t` for an actor at `at` turned `yaw` (an absolute rotation keeps the row's).
fn fx_world(row: &FormNanoRow, t: f32, (at, yaw): (Vec3, f32)) -> (Vec3, Vec3) {
    let (p, r) = fx_place(row, t);
    let (sin, cos) = yaw.to_radians().sin_cos();
    let world = at + Vec3::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos, p.z);
    let rot = if row.flags.get(4) == Some(&"abs") { r } else { Vec3::new(r.x, r.y + yaw, r.z) };
    (world, rot)
}

fn fx_start(cx: &mut Ctx, row: &'static FormNanoRow, owner: FxOwner, t: f32) {
    let Some(actor) = owner_actor(cx, owner) else { return };
    let state = row.flags.get(2) == Some(&"state");
    let (at, rotation) = fx_world(row, t, actor);
    let handle = if is_sound(row) {
        let place = match owner {
            // the weapon rides on Sora: its sound follows him
            FxOwner::Weapon(_) => {
                let (p, _) = fx_place(row, t);
                crate::host::Attach::Sora { bone: None, offset: p }
            }
            _ => crate::host::Attach::World(at),
        };
        let h = cx.play_sound(crate::host::SoundPlay { id: crate::host::SoundId(crate::notify::notify_asset_id(row.logical)), name: row.logical, at: place, volume: 1.0, pitch: 1.0, looping: state, spatial: true });
        FxHandle::Sound(h, v(row, 0))
    } else {
        // (AUDIT-B W-VFX2) a beam row is spawned on w_so172's own mesh (`kb:` anchor, flags[8]): the host follows the socket and
        // resolves the beam's ends there (no per-tick transform from here)
        let (place, rotation) = match fx_anchor(row) {
            Some(anchor) => (crate::host::Attach::Sora { bone: Some(anchor), offset: Vec3::ZERO }, Vec3::ZERO),
            None => (crate::host::Attach::World(at), rotation),
        };
        let h = cx.spawn_effect(crate::host::EffectSpawn { id: crate::host::EffectId(crate::notify::notify_asset_id(row.logical)), name: row.logical, at: place, rotation, scale: v(row, 1), persistent: state });
        // (AUDIT-B W-FIN) the row's FadeIn / StartDelayTime (values [4], [6]) and the FADEOUT end type's FadeOut (values [5])
        let timing = crate::host::EffectTiming { fade_in: v(row, 4), fade_out: if row.flags.get(5) == Some(&"FADEOUT") { v(row, 5) } else { 0.0 }, start_delay: v(row, 6) };
        if !timing.is_zero() {
            cx.host.effects.timing(h, timing);
        }
        FxHandle::Effect(h)
    };
    cx.pawn.combat.nano.clip_fx_started += 1;
    // a one-shot placed (ToAttach false) is left alone; everything else is followed
    if state || (row.flags.get(3) == Some(&"attach") && !is_sound(row)) {
        cx.pawn.combat.nano.clip_fx.push(ClipFx { row, owner, handle, state });
    }
}

fn fx_stop(cx: &mut Ctx, fx: &ClipFx) {
    match fx.handle {
        // LOOPEND (the clips' end type) = the emitters finish their loop: not immediate; NONE / others the same here (the
        // host's own endless rule ends a loop-forever one)
        FxHandle::Effect(h) => cx.stop_effect(h, false),
        FxHandle::Sound(h, fade) if fx.state => cx.stop_sound(h, fade),
        FxHandle::Sound(..) => {}
    }
}

/// The notify windows of the weapon's clip (Sora's sequence: the weapon plays its own clip of it while its form part is
/// shown: Nano Arms, the Zone Connector's form) and of the gate's clip this tick: their effects / sounds start, follow
/// their sockets, and end with their window, their clip or their owner.
fn clip_notifies(cx: &mut Ctx) {
    // ---- the weapon: Sora's body sequence and this tick's notify window (the rows are w_so172's own clips: only Nano
    // Gear's sequences, which only Nano Arms / its Zone Connector play)
    let req = cx.pawn.anim_ticked;
    let weapon_on = is_nano_sequence(req.body_samples()[0].0);
    let seq = req.body_samples()[0].0;
    let play = req.play_id;
    let (from, to) = req.notify_window;
    let fresh_weapon = cx.pawn.combat.nano.weapon_clock.is_none_or(|(p, _)| p != play);
    cx.pawn.combat.nano.weapon_clock = Some((play, req.time));
    // ---- the gate: its clip and clock
    let gate = cx.pawn.combat.nano.gate.and_then(|g| gate_sequence(g.clip).map(|s| (s, g.time)));
    let gate_from = match (cx.pawn.combat.nano.gate_clock, gate) {
        (Some((c, t)), Some((s, _))) if c == s => t,
        _ => -1.0,
    };
    cx.pawn.combat.nano.gate_clock = gate;
    // ---- what ends: an owner gone or replaced (the weapon's play changed, the gate's clip changed), a window over
    let live = std::mem::take(&mut cx.pawn.combat.nano.clip_fx);
    let mut keep = Vec::new();
    for fx in live {
        let (alive, t) = match fx.owner {
            FxOwner::Weapon(p) => (weapon_on && p == play, req.time),
            FxOwner::Gate(c) => (gate.is_some_and(|(s, _)| s == c), gate.map_or(0.0, |(_, t)| t)),
            FxOwner::Fist(key) => (cx.pawn.combat.nano.rockets.iter().any(|r| r.key == key && !r.dead), 0.0),
        };
        let over = fx.state && !matches!(fx.owner, FxOwner::Fist(_)) && t >= fx_window(fx.row).1 - 1.0e-4;
        if !alive || over {
            if fx.state {
                fx_stop(cx, &fx);
            }
            continue;
        }
        // follow the socket (or the fist)
        // (AUDIT-B W-VFX2) a `kb:`-anchored row is followed by the host on the weapon's mesh
        if let (FxHandle::Effect(h), Some(actor), None) = (fx.handle, owner_actor(cx, fx.owner), fx_anchor(fx.row)) {
            let (at, rotation) = fx_world(fx.row, t, actor);
            cx.host.effects.transform(h, at, rotation, Vec3::new(v(fx.row, 1), v(fx.row, 2), v(fx.row, 3)));
        }
        keep.push(fx);
    }
    keep.append(&mut cx.pawn.combat.nano.clip_fx);
    cx.pawn.combat.nano.clip_fx = keep;
    // ---- what starts this tick
    if weapon_on && is_nano_sequence(seq) && to > from {
        let lo = if fresh_weapon && from <= 0.0 { from - 1.0e-4 } else { from };
        // UE's notify query (GetAnimNotifies over the tick's range): an instant one whose time is in the range; a STATE one
        // whose [begin, end) overlaps it and that is not running yet (a clip started past its begin, as the hookshot's
        // M_8424 from (1000 - dxy) x 0.000333 s, begins its states at once)
        let running: Vec<*const FormNanoRow> = cx.pawn.combat.nano.clip_fx.iter().filter(|f| f.owner == FxOwner::Weapon(play)).map(|f| f.row as *const FormNanoRow).collect();
        let start: Vec<&'static FormNanoRow> = rows("wfx")
            .filter(|r| r.flags.first() == Some(&"weapon") && r.clip == seq)
            .filter(|r| {
                let (b, e) = fx_window(r);
                if r.flags.get(2) == Some(&"state") && fresh_weapon {
                    b <= to && e > from.max(0.0) && !running.contains(&(*r as *const FormNanoRow))
                } else {
                    b > lo && b <= to
                }
            })
            .collect();
        for r in start {
            fx_start(cx, r, FxOwner::Weapon(play), to);
        }
    }
    if let Some((s, t)) = gate {
        let lo = if gate_from < 0.0 { -1.0e-4 } else { gate_from };
        let start: Vec<&'static FormNanoRow> = rows("wfx").filter(|r| r.flags.first() == Some(&"gate") && r.clip == s).filter(|r| fx_window(r).0 > lo && fx_window(r).0 <= t).collect();
        for r in start {
            fx_start(cx, r, FxOwner::Gate(s), t);
        }
    }
}

/// (AUDIT-B F-NANO) The flying fist's own effects (TresProjGM_RocketPunch: m_AttachData ead_so170_com0, group 1620): started
/// with the fist, carried by it, loop-ended with it (`clip_notifies` moves and ends them).
fn fist_fx(cx: &mut Ctx, key: u16) {
    for r in rows("wfx").filter(|r| r.flags.first() == Some(&"fist")) {
        fx_start(cx, r, FxOwner::Fist(key), 0.0);
    }
}

// ------------------------------------------------------------------------------------------------ for hosts and INT

/// A Rocket Fist as a host draws it (w_so173 is not in the pack: its effect / a stand-in).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RocketView {
    pub stage: u8,
    pub position: Vec3,
    pub forward: Vec3,
    pub age: f32,
}

impl NanoState {
    pub fn rocket_views(&self) -> impl Iterator<Item = RocketView> + '_ {
        self.rockets.iter().filter(|r| !r.dead).map(|r| RocketView { stage: r.stage, position: r.position, forward: r.forward, age: r.age })
    }
}

/// The gate's clip of a logical name (w_so174's own set: M_8416 -> 8416 ...).
pub fn gate_sequence(logical: &str) -> Option<&'static str> {
    rows("gate").find(|r| r.logical == logical).map(|r| r.clip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::AnimId;

    fn din(combo: i32, last: u8, reach: u8, n: i32) -> GmDecideIn {
        GmDecideIn { combo, reach, last_kind: last, dist: 150.0, dz: 0.0, target_high: false, air: false, num_enemies: 1, finish_count: n }
    }

    /// A chain of decisions in a fixed situation (each kind the next one's previous kind, as pawn+0xE10 carries it).
    fn chain(base: GmDecideIn) -> Vec<u8> {
        let (mut last, mut reach, mut n, mut out) = (0u8, 0u8, 0, Vec::new());
        for combo in 0..8 {
            let d = decide(&GmDecideIn { combo, last_kind: last, reach, finish_count: n, ..base });
            out.push(d.kind);
            (last, reach, n) = (d.kind, d.reach, d.finish_count);
            if d.finish_end {
                break;
            }
        }
        out
    }

    #[test]
    fn the_combo_of_combo_gm() {
        assert_eq!((max_combo(), finishers()), (6, 3), "0x141433570 / 0x141433A60 case 0x86");
        // Ground, one near target: the blade 1, 2, 3, then the crash, the stamp, the Rocket Fist.
        assert_eq!(chain(din(0, 0, 0, 0)), [1, 2, 3, 0xB5, 0xB7, 0xB9]);
        // In the air: 0x10, 0x17 (0x10 -> 8, lifted), 0x12, then the drill, the yo-yos, the Rocket Fist.
        assert_eq!(chain(GmDecideIn { air: true, ..din(0, 0, 0, 0) }), [0x10, 0x17, 0x12, 0xB6, 0xB8, 0xB9]);
        // A crowd (reach 2): 3 first.
        assert_eq!(chain(GmDecideIn { num_enemies: 3, ..din(0, 0, 0, 0) }), [3, 2, 3, 0xB5, 0xB7, 0xB9]);
        // The finish end on the third finisher.
        let d = decide(&din(5, 0xB7, 0, 2));
        assert!(d.finish && d.finish_end && d.kind == 0xB9);
    }

    #[test]
    fn the_situation_kinds() {
        // Far: the hookshot (reach 1), not twice in a row: after it the approach 2.
        let d = decide(&GmDecideIn { dist: 550.0, ..din(0, 0, 0, 0) });
        assert_eq!((d.kind, d.reach), (kind::HOOK, 1));
        assert_eq!(decide(&GmDecideIn { dist: 550.0, ..din(1, kind::HOOK_END, 1, 0) }).kind, 2);
        // Above: the rise with its fixed DirZ; not twice.
        let d = decide(&GmDecideIn { dz: 250.0, target_high: true, ..din(0, 0, 0, 0) });
        assert_eq!((d.kind, d.fixed_dir_z), (kind::RISE, Some(crate::combat::tables::P.decide_rise_speed)));
        assert_eq!(decide(&GmDecideIn { dz: 250.0, target_high: true, ..din(1, kind::RISE, 0, 0) }).kind, 1 + 0x0F, "after the rise: 1, lifted (target high)");
        // Below: the fall.
        assert_eq!(decide(&GmDecideIn { dz: -200.0, air: true, ..din(0, 0, 0, 0) }).kind, kind::FALL);
        assert_eq!(decide(&GmDecideIn { dz: -200.0, air: true, ..din(1, kind::FALL, 0, 0) }).kind, 0x10);
    }

    #[test]
    fn the_sheet_carries_the_form() {
        for k in [0xB5u8, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD] {
            let r = attack_row(k, false).unwrap_or_else(|| panic!("{k:#x}"));
            assert!(AnimId::from_name(r.clip).is_some(), "{}: {} not in the animation table", r.id, r.clip);
            assert!(r.air_mode, "{}: every GM case breaks into the air mode", r.id);
        }
        assert_eq!(attack_row(kind::COUNTER, false).map(|r| r.clip), Some("MagicCast"));
        assert_eq!(attack_row(kind::COUNTER, true).map(|r| r.clip), Some("MagicAirCast"));
        assert_eq!(replaced_clip("1301"), Some("8301"));
        assert_eq!(replaced_clip("DodgeRollB"), Some("8051"));
        assert_eq!(replaced_clip("M_9999"), None);
        for c in ["8301", "8411", "8051", "8424"] {
            assert!(AnimId::from_name(c).is_some(), "{c}");
        }
        let rocket = proj("BX_ROCKET", 1).expect("the flying fist");
        assert_eq!((v(rocket, SPEED), v(rocket, LIFE), v(rocket, TURN_MAX)), (2000.0, 5.0, 180.0));
        assert!(fixed_group(CHANGE_GM, "StampL") && !fixed_group(CHANGE_GM, "CL_RStickL") && !fixed_group(0, "StampL"));
        assert_eq!(gate_sequence("M_8417"), Some("8417"));
    }

    #[test]
    fn the_tracks_sample_between_frames() {
        let row = rows("track").find(|r| r.clip == "8423" && r.logical == "YO_LSphere").expect("the hookshot's yo-yo");
        // The yo-yo is thrown about 9.5 m ahead at 0.67 s.
        let s = sample(row, 0.67, 6).unwrap();
        assert!(s[0] > 800.0 && s[1].abs() < 100.0, "{s:?}");
        let shape = shape_row("w_so172_Wep", "BD_FixImpactSS").unwrap();
        assert_eq!((radius(shape, 0.0), radius(shape, 1.0)), (150.0, 250.0));
    }

    /// (F-SHAPES) StampL and SwingM are tres_colltest01 hulls: SwingM upright at Sora's `center` (LOCAL), radius 550 and half
    /// height 200; StampL on the hammer head.
    #[test]
    fn the_convex_groups_are_hulls() {
        use crate::combat::hull::Volume;
        let swing = volume_shapes("SwingM", "8420", 0.8, Vec3::ZERO, 0.0).expect("SwingM");
        let [Volume::Hull(h)] = swing.as_slice() else { panic!("{swing:?}") };
        assert!((h.scale - Vec3::new(5.5, 5.5, 4.0)).length() < 1.0e-3 && h.rotation.angle_between(glam::Quat::IDENTITY) < 1.0e-4, "{h:?}");
        assert!(h.distance(h.centre + Vec3::new(540.0, 0.0, 190.0)) == 0.0 && h.distance(h.centre + Vec3::Z * 260.0) > 50.0);
        let stamp = volume_shapes("StampL", "8413", 1.5, Vec3::ZERO, 0.0).expect("StampL");
        assert!(matches!(stamp.as_slice(), [Volume::Hull(s)] if s.centre.x > 200.0), "{stamp:?}");
        assert!(rows("track").all(|r| r.logical != "StampL" && r.logical != "SwingM"), "no sphere track left for them");
    }

    /// (F-POLISH) The flying fist's level hit (HandleImpact 0x14147AD80, m_CheckMapHitNormal 60 of TresProjGM_RocketPunch):
    /// a glancing wall hit goes on along the wall with its speed, a head-on one stops it; the blast stage has no check.
    #[test]
    fn the_rocket_fist_glances_off_walls() {
        let fist = proj("BX_ROCKET", 1).expect("TresProjGM_RocketPunch");
        let angle = v(fist, MAP_NORMAL);
        assert_eq!(angle, 60.0);
        assert_eq!(v(proj("BX_ROCKET", 0).expect("RocketPunch0"), MAP_NORMAL), 0.0);
        let wall = Vec3::new(-1.0, 0.0, 0.0);
        // The head-on part -(dir . normal) against cos 60 = 0.5.
        let into = |deg: f32| Vec3::new(deg.to_radians().sin(), deg.to_radians().cos(), 0.0) * 2000.0;
        assert_eq!(map_impact(Vec3::new(2000.0, 0.0, 0.0), wall, angle), None, "head on");
        let slid = map_impact(into(20.0), wall, angle).expect("20 degrees into the wall (0.34): it glances");
        assert!((slid - Vec3::new(0.0, 2000.0, 0.0)).length() < 1.0e-2, "{slid:?}");
        assert_eq!(map_impact(into(45.0), wall, angle), None, "45 degrees into the wall (0.71): it stops");
        assert_eq!(map_impact(Vec3::new(500.0, 100.0, 0.0), wall, 0.0), None, "no check: every hit stops");
    }
}
