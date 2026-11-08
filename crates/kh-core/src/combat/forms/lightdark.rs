//! (weapons F-LIGHTDARK) KH3 Re Mind's Light Form (`form.LF`, CHANGE_LF 0xDD, Oathkeeper), Dark Form (`form.DF`, CHANGE_DF
//! 0xDE, Oblivion) and their second stage Double Form (`form.TF` CHANGE_TF 0xDF / `form.TF2` CHANGE_TF2 0xE0: Oathkeeper and
//! Oblivion together): their form package (documentation/KH3_WEAPONS_PLAN.md 1.3 / 4; build/sora_tmp/weapons/F-LIGHTDARK/
//! STATE.md). Every number that is not an address comes from `sheets/port/sora-form-lightdark.json`
//! (tools/sora_form_lightdark.js).
//!
//! WHAT KH3 DOES (decompiled C in re/decomp; vtables read from base/KH3.exe):
//! - THE ATTACK CLASSES (pawn slot 1337 0x141432F00): 0xDD -> COMBO_LF (159), 0xDE -> COMBO_DF (160), 0xDF / 0xE0 -> COMBO_TF
//!   (161). They are the Attack class with another Decide (slot 122: 0x141362300 / 0x1413606F0 / 0x141363CE0) and, for
//!   ComboDF, slot 136 0x141352160 (Dark Break's warp) and, for ComboTF, the anim interface's shot handler 0x1413590B0: the
//!   port's Attack state carries the class (`Attack::remind`, states/combo_lf.rs) like ComboLimit's `limit`.
//! - THE COMBO (`decide`): Light / Dark Form decide the base kinds exactly as the base class (rules 2 .. 18 and the launch,
//!   the wall-run kinds), MaxCombo 6, Finishers 3 (0x141433570 / 0x141433A60), and their finishers by the finish count:
//!   0xCD Zantetsuken, 0xCE Finishing Leap, 0xCF Black Hole (Light) / 0xD0 Dark Break (Dark); in the air 0xD1 Triple Edge,
//!   0xD2 Spinning Rush, 0xD3 Spiral Charge. Double Form has its own chain 0xD4 .. 0xD9 (+4 in the air), the rise 0x1F, the
//!   dive 0xDE (Spinning Strike), the launchers 0xDF (Launching Thrust) / 0xE0 (Groundbreaker) and the finishers 0xDA Double
//!   Blast, 0xDB Flash Charge, 0xDC Mirage Thrust, 0xDD Equilibrium Burst (air).
//! - THE CASES of PlayAttack 0x1413378B0 (the sheet's `attack.*` rows): clips of AnimSet22 / AnimSet23, root lengths, the
//!   effect group 0x834 (Light) / 0x898 (Dark), the air mode.
//! - THE REPRISALS: a guard counter in these forms is kind 0x25 (TryAttack 0x141437330): Light Burst / Dark Burst (the cast
//!   clip with the BARRIERCRACKER projectile of the form's projectile set) and Dual Smasher (M_N426); the window opens without
//!   the counter abilities and shows 0xD7 / 0xD8 / 0xD9 (FUN_1414145C0; actions.rs).
//! - LIGHT / DARK / DUAL GUARD: the Guard state's form tick (slot 119 0x14139F770): Reflec, all directions (states/guard.rs).
//! - SONIC DODGE: DodgeRollB of AnimSet22 (M051) with the effect group 0x834 / 0x898 (0x141368880; states/dodge_roll.rs).
//! - FINISH_TF / FINISH_TF2 (Ultimate Finish): STYLEFINISH slot 166 0x1413E0860 (`finish_tick`).
//! - THE TRANSFORMATION: StyleChange's three Second Form clips with the wear forms 8 / 9 / 10 (P3's framework; AnimSet23
//!   replaces StyleChangeEnd in Double Form).

use super::FormMoveset;
use crate::anim::{NotifyData, SEQUENCES, SOCKET_VFX_CURE};
use crate::combat::hit::{spawn_form_shot, FormShot, Shot, FORM_SHOT_SEQUENCE};
use crate::host::{Attach, EffectId, EffectSpawn, TargetId};
use crate::notify::notify_asset_id;
use crate::pawn::Ctx;
use glam::Vec3;

// The rows (sheets/port/sora-form-lightdark.json): `FormLightdarkRow`, `FORM_LIGHTDARK`, `FLD_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_lightdark.rs"));

/// CHANGE_LF: Light Form (Oathkeeper).
pub const CHANGE_LF: i32 = 0xDD;
/// CHANGE_DF: Dark Form (Oblivion).
pub const CHANGE_DF: i32 = 0xDE;
/// CHANGE_TF: Double Form out of Light Form.
pub const CHANGE_TF: i32 = 0xDF;
/// CHANGE_TF2: Double Form out of Dark Form.
pub const CHANGE_TF2: i32 = 0xE0;
/// FINISH_TF / FINISH_TF2: Ultimate Finish (CanStyleFinish 0x14142E030: 0xE1 only in 0xDF, 0xE2 only in 0xE0).
pub const FINISH_TF: i32 = 0xE1;
pub const FINISH_TF2: i32 = 0xE2;
/// The reprisal kind of a guard counter in these forms (PlayAttack case 0x25).
pub const KIND_BARRIER_CRACKER: u8 = 0x25;
/// Dark Break (ComboDF's third finisher, with the warp).
pub const KIND_DARK_BREAK: u8 = 0xD0;
/// The key range of this package's moving shots (`Window::key` = (FORM_SHOT_SEQUENCE, key)): 0x4C00 ..= 0x4CFF.
const KEY_BASE: u16 = 0x4C00;

/// The package: four forms, the two Ultimate Finishes.
pub(crate) const MOVESET: FormMoveset = FormMoveset {
    name: "lightdark",
    forms: &["form.LF", "form.TF", "form.DF", "form.TF2"],
    finishes: &[FINISH_TF, FINISH_TF2],
    finish_begin: crate::states::combo_tf::finish_begin,
    finish_tick: crate::states::combo_tf::finish_tick,
    replace_clip_in,
    hit_shape,
    shot_motion,
    ..FormMoveset::NONE
};

/// The attack class of a form (pawn slot 1337 0x141432F00).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// COMBO_LF (159): Light Form.
    Lf,
    /// COMBO_DF (160): Dark Form.
    Df,
    /// COMBO_TF (161): Double Form (both commands).
    Tf,
}

impl Class {
    /// The value `Attack::remind` carries (0 = none).
    pub fn value(self) -> u8 {
        match self {
            Class::Lf => 1,
            Class::Df => 2,
            Class::Tf => 3,
        }
    }

    pub fn from_value(v: u8) -> Option<Class> {
        match v {
            1 => Some(Class::Lf),
            2 => Some(Class::Df),
            3 => Some(Class::Tf),
            _ => None,
        }
    }

    /// The sheet's form key.
    pub fn key(self) -> &'static str {
        match self {
            Class::Lf => "LF",
            Class::Df => "DF",
            Class::Tf => "TF",
        }
    }
}

/// The class of a form command, None outside these forms.
pub fn class_of(cmd: i32) -> Option<Class> {
    match cmd {
        CHANGE_LF => Some(Class::Lf),
        CHANGE_DF => Some(Class::Df),
        CHANGE_TF | CHANGE_TF2 => Some(Class::Tf),
        _ => None,
    }
}

fn rows(what: &'static str) -> impl Iterator<Item = &'static FormLightdarkRow> {
    FORM_LIGHTDARK.iter().filter(move |r| r.what == what)
}

fn row(id: &str) -> Option<&'static FormLightdarkRow> {
    FORM_LIGHTDARK.iter().find(|r| r.id == id)
}

