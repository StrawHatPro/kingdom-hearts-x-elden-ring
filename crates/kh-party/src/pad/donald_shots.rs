//! (DON-KIT) DONALD'S MAGIC MISSILES AND THE KIT'S AREAS (documentation/DONALD_OVERHAUL.md 3, 4.2, 4.6, 5.1, 9; plan
//! build/sora_tmp/party/DON_PLAN.md 3 / 4.2). The user (2026-10-22): "Okay, implement the entire doc. He needs to play
//! just as the doc specifies". KH3 has no playable Donald: the RULES are the doc's (numbers the doc leaves open are sheet
//! rows marked `inferred`), the PIECES are KH3's (Flare Force's missile, his own element bolts, Sora's hit effects / SEs,
//! his staff clips).
//!
//! - A [`Missile`] flies at `missile.speed` and homes on its target with a CAPPED turn rate (a sidestep dodges it), for
//!   `missile.range`, then fades. Its damage is `missile.power` x its share (the slam's 0.75, Missile Volley's 0.5, the
//!   Charged Missile's 2.5 ...) x the range rule (x0.6 under 6 m) x Spell Weaver x the reaction's multiplier.
//! - Its element (his attunement; with Dual Attunement the missiles alternate, `DonCore::missile_element`) changes it:
//!   Fire splashes + Burns (and ignites the ground at full power), Blizzard Chills, Thunder chains (3 targets at full
//!   power), Water pierces + Wets, Aero curves round cover and launches on its third hit (`aero_missile_hit`), Light heals
//!   the party members it passes and Reveals.
//! - EVERY HIT goes through `Party::don_hit` (the reactions, the statuses, Exposed) and `DonCore::on_missile_hit` (the
//!   small Resonance gain); a hit while his MP charges shortens the charge (`mpcharge.missile_shorten`, 4.4).
//! - [`Area`]: the element slams (Thunder's bolt, Water's wave, Aero's tornado, Light's pulse), the Flame Step trail and
//!   the burning ground, the Ice Glide's touch, the Ward Step barrier.
//!
//! The missiles and areas live in the pad (`DonaldKit::shots`); the pawn spawns them ([`launch`], [`add_area`]) and the
//! PARTY steps them ([`party_step`], from `Party::step` with the host): only there are `Party::don_hit` and the other
//! members (heals, the ward) reachable. Frame-rate independent: everything moves and ticks by the seconds passed.

use super::donald::K;
use crate::donald::{data as don_data, Apply, ElemSet, Element, HitTag, Size};
use crate::host::PartyHost;
use crate::ids::MemberId;
use crate::pawn::{forward, horizontal, NpcPawn};
use crate::team::proj::{ProjKind, TeamProj};
use crate::view::{PartyEventKind, SoraView};
use glam::Vec3;
use kh_core::enums::{ETresDamageAttribute as Attr, ETresDamageKind as Kind};
use kh_core::host::{Attach, EffectHandle, EffectId, EffectSpawn, HitAttacker, HitInfo, HitRecord, HurtShape, SoundId, SoundPlay, TargetFlags, TargetId, TargetInfo};
use kh_core::status::{compute_damage, DamageAttack, DamageContext, Defender};

// ================================================================================================ the hit rows

/// A row of Sora's attack data as the kit hits with it (scaled).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoloRow {
    pub name: &'static str,
    pub power: f32,
    pub kind: Kind,
    pub attribute: Attr,
    pub power_scale: f32,
    pub parabola_angle: f32,
    pub move_length: f32,
    pub brake: f32,
    pub effect_time: f32,
    pub knockback: &'static str,
    pub bad_status: &'static str,
    pub bad_rate: f32,
    pub bad_time: f32,
    pub bad_param: f32,
    pub reaction_power: i32,
    pub armor_attack: i32,
    pub form_point: i32,
    pub revenge_point: f32,
    pub phys_force: &'static str,
    pub magic: bool,
    pub invalid_guard: bool,
    pub rapid_fire: bool,
    pub killer: bool,
    pub reflect_level: i8,
    pub reflect_reaction: &'static str,
}

impl SoloRow {
    /// The row `name` of `p_ex001_AttackData`: the combat sheet's (UNISON_*, FR_*) or the magic sheet's (MAG_*).
    pub fn sora(name: &'static str) -> Option<SoloRow> {
        if let Some((_, r)) = kh_core::combat::tables::attack_row(name) {
            return Some(SoloRow {
                name: r.id,
                power: r.power,
                kind: r.damage_kind,
                attribute: r.attribute,
                power_scale: r.power_scale,
                parabola_angle: r.parabola_angle,
                move_length: r.move_length,
                brake: r.brake,
                effect_time: r.effect_time,
                knockback: r.knockback,
                bad_status: r.bad_status,
                bad_rate: r.bad_gen_rate,
                bad_time: r.bad_time,
                bad_param: r.bad_param,
                reaction_power: r.reaction_power,
                armor_attack: r.armor_attack,
                form_point: r.form_point,
                revenge_point: r.revenge_point,
                phys_force: r.phys_force,
                magic: r.magic,
                invalid_guard: r.invalid_guard,
                rapid_fire: r.rapid_fire,
                killer: r.killer,
                reflect_level: r.reflect_level as i8,
                reflect_reaction: r.reflect_reaction,
            });
        }
        let r = kh_core::magic::spells::attack_row(name)?;
        Some(SoloRow {
            name: r.name,
            power: r.power,
            kind: r.kind,
            attribute: r.attribute,
            power_scale: r.power_scale,
            parabola_angle: r.parabola_angle,
            move_length: 0.0,
            brake: 0.0,
            effect_time: r.effect_time,
            knockback: "NORMAL",
            bad_status: r.bad_status,
            bad_rate: r.bad_rate,
            bad_time: r.bad_time,
            bad_param: 0.0,
            reaction_power: r.reaction_power,
            armor_attack: r.armor_attack,
            form_point: r.form_point,
            revenge_point: r.revenge_point,
            phys_force: r.phys_force,
            magic: r.magic,
            invalid_guard: r.invalid_guard,
            rapid_fire: r.rapid_fire,
            killer: r.killer,
            reflect_level: 0,
            reflect_reaction: "DEFAULT",
        })
    }

