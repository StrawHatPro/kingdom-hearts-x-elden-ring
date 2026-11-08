//! (BOSS-BARS) KH3'S ENEMY GAUGE WITH MORE THAN 20 BARS: the "+N" counter over the 20 stock squares, drawn offline for
//! several extra-bar counts (1, 9, 10, 99, 100, 150, 999, 1234), one row per count, plus a countdown 101 -> 99 -> 9
//! (the layout switching as bars drain). Each row is the gauge's corner of a 1920 x 1080 HUD, the counter's region
//! drawn 2x beside it.
//!
//!   cargo run -p kh-ui --release --example boss_bars -- OUT_DIR
use std::path::PathBuf;

use kh_ui::def::write_png;
use kh_ui::hud_side::{EnemyGauge, EnemyValues, HudRight, SideStrings, ENEMY_HP_STOCK_MAX};
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer};

const DT: f32 = 1.0 / 60.0;
const ENM_SET: &str = "hudRight.enmHpPack.enmHpSetAll.enmHpSet";
/// The gauge's corner of the 1920 x 1080 HUD (x, y, w, h).
const CROP: (usize, usize, usize, usize) = (1240, 10, 680, 90);
/// HP per bar of a small gauge (`EnemyGauge`: 200).
const UNIT: i32 = 200;

/// HP max giving `extra` bars over the 20 squares: stock = 20 + extra full bars under a full top bar.
fn max_for(extra: i32) -> i32 {
    (ENEMY_HP_STOCK_MAX + extra + 1) * UNIT
}

fn draw(right: &HudRight, renderer: &mut Renderer, rgba: &mut Vec<u8>) -> Vec<u8> {
    let mut canvas = Canvas::new(1920, 1080);
    renderer.draw(&mut canvas, &right.movie, &Matrix::IDENTITY);
    canvas.to_rgba8_over(|_, _| [0.12, 0.12, 0.14], rgba);
    let (x0, y0, w, h) = CROP;
    // the crop, then the crop again at 2x (nearest), side by side: width w + 2w
    let ow = w * 3;
    let mut out: Vec<u8> = [31u8, 31, 36, 255].repeat(ow * h * 2);
    for y in 0..h * 2 {
        for x in 0..ow {
            let (sx, sy) = if x < w { (x, y) } else { ((x - w) / 2, y / 2) };
            if x < w && y >= h {
                continue;
            }
            let src = &rgba[((y0 + sy) * 1920 + x0 + sx) * 4..][..4];
            out[(y * ow + x) * 4..][..4].copy_from_slice(src);
        }
    }
    out
}

/// `--dump`: every object under the counter (shapes with their bounds), at both of the movie's layouts.
fn dump(s: &kh_ui::player::Sprite, path: &str, m: &Matrix) {
    use kh_ui::player::Kind;
    for node in s.children.iter().chain(&s.extra) {
        let p = format!("{path}.{}", node.label());
        let mm = m.mul(&node.matrix);
        match &node.kind {
            Kind::Sprite(c) => {
                println!("{p} | sprite {} frame {} | tx {:.1} ty {:.1} a {:.2} d {:.2} | gfx {}", c.def.class.as_deref().unwrap_or("-"), c.frame, mm.tx, mm.ty, mm.a, mm.d, c.gfx.len());
                for r in &c.gfx {
                    println!("{p}   gfx rect x {:.1} y {:.1} w {:.1} h {:.1} fill tx {:.1}", r.x, r.y, r.w, r.h, r.fill.matrix.tx);
                }
                dump(c, &p, &mm);
            }
            Kind::Shape(sh) => {
                let b = sh.bounds.rect().transform(&mm);
                println!("{p} | SHAPE ch {} x {:.1}..{:.1} y {:.1}..{:.1}", node.ch, b.x0, b.x1, b.y0, b.y1);
            }
            _ => println!("{p} | other"),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--dump") {
        let mut pack = Pack::open_default().expect("the UI pack");
        let fonts = Fonts::load(&mut pack).unwrap();
        let side = SideStrings::load(&mut pack);
        for extra in [5, 50, 150] {
            let mut right = HudRight::new(pack.movie("HudRight").unwrap(), fonts.clone(), &side);
            let mut gauge = EnemyGauge::default();
            gauge.point_at(1, false, None);
            let (hp, max) = (max_for(extra), max_for(extra));
            for _ in 0..60 {
                gauge.update(&mut right, DT, &|_: u64| Some(EnemyValues { hp, hp_max: max, large: false, alive: true }), &mut Vec::new());
                right.tick(DT);
            }
            let at = format!("{ENM_SET}.enmHpAddS");
            println!("==== +{extra}");
            dump(right.movie.sprite(&at).unwrap(), &at, &Matrix::IDENTITY);
        }
        return;
    }
    let dir = PathBuf::from(args.first().cloned().unwrap_or_else(|| "build/sora_tmp/weapons/BOSS-BARS/shots".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut pack = Pack::open_default().expect("the UI pack");
    let fonts = Fonts::load(&mut pack).unwrap();
    let side = SideStrings::load(&mut pack);
    let (w, h) = (CROP.2 * 3, CROP.3 * 2);
    let mut rgba = Vec::new();
    let mut sheet = |name: &str, rows: &[(i32, i32)]| {
        // rows: (hp max's extra bars, the HP's extra bars)
        let mut img = Vec::new();
        for &(max_extra, extra) in rows {
            let mut right = HudRight::new(pack.movie("HudRight").unwrap(), fonts.clone(), &side);
            let mut gauge = EnemyGauge::default();
            let mut renderer = Renderer::new(fonts.clone());
            gauge.point_at(1, false, None);
            let max = max_for(max_extra);
            let hp = max_for(extra);
            for _ in 0..60 {
                let values_of = |_: u64| Some(EnemyValues { hp, hp_max: max, large: false, alive: true });
                gauge.update(&mut right, DT, &values_of, &mut Vec::new());
                right.tick(DT);
            }
            let counter = right.movie.sprite(&format!("{ENM_SET}.enmHpAddS.enmHpAddSCount.enmHpAddSSet.enmHpAddSNum")).and_then(|s| s.number.as_ref()).map(|p| (p.number, p.num_digit));
            println!("{name}: max +{max_extra}, hp +{extra}: stock {} (counter, digits {counter:?})", right.enemy_gauge_state().0);
            img.extend(draw(&right, &mut renderer, &mut rgba));
        }
        write_png(&dir.join(format!("{name}.png")), w as u32, (h * rows.len()) as u32, &img).unwrap();
    };
    sheet("counts", &[1, 9, 10, 99, 100, 150, 999, 1234].map(|n| (n, n)));
    sheet("boss_150_999", &[(150, 150), (999, 999)]);
    // a 101-extra boss drained bar by bar: 101, 100, 99, 10, 9, 1 (then the 20 squares alone)
    sheet("countdown", &[101, 100, 99, 10, 9, 1, 0].map(|n| (101, n)));
}
