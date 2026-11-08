//! (ER-TWEAKS, THE USER 2026-11-06: "Remove Rennala's respec and cosmetic change options.") RENNALA'S TALK LIST WITHOUT
//! "REBIRTH" AND "COSMETICS": pure rules of the talk-command detour (hooks.rs `event_hook` asks [`trimmed`] for every
//! AddTalkListData before anything else; game.rs sets [`ON`] once a frame). The pattern of the smiths' trim (smith.rs).
//!
//! READ in the game's own talk scripts (read-only copies in build/sora_tmp/er/conv/game/esd, all 365 scripts incl. the DLC
//! maps scanned 2026-11-06 by build/sora_tmp/weapons/ER-TWEAKS/tools/rennala_scan.js; her script dumped to
//! build/sora_tmp/weapons/ER-TWEAKS/rennala/t200001400.txt):
//! - RENNALA, QUEEN OF THE FULL MOON (NpcName 120000) talks through ONE script, t200001400 (m14_00_00_00, the Raya Lucaria
//!   Grand Library). Group x39 state 15 builds her talk list with three UNCONDITIONAL AddTalkListData (command 19):
//!   `c1_19(1, 22000000, -1)` "Rebirth", `c1_19(2, 22000001, -1)` "Cosmetics", `c1_19(99, 22000002, -1)` "Leave", then
//!   ShowShopMessage(1). Result 1 -> the Larval Tear check and the rebirth menu (`c1_113()`, menu type 19); result 2 ->
//!   the character-creation menu (`c1_81(0)`, menu type 16); anything else (99, B) -> leave.
//! - Texts 22000000 / 22000001 are added by NO other script of the 365 (and t200001400 is the only script that runs the
//!   rebirth menu `c1_113`). The flags her rebirth writes (14009206, 14002702, the counter 14009220, 14009207) are read by
//!   her own script only: no event script of the 940 in er33/game/event holds them (tools/flagsearch.js). Removing the two
//!   options touches no story.
//!
//! THE RULE (`rennala.trim`, default on; 0 = Elden Ring's list): in her talk (ids 200000000 ..= 200099999: character 2000,
//! every map variant, as Boc's 223xxxxxx) AddTalkListData (19 / 149) with text 22000000 or 22000001 is NOT RUN: neither
//! Elden Ring's list nor the KH bubble has the option. "Leave" keeps index 99 (the index is what the script reads back), so
//! her list is "Leave" alone; her greetings and her story lines (x39 states 6 .. 12, x41) are untouched.
//! CONFIDENCE: read from the scripts; NOT SEEN IN THE GAME.
//!
//! (ER-TWEAKS follow-up 2026-11-07, THE USER on the Roundtable Hold mirror: "Remove it") THE MIRROR WITHOUT "APPLY
//! COSMETICS": the mirror ("Use mirror", ActionButtonText 7190) talks through ONE script, t608001110 (m11_10_00_00, Roundtable
//! Hold); group x34 state 5 adds `c1_19(1, 26080000, -1)` "Apply cosmetics" and `c1_19(99, 26080001, -1)` "Leave";
//! result 1 -> the character-creation menu in its cosmetics mode (`c1_81(1)`, state 1). Text 26080000 is added by no other
//! of the 365 scripts (build/sora_tmp/weapons/ER-TWEAKS/tools/mirror_scan.js; dump ER-TWEAKS/rennala/t608001110.txt). The
//! same rule: in its talk (608000000 ..= 608099999) that AddTalkListData is not run; "Leave" (index 99) stays, as does the
//! mirror's prompt. Key `mirror.trim` (default on; 0 = Elden Ring's list).

use std::sync::atomic::{AtomicBool, Ordering};

use super::rules::{cmd, Val};

/// Her talk ids: character 2000, every map variant (t200001400 is the one the game has).
pub const TALK_FIRST: i32 = 200_000_000;
pub const TALK_LAST: i32 = 200_099_999;
/// EventTextForTalk "Rebirth" (the respec).
pub const REBIRTH: i32 = 22_000_000;
/// EventTextForTalk "Cosmetics" (the appearance change).
pub const COSMETICS: i32 = 22_000_001;
/// The options her list does not get.
pub const TRIM: [i32; 2] = [REBIRTH, COSMETICS];

/// `rennala.trim` on (game.rs stores it each frame; read by the detour).
pub static ON: AtomicBool = AtomicBool::new(true);