    pub fn scaled(mut self, m: f32) -> SoloRow {
        self.power *= m;
        self
    }

    /// A plain row (no KH3 row of that name).
    pub fn plain(name: &'static str, power: f32, kind: Kind, magic: bool) -> SoloRow {
        SoloRow { name, power, kind, attribute: Attr::Physical, power_scale: 1.0, parabola_angle: 0.0, move_length: 0.0, brake: 0.0, effect_time: 0.0, knockback: "NORMAL", bad_status: "NONE", bad_rate: 0.0, bad_time: 0.0, bad_param: 0.0, reaction_power: 1, armor_attack: 1, form_point: 0, revenge_point: 0.0, phys_force: "LV_1", magic, invalid_guard: false, rapid_fire: false, killer: true, reflect_level: 0, reflect_reaction: "DEFAULT" }
    }
}

/// The damage kind of a sheet cell ("SMALL", "BLOW" ...).
pub fn kind_of(name: &str) -> Kind {
    Kind::from_game_name(name).unwrap_or(Kind::Small)
}

/// The hit attribute of an element (Light and the plain missile: no element, KH3's NOTYPE).
pub fn attr_of(e: Option<Element>) -> Attr {
    match e {
        Some(Element::Fire) => Attr::Fire,
        Some(Element::Blizzard) => Attr::Blizzard,
        Some(Element::Thunder) => Attr::Thunder,
        Some(Element::Water) => Attr::Water,
        Some(Element::Aero) => Attr::Aero,
        Some(Element::Light) | None => Attr::Notype,
    }
}

/// THE MISSILE'S ROW: Flare Force's missile row (shape, reaction power) with the missile's power, kind and element.
pub fn missile_row(power: f32, kind: Kind, e: Option<Element>) -> SoloRow {
    let base = SoloRow::sora(K.missile_row).unwrap_or(SoloRow::plain("DON_MISSILE", 1.0, kind, true));
    SoloRow { power, kind, attribute: attr_of(e), magic: true, bad_status: "NONE", bad_rate: 0.0, ..base }
}

/// xorshift32 (the kit's own stream; the engine's is a process global).
pub fn next(rng: &mut u32) -> u32 {
    let mut x = (*rng).max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *rng = x;
    x
}

/// One hit of the kit on `target`: the HitRecord from the row with Donald's own Strength / Magic (the friends' damage
/// function), the bad-status roll of the shared collision, the victim classification. Returns the HP dealt when the host
/// accepted it.
#[allow(clippy::too_many_arguments)]
pub fn deliver(p: &mut NpcPawn, row: &SoloRow, target: TargetId, part: u32, at: Vec3, dir: Vec3, finisher: bool, host: &mut PartyHost, rng: &mut u32) -> Option<f32> {
    let defender = Defender::of_target(host.targets.defense(target).as_ref());
    let magic = row.magic || row.attribute != Attr::Physical;
    let attack = if magic { DamageAttack::magic(row.power, row.attribute) } else { DamageAttack::physical(row.power) };
    let damage = compute_damage(&p.status.as_attacker(), &defender, &attack, &DamageContext::new(p.status.game_level, 1)).amount;
    let mut hit = HitRecord::empty(target);
    hit.part = part;
    hit.attack_name = row.name;
    hit.position = at;
    let d = horizontal(dir).normalize_or_zero();
    hit.direction = if d == Vec3::ZERO { forward(p.yaw) } else { d };
    hit.damage = damage;
    hit.damage_kind = row.kind;
    hit.attribute = row.attribute;
    hit.attacker = HitAttacker { position: p.capsule_centre(), yaw: p.yaw, radius: p.params.capsule_radius, valid: true };
    let mut info = HitInfo {
        power: row.power,
        power_scale: row.power_scale,
        parabola_angle: row.parabola_angle,
        move_length: row.move_length,
        brake: row.brake,
        effect_time: row.effect_time,
        knockback: row.knockback,
        bad_status: row.bad_status,
        bad_time: row.bad_time,
        bad_param: row.bad_param,
        reaction_power: row.reaction_power,
        armor_attack_power: row.armor_attack,
        form_point: row.form_point,
        revenge_point: row.revenge_point,
        phys_force: row.phys_force,
        reflect_level: row.reflect_level,
        reflect_reaction: row.reflect_reaction,
        ..HitInfo::DEFAULT
    };
    info.flags50 |= (magic as u8) | (row.killer as u8) << 2 | (row.invalid_guard as u8) << 3 | (row.rapid_fire as u8) << 4;
    if finisher {
        info.flags50 |= HitInfo::FLAG50_FINISH;
    }
    if row.bad_status != "NONE" && row.bad_status != "TRES_BADSTAT_NONE" && row.bad_rate < 100.0 {
        let roll = (next(rng) >> 8) as f32 * kh_core::combat::tables::P.hit_bad_status_roll_scale;
        if row.bad_rate <= 0.0 || roll > row.bad_rate {
            info.clear_bad_status();
        }
    } else if row.bad_status == "NONE" || row.bad_status == "TRES_BADSTAT_NONE" {
        info.clear_bad_status();
    }
    hit.info = info;
    let body = host.targets.body(target, part);
    let shape = kh_core::combat::classify::Shape { no_clash: true, ..Default::default() };
    let (mut armor, mut endurance) = (None, None);
    let mut info = hit.info;
    kh_core::combat::classify::classify(&body, kh_core::combat::classify::MapResponse::Other, &shape, hit.damage_kind, hit.attribute, &mut info, &mut |pw, weak| {
        armor = Some((pw, weak));
        i32::MIN
    }, &mut |pw| {
        endurance = Some(pw);
        i32::MIN
    });
    if armor.is_some() || endurance.is_some() {
        let armor_left = armor.map(|(pw, weak)| host.targets.take_armor(target, pw, weak));
        let endurance_left = endurance.map(|pw| host.targets.take_reaction_power(target, pw));
        info = hit.info;
        kh_core::combat::classify::classify(&body, kh_core::combat::classify::MapResponse::Other, &shape, hit.damage_kind, hit.attribute, &mut info, &mut |_, _| armor_left.unwrap_or(0), &mut |_| endurance_left.unwrap_or(0));
    }
    hit.info = info;
    let res = host.targets.apply_hit(&hit);
    if !res.accepted {
        return None;
    }
    p.events.push(PartyEventKind::Hit { target, damage: res.damage_dealt, killed: res.killed, attack: row.name });
    Some(res.damage_dealt)
}

