//! (THE USER 2026-11-02, DECIDED: "Roderika's spirit tuning opens a KH3-STYLE SCREEN, not Elden Ring's menu") THE SPIRIT
//! TUNING SCREEN IN THE HUD (kh_ui::tuning: the screen). A host opens it with [`Hud::open_tuning`] (the Elden Ring mod: when
//! Roderika's talk script asks for its spirit-tuning window) and hands it its rows every frame ([`Hud::set_tuning_data`]:
//! the ashes, their levels, the next level's glovewort, munny and effects, read from the game at run time); while it is up
//! [`Hud::paused`] is true, it has the pad (D-pad / stick, LB / RB, A, B, Menu; the keyboard's keys arrive as those
//! buttons; `menu ...` words too) and it is drawn on the camp menu's layers ("CampMenu" for the screen, "Dialog" for KH3's
//! question: the camp menu and the shop are never up with it).
//!
//! THE STOCK AND THE MUNNY ARE THE HUD'S: each row's "Stock" is what the camp menu's stock holds of the glovewort, the
//! munny Sora's. A Yes is a REQUEST the host takes ([`Hud::take_tuning_requests`]): the host checks it again against the
//! game (the ash still held at that level), raises the ash's level in the game's inventory and then CHARGES the HUD
//! ([`Hud::charge_tuning`]): in [`Hud::apply_actions`] (the one place the HUD writes to the simulation) the gloveworts leave
//! the stock and the munny Sora's status, as far as he has them. The save keeps both (kh-host save.rs).

use kh_ui::camp::CampKey;
use kh_ui::tuning::{TuneData, TuneEvent, TuneMenu, TuneView};

use super::{Frame, Hud};

/// What the host charges for an upgrade it carried out.
#[derive(Clone, Debug, PartialEq)]
pub struct TuneCharge {
    /// The KH item key of the material (the glovewort) and how many.
    pub key: String,
    pub count: u32,
    pub munny: u32,
}

/// The screen's state in the HUD.
#[derive(Default)]
pub struct TuneState {
    view: Option<TuneView>,
    menu: Option<TuneMenu>,
    /// The host's rows (the stock and the munny are filled in by the HUD).
    host: Option<TuneData>,
    /// Yes on the question: (the ash's host id, the level asked), for the host.
    requests: Vec<(u32, u8)>,
    charges: Vec<TuneCharge>,
    /// The screen closed (its `out` ended) since the host last asked.
    closed: bool,
}

impl TuneState {
    pub fn load(pack: &mut kh_ui::Pack, fonts: &kh_ui::Fonts) -> TuneState {
        let view = TuneView::load(pack, fonts.clone()).map_err(|e| println!("hud: the Spirit Tuning screen is NOT available ({e})")).ok();
        TuneState { view, ..TuneState::default() }
    }

    pub fn is_open(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.is_open())
    }

    /// (THE USER 2026-11-03: "right stick scroll") the right stick's lines on the open screen's descriptions.
    pub(super) fn scroll_help(&mut self, lines: i32) {
        if let Some(v) = self.view.as_mut().filter(|v| v.is_open()) {
            v.movie.scroll_fits(lines);
        }
    }
}

impl Hud {
    /// OPENS THE SPIRIT TUNING SCREEN with the host's rows. False: it cannot open now (no camp menu movie in the pack,
    /// another screen of the HUD is up).
    pub fn open_tuning(&mut self, data: TuneData, sora: &kh_core::Sora) -> bool {
        if self.tuning.view.is_none() || self.tuning.is_open() || self.shop.is_open() || self.workshop.is_open() /* (weapons P6b) */ || self.flow.paused() || self.gameover.is_open() || self.savepoint.is_open() || self.saveload_open() || self.newgame_choice_open() || self.cutscene_pause.is_open() {
            return false;
        }
        let rows = data.rows.len();
        self.tuning.host = Some(data);
        self.tuning.requests.clear();
        let mut menu = TuneMenu::new(self.tuning_data(sora));
        if let Some(view) = self.tuning.view.as_mut() {
            view.open(&mut menu);
        }
        self.sounds.push(kh_ui::UiSound::Play(kh_ui::menu::SE_MENU_OPEN));
        self.tuning.menu = Some(menu);
        self.tuning.closed = false;
        (self.log)(&format!("tuning: OPENS ({rows} spirit ash(es)): {} munny", sora.status().munny));
        true
    }

    /// The host's rows again (every frame while the screen is up: after an upgrade the ash's level and id change).
    pub fn set_tuning_data(&mut self, data: TuneData) {
        if self.tuning.is_open() {
            self.tuning.host = Some(data);
        }
    }

