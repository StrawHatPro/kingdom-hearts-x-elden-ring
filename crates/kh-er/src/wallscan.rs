//! (IMPL_ER18) WHAT STANDS IN SORA'S WAY: measuring aids for "he cannot walk through an opened door" and "he cannot
//! traverse a fog wall". Nothing here changes the game; every read of an owner goes through `probe` (cannot fault).
//!
//! - `owner_text`: the class and (for a map object) the MSB part name of what owns a body a cast touched.
//! - `forward_scan` (`w.fscan = n`): one sphere cast from his chest along his facing with EVERY layer number in the low
//!   byte of the filter word, grouped by what it meets first: the layers that see a blocker and those that do not.
//! - `Walls`: the non-floor contacts his own movement casts met in the last second (`w.hits = 1`).

use std::collections::BTreeMap;

use glam::Vec3;

use crate::erlog;
use crate::havok::{self, CastHit};
use crate::probe;

/// "CSWorldGeomDynamicIns AEG027_039_1000" / "CSWorldGeomHitIns h002000" / "owner 0x..".
pub fn owner_text(owner_ptr: usize) -> String {
    if owner_ptr == 0 {
        return "no owner".to_string();
    }
    let class = probe::class_of(owner_ptr).unwrap_or_else(|| "?".to_string());
    let mut s = class.clone();
    if class.contains("Geom") {
        // (SDK layout, MEASURED IMPL_ER17: + 0x48 its MSB part, whose first word points at the UTF-16 name)
        if let Some(name) = probe::word(owner_ptr + 0x48).and_then(probe::word).and_then(utf16_at) {
            s.push(' ');
            s.push_str(&name);
        }
    }
    s
}

fn utf16_at(at: usize) -> Option<String> {
    let mut b = [0u8; 80];
    if !probe::read(at, &mut b) {
        return None;
    }
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
    let s = String::from_utf16_lossy(&units);
    (!s.is_empty() && s.chars().all(|c| c.is_ascii_graphic())).then_some(s)
}

fn first(hits: &mut Vec<CastHit>) -> Option<CastHit> {
    hits.retain(|h| h.fraction.is_finite());
    hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
    hits.first().copied()
}

/// `w.fscan`: from `chest` along `dir` (unit) over `len` metres, a sphere of `radius`, every layer 0..=255.
pub fn forward_scan(chest: Vec3, dir: Vec3, len: f32, radius: f32, base: u32) {
    let mut groups: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut hits = Vec::new();
    for layer in 0..256u32 {
        havok::cast_sphere(chest, dir * len, radius, (base & !0xFF) | layer, false, &mut hits);
        let what = match first(&mut hits) {
            Some(h) => format!("{:5.2} m: {} (owner type {}, body {}, normal ({:.2}, {:.2}, {:.2}), {} contact(s) on {} bodies)", h.fraction * len, owner_text(h.owner_ptr), h.owner, h.body, h.normal.x, h.normal.y, h.normal.z, hits.len(), { let mut b: Vec<u32> = hits.iter().map(|x| x.body).collect(); b.sort(); b.dedup(); b.len() }),
            None => "nothing".to_string(),
        };
        groups.entry(what).or_default().push(layer);
    }
    erlog!("fscan: from ({:.2}, {:.2}, {:.2}) along ({:.2}, {:.2}, {:.2}) {len} m, sphere {radius} m, filter word {:#x} with every layer: what each layer meets first:", chest.x, chest.y, chest.z, dir.x, dir.y, dir.z, base & !0xFF);
    for (what, layers) in groups {
        erlog!("fscan:   {what}: {}", ranges(&layers));
    }
    // the bits above the layer, one at a time, with the layer as it is
    let mut line = String::new();
    for bit in 8..32 {
        let word = (base & 0xFF) | (1 << bit);
        havok::cast_sphere(chest, dir * len, radius, word, false, &mut hits);
        let what = match first(&mut hits) {
            Some(h) => format!("{:.2} m {}", h.fraction * len, owner_text(h.owner_ptr)),
            None => "nothing".to_string(),
        };
        line.push_str(&format!(" | bit {bit}: {what}"));
    }
    erlog!("fscan: layer {:#x} with ONE bit above it:{line}", base & 0xFF);
}

