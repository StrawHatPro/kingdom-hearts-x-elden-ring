//! (weapons F-CLAWS) Agile Claws (`form.CL`, CHANGE_CL 0x8A, Happy Gear) -> Twin Yo-Yos (`form.YO`, CHANGE_YO 0x82) and
//! Blizzard Claws (`form.CL2`, CHANGE_CL2 0x90, Crystal Snow) -> Blizzard Blades (`form.SK`, CHANGE_SK 0x85): their form
//! package (documentation/KH3_WEAPONS_PLAN.md 1.3 / 4; build/sora_tmp/weapons/F-CLAWS/STATE.md). The data is
//! `sheets/port/sora-form-claws.json` (tools/sora_form_claws.js): every number below that is not an address comes from it.
//!
//! WHAT KH3 DOES (decompiled C in re/decomp; addresses here, in the sheet and in states/combo_cl.rs):
//! - THE CLASSES (pawn slot 1337 0x141432F00): forms 0x8A / 0x90 -> COMBO_CL (85), 0x82 -> COMBO_YO (77), 0x85 -> COMBO_SK
//!   (80): subclasses of TresPlayerState_Attack whose own parts are Decide (below), PlayAttack's cases (the rows
//!   `attack.<form>.<kind>`), ComboCL's attack-window begin (the CL2 row rename), ComboYO's End and on-contact (Anchor
//!   Shot's pull) and ComboSK's on-contact (0xAF into 0xB0). KH3 SHARES the claws' class, clips and Decide between Agile
//!   Claws and Blizzard Claws: Blizzard Claws differ only by their weapons (w_so082 / 083), the effect group 0x7DA of the
//!   plays (the ice variants of the effects) and the CL2_* rows of its hits (more blizzard attribute).
//! - MaxCombo 0x141433570: CL / CL2 / SK 5, YO 6; Finishers 0x141433A60: CL / CL2 1, YO / SK 2.
//! - The finish commands: Agile / Blizzard Claws have none (their full gauge offers the second stage); FINISH_YO (0x9C:
//!   StyleFinish slot 149 0x1413E0CD0, the giant yo-yo M_4420) and FINISH_SK (0x9F: StyleFinishSK 0x1413D6F30, the
//!   Chandelier): bodies in states/combo_cl.rs.
//! - THE HIT SHAPES: KH3's hit windows of these clips name the form weapons' own groups (CL_RStickM, YO_RSphere,
//!   SK_RFootM ...: their TresAtkColl0). Where they are is the weapon's socket under Sora's clip and the weapon's own clip:
//!   the generator reproduces that pose (FK) per frame (the rows `track.*`); [`group_shape`] reads it for the generic
//!   hit windows of combat/hit.rs (the KH hit path: every ER enemy, its reactions, launches and juggles).

use super::FormMoveset;
use crate::combat::form::KeybladeFormRow;
use crate::combat::tables::P;
use crate::host::TargetId;
use crate::pawn::Ctx;
use crate::states::StateData;
use glam::Vec3;

// The rows (sheets/port/sora-form-claws.json): `FormClawsRow`, `FORM_CLAWS`, `FCL_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_claws.rs"));

/// CHANGE_YO: Twin Yo-Yos (Happy Gear, stage 2).
pub const CHANGE_YO: i32 = 0x82;
/// CHANGE_SK: Blizzard Blades (Crystal Snow, stage 2).
pub const CHANGE_SK: i32 = 0x85;
/// CHANGE_CL: Agile Claws (Happy Gear, stage 1).
pub const CHANGE_CL: i32 = 0x8A;
/// CHANGE_CL2: Blizzard Claws (Crystal Snow, stage 1).
pub const CHANGE_CL2: i32 = 0x90;
/// FINISH_YO (SetStyleFinishCommand 0x1413EE030 in form 0x82).
pub const FINISH_YO: i32 = 0x9C;
/// FINISH_SK (in form 0x85).
pub const FINISH_SK: i32 = 0x9F;
/// The flow attack of the claw forms (ComboCL Decide: a forced 0x36 becomes it).
pub const KIND_CL_FLOW: u8 = 0x91;
/// Anchor Shot and its pull (ComboYO on-contact 0x1413561B0).
pub const KIND_YO_ANCHOR: u8 = 0x96;
pub const KIND_YO_PULL: u8 = 0x97;
/// Blizzard Blades' air step and what its hit on a tall victim becomes (ComboSK on-contact 0x141355DD0).
pub const KIND_SK_AIR: u8 = 0xAF;
pub const KIND_SK_TALL: u8 = 0xB0;

/// The package: the four forms, the two finish commands.
pub(crate) const MOVESET: FormMoveset = FormMoveset {
    name: "claws",
    forms: &["form.CL", "form.YO", "form.CL2", "form.SK"],
    finishes: &[FINISH_YO, FINISH_SK],
    on_end,
    attack,
    finish_begin: crate::states::combo_cl::finish_begin,
    finish_tick: crate::states::combo_cl::finish_tick,
    replace_clip,
    hit_shape,
    hit_volumes, // (F-SHAPES)
    row_rename,
    ..FormMoveset::NONE
};

/// The sheet's form key of a command.
pub fn form_key(cmd: i32) -> Option<&'static str> {
    match cmd {
        CHANGE_CL => Some("CL"),
        CHANGE_CL2 => Some("CL2"),
        CHANGE_YO => Some("YO"),
        CHANGE_SK => Some("SK"),
        _ => None,
    }
}

/// One of the package's forms.
pub fn is_claws(cmd: i32) -> bool {
    form_key(cmd).is_some()
}

fn rows(what: &'static str, form: &'static str) -> impl Iterator<Item = &'static FormClawsRow> {
    FORM_CLAWS.iter().filter(move |r| r.what == what && r.form == form)
}

/// The row of an attack kind in the held form (PlayAttack 0x1413378B0's case).
pub fn attack_row(cmd: i32, kind: u8) -> Option<&'static FormClawsRow> {
    let key = form_key(cmd)?;
    rows("attack", key).find(|r| r.code == i32::from(kind))
}

/// A kind of one of the package's classes (0x88 .. 0x91, 0x92 .. 0x9B, 0xAA .. 0xB4).
pub fn is_form_kind(kind: u8) -> bool {
    matches!(kind, 0x88..=0x8E | 0x91..=0x9B | 0xAA..=0xB4)
}

/// m_ReplaceAnimSets of the held form: the clip a logical name plays as (None = the base set's).
pub fn replaced_clip(cmd: i32, logical: &str) -> Option<&'static str> {
    form_key(cmd)?;
    super::replaced_clip(cmd, logical) // (F-LOCO) the one contract: the form's anim set chain (blend spaces too)
}

/// The clip a logical name plays as in the held form (the base name when the set has no row).
pub fn clip(cmd: i32, logical: &str) -> &str {
    if let Some(r) = rows("finish", form_key(cmd).unwrap_or("-")).find(|r| r.logical == logical) {
        return r.clip;
    }
    replaced_clip(cmd, logical).unwrap_or(logical)
}

