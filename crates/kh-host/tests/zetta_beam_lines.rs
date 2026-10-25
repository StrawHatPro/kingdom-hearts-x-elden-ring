//! (ZETTA round 5c) EVERY BEAM CARD LIES ON THE BEAM'S LINE (the user, with a screenshot of the ending's wide shot: "See the
//! bits of light coming towards the camera and it's being cut off? Thats the extra beam I'm talking about that needs to be
//! removed"; the coordinator: "No beam layer may start at or pass through the camera, and nothing may run off at another
//! angle").
//!
//! The beam's effects (sheet `party-don-zetta-fx`, rows of frames `beam_stream` and `end_beam`) are placed exactly as the
//! sequence places them (`zettaflare::stream_place_girth`, the ending beam's frame from the wand's tip along the aim line),
//! simulated by the real evaluator (kh-vfx) over each row's whole life with the packed effects, and every mesh they draw is
//! measured in the world with its own bounds (the hosts' mesh axes: glTF (x, z, y) x 100):
//! - a long card (its longest side 3x the next) runs ALONG the line (within 3 deg), its whole length between the wand's tip
//!   (never behind it: that is where Donald and the camera behind him are) and the line's far end, its centre line on it;
//! - a ring (two long sides, one thin) sits around the line (its thin axis along it, its centre on it);
//! - anything else (a ring turning about the beam's start, a lump) is centred on the line.
//! The cards and rings may stray as far as the event authored them (BEHIND_OK, SIDE_OK, RING_OK) and no further.
//! Several aims (level, rising, falling, sideways) and the beam's girth for a small and a huge target.
//!
//! The pack: `ZETTA_FX_DIR` (a `.../vfx` folder), else the shipped test-level pack `build/play/donpack/party/vfx`, else
//! ZETTA's staging pack; skipped (with a note) when none is there.
use glam::Vec3;
use kh_party::donald::zettaflare::{self as zf, Basis, DON_ZETTA_FX};
use kh_vfx::{rotator, Camera, DrawList, EffectDef, SpawnOptions, World, Xform};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn vfx_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("ZETTA_FX_DIR") {
        return Some(PathBuf::from(d));
    }
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    for rel in ["build/play/donpack/party/vfx", "build/sora_tmp/party/ZETTA/pack/vfx"] {
        if let Some(p) = here.ancestors().map(|a| a.join(rel)).find(|p| p.join("effects").is_dir()) {
            return Some(p);
        }
    }
    None
}

/// A packed mesh's bounds in the hosts' mesh space (glTF (x, z, y) x 100: kh-app vfx.rs).
fn mesh_bounds(dir: &Path, name: &str) -> Option<(Vec3, Vec3)> {
    let b = std::fs::read(dir.join("meshes").join(format!("{name}.glb"))).ok()?;
    let len = u32::from_le_bytes(b.get(12..16)?.try_into().ok()?) as usize;
    let j: serde_json::Value = serde_json::from_slice(b.get(20..20 + len)?).ok()?;
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for m in j["meshes"].as_array()? {
        for p in m["primitives"].as_array()? {
            let a = &j["accessors"][p["attributes"]["POSITION"].as_u64()? as usize];
            let v = |k: &str| -> Option<Vec3> {
                let x = a[k].as_array()?;
                let f = |i: usize| x.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                Some(Vec3::new(f(0), f(2), f(1)) * 100.0)
            };
            let (p0, p1) = (v("min")?, v("max")?);
            lo = lo.min(p0.min(p1));
            hi = hi.max(p0.max(p1));
        }
    }
    (lo.x <= hi.x).then_some((lo, hi))
}

struct Line {
    staff: Vec3,
    d: Vec3,
    /// how far along it the beam may reach (cm)
    reach: f32,
}

