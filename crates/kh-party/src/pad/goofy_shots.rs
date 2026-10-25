//! (party X3G) THE SOLO HITS OF GOOFY'S KIT: what his bashes, finishers, Bombardier Dive, the tier extras and Discus
//! Barrage deliver, run by the pad (no Sora, no link pawn). KH3's SGuard / GooF clips carry no Attack window of their own
//! (the link pawns and Sora's clips shot them): the hits are spheres at the clip times of the sheet (`party-kit-goofy`),
//! the shapes of stream D3 (ShieldGuardian / GoofyShoot shots) and D6b (TresProj_FusionSpin).
//!
//! Every hit is GOOFY'S: `deliver` builds the hit record from a row of Sora's attack data (`p_ex001_AttackData`:
//! FR_SHIELDGUARDIAN_*, FUSION_*, FR_GOOFYSHOOT_EXP) or of Goofy's own (FR_GOOFYSHOOT_RUSH, Attack6..8) with Goofy's
//! own Strength and abilities (kh-core's damage function), the victim classification (`classify`) and the row's power
//! scaled by the kit's multipliers.

use super::goofy_kit::K;
use crate::host::PartyHost;
use crate::ids::Chara;
use crate::pawn::{forward, horizontal, NpcPawn};
use crate::view::PartyEventKind;
use glam::Vec3;
use kh_core::enums::{ETresDamageAttribute as Attr, ETresDamageKind as Kind};
use kh_core::host::{Attach, EffectId, EffectSpawn, HitAttacker, HitInfo, HitRecord, HurtShape, SoundId, SoundPlay, TargetFlags, TargetId, TargetInfo};
use kh_core::status::{compute_damage, DamageAttack, DamageContext, Defender};

/// A row as the kit hits with it (scaled).
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
    pub reaction_power: i32,
    pub armor_attack: i32,
    pub magic: bool,
    pub invalid_guard: bool,
}

impl SoloRow {
    /// A row of Sora's attack data (`p_ex001_AttackData`, sora-combat-attack-data): FR_SHIELDGUARDIAN_*, FUSION_*, ...
    pub fn sora(name: &str) -> Option<SoloRow> {
        let (_, r) = kh_core::combat::tables::attack_row(name)?;
        Some(SoloRow {
            name: r.id,
            power: r.power,
            kind: r.damage_kind,
            attribute: r.attribute,
            power_scale: r.power_scale,
            parabola_angle: r.parabola_angle,
            move_length: r.move_length,
            brake: r.brake,
            effect_time: r.effect_time,
            reaction_power: r.reaction_power,
            armor_attack: r.armor_attack,
            magic: r.magic,
            invalid_guard: r.invalid_guard,
        })
    }

    /// One of Goofy's own rows (`party-attacks`: Attack1_Push .. Attack8_Discus, FR_GOOFYSHOOT_RUSH).
    pub fn goofy(name: &str) -> Option<SoloRow> {
        let r = crate::tables::attack_row(Chara::Goofy, name)?;
        Some(SoloRow {
            name: r.name,
            power: r.power,
            kind: Kind::from_game_name(r.damage_kind).unwrap_or(Kind::Small),
            attribute: Attr::from_game_name(r.attribute).unwrap_or(Attr::Physical),
            power_scale: r.power_scale,
            parabola_angle: r.parabola_angle,
            move_length: r.move_length,
            brake: r.brake,
            effect_time: r.effect_time,
            reaction_power: r.reaction_power,
            armor_attack: 0,
            magic: r.magic,
            invalid_guard: false,
        })
    }

    /// Sora's row of that name, else Goofy's.
    pub fn any(name: &str) -> Option<SoloRow> {
        SoloRow::sora(name).or_else(|| SoloRow::goofy(name))
    }

    pub fn scaled(mut self, m: f32) -> SoloRow {
        self.power *= m;
        self
    }

    pub fn with_kind(mut self, kind: Kind) -> SoloRow {
        self.kind = kind;
        self
    }
}