/// The framework's replacement hook has no form argument (P3's API): it answers the names whose clip is the same in every
/// form of the package that has them (the transformation and finish names); a name the sets replace differently
/// (IdleB, RunLoopB ...) is left alone here and played through [`clip`] by the package's own states.
fn replace_clip(logical: &str) -> Option<&'static str> {
    let mut found: Option<&'static str> = None;
    for r in FORM_CLAWS.iter().filter(|r| (r.what == "anim" || r.what == "finish") && r.logical == logical && r.clip != "-") {
        match found {
            Some(c) if c != r.clip => return None,
            _ => found = Some(r.clip),
        }
    }
    found
}

/// The attack row a window of `row` hits with in the held form (ComboCL attack-window begin 0x141356F10: SCCL_* -> CL2_*
/// in form 0x90).
pub fn row_rename(cmd: i32, row: &str) -> Option<&'static str> {
    if cmd != CHANGE_CL2 {
        return None;
    }
    rows("rename", "CL2").find(|r| r.logical.eq_ignore_ascii_case(row)).map(|r| r.clip)
}

/// The locomotion row of the held form's set for a B-set name (`RunLoopB`, `IdleB` ...).
/// (F-LOCO) The one contract (forms `replaced`): the Idle / Run states play it through the anim player themselves.
pub fn loco_clip(cmd: i32, logical: &str) -> Option<&'static str> {
    form_key(cmd)?;
    super::replaced_clip(cmd, logical)
}

/// KH3's Run state keeps the battle stance (the weapon out, the B set) in forms 0x85 / 0x87 / 0x88 / 0x8B (Run Tick
/// 0x1413AED70: `(form - 0x85) < 7 && (0x4D >> (form - 0x85)) & 1`): of this package, Blizzard Blades (he skates).
/// (F-LOCO) The generic rule (forms `keeps_battle_stance`), which the Idle and Run states apply.
pub fn keeps_battle_stance(cmd: i32) -> bool {
    super::keeps_battle_stance(cmd)
}

/// MaxCombo (pawn slot 928 0x141433570) and Finishers (slot 1253 0x141433A60) in the package's forms.
pub fn max_combo_and_finishers(cmd: i32) -> Option<(i32, i32)> {
    match cmd {
        CHANGE_CL | CHANGE_CL2 => Some((5, 1)),
        CHANGE_YO => Some((6, 2)),
        CHANGE_SK => Some((5, 2)),
        _ => None,
    }
}

// ================================================================================================ the hit shapes

/// The shape track of a window: its clip (sequence) and group in the held form.
fn track(cmd: i32, sequence: &str, group: &str) -> Option<&'static FormClawsRow> {
    let key = form_key(cmd)?;
    rows("track", key).find(|r| r.clip == sequence && r.logical == group)
}

/// A track's capsule at a clip time (actor space): [ax, ay, az, bx, by, bz, r] per frame, linear between frames.
pub fn track_at(row: &FormClawsRow, time: f32) -> Option<(Vec3, Vec3, f32)> {
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

/// THE FORM WEAPONS' HIT SHAPES (the package's `hit_shape` hook, combat/hit.rs `shape`): a window of a group of the held
/// form's weapons, where the generator's FK put it at clip time `time` of the window's sequence, in the world: (end 1,
/// end 2, radius). None = not one of the package's tracked windows.
fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let pawn = &*cx.pawn;
    let name = crate::anim::SEQUENCES.get(sequence)?.name;
    let row = track(pawn.combat.form.cmd, name, group)?;
    let (a, b, r) = track_at(row, time)?;
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    let world = |v: Vec3| pawn.cmc.location + Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z);
    Some((world(a), world(b), r))
}

/// (F-SHAPES) The world shapes of a group with a CONVEX on the form's weapons (`vol.<form>.<sequence>.<group>.<k>` rows: Twin
/// Yo-Yos' YO_Chew, tres_colltest01 x 3 + 50/s up to (8, 8, 5) at w_so065's L_body_center; before, a sphere of its m_Size 50)
/// at a clip time, for an actor at `location` with `yaw`.
pub fn volume_shapes(cmd: i32, group: &str, sequence: &str, time: f32, location: Vec3, yaw: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let key = form_key(cmd)?;
    let out: Vec<_> = rows("vol", key).filter(|r| r.clip == sequence && r.logical == group).filter_map(|r| crate::combat::hull::VolTrack::of_row(r.flags, r.code, r.length, r.values)).map(|t| t.placed(time, location, yaw)).collect();
    (!out.is_empty()).then_some(out)
}

/// (F-SHAPES) The package's `hit_volumes` hook (combat/hit.rs `sweep_volumes`).
fn hit_volumes(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<Vec<crate::combat::hull::Volume>> {
    let name = crate::anim::SEQUENCES.get(sequence)?.name;
    volume_shapes(cx.pawn.combat.form.cmd, group, name, time, cx.pawn.cmc.location, cx.pawn.yaw)
}

// ================================================================================================ Decide

/// What the three Decides read (PreDecide / TargetMetrics, the state and the pawn; decide.rs `DecideIn`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClawsDecideIn {
    /// state+0x128.
    pub combo: i32,
    /// state+0x154.
    pub reach: u8,
    /// pawn+0xE10.
    pub last_kind: u8,
    /// state+0x1CC / +0x1D0 / +0x1E0.
    pub dist: f32,
    pub dz: f32,
    pub target_high: bool,
    /// state+0x142 after PreDecide.
    pub air: bool,
    /// state+0x1C4.
    pub num_enemies: i32,
    /// state+0x1EC bit 3 (the request came out of a wall run).
    pub wall_run: bool,
    /// The forced kind (+0x1E8) after slot 124's refine.
    pub forced_kind: u8,
    /// pawn+0xE14.
    pub finish_count: i32,
}

/// What they leave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClawsDecision {
    pub kind: u8,
    pub reach: u8,
    pub air: bool,
    /// state+0x140 / +0x141 (and pawn vt+0x2740(1)).
    pub finish: bool,
    pub finish_end: bool,
    /// state+0x1DD: DirZ fixed; with `Some` DirZ is written (pawn+0xBAC x 200 = `P.decide_rise_speed`).
    pub fixed_dir_z: bool,
    pub dir_z: Option<f32>,
    pub finish_count: i32,
    /// pawn+0xE10 = 0 (pawn vt+0x1D40(0)).
    pub last_reset: bool,
    /// +0x1E8 = 0 (the forced kind was consumed by the decision).
    pub forced_taken: bool,
}

impl ClawsDecision {
    fn of(i: &ClawsDecideIn) -> ClawsDecision {
        ClawsDecision { kind: 0, reach: i.reach, air: i.air, finish: false, finish_end: false, fixed_dir_z: false, dir_z: None, finish_count: i.finish_count, last_reset: false, forced_taken: false }
    }
}

