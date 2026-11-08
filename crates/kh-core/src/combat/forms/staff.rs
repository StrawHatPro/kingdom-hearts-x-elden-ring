//! (weapons F-STAFF) Mirage Staff (`form.WA`, CHANGE_WA 0x88, Ever After): its form package (documentation/
//! KH3_WEAPONS_PLAN.md 1.3 / 4; build/sora_tmp/weapons/F-STAFF/STATE.md). A first-stage form of 20 s (wear form 2, Wisdom)
//! with FINISH_WA (0xA2). The data is `sheets/port/sora-form-staff.json` (tools/sora_form_staff.js): every number below
//! that is not an address comes from it.
//!
//! WHAT KH3 DOES (decompiled C in re/decomp; the vtable 0x146913968 of TresPlayerState_ComboWA read from the exe image):
//! - THE ATTACK CLASS (pawn slot 1337 0x141432F00: 0x88 -> 0x53): TresPlayerState_ComboWA (id 83, `states/combo_wa.rs`):
//!   Reset 0x141352EC0 (+0x234 bit 22: the homing target is taken again from the controller), Begin 0x14134F4D0, Decide
//!   0x1413642B0 ([`decide`]), no weapon recoil (slot 129 0x14041F340), the stick steering (slot 131 0x141355520), the beam
//!   chain (slot 132 0x14134E420), the held-button repeat (slot 133 0x1413527E0) and its shot handler 0x141359700.
//!   PlayAttack 0x1413378B0 cases 0x80 .. 0x87 (the sheet's `attack.*`); MaxCombo / Finishers ([`max_combo`]) 5 / 2, or 10 / 1
//!   while the mirror images are out.
//! - THE MIRROR IMAGES ([`Avatars`]): the controller's m_AvaterProjList and +0xB90 / +0xB94 (slot 868 0x14140B920
//!   [`avatar_command`]; the tick 0x1413F7030), each a TresProj_WandAvator (0x141484720 ...): spawned by the dodge
//!   (TryAirSlide 0x141436D20: weapon form 10 in battle: `states/air_slide.rs` (F-STAFF) lines), at most 4, gone 10 s after
//!   the last one or after a finish. While they are out the combo is the beam (0x80) and they fire with Sora.
//! - THE PROJECTILES ([`Beams`]): WA_BEAM / WA_FINISH / WA_BRAID of w_so070_ProjSet (TresProjWA_Beam / Beam2 / Braid: speed,
//!   homing, life, the attack shape and row from the blueprints): through `combat::hit`'s shot pass (`spawn_form_shot`), the
//!   generic KH hit path (launches and juggles of the rows on every Elden Ring enemy the host's lock-on accepts).
//! - FINISH_WA: TresPlayerState_StyleFinishWA (id 37): the tower (w_so073, TresStylePawnWATower), `states/combo_wa.rs`.
//! - THE ANIM SET: m_ReplaceAnimSets AnimSet10: [`replaced_clip`].
//!
//! Not in the core (hosts / INT; STATE.md): the drawing of the images, beams and the tower ([`AvatarView`], [`BeamView`],
//! [`tower`]), the replacement set for the states that do not play through the form (locomotion: [`replaced_clip`]), the
//! camera distance of the form ([`camera_distance_scale`]).

use super::FormMoveset;
use crate::combat::form::KeybladeFormRow;
use crate::combat::hit::{spawn_form_shot, FormShot, FORM_SHOT_SEQUENCE};
use crate::combat::{AttackCtx, PawnCombat};
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, TargetFlags, TargetId};
use crate::pawn::Ctx;
use crate::sora::wrap_degrees;
use crate::states::StateData;
use glam::Vec3;

// The rows (sheets/port/sora-form-staff.json): `FormStaffRow`, `FORM_STAFF`, `FST_*`.
include!(concat!(env!("KH_GEN_DIR"), "/sora_form_staff.rs"));

/// CHANGE_WA: Mirage Staff.
pub const CHANGE_WA: i32 = 0x88;
/// FINISH_WA (SetStyleFinishCommand 0x1413EE030).
pub const FINISH_WA: i32 = 0xA2;
/// The beam with the images out / its finisher; the combo; the dive, the rising spin; the two finishers (PlayAttack cases).
pub const KIND_BEAM: u8 = 0x80;
pub const KIND_BEAM_FINISH: u8 = 0x81;
pub const KIND_FRONT: u8 = 0x82;
pub const KIND_BREAK: u8 = 0x83;
pub const KIND_DOWN: u8 = 0x84;
pub const KIND_BRITZ: u8 = 0x85;
pub const KIND_BRAID: u8 = 0x86;
pub const KIND_BEHIND: u8 = 0x87;

/// The package: the form and its finish command (the bodies: `states/combo_wa.rs`).
pub(crate) const MOVESET: FormMoveset = FormMoveset {
    name: "staff",
    forms: &["form.WA"],
    finishes: &[FINISH_WA],
    on_start,
    on_end,
    attack,
    finish_begin: crate::states::combo_wa::finish_begin,
    finish_tick: crate::states::combo_wa::finish_tick,
    replace_clip,
    hit_shape,
    ..FormMoveset::NONE
};

/// The held form is Mirage Staff.
pub fn is_staff(cmd: i32) -> bool {
    cmd == CHANGE_WA
}

/// A row of the sheet by id.
pub fn row(id: &str) -> Option<&'static FormStaffRow> {
    FORM_STAFF.iter().find(|r| r.id == id)
}

fn values(id: &str) -> &'static [f32] {
    row(id).map_or(&[], |r| r.values)
}

fn value(id: &str, i: usize) -> f32 {
    values(id).get(i).copied().unwrap_or(0.0)
}

/// The row of an attack kind (`attack.0x80` ..).
pub fn attack_row(kind: u8) -> Option<&'static FormStaffRow> {
    FORM_STAFF.iter().find(|r| r.what == "attack" && r.code == i32::from(kind))
}

/// A flag of a row (`keep_face`, `steer` ...).
pub fn has_flag(row: &FormStaffRow, flag: &str) -> bool {
    row.flags.contains(&flag)
}

