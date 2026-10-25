//! Tier 2 checks of docs/sora-ui-spec.md 11b (items 6 - 9a): target cursors, enemy gauge, EXP
//! points, popups, the shotlock overlay. Pictures: build/sora_tmp/hud/t2_*.png.

use std::path::Path;

use glam::{Mat4, Vec3};
use kh_ui::def::write_png;
use kh_ui::hud_side::{EnemyGauge, EnemyValues, HudLeft, HudRight, SideStrings};
use kh_ui::hud_target::{HudTarget, TargetDriver, View, LOCKON_TYPE_AUTO, LOCKON_TYPE_FLOWSHOTATTACK, LOCKON_TYPE_NORMAL, SFSS_OUT_SHOOT_FLOW0};
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer, UiSound};

const DT: f32 = 1.0 / 60.0;

fn pack() -> Option<Pack> {
    Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()
}

fn shot(name: &str, movies: &[&Movie], fonts: &Fonts) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/hud");
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    let mut renderer = Renderer::new(fonts.clone());
    for movie in movies {
        renderer.draw(&mut canvas, movie, &Matrix::IDENTITY);
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    write_png(&dir.join(format!("t2_{name}.png")), 1920, 1080, &rgba).unwrap();
}

fn view() -> View {
    View { clip_from_world: Mat4::perspective_infinite_reverse_rh(60f32.to_radians(), 16.0 / 9.0, 0.1), rect: (0.0, 0.0, 1280.0, 720.0) }
}

#[test]
fn target_cursors_follow_and_hide() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut hud = HudTarget::new(pack.movie("HudTarget").unwrap(), fonts.clone());
    hud.native_root("loop");
    assert!(hud.movie.def.unknown_statements.is_empty(), "{:?}", hud.movie.def.unknown_statements);
    let mut driver = TargetDriver::default();
    let mut sounds = Vec::new();
    let positions = std::cell::RefCell::new(vec![Some(Vec3::new(0.0, 0.0, -5.0)), Some(Vec3::new(1.5, 0.5, -6.0))]);
    let position_of = |key: u64| positions.borrow().get(key as usize).copied().flatten();
    // The auto cursor is silent; the lock-on plays static SE 7.
    assert!(driver.add_cursor(&mut hud, 0, LOCKON_TYPE_AUTO, false, &mut sounds).is_some());
    assert!(sounds.is_empty());
    assert!(driver.add_cursor(&mut hud, 1, LOCKON_TYPE_NORMAL, false, &mut sounds).is_some());
    assert_eq!(sounds, vec![UiSound::Play(7)]);
    // The same thing cannot get a second cursor of the same type without allowMulti.
    assert!(driver.add_cursor(&mut hud, 1, LOCKON_TYPE_NORMAL, false, &mut sounds).is_none());
    driver.update(&mut hud, DT, Some(&view()), &position_of, &mut sounds);
    for _ in 0..40 {
        hud.tick();
        driver.update(&mut hud, DT, Some(&view()), &position_of, &mut sounds);
    }
    // A target at the view centre gives (960, 540).
    let node = hud.movie.node("planeParent.lockonMarkPlane.cursor0_0").unwrap();
    assert_eq!((node.matrix.tx, node.matrix.ty, node.is_visible()), (960.0, 540.0, true));
    assert_eq!(hud.movie.current_label("planeParent.lockonMarkPlane.cursor1_0"), Some("loop"));
    shot("cursors", &[&hud.movie], &fonts);
    // Outside the view (beyond the 100 px margin) or behind the camera: hidden, no clamping.
    positions.borrow_mut()[0] = Some(Vec3::new(30.0, 0.0, -5.0));
    positions.borrow_mut()[1] = Some(Vec3::new(0.0, 0.0, 4.0));
    driver.update(&mut hud, DT, Some(&view()), &position_of, &mut sounds);
    assert!(!hud.movie.node("planeParent.lockonMarkPlane.cursor0_0").unwrap().is_visible());
    assert!(!hud.movie.node("planeParent.lockonMarkPlane.cursor1_0").unwrap().is_visible());
    // Removing the lock-on plays static SE 8; a target that is gone loses its cursor.
    sounds.clear();
    driver.remove_cursor(&mut hud, 1, LOCKON_TYPE_NORMAL, &mut sounds);
    assert_eq!(sounds, vec![UiSound::Play(8)]);
    assert!(!hud.movie.exists("planeParent.lockonMarkPlane.cursor1_0"));
    positions.borrow_mut()[0] = None;
    driver.update(&mut hud, DT, Some(&view()), &position_of, &mut sounds);
    assert!(driver.cursors.is_empty() && !hud.movie.exists("planeParent.lockonMarkPlane.cursor0_0"));
    assert!(hud.movie.misses.is_empty(), "paths not found: {:?}", hud.movie.misses);
}

