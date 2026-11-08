//! (EQ) A FRIEND'S CAMP-MENU EDITS WORK EXACTLY LIKE SORA'S (the user, 2026-10-20: "equipping items/abilities/shortcuts
//! on a party leader that is not Sora is very glitchy as well, that all needs to be fixed to work exactly like Sora
//! does"). One path for every party member, driven by the member's own snapshot (`PartyMenuMember`), no names:
//! - item slots: Sora's pouch rules on a friend's slots: Auto-Reload (Triangle on the row, every slot of that item),
//!   a used auto-reload slot shown greyed with its item ("used, waiting for refill"), refilled from the shared stock
//!   when the menu closes (Sora's `on_unpause`), a used slot without it emptied; the new item takes the setting of the
//!   other slots holding it (kh-core command/customize.rs `equip_item` / `toggle_auto_reload`, magic/items.rs);
//! - the weapon family a friend may wear: the family of the weapon he wears (`WEP_DONALD_03` -> `WEP_DONALD`), not a
//!   table of names;
//! - the Items screen's benefit test: Sora's rule (`would_benefit`) on the member's own gauges (a knocked-out member
//!   gets nothing, as Sora).
//!   A child module of `hud` (it reads the Hud's fields), as hud_x2.rs.
//!
//! (weapons P11, documentation/KH3_WEAPONS_PLAN.md 1.4) KH3'S EQUIPMENT SCREEN FOR THE KEYBLADES AND THE BOOST ITEMS:
//! - [`Hud::eq_sora_finish`]: Sora's three weapon rows are the core's Keyblade slots (`Status::keyblades`, P2; the slot in
//!   hand marked current), each Keyblade at its Forge level (KH3's upgraded row: Strength / Magic / abilities of the
//!   level, its shotlocks by KH3's names); the stock offers every owned Keyblade (P1's record `crate::weapons`, and any in
//!   the camp stock) that is in no slot, one of each, in KH3's item order;
//! - [`Hud::eq_keyblade_equip`]: the commit: the core's slot (`Status::set_keyblade_slot`, KH3's rules: the Keyblade in
//!   hand cannot be removed), the camp stock (the new one leaves it, the old one comes back), P1's record
//!   (`weapons::set_slots`, every Keyblade held owned);
//! - every piece's ability list carries the abilities' help lines, with kh-core `keyblade::help_note` ("(no effect in
//!   this world)": Harvester, Chef Extraordinaire) after KH3's text; a friend's weapon abilities by KH3's names;
//! - [`Hud::eq_items_finish`] / [`Hud::eq_use_boost`]: the four boost items are usable from Items on ONE member chosen
//!   with the target buttons (KH3: Strength / Magic / Defense / AP + 1 for good; Sora's boost bytes, a friend's own
//!   record through `PartyMenuEdit::Boost`).

use super::{item_piece, Hud};
use crate::party::{PartyMenuEdit, PartyMenuMember};
use kh_core::status::{ItemKind, KeybladeId};
use kh_core::Sora;
use kh_ui::camp_equip::{boost_stat, with_note, EquipIn, EquipSlot, InvEntry, Piece, StockEntry, KIND_ITEM, KIND_WEAPON};

/// (weapons P11) KH3'S SHOTLOCK NAMES: a Keyblade's `m_ShootflowAbilitys` (`ETresTextAbilityKind`, P2's sheet column
/// `shotlocks`) -> the ability row's `m_NameTextID` (sheets/entities/Load/Tres/TresAbilityData.json; the test
/// `shotlock_names_are_the_ability_rows` checks every pair against it).
const SHOTLOCK_NAMES: [(&str, &str); 32] = [
    ("TEXT_SHOOTFLOW_KB00_00", "kh3_system.command.sf_magic_0_1"),
    ("TEXT_SHOOTFLOW_KB00_01", "kh3_system.command.sf_ragnarok_0"),
    ("TEXT_SHOOTFLOW_KB01_00", "kh3_system.command.sf_magic_1"),
    ("TEXT_SHOOTFLOW_KB01_01", "kh3_system.command.sf_drop_0"),
    ("TEXT_SHOOTFLOW_KB01_02", "kh3_system.command.sf_rocket_0"),
    ("TEXT_SHOOTFLOW_KB02_00", "kh3_system.command.sf_drain_0"),
    ("TEXT_SHOOTFLOW_KB02_01", "kh3_system.command.sf_pellet_0"),
    ("TEXT_SHOOTFLOW_KB03_00", "kh3_system.command.sf_otherself_0"),
    ("TEXT_SHOOTFLOW_KB03_01", "kh3_system.command.sf_rush_0"),
    ("TEXT_SHOOTFLOW_KB04_00", "kh3_system.command.sf_drain_0_1"),
    ("TEXT_SHOOTFLOW_KB04_01", "kh3_system.command.sf_laser_0"),
    ("TEXT_SHOOTFLOW_KB05_00", "kh3_system.command.sf_scalpel_0"),
    ("TEXT_SHOOTFLOW_KB05_01", "kh3_system.command.sf_vacuum_0"),
    ("TEXT_SHOOTFLOW_KB06_00", "kh3_system.command.sf_scalpel_0_1"),
    ("TEXT_SHOOTFLOW_KB06_01", "kh3_system.command.sf_vacuum_0_1"),
    ("TEXT_SHOOTFLOW_KB07_00", "kh3_system.command.sf_magic_0_2"),
    ("TEXT_SHOOTFLOW_KB07_01", "kh3_system.command.sf_drop_0_1"),
    ("TEXT_SHOOTFLOW_KB07_02", "kh3_system.command.sf_rocket_0_1"),
    ("TEXT_SHOOTFLOW_KB08_00", "kh3_system.command.sf_magic_1_1"),
    ("TEXT_SHOOTFLOW_KB08_01", "kh3_system.command.sf_gate_0_1"),
    ("TEXT_SHOOTFLOW_KB09_00", "kh3_system.command.sf_magic_0"),
    ("TEXT_SHOOTFLOW_KB09_01", "kh3_system.command.sf_gate_0"),
    ("TEXT_SHOOTFLOW_KB10_00", "kh3_system.command.sf_otherself_0_1"),
    ("TEXT_SHOOTFLOW_KB10_01", "kh3_system.command.sf_rush_0_1"),
    ("TEXT_SHOOTFLOW_KB11_00", "kh3_system.command.sf_magic_1_2"),
    ("TEXT_SHOOTFLOW_KB11_01", "kh3_system.command.sf_pellet_0_1"),
    ("TEXT_SHOOTFLOW_KB12_00", "kh3_system.command.sf_magic_1_3"),
    ("TEXT_SHOOTFLOW_KB12_01", "kh3_system.command.sf_dual"),
    ("TEXT_SHOOTFLOW_KB13_00", "kh3_system.command.sf_scalpel_0_3"),
    ("TEXT_SHOOTFLOW_KB14_00", "kh3_system.command.sf_ultima_0"),
    ("TEXT_SHOOTFLOW_KB17_00", "kh3_system.command.sf_scalpel_0_2"),
    ("TEXT_SHOOTFLOW_KB17_01", "kh3_system.command.sf_ragnarok_0_1"),
];

