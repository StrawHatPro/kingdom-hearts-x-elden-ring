//! (ER62) ONE LISTENER FOR EVERY SOUND: the camera actually on screen.
//!
//! THE USER'S REPORT (2026-10-14): "Directional audio is buggy". WHAT THE LOG SHOWED: under KH3's camera the game's
//! audio listener (`SET_WWISE_LISTENER`, detoured in `cam.rs`) was handed a matrix that was neither our view nor Elden
//! Ring's view of the previous step in 96 of 138 seconds (`listener calls 60 (given our view 0, not the camera's
//! matrix 60)`), and all of them in the seconds the camera stood still. The game passes its OWN camera of the frame,
//! computed before our write; the old test compared it with Elden Ring's view of the previous step, so the detour
//! replaced it only while that camera stood still. Elden Ring's sounds were heard from Elden Ring's own camera (which
//! looks another way: the right stick turns ours, not its) while moving, and from ours while still: left and right
//! jumped. Now (`audio.listener_fix`, default on) every call gets the view on screen (`cam::listener_view`).
//!
//! Sora's own sounds (kh-audio, KH3 space, cm) took their listener from `CSCamera::pers_cam_1` converted with the KH3
//! frame of THIS frame; the view there was made with the frame of the previous post, so a frame with an origin shift
//! put the listener off by the shift. Now they use the posted view on screen with the origin it was made with (and the
//! game's camera when Elden Ring has it). Axes: Elden Ring rows right / up / forward (m) -> KH3 (X forward, Y right,
//! Z up, cm) through `coords` only; both spaces are left-handed, so no mirror. Distances in cm against KH3's own
//! attenuation (full to 400 cm, nothing at 4000 cm: kh-audio `attenuation`).
//!
//! One `audio:` line a second: the listener (where from, KH3 and Elden Ring position, yaw / pitch), how far the old
//! read (`pers_cam_1`) was from it, what the game's listener was last handed, and one live 3D voice with its pan.

use std::sync::Mutex;

use glam::Vec3;
use kh_audio::{ActiveLine, Listener};

use crate::cam::{self, RawView};
use crate::coords::{self, Frame};
use crate::erlog;

/// Where the listener of a frame came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The view of ours on screen (`cam::rendered_view`).
    Screen,
    /// Elden Ring's camera (`pers_cam_1`): Elden Ring has the camera, or `audio.listener_fix = 0`.
    Game,
}

/// What the latest `pick` saw (for the log).
#[derive(Clone, Copy, Debug)]
struct Seen {
    source: Source,
    view: RawView,
    /// The listener the old rule (`pers_cam_1` with this frame's origin) would have given: its distance (cm) and angle
    /// (degrees) from the one used.
    old_off: Option<(f32, f32)>,
}

static SEEN: Mutex<Option<Seen>> = Mutex::new(None);

/// The listener of a camera given in Elden Ring space (rows right / up / forward, m), made when KH3's origin was at
/// `origin` (Elden Ring m): KH3 space, cm.
pub fn of_view(view: &RawView, origin: Vec3) -> Listener {
    Listener {
        position: Frame::at(origin).pos_er_to_kh(view.position),
        forward: coords::unit_er_to_kh(view.forward).normalize_or(Vec3::X),
        right: coords::unit_er_to_kh(view.right).normalize_or(Vec3::Y),
    }
}

fn finite(v: &RawView) -> bool {
    v.right.is_finite() && v.forward.is_finite() && v.position.is_finite() && v.right.length_squared() > 0.25 && v.forward.length_squared() > 0.25
}

/// Distance (cm) and angle (degrees, between the forwards) of two listeners.
fn apart(a: &Listener, b: &Listener) -> (f32, f32) {
    ((a.position - b.position).length(), a.forward.dot(b.forward).clamp(-1.0, 1.0).acos().to_degrees())
}

/// The listener of Sora's sounds this frame: the view on screen (`fix`), else Elden Ring's camera (`frame`: KH3's frame
/// of this frame). `None`: no camera at all (keep the last one).
pub fn pick(frame: &Frame, fix: bool) -> Option<Listener> {
    let game = crate::game::camera().map(|c| RawView { right: c.right, up: c.up, forward: c.forward, position: c.position, fov: c.fov }).filter(finite);
    let old = game.map(|v| of_view(&v, frame.origin));
    let screen = if fix { cam::rendered_view().filter(|(v, _)| finite(v)) } else { None };
    let (listener, seen) = match (screen, game) {
        (Some((view, origin)), _) => {
            let l = of_view(&view, origin);
            (l, Seen { source: Source::Screen, view, old_off: old.map(|o| apart(&o, &l)) })
        }
        (None, Some(view)) => (old?, Seen { source: Source::Game, view, old_off: Some((0.0, 0.0)) }),
        (None, None) => return None,
    };
    if let Ok(mut slot) = SEEN.try_lock() {
        *slot = Some(seen);
    }
    Some(listener)
}

