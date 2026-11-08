//! (GOOFY) THE HITS OF GOOFY'S KIT: hit spheres at clip times (his SGuard clips carry no Attack window of their own), run by his
//! state on `FriendStatus`, so a pad and an AI Goofy hit alike. Every hit is GOOFY'S: `deliver` builds the hit record from a row
//! with his own Strength and abilities (kh-core's damage function), the victim classification (`classify`) and the row's power.

use super::goofy::{g, gp};
use crate::goofy::K;
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
    /// A fixed damage instead of the formula (a batted shot: the bullet's own number).
    pub flat: Option<f32>,
    /// `HitRecord::flags` it carries (a Breaker's defeat).
    pub flags: u32,
}

impl SoloRow {
    /// A row with no attack row behind it: power 1 of `kind`.
    pub fn plain(name: &'static str, kind: Kind) -> SoloRow {
        SoloRow { name, power: 1.0, kind, attribute: Attr::Physical, power_scale: 1.0, parabola_angle: 0.0, move_length: 0.0, brake: 0.0, effect_time: 0.0, reaction_power: 0, armor_attack: 0, magic: false, invalid_guard: false, flat: None, flags: 0 }
    }

    /// A row of Sora's attack data (`p_ex001_AttackData`, sora-combat-attack-data).
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
            flat: None,
            flags: 0,
        })
    }

    /// One of Goofy's own rows (`party-attacks`: Attack1_Push .. Attack8_Discus).
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
            flat: None,
            flags: 0,
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

/// A clip of his set by name.
pub fn clip(name: &str) -> &'static crate::anim::ClipDef {
    crate::tables::clip(Chara::Goofy, name)
}

/// A copy of a clip without some notify classes / its root motion (cached, leaked once: a handful per run).
pub fn clip_copy(name: &'static str, drop_attack: bool, drop_effects: bool, drop_root: bool) -> &'static crate::anim::ClipDef {
    use std::sync::Mutex;
    static CLIPS: Mutex<Vec<(&'static str, u8, &'static crate::anim::ClipDef)>> = Mutex::new(Vec::new());
    let key = drop_attack as u8 | (drop_effects as u8) << 1 | (drop_root as u8) << 2;
    let mut all = CLIPS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = all.iter().find(|c| c.0 == name && c.1 == key) {
        return c.2;
    }
    let base = clip(name);
    let notifies: Vec<crate::anim::ClipNotify> = base
        .notifies
        .iter()
        .copied()
        .filter(|n| !(drop_attack && (matches!(n.data, crate::anim::ClipNotifyData::Attack { .. }) || n.class.contains("Traction"))))
        .filter(|n| !(drop_effects && n.class.contains("AttachEffect")))
        .collect();
    let notifies: &'static [crate::anim::ClipNotify] = Box::leak(notifies.into_boxed_slice());
    let c: &'static crate::anim::ClipDef = Box::leak(Box::new(crate::anim::ClipDef { notifies, root_motion: base.root_motion && !drop_root, root: if drop_root { &[] } else { base.root }, ..*base }));
    all.push((name, key, c));
    c
}

/// One hit of the kit on `target`. `dir` = the knock direction (horizontal; zero = from him to the target). Returns the HP
/// dealt when the host accepted it.
pub fn deliver(p: &mut NpcPawn, row: &SoloRow, target: TargetId, part: u32, at: Vec3, dir: Vec3, host: &mut PartyHost) -> Option<f32> {
    deliver_seen(p, row, target, part, at, dir, host).map(|l| l.dealt)
}

/// (THE USER 2026-11-02: shields guard, the attacker bounces off) What a delivered hit was: the HP dealt, KH3's rules guarded
/// it (the victim result GUARD, or the host said so), the attacker's result (REFLECT: he is repelled, repel.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landed {
    pub dealt: f32,
    pub guarded: bool,
    pub attacker: kh_core::host::ReactionResult,
}