/// A number of a row (`values[k]`).
fn value(id: &str, k: usize) -> Option<f32> {
    row(id).and_then(|r| r.values.get(k).copied())
}

/// The row has a flag (`flags`).
pub(crate) fn has_flag(r: &FormLightdarkRow, flag: &str) -> bool {
    r.flags.contains(&flag)
}

/// The PlayAttack case of `kind` in a class (`attack.<form>.<kind>`; kind 0x25 in the air: its AirBarrierCracker row).
pub fn attack_row(class: Class, kind: u8, air: bool) -> Option<&'static FormLightdarkRow> {
    let key = class.key();
    let mut found = rows("attack").filter(move |r| r.form == key && r.code == i32::from(kind));
    let first = found.next()?;
    let second = found.next();
    match second {
        Some(alt) if air == has_flag(alt, "air_variant") => Some(alt),
        Some(_) | None => Some(first).filter(|r| !has_flag(r, "air_variant") || air),
    }
}

/// m_ReplaceAnimSets: the clip (animation-table row) a logical name plays as in the held form (Light / Dark Form: AnimSet22;
/// Double Form: AnimSet23); None = the base set's.
pub fn replaced_clip(cmd: i32, logical: &str) -> Option<&'static str> {
    let set = match class_of(cmd)? {
        Class::Lf | Class::Df => "LD",
        Class::Tf => "TF",
    };
    rows("anim").find(|r| r.form == set && r.logical == logical && r.clip != "-").map(|r| r.clip)
}

/// The package's `replace_clip_in` (every play of the anim player goes through it while a form is held).
fn replace_clip_in(cmd: i32, logical: &str) -> Option<&'static str> {
    replaced_clip(cmd, logical)
}

/// MaxCombo (pawn slot 928 0x141433570) in these forms.
pub fn max_combo() -> i32 {
    value("rule.max_combo", 0).map_or(6, |v| v as i32)
}

/// Finishers (pawn slot 1253 0x141433A60) in these forms.
pub fn finishers() -> i32 {
    value("rule.finishers", 0).map_or(3, |v| v as i32)
}

/// The reaction command of the guard counter window in the held form (FUN_1414145C0: controller+0x8A4 = 0xD7 / 0xD8 /
/// 0xD9), None outside these forms.
pub fn counter_command(cmd: i32) -> Option<i32> {
    let class = class_of(cmd)?;
    value(&format!("rule.counter_command.{}", class.key()), 0).map(|v| v as i32)
}

/// The form guard of the held form (Guard slot 119 0x14139F770): (Reflec, AirReflec, ReflecHit, AirReflecHit clips, the
/// effect group); None outside these forms. "-" = the set has no such clip (KH3 then plays nothing new).
pub fn guard(cmd: i32) -> Option<FormGuard> {
    let class = class_of(cmd)?;
    let r = row(&format!("guard.{}", class.key()))?;
    let flag = |name: &str| r.flags.iter().find_map(|f| f.strip_prefix(name).and_then(|x| x.strip_prefix('='))).unwrap_or("-");
    Some(FormGuard { start: r.clip, air_start: flag("AirReflec"), hit: flag("ReflecHit"), air_hit: flag("AirReflecHit"), group: r.group })
}

/// See [`guard`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormGuard {
    pub start: &'static str,
    pub air_start: &'static str,
    pub hit: &'static str,
    pub air_hit: &'static str,
    pub group: i32,
}

/// The dodge's effect group (DodgeRoll 0x141368880: +0x90 = 0x834 in 0xDD, 0x898 in 0xDE), None = the default.
pub fn dodge_group(cmd: i32) -> Option<i32> {
    let class = class_of(cmd)?;
    row(&format!("dodge.{}", class.key())).map(|r| r.group)
}

// ------------------------------------------------------------------------------------------------ the decision

/// The decision of the three classes. `i` is what PreDecide measured (the base class's input: MaxCombo / Finishers are the
/// forms' 6 / 3); `a` the abilities; `wall_side` the wall-run request (+0x1EC bit 3 with its side +0x1C8; the kinds 0x31 ..
/// 0x34 are attack.rs's rule 4, made by the caller). Light / Dark Form (0x141362300 / 0x1413606F0) are the base decision
/// with their own finisher step; Double Form (0x141363CE0) has its own chain.
pub fn decide(class: Class, i: &crate::combat::decide::DecideIn, a: &crate::combat::decide::ComboAbilities) -> RemindDecision {
    match class {
        Class::Lf | Class::Df => decide_ld(class, i, a),
        Class::Tf => decide_tf(i),
    }
}

/// What a decision leaves (the base `Decision` and Dark Break's warp request).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RemindDecision {
    pub d: crate::combat::decide::Decision,
    /// ComboDF Decide for 0xD0: +0x1E1 = 1 (the warp), +0x25C = 0.5 (the invincible time), the homing target = the
    /// controller's (vt+0x16F0) when it has none.
    pub warp: bool,
}

/// ComboLF / ComboDF Decide (0x141362300 / 0x1413606F0): rules 2 .. 18 and the launch are the base class's (the code is
/// the same line for line, with `1 < num_enemies` for the crowd and the launch stocks set to 1 / 1 at combo 0 by pawn
/// vt+0x27B0(1) / vt+0x27C0(1)); the finisher step: finish count 1 -> 0xCE, 2 -> 0xCF (Light) / 0xD0 (Dark), else 0xCD;
/// in the air (+0x142) 0xD2 / 0xD3 / 0xD1; the air-down finisher 0x0E first as in the base; the ability upgrades are applied
/// to the normal kinds only (the finishers are none of 1 2 3 8 0xF 0x10 0x11 0x12).
fn decide_ld(class: Class, i: &crate::combat::decide::DecideIn, a: &crate::combat::decide::ComboAbilities) -> RemindDecision {
    let input = crate::combat::decide::DecideIn { endless: false, max_combo: max_combo(), finishers: finishers(), ..*i };
    let mut d = crate::combat::decide::decide(&input, a);
    if i.combo == 0 && !i.launch {
        d.stocks = (1, 1);
    }
    let mut warp = false;
    if d.finish && d.kind != 0x0E {
        let n = i.finish_count;
        d.kind = match (n, d.air, class) {
            (1, false, _) => 0xCE,
            (1, true, _) => 0xD2,
            (2, false, Class::Df) => KIND_DARK_BREAK,
            (2, false, _) => 0xCF,
            (2, true, _) => 0xD3,
            (_, false, _) => 0xCD,
            (_, true, _) => 0xD1,
        };
        warp = d.kind == KIND_DARK_BREAK;
    }
    RemindDecision { d, warp }
}

