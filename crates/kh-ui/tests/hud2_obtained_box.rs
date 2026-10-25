//! (HUD2 round 2) THE OBTAINED POPUP'S DESCRIPTION BOX FITS ITS TEXT. The user: "make the box that holds the description
//! dynamically sized so it always fits" (the 5-line caption of "Map: Limgrave, West" ran out of KH3's 130 px box).
//! Needs the UI pack.

use kh_ui::camp_menu::LIBRARIES;
use kh_ui::key_item::{KeyItemPopup, Obtained, Stage, MIN_TEXT_SCALE, PLATE_MAX_H};
use kh_ui::{Fonts, Pack};

const LIMGRAVE: &str = "Map of the western region of Limgrave. The demesne of Limgrave, far south of the Erdtree, is bordered to the northwest by the cliffs of Stormveil, and under its ever-clouded skies roll the great steppes of the pale lands.";

/// Opens a popup with this caption and plays its `in`; the layout, the plate's y scale and `getItemL`'s y on the stage.
fn open(caption: &str) -> Option<(kh_ui::key_item::CaptionLayout, f32, f32)> {
    let mut pack = Pack::open_default().map_err(|_| eprintln!("SKIPPED: no UI pack")).ok()?;
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut popup = KeyItemPopup::new(pack.movie_linked(kh_ui::key_item::MOVIE, &LIBRARIES).unwrap(), fonts);
    let mut sounds = Vec::new();
    popup.show(Obtained { picture: None, name: "Map: Limgrave, West".into(), caption: caption.into() }, &mut sounds);
    for _ in 0..200 {
        popup.tick(&mut sounds);
        if popup.stage() == Stage::Wait {
            break;
        }
    }
    assert_eq!(popup.stage(), Stage::Wait);
    let get = popup.movie.sprite("getItemL").unwrap();
    let plate_d = get.children.iter().find(|n| n.depth == 3).map(|n| n.matrix.d).unwrap();
    let y = popup.movie.node("getItemL").unwrap().matrix.ty;
    Some((popup.caption_layout(), plate_d, y))
}

/// The text inside the box with KH3's margins (19 above the field, 16 under it), and the box inside the stage.
fn fits(l: &kh_ui::key_item::CaptionLayout, plate_d: f32, y: f32) {
    assert!(l.text_top >= 19.0 && l.text_bottom + 16.0 <= l.box_h + 0.5, "the text stays inside the box: {l:?}");
    assert!((plate_d - l.box_h / 130.0).abs() < 1.0e-3, "the plate is drawn at the box's height after the timeline ran: {plate_d} vs {l:?}");
    let (top, bottom) = (y - 292.0, y + 212.0 + l.box_h);
    assert!(top >= 24.0 && bottom <= 1056.0 + 0.5, "the popup stays on the stage: {top} .. {bottom}");
}

#[test]
fn hud2_the_five_line_limgrave_caption_grows_the_box() {
    let Some((l, d, y)) = open(LIMGRAVE) else { return };
    assert_eq!((l.lines, l.scale), (5, 1.0), "{l:?}");
    assert!(l.box_h > 130.0, "the box grew: {l:?}");
    fits(&l, d, y);
    assert!((y - (540.0 - (l.box_h - 130.0) / 2.0)).abs() < 0.5, "lifted by half the growth: {y}");
}

#[test]
fn hud2_a_one_line_caption_keeps_kh3s_box() {
    let Some((l, d, y)) = open("Breaks the seal on imp statues.") else { return };
    assert_eq!((l.lines, l.box_h, l.scale), (1, 130.0, 1.0));
    assert_eq!(y, 540.0, "KH3's place");
    fits(&l, d, y);
    // centred in KH3's field: as much room above as below inside the 95 px field
    let (above, below) = (l.text_top - 19.0, 19.0 + 95.0 - l.text_bottom);
    assert!((above - below).abs() <= 1.0, "centred: {above} / {below}");
}

#[test]
fn hud2_a_very_long_caption_steps_down_and_still_fits() {
    let long = std::iter::repeat(LIMGRAVE).take(5).collect::<Vec<_>>().join(" ");
    let Some((l, d, y)) = open(&long) else { return };
    assert!(l.scale < 1.0 && l.scale >= MIN_TEXT_SCALE, "{l:?}");
    assert!(l.box_h <= PLATE_MAX_H + 0.5, "{l:?}");
    fits(&l, d, y);
}
