//! (INTERACT-MARK) KH3's reaction-target marker: the type-5 ("action") target cursor of `HudTarget`, clip
//! `ActLockonMark` (child `triAct`), which `ATresPlayerControllerBase` 0x1413F7410 adds on the reaction command's
//! `UTresReactorComponent` (`0x1415759C0(cockpit, reactor, 5, false, -1)`) and the projection pass puts at the
//! reactor's world location (vt+0xB88 = 0x140ACF250: ComponentToWorld translation). Its glow is a BLUR filter of the
//! movie (depth 23 of sprite 84: box 52 px in the loop), from the staged `filters/HudTarget.json`
//! (tools/sora_ui_filters.js). Pictures: build/sora_tmp/weapons/INTERACT-MARK/shots/.

use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3};
use kh_ui::def::{read_png, write_png};
use kh_ui::geom::Matrix;
use kh_ui::hud_target::{HudTarget, TargetDriver, View, LOCKON_POOL, LOCKON_TYPE_ACTION};
use kh_ui::{Canvas, Fonts, Movie, Pack, Renderer};

const DT: f32 = 1.0 / 60.0;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/INTERACT-MARK")
}

/// The pack, with the staged pack part (the movie's filters) over it when it is there.
fn pack() -> Option<Pack> {
    let mut pack = Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()?;
    pack.overlays.push(root().join("stage/ui"));
    Some(pack)
}

fn shots() -> PathBuf {
    let dir = root().join("shots");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// The movie drawn over a flat ground, cropped to `(x, y, w, h)` of the 1920 x 1080 stage and scaled `zoom` times.
fn shot(name: &str, movie: &Movie, fonts: &Fonts, crop: (usize, usize, usize, usize), zoom: usize) -> Vec<u8> {
    let mut canvas = Canvas::new(1920, 1080);
    let mut renderer = Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.42, 0.40, 0.36], &mut rgba);
    let (cx, cy, cw, ch) = crop;
    let mut out = vec![0u8; cw * zoom * ch * zoom * 4];
    for y in 0..ch * zoom {
        for x in 0..cw * zoom {
            let s = ((cy + y / zoom) * 1920 + cx + x / zoom) * 4;
            let d = (y * cw * zoom + x) * 4;
            out[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
        }
    }
    write_png(&shots().join(format!("{name}.png")), (cw * zoom) as u32, (ch * zoom) as u32, &out).unwrap();
    out
}

fn view() -> View {
    View { clip_from_world: Mat4::perspective_infinite_reverse_rh(60f32.to_radians(), 16.0 / 9.0, 0.1), rect: (0.0, 0.0, 1280.0, 720.0) }
}

fn green(p: &[u8]) -> bool {
    p[1] as i32 > p[0] as i32 + 40 && p[1] as i32 > p[2] as i32 + 40
}

/// The marker's own clip: its labels and its look (the green triangle and its blurred core), in, loop and gone without
/// an out animation.
#[test]
fn interact_mark_is_kh3s_green_action_cursor() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let def = pack.movie("HudTarget").unwrap();
    let mut hud = HudTarget::new(def.clone(), fonts.clone());
    hud.native_root("loop");
    assert_eq!(LOCKON_POOL[LOCKON_TYPE_ACTION], 2, "m_nLockonCursorTypeMax[5]");
    // the asset: ActLockonMark (symbol 84) > triAct (81), its core disc (82: r 10, #13FF08) at depths 21 / 23
    let id = *def.symbols.get("ActLockonMark").expect("ActLockonMark");
    assert_eq!(id, 84);
    let staged = root().join("stage/ui/filters/HudTarget.json").is_file();
    if let Some(kh_ui::def::Char::Sprite(s)) = def.chars.get(&id) {
        assert_eq!(s.label_frame("loop"), Some(55));
        if staged {
            let b = s.blur_at(100, 23).expect("the loop's blur");
            assert_eq!((b.x, b.y, b.passes), (52.0, 52.0, 1), "KH3's BlurFilter on the core");
            assert!(s.blur_at(7, 23).is_none(), "frame 7: a zero box is no blur");
            assert!(s.blur_at(100, 21).is_none(), "the sharp core has none");
        }
    } else {
        panic!("ActLockonMark is not a sprite");
    }
    let mut driver = TargetDriver::default();
    let mut sounds = Vec::new();
    let position_of = |_key: u64| Some(Vec3::new(0.0, 0.0, -5.0));
    assert!(driver.add_cursor(&mut hud, 1, LOCKON_TYPE_ACTION, false, &mut sounds).is_some());
    assert!(sounds.is_empty(), "the action cursor is silent: {sounds:?}");
    let path = kh_ui::hud_target::cursor_path(LOCKON_TYPE_ACTION, 0);
    assert_eq!(hud.movie.current_label(&path), Some("in"));
    let mut greens = Vec::new();
    for frame in 0..=120 {
        driver.update(&mut hud, DT, Some(&view()), &position_of, &mut sounds);
        if [4, 12, 24, 46, 60, 90, 120].contains(&frame) {
            let px = shot(&format!("act_cursor_f{frame:03}"), &hud.movie, &fonts, (960 - 80, 540 - 80, 160, 160), 3);
            greens.push((frame, px.chunks(4).filter(|p| green(p)).count()));
        }
        hud.tick();
    }
    eprintln!("green pixels by frame: {greens:?}; label now {:?}", hud.movie.current_label(&path));
    assert!(greens.iter().filter(|(f, _)| *f >= 46).all(|(_, g)| *g > 200), "a green mark in its loop: {greens:?}");
    let node = hud.movie.node(&path).unwrap();
    assert_eq!((node.matrix.tx, node.matrix.ty, node.is_visible()), (960.0, 540.0, true));
    // removed: removeChild at once (0x14166C730: only attraction marks play `out`)
    driver.remove_cursor(&mut hud, 1, LOCKON_TYPE_ACTION, &mut sounds);
    assert!(!hud.movie.exists(&path) && sounds.is_empty());
}

