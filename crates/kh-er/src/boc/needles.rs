//! (OUTFIT-UNLOCK) BOC'S TWO NEEDLES OPEN SORA'S OUTFITS (pure rules, unit-tested; boc.rs feeds them the game).
//!
//! THE USER (2026-11-05): "when you give Boc his Sewing Needle, it should unlock KH1 and KH2 clothes, when you give Boc his
//! Gold Sewing Needle, then it should unlock the rest of the KH3 costumes"; and "remove any requirements in Elden Ring for
//! giving Boc the gold sewing needle. If you have it, and have already given him the sewing needle, you can give him the gold
//! sewing needle as part of his quest".
//!
//! THE FLAGS are Elden Ring's own, READ in his five talk scripts (t223006000 / t223001100 / t223001105 / t223001400 /
//! t223003115, the same logic in each; re-extracted from the 2.7.2.0 install, byte-identical to the cached copies; state
//! note build/sora_tmp/weapons/OUTFIT-UNLOCK/STATE.md, sheet sheets/port/sora-er-outfit-unlocks.json):
//! - [`SEWING_NEEDLE_GIVEN`] 31159206: group x9, "Give the Sewing Needle" (EventTextForTalk 22231000) -> block 22301100 ->
//!   `SetEventFlag(31159206, 1)` -> `RemoveItemFromPlayer(goods 8161)`. EMEVD common 3959: 3946 + 31159206 -> 3947.
//! - [`GOLD_NEEDLE_GIVEN`] 1036489208: talk-list result 25 with offer flag 1036489207 -> group x31: block 22303100 with
//!   flag 1036489208 set at its end (x76 `SetEventFlag(arg1, 1)`) -> `RemoveItemFromPlayer(goods 8162)`.
//!
//! THE GOLD NEEDLE'S OFFER IN ELDEN RING (x18 state 2: `AddTalkListDataIf(GetEventFlag(1036489207) == 1, 25, 22230003)`;
//! the offer flag is cleared at every build of his talk list (x20) and set by x23 state 5 only when ALL of these hold):
//! 1. his quest state 3947 (EMEVD common 3959) and a talk menu at all (x0 -> x8 -> x18);
//! 2. x27 state 1 reaches x23 only with one of his LATER PLACES met: 1036489213 (Liurnia, m60_36_48 event 1036483700),
//!    1039519209 (Altus, m60_39_51), 11109819 (Leyndell, m11_00);
//! 3. 1036489206: his apology for the demigods' garb is heard (block 22303000, offer 1036489205 = 1039409257 (block 22302300,
//!    "Do you ever...make adjustments to your garb yourself?") AND 1036489211 (x41: the player has held Godfrey's, Radahn's,
//!    Mohg's, Maliketh's or Malenia's armour, protector 460100 / 470100 / 480100 / 760100 / 770100));
//! 4. the player holds goods 8162 (query 47 `ComparePlayerInventoryNumber(3, 8162, .., 1)`);
//! 5. 1036489208 not set.
//!
//! OUR RULE ([`gold_offer`]): the option is offered whenever the player holds the Gold Sewing Needle AND the Sewing Needle
//! was given (31159206) AND the gold one was not (1036489208); 1 .. 3 are ignored for this one option. It is done in the
//! game's own condition evaluation (talkkh/hooks.rs: the talk-script query `CSEzStateTalkEnv::invoke`, query 15 =
//! GetEventFlag, `case 0xf` of FUN_140ea24a0 in the 2.7.1.0 copy: argument 1 = the flag, the event-flag manager's answer as
//! an int): in Boc's talk, the offer flag reads 1 ([`flag_read`]). The other offers that share talk-list result 25
//! ([`SHARED_RESULT_OFFERS`]: x21 routes result 25 by the FIRST of them set, in that order) read 0 while ours is up, so the
//! choice cannot be taken by another of his lines; they come back with his next talk list once the needle is given.
//! Everything else is Elden Ring's: the line, the hand-over, the flag 1036489208 set by his script, the item removed, his
//! later steps.

/// (the user 2026-11-05: "once you have the sewing kit ... it costs 50,000 munny") Elden Ring's Tailoring Tools and Golden
/// Tailoring Tools (GoodsName 8163 / 8188: "Enables armor alterations at sites of grace"): either opens the camp menu's Outfits
/// entry; the munny one change costs.
pub const KIT_GOODS: [u32; 2] = [8163, 8188];
pub const OUTFIT_PRICE: i32 = 50_000;

