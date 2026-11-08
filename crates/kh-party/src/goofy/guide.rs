//! (GOOFY) THE PARRY TECHNIQUES PAGE'S WORDS (THE USER 2026-11-02: "Can we have a way to visually show in the camp menu what
//! parry techniques Goofy currently has, when to use them and how to perform them? (preferably using symbols from Kingdom
//! Hearts' controller icons)"): one row per technique, read from his live level and saved ranks, for the camp menu's
//! Abilities screen ("Show Next List" on Goofy: kh-ui camp_parry.rs). What each answers and how is documentation/
//! GOOFY_OVERHAUL.md 4.1, 4.3 and 6.1; the learn levels are the sheet's (`n_ex002.learn.*`, [`GoofyMove::learn_level`]), never
//! written here.
//!
//! (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique ... The parries are there to make the enemy's
//! stagger go up fast while taking 0 damage yourself. Redesign the menu for this as well") NO COUNTER COLUMN: each row says
//! what the technique does now, 0 damage and the Stagger it adds at his rank ([`parry::shown_stagger`], the kit's own rows),
//! drawn as a number and one to three bars; the page's [`FOOTER`] says how the full gauge's break leads to Goofy Breaker.
//!
//! COMPOSED TEXT (KH3 has no such page): every `when`, `how`, `detail` and the footer below is ours, in KH3's text markup. The
//! buttons are KH3's own pad icons of `UI/IconText` (`<icon(Square)>` the left face button = X on the Xbox pad, `<icon(Cross)>`
//! the bottom = A, `<icon(Circle)>` the right = B, `<icon(Triangle)>` the top = Y). The stick is KH3's left-stick icons; the kit
//! reads the stick against the enemy, not the screen, so "toward the enemy" is drawn as the stick pushed up (with the enemy
//! locked on the camera faces him), "away" as down, "sideways" as left / right, and the words say what is meant.

use super::learn::{self, GoofyMove, RankBank};
use super::parry::{self, Tech};

/// The page's title.
pub const TITLE: &str = "Parry Techniques";

/// COMPOSED: the line under the table, the loop of his kit (documentation/GOOFY_OVERHAUL.md 6): parry, Stagger, break, Breaker.
pub const FOOTER: &str = "Every technique takes 0 damage and fills the enemy's Stagger gauge. When it is full the enemy is broken: press <icon(Triangle)> for Goofy Breaker, his big hit.";

/// The eight techniques (documentation/GOOFY_OVERHAUL.md 11.1).
pub const TECHS: [Tech; 8] = [Tech::Deflect, Tech::ShieldReflect, Tech::HopCounter, Tech::PinningGuard, Tech::SlipCounter, Tech::BullBrace, Tech::SpellWard, Tech::RhythmDeflect];

/// The pad icons a row or the footer may name (KH3's `UI/IconText` pad set): the test checks the pack holds each.
pub const ICONS: [&str; 6] = ["Square", "Triangle", "Circle", "LstickUp", "LstickDown", "LstickLeftRight"];

/// The Stagger column's bars (COMPOSED): one under 30, two from 30, three from 40 (the shown number at his rank: Deflect's 25 is
/// one bar at rank 1 and two at rank 2; Bull Brace's 45 three).
pub const BARS_TWO: f32 = 30.0;
pub const BARS_THREE: f32 = 40.0;

/// What one technique answers and how it is done (COMPOSED, see the module text).
struct Words {
    /// The kind of attack, short (the page's "The enemy does" column).
    when: &'static str,
    /// The input, short, with KH3's pad icons (the "How" column).
    how: &'static str,
    /// The whole of it, for the help line under the page (its Stagger is added from the rows: [`GuideRow::help_text`]).
    detail: &'static str,
}

