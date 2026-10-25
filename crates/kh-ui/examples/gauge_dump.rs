//! Development aid (IMPL_ER21): the display tree under a clip of the camp menu at given frames, with stage bounds.
//! cargo run -p kh-ui --release --example gauge_dump -- [PATH] [FRAME]...
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

fn walk(s: &Sprite, m: &Matrix, indent: usize) {
    for n in s.children.iter().chain(&s.extra) {
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
        println!("{:indent$}depth {:3} clip {:3} {:6} {:16} x {:7.1} .. {:7.1} y {:7.1} .. {:7.1} | matrix a {:.3} b {:.3} c {:.3} d {:.3} tx {:.1} ty {:.1} {}", "", n.depth, n.clip, kind, n.name.as_deref().unwrap_or("-"), r.x0, r.x1, r.y0, r.y1, n.matrix.a, n.matrix.b, n.matrix.c, n.matrix.d, n.matrix.tx, n.matrix.ty, if n.is_visible() { "" } else { "HIDDEN" }, indent = indent);
        if let Kind::Sprite(c) = &n.kind {
            println!("{:indent$}  (frame {}/{})", "", c.frame, c.def.frames, indent = indent);
            walk(c, &w, indent + 2);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().cloned().unwrap_or_else(|| "topMenu.statusAnm.status.charaWindow01.expGg".to_string());
    let frames: Vec<u32> = args.iter().skip(1).filter_map(|a| a.parse().ok()).collect();
    let mut pack = Pack::open_default().expect("pack");
    for lib in LIBRARIES {
        let _ = pack.movie(lib);
    }
    let def = pack.movie("CampMenu").expect("movie");
    let mut movie = Movie::new(def);
    movie.goto_and_play("topMenu", "in");
    movie.goto_and_play("topMenu.statusAnm", "loop");
    movie.goto_and_stop("topMenu.statusAnm.status", "one");
    for _ in 0..40 {
        movie.tick();
    }
    for frame in if frames.is_empty() { vec![1, 76, 151] } else { frames } {
        movie.goto_frame(&path, frame);
        movie.tick();
        let (mut sprite, mut m) = (&movie.root, Matrix::IDENTITY);
        for name in path.split('.') {
            let node = sprite.child(name).expect("path");
            m = m.mul(&node.matrix);
            sprite = node.sprite().expect("sprite");
        }
        println!("---- {path} at frame {frame} (now {}/{})", sprite.frame, sprite.def.frames);
        walk(sprite, &m, 0);
    }
}
