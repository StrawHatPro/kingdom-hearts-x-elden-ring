//! (SHOT-B) Frozen Crescents (kind 15 `FZ_VACUUMWAVE`, Crystal Snow at MAX) and Flag Rampage (kind 17 `CA_VACUUMWAVE`,
//! Wheel of Fate at MAX): update 0x1413BBEE0 and its spawner 0x1413C2E70. Rows `vacuum.*` of sheet sora-shotlocks
//! (tools/shotlocks/vacuum.js).
//!
//! What KH3 does (instruction level: build/sora_tmp/weapons/SHOT-B/spec/vacuum.md; the delegate is ShootFlow vtable
//! 0x14693CA58 + 0x4F0, Begin 0x1413A5CD0 case 0xF / 0x11: names FZ_VACUUMWAVE / CA_VACUUMWAVE, group 0):
//! - sub 0: gravity off, Flying (stopped); THE WEAPON FORM: by the held Keyblade's weapon type (pawn vt+0x12C8), 7 ->
//!   ChangeWeapon vt+0x13C8(type, 7, 0, ...) = Blizzard Blades (form.SK's parts, wear form Grace) and `M_7481`, any other
//!   type -> vt+0x13C8(type, 0xF, 0, ...) = Storm Flag (form.FL's w_so093, wear form Brave) and `M_F481` (5.4333 s, group
//!   0); a swap of the held Keyblade's look, no form is started; face request; sub 4, LiftCheck(200) -> sub 3.
//! - sub 3: LiftStep -> sub 4, then sub 4's code in the same tick.
//! - sub 4 / 5: at the clip's end `ShootFlowEnd`, m_bEnableChange off, sub 2, WorldSlow off (0x14140E500(ctrl, 0):
//!   ctrl+0x878 bit 3; nothing when it is not on). (SHOT-FIX) 0x14140E500 is WorldSlow, not the aim mode's release: that
//!   is End's slot 809, so the locked list stays for the spawner's last spheres (re/decomp/14140/14140E500.c).
//! - sub 2: once m_bEnableChange is back (ShootFlowEnd's TresEnableChange, 0.3 s) and +0xB0 != 0: the weapon form of Begin
//!   back (vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1)), +0xB0 = 0; at the clip's end Finish (the tail still runs).
//! - the tail of every tick: while flagsCC bit 0 the spawner while +0x90 < N, else the bit off.
//! - flagsCC bit 0 and the queue +0x9C come from the SHARED handlers (magic/shotlocks `on_notifies`): each TresShot adds
//!   its LocationIndex to the queue (`ShootFlow::syphon_timer` holds it; the sums are Param0: 24 / 32), the TresAtk begin
//!   (slot 64) sets the bit for every kind.
//! - THE SPAWNER 0x1413C2E70 ([`spawner`]): +0xA0 counts down first; nothing while the queue is empty; the locked entry
//!   +0x110 LOCK BY LOCK (+0x114 against its lock count; a gone target: +0x110 += 1, +0x114 = 0, its locks lost); ONE
//!   projectile of +0xB8 ON the entry's lock-on component (vt+0xB88), direction its velocity (zero for a lock-on point),
//!   fired at once, no homing target (a stationary sphere, r 30, 3 s: it hits the body it sits in); +0x90 += 1, the queue
//!   -= 1, +0xA0 = 0.1 (7 ticks at 60 Hz).
//! - THE MELEE WINDOWS of the clips (TresAtk, row SF_VACUUMWAVE) run through the generic hit path (combat/hit.rs); their
//!   shapes are the form weapon's ([`hit_shape`]): Storm Flag's KB_StickL (CAPSULE2 ATK_00 -> ATK_01, r 120) tracked per
//!   frame of F481; Blizzard Blades have no KB_Stick* shape: those windows hit nothing.
//!
//! The weapon form is kept on the pawn (`PawnMagic::shot_weapon_form`): the hosts draw [`weapon_units`] and the wear form
//! [`wear_form`] while it is 7 / 0xF (INT lines in build/sora_tmp/weapons/SHOT-B/STATE.md).

use glam::Vec3;

use crate::anim::{AnimId, NotifyEvent, PlayArgs, SEQUENCES};
use crate::host::{TargetFlags, TargetId};
use crate::magic::projectile::{self, SpawnArgs};
use crate::magic::spells::cmd;
use crate::magic::tables::{Sheet, PV};
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: +0x110 / +0x114 (the spawner's locked entry and the spawns on it), +0xB0 != 0
/// (the weapon form of Begin is still owed: `restore`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    entry: usize,
    on_entry: i32,
    restore: bool,
}

