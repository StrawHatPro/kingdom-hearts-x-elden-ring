//! (party X2) SORA'S SPELLS CAST BY ANOTHER BODY: the spell actors, the hits, the Cure and the grand-magic gauge of a
//! FRIEND LEADER (Donald while the player controls him; build/sora_tmp/party/XD_PLAYABLE_KITS.md 1.1 / 1.5, approved
//! by the user 2026-10-15). KH3 has no playable Donald: the RULE "Donald casts Sora's magic while he leads" is the
//! port's design; every PIECE is Sora's, unchanged:
//!
//! - the actors are Sora's (`projectile.rs`: `StageDef::first(unit.proj)`, the same movement, shapes, respawns, rails,
//!   Aero pads, the Water barrier), spawned by the same rule as `TresPlayerState_FireMagic::OnShot` (0x14137BB30,
//!   magic spec A 1.4 steps 4 / 7: the muzzle point limited to actor Z + 163, the wall guard of a bolt, Thunder / Aero
//!   at the target's centre else 500 ahead, Water attached at (50, 0, 100) pitched 60 up), so Firaga is Firaga;
//! - the damage is `projectile::damage_of` (the stats spec's damage function) with THE CASTER'S Strength / Magic and
//!   abilities (Donald's Magic 67 at level 99 and his element Boosts) and Sora's game / battle level;
//! - every magic hit that deals damage feeds the formchange gauge through `grand::on_damage` -> `style::add` (the rates,
//!   the chain factor, Form Boost), the gauge tick and its decay are `style::tick`'s (20 s, -5 per s), and the full
//!   gauge rolls `grand::roll` (`RollGrandMagic` 0x1413ED500) with the -za ability counts the host gives (Sora's: they
//!   are shared like his magic). An offer lives `grand::lifetime` seconds after the queue's 0.5 s delay. There is no
//!   formchange offer (no Second Form for the friends: XD 1.5);
//! - the Cure actor is Sora's (`DoCure` 0x1414742A0): the main heal `m_AppendCure + max(1, Magic)` lands on the CASTER
//!   and `m_BonusAppendCure + max(1, Magic)` on every ally the host lists within `m_DetectRange` (a KO'd one half).
//!   The heals are returned ([`CasterStep::heals`]) for the host to apply: this caster owns no HP.
//!
//! What is the CASTER'S own (the host's, not here): the clip, its Shot timing, the cast effect, the voice, the MP
//! (Donald's own 100 MP and his 30 s charge: decision 2), invincibility.
//!
//! HANDLES: the proxy counts its sound / effect handles from the second half of its member's party range
//! (`PARTY_HANDLE_BASE + member x SPAN + SPAN / 2`), so they never meet Sora's or the member's own `SeamHandles`.
//! ATTACH: what Sora's actors attach to Sora (the Water barrier's body, the Cure's heal effect) is attached to the
//! member (`Attach::Party`), with the same bone and offset.

use glam::Vec3;

use super::projectile::{self, SpawnArgs, StageDef};
use super::spells::{self, Element};
use super::tables::Sheet;
use crate::anim::{AnimId, AnimPlayer};
use crate::cmc::Cmc;
use crate::enums::ETresAbilityKind;
use crate::host::party::{PartyMember, PARTY_HANDLE_BASE, PARTY_HANDLE_SPAN};
use crate::host::targets::{Ally, AttackShape, FinishKind, Friend, FriendLinkRequest, PushBody, TargetBody, TargetDefense};
use crate::host::{Attach, Audio, EffectHandle, EffectSpawn, Effects, Host, HitRecord, HitResult, HurtShape, SoundHandle, SoundPlay, TargetFlags, TargetId, TargetInfo, Targets};
use crate::params::SORA_PARAMS;
use crate::pawn::{Ctx, Pawn};
use crate::status::{AbilitySet, GameLevel, MpCost};
use crate::world::World;

