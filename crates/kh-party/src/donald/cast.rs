//! (DON-MAGIC) DONALD'S CASTS (documentation/DONALD_OVERHAUL.md 4.3, 5.3, 6.1 - 6.5): the cast on the pawn, its shot, the
//! Releases, the interruption by hits and Cast Guard. The spell actors themselves are `magic_fx.rs` / `allyfx.rs`.
//!
//! THE FLOW (DON1's `combat.rs` header):
//! 1. the caller (DON-KIT's menu / shortcut / Y route, DON-PROG's AI) gets a `CastPlan` from `DonCore::check_cast`;
//! 2. [`begin`]: the clip (`MagicFire` / `MagicAirFire` for the bolts of line 1 Fire / Blizzard, Flood's slam `M_1311`,
//!    `MagicCast` / `MagicAirCast` for everything else; played so its Shot comes at the row's `cast` time), the turn to
//!    the target, `plan.mp` spent AT ONCE (5.3: a hit while casting loses it), `DonCore::commit_cast`, Donald's own cast
//!    effect per element, the voice (KH3's cast state 0x14122D300: a Cure on another member = his line for that member
//!    `target_voice_number`; any other spell placed with MagicCast = `magic_voice_number` 101-103, the "big" callouts;
//!    the bolt clips carry their own random line), the air hover;
//! 3. at the shot (the party's step, [`party_step`]): the spell's actors (`magic_fx::launch`); the clip's rest
//!    (`cast.recover`) then frees him;
//! 4. every step: `party.don.events` `Released` -> [`release`] (the burst at the lock-on target, else the soft target,
//!    else around Donald; free), the actors, the worn spells, the Burn ticks, the statuses' visuals.
//!
//! INTERRUPTION (5.3, R23): any damaging hit that is not blocked ([`interrupt`] from `Party::receive_hit`) stops a cast
//! before its shot and the attune cast; the MP is lost. Cast Guard: `cast.guard_chance` (30 %) it does not.

use super::abilities::DonAbility;
use super::combat::{CastPlan, DonEvent};
use super::element::Element;
use super::magic_fx::{self, fx, magic, ActorKind, Aim, Burst, Ctx, FieldRule, Follow, FxLog, HitSpec, MagicFx, Special, Wall};
use super::react::HitTag;
use super::spells::{ArcaneLine, SpellRef};
use super::status::{Apply, Size};
use super::tune::data;
use crate::host::PartyHost;
use crate::ids::MemberId;
use crate::party::Party;
use crate::pawn::NpcPawn;
use crate::view::{PartyOutput, SoraHeal, SoraView};
use glam::Vec3;
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{Attach, EffectId, EffectSpawn, PartyMember, TargetFlags, TargetId};

/// Sora (the player, or AI Sora while a friend leads) as the member target of an ally spell (kh-partyhost
/// `aiworld::SORA_ACTOR`).
pub const SORA_MEMBER: MemberId = MemberId(7);

/// Where a cast goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CastTarget {
    Enemy(TargetId),
    /// A party member (Aero Barrier, Ball Lightning; [`SORA_MEMBER`] = Sora).
    Member(MemberId),
    Point(Vec3),
}

/// A cast under way on the pawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CastRun {
    pub spell: SpellRef,
    pub plan: CastPlan,
    pub target: CastTarget,
    /// Seconds since the cast began; the shot's time; the time he is free again.
    pub t: f32,
    pub shot_at: f32,
    pub end_at: f32,
    pub shot: bool,
    pub hover: bool,
    pub clip: &'static str,
}

/// The pawn's side (`NpcPawn::don_cast`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PawnCast {
    pub run: Option<CastRun>,
    /// Casts begun / interrupted / guarded by Cast Guard (tests, the debug text).
    pub begun: u32,
    pub interrupted: u32,
    pub guarded: u32,
    /// The last spell that left the wand (Spell Weaver's window, the HUD) and seconds since.
    pub last: Option<(SpellRef, f32)>,
    /// (DON-SPELLB) the cast presentation under way (spell_look.rs: the hold, the wand's charge).
    pub look: super::spell_look::LookRun,
}

/// (DON-SPELLB) THE USER'S GROUND RULE (2026-10-22): "Donald can not perform any ultimate magic or zettaflare while up in the
/// air, he must be on the ground": the six line-3 ultimates and Ultima (an ultimate-style cast); Zettaflare is gated by
/// ZETTA. The menu greys them with `Refusal::NeedsGround` (DON-UI) when [`ground_refusal`] says so.
pub fn needs_ground(spell: SpellRef) -> bool {
    matches!(spell, SpellRef::Ultimate(_) | SpellRef::Arcane(ArcaneLine::Ultima | ArcaneLine::Zettaflare, _))
}

/// (DON-SPELLB) The refusal of a spell for a pawn in the air (None: castable as far as the ground goes).
pub fn ground_refusal(p: &NpcPawn, spell: SpellRef) -> Option<super::combat::Refusal> {
    (needs_ground(spell) && !p.on_ground()).then_some(super::combat::Refusal::NeedsGround)
}

/// The cast clip of a spell (`air`: in the air).
pub fn clip_of(spell: SpellRef, air: bool) -> &'static str {
    match spell {
        SpellRef::Line1(Element::Fire | Element::Blizzard, t) if t <= 3 => {
            if air {
                "MagicAirFire"
            } else {
                "MagicFire"
            }
        }
        SpellRef::Line2(Element::Water, _) if !air => "1311",
        _ => {
            if air {
                "MagicAirCast"
            } else {
                "MagicCast"
            }
        }
    }
}

/// The clip time of the clip's shot (sheet `cast.shot_*`).
fn shot_clip_time(clip: &str) -> f32 {
    let m = magic();
    match clip {
        "MagicFire" => m.cast_shot_fire,
        "MagicAirFire" => m.cast_shot_air_fire,
        "1311" => m.cast_shot_slam,
        _ => m.cast_shot_cast,
    }
}

/// Donald's cast effect of a spell (his `n_ex001_Magic_ead` rows by element; the Y magic [R]: Gravity the Aero call,
/// Ultima the Cure call, Comet / Meteor / Zettaflare the Fire call).
pub fn cast_effect_of(spell: SpellRef) -> &'static str {
    match spell {
        SpellRef::Arcane(ArcaneLine::Gravity, _) => magic_fx::cast_effect_name(Element::Aero),
        SpellRef::Arcane(ArcaneLine::Ultima, _) => magic_fx::cast_effect_name(Element::Light),
        SpellRef::Arcane(..) => magic_fx::cast_effect_name(Element::Fire),
        s => magic_fx::cast_effect_name(s.element().unwrap_or(Element::Fire)),
    }
}

