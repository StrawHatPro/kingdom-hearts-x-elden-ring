//! THE HUD'S PICTURES, shared by every host (moved from kh-app/src/gfx_hud/mod.rs): each movie of the HUD is
//! one LAYER, rasterised by kh-ui's software compositor at the size it has on screen, only when its display
//! state changed and only the rectangle that changed (dirty rectangles); the menu movies (full-screen pictures
//! that take longer than a frame) are rasterised on a worker thread. A host shows each layer's picture as one
//! textured rectangle, in the order of [`LAYERS`] (the test app: a Bevy UI image per layer; Elden Ring: a
//! Direct3D 12 texture per layer, kh-er-render/src/hud.rs).
//!
//! THE STAGE ON SCREEN (docs/sora-ui-spec.md 11a): the 1920 x 1080 stage is stretched over the centred 16:9
//! rectangle of the output (ExactFit; bars on other aspect ratios): [`stage_root`].

use std::borrow::Cow;
use std::sync::Mutex;
use std::time::Instant;

use kh_ui::render::state_hash;
use kh_ui::{Canvas, Matrix, Movie, Renderer};

use crate::hud::Hud;

/// The movies in drawing order (back to front).
/// (cutscene) "CutscenePause" = the cutscene pause menu (the Pause movie's Continue / Skip Scene), alone on screen.
/// (new game) "NewGameText" / "NewGameDialog" = the new-game choice menu's words and window, alone on screen.
/// (CV1) "SpeechBubble" = the conversation system's speech bubble (kh_ui::bubble), over everything of the play.
pub const LAYERS: [&str; 15] = ["HudTarget", "HudStatus", "HudCommand", "HudLeft", "HudRight", "Pause", "CampMenu", "GameHelp", "Dialog", "GameOver", "SavePoint", "CutscenePause", "NewGameText", "NewGameDialog", "SpeechBubble"];
/// The first menu layer of `LAYERS` (the ones before it are the cockpit, drawn in place).
pub const FIRST_MENU_LAYER: usize = 5;

/// A rectangle of the output in pixels: x, y, width, height.
pub type PxRect = (i32, i32, usize, usize);

/// The stage -> output matrix and the stage's rectangle (left, top, width, height) for an output of
/// `width` x `height` pixels.
pub fn stage_root(width: f32, height: f32) -> (Matrix, (f32, f32, f32, f32)) {
    let (left, top, w, h) = kh_ui::frames::stage_viewport(width, height, (1.0, 1.0));
    (Matrix::new(w / 1920.0, 0.0, 0.0, h / 1080.0, left, top), (left, top, w, h))
}

/// What a rasteriser keeps between two pictures of one movie.
#[derive(Default)]
pub struct Draw {
    canvas: Canvas,
    pub rgba: Vec<u8>,
    /// The pieces last drawn (kh_ui::render::parts) and the rectangle of the picture: a change
    /// inside the same rectangle redraws only the part that differs.
    parts: Vec<(u64, kh_ui::Rect)>,
    drawn: PxRect,
    /// (perf) pixels rasterised for the latest picture.
    pub redrawn: usize,
    scratch: Canvas,
    scratch_rgba: Vec<u8>,
    /// (perf) one canvas and byte buffer per parallel band (allocations kept between pictures).
    bands: Vec<(Canvas, Vec<u8>)>,
}

/// (perf) HOW A LAYER'S PICTURE IS MADE. The default is the behaviour before these switches existed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Perf {
    /// The changed pieces are redrawn as SEPARATE rectangles (merged where they overlap or lie close) instead of
    /// their one bounding box: two small things changing at opposite corners of a full-screen menu no longer
    /// redraw the whole menu between them.
    pub split_dirty: bool,
    /// A rectangle of `band_pixels` pixels or more is rasterised in up to this many horizontal bands on as many
    /// threads (1 = on the calling thread). Bands are device-pixel rows: the picture is the same.
    pub bands: usize,
    pub band_pixels: usize,
}

