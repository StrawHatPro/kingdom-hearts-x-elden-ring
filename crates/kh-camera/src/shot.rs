//! THE SHOT CAMERA: A PORT-SIDE DESIGN, NOT GAME BEHAVIOUR (fixes pass 1, 2026-10-12).
//!
//! USER REQUEST: "the camera should be more cinematic when performing the actual shotlock move
//! and then be behind sora".
//!
//! WHAT THE GAME DOES, as far as it was read (docs/sora-magic-spec.md A3.7, part D section 7):
//! `StartCommand` ends the aim camera `ATresCameraShootFlow` with its 0.5 s ease-out blend the
//! moment the shot starts, and the ShootFlow state (Begin 0x1413A5CD0, Tick, End, the per-kind
//! updates) makes no camera call; Sora's clips of the move (1473 / 1474 King of Hearts, 1481 /
//! 1482 Ragnarok) do not animate the camera bone `t_camera`, and the shotlock rows carry no
//! camera field. So by the code as read the field camera shows the move, wherever the aim left
//! it. No authored camera was found to port. A host that wants exactly that does not create
//! this camera (the test app: `--game-exact-shotlock-camera`).
//!
//! THE DESIGN (every number is a constant of this file, chosen here, not read from the game):
//! 1. on firing, a blend of [`BLEND_IN`] s from the view on screen (the aim view) to a LOW
//!    FRONT THREE-QUARTER SHOT of Sora that orbits slowly round him while he lifts / charges;
//! 2. when the volley starts, a swing of [`SWING`] s to a WIDE SHOT from behind and beside him
//!    that frames him and the centroid of the locked targets, so that the shots are seen
//!    travelling and landing;
//! 3. when the move ends, a blend of [`BLEND_OUT`] s (ease-out, exponent 2, as the game's camera
//!    blends) to the camera underneath, which the host has put behind Sora (the field camera's
//!    own `reset`, the result of the R3 button).
//! The camera never passes through level geometry: each shot's position is the end of one
//! sphere sweep (radius [`PROBE_RADIUS`], the field camera's) from its look point. The sticks do
//! not reach it; the host also keeps them from the camera underneath while it shows.

use glam::Vec3;
use kh_core::World;

use crate::rig::{blend_views, ease_out};
use crate::{CameraTarget, CameraView, Rot};

/// Seconds from the view at the moment of firing to the close shot.
pub const BLEND_IN: f32 = 0.3;
/// Seconds of the swing from the close shot to the wide shot.
pub const SWING: f32 = 0.25;
/// Seconds back to the camera underneath when the move has ended.
pub const BLEND_OUT: f32 = 0.5;
/// Exponent of the ease-out of all three.
pub const BLEND_EXPONENT: f32 = 2.0;
/// The close shot: horizontal distance from his origin, cm; the camera's height above his
/// origin (low: it looks up at him); the height it looks at; where on the circle round him it
/// starts, degrees to the right of his facing; how fast it orbits, degrees a second.
pub const CLOSE_DISTANCE: f32 = 210.0;
pub const CLOSE_HEIGHT: f32 = 60.0;
pub const CLOSE_LOOK_HEIGHT: f32 = 115.0;
pub const CLOSE_START_ANGLE: f32 = 38.0;
pub const CLOSE_ORBIT_SPEED: f32 = 22.0;
/// The wide shot, in the frame "from Sora toward the targets": cm behind him, to the side and
/// above his chest, each growing with the horizontal distance to the targets (capped at
/// [`WIDE_SPAN_MAX`]); the chest height; how far ahead of him the look point is when nothing
/// is locked.
pub const WIDE_BACK: f32 = 330.0;
pub const WIDE_BACK_PER_CM: f32 = 0.30;
pub const WIDE_SIDE: f32 = 250.0;
pub const WIDE_SIDE_PER_CM: f32 = 0.22;
pub const WIDE_UP: f32 = 150.0;
pub const WIDE_UP_PER_CM: f32 = 0.14;
pub const WIDE_CHEST_HEIGHT: f32 = 100.0;
pub const WIDE_SPAN_MAX: f32 = 2500.0;
pub const WIDE_NO_TARGET_AHEAD: f32 = 600.0;
/// Radius of the sweep that keeps the camera out of the level (the field camera's probe).
pub const PROBE_RADIUS: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShotPhase {
    #[default]
    Off,
    /// The close shot of the lift / charge.
    Close,
    /// The wide shot of the volley.
    Wide,
    /// Blending back to the camera underneath.
    Out,
}

/// The shot camera: a host keeps one beside its camera stack, calls [`ShotCamera::start`] when a
/// shotlock is fired, [`ShotCamera::end`] when its state has ended, and [`ShotCamera::step`]
/// every step with the view of the camera underneath; `step` returns the view to show.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ShotCamera {
    pub phase: ShotPhase,
    /// The view the blend in progress started from, and its seconds so far and in all.
    from: Option<CameraView>,
    blend: f32,
    blend_time: f32,
    /// Seconds in the close shot (the orbit), and the side of the wide shot (+1 right, -1 left).
    orbit: f32,
    side: f32,
    /// The view shown last.
    last: Option<CameraView>,
}

