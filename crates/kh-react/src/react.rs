//! The reaction itself: `UTresAICharState_Damage` / `UTresEnemyState_Damage` as a state machine a
//! host steps (docs/sora-enemy-reaction-spec.md sections 2 - 5). Sub-step numbers in the
//! comments are the game's (`m_nSubStep`).

use glam::Vec3;
use kh_core::enums::ETresDamageKind;
use kh_core::host::{HitRecord, ReactionResult, TargetBody};

use crate::params::{Body, P};
use crate::world::ReactWorld;

/// The step the numbers are for (the game runs the handlers on the frame time; every brake is
/// written as `factor^(dt x 60)`, so another step length gives the same curves).
pub const STEP: f32 = 1.0 / 60.0;

/// A floor is a surface whose normal is at least this upright (UE walkable floor, 45 degrees).
const FLOOR_NORMAL_Z: f32 = 0.7;
/// How far below its feet a walking victim still finds its floor, and the lift of its sweeps, cm.
const FLOOR_PROBE: f32 = 30.0;
const SKIN: f32 = 2.0;
/// UE 4.17 default terminal velocity of a physics volume, cm/s.
const TERMINAL_VELOCITY: f32 = 4000.0;
/// The per-frame (60 Hz) decay of the spin's root-motion speed: a fit to the root track of
/// `e_ex003`'s `m_260` (docs/sora-enemy-reaction-spec.md 4.6); a host with a real clip replaces the
/// whole profile by its own root motion.
const SPIN_DECAY: f32 = 0.95;

/// What the host shows. One per clip family of the game (section 6 of the spec maps them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    /// No reaction: the host's own AI and movement run.
    #[default]
    None,
    /// SMALL on the ground: the flinch clip, sliding back.
    FlinchGround,
    /// SMALL in the air: the air flinch, hanging with gravity off (the juggle).
    FlinchAir,
    /// Landed on its feet out of an air flinch.
    LandFeet,
    /// Dropping in its ordinary fall pose (the air flinch clip ended in the air).
    Falling,
    /// BLOW / PARABOLA: thrown, tumbling.
    BlownFlying,
    /// Thrown and now dropping (the "break" fall of a BLOW, after a spin, off a wall).
    BlownFalling,
    /// SIDESPIN / VERTICALROLL: spun away along the clip's root motion.
    SpunAway,
    /// TOSS / LAUNCH: going up.
    LaunchedRising,
    /// TOSS / LAUNCH: the hover at the apex.
    LaunchedHover,
    /// TOSS / LAUNCH: coming down (on its back).
    LaunchedFalling,
    /// BEAT: driven straight down.
    SlammedDown,
    /// Stuck to a wall.
    WallHit,
    /// Landed and lying (the first part of the down clip).
    Down,
    /// The end of the down clip (see `ClipTimes::getup_tail`).
    GettingUp,
    /// Frozen / stunned for the bad status's time (not the damage state's own handlers: see the spec).
    Frozen,
    Stunned,
}

impl Phase {
    /// The body is thrown or lying: a host without poses tilts it.
    pub fn is_lying(self) -> bool {
        matches!(self, Phase::BlownFlying | Phase::BlownFalling | Phase::SpunAway | Phase::LaunchedRising | Phase::LaunchedHover | Phase::LaunchedFalling | Phase::SlammedDown | Phase::Down)
    }
}

/// Bad statuses the reaction knows (the others are the host's).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BadStatus {
    #[default]
    None,
    Freeze,
    Stun,
}

/// One hit, as the victim's damage state receives it (`TresDamageInfo` + the hit direction + the
/// instigator).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReactionRequest {
    /// `m_DamageKind` of the attack row.
    pub kind: ETresDamageKind,
    /// The classification's victim result (`ReactionResult::Damage` for a plain hit).
    pub victim_result: ReactionResult,
    /// The knockback direction (`ShotDirection`: for Sora's rows the unit vector from his root
    /// to the victim's), world.
    pub direction: Vec3,
    /// `m_DamagePowerScale`, `m_DamageParabolaAngle`, `m_DamageEffectTime` of the row.
    pub power_scale: f32,
    pub parabola_angle: f32,
    pub effect_time: f32,
    /// `m_RevengePoint` of the row x the victim's coefficient for the attribute (1 for most).
    pub revenge_point: f32,
    pub bad_status: BadStatus,
    pub bad_time: f32,
    /// The instigator pawn: root position, facing (degrees), capsule radius; `attacker_valid`
    /// false when there is none.
    pub attacker_position: Vec3,
    pub attacker_yaw: f32,
    pub attacker_radius: f32,
    pub attacker_valid: bool,
    /// The hit took hit points (revenge points are only earned by such hits).
    pub hp_lost: bool,
    /// The hit brought the victim to 0 HP: the host's death takes over.
    pub lethal: bool,
    /// (XSI, 2026-10-16) BREAK BOOST (kh-core abilities/xs, XS 2.16): a STRONG body whose endurance this hit emptied stays
    /// open this long instead of `strong_recover_time`; None = KH3's rule.
    pub open_time: Option<f32>,
}

impl ReactionRequest {
    /// A plain hit of `kind` along `direction` (power scale 1, nothing else).
    pub fn new(kind: ETresDamageKind, direction: Vec3) -> ReactionRequest {
        ReactionRequest { kind, victim_result: ReactionResult::Damage, direction, power_scale: 1.0, parabola_angle: 0.0, effect_time: 0.0, revenge_point: 0.0, bad_status: BadStatus::None, bad_time: 0.0, attacker_position: Vec3::ZERO, attacker_yaw: 0.0, attacker_radius: 0.0, attacker_valid: false, hp_lost: true, lethal: false, open_time: None }
    }

