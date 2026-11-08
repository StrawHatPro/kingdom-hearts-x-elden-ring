//! (F-HUD, ROUND-2) GHOST HORDE'S CAMERA THROUGH THE HOSTS' SELECTION: the real session (kh-core's shotlock bench:
//! Favorite Deputy, two training dummies, the aim through the pad, fire) stepped with `kh_camera::forms::select` and
//! `FormCameras` exactly as kh-er cam.rs does each step. KH3's attack camera (mode 2, 0x1414122A0 case 2) must have the
//! view from the first avatar to sub 5 (the rush) and only then; while it has it the port's own shot camera steps aside
//! (`attack_own`) and the log calls it "attack"; it orbits each avatar's target at 600 cm, the pitch within -45 .. 0.

#[path = "../../kh-core/tests/shotlocks_common/mod.rs"]
mod shotlocks_common;

use glam::{Vec2, Vec3};
use kh_camera::forms::{mode_label, select, FormCameras, FormShot};
use kh_camera::{CameraTarget, FieldCamera, TargetState, CAMERA_PARAMS};
use kh_core::{AnimId, TargetId};
use shotlocks_common::{dummy, level99_with, Bench};

#[test]
fn ghost_horde_gives_the_view_to_kh3_s_attack_camera_from_the_first_avatar_to_the_rush() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_03"), &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]);
    assert_eq!(b.aim_until(6, 300), 8, "Ghost Horde");
    b.fire();
    let mut cams = FormCameras::default();
    let mut field: Option<FieldCamera> = None;
    let (mut first_avatar, mut rush, mut on_from, mut on_to) = (None, None, None, None);
    let mut orbit_ok = true;
    let (mut current, mut targets_seen, mut checked) = (None, 0, 0);
    let mut labels = Vec::new();
    for k in 0..900 {
        let o = b.step(&[]);
        let target = CameraTarget { position: o.position, yaw: o.yaw, velocity: Vec3::ZERO, state: TargetState::Other, collision_half_height: 89.0 };
        let under = field.get_or_insert_with(|| FieldCamera::new(CAMERA_PARAMS, &target)).view();
        let targets = b.host.targets.clone();
        let point_of = |id: TargetId| targets.iter().find(|t| t.id == id).map(|t| t.center);
        let selection = select(&b.sora, false, &point_of);
        let shown = cams.step(1.0 / 60.0, selection.shot, &target, &under, Vec2::ZERO, &b.world);
        if first_avatar.is_none() && !b.sora.magic().shot_avatars.is_empty() {
            first_avatar = Some(k);
        }
        if rush.is_none() && o.anim.name == AnimId::M1456 {
            rush = Some(k);
        }
        if selection.attack_own {
            on_from = on_from.or(Some(k));
            on_to = Some(k);
            let Some(FormShot::Attack(a)) = selection.shot else { panic!("the attack camera's shot") };
            assert_eq!(a.mode, 2);
            let v = shown.expect("KH3's camera has the view");
            // each avatar's target a camera of its own; once blended in (0.1 s): 600 cm from its point, pitch -45 .. 0
            if current.map(|c: (TargetId, usize)| c.0) != Some(a.target) {
                current = Some((a.target, k));
                targets_seen += 1;
            }
            if let (Some(p), Some((_, since))) = (a.point, current) {
                if k > since + 8 {
                    checked += 1;
                    orbit_ok &= (v.view.look_point - p).length() < 1.0 && (v.view.distance - 600.0).abs() < 1.0 && (-45.0..=0.0).contains(&v.view.rotation.pitch);
                }
            }
        }
        if let Some(l) = mode_label(&selection, shown.is_some()) {
            if labels.last() != Some(&l) {
                labels.push(l);
            }
        }
        if o.state != kh_core::StateId::Shootflow && rush.is_some() {
            break;
        }
    }
    let (first_avatar, rush) = (first_avatar.expect("avatars"), rush.expect("the rush"));
    assert_eq!(on_from, Some(first_avatar), "KH3's camera from the first avatar's step");
    // sub 5 turns the avatars off after 0.5 s, sub 6 the attack camera (0x14140DF20(ctrl, 0)); M_1456 plays on the next step
    assert_eq!(on_to.map(|k| k + 2), Some(rush), "until sub 6, the step before the rush");
    assert!(orbit_ok && checked > 0, "the orbit on each avatar's target ({checked} steps checked)");
    assert_eq!(targets_seen, 2, "a camera on each of the two locked targets");
    assert_eq!(labels.first(), Some(&"attack"), "the log names it \"attack\": {labels:?}");
}
