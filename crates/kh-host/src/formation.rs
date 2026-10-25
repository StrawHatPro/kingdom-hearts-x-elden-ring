//! (party P) THE PARTY FORMATION: which characters are in the active party (at least 1, at most 3) and which of
//! them is the LEADER (the character the pad controls). Engine-free rules shared by every host (the test app and the
//! Elden Ring host): ONLY the save-point menu's "Party Formation" screen edits it (the user: "party formation should ONLY be
//! accessible by save point", "quick switching should be removed"), the save keeps it (`to_save` / `from_save`, versioned).
//!
//! NOT KH3 BEHAVIOUR (the user's request, 2026-10-15: "save points should have a new option called "Party
//! Formation" ... maximum 3 active, minimum 1 ... Indicate which slot is the leader"): KH3 has no party formation
//! screen (the world decides the party); the rules here are the port's.
//!
//! The roster is data ([`ROSTER`]): a new character is one row.

use crate::party::PartyChara;

/// One character the formation can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RosterEntry {
    pub chara: PartyChara,
    /// The save key and script word.
    pub key: &'static str,
    /// The face movie of the status HUD (UI pack) for its portrait.
    pub face: &'static str,
    /// (BN) The colour of its BONUS! / LEVEL UP! windows: a frame label of HudRight's `lvupBonusWin1Col`
    /// (`kh_ui::hud_side::PANEL_COLOURS`; KH3's 0x141661740: Sora "sora", Donald "donald", Goofy "goofy", other
    /// friends "fnpc3" / "fnpc4"). The windows' portrait is the save-load pack's face of `key` (`UI/Icon/MenuFace`).
    pub panel: &'static str,
    /// (BN) It casts the shared magic book (Sora's spells; Donald leading casts them, kh-party leader_magic.rs): a spell
    /// or tier learned shows on its BONUS! panel too, while it is in the party.
    pub casts_magic: bool,
}

/// THE ROSTER, in the order the screen lists it.
pub const ROSTER: [RosterEntry; 3] = [
    RosterEntry { chara: PartyChara::Sora, key: "sora", face: "FacePcExSora01", panel: "sora", casts_magic: true },
    RosterEntry { chara: PartyChara::Donald, key: "donald", face: "FaceNpcExDonald", panel: "donald", casts_magic: true },
    RosterEntry { chara: PartyChara::Goofy, key: "goofy", face: "FaceNpcExGoofy", panel: "goofy", casts_magic: false },
];

pub const MIN_ACTIVE: usize = 1;
pub const MAX_ACTIVE: usize = 3;

/// Why an edit was refused (the screen buzzes and says so).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormationError {
    /// The party would be empty.
    LastMember,
    /// Three are active already.
    Full,
    /// The leader cannot be slotted out: choose another leader first.
    Leader,
    /// The leader must be in the active party.
    NotActive,
    Unknown,
}

impl FormationError {
    /// The line the screen shows.
    pub fn text(self) -> &'static str {
        match self {
            FormationError::LastMember => "At least one party member is needed.",
            FormationError::Full => "The party is full (three members).",
            FormationError::Leader => "The leader cannot leave the party. Choose another leader first.",
            FormationError::NotActive => "Only an active party member can lead.",
            FormationError::Unknown => "That character is not available.",
        }
    }
}

/// The formation: the active members in slot order and the leader (always one of them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formation {
    active: Vec<PartyChara>,
    leader: PartyChara,
}

impl Default for Formation {
    /// The test level's start: Sora leading Donald and Goofy.
    fn default() -> Formation {
        Formation { active: ROSTER.iter().map(|r| r.chara).collect(), leader: PartyChara::Sora }
    }
}

