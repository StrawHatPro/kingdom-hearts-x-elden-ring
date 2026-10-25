//! (BN) KH3'S BONUS! / LEVEL UP! PANELS (HudRight `getBonus` x 5, `levelUp` x 5): the icons in front of the lines, the
//! windows' colour and portrait, the per-character queue (0x1415874F0 / 0x141659B40), the one stack under the level-up
//! windows and its 0.15 s slide (0x141660210 / 0x141652A20 / 0x141659E80), and the lettering: LEVEL UP! on the level-up
//! windows, BONUS! on the bonus windows. The movie parts need the UI pack (skipped without it).

use kh_ui::hud_side::{icon_line, panel_colour, HudRight, LineIcon, PanelChara, SideStrings, GET_BONUS_WINDOWS, PANEL_COLOURS};
use kh_ui::{Fonts, Pack};

#[test]
fn icons_are_kh3s_icon_text_tags() {
    assert_eq!(icon_line(LineIcon::Ability, "Flash Step"), "<icon(Ability)>Flash Step");
    assert_eq!(icon_line(LineIcon::Magic, "Fira"), "<icon(Magic)>Fira");
    assert_eq!(icon_line(LineIcon::None, "Maximum HP increased!"), "Maximum HP increased!");
    assert_eq!(icon_line(LineIcon::CampItem, "Strength Boost"), "<icon(CampItem)>Strength Boost");
    // a text that has its icon keeps it; an empty line stays empty
    assert_eq!(icon_line(LineIcon::Ability, "<icon(Magic)>Fira"), "<icon(Magic)>Fira");
    assert_eq!(icon_line(LineIcon::Ability, ""), "");
    // the magic icon is not the ability sparkle
    assert_ne!(LineIcon::Magic.tag(), LineIcon::Ability.tag());
}

#[test]
fn colours_are_the_movies_chara_labels() {
    assert_eq!(PANEL_COLOURS, ["sora", "donald", "goofy", "fnpc3", "fnpc4"]);
    assert_eq!(panel_colour("goofy"), 2);
    assert_eq!(panel_colour("DONALD"), 1);
    assert_eq!(panel_colour("someone new"), 4, "0x141661740's default");
}

fn right() -> Option<(HudRight, Pack, SideStrings)> {
    let Ok(mut pack) = Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return None;
    };
    let fonts = Fonts::load(&mut pack).unwrap();
    let strings = SideStrings::load(&mut pack);
    let r = HudRight::new(pack.movie("HudRight").unwrap(), fonts, &strings);
    Some((r, pack, strings))
}

#[test]
fn bonus_windows_stack_queue_and_slide_like_kh3() {
    let Some((mut r, mut pack, strings)) = right() else { return };
    let face = pack.texture("../saveload/textures/MenuFaceExGoofy.png").ok();
    let sora = PanelChara { id: 0, colour: panel_colour("sora"), portrait: None };
    let goofy = PanelChara { id: 2, colour: panel_colour("goofy"), portrait: face.clone() };
    // five windows, the lettering of each kind
    for w in 0..GET_BONUS_WINDOWS {
        let path = if w == 0 { "hudRight.getBonus".to_string() } else { format!("hudRight.getBonus{}", w + 1) };
        assert_eq!(r.movie.text(&format!("{path}.getBonusSet.getBonusTitle.coGetBonusTitle")).as_deref(), Some(strings.bonus.as_str()), "window {w}: BONUS!");
    }
    assert_eq!(strings.bonus, "BONUS!");
    assert_eq!(r.movie.text("hudRight.levelUp.levelUpSet.levelUpTitle.coLevelUpTitle").as_deref(), Some("LEVEL UP!"));
    let step = r.bonus_window_height();
    assert!((step - 80.0).abs() < 0.5, "rect1 is 80 tall: {step}");
    // a level-up window first: the bonus windows go under it (one stack)
    r.show_level_up_as(12, &sora, &["Strength increased!".into()]);
    assert!(r.show_bonus_panel(&sora, &[icon_line(LineIcon::Ability, "Flash Step")]));
    assert!(r.show_bonus_panel(&goofy, &["Maximum HP increased!".into()]));
    let (lv1, _) = r.level_up_heights();
    let y0 = r.level_up_y(0);
    assert!((r.bonus_y(0) - (y0 + lv1)).abs() < 0.6, "Sora's bonus under the level-up window: {} vs {}", r.bonus_y(0), y0 + lv1);
    assert!((r.bonus_y(1) - (y0 + lv1 + step)).abs() < 0.6, "Goofy's under Sora's");
    assert_eq!(r.window_colour(true, 0).as_deref(), Some("sora"));
    assert_eq!(r.window_colour(true, 1).as_deref(), Some("goofy"));
    assert!(!r.window_has_portrait(true, 0), "no picture given");
    assert_eq!(r.window_has_portrait(true, 1), face.is_some(), "Goofy's MenuFace bust");
    assert_eq!(r.bonus_line(0, 0).as_deref(), Some("<icon(Ability)>Flash Step"));
    // a second panel for Sora queues on his window (0x141659B40); Donald takes a window of his own
    assert!(r.show_bonus_panel(&sora, &[icon_line(LineIcon::Magic, "Fira")]));
    assert_eq!(r.bonus_windows()[0], (Some(0), 1));
    let donald = PanelChara { id: 1, colour: panel_colour("donald"), portrait: None };
    assert!(r.show_bonus_panel(&donald, &[icon_line(LineIcon::Magic, "Fira")]));
    assert_eq!(r.bonus_windows()[2], (Some(1), 0));
    // run until the level-up window has gone (272 + frames): the stack closes up by SLIDING (0.15 s, linear)
    let mut seen_between = false;
    let start = r.bonus_y(0);
    for _ in 0..300 {
        r.tick(1.0 / 60.0);
        let y = r.bonus_y(0);
        if y < start - 1.0 && y > y0 + 1.0 {
            seen_between = true;
        }
    }
    assert!(seen_between, "Sora's window slid up (not jumped)");
    // Sora's first panel has gone, his queued one shows on the same window: Fira
    assert_eq!(r.bonus_windows()[0].0, Some(0));
    assert_eq!(r.bonus_line(0, 0).as_deref(), Some("<icon(Magic)>Fira"));
    assert!((r.bonus_y(0) - r.level_up_y(0).min(y0)).abs() < 0.6 || (r.bonus_y(0) - y0).abs() < 0.6, "at the top now: {}", r.bonus_y(0));
}

#[test]
fn every_busy_window_refuses_as_the_game() {
    let Some((mut r, _, _)) = right() else { return };
    for id in 0..GET_BONUS_WINDOWS {
        assert!(r.show_bonus_panel(&PanelChara::plain(id), &["x".into()]));
    }
    assert!(!r.show_bonus_panel(&PanelChara { id: 9, colour: 4, portrait: None }, &["x".into()]), "a sixth character: nothing");
}
