//! `kh_core::World` over Elden Ring's Havok world: conversions (`coords`), the sphere cast
//! (`havok`), the PhysX-semantics adapter (`sweep`).
//!
//! WHAT BLOCKS (WALLS 2026-10-23): THE TARNISHED'S OWN COLLISION WORD (`havok::FILTER_PLAYER`, layer 2: what the game's player
//! controller casts with, read in the game's code), minus every body owned by a character. It meets the character-only
//! collision (layer 0x35: the invisible walls) and the characters' mesh, not the camera-only mesh or the water surface. The
//! leader and the friends cast through this world (the party gets Sora's); the camera has its own (cam.rs, the camera's word).
//! (Before: the old `havok::FILTER_SORA`, layer 0x56, which passed through the invisible walls.)
//!
//! CONTACT MARGIN: Havok stops a cast 1.5 cm before the surface (measured: the contact point is
//! radius + 0.015 m from the sphere's centre). The cast is made with the radius reduced by that
//! margin, so that contacts are at the radius `kh-core` asked for.

use std::cell::{Cell, RefCell};

use glam::Vec3;
use kh_core::{Hit, World};

use crate::coords::{dir_kh_to_er, unit_er_to_kh, Frame, CM_PER_M};
use crate::havok::{self, CastHit};
use crate::sweep::{self, Contact, Extra, SphereCaster};

/// Metres Havok keeps between a cast shape and what it hits.
pub const CAST_MARGIN_M: f32 = 0.015;

pub struct ErWorld {
    pub frame: Frame,
    pub filter: u32,
    /// Filter words of bodies that do not block him (`w.reject` of the control file; the water
    /// surface's, once known: IMPL_ER2_STATE.md "water").
    pub reject: RefCell<Vec<u32>>,
    /// What the casts touched since the last log line: (owner type, the hit's four unknown tag
    /// words) -> contacts.
    pub seen: RefCell<std::collections::BTreeMap<(u8, [u32; 4]), u32>>,
    scratch: RefCell<Vec<CastHit>>,
    /// Sphere casts made (for the log).
    pub casts: Cell<u32>,
    /// (IMPL_ER10) WHILE SORA FOLLOWS THE BODY (a cutscene, the wait for the ground after the game moved its player)
    /// the world is one level floor at this KH3 height: he stands on it whatever the game's collision says.
    pub flat: Cell<Option<f32>>,
    /// (IMPL_ER18, w.hits = 1) the non-floor bodies his casts met since the last log line.
    pub walls: RefCell<crate::wallscan::Walls>,
    pub log_walls: Cell<bool>,
    /// (IMPL_ER18) THE WIDEST CAST (KH3 cm; sheet row `collision.radius`, key `w.radius`). WHY: KH3's capsule is 60 cm in
    /// radius and Elden Ring's world is built for its own player's 40 cm: MEASURED, an opened double door (Stranded Graveyard,
    /// AEG027_039) leaves a gap of 0.80 m between its leaves, so Sora stood before every opened door and could not pass, and
    /// where the game's own action left its body between the leaves he was pushed back out. A cast wider than this is made
    /// with this radius and THE SAME HALF HEIGHT (the capsule's lowest and highest points stay where they were).
    pub radius_cap: Cell<f32>,
    /// (IMPL_ER23A) what the adapter does about edges, several overlaps at once and cracks between boards (sweep.rs `Options`;
    /// keys `w.edge`, `w.mtd`, `w.gap`; sheet row `collision.gap`).
    pub opts: Cell<sweep::Options>,
    /// (IMPL_ER24) THE FLOOR LAYER: the filter word of the second cast (a layer of the family that sees the map's OTHER
    /// collision mesh, the one characters stand on; 0 = none). Sheet row `collision.floor_layer`, key `w.floor`.
    pub floor_filter: Cell<u32>,
    scratch2: RefCell<Vec<CastHit>>,
    second: RefCell<Vec<Contact>>,
    /// (IMPL_ER24) Second-layer casts made, and contacts taken from them (for the log).
    pub casts2: Cell<u32>,
    pub taken2: Cell<u32>,
    /// (IMPL_ER24) THE LEG IK'S FLOOR: (the height he is supported at, how far below it a foot may reach), KH3 cm. A foot
    /// trace that goes deeper (through a hole in a plank floor, past a ledge) is answered with his own floor's height.
    pub ik_floor: Cell<Option<(f32, f32)>>,
    /// (IMPL_ER52) The graces' mounds (graceflame.rs `mound_body`) are passed through: None = as `grace.mound_collision`
    /// says (Sora's world), Some = this world's own rule (the save point's ground trace).
    pub mounds: Cell<Option<bool>>,
}