impl Default for Perf {
    fn default() -> Perf {
        Perf { split_dirty: false, bands: 1, band_pixels: 200_000 }
    }
}

/// (perf) The device rectangles that differ between two `Renderer::parts` lists (the pieces of
/// `kh_ui::render::dirty_rect`, not united).
fn dirty_rects(before: &[(u64, kh_ui::Rect)], now: &[(u64, kh_ui::Rect)]) -> Vec<kh_ui::Rect> {
    use std::collections::HashMap;
    let mut count: HashMap<u64, i32> = HashMap::with_capacity(before.len());
    for (hash, _) in before {
        *count.entry(*hash).or_insert(0) += 1;
    }
    let mut out = Vec::new();
    for (hash, rect) in now {
        match count.get_mut(hash) {
            Some(n) if *n > 0 => *n -= 1,
            _ => {
                if !rect.is_empty() {
                    out.push(*rect);
                }
            }
        }
    }
    for (hash, rect) in before {
        if let Some(n) = count.get_mut(hash) {
            if *n > 0 {
                *n -= 1;
                if !rect.is_empty() {
                    out.push(*rect);
                }
            }
        }
    }
    out
}

/// (perf) Integer rectangles (x0, y0, x1, y1) united until no two overlap or would cost less as one.
fn merge_rects(mut rects: Vec<(i32, i32, i32, i32)>) -> Vec<(i32, i32, i32, i32)> {
    let area = |r: &(i32, i32, i32, i32)| (r.2 - r.0).max(0) as i64 * (r.3 - r.1).max(0) as i64;
    loop {
        let mut merged = false;
        'pairs: for a in 0..rects.len() {
            for b in a + 1..rects.len() {
                let (ra, rb) = (rects[a], rects[b]);
                let union = (ra.0.min(rb.0), ra.1.min(rb.1), ra.2.max(rb.2), ra.3.max(rb.3));
                let overlap = ra.0 < rb.2 && rb.0 < ra.2 && ra.1 < rb.3 && rb.1 < ra.3;
                // (one rectangle when they overlap, or when the union is at most a quarter more than the two: a
                // rectangle has a fixed cost of its own)
                if overlap || area(&union) * 4 <= (area(&ra) + area(&rb)) * 5 + 4096 {
                    rects[a] = union;
                    rects.swap_remove(b);
                    merged = true;
                    break 'pairs;
                }
            }
        }
        if !merged {
            return rects;
        }
    }
}

/// (perf) Rasterises the device rectangle (x0, y0, w, h) of the movie into `out` (w * h * 4 bytes, straight alpha):
/// on this thread, or in horizontal bands on scoped threads when it is large and `perf.bands` > 1.
#[allow(clippy::too_many_arguments)]
fn raster_region(pool: &mut Vec<(Canvas, Vec<u8>)>, renderer: &mut Renderer, movie: &Movie, root: &Matrix, x0: i32, y0: i32, w: usize, h: usize, perf: Perf, out: &mut Vec<u8>) {
    let bands = if w * h >= perf.band_pixels.max(1) { perf.bands.min(h / 48).max(1) } else { 1 };
    if pool.len() < bands {
        pool.resize_with(bands, Default::default);
    }
    if bands <= 1 {
        let (canvas, _) = &mut pool[0];
        canvas.reset(x0, y0, w, h);
        renderer.draw(canvas, movie, root);
        canvas.to_rgba8(out);
        return;
    }
    let rows = h.div_ceil(bands);
    let fonts = &renderer.fonts;
    std::thread::scope(|scope| {
        for (k, (canvas, bytes)) in pool.iter_mut().take(bands).enumerate() {
            let from = k * rows;
            let count = rows.min(h.saturating_sub(from));
            if count == 0 {
                bytes.clear();
                continue;
            }
            scope.spawn(move || {
                let mut renderer = Renderer::new(fonts.clone());
                canvas.reset(x0, y0 + from as i32, w, count);
                renderer.draw(canvas, movie, root);
                canvas.to_rgba8(bytes);
            });
        }
    });
    out.clear();
    out.reserve(w * h * 4);
    for (_, bytes) in pool.iter().take(bands) {
        out.extend_from_slice(bytes);
    }
}

