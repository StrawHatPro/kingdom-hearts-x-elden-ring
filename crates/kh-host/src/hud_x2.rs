//! (party X2) CUSTOMIZE FOLLOWS THE LEADER (build/sora_tmp/party/XD_PLAYABLE_KITS.md section 3, approved 2026-10-15):
//! while Donald or Goofy leads, the camp menu's Customize shows HIS Magic page ("Abilities" for Goofy: KH3's own
//! `kh3_camp.customize.group_0010`) and HIS shortcut sets A / B / C, with his list (kh-core command/leader.rs
//! `assignable`: Donald every tier Sora has learned and his pouch items; Goofy his abilities by tier and his items).
//! The friend pages are the AI-run members (stream P's `set_ai_members`: the two non-leaders). The edits go back to
//! the host as `PartyMenuEdit::Custom` (kh-partyhost applies them to the leader's block); the local copy is edited at
//! once so the screen never shows the old value for a frame. A child module of `hud` (it reads the Hud's fields).

use super::{Hud, StatusAction};
use crate::party::{CustomEdit, PartyChara, PartyMenuEdit, PartyMenuMember};
use kh_core::command::leader::{entry_name, goofy, LeaderKind, MagicBook, Tab};
use kh_core::Sora;
use kh_ui::camp_custom::{CommandEntry, CustomizeIn, GROUP_ITEM, GROUP_MAGIC};

/// The label key of Goofy's page (KH3's "Abilities", the Friends layer's row title).
pub const GOOFY_LABEL_KEY: &str = "kh3_camp.customize.group_0010";

pub(super) fn leader_kind(c: PartyChara) -> Option<LeaderKind> {
    match c {
        PartyChara::Donald => Some(LeaderKind::Donald),
        PartyChara::Goofy => Some(LeaderKind::Goofy),
        PartyChara::Sora => None,
    }
}

#[allow(dead_code)] // (EQ) the list is Hud::eq_leader_items now
fn items_of(m: &PartyMenuMember) -> Vec<kh_core::status::ItemKind> {
    m.items.iter().flatten().copied().collect()
}

/// The friend's equipped abilities (the menu snapshot's (kind, owned, equipped mask)).
fn abilities_of(m: &PartyMenuMember) -> kh_core::status::AbilitySet {
    let mut set = kh_core::status::AbilitySet::EMPTY;
    for (kind, owned, mask) in &m.abilities {
        set.set(*kind, *owned, mask.count_ones() as u8);
    }
    set
}

/// The spell texts' "Costs {0} MP." with the cost (all-MP spells and others without a number keep their text).
pub(super) fn fill_cost(help: String, cost: Option<i32>) -> String {
    match cost {
        Some(c) if c > 0 => help.replace("{0}", &c.to_string()),
        _ => help,
    }
}

impl Hud {
    /// The leading friend's menu entry with his block (None: Sora leads, or the host keeps no block).
    pub(super) fn x2_leader_member(&self) -> Option<(usize, LeaderKind)> {
        // (EQ) the leader the host named, also while the HUD's shrink-and-grow swap still runs (the status slots swap only
        // when it ends, and it does not run under a paused menu): before, a camp menu opened in that window showed and
        // edited SORA's Customize page while Donald / Goofy led
        let leader = self.leader_anim.map_or(self.leader, |a| a.0);
        let kind = leader_kind(leader)?;
        let i = self.party_menu.iter().position(|m| m.chara == leader && m.custom.is_some())?;
        Some((i, kind))
    }

