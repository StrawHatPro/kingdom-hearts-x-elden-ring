//! (HYB) THE LOOK AND SOUND OF DONALD'S 15 HYBRID SPELLS (documentation/DONALD_OVERHAUL.md 6.4; the user, 2026-10-23:
//! "make ALL of the hybrid spells look amazingly polished and unique"). Presentation only: the rules (areas, ticks, damage,
//! statuses, durations) stay in `cast.rs` `hybrid` and `magic_fx`; this module puts the effects where and when the gameplay
//! is, from the cast clip's release on:
//! - THE BODIES: an effect that lives as long as its actor (a field, the Plasma ball): it is placed on the actor every step
//!   (a tornado wandering after its target, a storm front drifting, the ball flying), stopped (fading as authored) when the
//!   actor ends, and its DISSIPATION effects and sound play there at that moment;
//! - THE CUES: effects and sounds timed after the release (the build, the impact, KH3's own big element effects layered on
//!   the authored ones);
//! - THE MOVERS: an effect flown from a point to a target over a time (Thermal Shock's two comets);
//! - THE REPEATERS: something an actor does on its own clock while it lasts (Tempest's bolts as it passes, a storm's sounds);
//! - THE ARCS: a jagged bolt laid from one point to another (`Effects::transform`: turned and stretched), for every chain.
//! The authored effects `vd_hyb_*` are built by build/sora_tmp/party/HYB/tools/make_hyb_fx.js from KH3 emitters; the KH3
//! effects layered here are named where they are used. Light hybrids never heal and never use the Cure flowers: holy damage
//! plus Reveal, in golden-white light. Every sound is one of KH3's own element sounds ("effect:<KH3 effect>": the sound that
//! effect carries), several layered per beat.

use super::element::Element;
use super::magic_fx::{self, Ctx, MagicFx};
use super::spells::SpellRef;
use crate::ids::MemberId;
use glam::Vec3;
use kh_core::host::{Attach, EffectHandle, EffectId, EffectSpawn, PartyMember, SoundId, SoundPlay, TargetFlags, TargetId, Tint};

/// Thermal Shock's two comets fly this long before they meet in the target (its hit lands then too: `cast.rs`).
pub const THERMAL_TRAVEL: f32 = 0.3;
/// Sunburst's sun is drawn at most this high over the target (cm): seen from behind Donald, as KH3's camera frames it.
const SUN_HEIGHT: f32 = 340.0;
/// The shards / feathers of the cones fly at this speed (cm/s): each enemy in the cone is struck when they reach it.
const CONE_SPEED: f32 = 2600.0;

/// Where a cue happens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum At {
    World(Vec3),
    /// A target's centre (its last known centre if it is gone) plus an offset.
    Target(TargetId, Vec3, Vec3),
    /// The ground under a target (its feet) plus an offset.
    Feet(TargetId, Vec3, Vec3),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum What {
    Fx { name: &'static str, scale: f32, yaw: f32, tint: Option<Tint> },
    Sound { name: &'static str, volume: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cue {
    left: f32,
    caster: MemberId,
    spell: SpellRef,
    at: At,
    what: What,
}

/// The dissipation of a body (effects and sounds at its last place when it ends).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fin {
    fx: &'static [(&'static str, f32)],
    sounds: &'static [&'static str],
    /// Removed at once (its break covers it: Glacier's spires) instead of fading as authored.
    cut: bool,
}

const NO_FIN: Fin = Fin { fx: &[], sounds: &[], cut: false };

