//! THE ENEMY PUPPET (IMPL_ER5_STATE.md): what a hit of Sora's physically does to an Elden Ring
//! enemy is decided by KH3's own enemy-side reaction rules, `crates/kh-react` (one
//! `EnemyReaction` per enemy: the state machine + integrator of `UTresEnemyState_Damage`,
//! docs/sora-enemy-reaction-spec.md). This file is the Elden Ring side of it:
//!
//! - one `EnemyReaction` per hostile character Sora has hit or asked about, with a `Body` from
//!   THE CLASS TABLE (ermap.rs: size class, poise, hit points, explicit per-character rows):
//!   small bodies = Light, man-sized = Medium, armoured ones = Heavy, large bodies and bosses =
//!   Giant (KH3's STRONG hurt shape with a reaction endurance: basic hits never move them);
//! - `hit`: every hit record of the core goes to the enemy's reaction (`ReactionRequest::from_hit`);
//! - `step`: every fixed 60 Hz step each reaction is stepped against the map (one Havok sphere
//!   cast as its `ReactWorld`); while it says `suspended` the body is OURS, handled the way the
//!   passive player is: its own movement and actions are switched off (the two debug flags),
//!   gravity and root motion are off, the fall timer is held, position and facing are written in
//!   both physics phases; the target row of the frame is put at the simulated place so that Sora's
//!   targeting, combo continuation and air-combo height matching see the body where it is;
//! - the ANIMATION is the game's own hit reaction: the damage level of the hit that started the
//!   phase (bullet.rs; a light airborne level for a hit on a body in the air) and a knock-down
//!   asked of the game when the body comes to lie (`ErTargets::landing`);
//! - RELEASE, always: the reaction finished (landed + got up), death, despawn / leaving the area
//!   (found again by its handle through the game's character sets), revenge (KH3's own "this enemy
//!   breaks out"), a warp of Sora, `SUSPEND_MAX` seconds, `puppet = 0`, Sora stopping, the payload
//!   being replaced (`Drop`). Bystanders, NPCs and summons are not hostile: no reaction is ever
//!   made for them.

use std::collections::HashMap;

use eldenring::cs::{ChrIns, FieldInsHandle};
use glam::Vec3;
use kh_core::host::TargetBody;
use kh_core::{HitRecord, TargetId};
use kh_react::{Body, BodyClass, Contact, EnemyReaction, HitOutcome, Phase, Pose, ReactWorld, ReactionRequest};

use crate::coords::{self, Frame, CM_PER_M};
use crate::chranim::{self, AnimNow};
use crate::bossguard::{Guard, GuardRules, Verdict};
use crate::poise::{Gate, Meter, PoiseClass, PoiseRules, Tick};
use crate::ermap::{AnimRules, Params, Weight};
use crate::erlog;
use crate::game;
use crate::targets::Enemy;

/// Longest time one enemy stays suspended without a break (a reaction that never ends is given up).
const SUSPEND_MAX: f32 = 12.0;
/// Entries of characters not seen for this many steps are forgotten.
const FORGET_STEPS: u64 = 60 * 30;

/// What we changed on the character, to put back.
#[derive(Clone, Copy, Debug)]
struct Saved {
    no_move: bool,
    no_action: bool,
    motion_multiplier: f32,
    disable_fall_motion: bool,
    no_update: bool,
    speed: f32,
}

fn save(chr: &ChrIns) -> Saved {
    Saved { no_move: chr.debug_flags.disabled_movement(), no_action: chr.debug_flags.disabled_secondary_actions(), motion_multiplier: chr.modules.physics.motion_multiplier, disable_fall_motion: if game::plausible(crate::word!(chr.modules.fall)) { chr.modules.fall.disable_fall_motion } else { false }, no_update: chr.debug_flags.disabled_updates(), speed: crate::chranim::speed(chr) }
}

/// (testing aid, `dbg.hold`) A body held at a place without any reaction: the AI-suspension check.
struct Held {
    handle: FieldInsHandle,
    npc_id: i32,
    at: Vec3,
    yaw: f32,
    left: f32,
    ai_bits: u32,
    saved: Saved,
    /// (ZETTA 2026-10-23) held across the ground only: its height the game's, gravity on (never left hanging in the air)
    free_y: bool,
    /// (ZETTA 2026-10-23) its AI switch as it was before Zettaflare switched it off (None: not touched); put back with it
    ai_was: Option<bool>,
}

/// (ZETTA 2026-10-23) A body whose AI Zettaflare holds off (SetCharacterAIState off + no actions, written every frame) until
/// the hand-back: its AI switch and action switch as they were.
struct AiLock {
    handle: FieldInsHandle,
    ai_was: Option<bool>,
    no_action_was: bool,
    seen: bool,
}

/// (ZETTA) A Zettaflare-held body watched for animation.
pub struct ZettaWatch {
    pub handle: FieldInsHandle,
    pub last: crate::chranim::AnimNow,
    pub still: f32,
    pub ask_in: f32,
    /// (ZETTA round 5c) hit by the beam: its own hit reaction plays on (never asked back to its idle)
    pub hurt: bool,
    /// (ZETTA round 5c) when its generic hit reaction is asked again
    pub hurt_cadence: HurtCadence,
}

/// (ZETTA round 5c) Seconds between the held, hurt target's re-asked generic hit reactions.
pub const ZETTA_HURT_EVERY: f32 = 0.5;

/// (ZETTA round 5c, the user: "the enemy that is hit should play a hurt animation as a universal rule") THE CADENCE of a
/// held target's hurt in Elden Ring: from the beam's hit its generic hit reaction (a Small one, `Targets::ask_reaction`)
/// is asked at once and then every `ZETTA_HURT_EVERY` s while it stays held and hurt; not hurt (before the hit): none, and
/// the next hurt starts at once again. A reaction asked without a hit: no damage, no numbers (the forced damage of the
/// beam's hit is its own route). UNTESTED IN GAME.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HurtCadence {
    /// Seconds to the next ask; None: not hurt.
    next: Option<f32>,
}

impl HurtCadence {
    /// One call per hold step of `dt` s: is a reaction due now?
    pub fn step(&mut self, hurt: bool, dt: f32) -> bool {
        if !hurt {
            self.next = None;
            return false;
        }
        let left = self.next.map_or(0.0, |n| n - dt.max(0.0));
        if left <= 1e-6 {
            self.next = Some(left + ZETTA_HURT_EVERY);
            if self.next.is_some_and(|n| n <= 0.0) {
                self.next = Some(ZETTA_HURT_EVERY); // (a long step: one ask, never a burst)
            }
            return true;
        }
        self.next = Some(left);
        false
    }
}

/// (ZETTA) Seconds a Zettaflare hold lasts after the party's last ask (it asks every step).
pub const ZETTA_HOLD_LEASE: f32 = 0.5;

/// What a hit asks Elden Ring to play (the `dmgLevel` of its bullet is chosen by it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    None,
    /// A stagger on the spot.
    Flinch,
    /// Thrown off its feet: the animation whose airborne frame is held while the body flies.
    Blow,
}

/// What the body's animation must do in the present phase.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Want {
    /// The game plays what it plays.
    Free,
    /// In the air: the throw animation is held at the time point of this look.
    Air(AirPose),
    /// Lying, getting up: the rest of the throw animation at this speed.
    Down(f32),
    /// Landed on its feet: the rest of the throw animation is rushed through.
    Feet,
}

/// THE LOOK IN THE AIR (IMPL_ER7): three time points of the one throw animation the game gives a
/// body (measured on the soldier's 8100: upright recoil at 0.1 s, falling back at 0.3 s, lying from
/// 0.5 s). An animation only runs forward, so the points are in the order a flight passes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AirPose {
    /// Going up / struck in the air: the recoil, still nearly upright (half of `air_at`).
    Rise,
    /// The apex, thrown back: falling over backwards (`air_at`).
    Hover,
    /// Coming down, slammed: back first, nearly lying (60 % of the way from `air_at` to `down_at`).
    Fall,
}

impl AirPose {
    fn time(self, air_at: f32, down_at: f32) -> f32 {
        match self {
            AirPose::Rise => air_at * 0.5,
            AirPose::Hover => air_at,
            AirPose::Fall => air_at + (down_at - air_at).max(0.0) * 0.6,
        }
    }
}

