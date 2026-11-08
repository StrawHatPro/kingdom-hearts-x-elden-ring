//! (weapons F-HAMMER) Hyper Hammer (`form.HM`, CHANGE_HM 0x81) -> Drill Punch (`form.DR`, CHANGE_DR 0x84) of Favorite
//! Deputy and Boom Hammer (`form.HM2`, CHANGE_HM2 0x92) -> Clock Drill (`form.DR2`, CHANGE_DR2 0x93) of Classic Tone: their
//! form package (documentation/KH3_WEAPONS_PLAN.md 1.3 / 4; build/sora_tmp/weapons/F-HAMMER/STATE.md). The data is
//! `sheets/port/sora-form-hammer.json` (tools/sora_form_hammer.js): every number below that is not an address comes from it.
//!
//! WHAT KH3 DOES (decompiled C in re/decomp; addresses here, in the sheet and in states/combo_hm.rs):
//! - THE CLASSES (pawn slot 1337 0x141432F00): forms 0x81 / 0x92 -> COMBO_HM (76), 0x84 / 0x93 -> COMBO_DR (79):
//!   subclasses of TresPlayerState_Attack that override only Decide (vtables 0x14690FF70 / 0x14690D720, slot 122;
//!   slots 95 .. 98 and 120 are thunks to Attack's Reset / Begin / Tick / End / PlayAttack). KH3 SHARES the class, the
//!   clips (AnimSet3 / AnimSet6) and Decide between Favorite Deputy's and Classic Tone's forms: Classic Tone differs by
//!   its weapons (w_so112 / w_so113), the effect group 0x76C of every play (its toon effects), its HM2_* / DR2_* attack rows
//!   (the TresAtk begin renames 0x141357320 / 0x141357100), its sets AnimSet20 / 21 (Reflec, BarrierCracker, M_L421),
//!   Barrier Cracker as its guard counter (TryAttack 0x141437330: kind 0x25 in 0x92 / 0x93) and the Clock of its finish.
//! - MaxCombo 0x141433570: 5 in all four; Finishers 0x141433A60: 1 (the default branch).
//! - THE FINISH: FINISH_DR (0x9E) in the second stages (SetStyleFinishCommand 0x1413EE030; the first stages offer the second
//!   stage). TryStyleFinish 0x141447840: STYLEFINISH (33) in 0x84 (the drill: StyleFinish slot 148 0x1413DF1F0),
//!   STYLEFINISH_DR (34) in 0x93 (the Clock: StyleFinishDR Tick 0x1413D4990). Bodies in states/combo_hm.rs.
//! - THE HIT SHAPES: the clips' windows name the form weapons' own groups (StickM, ImpactS, Warhead ...: their
//!   TresAtkColl0); the generator poses them by FK per frame (the rows `track.*`), [`hit_shape`] reads them for the
//!   generic hit windows of combat/hit.rs (the KH hit path: every ER enemy, its reactions, launches and juggles).

use super::FormMoveset;
use crate::combat::form::KeybladeFormRow;
use crate::host::TargetId;
use crate::pawn::Ctx;
use crate::states::{FinishView, StateData};
use glam::Vec3;

// The rows (sheets/port/sora-form-hammer.json): `FormHammerRow`, `FORM_HAMMER`, `FHM_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_hammer.rs"));

/// CHANGE_HM: Hyper Hammer (Favorite Deputy, stage 1).
pub const CHANGE_HM: i32 = 0x81;
/// CHANGE_DR: Drill Punch (Favorite Deputy, stage 2).
pub const CHANGE_DR: i32 = 0x84;
/// CHANGE_HM2: Boom Hammer (Classic Tone, stage 1).
pub const CHANGE_HM2: i32 = 0x92;
/// CHANGE_DR2: Clock Drill (Classic Tone, stage 2).
pub const CHANGE_DR2: i32 = 0x93;
/// FINISH_DR: both second stages (SetStyleFinishCommand 0x1413EE030: P3 traced it for 0x93 too).
pub const FINISH_DR: i32 = 0x9E;
/// Barrier Cracker: the guard counter's forced kind in 0x92 / 0x93 (TryAttack 0x141437330 0x141437E1A).
pub const KIND_BARRIER_CRACKER: u8 = 0x25;
/// MaxCombo (pawn slot 928 0x141433570) and Finishers (slot 1253 0x141433A60) in the four forms.
pub const MAX_COMBO: i32 = 5;
pub const FINISHERS: i32 = 1;

/// The package: the four forms and FINISH_DR.
pub(crate) const MOVESET: FormMoveset = FormMoveset {
    name: "hammer",
    forms: &["form.HM", "form.DR", "form.HM2", "form.DR2"],
    finishes: &[FINISH_DR],
    on_start,
    attack,
    finish_begin: crate::states::combo_hm::finish_begin,
    finish_tick: crate::states::combo_hm::finish_tick,
    replace_clip,
    replace_clip_in: replaced_clip,
    hit_shape,
    row_rename,
    hit_volumes, // (F-SHAPES)
    ..FormMoveset::NONE
};

/// The sheet's form key of a command.
fn form_key(cmd: i32) -> Option<&'static str> {
    match cmd {
        CHANGE_HM => Some("HM"),
        CHANGE_DR => Some("DR"),
        CHANGE_HM2 => Some("HM2"),
        CHANGE_DR2 => Some("DR2"),
        _ => None,
    }
}

/// A hammer form (COMBO_HM): Hyper Hammer or Boom Hammer.
pub fn is_hammer(cmd: i32) -> bool {
    matches!(cmd, CHANGE_HM | CHANGE_HM2)
}

/// A drill form (COMBO_DR): Drill Punch or Clock Drill.
pub fn is_drill(cmd: i32) -> bool {
    matches!(cmd, CHANGE_DR | CHANGE_DR2)
}

/// One of the package's four forms.
pub fn is_hammer_family(cmd: i32) -> bool {
    form_key(cmd).is_some()
}

/// Classic Tone's forms: the toon variants.
pub fn is_toon(cmd: i32) -> bool {
    matches!(cmd, CHANGE_HM2 | CHANGE_DR2)
}

