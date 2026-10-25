//! Development aid (IMPL_ER34): what a clip's label animates: per frame, the matrix and alpha of each direct child.
//! cargo run -p kh-ui --release --example anim_dump -- CLIP.PATH LABEL [FRAMES] [SETUP=LABEL ...]
use kh_ui::camp_menu::LIBRARIES;
use kh_ui::{Movie, Pack};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut pack = Pack::open_default().expect("pack");
    let def = pack.movie_linked("CampMenu", &LIBRARIES).expect("movie");
    let mut movie = Movie::new(def);
    movie.goto_and_play("", "in");
    for _ in 0..40 {
        movie.tick();
    }
    for setup in args.iter().skip(3) {
        if let Some((path, label)) = setup.split_once('=') {
            movie.goto_and_play(path, label);
            for _ in 0..30 {
                movie.tick();
            }
        }
    }
    // (IMPL_ER37) LABEL `?` lists the clip's labels with their frames
    if args[1] == "?" {
        if let Some(sprite) = movie.sprite(&args[0]) {
            println!("{} frames {}: {:?}", args[0], sprite.def.ops.len(), sprite.def.labels);
        }
        return;
    }
    let frames: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(14);
    movie.goto_and_play(&args[0], &args[1]);
    for frame in 0..frames {
        let Some(sprite) = movie.sprite(&args[0]) else { break };
        let line: Vec<String> = sprite.children.iter().filter(|n| n.sprite().is_some()).map(|n| format!("{}[s {:.2} at {:.0},{:.0} a {:.2}{}]", n.name.as_deref().unwrap_or("-"), n.matrix.a, n.matrix.tx, n.matrix.ty, n.cx.mul[3], if n.is_visible() { "" } else { " HID" })).collect();
        println!("{frame:2} f{} {:?}: {}", sprite.frame, movie.current_label(&args[0]), line.join(" "));
        movie.tick();
    }
}
