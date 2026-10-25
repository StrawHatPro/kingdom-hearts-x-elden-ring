//! (ER boss) SDK EXTENSION: fields of Elden Ring 2.7.1.0 that the pinned SDK (fromsoftware-rs 79266c9) leaves unnamed
//! or private, found in the exe with Ghidra (build/sora_tmp/er/party/IMPL_BOSS_STATE.md has the handlers, addresses and
//! how each was read). Every accessor checks its pointers and returns None / false instead of guessing. Game thread only.
//!
//! - THE AI SWITCH (EMEVD 2004[1] SetCharacterAIState): `ChrIns.chr_ctrl -> ChrCtrl + 0xE8`, bit 5 (0x20): 1 = the
//!   character's AI is on, 0 = off. The SDK has this byte as the private `ChrCtrl::unke8` (the byte before the public
//!   `disable_move` at 0xE9). Handler: the bank-2004 dispatcher FUN_140569290, case 1 -> FUN_1405e1a00, which finds the
//!   character by entity id (FUN_1405eefd0) and does `ctrl[0xE8] = (ctrl[0xE8] & 0xDF) | (state & 1) << 5`; the same
//!   write is the leaf function at 0x1403ee260 (`ChrIns` in rcx, the state in dl). Nothing else is written.
//! - THE UPDATE RATE (EMEVD 2004[34] SetNetworkUpdateRate, which every boss event runs with the AI switch at its fight
//!   start): FUN_1405e1b00 -> 0x1403ea300 writes `ChrIns + 0xB8 = -2`, then 0x1403ee7b0 writes `ChrIns + 0x1C4` bit 0 =
//!   fixed and `ChrIns + 0xB4` = the frequency (0 = every frame). The SDK names these `omission_mode_override`,
//!   `chr_flags1c4.skip_omission_mode_updates` and `omission_mode`; they are read and written here as plain words so a
//!   value the SDK's enum does not list is never read as that enum.

use eldenring::cs::{ChrCtrl, ChrIns};

use crate::game::plausible;
use crate::word;

/// `ChrCtrl` + this = the byte whose bit 5 is the AI switch (SDK: private `unke8`).
pub const CHR_CTRL_AI_BYTE: usize = 0xE8;
/// The AI switch's bit in that byte.
pub const AI_ON_BIT: u8 = 1 << 5;
/// `ChrIns` offsets the update-rate instruction writes (checked against the SDK's own field offsets below).
pub const CHR_OMISSION_MODE: usize = 0xB4;
pub const CHR_OMISSION_OVERRIDE: usize = 0xB8;
pub const CHR_FLAGS_1C4: usize = 0x1C4;

// The SDK's layout must still put its named neighbours where the exe has them (a newer SDK pin fails here, not in game).
const _: () = assert!(std::mem::offset_of!(ChrCtrl, disable_move) == CHR_CTRL_AI_BYTE + 1);
const _: () = assert!(std::mem::offset_of!(ChrIns, omission_mode) == CHR_OMISSION_MODE);
const _: () = assert!(std::mem::offset_of!(ChrIns, omission_mode_override) == CHR_OMISSION_OVERRIDE);
const _: () = assert!(std::mem::offset_of!(ChrIns, chr_flags1c4) == CHR_FLAGS_1C4);

/// The address of a character's AI byte: its `ChrCtrl` must be plausible and own this very character.
fn ai_byte(chr: &ChrIns) -> Option<usize> {
    let ctrl = word!(chr.chr_ctrl);
    if !plausible(ctrl) {
        return None;
    }
    // SAFETY: a plausible pointer held by a live character; `owner` is the SDK's public field at 0x10.
    let owner = unsafe { (*(ctrl as *const ChrCtrl)).owner.as_ptr() as usize };
    (owner == chr as *const ChrIns as usize).then_some(ctrl + CHR_CTRL_AI_BYTE)
}