/// `deliver`, telling what the classification made of the hit (`Landed`).
pub fn deliver_seen(p: &mut NpcPawn, row: &SoloRow, target: TargetId, part: u32, at: Vec3, dir: Vec3, host: &mut PartyHost) -> Option<Landed> {
    let defender = Defender::of_target(host.targets.defense(target).as_ref());
    let magic = row.magic || row.attribute != Attr::Physical;
    let attack = if magic { DamageAttack::magic(row.power, row.attribute) } else { DamageAttack::physical(row.power) };
    let damage = row.flat.unwrap_or_else(|| compute_damage(&p.status.as_attacker(), &defender, &attack, &DamageContext::new(p.status.game_level, 1)).amount);
    let mut hit = HitRecord::empty(target);
    hit.part = part;
    hit.attack_name = row.name;
    hit.position = at;
    let d = horizontal(dir).normalize_or_zero();
    let away = host.targets.info(target).map(|i| horizontal(i.position - p.position()).normalize_or_zero()).unwrap_or(Vec3::ZERO);
    hit.direction = if d != Vec3::ZERO { d } else if away != Vec3::ZERO { away } else { forward(p.yaw) };
    hit.damage = damage;
    hit.damage_kind = row.kind;
    hit.flags |= row.flags;
    hit.attribute = row.attribute;
    hit.attacker = HitAttacker { position: p.capsule_centre(), yaw: p.yaw, radius: p.params.capsule_radius, valid: true };
    let mut info = HitInfo { power: row.power, power_scale: row.power_scale, parabola_angle: row.parabola_angle, move_length: row.move_length, brake: row.brake, effect_time: row.effect_time, reaction_power: row.reaction_power, armor_attack_power: row.armor_attack, ..HitInfo::DEFAULT };
    info.flags50 |= ((magic as u8) * HitInfo::FLAG50_MAGIC) | ((row.invalid_guard as u8) * HitInfo::FLAG50_IGNORE_GUARD);
    info.clear_bad_status();
    hit.info = info;
    // Classify (0x14061FE20) with the victim's body, as B's deliver does; (THE USER 2026-11-02) the body as he meets it (a
    // host's shield guards only from inside its arc: kh-core `TargetBody::seen_from`)
    let body = host.targets.body(target, part).seen_from(p.position(), host.targets.info(target).map_or(at, |i| i.position));
    let shape = kh_core::combat::classify::Shape { no_clash: true, ..Default::default() };
    let (mut armor, mut endurance) = (None, None);
    let mut info = hit.info;
    let mut c = kh_core::combat::classify::classify(&body, kh_core::combat::classify::MapResponse::Other, &shape, kh_core::combat::classify::DamageClass { kind: hit.damage_kind, attribute: hit.attribute }, &mut info, &mut |pw, weak| {
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
        c = kh_core::combat::classify::classify(&body, kh_core::combat::classify::MapResponse::Other, &shape, kh_core::combat::classify::DamageClass { kind: hit.damage_kind, attribute: hit.attribute }, &mut info, &mut |_, _| armor_left.unwrap_or(0), &mut |_| endurance_left.unwrap_or(0));
    }
    hit.info = info;
    hit.flags |= p.hit_flag(); // (PARTY-AI) an AI's shot passes through friendly NPCs
    let res = host.targets.apply_hit(&hit);
    if !res.accepted {
        return None;
    }
    p.hits_this_action += 1;
    p.events.push(PartyEventKind::Hit { target, damage: res.damage_dealt, killed: res.killed, attack: row.name });
    let guarded = res.guarded || matches!(c.victim, kh_core::host::ReactionResult::Guard | kh_core::host::ReactionResult::GuardReflect);
    Some(Landed { dealt: res.damage_dealt, guarded, attacker: c.attacker })
}

/// Sora's rule for CHOOSING a target (kh-core combat/targeting.rs: ALIVE | LOCKABLE): breakable pots, friendly NPCs, merchants
/// and the party's own ally bodies are HITTABLE but never LOCKABLE.
pub fn lockable(t: &TargetInfo) -> bool {
    t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE)
}