/// What the caster's body lends the spells this step (the friend's status as the damage function reads it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CasterStats {
    pub strength: i32,
    pub magic: i32,
    /// Sora's (the difficulty factor and the damage floors read them).
    pub game_level: GameLevel,
    pub battle_level: u8,
    /// The host's factor on the spell costs (`Status::mp_cost_scale`, IMPL_ER61; 1 = KH3).
    pub mp_cost_scale: f32,
    /// The caster's MP charge runs (the roll's `alt`: M = 20 instead of 10).
    pub mp_charging: bool,
}

impl Default for CasterStats {
    fn default() -> CasterStats {
        CasterStats { strength: 0, magic: 0, game_level: GameLevel::Standard, battle_level: 1, mp_cost_scale: 1.0, mp_charging: false }
    }
}

/// One cast leaving the caster at its Shot (the caster's own clip decides when).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    /// The command: 29 Fire .. 51 Curaga, or a grand-magic `S_*` (110 ..).
    pub cmd: u8,
    /// The caster's actor root and yaw now.
    pub origin: Vec3,
    pub yaw: f32,
    /// The muzzle (the staff's tip) in the world; `None` = the actor root (A 1.4 step 7).
    pub muzzle: Option<Vec3>,
    /// The lock / soft target (`NONE`: none).
    pub target: TargetId,
    /// The magic chain's Finish End (the `_FIN` rows), a grand magic (the `_S` rows), "MP was full at the first shot".
    pub finish: bool,
    pub grand: bool,
    pub full_mp: bool,
    /// The combo setter's four counters (info +0x52 .. +0x55).
    pub combo: [i32; 4],
}

/// A heal the Cure actor gave: `ally` None = the caster himself; otherwise the id the host listed it under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CasterHeal {
    pub ally: Option<TargetId>,
    pub amount: i32,
    /// The Recover kind (Sora's DoCure: 7 / 8 / 9 on the target by tier, 10 / 11 / 12 on the others).
    pub effect: u8,
}

/// A grand-magic offer on the caster's situation slot (Y).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrandOffer {
    pub serial: u32,
    /// `S_FIRA` 110 .. `S_CURAGAN` 125.
    pub cmd: u8,
    /// Seconds of the queue's appear delay left (0.5 at the offer), then of life left / at the offer.
    pub delay: f32,
    pub remaining: f32,
    pub total: f32,
}

/// What a step of the caster's actors did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CasterStep {
    pub heals: Vec<CasterHeal>,
    /// Hits delivered this step (damage dealt > 0 or not), and the new grand-magic offers.
    pub hits: u32,
    pub offered: Vec<u8>,
}

/// The spells of one friend leader (see the module text).
#[derive(Clone, Debug)]
pub struct SpellCaster {
    pawn: Pawn,
    member: PartyMember,
    stats: CasterStats,
    offers: Vec<GrandOffer>,
    next_serial: u32,
    /// Melee form points waiting for the next step (`add_form_points`).
    pending_points: Vec<(i32, u8)>,
    /// Hits delivered so far (tests, the debug text).
    pub hits_total: u32,
    /// Casts shot so far.
    pub shots: u32,
}

/// The handle base of a member's caster: the second half of the member's party range.
pub const fn handle_base(member: PartyMember) -> u32 {
    PARTY_HANDLE_BASE + member.0 as u32 * PARTY_HANDLE_SPAN + PARTY_HANDLE_SPAN / 2
}

/// The abilities of the grand-magic roll and the gauge that are SORA'S (shared like his magic, XD 1.5): Grand Magic
/// (x1.3), Magic Roulette, Grand Magic Extender (MAGIC_TIME), the -GAN counts, Form Boost, Cure Converter.
pub const GRAND_ABILITIES: [ETresAbilityKind; 11] = [
    ETresAbilityKind::GrandMagic,
    ETresAbilityKind::MagicRoulette,
    ETresAbilityKind::MagicTime,
    ETresAbilityKind::Firagan,
    ETresAbilityKind::Blizzagan,
    ETresAbilityKind::Thundagan,
    ETresAbilityKind::Watagan,
    ETresAbilityKind::Aerogan,
    ETresAbilityKind::Curagan,
    ETresAbilityKind::FormUp,
    ETresAbilityKind::PowerCure,
];

