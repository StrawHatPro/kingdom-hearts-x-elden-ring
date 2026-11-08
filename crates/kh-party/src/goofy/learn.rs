//! (GOOFY) HIS RANKS (documentation/GOOFY_OVERHAUL.md 11): eight parry techniques and seven Shield Arts, three ranks each. The
//! saved-rank store is GF-STATS' `crate::growth::RankBank` (`FriendStatus::growth.ranks`, the roster's copy, saved in the party
//! record, granted by the bosses; a step past rank 3 has already become Max HP +10 there). Here is the one rule that reads it:
//! rank 1 comes from his level, ranks 2 and 3 from bosses, and a banked rank waits for the level that teaches the move.

pub use crate::growth::{RankBank, RankLine as GoofyMove};

/// The top rank.
pub const MAX_RANK: u8 = 3;

/// The rank of `m` at `level`: 0 not learned yet, else 1 + the banked ranks.
pub fn rank(bank: &RankBank, m: GoofyMove, level: u32) -> u8 {
    if level >= m.learn_level() {
        bank.rank(m)
    } else {
        0
    }
}