/// (weapons P11) The text key of a shotlock's KH3 name.
pub(super) fn shotlock_name_key(shotlock: &str) -> Option<&'static str> {
    SHOTLOCK_NAMES.iter().find(|(k, _)| *k == shotlock).map(|(_, v)| *v)
}

impl Hud {
    /// (weapons P11, ROUND D9: "Weapon rows 2 and 3 are EMPTY although the khsave has spare_weapons") THE SAVE'S KEYBLADE
    /// SLOTS INTO THE CORE, once a save is applied (after `SoraSave::apply` and `run_weapons`; no `self`: a host calls
    /// `kh_host::hud::Hud::tie_keyblade_slots` beside `tie_keyblade_levels`): the record's slots when set
    /// (`weapons::slots`, save v6), else the save's older fields (slot 1 = the weapon in hand, slots 2 / 3 = `spare`, the
    /// `spare_weapons` of a v5 file), and the record's slot in hand. `SoraSave::status` fills only the weapon in hand, so
    /// without this the core's slots 2 / 3 stayed empty: the Equipment rows read empty, the D-pad had nothing to switch
    /// to, and the stock offered those Keyblades as unslotted. Levels are kept. A status whose weapon is no Keyblade (a
    /// test's other weapon) is left alone.
    pub fn tie_keyblade_slots(status: &mut kh_core::status::Status, spare: &[Option<String>; 2]) {
        if !status.weapon.is_empty() && status.current_keyblade().is_none() {
            return;
        }
        let (slots, current) = crate::weapons::with(|w| (w.slots_or(&status.weapon, spare), w.current));
        let keys: Vec<Option<&str>> = slots.iter().map(Option::as_deref).collect();
        let levels = status.keyblades().levels_by_key();
        let (k, _unknown) = kh_core::status::Keyblades::from_keys(&keys, current as usize, &levels);
        if k.slots() != status.keyblades().slots() || k.current_slot() != status.keyblades().current_slot() {
            status.set_keyblades(k);
        }
    }

    /// (weapons P11, ROUND D9) THE CORE'S KEYBLADE SLOTS INTO THE RECORD before a save is captured (before
    /// `take_running_weapons`; `kh_host::hud::Hud::keyblade_slots_into_record`): a D-pad switch or a test aid changes the
    /// core's slots without the record. Written only when they differ from what the record already says (its slots, else
    /// the older fields `weapon` / `spare`), so a save whose slots are still the older fields' stays a version-5 file.
    /// Every Keyblade in a slot is owned. A core that holds ONLY the weapon in hand while the record says more was never
    /// tied ([`Hud::tie_keyblade_slots`] not run: a host without that line): the record wins then, nothing is written
    /// (the bug of ROUND D9 must not reach the save).
    pub fn keyblade_slots_into_record(status: &kh_core::status::Status, spare: &[Option<String>; 2]) {
        if status.current_keyblade().is_none() {
            return;
        }
        let k = status.keyblades();
        let now = k.slot_keys().map(|s| s.map(str::to_string));
        let current = k.current_slot() as u8;
        let (said, said_current) = crate::weapons::with(|w| (w.slots_or(&status.weapon, spare), w.current));
        let filled = |s: &[Option<String>; 3]| s.iter().flatten().count();
        if (now == said && current == said_current) || (filled(&now) == 1 && filled(&said) > 1) {
            return;
        }
        crate::weapons::with(|w| *w = w.with_held(&status.weapon, status.weapon_upgrade, &now, &[]));
        crate::weapons::set_slots(now, current);
    }
}

/// (weapons P11) The Keyblades the camp menu may offer: every owned one (P1's record) and any Keyblade key in the camp
/// stock (a test's stock, a save older than the record), once each, in KH3's item order (the sheet's).
pub(super) fn owned_keyblades(stock: &[(String, u32)]) -> Vec<KeybladeId> {
    let mut out: Vec<KeybladeId> = crate::weapons::keyblades().iter().filter_map(|k| KeybladeId::from_key(&k.key)).chain(stock.iter().filter(|e| e.1 > 0).filter_map(|e| KeybladeId::from_key(&e.0))).collect();
    out.sort();
    out.dedup();
    out
}

/// The weapon family of a worn weapon key: the key without its last `_NN` part (`WEP_GOOFY_07` -> `WEP_GOOFY`).
pub(super) fn weapon_family(weapon: &str) -> &str {
    match weapon.rfind('_') {
        Some(i) if weapon[i + 1..].chars().all(|c| c.is_ascii_digit()) && i > 0 => &weapon[..i],
        _ => weapon,
    }
}

/// The Equipment rows of a member's item slots (Sora's: piece, Auto-Reload, used).
pub(super) fn item_rows(m: &PartyMenuMember) -> Vec<EquipSlot> {
    m.items
        .iter()
        .enumerate()
        .map(|(slot, item)| {
            let spent = m.item_spent.get(slot).copied().flatten();
            EquipSlot { kind: KIND_ITEM, slot, piece: item.or(spent).map(item_piece), auto_reload: m.item_reload.get(slot).copied().unwrap_or(false), used: item.is_none() && spent.is_some(), current: false }
        })
        .collect()
}

/// Sora's Items-screen benefit test (kh-core customize `would_benefit`) on a member's own gauges; a member has no
/// Focus gauge of Sora's (Refocusers give nothing, Elixir / Megalixir / Tent are HP or MP).
pub(super) fn benefit(m: &PartyMenuMember, item: ItemKind) -> bool {
    use ItemKind as I;
    if m.hp <= 0 {
        return false;
    }
    let (hp, mp) = (m.hp < m.hp_max, m.mp < m.mp_max);
    match item {
        I::Potion | I::HiPotion | I::MegaPotion => hp,
        I::Ether | I::HiEther | I::MegaEther => mp,
        I::Elixir | I::Megalixir | I::Tent => hp || mp,
        I::Refocuser | I::HiRefocuser | I::Panacea => false,
    }
}

/// The member's slot write with Sora's rules (the host's `FriendStatus::set_item_slot_kh3` does the same).
fn write_slot(m: &mut PartyMenuMember, slot: usize, new: Option<ItemKind>) {
    let n = m.items.len();
    m.item_reload.resize(n, false);
    m.item_spent.resize(n, None);
    if slot >= n {
        return;
    }
    let held = m.items[slot].or(m.item_spent[slot]);
    let reload = match new {
        Some(item) if held == Some(item) => m.item_reload[slot],
        Some(item) => (0..n).any(|k| k != slot && m.items[k].or(m.item_spent[k]) == Some(item) && m.item_reload[k]),
        None => false,
    };
    m.items[slot] = new;
    m.item_spent[slot] = None;
    m.item_reload[slot] = reload;
}

