//! OWNER: party stream D (team attacks). The friends' side of KH3's team attacks with Donald and
//! Goofy: the NPC link points and team-command offers (`TresNpcPawnBase` 0x14125EB40 ...), the NPC
//! FR states (`TresNpcState_FRCallMeteor` / `_FRFlareForce` / `_FRShieldGuardian` /
//! `_n_ex002_FRGoofyShoot` / `_n_ex001_FRDonaldSora` / `_n_ex002_FRGoofySora`) and the link pawns
//! (`ATresFriendLinkPawnCallMeteor` / `FlareForce` / `GoofyShoot` / `ShieldGuardian`) with their
//! projectiles. Contract: `docs/party-contracts.md` section D.
//!
//! How it runs (per fixed step, after `Party::step`): the host relays Sora's
//! `Targets::friend_link` requests to [`Team::request`], calls [`Team::step`] with the party (as
//! [`TeamParty`]) and the party host, and answers Sora's `Targets::friends` from
//! [`Team::friend_rows`]. While a member runs a team attack, its pawn is in B's passive FR state
//! (`NpcStateId::Fr(n)`), and this module drives it through [`TeamParty::fr_drive`].
//! (Stream B: this is D's file; B's placeholder was replaced, `pub mod team;` stays in lib.rs.)

use crate::command::{CommandTicket, MemberCommand};

use crate::ids::{Chara, MemberId};
use crate::states::NpcStateId;
use glam::Vec3;
use kh_core::host::TargetId;

/// The FR NPC states' `party-states` ids (`NpcStateId::Fr(n)`); stream D's rows of the sheet.
pub mod fr_id {
    /// `TresNpcState_FRCallMeteor` (ctor 0x14123FA10).
    pub const CALL_METEOR: u16 = 1;
    /// `TresNpcState_FRFlareForce` (ctor 0x14123FA60).
    pub const FLARE_FORCE: u16 = 2;
    /// `TresNpcState_n_ex002_FRGoofyShoot` (ctor 0x1411BBC70).
    pub const GOOFY_SHOOT: u16 = 3;
    /// `TresNpcState_FRShieldGuardian` (ctor 0x14123FC90), both friends.
    pub const SHIELD_GUARDIAN: u16 = 4;
    /// `TresNpcState_n_ex001_FRDonaldSora` (ctor 0x1411BBBB0): Unison Fire / Blizzard / Thunder.
    pub const DONALD_SORA: u16 = 5;
    /// `TresNpcState_n_ex002_FRGoofySora` (ctor 0x1411BBCB0): Fusion Spin / Rocket.
    pub const GOOFY_SORA: u16 = 6;
}

/// The nine team attacks in scope (plan 2.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TeamAttack {
    UnisonFire,
    UnisonBlizzard,
    UnisonThunder,
    FusionSpin,
    FusionRocket,
    DonaldMeteor,
    FlareForce,
    GoofyBombardier,
    TrinityGuard,
}

impl TeamAttack {
    /// `ETresCommandKind` value.
    pub const fn command(self) -> i32 {
        match self {
            TeamAttack::UnisonFire => 22,
            TeamAttack::UnisonBlizzard => 23,
            TeamAttack::UnisonThunder => 24,
            TeamAttack::FusionSpin => 25,
            TeamAttack::FusionRocket => 26,
            TeamAttack::DonaldMeteor => 207,
            TeamAttack::FlareForce => 208,
            TeamAttack::GoofyBombardier => 195,
            TeamAttack::TrinityGuard => 193,
        }
    }
}

/// One party member as the team code needs it (B fills it; see [`TeamParty`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeamMember {
    pub id: MemberId,
    pub chara: Chara,
    pub position: Vec3,
    pub yaw: f32,
    pub on_ground: bool,
    pub ko: bool,
    pub state: NpcStateId,
    pub in_battle: bool,
    /// Equipped COMBI_TIME (197) / COMBI_UP (196) copies.
    pub combi_time: u8,
    pub combi_up: u8,
    /// The running FR clip's name, time and "it reached its end" (B's anim slot), for the FR states.
    pub clip: &'static str,
    pub clip_time: f32,
    pub clip_ended: bool,
}

/// What the team code makes a member in its FR state do this step (B's passive FR state applies it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrDrive {
    /// Actor origin and yaw to hold (None = keep / B's own movement with gravity).
    pub transform: Option<(Vec3, f32)>,
    /// Play this anim-set row (name, blend-in s, loop); None = keep the running clip.
    pub motion: Option<(&'static str, f32, bool)>,
    pub visible: bool,
    pub weapon_visible: bool,
    /// `vt+0x2068` invincible / `vt+0x6A0` collision of the FR states.
    pub invincible: bool,
    pub collision: bool,
}

/// The party as the team code drives it (B implements this for `Party`).
pub trait TeamParty {
    /// Every member, spawn order.
    fn team_members(&self, out: &mut Vec<TeamMember>);
    /// `Party::command` (EnterFrState { state } / Stop).
    fn team_command(&mut self, id: MemberId, cmd: MemberCommand) -> CommandTicket;
    /// Applied in the member's FR state at its next step.
    fn fr_drive(&mut self, id: MemberId, drive: FrDrive);
    /// (D3) A hit of the member's OWN attack row (its `n_ex00x_AttackDataTable`) on `target` at
    /// `at`, delivered as that member's attack (B: `actions::deliver` with the row found by name):
    /// Goofy's `FR_GOOFYSHOOT_RUSH` while he rushes in Goofy Bombardier (his shield's ATK_00
    /// sphere 150 cm, `w_gu_WepBase` [A]). True when the host accepted it. Default: not delivered.
    fn fr_hit(&mut self, _id: MemberId, _row: &'static str, _target: TargetId, _at: Vec3, _host: &mut crate::host::PartyHost) -> bool {
        false
    }
}

/// What one team step produced (the hits are delivered through `PartyHost::targets`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TeamOutput {
    pub events: Vec<TeamEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TeamEvent {
    /// A friend offered a team command (`NotifySetActionCommand(kind, time, friend)`).
    Offered { member: MemberId, kind: i32, time: f32 },
    Started { kind: i32 },
    Ended { kind: i32 },
    Hit { target: TargetId, damage: f32 },
    ProjectileSpawned { kind: &'static str, at: Vec3 },
    /// `ReceiveActorSpecificAction(action)` of the state-event camera blueprints (sent by a link
    /// pawn; the goofy_shoot camera reads 3 tower / 4 boss / 5 generic). For the camera (D.4).
    ActorSpecificAction { action: u8 },
}


pub mod link;
pub mod proj;
pub use link::Team;
