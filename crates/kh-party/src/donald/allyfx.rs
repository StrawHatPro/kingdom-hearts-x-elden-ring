//! (DON-MAGIC) THE SPELLS WORN BY PARTY MEMBERS (documentation/DONALD_OVERHAUL.md 6.1, 11): Aero Barrier and Ball
//! Lightning, cast on Donald or ANY party member through the target list (KH1 style), and Sanctuary's short invincibility.
//!
//! - AERO BARRIER (KH1's Aero / Aerora / Aeroga; numbers R28 in the rows `l2.aero.*`): the wearer takes `power` less
//!   damage (-25 % / -40 % / -40 %) for `duration` (20 / 30 / 40 s); Lv 3 hurts enemies that touch it (`extra` power within
//!   `radius`, once per `barrier.contact_interval` per enemy). Recasting renews it; a member wears one barrier at a time.
//!   (DON-SPELLB) KH1's wind ribbons (`vd_don_aero1..3`, authored) wrap the wearer for the whole duration and fade out
//!   when it ends (time up, KO, the member gone); the HUD icon reads [`barrier_of`].
//! - BALL LIGHTNING (`l2.thunder.*`; the user's change of 2026-10-22: "balls of lightning that circle around ... They ONLY
//!   effect the enemy if they come into contact with the balls. The ball dissappears when it touches an enemy"; they orbit
//!   THE WEARER): `count` balls (3 / 4 / 5) orbit the wearer at body height (`orbs.orbit_radius`, `orbs.height`,
//!   `orbs.orbit_speed` of party-don-magic) for `duration` (5 / 12 / 20 s), each drawn as KH3's thunder glow crackling with
//!   KH3's thunder sparks and moved every step (`Effects::place`). A ball touches an enemy when the enemy's body (its
//!   capsule) overlaps the ball's own sphere (`radius` of the row, cm): that ball deals Thunder damage and a knockback (on a
//!   Wet enemy it Conducts, through the reaction rule) and is USED UP (gone), so all can be spent before the time runs out.
//!   No leap, no area: nothing else hits. Lv 2 recharges one used-up ball every `orbs.recharge`; Lv 3 arcs each touch to a
//!   second enemy nearby. Casting it again on the same member while balls remain hurls them all as homing shots at the
//!   lock-on target. One set per member. [`orbs_of`]: the set is active (for the HUD icon, DON-UI) while it has a ball and
//!   time left.
//! - The DAMAGE CUT: `Party::receive_hit` (party.rs, marked) multiplies a member's incoming hit by [`damage_mult`]; the
//!   hosts do the same for Sora's (kh-er sora.rs `damage_in`, marked) with [`SORA_MEMBER`].

use super::cast::SORA_MEMBER;
use super::element::{ElemSet, Element};
use super::magic_fx::{self, fx, magic, ActorKind, Aim, Burst, Ctx, HitSpec, MagicFx};
use super::react::HitTag;
use super::spells::SpellRef;
use super::tune::{data, DonSpellRow};
use crate::ids::MemberId;
use crate::party::Party;
use crate::view::SoraView;
use glam::Vec3;
use kh_core::enums::{ETresDamageAttribute as Attr, ETresDamageKind as Kind};
use kh_core::host::{Attach, EffectHandle, PartyMember, TargetFlags, TargetId};

/// A member's Aero Barrier as the HUD shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarrierView {
    pub level: u8,
    /// Seconds left / at the cast.
    pub left: f32,
    pub total: f32,
    /// The damage share it cuts (0.25 = -25 %).
    pub cut: f32,
}

/// A member's Ball Lightning set (the HUD shows an icon while `active`; the balls themselves show how many are left).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbsView {
    pub level: u8,
    /// Balls left (orbiting now).
    pub orbs: u8,
    /// A ball left and time left.
    pub active: bool,
    pub max: u8,
    pub left: f32,
    pub total: f32,
}

