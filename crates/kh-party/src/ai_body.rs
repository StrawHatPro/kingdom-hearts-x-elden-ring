//! Stream C's `kh_ai::AiBody` over a party member (contract C.2): the view the decorators read,
//! the blueprint functions by name, the commands the latent tasks issue and their tickets.

use crate::command::{CommandStatus, CommandTicket, LocomotionDef, MemberCommand, MoveGoal, TargetRef};
use crate::ids::MemberId;
use crate::party::Party;
use crate::states::NpcStateId;
use kh_ai::{ActorRef, AiBody, AiCommand, AiCommandStatus, AiGoal, BodyView};
use kh_core::host::TargetId;
use kh_core::status::ItemKind;

/// One member seen as the AI's body. F builds one per member per step.
pub struct PartyBody<'a> {
    pub party: &'a mut Party,
    pub member: MemberId,
}

/// `ETresStateID` name of a state (what `TresBTDecorator_TresStateCheck` compares; the enum's own
/// value names: IDLE 1, JUMP 3, HOPPING 9, SLOPESLIDE 10, DAMAGE 19, SUPER_SLIDE 21, RAIL_SLIDE 22,
/// FRIENDATTACK 29, TURN 44, DIE 59, REFLECT 66, PLAY_MOTION 72, AI_MOVE 98, AI_ACTION 99, USE_ITEM 111,
/// BADSTAT_STUN 122 / FREEZE 123 / BURN 124, WARP 125, SITUATION_ACTION 147, BADSTAT_SNEEZE 151).
pub fn state_name(s: NpcStateId) -> &'static str {
    match s {
        NpcStateId::Idle => "TSID_STATE_IDLE",
        NpcStateId::Move => "TSID_STATE_AI_MOVE",
        NpcStateId::Turn => "TSID_STATE_TURN",
        NpcStateId::Jump => "TSID_STATE_JUMP",
        NpcStateId::Action => "TSID_STATE_AI_ACTION",
        NpcStateId::PlayMotion => "TSID_STATE_PLAY_MOTION",
        NpcStateId::Damage => "TSID_STATE_DAMAGE",
        NpcStateId::Reflect => "TSID_STATE_REFLECT",
        NpcStateId::BadStatStun => "TSID_STATE_BADSTAT_STUN",
        NpcStateId::BadStatBurn => "TSID_STATE_BADSTAT_BURN",
        NpcStateId::BadStatFreeze => "TSID_STATE_BADSTAT_FREEZE",
        NpcStateId::BadStatSneeze => "TSID_STATE_BADSTAT_SNEEZE",
        NpcStateId::Die => "TSID_STATE_DIE",
        NpcStateId::UseItem => "TSID_STATE_USE_ITEM",
        NpcStateId::Warp => "TSID_STATE_WARP",
        NpcStateId::SituationAction => "TSID_STATE_SITUATION_ACTION",
        NpcStateId::SlopeSlide => "TSID_STATE_SLOPESLIDE",
        NpcStateId::RailSlide => "TSID_STATE_RAIL_SLIDE",
        NpcStateId::SuperSlide => "TSID_STATE_SUPER_SLIDE",
        NpcStateId::Hopping => "TSID_STATE_HOPPING",
        // (B5) ETresStateID 0x2B / 7 / 6
        NpcStateId::NavLinkJump => "TSID_STATE_NAVLINK_JUMP",
        NpcStateId::Climbing => "TSID_STATE_CLIMBING",
        NpcStateId::Dangle => "TSID_STATE_DANGLE",
        NpcStateId::Fr(_) => "TSID_STATE_FRIENDATTACK",
        // (party P) a pad-driven member is not the AI's; named for traces only
        NpcStateId::Pad(_) => "TSID_STATE_PAD",
        // (LADDER2) the port's ladder jump: no ETresStateID of the game (no tree check matches it)
        NpcStateId::Ladder => "TSID_STATE_LADDER_PORT",
    }
}

/// `FriendEvaluationData` row name of an item.
pub fn item_row(i: ItemKind) -> &'static str {
    match i {
        ItemKind::Potion => "POTION",
        ItemKind::HiPotion => "HIPOTION",
        ItemKind::MegaPotion => "MEGAPOTION",
        ItemKind::Ether => "ETHER",
        ItemKind::HiEther => "HIETHER",
        ItemKind::MegaEther => "MEGAETHER",
        ItemKind::Elixir => "ELIXIR",
        ItemKind::Megalixir => "LASTELIXIR",
        ItemKind::Refocuser => "FOCUS",
        ItemKind::Panacea => "ALLCURE",
        _ => "",
    }
}

fn item_of(row: &str) -> Option<ItemKind> {
    ItemKind::ALL.into_iter().find(|&i| item_row(i) == row)
}

fn target(a: ActorRef) -> TargetRef {
    match a {
        ActorRef::None => TargetRef::None,
        ActorRef::Player => TargetRef::Sora,
        ActorRef::Member(m) => TargetRef::Member(MemberId(m)),
        ActorRef::Enemy(t) => TargetRef::Enemy(TargetId(t)),
    }
}

