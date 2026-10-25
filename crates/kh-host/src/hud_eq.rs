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
//! A child module of `hud` (it reads the Hud's fields), as hud_x2.rs.

use super::{item_piece, Hud};
use crate::party::{PartyMenuEdit, PartyMenuMember};
use kh_core::status::ItemKind;
use kh_core::Sora;
use kh_ui::camp_equip::{EquipSlot, KIND_ITEM};

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
}