/// One mesh's fault against the line, if any.
fn fault(line: &Line, m: &kh_vfx::MeshInstance, ring_mesh: bool, lo: Vec3, hi: Vec3) -> Option<String> {
    let t = m.transform;
    let c = (lo + hi) * 0.5;
    let h = (hi - lo) * 0.5;
    let centre = Vec3::from(t.transform_point3a(c.into()));
    let axes = [Vec3::from(t.matrix3.x_axis), Vec3::from(t.matrix3.y_axis), Vec3::from(t.matrix3.z_axis)];
    let mut ext: Vec<(f32, Vec3)> = (0..3).map(|i| (axes[i].length() * h[i], axes[i].normalize_or_zero())).collect();
    ext.sort_by(|a, b| b.0.total_cmp(&a.0));
    if ext[0].0 < 1.0 {
        return None;
    }
    let along = |p: Vec3| (p - line.staff).dot(line.d);
    let off = |p: Vec3| (p - line.staff - line.d * along(p)).length();
    let on_line = |dir: Vec3| dir.dot(line.d).abs() >= 3f32.to_radians().cos();
    // a card along the line: its length between the tip (BEHIND_OK) and the line's end, its centre line on the line
    let card = |half: f32, dir: Vec3, width: f32| -> Option<String> {
        let (a, b) = (along(centre - dir * half), along(centre + dir * half));
        let (t0, t1) = (a.min(b), a.max(b));
        if t0 < -BEHIND_OK {
            return Some(format!("a card starts {:.0} cm BEHIND the wand's tip (toward Donald's back)", -t0));
        }
        if t1 > line.reach * 1.03 + 100.0 {
            return Some(format!("a card runs {:.0} cm past the beam's end", t1 - line.reach));
        }
        let side = off(centre);
        (side > width + SIDE_OK).then(|| format!("a card's centre line is {side:.0} cm off the line (its half width {width:.0})"))
    };
    let elongated = ext[0].0 > 3.0 * ext[1].0.max(1e-3);
    let flat = !elongated && ext[1].0 > 3.0 * ext[2].0.max(1e-3);
    if elongated && !ring_mesh {
        let (half, dir) = ext[0];
        if !on_line(dir) {
            return Some(format!("a card {:.0} cm long runs {:.1} deg off the line", 2.0 * half, dir.dot(line.d).abs().clamp(-1.0, 1.0).acos().to_degrees()));
        }
        card(half, dir, ext[1].0)
    } else if flat && !ring_mesh && on_line(ext[0].1) {
        card(ext[0].0, ext[0].1, ext[1].0)
    } else if flat && !ring_mesh && on_line(ext[1].1) {
        card(ext[1].0, ext[1].1, ext[0].0)
    } else if flat && !ring_mesh && !on_line(ext[2].1) {
        Some(format!("a flat card {:.0} x {:.0} cm lies at an angle to the line", 2.0 * ext[0].0, 2.0 * ext[1].0))
    } else {
        // a ring (around the line, or turning about the beam's start, as the event authored its aura rings), a disc
        // across the line, a lump: centred on the line, not behind the tip
        (off(centre) > RING_OK || along(centre) < -RING_OK).then(|| format!("a {:.0} cm mesh is centred {:.0} cm off the line, {:.0} along it", 2.0 * ext[0].0, off(centre), along(centre)))
    }
}

/// How far behind the wand's tip a card may start (cm): the event's own second c94 card starts 1 m back (at his hand)
const BEHIND_OK: f32 = 150.0;
/// How far beside the line a card's centre line may run beyond its own half width (cm): the event's second c94 core
/// card runs 1.5 m beside the first (a parallel strand)
const SIDE_OK: f32 = 250.0;
/// How far from the line (and behind the tip) a ring's or lump's centre may be (cm): the ending's aura rings turn about
/// the beam's start (vd_kg702b_96_zfr_00 aura_ring: they live its first 5 s, the wide shot 960 only)
const RING_OK: f32 = 300.0;

fn xform(at: Vec3, rot: Vec3, scale: Vec3) -> Xform {
    Xform { position: at, rotation: rotator(rot.x, rot.y, rot.z), scale }
}

