//! (GF-STATS) WHAT THE BOSSES GAVE A FRIEND, IN RAW AMOUNTS (build/sora_tmp/party/GOOFY/PLAN.md R9; documentation/
//! GOOFY_OVERHAUL.md 2, 11, 11.4; DONALD_OVERHAUL.md 2, 9). KH3's `BonusCounts` only knows its own steps (HP +3 / 5 / 10 / 15 /
//! 30), so the overhaul's +14, +9, +7, +4 ... live here: one [`Growth`] per friend, kept by the party host for the roster
//! whether he has joined or not (kh-partyhost growth.rs) and copied onto `FriendStatus::growth`.
//!
//! Max HP, max MP and the slots are DERIVED from it (status.rs), never added to saved values, so an old save's `bonuses` and
//! `slots` can't count twice. The game runs on Standard, so no Critical table applies to these amounts.
//!
//! THE TECHNIQUE AND ART STORE (for GF-CORE): [`RankBank`] counts the boss steps of each [`RankLine`], on
//! `FriendStatus::growth.ranks` and saved with the party record. It is the saved-rank store of GOOFY_OVERHAUL.md 11: a step
//! is banked even before he learns the move, in any boss order. His rank in a learned line is `1 + ranks.earned(line)`.
//! A step past rank 3 has already become Max HP +10 here ([`Growth::apply`]): nothing else may add that HP.

use crate::balance::BP;
use crate::ids::Chara;
use crate::status::{start_slots, MAX_SLOTS};

/// The party record's key (additive: a save without it reads as "nothing yet", and the catch-up rebuilds it).
pub const RECORD_KEY: &str = "friend_growth";

/// The ranks a boss can give a line: ranks 2 and 3 (level learning gives rank 1).
pub const BOSS_RANKS: u8 = 2;

/// One of Goofy's parry techniques or Shield Arts (GOOFY_OVERHAUL.md 11.1, 11.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RankLine {
    Deflect,
    ShieldReflect,
    HopCounter,
    PinningGuard,
    SlipCounter,
    BullBrace,
    SpellWard,
    RhythmDeflect,
    GoofyCharge,
    Taunt,
    GoofyTornado,
    GoofyBash,
    GoofyTurbo,
    Interpose,
    ShieldToss,
}

impl RankLine {
    pub const ALL: [RankLine; 15] = [RankLine::Deflect, RankLine::ShieldReflect, RankLine::HopCounter, RankLine::PinningGuard, RankLine::SlipCounter, RankLine::BullBrace, RankLine::SpellWard, RankLine::RhythmDeflect, RankLine::GoofyCharge, RankLine::Taunt, RankLine::GoofyTornado, RankLine::GoofyBash, RankLine::GoofyTurbo, RankLine::Interpose, RankLine::ShieldToss];

