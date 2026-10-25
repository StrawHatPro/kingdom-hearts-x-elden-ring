//! What the stage-1 states share: the game's "on ground", the states' ground probe, `Turn` and
//! `SetMovementInputXY` (docs/sora-movement-spec.md revision 2; addresses are the exe's).

use glam::Vec3;

use crate::pawn::Pawn;
use crate::sora::wrap_degrees;
use crate::world::World;

/// The game's "on ground" (0x1405AD850): not floating and movement mode Walking.
pub(crate) fn on_ground(s: &Pawn) -> bool {
    // (actions implementer, tier 3: "not floating" is pawn+0xD80 bit 0, the host's switch
    // `Sora::set_floating`; off unless a host turns it on.)
    !s.actions.traversal.floating && s.cmc.on_ground()
}

/// The states' ground probe (0x14063A650), spec 4.5, change D1: a sphere of radius 50 swept from
/// the actor origin + 55 straight down `range`. Returns (hit, distance, ground Z):
/// start-penetrating -> (false, 0); a surface less steep than 45 degrees -> (true, actor Z -
/// impact Z, impact Z) (the distance can be negative); steeper -> that distance is remembered and
/// the next hit is tried; nothing -> (false, range).
pub(crate) fn ground_probe(s: &Pawn, w: &dyn World, range: f32) -> (bool, f32, f32) {
    let p = s.params();
    let actor = s.cmc.location;
    let start = actor + Vec3::Z * p.probe_start_height;
    let r = p.probe_sphere_radius;
    let limit = p.probe_max_slope.to_radians().cos();
    let mut distance = range;
    for hit in w.sweep_capsule_all(start, start - Vec3::Z * range, r, r) {
        if hit.start_penetrating {
            return (false, 0.0, actor.z);
        }
        distance = actor.z - hit.position.z;
        if hit.impact_normal.z > limit {
            return (true, distance, hit.position.z);
        }
    }
    (false, distance, actor.z)
}

/// `Turn(yaw, a)` (pawn slot 438 -> 0x1405DDD90): slerp of the actor rotation toward
/// (0, yaw, 0) by clamp01(a); with pitch = roll = 0 a linear fraction of the remaining angle.
pub(crate) fn turn(s: &mut Pawn, yaw: f32, a: f32) {
    s.yaw = wrap_degrees(s.yaw + wrap_degrees(yaw - s.yaw) * a.clamp(0.0, 1.0));
}

/// `SetMovementInputXY((cos yaw, sin yaw), 1.0)` (pawn slot 724 0x1405B9BD0): assigned.
pub(crate) fn set_movement_input(s: &mut Pawn, yaw: f32) {
    let a = yaw.to_radians();
    s.cmc.control_input = Vec3::new(a.cos(), a.sin(), 0.0);
}
