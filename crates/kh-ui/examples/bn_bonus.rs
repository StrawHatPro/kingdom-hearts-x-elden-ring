//! (BN) THE KH3 BONUS! / LEVEL UP! PANELS, OFFLINE: HudRight's `getBonus` / `levelUp` windows drawn at 1920 x 1080,
//! cropped to the right-hand stack, with the windows filled as kh-host's `Hud` fills them (roster colour + MenuFace
//! portrait + KH3 icon tags).
//!   cargo run -p kh-ui --release --example bn_bonus -- OUT_DIR [SCENE] [FRAMES] [dump FILTER]
//! SCENE: ref (Sora "Flash Step" + Goofy "Maximum HP increased!"), donald_spell, sora_tier, levelup, all, slide
use kh_ui::hud_side::{icon_line, panel_colour, HudRight, LineIcon, PanelChara, SideStrings};
use kh_ui::player::{Kind, Sprite};
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer};

fn walk(s: &Sprite, path: &str, filter: &str, m: &Matrix) {
    for n in s.children.iter().chain(&s.extra) {
        let p = format!("{path}.{}", n.name.as_deref().map(str::to_string).unwrap_or_else(|| format!("#{}", n.ch)));
        let w = m.mul(&n.matrix);
        let vis = if n.is_visible() { "" } else { " HIDDEN" };
        let kind = match &n.kind {
            Kind::Shape(_) => "shape".to_string(),
            Kind::Image(_) => "image".to_string(),
            Kind::Sprite(c) => format!("sprite {} f{}/{} gfx {} icon {:?}", c.def.class.as_deref().unwrap_or("-"), c.frame, c.def.frames, c.gfx.len(), c.icon.map(|i| i.size)),
            Kind::Text(t) => format!("text size {} font {:?} bounds {:?}", t.def.size, t.def.font_class, t.def.bounds),
            _ => "other".to_string(),
        };
        if p.contains(filter) {
            println!("{p} | {kind} | world tx {:.1} ty {:.1} sx {:.3} sy {:.3} blend {} clip {} a {:.2}{vis}", w.tx, w.ty, w.a, w.d, n.blend, n.clip, n.cx.mul[3]);
        }
        if let Kind::Sprite(c) = &n.kind {
            walk(c, &p, filter, &w);
        }
    }
}