/// Degrees a second a body turns about its vertical axis while kh-react has it SPUN AWAY (a
/// port-side look: Elden Ring's characters have no spin animation; two turns a second).
const SPIN_RATE: f32 = 720.0;

/// Which `dmgLevel` plays a role's animation on one kind of character (learnt by trying the
/// sheet's order: `AnimRules`).
#[derive(Clone, Copy, Debug, Default)]
struct Memo {
    flinch: Option<u8>,
    flinch_tries: u8,
    blow: Option<u8>,
    blow_tries: u8,
}

/// The newest damage animation of the character's queue (ids x8000..x8999 of its animation set).
pub fn damage_anim(chr: &ChrIns) -> Option<AnimNow> {
    if !game::plausible(crate::word!(chr.modules.time_act)) {
        return None;
    }
    let ta = &chr.modules.time_act;
    (1..=3).map(|back| &ta.anim_queue[((ta.write_idx as usize) + 10 - back) % 10]).find(|a| (8000..9000).contains(&(a.anim_id.rem_euclid(1_000_000)))).map(|a| AnimNow { id: a.anim_id, time: a.play_time, length: a.anim_length })
}

/// A THROW animation (the game's 8100..8699 of a set: blown, flung, knocked up; 8000..8099 are
/// staggers on the spot, 8700 the stance break).
fn is_throw(a: &AnimNow) -> bool {
    (8100..8700).contains(&a.id.rem_euclid(1_000_000))
}

/// No map: for looking a step or two ahead.
struct NoWorld;

impl ReactWorld for NoWorld {
    fn sweep(&self, _start: Vec3, _end: Vec3) -> Option<Contact> {
        None
    }
}

struct Entry {
    want: Want,
    /// The role and level last asked of the game for this body, and when (the puppets' clock).
    asked: Option<(Role, u8, f32)>,
    /// Seconds in the air without a damage animation; the clock of the last extra ask.
    bare: f32,
    extra_ask: f32,
    extra_n: u8,
    /// The body is moved by us (false: a large armoured body, which only plays).
    movable: bool,
    /// Elden Ring's own stance break was seen (edge).
    broken: bool,
    reaction: EnemyReaction,
    /// (IMPL_ER21) What keeps an armoured body from being staggered again and again (bossguard.rs), and whether its
    /// endurance was empty at the last step (the refill is an edge).
    guard: Guard,
    was_empty: bool,
    /// (IMPL_ER45) The poise meter of a body we throw about (poise.rs).
    meter: Meter,
    handle: FieldInsHandle,
    npc_id: i32,
    class: BodyClass,
    /// Present while the body is ours.
    saved: Option<Saved>,
    suspended_for: f32,
    seen_step: u64,
    phase: Phase,
    /// Where it was taken (Elden Ring space) and what we last wrote.
    start: Vec3,
    written: Vec3,
    yaw: f32,
    /// (INT) What was written at the step before (drawn between the two by `reassert_at`, as Sora is).
    int_before: (Vec3, f32),
    peak: f32,
    air_hits: u32,
    /// Degrees the body is turned by the spin of SpunAway.
    spin: f32,
}

#[derive(Default)]
pub struct Puppets {
    /// (ZETTA) Zettaflare's held bodies watched: is each visibly animating (its TimeAct animation moving on)?
    pub zetta_watch: Vec<ZettaWatch>,
    /// (ZETTA 2026-10-23) bodies whose AI Zettaflare holds off until the hand-back ([`Puppets::zetta_ai_lock`])
    ai_locks: Vec<AiLock>,
    /// (ZETTA 2026-10-23) bodies whose animation Zettaflare holds (its heavy hurt at its middle, zpose.rs): no reaction of
    /// ours sets their animation speed, no idle is asked of them; speed 0 written every frame
    zetta_frozen: Vec<FieldInsHandle>,
    map: HashMap<u32, Entry>,
    pub launched: u32,
    /// `pup.ai` of the control file: which switches suspend the enemy (bit 0 the debug flag "no
    /// movement", bit 1 "no secondary actions"; default both).
    pub ai_bits: u32,
    pub draw: bool,
    /// `puppet = 0`: reactions are still kept (the endurance of armoured bodies), nobody is moved.
    pub off: bool,
    /// (IMPL_ER22) How much the world is slowed (1 - the rate of KH3's global game speed during a boss finish; 0 =
    /// normal, the default): the reactions' step is `1/60 x (1 - this)`.
    pub time_slow: f32,
    steps: u64,
    held: Vec<Held>,
    /// The animation rules of the sheet (set every frame by the host) and what was learnt.
    pub rules: AnimRules,
    memo: HashMap<i32, Memo>,
    clock: f32,
    filter: u32,
    /// `pup.anim = 0`: the animation is left to the game (no holding, no speed changes).
    pub no_anim: bool,
    /// (IMPL_ER21) The rules of bossguard.rs (the sheet's rows; `react.guard = 0`: `GuardRules::OFF`), set every frame.
    pub guard: Option<GuardRules>,
    /// (IMPL_ER45) The rules of poise.rs (the sheet's rows; `poise.kh = 0`: `PoiseRules::OFF`), set every frame.
    pub poise: Option<PoiseRules>,
}

/// The map of Elden Ring as kh-react's world: the body as a sphere of its capsule's radius.
struct ErReactWorld {
    frame: Frame,
    filter: u32,
    /// Metres.
    radius: f32,
}

impl ReactWorld for ErReactWorld {
    fn sweep(&self, start: Vec3, end: Vec3) -> Option<Contact> {
        let (a, b) = (self.frame.pos_kh_to_er(start), self.frame.pos_kh_to_er(end));
        let delta = b - a;
        if delta.length_squared() < 1e-10 {
            return None;
        }
        let mut hits = Vec::new();
        crate::havok::cast_sphere(a + Vec3::Y * (self.radius + 0.05), delta, self.radius, self.filter, false, &mut hits);
        // (a body leaving a surface it touches is not blocked by it)
        hits.retain(|h| h.normal.dot(delta) < 0.0);
        hits.sort_by(|x, y| x.fraction.total_cmp(&y.fraction));
        hits.first().map(|h| Contact { time: h.fraction.clamp(0.0, 1.0), normal: coords::unit_er_to_kh(h.normal).normalize_or(Vec3::Z) })
    }
}

/// THE CLASS TABLE's answer as a kh-react body.
pub fn body_of(params: &Params, e: &Enemy) -> Body {
    let class = match (e.puppet_ok, e.weight) {
        (true, Weight::Small) => BodyClass::Light,
        (true, _) => BodyClass::Medium,
        (false, Weight::Large) => BodyClass::Giant,
        (false, _) if e.hp_max as f32 > params.hp_armoured => BodyClass::Giant,
        (false, _) => BodyClass::Heavy,
    };
    let mut body = Body::of(class);
    body.radius = e.radius;
    let slide = params.anim.slide[match e.weight {
        Weight::Small => 0,
        Weight::Medium => 1,
        Weight::Large => 2,
    }];
    body.params.small_power *= slide;
    // (IMPL_ER21) KH3's revenge limit of the armoured classes, and its cool-down = the time the body is unflinchable
    match class {
        BodyClass::Giant => body.revenge_limit = params.guard.revenge_giant,
        BodyClass::Heavy => body.revenge_limit = params.guard.revenge_heavy,
        _ => {}
    }
    if body.strong_max > 0 {
        body.revenge_cooldown = params.guard.revenge_armour;
    }
    body
}

/// The body's feet are high above the ground (a flyer: bats, birds).
fn hovers(e: &Enemy, filter: u32, height_cm: f32) -> bool {
    let mut hits = Vec::new();
    crate::havok::cast_sphere(e.er_pos + Vec3::Y * 0.3, Vec3::NEG_Y * 40.0, 0.15, filter, false, &mut hits);
    let nearest = hits.iter().map(|h| h.fraction).fold(f32::MAX, f32::min);
    nearest < 1.5 && nearest * 40.0 - 0.3 > height_cm / CM_PER_M
}

impl Puppets {
    /// Bodies that are ours now.
    pub fn count(&self) -> usize {
        self.map.values().filter(|p| p.saved.is_some()).count()
    }

