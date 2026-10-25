//! (IT) THE ROSTER'S PERMANENT STAT BOOSTS, the party host's side (kh-party `boosts` has the total and the getters'
//! hook). The user, 2026-10-17 00:50: "make sure any permenant stat buff items you recieve are given to all possible
//! party members, even if they aren't unlocked yet".
//!
//! - [`PartyRuntime::receive_boost`]: a boost received (whoever leads, whoever is unlocked): the roster's total grows
//!   once, every friend in the party gets it at once (his max HP / MP rise with their gauges), the others when they
//!   spawn or join (`spawn_one` and every step copy the total: [`PartyRuntime::sync_roster`]). Sora takes the same
//!   boost through his own status (the host: his boost bytes / bonus counts, kept by his save).
//! - The total is saved ONCE, in the party record (`roster_boosts`, additive: format 2 / friends_version 5 readers
//!   ignore it). A save without it (older than this) sets `roster_unrecorded`; the host then seeds the total once from
//!   what Sora received ([`PartyRuntime::migrate_roster`]).

use kh_party::boosts::{Boost, RosterBoosts};
use kh_core::status::BonusCounts;
use serde_json::{json, Value};

use crate::runtime::PartyRuntime;
use crate::say;

/// The total as the party record keeps it: `{"strength", "magic", "defense", "ap", "bonuses": [[kind, count], ...]}`.
pub fn to_json(r: &RosterBoosts) -> Value {
    let steps: Vec<Value> = r.bonuses.0.iter().enumerate().filter(|(_, n)| **n > 0).map(|(k, n)| json!([k, n])).collect();
    json!({ "strength": r.strength, "magic": r.magic, "defense": r.defense, "ap": r.ap, "bonuses": steps })
}

/// The record's value (None: not an object, i.e. a save older than the total). Odd values are clamped, kinds that are
/// no max-HP / max-MP step dropped.
pub fn from_json(v: &Value) -> Option<RosterBoosts> {
    if !v.is_object() {
        return None;
    }
    let byte = |k: &str| v[k].as_u64().unwrap_or(0).min(255) as u8;
    let mut out = RosterBoosts { strength: byte("strength"), magic: byte("magic"), defense: byte("defense"), ap: byte("ap"), bonuses: BonusCounts::default() };
    for step in v["bonuses"].as_array().into_iter().flatten() {
        if let (Some(k), Some(n)) = (step[0].as_u64(), step[1].as_u64()) {
            if Boost::is_stat_bonus(k as usize) {
                out.bonuses.0[k as usize] = n.min(255) as u8;
            }
        }
    }
    Some(out)
}

impl PartyRuntime {
    /// A permanent boost received: into the roster's total and onto every friend now. Returns the panel's words
    /// ("Strength +1") or None when it is no stat boost (a slot bonus) or the counter is full.
    pub fn receive_boost(&mut self, b: Boost) -> Option<String> {
        if !self.roster_boosts.add(b) {
            return None;
        }
        self.sync_roster();
        let line = b.line();
        say!("party: roster boost {b:?} ({line}): total {:?}; friends now {}", self.roster_boosts, self.members.iter().filter_map(|(c, id)| self.party.status(*id).map(|s| format!("{} str {} mag {} def {} AP {} HP {} MP {}", c.name(), s.strength(), s.magic(), s.defense(), s.max_ap(), s.max_hp(), s.max_mp()))).collect::<Vec<_>>().join(", "));
        Some(line)
    }

    /// The total onto every spawned friend. A rise of his max HP / MP raises the gauge by as much (KH3's bonus rule
    /// fills it; a boost should never leave him lower than before).
    pub fn sync_roster(&mut self) {
        let total = self.roster_boosts;
        for (_, id) in self.members.clone() {
            if let Some(s) = self.party.status_mut(id) {
                if s.roster == total {
                    continue;
                }
                let (hp0, mp0) = (s.max_hp(), s.max_mp());
                s.roster = total;
                let (hp1, mp1) = (s.max_hp(), s.max_mp());
                if s.hp > 0 {
                    s.hp = (s.hp + (hp1 - hp0).max(0)).min(hp1);
                }
                s.mp = (s.mp + (mp1 - mp0).max(0)).min(mp1);
            }
        }
    }

    /// The party record's value (load): the total, or "not recorded" (an older save) for the host's migration.
    pub fn load_roster_record(&mut self, v: &Value) {
        match from_json(v) {
            Some(r) => {
                self.roster_boosts = r;
                self.roster_unrecorded = false;
            }
            None => {
                self.roster_boosts = RosterBoosts::NONE;
                self.roster_unrecorded = true;
            }
        }
        self.sync_roster();
    }

