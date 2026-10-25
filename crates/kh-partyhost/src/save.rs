//! THE PARTY'S SAVE RECORD (contract ER.6): the shape of save.kh's party record with the additive FRIENDS record
//! (version 5, for stream S6 to serialise in kh-host save.rs). Version 4 (today's `party` field) holds the formation
//! and the AI settings ([`kh_host::formation::party_record`]); v5 adds, per friend, what Elden Ring's real progression
//! changes: weapon, armour, accessories, item slots, abilities owned / equipped, victory bonuses (the friends' level is
//! derived from the shared EXP, so it is not stored).
//!
//! The test level does not write v5 (its friends take the party-test loadout at every spawn); nothing here changes
//! its behaviour: [`PartyRuntime::from_save`] keeps the records until the friend is spawned or applies them to a
//! spawned friend, [`PartyRuntime::to_save`] reads the live statuses (a friend slotted out: his last record).

use kh_core::enums::ETresAbilityKind;
use kh_core::status::ItemKind;
use kh_host::formation::Formation;
use kh_host::party::{FriendAiSettings, PartyChara};
use serde_json::{json, Value};

use crate::runtime::PartyRuntime;

/// The v5 friends record of one friend.
#[derive(Clone, Debug, PartialEq)]
pub struct FriendRecord {
    pub chara: PartyChara,
    /// Weapon key (`WEP_DONALD_00` ...).
    pub weapon: String,
    /// Equipment row ids, one per owned slot (None = empty).
    pub armour: Vec<Option<String>>,
    pub accessories: Vec<Option<String>>,
    /// The four item slots: (item pouch name, still there).
    pub items: Vec<(Option<String>, bool)>,
    pub item_slots: u8,
    pub armour_slots: u8,
    pub accessory_slots: u8,
    /// (game ability name, copies owned, equipped bit per copy).
    pub abilities: Vec<(String, u8, u8)>,
    /// `BonusCounts`: victory-bonus counts by the game's bonus index.
    pub bonuses: [u8; 32],
    /// (GOOFY-GG) Goofy's GUARD as saved: (points, broken). ADDITIVE key `guard` of the v5 friend record (no version
    /// change: a v5 save without it reads as None = full GUARD; an older build ignores the key). Its max is not stored:
    /// it follows the level (sheet party-guard-levels).
    pub guard: Option<(f32, bool)>,
    /// (EQ) Auto-Reload per item slot (the four, as `items`). ADDITIVE key `reload` (no version change: a save without it
    /// reads as all off, the behaviour before; an older build ignores the key).
    pub reload: Vec<bool>,
}

/// The whole party record (v4 + v5).
#[derive(Clone, Debug, PartialEq)]
pub struct PartySave {
    pub formation: Formation,
    /// Every character's AI settings (Sora's entry = AI Sora's).
    pub ai: Vec<(PartyChara, FriendAiSettings)>,
    /// v5: the friends' progression.
    pub friends: Vec<FriendRecord>,
}

pub const FRIENDS_RECORD_VERSION: u32 = 5;

/// (EQ) The Auto-Reload flags as the record keeps them: exactly one per item slot (the four), missing ones off.
fn normal_reload(r: &[bool]) -> Vec<bool> {
    (0..4).map(|k| r.get(k).copied().unwrap_or(false)).collect()
}

