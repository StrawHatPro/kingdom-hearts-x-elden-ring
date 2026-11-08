//! (GF-STATS) THE FRIENDS' BOSS REWARDS IN THE PARTY HOST (documentation/GOOFY_OVERHAUL.md 11.4, DONALD_OVERHAUL.md 9; build/
//! sora_tmp/party/GOOFY/PLAN.md R9, R10):
//!
//! - THE ROSTER'S RECORD ([`PartyRuntime::growth_of`]): one [`Growth`] per friend, whether he has joined or not, copied onto his
//!   status at every step and at his spawn. Saved in the party record's ADDITIVE key `friend_growth`.
//! - THE ONCE-ONLY RECORDS, in Sora's records beside `boss:`: `gof:<row>` for Goofy's rows, `donstat:<row>` for the raw steps on
//!   Donald's rows. Those are not `don:<row>`: a save whose Donald already took a row's magic still has its HP, MP and slots
//!   to come.
//! - THE RULE IS THE LIVE GRANT AND THE CATCH-UP OF AN OLDER SAVE ([`don_stat_due`], [`gof_due`]): a beaten boss whose record is
//!   absent is due. Old `bonuses` and old `slots` are never added to: HP, MP and slots are derived from the record, so a first
//!   load after the update rebuilds them once and the totals may shift a little.
//! - THE GROUP RULE (R10): Crucible Knights and Erdtree Avatars stay `reserved` for Sora, so no `boss:` record exists for them.
//!   A `gof.*` row with a `group` reads its members' defeat flags directly: the first one seen counts, the later copies stay
//!   small bosses, and an older save catches up from the flags.

use kh_host::bossrewards::{recorded, DonRow, GofRow, Table};
use kh_host::party::PartyChara;
use kh_party::growth::{Growth, GrowthGrant, GrowthStep, RankBank};
use kh_party::Chara;
use serde_json::{json, Value};

use crate::runtime::PartyRuntime;
use crate::say;

fn friend_json(g: &Growth) -> Value {
    // (weapons P11) "boosts": the boost items used on him (an ADDITIVE key: a record without it reads as none)
    json!({ "hp": g.hp, "mp": g.mp, "item": g.item_slots, "armour": g.armour_slots, "accessory": g.accessory_slots, "ranks": g.ranks.parts().to_vec(), "boosts": g.boosts.to_vec() })
}

fn friend_of_json(v: &Value, chara: Chara) -> Growth {
    let count = |k: &str| v[k].as_u64().unwrap_or(0).min(u8::MAX as u64) as u8;
    let amount = |k: &str| v[k].as_i64().unwrap_or(0).clamp(0, i32::MAX as i64) as i32;
    let ranks: Vec<u64> = v["ranks"].as_array().map(|a| a.iter().map(|x| x.as_u64().unwrap_or(0)).collect()).unwrap_or_default();
    let boost = |k: usize| v["boosts"][k].as_u64().unwrap_or(0).min(u8::MAX as u64) as u8; // (weapons P11)
    Growth { hp: amount("hp"), mp: amount("mp"), item_slots: count("item"), armour_slots: count("armour"), accessory_slots: count("accessory"), ranks: RankBank::from_parts(&ranks), boosts: [boost(0), boost(1), boost(2), boost(3)] }.clamped(chara)
}

/// The record: `{"v": 1, "donald": {...}, "goofy": {...}}`.
pub fn to_json(donald: &Growth, goofy: &Growth) -> Value {
    json!({ "v": 1, "donald": friend_json(donald), "goofy": friend_json(goofy) })
}

/// The record's value (None: not an object, a save older than the overhaul). Odd values are clamped.
pub fn from_json(v: &Value) -> Option<(Growth, Growth)> {
    v.is_object().then(|| (friend_of_json(&v["donald"], Chara::Donald), friend_of_json(&v["goofy"], Chara::Goofy)))
}

/// A record of the raw steps of Donald's row (`donstat:<row id>`, beside `don:<row id>`).
pub fn don_stat_record(id: &str) -> String {
    format!("donstat:{id}")
}

/// A record of Goofy's row (`gof:<row id>`).
pub fn gof_record(id: &str) -> String {
    format!("gof:{id}")
}

fn has(records: &[String], key: &str) -> bool {
    records.iter().any(|k| k.eq_ignore_ascii_case(key))
}

/// The steps of a row's tokens; the sheet's check (`bossrewards::Table::parse`) leaves no others.
pub fn steps_of(tokens: &[String]) -> Vec<GrowthStep> {
    tokens.iter().filter_map(|t| GrowthStep::parse(t)).collect()
}

/// THE RULE FOR DONALD (pure): rows with raw steps whose boss is beaten and whose `donstat:<id>` is absent.
pub fn don_stat_due<'a>(rows: &'a [DonRow], records: &[String]) -> Vec<&'a DonRow> {
    rows.iter().filter(|r| !r.growth.is_empty() && recorded(records, &r.boss) && !has(records, &don_stat_record(&r.boss))).collect()
}