impl Hud {
    /// The local copy of a friend's item-slot equip (the host's answer replaces it), Sora's rules.
    pub(super) fn eq_item_slot(&mut self, index: usize, slot: usize, new: Option<ItemKind>) {
        if let Some(m) = self.party_menu.get_mut(index) {
            write_slot(m, slot, new);
        }
    }

    /// Triangle on one of a friend's item rows: Auto-Reload of that item on every slot holding it (a used one emptied
    /// when it goes off), at once in the local copy, the edit to the host.
    pub(super) fn eq_auto_reload(&mut self, index: usize, item: &str, on: bool) {
        let Some(item) = ItemKind::from_name(item) else { return };
        let Some(m) = self.party_menu.get_mut(index) else { return };
        let n = m.items.len();
        m.item_reload.resize(n, false);
        m.item_spent.resize(n, None);
        let mut any = false;
        for k in 0..n {
            if m.items[k].or(m.item_spent[k]) == Some(item) {
                any = true;
                m.item_reload[k] = on;
                if !on && m.item_spent[k].is_some() {
                    m.item_spent[k] = None;
                }
            }
        }
        let chara = m.chara;
        (self.log)(&format!("camp: {} auto-reload of {} {}{}", chara.name(), item.name(), if on { "on" } else { "off" }, if any { "" } else { " (no slot holds it)" }));
        if any {
            self.party_edits.push(PartyMenuEdit::AutoReload { chara, item, on });
        }
    }

    /// THE MENU CLOSED (Sora's `on_unpause`): every friend's used auto-reload slot takes one of its item from the shared
    /// stock and is whole again (the edit to the host: the same item written back keeps the slot's Auto-Reload).
    pub(super) fn eq_refill_friends(&mut self, sora: &mut Sora) -> u32 {
        kh_core::command::customize::ensure(sora);
        let mut n = 0;
        for index in 0..self.party_menu.len() {
            for slot in 0..self.party_menu[index].items.len() {
                let m = &self.party_menu[index];
                let Some(item) = m.item_spent.get(slot).copied().flatten() else { continue };
                if !m.item_reload.get(slot).copied().unwrap_or(false) {
                    continue;
                }
                let Some(entry) = sora.magic_mut().pouch.stock.iter_mut().find(|s| s.0 == item && s.1 > 0) else { continue };
                entry.1 -= 1;
                let chara = m.chara;
                write_slot(&mut self.party_menu[index], slot, Some(item));
                self.party_edits.push(PartyMenuEdit::ItemSlot { chara, slot, item: Some(item) });
                (self.log)(&format!("camp: {} item slot {slot} refilled with {} from the stock (auto-reload)", chara.name(), item.name()));
                n += 1;
            }
        }
        if n > 0 {
            self.party_menu_dirty = true;
        }
        n
    }

    /// The items a friend leader's shortcut list offers (Sora's rule, kh-core customize `assignable`): his slots' items
    /// (used auto-reload ones too: the slot remembers them), then the battle items of the shared stock.
    pub(super) fn eq_leader_items(&self, sora: &Sora, m: &PartyMenuMember) -> Vec<ItemKind> {
        let mut out: Vec<ItemKind> = m.items.iter().zip(m.item_spent.iter().chain(std::iter::repeat(&None))).filter_map(|(i, s)| i.or(*s)).collect();
        for (item, count) in Self::pouch(sora).stock {
            if count > 0 && !out.contains(&item) {
                out.push(item);
            }
        }
        out
    }
}

// ------------------------------------------------------------------------------------------------ (weapons P11)
impl Hud {
    /// An ability of a piece by its game name (`PRIZE_DRAW`) or its English name: (the name shown, its help line). The
    /// help is the pack's KH3 text (`m_HelpTextID`), else the sheet's line, then kh-core's note where the ability has no
    /// effect in this world (`keyblade::help_note`). An unknown name is shown as it is, without help.
    pub(super) fn eq_ability(&self, name: &str) -> (String, String) {
        let Some(row) = kh_core::status::tables::ABILITIES.iter().find(|a| a.id == name || a.name == name) else { return (name.to_string(), String::new()) };
        let help = self.ability_data.get(&(row.kind.value() as i64)).map_or_else(|| if row.help_text == "-" { String::new() } else { row.help_text.to_string() }, |e| e.1.clone());
        (row.name.to_string(), with_note(&help, kh_core::status::keyblade::help_note(row.kind)))
    }

    /// A KEYBLADE AS AN EQUIPMENT PIECE at its Forge level `level` (KH3's upgraded row, `0x14112EBD0`): Strength / Magic =
    /// its base + every done level (P2's `KeybladeId::stats`), its abilities (its own, then each done level's), its
    /// shotlocks by KH3's names. The help text is the pack's (`kh3_item.weapon.detail_kb_NN` by the item key).
    pub(super) fn eq_keyblade_piece(&self, id: KeybladeId, level: u8) -> Piece {
        let level = id.clamp_level(level);
        let stats = id.stats(level);
        let (abilities, ability_help): (Vec<String>, Vec<String>) = id.ability_names(level).into_iter().map(|a| self.eq_ability(a)).unzip();
        let shotlocks = id.shotlocks().iter().map(|s| shotlock_name_key(s).map_or_else(|| s.to_string(), |key| self.camp_view.string(key))).collect();
        Piece { id: id.key().to_string(), name: id.name().to_string(), kind: KIND_WEAPON, attack: stats.strength, magic: stats.magic, abilities, ability_help, level: Some(level), shotlocks, ..Piece::default() }
    }

    /// Every piece's ability names by KH3's English names (a friend's weapon rows carry game names) and their help lines.
    fn eq_ability_lines(&self, piece: &mut Piece) {
        if piece.ability_help.len() == piece.abilities.len() && !piece.abilities.is_empty() {
            return;
        }
        let (names, help): (Vec<String>, Vec<String>) = piece.abilities.iter().map(|a| self.eq_ability(a)).unzip();
        piece.abilities = names;
        piece.ability_help = help;
    }

    fn eq_all_ability_lines(&self, data: &mut EquipIn) {
        let mut pieces: Vec<&mut Piece> = data.slots.iter_mut().filter_map(|s| s.piece.as_mut()).collect();
        pieces.extend(data.stock.iter_mut().map(|e| &mut e.piece));
        for p in pieces {
            self.eq_ability_lines(p);
        }
    }

