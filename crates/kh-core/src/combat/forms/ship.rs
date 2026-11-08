//! (weapons F-SHIP) Highwind (`form.HL`, CHANGE_HL 0x8C, stage 1) and Storm Flag (`form.FL`, CHANGE_FL 0x8D, stage 2,
//! FINISH_FL 0xA6): the two formchanges of Wheel of Fate (documentation/KH3_WEAPONS_PLAN.md 1.3 / 4;
//! build/sora_tmp/weapons/F-SHIP/STATE.md). The data is `sheets/port/sora-form-ship.json` (tools/sora_form_ship.js):
//! every number below that is not an address comes from it.
//!
//! WHAT KH3 DOES (decompiled C in re/decomp; addresses in the sheet and the files):
//! - THE CLASSES: pawn slot 1337 0x141432F00: form 0x8C -> TresPlayerState_ComboHL (87, `states/combo_hl.rs`), 0x8D ->
//!   TresPlayerState_ComboFL (88, `states/combo_fl.rs`), both Attack-state subclasses. MaxCombo 5, Finishers 2 (slots 928
//!   0x141433570 / 1253 0x141433A60): three steps, then two finishers.
//! - HIGHWIND (the spear, w_so092 on R_wep5_skt): ComboHL Decide 0x141361CC0 gives 0x9C .. 0x9E (M_E411 / E412 / E413), the
//!   finishers 0x9F (M_E414, the thrust) and 0xA0 (M_E416: a warp above the target, ComboHL slot 136 0x1413524A0, then the
//!   dive with the SPEAR RAIN: the clip's Shot notify (anim interface 0x141358E20) arms ComboHL slot 143 0x141364D20, which
//!   rains 15 HL_SPEAR projectiles in rings around him, 3 every 1/15 s), the rise 0xA1 (M_E417) and the dive 0xA2
//!   (M_E418; also Highwind's answer to the Sonic Cleave flow kind). The transformation StyleChange14 hits with
//!   SCHL_CHANGE / SCHL_CHANGE_END (WATER) on the transform unit w_so091 (the ship).
//! - STORM FLAG (the flag, w_so093 on R_wep6_skt): ComboFL Decide 0x141361530 gives 0xA3 (M_F411), the far flag throw 0xA4
//!   (M_F412 -> M_F413 -> M_F414, ComboFL slot 125 0x141366630), 0xA5 / 0xA6 (M_F415 / F416), the finishers 0xA7 (M_F417)
//!   and 0xA8 (M_F418: the whirlwind, SCFL_FINISH* AERO), the rise 0xA9 (M_F419). StyleChange15 hits with SCFL_CHANGE*
//!   (AERO). FINISH_FL = TresPlayerState_StyleFinishFL (39): the Kraken (w_so094) and its own hit windows
//!   (`states/combo_fl.rs`).
//! - THE START (StartStyle 0x14141A9D0 case 0x8C / 0x8D): Highwind doubles the soft target's range (ctrl+0x970 =
//!   MyAutoLockonLimit x 2: `combat/form.rs` `auto_lockon_scale_of`); Storm Flag preloads the Kraken (pawn slot 502: an
//!   asset load, the host's); both set a pawn flag 0x40000 whose reader was not found (STATE.md).
//!
//! The hit shapes of the form weapons are SHAPE TRACKS of the sheet (`hit_shape`): per frame of each clip, from FK of
//! Sora's skeleton and the weapons' own clips (tools/sora_form_ship.js).

use super::FormMoveset;
use crate::anim::SEQUENCES;
use crate::combat::form::KeybladeFormRow;
use crate::combat::hit::{spawn_form_shot, Convex, FormShot, Shot, FORM_SHOT_SEQUENCE};
use crate::host::{Attach, EffectId, EffectSpawn, SoundId, SoundPlay, TargetId};
use crate::notify::notify_asset_id;
use crate::pawn::Ctx;
use crate::states::{FinishView, StateData};
use glam::Vec3;

// The rows (sheets/port/sora-form-ship.json): `FormShipRow`, `FORM_SHIP`, `FSHIP_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_ship.rs"));

/// CHANGE_HL: Highwind.
pub const CHANGE_HL: i32 = 0x8C;
/// CHANGE_FL: Storm Flag.
pub const CHANGE_FL: i32 = 0x8D;
/// FINISH_FL (SetStyleFinishCommand 0x1413EE030 in form 0x8D).
pub const FINISH_FL: i32 = 0xA6;
/// The shot keys of this package's actors (`Window::key` = (FORM_SHOT_SEQUENCE, key)): the spears, the Kraken's shapes.
pub(crate) const SPEAR_KEY: u16 = 0xF000;
pub(crate) const KRAKEN_KEY: u16 = 0xF100;
/// Highwind's HL_Finish prism (the spear's tip in M_E416).
pub(crate) const FINISH_SHAPE_KEY: u16 = 0xF200;

/// The package: both forms, Storm Flag's finish command (STYLEFINISH 0xA6 body: `states/combo_fl.rs`).
pub(crate) const MOVESET: FormMoveset = FormMoveset {
    name: "ship",
    forms: &["form.HL", "form.FL"],
    finishes: &[FINISH_FL],
    on_start,
    on_end,
    attack,
    finish_begin: crate::states::combo_fl::finish_begin,
    finish_tick: crate::states::combo_fl::finish_tick,
    replace_clip,
    hit_shape,
    shot_motion,
    ..FormMoveset::NONE
};

/// The sheet's form key of a command: "HL" / "FL".
fn form_key(cmd: i32) -> Option<&'static str> {
    match cmd {
        CHANGE_HL => Some("HL"),
        CHANGE_FL => Some("FL"),
        _ => None,
    }
}

/// A Wheel of Fate form (Highwind or Storm Flag).
pub fn is_ship(cmd: i32) -> bool {
    form_key(cmd).is_some()
}

fn rows(what: &'static str, form: &'static str) -> impl Iterator<Item = &'static FormShipRow> {
    FORM_SHIP.iter().filter(move |r| r.what == what && r.form == form)
}

