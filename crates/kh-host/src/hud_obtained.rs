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

use kh_core::PadButton;
use kh_ui::key_item::{Obtained, Stage};

use super::{Frame, Hud};
use crate::party::PartyChara;

/// What was obtained.
#[derive(Clone, Debug, PartialEq)]
pub enum ObtainedKind {
    /// An Elden Ring key item: its name and caption as the game's message tables give them, its `EquipParamGoods.iconId`.
    ErKeyItem { name: String, caption: String, icon_id: Option<u32> },
    /// A KH3 key item by `TresItemData.m_KeyName` (`KEY_ITEM02` ...).
    KhKeyItem { key: String },
    /// A Keyblade by its item key (`WEP_KEYBLADE_SO_01` ...): THE HOOK for when Keyblades are obtainable.
    Keyblade { key: String },
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
            ObtainedKind::Character { .. } => "new party member",
        }
    }
}

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

    /// The picture, name and caption of a popup.
    pub fn obtained_content(&self, what: &ObtainedKind) -> Obtained {
        let art = &self.camp_view.art;
        match what {
            ObtainedKind::ErKeyItem { name, caption, icon_id } => Obtained { picture: icon_id.and_then(|i| art.host_icon(i)), name: name.clone(), caption: caption.clone() },
            ObtainedKind::KhKeyItem { key } | ObtainedKind::Keyblade { key } => {
                // the row's help id `kh3_item.<table>.detail_<id>` names its name too: `kh3_item.<table>.<id>`
                let help = art.help_id(key).map(str::to_string);
                let name = help.as_deref().map(|h| h.replacen(".detail_", ".", 1)).map(|k| self.camp_view.string(&k)).filter(|s| !s.is_empty()).unwrap_or_else(|| key.clone());
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