/// The voice number of a cast (see the module text): 0 = the clip's own line.
fn voice_of(p: &mut NpcPawn, spell: SpellRef, clip: &str, target: CastTarget) -> i32 {
    let m = PartyMember(p.id.0);
    let r = p.rng.next_u32();
    if let (SpellRef::Line1(Element::Light, _), CastTarget::Member(t)) = (spell, target) {
        if t != p.id {
            let uid = if t == SORA_MEMBER { 1 } else { 12 };
            return kh_core::host::party::target_voice_number(m, uid, false, r);
        }
        return 0;
    }
    if matches!(spell, SpellRef::Line1(Element::Light, _)) || clip.contains("Fire") {
        return 0;
    }
    let big = spell.is_za() || spell.is_ultimate() || matches!(spell, SpellRef::Arcane(..));
    if big || magic().cast_voice_big <= 0.0 || clip.contains("Cast") || clip == "1311" {
        return kh_core::host::party::magic_voice_number(m, r);
    }
    0
}

/// STARTS A CAST (see the module text). False = refused: not Donald, a cast already before its shot, the MP or the
/// plan does not fit (the caller's `check_cast` is the rule; this only guards).
pub fn begin(p: &mut NpcPawn, plan: &CastPlan, target: CastTarget, host: &mut PartyHost) -> bool {
    // (ZETTA) Zettaflare plays the KH3 cutscene's presentation (donald/zettaflare.rs): its clips, effects and sounds, aimed
    if super::zettaflare::takes(plan.spell) {
        return super::zettaflare::begin(p, plan, target, host);
    }
    let d = data();
    let m = magic();
    if p.status.donald.is_none() || p.status.ko() {
        return false;
    }
    // (DON-SPELLB) the ultimates and Ultima only on the ground (the pad's refusal plays KH3's refused SE)
    if needs_ground(plan.spell) && !p.on_ground() {
        return false;
    }
    if p.don_cast.run.is_some_and(|r| !r.shot) {
        return false;
    }
    if plan.mp > 0 && (p.status.mp < plan.mp || p.status.mp_charge_left > 0.0) {
        return false;
    }
    let Some(row) = plan.spell.row(&d).copied() else { return false };
    // the turn to the target (else the held stick)
    let goal = match target {
        CastTarget::Enemy(t) => host.targets.info(t).map(|i| i.center),
        CastTarget::Point(at) => Some(at),
        CastTarget::Member(_) => None,
    };
    let max = m.cast_turn_max;
    if let Some(g) = goal.filter(|g| crate::pawn::horizontal(*g - p.position()).length() > 1.0) {
        let want = crate::pawn::yaw_of(crate::pawn::horizontal(g - p.position()));
        let diff = crate::pawn::wrap180(want - p.yaw);
        p.yaw = crate::pawn::wrap180(p.yaw + diff.clamp(-max, max));
    } else if let Some(want) = p.pad.as_ref().filter(|d| d.stick_magnitude >= kh_core::params::SORA_PARAMS.input_stick_active_min).map(|d| d.stick_yaw) {
        let diff = crate::pawn::wrap180(want - p.yaw);
        p.yaw = crate::pawn::wrap180(p.yaw + diff.clamp(-max, max));
    }
    p.cmc.stop_movement();
    let air = !p.on_ground();
    // (DON-SPELLB) part B's spells take their own pose, wand effects and voice (spell_look.rs); the ultimates ZETTA's circle
    let looked = super::spell_look::begin_look(p, plan.spell, row.cast, target, air, host);
    let (clip, shot_at, end_at) = match looked {
        Some(x) => x,
        None => {
            let clip = clip_of(plan.spell, air);
            p.play(clip, 0.1);
            super::sora_magic::hold_pose(p, plan.spell); // (DON-MAGIC part A) the cast clip is not run over by his locomotion
            let cast = row.cast.max(0.05);
            let shot_clip = shot_clip_time(clip);
            p.anim.speed = (shot_clip / cast).clamp(m.cast_rate_min, m.cast_rate_max);
            (clip, cast, cast + m.cast_recover)
        }
    };
    let hover = air && !p.don_cast.look.ult;
    if hover {
        // Sora's air cast rule: gravity off until the shot
        p.cmc.velocity.z = 0.0;
        p.cmc.gravity_off = true;
        p.cmc.set_flying();
    }
    // the MP: spent at the cast's start (lost if a hit interrupts it), then the core
    if plan.mp > 0 {
        crate::leader_magic::use_mp(&mut p.status, plan.mp);
    }
    if let Some(st) = p.status.donald.as_mut() {
        let mods = st.abilities.mods();
        st.core.commit_cast(plan, &mods, &d);
    }
    // the cast effect at the staff, the voice (part B: its own, spell_look.rs)
    if looked.is_some() {
        p.don_cast.begun += 1;
        p.don_cast.run = Some(CastRun { spell: plan.spell, plan: *plan, target, t: 0.0, shot_at, end_at, shot: false, hover, clip });
        return true;
    }
    let name = cast_effect_of(plan.spell);
    let pm = PartyMember(p.id.0);
    let big = plan.spell.is_za() || plan.spell.is_ultimate() || matches!(plan.spell, SpellRef::Arcane(..));
    let h = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at: Attach::party(pm, Some("vfx_call")), rotation: Vec3::ZERO, scale: if big { 1.5 } else { 1.0 }, persistent: false });
    if matches!(plan.spell, SpellRef::Arcane(..)) {
        // [R] the Y magic's call in the Arcane colour (sheet color.arcane): it borrows an element's call effect
        let c = d.tune.color_arcane as u32;
        let t = kh_core::host::Tint::rgb(((c >> 16) & 0xFF) as f32 / 255.0, ((c >> 8) & 0xFF) as f32 / 255.0, (c & 0xFF) as f32 / 255.0);
        host.effects.tint(h, t);
    }
    let n = voice_of(p, plan.spell, clip, target);
    if n != 0 {
        host.audio.party_voice_group_no(pm, n);
    }
    // (no KH3 SpellCast event: that names the KH3 actions DonaldFire ..., which the overhaul replaced)
    let _ = def_of;
    p.don_cast.begun += 1;
    p.don_cast.run = Some(CastRun { spell: plan.spell, plan: *plan, target, t: 0.0, shot_at, end_at, shot: false, hover, clip });
    true
}

