//! The host seam and the state-machine host (round 4): what a host can rely on. No gameplay rule
//! is tested here, because none exists yet: sounds, effects and hits are pushed through the seam
//! with `Sora::debug_emit`.

use kh_core::host::{Call, EffectSpawn, EventKind, HitRecord, RecordingHost, SoundPlay};
use kh_core::test_world::TestWorld;
use kh_core::ui_model::UiPopup;
use kh_core::{Attach, DebugEmit, EffectHandle, EffectId, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, Sora, SoundHandle, SoundId, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn spawn() -> Sora {
    Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT)
}

fn forward() -> Input {
    Input { move_stick: Vec2::new(0.0, 1.0), ..Input::default() }
}

fn dummy(id: u32, x: f32, hp: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE), hp, hp_max: hp }
}

fn state_changes(out: &Output) -> Vec<(StateId, StateId)> {
    out.events.iter().filter_map(|e| if let EventKind::StateChange { from, to } = e.kind { Some((from, to)) } else { None }).collect()
}

#[test]
fn state_changes_are_events_of_the_step_they_happen_in() {
    let world = TestWorld::floor();
    let mut sora = spawn();
    let first = sora.step(DT, &Input::default(), &world);
    assert_eq!((first.step, first.events.len()), (1, 0), "standing still: nothing to report");
    // Idle -> Run: the request is queued by the controller and the queue changes state in the
    // same frame's MainProc (movement spec 1.3).
    let out = sora.step(DT, &forward(), &world);
    assert_eq!((out.step, out.state), (2, StateId::Run));
    assert_eq!(state_changes(&out), vec![(StateId::Idle, StateId::Run)]);
    assert!(out.events.iter().all(|e| e.step == 2));
    // The next step has no event: the list is per step.
    assert!(sora.step(DT, &forward(), &world).events.is_empty());
    // Run -> Jump -> (landing) -> Idle, each reported once.
    let mut seen = Vec::new();
    for i in 0..200 {
        let input = Input { jump_pressed: i == 0, jump_held: i < 30, ..Input::default() };
        seen.extend(state_changes(&sora.step(DT, &input, &world)));
    }
    assert_eq!(seen, vec![(StateId::Run, StateId::Jump), (StateId::Jump, StateId::Idle)]);
    assert_eq!(kh_core::states::implemented_states()[..3], [StateId::Idle, StateId::Run, StateId::Jump]);
}

#[test]
fn the_null_host_and_a_recording_host_give_the_same_simulation() {
    let world = TestWorld::floor().with_box(Vec3::new(300.0, -200.0, 0.0), Vec3::new(500.0, 200.0, 30.0));
    let (mut plain, mut hosted) = (spawn(), spawn());
    let mut host = RecordingHost::new();
    // (Hittable but not lockable: a lockable target within 900 cm becomes the soft target and
    // gets a cursor event since the targeting of combat strand D is implemented.)
    // (party I2) The target stands BESIDE the run line (y = 800): since TresPlayerState_Swaying (actions spec
    // II.6, swaying.rs `run_contact`) a living pawn touching his body circle while he runs makes him side-step
    // round it, which the null host (no targets) cannot do; at (400, 0) on the line he bumped it at step 26
    // (Run -> Swaying, RunSwayR) and the two simulations parted.
    let mut beside = dummy(1, 400.0, 1000.0);
    (beside.position.y, beside.center.y) = (800.0, 800.0);
    host.targets.push(TargetInfo { flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE), ..beside });
    for i in 0..400 {
        let input = Input { move_stick: if i < 250 { Vec2::new(0.3, 1.0) } else { Vec2::ZERO }, jump_pressed: i == 90, jump_held: (90..110).contains(&i), ..Input::default() };
        let a = plain.step(DT, &input, &world);
        let b = host.step(&world, |h| hosted.step_hosted(DT, &input, h));
        assert_eq!(a, b, "step {i}");
    }
    // One snapshot per step. Since the general notify dispatch (animation spec 9; notify.rs) the
    // sound / effect notifies of the clips themselves reach the host too (the landing sound here):
    // nothing else does, and no state makes a sound of its own.
    let published: Vec<&Call> = host.calls.iter().filter(|c| matches!(c, Call::Publish(_))).collect();
    assert_eq!(published.len(), 400);
    assert!(published.iter().enumerate().all(|(i, c)| **c == Call::Publish(i as u64 + 1)));
    assert!(host.calls.iter().all(|c| matches!(c, Call::Publish(_) | Call::Play(..) | Call::StopSound(..) | Call::Spawn(..) | Call::StopEffect(..))));
    for call in &host.calls {
        if let Call::Play(_, play) = call {
            println!("notify sound: {} (id {})", play.name, play.id.0);
        }
    }
}