    /// The request of a hit the core delivered (`Targets::apply_hit`): `hp_lost` and `lethal` are
    /// what the host's own HP bookkeeping made of it.
    pub fn from_hit(hit: &HitRecord, hp_lost: bool, lethal: bool) -> ReactionRequest {
        let bad_status = match hit.info.bad_status {
            "FREEZE" => BadStatus::Freeze,
            "STUN" => BadStatus::Stun,
            _ => BadStatus::None,
        };
        ReactionRequest {
            kind: hit.damage_kind,
            victim_result: hit.info.victim_result,
            direction: hit.direction,
            power_scale: hit.info.power_scale,
            parabola_angle: hit.info.parabola_angle,
            effect_time: hit.info.effect_time,
            revenge_point: hit.info.revenge_point,
            bad_status,
            bad_time: hit.info.bad_time,
            attacker_position: hit.attacker.position,
            attacker_yaw: hit.attacker.yaw,
            attacker_radius: hit.attacker.radius,
            attacker_valid: hit.attacker.valid,
            hp_lost,
            lethal,
            // (XSI) the Break Boost mark of the hit record
            open_time: (hit.flags & kh_core::abilities::xs::FLAG_BREAK_BOOST != 0).then_some(kh_core::abilities::xs::X.break_boost_open_time),
        }
    }
}

/// Where the victim is when a hit arrives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Its foot point, cm.
    pub position: Vec3,
    /// Its facing, degrees (forward = (cos, sin, 0)).
    pub yaw: f32,
    /// It stands on a floor.
    pub grounded: bool,
}

/// What a hit did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitOutcome {
    /// A reaction started (or restarted): the host suspends its AI until `ReactOutput::finished`.
    /// `downgraded`: a STRONG body turned a big kind into a flinch.
    Started { kind: ETresDamageKind, downgraded: bool },
    /// The hit lands (damage, effect) and the victim plays nothing.
    NoReaction,
    /// Guarded / repelled / armour break: the enemy's own guard or break action, not the damage state.
    HostState(ReactionResult),
    /// The hit killed: the host's death.
    Dying,
    /// A damage kind whose handler is not ported (STOP, MAGNET, CATCH, BIND, SUCTION ...): nothing done.
    NotPorted,
}

/// One step's answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReactOutput {
    /// The victim's foot point after this step and its velocity, cm and cm/s.
    pub position: Vec3,
    pub velocity: Vec3,
    /// Its facing, degrees.
    pub yaw: f32,
    /// It is on a floor.
    pub grounded: bool,
    pub phase: Phase,
    /// Seconds the current clip has played (for the host's animation time).
    pub phase_time: f32,
    /// The damage kind being acted out.
    pub kind: ETresDamageKind,
    /// Where the attacker was relative to the victim: 0 front, 1 behind, 2 right, 3 left (the
    /// game picks the clip by it), and the alternating variant of the ground flinch.
    pub direction_index: u8,
    pub variant: bool,
    /// The host's own AI and movement must not run this step: the position above is the truth.
    pub suspended: bool,
    /// The reaction ended in this step: from the next one the host is in control again.
    pub finished: bool,
    /// This step: it touched down / it hit a wall hard enough for the wall reaction.
    pub landed: bool,
    pub wall_hit: bool,
    /// Hits taken since this reaction chain began (the state's own counter).
    pub hits: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Walking,
    Falling,
    Flying,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Handler {
    #[default]
    None,
    Small,
    Blow,
    Parabola,
    Toss,
    Launch,
    Beat,
    Spin,
    Hold,
}

/// (GOOFY-HR, 2026-10-17) The clip family playing, for a host that animates each family with its own clip (the
/// party's friends: KH3's Blow(index, phase) start / loop / down, kh-party hurt.rs). Exported as `ReactClip`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Clip {
    #[default]
    None,
    SmallGround,
    SmallAir,
    Land,
    FallLoop,
    BlowStart,
    BlowLoop,
    BlowFallLoop,
    BlowDown,
    TossStart,
    TossRiseLoop,
    TossFallStart,
    TossFallLoop,
    TossDown,
    BeatFall,
    BeatDown,
    Spin,
    SpinDown,
    WallHit,
    WallFall,
    WallDown,
    Held,
}

/// The reaction endurance of a STRONG body (pawn+0x814: value, `m_MaxBodyStrongValue`, timer).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct Endurance {
    value: i32,
    timer: f32,
}

/// The revenge value (pawn+0x7F8).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct Revenge {
    value: f32,
    hold: f32,
    with_points: f32,
    cooldown: f32,
    has_points: bool,
    ready: bool,
}

/// One enemy's reaction: lives as long as the enemy, idle between hits.
#[derive(Clone, Debug)]
pub struct EnemyReaction {
    pub body: Body,
    // ---- the body's kinematics while a reaction runs
    position: Vec3,
    velocity: Vec3,
    yaw: f32,
    mode: Mode,
    grounded: bool,
    gravity_on: bool,
    gravity_scale: f32,
    // ---- the state's fields
    active: bool,
    handler: Handler,
    kind: ETresDamageKind,
    sub: u8,
    clip: Clip,
    clip_time: f32,
    request: ReactionRequest,
    /// +0x164: the hit direction normalised in XY; +0x1A8: the knock direction.
    knock: Vec3,
    index: u8,
    variant: bool,
    power: f32,
    brake: f32,
    velocity_z: f32,
    timer: f32,
    facing: f32,
    start: Vec3,
    /// +0x1F0 bits 0 / 1 / 2.
    brake_on: bool,
    was_flying: bool,
    landed: bool,
    hits: u32,
    spin_travel: f32,
    spin_speed: f32,
    hold_phase: Phase,
    // ---- this step's events
    ev_landed: bool,
    ev_wall: bool,
    // ---- what outlives a reaction
    endurance: Endurance,
    revenge: Revenge,
}

fn norm180(a: f32) -> f32 {
    let a = a % 360.0;
    if a > 180.0 {
        a - 360.0
    } else if a < -180.0 {
        a + 360.0
    } else {
        a
    }
}

fn yaw_of(v: Vec3) -> f32 {
    v.y.atan2(v.x).to_degrees()
}

fn forward(yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(c, s, 0.0)
}

/// `normalize2D` of the game: zero when shorter than 1e-4.
fn flat(v: Vec3) -> Vec3 {
    let f = Vec3::new(v.x, v.y, 0.0);
    if f.length_squared() < 1.0e-8 {
        Vec3::ZERO
    } else {
        f.normalize()
    }
}

fn horizontal_speed(v: Vec3) -> f32 {
    v.truncate().length()
}