/// ComboTF Decide (0x141363CE0), literally:
///
/// ```text
/// combo 0: reach = dist > 400 ? 1 : (num_enemies > 1 ? 2 : 0); last = 0; stocks 1 / 1        (also after 0xDF / 0xE0)
/// launch:  stock0 == 1 && (stock1 != 1 || !air || dz >= 100) ? 0xDF (DirZ 1000 on the ground) : 0xE0; that stock = 2; combo 0
/// normal (combo < max - fin):
///   ground: target high && last != 0x1F && dist <= 400 -> 0x1F (the rise: DirZ fixed)          finish count 0
///   air:    dz < -100 && last != 0xDE && dist <= 400 -> 0xDE                                      finish count 0
///   last 0xD4 / 0xD8 -> reach 1 ? 0xD7 : 0xD5;  0xD5 / 0xD9 -> reach 1 ? 0xD4 : 0xD7;  0xD6 -> 0xD4;  0xD7 -> 0xD4 + (reach 1)
///   else first: reach 1 -> 0xD6, 2 -> 0xD7, else 0xD4;  else by combo % 3: 0 -> 0xD4 + reach, 1 -> 0xD5 - (reach != 0),
///   2 -> reach 2 ? 0xD5 : 0xD7
///   (air || (combo > 0 && target high)) and 0xD4 / 0xD5 -> + 4;  finish count 0
/// finisher: finish; finish end when n >= fin - 1 || combo >= max - 1; n 1 -> 0xDB, 2 -> 0xDC, else air ? 0xDD : 0xDA; n + 1
/// ```
fn decide_tf(i: &crate::combat::decide::DecideIn) -> RemindDecision {
    use crate::combat::decide::Decision;
    use crate::combat::tables::P;
    let mut d = Decision { kind: 0xD4, reach: i.reach, air: i.air, combo: i.combo, finish: false, finish_end: false, fixed_dir_z: None, finish_count: i.finish_count, stocks: i.stocks, last_kind_reset: false };
    let mut last = i.last_kind;
    let mut first = false;
    let reach_rule = || if i.dist > P.decide_far_dist { 1 } else if i.num_enemies > 1 { 2 } else { 0 };
    if i.combo == 0 {
        d.reach = reach_rule();
        first = true;
        last = 0;
        d.last_kind_reset = true;
        d.stocks = (1, 1);
    } else if matches!(last, 0xDF | 0xE0) {
        d.reach = reach_rule();
        first = true;
    }
    let (max, fin) = (max_combo(), finishers());
    if i.launch {
        let kind = if d.stocks.0 == 1 && (d.stocks.1 != 1 || !d.air || i.dz >= P.decide_launch_air_dz) { 0xDF } else { 0xE0 };
        if !d.air && kind == 0xDF {
            d.fixed_dir_z = Some(1000.0);
        }
        if kind == 0xDF {
            d.stocks.0 = 2;
        } else {
            d.stocks.1 = 2;
        }
        (d.combo, d.kind) = (0, kind);
        return RemindDecision { d, warp: false };
    }
    if i.combo < max - fin {
        if !d.air {
            if i.target_high && last != 0x1F && i.dist <= P.decide_far_dist {
                (d.kind, d.fixed_dir_z, d.finish_count) = (0x1F, Some(P.decide_rise_speed), 0);
                return RemindDecision { d, warp: false };
            }
        } else if i.dz < P.decide_air_below_dz && last != 0xDE && i.dist <= P.decide_far_dist {
            (d.kind, d.finish_count) = (0xDE, 0);
            return RemindDecision { d, warp: false };
        }
        let reach = d.reach;
        let m3 = i.combo.rem_euclid(3);
        let mut kind: u8 = match last {
            0xD4 | 0xD8 => if reach == 1 { 0xD7 } else { 0xD5 },
            0xD5 | 0xD9 => if reach == 1 { 0xD4 } else { 0xD7 },
            0xD6 => 0xD4,
            0xD7 => 0xD4 + u8::from(reach == 1),
            _ if first => match reach {
                1 => 0xD6,
                2 => 0xD7,
                _ => 0xD4,
            },
            _ => match m3 {
                0 => 0xD4 + reach,
                1 => 0xD5 - u8::from(reach != 0),
                _ => if reach == 2 { 0xD5 } else { 0xD7 },
            },
        };
        if (d.air || (i.combo > 0 && i.target_high)) && matches!(kind, 0xD4 | 0xD5) {
            kind += 4;
        }
        (d.kind, d.finish_count) = (kind, 0);
        return RemindDecision { d, warp: false };
    }
    let n = i.finish_count;
    d.finish = true;
    if n >= fin - 1 || i.combo >= max - 1 {
        d.finish_end = true;
    }
    d.kind = match n {
        1 => 0xDB,
        2 => 0xDC,
        _ if d.air => 0xDD,
        _ => 0xDA,
    };
    d.finish_count = n + 1;
    RemindDecision { d, warp: false }
}

// ------------------------------------------------------------------------------------------------ the hooks

/// Ultimate Finish's start point: FUN_1413F4EF0(his location, 300, bFallback 1, bGround 1, pawn), read to its end:
/// - a spot is FREE when a sphere of radius 300 resting on it (FUN_141403130: centre + 300 + 2 above the point, swept by
///   0.2 cm, blocking channels 0 / 1 / 0x12) meets nothing;
/// - his own spot first; else rings of 1 x 300 and 2 x 300 cm around it, 18 directions each from his yaw (0, +20, -20, +40,
///   -40 ... degrees), the first free one his capsule can reach (pawn vt+0x9D8: a sweep from his spot) - KH3 stops the
///   search after 8 ms of real time (QueryPerformanceCounter): not a rule a simulation can keep, the whole search runs;
/// - none: bFallback asks the navigation system for a point within 299 cm (pawn vt+0x2B00 / FUN_1413FD4E0): the port has no
///   navmesh, so he stays where he is.
///
/// The ground snap of bGround (FUN_14140AED0 / FUN_14140AD90) is the movement's own landing. Returns the point (feet).
pub(crate) fn finish_start_point(cx: &Ctx) -> Vec3 {
    const R: f32 = 300.0;
    let here = cx.pawn.cmc.location;
    // (AUDIT-C TF) FUN_141403130: a sphere of R whose centre is R + 2 cm above the point, swept 0.2 cm: free when it meets
    // nothing, a shape it STARTS inside included (UE reports it): the host's overlap query (`World::overlap_sphere`).
    let free = |p: Vec3| !cx.world.overlap_sphere(p + Vec3::Z * (R + 2.0), R);
    if free(here) {
        return here;
    }
    let p = *cx.pawn.params();
    let (radius, half) = (p.capsule_radius, p.capsule_half_height);
    let centre = Vec3::Z * p.capsule_center_z;
    for ring in 1..3 {
        for k in 0..18i32 {
            let step = ((k + 1) >> 1) as f32 * 20.0;
            let turn = if k == 0 { 0.0 } else if (k + 1) & 1 != 0 { -step } else { step };
            let a = (cx.pawn.yaw + turn).to_radians();
            let to = here + Vec3::new(a.cos(), a.sin(), 0.0) * (ring as f32 * R);
            // (the capsule standing on the floor: a floor contact is no obstacle)
            if free(to) && cx.world.sweep_capsule(here + centre + Vec3::Z * 2.0, to + centre + Vec3::Z * 2.0, radius, half).is_none_or(|h| !h.start_penetrating && h.normal.z > 0.7) {
                return to;
            }
        }
    }
    here
}

/// FINISH_TF / FINISH_TF2 sub 5: the hold after M_N425 (+0x2D8 = 0.5).
pub(crate) fn finish_hold() -> f32 {
    value("finish.TF.5", 0).unwrap_or(0.5)
}

// ------------------------------------------------------------------------------------------------ the hit shapes

/// The weapon of a hand in a class: (sheet owner of its TresAtkColl0 shapes, ATK_01.x): Light Form Oathkeeper (w_so200),
/// Dark Form Oblivion (w_so210), Double Form Oathkeeper right and Oblivion left (sora-keyblade-forms `units`).
fn weapon(class: Class, left: bool) -> (&'static str, f32) {
    let model = match (class, left) {
        (Class::Df, _) | (Class::Tf, true) => "w_so210",
        _ => "w_so200",
    };
    (model, value(&format!("weapon.{model}"), 0).unwrap_or(80.0))
}

/// The shape row of a group on a weapon (`shape.R.*` = w_so200_Wep, `shape.L.*` = w_so210_Wep).
fn shape_row(model: &str, group: &str) -> Option<&'static FormLightdarkRow> {
    let side = if model == "w_so210" { "L" } else { "R" };
    rows("shape").find(|r| r.form == side && r.logical == group)
}