/// The cost of a spell for a caster with these abilities (`Status::effective_mp_cost` 1:1: the host's scale, MP
/// Thrift `MP_SAVE` x n, Magic Combo Thrift `MAGIC_COMBO_SAVE` minus the combo). `None`: not a spell; `Some(-1)`: all MP
/// (the Cure family); grand magic costs 0.
pub fn effective_cost(cmd: u8, combo: i32, abilities: &AbilitySet, mp_cost_scale: f32) -> Option<i32> {
    let unit = spells::unit(cmd)?;
    let mut cost = match unit.mp {
        MpCost::All => return Some(-1),
        MpCost::Points(p) => p,
    };
    if cost > 0 {
        if mp_cost_scale > 0.0 && mp_cost_scale != 1.0 {
            cost = ((cost as f32 * mp_cost_scale).round() as i32).max(1);
        }
        let n = abilities.count(ETresAbilityKind::MpSave);
        if n > 0 {
            cost = (cost - crate::status::math::ue_floor(crate::status::runtime::MP_SAVE_COEFF * n as f32 * cost as f32 + crate::status::runtime::MP_SAVE_BIAS)).max(1);
        }
        if abilities.has(ETresAbilityKind::MagicComboSave) {
            cost = (cost - combo).max(1);
        }
    }
    Some(cost)
}

impl SpellCaster {
    pub fn new(member: PartyMember) -> SpellCaster {
        let mut pawn = Pawn::new(SORA_PARAMS, Cmc::new(&SORA_PARAMS, Vec3::ZERO), 0.0, AnimPlayer::new(AnimId::IdleN));
        pawn.set_handle_base(handle_base(member));
        pawn.status.weapon = String::new();
        SpellCaster { pawn, member, stats: CasterStats::default(), offers: Vec::new(), next_serial: 0, pending_points: Vec::new(), hits_total: 0, shots: 0 }
    }

    pub fn member(&self) -> PartyMember {
        self.member
    }

    /// The caster's body this step: where the barrier rides, where the Cure is centred, the hits' attacker.
    pub fn place(&mut self, location: Vec3, yaw: f32) {
        self.pawn.cmc.location = location;
        self.pawn.yaw = yaw;
    }

    /// The caster's stats and abilities (his own: Boosts, MP Thrift ...) with `grand` = whose -za abilities count
    /// (Sora's, `GRAND_ABILITIES`).
    pub fn set_stats(&mut self, stats: CasterStats, own: &AbilitySet, grand: &AbilitySet) {
        self.stats = stats;
        let s = &mut self.pawn.status;
        s.base.strength = stats.strength;
        s.base.magic = stats.magic;
        s.game_level = stats.game_level;
        s.battle_level = stats.battle_level.max(1);
        s.mp_cost_scale = stats.mp_cost_scale;
        s.mp_charging = stats.mp_charging;
        let mut set = own.clone();
        for kind in GRAND_ABILITIES {
            let n = grand.count(kind).min(255) as u8;
            set.set(kind, n, n);
        }
        s.abilities = set;
    }

    /// The abilities the spells see (the caster's own + Sora's grand ones).
    pub fn abilities(&self) -> &AbilitySet {
        &self.pawn.status.abilities
    }

