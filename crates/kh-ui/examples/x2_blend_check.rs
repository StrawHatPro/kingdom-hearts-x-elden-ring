//! (party X2) BEFORE / AFTER OF THE ADD / SCREEN COVERAGE RULE (raster.rs `blend_layer`, 2026-10-16) over a mid-grey
//! and a bright scene. Every sprite of the listed movies whose own timeline places something with blend 8 (add) or 4
//! (screen) is put alone on the stage, played from frame 1 and from each of its labels, sampled every few frames, and
//! drawn twice (the legacy rule, the current rule) onto a transparent canvas that is then laid over the scene the way the
//! hosts lay the HUD (premultiplied over). Only those layers can differ. Also each movie whole, from each root label.
//!
//!   cargo run -p kh-ui --release --example x2_blend_check -- OUT_DIR
//!
//! Per movie: samples, pixels that changed by more than 8 / 32 (of 255), the largest brightening and the largest
//! darkening (new minus old), and how many changed pixels were a dark veil in the old picture (darker than the scene).
//! The worst samples are written as OUT_DIR/<movie>_<n>.png (old | new over the bright scene, then over grey).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use kh_ui::def::{write_png, Op, SpriteDef};
use kh_ui::{Canvas, Fonts, Matrix, Movie, MovieDef, Pack, Renderer};

const W: usize = 1920;
const H: usize = 1080;
const SCENES: [(&str, [f32; 3]); 2] = [("grey", [0.5, 0.5, 0.5]), ("bright", [0.92, 0.92, 0.88])];

#[derive(Default)]
struct Stats {
    samples: usize,
    over8: [usize; 2],
    over32: [usize; 2],
    brighter: [i32; 2],
    darker: [i32; 2],
    veil: [usize; 2],
}

fn probe_def(def: &MovieDef, ch: u32) -> Arc<MovieDef> {
    let op = Op { depth: 1, ch: Some(ch), matrix: Some(Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: W as f32 / 2.0, ty: H as f32 / 2.0 }), name: Some(Arc::from("probe")), visible: Some(true), ..Op::default() };
    let root = SpriteDef { id: 0, frames: 1, ops: vec![vec![op]], scripts: vec![Vec::new()], ..SpriteDef::default() };
    Arc::new(MovieDef { name: def.name.clone(), stage: def.stage, fps: def.fps, chars: def.chars.clone(), root: Arc::new(root), symbols: def.symbols.clone(), bitmaps: def.bitmaps.clone(), consts: def.consts.clone(), unknown_statements: Vec::new() })
}

fn draw(renderer: &mut Renderer, movie: &Movie, legacy: bool) -> Canvas {
    kh_ui::raster::set_legacy_add_coverage(legacy);
    let mut canvas = Canvas::new(W, H);
    renderer.draw(&mut canvas, movie, &Matrix::IDENTITY);
    kh_ui::raster::set_legacy_add_coverage(false);
    canvas
}

fn over(p: &[f32], bg: [f32; 3]) -> [i32; 3] {
    let a = p[3].clamp(0.0, 1.0);
    let c = |v: f32, b: f32| ((v + b * (1.0 - a)) * 255.0 + 0.5).clamp(0.0, 255.0) as i32;
    [c(p[0], bg[0]), c(p[1], bg[1]), c(p[2], bg[2])]
}

/// Compares one sample; returns the number of pixels over 8 on the bright scene (to rank the samples).
fn compare(old: &Canvas, new: &Canvas, st: &mut Stats) -> usize {
    st.samples += 1;
    let mut score = 0;
    for (k, (_, bg)) in SCENES.iter().enumerate() {
        let b = [(bg[0] * 255.0) as i32, (bg[1] * 255.0) as i32, (bg[2] * 255.0) as i32];
        for (p, q) in old.px.chunks_exact(4).zip(new.px.chunks_exact(4)) {
            if p[3] <= 0.0 && q[3] <= 0.0 {
                continue;
            }
            let (o, n) = (over(p, *bg), over(q, *bg));
            let d: Vec<i32> = (0..3).map(|i| n[i] - o[i]).collect();
            let (hi, lo) = (*d.iter().max().unwrap(), *d.iter().min().unwrap());
            st.brighter[k] = st.brighter[k].max(hi);
            st.darker[k] = st.darker[k].min(lo);
            let m = hi.max(-lo);
            if m > 8 {
                st.over8[k] += 1;
                if k == 1 {
                    score += 1;
                }
                // a veil: the old picture was darker than the scene there
                if (0..3).all(|i| o[i] < b[i] - 8) {
                    st.veil[k] += 1;
                }
            }
            if m > 32 {
                st.over32[k] += 1;
            }
        }
    }
    score
}