/// BESIDE KH3'S OWN FRAME (the user's picture): the marker drawn at that picture's scale (its width / 1920) onto the
/// picture itself, a little to the right of KH3's own marker, both zoomed: `shots/vs_kh3_reference.png`.
#[test]
fn interact_mark_beside_the_users_kh3_picture() {
    let Some(mut pack) = pack() else { return };
    let Ok((rw, rh, reference)) = read_png(&root().join("user_kh3_green_marker.png")) else {
        eprintln!("SKIPPED: no reference picture");
        return;
    };
    let (rw, rh) = (rw as usize, rh as usize);
    // KH3's marker in the picture: the middle of its green pixels
    let (mut sx, mut sy, mut n) = (0usize, 0usize, 0usize);
    for (i, p) in reference.chunks(4).enumerate() {
        if p[1] > 110 && green(p) {
            (sx, sy, n) = (sx + i % rw, sy + i / rw, n + 1);
        }
    }
    assert!(n > 10, "KH3's marker found in the picture ({n} px)");
    let (kx, ky) = ((sx / n) as f32, (sy / n) as f32);
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut hud = HudTarget::new(pack.movie("HudTarget").unwrap(), fonts.clone());
    hud.native_root("loop");
    let mut driver = TargetDriver::default();
    let mut sounds = Vec::new();
    driver.add_cursor(&mut hud, 1, LOCKON_TYPE_ACTION, false, &mut sounds);
    let position_of = |_key: u64| Some(Vec3::new(0.0, 0.0, -5.0));
    for _ in 0..100 {
        driver.update(&mut hud, DT, Some(&view()), &position_of, &mut sounds);
        hud.tick();
    }
    // ours at (kx + 60, ky) of the picture: the stage scaled by its width / 1920
    let k = rw as f32 / 1920.0;
    let (ox, oy) = (kx + 60.0, ky);
    let m = Matrix { a: k, b: 0.0, c: 0.0, d: k, tx: ox - 960.0 * k, ty: oy - 540.0 * k };
    let mut canvas = Canvas::new(rw, rh);
    Renderer::new(fonts.clone()).draw(&mut canvas, &hud.movie, &m);
    let mut ours = Vec::new();
    canvas.to_rgba8_over(|x, y| { let p = &reference[(y * rw + x) * 4..]; [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0] }, &mut ours);
    // the region around both, zoomed 4x: KH3's marker on the left, ours on the right
    let (cw, ch, zoom) = (140usize, 70usize, 4usize);
    let (cx0, cy0) = ((kx as usize).saturating_sub(40), (ky as usize).saturating_sub(35));
    let mut out = vec![0u8; cw * zoom * ch * zoom * 4];
    for y in 0..ch * zoom {
        for x in 0..cw * zoom {
            let (px, py) = ((cx0 + x / zoom).min(rw - 1), (cy0 + y / zoom).min(rh - 1));
            let d = (y * cw * zoom + x) * 4;
            out[d..d + 4].copy_from_slice(&ours[(py * rw + px) * 4..(py * rw + px) * 4 + 4]);
        }
    }
    write_png(&shots().join("vs_kh3_reference.png"), (cw * zoom) as u32, (ch * zoom) as u32, &out).unwrap();
    write_png(&shots().join("vs_kh3_reference_full.png"), rw as u32, rh as u32, &ours).unwrap();
    // ours is there, green, about KH3's size (the green spans of both, in the picture's pixels)
    let span = |img: &[u8], x0: usize, x1: usize| {
        let mut xs = Vec::new();
        for y in cy0..(cy0 + ch).min(rh) {
            for x in x0..x1.min(rw) {
                if green(&img[(y * rw + x) * 4..]) && img[(y * rw + x) * 4 + 1] > 110 {
                    xs.push(x);
                }
            }
        }
        xs.iter().max().zip(xs.iter().min()).map_or(0, |(a, b)| a - b)
    };
    let (theirs_w, ours_w) = (span(&reference, cx0, (kx + 30.0) as usize), span(&ours, (kx + 30.0) as usize, cx0 + cw));
    eprintln!("KH3's marker at ({kx:.0}, {ky:.0}) of the picture, {theirs_w} px wide; ours {ours_w} px wide");
    assert!(ours_w > 4, "ours is drawn");
}
