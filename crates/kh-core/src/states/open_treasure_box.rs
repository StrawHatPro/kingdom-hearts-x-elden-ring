//! `TresPlayerState_OpenTreasureBox` (id 74, `TSID_STATE_OPEN_TREASURE_BOX`): KH3's treasure-box
//! opening. OWNER: IMPL_ER58 (chest + ladder). Read from the decompiled C of
//! `UTresCharState_OpenTreasureBox` (decompiled C only; the vfuncs sheet gives the slots):
//! Reset slot 95 0x14150DFA0, Begin slot 96 0x141509800, Tick slot 97 0x14150F940, End slot 98
//! 0x14150BD10, the turn toward the box slot 115 0x141513C70, the large box's placement slot 116
//! 0x141507C50, the player's "on the ground" test slot 117 0x14138FBC0.
//!
//! - SMALL BOX (state +0x90 bit 0 clear), Begin: on the ground -> `Draw(1, 1, 1.0)` (pawn vt+0x1340:
//!   the Keyblade); sub-step 0. Tick 0: `StopMovement` (pawn vt+0x16E8 = 0x1405A5DC0), on the ground
//!   -> `Play("OpenTreasureS", 0, -1.0, 2)` (0.97 s; the Keyblade's `HIT7` weapon hit at 0.468 s),
//!   +0x94 = 0.5 s: the box is told to open when it runs out (bit 8 -> +0x3E0 delegate); sub-step 1.
//!   In the air: the idle clip, +0x98 = 0.5 s, sub-step 2. Tick 1: the turn toward the box, the
//!   0.5 s count; the clip's end -> the idle clip, `m_bEnableChange`, sub-step 2. Tick 2: the
//!   counts; with `m_bEnableChange` -> sub-step 8 (7 when +0x90 bit 1) -> the default: `DoFinish`.
//! - LARGE BOX (+0x90 bit 0, `ATresTreasureBox::m_bIsLargeBox`), Begin: the HUD / input locks of the
//!   event (not here), sub-step 3. Tick 3: `StopMovement`, `Play("OpenTreasureL", 0, -1.0, 2)`
//!   (2.97 s: a voice at 0.358 s, `se00001_023` at 0.485 s, `TresTiming` at 1.732 s, the Keyblade's
//!   `HIT7` at 1.745 s), +0x94 = 2.0 s; sub-step 4. Tick 4: the placement at the box's opener point and
//!   the camera animation `m_PlayerOpenTreasureBoxCameraAnim` (not here: the host has no box and no
//!   camera actor), `Draw(1, 1, 1.0)`; sub-step 5, falling through. Tick 5: the clip's end -> the idle
//!   clip, +0x94 = 0, bits 8 | 0x20; sub-step 6. Tick 6: the 2.0 s count (bit 8: the box opens); bits
//!   0x10 and 0x20 -> sub-step 7. Tick 7: the box has finished opening (vt+0x9E0; here at once) ->
//!   sub-step 8 -> `DoFinish`.
//!
//! THE PORT (a host's box): the request names the size; the box itself is the host's (Elden Ring's
//! chest opens by its own action). `TreasureView::opened` reports the moment the game tells its box
//! to open (0.5 s / 2.0 s in). NOT HERE: the box delegate, the HUD / input locks of Begin and End,
//! the camera animation, the turn toward the box and the placement (the host faces him to its box).

use super::common::on_ground;
use super::{PlayerState, StateData, StateId};
use crate::actions;
use crate::anim::{AnimId, PlayArgs};
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

/// Seconds after the play call at which the game tells a small / large box to open (+0x94).
pub const OPEN_AT_SMALL: f32 = 0.5;
pub const OPEN_AT_LARGE: f32 = 2.0;
/// +0x98 in the airborne path: `m_bEnableChange` after this long.
const CHANGE_AT: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OpenTreasureBox {
    sub: u8,
    /// +0x90 bit 0: the box is a large one.
    large: bool,
    /// +0x94: the count to the box's opening; +0x98: the count to `m_bEnableChange` (airborne path).
    open_in: f32,
    change_in: f32,
    /// +0x90 bit 8 (the box may open) and bit 0x10 (it was told); bit 0x20 (the clip is over, large box).
    may_open: bool,
    told: bool,
    clip_over: bool,
}