    /// AN OLDER SAVE (no total in its party record): the total seeded ONCE from what Sora received: his Strength /
    /// Magic / Defense boost bytes (only the boosts write them) and `small_ap` AP Boosts (the host counts them from its
    /// records). What it cannot tell apart is left out (AP or max MP of an Elden Ring item mapped to a stat, merged into
    /// Sora's AP byte / bonus counts with his own rewards). Returns whether it seeded.
    pub fn migrate_roster(&mut self, sora: kh_core::status::Boosts, small_ap: u8) -> bool {
        if !self.roster_unrecorded {
            return false;
        }
        self.roster_unrecorded = false;
        let b = sora;
        self.roster_boosts = RosterBoosts { strength: b.strength, magic: b.magic, defense: b.defense, ap: small_ap, ..RosterBoosts::NONE };
        self.sync_roster();
        say!("party: an older save: the roster's boosts seeded from Sora's: {:?}", self.roster_boosts);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::PartyRuntime;
    use kh_host::party::PartyChara;

    fn new() -> PartyRuntime {
        PartyRuntime::new(crate::RuntimeConfig { wanted: true, test: crate::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None })
    }

    fn rt() -> PartyRuntime {
        let mut rt = new();
        rt.progression = true;
        rt
    }

    /// A boost before Donald is unlocked (nobody spawned): he joins with it; Goofy, in the party, gets it at once.
    #[test]
    fn it_boost_before_join_and_on_the_party() {
        let mut rt = rt();
        rt.spawn_one(PartyChara::Goofy, kh_core::Vec3::ZERO, 0.0);
        let g = rt.id(PartyChara::Goofy).unwrap();
        let before = rt.party.status(g).unwrap().clone();
        assert_eq!(rt.receive_boost(Boost::Strength).as_deref(), Some("Strength +1"));
        rt.receive_boost(Boost::Defense);
        rt.receive_boost(Boost::Magic);
        rt.receive_boost(Boost::Ap(1));
        rt.receive_boost(Boost::Bonus(BonusCounts::MP_UP5));
        assert_eq!(rt.receive_boost(Boost::Bonus(BonusCounts::ACC_SLOT_UP1)), None, "a slot is not a stat");
        let after = rt.party.status(g).unwrap();
        assert_eq!((after.strength() - before.strength(), after.magic() - before.magic(), after.defense() - before.defense(), after.max_ap() - before.max_ap(), after.max_mp() - before.max_mp()), (1, 1, 1, 1, 5));
        assert_eq!(after.mp, before.mp + 5, "the MP gauge rises with its max");
        // Donald was never spawned (locked): he joins with every boost
        let fresh = kh_party::FriendStatus::new(kh_party::Chara::Donald, 0);
        rt.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        let d = rt.party.status(rt.id(PartyChara::Donald).unwrap()).unwrap();
        assert_eq!(d.roster, rt.roster_boosts);
        assert_eq!((d.strength() - fresh.strength(), d.defense() - fresh.defense(), d.max_ap() - fresh.max_ap(), d.max_mp() - fresh.max_mp()), (1, 1, 1, 5));
        assert_eq!(d.mp, d.max_mp(), "he joins full");
    }

    /// Save / reload: the total is saved once and comes back; a reset of the friends (a load) keeps it on them.
    #[test]
    fn it_boosts_survive_save_and_load() {
        let mut rt = rt();
        rt.receive_boost(Boost::Strength);
        rt.receive_boost(Boost::Strength);
        rt.receive_boost(Boost::Ap(2));
        let record = rt.party_record();
        assert_eq!(record[kh_party::boosts::RECORD_KEY]["strength"], 2);
        let mut back = new();
        back.progression = true;
        back.spawn_one(PartyChara::Donald, kh_core::Vec3::ZERO, 0.0);
        back.reset_friends_for_load(0);
        back.load_party_record(&record);
        assert_eq!(back.roster_boosts, rt.roster_boosts);
        assert!(!back.roster_unrecorded);
        let d = back.party.status(back.id(PartyChara::Donald).unwrap()).unwrap();
        assert_eq!((d.roster.strength, d.roster.ap), (2, 2));
        // an older save (no total): flagged, then seeded once from Sora's bytes
        let mut old = record.clone();
        old.as_object_mut().unwrap().remove(kh_party::boosts::RECORD_KEY);
        back.load_party_record(&old);
        assert!(back.roster_unrecorded && back.roster_boosts.is_empty());
        assert_eq!(from_json(&to_json(&rt.roster_boosts)), Some(rt.roster_boosts));
        assert_eq!(from_json(&json!({"strength": 999, "bonuses": [[30, 1], [9, 2]]})).map(|r| (r.strength, r.bonuses.0[9], r.bonuses.0[30])), Some((255, 2, 0)));
        let sora = kh_core::status::Boosts { strength: 3, magic: 2, defense: 2, ap: 32 };
        assert!(back.migrate_roster(sora, 2));
        assert!(!back.migrate_roster(sora, 2), "once");
        assert_eq!((back.roster_boosts.strength, back.roster_boosts.magic, back.roster_boosts.defense, back.roster_boosts.ap), (3, 2, 2, 2));
        assert_eq!(back.party.status(back.id(PartyChara::Donald).unwrap()).unwrap().roster, back.roster_boosts);
    }
}
