//! (IMPL_ER32; the user, 2026-10-13: "also Sora's attacks should break breakable objects and fire should explode
//! explosives") SORA'S HITS REACH ELDEN RING'S BREAKABLE MAP OBJECTS. UNTESTED IN THE GAME: written by reading the SDK
//! (Dasaav's fromsoftware-rs at the pinned revision) and this crate's measured notes; the engine-free parts are unit-tested.
//!
//! WHY NOTHING BROKE BEFORE: a hit of Sora's became a bullet of the player's only when kh-core counted a hit on a
//! CHARACTER (targets.rs `apply_hit`), a bullet 12 cm thick that lives 0.12 s and is aimed at that character's axis. No
//! bullet was ever made for an object, and a swing through a crate made nothing at all in the game.
//!
//! THE ROUTE (the bullet route; the game breaks the object itself: debris, sound, drops, its own explosion):
//! 1. FIND: sphere casts through the Havok world from Sora (two rings of directions and one cast down, `break.reach`
//!    long) and along every live spell / shotlock actor keep the contacts on bodies owned by a map object (owner type 6,
//!    `CSWorldGeomIns`). The object's `AssetEnvironmentGeometryParam` row is read through the pointer the SDK documents
//!    (`CSWorldGeomIns.info.asset_geometry_param`, object + 0x20; the MSB part name is 0x28 behind it at + 0x48, an offset
//!    this crate has MEASURED since IMPL_ER17, which confirms the layout for 2.7.1.0). `hp >= break.hp_min` (-1 is the
//!    game's "cannot break") or `isBurn` makes it a PROP. Every read is guarded (`probe`).
//! 2. HIT: a prop is offered to kh-core as a target that can be hit but not locked on to (`TargetFlags` ALIVE | HITTABLE
//!    | GIMMICK): one sphere of `break.hurt_radius` at each surface point the casts found. So EVERY attack volume of the
//!    core's (Keyblade windows, finishers, air hits, spell actors, shotlock, form attacks) reaches props by the very test
//!    that reaches enemies, and plays KH3's own hit effect and sound there.
//! 3. BREAK: `apply_hit` on a prop queues ONE bullet of the player's at that surface point (the borrowed rows of
//!    bullet.rs, in the one-bullet-a-frame turn of targets.rs, after the enemies' waiting bullets), whose attack row
//!    carries `atkObj` (the highest value any of the game's own player attack rows has, unless the sheet names one) and,
//!    for a KH3 fire attack, the fire damage type and the `spAttribute` the game gives its own pure fire rows.
//! 4. LEARN: whether a bullet that may pass through the map or one that may not is what breaks objects is NOT known;
//!    both are tried in turn and a cast 0.25 s later looks whether the object's body is still there. The variant that
//!    broke something is kept. An object that stays whole `break.give_up` times is left alone, and so is its model after
//!    the third such object; `break.give_up_total` tries without one break switch the feature off for this run.
//!
//! WALKING INTO THINGS (`break.collide`): Sora is stopped by pots and crates (his casts meet them), so the game's own
//! "broken by the player's touch" never happens under him. An object whose row says `isBreakByPlayerCollide`, within
//! `break.collide_reach` of his axis while he moves faster than `break.collide_speed`, gets the same bullet.
//!
//! `break.kh = 0`: no casts, no props, and the attack rows are patched exactly as before this pass (bullet.rs
//! `Attack::PLAIN`). Nothing is written to the game here: the only writes are bullet.rs's borrowed rows.

use std::collections::{HashMap, VecDeque};

use eldenring::param::ASSET_GEOMETORY_PARAM_ST;
use glam::Vec3;
use kh_core::{HitRecord, HitResult, HurtShape, TargetFlags, TargetId, TargetInfo};

use crate::bullet::{Attack, Carrier, Shape, Survey};
use crate::control::Control;
use crate::coords::{self, Frame, CM_PER_M};
use crate::erlog;
use crate::havok::{self, CastHit};
use crate::probe;

/// Target ids of props start here (characters count up from 1: targets.rs).
pub const PROP_BASE: u32 = 0x4000_0000;

pub fn is_prop(id: TargetId) -> bool {
    id.0 >= PROP_BASE
}

/// The owner type of a map object's body (`FieldInsBaseVmt`: CSWorldGeomIns = 6, SDK field_ins.rs).
const OWNER_GEOM: u8 = 6;
/// Offsets inside `CSWorldGeomIns` (SDK world_geom_man.rs: vfptr, handle, block data, then `info` at 0x18).
const GEOM_PARAM: usize = 0x20;
const GEOM_MSB_PART: usize = 0x48;

/// The sheet's rows `break.*` and `damage.element.*` (sora-er-host). Host choices, none is data of either game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreakRules {
    /// `break.reach`: cm the casts from Sora go out.
    pub reach: f32,
    /// `break.probe_radius`: cm, the radius of those casts.
    pub probe_radius: f32,
    /// `break.scan_interval`: s between two scans around Sora.
    pub scan_interval: f32,
    /// `break.hurt_radius`: cm, the sphere a found surface point is to kh-core's attack volumes.
    pub hurt_radius: f32,
    /// `break.hp_min`: an object with fewer hit points in its row cannot be broken (-1 in the game's rows).
    pub hp_min: i32,
    /// `break.atk_obj`: the object attack value of our bullets; 0 = the highest of the game's own player attack rows.
    pub atk_obj: u16,
    /// `break.atk_obj_floor`: the value when the game's rows carry none at all.
    pub atk_obj_floor: u16,
    /// `break.repeat`: s between two bullets at one object.
    pub repeat: f32,
    /// `break.give_up`: bullets one object may survive before it is left alone.
    pub give_up: u32,
    /// `break.give_up_total`: verified tries without a single break after which the feature goes quiet.
    pub give_up_total: u32,
    /// `break.collide`: objects the game's player breaks by touch break under Sora's touch.
    pub collide: bool,
    /// `break.collide_speed`: cm/s he must move at least.
    pub collide_speed: f32,
    /// `break.collide_reach`: cm from his axis to the object's surface.
    pub collide_reach: f32,
    /// `break.projectile_ahead`: s of a spell actor's flight its cast looks ahead.
    pub projectile_ahead: f32,
    /// `damage.element.*` by `ETresDamageAttribute` value (1 fire, 2 blizzard, 3 thunder, 4 water, 5 aero, 6 dark):
    /// the damage type that carries the number (`Carrier::from_code`; 0 = as before).
    pub element: [u8; 8],
}