    fn entry(&mut self, params: &Params, e: &Enemy) -> &mut Entry {
        let steps = self.steps;
        let filter = if self.filter != 0 { self.filter } else { crate::havok::FILTER_SORA };
        let entry = self.map.entry(e.id.0).or_insert_with(|| {
            let mut body = body_of(params, e);
            body.flying = e.alive && e.active && hovers(e, filter, params.anim.flying_height);
            if body.flying {
                erlog!("puppet: c{:04} (target {}) is a FLYER: its feet are more than {:.1} m above the ground", e.npc_id, e.id.0, params.anim.flying_height / CM_PER_M);
            }
            Entry { want: Want::Free, asked: None, bare: 0.0, extra_ask: -10.0, extra_n: 0, movable: body.class != BodyClass::Giant, broken: false, reaction: EnemyReaction::new(body), guard: Guard::new(), was_empty: false, meter: Meter::new(), handle: e.handle, npc_id: e.npc_id, class: body.class, saved: None, suspended_for: 0.0, seen_step: steps, phase: Phase::None, start: e.er_pos, written: e.er_pos, yaw: e.yaw, int_before: (e.er_pos, e.yaw), peak: 0.0, air_hits: 0, spin: 0.0 }
        });
        entry.seen_step = steps;
        entry
    }

    /// `Targets::body`: the hurt shape's reaction type (DEFAULT, or STRONG with its endurance).
    pub fn body(&self, params: &Params, e: &Enemy) -> TargetBody {
        match self.map.get(&e.id.0) {
            Some(p) => p.reaction.target_body(),
            None => TargetBody { reaction: body_of(params, e).reaction, ..TargetBody::DEFAULT },
        }
    }

    pub fn take_reaction_power(&mut self, params: &Params, e: &Enemy, power: i32) -> i32 {
        let entry = self.entry(params, e);
        // (IMPL_ER21) during its revenge, and for a while after a refill, nothing is taken off the endurance: it holds
        if entry.reaction.body.strong_max > 0 && entry.guard.holds_endurance() {
            return entry.reaction.endurance().max(1);
        }
        entry.reaction.take_reaction_power(power)
    }

    /// (IMPL_ER21) THE POISE DAMAGE of a hit on this body (`row` = its kind's poise number, scaled): none for bodies we
    /// throw about; for armoured ones by the rules of bossguard.rs.
    pub fn poise_for(&mut self, params: &Params, e: &Enemy, row: f32, reaction_power: i32) -> f32 {
        let rules = self.guard.unwrap_or(params.guard);
        let entry = self.entry(params, e);
        if entry.reaction.body.reaction != kh_core::host::BodyReaction::Strong {
            return 0.0;
        }
        entry.guard.poise(&rules, row, reaction_power, e.poise_max)
    }

    pub fn reaction_state(&self, id: TargetId) -> u8 {
        self.map.get(&id.0).map_or(0, |p| p.reaction.reaction_state())
    }

    /// (IMPL_ER21) The body is in its revenge: unflinchable.
    pub fn armoured(&self, id: TargetId) -> bool {
        self.map.get(&id.0).is_some_and(|p| p.guard.armoured())
    }

    /// (XSI, 2026-10-16) Rage Raid's END hit (kh-core abilities/xs/rage.rs): the revenge value and the revenge armour end.
    pub fn end_revenge(&mut self, id: TargetId) {
        if let Some(p) = self.map.get_mut(&id.0) {
            p.reaction.end_revenge();
            p.guard.end_revenge();
        }
    }

    /// The body is in the air in a reaction of ours.
    pub fn airborne(&self, id: TargetId) -> bool {
        self.map.get(&id.0).is_some_and(|p| p.saved.is_some() && matches!(p.phase, Phase::FlinchAir | Phase::Falling | Phase::BlownFlying | Phase::BlownFalling | Phase::SpunAway | Phase::LaunchedRising | Phase::LaunchedHover | Phase::LaunchedFalling | Phase::SlammedDown | Phase::WallHit))
    }

    /// The `dmgLevel` that plays `role` on this kind of character: the one seen to work, else the
    /// next of the sheet's order.
    pub fn level_for(&self, npc_id: i32, role: Role) -> u8 {
        let memo = self.memo.get(&npc_id).copied().unwrap_or_default();
        match role {
            Role::None => 0,
            Role::Flinch => memo.flinch.unwrap_or(self.rules.flinch_levels[memo.flinch_tries as usize % 4]),
            Role::Blow => memo.blow.unwrap_or(self.rules.blow_levels[memo.blow_tries as usize % 4]),
        }
    }

    /// WHERE THE BODY WILL BE `steps` fixed steps from now, relative to where it is (the core's cm):
    /// the game tests a bullet against the body a frame or two after we spawn it, and a body KH3
    /// has just launched rises 25 cm a frame (MEASURED: the launcher's own bullet and two more
    /// passed under a soldier's feet).
    pub fn lead(&self, id: TargetId, steps: u32) -> Vec3 {
        let Some(p) = self.map.get(&id.0).filter(|p| p.movable && p.reaction.is_active()) else { return Vec3::ZERO };
        let mut ahead = p.reaction.clone();
        let from = ahead.position();
        for _ in 0..steps {
            ahead.step(&NoWorld);
        }
        let lead = ahead.position() - from;
        if lead.is_finite() && lead.length() < 200.0 { lead } else { Vec3::ZERO }
    }

    /// A bullet carrying `level` for `role` went out at this body.
    pub fn asked(&mut self, id: TargetId, role: Role, level: u8) {
        let clock = self.clock;
        if let Some(p) = self.map.get_mut(&id.0) {
            p.asked = (role != Role::None).then_some((role, level, clock));
        }
    }

    /// The role a reaction outcome asks for on this body.
    pub fn role_of(&self, id: TargetId, outcome: &HitOutcome, airborne_before: bool) -> Role {
        let movable = self.map.get(&id.0).is_none_or(|p| p.movable);
        match outcome {
            HitOutcome::Started { kind, .. } if kind.value() == 1 && !airborne_before => Role::Flinch,
            HitOutcome::Started { .. } if !movable => Role::Flinch,
            HitOutcome::Started { .. } => Role::Blow,
            _ => Role::None,
        }
    }

    /// The body is KH3's STRONG hurt shape (armoured): Elden Ring's own poise damage is dealt to it.
    pub fn strong(&self, params: &Params, e: &Enemy) -> bool {
        self.body(params, e).reaction == kh_core::host::BodyReaction::Strong
    }