impl Formation {
    pub fn new(active: &[PartyChara], leader: PartyChara) -> Result<Formation, FormationError> {
        let mut f = Formation { active: Vec::new(), leader };
        for c in active {
            if !f.active.contains(c) {
                if f.active.len() >= MAX_ACTIVE {
                    return Err(FormationError::Full);
                }
                f.active.push(*c);
            }
        }
        if f.active.len() < MIN_ACTIVE {
            return Err(FormationError::LastMember);
        }
        if !f.active.contains(&leader) {
            return Err(FormationError::NotActive);
        }
        Ok(f)
    }

    pub fn active(&self) -> &[PartyChara] {
        &self.active
    }

    pub fn leader(&self) -> PartyChara {
        self.leader
    }

    pub fn is_active(&self, c: PartyChara) -> bool {
        self.active.contains(&c)
    }

    /// The slot (1-based, as the screen numbers them) of an active member.
    pub fn slot_of(&self, c: PartyChara) -> Option<usize> {
        self.active.iter().position(|x| *x == c).map(|i| i + 1)
    }

    /// Slot a character in (at the end) or out. The leader cannot leave; the party keeps 1 ..= 3 members.
    pub fn toggle(&mut self, c: PartyChara) -> Result<bool, FormationError> {
        if !ROSTER.iter().any(|r| r.chara == c) {
            return Err(FormationError::Unknown);
        }
        if let Some(i) = self.active.iter().position(|x| *x == c) {
            if c == self.leader {
                return Err(FormationError::Leader);
            }
            if self.active.len() <= MIN_ACTIVE {
                return Err(FormationError::LastMember);
            }
            self.active.remove(i);
            Ok(false)
        } else {
            if self.active.len() >= MAX_ACTIVE {
                return Err(FormationError::Full);
            }
            self.active.push(c);
            Ok(true)
        }
    }

    /// Make an active member the leader.
    pub fn set_leader(&mut self, c: PartyChara) -> Result<(), FormationError> {
        if !self.active.contains(&c) {
            return Err(FormationError::NotActive);
        }
        self.leader = c;
        Ok(())
    }

    /// Move an active member one slot up (-1) or down (+1).
    pub fn shift(&mut self, c: PartyChara, step: i32) -> bool {
        let Some(i) = self.active.iter().position(|x| *x == c) else { return false };
        let j = i as i32 + step;
        if j < 0 || j >= self.active.len() as i32 {
            return false;
        }
        self.active.swap(i, j as usize);
        true
    }

    /// The save record (`party` of the save file, version 1 of this record).
    pub fn to_save(&self) -> serde_json::Value {
        serde_json::json!({
            "format": 1,
            "leader": key_of(self.leader),
            "active": self.active.iter().map(|c| key_of(*c)).collect::<Vec<_>>(),
        })
    }

    /// From the save record; a missing / unreadable record = the default (all three, Sora leading). Unknown
    /// characters are dropped (a roster row removed later), the rules are applied again.
    pub fn from_save(v: &serde_json::Value) -> Formation {
        let active: Vec<PartyChara> = v["active"].as_array().map(|a| a.iter().filter_map(|k| k.as_str().and_then(chara_of)).collect()).unwrap_or_default();
        let leader = v["leader"].as_str().and_then(chara_of);
        match leader {
            Some(l) => Formation::new(&active, l).or_else(|_| Formation::new(&active, active.first().copied().unwrap_or(PartyChara::Sora))).unwrap_or_default(),
            None => Formation::default(),
        }
    }
}

/// (party P) THE PARTY RECORD of a save: the formation and the AI settings of every character who can be run by the AI
/// (Sora too, while a friend leads): record format 2 = format 1 + `ai` { key: [Combat Style, Abilities, Recovery,
/// targets] } (the camp menu's Customize values, `FriendAiSettings`). A format-1 record has no `ai` (the defaults).
pub fn party_record(f: &Formation, ai: &[(PartyChara, crate::party::FriendAiSettings)]) -> serde_json::Value {
    let mut v = f.to_save();
    v["format"] = serde_json::json!(2);
    let mut map = serde_json::Map::new();
    for (c, s) in ai {
        map.insert(key_of(*c).to_string(), serde_json::json!([s.battle, s.ability, s.cure, s.cure_targets]));
    }
    v["ai"] = serde_json::Value::Object(map);
    v
}