/// A row is beaten: its boss's `boss:` record, or the first defeat flag of its group (`flag`: the game's event flags). A
/// member whose flag has no row, or an unreadable flag, counts as not beaten.
pub fn gof_beaten(table: &Table, row: &GofRow, records: &[String], flag: &dyn Fn(u32) -> bool) -> bool {
    if row.group.is_empty() {
        return recorded(records, &row.boss);
    }
    row.group.iter().filter_map(|m| table.row(m)).filter_map(|r| r.flag).any(|(f, _)| flag(f))
}

/// THE RULE FOR GOOFY (pure): rows beaten whose `gof:<id>` is absent.
pub fn gof_due<'a>(table: &'a Table, records: &[String], flag: &dyn Fn(u32) -> bool) -> Vec<&'a GofRow> {
    table.gof.iter().filter(|r| gof_beaten(table, r, records, flag) && !has(records, &gof_record(&r.boss))).collect()
}

impl PartyRuntime {
    /// The roster's record of a friend (Sora has none).
    pub fn growth_of(&self, c: PartyChara) -> Growth {
        match c {
            PartyChara::Donald => self.friend_growth[0],
            PartyChara::Goofy => self.friend_growth[1],
            PartyChara::Sora => Growth::default(),
        }
    }

    /// The roster's record onto every spawned friend. The test level (no real progression) keeps its level-99 loadout.
    pub fn sync_growth(&mut self) {
        if !self.progression {
            return;
        }
        for (c, id) in self.members.clone() {
            let g = self.growth_of(c);
            if let Some(s) = self.party.status_mut(id) {
                s.set_growth(g);
            }
        }
    }

    /// The record of a loaded party record (none in an older save: nothing yet, the catch-up rebuilds it).
    pub fn load_growth_record(&mut self, v: &Value) {
        let (donald, goofy) = from_json(v).unwrap_or_default();
        self.friend_growth = [donald, goofy];
        self.sync_growth();
    }

    /// The party record's value of the roster's record.
    pub fn growth_record(&self) -> Value {
        to_json(&self.friend_growth[0], &self.friend_growth[1])
    }

    /// (weapons P11) A BOOST ITEM USED FROM THE CAMP MENU reaches each friend (THE USER: "Whole party"; `PartyMenuEdit::Boost`, one per friend; KH3's
    /// Strength / Magic / Defense / AP Boost raise that member's stat by one for good): into his own record (saved with
    /// it), onto his status at once; the test level (no progression: its level-99 loadout is not the record) takes it on
    /// the status alone. `stat`: 0 Strength, 1 Magic, 2 Defense, 3 AP. False: no such stat / friend, or the byte is full.
    pub fn apply_boost_item(&mut self, c: PartyChara, stat: u8) -> bool {
        let stat = stat as usize;
        let slot = match c {
            PartyChara::Donald => 0,
            PartyChara::Goofy => 1,
            PartyChara::Sora => return false,
        };
        if stat >= 4 || self.friend_growth[slot].boosts[stat] == u8::MAX {
            return false;
        }
        self.friend_growth[slot].boosts[stat] += 1;
        if self.progression {
            self.sync_growth();
        } else if let Some(s) = self.id(c).and_then(|id| self.party.status_mut(id)) {
            let mut g = s.growth;
            g.boosts[stat] = g.boosts[stat].saturating_add(1);
            s.set_growth(g);
        }
        say!("party: {} used a boost item (stat {stat}): his boosts now {:?}", c.name(), self.friend_growth[slot].boosts);
        true
    }

    /// One row's steps for a friend, joined or not: what they gave, in the row's order.
    fn grow(&mut self, c: PartyChara, tokens: &[String]) -> Vec<GrowthGrant> {
        let (slot, chara) = if c == PartyChara::Donald { (0, Chara::Donald) } else { (1, Chara::Goofy) };
        let grants = steps_of(tokens).into_iter().filter_map(|s| self.friend_growth[slot].apply(chara, s)).collect();
        self.sync_growth();
        grants
    }

    /// DONALD'S RAW STEPS OF THE BEATEN BOSSES (pure over `records`, which gets the `donstat:` records): (row, grants) in the
    /// table's order. The catch-up and the live grants alike.
    pub fn don_stat_claim_due(&mut self, rows: &[DonRow], records: &mut Vec<String>) -> Vec<(String, Vec<GrowthGrant>)> {
        let due: Vec<DonRow> = don_stat_due(rows, records).into_iter().cloned().collect();
        let mut out = Vec::new();
        for row in due {
            let grants = self.grow(PartyChara::Donald, &row.growth);
            records.push(don_stat_record(&row.boss));
            say!("party: Donald's boss reward {}: {}", row.boss, grants.iter().map(|g| g.label(0)).collect::<Vec<_>>().join(", "));
            out.push((row.boss, grants));
        }
        out
    }