    /// SORA'S EQUIPMENT SCREEN, FINISHED (the host's `equip_in` hands its rows here): the weapon rows become the core's
    /// three Keyblade slots, the Keyblades of the stock become every owned Keyblade in no slot (count 1: KH3 has one of
    /// each), every piece gets its ability help lines. A status whose weapon is no Keyblade (a test's other weapon)
    /// keeps the rows it was given.
    pub fn eq_sora_finish(&self, sora: &Sora, mut data: EquipIn) -> EquipIn {
        let status = sora.status();
        if status.weapon.is_empty() || status.current_keyblade().is_some() {
            let k = status.keyblades();
            let rows: Vec<EquipSlot> = (0..kh_core::status::keyblade::SLOTS).map(|slot| EquipSlot { kind: KIND_WEAPON, slot, piece: k.slot(slot).map(|id| self.eq_keyblade_piece(id, k.level(id))), current: k.current().is_some() && slot == k.current_slot(), ..EquipSlot::default() }).collect();
            let at = data.slots.iter().position(|s| s.kind == KIND_WEAPON).unwrap_or(0);
            data.slots.retain(|s| s.kind != KIND_WEAPON);
            data.slots.splice(at.min(data.slots.len())..at.min(data.slots.len()), rows);
            data.stock.retain(|e| KeybladeId::from_key(&e.piece.id).is_none());
            let free: Vec<StockEntry> = owned_keyblades(&self.inventory.equipment).into_iter().filter(|id| k.slot_of(*id).is_none()).map(|id| StockEntry { piece: self.eq_keyblade_piece(id, k.level(id)), count: 1 }).collect();
            data.stock.splice(0..0, free);
        }
        self.eq_all_ability_lines(&mut data);
        data
    }

    /// A FRIEND'S EQUIPMENT SCREEN, FINISHED (the host's `friend_equip_in` hands its rows here): his weapons are his own
    /// rows (party-weapons stats, never Sora's; duplicates allowed, the stock's count), their abilities by KH3's names
    /// with their help lines.
    pub fn eq_friend_finish(&self, mut data: EquipIn) -> EquipIn {
        self.eq_all_ability_lines(&mut data);
        data
    }

    /// THE KEYBLADE SLOT COMMIT (KH3's weapon-slot commit `0x140E98060`, the stock moves of `0x1415ECC00`): `id` (a
    /// Keyblade key; `None` = Remove) into Sora's slot `slot`. True when the action was a Keyblade slot's (done or
    /// refused: nothing else is to be done with it); false for anything else (a status whose weapon is no Keyblade, a
    /// weapon key that is no Keyblade): the caller's older path.
    pub fn eq_keyblade_equip(&mut self, sora: &mut Sora, slot: usize, id: Option<&str>) -> bool {
        let status = sora.status();
        if !status.weapon.is_empty() && status.current_keyblade().is_none() {
            return false;
        }
        let new = match id {
            Some(key) => match KeybladeId::from_key(key) {
                Some(k) => Some(k),
                None => return false,
            },
            None => None,
        };
        if let Some(k) = new.filter(|k| !owned_keyblades(&self.inventory.equipment).contains(k)) {
            (self.log)(&format!("camp: Keyblade slot {slot} = {}: REFUSED (not owned)", k.key()));
            return true;
        }
        let old = status.keyblades().slot(slot);
        if let Err(refusal) = sora.status_mut().set_keyblade_slot(slot, new) {
            (self.log)(&format!("camp: Keyblade slot {slot} = {}: refused ({refusal})", id.unwrap_or("-")));
            return true;
        }
        let status = sora.status();
        let k = *status.keyblades();
        // the camp stock: the Keyblade put in a slot leaves it, the one taken out comes back (one of each)
        if let Some(n) = new {
            self.inventory.equipment.retain(|e| e.0 != n.key());
        }
        if let Some(o) = old.filter(|o| k.slot_of(*o).is_none() && !self.inventory.equipment.iter().any(|e| e.0 == o.key() && e.1 > 0)) {
            self.inventory.add(o.key(), 1);
        }
        // the legacy fields as an older reader understands them (the weapon in hand + the other two slots) and P1's record
        let others: Vec<Option<String>> = (0..kh_core::status::keyblade::SLOTS).filter(|s| *s != k.current_slot()).map(|s| k.slot(s).map(|id| id.key().to_string())).collect();
        self.inventory.spare_weapons = [others.first().cloned().flatten(), others.get(1).cloned().flatten()];
        let (weapon, upgrade) = (status.weapon.clone(), status.weapon_upgrade);
        let spare = self.inventory.spare_weapons.clone();
        let stock = self.inventory.equipment.clone();
        crate::weapons::with(|w| *w = w.with_held(&weapon, upgrade, &spare, &stock));
        crate::weapons::set_slots(k.slot_keys().map(|s| s.map(str::to_string)), k.current_slot() as u8);
        (self.log)(&format!("camp: Keyblade slot {slot}: {} -> {} (slots {:?}, in hand slot {}; Strength {} Magic {})", old.map_or("-", KeybladeId::key), new.map_or("-", KeybladeId::key), k.slot_keys(), k.current_slot(), status.strength(), status.magic()));
        true
    }

    /// THE ITEMS SCREEN, FINISHED (the host's `items_in` hands its rows here): the boost items are usable with A alone
    /// (no target buttons: they raise the whole party), refused once Sora's stat byte is full (255).
    pub fn eq_items_finish(&self, sora: &Sora, mut out: Vec<InvEntry>) -> Vec<InvEntry> {
        let b = sora.status().boosts;
        for e in out.iter_mut() {
            let Some(stat) = boost_stat(&e.id) else { continue };
            e.usable = true;
            e.benefit = [b.strength, b.magic, b.defense, b.ap][stat] < u8::MAX;
        }
        out
    }