/// m_ReplaceAnimSets: the clip (animation-table row) a logical name plays as in Mirage Staff (AnimSet10); None = the base
/// set's row. For the pose hosts too (locomotion, magic).
pub fn replaced_clip(cmd: i32, logical: &str) -> Option<&'static str> {
    if !is_staff(cmd) {
        return None;
    }
    FORM_STAFF.iter().find(|r| r.what == "anim" && r.logical == logical && r.clip != "-").map(|r| r.clip)
}

fn replace_clip(logical: &str) -> Option<&'static str> {
    replaced_clip(CHANGE_WA, logical)
}

/// A hit shape of the staff's TresAtkColl0 (the rows `track.<clip>.<group>`: the groups are OWNER_ATTACHED_MESH, on Sora's
/// bones / sockets under the clip, tools/sora_form_staff.js) at clip time `time`, as (end 1, end 2, radius) in the world for
/// an actor at `location` turned `yaw`. None = not a tracked group of that clip.
pub fn track_shape(group: &str, clip: &str, time: f32, location: Vec3, yaw: f32) -> Option<(Vec3, Vec3, f32)> {
    let row = FORM_STAFF.iter().find(|r| r.what == "track" && r.clip == clip && r.logical == group)?;
    let frames = usize::try_from(row.code).ok().filter(|f| *f > 0 && row.values.len() >= f * 7)?;
    let last = frames - 1;
    let f = if row.length > 0.0 { (time / row.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let at = |k: usize| row.values[i * 7 + k] + (row.values[j * 7 + k] - row.values[i * 7 + k]) * t;
    let (sin, cos) = yaw.to_radians().sin_cos();
    let world = |v: Vec3| location + Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z);
    Some((world(Vec3::new(at(0), at(1), at(2))), world(Vec3::new(at(3), at(4), at(5))), at(6)))
}

/// A baked track of the tower under its own A417 (`track.tower.<socket / group>`: 6 numbers a frame, the tower's actor
/// space), linear between frames at tower time `time`.
pub fn tower_track(name: &str, time: f32) -> Option<[f32; 6]> {
    let row = FORM_STAFF.iter().find(|r| r.what == "track" && r.clip == "A417" && r.logical == name)?;
    let frames = usize::try_from(row.code).ok().filter(|f| *f > 0 && row.values.len() >= f * 6)?;
    let last = frames - 1;
    let f = if row.length > 0.0 { (time / row.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    Some(std::array::from_fn(|k| row.values[i * 6 + k] + (row.values[j * 6 + k] - row.values[i * 6 + k]) * t))
}

/// The package's `hit_shape` hook (combat/hit.rs `shape`).
fn hit_shape(cx: &Ctx, group: &str, sequence: usize, time: f32) -> Option<(Vec3, Vec3, f32)> {
    let clip = crate::anim::SEQUENCES.get(sequence)?.name;
    track_shape(group, clip, time, cx.pawn.cmc.location, cx.pawn.yaw)
}

// ------------------------------------------------------------------------------------------------ MaxCombo / Finishers

/// The mirror images are out (controller+0xB90, ctrl vt+0x1B28 0x14128B950).
pub fn avatars_out(c: &PawnCombat) -> bool {
    c.staff.avatars.out
}

/// MaxCombo (pawn slot 928 0x141433570) and Finishers (slot 1253 0x141433A60), case 0x88: 10 / 1 with the images out,
/// else 5 / 2 (`max`).
pub fn max_combo(c: &PawnCombat) -> (i32, i32) {
    let v = |i| value("max", i) as i32;
    if avatars_out(c) { (v(0), v(1)) } else { (v(2), v(3)) }
}

// ------------------------------------------------------------------------------------------------ the decision

/// What ComboWA's Decide reads: state+0x128 the combo, +0x154 the reach, +0x1AC the previous kind, +0x1C4 the enemies
/// around, +0x1CC / +0x1D0 the distance / height to the target (PreDecide), the images (+0xB90), pawn+0xE14 the finishers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaffDecideIn {
    pub combo: i32,
    pub reach: u8,
    pub last_kind: u8,
    pub num_enemies: i32,
    pub dist: f32,
    pub dz: f32,
    pub avatars: bool,
    pub finish_count: i32,
}

/// What it leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaffDecision {
    pub kind: u8,
    pub reach: u8,
    pub finish: bool,
    pub finish_end: bool,
    pub finish_count: i32,
    /// combo 0: +0x1AC = 0 and pawn vt+0x1D40(0).
    pub last_reset: bool,
    /// +0x1B8 / +0x1BC / +0x1C0: the step's slot of the combo (no reader in the class: carried for the debug text).
    pub slot: (i32, i32, i32),
}

/// `TresPlayerState_ComboWA::Decide` (0x1413642B0, decompiled C):
///
/// ```text
/// PreDecide; if combo == 0: reach = dist <= 400 ? (enemies > 1 ? 2 : 0) : 1; last = 0; pawn+0xE10 = 0
/// (+0x1EC bit 3, a wall-run request: +0x142 air = 1)
/// images out: (9, 10, combo % 9); kind 0x80; combo > 8: finish, pawn finish end, finish end, n + 1, 0x81
/// else: (3, 5, combo % 3); combo > 2: finish; n == 0 ? 0x86 : (pawn finish end, finish end, 0x87); n + 1
///       last != 0x84 && dz < -100 && dz < -dist: 0x84;  last != 0x85 && dz > 100 && dz > dist: 0x85
///       reach 1: last 0x82 -> 0x85, 0x83 -> 0x82, 0x85 -> 0x83, else combo 0 ? 0x85 : 0x83
///       reach 2: last 0x82 -> 0x83, 0x83 -> 0x85, 0x85 -> 0x82, else combo 0 ? 0x83 : 0x85
///       else:    last 0x82 -> 0x83, 0x83 -> 0x85, 0x85 -> 0x82, else combo 0 ? 0x82 : 0x83
/// ```
pub fn decide(i: &StaffDecideIn) -> StaffDecision {
    let mut d = StaffDecision { kind: KIND_FRONT, reach: i.reach, finish: false, finish_end: false, finish_count: i.finish_count, last_reset: false, slot: (0, 0, 0) };
    let mut last = i.last_kind;
    if i.combo == 0 {
        d.reach = if i.dist <= 400.0 { if i.num_enemies > 1 { 2 } else { 0 } } else { 1 };
        last = 0;
        d.last_reset = true;
    }
    if i.avatars {
        d.slot = (9, 10, i.combo % 9);
        d.kind = KIND_BEAM;
        if i.combo > 8 {
            (d.finish, d.finish_end, d.finish_count, d.kind) = (true, true, i.finish_count + 1, KIND_BEAM_FINISH);
        }
        return d;
    }
    d.slot = (3, 5, i.combo % 3);
    if i.combo > 2 {
        d.finish = true;
        if i.finish_count == 0 {
            d.kind = KIND_BRAID;
        } else {
            (d.finish_end, d.kind) = (true, KIND_BEHIND);
        }
        d.finish_count = i.finish_count + 1;
        return d;
    }
    let (dz, dist) = (i.dz, i.dist);
    if last != KIND_DOWN && dz < -100.0 && dz < -dist {
        d.kind = KIND_DOWN;
        return d;
    }
    if last != KIND_BRITZ && dz > 100.0 && dz > dist {
        d.kind = KIND_BRITZ;
        return d;
    }
    let first = i.combo == 0;
    d.kind = match (d.reach, last) {
        (1, KIND_FRONT) => KIND_BRITZ,
        (1, KIND_BREAK) => KIND_FRONT,
        (1, KIND_BRITZ) => KIND_BREAK,
        (1, _) => if first { KIND_BRITZ } else { KIND_BREAK },
        (2, KIND_FRONT) => KIND_BREAK,
        (2, KIND_BREAK) => KIND_BRITZ,
        (2, KIND_BRITZ) => KIND_FRONT,
        (2, _) => if first { KIND_BREAK } else { KIND_BRITZ },
        (_, KIND_FRONT) => KIND_BREAK,
        (_, KIND_BREAK) => KIND_BRITZ,
        (_, KIND_BRITZ) => KIND_FRONT,
        _ => if first { KIND_FRONT } else { KIND_BREAK },
    };
    d
}

// ------------------------------------------------------------------------------------------------ the hooks

/// StartStyle 0x14141A9D0 case 0x88 (`start`): the soft-target range controller+0x970 = MyAutoLockonLimit x 2.0; the
/// camera distance x 1.35 (camera +0x804 with controller+0xAE0 = 1: [`camera_distance_scale`]); pawn vt+0xFB0(0x88 / 0xA2)
/// loads the commands' assets (nothing in the core). The images were dropped by StartStyle's common part (slot 845): ours.
fn on_start(cx: &mut Ctx, row: &'static KeybladeFormRow) {
    if row.cmd != CHANGE_WA {
        return;
    }
    clear_avatars(cx);
    cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT * value("start", 1);
}

/// EndStyle 0x14141CA20: the images go (slot 845); the soft-target range back (slot 738 0x141410FD0).
fn on_end(cx: &mut Ctx, row: &'static KeybladeFormRow) {
    if row.cmd != CHANGE_WA {
        return;
    }
    clear_avatars(cx);
    cx.pawn.combat.lockon.soft_range = crate::combat::targeting::AUTO_LIMIT;
}

/// The attack request in the form: COMBO_WA (pawn slot 1337: 0x88 -> 0x53).
fn attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    crate::states::combo_wa::try_attack(state, cx, target)
}

