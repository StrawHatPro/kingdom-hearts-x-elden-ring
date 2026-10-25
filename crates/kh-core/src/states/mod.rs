//! The player states: `TresPlayerState_*` objects run by the pawn's `TresStateQueueComponent`
//! (docs/sora-movement-spec.md revision 2, section 1.3; the request interface: 2.5).
//!
//! # How a state is added
//!
//! 1. One file `states/<name>.rs` with a plain-data struct (`Clone, Copy, Debug, PartialEq`) for
//!    the state object's fields and `impl PlayerState for` it (host.rs: `ID`, `sub_step`,
//!    `begin`, `tick`, `end`, optionally `on_notifies` and `movement_state`), plus the request
//!    functions that queue it (`TryX` / `XRequest` of the game's request interface, written like
//!    `run::try_run` and `jump::jump_request`).
//! 2. One row in `player_states!` below.
//! 3. If the controller polls for it: one call in controller.rs, at its place in the game's
//!    dispatch order.
//!
//! Nothing else: `StateData`, the dispatch, `Output::state` / `sub_step` and the `StateChange`
//! event come from the row. Rules that several states share live outside this folder
//! (`combat/`, `magic/`, `abilities/`), data in `status/`.
//!
//! # Who owns what
//!
//! | file | owner | contents |
//! |---|---|---|
//! | `mod.rs` | shared: ONE ROW per state, nothing else | this registry |
//! | `host.rs` | the state-machine host (changes need a reason in the port plan) | `PlayerState`, `Queue` (`SetState` / `PushState` / `ClearQueue` / `DoFinish` / `DoChain`), `StateFlags`, `MainProc` |
//! | `common.rs` | shared helpers of the stage-1 states | "on ground", the ground probe, `Turn`, `SetMovementInputXY` |
//! | `idle.rs`, `run.rs`, `jump.rs` | movement (stage 1; behaviour pinned by tests/golden.rs) | `TresPlayerState_Idle / _Run / _Jump` and `TryRun`, `JumpRequest`, `Fall` ... |
//! | `<new>.rs` | the implementer of that state's spec | |
//!
//! # The model (spec 1.3)
//!
//! The queue holds state instances; its front is the running state. `SetState(id)` clears the
//! queue and pushes a fresh (Reset) instance: the change happens at the next `MainProc`.
//! `PushState` appends behind the running state: it begins when the running one finishes.
//! `DoFinish` ends the running state at the next `MainProc`; `DoChain` ends it and begins AND
//! ticks the next one in the same `MainProc` call. An empty queue yields the default state, IDLE.
//! There is no state stack in the game's component: a state that must return to another one
//! queues it. State flags (`m_bEnableChange`, `m_bReverseLeg`) and `m_nSubStep`: host.rs.
//!
//! The state `MainProc` runs LAST in a frame (spec 1.2): what a state writes (input vector,
//! speeds, walk mode, gravity scale, velocity zeroing, play calls) is acted on by the movement
//! component and the pose tick of the NEXT frame; what the movement did this frame (landing,
//! falling) the state sees in the same frame.

pub(crate) mod common; // (XSI) pub(crate): the XS abilities (abilities/xs) use its probes
mod host;

pub(crate) use host::{main_proc, on_notifies, PlayerState, Queue};
pub use attack::AttackView;
pub use reflect::ReflectView;
pub use risk_dodge::RiskDodgeView;
pub use style_finish::FinishView;
pub use damage::{direction_index, DamageView, Handler as DamageHandler, HitOnPlayer};
pub use athletic_flow::AirstepView;
pub use fire_magic::CastView;
pub use host::StateFlags;
pub use open_treasure_box::TreasureView;

use crate::anim::Notifies;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

/// `ETresStateID`: the ids of the game's states (generated from `sheets/reflect/enum-values`;
/// Idle 1, Run 2, Jump 3, Attack 12 ...). `name()` is the short name ("Idle"), `game_name()` the
/// game's ("TSID_STATE_IDLE").
pub use crate::enums::ETresStateID as StateId;