    /// Releases a cast's actor(s) (OnShot's spawn part). Returns the ids spawned (0 or 1).
    pub fn shoot(&mut self, world: &dyn World, targets: &mut dyn Targets, audio: &mut dyn Audio, effects: &mut dyn Effects, shot: &Shot) -> Vec<u32> {
        let Some(unit) = spells::unit(shot.cmd) else { return Vec::new() };
        let Some(def) = StageDef::first(unit.proj) else { return Vec::new() };
        self.place(shot.origin, shot.yaw);
        let member = self.member;
        let mut t = CasterTargets { inner: targets, allies: &[], heals: Vec::new() };
        let mut a = RemapAudio { inner: audio, member };
        let mut e = RemapEffects { inner: effects, member };
        let mut ui = crate::host::ui::NullUi;
        let mut host = Host { world, targets: &mut t, audio: &mut a, effects: &mut e, ui: &mut ui };
        let mut cx = Ctx::new(&mut self.pawn, &mut host);
        let sp = Sheet::Spells;
        let (yaw, origin) = (shot.yaw, shot.origin);
        let facing = {
            let r = yaw.to_radians();
            Vec3::new(r.cos(), r.sin(), 0.0)
        };
        let info = cx.target(shot.target).filter(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE));
        let target = if unit.element == Element::Cure { TargetId::NONE } else { info.map_or(TargetId::NONE, |t| t.id) };
        let common = |def: StageDef, position: Vec3, direction: Vec3| SpawnArgs { def, position, direction, cmd: shot.cmd, shoot_flow: 0, finish: shot.finish, situation: shot.grand, full_mp: shot.full_mp, combo: shot.combo, target, attached: None, fire: true };
        let id = if unit.element == Element::Water {
            // `ShotWater` 0x14137FFA0: the barrier on the caster's root at (50, 0, 100), pitched 60 up.
            let offset = sp.vec3("cast.shot.water_target_vec");
            let pitch = sp.num("cast.shot.water_pitch").to_radians();
            let direction = Vec3::new(facing.x * pitch.cos(), facing.y * pitch.cos(), pitch.sin());
            let (sin, cos) = yaw.to_radians().sin_cos();
            let position = origin + Vec3::new(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos, offset.z);
            let fire_at = shot.muzzle.unwrap_or(position);
            projectile::spawn_fired_from(&mut cx, &SpawnArgs { attached: Some(offset), ..common(def, position, direction) }, fire_at)
        } else {
            // A 1.4 step 7: the muzzle, Z limited to actor Z + 163.
            let mut p = shot.muzzle.unwrap_or(origin);
            p.z = p.z.min(origin.z + sp.num("cast.shot.max_height_above_actor"));
            if !unit.direct_pos {
                // PitchToTarget + ClampPitch for a clip without a pitch axis (Donald's casts have none):
                // |pitch| over the threshold -> the fixed value with its sign, else level.
                let mut pitch = 0.0f32;
                if let Some(t) = info {
                    let d = t.center - origin;
                    if d.length_squared() > 1.0e-8 {
                        let raw = d.normalize().z.clamp(-1.0, 1.0).asin().to_degrees();
                        if raw.abs() > sp.num("cast.pitch.no_blend_threshold") {
                            pitch = sp.num("cast.pitch.no_blend_value").copysign(raw);
                        }
                    }
                }
                let pitch = pitch.to_radians();
                let d = Vec3::new(facing.x * pitch.cos(), facing.y * pitch.cos(), pitch.sin());
                // The wall guard: a trace from P - D L to P; on a hit P = the hit point.
                let l = sp.num("cast.shot.wall_trace_min_len").max((p - origin).truncate().length());
                if let Some(hit) = cx.world.line_trace(p - d * l, p) {
                    p = hit.position;
                }
                projectile::spawn(&mut cx, &common(def, p, d))
            } else {
                // Thunder, Aero: at the target's centre (not below its root), else 500 ahead.
                let mut at = origin + facing * sp.num("cast.shot.direct_default_range");
                if let Some(i) = info {
                    at = i.center;
                    at.z = at.z.max(i.position.z);
                }
                // (DON-MAGIC tip) the fire effects at the wand / blade TIP itself (the shot's muzzle), not held below actor Z + 163
                projectile::spawn_fired_from(&mut cx, &common(def, at, facing), shot.muzzle.unwrap_or(p))
            }
        };
        // 9. Cure Converter (POWER_CURE, Sora's): points to the gauge by the MP spent.
        if unit.element == Element::Cure && !unit.grand {
            if let Some(points) = crate::abilities::passive::cure_converter_points(&cx.pawn.status) {
                super::grand::on_damage(&mut cx, points, shot.cmd);
            }
        }
        drop(cx);
        self.pawn.events.clear();
        self.shots += 1;
        vec![id]
    }

    /// Melee form points of a damaging hit of the caster's own attack (`AddStylePoints(m_FormPoint, 1, 0)`), taken at the
    /// next step.
    pub fn add_form_points(&mut self, points: i32) {
        if points > 0 {
            self.pending_points.push((points, 0));
        }
    }

    /// One step of the caster's actors (`projectile::tick`), the Cure heals, the gauge, the offers' lives.
    /// `allies` = the other bodies the Cure may reach (the host's ids).
    pub fn step(&mut self, world: &dyn World, targets: &mut dyn Targets, audio: &mut dyn Audio, effects: &mut dyn Effects, allies: &[Ally], dt: f32) -> CasterStep {
        let mut out = CasterStep::default();
        let member = self.member;
        // The cure actors waiting for their first tick (DoCure on the caster, Tick 0x1414840A0).
        let cures: Vec<(u32, (f32, i32, i32), &'static str)> = self.pawn.magic.projectiles.iter().filter(|p| !p.dead && p.started && p.hits == 0).filter_map(|p| p.def.cure.map(|c| (p.id, c, p.def.family))).collect();
        let mut t = CasterTargets { inner: targets, allies, heals: Vec::new() };
        {
            let mut a = RemapAudio { inner: audio, member };
            let mut e = RemapEffects { inner: effects, member };
            let mut ui = crate::host::ui::NullUi;
            let mut host = Host { world, targets: &mut t, audio: &mut a, effects: &mut e, ui: &mut ui };
            let mut cx = Ctx::new(&mut self.pawn, &mut host);
            for (points, cmd) in std::mem::take(&mut self.pending_points) {
                crate::combat::style::add(&mut cx, points as f32, 1, cmd as i32);
            }
            projectile::tick(&mut cx, dt);
            out.offered = gauge_tick(&mut cx, &mut self.offers, &mut self.next_serial, self.stats.mp_charging, dt);
        }
        for (id, (_, append, _), family) in cures {
            let cured = self.pawn.magic.projectiles.iter().find(|p| p.id == id).is_none_or(|p| p.hits > 0);
            if cured {
                let tier = match family {
                    "cure" => 0,
                    "cura" => 1,
                    _ => 2,
                };
                out.heals.push(CasterHeal { ally: None, amount: append + self.pawn.status.magic().max(1), effect: 7 + tier });
            }
        }
        out.heals.extend(t.heals);
        out.hits = self.pawn.events.iter().filter(|e| matches!(e.kind, crate::host::EventKind::Hit { .. })).count() as u32;
        self.hits_total += out.hits;
        self.pawn.events.clear();
        self.pawn.popups.clear();
        // The offers' lives (J 5.2: the countdown starts after the queue's 0.5 s delay).
        for o in &mut self.offers {
            if o.delay > 0.0 {
                o.delay = (o.delay - dt).max(0.0);
            } else {
                o.remaining -= dt;
            }
        }
        self.offers.retain(|o| o.remaining > 0.0);
        out
    }

    /// The spell actors alive (for drawing and the effect side: `effect` = the body effect's handle).
    pub fn projectiles(&self) -> impl Iterator<Item = super::ProjectileView> + '_ {
        self.pawn.magic.projectile_views()
    }

    pub fn alive(&self) -> usize {
        self.pawn.magic.projectiles.iter().filter(|p| !p.dead).count()
    }

    /// The Blizzard ice-rail nodes the caster's bolts laid.
    pub fn rails(&self) -> &[super::RailNode] {
        &self.pawn.magic.rails
    }

    /// (party X3D) The Aero residues (`AeroPad`) the caster's Aero spells left: the caster's body is launched by one as
    /// Sora is (`magic::pads_tick`), by the host of that body (kh-party pad/donald.rs `party_step`).
    pub fn pads(&self) -> &[super::AeroPad] {
        &self.pawn.magic.pads
    }

    /// (party X3D) A pad used up by its launch (pads_tick: the pad is removed when it throws the body).
    pub fn use_pad(&mut self, id: u32) {
        self.pawn.magic.pads.retain(|p| p.id != id);
    }

    /// The formchange gauge (0 ..= 100) and the points waiting for its next tick.
    pub fn gauge(&self) -> (f32, f32) {
        (self.pawn.combat.style.gauge, self.pawn.combat.style.pending)
    }

    /// The element tallies of the roll (`elementPoints`, by `Attr`).
    pub fn element_points(&self) -> [f32; 8] {
        self.pawn.magic.grand.element_points
    }

    /// The grand-magic offers on the situation slot, oldest first.
    pub fn offers(&self) -> &[GrandOffer] {
        &self.offers
    }

    /// The front offer that may be taken now (its appear delay over).
    pub fn ready_offer(&self) -> Option<GrandOffer> {
        self.offers.iter().find(|o| o.delay <= 0.0).copied()
    }

    /// Y on an offer: it leaves the queue and the gauge's bookkeeping is cleared (J 5.5: `clear_grand`, `grand::reset`).
    pub fn take_offer(&mut self, serial: u32) -> Option<u8> {
        let k = self.offers.iter().position(|o| o.serial == serial)?;
        let cmd = self.offers.remove(k).cmd;
        self.pawn.combat.style.clear();
        super::grand::reset(&mut self.pawn.magic.grand);
        Some(cmd)
    }

    /// TEST / DEBUG: the gauge to its maximum (the next step rolls).
    pub fn debug_fill_gauge(&mut self) {
        self.pawn.combat.style.pending = 100.0;
    }

    /// TEST / DEBUG: an offer at once (no delay).
    pub fn debug_offer(&mut self, cmd: u8) {
        self.next_serial += 1;
        let total = super::grand::lifetime(&self.pawn.status);
        self.offers.push(GrandOffer { serial: self.next_serial, cmd, delay: 0.0, remaining: total, total });
    }

    /// Stops every actor's body effect (the member left the party / the caster is dropped).
    pub fn stop_all(&mut self, effects: &mut dyn Effects) {
        for p in self.pawn.magic.projectiles.drain(..) {
            if let Some(h) = p.effect {
                effects.stop(h, false);
            }
        }
        self.pawn.magic.rails.clear();
        self.pawn.magic.pads.clear();
    }
}