/// The camera's distance scale while the form is held (camera +0x804: 1.35 with controller+0xAE0, StartStyle 0x88), else
/// 1.0: for the host's camera (INT line, STATE.md).
pub fn camera_distance_scale(c: &PawnCombat) -> f32 {
    if is_staff(c.form.cmd) { value("start", 0) } else { 1.0 }
}

// ================================================================================================ THE MIRROR IMAGES

/// What a mirror image does (TresProj_WandAvator +0x808).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvatarStep {
    /// 0 / 1: idle, turning toward the target (or Sora's facing).
    Idle,
    /// 2: a beam / finish command waits for its delay.
    Armed,
    /// 3: fired.
    Fired,
    /// 4: after a finish, the wait before all go.
    Finishing,
    /// 5: done (going).
    Done,
}

/// One mirror image (an actor of WA_AVATER: TresProjWA_Avater, native TresProj_WandAvator).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Avatar {
    pub position: Vec3,
    pub yaw: f32,
    pub step: AvatarStep,
    /// +0x818 the commanded kind (0x80 / 0x81), +0x81C its delay, +0x80C the timer of state 4.
    pub kind: u8,
    pub delay: f32,
    pub timer: f32,
    /// m_AttackTargetComponent.
    pub target: TargetId,
    /// Its particle (the blueprint's ParticleComp: `vw_so072_mrg_sora1`).
    pub effect: Option<EffectHandle>,
    pub age: f32,
}

/// The controller's mirror images: m_AvaterProjList (newest first), +0xB90 "out", +0xB94 the time since the last spawn.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Avatars {
    pub list: Vec<Avatar>,
    pub out: bool,
    pub timer: f32,
    /// Spawned / beams the images fired, so far (tests, the debug text).
    pub spawned: u32,
    pub fired: u32,
}

/// A mirror image as a host draws it: its particle at its place, facing `yaw`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvatarView {
    pub particle: &'static str,
    pub position: Vec3,
    pub yaw: f32,
    pub step: AvatarStep,
    pub effect: Option<EffectHandle>,
}

impl Avatars {
    pub fn views(&self) -> impl Iterator<Item = AvatarView> + '_ {
        let particle = proj("WA_AVATER").and_then(|r| r.effects.first().copied()).unwrap_or("-");
        self.list.iter().map(move |a| AvatarView { particle, position: a.position, yaw: a.yaw, step: a.step, effect: a.effect })
    }
}

