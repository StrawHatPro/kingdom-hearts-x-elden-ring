//! (SHOT-A rush) Drill Dive (kind 9 "トイ高速", kh3_system.command.sf_rush_0; Favorite Deputy's MAX kind; SHOT-C: kind 26
//! Noise Flux, Classic Tone's MAX kind): update 0x1413BC550 (TresPlayerState_ShootFlow vtable 0x14693CA58 +0x4B8), the
//! dash 0x1413BE650 (+0x4C8) and the warp 0x1413C4030 (+0x4C0). Rows `rush.*` of sheet sora-shotlocks
//! (tools/shotlocks/rush.js).
//!
//! What KH3 does (decompiled C and its disassembly; Begin 0x1413A5CD0 case 9: group 0, voice 0; case 0x1A: group 0x76C,
//! voice 1; no projectile name):
//! - sub 0: gravity off, Flying (stopped); the WEAPON CHANGE pawn vt+0x13C8(weapon type, 6, ...) to Drill Punch's weapon
//!   w_so053 and its anim set AnimSet6 (kind 0x1A: 0x15, Clock Drill's w_so113; no form is started); Play `M_6481`
//!   (0.6 s, mode 2, group +0xC0), voice +0xC4; m_bFaceAt; sub 4; LiftCheck(200) -> sub 3 (LiftStep, then sub 4).
//! - at M_6481's end (sub 3 / 4): the body collision off (vt+0x1010(0)), sub 5, the WORLD SLOW on (0x14140E500(ctrl, 1):
//!   object types 0x419C at 0.1), the entry index +0x110 = 0, +0xD0 = none, +0x104 = his rotation, +0xAC = its yaw,
//!   +0xE4 = +0xD8 = his centre; the first warp.
//! - THE WARP 0x1413C4030: the locked entry +0x110 (the aim mode's list, in order) must still be there (KH3: its
//!   component valid and not pending kill), else no warp. P = its lock-on point - 100 z; +0xAC -= 120; D = (cos, sin,
//!   0) of +0xAC; +0xD8 = P - 800 D, +0xF8 = P + 800 D; a new target: the attack camera off, then mode 2 on it
//!   (combat/camera.rs keeps mode 2's state only: its camera actor is not ported); SetActorLocation(+0xD8) (no sweep);
//!   Turn(0, +0xAC, 0) at alpha 1; the right-hand weapon's effect "WarpRush" (`ead_so053_stf0`, group 0x686; kind 0x1A:
//!   `ead_so113_stf0`, 0x76C); +0x114 += 1, at the entry's lock count the next entry and +0xAC -= 120 more. So Sora
//!   dives through every locked target once per lock, from 800 cm before it to 800 cm past it, each dive turned 120
//!   degrees further round.
//! - each warp: Play `M_6482` (0.1667 s looped: mode 3, group +0xC0), voice 0, +0xA4 = 0.3; kind 0x1A after the first
//!   warp: his mesh hidden (`PawnMagic::shot_body_hidden`).
//! - sub 5, THE DASH 0x1413BE650: SetActorLocation(+0xF8 - +0xA4 x 3.3333333 x (+0xF8 - +0xD8)), +0xA4 -= dt; at <= 0 the
//!   next warp; none left: the attack camera off, his mesh shown, Play `M_6483` (0.9667 s, mode 2, group +0xC0), voice
//!   +0xC4, SetActorLocation(+0xE4) and SetActorRotation(+0x104) (back where he started), sub 8.
//! - sub 8: at M_6483's end Play `ShootFlowEnd` (group +0xC0), voice 99, m_bEnableChange = 0, sub 2.
//! - sub 2: once m_bEnableChange is back (ShootFlowEnd's TresEnableChange 0.3 s) and +0xB0 != 0: the weapon of Begin back,
//!   the body collision on, the world slow off; at the clip's end StopMovement and Finish.
//! - THE HITS: M_6482's attack window (the whole clip, group StickL, row SF_WARPRUSH: power 1.2, SIDESPIN, magic,
//!   AttackInterval 0.3) runs through the generic hit path (combat/hit.rs; this module answers its shape: [`hit_shape`],
//!   the drill weapon's StickL capsule (radius 120, ATK_00 -> ATK_01) posed over M_6482 by FK), so every dive hits what it
//!   passes once, on every target the host's lock-on accepts. Kind 0x1A hits with SF_TRWARPRUSH (the ShootFlow TresAtk
//!   begin rename 0x1413B3590): [`update`] renames the window's row before the collision tick of its first frame.
//!
//! The weapon is kept on the pawn (`PawnMagic::shot_weapon_form`): the hosts draw that form's weapon parts
//! ([`weapon_units`]) meanwhile.