pub enum Drawn {
    /// Nothing to show.
    Empty,
    /// Nothing visible changed.
    Same,
    /// `Draw::rgba` holds the picture of this rectangle; `Some` = only that part of it (output
    /// pixels) was redrawn.
    Picture(PxRect, Option<PxRect>),
}

/// Rasterises what changed of a movie into `draw.rgba` (straight alpha, sRGB, 8 bits).
pub fn draw_movie(draw: &mut Draw, renderer: &mut Renderer, movie: &Movie, root: &Matrix, size: (f32, f32), resized: bool) -> Drawn {
    draw_movie_with(draw, renderer, movie, root, size, resized, Perf::default())
}

/// (perf) [`draw_movie`] with the switches of [`Perf`].
pub fn draw_movie_with(draw: &mut Draw, renderer: &mut Renderer, movie: &Movie, root: &Matrix, size: (f32, f32), resized: bool, perf: Perf) -> Drawn {
    draw_movie_in(draw, renderer, movie, root, size, resized, perf, None)
}

/// (ER party S6) [`draw_movie_with`] CLIPPED TO A RECTANGLE of the output (left, top, width, height): KH3's ExactFit
/// 16:9 viewport (0x1438F8E90) shows nothing of a movie outside the stage; a host whose output is wider or taller
/// than the stage (Elden Ring at 21:9 / 16:10) passes the stage's rectangle (`Layers::clip`) so that pieces placed
/// beyond the stage's edge (the friends' gauge plates of the status HUD reach 120 stage px past its right edge) do
/// not show in the pillar / letter box. `None` = the whole output, as before.
#[allow(clippy::too_many_arguments)]
pub fn draw_movie_in(draw: &mut Draw, renderer: &mut Renderer, movie: &Movie, root: &Matrix, size: (f32, f32), resized: bool, perf: Perf, clip: Option<(f32, f32, f32, f32)>) -> Drawn {
    let (width, height) = size;
    let limit = match clip {
        Some((x, y, w, h)) => kh_ui::Rect::new(x.max(0.0), y.max(0.0), (x + w).min(width), (y + h).min(height)),
        None => kh_ui::Rect::new(0.0, 0.0, width, height),
    };
    let bounds = renderer.bounds(movie, root).intersect(&limit);
    if bounds.is_empty() {
        return Drawn::Empty;
    }
    let (x0, y0) = ((bounds.x0 - 1.0).floor().max(limit.x0.floor()) as i32, (bounds.y0 - 1.0).floor().max(limit.y0.floor()) as i32); // (ER party S6) the clip's edge (0 without one)
    let (x1, y1) = ((bounds.x1 + 1.0).ceil().min(limit.x1.ceil()) as i32, (bounds.y1 + 1.0).ceil().min(limit.y1.ceil()) as i32);
    let (cw, ch) = ((x1 - x0).max(1) as usize, (y1 - y0).max(1) as usize);
    let parts = renderer.parts(movie, root);
    let dirty = kh_ui::render::dirty_rect(&draw.parts, &parts).intersect(&bounds);
    // (perf) the same pieces one by one
    let pieces = if perf.split_dirty { dirty_rects(&draw.parts, &parts) } else { Vec::new() };
    draw.parts = parts;
    let same_picture = !resized && draw.drawn == (x0, y0, cw, ch) && draw.rgba.len() == cw * ch * 4;
    draw.drawn = (x0, y0, cw, ch);
    let (dx0, dy0) = (((dirty.x0 - 2.0).floor() as i32).max(x0), ((dirty.y0 - 2.0).floor() as i32).max(y0));
    let (dx1, dy1) = (((dirty.x1 + 2.0).ceil() as i32).min(x1), ((dirty.y1 + 2.0).ceil() as i32).min(y1));
    if same_picture && (dirty.is_empty() || dx1 <= dx0 || dy1 <= dy0) {
        // Nothing visible changed (a hidden clip moved).
        return Drawn::Same;
    }
    // (perf) THE PIECES ONE RECTANGLE EACH: every piece grown by the same 2 pixels the one rectangle gets, cut to the
    // picture, united where they overlap or lie close; taken when together they are well under the one rectangle.
    if same_picture && perf.split_dirty && pieces.len() > 1 {
        let rects: Vec<(i32, i32, i32, i32)> = pieces
            .iter()
            .map(|r| r.intersect(&bounds))
            .filter(|r| !r.is_empty())
            .map(|r| (((r.x0 - 2.0).floor() as i32).max(x0), ((r.y0 - 2.0).floor() as i32).max(y0), ((r.x1 + 2.0).ceil() as i32).min(x1), ((r.y1 + 2.0).ceil() as i32).min(y1)))
            .filter(|r| r.2 > r.0 && r.3 > r.1)
            .collect();
        let rects = merge_rects(rects);
        let total: usize = rects.iter().map(|r| (r.2 - r.0) as usize * (r.3 - r.1) as usize).sum();
        let one = (dx1 - dx0) as usize * (dy1 - dy0) as usize;
        if rects.len() > 1 && total * 10 < one * 8 && total * 10 < cw * ch * 7 {
            for r in &rects {
                let (rw, rh) = ((r.2 - r.0) as usize, (r.3 - r.1) as usize);
                raster_region(&mut draw.bands, renderer, movie, root, r.0, r.1, rw, rh, perf, &mut draw.scratch_rgba);
                for row in 0..rh {
                    let to = (((r.1 - y0) as usize + row) * cw + (r.0 - x0) as usize) * 4;
                    draw.rgba[to..to + rw * 4].copy_from_slice(&draw.scratch_rgba[row * rw * 4..(row + 1) * rw * 4]);
                }
            }
            draw.redrawn = total;
            return Drawn::Picture((x0, y0, cw, ch), Some((dx0, dy0, (dx1 - dx0) as usize, (dy1 - dy0) as usize)));
        }
    }
    if same_picture && ((dx1 - dx0) as usize * (dy1 - dy0) as usize) * 10 < cw * ch * 7 {
        // Only the part that differs.
        let (dw, dh) = ((dx1 - dx0) as usize, (dy1 - dy0) as usize);
        if perf.bands > 1 {
            raster_region(&mut draw.bands, renderer, movie, root, dx0, dy0, dw, dh, perf, &mut draw.scratch_rgba);
        } else {
            draw.scratch.reset(dx0, dy0, dw, dh);
            renderer.draw(&mut draw.scratch, movie, root);
            draw.scratch.to_rgba8(&mut draw.scratch_rgba);
        }
        for row in 0..dh {
            let to = (((dy0 - y0) as usize + row) * cw + (dx0 - x0) as usize) * 4;
            draw.rgba[to..to + dw * 4].copy_from_slice(&draw.scratch_rgba[row * dw * 4..(row + 1) * dw * 4]);
        }
        draw.redrawn = dw * dh;
        Drawn::Picture((x0, y0, cw, ch), Some((dx0, dy0, dw, dh)))
    } else {
        if perf.bands > 1 {
            raster_region(&mut draw.bands, renderer, movie, root, x0, y0, cw, ch, perf, &mut draw.rgba);
        } else {
            draw.canvas.reset(x0, y0, cw, ch);
            renderer.draw(&mut draw.canvas, movie, root);
            draw.canvas.to_rgba8(&mut draw.rgba);
        }
        draw.redrawn = cw * ch;
        Drawn::Picture((x0, y0, cw, ch), None)
    }
}