impl EnemyReaction {
    pub fn new(body: Body) -> EnemyReaction {
        EnemyReaction {
            body,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            yaw: 0.0,
            mode: Mode::Walking,
            grounded: true,
            gravity_on: true,
            gravity_scale: body.params.gravity_scale,
            active: false,
            handler: Handler::None,
            kind: ETresDamageKind::None,
            sub: 0,
            clip: Clip::None,
            clip_time: 0.0,
            request: ReactionRequest::new(ETresDamageKind::None, Vec3::X),
            knock: Vec3::X,
            index: 0,
            variant: false,
            power: 0.0,
            brake: 1.0,
            velocity_z: 0.0,
            timer: 0.0,
            facing: 0.0,
            start: Vec3::ZERO,
            brake_on: false,
            was_flying: false,
            landed: false,
            hits: 0,
            spin_travel: 0.0,
            spin_speed: 0.0,
            hold_phase: Phase::None,
            ev_landed: false,
            ev_wall: false,
            endurance: Endurance { value: body.strong_max.max(0), timer: 0.0 },
            revenge: Revenge::default(),
        }
    }

    /// A reaction is running: the host's AI and movement are suspended.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The damage kind being acted out (`Targets::reaction_state`: 0 when idle).
    pub fn reaction_state(&self) -> u8 {
        if self.active {
            self.kind.value() as u8
        } else {
            0
        }
    }

    pub fn position(&self) -> Vec3 {
        self.position
    }

    /// The host moved the victim itself while a reaction runs (a teleport, an origin shift).
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    /// The body's answers to the hit classification (`Targets::body`).
    pub fn target_body(&self) -> TargetBody {
        TargetBody { reaction: self.body.reaction, ..TargetBody::DEFAULT }
    }

    /// (GOOFY-HR) The clip family playing (`Clip::None` when idle), the direction index and the ground flinch's
    /// alternating variant of the running reaction, and the seconds the clip has played.
    pub fn clip(&self) -> Clip {
        if self.active { self.clip } else { Clip::None }
    }

    pub fn index(&self) -> u8 {
        self.index
    }

    pub fn variant(&self) -> bool {
        self.variant
    }

    pub fn clip_time(&self) -> f32 {
        self.clip_time
    }

    /// (GOOFY-HR) Hits taken since this reaction chain began (the state's own counter, as `ReactOutput::hits`).
    pub fn hits(&self) -> u32 {
        self.hits
    }

    /// The endurance left (for a HUD or a test).
    pub fn endurance(&self) -> i32 {
        self.endurance.value
    }

    pub fn revenge_value(&self) -> f32 {
        self.revenge.value
    }

    /// `ai.slot32(power)` (0x140576400, `Targets::take_reaction_power`): a STRONG body takes
    /// `power` off its endurance and answers what is left (> 0: it holds).
    pub fn take_reaction_power(&mut self, power: i32) -> i32 {
        let max = self.body.strong_max;
        let e = &mut self.endurance;
        if max <= 0 {
            (e.value, e.timer) = (0, 0.0);
            return 0;
        }
        if e.value <= 0 {
            if e.timer <= 0.0 {
                e.timer = P.strong_recover_time;
            }
            return e.value;
        }
        if power <= 0 {
            return e.value;
        }
        let left = e.value - power;
        e.timer = P.strong_recover_time;
        e.value = if left < 0 { 0 } else { left.min(max) };
        e.value
    }

    /// The revenge value crossed the limit since the last call (the game raises a flag its AI
    /// answers with a revenge action). Asking clears the flag; the value itself is cleared by
    /// [`EnemyReaction::clear_revenge`] when the host's enemy really breaks out.
    pub fn take_revenge(&mut self) -> bool {
        std::mem::take(&mut self.revenge.ready)
    }

    /// (XSI, 2026-10-16) Rage Raid: the body's revenge runs (the cool-down after its break-out).
    pub fn in_revenge(&self) -> bool {
        self.revenge.cooldown > 0.0
    }

    /// (XSI) Rage Raid's END hit: the revenge value and its cool-down are cleared.
    pub fn end_revenge(&mut self) {
        self.revenge = Revenge::default();
    }

    /// The endurance's open timer (for a HUD or a test).
    pub fn endurance_timer(&self) -> f32 {
        self.endurance.timer
    }

    /// The enemy broke out of the combo: the value back to 0, the cool-down started, the running
    /// reaction (if any) ended.
    pub fn clear_revenge(&mut self) {
        self.revenge = Revenge { cooldown: self.body.revenge_cooldown, ..Revenge::default() };
        if self.active {
            self.finish();
        }
    }

    /// Ends a running reaction at once (death, despawn, the host takes the enemy back).
    pub fn release(&mut self) {
        if self.active {
            self.finish();
        }
    }

    fn add_revenge(&mut self, points: f32) {
        let limit = self.body.revenge_limit;
        let r = &mut self.revenge;
        if points == 0.0 || limit <= 0.0 || r.cooldown > 0.0 {
            return;
        }
        let before = r.value;
        let add = if r.with_points >= P.revenge_full_after { limit } else { points };
        r.value = (before + add).clamp(0.0, 100.0);
        if before < limit && limit <= r.value {
            r.ready = true;
        }
        if r.value > 0.0 {
            r.has_points = true;
            r.hold = P.revenge_hold_time;
        }
    }

    /// Timers that run whether or not a reaction does (0x140598CD0, 0x140598D30).
    fn tick_body(&mut self, dt: f32) {
        let max = self.body.strong_max;
        let e = &mut self.endurance;
        if max > 0 && e.timer > 0.0 {
            e.timer -= dt;
            if e.timer <= 0.0 {
                e.timer = 0.0;
                if e.value < 1 {
                    e.value = max;
                } else {
                    e.value = (e.value + 1).min(max);
                    if e.value < max {
                        e.timer = P.strong_recover_time;
                    }
                }
            }
        }
        let in_damage = self.active;
        let r = &mut self.revenge;
        if r.cooldown > 0.0 {
            r.cooldown = (r.cooldown - dt).max(0.0);
            return;
        }
        if self.body.revenge_limit > 0.0 {
            if r.has_points {
                r.hold -= dt;
                if r.hold <= 0.0 && !in_damage {
                    r.hold = 0.0;
                    r.value -= dt * P.revenge_decay_per_second;
                    if r.value <= 0.0 {
                        (r.value, r.has_points) = (0.0, false);
                    }
                }
            }
            r.with_points = if r.value <= 0.0 { 0.0 } else { (r.with_points + dt).min(P.revenge_full_after) };
        }
    }

