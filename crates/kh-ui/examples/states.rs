//! Pixel comparison of LIVE-DRIVEN states with the offline compositor: drives the ported movies
//! through their drivers, then writes, per state, the player's state as JSON (for
//! `node build/sora_tmp/hud/ref_render.js`) and this crate's rendering of it.
//!
//!   cargo run -p kh-ui --release --example states -- OUT_DIR          writes NAME.json + NAME_rust.png
//!   node build/sora_tmp/hud/ref_render.js OUT_DIR/NAME.json OUT_DIR/NAME_ref.png
//!   cargo run -p kh-ui --release --example states -- OUT_DIR compare  prints the differences

use std::path::{Path, PathBuf};

use kh_ui::command::{CommandDisplay, CommandStrings};
use kh_ui::command_debug::DebugCommandSystem;
use kh_ui::def::{read_png, write_png};
use kh_ui::hud_command::HudCommand;
use kh_ui::hud_side::{EnemyGauge, EnemyValues, HudRight, SideStrings};
use kh_ui::hud_status::{HudStatus, SlotValues, StatusDriver, StatusValues};
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer};

const DT: f32 = 1.0 / 60.0;
const BG: [f32; 3] = [0.3, 0.38, 0.46];
const NAMES: [&str; 4] = ["st_hit", "st_dying_charge", "cmd_sub", "enemy"];

fn values(hp: i32, mp: i32, focus: i32) -> StatusValues {
    let mut v = StatusValues { focus, focus_max: 100, mp_gauge_unlocked: true, focus_gauge_unlocked: true, ..StatusValues::default() };
    v.slots[0] = SlotValues { active: true, hp, hp_max: 265, mp, mp_max: 169, ..SlotValues::default() };
    v
}

