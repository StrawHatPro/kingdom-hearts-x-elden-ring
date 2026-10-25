//! The event list of one step: everything the core told the host, in the order it happened.
//! `Output::events` holds the step's list; the same calls were made on the host's traits.

use super::{EffectHandle, EffectSpawn, HitRecord, HitResult, SoundHandle, SoundPlay};
use crate::combat::CursorKind;
use crate::host::TargetId;
use crate::states::{HitOnPlayer, StateId};
use crate::ui_model::UiPopup;

/// One thing that happened in a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    /// The step it happened in: `Output::step` of that step (the first step is 1).
    pub step: u64,
    pub kind: EventKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventKind {
    /// The state queue ended `from` and began `to` (`MainProc`; also when a state is replaced by
    /// a new instance of itself).
    StateChange { from: StateId, to: StateId },
    /// `Audio::play` was called.
    SoundPlay { handle: SoundHandle, play: SoundPlay },
    /// `Audio::stop` was called.
    SoundStop { handle: SoundHandle, fade_out: f32 },
    /// `Effects::spawn` was called.
    EffectSpawn { handle: EffectHandle, spawn: EffectSpawn },
    /// `Effects::stop` was called.
    EffectStop { handle: EffectHandle, immediate: bool },
    /// `Targets::apply_hit` was called and answered.
    Hit { record: HitRecord, result: HitResult },
    /// A popup for the HUD (also in this step's `UiSnapshot::popups`).
    Popup(UiPopup),
    /// The HUD's target cursor (combat strand D 7.11): `kind` Some = the cursor of that kind is
    /// put on the target (auto = the soft target, lock = a manual lock; the first cursor of a
    /// target also makes it the enemy HP gauge's target; a lock cursor plays UI sound 7), None =
    /// the target's cursors are removed.
    LockCursor { target: TargetId, kind: Option<CursorKind> },
    /// `Camera.SetLockon` (strand D 7.12): the lock-on camera's target; `TargetId::NONE` = unlock.
    CameraLock { target: TargetId },
    /// A hit on Sora was applied (`Sora::take_hit`): the HP it took (combat strand E 8.4, 9.7).
    PlayerHit { hit: HitOnPlayer, hp_lost: i32, hp_after: i32 },
    /// His HP reached 0 and the Die state was queued (strand E 9.8): the game-over flow is the host's.
    PlayerDied { causer: TargetId },
    /// A situation command is offered (combat strand F 10.2 / 10.3): the full formchange gauge
    /// offers `CHANGE_L1` for `lifetime` seconds. (The queue itself is not implemented yet.)
    SituationCommand { kind: crate::enums::ETresCommandKind, lifetime: f32 },
    /// The shotlock aim mode opened or closed (magic spec F): the host switches its aim camera
    /// (`kh_camera::aim`). `reason` when closing: 1 cancel, 2 fired, 3 attack finished, 4 airstep,
    /// 5 airstep finished (the HUD close animations of F A9).
    AimMode { open: bool, reason: u8 },
    /// A shotlock lock marker: `locks` on `target` now (0 = the marker is removed).
    LockMarker { target: TargetId, locks: i32 },
    /// A sound-system state (`STATE_SLOW_START` / `STATE_SLOW_END`).
    SoundState { name: &'static str },
    /// The weapon is to be shown or hidden (no trait call: the host draws the weapon).
    /// `instant` (fixes pass 1, PORT OPTION `Tuning::weapon_instant_on_attack`): the mesh is to
    /// be fully visible at once, without the appear sequence's 0.22 s delay and 0.06 s ramp; it
    /// may arrive while the weapon is already being shown (the running sequence is cut short).
    WeaponVisible { visible: bool, instant: bool },
    /// The battle flag changed (actions spec II.3: the host's engaged-enemy count, plus 1.0 s).
    BattleMode { on: bool },
    /// A hit on Sora was dropped by his reaction type 11 (Dodge Roll, Air Slide ...: actions spec
    /// II.1): no damage, no reaction, and the attack is not consumed.
    PlayerDodged { hit: HitOnPlayer },
    /// A hit on Sora was guarded (actions spec II.1 "Guarded hit"): no HP loss. `success` = a
    /// player-requested guard caught it (the counter window opens), `critical` = a just guard.
    PlayerGuarded { hit: HitOnPlayer, success: bool, critical: bool },
    /// (actions implementer) The HUD's risk-dodge prompt (`FUN_141414170` -> 0x14158B260(hud,
    /// mode)): shown while the Reflect state's 0.6 s window is open (press JUMP), hidden when it
    /// closes; `used` = it was taken (mode 3).
    RiskDodgePrompt { shown: bool, used: bool },
    /// (actions implementer) A traversal affordance of the level was taken or left: `kind` =
    /// "wallrun", "pole", "rail", "dangle", "slope", "sway", "hop", "swim", "floating"; `id` = the
    /// host's id of the pole / rail / hop point / ledge component / pawn (0 when there is none).
    Traversal { kind: &'static str, id: u32, on: bool },
    // ---- added by the second combat implementer (2026-10-11)
    /// A command entered (or was refreshed in) one of the queue's single slots: A (context /
    /// reaction commands), B (finish commands, their extras, the single-slot reaction commands).
    /// `total` -1 = no countdown. (Entries of the counted list keep the `SituationCommand` event.)
    SituationSlot { kind: crate::enums::ETresCommandKind, slot: crate::combat::situation::Slot, total: f32, refreshed: bool },
    /// A situation command left the queue: used, expired, evicted by a 7th entry, or deleted.
    SituationRemoved { kind: crate::enums::ETresCommandKind, why: crate::combat::situation::Removal },
    /// L2 moved the selection: the list index, -1 = the event button.
    SituationSelect { index: i32 },
    /// A formchange began (`StartStyle`: combat strand F 10.3): the form's command, its duration.
    FormBegin { kind: crate::enums::ETresCommandKind, duration: f32 },
    /// The wear form is applied (STYLECHANGE sub 3: 5 = Second Form, three texture swaps; the
    /// Keyblade is unchanged) or back to NORMAL (0) (`EndStyle`).
    WearForm { form: u8 },
    /// The form ended (`EndStyle`).
    FormEnd { kind: crate::enums::ETresCommandKind },
    /// The per-object-type time dilation changed (host/time.rs): `mask` = object-type bits,
    /// `rate` = their new rate (1.0 = back to normal).
    TimeDilation { mask: u32, rate: f32 },
    /// A hit on Sora was repelled by him (victim result REFLECT / GUARD_REFLECT: he recoils) or
    /// guarded with a result for the attacker (strand C 8.2.6 with Sora as the victim).
    /// `attacker` REFLECT / GUARD_REFLECT = the host's attacker recoils.
    PlayerGuard { hit: HitOnPlayer, victim: super::ReactionResult, attacker: super::ReactionResult },
    /// An attack of Sora touched level geometry (strand C 8.2.5 step 9): where, the surface normal.
    WallHit { position: glam::Vec3, normal: glam::Vec3 },
    /// An attack of Sora clashed with an attack shape of `target` (strand C 8.2.4): the result
    /// for Sora (REFLECT = he is repelled, GUARD_REFLECT = both recoil).
    Clash { target: TargetId, result: super::ReactionResult },
    /// His auto-revive item was used (strand E 9.8): HP after it.
    AutoRevived { hp: i32 },
    /// (audio implementer) The pawn's voice group number changed (`Audio::voice_group_no` was
    /// called): the unit a voice-GROUP notify plays from now on (audio spec 5.2).
    VoiceGroupNo { number: i32 },
    // ---- added by the third combat implementer (2026-10-12)
    /// The bad status on Sora changed (combat spec 9.9; `ETresBadStatusType`: 7 freeze, 8 stop,
    /// 12 stun, 13 sneeze, 14 honey, 15 cloud, 16 discharge, 17 burn, 18 HP break; 0 = none):
    /// the HUD shows / hides its mark, the host spawns / removes the status actor of 15 / 16.
    BadStatus { from: u8, to: u8, time: f32 },
    // ---- added by the fourth combat pass (2026-10-12)
    /// The attack camera (controller 0x1414122A0, thunk 0x14140DF20; combat spec 7.12.1,
    /// `combat/camera.rs`): `mode` 4 / 5 / 6 / 7 = the normal camera's lag / distance changes of
    /// that mode, 3 = only the lock (sent as `CameraLock`), 0 = off, with `restore` = the seconds
    /// the lag takes back to its defaults (negative: at once). `kh_camera::CameraRig::set_attack_mode`.
    AttackCamera { mode: u8, restore: f32 },
    /// `ClientPlayForceFeedback(effect table[index], looping, no tag)` (pawn vt+0x1048 ->
    /// controller interface slot 4 0x141444270): the game's rumble effect by its index in the
    /// game-wide table (0 = a guard hit, 1 = Stun Impact's blow and the big spells' release).
    ForceFeedback { effect: u8, looping: bool },
    // ---- added for the boss finish (2026-10-13, docs/sora-bossfinish-spec.md)
    /// A hit of Sora's killed a target the host calls finish-eligible (`Targets::finish_kind`): the
    /// host starts the presentation (`combat::finish::FinishSequence`). A host whose kills are
    /// confirmed later (damage through another game's pipeline) starts it on its own confirmation
    /// instead and may ignore this event.
    BossFinish { target: TargetId, kind: super::FinishKind },
    // ---- party stream D (team attacks, 2026-10-15; docs/party-contracts.md D)
    /// `ReceivePlayerSpecificAction(ActionID, SubID)` of the running state's state-event camera
    /// blueprint (`BP_TresPlayerStateEvent*`): what the FR states send for the cameras (e.g.
    /// FRCallMeteor 78 / 79 / 80, FRFlareForce 87). `state` = the sending state.
    PlayerSpecificAction { state: StateId, action: u8, sub: i32 },
    /// A team attack began (phase 0), reached a stage of its own (1..) or ended (255); `kind` =
    /// the command kind (193 Trinity Guard, 195 Goofy Bombardier, 207 Donald Meteor, 208 Flare
    /// Force; 0x3F..0x43 for the unison / fusion finishers).
    TeamAttack { kind: i32, phase: u8 },
    /// `ReceiveAnimNotifyStartBpEvent` / `...EndBpEvent(SendActor = Sora, AnimSeqName, EventID, Param)`
    /// of the state-event camera blueprints: a `TresAnimNotifyState_BpEvent` of Sora's clip `seq`
    /// began (`begin`) or ended. Emitted by the FR states for their clips.
    AnimBpEvent { seq: &'static str, param: i32, begin: bool },
}

impl EventKind {
    /// A short word for logs and overlays: "state", "sound", "sound-stop", "effect",
    /// "effect-stop", "hit", "popup".
    pub const fn label(&self) -> &'static str {
        match self {
            EventKind::StateChange { .. } => "state",
            EventKind::SoundPlay { .. } => "sound",
            EventKind::SoundStop { .. } => "sound-stop",
            EventKind::EffectSpawn { .. } => "effect",
            EventKind::EffectStop { .. } => "effect-stop",
            EventKind::Hit { .. } => "hit",
            EventKind::Popup(_) => "popup",
            EventKind::WeaponVisible { .. } => "weapon",
            EventKind::AimMode { .. } => "aim",
            EventKind::LockMarker { .. } => "lock",
            EventKind::SoundState { .. } => "sound-state",
            EventKind::BattleMode { .. } => "battle",
            EventKind::PlayerDodged { .. } => "player-dodged",
            EventKind::PlayerGuarded { .. } => "player-guarded",
            EventKind::RiskDodgePrompt { .. } => "risk-dodge",
            EventKind::Traversal { .. } => "traversal",
            EventKind::SituationCommand { .. } => "command",
            EventKind::PlayerHit { .. } => "player-hit",
            EventKind::PlayerDied { .. } => "player-died",
            EventKind::LockCursor { .. } => "cursor",
            EventKind::CameraLock { .. } => "camera-lock",
            EventKind::SituationSlot { .. } | EventKind::SituationRemoved { .. } | EventKind::SituationSelect { .. } => "command",
            EventKind::FormBegin { .. } | EventKind::FormEnd { .. } | EventKind::WearForm { .. } => "form",
            EventKind::TimeDilation { .. } => "time",
            EventKind::PlayerGuard { .. } => "player-guard",
            EventKind::WallHit { .. } => "wall-hit",
            EventKind::Clash { .. } => "clash",
            EventKind::AutoRevived { .. } => "revive",
            EventKind::BadStatus { .. } => "bad-status",
            EventKind::AttackCamera { .. } => "attack-camera",
            EventKind::ForceFeedback { .. } => "rumble",
            EventKind::VoiceGroupNo { .. } => "voice-group",
            EventKind::BossFinish { .. } => "boss-finish",
            EventKind::PlayerSpecificAction { .. } => "specific-action",
            EventKind::TeamAttack { .. } => "team-attack",
            EventKind::AnimBpEvent { .. } => "anim-bp-event",
        }
    }
}