/// The socket track of a sequence for a hand (`sock.R.<seq>` / `sock.L.<seq>`): (ATK_00, the weapon's x axis) at `time`,
/// actor space.
fn socket_at(seq: &str, left: bool, time: f32) -> Option<(Vec3, Vec3)> {
    let side = if left { "L" } else { "R" };
    let r = rows("sock").find(|r| r.form == side && r.clip == seq)?;
    let frames = usize::try_from(r.code).ok().filter(|n| *n > 0)?;
    let last = frames - 1;
    let f = if r.length > 0.0 { (time / r.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let at = |k: usize| {
        let (a, b) = (r.values.get(i * 6 + k).copied().unwrap_or(0.0), r.values.get(j * 6 + k).copied().unwrap_or(0.0));
        a + (b - a) * t
    };
    Some((Vec3::new(at(0), at(1), at(2)), Vec3::new(at(3), at(4), at(5)).normalize_or_zero()))
}

/// Time since the hit window of `group` began (the clip's TresAnimNotifyState_Attack around `time`).
fn window_time(sequence: usize, group: &str, time: f32) -> f32 {
    let Some(seq) = SEQUENCES.get(sequence) else { return 0.0 };
    seq.notifies
        .iter()
        .filter(|n| matches!(n.data, NotifyData::Attack { group: g, .. } if g == group) && n.time <= time + 1.0e-4)
        .map(|n| time - n.time)
        .fold(f32::INFINITY, f32::min)
        .clamp(0.0, 10.0)
}

/// The package's `hit_shape`: a hit window of a clip the forms play, on the held form's weapons (their own TresAtkColl0
/// groups and the KB_* sticks of Oathkeeper / Oblivion), from the sheet's per-frame sockets of both hands. None = not this
/// package's (no form of it held, or a clip without a track: the Kingdom Key's tracks of the pack apply).
fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let class = class_of(cx.pawn.combat.form.cmd)?;
    let seq = SEQUENCES.get(sequence)?.name;
    let left = class == Class::Tf && group.starts_with("L_");
    let (model, length) = weapon(class, left);
    let r = shape_row(model, group)?;
    shape_of(cx, r, seq, left, length, time, window_time(sequence, group, time))
}

/// A group a hit window opens on BOTH weapons of the held form: KH3 activates a group on every TresAtkColl component that has
/// it (the pawn's attack collision manager, strand C 8.2.1), so in Double Form a group of both w_so200_Wep and w_so210_Wep
/// (KB_Stick*, KB_Sphere*, Sphere*, ImpactM ...) hits from Oathkeeper AND Oblivion, each with its own hit list; R_* / L_*
/// are on one weapon only.
pub(crate) fn both_weapons(cmd: i32, group: &str) -> bool {
    class_of(cmd) == Some(Class::Tf) && shape_row("w_so200", group).is_some() && shape_row("w_so210", group).is_some()
}

/// (AUDIT-C TF) A hit window that is on Double Form's LEFT weapon (Oblivion, w_so210_Wep): an `L_*` group (`hit_shape`) or
/// the second activation of a group both weapons carry (key `notify | 0x8000` of a clip's window, `both_weapons`). Its HITn
/// alias plays from that weapon's hit set (KH3: the collision's weapon actor).
pub(crate) fn on_left_weapon(cmd: i32, group: &str, key: (u16, u16)) -> bool {
    let clip_window = key.0 != u16::MAX && key.0 != crate::combat::hit::FORM_SHOT_SEQUENCE;
    class_of(cmd) == Some(Class::Tf) && clip_window && (group.starts_with("L_") || key.1 & 0x8000 != 0)
}

/// The second weapon's (Oblivion's, the left hand's) shape of a group in Double Form (see [`both_weapons`]).
pub(crate) fn second_weapon_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let class = class_of(cx.pawn.combat.form.cmd).filter(|c| *c == Class::Tf)?;
    let seq = SEQUENCES.get(sequence)?.name;
    let (model, length) = weapon(class, true);
    let r = shape_row(model, group)?;
    shape_of(cx, r, seq, true, length, time, window_time(sequence, group, time))
}

/// A shape row placed: (end 1, end 2, radius), world.
fn shape_of(cx: &Ctx, r: &FormLightdarkRow, seq: &str, left: bool, length: f32, time: f32, since: f32) -> Option<(Vec3, Vec3, f32)> {
    let pawn = &*cx.pawn;
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    let world = |v: Vec3| pawn.cmc.location + Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z);
    let v = |k: usize| r.values.get(k).copied().unwrap_or(0.0);
    let flag = |k: usize| r.flags.get(k).copied().unwrap_or("None");
    // m_Size.x, growing by m_IncSize.x a second up to m_IncMaxSize.x (0x140649E40).
    let radius = if v(3) > 0.0 { (v(0) + v(3) * since).min(v(4).max(v(0))) } else { v(0) };
    let loc = Vec3::new(v(5), v(6), v(7));
    let (shape, socket1, socket2, attach) = (flag(0), flag(1), flag(2), flag(3));
    if attach == "OWNER_ATTACHED_MESH" {
        // Sora's own sockets (the pawn's mesh): `center` 91 up (hit.rs), `vfx_cure` (the pack's socket), `kh_root` his root.
        let base = match socket1 {
            "center" => Vec3::new(0.0, 0.0, 91.0),
            "vfx_cure" => Vec3::from(SOCKET_VFX_CURE),
            _ => Vec3::ZERO,
        };
        let p = base + loc;
        return Some(match shape {
            "CAPSULE1" => (world(p) - Vec3::Z * v(2), world(p) + Vec3::Z * v(2), radius),
            _ => (world(p), world(p), radius),
        });
    }
    let (a, dir) = socket_at(seq, left, time)?;
    let point = |name: &str| match name {
        "ATK_00" => Some(a),
        "ATK_01" => Some(a + dir * length),
        _ => None,
    };
    let p1 = point(socket1)?;
    Some(match shape {
        "CAPSULE2" => (world(p1), world(point(socket2).unwrap_or(p1)), radius),
        "CAPSULE1" => (world(p1) - Vec3::Z * v(2), world(p1) + Vec3::Z * v(2), radius),
        _ => (world(p1), world(p1), radius),
    })
}

// ------------------------------------------------------------------------------------------------ the projectiles

/// The live moving shots of this package (Joint Struggle: TresProjTF_JointStruggle0 .. 3 fly at InitialSpeed until
/// m_MaxLifeDistance).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LightDarkState {
    pub moving: Vec<Moving>,
    /// One actor's shapes: (its first shot key, the shape count).
    pub actors: Vec<(u16, u8)>,
    /// The next key (`KEY_BASE | next`).
    pub next: u8,
    /// (AUDIT-C TF) pawn+0x1560 / +0x155C (FUN_1414466F0): the point the next pawn tick puts him at (0x141448AA0's head:
    /// FUN_143EAC3D0, SetActorLocation). Ultimate Finish's sub 1 sets it to its start point (+0x2B4) as M_N425 begins.
    pub finish_warp: Option<Vec3>,
    /// (AUDIT-C TF) Ultimate Finish's +0x2B4: its start point (FUN_1413F4EF0's answer), kept from sub 0 for sub 1.
    pub finish_start: Vec3,
}

/// One moving shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moving {
    pub key: u16,
    pub origin: Vec3,
    pub velocity: Vec3,
    /// m_MaxLifeDistance, cm (0 = none).
    pub max: f32,
}

/// The projectile row of a form (`proj.<form>.<name>`): Light / Dark Form their own BARRIERCRACKER / SCDF_DARKBREAK,
/// Double Form the TF_* rows.
fn proj(class: Class, name: &str) -> Option<&'static FormLightdarkRow> {
    rows("proj").find(|r| r.form == class.key() && r.logical == name)
}

