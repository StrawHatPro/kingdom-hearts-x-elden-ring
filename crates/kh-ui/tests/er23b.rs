//! (IMPL_ER23B) The Items screen's tab icons (`tres.controls.ImageIcon`), the movies' localisation hook, the pad set.
//! Skipped without the UI pack.

use kh_ui::camp_menu::LIBRARIES;
use kh_ui::controls::image_icon_set;
use kh_ui::def::Control;
use kh_ui::list::{TabBar, TAB_ICON_BITMAP, TAB_ICON_CELL, TAB_ICON_COLUMNS};
use kh_ui::{Movie, Pack};

fn pack() -> Option<Pack> {
    Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()
}

/// Each tab of the Items screen shows ITS cell of `ItemListLibrary_bmpIcon`; the white authoring placeholder is gone.
#[test]
fn the_item_tabs_draw_their_icons() {
    let Some(mut pack) = pack() else { return };
    let mut movie = Movie::new(pack.movie_linked("CampMenu", &LIBRARIES).unwrap());
    let path = "backyard.backyardAnm.backyardItem.listTabCamp";
    // the template's icon: an `ImageIcon` whose constructor removed the placeholder
    let template = movie.sprite(&format!("{path}.tabItem.imageIcon")).expect("the tab template's icon");
    assert_eq!(template.def.control, Control::ImageIcon);
    assert!(template.children.is_empty() && template.extra.is_empty(), "the white placeholder is still there");
    let mut tabs = TabBar::new(path);
    let icons = kh_ui::camp_equip::ITEM_TAB_ICONS;
    tabs.build(&mut movie, icons.len(), &icons);
    for (k, index) in icons.iter().enumerate() {
        let icon = movie.sprite(&format!("{path}.t{k}.imageIcon")).unwrap_or_else(|| panic!("tab {k} has no icon"));
        assert!(icon.children.is_empty());
        assert_eq!(icon.gfx.len(), 1, "tab {k}: one drawn cell");
        let rect = &icon.gfx[0];
        assert_eq!((rect.x, rect.y, rect.w, rect.h), (0.0, 0.0, TAB_ICON_CELL.0, TAB_ICON_CELL.1));
        // `draw()`: the bitmap is moved so that the cell (index % column, index / column) lands on the rectangle
        let (column, row) = (index % TAB_ICON_COLUMNS, index / TAB_ICON_COLUMNS);
        assert_eq!((rect.fill.matrix.tx, rect.fill.matrix.ty), (-(column as f32) * TAB_ICON_CELL.0, -(row as f32) * TAB_ICON_CELL.1), "tab {k}");
    }
    // the game's order of the seven tabs (0x14010F4B0) and their texts
    let keys: Vec<&str> = kh_ui::camp_equip::ITEM_TABS.iter().map(|t| t.0.rsplit('_').next().unwrap()).collect();
    assert_eq!(keys, ["0001", "0002", "0003", "0007", "0008", "0005", "0006"]);
    // a sprite that is no ImageIcon is refused; a row / column of 0 draws nothing
    assert!(!image_icon_set(movie.sprite_mut(path).unwrap(), TAB_ICON_BITMAP, 68.0, 32.0, 3, 3, 0));
    let icon = movie.sprite_mut(&format!("{path}.t0.imageIcon")).unwrap();
    assert!(image_icon_set(icon, TAB_ICON_BITMAP, 68.0, 32.0, 0, 3, 0) && icon.gfx.is_empty());
}

/// A text field authored as a link `^KEY` shows the string table's text: the EXP popup's unit is the game's "p".
#[test]
fn the_movies_fields_are_localised() {
    let Some(mut pack) = pack() else { return };
    let mut movie = Movie::new(pack.movie("HudTarget").unwrap());
    assert!(movie.add_instance("planeParent.poExpPntPlane", "ExpPnt", "exp0"));
    let unit = movie.text("planeParent.poExpPntPlane.exp0.expPntCount.expPntTxt.textField");
    assert_eq!(unit.as_deref(), Some(pack.string("kh3_system.cockpit.pop_0010").unwrap().as_str()));
    assert_eq!(unit.as_deref(), Some("p"), "the authoring placeholder was \"t\"");
}

/// The button icons come from the Xbox pad set by default (the folder the game picks for an XInput pad).
#[test]
fn the_pad_icons_are_the_xbox_set() {
    let Some(mut pack) = pack() else { return };
    if std::env::var("KH_UI_GLYPHS").is_ok() {
        return;
    }
    assert_eq!(kh_ui::render::pad_glyphs(), "xboxone");
    let data = pack.data("icon_text").unwrap();
    let Some(file) = data["textures"]["pad_xbox"].as_str() else {
        eprintln!("SKIPPED: the pack has no Xbox pad set (node tools/sora_ui_pack.js)");
        return;
    };
    let set = kh_ui::render::IconSet::load(&mut pack).unwrap();
    let xbox = pack.texture(file).unwrap();
    let (texture, ..) = set.icons["triangle"];
    assert!(std::sync::Arc::ptr_eq(&set.textures[texture], &xbox), "<icon(Triangle)> is not drawn from the Xbox set");
    // the category icons stay in the common picture
    let (common, ..) = set.icons["weapon"];
    assert!(!std::sync::Arc::ptr_eq(&set.textures[common], &xbox));
}