/// One hit of the kit on `target`. `dir` = the knock direction (horizontal; zero = from him to the target). Returns the
/// HP dealt when the host accepted it.
pub fn deliver(p: &mut NpcPawn, row: &SoloRow, target: TargetId, part: u32, at: Vec3, dir: Vec3, host: &mut PartyHost) -> Option<f32> {
    let defender = Defender::of_target(host.targets.defense(target).as_ref());
    let magic = row.magic || row.attribute != Attr::Physical;
    let attack = if magic { DamageAttack::magic(row.power, row.attribute) } else { DamageAttack::physical(row.power) };
    let damage = compute_damage(&p.status.as_attacker(), &defender, &attack, &DamageContext::new(p.status.game_level, 1)).amount;
    let mut hit = HitRecord::empty(target);
    hit.part = part;
    hit.attack_name = row.name;
    hit.position = at;
    let d = horizontal(dir).normalize_or_zero();
    let away = host.targets.info(target).map(|i| horizontal(i.position - p.position()).normalize_or_zero()).unwrap_or(Vec3::ZERO);
    hit.direction = if d != Vec3::ZERO { d } else if away != Vec3::ZERO { away } else { forward(p.yaw) };
    hit.damage = damage;
    hit.damage_kind = row.kind;
    hit.attribute = row.attribute;
    hit.attacker = HitAttacker { position: p.capsule_centre(), yaw: p.yaw, radius: p.params.capsule_radius, valid: true };
    let mut info = HitInfo { power: row.power, power_scale: row.power_scale, parabola_angle: row.parabola_angle, move_length: row.move_length, brake: row.brake, effect_time: row.effect_time, reaction_power: row.reaction_power, armor_attack_power: row.armor_attack, ..HitInfo::DEFAULT };
    info.flags50 |= (magic as u8) * HitInfo::FLAG50_MAGIC | (row.invalid_guard as u8) * HitInfo::FLAG50_IGNORE_GUARD;
    info.clear_bad_status();
    hit.info = info;
    // Classify (0x14061FE20) with the victim's body, as B's deliver does
    let body = host.targets.body(target, part);
    let shape = kh_core::combat::classify::Shape { no_clash: true, ..Default::default() };
    let (mut armor, mut endurance) = (None, None);
    let mut info = hit.info;
    let _ = kh_core::combat::classify::classify(&body, kh_core::combat::classify::MapResponse::Other, &shape, hit.damage_kind, hit.attribute, &mut info, &mut |pw, weak| {
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
    p.hits_this_action += 1;
    p.events.push(PartyEventKind::Hit { target, damage: res.damage_dealt, killed: res.killed, attack: row.name });
    super::goofy::on_hit_landed(p, res.damage_dealt);
    Some(res.damage_dealt)
}

/// (GOOFY-GK, after AUD's targeting fix) Sora's rule for CHOOSING a target (kh-core combat/targeting.rs: ALIVE |
/// LOCKABLE): Elden Ring's breakable pots, friendly NPCs, merchants and the party's own ally bodies are HITTABLE but never
/// LOCKABLE, so the Discus's bounces, Discus Barrage's locks and the finisher's crowd count never pick them.
pub fn lockable(t: &TargetInfo) -> bool {
    t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE)
}

/// The enemies the kit may CHOOSE (bounce to, lock, count as a crowd): alive and lockable (`lockable`).
pub fn enemies(host: &PartyHost) -> Vec<TargetInfo> {
    let mut list = Vec::new();
    host.targets.enumerate(&mut list);
    list.retain(lockable);
    list
}

/// (GOOFY-GK) Everything the kit's hit spheres may TOUCH (HITTABLE): a swing still breaks a pot it hits.
pub fn hittables(host: &PartyHost) -> Vec<TargetInfo> {
    let mut list = Vec::new();
    host.targets.enumerate(&mut list);
    list.retain(|t| t.flags.has(TargetFlags::HITTABLE));
    list
}

/// A one-shot effect at a point (seam handles of the member).
pub fn effect(p: &mut NpcPawn, host: &mut PartyHost, name: &'static str, at: Vec3, scale: f32) -> u32 {
    if name.is_empty() {
        return 0;
    }
    p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), rotation: Vec3::new(0.0, p.yaw, 0.0), scale, persistent: false }).0
}

pub fn sound(p: &mut NpcPawn, host: &mut PartyHost, name: &'static str, at: Vec3) {
    if name.is_empty() {
        return;
    }
    p.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
}

/// The first hurt shape of `t` a sphere touches.
pub fn touch(host: &PartyHost, t: &TargetInfo, centre: Vec3, radius: f32) -> Option<HurtShape> {
    let mut shapes = Vec::new();
    host.targets.hurt_shapes(t.id, &mut shapes);
    shapes.into_iter().find(|s| crate::actions::seg_seg_dist(centre, centre, s.a, s.b) <= radius + s.radius)
}

/// How a shot pushes what it hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Push {
    /// Away from him (the default).
    #[default]
    Away,
    /// Toward him (the Tornado's pull).
    Inward,
    /// Along his facing.
    Forward,
}

