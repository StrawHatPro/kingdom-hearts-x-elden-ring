//! (HB) SORA'S STATUS GAUGES AGAINST THE FRIENDS', THROUGH THE ELDEN RING HOST'S LAYER PATH, OFFLINE. The status HUD
//! (Sora + Donald + Goofy) is drawn by `kh_host::hud_layers::draw_movie_in` at the stage root the Elden Ring host uses,
//! clipped to the stage, as straight-alpha sRGB bytes (what kh-er hands the overlay), then laid over a dark, a mid and a
//! bright scene the way kh-er-render's overlay does on the user's 10-bit back buffer (format 24: display-encoded
//! blending, ONE / INV_SRC_ALPHA of colour * alpha). Each picture is made with the add / screen coverage rule before
//! X2-b (`set_legacy_add_coverage(true)`) and the current one.
//!
//!   cargo run -p kh-host --release --example hb_gauge_check -- OUT_DIR [x2legacy|before|after|all|tree|hide]
//!
//! Modes: x2legacy = the add / screen coverage rule before X2-b, masks as before; before = what the live build draws;
//! after = solid sprite masks (HB). Prints per gauge (its pixels = those that change when it is hidden) the mean colour
//! over each scene, and writes OUT_DIR/<mode>_<w>x<h>_<scene>.png and zoom_*.png (cropped to the HUD). `tree` prints
//! the live display tree (blend, colour transform, fills), `hide` the colour each node gives.

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::{draw_movie_in, stage_root, Draw, Drawn, Perf};
use kh_host::party::{PartyChara, PartyHudSlot};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn walk(s: &kh_ui::player::Sprite, path: &str) {
    for n in s.children.iter().chain(&s.extra) {
        let p = format!("{path}.{}", n.name.as_deref().map(str::to_string).unwrap_or_else(|| format!("#{}", n.ch)));
        let vis = if n.is_visible() { "" } else { " HIDDEN" };
        let cx = if n.cx.mul != [1.0; 4] || n.cx.add != [0.0; 4] { format!(" cx mul {:?} add {:?}", n.cx.mul, n.cx.add) } else { String::new() };
        let kind = match &n.kind {
            kh_ui::player::Kind::Shape(sh) => {
                let fills: Vec<String> = sh.fills.iter().map(|f| match &f.paint {
                    kh_ui::def::Paint::Solid(c) => format!("solid {c:?}"),
                    kh_ui::def::Paint::Gradient { radial, stops, .. } => format!("grad r{radial} {stops:?}"),
                    kh_ui::def::Paint::Bitmap(b) => {
                        let t = &b.texture;
                        // mean colour of the texture's opaque texels
                        let (mut n, mut s) = (0f32, [0f32; 3]);
                        for p in t.rgba.chunks_exact(4) {
                            if p[3] > 128 {
                                n += 1.0;
                                for c in 0..3 {
                                    s[c] += p[c] as f32;
                                }
                            }
                        }
                        if std::env::var("HB_TEXELS").is_ok() {
                            for (x, y) in [(12u32, 60u32), (20, 60), (60, 8), (60, 14), (300, 8), (300, 14), (20, 190), (60, 140), (120, 135), (70, 195), (65, 185), (165, 150), (160, 140), (60, 60), (330, 120), (450, 200)] {
                                let i = ((y * t.width + x) * 4) as usize;
                                println!("TEXEL {} ({x},{y}) {:?}", t.file, &t.rgba[i..i + 4]);
                            }
                        }
                        format!("bitmap {} {}x{} mean ({:.0},{:.0},{:.0})", t.file, t.width, t.height, s[0] / n.max(1.0), s[1] / n.max(1.0), s[2] / n.max(1.0))
                    }
                    kh_ui::def::Paint::None => "none".to_string(),
                }).collect();
                format!("shape {}", fills.join("; "))
            }
            kh_ui::player::Kind::Image(_) => "image".to_string(),
            kh_ui::player::Kind::Sprite(c) => format!("sprite {} f{}/{}", c.def.class.as_deref().unwrap_or("-"), c.frame, c.def.frames),
            kh_ui::player::Kind::Text(_) => "text".to_string(),
            _ => "other".to_string(),
        };
        println!("{p} | {kind} | blend {} clip {}{cx}{vis}", n.blend, n.clip);
        if let kh_ui::player::Kind::Sprite(c) = &n.kind {
            if n.is_visible() {
                walk(c, &p);
            }
        }
    }
}