/// Sora's targeting rule (ALIVE | LOCKABLE): what the missiles CHOOSE (home on, chain to). Pots / friendly NPCs never.
pub fn lockable(t: &TargetInfo) -> bool {
    t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE)
}

/// What the kit's shapes may HIT (a pot it touches still breaks).
pub fn hittables(host: &PartyHost) -> Vec<TargetInfo> {
    let mut list = Vec::new();
    host.targets.enumerate(&mut list);
    list.retain(|t| t.flags.has(TargetFlags::HITTABLE));
    list
}

/// The enemies the kit may choose (homing, chains, the crowd rule).
pub fn enemies(host: &PartyHost) -> Vec<TargetInfo> {
    let mut list = Vec::new();
    host.targets.enumerate(&mut list);
    list.retain(lockable);
    list
}

/// HOW BIG AN ENEMY IS (Freeze's and the launches' rule, 4.2 / 6.1): by its bounds and max HP (sheet `size.*`).
pub fn size_of(t: &TargetInfo) -> Size {
    if t.hp_max > K.size_boss_hp {
        Size::Boss
    } else if t.half_height > K.size_large_height {
        Size::Large
    } else if t.half_height > K.size_medium_height {
        Size::Medium
    } else {
        Size::Small
    }
}

/// The statuses a missile of `e` applies (4.2); `double`: the Charged Missile's doubled effect (3.3).
pub fn applies_of(e: Option<Element>, double: bool) -> Vec<Apply> {
    let n = if double { 2 } else { 1 };
    match e {
        Some(Element::Fire) => vec![Apply::Burn],
        Some(Element::Blizzard) => vec![Apply::Chill(n)],
        Some(Element::Water) => vec![Apply::Wet],
        Some(Element::Light) => vec![Apply::Reveal],
        _ => Vec::new(),
    }
}

/// (KITFX) the Water missile's tint (its foam and droplets water-blue, never white; the foam is HDR-bright, so a pale tint
/// still clamps to white: the blue is strong).
pub const WATER_FOAM: kh_core::host::Tint = kh_core::host::Tint::rgb(0.12, 0.42, 1.0);

fn effect_at(p: &mut NpcPawn, host: &mut PartyHost, name: &'static str, at: Vec3, rotation: Vec3, scale: f32, persistent: bool) -> u32 {
    if name.is_empty() || name == "-" {
        return 0;
    }
    let h = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), rotation, scale, persistent }).0;
    // (KITFX) the Water missile's body and splash in water blue: KH3's white foam alone read as snow next to the ice it replaced
    if name == K.missile_water_body || name == K.missile_water_hit_fx {
        host.effects.tint(EffectHandle(h), WATER_FOAM);
    }
    // (AUD) the effects that are silent alone in the pack (Sora's strikes, the strobe-free copies ...) sound as the spells' do
    if let Some(se) = crate::donald::magic_fx::fx::extra_se(name) {
        p.seam.play(host.audio, &kh_core::host::SoundPlay { id: kh_core::host::SoundId(kh_core::notify::notify_asset_id(se)), name: se, at: Attach::World(at), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
    }
    h
}

/// A one-shot effect in the world.
pub fn effect(p: &mut NpcPawn, host: &mut PartyHost, name: &'static str, at: Vec3, scale: f32) -> u32 {
    let r = Vec3::new(0.0, p.yaw, 0.0);
    effect_at(p, host, name, at, r, scale, false)
}

/// A one-shot sound in the world.
pub fn sound(p: &mut NpcPawn, host: &mut PartyHost, name: &'static str, at: Vec3) {
    if name.is_empty() || name == "-" {
        return;
    }
    p.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
}

fn rotation_of(d: Vec3) -> Vec3 {
    let d = d.normalize_or_zero();
    Vec3::new(d.z.clamp(-1.0, 1.0).asin().to_degrees(), d.y.atan2(d.x).to_degrees(), 0.0)
}

// ================================================================================================ the missiles

/// What fired a missile (its rules, the logs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissileKind {
    /// A ground combo step (waves, Combo Plus steps).
    Step,
    /// The plain slam's / the Fire slam's / Missile Volley's missiles.
    Slam,
    /// Arcane Bolt / the Blizzard slam's ice lance (one big piercing missile).
    Bolt,
    /// The air combo.
    Air,
    /// Starfall's rain.
    Starfall,
    /// Hold A: the Charged Missile.
    Charged,
    /// A perfect Magic Block's returned projectile.
    Reflect,
}

impl MissileKind {
    /// The hit's row name (the hosts' logs, the tests).
    pub fn row_name(self) -> &'static str {
        match self {
            MissileKind::Step => "DON_MISSILE",
            MissileKind::Slam => "DON_SLAM",
            MissileKind::Bolt => "DON_SLAM_BOLT",
            MissileKind::Air => "DON_AIR_MISSILE",
            MissileKind::Starfall => "DON_STARFALL",
            MissileKind::Charged => "DON_CHARGED",
            MissileKind::Reflect => "DON_REFLECT",
        }
    }

    pub fn finisher(self) -> bool {
        matches!(self, MissileKind::Slam | MissileKind::Bolt | MissileKind::Starfall)
    }
}

