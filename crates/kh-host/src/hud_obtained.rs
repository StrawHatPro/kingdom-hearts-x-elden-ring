//! (HUD2) THE MAJOR ITEM POPUP "OBTAINED" IN THE HUD (kh_ui::key_item: KH3's `KeyItem` movie and `UTresUIP_KeyItem`).
//!
//! The user (2026-10-20, ER_PHASE_NOTES "HP bar, major item popup, save point polish"): key items of Elden Ring and of
//! Kingdom Hearts, Keyblades (when implemented) and new party members get KH3's OBTAINED popup INSTEAD OF the standard
//! item notification; it PAUSES the game and only A closes it.
//!
//! - WHAT FIRES IT: [`Hud::show_obtained`] with an [`ObtainedKind`]; the picture, name and caption are resolved here:
//!   an Elden Ring key item = its own `MENU_ItemIcon_<iconId>` (the pack's `er_icons`, IMPL_ER43) and the caption of its
//!   message tables (the host's text); a KH3 key item / a Keyblade = its `TresItemData` icon and KH3 texts
//!   (`kh3_item.<table>.<id>` and `.detail_<id>`, the row's help id); a party member = his KH3 menu portrait (the
//!   roster's `MenuFaceEx` bust, BN) under his KH3 name (`kh3_char_name`), the caption the host's.
//! - WHEN IT SHOWS: at once when the world is the player's (no menu, no cutscene, no game over, no save screen);
//!   otherwise it waits until it is. Several in a row: one after the other.
//! - WHILE IT IS UP: [`Hud::paused`] is true (the host holds its world the way it does for the menus: kh-er's
//!   `pause_phase`, the game's own pause), the cockpit stands still, Start opens nothing; A (pad `FaceBottom`; the
//!   keyboard's confirm key arrives as that button) or the word `menu decide` closes it once its `in` is done, with
//!   KH3's Decide sound (static SE 1). B, Start and the rest do nothing.
//! - It is drawn on the overlay layer "Pause" (KH3's pause menu can never be up with it): no new layer.
//! - (P9b ACQUISITION) A WEAPON (a Keyblade or a friend's weapon) gets it when it reaches the stock from a boss, a chest, a
//!   rare drop, a Collector's Goal or the catch-up ([`Hud::obtained_weapon`], called by `take_given_items`); a purchase
//!   never (the shop puts it straight into the stock); a synthesis after KH3's own result screen (P6b holds it with
//!   `crate::bossrewards::hold_weapon_popups`).

use kh_core::PadButton;
use kh_ui::key_item::{Obtained, Stage};

use super::{Frame, Hud};
use crate::bossrewards::weapon_popups_held;
use crate::party::PartyChara;

/// What was obtained.
#[derive(Clone, Debug, PartialEq)]
pub enum ObtainedKind {
    /// An Elden Ring key item: its name and caption as the game's message tables give them, its `EquipParamGoods.iconId`.
    ErKeyItem { name: String, caption: String, icon_id: Option<u32> },
    /// A KH3 key item by `TresItemData.m_KeyName` (`KEY_ITEM02` ...).
    KhKeyItem { key: String },
    /// A Keyblade by its item key (`WEP_KEYBLADE_SO_01` ...): the first hook (kept; the same popup as [`ObtainedKind::Weapon`]).
    Keyblade { key: String },
    /// (P9b ACQUISITION, plan 1.7; THE USER: "Obtaining a weapon should give the same pop up as obtaining a map") A Keyblade
    /// or a friend's weapon by its item key (`WEP_KEYBLADE_SO_01`, `WEP_DONALD_05`, `WEP_GOOFY_09`): its `TresItemData`
    /// icon, KH3's `kh3_item.weapon.*` name and help, static SE 10 on open, A closes. Raised when the weapon reaches the
    /// stock from a boss, a chest, a rare drop, a Collector's Goal or the catch-up ([`Hud::obtained_weapon`]); a purchase
    /// never shows it; a synthesis after KH3's own result screen ([`crate::bossrewards::hold_weapon_popups`]).
    Weapon { key: String },
    /// A new party member (replaces the unlock banner): his portrait, his KH3 name, the host's description.
    Character { chara: PartyChara, caption: String },
}

