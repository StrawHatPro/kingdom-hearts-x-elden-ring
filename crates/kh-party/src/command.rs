//! Commands in (contract B.4): what stream C's BT tasks, stream D's team-attack hand-over and the
//! test app's debug words ask a member to do. One command runs at a time per member; its ticket's
//! status is what a latent BT task polls (`EBTNodeResult` InProgress / Succeeded / Failed / Aborted).

use crate::ids::MemberId;
pub use kh_core::status::ItemKind;
use glam::Vec3;
use kh_core::TargetId;

/// Where a move / turn goes. Moving goals are re-read every step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MoveGoal {
    Point(Vec3),
    Sora,
    Member(MemberId),
    Target(TargetId),
}

/// The target of an action, an item or a heal.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum TargetRef {
    #[default]
    None,
    Sora,
    Member(MemberId),
    Enemy(TargetId),
    Point(Vec3),
}

/// `UTresNpcLocomotionDefinition*` assets of the pawns [A] (`n_ex00x_Move1`, `Move_Slow`, the
/// follow-player and slope-slide definitions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LocomotionDef {
    /// `n_ex00x_Move1` (`m_DefaultLocomotionDefinition`): 140 .. 680 cm/s.
    #[default]
    Move1,
    /// `Move_Slow` (`m_Walk`): 140 cm/s, no run start, yaw rate 90.
    MoveSlow,
    /// `UTresNpcLocomotionDefinition_FollowPlayer`.
    FollowPlayer,
    /// `m_SlopeSlide`.
    SlopeSlide,
}

/// The BT action assets (`Blueprints/Npc/n_ex00x/BT/Attack*` / `Action*`; plan 2.4) [A].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActionDef {
    DonaldBeat,
    DonaldSwing,
    DonaldJump,
    DonaldFire,
    DonaldBlizzard,
    DonaldThunder,
    DonaldAero,
    DonaldCure,
    GoofyPush,
    GoofySweep,
    GoofyJump,
    GoofyTornado,
    GoofyTurbo,
    GoofyDiscus,
}

impl ActionDef {
    pub const ALL: [ActionDef; 14] = [
        ActionDef::DonaldBeat,
        ActionDef::DonaldSwing,
        ActionDef::DonaldJump,
        ActionDef::DonaldFire,
        ActionDef::DonaldBlizzard,
        ActionDef::DonaldThunder,
        ActionDef::DonaldAero,
        ActionDef::DonaldCure,
        ActionDef::GoofyPush,
        ActionDef::GoofySweep,
        ActionDef::GoofyJump,
        ActionDef::GoofyTornado,
        ActionDef::GoofyTurbo,
        ActionDef::GoofyDiscus,
    ];

    /// The asset name under `Blueprints/Npc/n_ex00x/BT/` [A].
    pub const fn asset(self) -> &'static str {
        match self {
            ActionDef::DonaldBeat => "Attack1_Beat",
            ActionDef::DonaldSwing => "Attack2_Swing",
            ActionDef::DonaldJump => "Attack3_Jump",
            ActionDef::DonaldFire => "Attack4_MagFire",
            ActionDef::DonaldBlizzard => "Attack5_MagBlizzard",
            ActionDef::DonaldThunder => "Attack6_MagThunder",
            ActionDef::DonaldAero => "Attack7_MagAero",
            ActionDef::DonaldCure => "Action2_MagCure",
            ActionDef::GoofyPush => "Attack1_Push",
            ActionDef::GoofySweep => "Attack2_Sweep",
            ActionDef::GoofyJump => "Attack3_Jump",
            ActionDef::GoofyTornado => "Attack6_Tornado",
            ActionDef::GoofyTurbo => "Attack7_Turbo",
            ActionDef::GoofyDiscus => "Attack8_Discus",
        }
    }

    pub const fn chara(self) -> crate::Chara {
        match self {
            ActionDef::DonaldBeat | ActionDef::DonaldSwing | ActionDef::DonaldJump | ActionDef::DonaldFire | ActionDef::DonaldBlizzard | ActionDef::DonaldThunder | ActionDef::DonaldAero | ActionDef::DonaldCure => crate::Chara::Donald,
            _ => crate::Chara::Goofy,
        }
    }

    /// From an asset name and the character whose folder it is in (both have `Attack3_Jump`).
    pub fn from_asset(chara: crate::Chara, asset: &str) -> Option<ActionDef> {
        let asset = asset.trim_end_matches("_C");
        ActionDef::ALL.into_iter().find(|d| d.chara() == chara && d.asset() == asset)
    }
}

/// A command (contract B.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MemberCommand {
    MoveTo { goal: MoveGoal, locomotion: LocomotionDef, walk: bool, acceptable_radius: f32, follow_path: bool },
    TurnTo { goal: MoveGoal },
    Action { def: ActionDef, target: TargetRef },
    UseItem { item: ItemKind, target: TargetRef },
    Cure { target: TargetRef },
    Warp { goal: Vec3, yaw: f32 },
    SetBattleFollowMode(bool),
    Stop,
    PlayMotion { name: &'static str, looping: bool },
    /// `TresBTTask_Idle`: stand for `time` seconds, then Succeeded.
    Idle { time: f32 },
    /// The controller focus (GameplayFocus service): idle facing; instant.
    Focus(TargetRef),
    /// Stream D: enter an FR (team attack) NPC state by its `party-states` id.
    EnterFrState { state: u16 },
    /// (LADDER2) PORT DESIGN, KH3 has no ladders: leap up the host's ladder onto its landing (crate::ladder; the ER
    /// allies jump right after the leader).
    LadderJump(crate::ladder::LadderJump),
}

/// A command's handle; counts up from 1 per member (0 = none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct CommandTicket(pub u32);

/// What became of a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CommandStatus {
    Running,
    Succeeded,
    Failed,
    /// Replaced by a newer command, or stopped by a reaction (damage, KO, warp).
    Aborted,
    /// No such ticket (never issued, or older than the member's history).
    #[default]
    Unknown,
}
