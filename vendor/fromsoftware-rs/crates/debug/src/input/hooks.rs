use std::sync::atomic::Ordering;

use crate::input::{INPUT_BLOCKER, InputFlags, dinput};

pub use ilhook::HookError;

pub(super) unsafe fn install() -> Result<(), HookError> {
    unsafe { dinput::install()? };
    Ok(())
}

/// Returns true if any of `flags` are currently set in the input blocker
pub(crate) fn is_blocked(flags: InputFlags) -> bool {
    INPUT_BLOCKER.get().is_some_and(|b| {
        InputFlags::from_bits_retain(b.flags.load(Ordering::Relaxed)).intersects(flags)
    })
}