/// One barrier on a member.
#[derive(Clone, Debug, PartialEq)]
pub struct Barrier {
    pub member: MemberId,
    pub caster: MemberId,
    pub level: u8,
    pub left: f32,
    pub total: f32,
    pub cut: f32,
    /// Lv 3: the contact hit's power and reach.
    pub contact: Option<(f32, f32)>,
    /// (enemy, seconds until it can be hurt again).
    pub touched: Vec<(TargetId, f32)>,
    pub effect: Option<EffectHandle>,
}

/// One ball of a set: slot `k` of `max` sits at the orbit angle `spin + k x 360 / max`.
#[derive(Clone, Debug, PartialEq)]
pub struct Ball {
    /// Not used up yet.
    pub live: bool,
    /// Its glow core (moved every step).
    pub core: Option<EffectHandle>,
    /// Seconds until its next crackle.
    pub crackle: f32,
    /// Which crackle comes next.
    pub flip: bool,
}

/// One set of balls on a member.
#[derive(Clone, Debug, PartialEq)]
pub struct Orbs {
    pub member: MemberId,
    pub caster: MemberId,
    pub level: u8,
    /// Balls left.
    pub orbs: u8,
    pub max: u8,
    pub left: f32,
    pub total: f32,
    /// A ball's own contact sphere (cm, the row's `radius`).
    pub ball_radius: f32,
    pub power: f32,
    /// The orbit's angle now (deg).
    pub spin: f32,
    /// Lv 2+: seconds until the next used-up ball comes back.
    pub recharge_wait: f32,
    pub chain: bool,
    pub balls: Vec<Ball>,
}

impl Orbs {
    /// Where slot `k` is with the wearer at `at` (feet), cm.
    pub fn ball_at(&self, k: usize, at: Vec3) -> Vec3 {
        let m = magic();
        let a = (self.spin + k as f32 * 360.0 / self.max.max(1) as f32).to_radians();
        at + Vec3::new(a.cos(), a.sin(), 0.0) * m.orbs_orbit_radius + Vec3::Z * m.orbs_height
    }

    /// The live balls and where they are.
    pub fn live_balls(&self, at: Vec3) -> Vec<(usize, Vec3)> {
        (0..self.balls.len()).filter(|k| self.balls[*k].live).map(|k| (k, self.ball_at(k, at))).collect()
    }
}

/// A ball at `p` (contact sphere `r`) touches an enemy: the enemy's body (a vertical capsule round its centre: its
/// radius, its half height) overlaps the ball's sphere.
pub fn ball_touches(e: &kh_core::host::TargetInfo, p: Vec3, r: f32) -> bool {
    let half = (e.half_height - e.radius).max(0.0);
    let z = p.z.clamp(e.center.z - half, e.center.z + half);
    let axis = Vec3::new(e.center.x, e.center.y, z);
    (p - axis).length() <= r + e.radius.max(0.0)
}

/// Everything worn (`MagicFx::worn`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Worn {
    pub barriers: Vec<Barrier>,
    pub orbs: Vec<Orbs>,
    /// Sanctuary: (member, seconds of invincibility left).
    pub invincible: Vec<(MemberId, f32)>,
}

impl Worn {
    pub fn barrier(&self, m: MemberId) -> Option<&Barrier> {
        self.barriers.iter().find(|b| b.member == m)
    }

    pub fn orbs(&self, m: MemberId) -> Option<&Orbs> {
        self.orbs.iter().find(|o| o.member == m)
    }

    pub fn grant_invincible(&mut self, m: MemberId, secs: f32) {
        if secs <= 0.0 {
            return;
        }
        match self.invincible.iter_mut().find(|i| i.0 == m) {
            Some(i) => i.1 = i.1.max(secs),
            None => self.invincible.push((m, secs)),
        }
    }
}

/// The barrier `member` wears (None: none).
pub fn barrier_of(party: &Party, member: MemberId) -> Option<BarrierView> {
    party.don_fx.worn.barrier(member).map(|b| BarrierView { level: b.level, left: b.left, total: b.total, cut: b.cut })
}

/// The orbs circling `member` (None: none).
pub fn orbs_of(party: &Party, member: MemberId) -> Option<OrbsView> {
    party.don_fx.worn.orbs(member).map(|o| OrbsView { level: o.level, orbs: o.orbs, active: o.orbs > 0 && o.left > 0.0, max: o.max, left: o.left, total: o.total })
}