fn row(id: &str) -> Option<&'static FormShipRow> {
    FORM_SHIP.iter().find(|r| r.id == id)
}

fn value(id: &str, k: usize) -> f32 {
    row(id).and_then(|r| r.values.get(k).copied()).unwrap_or(0.0)
}

/// The row of an attack kind in a Wheel of Fate form (`attack.<form>.<kind>`).
pub fn attack_row(cmd: i32, kind: u8) -> Option<&'static FormShipRow> {
    let form = form_key(cmd)?;
    rows("attack", form).find(|r| r.code == i32::from(kind))
}

/// A flag of a row (`root_keep`, `dive`, `warp` ...).
pub(crate) fn has_flag(row: &FormShipRow, flag: &str) -> bool {
    row.flags.contains(&flag)
}

/// m_ReplaceAnimSets: the clip (animation-table row) a logical name plays as in the form (Highwind: AnimSet14; Storm Flag:
/// AnimSet15); None = the base set's row. For the pose hosts too (locomotion, guard: STATE.md).
pub fn replaced_clip(cmd: i32, logical: &str) -> Option<&'static str> {
    let form = form_key(cmd)?;
    rows("anim", form).find(|r| r.logical == logical && r.clip != "-").map(|r| r.clip)
}

/// The clip a logical name plays as in the held form, else the name itself.
pub(crate) fn clip(cmd: i32, logical: &str) -> &str {
    replaced_clip(cmd, logical).unwrap_or(logical)
}

/// The form package's `replace_clip` (the framework's own plays: the finish body of Storm Flag; Highwind has no finish
/// command). The hook gets no command: Storm Flag's set first, then Highwind's (the names they share, `StyleChangeEasy`
/// E403 / F403 ..., carry the same notifies).
fn replace_clip(logical: &str) -> Option<&'static str> {
    replaced_clip(CHANGE_FL, logical).or_else(|| replaced_clip(CHANGE_HL, logical))
}

/// StartStyle 0x14141A9D0: controller slot 738 (0x141410FD0) first resets ctrl+0x970, the soft target's range, to
/// MyAutoLockonLimit (pawn+0xC88); then case 0x8C: x 2 (`start.HL` values[0]); case 0x8D: the Kraken's asset load (pawn slot
/// 502 0x141448D30 with FINISH_FL: the host's, STATE.md) and the pawn flag 0x40000 (both forms; its reader was not found).
fn on_start(cx: &mut Ctx, row: &'static KeybladeFormRow) {
    let auto = crate::combat::targeting::AUTO_LIMIT;
    cx.pawn.combat.lockon.soft_range = if row.cmd == CHANGE_HL { auto * value("start.HL", 0) } else { auto };
}

/// EndStyle 0x14141CA20: case 0x8C deletes CHANGE_FL (the row's `end_delete`) and resets ctrl+0x970 (slot 738); case 0x8D
/// deletes FINISH_FL only (its range is MyAutoLockonLimit already: StartStyle reset it).
fn on_end(cx: &mut Ctx, row: &'static KeybladeFormRow) {
    if row.cmd == CHANGE_HL {
        cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT;
    }
}

/// The attack request in the forms: COMBO_HL / COMBO_FL (pawn slot 1337 0x141432F00: 0x8C -> 0x57, 0x8D -> 0x58).
fn attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    crate::states::combo_hl::try_attack(state, cx, target)
}

// ------------------------------------------------------------------------------------------------ the decision

/// What ComboHL / ComboFL's Decide reads (state+0x128 the combo, +0x154 the reach, pawn+0xE10 the previous kind, +0x1CC /
/// +0x1D0 the distance / height to the target, +0x1E0 target high, +0x1C4 the crowd, pawn+0xE14 the finishers played,
/// +0x1F4 bit 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipDecideIn {
    pub combo: i32,
    pub reach: u8,
    pub last_kind: u8,
    pub dist: f32,
    pub dz: f32,
    pub target_high: bool,
    pub crowd: i32,
    pub finish_count: i32,
    /// state+0x1F4 bit 1: the finish end at once (no writer of the bit was found: STATE.md).
    pub finish_now: bool,
}

/// What it leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShipDecision {
    pub kind: u8,
    pub reach: u8,
    pub finish: bool,
    pub finish_end: bool,
    pub finish_count: i32,
    /// combo 0: pawn vt+0x1D40(0) (pawn+0xE10 = 0).
    pub last_reset: bool,
    /// +0x1DD / +0x1D4: the rise's fixed DirZ (pawn+0xBAC x 200, `P.decide_rise_speed`).
    pub rise: bool,
    /// The homing target becomes the controller's current (vt+0x16F0) when it has none.
    pub homing: bool,
    /// +0x1E1: the warp of Highwind's last finisher.
    pub warp: bool,
}

/// MaxCombo / Finishers in both forms (slots 928 / 1253).
pub(crate) const MAX_COMBO: i32 = 5;
pub(crate) const FINISHERS: i32 = 2;

/// `ComboHL::Decide` (0x141361CC0, decompiled C; the sheet's row `decide.HL` carries the thresholds and the reading).
pub fn decide_hl(i: &ShipDecideIn) -> ShipDecision {
    let (near, high, low) = (value("decide.HL", 0), value("decide.HL", 1), value("decide.HL", 2));
    let mut d = ShipDecision { kind: 0, reach: i.reach, finish: false, finish_end: false, finish_count: i.finish_count, last_reset: false, rise: false, homing: false, warp: false };
    if i.finish_now {
        (d.kind, d.finish, d.finish_end, d.warp, d.homing) = (0xA0, true, true, true, true);
        return d;
    }
    let mut last = i.last_kind;
    if i.combo == 0 {
        if i.dist <= near {
            if i.crowd > 1 {
                d.reach = 2;
            }
        } else {
            d.reach = 0;
        }
        last = 0;
        d.last_reset = true;
    }
    if i.combo < MAX_COMBO - FINISHERS {
        let two = u8::from(d.reach == 2);
        if i.target_high && i.dz > high && last != 0xA1 && i.dist <= near {
            (d.kind, d.rise) = (0xA1, true);
        } else if !(i.dz >= low || last == 0xA2 || i.dist > near) {
            d.kind = 0xA2;
        } else {
            d.kind = match last {
                0x9C => 0x9D + two,
                0x9D => {
                    if two == 1 {
                        0x9C
                    } else {
                        0x9E
                    }
                }
                0x9E => 0x9C + two,
                _ if i.combo == 0 => 0x9C + two,
                _ => 0x9D + two,
            };
        }
        return d;
    }
    d.finish = true;
    let n = i.finish_count;
    if n < 1 && i.combo < MAX_COMBO - 1 {
        d.kind = 0x9F;
    } else {
        (d.kind, d.finish_end, d.warp, d.homing) = (0xA0, true, true, true);
    }
    d.finish_count = n + 1;
    d
}