#[derive(Clone, Copy, Debug, PartialEq)]
enum Owner {
    /// Lives while this actor of `MagicFx::actors` does, placed on it (+ `off`) each step unless `fixed`.
    Actor { id: u32, fixed: bool },
    /// Lives this many seconds more.
    Timer(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Body {
    handle: EffectHandle,
    caster: MemberId,
    spell: SpellRef,
    owner: Owner,
    off: Vec3,
    last: Vec3,
    fin: Fin,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Mover {
    handle: EffectHandle,
    from: Vec3,
    to: At,
    t: f32,
    dur: f32,
    /// Sideways bow of the flight (cm, along `side`).
    bow: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Rep {
    /// Bolts striking the ground at random within `r` of the actor (Tempest).
    Bolts { r: f32 },
    /// A sound at the actor.
    Sound(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Repeater {
    actor: u32,
    caster: MemberId,
    spell: SpellRef,
    every: f32,
    next: f32,
    what: Rep,
}

/// The hybrids' presentation under way (`MagicFx::hyb`).
#[derive(Clone, Debug, Default)]
pub struct HybLook {
    cues: Vec<Cue>,
    bodies: Vec<Body>,
    movers: Vec<Mover>,
    reps: Vec<Repeater>,
    seed: u32,
}

impl HybLook {
    /// Effects and timers still running (tests).
    pub fn busy(&self) -> usize {
        self.cues.len() + self.bodies.len() + self.movers.len() + self.reps.len()
    }

    fn rnd(&mut self) -> f32 {
        // xorshift: deterministic, frame-rate independent (drawn per event, not per frame)
        let mut x = self.seed.wrapping_add(0x9E37_79B9);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn fx(&mut self, caster: MemberId, spell: SpellRef, delay: f32, at: At, name: &'static str, scale: f32, yaw: f32, tint: Option<Tint>) {
        self.cues.push(Cue { left: delay, caster, spell, at, what: What::Fx { name, scale, yaw, tint } });
    }

    fn snd(&mut self, caster: MemberId, spell: SpellRef, delay: f32, at: At, name: &'static str) {
        self.cues.push(Cue { left: delay, caster, spell, at, what: What::Sound { name, volume: 1.0 } });
    }
}

// =============================================================================================== the spawn helpers

fn spawn(cx: &mut Ctx, caster: MemberId, spell: SpellRef, name: &'static str, at: Attach, yaw: f32, scale: f32, tint: Option<Tint>, persistent: bool) -> Option<EffectHandle> {
    let host = &mut *cx.host;
    let m = cx.party.member_mut(caster)?;
    let h = m.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at, rotation: Vec3::new(0.0, yaw, 0.0), scale, persistent });
    if let Some(t) = tint.or_else(|| own_tint(spell, name)).or_else(|| magic_fx::fx::tint_of(Some(spell), name)) {
        host.effects.tint(h, t);
    }
    // a KH3 effect whose own sound other characters' spells also carry (magic_fx's table)
    if let Some(se) = magic_fx::fx::extra_se(name) {
        m.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(se)), name: se, at, volume: 1.0, pitch: 1.0, looping: false, spatial: true });
    }
    Some(h)
}

/// The recolours of the KH3 effects layered here (the authored ones are in their own colours).
fn own_tint(spell: SpellRef, name: &str) -> Option<Tint> {
    match (spell, name) {
        (SpellRef::Hybrid(Element::Fire, Element::Thunder), "vm_thd_str4") => Some(VIOLET),
        _ => None,
    }
}

fn sound(cx: &mut Ctx, caster: MemberId, name: &'static str, at: Vec3, volume: f32) {
    let host = &mut *cx.host;
    let Some(m) = cx.party.member_mut(caster) else { return };
    m.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), volume, pitch: 1.0, looping: false, spatial: true });
}

fn resolve(cx: &Ctx, at: At) -> Vec3 {
    match at {
        At::World(p) => p,
        At::Target(t, last, off) => cx.host.targets.info(t).filter(|i| i.flags.has(TargetFlags::ALIVE)).map_or(last, |i| i.center) + off,
        At::Feet(t, last, off) => cx.host.targets.info(t).filter(|i| i.flags.has(TargetFlags::ALIVE)).map_or(last, |i| i.position) + off,
    }
}

/// Where an actor is now (None: gone or done).
fn actor_point(fxw: &MagicFx, id: u32) -> Option<Vec3> {
    let a = fxw.actors.iter().find(|a| a.id == id && !a.done)?;
    Some(match &a.kind {
        magic_fx::ActorKind::Ball { pos, .. } => *pos,
        magic_fx::ActorKind::Field { centre, .. } => *centre,
        magic_fx::ActorKind::Chain { pos, .. } => *pos,
        magic_fx::ActorKind::Wave { origin, .. } => *origin,
        magic_fx::ActorKind::Delayed { .. } => return None,
    })
}

/// THE ARC: a jagged bolt (vd_hyb_arc / _gold: a 10 m strip down its local -Z from its pivot) laid from `a` to `b`.
pub fn arc(cx: &mut Ctx, caster: MemberId, spell: SpellRef, name: &'static str, a: Vec3, b: Vec3, tint: Option<Tint>) {
    let d = b - a;
    let len = d.length();
    if len < 1.0 {
        return;
    }
    let (rot, scale) = arc_transform(d);
    if let Some(h) = spawn(cx, caster, spell, name, Attach::World(a), 0.0, 1.0, tint, false) {
        cx.host.effects.transform(h, a, rot, scale);
    }
}

/// The rotation (pitch, yaw, roll degrees) and per-axis scale that lay the arc strip (local -Z, 1000 cm) along `d`.
pub fn arc_transform(d: Vec3) -> (Vec3, Vec3) {
    let len = d.length().max(1.0);
    let flat = (d.x * d.x + d.y * d.y).sqrt();
    let yaw = d.y.atan2(d.x).to_degrees();
    let elev = d.z.atan2(flat).to_degrees();
    // a pitch of 90 + elevation turns -Z onto the heading's horizontal, then up by the elevation; the yaw onto the heading
    (Vec3::new(90.0 + elev, yaw, 0.0), Vec3::new(1.0, 1.0, len / 1000.0))
}

