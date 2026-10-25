//! Development aid (game over): the game-over menu's picture alone, over black.
//! cargo run -p kh-ui --release --example gameover -- OUT.png [--type N] [--hint TEXT] [--ticks N] [--size WxH] [--down N]
//! [--under PICTURE.png] (the menu over a picture of the same size: the 3D scene)
use kh_ui::camp::CampKey;
use kh_ui::gameover::GameOverMenu;
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let out = args.first().expect("OUT.png");
    let menu_type: usize = value("--type").and_then(|v| v.parse().ok()).unwrap_or(2);
    let hint = value("--hint").unwrap_or_default();
    let ticks: u32 = value("--ticks").and_then(|v| v.parse().ok()).unwrap_or(60);
    let down: u32 = value("--down").and_then(|v| v.parse().ok()).unwrap_or(0);
    let (w, h) = value("--size").and_then(|v| v.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap()))).unwrap_or((1280usize, 720usize));
    let mut pack = Pack::open_default().expect("pack");
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut screen = GameOverMenu::load(&mut pack, fonts.clone()).unwrap();
    screen.open_type(menu_type, &[], &hint);
    let mut sounds = Vec::new();
    for _ in 0..ticks {
        screen.tick();
    }
    for _ in 0..down {
        screen.key(CampKey::Down, false, &mut sounds);
        for _ in 0..20 {
            screen.tick();
        }
    }
    let k = w as f32 / 1920.0;
    let mut canvas = Canvas::new(w, h);
    Renderer::new(fonts).draw(&mut canvas, &screen.movie, &Matrix::scale(k, k));
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.0, 0.0, 0.0], &mut rgba);
    kh_ui::def::write_png(std::path::Path::new(out), w as u32, h as u32, &rgba).unwrap();
    println!("{out}: type {menu_type}, buttons {:?}, cursor {}, light cursor at {:?}", screen.buttons(), screen.cursor(), screen.light_cursor());
}
