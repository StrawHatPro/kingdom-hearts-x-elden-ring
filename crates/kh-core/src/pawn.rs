//! The pawn as the states see it: everything of Sora except the state that is currently running
//! (`Pawn`), and the per-step context that adds the host (`Ctx`). A state's `begin` / `tick` /
//! `end` get `&mut self` and `&mut Ctx`; through the context they reach the movement component,
//! the animation slot, the state queue's request interface, the status, this step's input, and
//! the host seam (world queries, targets, sounds, effects, popups).
//!
//! EVERYTHING THE CORE TELLS THE HOST GOES THROUGH THE METHODS OF `Ctx` AT THE END OF THIS FILE
//! (`play_sound`, `spawn_effect`, `deliver_hit`, `popup` ...): each makes the trait call AND
//! appends the `Event` to the step's list, so the two never disagree. Do not call
//! `cx.host.audio` and friends directly.

use std::ops::{Deref, DerefMut};

use crate::anim::{AnimPlayer, AnimRequest};
use crate::cmc::Cmc;
use crate::host::{EffectHandle, EffectSpawn, Event, EventKind, HitRecord, HitResult, Host, HurtShape, SoundHandle, SoundPlay, TargetId, TargetInfo};
use crate::params::SoraParams;
use crate::sora::{Input, Tuning};
use crate::states::{Queue, StateId};
use crate::status::Status;
use crate::ui_model::UiPopup;
use crate::world::World;

/// Sora without his current state. Plain data.
#[derive(Clone, Debug)]
pub(crate) struct Pawn {
    params: SoraParams,
    pub tuning: Tuning,
    /// The movement component (`TresAICharMovementComponent`, pawn +0x578).
    pub cmc: Cmc,
    /// The actor's yaw, degrees.
    pub yaw: f32,
    /// The `FullBody` montage slot. `anim.reverse_leg` is the state flag `m_bReverseLeg`
    /// (state +0x84 bit 1) as the latest pose tick left it.
    pub anim: AnimPlayer,
    /// The slot as the latest pose tick left it.
    pub anim_ticked: AnimRequest,
    /// Stick magnitude of the latest controller update (controller +0x854).
    pub stick_magnitude: f32,
    /// Things the port does not do yet, said once each.
    pub notes: Vec<&'static str>,
    /// The state queue (`TresStateQueueComponent`, pawn +0x5A0) without the running state.
    pub queue: Queue,
    /// Level, gauges, abilities ... (data only; see status/).
    pub status: Status,
    /// The input of the step in progress (the pad's buttons for the states to come).
    pub input: Input,
    /// Steps taken, counting the one in progress: the first step is 1.
    pub step: u64,
    /// What this step told the host so far, in order (`Output::events`).
    pub events: Vec<Event>,
    /// This step's popups (`UiSnapshot::popups`).
    pub popups: Vec<UiPopup>,
    next_sound: u32,
    next_effect: u32,
    /// (audio implementer) The voice group number (pawn+0x4D8, `SetVoiceGroupNo`): which unit a
    /// voice-group notify plays. Set through `Ctx::set_voice_group_no`; 0 = the default unit.
    pub voice_group_no: i32,
    /// What the notify STATES of the playing animations started (notify.rs): (sequence, notify,
    /// handle), stopped at the state's End.
    pub notify_handles: Vec<(u16, u16, crate::notify::NotifyHandle)>,
    /// (vfx implementer 2) The effect group of the playing animation: the `effectGroup` argument of
    /// the play API (0x1405A0D70 / 0x1405A0E50 call `UTresEffectAttachComponent::Set(mesh, group,
    /// false)`, docs/sora-vfx-spec.md 4.2). An effect row of an AttachEffect notify spawns only if
    /// its `GroupID` is this group or -1 (notify.rs). 0 for every play of the plain Keyblade
    /// except the few of spec 7.4.
    pub effect_group: i32,
    /// The group of the NEXT play: a state that plays with a group other than 0 sets it just
    /// before the play; notify.rs takes it when it sees the new play. None = 0.
    pub next_effect_group: Option<i32>,
    /// The play (`AnimPlayer::play_id`) `effect_group` belongs to.
    pub(crate) effect_group_play: u32,
    /// Scale of the root-motion translation the pose tick hands the movement: (XY, Z)
    /// (`FUN_1405A1D30(pawn+0x440, scaleXY, scaleZ, 0)`, combat spec 6.1). (1, 1) unless a state
    /// sets it; the state that sets it restores it.
    pub root_scale: (f32, f32),
    /// The pawn's melee fields (combat/mod.rs): last attack kind, finisher count, targets ...
    pub combat: crate::combat::PawnCombat,
    /// The magic fields on the pawn and the controller (magic/mod.rs): projectiles, pouch,
    /// the aim mode, the slow-motion channels.
    pub magic: crate::magic::PawnMagic,
    /// The command system (command/mod.rs): the menu, shortcuts, the command timers; and the
    /// menu as the latest step published it.
    pub command: crate::command::CommandState,
    pub command_shown: crate::command::CommandSnapshot,
    /// The stick's world yaw of the latest controller update (controller +0x85C), and the real
    /// (undilated) dt of the step in progress.
    pub stick_yaw: f32,
    pub real_dt: f32,
    /// (DON-KIT) PORT ADDITION, Donald's party auras on Sora (documentation/DONALD_OVERHAUL.md 4.2; `Sora::set_aura`): the
    /// Aero aura's evasion share (the dodges' invincible windows x (1 + this)) and recovery share (his ground hit
    /// reactions and get-up x (1 + this)), and the seconds of the extended window still to run. All 0 = KH3's behaviour.
    pub aura_evasion: f32,
    pub aura_recovery: f32,
    pub aura_ghost: f32,
    /// The fields of the action states (actions.rs): availability bits, reaction type, stance.
    pub actions: crate::actions::PawnActions,
}

