//! (loot, IMPL_ER25) `tres.controls.Icon`: the item picture of the "obtained" popup (`HudLeftGetItem.setParam(icon, name,
//! stockFull)` -> `m_icon.load(icon)`). Skipped without the UI pack.

use kh_ui::controls::icon_load;
use kh_ui::def::Control;
use kh_ui::{Movie, Pack};

/// The popup's icon clip is an `Icon` whose constructor kept its default size and removed the authoring placeholder;
/// loading a picture draws ONE rectangle of the default size with the picture scaled onto it; unloading clears it.
/// The pack holds a picture for every item of the item table (data/item_icons.json).
#[test]
fn the_obtained_popup_draws_the_items_picture() {
    let Some(mut pack) = Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok() else { return };
    let icons = pack.data("item_icons").expect("data/item_icons.json (tools/sora_ui_pack.js 1.5.0)");
    let files = icons["icons"].as_object().unwrap();
    // (IMPL_ER37) 240 items + 55 weapons + 12 key items with a picture (tools/sora_ui_pack.js 1.5.1)
    assert_eq!(files.len(), 307, "one picture per item of sheets/port/sora-item-table.json, Keyblade and key item");
    for key in ["ITEM_POTION", "ITEM_LASTELIXIR", "MAT_ITEM01", "MAT_ITEM53", "PRT_ITEM04", "ACC_ITEM12", "ITEM_POWERUP"] {
        let texture = pack.texture(files[key].as_str().unwrap()).unwrap_or_else(|e| panic!("{key}: {e:?}"));
        assert!(texture.width > 0 && texture.height > 0);
    }
    let potion = pack.texture(files["ITEM_POTION"].as_str().unwrap()).unwrap();
    let mut movie = Movie::new(pack.movie("HudLeft").unwrap());
    let path = "hudleft.getItemS.itemIconSAnm.itemIconS";
    let icon = movie.sprite_mut(path).expect("the popup's icon clip");
    assert_eq!(icon.def.control, Control::Icon);
    assert!(icon.children.is_empty(), "the authoring placeholder is still there");
    let props = icon.icon.expect("the constructor's default size");
    assert!(props.size.0 > 8.0 && props.size.1 > 8.0, "{props:?}");
    assert!(icon_load(icon, Some(potion.clone())));
    assert_eq!(icon.gfx.len(), 1);
    let rect = &icon.gfx[0];
    let (x, y, w, h) = props.clip.unwrap_or((0.0, 0.0, props.size.0, props.size.1));
    assert_eq!((rect.x, rect.y, rect.w, rect.h), (x, y, w, h));
    // the whole picture is scaled onto the default size
    assert!((rect.fill.matrix.a - props.size.0 / potion.width as f32).abs() < 1.0e-6 && (rect.fill.matrix.d - props.size.1 / potion.height as f32).abs() < 1.0e-6);
    assert!(!rect.fill.repeat && rect.fill.smooth);
    // unload
    assert!(icon_load(icon, None));
    assert!(icon.gfx.is_empty());
    println!("icon clip default size {:?}, clip {:?}, picture {} x {}", props.size, props.clip, potion.width, potion.height);
}