impl BreakRules {
    pub const DEFAULT: BreakRules = BreakRules {
        reach: 300.0,
        probe_radius: 50.0,
        scan_interval: 0.1,
        hurt_radius: 45.0,
        hp_min: 0,
        atk_obj: 0,
        atk_obj_floor: 100,
        repeat: 0.15,
        give_up: 4,
        give_up_total: 40,
        collide: true,
        collide_speed: 250.0,
        collide_reach: 55.0,
        projectile_ahead: 0.15,
        element: [0, 2, 1, 3, 0, 0, 0, 0],
    };

    pub fn apply_row(&mut self, id: &str, v: f32) -> bool {
        match id {
            "break.reach" if (50.0..=1000.0).contains(&v) => self.reach = v,
            "break.probe_radius" if (5.0..=150.0).contains(&v) => self.probe_radius = v,
            "break.scan_interval" if (0.016..=2.0).contains(&v) => self.scan_interval = v,
            "break.hurt_radius" if (5.0..=200.0).contains(&v) => self.hurt_radius = v,
            "break.hp_min" if (-1.0..=30000.0).contains(&v) => self.hp_min = v as i32,
            "break.atk_obj" if (0.0..=60000.0).contains(&v) => self.atk_obj = v as u16,
            "break.atk_obj_floor" if (1.0..=60000.0).contains(&v) => self.atk_obj_floor = v as u16,
            "break.repeat" if (0.0..=5.0).contains(&v) => self.repeat = v,
            "break.give_up" if (1.0..=1000.0).contains(&v) => self.give_up = v as u32,
            "break.give_up_total" if (1.0..=100000.0).contains(&v) => self.give_up_total = v as u32,
            "break.collide" => self.collide = v != 0.0,
            "break.collide_speed" if (0.0..=5000.0).contains(&v) => self.collide_speed = v,
            "break.collide_reach" if (0.0..=300.0).contains(&v) => self.collide_reach = v,
            "break.projectile_ahead" if (0.0..=2.0).contains(&v) => self.projectile_ahead = v,
            _ => match (id.strip_prefix("damage.element.").and_then(element_index), (0.0..=4.0).contains(&v) && v.fract() == 0.0) {
                (Some(k), true) => self.element[k] = v as u8,
                _ => return false,
            },
        }
        true
    }

    /// The damage type that carries a hit of KH3 attribute `attribute` (an `ETresDamageAttribute` value).
    /// `Carrier::Shape` (the rows as before) for physical and for an attribute the sheet leaves at 0.
    pub fn carrier(&self, attribute: u8) -> Carrier {
        match self.element.get(attribute as usize).copied().unwrap_or(0) {
            0 => Carrier::Shape,
            code => Carrier::from_code(code),
        }
    }

    /// The object attack value: the sheet's (or the key's), else the highest of the game's own player rows, else the floor.
    pub fn object_attack(&self, survey: &Survey) -> (u16, &'static str) {
        if self.atk_obj > 0 {
            (self.atk_obj, "the sheet's break.atk_obj / the key break.obj")
        } else if survey.obj_max > 0 {
            (survey.obj_max, "the highest atkObj of the game's own player attack rows")
        } else {
            (self.atk_obj_floor, "break.atk_obj_floor (the game's player rows carry no atkObj)")
        }
    }
}

fn element_index(name: &str) -> Option<usize> {
    ["physical", "fire", "blizzard", "thunder", "water", "aero", "dark"].iter().position(|n| *n == name).filter(|k| *k > 0)
}

/// What an object's `AssetEnvironmentGeometryParam` row says about breaking it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facts {
    pub hp: i16,
    pub defense: u16,
    pub by_player_collide: bool,
    pub burn: bool,
    /// `breakBulletBehaviorId` (an explosive's own explosion) and `breakBulletAttributeDamageType`.
    pub break_bullet: i32,
    pub break_attribute: i8,
    pub burn_bullet: i32,
    /// `breakItemLotParamId`: what it drops.
    pub lot: i32,
}

impl Facts {
    pub fn of(row: &ASSET_GEOMETORY_PARAM_ST) -> Facts {
        Facts { hp: row.hp(), defense: row.defense(), by_player_collide: row.is_break_by_player_collide(), burn: row.is_burn(), break_bullet: row.break_bullet_behavior_id(), break_attribute: row.break_bullet_attribute_damage_type(), burn_bullet: row.burn_bullet_behavior_id(), lot: row.break_item_lot_param_id() }
    }

    pub fn breakable(&self, rules: &BreakRules) -> bool {
        self.hp as i32 >= rules.hp_min || self.burn
    }
}

/// "AEG027_038" of "AEG027_038_0500": the model of a part name.
pub fn model_of(name: &str) -> &str {
    match name.match_indices('_').nth(1) {
        Some((at, _)) => &name[..at],
        None => name,
    }
}