    /// The sheet's token: `tech:<name>` for a technique, `art:<name>` for a Shield Art.
    pub const fn token(self) -> &'static str {
        match self {
            RankLine::Deflect => "tech:deflect",
            RankLine::ShieldReflect => "tech:shield_reflect",
            RankLine::HopCounter => "tech:hop_counter",
            RankLine::PinningGuard => "tech:pinning_guard",
            RankLine::SlipCounter => "tech:slip_counter",
            RankLine::BullBrace => "tech:bull_brace",
            RankLine::SpellWard => "tech:spell_ward",
            RankLine::RhythmDeflect => "tech:rhythm_deflect",
            RankLine::GoofyCharge => "art:goofy_charge",
            RankLine::Taunt => "art:taunt",
            RankLine::GoofyTornado => "art:goofy_tornado",
            RankLine::GoofyBash => "art:goofy_bash",
            RankLine::GoofyTurbo => "art:goofy_turbo",
            RankLine::Interpose => "art:interpose",
            RankLine::ShieldToss => "art:shield_toss",
        }
    }

    /// The name the popups show.
    pub const fn label(self) -> &'static str {
        match self {
            RankLine::Deflect => "Deflect",
            RankLine::ShieldReflect => "Shield Reflect",
            RankLine::HopCounter => "Hop Counter",
            RankLine::PinningGuard => "Pinning Guard",
            RankLine::SlipCounter => "Slip Counter",
            RankLine::BullBrace => "Bull Brace",
            RankLine::SpellWard => "Spell Ward",
            RankLine::RhythmDeflect => "Rhythm Deflect",
            RankLine::GoofyCharge => "Goofy Charge",
            RankLine::Taunt => "Taunt",
            RankLine::GoofyTornado => "Goofy Tornado",
            RankLine::GoofyBash => "Goofy Bash",
            RankLine::GoofyTurbo => "Goofy Turbo",
            RankLine::Interpose => "Interpose",
            RankLine::ShieldToss => "Shield Toss",
        }
    }

    /// The level he learns it at (0 = from the start), for the saved-rank wording of the popup.
    pub const fn learn_level(self) -> u32 {
        match self {
            RankLine::Deflect => BP.n_ex002_learn_deflect as u32,
            RankLine::ShieldReflect => BP.n_ex002_learn_shield_reflect as u32,
            RankLine::HopCounter => BP.n_ex002_learn_hop_counter as u32,
            RankLine::PinningGuard => BP.n_ex002_learn_pinning_guard as u32,
            RankLine::SlipCounter => BP.n_ex002_learn_slip_counter as u32,
            RankLine::BullBrace => BP.n_ex002_learn_bull_brace as u32,
            RankLine::SpellWard => BP.n_ex002_learn_spell_ward as u32,
            RankLine::RhythmDeflect => BP.n_ex002_learn_rhythm_deflect as u32,
            RankLine::GoofyCharge => BP.n_ex002_learn_goofy_charge as u32,
            RankLine::Taunt => BP.n_ex002_learn_taunt as u32,
            RankLine::GoofyTornado => BP.n_ex002_learn_goofy_tornado as u32,
            RankLine::GoofyBash => BP.n_ex002_learn_goofy_bash as u32,
            RankLine::GoofyTurbo => BP.n_ex002_learn_goofy_turbo as u32,
            RankLine::Interpose => BP.n_ex002_learn_interpose as u32,
            RankLine::ShieldToss => BP.n_ex002_learn_shield_toss as u32,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// The boss steps banked per line: the saved-rank store (module text).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct RankBank {
    steps: [u8; RankLine::ALL.len()],
}

impl RankBank {
    /// Boss steps received for this line, past-the-top ones included.
    pub fn steps(&self, line: RankLine) -> u8 {
        self.steps[line.index()]
    }

    /// The ranks the bosses have earned this line (0..=2), whether he has learned it or not.
    pub fn earned(&self, line: RankLine) -> u8 {
        self.steps(line).min(BOSS_RANKS)
    }

    /// The line's rank once learned (1..=3), the saved ranks applying at once.
    pub fn rank(&self, line: RankLine) -> u8 {
        1 + self.earned(line)
    }

    /// The counts in `RankLine::ALL` order (the save).
    pub fn parts(&self) -> [u8; RankLine::ALL.len()] {
        self.steps
    }

    /// From a saved list; a short list leaves the rest at 0.
    pub fn from_parts(list: &[u64]) -> RankBank {
        let mut steps = [0u8; RankLine::ALL.len()];
        for (to, from) in steps.iter_mut().zip(list) {
            *to = (*from).min(u8::MAX as u64) as u8;
        }
        RankBank { steps }
    }
}

/// A slot kind of a reward row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Item,
    Armour,
    Accessory,
}

/// One step of a `gof.<row>` or `don.<row>` reward. Tokens: `hp:<n>`, `mp:<n>`, `slot:item|armour|accessory`, `tech:<name>`,
/// `art:<name>` (the last two Goofy's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrowthStep {
    MaxHp(i32),
    MaxMp(i32),
    Slot(SlotKind),
    Rank(RankLine),
}

impl GrowthStep {
    /// A sheet token; `None`: not one of these.
    pub fn parse(text: &str) -> Option<GrowthStep> {
        let t = text.trim().to_ascii_lowercase();
        if let Some(line) = RankLine::ALL.into_iter().find(|l| l.token() == t) {
            return Some(GrowthStep::Rank(line));
        }
        let (kind, what) = t.split_once(':')?;
        let amount = what.trim().parse::<i32>().ok().filter(|n| *n > 0);
        Some(match (kind.trim(), what.trim()) {
            ("hp", _) => GrowthStep::MaxHp(amount?),
            ("mp", _) => GrowthStep::MaxMp(amount?),
            ("slot", "item") => GrowthStep::Slot(SlotKind::Item),
            ("slot", "armour" | "armor") => GrowthStep::Slot(SlotKind::Armour),
            ("slot", "accessory") => GrowthStep::Slot(SlotKind::Accessory),
            _ => return None,
        })
    }
}

/// What one step gave, for the log and the BONUS! panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrowthGrant {
    MaxHp(i32),
    MaxMp(i32),
    Slot(SlotKind),
    /// A rank of a line: `rank` 2 or 3, waiting (saved) or usable depends on his level ([`GrowthGrant::label`]).
    Rank(RankLine, u8),
}