// =============================================================================================== the launch

/// What the cast knows at the release.
#[derive(Clone, Copy, Debug)]
pub struct Launch {
    pub caster: MemberId,
    pub spell: SpellRef,
    /// The target (its id and centre / feet when the cast aimed one).
    pub target: Option<(TargetId, Vec3, Vec3)>,
    /// The aim's ground point.
    pub ground: Vec3,
    /// Donald's feet, facing (deg), wand tip.
    pub me: Vec3,
    pub yaw: f32,
    pub wand: Vec3,
    /// The spell's main actor (a field, the ball) when it has one.
    pub actor: Option<u32>,
    pub radius: f32,
    pub range: f32,
    pub angle: f32,
    pub duration: f32,
}

const GOLD: Tint = Tint { color: [1.0, 0.8, 0.38], intensity: 1.2 };
const VIOLET: Tint = Tint { color: [0.78, 0.42, 1.0], intensity: 1.25 };

fn fwd_of(yaw: f32) -> Vec3 {
    Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0)
}

fn body(hl: &mut HybLook, cx: &mut Ctx, l: &Launch, name: &'static str, at: Vec3, yaw: f32, owner: Owner, off: Vec3, fin: Fin) {
    let place = match owner {
        Owner::Actor { .. } => at + off,
        Owner::Timer(_) => at,
    };
    if let Some(h) = spawn(cx, l.caster, l.spell, name, Attach::World(place), yaw, 1.0, None, true) {
        hl.bodies.push(Body { handle: h, caster: l.caster, spell: l.spell, owner, off, last: place, fin });
    }
}

fn now(hl: &mut HybLook, cx: &mut Ctx, l: &Launch, name: &'static str, at: Vec3, yaw: f32, scale: f32, tint: Option<Tint>) {
    let _ = hl;
    let _ = spawn(cx, l.caster, l.spell, name, Attach::World(at), yaw, scale, tint, false);
}

