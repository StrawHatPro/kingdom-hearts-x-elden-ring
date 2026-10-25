//! The state-machine host: the `PlayerState` trait every state implements, the state queue
//! (`TresStateQueueComponent`) and its `MainProc` (0x14150A7E0), as docs/sora-movement-spec.md
//! revision 2 section 1.3 gives them.
//!
//! ```text
//! MainProc (queue flags, component +0x268: 1 finish, 2 restart, 4 loop again, 8 busy):
//!   do:
//!     flags &= ~4
//!     if NeedChange():            // (flags & 3) || !cur || queue empty || cur != queue[0]
//!         if cur: cur.End()
//!         flags &= ~1
//!         if queue empty: PushState(default = IDLE)
//!         if flags & 2: flags &= ~2; cur = queue[0]
//!         else: recycle(old); if old == queue[0]: pop front (push default if now empty); cur = queue[0]
//!         if cur: cur.Begin()
//!     if cur && (flags & 7) == 0: cur.Tick(dt)     // a state begun in this call is ticked in this call
//!   while flags & 4
//! ```
//!
//! In this port the running state is held OUTSIDE the queue (`Sora::state`), so that its
//! `&mut self` and the `&mut Ctx` (which contains the queue) never alias; `Queue` holds the
//! instances behind it and remembers whether the running one is still the game's `queue[0]`.
//! The busy flag (8) guards the game against re-entry and has no counterpart here.
//!
//! State object fields that every state has (state +0x78, +0x84):
//! - `m_nSubStep`: each state struct keeps its own `sub` field and reports it through
//!   `PlayerState::sub_step` (the base Reset zeroes it: a fresh instance starts at its `RESET`).
//! - `m_bEnableChange` (+0x84 bit 0): `Queue::flags.enable_change`: set by the
//!   `TresEnableChange` notify, cleared when a new instance becomes the running state (the base
//!   Reset that `PushState` calls).
//! - `m_bReverseLeg` (+0x84 bit 1): `Pawn::anim.reverse_leg`: the `TresReverseLeg` notify state
//!   was in the latest pose tick's window.

use crate::anim::Notifies;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;
use crate::states::{StateData, StateId};

/// One `TresPlayerState_*` class. The struct is the state object's fields (plain data, `Copy`:
/// it is part of Sora's save state); a `RESET` constant is the object after the game's `Reset`.
pub(crate) trait PlayerState: Copy {
    /// The state's `ETresStateID`.
    const ID: StateId;

    /// `m_nSubStep` (state +0x78), for `Output::sub_step`.
    fn sub_step(&self) -> u8;

    /// `Begin`: called by `MainProc` when the instance becomes the running state.
    fn begin(&mut self, cx: &mut Ctx);

    /// `Tick(dt)`: once per `MainProc` pass while it runs, also in the pass that began it.
    fn tick(&mut self, cx: &mut Ctx, dt: f32);

    /// `End`: called by `MainProc` before the next state begins.
    fn end(&mut self, cx: &mut Ctx);

    /// The notifies this frame's pose tick fired (in M(N); the state's tick follows in S(N)).
    /// `m_bEnableChange` has already been set from `notifies.enable_change`.
    fn on_notifies(&mut self, _cx: &mut Ctx, _notifies: &Notifies) {}

    /// The readable summary for traces and debug text (`Output::movement_state`).
    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// The flag byte of the running state (state +0x84).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct StateFlags {
    /// Bit 0, `m_bEnableChange`: the animation has passed its `TresEnableChange` notify: the
    /// state may be left for a new action.
    pub enable_change: bool,
}

/// `TresStateQueueComponent` without the running state: the instances queued behind it and the
/// queue flags. The methods are the game's own operations.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Queue {
    /// The game's queue minus its front while `front_is_running`, the whole queue otherwise.
    queued: Vec<StateData>,
    /// The running state is still the game's `queue[0]` (false once the queue was cleared
    /// under it: `NeedChange` then holds).
    front_is_running: bool,
    /// Flag 1.
    finish: bool,
    /// Flag 2.
    restart: bool,
    /// Flag 4.
    chain: bool,
    /// The running state's flag byte.
    pub flags: StateFlags,
}

/// More `DoChain`s than this in one `MainProc` call is a state loop: the call stops and says so.
const MAX_PASSES: usize = 8;

// `push_state`, `do_finish` and `do_restart` have no caller among the stage-1 states.
#[allow(dead_code)]
impl Queue {
    pub fn new() -> Queue {
        Queue { queued: Vec::new(), front_is_running: true, finish: false, restart: false, chain: false, flags: StateFlags::default() }
    }

    /// `PushState` (0x141513260): appends an instance (the caller built it from the state's
    /// `RESET` and filled in what the game's request function fills in). It begins when
    /// everything before it has finished.
    pub fn push_state(&mut self, state: impl Into<StateData>) {
        self.queued.push(state.into());
    }

    /// `SetState` (0x141514920): clear the queue, then `PushState`: the running state is
    /// replaced at the next `MainProc`. A state queued earlier in the same frame is lost.
    pub fn set_state(&mut self, state: impl Into<StateData>) {
        self.clear();
        self.push_state(state);
    }

    /// `ClearQueue`: drops everything queued, the running state's own entry included (it ends at
    /// the next `MainProc`; with nothing pushed afterwards the default state begins).
    pub fn clear(&mut self) {
        self.queued.clear();
        self.front_is_running = false;
    }

    /// `DropQueued` (0x1405A7FF0): removes what is queued BEHIND the running state; the running
    /// state keeps its entry. (Reason for the addition: actions spec II.1, the requests that drop
    /// a queued attack, and the Guard state's queued jump.)
    pub fn drop_queued(&mut self) {
        if self.front_is_running {
            self.queued.clear();
        }
    }