/// `TresPlayerState_ComboCL::Decide` (slot 122 0x141360440), Agile Claws and Blizzard Claws. (+0x1F4 bit 1, its first
/// test - finisher at once - has no writer in the exe's Attack classes: text search of the decompiled 0x1413 / 0x1414
/// functions for `field_0x1f4 | 2` and `0x1ac) | 2`: never set here.)
pub fn decide_cl(i: &ClawsDecideIn) -> ClawsDecision {
    let mut d = ClawsDecision::of(i);
    // slot 124(0x36): a flow attack (forced 0x36) is the claws' own 0x91, in the air, the previous kind cleared.
    if i.forced_kind == 0x36 {
        (d.kind, d.air, d.last_reset, d.forced_taken) = (KIND_CL_FLOW, true, true, true);
        return d;
    }
    let mut last = i.last_kind;
    if i.combo == 0 {
        if i.dist > 400.0 {
            d.reach = 1;
        } else if i.num_enemies > 1 {
            d.reach = 2;
        }
        last = 0;
        d.last_reset = true;
    }
    let (max, fin) = max_combo_and_finishers(CHANGE_CL).unwrap_or((5, 1));
    let m4 = i.combo.rem_euclid(4);
    if i.wall_run {
        d.air = true;
    }
    if i.combo >= max - fin {
        (d.finish, d.finish_end, d.kind) = (true, true, 0x8C);
        return d;
    }
    if i.target_high && last != 0x8D && i.dist <= 400.0 {
        // the rise: DirZ = pawn+0xBAC x 200 only on the ground; the fixed bit always.
        if !d.air {
            d.dir_z = Some(P.decide_rise_speed);
        }
        (d.fixed_dir_z, d.kind) = (true, 0x8D);
        return d;
    }
    if i.dz < -150.0 && last != 0x8E && i.dist <= 400.0 {
        d.kind = 0x8E;
        return d;
    }
    d.kind = if m4 == 3 {
        0x8B
    } else {
        match last {
            0x88 => 0x8A,
            0x89 => 0x88,
            0x8A => 0x89,
            _ if i.combo != 0 => match d.reach {
                1 => 0x88,
                2 => 0x89,
                _ => 0x8A,
            },
            _ => match d.reach {
                1 => 0x89,
                2 => 0x8A,
                _ => 0x88,
            },
        }
    };
    d
}

/// `TresPlayerState_ComboYO::Decide` (slot 122 0x1413646B0), Twin Yo-Yos. (+0x1F4 bit 1: as in [`decide_cl`].)
pub fn decide_yo(i: &ClawsDecideIn) -> ClawsDecision {
    let mut d = ClawsDecision::of(i);
    let mut last = i.last_kind;
    if i.combo == 0 {
        if i.num_enemies > 2 {
            d.reach = 2;
        }
        last = 0;
        d.last_reset = true;
    }
    let (max, fin) = max_combo_and_finishers(CHANGE_YO).unwrap_or((6, 2));
    let m4 = i.combo.rem_euclid(4);
    if i.combo >= max - fin {
        let n = i.finish_count;
        d.finish = true;
        if n < fin - 1 && i.combo < max - 1 {
            d.kind = 0x9B;
        } else {
            (d.finish_end, d.kind) = (true, 0x9A);
        }
        d.finish_count = n + 1;
        return d;
    }
    d.kind = if m4 == 3 {
        if d.reach == 2 { 0x93 } else { 0x95 }
    } else if i.target_high && i.dz > 150.0 && last != 0x98 && i.dist <= 400.0 {
        (d.fixed_dir_z, d.dir_z) = (true, Some(P.decide_rise_speed));
        0x98
    } else if !matches!(last, 0x96 | 0x97) && i.dist > 400.0 {
        KIND_YO_ANCHOR
    } else if i.dz < -100.0 && last != 0x99 && i.dist <= 400.0 {
        0x99
    } else if d.reach == 2 {
        match m4 {
            1 => 0x92,
            2 => 0x94,
            _ => 0x95,
        }
    } else {
        match m4 {
            1 => 0x93,
            2 => 0x94,
            _ => 0x92,
        }
    };
    d
}

/// `TresPlayerState_ComboSK::Decide` (slot 122 0x141363570), Blizzard Blades.
pub fn decide_sk(i: &ClawsDecideIn) -> ClawsDecision {
    let mut d = ClawsDecision::of(i);
    let mut last = i.last_kind;
    if i.combo == 0 {
        d.reach = if i.dist > 400.0 {
            1
        } else if i.num_enemies > 1 {
            2
        } else {
            0
        };
        last = 0;
        d.last_reset = true;
    }
    let (max, fin) = max_combo_and_finishers(CHANGE_SK).unwrap_or((5, 2));
    if i.combo >= max - fin {
        let n = i.finish_count;
        d.finish = true;
        if !i.air && n < fin - 1 && i.combo < max - 1 {
            d.kind = 0xAE;
        } else {
            d.finish_end = true;
            d.kind = if i.air { 0xB2 } else { 0xB1 };
        }
        d.finish_count = n + 1;
        return d;
    }
    d.kind = if i.air {
        if i.dz >= -250.0 || i.dist > 500.0 { KIND_SK_AIR } else { 0xB4 }
    } else if last != 0xB3 && i.dz > 100.0 && i.dz > i.dist {
        0xB3
    } else {
        match last {
            0xAA => if d.reach == 2 { 0xAC } else { 0xAB },
            0xAB => 0xAC,
            0xAC => if d.reach == 0 { 0xAA } else { 0xAD },
            0xAD => if d.reach == 1 { 0xAB } else { 0xAA },
            _ if i.combo == 0 => match d.reach {
                1 => 0xAB,
                2 => 0xAD,
                _ => 0xAA,
            },
            _ => match d.reach {
                1 => 0xAC,
                2 => 0xAA,
                _ => 0xAB,
            },
        }
    };
    d
}

/// The held form's Decide.
pub fn decide(cmd: i32, i: &ClawsDecideIn) -> Option<ClawsDecision> {
    match cmd {
        CHANGE_CL | CHANGE_CL2 => Some(decide_cl(i)),
        CHANGE_YO => Some(decide_yo(i)),
        CHANGE_SK => Some(decide_sk(i)),
        _ => None,
    }
}

// ================================================================================================ the hooks

/// EndStyle's part (0x14141CA20): FINISH_SK's chandelier does not outlive the form (StyleFinishSK End 0x1413CF2D0 ->
/// 0x1413CFE10 cleans it up; a finish left early ends the form here).
fn on_end(cx: &mut Ctx, _: &'static KeybladeFormRow) {
    // (AUDIT-B F-CLAWS) the form's weapons go: their clips' live effects / sounds end with them
    wfx_end(cx, WFX_KEY);
    if cx.pawn.combat.xs.chandelier.is_some() {
        crate::states::combo_cl::chandelier_clean(cx);
    }
}