/// (party P) The AI settings of a party record (none in a format-1 record or a missing one).
pub fn ai_of_record(v: &serde_json::Value) -> Vec<(PartyChara, crate::party::FriendAiSettings)> {
    let Some(map) = v["ai"].as_object() else { return Vec::new() };
    map.iter()
        .filter_map(|(k, s)| {
            let c = chara_of(k)?;
            let b = |i: usize| s.get(i).and_then(|x| x.as_u64()).map(|x| x.min(255) as u8);
            Some((c, crate::party::FriendAiSettings { battle: b(0)?.min(3), ability: b(1)?.min(2), cure: b(2)?.min(2), cure_targets: b(3)? & 7 }))
        })
        .collect()
}

/// (SV, ERT4's finding 2026-10-17: a Goofy start's first save said `"leader": "sora"` beside `"starter": "goofy"`, so its slot
/// card showed Sora) A SAVE'S PARTY RECORD MADE TRUE TO ITS UNLOCKS: when the record names who is unlocked (`unlocked`, the
/// unlock director's stamp), its formation keeps only unlocked characters; the leader stays when he is one of them, else the
/// starter when he is, else the first one left; nobody left = the unlocked ones (at most three). A record without an
/// `unlocked` list, or whose list names nobody known, is left as it is (a save from before the unlocks: everyone available).
/// Every other field (`format`, `ai`, the friends' records ...) stays. Data-driven by [`ROSTER`]: a new character is covered
/// by his row. True when it changed something.
pub fn fit_to_unlocked(record: &mut serde_json::Value) -> bool {
    let known = |v: &serde_json::Value| -> Vec<PartyChara> {
        let mut out: Vec<PartyChara> = Vec::new();
        for c in v.as_array().into_iter().flatten().filter_map(|k| k.as_str().and_then(chara_of)) {
            if !out.contains(&c) {
                out.push(c);
            }
        }
        out
    };
    let unlocked = known(&record["unlocked"]);
    if unlocked.is_empty() || !record.is_object() {
        return false;
    }
    let active = known(&record["active"]);
    let leader = record["leader"].as_str().and_then(chara_of);
    let starter = record["starter"].as_str().and_then(chara_of);
    let mut keep: Vec<PartyChara> = active.iter().copied().filter(|c| unlocked.contains(c)).collect();
    if keep.is_empty() {
        keep = unlocked.iter().copied().take(3).collect();
    }
    let new_leader = leader.filter(|l| keep.contains(l)).or_else(|| starter.filter(|s| keep.contains(s))).unwrap_or(keep[0]);
    let Ok(f) = Formation::new(&keep, new_leader) else { return false };
    let new_active: Vec<&str> = f.active().iter().map(|c| key_of(*c)).collect();
    let changed = leader != Some(f.leader()) || active != f.active();
    if changed {
        record["leader"] = serde_json::json!(key_of(f.leader()));
        record["active"] = serde_json::json!(new_active);
    }
    changed
}

/// (SV) The leader a save's party record names once it is made true to its unlocks ([`fit_to_unlocked`]); `None`: no
/// record (a save from before the party: Sora).
pub fn record_leader(record: &serde_json::Value) -> Option<PartyChara> {
    let mut r = record.clone();
    fit_to_unlocked(&mut r);
    r["leader"].as_str().and_then(chara_of)
}

pub fn key_of(c: PartyChara) -> &'static str {
    ROSTER.iter().find(|r| r.chara == c).map_or("sora", |r| r.key)
}

/// (BN) The roster row of a character (None: not in the roster).
pub fn entry_of(c: PartyChara) -> Option<&'static RosterEntry> {
    ROSTER.iter().find(|r| r.chara == c)
}