/// The kind row group of a weapon form ("k15" for Blizzard Blades 7, "k17" for Storm Flag 0xF).
fn key_of_form(form: u8) -> Option<&'static str> {
    match form {
        _ if form == S.int("vacuum.k15.weapon_form") as u8 => Some("k15"),
        _ if form == S.int("vacuum.k17.weapon_form") as u8 => Some("k17"),
        _ => None,
    }
}

/// The weapon form sub 0 puts on for weapon type `weapon_type` (7 -> 7, any other -> 0xF).
pub fn weapon_form(weapon_type: u8) -> u8 {
    let key = if weapon_type == S.int("vacuum.form_weapon_type") as u8 { "k15" } else { "k17" };
    S.int(&format!("vacuum.{key}.weapon_form")) as u8
}

/// The parts of weapon form `form` (`unit@part:socket=blueprint`): what the hosts draw instead of the Keyblade while
/// `PawnMagic::shot_weapon_form` is 7 / 0xF; empty for any other form.
pub fn weapon_units(form: u8) -> &'static [&'static str] {
    match key_of_form(form).and_then(|k| S.find(&format!("vacuum.{k}.weapon_units"))) {
        Some(PV::L(units)) => units,
        _ => &[],
    }
}

/// The wear form that goes with weapon form `form` (equipment vt+0x5F8: the form's m_OwnerWearForm): Grace 4 / Brave 1;
/// `None` for any other form.
pub fn wear_form(form: u8) -> Option<u8> {
    key_of_form(form).map(|k| S.int(&format!("vacuum.{k}.wear_form")) as u8)
}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin stores the delegate, +0xB8, +0xB0 / +0xB1).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// The weapon form of Begin back (sub 2 / End: vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1)).
fn give_weapon_back(st: &mut State, cx: &mut Ctx) {
    if std::mem::take(&mut st.restore) {
        cx.pawn.magic.shot_weapon_form = 0;
    }
}

/// The update delegate 0x1413BBEE0; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    let mut running = true;
    match sf.sub {
        0 => {
            let form = weapon_form(super::weapon_type(cx.pawn));
            let pawn = &mut *cx.pawn;
            pawn.cmc.gravity_off = true;
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            pawn.magic.shot_weapon_form = form;
            st.restore = true;
            let key = key_of_form(form).unwrap_or("k17");
            let clip = AnimId::from_name(S.text(&format!("vacuum.{key}.sequence"))).unwrap_or(AnimId::F481);
            super::play(sf, cx, clip);
            sf.face_request = true;
            sf.sub = 4;
            if sf.lift_check(cx, S.num("vacuum.lift.height")) {
                sf.sub = 3;
            }
        }
        3..=5 => {
            if sf.sub == 3 && sf.lift_step(cx, dt) {
                sf.sub = 4;
            }
            if cx.pawn.anim.ended {
                cx.pawn.anim.play_with(AnimId::from_name(S.text("vacuum.clip_end")).unwrap_or(AnimId::ShootFlowEnd), PlayArgs::mode(2, -1.0));
                cx.pawn.queue.flags.enable_change = false;
                sf.sub = 2;
                // (SHOT-FIX) 0x14140E500(ctrl, 0) = WorldSlow off (1413BBEE0.c:92), not the release (End's slot 809).
                crate::combat::form::world_slow(cx, false);
            }
        }
        _ => {
            if cx.pawn.queue.flags.enable_change && st.restore {
                give_weapon_back(st, cx);
            }
            if cx.pawn.anim.ended {
                cx.pawn.queue.do_chain();
                running = false;
            }
        }
    }
    // The tail of every tick (after Finish too).
    if sf.ring_active {
        if sf.spawn_index < sf.n {
            spawner(sf, st, cx, dt);
        } else {
            sf.ring_active = false;
        }
    }
    running
}

/// The spawner 0x1413C2E70(dt): at most one projectile per call (see the module doc). The queue +0x9C is
/// `ShootFlow::syphon_timer` (the shared Shot handler adds LocationIndex to it).
fn spawner(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) {
    if sf.ring_timer > 0.0 {
        sf.ring_timer -= dt;
        if sf.ring_timer > 0.0 {
            return;
        }
        sf.ring_timer = 0.0;
    }
    if sf.syphon_timer < 1.0 {
        return;
    }
    let entry = cx.pawn.magic.aim.locked.get(st.entry).copied();
    let info = entry.and_then(|e| cx.target(e.target)).filter(|t| t.flags.has(TargetFlags::ALIVE));
    let (Some(e), Some(t)) = (entry, info) else {
        st.entry += 1;
        st.on_entry = 0;
        return;
    };
    let Some(def) = super::projectile_named(cx, sf.proj_name) else { return };
    // On the lock-on component (vt+0xB88), turned by its velocity (vt+0x418): a lock-on point does not move (zero).
    let min = S.text("vacuum.spawn.min_len_sq").parse::<f32>().unwrap_or(0.0);
    let v = Vec3::ZERO;
    let direction = if v.length_squared() > min { v.normalize() } else { v };
    projectile::spawn(cx, &SpawnArgs { def, position: t.center, direction, cmd: cmd::SHOOTFLOW, shoot_flow: sf.kind, finish: false, situation: false, full_mp: false, combo: [0; 4], target: TargetId::NONE, attached: None, fire: true });
    st.on_entry += 1;
    sf.spawn_index += 1;
    sf.syphon_timer -= 1.0;
    if st.on_entry >= e.lock_count {
        st.on_entry = 0;
        st.entry += 1;
    }
    sf.ring_timer = S.num("vacuum.spawn.interval");
}

