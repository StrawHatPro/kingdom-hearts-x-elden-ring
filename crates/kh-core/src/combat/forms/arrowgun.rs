//! (weapons F-ARROWGUN) The gun formchanges: Double Arrowguns (`form.DW`, CHANGE_DW 0x80) and Magic Launcher (`form.BZ`,
//! CHANGE_BZ 0x83) of Shooting Star, Hunny Blasters (`form.DW2`, 0x8E) and Hunny Launcher (`form.BZ2`, 0x8F) of Hunny Spout:
//! their form package (documentation/KH3_WEAPONS_PLAN.md 1.3 / 4; build/sora_tmp/weapons/F-ARROWGUN/STATE.md). The data is
//! `sheets/port/sora-form-arrowgun.json` (tools/sora_form_arrowgun.js): every number below that is not an address or marked
//! GUESS comes from it.
//!
//! WHAT KH3 DOES (decompiled C in re/decomp):
//! - THE ATTACK CLASS (pawn slot 1337 0x141432F00): 0x80 / 0x8E -> TresPlayerState_ComboDW (id 75), 0x83 / 0x8F ->
//!   TresPlayerState_ComboBZ (id 78). Both are TresPlayerState_Attack with their own Decide (slot 122: 0x141361270 /
//!   0x1413602A0, [`decide_dw`] / [`decide_bz`]) and their own anim-interface shot handler (anim vtable 0x14690E330 /
//!   0x14690C3A8 slot 69 = +0x228: 0x1413582D0 / 0x141357FA0, `states/combo_dw.rs`); PlayAttack 0x1413378B0 has their cases
//!   0x44 .. 0x48 / 0x4A .. 0x4E (the sheet's `attack.*` rows). MaxCombo 0x141433570: 5 / 3; Finishers 0x141433A60: 2.
//!   Hunny Blasters / Launcher share the classes and the clips; their shots are the w_so040_ProjSet classes (TresProjPO_*,
//!   rows DW2_* / BZ2_*: honey) and their plays pass the effect groups 0x71E / 0x721 .. 0x723 (+0x198).
//! - THE BULLETS ([`Bullets`]): TresProjectileBase actors of the projectile sets with their ProjectileComp (speed, homing
//!   delay / min distance / turn rate and its acceleration, spin), TresAtkColl0 (one SPHERE, its growth), the respawn chain
//!   (BombShot1 -> BombShot1Ex ...: the explosion, taking over the hit list and the finish flag) and TresRoot (the level
//!   sweep). They hit through `combat::hit`'s shot pass (`hit::spawn_form_shot`): the generic KH hit path, so launches and
//!   juggles of the rows (VERTICALROLL, BLOW ...) land on every Elden Ring enemy the host's lock-on accepts.
//! - FINISH_BZ (0x9D, both launchers): StyleFinish slot 143 0x1413DF100: `M_5417`, then its end; the shot of its TresShot
//!   (StyleFinish anim interface 0x1413DD180): BZ_BIG_BANG from weapon 0's muzzle, 30 degrees up, homing to the point
//!   1000 cm ahead of Sora's centre, and the world slow ends. The carrier always becomes BigBangEx (550 -> 1500 cm).
//! - THE ANIM SETS: m_ReplaceAnimSets AnimSet2 (DW, + AnimSet16 for DW2) / AnimSet5 (BZ, + AnimSet17 for BZ2):
//!   [`replaced_clip`].
//!
//! - SHOOTERMODE (state 40: hold RT, Auto-Shot / Charged Shot): states/shooter_mode.rs; the controller's aim (the RT hold,
//!   the request, the screen-centre pick, the switch list, the multi-lock): [`aim`] (StartStyle / EndStyle: [`on_start`] /
//!   [`on_end`]).
//!
//! Not in the core (STATE.md): the shooter / multi-lock cameras and the HUD scope (the hosts read [`aim::Aim::view`]), the
//! hosts' drawing of the bullets ([`BulletView`]) and the form weapons' muzzles.

pub mod aim;

use super::FormMoveset;
use crate::combat::form::KeybladeFormRow;
use crate::combat::hit::{spawn_form_shot, FormShot, FORM_SHOT_SEQUENCE};
use crate::combat::AttackCtx;
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, TargetFlags, TargetId};
use crate::pawn::Ctx;
use crate::states::StateData;
use glam::Vec3;

// The rows (sheets/port/sora-form-arrowgun.json): `FormArrowgunRow`, `FORM_ARROWGUN`, `form_arrowgun_by_id`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_arrowgun.rs"));

/// CHANGE_DW: Double Arrowguns.
pub const CHANGE_DW: i32 = 0x80;
/// CHANGE_BZ: Magic Launcher.
pub const CHANGE_BZ: i32 = 0x83;
/// CHANGE_DW2: Hunny Blasters.
pub const CHANGE_DW2: i32 = 0x8E;
/// CHANGE_BZ2: Hunny Launcher.
pub const CHANGE_BZ2: i32 = 0x8F;
/// Barrier Cracker, the Hunny forms' guard counter (TryAttack 0x141437E1A: forced kind 0x25 in 0x8E / 0x8F; combat/guard.rs).
pub const KIND_BARRIER_CRACKER: u8 = 0x25;
/// FINISH_BZ, the launchers' finish command (SetStyleFinishCommand 0x1413EE030; Hunny Launcher too: P3's correction).
pub const FINISH_BZ: i32 = 0x9D;

