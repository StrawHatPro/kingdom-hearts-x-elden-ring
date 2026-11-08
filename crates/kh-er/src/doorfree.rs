//! (ER-TWEAKS, THE USER 2026-11-06: "Remove animations for door opening currently used by the party leader, when Y is
//! pressed the door opens but the party leader isn't locked into any animation regardless of door type") EVERY DOOR AND
//! LEVER OF ELDEN RING LEAVES THE LEADER FREE.
//!
//! WHAT THE GAME DOES (read from its own data, both regulations: build/sora_tmp/er/port272/reg/regulation_2710.bin and
//! regulation_2720.bin give the same rows; tool build/sora_tmp/weapons/ER-TWEAKS/tools/objact_scan.js, objact_detail.js):
//! Y at a door's prompt reaches the game, which runs the door's ObjAct (ObjActParam row): it plays the row's
//! `playerAnimId` on its (hidden) body and the row's `objAnimId` on the door itself; the event that opens it for good (its
//! flag, the EMEVD ObjAct event) is the door's, kicked by the ObjAct (`eventKickTiming`), never by where the player stands.
//! Every ObjActParam row's player animation, by its prompt (ActionButtonParam.textId -> ActionButtonText) and whether the
//! row gives a treasure (`enableTreasureDelaySec` > 0: a chest, chest.rs' own):
//!
//! | player animation | prompt | rows | kind |
//! |---|---|---|---|
//! | 60000 | Open | 42 | single door (the commonest) |
//! | 60002, 60020, 60040, 60160, 60170, 60180, 60190, 60370, 60500 | Open | 3, 2, 4, 6, 12, 12, 4, 4, 1 | doors of other shapes (60370: AEG110_064 and kin, measured in the user's log `final3` 353.342) |
//! | 60030 | Open / Close | 32 | double door (MEASURED IMPL_ER17: the Stranded Graveyard's AEG027_039, 6.83 s) |
//! | 60200, 60202 | Pull lever | 46, 4 | lever (doors, gates, lifts) |
//! | 60231, 60232 | Push lever | 3, 1 | lever |
//! | 60080, 60110, 60120 | Open | 7, 1, 3 | chests (treasure): NOT doors (chest.rs) |
//!
//! WHAT THE MOD DID: the double door's 60030 was a row `own.move.door_double` (sheets/port/sora-er-host.json): the leader
//! followed the body (`Follow::Action`), took no input and showed the Tarnished's door push retargeted onto him for 6.83 s;
//! the key `own.unlisted` (off by default) would have done the same for every other door.
//!
//! THE FIX: every door / lever animation above is a FREE action, as Elden Ring's pickup already is (IMPL_ER67,
//! guards.rs `pickup_anim`): the leader is neither held nor seated on the body nor shown in its pose; he keeps the pad and the
//! body is written to where he is as always. The body still plays its animation (hidden) and the game opens the door with its
//! own ObjAct, flag and event. The row `own.move.door_double` is gone from the sheet and the defaults. Any leader (Sora in any
//! outfit, Donald, Goofy, the Mimic, later ones): the rule is on the body's animation, read before anyone leads.
//! Keys: `door.free = 0` gives the old behaviour back; `door.anims = a-b, c` replaces the list.

/// The door and lever animations of the Tarnished (ObjActParam `playerAnimId`, every non-treasure row whose prompt is
/// "Open", "Close", "Pull lever" or "Push lever"; the table above).
pub const DOOR_ANIMS: [(i32, i32); 15] = [
    (60000, 60000),
    (60002, 60002),
    (60020, 60020),
    (60030, 60030),
    (60040, 60040),
    (60160, 60160),
    (60170, 60170),
    (60180, 60180),
    (60190, 60190),
    (60370, 60370),
    (60500, 60500),
    (60200, 60200),
    (60202, 60202),
    (60231, 60231),
    (60232, 60232),
];

/// True: the body's animation `anim` opens a door or pulls a lever, and the leader stays free. `keyed`: the key `door.anims`
/// (ranges as `own.anims`; empty or absent = [`DOOR_ANIMS`]). A chest's animation is never one (chest.rs has it).
pub fn free_anim(anim: i32, keyed: Option<&str>) -> bool {
    if crate::chest::default_anim(anim) {
        return false;
    }
    match keyed.map(str::trim) {
        None | Some("") => DOOR_ANIMS.iter().any(|(a, b)| (*a..=*b).contains(&anim)),
        Some(text) => crate::ermap::own_ranges(text).iter().any(|(a, b)| (*a..=*b).contains(&anim)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every door shape and every lever of the regulation leaves the leader free.
    #[test]
    fn every_door_and_lever_is_free() {
        for anim in [60000, 60002, 60020, 60030, 60040, 60160, 60170, 60180, 60190, 60370, 60500, 60200, 60202, 60231, 60232] {
            assert!(free_anim(anim, None), "{anim} is a door / lever");
        }
    }

    /// Chests, the fog, pickups, graces, ladders and the game's other actions are not doors.
    #[test]
    fn chests_fog_and_other_actions_are_not_doors() {
        for anim in [60080, 60110, 60120, 60060, 60071, 60260, 60010, 60530, 61010, 26011, 68000, 68011, 28000, 0, -1] {
            assert!(!free_anim(anim, None), "{anim} is no door");
        }
    }

    /// The key replaces the list; a chest stays chest.rs' own even if keyed.
    #[test]
    fn the_key_replaces_the_list() {
        assert!(free_anim(60100, Some("60100")));
        assert!(!free_anim(60000, Some("60100")));
        assert!(free_anim(60000, Some("  ")), "empty = the default list");
        assert!(!free_anim(60080, Some("60000-60999")), "a chest is never a door");
    }

    /// The double door is no longer a followed row of the sheet or of the defaults (it was `own.move.door_double`).
    #[test]
    fn the_double_door_is_no_followed_row() {
        let p = crate::ermap::Params::default();
        assert!(!p.own_moves.iter().any(|(a, b)| (*a..=*b).contains(&60030)), "60030 followed by default");
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-er-host.json")).expect("the host sheet");
        assert!(!text.contains("\"own.move.door_double\""), "the sheet still follows the double door");
        let (sheet, problems) = crate::ermap::Params::from_sheet(&text).expect("the host sheet parses");
        assert!(problems.is_empty(), "{problems:?}");
        for (a, b) in sheet.own_moves.iter().chain(sheet.own_anims.iter()).filter(|(a, b)| b >= a) {
            for anim in *a..=*b {
                assert!(!free_anim(anim, None), "the sheet still holds / follows the door animation {anim}");
            }
        }
    }
}