/// `ComboFL::Decide` (0x141361530, decompiled C; the row `decide.FL`).
pub fn decide_fl(i: &ShipDecideIn) -> ShipDecision {
    let (near, high, low) = (value("decide.FL", 0), value("decide.FL", 1), value("decide.FL", 2));
    let mut d = ShipDecision { kind: 0, reach: i.reach, finish: false, finish_end: false, finish_count: i.finish_count, last_reset: false, rise: false, homing: false, warp: false };
    if i.finish_now {
        (d.kind, d.finish, d.finish_end, d.homing) = (0xA8, true, true, true);
        return d;
    }
    let mut last = i.last_kind;
    if i.combo == 0 {
        if i.dist <= near {
            if i.crowd > 1 {
                d.reach = 2;
            }
        } else {
            d.reach = 1;
        }
        last = 0;
        d.last_reset = true;
    }
    if i.combo < MAX_COMBO - FINISHERS {
        let two = d.reach == 2;
        if i.target_high && i.dz > high && last != 0xA9 && i.dist <= near {
            (d.kind, d.rise) = (0xA9, true);
        } else if !(i.dz >= low || last == 0xA3 || i.dist > near) {
            d.kind = 0xA3;
        } else {
            d.kind = match last {
                0xA3 => 0xA5 + u8::from(two),
                0xA4 => 0xA5,
                0xA5 => {
                    if two {
                        0xA3
                    } else {
                        0xA6
                    }
                }
                0xA6 => {
                    if two {
                        0xA5
                    } else {
                        0xA3
                    }
                }
                _ if i.combo == 0 => {
                    if two {
                        0xA6
                    } else {
                        0xA3 + u8::from(d.reach == 1)
                    }
                }
                _ => 0xA5,
            };
        }
        return d;
    }
    d.finish = true;
    let n = i.finish_count;
    if n < 1 && i.combo < MAX_COMBO - 1 {
        d.kind = 0xA7;
    } else {
        (d.kind, d.finish_end, d.homing) = (0xA8, true, true);
    }
    d.finish_count = n + 1;
    d
}

// ------------------------------------------------------------------------------------------------ the hit shapes

/// The form package's `hit_shape`: a window of one of the forms' clips takes its group's SHAPE TRACK (`track.<form>.
/// <sequence>.<group>`, actor space per frame) at the clip time, in the world. None = no track (or a CONVEX: those are
/// shots of the state, `convex_of`).
fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let seq = SEQUENCES.get(sequence)?;
    let track = FORM_SHIP.iter().find(|r| r.what == "track" && r.clip == seq.name && r.logical == group)?;
    let frames = usize::try_from(track.code).ok().filter(|f| *f > 0)?;
    let f = if track.length > 0.0 { (time / track.length).clamp(0.0, 1.0) * (frames - 1) as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(frames - 1), f - f.floor());
    let j = (i + 1).min(frames - 1);
    let at = |k: usize| {
        let (a, b) = (track.values.get(i * 7 + k).copied()?, track.values.get(j * 7 + k).copied()?);
        Some(a + (b - a) * t)
    };
    let r = at(6)?;
    if r < 0.0 {
        return None;
    }
    let pawn = &*cx.pawn;
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    let world = |x: f32, y: f32, z: f32| pawn.cmc.location + Vec3::new(x * cos - y * sin, x * sin + y * cos, z);
    Some((world(at(0)?, at(1)?, at(2)?), world(at(3)?, at(4)?, at(5)?), r))
}

/// A CONVEX group of the forms (`shape.*` rows with flags[0] CONVEX: Highwind's HL_Finish on the spear's tip, the Kraken's
/// FinishFL / FinishFL2): the prism of `tres_colltest01` under the shape's `m_Scale` (combat/hit.rs `Convex`) and its
/// relative location (values 3..5).
pub(crate) fn convex_of(group: &str) -> Option<(Convex, Vec3)> {
    let r = FORM_SHIP.iter().find(|r| r.what == "shape" && r.logical == group && r.flags.first() == Some(&"CONVEX"))?;
    let v = |k: usize| r.values.get(k).copied().unwrap_or(0.0);
    Some((Convex { scale: Vec3::new(v(6), v(7), v(8)), inc_scale: Vec3::ZERO, max_scale: Vec3::ZERO, yaw: 0.0 }, Vec3::new(v(3), v(4), v(5))))
}

/// Where a CONVEX group of a clip stands at a clip time: the track's point (actor space) in the world.
pub(crate) fn convex_point(cx: &Ctx, clip: &str, group: &str, time: f32) -> Option<Vec3> {
    let track = FORM_SHIP.iter().find(|r| r.what == "track" && r.clip == clip && r.logical == group)?;
    let frames = usize::try_from(track.code).ok().filter(|f| *f > 0)?;
    let i = if track.length > 0.0 { ((time / track.length).clamp(0.0, 1.0) * (frames - 1) as f32).round() as usize } else { 0 };
    let v = |k: usize| track.values.get(i.min(frames - 1) * 7 + k).copied().unwrap_or(0.0);
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    Some(cx.pawn.cmc.location + Vec3::new(v(0) * cos - v(1) * sin, v(0) * sin + v(1) * cos, v(2)))
}

