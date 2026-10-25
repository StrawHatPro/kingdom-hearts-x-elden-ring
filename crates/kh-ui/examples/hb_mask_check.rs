//! (HB) BEFORE / AFTER OF SOLID SPRITE MASKS (render.rs `set_solid_masks`, 2026-10-16): every listed movie whole, from
//! frame 1 and from each root label, sampled every 8 frames of 40, drawn with sprite masks as before (their shapes'
//! paint) and as now (coverage only), laid over a mid-grey scene. Prints per movie the samples, the pixels that changed
//! by more than 8 / 32 (of 255), the largest brightening and darkening; writes the worst sample as OUT_DIR/<movie>.png
//! (before | after).
//!
//!   cargo run -p kh-ui --release --example hb_mask_check -- OUT_DIR

use std::path::PathBuf;

use std::sync::Arc;

use kh_ui::def::{write_png, Op, SpriteDef};
use kh_ui::MovieDef;
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer};

const W: usize = 1920;
const H: usize = 1080;
const BG: [f32; 3] = [0.5, 0.5, 0.5];

fn draw(renderer: &mut Renderer, movie: &Movie, solid: bool) -> Canvas {
    kh_ui::render::set_solid_masks(solid);
    let mut canvas = Canvas::new(W, H);
    renderer.draw(&mut canvas, movie, &Matrix::IDENTITY);
    kh_ui::render::set_solid_masks(true);
    canvas
}

fn over(p: &[f32]) -> [i32; 3] {
    let a = p[3].clamp(0.0, 1.0);
    let c = |v: f32, b: f32| ((v + b * (1.0 - a)) * 255.0 + 0.5).clamp(0.0, 255.0) as i32;
    [c(p[0], BG[0]), c(p[1], BG[1]), c(p[2], BG[2])]
}

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "build/sora_tmp/er/hb/masks".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut pack = Pack::open_default().expect("the UI pack");
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut renderer = Renderer::new(fonts.clone());
    let movies = ["HudStatus", "HudCommand", "HudTarget", "HudLeft", "HudRight", "HudInput", "FacePcExSora01", "FaceNpcExDonald", "FaceNpcExGoofy", "FaceNpcExSora", "SaveLoad", "Dialog", "Pause", "CenterInformation", "ItemListLibrary", "SystemLibrary", "MainLibrary", "KeyItem", "GameOver", "CampMenu"];
    for name in movies {
        let Ok(def) = pack.movie(name) else {
            println!("{name}: not in the pack");
            continue;
        };
        let (mut samples, mut over8, mut over32, mut brighter, mut darker) = (0, 0usize, 0usize, 0i32, 0i32);
        let mut worst: Option<(usize, Vec<u8>)> = None;
        // (probes) every sprite that places a SPRITE as a mask (clip depth), alone, from frame 1 and each label
        let mut probes: Vec<u32> = def.chars.keys().copied().filter(|id| def.sprite(*id).is_some_and(|s| s.ops.iter().flatten().any(|op| op.clip.is_some_and(|c| c > 0) && op.ch.is_some_and(|ch| def.sprite(ch).is_some())))).collect();
        probes.sort();
        let mut runs: Vec<(Arc<MovieDef>, Option<String>, u32, bool)> = Vec::new();
        for id in &probes {
            let op = Op { depth: 1, ch: Some(*id), matrix: Some(Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: W as f32 / 2.0, ty: H as f32 / 2.0 }), name: Some(Arc::from("probe")), visible: Some(true), ..Op::default() };
            let root = SpriteDef { id: 0, frames: 1, ops: vec![vec![op]], scripts: vec![Vec::new()], ..SpriteDef::default() };
            let probe = Arc::new(MovieDef { name: def.name.clone(), stage: def.stage, fps: def.fps, chars: def.chars.clone(), root: Arc::new(root), symbols: def.symbols.clone(), bitmaps: def.bitmaps.clone(), consts: def.consts.clone(), unknown_statements: Vec::new() });
            let frames = probe.sprite(*id).map_or(1, |s| s.frames);
            let labels: Vec<String> = probe.sprite(*id).map(|s| s.labels.iter().map(|l| l.0.clone()).collect()).unwrap_or_default();
            runs.push((probe.clone(), None, frames, true));
            for l in labels {
                runs.push((probe.clone(), Some(l), 48, true));
            }
        }
        let root_labels: Vec<String> = def.root.labels.iter().map(|l| l.0.clone()).collect();
        runs.push((def.clone(), None, 40, false));
        for l in root_labels {
            runs.push((def.clone(), Some(l), 40, false));
        }
        for (mdef, start, frames, is_probe) in runs {
            let mut movie = Movie::new(mdef.clone());
            movie.tick();
            if let Some(label) = &start {
                movie.goto_and_play(if is_probe { "probe" } else { "" }, label);
            }
            let step = (frames / 24).max(1) as usize;
            for f in 0..frames.min(720) as usize {
                if f % step == 0 {
                    let (old, new) = (draw(&mut renderer, &movie, false), draw(&mut renderer, &movie, true));
                    samples += 1;
                    let mut score = 0;
                    for (p, q) in old.px.chunks_exact(4).zip(new.px.chunks_exact(4)) {
                        if p[3] <= 0.0 && q[3] <= 0.0 {
                            continue;
                        }
                        let (o, n) = (over(p), over(q));
                        let d: Vec<i32> = (0..3).map(|i| n[i] - o[i]).collect();
                        let (hi, lo) = (*d.iter().max().unwrap(), *d.iter().min().unwrap());
                        brighter = brighter.max(hi);
                        darker = darker.min(lo);
                        let m = hi.max(-lo);
                        if m > 8 {
                            over8 += 1;
                            score += 1;
                        }
                        if m > 32 {
                            over32 += 1;
                        }
                    }
                    if score > 0 && worst.as_ref().is_none_or(|w| w.0 < score) {
                        let (w2, h2) = (W / 2, H / 2);
                        let mut img = vec![0u8; W * h2 * 4];
                        for (col, c) in [&old, &new].iter().enumerate() {
                            for y in 0..h2 {
                                for x in 0..w2 {
                                    let v = over(&c.px[((y * 2) * W + x * 2) * 4..((y * 2) * W + x * 2) * 4 + 4]);
                                    let o = (y * W + col * w2 + x) * 4;
                                    img[o..o + 4].copy_from_slice(&[v[0] as u8, v[1] as u8, v[2] as u8, 255]);
                                }
                            }
                        }
                        worst = Some((score, img));
                    }
                }
                movie.tick();
            }
        }
        if let Some((_, img)) = &worst {
            write_png(&dir.join(format!("{name}.png")), W as u32, (H / 2) as u32, img).unwrap();
        }
        println!("{name}: {} sprite-mask sprites, {samples} samples | >8 {over8}, >32 {over32}, brighter {brighter}, darker {darker}", probes.len());
    }
}