    // ------------------------------------------------------------------------------------------
    // The hit: the enemy's reaction gate (0x140DEF290, 0x140DF84F0) and Setup (0x140457380).
    // ------------------------------------------------------------------------------------------

    /// A hit landed on this enemy at `pose` (ignored while a reaction runs: then the reaction's own
    /// position is used).
    pub fn hit(&mut self, request: &ReactionRequest, pose: Pose) -> HitOutcome {
        use ETresDamageKind as K;
        // (XSI) Break Boost: the endurance this hit's reaction power emptied (take_reaction_power, just before) stays open longer
        if let Some(open) = request.open_time {
            let e = &mut self.endurance;
            if self.body.strong_max > 0 && e.value <= 0 && (e.timer - P.strong_recover_time).abs() < 1.0e-4 {
                e.timer = open;
            }
        }
        if request.hp_lost && request.bad_status == BadStatus::None {
            self.add_revenge(request.revenge_point);
        }
        if request.lethal {
            self.release();
            return HitOutcome::Dying;
        }
        match request.victim_result {
            ReactionResult::Reflect | ReactionResult::GuardReflect | ReactionResult::Guard | ReactionResult::ArmorBreak => return HitOutcome::HostState(request.victim_result),
            ReactionResult::None | ReactionResult::NoReaction => return HitOutcome::NoReaction,
            _ => {}
        }
        if self.body.never_reacts {
            return HitOutcome::NoReaction;
        }
        // 0x140DF84F0: what a STRONG body lets through. "Big" kinds: mask 0x20000F9C.
        let mut kind = request.kind;
        let small = kind == K::Small;
        let big = matches!(kind.value(), 2 | 3 | 4 | 7 | 8 | 9 | 10 | 11 | 29);
        let mut downgraded = false;
        match request.victim_result {
            ReactionResult::StrongBodyDamage1 if small || big => return HitOutcome::NoReaction,
            ReactionResult::StrongBodyDamage2 | ReactionResult::StrongBodyDamage3 => {
                if small {
                    return HitOutcome::NoReaction;
                }
                if big {
                    (kind, downgraded) = (K::Small, true);
                }
            }
            _ => {}
        }
        // Kinds 12 - 14 never reach a reaction (WITHOUT, RECOVER, KILL); 15 and up have handlers
        // of their own that are not ported.
        if matches!(kind.value(), 12..=14) {
            return HitOutcome::NoReaction;
        }
        if kind.value() >= 15 && !matches!(kind.value(), 21 | 22) {
            return HitOutcome::NotPorted;
        }
        if !self.active {
            (self.position, self.yaw, self.grounded) = (pose.position, pose.yaw, pose.grounded);
            self.mode = if self.body.flying { Mode::Flying } else if pose.grounded { Mode::Walking } else { Mode::Falling };
            self.hits = 0;
            // Begin (0x140433380): the Flying bit.
            self.was_flying = self.mode == Mode::Flying;
        }
        self.setup(request, kind);
        HitOutcome::Started { kind, downgraded }
    }

    fn setup(&mut self, request: &ReactionRequest, kind: ETresDamageKind) {
        self.active = true;
        self.request = *request;
        self.kind = kind;
        // +0x164: the hit direction flat; without one, the attacker's facing, else away from the victim's own.
        let mut dir = flat(request.direction);
        if dir == Vec3::ZERO {
            dir = if request.attacker_valid { forward(request.attacker_yaw) } else { -forward(self.yaw) };
        }
        self.knock = dir;
        (self.brake_on, self.landed) = (false, false);
        self.sub = 0;
        self.hits += 1;
        // StopMovement, the class-default gravity scale, gravity on (0x140455C70).
        self.velocity = Vec3::ZERO;
        self.gravity_scale = self.body.params.gravity_scale;
        self.gravity_on = true;
        self.clip = Clip::None;
        self.clip_time = 0.0;
        if self.grounded {
            // (+0x1E4 / +0x1E8, the counters of the paint attraction, are cleared here in the game.)
        }
        if request.bad_status != BadStatus::None && request.bad_time > 0.0 {
            self.handler = Handler::Hold;
            self.timer = request.bad_time;
            self.hold_phase = if request.bad_status == BadStatus::Freeze { Phase::Frozen } else { Phase::Stunned };
            self.play(Clip::Held);
            return;
        }
        let has_spin = self.body.clips.spin.is_some();
        self.handler = match kind.value() {
            2 | 5 | 21 => Handler::Blow,
            3 | 4 if has_spin => Handler::Spin,
            3 | 4 => {
                self.kind = ETresDamageKind::Blow;
                Handler::Blow
            }
            6 | 7 | 22 => Handler::Toss,
            8 => Handler::Launch,
            9 => Handler::Beat,
            10 | 11 => Handler::Parabola,
            _ => Handler::Small,
        };
        // The handler's sub-step 0 runs inside Setup.
        self.run_handler(None, STEP);
    }

    // ------------------------------------------------------------------------------------------
    // One step.
    // ------------------------------------------------------------------------------------------

    /// Advances one 60 Hz step. Call it every step for every enemy that has one of these: idle it
    /// only runs the endurance and revenge timers and answers `suspended: false`.
    pub fn step(&mut self, world: &dyn ReactWorld) -> ReactOutput {
        self.step_dt(world, STEP)
    }

    /// (boss finish) As [`EnemyReaction::step`] with a step of `dt` seconds: a host whose world runs
    /// slowed (KH3's global game speed, `kh_core::host::time::GameSpeed`) passes `STEP x rate`.
    pub fn step_dt(&mut self, world: &dyn ReactWorld, dt: f32) -> ReactOutput {
        self.tick_body(dt);
        (self.ev_landed, self.ev_wall) = (false, false);
        let mut finished = false;
        if self.active {
            // The movement component ticks first (the velocity a handler wrote acts from the next
            // step), then the pose (the clip time), then the state.
            self.move_body(world, dt);
            self.clip_time += dt;
            if self.run_handler(Some(world), dt) {
                self.finish();
                finished = true;
            }
        }
        self.output(finished)
    }

    fn output(&self, finished: bool) -> ReactOutput {
        ReactOutput {
            position: self.position,
            velocity: self.velocity,
            yaw: self.yaw,
            grounded: self.grounded,
            phase: self.phase(),
            phase_time: self.clip_time,
            kind: if self.active { self.kind } else { ETresDamageKind::None },
            direction_index: self.index,
            variant: self.variant,
            suspended: self.active,
            finished,
            landed: self.ev_landed,
            wall_hit: self.ev_wall,
            hits: self.hits,
        }
    }

