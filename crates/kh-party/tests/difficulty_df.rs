//! (DF, 2026-10-15) KH3 Critical for Donald and Goofy: the starting max HP / MP halved like Sora's (0x140575D73: their data
//! sets are the class default CDTS_TYPE_PLAYER), no Critical AP or abilities. (GF-STATS, PLAN.md R9) The boss rewards are raw
//! amounts: no Critical table applies to them.

use kh_core::status::GameLevel;
use kh_party::growth::{Growth, GrowthStep};
use kh_party::{Chara, FriendStatus};

#[test]
fn critical_halves_the_friends_starts_and_not_their_boss_rewards() {
    for (chara, hp, mp) in [(Chara::Donald, 89, 138), (Chara::Goofy, 142, 88)] {
        let mut s = FriendStatus::new(chara, 0);
        assert_eq!((s.max_hp(), s.max_mp()), (hp, mp), "{chara:?} Standard");
        s.game_level = GameLevel::Critical;
        assert_eq!((s.max_hp(), s.max_mp()), (hp / 2, mp / 2), "{chara:?} Critical");
        let mut record = Growth::default();
        record.apply(chara, GrowthStep::MaxHp(14));
        s.set_growth(record);
        assert_eq!(s.max_hp(), hp / 2 + 14);
        s.game_level = GameLevel::Proud;
        assert_eq!(s.max_hp(), hp + 14, "Proud: Standard's numbers");
    }
    assert_eq!(kh_party::status::critical_base(1, GameLevel::Critical), 1, "at least 1");
}