// The numbers of the `avatar` row.
const AV_MAX: usize = 0;
const AV_LIFE: usize = 1;
const AV_BEAM_STEP: usize = 2;
const AV_FINISH_WAIT: usize = 3;
const AV_FIRE_LOCK: usize = 4;
const AV_TURN_IDLE: usize = 5;
const AV_TURN_BUSY: usize = 6;
const AV_MUZZLE: usize = 7;

/// The images' target (TresProj_WandAvator 0x141481DB0): the controller's lock-on target, else its soft target; both are
/// `lockon.current` here (the host's lock-on rules decide what is lockable).
fn avatar_target(cx: &Ctx) -> TargetId {
    let t = cx.pawn.combat.lockon.current;
    if cx.target(t).is_some_and(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE)) { t } else { TargetId::NONE }
}

/// Controller slot 845 (0x1413FA210): every image goes (its vt+0xA10), the list is emptied, +0xB90 = 0.
pub(crate) fn clear_avatars(cx: &mut Ctx) {
    let list = std::mem::take(&mut cx.pawn.combat.staff.avatars.list);
    for a in list {
        if let Some(h) = a.effect {
            cx.stop_effect(h, false);
        }
    }
    cx.pawn.combat.staff.avatars.out = false;
}

/// Controller slot 868 (0x14140B920) `(mode)`: first slot 843 (+0xB90 = 1, +0xB94 = 0); then 0: a new image (WA_AVATER at
/// Sora's root with his rotation, put at the FRONT; with five the fifth (the oldest) goes); 1: every image fires a beam
/// (kind 0x80, each one 0.033333 s later than the one before, from the first), at the lock-on target if the lock is on,
/// else none (the image takes its own); 2: every image fires the finish (0x81) at once, at the lock-on / soft target;
/// 3: all go (slot 845).
pub(crate) fn avatar_command(cx: &mut Ctx, mode: u8) {
    {
        let av = &mut cx.pawn.combat.staff.avatars;
        (av.out, av.timer) = (true, 0.0);
    }
    match mode {
        0 => {
            let (position, yaw) = (cx.pawn.cmc.location, cx.pawn.yaw);
            let particle = proj("WA_AVATER").and_then(|r| r.effects.first().copied()).filter(|p| *p != "-");
            let effect = particle.map(|name| cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name, at: Attach::World(position), rotation: Vec3::new(0.0, yaw, 0.0), scale: 1.0, persistent: true }));
            let av = &mut cx.pawn.combat.staff.avatars;
            av.list.insert(0, Avatar { position, yaw, step: AvatarStep::Idle, kind: 0, delay: 0.0, timer: 0.0, target: TargetId::NONE, effect, age: 0.0 });
            av.spawned += 1;
            let max = value("avatar", AV_MAX) as usize;
            if av.list.len() > max {
                let gone = av.list.remove(max);
                if let Some(h) = gone.effect {
                    cx.stop_effect(h, false);
                }
            }
        }
        1 | 2 => {
            let locked = cx.pawn.combat.lockon.locked;
            let target = if mode == 2 || locked { avatar_target(cx) } else { TargetId::NONE };
            let (kind, step) = if mode == 1 { (KIND_BEAM, value("avatar", AV_BEAM_STEP)) } else { (KIND_BEAM_FINISH, 0.0) };
            let lock = value("avatar", AV_FIRE_LOCK);
            let mut delay = 0.0;
            for a in cx.pawn.combat.staff.avatars.list.iter_mut() {
                delay += step;
                // 0x141481CE0: only before it fired (+0x808 < 3) and for 0x80 / 0x81.
                if matches!(a.step, AvatarStep::Idle | AvatarStep::Armed) {
                    if target != TargetId::NONE {
                        a.target = target;
                    }
                    (a.delay, a.kind, a.timer, a.step) = (delay, kind, lock, AvatarStep::Armed);
                }
            }
        }
        _ => clear_avatars(cx),
    }
}

/// The controller tick's part (0x1413F7030): while the images are out they go at once when ctrl+0xA08 bit 1 is set (the
/// shotlock's release: `magic::shotlock::flag::RELEASE_PENDING`), or when a link (ctrl+0xBB8, setter slot 880 0x1411B5CD0:
/// written by the attraction ride AFRollerCoaster), an attraction (+0xC08, slot 891) or a world team attack (+0xC09, slot
/// 895: FRMickeyChain / FRSnowChase / FRSnowCurling) is on - none of those three exists in this port (the attractions are
/// excluded by the user, the world team attacks are not ported); else +0xB94 += dt and past 10 s they go.
fn avatars_timer(cx: &mut Ctx, dt: f32) {
    if !cx.pawn.combat.staff.avatars.out {
        return;
    }
    let release = cx.pawn.magic.aim.flags & crate::magic::shotlock::flag::RELEASE_PENDING != 0;
    let av = &mut cx.pawn.combat.staff.avatars;
    av.timer += dt;
    if release || av.timer > value("avatar", AV_LIFE) {
        clear_avatars(cx);
    }
}

