//! (ZDRAW) ZETTAFLARE'S TARGET DRAWN BY US, FROM ITS OWN MODEL. The user: "it has to look as perfect as the test level".
//! The test level's look (kh-app zetta_crumble.rs) is a MASK of the body's own triangles as drawn (the whole body skinned
//! from its joints; then its 3D Voronoi pieces, their cracks and ash) coloured over the picture by the overlay. Elden Ring's
//! screen route (finale.rs) had to guess that mask from the game's pixels (stencil + depth + a capsule): on a standing enemy it
//! came out as a small, smeared, mottled blob. Here the mask is drawn from the TARGET'S OWN MODEL: kh-er reads its FLVER from
//! the game's files (kh-erfile) and its bones from the game's memory every frame, this module skins it (the pure part below,
//! no GPU), breaks it apart exactly as kh-app does (`Crumble`, a line-for-line port in game metres, z up) and writes its
//! triangles into finale.rs's mask (`draw_mask`), whose comp is the test level's overlay (er_char.wgsl `fz_look`). Generic:
//! any character model, any number of meshes; meshes in a display group the character does not show are left out
//! (`disp_group`); a body whose model or pose cannot be had keeps finale.rs's screen route (kh-er zdraw.rs decides, and logs
//! the route per target).
//!
//! As the test level draws it: the mask is drawn 960 pixels wide (the target's aspect) and laid over the full-size one with
//! linear filtering (the same crack widths and soft edges at any screen size); the cracks and embers are tested against the
//! body's own depth in that small mask (`CRACK_NUDGE`), so on an Elden Ring model (armour over cloth over skin) they show on
//! the surface seen, as on the test level's single-layer Sora. The body IN THE LIGHT (a survivor, and a killed one before its
//! silhouette has darkened) is its own model in the test level's light grey (`IMMERSED`, `draw_light`), faded in by
//! `immerse`, behind whatever the scene depth holds nearer. The break-apart is cut on a thread of its own ahead of the first
//! crumble step while the body is held (`Driver::threaded`).
//!
//! Spaces: the model and its bones in Elden Ring's space (metres, y up); everything handed here by the host in GAME METRES
//! (the renderer's game space, KH3 axes: x forward, y right, z up, divided by 100); the soup handed to the GPU in game
//! centimetres (the space of finale.rs `View::clip`).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use glam::{Mat4, Quat, Vec3};

// ------------------------------------------------------------------------------------------------ the model

/// A character's model as the host read it (kh-er zdraw.rs from kh-erfile): flat arrays, Elden Ring model space (m, y up).
#[derive(Clone, Debug, Default)]
pub struct Model {
    pub name: String,
    pub bone_names: Vec<String>,
    /// The inverse of each bone's bind (model space).
    pub inv_bind: Vec<Mat4>,
    /// (round 3) each bone's parent (-1: none) and bind (model space): a bone the game's pose leaves unreadable is placed
    /// from its parent (`mend_pose`)
    pub parent: Vec<i32>,
    pub bind: Vec<Mat4>,
    pub positions: Vec<Vec3>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    /// Its meshes: their triangles in `indices` (first index, index count) and display group.
    pub meshes: Vec<MeshPart>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct MeshPart {
    pub first: usize,
    pub count: usize,
    /// The display group its material's name gives (`#NN#` / `＃NN＃` first): shown only when the character's mask has it.
    pub group: Option<u8>,
    pub material: String,
}

/// At most this many triangles of a model are drawn each frame (a model over it is reduced once, in its bind pose: the mask
/// is 960 pixels wide, its detail is not lost).
pub const DRAW_TRIS: usize = 60_000;

impl Model {
    /// The model with at most `max` triangles: its vertices clustered on a grid in the bind pose (each cluster drawn by its
    /// first vertex, which keeps its own skin), the triangles that collapse or repeat dropped, mesh by mesh (their display
    /// groups kept). The grid grows until it fits.
    pub fn reduced(&self, max: usize) -> Model {
        if self.indices.len() / 3 <= max || self.positions.is_empty() {
            return self.clone();
        }
        let (lo, hi) = self.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
        let mut cell = ((hi - lo).length() / 1500.0).max(1e-4);
        loop {
            let mut rep: HashMap<(i32, i32, i32), u32> = HashMap::new();
            let first: Vec<u32> = self.positions.iter().enumerate().map(|(i, p)| *rep.entry(((p.x / cell).floor() as i32, (p.y / cell).floor() as i32, (p.z / cell).floor() as i32)).or_insert(i as u32)).collect();
            let mut out = Model { meshes: Vec::new(), indices: Vec::new(), ..self.clone() };
            for m in &self.meshes {
                let start = out.indices.len();
                let mut seen: std::collections::HashSet<[u32; 3]> = std::collections::HashSet::new();
                for t in self.indices[m.first..m.first + m.count].chunks_exact(3) {
                    let r = [t[0], t[1], t[2]].map(|i| first.get(i as usize).copied().unwrap_or(i));
                    if r[0] == r[1] || r[1] == r[2] || r[0] == r[2] {
                        continue;
                    }
                    let mut key = r;
                    key.sort_unstable();
                    if seen.insert(key) {
                        out.indices.extend(r);
                    }
                }
                out.meshes.push(MeshPart { first: start, count: out.indices.len() - start, group: m.group, material: m.material.clone() });
            }
            if out.indices.len() / 3 <= max || cell > (hi - lo).length() {
                return out;
            }
            cell *= 1.3;
        }
    }
}

/// (round 3, Margit in game: "its bones cannot be read this frame") THE GAME'S POSE MENDED: the bones whose matrices are
/// not usable (`ok` false: not finite, absurd, far from the body) placed from their parent's (the bind's turn between them),
/// up to the first usable ancestor; a bone with none, from the first usable bone by the bind (rigid). Returns how many were
/// mended, or None when no bone at all is usable.
pub fn mend_pose(model: &Model, world: &mut [Mat4], ok: &[bool]) -> Option<usize> {
    let n = world.len().min(ok.len());
    let first = (0..n).find(|k| ok[*k])?;
    let bind = |k: usize| model.bind.get(k).copied().unwrap_or_else(|| model.inv_bind.get(k).map_or(Mat4::IDENTITY, |m| m.inverse()));
    let mut done: Vec<bool> = ok[..n].to_vec();
    let mut mended = 0;
    for k in 0..n {
        if done[k] {
            continue;
        }
        // its chain up to a usable (or mended) ancestor
        let mut chain = vec![k];
        let mut p = model.parent.get(k).copied().unwrap_or(-1);
        while p >= 0 && (p as usize) < n && !done[p as usize] && chain.len() <= n {
            chain.push(p as usize);
            p = model.parent.get(p as usize).copied().unwrap_or(-1);
        }
        let (mut at, mut from) = if p >= 0 && (p as usize) < n && done[p as usize] { (world[p as usize], p as usize) } else { (world[first], first) };
        for j in chain.into_iter().rev() {
            at = at * bind(from).inverse() * bind(j);
            world[j] = at;
            done[j] = true;
            from = j;
            mended += 1;
        }
    }
    Some(mended)
}

/// A material name's display group: a leading `#NN#` (also in full-width signs, as c4311's are). None: always shown.
pub fn disp_group(material: &str) -> Option<u8> {
    let s = material.trim_start();
    let s = s.strip_prefix('#').or_else(|| s.strip_prefix('＃'))?;
    let end = s.find(['#', '＃'])?;
    let digits = &s[..end];
    (!digits.is_empty() && digits.len() <= 2 && digits.chars().all(|c| c.is_ascii_digit())).then(|| digits.parse().ok()).flatten()
}

/// One triangle of the body as drawn (game metres, z up).
#[derive(Clone, Copy, Debug)]
pub struct Tri {
    pub p: [Vec3; 3],
    pub n: Vec3,
    pub c: Vec3,
}

impl Tri {
    pub fn new(a: Vec3, b: Vec3, c: Vec3) -> Option<Tri> {
        let n = (b - a).cross(c - a);
        (n.length_squared() > 1e-14 && a.is_finite() && b.is_finite() && c.is_finite()).then(|| Tri { p: [a, b, c], n: n.normalize(), c: (a + b + c) / 3.0 })
    }
}

/// THE BODY AS DRAWN NOW: every shown mesh skinned from the bones' world matrices (Elden Ring world, the order of the model's
/// bones), into game metres by `to_game`. `shown(group)`: the character shows that display group. Triangles collapsed by a
/// bone scaled to nothing (how the game hides parts) are left out.
pub fn skin(model: &Model, world: &[Mat4], shown: &dyn Fn(u8) -> bool, to_game: &dyn Fn(Vec3) -> Vec3) -> Vec<Tri> {
    let skins: Vec<Mat4> = world.iter().zip(&model.inv_bind).map(|(w, b)| *w * *b).collect();
    let mut placed: Vec<Option<Vec3>> = vec![None; model.positions.len()];
    let mut at = |i: usize| -> Vec3 {
        if let Some(p) = placed[i] {
            return p;
        }
        let v = model.positions[i];
        let (j, w) = (model.joints[i], model.weights[i]);
        let mut acc = Vec3::ZERO;
        let mut sum = 0.0;
        for k in 0..4 {
            if w[k] > 0.0 {
                if let Some(m) = skins.get(j[k] as usize) {
                    acc += m.transform_point3(v) * w[k];
                    sum += w[k];
                }
            }
        }
        let p = to_game(if sum > 1e-4 { acc / sum } else { v });
        placed[i] = Some(p);
        p
    };
    let mut out = Vec::new();
    let n = model.positions.len();
    for m in &model.meshes {
        if m.group.is_some_and(|g| !shown(g)) {
            continue;
        }
        for t in model.indices[m.first..m.first + m.count].chunks_exact(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            if a >= n || b >= n || c >= n {
                continue;
            }
            if let Some(tri) = Tri::new(at(a), at(b), at(c)) {
                out.push(tri);
            }
        }
    }
    out
}

/// At most this many triangles go into the break-apart (a body over it is simplified by clustering its vertices first).
pub const CRUMBLE_TRIS: usize = 40_000;

/// A body's triangles with fewer of them (vertices clustered on a grid of `cell` m; collapsed triangles dropped).
pub fn simplify(tris: &[Tri], cell: f32) -> Vec<Tri> {
    let key = |p: Vec3| ((p.x / cell).round() as i32, (p.y / cell).round() as i32, (p.z / cell).round() as i32);
    let mut sums: HashMap<(i32, i32, i32), (Vec3, f32)> = HashMap::new();
    for t in tris {
        for p in t.p {
            let e = sums.entry(key(p)).or_insert((Vec3::ZERO, 0.0));
            e.0 += p;
            e.1 += 1.0;
        }
    }
    let mut seen: std::collections::HashSet<[(i32, i32, i32); 3]> = std::collections::HashSet::new();
    tris.iter()
        .filter_map(|t| {
            let k = t.p.map(key);
            if k[0] == k[1] || k[1] == k[2] || k[0] == k[2] {
                return None;
            }
            let mut sorted = k;
            sorted.sort();
            if !seen.insert(sorted) {
                return None;
            }
            let at = |k: (i32, i32, i32)| sums.get(&k).map_or(Vec3::ZERO, |s| s.0 / s.1);
            Tri::new(at(k[0]), at(k[1]), at(k[2]))
        })
        .collect()
}

/// A body simplified until it is within `CRUMBLE_TRIS` (the cell grown step by step from `height` / 400).
pub fn within_budget(tris: Vec<Tri>, height: f32) -> Vec<Tri> {
    if tris.len() <= CRUMBLE_TRIS {
        return tris;
    }
    // (the first grid from the mean edge: the triangles go as the square of the cell, so one or two passes fit)
    let edge = tris.iter().map(|t| (t.p[0].distance(t.p[1]) + t.p[1].distance(t.p[2]) + t.p[2].distance(t.p[0])) / 3.0).sum::<f32>() / tris.len() as f32;
    let mut cell = (edge * 0.8 * (tris.len() as f32 / CRUMBLE_TRIS as f32).sqrt()).max((height / 400.0).max(0.002));
    let mut out = simplify(&tris, cell);
    while out.len() > CRUMBLE_TRIS && cell < height {
        cell *= 1.4;
        out = simplify(&tris, cell);
    }
    out
}

// ------------------------------------------------------------------------------------------------ the break-apart
// kh-app zetta_crumble.rs, line for line, in game metres with z up (the test level's y up): the same constants, timeline,
// seeds, cells, chunks, fragments, cracks and ash.

/// The chunks a body breaks into.
pub const PIECES: usize = 56;
/// The fragments each chunk breaks into.
pub const SPLIT: usize = 3;
/// The crumble's seconds (`zetta.finale_crumble_time`): the flights' clock.
pub const SECS: f32 = 2.2;

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hash01(seed: u32, k: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x & 0x00FF_FFFF) as f32 / 16_777_216.0
}

