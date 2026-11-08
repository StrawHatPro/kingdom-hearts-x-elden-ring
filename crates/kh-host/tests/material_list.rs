//! (SYNTH-MOOGLE, the user 2026-11-07: "Synthesis workshop's material list can do this in the second picture, fix it")
//! THE WORKSHOP'S MATERIAL LIST KEEPS ITS TWO-COLUMN GRID ON EVERY VISIT, through the host's own drawing path.
//!
//! The user's picture (build/sora_tmp/synth_list/user_material_list.jpg): one line only, "? ? ?" drawn over
//! "Lightning Crystal" and two stock digits over each other. The list widget lays its rows in one column; the view
//! moves them into KH3's grid (`columnNum 2`) and took the grid from slots i0 / i1 read back from the movie. After the
//! first placement i1 sits on i0's line, so the second visit of the list (B and in again, or the next talk to the
//! Moogle) read a line pitch of 0: every line of a column was drawn on its first one.
//!
//! Here the list is visited three times (twice in one talk, once after closing and opening the menu again), scrolled
//! and paged with LB / RB each time: after every step each visible row slot holds the material of its place, stands on
//! its own grid cell (no two rows on one place), and the picture the HUD layer keeps (hash gate + split dirty
//! rectangles, as in game) equals a fresh full drawing. Pictures: build/sora_tmp/synth_list/shots/.

use kh_host::hud_layers::{draw_movie_with, Draw, Drawn, Perf};
use kh_ui::camp::CampKey;
use kh_ui::workshop::view::WorkshopView;
use kh_ui::workshop::*;

/// 45 materials as an early Sora has them: most unknown ("? ? ?", stock 0), a few known (KH3's names).
fn data() -> WorkshopData {
    let known = ["MAT_ITEM01", "MAT_ITEM04", "MAT_ITEM05", "MAT_ITEM12", "MAT_ITEM40", "MAT_ITEM49"];
    let materials = (1..=45)
        .map(|k| {
            let key = format!("MAT_ITEM{k:02}");
            let is_known = known.contains(&key.as_str()) || k % 9 == 2;
            let name = if is_known { kh_host::hud_workshop::item_name(&key).0 } else { "? ? ?".into() };
            MaterialRow { stock: if is_known { (k as u32 * 7) % 23 } else { 0 }, known: is_known, rank: 3u8.saturating_sub((k / 12) as u8), key, name }
        })
        .collect();
    WorkshopData { title: "Roundtable Hold".into(), munny: 40_098, commands: Screen::COMMANDS.iter().map(|s| (*s, false)).collect(), materials, ..WorkshopData::default() }
}

/// The host's layer: drawn again only when the movie's state hash changed, by split dirty rectangles (as kh-er's HUD).
struct Layer {
    draw: Draw,
    renderer: kh_ui::Renderer,
    hash: u64,
    rect: (i32, i32, usize, usize),
    first: bool,
}

impl Layer {
    fn frame(&mut self, movie: &kh_ui::Movie) {
        let hash = kh_ui::render::state_hash(movie);
        if hash == self.hash && !self.first {
            return;
        }
        self.hash = hash;
        let root = kh_ui::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        let perf = Perf { split_dirty: true, bands: 1, band_pixels: 200_000 };
        if let Drawn::Picture(rect, _) = draw_movie_with(&mut self.draw, &mut self.renderer, movie, &root, (1920.0, 1080.0), self.first, perf) {
            self.rect = rect;
        }
        self.first = false;
    }
}

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
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/synth_list/shots");
    std::fs::create_dir_all(&dir).ok();
    kh_ui::def::write_png(&dir.join(format!("{name}.png")), 1920, 1080, &out).unwrap();
}

struct Rig {
    view: WorkshopView,
    menu: WorkshopMenu,
    layer: Layer,
    fonts: kh_ui::Fonts,
    sounds: Vec<kh_ui::UiSound>,
}

impl Rig {
    fn step(&mut self, frames: usize) {
        for _ in 0..frames {
            self.view.sync(&mut self.menu);
            self.view.tick(Some(&mut self.menu));
            self.layer.frame(&self.view.movie);
        }
    }

    fn key(&mut self, key: CampKey) {
        for _ in 0..60 {
            if self.view.ready() {
                break;
            }
            self.step(1);
        }
        self.menu.key(key, false, &mut self.sounds);
        self.step(30);
    }

