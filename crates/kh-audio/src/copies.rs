//! (MIMIC-FX, THE USER 2026-11-06: "sounds made by the copy MUST be played from it's position and not the player's") THE
//! COPIES' OWN EMITTERS. Every copy of a character (the Mimic Tear bosses' copies, the boss's summoned Mimic Tear, the
//! player's Mimic Sora) is a party member of its OWN id: its seam handles lie in that id's party range (kh_core::host::party
//! `PARTY_HANDLE_BASE + id * PARTY_HANDLE_SPAN`), every sound of it is attached to that id (`Attach::Party { member: id }`,
//! resolved on ITS taps by the host), and the engine keeps an audio state of its own for it ([`crate::engine::PartyAudio`]):
//! the routes of the character it copies, its own voice line and voice group number, its own random-voice memory, its own
//! motion-sound detector (footsteps from ITS bones), its own weapon sets. Two instances of the same character never share
//! any of it, so a copy of the leader never cuts, blocks or lip-syncs the leader's voice and never plays at his place.
//!
//! The ids sit above the party's own (0 Donald, 1 Goofy; the staged friends use the same) and below the team range (15).

/// A Donald copy (the boss's copy of a Donald leader).
pub const DONALD: u8 = 8;
/// A Goofy copy (the boss's copy of a Goofy leader).
pub const GOOFY: u8 = 9;
/// The Mimic Tear boss's Sora copy.
pub const BOSS_SORA: u8 = 10;
/// The boss's own summoned Mimic Tear (a Sora copy).
pub const BOSS_MIMIC: u8 = 11;
/// The player's summoned Mimic Sora.
pub const MIMIC_SORA: u8 = 12;

/// Who a copy copies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyOf {
    Donald,
    Goofy,
    Sora,
}

/// The character member id `member` copies (None: not a copy's id).
pub fn copy_of(member: u8) -> Option<CopyOf> {
    match member {
        DONALD => Some(CopyOf::Donald),
        GOOFY => Some(CopyOf::Goofy),
        BOSS_SORA | BOSS_MIMIC | MIMIC_SORA => Some(CopyOf::Sora),
        _ => None,
    }
}

/// A copy's name for the logs.
pub fn name(member: u8) -> &'static str {
    match member {
        DONALD => "Donald's copy",
        GOOFY => "Goofy's copy",
        BOSS_SORA => "the boss's Sora copy",
        BOSS_MIMIC => "the boss's Mimic Sora",
        MIMIC_SORA => "Mimic Sora",
        _ => "member",
    }
}

/// (MIMIC-BOSS hotfix 3, MB3-AUDIO) kh_core::host::party's seam handle layout (kh-audio does not depend on kh-core; kh-er copyfx.rs's test holds
/// them equal): member `m`'s range starts at `PARTY_HANDLE_BASE + m * PARTY_HANDLE_SPAN`.
pub const PARTY_HANDLE_BASE: u32 = 0x2000_0000;
/// (MIMIC-BOSS hotfix 3, MB3-AUDIO) See [`PARTY_HANDLE_BASE`].
pub const PARTY_HANDLE_SPAN: u32 = 0x0100_0000;

/// (MIMIC-BOSS hotfix 3, MB3-AUDIO) The copy whose seam / effect handle `handle` is (kh-er copyfx.rs `own_handle`), None for any other handle.
pub fn of_handle(handle: u32) -> Option<u8> {
    let member = handle.checked_sub(PARTY_HANDLE_BASE)? / PARTY_HANDLE_SPAN;
    let member = u8::try_from(member).ok()?;
    copy_of(member).map(|_| member)
}

/// (MIMIC-BOSS hotfix 3, MB3-AUDIO) THE DRIVER SCOPE OF A MEMBER'S SOUNDS (`Driver::set_scope`): every copy is its own instance (its id), so the
/// player's Sora and a copy of him never cancel, stop or steal each other's sounds through KH3's start macros, category
/// pools or UE concurrency; 0 (the KH3 world, KH3's rules among Sora, Donald and Goofy as in KH3) for everyone else.
pub fn scope(member: u8) -> u32 {
    if copy_of(member).is_some() {
        u32::from(member)
    } else {
        0
    }
}

/// A Sora copy's id (his own Sora routes, his own voice line).
pub fn is_sora_copy(member: u8) -> bool {
    copy_of(member) == Some(CopyOf::Sora)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mimic_fx_the_copy_ids_are_their_own_and_below_the_team_range() {
        let ids = [DONALD, GOOFY, BOSS_SORA, BOSS_MIMIC, MIMIC_SORA];
        for (i, a) in ids.iter().enumerate() {
            assert!(*a > 1 && *a < 15, "{a}: above the party's own ids, below the team range");
            assert!(ids[i + 1..].iter().all(|b| b != a));
            assert!(copy_of(*a).is_some());
        }
        assert_eq!(copy_of(0), None);
        assert_eq!(copy_of(1), None);
        assert_eq!(copy_of(15), None);
        assert!(is_sora_copy(MIMIC_SORA) && !is_sora_copy(DONALD));
    }
}

#[cfg(test)]
mod engine_tests {
    use super::*;

    /// (MIMIC-FX) The engine knows every copy as a member of its own: a friend copy with its character's routes, a Sora copy
    /// with Sora's own (owner ""); the real party's ids unchanged; nothing else.
    #[test]
    fn mimic_fx_the_engine_gives_every_copy_its_characters_routes() {
        use crate::engine::party_owner;
        assert_eq!(party_owner(0), Some(("n_ex001", "Donald")));
        assert_eq!(party_owner(1), Some(("n_ex002", "Goofy")));
        assert_eq!(party_owner(DONALD), Some(("n_ex001", "Donald's copy")));
        assert_eq!(party_owner(GOOFY), Some(("n_ex002", "Goofy's copy")));
        for m in [BOSS_SORA, BOSS_MIMIC, MIMIC_SORA] {
            assert_eq!(party_owner(m).map(|o| o.0), Some(""), "{}", name(m));
        }
        assert_eq!(party_owner(2), None);
        assert_eq!(party_owner(15), None);
    }
}