fn rand_dir(seed: u32, k: u32) -> Vec3 {
    let z = hash01(seed, k) * 2.0 - 1.0;
    let a = hash01(seed, k + 1) * std::f32::consts::TAU;
    let r = (1.0 - z * z).max(0.0).sqrt();
    Vec3::new(r * a.cos(), r * a.sin(), z)
}

type Q3 = (i32, i32, i32);

fn q(p: Vec3) -> Q3 {
    ((p.x * 2000.0).round() as i32, (p.y * 2000.0).round() as i32, (p.z * 2000.0).round() as i32)
}

fn edge_key(a: Vec3, b: Vec3) -> (Q3, Q3) {
    let (qa, qb) = (q(a), q(b));
    if qa <= qb {
        (qa, qb)
    } else {
        (qb, qa)
    }
}

/// Farthest-point seeds over the triangles' centres (a sample of at most 3000), the first the top-most.
fn seeds(tris: &[Tri], ids: &[usize], k: usize) -> Vec<Vec3> {
    if ids.is_empty() {
        return Vec::new();
    }
    let stride = (ids.len() / 3000).max(1);
    let sample: Vec<Vec3> = ids.iter().step_by(stride).map(|&i| tris[i].c).collect();
    let first = sample.iter().copied().fold(sample[0], |a, b| if b.z > a.z { b } else { a });
    let mut out = vec![first];
    let mut best: Vec<f32> = sample.iter().map(|p| p.distance_squared(first)).collect();
    while out.len() < k.min(sample.len()) {
        let (i, _) = best.iter().enumerate().fold((0, -1.0), |a, (i, d)| if *d > a.1 { (i, *d) } else { a });
        let s = sample[i];
        out.push(s);
        for (b, p) in best.iter_mut().zip(&sample) {
            *b = b.min(p.distance_squared(s));
        }
    }
    out
}

fn nearest(seeds: &[Vec3], p: Vec3) -> usize {
    seeds.iter().enumerate().fold((0, f32::MAX), |a, (i, s)| {
        let d = s.distance_squared(p);
        if d < a.1 {
            (i, d)
        } else {
            a
        }
    }).0
}

/// A chunk's shape about `centre`: its own surface and, on its open edges, the side walls of its thickness (its back face, the
/// test level's too, covers what the front covers: no mask needs it).
fn chunk_mesh(tris: &[Tri], ids: &[usize], centre: Vec3, thick: f32) -> Vec<[Vec3; 3]> {
    let mut out: Vec<[Vec3; 3]> = Vec::with_capacity(ids.len() * 2);
    let mut edges: HashMap<(Q3, Q3), (usize, usize, u32)> = HashMap::new();
    for &i in ids {
        let t = &tris[i];
        out.push([t.p[0] - centre, t.p[1] - centre, t.p[2] - centre]);
        for k in 0..3 {
            let e = edges.entry(edge_key(t.p[k], t.p[(k + 1) % 3])).or_insert((i, k, 0));
            e.2 += 1;
        }
    }
    for (_, (i, k, n)) in edges {
        if n != 1 {
            continue;
        }
        let t = &tris[i];
        let (a, b) = (t.p[k] - centre, t.p[(k + 1) % 3] - centre);
        let (a2, b2) = (a - t.n * thick, b - t.n * thick);
        out.push([a, b2, b]);
        out.push([a, a2, b2]);
    }
    out
}

struct Frag {
    geo: Vec<[Vec3; 3]>,
    offset: Vec3,
    dv: Vec3,
    axis: Vec3,
    rate: f32,
    dissolve: (f32, f32),
    size: f32,
    seed: u32,
}

struct Chunk {
    geo: Vec<[Vec3; 3]>,
    centre: Vec3,
    v0: Vec3,
    axis: Vec3,
    rate: f32,
    release: f32,
    split: f32,
    size: f32,
    frags: Vec<Frag>,
}

struct Crack {
    a: Vec3,
    b: Vec3,
    n: Vec3,
    reveal: f32,
    pieces: (usize, usize),
}

struct Ash {
    p0: Vec3,
    v0: Vec3,
    born: f32,
    end: f32,
    size: f32,
    axis: Vec3,
    ember: bool,
}

/// One body's break-apart, from its snapshot (the first crumble step).
pub struct Crumble {
    pub id: u32,
    floor: f32,
    scale: f32,
    push: Vec3,
    chunks: Vec<Chunk>,
    cracks: Vec<Crack>,
    ash: Vec<Ash>,
    pub triangles: usize,
}

/// The regular tetrahedron of the ash's grit (Bevy's `Tetrahedron::default()`).
const GRIT: [Vec3; 4] = [Vec3::new(0.5, 0.5, 0.5), Vec3::new(-0.5, 0.5, -0.5), Vec3::new(-0.5, -0.5, 0.5), Vec3::new(0.5, -0.5, -0.5)];

