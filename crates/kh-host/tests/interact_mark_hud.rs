//! (INTERACT-MARK, the user 2026-11-05: "Add this green effect from KH3 onto what the interact prompt is focused on (people,
//! etc)") THE HUD SIDE: while a host reaction is offered, KH3's type-5 target cursor (`ActLockonMark`, the green triangle) is
//! on the point the host gives (`Hud::set_host_reaction_target`); a new target replays its `in`; it goes at once with the
//! reaction or the target. Real Hud + UI pack (skipped without it).

use glam::Mat4;
use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, HostReaction, Hud, HudTargets, TargetView, View};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

const CURSOR: &str = "planeParent.lockonMarkPlane.cursor5_0";

fn step(hud: &mut Hud, sora: &Sora, snapshot: &kh_core::UiSnapshot, n: usize) {
    // a camera at the origin looking down -Z (the kh-ui projection rule's test view)
    let view = View { clip_from_world: Mat4::perspective_infinite_reverse_rh(60f32.to_radians(), 16.0 / 9.0, 0.1), size: (1920.0, 1080.0) };
    for _ in 0..n {
        hud.frame(&mut Frame { snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: Some(View { ..view }), words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

#[test]
fn interact_mark_follows_the_first_host_reaction() {
    let Ok((mut hud, _fonts)) = Hud::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
    let snap = sora.ui_snapshot();
    step(&mut hud, &sora, &snap, 60);
    assert!(!hud.reaction_mark_shown());
    // a target with no reaction offered: nothing
    hud.set_host_reaction_target(Some((7, Vec3::new(0.0, 0.0, -5.0))));
    step(&mut hud, &sora, &snap, 5);
    assert!(!hud.reaction_mark_shown(), "no reaction, no marker");
    // an NPC's Talk: the marker on him, at his point's screen place (the view's middle)
    hud.set_host_reactions(&[HostReaction::kind(Hud::TALK_COMMAND)]);
    step(&mut hud, &sora, &snap, 30);
    assert!(hud.reaction_mark_shown());
    let m = hud.movie("HudTarget").unwrap();
    let node = m.node(CURSOR).expect("the action cursor");
    assert_eq!((node.matrix.tx, node.matrix.ty, node.is_visible()), (960.0, 540.0, true));
    assert_eq!(m.current_label(CURSOR), Some("in"), "half a second in: still its `in` (frames 7 .. 54)");
    step(&mut hud, &sora, &snap, 60);
    assert_eq!(hud.movie("HudTarget").unwrap().current_label(CURSOR), Some("loop"));
    // the target moves: it follows
    hud.set_host_reaction_target(Some((7, Vec3::new(1.0, 0.0, -5.0))));
    step(&mut hud, &sora, &snap, 1);
    let x = hud.movie("HudTarget").unwrap().node(CURSOR).unwrap().matrix.tx;
    assert!(x > 1000.0, "{x}");
    // another target: removed and added again (its `in` again)
    hud.set_host_reaction_target(Some((8, Vec3::new(0.0, 0.0, -6.0))));
    step(&mut hud, &sora, &snap, 2);
    assert_eq!(hud.movie("HudTarget").unwrap().current_label(CURSOR), Some("in"));
    // behind the camera: hidden, not moved to an edge
    hud.set_host_reaction_target(Some((8, Vec3::new(0.0, 0.0, 6.0))));
    step(&mut hud, &sora, &snap, 1);
    assert!(!hud.movie("HudTarget").unwrap().node(CURSOR).unwrap().is_visible());
    // the reaction goes: the marker goes at once (no out animation)
    hud.set_host_reactions(&[]);
    step(&mut hud, &sora, &snap, 1);
    assert!(!hud.reaction_mark_shown() && !hud.movie("HudTarget").unwrap().exists(CURSOR));
    // the target goes while the reaction stays: gone too
    hud.set_host_reactions(&[HostReaction::named("Pick Up")]);
    hud.set_host_reaction_target(Some((9, Vec3::new(0.0, -1.0, -4.0))));
    step(&mut hud, &sora, &snap, 3);
    assert!(hud.reaction_mark_shown());
    hud.set_host_reaction_target(None);
    step(&mut hud, &sora, &snap, 1);
    assert!(!hud.reaction_mark_shown());
}