/// The package: the four forms, their finish command.
pub(crate) const MOVESET: FormMoveset = FormMoveset {
    name: "arrowgun",
    forms: &["form.DW", "form.BZ", "form.DW2", "form.BZ2"],
    finishes: &[FINISH_BZ],
    attack,
    finish_begin: crate::states::combo_bz::finish_begin,
    finish_tick: crate::states::combo_bz::finish_tick,
    replace_clip,
    hit_shape: group_shape,
    row_rename,
    on_start, // (F-ARROWGUN) StartStyle 0x14141A9D0: the multi-lock in 0x80 / 0x8E
    on_end,
    ..FormMoveset::NONE
};

/// (F-ARROWGUN) StartStyle 0x14141A9D0: its common part ends the multi-lock (slot 791(1), 0x14141AE53) and the shooter aim
/// (slot 840 0x1413FB4A0); the case 0x80 / 0x8E then starts the multi-lock (slot 790 0x141419C00) and falls into 0x87's:
/// the soft-target range controller+0x970 = pawn vt+0x2980 (MyAutoLockonLimit) x 2.0; the case 0x83 / 0x8F: x 3.0.
fn on_start(cx: &mut Ctx, row: &'static KeybladeFormRow) {
    aim::multilock_end(cx);
    aim::shooter_end(cx);
    match row.cmd {
        CHANGE_DW | CHANGE_DW2 => {
            aim::multilock_start(cx);
            cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT * 2.0;
        }
        CHANGE_BZ | CHANGE_BZ2 => cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT * 3.0,
        _ => {}
    }
}

/// (F-ARROWGUN) EndStyle 0x14141CA20: slot 791 (the multi-lock ends) and slot 840 (the shooter aim ends: a running
/// ShooterMode sees it and plays its end); the soft-target range back (slot 738 0x141410FD0).
fn on_end(cx: &mut Ctx, row: &'static KeybladeFormRow) {
    aim::multilock_end(cx);
    aim::shooter_end(cx);
    if family(row.cmd).is_some() {
        cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT;
    }
    // (AUDIT-A ARROWGUN) EndStyle 0x14141CA20's last block: the state is SHOOTERMODE -> iface slot 172 Fall(state+0xA8), the
    // state's DoChain (ShooterModeEnd does not play). A running ShooterMode carries it out at its next Tick (its Begin
    // clears a stale one).
    cx.pawn.combat.arrowgun.aim.end_style_fall = true;
}

/// (AUDIT-A ARROWGUN) The weapon glow of ShooterMode's charge: pawn vt+0x1468(level) = 0x1405BDB70 -> MyEquipment+0x200
/// slot 60 0x14178A5B0 -> every weapon's +0x440 slot 60 0x14178A480(0, level): with the weapon's m_CmnChargeEffect, level
/// < 0 ends every group it started (vt+0x1A8(group, 1)), else group 0x802 + i starts for each i <= level not yet started
/// (vt+0x1A0(asset, group, 0)) and a started group above the level ends; three slots (+0x220 ..). The rows `charge.<form>
/// .<group>` (the launchers' ead_wep_chg0 / ead_wep_chg1: group 2050 only names an effect). The particle is placed on the
/// weapon's own socket (`kb:<model>:<unit socket>:<socket>`, kh-host vfx kbweapon) with the entry's LocationOffset.
pub(crate) fn charge_glow(cx: &mut Ctx, slots: &mut [Option<EffectHandle>; 3], level: i32) {
    let Some(form) = form_key(cx.pawn.combat.form.cmd) else { return };
    for (i, slot) in slots.iter_mut().enumerate() {
        let wanted = level >= 0 && i as i32 <= level;
        if !wanted {
            // EndEffect(group, 1): the group's particles end by their end type (LOOPEND)
            if let Some(h) = slot.take() {
                cx.stop_effect(h, false);
            }
            continue;
        }
        if slot.is_some() {
            continue;
        }
        let group = 0x802 + i as i32;
        // a group whose entry names no effect starts nothing (the slot stays free, as KH3's EndEffect of it is a no-op)
        let Some(row) = rows("charge").find(|r| r.form == form && r.code == group) else { continue };
        let Some(bone) = row.flags.get(4).copied() else { continue };
        let offset = Vec3::new(v(row, 0), v(row, 1), v(row, 2));
        *slot = Some(cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name: row.clip, at: Attach::Sora { bone: Some(bone), offset }, rotation: Vec3::new(v(row, 3), v(row, 4), v(row, 5)), scale: 1.0, persistent: true }));
    }
}

/// The two attack classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// COMBO_DW (75): the arrowguns (0x80, 0x8E).
    Dw,
    /// COMBO_BZ (78): the launchers (0x83, 0x8F).
    Bz,
}

/// The attack class of a form command (pawn slot 1337 0x141432F00).
pub fn family(cmd: i32) -> Option<Family> {
    match cmd {
        CHANGE_DW | CHANGE_DW2 => Some(Family::Dw),
        CHANGE_BZ | CHANGE_BZ2 => Some(Family::Bz),
        _ => None,
    }
}

/// Hunny Spout's two forms (the w_so040 projectile set, the honey rows, the effect groups).
pub fn is_hunny(cmd: i32) -> bool {
    matches!(cmd, CHANGE_DW2 | CHANGE_BZ2)
}

fn form_key(cmd: i32) -> Option<&'static str> {
    match cmd {
        CHANGE_DW => Some("DW"),
        CHANGE_BZ => Some("BZ"),
        CHANGE_DW2 => Some("DW2"),
        CHANGE_BZ2 => Some("BZ2"),
        _ => None,
    }
}

fn rows(what: &'static str) -> impl Iterator<Item = &'static FormArrowgunRow> {
    FORM_ARROWGUN.iter().filter(move |r| r.what == what)
}

/// m_ReplaceAnimSets: the clip (animation-table row) a logical name plays as in the form; None = the base set's row.
pub fn replaced_clip(cmd: i32, logical: &str) -> Option<&'static str> {
    form_key(cmd)?;
    super::replaced_clip(cmd, logical) // (F-LOCO) the one contract: the form's anim set chain (a blend space as itself)
}