/// The KH3 action whose events the HUD / log know a cast by.
fn def_of(spell: SpellRef) -> crate::command::ActionDef {
    use crate::command::ActionDef as A;
    match spell.element() {
        Some(Element::Fire) => A::DonaldFire,
        Some(Element::Blizzard) => A::DonaldBlizzard,
        Some(Element::Aero) => A::DonaldAero,
        Some(Element::Light) => A::DonaldCure,
        _ => A::DonaldThunder,
    }
}

/// A hit (or a cancel when `by_hit` is false) on a casting Donald: true when the cast (or the attune cast) was stopped
/// (the MP is lost). Cast Guard: a hit is shrugged off with `cast.guard_chance`.
pub fn interrupt(p: &mut NpcPawn, by_hit: bool) -> bool {
    let casting = p.don_cast.run.is_some_and(|r| !r.shot);
    let attuning = p.status.donald.as_ref().is_some_and(|st| st.core.attune.cast.is_some());
    if !casting && !attuning {
        return false;
    }
    if by_hit && p.status.donald.as_ref().is_some_and(|st| st.abilities.has(DonAbility::CastGuard)) {
        let roll = (p.rng.next_u32() >> 8) as f32 / (1u32 << 24) as f32;
        if roll < magic().cast_guard_chance {
            p.don_cast.guarded += 1;
            return false;
        }
    }
    if casting {
        if p.don_cast.look.ult {
            super::zettaflare::ultimate_cancel(p); // (DON-SPELLB) the ultimate's circle and clips end with the cast
            p.don_cast.look.ult = false;
        }
        p.don_cast.run = None;
        p.don_cast.interrupted += 1;
        p.anim.speed = 1.0;
        end_hover(p);
    }
    if attuning {
        if let Some(st) = p.status.donald.as_mut() {
            st.core.interrupt_attune();
        }
    }
    true
}

/// A cast is running (from its start until the clip's rest after the shot).
pub fn casting(p: &NpcPawn) -> bool {
    p.don_cast.run.is_some()
}

/// The cast under way.
pub fn current(p: &NpcPawn) -> Option<&CastRun> {
    p.don_cast.run.as_ref()
}

/// The cast is before its shot (a hit would interrupt it; the menu may not start another).
pub fn before_shot(p: &NpcPawn) -> bool {
    p.don_cast.run.is_some_and(|r| !r.shot)
}

fn end_hover(p: &mut NpcPawn) {
    if p.don_cast.run.is_some_and(|r| r.hover) || p.cmc.gravity_off {
        p.cmc.gravity_off = false;
        if !p.on_ground() {
            p.cmc.set_falling();
        }
    }
}

/// THE RELEASE (4.3): `member`'s `e` Release burst at his lock-on target, else the soft target, else around him; free.
pub fn release(party: &mut Party, member: MemberId, e: Element, host: &mut PartyHost) {
    let mut fxw = std::mem::take(&mut party.don_fx);
    {
        let mut log = std::mem::take(&mut fxw.log);
        {
            let mut cx = Ctx { party, host, log: &mut log };
            release_into(&mut fxw, &mut cx, member, e);
        }
        fxw.log.extend(log);
    }
    party.don_fx = fxw;
}

/// Where a Release bursts: the lock-on target, else the soft target, else the nearest foe within 15 m, else Donald.
fn release_aim(cx: &Ctx, member: MemberId) -> (Aim, Option<TargetId>) {
    let Some(p) = cx.party.member(member) else { return (Aim::Point(Vec3::ZERO), None) };
    let alive = |t: TargetId| cx.host.targets.info(t).filter(|i| i.flags.has(TargetFlags::ALIVE));
    let pick = p
        .pad
        .as_ref()
        .and_then(|d| alive(d.lock).or_else(|| alive(d.soft)))
        .or_else(|| match p.focus {
            crate::command::TargetRef::Enemy(t) => alive(t),
            _ => None,
        })
        .or_else(|| magic_fx::foes(cx.host).into_iter().filter(|f| (f.position - p.position()).length() <= 1500.0).min_by(|a, b| (a.position - p.position()).length().total_cmp(&(b.position - p.position()).length())));
    match pick {
        Some(i) => (Aim::Target(i.id, i.center), Some(i.id)),
        None => (Aim::Point(p.position()), None),
    }
}

fn release_into(fxw: &mut MagicFx, cx: &mut Ctx, member: MemberId, e: Element) {
    let d = data();
    let m = magic();
    let spell = SpellRef::Release(e);
    let Some(row) = spell.row(&d).copied() else { return };
    let (aim, _) = release_aim(cx, member);
    let mut spec = HitSpec::of(spell, &row);
    let boost = cx.party.status(member).and_then(|s| s.donald.as_ref()).map_or(1.0, |st| magic_fx::boost_of(spell, &st.abilities));
    spec.mult = boost;
    let at = aim.ground(cx.host);
    let mut burst = Burst::new(spec.clone(), row.radius);
    match e {
        Element::Fire => burst = burst.fx(&[fx::FIRESTORM]),
        Element::Blizzard => {
            burst.spec.tag = HitTag { elements: super::element::ElemSet::one(Element::Blizzard), breaker: true };
            burst = burst.fx(&fx::SHATTER);
        }
        Element::Thunder => {
            burst.spec.hit_fx = fx::OVERLOAD_HIT; // (AUD) strobe-free on each enemy
            burst = burst.fx(&[fx::OVERLOAD]);
        }
        Element::Water => burst = burst.fx(&[fx::DELUGE]),
        Element::Aero => {
            burst.spec.toward = Some(at);
            burst.spec.push = m.rel_cyclone_pull;
            burst.spec.kind = Kind::Blow;
            burst = burst.fx(&[fx::CYCLONE]);
        }
        Element::Light => {
            burst.special = Special::Sanctuary { heal_share: m.rel_sanctuary_heal, invincible: row.duration };
        }
    }
    let _ = magic_fx::spawn_fx(cx, member, Some(spell), "", Attach::World(at), 1.0, false);
    // (DON-SPELLB) the burst's look (spell_payload.rs)
    let me = cx.party.member(member).map_or(at, |p| p.position());
    super::spell_payload::release(fxw, member, e, at, me, row.radius);
    fxw.add(member, spell, ActorKind::Delayed { aim, delay: 0.0, burst });
}

// ============================================================================================== the shot