/// A hit sphere that lives for a while (grows, may follow him): the bashes' contact, Fusion Spin's actor, the
/// Bombardier explosion, the tier extras.
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub row: SoloRow,
    pub centre: Vec3,
    /// Follows him: the centre is his feet + this (his frame: x forward, y right, z up).
    pub follow: Option<Vec3>,
    pub age: f32,
    pub life: f32,
    /// Sphere: size at the shot, growth per second, the cap (0 = none).
    pub size: (f32, f32, f32),
    pub hit: Vec<TargetId>,
    pub hit_effect: &'static str,
    pub hit_se: &'static str,
    pub push: Push,
    /// What made it (tests, the taunt, Payback).
    pub tag: &'static str,
}

impl Area {
    pub fn new(row: SoloRow, centre: Vec3, life: f32, radius: f32, tag: &'static str) -> Area {
        Area { row, centre, follow: None, age: 0.0, life, size: (radius, 0.0, 0.0), hit: Vec::new(), hit_effect: "", hit_se: "", push: Push::Away, tag }
    }

    /// The radius now (size + inc x age, capped).
    pub fn radius(&self) -> f32 {
        let v = self.size.0 + self.size.1 * self.age;
        if self.size.2 > 0.0 { v.min(self.size.2) } else { v }
    }
}

/// His frame: x forward, y right, z up.
pub fn local(p: &NpcPawn, v: Vec3) -> Vec3 {
    let f = forward(p.yaw);
    let r = Vec3::new(-f.y, f.x, 0.0);
    p.position() + f * v.x + r * v.y + Vec3::Z * v.z
}

/// One step of the live areas: each target once per area.
pub fn step_areas(p: &mut NpcPawn, host: &mut PartyHost, dt: f32) {
    let Some(pad) = p.pad.as_mut() else { return };
    if pad.goofy.shots.is_empty() {
        return;
    }
    let mut areas = std::mem::take(&mut pad.goofy.shots);
    let list = hittables(host); // (GOOFY-GK) contact: anything hittable (pots break), choices use `enemies`
    for a in &mut areas {
        if let Some(off) = a.follow {
            a.centre = local(p, off);
        }
        let r = a.radius();
        if r > 0.0 {
            for t in &list {
                if a.hit.contains(&t.id) {
                    continue;
                }
                let Some(s) = touch(host, t, a.centre, r) else { continue };
                a.hit.push(t.id);
                let dir = match a.push {
                    Push::Away => horizontal(t.position - p.position()),
                    Push::Inward => horizontal(p.position() - t.position),
                    Push::Forward => forward(p.yaw),
                };
                let at = (s.a + s.b) * 0.5;
                if deliver(p, &a.row, t.id, s.part, at, dir, host).is_some() {
                    effect(p, host, a.hit_effect, at, 1.0);
                    sound(p, host, a.hit_se, at);
                    super::goofy::on_area_hit(p, a.tag, t.id);
                }
            }
        }
        a.age += dt;
    }
    areas.retain(|a| a.age < a.life);
    if let Some(pad) = p.pad.as_mut() {
        // areas added while these ran (none today) are kept behind them
        areas.append(&mut pad.goofy.shots);
        pad.goofy.shots = areas;
    }
}

/// Puts an area in.
pub fn add(p: &mut NpcPawn, a: Area) {
    if let Some(pad) = p.pad.as_mut() {
        pad.goofy.shots.push(a);
    }
}

/// The nearest hittable enemy to `from` not in `skip`, within `range`.
pub fn nearest(host: &PartyHost, from: Vec3, range: f32, skip: &[TargetId]) -> Option<TargetInfo> {
    enemies(host).into_iter().filter(|t| !skip.contains(&t.id) && t.position.distance(from) <= range).min_by(|a, b| a.position.distance(from).total_cmp(&b.position.distance(from)))
}

/// The bash rows by name, scaled (`K` names the rows).
pub fn row(name: &str, mult: f32) -> Option<SoloRow> {
    SoloRow::any(name).map(|r| r.scaled(mult))
}

/// The kit's sphere for a bash (D3's TeamShot shape).
pub fn bash_area(p: &NpcPawn, row: SoloRow, tag: &'static str) -> Area {
    let mut a = Area::new(row, local(p, Vec3::new(K.bash_forward, 0.0, K.bash_up)), 0.1, K.bash_radius, tag);
    a.follow = Some(Vec3::new(K.bash_forward, 0.0, K.bash_up));
    a.hit_effect = "";
    a
}