/// What a missile is launched with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissileSpec {
    pub kind: MissileKind,
    pub element: Option<Element>,
    /// Its share of one plain missile's damage.
    pub mult: f32,
    pub origin: Vec3,
    pub dir: Vec3,
    pub target: TargetId,
    /// Where it flies without a target (else the target's centre).
    pub aim: Option<Vec3>,
    /// Extra enemies it flies through (Water, Arcane Bolt, the lance, the Charged Missile).
    pub pierce: u8,
    /// The Charged Missile: Shatters Frozen whatever its element (R24, `HitTag::breaker`).
    pub breaker: bool,
    /// The attunement effect doubled (the Charged Missile, 3.3).
    pub double: bool,
    pub scale: f32,
    /// Spell Weaver (9): launched right after a spell.
    pub weaver: bool,
    /// Its body instead of the element's (the ice lance).
    pub body: Option<&'static str>,
    pub kind_dmg: Kind,
}

impl MissileSpec {
    pub fn new(kind: MissileKind, element: Option<Element>, origin: Vec3, dir: Vec3, target: TargetId) -> MissileSpec {
        MissileSpec { kind, element, mult: 1.0, origin, dir, target, aim: None, pierce: if element == Some(Element::Water) { K.missile_water_pierce as u8 } else { 0 }, breaker: false, double: false, scale: 1.0, weaver: false, body: None, kind_dmg: kind_of(K.missile_kind) }
    }
}

/// One missile in flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Missile {
    pub spec: MissileSpec,
    pub position: Vec3,
    pub velocity: Vec3,
    pub travelled: f32,
    pub age: f32,
    /// Fading out (seconds left; the range is over or it hit), None = flying.
    pub fading: Option<f32>,
    pub pierce_left: u8,
    pub hit: Vec<TargetId>,
    /// The members a Light missile healed.
    pub healed: Vec<u32>,
    /// The body effect (persistent), 0 = none.
    pub effect: u32,
    /// The plain missile's mesh (Flare Force's o_ex208_0 through the hosts' team props).
    pub proj: Option<TeamProj>,
    pub alive: bool,
}

impl Missile {
    pub fn turn_cap(&self) -> f32 {
        if self.spec.element == Some(Element::Aero) { K.missile_aero_turn } else { K.missile_turn }
    }
}

/// THE HOMING RULE (pure): the new velocity after `dt` toward `aim`, turning by at most `turn_deg_s` x dt.
pub fn home(velocity: Vec3, from: Vec3, aim: Vec3, turn_deg_s: f32, dt: f32) -> Vec3 {
    let want = aim - from;
    if want.length_squared() < 1.0 || velocity.length_squared() < 1e-6 {
        return velocity;
    }
    let max = (turn_deg_s.max(0.0) * dt).to_radians().min(std::f32::consts::PI);
    kh_core::magic::projectile::steer(velocity, want, max, -1.0)
}

/// THE RANGE RULE (3.1): full damage from 6 m, x0.6 under 6 m (`dist` = Donald to the target).
pub fn range_mult(dist: f32) -> f32 {
    if dist < K.missile_near {
        K.missile_near_mult
    } else {
        1.0
    }
}

// ================================================================================================ the areas

/// What an area does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaKind {
    /// A hit on each enemy inside (once, or every `interval`).
    Hit,
    /// Heals the party members inside, once (Light's slam pulse).
    Heal,
    /// The Ward Step barrier: a party member inside has one hit absorbed (once per member).
    Ward,
}

/// A shape on the ground (slams, trails, the burning ground, the barrier).
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub kind: AreaKind,
    pub centre: Vec3,
    pub radius: f32,
    /// Seconds before it acts (Thunder's bolt from above lands a little later).
    pub delay: f32,
    pub life: f32,
    pub age: f32,
    /// MV of its hit (x Magic).
    pub power: f32,
    pub element: Option<Element>,
    pub kind_dmg: Kind,
    /// Re-hit every this (0 = each enemy once).
    pub interval: f32,
    /// A cone: (his yaw, half angle deg) (the Water wave).
    pub arc: Option<(f32, f32)>,
    /// It follows Donald (the Ice Glide).
    pub follow: bool,
    /// Knock enemies back this far (cm, the Water wave).
    pub push: f32,
    /// Launches (Aero's tornado).
    pub launch: bool,
    /// A physical hit (the Wand Bonk: Strength), else magic.
    pub physical: bool,
    /// Heal share (% max HP).
    pub heal_pct: f32,
    pub hit: Vec<(u32, f32)>,
    pub hit_fx: &'static str,
    pub hit_se: &'static str,
    /// Its look (persistent while it lives), and the handle.
    pub fx: &'static str,
    pub fx_handle: u32,
    pub tag: &'static str,
    /// Its number in the bag (the ward's per-member use).
    pub serial: u32,
}

impl Area {
    pub fn new(kind: AreaKind, centre: Vec3, radius: f32, life: f32, tag: &'static str) -> Area {
        Area { kind, centre, radius, delay: 0.0, life, age: 0.0, power: 0.0, element: None, kind_dmg: Kind::Small, interval: 0.0, arc: None, follow: false, push: 0.0, launch: false, physical: false, heal_pct: 0.0, hit: Vec::new(), hit_fx: "", hit_se: "", fx: "", fx_handle: 0, tag, serial: 0 }
    }

    /// Inside (a vertical cylinder of the radius, 4 m tall around the centre; the arc when it has one).
    pub fn contains(&self, at: Vec3, reach: f32) -> bool {
        let d = horizontal(at - self.centre);
        if d.length() > self.radius + reach || (at.z - self.centre.z).abs() > 400.0 {
            return false;
        }
        match self.arc {
            Some((yaw, half)) if d.length() > 1.0 => crate::pawn::wrap180(crate::pawn::yaw_of(d) - yaw).abs() <= half,
            _ => true,
        }
    }

    fn active(&self) -> bool {
        self.age >= self.delay
    }
}

// ================================================================================================ the bag