/// The clip a logical name plays as in the held form, else the name itself.
pub(crate) fn clip(cmd: i32, logical: &str) -> &str {
    replaced_clip(cmd, logical).unwrap_or(logical)
}

/// The package's `replace_clip` (the framework's own plays: the transformation, FINISH_BZ's `M_5417`). The hook gets no
/// command: the launchers' map (the only finish body is theirs; the transformation clips StyleChange2 / 5 / 16 / 17 are
/// in none of the sets).
fn replace_clip(logical: &str) -> Option<&'static str> {
    replaced_clip(CHANGE_BZ, logical)
}

/// The attack row a hit window of `row` hits with (the package's `row_rename`): ComboBZ's attack-window begin 0x141356DC0
/// renames, in form 0x8F only, SCBZ_UPPER / SWING / LAUNCH / LAUNCH_END to BZ2_* (the honey rows); nothing else is renamed
/// (Hunny Blasters' shots are their own classes with DW2_* rows).
fn row_rename(cmd: i32, row: &str) -> Option<&'static str> {
    if cmd != CHANGE_BZ2 {
        return None;
    }
    match row {
        "SCBZ_UPPER" => Some("BZ2_UPPER"),
        "SCBZ_SWING" => Some("BZ2_SWING"),
        "SCBZ_LAUNCH" => Some("BZ2_LAUNCH"),
        "SCBZ_LAUNCH_END" => Some("BZ2_LAUNCH_END"),
        _ => None,
    }
}

/// One attack kind of COMBO_DW / COMBO_BZ (PlayAttack's case and the shot handler's).
pub(crate) fn attack_row(cmd: i32, kind: u8) -> Option<&'static FormArrowgunRow> {
    let form = match family(cmd)? {
        Family::Dw => "DW",
        Family::Bz => "BZ",
    };
    rows("attack").find(|r| r.form == form && r.code == i32::from(kind))
}

/// A flag of an attack row (`keep_face`, `air_mode` ...) and a `key:value` flag's value.
pub(crate) fn flag(row: &FormArrowgunRow, name: &str) -> bool {
    row.flags.contains(&name)
}

pub(crate) fn flag_value(row: &FormArrowgunRow, key: &str) -> Option<&'static str> {
    row.flags.iter().find_map(|f| f.strip_prefix(key).and_then(|v| v.strip_prefix(':')))
}

/// MaxCombo (pawn slot 928 0x141433570): 5 in the arrowguns, 3 in the launchers.
pub(crate) fn max_combo(f: Family) -> i32 {
    match f {
        Family::Dw => 5,
        Family::Bz => 3,
    }
}

// ================================================================================================ the decisions

/// What Decide reads (PreDecide 0x14134C420 has run): the state's fields and the pawn's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunDecideIn {
    /// +0x128.
    pub combo: i32,
    /// +0x154 as it stands (the previous step's reach).
    pub reach: u8,
    /// pawn+0xE10 (the previous kind; cleared at combo 0).
    pub last_kind: u8,
    /// +0x1CC / +0x1D0 / +0x1C4: distance, height difference, enemies around.
    pub dist: f32,
    pub dz: f32,
    pub num_enemies: i32,
    /// +0x1EC bit 3: requested out of a wall run (Decide: +0x142 = 1).
    pub wall_run: bool,
    pub finish_count: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunDecision {
    pub kind: u8,
    pub reach: u8,
    /// +0x142 forced on (the wall-run request).
    pub air: bool,
    pub finish: bool,
    pub finish_end: bool,
    pub finish_count: i32,
    /// combo 0: pawn vt+0x1D40(0) (pawn+0xE10 = 0).
    pub last_reset: bool,
}

/// TresPlayerState_ComboDW::Decide (0x141361270).
pub fn decide_dw(i: &GunDecideIn) -> GunDecision {
    let mut d = GunDecision { kind: 0x44, reach: i.reach, air: i.wall_run, finish: false, finish_end: false, finish_count: i.finish_count, last_reset: false };
    let mut last = i.last_kind;
    if i.combo == 0 {
        // +0x154 = dist <= 400 ? (enemies > 1 ? 2 : 0) : 1; +0x1AC = 0; pawn vt+0x1D40(0).
        d.reach = if i.dist <= 400.0 { if i.num_enemies > 1 { 2 } else { 0 } } else { 1 };
        last = 0;
        d.last_reset = true;
    }
    let m3 = i.combo.rem_euclid(3);
    if i.combo >= 3 {
        // The finisher: +0x140 = +0x141 = 1, pawn vt+0x2740(1), the finish count + 1; kind 0x47.
        d.finish = true;
        d.finish_end = true;
        d.finish_count = i.finish_count + 1;
        d.kind = 0x47;
        return d;
    }
    let spread = d.reach == 2;
    d.kind = if last != 0x48 && i.dz < -100.0 && i.dz < -i.dist {
        0x48
    } else if last == 0x44 {
        0x45 + spread as u8
    } else if last == 0x45 {
        if spread { 0x44 } else { 0x46 }
    } else if last == 0x46 {
        0x44 + spread as u8
    } else if i.combo == 0 || m3 == 0 {
        if spread { 0x46 } else { 0x44 }
    } else if m3 == 1 {
        0x45
    } else if spread {
        0x44
    } else {
        0x46
    };
    d
}