/// Every notify event of a pose tick: nothing of its own (the shared Shot / TresAtk handlers fill the queue and the bit).
pub(crate) fn on_notify(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx, _event: &NotifyEvent) {}

/// Before the common End's release of the aim mode: a state dropped before sub 2 gives the weapon back (End 0x1413ABA70:
/// +0xB0 != 0).
pub(crate) fn end(_sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    give_weapon_back(st, cx);
}

/// The Shot handler's spawn point: none (the TresShots of these kinds only queue hits).
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, _cx: &Ctx, _index: usize) -> Option<Vec3> {
    None
}

/// The shape of a hit window while this family's weapon form is on (combat/hit.rs `shape`): `Some(None)` = the form
/// weapon has no primitive of the group (Blizzard Blades' KB_Stick*: no hit), `Some(Some(capsule))` = Storm Flag's
/// KB_StickL at the clip time of sequence `sequence` (F481's track, turned by his yaw), `None` = not this family's.
pub(crate) fn hit_shape(cx: &Ctx, group: &str, sequence: usize) -> Option<Option<(Vec3, Vec3, f32)>> {
    let key = key_of_form(cx.pawn.magic.shot_weapon_form)?;
    let groups = |id: &str| match S.find(id) {
        Some(PV::L(list)) => list.contains(&group),
        _ => false,
    };
    if groups(&format!("vacuum.{key}.groups_without_shape")) {
        return Some(None);
    }
    if key != "k17" || group != "KB_StickL" {
        return None;
    }
    let seq = SEQUENCES.get(sequence)?;
    if seq.name != S.text("vacuum.k17.sequence") {
        return None;
    }
    let track = S.list("vacuum.k17.track_kb_stickl");
    let frames = S.int("vacuum.k17.track_frames").max(1) as usize;
    if track.len() < frames * 7 {
        return None;
    }
    let playing = cx.pawn.anim.name.data().samples.iter().any(|&(_, s)| s == sequence);
    let time = if playing { cx.pawn.anim.time } else { seq.length };
    let f = if seq.length > 0.0 { (time / seq.length).clamp(0.0, 1.0) * (frames - 1) as f32 } else { 0.0 };
    let (i, w) = ((f.floor() as usize).min(frames - 1), f - f.floor());
    let j = (i + 1).min(frames - 1);
    let at = |k: usize| track[i * 7 + k] + (track[j * 7 + k] - track[i * 7 + k]) * w;
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    let world = |v: Vec3| cx.pawn.cmc.location + Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z);
    Some(Some((world(Vec3::new(at(0), at(1), at(2))), world(Vec3::new(at(3), at(4), at(5))), at(6))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        for k in [15, 17] {
            assert_eq!(super::super::family(k), "vacuum", "kind {k}");
            assert!(super::super::ready(k));
        }
        assert_eq!((super::super::max_kind(7, false), super::super::max_kind(10, false)), (15, 17));
        assert_eq!((weapon_form(7), weapon_form(10), weapon_form(1)), (7, 0xF, 0xF));
        assert_eq!((wear_form(7), wear_form(0xF), wear_form(9)), (Some(4), Some(1), None));
        assert!(weapon_units(0xF).iter().any(|u| u.ends_with("w_so093_Wep")) && weapon_units(7).len() == 4 && weapon_units(9).is_empty());
        let sum = |k: &str| S.list(&format!("vacuum.{k}.shot_counts")).iter().sum::<f32>() as i32;
        assert_eq!((sum("k15"), sum("k17")), (24, 32), "the TresShot counts add up to Param0");
        assert_eq!((S.num("vacuum.lift.height"), S.num("vacuum.spawn.interval")), (200.0, 0.1));
        assert_eq!(S.list("vacuum.k17.track_kb_stickl").len(), S.int("vacuum.k17.track_frames") as usize * 7);
        assert_eq!((AnimId::from_name(S.text("vacuum.k15.sequence")), AnimId::from_name(S.text("vacuum.k17.sequence"))), (Some(AnimId::A7481), Some(AnimId::F481)));
    }
}