/// Everything the kit has released and not finished (`DonaldKit::shots`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Shots {
    pub missiles: Vec<Missile>,
    pub areas: Vec<Area>,
    /// Missiles launched (Dual's alternation count, the logs).
    pub fired: u32,
    /// Hits of the last party step: (row name, element, damage) (the kit's log, the tests).
    pub landed: Vec<(&'static str, Option<Element>, i32)>,
    /// Reactions landed by the kit's hits (the log, the tests).
    pub reactions: Vec<crate::donald::Reaction>,
    pub rng: u32,
    /// The last area number given.
    pub serial: u32,
}

impl Shots {
    pub fn is_empty(&self) -> bool {
        self.missiles.is_empty() && self.areas.is_empty()
    }

    /// (effect handle, position, forward) of every live body effect: the hosts make them follow.
    pub fn taps(&self) -> Vec<(u32, Vec3, Vec3)> {
        self.missiles.iter().filter(|m| m.alive && m.effect != 0).map(|m| (m.effect, m.position, m.velocity.normalize_or_zero())).collect()
    }

    /// The plain missiles' meshes (Flare Force's), as stream D's `TeamProj` for the hosts' team props.
    pub fn props(&self) -> Vec<TeamProj> {
        self.missiles.iter().filter(|m| m.alive && m.fading.is_none()).filter_map(|m| {
            let mut pr = m.proj.clone()?;
            pr.position = m.position;
            pr.forward = m.velocity.normalize_or_zero();
            Some(pr)
        }).collect()
    }

    /// Stops every body effect (the pad let go, the member left).
    pub fn stop_all(&mut self, effects: &mut dyn kh_core::host::Effects) {
        for m in &mut self.missiles {
            if m.effect != 0 {
                effects.stop(EffectHandle(m.effect), false);
                m.effect = 0;
            }
        }
        for a in &mut self.areas {
            if a.fx_handle != 0 {
                effects.stop(EffectHandle(a.fx_handle), false);
                a.fx_handle = 0;
            }
        }
        *self = Shots { rng: self.rng, serial: self.serial, ..Shots::default() };
    }
}

fn shots_mut(p: &mut NpcPawn) -> Option<&mut Shots> {
    p.pad.as_mut().map(|d| &mut d.donald.shots)
}

/// THE BODY OF A MISSILE: the plain one is Flare Force's missile (its class body + mesh), the elemental ones his element
/// bolts (sheet `missile.<element>.body`).
pub fn body_of(e: Option<Element>) -> &'static str {
    match e {
        None => K.missile_body,
        Some(Element::Fire) => K.missile_fire_body,
        Some(Element::Blizzard) => K.missile_blizzard_body,
        Some(Element::Thunder) => K.missile_thunder_body,
        Some(Element::Water) => K.missile_water_body,
        Some(Element::Aero) => K.missile_aero_body,
        Some(Element::Light) => K.missile_light_body,
    }
}

/// (KITFX) The launch sound of a missile of `e` ("-": its body carries its own, or none in KH3: the plain Flare Force missile).
pub fn launch_se(e: Option<Element>) -> &'static str {
    match e {
        Some(Element::Thunder) => K.missile_thunder_launch_se,
        Some(Element::Aero) => K.missile_aero_launch_se,
        Some(Element::Light) => K.missile_light_launch_se,
        _ => "-",
    }
}

/// The hit effect and sound of a missile of `e`.
pub fn hit_look(e: Option<Element>) -> (&'static str, &'static str) {
    match e {
        None => (K.missile_hit_fx, K.missile_hit_se),
        Some(Element::Fire) => (K.missile_fire_hit_fx, K.missile_fire_hit_se),
        Some(Element::Blizzard) => (K.missile_blizzard_hit_fx, K.missile_blizzard_hit_se),
        Some(Element::Thunder) => (K.missile_thunder_hit_fx, K.missile_thunder_hit_se),
        Some(Element::Water) => (K.missile_water_hit_fx, K.missile_water_hit_se),
        Some(Element::Aero) => (K.missile_aero_hit_fx, K.missile_aero_hit_se),
        Some(Element::Light) => (K.missile_light_hit_fx, K.missile_light_hit_se),
    }
}

/// LAUNCHES A MISSILE from the pawn: its body (persistent, followed by the hosts), the flash at the wand.
pub fn launch(p: &mut NpcPawn, host: &mut PartyHost, spec: MissileSpec) {
    let dir = spec.dir.normalize_or_zero();
    let dir = if dir == Vec3::ZERO { forward(p.yaw) } else { dir };
    let body = spec.body.unwrap_or_else(|| body_of(spec.element));
    let effect = effect_at(p, host, body, spec.origin, rotation_of(dir), spec.scale, true);
    effect_at(p, host, K.missile_fire_fx, spec.origin, rotation_of(dir), spec.scale.min(1.5), false);
    // (KITFX) the launch sound of the elements whose body carries none (Thunder's bolt, Aero's wind, Light's orb; Fire,
    // Blizzard and Water bodies carry their KH3 flight loop): sheet `missile.<element>.launch_se`
    if spec.body.is_none() {
        let se = launch_se(spec.element);
        if !se.is_empty() && se != "-" {
            p.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(se)), name: se, at: Attach::World(spec.origin), volume: K.missile_launch_volume, pitch: 1.0, looping: false, spatial: true });
        }
    }
    let proj = (spec.element.is_none() && spec.body.is_none()).then(|| {
        let mut pr = TeamProj::new(ProjKind::Missile, 0, spec.origin, dir);
        pr.visible = true;
        pr.flying = true;
        pr.scale = spec.scale;
        pr
    });
    p.events.push(PartyEventKind::ProjectileSpawned { kind: body, at: spec.origin });
    let m = Missile { spec: MissileSpec { dir, ..spec }, position: spec.origin, velocity: dir * K.missile_speed, travelled: 0.0, age: 0.0, fading: None, pierce_left: spec.pierce, hit: Vec::new(), healed: Vec::new(), effect, proj, alive: true };
    if let Some(s) = shots_mut(p) {
        s.fired = s.fired.wrapping_add(1);
        s.missiles.push(m);
    }
}