pub fn chara_of(key: &str) -> Option<PartyChara> {
    ROSTER.iter().find(|r| r.key.eq_ignore_ascii_case(key)).map(|r| r.chara)
}

#[cfg(test)]
mod tests {
    use super::*;
    use PartyChara::*;

    #[test]
    fn at_least_one_at_most_three_and_the_leader_stays() {
        let mut f = Formation::default();
        assert_eq!(f.active(), [Sora, Donald, Goofy]);
        assert_eq!(f.toggle(Sora), Err(FormationError::Leader), "the leader cannot be slotted out");
        assert_eq!(f.toggle(Goofy), Ok(false));
        assert_eq!(f.toggle(Donald), Ok(false));
        assert_eq!(f.active(), [Sora]);
        assert_eq!(f.toggle(Sora), Err(FormationError::Leader));
        assert_eq!(f.toggle(Goofy), Ok(true));
        assert_eq!(f.set_leader(Goofy), Ok(()));
        assert_eq!(f.toggle(Sora), Ok(false), "Sora may leave once Goofy leads");
        assert_eq!(f.active(), [Goofy]);
        assert_eq!(f.toggle(Goofy), Err(FormationError::Leader));
        assert_eq!(f.set_leader(Donald), Err(FormationError::NotActive));
        f.toggle(Donald).unwrap();
        f.toggle(Sora).unwrap();
        assert_eq!(f.active().len(), 3);
        assert!(Formation::new(&[Sora, Donald, Goofy, Sora], Sora).is_ok(), "duplicates are one");
        assert_eq!(Formation::new(&[], Sora), Err(FormationError::LastMember));
        assert_eq!(Formation::new(&[Donald], Sora), Err(FormationError::NotActive));
    }

    #[test]
    fn slots_reorder_and_the_record_round_trips() {
        let mut f = Formation::new(&[Sora, Goofy], Sora).unwrap();
        f.set_leader(Goofy).unwrap();
        assert!(f.shift(Goofy, -1));
        assert_eq!(f.slot_of(Goofy), Some(1));
        let back = Formation::from_save(&f.to_save());
        assert_eq!(back, f);
        // an older save without the record, or a broken one: the default
        assert_eq!(Formation::from_save(&serde_json::Value::Null), Formation::default());
        assert_eq!(Formation::from_save(&serde_json::json!({"leader": "riku", "active": ["riku"]})), Formation::default());
        // a leader that is not active falls back to the first active member
        assert_eq!(Formation::from_save(&serde_json::json!({"leader": "sora", "active": ["goofy"]})).leader(), Goofy);
    }

    #[test]
    fn a_save_slot_shows_its_party_leader_and_an_old_save_is_soras() {
        use kh_ui::saveload::Portrait;
        let mut save = crate::save::SoraSave::default();
        assert_eq!(crate::save::slot_summary_of(0, &save).portrait, Portrait::Sora, "no record: Sora");
        save.party = party_record(&Formation::new(&[Sora, Donald], Donald).unwrap(), &[]);
        assert_eq!(crate::save::slot_summary_of(0, &save).portrait, Portrait::Donald);
        save.party = Formation::new(&[Goofy], Goofy).unwrap().to_save();
        assert_eq!(crate::save::slot_summary_of(0, &save).portrait, Portrait::Goofy);
    }

    #[test]
    fn the_party_record_keeps_the_ai_settings_and_reads_format_1() {
        use crate::party::FriendAiSettings as S;
        let f = Formation::new(&[Donald, Sora], Donald).unwrap();
        let ai = vec![(Sora, S { battle: 2, ability: 1, cure: 0, cure_targets: 5 }), (Goofy, S { battle: 0, ability: 0, cure: 1, cure_targets: 7 })];
        let rec = party_record(&f, &ai);
        assert_eq!(rec["format"], 2);
        assert_eq!(Formation::from_save(&rec), f, "a format-2 record is a formation record too");
        let mut back = ai_of_record(&rec);
        back.sort_by_key(|(c, _)| *c as u8);
        assert_eq!(back, ai);
        assert!(ai_of_record(&f.to_save()).is_empty(), "format 1: no AI settings");
        assert!(ai_of_record(&serde_json::Value::Null).is_empty());
    }
}