/// `StyleGaugeTick` (combat/style.rs `tick`, 0x14141C4F0) without the form's clock and with the friend's award: the
/// pending points into the gauge; at the maximum (`AwardAtMax` 0x1413F0ED0) the grand-magic roll, each offer queued
/// with the 0.5 s appear delay and `grand::lifetime`; no formchange offer (no Second Form for a friend); the gauge is
/// emptied. Then the decay (20 s without gain, then -5 every 1 s) and the tallies' reset with an empty gauge.
fn gauge_tick(cx: &mut Ctx, offers: &mut Vec<GrandOffer>, serial: &mut u32, alt: bool, dt: f32) -> Vec<u8> {
    use crate::combat::tables::P;
    let max = if P.form_gauge_max > 0.0 { P.form_gauge_max } else { 100.0 };
    let mut offered = Vec::new();
    {
        let style = &mut cx.pawn.combat.style;
        style.gauge = (style.gauge + style.pending).min(max);
        style.pending = 0.0;
    }
    if cx.pawn.combat.style.gauge >= max {
        cx.pawn.combat.style.awards += 1;
        let life = super::grand::lifetime(&cx.pawn.status);
        for cmd in super::grand::roll(cx, alt) {
            // `situation::enqueue`: one entry per command kind (an offer already queued is refreshed)
            match offers.iter_mut().find(|o| o.cmd == cmd) {
                Some(o) => o.remaining = o.total,
                None => {
                    *serial += 1;
                    offers.push(GrandOffer { serial: *serial, cmd, delay: P.queue_start_delay + P.queue_appear_time, remaining: life, total: life });
                }
            }
            offered.push(cmd);
        }
        cx.pawn.combat.style.clear();
    }
    let style = &mut cx.pawn.combat.style;
    style.dec_start -= dt;
    if style.dec_start <= 0.0 {
        style.dec_start = 0.0;
        style.dec_interval -= dt;
        if style.dec_interval <= 0.0 {
            style.dec_interval = P.form_gauge_dec_interval;
            style.gauge -= P.form_gauge_dec_amount;
        }
        if style.gauge <= 0.0 {
            style.clear();
        }
    }
    let style = &cx.pawn.combat.style;
    if style.gauge <= 0.0 && style.pending <= 0.0 {
        super::grand::reset(&mut cx.pawn.magic.grand);
    }
    offered
}