/// Adds an area (its look spawned now when it has one).
pub fn add_area(p: &mut NpcPawn, host: &mut PartyHost, mut a: Area) {
    if !a.fx.is_empty() && a.fx != "-" {
        let persistent = a.life > 0.6;
        let h = effect_at(p, host, a.fx, a.centre, Vec3::new(0.0, p.yaw, 0.0), 1.0, persistent);
        if let Some(t) = crate::donald::magic_fx::fx::kit_tint(a.tag) {
            host.effects.tint(EffectHandle(h), t); // (DON-MAGIC tint) [R] Ward Step gold, the Aero slam green
        }
        if persistent {
            a.fx_handle = h;
        }
    }
    if let Some(s) = shots_mut(p) {
        s.serial = s.serial.wrapping_add(1).max(1);
        a.serial = s.serial;
        s.areas.push(a);
    }
}

// ================================================================================================ the party step

/// THE PARTY'S STEP OF EVERY PAD DONALD'S MISSILES AND AREAS (`Party::step`, after the members, with the host): the
/// flights, the hits through `Party::don_hit`, the heals, the ward barrier.
pub fn party_step(party: &mut crate::party::Party, dt: f32, sora: &SoraView, host: &mut PartyHost) {
    let ids: Vec<MemberId> = party.members().collect();
    // the ward barrier: who is inside one (cleared every step, set by the barriers below)
    for id in ids.iter().copied() {
        if let Some(m) = party.member_mut(id) {
            m.don_ward_offer = 0; // (DON-KIT)
        }
    }
    for id in ids.iter().copied() {
        let Some(m) = party.member_mut(id) else { continue };
        if m.chara != crate::ids::Chara::Donald || m.pad.is_none() {
            continue;
        }
        let Some(mut bag) = m.pad.as_mut().map(|d| std::mem::take(&mut d.donald.shots)) else { continue };
        bag.landed.clear();
        bag.reactions.clear();
        step_missiles(party, id, &mut bag, dt, sora, host);
        step_areas(party, id, &mut bag, dt, sora, host);
        if let Some(d) = party.member_mut(id).and_then(|m| m.pad.as_mut()) {
            // anything launched during the step (none: launches are pawn-side) stays
            let mut newer = std::mem::take(&mut d.donald.shots);
            bag.missiles.append(&mut newer.missiles);
            bag.areas.append(&mut newer.areas);
            bag.fired = bag.fired.max(newer.fired);
            for r in bag.reactions.clone() {
                d.donald.note(super::donald::KitEvent::Reaction(r));
            }
            d.donald.shots = bag;
        }
    }
}

/// The members a heal / the ward reaches: (id, position) of every member, and AI Sora.
fn party_points(party: &crate::party::Party, sora: &SoraView) -> Vec<(MemberId, Vec3, bool)> {
    let mut out: Vec<(MemberId, Vec3, bool)> = party.members().filter_map(|id| party.member(id).map(|m| (id, m.capsule_centre(), true))).collect();
    if let Some((id, at)) = sora.ai_sora {
        out.push((id, at + Vec3::Z * 90.0, false));
    }
    out
}

/// Heals member `to` by `pct` % of his max HP (AI Sora through the host's heal request).
fn heal(party: &mut crate::party::Party, from: MemberId, to: MemberId, member: bool, pct: f32, sora: &SoraView) {
    if member {
        let amount = party.status(to).map_or(0, |s| ((s.max_hp() as f32 * pct / 100.0).round() as i32).max(1));
        party.heal(to, amount, 1);
        if let Some(m) = party.member_mut(from) {
            m.requests.push(crate::pawn::Request::HealEffect { member: to, kind: 1 });
        }
    } else {
        let max = sora.hp_max.max(1);
        let amount = ((max as f32 * pct / 100.0).round() as i32).max(1);
        if let Some(m) = party.member_mut(from) {
            m.requests.push(crate::pawn::Request::Heal { target: crate::command::TargetRef::Member(to), amount, effect: 1 });
        }
    }
}

/// One hit of the kit's (missile, splash, chain, area) on `t`: the reaction rule, the statuses, the damage, the meters.
#[allow(clippy::too_many_arguments)]
fn land(party: &mut crate::party::Party, id: MemberId, bag: &mut Shots, host: &mut PartyHost, t: &TargetInfo, part: u32, at: Vec3, dir: Vec3, element: Option<Element>, power: f32, kind: Kind, breaker: bool, double: bool, finisher: bool, missile: bool, push: f32, launch: bool, physical: bool, name: &'static str) -> Option<f32> {
    let d = don_data();
    let size = size_of(t);
    let tag = HitTag { elements: element.map(ElemSet::one).unwrap_or_default(), breaker };
    let applies = applies_of(element, double);
    let out = party.don_hit(Some(id), t.id.0, size, tag, &applies);
    bag.reactions.extend(out.reactions.iter().copied());
    // Aero: the third missile hit launches a small enemy (a medium one at full power), the doubled effect counts twice
    let mut kind = kind;
    let aero_full = party.status(id).and_then(|s| s.donald.as_ref()).is_some_and(|st| st.core.full_power(Element::Aero, &d));
    if launch && crate::donald::TargetStatus::new(size).launchable(aero_full) {
        kind = Kind::Toss;
    }
    if missile && element == Some(Element::Aero) {
        let n = if double { 2 } else { 1 };
        let mut up = false;
        for _ in 0..n {
            up |= party.don.status.entry(t.id.0, size).aero_missile_hit(aero_full, &d.tune);
        }
        if up {
            kind = Kind::Toss;
        }
    }
    if out.events.iter().any(|e| matches!(e, crate::donald::StatusEvent::Launch)) {
        kind = Kind::Toss;
    }
    let mut row = SoloRow { name, move_length: push, ..missile_row(power * out.damage_mult, kind, element) };
    if physical {
        (row.attribute, row.magic) = (Attr::Physical, false);
    }
    let mut rng = bag.rng.max(1);
    let dealt = {
        let m = party.member_mut(id)?;
        let dealt = deliver(m, &row, t.id, part, at, dir, finisher, host, &mut rng);
        if dealt.is_some() {
            let (fx, se) = hit_look(element);
            effect_at(m, host, fx, at, Vec3::ZERO, 1.0, false);
            sound(m, host, se, at);
        }
        dealt
    };
    bag.rng = rng;
    if let Some(dmg) = dealt {
        bag.landed.push((row.name, element, dmg as i32));
        if let Some(s) = party.status_mut(id) {
            if missile {
                if let Some(st) = s.donald.as_mut() {
                    st.core.on_missile_hit(element, &d);
                }
            }
            // 4.4: a missile landed while his MP charges shortens the charge
            if missile && s.mp_charge_left > 0.0 {
                s.mp_charge_left = (s.mp_charge_left - K.mpcharge_missile_shorten).max(1e-4);
            }
        }
    }
    dealt
}

