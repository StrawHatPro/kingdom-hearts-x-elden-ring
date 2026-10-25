//! Development aid: the LIVE display tree of a pack movie after some timeline commands.
//! cargo run -p kh-ui --release --example tree -- MOVIE [PATH=LABEL | tickN | +PATH:CLASS:NAME]... [--at PREFIX] [--depth N]
//! `PATH=LABEL` = gotoAndPlay, `tick30` = 30 frames, `--at` prints only that subtree.
use kh_ui::camp_menu::LIBRARIES;
use kh_ui::player::{Kind, Sprite};
use kh_ui::{Movie, Pack};

fn walk(s: &Sprite, path: &str, depth: usize, max: usize, x: f32, y: f32) {
    for node in s.children.iter().chain(&s.extra) {
        let p = if path.is_empty() { node.label() } else { format!("{path}.{}", node.label()) };
        let (nx, ny) = (x + node.matrix.tx, y + node.matrix.ty);
        let vis = if node.is_visible() { "" } else { " HIDDEN" };
        if node.name.is_none() && !std::env::args().any(|a| a == "--all") {
            continue;
        }
        match &node.kind {
            Kind::Sprite(child) => {
                if matches!(node.name.as_deref(), Some("rect" | "anchor")) {
                    for c in &child.children {
                        if let Kind::Shape(sh) = &c.kind {
                            let b = sh.bounds.rect().transform(&node.matrix.mul(&c.matrix));
                            println!("{p} RECT local x {:.1} y {:.1} w {:.1} h {:.1}", b.x0, b.y0, b.x1 - b.x0, b.y1 - b.y0);
                        }
                    }
                }
                let labels: Vec<String> = child.def.labels.iter().map(|(l, f)| format!("{l}:{f}")).collect();
                println!("{p} | {} | at {nx:.0},{ny:.0} | frame {}/{} {}{}{vis}", child.def.class.as_deref().unwrap_or("-"), child.frame, child.def.frames, if child.playing { "playing " } else { "" }, labels.join(" "));
                if depth < max {
                    walk(child, &p, depth + 1, max, nx, ny);
                }
            }
            Kind::Text(t) => println!("{p} | TEXT \"{}\" size {} {:?} | at {nx:.0},{ny:.0}{vis}", t.shown().replace('\n', "\\n"), t.def.size, t.def.align),
            _ => {}
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut pack = Pack::open_default().expect("pack");
    let name = args.first().expect("movie name");
    let def = pack.movie_linked(name, &LIBRARIES).expect("movie");
    let mut movie = Movie::new(def);
    let (mut at, mut max) = (String::new(), 99usize);
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if a == "--at" {
            at = args[i + 1].clone();
            i += 1;
        } else if a == "--depth" {
            max = args[i + 1].parse().unwrap();
            i += 1;
        } else if a == "--png" {
            // Renders the movie as it is now.
            let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
            let mut canvas = kh_ui::Canvas::new(1920, 1080);
            kh_ui::Renderer::new(fonts).draw(&mut canvas, &movie, &kh_ui::Matrix::IDENTITY);
            let mut rgba = Vec::new();
            canvas.to_rgba8_over(|_, _| [0.3, 0.38, 0.46], &mut rgba);
            kh_ui::def::write_png(std::path::Path::new(&args[i + 1]), 1920, 1080, &rgba).unwrap();
            i += 1;
        } else if let Some(path) = a.strip_prefix("hide:") {
            println!("hide {path}: {}", movie.set_visible(path, false));
        } else if let Some(n) = a.strip_prefix("tick") {
            for _ in 0..n.parse::<u32>().unwrap_or(1) {
                movie.tick();
            }
        } else if let Some(rest) = a.strip_prefix('+') {
            let parts: Vec<&str> = rest.split(':').collect();
            println!("add_instance {rest}: {}", movie.add_instance(parts[0], parts[1], parts[2]));
        } else if let Some((path, label)) = a.split_once('=') {
            if !movie.goto_and_play(path, label) {
                println!("goto {path} {label}: FAILED");
            }
        }
        i += 1;
    }
    match movie.sprite(&at) {
        Some(s) => walk(s, &at, 0, max, 0.0, 0.0),
        None => println!("no sprite at {at}"),
    }
    for event in movie.take_events() {
        println!("event {} {} {}", event.path, event.name, event.value);
    }
}