/// TresPlayerState_ComboBZ::Decide (0x1413602A0).
pub fn decide_bz(i: &GunDecideIn) -> GunDecision {
    let mut d = GunDecision { kind: 0x4A, reach: (i.dist > 600.0) as u8, air: i.wall_run, finish: false, finish_end: false, finish_count: i.finish_count, last_reset: false };
    if i.combo == 0 {
        d.last_reset = true;
    }
    let far = d.reach == 1;
    if i.combo < 2 {
        d.kind = if far {
            0x4D
        } else if i.combo.rem_euclid(2) == 1 {
            0x4B
        } else {
            0x4A
        };
        return d;
    }
    d.finish = true;
    d.finish_end = true;
    d.finish_count = i.finish_count + 1;
    d.kind = if far { 0x4E } else { 0x4C };
    d
}

// ================================================================================================ the attack request

/// The form package's `attack` hook: the attack request in the four forms queues COMBO_DW / COMBO_BZ.
fn attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    let f = family(cx.pawn.combat.form.cmd)?;
    crate::states::combo_dw::try_attack(state, cx, target, f)
}

// ================================================================================================ the bullets

/// The root sphere of a bullet that has none in its blueprint (every TresRoot of the sets has r 15).
const ROOT_RADIUS: f32 = 15.0;

/// A bullet's life cap when its blueprint gives none (`m_MainLife` 0: the shotlock's own classes; not fired here).
const NO_LIFE: f32 = 1.0e6;

fn actor_to_world(cx: &Ctx, v: Vec3) -> Vec3 {
    // (AUDIT-A ARROWGUN) the actor's pitch (ShooterMode's SetActorRotation(pitch, yaw, 0)) turns the weapons' sockets with
    // him about his capsule centre (UE's actor location), nose up positive
    let v = if cx.pawn.pitch != 0.0 {
        let cz = cx.pawn.params().capsule_center_z;
        let (sp, cp) = cx.pawn.pitch.to_radians().sin_cos();
        let d = v - Vec3::Z * cz;
        Vec3::new(d.x * cp - d.z * sp, d.y, d.x * sp + d.z * cp + cz)
    } else {
        v
    };
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    cx.pawn.cmc.location + Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z)
}

/// The per-frame track of the playing clip in the held form (`muzzle.<form>.<clip>` / `track.<form>.<clip>.<group>`):
/// the nearest frame of the clip's time; `stride` floats a frame after `skip` leading ones.
fn sample(row: &FormArrowgunRow, time: f32, skip: usize, stride: usize) -> Option<&'static [f32]> {
    let data = row.values.get(skip..)?;
    let frames = data.len() / stride;
    if frames == 0 {
        return None;
    }
    let f = if row.length > 0.0 { (time / row.length).clamp(0.0, 1.0) * (frames - 1) as f32 } else { 0.0 };
    let i = (f.round() as usize).min(frames - 1);
    data.get(i * stride..(i + 1) * stride)
}

fn track(cx: &Ctx, what: &'static str, clip: &str, group: Option<&str>) -> Option<&'static FormArrowgunRow> {
    let form = form_key(cx.pawn.combat.form.cmd)?;
    rows(what).find(|r| r.form == form && r.clip == clip && group.is_none_or(|g| r.logical == g))
}

/// The world position of weapon `index`'s muzzle (pawn vt+0x1410(i) 0x1405AA790: weapon[i]'s ATK_01; index 0 when out of
/// range), from the FK track of the playing clip (the sheet's `muzzle.*`); without one (a clip that fires nothing in
/// KH3): the actor location, as 0x1405AA790 without a weapon.
pub(crate) fn muzzle(cx: &Ctx, index: i32) -> Vec3 {
    let k = if (0..2).contains(&index) { index as usize } else { 0 };
    // (F-LOCO) the playing row's heaviest sample: a cast plays the form's blend space (MagicFire5), its track is the sample's
    let playing = cx.pawn.anim.request().body_samples()[0].0;
    match track(cx, "muzzle", playing, None).and_then(|r| sample(r, cx.pawn.anim.time, 0, 6)) {
        Some(m) => actor_to_world(cx, Vec3::new(m[k * 3], m[k * 3 + 1], m[k * 3 + 2])),
        None => cx.pawn.cmc.location,
    }
}

/// The launchers' melee shapes (w_so024 / w_so044 TresAtkColl0 StickM / StickL / EnergyRelease) at the playing clip's
/// time: (end 1, end 2, radius), world: the package's `hit_shape` hook for `combat::hit`'s windows of those groups (the
/// Kingdom Key's component has none); `sequence` = the window's clip (index into `SEQUENCES`), `time` its clip time.
fn group_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let clip = crate::anim::SEQUENCES.get(sequence)?.name;
    let row = track(cx, "track", clip, Some(group))?;
    let radius = *row.values.first()?;
    let s = sample(row, time, 1, 6)?;
    Some((actor_to_world(cx, Vec3::new(s[0], s[1], s[2])), actor_to_world(cx, Vec3::new(s[3], s[4], s[5])), radius))
}

/// Sora's centre (pawn vt+0x6E0: the lock-on point, the capsule centre).
pub(crate) fn centre(cx: &Ctx) -> Vec3 {
    cx.pawn.cmc.location + Vec3::Z * cx.pawn.params().capsule_center_z
}

/// What a bullet homes on: the homing target component (vt+0x1220) or a world point (vt+0x1210).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Homing {
    None,
    Target(TargetId),
    Point(Vec3),
}

/// One live bullet (an actor of a projectile set).
#[derive(Clone, Debug, PartialEq)]
pub struct Bullet {
    /// The shot key in `combat.shots` (`Window::key.1`).
    pub key: u16,
    pub row: &'static FormArrowgunRow,
    pub position: Vec3,
    pub velocity: Vec3,
    pub forward: Vec3,
    pub age: f32,
    pub distance: f32,
    pub homing: Homing,
    homing_on: bool,
    homing_delay: f32,
    turn_max: f32,
    spin_time: f32,
    move_delay: f32,
    /// The attack's data handed to the shot (vt+0x1130 / 0x1140 / 0x1120 / 0x1180).
    pub info: AttackCtx,
    pub effect: Option<EffectHandle>,
    pub dead: bool,
}