/// KH3 yaw and pitch (degrees) of a KH3 direction.
fn yaw_pitch(d: Vec3) -> (f32, f32) {
    (d.y.atan2(d.x).to_degrees(), d.z.clamp(-1.0, 1.0).asin().to_degrees())
}

fn side(pan: f32) -> &'static str {
    match pan {
        p if p.abs() < 0.1 => "ahead",
        p if p.abs() > 0.9 => "behind",
        p if p > 0.0 => "right",
        _ => "left",
    }
}

/// The once-a-second `audio:` line. `listener`: the one Sora's sounds use; `active`: the engine's live sounds.
pub fn log(listener: &Listener, active: &[ActiveLine], fix: bool) {
    let seen = SEEN.try_lock().ok().and_then(|s| *s);
    let (yaw, pitch) = yaw_pitch(listener.forward);
    let p = listener.position;
    let mut line = format!(
        "audio: listener {} | KH ({:.0}, {:.0}, {:.0}) cm yaw {:.1} pitch {:.1}",
        match seen.map(|s| s.source) {
            Some(Source::Screen) => "= the view on screen",
            Some(Source::Game) => "= Elden Ring's camera",
            None => "= none yet",
        },
        p.x,
        p.y,
        p.z,
        yaw,
        pitch
    );
    if let Some(s) = seen {
        line += &format!(" | ER ({:.2}, {:.2}, {:.2}) m", s.view.position.x, s.view.position.y, s.view.position.z);
        if let Some((cm, deg)) = s.old_off {
            line += &format!(" | the old read (pers_cam_1, this frame's origin) was {cm:.0} cm / {deg:.1} deg off");
        }
        // What the game's own listener was handed last, against the view on screen.
        if s.source == Source::Screen {
            match cam::game_listener() {
                Some(g) => line += &format!(" | game's listener last given {:.2} m / {:.1} deg from it", (g.position - s.view.position).length(), g.forward.dot(s.view.forward).clamp(-1.0, 1.0).acos().to_degrees()),
                None => line += " | game's listener not replaced yet",
            }
        }
    }
    // One live 3D voice (the newest): where it is, its pan as sent, the pan its place has from this listener.
    match active.iter().rev().find(|a| a.position.is_some() && a.voices > 0) {
        Some(a) => {
            let at = a.position.unwrap_or(Vec3::ZERO);
            let expected = kh_audio::pan_of(listener, at);
            line += &format!(
                " | voice {}#{} ({}) at ({:.0}, {:.0}, {:.0}) {:.0} cm: pan {:.2} ({}), from here {:.2} ({}), depth {:.2}, volume {:.2}",
                a.bank,
                a.index,
                if a.cause.is_empty() { "-" } else { &a.cause },
                at.x,
                at.y,
                at.z,
                a.distance.unwrap_or(0.0),
                a.params.pan,
                side(a.params.pan),
                expected,
                side(expected),
                a.params.pan_depth,
                a.params.volume
            );
        }
        None => line += " | no 3D voice playing",
    }
    if !fix {
        line += " | audio.listener_fix = 0: THE OLD RULES";
    }
    erlog!("{line}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cam::{listener_view, raw_of, RawView};
    use kh_camera::Rot;

    /// A KH3 camera at `at` (cm) with this yaw / pitch, as `cam.rs` posts it, through `of_view`.
    fn listener(frame: &Frame, at: Vec3, yaw: f32, pitch: f32) -> (RawView, Listener) {
        let view = raw_of(frame, at, Rot::new(pitch, yaw, 0.0), 1.18);
        (view, of_view(&view, frame.origin))
    }

    fn mono(pan: f32) -> [f32; 2] {
        kh_audio::mixer::gains(&kh_audio::VoiceParams { volume: 1.0, pitch: 1.0, pan, pan_depth: 1.0, lowpass: 1.0, reverb_send: 0.0 }, false)
    }

    #[test]
    fn a_sound_to_the_cameras_right_pans_right() {
        let frame = Frame::at(Vec3::new(1234.0, 56.0, -789.0));
        for yaw in [0.0f32, 37.0, 90.0, 180.0, -120.0] {
            let cam_at = Vec3::new(-300.0, 250.0, 180.0);
            let (_, l) = listener(&frame, cam_at, yaw, -10.0);
            // KH3's right of a yaw, independent of the code under test: yaw turns +X toward +Y.
            let a = yaw.to_radians();
            let (forward, right) = (Vec3::new(a.cos(), a.sin(), 0.0), Vec3::new(-a.sin(), a.cos(), 0.0));
            assert!((l.position - cam_at).length() < 0.1, "{yaw}: {:?}", l.position);
            assert!((l.right - right).length() < 1e-4 && l.forward.dot(forward) > 0.98, "{yaw}: {l:?}");
            let to_right = kh_audio::pan_of(&l, cam_at + forward * 400.0 + right * 300.0);
            let to_left = kh_audio::pan_of(&l, cam_at + forward * 400.0 - right * 300.0);
            assert!(to_right > 0.15 && to_right < 0.5, "{yaw}: {to_right}");
            assert!(to_left < -0.15 && to_left > -0.5, "{yaw}: {to_left}");
            let [left_gain, right_gain] = mono(to_right);
            assert!(right_gain > left_gain * 1.5, "{yaw}: {left_gain} {right_gain}");
            // Sora 4 m in front of the camera (KH3's field camera): ahead, full volume (inside 400 cm).
            assert!(kh_audio::pan_of(&l, cam_at + forward * 400.0).abs() < 0.08);
            assert_eq!(kh_audio::driver::attenuation(400.0).0, 1.0);
        }
    }

    #[test]
    fn an_origin_shift_after_the_post_moves_nothing() {
        // The view was posted with origin A; the game then shifts its physics origin by `shift` (Frame::rebase). The
        // listener taken with the origin of the post is where the camera is in KH3 space; the old read (the view's
        // Elden Ring position with the NEW frame) is off by the whole shift.
        let mut frame = Frame::at(Vec3::new(100.0, 10.0, 200.0));
        let cam_at = Vec3::new(500.0, -200.0, 150.0);
        let (view, _) = listener(&frame, cam_at, 45.0, 0.0);
        let posted_origin = frame.origin;
        let shift = Vec3::new(-64.0, 0.0, 128.0);
        frame.rebase(shift);
        let now = of_view(&view, posted_origin);
        let old = of_view(&view, frame.origin);
        assert!((now.position - cam_at).length() < 0.1);
        assert!((old.position - cam_at).length() > 10_000.0);
    }

    #[test]
    fn the_games_listener_gets_the_view_on_screen_whatever_matrix_it_was_given() {
        let frame = Frame::at(Vec3::ZERO);
        // Elden Ring's own camera of the frame (another place, looking another way) and ours, on screen.
        let given = raw_of(&frame, Vec3::new(0.0, 0.0, 170.0), Rot::new(0.0, 0.0, 0.0), 0.8378);
        let ours = raw_of(&frame, Vec3::new(-400.0, 300.0, 220.0), Rot::new(-10.0, -90.0, 0.0), 1.18);
        let out = listener_view(&given, None, &ours, 1.0);
        assert_eq!((out.position, out.forward, out.right), (ours.position, ours.forward, ours.right));
        assert_eq!(out.fov, given.fov);
        // A sound to the right of OUR camera is to the right of the game's listener too.
        let l = of_view(&out, frame.origin);
        let a = (-90.0f32).to_radians();
        let point = Vec3::new(-400.0, 300.0, 220.0) + Vec3::new(-a.sin(), a.cos(), 0.0) * 500.0;
        assert!(kh_audio::pan_of(&l, point) > 0.4);
        // In a hand-over (weight 0.5) it is the blend of the two, as drawn; a matrix already ours is kept.
        let half = listener_view(&given, None, &ours, 0.5);
        assert!((half.position - given.blend(&ours, 0.5).position).length() < 1e-5);
        let kept = listener_view(&ours, Some(ours), &ours, 0.5);
        assert_eq!(kept.position, ours.position);
        // Weight 0 (Elden Ring's camera): its own matrix.
        assert!((listener_view(&given, None, &ours, 0.0).position - given.position).length() < 1e-5);
    }
}