impl OpenTreasureBox {
    /// Reset (0x14150DFA0): everything zero.
    pub const RESET: OpenTreasureBox = OpenTreasureBox { sub: 0, large: false, open_in: 0.0, change_in: 0.0, may_open: false, told: false, clip_over: false };
}

/// What a host sees of the running state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreasureView {
    pub large: bool,
    pub sub_step: u8,
    /// The game has told its box to open.
    pub opened: bool,
}

pub(crate) fn view(state: &StateData) -> Option<TreasureView> {
    match state {
        StateData::OpenTreasureBox(t) => Some(TreasureView { large: t.large, sub_step: t.sub, opened: t.told }),
        _ => None,
    }
}

/// The host's request (the game's is the box's `Open` reaction: `SetState(74)` with the box in
/// `m_pTreasureBox` and the size in +0x90 bit 0): the state replaces the running one at the next
/// `MainProc`.
pub(crate) fn request(s: &mut Pawn, large: bool) {
    s.queue.set_state(OpenTreasureBox { large, ..OpenTreasureBox::RESET });
}

impl OpenTreasureBox {
    /// The idle clip the state plays after the opening (pawn vt+0x1C68: the idle name of the
    /// stance; `IdleN` with the weapon away or without the battle pose).
    fn play_idle(s: &mut Pawn) {
        let name = actions::idle_name(s.actions.weapon_out, actions::battle_anim(s), s.combat.lockon.locked);
        s.anim.play_with(name, PlayArgs::mode(3, -1.0));
    }

    /// The 0.5 s / 2.0 s count and the box told once (bits 8, 0x10).
    fn count(&mut self, dt: f32) {
        if self.open_in > 0.0 {
            self.open_in -= dt;
            if self.open_in <= 0.0 {
                self.may_open = true;
            }
        }
        if self.may_open && !self.told {
            self.told = true;
        }
    }
}

impl PlayerState for OpenTreasureBox {
    const ID: StateId = StateId::OpenTreasureBox;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin (0x141509800).
    fn begin(&mut self, cx: &mut Ctx) {
        if self.large {
            self.sub = 3;
            return;
        }
        self.sub = 0;
        if on_ground(cx.pawn) {
            actions::draw(cx, true);
        }
    }

    /// Tick (0x14150F940).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        match self.sub {
            0 => {
                let s = &mut *cx.pawn;
                s.cmc.stop_movement();
                if on_ground(s) {
                    s.anim.play_with(AnimId::OpenTreasureS, PlayArgs::mode(2, -1.0));
                    self.open_in = OPEN_AT_SMALL;
                    self.sub = 1;
                } else {
                    Self::play_idle(s);
                    self.change_in = CHANGE_AT;
                    self.sub = 2;
                }
            }
            1 => {
                self.count(dt);
                if cx.pawn.anim.ended {
                    Self::play_idle(cx.pawn);
                    cx.pawn.queue.flags.enable_change = true;
                    self.may_open = true;
                    self.count(0.0);
                    self.sub = 2;
                }
            }
            2 => {
                self.count(dt);
                if self.change_in > 0.0 {
                    self.change_in -= dt;
                    if self.change_in <= 0.0 {
                        cx.pawn.queue.flags.enable_change = true;
                    }
                }
                if cx.pawn.queue.flags.enable_change {
                    self.may_open = true;
                    self.count(0.0);
                    self.sub = 8;
                }
            }
            3 => {
                let s = &mut *cx.pawn;
                s.cmc.stop_movement();
                s.anim.play_with(AnimId::OpenTreasureL, PlayArgs::mode(2, -1.0));
                self.open_in = OPEN_AT_LARGE;
                self.sub = 4;
            }
            4 | 5 | 6 => {
                if self.sub == 4 {
                    // (the placement and the camera animation are the host's) Draw(1, 1, 1.0)
                    actions::draw(cx, true);
                    self.sub = 5;
                }
                if self.sub == 5 && cx.pawn.anim.ended {
                    Self::play_idle(cx.pawn);
                    self.open_in = 0.0;
                    self.may_open = true;
                    self.clip_over = true;
                    self.sub = 6;
                }
                self.count(dt);
                if self.told && self.clip_over {
                    self.sub = 7;
                }
            }
            7 => self.sub = 8,
            _ => cx.pawn.queue.do_finish(),
        }
    }

    /// End (0x14150BD10): the event's locks and the camera animation are released (not here).
    fn end(&mut self, _cx: &mut Ctx) {}

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