    /// The phase of the clip that is playing.
    pub fn phase(&self) -> Phase {
        if !self.active {
            return Phase::None;
        }
        let down = |length: f32| if self.clip_time >= length - self.body.clips.getup_tail { Phase::GettingUp } else { Phase::Down };
        let c = &self.body.clips;
        match self.clip {
            Clip::None => match self.handler {
                Handler::Small if self.grounded => Phase::FlinchGround,
                Handler::Small => Phase::FlinchAir,
                Handler::Toss | Handler::Launch => Phase::LaunchedRising,
                Handler::Beat => Phase::SlammedDown,
                Handler::Spin => Phase::SpunAway,
                _ => Phase::BlownFlying,
            },
            Clip::SmallGround => Phase::FlinchGround,
            Clip::SmallAir => Phase::FlinchAir,
            Clip::Land => Phase::LandFeet,
            Clip::FallLoop => Phase::Falling,
            Clip::BlowStart | Clip::BlowLoop => Phase::BlownFlying,
            Clip::BlowFallLoop | Clip::WallFall => Phase::BlownFalling,
            Clip::BlowDown => down(c.blow_down),
            Clip::TossStart | Clip::TossRiseLoop => {
                if self.sub == 16 {
                    Phase::LaunchedHover
                } else {
                    Phase::LaunchedRising
                }
            }
            Clip::TossFallStart | Clip::TossFallLoop => Phase::LaunchedFalling,
            Clip::TossDown => down(c.toss_down),
            Clip::BeatFall => Phase::SlammedDown,
            Clip::BeatDown => down(c.beat_down.unwrap_or(0.0)),
            Clip::Spin => {
                if self.sub >= 5 {
                    Phase::BlownFalling
                } else {
                    Phase::SpunAway
                }
            }
            Clip::SpinDown => down(c.spin_down),
            Clip::WallHit => Phase::WallHit,
            Clip::WallDown => down(c.wall_down),
            Clip::Held => self.hold_phase,
        }
    }

    fn finish(&mut self) {
        // End (0x140434C50 -> 0x140455C70): speeds, the gravity scale and gravity itself are restored.
        self.active = false;
        self.handler = Handler::None;
        self.clip = Clip::None;
        self.gravity_on = true;
        self.gravity_scale = self.body.params.gravity_scale;
        self.brake_on = false;
    }

    fn play(&mut self, clip: Clip) {
        self.clip = clip;
        self.clip_time = 0.0;
    }

    fn clip_length(&self, clip: Clip) -> Option<f32> {
        let c = &self.body.clips;
        Some(match clip {
            Clip::SmallGround => c.small_ground,
            Clip::SmallAir => c.small_air,
            Clip::Land => c.land,
            Clip::BlowStart => c.blow_start,
            Clip::BlowDown => c.blow_down,
            Clip::TossStart => c.toss_start,
            Clip::TossFallStart => c.toss_fall_start,
            Clip::TossDown => c.toss_down,
            Clip::BeatDown => c.beat_down?,
            Clip::Spin => c.spin?,
            Clip::SpinDown => c.spin_down,
            Clip::WallHit => c.wall,
            Clip::WallDown => c.wall_down,
            // Loops never end.
            _ => return None,
        })
    }

    fn anim_end(&self) -> bool {
        self.clip_length(self.clip).is_some_and(|length| self.clip_time >= length - 1.0e-4)
    }

    /// pawn vt+0x23F0 (0x1405A94E0): where the attacker is, from the victim's facing.
    fn direction_index(&self, knock: Vec3) -> u8 {
        let delta = norm180(yaw_of(-knock) - self.yaw);
        if (-45.0..=45.0).contains(&delta) {
            0
        } else if delta > 45.0 && delta <= 135.0 {
            2
        } else if (-135.0..-45.0).contains(&delta) {
            3
        } else {
            1
        }
    }

    /// The facing held during a flight: toward the attacker, or away when it is behind.
    fn facing_for(&self) -> f32 {
        if self.index == 1 {
            yaw_of(self.knock)
        } else {
            yaw_of(-self.knock)
        }
    }

    /// pawn vt+0xDB0: turn toward `target` by `alpha` of the remaining angle.
    fn turn(&mut self, target: f32, alpha: f32) {
        let delta = norm180(target - self.yaw);
        self.yaw = norm180(self.yaw + delta * alpha.clamp(0.0, 1.0));
    }

    /// 0x140431E10: `SetVelXY(Velocity x brake^(dt x 60))`.
    fn apply_brake(&mut self, brake: f32, dt: f32) {
        let f = brake.powf(dt * 60.0);
        self.velocity.x *= f;
        self.velocity.y *= f;
    }

    fn set_falling(&mut self) {
        self.mode = Mode::Falling;
        self.grounded = false;
    }

    fn stop(&mut self) {
        self.velocity = Vec3::ZERO;
    }

    fn run_handler(&mut self, world: Option<&dyn ReactWorld>, dt: f32) -> bool {
        match self.handler {
            Handler::None => true,
            Handler::Small => self.small(dt),
            Handler::Blow => self.blow(dt),
            Handler::Parabola => self.parabola(dt),
            Handler::Toss => self.toss(dt, false),
            Handler::Launch => self.toss(dt, true),
            Handler::Beat => self.beat(world, dt),
            Handler::Spin => self.spin(dt),
            Handler::Hold => self.hold(dt),
        }
    }

    // ---- SMALL (slot 116, 0x14044F650) -------------------------------------------------------