impl ErWorld {
    pub fn new(frame: Frame) -> Self {
        Self { frame, filter: havok::FILTER_PLAYER /* (WALLS) */, reject: RefCell::new(Vec::new()), seen: RefCell::new(std::collections::BTreeMap::new()), scratch: RefCell::new(Vec::new()), casts: Cell::new(0), flat: Cell::new(None), walls: RefCell::new(Default::default()), log_walls: Cell::new(false), radius_cap: Cell::new(f32::MAX), opts: Cell::new(sweep::Options::DEFAULT), floor_filter: Cell::new(0), scratch2: RefCell::new(Vec::new()), second: RefCell::new(Vec::new()), casts2: Cell::new(0), taken2: Cell::new(0), ik_floor: Cell::new(None), mounds: Cell::new(None) }
    }
}

impl SphereCaster for ErWorld {
    fn cast(&self, start: Vec3, delta: Vec3, radius: f32, out: &mut Vec<Contact>) {
        out.clear();
        if let Some(floor) = self.flat.get() {
            if let Some(contact) = flat_contact(floor, start, delta, radius) {
                out.push(contact);
            }
            return;
        }
        let mut hits = self.scratch.borrow_mut();
        let radius_m = (radius / CM_PER_M - CAST_MARGIN_M).max(0.001);
        havok::cast_sphere(self.frame.pos_kh_to_er(start), dir_kh_to_er(delta), radius_m, self.filter, false, &mut hits);
        self.casts.set(self.casts.get() + 1);
        let reject = self.reject.borrow();
        let mounds = self.mounds.get().unwrap_or_else(crate::graceflame::mounds_passable);
        let mut seen = self.seen.borrow_mut();
        for h in hits.iter() {
            if seen.len() < 48 || seen.contains_key(&(h.owner, h.tags)) {
                *seen.entry((h.owner, h.tags)).or_insert(0) += 1;
            }
            if reject.contains(&h.filter) {
                continue;
            }
            // (IMPL_ER52) the grace's mound: no collision for him
            if mounds && h.owner == 6 && crate::graceflame::mound_body(h.owner_ptr) {
                continue;
            }
            if self.log_walls.get() {
                crate::wallscan::note(&mut self.walls.borrow_mut(), h, self.frame.pos_kh_to_er(start) - Vec3::Y * 0.9);
            }
            let normal = unit_er_to_kh(h.normal).normalize_or_zero();
            if normal == Vec3::ZERO || !h.fraction.is_finite() {
                continue;
            }
            out.push(Contact { fraction: h.fraction.clamp(0.0, 1.0), point: self.frame.pos_er_to_kh(h.position), normal, body: h.body + 1 });
        }
    }

    /// (IMPL_ER24) The cast, and with it the floor layer's: the same sphere once more with the second filter word; of what
    /// that meets the map's bodies only (an object answers both layers: MEASURED, IMPL_ER18's layer survey), and of those
    /// what `extra` asks for (`sweep::merge_extra`: floors, not walls).
    fn cast_extra(&self, start: Vec3, delta: Vec3, radius: f32, extra: Extra, out: &mut Vec<Contact>) {
        self.cast(start, delta, radius, out);
        let filter = self.floor_filter.get();
        if extra == Extra::None || filter == 0 || filter == self.filter || self.flat.get().is_some() {
            return;
        }
        let mut hits = self.scratch2.borrow_mut();
        let radius_m = (radius / CM_PER_M - CAST_MARGIN_M).max(0.001);
        havok::cast_sphere(self.frame.pos_kh_to_er(start), dir_kh_to_er(delta), radius_m, filter, false, &mut hits);
        self.casts2.set(self.casts2.get() + 1);
        let mut second = self.second.borrow_mut();
        second.clear();
        for h in hits.iter() {
            let normal = unit_er_to_kh(h.normal).normalize_or_zero();
            if h.owner == 6 || normal == Vec3::ZERO || !h.fraction.is_finite() {
                continue;
            }
            second.push(Contact { fraction: h.fraction.clamp(0.0, 1.0), point: self.frame.pos_er_to_kh(h.position), normal, body: h.body + 1 });
        }
        let before = out.len();
        sweep::merge_extra(out, &second, start, delta, radius, extra);
        self.taken2.set(self.taken2.get() + (out.len() - before) as u32);
    }
}