impl FriendRecord {
    pub fn of(chara: PartyChara, s: &kh_party::FriendStatus) -> FriendRecord {
        FriendRecord {
            chara,
            weapon: s.weapon.to_string(),
            armour: s.armour.iter().take(s.armour_slots as usize).map(|a| a.map(str::to_string)).collect(),
            accessories: s.accessories.iter().take(s.accessory_slots as usize).map(|a| a.map(str::to_string)).collect(),
            items: s.items.iter().map(|i| (i.item.map(|k| k.name().to_string()), i.present)).collect(),
            item_slots: s.item_slots,
            armour_slots: s.armour_slots,
            accessory_slots: s.accessory_slots,
            abilities: ETresAbilityKind::ALL.iter().filter(|k| s.abilities.owned(**k) > 0).map(|k| (k.name().to_string(), s.abilities.owned(*k), s.abilities.equipped_mask(*k))).chain(s.kit.record() /* (party X3D) KIT_* */).chain(s.goofy_kit.record() /* (party X3G) Goofy's KIT_* */).chain(s.donald.as_ref().map(|d| d.abilities.record()).unwrap_or_default() /* (DON1) Donald's DON_* */).collect(),
            bonuses: s.bonuses.0,
            guard: s.guard.map(|g| (g.value, g.broken)), // (GOOFY-GG)
            reload: normal_reload(&s.item_reload), // (EQ)
        }
    }

    /// The record onto a friend's status (unknown names are skipped, counts clamped to the 4 slots).
    pub fn apply(&self, s: &mut kh_party::FriendStatus) {
        let _ = s.equip_weapon(&self.weapon);
        s.bonuses = kh_core::status::BonusCounts(self.bonuses);
        s.item_slots = self.item_slots.min(4);
        s.armour_slots = self.armour_slots.min(4);
        s.accessory_slots = self.accessory_slots.min(4);
        for slot in 0..4 {
            s.set_armour(slot, self.armour.get(slot).cloned().flatten().as_deref());
            s.set_accessory(slot, self.accessories.get(slot).cloned().flatten().as_deref());
        }
        for (slot, (item, present)) in self.items.iter().enumerate().take(4) {
            let item = item.as_deref().and_then(ItemKind::from_name);
            s.items[slot] = kh_party::ItemSlot { item, present: *present && item.is_some() };
            s.item_reload[slot] = item.is_some() && self.reload.get(slot).copied().unwrap_or(false); // (EQ)
        }
        // (GOOFY-GG) GUARD: the saved points at his level's max (None: full)
        let level = s.level;
        if let Some(g) = s.guard.as_mut() {
            g.set_level(level);
            match self.guard {
                Some((value, broken)) => {
                    g.refill();
                    g.value = value.clamp(0.0, g.max);
                    if broken || g.value <= 0.0 {
                        g.break_now();
                        g.value = value.clamp(0.0, g.max);
                    }
                }
                None => g.refill(),
            }
        }
        for (name, owned, mask) in &self.abilities {
            if let Some(kind) = ETresAbilityKind::from_game_name(name).or_else(|| ETresAbilityKind::ALL.iter().copied().find(|k| k.name() == name)) {
                // (EQ) THE SAVED COPIES, bit by bit (was: their count, which lit the first copies: a second copy worn alone
                // came back as the first)
                s.abilities.set(kind, *owned, 0);
                for copy in 0..(*owned).min(8) {
                    if *mask & (1 << copy) != 0 {
                        s.abilities.set_copy(kind, copy, true);
                    }
                }
            } else {
                if s.donald.as_mut().is_some_and(|d| d.abilities.apply_record(name, *owned, *mask)) /* (DON1) Donald's DON_* */ {
                } else if !s.goofy_kit.apply_record(name, *owned, *mask) /* (party X3G) Goofy's kit abilities */ {
                    s.kit.apply_record(name, *owned, *mask); // (party X3D) Donald's kit abilities (KIT_*)
                }
            }
        }
    }