    /// A BOOST ITEM USED (THE USER 2026-11-03: "Whole party", as the boss-reward boosts): one taken from the stock, its
    /// stat + 1 for good on EVERY party member: Sora (his boost bytes, the status rebuilt) and Donald and Goofy, joined
    /// or not (each his own record: `PartyMenuEdit::Boost` to the host, which keeps it in `growth.boosts` even for a
    /// friend not spawned yet; the menu's copies raised at once). `_member`: the friend the menu's route named (none
    /// for Sora); a boost ignores it. True when `id` is a boost item (used or not); false: not a boost, the caller's path.
    pub fn eq_use_boost(&mut self, sora: &mut Sora, _member: Option<usize>, id: &str) -> bool {
        let Some(stat) = boost_stat(id) else { return false };
        if !self.inventory.take(id) {
            (self.log)(&format!("camp: {id} not used (none in the stock)"));
            return true;
        }
        let status = sora.status_mut();
        let b = &mut status.boosts;
        let byte = match stat {
            0 => &mut b.strength,
            1 => &mut b.magic,
            2 => &mut b.defense,
            _ => &mut b.ap,
        };
        *byte = byte.saturating_add(1);
        status.rebuild();
        (self.log)(&format!("camp: used {id} on the whole party (Sora: Strength {} Magic {} Defense {} max AP {})", status.strength(), status.magic(), status.defense(), status.max_ap()));
        for chara in [crate::party::PartyChara::Donald, crate::party::PartyChara::Goofy] {
            self.party_edits.push(PartyMenuEdit::Boost { chara, stat: stat as u8 });
        }
        for m in self.party_menu.iter_mut() {
            match stat {
                0 => m.strength += 1,
                1 => m.magic += 1,
                2 => m.defense += 1,
                _ => m.ap_max += 1,
            }
        }
        self.party_menu_dirty = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_weapon_family_is_the_worn_weapons_key_without_its_number() {
        assert_eq!(weapon_family("WEP_DONALD_03"), "WEP_DONALD");
        assert_eq!(weapon_family("WEP_GOOFY_00"), "WEP_GOOFY");
        assert_eq!(weapon_family("WEP_KEYBLADE_SO_00"), "WEP_KEYBLADE_SO");
        assert_eq!(weapon_family("PLAIN"), "PLAIN");
    }

    // ---------------------------------------------------------------------------------------- (weapons P11)
    use crate::formation::Formation;
    use crate::hud::{Frame, HudTargets, StatusAction, TargetView};
    use crate::party::{FriendAiSettings, PartyChara};
    use kh_core::{PadButtons, TargetId, Vec3};
    use kh_ui::camp::CampScreen;
    use kh_core::status::keyblade::NO_EFFECT_NOTE as NO_EFFECT_HELP_PROBE;
    use kh_ui::camp_equip::EquipLayer;

    #[test]
    fn shotlock_names_are_the_ability_rows() {
        // every pair against KH3's ability table, and every Keyblade's shotlocks have a name
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sheets/entities/Load/Tres/TresAbilityData.json");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let sheet: serde_json::Value = serde_json::from_str(&text).expect("TresAbilityData");
        let rows: Vec<&serde_json::Value> = sheet["rows"].as_object().map(|o| o.values().collect()).or_else(|| sheet["rows"].as_array().map(|a| a.iter().collect())).unwrap_or_default();
        for (key, name) in SHOTLOCK_NAMES {
            let row = rows.iter().find(|r| r["m_KeyName"] == key).unwrap_or_else(|| panic!("{key}: no ability row"));
            assert_eq!(row["m_NameTextID"].as_str(), Some(name), "{key}");
        }
        for id in KeybladeId::all() {
            for s in id.shotlocks() {
                assert!(shotlock_name_key(s).is_some(), "{}: shotlock {s} has no name", id.key());
            }
        }
    }

    #[test]
    fn p11_the_collectors_goals_give_the_four_boosts_the_items_screen_uses() {
        // KH3: IC_44 Defense, IC_45 AP, IC_46 Strength, IC_47 Magic Boost (P6a's kh-workshop rows); each one an item of the table
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sheets/port/kh-workshop.json");
        let sheet: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).expect("kh-workshop");
        let reward = |goal: &str| sheet["rows"].as_array().and_then(|rows| rows.iter().find(|r| r[0] == goal)).and_then(|r| r[1]["reward"]["item"].as_str()).map(str::to_string);
        let given: Vec<Option<String>> = ["goal.IC_46", "goal.IC_47", "goal.IC_44", "goal.IC_45"].iter().map(|g| reward(g)).collect();
        assert_eq!(given, kh_ui::camp_equip::BOOST_ITEMS.map(|b| Some(b.to_string())).to_vec());
        for b in kh_ui::camp_equip::BOOST_ITEMS {
            assert!(crate::items::by_key(b).is_some(), "{b} is an item of the table");
        }
    }

    struct NoTargets;
    impl HudTargets for NoTargets {
        fn target(&self, _id: TargetId) -> Option<TargetView> {
            None
        }
    }

    fn same(p: Vec3) -> Vec3 {
        p
    }

    /// THE REAL HUD AS A HOST RUNS IT, with INT's hand-over lines (STATE.md) played by [`Rig::int_lines`] and
    /// [`Rig::finish`] until they are in hud.rs.
    struct Rig {
        hud: Hud,
        fonts: kh_ui::Fonts,
        sora: Sora,
        edits: Vec<PartyMenuEdit>,
        out: std::path::PathBuf,
    }

    impl Rig {
        fn new() -> Option<Rig> {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            let dir = root.join("build/sora_pack/ui");
            let Ok((mut hud, fonts)) = Hud::load(&dir) else {
                eprintln!("no UI pack at {}: skipped", dir.display());
                return None;
            };
            let out = root.join("build/sora_tmp/weapons/P11/shots");
            let _ = std::fs::create_dir_all(&out);
            let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&crate::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
            kh_core::command::customize::ensure(&mut sora);
            // P1's record: the Kingdom Key and Hunny Spout (in slot 2), Hero's Origin and Favorite Deputy in the camp stock
            crate::weapons::begin_new_game();
            let stock: Vec<(String, u32)> = [("WEP_KEYBLADE_SO_01", 1), ("WEP_KEYBLADE_SO_03", 1), ("WEP_DONALD_00", 1), ("WEP_DONALD_02", 2), ("WEP_GOOFY_04", 1), ("ITEM_POWERUP", 2), ("ITEM_APUP", 1)].iter().map(|(k, n)| (k.to_string(), *n)).collect();
            crate::weapons::with(|w| *w = w.with_held("WEP_KEYBLADE_SO_00", 0, &[Some("WEP_KEYBLADE_SO_07".into())], &stock));
            let hunny = KeybladeId::from_key("WEP_KEYBLADE_SO_07").expect("Hunny Spout");
            sora.status_mut().set_keyblade_slot(1, Some(hunny)).expect("slot 2");
            hud.stock_equipment(&stock);
            Some(Rig { hud, fonts, sora, edits: Vec::new(), out })
        }

        /// What INT's lines 3 / 4 / 5 / 7 do in `apply_actions` / `apply_party_actions` (STATE.md).
        fn int_lines(&mut self) {
            for a in std::mem::take(&mut self.hud.status_actions) {
                match a {
                    StatusAction::Equip { kind: KIND_WEAPON, slot, id } if self.hud.eq_keyblade_equip(&mut self.sora, slot, id.as_deref()) => {}
                    StatusAction::UseItem { id } if self.hud.eq_use_boost(&mut self.sora, None, &id) => {}
                    StatusAction::Discard { id } if !kh_ui::camp_equip::can_discard(&id) => {}
                    other => self.hud.status_actions.push(other),
                }
            }
            for (member, a) in std::mem::take(&mut self.hud.party_pending) {
                let index = self.hud.friend_index(member);
                match a {
                    StatusAction::UseItem { id } if self.hud.eq_use_boost(&mut self.sora, index, &id) => {}
                    other => self.hud.party_pending.push((member, other)),
                }
            }
        }

        /// What INT's lines 1 / 2 / 6 do in `refresh_camp` (the screens' data through the finish functions).
        fn finish(&mut self) {
            let Some((screen, member)) = self.hud.camp.as_ref().map(|c| (c.screen, c.member)) else { return };
            match (screen, self.hud.friend(member).cloned()) {
                (CampScreen::Equipment, None) => {
                    let data = self.hud.eq_sora_finish(&self.sora, self.hud.equip_in(&self.sora));
                    if let Some(c) = self.hud.camp.as_mut().filter(|c| c.equipment.data != data) {
                        c.equipment.set(data);
                    }
                }
                (CampScreen::Equipment, Some(m)) => {
                    let data = self.hud.eq_friend_finish(self.hud.friend_equip_in(&self.sora, &m));
                    if let Some(c) = self.hud.camp.as_mut().filter(|c| c.equipment.data != data) {
                        c.equipment.set(data);
                    }
                }
                (CampScreen::Items, _) => {
                    let data = self.hud.eq_items_finish(&self.sora, self.hud.items_in(&self.sora));
                    if let Some(c) = self.hud.camp.as_mut().filter(|c| c.items.entries != data) {
                        c.items.set(data);
                    }
                }
                _ => {}
            }
        }