#[test]
fn shotlock_scope_ring_and_lock_count() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut hud = HudTarget::new(pack.movie("HudTarget").unwrap(), fonts.clone());
    hud.native_root("loop");
    let mut driver = TargetDriver::default();
    let mut sounds = Vec::new();
    let position_of = |key: u64| Some(Vec3::new(key as f32 * 0.8 - 1.2, 0.2, -6.0));
    // Aim of 6 s: scope `in`, static SE 18.
    driver.open_scope(&mut hud, 6.0, false, &mut sounds);
    assert_eq!(sounds, vec![UiSound::Play(18)]);
    assert_eq!(hud.movie.current_label("flowScope"), Some("in"));
    // After 3 s: updateFlowScope(159).
    for _ in 0..180 {
        driver.update(&mut hud, DT, Some(&view()), &position_of, &mut sounds);
        hud.tick();
    }
    let ring = hud.movie.current_frame("flowScope.fSGage").unwrap();
    assert!((158..=160).contains(&ring), "ring frame {ring}");
    assert_eq!(hud.movie.current_label("flowScope"), Some("loop"));
    // Locks 1 .. 24: setLockonCount(n, 24) with SE 19, the 24th with SE 20 and `max`.
    sounds.clear();
    for n in 1..=24 {
        driver.add_cursor(&mut hud, (n % 4) as u64, LOCKON_TYPE_FLOWSHOTATTACK, true, &mut sounds);
        driver.set_lock_count(&mut hud, n, 24, &mut sounds);
        driver.update(&mut hud, DT, Some(&view()), &position_of, &mut sounds);
        hud.tick();
        if n == 7 {
            for _ in 0..30 {
                hud.tick();
            }
            shot("scope", &[&hud.movie], &fonts);
        }
    }
    assert_eq!(sounds.iter().filter(|s| **s == UiSound::Play(19)).count(), 23);
    assert_eq!(sounds.last(), Some(&UiSound::Play(20)));
    assert_eq!(hud.movie.current_label("flowScope.flowLockMax"), Some("max"));
    assert_eq!(hud.movie.sprite("flowScope.flowLock.sFLockCount.coFSCount").unwrap().number.as_ref().unwrap().number, 24);
    assert_eq!(driver.cursors.len(), 24);
    // Fired: `select`; an aim without time plays SE 25 and starts with the ring at its end.
    driver.close_scope(&mut hud, SFSS_OUT_SHOOT_FLOW0);
    assert_eq!(hud.movie.current_label("flowScope"), Some("select"));
    sounds.clear();
    driver.open_scope(&mut hud, 0.0, false, &mut sounds);
    assert_eq!(sounds, vec![UiSound::Play(25)]);
    assert_eq!(hud.movie.current_frame("flowScope.fSGage"), Some(317));
    // Style gauge full: SE 26 and the notice.
    sounds.clear();
    driver.set_style_ratio(1.0);
    driver.update(&mut hud, DT, Some(&view()), &position_of, &mut sounds);
    assert_eq!(sounds, vec![UiSound::Play(26)]);
    assert_eq!(hud.movie.current_label("styleMaxInfo"), Some("in"));
    assert!(hud.movie.misses.is_empty(), "paths not found: {:?}", hud.movie.misses);
}