pub(crate) fn rows(what: &'static str, form: &'static str) -> impl Iterator<Item = &'static FormHammerRow> {
    FORM_HAMMER.iter().filter(move |r| r.what == what && r.form == form)
}

/// The row of an attack kind in a form (`attack.<form>.<kind>`; Barrier Cracker 0x25 has an air row of its own: PlayAttack
/// case 0x25 picks `AirBarrierCracker` with +0x142).
pub fn attack_row(cmd: i32, kind: u8) -> Option<&'static FormHammerRow> {
    attack_row_for(cmd, kind, false)
}

/// [`attack_row`] with the attack's air flag (+0x142).
pub fn attack_row_for(cmd: i32, kind: u8, air: bool) -> Option<&'static FormHammerRow> {
    let form = form_key(cmd)?;
    let mut found = rows("attack", form).filter(|r| r.code == i32::from(kind));
    let first = found.next()?;
    Some(found.chain(std::iter::once(first)).find(|r| r.flags.contains(&"air") == air).unwrap_or(first))
}

/// The Barrier Cracker projectile (`proj.BARRIERCRACKER`: TresProj_BarrierCrackerSO): (its row, radius cm, life s, hit
/// effect).
pub(crate) fn barrier_cracker() -> Option<(&'static str, f32, f32, Option<&'static str>)> {
    let r = FORM_HAMMER.iter().find(|r| r.what == "proj" && r.logical == "BARRIERCRACKER")?;
    Some((r.rows.first().copied()?, r.values.get(1).copied()?, r.values.first().copied()?, r.effects.first().copied().filter(|e| *e != "-")))
}

/// The muzzle (the form weapon's ATK_01) of a clip of the held form at a clip time, world (`track.<form>.<clip>.ATK_01`).
pub(crate) fn muzzle(cx: &Ctx, sequence: &str, time: f32) -> Option<Vec3> {
    let form = form_key(cx.pawn.combat.form.cmd)?;
    let row = track(form, sequence, "ATK_01")?;
    let (c, _) = track_at(row, time)?;
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    Some(cx.pawn.cmc.location + Vec3::new(c.x * cos - c.y * sin, c.x * sin + c.y * cos, c.z))
}

/// (F-POLISH) The drill's X_link_skt (`track.DR.<clip>.X_link_skt`) at a clip time of M_6421 / M_6423, world: where
/// FINISH_DR carries its caught pawn (StyleFinish slot 148: m_pCatchPawn in state 62 linked to it).
pub(crate) fn catch_link(cx: &Ctx, sequence: &str, time: f32) -> Option<Vec3> {
    let row = track("DR", sequence, "X_link_skt")?;
    let (c, _) = track_at(row, time)?;
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    Some(cx.pawn.cmc.location + Vec3::new(c.x * cos - c.y * sin, c.x * sin + c.y * cos, c.z))
}

/// m_ReplaceAnimSets: the animation-table row a logical name plays as in the form (HM: AnimSet3, DR: AnimSet6, HM2:
/// AnimSet3 + AnimSet20, DR2: AnimSet6 + AnimSet21); None = the base set's row. For the pose hosts too (guard, dodge ...).
pub fn replaced_clip(cmd: i32, logical: &str) -> Option<&'static str> {
    let form = form_key(cmd)?;
    rows("anim", form).find(|r| r.logical == logical && r.clip != "-").map(|r| r.clip)
}

/// The form package's `replace_clip` (the framework's own plays: the transformation and the finish bodies). One moveset holds
/// the four forms and the hook gets no command: the names the framework plays for them are the finish bodies' (`M_6421` /
/// `M_6423` of AnimSet6, `M_L421` of AnimSet21), which no other of the four sets names differently.
fn replace_clip(logical: &str) -> Option<&'static str> {
    match logical {
        "M_6421" | "M_6423" => replaced_clip(CHANGE_DR, logical),
        "M_L421" => replaced_clip(CHANGE_DR2, logical),
        _ => None,
    }
}

/// The attack row a hit window of `row` hits with in the held form (the TresAtk begin handlers: ComboHM 0x141357320, ComboDR
/// 0x141357100; the dodge roll's 0x14137B880), any kind; the kind-bound rename (SCHM_UPPER of kind 0x5F) is
/// [`row_rename_kind`].
pub fn row_rename(cmd: i32, row: &str) -> Option<&'static str> {
    let form = form_key(cmd)?;
    rows("rename", form).find(|r| r.code <= 0 && r.logical.eq_ignore_ascii_case(row)).map(|r| r.clip)
}

/// The kind-bound rename alone (0x141357320: `if kind == 0x5F: SCHM_UPPER -> SCHM_RISE` (0x81), `-> HM2_RISE` (0x92));
/// combat/hit.rs applies it before the form's [`row_rename`].
pub fn row_rename_bound(cmd: i32, kind: u8, row: &str) -> Option<&'static str> {
    let form = form_key(cmd)?;
    rows("rename", form).find(|r| r.code > 0 && r.code == i32::from(kind) && r.logical.eq_ignore_ascii_case(row)).map(|r| r.clip)
}

/// The rename with the attack kind: [`row_rename_bound`], else [`row_rename`].
pub fn row_rename_kind(cmd: i32, kind: u8, row: &str) -> Option<&'static str> {
    row_rename_bound(cmd, kind, row).or_else(|| row_rename(cmd, row))
}

/// A group of the held form's weapon whose place is frozen after its first tick (m_bDisableLocation1Attach: the shape rows'
/// last value).
pub fn group_frozen(cmd: i32, group: &str) -> bool {
    let Some(form) = form_key(cmd) else { return false };
    rows("shape", form).any(|r| r.logical == group && r.values.get(15).is_some_and(|v| *v > 0.5))
}

/// The m_IncSize / m_IncMaxSize of a form weapon's shape group (the shape rows' values 6 / 9): (growth per s, cap).
fn growth(form: &'static str, group: &str) -> (f32, f32) {
    rows("shape", form).find(|r| r.logical == group).map_or((0.0, 0.0), |r| (r.values.get(6).copied().unwrap_or(0.0), r.values.get(9).copied().unwrap_or(0.0)))
}