/// The attack request in the forms: TryAttack 0x141437330 with the class the form asks for (states/combo_cl.rs).
fn attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    crate::states::combo_cl::try_attack(state, cx, target)
}

/// FINISH_SK's Chandelier as the hosts draw it (KH3's StylePawn w_so088 at the spot it was spawned, its clip 7423 at the
/// finish's own clock), from the finish view (sub-steps 2 .. 5) and the chandelier record of abilities/xs.
pub fn finish_sk_pawn(finish: Option<crate::states::FinishView>, chandelier: Option<&crate::abilities::xs::chandelier::Chandelier>) -> Option<crate::abilities::xs::chandelier::ChandelierPawn> {
    let f = finish?;
    let c = chandelier?;
    let length = FORM_CLAWS.iter().find(|r| r.id == "pawn.SK.w_so088").map_or(0.0, |r| r.length);
    (f.cmd == FINISH_SK && (2..=5).contains(&f.sub) && f.root_len <= length).then_some(crate::abilities::xs::chandelier::ChandelierPawn { ground: c.ground, yaw: c.yaw, time: f.root_len })
}

/// The Chandelier pawn's row (its clip length, window, MaxTime and FinishSK2 shape).
pub(crate) fn chandelier_row() -> Option<&'static FormClawsRow> {
    FORM_CLAWS.iter().find(|r| r.id == "pawn.SK.w_so088")
}

/// (F-SHAPES) The Chandelier's FinishSK2 per frame of its 7423 (`vol.SK.7423.FinishSK2.0`: tres_colltest01 x (3, 3, 8) +
/// (30, 30, 0)/s up to 30 on its kh_sk + (0, -300, 0), roll 90 - upright, 300 above its root), in the pawn's root space:
/// combat/hull.rs `VolTrack`.
pub(crate) fn chandelier_track() -> Option<crate::combat::hull::VolTrack> {
    let r = FORM_CLAWS.iter().find(|r| r.id == "vol.SK.7423.FinishSK2.0")?;
    crate::combat::hull::VolTrack::of_row(r.flags, r.code, r.length, r.values)
}

/// The Chandelier's hit window (its clip's `TresAnimNotifyState_Attack`): (start, end, row).
pub(crate) fn chandelier_window() -> Option<(f32, f32, &'static str)> {
    let w = chandelier_row()?.windows.first()?;
    let mut p = w.split('@');
    let (a, b) = p.next()?.split_once('-')?;
    let _group = p.next()?;
    let row = p.next()?;
    Some((a.parse().ok()?, b.parse().ok()?, row))
}

// ================================================================================================ (AUDIT-B F-CLAWS) the clips' own notifies

/// (AUDIT-B F-CLAWS) THE CATCH LINK of FINISH_YO: the giant yo-yo w_so065's bone X_link01 at clip time `time` of `sequence`
/// (`track.YO.4420.X_link01`, FK as the shape tracks), in the world: where StyleFinish slot 149 (0x1413E0CD0) attaches
/// m_pCatchPawn (FUN_143E8FC60 "X_link01", relative location 0 in sub 5).
/// For an actor at `location` turned `yaw` (Sora: `cx.pawn.cmc.location`, `cx.pawn.yaw`).
pub fn catch_link(location: Vec3, yaw: f32, sequence: &str, time: f32) -> Option<Vec3> {
    let row = rows("track", "YO").find(|r| r.clip == sequence && r.logical == "X_link01")?;
    let frames = usize::try_from(row.code).ok().filter(|n| *n > 0)?;
    let at = sample(row, time, frames, 4, 0)?;
    let (sin, cos) = yaw.to_radians().sin_cos();
    Some(location + Vec3::new(at[0] * cos - at[1] * sin, at[0] * sin + at[1] * cos, at[2]))
}

/// A per-frame track's values at `time` (linear between frames): `stride` numbers a frame after `head` numbers.
fn sample(row: &FormClawsRow, time: f32, frames: usize, stride: usize, head: usize) -> Option<Vec<f32>> {
    if frames == 0 || row.values.len() < head + frames * stride {
        return None;
    }
    let last = frames - 1;
    let f = if row.length > 0.0 { (time / row.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    Some((0..stride).map(|k| row.values[head + i * stride + k] + (row.values[head + j * stride + k] - row.values[head + i * stride + k]) * t).collect())
}

// KH3: every form weapon plays its OWN clip of Sora's play (keyblade.rs `rig_of`: the clip of the same logical name, else of
// the same sequence) and the TresAnimNotify(State)_AttachEffect / SQEXSEADAnimNotify(State)_PlaySound / AttachSound notifies
// of THAT clip fire on the weapon (SQEX_ATTACH_OBJECT_OWNER = the weapon actor): effects on its sockets and bones, sounds; the
// Chandelier w_so088 the same with its 7423. The rows `wfx.<form>.<model>.<seq>.<k>` (tools/sora_form_claws.js) carry each
// notify with the per-frame transform of its socket x LocationOffset / RotationOffset in the owner's actor space (FK of Sora's
// clip -> the part's socket -> the weapon's clip). Effect rows: values = [GroupID, ScaleOffset xyz, FadeIn, FadeOut,
// StartDelayTime, per frame x y z pitch yaw roll]; sound rows: [FadeOutDuration, VolumeMultiplier, PitchMultiplier, per frame
// x y z 0 0 0]. flags = [owner, effect | sound, state | instant, attach | place, abs | rel, end type, change type, socket,
// model, the weapon's sequence]. Live ones are kept in `Pawn::notify_handles` under their own key ranges (as F-SHIP's 0xF000
// and AUDIT-A SHIELD's 0xE000 do): (WFX_KEY | play id mod 0x1000, row index) for the weapons, (PAWN_FX_KEY, row index) for
// the Chandelier; Sora's own notify keys are sequence indices (< 0xC000).
const WFX_KEY: u16 = 0xD000;
const PAWN_FX_KEY: u16 = 0xC000;
const FX_HEAD: usize = 7;
const SOUND_HEAD: usize = 3;

fn wfx_window(row: &FormClawsRow) -> (f32, f32) {
    row.windows.first().and_then(|w| w.split_once('-')).and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).unwrap_or((0.0, 0.0))
}

fn wfx_flag(row: &FormClawsRow, k: usize) -> &'static str {
    row.flags.get(k).copied().unwrap_or("-")
}

fn wfx_value(row: &FormClawsRow, k: usize) -> f32 {
    row.values.get(k).copied().unwrap_or(0.0)
}

/// (AUDIT-B W-VFX2) A TypeDataSQEX_Thunder row's host-resolved anchor (`kb:<model>:<mount>:<socket>[:to<EndSocketName>]`,
/// flags[10], tools/sora_form_claws.js via tools/lib/wnotify_rows.js `thunderAnchor`). KH3's thunder tick (0x144250690) reads
/// the beam's source / target (SetThunderAttachBones' SocketName / EndSocketName, or the bone list) from the mesh the
/// component is attached to, so the row names that mesh; the hosts resolve both ends there (kh-host `EffectHost::beam_ends`).
/// None: an ordinary row (the FK track).
pub fn wfx_anchor(row: &FormClawsRow) -> Option<&'static str> {
    row.flags.iter().skip(10).copied().find(|f| f.starts_with("kb:"))
}