    fn small(&mut self, dt: f32) -> bool {
        let p = self.body.params;
        match self.sub {
            0 => {
                self.index = self.direction_index(self.knock);
                if self.index < 2 {
                    self.variant = !self.variant;
                }
                self.power = p.small_power * self.request.power_scale;
                self.velocity_z = if self.grounded { 0.0 } else { p.small_air_velocity_z };
                self.sub = 1;
            }
            1 => {
                if !self.grounded && self.mode != Mode::Flying {
                    self.gravity_on = false;
                    self.velocity = Vec3::new(self.power * self.knock.x, self.power * self.knock.y, self.velocity_z);
                    self.play(Clip::SmallAir);
                } else {
                    self.velocity.x = self.power * self.knock.x;
                    self.velocity.y = self.power * self.knock.y;
                    self.play(if self.grounded { Clip::SmallGround } else { Clip::SmallAir });
                }
                self.brake_on = true;
                self.facing = self.facing_for();
                self.sub = 4;
                return false;
            }
            4 => {
                self.turn(self.facing, P.turn_rate * dt * 60.0);
                if !self.gravity_on {
                    if !self.grounded && self.mode != Mode::Flying {
                        self.velocity.z *= p.small_brake.powf(dt * 60.0);
                    }
                    if horizontal_speed(self.velocity) <= p.small_start_falling_speed {
                        self.gravity_on = true;
                    }
                }
                if self.grounded && self.clip == Clip::SmallAir {
                    // Landed while the air clip plays: it is played out, then the landing.
                    if self.anim_end() {
                        self.play(Clip::Land);
                        self.sub = 5;
                    }
                } else if self.anim_end() {
                    if !self.grounded && !self.body.flying {
                        // An enemy whose default movement is walking drops in its fall loop.
                        self.play(Clip::FallLoop);
                        self.gravity_on = true;
                        self.sub = 15;
                    } else {
                        return true;
                    }
                }
            }
            5 => {
                if self.anim_end() {
                    return true;
                }
            }
            15 => {
                if self.landed {
                    self.play(Clip::Land);
                    self.sub = 5;
                }
            }
            _ => return true,
        }
        if self.brake_on {
            self.apply_brake(p.small_brake, dt);
        }
        false
    }

    // ---- BLOW (slot 117, enemy override 0x140DD8980) -----------------------------------------

    fn blow(&mut self, dt: f32) -> bool {
        let p = self.body.params;
        match self.sub {
            0 => {
                self.index = self.direction_index(self.knock);
                let ground = self.grounded;
                self.power = if ground { p.blow_ground_power } else { p.blow_air_power } * self.request.power_scale;
                self.brake = if ground { p.blow_ground_brake } else { p.blow_air_brake };
                self.velocity_z = if ground { p.blow_ground_velocity_z } else { p.blow_air_velocity_z };
                self.sub = 1;
            }
            1 => {
                let ground = self.grounded;
                if !ground {
                    self.gravity_on = false;
                }
                self.play(Clip::BlowStart);
                if self.mode != Mode::Flying {
                    self.set_falling();
                }
                self.velocity = Vec3::new(self.power * self.knock.x, self.power * self.knock.y, self.velocity_z);
                self.brake_on = true;
                self.timer = if ground { 0.0 } else { p.blow_air_start_brake_time };
                self.facing = self.facing_for();
                self.sub = 2;
                return false;
            }
            2 => {
                self.turn(self.facing, P.turn_rate * dt * 60.0);
                if !self.landed {
                    if self.anim_end() && self.clip == Clip::BlowStart {
                        self.play(Clip::BlowLoop);
                    }
                    self.timer -= dt;
                    if self.timer <= 0.0 {
                        self.sub = 3;
                    }
                } else if !(self.clip == Clip::BlowStart && !self.anim_end()) {
                    if self.clip == Clip::BlowStart {
                        self.play(Clip::BlowLoop);
                    }
                    self.sub = 3;
                }
            }
            3 | 4 => {
                if self.sub == 3 {
                    if self.clip == Clip::BlowStart {
                        if !self.anim_end() {
                            self.apply_brake(self.brake, dt);
                            return false;
                        }
                        self.play(Clip::BlowLoop);
                    }
                    self.sub = 4;
                }
                if !self.gravity_on {
                    self.velocity.z *= self.brake.powf(dt * 60.0);
                    if horizontal_speed(self.velocity) <= p.blow_start_falling_speed {
                        self.gravity_on = true;
                    }
                } else if !self.landed {
                    let slow = horizontal_speed(self.velocity) <= P.blow_flying_stop_speed;
                    if self.was_flying {
                        if slow {
                            self.blow_down();
                        }
                    } else if slow && self.velocity.z <= P.blow_break_fall_speed {
                        self.play(Clip::BlowFallLoop);
                        self.sub = 6;
                    }
                } else {
                    self.blow_down();
                }
            }
            6 => {
                if self.clip == Clip::BlowDown {
                    if self.anim_end() {
                        // (`240_3`, the separate get-up, exists on one enemy: the reaction ends here.)
                        return true;
                    }
                } else if self.landed {
                    // The fall loop's own landing clip (DmgAirBlowBr*End): the down clip's time.
                    self.stop();
                    self.play(Clip::BlowDown);
                }
            }
            8 => {
                self.turn(self.facing, P.wall_turn_rate * dt);
                if self.anim_end() {
                    self.stop();
                    self.gravity_scale = p.gravity_scale;
                    self.gravity_on = true;
                    if self.grounded {
                        self.play(Clip::WallDown);
                        self.sub = 10;
                    } else {
                        self.set_falling();
                        self.landed = false;
                        self.play(Clip::WallFall);
                        self.sub = 9;
                    }
                }
            }
            9 => {
                if self.landed {
                    self.stop();
                    self.play(Clip::WallDown);
                    self.sub = 10;
                }
            }
            10 => {
                if self.anim_end() {
                    return true;
                }
            }
            _ => return true,
        }
        if self.brake_on {
            self.apply_brake(self.brake, dt);
        }
        false
    }

    fn blow_down(&mut self) {
        self.play(Clip::BlowDown);
        if self.was_flying {
            self.gravity_on = false;
            self.velocity.z = 0.0;
        }
        self.sub = 6;
    }

    // ---- PARABOLA (slot 127, 0x14044B990) ----------------------------------------------------

