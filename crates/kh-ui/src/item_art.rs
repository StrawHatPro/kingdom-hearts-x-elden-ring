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

/// (IMPL_ER43) The icon number of a host key item's row id (`HOSTKEY_3.icon264` -> 264); other ids: `None`.
pub fn host_icon_id(id: &str) -> Option<u32> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let _ = std::fs::remove_dir_all(dir);
    }
}