/// The row's transform at its clip time `t` in its owner's actor space (yaw 0): (location, (pitch, yaw, roll) degrees).
pub fn wfx_place(row: &FormClawsRow, t: f32) -> (Vec3, Vec3) {
    let head = if wfx_flag(row, 1) == "sound" { SOUND_HEAD } else { FX_HEAD };
    let frames = usize::try_from(row.code).unwrap_or(0);
    let Some(s) = sample(row, t, frames, 6, head) else { return (Vec3::ZERO, Vec3::ZERO) };
    // the rotator: the nearest frame's (no angle blend across +-180)
    let last = frames.saturating_sub(1);
    let f = if row.length > 0.0 { ((t / row.length).clamp(0.0, 1.0) * last as f32).round() as usize } else { 0 };
    let r = &row.values[head + f.min(last) * 6 + 3..head + f.min(last) * 6 + 6];
    (Vec3::new(s[0], s[1], s[2]), Vec3::new(r[0], r[1], r[2]))
}

/// The world transform of a row at `t` for its owner standing at `at` turned `yaw` (an absolute rotation keeps the row's).
pub fn wfx_world(row: &FormClawsRow, t: f32, at: Vec3, yaw: f32) -> (Vec3, Vec3) {
    let (p, r) = wfx_place(row, t);
    let (sin, cos) = yaw.to_radians().sin_cos();
    let world = at + Vec3::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos, p.z);
    // (AUDIT-B W-FIN) every row as its data places it: the Chandelier's shatter pieces (hahen00 .. 03, socket None) too. SQEX
    // attaches socket None to the mesh COMPONENT (0x141765EC0 -> SpawnEmitterAttached 0x14411B7A0 with the row's SocketName),
    // and w_so088's mesh component is its root (no relative rotation: tools/sora_form_claws.js PAWN_BP), so the rows carry yaw
    // 0, roll 0; the roll -90 kept here before (XW-d's picture) is gone. The pieces' own emitter roll is kh-vfx's fragment rule.
    let rot = if wfx_flag(row, 4) == "abs" { r } else { Vec3::new(r.x, r.y + yaw, r.z) };
    (world, rot)
}

/// The rows of the held form's weapons' clips (claws package forms only).
pub fn weapon_fx_rows(cmd: i32) -> impl Iterator<Item = (usize, &'static FormClawsRow)> {
    let key = form_key(cmd).unwrap_or("-");
    FORM_CLAWS.iter().enumerate().filter(move |(_, r)| r.what == "wfx" && r.form == key && wfx_flag(r, 0) == "weapon")
}

/// The Chandelier's own clip rows (w_so088, 7423).
pub fn pawn_fx_rows() -> impl Iterator<Item = (usize, &'static FormClawsRow)> {
    FORM_CLAWS.iter().enumerate().filter(|(_, r)| r.what == "wfx" && wfx_flag(r, 0) == "pawn")
}

/// Starts one row for an owner at (`at`, `yaw`) at clip time `t`; Some = its handle is kept (a state, or an attached effect
/// that is followed).
fn wfx_start(cx: &mut Ctx, row: &'static FormClawsRow, t: f32, at: Vec3, yaw: f32, sora: bool) -> Option<crate::notify::NotifyHandle> {
    use crate::notify::NotifyHandle;
    let state = wfx_flag(row, 2) == "state";
    if wfx_flag(row, 1) == "sound" {
        let (p, _) = wfx_place(row, t);
        // the weapon rides on Sora: its sound is put on him at the socket's offset; the pawn's at its point
        let place = if sora { crate::host::Attach::Sora { bone: None, offset: p } } else { crate::host::Attach::World(wfx_world(row, t, at, yaw).0) };
        let h = cx.play_sound(crate::host::SoundPlay { id: crate::host::SoundId(crate::notify::notify_asset_id(row.logical)), name: row.logical, at: place, volume: wfx_value(row, 1), pitch: wfx_value(row, 2), looping: state, spatial: true });
        return state.then_some(NotifyHandle::Sound(h, wfx_value(row, 0)));
    }
    let (world, rotation) = wfx_world(row, t, at, yaw);
    let scale = Vec3::new(wfx_value(row, 1), wfx_value(row, 2), wfx_value(row, 3));
    // (AUDIT-B W-VFX2) a beam row is spawned on its weapon's own mesh (`kb:` anchor, flags[10]): the host follows the socket
    // and resolves the beam's ends there (no per-tick transform from here)
    let (at_place, spawn_rotation) = match wfx_anchor(row) {
        Some(anchor) => (crate::host::Attach::Sora { bone: Some(anchor), offset: Vec3::ZERO }, Vec3::ZERO),
        None => (crate::host::Attach::World(world), rotation),
    };
    let h = cx.spawn_effect(crate::host::EffectSpawn { id: crate::host::EffectId(crate::notify::notify_asset_id(row.logical)), name: row.logical, at: at_place, rotation: spawn_rotation, scale: scale.x, persistent: state });
    // (AUDIT-B W-FIN) the row's FadeIn / StartDelayTime, and FadeOut for the FADEOUT end type (LOOPEND / NONE never fade)
    let timing = wfx_timing(row);
    if !timing.is_zero() {
        cx.host.effects.timing(h, timing);
    }
    if scale != Vec3::splat(scale.x) && wfx_anchor(row).is_none() {
        cx.host.effects.transform(h, world, rotation, scale);
    }
    // ToAttach false: spawned at the socket and left there; else it follows its socket
    (state || wfx_flag(row, 3) == "attach").then_some(NotifyHandle::Effect(h))
}

/// (AUDIT-B W-FIN) An effect row's attach timing (values [4..7] = FadeIn, FadeOut, StartDelayTime): KH3's post-spawn
/// 0x141772CD0 takes FadeIn and StartDelayTime for every row; FadeOut is used by the FADEOUT end type only (0x141763E60).
pub fn wfx_timing(row: &FormClawsRow) -> crate::host::EffectTiming {
    let fade_out = if wfx_flag(row, 5) == "FADEOUT" { wfx_value(row, 5) } else { 0.0 };
    crate::host::EffectTiming { fade_in: wfx_value(row, 4), fade_out, start_delay: wfx_value(row, 6) }
}

fn wfx_stop(cx: &mut Ctx, row: &FormClawsRow, handle: crate::notify::NotifyHandle) {
    use crate::notify::NotifyHandle;
    match handle {
        // a state's End: the effect loop-ends (LOOPEND, not at once); an instant one's handle is only dropped (it runs out on
        // its own; an endless one is loop-ended by the host at the animation change, WhenChangeAnimationType END)
        NotifyHandle::Effect(h) if wfx_flag(row, 2) == "state" => cx.stop_effect(h, false),
        NotifyHandle::Sound(h, fade) => cx.stop_sound(h, fade),
        NotifyHandle::Effect(_) => {}
    }
}