use glam::Vec3;

use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyEvent, NotifyPhase, PlayArgs, SEQUENCES};
use crate::host::{Attach, EffectId, EffectSpawn, EventKind, TargetId};
use crate::magic::tables::{Sheet, PV};
use crate::pawn::Ctx;
use crate::states::shoot_flow::ShootFlow;

const S: Sheet = Sheet::Shotlocks;

/// The family runs (`magic/shotlocks::ready`).
pub(crate) const PORTED: bool = true;

/// The family's fields of the state object: +0x110 / +0x114 (the locked entry the warps serve and its warps so far),
/// +0xD8 / +0xF8 (the dash's start and end), +0x104 (his rotation at the first warp: pitch, yaw), +0xB0 != 0 (Begin's
/// weapon is to come back: `restore`), and what the port undoes in End (the world slow, the body collision). The timer
/// +0xA4 is the state's `syphon_timer`, +0xAC its `face_yaw`, +0xD0 its `first_target`, +0xE4 its `origin`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct State {
    entry: i32,
    warps: i32,
    start: Vec3,
    end: Vec3,
    rotation: (f32, f32),
    restore: bool,
    slowed: bool,
    body_off: bool,
}

/// The kind whose rows hold this kind's values: the code branches on 0x1A (Noise Flux) only, every other kind of the
/// update is Drill Dive's (9).
fn row_kind(kind: u8) -> u8 {
    if i32::from(kind) == S.int("rush.toon_kind") {
        kind
    } else {
        9
    }
}

/// The weapon form the update puts on (vt+0x13C8's second argument): 6 Drill Punch, kind 0x1A: 0x15 Clock Drill.
pub fn weapon_form(kind: u8) -> u8 {
    S.int(&format!("rush.k{}.weapon_form", row_kind(kind))) as u8
}

/// The weapon parts of that form (`unit@part:socket=blueprint`): what the hosts draw instead of the Keyblade while
/// `PawnMagic::shot_weapon_form` is [`weapon_form`].
pub fn weapon_units(kind: u8) -> &'static [&'static str] {
    match S.find(&format!("rush.k{}.weapon_units", row_kind(kind))) {
        Some(PV::L(units)) => units,
        _ => &[],
    }
}

/// The kind of this family whose weapon form is on (None: no drill of a rush shotlock).
fn kind_of_form(form: u8) -> Option<u8> {
    [9, S.int("rush.toon_kind") as u8].into_iter().find(|&k| form != 0 && weapon_form(k) == form)
}

/// The clip of a step (`start` M_6481, `dash` M_6482, `end` M_6483 of AnimSet6).
fn clip(key: &str) -> AnimId {
    let fallback = match key {
        "start" => AnimId::A6481,
        "dash" => AnimId::A6482,
        _ => AnimId::A6483,
    };
    AnimId::from_name(S.text(&format!("rush.clip.{key}.sequence"))).unwrap_or(fallback)
}

/// `Play(clip, slot 0, blend -1, mode, +0xC0, rate 1)` (`None`: ShootFlowEnd, mode 2), then the voice group (anim
/// interface vt+0x1F0).
fn play(sf: &ShootFlow, cx: &mut Ctx, key: Option<&str>, voice: i32) {
    let (id, mode) = match key {
        Some(k) => (clip(k), S.int(&format!("rush.clip.{k}.mode")) as u8),
        None => (AnimId::ShootFlowEnd, 2),
    };
    if sf.group != 0 {
        cx.pawn.next_effect_group = Some(sf.group);
    }
    cx.pawn.anim.play_with(id, PlayArgs::mode(mode, -1.0));
    cx.set_voice_group_no(voice);
}

/// After the common Begin and the per-kind set-up: nothing of its own (Begin cases 9 / 0x1A: the delegate, group, voice).
pub(crate) fn begin(_sf: &mut ShootFlow, _st: &mut State, _cx: &mut Ctx) {}

/// Begin's weapon back (sub 2 / End: vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1), +0xB0 = 0).
fn give_weapon_back(st: &mut State, cx: &mut Ctx) {
    if std::mem::take(&mut st.restore) {
        cx.pawn.magic.shot_weapon_form = 0;
    }
}

/// `WorldSlow(ctrl, on)` 0x14140E500.
fn world_slow(st: &mut State, cx: &mut Ctx, on: bool) {
    st.slowed = on;
    crate::combat::form::world_slow(cx, on);
}

/// pawn vt+0x1010(on, 0): the body collision.
fn body_collision(st: &mut State, cx: &mut Ctx, on: bool) {
    st.body_off = !on;
    cx.pawn.actions.body_collision_off = !on;
}

