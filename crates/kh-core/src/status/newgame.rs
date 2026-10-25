//! (new game) THE TWO CHOICES OF THE OPENING (docs/sora-stats-spec.md 2.3 and section 14).
//!
//! In the game the opening level (`Levels/ew/ew_01`) asks twice, each time with three mirrors
//! (left, centre, right) [A, blueprints `ew_01_gameflow` and `ew_01_camera_ms010`]:
//!
//! | question (`kh3_level.ew`) | left | centre | right |
//! |---|---|---|---|
//! | `text_0020` "What do you desire?" | Wisdom (`text_0120`) | Vitality (`text_0110`) | Balance (`text_0130`) |
//! | `text_0030` "What power do you seek?" | the guardian (`text_0230`) | the warrior (`text_0210`) | the mystic (`text_0220`) |
//!
//! What they set [A + I]: the first one ONLY a victory bonus (`Vbonus_079` HP_UP30 / `Vbonus_080`
//! HP_UP15 + MP_UP10 / `Vbonus_081` MP_UP20; the growth type also stored at GW+0x7E0C has no
//! reader); the second one ONLY the ability path GW+0x7E0D = the column of the level table
//! (warrior 0 = `m_AbilityKind1`, mystic 1 = Kind2, guardian 2 = Kind3). Neither changes Strength,
//! Magic, Defense, AP, the start abilities or anything else; all three paths hold the same 28
//! abilities by level 52.

use super::{Growth, Loadout};

/// The second choice: the ability path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Power {
    /// The centre mirror, `2ndChoice` 0: path A (`m_AbilityKind1`; Combo Plus first).
    Warrior = 0,
    /// The right mirror, `2ndChoice` 1: path B (`m_AbilityKind2`; Magic Combo Thrift first).
    Mystic = 1,
    /// The left mirror, `2ndChoice` 2: path C (`m_AbilityKind3`; Damage Control first).
    Guardian = 2,
}

impl Power {
    pub const ALL: [Power; 3] = [Power::Warrior, Power::Mystic, Power::Guardian];

    /// The ability path 0 / 1 / 2 (GW+0x7E0D).
    pub const fn path(self) -> u8 {
        self as u8
    }

    pub const fn from_path(path: u8) -> Power {
        match path {
            1 => Power::Mystic,
            2 => Power::Guardian,
            _ => Power::Warrior,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Power::Warrior => "warrior",
            Power::Mystic => "mystic",
            Power::Guardian => "guardian",
        }
    }

    pub fn from_name(name: &str) -> Option<Power> {
        Power::ALL.into_iter().find(|p| p.name().eq_ignore_ascii_case(name.trim()))
    }

    /// The letter of the level table's column.
    pub const fn letter(self) -> char {
        match self {
            Power::Warrior => 'A',
            Power::Mystic => 'B',
            Power::Guardian => 'C',
        }
    }
}

/// The pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OpeningChoice {
    pub desire: Growth,
    pub power: Power,
}

impl OpeningChoice {
    /// What a host that does not ask starts with (the pair of the test loadouts): NOT a default of
    /// the game, which always asks.
    pub const DEFAULT: OpeningChoice = OpeningChoice { desire: Growth::Balance, power: Power::Warrior };

    /// The nine pairs.
    pub fn all() -> impl Iterator<Item = OpeningChoice> {
        Growth::ALL.into_iter().flat_map(|desire| Power::ALL.into_iter().map(move |power| OpeningChoice { desire, power }))
    }

    /// As the loadout row `choice.default` writes it: "balance + ability path A".
    pub fn loadout_text(self) -> String {
        format!("{} + ability path {}", self.desire.name(), self.power.letter())
    }

    /// The pair a loadout names (`choice.default`); `None` without a first word it knows.
    pub fn of_loadout(loadout: &Loadout) -> Option<OpeningChoice> {
        Some(OpeningChoice { desire: loadout.growth()?, power: Power::from_path(loadout.ability_path()) })
    }

    /// Max HP and max MP of a new game on Standard / Beginner / Proud (base 90 / 100 + the choice row).
    pub const fn start_hp_mp(self) -> (i32, i32) {
        match self.desire {
            Growth::Vitality => (120, 100),
            Growth::Balance => (105, 110),
            Growth::Wisdom => (90, 120),
        }
    }
}

impl Loadout {
    /// Puts the opening pair into the loadout (`choice.default`) and picks its stated HP / MP rows again.
    pub fn set_choice(&mut self, choice: OpeningChoice) {
        self.choice = choice.loadout_text();
        let bonuses = self.bonuses;
        if self.resolve().is_err() {
            // No `hp.*` rows for this pair: nothing is stated, the rules compute.
            (self.hp_max, self.mp_max) = (0.0, 0.0);
        }
        // (`resolve` fills a missing bonus row with the level-99 set: not wanted here)
        self.bonuses = bonuses;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_form_round_trips() {
        for choice in OpeningChoice::all() {
            let mut loadout = Loadout::new("t");
            loadout.set_choice(choice);
            assert_eq!(OpeningChoice::of_loadout(&loadout), Some(choice), "{}", choice.loadout_text());
            assert_eq!(loadout.bonuses, None);
        }
        assert_eq!(OpeningChoice::all().count(), 9);
        assert_eq!(OpeningChoice::DEFAULT.loadout_text(), "balance + ability path A");
        assert_eq!((Power::from_name("Guardian"), Power::Guardian.path(), Power::from_path(1)), (Some(Power::Guardian), 2, Power::Mystic));
    }
}