/// The per-frame track of a group in a clip (its sequence) of the held form (`track.<form>.<sequence>.<group>.0`).
fn track(form: &'static str, sequence: &str, group: &str) -> Option<&'static FormHammerRow> {
    rows("track", form).find(|r| r.clip == sequence && r.logical == group)
}

/// The track's sphere [x, y, z, r] (actor space, his yaw 0) at a clip time, linear between frames.
fn track_at(row: &FormHammerRow, time: f32) -> Option<(Vec3, f32)> {
    let frames = usize::try_from(row.code).ok().filter(|f| *f > 0)?;
    let v = row.values;
    if v.len() < frames * 4 {
        return None;
    }
    let last = frames - 1;
    let f = if row.length > 0.0 && last > 0 { (time / row.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let at = |k: usize| v[i * 4 + k] + (v[j * 4 + k] - v[i * 4 + k]) * t;
    Some((Vec3::new(at(0), at(1), at(2)), at(3)))
}

/// The start of the window of `group` in a clip that contains `time` (the attack / finish rows' windows), for the growing
/// shapes (ImpactH: m_IncSize 1600 up to 800).
fn window_start(form: &'static str, sequence: &str, group: &str, time: f32) -> f32 {
    let mut best = 0.0;
    let sequence_of = |row: &str| crate::anim::AnimId::from_name(row).and_then(|a| a.data().samples.first().map(|&(_, k)| crate::anim::SEQUENCES[k].name));
    for r in FORM_HAMMER.iter().filter(|r| r.form == form && (r.what == "attack" || r.what == "finish") && sequence_of(r.clip) == Some(sequence)) {
        for w in r.windows {
            let mut p = w.split('@');
            let (Some(span), Some(g)) = (p.next(), p.next()) else { continue };
            let Some((a, b)) = span.split_once('-') else { continue };
            let (Ok(a), Ok(b)) = (a.parse::<f32>(), b.parse::<f32>()) else { continue };
            if g == group && time >= a && time <= b + 0.05 {
                best = a;
            }
        }
    }
    best
}

/// The form package's `hit_shape` (combat/hit.rs): a window of the held form's weapon group `group`, opened by the notify of
/// sequence `sequence` (index into `SEQUENCES`) at clip time `time`: (end 1, end 2, radius) in the world, from the FK track
/// (a sphere: both ends at its centre). The growing groups add m_IncSize per second of the window up to m_IncMaxSize.
fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let pawn = &*cx.pawn;
    let form = form_key(pawn.combat.form.cmd)?;
    let name = crate::anim::SEQUENCES.get(sequence)?.name;
    let row = track(form, name, group)?;
    let (c, mut r) = track_at(row, time)?;
    let (inc, cap) = growth(form, group);
    if inc > 0.0 {
        r = (r + inc * (time - window_start(form, name, group, time)).max(0.0)).min(cap.max(r));
    }
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    let p = pawn.cmc.location + Vec3::new(c.x * cos - c.y * sin, c.x * sin + c.y * cos, c.z);
    Some((p, p, r))
}

/// (F-SHAPES, replaces the Clock's centre track and its sphere radius) The Clock's CONVEX shape per frame of its L421
/// (`vol.DR2.L421.<group>.0`: tres_colltest01 under the `wep` bone's FK, m_RelativeRocation roll 90 and the growth of the
/// group's window, in the Clock's root space): combat/hull.rs `VolTrack`, placed at the Clock's root and yaw.
pub(crate) fn clock_track(group: &str) -> Option<crate::combat::hull::VolTrack> {
    let row = rows("vol", "DR2").find(|r| r.clip == "w_so114" && r.logical == group)?;
    crate::combat::hull::VolTrack::of_row(row.flags, row.code, row.length, row.values)
}

/// (F-SHAPES) The world shapes of a group of the held form's weapon that is a CONVEX hull (`vol.<form>.<sequence>.<group>.<k>`:
/// StampL of the hammers, SwingS / SwingM / SwingL of the drills) at a clip time, for an actor at `location` with `yaw`.
pub fn volume_shapes(cmd: i32, group: &str, sequence: &str, time: f32, location: Vec3, yaw: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let form = form_key(cmd)?;
    let out: Vec<_> = rows("vol", form).filter(|r| r.clip == sequence && r.logical == group).filter_map(|r| crate::combat::hull::VolTrack::of_row(r.flags, r.code, r.length, r.values)).map(|t| t.placed(time, location, yaw)).collect();
    (!out.is_empty()).then_some(out)
}

/// (F-SHAPES) The package's `hit_volumes` hook (combat/hit.rs `sweep_volumes`).
fn hit_volumes(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let name = crate::anim::SEQUENCES.get(sequence)?.name;
    volume_shapes(cx.pawn.combat.form.cmd, group, name, time, cx.pawn.cmc.location, cx.pawn.yaw)
}

/// The Clock's windows ("start-end@group@row@interval") and its m_Param_MaxTime.
pub(crate) fn clock() -> Option<&'static FormHammerRow> {
    rows("pawn", "DR2").next()
}

/// The drill forms' dodge (DodgeRoll Begin 0x141368880 / Tick 0x141373EE0: weapon form 6 / 0x15): the MoleDodge clip of the
/// form's set and the play's effect group (+0x90: 0x76C in 0x15); None = not a drill form.
pub fn mole_dodge(cmd: i32) -> Option<(crate::anim::AnimId, i32)> {
    if !is_drill(cmd) {
        return None;
    }
    let clip = crate::anim::AnimId::from_name(replaced_clip(cmd, "MoleDodge")?)?;
    Some((clip, if is_toon(cmd) { 0x76C } else { 0 }))
}

// ------------------------------------------------------------------------------------------------ the hooks

/// StartStyle's per-form tail (0x14141A9D0): 0x81 / 0x84 `pawn+0xE60 |= 0x40000` (bit 18; also 0x82, 0x8A, 0x8C, 0x8D: no
/// reader was found, STATE.md), 0x93 `pawn vt+0xFB0(0x9E)` (the finish's accompany pawn made ready: the Clock; no seam in
/// the core). Nothing the core runs.
fn on_start(_: &mut Ctx, _: &'static KeybladeFormRow) {}

/// The attack request in the forms: COMBO_HM / COMBO_DR (pawn slot 1337 0x141432F00).
fn attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    crate::states::combo_hm::try_attack(state, cx, target)
}

// ------------------------------------------------------------------------------------------------ the decisions

/// What ComboHM's / ComboDR's Decide read (PreDecide / TargetMetrics, the state and the pawn; decide.rs `DecideIn`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HammerDecideIn {
    /// state+0x128.
    pub combo: i32,
    /// state+0x154.
    pub reach: u8,
    /// pawn+0xE10 (pawn vt+0x1DC8): the previous kind of the chain.
    pub last_kind: u8,
    /// state+0x1AC: the previous kind as PreDecide read it (kept at combo 0, where pawn+0xE10 is zeroed).
    pub last_at_begin: u8,
    /// state+0x1CC / +0x1D0 / +0x1E0 / +0x1DC / +0x142 / +0x1C4.
    pub dist: f32,
    pub dz: f32,
    pub target_high: bool,
    pub target_on_ground: bool,
    pub air: bool,
    pub num_enemies: i32,
    /// state+0x1E8: the forced kind, and the abilities slot 124 (0x1413325F0) asks for (TURN_CUTTER 93, SONIC_SLASH 91,
    /// SONIC_DOWN 92, SUMMERSALT 94).
    pub forced_kind: u8,
    pub flow_abilities: [bool; 4],
}