impl Pawn {
    pub fn new(params: SoraParams, cmc: Cmc, yaw: f32, anim: AnimPlayer) -> Pawn {
        Pawn {
            params,
            tuning: Tuning::default(),
            cmc,
            yaw,
            anim_ticked: anim.request(),
            anim,
            stick_magnitude: 0.0,
            notes: Vec::new(),
            queue: Queue::new(),
            status: Status::from_loadout(&crate::status::Loadout::empty_placeholder()),
            input: Input::default(),
            step: 0,
            events: Vec::new(),
            popups: Vec::new(),
            next_sound: 0,
            voice_group_no: 0,
            next_effect: 0,
            notify_handles: Vec::new(),
            effect_group: 0,
            next_effect_group: None,
            effect_group_play: 0,
            root_scale: (1.0, 1.0),
            combat: crate::combat::PawnCombat::default(),
            magic: crate::magic::PawnMagic::default(),
            command: crate::command::CommandState::default(),
            command_shown: crate::command::CommandSnapshot::default(),
            stick_yaw: 0.0,
            real_dt: 0.0,
            aura_evasion: 0.0, // (DON-KIT)
            aura_recovery: 0.0,
            aura_ghost: 0.0,
            actions: crate::actions::PawnActions::default(),
        }
    }

    /// After a teleport: the step and handle counters go on from the old pawn's.
    pub fn carry_counters(&mut self, old: &Pawn) {
        (self.step, self.next_sound, self.next_effect) = (old.step, old.next_sound, old.next_effect);
    }

    /// (party X2) The sound / effect handles of this pawn count up from `base` (magic/caster.rs: a friend leader's
    /// proxy caster takes them from its member's party range, never Sora's 1, 2, 3 ...).
    pub fn set_handle_base(&mut self, base: u32) {
        (self.next_sound, self.next_effect) = (base, base);
    }

    pub fn params(&self) -> &SoraParams {
        &self.params
    }

    /// (ER69) For a host that changes the body's size (an outfit of other proportions).
    pub fn params_mut(&mut self) -> &mut SoraParams {
        &mut self.params
    }

    /// Says something once (`Sora::notes`).
    pub fn note(&mut self, note: &'static str) {
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
    }

    pub fn event(&mut self, kind: EventKind) {
        self.events.push(Event { step: self.step, kind });
    }

    /// The state queue began `to` after ending `from` (called by `MainProc`).
    pub fn state_changed(&mut self, from: StateId, to: StateId) {
        self.event(EventKind::StateChange { from, to });
    }
}