// ------------------------------------------------------------------------------------- the seams, re-attached

fn remap(at: Attach, member: PartyMember) -> Attach {
    match at {
        Attach::Sora { bone, offset } => Attach::Party { member, bone, offset },
        other => other,
    }
}

struct RemapEffects<'a> {
    inner: &'a mut dyn Effects,
    member: PartyMember,
}

impl Effects for RemapEffects<'_> {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
        let s = EffectSpawn { at: remap(spawn.at, self.member), ..*spawn };
        self.inner.spawn(handle, &s);
    }
    fn stop(&mut self, handle: EffectHandle, immediate: bool) {
        self.inner.stop(handle, immediate);
    }
    fn tint(&mut self, handle: EffectHandle, tint: crate::host::Tint) {
        self.inner.tint(handle, tint); // (DON-MAGIC tint) passed on
    }
    fn place(&mut self, handle: EffectHandle, at: glam::Vec3) {
        self.inner.place(handle, at); // (DON-MAGIC orbit) passed on
    }
    fn transform(&mut self, handle: EffectHandle, at: glam::Vec3, rotation: glam::Vec3, scale: glam::Vec3) {
        self.inner.transform(handle, at, rotation, scale); // (ZETTA) passed on
    }
}

struct RemapAudio<'a> {
    inner: &'a mut dyn Audio,
    member: PartyMember,
}