/// "0x2-0xc 0x15 0x3a-0x3b".
pub fn ranges(layers: &[u32]) -> String {
    let mut out = Vec::new();
    let mut k = 0;
    while k < layers.len() {
        let mut j = k;
        while j + 1 < layers.len() && layers[j + 1] == layers[j] + 1 {
            j += 1;
        }
        out.push(if j > k { format!("{:#x}-{:#x}", layers[k], layers[j]) } else { format!("{:#x}", layers[k]) });
        k = j + 1;
    }
    out.join(" ")
}

/// `w.grid = n,yaw,len,half,step,height,radius`: parallel casts along `dir` from points left .. right of `chest` (ER space,
/// `side` = unit vector to the right of `dir`): where each one stops and on what = the outline of what stands ahead.
pub fn grid(chest: Vec3, dir: Vec3, len: f32, half: f32, step: f32, radius: f32, filter: u32) {
    let side = Vec3::new(-dir.z, 0.0, dir.x);
    let mut hits = Vec::new();
    let mut names: BTreeMap<u32, String> = BTreeMap::new();
    let mut line = String::new();
    let n = (half / step.max(0.01)).round() as i32;
    for k in -n..=n {
        let off = k as f32 * step;
        havok::cast_sphere(chest + side * off, dir * len, radius, filter, false, &mut hits);
        match first(&mut hits) {
            Some(h) => {
                names.entry(h.body).or_insert_with(|| owner_text(h.owner_ptr));
                line.push_str(&format!(" {off:+.2}:{:.2}/{}", h.fraction * len, h.body));
            }
            None => line.push_str(&format!(" {off:+.2}:FREE")),
        }
    }
    erlog!("grid: from ({:.2}, {:.2}, {:.2}) along ({:.2}, {:.2}), side ({:.2}, {:.2}), {len} m, sphere {radius} m, filter {filter:#x}: offset:distance/body:{line}", chest.x, chest.y, chest.z, dir.x, dir.z, side.x, side.z);
    erlog!("grid: bodies: {}", names.iter().map(|(b, n)| format!("{b} = {n}")).collect::<Vec<_>>().join(", "));
}
/// One body his casts met (not a floor): how often, where last, what it is.
#[derive(Clone, Copy)]
pub struct Wall {
    pub count: u32,
    pub position: Vec3,
    pub normal: Vec3,
    pub owner_ptr: usize,
    pub nearest: f32,
}

/// (body, filter word of the hit, owner type) -> what was met.
pub type Walls = BTreeMap<(u32, u32, u8), Wall>;

pub fn note(walls: &mut Walls, h: &CastHit, feet: Vec3) {
    if h.normal.y.abs() > 0.7 || walls.len() >= 24 && !walls.contains_key(&(h.body, h.filter, h.owner)) {
        return;
    }
    let d = ((h.position - feet) * Vec3::new(1.0, 0.0, 1.0)).length();
    let w = walls.entry((h.body, h.filter, h.owner)).or_insert(Wall { count: 0, position: h.position, normal: h.normal, owner_ptr: h.owner_ptr, nearest: f32::MAX });
    w.count += 1;
    if d < w.nearest {
        (w.nearest, w.position, w.normal) = (d, h.position, h.normal);
    }
}

pub fn log(walls: &Walls, feet: Vec3) {
    if walls.is_empty() {
        return;
    }
    erlog!("hits: what his casts met this second that is not a floor (feet at ({:.2}, {:.2}, {:.2})):", feet.x, feet.y, feet.z);
    let mut list: Vec<_> = walls.iter().collect();
    list.sort_by(|a, b| a.1.nearest.total_cmp(&b.1.nearest));
    for ((body, filter, owner), w) in list.into_iter().take(10) {
        erlog!("hits:   {:.2} m away, {:+.2} m above his feet, normal ({:.2}, {:.2}, {:.2}): {} | body {body} filter {filter:#x} owner type {owner} | {} contact(s)", w.nearest, w.position.y - feet.y, w.normal.x, w.normal.y, w.normal.z, owner_text(w.owner_ptr), w.count);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn layer_lists_are_written_as_ranges() {
        assert_eq!(super::ranges(&[2, 3, 4, 9, 0x15, 0x16]), "0x2-0x4 0x9 0x15-0x16");
        assert_eq!(super::ranges(&[]), "");
    }
}
