//! (FORGE-PREVIEW, the user 2026-11-06: "upgrade menu needs to be fixed so it shows the keyblade that is selected instead
//! of the Kingdom Key") THE KEYBLADE FORGE'S BIG PREVIEW FOLLOWS THE SELECTION, through the host's own drawing path.
//!
//! KH3's preview is `menuKeyb.menuObj.infoKeyb.img.coImg` (`CoImgKeyb`, a `tres.controls.Icon` of 104 x 4.92 = 512 px):
//! the selected Keyblade's whole picture, loaded into it on every move. The port loaded the right picture, but every
//! Keyblade picture has the same size: the rectangle and its matrix
//! stayed the same, the dirty-rectangle hashes (`kh_ui::render::state_hash` / `Renderer::parts`) did not include the
//! picture, so the HUD layer (`hud_layers::draw_movie_in`, split rectangles on in game) never redrew that part and the
//! first Keyblade shown (the Kingdom Key) stayed. Offline pictures drew the whole movie and never showed it.
//!
//! Here every Sora Keyblade (DLC included) is selected in turn on the UI pack (skipped without it): the preview holds
//! that Keyblade's own picture, the name line carries KH3's Keyblade mark, and the picture the HUD layer keeps frame by
//! frame (hash gate + split dirty rectangles, as in game) equals a fresh full drawing. Pictures of three Keyblades:
//! build/sora_tmp/weapons/FORGE-PREVIEW/shots/.
//!
//! (FORGE-PREVIEW, the user 2026-11-06: "for the keyblade forge, the icon up top needs to be the keychain like in the
//! example picture, not the whole keyblade itself") KH3's Forge draws the pictures of its own table
//! `TresItemWeaponIconData`: the selector's round buttons (`CoImgPickerKeyb`) hold the KEYCHAIN (`PickerIconAsset`,
//! chainTopNN, 104 px), the preview the whole Keyblade (`KeybladeIconAsset`, keybladeNN, 512 px). Every visible selector
//! slot is checked to hold the keychain of the Keyblade at its place, and the preview the whole Keyblade of the selection.

use kh_host::hud_layers::{draw_movie_with, Draw, Drawn, Perf};
use kh_ui::camp::CampKey;
use kh_ui::workshop::view::{forge_name, WorkshopView};
use kh_ui::workshop::*;

const PREVIEW: &str = "menuKeyb.menuObj.infoKeyb.img.coImg";
const PICKER: &str = "menuKeyb.menuObj.picker.container";

/// Every Sora Keyblade of the port's weapon table, in table order, as Forge rows.
fn rows() -> Vec<ForgeRow> {
    kh_host::weapons::table()
        .iter()
        .filter(|w| kh_host::weapons::is_keyblade(&w.key))
        .enumerate()
        .map(|(k, w)| {
            let mat = |key: &str, need: u32, stock: u32| MatNeed { key: key.into(), name: kh_host::hud_workshop::item_name(key).0, need, stock };
            let level = (k % 4) as u8 * 3;
            ForgeRow { key: w.key.clone(), name: w.name.clone(), level, max_level: 10, strength: 4 + k as i32 % 5, magic: 3 + k as i32 % 4, next: (level < 10).then(|| ForgeNext { level: level + 1, strength: 5 + k as i32 % 5, magic: 3 + k as i32 % 4, ability: None, materials: vec![mat("MAT_ITEM49", 1, 15), mat("MAT_ITEM04", 4, 0)], ready: false }) }
        })
        .collect()
}

fn data() -> WorkshopData {
    WorkshopData { title: "Roundtable Hold".into(), munny: 40_400, keyblades: rows(), ..WorkshopData::default() }
}

/// The picture the preview holds now (`None`: nothing loaded).
fn preview_texture(view: &WorkshopView) -> Option<std::sync::Arc<kh_ui::def::Texture>> {
    view.movie.sprite(PREVIEW).and_then(|s| s.gfx.first()).map(|g| g.fill.texture.clone())
}

