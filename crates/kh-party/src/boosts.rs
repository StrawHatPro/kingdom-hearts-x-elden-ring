//! (IT) THE ROSTER'S PERMANENT STAT BOOSTS. The user, 2026-10-17 00:50 (ER_PHASE_NOTES): "make sure any permenant stat
//! buff items you recieve are given to all possible party members, even if they aren't unlocked yet".
//!
//! One total for the whole roster: what was RECEIVED (KH3's Strength / Magic / Defense / AP Boost, and every Elden Ring
//! item the port maps to a permanent KH stat: the Whetblades' AP +1, the Memory Stone's max MP +5, any `reward:max_hp`
//! / `reward:max_mp` / `reward:ap` row). The party host keeps it (kh-partyhost `PartyRuntime::roster_boosts`), saves it
//! once in the party record, and copies it onto every friend's status (`FriendStatus::roster`), so a friend spawned
//! or unlocked later arrives with all of it, and a future character gets it the same way (nothing here names one).
//! Sora takes the same boost through his own status (his boost bytes / victory-bonus counts in his save) as before.
//!
//! The getters of `FriendStatus` add it ON TOP of the stat tables (stream BAL's values): marked `(IT)` there.

use kh_core::status::{BonusCounts, GameLevel};

/// One permanent boost received.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boost {
    /// Strength Boost (ITEM_POWERUP 514): Strength +1.
    Strength,
    /// Magic Boost (ITEM_MAGICUP 515): Magic +1.
    Magic,
    /// Defense Boost (ITEM_GUARDUP 516): Defense +1.
    Defense,
    /// AP Boost (ITEM_APUP 517) and the ER items mapped to AP: maximum AP + n.
    Ap(u8),
    /// A max-HP / max-MP step of KH3's victory-bonus kinds (`BonusCounts::HP_UP5`, `MP_UP5` ...): an ER item mapped to a
    /// permanent max HP / MP (the Memory Stone's MP +5). Slot bonuses are not stats and are not taken here.
    Bonus(usize),
}

impl Boost {
    /// Whether a victory-bonus kind is a max-HP / max-MP step (the only bonus kinds that are stats).
    pub const fn is_stat_bonus(kind: usize) -> bool {
        matches!(kind, BonusCounts::HP_UP3 | BonusCounts::HP_UP5 | BonusCounts::HP_UP10 | BonusCounts::HP_UP15 | BonusCounts::HP_UP30 | BonusCounts::MP_UP3 | BonusCounts::MP_UP5 | BonusCounts::MP_UP10 | BonusCounts::MP_UP20)
    }

    /// The panel's second line ("Strength +1"), KH3's wording of the boost items.
    pub fn line(self) -> String {
        match self {
            Boost::Strength => "Strength +1".to_string(),
            Boost::Magic => "Magic +1".to_string(),
            Boost::Defense => "Defense +1".to_string(),
            Boost::Ap(n) => format!("Maximum AP +{n}"),
            Boost::Bonus(k) => {
                let (hp, mp) = BonusCounts({
                    let mut c = [0u8; 32];
                    c[k.min(31)] = 1;
                    c
                })
                .totals(GameLevel::Standard);
                if hp > 0 {
                    format!("Max HP +{hp}")
                } else {
                    format!("Max MP +{mp}")
                }
            }
        }
    }
}

/// The roster's total.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct RosterBoosts {
    pub strength: u8,
    pub magic: u8,
    pub defense: u8,
    pub ap: u8,
    /// Max-HP / max-MP steps received (victory-bonus kinds, counted as `BonusCounts`).
    pub bonuses: BonusCounts,
}

/// The party record's key (additive: a save without it reads as "not recorded yet", see kh-partyhost's migration;
/// the JSON shape is kh-partyhost boosts.rs `to_json` / `from_json`).
pub const RECORD_KEY: &str = "roster_boosts";

impl RosterBoosts {
    pub const NONE: RosterBoosts = RosterBoosts { strength: 0, magic: 0, defense: 0, ap: 0, bonuses: BonusCounts([0; 32]) };

    /// One more boost; false = not a stat boost (a slot bonus) or the counter is full.
    pub fn add(&mut self, b: Boost) -> bool {
        let before = *self;
        match b {
            Boost::Strength => self.strength = self.strength.saturating_add(1),
            Boost::Magic => self.magic = self.magic.saturating_add(1),
            Boost::Defense => self.defense = self.defense.saturating_add(1),
            Boost::Ap(n) => self.ap = self.ap.saturating_add(n),
            Boost::Bonus(k) if Boost::is_stat_bonus(k) => self.bonuses.0[k] = self.bonuses.0[k].saturating_add(1),
            Boost::Bonus(_) => {}
        }
        *self != before
    }

    pub fn is_empty(&self) -> bool {
        *self == RosterBoosts::NONE
    }

    /// (max HP, max MP) the steps add at `level` (the friends' own Critical amounts, `BonusCounts::totals`).
    pub fn hp_mp(&self, level: GameLevel) -> (i32, i32) {
        self.bonuses.totals(level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_adds_and_round_trips() {
        let mut r = RosterBoosts::NONE;
        assert!(r.add(Boost::Strength) && r.add(Boost::Strength) && r.add(Boost::Magic) && r.add(Boost::Defense) && r.add(Boost::Ap(2)));
        assert!(r.add(Boost::Bonus(BonusCounts::MP_UP5)));
        assert!(!r.add(Boost::Bonus(BonusCounts::ACC_SLOT_UP1)), "a slot is not a stat");
        assert_eq!((r.strength, r.magic, r.defense, r.ap), (2, 1, 1, 2));
        assert_eq!(r.hp_mp(GameLevel::Standard), (0, 5));
        assert_eq!(Boost::Bonus(BonusCounts::MP_UP5).line(), "Max MP +5");
    }
}