impl Crumble {
    /// The snapshot: `tris` the body as drawn now (game metres), `floor` its feet's height, `axis` its feet (only x, y used),
    /// `height` and `radius` its size (m), `push` the beam's direction at it (game axes; zero: none).
    pub fn new(id: u32, tris: &[Tri], floor: f32, axis: Vec3, height: f32, radius: f32, push: Vec3) -> Crumble {
        let seed = id.wrapping_mul(2_654_435_761) ^ 0x5EED;
        let height = height.max(0.2);
        let scale = (height / 1.8).max(0.1);
        let push = if push.length_squared() > 1e-6 { push.normalize() } else { Vec3::ZERO };
        let all: Vec<usize> = (0..tris.len()).collect();
        let seeds1 = seeds(tris, &all, PIECES);
        let mut cell = vec![0usize; tris.len()];
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); seeds1.len()];
        for (i, t) in tris.iter().enumerate() {
            let k = nearest(&seeds1, t.c);
            cell[i] = k;
            cells[k].push(i);
        }
        let axis_c = Vec3::new(axis.x, axis.y, 0.0);
        let thick = 0.018 * scale;
        let mut chunks = Vec::new();
        let mut ash = Vec::new();
        let sweep = push * scale.sqrt();
        for (k, ids) in cells.iter().enumerate() {
            if ids.is_empty() {
                chunks.push(Chunk { geo: Vec::new(), centre: Vec3::ZERO, v0: Vec3::ZERO, axis: Vec3::Z, rate: 0.0, release: 1.0, split: 1.0, size: 0.0, frags: Vec::new() });
                continue;
            }
            let ku = k as u32 * 37;
            let centre = ids.iter().map(|&i| tris[i].c).sum::<Vec3>() / ids.len() as f32;
            let size = ids.iter().map(|&i| tris[i].c.distance(centre)).fold(0.0, f32::max).max(0.01);
            let up = ((centre.z - floor) / height).clamp(0.0, 1.0);
            let (release, split, _, _) = crate::finale::timeline(up, hash01(seed, ku), 0.0);
            let out = Vec3::new(centre.x - axis_c.x, centre.y - axis_c.y, 0.0).normalize_or_zero();
            let v0 = if push.length_squared() > 0.5 {
                (push * (2.2 + 1.8 * hash01(seed, ku + 3)) + out * (0.25 + 0.35 * hash01(seed, ku + 1)) + Vec3::Z * (0.1 + 0.3 * hash01(seed, ku + 2)) + rand_dir(seed, ku + 4) * 0.3) * scale.sqrt()
            } else {
                (out * (0.5 + 1.1 * hash01(seed, ku + 1)) + Vec3::Z * (0.9 + 1.4 * hash01(seed, ku + 2)) + rand_dir(seed, ku + 4) * 0.35) * scale.sqrt()
            };
            let c_axis = rand_dir(seed, ku + 6);
            let c_rate = 2.0 + 5.0 * hash01(seed, ku + 8);
            // where it breaks into its fragments (closed form: the place, turn and velocity at the split)
            let age_split = (split - release) * SECS;
            let (p_split, land_split) = crate::finale::swept(centre, v0, sweep, floor + size * 0.3, age_split);
            let v_split = crate::finale::fly(centre, v0, floor + size * 0.3, age_split).1 + sweep * (1.6 * age_split);
            let r_split = Quat::from_axis_angle(c_axis, c_rate * age_split.min(land_split));
            let s2 = seeds(tris, ids, SPLIT);
            let mut sub: Vec<Vec<usize>> = vec![Vec::new(); s2.len()];
            for &i in ids {
                sub[nearest(&s2, tris[i].c)].push(i);
            }
            let mut frags = Vec::new();
            for (j, sids) in sub.iter().enumerate() {
                if sids.is_empty() {
                    continue;
                }
                let ju = ku * 11 + j as u32 * 7 + 1000;
                let fc = sids.iter().map(|&i| tris[i].c).sum::<Vec3>() / sids.len() as f32;
                let fsize = sids.iter().map(|&i| tris[i].c.distance(fc)).fold(0.0, f32::max).max(0.006);
                let (_, _, d0, d1) = crate::finale::timeline(up, hash01(seed, ku), hash01(seed, ju));
                let fr = Frag {
                    geo: chunk_mesh(tris, sids, fc, thick * 0.8),
                    offset: fc - centre,
                    dv: ((fc - centre).normalize_or_zero() * 0.35 + rand_dir(seed, ju + 1) * 0.3 + push * 0.8 + Vec3::Z * 0.1) * scale.sqrt(),
                    axis: rand_dir(seed, ju + 3),
                    rate: 3.0 + 6.0 * hash01(seed, ju + 5),
                    dissolve: (d0, d1),
                    size: fsize,
                    seed: ju,
                };
                // its ash, shed as it starts to dissolve (where it is then)
                let age = (d0 - split) * SECS;
                let (p, _) = crate::finale::swept(p_split + r_split * fr.offset, v_split + fr.dv, sweep, floor + fr.size * 0.25, age);
                for a in 0..5u32 {
                    let id = fr.seed * 13 + a + k as u32;
                    let at = p + rand_dir(seed, id) * fr.size * 0.5;
                    let ember = a == 0;
                    let size = (0.012 + 0.02 * hash01(seed, id + 7)) * scale * if ember { 0.6 } else { 1.0 };
                    let v = (Vec3::Z * (0.1 + 0.2 * hash01(seed, id + 9)) + push * (1.2 + 0.8 * hash01(seed, id + 10)) + rand_dir(seed, id + 11) * 0.25) * scale.sqrt();
                    ash.push(Ash { p0: at, v0: v, born: d0, end: (d0 + 0.12 + 0.1 * hash01(seed, id + 13)).min(1.0), size, axis: rand_dir(seed, id + 15), ember });
                }
                frags.push(fr);
            }
            chunks.push(Chunk { geo: chunk_mesh(tris, ids, centre, thick), centre, v0, axis: c_axis, rate: c_rate, release, split, size, frags });
        }
        // the cracks: the cells' borders, spreading from the side the beam strikes
        let mut shared: HashMap<(Q3, Q3), Vec<(usize, Vec3, Vec3, Vec3)>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            for k in 0..3 {
                shared.entry(edge_key(t.p[k], t.p[(k + 1) % 3])).or_default().push((cell[i], t.n, t.p[k], t.p[(k + 1) % 3]));
            }
        }
        let origin = Vec3::new(axis_c.x, axis_c.y, floor + height * 0.62) - push * radius;
        let mut cracks: Vec<Crack> = shared.values().filter(|v| v.len() == 2 && v[0].0 != v[1].0).map(|v| Crack { a: v[0].2, b: v[0].3, n: (v[0].1 + v[1].1).normalize_or(v[0].1), reveal: 0.0, pieces: (v[0].0, v[1].0) }).collect();
        // (a fixed order: the map's order is not the same every run)
        cracks.sort_by(|a, b| q(a.a).cmp(&q(b.a)).then(q(a.b).cmp(&q(b.b))));
        let far = cracks.iter().map(|c| ((c.a + c.b) * 0.5).distance(origin)).fold(1e-3, f32::max);
        for (i, c) in cracks.iter_mut().enumerate() {
            c.reveal = crate::finale::crack_reveal(((c.a + c.b) * 0.5).distance(origin) / far, hash01(seed, 50_000 + i as u32));
        }
        Crumble { id, floor, scale, push, chunks, cracks, ash, triangles: tris.len() }
    }

    /// Its mask at crumble `k` (0..1): triangles in game CENTIMETRES with their channel (1: the silhouette and its pieces, 2:
    /// the light: cracks, embers), as `[x, y, z, channel]` per vertex. Nothing at 1.
    pub fn soup(&self, k: f32, out: &mut Vec<[f32; 4]>) {
        let k = k.clamp(0.0, 1.0);
        let mut push_tri = |a: Vec3, b: Vec3, c: Vec3, ch: f32| {
            for p in [a, b, c] {
                out.push([p.x * 100.0, p.y * 100.0, p.z * 100.0, ch]);
            }
        };
        // cracks
        let w = 0.011 * self.scale;
        let lift = 0.0025 * self.scale;
        for cr in &self.cracks {
            let gone = self.chunks.get(cr.pieces.0).map_or(1.0, |p| p.release).min(self.chunks.get(cr.pieces.1).map_or(1.0, |p| p.release));
            if k < cr.reveal || k >= gone {
                continue;
            }
            let grow = smooth(cr.reveal, cr.reveal + 0.05, k);
            let (a, b) = (cr.a + cr.n * lift, cr.b + cr.n * lift);
            let mid = (a + b) * 0.5;
            let (a, b) = (mid + (a - mid) * grow, mid + (b - mid) * grow);
            let open = 1.0 + 0.8 * smooth(gone - 0.08, gone, k);
            let side = (b - a).cross(cr.n).normalize_or_zero() * (w * 0.5 * open);
            let rise = cr.n * (w * 0.5 * open);
            push_tri(a - side, b - side, b + side, 2.0);
            push_tri(a - side, b + side, a + side, 2.0);
            push_tri(a - rise, b - rise, b + rise, 2.0);
            push_tri(a - rise, b + rise, a + rise, 2.0);
        }
        let floor = self.floor;
        let sweep = self.push * self.scale.sqrt();
        for ch in &self.chunks {
            if ch.geo.is_empty() {
                continue;
            }
            if k < ch.split {
                let age = ((k - ch.release) * SECS).max(0.0);
                let (p, land) = crate::finale::swept(ch.centre, ch.v0, sweep, floor + ch.size * 0.3, age);
                let r = Quat::from_axis_angle(ch.axis, ch.rate * age.min(land));
                for t in &ch.geo {
                    push_tri(p + r * t[0], p + r * t[1], p + r * t[2], 1.0);
                }
                continue;
            }
            let age_s = (ch.split - ch.release) * SECS;
            let (p0, land_s) = crate::finale::swept(ch.centre, ch.v0, sweep, floor + ch.size * 0.3, age_s);
            let v0 = crate::finale::fly(ch.centre, ch.v0, floor + ch.size * 0.3, age_s).1 + sweep * (1.6 * age_s);
            let r0 = Quat::from_axis_angle(ch.axis, ch.rate * age_s.min(land_s));
            let age = (k - ch.split) * SECS;
            for fr in &ch.frags {
                if k >= fr.dissolve.1 {
                    continue;
                }
                let (p, land) = crate::finale::swept(p0 + r0 * fr.offset, v0 + fr.dv, sweep, floor + fr.size * 0.25, age);
                let s = (1.0 - smooth(fr.dissolve.0, fr.dissolve.1, k)).max(0.001);
                let r = Quat::from_axis_angle(fr.axis, fr.rate * age.min(land)) * r0;
                for t in &fr.geo {
                    push_tri(p + r * (t[0] * s), p + r * (t[1] * s), p + r * (t[2] * s), 1.0);
                }
            }
        }
        for a in &self.ash {
            if k < a.born || k >= a.end {
                continue;
            }
            let t = (k - a.born) * SECS;
            let at = a.p0 + a.v0 * t + (sweep * 1.6 + Vec3::Z * 0.15) * (0.5 * t * t);
            let r = Quat::from_axis_angle(a.axis, 4.0 * t);
            let s = (a.size * (1.0 - smooth(a.born, a.end, k))).max(0.0001);
            let v = GRIT.map(|g| at + r * (g * s));
            let ch = if a.ember { 2.0 } else { 1.0 };
            for (i, j, l) in [(0, 1, 2), (0, 3, 1), (0, 2, 3), (1, 3, 2)] {
                push_tri(v[i], v[j], v[l], ch);
            }
        }
    }
}