impl Audio for RemapAudio<'_> {
    fn play(&mut self, handle: SoundHandle, play: &SoundPlay) {
        let p = SoundPlay { at: remap(play.at, self.member), ..*play };
        self.inner.play(handle, &p);
    }
    fn stop(&mut self, handle: SoundHandle, fade_out: f32) {
        self.inner.stop(handle, fade_out);
    }
    fn voice_group_no(&mut self, number: i32) {
        self.inner.party_voice_group_no(self.member, number);
    }
    fn party_voice_group_no(&mut self, member: PartyMember, number: i32) {
        self.inner.party_voice_group_no(member, number);
    }
}

/// The host's targets for the caster's actors, with the Cure's allies the host listed and its heals recorded.
struct CasterTargets<'a, 'b> {
    inner: &'a mut dyn Targets,
    allies: &'b [Ally],
    heals: Vec<CasterHeal>,
}

impl Targets for CasterTargets<'_, '_> {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        self.inner.enumerate(out);
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.inner.info(id)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        self.inner.hurt_shapes(id, out);
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.inner.apply_hit(hit)
    }
    fn defense(&self, id: TargetId) -> Option<TargetDefense> {
        self.inner.defense(id)
    }
    fn allies(&self, out: &mut Vec<Ally>) {
        out.extend_from_slice(self.allies);
    }
    fn heal_ally(&mut self, id: TargetId, amount: i32, effect: u8) {
        self.heals.push(CasterHeal { ally: Some(id), amount, effect });
    }
    fn lock_on_range(&self, id: TargetId) -> f32 {
        self.inner.lock_on_range(id)
    }
    fn engaged_count(&self) -> u32 {
        self.inner.engaged_count()
    }
    fn body(&self, id: TargetId, part: u32) -> TargetBody {
        self.inner.body(id, part)
    }
    fn take_armor(&mut self, id: TargetId, power: i32, weak: bool) -> i32 {
        self.inner.take_armor(id, power, weak)
    }
    fn take_reaction_power(&mut self, id: TargetId, power: i32) -> i32 {
        self.inner.take_reaction_power(id, power)
    }
    fn rush_suppression(&self, id: TargetId) -> bool {
        self.inner.rush_suppression(id)
    }
    fn terrain_dz(&self, id: TargetId) -> f32 {
        self.inner.terrain_dz(id)
    }
    fn push_body(&self, id: TargetId) -> Option<PushBody> {
        self.inner.push_body(id)
    }
    fn reaction_state(&self, id: TargetId) -> u8 {
        self.inner.reaction_state(id)
    }
    fn attack_shapes(&self, out: &mut Vec<AttackShape>) {
        self.inner.attack_shapes(out);
    }
    fn touched(&mut self, hit: &HitRecord) {
        self.inner.touched(hit);
    }
    fn finish_kind(&self, id: TargetId) -> FinishKind {
        self.inner.finish_kind(id)
    }
    // (XSI) the XS abilities' questions go to the wrapped targets
    fn foe_level(&self, id: TargetId) -> Option<u8> {
        self.inner.foe_level(id)
    }
    fn damage_to_hp(&self) -> f32 {
        self.inner.damage_to_hp()
    }
    fn in_revenge(&self, id: TargetId) -> bool {
        self.inner.in_revenge(id)
    }
    fn end_revenge(&mut self, id: TargetId) {
        self.inner.end_revenge(id)
    }
    // the team-attack seam is Sora's: the caster's actors never ask it
    fn friends(&self, _out: &mut Vec<Friend>) {}
    fn friend_link(&mut self, _req: FriendLinkRequest) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_are_sora_s_with_the_caster_s_thrift() {
        let none = AbilitySet::EMPTY;
        assert_eq!(effective_cost(spells::cmd::FIRAGA, 0, &none, 1.0), Some(20));
        assert_eq!(effective_cost(spells::cmd::THUNDAGA, 0, &none, 1.0), Some(28));
        assert_eq!(effective_cost(spells::cmd::CURA, 0, &none, 1.0), Some(-1));
        assert_eq!(effective_cost(110, 0, &none, 1.0), Some(0));
        let mut thrift = AbilitySet::EMPTY;
        thrift.set(ETresAbilityKind::MpSave, 1, 1);
        // 20 - floor(0.2 x 20 + 0.9) = 16
        assert_eq!(effective_cost(spells::cmd::FIRAGA, 0, &thrift, 1.0), Some(16));
    }

    #[test]
    fn handles_are_in_the_member_s_upper_half() {
        let b = handle_base(PartyMember(0));
        assert!(crate::host::party::is_party_handle(b + 1) && crate::host::party::handle_member(b + 1) == Some(PartyMember(0)));
        assert!(b > PARTY_HANDLE_BASE + PARTY_HANDLE_SPAN / 4);
    }
}