#[cfg(test)]
mod sv_tests {
    use super::*;
    use serde_json::json;
    use PartyChara::*;

    /// (SV, ERT4) A save's party record is made true to its unlocks: the title-time record of a Goofy start names Goofy.
    #[test]
    fn a_record_follows_its_unlocks_and_keeps_everything_else() {
        // the title-time file of a Goofy start (the HUD's default formation + the unlock stamp)
        let mut r = json!({ "format": 1, "leader": "sora", "active": ["sora", "donald", "goofy"], "unlocked": ["goofy"], "starter": "goofy" });
        assert!(fit_to_unlocked(&mut r));
        assert_eq!((r["leader"].clone(), r["active"].clone()), (json!("goofy"), json!(["goofy"])));
        assert_eq!((r["format"].clone(), r["starter"].clone()), (json!(1), json!("goofy")));
        assert_eq!(record_leader(&json!({ "leader": "sora", "active": ["sora"], "unlocked": ["donald"], "starter": "donald" })), Some(Donald));
        // a true record is left alone (and its other fields with it)
        let mut ok = json!({ "format": 2, "leader": "goofy", "active": ["sora", "goofy"], "unlocked": ["sora", "goofy"], "starter": "goofy", "ai": { "sora": [1, 1, 1, 1] }, "zero_exp": true });
        let before = ok.clone();
        assert!(!fit_to_unlocked(&mut ok));
        assert_eq!(ok, before);
        // a locked member leaves; the leader stays when unlocked
        let mut r = json!({ "leader": "donald", "active": ["sora", "donald", "goofy"], "unlocked": ["donald", "goofy"], "starter": "donald" });
        assert!(fit_to_unlocked(&mut r));
        assert_eq!((r["leader"].clone(), r["active"].clone()), (json!("donald"), json!(["donald", "goofy"])));
        // the leader locked: the starter when he is in, else the first left
        let mut r = json!({ "leader": "sora", "active": ["sora", "donald", "goofy"], "unlocked": ["donald", "goofy"], "starter": "goofy" });
        fit_to_unlocked(&mut r);
        assert_eq!(r["leader"], json!("goofy"));
        let mut r = json!({ "leader": "sora", "active": ["sora", "donald"], "unlocked": ["donald", "goofy"], "starter": "goofy" });
        fit_to_unlocked(&mut r);
        assert_eq!((r["leader"].clone(), r["active"].clone()), (json!("donald"), json!(["donald"])));
        // nobody of the formation unlocked: the unlocked ones
        let mut r = json!({ "leader": "sora", "active": ["sora"], "unlocked": ["goofy", "donald"] });
        fit_to_unlocked(&mut r);
        assert_eq!(Formation::from_save(&r).active().len(), 2);
        // no unlock list (a save from before the unlocks), an unknown name only, no record: unchanged
        for mut r in [json!({ "leader": "sora", "active": ["sora", "goofy"] }), json!({ "leader": "sora", "unlocked": ["riku"] }), serde_json::Value::Null] {
            let before = r.clone();
            assert!(!fit_to_unlocked(&mut r));
            assert_eq!(r, before);
        }
        assert_eq!(record_leader(&serde_json::Value::Null), None);
        // every roster row is a possible leader (a new character is one row)
        for row in ROSTER {
            let key = row.key;
            assert_eq!(record_leader(&json!({ "leader": "sora", "active": ["sora"], "unlocked": [key], "starter": key })), Some(row.chara));
        }
    }
}