    fn parabola(&mut self, dt: f32) -> bool {
        let p = self.body.params;
        match self.sub {
            0 => {
                self.index = self.direction_index(self.knock);
                self.start = self.position;
                self.sub = 1;
            }
            1 => {
                self.play(Clip::BlowStart);
                self.set_falling();
                self.gravity_on = true;
                let angle = if self.request.parabola_angle > 0.0 { self.request.parabola_angle } else { p.parabola_angle };
                let power = p.parabola_power * self.request.power_scale;
                let (s, c) = angle.to_radians().sin_cos();
                self.velocity = Vec3::new(power * c * self.knock.x, power * c * self.knock.y, power * s);
                self.brake_on = true;
                self.facing = self.facing_for();
                self.sub = 3;
                return false;
            }
            3 | 4 | 5 => {
                self.turn(self.facing, dt * 30.0);
                if self.sub == 3 {
                    if self.anim_end() {
                        self.play(Clip::BlowLoop);
                        self.timer = P.parabola_loop_min_time;
                        self.sub = 4;
                    }
                }
                if self.sub == 4 {
                    self.timer -= dt;
                    if self.timer <= 0.0 {
                        self.timer = 0.0;
                        self.sub = 5;
                    }
                }
                if self.sub == 5 && (self.landed || self.position.z - self.start.z <= 0.0) {
                    self.stop();
                    self.play(Clip::BlowDown);
                    self.sub = 6;
                }
            }
            6 => {
                self.turn(self.facing, dt * 30.0);
                if self.anim_end() {
                    return true;
                }
            }
            8 => {
                self.turn(self.facing, P.wall_turn_rate * dt);
                if self.anim_end() {
                    self.gravity_scale = p.gravity_scale;
                    self.gravity_on = true;
                    self.set_falling();
                    self.landed = false;
                    self.play(Clip::WallFall);
                    self.sub = 9;
                }
            }
            9 => {
                if self.landed {
                    self.stop();
                    self.play(Clip::WallDown);
                    self.sub = 10;
                }
            }
            10 => {
                if self.anim_end() {
                    return true;
                }
            }
            _ => return true,
        }
        if self.brake_on && horizontal_speed(self.velocity) > p.parabola_limit_brake {
            self.apply_brake(p.parabola_brake, dt);
        }
        false
    }

    // ---- TOSS (slot 120, 0x1404527F0) and LAUNCH (slot 121, 0x1404499F0) ----------------------

    fn toss(&mut self, dt: f32, launch: bool) -> bool {
        let p = self.body.params;
        let fall_scale = if launch { p.launch_fall_gravity_scale } else { p.toss_fall_gravity_scale };
        match self.sub {
            0 => {
                self.power = if launch { p.launch_power } else { p.toss_power } * self.request.power_scale;
                if launch && self.request.effect_time > 0.0 {
                    self.power = self.request.effect_time;
                }
                self.sub = 1;
            }
            1 => {
                self.set_falling();
                self.gravity_on = true;
                self.velocity.z = self.power;
                self.gravity_scale = P.rise_gravity_scale;
                self.play(Clip::TossStart);
                self.sub = 12;
            }
            12 => {
                if self.anim_end() {
                    self.play(Clip::TossRiseLoop);
                    self.sub = 13;
                }
            }
            13 => {
                if self.velocity.z <= 0.0 {
                    if launch {
                        self.gravity_scale = P.launch_hover_gravity_scale;
                    } else {
                        self.gravity_on = false;
                    }
                    self.timer = if launch { p.launch_flight_duration } else { p.toss_flight_duration };
                    self.sub = 16;
                }
            }
            16 => {
                self.timer -= dt;
                if self.timer <= 0.0 {
                    self.gravity_on = true;
                    self.gravity_scale = fall_scale;
                    self.play(Clip::TossFallStart);
                    self.sub = 14;
                }
            }
            14 => {
                if self.anim_end() {
                    self.play(Clip::TossFallLoop);
                    self.sub = 15;
                }
            }
            15 => {
                if self.landed {
                    self.stop();
                    self.play(Clip::TossDown);
                    self.sub = 6;
                }
            }
            6 => {
                if self.anim_end() {
                    // (`m_250_5`, the separate get-up, exists on one enemy.)
                    return true;
                }
            }
            _ => return true,
        }
        false
    }

    // ---- BEAT (slot 122, 0x1404463F0) --------------------------------------------------------

    fn beat(&mut self, world: Option<&dyn ReactWorld>, _dt: f32) -> bool {
        let p = self.body.params;
        match self.sub {
            0 => {
                self.power = p.beat_power * self.request.power_scale;
                self.sub = 1;
            }
            1 => {
                let on_ground = self.grounded;
                self.set_falling();
                self.velocity.z = -self.power;
                if let (true, false, Some(world)) = (self.request.attacker_valid, on_ground, world) {
                    // The ground under it within 2500 cm: slide out from under the attacker in the
                    // time the fall takes.
                    let down = Vec3::Z * P.beat_trace_range;
                    if let Some(c) = world.sweep(self.position, self.position - down) {
                        let (height, g) = (c.time * P.beat_trace_range, p.gravity * self.gravity_scale);
                        let disc = self.power * self.power + 2.0 * g * height;
                        let t = if g > 0.0 { (disc.sqrt() - self.power) / g } else if self.power > 0.0 { height / self.power } else { 0.0 };
                        if t > 0.0 {
                            let speed = P.beat_radius_factor * (self.body.radius + self.request.attacker_radius) / t;
                            self.velocity.x = speed * self.knock.x;
                            self.velocity.y = speed * self.knock.y;
                        }
                    }
                }
                self.play(Clip::BeatFall);
                self.sub = 15;
            }
            15 => {
                if self.landed {
                    self.stop();
                    // Most enemies have no slam clip (`m_259`): the reaction ends at the landing.
                    if self.body.clips.beat_down.is_none() {
                        return true;
                    }
                    self.play(Clip::BeatDown);
                    self.sub = 6;
                }
            }
            6 => {
                if self.anim_end() {
                    return true;
                }
            }
            _ => return true,
        }
        false
    }

    // ---- SIDESPIN (slot 118, 0x14044EDC0) and VERTICALROLL (slot 119, 0x140452E00) ------------

