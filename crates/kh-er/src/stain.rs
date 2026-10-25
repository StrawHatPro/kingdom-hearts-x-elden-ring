//! (IMPL_ER17) THE EMPTY BLOODSTAIN of Elden Ring's own death (the game-over menu's Retry), put out of the way.
//!
//! Sora's munny is never dropped (the rune counter is held at 0 while he dies: prize.rs), but the game still leaves its
//! marker where he died, with the prompt "Retrieve lost runes", which can take the Y press at a save point.
//!
//! WHAT IT IS (MEASURED, IMPL_ER17_STATE.md): `CSEventMan + 0x80` = `CSEventBloodStainCtrl`; `+0x08` -> its data:
//! `+0x30` -> the stain's record { `+0x00` block-local place x y z, `+0x0C` yaw, `+0x20` block id }, `+0x38` -> the
//! `FXHGSfxCtrl` of the marker's effect (`+0x18` -> its record in the effect system's pool, as graceflame.rs describes).
//! With the record's height at -5000 the prompt does not come up; with the effect's seven floats at 0 the marker is not drawn.
//! Both are put back with `bloodstain.keep = 1` and when the payload goes.
//!
//! Every read goes through `probe::read`; the writes are plain writes of words read an instant before in the same call.

use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::Mutex;

use crate::control::Control;
use crate::erlog;
use crate::probe::{class_of, floats, u32_at, word};

const CTRL_FROM_EVENT_MAN: usize = 0x80;
const DATA_RECORD: usize = 0x30;
const DATA_EFFECT: usize = 0x38;
const AWAY: f32 = -5000.0;

static VT_CTRL: AtomicUsize = AtomicUsize::new(0);
static VT_FX: AtomicUsize = AtomicUsize::new(0);

fn is(p: usize, known: &AtomicUsize, class: &str) -> bool {
    let Some(vt) = word(p) else { return false };
    let have = known.load(Relaxed);
    if have != 0 {
        return vt == have;
    }
    if class_of(p).is_some_and(|c| c == class) {
        known.store(vt, Relaxed);
        return true;
    }
    false
}

#[derive(Default)]
struct State {
    /// The stain record we moved and the height it had.
    moved: Option<(usize, f32)>,
    /// The marker's effect record, its id and the floats it had.
    hidden: Option<(usize, u32, [f32; 7])>,
    told: Option<bool>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// (the stain's record, the marker's effect control), when the game has a stain.
fn parts() -> Option<(usize, usize)> {
    use fromsoftware_shared::FromStatic;
    // SAFETY: the game's singleton; pointer reads through the system.
    let man = unsafe { eldenring::cs::CSEventManImp::instance() }.ok()? as *const _ as usize;
    let ctrl = word(man + CTRL_FROM_EVENT_MAN).filter(|c| *c != 0)?;
    if !is(ctrl, &VT_CTRL, "CSEventBloodStainCtrl@CS") {
        return None;
    }
    let data = word(ctrl + 8).filter(|d| *d != 0)?;
    let record = word(data + DATA_RECORD).filter(|r| *r != 0)?;
    // (a record of a real block, at a finite place)
    let block = u32_at(record + 0x20)?;
    let place = floats::<3>(record)?;
    if block == u32::MAX || block == 0 || !place.iter().all(|v| v.is_finite() && v.abs() < 1.0e5) {
        return None;
    }
    Some((record, word(data + DATA_EFFECT).unwrap_or(0)))
}

/// ONCE A FRAME in the world (`ChrIns_PrePhysics`). On while Sora's munny is the currency (`munny.mirror`, the default)
/// unless `bloodstain.keep = 1`.
pub fn frame(control: &Control) {
    let Ok(mut guard) = STATE.lock() else { return };
    let state = guard.get_or_insert_with(State::default);
    let on = control.on("munny.mirror") && control.flag("bloodstain.keep") != Some(true);
    if state.told != Some(on) {
        state.told = Some(on);
        erlog!("stain: Elden Ring's bloodstain (always empty: Sora's munny is never dropped) is {}", if on { "PUT OUT OF THE WAY when one exists: no marker, no \"Retrieve lost runes\" prompt (bloodstain.keep = 1 leaves it)" } else { "left to the game" });
    }
    if !on {
        restore(state);
        return;
    }
    let Some((record, effect)) = parts() else { return };
    if let Some(y) = floats::<1>(record + 4).map(|f| f[0]) {
        if y > AWAY + 1000.0 {
            erlog!("stain: a bloodstain at block-local height {y:.2} (record {record:#x}): moved to {AWAY} (its prompt cannot be reached)");
            state.moved = Some((record, y));
            // SAFETY: the record was reached through the checked controller and read an instant ago; game thread.
            unsafe { std::ptr::write_volatile((record + 4) as *mut f32, AWAY) };
        }
    }
    let Some(system) = crate::probe::fx_system() else { return };
    if effect != 0 && is(effect, &VT_FX, "FXHGSfxCtrl@FX4HG") {
        if let Some(fx) = word(effect + 0x18).filter(|r| *r != 0) {
            if let (Some(id), Some(now)) = (crate::probe::fx_record(fx, system), floats::<7>(fx + 0x0C)) {
                if now.iter().any(|f| *f != 0.0) {
                    erlog!("stain: its marker (effect {id}, record {fx:#x}) is not drawn");
                    state.hidden = Some((fx, id, now));
                    for k in 0..7 {
                        // SAFETY: the effect record's system pointer and id were checked and its floats read an instant ago.
                        unsafe { std::ptr::write_volatile((fx + 0x0C + 4 * k) as *mut f32, 0.0) };
                    }
                }
            }
        }
    }
}

fn restore(state: &mut State) {
    if let Some((record, y)) = state.moved.take() {
        if parts().is_some_and(|(r, _)| r == record) && floats::<1>(record + 4).is_some_and(|f| f[0] == AWAY) {
            // SAFETY: as in `frame`.
            unsafe { std::ptr::write_volatile((record + 4) as *mut f32, y) };
        }
    }
    if let (Some((fx, id, was)), Some(system)) = (state.hidden.take(), crate::probe::fx_system()) {
        if crate::probe::fx_record(fx, system) == Some(id) && floats::<7>(fx + 0x0C).is_some_and(|now| now.iter().all(|f| *f == 0.0)) {
            for (k, v) in was.iter().enumerate() {
                // SAFETY: as in `frame`.
                unsafe { std::ptr::write_volatile((fx + 0x0C + 4 * k) as *mut f32, *v) };
            }
        }
    }
}

/// The payload goes.
pub fn release() {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            restore(state);
        }
        *guard = None;
    }
}
