//! (BN) The kit abilities a level-up teaches go on that level's LEVEL UP! window: `level_kit_abilities(c, from, to)` names
//! exactly the abilities of his kit's level table reached between the two levels.

use kh_host::party::PartyChara;
use kh_party::pad::{donald_kit::KitAbility, goofy_kit::GoofyAbility};
use kh_partyhost::custom::level_kit_abilities;

#[test]
fn a_level_names_the_kit_abilities_it_reaches() {
    for k in KitAbility::OWN {
        let Some(lv) = k.level() else { continue };
        assert!(level_kit_abilities(PartyChara::Donald, lv - 1, lv).contains(&k.label().to_string()), "{} at {lv}", k.label());
        assert!(!level_kit_abilities(PartyChara::Donald, lv, lv + 1).contains(&k.label().to_string()), "{}: not again after {lv}", k.label());
    }
    for k in GoofyAbility::OWN {
        let Some(lv) = k.level() else { continue };
        assert!(level_kit_abilities(PartyChara::Goofy, lv - 1, lv).contains(&k.label().to_string()), "{} at {lv}", k.label());
    }
    assert!(level_kit_abilities(PartyChara::Sora, 1, 99).is_empty(), "Sora's level abilities come with his own level-up (kh-core)");
    assert!(level_kit_abilities(PartyChara::Donald, 99, 99).is_empty());
}
