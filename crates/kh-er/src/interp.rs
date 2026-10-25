//! (INT) THE WHOLE KH3 SIDE OF THE FRAME IS DRAWN BETWEEN TWO FIXED STEPS: frame-rate independence (the user's standing
//! rule: simulation on a fixed step, the picture interpolated between the last two steps).
//!
//! Sora's KH3 simulation (his core, the camera, the friends, the effects, the team props, the enemies we carry in a KH3
//! reaction) is stepped at a fixed 1/60 s by sora.rs's accumulator. Drawn at the latest step, every drawn place moves 0, 1
//! or 2 steps in a frame of Elden Ring's uneven pacing (16 - 33 ms, hitches): everything of ours judders together (SM's
//! measurement: ~14 000 - 18 000 m/s3 of drawn jerk on the friends, ~1/50 of that interpolated). So EVERYTHING of ours
//! is drawn at one `alpha` = how far the frame is between the two latest steps (the test app's `Sim::alpha`):
//! - Sora's pose and transform, the Keyblade and the form weapons, his clip times (sora.rs `submit`);
//! - the camera (cam.rs `post`; a cut is never blended: `camera_cut`);
//! - the friends (partydraw.rs, `PosedMember::posed`), the hero light's carrier, the team props (`Props`);
//! - the effects (fx.rs `draw_at`): emitter nodes, particles (born ones with their emitter), trails' heads, decals, the
//!   effects' point lights (kh-vfx);
//! - the enemies in a KH3 reaction (puppet.rs `reassert_at`);
//! - the HUD's world marks and the bubbles' tails: projected through the view drawn in THIS frame (`drawn_camera`).
//!
//! NOTHING THAT IS SIMULATED READS A DRAWN VALUE: the drawn state depends on the frame's timing, the steps never do.
//! A DISCONTINUITY (teleport, reseat, warp, load: `jumped`) is never blended: everything is drawn at the latest step
//! until the next one. `r.interp = 0` (the off key) draws the latest step as before.

use glam::{Quat, Vec3};
use kh_anim::Trs;

use crate::control::Control;

/// The fixed step (sora.rs `STEP`).
pub const STEP: f32 = 1.0 / 60.0;

/// A move of more than this from one step to the next (cm) is a discontinuity, not motion (fx.rs cuts the trails at the
/// same distance). Sora's fastest moves (XS dashes, flowmotion) stay far below 180 m/s.
pub const SNAP_CM: f32 = 300.0;

/// A camera that moves more than this in a step (cm) or turns more than `CAMERA_CUT_DEG` is a CUT (a conversation shot,
/// a team attack's camera, the boss finish): drawn at its new view at once.
pub const CAMERA_CUT_CM: f32 = 150.0;
pub const CAMERA_CUT_DEG: f32 = 30.0;

/// `r.interp` (default ON; `r.interp = 0`: the latest step, as before).
pub fn on(control: &Control) -> bool {
    control.flag("r.interp") != Some(false)
}