impl ObtainedKind {
    /// Which popup kind this is (the log, tests).
    pub fn label(&self) -> &'static str {
        match self {
            ObtainedKind::ErKeyItem { .. } => "Elden Ring key item",
            ObtainedKind::KhKeyItem { .. } => "KH3 key item",
            ObtainedKind::Keyblade { .. } => "Keyblade",
            ObtainedKind::Weapon { key } if crate::weapons::is_keyblade(key) => "weapon (Keyblade)",
            ObtainedKind::Weapon { .. } => "weapon (a friend's)",
            ObtainedKind::Character { .. } => "new party member",
        }
    }
}

/// (P9b) The weapons whose popup a synthesis holds (`crate::bossrewards::hold_weapon_popups`) until its result screen closes.
static HELD: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// KH3's name of a party member (`kh3_char_name.character`): Sora, Donald Duck, Goofy.
pub(super) const CHARA_NAME_KEYS: [(PartyChara, &str); 3] = [(PartyChara::Sora, "kh3_char_name.character.main_0010"), (PartyChara::Donald, "kh3_char_name.character.main_0020"), (PartyChara::Goofy, "kh3_char_name.character.main_0030")];

impl Hud {
    /// The OBTAINED popup is available (the UI pack has the KeyItem movie).
    pub fn obtained_available(&self) -> bool {
        self.key_item.is_some() && self.obtained_on
    }

    /// The popup is up (or one waits for its turn): the host must not give the item a notification of its own.
    pub fn obtained_open(&self) -> bool {
        self.key_item.as_ref().is_some_and(|k| k.is_open())
    }

    /// Popups waiting for a free moment (not yet up).
    pub fn obtained_waiting(&self) -> usize {
        self.obtained_wait.len() + self.key_item.as_ref().map_or(0, |k| k.queued())
    }

    /// (HUD2) KH3'S OBTAINED POPUP for `what` (see the module): resolved now, shown at the first free frame. False: the
    /// pack has no KeyItem movie (the caller keeps its old notification).
    pub fn show_obtained(&mut self, what: ObtainedKind) -> bool {
        if self.key_item.is_none() || !self.obtained_on {
            return false;
        }
        let shown = self.obtained_content(&what);
        (self.log)(&format!("hud: OBTAINED popup ({}): \"{}\" / \"{}\", picture {}", what.label(), shown.name, shown.caption.replace('\n', " / "), shown.picture.as_ref().map_or("none".to_string(), |t| format!("{} {}x{}", t.file, t.width, t.height))));
        self.obtained_wait.push_back(shown);
        true
    }

    /// (P9b) A WEAPON REACHED THE STOCK through the weapons' door (a boss, a chest, a rare drop's box, a Collector's Goal, the
    /// catch-up; `take_given_items` calls it for each Keyblade and friend's weapon): KH3's OBTAINED popup for it
    /// ([`ObtainedKind::Weapon`]), or held while [`crate::bossrewards::hold_weapon_popups`] is on (a synthesis' result screen first). False: the
    /// pack has no KeyItem movie or `hud.obtained` is off (the caller shows its old item notification).
    pub fn obtained_weapon(&mut self, key: &str) -> bool {
        if !self.obtained_available() {
            return false;
        }
        if weapon_popups_held() {
            (self.log)(&format!("hud: OBTAINED popup for {key} held until the synthesis result screen closes"));
            HELD.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(key.to_string());
            return true;
        }
        self.show_obtained(ObtainedKind::Weapon { key: key.to_string() })
    }

