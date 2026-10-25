//! The numbers: the generated sheet constants, the per-victim damage parameters built from them
//! (`TresDamageCommonParam` as an enemy reads it), the clip lengths that end the phases, and the
//! body (what kind of thing is being hit).

use kh_core::host::BodyReaction;

include!(env!("KH_ENEMY_REACTION_PARAMS_RS"));

/// The sheet's values (sheets/port/sora-enemy-reaction.json).
pub const P: EnemyReactionParams = ENEMY_REACTION_PARAMS;

/// `TresDamageCommonParam` as `UTresAICharState_Damage` reads it (state slot 141 gives the object:
/// the pawn's own `m_EnemyCommonParam` class default object, else the game mode's). Every enemy
/// without an override uses [`DamageParams::ENEMY`]; the fifteen overrides of the game are in
/// docs/sora-enemy-reaction-spec.md section 3.2 (a host that wants one copies the default and
/// changes the field).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageParams {
    pub small_power: f32,
    pub small_brake: f32,
    pub small_start_falling_speed: f32,
    pub small_air_velocity_z: f32,
    pub blow_ground_power: f32,
    pub blow_ground_velocity_z: f32,
    pub blow_ground_brake: f32,
    pub blow_air_power: f32,
    pub blow_air_velocity_z: f32,
    pub blow_air_brake: f32,
    pub blow_air_start_brake_time: f32,
    pub blow_start_falling_speed: f32,
    pub toss_power: f32,
    pub toss_flight_duration: f32,
    pub toss_fall_gravity_scale: f32,
    pub launch_power: f32,
    pub launch_flight_duration: f32,
    pub launch_fall_gravity_scale: f32,
    pub beat_power: f32,
    pub parabola_power: f32,
    pub parabola_angle: f32,
    pub parabola_brake: f32,
    pub parabola_limit_brake: f32,
    pub wall_hit_speed: f32,
    pub wall_hit_angle: f32,
    /// World gravity, cm/s² (positive), and the victim's class-default `GravityScale`.
    pub gravity: f32,
    pub gravity_scale: f32,
}

impl DamageParams {
    /// `/Game/Blueprints/Enemy/EnemyCommonParam` over the native constructors.
    pub const ENEMY: DamageParams = DamageParams {
        small_power: P.small_power,
        small_brake: P.small_brake,
        small_start_falling_speed: P.small_start_falling_speed,
        small_air_velocity_z: P.small_air_velocity_z,
        blow_ground_power: P.blow_ground_power,
        blow_ground_velocity_z: P.blow_ground_velocity_z,
        blow_ground_brake: P.blow_ground_brake,
        blow_air_power: P.blow_air_power,
        blow_air_velocity_z: P.blow_air_velocity_z,
        blow_air_brake: P.blow_air_brake,
        blow_air_start_brake_time: P.blow_air_start_brake_time,
        blow_start_falling_speed: P.blow_start_falling_speed,
        toss_power: P.toss_power,
        toss_flight_duration: P.toss_flight_duration,
        toss_fall_gravity_scale: P.toss_fall_gravity_scale,
        launch_power: P.launch_power,
        launch_flight_duration: P.launch_flight_duration,
        launch_fall_gravity_scale: P.launch_fall_gravity_scale,
        beat_power: P.beat_power,
        parabola_power: P.parabola_power,
        parabola_angle: P.parabola_angle,
        parabola_brake: P.parabola_brake,
        parabola_limit_brake: P.parabola_limit_brake,
        wall_hit_speed: P.wall_hit_speed,
        wall_hit_angle: P.wall_hit_angle,
        gravity: P.gravity,
        gravity_scale: 1.0,
    };
}

/// How long the victim's reaction clips are, seconds. In the game a phase ends when ITS clip
/// ends, and every enemy has its own clips: the defaults are the medians over the game's enemies
/// (the sheet's `clip.*` rows). A host with its own animations puts their lengths here, so that
/// the motion and the picture end together.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipTimes {
    /// Ground flinch (`200`..`206`): the whole SMALL reaction on the ground.
    pub small_ground: f32,
    /// Air flinch (`210`..`216`).
    pub small_air: f32,
    /// Landing on its feet out of an air flinch (`Land`).
    pub land: f32,
    /// BLOW / PARABOLA start (`240`), then the loop.
    pub blow_start: f32,
    /// Landing + lying + getting up after a BLOW (`240_2`): one clip.
    pub blow_down: f32,
    /// TOSS / LAUNCH start (`m_250`), then the rise loop.
    pub toss_start: f32,
    /// The fall start played when the apex hover ends (`m_250_2`), then the fall loop.
    pub toss_fall_start: f32,
    /// Landing + lying + getting up after TOSS / LAUNCH (`m_250_4`).
    pub toss_down: f32,
    /// The ground slam of a BEAT (`m_259`); `None` (most enemies have no such clip): the reaction
    /// ends at the landing.
    pub beat_down: Option<f32>,
    /// SIDESPIN / VERTICALROLL (`m_260` / `m_270`); `None`: the enemy has no such clip and the hit
    /// is a BLOW (Setup 0x140457380).
    pub spin: Option<f32>,
    /// The distance the spin's root motion carries the victim away from the attacker, cm.
    pub spin_distance: f32,
    /// Landing + getting up after the spin (`m_260_1`).
    pub spin_down: f32,
    /// Stuck to the wall (`280`).
    pub wall: f32,
    /// Landing + getting up after a wall hit (`280_2`).
    pub wall_down: f32,
    /// NOT GAME DATA: how much of the END of a down clip the phase is reported as
    /// [`crate::Phase::GettingUp`] instead of [`crate::Phase::Down`]. The game has one clip for
    /// both and no such boundary; a host that animates the two separately sets its own time.
    pub getup_tail: f32,
}