fn picture(old: &Canvas, new: &Canvas) -> Vec<u8> {
    // 2 x 2: old | new over the bright scene, old | new over grey (full stage, halved)
    let (w2, h2) = (W / 2, H / 2);
    let mut out = vec![0u8; (W * H * 4) as usize];
    for (row, (_, bg)) in SCENES.iter().rev().enumerate() {
        for (col, c) in [old, new].iter().enumerate() {
            for y in 0..h2 {
                for x in 0..w2 {
                    let i = ((y * 2) * W + x * 2) * 4;
                    let v = over(&c.px[i..i + 4], *bg);
                    let o = (((row * h2 + y) * W) + col * w2 + x) * 4;
                    out[o..o + 4].copy_from_slice(&[v[0] as u8, v[1] as u8, v[2] as u8, 255]);
                }
            }
        }
    }
    out
}

fn add_screen_sprites(def: &MovieDef) -> Vec<u32> {
    let mut ids: Vec<u32> = def.chars.keys().copied().filter(|id| def.sprite(*id).is_some_and(|s| s.ops.iter().flatten().any(|op| matches!(op.blend, Some(8) | Some(4))))).collect();
    ids.sort();
    ids
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(args.first().cloned().unwrap_or_else(|| "build/sora_tmp/party/x2/blend".into()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut pack = Pack::open_default().expect("the UI pack");
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut renderer = Renderer::new(fonts.clone());
    let movies = ["HudStatus", "HudCommand", "HudTarget", "HudLeft", "HudRight", "HudInput", "FacePcExSora01", "FaceNpcExDonald", "FaceNpcExGoofy", "FaceNpcExSora", "SaveLoad", "Dialog", "Pause", "CenterInformation", "ItemListLibrary", "SystemLibrary", "MainLibrary", "KeyItem"];
    let mut total = Stats::default();
    for name in movies {
        let Ok(def) = pack.movie(name) else {
            println!("{name}: not in the pack");
            continue;
        };
        let mut st = Stats::default();
        let mut worst: Vec<(usize, Vec<u8>)> = Vec::new();
        let keep = |score: usize, old: &Canvas, new: &Canvas, worst: &mut Vec<(usize, Vec<u8>)>| {
            if score > 0 && (worst.len() < 3 || worst.iter().any(|w| w.0 < score)) {
                worst.push((score, picture(old, new)));
                worst.sort_by(|a, b| b.0.cmp(&a.0));
                worst.truncate(3);
            }
        };
        // every add / screen sprite alone, from frame 1 and from each label
        let ids = add_screen_sprites(&def);
        for id in &ids {
            let probe = probe_def(&def, *id);
            let labels: Vec<String> = probe.sprite(*id).map(|s| s.labels.iter().map(|l| l.0.clone()).collect()).unwrap_or_default();
            for start in std::iter::once(None).chain(labels.iter().map(Some)) {
                let mut movie = Movie::new(probe.clone());
                movie.tick();
                if let Some(label) = start {
                    movie.goto_and_play("probe", label);
                }
                for f in 0..48 {
                    if f % 4 == 0 {
                        let (old, new) = (draw(&mut renderer, &movie, true), draw(&mut renderer, &movie, false));
                        let score = compare(&old, &new, &mut st);
                        keep(score, &old, &new, &mut worst);
                    }
                    movie.tick();
                }
            }
        }
        // the whole movie from each root label
        let labels: Vec<String> = def.root.labels.iter().map(|l| l.0.clone()).collect();
        for start in std::iter::once(None).chain(labels.iter().map(Some)) {
            let mut movie = Movie::new(def.clone());
            movie.tick();
            if let Some(label) = start {
                movie.goto_and_play("", label);
            }
            for f in 0..40 {
                if f % 8 == 0 {
                    let (old, new) = (draw(&mut renderer, &movie, true), draw(&mut renderer, &movie, false));
                    let score = compare(&old, &new, &mut st);
                    keep(score, &old, &new, &mut worst);
                }
                movie.tick();
            }
        }
        for (n, (_, img)) in worst.iter().enumerate() {
            write_png(&dir.join(format!("{name}_{n}.png")), W as u32, H as u32, img).unwrap();
        }
        println!(
            "{name}: {} add/screen sprites, {} samples | grey: >8 {} (veil {}), >32 {}, brighter {}, darker {} | bright: >8 {} (veil {}), >32 {}, brighter {}, darker {}",
            ids.len(), st.samples, st.over8[0], st.veil[0], st.over32[0], st.brighter[0], st.darker[0], st.over8[1], st.veil[1], st.over32[1], st.brighter[1], st.darker[1]
        );
        total.samples += st.samples;
        for k in 0..2 {
            total.over8[k] += st.over8[k];
            total.over32[k] += st.over32[k];
            total.veil[k] += st.veil[k];
            total.brighter[k] = total.brighter[k].max(st.brighter[k]);
            total.darker[k] = total.darker[k].min(st.darker[k]);
        }
    }
    println!("ALL: {} samples | grey: >8 {} (veil {}), brighter {}, darker {} | bright: >8 {} (veil {}), brighter {}, darker {}", total.samples, total.over8[0], total.veil[0], total.brighter[0], total.darker[0], total.over8[1], total.veil[1], total.brighter[1], total.darker[1]);
    let _ = Path::new(".");
}
