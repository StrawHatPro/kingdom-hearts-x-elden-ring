//! The NPC states (plan 2.5): `TresNpcState_*` / `TresAICharState_*` as the pawn runs them. One
//! file per state family; the ids are this crate's (the sheet `party-states` maps them to the
//! game's classes, sizes and vtable / Begin / Tick / End addresses).

/// The states of a party member. The values are stable (`party-states.id`); stream D's FR states
/// are `Fr(n)` with `n` their own `party-states` id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum NpcStateId {
    /// `TresNpcState_Idle` (`TSID_STATE_IDLE`).
    #[default]
    Idle,
    /// `TresNpcState_Run` : `TresAICharState_Run` (`TSID_STATE_AI_MOVE`): a move-to under a
    /// locomotion definition.
    Move,
    /// `TresNpcState_Turn` (`TSID_STATE_TURN`).
    Turn,
    /// `TresNpcState_Jump`.
    Jump,
    /// `TresAICharState_Action` / `AttackBase`: one action definition running.
    Action,
    /// `TresAICharState_PlayMotion`.
    PlayMotion,
    /// `TresNpcState_Damage` (+ Goofy's `n_ex002_Damage`) (`TSID_STATE_DAMAGE`).
    Damage,
    /// `TresNpcState_Reflect` (+ Goofy's `n_ex002_Reflect`).
    Reflect,
    BadStatStun,
    BadStatBurn,
    BadStatFreeze,
    BadStatSneeze,
    /// `TresNpcState_Die`: KO, revive timer.
    Die,
    /// `TresNpcState_UseItem`.
    UseItem,
    /// `TresNpcState_Warp`.
    Warp,
    /// `TresNpcState_SituationAction`.
    SituationAction,
    SlopeSlide,
    RailSlide,
    SuperSlide,
    Hopping,
    /// (B5) `UTresAICharState_NavLinkJumpCodeDriven` (`TSID_STATE_NAVLINK_JUMP` 0x2B, ctor 0x14042FA70): the
    /// friends' every jump and fall (RequestJump 0x14054C2E0 tries 0x2B before 3; JumpCodeDriven 0x14054C6A0).
    /// `Jump` (TresNpcState_Jump, id 3) is registered too (ctor 0x141254FF0) but never requested for them.
    NavLinkJump,
    /// (B5) `UTresNpcState_Climbing` (`TSID_STATE_CLIMBING` 7, ctor 0x14123F7F0): the vault onto a ledge.
    Climbing,
    /// (B5) `UTresNpcState_Dangle` : `UTresCharState_Dangle` (`TSID_STATE_DANGLE` 6, ctor 0x14123F870): the hang.
    Dangle,
    /// Stream D: a team-attack (FR) NPC state.
    Fr(u16),
    /// (party P) a pad-driven state of stream P (its own numbering); does not refuse commands.
    Pad(u8),
    /// (LADDER2) PORT DESIGN, KH3 has no ladders: a member leaping up the game host's ladder (crate::ladder).
    Ladder,
}

impl NpcStateId {
    /// The game's class name.
    pub const fn class(self) -> &'static str {
        match self {
            NpcStateId::Idle => "TresNpcState_Idle",
            NpcStateId::Move => "TresNpcState_Run",
            NpcStateId::Turn => "TresNpcState_Turn",
            NpcStateId::Jump => "TresNpcState_Jump",
            NpcStateId::Action => "TresAICharState_Action",
            NpcStateId::PlayMotion => "TresAICharState_PlayMotion",
            NpcStateId::Damage => "TresNpcState_Damage",
            NpcStateId::Reflect => "TresNpcState_Reflect",
            NpcStateId::BadStatStun => "TresNpcState_BadStatStun",
            NpcStateId::BadStatBurn => "TresNpcState_BadStatBurn",
            NpcStateId::BadStatFreeze => "TresNpcState_BadStatFreeze",
            NpcStateId::BadStatSneeze => "TresNpcState_BadStatSneeze",
            NpcStateId::Die => "TresNpcState_Die",
            NpcStateId::UseItem => "TresNpcState_UseItem",
            NpcStateId::Warp => "TresNpcState_Warp",
            NpcStateId::SituationAction => "TresNpcState_SituationAction",
            NpcStateId::SlopeSlide => "TresNpcState_SlopeSlide",
            NpcStateId::RailSlide => "TresNpcState_RailSlide",
            NpcStateId::SuperSlide => "TresNpcState_SuperSlide",
            NpcStateId::Hopping => "TresNpcState_Hopping",
            // (B5)
            NpcStateId::NavLinkJump => "TresAICharState_NavLinkJumpCodeDriven",
            NpcStateId::Climbing => "TresNpcState_Climbing",
            NpcStateId::Dangle => "TresNpcState_Dangle",
            NpcStateId::Fr(_) => "TresNpcState_FR",
            // (party P)
            NpcStateId::Pad(_) => "TresNpcState_Pad (port)",
            // (party ER S0)
            NpcStateId::Ladder => "TresNpcState_Ladder (port)",
        }
    }

    /// A state in which a new command is refused (contract B.4).
    pub const fn refuses_commands(self) -> bool {
        matches!(self, NpcStateId::Damage | NpcStateId::Reflect | NpcStateId::BadStatStun | NpcStateId::BadStatFreeze | NpcStateId::BadStatSneeze | NpcStateId::Die | NpcStateId::Warp | NpcStateId::UseItem | NpcStateId::Fr(_) | NpcStateId::Ladder) // (LADDER2) Ladder: Stop / Warp only (pawn.rs)
    }
}