fn shot(right: &HudRight, fonts: &Fonts, path: &std::path::Path) {
    let (w, h) = (1920usize, 1080usize);
    let mut canvas = Canvas::new(w, h);
    Renderer::new(fonts.clone()).draw(&mut canvas, &right.movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    // a warm mid-tone backdrop like the reference's
    canvas.to_rgba8_over(|x, y| { let t = (x as f32 / 1920.0) * 0.2 + (y as f32 / 1080.0) * 0.1; [0.52 - t, 0.40 - t * 0.8, 0.30 - t * 0.6] }, &mut rgba);
    let (cx, cy, cw, ch) = (1280usize, 20usize, 640usize, 400usize);
    let mut crop = Vec::with_capacity(cw * ch * 4);
    for y in cy..cy + ch {
        crop.extend_from_slice(&rgba[(y * w + cx) * 4..(y * w + cx + cw) * 4]);
    }
    kh_ui::def::write_png(path, cw as u32, ch as u32, &crop).unwrap();
    println!("written {}", path.display());
    // NEXT TO THE REFERENCE: KH3's own picture (party/bn/ref, 1:1 with a 1080p stage: its first panel's left edge at x 8,
    // top at y 2) above ours cut at the same place (our first window's left edge at stage x 1360, top at y 72)
    let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/bn/ref/kh3_bonus_panels_reference.png");
    if let Ok((rw, rh, rref)) = kh_ui::def::read_png(&reference) {
        let (rw, rh) = (rw as usize, rh as usize);
        let (x0, y0) = (1352usize, 70usize);
        let ours_h = rh.max(240);
        let gap = 8usize;
        let (ow, oh) = (rw, rh + gap + ours_h);
        let mut out = vec![0u8; ow * oh * 4];
        for y in 0..rh {
            out[y * ow * 4..(y + 1) * ow * 4].copy_from_slice(&rref[y * rw * 4..(y + 1) * rw * 4]);
        }
        for y in rh..rh + gap {
            for x in 0..ow {
                out[(y * ow + x) * 4..][..4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let mut diff = 0.0f64;
        for y in 0..ours_h {
            for x in 0..rw {
                let (sx, sy) = (x0 + x, y0 + y);
                let px = if sx < w && sy < h { &rgba[(sy * w + sx) * 4..][..4] } else { &[0, 0, 0, 255][..] };
                out[((rh + gap + y) * ow + x) * 4..][..4].copy_from_slice(px);
                if y < rh {
                    diff += (0..3).map(|c| (px[c] as f64 - rref[(y * rw + x) * 4 + c] as f64).abs()).sum::<f64>() / 3.0;
                }
            }
        }
        let side = path.with_file_name(format!("{}_vs_ref.png", path.file_stem().unwrap().to_string_lossy()));
        kh_ui::def::write_png(&side, ow as u32, oh as u32, &out).unwrap();
        println!("written {} (mean |ours - KH3| over the reference's area: {:.1} / 255)", side.display(), diff / (rw * rh) as f64);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = std::path::PathBuf::from(args.first().cloned().unwrap_or_else(|| "build/sora_tmp/party/bn/shots".into()));
    let scene = args.get(1).cloned().unwrap_or_else(|| "ref".into());
    let frames: usize = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(120);
    std::fs::create_dir_all(&out).unwrap();
    let mut pack = Pack::open_default().expect("pack");
    let fonts = Fonts::load(&mut pack).unwrap();
    let strings = SideStrings::load(&mut pack);
    let mut right = HudRight::new(pack.movie("HudRight").unwrap(), fonts.clone(), &strings);
    // the roster rows as kh-host formation::ROSTER holds them (key, colour label) and the save-load pack's busts
    let mut who = |id: usize, key: &str, colour: &str| {
        let face = match key { "sora" => "MenuFaceExSora", "donald" => "MenuFaceExDonald", _ => "MenuFaceExGoofy" };
        PanelChara { id, colour: panel_colour(colour), portrait: pack.texture(&format!("../saveload/textures/{face}.png")).ok() }
    };
    let (sora, donald, goofy) = (who(0, "sora", "sora"), who(1, "donald", "donald"), who(2, "goofy", "goofy"));
    let flash = icon_line(LineIcon::Ability, "Flash Step");
    let max_hp = icon_line(LineIcon::None, &strings.bonus_lines[0]);
    match scene.as_str() {
        "ref" => {
            right.show_bonus_panel(&sora, &[flash]);
            right.show_bonus_panel(&goofy, &[max_hp]);
        }
        "sora_ability" => {
            right.show_bonus_panel(&sora, &[flash]);
        }
        "goofy_hp" => {
            right.show_bonus_panel(&goofy, &[max_hp]);
        }
        "donald_spell" => {
            right.show_bonus_panel(&donald, &[icon_line(LineIcon::Magic, "Thunder")]);
        }
        "sora_tier" => {
            right.show_bonus_panel(&sora, &[icon_line(LineIcon::Magic, "Fira")]);
        }
        "levelup" => {
            right.show_level_up_as(12, &goofy, &[strings.level_up_lines[1].clone(), icon_line(LineIcon::Ability, "Aero Float")]);
            right.show_bonus_panel(&goofy, &[max_hp]);
        }
        "all" => {
            right.show_level_up_as(25, &sora, &[strings.level_up_lines[0].clone(), icon_line(LineIcon::Ability, "Flash Step")]);
            right.show_bonus_panel(&sora, &[icon_line(LineIcon::Magic, "Fira")]);
            right.show_bonus_panel(&donald, &[icon_line(LineIcon::Magic, "Fira"), max_hp.clone()]);
            right.show_bonus_panel(&goofy, &[max_hp]);
        }
        "slide" => {
            // Sora's panel shown, Goofy's 60 frames later: then Sora's closes and Goofy's slides up
            right.show_bonus_panel(&sora, &[flash]);
            for _ in 0..60 {
                right.tick(1.0 / 60.0);
            }
            right.show_bonus_panel(&goofy, &[max_hp]);
        }
        _ => panic!("unknown scene {scene}"),
    }
    let mut ys = Vec::new();
    for f in 0..frames {
        right.tick(1.0 / 60.0);
        ys.push((f, right.bonus_y(0), right.bonus_y(1)));
    }
    if scene == "slide" {
        for (f, a, b) in ys.iter().filter(|(f, _, _)| f % 10 == 0 || (*f >= 210 && *f <= 230)) {
            println!("frame {f}: bonus window y {a:.1} / {b:.1}");
        }
    }
    println!("windows {:?}", right.bonus_windows());
    if args.get(3).map(String::as_str) == Some("dump") {
        walk(&right.movie.root, "", args.get(4).map(String::as_str).unwrap_or(""), &Matrix::IDENTITY);
    }
    shot(&right, &fonts, &out.join(format!("bn_{scene}_f{frames}.png")));
}