/// The menu movies are rasterised on a worker thread (the camp menu's full-screen pictures take
/// longer than a frame): the main thread sends a snapshot of the movie and shows the picture
/// when it arrives, a frame or two later. With `wait_for_worker` each picture is waited for
/// (an unattended run's screenshots are exact).
struct Job {
    layer: usize,
    generation: u64,
    movie: Movie,
    root: Matrix,
    size: (f32, f32),
    resized: bool,
    perf: Perf,
    clip: Option<(f32, f32, f32, f32)>, // (ER party S6)
}

struct Done {
    layer: usize,
    generation: u64,
    /// The rectangle, the whole picture and the part that was redrawn; `None` = nothing to show /
    /// nothing changed.
    picture: Option<(PxRect, Vec<u8>, Option<PxRect>)>,
    empty: bool,
    seconds: f64,
    /// (perf) pixels rasterised.
    redrawn: usize,
}

struct Worker {
    /// (`None` only while dropping)
    jobs: Option<std::sync::mpsc::Sender<Job>>,
    done: Mutex<std::sync::mpsc::Receiver<Done>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// The thread has ended when its `Layers` is gone (a host in a DLL that is unloaded next must not leave a
/// thread running its code): the job queue closes, the picture in work is finished, the thread is joined.
impl Drop for Worker {
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn start_worker(fonts: kh_ui::Fonts) -> Option<Worker> {
    let (jobs, job_rx) = std::sync::mpsc::channel::<Job>();
    let (done_tx, done) = std::sync::mpsc::channel::<Done>();
    let thread = std::thread::Builder::new()
        .name("hud-raster".into())
        .spawn(move || {
            let mut renderer = Renderer::new(fonts);
            let mut draws: Vec<Draw> = (0..LAYERS.len()).map(|_| Draw::default()).collect();
            let mut generations: Vec<u64> = vec![0; LAYERS.len()];
            while let Ok(job) = job_rx.recv() {
                let started = Instant::now();
                // (GOOFY-GG) a layer hidden since its last picture (a new generation) starts from a whole new picture, so an
                // unchanged movie that comes back is shown again (the in-place layers' rule in `Layers::update`)
                if generations[job.layer] != job.generation {
                    generations[job.layer] = job.generation;
                    draws[job.layer] = Draw::default();
                }
                let draw = &mut draws[job.layer];
                let drawn = draw_movie_in(draw, &mut renderer, &job.movie, &job.root, job.size, job.resized, job.perf, job.clip);
                // (perf) the time of the drawing itself, without the copy of the picture for the other thread
                let seconds = started.elapsed().as_secs_f64();
                let redrawn = draw.redrawn;
                let (picture, empty) = match drawn {
                    Drawn::Picture(rect, dirty) => (Some((rect, draw.rgba.clone(), dirty)), false),
                    Drawn::Empty => (None, true),
                    Drawn::Same => (None, false),
                };
                if done_tx.send(Done { layer: job.layer, generation: job.generation, picture, empty, seconds, redrawn }).is_err() {
                    break;
                }
            }
        })
        .ok()?;
    Some(Worker { jobs: Some(jobs), done: Mutex::new(done), thread: Some(thread) })
}

/// One layer's bookkeeping (the numbers are for a host's performance line).
pub struct Layer {
    pub name: &'static str,
    /// Hash of the display state last drawn (or sent to the worker).
    hash: u64,
    draw: Draw,
    /// A menu layer whose picture the worker thread is making; results of an older
    /// `generation` (the movie went away meanwhile) are dropped.
    busy: bool,
    generation: u64,
    pub partial: u64,
    pub redraws: u64,
    /// Seconds spent drawing it, and the pixels of its latest picture.
    pub seconds: f64,
    pub pixels: usize,
    /// (perf) when the state now being drawn was sent to the worker.
    sent_at: Option<Instant>,
    /// (IMPL_ER34) States sent to the worker and results taken back from it (a host that draws something of its own
    /// WITH this layer's picture keeps it in step by these: kh_host::menu_portrait::PictureSync).
    pub sent: u64,
    pub taken: u64,
}

impl Layer {
    /// (IMPL_ER34) The worker is making this layer's picture (what is on screen is an older state).
    pub fn busy(&self) -> bool {
        self.busy
    }
}

/// What a host does with one layer after [`Layers::update`].
pub enum LayerEvent<'a> {
    /// The layer's picture is `rgba` (straight alpha, sRGB) at `rect`; `dirty` = only that part
    /// (output pixels, inside `rect`) differs from the picture last shown at the same `rect`.
    Show { rect: PxRect, rgba: Cow<'a, [u8]>, dirty: Option<PxRect> },
    Hide,
}

pub struct Layers {
    renderer: Renderer,
    worker: Option<Worker>,
    /// The worker's pictures are waited for (an unattended run).
    pub wait_for_worker: bool,
    pub layers: Vec<Layer>,
    /// Layers from this index on are rasterised on the worker thread (default: the menu layers). 0 = every
    /// layer: the host's own thread only snapshots the movies (a host whose frame time is precious: Elden Ring).
    pub first_worker_layer: usize,
    force: bool,
    /// (perf) With `trace` every picture that comes back from the worker leaves a line here (layer, the worker's
    /// time, the redrawn area, the time from the request to the picture); the host takes and logs them.
    pub trace: bool,
    pub trace_lines: Vec<String>,
    /// (perf) How the pictures are made ([`Perf`]; the default changes nothing).
    pub perf: Perf,
    /// (ER party S6) The output rectangle the pictures are clipped to ([`draw_movie_in`]; None = the whole output).
    pub clip: Option<(f32, f32, f32, f32)>,
    /// (ER party S6) The layers (indices of [`LAYERS`]) the clip applies to; empty = every layer.
    pub clip_only: Vec<usize>,
}

impl Layers {
    pub fn new(fonts: kh_ui::Fonts, wait_for_worker: bool) -> Layers {
        let layers = LAYERS.iter().map(|name| Layer { name, hash: 0, draw: Draw::default(), busy: false, generation: 0, partial: 0, redraws: 0, seconds: 0.0, pixels: 0, sent_at: None, sent: 0, taken: 0 }).collect();
        Layers { renderer: Renderer::new(fonts.clone()), worker: start_worker(fonts), wait_for_worker, layers, first_worker_layer: FIRST_MENU_LAYER, force: false, trace: false, trace_lines: Vec::new(), perf: Perf::default(), clip: None, clip_only: Vec::new() }
    }