/// WHERE THE BULLET FOR A SURFACE POINT STARTS AND WHICH WAY IT FLIES (Elden Ring space, metres). The game flies our
/// bullets level whatever direction is given (MEASURED, targets.rs `aim`): it starts `back` in front of the point at the
/// point's height and flies level into the surface; where the surface faces up or down (no level normal) it comes from
/// Sora's side.
pub fn aim_prop(point: Vec3, normal_out: Vec3, sora: Vec3, back: f32) -> (Vec3, Vec3) {
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let into = flat(-normal_out);
    if into.length() > 0.3 {
        let dir = into.normalize();
        return (point - dir * back, dir);
    }
    // (a top or an underside: 10 cm inside the object, from his side, from no farther than 1.5 m)
    let inside = point - normal_out * 0.1;
    let to = flat(inside - sora);
    let dir = to.normalize_or(Vec3::Z);
    (inside - dir * to.length().clamp(back, 1.5), dir)
}

/// The two bullets tried: 0 = stopped by the map like an arrow, 1 = passes through it (the enemies' bullet of IMPL_ER6).
pub const VARIANTS: usize = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub broke: u32,
    pub still: u32,
}

/// Which variant the next bullet at an object is: the forced one; else the one that has broken the most; else, while
/// nothing has broken yet, the two in turn per object.
pub fn choose_variant(forced: Option<usize>, tallies: &[Tally; VARIANTS], shots_at_object: u32) -> usize {
    if let Some(v) = forced.filter(|v| *v < VARIANTS) {
        return v;
    }
    match (0..VARIANTS).filter(|v| tallies[*v].broke > 0).max_by_key(|v| (tallies[*v].broke, std::cmp::Reverse(*v))) {
        Some(best) => best,
        None => shots_at_object as usize % VARIANTS,
    }
}

/// The touch rule: fast enough, the surface within reach of his axis and at his body's height (cm).
pub fn touches(rules: &BreakRules, speed: f32, flat_distance: f32, dz: f32) -> bool {
    rules.collide && speed >= rules.collide_speed && flat_distance <= rules.collide_reach && dz.abs() <= 100.0
}

/// The level directions of a ring of `n` casts (KH3 space).
fn ring(n: usize) -> impl Iterator<Item = Vec3> {
    (0..n).map(move |k| {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        Vec3::new(a.cos(), a.sin(), 0.0)
    })
}

#[derive(Clone, Copy, Debug)]
struct Point {
    /// The surface point and the surface's normal (out of it), Elden Ring space.
    er: Vec3,
    normal: Vec3,
    filter: u32,
    at: f32,
}

#[derive(Clone, Copy, Debug)]
struct Verify {
    due: f32,
    from: Vec3,
    dir: Vec3,
    length: f32,
    filter: u32,
    variant: usize,
    attack: &'static str,
}

struct Asset {
    id: u32,
    owner: usize,
    name: String,
    facts: Facts,
    points: Vec<Point>,
    last_shot: f32,
    shots: u32,
    fails: u32,
    verify: Option<Verify>,
    /// Broken, or given up on.
    dead: bool,
    logged: bool,
}

#[derive(Clone, Copy, Debug)]
struct Shot {
    asset: u32,
    point: Vec3,
    normal: Vec3,
    filter: u32,
    wanted: i32,
    attribute: u8,
    attack: &'static str,
    at: f32,
}

/// What is known of an owner pointer: the prop's index, or "not a prop" until `again`.
enum Known {
    Prop(usize),
    Not { again: f32 },
}

pub struct Props {
    pub rules: BreakRules,
    pub on: bool,
    said_on: Option<bool>,
    /// The game's rows as surveyed, the object attack value in use.
    survey: Option<Survey>,
    obj: u16,
    elements: bool,
    forced_variant: Option<usize>,
    assets: Vec<Asset>,
    known: HashMap<usize, Known>,
    ignored_models: HashMap<String, u32>,
    next_id: u32,
    frame: Frame,
    clock: f32,
    next_scan: f32,
    scan_no: u32,
    sora: Vec3,
    sora_before: Option<Vec3>,
    speed: f32,
    fast_until: f32,
    queue: VecDeque<Shot>,
    pub tallies: [Tally; VARIANTS],
    gave_up: bool,
    tokens: f32,
    scratch: Vec<CastHit>,
    pub seen: u32,
    pub shots: u32,
    next_summary: f32,
    summary_shots: u32,
}

impl Default for Props {
    fn default() -> Props {
        Props { rules: BreakRules::DEFAULT, on: false, said_on: None, survey: None, obj: 0, elements: false, forced_variant: None, assets: Vec::new(), known: HashMap::new(), ignored_models: HashMap::new(), next_id: PROP_BASE, frame: Frame::at(Vec3::ZERO), clock: 0.0, next_scan: 0.0, scan_no: 0, sora: Vec3::ZERO, sora_before: None, speed: 0.0, fast_until: -1.0, queue: VecDeque::new(), tallies: [Tally::default(); VARIANTS], gave_up: false, tokens: 4.0, scratch: Vec::new(), seen: 0, shots: 0, next_summary: 0.0, summary_shots: 0 }
    }
}

fn utf16_at(at: usize) -> Option<String> {
    let mut b = [0u8; 80];
    if !probe::read(at, &mut b) {
        return None;
    }
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
    let s = String::from_utf16_lossy(&units);
    (!s.is_empty() && s.chars().all(|c| c.is_ascii_graphic())).then_some(s)
}

/// The MSB part name of a map object (guarded reads).
fn name_of(owner: usize) -> Option<String> {
    probe::word(owner + GEOM_MSB_PART).and_then(probe::word).and_then(utf16_at)
}

/// The object's row, copied out through guarded reads.
fn facts_of(owner: usize) -> Option<Facts> {
    let row = probe::word(owner + GEOM_PARAM).filter(|p| *p % 4 == 0)?;
    let mut bytes = [0u8; std::mem::size_of::<ASSET_GEOMETORY_PARAM_ST>()];
    if !probe::read(row, &mut bytes) {
        return None;
    }
    // SAFETY: a parameter row is plain numbers; the buffer is its size.
    let row: ASSET_GEOMETORY_PARAM_ST = unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const ASSET_GEOMETORY_PARAM_ST) };
    Some(Facts::of(&row))
}

