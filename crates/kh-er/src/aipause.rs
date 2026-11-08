//! (GF-POLISH) PIN DOWN'S "THE ENEMY'S NEXT ATTACK IS DELAYED" (documentation/GOOFY_OVERHAUL.md 6.1): the character's AI off for a moment
//! (SetCharacterAIState, `sdkx::set_ai_on`), the swing it is in plays on, then its AI back. Asked through `StaggerAsk::Delay`.
//!
//! THE RULES, all of them for the one bit this writes:
//! - Never on a body barred from the kit's tricks (`ErTargets::stagger_barred`: an event, a cutscene, a no-death state, Zettaflare), and
//!   only on a body whose AI is on: one that is off is an event's or Zettaflare's, and stays theirs.
//! - It comes back when the time is up, or at once when the body turns barred meanwhile.
//! - A body nobody can find has no bit to give back: it is forgotten `lost` seconds later. A load forgets them all and writes nothing:
//!   a map's enemies keep their handles across a reload, and the fresh body's AI is the map's, not ours.
//! - Every other reader of the AI bit asks [`crate::puppet::ai_switch`], which answers "on" for a body paused here. So Zettaflare or a
//!   stance freeze that takes the body meanwhile gives it back on, never the "off" read from us.

use std::sync::Mutex;

use eldenring::cs::FieldInsHandle;
use kh_core::TargetId;

use crate::erlog;
use crate::puppet::ai_switch;
use crate::targets::ErTargets;

struct Pause {
    id: TargetId,
    handle: FieldInsHandle,
    /// Seconds of the pause left.
    left: f32,
    /// Seconds nobody could find it, and how long that may go on.
    lost: f32,
    lost_max: f32,
}

/// What a frame does with a pause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    Hold,
    Restore,
    Forget,
}

fn act(found: bool, barred: bool, left: f32, lost: f32, lost_max: f32) -> Act {
    match found {
        false if lost >= lost_max => Act::Forget,
        false => Act::Hold,
        true if barred || left <= 0.0 => Act::Restore,
        true => Act::Hold,
    }
}

static PAUSES: Mutex<Vec<Pause>> = Mutex::new(Vec::new());
/// `game::player_age` of the last frame: it falling is a load.
static AGE: Mutex<u32> = Mutex::new(0);

/// `handle`'s AI off for `seconds`, or for longer if it is paused already. False: it is not switched off (no AI bit to read, or an
/// AI that is off already and so not ours to bring back).
pub fn begin(id: TargetId, handle: FieldInsHandle, seconds: f32, lost: f32) -> bool {
    let mut list = PAUSES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = list.iter_mut().find(|p| p.handle == handle) {
        p.left = p.left.max(seconds);
        return true;
    }
    let Some(chr) = crate::targets::find_chr(&handle) else { return false };
    if ai_switch::ai_bit(chr) != Some(true) || !ai_switch::set_ai_on(chr, false) {
        return false;
    }
    erlog!("aipause: target {}: its AI stands still for {seconds:.2} s", id.0);
    list.push(Pause { id, handle, left: seconds, lost: 0.0, lost_max: lost });
    true
}

/// This body's AI is off because of a pause here (its true state is "on").
pub fn holds(handle: &FieldInsHandle) -> bool {
    PAUSES.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|p| p.handle == *handle)
}

/// Once a frame.
pub fn frame(targets: &ErTargets, dt: f32) {
    let age = crate::game::player_age();
    let loaded = {
        let mut seen = AGE.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::replace(&mut *seen, age) > age
    };
    let mut list = PAUSES.lock().unwrap_or_else(|e| e.into_inner());
    if loaded && !list.is_empty() {
        erlog!("aipause: a load: {} pause(s) forgotten, nothing written", list.len());
        list.clear();
    }
    list.retain_mut(|p| {
        p.left -= dt;
        let chr = crate::targets::find_chr(&p.handle);
        if chr.is_none() {
            p.lost += dt;
        }
        let barred = chr.is_some() && targets.stagger_barred(p.id).is_some();
        match act(chr.is_some(), barred, p.left, p.lost, p.lost_max) {
            Act::Hold => true,
            Act::Forget => {
                erlog!("aipause: target {}: the body is gone: forgotten", p.id.0);
                false
            }
            Act::Restore => {
                if let Some(chr) = chr {
                    let written = ai_switch::set_ai_on(chr, true);
                    erlog!("aipause: target {}: its AI is back ({written}){}", p.id.0, if barred { ", early: the body may not be touched now" } else { "" });
                }
                false
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_comes_back_when_the_time_is_up() {
        assert_eq!(act(true, false, 0.2, 0.0, 2.0), Act::Hold);
        assert_eq!(act(true, false, 0.0, 0.0, 2.0), Act::Restore);
        assert_eq!(act(true, false, -0.1, 0.0, 2.0), Act::Restore);
    }

    #[test]
    fn a_body_that_turns_barred_comes_back_at_once() {
        assert_eq!(act(true, true, 0.7, 0.0, 2.0), Act::Restore);
    }

    #[test]
    fn a_body_gone_for_a_while_is_kept_then_forgotten_and_one_that_returns_is_restored() {
        assert_eq!(act(false, false, 0.5, 0.1, 2.0), Act::Hold);
        assert_eq!(act(false, false, -5.0, 1.9, 2.0), Act::Hold, "the time alone never forgets a body that may be back");
        assert_eq!(act(false, false, -5.0, 2.0, 2.0), Act::Forget);
        assert_eq!(act(true, false, -5.0, 1.9, 2.0), Act::Restore);
    }

    #[test]
    fn a_handle_nobody_paused_is_not_held() {
        let list = PAUSES.lock().unwrap_or_else(|e| e.into_inner());
        assert!(list.is_empty());
    }
}