fn hurt_touch(host: &PartyHost, t: &TargetInfo, a: Vec3, b: Vec3, r: f32) -> Option<HurtShape> {
    let mut hurt: Vec<HurtShape> = Vec::new();
    host.targets.hurt_shapes(t.id, &mut hurt);
    if hurt.is_empty() {
        return (crate::actions::seg_seg_dist(a, b, t.center - Vec3::Z * t.half_height.max(1.0) * 0.5, t.center + Vec3::Z * t.half_height.max(1.0) * 0.5) <= r + t.radius).then_some(HurtShape { a: t.center, b: t.center, radius: t.radius, part: 0 });
    }
    hurt.into_iter().find(|s| crate::actions::seg_seg_dist(a, b, s.a, s.b) <= s.radius + r)
}

fn step_missiles(party: &mut crate::party::Party, id: MemberId, bag: &mut Shots, dt: f32, sora: &SoraView, host: &mut PartyHost) {
    let d = don_data();
    let targets = hittables(host);
    let choose = enemies(host);
    let me = party.member(id).map_or(Vec3::ZERO, |m| m.position());
    let full = |party: &crate::party::Party, e: Element| party.status(id).and_then(|s| s.donald.as_ref()).is_some_and(|st| st.core.full_power(e, &d));
    let weaver_mult = if party.status(id).and_then(|s| s.donald.as_ref()).is_some_and(|st| st.abilities.has(crate::donald::DonAbility::SpellWeaver)) { K.weaver_mult } else { 1.0 };
    let points = party_points(party, sora);
    let mut missiles = std::mem::take(&mut bag.missiles);
    for m in &mut missiles {
        if !m.alive {
            continue;
        }
        if let Some(f) = m.fading.as_mut() {
            *f -= dt;
            if *f <= 0.0 {
                m.alive = false;
            }
            continue;
        }
        m.age += dt;
        let prev = m.position;
        // homing on its target (the capped turn: a sidestep dodges it)
        let tinfo = targets.iter().find(|t| t.id == m.spec.target && t.flags.has(TargetFlags::ALIVE)).copied();
        let mut aim = tinfo.map(|t| t.center).or(m.spec.aim);
        if m.spec.element == Some(Element::Aero) {
            if let Some(a) = aim {
                // curves round cover: with the line to the target blocked it aims over it
                if host.world.line_trace(prev, a).is_some() {
                    aim = Some(a + Vec3::Z * K.missile_aero_lift);
                }
            }
        }
        if m.age >= K.missile_homing_delay {
            if let Some(a) = aim {
                m.velocity = home(m.velocity, prev, a, m.turn_cap(), dt);
            }
        }
        let speed = m.velocity.length();
        m.position += m.velocity * dt;
        m.travelled += speed * dt;
        // the map (Aero curves round it: it is not stopped)
        if m.spec.element != Some(Element::Aero) {
            if let Some(h) = host.world.sweep_capsule(prev, m.position, K.missile_radius.min(25.0), K.missile_radius.min(25.0)).filter(|h| !h.start_penetrating) {
                m.position = prev + (m.position - prev) * h.time;
                m.fading = Some(K.missile_fade);
                if let Some(pm) = party.member_mut(id) {
                    let (fx, se) = hit_look(m.spec.element);
                    effect_at(pm, host, fx, m.position, Vec3::ZERO, 1.0, false);
                    sound(pm, host, se, m.position);
                }
                continue;
            }
        }
        // Light: the party members it passes are healed a little (once each)
        if m.spec.element == Some(Element::Light) {
            for (mid, at, member) in points.iter().copied() {
                if mid == id || m.healed.contains(&(mid.0 as u32)) {
                    continue;
                }
                if crate::actions::seg_seg_dist(prev, m.position, at, at) <= K.missile_light_heal {
                    m.healed.push(mid.0 as u32);
                    let pct = K.missile_light_heal_pct * if m.spec.double { 2.0 } else { 1.0 };
                    heal(party, id, mid, member, pct, sora);
                }
            }
        }
        // the targets it touches
        for t in &targets {
            if m.hit.contains(&t.id) || !t.flags.has(TargetFlags::ALIVE) {
                continue;
            }
            let Some(s) = hurt_touch(host, t, prev, m.position, K.missile_radius * m.spec.scale.max(1.0)) else { continue };
            m.hit.push(t.id);
            let dist = horizontal(t.position - me).length() - t.radius;
            let weaver = if m.spec.weaver { weaver_mult } else { 1.0 };
            let power = K.missile_power * m.spec.mult * range_mult(dist) * weaver;
            let at = m.position;
            let dir = m.velocity;
            let e = m.spec.element;
            land(party, id, bag, host, t, s.part, at, dir, e, power, m.spec.kind_dmg, m.spec.breaker, m.spec.double, m.spec.kind.finisher(), true, 0.0, false, false, m.spec.kind.row_name());
            // Fire: the splash (and the ground fire at full power)
            if e == Some(Element::Fire) {
                let r = K.missile_fire_splash * if m.spec.double { 2.0 } else { 1.0 };
                for o in &targets {
                    if o.id == t.id || !o.flags.has(TargetFlags::ALIVE) || m.hit.contains(&o.id) {
                        continue;
                    }
                    if let Some(os) = hurt_touch(host, o, at, at, r) {
                        land(party, id, bag, host, o, os.part, o.center, o.center - at, e, power * K.missile_fire_splash_mult, Kind::Small, false, m.spec.double, false, false, 0.0, false, false, "DON_SPLASH");
                    }
                }
                if full(party, Element::Fire) {
                    let mut a = Area::new(AreaKind::Hit, Vec3::new(at.x, at.y, t.position.z), K.missile_fire_ground_radius, K.missile_fire_ground_life, "DON_FIRE_GROUND");
                    (a.power, a.element, a.interval, a.fx) = (K.missile_fire_ground_power, Some(Element::Fire), K.missile_fire_ground_tick, K.missile_fire_ground_fx);
                    if let Some(pm) = party.member_mut(id) {
                        let h = effect_at(pm, host, a.fx, a.centre, Vec3::ZERO, 1.0, true);
                        a.fx_handle = h;
                    }
                    bag.serial = bag.serial.wrapping_add(1).max(1);
                    a.serial = bag.serial;
                    bag.areas.push(a);
                }
            }
            // Thunder: the chain to the next nearest enemies (3 targets at full power; one more with the doubled effect)
            if e == Some(Element::Thunder) {
                let n = if full(party, Element::Thunder) { K.missile_thunder_chain_targets_full } else { K.missile_thunder_chain_targets } as usize + m.spec.double as usize;
                let mut from = *t;
                let mut done: Vec<TargetId> = vec![t.id];
                for _ in 1..n {
                    let next = choose.iter().filter(|o| !done.contains(&o.id) && horizontal(o.position - from.position).length() <= K.missile_thunder_chain).min_by(|a, b| a.position.distance(from.position).total_cmp(&b.position.distance(from.position))).copied();
                    let Some(o) = next else { break };
                    done.push(o.id);
                    m.hit.push(o.id);
                    if let Some(pm) = party.member_mut(id) {
                        effect_at(pm, host, K.missile_thunder_chain_fx, o.center, Vec3::ZERO, 1.0, false);
                    }
                    land(party, id, bag, host, &o, 0, o.center, o.center - from.center, e, power * K.missile_thunder_chain_mult, Kind::Small, false, m.spec.double, false, true, 0.0, false, false, "DON_CHAIN");
                    from = o;
                }
            }
            if m.pierce_left > 0 {
                m.pierce_left -= 1;
            } else {
                m.fading = Some(K.missile_fade);
                break;
            }
        }
        if m.fading.is_none() && m.travelled >= K.missile_range {
            m.fading = Some(K.missile_fade);
        }
        if m.fading.is_some() && m.effect != 0 {
            host.effects.stop(EffectHandle(m.effect), false);
            m.effect = 0;
        }
    }
    for m in &mut missiles {
        if !m.alive && m.effect != 0 {
            host.effects.stop(EffectHandle(m.effect), false);
            m.effect = 0;
        }
    }
    missiles.retain(|m| m.alive);
    bag.missiles = missiles;
}