#[test]
fn every_beam_card_lies_on_the_line_from_the_wand_tip() {
    let Some(dir) = vfx_dir() else {
        eprintln!("zetta_beam_lines: no effect pack here (ZETTA_FX_DIR or build/play/donpack/party/vfx): skipped");
        return;
    };
    let z = zf::zetta();
    let rows: Vec<_> = DON_ZETTA_FX.iter().filter(|r| r.use_ == 1 && r.kind == "fx" && (r.frame == "beam_stream" || r.frame == "end_beam")).collect();
    assert!(rows.iter().any(|r| r.frame == "beam_stream") && rows.iter().any(|r| r.frame == "end_beam"), "the beam's rows are in the sheet");
    let mut bounds: HashMap<String, Option<(Vec3, Vec3)>> = HashMap::new();
    let mut faults = Vec::new();
    let mut seen = 0usize;
    let staff = Vec3::new(0.0, 0.0, 120.0);
    // (aim, girth): level ahead, rising, falling, to the side, far; the girth of a small and a huge target
    let aims = [(Vec3::new(700.0, 0.0, 90.0), 1.0), (Vec3::new(1200.0, 300.0, 700.0), 1.0), (Vec3::new(900.0, -200.0, -150.0), 1.0), (Vec3::new(-30.0, 2500.0, 120.0), 1.0), (Vec3::new(3000.0, 0.0, 400.0), 4.0)];
    // (the cards turned to a camera beside the line, below it, or none: their rows' own rolls)
    for (k, (aim, girth)) in aims.into_iter().enumerate() {
        let eye = [None, Some(Vec3::new(400.0, -600.0, 60.0)), Some(Vec3::new(300.0, 250.0, -200.0))][k % 3];
        let beam = Basis::beam(staff, aim);
        for row in &rows {
            let path = dir.join("effects").join(format!("{}.json", row.asset));
            let Ok(text) = std::fs::read_to_string(&path) else {
                faults.push(format!("{}: {} is not packed", row.id, row.asset));
                continue;
            };
            let def = EffectDef::parse(&text).expect("the effect parses");
            let mut w = World::new();
            w.register(def);
            // the row's place (as the sequence puts it) at matinee time `mt`
            let place = |mt: f32| -> (Xform, Line) {
                if row.frame == "beam_stream" {
                    let (at, rot, scale) = zf::stream_place_facing(row, staff, beam, mt, &z, girth, eye);
                    let reach = (aim - staff).length() + z.zetta_beam_beyond.max(0.0);
                    (xform(at, rot, scale), Line { staff, d: beam.x, reach })
                } else {
                    let mut b = beam;
                    b.origin = staff;
                    let off = Vec3::from_slice(row.offset);
                    let rot = Vec3::from_slice(row.rotation);
                    (xform(b.point(off), b.rotation(rot), Vec3::splat(row.scale)), Line { staff, d: beam.x, reach: 20000.0 })
                }
            };
            let start = row.start;
            let end = if row.end > row.start { row.end } else { row.start + 4.0 };
            let (x0, _) = place(start);
            let Some(id) = w.spawn(row.asset, x0, SpawnOptions { seed: Some(7), ..SpawnOptions::default() }) else {
                faults.push(format!("{}: {} did not spawn", row.id, row.asset));
                continue;
            };
            let dt = 1.0 / 30.0;
            let mut mt = start;
            let mut k = 0;
            while mt < end + 1.0 {
                let (x, line) = place(mt.min(end));
                w.set_transform(id, x);
                w.step(dt);
                mt += dt;
                k += 1;
                if k % 6 != 0 {
                    continue;
                }
                let mut out = DrawList::default();
                w.draw(1.0, &Camera::DEFAULT, &mut out);
                for m in &out.meshes {
                    if m.color[3] <= 1e-3 {
                        continue;
                    }
                    let name = w.mesh_name(m.mesh).unwrap_or("?").to_string();
                    let Some((lo, hi)) = *bounds.entry(name.clone()).or_insert_with(|| mesh_bounds(&dir, &name)) else { continue };
                    seen += 1;
                    if let Some(f) = fault(&line, m, name.contains("_rng_"), lo, hi) {
                        let msg = format!("{} ({} mesh {name}) at {mt:.2}, aim {aim:?}: {f}", row.id, row.asset);
                        if !faults.iter().any(|x: &String| x.starts_with(&format!("{} ({} mesh {name})", row.id, row.asset))) {
                            faults.push(msg);
                        }
                    }
                }
            }
        }
    }
    assert!(seen > 100, "the beam's cards were drawn and measured ({seen})");
    assert!(faults.is_empty(), "beam layers off the line from the wand's tip:\n{}", faults.join("\n"));
}