/// A prism shot of a CONVEX group (KH3: the group's primitive while its window is open): `life` s at `at` (the shape's
/// relative location added in the actor's frame, yaw `yaw`), with the window's row and interval.
pub(crate) struct ConvexShot {
    pub key: u16,
    pub group: &'static str,
    pub row: &'static str,
    pub at: Vec3,
    pub yaw: f32,
    pub life: f32,
    pub interval: f32,
    pub hit_se: Option<&'static str>,
}

/// Spawns a [`ConvexShot`] (combat/hit.rs `spawn_form_shot` with the prism).
pub(crate) fn spawn_convex(cx: &mut Ctx, c: ConvexShot) {
    let Some((mut convex, offset)) = convex_of(c.group) else { return };
    convex.yaw = c.yaw;
    let info = cx.pawn.combat.attack_ctx;
    spawn_form_shot(cx, FormShot { key: c.key, group: c.group, row: c.row, position: c.at, yaw: c.yaw, size: (0.0, 0.0, 0.0), life: c.life, hit_effect: None, hit_se: c.hit_se, info, hit_list: Vec::new() });
    if let Some(s) = cx.pawn.combat.shots.last_mut() {
        s.convex = Some(convex);
        s.offset = offset;
        s.window.interval = c.interval;
        // (AUDIT-B F-SHIP) the hit effects of the component the group belongs to (not the Kingdom Key's default)
        s.window.source = convex_hit_source(c.group);
    }
}

// ------------------------------------------------------------------------------------------------ the spear rain

/// The spear rain's numbers (`rain.HL`): interval, the last count, per wave, the angle step, radius base, radius per spear,
/// height.
pub(crate) fn rain() -> [f32; 7] {
    let mut out = [0.0; 7];
    for (k, o) in out.iter_mut().enumerate() {
        *o = value("rain.HL", k);
    }
    out
}

/// The warp of kind 0xA0 (`warp.HL.0xA0`): delay, height, drop, radius.
pub(crate) fn warp() -> [f32; 4] {
    [value("warp.HL.0xA0", 0), value("warp.HL.0xA0", 1), value("warp.HL.0xA0", 2), value("warp.HL.0xA0", 3)]
}

/// (AUDIT-B F-SHIP) The warp's aura group `group` (11 its head, 12 its end): the anim interface's vt+0x1A0(m_CmnAuraEffect,
/// group, 0) with Sora's ead_pc_aura_com0 (rows `aura.HL.<group>.*`: vs_ddg_wrp_in01 / 03 / 04, vs_ddg_wrp_out01 / 03 / 04 on
/// vfx_center (ToAttach false: left where he vanished) and on his root).
pub(crate) fn warp_aura(cx: &mut Ctx, group: i32) {
    for r in FORM_SHIP.iter().filter(|r| r.what == "aura" && r.code == group) {
        let Some(name) = r.effects.first().copied() else { continue };
        let bone = r.flags.first().copied().filter(|s| *s != "-");
        let v = |i: usize| r.values.get(i).copied().unwrap_or(0.0);
        cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(name)), name, at: Attach::Sora { bone, offset: Vec3::new(v(0), v(1), v(2)) }, rotation: Vec3::new(v(3), v(4), v(5)), scale: 1.0, persistent: false });
    }
}

/// The projectile HL_SPEAR (`proj.HL_SPEAR`): life, speed, move start delay, attack radius, root radius.
fn spear() -> [f32; 5] {
    [value("proj.HL_SPEAR", 0), value("proj.HL_SPEAR", 1), value("proj.HL_SPEAR", 2), value("proj.HL_SPEAR", 3), value("proj.HL_SPEAR", 4)]
}

/// The spear's particle templates (bullet, hit, map hit).
fn spear_effect(k: usize) -> Option<&'static str> {
    row("proj.HL_SPEAR").and_then(|r| r.effects.get(k).copied())
}

/// Spawns one spear (pawn vt+0x1450(name, origin, `at`, (0, 0, -1), team, 0), then the shot gets the attack's counters):
/// TresProjHL_Spear: SCHL_FINISH's sphere, `m_MainLife`, not destroyed by a hit; it falls once its move delay is over
/// (`shot_motion`).
pub(crate) fn spawn_spear(cx: &mut Ctx, index: i32, at: Vec3) {
    let [life, _, _, radius, _] = spear();
    let r = row("proj.HL_SPEAR");
    let shot_row = r.and_then(|r| r.rows.first().copied()).unwrap_or("SCHL_FINISH");
    let info = cx.pawn.combat.attack_ctx;
    let key = SPEAR_KEY + u16::try_from(index.clamp(0, 0xFF)).unwrap_or(0);
    spawn_form_shot(cx, FormShot { key, group: "HL_SPEAR", row: shot_row, position: at, yaw: 0.0, size: (radius, 0.0, radius), life, hit_effect: spear_effect(1), hit_se: None, info, hit_list: Vec::new() });
    // (F-POLISH) TresProjHL_Spear's m_bAttachToHitActor (the row's `attach_to_hit_actor` flag): it sticks in the enemy it
    // hits and moves with it (combat/hit.rs `ShotStick`, generic for every target).
    if r.is_some_and(|r| r.flags.contains(&"attach_to_hit_actor")) {
        if let Some(s) = cx.pawn.combat.shots.last_mut().filter(|s| s.window.key == (FORM_SHOT_SEQUENCE, key)) {
            s.stick = Some(crate::combat::hit::ShotStick::ARMED);
        }
    }
}