/// The pawn's projectile spawn (pawn vt+0x1450(name, location, location, direction, team, 0)) of a form projectile: one
/// shot per attack shape of its TresAtkColl0 (`m_Size` / `m_IncSize` / `m_IncMaxSize`, an upright CAPSULE1 with its half
/// length `m_Size.Z` along the actor's up axis, `m_RelativeLocation1` in the actor's frame, the default row, the hit
/// effect / SE), all of one actor sharing its hit list; the counters of the attack that fired it (shot vt+0x1130 / +0x1140 /
/// +0x1120); the particle and the fire effect at its root; a projectile with a speed flies (`shot_motion`). False = the form
/// has no such projectile.
pub(crate) fn spawn_projectile(cx: &mut Ctx, class: Class, name: &str, at: Vec3, dir: Vec3) -> bool {
    let Some(r) = proj(class, name) else { return false };
    let v = |k: usize| r.values.get(k).copied().unwrap_or(0.0);
    let flag = |k: usize| r.flags.get(k).copied().filter(|f| *f != "-");
    let d = if dir.length_squared() > 1.0e-8 { dir.normalize() } else { Vec3::X };
    let yaw = d.y.atan2(d.x).to_degrees();
    let pitch = d.z.clamp(-1.0, 1.0).asin();
    let (life, speed, max) = (v(0), v(1), v(2));
    let shapes = (v(3) as usize).max(1);
    // The actor's up axis (its rotation is the direction's: pitch, yaw).
    let (ys, yc) = yaw.to_radians().sin_cos();
    let up = Vec3::new(-pitch.sin() * yc, -pitch.sin() * ys, pitch.cos());
    if let Some(particle) = flag(1) {
        cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(particle)), name: particle, at: Attach::World(at), rotation: Vec3::new(pitch.to_degrees(), yaw, 0.0), scale: 1.0, persistent: false });
    }
    if let Some(fire) = flag(4) {
        cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(fire)), name: fire, at: Attach::World(at), rotation: Vec3::new(pitch.to_degrees(), yaw, 0.0), scale: 1.0, persistent: false });
    }
    let info = cx.pawn.combat.attack_ctx;
    // m_MainLife 0 = none: the shot then lives until m_MaxLifeDistance (the moving ones) - 60 s as a bound.
    let life = if life > 0.0 { life } else { 60.0 };
    let state = &mut cx.pawn.combat.lightdark;
    let first = KEY_BASE | u16::from(state.next);
    state.next = state.next.wrapping_add(shapes as u8);
    state.actors.retain(|a| a.0 != first);
    state.actors.push((first, shapes as u8));
    for k in 0..shapes {
        let b = 4 + k * 8;
        let key = KEY_BASE | (first.wrapping_add(k as u16) & 0xFF);
        let row_name = r.rows.get(k).copied().unwrap_or("None");
        spawn_form_shot(cx, FormShot { key, group: r.logical, row: row_name, position: at, yaw, size: (v(b + 1), v(b + 2), v(b + 3).max(v(b + 1))), life, hit_effect: flag(2), hit_se: flag(3), info, hit_list: Vec::new() });
        if let Some(shot) = cx.pawn.combat.shots.iter_mut().rev().find(|x| x.window.key == (FORM_SHOT_SEQUENCE, key)) {
            // (AUDIT-C TF) the hit comes from the PROJECTILE's own TresAtkColl0 (chooser 0x140628960): its m_PawnHitEffect /
            // m_PawnHitSEAsset (the row's flags 2 / 3), else nothing. TresProjTF_BraveShoot's component has no m_HitEffects
            // and TresProjDF_DarkBreak none either: no spark (the Kingdom Key's default table was used); no m_DirectHitEffect
            // on any of them, and their m_MapHitType is NOHIT.
            shot.window.source = crate::status::keyblade::HitSource::NoEntry;
            shot.offset = Vec3::new(v(b + 5), v(b + 6), v(b + 7));
            if v(b) == 1.0 {
                shot.axis = up * v(b + 4);
            }
        }
        let state = &mut cx.pawn.combat.lightdark;
        state.moving.retain(|m| m.key != key);
        if speed > 0.0 {
            state.moving.push(Moving { key, origin: at, velocity: d * speed, max });
        }
    }
    true
}

/// The package's `shot_motion`: its moving shots fly (TresProjectileMovementComponent: velocity = direction x
/// InitialSpeed, no gravity) and end at m_MaxLifeDistance; the shapes of one actor share its hit list.
fn shot_motion(cx: &mut Ctx, shot: &mut Shot) {
    let (seq, key) = shot.window.key;
    if seq != FORM_SHOT_SEQUENCE || key & 0xFF00 != KEY_BASE {
        return;
    }
    let state = &cx.pawn.combat.lightdark;
    if let Some(&(first, n)) = state.actors.iter().find(|(first, n)| (0..u16::from(*n)).any(|k| KEY_BASE | (first.wrapping_add(k) & 0xFF) == key)) {
        let siblings: Vec<u16> = (0..u16::from(n)).map(|k| KEY_BASE | (first.wrapping_add(k) & 0xFF)).filter(|k| *k != key).collect();
        let hits: Vec<TargetId> = cx.pawn.combat.shots.iter().filter(|x| x.window.key.0 == FORM_SHOT_SEQUENCE && siblings.contains(&x.window.key.1)).flat_map(|x| x.window.hit_list().to_vec()).collect();
        shot.window.share_hits(&hits);
    }
    let state = &mut cx.pawn.combat.lightdark;
    let Some(m) = state.moving.iter().find(|m| m.key == key).copied() else { return };
    // the shot's own clock (Window::time: time since it was spawned)
    let flown = m.velocity * shot.window.time;
    shot.position = m.origin + flown;
    if m.max > 0.0 && flown.length() >= m.max {
        shot.life = 0.0;
        state.moving.retain(|x| x.key != key);
    }
}

/// The shot handlers of the classes for one TresAnimNotify_Shot (`name` = its m_ProjectileName, `index` = its
/// LocationIndex): Attack's 0x141344580 for kinds 0x25 (BARRIERCRACKER at the weapon's muzzle) and 0xD0 (SCDF_DARKBREAK
/// once landed: both shot bits, attack.rs), ComboTF's 0x1413590B0 for 0xDA (the notify's projectile `index` x 200 ahead of
/// his root, flying forward) and 0xDC (the notify's projectile 300 cm from the target on a random direction - pitch -60 .. 0,
/// yaw -180 .. 180 for the first ten, then yaw +-90 alternating - aimed at it; no target: 500 ahead of the muzzle). True =
/// handled (a shot spawned or the call was this class's).
/// One TresAnimNotify_Shot of a running attack of these classes (see [`shot`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShotCall<'a> {
    pub class: Class,
    pub kind: u8,
    /// m_ProjectileName (None = the notify names none).
    pub name: Option<&'a str>,
    /// LocationIndex.
    pub index: i32,
    /// The weapon's muzzle (pawn vt+0x1410: ATK_01 of the playing clip).
    pub muzzle: Vec3,
    pub target: TargetId,
    /// The attack's shots so far (state+0x130).
    pub shots: i32,
}