/// Elden Ring goods: the Sewing Needle, the Gold Sewing Needle (GoodsName 8161 / 8162).
pub const SEWING_NEEDLE: u32 = 8161;
pub const GOLD_SEWING_NEEDLE: u32 = 8162;
/// Elden Ring's "Sewing Needle given to Boc" (t2230NNNNN x9 state 11).
pub const SEWING_NEEDLE_GIVEN: u32 = 31_159_206;
/// Elden Ring's "Gold Sewing Needle given to Boc" (t2230NNNNN x31: x76's flag argument).
pub const GOLD_NEEDLE_GIVEN: u32 = 1_036_489_208;
/// The offer flag of "Give the Gold Sewing Needle" (EventTextForTalk 22230003) in his talk list.
pub const GOLD_OFFER: u32 = 1_036_489_207;
/// The other offer flags of talk-list result 25, in x21's order (22230001 "Don't tell me...", 22230002 "I'm sorry...",
/// 22230004 "Surely she'd be happy for you?", 22230013 "Did you see it?").
pub const SHARED_RESULT_OFFERS: [u32; 4] = [1_039_409_256, 1_036_489_205, 1_036_489_209, 11_109_307];
/// The talk-script query GetEventFlag (`15 <flag> call/1` in the scripts; `case 0xf` of the query function).
pub const QUERY_GET_EVENT_FLAG: i32 = 15;

/// The records of Sora's `.khsave` (once-only list; an older DLL keeps them as they are): the needle seen given.
pub const RECORD_NEEDLE: &str = "boc:needle";
pub const RECORD_GOLD: &str = "boc:gold_needle";

/// Which needle opens an outfit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// KH3's default outfit: from the start.
    Start,
    /// KH1 and KH2: the Sewing Needle.
    Needle,
    /// Every other outfit (Toy Box, Monstropolis, The Caribbean, San Fransokyo, Verum Rex / Quadratum, any later one): the Gold
    /// Sewing Needle.
    Gold,
}

impl Tier {
    /// The record that holds it (None: always open).
    pub const fn record(self) -> Option<&'static str> {
        match self {
            Tier::Start => None,
            Tier::Needle => Some(RECORD_NEEDLE),
            Tier::Gold => Some(RECORD_GOLD),
        }
    }

    /// Elden Ring's flag that opens it.
    pub const fn flag(self) -> Option<u32> {
        match self {
            Tier::Start => None,
            Tier::Needle => Some(SEWING_NEEDLE_GIVEN),
            Tier::Gold => Some(GOLD_NEEDLE_GIVEN),
        }
    }

    /// The needle's goods id (the popup's picture).
    pub const fn goods(self) -> Option<u32> {
        match self {
            Tier::Start => None,
            Tier::Needle => Some(SEWING_NEEDLE),
            Tier::Gold => Some(GOLD_SEWING_NEEDLE),
        }
    }
}

/// The needle of an outfit (`outfits.json` id).
pub fn tier_of(id: &str) -> Tier {
    match id {
        "kh3" => Tier::Start,
        "kh1" | "kh2" => Tier::Needle,
        _ => Tier::Gold,
    }
}

/// The tier is open in these records.
pub fn has_tier(records: &[String], tier: Tier) -> bool {
    tier.record().is_none_or(|r| records.iter().any(|x| x == r))
}

/// The outfit is open in these records.
pub fn is_unlocked(records: &[String], id: &str) -> bool {
    has_tier(records, tier_of(id))
}

/// ADDITIVE: each needle whose flag reads set gets its record when it has none; nothing is ever taken away. The tiers newly
/// opened, in order.
pub fn take(records: &mut Vec<String>, needle_given: bool, gold_given: bool) -> Vec<Tier> {
    let mut opened = Vec::new();
    for (tier, given) in [(Tier::Needle, needle_given), (Tier::Gold, gold_given)] {
        if let Some(r) = tier.record().filter(|_| given && !has_tier(records, tier)) {
            records.push(r.to_string());
            opened.push(tier);
        }
    }
    opened
}