/// COMPOSED: the words of each technique (documentation/GOOFY_OVERHAUL.md 4.3 "The enemy does... / The answer", 6.1).
const fn words(t: Tech) -> Words {
    match t {
        Tech::Deflect => Words {
            when: "A normal attack",
            how: "Tap <icon(Square)> as the hit lands",
            detail: "Tap <icon(Square)> just before a normal attack lands, stick left alone. The hit clangs off his shield: 0 damage, no GUARD lost.",
        },
        Tech::ShieldReflect => Words {
            when: "An arrow or a projectile",
            how: "<icon(LstickUp)> at the shooter + tap <icon(Square)>",
            detail: "Push <icon(LstickUp)> toward the shooter and tap <icon(Square)> as the shot arrives: it stops on his shield for 0 damage, and the shooter staggers.",
        },
        Tech::HopCounter => Words {
            when: "A sweep or a low swing",
            how: "<icon(Circle)> jump over it",
            detail: "Jump with <icon(Circle)> as the sweep comes, and stay off the ground while it passes under him: 0 damage, and the enemy staggers.",
        },
        Tech::PinningGuard => Words {
            when: "A thrust or a lunge",
            how: "<icon(LstickUp)> toward the enemy + tap <icon(Square)> as it lands",
            detail: "Hold <icon(LstickUp)> toward the enemy and tap <icon(Square)> as the thrust lands: he pins it on his shield for 0 damage.",
        },
        Tech::SlipCounter => Words {
            when: "A grab",
            how: "<icon(LstickLeftRight)> sideways + <icon(Square)>: hop aside",
            detail: "Hop aside with <icon(Square)> and <icon(LstickLeftRight)> to the side as the grab closes: the hop can't be caught, 0 damage.",
        },
        Tech::BullBrace => Words {
            when: "An unblockable heavy or a charge",
            how: "Hold <icon(Square)> + <icon(LstickDown)> away, before impact",
            detail: "Hold <icon(Square)> and pull <icon(LstickDown)> away from the enemy before the blow arrives: Goofy plants his feet and takes it for 0 damage. The most Stagger of all.",
        },
        Tech::SpellWard => Words {
            when: "Magic or an incantation",
            how: "Hold <icon(Square)> through the cast, let go, tap <icon(Square)> on impact",
            detail: "Hold <icon(Square)> through the cast, let go, and tap <icon(Square)> again as the spell hits: his shield wards it off for 0 damage.",
        },
        Tech::RhythmDeflect => Words {
            when: "A long combo string",
            how: "Tap <icon(Square)> on every hit, in time",
            detail: "Deflect every hit of a string in time, 0 damage each. From the second deflect on, each one fills more Stagger than the last.",
        },
    }
}

/// An attack the danger "!" warns of (documentation/GOOFY_OVERHAUL.md 4.3: thrust, sweep, grab, unblockable heavy).
pub const fn perilous(t: Tech) -> bool {
    matches!(t, Tech::PinningGuard | Tech::HopCounter | Tech::SlipCounter | Tech::BullBrace)
}

/// The Stagger column's bars for a shown amount (1 ..= 3).
pub fn bars(amount: f32) -> u8 {
    if amount >= BARS_THREE {
        3
    } else if amount >= BARS_TWO {
        2
    } else {
        1
    }
}

/// The Stagger column's words: "+25", Rhythm Deflect's range "+28 to +46".
pub fn stagger_text(low: f32, high: f32) -> String {
    if (high - low).abs() < 0.5 {
        format!("+{low:.0}")
    } else {
        format!("+{low:.0} to +{high:.0}")
    }
}

/// One technique as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuideRow {
    pub tech: Tech,
    /// Its name ([`GoofyMove::label`]).
    pub name: &'static str,
    /// The level he learns it at (the sheet's).
    pub learn_level: u32,
    /// Its rank now: 0 not learned yet, else 1 ..= 3 ([`learn::rank`], the kit's own rule).
    pub rank: u8,
    /// Not learned yet and a boss has already given it a rank: the rank it will be learned at (2 or 3); else 0.
    pub saved: u8,
    /// The danger "!" warns of what it answers.
    pub perilous: bool,
    pub when: &'static str,
    pub how: &'static str,
    /// The Stagger it adds at his rank (rank 1 while locked): "+25" ([`stagger_text`]).
    pub stagger: String,
    /// Its bars, 1 ..= 3 ([`bars`]).
    pub stagger_bars: u8,
    pub detail: String,
}