/// (2026-10-23) the user: "the beam must NEVER extend behind Donald during the zoomed out shot"; the coordinator: "at scale
/// factors 1, 2.5, 5 and 10, the back end of every beam piece is at or in front of Donald's plane". Every piece of the beam
/// (its cards along the line, the ending beam, the light at its source), placed exactly as the sequence places it at that
/// size (`stream_place_facing` with the girth, `end_beam_at`, `end_glow_at`), simulated over its whole life with the packed
/// effects: every corner of every mesh it draws lies at or in front of the plane through Donald square to the beam.
#[test]
fn no_beam_piece_reaches_behind_donald_at_any_size() {
    let Some(dir) = vfx_dir() else {
        eprintln!("zetta_beam_lines: no effect pack here: skipped");
        return;
    };
    let z = zf::zetta();
    let rows: Vec<_> = DON_ZETTA_FX.iter().filter(|r| r.use_ == 1 && r.kind == "fx" && matches!(r.frame, "beam_stream" | "end_beam" | "end_glow")).collect();
    let mut bounds: HashMap<String, Option<(Vec3, Vec3)>> = HashMap::new();
    let mut faults = Vec::new();
    let mut worst: HashMap<String, f32> = HashMap::new();
    let mut starts: HashMap<String, f32> = HashMap::new();
    let mut seen = 0usize;
    let donald = zf::Frame { origin: Vec3::ZERO, yaw: 0.0 };
    let staff = donald.point(zf::staff_local(&z));
    let body = Vec3::new(0.0, 0.0, 60.0);
    for (aim, f) in [(Vec3::new(1500.0, 0.0, 90.0), 1.0f32), (Vec3::new(1500.0, 200.0, 400.0), 2.5), (Vec3::new(2500.0, -300.0, 200.0), 5.0), (Vec3::new(4000.0, 0.0, 500.0), 10.0), (Vec3::new(900.0, 0.0, 60.0), 10.0)] {
        let beam = Basis::beam(staff, aim);
        let mut tipb = beam;
        tipb.origin = staff;
        let plane = (body - staff).dot(beam.x);
        for row in &rows {
            let Ok(text) = std::fs::read_to_string(dir.join("effects").join(format!("{}.json", row.asset))) else { continue };
            let def = EffectDef::parse(&text).expect("the effect parses");
            let mut w = World::new();
            w.register(def);
            let place = |mt: f32| -> Xform {
                match row.frame {
                    "beam_stream" => {
                        let (at, rot, scale) = zf::stream_place_facing(row, staff, beam, mt, &z, f, Some(Vec3::new(400.0, -600.0, 60.0)));
                        xform(at, rot, scale)
                    }
                    "end_beam" => {
                        let s = (if row.scale > 0.0 { row.scale } else { 1.0 }) * f;
                        let (at, sc) = zf::end_beam_draw(row, tipb, s, f, mt - row.start);
                        xform(at, tipb.rotation(Vec3::from_slice(row.rotation)), sc)
                    }
                    _ => {
                        // (the wide shot's override: 1.4 x)
                        let s = zf::glow_share(row, mt) * f * 1.4;
                        xform(zf::end_glow_at(row, tipb, s), Vec3::from_slice(row.rotation), Vec3::splat(s.max(1e-3)))
                    }
                }
            };
            let end = if row.end > row.start { row.end } else { row.start + 4.0 };
            let Some(id) = w.spawn(row.asset, place(row.start), SpawnOptions { seed: Some(7), ..SpawnOptions::default() }) else { continue };
            let dt = 1.0 / 30.0;
            let mut mt = row.start;
            let mut k = 0;
            while mt < end + 1.0 {
                w.set_transform(id, place(mt.min(end)));
                w.step(dt);
                mt += dt;
                k += 1;
                if k % 6 != 0 {
                    continue;
                }
                let mut out = DrawList::default();
                w.draw(1.0, &Camera::DEFAULT, &mut out);
                for m in &out.meshes {
                    if m.color[3] <= 1e-3 {
                        continue;
                    }
                    let name = w.mesh_name(m.mesh).unwrap_or("?").to_string();
                    let Some((lo, hi)) = *bounds.entry(name.clone()).or_insert_with(|| mesh_bounds(&dir, &name)) else { continue };
                    seen += 1;
                    let mut back = f32::MAX;
                    for i in 0..8 {
                        let c = Vec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
                        back = back.min((Vec3::from(m.transform.transform_point3a(c.into())) - staff).dot(beam.x));
                    }
                    // (every piece by every corner: the ending beam's 100 m cards too, grown across the line only)
                    // (its aura rings: vd_kg702b_96_zfr_00 aura_ring / aura_ring_z, the glow turning round his wand at the
                    // beam's start: his aura, which the coordinator keeps, "it's his glow, not the beam")
                    if row.frame == "end_beam" && name.contains("_rng_") {
                        continue;
                    }
                    // (the ending beam starts AT the tip at every size: no gap opens between his wand and it)
                    if row.frame == "end_beam" && back > 100.0 && !faults.iter().any(|x: &String| x.starts_with(&format!("{} ({name}) x{f} gap", row.id))) {
                        faults.push(format!("{} ({name}) x{f} gap at {mt:.2}: it starts {back:.0} cm ahead of the wand's tip", row.id));
                    }
                    // the card's centre line: where it starts along the beam (its long axis' back end)
                    {
                        let c = (lo + hi) * 0.5;
                        let h = (hi - lo) * 0.5;
                        let centre = Vec3::from(m.transform.transform_point3a(c.into()));
                        let axes = [Vec3::from(m.transform.matrix3.x_axis), Vec3::from(m.transform.matrix3.y_axis), Vec3::from(m.transform.matrix3.z_axis)];
                        let mut ext: Vec<(f32, Vec3)> = (0..3).map(|i| (axes[i].length() * h[i], axes[i].normalize_or_zero())).collect();
                        ext.sort_by(|a, b| b.0.total_cmp(&a.0));
                        if ext[0].0 > 3.0 * ext[1].0.max(1e-3) || row.frame == "end_beam" {
                            let along = |p: Vec3| (p - staff).dot(beam.x);
                            let (a0, b0) = (along(centre - ext[0].1 * ext[0].0), along(centre + ext[0].1 * ext[0].0));
                            let e = starts.entry(format!("{} x{f}", row.asset)).or_insert(f32::MAX);
                            *e = e.min(a0.min(b0));
                        }
                    }
                    let key = format!("{} x{f}", row.asset);
                    let e = worst.entry(key).or_insert(f32::MAX);
                    *e = e.min(back);
                    if back < plane - 1.0 && !faults.iter().any(|x: &String| x.starts_with(&format!("{} ({name}) x{f} ", row.id))) {
                        faults.push(format!("{} ({name}) x{f} at {mt:.2}: reaches {:.0} cm behind the wand's tip, {:.0} cm behind Donald's plane", row.id, -back, plane - back));
                    }
                }
            }
        }
    }
    // (the user: "the beam MUST start at Donald's wand") every card of the beam and the ending beam starts ON the tip at
    // every size (its earliest start over its life, within 25 cm)
    let mut st: Vec<_> = starts.iter().collect();
    st.sort_by(|a, b| a.0.cmp(b.0));
    for (k, v) in st {
        eprintln!("zetta_beam_lines: {k}: its centre line starts {v:.0} cm from the tip");
        // the ending beam: on the tip; the beam's cards: no further ahead than the event's own start (with 50 cm at the widest; its cards begin
        // 0.6 .. 1.1 m ahead of the tip, in the tip's flash) at any size: no gap opens as it widens
        let limit = if k.starts_with("vd_kg702b_96") { 25.0 } else { 160.0 };
        if *v > limit || (k.starts_with("vd_kg702b_96") && *v < -25.0) {
            faults.push(format!("{k}: its centre line starts {v:.0} cm from the wand's tip"));
        }
    }
        let mut w: Vec<_> = worst.into_iter().collect();
    w.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, v) in &w {
        eprintln!("zetta_beam_lines: {k}: its back end {v:.0} cm along the line from the tip");
    }
    assert!(seen > 50, "the beam's pieces were drawn and measured ({seen})");
    assert!(faults.is_empty(), "beam pieces behind Donald:\n{}", faults.join("\n"));
}