/// The muzzle: the staff's attack socket of the clip now, else the body's front.
pub(crate) fn muzzle(p: &NpcPawn) -> Vec3 { // (DON-MAGIC tip) pub(crate): the tests
    let f = crate::pawn::forward(p.yaw);
    let right = Vec3::new(-f.y, f.x, 0.0);
    let keys = p.anim.clip.socket;
    if keys.is_empty() {
        p.capsule_centre() + f * p.params.capsule_radius
    } else {
        let v = crate::actions::socket_track_at(keys, p.anim.time);
        p.position() + f * v.x + right * v.y + Vec3::Z * v.z
    }
}

/// THE SPELL LEAVES THE WAND: its actors (see `magic_fx` / `allyfx`).
pub fn launch(fxw: &mut MagicFx, cx: &mut Ctx, caster: MemberId, run: &CastRun, sora: Option<&SoraView>) {
    let d = data();
    let m = magic();
    let spell = run.spell;
    let Some(row) = spell.row(&d).copied() else { return };
    let Some((pos, yaw, centre, mz, abilities)) = cx.party.member(caster).and_then(|p| p.status.donald.as_ref().map(|st| (p.position(), p.yaw, p.capsule_centre(), muzzle(p), st.abilities))) else { return };
    let fwd = crate::pawn::forward(yaw);
    let aim = match run.target {
        CastTarget::Enemy(t) => match cx.host.targets.info(t).filter(|i| i.flags.has(TargetFlags::ALIVE)) {
            Some(i) => Aim::Target(t, i.center),
            None => Aim::Point(centre + fwd * 500.0),
        },
        CastTarget::Point(p) => Aim::Point(p),
        CastTarget::Member(_) => Aim::Point(centre + fwd * 500.0),
    };
    let ground = aim.ground(cx.host);
    let mut base = HitSpec::of(spell, &row);
    base.mult = magic_fx::boost_of(spell, &abilities);
    let member = match run.target {
        CastTarget::Member(t) => t,
        _ => caster,
    };
    let sora_is_member = sora.and_then(|s| s.ai_sora).is_some_and(|a| a.0 == member);
    if let Some(p) = cx.party.member_mut(caster) {
        p.don_cast.last = Some((spell, 0.0));
    }
    // (DON-SPELLB) the ultimates' and Ultima's big visual payload (spell_payload.rs); their rules follow below
    if spell.is_ultimate() || matches!(spell, SpellRef::Arcane(ArcaneLine::Ultima, _)) {
        // (AUD) the foes standing now: the ultimates' storms strike them first (Judgment Bolt's recipe)
        let foes: Vec<(TargetId, Vec3)> = magic_fx::foes(cx.host).into_iter().map(|f| (f.id, f.position)).collect();
        super::spell_payload::launch_foes(fxw, caster, spell, ground, pos, yaw, row.radius.max(row.range.min(1200.0)), row.radius, &foes);
    }
    match spell {
        SpellRef::Line1(e, tier) => super::sora_magic::shoot(fxw, cx, caster, e, tier, aim, pos, yaw, mz), // (DON-MAGIC part A)
        // ---- line 2
        SpellRef::Line2(Element::Fire, lv) => {
            // Flare: the star flies at the target, shrinks 0.5 s, detonates (2 / 3 m), Burn consumed +50 %; Lv 3 after-flare
            let mut spec = base.clone().kind(if m.flare_stagger > 0.0 { Kind::Blow } else { Kind::Small });
            spec.flare = true;
            spec.hit_fx = fx::FLARE_HIT;
            let burst = Burst::new(spec.clone(), row.radius).fx(&[fx::FLARE_BOOM, fx::FLARE_BLAST]); // (AUD) the blast
            let after = (lv >= 3 && row.extra > 0.0).then(|| {
                let mut s = spec.clone().kind(Kind::Small);
                s.flare = false;
                s.mult *= row.extra;
                (m.flare_after_delay, Burst::new(s, row.radius).fx(&[fx::FLARE_AFTER]))
            });
            let dir = (aim.point(cx.host) - mz).normalize_or_zero();
            let id = fxw.add(caster, spell, ActorKind::Ball { pos: mz, aim, speed: m.flare_speed, turn: m.bolt_turn, dir, life: m.bolt_life, shrink: m.flare_shrink, arrived: None, burst, after });
            attach_body(fxw, cx, caster, id, fx::FLARE_STAR, Attach::World(mz));
            // (KITFX) Flare's launch and its flight hum (stopped at the detonation: magic_fx `kitfx_hums`)
            if let Some(p) = cx.party.member_mut(caster) {
                let audio = &mut *cx.host.audio;
                let se = |name: &'static str, volume: f32, looping: bool| kh_core::host::SoundPlay { id: kh_core::host::SoundId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(mz), volume, pitch: 1.0, looping, spatial: true };
                p.seam.play(audio, &se(fx::FLARE_LAUNCH_SE, 1.0, false));
                let h = p.seam.play(audio, &se(fx::FLARE_HUM_SE, fx::FLARE_HUM_VOLUME, true));
                fxw.kitfx_hums.push((id, h));
            }
        }
        SpellRef::Line2(Element::Blizzard, lv) => {
            // Freeze: 1 s of frost on the target (the warning), then the prison
            if let Some(t) = aim.target() {
                let _ = magic_fx::spawn_fx(cx, caster, Some(spell), fx::FREEZE_FROST, Attach::Target { target: t, offset: Vec3::ZERO }, 1.0, false);
                // (AUD) the warning reads: frost drawn in onto the body and a cold flash (the Frozen's own "freeze in" look) as it
                // gathers; the bolt burst alone was a speck at the player's distance
                let _ = magic_fx::spawn_fx(cx, caster, Some(spell), fx::FROZEN_IN, Attach::Target { target: t, offset: Vec3::ZERO }, 1.15, false);
            }
            let mut burst = Burst::new(base.clone().applies(Vec::new()), if lv >= 2 { row.radius } else { 0.0 });
            burst.special = Special::Freeze { seconds: row.duration, max: Size::Medium, shards: lv >= 3 };
            fxw.add(caster, spell, ActorKind::Delayed { aim, delay: row.extra.max(0.0), burst });
        }
        SpellRef::Line2(Element::Thunder, lv) => super::allyfx::put_orbs(fxw, cx, caster, member, lv, &row, lock_aim(cx, caster), sora_is_member, super::allyfx::wearer_at(cx.party, sora, member).unwrap_or(pos)),
        SpellRef::Line2(Element::Water, lv) => {
            // Flood: the wave in front (90 / 120 / 360 deg, 8 / 12 / 15 m), push 5 / 7 / 7 m, Wet, wall stun; a 6 s puddle
            let mut spec = base.clone().kind(Kind::Blow);
            spec.push = row.push;
            let half = (row.angle * 0.5).clamp(1.0, 180.0);
            let _ = magic_fx::spawn_fx(cx, caster, Some(spell), fx::FLOOD_WAVE, Attach::World(pos), if lv >= 3 { 1.5 } else { 1.0 }, false);
            // (DON-SPELLB) Flood's look: the slam burst, the wave across the arc, the puddle's splashes (spell_payload.rs)
            {
                let (pc, pr) = if half >= 179.0 { (pos, m.flood_puddle_radius * 1.5) } else { (pos + fwd * (row.range * 0.4), m.flood_puddle_radius) };
                super::spell_payload::flood(fxw, caster, lv, pos, yaw, half, row.range, pc, pr, row.duration);
            }
            fxw.add(caster, spell, ActorKind::Wave { origin: pos, yaw, half, reach: row.range, speed: m.flood_wave_speed, front: 0.0, spec, hit: Vec::new(), flood: true });
            let (pc, pr) = if half >= 179.0 { (pos, m.flood_puddle_radius * 1.5) } else { (pos + fwd * (row.range * 0.4), m.flood_puddle_radius) };
            let mut wet = base.clone().power(0.0);
            wet.no_reaction = true;
            wet.chains = false;
            wet.mult = 1.0;
            let id = fxw.add(caster, spell, ActorKind::Field { centre: pc, follow: Follow::Fixed, radius: pr, life: row.duration, tick: m.flood_puddle_tick, next: m.flood_puddle_tick, spec: wet, pull: 0.0, rule: FieldRule::Puddle, special: Special::None });
            attach_body(fxw, cx, caster, id, fx::FLOOD_PUDDLE, Attach::World(pc));
        }
        SpellRef::Line2(Element::Aero, lv) => super::allyfx::put_barrier(fxw, cx, caster, member, lv, &row, sora_is_member),
        SpellRef::Line2(Element::Light, lv) => {
            // Banish: a holy ray on the target (Reveal); II / III more rays on nearby enemies
            let Some(t) = aim.target() else { return };
            let mut spec = base.clone();
            spec.hit_fx = fx::HIT_LIGHT;
            let mut ids = vec![t];
            if lv >= 2 {
                let at = aim.point(cx.host);
                let mut near: Vec<_> = magic_fx::foes(cx.host).into_iter().filter(|e| e.id != t && (e.position - at).length() <= row.radius).collect();
                near.sort_by(|a, b| (a.position - at).length().total_cmp(&(b.position - at).length()));
                ids.extend(near.into_iter().map(|e| e.id));
            }
            for k in 0..row.count.max(1) {
                let id = ids[k as usize % ids.len()];
                let c = cx.host.targets.info(id).map_or(ground, |i| i.center);
                let mut b = Burst::new(spec.clone(), 0.0).fx(&[if k == 0 { fx::BANISH_RAY } else { fx::BANISH_RAY_SOFT }]); // (AUD)
                b.fx_scale = 0.7; // (AUD) a ray, not Judgment Bolt's storm
                fxw.add(caster, spell, ActorKind::Delayed { aim: Aim::Target(id, c), delay: k as f32 * m.banish_ray_gap, burst: b });
            }
        }
        // ---- the ultimates
        SpellRef::Ultimate(Element::Fire) => {
            let id = fxw.add(caster, spell, ActorKind::Field { centre: ground, follow: Follow::Fixed, radius: row.radius, life: row.duration, tick: m.inferno_tick, next: 0.0, spec: base.clone(), pull: 0.0, rule: FieldRule::Area, special: Special::None });
            attach_body(fxw, cx, caster, id, fx::INFERNO[0], Attach::World(ground));
            for n in &fx::INFERNO[1..] {
                let _ = magic_fx::spawn_fx(cx, caster, Some(spell), n, Attach::World(ground), 1.0, false);
            }
        }
        SpellRef::Ultimate(Element::Blizzard) => {
            for n in fx::DIAMOND_DUST {
                let _ = magic_fx::spawn_fx(cx, caster, Some(spell), n, Attach::World(ground), 1.0, false);
            }
            let mut burst = Burst::new(base.clone().applies(Vec::new()), row.radius);
            burst.special = Special::Freeze { seconds: row.duration, max: Size::Boss, shards: false };
            fxw.add(caster, spell, ActorKind::Delayed { aim: Aim::Point(ground), delay: m.dust_delay, burst });
        }
        SpellRef::Ultimate(Element::Thunder) => {
            let mut spec = base.clone();
            spec.hit_fx = fx::HIT_THUNDER;
            fxw.add(caster, spell, ActorKind::Field { centre: pos, follow: Follow::Caster, radius: row.radius, life: row.count as f32 * m.judgment_gap + 1.0, tick: m.judgment_gap, next: 0.0, spec, pull: 0.0, rule: FieldRule::Storm { left: row.count, next: 0 }, special: Special::None });
        }
        SpellRef::Ultimate(Element::Water) => {
            let mut spec = base.clone().kind(Kind::Blow);
            spec.push = row.push;
            for n in fx::TSUNAMI {
                let _ = magic_fx::spawn_fx(cx, caster, Some(spell), n, Attach::World(pos), 1.0, false);
            }
            fxw.add(caster, spell, ActorKind::Wave { origin: pos, yaw, half: 180.0, reach: row.radius, speed: m.tsunami_speed, front: 0.0, spec, hit: Vec::new(), flood: false });
        }
        SpellRef::Ultimate(Element::Aero) => {
            let mut burst = Burst::new(base.clone().kind(Kind::Blow), 0.0).fx(&fx::AERIAL_BLAST);
            burst.line = Some((pos, yaw, row.range, m.aerial_width));
            burst.spec.push = 300.0;
            fxw.add(caster, spell, ActorKind::Delayed { aim: Aim::Point(pos + fwd * 300.0), delay: 0.0, burst });
        }
        SpellRef::Ultimate(Element::Light) => {
            let Some(t) = aim.target() else { return };
            let _ = magic_fx::spawn_fx(cx, caster, Some(spell), fx::HOLY[0], Attach::World(centre), 1.0, false);
            let _ = magic_fx::spawn_fx(cx, caster, Some(spell), fx::HOLY[1], Attach::Target { target: t, offset: Vec3::ZERO }, 1.0, false);
            let mut spec = base.clone().kind(Kind::Blow);
            spec.hit_fx = fx::HOLY[3];
            fxw.add(caster, spell, ActorKind::Delayed { aim, delay: m.holy_delay, burst: Burst::new(spec, row.radius).fx(&[fx::HOLY[2]]) });
        }
        // ---- the hybrids (6.4)
        SpellRef::Hybrid(a, b) => hybrid(fxw, cx, caster, spell, (a, b), &row, base, aim, pos, yaw, mz),
        // ---- Y Arcane (6.3, 6.5)
        SpellRef::Arcane(line, lv) => arcane(fxw, cx, caster, spell, line, lv, &row, base, aim, pos, yaw), // (YFX) + Donald's place and facing
        SpellRef::Release(e) => release_into(fxw, cx, caster, e),
    }
}

