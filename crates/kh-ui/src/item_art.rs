//! (IMPL_ER37, the user: "the items in the item screen and in the equipment screen aren't showing their pictures or
//! descriptions, same with the keyblade in the equip menu") THE PICTURE AND THE HELP TEXT OF AN ITEM BY ITS KEY.
//!
//! The game's item table `TresItemData` names, per row (`m_KeyName`: ITEM_POTION, WEP_KEYBLADE_SO_00, PRT_ITEM01,
//! ACC_ITEM01, MAT_ITEM01, KEY_ITEM02 ...), the picture `m_IconAsset` (/Game/UI/Icon/Item/<name>, handed by the native
//! side to the status pane's `CoItemImage`, a `tres.controls.Icon`, whose `load` draws it scaled to the clip's default
//! size: [`crate::controls::icon_load`]) and the help text `m_HelpTextID` (`kh3_item.<table>.detail_<id>`). The UI pack
//! carries both in `data/item_icons.json` (tools/sora_ui_pack.js `itemIconsOut`: `icons[key]` = texture file,
//! `help[key]` = text id). A pack without the file (or an older one without `help`): no pictures, and the help text is
//! found by the shown name as before (`CampMenuView::item_help`).
//!
//! (IMPL_ER43, the user: "Find a way to make the icons load, the simplest way is the best") ELDEN RING'S KEY ITEMS: the
//! host's rows are `HOSTKEY_<n>` and, when the host knows the item's icon number (`EquipParamGoods.iconId`),
//! `HOSTKEY_<n>.icon<iconId>` ([`host_key_id`]). The pack's `data/er_icons.json` (`tools/er_icons.js`: Elden Ring's own
//! `MENU_ItemIcon_<iconId>` cut from the menu atlases of the user's install, 160 x 160 PNG under `er_icons/`) maps the
//! number to its picture, loaded on first use and kept. No index, no such number, or an unreadable file: no picture (the
//! pane shows the text only, as before).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::def::{Pack, Texture};

/// The item pictures and help-text ids of the pack, by item key.
#[derive(Clone, Default)]
pub struct ItemArt {
    pub icons: HashMap<String, Arc<Texture>>,
    pub help: HashMap<String, String>,
    /// (IMPL_ER43) Elden Ring's item pictures by icon number: the file (relative to `pack_dir`), and what was loaded.
    pub host_icons: HashMap<u32, String>,
    pub pack_dir: PathBuf,
    host_loaded: Arc<Mutex<HashMap<u32, Option<Arc<Texture>>>>>,
}

/// (IMPL_ER43) The row id of the host's key item `index` with its icon number (`HOSTKEY_3.icon264`; none: `HOSTKEY_3`).
pub fn host_key_id(index: usize, icon_id: Option<u32>) -> String {
    match icon_id {
        Some(icon) => format!("HOSTKEY_{index}.icon{icon}"),
        None => format!("HOSTKEY_{index}"),
    }
}

/// (THE USER 2026-11-03) THE SHARED ELDEN RING ICON ROUTE: the picture id of Elden Ring's icon number `icon_id`
/// (`EquipParamGoods.iconId` ...; `ERICON_2100`). Any screen's [`ItemArt::icon`] gives Elden Ring's own `MENU_ItemIcon_<n>`
/// for it (the pack's `er_icons`, 160 x 160, drawn by `tres.controls.Icon` scaled to the KH3 clip it is loaded into). A
/// host that knows a goods id finds its icon number in the game's params (kh-er `ericon::goods_icon_key`).
pub fn er_icon_key(icon_id: u32) -> String {
    format!("ERICON_{icon_id}")
}

/// (IMPL_ER43) The icon number of a host key item's row id (`HOSTKEY_3.icon264` -> 264); (THE USER 2026-11-03) and of an
/// [`er_icon_key`] (`ERICON_2100` -> 2100); other ids: `None`.
pub fn host_icon_id(id: &str) -> Option<u32> {
    if let Some(n) = id.strip_prefix("ERICON_") {
        return n.parse().ok();
    }
    let (key, rest) = id.split_once('.')?;
    if !key.starts_with("HOSTKEY_") {
        return None;
    }
    rest.strip_prefix("icon")?.parse().ok()
}

/// The game's key of a host id: an upgraded Keyblade (`WEP_KEYBLADE_SO_00.upgrade.3`) is its Keyblade; the port's
/// pouch spelling `ITEM_MEGAELIXIR` is the game's `ITEM_LASTELIXIR` (Megalixir, I01007). Other ids are the game's keys.
pub fn game_key(id: &str) -> &str {
    let id = id.split_once('.').map_or(id, |(base, _)| base);
    match id {
        "ITEM_MEGAELIXIR" => "ITEM_LASTELIXIR",
        _ => id,
    }
}