    /// (party X2) Customize's data for a leading friend; None = Sora's (the caller's own rule).
    pub(super) fn x2_customize_in(&self, sora: &Sora) -> Option<CustomizeIn> {
        let (i, kind) = self.x2_leader_member()?;
        let m = &self.party_menu[i];
        let custom = m.custom?;
        let book = MagicBook::of(sora.status(), sora.command().magic_slots);
        let mut entries = Vec::new();
        let leader_items = self.eq_leader_items(sora, m); // (EQ) his slots' items, then the shared stock's (Sora's list rule)
        // (DON-UI) the leader's own list (Donald's from his own book: kh-core `LeaderCustom::list`)
        for a in custom.list(kind, &book, &leader_items) {
            let help = match (kind, a.tab) {
                // (DON-UI) Donald's own ids: his spells, hybrids and Attunements (kh-core leader::donald::help)
                (LeaderKind::Donald, Tab::Magic) if kh_core::command::leader::donald::is_donald(a.kind) => kh_core::command::leader::donald::help(a.kind),
                // Sora's spell texts (asset order index x 10 + tier, as Sora's own list)
                (LeaderKind::Donald, Tab::Magic) => {
                    let unit = kh_core::magic::spells::unit(a.kind);
                    let index = unit.and_then(|u| kh_core::command::leader::ASSET_ORDER.iter().position(|e| *e == u.element)).unwrap_or(0);
                    let help = self.camp_view.string(&format!("kh3_camp.customize.detail_{}", 1000 + index * 10 + unit.map_or(1, |u| u.tier as usize)));
                    // Sora's cost with Donald's own MP Thrift (the cost he pays)
                    fill_cost(help, kh_core::magic::caster::effective_cost(a.kind, 0, &abilities_of(m), sora.status().mp_cost_scale))
                }
                // Goofy's abilities: KH3's own texts (kh3_system.ability detail_act_0300 / 0310 / 0320, "MP Cost: {0}")
                (LeaderKind::Goofy, Tab::Magic) => match goofy::split(a.kind) {
                    Some((index, _)) => {
                        let help = self.camp_view.string(&format!("kh3_system.ability.detail_act_{:04}", 300 + index as usize * 10));
                        fill_cost(help, m.custom_costs.iter().find(|c| c.0 == a.kind).map(|c| c.1))
                    }
                    None => String::new(),
                },
                _ => String::new(),
            };
            // (EQ) an item entry shows the Auto-Reload of the slots holding it, as Sora's list does
            let auto_reload = kh_core::magic::items::item_of_command(a.kind).is_some_and(|item| m.items.iter().zip(m.item_spent.iter()).zip(m.item_reload.iter()).any(|((i, s), r)| *r && i.or(*s) == Some(item)));
            entries.push(CommandEntry { kind: a.kind, name: a.name.to_string(), group: if a.tab == Tab::Items { GROUP_ITEM } else { GROUP_MAGIC }, auto_reload, help });
        }
        let mut names: Vec<(u8, String)> = (27u8..=68).map(|k| (k, kh_core::magic::spells::command_name(k).to_string())).collect();
        if kind == LeaderKind::Goofy {
            names.extend((goofy::BASE..=goofy::LAST).map(|k| (k, entry_name(k).to_string())));
        }
        if kind == LeaderKind::Donald {
            // (DON-UI) Donald's own ids
            let d = kh_core::command::leader::donald::FIRST..=kh_core::command::leader::donald::LAST;
            names.extend(d.map(|k| (k, entry_name(k).to_string())));
        }
        let mut slots = custom.magic_slots;
        for s in slots.iter_mut().skip(kind.slot_count()) {
            *s = 0;
        }
        Some(CustomizeIn {
            magic_slots: slots,
            shortcuts: custom.shortcuts,
            entries,
            names,
            friends: self.friends_in(),
            magic_label: (kind == LeaderKind::Goofy).then(|| GOOFY_LABEL_KEY.to_string()),
            tier_groups: if kind == LeaderKind::Goofy { (0..3u8).map(|i| [goofy::id(i, 1), goofy::id(i, 2), goofy::id(i, 3)]).collect() } else { Vec::new() },
            shotlock: None, // (XSI) a friend leader's shotlock is his kit's own: no choice
        })
    }