/// The enemies the kit may CHOOSE: alive and lockable (`lockable`).
pub fn enemies(host: &PartyHost) -> Vec<TargetInfo> {
    let mut list = Vec::new();
    host.targets.enumerate(&mut list);
    list.retain(lockable);
    list
}

/// Everything the kit's hit spheres may TOUCH (HITTABLE): a swing still breaks a pot it hits.
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

/// His shield's face, as the hosts' party taps name it (the weapon's own socket vfx_call): where KH3's AttachEffect rows
/// put the guard barrier VN_bsh_inl0 (SGuardStart: SQEX_ATTACH_OBJECT_ATTACH_TO_WEAPON, vfx_call).
pub const SHIELD_SOCKET: &str = "wep:vfx_call";

/// A one-shot effect ON his shield, following it while it plays, whatever the clip's pose (a fixed point ahead of his
/// capsule is off the shield in most poses). (THE USER 2026-11-02: "this effect is not aligned properly")
pub fn shield_spawn(p: &NpcPawn, name: &'static str) -> EffectSpawn {
    let at = Attach::Party { member: kh_core::host::PartyMember(p.id.0), bone: Some(SHIELD_SOCKET), offset: Vec3::ZERO };
    EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at, rotation: Vec3::ZERO, scale: 1.0, persistent: false }
}

pub fn effect_on_shield(p: &mut NpcPawn, host: &mut PartyHost, name: &'static str) -> u32 {
    if name.is_empty() {
        return 0;
    }
    let spawn = shield_spawn(p, name);
    p.seam.spawn(host.effects, &spawn).0
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

/// (COMBAT-FIX) The first hurt shape of `t` one of KH3's attack shapes touches (the test of `actions::hit_test`).
pub fn touch_shape(host: &PartyHost, t: &TargetInfo, shape: &crate::actions::AtkShape) -> Option<HurtShape> {
    let mut shapes = Vec::new();
    host.targets.hurt_shapes(t.id, &mut shapes);
    shapes.into_iter().find(|s| match shape.prism {
        Some(pr) => pr.touches(s.a, s.b, s.radius),
        None => crate::actions::seg_seg_dist(shape.a, shape.b, s.a, s.b) <= shape.radius + s.radius,
    })
}

/// How a shot pushes what it hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Push {
    /// Away from him (the default).
    #[default]
    Away,
    /// Toward him.
    Inward,
    /// Along his facing.
    Forward,
}

/// A hit sphere that lives for a while (grows, may follow him).
#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub row: SoloRow,
    pub centre: Vec3,
    /// Follows him: the centre is his feet + this (his frame: x forward, y right, z up).
    pub follow: Option<Vec3>,
    /// Seconds lived; below zero it has not arrived yet.
    pub age: f32,
    pub life: f32,
    /// Sphere: size at the shot, growth per second, the cap (0 = none).
    pub size: (f32, f32, f32),
    pub hit: Vec<TargetId>,
    pub hit_effect: &'static str,
    pub hit_se: &'static str,
    pub push: Push,
    /// What made it (tests, the landed-hit rules).
    pub tag: &'static str,
    /// The Stagger it adds to what it hits.
    pub stagger: f32,
    /// (COMBAT-FIX) KH3's own shape instead of the sphere: the collision group of his weapon (`actions::attack_shape`:
    /// Attack_Tornado's prism, Attack_Body's capsule), rebuilt on him each step.
    pub group: Option<&'static str>,
}

impl Area {
    pub fn new(row: SoloRow, centre: Vec3, life: f32, radius: f32, tag: &'static str) -> Area {
        Area { row, centre, follow: None, age: 0.0, life, size: (radius, 0.0, 0.0), hit: Vec::new(), hit_effect: "", hit_se: "", push: Push::Away, tag, stagger: 0.0, group: None }
    }