/// One step's view of the pawn and the host. Derefs to [`Pawn`]: `cx.cmc`, `cx.anim`, `cx.queue`.
pub(crate) struct Ctx<'a, 'h> {
    pub pawn: &'a mut Pawn,
    pub host: &'a mut Host<'h>,
    /// The host's collision world (`host.world`).
    pub world: &'a dyn World,
}

impl<'a, 'h> Ctx<'a, 'h> {
    pub fn new(pawn: &'a mut Pawn, host: &'a mut Host<'h>) -> Ctx<'a, 'h> {
        let world: &'a dyn World = host.world;
        Ctx { pawn, host, world }
    }
}

impl Deref for Ctx<'_, '_> {
    type Target = Pawn;

    fn deref(&self) -> &Pawn {
        self.pawn
    }
}

impl DerefMut for Ctx<'_, '_> {
    fn deref_mut(&mut self) -> &mut Pawn {
        self.pawn
    }
}

// ============================================================================ the host seam
// Not all of these have a caller in the stage-1 states: they are the seam for the states to come.
#[allow(dead_code)]
impl Ctx<'_, '_> {
    /// Starts a sound: `Audio::play` + the event. The handle stops it later.
    pub fn play_sound(&mut self, play: SoundPlay) -> SoundHandle {
        self.pawn.next_sound += 1;
        let handle = SoundHandle(self.pawn.next_sound);
        self.host.audio.play(handle, &play);
        self.pawn.event(EventKind::SoundPlay { handle, play });
        handle
    }

    /// (audio implementer) `SetVoiceGroupNo` (the anim interface's slot +0x1F0 = 0x1405A1D50): a
    /// state calls it after it started an animation. Tells the host only when the number changes.
    pub fn set_voice_group_no(&mut self, number: i32) {
        if self.pawn.voice_group_no != number {
            self.pawn.voice_group_no = number;
            self.host.audio.voice_group_no(number);
            self.pawn.event(EventKind::VoiceGroupNo { number });
        }
    }

    pub fn stop_sound(&mut self, handle: SoundHandle, fade_out: f32) {
        self.host.audio.stop(handle, fade_out);
        self.pawn.event(EventKind::SoundStop { handle, fade_out });
    }

    /// Spawns an effect: `Effects::spawn` + the event.
    pub fn spawn_effect(&mut self, spawn: EffectSpawn) -> EffectHandle {
        self.pawn.next_effect += 1;
        let handle = EffectHandle(self.pawn.next_effect);
        self.host.effects.spawn(handle, &spawn);
        self.pawn.event(EventKind::EffectSpawn { handle, spawn });
        handle
    }

    pub fn stop_effect(&mut self, handle: EffectHandle, immediate: bool) {
        self.host.effects.stop(handle, immediate);
        self.pawn.event(EventKind::EffectStop { handle, immediate });
    }

    /// Delivers a hit: `Targets::apply_hit` + the event with the target's answer.
    pub fn deliver_hit(&mut self, record: &HitRecord) -> HitResult {
        let result = self.host.targets.apply_hit(record);
        self.pawn.event(EventKind::Hit { record: *record, result });
        // (boss finish) The kill of a target the host calls finish-eligible (docs/sora-bossfinish-spec.md 2).
        if result.killed {
            let kind = self.host.targets.finish_kind(record.target);
            if kind != crate::host::FinishKind::None {
                self.pawn.event(EventKind::BossFinish { target: record.target, kind });
            }
        }
        result
    }

    /// The weapon is shown or hidden (`TresAnimNotify_SetWeaponVisible`; later the stance
    /// logic): an event only, the host draws the weapon.
    pub fn set_weapon_visible(&mut self, visible: bool) {
        self.pawn.event(EventKind::WeaponVisible { visible, instant: false });
    }

    /// Raises a popup: it is in this step's `UiSnapshot` and event list.
    pub fn popup(&mut self, popup: UiPopup) {
        self.pawn.popups.push(popup);
        self.pawn.event(EventKind::Popup(popup));
    }

    /// The host's candidate targets, appended to `out`.
    pub fn targets(&self, out: &mut Vec<TargetInfo>) {
        self.host.targets.enumerate(out);
    }

    pub fn target(&self, id: TargetId) -> Option<TargetInfo> {
        self.host.targets.info(id)
    }

    pub fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        self.host.targets.hurt_shapes(id, out);
    }
}