#[test]
fn debug_emissions_reach_the_host_and_the_event_list_in_order() {
    let world = TestWorld::floor();
    let mut sora = spawn();
    let mut host = RecordingHost::new();
    host.targets.push(TargetInfo { flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE), ..dummy(7, 300.0, 100.0) });
    let sound = SoundPlay { at: Attach::World(Vec3::new(1.0, 2.0, 3.0)), volume: 0.5, ..SoundPlay::at_sora(SoundId(11), "test.sound") };
    let effect = EffectSpawn { persistent: true, ..EffectSpawn::at_sora(EffectId(22), "test.effect") };
    let hit = HitRecord { damage: 30.0, position: Vec3::new(300.0, 0.0, 90.0), attack_name: "test.hit", ..HitRecord::empty(TargetId(7)) };
    let popup = UiPopup::new(UiPopup::KIND_DEBUG, "test", 30.0, Some(Vec3::new(300.0, 0.0, 200.0)));
    for emit in [DebugEmit::Sound(sound), DebugEmit::Effect(effect), DebugEmit::Hit(hit), DebugEmit::Popup(popup), DebugEmit::Hit(HitRecord::empty(TargetId(99)))] {
        sora.debug_emit(emit);
    }
    let out = host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h));
    // The same things, in the same order, on both channels; handles count from 1.
    let labels: Vec<&str> = out.events.iter().map(|e| e.kind.label()).collect();
    assert_eq!(labels, ["sound", "effect", "hit", "popup", "hit"]);
    assert_eq!(out.events[0].kind, EventKind::SoundPlay { handle: SoundHandle(1), play: sound });
    assert_eq!(out.events[1].kind, EventKind::EffectSpawn { handle: EffectHandle(1), spawn: effect });
    let EventKind::Hit { record, result } = out.events[2].kind else { panic!() };
    assert_eq!((record, result.accepted, result.damage_dealt, result.hp_after, result.killed), (hit, true, 30.0, 70.0, false));
    let EventKind::Hit { result: missed, .. } = out.events[4].kind else { panic!() };
    assert!(!missed.accepted, "no such target: the hit is reported as not accepted");
    assert_eq!(host.calls.len(), 5, "sound, effect, two hits, one snapshot (a popup is part of the snapshot)");
    assert_eq!(host.calls[0], Call::Play(SoundHandle(1), sound));
    assert_eq!(host.calls[1], Call::Spawn(EffectHandle(1), effect));
    assert_eq!(host.hits().len(), 2);
    assert_eq!(host.targets[0].hp, 70.0);
    assert_eq!(host.ui.as_ref().unwrap().popups, vec![popup]);
    // Stops, in the next step; the popup list is per step.
    sora.debug_emit(DebugEmit::StopSound(SoundHandle(1), 0.25));
    sora.debug_emit(DebugEmit::StopEffect(EffectHandle(1), true));
    let out = host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h));
    assert_eq!(out.events.iter().map(|e| e.kind.label()).collect::<Vec<_>>(), ["sound-stop", "effect-stop"]);
    assert!(host.ui.as_ref().unwrap().popups.is_empty());
    assert_eq!(host.calls[5..], [Call::StopSound(SoundHandle(1), 0.25), Call::StopEffect(EffectHandle(1), true), Call::Publish(2)]);
    // A kill: hp never goes below 0 and the target stops being alive / hittable in this fixture.
    sora.debug_emit(DebugEmit::Hit(HitRecord { damage: 500.0, ..hit }));
    sora.debug_emit(DebugEmit::Hit(hit));
    let out = host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h));
    let results: Vec<_> = out.events.iter().filter_map(|e| if let EventKind::Hit { result, .. } = e.kind { Some((result.accepted, result.damage_dealt, result.killed)) } else { None }).collect();
    assert_eq!(results, vec![(true, 70.0, true), (false, 0.0, false)]);
}