    /// GOOFY'S STEPS OF THE BEATEN BOSSES (pure over `records`, which gets the `gof:` records; `flag`: the game's event flags,
    /// for the groups): (row, grants) in the table's order. The catch-up and the live grants alike.
    pub fn gof_claim_due(&mut self, table: &Table, records: &mut Vec<String>, flag: &dyn Fn(u32) -> bool) -> Vec<(String, Vec<GrowthGrant>)> {
        let due: Vec<GofRow> = gof_due(table, records, flag).into_iter().cloned().collect();
        let mut out = Vec::new();
        for row in due {
            let grants = self.grow(PartyChara::Goofy, &row.steps);
            records.push(gof_record(&row.boss));
            let level = self.goofy_level();
            say!("party: Goofy's boss reward {}: {}", row.boss, grants.iter().map(|g| g.label(level)).collect::<Vec<_>>().join(", "));
            out.push((row.boss, grants));
        }
        out
    }

    /// Goofy's level now, joined or not (his own table at the shared EXP): what a rank's "ready" wording reads.
    pub fn goofy_level(&self) -> u32 {
        self.id(PartyChara::Goofy).and_then(|id| self.party.status(id)).map_or_else(|| kh_party::tables::level_for_exp(Chara::Goofy, self.last_exp.max(0)), |s| s.level)
    }
}

/// (GF-STATS, GF-UI) THE BONUS! PANEL'S LINE OF A FRIEND'S BOSS REWARD: HP and MP with their amounts and a rank through
/// `GrowthGrant::label` ("Max HP +10", "Pinning Guard: Rank 2 ready"), a rank after the ability sparkle, a slot in KH3's own words.
/// `level`: his level, for the "ready" wording. Both hosts' panels.
pub fn growth_line(g: &GrowthGrant, level: u32) -> String {
    use kh_host::party::{icon_line, LineIcon, SideStrings};
    use kh_party::growth::SlotKind;
    let side = SideStrings::default();
    match g {
        GrowthGrant::MaxHp(_) | GrowthGrant::MaxMp(_) => g.label(level),
        GrowthGrant::Slot(SlotKind::Armour) => side.bonus_lines[2].clone(),
        GrowthGrant::Slot(SlotKind::Accessory) => side.bonus_lines[3].clone(),
        GrowthGrant::Slot(SlotKind::Item) => side.bonus_lines[4].clone(),
        GrowthGrant::Rank(..) => icon_line(LineIcon::Ability, &g.label(level)),
    }
}

impl PartyRuntime {
    /// (GF-UI) WHAT THE LEVELS `from` + 1 ..= `to` TAUGHT GOOFY OF HIS TECHNIQUES AND ARTS: their names, with the rank his saved boss
    /// steps give it at once ("Pinning Guard: Rank 2"), for the LEVEL UP! window.
    pub fn gof_level_lines(&self, c: PartyChara, from: u32, to: u32) -> Vec<String> {
        let Some(s) = self.id(c).filter(|_| c == PartyChara::Goofy).and_then(|id| self.party.status(id)) else { return Vec::new() };
        let mut lines: Vec<_> = kh_party::growth::RankLine::ALL.into_iter().filter(|l| l.learn_level() > from && l.learn_level() <= to).collect();
        lines.sort_by_key(|l| l.learn_level());
        lines
            .into_iter()
            .map(|l| match s.growth.ranks.rank(l) {
                1 => l.label().to_string(),
                rank => GrowthGrant::Rank(l, rank).label(to),
            })
            .collect()
    }

    /// (GF-UI) Takes the friends' BONUS! panels the test level's boss words queued.
    pub fn take_bonus_panels(&mut self) -> Vec<(PartyChara, Vec<String>)> {
        std::mem::take(&mut self.bonus_panels)
    }

    /// (GF-UI) THE TEST LEVEL'S STAND-IN FOR ELDEN RING'S POLL: the boss `id` is beaten: Goofy's row, its panel as the hosts show it.
    pub fn gof_boss_word(&mut self, table: &Table, records: &mut Vec<String>) -> Vec<String> {
        let got = self.gof_claim_due(table, records, &|_| false);
        let level = self.goofy_level();
        let mut said = Vec::new();
        for (_, grants) in got {
            let lines: Vec<String> = grants.iter().map(|g| growth_line(g, level)).collect();
            said.extend(lines.iter().cloned());
            self.bonus_panels.push((PartyChara::Goofy, lines));
        }
        said
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FriendRecord, FriendReward, PartyTest, RuntimeConfig};
    use kh_core::status::BonusCounts;
    use kh_party::growth::{RankLine, SlotKind, RECORD_KEY};

    fn rt() -> PartyRuntime {
        let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        rt.progression = true;
        rt.party = kh_party::Party::new(kh_party::PartyConfig { level99: false, total_exp: 0, seed: 1 });
        rt
    }

    fn table() -> Table {
        Table::parse(kh_host::bossrewards::SHEET)
    }

    fn no_flag(_: u32) -> bool {
        false
    }