impl Props {
    /// A log line of the per-object kind may be written now (a few a second).
    fn may_log(&mut self) -> bool {
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn live(&self, p: &Point) -> bool {
        self.clock - p.at <= self.rules.scan_interval * 2.0 + 0.06
    }

    /// The physics origin moved: the remembered points are of the old origin.
    pub fn rebased(&mut self) {
        for a in &mut self.assets {
            a.points.clear();
            a.verify = None;
        }
        self.queue.clear();
        self.sora_before = None;
        self.next_scan = 0.0;
    }

    fn reset(&mut self) {
        self.assets.clear();
        self.known.clear();
        self.queue.clear();
        self.sora_before = None;
    }

    /// The damage type and the element mark of a hit of KH3 attribute `attribute` on anything (enemies too).
    pub fn element_of(&self, attribute: u8) -> (Carrier, Option<u8>) {
        if !self.on || !self.elements {
            return (Carrier::Shape, None);
        }
        let carrier = self.rules.carrier(attribute);
        (carrier, self.survey.as_ref().and_then(|s| s.sp_of(carrier)))
    }

    /// `atkObj` of every bullet of Sora's while the feature is on.
    pub fn object_attack(&self) -> Option<u16> {
        (self.on && self.obj > 0).then_some(self.obj)
    }

    fn usable(&self, a: &Asset) -> bool {
        !a.dead && a.points.iter().any(|p| self.live(p))
    }

    fn info_of(&self, a: &Asset) -> Option<TargetInfo> {
        let nearest = a.points.iter().filter(|p| self.live(p)).min_by(|x, y| (x.er - self.sora).length_squared().total_cmp(&(y.er - self.sora).length_squared()))?;
        let centre = self.centre_kh(nearest);
        let r = self.rules.hurt_radius;
        Some(TargetInfo { id: TargetId(a.id), position: centre, center: centre, velocity: Vec3::ZERO, yaw: 0.0, radius: r, half_height: r, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::GIMMICK), hp: 1.0, hp_max: 1.0 })
    }

    /// The hurt sphere of a surface point: half inside the object (KH3 space, cm).
    fn centre_kh(&self, p: &Point) -> Vec3 {
        self.frame.pos_er_to_kh(p.er - p.normal * (self.rules.hurt_radius * 0.5 / CM_PER_M))
    }

    pub fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        if !self.on || self.gave_up {
            return;
        }
        out.extend(self.assets.iter().filter(|a| self.usable(a)).filter_map(|a| self.info_of(a)));
    }

    pub fn info(&self, id: TargetId) -> Option<TargetInfo> {
        if !self.on || self.gave_up {
            return None;
        }
        self.assets.iter().find(|a| a.id == id.0 && self.usable(a)).and_then(|a| self.info_of(a))
    }

    pub fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        let Some(a) = self.assets.iter().find(|a| a.id == id.0) else { return };
        for (k, p) in a.points.iter().enumerate().filter(|(_, p)| self.live(p)) {
            let c = self.centre_kh(p);
            out.push(HurtShape { a: c, b: c, radius: self.rules.hurt_radius, part: k as u32 });
        }
    }

    /// A hit of the core's on a prop: one bullet is queued for it. `wanted`: the attack value (targets.rs `damage_out`).
    /// Never "accepted": no enemy gauge, no style points, no kill for a crate.
    pub fn hit(&mut self, hit: &HitRecord, wanted: i32) -> HitResult {
        let none = HitResult { accepted: false, damage_dealt: 0.0, hp_after: 1.0, killed: false, guarded: false };
        if !self.on || self.gave_up {
            return none;
        }
        let (clock, repeat) = (self.clock, self.rules.repeat);
        let contact = self.frame.pos_kh_to_er(hit.position);
        let Some(a) = self.assets.iter_mut().find(|a| a.id == hit.target.0) else { return none };
        if a.dead || clock - a.last_shot < repeat || self.queue.iter().any(|s| s.asset == a.id) {
            return none;
        }
        // (the part the core touched; else the point nearest the contact)
        let point = a.points.get(hit.part as usize).or_else(|| a.points.iter().min_by(|x, y| (x.er - contact).length_squared().total_cmp(&(y.er - contact).length_squared()))).copied();
        let Some(p) = point else { return none };
        if self.queue.len() < 16 {
            self.queue.push_back(Shot { asset: a.id, point: p.er, normal: p.normal, filter: p.filter, wanted: wanted.max(1), attribute: hit.attribute as i32 as u8, attack: hit.attack_name, at: clock });
        }
        none
    }

    /// Contacts of a cast on map objects become points of props.
    fn take(&mut self, hits: &[CastHit], filter: u32) {
        for h in hits {
            if h.owner != OWNER_GEOM || h.owner_ptr == 0 || !h.position.is_finite() || !h.normal.is_finite() || h.normal.length_squared() < 0.25 {
                continue;
            }
            let Some(k) = self.asset_of(h.owner_ptr) else { continue };
            let (clock, sora) = (self.clock, self.sora);
            let ttl = self.rules.scan_interval * 2.0 + 0.06;
            let a = &mut self.assets[k];
            a.points.retain(|p| clock - p.at <= ttl);
            let point = Point { er: h.position, normal: h.normal.normalize(), filter, at: clock };
            match a.points.iter().position(|p| (p.er - h.position).length() < 0.2) {
                Some(k) => a.points[k] = point,
                None if a.points.len() < 6 => a.points.push(point),
                None => {
                    // (full: the farthest from him gives way to a nearer one)
                    let d = |p: &Point| (p.er - sora).length_squared();
                    if let Some(far) = a.points.iter_mut().max_by(|x, y| d(x).total_cmp(&d(y))) {
                        if d(&point) < d(far) {
                            *far = point;
                        }
                    }
                }
            }
        }
    }

    /// The prop behind an owner pointer; `None`: not a breakable map object.
    fn asset_of(&mut self, owner: usize) -> Option<usize> {
        match self.known.get(&owner) {
            Some(Known::Prop(k)) => return Some(*k),
            Some(Known::Not { again }) if self.clock < *again => return None,
            _ => {}
        }
        let not = Known::Not { again: self.clock + 5.0 };
        let found = (|| {
            let class = probe::class_of(owner)?;
            if !class.contains("WorldGeom") || class.contains("Hit") {
                return None;
            }
            let name = name_of(owner)?;
            let facts = facts_of(owner)?;
            Some((name, facts))
        })();
        let Some((name, facts)) = found else {
            self.known.insert(owner, not);
            return None;
        };
        if !facts.breakable(&self.rules) || self.ignored_models.get(model_of(&name)).is_some_and(|n| *n >= 3) || self.assets.len() >= 512 {
            self.known.insert(owner, not);
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.seen += 1;
        self.assets.push(Asset { id, owner, name, facts, points: Vec::new(), last_shot: -100.0, shots: 0, fails: 0, verify: None, dead: false, logged: false });
        self.known.insert(owner, Known::Prop(self.assets.len() - 1));
        Some(self.assets.len() - 1)
    }

    fn cast(&mut self, from_kh: Vec3, delta_kh: Vec3, radius_cm: f32, filter: u32) {
        let mut hits = std::mem::take(&mut self.scratch);
        havok::cast_sphere_of(self.frame.pos_kh_to_er(from_kh), coords::dir_kh_to_er(delta_kh), (radius_cm / CM_PER_M).max(0.01), filter, false, Some(OWNER_GEOM), &mut hits);
        self.take(&hits, filter);
        self.scratch = hits;
    }

    /// The scan around a point: two rings of level casts and one down.
    fn scan(&mut self, centre_kh: Vec3, directions: usize, filter: u32) {
        let (reach, r) = (self.rules.reach, self.rules.probe_radius);
        for dz in [-45.0, 45.0] {
            for d in ring(directions) {
                self.cast(centre_kh + Vec3::Z * dz, d * reach, r, filter);
            }
        }
        self.cast(centre_kh, Vec3::NEG_Z * reach, r, filter);
    }

    /// The objects that moved on to another body at the same address (a map unloaded and another loaded): forgotten.
    fn revalidate(&mut self) {
        let mut stale = false;
        for a in self.assets.iter_mut().filter(|a| !a.points.is_empty()) {
            if name_of(a.owner).as_deref() != Some(a.name.as_str()) {
                stale = true;
            }
        }
        if stale || self.assets.len() >= 512 {
            self.reset();
        }
    }
}