/// The body whole into a soup (channel 1), centimetres.
pub fn body_soup(tris: &[Tri], out: &mut Vec<[f32; 4]>) {
    for t in tris {
        for p in t.p {
            out.push([p.x * 100.0, p.y * 100.0, p.z * 100.0, 1.0]);
        }
    }
}

/// What the finale asks of one body this frame (finale.rs `FinaleTarget`, the host's).
#[derive(Clone, Copy, Debug)]
pub struct Ask {
    pub id: u32,
    pub silhouette: bool,
    pub crumble: f32,
    /// Its feet (game cm), radius and half height (cm), the beam's direction at it (game axes).
    pub feet: Vec3,
    pub radius: f32,
    pub half_height: f32,
    pub push: Vec3,
}

/// The bounds of a body as drawn (game metres): the pose a break-apart was cut from.
fn bounds(tris: &[Tri]) -> (Vec3, Vec3) {
    tris.iter().flat_map(|t| t.p).fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(p), b.max(p)))
}

/// Two poses are the same for the break-apart when their bounds are within this (m).
const SAME_POSE_M: f32 = 0.03;

fn same_pose(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    (a.0 - b.0).abs().max_element() < SAME_POSE_M && (a.1 - b.1).abs().max_element() < SAME_POSE_M
}

/// The host's driver of the bodies drawn by us: their break-apart kept from their first crumble step.
///
/// `threaded` (the game): the break-apart is CUT AHEAD on a thread of its own while the body is a silhouette not yet
/// crumbling (Zettaflare holds it frozen in its hurt pose: ZETTA), so the first crumble step finds it ready (no hitch, no
/// step without cracks); a cut whose pose no longer matches the body (its bounds moved by more than 3 cm) is cut again. At the
/// first crumble step the cut that matches is kept for good; while none is ready the whole silhouette stays.
#[derive(Default)]
pub struct Driver {
    /// the cuts ready: (cut, the bounds it was cut from, kept for good)
    crumbles: Vec<(Crumble, (Vec3, Vec3), bool)>,
    pub threaded: bool,
    #[allow(clippy::type_complexity)]
    cutting: Vec<(u32, (Vec3, Vec3), std::thread::JoinHandle<(Crumble, Duration)>)>,
    /// (for the log) the last cut kept: whose, its triangles, how long it took
    pub last_cut: Option<(u32, usize, Duration)>,
}

impl Driver {
    fn start(&mut self, ask: &Ask, tris: &[Tri]) {
        let key = bounds(tris);
        let tris = tris.to_vec();
        let ask = *ask;
        let cut = move || {
            let started = Instant::now();
            let floor = ask.feet.z / 100.0;
            let top = tris.iter().flat_map(|t| t.p).map(|p| p.z).fold(floor, f32::max);
            let height = (top - floor).max(ask.half_height * 2.0 / 100.0 * 0.25);
            let cut = within_budget(tris, height);
            let c = Crumble::new(ask.id, &cut, floor, ask.feet / 100.0, height, ask.radius / 100.0, ask.push);
            (c, started.elapsed())
        };
        if self.threaded {
            if let Ok(h) = std::thread::Builder::new().name("kh-zdraw-cut".into()).spawn(cut) {
                self.cutting.push((ask.id, key, h));
            }
        } else {
            let (c, took) = cut();
            self.last_cut = Some((ask.id, c.triangles, took));
            self.crumbles.push((c, key, false));
        }
    }

    /// The finished cuts of `id` taken in.
    fn collect(&mut self, id: u32) {
        while let Some(k) = self.cutting.iter().position(|c| c.0 == id && c.2.is_finished()) {
            let (_, key, h) = self.cutting.remove(k);
            if let Ok((c, took)) = h.join() {
                self.crumbles.retain(|x| x.0.id != id || x.2);
                if !self.crumbles.iter().any(|x| x.0.id == id) {
                    self.last_cut = Some((id, c.triangles, took));
                    self.crumbles.push((c, key, false));
                }
            }
        }
    }

    /// One body this frame: `body` its triangles as drawn now (None: not to be had this frame). Its soup (None: not ours).
    pub fn step(&mut self, ask: &Ask, body: Option<&[Tri]>) -> Option<Vec<[f32; 4]>> {
        if !ask.silhouette {
            self.crumbles.retain(|c| c.0.id != ask.id);
            self.cutting.retain(|c| c.0 != ask.id);
            return None;
        }
        let mut out = Vec::new();
        self.collect(ask.id);
        let now = body.map(bounds);
        if ask.crumble <= 0.0 {
            let tris = body?;
            // (a new finale on it: nothing kept from before)
            self.crumbles.retain(|c| c.0.id != ask.id || !c.2);
            if self.threaded {
                let fits = self.crumbles.iter().any(|c| c.0.id == ask.id && now.is_some_and(|n| same_pose(c.1, n)));
                if !fits {
                    self.crumbles.retain(|c| c.0.id != ask.id);
                    if !self.cutting.iter().any(|c| c.0 == ask.id) {
                        self.start(ask, tris);
                    }
                }
            }
            body_soup(tris, &mut out);
            return Some(out);
        }
        if !self.crumbles.iter().any(|c| c.0.id == ask.id && c.2) {
            // the first crumble steps: keep the cut that matches the pose now (or any, once the body cannot be read)
            if let Some(c) = self.crumbles.iter_mut().find(|c| c.0.id == ask.id && now.is_none_or(|n| same_pose(c.1, n))) {
                c.2 = true;
            } else if let Some(tris) = body {
                self.crumbles.retain(|c| c.0.id != ask.id);
                if !self.cutting.iter().any(|c| c.0 == ask.id) {
                    self.start(ask, tris);
                    self.collect(ask.id);
                    if let Some(c) = self.crumbles.iter_mut().find(|c| c.0.id == ask.id) {
                        c.2 = true;
                    }
                }
            }
        }
        match self.crumbles.iter().find(|c| c.0.id == ask.id && c.2) {
            Some(c) => c.0.soup(ask.crumble, &mut out),
            // (being cut: the whole silhouette meanwhile)
            None => body_soup(body?, &mut out),
        }
        Some(out)
    }

    /// Has body `id` its break-apart kept (its pose is no longer needed)?
    pub fn has(&self, id: u32) -> bool {
        self.crumbles.iter().any(|c| c.0.id == id && c.2)
    }

    /// Forgets the bodies not asked for this frame.
    pub fn keep_only(&mut self, ids: &[u32]) {
        self.crumbles.retain(|c| ids.contains(&c.0.id));
        // (a cut no longer wanted finishes on its own thread and is dropped)
        self.cutting.retain(|c| ids.contains(&c.0));
    }
}

// ------------------------------------------------------------------------------------------------ the hand-over

struct Soups {
    at: Instant,
    /// the silhouettes' masks (id, soup)
    list: Vec<(u32, Vec<[f32; 4]>)>,
    /// the bodies in the light (id, immerse, soup): the test level's light grey over the game's draw of them
    bodies: Vec<(u32, f32, Vec<[f32; 4]>)>,
}

static SOUPS: Mutex<Option<Soups>> = Mutex::new(None);
/// A soup older than this is gone (the host sends it every frame).
const STALE: Duration = Duration::from_millis(500);
/// (for the log) frames drawn from our own models: masks, bodies in the light
static FRAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static FRAMES_LIGHT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The host, every frame: the silhouettes drawn by us (id, soup). Empty: none. (No bodies in the light.)
pub fn set(list: Vec<(u32, Vec<[f32; 4]>)>) {
    set_frame(list, Vec::new());
}

/// The host, every frame: the silhouettes' masks and the bodies in the light (id, immerse, soup) drawn by us.
pub fn set_frame(list: Vec<(u32, Vec<[f32; 4]>)>, bodies: Vec<(u32, f32, Vec<[f32; 4]>)>) {
    if let Ok(mut s) = SOUPS.lock() {
        *s = if list.is_empty() && bodies.is_empty() { None } else { Some(Soups { at: Instant::now(), list, bodies }) };
    }
}

/// Is body `id`'s silhouette drawn by us now (finale.rs leaves its screen route out; kh-er zfinale.rs hides the body from its
/// first crumble step, as for a snapshot break-apart)?
pub fn owns(id: u32) -> bool {
    // (our passes could not be made: the screen route keeps every body)
    !GPU_FAILED.load(std::sync::atomic::Ordering::Relaxed) && SOUPS.lock().ok().is_some_and(|s| s.as_ref().is_some_and(|s| s.at.elapsed() < STALE && s.list.iter().any(|x| x.0 == id)))
}