fn goal(g: AiGoal) -> MoveGoal {
    match g {
        AiGoal::Point(p) => MoveGoal::Point(p),
        AiGoal::Actor(ActorRef::Player) | AiGoal::Actor(ActorRef::None) => MoveGoal::Sora,
        AiGoal::Actor(ActorRef::Member(m)) => MoveGoal::Member(MemberId(m)),
        AiGoal::Actor(ActorRef::Enemy(t)) => MoveGoal::Target(TargetId(t)),
    }
}

impl AiBody for PartyBody<'_> {
    fn view(&self) -> BodyView {
        let Some(v) = self.party.view(self.member) else { return BodyView::default() };
        let mut items = [None; 4];
        for (i, s) in v.items.iter().enumerate() {
            if s.present {
                items[i] = s.item.map(item_row);
            }
        }
        BodyView {
            member: self.member.0,
            uid: v.chara.uid() as i32,
            position: v.position,
            yaw: v.yaw,
            velocity: v.velocity,
            on_ground: v.on_ground,
            radius: v.capsule.0,
            half_height: v.capsule.1,
            state: state_name(v.state),
            hp: v.hp,
            hp_max: v.hp_max,
            mp: v.mp,
            mp_max: v.mp_max,
            mp_charging: v.mp_charging,
            ko: v.ko,
            in_battle: v.in_battle,
            swimming: v.swimming,
            ride_on: v.ride_on,
            can_battle: v.can_battle,
            in_damage_reaction: v.state == NpcStateId::Damage,
            items,
        }
    }

    fn function(&self, name: &str) -> Option<f32> {
        let m = self.party.member(self.member)?;
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        match name {
            "IsStateMoving" => Some(b(m.state == NpcStateId::Move)),
            "IsBattleFollowMode" => Some(b(m.battle_follow)),
            // Stream C (relay 2026-10-15): Goofy's root waits forever under "IsActionOff() != true";
            // the normal-play answer is 1.
            "IsActionOff" => Some(1.0),
            "IsBreakPoseNow" | "CanLead" | "CanNpcMode" => Some(0.0),
            "CanbeSwirlUpDamageReactionByEnemy" => Some(b(m.state != NpcStateId::Die)),
            _ => self.party.function(self.member, name),
        }
    }

    fn command(&mut self, cmd: AiCommand) -> u32 {
        let chara = match self.party.member(self.member) {
            Some(m) => m.chara,
            None => return 0,
        };
        let mc = match cmd {
            AiCommand::MoveTo { goal: g, locomotion, walk, acceptable_radius, follow_path } => {
                let loco = if locomotion.contains("Slow") { LocomotionDef::MoveSlow } else if locomotion.contains("SlopeSlide") { LocomotionDef::SlopeSlide } else { LocomotionDef::Move1 };
                MemberCommand::MoveTo { goal: goal(g), locomotion: loco, walk, acceptable_radius, follow_path }
            }
            AiCommand::TurnTo { goal: g } => MemberCommand::TurnTo { goal: goal(g) },
            AiCommand::Action { def, target: t, location } => {
                let name = def.trim_start_matches("n_ex001_").trim_start_matches("n_ex002_");
                match crate::ActionDef::from_asset(chara, name) {
                    Some(d) => MemberCommand::Action { def: d, target: if t == ActorRef::None { location.map(TargetRef::Point).unwrap_or(TargetRef::None) } else { target(t) } },
                    None => return 0, // ticket 0 = Unknown -> the task fails
                }
            }
            AiCommand::UseItem { item, target: t } => match item_of(item) {
                Some(i) => MemberCommand::UseItem { item: i, target: target(t) },
                None => return 0,
            },
            AiCommand::Cure { target: t } => MemberCommand::Cure { target: target(t) },
            AiCommand::Warp { goal, yaw } => MemberCommand::Warp { goal, yaw },
            AiCommand::SetBattleFollowMode(on) => MemberCommand::SetBattleFollowMode(on),
            AiCommand::Focus(a) => MemberCommand::Focus(target(a)),
            AiCommand::Idle { time } => MemberCommand::Idle { time },
            AiCommand::Stop => MemberCommand::Stop,
        };
        self.party.command(self.member, mc).0
    }

    /// (B5) the RailSlide state's off-rail warp query (traverse::warp_query_wanted).
    fn wants_warp_query(&self) -> bool {
        self.party.warp_query_wanted(self.member)
    }

    fn warp_query_result(&mut self, best: Option<glam::Vec3>) {
        self.party.answer_warp_query(self.member, best);
    }

    fn command_status(&self, ticket: u32) -> AiCommandStatus {
        match self.party.command_status(self.member, CommandTicket(ticket)) {
            CommandStatus::Running => AiCommandStatus::Running,
            CommandStatus::Succeeded => AiCommandStatus::Succeeded,
            CommandStatus::Failed => AiCommandStatus::Failed,
            CommandStatus::Aborted => AiCommandStatus::Aborted,
            CommandStatus::Unknown => AiCommandStatus::Unknown,
        }
    }
}