    fn spin(&mut self, dt: f32) -> bool {
        let c = self.body.clips;
        let length = c.spin.unwrap_or(0.0);
        match self.sub {
            0 => {
                self.index = self.direction_index(self.knock);
                self.sub = 1;
            }
            1 => {
                // Turned to the attacker at once (VERTICALROLL from behind: `m_271`, kept turned away),
                // gravity off, stopped, Flying: the clip's root motion carries it away.
                let behind = self.kind == ETresDamageKind::Verticalroll && self.index == 1;
                self.yaw = if behind { yaw_of(self.knock) } else { yaw_of(-self.knock) };
                self.gravity_on = false;
                self.stop();
                self.mode = Mode::Flying;
                self.grounded = false;
                self.play(Clip::Spin);
                // Distance D over the clip with a speed decaying by SPIN_DECAY per 1/60 s.
                let frames = (length * 60.0).max(1.0);
                self.spin_speed = c.spin_distance * (1.0 - SPIN_DECAY) / (1.0 - SPIN_DECAY.powf(frames)) * 60.0;
                self.spin_travel = 0.0;
                self.sub = 4;
            }
            4 => {
                if self.anim_end() {
                    // The last frame's root delta becomes the velocity; gravity back, Falling.
                    self.gravity_on = true;
                    self.set_falling();
                    self.landed = false;
                    self.sub = 5;
                } else {
                    self.velocity = self.knock * self.spin_speed;
                    self.spin_speed *= SPIN_DECAY.powf(dt * 60.0);
                }
            }
            5 => {
                if self.landed {
                    self.stop();
                    self.play(Clip::SpinDown);
                    self.sub = 6;
                }
            }
            6 => {
                if self.anim_end() {
                    return true;
                }
            }
            _ => return true,
        }
        false
    }

    // ---- frozen / stunned: the bad status's time, standing or dropping ------------------------

    fn hold(&mut self, dt: f32) -> bool {
        self.timer -= dt;
        if self.grounded {
            self.stop();
        }
        self.timer <= 0.0
    }

    // ------------------------------------------------------------------------------------------
    // The body's movement (the movement component's part: stock UE falling / walking).
    // ------------------------------------------------------------------------------------------

    fn move_body(&mut self, world: &dyn ReactWorld, dt: f32) {
        let before = self.velocity;
        match self.mode {
            Mode::Walking => {
                let delta = Vec3::new(self.velocity.x, self.velocity.y, 0.0) * dt;
                self.walk(world, delta, before);
            }
            Mode::Flying => {
                let delta = self.velocity * dt;
                self.fly(world, delta, before);
            }
            Mode::Falling => {
                let mut delta = self.velocity * dt;
                if self.gravity_on {
                    let g = self.body.params.gravity * self.gravity_scale;
                    let z = (self.velocity.z - g * dt).max(-TERMINAL_VELOCITY);
                    delta.z = 0.5 * (self.velocity.z + z) * dt;
                    self.velocity.z = z;
                }
                // A floor right under the feet is found even when the step barely moves down.
                let probe = if delta.z <= 0.0 { SKIN } else { 0.0 };
                let end = self.position + delta - Vec3::Z * probe;
                match world.sweep(self.position, end) {
                    Some(c) if c.normal.z >= FLOOR_NORMAL_Z && delta.z <= 0.0 => {
                        self.position += (delta - Vec3::Z * probe) * c.time;
                        self.land();
                        // The rest of the step is walked (the engine starts the new physics with the time left).
                        let rest = Vec3::new(delta.x, delta.y, 0.0) * (1.0 - c.time);
                        if rest.length_squared() > 0.0 {
                            self.walk(world, rest, before);
                        }
                    }
                    Some(c) => {
                        self.position += (end - self.position) * c.time + c.normal * 0.1;
                        self.blocked(c.normal, before);
                    }
                    None => self.position += delta,
                }
            }
        }
    }

    fn land(&mut self) {
        self.velocity.z = 0.0;
        self.grounded = true;
        self.landed = true;
        self.ev_landed = true;
        self.mode = if self.body.flying { Mode::Flying } else { Mode::Walking };
    }

    fn walk(&mut self, world: &dyn ReactWorld, delta: Vec3, before: Vec3) {
        if delta.length_squared() > 0.0 {
            let lift = Vec3::Z * SKIN;
            match world.sweep(self.position + lift, self.position + lift + delta) {
                Some(c) if c.normal.z < FLOOR_NORMAL_Z => {
                    self.position += delta * c.time + c.normal * 0.1;
                    self.blocked(c.normal, before);
                }
                _ => self.position += delta,
            }
        }
        // Keep to the floor; walking off an edge is a fall.
        let lift = Vec3::Z * SKIN;
        match world.sweep(self.position + lift, self.position - Vec3::Z * FLOOR_PROBE) {
            Some(c) if c.normal.z >= FLOOR_NORMAL_Z => {
                self.position.z += SKIN - c.time * (SKIN + FLOOR_PROBE);
                self.grounded = true;
            }
            _ => {
                if self.mode == Mode::Walking {
                    self.set_falling();
                }
            }
        }
    }

    fn fly(&mut self, world: &dyn ReactWorld, delta: Vec3, before: Vec3) {
        if delta.length_squared() == 0.0 {
            return;
        }
        match world.sweep(self.position, self.position + delta) {
            Some(c) if c.normal.z >= FLOOR_NORMAL_Z => {
                // Skimming a floor: keep the horizontal part.
                self.position += Vec3::new(delta.x, delta.y, 0.0) + Vec3::Z * (delta.z * c.time);
            }
            Some(c) => {
                self.position += delta * c.time + c.normal * 0.1;
                self.blocked(c.normal, before);
            }
            None => self.position += delta,
        }
    }

    /// Something not a floor stopped the move: the wall-hit test (0x1404434C0), else slide.
    fn blocked(&mut self, normal: Vec3, before: Vec3) {
        let p = self.body.params;
        let armed = matches!(self.handler, Handler::Blow | Handler::Parabola) && self.sub < 8;
        let head_on = self.knock.dot(normal) <= -p.wall_hit_angle.to_radians().cos();
        if armed && horizontal_speed(before) > p.wall_hit_speed && head_on {
            self.ev_wall = true;
            // Front / back clip by the side the wall is on; a side index is settled by the wall.
            if self.index >= 2 {
                let right = Vec3::new(-forward(self.yaw).y, forward(self.yaw).x, 0.0) * if self.index == 2 { 1.0 } else { -1.0 };
                self.index = (right.dot(normal) < 0.0) as u8;
            }
            self.facing = if self.index == 0 { yaw_of(flat(normal)) } else { yaw_of(-flat(normal)) };
            self.brake_on = false;
            self.stop();
            self.gravity_on = false;
            self.play(Clip::WallHit);
            self.sub = 8;
            return;
        }
        // Slide: the velocity into the surface is lost.
        let into = self.velocity.dot(normal);
        if into < 0.0 {
            self.velocity -= normal * into;
        }
    }
}