/// A bullet as a host draws it: its particle (the blueprint's ParticleComp template) at its place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BulletView {
    pub class: &'static str,
    pub particle: &'static str,
    pub position: Vec3,
    pub forward: Vec3,
    pub effect: Option<EffectHandle>,
    pub age: f32,
}

/// The live bullets of the gun forms (they outlive the form: a shot in flight lands after the form ended).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Bullets {
    pub list: Vec<Bullet>,
    next: u16,
    /// Bullets fired / respawned / that hit, so far (tests, the debug text).
    pub fired: u32,
    pub respawned: u32,
    pub hits: u32,
    /// The controller's aim state (the switch list, the shooter aim, the multi-lock): [`aim::Aim`].
    pub aim: aim::Aim,
}

impl Bullets {
    pub fn views(&self) -> impl Iterator<Item = BulletView> + '_ {
        self.list.iter().filter(|b| !b.dead).map(|b| BulletView { class: b.row.logical, particle: b.row.clip, position: b.position, forward: b.forward, effect: b.effect, age: b.age })
    }
}

// The numbers of a `proj.*` row (tools/sora_form_arrowgun.js `values`).
const LIFE: usize = 0;
const MAX_DISTANCE: usize = 1;
const SPEED: usize = 2;
const MAX_SPEED: usize = 3;
const HOMING: usize = 4;
const HOMING_DELAY: usize = 5;
const HOMING_MIN: usize = 6;
const TURN_MAX: usize = 7;
const TURN_ACCEL: usize = 8;
const MAX_TURN: usize = 9;
const SIZE: usize = 10;
const INC_SIZE: usize = 11;
const INC_MAX: usize = 12;
const EXPLODE_ON_TIME_OUT: usize = 14;
const RESPAWN: usize = 15;
const TAKE_HIT_LIST: usize = 16;
const NO_DESTROY_ON_HIT: usize = 18;
const SPIN: usize = 19;
const SPIN_YAW: usize = 20;
const SPIN_MAX: usize = 21;
const ROOT: usize = 22;
const ACCEL: usize = 23;
const MOVE_DELAY: usize = 24;

pub(crate) fn v(row: &FormArrowgunRow, i: usize) -> f32 {
    row.values.get(i).copied().unwrap_or(0.0)
}

fn s(row: &FormArrowgunRow, i: usize) -> Option<&'static str> {
    row.flags.get(i).copied().filter(|x| *x != "-" && *x != "None")
}

/// A bullet's persistent particle (an effect spawn of this name follows a [`BulletView`]: the hosts anchor it to the bullet,
/// as a spell actor's).
pub fn is_bullet_particle(name: &str) -> bool {
    rows("proj").any(|r| r.clip == name)
}

/// AirSlide's anim-interface TresShot handler (slot 69 0x141343E10) with flag 4 (the arrowguns' Sliding Shot / Back
/// Slide): "DW_NORMAL" from two muzzles (the notify's LocationIndex when < 2, else 0, then the next; index 2 is weapon 0),
/// each flat and outward from his centre through the muzzle, fired at once; homing on the switch list's entry
/// `shots % count` (none with an empty list). Returns the new count (+0xA0: + 1 per projectile spawned).
pub(crate) fn slide_shot(cx: &mut Ctx, location: i32, shots: i32) -> i32 {
    let first = if location < 2 { location } else { 0 };
    let c = centre(cx);
    let mut shots = shots;
    for i in 0..2 {
        let at = muzzle(cx, first + i);
        let d = (at - c).truncate().normalize_or_zero();
        let list = &cx.pawn.combat.arrowgun.aim.switch_list;
        let homing = if list.is_empty() { Homing::None } else { Homing::Target(list[shots.rem_euclid(list.len() as i32) as usize]) };
        if fire(cx, "DW_NORMAL", at, Vec3::new(d.x, d.y, 0.0), homing, AttackCtx::default()) {
            shots += 1;
        }
    }
    shots
}

/// The projectile row of a blueprint class (`TresProjSS_Curve` / `.../TresProjSS_Curve.TresProjSS_Curve_C`).
pub fn proj_row(class: &str) -> Option<&'static FormArrowgunRow> {
    let key = class.rsplit('/').next().unwrap_or(class).split('.').next().unwrap_or(class).trim_end_matches("_C");
    rows("proj").find(|r| r.logical == key)
}

/// The class a projectile NAME spawns in the held form (equipment slot 182 0x1417817F0: the form's m_ReplaceProjectileSet).
pub(crate) fn class_of(cx: &Ctx, name: &str) -> Option<&'static FormArrowgunRow> {
    proj_row(cx.pawn.combat.form.projectile(name)?)
}

/// pawn vt+0x1450(name, P, P, dir, team, fire) + the setters: a bullet of the held form's set at `at` along `dir`.
pub(crate) fn fire(cx: &mut Ctx, name: &str, at: Vec3, dir: Vec3, homing: Homing, info: AttackCtx) -> bool {
    let Some(row) = class_of(cx, name) else {
        cx.pawn.note("a gun form asked for a projectile its set does not have: nothing is fired");
        return false;
    };
    // SpawnFireEffects (slot 575): MyFireEffects at the muzzle.
    if let Some(fx) = s(row, 1) {
        let d = dir.normalize_or_zero();
        let rotation = Vec3::new(d.z.clamp(-1.0, 1.0).asin().to_degrees(), d.y.atan2(d.x).to_degrees(), 0.0);
        cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name: fx, at: Attach::World(at), rotation, scale: 1.0, persistent: false });
    }
    spawn(cx, row, at, dir, homing, info, Vec::new());
    cx.pawn.combat.arrowgun.fired += 1;
    true
}

