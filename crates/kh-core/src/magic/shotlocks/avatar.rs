//! (SHOT-C avatar; moved from SHOT-A 2026-11-03) Ghost Horde (kind 8, Favorite Deputy = weapon type 4's base kind) and
//! Phantom Rush (kind 25, Classic Tone = weapon type 12's base kind): update 0x1413B5E30. Rows `avatar.*` of sheet
//! sora-shotlocks (tools/shotlocks/avatar.js, which carries the full reading of the code).
//!
//! What KH3 does (decompiled C, constants checked in the disassembly; ShootFlow vtable 0x14693CA58 + 0x498; Begin
//! 0x1413A5CD0 case 0x19 = +0xC0 0x76C, +0xC4 1, then case 8's delegate; neither names a projectile):
//! - sub 0: gravity off, Flying stopped, `M_1454` (group +0xC0, voice group +0xC4), m_bFaceAt, sub 4; LiftCheck(100) ->
//!   sub 3 (LiftStep, then sub 4's code in the same tick).
//! - sub 4: at the clip's end the body collision off (pawn vt+0x1010(0, 0)), WorldSlow on (0x14140E500(ctrl, 1): types
//!   0x419C at 0.1), +0xA4 = 0.133333, +0xC8 = 10, sub 1.
//! - sub 1: the AVATARS (0x1413BD290(1), one per locked TARGET every 0.3 s after the first 0.133333 s): the held
//!   Keyblade's m_pShootFlowEffect particle spawned at his lock point and moved (0x1413BDB80, 6 x dt of the move a tick,
//!   for 0.166666 s) to 100 cm past its target; when every avatar arrived (as many as locked targets) or after 10 s:
//!   sub 5, +0xA4 = 0.5. The avatars do no damage.
//!   THE ATTACK CAMERA (0x1413BD290 after each avatar: 0x14140DF20(ctrl, 0) then (ctrl, 1, mode 2, the avatar's target,
//!   &ShootFlow+0xE4)): (SHOT-FIX) published through combat/camera.rs (`request_at`: mode 2, the target, the anchor
//!   +0xE4) as the other states' camera modes are.
//! - sub 5: after 0.5 s the avatars off (0x1413BD290(0)), sub 6, the attack camera off (0x14140DF20(ctrl, 0)).
//! - sub 6: the body collision on, StopMovement, `M_1456`, THE RUSH (0x1413BF420): WorldSlow off; per locked target that
//!   exists, per lock, one "SF_AVATARRUSH" projectile of the held set (TresProjTS_AvatarRush: SF_AVATARRUSH; TR_: SF_
//!   TRAVATARRUSH; static, sphere 30, life 0.6) at the target, attached to it, its fire delayed 0.033333 s x the lock's
//!   index; m_bEnableChange off, sub 7.
//! - sub 7: `ShootFlowEnd` at M_1456's end, sub 2; sub 2: Finish at its end.
//! - M_1456's TresShot (0.5 s): kind 8 is the Shot handler's empty case, kind 25 its default branch with no projectile
//!   name: nothing (the shared handler's `projectile_named("-")`).
//! - End 0x1413ABA70: the attack camera off, WorldSlow off, the +0x148 avatars destroyed AT ONCE (component vt+0x368 =
//!   DestroyComponent; sub 5's 0x1413BD290(0) only deactivates them: 0x1442633A0(comp, 0.1) / vt+0xAE8).
//!
//! The port: the avatars are kept on the pawn (`PawnMagic::shot_avatars`, the state's +0x148 list; a state struct is
//! `Copy`); the hosts move each avatar's persistent effect to its `position` (INT line). (SHOT-FIX) The rush spawns every
//! projectile at once, as 0x1413BF420 does: each at its target, attached to it (magic/projectile.rs `attach_to`), with
//! SetDelayFire(1, 0.033333 x i, 0) and StartFire (`set_delay_fire` / `start_fire`): the projectile waits its delay with
//! its collision off, then fires (the projectile's own tick 0x141484B10). They live on without the state.

use glam::Vec3;