#[cfg(test)]
mod tests {
    use crate::test_world::TestWorld;
    use crate::{AnimId, Input, Pose, Sora, StateId, Tuning, Vec3, SORA_PARAMS};

    const DT: f32 = 1.0 / 60.0;

    fn standing() -> (Sora, TestWorld) {
        let world = TestWorld::floor();
        let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT);
        // (the placeholder loadout has no HP: alive is all the request asks)
        sora.pawn.status.hp = 100;
        for _ in 0..30 {
            sora.step(DT, &Input::default(), &world);
        }
        (sora, world)
    }

    /// A large box: `OpenTreasureL` from the next step, the Keyblade drawn, the box told at 2.0 s,
    /// the state over after the clip (2.97 s), back to Idle.
    #[test]
    fn a_large_box_plays_open_treasure_l_and_ends_in_idle() {
        let (mut sora, world) = standing();
        assert!(sora.open_treasure_box(true));
        let mut opened_at = None;
        let mut over_at = None;
        let mut clip = None;
        for k in 1..=300 {
            let out = sora.step(DT, &Input::default(), &world);
            if k == 3 {
                assert_eq!(out.state, StateId::OpenTreasureBox);
                clip = Some(out.anim.name);
            }
            if opened_at.is_none() && sora.treasure_box().is_some_and(|t| t.opened) {
                opened_at = Some(k as f32 * DT);
            }
            if over_at.is_none() && out.state == StateId::Idle && k > 3 {
                over_at = Some(k as f32 * DT);
            }
        }
        assert_eq!(clip, Some(AnimId::OpenTreasureL));
        assert!(sora.actions().weapon_out, "the Keyblade is drawn (Draw(1) in tick 4)");
        let opened = opened_at.expect("the box is told to open");
        assert!((opened - 2.0).abs() < 0.1, "{opened}");
        let over = over_at.expect("the state ends");
        assert!(over > 2.9 && over < 3.2, "{over}");
    }

    /// A small box: `OpenTreasureS` (0.97 s), the box told at 0.5 s, the state over at the clip's end.
    #[test]
    fn a_small_box_plays_open_treasure_s() {
        let (mut sora, world) = standing();
        assert!(sora.open_treasure_box(false));
        let mut seen = false;
        let mut opened_at = None;
        let mut over_at = None;
        for k in 1..=120 {
            let out = sora.step(DT, &Input::default(), &world);
            seen |= out.anim.name == AnimId::OpenTreasureS;
            if opened_at.is_none() && sora.treasure_box().is_some_and(|t| t.opened) {
                opened_at = Some(k as f32 * DT);
            }
            if seen && over_at.is_none() && out.state == StateId::Idle {
                over_at = Some(k as f32 * DT);
            }
        }
        assert!(seen);
        let opened = opened_at.expect("told");
        assert!((opened - 0.5).abs() < 0.1, "{opened}");
        let over = over_at.expect("over");
        assert!(over > 0.9 && over < 1.2, "{over}");
    }
}
