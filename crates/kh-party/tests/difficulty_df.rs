//! (DF, 2026-10-15) KH3 Critical for Donald and Goofy: the base max HP / MP halved like Sora's (0x140575D73: their data
//! sets are the class default CDTS_TYPE_PLAYER), the bonus amounts of the Critical table, no Critical AP or abilities.

use kh_core::status::{BonusCounts, GameLevel};
use kh_party::{Chara, FriendStatus};

#[test]
fn critical_halves_the_friends_bases_and_their_bonus_steps() {
    for (chara, hp, mp) in [(Chara::Donald, 140, 100), (Chara::Goofy, 160, 100)] {
        let mut s = FriendStatus::new(chara, 0);
        assert_eq!((s.max_hp(), s.max_mp()), (hp, mp), "{chara:?} Standard");
        s.game_level = GameLevel::Critical;
        assert_eq!((s.max_hp(), s.max_mp()), (hp / 2, mp / 2), "{chara:?} Critical");
        // HP_UP10: 10 on Standard, 5 on Critical (0x1405864C0)
        s.bonuses.add(BonusCounts::HP_UP10);
        assert_eq!(s.max_hp(), hp / 2 + 5);
        s.game_level = GameLevel::Proud;
        assert_eq!(s.max_hp(), hp + 10, "Proud: Standard's numbers");
    }
    assert_eq!(kh_party::status::critical_base(1, GameLevel::Critical), 1, "at least 1");
}