/// The share of an incoming hit `member` takes (1 - the barrier's cut; Sora = [`SORA_MEMBER`]).
pub fn damage_mult(party: &Party, member: MemberId) -> f32 {
    barrier_of(party, member).map_or(1.0, |b| (1.0 - b.cut).clamp(0.0, 1.0))
}

/// Sanctuary's invincibility is on for `member`.
pub fn invincible(party: &Party, member: MemberId) -> bool {
    party.don_fx.worn.invincible.iter().any(|i| i.0 == member && i.1 > 0.0)
}

/// Where a wearer stands (a member, or Sora / AI Sora).
pub fn wearer_at(party: &Party, sora: Option<&SoraView>, m: MemberId) -> Option<Vec3> {
    if m == SORA_MEMBER {
        return sora.map(|s| s.ai_sora.map_or(s.position, |a| a.1));
    }
    if let Some(s) = sora.and_then(|s| s.ai_sora).filter(|a| a.0 == m) {
        return Some(s.1);
    }
    party.member(m).filter(|p| !p.status.ko()).map(|p| p.position())
}

fn attach_on(m: MemberId, sora_is_member: bool, offset: Vec3) -> Attach {
    if m == SORA_MEMBER || sora_is_member {
        Attach::Sora { bone: None, offset }
    } else {
        Attach::Party { member: PartyMember(m.0), bone: None, offset }
    }
}

/// AERO BARRIER on `member` (recast renews; one per member).
pub fn put_barrier(fxw: &mut MagicFx, cx: &mut Ctx, caster: MemberId, member: MemberId, level: u8, row: &DonSpellRow, sora_is_member: bool) {
    let contact = (level >= 3 && row.extra > 0.0).then_some((row.extra, row.radius));
    let spell = SpellRef::Line2(Element::Aero, level);
    let at = attach_on(member, sora_is_member, Vec3::ZERO);
    // (DON-SPELLB) one worn spell per member: the barrier replaces Ball Lightning (its balls dissolve)
    end_orbs_of(fxw, cx, member);
    // (DON-SPELLB) the cast's gust on the wearer: Sora's Aero call (Donald's vn_ex001_air_han_00 is his whole KH3 Aero
    // tornado, no gust: no longer spawned here)
    let _ = magic_fx::spawn_fx(cx, caster, Some(spell), fx::BARRIER_GUST, at, barrier_scale(cx.party, member, sora_is_member), false);
    if let Some(b) = fxw.worn.barriers.iter_mut().find(|b| b.member == member) {
        // a recast renews (at the level cast now); a new level changes the ribbons (the old ones fade out)
        let new_look = b.level != level || b.effect.is_none();
        b.level = level;
        b.left = row.duration;
        b.total = row.duration;
        b.cut = row.power;
        b.contact = contact;
        b.caster = caster;
        if new_look {
            if let Some(h) = b.effect.take() {
                cx.host.effects.stop(h, false);
            }
            b.effect = spawn_barrier_look(cx, caster, member, level, sora_is_member);
        }
        return;
    }
    let effect = spawn_barrier_look(cx, caster, member, level, sora_is_member);
    fxw.worn.barriers.push(Barrier { member, caster, level, left: row.duration, total: row.duration, cut: row.power, contact, touched: Vec::new(), effect });
}

/// (DON-SPELLB) The barrier's size on its wearer (sheet `barrier.fx_scale_*`: the ribbons are authored round Sora).
pub fn barrier_scale(party: &Party, member: MemberId, sora_is_member: bool) -> f32 {
    let m = magic();
    if member == SORA_MEMBER || sora_is_member {
        return 1.0;
    }
    match party.member(member).map(|p| p.chara) {
        Some(crate::ids::Chara::Donald) => m.barrier_fx_scale_donald,
        Some(crate::ids::Chara::Goofy) => m.barrier_fx_scale_goofy,
        None => 1.0,
    }
}