    /// The screen is up (opening, open or closing).
    pub fn tuning_open(&self) -> bool {
        self.tuning.is_open()
    }

    /// True once after the screen closed (its `out` ended): the host's talk goes on.
    pub fn take_tuning_closed(&mut self) -> bool {
        std::mem::take(&mut self.tuning.closed)
    }

    /// The upgrades asked since the host last took them: (the ash's host id, the level asked).
    pub fn take_tuning_requests(&mut self) -> Vec<(u32, u8)> {
        std::mem::take(&mut self.tuning.requests)
    }

    /// The host carried an upgrade out: the gloveworts and the munny leave at the next `apply_actions`.
    pub fn charge_tuning(&mut self, key: &str, count: u32, munny: u32) {
        self.tuning.charges.push(TuneCharge { key: key.to_string(), count, munny });
    }

    /// The host closes it from outside (a cutscene, a load, the talk was cut off).
    pub fn close_tuning(&mut self) {
        if let Some(view) = self.tuning.view.as_mut() {
            view.close();
        }
    }

    /// (tests) The screen's logic while it is up.
    pub fn tuning_menu(&self) -> Option<&TuneMenu> {
        self.tuning.menu.as_ref().filter(|_| self.tuning.is_open())
    }

    /// The screen's movies (the screen, its question) for the camp menu's layers.
    pub(super) fn tuning_movie(&self, name: &str) -> Option<Option<&kh_ui::Movie>> {
        let view = self.tuning.view.as_ref().filter(|v| v.is_open())?;
        Some(match name {
            "CampMenu" => Some(&view.movie),
            "Dialog" => view.dialog_movie(),
            _ => None,
        })
    }

    /// THE ROWS AS SHOWN: the host's, each glovewort's stock from the camp menu's stock, Sora's munny.
    fn tuning_data(&self, sora: &kh_core::Sora) -> TuneData {
        let mut data = self.tuning.host.clone().unwrap_or_default();
        for row in &mut data.rows {
            if let Some(c) = row.cost.as_mut() {
                c.owned = self.inventory.equipment.iter().filter(|e| e.0 == c.key).map(|e| e.1).sum();
            }
        }
        data.munny = sora.status().munny as i64;
        data
    }

    /// The screen's part of a frame: the pad drives it, its movies run. True while it has the frame.
    pub(super) fn tuning_frame(&mut self, f: &Frame, nav: &[(u8, bool)], words: &[String]) -> bool {
        use kh_core::PadButton as B;
        if !self.tuning.is_open() {
            return false;
        }
        let data = self.tuning_data(f.sora);
        let mut keys: Vec<(CampKey, bool)> = nav.iter().map(|(dir, repeat)| ([CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3], *repeat)).collect();
        for (button, key) in [(B::FaceBottom, CampKey::Decide), (B::FaceRight, CampKey::Cancel), (B::LeftShoulder, CampKey::L1), (B::RightShoulder, CampKey::R1), (B::SpecialRight, CampKey::Menu)] {
            if f.pressed.has(button) {
                keys.push((key, false));
            }
        }
        for word in words {
            let key = match word.as_str() {
                "menu up" => CampKey::Up,
                "menu down" => CampKey::Down,
                "menu left" => CampKey::Left,
                "menu right" => CampKey::Right,
                "menu decide" => CampKey::Decide,
                "menu cancel" => CampKey::Cancel,
                "menu menu" => CampKey::Menu,
                "menu l1" => CampKey::L1,
                "menu r1" => CampKey::R1,
                _ => continue,
            };
            keys.push((key, false));
        }
        let (Some(menu), Some(view)) = (self.tuning.menu.as_mut(), self.tuning.view.as_mut()) else { return false };
        // (an upgrade asked and not yet charged: the screen keeps what it showed at the Yes)
        if self.tuning.requests.is_empty() && self.tuning.charges.is_empty() {
            menu.set_data(data);
        }
        for (key, repeat) in keys {
            menu.key(key, repeat, &mut self.sounds);
        }
        let mut close = false;
        for event in menu.take_events() {
            match event {
                TuneEvent::Upgrade { id, to_level } => {
                    (self.log)(&format!("tuning: Yes: spirit ash {id} to +{to_level} (asked of the host)"));
                    self.tuning.requests.push((id, to_level));
                }
                TuneEvent::Closed => close = true,
            }
        }
        if close {
            view.close();
            (self.log)("tuning: B on the list: the screen closes");
        }
        for _ in 0..f.ticks.max(1) {
            view.sync(menu);
            if view.tick() {
                self.tuning.closed = true;
                self.tuning.menu = None;
                self.tuning.host = None;
                self.status_actions.push(super::StatusAction::Unpause); // (the pouch's refill, as after the camp menu)
                (self.log)("tuning: closed");
                break;
            }
        }
        true
    }