        fn frames(&mut self, n: u32, word: Option<&str>) {
            for k in 0..n {
                let snapshot = self.sora.ui_snapshot();
                let w: Vec<String> = if k == 0 { word.map(str::to_string).into_iter().collect() } else { Vec::new() };
                self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 100, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
                self.int_lines();
                self.hud.apply_actions(&mut self.sora);
                self.finish();
                self.edits.extend(self.hud.take_party_edits());
                let _ = self.hud.take_sounds();
            }
        }

        fn keys(&mut self, words: &[&str]) {
            for w in words {
                self.frames(14, Some(w));
            }
        }

        fn open(&mut self) {
            self.frames(2, Some("menu open"));
            self.frames(80, None);
        }

        fn equipment(&self) -> &kh_ui::camp_equip::EquipScreen {
            &self.hud.camp_menu().expect("the camp menu is open").equipment
        }

        fn shoot(&self, name: &str) {
            let Some(movie) = self.hud.movie("CampMenu") else { return };
            let (w, h) = (1920usize, 1080usize);
            let (root, _) = crate::hud_layers::stage_root(w as f32, h as f32);
            let mut canvas = kh_ui::Canvas::new(w, h);
            kh_ui::Renderer::new(self.fonts.clone()).draw(&mut canvas, movie, &root);
            // the camp menu's dialog window is a layer of its own, over it
            if let Some(dialog) = self.hud.movie("Dialog") {
                kh_ui::Renderer::new(self.fonts.clone()).draw(&mut canvas, dialog, &root);
            }
            let mut rgba = Vec::new();
            canvas.to_rgba8_over(|_, _| [0.05, 0.05, 0.08], &mut rgba);
            let _ = kh_ui::def::write_png(&self.out.join(format!("{name}.png")), w as u32, h as u32, &rgba);
        }
    }

    fn donald() -> PartyMenuMember {
        PartyMenuMember {
            chara: PartyChara::Donald,
            level: 5,
            exp: 300,
            exp_level: 200,
            exp_next: 400,
            hp: 100,
            hp_max: 140,
            mp: 50,
            mp_max: 100,
            ap_max: 20,
            ap_in_use: 0,
            strength: 8,
            magic: 9,
            defense: 7,
            resists: [0; 6],
            weapon: "WEP_DONALD_00".into(),
            armour: vec![None],
            accessories: vec![None],
            items: vec![Some(ItemKind::Potion)],
            abilities: Vec::new(),
            ai: FriendAiSettings::default(),
            custom: None,
            custom_costs: Vec::new(),
            kit: Vec::new(),
            guard: None,
            item_reload: vec![false],
            item_spent: vec![None],
            parry: None,
        }
    }

    /// The friends' weapons as kh-er hands them (party-weapons.json: their own Strength / Magic, game ability names).
    fn friend_weapons() -> Vec<Piece> {
        let w = |key: &str, s: i32, m: i32, a: &[&str]| Piece { id: key.into(), kind: KIND_WEAPON, attack: s, magic: m, abilities: a.iter().map(|x| x.to_string()).collect(), ..Piece::default() };
        vec![w("WEP_DONALD_00", 3, 4, &[]), w("WEP_DONALD_02", 5, 4, &[]), w("WEP_GOOFY_04", 4, 1, &["HP_CONVERTER"])]
    }

    /// 1. SORA'S THREE KEYBLADE SLOTS: the core's slots (in hand marked), each Keyblade at its Forge level; the stock =
    ///    the owned ones in no slot; the commit moves the stock, the core's slots and P1's record; Remove refused in hand.
    #[test]
    fn p11_soras_keyblade_slots_through_the_real_hud() {
        let Some(mut r) = Rig::new() else { return };
        r.open();
        r.keys(&["menu decide", "menu decide"]); // the character list (Sora), then his page
        r.frames(30, None);
        let e = r.equipment();
        let weapons: Vec<(Option<&str>, bool, Option<u8>)> = e.data.slots.iter().filter(|s| s.kind == KIND_WEAPON).map(|s| (s.piece.as_ref().map(|p| p.id.as_str()), s.current, s.piece.as_ref().and_then(|p| p.level))).collect();
        assert_eq!(weapons, vec![(Some("WEP_KEYBLADE_SO_00"), true, Some(0)), (Some("WEP_KEYBLADE_SO_07"), false, Some(3)), (None, false, None)], "the core's slots; Hunny Spout at its init level +3");
        let stock: Vec<&str> = e.data.stock.iter().filter(|s| s.piece.kind == KIND_WEAPON).map(|s| s.piece.id.as_str()).collect();
        assert_eq!(stock, vec!["WEP_KEYBLADE_SO_01", "WEP_KEYBLADE_SO_03"], "the owned Keyblades in no slot, KH3's order; no friend weapon in Sora's list");
        let kk = e.data.slots[0].piece.clone().unwrap();
        let id = KeybladeId::KINGDOM_KEY;
        assert_eq!((kk.attack, kk.magic, kk.shotlocks.clone()), (id.stats(0).strength, id.stats(0).magic, vec!["King of Hearts".to_string(), "Ragnarok".to_string()]));
        r.shoot("p11_1_keyblade_slots");
        // Remove on the Keyblade in hand: KH3's refusal
        r.keys(&["menu square"]);
        assert_eq!(r.hud.camp_menu().and_then(|c| c.dialog.as_ref()).map(|d| d.text_key), Some("kh3_camp.equip.dialog_0001"));
        r.shoot("p11_4_remove_refused");
        r.keys(&["menu decide"]);
        // slot 3: the stock list, Hero's Origin first; the preview is re-based (slot 3 is empty: the Kingdom Key's terms out)
        r.keys(&["menu down", "menu down", "menu decide"]);
        assert_eq!(r.equipment().layer, EquipLayer::Stock);
        let p = r.equipment().preview(&r.hud.stats_in(&r.sora));
        let s = r.sora.status();
        let ho = KeybladeId::from_key("WEP_KEYBLADE_SO_01").unwrap().stats(0);
        assert_eq!((p.now[3], p.new[3]), (s.strength() - id.stats(0).strength, s.strength() - id.stats(0).strength + ho.strength));
        r.shoot("p11_2_keyblade_stock");
        r.keys(&["menu decide"]);
        let k = *r.sora.status().keyblades();
        assert_eq!(k.slot_keys(), [Some("WEP_KEYBLADE_SO_00"), Some("WEP_KEYBLADE_SO_07"), Some("WEP_KEYBLADE_SO_01")]);
        assert!(!r.hud.inventory.equipment.iter().any(|e| e.0 == "WEP_KEYBLADE_SO_01" && e.1 > 0), "it left the stock");
        assert_eq!(crate::weapons::slots(), Some([Some("WEP_KEYBLADE_SO_00".to_string()), Some("WEP_KEYBLADE_SO_07".to_string()), Some("WEP_KEYBLADE_SO_01".to_string())]), "P1's record");
        // Remove Hunny Spout (slot 2, not in hand): back to the stock, its slot empty
        r.keys(&["menu up", "menu square"]);
        assert_eq!(r.sora.status().keyblades().slot(1), None);
        assert!(r.hud.inventory.equipment.iter().any(|e| e.0 == "WEP_KEYBLADE_SO_07" && e.1 == 1));
        assert!(r.equipment().data.stock.iter().any(|e| e.piece.id == "WEP_KEYBLADE_SO_07"));
        // the Kingdom Key cannot be taken out while in hand, whatever the menu asks (the core's rule too)
        assert!(r.hud.eq_keyblade_equip(&mut r.sora, 0, None));
        assert_eq!(r.sora.status().keyblades().slot(0), Some(KeybladeId::KINGDOM_KEY));
        // a Keyblade he does not own is refused
        assert!(r.hud.eq_keyblade_equip(&mut r.sora, 1, Some("WEP_KEYBLADE_SO_14")));
        assert_eq!(r.sora.status().keyblades().slot(1), None, "Ultima Weapon is not his");
    }