/// TresProj_WandAvator's tick (0x141484720) for every image: the command's delay, its shot (0x14147E8E0), the wait after a
/// finish (then slot 868(3): all go), the turn toward its target (or Sora's facing) at 12 (idle) or 30 per second.
fn avatars_tick(cx: &mut Ctx, dt: f32) {
    let mut list = std::mem::take(&mut cx.pawn.combat.staff.avatars.list);
    let mut all_go = false;
    let mut shots: Vec<(Vec3, f32, u8, TargetId)> = Vec::new();
    for a in list.iter_mut() {
        a.age += dt;
        match a.step {
            AvatarStep::Done => continue,
            AvatarStep::Finishing => {
                a.timer -= dt;
                if a.timer <= 0.0 {
                    all_go = true;
                    a.step = AvatarStep::Done;
                }
                continue;
            }
            AvatarStep::Fired => {
                if a.kind == KIND_BEAM_FINISH {
                    (a.step, a.timer) = (AvatarStep::Finishing, value("avatar", AV_FINISH_WAIT));
                    continue;
                }
                (a.kind, a.step) = (0, AvatarStep::Idle);
            }
            AvatarStep::Armed => {
                a.delay -= dt;
                if a.delay <= 0.0 {
                    if a.target == TargetId::NONE {
                        a.target = avatar_target(cx);
                    }
                    shots.push((a.position, a.yaw, a.kind, a.target));
                    (a.timer, a.delay, a.step) = (value("avatar", AV_FIRE_LOCK), 0.0, AvatarStep::Fired);
                }
            }
            AvatarStep::Idle => {}
        }
        // LAB_14148483B: the target again (0x141481DB0), the turn.
        let t = avatar_target(cx);
        a.target = t;
        let rate = if a.step == AvatarStep::Idle { value("avatar", AV_TURN_IDLE) } else { value("avatar", AV_TURN_BUSY) };
        let want = match cx.target(t) {
            Some(info) => {
                let d = info.center - a.position;
                if d.truncate().length_squared() > 1.0e-6 { d.y.atan2(d.x).to_degrees() } else { a.yaw }
            }
            None => cx.pawn.yaw,
        };
        a.yaw = wrap_degrees(a.yaw + wrap_degrees(want - a.yaw) * (rate * dt).clamp(0.0, 1.0));
    }
    list.append(&mut cx.pawn.combat.staff.avatars.list);
    cx.pawn.combat.staff.avatars.list = list;
    for (position, yaw, kind, target) in shots {
        avatar_shot(cx, position, yaw, kind, target);
    }
    if all_go {
        avatar_command(cx, 3);
    }
}

/// The image's shot (0x14147E8E0, through its anim interface vt+0x228): WA_BEAM / WA_FINISH from its root + (125, 0, 168)
/// in its frame, at its target's lock-on point (homing on it), else 1000 cm ahead (homing to that point); fired by Sora
/// (pawn vt+0x1450 of the owner), with no combo counters.
fn avatar_shot(cx: &mut Ctx, position: Vec3, yaw: f32, kind: u8, target: TargetId) {
    let name = if kind == KIND_BEAM_FINISH { "WA_FINISH" } else { "WA_BEAM" };
    let m = values("avatar");
    let local = Vec3::new(m.get(AV_MUZZLE).copied().unwrap_or(0.0), m.get(AV_MUZZLE + 1).copied().unwrap_or(0.0), m.get(AV_MUZZLE + 2).copied().unwrap_or(0.0));
    let (sin, cos) = yaw.to_radians().sin_cos();
    let at = position + Vec3::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos, local.z);
    let forward = Vec3::new(cos, sin, 0.0);
    let (dir, homing) = match cx.target(target).filter(|t| t.flags.has(TargetFlags::ALIVE)) {
        Some(t) => ((t.center - at).normalize_or_zero(), Homing::Target(t.id)),
        None => (forward, Homing::Point(position + forward * 1000.0)),
    };
    fire(cx, name, at, dir, homing, AttackCtx::default());
    cx.pawn.combat.staff.avatars.fired += 1;
}

// ================================================================================================ THE PROJECTILES

/// What a beam homes on: the homing target (vt+0x1220) or a world point (vt+0x1210).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Homing {
    Target(TargetId),
    Point(Vec3),
}

/// One live projectile of the form (an actor of w_so070_ProjSet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beam {
    /// The shot key in `combat.shots` (`Window::key.1`).
    pub key: u16,
    pub row: &'static FormStaffRow,
    pub position: Vec3,
    pub velocity: Vec3,
    pub forward: Vec3,
    pub age: f32,
    pub distance: f32,
    pub homing: Option<Homing>,
    homing_delay: f32,
    turn_max: f32,
    pub effect: Option<EffectHandle>,
    pub dead: bool,
}

/// A beam as a host draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamView {
    pub class: &'static str,
    pub particle: &'static str,
    pub position: Vec3,
    pub forward: Vec3,
    pub effect: Option<EffectHandle>,
}

/// The live projectiles (they outlive the form).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Beams {
    pub list: Vec<Beam>,
    next: u16,
    /// Fired / that hit, so far (tests, the debug text).
    pub fired: u32,
    pub hits: u32,
}

impl Beams {
    pub fn views(&self) -> impl Iterator<Item = BeamView> + '_ {
        self.list.iter().filter(|b| !b.dead).map(|b| BeamView { class: b.row.clip, particle: b.row.effects.first().copied().unwrap_or("-"), position: b.position, forward: b.forward, effect: b.effect })
    }
}

/// The package's state on the pawn (`PawnCombat::staff`): the images and the projectiles.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StaffState {
    pub avatars: Avatars,
    pub beams: Beams,
    /// FINISH_WA's tower (StyleFinishWA +0x2E0 m_pTower) while it stands.
    pub tower: Option<Tower>,
}

/// The tower (w_so073, TresStylePawnWATower) of FINISH_WA: spawned at Sora's root with his yaw, it plays its own M_A417 from
/// `time` 0; `running` = tower +0xB48.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tower {
    pub position: Vec3,
    pub yaw: f32,
    pub time: f32,
    pub running: bool,
}

// The numbers of a `proj.*` row (tools/sora_form_staff.js).
const P_LIFE: usize = 0;
const P_MAX_DISTANCE: usize = 1;
const P_SPEED: usize = 2;
const P_MAX_SPEED: usize = 3;
const P_HOMING_DELAY: usize = 4;
const P_HOMING_MIN: usize = 5;
const P_TURN_MAX: usize = 6;
const P_TURN_ACCEL: usize = 7;
const P_MAX_TURN: usize = 8;
const P_RADIUS: usize = 9;
const P_HALF: usize = 10;
const P_OFFSET: usize = 11;

/// The keys of this package's shots: 0xA000 .. (F-ARROWGUN's bullets count up from 1).
const KEY_BASE: u16 = 0xA000;
/// The key of the tower's shots (never one of the beams': those stay below KEY_BASE + 0x2000).
const KEY_TOWER: u16 = 0xC000;

