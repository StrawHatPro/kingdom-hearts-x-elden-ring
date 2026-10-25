//! (HUD2) KH3'S ENEMY (SCAN) GAUGE UNDER ELDEN RING'S HITS, FRAME BY FRAME, BEFORE AND AFTER THE FIX. The gauge
//! (`HudRight` enmHpPack + the native update 0x14165F820 as ported in `EnemyGauge`) is fed, per frame, the HP the
//! Elden Ring host gave it: OLD = the target row's `hp` (a bullet's expected HP until the game's next read), NEW = the
//! game's own HP (`hp_game`). Per frame: the HP fed, the green clip's frame, the red clip's frame, the stock squares;
//! the green bar's lit pixels are counted from the picture. A JUMP = the green going back up without a heal.
//!
//!   cargo run -p kh-ui --release --example hud2_enemy_gauge -- OUT_DIR
use std::io::Write as _;
use std::path::PathBuf;

use kh_ui::def::write_png;
use kh_ui::hud_side::{EnemyGauge, EnemyValues, HudRight, SideStrings};
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer};

const DT: f32 = 1.0 / 60.0;
const ENM_SET: &str = "hudRight.enmHpPack.enmHpSetAll.enmHpSet";

/// One Elden Ring frame of a target row: (the row's `hp` as the HUD read it, the game's own HP).
type Row = (i32, i32);