impl GrowthGrant {
    /// The words of a grant. `level`: Goofy's level now, which decides a rank's "ready" wording (GOOFY_OVERHAUL.md 11).
    pub fn label(&self, level: u32) -> String {
        match *self {
            GrowthGrant::MaxHp(n) => format!("Max HP +{n}"),
            GrowthGrant::MaxMp(n) => format!("Max MP +{n}"),
            GrowthGrant::Slot(SlotKind::Item) => "Item slot +1".to_string(),
            GrowthGrant::Slot(SlotKind::Armour) => "Armour slot +1".to_string(),
            GrowthGrant::Slot(SlotKind::Accessory) => "Accessory slot +1".to_string(),
            GrowthGrant::Rank(line, rank) if level < line.learn_level() => format!("{}: Rank {rank} ready", line.label()),
            GrowthGrant::Rank(line, rank) => format!("{}: Rank {rank}", line.label()),
        }
    }
}

/// What the bosses gave one friend (module text).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Growth {
    pub hp: i32,
    pub mp: i32,
    pub item_slots: u8,
    pub armour_slots: u8,
    pub accessory_slots: u8,
    /// Goofy's banked rank steps; empty for Donald.
    pub ranks: RankBank,
    /// (weapons P11) THE BOOST ITEMS USED ON HIM from the camp menu's Items screen (KH3's Strength / Magic / Defense / AP
    /// Boost on one chosen member: `kh_ui::camp_equip::BOOST_ITEMS`' order): his own, added by the stat getters on top of
    /// the roster's boosts (which every friend shares). Saved with this record.
    pub boosts: [u8; 4],
}

impl Growth {
    /// Every boss reward of `chara` (sheet rows `<chara>.rewards.*`): the test level's level-99 loadout.
    pub fn full(chara: Chara) -> Growth {
        match chara {
            Chara::Donald => Growth { hp: BP.n_ex001_rewards_hp as i32, mp: BP.n_ex001_rewards_mp as i32, item_slots: BP.n_ex001_rewards_item_slots as u8, armour_slots: BP.n_ex001_rewards_armour_slots as u8, accessory_slots: BP.n_ex001_rewards_accessory_slots as u8, ranks: RankBank::default(), boosts: [0; 4] /* (weapons P11) */ },
            Chara::Goofy => {
                let mut ranks = RankBank::default();
                for line in RankLine::ALL {
                    ranks.steps[line.index()] = BOSS_RANKS;
                }
                Growth { hp: BP.n_ex002_rewards_hp as i32, mp: BP.n_ex002_rewards_mp as i32, item_slots: BP.n_ex002_rewards_item_slots as u8, armour_slots: BP.n_ex002_rewards_armour_slots as u8, accessory_slots: BP.n_ex002_rewards_accessory_slots as u8, ranks, boosts: [0; 4] /* (weapons P11) */ }
            }
        }
    }

    /// One step onto this record. A step past rank 3 becomes `rank_overflow_hp` raw HP, and a slot past the party's limit
    /// gives nothing (`None`).
    pub fn apply(&mut self, chara: Chara, step: GrowthStep) -> Option<GrowthGrant> {
        match step {
            GrowthStep::MaxHp(n) => {
                self.hp += n;
                Some(GrowthGrant::MaxHp(n))
            }
            GrowthStep::MaxMp(n) => {
                self.mp += n;
                Some(GrowthGrant::MaxMp(n))
            }
            GrowthStep::Slot(kind) => {
                let (start, count) = match (kind, start_slots(chara)) {
                    (SlotKind::Item, (item, _, _)) => (item, &mut self.item_slots),
                    (SlotKind::Armour, (_, armour, _)) => (armour, &mut self.armour_slots),
                    (SlotKind::Accessory, (_, _, accessory)) => (accessory, &mut self.accessory_slots),
                };
                if (start + *count) as usize >= MAX_SLOTS {
                    return None;
                }
                *count += 1;
                Some(GrowthGrant::Slot(kind))
            }
            GrowthStep::Rank(line) => {
                let steps = &mut self.ranks.steps[line.index()];
                *steps = steps.saturating_add(1);
                if *steps > BOSS_RANKS {
                    let n = BP.n_ex002_rank_overflow_hp as i32;
                    self.hp += n;
                    return Some(GrowthGrant::MaxHp(n));
                }
                Some(GrowthGrant::Rank(line, 1 + *steps))
            }
        }
    }