fn write(dir: &Path, name: &str, movies: &[(&str, &Movie)], fonts: &Fonts) {
    let mut canvas = Canvas::new(1920, 1080);
    for p in canvas.px.chunks_exact_mut(4) {
        p.copy_from_slice(&[BG[0], BG[1], BG[2], 1.0]);
    }
    let mut renderer = Renderer::new(fonts.clone());
    let mut list = Vec::new();
    for (movie_name, movie) in movies {
        renderer.draw(&mut canvas, movie, &Matrix::IDENTITY);
        let mut entry = serde_json::json!({ "name": movie_name, "state": movie.dump_state() });
        if *movie_name == "HudStatus" {
            entry["attach"] = serde_json::json!([{ "host": "sttsPc.pcHp.pcHpPack.poPcFace", "movie": "FacePcExSora01", "child": "facePcExSora01" }]);
        }
        list.push(entry);
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8(&mut rgba);
    write_png(&dir.join(format!("{name}_rust.png")), 1920, 1080, &rgba).unwrap();
    let state = serde_json::json!({ "size": [1920, 1080], "bg": BG, "movies": list });
    std::fs::write(dir.join(format!("{name}.json")), serde_json::to_string(&state).unwrap()).unwrap();
    println!("states: wrote {name}.json and {name}_rust.png");
}

fn compare(dir: &Path) {
    for name in NAMES {
        let (Ok((w, h, a)), Ok((_, _, b))) = (read_png(&dir.join(format!("{name}_rust.png"))), read_png(&dir.join(format!("{name}_ref.png")))) else {
            println!("{name}: missing picture");
            continue;
        };
        let (mut max, mut sum, mut over8, mut over32) = (0u32, 0u64, 0usize, 0usize);
        let mut img = Vec::with_capacity(a.len());
        for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
            let m = (0..3).map(|i| (p[i] as i32 - q[i] as i32).unsigned_abs()).max().unwrap();
            max = max.max(m);
            sum += m as u64;
            over8 += (m > 8) as usize;
            over32 += (m > 32) as usize;
            let v = (m * 4).min(255) as u8;
            img.extend_from_slice(&[v, v, v, 255]);
        }
        write_png(&dir.join(format!("{name}_diff.png")), w, h, &img).unwrap();
        let n = (w * h) as usize;
        println!("{name}: max channel difference {max} / 255, mean {:.5}, pixels over 8: {over8} ({:.4} %), over 32: {over32} ({:.4} %) of {n}", sum as f64 / n as f64, over8 as f64 * 100.0 / n as f64, over32 as f64 * 100.0 / n as f64);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().cloned().unwrap_or_else(|| "build/sora_tmp/hud/states".into()));
    std::fs::create_dir_all(&dir).unwrap();
    if args.get(1).map(String::as_str) == Some("compare") {
        return compare(&dir);
    }
    let mut pack = Pack::open_default().expect("the UI pack");
    let fonts = Fonts::load(&mut pack).unwrap();
    let face = pack.movie("FacePcExSora01").unwrap();
    let mut sounds = Vec::new();

    // The command menu, shown, at rest.
    let mut command = HudCommand::new(pack.movie("HudCommand").unwrap(), fonts.clone());
    let mut display = CommandDisplay::new();
    let mut system = DebugCommandSystem::new(Some(&mut pack));
    let strings = CommandStrings::load(&mut pack);
    system.set_status(169, 169, false);
    let mut menu = |words: &[&str], frames: usize, command: &mut HudCommand, sounds: &mut Vec<kh_ui::UiSound>| {
        for frame in 0..frames.max(1) {
            system.begin_frame();
            if frame == 0 {
                for w in words {
                    system.command(w).unwrap();
                }
            }
            system.step(DT);
            display.update(command, &system.model, &strings, DT, sounds);
            command.tick();
        }
    };
    menu(&["show"], 60, &mut command, &mut sounds);

    // 1: a hit 265 -> 180, 0.95 s later (the red gauge is following); MP 100, focus 60.
    let mut status = HudStatus::new(pack.movie("HudStatus").unwrap());
    let mut driver = StatusDriver::default();
    driver.faces[0] = Some(face.clone());
    status.set_status(0, 0);
    let mut run = |v: &StatusValues, frames: usize, status: &mut HudStatus, sounds: &mut Vec<kh_ui::UiSound>| {
        for _ in 0..frames {
            driver.update(status, v, DT, sounds);
            status.tick(DT);
        }
    };
    run(&values(265, 100, 60), 90, &mut status, &mut sounds);
    run(&values(180, 100, 60), 57, &mut status, &mut sounds);
    write(&dir, NAMES[0], &[("HudCommand", &command.movie), ("HudStatus", &status.movie)], &fonts);

    // 2: pinch (HP 40) with MP Charge running, 1.6 s later.
    let mut v = values(40, 0, 100);
    v.slots[0].mp_charging = true;
    v.slots[0].mp_charge_value = 70.0;
    run(&v, 96, &mut status, &mut sounds);
    write(&dir, NAMES[1], &[("HudStatus", &status.movie)], &fonts);

    // 3: battle colour, the Magic list open with the cursor on its third row.
    menu(&["battle 2"], 20, &mut command, &mut sounds);
    menu(&["open magic"], 20, &mut command, &mut sounds);
    menu(&["down"], 10, &mut command, &mut sounds);
    menu(&["down"], 40, &mut command, &mut sounds);
    write(&dir, NAMES[2], &[("HudCommand", &command.movie)], &fonts);

    // 4: the enemy gauge at 1234 / 3000.
    let side = SideStrings::load(&mut pack);
    let mut right = HudRight::new(pack.movie("HudRight").unwrap(), fonts.clone(), &side);
    let mut gauge = EnemyGauge::default();
    gauge.point_at(1, false, None);
    let hp = std::cell::Cell::new(3000);
    let values_of = |_: u64| Some(EnemyValues { hp: hp.get(), hp_max: 3000, large: false, alive: true });
    for frame in 0..120 {
        if frame == 60 {
            hp.set(1234);
        }
        gauge.update(&mut right, DT, &values_of, &mut sounds);
        right.tick(DT);
    }
    write(&dir, NAMES[3], &[("HudRight", &right.movie)], &fonts);
}
