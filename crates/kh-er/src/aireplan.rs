//! (REVENGE, 2026-11-06) A BOSS'S AI ASKED TO PLAN ITS NEXT ACTION NOW: what FromSoftware's own Lua AI does with
//! `ai:Replaning()`, written the way that binding writes it. The boss's battle goal then chooses its next act from its
//! own moveset and its own weights; no attack id is forced (bossrevenge.rs rule 3).
//!
//! READ FROM eldenring.exe 2.7.1.0 (a copy, Ghidra project build/sora_tmp/er/boss/ghidra/erboss) and checked byte for
//! byte in 2.7.2.0 (build/sora_tmp/er/port272/eldenring_2720.exe); build/sora_tmp/revenge/STATE.md section 2:
//! - The Lua binding "Replaning" (registered at 0x14008df40: name 0x142a0e1e0, function 0x140302b10) is
//!   `mov rcx,[rcx+8]; add rcx,0xDEE8; jmp 0x1402d68e0`, and 0x1402d68e0 is `mov byte [rcx+0x11],1; ret`: the AI think
//!   object + 0xDEE8 is its replanning controller, + 0x11 of it (think + 0xDEF9) the request.
//! - The same think object: `ChrIns` vfunc +0x1D0 (EnemyIns: 0x1404d04a0 / 2.7.2.0 0x1404d0580 = its `com_manipulator`
//!   at +0x580 when that is a ComManipulator, type 5) + 0xC0 (0x1402ca700; EMEVD 2004[17] SetCharacterAICommand writes
//!   its event slots at + 0xC45C, which Lua's GetEventRequest reads through the same `ai` + 8).
//! - Who asks it besides Lua: the SpEffect apply 0x1403fb010 (a SpEffect with its replan flag); who acts on it: the think
//!   update 0x1402c9300 reads + 0x11 (0x1402d67f0) and calls 0x1402d6780, which (when 0x1402d69f0 allows: not forced off
//!   by its current goal) runs the impl's vfunc 0 = 0x1402c12e0: the planner's goal stack cleared and the top goal
//!   activated again, then the flag cleared.
//! - The think constructor 0x1402bda40 lays out: + 0xDED8 the impl (vtable `CSIAiReplanningCtrlImpl@CSAiThink@CS`),
//!   + 0xDEE0 the think, + 0xDEE8 the controller (+0 the think, +8 the impl): checked here before the one byte is written.
//!
//! Every read goes through probe.rs (`ReadProcessMemory`: a bad pointer is "not read", never a fault); the one write is
//! `probe::poke` (a masked write of that byte). Game thread only.

use eldenring::cs::{ChrIns, EnemyIns};
use fromsoftware_shared::Superclass;

use crate::probe;

/// `EnemyIns` + this = its `com_manipulator` (the SDK's field; asserted below).
pub const ENEMY_COM_MANIPULATOR: usize = 0x580;
/// The ComManipulator + this = its AI think object (0x1402ca700).
pub const COM_THINK: usize = 0xC0;
/// The think + these: the replanning impl (its vtable), the impl's think, the controller (its think, its impl).
pub const THINK_REPLAN_IMPL: usize = 0xDED8;
pub const THINK_REPLAN_CTRL: usize = 0xDEE8;
/// The controller + this = the request byte (Lua's `ai:Replaning()` writes 1).
pub const CTRL_REQUEST: usize = 0x11;

const _: () = assert!(std::mem::offset_of!(EnemyIns, com_manipulator) == ENEMY_COM_MANIPULATOR);

/// The RTTI names (probe::class_of: without `.?AV` and the closing `@@`).
const COM_CLASS: &str = "ComManipulator@CS";
const IMPL_CLASS: &str = "CSIAiReplanningCtrlImpl@CSAiThink@CS";

/// The layout checks of a think object, on the words read (pure: unit-tested).
pub fn layout_ok(think: usize, impl_back: Option<usize>, ctrl_think: Option<usize>, ctrl_impl: Option<usize>) -> bool {
    think != 0 && impl_back == Some(think) && ctrl_think == Some(think) && ctrl_impl == Some(think + THINK_REPLAN_IMPL)
}

/// The AI think object of a character (None: not an enemy driven by an AI, or something does not check out).
fn think_of(chr: &ChrIns) -> Result<usize, &'static str> {
    let enemy = chr.as_subclass::<EnemyIns>().ok_or("not an EnemyIns")?;
    let com = enemy.com_manipulator;
    if probe::class_of(com).as_deref() != Some(COM_CLASS) {
        return Err("its manipulator is not a ComManipulator (no AI of its own)");
    }
    let think = probe::word(com + COM_THINK).filter(|t| *t != 0).ok_or("no AI think object")?;
    if probe::class_of(think + THINK_REPLAN_IMPL).as_deref() != Some(IMPL_CLASS) {
        return Err("the think object's replanning part is not where 2.7.1.0 / 2.7.2.0 have it");
    }
    let (impl_back, ctrl_think, ctrl_impl) = (probe::word(think + THINK_REPLAN_IMPL + 8), probe::word(think + THINK_REPLAN_CTRL), probe::word(think + THINK_REPLAN_CTRL + 8));
    if !layout_ok(think, impl_back, ctrl_think, ctrl_impl) {
        return Err("the replanning controller does not point back at its think object");
    }
    Ok(think)
}

/// What an ask did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// The request is set: the think update replans at its next chance.
    Set,
    /// A request was already pending (the game's own, or an earlier one of ours).
    Pending,
}

/// ASK THE CHARACTER'S AI TO REPLAN (Lua's `ai:Replaning()`). Err: why nothing was written.
pub fn request(chr: &ChrIns) -> Result<Asked, &'static str> {
    let think = think_of(chr)?;
    let at = think + THINK_REPLAN_CTRL + CTRL_REQUEST;
    let mut now = [0u8; 1];
    if !probe::read(at, &mut now) {
        return Err("the request byte is not readable");
    }
    if now[0] != 0 {
        return Ok(Asked::Pending);
    }
    if probe::poke(at, 1, 0xFF) { Ok(Asked::Set) } else { Err("the request byte is not writable") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_must_point_back_at_its_think_object() {
        let t = 0x7FF0_0000_0000usize;
        assert!(layout_ok(t, Some(t), Some(t), Some(t + THINK_REPLAN_IMPL)));
        assert!(!layout_ok(t, Some(t + 8), Some(t), Some(t + THINK_REPLAN_IMPL)));
        assert!(!layout_ok(t, Some(t), None, Some(t + THINK_REPLAN_IMPL)));
        assert!(!layout_ok(t, Some(t), Some(t), Some(t)));
        assert!(!layout_ok(0, Some(0), Some(0), Some(THINK_REPLAN_IMPL)));
        // the request byte is the one Lua's binding writes: think + 0xDEE8 + 0x11
        assert_eq!(THINK_REPLAN_CTRL + CTRL_REQUEST, 0xDEF9);
    }
}