    /// Every boss row of the table beaten, every flag of its groups set.
    fn all_bosses(t: &Table) -> Vec<String> {
        t.rows.iter().filter(|r| !r.reserved).map(|r| format!("boss:{}", r.id)).collect()
    }

    fn sum(rows: impl Iterator<Item = String>) -> (i32, i32, [i32; 3]) {
        let (mut hp, mut mp, mut slots) = (0, 0, [0; 3]);
        for t in rows {
            match GrowthStep::parse(&t) {
                Some(GrowthStep::MaxHp(n)) => hp += n,
                Some(GrowthStep::MaxMp(n)) => mp += n,
                Some(GrowthStep::Slot(SlotKind::Item)) => slots[0] += 1,
                Some(GrowthStep::Slot(SlotKind::Armour)) => slots[1] += 1,
                Some(GrowthStep::Slot(SlotKind::Accessory)) => slots[2] += 1,
                _ => {}
            }
        }
        (hp, mp, slots)
    }

    /// The docs' sums, read from the sheet's rows (GOOFY_OVERHAUL.md 2, DONALD_OVERHAUL.md 2).
    #[test]
    fn the_sheets_rows_add_up_to_the_docs_totals() {
        let t = table();
        assert!(t.problems.is_empty(), "{:?}", t.problems);
        assert_eq!(sum(t.don.iter().flat_map(|d| d.growth.clone())), (143, 80, [3, 1, 3]), "Donald 89 -> 232 HP, 138 -> 218 MP");
        assert_eq!(sum(t.gof.iter().flat_map(|g| g.steps.clone())), (224, 48, [2, 3, 2]), "Goofy 142 -> 366 HP, 88 -> 136 MP");
    }

    /// Every boss of the doc's lists gives what they say. The doc's rows that have only an ability have no `gof.` row.
    #[test]
    fn goofys_list_is_the_docs_boss_by_boss() {
        let t = table();
        let of = |boss: &str| t.gof.iter().find(|g| g.boss == boss).map(|g| g.steps.join(" ")).unwrap_or_default();
        for (boss, want) in [("tree_sentinel", "hp:14"), ("agheel", "art:taunt"), ("margit", "tech:deflect slot:armour"), ("radahn", "tech:shield_reflect mp:4 slot:item"), ("malenia", "tech:deflect tech:rhythm_deflect"), ("gideon", "hp:14 mp:4"), ("consort_radahn", "hp:14 slot:accessory"), ("adula", "art:shield_toss"), ("crucible_knights", "tech:pinning_guard"), ("erdtree_avatars", "tech:bull_brace"), ("loretta_caria", ""), ("rennala", "")] {
            assert_eq!(of(boss), want, "{boss}");
        }
        // each line has its two bosses, and Deflect its three
        for line in RankLine::ALL {
            let n = t.gof.iter().flat_map(|g| &g.steps).filter(|s| s.as_str() == line.token()).count();
            assert_eq!(n, if line == RankLine::Deflect { 3 } else { 2 }, "{line:?}");
        }
        let don = |boss: &str| t.don.iter().find(|d| d.boss == boss).map(|d| d.growth.join(" ")).unwrap_or_default();
        for (boss, want) in [("tree_sentinel", "hp:9"), ("makar", "hp:9 slot:item"), ("godrick", "slot:accessory"), ("elemer", "slot:armour"), ("gideon", "hp:9 mp:7"), ("metyr", "hp:9 mp:3"), ("consort_radahn", "hp:9"), ("margit", "")] {
            assert_eq!(don(boss), want, "{boss}");
        }
    }

    /// EVERY REWARD, in the table's order and backwards, joined late or from the start: the docs' totals, and once only.
    #[test]
    fn with_every_reward_the_totals_are_the_docs_in_any_order_joined_or_not() {
        let t = table();
        for reverse in [false, true] {
            for joined in [false, true] {
                let mut r = rt();
                if joined {
                    r.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
                    r.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
                }
                let mut bosses = all_bosses(&t);
                if reverse {
                    bosses.reverse();
                }
                let mut records: Vec<String> = Vec::new();
                let flags = |f: u32| f == t.row("crucible_knight_caelid").unwrap().flag.unwrap().0 || f == t.row("erdtree_avatar_weeping").unwrap().flag.unwrap().0;
                for b in bosses {
                    records.push(b);
                    r.don_stat_claim_due(&t.don, &mut records);
                    r.gof_claim_due(&t, &mut records, &flags);
                }
                assert!(r.don_stat_claim_due(&t.don, &mut records).is_empty() && r.gof_claim_due(&t, &mut records, &flags).is_empty(), "nothing twice");
                if !joined {
                    r.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
                    r.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
                }
                let s = |c: PartyChara| r.party.status(r.id(c).unwrap()).unwrap();
                let (d, g) = (s(PartyChara::Donald), s(PartyChara::Goofy));
                assert_eq!((d.max_hp(), d.max_mp(), d.item_slots, d.armour_slots, d.accessory_slots), (232, 218, 5, 2, 5), "Donald, reverse {reverse}, joined {joined}");
                assert_eq!((g.max_hp(), g.max_mp(), g.item_slots, g.armour_slots, g.accessory_slots), (376, 136, 5, 5, 3), "Goofy: 366 + Deflect's third boss's 10");
                assert!(RankLine::ALL.iter().all(|l| g.growth.ranks.rank(*l) == 3), "every line at rank 3");
            }
        }
    }