/// A persistent body effect for an actor (stopped when the actor ends).
fn attach_body(fxw: &mut MagicFx, cx: &mut Ctx, caster: MemberId, id: u32, name: &'static str, at: Attach) {
    let spell = fxw.actors.iter().find(|a| a.id == id).map(|a| a.spell);
    let h = magic_fx::spawn_fx(cx, caster, spell, name, at, 1.0, true);
    if let Some(a) = fxw.actors.iter_mut().find(|a| a.id == id) {
        a.effect = h;
    }
}

/// The caster's lock-on (else soft) target as an aim (Ball Lightning's hurl).
fn lock_aim(cx: &Ctx, caster: MemberId) -> Option<Aim> {
    let (aim, t) = release_aim(cx, caster);
    t.map(|_| aim)
}

#[allow(clippy::too_many_arguments)]
fn hybrid(fxw: &mut MagicFx, cx: &mut Ctx, caster: MemberId, spell: SpellRef, pair: (Element, Element), row: &super::tune::DonSpellRow, base: HitSpec, aim: Aim, pos: Vec3, yaw: f32, mz: Vec3) {
    use Element::*;
    let m = magic();
    let mut base = base;
    base.hit_fx = super::hybrid_look::hit_fx(pair.0, pair.1); // (HYB) the hybrid's own hit (the Light ones gold)
    let mut main: Option<u32> = None; // (HYB) the spell's main actor, for its look (hybrid_look::launch)
    let ground = aim.ground(cx.host);
    let fwd = crate::pawn::forward(yaw);
    let field = |spec: HitSpec, centre: Vec3, follow: Follow, tick: f32, pull: f32, rule: FieldRule, special: Special| ActorKind::Field { centre, follow, radius: row.radius, life: row.duration, tick, next: 0.0, spec, pull, rule, special };
    match pair {
        (Fire, Blizzard) => {
            let mut spec = base.kind(Kind::Blow);
            spec.def_ignore = row.extra.max(m.thermal_def_ignore);
            fxw.add(caster, spell, ActorKind::Delayed { aim, delay: super::hybrid_look::THERMAL_TRAVEL, burst: Burst::new(spec, 0.0).fx(&fx::THERMAL) } /* (HYB) lands as the two comets meet */);
        }
        (Fire, Thunder) => {
            let mut burst = Burst::new(base, row.radius).fx(&[fx::HIT_FIRE, fx::HIT_THUNDER]);
            burst.special = Special::Plasma { period: row.extra.max(0.2), life: row.duration };
            let dir = (aim.point(cx.host) - mz).normalize_or_zero();
            let id = fxw.add(caster, spell, ActorKind::Ball { pos: mz, aim, speed: m.plasma_speed, turn: m.bolt_turn, dir, life: m.bolt_life * 3.0, shrink: 0.0, arrived: None, burst, after: None });
            main = Some(id); // (HYB)
        }
        (Fire, Water) => {
            let id = fxw.add(caster, spell, field(base, ground, Follow::Fixed, m.scald_tick, 0.0, FieldRule::Area, Special::None));
            main = Some(id); // (HYB)
        }
        (Fire, Aero) => {
            let follow = aim.target().map_or(Follow::Fixed, |t| Follow::Target(t, row.extra.max(m.whirl_speed)));
            let id = fxw.add(caster, spell, field(base, ground, follow, m.whirl_tick, m.whirl_pull, FieldRule::Area, Special::None));
            main = Some(id); // (HYB)
        }
        (Fire, Light) => {
            let id = fxw.add(caster, spell, field(base, ground, Follow::Fixed, m.sunburst_tick, 0.0, FieldRule::Area, Special::Sunburst { bonus: row.extra.max(1.0) }));
            main = Some(id); // (HYB)
        }
        (Blizzard, Thunder) => {
            let id = fxw.add(caster, spell, field(base, ground, Follow::Fixed, m.hail_tick, 0.0, FieldRule::Hail, Special::None));
            main = Some(id); // (HYB)
        }
        (Blizzard, Water) => {
            // a wall of ice in a line toward the target: enemies caught are Frozen; it blocks projectiles for 5 s
            let to = aim.ground(cx.host);
            let dirv = crate::pawn::horizontal(to - pos).normalize_or(fwd);
            let wyaw = crate::pawn::yaw_of(dirv);
            let mut burst = Burst::new(base, 0.0);
            burst.line = Some((pos, wyaw, row.range, m.glacier_width * 0.5));
            fxw.add(caster, spell, ActorKind::Delayed { aim: Aim::Point(pos + dirv * row.range * 0.5), delay: 0.0, burst });
            // (HYB) the wall's look: hybrid_look (its segments burst up along this line)
            fxw.walls.push(Wall { a: pos, b: pos + dirv * row.range, half_width: m.glacier_width * 0.5, height: m.glacier_height, left: row.duration });
        }
        (Blizzard, Aero) => {
            let mut spec = base;
            if !spec.applies.contains(&Apply::Blind) {
                spec.applies.push(Apply::Blind);
            }
            let id = fxw.add(caster, spell, field(spec, pos, Follow::Caster, m.whiteout_tick, 0.0, FieldRule::Area, Special::None));
            main = Some(id); // (HYB)
        }
        (Blizzard, Light) => {
            // [R] rainbow: one shard spray per colour, fanned over the cone
            // (HYB) the shard fan's look: hybrid_look
            let mut burst = Burst::new(base, 0.0);
            burst.arc = Some((pos, yaw, (row.angle * 0.5).max(5.0), row.range));
            burst.special = Special::Prism { count: row.count.max(1) };
            fxw.add(caster, spell, ActorKind::Delayed { aim: Aim::Point(pos + fwd * 200.0), delay: 0.0, burst });
        }
        (Thunder, Water) => {
            let id = fxw.add(caster, spell, field(base, ground, Follow::Fixed, m.storm_tick, 0.0, FieldRule::WetStrikes, Special::None));
            main = Some(id); // (HYB)
        }
        (Thunder, Aero) => {
            let id = fxw.add(caster, spell, field(base, pos + fwd * 200.0, Follow::Drift(fwd * m.tempest_speed), m.tempest_tick, 0.0, FieldRule::Area, Special::None));
            main = Some(id); // (HYB)
        }
        (Thunder, Light) => {
            let start = aim.target().or_else(|| magic_fx::foes(cx.host).into_iter().min_by(|a, b| (a.position - pos).length().total_cmp(&(b.position - pos).length())).map(|e| e.id));
            if let Some(t) = start {
                main = Some(fxw.add(caster, spell, ActorKind::Chain { at: t, pos: mz, left: row.count.saturating_sub(1), range: m.spark_jump_range, gap: m.spark_gap, next: 0.0, spec: base.clone(), hit: Vec::new() })); // (HYB) main
            }
        }
        (Water, Aero) => {
            let id = fxw.add(caster, spell, field(base, ground, Follow::Fixed, m.maelstrom_tick, row.extra.max(0.0), FieldRule::Area, Special::None));
            main = Some(id); // (HYB)
        }
        (Water, Light) => {
            let id = fxw.add(caster, spell, field(base, ground, Follow::Fixed, m.rain_tick, 0.0, FieldRule::Area, Special::None));
            main = Some(id); // (HYB)
        }
        (Aero, Light) => {
            let mut spec = base.kind(Kind::Blow);
            spec.push = row.push;
            if !spec.applies.contains(&Apply::Launch) {
                spec.applies.push(Apply::Launch);
            }
            let mut burst = Burst::new(spec, 0.0); // (HYB) its look: hybrid_look
            burst.arc = Some((pos, yaw, (row.angle * 0.5).max(5.0), row.range));
            burst.special = Special::Gale;
            fxw.add(caster, spell, ActorKind::Delayed { aim: Aim::Point(pos + fwd * 200.0), delay: 0.0, burst });
        }
        _ => {}
    }
    // (HYB) THE LOOK: every effect, sound and timer of the hybrid at its gameplay places (donald/hybrid_look.rs)
    let target = aim.target().and_then(|t| cx.host.targets.info(t)).map(|i| (i.id, i.center, i.position));
    super::hybrid_look::launch(fxw, cx, super::hybrid_look::Launch { caster, spell, target, ground, me: pos, yaw, wand: mz, actor: main, radius: row.radius, range: row.range, angle: row.angle, duration: row.duration });
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn arcane(fxw: &mut MagicFx, cx: &mut Ctx, caster: MemberId, spell: SpellRef, line: ArcaneLine, lv: u8, row: &super::tune::DonSpellRow, base: HitSpec, aim: Aim, me: Vec3, yaw: f32) {
    let m = magic();
    let ground = aim.ground(cx.host);
    let mut base = base;
    base.attr = kh_core::enums::ETresDamageAttribute::Notype;
    base.hit_fx = fx::HIT_ARCANE;
    match line {
        ArcaneLine::Comet => {
            // (YFX) the stars' look and sound on the hits' clock (yfx.rs): the twinkle, the dive, the star blast
            super::yfx::comet(fxw, caster, spell, lv, ground, me, yaw, row.count.max(1), m.comet_gap, m.comet_fall, m.comet_height);
            for k in 0..row.count.max(1) {
                let b = Burst::new(base.clone().kind(Kind::Blow), row.radius);
                fxw.add(caster, spell, ActorKind::Delayed { aim, delay: m.comet_fall + k as f32 * m.comet_gap, burst: b });
            }
        }
        ArcaneLine::Gravity => {
            let mut spec = base.power(0.0);
            spec.hp_share = Some((magic_fx::gravity_base(lv), 0.0)); // (the user, 2026-10-22) the share rule: magic_fx::gravity_share
            spec.push = m.gravity_pull;
            // the sphere (DON-MAGIC's authored effect, its life = the delay + the hold + the implosion) on the ground under the aim
            // (YFX) made bigger only for a target too tall for it (its run-time height), and Gravity's own sounds (yfx.rs)
            let tall = aim.target().and_then(|t| cx.host.targets.info(t)).map_or(0.0, |i| 2.0 * i.half_height);
            let ball = super::yfx::gravity_scale(lv, tall);
            let _ = magic_fx::spawn_fx(cx, caster, Some(spell), fx::GRAVITY[(lv.clamp(1, 3) - 1) as usize], Attach::World(ground), ball, false);
            super::yfx::gravity(fxw, caster, spell, lv, ground, ball);
            let mut b = Burst::new(spec, row.radius).fx(&[fx::GRAVITY[3]]);
            b.special = Special::Gravity { hold: row.duration };
            // (the user, 2026-10-22) the crush lands as the ball starts pressing down
            fxw.add(caster, spell, ActorKind::Delayed { aim, delay: m.gravity_press_at, burst: b });
        }
        ArcaneLine::Meteor => {
            // a sunflower pattern over the radius (deterministic); (YFX) the rift, the burning rocks, their landings (yfx.rs)
            let spots = super::yfx::meteor_spots(ground, row.count.max(1), row.radius);
            super::yfx::meteor(fxw, caster, spell, lv, ground, me, yaw, &spots, m.meteor_gap, m.meteor_fall, m.meteor_height);
            for (k, at) in spots.into_iter().enumerate() {
                let delay = k as f32 * m.meteor_gap;
                let b = Burst::new(base.clone().kind(if lv >= 3 { Kind::Blow } else { Kind::Small }), m.meteor_blast);
                fxw.add(caster, spell, ActorKind::Delayed { aim: Aim::Point(at), delay: delay + m.meteor_fall, burst: b });
            }
        }
        ArcaneLine::Ultima | ArcaneLine::Zettaflare => {
            let (names, delay) = if line == ArcaneLine::Ultima { (fx::ULTIMA, m.ultima_delay) } else { (fx::ZETTAFLARE, m.zetta_delay) };
            let mut spec = base.kind(Kind::Blow);
            spec.ignore_resist = true;
            if line == ArcaneLine::Ultima {
                // (YFX) Ultima's own look and sound: the gathering, the 10 m sphere, the aftermath (yfx.rs)
                super::yfx::ultima(fxw, caster, spell, ground, delay);
                fxw.add(caster, spell, ActorKind::Delayed { aim, delay, burst: Burst::new(spec, row.radius) });
            } else {
                let _ = magic_fx::spawn_fx(cx, caster, Some(spell), names[0], Attach::World(ground), 1.0, false);
                fxw.add(caster, spell, ActorKind::Delayed { aim, delay, burst: Burst::new(spec, row.radius).fx(&[names[1]]) });
            }
        }
    }
}

// ============================================================================================== line 1: Sora's actors
// (DON-MAGIC part A) the magic Donald shares with Sora: donald/sora_magic.rs (Sora's own kh-core actors, cast pose)

// ============================================================================================== the party's step

/// THE PARTY STEP'S PART (party.rs, after the members and the X2 leaders): every Donald's cast (the shot, the end),
/// the Releases, Sora's actors of line 1, the spell actors, the worn spells, the Burn ticks, the statuses' visuals,
/// the heals for Sora.
pub fn party_step(party: &mut Party, dt: f32, sora: &SoraView, host: &mut PartyHost, out: &mut PartyOutput) {
    let mut fxw = std::mem::take(&mut party.don_fx);
    let mut log = std::mem::take(&mut fxw.log);
    let log_start = log.len(); // (DON-SPELLB status looks) this step's hits begin here
    {
        let mut cx = Ctx { party, host, log: &mut log };
        // 0. TEST / DEBUG: the script's casts (`party don-cast ID [sora]`): the first Donald casts at the nearest foe (an
        //    ally spell on himself, or on Sora with `sora`); the rules' price when they allow it, else free
        let on_sora = std::mem::take(&mut fxw.debug_on_sora);
        for spell in std::mem::take(&mut fxw.debug_casts) {
            let Some(id) = cx.party.members().find(|i| cx.party.status(*i).is_some_and(|s| s.donald.is_some())) else { break };
            let Some(p) = cx.party.member_mut(id) else { continue };
            let d = data();
            let Some(row) = spell.row(&d).copied() else { continue };
            let plan = {
                let s = &p.status;
                let st = s.donald.as_ref().unwrap();
                st.core.check_cast(spell, &st.learned, s.mp, s.max_mp(), st.abilities.mods().dual, &d)
            }
            .unwrap_or(CastPlan { spell, mp: 0, pct: 0.0, attuned: true, quick: false, echo: false, cast: row.cast, charges: 0 });
            let pos = p.position();
            let target = if on_sora && spell.ally_target() {
                CastTarget::Member(SORA_MEMBER)
            } else if spell.ally_target() || matches!(spell, SpellRef::Line1(Element::Light, _)) {
                CastTarget::Member(id)
            } else {
                magic_fx::foes(cx.host).into_iter().min_by(|a, b| (a.position - pos).length().total_cmp(&(b.position - pos).length())).map_or(CastTarget::Point(pos + crate::pawn::forward(p.yaw) * 600.0), |f| CastTarget::Enemy(f.id))
            };
            p.don_cast.run = None;
            let ok = begin(p, &plan, target, cx.host);
            cx.log.push(FxLog::Sora { line: if ok { 0xD0 } else { 0xDF }, cmd: 0 });
        }
        // 1. the casts
        let ids: Vec<MemberId> = cx.party.members().collect();
        for id in ids.iter().copied() {
            let Some(p) = cx.party.member_mut(id) else { continue };
            if let Some((_, age)) = p.don_cast.last.as_mut() {
                *age += dt;
            }
            super::spell_look::step(p, cx.host); // (DON-SPELLB) the hold, a gone cast's charge
            let Some(mut run) = p.don_cast.run else { continue };
            run.t += dt;
            let shoot = !run.shot && run.t >= run.shot_at;
            if shoot {
                run.shot = true;
                end_hover(p);
                p.anim.speed = 1.0;
                super::spell_look::at_shot(p, &run, cx.host); // (DON-SPELLB) the release flash at the wand
            }
            p.don_cast.run = if run.t >= run.end_at { None } else { Some(run) };
            if shoot {
                launch(&mut fxw, &mut cx, id, &run, Some(sora));
            }
        }
        // 2. the Releases of this step's attunes
        let released: Vec<(MemberId, Element)> = cx.party.don.events.iter().filter_map(|(id, e)| if let DonEvent::Released { element } = e { Some((*id, *element)) } else { None }).collect();
        for (id, e) in released {
            if let Some(p) = cx.party.member_mut(id) {
                super::spell_look::release_gesture(p, e, cx.host); // (DON-SPELLB) the switch is a move: the wand twirl
            }
            release_into(&mut fxw, &mut cx, id, e);
        }
        // 3. Sora's actors, the spell actors, the worn spells, the Burn ticks, the statuses' visuals
        super::sora_magic::step_casters(&mut fxw, &mut cx, Some(sora), dt); // (DON-MAGIC part A)
        super::spell_payload::step(&mut fxw, &mut cx, dt); // (DON-SPELLB)
        super::yfx::step(&mut fxw, &mut cx, dt); // (YFX) the Y spells' cues, falling bodies and sounds
        magic_fx::step_actors(&mut fxw, cx.party, cx.host, dt);
        super::hybrid_look::step(&mut fxw, &mut cx, dt); // (HYB) the hybrids' bodies follow their actors, their cues
        super::allyfx::step_worn(&mut fxw, &mut cx, Some(sora), dt);
        magic_fx::burn_ticks(&mut cx);
        magic_fx::step_squash(&mut fxw, dt); // (the user, 2026-10-22) Gravity's flatten ages
        magic_fx::sync_status_fx(&mut fxw, cx.party, cx.host, dt, cx.log.get(log_start..).unwrap_or(&[])); // (DON-SPELLB status looks)
        // the Blind records end with the Blind
        let book = &cx.party.don.status;
        fxw.blind_at.retain(|b| book.get(b.0).is_some_and(|s| s.is_blind()));
    }
    fxw.log.extend(log);
    // 4. the heals for Sora (Sanctuary's share: amount -1)
    for (from, amount, effect) in std::mem::take(&mut fxw.sora_heals) {
        let amount = if amount < 0 { ((sora.hp_max.max(1) as f32) * magic().rel_sanctuary_heal).round() as i32 } else { amount };
        let h = SoraHeal { amount, effect, mp: 0, from };
        match sora.ai_sora {
            Some(_) => out.ai_sora_heals.push(h),
            None => out.sora_heals.push(h),
        }
    }
    party.don_fx = fxw;
}

/// The Cast Guard / interruption route of a member's hit (`Party::receive_hit`, marked): a damaging hit that was not
/// guarded interrupts his cast.
pub fn on_member_hit(party: &mut Party, id: MemberId, dealt: f32, guarded: bool) {
    if dealt <= 0.0 || guarded {
        return;
    }
    if let Some(p) = party.member_mut(id) {
        if p.status.donald.is_some() {
            interrupt(p, true);
        }
    }
}