    /// A hit record of the core on this enemy: KH3's reaction rules decide what it does to the body.
    pub fn hit(&mut self, params: &Params, e: &Enemy, hit: &HitRecord, hp_lost: bool, lethal: bool) -> HitOutcome {
        let off = self.off;
        let rules = self.guard.unwrap_or(params.guard);
        let prules = self.poise.unwrap_or(params.poise);
        let entry = self.entry(params, e);
        if off || !e.hostile || !e.alive || !e.active || e.summon {
            return HitOutcome::NoReaction;
        }
        let was_active = entry.reaction.is_active();
        let mut request = ReactionRequest::from_hit(hit, hp_lost, lethal);
        // (ZETTA) a forced hit (Zettaflare): past the poise meter, the revenge armour and the chain cap
        let force = hit.flags & kh_core::host::HIT_FLAG_FORCE != 0;
        // (IMPL_ER45) THE POISE METER of a body we throw about (poise.rs): until it breaks a knight takes no reaction
        // from light hits (a MID one a flinch from the big kinds); armoured bodies keep bossguard.rs, a lethal hit dies
        let mut gate = Gate::Full;
        let mut meter_note = String::new();
        if entry.reaction.body.strong_max == 0 && !lethal && !force {
            let kind = request.kind.value() as u8;
            let row = params.reaction(kind as i32).poise * params.poise_scale;
            let hyper = e.hyper.unwrap_or(prules.hyper_by_anim && e.in_attack);
            let recover = if e.poise_recover > 0.0 { e.poise_recover } else { prules.recover };
            let o = entry.meter.hit(&prules, &rules, e.poise_max, recover, row, crate::poise::big_kind(kind), hyper, was_active);
            gate = o.gate;
            if prules.on && o.class != PoiseClass::Low {
                let what = match (o.broke, o.in_window, o.recovering, o.chain_refused, o.gate) {
                    (true, _, _, _, _) => "BROKEN: the reaction in full".to_string(),
                    (_, true, _, _, _) => format!("in its break window ({:.1} s left): the reaction in full", entry.meter.window_left()),
                    (_, _, true, _, _) => format!("RECOVERING ({:.1} s left): no reaction", entry.meter.recovery_left()),
                    (_, _, _, true, _) => "the meter reached 0 but the chain cap refuses the break: no reaction, the meter is full again".to_string(),
                    (_, _, _, _, Gate::Flinch) => "unbroken: a flinch on the spot instead".to_string(),
                    _ => format!("unbroken{}: no reaction", if hyper { " in its hyper armour" } else if was_active { ", a reaction of ours runs" } else { "" }),
                };
                meter_note = format!(" | poise {:?} {:.0} -> {:.0}/{:.0} (-{:.0}{}): {what}", o.class, o.before.max(0.0), o.after, e.poise_max, o.damage, if hyper { " x hyper armour" } else { "" });
            }
            if o.broke {
                erlog!("poise: target {} (c{:04}, NpcParam {}): POISE BROKEN no. {} by '{}' (kind {:?}, -{:.0} of {:.0}{}): KH3's reaction in full; every hit reacts for {:.1} s (the juggle), then it recovers for {:.1} s and must be broken again", e.id.0, e.npc_id, e.npc_param, entry.meter.breaks, hit.attack_name, hit.damage_kind, o.damage, e.poise_max, if hyper { ", in its hyper armour" } else { "" }, prules.window, prules.lockout);
            }
            if o.chain_refused {
                erlog!("poise: target {} (c{:04}): its poise reached 0 but the chain cap (at most {} breaks in {:.0} s) refuses the break: no reaction, its poise is full again", e.id.0, e.npc_id, rules.chain_max, rules.chain_window);
            }
        }
        // (while a reaction runs the pose is ignored: the reaction's own position is the truth)
        let pose = Pose { position: e.pos, yaw: e.yaw, grounded: true };
        let mut outcome = match gate {
            Gate::None => HitOutcome::NoReaction,
            Gate::Flinch => {
                request.kind = kh_react::ETresDamageKind::Small;
                entry.reaction.hit(&request, pose)
            }
            Gate::Full => entry.reaction.hit(&request, pose),
        };
        // (IMPL_ER21) an armoured body: no reaction during its revenge, and not more than the chain cap lets through
        let mut refused = "";
        if force && !matches!(outcome, HitOutcome::Started { .. }) {
            // (ZETTA) a forced hit the body's own endurance refused: its revenge cleared and asked again with no endurance
            entry.reaction.clear_revenge();
            let saved = entry.reaction.body;
            entry.reaction.body.strong_max = 0;
            outcome = entry.reaction.hit(&request, pose);
            entry.reaction.body = saved;
            if !matches!(outcome, HitOutcome::Started { .. }) {
                erlog!("puppet: target {} (c{:04}): Zettaflare's forced reaction could not be forced ({:?}): the safety net, its hit only", e.id.0, e.npc_id, outcome);
            }
        }
        if !force && entry.reaction.body.strong_max > 0 && matches!(outcome, HitOutcome::Started { .. }) {
            let verdict = entry.guard.reaction(&rules);
            if verdict != Verdict::Allow {
                if !was_active {
                    entry.reaction.release();
                }
                outcome = HitOutcome::NoReaction;
                refused = if verdict == Verdict::Armoured { " (REFUSED: it is in its revenge, unflinchable)" } else { " (REFUSED: the chain cap, its cool-down runs)" };
            }
        }
        // (the time-out counts from the last hit: a long juggle is not cut off in the air)
        entry.suspended_for = 0.0;
        if was_active && entry.saved.is_some() {
            entry.air_hits += 1;
        }
        erlog!(
            "puppet: target {} (c{:04}, {:?} = {:?}, endurance {}, revenge {:.1}) hit '{}' kind {:?}{} -> {:?}{meter_note}",
            e.id.0, e.npc_id, e.weight, entry.class, entry.reaction.endurance(), entry.reaction.revenge_value(), hit.attack_name, hit.damage_kind, if was_active { " while in a reaction" } else { "" }, outcome
        );
        if !refused.is_empty() {
            let left = entry.guard.left();
            erlog!("puppet: target {}{refused}: revenge armour {:.1} s, chain cool-down {:.1} s left", e.id.0, left[0], left[3]);
        }
        outcome
    }

    /// (testing aid, `dbg.launch`) A reaction of `kind` asked of a body without any hit.
    pub fn ask(&mut self, params: &Params, e: &Enemy, kind: kh_core::enums::ETresDamageKind, direction: Vec3) -> HitOutcome {
        let entry = self.entry(params, e);
        entry.reaction.hit(&ReactionRequest::new(kind, direction), Pose { position: e.pos, yaw: e.yaw, grounded: true })
    }