/// What they leave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HammerDecision {
    pub kind: u8,
    pub reach: u8,
    pub air: bool,
    /// +0x140 / +0x141 (and pawn vt+0x2740(1): the pawn's finish-end bit).
    pub finish: bool,
    pub finish_end: bool,
    /// +0x1DD / +0x1D4.
    pub fixed_dir_z: Option<f32>,
    /// combo 0: pawn vt+0x1D40(0) (pawn+0xE10 = 0).
    pub last_reset: bool,
    /// +0x1E8 after slot 124 and Decide.
    pub forced_kind: u8,
}

/// Slot 124 (0x1413325F0) with its argument: the flow-attack choice of a forced 0x36 / 0x37 (2+ enemies and TURN_CUTTER ->
/// 0x38; dist > 800 and SONIC_SLASH -> 0x36; dist < |dz| and SONIC_DOWN -> 0x37); a flow kind other than `keep` without its
/// ability falls back to an air kind by the reach. (The weapon display call of the fallback, pawn vt+0x1388(1, 0, 0), and
/// +0x234 bit 22 have no seam in the core.)
fn refine_flow(i: &HammerDecideIn, keep: u8) -> u8 {
    let [turn_cutter, sonic_slash, sonic_down, summersalt] = i.flow_abilities;
    let mut k = i.forced_kind;
    if matches!(k, 0x36 | 0x37) {
        if i.num_enemies >= 2 && turn_cutter {
            k = 0x38;
        } else if i.dist > 800.0 && sonic_slash {
            k = 0x36;
        } else if i.dist < i.dz.abs() && sonic_down {
            k = 0x37;
        }
    }
    if k == keep {
        return k;
    }
    let has = match k {
        0x36 => sonic_slash,
        0x37 => sonic_down,
        0x38 => turn_cutter,
        0x39 => summersalt,
        _ => return k,
    };
    if has {
        return k;
    }
    match i.reach {
        1 => 0x11,
        2 => 0x12,
        3 => 0x0F,
        _ => 0x10,
    }
}

/// `TresPlayerState_ComboHM::Decide` (0x141362030, decompiled C):
///
/// ```text
/// (+0x1F4 bit 1: finish, finish end, 0x5D / 0x5E by the air: no writer found, never set here)
/// slot124(0x37); if forced == 0x37: air = 1, forced = 0x60
/// last = pawn+0xE10; combo 0: dist <= 400 ? (enemies > 1: reach 2) : reach 1; pawn+0xE10 = 0; last = 0
/// ground, combo < 5 - 1:  last 0x58 -> reach != 2 ? 0x5B : 0x5A;  0x59 -> reach 2 ? 0x5B : 0x58;  0x5A -> 0x59;
///                         0x5B -> reach 2 ? 0x58 : 0x5A;  else target high && last != 0x5F && dist <= 600 -> 0x5F;
///                         else combo != 0 -> reach 0 ? 0x5B : 0x58;  combo 0: reach 1 -> 0x59, reach 2 -> 0x5B, else 0x58
/// ground, else:           0x5D, finish end
/// air, combo < 5 - 1:     dz < -100 and +0x1AC != 0x60 -> 0x60; else last 0x5B, or (last != 0x5C and combo 0) -> 0x5C;
///                         else combo < 3 ? 0x5B : 0x59
/// air, else:              0x5E, finish end
/// ```
pub fn decide_hm(i: &HammerDecideIn) -> HammerDecision {
    let mut d = HammerDecision { kind: 0, reach: i.reach, air: i.air, finish: false, finish_end: false, fixed_dir_z: None, last_reset: false, forced_kind: refine_flow(i, 0x37) };
    if d.forced_kind == 0x37 {
        d.air = true;
        d.forced_kind = 0x60;
    }
    let mut last = i.last_kind;
    if i.combo == 0 {
        if i.dist <= 400.0 {
            if i.num_enemies > 1 {
                d.reach = 2;
            }
        } else {
            d.reach = 1;
        }
        d.last_reset = true;
        last = 0;
    }
    let reach = d.reach;
    let normal = i.combo < MAX_COMBO - FINISHERS;
    d.kind = if !d.air {
        if !normal {
            d.finish_end = true;
            0x5D
        } else {
            match last {
                0x58 => if reach != 2 { 0x5B } else { 0x5A },
                0x59 => if reach == 2 { 0x5B } else { 0x58 },
                0x5A => 0x59,
                0x5B => if reach == 2 { 0x58 } else { 0x5A },
                _ if i.target_high && last != 0x5F && i.dist <= 600.0 => 0x5F,
                _ if i.combo != 0 => if reach == 0 { 0x5B } else { 0x58 },
                _ => match reach {
                    1 => 0x59,
                    2 => 0x5B,
                    _ => 0x58,
                },
            }
        }
    } else if !normal {
        d.finish_end = true;
        0x5E
    } else if i.dz < -100.0 && i.last_at_begin != 0x60 {
        0x60
    } else if last == 0x5B || (last != 0x5C && i.combo == 0) {
        0x5C
    } else if i.combo < 3 {
        0x5B
    } else {
        0x59
    };
    d
}

