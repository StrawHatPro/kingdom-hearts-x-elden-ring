//! (party PF) Development aid: library symbols drawn alone with their bounds (the parts the Party Formation screen
//! reuses). cargo run -p kh-ui --release --example pf_parts -- CLASS[=LABEL] ... ; picture build/sora_tmp/party/pf/parts.png
use kh_ui::{Canvas, Matrix, Movie, Pack, Renderer};

fn main() {
    let mut pack = Pack::open_default().expect("pack");
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    let def = pack.movie_linked("CampMenu", &kh_ui::camp_menu::LIBRARIES).unwrap();
    let lib = pack.movie("ItemListLibrary").unwrap();
    let mut movie = Movie::new(def);
    let mut y = 80.0;
    for arg in std::env::args().skip(1) {
        let (class, label) = arg.split_once('=').map_or((arg.as_str(), None), |(c, l)| (c, Some(l)));
        let (lib, class) = match class.split_once(':') { Some((l, c)) => (pack.movie(l).unwrap(), c), None => (lib.clone(), class) };
        let name = format!("p{}", y as i32);
        let Some(mut node) = Movie::instance_of(&lib, class, &name, label) else { println!("{class}: not found"); continue };
        node.matrix = Matrix::translate(600.0, y);
        movie.root.extra.push(node);
        y += 130.0;
    }
    for _ in 0..20 {
        movie.tick();
    }
    // only the extras
    movie.root.children.clear();
    let r = Renderer::new(fonts.clone());
    for n in &movie.root.extra {
        let mut m = Movie::new(movie.def.clone());
        m.root.children.clear();
        m.root.extra.push(n.clone());
        let b = r.bounds(&m, &Matrix::IDENTITY);
        println!("{}: bounds x {:.1}..{:.1} y {:.1}..{:.1} (origin {:.0},{:.0})", n.name.as_deref().unwrap_or(""), b.x0, b.x1, b.y0, b.y1, n.matrix.tx, n.matrix.ty);
    }
    let mut canvas = Canvas::new(1920, 1080);
    Renderer::new(fonts).draw(&mut canvas, &movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.08, 0.1, 0.2], &mut rgba);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/pf/parts.png");
    kh_ui::def::write_png(&out, 1920, 1080, &rgba).unwrap();
}