/// The form package's `shot_motion` (every shot, every tick, whether a form is held or not): a spear falls at
/// `InitialSpeed` along (0, 0, -1) after `m_fMoveStartDelay` and stops on the level (its root sphere); the fall is the
/// shot's offset below its spawn point, a function of its age, so the update is the same from any caller.
fn shot_motion(cx: &mut Ctx, shot: &mut Shot) {
    let (seq, key) = shot.window.key;
    if seq != FORM_SHOT_SEQUENCE || !(SPEAR_KEY..SPEAR_KEY + 0x100).contains(&key) {
        return;
    }
    // (F-POLISH) stuck in an enemy: it moves with it (combat/hit.rs), no fall of its own.
    if shot.stick.is_some_and(|s| s.stuck()) {
        return;
    }
    let [life, speed, delay, _, root] = spear();
    let fall = speed * (shot.window.time - delay).max(0.0);
    let floor = cx.world.line_trace(shot.position, shot.position - Vec3::Z * (speed * life + root)).map(|h| (shot.position.z - h.position.z - root).max(0.0));
    let landed = floor.is_some_and(|f| fall >= f);
    let fall = floor.map_or(fall, |f| fall.min(f));
    let was = -shot.offset.z;
    shot.offset = Vec3::new(0.0, 0.0, -fall);
    if landed && was < fall - 1.0e-3 {
        // The spear sticks in the floor: its bullet and map-hit particles there.
        let at = shot.position - Vec3::Z * fall;
        for name in [spear_effect(0), spear_effect(2)].into_iter().flatten() {
            cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(name)), name, at: Attach::World(at), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
        }
    }
}

// ------------------------------------------------------------------------------------------------ the Kraken

/// The Kraken's numbers: m_Param_MaxTime (`pawn.FL.w_so094` values[0], from its blueprint).
pub(crate) fn kraken_time() -> f32 {
    value("pawn.FL.w_so094", 0)
}

/// One hit window of the Kraken's own clip: (begin, end, interval, row, group).
pub(crate) fn kraken_windows() -> Vec<(f32, f32, f32, &'static str, &'static str)> {
    let Some(r) = row("pawn.FL.w_so094") else { return Vec::new() };
    r.windows
        .iter()
        .zip(r.rows.iter().zip(r.groups.iter()))
        .filter_map(|(w, (row, group))| {
            let (_, span) = w.split_once(':')?;
            let (span, interval) = span.split_once('/').map_or((span, 0.0), |(s, i)| (s, i.parse().unwrap_or(0.0)));
            let (a, b) = span.split_once('-')?;
            Some((a.parse().ok()?, b.parse().ok()?, interval, *row, *group))
        })
        .collect()
}

/// The Kraken clip's hit sound (its windows' m_HitSEAsset). (AUDIT-B F-SHIP) Its effects and its AttachSound are fired by
/// the weapon-clip pass (`on_notifies`, rows `wnotify.w_so094.F420.*`) on its own bones.
pub(crate) fn kraken_hit_se() -> Option<&'static str> {
    row("pawn.FL.w_so094").and_then(|r| r.sounds.iter().find_map(|s| s.strip_prefix("hit@")))
}

/// The style pawn the host draws during FINISH_FL (P5's `kh_er_render::keyblade::form_pawn`): its model and the logical
/// clip it plays, at Sora (it stands where he does: StyleFinishFL Tick sub 0 moves him to it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinisherPawn {
    pub model: &'static str,
    pub clip: Option<&'static str>,
}

/// The Kraken while FINISH_FL runs its sub-steps 0 .. 5 (from `Sora::finish()`); removed at sub-step 6 (0x1413CFAF0).
pub fn finisher_pawns(finish: Option<FinishView>) -> Vec<FinisherPawn> {
    let Some(f) = finish.filter(|f| f.cmd == FINISH_FL && f.sub <= 5) else { return Vec::new() };
    let _ = f;
    row("pawn.FL.w_so094").map_or(Vec::new(), |r| vec![FinisherPawn { model: r.clip, clip: r.flags.first().copied() }])
}

// ------------------------------------------------------------------------------------------------ (AUDIT-B F-SHIP) the clips' extras

/// (AUDIT-B F-SHIP) The sequences whose `TresAnimNotifyState_BpEvent` Wheel of Fate's camera blueprint
/// BP_TresPlayerStateEventCamera_ca_key reads (`ReceiveAnimNotifyStartBpEvent` @5188, bytecode read 2026-11-03,
/// build/sora_tmp/weapons/AUDIT-B/F-SHIP/traces/bp_ca_key.txt): E401 / F401 (StyleChange14 / 15), E416 (Highwind's last
/// finisher), F418 (Storm Flag's), F420 (FINISH_FL), F481 (the Storm Flag shotlock's clip). kh-camera `forms_ship`.
pub const CAMERA_SEQUENCES: [&str; 6] = ["E401", "F401", "E416", "F418", "F420", "F481"];

/// (AUDIT-B F-SHIP) The key range of the weapons' clip states in `Pawn::notify_handles`: (0xF000 | the play's id mod 0x1000,
/// the row's index); never a sequence index (SEQUENCES has < 0xF000 rows).
const WNOTIFY_KEY: u16 = 0xF000;

/// (AUDIT-B F-SHIP) After the generic notify handlers of a pose tick (sora.rs), in every state. (The BpEvents of
/// [`CAMERA_SEQUENCES`] reach the hosts as `EventKind::AnimBpEvent` through AUDIT-C's generic sender, states/bp_event.rs,
/// while the form is held: StyleChange14 / 15 already hold it.)
/// - THE WEAPONS' OWN CLIPS (`wnotify.*` rows, from build/sora_pack/keyblades/<model>/model.json): KH3's weapon actors and
///   the Kraken play the row of Sora's logical name on their own anim instance (in step with his clip: the transform unit
///   w_so091 StyleChange14 / 15, the spear w_so092 M_E411 .., the thrown flag w_so093 M_F413 (TresAnimNotify_ThrowWeapon),
///   the Kraken w_so094 M_F420 from StyleFinishFL Tick sub 0, his own M_F420 started in the same tick), and its notifies
///   fire on THEIR mesh: effects on the weapon's own bones / sockets (anchor `kb:<model>:<mount>:<bone>`, resolved by the
///   host on the weapon's skeleton under that clip), sounds at the weapon. A state notify ends at its end time or when
///   Sora's play changes (`WhenChangeAnimationType` END: the weapon's play changes with his).
pub(crate) fn on_notifies(cx: &mut Ctx, _notifies: &crate::anim::Notifies, dt: f32) {
    weapon_notifies(cx, dt); // (AUDIT-A THROW) dt: the thrown flag's own M_F413 clock (`thrown` rows)
}

