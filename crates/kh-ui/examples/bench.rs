//! Where the rasteriser's time goes: draws the level-99 status HUD and the command menu N times.
//! cargo run -p kh-ui --release --example bench -- [SCALE]
use kh_ui::command::{CommandDisplay, CommandStrings};
use kh_ui::command_debug::DebugCommandSystem;
use kh_ui::hud_command::HudCommand;
use kh_ui::hud_status::{HudStatus, SlotValues, StatusDriver, StatusValues};
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer};
use std::time::Instant;

fn main() {
    let scale: f32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(2.0 / 3.0);
    let mut pack = Pack::open_default().expect("pack");
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut status = HudStatus::new(pack.movie("HudStatus").unwrap());
    let mut driver = StatusDriver::default();
    driver.faces[0] = Some(pack.movie("FacePcExSora01").unwrap());
    status.set_status(0, 0);
    let mut v = StatusValues { focus: 100, focus_max: 100, mp_gauge_unlocked: true, focus_gauge_unlocked: true, ..StatusValues::default() };
    v.slots[0] = SlotValues { active: true, hp: 265, hp_max: 265, mp: 169, mp_max: 169, ..SlotValues::default() };
    let mut sounds = Vec::new();
    let mut command = HudCommand::new(pack.movie("HudCommand").unwrap(), fonts.clone());
    let mut display = CommandDisplay::new();
    let mut system = DebugCommandSystem::new(Some(&mut pack));
    let strings = CommandStrings::load(&mut pack);
    system.command("show").unwrap();
    for _ in 0..120 {
        driver.update(&mut status, &v, 1.0 / 60.0, &mut sounds);
        status.tick(1.0 / 60.0);
        system.begin_frame();
        system.step(1.0 / 60.0);
        display.update(&mut command, &system.model, &strings, 1.0 / 60.0, &mut sounds);
        command.tick();
    }
    let root = Matrix::scale(scale, scale);
    for (name, movie) in [("HudStatus", &status.movie as &Movie), ("HudCommand", &command.movie)] {
        let mut renderer = Renderer::new(fonts.clone());
        let b = renderer.bounds(movie, &root);
        let (x0, y0) = (b.x0.floor() as i32, b.y0.floor() as i32);
        let (w, h) = ((b.x1.ceil() as i32 - x0) as usize, (b.y1.ceil() as i32 - y0) as usize);
        let mut canvas = Canvas::region(x0, y0, w, h);
        let n = 200;
        let started = Instant::now();
        for _ in 0..n {
            canvas.reset(x0, y0, w, h);
            renderer.draw(&mut canvas, movie, &root);
        }
        let total = started.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let s = renderer.stats;
        let ms = |ns: u64| ns as f64 / 1.0e6 / n as f64;
        println!("{name}: {w} x {h}: {total:.3} ms per draw; masks {} ({:.3} ms), layers {} ({:.3} ms), shapes {} cover {:.3} ms fill {:.3} ms, texts {} ({:.3} ms), gfx {}", s.masks / n, ms(s.ns_mask), s.layers / n, ms(s.ns_layer), s.shapes / n, ms(s.ns_cover), ms(s.ns_fill), s.texts / n, ms(s.ns_text), s.gfx_rects / n);
    }
}