/// (DON-SPELLB) THE BARRIER'S LOOK (the user, 2026-10-22: KH1's Aero, "it needs to last the ENTIRE duration of the spell,
/// fading away when it ends"): the authored wind ribbons `vd_don_aero1..3` (DON-SPELLB tools/make_aero.js), ENDLESS, on the
/// wearer, in the pale wind colour; stopped (not cut) when the barrier ends for any reason, so they fade out.
fn spawn_barrier_look(cx: &mut Ctx, caster: MemberId, member: MemberId, level: u8, sora_is_member: bool) -> Option<EffectHandle> {
    let spell = SpellRef::Line2(Element::Aero, level);
    let name = fx::BARRIER_LOOK[(level.clamp(1, 3) - 1) as usize];
    let scale = barrier_scale(cx.party, member, sora_is_member);
    // its colours are authored in the effect (no recolour: a tint lifts every streak to its brightest channel, the thick look)
    magic_fx::spawn_fx_tint(cx, caster, Some(spell), name, attach_on(member, sora_is_member, Vec3::ZERO), scale, true, None)
}

/// (DON-SPELLB) ONE WORN SPELL PER MEMBER (the user, 2026-10-22: "each character can only have one effect like that active
/// on them at a time"): a member's Aero Barrier ends, its ribbons fading out (stopped, not cut).
pub fn end_barrier_of(fxw: &mut MagicFx, cx: &mut Ctx, member: MemberId) {
    for b in fxw.worn.barriers.iter().filter(|b| b.member == member) {
        if let Some(h) = b.effect {
            cx.host.effects.stop(h, false);
        }
    }
    fxw.worn.barriers.retain(|b| b.member != member);
}

/// (DON-SPELLB) ... and a member's Ball Lightning ends, each ball popping (its glow stopped, the Thunder spark).
pub fn end_orbs_of(fxw: &mut MagicFx, cx: &mut Ctx, member: MemberId) {
    let Some(k) = fxw.worn.orbs.iter().position(|o| o.member == member) else { return };
    let set = fxw.worn.orbs.remove(k);
    for b in set.balls.iter() {
        if let Some(h) = b.core {
            cx.host.effects.stop(h, false);
        }
    }
}

/// (DON-SPELLB) The worn spell a member has now (None, or the barrier / Ball Lightning): one at a time.
pub fn worn_of(party: &Party, member: MemberId) -> Option<SpellRef> {
    if let Some(b) = party.don_fx.worn.barrier(member) {
        return Some(SpellRef::Line2(Element::Aero, b.level));
    }
    party.don_fx.worn.orbs(member).filter(|o| o.orbs > 0 && o.left > 0.0).map(|o| SpellRef::Line2(Element::Thunder, o.level))
}

/// A ball's glow core at `p` (moved every step by [`step_worn`]).
fn spawn_core(cx: &mut Ctx, caster: MemberId, spell: SpellRef, p: Vec3) -> Option<EffectHandle> {
    magic_fx::spawn_fx(cx, caster, Some(spell), fx::ORB_CORE, Attach::World(p), magic().orbs_ball_scale, true)
}