    /// 2. THE ABILITY LIST'S NOTE: Hunny Spout's Harvester carries "(no effect in this world)" after KH3's text.
    #[test]
    fn p11_the_ability_list_shows_the_no_effect_note() {
        let Some(mut r) = Rig::new() else { return };
        let hunny = r.hud.eq_keyblade_piece(KeybladeId::from_key("WEP_KEYBLADE_SO_07").unwrap(), 3);
        let at = hunny.abilities.iter().position(|a| a == "Harvester").expect("Hunny Spout's Harvester");
        assert!(hunny.ability_help[at].ends_with(NO_EFFECT_HELP_PROBE), "{:?}", hunny.ability_help[at]);
        assert!(hunny.ability_help.iter().enumerate().all(|(k, h)| k == at || !h.ends_with(NO_EFFECT_HELP_PROBE)), "only Harvester: {:?}", hunny.ability_help);
        r.open();
        r.keys(&["menu decide", "menu decide", "menu down", "menu triangle"]);
        r.frames(20, None);
        assert_eq!(r.equipment().layer, EquipLayer::Abilities);
        let down = vec!["menu down"; at];
        r.keys(&down);
        assert!(r.equipment().ability_help().is_some_and(|h| h.ends_with(NO_EFFECT_HELP_PROBE)));
        r.shoot("p11_3_keyblade_abilities");
    }

    /// 3. ITEMS: a Keyblade cannot be discarded; a Strength Boost is used with A alone (no target buttons) and raises the
    ///    WHOLE party (THE USER 2026-11-03: "Whole party"): Sora's byte, Donald in the party AND Goofy outside it (each
    ///    his own record: one edit each), one taken from the stock per use.
    #[test]
    fn p11_items_discard_block_and_boosts_on_the_whole_party() {
        let Some(mut r) = Rig::new() else { return };
        let f = Formation::new(&[PartyChara::Sora, PartyChara::Donald], PartyChara::Sora).expect("formation");
        r.hud.set_formation(&f);
        r.hud.set_leader(f.leader(), true);
        r.hud.set_party_menu(vec![donald()]);
        r.frames(20, None);
        r.open();
        r.keys(&["menu down", "menu down", "menu decide"]);
        r.frames(30, None);
        assert_eq!(r.hud.camp_screen(), Some(CampScreen::Items));
        let rows: Vec<(String, bool)> = r.hud.camp_menu().unwrap().items.rows().iter().map(|e| (e.id.clone(), e.usable)).collect();
        let kb = rows.iter().position(|e| e.0 == "WEP_KEYBLADE_SO_01").expect("the Keyblade in the stock");
        let boost = rows.iter().position(|e| e.0 == "ITEM_POWERUP").expect("the Strength Boost");
        assert!(rows[boost].1, "a boost is usable");
        // the Keyblade: Triangle refused
        r.keys(&vec!["menu down"; kb]);
        r.keys(&["menu triangle"]);
        assert_eq!(r.hud.camp_menu().and_then(|c| c.dialog.as_ref()).map(|d| d.text_key), Some("kh3_camp.item.dialog_0004"));
        r.shoot("p11_5_items_discard_refused");
        r.keys(&["menu decide"]);
        assert!(r.hud.inventory.equipment.iter().any(|e| e.0 == "WEP_KEYBLADE_SO_01" && e.1 == 1), "still there");
        // the boost: A alone, the whole party
        let to = boost as i64 - kb as i64;
        let words = if to >= 0 { vec!["menu down"; to as usize] } else { vec!["menu up"; (-to) as usize] };
        r.keys(&words);
        r.shoot("p11_6_boost_target");
        let before = r.sora.status().strength();
        r.keys(&["menu decide"]);
        r.frames(10, None);
        assert!(r.hud.camp_menu().is_some_and(|c| !c.items.target), "no target buttons for a boost");
        assert_eq!((r.sora.status().strength(), r.sora.status().boosts.strength), (before + 1, 1));
        let boosts: Vec<&PartyMenuEdit> = r.edits.iter().filter(|e| matches!(e, PartyMenuEdit::Boost { .. })).collect();
        assert_eq!(boosts, vec![&PartyMenuEdit::Boost { chara: PartyChara::Donald, stat: 0 }, &PartyMenuEdit::Boost { chara: PartyChara::Goofy, stat: 0 }], "Donald and Goofy (not in the party) each once");
        assert_eq!(r.hud.party_menu[0].strength, 9, "the menu's copy at once");
        assert_eq!(r.hud.inventory.equipment.iter().find(|e| e.0 == "ITEM_POWERUP").map(|e| e.1), Some(1), "one used");
        r.shoot("p11_6b_boost_used");
        // the second one: the row goes, Sora + 2 in all
        r.keys(&["menu decide"]);
        assert_eq!((r.sora.status().boosts.strength, r.edits.iter().filter(|e| matches!(e, PartyMenuEdit::Boost { .. })).count()), (2, 4));
        assert!(r.hud.camp_menu().is_some_and(|c| c.items.rows().iter().all(|e| e.id != "ITEM_POWERUP")));
        // not a boost: the item path's (false)
        assert!(!r.hud.eq_use_boost(&mut r.sora, None, "Potion"));
    }

