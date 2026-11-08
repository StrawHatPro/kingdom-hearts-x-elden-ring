//! (GOOFY, THE USER 2026-11-02: "Can we have a way to visually show in the camp menu what parry techniques Goofy currently
//! has, when to use them and how to perform them?") GOOFY'S PARRY TECHNIQUES PAGE FOR THE CAMP MENU: kh-party's rows
//! (`goofy::guide`: learned or not by his level, the ranks the bosses gave, the COMPOSED words of when and how, the Stagger
//! each adds at his rank, the footer that leads the break to Goofy Breaker) as the camp menu's feed (kh-ui camp_parry.rs, shown
//! by his Abilities screen's "Show Next List"). Built from the live `FriendStatus` each time the camp menu reads the party
//! (`PartyRuntime::menu_members`), for both hosts (the test level and Elden Ring), whether he leads or is an AI member.
//! (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique ... Redesign the menu for this as well") no
//! counter column any more.

use kh_host::party::{ParryPage, ParryRow};
use kh_party::goofy::guide;
use kh_party::goofy::RankBank;

/// The page of a Goofy at `level` with the saved ranks `bank`.
pub fn page(name: &str, level: u32, bank: &RankBank) -> ParryPage {
    let rows = guide::rows(bank, level)
        .into_iter()
        .map(|r| ParryRow {
            name: r.name.to_string(),
            learn_level: r.learn_level,
            rank: r.rank,
            saved: r.saved,
            perilous: r.perilous,
            when: r.when.to_string(),
            how: r.how.to_string(),
            stagger: r.stagger,
            stagger_bars: r.stagger_bars,
            detail: r.detail,
        })
        .collect();
    ParryPage { title: guide::TITLE.to_string(), name: name.to_string(), level, rows, footer: guide::FOOTER.to_string() }
}

/// The page of a friend's status: Goofy's (the overhaul's state is his), None for anyone else.
pub fn of_status(name: &str, s: &kh_party::status::FriendStatus) -> Option<ParryPage> {
    s.goofy.as_ref().map(|_| page(name, s.level, &s.growth.ranks))
}