    /// The worker's finished pictures.
    fn take_done(&mut self, block: bool, sink: &mut dyn FnMut(usize, LayerEvent)) {
        let Some(worker) = &self.worker else { return };
        let receiver = worker.done.lock().unwrap();
        loop {
            let done = if block && self.layers.iter().any(|l| l.busy) { receiver.recv().ok() } else { receiver.try_recv().ok() };
            let Some(done) = done else { break };
            let layer = &mut self.layers[done.layer];
            layer.busy = false;
            layer.taken += 1;
            layer.seconds += done.seconds;
            if self.trace && self.trace_lines.len() < 256 {
                let waited = layer.sent_at.map_or(0.0, |at| at.elapsed().as_secs_f64());
                let what = match &done.picture {
                    Some((rect, _, Some(d))) => format!("PART {} x {} = {} px of {} x {}", d.2, d.3, d.2 * d.3, rect.2, rect.3),
                    Some((rect, _, None)) => format!("WHOLE {} x {} = {} px", rect.2, rect.3, rect.2 * rect.3),
                    None if done.empty => "empty".to_string(),
                    None => "same".to_string(),
                };
                self.trace_lines.push(format!("{}: worker {:.1} ms, request -> picture {:.1} ms, {what}, rasterised {} px", layer.name, done.seconds * 1000.0, waited * 1000.0, done.redrawn));
            }
            if done.generation != layer.generation {
                continue;
            }
            match done.picture {
                Some((rect, rgba, dirty)) => {
                    layer.partial += dirty.is_some() as u64;
                    layer.pixels = rect.2 * rect.3;
                    sink(done.layer, LayerEvent::Show { rect, rgba: Cow::Owned(rgba), dirty });
                }
                None if done.empty => sink(done.layer, LayerEvent::Hide),
                None => {}
            }
        }
    }