/// `TresPlayerState_ComboDR::Decide` (0x141360F70, decompiled C):
///
/// ```text
/// (+0x1F4 bit 1: finish, finish end, ground: enemies < 3 ? 0x6A : 0x66, air 0x67: never set here)
/// slot124(0x37); kind = 0x68; if forced == 0x37: air = 1, forced = 0x68
/// last = pawn+0xE10; combo 0: last = 0, pawn+0xE10 = 0
/// reach (every step): dist <= 600 ? (enemies > 2 ? 2 : 0) : 1
/// combo >= 5 - 1: finish, finish end; ground: reach 2 ? 0x66 : 0x6A; air: 0x67
/// ground: target high && last != 0x69 && dist <= 400 -> 0x69 (DirZ fixed pawn+0xBAC x 200); reach 1 -> 0x62;
///         reach 2 -> last in {0x64, 0x65} ? 0x65 : 0x64; last 0x61 -> 0x64; 0x62 -> 0x63; 0x63 -> 0x61;
///         else combo != 0 ? 0x63 : 0x62
/// air:    forced 0x68: forced = 0 (pawn vt+0x1388(0, 1, 0)), 0x68; dz < -100 -> target on the ground ? 0x68 : 0x62;
///         last 0x61 -> 0x62; 0x62 -> 0x61; else combo != 0 ? 0x61 : 0x62
/// ```
pub fn decide_dr(i: &HammerDecideIn) -> HammerDecision {
    let mut d = HammerDecision { kind: 0x68, reach: i.reach, air: i.air, finish: false, finish_end: false, fixed_dir_z: None, last_reset: false, forced_kind: refine_flow(i, 0x37) };
    if d.forced_kind == 0x37 {
        d.air = true;
        d.forced_kind = 0x68;
    }
    let mut last = i.last_kind;
    if i.combo == 0 {
        last = 0;
        d.last_reset = true;
    }
    d.reach = if i.dist <= 600.0 { if i.num_enemies > 2 { 2 } else { 0 } } else { 1 };
    if i.combo >= MAX_COMBO - FINISHERS {
        (d.finish, d.finish_end) = (true, true);
        d.kind = if d.air { 0x67 } else if d.reach == 2 { 0x66 } else { 0x6A };
        return d;
    }
    d.kind = if !d.air {
        if i.target_high && last != 0x69 && i.dist <= 400.0 {
            d.fixed_dir_z = Some(crate::combat::tables::P.decide_rise_speed);
            0x69
        } else if d.reach == 1 {
            0x62
        } else if d.reach == 2 {
            if matches!(last, 0x64 | 0x65) { 0x65 } else { 0x64 }
        } else {
            match last {
                0x61 => 0x64,
                0x62 => 0x63,
                0x63 => 0x61,
                _ => if i.combo != 0 { 0x63 } else { 0x62 },
            }
        }
    } else if d.forced_kind == 0x68 {
        d.forced_kind = 0;
        0x68
    } else if i.dz < -100.0 {
        if i.target_on_ground { 0x68 } else { 0x62 }
    } else {
        match last {
            0x61 => 0x62,
            0x62 => 0x61,
            _ => if i.combo != 0 { 0x61 } else { 0x62 },
        }
    };
    d
}

// ------------------------------------------------------------------------------------------------ for hosts and INT

/// One style pawn the host draws during FINISH_DR (P5's `kh_er_render::keyblade::form_pawn`): its model, the logical clip
/// it plays, its root (world, cm) and yaw (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinisherPawn {
    pub model: &'static str,
    pub clip: &'static str,
    pub at: Vec3,
    pub yaw: f32,
}

/// The style pawns of the running FINISH_DR (from `Sora::finish()` and the form command): the Clock (w_so114, M_L421) of
/// Clock Drill from its spawn (sub-step 0) until it is cleaned up (sub-step 6), where it was spawned.
pub fn finisher_pawns(finish: Option<FinishView>, cmd: i32) -> Vec<FinisherPawn> {
    let Some(f) = finish.filter(|f| f.cmd == FINISH_DR && cmd == CHANGE_DR2) else { return Vec::new() };
    match (f.accompany, f.sub) {
        (Some((at, yaw)), 0..=5) => vec![FinisherPawn { model: "w_so114", clip: "M_L421", at, yaw }],
        _ => Vec::new(),
    }
}

// ------------------------------------------------------------------------------------------------ (AUDIT-B F-HAMMER) the cameras' events

/// (AUDIT-B F-HAMMER) Every `TresAnimNotifyState_BpEvent` of the four forms' own clips (AnimSet3 / AnimSet6 / AnimSet20 /
/// AnimSet21; listed from the generated SEQUENCES, 2026-11-03): 3401 StyleChange3, 3403, 3416 / 3417 (Giant Stamp), 6401
/// StyleChange6, 6403, 6419 (the combo finish), 6421 / 6422 / 6423 (FINISH_DR), 6424, 6483 (Drill Dash), K401 StyleChange20,
/// L401 StyleChange21, L421 (the Clock). The camera blueprints that read them: BP_TresPlayerStateEvent_camera_toy_key
/// (`ReceiveAnimNotifyStartBpEvent` @12647, `...EndBpEvent` @13065) and BP_TresPlayerStateEventCamera_Timlesskey (@909),
/// kh-camera `forms_hammer` (bytecode: build/sora_tmp/weapons/AUDIT-B/F-HAMMER/traces/bp_*_cam.txt). 3403 / 6403 / 6422 are
/// read by neither. WHO SENDS THEM (`EventKind::AnimBpEvent`, once each): AUDIT-C's generic sender `states::bp_event::emit`
/// (every BpEvent of Sora's clips while a form is held, and the end of every window it opened), and for 6483 (Drill Dive's /
/// Noise Flux's M_6483, a SHOOTFLOW clip) the shotlock's own `magic::shotlocks::rush::on_notify`. No sender of this
/// package's own (a second one doubled every event: kh-camera tests/audit_b_hammer_cameras.rs checks there is one).
pub const CAMERA_SEQUENCES: [&str; 15] = ["3401", "3403", "3416", "3417", "6401", "6403", "6419", "6421", "6422", "6423", "6424", "6483", "K401", "L401", "L421"];