#[test]
fn the_snapshot_carries_the_status_and_the_status_is_part_of_the_save_state() {
    let world = TestWorld::floor();
    // A loadout built in code: level 99, no victory bonus, nothing equipped. The numbers are
    // computed by the status rules (docs/sora-stats-spec.md): base HP 90, MP 100, focus 100.
    let loadout = Loadout { level: 99, ..Loadout::new("test") };
    let mut sora = spawn();
    assert!(sora.status().placeholder, "without a loadout the status is the marked empty placeholder");
    assert_eq!(sora.ui_snapshot().gauges.hp_max, 0);
    sora = sora.with_status(Status::from_loadout(&loadout));
    sora.status_mut().hp = 77;
    let mut host = RecordingHost::new();
    host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h));
    let ui = host.ui.clone().unwrap();
    assert_eq!((ui.step, ui.gauges.level, ui.gauges.hp, ui.gauges.hp_max, ui.gauges.mp, ui.gauges.focus_max), (1, 99, 77, 90, 100, 100));
    assert!(!ui.command_menu.visible && ui.command_menu.entries.is_empty() && ui.lock_on == TargetId::NONE);
    // A clone is a save state: both go on identically, status included; a teleport keeps the status.
    let mut copy = sora.clone();
    for _ in 0..30 {
        assert_eq!(sora.step(DT, &forward(), &world), copy.step(DT, &forward(), &world));
    }
    assert_eq!(sora.status(), copy.status());
    sora.teleport(Pose { position: Vec3::new(500.0, 0.0, 0.2), yaw: 90.0 });
    assert_eq!((sora.status().level, sora.status().hp), (99, 77));
    assert_eq!(sora.step(DT, &Input::default(), &world).step, 32, "the step counter goes on across a teleport");
}

#[test]
fn the_pad_reaches_the_core_and_jump_is_the_right_face_button() {
    let world = TestWorld::floor();
    let (mut by_flags, mut by_pad) = (spawn(), spawn());
    let mut before = PadButtons::NONE;
    for i in 0..120 {
        let held = (10..40).contains(&i);
        let mut now = PadButtons::NONE;
        now.set(PadButton::FaceRight, held);
        // Buttons without an action in this situation do nothing, whatever is pressed (the
        // attack button, FaceBottom, has one since the combat spec is implemented: it is not
        // pressed here; RB taps with no target in range do nothing).
        now.set(PadButton::FaceLeft, i % 7 == 0);
        now.set(PadButton::RightShoulder, i % 5 == 0);
        let pad = PadState::from_held(now, before);
        before = now;
        let a = by_flags.step(DT, &Input { jump_pressed: i == 10, jump_held: held, ..Input::default() }, &world);
        let b = by_pad.step(DT, &Input::default().with_pad(pad), &world);
        assert_eq!(a, b, "step {i}");
    }
    assert_eq!(PadButton::JUMP, PadButton::FaceRight);
    assert_eq!(PadButton::ATTACK, PadButton::FaceBottom);
}
