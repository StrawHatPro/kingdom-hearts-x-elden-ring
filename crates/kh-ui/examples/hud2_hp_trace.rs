//! (HUD2) THE HP GAUGES UNDER DAMAGE, FRAME BY FRAME: the status HUD (Sora + 2 friends) is driven through its native
//! driver with damage sequences and drawn every frame; per slot the green fill's pixels and the red lag's pixels are
//! counted, and the gauge clips' frames logged. A jump = the green pixel count going the wrong way (or up and back).
//!
//!   cargo run -p kh-ui --release --example hud2_hp_trace -- OUT_DIR [png]
use std::io::Write as _;
use std::path::PathBuf;

use kh_ui::def::write_png;
use kh_ui::hud_status::{HudStatus, SlotValues, StatusDriver, StatusValues};
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer};

const DT: f32 = 1.0 / 60.0;

fn values(hp: [i32; 3], max: [i32; 3]) -> StatusValues {
    let mut v = StatusValues { focus: 60, focus_max: 100, mp_gauge_unlocked: true, focus_gauge_unlocked: true, ..StatusValues::default() };
    for i in 0..3 {
        v.slots[i] = SlotValues { active: true, hp: hp[i], hp_max: max[i], mp: 100, mp_max: 100, ..SlotValues::default() };
    }
    v
}

/// Green / red pixels inside a box (x0, y0, x1, y1).
fn count(rgba: &[u8], w: usize, b: (usize, usize, usize, usize)) -> (u32, u32) {
    let (mut g, mut r) = (0, 0);
    for y in b.1..b.3 {
        for x in b.0..b.2 {
            let p = &rgba[(y * w + x) * 4..][..3];
            let (pr, pg, pb) = (p[0] as i32, p[1] as i32, p[2] as i32);
            if pg > 120 && pg > pr + 40 && pg > pb + 40 {
                g += 1;
            } else if pr > 120 && pr > pg + 60 && pr > pb + 40 {
                r += 1;
            }
        }
    }
    (g, r)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().cloned().unwrap_or_else(|| "build/sora_tmp/party/HUD2/trace".into()));
    let png = args.iter().any(|a| a == "png");
    std::fs::create_dir_all(&dir).unwrap();
    let mut pack = Pack::open_default().expect("the UI pack");
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut hud = HudStatus::new(pack.movie("HudStatus").unwrap());
    let mut driver = StatusDriver::default();
    driver.faces[0] = pack.movie("FacePcExSora01").ok();
    driver.faces[1] = pack.movie("FaceNpcExDonald").ok();
    driver.faces[2] = pack.movie("FaceNpcExGoofy").ok();
    hud.set_status(0, 0);
    let mut sounds = Vec::new();
    let max = [250, 180, 220];
    let mut hp = max;
    let mut log = std::fs::File::create(dir.join("trace.tsv")).unwrap();
    writeln!(log, "frame\tevent\tslot0 hp\tgreen clip\tred clip\tgreen px\tred px\tslot1 hp\tgreen clip\tred clip\tgreen px\tred px\tslot2 hp\tgreen clip\tred clip\tgreen px\tred px").unwrap();
    // The script: (frame, event, new HP per slot).
    let script: Vec<(usize, &str, [i32; 3])> = vec![
        (60, "single hit Sora 250->200, Donald 180->150, Goofy 220->170", [200, 150, 170]),
        (150, "multi-hit in one frame (3 values in one update) -> 140/120/130", [140, 120, 130]),
        (152, "hit again while red is still waiting -> 120/100/110", [120, 100, 110]),
        (250, "heal +80 while red follows", [200, 180, 190]),
        (256, "hit right after the heal -> 150/140/150", [150, 140, 150]),
        (400, "small hit -> 148/138/148", [148, 138, 148]),
    ];
    let boxes = [(1480, 840, 1822, 1080), (1822, 850, 1920, 962), (1822, 962, 1920, 1080)];
    let mut canvas = Canvas::new(1920, 1080);
    let mut renderer = Renderer::new(fonts.clone());
    let mut rgba = Vec::new();
    for frame in 0..520usize {
        let mut event = "";
        for (f, e, v) in &script {
            if *f == frame {
                for s in 0..3 {
                    if v[s] < hp[s] {
                        driver.notify_damage(s); // as kh-host does when the HP went down
                    }
                }
                hp = *v;
                event = e;
            }
        }
        driver.update(&mut hud, &values(hp, max), DT, &mut sounds);
        hud.tick(DT);
        canvas = Canvas::new(1920, 1080);
        renderer.draw(&mut canvas, &hud.movie, &Matrix::IDENTITY);
        canvas.to_rgba8_over(|_, _| [0.12, 0.12, 0.14], &mut rgba);
        let mut row = format!("{frame}\t{event}");
        for s in 0..3 {
            let (g, r) = count(&rgba, 1920, boxes[s]);
            row += &format!("\t{}\t{}\t{}\t{g}\t{r}", hp[s], hud.hp_frame(s), hud.hp_red_frame(s));
        }
        if std::env::var("HUD2_ANIM").is_ok() && (55..100).contains(&frame) {
            let s = hud.movie.sprite("sttsPc").unwrap();
            let kids: Vec<String> = s.children.iter().filter(|n| n.sprite().is_some()).map(|n| format!("{}@{:.1},{:.1}s{:.2}", n.name.as_deref().unwrap_or("-"), n.matrix.tx, n.matrix.ty, n.matrix.a)).collect();
            let me = hud.movie.node("sttsPc").map(|n| (n.matrix.tx, n.matrix.ty)).unwrap_or_default();
            println!("{frame} sttsPc f{} {:?} at {me:?}: {}", s.frame, hud.movie.current_label("sttsPc"), kids.join(" "));
        }
        writeln!(log, "{row}").unwrap();
        if png && (frame == 0 || (58..80).contains(&frame) || frame == 120) {
            // the HUD's corner (x 1480.., y 860..1050), doubled
            let (x0, y0, w, h) = (1480usize, 840usize, 440usize, 240usize);
            let mut crop = vec![0u8; w * h * 16];
            for y in 0..h * 2 {
                for x in 0..w * 2 {
                    let src = ((y0 + y / 2) * 1920 + x0 + x / 2) * 4;
                    crop[(y * w * 2 + x) * 4..][..4].copy_from_slice(&rgba[src..src + 4]);
                }
            }
            write_png(&dir.join(format!("f{frame:03}.png")), (w * 2) as u32, (h * 2) as u32, &crop).unwrap();
        }
    }
    let _ = canvas;
    println!("written {}", dir.join("trace.tsv").display());
}