/// (2026-10-23) THE ENDING BEAM'S CARD LENGTH over its life, at its own size (cm): the longest of its cards (its aura rings
/// aside), sampled every 0.25 s from its start. `zettaflare::END_CARD_LEN` must never be shorter (it moves the beam forward
/// by half the length it adds, so the back end stays at the tip when the beam is drawn longer for the wide shot).
#[test]
fn the_ending_beams_card_length_table_covers_its_cards() {
    let Some(dir) = vfx_dir() else {
        eprintln!("zetta_beam_lines: no effect pack here: skipped");
        return;
    };
    let mut bounds: HashMap<String, Option<(Vec3, Vec3)>> = HashMap::new();
    let mut short = Vec::new();
    for row in DON_ZETTA_FX.iter().filter(|r| r.use_ == 1 && r.kind == "fx" && r.frame == "end_beam") {
        let Ok(text) = std::fs::read_to_string(dir.join("effects").join(format!("{}.json", row.asset))) else { continue };
        let mut w = World::new();
        w.register(EffectDef::parse(&text).expect("the effect parses"));
        let x = Vec3::X;
        let place = xform(Vec3::from_slice(row.offset), Vec3::from_slice(row.rotation), Vec3::ONE);
        let Some(id) = w.spawn(row.asset, place, SpawnOptions { seed: Some(7), ..SpawnOptions::default() }) else { continue };
        let dt = 1.0 / 60.0;
        let mut t = 0.0f32;
        let mut k = 0usize;
        let mut out_rows = Vec::new();
        while t < row.end - row.start + 0.01 {
            w.step(dt);
            t += dt;
            k += 1;
            if k % 15 != 0 {
                continue;
            }
            let mut out = DrawList::default();
            w.draw(1.0, &Camera::DEFAULT, &mut out);
            let mut longest = 0.0f32;
            for m in &out.meshes {
                if m.color[3] <= 1e-3 {
                    continue;
                }
                let name = w.mesh_name(m.mesh).unwrap_or("?").to_string();
                if name.contains("_rng_") {
                    continue;
                }
                let Some((lo, hi)) = *bounds.entry(name.clone()).or_insert_with(|| mesh_bounds(&dir, &name)) else { continue };
                let (mut a, mut b) = (f32::MAX, f32::MIN);
                for i in 0..8 {
                    let c = Vec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
                    let p = Vec3::from(m.transform.transform_point3a(c.into())).dot(x);
                    a = a.min(p);
                    b = b.max(p);
                }
                longest = longest.max(b - a);
            }
            out_rows.push((t, longest));
            let table = zf::end_card_len(row.asset, t);
            if table + 1.0 < longest {
                short.push(format!("{} at {t:.2} s: its longest card {longest:.0} cm, the table {table:.0}", row.asset));
            }
        }
        eprintln!("zetta_beam_lines: {} card lengths: {}", row.asset, out_rows.iter().map(|(t, l)| format!("{t:.2}:{l:.0}")).collect::<Vec<_>>().join(" "));
    }
    assert!(short.is_empty(), "END_CARD_LEN shorter than the cards:
{}", short.join("
"));
}