    /// The radius now (size + inc x age, capped).
    pub fn radius(&self) -> f32 {
        let v = self.size.0 + self.size.1 * self.age.max(0.0);
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
    if g(p).shots.is_empty() {
        return;
    }
    let mut areas = std::mem::take(&mut gp(p).shots);
    let list = hittables(host); // contact: anything hittable (pots break), choices use `enemies`
    for a in &mut areas {
        if let Some(off) = a.follow {
            a.centre = local(p, off);
        }
        // (COMBAT-FIX) KH3's own shape of a group area, rebuilt on him
        let shape = a.group.map(|grp| crate::actions::attack_shape(p, grp));
        if let Some(s) = shape {
            a.centre = (s.a + s.b) * 0.5;
        }
        let r = a.radius();
        if r > 0.0 && a.age >= 0.0 {
            for t in &list {
                if a.hit.contains(&t.id) {
                    continue;
                }
                let touched = match &shape {
                    Some(s) => touch_shape(host, t, s),
                    None => touch(host, t, a.centre, r),
                };
                let Some(s) = touched else { continue };
                a.hit.push(t.id);
                let dir = match a.push {
                    Push::Away => horizontal(t.position - p.position()),
                    Push::Inward => horizontal(p.position() - t.position),
                    Push::Forward => forward(p.yaw),
                };
                let at = (s.a + s.b) * 0.5;
                let landed = deliver_seen(p, &a.row, t.id, s.part, at, dir, host);
                // (THE USER 2026-11-02: "Shields don't count as guarding ... make the character who attacks a shield bounce
                // off") a guarded hit: KH3's guard spark (it carries its own clang), none of the shape's hit effect / sound, no
                // landed hit for the combo or Stagger; a shape riding his body is his own swing: it bounces him
                // (repel.rs, one path for every member)
                if let Some(l) = landed.filter(|l| l.guarded) {
                    effect(p, host, kh_core::combat::hit::EFFECT_GUARD_REFLECT, at, 1.0);
                    if a.follow.is_some() {
                        let _ = crate::repel::after_swing(p, l.attacker, t.position);
                    }
                    continue;
                }
                if landed.is_some() {
                    effect(p, host, a.hit_effect, at, 1.0);
                    sound(p, host, a.hit_se, at);
                    super::goofy::on_area_hit(p, a, t.id);
                }
            }
        }
        a.age += dt;
    }
    areas.retain(|a| a.age < a.life);
    // areas added while these ran are kept behind them
    areas.append(&mut gp(p).shots);
    gp(p).shots = areas;
}

/// Puts an area in.
pub fn add(p: &mut NpcPawn, a: Area) {
    gp(p).shots.push(a);
}

/// The kit's sound and effect names, for the pack check.
pub fn cue_names() -> (Vec<&'static str>, Vec<&'static str>) {
    let mut fx = vec![K.guard_block_effect, K.deflect_effect, K.deflect_spark, K.parry_right_effect, K.combo_hit_effect, K.breaker_effect, K.stagger_break_effect, K.batted_effect];
    let mut se = vec![K.guard_block_se, K.deflect_se, K.parry_right_se, K.guard_break_se, K.combo_bash_swing_se, K.step_shove1_hit_se, K.step_shove2_hit_se, K.step_shove3_hit_se, K.step_bonk_hit_se, K.breaker_se, K.stagger_break_se, K.batted_se];
    // (GF-ARTS) the Shield Arts' cues
    fx.extend([K.art_charge_trail, K.art_charge_hit_effect, K.art_tornado_hit_effect, K.art_bash_hit_effect, K.art_turbo_hit_effect, K.art_interpose_trail, K.art_toss_hit_effect]);
    se.extend([K.art_charge_se, K.art_charge_hit_se, K.art_tornado_hit_se, K.art_bash_hit_se, K.art_turbo_hit_se, K.art_taunt_se, K.art_interpose_se, K.art_toss_hit_se, K.art_toss_catch_se]);
    (fx, se)
}
