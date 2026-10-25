//! (HUD2) Development aid: the KeyItem movie's caption plate and fields (matrices, shape bounds, text bounds).
use kh_ui::camp_menu::LIBRARIES;
use kh_ui::player::{Kind, Sprite};
use kh_ui::{Movie, Pack};

fn walk(s: &Sprite, path: &str, depth: usize) {
    for (i, n) in s.children.iter().chain(&s.extra).enumerate() {
        let p = format!("{path}.{}", n.name.clone().unwrap_or_else(|| format!("@{}#{i}", n.depth).into()));
        let m = &n.matrix;
        let what = match &n.kind {
            Kind::Shape(sh) => format!("shape bounds {:?}", sh.bounds),
            Kind::Text(t) => format!("text bounds {:?} wrap {} leading {} size {}", t.def.bounds, t.def.word_wrap, t.def.leading, t.def.size),
            Kind::Sprite(c) => format!("sprite {} f{}", c.def.class.as_deref().unwrap_or("-"), c.frame),
            _ => "other".into(),
        };
        println!("{p} | m [{:.3} {:.3} {:.3} {:.3} {:.1} {:.1}] clip {} | {what}", m.a, m.b, m.c, m.d, m.tx, m.ty, n.clip);
        if let Kind::Sprite(c) = &n.kind {
            if depth > 0 {
                walk(c, &p, depth - 1);
            }
        }
    }
}

fn main() {
    let mut pack = Pack::open_default().expect("pack");
    let def = pack.movie_linked("KeyItem", &LIBRARIES).expect("movie");
    let mut movie = Movie::new(def);
    movie.goto_and_play("", "loop");
    movie.goto_and_play("getItemL", "in");
    for _ in 0..130 {
        movie.tick();
    }
    let s = movie.sprite("getItemL").unwrap();
    walk(s, "getItemL", 3);
    if std::env::var("FRAMES").is_ok() { frames(); }
}

#[allow(dead_code)]
pub fn frames() {
    let mut pack = Pack::open_default().expect("pack");
    let def = pack.movie_linked("KeyItem", &LIBRARIES).expect("movie");
    let mut movie = Movie::new(def);
    movie.goto_and_play("", "loop");
    movie.goto_and_play("getItemL", "in");
    for f in 0..140 {
        movie.tick();
        if f == 120 {
            movie.goto_and_play("getItemL", "out");
        }
        let s = movie.sprite("getItemL").unwrap();
        let line: Vec<String> = s.children.iter().filter(|n| matches!(n.depth, 3 | 6 | 14 | 41)).map(|n| format!("d{} [{:.2} {:.2} {:.0} {:.0}] a{:.2}", n.depth, n.matrix.a, n.matrix.d, n.matrix.tx, n.matrix.ty, n.cx.mul[3])).collect();
        println!("{f} f{} {}", s.frame, line.join(" | "));
    }
}