    /// (party X2) A Customize action while a friend leads: his block's edit (sent to the host, applied to the local copy
    /// at once). Returns the action back when it is not his (Sora's rule then).
    pub(super) fn x2_custom_action(&mut self, sora: &Sora, action: StatusAction) -> Option<StatusAction> {
        let Some((i, kind)) = self.x2_leader_member() else { return Some(action) };
        let edit = match action {
            StatusAction::SetMagicSlot { slot, kind } => CustomEdit::SetMagicSlot { slot, kind },
            StatusAction::SwapMagicSlots(a, b) => CustomEdit::SwapMagicSlots(a, b),
            StatusAction::SetShortcut { page, slot, kind } => CustomEdit::SetShortcut { page, slot, kind },
            other => return Some(other),
        };
        let book = MagicBook::of(sora.status(), sora.command().magic_slots);
        let items = self.eq_leader_items(sora, &self.party_menu[i]); // (EQ) the host's list rule (slots, then the stock)
        let chara = self.party_menu[i].chara;
        if let Some(c) = self.party_menu[i].custom.as_mut() {
            let ok = apply(c, kind, &book, &items, edit);
            (self.log)(&format!("camp: {} {edit:?} ({})", chara.name(), if ok { "set" } else { "refused" }));
        }
        self.party_edits.push(PartyMenuEdit::Custom { chara, edit });
        None
    }
}

/// One edit on a leader's block (the same rules the host applies).
pub fn apply(c: &mut kh_core::command::leader::LeaderCustom, kind: LeaderKind, book: &MagicBook, items: &[kh_core::status::ItemKind], edit: CustomEdit) -> bool {
    match edit {
        CustomEdit::SetMagicSlot { slot, kind: cmd } => c.set_slot(kind, book, slot, cmd),
        CustomEdit::SwapMagicSlots(a, b) => c.swap(kind, a, b),
        CustomEdit::SetShortcut { page, slot, kind: cmd } => c.set_shortcut(kind, book, items, page, slot, cmd),
    }
}

// ------------------------------------------------------------------------- (party X2-b) the friends' kit abilities

/// The Abilities rows of a friend's kit abilities carry `KIT_ID_BASE + index into PartyMenuMember::kit` (no
/// `ETresAbilityKind` value is that large).
pub const KIT_ID_BASE: i32 = 1_000_000;

/// A Abilities-screen toggle of a kit row as the friend's action.
pub(super) fn kit_action(id: i32, on: bool) -> Option<StatusAction> {
    (id >= KIT_ID_BASE).then_some(StatusAction::SetKit { id, on })
}

impl Hud {
    /// (party X2-b) His kit abilities on his Abilities page, after KH3's (a Unison / Fusion with KH3's own name and help).
    pub(super) fn x2_kit_rows(&self, m: &PartyMenuMember) -> Vec<kh_ui::camp::AbilityIn> {
        m.kit
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let kh3 = k.kh3.and_then(|kind| kh_core::status::tables::ABILITIES.iter().find(|r| r.kind == kind));
                let help = k.kh3.and_then(|kind| self.ability_data.get(&(kind.value() as i64))).map(|e| e.1.clone()).filter(|h| !h.is_empty()).unwrap_or_else(|| k.help.clone());
                kh_ui::camp::AbilityIn {
                    id: KIT_ID_BASE + i as i32,
                    name: kh3.map_or(k.name.clone(), |r| r.name.to_string()),
                    category: k.category,
                    ap: k.ap,
                    priority: 200_000 + i as i32,
                    owned: 1,
                    equipped: k.equipped as u8,
                    help,
                }
            })
            .collect()
    }

    /// (party X2-b) A kit row toggled on a friend's page: the local copy at once, the edit to the host.
    pub(super) fn x2_kit_edit(&mut self, index: usize, id: i32, on: bool) {
        let Some(m) = self.party_menu.get_mut(index) else { return };
        let chara = m.chara;
        let Some(k) = usize::try_from(id - KIT_ID_BASE).ok().and_then(|i| m.kit.get_mut(i)) else { return };
        if k.equipped == on {
            return;
        }
        k.equipped = on;
        let (key, ap, name) = (k.key.clone(), k.ap, k.name.clone());
        m.ap_in_use += if on { ap } else { -ap };
        (self.log)(&format!("camp: {} {} {name} ({key}, {ap} AP)", chara.name(), if on { "equipped" } else { "removed" }));
        self.party_edits.push(PartyMenuEdit::Kit { chara, key, on });
    }
}