    /// The v5 JSON shape (stream S6 embeds it as `party.friends[]`).
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "chara": self.chara.name(),
            "weapon": self.weapon,
            "armour": self.armour,
            "accessories": self.accessories,
            "items": self.items.iter().map(|(i, p)| json!([i, p])).collect::<Vec<_>>(),
            "slots": [self.item_slots, self.armour_slots, self.accessory_slots],
            "abilities": self.abilities.iter().map(|(n, o, m)| json!([n, o, m])).collect::<Vec<_>>(),
            "bonuses": self.bonuses.to_vec(),
        });
        if let Some((value, broken)) = self.guard {
            v["guard"] = json!([value, broken]); // (GOOFY-GG) additive
        }
        v["reload"] = json!(normal_reload(&self.reload)); // (EQ) additive: always the four slots
        v
    }

    pub fn from_json(v: &Value) -> Option<FriendRecord> {
        let chara = PartyChara::from_name(v["chara"].as_str()?).filter(|c| *c != PartyChara::Sora)?;
        let strings = |k: &str| v[k].as_array().map(|a| a.iter().map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>()).unwrap_or_default();
        let slot = |i: usize, d: u8| v["slots"].get(i).and_then(Value::as_u64).map_or(d, |x| x.min(4) as u8);
        let mut bonuses = [0u8; 32];
        for (i, b) in v["bonuses"].as_array().into_iter().flatten().enumerate().take(32) {
            bonuses[i] = b.as_u64().unwrap_or(0).min(255) as u8;
        }
        Some(FriendRecord {
            chara,
            weapon: v["weapon"].as_str().unwrap_or("").to_string(),
            armour: strings("armour"),
            accessories: strings("accessories"),
            items: v["items"].as_array().into_iter().flatten().map(|x| (x[0].as_str().map(str::to_string), x[1].as_bool().unwrap_or(false))).collect(),
            item_slots: slot(0, kh_party::status::INIT_ITEM_SLOT_DEFAULT),
            armour_slots: slot(1, kh_party::status::INIT_SLOT_DEFAULT),
            accessory_slots: slot(2, kh_party::status::INIT_SLOT_DEFAULT),
            abilities: v["abilities"].as_array().into_iter().flatten().filter_map(|x| Some((x[0].as_str()?.to_string(), x[1].as_u64()?.min(255) as u8, x[2].as_u64()?.min(255) as u8))).collect(),
            bonuses,
            guard: v["guard"].as_array().and_then(|g| Some((g.first()?.as_f64()? as f32, g.get(1).and_then(Value::as_bool).unwrap_or(false)))), // (GOOFY-GG)
            reload: normal_reload(&v["reload"].as_array().map(|a| a.iter().map(|r| r.as_bool().unwrap_or(false)).collect::<Vec<_>>()).unwrap_or_default()), // (EQ) a missing / short key: off
        })
    }
}

impl PartyRuntime {
    /// The party record as it stands (contract ER.6).
    pub fn to_save(&self) -> PartySave {
        let mut friends = Vec::new();
        for c in PartyChara::FRIENDS {
            if let Some(s) = self.id(c).and_then(|id| self.party.status(id)) {
                friends.push(FriendRecord::of(c, s));
            } else if let Some(r) = self.kept_records.iter().find(|r| r.chara == c) {
                friends.push(r.clone());
            }
        }
        let ai = std::iter::once((PartyChara::Sora, self.lead.sora_settings)).chain(PartyChara::FRIENDS.into_iter().map(|c| (c, self.ai_settings(c)))).collect();
        PartySave { formation: self.lead.current_formation(), ai, friends }
    }

    /// A loaded party record: the formation and the AI settings at the next step (as the HUD's requests), the friends'
    /// records onto the spawned friends now and kept for the others until they spawn.
    pub fn from_save(&mut self, save: &PartySave) {
        self.set_formation(save.formation.clone());
        for (c, s) in &save.ai {
            if *c == PartyChara::Sora {
                self.set_sora_ai_settings(*s);
            } else {
                self.set_ai_settings(*c, *s);
            }
        }
        for r in &save.friends {
            match self.id(r.chara) {
                Some(id) => {
                    if let Some(s) = self.party.status_mut(id) {
                        r.apply(s);
                    }
                }
                None => {
                    self.kept_records.retain(|k| k.chara != r.chara);
                    self.kept_records.push(r.clone());
                    self.pending_records.retain(|k| k.chara != r.chara);
                    self.pending_records.push(r.clone());
                }
            }
        }
    }
}
