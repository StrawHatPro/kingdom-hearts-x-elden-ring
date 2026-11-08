//! (FORGE-PREVIEW, the user 2026-11-06: "for the keyblade forge, the icon up top needs to be the keychain like in the
//! example picture, not the whole keyblade itself. The keyblade itself is only shown where it's shown in the example")
//! KH3'S KEYBLADE FORGE PICTURES. KH3's Forge does not draw the item picture (`TresItemData.m_IconAsset`). It draws the
//! two pictures its own table `TresItemWeaponIconData` names per Keyblade:
//! - `PickerIconAsset` (`/Game/UI/Icon/Keyblade/chainTopNN`, 104 x 104): the KEYCHAIN in the selector's round buttons
//!   (`CoImgPickerKeyb`).
//! - `KeybladeIconAsset` (`keybladeNN`, 512 x 512): the whole Keyblade in the big preview (`CoImgKeyb`) and in the
//!   power-up effect (`anmKeyb.img1 / img2`).
//!
//! The pack carries both in `data/forge_icons.json` (`tools/forge_icons.js`). Each picture is read the first time it is
//! shown and then kept. A pack without the file, or a Keyblade without a row, gives no picture. The caller decides
//! what then shows: the preview falls back to the item picture (also a whole-Keyblade picture), and the selector shows
//! nothing rather than a whole Keyblade.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::def::{Pack, Texture};

#[derive(Clone, Default)]
pub struct ForgeArt {
    /// Keychain picture file by Keyblade key (`picker` in the data file).
    pub picker: HashMap<String, String>,
    /// Whole-Keyblade picture file by Keyblade key (`keyblade` in the data file).
    pub keyblade: HashMap<String, String>,
    /// Where each picture file is read from (a staged pack part first, as `Pack::texture` does).
    dirs: Vec<PathBuf>,
    loaded: HashMap<String, Option<Arc<Texture>>>,
}

impl ForgeArt {
    /// Reads the pack's `data/forge_icons.json` (no file: an empty set).
    pub fn load(pack: &Pack) -> ForgeArt {
        let mut art = ForgeArt { dirs: pack.overlays.iter().cloned().chain(std::iter::once(pack.dir.clone())).collect(), ..ForgeArt::default() };
        let Ok(data) = pack.data("forge_icons") else { return art };
        for (field, map) in [("picker", &mut art.picker), ("keyblade", &mut art.keyblade)] {
            for (key, file) in data[field].as_object().into_iter().flatten() {
                if let Some(file) = file.as_str() {
                    map.insert(key.clone(), file.to_string());
                }
            }
        }
        art
    }

    fn texture(&mut self, file: Option<String>) -> Option<Arc<Texture>> {
        let file = file?;
        if let Some(found) = self.loaded.get(&file) {
            return found.clone();
        }
        let path = self.dirs.iter().map(|d| d.join(&file)).find(|p| p.is_file())?;
        let texture = crate::def::read_png(&path).ok().map(|(width, height, rgba)| Arc::new(Texture { file: file.clone(), width, height, rgba }));
        self.loaded.insert(file, texture.clone());
        texture
    }

    /// The Keyblade's keychain (the Forge selector), by its key (an upgraded id `WEP_KEYBLADE_SO_00.upgrade.3` counts as
    /// its Keyblade).
    pub fn picker(&mut self, key: &str) -> Option<Arc<Texture>> {
        let file = self.picker.get(crate::item_art::game_key(key)).cloned();
        self.texture(file)
    }

    /// The whole Keyblade (the Forge preview and its power-up effect), by its key.
    pub fn keyblade(&mut self, key: &str) -> Option<Arc<Texture>> {
        let file = self.keyblade.get(crate::item_art::game_key(key)).cloned();
        self.texture(file)
    }
}