/// BALL LIGHTNING on `member` (standing at `from`): a new set, or (balls remaining) the recast hurls them at `aim`.
#[allow(clippy::too_many_arguments)]
pub fn put_orbs(fxw: &mut MagicFx, cx: &mut Ctx, caster: MemberId, member: MemberId, level: u8, row: &DonSpellRow, aim: Option<Aim>, _sora_is_member: bool, from: Vec3) {
    let m = magic();
    let spell = SpellRef::Line2(Element::Thunder, level);
    // (DON-SPELLB) one worn spell per member: Ball Lightning replaces the barrier (its ribbons fade out)
    end_barrier_of(fxw, cx, member);
    if let Some(k) = fxw.worn.orbs.iter().position(|o| o.member == member && o.orbs > 0) {
        let set = fxw.worn.orbs.remove(k);
        let balls = set.live_balls(from);
        for h in set.balls.iter().filter_map(|b| b.core) {
            cx.host.effects.stop(h, false);
        }
        if let Some(aim) = aim {
            let mut spec = HitSpec::of(spell, row).power(m.orbs_hurl_power).kind(Kind::Small);
            spec.tag = HitTag::of(ElemSet::one(Element::Thunder));
            for (_, pos) in balls {
                // each ball flies from where it orbits, its glow core carried along (`Effects::place` by step_actors)
                let burst = Burst::new(spec.clone(), 0.0).fx(&[fx::ORB_POP]);
                let dir = (aim.point(cx.host) - pos).normalize_or_zero();
                let id = fxw.add(caster, spell, ActorKind::Ball { pos, aim, speed: m.bolt_speed, turn: m.bolt_turn, dir, life: m.bolt_life, shrink: 0.0, arrived: None, burst, after: None });
                let core = spawn_core(cx, caster, spell, pos);
                if let Some(a) = fxw.actors.iter_mut().find(|a| a.id == id) {
                    a.effect = core;
                }
            }
        }
        return;
    }
    // a fresh set (a spent set on the member is replaced: one set per member)
    if let Some(k) = fxw.worn.orbs.iter().position(|o| o.member == member) {
        let old = fxw.worn.orbs.remove(k);
        for h in old.balls.iter().filter_map(|b| b.core) {
            cx.host.effects.stop(h, false);
        }
    }
    let n = row.count.clamp(1, 8) as u8;
    let mut set = Orbs { member, caster, level, orbs: n, max: n, left: row.duration, total: row.duration, ball_radius: row.radius.max(1.0), power: row.power, spin: 0.0, recharge_wait: m.orbs_recharge, chain: level >= 3 && row.extra > 0.0, balls: Vec::new() };
    for k in 0..n as usize {
        let p = set.ball_at(k, from);
        let core = spawn_core(cx, caster, spell, p);
        // the crackles staggered round the ring
        set.balls.push(Ball { live: true, core, crackle: m.orbs_crackle_gap * k as f32 / n as f32, flip: k % 2 == 1 });
    }
    fxw.worn.orbs.push(set);
}

