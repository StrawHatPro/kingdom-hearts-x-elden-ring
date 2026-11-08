//! (THE USER 2026-11-03: "gloveworts should use their Elden Ring icon in all of the KH menus and notifications") THE SHARED
//! ELDEN RING ICON ROUTE, THE GAME HALF: any Elden Ring goods id -> its icon number (`EquipParamGoods.iconId`, read in the
//! game's own params at run time) -> the picture id every KH screen's item art resolves (`kh_ui::item_art::er_icon_key`:
//! Elden Ring's own `MENU_ItemIcon_<n>` from the pack's `er_icons`, 160 x 160, cut from the player's own install by
//! tools/er_icons.js at setup; drawn by `tres.controls.Icon` scaled to the KH3 clip it is loaded into, as the key items are).
//! For any code that shows an Elden Ring goods item on a KH screen (the spirit ashes of Roderika's Spirit Tuning; the
//! Summon command's ashes; ...): put [`goods_icon_key`] where the screen takes an item id for its picture.

use eldenring::cs::{EquipParamGoods, SoloParamRepository};
use fromsoftware_shared::FromStatic;

/// The icon number of a goods row (`None`: no params yet, no such row, or no icon).
pub fn goods_icon_id(goods: u32) -> Option<u32> {
    // SAFETY: the singleton lives as long as the game; its holder table is checked by the SDK.
    let repo = unsafe { SoloParamRepository::instance() }.ok()?;
    let icon = repo.get::<EquipParamGoods>(goods)?.icon_id() as u32;
    (icon > 0).then_some(icon)
}

/// The picture id of a goods row for a KH screen's item art (`ERICON_<iconId>`; `None` as [`goods_icon_id`]).
pub fn goods_icon_key(goods: u32) -> Option<String> {
    goods_icon_id(goods).map(kh_ui::item_art::er_icon_key)
}