    /// Every visible row slot holds the material of its place on its own grid cell; the HUD's picture is a fresh one.
    fn check(&self, what: &str) {
        assert_eq!(self.menu.screen, Screen::Materials, "{what}: on the Material List");
        let rows = MATERIAL_COLUMNS * MATERIAL_LINES;
        let first = self.menu.material_line * MATERIAL_COLUMNS;
        let mut cells: Vec<(i32, i32)> = Vec::new();
        for slot in 0..rows {
            let path = format!("{MATERIAL_LIST}.container.i{slot}");
            let node = self.view.movie.node(&path).unwrap_or_else(|| panic!("{what}: {path} exists"));
            let index = first + slot;
            if index >= self.menu.data.materials.len() {
                assert!(!node.is_visible(), "{what}: slot {slot} past the end is hidden");
                continue;
            }
            assert!(node.is_visible(), "{what}: slot {slot} shows material {index}");
            let name = self.view.movie.text(&format!("{path}.itemLabel.txName.textField")).unwrap_or_default();
            assert_eq!(name, self.menu.data.materials[index].name, "{what}: slot {slot}'s name");
            let stock = self.view.movie.text(&format!("{path}.numShojisuu.textField")).unwrap_or_default();
            assert_eq!(stock, self.menu.data.materials[index].stock.to_string(), "{what}: slot {slot}'s stock");
            let cell = (node.matrix.tx.round() as i32, node.matrix.ty.round() as i32);
            assert!(!cells.contains(&cell), "{what}: slot {slot} drawn on another row's place {cell:?} (the user's overlap)");
            cells.push(cell);
        }
        // the grid: two columns 768 apart, the lines one pitch apart and the same pitch on every visit
        let at = |slot: usize| self.view.movie.node(&format!("{MATERIAL_LIST}.container.i{slot}")).map(|n| (n.matrix.tx, n.matrix.ty)).unwrap();
        let ((x0, y0), (x1, y1), (_, y2)) = (at(0), at(1), at(2));
        assert!((x1 - x0 - 768.0).abs() < 0.5 && (y1 - y0).abs() < 0.5, "{what}: i1 stands right of i0 ({x0},{y0}) ({x1},{y1})");
        assert!(y2 - y0 > 20.0, "{what}: line 2 stands below line 1 (pitch {})", y2 - y0);
        let (whole, rect) = fresh(&self.view.movie, &self.fonts);
        assert_eq!(rect, self.layer.rect, "{what}: the layer's rectangle");
        let n_diff = self.layer.draw.rgba.as_chunks::<4>().0.iter().zip(whole.as_chunks::<4>().0).filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 1)).count();
        assert!(n_diff * 10_000 <= whole.len() / 4, "{what}: {n_diff} pixel(s) of the HUD's picture are stale");
    }

    fn visit(&mut self, visit: usize) {
        // the top: Synthesize Items .. Material List (the fifth command)
        for _ in 0..5 {
            if self.menu.data.commands.get(self.menu.top).map(|c| c.0) == Some(Screen::Materials) {
                break;
            }
            self.key(CampKey::Down);
        }
        self.key(CampKey::Decide);
        self.step(20);
        self.check(&format!("visit {visit}: in"));
        if visit == 2 {
            save(&self.layer.draw.rgba, self.layer.rect, "materials_visit2_in");
        }
        for k in 0..3 {
            self.key(CampKey::Down);
            self.check(&format!("visit {visit}: scroll down {k}"));
        }
        self.key(CampKey::R1);
        self.check(&format!("visit {visit}: RB"));
        if visit == 2 {
            save(&self.layer.draw.rgba, self.layer.rect, "materials_visit2_rb");
        }
        self.key(CampKey::R1);
        self.check(&format!("visit {visit}: RB again"));
        self.key(CampKey::Up);
        self.check(&format!("visit {visit}: scroll up"));
        self.key(CampKey::L1);
        self.check(&format!("visit {visit}: LB"));
        self.key(CampKey::L1);
        self.check(&format!("visit {visit}: LB again"));
        self.key(CampKey::Cancel);
        self.step(20);
        assert_eq!(self.menu.screen, Screen::Top, "visit {visit}: B back to the top");
    }
}

#[test]
fn the_material_list_keeps_its_grid_on_every_visit() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    let Ok(mut pack) = kh_ui::Pack::open(&dir) else {
        eprintln!("SKIPPED: no UI pack");
        return;
    };
    let Ok(fonts) = kh_ui::Fonts::load(&mut pack) else { return };
    let Ok(view) = WorkshopView::load(&mut pack, fonts.clone()) else {
        eprintln!("SKIPPED: no Shop movie");
        return;
    };
    let menu = WorkshopMenu::new(data(), Entry::Workshop);
    let layer = Layer { draw: Draw::default(), renderer: kh_ui::Renderer::new(fonts.clone()), hash: 0, rect: (0, 0, 0, 0), first: true };
    let mut rig = Rig { view, menu, layer, fonts, sounds: Vec::new() };
    rig.view.open(&mut rig.menu);
    rig.step(50);
    rig.visit(1);
    rig.visit(2);
    // the next talk to the Moogle: the same view, the menu closed and opened again
    rig.menu.key(CampKey::Cancel, false, &mut rig.sounds);
    rig.view.close();
    for _ in 0..120 {
        if rig.view.tick(Some(&mut rig.menu)) {
            break;
        }
    }
    rig.menu = WorkshopMenu::new(data(), Entry::Workshop);
    rig.view.open(&mut rig.menu);
    rig.layer.first = true;
    rig.step(50);
    rig.visit(3);
}