/// One step of everything worn: timers, the barrier's contact hits, the orbs' leaps and recharges.
pub fn step_worn(fxw: &mut MagicFx, cx: &mut Ctx, sora: Option<&SoraView>, dt: f32) {
    let m = magic();
    let d = data();
    for i in fxw.worn.invincible.iter_mut() {
        i.1 -= dt;
    }
    fxw.worn.invincible.retain(|i| i.1 > 0.0);
    // barriers
    let mut barriers = std::mem::take(&mut fxw.worn.barriers);
    for b in barriers.iter_mut() {
        b.left -= dt;
        for t in b.touched.iter_mut() {
            t.1 -= dt;
        }
        b.touched.retain(|t| t.1 > 0.0);
        let Some(at) = wearer_at(cx.party, sora, b.member) else {
            b.left = 0.0;
            continue;
        };
        if let Some((power, reach)) = b.contact {
            let row = d.row(&SpellRef::Line2(Element::Aero, b.level).id()).copied();
            let Some(row) = row else { continue };
            let spec = HitSpec::of(SpellRef::Line2(Element::Aero, b.level), &row).power(power).kind(Kind::Small);
            let near = magic_fx::within(cx.host, at, reach, None);
            for e in near {
                if b.touched.iter().any(|t| t.0 == e.id) {
                    continue;
                }
                b.touched.push((e.id, m.barrier_contact_interval));
                let _ = magic_fx::spawn_fx(cx, b.caster, Some(spec.spell), fx::BARRIER_CONTACT, Attach::World(e.center), 1.0, false);
                magic_fx::deliver(cx, b.caster, &spec, &e, at);
            }
        }
    }
    for b in barriers.iter().filter(|b| b.left <= 0.0) {
        if let Some(h) = b.effect {
            cx.host.effects.stop(h, false);
        }
    }
    barriers.retain(|b| b.left > 0.0);
    fxw.worn.barriers = barriers;
    // Ball Lightning: the balls orbit the wearer; a ball that touches an enemy hits it and is used up
    let mut sets = std::mem::take(&mut fxw.worn.orbs);
    for o in sets.iter_mut() {
        o.left -= dt;
        let Some(at) = wearer_at(cx.party, sora, o.member) else {
            o.left = 0.0;
            continue;
        };
        o.spin = (o.spin + m.orbs_orbit_speed * dt).rem_euclid(360.0);
        let spell = SpellRef::Line2(Element::Thunder, o.level);
        // Lv 2+: one used-up ball comes back at a time
        if o.level >= 2 && o.orbs < o.max {
            o.recharge_wait -= dt;
            if o.recharge_wait <= 0.0 {
                o.recharge_wait = m.orbs_recharge;
                if let Some(k) = o.balls.iter().position(|b| !b.live) {
                    let p = o.ball_at(k, at);
                    let core = spawn_core(cx, o.caster, spell, p);
                    o.balls[k] = Ball { live: true, core, crackle: 0.0, flip: false };
                    o.orbs += 1;
                }
            }
        }
        let Some(row) = d.row(&spell.id()).copied() else { continue };
        let foes = magic_fx::foes(cx.host);
        for k in 0..o.balls.len() {
            if !o.balls[k].live {
                continue;
            }
            let p = o.ball_at(k, at);
            if let Some(h) = o.balls[k].core {
                cx.host.effects.place(h, p);
            }
            // the crackle: KH3's thunder sparks at the ball, two kinds in turn
            o.balls[k].crackle -= dt;
            if o.balls[k].crackle <= 0.0 {
                o.balls[k].crackle += m.orbs_crackle_gap.max(0.05);
                o.balls[k].flip = !o.balls[k].flip;
                let name = fx::ORB_CRACKLE[o.balls[k].flip as usize];
                let _ = magic_fx::spawn_fx(cx, o.caster, Some(spell), name, Attach::World(p), m.orbs_ball_scale, false);
            }
            // contact only: the enemy whose body overlaps this ball's sphere (the nearest when several do)
            let touched = foes.iter().filter(|e| ball_touches(e, p, o.ball_radius)).min_by(|a, b| (a.center - p).length().total_cmp(&(b.center - p).length())).copied();
            let Some(first) = touched else { continue };
            // the ball is used up
            o.balls[k].live = false;
            if let Some(h) = o.balls[k].core.take() {
                cx.host.effects.stop(h, false);
            }
            o.orbs = o.orbs.saturating_sub(1);
            let mut spec = HitSpec::of(spell, &row).power(o.power).kind(Kind::Blow);
            spec.push = m.orbs_knockback;
            spec.attr = Attr::Thunder;
            let _ = magic_fx::spawn_fx(cx, o.caster, Some(spell), fx::ORB_POP, Attach::World(p), 1.0, false);
            magic_fx::deliver(cx, o.caster, &spec, &first, p);
            if o.chain {
                // Lv 3: the touch arcs to a second enemy nearby
                let second = foes.iter().filter(|e| e.id != first.id && e.flags.has(TargetFlags::ALIVE) && (e.position - first.position).length() <= m.orbs_chain_range).min_by(|a, b| (a.position - first.position).length().total_cmp(&(b.position - first.position).length())).copied();
                if let Some(s) = second {
                    let _ = magic_fx::spawn_fx(cx, o.caster, Some(spell), fx::ORB_ARC, Attach::World(s.center), 1.0, false);
                    magic_fx::deliver(cx, o.caster, &spec, &s, first.center);
                }
            }
        }
        if o.orbs == 0 && o.level < 2 {
            o.left = 0.0;
        }
    }
    for o in sets.iter().filter(|o| o.left <= 0.0) {
        for h in o.balls.iter().filter_map(|b| b.core) {
            cx.host.effects.stop(h, false);
        }
    }
    sets.retain(|o| o.left > 0.0);
    fxw.worn.orbs = sets;
}

/// A member left the party (or is gone): his worn spells end.
pub fn drop_member(fxw: &mut MagicFx, effects: &mut dyn kh_core::host::Effects, m: MemberId) {
    for b in fxw.worn.barriers.iter().filter(|b| b.member == m) {
        if let Some(h) = b.effect {
            effects.stop(h, false);
        }
    }
    for o in fxw.worn.orbs.iter().filter(|o| o.member == m) {
        for h in o.balls.iter().filter_map(|b| b.core) {
            effects.stop(h, false);
        }
    }
    fxw.worn.barriers.retain(|b| b.member != m);
    fxw.worn.orbs.retain(|o| o.member != m);
}