    /// Draws what changed. `root` = the stage -> output matrix, `size` = the output in pixels,
    /// `resized` = either changed since the last call (everything is drawn again).
    pub fn update(&mut self, hud: &Hud, root: &Matrix, size: (f32, f32), resized: bool, sink: &mut dyn FnMut(usize, LayerEvent)) {
        let resized = resized || std::mem::take(&mut self.force);
        self.take_done(false, sink);
        for index in 0..self.layers.len() {
            let layer = &mut self.layers[index];
            let Some(movie): Option<&Movie> = hud.movie(layer.name) else {
                // A menu movie that is not up.
                if layer.hash != 0 {
                    layer.hash = 0;
                    layer.generation += 1;
                    // (GOOFY-GG) the hidden layer forgets its picture: when the movie comes back UNCHANGED (a still status
                    // HUD after a cutscene) the next draw is a whole new picture and is shown, not `Drawn::Same` (which
                    // left the overlay's layer hidden until something in the movie moved: AUD's Elden Ring run, Goofy leading)
                    layer.draw = Draw::default();
                    sink(index, LayerEvent::Hide);
                }
                continue;
            };
            if layer.busy {
                // Its picture is being made; the newest state is sent when that one is back.
                continue;
            }
            let hash = state_hash(movie);
            if hash == layer.hash && !resized {
                continue;
            }
            layer.hash = hash;
            layer.redraws += 1;
            // (ER party S6) the clip of this layer (None = the whole output)
            let clip = self.clip.filter(|_| self.clip_only.is_empty() || self.clip_only.contains(&index));
            if let (true, Some(worker)) = (index >= self.first_worker_layer, &self.worker) {
                if worker.jobs.as_ref().is_some_and(|jobs| jobs.send(Job { layer: index, generation: layer.generation, movie: movie.clone(), root: *root, size, resized, perf: self.perf, clip }).is_ok()) {
                    layer.busy = true;
                    layer.sent += 1;
                    layer.sent_at = Some(Instant::now());
                    continue;
                }
            }
            let layer_started = Instant::now();
            match draw_movie_in(&mut layer.draw, &mut self.renderer, movie, root, size, resized, self.perf, clip) {
                Drawn::Empty => {
                    layer.draw = Draw::default(); // (GOOFY-GG) as above: the picture after an empty one is shown whole
                    sink(index, LayerEvent::Hide)
                }
                Drawn::Same => {}
                Drawn::Picture(rect, dirty) => {
                    layer.partial += dirty.is_some() as u64;
                    layer.pixels = rect.2 * rect.3;
                    sink(index, LayerEvent::Show { rect, rgba: Cow::Borrowed(&layer.draw.rgba), dirty });
                }
            }
            layer.seconds += layer_started.elapsed().as_secs_f64();
        }
        if self.wait_for_worker {
            self.take_done(true, sink);
        }
    }