const SCENES: [(&str, [f32; 3]); 3] = [("dark", [0.05, 0.05, 0.06]), ("mid", [0.38, 0.36, 0.32]), ("bright", [0.86, 0.85, 0.80])];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = args.first().map(std::path::PathBuf::from).unwrap_or_else(|| root.join("build/sora_tmp/er/hb/frames"));
    let which = args.get(1).cloned().unwrap_or_else(|| "both".into());
    std::fs::create_dir_all(&out).unwrap();
    let (mut hud, fonts) = Hud::load(&root.join("build/sora_pack/ui")).expect("the UI pack");
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let slots = [
        PartyHudSlot { chara: PartyChara::Donald, hp: 140, hp_max: 200, mp: 70, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
        PartyHudSlot { chara: PartyChara::Goofy, hp: 160, hp_max: 220, mp: 100, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
    ];
    hud.set_party(&slots);
    hud.set_leader(PartyChara::Sora, true);
    let mut snapshot = sora.ui_snapshot();
    // part-full gauges, the way a fight shows them
    let g = &mut snapshot.gauges;
    g.hp = g.hp_max * 7 / 10;
    g.mp = g.mp_max * 6 / 10;
    g.focus = g.focus_max / 2;
    for _ in 0..150 {
        hud.frame(&mut Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
    let movie = hud.movie("HudStatus").expect("HudStatus");
    if which == "hide" {
        let (stage, view) = stage_root(2560.0, 1440.0);
        let mut full: Vec<u8> = Vec::new();
        for hide in ["", "sttsPc.pcHp.pcHpPack.pcHpGgSet.pcHpGgR", "sttsPc.pcHp.pcHpPack.pcHpGgSet.pcHpDodai", "sttsPc.pcHp.pcHpPack.pcFaceDodai", "sttsPc.pcHp.pcHpPack.poPcFace", "sttsPc.pcEx.pcFocusGg.pcExGgR", "sttsPc.pcEx.pcFocusGg.pcExDodai", "sttsPc.pcHp.pcHpPack.pcHpGgSet.pcHpGg", "sttsPc.pcEx.pcFocusGg.pcExGg", "sttsPc.pcHpEff"] {
            let mut m = movie.clone();
            if !hide.is_empty() {
                assert!(m.set_visible(hide, false), "{hide}");
            }
            let mut draw = Draw::default();
            let mut renderer = kh_ui::render::Renderer::new(fonts.clone());
            let Drawn::Picture(rect, _) = draw_movie_in(&mut draw, &mut renderer, &m, &stage, (2560.0, 1440.0), true, Perf::default(), Some(view)) else { panic!() };
            let _ = rect;
            if hide.is_empty() {
                full = draw.rgba.clone();
                continue;
            }
            // the pixels this node changes: their colour in the full picture and without it
            let (mut n, mut a, mut b) = (0f32, [0f32; 4], [0f32; 4]);
            let mut sample = String::new();
            for (k, (p, q)) in full.chunks_exact(4).zip(draw.rgba.chunks_exact(4)).enumerate() {
                if p.iter().zip(q).any(|(x, y)| x.abs_diff(*y) > 24) {
                    n += 1.0;
                    for c in 0..4 {
                        a[c] += p[c] as f32;
                        b[c] += q[c] as f32;
                    }
                    if sample.is_empty() && n > 200.0 {
                        sample = format!("first at ({}, {})", k % rect.2, k / rect.2);
                    }
                }
            }
            let n1 = n.max(1.0);
            println!("hide {hide:50}: {n} px; full ({:.0},{:.0},{:.0},{:.0}) without ({:.0},{:.0},{:.0},{:.0}) {sample}", a[0] / n1, a[1] / n1, a[2] / n1, a[3] / n1, b[0] / n1, b[1] / n1, b[2] / n1, b[3] / n1);
        }
        return;
    }
    if which == "tree" {
        walk(&movie.root, "");
        return;
    }
    // (mode name, X2-b's old add / screen coverage rule, solid sprite masks (HB))
    let modes: Vec<(&str, bool, bool)> = match which.as_str() {
        "x2legacy" => vec![("x2legacy", true, false)],
        "before" => vec![("before", false, false)],
        "after" => vec![("after", false, true)],
        _ => vec![("x2legacy", true, false), ("before", false, false), ("after", false, true)],
    };
    // the gauges measured: their pixels = what changes when the gauge is hidden
    let gauges = [
        ("sora_hp", "sttsPc.pcHp.pcHpPack.pcHpGgSet.pcHpGg"),
        ("sora_mp", "sttsPc.pcMp.pcMpPack.pcMpGg"),
        ("sora_focus", "sttsPc.pcEx.pcFocusGg.pcExGg"),
        ("donald_hp", "sttsNpc1.sttsNpc.npcHp.npcHpGg"),
        ("donald_mp", "sttsNpc1.sttsNpc.npcMp.npcMpGg"),
    ];
    let picture = |m: &kh_ui::Movie, w: usize, h: usize| -> ((i32, i32, usize, usize), Vec<u8>) {
        let (stage, view) = stage_root(w as f32, h as f32);
        let mut draw = Draw::default();
        let mut renderer = kh_ui::render::Renderer::new(fonts.clone());
        match draw_movie_in(&mut draw, &mut renderer, m, &stage, (w as f32, h as f32), true, Perf::default(), Some(view)) {
            Drawn::Picture(rect, _) => (rect, draw.rgba),
            _ => panic!("nothing drawn"),
        }
    };
    // (sheet) per output size: one row per scene, one column per mode
    let mut sheet: Vec<(String, (usize, usize), Vec<u8>)> = Vec::new();
    for (w, h) in [(1920usize, 1080usize), (2560, 1440)] {
        sheet.clear();
        // the gauge pixels, found once with the fix on (full strength: every gauge pixel differs when hidden)
        kh_ui::render::set_solid_masks(true);
        let (_, full) = picture(movie, w, h);
        let masks: Vec<Vec<bool>> = gauges
            .iter()
            .map(|(_, path)| {
                let mut m = movie.clone();
                assert!(m.set_visible(path, false), "{path}");
                let (_, without) = picture(&m, w, h);
                full.chunks_exact(4).zip(without.chunks_exact(4)).map(|(p, q)| p[3] > 200 && p.iter().zip(q).any(|(x, y)| x.abs_diff(*y) > 24)).collect()
            })
            .collect();
        for (mode, legacy, solid) in &modes {
            kh_ui::raster::set_legacy_add_coverage(*legacy);
            kh_ui::render::set_solid_masks(*solid);
            let (rect, rgba) = picture(movie, w, h);
            kh_ui::raster::set_legacy_add_coverage(false);
            kh_ui::render::set_solid_masks(true);
            let _ = kh_ui::def::write_png(&out.join(format!("{mode}_{w}x{h}_alpha.png")), rect.2 as u32, rect.3 as u32, &rgba);
            for (scene, bg) in SCENES.iter() {
                // the overlay's display-encoded blend (the user's 10-bit back buffer): c * a + bg * (1 - a)
                let mut img = vec![0u8; rect.2 * rect.3 * 4];
                for (i, p) in rgba.chunks_exact(4).enumerate() {
                    let a = p[3] as f32 / 255.0;
                    for c in 0..3 {
                        img[i * 4 + c] = ((p[c] as f32 / 255.0 * a + bg[c] * (1.0 - a)) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
                    }
                    img[i * 4 + 3] = 255;
                }
                let _ = kh_ui::def::write_png(&out.join(format!("{mode}_{w}x{h}_{scene}.png")), rect.2 as u32, rect.3 as u32, &img);
                sheet.push((format!("{mode} {scene}"), (rect.2, rect.3), img.clone()));
                // 3x zoom (nearest)
                let z = if w == 1920 { 3 } else { 2 };
                let (zw, zh) = (rect.2 * z, rect.3 * z);
                let mut big = vec![0u8; zw * zh * 4];
                for y in 0..zh {
                    for x in 0..zw {
                        let i = ((y / z) * rect.2 + x / z) * 4;
                        big[(y * zw + x) * 4..(y * zw + x) * 4 + 4].copy_from_slice(&img[i..i + 4]);
                    }
                }
                let _ = kh_ui::def::write_png(&out.join(format!("zoom_{mode}_{w}x{h}_{scene}.png")), zw as u32, zh as u32, &big);
                let mut line = format!("{mode:8} {w}x{h} {scene:6}");
                for ((name, _), mask) in gauges.iter().zip(&masks) {
                    let (mut n, mut sum, mut sat) = (0f32, [0f32; 3], 0f32);
                    for (k, on) in mask.iter().enumerate() {
                        if !on {
                            continue;
                        }
                        n += 1.0;
                        let p = &img[k * 4..k * 4 + 3];
                        for c in 0..3 {
                            sum[c] += p[c] as f32;
                        }
                        let (mx, mn) = (p[0].max(p[1]).max(p[2]) as f32, p[0].min(p[1]).min(p[2]) as f32);
                        sat += if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
                    }
                    let n1 = n.max(1.0);
                    line += &format!(" | {name} {n:.0}px ({:.0},{:.0},{:.0}) sat {:.2}", sum[0] / n1, sum[1] / n1, sum[2] / n1, sat / n1);
                }
                println!("{line}");
            }
        }
        if !sheet.is_empty() {
            let (pw, ph) = sheet[0].1;
            let cols = modes.len();
            let rows = SCENES.len();
            let (sw, sh) = (pw * cols + 8 * (cols - 1), ph * rows + 8 * (rows - 1));
            let mut px = vec![255u8; sw * sh * 4];
            for (k, (_, (iw, ih), img)) in sheet.iter().enumerate() {
                let (col, row) = (k / rows, k % rows);
                for y in 0..*ih {
                    let to = ((row * (ph + 8) + y) * sw + col * (pw + 8)) * 4;
                    px[to..to + iw * 4].copy_from_slice(&img[y * iw * 4..(y + 1) * iw * 4]);
                }
            }
            let names: Vec<&str> = modes.iter().map(|m| m.0).collect();
            let _ = kh_ui::def::write_png(&out.join(format!("sheet_{w}x{h}_{}.png", names.join("_"))), sw as u32, sh as u32, &px);
        }
    }
}