/// THE REGISTRY: one row per implemented state, `Variant => module::Type`. The module is
/// `states/<module>.rs`; the variant names the state in `StateData`.
macro_rules! player_states {
    ($($variant:ident => $module:ident :: $type:ident,)+) => {
        $(pub(crate) mod $module;)+

        /// A state instance: the fields of one `TresPlayerState_*` object.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub(crate) enum StateData {
            $($variant($module::$type),)+
        }

        impl StateData {
            /// The ids of the states that are implemented, in registry order.
            pub const IMPLEMENTED: &'static [StateId] = &[$(<$module::$type as PlayerState>::ID,)+];

            pub fn id(&self) -> StateId {
                match self {
                    $(StateData::$variant(_) => <$module::$type as PlayerState>::ID,)+
                }
            }

            /// `m_nSubStep` (state +0x78).
            pub fn sub_step(&self) -> u8 {
                match self {
                    $(StateData::$variant(state) => state.sub_step(),)+
                }
            }

            pub fn begin(&mut self, cx: &mut Ctx) {
                match self {
                    $(StateData::$variant(state) => state.begin(cx),)+
                }
            }

            pub fn tick(&mut self, cx: &mut Ctx, dt: f32) {
                match self {
                    $(StateData::$variant(state) => state.tick(cx, dt),)+
                }
            }

            pub fn end(&mut self, cx: &mut Ctx) {
                match self {
                    $(StateData::$variant(state) => state.end(cx),)+
                }
            }

            pub fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
                match self {
                    $(StateData::$variant(state) => state.on_notifies(cx, notifies),)+
                }
            }

            pub fn movement_state(&self, pawn: &Pawn) -> MovementState {
                match self {
                    $(StateData::$variant(state) => state.movement_state(pawn),)+
                }
            }
        }

        $(impl From<$module::$type> for StateData {
            fn from(state: $module::$type) -> StateData {
                StateData::$variant(state)
            }
        })+
    };
}

player_states! {
    Idle => idle::Idle,
    Run => run::Run,
    Jump => jump::Jump,
    Attack => attack::Attack,
    Damage => damage::Damage,
    Die => die::Die,
    FireMagic => fire_magic::FireMagic,
    UseItem => use_item::UseItem,
    ShootFlow => shoot_flow::ShootFlow,
    AthleticFlow => athletic_flow::AthleticFlow,
    DodgeRoll => dodge_roll::DodgeRoll,
    Guard => guard::Guard,
    AirSlide => air_slide::AirSlide,
    Glide => glide::Glide,
    WallKick => wall_kick::WallKick,
    SuperSlide => super_slide::SuperSlide,
    Reflect => reflect::Reflect,
    RiskDodge => risk_dodge::RiskDodge,
    WallRun => wall_run::WallRun,
    PoleTurn => pole_turn::PoleTurn,
    RailSlide => rail_slide::RailSlide,
    Dangle => dangle::Dangle,
    SlopeSlide => slope_slide::SlopeSlide,
    Swaying => swaying::Swaying,
    Hopping => hopping::Hopping,
    Floating => floating::Floating,
    StyleChange => style_change::StyleChange,
    ComboLimit => combo_limit::ComboLimit,
    StyleFinish => style_finish::StyleFinish,
    BadstatStun => badstat_stun::BadstatStun,
    BadstatFreeze => badstat_freeze::BadstatFreeze,
    BadstatBurn => badstat_burn::BadstatBurn,
    OpenTreasureBox => open_treasure_box::OpenTreasureBox,
    // (party stream D) the team attacks
    FrShieldGuardian => fr_shield_guardian::FrShieldGuardian,
    FrGoofyShoot => fr_goofy_shoot::FrGoofyShoot,
    FrCallMeteor => fr_call_meteor::FrCallMeteor,
    FrFlareForce => fr_flare_force::FrFlareForce,
}

impl StateData {
    /// The queue's default state (component default `TSID_STATE_IDLE`): what begins when the
    /// queue is empty.
    pub const DEFAULT: StateData = StateData::Idle(idle::Idle::RESET);
}

/// The ids of the states this build implements (the rows of the registry).
pub fn implemented_states() -> &'static [StateId] {
    StateData::IMPLEMENTED
}