impl ClipTimes {
    /// NOT MEASURED (sheet row `clip.land` is unchecked): the enemies' `Land` clip length.
    pub const LAND_STAND_IN: f32 = 0.3;

    /// The medians of the game's enemies.
    pub const MEDIAN: ClipTimes = ClipTimes {
        small_ground: P.clip_small_ground,
        small_air: P.clip_small_air,
        land: ClipTimes::LAND_STAND_IN,
        blow_start: P.clip_blow_start,
        blow_down: P.clip_blow_down,
        toss_start: P.clip_toss_start,
        toss_fall_start: P.clip_toss_fall_start,
        toss_down: P.clip_toss_down,
        beat_down: None,
        spin: Some(P.clip_spin),
        spin_distance: P.clip_spin_distance,
        spin_down: P.clip_spin_down,
        wall: P.clip_wall,
        wall_down: P.clip_wall_down,
        getup_tail: 0.6,
    };
}

/// A coarse name for what is being hit. KH3 itself has NO weight that scales a trajectory: every
/// enemy on the common parameters flies the same arcs. What differs per enemy is its row of
/// `TresChrBaseParamDataTable` (`m_BodyPushPower`, `m_MaxBodyStrongValue`, `m_MaxArmorHP`,
/// `m_RevengeLimit`) and the reaction type of its hurt shapes; the classes below are the four
/// groups those rows fall into (docs/sora-enemy-reaction-spec.md section 7), for a host that has
/// to sort foreign enemies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BodyClass {
    /// Push level PPL_10..40, DEFAULT body, no endurance (`e_ex001`).
    Light,
    /// PPL_50..60, mostly a DEFAULT body without endurance, often a revenge limit of 6.5..8 (`e_ex003`).
    #[default]
    Medium,
    /// PPL_70..90, often with `m_MaxBodyStrongValue` 5..15 and a STRONG body: reacts only to hits
    /// that carry reaction power once the endurance is broken (`e_ex011`, `e_ex018`).
    Heavy,
    /// PPL_MAX / endurance 25..50 / bosses: as Heavy with a deeper endurance; many never leave
    /// the ground. A host may also give it `Body::never_reacts`.
    Giant,
}

/// The victim: what the reaction needs to know about it besides the hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub class: BodyClass,
    /// `ETresBodyCollReactionType` of its hurt shapes, for the host's `Targets::body`.
    pub reaction: BodyReaction,
    /// `m_MaxBodyStrongValue`: the reaction endurance of a STRONG body.
    pub strong_max: i32,
    /// `m_RevengeLimit` (0 = no revenge), `m_RevengeCoolDownTime`.
    pub revenge_limit: f32,
    pub revenge_cooldown: f32,
    /// Capsule radius, cm (the BEAT spacing).
    pub radius: f32,
    /// Its default movement is flying (`DefaultLandMovementMode` is not Walking): it does not fall.
    pub flying: bool,
    /// `m_bIsNoDamageResponse` (pawn vt+0x2070): no hit ever starts a reaction.
    pub never_reacts: bool,
    pub params: DamageParams,
    pub clips: ClipTimes,
}

impl Body {
    /// A body of `class` with the game's typical row for it (the medians of the groups in the
    /// spec's section 7) and the common parameters.
    pub const fn of(class: BodyClass) -> Body {
        let (reaction, strong_max, revenge_limit, radius) = match class {
            BodyClass::Light => (BodyReaction::Default, 0, 0.0, 30.0),
            BodyClass::Medium => (BodyReaction::Default, 0, 0.0, 40.0),
            BodyClass::Heavy => (BodyReaction::Strong, 5, 0.0, 80.0),
            BodyClass::Giant => (BodyReaction::Strong, 25, 7.0, 150.0),
        };
        Body { class, reaction, strong_max, revenge_limit, revenge_cooldown: 0.0, radius, flying: false, never_reacts: false, params: DamageParams::ENEMY, clips: ClipTimes::MEDIAN }
    }
}

impl Default for Body {
    fn default() -> Body {
        Body::of(BodyClass::Medium)
    }
}