/// The particle of a projectile or a mirror image (the blueprint's ParticleComp template): the host anchors it on the actor
/// ([`BeamView`] / [`AvatarView`] by its effect handle).
pub fn is_projectile_particle(name: &str) -> bool {
    FORM_STAFF.iter().any(|r| r.what == "proj" && r.effects.first() == Some(&name))
}

/// The row of a projectile name (`proj.WA_BEAM` ..).
pub fn proj(name: &str) -> Option<&'static FormStaffRow> {
    FORM_STAFF.iter().find(|r| r.what == "proj" && r.logical == name)
}

fn pv(row: &FormStaffRow, i: usize) -> f32 {
    row.values.get(i).copied().unwrap_or(0.0)
}

fn effect_of(row: &FormStaffRow, prefix: &str) -> Option<&'static str> {
    row.effects.iter().find_map(|e| e.strip_prefix(prefix))
}

/// pawn vt+0x1450(name, P, P, dir, team) + the setters (homing vt+0x1220 / vt+0x1210, the counters vt+0x1130 / +0x1140 /
/// +0x1120, SpawnFireEffects vt+0x11F8, StartFire vt+0x1270): a projectile of w_so070_ProjSet at `at` along `dir`.
pub(crate) fn fire(cx: &mut Ctx, name: &str, at: Vec3, dir: Vec3, homing: Homing, info: AttackCtx) -> bool {
    let Some(row) = proj(name) else {
        cx.pawn.note("Mirage Staff asked for a projectile its set does not have: nothing is fired");
        return false;
    };
    let d = if dir.length_squared() > 1.0e-8 { dir.normalize() } else { Vec3::X };
    let rotation = Vec3::new(d.z.clamp(-1.0, 1.0).asin().to_degrees(), d.y.atan2(d.x).to_degrees(), 0.0);
    if let Some(fx) = effect_of(row, "fire@") {
        cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name: fx, at: Attach::World(at), rotation, scale: 1.0, persistent: false });
    }
    let effect = row.effects.first().copied().filter(|p| !p.contains('@')).map(|name| cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name, at: Attach::World(at), rotation, scale: 1.0, persistent: true }));
    let beams = &mut cx.pawn.combat.staff.beams;
    let key = KEY_BASE.wrapping_add(beams.next);
    beams.next = (beams.next + 1) % 0x2000;
    beams.fired += 1;
    beams.list.push(Beam { key, row, position: at, velocity: d * pv(row, P_SPEED), forward: d, age: 0.0, distance: 0.0, homing: Some(homing), homing_delay: pv(row, P_HOMING_DELAY), turn_max: pv(row, P_TURN_MAX), effect, dead: false });
    let radius = pv(row, P_RADIUS);
    let hit_effect = effect_of(row, "hit@");
    let group = row.logical;
    let shot_row = row.rows.first().copied().unwrap_or("None");
    spawn_form_shot(cx, FormShot { key, group, row: shot_row, position: shape_centre(row, at, d), yaw: d.y.atan2(d.x).to_degrees(), size: (radius, 0.0, radius), life: 1.0e6, hit_effect, hit_se: None, info, hit_list: Vec::new() });
    if let Some(s) = cx.pawn.combat.shots.last_mut() {
        s.axis = shape_axis(row, d);
    }
    true
}

/// The actor's axes for a flight direction (its rotation follows its velocity: pitch, yaw, no roll): forward, right, up.
fn axes(forward: Vec3) -> (Vec3, Vec3, Vec3) {
    let d = forward.normalize_or_zero();
    let (sp, cp) = d.z.clamp(-1.0, 1.0).asin().sin_cos();
    let yaw = d.y.atan2(d.x);
    let (sy, cy) = yaw.sin_cos();
    (Vec3::new(cp * cy, cp * sy, sp), Vec3::new(-sy, cy, 0.0), Vec3::new(-sp * cy, -sp * sy, cp))
}

/// The attack shape's centre: the actor + its m_RelativeLocation1 in the actor's frame (TRES_ACRA_NORMAL).
fn shape_centre(row: &FormStaffRow, at: Vec3, forward: Vec3) -> Vec3 {
    let (f, r, u) = axes(forward);
    at + f * pv(row, P_OFFSET) + r * pv(row, P_OFFSET + 1) + u * pv(row, P_OFFSET + 2)
}

/// A CAPSULE1 shape's half axis: m_Size.Z along the actor's Z (m_RelativeRocation 0 in these blueprints); zero for a
/// SPHERE.
fn shape_axis(row: &FormStaffRow, forward: Vec3) -> Vec3 {
    if row.flags.first() != Some(&"CAPSULE1") {
        return Vec3::ZERO;
    }
    axes(forward).2 * pv(row, P_HALF)
}

/// A capsule shot of the form (FINISH_WA's tower): through the shot pass with its half axis (`Shot::axis`), the radius
/// `size` = (now, growth / s, cap); the counters of the running attack.
pub(crate) fn spawn_capsule_shot(cx: &mut Ctx, group: &'static str, row: &'static str, centre: Vec3, axis: Vec3, size: (f32, f32, f32), life: f32) {
    let info = cx.pawn.combat.attack_ctx;
    spawn_form_shot(cx, FormShot { key: KEY_TOWER, group, row, position: centre, yaw: 0.0, size, life, hit_effect: None, hit_se: None, info, hit_list: Vec::new() });
    if let Some(s) = cx.pawn.combat.shots.last_mut() {
        s.axis = axis;
    }
}