fn sounds(cx: &mut Ctx, l: &Launch, at: Vec3, list: &[&'static str]) {
    for s in list {
        sound(cx, l.caster, s, at, 1.0);
    }
}

fn rep(hl: &mut HybLook, l: &Launch, every: f32, first: f32, what: Rep) {
    if let Some(actor) = l.actor {
        hl.reps.push(Repeater { actor, caster: l.caster, spell: l.spell, every, next: first, what });
    }
}

/// THE RELEASE OF A HYBRID (`cast.rs` `hybrid`, after its actors are made): every effect, sound and timer of its look.
pub fn launch(fxw: &mut MagicFx, cx: &mut Ctx, l: Launch) {
    use Element::*;
    let mut hl = std::mem::take(&mut fxw.hyb);
    hl.seed = hl.seed.wrapping_mul(31).wrapping_add(l.actor.unwrap_or(7)).wrapping_add((l.ground.x.abs() as u32) ^ (l.ground.y.abs() as u32));
    let SpellRef::Hybrid(a, b) = l.spell else {
        fxw.hyb = hl;
        return;
    };
    let fwd = fwd_of(l.yaw);
    let side = Vec3::new(-fwd.y, fwd.x, 0.0);
    let (c, s) = (l.caster, l.spell);
    let tgt = |off: Vec3| l.target.map_or(At::World(l.ground + Vec3::Z * 100.0 + off), |(t, centre, _)| At::Target(t, centre, off));
    let feet = |off: Vec3| l.target.map_or(At::World(l.ground + off), |(t, _, f)| At::Feet(t, f, off));
    let centre = l.target.map_or(l.ground + Vec3::Z * 100.0, |t| t.1);
    let fix = |id: Option<u32>| id.map(|id| Owner::Actor { id, fixed: true });
    match (a, b) {
        // THERMAL SHOCK: a comet of fire and one of ice sweep in from either side and collide in the target
        (Fire, Blizzard) => {
            for (name, k) in [("vd_hyb_ts_fire", 1.0f32), ("vd_hyb_ts_ice", -1.0)] {
                let from = centre + side * (380.0 * k) + Vec3::Z * 260.0 - fwd * 160.0;
                if let Some(h) = spawn(cx, c, s, name, Attach::World(from), 0.0, 1.0, None, true) {
                    hl.movers.push(Mover { handle: h, from, to: tgt(Vec3::ZERO), t: 0.0, dur: THERMAL_TRAVEL, bow: Vec3::Z * 60.0 });
                }
            }
            sounds(cx, &l, centre, &["effect:vm_fir_cal0", "effect:vm_blz_cal0"]);
            hl.fx(c, s, THERMAL_TRAVEL, tgt(Vec3::ZERO), "vd_hyb_ts_boom", 1.0, l.yaw, None);
            hl.fx(c, s, THERMAL_TRAVEL, feet(Vec3::ZERO), "vm_fir_han_015", 0.6, 0.0, None);
            hl.fx(c, s, THERMAL_TRAVEL + 0.03, feet(Vec3::ZERO), "vm_blz_han040", 0.65, 0.0, None);
            for n in ["effect:vm_fir_han_010", "effect:vm_blz_han040", "effect:vs_abn_frz3"] {
                hl.snd(c, s, THERMAL_TRAVEL, tgt(Vec3::ZERO), n);
            }
        }
        // PLASMA: the humming violet ball flies; on contact it detonates (its body's dissipation)
        (Fire, Thunder) => {
            if let Some(id) = l.actor {
                const FIN: Fin = Fin { fx: &[("vd_hyb_pl_boom", 1.0), ("vm_fir_han_025a", 0.6), ("vm_thd_str4", 0.8)], sounds: &["effect:vm_fir_han_020", "effect:vm_thd_str03", "effect:vm_fic_rng0"], cut: false };
                body(&mut hl, cx, &l, "vd_hyb_pl_orb", l.wand, 0.0, Owner::Actor { id, fixed: false }, Vec3::ZERO, FIN);
                rep(&mut hl, &l, 0.45, 0.2, Rep::Sound("effect:vs_abn_cld_thd0"));
            }
            sounds(cx, &l, l.wand, &["effect:vm_thd_cal0", "effect:vm_fir_cal0", "effect:vm_fir_han_010"]);
        }
        // SCALD: a boiling pool and a rolling bank of steam
        (Fire, Water) => {
            const FIN: Fin = Fin { fx: &[("vd_hyb_sc_burst", 0.6)], sounds: &["effect:vs_abn_frz1"], cut: false };
            body(&mut hl, cx, &l, "vd_hyb_sc_body", l.ground, 0.0, fix(l.actor).unwrap_or(Owner::Timer(l.duration)), Vec3::ZERO, FIN);
            now(&mut hl, cx, &l, "vd_hyb_sc_burst", l.ground, 0.0, 1.0, None);
            now(&mut hl, cx, &l, "vp_wfi_han0", l.ground, l.yaw, 1.0, None);
            now(&mut hl, cx, &l, "vm_wat_han040", l.ground, 0.0, 0.55, None);
            sounds(cx, &l, l.ground, &["effect:vn_ex001_wfi_han00", "effect:vm_wat_han040", "effect:vm_fir_cal0"]);
            rep(&mut hl, &l, 1.3, 0.9, Rep::Sound("effect:vm_wat_han010"));
        }
        // FIRE WHIRL: a tornado of fire wandering after its target
        (Fire, Aero) => {
            if let Some(id) = l.actor {
                const FIN: Fin = Fin { fx: &[("vd_hyb_fw_end", 1.0)], sounds: &["effect:vm_fir_han_010", "effect:vm_air_hit0"], cut: false };
                body(&mut hl, cx, &l, "vd_hyb_fw_body", l.ground, 0.0, Owner::Actor { id, fixed: false }, Vec3::ZERO, FIN);
                rep(&mut hl, &l, 1.4, 1.0, Rep::Sound("effect:vm_air_han_040"));
                rep(&mut hl, &l, 1.4, 1.7, Rep::Sound("effect:vm_fir_cal0"));
            }
            now(&mut hl, cx, &l, "vm_fir_han_015", l.ground, 0.0, 0.6, None);
            sounds(cx, &l, l.ground, &["effect:vm_air_han_080", "effect:vm_fir_han_010"]);
        }
        // SUNBURST: a small sun over the target pouring holy light down
        (Fire, Light) => {
            // the sun hangs over the target where the player's camera sees it (the field is on the ground; sunburst.height is
            // its ceiling)
            let up = Vec3::Z * magic_fx::magic().sunburst_height.min(SUN_HEIGHT);
            const FIN: Fin = Fin { fx: &[("vd_hyb_sb_end", 1.0)], sounds: &["effect:vn_ex003_hly_hit0", "effect:vm_fir_han_010"], cut: false };
            let own = fix(l.actor).unwrap_or(Owner::Timer(l.duration));
            body(&mut hl, cx, &l, "vd_hyb_sb_sun", l.ground, 0.0, own, up, FIN);
            body(&mut hl, cx, &l, "vd_hyb_sb_ground", l.ground, 0.0, own, Vec3::ZERO, NO_FIN);
            now(&mut hl, cx, &l, "vd_hyb_sb_ignite", l.ground + up, 0.0, 1.0, None);
            sounds(cx, &l, l.ground + up, &["effect:vn_ex003_hly_cal0", "effect:vm_fir_han_010", "effect:vm_lgt_2"]);
            rep(&mut hl, &l, 1.6, 1.2, Rep::Sound("effect:vm_fir_cal0"));
        }
        // HAILSTORM: a dark cloud pelting hail, lightning inside it
        (Blizzard, Thunder) => {
            const FIN: Fin = Fin { fx: &[], sounds: &["effect:vm_thd_str00"], cut: false };
            body(&mut hl, cx, &l, "vd_hyb_hs_body", l.ground, 0.0, fix(l.actor).unwrap_or(Owner::Timer(l.duration)), Vec3::ZERO, FIN);
            now(&mut hl, cx, &l, "vd_hyb_hs_start", l.ground, 0.0, 1.0, None);
            sounds(cx, &l, l.ground, &["effect:vm_blz_han060", "effect:vm_thd_cal0"]);
            hl.snd(c, s, 0.35, At::World(l.ground), "effect:vm_thd_str00");
            rep(&mut hl, &l, 0.9, 0.6, Rep::Sound("effect:vm_blz_han010"));
        }
        // GLACIER: the wall of ice bursts up segment by segment from Donald toward the target, holds, then breaks
        (Blizzard, Water) => {
            let dir = crate::pawn::horizontal(l.ground - l.me).normalize_or(fwd);
            let wyaw = crate::pawn::yaw_of(dir);
            let n = ((l.range / 150.0).ceil() as usize).max(2);
            for i in 0..n {
                let at = l.me + dir * (l.range * (i as f32 + 0.5) / n as f32);
                let d = 0.04 * i as f32;
                hl.fx(c, s, d, At::World(at), "vd_hyb_gl_rise", 1.0, wyaw, None);
                hl.cues.push(Cue { left: d, caster: c, spell: s, at: At::World(at), what: What::Fx { name: "vd_hyb_gl_seg", scale: -(l.duration - d).max(0.5), yaw: wyaw + 37.0 * i as f32, tint: None } });
            }
            sounds(cx, &l, l.me + dir * l.range * 0.3, &["effect:vm_blz_han060", "effect:vm_blz_han040", "effect:vm_wat_han040"]);
            hl.snd(c, s, 0.18, At::World(l.me + dir * l.range * 0.7), "effect:vs_abn_frz2");
            hl.snd(c, s, l.duration, At::World(l.me + dir * l.range * 0.5), "effect:vs_abn_frz3");
            hl.snd(c, s, l.duration + 0.05, At::World(l.me + dir * l.range * 0.5), "effect:ve_ex731_ded_ice00");
        }
        // WHITEOUT: a cyclone of snow round Donald
        (Blizzard, Aero) => {
            if let Some(m) = cx.party.member(c) {
                let at = Attach::party(PartyMember(c.0), None);
                let feet_at = m.position();
                if let Some(h) = spawn(cx, c, s, "vd_hyb_wo_body", at, 0.0, 1.0, None, true) {
                    const FIN: Fin = Fin { fx: &[("vd_hyb_wo_burst", 0.7)], sounds: &["effect:vm_air_han_040"], cut: false };
                    let owner = l.actor.map_or(Owner::Timer(l.duration), |id| Owner::Actor { id, fixed: true });
                    hl.bodies.push(Body { handle: h, caster: c, spell: s, owner, off: Vec3::ZERO, last: feet_at, fin: FIN });
                }
                now(&mut hl, cx, &l, "vd_hyb_wo_burst", feet_at, 0.0, 1.0, None);
                now(&mut hl, cx, &l, "vm_blz_han040", feet_at, 0.0, 0.7, None);
                sounds(cx, &l, feet_at, &["effect:vm_air_han_080", "effect:vm_blz_han040"]);
                rep(&mut hl, &l, 1.8, 1.4, Rep::Sound("effect:vm_air_han_040"));
            }
        }
        // PRISM FROST: a fan of prismatic ice shards across the cone
        (Blizzard, Light) => {
            now(&mut hl, cx, &l, "vd_hyb_pf_cone", l.me, l.yaw, 1.0, None);
            sounds(cx, &l, l.wand, &["effect:vm_blz_cal0", "effect:vn_ex003_hly_fir2", "effect:vm_blz_han010"]);
            cone_hits(&mut hl, cx, &l, "vd_hyb_pf_hit", &["effect:vs_abn_frz2", "effect:vm_blz_han010"]);
        }
        // THUNDERSTORM: a black storm cloud raining over the field (the strikes and chains: magic_fx WetStrikes + `chain`)
        (Thunder, Water) => {
            const FIN: Fin = Fin { fx: &[], sounds: &["effect:vm_thd_str00"], cut: false };
            body(&mut hl, cx, &l, "vd_hyb_st_body", l.ground, 0.0, fix(l.actor).unwrap_or(Owner::Timer(l.duration)), Vec3::ZERO, FIN);
            now(&mut hl, cx, &l, "vd_hyb_st_start", l.ground, 0.0, 1.0, None);
            sounds(cx, &l, l.ground, &["effect:vn_ex001_wth_cal00", "effect:vm_wat_han060"]);
            hl.snd(c, s, 0.4, At::World(l.ground), "effect:vm_thd_str01");
            rep(&mut hl, &l, 2.2, 1.6, Rep::Sound("effect:vm_wat_bar0"));
        }
        // TEMPEST: the storm front rolls forward, bolts striking as it passes
        (Thunder, Aero) => {
            if let Some(id) = l.actor {
                const FIN: Fin = Fin { fx: &[("vd_hyb_tp_end", 1.0)], sounds: &["effect:vm_air_hit0", "effect:vm_thd_str00"], cut: false };
                body(&mut hl, cx, &l, "vd_hyb_tp_body", l.me + fwd * 200.0, l.yaw, Owner::Actor { id, fixed: false }, Vec3::ZERO, FIN);
                rep(&mut hl, &l, 0.33, 0.15, Rep::Bolts { r: l.radius * 0.8 });
                rep(&mut hl, &l, 1.5, 0.8, Rep::Sound("effect:vm_air_han_120"));
            }
            sounds(cx, &l, l.me + fwd * 200.0, &["effect:vm_air_han_140", "effect:vm_air_han_080", "effect:vm_thd_cal0"]);
        }
        // DIVINE SPARK: the leaps are drawn as they happen (`spark_hop`)
        (Thunder, Light) => {
            sounds(cx, &l, l.wand, &["effect:vn_ex003_hly_fir2", "effect:vm_thd_cal0"]);
        }
        // MAELSTROM: a whirlpool with a waterspout at its heart
        (Water, Aero) => {
            const FIN: Fin = Fin { fx: &[("vd_hyb_ml_end", 1.0)], sounds: &["effect:vm_wat_han040"], cut: false };
            body(&mut hl, cx, &l, "vd_hyb_ml_body", l.ground, 0.0, fix(l.actor).unwrap_or(Owner::Timer(l.duration)), Vec3::ZERO, FIN);
            now(&mut hl, cx, &l, "vp_wai_han0", l.ground, l.yaw, 1.2, None);
            now(&mut hl, cx, &l, "vm_wat_han040", l.ground, 0.0, 0.7, None);
            sounds(cx, &l, l.ground, &["effect:vp_wai_han0", "effect:vm_wat_han060", "effect:vm_air_han_080"]);
            rep(&mut hl, &l, 1.5, 1.0, Rep::Sound("effect:vm_wat_han010"));
            rep(&mut hl, &l, 1.5, 1.75, Rep::Sound("effect:vm_air_han_040"));
        }
        // SHINING RAIN: a canopy of light raining glittering gold
        (Water, Light) => {
            body(&mut hl, cx, &l, "vd_hyb_sr_body", l.ground, 0.0, fix(l.actor).unwrap_or(Owner::Timer(l.duration)), Vec3::ZERO, NO_FIN);
            now(&mut hl, cx, &l, "vd_hyb_sr_start", l.ground, 0.0, 1.0, None);
            sounds(cx, &l, l.ground + Vec3::Z * 500.0, &["effect:vn_ex003_hly_cal0", "effect:vm_lgt_2", "effect:vm_wat_cal0"]);
            rep(&mut hl, &l, 1.7, 1.0, Rep::Sound("effect:vm_wat_han010"));
        }
        // RADIANT GALE: a gust of golden light and feathers across the cone
        (Aero, Light) => {
            now(&mut hl, cx, &l, "vd_hyb_rg_cone", l.me, l.yaw, 1.0, None);
            sounds(cx, &l, l.wand, &["effect:vm_air_han_080", "effect:vm_air_han_120", "effect:vn_ex003_hly_fir2"]);
            cone_hits(&mut hl, cx, &l, "vd_hyb_rg_hit", &["effect:vm_air_hit0", "effect:vn_ex003_hly_hit0"]);
        }
        _ => {}
    }
    let _ = GOLD;
    fxw.hyb = hl;
}

/// A cone's strike on each enemy in it, when the shards / feathers reach it.
fn cone_hits(hl: &mut HybLook, cx: &mut Ctx, l: &Launch, name: &'static str, snd: &[&'static str]) {
    let list = magic_fx::within(cx.host, l.me, l.range, Some((l.yaw, (l.angle * 0.5).max(5.0))));
    for (k, e) in list.into_iter().enumerate() {
        let d = (e.center - l.wand).length() / CONE_SPEED;
        hl.fx(l.caster, l.spell, d, At::Target(e.id, e.center, Vec3::ZERO), name, 1.0, l.yaw, None);
        if k < 3 {
            for n in snd {
                hl.snd(l.caster, l.spell, d, At::Target(e.id, e.center, Vec3::ZERO), n);
            }
        }
    }
}

