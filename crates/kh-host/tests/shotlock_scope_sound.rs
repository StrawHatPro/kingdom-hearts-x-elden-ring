//! (THE USER 2026-11-02: "shotlock's startup sound effect continues to play its audio even after it's cancelled, make it
//! stop if shotlock gets cancelled") THE SHOTLOCK AIM'S SOUND (static SE 18, se00000_039, 7.07 s; KH3's
//! `m_FocusModeAudio`) through the real HUD: it starts with the scope and gets ONE stop (fade 0 = the asset's own 0.6 s)
//! however the aim ends: a cancel (the core's aim mode back to 0: R1, a hit, death, the slow option, a warp: kh-core
//! tests/shotlock_cancel.rs), the shot (mode 2), the airstep (mode 3), or a frame the HUD gives to something else (the
//! game over, a popup, a menu). Needs the UI pack.

use kh_core::{PadButtons, Sora, TargetId, UiSnapshot, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_core::SoundHandle;
use kh_host::seam::AudioCommand;

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn load() -> Option<(Hud, Sora)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok((hud, _)) = Hud::load(&root.join("build/sora_pack/ui")) else {
        eprintln!("SKIPPED: no UI pack");
        return None;
    };
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    Some((hud, sora))
}

/// One HUD frame with the aim in `mode`: its sound commands.
fn frame(hud: &mut Hud, sora: &Sora, mode: u8) -> Vec<(AudioCommand, String)> {
    let mut snapshot: UiSnapshot = sora.ui_snapshot();
    (snapshot.aim.mode, snapshot.aim.aim_time) = (mode, if mode == 0 { 0.0 } else { 6.0 });
    hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    hud.take_sounds()
}

/// The handle of the aim's SE 18 play among these commands.
fn scope_play(sounds: &[(AudioCommand, String)]) -> Option<SoundHandle> {
    sounds.iter().find_map(|(c, line)| match c {
        AudioCommand::Play(h, _) if line.starts_with("static SE 18 ") => Some(*h),
        _ => None,
    })
}

fn stops(sounds: &[(AudioCommand, String)]) -> Vec<(SoundHandle, f32)> {
    sounds.iter().filter_map(|(c, _)| if let AudioCommand::Stop(h, fade) = c { Some((*h, *fade)) } else { None }).collect()
}

/// Opens the aim; returns its sound's handle after a few aiming frames (no stop meanwhile).
fn open(hud: &mut Hud, sora: &Sora) -> SoundHandle {
    let first = frame(hud, sora, 1);
    let handle = scope_play(&first).expect("static SE 18 with the scope");
    for _ in 0..30 {
        let s = frame(hud, sora, 1);
        assert!(stops(&s).is_empty() && scope_play(&s).is_none(), "aiming: one play, no stop");
    }
    assert!(hud.scope_sound_live());
    handle
}

#[test]
fn every_way_out_of_the_aim_stops_its_sound_once() {
    let Some((mut hud, sora)) = load() else { return };
    for _ in 0..5 {
        frame(&mut hud, &sora, 0);
    }
    // a cancel (mode 0), the shot (2), the airstep (3)
    for (mode, route) in [(0u8, "cancel"), (2, "the shot"), (3, "the airstep")] {
        let handle = open(&mut hud, &sora);
        let s = frame(&mut hud, &sora, mode);
        assert_eq!(stops(&s), vec![(handle, 0.0)], "{route}: one Stop with fade 0 (the asset's own 0.6 s)");
        assert!(!hud.scope_sound_live());
        // nothing more on later frames
        for _ in 0..10 {
            assert!(stops(&frame(&mut hud, &sora, 0)).iter().all(|(h, _)| *h != handle), "{route}: stopped once");
        }
    }
}

#[test]
fn a_frame_taken_by_the_game_over_stops_the_aims_sound() {
    let Some((mut hud, sora)) = load() else { return };
    for _ in 0..5 {
        frame(&mut hud, &sora, 0);
    }
    let handle = open(&mut hud, &sora);
    assert!(hud.open_gameover(kh_host::gameover::GameOverSetup::default()));
    // the aim is kept open in the snapshot (as if the core stood still): the cockpit goes on under the rising plate, and
    // the moment the void scene takes the HUD's frame the aim's sound stops (a real death closes the aim at once:
    // kh-core tests/shotlock_cancel.rs `death_cancels`)
    let mut stopped_at = None;
    for n in 0..240 {
        let s = frame(&mut hud, &sora, 1);
        if stops(&s).contains(&(handle, 0.0)) {
            stopped_at = Some(n);
            break;
        }
    }
    assert!(stopped_at.is_some_and(|n| n <= 90), "the game over's void took the frame: stopped at {stopped_at:?}");
    assert!(!hud.scope_sound_live());
}