use crate::anim::{AnimId, NotifyEvent};
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, TargetId};
use crate::magic::projectile::{self, SpawnArgs};
use crate::magic::shotlock::lock_point;
use crate::magic::spells::cmd;
use crate::magic::tables::Sheet;
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// One avatar (an entry of ShootFlow +0x148, 0x24 bytes): its particle, where it is, the move still to make, its timer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Avatar {
    /// The persistent effect (the held Keyblade's m_pShootFlowEffect); the hosts move it to `position`.
    pub effect: Option<EffectHandle>,
    pub position: Vec3,
    /// Orientation, degrees (pitch, yaw, roll): the rotation of the direction to its target.
    pub rotation: Vec3,
    movement: Vec3,
    timer: f32,
}

/// The family's fields of the state object.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    /// +0xA4 (the avatar / finish timer), +0xC8 (the 10 s cap).
    timer: f32,
    cap: f32,
    /// +0x110 (avatars arrived), +0x114 (the next locked entry).
    arrived: i32,
    next: usize,
    /// WorldSlow is on (ctrl+0x878 bit 3) / the body collision is off: what End gives back.
    slowed: bool,
    body_off: bool,
}

fn num(id: &str) -> f32 {
    S.num(&format!("avatar.{id}"))
}

/// After the common Begin and the per-kind set-up: nothing of its own.
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

fn play(sf: &ShootFlow, cx: &mut Ctx, clip: AnimId, voice: i32) {
    super::play(sf, cx, clip);
    cx.set_voice_group_no(voice);
}

/// The update delegate 0x1413B5E30; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    match sf.sub {
        0 => {
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            play(sf, cx, AnimId::M1454, sf.voice);
            sf.face_request = true;
            sf.sub = 4;
            if sf.lift_check(cx, num("lift.height")) {
                sf.sub = 3;
            }
        }
        3 | 4 => {
            if sf.sub == 3 && sf.lift_step(cx, dt) {
                sf.sub = 4;
            }
            if cx.pawn.anim.ended {
                // pawn vt+0x1010(0, 0): the body collision off; WorldSlow(ctrl, 1).
                cx.pawn.actions.body_collision_off = true;
                st.body_off = true;
                sf.sub = 1;
                crate::combat::form::world_slow(cx, true);
                st.slowed = true;
                (st.timer, st.cap, st.arrived, st.next) = (num("first_delay"), num("max_time"), 0, 0);
                // +0xE4 = the root component's location (UE's actor location: the capsule centre). (SHOT-FIX) the
                // attack camera's anchor.
                sf.origin = cx.pawn.cmc.capsule_centre(cx.pawn.params());
            }
        }
        1 => {
            st.cap -= dt;
            if move_avatars(st, cx, dt) || st.cap <= 0.0 {
                sf.sub = 5;
                st.timer = num("finish_wait");
            } else {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    spawn_avatar(sf, st, cx);
                    st.timer = num("interval");
                }
            }
        }
        5 => {
            st.timer -= dt;
            if st.timer <= 0.0 {
                avatars_off(cx, false);
                sf.sub = 6;
                // (SHOT-FIX) thunk 0x14140DF20(ctrl, 0, 0, 0, 0): the attack camera off (1413B5E30.c:103).
                crate::combat::camera::off(cx);
            }
        }
        6 => {
            // vt+0x1010(1, 0), StopMovement, vt+0x1628 (MaxFlySpeed default: the port's is unchanged).
            cx.pawn.actions.body_collision_off = false;
            st.body_off = false;
            cx.pawn.cmc.stop_movement();
            play(sf, cx, AnimId::M1456, 0);
            rush(sf, st, cx);
            cx.pawn.queue.flags.enable_change = false;
            sf.sub = 7;
        }
        7 => {
            if cx.pawn.anim.ended {
                play(sf, cx, AnimId::ShootFlowEnd, 0);
                cx.pawn.queue.flags.enable_change = false;
                sf.sub = 2;
            }
        }
        _ => {
            if cx.pawn.anim.ended {
                // vt+0x1388(0, 0, 0) (the weapon presentation: not in the port), Finish.
                cx.pawn.queue.do_chain();
                return false;
            }
        }
    }
    true
}