// =============================================================================================== the hooks from magic_fx

/// DIVINE SPARK'S LEAP (magic_fx's Chain actor, each hop): a golden holy bolt from `from` to the struck enemy, and the burst
/// of holy light on it.
pub fn spark_hop(cx: &mut Ctx, caster: MemberId, spell: SpellRef, from: Vec3, to: Vec3) {
    arc(cx, caster, spell, "vd_hyb_arc_gold", from, to, None);
    let _ = spawn(cx, caster, spell, "vd_hyb_ds_hit", Attach::World(to), 0.0, 1.0, None, false);
    sound(cx, caster, "effect:vm_thd_str01", to, 0.8);
    sound(cx, caster, "effect:vn_ex003_hly_hit0", to, 1.0);
}

/// LIGHTNING CHAINING between enemies (Thunderstorm's Wet, Hailstorm's Chilled): an arc from each to the next.
pub fn chain(cx: &mut Ctx, caster: MemberId, spell: SpellRef, points: &[Vec3]) {
    for w in points.windows(2) {
        arc(cx, caster, spell, "vd_hyb_arc", w[0], w[1], None);
    }
}

/// PLASMA'S BURN ARCING to the next enemy: a violet arc from the last one and the strike on it.
pub fn plasma_arc(cx: &mut Ctx, caster: MemberId, from: Vec3, to: Vec3) {
    let spell = SpellRef::Hybrid(Element::Fire, Element::Thunder);
    arc(cx, caster, spell, "vd_hyb_arc", from, to, Some(VIOLET));
    let _ = spawn(cx, caster, spell, "vd_hyb_pl_arc", Attach::World(to), 0.0, 1.0, None, false);
    sound(cx, caster, "effect:vm_thd_str02", to, 0.8);
    sound(cx, caster, "effect:vm_fir_cal0", to, 1.0);
}