    /// Everything is drawn again at the next update (the host lost its textures).
    pub fn invalidate(&mut self) {
        self.force = true;
        for layer in &mut self.layers {
            layer.hash = 0;
            layer.generation += 1;
            layer.draw = Draw::default();
        }
    }

    /// "HudStatus 12 / 3 partial (0.41 ms each, 51200 px), ..." for a host's performance line.
    pub fn report(&self) -> String {
        self.layers.iter().map(|l| format!("{} {} / {} partial ({:.2} ms each, {} px)", l.name, l.redraws, l.partial, l.seconds * 1000.0 / l.redraws.max(1) as f64, l.pixels)).collect::<Vec<_>>().join(", ")
    }
}

#[cfg(test)]
mod perf_tests {
    use super::*;

    /// (perf) The switches of `Perf` change HOW a picture is made, not the picture. The Load screen (a full-screen
    /// list: text, images, a cursor that moves) is drawn frame by frame the default way and with the switches:
    /// PARALLEL BANDS give the same bytes after every frame; SPLIT RECTANGLES redraw a part where the default redraws
    /// more, and a redrawn part may differ from the whole picture's pixels by the rasteriser's own rounding (seen: 2
    /// pixels of 1,440,000 by 1 of 255): at most 1 of 255 in at most one pixel of 10,000 is asked for.
    #[test]
    fn split_rectangles_and_bands_give_the_same_picture() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
        for (name, fast, exact) in [("bands", Perf { split_dirty: false, bands: 5, band_pixels: 20_000 }, true), ("split + bands", Perf { split_dirty: true, bands: 5, band_pixels: 20_000 }, false)] {
            let Ok((mut hud, fonts)) = Hud::load(&dir) else {
                eprintln!("no UI pack at {}: nothing compared", dir.display());
                return;
            };
            if !hud.saveload_available() {
                eprintln!("the pack has no Save / Load movie: nothing compared");
                return;
            }
            let slots: Vec<kh_ui::saveload::SlotSummary> = crate::save::slot_summaries(std::path::Path::new("no-such-folder"), 10);
            assert!(hud.open_load(&slots));
            let (root, _) = stage_root(1600.0, 900.0);
            let size = (1600.0, 900.0);
            let (mut plain, mut quick) = (Draw::default(), Draw::default());
            let (mut r1, mut r2) = (Renderer::new(fonts.clone()), Renderer::new(fonts));
            let (mut compared, mut partial, mut saved) = (0, 0, 0usize);
            for frame in 0..40 {
                // (the cursor goes down at frames 20 and 30: pieces change far apart)
                let nav: Vec<(u8, bool)> = if frame == 20 || frame == 30 { vec![(1, false)] } else { Vec::new() };
                hud.saveload_frame(kh_core::PadButtons::NONE, &nav, &[], 1);
                let Some(movie) = hud.movie("NewGameText") else { continue };
                let a = draw_movie(&mut plain, &mut r1, movie, &root, size, frame == 0);
                let b = draw_movie_with(&mut quick, &mut r2, movie, &root, size, frame == 0, fast);
                match (&a, &b) {
                    (Drawn::Picture(ra, _), Drawn::Picture(rb, db)) => {
                        assert_eq!(ra, rb, "{name}, frame {frame}: the picture's rectangle");
                        partial += db.is_some() as u32;
                        saved += plain.redrawn.saturating_sub(quick.redrawn);
                    }
                    (Drawn::Same, Drawn::Same) | (Drawn::Empty, Drawn::Empty) => {}
                    _ => panic!("{name}, frame {frame}: one drew and the other did not"),
                }
                let n = plain.rgba.chunks_exact(4).zip(quick.rgba.chunks_exact(4)).filter(|(a, b)| a != b).count();
                let worst = plain.rgba.iter().zip(&quick.rgba).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
                assert_eq!(plain.rgba.len(), quick.rgba.len(), "{name}, frame {frame}");
                if exact {
                    assert!(n == 0, "{name}, frame {frame}: {n} pixel(s) differ, by at most {worst} of 255");
                } else {
                    assert!(worst <= 1 && n * 10_000 <= plain.rgba.len() / 4, "{name}, frame {frame}: {n} pixel(s) differ, by at most {worst} of 255");
                }
                compared += 1;
            }
            assert!(compared >= 30 && partial >= 2, "{name}: compared {compared} frame(s), {partial} partial");
            eprintln!("{name}: {compared} frames compared, {partial} partial, {saved} px less rasterised than the default way");
        }
    }
}