/// (FORGE-PREVIEW) The picture the selector slot `coItem<slot>` (1..9) holds now.
fn slot_texture(view: &WorkshopView, slot: usize) -> Option<std::sync::Arc<kh_ui::def::Texture>> {
    view.movie.sprite(&format!("{PICKER}.coItem{slot}.anim.btn.img.coImg")).and_then(|s| s.gfx.first()).map(|g| g.fill.texture.clone())
}

/// The host's layer: drawn again only when the movie's state hash changed, by split dirty rectangles (as kh-er's HUD).
struct Layer {
    draw: Draw,
    renderer: kh_ui::Renderer,
    hash: u64,
    rect: (i32, i32, usize, usize),
}

impl Layer {
    fn frame(&mut self, movie: &kh_ui::Movie, first: bool) {
        let hash = kh_ui::render::state_hash(movie);
        if hash == self.hash && !first {
            return;
        }
        self.hash = hash;
        let root = kh_ui::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        let perf = Perf { split_dirty: true, bands: 1, band_pixels: 200_000 };
        if let Drawn::Picture(rect, _) = draw_movie_with(&mut self.draw, &mut self.renderer, movie, &root, (1920.0, 1080.0), first, perf) {
            self.rect = rect;
        }
    }
}

/// A fresh drawing of the whole movie (what an offline picture shows).
fn fresh(movie: &kh_ui::Movie, fonts: &kh_ui::Fonts) -> (Vec<u8>, (i32, i32, usize, usize)) {
    let mut draw = Draw::default();
    let mut r = kh_ui::Renderer::new(fonts.clone());
    let root = kh_ui::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let rect = match draw_movie_with(&mut draw, &mut r, movie, &root, (1920.0, 1080.0), true, Perf::default()) {
        Drawn::Picture(rect, _) => rect,
        _ => (0, 0, 0, 0),
    };
    (draw.rgba, rect)
}

/// The layer's picture (straight alpha) over a dark backdrop, 1920 x 1080.
fn save(rgba: &[u8], rect: (i32, i32, usize, usize), name: &str) {
    let mut out = [31u8, 33, 41, 255].repeat(1920 * 1080);
    let (x0, y0, w, h) = rect;
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x0 + x as i32, y0 + y as i32);
            if !(0..1920).contains(&dx) || !(0..1080).contains(&dy) {
                continue;
            }
            let s = &rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
            let d = &mut out[(dy as usize * 1920 + dx as usize) * 4..(dy as usize * 1920 + dx as usize) * 4 + 4];
            let a = u32::from(s[3]);
            for c in 0..3 {
                d[c] = ((u32::from(s[c]) * a + u32::from(d[c]) * (255 - a)) / 255) as u8;
            }
        }
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/FORGE-PREVIEW/shots");
    std::fs::create_dir_all(&dir).ok();
    kh_ui::def::write_png(&dir.join(format!("{name}.png")), 1920, 1080, &out).unwrap();
}