/// ONCE A FRAME, after `ErTargets::refresh`: the switches, the scans, the touch rule, the check of earlier bullets and
/// the next bullet in its turn. `projectiles`: (position, velocity) of the core's live spell and shotlock actors, KH3 space.
pub fn frame(t: &mut crate::targets::ErTargets, control: &Control, projectiles: &[(Vec3, Vec3)], dt: f32) {
    let available = t.route.mode == crate::damage::Mode::Bullet && !t.route.off && t.bullets.available();
    let wanted = control.flag("break.kh") != Some(false);
    let (frame, sora, filter) = (t.frame, t.sora, t.cast_filter);
    let mut rules = t.params.breaking;
    if let Some(v) = control.flag("break.collide") {
        rules.collide = v;
    }
    if let Some(v) = control.i64("break.obj").filter(|v| (0..=60000).contains(v)) {
        rules.atk_obj = v as u16;
    }
    if let Some(v) = control.f32("break.reach").filter(|v| (50.0..=1000.0).contains(v)) {
        rules.reach = v;
    }
    let p = &mut t.props;
    p.rules = rules;
    p.elements = control.flag("dmg.element") != Some(false);
    p.forced_variant = control.i64("break.variant").filter(|v| (0..VARIANTS as i64).contains(v)).map(|v| v as usize);
    p.clock += dt.max(0.0);
    p.tokens = (p.tokens + dt.max(0.0) * 4.0).min(4.0);
    p.frame = frame;
    p.on = wanted && available;
    if p.said_on != Some(p.on) && (available || !wanted) {
        p.said_on = Some(p.on);
        if !p.on {
            erlog!("break: OFF (break.kh = 0): no object is looked for, the attack rows are patched as before IMPL_ER32");
            p.reset();
        }
    }
    if !p.on {
        return;
    }
    if p.survey.is_none() {
        let survey = t.bullets.survey();
        let (obj, why) = rules.object_attack(&survey);
        p.obj = obj;
        let mark = |k: usize| survey.sp[k].map_or("none found".to_string(), |(sp, n, of)| format!("{sp} ({n} of {of} pure rows)"));
        erlog!(
            "break: ON (the user: \"Sora's attacks should break breakable objects and fire should explode explosives\"). Every bullet of Sora's carries atkObj {obj} ({why}). The game's {} player attack rows: {} with an atkObj, commonest {}, highest {}. The rows we borrow have as the game made them (attack row: atkObj): {}",
            survey.rows, survey.obj_rows, survey.obj_common, survey.obj_max, survey.donors.iter().map(|(id, obj)| format!("{id}: {obj}")).collect::<Vec<_>>().join(", ")
        );
        erlog!(
            "break: elements {} (dmg.element): KH3 fire -> {:?}, blizzard -> {:?}, thunder -> {:?}, water -> {:?}, aero -> {:?}, dark -> {:?} (Shape = the row as before). The game's own spAttribute of its pure magic rows: {}; fire: {}; lightning: {}; holy: {}",
            if p.elements { "ON" } else { "OFF" }, rules.carrier(1), rules.carrier(2), rules.carrier(3), rules.carrier(4), rules.carrier(5), rules.carrier(6), mark(1), mark(2), mark(3), mark(4)
        );
        p.survey = Some(survey);
    } else if rules.atk_obj > 0 && rules.atk_obj != p.obj {
        p.obj = rules.atk_obj;
        erlog!("break: atkObj is now {} (break.obj)", p.obj);
    }
    if p.gave_up {
        return;
    }
    // ---- his speed (the touch rule)
    p.speed = match p.sora_before {
        Some(before) if dt > 1e-4 && (sora - before).length() < 500.0 => ((sora - before) * Vec3::new(1.0, 1.0, 0.0)).length() / dt,
        _ => 0.0,
    };
    p.sora_before = Some(sora);
    p.sora = frame.pos_kh_to_er(sora);
    // (the object stops him: the run still counts for a moment after)
    if p.speed >= rules.collide_speed {
        p.fast_until = p.clock + 0.3;
    }

    // ---- the scans: around him every `scan_interval` (the two layers in turn), along every spell actor every frame
    let filters = [filter, havok::FILTER_CAMERA];
    let scan_now = p.clock >= p.next_scan;
    if scan_now {
        p.next_scan = p.clock + rules.scan_interval * 0.5;
        p.scan_no += 1;
        let layer = filters[p.scan_no as usize % 2];
        if p.scan_no % 20 == 0 {
            p.revalidate();
        }
        p.scan(sora, 16, layer);
    }
    for (k, (at, velocity)) in projectiles.iter().copied().take(6).enumerate() {
        let speed = velocity.length();
        if !at.is_finite() || !speed.is_finite() {
            continue;
        }
        if speed > 100.0 {
            let reach = speed * rules.projectile_ahead + rules.probe_radius;
            p.cast(at - velocity / speed * 10.0, velocity / speed * reach, rules.probe_radius * 0.6, filters[(p.scan_no as usize + k) % 2]);
        } else if scan_now {
            // (an area that stays: the ring around it)
            p.scan(at, 8, filters[p.scan_no as usize % 2]);
        }
    }

    // ---- first sight of an object: what its row says
    for k in 0..p.assets.len() {
        if p.assets[k].logged || p.assets[k].points.is_empty() || !p.may_log() {
            continue;
        }
        let a = &mut p.assets[k];
        a.logged = true;
        let f = a.facts;
        erlog!("break: sees {} (target {:#x}): hp {} defense {}{}{}{}{}", a.name, a.id, f.hp, f.defense, if f.by_player_collide { ", breaks at the player's touch" } else { "" }, if f.burn { ", burns" } else { "" }, if f.break_bullet > 0 { format!(", its own bullet {} when broken (attribute condition {})", f.break_bullet, f.break_attribute) } else { String::new() }, if f.lot > 0 { format!(", drops lot {}", f.lot) } else { String::new() });
    }

    // ---- the touch rule
    if rules.collide && p.clock <= p.fast_until {
        let (clock, speed, sora_er) = (p.clock, rules.collide_speed, p.sora);
        for k in 0..p.assets.len() {
            let a = &p.assets[k];
            if a.dead || !a.facts.by_player_collide || clock - a.last_shot < rules.repeat.max(0.3) || p.queue.iter().any(|s| s.asset == a.id) {
                continue;
            }
            let near = a.points.iter().filter(|q| p.live(q)).find(|q| {
                let d = q.er - sora_er;
                touches(&rules, speed, Vec3::new(d.x, 0.0, d.z).length() * CM_PER_M, d.y * CM_PER_M)
            });
            if let (Some(q), true) = (near.copied(), p.queue.len() < 16) {
                p.queue.push_back(Shot { asset: a.id, point: q.er, normal: q.normal, filter: q.filter, wanted: 1, attribute: 0, attack: "(touched at a run)", at: clock });
            }
        }
    }

    // ---- what became of the objects shot at
    for k in 0..p.assets.len() {
        let Some(v) = p.assets[k].verify.filter(|v| p.clock >= v.due) else { continue };
        p.assets[k].verify = None;
        let owner = p.assets[k].owner;
        let mut hits = std::mem::take(&mut p.scratch);
        let mut there = false;
        for layer in [v.filter, filters[0], filters[1]] {
            havok::cast_sphere_of(v.from, v.dir * v.length, 0.15, layer, false, Some(OWNER_GEOM), &mut hits);
            there |= hits.iter().any(|h| h.owner_ptr == owner);
        }
        p.scratch = hits;
        let give_up = rules.give_up;
        let a = &mut p.assets[k];
        if there {
            a.fails += 1;
            p.tallies[v.variant].still += 1;
            let left = a.fails >= give_up;
            if left {
                a.dead = true;
                a.points.clear();
                *p.ignored_models.entry(model_of(&a.name).to_string()).or_insert(0) += 1;
            }
            let (name, fails, f, obj) = (a.name.clone(), a.fails, a.facts, p.obj);
            if p.may_log() || left {
                erlog!("break: {name} is STILL WHOLE 0.25 s after the bullet of '{}' (variant {}: {}; atkObj {obj} against hp {} defense {}; {fails} bullet(s) so far){}", v.attack, v.variant, variant_text(v.variant), f.hp, f.defense, if left { ": left alone from now on" } else { "" });
            }
        } else {
            a.dead = true;
            a.points.clear();
            p.tallies[v.variant].broke += 1;
            let name = a.name.clone();
            // (always written: this is the line that says the route works)
            erlog!("break: {name} BROKE (its body is gone 0.25 s after the bullet of '{}', variant {}: {}); so far broken by variant 0: {}, by variant 1: {}", v.attack, v.variant, variant_text(v.variant), p.tallies[0].broke, p.tallies[1].broke);
        }
        let (broke, still) = (p.tallies.iter().map(|t| t.broke).sum::<u32>(), p.tallies.iter().map(|t| t.still).sum::<u32>());
        if broke == 0 && still >= rules.give_up_total {
            p.gave_up = true;
            erlog!("break: {still} bullets at objects and NOTHING BROKE: the bullet route does not break objects as it is. The feature is quiet until Sora starts again (send this log; break.variant / break.obj are the keys to try)");
            return;
        }
    }

    // ---- the tally, every few seconds while bullets go out
    if p.clock >= p.next_summary && p.shots != p.summary_shots {
        p.next_summary = p.clock + 5.0;
        p.summary_shots = p.shots;
        erlog!("break: so far {} breakable object(s) seen, {} bullet(s) at objects | variant 0 (stopped by the map): {} broke, {} still whole | variant 1 (passes through): {} broke, {} still whole | {} model(s) left alone", p.seen, p.shots, p.tallies[0].broke, p.tallies[0].still, p.tallies[1].broke, p.tallies[1].still, p.ignored_models.values().filter(|n| **n >= 3).count());
    }

    // ---- the next bullet, in its turn
    while p.queue.front().is_some_and(|s| p.clock - s.at > 0.5) {
        p.queue.pop_front();
    }
    let Some(shot) = p.queue.front().copied() else { return };
    let Some(k) = p.assets.iter().position(|a| a.id == shot.asset && !a.dead) else {
        p.queue.pop_front();
        return;
    };
    let variant = choose_variant(p.forced_variant, &p.tallies, p.assets[k].shots);
    let (carrier, sp_attribute) = p.element_of(shot.attribute);
    let attack = Attack { phys: shot.wanted.clamp(1, 60000) as u16, carrier, sp_attribute, obj: p.object_attack(), ..Attack::PLAIN };
    let (from, dir) = aim_prop(shot.point, shot.normal, p.sora, t.shape.back);
    let length = (shot.point - from).length() + 0.6;
    let shape = Shape { life: t.shape.life.max(length / t.shape.speed.max(1.0)), penetrate: variant == 1, ..t.shape };
    match t.fire_prop(from, dir, attack, shape) {
        // (not its turn: an enemy's bullet went out this frame, or no row pair is free)
        None => {}
        Some(result) => {
            let p = &mut t.props;
            p.queue.pop_front();
            let clock = p.clock;
            let a = &mut p.assets[k];
            a.last_shot = clock;
            match result {
                Ok(row) => {
                    a.shots += 1;
                    p.shots += 1;
                    a.verify = Some(Verify { due: clock + 0.25, from: from - dir * 0.3, dir, length: length + 0.3, filter: shot.filter, variant, attack: shot.attack });
                    let name = a.name.clone();
                    if p.may_log() {
                        erlog!("break: '{}' (KH3 attribute {}) hit {name}: a bullet (row {row}, variant {variant}: {}) of attack {} as {carrier:?}, spAttribute {sp_attribute:?}, atkObj {:?}, from ({:.2}, {:.2}, {:.2}) along ({:.2}, {:.2})", shot.attack, shot.attribute, variant_text(variant), attack.phys, attack.obj, from.x, from.y, from.z, dir.x, dir.z);
                    }
                }
                Err(why) => {
                    let name = a.name.clone();
                    if p.may_log() {
                        erlog!("break: the bullet of '{}' at {name} could not be made ({why})", shot.attack);
                    }
                }
            }
        }
    }
}