#[test]
fn exp_points_and_left_popups() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let strings = SideStrings::load(&mut pack);
    assert_eq!(strings.level_up_lines[0], "Strength increased!");
    let mut target = HudTarget::new(pack.movie("HudTarget").unwrap(), fonts.clone());
    target.native_root("loop");
    let mut driver = TargetDriver::default();
    let mut left = HudLeft::new(pack.movie("HudLeft").unwrap(), fonts.clone(), &strings);
    let mut right = HudRight::new(pack.movie("HudRight").unwrap(), fonts.clone(), &strings);
    assert!(left.movie.def.unknown_statements.is_empty() && right.movie.def.unknown_statements.is_empty());
    let mut sounds = Vec::new();
    let nothing = |_: u64| None;
    // EXP award of 120 at level 1: EXP point "120", level-up "LV 2", Next LV 280 - 120 = 160.
    driver.add_exp_point(&mut target, Vec3::new(0.3, 0.4, -6.0), 120);
    right.show_level_up(2, 0, &[strings.level_up_lines[0].clone(), strings.level_up_lines[3].clone()]);
    left.show_next_lv(160);
    let (mut exp_frames, mut next_frames, mut level_frames) = (0, 0, 0);
    for frame in 0..400 {
        driver.update(&mut target, DT, Some(&view()), &nothing, &mut sounds);
        target.tick();
        left.tick();
        right.tick(DT);
        exp_frames += (target.movie.current_label("planeParent.poExpPntPlane.exp1") != Some("off")) as u32;
        next_frames += left.is_visible(1) as u32;
        level_frames += right.level_up_visible() as u32;
        if frame == 50 {
            shot("exp_levelup_nextlv", &[&target.movie, &left.movie, &right.movie], &fonts);
        }
    }
    assert_eq!(target.movie.text("planeParent.poExpPntPlane.exp1.expPntCount.expPntSuji.textField").as_deref(), Some("120"));
    assert_eq!(left.movie.text("hudleft.nextLv.nextLvCountP.nextLvCountC.textField").as_deref(), Some("160"));
    assert_eq!(right.movie.text("hudRight.levelUp.levelUpSet.levelUpStatus.txLvNumber.textField").as_deref(), Some("2"));
    // The clips: EXP point 122 frames (2.03 s); Next LV from `in` (frame 9) to its out event
    // (frame 139; the spec counts the 146 frames to the clip's end); level up 272 frames.
    assert!((120..=124).contains(&exp_frames), "EXP point {exp_frames} frames");
    assert!((129..=147).contains(&next_frames), "Next LV {next_frames} frames");
    assert!((270..=274).contains(&level_frames), "level up {level_frames} frames");
    // Munny 10 then 25 within 2.4 s: the counter shows 10 then 35.
    assert_eq!(left.show_munny(10), 10);
    for _ in 0..60 {
        left.tick();
    }
    assert_eq!(left.show_munny(25), 35);
    assert_eq!(left.movie.text("hudleft.getMunny.munnyCountP.munnyCountC.textField").as_deref(), Some("35"));
    for _ in 0..200 {
        left.tick();
    }
    assert_eq!(left.show_munny(7), 7, "it restarts once it has closed");
    // Three items at once: three popups in sequence.
    assert!(left.show_item("Potion", false));
    assert!(!left.show_item("Ether", false) && !left.show_item("Elixir", true));
    let mut names = vec![left.movie.text("hudleft.getItemS.txItemNameS.coItemNameSTxt").unwrap()];
    for frame in 0..400 {
        left.tick();
        let name = left.movie.text("hudleft.getItemS.txItemNameS.coItemNameSTxt").unwrap();
        if names.last() != Some(&name) {
            names.push(name);
        }
        if frame == 30 {
            shot("left_stack", &[&left.movie], &fonts);
        }
    }
    assert_eq!(names, ["Potion", "Ether", "Elixir"]);
    assert!(left.movie.misses.is_empty() && right.movie.misses.is_empty() && target.movie.misses.is_empty(), "{:?} {:?} {:?}", left.movie.misses, right.movie.misses, target.movie.misses);
}

#[test]
fn enemy_gauge_bars_squares_and_hide_timer() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let strings = SideStrings::load(&mut pack);
    let mut right = HudRight::new(pack.movie("HudRight").unwrap(), fonts.clone(), &strings);
    let mut gauge = EnemyGauge::default();
    let mut sounds = Vec::new();
    let hp = std::cell::Cell::new(3000);
    let values_of = |_: u64| Some(EnemyValues { hp: hp.get(), hp_max: 3000, large: false, alive: hp.get() > 0 });
    // Max HP 3000: 15 bars = 14 stock squares under a full top bar.
    gauge.point_at(1, false, None);
    gauge.update(&mut right, DT, &values_of, &mut sounds);
    let calls = right.calls.join(" | ");
    assert!(calls.contains("enmHpPack.in") && calls.contains("setEHPMax(14, 501)") && calls.contains("setEHP(14, 501, 501)"), "{calls}");
    // HP 1234: 6 squares, top bar frame 86.
    hp.set(1234);
    gauge.update(&mut right, DT, &values_of, &mut sounds);
    assert_eq!(right.enemy_gauge_state(), (6, 86));
    assert!(right.calls.last().unwrap().starts_with("setEHP(6, 86, 501)"), "{:?}", right.calls.last());
    for _ in 0..90 {
        gauge.update(&mut right, DT, &values_of, &mut sounds);
        right.tick(DT);
    }
    shot("enemy_gauge", &[&right.movie], &fonts);
    assert!(right.enemy_gauge_shown());
    // The last cursor went: the gauge stays 0.5 s, then hides.
    gauge.clear(false);
    for _ in 0..28 {
        gauge.update(&mut right, DT, &values_of, &mut sounds);
        right.tick(DT);
    }
    assert!(right.enemy_gauge_shown(), "still shown after 0.47 s");
    for _ in 0..4 {
        gauge.update(&mut right, DT, &values_of, &mut sounds);
        right.tick(DT);
    }
    assert!(!right.enemy_gauge_shown() && gauge.target().is_none());
    // Pointed at by a hit: the red gauge starts from the HP before the hit.
    gauge.point_at(1, false, Some(1234));
    hp.set(900);
    gauge.update(&mut right, DT, &values_of, &mut sounds);
    assert!(right.enemy_gauge_shown());
    assert!(right.calls.last().unwrap().starts_with("setEHP(4, 251, 501)"), "{:?}", right.calls.last());
    // It hides at once when the target dies.
    hp.set(0);
    gauge.update(&mut right, DT, &values_of, &mut sounds);
    assert!(!right.enemy_gauge_shown());
    assert!(right.movie.misses.is_empty(), "paths not found: {:?}", right.movie.misses);
}
