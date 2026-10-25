use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use bitflags::bitflags;

mod dinput;
pub mod hooks;

pub use hooks::HookError;
use hudhook::imgui::Io;

pub(crate) static INPUT_BLOCKER: OnceLock<&'static InputBlocker> = OnceLock::new();

#[derive(Default)]
pub struct InputBlocker {
    flags: AtomicU8,
    hooks_installed: AtomicBool,
}

impl InputBlocker {
    pub const fn new() -> Self {
        Self {
            flags: AtomicU8::new(0),
            hooks_installed: AtomicBool::new(false),
        }
    }

    pub fn get_instance() -> &'static InputBlocker {
        INPUT_BLOCKER.get_or_init(|| {
            static INSTANCE: InputBlocker = InputBlocker::new();
            &INSTANCE
        })
    }

    /// Receives the context from the pre-reload DLL
    pub fn forward_instance(instance: &'static InputBlocker) {
        if INPUT_BLOCKER.set(instance).is_ok() {
            instance.hooks_installed.store(true, Ordering::Relaxed);
        }
    }

    /// # Safety
    ///
    /// Subject to all standard hooking caveats, must be called from the single threaded context, etc...
    pub unsafe fn install_hooks(&self) -> Result<(), HookError> {
        if self.hooks_installed.swap(true, Ordering::Relaxed) {
            return Ok(());
        }
        unsafe { hooks::install() }
    }

    pub fn block(&self, inputs: InputFlags) {
        self.flags.fetch_or(inputs.bits(), Ordering::Relaxed);
    }

    pub fn block_only(&self, inputs: InputFlags) {
        self.flags.store(inputs.bits(), Ordering::Relaxed);
    }

    pub fn block_from_io(&self, io: &Io) {
        let mut flag = InputFlags::empty();
        if io.want_capture_mouse {
            flag |= InputFlags::Mouse;
        }
        if io.want_capture_keyboard {
            flag |= InputFlags::Keyboard;
        }
        if io.want_capture_mouse && io.want_capture_keyboard {
            flag |= InputFlags::GamePad;
        }
        self.block_only(flag);
    }

    pub fn unblock(&self, inputs: InputFlags) {
        self.flags
            .fetch_and(inputs.complement().bits(), Ordering::Relaxed);
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct InputFlags: u8 {
        const GamePad  = 0b001;
        const Keyboard = 0b010;
        const Mouse    = 0b100;
    }
}