/// Ends every live row of a key range (WFX_KEY: the weapons'; PAWN_FX_KEY: the Chandelier's).
pub(crate) fn wfx_end(cx: &mut Ctx, range: u16) {
    let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut cx.pawn.notify_handles).into_iter().partition(|h| h.0 & 0xF000 == range);
    cx.pawn.notify_handles = rest;
    for (_, index, handle) in mine {
        if let Some(row) = FORM_CLAWS.get(usize::from(index)) {
            wfx_stop(cx, row, handle);
        }
    }
}

/// Moves the live rows of a key (their owner at `at` / `yaw`, clip time `t`) and ends those whose window is over; `keep` =
/// the key that is still the owner's play (others end).
fn wfx_follow(cx: &mut Ctx, range: u16, keep: Option<u16>, t: f32, at: Vec3, yaw: f32) {
    let all = std::mem::take(&mut cx.pawn.notify_handles);
    let mut out = Vec::with_capacity(all.len());
    for (key, index, handle) in all {
        let row = FORM_CLAWS.get(usize::from(index));
        let (Some(row), true) = (row, key & 0xF000 == range) else {
            out.push((key, index, handle));
            continue;
        };
        let over = wfx_flag(row, 2) == "state" && t >= wfx_window(row).1 - 1.0e-4;
        if Some(key) != keep || over {
            wfx_stop(cx, row, handle);
            continue;
        }
        // (AUDIT-B W-VFX2) a `kb:`-anchored row is followed by the host on the weapon's mesh
        if let (crate::notify::NotifyHandle::Effect(h), None) = (handle, wfx_anchor(row)) {
            let (world, rotation) = wfx_world(row, t, at, yaw);
            cx.host.effects.transform(h, world, rotation, Vec3::new(wfx_value(row, 1), wfx_value(row, 2), wfx_value(row, 3)));
        }
        out.push((key, index, handle));
    }
    out.append(&mut cx.pawn.notify_handles);
    cx.pawn.notify_handles = out;
}

/// Whether a notify at `b` fires in the tick's window `(from, to]` (a looping play's window wraps: `to < from`).
fn fires(b: f32, from: f32, to: f32) -> bool {
    if to >= from {
        b > from && b <= to
    } else {
        b > from || b <= to
    }
}