    /// (P9b) The weapon popups held by a synthesis, once the hold is off: queued now.
    fn release_held_weapons(&mut self) {
        if weapon_popups_held() {
            return;
        }
        let held = std::mem::take(&mut *HELD.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
        for key in held {
            self.show_obtained(ObtainedKind::Weapon { key });
        }
    }

    /// The picture, name and caption of a popup.
    pub fn obtained_content(&self, what: &ObtainedKind) -> Obtained {
        let art = &self.camp_view.art;
        match what {
            ObtainedKind::ErKeyItem { name, caption, icon_id } => Obtained { picture: icon_id.and_then(|i| art.host_icon(i)), name: name.clone(), caption: caption.clone() },
            ObtainedKind::KhKeyItem { key } | ObtainedKind::Keyblade { key } | ObtainedKind::Weapon { key } => {
                // the row's help id `kh3_item.<table>.detail_<id>` names its name too: `kh3_item.<table>.<id>`
                // ((P9b) a weapon without a KH3 text: the weapon table's English name)
                let help = art.help_id(key).map(str::to_string);
                let fallback = || crate::weapons::by_key(key).map_or_else(|| key.clone(), |w| w.name.clone());
                let name = help.as_deref().map(|h| h.replacen(".detail_", ".", 1)).map(|k| self.camp_view.string(&k)).filter(|s| !s.is_empty() && !s.starts_with("kh3_item.")).unwrap_or_else(fallback);
                let caption = help.as_deref().map(|h| self.camp_view.string(h)).unwrap_or_default();
                Obtained { picture: art.icon(key), name, caption }
            }
            ObtainedKind::Character { chara, caption } => {
                let name = self.chara_names.iter().find(|c| c.0 == *chara).map(|c| c.1.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| chara.name().to_string());
                Obtained { picture: self.panel_for(*chara).portrait.clone(), name, caption: caption.clone() }
            }
        }
    }

    /// The world is free for a popup: no menu, cutscene, game over, save screen or new-game menu of ours is up.
    pub(super) fn obtained_free(&self) -> bool { // (ALLYJOIN) pub(super): the ally question waits for the same free world
        !self.flow.paused() && self.force_camp == 0 /* a camp menu asked for */ && !self.camp_view.is_open() && !self.pause.is_open() && !self.cutscene && !self.savepoint.is_open() && !self.cutscene_pause.is_open() && !self.gameover.is_open() && !self.gameover_seq.as_ref().is_some_and(|s| s.scene_on()) && !self.newgame_choice_open() && !self.saveload_open()
    }

    /// (HUD2) One frame of the popup. True: it is up and has the frame (the cockpit stands still, nothing else of the
    /// HUD takes input); false: nothing is up (a waiting popup opens here when the world is free).
    pub(super) fn obtained_frame(&mut self, f: &Frame, words: &[String]) -> bool {
        self.release_held_weapons(); // (P9b) a synthesis' weapons after its result screen
        let Some(open) = self.key_item.as_ref().map(|k| k.is_open()) else { return false };
        if !open {
            if self.obtained_wait.is_empty() || !self.obtained_free() {
                return false;
            }
            let next = self.obtained_wait.pop_front().unwrap();
            if let Some(popup) = self.key_item.as_mut() {
                popup.show(next, &mut self.sounds);
            }
            return true;
        }
        let decide = f.pressed.has(PadButton::FaceBottom) || words.iter().any(|w| w == "menu decide");
        let Some(popup) = self.key_item.as_mut() else { return false };
        let mut closed = false;
        if decide && popup.stage() == Stage::Wait {
            closed = popup.decide(&mut self.sounds);
        }
        for _ in 0..f.ticks {
            popup.tick(&mut self.sounds);
        }
        // the ones that came while it was up follow it, still with the world held
        if !popup.is_open() {
            if let Some(next) = self.obtained_wait.pop_front() {
                popup.show(next, &mut self.sounds);
            }
        }
        if closed {
            (self.log)("hud: OBTAINED popup closed by A (static SE 1)");
        }
        true
    }

    /// The popup's movie when it is up (drawn on the "Pause" layer).
    pub(super) fn obtained_movie(&self) -> Option<&kh_ui::Movie> {
        self.key_item.as_ref().filter(|k| k.is_open()).map(|k| &k.movie)
    }

    /// (HUD2) A load / the host going away: every popup goes at once.
    pub fn obtained_reset(&mut self) {
        self.obtained_wait.clear();
        if let Some(k) = self.key_item.as_mut() {
            k.force_close();
        }
    }
}

/// Loads the popup's movie (None: the pack has none; the host keeps its old notifications).
pub(super) fn load(pack: &mut kh_ui::Pack, fonts: &kh_ui::Fonts) -> Option<kh_ui::key_item::KeyItemPopup> {
    match pack.movie_linked(kh_ui::key_item::MOVIE, &kh_ui::camp_menu::LIBRARIES) {
        Ok(def) => Some(kh_ui::key_item::KeyItemPopup::new(def, fonts.clone())),
        Err(error) => {
            println!("hud: the OBTAINED popup is NOT available ({error})");
            None
        }
    }
}

/// KH3's names of the party members from the pack.
pub(super) fn chara_names(pack: &mut kh_ui::Pack) -> Vec<(PartyChara, String)> {
    CHARA_NAME_KEYS.iter().map(|(c, k)| (*c, pack.string(k).unwrap_or_default())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (P9b) THE POPUP KIND OF A WEAPON: a Keyblade's and a friend's weapon's label; through the real HUD and UI pack
    /// (skipped without it) KH3's texts and TresItemData icon, the weapon table's name where the pack has no text; a
    /// synthesis holds the popup until its result screen closes, then it is queued.
    #[test]
    fn p9b_the_weapon_popup_kind_and_the_synthesis_hold() {
        assert_eq!(ObtainedKind::Weapon { key: "WEP_KEYBLADE_SO_05".into() }.label(), "weapon (Keyblade)");
        assert_eq!(ObtainedKind::Weapon { key: "WEP_GOOFY_09".into() }.label(), "weapon (a friend's)");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
        let Ok((mut hud, _)) = Hud::load(&dir) else {
            eprintln!("SKIPPED: no UI pack");
            return;
        };
        hud.obtained_on = true;
        let happy = hud.obtained_content(&ObtainedKind::Weapon { key: "WEP_KEYBLADE_SO_05".into() });
        assert_eq!(happy.name, "Happy Gear");
        assert!(happy.picture.is_some() && !happy.caption.is_empty(), "{:?}", happy.caption);
        let wand = hud.obtained_content(&ObtainedKind::Weapon { key: "WEP_DONALD_05".into() });
        assert!(wand.name.starts_with("Magician's Wand") && wand.picture.is_some(), "{}", wand.name);
        let dead = hud.obtained_content(&ObtainedKind::Weapon { key: "WEP_KEYBLADE_SO_18".into() });
        assert!(dead.name == "Dead of Night" && dead.picture.as_ref().is_some_and(|t| t.file.ends_with("Steam/Item/weapon017.png")), "{} {:?}", dead.name, dead.picture.map(|t| t.file.clone()));
        // EVERY WEAPON ROW (17 Keyblades, 28 friends' weapons) HAS ITS KH3 ICON AND TEXT in the pack (tools/sora_ui_pack.js
        // itemIconsOut; the current pack patched by build/sora_tmp/weapons/P9b/tools/item_icon_patch.js)
        assert_eq!(crate::weapons::table().len(), 45);
        let bare: Vec<&str> = crate::weapons::table().iter().filter(|w| hud.obtained_content(&ObtainedKind::Weapon { key: w.key.clone() }).picture.is_none()).map(|w| w.key.as_str()).collect();
        assert!(bare.is_empty(), "weapons without an icon in the UI pack: {bare:?}");
        // the synthesis' hold
        crate::bossrewards::hold_weapon_popups(true);
        assert!(hud.obtained_weapon("WEP_KEYBLADE_SO_14"));
        assert_eq!(hud.obtained_waiting(), 0, "held while the result screen is up");
        crate::bossrewards::hold_weapon_popups(false);
        hud.release_held_weapons();
        assert_eq!(hud.obtained_waiting(), 1, "queued once the hold is off");
        assert!(hud.obtained_weapon("WEP_GOOFY_09"));
        assert_eq!(hud.obtained_waiting(), 2);
        hud.obtained_on = false;
        assert!(!hud.obtained_weapon("WEP_GOOFY_09"), "hud.obtained off: the caller's old notification");
    }
}