/// The raw AI byte (all eight bits, for the log).
pub fn ai_byte_value(chr: &ChrIns) -> Option<u8> {
    let at = ai_byte(chr)?;
    // SAFETY: inside the live ChrCtrl (0xE8 < its size, the SDK's struct is 0x3D0 bytes).
    Some(unsafe { std::ptr::read_volatile(at as *const u8) })
}

/// The AI switch as SetCharacterAIState left it (None: the character's ChrCtrl is not readable).
pub fn ai_on(chr: &ChrIns) -> Option<bool> {
    ai_byte_value(chr).map(|b| b & AI_ON_BIT != 0)
}

/// SetCharacterAIState(chr, on), written as its handler writes it (only bit 5 changes). False: not written.
pub fn set_ai_on(chr: &mut ChrIns, on: bool) -> bool {
    let Some(at) = ai_byte(chr) else { return false };
    // SAFETY: as `ai_byte_value`; one byte of the game's own object, written on the game thread as the event does.
    unsafe {
        let b = std::ptr::read_volatile(at as *const u8);
        std::ptr::write_volatile(at as *mut u8, with_ai_bit(b, on));
    }
    true
}

/// The byte after SetCharacterAIState's write: 0x1405e1a00 does `(b & 0xDF) | (state & 1) << 5`.
pub const fn with_ai_bit(b: u8, on: bool) -> u8 {
    (b & !AI_ON_BIT) | ((on as u8) << 5)
}

/// The update-rate words (mode at 0xB4, override at 0xB8, the fixed bit 0 of 0x1C4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateRate {
    pub mode: i32,
    pub override_: i32,
    pub fixed: bool,
}

/// What a boss's fight start writes (2004[34] with fixed = 1, frequency 0): every frame, whatever the distance.
pub const EVENT_FIGHT_RATE: UpdateRate = UpdateRate { mode: 0, override_: -2, fixed: true };

pub fn update_rate(chr: &ChrIns) -> UpdateRate {
    let base = chr as *const ChrIns as usize;
    // SAFETY: three words inside the live character (offsets checked against the SDK at compile time).
    unsafe {
        UpdateRate {
            mode: std::ptr::read_volatile((base + CHR_OMISSION_MODE) as *const i32),
            override_: std::ptr::read_volatile((base + CHR_OMISSION_OVERRIDE) as *const i32),
            fixed: std::ptr::read_volatile((base + CHR_FLAGS_1C4) as *const u8) & 1 != 0,
        }
    }
}

/// SetNetworkUpdateRate's writes (in its order: the override, then the fixed bit and the mode).
pub fn set_update_rate(chr: &mut ChrIns, r: UpdateRate) {
    let base = chr as *mut ChrIns as usize;
    // SAFETY: as `update_rate`; the same words the game's handler writes, on the game thread.
    unsafe {
        std::ptr::write_volatile((base + CHR_OMISSION_OVERRIDE) as *mut i32, r.override_);
        let f = std::ptr::read_volatile((base + CHR_FLAGS_1C4) as *const u8);
        std::ptr::write_volatile((base + CHR_FLAGS_1C4) as *mut u8, (f & !1) | r.fixed as u8);
        std::ptr::write_volatile((base + CHR_OMISSION_MODE) as *mut i32, r.mode);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_is_bit_5_and_nothing_else_changes() {
        for b in 0..=255u8 {
            // the handler's own arithmetic (and dl,1; shl dl,5; and [x],0DFh; or [x],dl)
            assert_eq!(with_ai_bit(b, true), (b & 0xDF) | (1 << 5));
            assert_eq!(with_ai_bit(b, false), b & 0xDF);
            assert_eq!(with_ai_bit(b, true) & !0x20, b & !0x20, "no other bit moves");
        }
        assert_eq!(EVENT_FIGHT_RATE, UpdateRate { mode: 0, override_: -2, fixed: true });
    }
}