fn spawn(cx: &mut Ctx, row: &'static FormArrowgunRow, at: Vec3, dir: Vec3, homing: Homing, info: AttackCtx, hit_list: Vec<TargetId>) {
    let d = if dir.length_squared() > 1.0e-8 { dir.normalize() } else { Vec3::X };
    let effect = (row.clip != "-").then(|| cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name: row.clip, at: Attach::World(at), rotation: Vec3::new(d.z.asin().to_degrees(), d.y.atan2(d.x).to_degrees(), 0.0), scale: 1.0, persistent: true }));
    let bullets = &mut cx.pawn.combat.arrowgun;
    bullets.next = bullets.next.wrapping_add(1);
    let key = bullets.next;
    let delay = v(row, MOVE_DELAY);
    bullets.list.push(Bullet {
        key,
        row,
        position: at,
        velocity: d * v(row, SPEED),
        forward: d,
        age: 0.0,
        distance: 0.0,
        homing,
        homing_on: v(row, HOMING) > 0.0,
        homing_delay: v(row, HOMING_DELAY),
        turn_max: v(row, TURN_MAX),
        spin_time: 0.0,
        move_delay: delay,
        info,
        effect,
        dead: false,
    });
    // The attack shape (TresAtkColl0: one SPHERE; its growth to m_IncMaxSize).
    let size = (v(row, SIZE), v(row, INC_SIZE), v(row, INC_MAX));
    let hit_effect = s(row, 2);
    let hit_se = s(row, 3);
    spawn_form_shot(cx, FormShot { key, group: row.logical, row: row.flags.first().copied().unwrap_or("None"), position: at, yaw: d.y.atan2(d.x).to_degrees(), size, life: NO_LIFE, hit_effect, hit_se, info, hit_list });
}

/// The bullets' step (real time: projectiles are object type 5): life, movement (homing, spin, accel), the level sweep, the
/// shape's place, the explosion on a hit, the respawn chain.
pub(crate) fn tick(cx: &mut Ctx, dt: f32) {
    if cx.pawn.combat.arrowgun.list.is_empty() {
        return;
    }
    let mut list = std::mem::take(&mut cx.pawn.combat.arrowgun.list);
    let mut respawns: Vec<Respawn> = Vec::new();
    for b in list.iter_mut() {
        step(cx, b, dt, &mut respawns);
    }
    for b in list.iter().filter(|b| b.dead) {
        if let Some(handle) = b.effect {
            cx.stop_effect(handle, false);
        }
        let key = b.key;
        cx.pawn.combat.shots.retain(|s| s.window.key != (FORM_SHOT_SEQUENCE, key));
    }
    list.retain(|b| !b.dead);
    list.append(&mut cx.pawn.combat.arrowgun.list);
    cx.pawn.combat.arrowgun.list = list;
    for r in respawns {
        spawn(cx, r.row, r.at, r.dir, Homing::None, r.info, r.hit_list);
        cx.pawn.combat.arrowgun.respawned += 1;
    }
}

/// A successor to spawn at the end of the step (`m_ReSpawnProjectileClass` at the actor's place and rotation).
struct Respawn {
    row: &'static FormArrowgunRow,
    at: Vec3,
    dir: Vec3,
    info: AttackCtx,
    hit_list: Vec<TargetId>,
}

/// Explode (slot 573): the respawn (m_bReSpawnProjectileOnTimeOut: the master enable; the check type is the native default
/// HIT_ALWAYS), the actor is gone.
fn explode(cx: &Ctx, b: &mut Bullet, respawns: &mut Vec<Respawn>) {
    b.dead = true;
    if v(b.row, RESPAWN) <= 0.0 {
        return;
    }
    let Some(next) = s(b.row, 4).and_then(proj_row) else { return };
    let taken = if v(b.row, TAKE_HIT_LIST) > 0.0 { cx.pawn.combat.shots.iter().find(|s| s.window.key == (FORM_SHOT_SEQUENCE, b.key)).map(|s| s.window.hit_list().to_vec()).unwrap_or_default() } else { Vec::new() };
    respawns.push(Respawn { row: next, at: b.position, dir: b.forward, info: b.info, hit_list: taken });
}

fn step(cx: &mut Ctx, b: &mut Bullet, dt: f32, respawns: &mut Vec<Respawn>) {
    let row = b.row;
    // A character hit of the shot's pass (hit::tick, last frame): a moving bolt dies on it (Explode(hit)).
    let hit = cx.pawn.combat.shots.iter().find(|s| s.window.key == (FORM_SHOT_SEQUENCE, b.key)).is_some_and(|s| s.window.hits > 0);
    if hit && v(row, NO_DESTROY_ON_HIT) <= 0.0 {
        cx.pawn.combat.arrowgun.hits += 1;
        explode(cx, b, respawns);
        return;
    }
    b.age += dt;
    let life = if v(row, LIFE) > 0.0 { v(row, LIFE) } else { NO_LIFE };
    let max_distance = v(row, MAX_DISTANCE);
    if b.age > life || (max_distance > 0.0 && b.distance > max_distance) {
        if v(row, EXPLODE_ON_TIME_OUT) > 0.0 {
            explode(cx, b, respawns);
        } else {
            b.dead = true;
        }
        return;
    }
    let start = b.position;
    movement(cx, b, dt);
    // The root sweep against the level (TresRoot: a sphere); the explosions (speed 0) do not move.
    if b.position != start {
        let r = if v(row, ROOT) > 0.0 { v(row, ROOT) } else { ROOT_RADIUS };
        if let Some(h) = cx.world.sweep_capsule(start, b.position, r, r).filter(|h| !h.start_penetrating) {
            b.position = start + (b.position - start) * h.time.clamp(0.0, 1.0);
            if let Some(fx) = s(row, 5) {
                cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name: fx, at: Attach::World(b.position), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
            }
            explode(cx, b, respawns);
        }
        b.distance += (b.position - start).length();
    }
    let (key, position, yaw) = (b.key, b.position, b.forward.y.atan2(b.forward.x).to_degrees());
    if let Some(shot) = cx.pawn.combat.shots.iter_mut().find(|s| s.window.key == (FORM_SHOT_SEQUENCE, key)) {
        shot.position = position;
        shot.yaw = yaw;
    }
    // MapHitType NOHIT / IGNORE_GROUND (the sheet's last value): the shape never stops on the level; the root sweep above does.
}

