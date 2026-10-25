//! (party I2) The level-up windows' stacking step = the window's measured height (UTresUIP_HudRightLevelUp init
//! 0x141653B10: `rect1` / `rect3` member "height" of the clip, 0x14163B6F0 -> part +0x114 / +0x118; setParam
//! 0x14165D4B0: +0x104 = rect3's with a third line, else rect1's; HudRight layout 0x141660210 / 0x141652A20:
//! y = the first visible window's base y + the heights of the visible windows before it).

use kh_ui::hud_side::{HudRight, SideStrings};
use kh_ui::{Fonts, Pack};

#[test]
fn level_up_windows_stack_by_their_measured_heights() {
    let Ok(mut pack) = Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return;
    };
    let fonts = Fonts::load(&mut pack).unwrap();
    let strings = SideStrings::load(&mut pack);
    let mut right = HudRight::new(pack.movie("HudRight").unwrap(), fonts, &strings);
    if let Some(s) = right.movie.sprite("hudRight.levelUp") {
        let names: Vec<String> = s.children.iter().filter_map(|n| n.name.as_ref().map(|x| x.to_string())).collect();
        println!("hudRight.levelUp children: {names:?}");
    }
    let (two, three) = right.level_up_heights();
    println!("rect1 height {two}, rect3 height {three}");
    assert!(two > 0.0 && three >= two, "measured: {two} / {three}");
    right.show_level_up(99, 0, &["a".into(), "b".into()]);
    right.show_level_up(99, 1, &["a".into(), "b".into(), "c".into()]);
    right.show_level_up(99, 2, &["a".into()]);
    let y: Vec<f32> = (0..3).map(|i| right.level_up_y(i)).collect();
    println!("window y {y:?}");
    assert!((y[1] - y[0] - two).abs() < 1e-3, "Sora's two-line window is rect1 tall");
    assert!((y[2] - y[1] - three).abs() < 1e-3, "Donald's three-line window is rect3 tall");
}