    /// 5. (ROUND D9) A VERSION-5 SAVE WITH `spare_weapons` [Oathkeeper, Crystal Snow]: written and read back as a file, applied
    ///    as kh-er's load does (apply, run_weapons, then the slot / level ties): the core's three slots and the Equipment
    ///    screen's three rows are the save's (Kingdom Key in hand, Oathkeeper, Crystal Snow), the stock does not offer the
    ///    two again; a D-pad switch reaches the record before the next save (then v6), an unchanged one stays v5.
    #[test]
    fn p11_d9_a_v5_saves_spare_keyblades_fill_the_rows() {
        let Some(mut r) = Rig::new() else { return };
        let base = crate::save::new_game_loadout(&kh_core::status::Loadout::default(), None);
        // the save: a v5 file of the older fields
        crate::weapons::begin_new_game();
        let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&base));
        let inventory = crate::hud::Inventory { spare_weapons: [Some("WEP_KEYBLADE_SO_12".into()), Some("WEP_KEYBLADE_SO_06".into())], ..crate::hud::Inventory::default() };
        // an untied core (the weapon in hand alone) never overwrites the record's / older fields' slots
        Hud::keyblade_slots_into_record(sora.status(), &inventory.spare_weapons);
        assert_eq!(crate::weapons::slots(), None);
        Hud::tie_keyblade_slots(sora.status_mut(), &inventory.spare_weapons);
        let mut save = crate::save::SoraSave::capture(&sora, &inventory, "Tarnished", 10, 1000);
        Hud::keyblade_slots_into_record(sora.status(), &inventory.spare_weapons);
        save.take_running_weapons();
        assert!(save.written_version() < crate::save::VERSION, "slots still the older fields': no weapons block (v{})", save.written_version());
        let text = save.to_json();
        // a new session reads it
        crate::weapons::begin_new_game();
        let read = crate::save::SoraSave::from_json(&text).expect("the v5 file reads");
        let mut loaded = crate::hud::Inventory::default();
        read.apply(&base, &mut sora, &mut loaded);
        read.run_weapons();
        assert_eq!(sora.status().keyblades().slot_keys(), [Some("WEP_KEYBLADE_SO_00"), None, None], "THE BUG: apply fills the weapon in hand only");
        Hud::tie_keyblade_slots(sora.status_mut(), &loaded.spare_weapons);
        crate::hud::tie_keyblade_levels(sora.status_mut());
        assert_eq!((sora.status().keyblades().slot_keys(), sora.status().keyblades().current_slot()), ([Some("WEP_KEYBLADE_SO_00"), Some("WEP_KEYBLADE_SO_12"), Some("WEP_KEYBLADE_SO_06")], 0));
        // the real HUD's Equipment screen
        r.sora = sora;
        r.hud.inventory = loaded;
        r.open();
        r.keys(&["menu decide", "menu decide"]);
        r.frames(30, None);
        let e = r.equipment();
        let rows: Vec<(Option<&str>, bool)> = e.data.slots.iter().filter(|s| s.kind == KIND_WEAPON).map(|s| (s.piece.as_ref().map(|p| p.id.as_str()), s.current)).collect();
        assert_eq!(rows, vec![(Some("WEP_KEYBLADE_SO_00"), true), (Some("WEP_KEYBLADE_SO_12"), false), (Some("WEP_KEYBLADE_SO_06"), false)]);
        assert!(e.data.stock.iter().all(|s| s.piece.id != "WEP_KEYBLADE_SO_12" && s.piece.id != "WEP_KEYBLADE_SO_06"), "not offered again: {:?}", e.data.stock.iter().map(|s| &s.piece.id).collect::<Vec<_>>());
        r.shoot("p11_8_v5_spare_rows");
        // a D-pad switch (the core alone) reaches the record at the next capture: slots set, slot 2 in hand
        assert!(r.sora.status_mut().switch_keyblade(true).is_some());
        Hud::keyblade_slots_into_record(r.sora.status(), &r.hud.inventory.spare_weapons);
        assert_eq!((crate::weapons::slots(), crate::weapons::current_slot()), (Some([Some("WEP_KEYBLADE_SO_00".to_string()), Some("WEP_KEYBLADE_SO_12".to_string()), Some("WEP_KEYBLADE_SO_06".to_string())]), 1));
        // and a v6 record's slots come back in hand order after the next load
        let mut again = r.sora.clone();
        *again.status_mut() = kh_core::status::Status::from_loadout(&base);
        Hud::tie_keyblade_slots(again.status_mut(), &[None, None]);
        assert_eq!((again.status().keyblades().slot_keys()[2], again.status().keyblades().current_slot(), again.status().weapon.as_str()), (Some("WEP_KEYBLADE_SO_06"), 1, "WEP_KEYBLADE_SO_12"));
    }

    /// 4. A FRIEND'S WEAPON LIST: his family only, his own stats, duplicates with their count, abilities by KH3's names.
    #[test]
    fn p11_a_friends_weapon_list_is_his_own() {
        let Some(mut r) = Rig::new() else { return };
        r.hud.set_party_weapons(friend_weapons());
        let data = r.hud.eq_friend_finish(r.hud.friend_equip_in(&r.sora, &donald()));
        assert_eq!(data.slots[0].piece.as_ref().map(|p| (p.id.as_str(), p.attack, p.magic, p.level)), Some(("WEP_DONALD_00", 3, 4, None)), "his own row, no Forge level");
        let weapons: Vec<(&str, u32, i32)> = data.stock.iter().filter(|e| e.piece.kind == KIND_WEAPON).map(|e| (e.piece.id.as_str(), e.count, e.piece.attack)).collect();
        assert_eq!(weapons, vec![("WEP_DONALD_00", 1, 3), ("WEP_DONALD_02", 2, 5)], "his staves only (duplicates counted), not Goofy's shield nor a Keyblade");
        let goofy = PartyMenuMember { chara: PartyChara::Goofy, weapon: "WEP_GOOFY_04".into(), ..donald() };
        let data = r.hud.eq_friend_finish(r.hud.friend_equip_in(&r.sora, &goofy));
        let shield = data.slots[0].piece.clone().unwrap();
        assert_eq!((shield.abilities.clone(), shield.ability_help.len()), (vec!["HP Converter".to_string()], 1), "KH3's ability name with its help line");
        // the picture: Donald's page, his weapon slot's stock list
        let f = Formation::new(&[PartyChara::Sora, PartyChara::Donald], PartyChara::Sora).expect("formation");
        r.hud.set_formation(&f);
        r.hud.set_leader(f.leader(), true);
        r.hud.set_party_menu(vec![donald()]);
        r.frames(20, None);
        r.open();
        r.keys(&["menu decide", "menu down", "menu decide"]);
        r.frames(30, None);
        assert_eq!(r.hud.camp_menu().map(|c| c.stats.name.clone()), Some("Donald".into()));
        r.keys(&["menu decide"]);
        r.frames(20, None);
        assert_eq!(r.equipment().stock_rows.iter().map(|s| (s.piece.id.as_str(), s.count)).collect::<Vec<_>>(), vec![("WEP_DONALD_00", None), ("WEP_DONALD_00", Some(1)), ("WEP_DONALD_02", Some(2))]);
        r.keys(&["menu down", "menu down"]);
        r.shoot("p11_7_friend_weapons");
    }
}