impl ItemArt {
    /// Reads `data/item_icons.json` and its textures (missing pieces are skipped).
    pub fn load(pack: &mut Pack) -> ItemArt {
        let mut art = ItemArt::default();
        let Ok(data) = pack.data("item_icons") else { return art };
        for (key, file) in data["icons"].as_object().into_iter().flatten() {
            if let Some(texture) = file.as_str().and_then(|f| pack.texture(f).ok()) {
                art.icons.insert(key.clone(), texture);
            }
        }
        for (key, text) in data["help"].as_object().into_iter().flatten() {
            if let Some(text) = text.as_str() {
                art.help.insert(key.clone(), text.to_string());
            }
        }
        // (IMPL_ER43) Elden Ring's pictures: only the index now, each file when first shown
        art.pack_dir = pack.dir.clone();
        if let Ok(data) = pack.data("er_icons") {
            for (number, file) in data["icons"].as_object().into_iter().flatten() {
                if let (Ok(number), Some(file)) = (number.parse::<u32>(), file.as_str()) {
                    art.host_icons.insert(number, file.to_string());
                }
            }
        }
        // (THE USER 2026-11-03) the host's items that wear an Elden Ring icon (the gloveworts): that picture under their key
        for (key, icon_id) in icon_aliases() {
            if let Some(texture) = art.host_icon(icon_id) {
                art.icons.insert(key, texture);
            }
        }
        art
    }

    /// (IMPL_ER43) Elden Ring's picture of an icon number (`None`: not in the pack's index, or the file is unreadable).
    pub fn host_icon(&self, icon_id: u32) -> Option<Arc<Texture>> {
        let mut loaded = self.host_loaded.lock().ok()?;
        if let Some(found) = loaded.get(&icon_id) {
            return found.clone();
        }
        let texture = self.host_icons.get(&icon_id).and_then(|file| {
            let (width, height, rgba) = crate::def::read_png(&self.pack_dir.join(file)).ok()?;
            Some(Arc::new(Texture { file: file.clone(), width, height, rgba }))
        });
        loaded.insert(icon_id, texture.clone());
        texture
    }

    /// The picture of a host id (`None`: the game has none for it). (IMPL_ER43) Elden Ring's key items `HOSTKEY_n.iconN`:
    /// Elden Ring's own picture from the pack ([`ItemArt::host_icon`]); `HOSTKEY_n` without a number: none.
    pub fn icon(&self, id: &str) -> Option<Arc<Texture>> {
        if let Some(icon_id) = host_icon_id(id) {
            return self.host_icon(icon_id);
        }
        self.icons.get(game_key(id)).cloned()
    }