/// The package's step, once per core step (sora.rs, beside the other projectiles): the controller's image timer (`dt`),
/// the images and the projectiles (projectiles are object type 5: the real step time).
pub(crate) fn tick(cx: &mut Ctx, dt: f32, real_dt: f32) {
    avatars_timer(cx, dt);
    if !cx.pawn.combat.staff.avatars.list.is_empty() {
        avatars_tick(cx, real_dt);
    }
    if cx.pawn.combat.staff.beams.list.is_empty() {
        return;
    }
    let mut list = std::mem::take(&mut cx.pawn.combat.staff.beams.list);
    for b in list.iter_mut() {
        step(cx, b, real_dt);
    }
    for b in list.iter().filter(|b| b.dead) {
        if let Some(h) = b.effect {
            cx.stop_effect(h, false);
        }
        let key = b.key;
        cx.pawn.combat.shots.retain(|s| s.window.key != (FORM_SHOT_SEQUENCE, key));
    }
    list.retain(|b| !b.dead);
    list.append(&mut cx.pawn.combat.staff.beams.list);
    cx.pawn.combat.staff.beams.list = list;
}

fn step(cx: &mut Ctx, b: &mut Beam, dt: f32) {
    let row = b.row;
    // A hit of the shot pass (hit::tick, last frame): the beams die on it, the braid (m_bNoDestroyOnHit) flies on.
    let hit = cx.pawn.combat.shots.iter().find(|s| s.window.key == (FORM_SHOT_SEQUENCE, b.key)).is_some_and(|s| s.window.hits > 0);
    if hit && !has_flag(row, "no_destroy_on_hit") {
        cx.pawn.combat.staff.beams.hits += 1;
        if let Some(fx) = effect_of(row, "hit@") {
            cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name: fx, at: Attach::World(b.position), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
        }
        b.dead = true;
        return;
    }
    b.age += dt;
    let (life, max_distance) = (pv(row, P_LIFE), pv(row, P_MAX_DISTANCE));
    if (life > 0.0 && b.age > life) || (max_distance > 0.0 && b.distance > max_distance) {
        b.dead = true;
        return;
    }
    let start = b.position;
    movement(cx, b, dt);
    // MapHitType NOHIT: the shape passes the level; the root sphere (TresRoot: radius 15) is not swept (no map hit type).
    b.distance += (b.position - start).length();
    let (key, centre, axis, yaw) = (b.key, shape_centre(row, b.position, b.forward), shape_axis(row, b.forward), b.forward.y.atan2(b.forward.x).to_degrees());
    if let Some(shot) = cx.pawn.combat.shots.iter_mut().find(|s| s.window.key == (FORM_SHOT_SEQUENCE, key)) {
        shot.position = centre;
        shot.axis = axis;
        shot.yaw = yaw;
    }
}

/// UTresProjectileMovementComponent: the homing (0x1414751E0 -> 0x1414859E0; magic/projectile.rs ports it) and
/// ComputeVelocity (no m_Accel in these blueprints: the speed is InitialSpeed, capped by MaxSpeed).
fn movement(cx: &Ctx, b: &mut Beam, dt: f32) {
    let row = b.row;
    if let Some(h) = b.homing {
        let point = match h {
            Homing::Target(t) => match cx.target(t).filter(|t| t.flags.has(TargetFlags::ALIVE)) {
                Some(t) => Some(t.center),
                None => {
                    b.homing = None;
                    None
                }
            },
            Homing::Point(p) => Some(p),
        };
        if b.homing_delay > 0.0 {
            b.homing_delay = (b.homing_delay - dt).max(0.0);
        }
        if let (true, Some(point)) = (b.homing_delay <= 0.0, point) {
            if b.turn_max > 0.0 {
                b.turn_max += dt * pv(row, P_TURN_ACCEL);
                if pv(row, P_MAX_TURN) > 0.0 {
                    b.turn_max = b.turn_max.min(pv(row, P_MAX_TURN));
                }
            }
            let delta = point - b.position;
            let min = pv(row, P_HOMING_MIN);
            if min > 0.0 && delta.length_squared() <= min * min {
                b.homing = None;
            } else {
                let turn = if b.turn_max > 0.0 { (b.turn_max.to_radians() * dt).min(std::f32::consts::PI) } else { std::f32::consts::PI };
                b.velocity = crate::magic::projectile::steer(b.velocity, delta, turn, -1.0);
            }
        }
    }
    let max = pv(row, P_MAX_SPEED);
    if max > 0.0 && b.velocity.length() > max {
        b.velocity = b.velocity.normalize() * max;
    }
    b.position += b.velocity * dt;
    if b.velocity.length_squared() > 1.0e-8 {
        b.forward = b.velocity.normalize();
    }
}


// ================================================================================================ FINISH_WA's tower

/// The tower while FINISH_WA runs (for the host's drawing: P5's `form_pawn("w_so070", .., "w_so073", ..)` with `clip` at
/// `time`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TowerView {
    pub model: &'static str,
    pub clip: &'static str,
    pub position: Vec3,
    pub yaw: f32,
    pub time: f32,
}

/// The tower of the running FINISH_WA, if any (`StyleFinish` keeps it: `states/combo_wa.rs`).
pub fn tower(finish: Option<crate::states::FinishView>, c: &PawnCombat) -> Option<TowerView> {
    let f = finish.filter(|f| f.cmd == FINISH_WA)?;
    let t = c.staff.tower.filter(|_| (0..=5).contains(&f.sub))?;
    Some(TowerView { model: "w_so073", clip: row("pawn.w_so073").map_or("A417", |r| r.clip), position: t.position, yaw: t.yaw, time: t.time })
}

// ================================================================================================ (AUDIT-A STAFF) own clips

/// (AUDIT-A STAFF) This package's key range in `Pawn::notify_handles` (forms/ship.rs `wnotify_pass`; F-SHIP 0xF000, F-HAMMER
/// 0x9000, AUDIT-C SW 0xB000, F-CLAWS 0xC000 / 0xD000, AUDIT-A SHIELD 0xE000).
const WNOTIFY_KEY: u16 = 0x7000;