    /// One fixed 60 Hz step of every reaction (called before each of Sora's steps). The rows of
    /// `list` of bodies that are ours are put at the simulated places. `landed`: bodies that came
    /// to lie in this step (the caller asks the game for their knock-down).
    pub fn step(&mut self, list: &mut [Enemy], frame: Frame, filter: u32, landed: &mut Vec<(TargetId, u8)>) {
        self.steps += 1;
        self.clock += kh_react::STEP;
        self.filter = filter;
        let time_scale = (1.0 - self.time_slow).clamp(0.0, 1.0);
        let (steps, ai_bits, off, clock, rules, no_anim) = (self.steps, self.ai_bits, self.off, self.clock, self.rules, self.no_anim);
        let guard_rules = self.guard.unwrap_or(GuardRules::DEFAULT);
        let memo = &mut self.memo;
        let frozen = self.zetta_frozen.clone(); // (ZETTA 2026-10-23)
        let mut taken = 0;
        self.map.retain(|id, p| {
            let row = list.iter_mut().find(|e| e.id.0 == *id);
            let present = row.is_some();
            if present {
                p.seen_step = steps;
            }
            // ---- the release rules that do not come from the reaction
            let why = match &row {
                _ if p.saved.is_none() => None,
                _ if off => Some("puppet switched off"),
                None => Some("it left the area Sora's targets are read in"),
                Some(e) if !e.alive => Some("it died"),
                Some(e) if !e.active => Some("it became inactive"),
                _ if p.suspended_for > SUSPEND_MAX => Some("timeout"),
                _ => None,
            };
            if let Some(why) = why {
                p.reaction.release();
                release(p, *id, why);
                return present;
            }
            // ELDEN RING'S OWN STANCE BREAK on an armoured body empties KH3's reaction endurance: the
            // broken window of the two games is one (kh-react keeps it open for its own 10 s).
            if let Some(e) = row.as_deref() {
                let kneels = crate::targets::find_chr(&p.handle).and_then(|c| damage_anim(c)).is_some_and(|a| a.id.rem_euclid(1_000_000) == 8700);
                let broken = e.alive && (kneels || (e.poise_max > 0.0 && e.poise <= 0.0));
                if broken && !p.broken && p.reaction.body.strong_max > 0 {
                    // (IMPL_ER21) not while the endurance is locked after a refill, not during a revenge; and from now
                    // on no poise damage of ours for the lock-out (bossguard.rs)
                    let empties = p.guard.stance_broken(&guard_rules);
                    let left = if empties { p.reaction.take_reaction_power(10_000) } else { p.reaction.endurance() };
                    erlog!("puppet: target {id} (c{:04}, {:?}): ELDEN RING'S STANCE BREAK no. {} (poise {:.0}/{:.0}{}): {}; no poise damage of ours for {:.0} s", p.npc_id, p.class, p.guard.stance_breaks, e.poise, e.poise_max, if kneels { ", it kneels" } else { "" }, if empties { format!("KH3's reaction endurance emptied (left {left}): hits with reaction power now land in full, at most {} reactions in {:.0} s", guard_rules.chain_max, guard_rules.chain_window) } else { format!("KH3's endurance HOLDS ({left}: locked after its refill, or in its revenge)") }, guard_rules.poise_lockout);
                }
                p.broken = broken;
            }
            let world = ErReactWorld { frame, filter, radius: (p.reaction.body.radius / CM_PER_M).clamp(0.2, 0.5) };
            let out = p.reaction.step_dt(&world, kh_react::STEP * time_scale);
            // (IMPL_ER21) the guard's clock; the endurance's refill (KH3's 10 s window closed) starts its lock-out
            p.guard.tick(kh_react::STEP * time_scale);
            // (IMPL_ER45) the poise meter's clock: the break window closes (it recovers), or it refills unhit
            match p.meter.tick(kh_react::STEP * time_scale) {
                Tick::WindowClosed => erlog!("poise: target {id} (c{:04}): its break window closed: it RECOVERS for {:.1} s (no poise damage, no reaction), then its poise {:.0} must be broken again", p.npc_id, p.meter.recovery_left(), p.meter.value().1),
                Tick::Recovered => erlog!("poise: target {id} (c{:04}): no poise damage for a while: its poise is full again ({:.0})", p.npc_id, p.meter.value().1),
                Tick::Nothing => {}
            }
            if p.reaction.body.strong_max > 0 {
                let empty = p.reaction.endurance() <= 0;
                if p.was_empty && !empty {
                    p.guard.endurance_refilled(&guard_rules);
                    erlog!("puppet: target {id} (c{:04}): KH3's reaction endurance REFILLED ({}): nothing takes from it for {:.0} s", p.npc_id, p.reaction.endurance(), guard_rules.endurance_lockout);
                }
                p.was_empty = empty;
            }
            // ---- what Elden Ring played for the level we asked (learning which level plays a role)
            if let (Some((role, level, at)), Some(chr)) = (p.asked, crate::targets::find_chr(&p.handle)) {
                let since = clock - at;
                // (IMPL_ER21) the stance break (8700) is the game's own answer to poise, not to the level we asked
                let seen = damage_anim(chr).filter(|a| a.time < since + 0.1 && a.id.rem_euclid(1_000_000) != 8700);
                let m = memo.entry(p.npc_id).or_default();
                match (seen, role) {
                    (Some(a), _) if since > 0.03 => {
                        let known = if role == Role::Flinch { &mut m.flinch } else { &mut m.blow };
                        if known.is_none() {
                            *known = Some(level);
                            erlog!("puppet: c{:04}: dmgLevel {level} plays animation {} ({:.2} s) for {role:?}: remembered", p.npc_id, a.id, a.length);
                        }
                        p.asked = None;
                    }
                    (None, _) if since > 0.45 => {
                        let (known, tries) = if role == Role::Flinch { (&m.flinch, &mut m.flinch_tries) } else { (&m.blow, &mut m.blow_tries) };
                        if known.is_none() && *tries < 8 {
                            *tries += 1;
                            erlog!("puppet: c{:04}: dmgLevel {level} started NO damage animation for {role:?} (try {}): the next level of the order is asked next time", p.npc_id, *tries);
                        }
                        p.asked = None;
                    }
                    _ => {}
                }
            }
            // KH3's revenge: the enemy breaks out of the combo.
            if p.reaction.take_revenge() {
                p.reaction.clear_revenge();
                // (IMPL_ER21) an armoured body is never ours to release: its revenge makes it UNFLINCHABLE for a while
                // (no reaction, no damage level, no poise damage: its own attack cannot be interrupted)
                if p.reaction.body.strong_max > 0 && guard_rules.revenge_armour > 0.0 {
                    p.guard.revenge(&guard_rules);
                    erlog!("puppet: target {id} (c{:04}, {:?}): REVENGE no. {} (KH3's revenge value crossed its limit {:.1}): UNFLINCHABLE for {:.1} s (no reaction, no damage level, no poise damage; its endurance holds)", p.npc_id, p.class, p.guard.revenges, p.reaction.body.revenge_limit, guard_rules.revenge_armour);
                }
                if p.saved.is_some() {
                    release(p, *id, "REVENGE: KH3 lets this enemy break out of the combo");
                }
                return true;
            }
            if out.suspended && !p.movable {
                // A large armoured body is never moved: it only plays (the game's own animations).
                p.phase = out.phase;
                return true;
            }
            if out.suspended {
                let Some(row) = row else { return true };
                let er = frame.pos_kh_to_er(out.position);
                let fresh = p.saved.is_none(); // (INT)
                if p.saved.is_none() {
                    // SAFETY: the row was read from the game's character sets in this frame phase.
                    let Some(chr) = crate::targets::find_chr(&p.handle) else { return true };
                    p.saved = Some(save(chr));
                    (p.start, p.peak, p.air_hits, p.suspended_for) = (row.er_pos, 0.0, 0, 0.0);
                    taken += 1;
                    erlog!("puppet: target {id} (c{:04}, {:?}) TAKEN for {:?} ({:?}); its animation was {}", p.npc_id, p.class, out.kind, out.phase, row.anim);
                }
                if out.phase != p.phase {
                    erlog!("puppet: target {id}: {:?} -> {:?} at height {:+.2} m, {:.2} m from where it was taken; its animation is {}", p.phase, out.phase, er.y - p.start.y, ((er - p.start) * Vec3::new(1.0, 0.0, 1.0)).length(), row.anim);
                    p.phase = out.phase;
                    let anim = crate::targets::find_chr(&p.handle).and_then(|c| damage_anim(c));
                    p.want = match out.phase {
                        Phase::FlinchAir | Phase::LaunchedRising => Want::Air(AirPose::Rise),
                        Phase::BlownFlying | Phase::SpunAway | Phase::LaunchedHover | Phase::WallHit => Want::Air(AirPose::Hover),
                        Phase::Falling | Phase::BlownFalling | Phase::LaunchedFalling | Phase::SlammedDown => Want::Air(AirPose::Fall),
                        Phase::Down | Phase::GettingUp => match p.want {
                            Want::Down(rate) => Want::Down(rate),
                            _ => {
                                // The rest of the throw animation (lying, getting up) is played in the
                                // time KH3's down clip takes.
                                let clips = &p.reaction.body.clips;
                                let total = match out.kind.value() {
                                    2 | 5 | 10 | 11 => clips.blow_down,
                                    3 | 4 => clips.spin_down,
                                    _ => clips.toss_down,
                                }
                                .max(0.3);
                                let (_, down_at) = rules.timing(p.npc_id);
                                let rest = anim.map_or(total, |a| (a.length - down_at.max(a.time)).max(0.1));
                                erlog!("puppet: target {id}: lying and getting up: {:.2} s of animation {} in KH3's {total:.2} s (speed {:.2})", rest, anim.map_or(0, |a| a.id), (rest / total).clamp(0.6, 3.0));
                                Want::Down((rest / total).clamp(0.6, 3.0))
                            }
                        },
                        Phase::LandFeet => Want::Feet,
                        _ => Want::Free,
                    };
                }
                // In the air without a throw animation (a launch without a bullet, a bullet that
                // missed, an animation the game ended): one is asked for, at most twice a second.
                if matches!(p.want, Want::Air(_)) && !no_anim {
                    let has = crate::targets::find_chr(&p.handle).and_then(|c| damage_anim(c)).is_some_and(|a| is_throw(&a));
                    p.bare = if has { 0.0 } else { p.bare + kh_react::STEP };
                    if has {
                        p.extra_n = 0;
                    }
                    if p.bare > 0.12 && clock - p.extra_ask > 0.5 && p.asked.is_none() {
                        p.extra_ask = clock;
                        let m = memo.get(&p.npc_id).copied().unwrap_or_default();
                        // (the level known to work first; when that did not start one this time, the others of the order in turn)
                        let level = match (m.blow, p.extra_n) {
                            (Some(known), 0) => known,
                            (_, n) => rules.blow_levels[(m.blow_tries as usize + n as usize) % 4],
                        };
                        p.extra_n = p.extra_n.wrapping_add(1);
                        p.asked = Some((Role::Blow, level, clock));
                        landed.push((TargetId(*id), level));
                    }
                }
                p.suspended_for += kh_react::STEP;
                p.peak = p.peak.max(er.y - p.start.y);
                // SPUN AWAY (SIDESPIN / VERTICALROLL): the body turns about its vertical axis while it flies.
                p.spin = if out.phase == Phase::SpunAway && !no_anim { (p.spin + SPIN_RATE * kh_react::STEP) % 360.0 } else { 0.0 };
                // (INT) the step before's place (taken at this step: where the game had it; its new facing at once)
                p.int_before = if fresh { (row.er_pos, out.yaw + p.spin) } else { (p.written, p.yaw) };
                (p.written, p.yaw) = (er, out.yaw + p.spin);
                if let Some(chr) = crate::targets::find_chr(&p.handle) {
                    write(chr, p.written, p.yaw, ai_bits);
                    if !no_anim && !frozen.contains(&p.handle) {
                        animate(chr, p, &rules);
                    }
                }
                row.er_pos = er;
                row.pos = out.position;
                row.velocity = out.velocity;
                row.yaw = out.yaw;
                row.airborne = !out.grounded;
            } else if p.saved.is_some() {
                p.phase = out.phase;
                p.want = Want::Free;
                release(p, *id, if out.finished { "the reaction is over" } else { "the reaction no longer holds it" });
            } else {
                p.phase = out.phase;
            }
            present || steps.saturating_sub(p.seen_step) < FORGET_STEPS
        });
        self.launched += taken;
    }