/// UTresProjectileMovementComponent (0x1414751E0 homing, 0x1414729E0 ComputeVelocity), as magic/projectile.rs ports it,
/// with the homing point (vt+0x1210, +0x364) and the spin (`m_bEnableSpin`: the rotator `m_Spin` added once per tick
/// while `m_SpinMaxTime` runs).
fn movement(cx: &Ctx, b: &mut Bullet, dt: f32) {
    let row = b.row;
    if b.move_delay > 0.0 {
        b.move_delay -= dt;
        if b.move_delay > 0.0 {
            return;
        }
    }
    if v(row, SPEED) <= 0.0 && b.velocity == Vec3::ZERO {
        return;
    }
    if b.homing_on {
        let point = match b.homing {
            Homing::Target(t) => match cx.target(t).filter(|t| t.flags.has(TargetFlags::ALIVE)) {
                Some(t) => Some(t.center),
                None => {
                    b.homing = Homing::None;
                    None
                }
            },
            Homing::Point(p) => Some(p),
            Homing::None => None,
        };
        if b.homing_delay > 0.0 {
            b.homing_delay = (b.homing_delay - dt).max(0.0);
        }
        if let (true, Some(point)) = (b.homing_delay <= 0.0, point) {
            if b.turn_max > 0.0 {
                b.turn_max += dt * v(row, TURN_ACCEL);
                if v(row, MAX_TURN) > 0.0 {
                    b.turn_max = b.turn_max.min(v(row, MAX_TURN));
                }
            }
            let delta = point - b.position;
            let min = v(row, HOMING_MIN);
            if min > 0.0 && delta.length_squared() <= min * min {
                b.homing_on = false;
            } else {
                let turn = if b.turn_max > 0.0 { (b.turn_max.to_radians() * dt).min(std::f32::consts::PI) } else { std::f32::consts::PI };
                b.velocity = crate::magic::projectile::steer(b.velocity, delta, turn, -1.0);
            }
        }
    }
    if v(row, SPIN) > 0.0 && (v(row, SPIN_MAX) <= 0.0 || b.spin_time < v(row, SPIN_MAX)) {
        let (sin, cos) = v(row, SPIN_YAW).to_radians().sin_cos();
        let u = b.velocity;
        b.velocity = Vec3::new(u.x * cos - u.y * sin, u.x * sin + u.y * cos, u.z);
        b.spin_time += dt;
    }
    let v0 = b.velocity;
    let mut v1 = v0;
    if v(row, ACCEL) > 0.0 {
        v1 += v0.normalize_or_zero() * v(row, ACCEL) * dt;
    }
    let max = v(row, MAX_SPEED);
    if max > 0.0 && v1.length() > max {
        v1 = v1.normalize() * max;
    }
    b.position += v0 * dt + (v1 - v0) * dt * 0.5;
    b.velocity = v1;
    if v1.length_squared() > 1.0e-8 {
        b.forward = v1.normalize();
    }
}

// ================================================================================================ FINISH_BZ

/// The row of FINISH_BZ (`finish.BZ.157`): values = [shot time, pitch, aim distance, TresEnableChange].
pub(crate) fn finish_row() -> Option<&'static FormArrowgunRow> {
    rows("finish").find(|r| r.code == FINISH_BZ)
}

/// 0x1413DD180 for 0x9D: BZ_BIG_BANG (the notify's name is ignored) from weapon 0's muzzle, pitch 30 and the yaw of muzzle -
/// centre, fired at once, homing on the WORLD POINT centre + forward x 1000; then WorldSlow(ctrl, 0).
pub(crate) fn big_bang(cx: &mut Ctx, row: &FormArrowgunRow) {
    let name = row.flags.first().copied().unwrap_or("BZ_BIG_BANG");
    let at = muzzle(cx, 0);
    let c = centre(cx);
    let flat = (at - c).truncate();
    let yaw = if flat.length_squared() > 1.0e-6 { flat.y.atan2(flat.x) } else { cx.pawn.yaw.to_radians() };
    let pitch = v(row, 1).to_radians();
    let dir = Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin());
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    let aim = c + Vec3::new(cos, sin, 0.0) * v(row, 2);
    let info = cx.pawn.combat.attack_ctx;
    fire(cx, name, at, dir, Homing::Point(aim), info);
    crate::combat::form::world_slow(cx, false);
}

// ================================================================================================ (AUDIT-A ARROWGUN) own clips

/// (AUDIT-A ARROWGUN) This package's key range in `Pawn::notify_handles` (forms/ship.rs `wnotify_pass`; F-SHIP 0xF000,
/// F-HAMMER 0x9000, AUDIT-C SW 0xB000, F-CLAWS 0xC000 / 0xD000, AUDIT-A SHIELD 0xE000, AUDIT-A STAFF 0x7000; Sora's own
/// notify keys are sequence indices < 0x0700).
const WNOTIFY_KEY: u16 = 0x6000;