/// Is body `id` in the light drawn by us now (finale.rs leaves the screen route's white out for it)?
pub fn lights(id: u32) -> bool {
    !GPU_FAILED.load(std::sync::atomic::Ordering::Relaxed) && SOUPS.lock().ok().is_some_and(|s| s.as_ref().is_some_and(|s| s.at.elapsed() < STALE && s.bodies.iter().any(|x| x.0 == id)))
}

pub fn status() -> String {
    format!("zdraw: {} frame(s) of a Zettaflare silhouette and {} of a body in the light drawn from its own model", FRAMES.load(std::sync::atomic::Ordering::Relaxed), FRAMES_LIGHT.load(std::sync::atomic::Ordering::Relaxed))
}

/// The rim band (pixels of the target, `w` wide) of body `id` as the camera sees it now: the test level's 3.5 % of the
/// silhouette on the screen, taken across its thinner side (the test level's reference body lies along the beam: its height
/// on the screen is its thickness), clamped to 1.5 .. 18 px of a 960-wide mask. None: not ours / not on the screen.
pub fn band(id: u32, clip: &Mat4, w: u32, h: u32) -> Option<f32> {
    let guard = SOUPS.lock().ok()?;
    let s = guard.as_ref().filter(|s| s.at.elapsed() < STALE)?;
    let soup = &s.list.iter().find(|x| x.0 == id)?.1;
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    for v in soup.iter().filter(|v| v[3] < 1.5).step_by(3) {
        let c = *clip * glam::Vec4::new(v[0], v[1], v[2], 1.0);
        if c.w <= 1e-3 {
            continue;
        }
        let p = glam::Vec2::new((c.x / c.w * 0.5 + 0.5) * w as f32, (0.5 - c.y / c.w * 0.5) * h as f32);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    if hi.x < lo.x {
        return None;
    }
    let k = w.max(1) as f32 / 960.0;
    let thin = (hi - lo).min_element().max(0.0) / k;
    Some((thin * 0.035).clamp(1.5, 18.0) * k)
}

/// The test level's light grey of a body in the light (kh-app zetta_target.rs `IMMERSED`, display-linear) and its glow.
pub const IMMERSED: Vec3 = Vec3::new(0.70, 0.70, 0.74);
pub const IMMERSED_GLOW: f32 = 0.12;
/// How much its soft key shades it (the reference: one flat light grey).
pub const IMMERSED_SHADE: f32 = 0.06;
/// A body in front of the scene depth by less than this (cm) is not behind it (its own draw by the game is at its place).
pub const DEPTH_SLACK_CM: f32 = 30.0;

// ------------------------------------------------------------------------------------------------ the GPU part

use std::ffi::c_void;
use windows::core::{Interface, PCSTR};
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

const SLOTS: usize = 3;

struct Ring {
    buffer: ID3D12Resource,
    mapped: *mut u8,
    bytes: usize,
}

struct Gpu {
    device: usize,
    /// our own root signature (b0, b2, a table of 7 views t0.., the input assembler's layout allowed: the finale's has no
    /// input layout)
    root: ID3D12RootSignature,
    /// the mask's two pipelines (the body writing the small depth, its light tested against it) per depth direction
    mask: Vec<(ID3D12PipelineState, ID3D12PipelineState)>,
    /// the light grey over the scene, per target format
    light: Vec<(i32, ID3D12PipelineState)>,
    light_shaders: Option<(Vec<u8>, Vec<u8>)>,
    rings: Vec<Option<Ring>>,
    body_rings: Vec<Option<Ring>>,
    /// (the test level's mask size) the small mask (960 wide, the target's aspect), its target view, its shader view (in a
    /// heap of our own, 7 entries: our table's size) and the pass that lays it over the finale's mask
    small: Option<(ID3D12Resource, u32, u32)>,
    small_depth: Option<ID3D12Resource>,
    small_rtv: ID3D12DescriptorHeap,
    small_dsv: ID3D12DescriptorHeap,
    small_srv: ID3D12DescriptorHeap,
    up: ID3D12PipelineState,
}

// SAFETY: used under its lock on the submission thread only; `mapped` points into the ring's own buffer.
unsafe impl Send for Gpu {}

static GPU: Mutex<Option<Gpu>> = Mutex::new(None);
static GPU_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn release() {
    if let Ok(mut g) = GPU.lock() {
        *g = None;
    }
}

fn shader(name: &str) -> Option<Vec<u8>> {
    if let Some(bytes) = std::env::var_os("KH_FINALE_SHADERS").and_then(|d| std::fs::read(std::path::Path::new(&d).join(name)).ok()) {
        return Some(bytes);
    }
    let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()))?;
    std::fs::read(pack.join("materials").join("er").join(name)).ok()
}

/// A pipeline of ours: the soup's layout, no depth, one target of `format` blended by `blend`.
unsafe fn pipeline(device: &ID3D12Device, root: &ID3D12RootSignature, vs: &[u8], ps: &[u8], format: DXGI_FORMAT, blend: D3D12_RENDER_TARGET_BLEND_DESC) -> windows::core::Result<ID3D12PipelineState> {
    pipeline_in(device, root, vs, ps, format, blend, true, None)
}

/// As `pipeline`; `soup` false: no vertex input (a full-screen triangle).
/// `depth`: the small mask's depth (D32) tested with that function, written or not.
#[allow(clippy::too_many_arguments)]
unsafe fn pipeline_in(device: &ID3D12Device, root: &ID3D12RootSignature, vs: &[u8], ps: &[u8], format: DXGI_FORMAT, blend: D3D12_RENDER_TARGET_BLEND_DESC, soup: bool, depth: Option<(D3D12_COMPARISON_FUNC, bool)>) -> windows::core::Result<ID3D12PipelineState> {
    let element = [D3D12_INPUT_ELEMENT_DESC { SemanticName: PCSTR(b"LOC\0".as_ptr()), SemanticIndex: 0, Format: DXGI_FORMAT_R32G32B32A32_FLOAT, InputSlot: 0, AlignedByteOffset: 0, InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, InstanceDataStepRate: 0 }];
    let mut blend_desc = D3D12_BLEND_DESC::default();
    blend_desc.RenderTarget[0] = blend;
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = format;
    let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vs.as_ptr() as *const c_void, BytecodeLength: vs.len() },
        PS: D3D12_SHADER_BYTECODE { pShaderBytecode: ps.as_ptr() as *const c_void, BytecodeLength: ps.len() },
        BlendState: blend_desc,
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: true.into(), ..Default::default() },
        DepthStencilState: match depth {
            Some((func, write)) => D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: if write { D3D12_DEPTH_WRITE_MASK_ALL } else { D3D12_DEPTH_WRITE_MASK_ZERO }, DepthFunc: func, StencilEnable: false.into(), ..Default::default() },
            None => D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
        },
        InputLayout: if soup { D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: element.as_ptr(), NumElements: 1 } } else { D3D12_INPUT_LAYOUT_DESC::default() },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        RTVFormats: formats,
        DSVFormat: if depth.is_some() { DXGI_FORMAT_D32_FLOAT } else { DXGI_FORMAT_UNKNOWN },
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };
    device.CreateGraphicsPipelineState(&pso)
}

unsafe fn make(device: &ID3D12Device) -> Result<Gpu, String> {
    let (Some(vs), Some(ps)) = (shader("zdraw.vs.dxil"), shader("zdraw.ps.dxil")) else { return Err("materials/er/zdraw.vs.dxil / zdraw.ps.dxil missing".to_string()) };
    let cbv = |r: u32| D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: r, RegisterSpace: 0 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL };
    let range = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV, NumDescriptors: 7, BaseShaderRegister: 0, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
    let params = [cbv(0), cbv(2), D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE, Anonymous: D3D12_ROOT_PARAMETER_0 { DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE { NumDescriptorRanges: 1, pDescriptorRanges: range.as_ptr() } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL }, cbv(3)];
    let desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: params.len() as u32, pParameters: params.as_ptr(), NumStaticSamplers: 0, pStaticSamplers: std::ptr::null(), Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT };
    let mut blob: Option<windows::Win32::Graphics::Direct3D::ID3DBlob> = None;
    D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None).map_err(|e| format!("root signature: {e}"))?;
    let blob = blob.ok_or("no root signature")?;
    let root: ID3D12RootSignature = device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())).map_err(|e| format!("root signature: {e}"))?;
    let max = D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: true.into(),
        LogicOpEnable: false.into(),
        SrcBlend: D3D12_BLEND_ONE,
        DestBlend: D3D12_BLEND_ONE,
        BlendOp: D3D12_BLEND_OP_MAX,
        SrcBlendAlpha: D3D12_BLEND_ONE,
        DestBlendAlpha: D3D12_BLEND_ONE,
        BlendOpAlpha: D3D12_BLEND_OP_MAX,
        LogicOp: D3D12_LOGIC_OP_NOOP,
        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    // (per depth direction, [forward z, reverse z]: the body writing its depth, its light tested against it)
    let mut mask = Vec::new();
    for func in [D3D12_COMPARISON_FUNC_LESS_EQUAL, D3D12_COMPARISON_FUNC_GREATER_EQUAL] {
        let body = pipeline_in(device, &root, &vs, &ps, DXGI_FORMAT_R8G8B8A8_UNORM, max, true, Some((func, true))).map_err(|e| format!("pipeline: {e}"))?;
        let light = pipeline_in(device, &root, &vs, &ps, DXGI_FORMAT_R8G8B8A8_UNORM, max, true, Some((func, false))).map_err(|e| format!("pipeline: {e}"))?;
        mask.push((body, light));
    }
    let light_shaders = shader("zdraw_imm.vs.dxil").zip(shader("zdraw_imm.ps.dxil"));
    let (Some(up_vs), Some(up_ps)) = (shader("zdraw_up.vs.dxil"), shader("zdraw_up.ps.dxil")) else { return Err("materials/er/zdraw_up.vs.dxil / zdraw_up.ps.dxil missing".to_string()) };
    let up = pipeline_in(device, &root, &up_vs, &up_ps, DXGI_FORMAT_R8G8B8A8_UNORM, max, false, None).map_err(|e| format!("the mask's lay-over: {e}"))?;
    let small_dsv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| format!("views: {e}"))?;
    let small_rtv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| format!("views: {e}"))?;
    let small_srv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: 7, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 }).map_err(|e| format!("views: {e}"))?;
    let size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
    for k in 0..7 {
        let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R8G8B8A8_UNORM, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
        device.CreateShaderResourceView(None, Some(&desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: small_srv.GetCPUDescriptorHandleForHeapStart().ptr + k * size });
    }
    Ok(Gpu { device: device.as_raw() as usize, root, mask, light: Vec::new(), light_shaders, rings: (0..SLOTS).map(|_| None).collect(), body_rings: (0..SLOTS).map(|_| None).collect(), small: None, small_depth: None, small_rtv, small_dsv, small_srv, up })
}