impl World for ErWorld {
    fn sweep_capsule(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Option<Hit> {
        if radius <= 0.0 && half_height <= 0.0 {
            return sweep::line_first(self, start, end, &self.opts.get());
        }
        sweep::sweep_first(self, start, end, radius.min(self.radius_cap.get()), half_height, &self.opts.get())
    }

    fn line_trace(&self, start: Vec3, end: Vec3) -> Option<Hit> {
        sweep::line_first(self, start, end, &self.opts.get())
    }

    fn sweep_capsule_all(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Vec<Hit> {
        sweep::sweep_all(self, start, end, radius.min(self.radius_cap.get()), half_height, &self.opts.get())
    }
}

impl kh_anim::IkWorld for ErWorld {
    fn sweep_sphere(&self, start: Vec3, end: Vec3, radius: f32) -> Option<kh_anim::SweepHit> {
        let hit = sweep::sweep_first(self, start, end, radius, radius, &self.opts.get());
        // (IMPL_ER24) a foot never reaches through a hole to a floor far below his own
        if let Some((floor, drop)) = self.ik_floor.get() {
            let delta = end - start;
            if delta.z < -1.0 && hit.is_none_or(|h| h.position.z < floor - drop) {
                let contact = flat_contact(floor, start, delta, radius)?;
                return Some(kh_anim::SweepHit { time: contact.fraction, impact_point: contact.point, impact_normal: Vec3::Z, start_penetrating: false });
            }
        }
        let hit = hit?;
        Some(kh_anim::SweepHit { time: hit.time, impact_point: hit.position, impact_normal: hit.impact_normal, start_penetrating: hit.start_penetrating })
    }
}

/// The contact of a sphere cast with the level floor at height `floor` (KH3 space, Z up), as the Havok
/// cast reports contacts: the point on the surface, the surface normal; a start in overlap = fraction 0.
pub fn flat_contact(floor: f32, start: Vec3, delta: Vec3, radius: f32) -> Option<Contact> {
    let gap = start.z - radius - floor;
    if gap <= 0.0 {
        return Some(Contact { fraction: 0.0, point: Vec3::new(start.x, start.y, floor), normal: Vec3::Z, body: 1 });
    }
    if delta.z >= 0.0 {
        return None;
    }
    let fraction = gap / -delta.z;
    (fraction <= 1.0).then(|| {
        let at = start + delta * fraction;
        Contact { fraction, point: Vec3::new(at.x, at.y, floor), normal: Vec3::Z, body: 1 }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flat_floor_answers_like_a_cast() {
        // a sphere of 10 falling 100 from 50 above its resting height: stopped half way
        let c = flat_contact(0.0, Vec3::new(3.0, 4.0, 60.0), Vec3::new(0.0, 0.0, -100.0), 10.0).unwrap();
        assert!((c.fraction - 0.5).abs() < 1e-6 && c.point == Vec3::new(3.0, 4.0, 0.0) && c.normal == Vec3::Z);
        // moving away or level: nothing; too short: nothing; in overlap: fraction 0
        assert!(flat_contact(0.0, Vec3::new(0.0, 0.0, 60.0), Vec3::new(5.0, 0.0, 0.0), 10.0).is_none());
        assert!(flat_contact(0.0, Vec3::new(0.0, 0.0, 60.0), Vec3::new(0.0, 0.0, -20.0), 10.0).is_none());
        assert_eq!(flat_contact(0.0, Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -20.0), 10.0).unwrap().fraction, 0.0);
    }

    /// (IMPL_ER24) The leg IK's trace over a hole in a plank floor: the floor far below is not where the foot goes.
    #[test]
    fn a_foot_does_not_reach_through_a_hole() {
        use kh_anim::IkWorld;
        let world = ErWorld::new(Frame::at(Vec3::ZERO));
        // (the level floor of the follow mode stands for what the trace finds: 3 m below his own floor)
        world.flat.set(Some(-300.0));
        let (from, to) = (Vec3::new(10.0, 0.0, 50.0), Vec3::new(10.0, 0.0, -400.0));
        let free = world.sweep_sphere(from, to, 5.0).unwrap();
        assert!((free.impact_point.z + 300.0).abs() < 1e-3, "without the rule: the floor below: {free:?}");
        world.ik_floor.set(Some((0.0, 25.0)));
        let held = world.sweep_sphere(from, to, 5.0).unwrap();
        assert!(held.impact_point.z.abs() < 1e-3 && held.impact_normal == Vec3::Z && (held.time - 45.0 / 450.0).abs() < 1e-4, "his own floor's height: {held:?}");
        // a step down within the limit is a step down; a trace that finds nothing at all is answered with his floor too
        world.flat.set(Some(-20.0));
        assert!((world.sweep_sphere(from, to, 5.0).unwrap().impact_point.z + 20.0).abs() < 1e-3);
    }
}