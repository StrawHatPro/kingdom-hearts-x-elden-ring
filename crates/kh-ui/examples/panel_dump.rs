//! Development aid (character materials / menu model): the direct children of the camp menu's
//! character panel with their stage bounds.
//! cargo run -p kh-ui --release --example panel_dump -- [one|two|three|four|five] [PANEL.PATH]
use kh_ui::camp_menu::LIBRARIES;
use kh_ui::player::{Kind, Sprite};
use kh_ui::{Matrix, Movie, Pack, Rect};

fn bounds(s: &Sprite, m: &Matrix, out: &mut Rect) {
    for n in s.children.iter().chain(&s.extra) {
        let w = m.mul(&n.matrix);
        match &n.kind {
            Kind::Shape(sh) => *out = out.union(&sh.bounds.rect().transform(&w)),
            Kind::Sprite(c) => bounds(c, &w, out),
            _ => {}
        }
    }
}

fn main() {
    let mut pack = Pack::open_default().expect("pack");
    for lib in LIBRARIES {
        let _ = pack.movie(lib);
    }
    let def = pack.movie("CampMenu").expect("movie");
    let mut movie = Movie::new(def);
    movie.goto_and_play("topMenu", "in");
    movie.goto_and_play("topMenu.statusAnm", "loop");
    let party = std::env::args().nth(1).unwrap_or_else(|| "one".to_string());
    let panel = std::env::args().nth(2).unwrap_or_else(|| "topMenu.statusAnm.status.charaWindow01".to_string());
    movie.goto_and_stop("topMenu.statusAnm.status", &party);
    for (clip, label) in [("ability", "loop"), ("status", "loop")] {
        if panel.starts_with(clip) {
            movie.goto_and_stop(clip, label);
        }
    }
    movie.goto_and_stop(&format!("{panel}.charaAnm"), "loop");
    for _ in 0..40 {
        movie.tick();
    }
    let (mut sprite, mut m) = (&movie.root, Matrix::IDENTITY);
    if let Some(c) = movie.sprite(&format!("{panel}.charaAnm")) { dump_chara(c, 0); }
    for name in panel.split('.') {
        println!("{name}: matrix {:?}", sprite.child(name).map(|n| n.matrix));
        let node = sprite.child(name).expect("path");
        m = m.mul(&node.matrix);
        sprite = node.sprite().expect("sprite");
    }
    for n in sprite.children.iter().chain(&sprite.extra) {
        let w = m.mul(&n.matrix);
        let (kind, r) = match &n.kind {
            Kind::Shape(sh) => ("shape", sh.bounds.rect().transform(&w)),
            Kind::Sprite(c) => {
                let mut r = Rect::EMPTY;
                bounds(c, &w, &mut r);
                ("sprite", r)
            }
            Kind::Text(_) => ("text", Rect::new(w.tx, w.ty, w.tx, w.ty)),
            _ => ("other", Rect::EMPTY),
        };
        println!("depth {:3} clip {:3} {:6} {:22} x {:7.1} .. {:7.1}  y {:7.1} .. {:7.1} {}", n.depth, n.clip, kind, n.name.as_deref().unwrap_or("-"), r.x0, r.x1, r.y0, r.y1, if n.is_visible() { "" } else { "HIDDEN" });
    }
}

#[allow(dead_code)]
fn dump_chara(s: &Sprite, depth: usize) {
    for n in s.children.iter().chain(&s.extra) {
        match &n.kind {
            Kind::Shape(sh) => println!("{}shape id {} bounds {:?} matrix {:?} fills {:?}", " ".repeat(depth), sh.id, sh.bounds.rect(), n.matrix, sh.fills.iter().map(|f| format!("{:?}", f.paint).chars().take(300).collect::<String>()).collect::<Vec<_>>()),
            Kind::Image(im) => println!("{}image size {:?} matrix {:?} fill {:?}", " ".repeat(depth), im.size, n.matrix, format!("{:?}", im.fill).chars().take(300).collect::<String>()),
            Kind::Sprite(c) => {
                println!("{}sprite {:?} class {:?} matrix {:?} icon {:?}", " ".repeat(depth), n.name, c.def.class, n.matrix, c.icon);
                dump_chara(c, depth + 1);
            }
            _ => println!("{}other", " ".repeat(depth)),
        }
    }
}