/// (ER-TWEAKS follow-up) The Roundtable Hold mirror's talk ids (character 6080, every map variant; t608001110 is the one).
pub const MIRROR_FIRST: i32 = 608_000_000;
pub const MIRROR_LAST: i32 = 608_099_999;
/// EventTextForTalk "Apply cosmetics" (the mirror's appearance change).
pub const APPLY_COSMETICS: i32 = 26_080_000;
/// `mirror.trim` on (game.rs stores it each frame).
pub static MIRROR_ON: AtomicBool = AtomicBool::new(true);

/// The control key's value -> on (default on; only an explicit 0 gives Elden Ring's list back).
pub fn on_from(key: Option<bool>) -> bool {
    key != Some(false)
}

/// Rennala's talk script.
pub fn is_rennala_talk(talk_id: i32) -> bool {
    (TALK_FIRST..=TALK_LAST).contains(&talk_id)
}

/// A COMMAND of a talk script: `Some(text)` = an option of hers that is NOT added (the detour does not run the command).
/// `on` = the key; `id` = the command; `args` = its arguments after the command itself (index, text, flag ...).
pub fn trim(on: bool, talk_id: i32, id: i32, args: &[Val]) -> Option<i32> {
    trim_in(on && is_rennala_talk(talk_id), &TRIM, id, args)
}

/// (ER-TWEAKS follow-up) The mirror's talk script.
pub fn is_mirror_talk(talk_id: i32) -> bool {
    (MIRROR_FIRST..=MIRROR_LAST).contains(&talk_id)
}

/// (ER-TWEAKS follow-up) [`trim`] for the Roundtable Hold mirror: "Apply cosmetics" is not added (`on` = `mirror.trim`).
pub fn trim_mirror(on: bool, talk_id: i32, id: i32, args: &[Val]) -> Option<i32> {
    trim_in(on && is_mirror_talk(talk_id), &[APPLY_COSMETICS], id, args)
}

/// The shared rule: in a talk the caller matched (`here`), an AddTalkListData whose text is one of `texts`.
fn trim_in(here: bool, texts: &[i32], id: i32, args: &[Val]) -> Option<i32> {
    if !here || !(id == cmd::ADD_LIST || id == cmd::ADD_LIST_ALT) {
        return None;
    }
    let int = |k: usize| args.get(k).and_then(Val::int);
    // (a 19 with four arguments: the conditional form with its condition still first, as smith.rs reads it)
    [int(1), if id == cmd::ADD_LIST && args.len() == 4 { int(2) } else { None }].into_iter().flatten().find(|m| texts.contains(m))
}