#[test]
fn the_forge_preview_shows_the_selected_keyblade() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    let Ok(mut pack) = kh_ui::Pack::open(&dir) else {
        eprintln!("SKIPPED: no UI pack");
        return;
    };
    let Ok(fonts) = kh_ui::Fonts::load(&mut pack) else { return };
    let Ok(mut view) = WorkshopView::load(&mut pack, fonts.clone()) else {
        eprintln!("SKIPPED: no Shop movie");
        return;
    };
    let data = data();
    let n = data.keyblades.len();
    assert!(n >= 17, "every Sora Keyblade, DLC included ({n})");
    for k in ["WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_17", "WEP_KEYBLADE_SO_18"] {
        assert!(data.keyblades.iter().any(|r| r.key == k), "{k} in the list");
    }
    let mut menu = WorkshopMenu::new(data, Entry::Forge);
    let mut s = Vec::new();
    let mut layer = Layer { draw: Draw::default(), renderer: kh_ui::Renderer::new(fonts.clone()), hash: 0, rect: (0, 0, 0, 0) };
    view.open(&mut menu);
    let mut first = true;
    let mut step = |view: &mut WorkshopView, menu: &mut WorkshopMenu, layer: &mut Layer, frames: usize| {
        for _ in 0..frames {
            view.sync(menu);
            view.tick(Some(menu));
            layer.frame(&view.movie, first);
            first = false;
        }
    };
    step(&mut view, &mut menu, &mut layer, 50);
    let mut seen = Vec::new();
    // the user's picture (Hunny Spout), the first one (Kingdom Key) and a DLC one
    let pictured = |row: &ForgeRow| row.key == "WEP_KEYBLADE_SO_00" || row.name == "Hunny Spout" || row.key == "WEP_KEYBLADE_SO_18";
    for k in 0..n {
        if k > 0 {
            menu.key(CampKey::Right, false, &mut s);
            for _ in 0..40 {
                if view.ready() {
                    break;
                }
                step(&mut view, &mut menu, &mut layer, 1);
            }
            step(&mut view, &mut menu, &mut layer, 20);
        }
        let row = menu.forge_row().cloned().expect("a Keyblade selected");
        // the preview holds THIS Keyblade's own whole picture (KH3's KeybladeIconAsset)
        let want = view.forge_art.keyblade(&row.key).unwrap_or_else(|| panic!("{}: the pack has its Forge picture", row.key));
        assert!(want.file.contains("/Keyblade/keyblade"), "{}: {}", row.key, want.file);
        let got = preview_texture(&view).unwrap_or_else(|| panic!("{}: the preview holds a picture", row.key));
        assert!(std::sync::Arc::ptr_eq(&want, &got), "{}: the preview shows {} instead", row.key, got.file);
        // every visible selector slot holds the KEYCHAIN of the Keyblade at its place (KH3's PickerIconAsset), never a whole Keyblade
        let offsets = picker_offsets(n);
        for (slot, offset) in (-4..=4).enumerate() {
            if !offsets.contains(&offset) {
                continue;
            }
            let at = &menu.data.keyblades[(menu.keyblade as i32 + offset).rem_euclid(n as i32) as usize].key;
            let chain = view.forge_art.picker(at).unwrap_or_else(|| panic!("{at}: the pack has its keychain"));
            assert!(chain.file.contains("/Keyblade/chainTop"), "{at}: {}", chain.file);
            let got = slot_texture(&view, slot + 1).unwrap_or_else(|| panic!("{}: slot {} ({at}) holds a picture", row.key, slot + 1));
            assert!(std::sync::Arc::ptr_eq(&chain, &got), "{}: slot {} shows {} instead of {at}'s keychain", row.key, slot + 1, got.file);
        }
        assert_eq!(view.movie.text("menuKeyb.menuObj.infoKeyb.txName").unwrap_or_default(), forge_name(&row.name), "{}: KH3's Keyblade mark before the name", row.key);
        // what the HUD layer keeps equals a fresh drawing of the whole movie (the old bug: the first Keyblade stayed)
        let (whole, rect) = fresh(&view.movie, &fonts);
        assert_eq!(rect, layer.rect, "{}: the layer's rectangle", row.key);
        let n_diff = layer.draw.rgba.as_chunks::<4>().0.iter().zip(whole.as_chunks::<4>().0).filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 1)).count();
        assert!(n_diff * 10_000 <= whole.len() / 4, "{}: {n_diff} pixel(s) of the HUD's picture are stale", row.key);
        if pictured(&row) {
            save(&layer.draw.rgba, layer.rect, &format!("forge_{}", row.key.to_lowercase()));
        }
        seen.push(row.key);
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), n, "every Keyblade selected once");
}
