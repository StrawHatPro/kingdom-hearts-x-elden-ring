//! (F-HUD) BEFORE / AFTER THE LAYER-BLEND FIX (kh_ui render.rs: Flash's LAYER / ERASE / ALPHA, `set_layer_blends`): the
//! screens whose movies use them (CampMenu, HudLeft, HudRight's NaviMap, ItemStatusLibrary, KeyItem) through the REAL
//! `Hud`, every layer of `hud_layers::LAYERS` drawn by the kh_ui software renderer with the fix off (left) and on (right).
//! Pictures: build/sora_tmp/weapons/F-HUD/shots/layerfix/*.png (to be looked at), with the changed pixel count and box.

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::{stage_root, LAYERS};
use kh_ui::camp::TopEntry;

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn frames(hud: &mut Hud, sora: &Sora, n: u32, words: &[&str]) {
    let snapshot = sora.ui_snapshot();
    for k in 0..n {
        let w: Vec<String> = if k == 0 { words.iter().map(|s| s.to_string()).collect() } else { Vec::new() };
        hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 3725, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

const W: usize = 1920;
const H: usize = 1080;

fn draw(hud: &Hud, fonts: &kh_ui::Fonts, on: bool) -> Vec<u8> {
    kh_ui::render::set_layer_blends(on);
    let (root, _) = stage_root(W as f32, H as f32);
    let mut canvas = kh_ui::Canvas::new(W, H);
    let mut renderer = kh_ui::Renderer::new(fonts.clone());
    for name in LAYERS {
        if let Some(movie) = hud.movie(name) {
            renderer.draw(&mut canvas, movie, &root);
        }
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    kh_ui::render::set_layer_blends(true);
    rgba
}

/// Before | after, side by side (half size each), and the changed pixels' count and box (full size).
fn compare(hud: &Hud, fonts: &kh_ui::Fonts, out: &std::path::Path, name: &str) -> (usize, Option<(usize, usize, usize, usize)>) {
    let (a, b) = (draw(hud, fonts, false), draw(hud, fonts, true));
    let (mut n, mut bx) = (0, None::<(usize, usize, usize, usize)>);
    for y in 0..H {
        for x in 0..W {
            let i = (y * W + x) * 4;
            if (0..3).any(|c| (a[i + c] as i32 - b[i + c] as i32).abs() > 8) {
                n += 1;
                bx = Some(bx.map_or((x, y, x, y), |(x0, y0, x1, y1)| (x0.min(x), y0.min(y), x1.max(x), y1.max(y))));
            }
        }
    }
    let (hw, hh) = (W / 2, H / 2);
    let mut side = vec![0u8; hw * 2 * hh * 4];
    for y in 0..hh {
        for x in 0..hw {
            for (k, src) in [&a, &b].into_iter().enumerate() {
                let s = (y * 2 * W + x * 2) * 4;
                let d = (y * hw * 2 + k * hw + x) * 4;
                side[d..d + 4].copy_from_slice(&src[s..s + 4]);
            }
        }
    }
    kh_ui::def::write_png(&out.join(format!("{name}.png")), (hw * 2) as u32, hh as u32, &side).unwrap();
    // The changed region at full size, before over after, when there is one.
    if let Some((x0, y0, x1, y1)) = bx {
        let (cw, ch) = ((x1 - x0 + 1).min(900), (y1 - y0 + 1).min(500));
        let mut crop = vec![0u8; cw * ch * 2 * 4];
        for (k, src) in [&a, &b].into_iter().enumerate() {
            for y in 0..ch {
                let s = ((y0 + y) * W + x0) * 4;
                let d = ((k * ch + y) * cw) * 4;
                crop[d..d + cw * 4].copy_from_slice(&src[s..s + cw * 4]);
            }
        }
        kh_ui::def::write_png(&out.join(format!("{name}_diff.png")), cw as u32, (ch * 2) as u32, &crop).unwrap();
    }
    eprintln!("layerfix {name}: {n} pixels changed, box {bx:?}");
    (n, bx)
}

fn sora() -> Sora {
    Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)))
}

#[test]
fn the_layer_blend_fix_before_and_after_on_the_screens_that_use_it() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok((mut hud, fonts)) = Hud::load(&root.join("build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let out = root.join("build/sora_tmp/weapons/F-HUD/shots/layerfix");
    let _ = std::fs::create_dir_all(&out);
    let sora = sora();
    frames(&mut hud, &sora, 120, &[]);
    // The camp menu: the top and its main pages (Items = ItemStatusLibrary's panes).
    assert!(hud.open_camp_menu());
    frames(&mut hud, &sora, 60, &[]);
    compare(&hud, &fonts, &out, "05_camp_top");
    let entries = hud.camp_menu().map(|c| c.entries.clone()).unwrap_or_default();
    for (k, page) in [TopEntry::Equipment, TopEntry::Abilities, TopEntry::Items, TopEntry::Customize, TopEntry::Stats, TopEntry::Config].into_iter().enumerate() {
        let Some(at) = entries.iter().position(|e| *e == page) else { continue };
        for _ in 0..at {
            frames(&mut hud, &sora, 4, &["menu down"]);
        }
        frames(&mut hud, &sora, 50, &["menu decide"]);
        compare(&hud, &fonts, &out, &format!("{:02}_camp_{page:?}", 6 + k));
        if page == TopEntry::Items {
            // the item's status pane with a row in focus
            frames(&mut hud, &sora, 30, &["menu decide"]);
            compare(&hud, &fonts, &out, "06x_camp_items_pane");
            frames(&mut hud, &sora, 20, &["menu cancel"]);
        }
        frames(&mut hud, &sora, 40, &["menu cancel"]);
        for _ in 0..at {
            frames(&mut hud, &sora, 4, &["menu up"]);
        }
        frames(&mut hud, &sora, 10, &[]);
    }
    // out of the camp menu (back from the top), then the cockpit
    for _ in 0..6 {
        if !hud.paused() {
            break;
        }
        frames(&mut hud, &sora, 40, &["menu cancel"]);
    }
    assert!(!hud.paused(), "the camp menu closed");
    frames(&mut hud, &sora, 120, &[]);
    // The cockpit in the field and in battle (HudLeft / HudRight corners, the NaviMap).
    frames(&mut hud, &sora, 120, &[]);
    compare(&hud, &fonts, &out, "01_field");
    frames(&mut hud, &sora, 90, &["battle 2"]);
    compare(&hud, &fonts, &out, "02_battle");
    // HudLeft's item popup.
    let id = |key: &str| kh_host::items::by_key(key).unwrap().item_id;
    assert!(kh_host::items::give(id("MAT_ITEM01"), 2));
    frames(&mut hud, &sora, 25, &[]);
    compare(&hud, &fonts, &out, "03_item_popup");
    frames(&mut hud, &sora, 400, &["battle live"]);
    // KeyItem: KH3's OBTAINED popup.
    hud.obtained_on = true;
    assert!(hud.obtained_weapon("WEP_KEYBLADE_SO_01"), "the KeyItem movie");
    frames(&mut hud, &sora, 70, &[]);
    compare(&hud, &fonts, &out, "04_key_item");
    eprintln!("pictures: {}", out.display());
}