/// SetActorLocation(centre) without a sweep (FUN_143EAC3D0(pawn, p, 0, null, 0)): UE's actor location is the capsule
/// centre, the port's his feet.
fn put(cx: &mut Ctx, centre: Vec3) {
    let up = cx.pawn.cmc.capsule_centre(cx.pawn.params()) - cx.pawn.cmc.location;
    cx.pawn.cmc.location = centre - up;
}

/// Sub 0: the weapon change, M_6481, the lift check.
fn start(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    let pawn = &mut *cx.pawn;
    pawn.cmc.gravity_off = true;
    pawn.cmc.stop_movement();
    pawn.cmc.set_flying();
    pawn.magic.shot_weapon_form = weapon_form(sf.kind);
    st.restore = true;
    play(sf, cx, Some("start"), sf.voice);
    sf.face_request = true;
    sf.sub = 4;
    if sf.lift_check(cx, S.num("rush.lift.height")) {
        sf.sub = 3;
    }
}

/// M_6481's end: the body collision off, the world slow on, the origin and the first warp.
fn first_warp(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    body_collision(st, cx, false);
    sf.sub = 5;
    world_slow(st, cx, true);
    // FUN_1405D4D90(pawn, 1, 0): pawn+0x468's flag and his location kept at pawn+0x500 (no core part).
    st.entry = 0;
    sf.first_target = TargetId::NONE;
    st.rotation = (cx.pawn.pitch, cx.pawn.yaw);
    sf.face_yaw = cx.pawn.yaw;
    let centre = super::super::shotlock::lock_point(cx);
    sf.origin = centre;
    st.start = centre;
    if !warp(sf, st, cx) {
        no_warp_left(sf, st, cx);
        return;
    }
    play(sf, cx, Some("dash"), S.int("rush.voice.dash"));
    if row_kind(sf.kind) != 9 {
        // vt+0xB58 -> SetVisibility(0, 1): his mesh hidden.
        cx.pawn.magic.shot_body_hidden = true;
    }
    sf.syphon_timer = S.num("rush.dash.time");
}

/// The warp vt+0x4C0 = 0x1413C4030: true when there was a locked entry to warp to.
fn warp(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) -> bool {
    let entry = usize::try_from(st.entry).ok().and_then(|i| cx.pawn.magic.aim.locked.get(i).copied());
    let Some(entry) = entry else { return false };
    let previous = sf.first_target;
    // The entry's component (a weak pointer): gone or pending kill -> +0xD0 = none, false. The port: the host no longer
    // reports the target (KH3 destroys a dead enemy's actor only after its death motion: a dying target is still warped
    // to, as in KH3).
    let Some(t) = cx.target(entry.target) else {
        sf.first_target = TargetId::NONE;
        return false;
    };
    sf.first_target = entry.target;
    let p = t.center + Vec3::Z * S.num("rush.warp.down");
    sf.face_yaw -= S.num("rush.warp.turn");
    let yaw = sf.face_yaw;
    let (sin, cos) = yaw.to_radians().sin_cos();
    let d = Vec3::new(cos, sin, 0.0) * S.num("rush.warp.reach");
    st.start = p - d;
    st.end = p + d;
    // (state vt+0x140 = GetOwner: its result is not used.) A new target actor: the attack camera on it, mode 2.
    if entry.target != previous {
        crate::combat::camera::off(cx);
        crate::combat::camera::request(cx, true, S.int("rush.warp.camera_mode") as u8, entry.target);
    }
    put(cx, st.start);
    crate::states::common::turn(cx.pawn, yaw, 1.0);
    // The right-hand weapon's TresEffectAttach0 "WarpRush" (FUN_1405B9370(pawn+0x440, data, group, 0)).
    let k = row_kind(sf.kind);
    let socket = S.text(&format!("rush.k{k}.effect_socket"));
    cx.spawn_effect(EffectSpawn { id: EffectId(S.int(&format!("rush.k{k}.effect_group")) as u32), name: S.text(&format!("rush.k{k}.effect")), at: Attach::Sora { bone: Some(socket), offset: Vec3::ZERO }, rotation: Vec3::ZERO, scale: 1.0, persistent: false });
    st.warps += 1;
    if st.warps >= entry.lock_count {
        st.entry += 1;
        st.warps = 0;
        sf.face_yaw -= S.num("rush.warp.turn");
    }
    true
}