    /// THE CHARGES of the host's upgrades, on Sora's munny and the stock (called by `apply_actions`).
    pub(super) fn tuning_apply(&mut self, sora: &mut kh_core::Sora) {
        for c in std::mem::take(&mut self.tuning.charges) {
            let held = self.inventory.equipment.iter().filter(|e| e.0 == c.key).map(|e| e.1).sum::<u32>();
            if let Some(e) = self.inventory.equipment.iter_mut().find(|e| e.0 == c.key) {
                e.1 = e.1.saturating_sub(c.count);
            }
            self.inventory.equipment.retain(|e| e.1 > 0);
            let munny = sora.status().munny;
            sora.status_mut().munny = (munny as i64 - c.munny as i64).max(0) as i32;
            (self.log)(&format!("tuning: charged {} x{} (held {held}) and {} munny ({munny} -> {})", c.key, c.count.min(held), c.munny, sora.status().munny));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_ui::tuning::{TuneCost, TuneRow};

    fn rows() -> TuneData {
        let row = |id: u32, level: u8, key: &str| TuneRow { id, icon: String::new(), name: format!("Ash {id}"), level, max_level: 10, cost: Some(TuneCost { key: key.into(), name: key.into(), count: 1, owned: 0 }), munny: 1000, gives: Vec::new() };
        TuneData { title: "Roderika".into(), munny: 0, rows: vec![row(200003, 3, "MAT_GLOVE_GHOST_04"), row(214001, 1, "MAT_GLOVE_GRAVE_02")] }
    }

    /// (THE USER 2026-11-02) THROUGH THE HUD: the screen opens with the host's rows, the stock and the munny are the HUD's,
    /// it holds the world; A then Yes leaves a request for the host (nothing is charged by the screen itself); the host's
    /// charge takes the glovewort and the munny at `apply_actions`; B closes it and the host is told once.
    #[test]
    fn tuning_through_the_hud() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
        let Ok((mut hud, _)) = Hud::load(&dir) else {
            eprintln!("SKIPPED: no UI pack");
            return;
        };
        let mut sora = crate::savepoint::tests::sora99();
        sora.status_mut().munny = 2500;
        hud.inventory.add("MAT_GLOVE_GHOST_04", 2);
        assert!(hud.open_tuning(rows(), &sora));
        assert!(hud.paused() && hud.tuning_open());
        assert!(!hud.open_tuning(rows(), &sora), "not twice");
        let data = hud.tuning_data(&sora);
        assert_eq!((data.munny, data.rows[0].cost.as_ref().map(|c| c.owned), data.rows[1].cost.as_ref().map(|c| c.owned)), (2500, Some(2), Some(0)));
        let snapshot = sora.ui_snapshot();
        let frame = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<String>>();
        let step = |hud: &mut Hud, sora: &kh_core::Sora, words: Vec<String>| {
            let f = Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &words, dt: 1.0 / 60.0, ticks: 1, pressed: kh_core::PadButtons::NONE, nav: Vec::new() };
            hud.tuning_frame(&f, &[], &words);
        };
        for _ in 0..40 {
            step(&mut hud, &sora, Vec::new());
        }
        step(&mut hud, &sora, frame(&["menu decide"]));
        step(&mut hud, &sora, frame(&["menu decide"]));
        assert_eq!(hud.take_tuning_requests(), vec![(200003, 4)]);
        assert_eq!(sora.status().munny, 2500, "the screen charges nothing itself");
        hud.charge_tuning("MAT_GLOVE_GHOST_04", 1, 1000);
        hud.apply_actions(&mut sora);
        assert_eq!((sora.status().munny, hud.inventory.equipment.iter().find(|e| e.0 == "MAT_GLOVE_GHOST_04").map(|e| e.1)), (1500, Some(1)));
        step(&mut hud, &sora, frame(&["menu cancel"]));
        let mut closed = false;
        for _ in 0..120 {
            step(&mut hud, &sora, Vec::new());
            if hud.take_tuning_closed() {
                closed = true;
                break;
            }
        }
        assert!(closed && !hud.tuning_open() && !hud.take_tuning_closed(), "closed, told once");
    }

    struct NoTargets;
    impl crate::hud::HudTargets for NoTargets {
        fn target(&self, _id: kh_core::TargetId) -> Option<crate::hud::TargetView> {
            None
        }
    }

    fn same(p: kh_core::Vec3) -> kh_core::Vec3 {
        p
    }
}