// =============================================================================================== every step

/// EVERY STEP (`cast::party_step`, after the actors moved): the cues due, the bodies placed / ended, the movers, the repeaters.
pub fn step(fxw: &mut MagicFx, cx: &mut Ctx, dt: f32) {
    let mut hl = std::mem::take(&mut fxw.hyb);
    if hl.busy() == 0 {
        fxw.hyb = hl;
        return;
    }
    // the cues
    let mut due = Vec::new();
    for q in hl.cues.iter_mut() {
        q.left -= dt;
        if q.left <= 0.0 {
            due.push(*q);
        }
    }
    hl.cues.retain(|q| q.left > 0.0);
    for q in due {
        let p = resolve(cx, q.at);
        match q.what {
            // a negative scale: a timed body of that many seconds (Glacier's segments)
            What::Fx { name, scale, yaw, tint } if scale < 0.0 => {
                if let Some(h) = spawn(cx, q.caster, q.spell, name, Attach::World(p), yaw, 1.0, tint, true) {
                    const FIN: Fin = Fin { fx: &[("vd_hyb_gl_break", 1.0)], sounds: &[], cut: true };
                    hl.bodies.push(Body { handle: h, caster: q.caster, spell: q.spell, owner: Owner::Timer(-scale), off: Vec3::ZERO, last: p, fin: FIN });
                }
            }
            What::Fx { name, scale, yaw, tint } => {
                let _ = spawn(cx, q.caster, q.spell, name, Attach::World(p), yaw, scale, tint, false);
            }
            What::Sound { name, volume } => sound(cx, q.caster, name, p, volume),
        }
    }
    // the bodies
    let mut ended = Vec::new();
    for bd in hl.bodies.iter_mut() {
        let alive = match &mut bd.owner {
            Owner::Actor { id, fixed } => match actor_point(fxw, *id) {
                Some(p) => {
                    if !*fixed {
                        bd.last = p + bd.off;
                        cx.host.effects.place(bd.handle, bd.last);
                    } else {
                        bd.last = p + bd.off;
                    }
                    true
                }
                None => false,
            },
            Owner::Timer(left) => {
                *left -= dt;
                *left > 0.0
            }
        };
        if !alive {
            ended.push(*bd);
        }
    }
    hl.bodies.retain(|bd| !ended.iter().any(|e| e.handle == bd.handle));
    for bd in ended {
        cx.host.effects.stop(bd.handle, bd.fin.cut);
        // a body attached to Donald (Whiteout) ends where he stands
        let at = if bd.spell == SpellRef::Hybrid(Element::Blizzard, Element::Aero) { cx.party.member(bd.caster).map_or(bd.last, |m| m.position()) } else { bd.last };
        for (name, scale) in bd.fin.fx {
            let _ = spawn(cx, bd.caster, bd.spell, name, Attach::World(at), 0.0, *scale, None, false);
        }
        for n in bd.fin.sounds {
            sound(cx, bd.caster, n, at, 1.0);
        }
    }
    // the movers
    let mut arrived = Vec::new();
    for mv in hl.movers.iter_mut() {
        mv.t += dt;
        let u = (mv.t / mv.dur.max(1e-3)).min(1.0);
        let to = resolve(cx, mv.to);
        let p = mv.from.lerp(to, u * u) + mv.bow * (4.0 * u * (1.0 - u));
        cx.host.effects.place(mv.handle, p);
        if u >= 1.0 {
            arrived.push(mv.handle);
        }
    }
    hl.movers.retain(|m| !arrived.contains(&m.handle));
    for h in arrived {
        cx.host.effects.stop(h, false);
    }
    // the repeaters
    let mut fire = Vec::new();
    for r in hl.reps.iter_mut() {
        r.next -= dt;
        if r.next <= 0.0 {
            r.next += r.every;
            fire.push(*r);
        }
    }
    let alive: Vec<u32> = hl.reps.iter().map(|r| r.actor).filter(|id| actor_point(fxw, *id).is_some()).collect();
    hl.reps.retain(|r| alive.contains(&r.actor));
    for r in fire {
        let Some(p) = actor_point(fxw, r.actor) else { continue };
        match r.what {
            Rep::Sound(n) => sound(cx, r.caster, n, p, 0.9),
            Rep::Bolts { r: rad } => {
                let (a, d) = (hl.rnd() * std::f32::consts::TAU, hl.rnd().sqrt() * rad);
                let at = Vec3::new(p.x + a.cos() * d, p.y + a.sin() * d, p.z);
                let _ = spawn(cx, r.caster, r.spell, "vd_don_thd_str0s", Attach::World(at), 0.0, 0.75, Some(Tint { color: [1.0, 0.92, 0.45], intensity: 1.2 }), false);
            }
        }
    }
    fxw.hyb = hl;
}