    /// The positions again after the game's own physics (`ChrIns_PostPhysics`).
    pub fn reassert(&self) {
        self.reassert_at(1.0);
    }

    /// (INT) As `reassert`, the bodies we carry `alpha` of the way from the step before to the latest one: the game draws
    /// them where they are written last in the frame, between the steps as Sora and his effects are (a jump of more than
    /// 3 m between the steps is not blended). 1 = the latest step (as before).
    pub fn reassert_at(&self, alpha: f32) {
        for p in self.map.values().filter(|p| p.saved.is_some()) {
            if let Some(chr) = crate::targets::find_chr(&p.handle) {
                let (at, yaw) = if alpha < 1.0 && p.int_before.0.distance(p.written) < 3.0 {
                    (p.int_before.0.lerp(p.written, alpha), p.int_before.1 + ((p.yaw - p.int_before.1 + 540.0).rem_euclid(360.0) - 180.0) * alpha)
                } else {
                    (p.written, p.yaw)
                };
                write(chr, at, yaw, self.ai_bits);
                if !self.no_anim && !self.zetta_frozen.contains(&p.handle) {
                    animate(chr, p, &self.rules);
                }
            }
        }
        for h in &self.held {
            if let Some(chr) = crate::targets::find_chr(&h.handle) {
                write(chr, h.at, h.yaw, h.ai_bits);
            }
        }
        if self.draw {
            if let Some(mut draw) = game::Draw::get() {
                draw.colour(game::MAGENTA);
                for p in self.map.values().filter(|p| p.saved.is_some()) {
                    draw.cross(p.written, 0.4);
                    draw.line(p.written, Vec3::new(p.written.x, p.start.y, p.written.z));
                }
            }
        }
    }

    /// (testing aid) The body is held `height` metres above where it is for `seconds`, with the
    /// suspension switches `ai_bits`; nothing else is done to it.
    pub fn hold(&mut self, e: &Enemy, height: f32, seconds: f32, ai_bits: u32) -> bool {
        if !e.hostile || !e.alive || !e.active || self.held.iter().any(|h| h.handle == e.handle) || self.map.get(&e.id.0).is_some_and(|p| p.saved.is_some()) {
            return false;
        }
        let Some(chr) = crate::targets::find_chr(&e.handle) else { return false };
        self.held.push(Held { handle: e.handle, npc_id: e.npc_id, at: e.er_pos + Vec3::Y * height, yaw: e.yaw, left: seconds, ai_bits, saved: save(chr), free_y: false, ai_was: None });
        true
    }

    /// (ZETTA) ZETTAFLARE'S HELD TARGET (kh_core `Targets::script_hold`; the user: "the enemy needs to freeze, look at
    /// Donald" and "make sure the enemy is never shown during the cutscene without an appropriate animation"): the body is
    /// held where it stands, its own movement and actions switched off (switches 1 | 2, the passive player's route; its
    /// ANIMATION RUNS: never stood still), asked once for its idle through the game's behaviour events (`W_Idle`,
    /// chranim.rs `fire`), turned toward `face_yaw` (KH3 yaw) by at most `turn` degrees per call (its idle plays while it
    /// turns); each call keeps it `ZETTA_HOLD_LEASE` s more, so a hold the party stops asking for (party off, Sora stopped,
    /// a crash of its step) gives the body back by itself. Refused for a body a reaction owns, a dead / inactive / friendly
    /// one. Bosses too (the user did not exempt them); UNTESTED IN GAME.
    pub fn zetta_hold(&mut self, e: &Enemy, face_yaw: f32, turn: f32, hurt: bool) -> bool {
        // (ZETTA round 5c) hit: its own hit reaction plays on (the game's damage animation of the beam's hit), never asked
        // back to its idle (UNTESTED IN GAME; a looping hurt needs the enemy's own damage behaviour event: see the report)
        if let Some(w) = self.zetta_watch.iter_mut().find(|w| w.handle == e.handle) {
            w.hurt = hurt;
        }
        if let Some(h) = self.held.iter_mut().find(|h| h.handle == e.handle) {
            let diff = ((face_yaw - h.yaw + 540.0) % 360.0) - 180.0;
            h.yaw += diff.clamp(-turn.abs(), turn.abs());
            h.left = h.left.max(ZETTA_HOLD_LEASE);
            return true;
        }
        if !self.hold(e, 0.0, ZETTA_HOLD_LEASE, 1 | 2) {
            return false;
        }
        // (2026-10-23, the user: "if an enemy does not die from Zettaflare they need to be thrown back during the longshot not
        // kept up in the air") held across the ground only: a body taken in the air comes down onto the ground
        if let Some(h) = self.held.last_mut() {
            h.free_y = true;
            // (2026-10-23, the user: "Melania was also attacking during Zettaflare!") its AI off too (the game's own
            // SetCharacterAIState, put back with the body): no action of its own from the cast on
            if let Some(chr) = crate::targets::find_chr(&h.handle) {
                h.ai_was = ai_switch::ai_on(chr);
                let _ = ai_switch::set_ai_on(chr, false);
            }
        }
        let asked = crate::chranim::available() && crate::targets::find_chr(&e.handle).is_some_and(|chr| crate::chranim::fire(chr, "W_Idle"));
        self.zetta_watch.retain(|x| x.handle != e.handle);
        self.zetta_watch.push(ZettaWatch { handle: e.handle, last: crate::chranim::AnimNow::default(), still: 0.0, ask_in: 1.0, hurt, hurt_cadence: HurtCadence::default() });
        erlog!("puppet: Zettaflare holds c{:04} (target {}): in place, its own animation running (idle asked: {asked}), turning to Donald", e.npc_id, e.id.0);
        true
    }

    /// (ZETTA 2026-10-23, the user: "Melania was also attacking during Zettaflare!") A BODY ZETTAFLARE KILLED, KEPT UNTIL THE
    /// HAND-BACK: held where it stands (across the ground), no movement, no actions, its AI off (`zetta_hold`'s hold), whatever
    /// KH3's side says of it (dead) and whether the game draws it (hidden by the crumble: then frozen outright, `hidden`: the
    /// game's "no updates" switch, so not even an attack it had begun goes on). Its lease is renewed by every call; the
    /// hand-back's `zetta_release` gives it back. False: the game has no such character (or it is dead there).
    pub fn zetta_keep(&mut self, e: &Enemy, hidden: bool) -> bool {
        let bits = zetta_keep_bits(hidden);
        if let Some(h) = self.held.iter_mut().find(|h| h.handle == e.handle) {
            h.left = h.left.max(ZETTA_HOLD_LEASE);
            h.ai_bits = bits;
            if h.ai_was.is_none() {
                if let Some(chr) = crate::targets::find_chr(&e.handle) {
                    h.ai_was = ai_switch::ai_on(chr);
                }
            }
            return true;
        }
        let Some(chr) = crate::targets::find_chr(&e.handle) else { return false };
        if chr.modules.data.hp <= 0 {
            return false;
        }
        let ai_was = ai_switch::ai_on(chr);
        let _ = ai_switch::set_ai_on(chr, false);
        self.held.push(Held { handle: e.handle, npc_id: e.npc_id, at: e.er_pos, yaw: e.yaw, left: ZETTA_HOLD_LEASE, ai_bits: bits, saved: save(chr), free_y: true, ai_was });
        erlog!("puppet: Zettaflare KEEPS the body it killed, c{:04} (target {}), until the hand-back: held, no movement, no actions, AI off{}", e.npc_id, e.id.0, if hidden { ", frozen (hidden)" } else { "" });
        true
    }