/// One technique at `level` with the saved ranks of `bank`.
pub fn row(t: Tech, bank: &RankBank, level: u32) -> GuideRow {
    let mv: GoofyMove = t.mv();
    let rank = learn::rank(bank, mv, level);
    let saved = if rank == 0 && bank.earned(mv) > 0 { bank.rank(mv) } else { 0 };
    let w = words(t);
    let (low, high) = parry::shown_stagger(t, rank);
    let stagger = stagger_text(low, high);
    let detail = format!("{} Stagger {stagger}.", w.detail);
    GuideRow { tech: t, name: mv.label(), learn_level: mv.learn_level(), rank, saved, perilous: perilous(t), when: w.when, how: w.how, stagger_bars: bars(high), stagger, detail }
}

/// THE PAGE'S ROWS: all eight, in the order he learns them (the sheet's levels; equal levels keep [`TECHS`]' order).
pub fn rows(bank: &RankBank, level: u32) -> Vec<GuideRow> {
    let mut out: Vec<GuideRow> = TECHS.iter().map(|t| row(*t, bank, level)).collect();
    out.sort_by_key(|r| r.learn_level);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goofy::K;

    fn bank(steps: &[(GoofyMove, u64)]) -> RankBank {
        let mut parts = [0u64; GoofyMove::ALL.len()];
        for (m, n) in steps {
            if let Some(i) = GoofyMove::ALL.iter().position(|x| x == m) {
                parts[i] = *n;
            }
        }
        RankBank::from_parts(&parts)
    }

    #[test]
    fn eight_rows_in_learn_order_with_the_sheets_levels() {
        let r = rows(&RankBank::default(), 1);
        assert_eq!(r.len(), 8);
        let levels: Vec<u32> = r.iter().map(|x| x.learn_level).collect();
        assert!(levels.windows(2).all(|w| w[0] <= w[1]), "learn order: {levels:?}");
        for x in &r {
            assert_eq!(x.learn_level, x.tech.mv().learn_level(), "{}: the sheet's level", x.name);
            assert!(!x.when.is_empty() && !x.how.is_empty() && !x.detail.is_empty(), "{}", x.name);
        }
        assert_eq!(r.iter().map(|x| x.tech).collect::<std::collections::HashSet<_>>().len(), 8, "each technique once");
    }

    #[test]
    fn learned_or_locked_follows_the_level() {
        let empty = RankBank::default();
        for level in [1u32, 10, 20, 39, 40, 99] {
            for x in rows(&empty, level) {
                assert_eq!(x.rank > 0, level >= x.learn_level, "{} at level {level}", x.name);
                assert!(x.rank <= 1 && x.saved == 0, "{}: no boss rank yet", x.name);
            }
        }
        assert!(rows(&empty, 40).iter().all(|x| x.rank == 1), "all eight by level 40 (THE USER 2026-11-02)");
        // the start: Deflect only (it is learned at 0)
        let start: Vec<&str> = rows(&empty, 1).iter().filter(|x| x.rank > 0).map(|x| x.name).collect();
        assert!(start.contains(&"Deflect"), "{start:?}");
    }

    #[test]
    fn boss_ranks_show_and_wait() {
        // two Deflect steps (rank 3), one Pinning Guard step (rank 2) banked early
        let b = bank(&[(GoofyMove::Deflect, 2), (GoofyMove::PinningGuard, 1)]);
        let pin_level = GoofyMove::PinningGuard.learn_level();
        let find = |level: u32, t: Tech| rows(&b, level).into_iter().find(|x| x.tech == t).unwrap();
        assert_eq!(find(1, Tech::Deflect).rank, 3);
        let early = find(pin_level.saturating_sub(1).max(1), Tech::PinningGuard);
        if pin_level > 1 {
            assert_eq!((early.rank, early.saved), (0, 2), "the saved rank waits for the learn level");
        }
        let learned = find(pin_level, Tech::PinningGuard);
        assert_eq!((learned.rank, learned.saved), (2, 0), "learned at once at the saved rank");
        // past the top: still rank 3
        assert_eq!(find(99, Tech::Deflect).rank, 3);
        assert_eq!(rows(&bank(&[(GoofyMove::Deflect, 9)]), 1)[0].rank, 3);
    }

    #[test]
    fn the_words_name_only_the_pads_icons_and_no_counter() {
        let mut texts = vec![FOOTER.to_string()];
        for x in rows(&RankBank::default(), 99) {
            texts.extend([x.when.to_string(), x.how.to_string(), x.detail.clone()]);
            assert!(x.how.contains("<icon("), "{}: the input is drawn with the pad's icons", x.name);
            // (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique") no A after any of them
            assert!(!x.detail.contains("<icon(Cross)>") && !x.how.contains("<icon(Cross)>"), "{}: no A counter: {}", x.name, x.detail);
            assert!(x.detail.contains("0 damage") && x.detail.ends_with(&format!("Stagger {}.", x.stagger)), "{}: what it does now: {}", x.name, x.detail);
        }
        for text in &texts {
            let lower = text.to_lowercase();
            for gone in ["counter bash", "pin down", "goofy stomp", "pratfall", "return fire", "ward burst", "shield shove", "crescendo", "sekiro", "mikiri"] {
                assert!(!lower.contains(gone), "{gone} in {text}");
            }
            let mut rest = text.as_str();
            while let Some(at) = rest.find("<icon(") {
                let after = &rest[at + 6..];
                let end = after.find(")>").expect("closed icon tag");
                assert!(ICONS.contains(&&after[..end]), "unknown icon {} in {text}", &after[..end]);
                rest = &after[end + 2..];
            }
        }
        assert!(FOOTER.contains("<icon(Triangle)>") && FOOTER.contains("Goofy Breaker"), "the break leads to the Breaker on Y");
        assert_eq!(rows(&RankBank::default(), 1).iter().filter(|x| x.perilous).count(), 4, "thrust, sweep, grab, heavy");
    }

    /// (THE USER 2026-11-02) the Stagger column: each technique's own row at his rank, Rhythm Deflect's range, the bars.
    #[test]
    fn the_stagger_column_reads_the_kits_rows_at_his_rank() {
        let at = |b: &RankBank, level: u32, t: Tech| rows(b, level).into_iter().find(|x| x.tech == t).unwrap();
        let none = RankBank::default();
        let want = [(Tech::Deflect, "+25", 1), (Tech::ShieldReflect, "+30", 2), (Tech::SpellWard, "+34", 2), (Tech::HopCounter, "+36", 2), (Tech::PinningGuard, "+40", 3), (Tech::SlipCounter, "+40", 3), (Tech::BullBrace, "+45", 3), (Tech::RhythmDeflect, "+28 to +46", 3)];
        for (t, text, n) in want {
            let r = at(&none, 40, t);
            assert_eq!((r.stagger.as_str(), r.stagger_bars), (text, n), "{t:?}");
        }
        // a locked one shows its rank 1 amount
        assert_eq!(at(&none, 1, Tech::BullBrace).stagger, format!("+{:.0}", K.stagger_brace));
        // ranks raise it: Deflect at rank 2 and 3
        assert_eq!(at(&bank(&[(GoofyMove::Deflect, 1)]), 1, Tech::Deflect).stagger, format!("+{:.0}", K.stagger_deflect * K.stagger_rank2_mult));
        assert_eq!(at(&bank(&[(GoofyMove::Deflect, 2)]), 1, Tech::Deflect).stagger, format!("+{:.0}", K.stagger_deflect * K.stagger_rank3_mult));
        assert_eq!(at(&bank(&[(GoofyMove::Deflect, 1)]), 1, Tech::Deflect).stagger_bars, 2);
    }
}
