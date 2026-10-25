//! (IMPL_ER37, the user: "fix the menu when leaving the equipment menu without selecting a character") Backing out of
//! the character step of Equipment / Abilities / Stats plays the top clip's `outChara2` (KH3 0x1415BDA30): the
//! character list grows away, the entries come back, the status panel (Sora's portrait) never moves. Movie part skipped
//! without the UI pack.

use kh_ui::camp::{CampKey, CampMenu, CampScreen, StatsIn, TopEntry};
use kh_ui::camp_menu::{top_label, CampMenuView};

fn stats() -> StatsIn {
    StatsIn { name: "Sora".into(), level: 9, exp: 100, next_level_exp: 50, level_exp: 80, hp: 100, hp_max: 120, mp: 100, mp_max: 100, focus: 100, focus_max: 100, focus_unlocked: true, ap_max: 20, strength: 10, magic: 10, defense: 10, resists: [0; 6], munny: 0, play_seconds: 60 }
}

fn child(view: &CampMenuView, clip: &str, name: &str) -> (f32, f32, bool) {
    let node = view.movie.sprite(clip).and_then(|s| s.child(name)).unwrap_or_else(|| panic!("{clip}.{name}"));
    (node.matrix.a, node.cx.mul[3], node.is_visible())
}

fn frames(view: &mut CampMenuView, menu: &mut CampMenu, n: usize) {
    for _ in 0..n {
        view.sync(menu);
        view.tick();
    }
}

#[test]
fn the_top_clip_label_per_level() {
    let chara = |e| CampScreen::Character(e);
    for entry in [TopEntry::Equipment, TopEntry::Abilities, TopEntry::Stats] {
        assert_eq!(top_label(CampScreen::Top, chara(entry)), Some("inChara"));
        assert_eq!(top_label(chara(entry), CampScreen::Top), Some("outChara2"), "B on the character list: back to the top");
        assert_eq!(top_label(chara(entry), CampScreen::Equipment), Some("outChara"), "on to the sub screen");
        assert_eq!(top_label(CampScreen::Equipment, chara(entry)), Some("backChara"));
    }
    for sub in [CampScreen::Equipment, CampScreen::Abilities, CampScreen::Items, CampScreen::Customize, CampScreen::Stats, CampScreen::GameHelp, CampScreen::Config] {
        assert_eq!(top_label(CampScreen::Top, sub), Some("out"), "{sub:?}");
        assert_eq!(top_label(sub, CampScreen::Top), Some("back"), "{sub:?}");
    }
    assert_eq!(top_label(CampScreen::Top, CampScreen::Top), None);
}

/// For each entry with a character step: open it, B before choosing Sora. One press = one level, the menu stays open on
/// the top screen with the same entry focused; the character list grows away (never shrinks), the entries come back to
/// full size, the status panel holds still; afterwards the top clip loops with the list gone.
#[test]
fn backing_out_of_the_character_step_returns_to_the_top_screen() {
    let Ok(mut pack) = kh_ui::Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return;
    };
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    for entry in [TopEntry::Equipment, TopEntry::Abilities, TopEntry::Stats] {
        let mut view = CampMenuView::load(&mut pack, fonts.clone()).unwrap();
        let mut sounds = Vec::new();
        let mut menu = CampMenu::open(false, false, stats(), &mut sounds);
        view.open(&menu);
        frames(&mut view, &mut menu, 50);
        while menu.focused_entry() != Some(entry) {
            menu.key(CampKey::Down, false, &mut sounds);
            frames(&mut view, &mut menu, 2);
        }
        let cursor = menu.top_cursor;
        menu.key(CampKey::Decide, false, &mut sounds);
        assert_eq!(menu.screen, CampScreen::Character(entry));
        frames(&mut view, &mut menu, 30);
        assert_eq!(view.movie.current_label("topMenu"), Some("loopChara"), "{entry:?}");
        sounds.clear();
        let actions = menu.key(CampKey::Cancel, false, &mut sounds);
        assert!(actions.is_empty() && menu.screen == CampScreen::Top && menu.top_cursor == cursor, "{entry:?}: one level back: {actions:?}");
        assert_eq!(sounds.len(), 1);
        let mut list_scales = Vec::new();
        for _ in 0..16 {
            frames(&mut view, &mut menu, 1);
            let status = child(&view, "topMenu", "statusAnm");
            assert!(status.2 && (status.0 - 1.0).abs() < 1e-3 && (status.1 - 1.0).abs() < 1e-3, "{entry:?}: the status panel moved: {status:?}");
            let list = child(&view, "topMenu", "charaList");
            if list.2 {
                list_scales.push(list.0);
            }
        }
        assert!(list_scales.windows(2).all(|w| w[1] >= w[0] - 1e-4) && list_scales.iter().all(|s| *s >= 1.0 - 1e-4), "{entry:?}: the list must grow away: {list_scales:?}");
        assert_eq!(view.movie.current_label("topMenu"), Some("loop"), "{entry:?}");
        let entries = child(&view, "topMenu", "cmdBtnTopMenu");
        assert!(entries.2 && (entries.0 - 1.0).abs() < 1e-3 && (entries.1 - 1.0).abs() < 1e-3, "{entry:?}: the entries are back: {entries:?}");
        assert!(!child(&view, "topMenu", "charaList").2, "{entry:?}: the character list is gone");
    }
}
