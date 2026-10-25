//! Views out (contract B.5): what the AI (stream C), the HUD and menus (stream F), the drawing
//! (stream A) and the sound / effect layer (stream E) read after a step.

use crate::command::{ActionDef, CommandStatus, CommandTicket, TargetRef};
use crate::ids::{Chara, MemberId};
use crate::states::NpcStateId;
use glam::Vec3;
use kh_core::anim::NotifyPhase;
use kh_core::status::{GameLevel, ItemKind};
use kh_core::TargetId;

/// What the party reads of Sora each step (F fills it from Sora's `Output` / `Status`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoraView {
    /// Actor origin (foot level), cm.
    pub position: Vec3,
    pub yaw: f32,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub dead: bool,
    pub in_battle: bool,
    pub lock_on: TargetId,
    /// THE shared EXP total (`GW+0x7DF0`, stats spec 5.2).
    pub total_exp: i32,
    pub game_level: GameLevel,
    /// The drawn camera (the Warp state keeps the friend and its warp point out of view, FUN_140E5AC70);
    /// None = no camera known (nothing counts as in view).
    pub camera: Option<PartyCamera>,
    /// (B4) The leader's `ETresStateID` number (Idle 1, Run 2, Jump 3 .. RailSlide 0x16), 0 = unknown: the
    /// RailSlide state's off-rail warp waits while the leader is in 4 / 8 / 9 / 0x16 / 0x8A (0x14124E890).
    pub state: i32,
    /// (party K1) The leader's MaxWalkSpeed this step (cm/s: Sora 680, 980 sprinting; a friend 680), 0 = unknown: the
    /// FollowPlayer speed rule (0x1412196C0) follows it (crate::locomotion).
    pub max_speed: f32,
    /// (party K1) The leader's AnalogInputModifier (1, or the walk rate in walk mode): the same rule near him.
    pub analog: f32,
    /// (party AIX) AI SORA while a friend leads (our design: KH3 has no friend leader): his id among the AI's allies
    /// (kh-partyhost SORA_ACTOR) and his feet. A Cure, an item or an Ether the friends aim at him, and the Cure's area
    /// round its target, reach him through `PartyOutput::ai_sora_heals` (before: lost on a member that does not exist).
    pub ai_sora: Option<(MemberId, Vec3)>,
}

/// The view the Warp state tests against: position, rotation (deg), horizontal FOV (deg), aspect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartyCamera {
    pub position: Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub fov: f32,
    pub aspect: f32,
}

impl PartyCamera {
    /// FUN_140E5AC70 (as stream C2 read it, kh-ai `controller::in_view`): a sphere is in view when it is in
    /// front of the near plane (10 cm) and inside the horizontal / vertical frustum widened by its radius.
    pub fn sees(&self, center: Vec3, radius: f32) -> bool {
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let fwd = Vec3::new(cp * cy, cp * sy, sp);
        let right = Vec3::new(-sy, cy, 0.0);
        let up = right.cross(fwd).normalize_or_zero() * -1.0;
        let d = center - self.position;
        let x = d.dot(fwd);
        if x + radius < 10.0 {
            return false;
        }
        let th = (self.fov.max(1.0).to_radians() * 0.5).tan();
        let tv = th / self.aspect.max(0.1);
        d.dot(right).abs() <= x * th + radius * (1.0 + th * th).sqrt() && d.dot(up).abs() <= x * tv + radius * (1.0 + tv * tv).sqrt()
    }
}

impl Default for SoraView {
    fn default() -> SoraView {
        SoraView { position: Vec3::ZERO, yaw: 0.0, velocity: Vec3::ZERO, on_ground: true, hp: 1, hp_max: 1, mp: 0, mp_max: 0, dead: false, in_battle: false, lock_on: TargetId::NONE, total_exp: 0, game_level: GameLevel::Standard, camera: None, state: 0, max_speed: 0.0, analog: 1.0, ai_sora: None }
    }
}

/// One of the four item slots a friend can have (Donald / Goofy start with two; `TresChrInitData`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ItemSlot {
    pub item: Option<ItemKind>,
    /// Still there (used items are gone until the pouch is refilled).
    pub present: bool,
}

/// Seconds left on each action's cooldown (the BT's cooldown decorators run in C; these are the
/// pawn-side waits: MP charge, the attack's own recovery).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ActionCooldowns {
    /// The pawn is in an action's recovery and refuses a new action for this long.
    pub recovery: f32,
}