// (AUDIT-A SOUND) `mount_of` (the anchor's mount socket on Sora, where the weapon's own sounds were attached) is gone: a
// weapon's / pawn's own sound hangs on the row's very anchor now (`wnotify_pass`), and a host without that mesh falls back
// to the mount itself (kh-host audio/anchors.rs).

fn weapon_notifies(cx: &mut Ctx, dt: f32) {
    // (AUDIT-B F-HAMMER) the pass is shared (`wnotify_pass`): this package's rows under its key range
    let row = |i: usize| FORM_SHIP.get(i).filter(|r| r.what == "wnotify").map(|r| WNotify { clip: r.clip, code: r.code, flags: r.flags, effects: r.effects, sounds: r.sounds, values: r.values });
    wnotify_pass_dt(cx, WNOTIFY_KEY, FORM_SHIP.len(), &row, dt); // (AUDIT-A THROW) dt only moves `thrown` rows (the flag's M_F413)
}

/// (AUDIT-B F-HAMMER) One `wnotify` row of a form package's sheet, in F-SHIP's columns (tools/sora_form_ship.js; the hammer
/// forms' rows come from tools/lib/wnotify_rows.js): what [`wnotify_pass`] reads. `clip` = the sequence SORA plays (the
/// weapon plays its own row of the same logical name: the host's `kb:` anchor poses it by that name).
#[derive(Clone, Copy)]
pub(crate) struct WNotify {
    pub clip: &'static str,
    pub code: i32,
    pub flags: &'static [&'static str],
    pub effects: &'static [&'static str],
    pub sounds: &'static [&'static str],
    pub values: &'static [f32],
}

/// (AUDIT-B F-HAMMER) THE SHARED WEAPON-CLIP PASS (F-SHIP's, made generic): `row(i)` gives row `i` of a package's sheet
/// when it is a `wnotify` row that may play now (None otherwise: a row of a form not held ends), `count` the sheet's length,
/// `base` the package's key range in `Pawn::notify_handles` (`base | play id mod 0x1000`, the row's index; `key & 0xF000 ==
/// base`). F-SHIP 0xF000, F-HAMMER 0x9000.
pub(crate) fn wnotify_pass(cx: &mut Ctx, base: u16, count: usize, row: &dyn Fn(usize) -> Option<WNotify>) {
    wnotify_pass_dt(cx, base, count, row, 0.0); // (AUDIT-A SHIELD) the pass's body is `wnotify_pass_dt`'s (dt only for `thrown` rows)
}