// ------------------------------------------------------------------------------------------------ (AUDIT-B F-HAMMER) the weapons' own clips

/// (AUDIT-B F-HAMMER) Whether a hit of shape group `group` in form `cmd` is of a phys primitive (`m_bIsPhysAttackCollision`
/// of the form's weapon units / Clock Drill's Clock: row `phys.<form>`): the chooser 0x140628960 then spawns the entry's
/// m_DirectHitEffect too (combat/hit.rs).
pub fn group_phys(cmd: i32, group: &str) -> bool {
    form_key(cmd).is_some_and(|form| rows("phys", form).any(|r| r.flags.contains(&group)))
}

/// (AUDIT-B F-HAMMER) This package's key range in `Pawn::notify_handles` (forms/ship.rs `wnotify_pass`).
const WNOTIFY_KEY: u16 = 0x9000;

/// (AUDIT-B F-HAMMER) Whether `wnotify` row `r` may play: its form is held (the units w_so052 / w_so053 / w_so112 / w_so113,
/// the transform units w_so051 / w_so111 on their form's StyleChange, the Clock w_so114 in Clock Drill), or, for a `SHOT` row
/// (the drills' M_6481 .. M_6483 of the Rush shotlocks: no form held), the shotlock's weapon form is its `wf<N>`.
fn wnotify_live(r: &FormHammerRow, cmd: i32, shot_weapon_form: u8) -> bool {
    if r.form == "SHOT" {
        return shot_weapon_form != 0 && r.flags.get(4).and_then(|f| f.strip_prefix("wf")).and_then(|n| n.parse::<u8>().ok()) == Some(shot_weapon_form);
    }
    form_key(cmd) == Some(r.form)
}