pub(crate) fn shot(cx: &mut Ctx, call: ShotCall<'_>) -> bool {
    let ShotCall { class, kind, name, index, muzzle, target, shots } = call;
    let a = cx.pawn.yaw.to_radians();
    let forward = Vec3::new(a.cos(), a.sin(), 0.0);
    match (class, kind) {
        (Class::Lf | Class::Df, KIND_BARRIER_CRACKER) => spawn_projectile(cx, class, "BARRIERCRACKER", muzzle, forward),
        (Class::Tf, 0xDA) => {
            let at = cx.pawn.cmc.location + forward * (index as f32 * 200.0);
            name.is_some_and(|n| spawn_projectile(cx, class, n, at, forward))
        }
        (Class::Tf, 0xDC) => {
            let aim = cx.target(target).map_or(muzzle + forward * 500.0, |t| t.center);
            let (pitch, yaw) = if shots < 10 {
                // (xorshift >> 8) x 3.5762787e-06 - 60, then (xorshift >> 8) x 2.1457672e-05 - 180 (FUN_142CFF460: the
                // engine's global stream; here the pawn's own, as hit.rs's rolls)
                let p = roll(cx) as f32 * 3.576_278_7e-6 - 60.0;
                (p, roll(cx) as f32 * 2.145_767_2e-5 - 180.0)
            } else if shots & 1 == 0 {
                (0.0, -90.0)
            } else {
                (0.0, 90.0)
            };
            // FUN_142D8D890(rotator, (300, 0, 0)): the offset; the shot spawns at aim - offset, aimed along it.
            let (p, y) = (f32::to_radians(pitch), f32::to_radians(yaw));
            let offset = Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), p.sin()) * 300.0;
            name.is_some_and(|n| spawn_projectile(cx, class, n, aim - offset, offset))
        }
        _ => false,
    }
}

/// One draw of the pawn's xorshift stream (hit.rs's): the top 24 bits.
fn roll(cx: &mut Ctx) -> u32 {
    let rng = &mut cx.pawn.combat.rng;
    if *rng == 0 {
        *rng = 0x9E37_79B9;
    }
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    *rng >> 8
}

/// The shot attack.rs's shot handler asked for in these classes (0x141344580 case 0xD0: "SCDF_DARKBREAK" once landed): the
/// form's projectile at the muzzle, forward. False = not this package's name.
pub(crate) fn spawn_pending(cx: &mut Ctx, remind: u8, name: &str, at: Vec3) -> bool {
    let Some(class) = Class::from_value(remind) else { return false };
    let a = cx.pawn.yaw.to_radians();
    proj(class, name).is_some() && spawn_projectile(cx, class, name, at, Vec3::new(a.cos(), a.sin(), 0.0))
}

/// MaxCombo / Finishers of the held form (None = not these forms): attack.rs `max_combo` / `finishers`.
pub fn counts_in(cmd: i32) -> Option<(i32, i32)> {
    class_of(cmd).map(|_| (max_combo(), finishers()))
}

// ------------------------------------------------------------------------------------------------ Dark Break's warp

/// ComboDF slot 136 0x141352160 for kind 0xD0 (the Attack class's warp 0x141337320 for the others), called by the warp
/// countdown (0x14134AA80 with 150 / 100): from his centre (slot 134 0x141336940: his location at his lock-on height) to
/// 200 cm over the top of the target (its lock-on point + its half height, vt+0x548), swept with radius 60 (pawn
/// vt+0x2750), then down by 100 the same way; he is put there (FUN_143EAC3D0). No target: he stays.
pub(crate) fn dark_break_warp(cx: &mut Ctx, target: TargetId, drop: f32) {
    let Some(t) = cx.target(target) else { return };
    let p = *cx.pawn.params();
    let up = Vec3::Z * p.capsule_center_z;
    let start = cx.pawn.cmc.location + up;
    let over = Vec3::new(t.center.x, t.center.y, t.center.z + t.half_height + 200.0);
    // (F-CLAWS for F-LIGHTDARK) the two legs are pawn vt+0x2750 = 0x141431630, the "Warp" LINE test (60 = its z offset, not a
    // radius; level only: a pawn body never blocks), as the shared slot 136 (states/attack.rs `warp_test`): a hit gives its
    // location - 60 (nudged 1 cm), a clear leg its end; the second leg starts from the first's result. He is put at the result
    // (FUN_143EAC3D0), here swept (`swept_warp`): never into or through the level.
    let first = crate::states::attack::warp_test(cx, start, over).unwrap_or_else(|at| at);
    let second = crate::states::attack::warp_test(cx, first, first - Vec3::Z * drop).unwrap_or_else(|at| at);
    let landed = crate::states::attack::swept_warp(cx, second - up) + up;
    cx.pawn.cmc.location = landed - up;
    cx.pawn.cmc.stop_movement();
    let d = (t.center - landed).truncate();
    if d.length_squared() > 1.0e-6 {
        cx.pawn.yaw = crate::sora::wrap_degrees(d.y.atan2(d.x).to_degrees());
    }
}

/// Dark Break's warp delay (PlayAttack head with +0x1E1: +0x258 = 0.2) and invincible time (ComboDF Decide: +0x25C = 0.5).
pub(crate) fn dark_break_times() -> (f32, f32) {
    let r = attack_row(Class::Df, KIND_DARK_BREAK, true);
    (r.and_then(|r| r.values.get(1).copied()).unwrap_or(0.2), r.and_then(|r| r.values.first().copied()).unwrap_or(0.5))
}

/// (AUDIT-C TF) One row of an attach data asset Double Form's clips name (the sheet's `ead` rows, SQEX_StructParticleAttachData):
/// AirDodge N054's `vw_so220_warp` (Sonic Step's warp: twelve systems on atama / mune / center / R_sune / L_sune).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttachRow {
    /// The row's place in its asset (two rows of one system may share a socket with other offsets: bra0 on center at 0 and
    /// at y 50), so a host keys the row's placement by it.
    pub index: usize,
    /// The particle system the row plays.
    pub system: &'static str,
    /// The row's socket (`SocketName`) on the owner (every row of vw_so220_warp is SQEX_ATTACH_OBJECT_OWNER: Sora).
    pub socket: &'static str,
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

