//! (GF-SENSOR) WHAT KIND OF ATTACK AN ELDEN RING ROW IS (R2 of build/sora_tmp/party/GOOFY/PLAN.md). Everything comes from the
//! running game's own params: TAE judge id -> BehaviorParam -> AtkParam_Npc (or Bullet -> AtkParam_Npc). No per-enemy data.
//! The rule below is pure, so the test level's script words and the offline tests can use it.

use kh_core::host::AttackClass;

/// `ATK_PARAM_ST.atkAttribute` 2: thrust (the meaning is checked in game, SENSOR_NOTES.md).
const ATTR_THRUST: u8 = 2;

/// The facts of one attack row the rule reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RowFacts {
    pub throw_type: u16,
    pub throw_flag: u8,
    pub disable_guard: bool,
    pub guard_rate: u16,
    pub attribute: u8,
    pub level: u8,
    pub arrow: bool,
    pub bullet: bool,
    /// The row deals magic, fire, lightning or holy damage.
    pub element: bool,
    /// The row deals physical damage.
    pub physical: bool,
}

/// `dmgLevel`: 3 Large, 4 ExLarge, 5 Push, 6 Fling, 7 SmallBlow, 9 Upper, 10 ExBlast, 11 Breath are heavy; 1 Small, 2 Middle and
/// 8 Minimum are light (IMPL_ER6_STATE.md's survey of the names). R2's "level 3 or more" with Minimum left out.
pub fn heavy_level(level: u8) -> bool {
    level >= 3 && level != 8
}

/// R2: the class of a row, and whether the danger cue shows.
pub fn classify(f: &RowFacts) -> (AttackClass, bool) {
    if f.throw_type != 0 || f.throw_flag != 0 {
        return (AttackClass::Grab, true);
    }
    if f.disable_guard || f.guard_rate == 999 {
        return (AttackClass::Heavy, true);
    }
    if f.bullet || f.arrow {
        let magic = f.bullet && f.element && !f.physical && !f.arrow;
        return (if magic { AttackClass::Magic } else { AttackClass::Projectile }, false);
    }
    if !heavy_level(f.level) {
        return (AttackClass::Normal, false);
    }
    if f.attribute == ATTR_THRUST {
        (AttackClass::Thrust, true)
    } else {
        (AttackClass::Sweep, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn melee(attribute: u8, level: u8) -> RowFacts {
        RowFacts { attribute, level, physical: true, ..RowFacts::default() }
    }

    #[test]
    fn melee_by_level_and_attribute() {
        assert_eq!(classify(&melee(0, 1)), (AttackClass::Normal, false));
        assert_eq!(classify(&melee(2, 2)), (AttackClass::Normal, false), "a light thrust is a normal attack");
        assert_eq!(classify(&melee(2, 3)), (AttackClass::Thrust, true));
        assert_eq!(classify(&melee(3, 4)), (AttackClass::Sweep, true), "perilous by elimination");
        assert_eq!(classify(&melee(0, 8)), (AttackClass::Normal, false), "Minimum is the lightest level");
    }

    #[test]
    fn grabs_and_unblockables_first() {
        assert_eq!(classify(&RowFacts { throw_type: 1, level: 1, ..RowFacts::default() }).0, AttackClass::Grab);
        assert_eq!(classify(&RowFacts { throw_flag: 1, ..RowFacts::default() }).0, AttackClass::Grab);
        assert_eq!(classify(&RowFacts { disable_guard: true, level: 3, attribute: 2, ..RowFacts::default() }).0, AttackClass::Heavy);
        assert_eq!(classify(&RowFacts { guard_rate: 999, ..RowFacts::default() }), (AttackClass::Heavy, true));
    }

    #[test]
    fn bullets_are_projectiles_or_magic() {
        assert_eq!(classify(&RowFacts { bullet: true, physical: true, attribute: 2, level: 3, ..RowFacts::default() }), (AttackClass::Projectile, false));
        assert_eq!(classify(&RowFacts { bullet: true, element: true, ..RowFacts::default() }), (AttackClass::Magic, false));
        assert_eq!(classify(&RowFacts { arrow: true, element: true, ..RowFacts::default() }).0, AttackClass::Projectile, "a fire arrow is an arrow");
    }
}