/// (AUDIT-A ARROWGUN) After the generic notify handlers of a pose tick (sora.rs), in every state: THE FORM WEAPONS' AND THE
/// TRANSFORM UNITS' OWN CLIPS (`wnotify.*` rows of sheets/port/sora-form-arrowgun.json, tools/sora_form_arrowgun.js with
/// AUDIT-B's tools/lib/wnotify_rows.js): KH3's arrowguns w_so022 / w_so023 / w_so042 / w_so043, launchers w_so024 / w_so044
/// and transform units w_so021 / w_so041 play the row of Sora's logical name on their own anim instances, and their
/// AttachEffect / AttachSound / PlaySound notifies fire on THEIR meshes (anchors `kb:<model>:<mount>:<bone>`, the launchers'
/// FINISH_BZ rows on `kh_root`: their ChangeAttachParent, placed by forms/placement.rs `part_parent`): the transformations' star pieces / wings / honey
/// and bees (StyleChange2 / 5 / 16 / 17), the gatling barrels (2405), the 0x47 charge (M_2414), the muzzle glows (M_2416),
/// Rising Blast's honey and the launcher's sounds (M_5413), FINISH_BZ's charge, wings, muzzle and sounds (M_5417). Played
/// by F-SHIP's shared pass while their form is held; a state row ends at its end, on a play change and with the form.
pub(crate) fn on_notifies(cx: &mut Ctx) {
    let held = form_key(cx.pawn.combat.form.cmd);
    if held.is_none() && !cx.pawn.notify_handles.iter().any(|h| h.0 & 0xF000 == WNOTIFY_KEY) {
        return;
    }
    let row = |i: usize| {
        FORM_ARROWGUN.get(i).filter(|r| r.what == "wnotify" && Some(r.form) == held).map(|r| super::ship::WNotify {
            clip: r.clip,
            code: r.code,
            flags: r.flags,
            effects: if r.code == 0 { std::slice::from_ref(&r.logical) } else { &[] },
            sounds: if r.code == 0 { &[] } else { std::slice::from_ref(&r.logical) },
            values: r.values,
        })
    };
    super::ship::wnotify_pass(cx, WNOTIFY_KEY, FORM_ARROWGUN.len(), &row);
}

/// The form row of a command (for tests).
pub fn form_row(cmd: i32) -> Option<&'static KeybladeFormRow> {
    crate::combat::form::row_of(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(combo: i32, last: u8, dist: f32) -> GunDecideIn {
        GunDecideIn { combo, reach: 0, last_kind: last, dist, dz: 0.0, num_enemies: 1, wall_run: false, finish_count: 0 }
    }

    #[test]
    fn the_arrowgun_combo_alternates_its_curves_then_bursts() {
        // A close single target: 0x44 -> 0x45 -> 0x46 -> finisher 0x47 (twice: Finishers 2, MaxCombo 5).
        let mut last = 0;
        let mut kinds = Vec::new();
        let mut reach = 0;
        for combo in 0..5 {
            let d = decide_dw(&GunDecideIn { reach, ..input(combo, last, 300.0) });
            kinds.push(d.kind);
            last = d.kind;
            reach = d.reach;
        }
        assert_eq!(kinds, vec![0x44, 0x45, 0x46, 0x47, 0x47]);
        // A crowd: the spread kinds.
        let d = decide_dw(&GunDecideIn { num_enemies: 3, ..input(0, 0, 300.0) });
        assert_eq!((d.kind, d.reach), (0x46, 2));
        // The target well below: the sliding down-shot 0x48.
        assert_eq!(decide_dw(&GunDecideIn { dz: -400.0, ..input(1, 0x44, 200.0) }).kind, 0x48);
        assert!(decide_dw(&input(3, 0x46, 300.0)).finish_end);
    }

    #[test]
    fn the_launcher_swings_close_and_shoots_far() {
        assert_eq!(decide_bz(&input(0, 0, 300.0)).kind, 0x4A);
        assert_eq!(decide_bz(&input(1, 0x4A, 300.0)).kind, 0x4B);
        assert_eq!(decide_bz(&input(2, 0x4B, 300.0)).kind, 0x4C);
        assert_eq!(decide_bz(&input(0, 0, 900.0)).kind, 0x4D);
        let d = decide_bz(&input(2, 0x4D, 900.0));
        assert_eq!((d.kind, d.finish, d.finish_count), (0x4E, true, 1));
    }

    #[test]
    fn every_attack_kind_has_its_clip_and_the_shots_their_classes() {
        for r in rows("attack") {
            assert!(crate::anim::AnimId::from_name(r.clip).is_some(), "{}: clip {} is not in the animation table", r.id, r.clip);
            if let Some(name) = flag_value(r, "shot") {
                for cmd in match r.form {
                    "DW" => [CHANGE_DW, CHANGE_DW2],
                    _ => [CHANGE_BZ, CHANGE_BZ2],
                } {
                    let bp = form_row(cmd).and_then(|f| f.projectiles.iter().find_map(|p| p.split_once('=').filter(|(n, _)| *n == name).map(|(_, b)| b)));
                    assert!(bp.and_then(proj_row).is_some(), "{} in {cmd:#x}: {name} has no class row", r.id);
                }
            }
        }
        assert_eq!(replaced_clip(CHANGE_BZ2, "M_5417"), Some("5417"));
        assert_eq!(replaced_clip(CHANGE_DW, "MagicFire"), Some("MagicFire2")); // (F-LOCO) the blend space (2321 at 0 degrees)
        assert_eq!(proj_row("/Game/Blueprints/Projectile/BZ/TresProjSS_BombShot1.TresProjSS_BombShot1_C").map(|r| r.flags[4]), Some("TresProjSS_BombShot1Ex"));
    }
}