/// 0x1413BD290(1): the avatar of locked entry +0x114 (+0x114 counts on even when its target is gone); then the attack
/// camera on its target (SHOT-FIX: 1413BD290.c:223-224, off then on: mode 2, the entry's component, &ShootFlow+0xE4).
fn spawn_avatar(sf: &ShootFlow, st: &mut State, cx: &mut Ctx) {
    let Some(entry) = cx.pawn.magic.aim.locked.get(st.next).copied() else { return };
    st.next += 1;
    let Some(t) = cx.target(entry.target) else { return };
    let from = lock_point(cx);
    let delta = t.center - from;
    let d = if delta.length_squared() > 1.0e-8 { delta.normalize() } else { delta };
    let movement = delta + d * num("move.beyond");
    let rotation = Vec3::new(d.z.clamp(-1.0, 1.0).asin().to_degrees(), d.y.atan2(d.x).to_degrees(), 0.0);
    let key = cx.pawn.status.current_keyblade().map(|k| k.key().to_ascii_lowercase()).unwrap_or_default();
    let effect = S.text_or(&format!("avatar.effect.{key}"), "");
    let effect = (!effect.is_empty()).then(|| cx.spawn_effect(EffectSpawn { id: EffectId(crate::notify::notify_asset_id(effect)), name: effect, at: Attach::World(from), rotation, scale: 1.0, persistent: true }));
    cx.pawn.magic.shot_avatars.push(Avatar { effect, position: from, rotation, movement, timer: num("move.time") });
    crate::combat::camera::off(cx);
    crate::combat::camera::request_at(cx, S.int("avatar.camera.mode") as u8, entry.target, sf.origin);
}

/// 0x1413BDB80: the avatars move; true when as many as there are locked targets have arrived.
fn move_avatars(st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    let locked = cx.pawn.magic.aim.locked.len() as i32;
    if st.arrived >= locked {
        return true;
    }
    st.arrived = 0;
    let (flat, rate, cap) = (num("move.flat_below"), num("move.rate"), num("move.cap"));
    for a in cx.pawn.magic.shot_avatars.iter_mut() {
        if a.timer > 0.0 {
            if a.timer <= flat {
                a.movement.z = 0.0;
            }
            a.position += a.movement * (dt * rate).min(cap);
            a.timer -= dt;
        } else {
            st.arrived += 1;
        }
    }
    st.arrived >= locked
}

/// 0x1413BD290(0) (`immediate` false: the first avatar's 0x1442633A0(comp, 0.1), the others' vt+0xAE8 = deactivate) /
/// End (`immediate`: vt+0x368 = DestroyComponent): every avatar's particle off, the list emptied.
fn avatars_off(cx: &mut Ctx, immediate: bool) {
    for a in std::mem::take(&mut cx.pawn.magic.shot_avatars) {
        if let Some(h) = a.effect {
            cx.stop_effect(h, immediate);
        }
    }
}

/// 0x1413BF420: the rush, once (+0x90 < 1): WorldSlow off; per locked target that exists, per lock, one SF_AVATARRUSH
/// spawned NOW at the target's component location (vt+0xB88), vt+0x1100(cmd, kind), attached to the component
/// (FUN_143E8FC60, KeepWorldTransform), SetDelayFire(1, `delay_step` x the lock's index within its entry, 0) and StartFire
/// (vt+0x1270): the lock i fires i x 0.033333 s later (the first at once). (SHOT-FIX: was spawned late by the state.)
fn rush(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    if sf.spawn_index >= 1 {
        return;
    }
    crate::combat::form::world_slow(cx, false);
    st.slowed = false;
    let step = num("rush.delay_step");
    let Some(def) = super::projectile_named(cx, S.text("avatar.rush.proj")) else { return };
    let locked: Vec<(TargetId, i32)> = cx.pawn.magic.aim.locked.iter().map(|e| (e.target, e.lock_count)).collect();
    for (target, count) in locked {
        let Some(t) = cx.target(target) else { continue };
        let mut delay = 0.0;
        for _ in 0..count.max(0) {
            let id = projectile::spawn(cx, &SpawnArgs { def, position: t.center, direction: Vec3::ZERO, cmd: cmd::SHOOTFLOW, shoot_flow: sf.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target: TargetId::NONE, attached: None, fire: false });
            if let Some(p) = cx.pawn.magic.projectiles.iter_mut().find(|p| p.id == id) {
                p.attach_to(&t);
                p.set_delay_fire(true, delay, 0.0);
            }
            projectile::start_fire(cx, id);
            delay += step;
            sf.spawn_index += 1;
        }
    }
}