/// (AUDIT-B F-HAMMER) After the generic notify handlers of a pose tick (sora.rs), in every state: THE FORM WEAPONS', THE
/// TRANSFORM UNITS' AND THE CLOCK'S OWN CLIPS (`wnotify.*` rows from build/sora_pack/keyblades/<model>/model.json, written
/// by tools/lib/wnotify_rows.js): KH3's weapon actors play the row of the logical name Sora plays on their own anim instance
/// (the Clock its M_L421 from StyleFinishDR Tick sub 0, with his), and its AttachEffect / AttachSound / PlaySound notifies
/// fire on THEIR mesh: the drill's rope lines on himo01 .. himo16 (M_6421 / M_6422 / M_6423), its vw_so053_fin0 on `shaft`
/// and se02005_033 x5 (M_6423), the hammer's vw_so052_chg3 / se02005_022 / se02005_025 (Giant Stamp), the transform units'
/// StyleChange3 / 6 / 20 / 21 effects and SEs, Clock Drill's toon effects (group 0x76C) on w_so113's ATK_01 / vfx_rap*,
/// the Clock's vw_so114_fns_mnk00 / edg00 / loc00 / fns_glw00 and se02011_028. Played by F-SHIP's shared pass.
pub(crate) fn on_notifies(cx: &mut Ctx) {
    let (cmd, shot) = (cx.pawn.combat.form.cmd, cx.pawn.magic.shot_weapon_form);
    if !is_hammer_family(cmd) && shot == 0 && !cx.pawn.notify_handles.iter().any(|h| h.0 & 0xF000 == WNOTIFY_KEY) {
        return;
    }
    let row = |i: usize| {
        FORM_HAMMER.get(i).filter(|r| r.what == "wnotify" && wnotify_live(r, cmd, shot)).map(|r| super::ship::WNotify { clip: r.clip, code: r.code, flags: r.flags, effects: r.effects, sounds: r.sounds, values: r.values })
    };
    super::ship::wnotify_pass(cx, WNOTIFY_KEY, FORM_HAMMER.len(), &row);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::AnimId;

    fn din(combo: i32, last: u8, reach: u8) -> HammerDecideIn {
        HammerDecideIn { combo, reach, last_kind: last, last_at_begin: last, dist: 150.0, dz: 0.0, target_high: false, target_on_ground: true, air: false, num_enemies: 1, forced_kind: 0, flow_abilities: [false; 4] }
    }

    fn chain(decide: fn(&HammerDecideIn) -> HammerDecision, base: HammerDecideIn) -> Vec<u8> {
        let (mut last, mut reach, mut out) = (0u8, 0u8, Vec::new());
        for combo in 0..MAX_COMBO {
            let d = decide(&HammerDecideIn { combo, last_kind: last, last_at_begin: last, reach, ..base });
            out.push(d.kind);
            (last, reach) = (d.kind, d.reach);
            if d.finish_end {
                break;
            }
        }
        out
    }

    /// COMBO_HM near one target on the ground: 0x58 0x5B 0x5A 0x59, then the Giant Stamp; far: the dash first.
    #[test]
    fn the_hammer_combo() {
        assert_eq!(chain(decide_hm, din(0, 0, 0)), [0x58, 0x5B, 0x5A, 0x59, 0x5D]);
        assert_eq!(chain(decide_hm, HammerDecideIn { dist: 550.0, ..din(0, 0, 0) }), [0x59, 0x58, 0x5B, 0x5A, 0x5D], "reach 1 is kept");
        assert_eq!(chain(decide_hm, HammerDecideIn { num_enemies: 3, ..din(0, 0, 0) }), [0x5B, 0x58, 0x5A, 0x59, 0x5D], "reach 2");
        // In the air: 0x5C first, then 0x5B, 0x5C, ... the air finisher; the target far below: the falling stamp, once.
        assert_eq!(chain(decide_hm, HammerDecideIn { air: true, ..din(0, 0, 0) }), [0x5C, 0x5B, 0x5C, 0x59, 0x5E]);
        let below = HammerDecideIn { air: true, dz: -300.0, ..din(0, 0, 0) };
        assert_eq!(decide_hm(&below).kind, 0x60);
        assert_eq!(decide_hm(&HammerDecideIn { last_at_begin: 0x60, last_kind: 0x60, combo: 1, ..below }).kind, 0x5B, "not right after 0x60: the air chain (combo 1 < 3, after a kind other than 0x5B / 0x5C)");
        // The target high: the rise (not twice).
        let high = HammerDecideIn { target_high: true, ..din(1, 0x5D, 0) };
        assert_eq!(decide_hm(&high).kind, 0x5F);
        assert_eq!(decide_hm(&HammerDecideIn { last_kind: 0x5F, ..high }).kind, 0x5B);
        // A flow attack refined to 0x37 becomes the falling stamp in the air.
        let d = decide_hm(&HammerDecideIn { forced_kind: 0x37, ..din(0, 0, 0) });
        assert_eq!((d.forced_kind, d.air), (0x60, true));
    }

    /// COMBO_DR near one target: 0x62 0x63 0x61 0x64, then the giga upper; three enemies: the crack chain and 0x66.
    #[test]
    fn the_drill_combo() {
        assert_eq!(chain(decide_dr, din(0, 0, 0)), [0x62, 0x63, 0x61, 0x64, 0x6A]);
        assert_eq!(chain(decide_dr, HammerDecideIn { num_enemies: 3, ..din(0, 0, 0) }), [0x64, 0x65, 0x65, 0x65, 0x66]);
        assert_eq!(chain(decide_dr, HammerDecideIn { dist: 700.0, ..din(0, 0, 0) }), [0x62, 0x62, 0x62, 0x62, 0x6A]);
        assert_eq!(chain(decide_dr, HammerDecideIn { air: true, ..din(0, 0, 0) }), [0x62, 0x61, 0x62, 0x61, 0x67]);
        let d = decide_dr(&HammerDecideIn { target_high: true, ..din(1, 0x62, 0) });
        assert_eq!((d.kind, d.fixed_dir_z), (0x69, Some(700.0)), "DirZ = pawn+0xBAC (3.5) x 200");
        assert_eq!(decide_dr(&HammerDecideIn { air: true, dz: -200.0, ..din(1, 0x62, 0) }).kind, 0x68);
        assert_eq!(decide_dr(&HammerDecideIn { air: true, dz: -200.0, target_on_ground: false, ..din(1, 0x62, 0) }).kind, 0x62);
    }

    #[test]
    fn the_sheet_carries_the_four_forms() {
        for (cmd, lo, hi) in [(CHANGE_HM, 0x58u8, 0x60u8), (CHANGE_HM2, 0x58, 0x60), (CHANGE_DR, 0x61, 0x6A), (CHANGE_DR2, 0x61, 0x6A)] {
            for kind in lo..=hi {
                let r = attack_row(cmd, kind).unwrap_or_else(|| panic!("{cmd:#x} {kind:#x}"));
                assert!(AnimId::from_name(r.clip).is_some(), "{} {}: not in the animation table", r.id, r.clip);
                assert_eq!(r.group, if is_toon(cmd) { 0x76C } else { 0 }, "{}: the toon effect group of Classic Tone", r.id);
            }
        }
        assert_eq!(replaced_clip(CHANGE_HM, "M_3411"), Some("3411"));
        assert_eq!(replaced_clip(CHANGE_DR2, "Reflec"), Some("L279"));
        assert_eq!(replaced_clip(CHANGE_HM2, "BarrierCracker"), Some("3341"));
        assert_eq!(replace_clip("M_L421"), Some("L421"));
        assert_eq!(row_rename(CHANGE_HM2, "SCHM_BREAK"), Some("HM2_BREAK"));
        assert_eq!(row_rename_kind(CHANGE_HM2, 0x5F, "SCHM_UPPER"), Some("HM2_RISE"));
        assert_eq!(row_rename_kind(CHANGE_HM2, 0x59, "SCHM_UPPER"), Some("HM2_UPPER"));
        assert_eq!(row_rename_kind(CHANGE_HM, 0x5F, "SCHM_UPPER"), Some("SCHM_RISE"));
        assert_eq!(row_rename(CHANGE_HM, "SCHM_BREAK"), None);
        assert_eq!(row_rename(CHANGE_DR2, "SCDR_DODGE"), Some("DR2_DODGE"));
        for row in ["SCHM_BREAK", "HM2_RISE", "SCDR_FINISH", "DR2_FINISH_NEEDLE"] {
            assert!(crate::combat::tables::attack_row(row).is_some(), "{row}: not in sora-combat-attack-data");
        }
        assert!(clock().is_some_and(|c| c.values.first() == Some(&10.0)), "the Clock's m_Param_MaxTime");
        // (F-SHAPES) the Clock's hulls: tres_colltest01 x (10, 10, 15) once its `wep` bone has grown to 1, FinishDR2 + 10/s up to 12.
        use crate::combat::hull::Volume;
        let at = |g: &str, t: f32| match clock_track(g).map(|k| k.placed(t, Vec3::ZERO, 0.0)) {
            Some(Volume::Hull(h)) => h,
            other => panic!("{g}: {other:?}"),
        };
        assert!((at("FinishDR", 2.0).scale - Vec3::new(10.0, 10.0, 15.0)).length() < 1.0e-3);
        assert!((at("FinishDR2", 4.4).scale - Vec3::new(12.0, 12.0, 15.0)).length() < 1.0e-3, "m_IncScale 10 / s up to 12");
    }

    /// (F-SHAPES) KH3's CONVEX shapes of the forms: hulls under their real transform. The Clock's prism (roll 90 under the
    /// `wep` bone) stands upright, 1000 wide and 750 high each way from 300 up; the old stand-in, a sphere of 1000, reached
    /// 1300 up. The drills' SwingM (LOCAL: no turn) stands at Sora's `center`; the hammer's StampL lies along the head.
    #[test]
    fn the_convex_shapes_are_hulls() {
        use crate::combat::hull::Volume;
        let Some(Volume::Hull(clock)) = clock_track("FinishDR").map(|k| k.placed(2.0, Vec3::ZERO, 0.0)) else { panic!("the Clock's hull") };
        let c = clock.centre;
        assert!((c - Vec3::new(0.0, 0.0, 300.0)).length() < 1.0, "{c}");
        assert!(clock.distance(c + Vec3::new(990.0, 0.0, 0.0)) == 0.0 && clock.distance(c + Vec3::Z * 740.0) == 0.0);
        assert!((clock.distance(c + Vec3::Z * 900.0) - 150.0).abs() < 0.5, "the top 750 above the centre: {}", clock.distance(c + Vec3::Z * 900.0));
        let shapes = |cmd: i32, g: &str, seq: &str, t: f32| volume_shapes(cmd, g, seq, t, Vec3::ZERO, 0.0).unwrap_or_else(|| panic!("{g} in {seq}"));
        let drill = shapes(CHANGE_DR, "SwingM", "6414", 1.0);
        let [Volume::Hull(swing)] = drill.as_slice() else { panic!("one hull") };
        assert!((swing.scale - Vec3::new(5.5, 5.5, 4.0)).length() < 1.0e-3 && swing.rotation.angle_between(glam::Quat::IDENTITY) < 1.0e-3, "{swing:?}");
        let hammer = shapes(CHANGE_HM2, "StampL", "3416", 2.0);
        let [Volume::Hull(stamp)] = hammer.as_slice() else { panic!("one hull") };
        assert!(stamp.scale.x > 7.9, "tres_colltest01 x 8 (x the weapon's bone scale): {stamp:?}");
        assert!(track("HM2", "3416", "StampL").is_none(), "no sphere track left for a CONVEX");
        assert!(volume_shapes(CHANGE_HM, "ImpactS", "3411", 0.5, Vec3::ZERO, 0.0).is_none(), "a SPHERE group keeps its track");
    }

    #[test]
    fn the_weapon_shapes_follow_the_clip() {
        let row = track("HM", "3411", "ImpactS").expect("Ground Wallop's impact");
        let (a, r) = track_at(row, 0.0).unwrap();
        let (b, _) = track_at(row, 0.75).unwrap();
        assert_eq!(r, 350.0);
        assert!(a.distance(b) > 50.0, "the hammer head moves: {a} -> {b}");
        assert!(track("DR", "6421", "Warhead").is_some(), "the drill's warhead");
        // (F-SHAPES) Giant Stamp's StampL (tres_colltest01 x 8) is a hull row now: `the_convex_shapes_are_hulls`.
        assert_eq!(growth("DR", "ImpactH"), (1600.0, 800.0));
    }

    /// (F-POLISH) FINISH_DR's catch link: the drill's X_link_skt has a per-frame track in both clips of the body, and in
    /// M_6423 it is out in front of him (the caught pawn is carried on the spinning drill, not held where it was hit).
    #[test]
    fn the_drill_carries_its_catch_on_x_link_skt() {
        for clip in ["6421", "6423"] {
            let row = track("DR", clip, "X_link_skt").unwrap_or_else(|| panic!("track.DR.{clip}.X_link_skt"));
            assert!(row.code > 1 && row.values.len() == row.code as usize * 4, "{clip}: per frame");
        }
        let (start, _) = track_at(track("DR", "6423", "X_link_skt").unwrap(), 0.0).unwrap();
        assert!(start.x > 300.0 && start.y.abs() < 100.0, "ahead of him: {start:?}");
    }

    /// (AUDIT-B F-HAMMER) The weapons' own clip notifies are rows (tools/lib/wnotify_rows.js): every model of the four forms
    /// has its rows on the sequence Sora plays with it, and the gate follows the held form / the shotlock's weapon form.
    #[test]
    fn the_weapons_own_clips_are_rows() {
        let w = |form: &str, model: &str| FORM_HAMMER.iter().filter(|r| r.what == "wnotify" && r.form == form && r.flags.first().is_some_and(|a| a.split(':').nth(1) == Some(model))).count();
        for (form, model, n) in [("HM", "w_so052", 8), ("DR", "w_so053", 81), ("HM2", "w_so112", 10), ("DR2", "w_so113", 81), ("HM", "w_so051", 9), ("DR", "w_so051", 9), ("HM2", "w_so111", 31), ("DR2", "w_so111", 14), ("DR2", "w_so114", 6), ("SHOT", "w_so113", 4)] {
            assert_eq!(w(form, model), n, "{form} {model}");
        }
        // Clock Drill's w_so113 plays L411 with his 6411 (both M_6411): the row is on HIS sequence
        assert!(FORM_HAMMER.iter().any(|r| r.what == "wnotify" && r.form == "DR2" && r.clip == "6411" && r.id.starts_with("wnotify.w_so113.L411.")));
        // M_6423's five se02005_033 on the drill's shaft
        assert_eq!(FORM_HAMMER.iter().filter(|r| r.what == "wnotify" && r.clip == "6423" && r.sounds.first() == Some(&"se02005_033")).count(), 5);
        let shot = FORM_HAMMER.iter().find(|r| r.what == "wnotify" && r.form == "SHOT").unwrap();
        assert!(wnotify_live(shot, 0, 0x15) && !wnotify_live(shot, 0, 0) && !wnotify_live(shot, CHANGE_DR2, 6));
        let clock = FORM_HAMMER.iter().find(|r| r.id == "wnotify.w_so114.L421.0").unwrap();
        assert!(wnotify_live(clock, CHANGE_DR2, 0) && !wnotify_live(clock, CHANGE_DR, 0));
        // the phys shapes: the chooser's direct effect
        assert!(group_phys(CHANGE_DR2, "FinishDR") && group_phys(CHANGE_DR2, "FinishDR2") && group_phys(CHANGE_HM, "SwingS"));
        assert!(!group_phys(CHANGE_HM, "StampL") && !group_phys(0, "StickM") && !group_phys(CHANGE_DR, "FinishDR"));
        // the locomotion blend spaces of AnimSet3 / AnimSet6 play as themselves (F-LOCO's contract; STATE.md's "base set" line)
        for (cmd, run, start) in [(CHANGE_HM, "RunLoopB3", "RunStartB3"), (CHANGE_DR, "RunLoopB6", "RunStartB6"), (CHANGE_HM2, "RunLoopB3", "RunStartB3"), (CHANGE_DR2, "RunLoopB6", "RunStartB6")] {
            assert_eq!(super::super::replaced_clip(cmd, "RunLoopB"), Some(run), "{cmd:#x}");
            assert_eq!(super::super::replaced_clip(cmd, "RunStartB"), Some(start), "{cmd:#x}");
        }
    }
}