unsafe fn ring(device: &ID3D12Device, bytes: usize) -> windows::core::Result<Ring> {
    let bytes = bytes.next_power_of_two().max(1 << 16);
    let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: bytes as u64, Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() };
    let mut out: Option<ID3D12Resource> = None;
    device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_UPLOAD, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_GENERIC_READ, None, &mut out)?;
    let buffer = out.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;
    let mut at: *mut c_void = std::ptr::null_mut();
    buffer.Map(0, None, Some(&mut at))?;
    Ok(Ring { buffer, mapped: at as *mut u8, bytes })
}

/// Our GPU objects (made the first time; None: they cannot be made, the screen route for every body from then on).
fn gpu<'a>(guard: &'a mut Option<Gpu>, device: &ID3D12Device) -> Option<&'a mut Gpu> {
    if guard.as_ref().is_some_and(|g| g.device != unsafe { device.as_raw() } as usize) {
        *guard = None;
    }
    if guard.is_none() {
        // SAFETY: objects made on the caller's device.
        match unsafe { make(device) } {
            Ok(g) => {
                crate::rlog!("render: (ZDRAW) the own-model passes are made: Zettaflare's target drawn from its own model (its mask; its light grey {})", if g.light_shaders.is_some() { "too" } else { "NOT: zdraw_imm.*.dxil missing, the screen route's white stays" });
                *guard = Some(g);
            }
            Err(e) => {
                crate::rlog!("render: (ZDRAW) the own-model passes cannot be made ({e}): the screen route stays");
                GPU_FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
                return None;
            }
        }
    }
    guard.as_mut()
}

/// (finale.rs `draw`) The soups of `ids` into the finale's mask (`mask_rtv`, `w` x `h`; max blending: r its body and
/// pieces, g its light), drawn as the test level draws its mask: into a mask 960 pixels wide (the target's aspect), laid over
/// with linear filtering. Returns the ids drawn. Draws with a root signature and a view heap of its own and binds `root` and
/// `heap` again after: the caller sets its root arguments (CBVs, table), pipeline, targets, viewport and scissor again.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn draw_mask(device: &ID3D12Device, list: &ID3D12GraphicsCommandList, root: &ID3D12RootSignature, heap: &ID3D12DescriptorHeap, slot: usize, view: &crate::finale::View, ids: &[u32], mask_rtv: D3D12_CPU_DESCRIPTOR_HANDLE, w: u32, h: u32) -> Vec<u32> {
    let clip = &view.clip;
    if ids.is_empty() || slot >= SLOTS || GPU_FAILED.load(std::sync::atomic::Ordering::Relaxed) || w == 0 || h == 0 {
        return Vec::new();
    }
    let soups: Vec<(u32, Vec<[f32; 4]>)> = {
        let Ok(guard) = SOUPS.lock() else { return Vec::new() };
        let Some(s) = guard.as_ref().filter(|s| s.at.elapsed() < STALE) else { return Vec::new() };
        s.list.iter().filter(|x| ids.contains(&x.0) && !x.1.is_empty()).cloned().collect()
    };
    if soups.is_empty() {
        return Vec::new();
    }
    let Ok(mut guard) = GPU.lock() else { return Vec::new() };
    let Some(g) = gpu(&mut guard, device) else { return Vec::new() };
    // the small mask: the test level's 960 wide at the target's aspect (a target under that: its own size)
    let (sw, sh) = small_size(w, h);
    if g.small.as_ref().map(|s| (s.1, s.2)) != Some((sw, sh)) {
        let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: sw as u64, Height: sh, DepthOrArraySize: 1, MipLevels: 1, Format: DXGI_FORMAT_R8G8B8A8_UNORM, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, ..Default::default() };
        let mut out: Option<ID3D12Resource> = None;
        if device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, None, &mut out).is_err() {
            return Vec::new();
        }
        let Some(t) = out else { return Vec::new() };
        device.CreateRenderTargetView(&t, None, g.small_rtv.GetCPUDescriptorHandleForHeapStart());
        device.CreateShaderResourceView(&t, None, g.small_srv.GetCPUDescriptorHandleForHeapStart());
        g.small = Some((t, sw, sh));
        // its depth: the body and its pieces, then their light tested against them
        let ddesc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: sw as u64, Height: sh, DepthOrArraySize: 1, MipLevels: 1, Format: DXGI_FORMAT_D32_FLOAT, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL, ..Default::default() };
        let mut d: Option<ID3D12Resource> = None;
        if device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &ddesc, D3D12_RESOURCE_STATE_DEPTH_WRITE, None, &mut d).is_err() {
            g.small = None;
            return Vec::new();
        }
        let Some(d) = d else { return Vec::new() };
        device.CreateDepthStencilView(&d, None, g.small_dsv.GetCPUDescriptorHandleForHeapStart());
        g.small_depth = Some(d);
    }
    let Some((small, ..)) = g.small.clone() else { return Vec::new() };
    // the projection's depth direction (reverse z: nearer is greater), from two points ahead of the camera
    let ndc = |m: f32| {
        let c = *clip * (view.camera + view.forward * m).extend(1.0);
        c.z / c.w.max(1e-6)
    };
    let reverse = ndc(100.0) > ndc(10_000.0);
    // the body and its pieces first, then their light (cracks, embers)
    let mut ordered: Vec<[f32; 4]> = Vec::with_capacity(soups.iter().map(|s| s.1.len()).sum());
    for (_, s) in &soups {
        ordered.extend(s.chunks_exact(3).filter(|t| t[0][3] < 1.5).flatten());
    }
    let body_verts = ordered.len();
    for (_, s) in &soups {
        ordered.extend(s.chunks_exact(3).filter(|t| t[0][3] >= 1.5).flatten());
    }
    let verts = ordered.len();
    let need = 512 + verts * 16;
    if g.rings[slot].as_ref().is_none_or(|r| r.bytes < need) {
        // (this slot's last frame is done: the slot is being used again)
        g.rings[slot] = ring(device, need).ok();
    }
    let Some(r) = g.rings[slot].as_ref() else { return Vec::new() };
    let cols = clip.to_cols_array();
    std::ptr::copy_nonoverlapping(cols.as_ptr() as *const u8, r.mapped, 64);
    // (the light nudged 1.5 % of its distance toward the camera: on the surface seen and what lies just under it (a crack runs
    // through a strap and the cloth under it), never on a layer centimetres under another)
    let bias = [if reverse { CRACK_NUDGE } else { -CRACK_NUDGE * 0.25 }, 0.0, 0.0, 0.0];
    std::ptr::copy_nonoverlapping(bias.as_ptr() as *const u8, r.mapped.add(64), 16);
    let up = [w as f32, h as f32, sw as f32, sh as f32];
    std::ptr::copy_nonoverlapping(up.as_ptr() as *const u8, r.mapped.add(256), 16);
    std::ptr::copy_nonoverlapping(ordered.as_ptr() as *const u8, r.mapped.add(512), verts * 16);
    let base = r.buffer.GetGPUVirtualAddress();
    let small_rtv = g.small_rtv.GetCPUDescriptorHandleForHeapStart();
    // 1. the soups into the small mask
    list.ResourceBarrier(&[transition(&small, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
    list.ClearRenderTargetView(small_rtv, &[0.0, 0.0, 0.0, 0.0], None);
    let small_dsv = g.small_dsv.GetCPUDescriptorHandleForHeapStart();
    list.ClearDepthStencilView(small_dsv, D3D12_CLEAR_FLAG_DEPTH, if reverse { 0.0 } else { 1.0 }, 0, None);
    list.OMSetRenderTargets(1, Some(&small_rtv), false, Some(&small_dsv));
    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: sw as f32, Height: sh as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
    list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: sw as i32, bottom: sh as i32 }]);
    list.SetGraphicsRootSignature(&g.root);
    let (body_state, light_state) = g.mask[usize::from(reverse)].clone();
    list.SetPipelineState(&body_state);
    list.SetGraphicsRootConstantBufferView(0, base);
    list.SetGraphicsRootConstantBufferView(1, base);
    list.SetGraphicsRootConstantBufferView(3, base + 256);
    list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    list.IASetVertexBuffers(0, Some(&[D3D12_VERTEX_BUFFER_VIEW { BufferLocation: base + 512, SizeInBytes: (verts * 16) as u32, StrideInBytes: 16 }]));
    if body_verts > 0 {
        list.DrawInstanced(body_verts as u32, 1, 0, 0);
    }
    if verts > body_verts {
        list.SetPipelineState(&light_state);
        list.DrawInstanced((verts - body_verts) as u32, 1, body_verts as u32, 0);
    }
    list.ResourceBarrier(&[transition(&small, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
    // 2. laid over the finale's mask, linearly filtered
    list.OMSetRenderTargets(1, Some(&mask_rtv), false, None);
    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: w as f32, Height: h as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
    list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 }]);
    list.SetDescriptorHeaps(&[Some(g.small_srv.clone())]);
    list.SetGraphicsRootDescriptorTable(2, g.small_srv.GetGPUDescriptorHandleForHeapStart());
    list.SetPipelineState(&g.up);
    list.DrawInstanced(3, 1, 0, 0);
    FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    // (the caller's heap and root signature again: its root arguments are to be set again by it)
    list.SetDescriptorHeaps(&[Some(heap.clone())]);
    list.SetGraphicsRootSignature(root);
    soups.iter().map(|s| s.0).collect()
}