    /// (ZETTA 2026-10-23) A body whose AI Zettaflare holds off until the hand-back (a survivor in its throw, the target's
    /// phase rows on its spot): SetCharacterAIState off and no actions, written again every call (the game puts them back);
    /// `zetta_ai_unlock` (or a frame without the call: [`Puppets::zetta_ai_frame`]) gives them back as they were.
    pub fn zetta_ai_lock(&mut self, handle: &FieldInsHandle) {
        let Some(chr) = crate::targets::find_chr(handle) else { return };
        if !self.ai_locks.iter().any(|l| l.handle == *handle) {
            self.ai_locks.push(AiLock { handle: *handle, ai_was: ai_switch::ai_on(chr), no_action_was: chr.debug_flags.disabled_secondary_actions(), seen: true });
        }
        if let Some(l) = self.ai_locks.iter_mut().find(|l| l.handle == *handle) {
            l.seen = true;
        }
        let _ = ai_switch::set_ai_on(chr, false);
        chr.debug_flags.set_disabled_secondary_actions(true);
    }

    /// (ZETTA 2026-10-23) Once a frame after the locks were asked: a lock not asked this frame is given back.
    pub fn zetta_ai_frame(&mut self) {
        self.ai_locks.retain_mut(|l| {
            if std::mem::take(&mut l.seen) {
                return true;
            }
            ai_unlock(l);
            false
        });
    }

    /// (ZETTA 2026-10-23) The body's AI lock given back (the hand-back).
    pub fn zetta_ai_unlock(&mut self, handle: &FieldInsHandle) {
        self.ai_locks.retain_mut(|l| {
            if l.handle != *handle {
                return true;
            }
            ai_unlock(l);
            false
        });
    }

    /// (ZETTA 2026-10-23) Zettaflare holds (`on`) the body's animation where it is (zpose.rs), or lets it go (its speed back
    /// to 1 unless a hold gives it back itself).
    pub fn zetta_freeze(&mut self, handle: &FieldInsHandle, on: bool) {
        let was = self.zetta_frozen.contains(handle);
        self.zetta_frozen.retain(|h| h != handle);
        if on {
            self.zetta_frozen.push(*handle);
            if let Some(chr) = crate::targets::find_chr(handle) {
                crate::chranim::set_speed(chr, 0.0);
            }
        } else if was {
            if let Some(chr) = crate::targets::find_chr(handle) {
                crate::chranim::set_speed(chr, 1.0);
            }
        }
    }

    /// (ZETTA 2026-10-23) Is the body's animation held by Zettaflare?
    pub fn zetta_frozen(&self, handle: &FieldInsHandle) -> bool {
        self.zetta_frozen.contains(handle)
    }

    /// (ZETTA 2026-10-23) Is the body's AI held off by Zettaflare (a hold or a lock)?
    pub fn zetta_ai_held(&self, handle: &FieldInsHandle) -> bool {
        self.ai_locks.iter().any(|l| l.handle == *handle) || self.held.iter().any(|h| h.handle == *handle && h.ai_was.is_some())
    }

    /// (ZETTA round 5c) Is the held body's generic hit reaction due this hold step (see [`HurtCadence`])? False for a body
    /// Zettaflare does not hold.
    pub fn zetta_hurt_due(&mut self, handle: &FieldInsHandle, hurt: bool, dt: f32) -> bool {
        self.zetta_watch.iter_mut().find(|w| w.handle == *handle).is_some_and(|w| w.hurt_cadence.step(hurt, dt))
    }

    /// (ZETTA) The held target given back (the hit, a cancel, an interrupt): as `release_held` for that body only.
    pub fn zetta_release(&mut self, handle: &FieldInsHandle) {
        self.zetta_watch.retain(|x| x.handle != *handle);
        self.zetta_frozen.retain(|h| h != handle); // (ZETTA 2026-10-23) the hold's give-back puts its speed back
        self.held.retain(|h| {
            if h.handle != *handle {
                return true;
            }
            if let Some(chr) = crate::targets::find_chr(&h.handle) {
                give_back(chr, &h.saved);
                give_ai_back(chr, h.ai_was); // (ZETTA 2026-10-23)
                erlog!("puppet: Zettaflare gives c{:04} back", h.npc_id);
            }
            false
        });
        self.zetta_ai_unlock(handle); // (ZETTA 2026-10-23)
    }

    /// The held body's place (Elden Ring space), while it is held.
    pub fn held_at(&self, handle: &FieldInsHandle) -> Option<Vec3> {
        self.held.iter().find(|h| h.handle == *handle).map(|h| h.at)
    }

    /// (ZETTA) THE GENERIC "IS IT ANIMATING" RULE for Zettaflare's held bodies (no per-enemy data): its newest TimeAct
    /// animation (id, time) is read every frame; one that has not moved for a moment is asked for its idle again through
    /// the game's own behaviour events (`W_Idle`, at most once a second); `animating` = it moved within the last 0.3 s.
    pub fn zetta_tick(&mut self, dt: f32) {
        for w in self.zetta_watch.iter_mut() {
            let Some(chr) = crate::targets::find_chr(&w.handle) else { continue };
            let now = crate::chranim::now(chr);
            w.still = if now == w.last { w.still + dt } else { 0.0 };
            w.last = now;
            w.ask_in -= dt;
            if !w.hurt && w.still > 0.25 && w.ask_in <= 0.0 && crate::chranim::available() && !self.zetta_frozen.contains(&w.handle) {
                let _ = crate::chranim::fire(chr, "W_Idle");
                w.ask_in = 1.0;
            }
        }
    }

    /// (ZETTA) Is this held body visibly animating now (see [`Puppets::zetta_tick`])?
    pub fn zetta_animating(&self, handle: &FieldInsHandle) -> bool {
        self.zetta_watch.iter().any(|w| w.handle == *handle && w.still < 0.3)
    }

    /// Once a frame: held bodies are written; a hold that ran out gives its body back.
    pub fn hold_tick(&mut self, dt: f32) {
        self.zetta_tick(dt); // (ZETTA)
        let zetta_frozen = self.zetta_frozen.clone(); // (ZETTA 2026-10-23)
        self.held.retain_mut(|h| {
            h.left -= dt;
            let chr = crate::targets::find_chr(&h.handle);
            match chr {
                Some(chr) if h.left > 0.0 && chr.modules.data.hp > 0 => {
                    if h.free_y {
                        // (ZETTA 2026-10-23) its height the game's (on the ground, falling onto it), gravity on
                        let now = game::v3(&chr.modules.physics.position);
                        write(chr, Vec3::new(h.at.x, now.y, h.at.z), h.yaw, h.ai_bits);
                        chr.modules.physics.as_mut().gravity_disabled = false;
                    } else {
                        write(chr, h.at, h.yaw, h.ai_bits);
                    }
                    // (ZETTA 2026-10-23) its AI kept off every frame (the game's events may switch it back on)
                    if h.ai_was.is_some() {
                        let _ = ai_switch::set_ai_on(chr, false);
                    }
                    // (ZETTA 2026-10-23) its heavy hurt held at its middle (zpose.rs)
                    if zetta_frozen.contains(&h.handle) {
                        crate::chranim::set_speed(chr, 0.0);
                    }
                    true
                }
                Some(chr) => {
                    give_back(chr, &h.saved);
                    give_ai_back(chr, h.ai_was); // (ZETTA 2026-10-23)
                    erlog!("puppet: the held c{:04} was given back (hold over)", h.npc_id);
                    false
                }
                None => false,
            }
        });
    }

    pub fn release_held(&mut self) {
        self.zetta_frozen.clear(); // (ZETTA 2026-10-23)
        for mut l in std::mem::take(&mut self.ai_locks) {
            ai_unlock(&mut l); // (ZETTA 2026-10-23)
        }
        for h in self.held.drain(..) {
            if let Some(chr) = crate::targets::find_chr(&h.handle) {
                give_back(chr, &h.saved);
                give_ai_back(chr, h.ai_was); // (ZETTA 2026-10-23)
                erlog!("puppet: the held c{:04} was given back", h.npc_id);
            }
        }
    }

    /// Every body goes back to the game (a warp of Sora, the key, Sora stopping).
    pub fn release_all(&mut self, why: &str) {
        self.release_held();
        for (id, p) in self.map.iter_mut() {
            p.reaction.release();
            if p.saved.is_some() {
                release(p, *id, why);
            }
        }
    }
}

impl Drop for Puppets {
    fn drop(&mut self) {
        if self.count() > 0 || !self.held.is_empty() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.release_all("Sora stopped")));
        }
    }
}