/// [`trim`] with the key the frame stored (the detour's call).
/// (ER-TWEAKS follow-up) and [`trim_mirror`] with `mirror.trim`.
pub fn trimmed(talk_id: i32, id: i32, args: &[Val]) -> Option<i32> {
    trim(ON.load(Ordering::Relaxed), talk_id, id, args).or_else(|| trim_mirror(MIRROR_ON.load(Ordering::Relaxed), talk_id, id, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HER: i32 = 200_001_400;
    fn i(v: i32) -> Val {
        Val::Int(v)
    }

    #[test]
    fn her_rebirth_and_cosmetics_are_not_added() {
        assert_eq!(trim(true, HER, cmd::ADD_LIST, &[i(1), i(REBIRTH), i(-1)]), Some(REBIRTH));
        assert_eq!(trim(true, HER, cmd::ADD_LIST, &[i(2), i(COSMETICS), i(-1)]), Some(COSMETICS));
        // the other command form and the four-argument conditional form
        assert_eq!(trim(true, HER, cmd::ADD_LIST_ALT, &[i(1), i(REBIRTH), i(-1), i(0), i(0)]), Some(REBIRTH));
        assert_eq!(trim(true, HER, cmd::ADD_LIST, &[i(1), i(1), i(COSMETICS), i(-1)]), Some(COSMETICS));
        // any map variant of hers
        assert_eq!(trim(true, TALK_FIRST, cmd::ADD_LIST, &[i(1), i(REBIRTH), i(-1)]), Some(REBIRTH));
        assert_eq!(trim(true, TALK_LAST, cmd::ADD_LIST, &[i(2), i(COSMETICS), i(-1)]), Some(COSMETICS));
    }

    #[test]
    fn her_leave_and_everything_else_stay() {
        assert_eq!(trim(true, HER, cmd::ADD_LIST, &[i(99), i(22_000_002), i(-1)]), None, "Leave stays");
        assert_eq!(trim(true, HER, cmd::ADD_LIST, &[i(3), i(22_001_000), i(-1)]), None);
        // other commands of hers (the menus themselves, the dialogs, flags) are the game's
        for id in [cmd::SHOW_SHOP_MESSAGE, cmd::GENERIC_DIALOG, cmd::CLEAR_LIST, 11, 81, 113] {
            assert_eq!(trim(true, HER, id, &[i(1), i(REBIRTH), i(-1)]), None, "command {id}");
        }
        // missing or odd arguments
        assert_eq!(trim(true, HER, cmd::ADD_LIST, &[]), None);
        assert_eq!(trim(true, HER, cmd::ADD_LIST, &[i(1), Val::Other, i(-1)]), None);
    }

    #[test]
    fn other_characters_and_the_mirror_are_untouched() {
        // Boc, Kale, the grace, the Roundtable Hold mirror (t608001110: "Apply cosmetics" 26080000), Rennala's neighbours
        for talk in [223_006_000, 200_000_000 - 1, 200_100_000, 200_101_400, 200_111_400, 1000, 608_001_110] {
            assert_eq!(trim(true, talk, cmd::ADD_LIST, &[i(1), i(REBIRTH), i(-1)]), None, "talk {talk}");
            assert_eq!(trim(true, talk, cmd::ADD_LIST, &[i(1), i(26_080_000), i(-1)]), None, "talk {talk}");
        }
    }

    /// (ER-TWEAKS follow-up, the user: "Remove it") The Roundtable Hold mirror: "Apply cosmetics" is not added, its "Leave"
    /// and every other command stay; no other talk loses that text; Rennala's rule and the mirror's are apart.
    #[test]
    fn the_mirrors_apply_cosmetics_is_not_added() {
        const MIRROR: i32 = 608_001_110;
        assert_eq!(trim_mirror(true, MIRROR, cmd::ADD_LIST, &[i(1), i(APPLY_COSMETICS), i(-1)]), Some(APPLY_COSMETICS));
        assert_eq!(trim_mirror(true, MIRROR, cmd::ADD_LIST_ALT, &[i(1), i(APPLY_COSMETICS), i(-1), i(0), i(0)]), Some(APPLY_COSMETICS));
        assert_eq!(trim_mirror(true, MIRROR, cmd::ADD_LIST, &[i(1), i(1), i(APPLY_COSMETICS), i(-1)]), Some(APPLY_COSMETICS), "the conditional form");
        assert_eq!(trim_mirror(true, MIRROR_LAST, cmd::ADD_LIST, &[i(1), i(APPLY_COSMETICS), i(-1)]), Some(APPLY_COSMETICS));
        // its Leave, its other commands (the menu itself: c1_81), odd arguments
        assert_eq!(trim_mirror(true, MIRROR, cmd::ADD_LIST, &[i(99), i(26_080_001), i(-1)]), None, "Leave stays");
        for id in [cmd::SHOW_SHOP_MESSAGE, cmd::CLEAR_LIST, 81] {
            assert_eq!(trim_mirror(true, MIRROR, id, &[i(1), i(APPLY_COSMETICS), i(-1)]), None, "command {id}");
        }
        assert_eq!(trim_mirror(true, MIRROR, cmd::ADD_LIST, &[]), None);
        // other talks keep the text; the mirror keeps Rennala's texts (neither rule reaches the other's talk)
        for talk in [MIRROR_FIRST - 1, MIRROR_LAST + 1, 200_001_400, 223_006_000, 1000] {
            assert_eq!(trim_mirror(true, talk, cmd::ADD_LIST, &[i(1), i(APPLY_COSMETICS), i(-1)]), None, "talk {talk}");
        }
        assert_eq!(trim_mirror(true, MIRROR, cmd::ADD_LIST, &[i(1), i(REBIRTH), i(-1)]), None);
        assert_eq!(trim(true, MIRROR, cmd::ADD_LIST, &[i(1), i(APPLY_COSMETICS), i(-1)]), None);
        // the key `mirror.trim = 0`: Elden Ring's list
        assert_eq!(trim_mirror(false, MIRROR, cmd::ADD_LIST, &[i(1), i(APPLY_COSMETICS), i(-1)]), None);
    }

    #[test]
    fn the_key_gives_elden_rings_list_back() {
        assert!(on_from(None));
        assert!(on_from(Some(true)));
        assert!(!on_from(Some(false)));
        assert_eq!(trim(false, HER, cmd::ADD_LIST, &[i(1), i(REBIRTH), i(-1)]), None);
        assert_eq!(trim(false, HER, cmd::ADD_LIST, &[i(2), i(COSMETICS), i(-1)]), None);
    }
}