/// How far toward the camera a crack is tested against its body's depth (a share of its distance).
pub const CRACK_NUDGE: f32 = 0.015;

/// The test level's mask size for a target of `w` x `h`: 960 wide at its aspect (a narrower target: its own size).
pub fn small_size(w: u32, h: u32) -> (u32, u32) {
    if w <= 960 {
        return (w.max(1), h.max(1));
    }
    (960, ((h as f64 * 960.0 / w as f64).round() as u32).max(1))
}

fn transition(resource: &ID3D12Resource, before: D3D12_RESOURCE_STATES, after: D3D12_RESOURCE_STATES) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            // SAFETY: a borrowed copy of the interface pointer: never released through this struct.
            Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER { pResource: unsafe { std::mem::transmute_copy(resource) }, Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, StateBefore: before, StateAfter: after }),
        },
    }
}

/// (finale.rs `draw`, the scene target bound, its viewport set, the scene depth readable as t0 of `table` in the bound heap)
/// The bodies of `ids` in the light: their own model in the test level's light grey, faded in by `immerse`, behind whatever
/// the scene depth holds nearer. `scale`: the light's scale to the target (vfx.rs `effect_scale`). Returns the ids drawn;
/// binds `root` again after (the caller sets its root arguments again).
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn draw_light(device: &ID3D12Device, list: &ID3D12GraphicsCommandList, root: &ID3D12RootSignature, slot: usize, format: DXGI_FORMAT, view: &crate::finale::View, scale: f32, table: D3D12_GPU_DESCRIPTOR_HANDLE, ids: &[u32]) -> Vec<u32> {
    if ids.is_empty() || slot >= SLOTS || GPU_FAILED.load(std::sync::atomic::Ordering::Relaxed) {
        return Vec::new();
    }
    let bodies: Vec<(u32, f32, Vec<[f32; 4]>)> = {
        let Ok(guard) = SOUPS.lock() else { return Vec::new() };
        let Some(s) = guard.as_ref().filter(|s| s.at.elapsed() < STALE) else { return Vec::new() };
        s.bodies.iter().filter(|x| ids.contains(&x.0) && !x.2.is_empty() && x.1 > 0.0).cloned().collect()
    };
    if bodies.is_empty() {
        return Vec::new();
    }
    let Ok(mut guard) = GPU.lock() else { return Vec::new() };
    let Some(g) = gpu(&mut guard, device) else { return Vec::new() };
    let Some((vs, ps)) = g.light_shaders.clone() else { return Vec::new() };
    let state = match g.light.iter().find(|(f, _)| *f == format.0) {
        Some((_, s)) => s.clone(),
        None => {
            let blend = D3D12_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                LogicOpEnable: false.into(),
                SrcBlend: D3D12_BLEND_SRC_ALPHA,
                DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D12_BLEND_OP_ADD,
                SrcBlendAlpha: D3D12_BLEND_ZERO,
                DestBlendAlpha: D3D12_BLEND_ONE,
                BlendOpAlpha: D3D12_BLEND_OP_ADD,
                LogicOp: D3D12_LOGIC_OP_NOOP,
                RenderTargetWriteMask: (D3D12_COLOR_WRITE_ENABLE_RED.0 | D3D12_COLOR_WRITE_ENABLE_GREEN.0 | D3D12_COLOR_WRITE_ENABLE_BLUE.0) as u8,
            };
            match pipeline(device, &g.root, &vs, &ps, format, blend) {
                Ok(s) => {
                    g.light.push((format.0, s.clone()));
                    s
                }
                Err(e) => {
                    crate::rlog!("render: (ZDRAW) the light grey's pipeline cannot be made for format {} ({e}): the screen route's white", format.0);
                    g.light_shaders = None;
                    return Vec::new();
                }
            }
        }
    };
    let verts: usize = bodies.iter().map(|s| s.2.len()).sum();
    let blocks = bodies.len() * 256;
    let need = blocks + verts * 16;
    if g.body_rings[slot].as_ref().is_none_or(|r| r.bytes < need) {
        g.body_rings[slot] = ring(device, need).ok();
    }
    let Some(r) = g.body_rings[slot].as_ref() else { return Vec::new() };
    let base = r.buffer.GetGPUVirtualAddress();
    list.SetGraphicsRootSignature(&g.root);
    list.SetPipelineState(&state);
    list.SetGraphicsRootDescriptorTable(2, table);
    list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    let mut at = blocks;
    for (k, (_, immerse, soup)) in bodies.iter().enumerate() {
        let mut block: Vec<f32> = view.clip.to_cols_array().to_vec();
        block.extend_from_slice(&view.camera.extend(view.depth_a).to_array());
        block.extend_from_slice(&view.forward.extend(view.depth_b).to_array());
        block.extend_from_slice(&[view.width as f32, view.height as f32, 1.0 / view.width.max(1) as f32, 1.0 / view.height.max(1) as f32]);
        block.extend_from_slice(&[immerse.clamp(0.0, 1.0), scale.max(1e-4), DEPTH_SLACK_CM, 1.0]);
        block.extend_from_slice(&IMMERSED.extend(IMMERSED_GLOW).to_array());
        block.extend_from_slice(&(view.up * 0.8 - view.forward * 0.5 + view.right * 0.3).extend(IMMERSED_SHADE).to_array());
        std::ptr::copy_nonoverlapping(block.as_ptr() as *const u8, r.mapped.add(k * 256), block.len() * 4);
        std::ptr::copy_nonoverlapping(soup.as_ptr() as *const u8, r.mapped.add(at), soup.len() * 16);
        list.SetGraphicsRootConstantBufferView(0, base + (k * 256) as u64);
        list.SetGraphicsRootConstantBufferView(1, base + (k * 256) as u64);
        list.IASetVertexBuffers(0, Some(&[D3D12_VERTEX_BUFFER_VIEW { BufferLocation: base + at as u64, SizeInBytes: (soup.len() * 16) as u32, StrideInBytes: 16 }]));
        list.DrawInstanced(soup.len() as u32, 1, 0, 0);
        at += soup.len() * 16;
    }
    FRAMES_LIGHT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    list.SetGraphicsRootSignature(root);
    bodies.iter().map(|b| b.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube_body(h: f32) -> Vec<Tri> {
        // a 0.5 x 0.5 x h box standing on z = 0, its faces split into small triangles
        let mut out = Vec::new();
        let n = 12;
        let quad = |o: Vec3, u: Vec3, v: Vec3, out: &mut Vec<Tri>| {
            for i in 0..n {
                for j in 0..n {
                    let (a, b) = (i as f32 / n as f32, j as f32 / n as f32);
                    let s = 1.0 / n as f32;
                    let p = |x: f32, y: f32| o + u * x + v * y;
                    out.extend(Tri::new(p(a, b), p(a + s, b), p(a + s, b + s)));
                    out.extend(Tri::new(p(a, b), p(a + s, b + s), p(a, b + s)));
                }
            }
        };
        let (x, y, z) = (Vec3::X * 0.5, Vec3::Y * 0.5, Vec3::Z * h);
        let o = Vec3::new(-0.25, -0.25, 0.0);
        quad(o, y, x, &mut out);
        quad(o + z, x, y, &mut out);
        quad(o, x, z, &mut out);
        quad(o + y, z, x, &mut out);
        quad(o, z, y, &mut out);
        quad(o + x, y, z, &mut out);
        out
    }

    #[test]
    fn zdraw_a_big_model_is_reduced_within_its_budget() {
        // a 200 x 200 grid of quads (80 000 triangles), one bone
        let n = 200usize;
        let mut m = Model { bone_names: vec!["b".into()], inv_bind: vec![Mat4::IDENTITY], ..Default::default() };
        for j in 0..=n {
            for i in 0..=n {
                m.positions.push(Vec3::new(i as f32 / n as f32, j as f32 / n as f32, 0.0));
                m.joints.push([0; 4]);
                m.weights.push([1.0, 0.0, 0.0, 0.0]);
            }
        }
        for j in 0..n {
            for i in 0..n {
                let a = (j * (n + 1) + i) as u32;
                let (b, c, d) = (a + 1, a + n as u32 + 1, a + n as u32 + 2);
                m.indices.extend([a, b, d, a, d, c]);
            }
        }
        m.meshes.push(MeshPart { first: 0, count: m.indices.len(), group: Some(3), material: "#03#x".into() });
        let r = m.reduced(20_000);
        assert!(r.indices.len() / 3 <= 20_000 && r.indices.len() / 3 > 5_000, "{} triangles", r.indices.len() / 3);
        assert_eq!(r.meshes[0].group, Some(3));
        let tris = skin(&r, &[Mat4::IDENTITY], &|_| true, &|p| p);
        let area: f32 = tris.iter().map(|t| (t.p[1] - t.p[0]).cross(t.p[2] - t.p[0]).length() * 0.5).sum();
        assert!((area - 1.0).abs() < 0.05, "the shape kept (area {area})");
        assert_eq!(m.reduced(100_000).indices.len(), m.indices.len(), "a model within its budget is kept whole");
    }

    #[test]
    fn zdraw_unreadable_bones_are_placed_from_their_parents() {
        // a chain root -> a (1 m up) -> b (1 m up); the game's matrices: root moved 5 m along x, a usable, b unreadable
        let binds = [Mat4::IDENTITY, Mat4::from_translation(Vec3::Y), Mat4::from_translation(Vec3::Y * 2.0)];
        let m = Model { parent: vec![-1, 0, 1], bind: binds.to_vec(), inv_bind: binds.iter().map(|b| b.inverse()).collect(), ..Default::default() };
        let mut world = [Mat4::from_translation(Vec3::X * 5.0), Mat4::from_translation(Vec3::X * 5.0 + Vec3::Y), Mat4::from_cols_array(&[f32::NAN; 16])];
        assert_eq!(mend_pose(&m, &mut world, &[true, true, false]), Some(1));
        assert!((world[2].w_axis.truncate() - Vec3::new(5.0, 2.0, 0.0)).length() < 1e-5);
        // the root unreadable: from the first usable bone, rigidly
        let mut world = [Mat4::from_cols_array(&[f32::NAN; 16]), Mat4::from_translation(Vec3::X * 5.0 + Vec3::Y), Mat4::from_translation(Vec3::X * 5.0 + Vec3::Y * 2.0)];
        assert_eq!(mend_pose(&m, &mut world, &[false, true, true]), Some(1));
        assert!((world[0].w_axis.truncate() - Vec3::new(5.0, 0.0, 0.0)).length() < 1e-5);
        assert_eq!(mend_pose(&m, &mut world, &[false, false, false]), None);
    }

    #[test]
    fn zdraw_disp_groups_of_material_names() {
        assert_eq!(disp_group("#10#_mant"), Some(10));
        assert_eq!(disp_group("＃22＃"), Some(22));
        assert_eq!(disp_group("#00#_Sword"), Some(0));
        assert_eq!(disp_group("Material #54"), None);
        assert_eq!(disp_group("c2120_Arts_Petal"), None);
        assert_eq!(disp_group("#11#c2120_wing_color"), Some(11));
    }

    #[test]
    fn zdraw_skin_follows_the_bones() {
        // two bones, a triangle on the second; the second moved 1 m up in the world: the triangle goes with it
        let model = Model {
            name: "t".into(),
            bone_names: vec!["a".into(), "b".into()],
            inv_bind: vec![Mat4::IDENTITY, Mat4::IDENTITY],
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Z],
            joints: vec![[1, 0, 0, 0]; 3],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
            meshes: vec![MeshPart { first: 0, count: 3, group: None, material: String::new() }, MeshPart { first: 0, count: 3, group: Some(5), material: String::new() }],
            indices: vec![0, 1, 2],
            ..Default::default()
        };
        let world = [Mat4::IDENTITY, Mat4::from_translation(Vec3::Y)];
        let tris = skin(&model, &world, &|_| false, &|p| p);
        assert_eq!(tris.len(), 1, "the group-5 mesh is not shown");
        assert!((tris[0].p[0] - Vec3::Y).length() < 1e-6);
        assert_eq!(skin(&model, &world, &|g| g == 5, &|p| p).len(), 2);
        // a bone scaled to nothing hides what is on it
        let gone = [Mat4::IDENTITY, Mat4::from_scale(Vec3::ZERO)];
        assert!(skin(&model, &gone, &|_| false, &|p| p).is_empty());
    }

    #[test]
    fn zdraw_crumble_tiles_the_body_cracks_first_and_is_gone_at_one() {
        let body = cube_body(1.8);
        let c = Crumble::new(7, &body, 0.0, Vec3::ZERO, 1.8, 0.3, Vec3::X);
        assert!(c.chunks.iter().filter(|c| !c.geo.is_empty()).count() >= PIECES / 2);
        // at 0: every triangle of the body is in place (the pieces tile it), no crack yet
        let mut s = Vec::new();
        c.soup(0.0, &mut s);
        let lit = s.iter().filter(|v| v[3] > 1.5).count();
        assert_eq!(lit, 0, "no light at 0");
        let front: Vec<[f32; 4]> = s.iter().copied().filter(|v| v[3] < 1.5).collect();
        assert!(front.len() >= body.len() * 3, "every triangle of the body is drawn at 0 ({} of {})", front.len() / 3, body.len());
        // cracks before any piece goes
        s.clear();
        c.soup(0.12, &mut s);
        assert!(s.iter().any(|v| v[3] > 1.5), "cracks by 0.12");
        // nothing at 1
        s.clear();
        c.soup(1.0, &mut s);
        assert!(s.is_empty(), "{} vertices left at 1", s.len());
        // the pieces go down the beam (+x), not just down
        s.clear();
        c.soup(0.8, &mut s);
        let mean_x = s.iter().filter(|v| v[3] < 1.5).map(|v| v[0]).sum::<f32>() / s.iter().filter(|v| v[3] < 1.5).count().max(1) as f32;
        assert!(mean_x > 50.0, "carried along the beam ({mean_x} cm)");
    }

    #[test]
    fn zdraw_threaded_driver_cuts_ahead_while_held_and_recuts_a_moved_pose() {
        let body = cube_body(1.8);
        let mut d = Driver::default();
        d.threaded = true;
        let ask = |crumble: f32| Ask { id: 5, silhouette: true, crumble, feet: Vec3::ZERO, radius: 30.0, half_height: 90.0, push: Vec3::X };
        // held as a silhouette: the whole body, the cut made meanwhile
        for _ in 0..200 {
            assert_eq!(d.step(&ask(0.0), Some(&body)).unwrap().len(), body.len() * 3);
            if d.crumbles.iter().any(|c| c.0.id == 5) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(d.crumbles.iter().any(|c| c.0.id == 5), "cut ahead while held");
        // its first crumble step finds it ready: kept, cracks at once
        let s = d.step(&ask(0.12), Some(&body)).unwrap();
        assert!(d.has(5) && s.iter().any(|v| v[3] > 1.5), "the cut kept at the first crumble step, its cracks drawn");
        // a pose that moved before the crumble: cut again (the old cut not kept)
        let mut d = Driver::default();
        d.threaded = true;
        for _ in 0..200 {
            d.step(&ask(0.0), Some(&body));
            if d.crumbles.iter().any(|c| c.0.id == 5) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let moved: Vec<Tri> = body.iter().map(|t| Tri::new(t.p[0] + Vec3::X, t.p[1] + Vec3::X, t.p[2] + Vec3::X).unwrap()).collect();
        let s = d.step(&ask(0.05), Some(&moved)).unwrap();
        assert!(!d.has(5) && s.len() == moved.len() * 3, "the moved body whole while its cut is made again");
    }

    #[test]
    fn zdraw_driver_snapshot_once_and_the_band() {
        let body = cube_body(1.8);
        let mut d = Driver::default();
        let ask = |crumble: f32| Ask { id: 3, silhouette: true, crumble, feet: Vec3::ZERO, radius: 30.0, half_height: 90.0, push: Vec3::X };
        let whole = d.step(&ask(0.0), Some(&body)).unwrap();
        assert_eq!(whole.len(), body.len() * 3);
        let first = d.step(&ask(0.3), Some(&body)).unwrap();
        // the snapshot is kept: the same at the same value without a body this frame
        let again = d.step(&ask(0.3), None).unwrap();
        assert_eq!(first.len(), again.len());
        assert!(d.step(&Ask { silhouette: false, ..ask(0.3) }, Some(&body)).is_none(), "a survivor is not ours");
        d.keep_only(&[]);
        assert!(d.step(&ask(0.3), None).is_none(), "forgotten: no body, nothing");
        // the band: across the thinner side of the silhouette on the screen
        set(vec![(3, whole)]);
        let clip = Mat4::orthographic_rh(-100.0, 100.0, -100.0, 100.0, -1000.0, 1000.0) * Mat4::look_at_rh(Vec3::new(-500.0, 0.0, 90.0), Vec3::new(0.0, 0.0, 90.0), Vec3::Z);
        let b = band(3, &clip, 960, 960).unwrap();
        // 50 cm wide of 200 cm across 960 px = 240 px: 3.5 % = 8.4 px
        assert!((b - 8.4).abs() < 0.6, "band {b}");
        assert!(owns(3) && !owns(4));
        set(Vec::new());
    }
}