/// (AUDIT-A SHIELD) Two optional row flags (any position after the group, `flags[4..]`), for the rows of a mesh that does not
/// play in step with Sora's heaviest sample; rows without them run exactly as before:
/// - `all`: the row is a notify of one SAMPLE of a blend space whose NotifyTriggerMode is AllAnimations (the engine's
///   default; UBlendSpaceBase::TickAssetPlayer 0x143F79E00): it fires while that sample is any of the samples Sora's play
///   blends above 1e-5 (`AnimRequest::active_samples`; the finisher pawns' M_B425, driven with his axes by FUN_1413E34B0),
///   and a state of it ends when the sample leaves the blend.
/// - `thrown`: the row is a notify of a THROWN part's own throw clip (`flags[1]` = its model; kh-core placement::ThrownWeapon:
///   TresAnimNotify_ThrowWeapon -> weapon vt+0x10C0 0x14178F080 plays m_WeaponAnimName from 0 at the throw, looped by
///   m_bWeaponAnimLoop): it runs on THAT clip's clock (`clip_time`, advanced by this tick's `dt` as the weapon's anim ticks),
///   whatever Sora plays, and its states end at their end time, at the loop's end, or at the recall (no ThrownWeapon).
pub(crate) fn wnotify_pass_dt(cx: &mut Ctx, base: u16, count: usize, row: &dyn Fn(usize) -> Option<WNotify>, dt: f32) {
    use crate::notify::NotifyHandle;
    let req = cx.pawn.anim_ticked;
    let seq = req.body_samples()[0].0;
    let play = base | (req.play_id & 0x0FFF) as u16;
    // (AUDIT-A SHIELD) `all` rows: the samples in the blend; `thrown` rows: the part's throw-clip clock (before, after this tick)
    let has = |r: &WNotify, flag: &str| r.flags.iter().skip(4).any(|f| *f == flag);
    let samples = req.active_samples();
    let clocks: Vec<(&'static str, f32, f32)> = cx
        .pawn
        .combat
        .thrown
        .iter()
        .map(|t| {
            let mut after = *t;
            after.time += dt;
            (t.model, t.clip_time(), after.clip_time())
        })
        .collect();
    // (AUDIT-A THROW) the part: `flags[1]` (the shield's rows), else the model of the row's `kb:<model>:..` anchor (F-SHIP's
    // rows, whose flags[1] is their end type / follow rule)
    let thrown_model = |r: &WNotify| r.flags.get(1).copied().filter(|m| clocks.iter().any(|c| c.0 == *m)).or_else(|| r.flags.first().and_then(|a| a.strip_prefix("kb:")).and_then(|a| a.split(':').next()));
    let thrown_clock = |r: &WNotify| thrown_model(r).and_then(|m| clocks.iter().find(|c| c.0 == m)).map(|c| (c.1, c.2));
    let thrown_alive = |r: &WNotify| thrown_clock(r).is_some_and(|(_, now)| now >= r.values.first().copied().unwrap_or(0.0) && now < r.values.get(1).copied().unwrap_or(0.0));
    // The states of an earlier play, or past their end, stop.
    let time = req.time;
    // (AUDIT-A ARROWGUN) an update that wrapped a loop (a looping play, `to < from`: `(from, length]` then `(0, to]`, anim.rs): the loop's
    // states all passed their end (UE: NotifyEnd at the loop's end, NotifyBegin again in the new loop: ShooterMode's M_2416)
    let wrapped = req.looping && req.notify_window.1 < req.notify_window.0;
    let mut k = 0;
    while k < cx.pawn.notify_handles.len() {
        let (key, index, handle) = cx.pawn.notify_handles[k];
        if key & 0xF000 != base {
            k += 1;
            continue;
        }
        let end = row(usize::from(index)).and_then(|r| r.values.get(1).copied()).unwrap_or(0.0);
        // (AUDIT-A SHIELD) a `thrown` row lives on its part's clock; an `all` row while its sample is in the blend
        let special = row(usize::from(index)).map(|r| (has(&r, "thrown").then(|| thrown_alive(&r)), has(&r, "all") && !samples.iter().any(|s| s.0 == r.clip)));
        if let Some((Some(alive), _)) = special {
            if alive {
                k += 1;
                continue;
            }
        } else if key == play && time < end && !wrapped && !special.is_some_and(|s| s.1) {
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
    // (AUDIT-A SHIELD) a thrown part's clock runs whether Sora's advanced or not
    if (from, to) == (0.0, 0.0) && clocks.is_empty() {
        return;
    }
    let group = cx.pawn.effect_group;
    // (AUDIT-A SHIELD) `all` rows: any sample in the blend; `thrown` rows: their part is thrown
    let wanted = |r: &WNotify| if has(r, "thrown") { thrown_clock(r).is_some() } else if has(r, "all") { samples.iter().any(|s| s.0 == r.clip) } else { r.clip == seq };
    for (index, r) in (0..count).filter_map(|i| row(i).filter(|r| wanted(r)).map(|r| (i, r))) {
        let v = |i: usize| r.values.get(i).copied().unwrap_or(0.0);
        let t = v(0);
        let state = r.flags.get(2) == Some(&"state");
        // (AUDIT-A ARROWGUN) after a wrap: the new loop's rows `(0, to]` (from its start, as UE's looped extract), and the
        // instant rows of the old loop's tail `(from, length]` (a state row there would begin and end in this update)
        let due = if let Some((a, b)) = has(&r, "thrown").then(|| thrown_clock(&r)).flatten() {
            // (AUDIT-A SHIELD) the part's own clip clock `(a, b]` (from its start on the weapon's first tick: a = 0); a state
            // row begins once (its handle runs until `thrown_alive` ends it)
            let begun = cx.pawn.notify_handles.iter().any(|h| h.0 & 0xF000 == base && usize::from(h.1) == index);
            !begun && if b >= a { t > a && t <= b || (a == 0.0 && t == 0.0) } else { t > a || t <= b }
        } else if (from, to) == (0.0, 0.0) {
            false
        } else if wrapped {
            t <= to || (!state && t > from)
        } else {
            t > from && t <= to || (from < 0.0 && t == 0.0)
        };
        if !due {
            continue;
        }
        let Some(anchor) = r.flags.first().copied() else { continue };
        let Ok(index) = u16::try_from(index) else { continue };
        // (AUDIT-A SHIELD) a thrown part's states outlive Sora's play: their own key
        let play = if has(&r, "thrown") { base | 0x0FFF } else { play };
        match r.code {
            0 => {
                let row_group = r.flags.get(3).and_then(|g| g.strip_prefix('g')).and_then(|g| g.parse::<i32>().ok()).unwrap_or(0);
                if !crate::notify::effect_group_passes(row_group, true, group) {
                    continue;
                }
                let Some(name) = r.effects.first().copied() else { continue };
                let spawn = EffectSpawn { id: EffectId(notify_asset_id(name)), name, at: Attach::Sora { bone: Some(anchor), offset: Vec3::new(v(2), v(3), v(4)) }, rotation: Vec3::new(v(5), v(6), v(7)), scale: 1.0, persistent: state };
                let handle = cx.spawn_effect(spawn);
                // (AUDIT-B W-FIN) the row's FadeIn / StartDelayTime (values [9], [10], when the row has them) and the FADEOUT
                // end type's time (values [8]): the attach row's post-spawn 0x141772CD0, every package's rows alike
                let timing = crate::host::EffectTiming { fade_in: v(9), fade_out: if r.flags.get(1) == Some(&"FADEOUT") { v(8) } else { 0.0 }, start_delay: v(10) };
                if !timing.is_zero() {
                    cx.host.effects.timing(handle, timing);
                }
                if state {
                    cx.pawn.notify_handles.push((play, index, NotifyHandle::Effect(handle)));
                }
            }
            _ => {
                let Some(name) = r.sounds.first().copied() else { continue };
                // (AUDIT-A SOUND) KH3 plays a weapon's / pawn's own AttachSound / PlaySound / PlayVoice on THAT mesh: at its
                // AttachPointName / m_SocketName (the row's `kb:<model>:<mount>:<bone>`; empty bone = the mesh's root), as its
                // effects. The hosts place it there (kh-host audio/anchors.rs, with the effect side's `kb:` points).
                let at = Attach::Sora { bone: Some(anchor), offset: Vec3::ZERO };
                let handle = cx.play_sound(SoundPlay { id: SoundId(notify_asset_id(name)), name, at, volume: 1.0, pitch: 1.0, looping: state, spatial: true });
                if state {
                    cx.pawn.notify_handles.push((play, index, NotifyHandle::Sound(handle, v(8))));
                }
            }
        }
    }
}

/// (AUDIT-B F-SHIP) The weapon component a CONVEX group of the package hits with: HL_Finish is the spear's (w_so092_Wep
/// TresAtkColl0, whose `m_HitEffects` entry `None` gives vw_so090_hit0 / vw_so090_wht0); FinishFL / FinishFL2 are the
/// Kraken's (StylePawn/w_so094 TresAtkColl0: no `m_HitEffects` entry, so KH3's 0x1406294F0 spawns no hit effect).
pub(crate) fn convex_hit_source(group: &str) -> crate::status::keyblade::HitSource {
    let blueprint = if group.starts_with("HL_") { "w_so092_Wep" } else { "w_so094" };
    crate::status::keyblade::hit_source(&[blueprint], false, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::AnimId;

    fn din(combo: i32, last: u8, reach: u8, n: i32) -> ShipDecideIn {
        ShipDecideIn { combo, reach, last_kind: last, dist: 150.0, dz: 0.0, target_high: false, crowd: 1, finish_count: n, finish_now: false }
    }

    fn chain(f: fn(&ShipDecideIn) -> ShipDecision, first: ShipDecideIn) -> Vec<u8> {
        let (mut last, mut reach, mut n, mut kinds) = (0u8, first.reach, 0, Vec::new());
        for combo in 0..MAX_COMBO {
            let d = f(&ShipDecideIn { combo, last_kind: last, reach, finish_count: n, ..first });
            kinds.push(d.kind);
            (last, reach, n) = (d.kind, d.reach, d.finish_count);
            if d.finish_end {
                break;
            }
        }
        kinds
    }

    /// Highwind near one target: E411, E412, E413, the thrust, the warp + rain; with a crowd the other order; above /
    /// below: the rise / the dive.
    #[test]
    fn the_combo_of_combo_hl() {
        assert_eq!(chain(decide_hl, din(0, 0, 0, 0)), [0x9C, 0x9D, 0x9E, 0x9F, 0xA0]);
        assert_eq!(chain(decide_hl, ShipDecideIn { crowd: 3, ..din(0, 0, 0, 0) }), [0x9D, 0x9C, 0x9E, 0x9F, 0xA0]);
        let d = decide_hl(&din(4, 0x9F, 0, 1));
        assert!(d.finish && d.finish_end && d.warp && d.homing && d.kind == 0xA0);
        let up = decide_hl(&ShipDecideIn { dz: 300.0, target_high: true, ..din(0, 0, 0, 0) });
        assert_eq!((up.kind, up.rise), (0xA1, true));
        assert_eq!(decide_hl(&ShipDecideIn { dz: -300.0, ..din(0, 0, 0, 0) }).kind, 0xA2);
        assert_eq!(decide_hl(&ShipDecideIn { dz: -300.0, ..din(1, 0xA2, 0, 0) }).kind, 0x9D, "no second dive in a row");
        assert_eq!(decide_hl(&ShipDecideIn { dist: 900.0, ..din(0, 0, 2, 0) }).reach, 0, "far: reach 0");
    }

    /// Storm Flag: near F411, F415, F416 (reach 0: 0xA3 -> 0xA5 -> 0xA6), far the flag throw first.
    #[test]
    fn the_combo_of_combo_fl() {
        assert_eq!(chain(decide_fl, din(0, 0, 0, 0)), [0xA3, 0xA5, 0xA6, 0xA7, 0xA8]);
        assert_eq!(chain(decide_fl, ShipDecideIn { dist: 800.0, ..din(0, 0, 0, 0) }), [0xA4, 0xA5, 0xA6, 0xA7, 0xA8]);
        assert_eq!(chain(decide_fl, ShipDecideIn { crowd: 2, ..din(0, 0, 0, 0) }), [0xA6, 0xA5, 0xA3, 0xA7, 0xA8]);
        assert_eq!(decide_fl(&ShipDecideIn { dz: 300.0, target_high: true, ..din(0, 0, 0, 0) }).kind, 0xA9);
    }

    #[test]
    fn the_sheet_carries_both_forms() {
        for (cmd, kinds) in [(CHANGE_HL, 0x9C..=0xA2u8), (CHANGE_FL, 0xA3..=0xA9u8)] {
            for kind in kinds {
                let r = attack_row(cmd, kind).unwrap_or_else(|| panic!("{cmd:#x} {kind:#x}"));
                assert!(AnimId::from_name(r.clip).is_some(), "{} {}: not in the animation table", r.id, r.clip);
            }
        }
        assert_eq!(replaced_clip(CHANGE_HL, "M_E411"), Some("E411"));
        assert_eq!(replaced_clip(CHANGE_FL, "M_F420"), Some("F420"));
        assert_eq!(replace_clip("M_F420"), Some("F420"));
        assert_eq!(rain(), [0.0667, 14.0, 3.0, 120.0, 150.0, 50.0, 200.0]);
        assert_eq!(spear()[..3], [5.0, 4000.0, 0.1]);
        assert_eq!(kraken_time(), 10.0);
        let w = kraken_windows();
        assert_eq!(w.len(), 3);
        assert_eq!((w[1].2, w[1].3, w[1].4), (0.6, "SCFL_KRAKEN2", "FinishFL2"));
        let (c, off) = convex_of("FinishFL2").unwrap();
        assert_eq!((c.scale, off), (Vec3::new(20.0, 20.0, 12.0), Vec3::new(0.0, 0.0, 400.0)));
        assert!(convex_of("KB_StickM").is_none());
        assert!(attack_row(CHANGE_HL, 0xA0).is_some_and(|r| has_flag(r, "warp") && has_flag(r, "shot@0.3333@HL_SPEAR")));
        assert!(["M_F413", "M_F414"].iter().all(|l| rows("chain", "FL").any(|r| r.logical == *l)));
    }

    // Every hit window of the forms' clips has a shape track, and the tracks stand where the weapon is (within 25 m).
    #[test]
    fn every_window_has_a_track() {
        for r in FORM_SHIP.iter().filter(|r| matches!(r.what, "attack" | "chain")) {
            for g in r.groups {
                assert!(FORM_SHIP.iter().any(|t| t.what == "track" && t.clip == r.clip && t.logical == *g), "{} {g}", r.id);
            }
        }
        for t in FORM_SHIP.iter().filter(|r| r.what == "track") {
            assert_eq!(t.values.len(), t.code as usize * 7, "{}", t.id);
            assert!(t.values.chunks(7).all(|v| v[..6].iter().all(|x| x.abs() < 2500.0)), "{}", t.id);
        }
    }
}