/// Every notify event of a pose tick: nothing of its own (M_1456's TresShot goes to the shared handler: no projectile).
pub(crate) fn on_notify(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx, _event: &NotifyEvent) {}

/// Before the common End's release: End 0x1413ABA70 turns the attack camera off (0x14140DF20(ctrl, 0)) and WorldSlow off
/// (0x14140E500(ctrl, 0)) and DESTROYS the +0x148 avatars at once (vt+0x368). PORT ADDITION for a state dropped early:
/// the body collision back on. The rush projectiles live on without the state (their own delayed fire).
pub(crate) fn end(_sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    crate::combat::camera::off(cx); // (SHOT-FIX)
    if std::mem::take(&mut st.slowed) {
        crate::combat::form::world_slow(cx, false);
    }
    if std::mem::take(&mut st.body_off) {
        cx.pawn.actions.body_collision_off = false;
    }
    avatars_off(cx, true); // (SHOT-FIX) DestroyComponent: at once
}

/// The Shot handler's spawn point: none (no projectile name).
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, _cx: &Ctx, _index: usize) -> Option<Vec3> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_and_the_kinds() {
        for k in [8, 25] {
            assert_eq!(super::super::family(k), "avatar", "kind {k}");
        }
        assert_eq!((super::super::base_kind(4), super::super::base_kind(12)), (8, 25));
        assert_eq!(S.text("avatar.rush.proj"), "SF_AVATARRUSH");
        assert_eq!(S.text("avatar.effect.wep_keyblade_so_11"), "vw_so110_stf_sor00");
        assert!((num("move.rate") * num("move.time") - 1.0).abs() < 1.0e-3, "the whole move in the timer");
        assert_eq!(S.int("avatar.camera.mode"), 2); // (SHOT-FIX)
    }

    /// (SHOT-FIX) End 0x1413ABA70 destroys the +0x148 avatars at once (component vt+0x368), sub 5's 0x1413BD290(0) only
    /// deactivates them; End also turns the attack camera off.
    #[test]
    fn end_destroys_the_avatars_at_once() {
        use crate::anim::AnimPlayer;
        use crate::cmc::Cmc;
        use crate::host::{Call, RecordingHost};
        use crate::params::SORA_PARAMS;
        use crate::pawn::Pawn;
        use crate::test_world::TestWorld;
        let mut p = Pawn::new(SORA_PARAMS, Cmc::new(&SORA_PARAMS, Vec3::ZERO), 0.0, AnimPlayer::new(AnimId::IdleN));
        let world = TestWorld::floor();
        let mut rec = RecordingHost::new();
        let handles = rec.step(&world, |h| {
            let mut cx = Ctx::new(&mut p, h);
            let mut sf = ShootFlow { kind: 8, ..ShootFlow::RESET };
            let mut st = State::default();
            let mut handles = Vec::new();
            for _ in 0..2 {
                let e = cx.spawn_effect(EffectSpawn { id: EffectId(1), name: "vw_so050_stf_tsn1", at: Attach::World(Vec3::ZERO), rotation: Vec3::ZERO, scale: 1.0, persistent: true });
                handles.push(e);
                cx.pawn.magic.shot_avatars.push(Avatar { effect: Some(e), position: Vec3::ZERO, rotation: Vec3::ZERO, movement: Vec3::ZERO, timer: 0.1 });
            }
            crate::combat::camera::request_at(&mut cx, 2, TargetId(1), Vec3::ZERO);
            end(&mut sf, &mut st, &mut cx);
            assert!(cx.pawn.magic.shot_avatars.is_empty());
            assert_eq!((cx.pawn.combat.attack_cam.mode, cx.pawn.combat.attack_cam.anchor), (0, None), "the attack camera off");
            handles
        });
        for h in handles {
            assert!(rec.calls.contains(&Call::StopEffect(h, true)), "{h:?} destroyed at once");
        }
    }
}