/// (AUDIT-A STAFF) After the generic notify handlers of a pose tick (sora.rs), in every state: THE STAFF'S, THE TRANSFORM
/// UNIT'S AND THE TOWER'S OWN CLIPS (`wnotify.*` rows of sheets/port/sora-form-staff.json, tools/sora_form_staff.js with
/// AUDIT-B's tools/lib/wnotify_rows.js): KH3's w_so072 (RWeapon), w_so071 (Deform) and the tower w_so073 play the row of
/// Sora's logical name on their own anim instances (the tower its M_A417 from StyleFinishWA Tick sub 0, with his), and
/// their AttachEffect / AttachSound / PlaySound / PlayVoice notifies fire on THEIR meshes (anchors `kb:<model>:<mount>:
/// <bone>`; the tower's mount `tower` = its own actor, kh-host vfx.rs). Played by F-SHIP's shared pass while the form is
/// held; a state row ends at its end, on a play change and with the form.
pub(crate) fn on_notifies(cx: &mut Ctx) {
    let held = is_staff(cx.pawn.combat.form.cmd);
    if !held && !cx.pawn.notify_handles.iter().any(|h| h.0 & 0xF000 == WNOTIFY_KEY) {
        return;
    }
    let row = |i: usize| FORM_STAFF.get(i).filter(|r| held && r.what == "wnotify").map(|r| super::ship::WNotify { clip: r.clip, code: r.code, flags: r.flags, effects: r.effects, sounds: r.sounds, values: r.values });
    super::ship::wnotify_pass(cx, WNOTIFY_KEY, FORM_STAFF.len(), &row);
}

// (AUDIT-A PLACEMENT) The staff's A416 TresAnimNotifyState_ChangeAttachParent (kh_root 0 .. 3.1 s): the generic reader of
// every form weapon's window, combat::forms::placement::part_parent (it replaced this file's `part_parent`).

#[cfg(test)]
mod tests {
    use super::*;

    fn din(combo: i32, last: u8, reach: u8, n: i32) -> StaffDecideIn {
        StaffDecideIn { combo, reach, last_kind: last, num_enemies: 1, dist: 150.0, dz: 0.0, avatars: false, finish_count: n }
    }

    /// The ground combo near one target: 0x82, 0x83, 0x85, then the braid and the behind spin.
    #[test]
    fn the_combo_of_combo_wa() {
        let (mut last, mut reach, mut n, mut kinds) = (0u8, 0u8, 0, Vec::new());
        for combo in 0..5 {
            let d = decide(&din(combo, last, reach, n));
            kinds.push(d.kind);
            (last, reach, n) = (d.kind, d.reach, d.finish_count);
            if d.finish_end {
                break;
            }
        }
        assert_eq!(kinds, [0x82, 0x83, 0x85, 0x86, 0x87]);
        // Far (reach 1): 0x85 first; a crowd (reach 2): 0x83 first.
        assert_eq!(decide(&StaffDecideIn { dist: 600.0, ..din(0, 0, 0, 0) }).kind, 0x85);
        assert_eq!(decide(&StaffDecideIn { num_enemies: 3, ..din(0, 0, 0, 0) }).kind, 0x83);
        // The target well below: the dive; well above: the rising spin (not twice in a row).
        assert_eq!(decide(&StaffDecideIn { dz: -300.0, ..din(1, 0x82, 0, 0) }).kind, 0x84);
        assert_eq!(decide(&StaffDecideIn { dz: 300.0, ..din(1, 0x82, 0, 0) }).kind, 0x85);
        assert_eq!(decide(&StaffDecideIn { dz: 300.0, ..din(2, 0x85, 0, 0) }).kind, 0x82);
        // The images out: the beam nine times, then its finisher.
        let a = |combo| decide(&StaffDecideIn { avatars: true, ..din(combo, 0x80, 0, 0) });
        assert!((0..9).all(|c| a(c).kind == 0x80 && !a(c).finish));
        assert!(a(9).kind == 0x81 && a(9).finish_end);
    }

    #[test]
    fn the_sheet_carries_the_form() {
        for kind in 0x80..=0x87u8 {
            let r = attack_row(kind).unwrap_or_else(|| panic!("{kind:#x}"));
            assert!(crate::anim::AnimId::from_name(r.clip).is_some(), "{} {}: not in the animation table", r.id, r.clip);
        }
        assert_eq!(replaced_clip(CHANGE_WA, "M_A417"), Some("A417"));
        assert_eq!(replaced_clip(CHANGE_WA, "AirDodge"), Some("A054"));
        assert_eq!(replaced_clip(0x94, "AirDodge"), None);
        assert_eq!(proj("WA_BEAM").map(|r| (pv(r, P_SPEED), pv(r, P_LIFE), pv(r, P_RADIUS))), Some((6000.0, 3.0, 100.0)));
        assert!(proj("WA_BRAID").is_some_and(|r| has_flag(r, "no_destroy_on_hit")));
        // The braid's CAPSULE1 stands along the actor's Z (half length 400), 50 ahead and 200 up; the beams are spheres.
        let braid = proj("WA_BRAID").unwrap();
        assert_eq!(shape_axis(braid, Vec3::X), Vec3::Z * 400.0);
        assert!(shape_centre(braid, Vec3::ZERO, Vec3::Y).distance(Vec3::new(0.0, 50.0, 200.0)) < 1.0e-3);
        assert_eq!(shape_axis(proj("WA_BEAM").unwrap(), Vec3::X), Vec3::ZERO);
        // The tower's baked tracks: X_link01_skt rises with its A417; FinishWA stands upright 1200 up.
        let (low, high) = (tower_track("X_link01_skt", 0.0).unwrap(), tower_track("X_link01_skt", 6.0).unwrap());
        assert!(high[2] > low[2] + 2000.0, "{low:?} -> {high:?}");
        let w = tower_track("FinishWA", 7.0).unwrap();
        assert!((w[2] - 1200.0).abs() < 5.0 && w[5] > 0.99, "{w:?}");
        for id in ["SCWA_BEAM", "SCWA_FINISH", "SCWA_BRAID", "SCWA_FRONT", "SCWA_TOWER", "SCWA_TOWER2"] {
            assert!(crate::combat::tables::attack_row(id).is_some(), "{id}: not in sora-combat-attack-data");
        }
        let mut c = PawnCombat::default();
        assert_eq!(max_combo(&c), (5, 2));
        c.staff.avatars.out = true;
        assert_eq!(max_combo(&c), (10, 1));
    }
}