fn variant_text(variant: usize) -> &'static str {
    if variant == 1 { "passes through the map" } else { "stopped by the map, like an arrow" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_the_defaults_and_fire_is_fire() {
        let r = BreakRules::DEFAULT;
        // KH3 fire -> fire, blizzard -> magic, thunder -> lightning; physical, water, aero, dark: the row as before
        assert_eq!([r.carrier(0), r.carrier(1), r.carrier(2), r.carrier(3), r.carrier(4), r.carrier(5), r.carrier(6), r.carrier(99)], [Carrier::Shape, Carrier::Fire, Carrier::Magic, Carrier::Lightning, Carrier::Shape, Carrier::Shape, Carrier::Shape, Carrier::Shape]);
        let mut changed = r;
        assert!(changed.apply_row("damage.element.water", 1.0) && changed.carrier(4) == Carrier::Magic);
        assert!(changed.apply_row("break.reach", 400.0) && changed.reach == 400.0);
        assert!(!changed.apply_row("damage.element.physical", 2.0), "physical is never moved");
        assert!(!changed.apply_row("damage.element.fire", 9.0) && !changed.apply_row("break.nothing", 1.0) && !changed.apply_row("break.reach", 5.0));
    }

    /// The total a hit asks of the game is the same whatever type carries it (the user's `damage.out.scale` stays what it is).
    #[test]
    fn an_element_changes_the_type_never_the_total() {
        let r = BreakRules::DEFAULT;
        for attribute in 0..8u8 {
            let attack = Attack { phys: 1234, carrier: r.carrier(attribute), ..Attack::PLAIN };
            let parts = attack.split(0);
            assert_eq!(parts.iter().map(|v| *v as u32).sum::<u32>(), 1234, "attribute {attribute}");
            assert_eq!(parts.iter().filter(|v| **v > 0).count(), 1);
        }
        assert_eq!(Attack { phys: 50, carrier: r.carrier(1), ..Attack::PLAIN }.split(0), [0, 0, 50, 0, 0]);
    }

    #[test]
    fn the_object_attack_comes_from_the_games_rows() {
        let mut r = BreakRules::DEFAULT;
        let survey = Survey { obj_max: 320, obj_common: 50, ..Survey::default() };
        assert_eq!(r.object_attack(&survey).0, 320);
        assert_eq!(r.object_attack(&Survey::default()).0, r.atk_obj_floor);
        r.atk_obj = 75;
        assert_eq!(r.object_attack(&survey).0, 75);
    }

    #[test]
    fn what_can_break() {
        let r = BreakRules::DEFAULT;
        let f = Facts { hp: -1, defense: 0, by_player_collide: false, burn: false, break_bullet: -1, break_attribute: 0, burn_bullet: -1, lot: -1 };
        assert!(!f.breakable(&r), "-1: the game's 'cannot break'");
        assert!(Facts { hp: 0, ..f }.breakable(&r) && Facts { hp: 30, ..f }.breakable(&r) && Facts { burn: true, ..f }.breakable(&r));
        assert_eq!((model_of("AEG027_038_0500"), model_of("AEG099_001"), model_of("h002000")), ("AEG027_038", "AEG099_001", "h002000"));
        // the row's fields are read where the SDK has them
        // SAFETY: a parameter row is plain numbers.
        let mut row: ASSET_GEOMETORY_PARAM_ST = unsafe { std::mem::zeroed() };
        row.set_hp(12);
        row.set_defense(3);
        row.set_is_break_by_player_collide(true);
        row.set_break_bullet_behavior_id(4400);
        let facts = Facts::of(&row);
        assert_eq!((facts.hp, facts.defense, facts.by_player_collide, facts.burn, facts.break_bullet), (12, 3, true, false, 4400));
    }

    #[test]
    fn the_bullet_flies_level_into_the_surface() {
        // a crate's side facing -X (normal out = -X): the bullet starts 0.35 m in front and flies +X
        let (from, dir) = aim_prop(Vec3::new(10.0, 1.0, 5.0), Vec3::NEG_X, Vec3::new(8.0, 0.0, 5.0), 0.35);
        assert!((dir - Vec3::X).length() < 1e-5 && (from - Vec3::new(9.65, 1.0, 5.0)).length() < 1e-5);
        // the top of a pot (normal up): from Sora's side, level, at the point's height
        // (10 cm under the top, from where he is, at most 1.5 m away)
        let (from, dir) = aim_prop(Vec3::new(10.0, 0.6, 5.0), Vec3::Y, Vec3::new(10.0, 0.9, 3.0), 0.35);
        assert!((dir - Vec3::Z).length() < 1e-5 && (from - Vec3::new(10.0, 0.5, 3.5)).length() < 1e-5, "{from:?}");
        let (from, _) = aim_prop(Vec3::new(10.0, 0.6, 5.0), Vec3::Y, Vec3::new(10.0, 0.9, 4.9), 0.35);
        assert!((from - Vec3::new(10.0, 0.5, 4.65)).length() < 1e-5, "never nearer than the bullet's run-up: {from:?}");
    }

    #[test]
    fn the_variant_that_broke_something_is_kept() {
        let none = [Tally::default(); VARIANTS];
        assert_eq!((choose_variant(None, &none, 0), choose_variant(None, &none, 1), choose_variant(None, &none, 2)), (0, 1, 0), "in turn while nothing broke");
        let one = [Tally { broke: 0, still: 3 }, Tally { broke: 2, still: 0 }];
        assert_eq!((choose_variant(None, &one, 0), choose_variant(None, &one, 1)), (1, 1));
        assert_eq!(choose_variant(None, &[Tally { broke: 2, still: 0 }, Tally { broke: 2, still: 0 }], 1), 0, "a tie: the arrow-like one");
        assert_eq!((choose_variant(Some(0), &one, 0), choose_variant(Some(7), &one, 0)), (0, 1));
    }

    #[test]
    fn the_touch_rule() {
        let r = BreakRules::DEFAULT;
        assert!(touches(&r, 400.0, 40.0, 30.0));
        assert!(!touches(&r, 100.0, 40.0, 30.0), "walking");
        assert!(!touches(&r, 400.0, 90.0, 30.0), "not at it");
        assert!(!touches(&r, 400.0, 40.0, 180.0), "above it");
        assert!(!touches(&BreakRules { collide: false, ..r }, 400.0, 40.0, 30.0));
    }

    #[test]
    fn a_prop_is_hit_but_never_locked_on_to() {
        let mut p = Props { on: true, ..Props::default() };
        p.assets.push(Asset { id: PROP_BASE, owner: 0x1000, name: "AEG099_001_9000".into(), facts: Facts { hp: 1, defense: 0, by_player_collide: true, burn: false, break_bullet: -1, break_attribute: 0, burn_bullet: -1, lot: -1 }, points: vec![Point { er: Vec3::new(2.0, 1.0, 0.0), normal: Vec3::NEG_X, filter: 1, at: 0.0 }], last_shot: -100.0, shots: 0, fails: 0, verify: None, dead: false, logged: false });
        let mut out = Vec::new();
        p.enumerate(&mut out);
        assert_eq!(out.len(), 1);
        let t = out[0];
        assert!(is_prop(t.id) && !is_prop(TargetId(77)));
        assert!(t.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::GIMMICK) && !t.flags.has(TargetFlags::LOCKABLE) && !t.flags.has(TargetFlags::ENEMY));
        let mut shapes = Vec::new();
        p.hurt_shapes(t.id, &mut shapes);
        assert_eq!(shapes.len(), 1);
        assert!((shapes[0].radius - p.rules.hurt_radius).abs() < 1e-6 && shapes[0].a == shapes[0].b);
        // the points age out: a prop nobody has seen for a while is no target
        p.clock = 5.0;
        out.clear();
        p.enumerate(&mut out);
        assert!(out.is_empty() && p.info(TargetId(PROP_BASE)).is_none());
        // off: nothing, and the rows as before
        p.clock = 0.0;
        p.on = false;
        p.enumerate(&mut out);
        assert!(out.is_empty());
        assert_eq!((p.element_of(1), p.object_attack()), ((Carrier::Shape, None), None));
        p.on = true;
        p.elements = true;
        p.obj = 300;
        p.survey = Some(Survey { sp: [None, None, Some((2, 5, 6)), None, None], ..Survey::default() });
        assert_eq!((p.element_of(1), p.element_of(3), p.element_of(0), p.object_attack()), ((Carrier::Fire, Some(2)), (Carrier::Lightning, None), (Carrier::Shape, None), Some(300)));
    }
}