fn step_areas(party: &mut crate::party::Party, id: MemberId, bag: &mut Shots, dt: f32, sora: &SoraView, host: &mut PartyHost) {
    let targets = hittables(host);
    let me = party.member(id).map(|m| (m.position(), m.yaw));
    let points = party_points(party, sora);
    let mut areas = std::mem::take(&mut bag.areas);
    for a in &mut areas {
        if a.follow {
            if let Some((p, _)) = me {
                a.centre = p;
            }
        }
        let was_active = a.active();
        a.age += dt;
        if !a.active() {
            continue;
        }
        match a.kind {
            AreaKind::Hit => {
                if !was_active && a.delay > 0.0 && !a.fx.is_empty() {
                    if let Some(pm) = party.member_mut(id) {
                        effect_at(pm, host, a.fx, a.centre, Vec3::ZERO, 1.0, false);
                    }
                }
                for t in &targets {
                    if !t.flags.has(TargetFlags::ALIVE) {
                        continue;
                    }
                    let ready = a.hit.iter().find(|h| h.0 == t.id.0).is_none_or(|h| a.interval > 0.0 && a.age - h.1 >= a.interval);
                    if !ready || !a.contains(t.center, t.radius) {
                        continue;
                    }
                    a.hit.retain(|h| h.0 != t.id.0);
                    a.hit.push((t.id.0, a.age));
                    let dir = t.center - a.centre;
                    land(party, id, bag, host, t, 0, t.center, dir, a.element, a.power, a.kind_dmg, false, false, a.tag.starts_with("DON_SLAM"), false, a.push, a.launch, a.physical, a.tag);
                    if !a.hit_fx.is_empty() {
                        if let Some(pm) = party.member_mut(id) {
                            effect_at(pm, host, a.hit_fx, t.center, Vec3::ZERO, 1.0, false);
                            sound(pm, host, a.hit_se, t.center);
                        }
                    }
                }
            }
            AreaKind::Heal => {
                for (mid, at, member) in points.iter().copied() {
                    if a.hit.iter().any(|h| h.0 == mid.0 as u32) || !a.contains(at, 0.0) {
                        continue;
                    }
                    a.hit.push((mid.0 as u32, a.age));
                    heal(party, id, mid, member, a.heal_pct, sora);
                }
            }
            AreaKind::Ward => {
                for (mid, at, member) in points.iter().copied() {
                    if !member || a.hit.iter().any(|h| h.0 == mid.0 as u32) || !a.contains(at, 0.0) {
                        continue;
                    }
                    if let Some(m) = party.member_mut(mid) {
                        // the barrier offers its block; the member's next hit uses it (pad/donald.rs `on_hit`)
                        if m.don_ward_used == a.serial {
                            a.hit.push((mid.0 as u32, a.age));
                            continue;
                        }
                        m.don_ward_offer = a.serial;
                    }
                }
            }
        }
        if a.age >= a.life + a.delay && a.fx_handle != 0 {
            host.effects.stop(EffectHandle(a.fx_handle), false);
            a.fx_handle = 0;
        }
    }
    areas.retain(|a| a.age < a.life + a.delay);
    bag.areas = areas;
}