/// (AUDIT-C TF) The rows of the attach data asset `asset` (its name, any case) of the request's `group`: what KH3's
/// TresAnimNotifyState_AttachEffect plays for it (each row where ITS row attaches it; LOOPEND at the window's end). Empty: not
/// such an asset of this package.
pub fn attach_rows(asset: &str, group: i32) -> Vec<AttachRow> {
    rows("ead")
        .filter(|r| r.logical.eq_ignore_ascii_case(asset) && r.code == group)
        .enumerate()
        .filter_map(|(index, r)| {
            let v = |i: usize| r.values.get(i).copied().unwrap_or(0.0);
            Some(AttachRow {
                index,
                system: r.effects.first().copied()?,
                socket: r.clip,
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

/// (AUDIT-C TF) Every attach data asset of the package's clips (the host's placement table).
pub fn attach_assets() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for r in rows("ead") {
        if !out.contains(&r.logical) {
            out.push(r.logical);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::AnimId;
    use crate::combat::decide::{ComboAbilities, DecideIn};

    fn din(combo: i32, last: u8, reach: u8, air: bool, n: i32) -> DecideIn {
        DecideIn { combo, reach, last_kind: last, dist: 150.0, dz: if air { 0.0 } else { 90.0 }, target_high: false, target_on_ground: true, air, num_enemies: 1, launch: false, endless: false, max_combo: 6, finishers: 3, finish_count: n, stocks: (1, 1) }
    }

    fn chain(class: Class, air: bool, a: &ComboAbilities) -> Vec<u8> {
        let (mut last, mut reach, mut n, mut out) = (0u8, 0u8, 0, Vec::new());
        for combo in 0..8 {
            let r = decide(class, &din(combo, last, reach, air, n), a);
            out.push(r.d.kind);
            (last, reach, n) = (r.d.kind, r.d.reach, r.d.finish_count);
            if r.d.finish_end {
                break;
            }
        }
        out
    }

    /// The combos of the three classes near one target: three normal steps, then the three finishers.
    #[test]
    fn the_three_classes_chain_three_steps_and_three_finishers() {
        let none = ComboAbilities::NONE;
        assert_eq!(chain(Class::Lf, false, &none), [1, 2, 3, 0xCD, 0xCE, 0xCF]);
        assert_eq!(chain(Class::Df, false, &none), [1, 2, 3, 0xCD, 0xCE, KIND_DARK_BREAK]);
        assert_eq!(chain(Class::Lf, true, &none), [0x10, 0x17, 0x12, 0xD1, 0xD2, 0xD3]);
        // Light Form's wear form gives the upgrades (p_ex001_FormAbility of wear form 8): the normal steps take them.
        assert_eq!(chain(Class::Lf, false, &ComboAbilities::ALL), [9, 0x0A, 0x0B, 0xCD, 0xCE, 0xCF]);
        assert_eq!(chain(Class::Tf, false, &none), [0xD4, 0xD5, 0xD7, 0xDA, 0xDB, 0xDC]);
        assert_eq!(chain(Class::Tf, true, &none), [0xD8, 0xD9, 0xD7, 0xDD, 0xDB, 0xDC]);
        // Dark Break asks for the warp.
        let r = decide(Class::Df, &din(5, 0xCE, 0, false, 2), &none);
        assert!(r.warp && r.d.finish_end && r.d.kind == KIND_DARK_BREAK);
    }

    #[test]
    fn double_form_rises_dives_and_launches() {
        let rise = decide(Class::Tf, &DecideIn { target_high: true, dz: 250.0, ..din(1, 0xD4, 0, false, 0) }, &ComboAbilities::NONE);
        assert_eq!((rise.d.kind, rise.d.fixed_dir_z.is_some()), (0x1F, true));
        let dive = decide(Class::Tf, &DecideIn { dz: -200.0, ..din(1, 0xD8, 0, true, 0) }, &ComboAbilities::NONE);
        assert_eq!(dive.d.kind, 0xDE);
        let launch = decide(Class::Tf, &DecideIn { launch: true, ..din(1, 0xD4, 0, false, 0) }, &ComboAbilities::NONE);
        assert_eq!((launch.d.kind, launch.d.stocks, launch.d.fixed_dir_z), (0xDF, (2, 1), Some(1000.0)));
        let second = decide(Class::Tf, &DecideIn { launch: true, stocks: (2, 1), ..din(1, 0xD4, 0, true, 0) }, &ComboAbilities::NONE);
        assert_eq!((second.d.kind, second.d.stocks), (0xE0, (2, 2)));
        // far: the approach 0xD6 first.
        assert_eq!(decide(Class::Tf, &DecideIn { dist: 550.0, ..din(0, 0, 0, false, 0) }, &ComboAbilities::NONE).d.kind, 0xD6);
    }

    /// The sheet carries every case, each clip is in the animation table, and the replacement sets resolve.
    #[test]
    fn sonic_steps_warp_is_its_attach_datas_twelve_rows() {
        // (AUDIT-C TF) vw_so220_warp (AnimSet23 AirDodge N054): 12 rows of group 0, each on its own socket and offset
        let rows = attach_rows("vw_so220_warp", 0);
        assert_eq!(rows.len(), 12);
        assert!(attach_rows("VW_SO220_WARP", 1).is_empty(), "group 1 has no rows");
        let blur: Vec<Vec3> = rows.iter().filter(|r| r.system == "vw_so220_warp_bra0" && r.socket == "center").map(|r| r.offset).collect();
        assert_eq!(blur, [Vec3::ZERO, Vec3::new(0.0, 50.0, 0.0)], "two blur rows on center, apart");
        let skin = rows.iter().find(|r| r.system == "vw_so220_warp_skn0" && r.socket == "R_sune").unwrap();
        assert_eq!((skin.offset, skin.scale), (Vec3::new(20.0, 0.0, 0.0), Vec3::new(1.0, 0.3, 1.0)));
        assert!(rows.iter().all(|r| r.follow && !r.absolute_rotation));
        assert_eq!(attach_assets(), ["vw_so220_warp"]);
    }

    #[test]
    fn the_sheet_is_complete() {
        for (class, kinds) in [(Class::Lf, &[0x1F, 0x25, 0xCD, 0xCE, 0xCF, 0xD1, 0xD2, 0xD3][..]), (Class::Df, &[0x1F, 0x25, 0xCD, 0xCE, 0xD0, 0xD1, 0xD2, 0xD3][..]), (Class::Tf, &[0x1F, 0x25, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE0][..])] {
            for &k in kinds {
                let r = attack_row(class, k, false).unwrap_or_else(|| panic!("{class:?} {k:#x}"));
                assert!(AnimId::from_name(r.clip).is_some(), "{}: {} not in the animation table", r.id, r.clip);
            }
        }
        assert_eq!(attack_row(Class::Lf, 0x25, true).map(|r| r.clip), Some("MagicAirCast"));
        assert_eq!(attack_row(Class::Lf, 0x25, false).map(|r| r.clip), Some("MagicCast"));
        assert_eq!(attack_row(Class::Lf, 0xCF, false).map(|r| (r.group, r.root_len)), Some((0x834, 380.0)));
        assert_eq!(attack_row(Class::Df, 0xCD, false).map(|r| r.group), Some(0x898));
        assert_eq!(replaced_clip(CHANGE_LF, "DodgeRollB"), Some("M051"));
        assert_eq!(replaced_clip(CHANGE_TF, "StyleChangeEnd"), Some("N401"));
        assert_eq!(replaced_clip(CHANGE_LF, "StyleChangeEnd"), None);
        assert_eq!(replaced_clip(CHANGE_TF2, "M_N425"), Some("N425"));
        assert_eq!((counter_command(CHANGE_LF), counter_command(CHANGE_DF), counter_command(CHANGE_TF2), counter_command(0x94)), (Some(0xD7), Some(0xD8), Some(0xD9), None));
        assert_eq!((max_combo(), finishers()), (6, 3));
        assert_eq!(guard(CHANGE_TF).map(|g| (g.start, g.group)), Some(("N279", 0xD48)));
        assert_eq!(dodge_group(CHANGE_DF), Some(0x898));
        assert!(proj(Class::Tf, "TF_JOINTSTRUGGLE_L").is_some_and(|r| r.values[1] == 2000.0 && r.values[2] == 600.0));
        // Brave Shoot: two upright pillars (radius 100, half length 300) at (0, +-100, 200); Dark Break: one, (0, 0, 450), 100 -> 450.
        assert_eq!(proj(Class::Tf, "TF_BRAVESHOOT").map(|r| r.values[3..].to_vec()), Some(vec![2.0, 1.0, 100.0, 0.0, 0.0, 300.0, 0.0, 100.0, 200.0, 1.0, 100.0, 0.0, 0.0, 300.0, 0.0, -100.0, 200.0]));
        assert_eq!(proj(Class::Df, "SCDF_DARKBREAK").map(|r| r.values[3..].to_vec()), Some(vec![1.0, 1.0, 100.0, 1200.0, 450.0, 450.0, 0.0, 0.0, 450.0]));
        assert_eq!(dark_break_times(), (0.2, 0.5));
    }

    /// FUN_1414145C0's form table (actions.rs `form_counter`), one case per form: the command and the shields' 2 s.
    #[test]
    fn the_counter_window_table_of_every_form() {
        use crate::actions::form_counter;
        assert_eq!(form_counter(0x86, 1), Some((0x0E, None)), "Nano Arms (F-NANO)");
        assert_eq!(form_counter(0x87, 1), Some((0x0F, None)), "Ultimate Form (F-ULTIMATE)");
        assert_eq!((form_counter(0x89, 1), form_counter(0x89, 2), form_counter(0x89, 3)), (Some((0x10, Some(2.0))), Some((0x11, Some(2.0))), Some((0x11, Some(2.0)))), "Counter Shield (F-SHIELD)");
        assert_eq!((form_counter(0x91, 1), form_counter(0x91, 3)), (Some((0x12, Some(2.0))), Some((0x13, Some(2.0)))), "Frying Pan (F-SHIELD)");
        assert_eq!((form_counter(0x8E, 1), form_counter(0x8F, 1)), (Some((6, None)), Some((6, None))), "Hunny Blasters / Hunny Launcher (F-ARROWGUN)");
        assert_eq!((form_counter(0x92, 1), form_counter(0x93, 1)), (Some((6, None)), Some((6, None))), "Boom Hammer / Clock Drill (F-HAMMER)");
        assert_eq!((form_counter(0xDD, 1), form_counter(0xDE, 1), form_counter(0xDF, 1), form_counter(0xE0, 1)), (Some((0xD7, None)), Some((0xD8, None)), Some((0xD9, None)), Some((0xD9, None))));
        for keyblade_rule in [0, 0x80, 0x81, 0x94, 0x8B] {
            assert_eq!(form_counter(keyblade_rule, 1), None, "{keyblade_rule:#x}: the abilities' rule");
        }
    }

    fn pawn() -> crate::pawn::Pawn {
        let mut p = crate::pawn::Pawn::new(crate::params::SORA_PARAMS, crate::cmc::Cmc::new(&crate::params::SORA_PARAMS, Vec3::ZERO), 0.0, crate::anim::AnimPlayer::new(AnimId::IdleN));
        p.status.hp = 100;
        p
    }

    /// Brave Shoot is two upright capsules of one actor (radius 100, half length 300, 100 cm either side, 200 up), Dark Break
    /// one (450 up, half length 450, radius 100 growing to 450), Joint Struggle a flying sphere.
    #[test]
    fn the_projectiles_are_their_blueprints_shapes() {
        let world = crate::test_world::TestWorld::floor();
        let mut null = crate::host::NullHost::default();
        let mut host = null.host(&world);
        let mut p = pawn();
        let mut cx = Ctx::new(&mut p, &mut host);
        assert!(spawn_projectile(&mut cx, Class::Tf, "TF_BRAVESHOOT", Vec3::new(200.0, 0.0, 0.0), Vec3::X));
        let shots: Vec<(Vec3, Vec3, f32)> = cx.pawn.combat.shots.iter().map(|s| (s.centre(), s.axis, s.radius())).collect();
        assert_eq!(shots, [(Vec3::new(200.0, 100.0, 200.0), Vec3::new(0.0, 0.0, 300.0), 100.0), (Vec3::new(200.0, -100.0, 200.0), Vec3::new(0.0, 0.0, 300.0), 100.0)]);
        assert_eq!(cx.pawn.combat.lightdark.actors.len(), 1, "one actor, two shapes");
        cx.pawn.combat.shots.clear();
        assert!(spawn_projectile(&mut cx, Class::Df, "SCDF_DARKBREAK", Vec3::ZERO, Vec3::X));
        let s = &cx.pawn.combat.shots[0];
        assert_eq!((s.centre(), s.axis, s.radius(), s.life), (Vec3::new(0.0, 0.0, 450.0), Vec3::new(0.0, 0.0, 450.0), 100.0, 0.5));
        cx.pawn.combat.shots.clear();
        assert!(spawn_projectile(&mut cx, Class::Tf, "TF_JOINTSTRUGGLE_L", Vec3::ZERO, Vec3::Y));
        assert_eq!((cx.pawn.combat.shots[0].axis, cx.pawn.combat.lightdark.moving.last().map(|m| (m.velocity, m.max))), (Vec3::ZERO, Some((Vec3::new(0.0, 2000.0, 0.0), 600.0))));
    }

    /// FUN_1413F4EF0: his own spot when 300 cm of room are free, else the first free point of the rings (here: a wall 100 cm
    /// in front of him pushes the start 300 cm behind him... the first direction of ring 1 that is free and reachable).
    #[test]
    fn ultimate_finish_finds_room() {
        let mut p = pawn();
        {
            let mut null = crate::host::NullHost::default();
            let open = crate::test_world::TestWorld::floor();
            let mut host = null.host(&open);
            let cx = Ctx::new(&mut p, &mut host);
            assert_eq!(finish_start_point(&cx), Vec3::ZERO);
        }
        let walled = crate::test_world::TestWorld::floor().with_box(Vec3::new(100.0, -2000.0, 0.0), Vec3::new(200.0, 2000.0, 1000.0));
        let mut null = crate::host::NullHost::default();
        let mut host = null.host(&walled);
        let cx = Ctx::new(&mut p, &mut host);
        let to = finish_start_point(&cx);
        assert!(to.x < -200.0, "away from the wall: {to}");
        assert!((to.truncate().length() - 300.0).abs() < 1.0 || (to.truncate().length() - 600.0).abs() < 1.0, "on a ring: {to}");
    }

    /// (AUDIT-C TF) FUN_141403130 is an OVERLAP test (UE's sweep reports what it starts inside): a pillar wholly inside the
    /// 300 cm ball over his spot, that no 0.2 cm sweep and no trace from the ball's centre meets, still takes the spot. The
    /// fixture answers sweeps and traces with nothing (as a host whose sweeps skip start-inside shapes) and the overlap from
    /// the TestWorld geometry: the start point moves to a ring.
    #[test]
    fn ultimate_finish_takes_the_overlap_not_the_sweep() {
        use crate::world::World as _;
        struct SweepBlind(crate::test_world::TestWorld);
        impl crate::world::World for SweepBlind {
            fn sweep_capsule(&self, _: Vec3, _: Vec3, _: f32, _: f32) -> Option<crate::world::Hit> {
                None
            }
            fn line_trace(&self, _: Vec3, _: Vec3) -> Option<crate::world::Hit> {
                None
            }
            fn overlap_sphere(&self, center: Vec3, radius: f32) -> bool {
                self.0.overlap_sphere(center, radius)
            }
        }
        // a pillar 50 x 50 x 50 cm, 100 cm ahead and 250 cm up: inside the ball over his spot (centre z 302), off every ring
        let pillar = crate::test_world::TestWorld::floor().with_box(Vec3::new(75.0, -25.0, 225.0), Vec3::new(125.0, 25.0, 275.0));
        assert!(pillar.overlap_sphere(Vec3::new(0.0, 0.0, 302.0), 300.0) && !pillar.overlap_sphere(Vec3::new(0.0, 0.0, 302.0), 50.0));
        let world = SweepBlind(pillar);
        let mut p = pawn();
        let mut null = crate::host::NullHost::default();
        let mut host = null.host(&world);
        let cx = Ctx::new(&mut p, &mut host);
        let to = finish_start_point(&cx);
        assert!(to.truncate().length() > 299.0, "the overlapped spot is refused: {to}");
    }

    /// Double Form's groups on both weapons: the KB_* sticks of Oathkeeper and Oblivion, R_* / L_* on one only.
    #[test]
    fn double_form_groups_on_both_weapons() {
        assert!(both_weapons(CHANGE_TF, "KB_StickL") && both_weapons(CHANGE_TF2, "KB_SphereSS"));
        assert!(!both_weapons(CHANGE_TF, "R_Stick") && !both_weapons(CHANGE_TF, "L_Sphere") && !both_weapons(CHANGE_LF, "KB_StickL"));
    }

    /// The sockets of a Double Form clip: both hands, the left one on the other side of him.
    #[test]
    fn both_hands_have_tracks() {
        let (r, _) = socket_at("N411", false, 0.3).unwrap();
        let (l, _) = socket_at("N411", true, 0.3).unwrap();
        assert!((r - l).length() > 20.0, "{r} {l}");
        assert!(socket_at("1301", false, 0.2).is_some() && socket_at("1301", true, 0.2).is_none());
        assert_eq!(weapon(Class::Df, false), ("w_so210", 103.0));
        assert_eq!(weapon(Class::Tf, true).0, "w_so210");
        assert_eq!(weapon(Class::Lf, false), ("w_so200", 80.0));
    }
}