/// The body goes back to the game: gravity, root motion and its own actions as they were.
fn release(p: &mut Entry, id: u32, why: &str) {
    let Some(saved) = p.saved.take() else { return };
    let found = match crate::targets::find_chr(&p.handle) {
        Some(chr) => {
            give_back(chr, &saved);
            "given back"
        }
        None => "gone from the game (despawned / unloaded): nothing to give back",
    };
    erlog!("puppet: target {id} (c{:04}) RELEASED after {:.2} s ({why}): {found}; {} hit(s) while ours, peak {:+.2} m, {:.2} m from where it was taken, last phase {:?}", p.npc_id, p.suspended_for, p.air_hits, p.peak, ((p.written - p.start) * Vec3::new(1.0, 0.0, 1.0)).length(), p.phase);
    p.suspended_for = 0.0;
    p.phase = Phase::None;
}

/// THE ANIMATION OF THE PHASE: the throw animation Elden Ring started for our bullet is held at
/// its airborne frame while the body flies (animation speed 0), and its rest (lying, getting up)
/// runs when the body has landed, in the time KH3's down clip takes.
fn animate(chr: &mut ChrIns, p: &Entry, rules: &AnimRules) {
    let (air_at, down_at) = rules.timing(p.npc_id);
    let anim = damage_anim(chr);
    let speed = match (p.want, anim) {
        (Want::Free, _) | (_, None) => 1.0,
        (Want::Air(pose), Some(a)) if a.time < pose.time(air_at, down_at) || !is_throw(&a) => 1.0,
        (Want::Air(_), Some(_)) => 0.0,
        (Want::Down(_), Some(a)) if a.time < down_at => 6.0,
        (Want::Down(rate), Some(_)) => rate,
        (Want::Feet, Some(_)) => 6.0,
    };
    chranim::set_speed(chr, speed);
}

fn write(chr: &mut ChrIns, at: Vec3, yaw: f32, ai_bits: u32) {
    let q = game::quaternion(coords::quat_er_from_yaw(yaw));
    let physics = chr.modules.physics.as_mut();
    physics.position = game::hp(at);
    physics.orientation = q;
    physics.interpolated_orientation = q;
    // (the game resets these every frame: ER1 state note, the passive player)
    physics.gravity_disabled = true;
    physics.motion_multiplier = 0.0;
    if game::plausible(crate::word!(chr.modules.fall)) {
        let fall = chr.modules.fall.as_mut();
        fall.fall_timer = 0.0;
        fall.disable_fall_motion = true;
    }
    if ai_bits & 1 != 0 {
        chr.debug_flags.set_disabled_movement(true);
    }
    if ai_bits & 2 != 0 {
        chr.debug_flags.set_disabled_secondary_actions(true);
    }
    // (experiments of IMPL_ER6: 4 = the animation stands still, 8 = the game's "no updates" flag)
    if ai_bits & 4 != 0 {
        crate::chranim::set_speed(chr, 0.0);
    }
    if ai_bits & 8 != 0 {
        chr.debug_flags.set_disabled_updates(true);
    }
}

/// (ZETTA 2026-10-23) The switches a body Zettaflare keeps is held with (`write`'s `ai_bits`): no movement (1), no actions:
/// no attack, jump or any other (2); hidden by the crumble: no updates at all (8: not even an attack it had begun goes on).
pub fn zetta_keep_bits(hidden: bool) -> u32 {
    1 | 2 | if hidden { 8 } else { 0 }
}

/// (ZETTA 2026-10-23) The AI switch (sdkx.rs, built with `party`); without it the switch is never touched.
mod ai_switch {
    use eldenring::cs::ChrIns;
    #[cfg(all(windows, feature = "party"))]
    pub fn ai_on(chr: &ChrIns) -> Option<bool> {
        crate::sdkx::ai_on(chr)
    }
    #[cfg(all(windows, feature = "party"))]
    pub fn set_ai_on(chr: &mut ChrIns, on: bool) -> bool {
        crate::sdkx::set_ai_on(chr, on)
    }
    #[cfg(not(all(windows, feature = "party")))]
    pub fn ai_on(_chr: &ChrIns) -> Option<bool> {
        None
    }
    #[cfg(not(all(windows, feature = "party")))]
    pub fn set_ai_on(_chr: &mut ChrIns, _on: bool) -> bool {
        false
    }
}

/// (ZETTA 2026-10-23) The AI switch as it was before Zettaflare (None: never touched).
fn give_ai_back(chr: &mut ChrIns, was: Option<bool>) {
    if let Some(on) = was {
        let _ = ai_switch::set_ai_on(chr, on);
    }
}

/// (ZETTA 2026-10-23) An AI lock given back: its AI switch and action switch as they were.
fn ai_unlock(l: &mut AiLock) {
    if let Some(chr) = crate::targets::find_chr(&l.handle) {
        give_ai_back(chr, l.ai_was);
        chr.debug_flags.set_disabled_secondary_actions(l.no_action_was);
    }
}

fn give_back(chr: &mut ChrIns, saved: &Saved) {
    let physics = chr.modules.physics.as_mut();
    physics.gravity_disabled = false;
    physics.motion_multiplier = saved.motion_multiplier;
    if game::plausible(crate::word!(chr.modules.fall)) {
        let fall = chr.modules.fall.as_mut();
        fall.disable_fall_motion = saved.disable_fall_motion;
        fall.fall_timer = 0.0;
    }
    chr.debug_flags.set_disabled_movement(saved.no_move);
    chr.debug_flags.set_disabled_secondary_actions(saved.no_action);
    chr.debug_flags.set_disabled_updates(saved.no_update);
    crate::chranim::set_speed(chr, if saved.speed > 0.0 { saved.speed } else { 1.0 });
}

#[cfg(test)]
mod zetta_keep_tests {
    /// (2026-10-23, the user: "Melania was also attacking during Zettaflare!") a body Zettaflare keeps can neither move nor act
    /// (no attack: the game's "no secondary actions"), frozen outright once hidden; the AI switch is off besides (zetta_keep)
    #[test]
    fn zetta_a_kept_body_neither_moves_nor_acts_frozen_when_hidden() {
        let seen = super::zetta_keep_bits(false);
        let hidden = super::zetta_keep_bits(true);
        assert!(seen & 1 != 0 && seen & 2 != 0 && seen & 8 == 0, "seen: no movement, no actions, its hurt animation runs");
        assert!(hidden & 1 != 0 && hidden & 2 != 0 && hidden & 8 != 0, "hidden: no updates at all");
    }
}

#[cfg(test)]
mod zetta_hurt_tests {
    use super::*;

    #[test]
    fn a_held_hurt_target_is_asked_its_reaction_every_half_second() {
        // (ZETTA round 5c) at the hit at once, then every ZETTA_HURT_EVERY s while held and hurt; none before the hit
        for dt in [1.0 / 60.0, 1.0 / 30.0, 1.0 / 144.0, 0.1] {
            let mut c = HurtCadence::default();
            let mut t = 0.0;
            let mut asks = Vec::new();
            while t < 3.0 {
                let hurt = t >= 1.0 - 1e-6;
                if c.step(hurt, dt) {
                    asks.push(t);
                }
                t += dt;
            }
            assert!(asks.first().is_some_and(|a| (a - 1.0).abs() < dt + 1e-4), "dt {dt}: at the hit ({asks:?})");
            assert!(asks.iter().all(|a| *a >= 1.0 - 1e-4), "dt {dt}: none before the hit");
            assert!(asks.len() >= 4 && asks.len() <= 5, "dt {dt}: about every 0.5 s over 2 s ({asks:?})");
            for w in asks.windows(2) {
                assert!(w[1] - w[0] > ZETTA_HURT_EVERY - dt - 1e-4 && w[1] - w[0] < ZETTA_HURT_EVERY + dt + 1e-4, "dt {dt}: the cadence ({asks:?})");
            }
        }
        // the hurt over (released, a new hold): none, and the next hurt asks at once
        let mut c = HurtCadence::default();
        assert!(c.step(true, 0.016));
        assert!(!c.step(true, 0.016));
        assert!(!c.step(false, 0.016));
        assert!(c.step(true, 0.016));
        // a long hitch asks once, never a burst
        let mut c = HurtCadence::default();
        assert!(c.step(true, 0.016));
        assert!(c.step(true, 3.0));
        assert!(!c.step(true, 0.016));
    }
}