/// The dash vt+0x4C8 = 0x1413BE650(dt): true when its 0.3 s are over.
fn dash(sf: &mut ShootFlow, st: &State, cx: &mut Ctx, dt: f32) -> bool {
    let k = sf.syphon_timer * S.num("rush.dash.rate");
    put(cx, st.end - (st.end - st.start) * k);
    sf.syphon_timer -= dt;
    if sf.syphon_timer <= 0.0 {
        sf.syphon_timer = 0.0;
        return true;
    }
    false
}

/// No warp left: the attack camera off, his mesh shown, M_6483 where he started, sub 8.
fn no_warp_left(sf: &mut ShootFlow, st: &State, cx: &mut Ctx) {
    crate::combat::camera::off(cx);
    cx.pawn.magic.shot_body_hidden = false;
    play(sf, cx, Some("end"), sf.voice);
    put(cx, sf.origin);
    // SetActorRotation(+0x104) (FUN_143EAD160): his rotation at the first warp.
    (cx.pawn.pitch, cx.pawn.yaw) = st.rotation;
    sf.sub = 8;
}

/// The ShootFlow TresAtk begin rename 0x1413B3590 for kind 0x1A (SF_WARPRUSH -> SF_TRWARPRUSH), on the windows the pose
/// tick of this frame opened (combat/hit.rs `on_notifies`; the collision tick that tests them comes after this update).
fn rename_rows(sf: &ShootFlow, cx: &mut Ctx) {
    let (from, to) = (S.text("rush.k9.row"), S.text(&format!("rush.k{}.row", row_kind(sf.kind))));
    if from == to {
        return;
    }
    let Some((index, row)) = crate::combat::tables::attack_row(to) else { return };
    for w in cx.pawn.combat.windows.iter_mut().filter(|w| w.row_name.eq_ignore_ascii_case(from)) {
        w.row = index as u32;
        w.row_name = row.id;
    }
}

/// The update delegate 0x1413BC550; `false` = the state finished this tick (no PostTick).
pub(crate) fn update(sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx, dt: f32) -> bool {
    rename_rows(sf, cx);
    match sf.sub {
        0 => start(sf, st, cx),
        3 | 4 => {
            // Sub 3's LiftStep, then sub 3 and 4 wait for M_6481's end.
            if sf.sub == 3 && sf.lift_step(cx, dt) {
                sf.sub = 4;
            }
            if cx.pawn.anim.ended {
                first_warp(sf, st, cx);
            }
        }
        5 => {
            if dash(sf, st, cx, dt) {
                if warp(sf, st, cx) {
                    play(sf, cx, Some("dash"), S.int("rush.voice.dash"));
                    sf.syphon_timer = S.num("rush.dash.time");
                } else {
                    no_warp_left(sf, st, cx);
                }
            }
        }
        8 => {
            if cx.pawn.anim.ended {
                play(sf, cx, None, S.int("rush.voice.end"));
                cx.pawn.queue.flags.enable_change = false;
                sf.sub = 2;
            }
        }
        _ => {
            if cx.pawn.queue.flags.enable_change && st.restore {
                give_weapon_back(st, cx);
                body_collision(st, cx, true);
                world_slow(st, cx, false);
            }
            if cx.pawn.anim.ended {
                // vt+0x16E8 (StopMovement), vt+0x1628 (not read), Finish.
                cx.pawn.cmc.stop_movement();
                cx.pawn.queue.do_chain();
                return false;
            }
        }
    }
    true
}

/// Every notify event of a pose tick: M_6483's `TresAnimNotifyState_BpEvent` (the state-event camera blueprint's start /
/// end events) goes to the hosts as `AnimBpEvent`, as Diving Barrage's. The window is the generic path's.
pub(crate) fn on_notify(_sf: &mut ShootFlow, _st: &mut State, cx: &mut Ctx, event: &NotifyEvent) {
    if event.class() != NotifyClass::TresAnimNotifyStateBpEvent {
        return;
    }
    let param = match event.def().data {
        NotifyData::Param { param } => param.unwrap_or(0),
        _ => 0,
    };
    let begin = match event.phase {
        NotifyPhase::Begin => true,
        NotifyPhase::End => false,
        _ => return,
    };
    cx.pawn.event(EventKind::AnimBpEvent { seq: event.sequence_name(), param, begin });
}

/// Before the common End's release of the aim mode: End 0x1413ABA70 shows his mesh (vt+0xB58 -> SetVisibility(1, 1)),
/// turns the attack camera and the world slow off (0x14140DF20(ctrl, 0), 0x14140E500(ctrl, 0)) and gives the weapon back
/// (+0xB0 != 0). PORT ADDITION for a state dropped early: the body collision back on (as avatar.rs).
pub(crate) fn end(_sf: &mut ShootFlow, st: &mut State, cx: &mut Ctx) {
    cx.pawn.magic.shot_body_hidden = false;
    crate::combat::camera::off(cx);
    if st.slowed {
        world_slow(st, cx, false);
    }
    if st.body_off {
        body_collision(st, cx, true);
    }
    give_weapon_back(st, cx);
}