/// The hit effect of a hybrid's hits (the field ticks, the cones): the Light hybrids' in gold (holy, never a Cure).
pub fn hit_fx(a: Element, b: Element) -> &'static str {
    use Element::*;
    match (a, b) {
        (Blizzard, Light) | (Water, Light) | (Aero, Light) => magic_fx::fx::HIT_LIGHT,
        (Fire, Water) => magic_fx::fx::HIT_WATER,
        (Fire, Aero) | (Fire, Thunder) | (Fire, Blizzard) | (Fire, Light) => magic_fx::fx::HIT_FIRE,
        (Thunder, Aero) | (Thunder, Water) | (Blizzard, Thunder) | (Thunder, Light) => magic_fx::fx::HIT_THUNDER,
        (Water, Aero) => magic_fx::fx::HIT_WATER,
        _ => magic_fx::fx::hit_of(Some(a)),
    }
}

/// Every effect name this module spawns (the pack harness checks each one).
pub fn all_names() -> Vec<&'static str> {
    let mut v = vec![
        "vd_hyb_ts_fire", "vd_hyb_ts_ice", "vd_hyb_ts_boom", "vm_fir_han_015", "vm_blz_han040", "vd_hyb_pl_orb", "vd_hyb_pl_boom", "vm_fir_han_025a", "vm_thd_str4", "vd_hyb_pl_arc",
        "vd_hyb_sc_body", "vd_hyb_sc_burst", "vp_wfi_han0", "vm_wat_han040", "vd_hyb_fw_body", "vd_hyb_fw_end", "vd_hyb_sb_sun", "vd_hyb_sb_ground", "vd_hyb_sb_ignite", "vd_hyb_sb_end",
        "vd_hyb_hs_body", "vd_hyb_hs_start", "vd_hyb_gl_seg", "vd_hyb_gl_rise", "vd_hyb_gl_break", "vd_hyb_wo_body", "vd_hyb_wo_burst", "vd_hyb_pf_cone", "vd_hyb_pf_hit",
        "vd_hyb_st_body", "vd_hyb_st_start", "vd_hyb_arc", "vd_hyb_arc_gold", "vd_hyb_tp_body", "vd_hyb_tp_end", "vd_don_thd_str0s", "vd_hyb_ds_hit", "vd_hyb_ml_body", "vd_hyb_ml_end", "vp_wai_han0",
        "vd_hyb_sr_body", "vd_hyb_sr_start", "vd_hyb_rg_cone", "vd_hyb_rg_hit",
    ];
    v.sort_unstable();
    v.dedup();
    v
}