/// THE RULE OF THE GOLD NEEDLE'S OPTION (the user): held, the first needle given, the gold one not.
pub fn gold_offer(has_gold: bool, needle_given: bool, gold_given: bool) -> bool {
    has_gold && needle_given && !gold_given
}

/// A flag the rule may answer for in Boc's talk.
pub fn answers(flag: u32) -> bool {
    flag == GOLD_OFFER || SHARED_RESULT_OFFERS.contains(&flag)
}

/// What Boc's talk script reads for `flag` while the rule says `offer` (None: the game's own answer).
pub fn flag_read(flag: u32, offer: bool) -> Option<bool> {
    if !offer {
        None
    } else if flag == GOLD_OFFER {
        Some(true)
    } else if SHARED_RESULT_OFFERS.contains(&flag) {
        Some(false)
    } else {
        None
    }
}

/// The outfits a tier opens (id, shown name), in the pack's order.
pub fn opened_by(tier: Tier, outfits: &[(String, String)]) -> Vec<&(String, String)> {
    outfits.iter().filter(|(id, _)| tier_of(id) == tier).collect()
}

/// KH3's OBTAINED popup for a tier (name, caption); None: no outfit of the pack is in it.
pub fn popup_text(tier: Tier, outfits: &[(String, String)]) -> Option<(String, String)> {
    let names: Vec<&str> = opened_by(tier, outfits).iter().map(|(_, n)| n.as_str()).collect();
    if names.is_empty() || tier == Tier::Start {
        return None;
    }
    let title = if names.len() == 1 { "New Outfit".to_string() } else { "New Outfits".to_string() };
    Some((title, format!("{}\nBoc can now alter Sora's clothes into {}.", names.join("\n"), if names.len() == 1 { "it" } else { "them" })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> Vec<(String, String)> {
        [("kh3", "KINGDOM HEARTS III"), ("kh2", "KINGDOM HEARTS II"), ("kh1", "KINGDOM HEARTS"), ("toybox", "Toy Box"), ("monstropolis", "Monstropolis"), ("caribbean", "The Caribbean"), ("sanfransokyo", "San Fransokyo"), ("verumrex", "Quadratum")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    fn open(records: &[String]) -> Vec<String> {
        pack().into_iter().filter(|(id, _)| is_unlocked(records, id)).map(|(id, _)| id).collect()
    }

    #[test]
    fn only_kh3_s_outfit_is_open_at_the_start_and_nothing_else_opens_them() {
        let mut records = vec!["boss:margit".to_string(), "boc:met".to_string(), "outfit:toybox".to_string()];
        assert_eq!(open(&records), vec!["kh3"], "a chosen-outfit record, Boc met, a boss: none of it opens an outfit");
        assert!(take(&mut records, false, false).is_empty());
        assert_eq!(open(&records), vec!["kh3"]);
    }

    #[test]
    fn the_sewing_needle_opens_kh1_and_kh2() {
        let mut records = Vec::new();
        assert_eq!(take(&mut records, true, false), vec![Tier::Needle]);
        assert_eq!(open(&records), vec!["kh3", "kh2", "kh1"]);
        assert!(take(&mut records, true, false).is_empty(), "once");
        assert_eq!(records, vec![RECORD_NEEDLE.to_string()]);
    }

    #[test]
    fn the_gold_needle_opens_every_other_outfit() {
        let mut records = vec![RECORD_NEEDLE.to_string()];
        assert_eq!(take(&mut records, true, true), vec![Tier::Gold]);
        assert_eq!(open(&records), pack().into_iter().map(|(id, _)| id).collect::<Vec<_>>());
        // a later outfit of the pack is the gold needle's too
        assert_eq!(tier_of("some_new_world"), Tier::Gold);
    }

    #[test]
    fn the_order_does_not_matter_and_a_loaded_save_opens_both_at_once() {
        // the gold one first (our rule allows it only after the first needle, but a save may say so): both records, either order
        let mut gold_first = Vec::new();
        assert_eq!(take(&mut gold_first, false, true), vec![Tier::Gold]);
        assert_eq!(open(&gold_first), vec!["kh3", "toybox", "monstropolis", "caribbean", "sanfransokyo", "verumrex"]);
        assert_eq!(take(&mut gold_first, true, true), vec![Tier::Needle]);
        let mut loaded = Vec::new();
        assert_eq!(take(&mut loaded, true, true), vec![Tier::Needle, Tier::Gold], "a save with both flags: both at the first poll");
        assert_eq!(open(&gold_first), open(&loaded));
    }

    #[test]
    fn never_taken_away() {
        let mut records = vec![RECORD_NEEDLE.to_string(), RECORD_GOLD.to_string()];
        assert!(take(&mut records, false, false).is_empty());
        assert_eq!(open(&records).len(), 8, "flags read off later (another character's flags, unreadable) take nothing back");
    }

    #[test]
    fn the_gold_option_needs_the_needle_held_the_first_one_given_and_the_gold_one_not() {
        assert!(gold_offer(true, true, false));
        assert!(!gold_offer(false, true, false), "not held");
        assert!(!gold_offer(true, false, false), "the Sewing Needle not given");
        assert!(!gold_offer(true, true, true), "already given");
        assert!(!gold_offer(false, false, true));
    }

    #[test]
    fn the_talk_reads_the_offer_set_and_the_other_result_25_offers_clear_only_while_the_rule_holds() {
        assert_eq!(flag_read(GOLD_OFFER, true), Some(true));
        for f in SHARED_RESULT_OFFERS {
            assert_eq!(flag_read(f, true), Some(false), "{f}: x21 would route result 25 to it first");
            assert_eq!(flag_read(f, false), None);
            assert!(answers(f));
        }
        assert_eq!(flag_read(GOLD_OFFER, false), None, "the rule off: Elden Ring's own answer");
        for f in [SEWING_NEEDLE_GIVEN, GOLD_NEEDLE_GIVEN, 1_036_489_206, 1_036_489_213, 3947, 1_039_519_208] {
            assert_eq!(flag_read(f, true), None, "{f}: never touched (the hand-over and his quest are Elden Ring's)");
            assert!(!answers(f));
        }
    }

    #[test]
    fn the_popup_names_the_outfits() {
        let p = pack();
        assert_eq!(popup_text(Tier::Needle, &p), Some(("New Outfits".to_string(), "KINGDOM HEARTS II\nKINGDOM HEARTS\nBoc can now alter Sora's clothes into them.".to_string())));
        let (_, gold) = popup_text(Tier::Gold, &p).unwrap();
        assert!(gold.starts_with("Toy Box\nMonstropolis\nThe Caribbean\nSan Fransokyo\nQuadratum\n"), "{gold}");
        assert_eq!(popup_text(Tier::Start, &p), None);
        assert_eq!(popup_text(Tier::Gold, &p[..3]), None, "nothing of the tier in the pack: no popup");
    }

    /// The sheet (sheets/port/sora-er-outfit-unlocks.json) says the same flags and goods as the code.
    #[test]
    fn the_sheet_agrees() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-er-outfit-unlocks.json");
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let row = |id: &str| json["rows"].as_array().unwrap().iter().find(|r| r[0] == id).unwrap_or_else(|| panic!("no row {id}"))[1].clone();
        assert_eq!(row("flag.sewing_needle_given"), SEWING_NEEDLE_GIVEN);
        assert_eq!(row("flag.gold_needle_given"), GOLD_NEEDLE_GIVEN);
        assert_eq!(row("flag.gold_offer"), GOLD_OFFER);
        assert_eq!(row("flag.shared_result_offers"), serde_json::json!(SHARED_RESULT_OFFERS));
        assert_eq!(row("goods.sewing_needle"), SEWING_NEEDLE);
        assert_eq!(row("goods.gold_sewing_needle"), GOLD_SEWING_NEEDLE);
        assert_eq!(row("query.get_event_flag"), QUERY_GET_EVENT_FLAG);
        assert_eq!(row("tier.needle"), serde_json::json!(["kh1", "kh2"]));
        assert_eq!(row("tier.start"), serde_json::json!(["kh3"]));
        assert_eq!(row("goods.tailoring_tools"), serde_json::json!(KIT_GOODS));
        assert_eq!(row("rule.price"), OUTFIT_PRICE);
        for r in json["rows"].as_array().unwrap() {
            assert!(r[0].as_str().is_some_and(|id| !id.starts_with("flag.")) || r[6] == "verified", "{}: ref_conf", r[0]);
        }
    }
}