    /// The id of the next queued state (`FUN_1405AB6E0`), if any.
    pub fn queued_id(&self) -> Option<StateId> {
        self.queued.first().map(|s| s.id())
    }

    /// "Nothing queued": the running state is the queue's only entry.
    pub fn nothing_queued(&self) -> bool {
        self.front_is_running && self.queued.is_empty()
    }

    /// `DoFinish` (flags |= 1): the running state ends at the next `MainProc`.
    pub fn do_finish(&mut self) {
        self.finish = true;
    }

    /// `DoChain` (flags |= 5): end the running state and begin and tick the next one in this
    /// `MainProc` call.
    pub fn do_chain(&mut self) {
        self.finish = true;
        self.chain = true;
    }

    /// Flag 2: end the running state and begin it again (the same instance, no Reset) when it is
    /// still the queue's front.
    pub fn do_restart(&mut self) {
        self.restart = true;
    }

    /// `NeedChange` (0x14150E830).
    fn need_change(&self) -> bool {
        self.finish || self.restart || !self.front_is_running
    }

    /// The change itself, after the old state's `End`: which instance runs next, and whether it
    /// is a new one (a Reset instance whose flags start clear).
    fn next(&mut self, old: StateData) -> (StateData, bool) {
        self.finish = false;
        let (next, fresh) = if std::mem::take(&mut self.restart) && self.front_is_running {
            // cur = queue[0]: the same instance again.
            (old, false)
        } else if self.queued.is_empty() {
            // The old one was the only entry (popped) or the queue was cleared: the default.
            (StateData::DEFAULT, true)
        } else {
            (self.queued.remove(0), true)
        };
        self.front_is_running = true;
        (next, fresh)
    }
}

/// `MainProc` (0x14150A7E0), spec 1.3: the last thing in a frame.
pub(crate) fn main_proc(state: &mut StateData, s: &mut Ctx, dt: f32) {
    // PORT OPTION `Tuning::speed_ramp`: only a tick of the run loop sets the ramped limit.
    s.cmc.speed_limit_override = None;
    for pass in 0.. {
        s.queue.chain = false;
        if s.queue.need_change() {
            let from = state.id();
            state.end(s);
            let (next, fresh) = s.queue.next(*state);
            *state = next;
            if fresh {
                // PushState calls Reset: the base Reset clears m_bEnableChange.
                s.queue.flags = StateFlags::default();
            }
            s.state_changed(from, state.id());
            // (audio implementer) The voice group number: "most sites pass 0" after they start an
            // animation (audio spec 5.2); the states with a number of their own (attack kinds,
            // magic, items, death) set it in their Begin / at their voice notify. [inf: one
            // reset per state begin stands for the per-site SetVoiceGroupNo(0) calls.]
            s.set_voice_group_no(0);
            state.begin(s);
        }
        // (flags & 7) == 0: a Begin that finished, restarted or chained its own state is not ticked.
        if !(s.queue.finish || s.queue.restart || s.queue.chain) {
            state.tick(s, dt);
        }
        if !s.queue.chain {
            break;
        }
        if pass + 1 >= MAX_PASSES {
            s.note("the state queue chained more than 8 states in one MainProc call: stopped for this frame (a state loop)");
            break;
        }
    }
}

/// The notifies of this frame's pose tick, routed to the running state (fired in M(N), seen by
/// S(N): spec 1.2).
pub(crate) fn on_notifies(state: &mut StateData, s: &mut Ctx, notifies: &Notifies) {
    // TresAnimNotify_EnableChange -> m_bEnableChange = 1.
    if notifies.enable_change {
        s.queue.flags.enable_change = true;
    }
    state.on_notifies(s, notifies);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::states::idle::Idle;
    use crate::states::run::Run;

    #[test]
    fn the_queue_operations_of_spec_1_3() {
        let idle = StateData::DEFAULT;
        let run = StateData::Run(Run::RESET);
        let with_sub = |sub: u8| {
            let mut state = Run::RESET;
            state.sub = sub;
            state
        };
        let mut q = Queue::new();
        assert!(!q.need_change() && q.nothing_queued());
        // PushState appends behind the running state: no change until that one finishes.
        q.push_state(Run::RESET);
        assert!(!q.need_change() && !q.nothing_queued());
        q.do_finish();
        assert!(q.need_change());
        assert_eq!(q.next(idle), (run, true));
        assert!(!q.need_change() && q.nothing_queued());
        // SetState clears what was queued and replaces the running state at the next MainProc.
        q.push_state(Idle::RESET);
        q.set_state(with_sub(1));
        assert!(q.need_change());
        assert_eq!(q.next(run), (StateData::Run(with_sub(1)), true));
        // DoFinish with nothing queued: the default state.
        q.do_finish();
        assert_eq!(q.next(run), (StateData::DEFAULT, true));
        // ClearQueue alone ends the running state too: the default state.
        q.clear();
        assert!(q.need_change());
        assert_eq!(q.next(run), (StateData::DEFAULT, true));
        // DoChain is finish + loop again.
        q.do_chain();
        assert!(q.finish && q.chain && q.need_change());
        q.next(idle);
        // Flag 2: the same instance begins again, as it is (no Reset).
        let stopping = StateData::Run(with_sub(4));
        q.do_restart();
        assert!(q.need_change());
        assert_eq!(q.next(stopping), (stopping, false));
        assert!(!q.need_change());
        // ... unless the queue was replaced under it: then the new front begins.
        q.do_restart();
        q.set_state(Idle::RESET);
        assert_eq!(q.next(stopping), (StateData::DEFAULT, true));
    }
}