impl ShotCamera {
    /// A shotlock was fired: `now` is the view on screen.
    pub fn start(&mut self, now: &CameraView) {
        *self = ShotCamera { phase: ShotPhase::Close, from: Some(*now), blend: 0.0, blend_time: BLEND_IN, orbit: 0.0, side: 1.0, last: Some(*now) };
    }

    /// The move has ended (or was broken off): blend back to the camera underneath.
    pub fn end(&mut self) {
        if matches!(self.phase, ShotPhase::Close | ShotPhase::Wide) {
            (self.phase, self.from, self.blend, self.blend_time) = (ShotPhase::Out, self.last, 0.0, BLEND_OUT);
        }
    }

    /// It shows something (the blend out included).
    pub fn active(&self) -> bool {
        self.phase != ShotPhase::Off
    }

    /// It has the view to itself: the host keeps the sticks from the camera underneath.
    pub fn owns_view(&self) -> bool {
        matches!(self.phase, ShotPhase::Close | ShotPhase::Wide)
    }

    /// One step (`dt` real time). `focus` = the centroid of the locked targets, `wide` = the
    /// volley has started (the close shot gives way to the wide one), `under` = the view of the
    /// camera underneath after its own step.
    pub fn step(&mut self, dt: f32, target: &CameraTarget, focus: Option<Vec3>, wide: bool, world: &dyn World, under: &CameraView) -> CameraView {
        let shown = match self.phase {
            ShotPhase::Off => return *under,
            ShotPhase::Out => {
                self.blend += dt;
                if self.blend >= self.blend_time {
                    *self = ShotCamera::default();
                    return *under;
                }
                blend_views(&self.from.unwrap_or(*under), under, ease_out(self.blend / self.blend_time, BLEND_EXPONENT))
            }
            ShotPhase::Close | ShotPhase::Wide => {
                if wide && self.phase == ShotPhase::Close {
                    // The wide shot goes to the side the close shot is on, so the swing is short.
                    let angle = CLOSE_START_ANGLE + CLOSE_ORBIT_SPEED * self.orbit;
                    self.side = if crate::normalize_axis(angle) >= 0.0 { 1.0 } else { -1.0 };
                    (self.phase, self.from, self.blend, self.blend_time) = (ShotPhase::Wide, self.last, 0.0, SWING);
                }
                let own = if self.phase == ShotPhase::Close {
                    self.orbit += dt;
                    close_shot(target, self.orbit, world, under)
                } else {
                    wide_shot(target, focus, self.side, world, under)
                };
                self.blend += dt;
                match self.from {
                    Some(from) if self.blend < self.blend_time => blend_views(&from, &own, ease_out(self.blend / self.blend_time, BLEND_EXPONENT)),
                    _ => own,
                }
            }
        };
        self.last = Some(shown);
        shown
    }
}

fn dir(yaw_degrees: f32) -> Vec3 {
    let yaw = yaw_degrees.to_radians();
    Vec3::new(yaw.cos(), yaw.sin(), 0.0)
}

/// A view at `wanted` looking at `look`, pulled toward `look` by the level.
fn view_from(look: Vec3, wanted: Vec3, world: &dyn World, like: &CameraView) -> CameraView {
    let (position, collided) = match world.sweep_capsule(look, wanted, PROBE_RADIUS, PROBE_RADIUS) {
        Some(hit) => (look + (wanted - look) * hit.time, true),
        None => (wanted, false),
    };
    let rotation = Rot::looking_along(look - position);
    CameraView { position, rotation, rotation_target: rotation, look_point: look, distance: (look - position).length(), stick_active: false, followed: false, collided, ..*like }
}

/// The low front three-quarter shot, `orbit` seconds into its orbit.
pub fn close_shot(target: &CameraTarget, orbit: f32, world: &dyn World, like: &CameraView) -> CameraView {
    let look = target.position + Vec3::Z * CLOSE_LOOK_HEIGHT;
    let around = dir(target.yaw + CLOSE_START_ANGLE + CLOSE_ORBIT_SPEED * orbit);
    view_from(look, target.position + around * CLOSE_DISTANCE + Vec3::Z * CLOSE_HEIGHT, world, like)
}

/// The wide shot that frames Sora and `focus` from behind him, on `side` (+1 right, -1 left).
pub fn wide_shot(target: &CameraTarget, focus: Option<Vec3>, side: f32, world: &dyn World, like: &CameraView) -> CameraView {
    let chest = target.position + Vec3::Z * WIDE_CHEST_HEIGHT;
    let focus = focus.unwrap_or(chest + dir(target.yaw) * WIDE_NO_TARGET_AHEAD);
    let flat = Vec3::new(focus.x - chest.x, focus.y - chest.y, 0.0);
    let span = flat.length().min(WIDE_SPAN_MAX);
    let toward = if flat.length() > 1.0 { flat.normalize() } else { dir(target.yaw) };
    // (X forward, Y right, Z up, left-handed: the right of (1, 0) is (0, 1).)
    let right = Vec3::new(-toward.y, toward.x, 0.0);
    let wanted = chest - toward * (WIDE_BACK + WIDE_BACK_PER_CM * span) + right * side * (WIDE_SIDE + WIDE_SIDE_PER_CM * span) + Vec3::Z * (WIDE_UP + WIDE_UP_PER_CM * span);
    view_from(chest.lerp(focus, 0.5), wanted, world, like)
}