/// How far the frame is between the two latest steps (1 = the latest step).
pub fn alpha_of(on: bool, accumulator: f32) -> f32 {
    if on {
        (accumulator / STEP).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// sora.rs's alpha of this frame.
pub fn alpha(control: &Control, accumulator: f32) -> f32 {
    alpha_of(on(control), accumulator)
}

/// A discontinuity between two steps' places (cm).
pub fn jumped(before: Vec3, now: Vec3) -> bool {
    !before.is_finite() || !now.is_finite() || before.distance(now) > SNAP_CM
}

/// A turn of more than this in one step (degrees) is a discontinuity too: KH3 sets his facing at once in some moves (a
/// combo's turn to a target behind him: 180 degrees in a step); blended, his mesh would sweep through the half-turn while the
/// effects on him cut across it. His own turning is far slower (at most ~20 degrees a step).
pub const SNAP_DEG: f32 = 90.0;

/// His discontinuity between two steps: a jump of his place or of his facing.
pub fn discontinuous(before: &kh_core::Output, now: &kh_core::Output) -> bool {
    jumped(before.position, now.position) || ((now.yaw - before.yaw + 540.0).rem_euclid(360.0) - 180.0).abs() > SNAP_DEG
}

pub fn lerp_trs(a: &Trs, b: &Trs, alpha: f32) -> Trs {
    Trs { translation: a.translation.lerp(b.translation, alpha), rotation: a.rotation.slerp(b.rotation, alpha), scale: a.scale.lerp(b.scale, alpha) }
}

/// Sora's mesh component in the KH3 world (sora.rs `component_to_world`).
pub fn component_to_world(position: Vec3, yaw: f32) -> Trs {
    Trs { translation: position, rotation: Quat::from_rotation_z((yaw - 90.0).to_radians()), scale: Vec3::ONE }
}

/// SORA AS DRAWN: his component transform, his pose and the keychain's chain between the two latest steps (sora.rs
/// `submit`; the same rule the harness measures). A pose of another length (a new skeleton) or `alpha` 1: the latest.
pub fn sora_drawn(before: &kh_core::Output, now: &kh_core::Output, pose_before: &kh_anim::Pose, pose: &kh_anim::Pose, chain_before: &[Trs], chain: &[Trs], alpha: f32) -> (kh_anim::Pose, Trs, Vec<Trs>) {
    if alpha < 1.0 && pose_before.component.len() == pose.component.len() && !discontinuous(before, now) {
        let to_world = lerp_trs(&component_to_world(before.position, before.yaw), &component_to_world(now.position, now.yaw), alpha);
        let chain = if chain_before.len() == chain.len() { kh_anim::Pose { local: Vec::new(), component: chain_before.to_vec() }.lerp(&kh_anim::Pose { local: Vec::new(), component: chain.to_vec() }, alpha).component } else { chain.to_vec() };
        (pose_before.lerp(pose, alpha), to_world, chain)
    } else {
        (pose.clone(), component_to_world(now.position, now.yaw), chain.to_vec())
    }
}

/// His facing as drawn (degrees; a discontinuity: the latest).
pub fn yaw_drawn(before: &kh_core::Output, now: &kh_core::Output, alpha: f32) -> f32 {
    if alpha >= 1.0 || discontinuous(before, now) {
        now.yaw
    } else {
        before.yaw + ((now.yaw - before.yaw + 540.0).rem_euclid(360.0) - 180.0) * alpha
    }
}

/// A clip's time as drawn: between the two steps' times while the same clip plays forward (a new clip or a loop's
/// wrap: the latest time).
pub fn clip_time(same_clip: bool, before: f32, now: f32, alpha: f32) -> f32 {
    if same_clip && now >= before && now - before < 0.5 {
        before + (now - before) * alpha
    } else {
        now
    }
}

/// A friend's posed figure between two steps' (unlock/host.rs `StageDraw::at`: the staged friends): the same blend as
/// `PosedMember::posed` (bones, place, weapon); a jump of his place: the latest.
#[cfg(feature = "party")]
pub fn posed_between(before: &kh_partyhost::PosedChar, now: &kh_partyhost::PosedChar, alpha: f32) -> kh_partyhost::PosedChar {
    if alpha >= 1.0 || before.component.len() != now.component.len() || jumped(before.component_to_world.translation, now.component_to_world.translation) {
        return now.clone();
    }
    let pose = kh_anim::Pose { local: before.local.clone(), component: before.component.clone() }.lerp(&kh_anim::Pose { local: now.local.clone(), component: now.component.clone() }, alpha);
    let weapon = match (&before.weapon, &now.weapon) {
        (Some(b), Some(n)) if b.0 == n.0 => Some((n.0.clone(), lerp_trs(&b.1, &n.1, alpha), n.2)),
        _ => now.weapon.clone(),
    };
    kh_partyhost::PosedChar { component_to_world: lerp_trs(&before.component_to_world, &now.component_to_world, alpha), local: pose.local, component: pose.component, weapon }
}

/// The camera cut between two steps' views.
pub fn camera_cut(before: &kh_camera::CameraView, now: &kh_camera::CameraView) -> bool {
    let turn = |a: f32, b: f32| ((b - a + 540.0).rem_euclid(360.0) - 180.0).abs();
    let r = (&before.rotation, &now.rotation);
    !before.position.is_finite() || before.position.distance(now.position) > CAMERA_CUT_CM || turn(r.0.yaw, r.1.yaw) > CAMERA_CUT_DEG || turn(r.0.pitch, r.1.pitch) > CAMERA_CUT_DEG
}

/// THE VIEW DRAWN IN THIS FRAME for a projection made in the frame task (the HUD's world marks, the bubbles' tails): ours
/// as posted this frame (cam.rs `post`, written over the game's camera in its camera step, which runs AFTER the frame
/// task: `game::camera()` there is still the previous frame's), else the game's camera.
pub fn drawn_camera() -> Option<crate::game::Camera> {
    let game = crate::game::camera();
    match (crate::cam::rendered_view(), game) {
        (Some((v, _)), Some(g)) => Some(crate::game::Camera { right: v.right, up: v.up, forward: v.forward, position: v.position, fov: v.fov, ..g }),
        (_, g) => g,
    }
}

/// THE TEAM PROPS BETWEEN THE STEPS (partydraw.rs): each prop's transform at the two latest steps, by (model, index).
#[derive(Default)]
pub struct Props {
    /// (model, index) -> (the step before, the latest step, the step it was last seen in).
    map: std::collections::HashMap<(&'static str, usize), (Trs, Trs, u64)>,
    step: u64,
}

impl Props {
    /// After every step: the props of the step (`party::team::props`).
    pub fn record(&mut self, props: impl Iterator<Item = (&'static str, usize, Trs)>) {
        self.step += 1;
        let step = self.step;
        for (model, index, t) in props {
            let e = self.map.entry((model, index)).or_insert((t, t, step));
            // (a prop not seen at the step before is new there: no blend from an older place)
            let before = if e.2 + 1 == step && !jumped(e.1.translation, t.translation) { e.1 } else { t };
            *e = (before, t, step);
        }
        self.map.retain(|_, e| e.2 == step);
    }

    /// A prop as drawn (`now` = its latest transform).
    pub fn at(&self, model: &'static str, index: usize, now: Trs, alpha: f32) -> Trs {
        match self.map.get(&(model, index)) {
            Some((before, latest, _)) if alpha < 1.0 && latest.translation.distance(now.translation) < 1.0e-3 => lerp_trs(before, latest, alpha),
            _ => now,
        }
    }
}

pub static PROPS: std::sync::Mutex<Option<Props>> = std::sync::Mutex::new(None);

/// sora.rs, after every party step: the team props of the step.
pub fn record_props(props: impl Iterator<Item = (&'static str, usize, Trs)>) {
    if let Ok(mut g) = PROPS.try_lock() {
        g.get_or_insert_with(Props::default).record(props);
    }
}

/// partydraw.rs: a prop as drawn.
pub fn prop_at(model: &'static str, index: usize, now: Trs, alpha: f32) -> Trs {
    PROPS.try_lock().ok().and_then(|g| g.as_ref().map(|p| p.at(model, index, now, alpha))).unwrap_or(now)
}

/// (INT) sora.rs's own part of the drawn history: the chandelier pawn's clip time at the two latest steps, the snaps.
#[derive(Default)]
pub struct Track {
    chandelier: (Option<f32>, Option<f32>),
    pub snaps: u32,
    pub told: Option<bool>,
}

impl Track {
    /// After every step of his.
    pub fn step(&mut self, chandelier_time: Option<f32>) {
        self.chandelier = (self.chandelier.1, chandelier_time);
    }

    /// A discontinuity: the latest step is the step before as well.
    pub fn snap(&mut self) {
        self.chandelier.0 = self.chandelier.1;
        self.snaps += 1;
    }

    /// The chandelier pawn's clip time as drawn (`now` = its latest).
    pub fn chandelier_time(&self, now: f32, alpha: f32) -> f32 {
        match self.chandelier {
            (Some(before), Some(latest)) if (latest - now).abs() < 1.0e-4 => clip_time(true, before, latest, alpha),
            _ => now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_alpha_is_the_frame_between_the_steps() {
        assert_eq!(alpha_of(false, 0.008), 1.0);
        assert!((alpha_of(true, STEP * 0.25) - 0.25).abs() < 1e-6);
        assert_eq!(alpha_of(true, 1.0), 1.0);
        assert_eq!(alpha_of(true, -1.0), 0.0);
    }

    #[test]
    fn int_clip_time_blends_only_forward_in_one_clip() {
        assert!((clip_time(true, 1.0, 1.1, 0.5) - 1.05).abs() < 1e-6);
        assert_eq!(clip_time(false, 1.0, 1.1, 0.5), 1.1);
        assert_eq!(clip_time(true, 1.0, 0.1, 0.5), 0.1); // a loop's wrap
    }

    #[test]
    fn int_props_blend_and_never_across_a_new_prop() {
        let t = |x: f32| Trs { translation: Vec3::new(x, 0.0, 0.0), ..Trs::IDENTITY };
        let mut p = Props::default();
        p.record([("rock", 0, t(0.0))].into_iter());
        assert_eq!(p.at("rock", 0, t(0.0), 0.5).translation.x, 0.0);
        p.record([("rock", 0, t(10.0))].into_iter());
        assert!((p.at("rock", 0, t(10.0), 0.5).translation.x - 5.0).abs() < 1e-4);
        // a jump (a new throw far away) is not blended
        p.record([("rock", 0, t(1000.0))].into_iter());
        assert_eq!(p.at("rock", 0, t(1000.0), 0.5).translation.x, 1000.0);
        // gone for a step, back: no blend from its old place
        p.record(std::iter::empty());
        p.record([("rock", 0, t(1010.0))].into_iter());
        assert_eq!(p.at("rock", 0, t(1010.0), 0.5).translation.x, 1010.0);
    }
}