    /// Nothing of Sora's reaches a friend: not his HP, his MP or his opening choice.
    #[test]
    fn the_friends_stats_are_independent_of_sora() {
        let t = table();
        let make = |choice: kh_core::status::OpeningChoice, reward: bool| {
            let mut r = rt();
            let base = kh_core::status::Loadout::default();
            let mut sora = kh_core::Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: kh_core::Vec3::ZERO, yaw: 0.0 });
            *sora.status_mut() = kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout_for(&base, choice));
            if reward {
                sora.grant_quiet(kh_core::status::Reward::max_hp(30).unwrap());
                sora.grant_quiet(kh_core::status::Reward::max_mp(20).unwrap());
            }
            let mut records = vec!["boss:margit".to_string(), "boss:tree_sentinel".to_string()];
            r.don_stat_claim_due(&t.don, &mut records);
            r.gof_claim_due(&t, &mut records, &no_flag);
            r.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
            r.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
            r.feed_leader_magic(&sora);
            let s = |c: PartyChara| r.party.status(r.id(c).unwrap()).unwrap().clone();
            let (d, g) = (s(PartyChara::Donald), s(PartyChara::Goofy));
            (d.max_hp(), d.max_mp(), g.max_hp(), g.max_mp(), g.strength(), d.magic())
        };
        let usual = make(kh_core::status::OpeningChoice::DEFAULT, false);
        assert_eq!(usual, (98, 138, 156, 88, 11, 18));
        for choice in kh_core::status::OpeningChoice::all() {
            assert_eq!(make(choice, false), usual, "{choice:?}");
            assert_eq!(make(choice, true), usual, "{choice:?} with Sora's own rewards");
        }
    }

    /// AN OLD SAVE: Goofy has four HP_UP10 and two old slots in his friend record, Donald's magic steps are recorded as don:.
    /// The first load rebuilds HP, MP and slots from the boss records, once, with nothing counted twice; his gear stays on.
    #[test]
    fn an_old_save_catches_up_once_and_nothing_counts_twice() {
        let t = table();
        let mut record = {
            let mut r = rt();
            r.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
            r.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
            r.party_record()
        };
        record.as_object_mut().unwrap().remove(RECORD_KEY);
        let armour = kh_core::status::tables::EQUIPMENT.iter().find(|r| r.kind == "armour").unwrap().id;
        for f in record["friends"].as_array_mut().unwrap() {
            let bonus = &mut f["bonuses"].as_array_mut().unwrap()[BonusCounts::HP_UP10];
            *bonus = json!(4);
            if f["chara"].as_str().unwrap().eq_ignore_ascii_case("goofy") {
                // Mohg the Omen's old armour slot, Godfrey's old item slot, Nokstella's old accessory slot; a piece in the third
                f["slots"] = json!([4, 3, 2]);
                f["armour"] = json!([null, null, armour]);
            }
        }
        let mut old: Vec<String> = ["boss:margit", "boss:mohg_omen", "boss:godfrey_shade", "boss:dragonkin_nokstella", "boss:tree_sentinel", "don:margit", "don:tree_sentinel", "party:tree_sentinel:goofy"].map(String::from).to_vec();
        let mut r = rt();
        r.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        r.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        r.reset_friends_for_load(0);
        r.load_party_record(&record);
        let gid = r.id(PartyChara::Goofy).unwrap();
        let did = r.id(PartyChara::Donald).unwrap();
        let g = r.party.status(gid).unwrap();
        assert_eq!((g.max_hp(), g.max_mp()), (142, 88), "old bonuses count for nothing");
        assert_eq!((g.item_slots, g.armour_slots, g.accessory_slots, g.armour[2]), (4, 3, 2, Some(armour)), "his old slots and gear stay until the catch-up");
        // the catch-up
        let got = r.gof_claim_due(&t, &mut old, &no_flag);
        assert_eq!(got.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["tree_sentinel", "margit", "dragonkin_nokstella", "godfrey_shade", "mohg_omen"]);
        let stats = r.don_stat_claim_due(&t.don, &mut old);
        assert_eq!(stats.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["tree_sentinel", "dragonkin_nokstella"], "don: records don't hide the new raw steps");
        let g = r.party.status(gid).unwrap();
        // 142 + Tree Sentinel 14 + Mohg the Omen 14; 88 + Nokstella 4; item 3 + 1, armour 2 + 2 (Margit, Mohg), accessory 1 + 1
        assert_eq!((g.max_hp(), g.max_mp(), g.item_slots, g.armour_slots, g.accessory_slots), (170, 92, 4, 4, 2));
        assert_eq!(g.armour[2], Some(armour));
        let d = r.party.status(did).unwrap();
        assert_eq!((d.max_hp(), d.max_mp()), (98, 145), "Tree Sentinel +9, Nokstella +7");
        // saved and loaded: the same numbers, nothing more
        let saved = r.party_record();
        let mut back = rt();
        back.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        back.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        back.reset_friends_for_load(0);
        back.load_party_record(&saved);
        assert!(back.gof_claim_due(&t, &mut old, &no_flag).is_empty() && back.don_stat_claim_due(&t.don, &mut old).is_empty());
        let g2 = back.party.status(back.id(PartyChara::Goofy).unwrap()).unwrap();
        assert_eq!((g2.max_hp(), g2.max_mp(), g2.item_slots, g2.armour_slots, g2.accessory_slots, g2.growth), (170, 92, 4, 4, 2, r.growth_of(PartyChara::Goofy)));
    }

    /// The save loads before and after a friend joins, and a friend's rewards from before his join reach him.
    #[test]
    fn the_record_round_trips_before_and_after_a_join() {
        let t = table();
        let mut r = rt();
        let mut records: Vec<String> = ["boss:margit", "boss:tree_sentinel", "boss:morgott", "boss:malenia", "boss:draconic_tree_sentinel"].map(String::from).to_vec();
        r.gof_claim_due(&t, &mut records, &no_flag);
        r.don_stat_claim_due(&t.don, &mut records);
        // not joined yet: saved and loaded
        let text = r.party_record().to_string();
        let mut back = rt();
        back.load_party_record(&serde_json::from_str(&text).unwrap());
        assert_eq!(back.friend_growth, r.friend_growth);
        back.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        back.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let g = back.party.status(back.id(PartyChara::Goofy).unwrap()).unwrap();
        // Tree Sentinel 14, Morgott 14, Malenia's Deflect past rank 3: 10; Margit's armour slot
        assert_eq!((g.max_hp(), g.armour_slots, g.growth.ranks.rank(RankLine::Deflect), g.growth.ranks.earned(RankLine::PinningGuard)), (180, 3, 3, 1));
        assert_eq!(g.hp, g.max_hp(), "he joins full");
        // joined: saved and loaded onto spawned friends
        let text = back.party_record().to_string();
        let mut again = rt();
        again.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        again.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        again.reset_friends_for_load(0);
        again.load_party_record(&serde_json::from_str(&text).unwrap());
        assert_eq!(again.friend_growth, r.friend_growth);
        let d = again.party.status(again.id(PartyChara::Donald).unwrap()).unwrap();
        assert_eq!(d.growth, r.growth_of(PartyChara::Donald));
        // an odd record is clamped; a missing one is nothing yet
        let (dg, gg) = from_json(&json!({ "donald": { "hp": -9, "item": 200 }, "goofy": { "armour": 9, "ranks": [2, 1] } })).unwrap();
        assert_eq!((dg.hp, dg.item_slots, gg.armour_slots, gg.ranks.steps(RankLine::ShieldReflect)), (0, 3, 3, 1));
        assert_eq!(from_json(&Value::Null), None);
    }

    /// THE FIFTH SLOT survives a save: armour and items of the slot at index 4, through text, onto a spawned friend and onto a
    /// friend who joins later.
    #[test]
    fn the_fifth_slot_round_trips_in_the_save() {
        use kh_core::status::ItemKind;
        let t = table();
        let armour = kh_core::status::tables::EQUIPMENT.iter().find(|r| r.kind == "armour").unwrap().id;
        let accessory = kh_core::status::tables::EQUIPMENT.iter().find(|r| r.kind == "accessory").unwrap().id;
        let mut r = rt();
        r.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        r.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let mut records = all_bosses(&t);
        r.gof_claim_due(&t, &mut records, &no_flag);
        r.don_stat_claim_due(&t.don, &mut records);
        let (gid, did) = (r.id(PartyChara::Goofy).unwrap(), r.id(PartyChara::Donald).unwrap());
        let g = r.party.status_mut(gid).unwrap();
        assert!(g.set_armour(4, Some(armour)), "Goofy's fifth armour slot");
        g.set_item_slot_kh3(4, Some(ItemKind::HiPotion));
        g.set_auto_reload(ItemKind::HiPotion, true);
        let d = r.party.status_mut(did).unwrap();
        assert!(d.set_accessory(4, Some(accessory)), "Donald's fifth accessory slot");
        assert!(!d.set_armour(2, Some(armour)), "Donald's armour stops at 2");
        let saved = r.party_record().to_string();
        let check = |x: &PartyRuntime| {
            let g = x.party.status(x.id(PartyChara::Goofy).unwrap()).unwrap();
            let d = x.party.status(x.id(PartyChara::Donald).unwrap()).unwrap();
            assert_eq!((g.armour[4], g.items[4].item, g.item_reload[4], d.accessories[4]), (Some(armour), Some(ItemKind::HiPotion), true, Some(accessory)));
        };
        // spawned friends
        let mut back = rt();
        back.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        back.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        back.reset_friends_for_load(0);
        back.load_party_record(&serde_json::from_str(&saved).unwrap());
        check(&back);
        // friends who join after the load
        let mut late = rt();
        late.load_party_record(&serde_json::from_str(&saved).unwrap());
        late.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        late.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        check(&late);
        assert_eq!(FriendRecord::of(PartyChara::Goofy, late.party.status(late.id(PartyChara::Goofy).unwrap()).unwrap()).armour.len(), 5);
    }

    /// R10: the first Crucible Knight seen counts, the later copies give nothing, an older save catches up from the flags, and
    /// Sora's side is untouched (no boss: record).
    #[test]
    fn the_group_rule_counts_the_first_copy_once() {
        let t = table();
        let flag_of = |id: &str| t.row(id).unwrap().flag.unwrap().0;
        assert!(t.row("crucible_knight_stormhill").unwrap().reserved && t.row("erdtree_avatar_weeping").unwrap().reserved, "Sora's rows stay reserved");
        let mut r = rt();
        let mut records: Vec<String> = Vec::new();
        assert!(r.gof_claim_due(&t, &mut records, &no_flag).is_empty(), "nothing beaten");
        let first = flag_of("crucible_knight_caelid");
        let got = r.gof_claim_due(&t, &mut records, &|f| f == first);
        assert_eq!(got, vec![("crucible_knights".to_string(), vec![GrowthGrant::Rank(RankLine::PinningGuard, 2)])]);
        assert_eq!(records, vec!["gof:crucible_knights".to_string()]);
        // a second and third copy: nothing more; the Avatars are their own group
        let second = flag_of("crucible_knight_siluria");
        assert!(r.gof_claim_due(&t, &mut records, &|f| f == first || f == second).is_empty());
        assert_eq!(r.growth_of(PartyChara::Goofy).ranks.steps(RankLine::PinningGuard), 1);
        let avatar = flag_of("erdtree_avatar_liurnia_east");
        let got = r.gof_claim_due(&t, &mut records, &|f| f == first || f == avatar);
        assert_eq!(got, vec![("erdtree_avatars".to_string(), vec![GrowthGrant::Rank(RankLine::BullBrace, 2)])]);
        assert!(!records.iter().any(|k| k.starts_with("boss:")), "Sora gets no record of them");
        // an older save with all seven flags: one step per group
        let mut o = rt();
        let mut rec: Vec<String> = Vec::new();
        assert_eq!(o.gof_claim_due(&t, &mut rec, &|_| true).len(), 2, "every flag set, no boss: record: the two groups only");
        assert_eq!((o.growth_of(PartyChara::Goofy).ranks.steps(RankLine::PinningGuard), o.growth_of(PartyChara::Goofy).ranks.steps(RankLine::BullBrace)), (1, 1));
    }

    /// The abilities of the docs' lists stay on the party.* rows: they land, and no bonus token is left there.
    #[test]
    fn the_party_rows_keep_the_abilities_and_no_bonus() {
        let v: Value = serde_json::from_str(kh_host::bossrewards::SHEET).unwrap();
        let rows: Vec<&Value> = v["rows"].as_array().unwrap().iter().filter(|r| r[0].as_str().is_some_and(|i| i.starts_with("party."))).collect();
        let of = |boss: &str, who: &str| -> Vec<String> { rows.iter().find(|r| r[0] == format!("party.{boss}").as_str()).map_or(Vec::new(), |r| r[1][who].as_array().unwrap().iter().map(|t| t.as_str().unwrap().to_string()).collect()) };
        for r in &rows {
            for who in ["donald", "goofy"] {
                for token in r[1][who].as_array().unwrap() {
                    assert!(FriendReward::parse(token.as_str().unwrap()).is_some(), "{token}");
                }
            }
        }
        for (boss, ability) in [("tree_sentinel", "LUCK_UP"), ("loretta_caria", "COMBI_TIME"), ("rennala", "LAST_LEAVE"), ("mimic_tear", "SHARE_PRIZE"), ("godskin_noble", "HYPER_HEALING"), ("mohg_lord", "CRITICAL_HALF"), ("godskin_duo", "COMBO_LEAVE"), ("maliketh", "COMBI_UP")] {
            assert_eq!(of(boss, "goofy"), vec![format!("ability:{ability}")], "{boss}");
        }
        for (boss, ability) in [("tree_sentinel", "ability:MP_HASTE"), ("leonine", "ability:FIRE_UP"), ("smarag", "ability:COMBI_TIME"), ("red_wolf", "don:AERO_FLOAT"), ("radahn", "ability:DAMAGE_ASPIR"), ("elemer", "ability:THUNDER_UP"), ("morgott", "ability:BLIZZARD_UP"), ("fire_giant", "ability:HYPER_HEALING"), ("maliketh", "ability:COMBI_UP"), ("elden_beast", "ability:AERO_UP")] {
            assert_eq!(of(boss, "donald"), vec![ability.to_string()], "{boss}");
        }
        // the doc's AP of Goofy's list
        for (key, ap) in [("LUCK_UP", 5), ("COMBI_TIME", 3), ("LAST_LEAVE", 4), ("SHARE_PRIZE", 4), ("HYPER_HEALING", 3), ("CRITICAL_HALF", 4), ("COMBO_LEAVE", 4), ("COMBI_UP", 5)] {
            assert_eq!(kh_party::tables::ability_cost(key).map(|c| c.0), Some(ap), "{key}");
        }
    }

    #[test]
    fn a_rank_before_he_learns_it_says_ready() {
        let t = table();
        let mut r = rt();
        let mut records = vec!["boss:draconic_tree_sentinel".to_string()];
        let got = r.gof_claim_due(&t, &mut records, &no_flag);
        assert_eq!(got[0].1[0].label(r.goofy_level()), "Pinning Guard: Rank 2 ready");
    }

    /// The test level keeps its loadout: no record reaches it.
    #[test]
    fn the_test_level_keeps_every_reward() {
        let mut r = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        r.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        r.sync_growth();
        let g = r.party.status(r.id(PartyChara::Goofy).unwrap()).unwrap();
        assert_eq!((g.max_hp(), g.max_mp(), g.armour_slots), (376, 136, 5));
    }

    /// (weapons P11) A BOOST ITEM ON ONE FRIEND (the camp menu's Items screen, `PartyMenuEdit::Boost`): only him, his own
    /// stat + 1, kept by his record through the save; before he joins it waits in the record; the test level takes it on
    /// his status; Sora / a bad stat are refused.
    #[test]
    fn p11_a_boost_item_reaches_each_friend_joined_or_not_and_survives_the_save() {
        let mut r = rt();
        r.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        r.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let (gid, did) = (r.id(PartyChara::Goofy).unwrap(), r.id(PartyChara::Donald).unwrap());
        let stats = |r: &PartyRuntime, id| r.party.status(id).map(|s| (s.strength(), s.magic(), s.defense(), s.max_ap())).unwrap();
        let (g0, d0) = (stats(&r, gid), stats(&r, did));
        r.apply_edit(&kh_host::party::PartyMenuEdit::Boost { chara: PartyChara::Goofy, stat: 0 });
        r.apply_edit(&kh_host::party::PartyMenuEdit::Boost { chara: PartyChara::Goofy, stat: 3 });
        r.apply_edit(&kh_host::party::PartyMenuEdit::Boost { chara: PartyChara::Donald, stat: 1 });
        assert_eq!(stats(&r, gid), (g0.0 + 1, g0.1, g0.2, g0.3 + 1), "Goofy: Strength and AP");
        assert_eq!(stats(&r, did), (d0.0, d0.1 + 1, d0.2, d0.3), "Donald: Magic alone (his stats are his own)");
        assert!(!r.apply_boost_item(PartyChara::Sora, 0) && !r.apply_boost_item(PartyChara::Goofy, 4));
        // a friend NOT JOINED yet (the whole party gets a boost): his record keeps it, he joins with it
        let mut later = rt();
        later.apply_edit(&kh_host::party::PartyMenuEdit::Boost { chara: PartyChara::Donald, stat: 2 });
        assert_eq!(later.growth_of(PartyChara::Donald).boosts, [0, 0, 1, 0]);
        let mut fresh = rt();
        fresh.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        later.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let def = |r: &PartyRuntime| r.party.status(r.id(PartyChara::Donald).unwrap()).unwrap().defense();
        assert_eq!(def(&later), def(&fresh) + 1, "he joins with the boost");
        // the save: the record keeps them; loaded onto spawned friends
        let text = r.party_record().to_string();
        let mut back = rt();
        back.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        back.reset_friends_for_load(0);
        back.load_party_record(&serde_json::from_str(&text).unwrap());
        assert_eq!((back.growth_of(PartyChara::Goofy).boosts, back.growth_of(PartyChara::Donald).boosts), ([1, 0, 0, 1], [0, 1, 0, 0]));
        let g = back.party.status(back.id(PartyChara::Goofy).unwrap()).unwrap();
        assert_eq!(g.growth.boosts, [1, 0, 0, 1]);
        // a record without the key (an older save): none
        let (dg, _) = from_json(&json!({ "donald": { "hp": 3 } })).unwrap();
        assert_eq!(dg.boosts, [0; 4]);
        // the test level (no progression): on his status at once
        let mut t = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        t.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        let before = t.party.status(t.id(PartyChara::Goofy).unwrap()).unwrap().defense();
        t.apply_edit(&kh_host::party::PartyMenuEdit::Boost { chara: PartyChara::Goofy, stat: 2 });
        assert_eq!(t.party.status(t.id(PartyChara::Goofy).unwrap()).unwrap().defense(), before + 1);
    }
}