/// The clip the drawing evaluates (A's `party_draw` takes these names).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct AnimView {
    /// Anim-set row name of the clip in the montage slot ("" = none yet).
    pub clip: &'static str,
    /// Sequence asset name (the pack file `anims/<sequence>.glb`).
    pub sequence: &'static str,
    pub time: f32,
    /// Blend-in weight of `clip` over the saved pose of the previous clip, 0..1.
    pub weight: f32,
    pub prev_clip: &'static str,
    pub prev_sequence: &'static str,
    pub prev_time: f32,
    /// Blend-space X input when the clip is a blend space: the locomotion speed (cm/s) for 1D rows; for the
    /// 2D rows (WalkRunN / W / B) the turn input (the yaw rate, deg/s [inf]).
    pub blend_axis: f32,
    /// Blend-space Y input: the speed (cm/s) of the 2D rows, 0 for 1D rows / sequences (A2 request).
    pub blend_axis_y: f32,
    /// (ZETTA) The face-clip slot (crate::face): a face clip the host plays on both face slots over the body pose.
    pub face: crate::face::FaceView,
}

/// A member as the others see it after a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MemberView {
    pub id: MemberId,
    pub chara: Chara,
    pub state: NpcStateId,
    pub state_name: &'static str,
    pub position: Vec3,
    pub yaw: f32,
    pub velocity: Vec3,
    pub on_ground: bool,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    /// MP is recharging (after a Cure / running out): `MyMpChargeTime` 30 s [A].
    pub mp_charging: bool,
    /// Charge progress 0..1 while `mp_charging`.
    pub mp_charge: f32,
    pub level: u32,
    pub ko: bool,
    /// Seconds until a KO'd member gets up.
    pub revive_left: f32,
    pub in_battle: bool,
    pub swimming: bool,
    pub ride_on: bool,
    pub can_battle: bool,
    pub items: [ItemSlot; 4],
    pub command: (CommandTicket, CommandStatus),
    pub cooldowns: ActionCooldowns,
    pub anim: AnimView,
    /// Capsule (radius, half height) for the host's targets / push bodies.
    pub capsule: (f32, f32),
    /// (B5) The body pitch (deg, UE sign: + nose up) the actor rotation holds: the SlopeSlide tick 0x14126C950 eases it
    /// toward the slope's pitch + 25 (vt+0xDB0); 0 in every other state (the Ends set the rotation to the yaw only,
    /// FUN_143EAD160). The drawing rotates the actor by (pitch, yaw, 0) about the capsule centre.
    pub pitch: f32,
    /// (party I2) The AI controller focus (TresBTService_GameplayFocus -> MemberCommand::Focus): the head look-at
    /// target of ATresAIController_BT 0x140527460 (None / the own pawn = no look-at).
    pub focus: TargetRef,
}

/// Something a member did this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartyEvent {
    pub member: MemberId,
    pub kind: PartyEventKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PartyEventKind {
    StateChanged { from: NpcStateId, to: NpcStateId },
    /// A notify of the running clip (E plays the notify-driven sounds / effects).
    Notify { clip: &'static str, sequence: &'static str, index: u16, phase: NotifyPhase },
    /// A state-driven voice line by `TresVoiceGroupSet` group name (E picks the entry).
    Voice { group: &'static str },
    Hit { target: TargetId, damage: f32, killed: bool, attack: &'static str },
    Damaged { amount: i32, hp: i32 },
    Ko,
    Revived,
    LevelUp { level: u32 },
    ItemUsed { item: ItemKind, target: TargetRef },
    SpellCast { def: ActionDef },
    ProjectileSpawned { kind: &'static str, at: Vec3 },
    ProjectileEnded { kind: &'static str, at: Vec3 },
    Warped { from: Vec3, to: Vec3 },
    /// (B4) A situation command offered / taken back by the friend (pawn vt+0x2618(id, 0, 1) / FUN_14125CE90;
    /// Goofy's coop 0xCE): F's HUD shows it.
    SituationCommand { command: u16, on: bool },
}

/// A heal on Sora from a member (Cure, an item): the host applies it to Sora's status.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoraHeal {
    pub amount: i32,
    /// HUD effect kind (as `Targets::heal_ally`).
    pub effect: u8,
    /// MP restored (Ether on Sora), 0 = none.
    pub mp: i32,
    pub from: MemberId,
}

/// The result of one `Party::step`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PartyOutput {
    pub events: Vec<PartyEvent>,
    pub sora_heals: Vec<SoraHeal>,
    /// (party AIX) heals / MP aimed at AI Sora (`SoraView::ai_sora`): the host applies them to Sora's status.
    pub ai_sora_heals: Vec<SoraHeal>,
}