    /// The help text id of a host id (`kh3_item.weapon.detail_kb_01`).
    pub fn help_id(&self, id: &str) -> Option<&str> {
        self.help.get(game_key(id)).map(String::as_str)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// (THE USER 2026-11-03: "gloveworts should use their Elden Ring icon in all of the KH menus and notifications") A HOST ITEM
// THAT WEARS AN ELDEN RING ICON. The host registers (item key, Elden Ring icon number) pairs ([`set_icon_aliases`]; kh-host
// does it from its item table's `er_icon` column: the gloveworts' `EquipParamGoods.iconId`, 2100 .. 2119), and every picture
// map of the screens gets Elden Ring's own `MENU_ItemIcon_<n>` under that key: the camp menu's Items / Materials lists and
// panes, the shops, the Spirit Tuning screen ([`ItemArt::load`]) and the HUD's OBTAINED popup ([`add_alias_icons`]). The
// pictures are the pack's `er_icons` (tools/er_icons.js cuts them from the player's own install: 160 x 160, drawn by
// `tres.controls.Icon` scaled to the clip's size, as Elden Ring's key items already are). No index or no file: no picture.

static ALIASES: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());

/// The host's (item key, Elden Ring icon number) pairs; replaces the last list.
pub fn set_icon_aliases(list: Vec<(String, u32)>) {
    if let Ok(mut a) = ALIASES.lock() {
        *a = list;
    }
}

/// The registered pairs.
pub fn icon_aliases() -> Vec<(String, u32)> {
    ALIASES.lock().map(|a| a.clone()).unwrap_or_default()
}

/// The registered items' Elden Ring pictures, read from the pack's `er_icons`, put under their keys in a key -> picture map
/// (the HUD's OBTAINED popup keeps its own map).
pub fn add_alias_icons(pack: &mut Pack, icons: &mut HashMap<String, Arc<Texture>>) {
    let aliases = icon_aliases();
    if aliases.is_empty() {
        return;
    }
    let Ok(data) = pack.data("er_icons") else { return };
    for (key, icon_id) in aliases {
        let Some(file) = data["icons"][icon_id.to_string()].as_str() else { continue };
        if let Ok((width, height, rgba)) = crate::def::read_png(&pack.dir.join(file)) {
            icons.insert(key, Arc::new(Texture { file: file.to_string(), width, height, rgba }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (THE USER 2026-11-03: "gloveworts should use their Elden Ring icon in all of the KH menus and notifications") A
    /// registered key shows Elden Ring's picture of its icon number (the same file the key items use), loaded once; an
    /// unregistered key keeps KH3's; a number the index lacks gives none.
    #[test]
    fn registered_keys_wear_their_elden_ring_icons() {
        let dir = std::env::temp_dir().join(format!("kh_ui_alias_icons_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("er_icons")).unwrap();
        let file = std::fs::File::create(dir.join("er_icons/02100.png")).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.write_header().unwrap().write_image_data(&[200u8; 16]).unwrap();
        let mut art = ItemArt { pack_dir: dir.clone(), ..ItemArt::default() };
        art.host_icons.insert(2100, "er_icons/02100.png".into());
        set_icon_aliases(vec![("MAT_GLOVE_GRAVE_01".into(), 2100), ("MAT_GLOVE_GHOST_01".into(), 2110)]);
        assert_eq!(icon_aliases().len(), 2);
        for (key, icon_id) in icon_aliases() {
            if let Some(t) = art.host_icon(icon_id) {
                art.icons.insert(key, t);
            }
        }
        let grave = art.icon("MAT_GLOVE_GRAVE_01").expect("Elden Ring's icon 2100");
        assert_eq!((grave.width, grave.height, grave.file.as_str()), (2, 2, "er_icons/02100.png"));
        assert!(Arc::ptr_eq(&grave, &art.host_icon(2100).unwrap()), "one load, shared with the icon number");
        assert!(art.icon("MAT_GLOVE_GHOST_01").is_none(), "no file for 2110 in this index: no picture");
        set_icon_aliases(Vec::new());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn host_ids_map_to_the_game_keys() {
        assert_eq!(game_key("WEP_KEYBLADE_SO_00"), "WEP_KEYBLADE_SO_00");
        assert_eq!(game_key("WEP_KEYBLADE_SO_00.upgrade.3"), "WEP_KEYBLADE_SO_00");
        assert_eq!(game_key("ITEM_MEGAELIXIR"), "ITEM_LASTELIXIR");
        assert_eq!(game_key("ACC_ITEM03"), "ACC_ITEM03");
        assert_eq!(game_key("HOSTKEY_0"), "HOSTKEY_0");
        assert_eq!(game_key("HOSTKEY_0.icon264"), "HOSTKEY_0");
    }

    /// (IMPL_ER43) A host key item's row id carries its icon number; the index finds its picture, loads it once, and a
    /// number the index lacks (or a row without a number) has none.
    #[test]
    fn host_key_items_find_their_pictures_by_icon_number() {
        assert_eq!(host_key_id(3, Some(264)), "HOSTKEY_3.icon264");
        assert_eq!(host_key_id(3, None), "HOSTKEY_3");
        assert_eq!(host_icon_id("HOSTKEY_3.icon264"), Some(264));
        assert_eq!(host_icon_id("HOSTKEY_3"), None);
        assert_eq!(host_icon_id("WEP_KEYBLADE_SO_00.upgrade.3"), None);
        assert_eq!(host_icon_id("HOSTKEY_3.iconX"), None);
        // (THE USER 2026-11-03) the shared route: any Elden Ring icon number
        assert_eq!((er_icon_key(2100).as_str(), host_icon_id(&er_icon_key(2100)), host_icon_id("ERICON_x")), ("ERICON_2100", Some(2100), None));

        let dir = std::env::temp_dir().join(format!("kh_ui_er_icons_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("er_icons")).unwrap();
        let file = std::fs::File::create(dir.join("er_icons/00264.png")).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.write_header().unwrap().write_image_data(&[255, 0, 0, 255, 0, 255, 0, 128]).unwrap();
        let mut art = ItemArt { pack_dir: dir.clone(), ..ItemArt::default() };
        art.host_icons.insert(264, "er_icons/00264.png".into());
        art.host_icons.insert(265, "er_icons/missing.png".into());
        let icon = art.icon(&host_key_id(0, Some(264))).expect("the picture of icon 264");
        assert_eq!((icon.width, icon.height, &icon.rgba[..]), (2, 1, &[255u8, 0, 0, 255, 0, 255, 0, 128][..]));
        assert!(Arc::ptr_eq(&icon, &art.icon("HOSTKEY_5.icon264").unwrap()), "loaded once");
        assert!(art.icon("HOSTKEY_0.icon265").is_none() && art.icon("HOSTKEY_0.icon7").is_none() && art.icon("HOSTKEY_0").is_none());
        assert!(Arc::ptr_eq(&icon, &art.icon(&er_icon_key(264)).unwrap()), "the shared route: the same picture");
        let _ = std::fs::remove_dir_all(dir);
    }
}