/// The scenarios: the rows frame by frame (a hold of the last row follows), the max, large (boss) gauge.
fn scenarios() -> Vec<(&'static str, Vec<Row>, i32, bool)> {
    let mut v = Vec::new();
    // 1. ONE HIT, the bullet lands one read later; the first hits on a kind guess with ratio 1 (126 asked, 70 taken).
    let mut s = vec![(2169, 2169); 30];
    s.push((2043, 2169)); // the hit frame: the row holds the guess
    s.push((2099, 2099)); // next read: landed
    v.push(("single_hit", s, 2169, false));
    // 2. ONE HIT, the bullet lands two reads later (the game applies it after its phase): up, then down.
    let mut s = vec![(2169, 2169); 30];
    s.push((2043, 2169));
    s.push((2169, 2169));
    s.push((2099, 2099));
    v.push(("late_landing", s, 2169, false));
    // 3. A COMBO OF 3 HITS, bullets one a frame (the others wait their turn: the guess, then the untouched HP).
    let mut s = vec![(2169, 2169); 30];
    s.extend([(2043, 2169), (2169, 2169), (2043, 2099), (2099, 2099), (1973, 2099), (2099, 2029), (2029, 2029), (1959, 1959)]);
    v.push(("multi_hit", s, 2169, false));
    // 4. A HEAL (the game's regeneration) then a hit.
    let mut s = vec![(1500, 1500); 30];
    s.extend([(1600, 1600); 10]);
    s.extend([(1474, 1600), (1530, 1530)]);
    v.push(("heal_then_hit", s, 2169, false));
    // 5. A BULLET THAT MISSES and is fired again 12 frames later (the guess shown, then back for 12 frames).
    let mut s = vec![(2169, 2169); 30];
    s.push((2043, 2169));
    s.extend([(2169, 2169); 12]);
    s.push((2043, 2169));
    s.push((2099, 2099));
    v.push(("miss_and_retry", s, 2169, false));
    // 6. A BOSS (Margit ~4200: 21 bars of 200, 20 squares + 1): a hit across a bar's edge.
    let mut s = vec![(4210, 4210); 30];
    s.push((4084, 4210));
    s.push((4140, 4140));
    v.push(("boss_bar_edge", s, 4210, false));
    v
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().cloned().unwrap_or_else(|| "build/sora_tmp/party/HUD2/enemy".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut pack = Pack::open_default().expect("the UI pack");
    let fonts = Fonts::load(&mut pack).unwrap();
    let side = SideStrings::load(&mut pack);
    let mut log = std::fs::File::create(dir.join("enemy_gauge.tsv")).unwrap();
    writeln!(log, "scenario\tfeed\tframe\thp fed\tgreen clip\tred clip\tstock\tgreen px\tred px\tjump").unwrap();
    let mut summary = String::new();
    for (name, rows, max, large) in scenarios() {
        for feed in ["old", "new"] {
            let mut right = HudRight::new(pack.movie("HudRight").unwrap(), fonts.clone(), &side);
            let mut gauge = EnemyGauge::default();
            gauge.point_at(1, false, None);
            let mut sounds = Vec::new();
            let mut renderer = Renderer::new(fonts.clone());
            let mut rgba = Vec::new();
            let (mut last_green, mut last_fed, mut jumps) = (None::<i64>, None::<i32>, 0);
            let total = rows.len() + 75;
            let mut strip: Vec<Vec<u8>> = Vec::new();
            for frame in 0..total {
                let row = rows[frame.min(rows.len() - 1)];
                let hp = if feed == "old" { row.0 } else { row.1 };
                let values_of = |_: u64| Some(EnemyValues { hp, hp_max: max, large, alive: true });
                gauge.update(&mut right, DT, &values_of, &mut sounds);
                right.tick(DT);
                let (stock, green) = right.enemy_gauge_state();
                let red = right.movie.current_frame(&format!("{ENM_SET}.enmHp.enmHpGgR")).unwrap_or(0);
                // the gauge's corner, drawn
                let mut canvas = Canvas::new(1920, 1080);
                renderer.draw(&mut canvas, &right.movie, &Matrix::IDENTITY);
                canvas.to_rgba8_over(|_, _| [0.12, 0.12, 0.14], &mut rgba);
                let (x0, y0, w, h) = (1300usize, 20usize, 620usize, 70usize);
                let (mut gpx, mut rpx) = (0, 0);
                for y in y0..y0 + h {
                    for x in x0..x0 + w {
                        let p = &rgba[(y * 1920 + x) * 4..][..3];
                        let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
                        if g > 120 && g > r + 40 && g > b + 40 {
                            gpx += 1;
                        } else if r > 80 && r > g + 45 && r > b + 30 {
                            rpx += 1;
                        }
                    }
                }
                // a jump: the green bar going up while the HP fed did not rise from a heal of the game's
                let healed = last_fed.is_some_and(|l| hp > l) && row.1 > rows[frame.saturating_sub(1).min(rows.len() - 1)].1;
                let level = stock as i64 * 1000 + green as i64; // the stock squares first: a bar that empties shows the next one full
                let jump = last_green.is_some_and(|l| level > l) && !healed;
                jumps += jump as u32;
                writeln!(log, "{name}\t{feed}\t{frame}\t{hp}\t{green}\t{red}\t{stock}\t{gpx}\t{rpx}\t{}", if jump { "JUMP" } else { "" }).unwrap();
                last_green = Some(level);
                last_fed = Some(hp);
                if (28..36).contains(&frame) || frame == 85 || frame == total - 1 {
                    let mut crop = vec![0u8; w * h * 4];
                    for y in 0..h {
                        crop[y * w * 4..(y + 1) * w * 4].copy_from_slice(&rgba[((y0 + y) * 1920 + x0) * 4..][..w * 4]);
                    }
                    strip.push(crop);
                }
            }
            // a strip: one row per kept frame
            let n = strip.len();
            let (w, h) = (620usize, 70usize);
            let mut img = vec![0u8; w * h * n * 4];
            for (k, c) in strip.iter().enumerate() {
                img[k * w * h * 4..(k + 1) * w * h * 4].copy_from_slice(c);
            }
            write_png(&dir.join(format!("{name}_{feed}.png")), w as u32, (h * n) as u32, &img).unwrap();
            summary += &format!("{name:16} {feed}: {jumps} jump(s) of the green bar\n");
        }
    }
    std::fs::write(dir.join("summary.txt"), &summary).unwrap();
    print!("{summary}");
}