/// The Shot handler's spawn point (none: M_6481..3 have no TresShot).
pub(crate) fn muzzle(_sf: &ShootFlow, _st: &State, _cx: &Ctx, _index: usize) -> Option<Vec3> {
    None
}

/// The shape of a hit window on the drill weapon a rush shotlock put on (combat/hit.rs `shape`): group
/// `rush.window.group` (StickL) opened by M_6482 (`sequence`, an index into `SEQUENCES`): the CAPSULE2 between the
/// weapon's ATK_00 and ATK_01 (`rush.k<kind>.shape.track`, actor space per frame of M_6482) turned by his yaw onto his
/// location, radius `rush.k<kind>.shape.radius`. `None` for any other group / clip or without the weapon form.
pub(crate) fn hit_shape(cx: &Ctx, group: &str, sequence: usize) -> Option<(Vec3, Vec3, f32)> {
    let kind = kind_of_form(cx.pawn.magic.shot_weapon_form)?;
    let seq = SEQUENCES.get(sequence)?;
    if group != S.text("rush.window.group") || seq.name != S.text("rush.clip.dash.sequence") {
        return None;
    }
    let track = S.list(&format!("rush.k{kind}.shape.track"));
    let last = (track.len() / 6).checked_sub(1)?;
    let playing = cx.pawn.anim.name.data().samples.iter().any(|&(_, s)| s == sequence);
    let time = if playing { cx.pawn.anim.time } else { seq.length };
    let f = if seq.length > 0.0 && last > 0 { (time / seq.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
    let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
    let j = (i + 1).min(last);
    let at = |k: usize| track[i * 6 + k] + (track[j * 6 + k] - track[i * 6 + k]) * t;
    let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
    let world = |x: f32, y: f32, z: f32| cx.pawn.cmc.location + Vec3::new(x * cos - y * sin, x * sin + y * cos, z);
    Some((world(at(0), at(1), at(2)), world(at(3), at(4), at(5)), S.num(&format!("rush.k{kind}.shape.radius"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_kh3_s() {
        assert_eq!(S.text("rush.update"), "0x1413BC550");
        assert_eq!((super::super::family(9), super::super::family(26)), ("rush", "rush"));
        assert!(super::super::ready(9) && super::super::ready(26));
        // Favorite Deputy: base 8 (Ghost Horde), MAX 9; Classic Tone: 25 / 26.
        assert_eq!((super::super::base_kind(4), super::super::max_kind(4, false)), (8, 9));
        assert_eq!(super::super::max_kind(12, false), 26);
        assert_eq!((weapon_form(9), weapon_form(26)), (6, 21));
        assert_eq!((kind_of_form(6), kind_of_form(21), kind_of_form(0), kind_of_form(2)), (Some(9), Some(26), None, None));
        assert!(weapon_units(9).iter().any(|u| u.ends_with("w_so053_Wep")) && weapon_units(26).iter().any(|u| u.ends_with("w_so113_Wep")));
        assert_eq!((clip("start"), clip("dash"), clip("end")), (AnimId::A6481, AnimId::A6482, AnimId::A6483));
        assert_eq!((S.num("rush.dash.time"), S.num("rush.warp.reach"), S.num("rush.warp.turn"), S.num("rush.warp.down"), S.num("rush.lift.height")), (0.3, 800.0, 120.0, -100.0, 200.0));
        assert!((S.num("rush.dash.time") * S.num("rush.dash.rate") - 1.0).abs() < 1.0e-6);
        assert_eq!((S.text("rush.window.group"), S.text("rush.k9.row"), S.text("rush.k26.row")), ("StickL", "SF_WARPRUSH", "SF_TRWARPRUSH"));
        assert_eq!((S.int("rush.k9.effect_group"), S.int("rush.k26.effect_group")), (0x686, 0x76C));
        assert_eq!((S.text("rush.k9.effect"), S.text("rush.k26.effect")), ("ead_so053_stf0", "ead_so113_stf0"));
        for k in [9, 26] {
            assert_eq!(S.num(&format!("rush.k{k}.shape.radius")), 120.0);
            assert!(S.list(&format!("rush.k{k}.shape.track")).len() >= 12);
        }
        assert!(crate::combat::tables::attack_row("SF_TRWARPRUSH").is_some() && crate::combat::tables::attack_row("SF_WARPRUSH").is_some());
    }
}