/// (AUDIT-B F-CLAWS) After the generic notify handlers of a pose tick (kh-core sora.rs): the held claws-package form's weapons
/// play their own clips of Sora's play: their effects / sounds start in the tick's notify window (the effect-group rule with
/// the play's group: Blizzard Claws' 0x7DA rows), follow their sockets, end with their window, the play or the form.
pub(crate) fn on_notifies(cx: &mut Ctx, _notifies: &crate::anim::Notifies) {
    // the Chandelier gone (0x1413CFE10, or the form ended mid-finish): its rows end with it
    if cx.pawn.combat.xs.chandelier.is_none() {
        wfx_end(cx, PAWN_FX_KEY);
    }
    let cmd = cx.pawn.combat.form.cmd;
    let req = cx.pawn.anim_ticked;
    let play = WFX_KEY | (req.play_id & 0x0FFF) as u16;
    let (at, yaw) = (cx.pawn.cmc.location, cx.pawn.yaw);
    let held = form_key(cmd).is_some();
    wfx_follow(cx, WFX_KEY, held.then_some(play), req.time, at, yaw);
    if !held {
        return;
    }
    let seq = req.body_samples()[0].0;
    let (from, to) = req.notify_window;
    if (from, to) == (0.0, 0.0) {
        return;
    }
    let group = cx.pawn.effect_group;
    let running: Vec<u16> = cx.pawn.notify_handles.iter().filter(|h| h.0 == play).map(|h| h.1).collect();
    let start: Vec<(usize, &'static FormClawsRow)> = weapon_fx_rows(cmd)
        .filter(|(i, r)| {
            // (AUDIT-B W-DATA) ... or the play's own row: a weapon clip of one of his BLEND-SPACE rows (Twin Yo-Yos' 4321 under
            // his MagicFire4: `wfx.YO.<model>.MagicFire4.ead*`, the yo-yos' ead_w_so060_com rows) plays whatever sample weighs most
            if (r.clip != seq && r.clip != req.name.name()) || running.contains(&(*i as u16)) {
                return false;
            }
            if wfx_flag(r, 1) == "effect" && !crate::notify::effect_group_passes(wfx_value(r, 0) as i32, true, group) {
                return false;
            }
            let (b, e) = wfx_window(r);
            // UE's notify query over the tick's range: an instant one whose time is in it; a STATE one whose [begin, end)
            // holds the tick's end and that is not running (a clip started past its begin begins its states at once)
            if wfx_flag(r, 2) == "state" {
                b <= to && to < e
            } else {
                fires(b, from, to)
            }
        })
        .collect();
    for (i, r) in start {
        if let Some(h) = wfx_start(cx, r, to, at, yaw, true) {
            cx.pawn.notify_handles.push((play, i as u16, h));
        }
    }
}

/// (AUDIT-B F-CLAWS) FINISH_SK's Chandelier: its clip's own effects and sounds between its clip times `(from, to]` at its
/// foot point `ground` turned `yaw` (StyleFinishSK: the pawn plays 7423 from sub 2), the live ones followed, the states ended.
pub(crate) fn chandelier_fx(cx: &mut Ctx, ground: Vec3, yaw: f32, from: f32, to: f32) {
    wfx_follow(cx, PAWN_FX_KEY, Some(PAWN_FX_KEY), to, ground, yaw);
    let running: Vec<u16> = cx.pawn.notify_handles.iter().filter(|h| h.0 == PAWN_FX_KEY).map(|h| h.1).collect();
    let start: Vec<(usize, &'static FormClawsRow)> = pawn_fx_rows()
        .filter(|(i, r)| {
            let (b, e) = wfx_window(r);
            !running.contains(&(*i as u16)) && if wfx_flag(r, 2) == "state" { b <= to && to < e } else { fires(b, from, to) }
        })
        .collect();
    for (i, r) in start {
        if let Some(h) = wfx_start(cx, r, to, ground, yaw, false) {
            cx.pawn.notify_handles.push((PAWN_FX_KEY, i as u16, h));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn din(combo: i32, last: u8, reach: u8) -> ClawsDecideIn {
        ClawsDecideIn { combo, reach, last_kind: last, dist: 150.0, dz: 0.0, target_high: false, air: false, num_enemies: 1, wall_run: false, forced_kind: 0, finish_count: 0 }
    }

    /// A chain of decisions against a fixed situation (the pawn carries the previous kind, the reach and the finish count).
    fn chain(f: fn(&ClawsDecideIn) -> ClawsDecision, base: ClawsDecideIn, max: i32) -> Vec<u8> {
        let (mut last, mut reach, mut n) = (0u8, base.reach, 0);
        let mut out = Vec::new();
        for combo in 0..max {
            let d = f(&ClawsDecideIn { combo, last_kind: last, reach, finish_count: n, ..base });
            out.push(d.kind);
            (last, reach, n) = (d.kind, d.reach, d.finish_count);
            if d.finish_end {
                break;
            }
        }
        out
    }

    #[test]
    fn the_claw_combo_of_combo_cl() {
        // near one target: 0x88 0x8A 0x89 0x8B, then the finisher 0x8C (MaxCombo 5, one finisher)
        assert_eq!(chain(decide_cl, din(0, 0, 0), 8), [0x88, 0x8A, 0x89, 0x8B, 0x8C]);
        // far (reach 1 at the first step): 0x89 0x88 0x8A 0x8B 0x8C
        assert_eq!(chain(decide_cl, ClawsDecideIn { dist: 550.0, ..din(0, 0, 0) }, 8), [0x89, 0x88, 0x8A, 0x8B, 0x8C]);
        // a crowd (reach 2): 0x8A 0x89 0x88 0x8B 0x8C
        assert_eq!(chain(decide_cl, ClawsDecideIn { num_enemies: 3, ..din(0, 0, 0) }, 8), [0x8A, 0x89, 0x88, 0x8B, 0x8C]);
        // target high: the rise once, then the combo; target below: the falling attack once
        let high = decide_cl(&ClawsDecideIn { target_high: true, ..din(0, 0, 0) });
        assert_eq!((high.kind, high.fixed_dir_z, high.dir_z), (0x8D, true, Some(P.decide_rise_speed)));
        assert_eq!(decide_cl(&ClawsDecideIn { target_high: true, air: true, ..din(1, 0x88, 0) }).dir_z, None, "no DirZ in the air");
        assert_eq!(decide_cl(&ClawsDecideIn { target_high: true, ..din(1, 0x8D, 0) }).kind, 0x8A, "not twice");
        assert_eq!(decide_cl(&ClawsDecideIn { dz: -200.0, ..din(1, 0x88, 0) }).kind, 0x8E);
        assert_eq!(decide_cl(&ClawsDecideIn { dz: -200.0, ..din(1, 0x8E, 0) }).kind, 0x8A, "not twice");
        // the flow attack
        let flow = decide_cl(&ClawsDecideIn { forced_kind: 0x36, ..din(0, 0, 0) });
        assert_eq!((flow.kind, flow.air, flow.forced_taken), (KIND_CL_FLOW, true, true));
    }

    #[test]
    fn the_yo_yo_combo_of_combo_yo() {
        // near: 0x92 0x93 0x94 0x95, then the finishers 0x9B and 0x9A (MaxCombo 6, two finishers)
        assert_eq!(chain(decide_yo, din(0, 0, 0), 8), [0x92, 0x93, 0x94, 0x95, 0x9B, 0x9A]);
        // three targets around (reach 2): 0x95 0x92 0x94 0x93 ...
        assert_eq!(chain(decide_yo, ClawsDecideIn { num_enemies: 3, ..din(0, 0, 0) }, 8), [0x95, 0x92, 0x94, 0x93, 0x9B, 0x9A]);
        // far: Anchor Shot (not right after itself or its pull)
        assert_eq!(decide_yo(&ClawsDecideIn { dist: 600.0, ..din(0, 0, 0) }).kind, KIND_YO_ANCHOR);
        assert_eq!(decide_yo(&ClawsDecideIn { dist: 600.0, ..din(1, KIND_YO_PULL, 0) }).kind, 0x93);
        // high / below
        assert_eq!(decide_yo(&ClawsDecideIn { target_high: true, dz: 200.0, ..din(1, 0x92, 0) }).kind, 0x98);
        assert_eq!(decide_yo(&ClawsDecideIn { dz: -150.0, ..din(1, 0x92, 0) }).kind, 0x99);
    }

    #[test]
    fn the_blade_combo_of_combo_sk() {
        // ground, near: 0xAA 0xAB 0xAC, then 0xAE and the last finisher 0xB1 (MaxCombo 5, two finishers)
        assert_eq!(chain(decide_sk, din(0, 0, 0), 8), [0xAA, 0xAB, 0xAC, 0xAE, 0xB1]);
        // far: 0xAB 0xAC 0xAD ...
        assert_eq!(chain(decide_sk, ClawsDecideIn { dist: 550.0, ..din(0, 0, 0) }, 8), [0xAB, 0xAC, 0xAD, 0xAE, 0xB1]);
        // in the air: the air step, the dive far below, the air finisher at once
        assert_eq!(chain(decide_sk, ClawsDecideIn { air: true, ..din(0, 0, 0) }, 8), [KIND_SK_AIR, KIND_SK_AIR, KIND_SK_AIR, 0xB2]);
        assert_eq!(decide_sk(&ClawsDecideIn { air: true, dz: -300.0, ..din(1, 0, 0) }).kind, 0xB4);
        assert_eq!(decide_sk(&ClawsDecideIn { dz: 250.0, dist: 100.0, ..din(1, 0xAA, 0) }).kind, 0xB3, "the rise");
    }

    #[test]
    fn the_sheet_carries_the_four_forms() {
        for (cmd, kinds) in [(CHANGE_CL, 0x88..=0x91), (CHANGE_CL2, 0x88..=0x91), (CHANGE_YO, 0x92..=0x9B), (CHANGE_SK, 0xAA..=0xB4)] {
            for k in kinds.filter(|k| *k != 0x8F && *k != 0x90) {
                let row = attack_row(cmd, k).unwrap_or_else(|| panic!("{cmd:#X} kind {k:#X}"));
                assert!(row.air_mode && row.length > 0.0, "{}: every case of these classes is an air-mode attack", row.id);
                assert!(crate::anim::AnimId::from_name(row.clip).is_some(), "{}: clip {} in the animation table (FORM_ANIM_SETS)", row.id, row.clip);
            }
        }
        assert_eq!(attack_row(CHANGE_CL2, 0x88).map(|r| r.group), Some(0x7DA), "Blizzard Claws' ice effect group");
        assert_eq!(attack_row(CHANGE_CL, 0x88).map(|r| r.group), Some(0));
        assert_eq!(row_rename(CHANGE_CL2, "SCCL_CROSS"), Some("CL2_CROSS"));
        assert_eq!(row_rename(CHANGE_CL, "SCCL_CROSS"), None);
        assert_eq!((clip(CHANGE_CL, "IdleB"), clip(CHANGE_YO, "IdleB"), clip(CHANGE_SK, "IdleB")), ("C015", "4015", "7015"));
        assert_eq!(replace_clip("IdleB"), None, "differs by form: not answered by the form-less hook");
        assert_eq!(replace_clip("M_4420"), Some("4420"));
        assert!(keeps_battle_stance(CHANGE_SK) && !keeps_battle_stance(CHANGE_CL) && !keeps_battle_stance(CHANGE_YO));
        assert!(loco_clip(CHANGE_SK, "RunLoopB").is_some());
        let (a, b, w) = chandelier_window().unwrap();
        assert!((a, b, w) == (5.0, 5.6, "SCSK_CHANDELIER_END"), "{a} {b} {w}");
    }

    #[test]
    fn the_tracks_put_the_weapons_where_the_clips_swing_them() {
        // the claws' first swing: both claws within reach of his hands; the yo-yo flies far out
        let near = |cmd: i32, seq: &str, group: &str, t: f32| {
            let (a, b, r) = track_at(track(cmd, seq, group).unwrap_or_else(|| panic!("{seq} {group}")), t).unwrap();
            (a.truncate().length().max(b.truncate().length()), r)
        };
        let (d, r) = near(CHANGE_CL, "C411", "CL_RStickM", 0.3);
        assert!(d < 250.0 && r == 80.0, "{d} {r}");
        let far = (0..40).map(|k| near(CHANGE_YO, "4411", "YO_RSphere", k as f32 * 0.033).0).fold(0.0f32, f32::max);
        assert!(far > 400.0, "the yo-yo's throw reaches {far} cm");
        assert!(track(CHANGE_SK, "7411", "SK_RFootM").is_some() && track(CHANGE_CL, "7411", "SK_RFootM").is_none());
    }

    /// (F-SHAPES) Twin Yo-Yos' YO_Chew (FINISH_YO's M_4420) is a tres_colltest01 hull growing 50 a second from x 3 to (8, 8, 5)
    /// from each window's start, not a sphere of 50.
    #[test]
    fn yo_chew_is_a_growing_hull() {
        use crate::combat::hull::Volume;
        let at = |t: f32| match volume_shapes(CHANGE_YO, "YO_Chew", "4420", t, Vec3::ZERO, 0.0).as_deref() {
            Some([Volume::Hull(h)]) => *h,
            other => panic!("{other:?}"),
        };
        assert!((at(3.3333).scale - Vec3::splat(3.0)).length() < 0.05, "{:?}", at(3.3333).scale);
        assert!((at(4.5).scale - Vec3::new(8.0, 8.0, 5.0)).length() < 1.0e-3, "{:?}", at(4.5).scale);
        assert!(track(CHANGE_YO, "4420", "YO_Chew").is_none());
    }

    /// (AUDIT-B F-CLAWS) The weapons' own clips as rows: FINISH_YO's giant yo-yo (w_so065 under M_4420) carries its effects on
    /// its own bones 4 - 5 m out, and they move with it; X_link01 (the catch) rides on the same yo-yo.
    #[test]
    fn the_weapons_own_clip_notifies_are_rows_that_follow_their_bones() {
        let rows: Vec<&FormClawsRow> = weapon_fx_rows(CHANGE_YO).map(|(_, r)| r).filter(|r| r.clip == "4420").collect();
        assert!(rows.len() >= 20, "{}", rows.len());
        let body = rows.iter().find(|r| r.logical == "vw_so064_fsh_sho00" && wfx_flag(r, 7) == "L_body").expect("the yo-yo body's effect");
        assert_eq!((wfx_flag(body, 2), wfx_flag(body, 3), wfx_window(body)), ("state", "attach", (0.7678, 3.309)));
        let (a, _) = wfx_place(body, 1.0);
        let (b, _) = wfx_place(body, 3.0);
        assert!(a.truncate().length() > 300.0 && (a - b).length() > 20.0, "out on the giant yo-yo and moving with it: {a} {b}");
        let link = catch_link(Vec3::ZERO, 0.0, "4420", 2.0).expect("X_link01");
        let (edge, _) = wfx_place(rows.iter().find(|r| wfx_flag(r, 7) == "L_edge").expect("L_edge"), 2.0);
        assert!((link - edge).length() < 250.0 && link.x > 300.0, "the catch on the same yo-yo: {link} {edge}");
        // the world place turns with his yaw
        let (w, _) = wfx_world(body, 1.0, Vec3::new(100.0, 0.0, 0.0), 90.0);
        assert!((w - Vec3::new(100.0 - a.y, a.x, a.z)).length() < 1.0e-3, "{w} {a}");
        // Blizzard Claws' weapons: the ice group 0x7DA (they pass only with its plays' group)
        // ((AUDIT-B W-FIN) the transform unit w_so081's own StyleChange rows are not the claws': group 0, and Blizzard Blades has
        // only those)
        let unit = |r: &FormClawsRow| wfx_flag(r, 8) == "w_so081" || wfx_flag(r, 8) == "w_so061";
        assert!(weapon_fx_rows(CHANGE_CL2).filter(|(_, r)| wfx_flag(r, 1) == "effect" && !unit(r)).all(|(_, r)| wfx_value(r, 0) as i32 == 0x7DA));
        assert!(weapon_fx_rows(CHANGE_CL).count() > 0 && weapon_fx_rows(CHANGE_SK).filter(|(_, r)| !unit(r)).count() == 0, "Blizzard Blades' weapon clips name no effect asset (7481: trails without one)");
        assert!(weapon_fx_rows(CHANGE_SK).all(|(_, r)| r.clip == "7401") && weapon_fx_rows(CHANGE_SK).count() == 8, "(AUDIT-B W-FIN) w_so081's 7401 rows");
    }

    /// (AUDIT-B F-CLAWS) The Chandelier's 7423 from the data: 17 effects (the sparkles on its vfx_center sockets, high on its
    /// body) and its two sounds (se02008_041 at 0.0001 s, se02008_042 at 3.87 s, at its `center`).
    #[test]
    fn the_chandelier_clip_rows_carry_its_effects_and_sounds() {
        let rows: Vec<&FormClawsRow> = pawn_fx_rows().map(|(_, r)| r).collect();
        let sounds: Vec<(&str, f32)> = rows.iter().filter(|r| wfx_flag(r, 1) == "sound").map(|r| (r.logical.rsplit('.').next().unwrap_or(""), wfx_window(r).0)).collect();
        assert_eq!(sounds, [("se02008_041", 0.0001), ("se02008_042", 3.8723)]);
        assert_eq!(rows.iter().filter(|r| wfx_flag(r, 1) == "effect").count(), 17);
        let kira = rows.iter().find(|r| r.logical == "vw_so088_fin_kira00").expect("kira");
        assert!(wfx_place(kira, wfx_window(kira).0).0.z > 150.0, "on its body, not at its foot");
        let hahen = rows.iter().find(|r| r.logical == "vw_so088_fin_hahen00").expect("hahen");
        // (AUDIT-B W-FIN) the data's rotation (socket None = the mesh component = the root, yaw 0): no roll; was XW-d's -90
        assert_eq!(wfx_world(hahen, 4.84, Vec3::ZERO, 0.0).1, Vec3::ZERO);
    }
}