    /// An odd saved record made safe: no negative amounts, no slot past the party's limit.
    pub fn clamped(self, chara: Chara) -> Growth {
        let (item, armour, accessory) = start_slots(chara);
        let room = |start: u8, n: u8| n.min((MAX_SLOTS as u8).saturating_sub(start));
        Growth { hp: self.hp.max(0), mp: self.mp.max(0), item_slots: room(item, self.item_slots), armour_slots: room(armour, self.armour_slots), accessory_slots: room(accessory, self.accessory_slots), ranks: self.ranks, boosts: self.boosts /* (weapons P11) */ }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_parse_and_every_line_has_its_own() {
        assert_eq!(GrowthStep::parse("hp:14"), Some(GrowthStep::MaxHp(14)));
        assert_eq!(GrowthStep::parse(" MP:7 "), Some(GrowthStep::MaxMp(7)));
        assert_eq!(GrowthStep::parse("slot:armor"), Some(GrowthStep::Slot(SlotKind::Armour)));
        assert_eq!(GrowthStep::parse("slot:accessory"), Some(GrowthStep::Slot(SlotKind::Accessory)));
        for bad in ["hp:0", "hp:-2", "mp:x", "slot:nope", "tech:nope", "magic:fire", "banish"] {
            assert_eq!(GrowthStep::parse(bad), None, "{bad}");
        }
        for line in RankLine::ALL {
            assert_eq!(GrowthStep::parse(line.token()), Some(GrowthStep::Rank(line)));
        }
        assert_eq!(RankLine::ALL.map(RankLine::index), std::array::from_fn(|k| k), "ALL is the save's order");
    }

    #[test]
    fn the_doc_table_of_learning_levels() {
        // (THE USER 2026-11-02: every technique by level 40)
        let levels = [(RankLine::Deflect, 0), (RankLine::ShieldReflect, 5), (RankLine::HopCounter, 10), (RankLine::PinningGuard, 15), (RankLine::SlipCounter, 20), (RankLine::BullBrace, 26), (RankLine::SpellWard, 33), (RankLine::RhythmDeflect, 40), (RankLine::GoofyCharge, 0), (RankLine::Taunt, 0), (RankLine::GoofyTornado, 8), (RankLine::GoofyBash, 15), (RankLine::GoofyTurbo, 25), (RankLine::Interpose, 35), (RankLine::ShieldToss, 50)];
        for (line, level) in levels {
            assert_eq!(line.learn_level(), level, "{line:?}");
        }
    }

    #[test]
    fn ranks_wait_and_a_step_past_rank_3_is_max_hp_10() {
        let mut g = Growth::default();
        let chara = Chara::Goofy;
        assert_eq!(g.apply(chara, GrowthStep::Rank(RankLine::PinningGuard)), Some(GrowthGrant::Rank(RankLine::PinningGuard, 2)));
        assert_eq!(g.ranks.rank(RankLine::PinningGuard), 2);
        assert_eq!(GrowthGrant::Rank(RankLine::PinningGuard, 2).label(10), "Pinning Guard: Rank 2 ready");
        assert_eq!(GrowthGrant::Rank(RankLine::PinningGuard, 2).label(20), "Pinning Guard: Rank 2");
        assert_eq!(g.apply(chara, GrowthStep::Rank(RankLine::Deflect)), Some(GrowthGrant::Rank(RankLine::Deflect, 2)));
        assert_eq!(g.apply(chara, GrowthStep::Rank(RankLine::Deflect)), Some(GrowthGrant::Rank(RankLine::Deflect, 3)));
        assert_eq!(g.hp, 0);
        assert_eq!(g.apply(chara, GrowthStep::Rank(RankLine::Deflect)), Some(GrowthGrant::MaxHp(10)));
        assert_eq!((g.hp, g.ranks.rank(RankLine::Deflect), g.ranks.steps(RankLine::Deflect)), (10, 3, 3));
    }

    #[test]
    fn slots_stop_at_the_partys_limit_and_a_saved_record_is_clamped() {
        let mut g = Growth::default();
        for _ in 0..3 {
            assert_eq!(g.apply(Chara::Goofy, GrowthStep::Slot(SlotKind::Armour)), Some(GrowthGrant::Slot(SlotKind::Armour)));
        }
        assert_eq!(g.apply(Chara::Goofy, GrowthStep::Slot(SlotKind::Armour)), None, "2 + 3 = the limit of 5");
        assert_eq!(g.armour_slots, 3);
        let odd = Growth { hp: -5, mp: 3, item_slots: 200, armour_slots: 9, accessory_slots: 1, ranks: RankBank::from_parts(&[900, 2]), boosts: [1, 0, 0, 2] /* (weapons P11) */ };
        let c = odd.clamped(Chara::Donald);
        assert_eq!((c.hp, c.mp, c.item_slots, c.armour_slots, c.accessory_slots), (0, 3, 3, 4, 1));
        assert_eq!((c.ranks.steps(RankLine::Deflect), c.ranks.steps(RankLine::ShieldReflect)), (255, 2));
        assert_eq!(c.boosts, [1, 0, 0, 2], "(weapons P11) the boost items used on him survive the clamp");
    }
}
