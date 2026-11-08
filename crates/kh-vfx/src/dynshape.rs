//! (VFX-RING S) `ParticleModuleTypeDataSQEX_DynamicShape` as KH3 builds and draws it: a SPRING (helix) of flat strips.
//!
//! The class (constructor 0x144347AE0: PolygonNumber 10, Radius 100, Size 10, Height 150, Spin 1, UVTiling (1, 1),
//! CrossPolygonRotation 90, PolygonSplit 2, bIsCrossPolygon on, UVType NONE) makes its emitter instance in CreateInstance
//! 0x14434AB80 (0x890 bytes, ctor 0x14437D420, vtable 0x14775AA00; +0x868 the type data, +0x870 / +0x878 the strip-edge
//! entries, +0x880 / +0x888 the split colours). The instance overrides the stock emitter's:
//! - Tick (slot 0x30) = 0x14439CF70: the stock tick 0x14429B6F0 (its carrier particle: the emitter's Color / modules),
//!   then, unless the type data's +0x50 is set, the SHAPE BUILD 0x144371D90 (below) every tick;
//! - FillReplayData (slot 0x580) = 0x144384290: hands the entries and split colours to the render thread; it draws while
//!   it has a particle or at least two entries, with: strips = entries / 2, columns = PolygonSplit with
//!   bIsUseCustomPolygon (else 1), the cross flag, bIsUseCustomUV, UVType, bTexUReverse, bTexVReverse;
//! - its vertex fill 0x144320580 (the trail-vertex layout: position, old position, size 25, colour, UV): for every strip
//!   (entry pair e, e + 1, stepping 2 with the cross strip, else 1) and every column s of `columns`, one triangle strip
//!   with a pair of vertices per ring point, at the column's fractions s / columns and (s + 1) / columns between the two
//!   edges; colour = the emitter's particle 0 colour (white with none) x the edge entry's ring colour x the split colour
//!   of the column's side (s on edge e's side, s + 1 on edge e + 1's; white unless at least 2 split colours exist).
//!
//! THE SHAPE BUILD 0x144371D90, for ring point k = 0 ..= PolygonNumber (N), t = k / N:
//! - height h = Height / N x k (0 with Height 0), turn a = Spin x 360 / N x k degrees (yaw), radius r = RadiusScale(t) x
//!   Radius, half width w = SizeScale(t) x Size (SizeScale is read only when RadiusScale has a distribution: the code reuses
//!   RadiusScale's pointer as SizeScale's test), height scale HeightScale(t) (1 without a distribution);
//! - centre = Rot(0, a, 0) x (0, r, 0); edge offsets (0, +w, 0) and (0, -w, 0), turned by PolygonRotation (strip 0), or
//!   by Rot(0, 0, CrossPolygonRotation) then PolygonRotation (the cross strip); point = Rot(0, a, 0) x offset + centre +
//!   (0, 0, h x HeightScale(t)); then x the instance scale (+0x390 / +0x39C) and the node matrix (+0x60): here, the node's
//!   world transform;
//! - per entry and point: colour (PolygonColor(t), black without a distribution; PolygonAlpha(t), 1 without) and, with
//!   bIsUseCustomUV, UV = (0, v) on the first edge and (u, v) on the second: u = UTilingScale(t) x UVTiling.X, v = t x
//!   UVTiling.Y x VTilingScale(t);
//! - split colours (bIsUseCustomPolygon): for i = 0 ..= PolygonSplit, (SplitColor(i / Split), black without; SplitAlpha,
//!   1 without).
//!
//! The vertex UV (0x144320580): custom: u = lerp(edge u's, the column fraction (the other one with bTexUReverse)), v =
//! lerp(edge v's), negated with bTexVReverse; not custom: u = the column fraction, v = k / N. Then UVType 1 (RIGHT_ROT)
//! (u, v) -> (v, -u), 2 (LIGHT_ROT) -> (-v, u).
//!
//! Every curve is sampled at the ring's own t, none at the emitter time: the shape in the node's frame is fixed, so it is
//! built once here; only the node and the particle colour move it in time.

use glam::{Affine3A, Vec3};
use serde_json::Value;

use crate::dist::Dist;
use crate::draw::{Blend, DrawList, Ribbon, RibbonPoint};
use crate::math::rotator;
use crate::rng::Rng;

/// One point of one strip edge in the node's frame: its place, its UV (custom UV only) and its ring colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgePoint {
    pub pos: Vec3,
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

/// The built spring.
#[derive(Clone, Debug, PartialEq)]
pub struct DynShapeDef {
    /// The strip edges (+0x870): 2 (one strip) or 4 (with the cross strip), each with N + 1 points.
    pub edges: Vec<Vec<EdgePoint>>,
    /// The split colours (+0x880), empty without bIsUseCustomPolygon.
    pub split: Vec<[f32; 4]>,
    /// Columns per strip: PolygonSplit with bIsUseCustomPolygon, else 1.
    pub columns: u32,
    pub cross: bool,
    pub custom_uv: bool,
    /// `SQEX_PARTICLE_UV_ROT_TYPE`: 0 none, 1 RIGHT_ROT, 2 LIGHT_ROT.
    pub uv_type: u8,
    pub u_reverse: bool,
    pub v_reverse: bool,
}

/// A ribbon drawn EXACTLY as listed (no smoothing between its points), with its own UVs per vertex and its own colour on
/// the B edge (`DrawList::ribbon_exact`): the trail-vertex strips of a dynamic shape.
#[derive(Clone, Debug, PartialEq)]
pub struct RibbonExact {
    /// Index of the ribbon in `DrawList::ribbons`.
    pub ribbon: u32,
    /// Per point: (A u, A v, B u, B v).
    pub uvs: Vec<[f32; 4]>,
    /// Per point: the B edge's colour (the point's `color` is the A edge's).
    pub b_colors: Vec<[f32; 4]>,
}

fn dist(params: &Value, name: &str) -> Option<Dist> {
    params.get(name).and_then(Dist::parse)
}

fn num(params: &Value, name: &str, default: f32) -> f32 {
    params.get(name).and_then(Value::as_f64).map_or(default, |v| v as f32)
}

fn flag(params: &Value, name: &str, default: bool) -> bool {
    params.get(name).and_then(Value::as_bool).unwrap_or(default)
}

fn rot_of(params: &Value, name: &str) -> (f32, f32, f32) {
    let v = params.get(name);
    let n = |k: &str| v.and_then(|v| v.get(k)).and_then(Value::as_f64).map_or(0.0, |f| f as f32);
    (n("pitch"), n("yaw"), n("roll"))
}

impl DynShapeDef {
    /// From the type data's `params`; None for a shape type other than Spring (the only one 0x144371D90 builds; the
    /// preview 0x14436ED30 draws Spring only too).
    pub fn parse(params: &Value) -> Option<DynShapeDef> {
        let kind = params.get("DynamicShapeType").and_then(Value::as_str).unwrap_or("ESQEX_DYNAMIC_SHAPE_TYPE_Spring");
        if !kind.ends_with("_Spring") {
            return None;
        }
        // the curves have no randomness here, but they take the instance's random stream (component +0x18)
        let mut rng = Rng::new(1);
        let n = params.get("PolygonNumber").and_then(Value::as_i64).unwrap_or(10).max(0) as usize;
        let (radius, size, height, spin) = (num(params, "Radius", 100.0), num(params, "Size", 10.0), num(params, "Height", 150.0), num(params, "Spin", 1.0));
        let (radius_scale, size_scale, height_scale) = (dist(params, "RadiusScale"), dist(params, "SizeScale"), dist(params, "HeightScale"));
        let (color, alpha) = (dist(params, "PolygonColor"), dist(params, "PolygonAlpha"));
        let cross = flag(params, "bIsCrossPolygon", true);
        let custom_uv = flag(params, "bIsUseCustomUV", false);
        let tiling = params.get("UVTiling").map_or((1.0, 1.0), |v| (v.get("x").and_then(Value::as_f64).map_or(1.0, |f| f as f32), v.get("y").and_then(Value::as_f64).map_or(1.0, |f| f as f32)));
        let (u_scale, v_scale) = (dist(params, "UTilingScale"), dist(params, "VTilingScale"));
        let (pp, py, pr) = rot_of(params, "PolygonRotation");
        let poly = rotator(pp, py, pr);
        let cross_rot = rotator(0.0, 0.0, num(params, "CrossPolygonRotation", 90.0));
        let mut edges = vec![Vec::with_capacity(n + 1); if cross { 4 } else { 2 }];
        let nf = n as f32;
        for k in 0..=n {
            let kf = k as f32;
            let t = if k == 0 { 0.0 } else { kf / nf };
            let h = if height == 0.0 { 0.0 } else { height / nf * kf };
            let a = if spin == 0.0 { 0.0 } else { spin * 360.0 / nf * kf };
            let r = radius_scale.as_ref().map_or(1.0, |d| d.float(t, &mut rng)) * radius;
            let w = if radius_scale.is_some() { size_scale.as_ref().map_or(1.0, |d| d.float(t, &mut rng)) } else { 1.0 } * size;
            let hs = height_scale.as_ref().map_or(1.0, |d| d.float(t, &mut rng));
            let (u, v) = if custom_uv {
                (u_scale.as_ref().map_or(1.0, |d| d.float(t, &mut rng)) * tiling.0, t * tiling.1 * v_scale.as_ref().map_or(1.0, |d| d.float(t, &mut rng)))
            } else {
                (0.0, 0.0)
            };
            let c = color.as_ref().map_or(Vec3::ZERO, |d| d.vec3(t, &mut rng));
            let rgba = [c.x, c.y, c.z, alpha.as_ref().map_or(1.0, |d| d.float(t, &mut rng))];
            let turn = rotator(0.0, a, 0.0);
            let centre = turn * Vec3::new(0.0, r, 0.0) + Vec3::new(0.0, 0.0, h * hs);
            for (e, edge) in edges.iter_mut().enumerate() {
                let off = Vec3::new(0.0, if e % 2 == 0 { w } else { -w }, 0.0);
                let off = if e < 2 { poly * off } else { poly * (cross_rot * off) };
                edge.push(EdgePoint { pos: turn * off + centre, uv: if e % 2 == 0 { [0.0, v] } else { [u, v] }, color: rgba });
            }
        }
        let custom_polygon = flag(params, "bIsUseCustomPolygon", false);
        let splits = params.get("PolygonSplit").and_then(Value::as_i64).unwrap_or(2).max(0) as usize;
        let split = if custom_polygon {
            let (sc, sa) = (dist(params, "SplitColor"), dist(params, "SplitAlpha"));
            (0..=splits)
                .map(|i| {
                    let t = if i == 0 { 0.0 } else { i as f32 / splits as f32 };
                    let c = sc.as_ref().map_or(Vec3::ZERO, |d| d.vec3(t, &mut rng));
                    [c.x, c.y, c.z, sa.as_ref().map_or(1.0, |d| d.float(t, &mut rng))]
                })
                .collect()
        } else {
            Vec::new()
        };
        let uv_type = match params.get("UVType").and_then(Value::as_str).unwrap_or("") {
            t if t.ends_with("RIGHT_ROT") => 1,
            t if t.ends_with("LIGHT_ROT") => 2,
            _ => 0,
        };
        Some(DynShapeDef {
            edges,
            split,
            columns: if custom_polygon { splits as u32 } else { 1 },
            cross,
            custom_uv,
            uv_type,
            u_reverse: flag(params, "bTexUReverse", false),
            v_reverse: flag(params, "bTexVReverse", false),
        })
    }

    /// The UV of a vertex at across-fraction `f` of a strip (0x144320580): `a` / `b` the two edges' points, `k` the point.
    fn uv(&self, a: &EdgePoint, b: &EdgePoint, k: usize, f: f32, f_other: f32) -> [f32; 2] {
        let count = a_len(&self.edges).max(2) as f32;
        let (u, v) = if self.custom_uv {
            let fu = if self.u_reverse { f_other } else { f };
            (a.uv[0] + (b.uv[0] - a.uv[0]) * fu, a.uv[1] + (b.uv[1] - a.uv[1]) * f)
        } else {
            (if self.u_reverse { f_other } else { f }, k as f32 / (count - 1.0))
        };
        let v = if self.v_reverse { -v } else { v };
        match self.uv_type {
            1 => [v, -u],
            2 => [-v, u],
            _ => [u, v],
        }
    }

    /// Draws the spring on `node` (the node in the world with its scale): one exact ribbon per strip column.
    pub fn draw(&self, node: &Affine3A, look: &Look, out: &mut DrawList) {
        let Look { particle, dynamic, material, time, blend } = *look;
        let white = [1.0f32; 4];
        let split = |i: usize| if self.split.len() < 2 { white } else { self.split.get(i).copied().unwrap_or(white) };
        let mul = |a: [f32; 4], b: [f32; 4], c: [f32; 4]| [a[0] * b[0] * c[0], a[1] * b[1] * c[1], a[2] * b[2] * c[2], a[3] * b[3] * c[3]];
        let step = if self.cross { 2 } else { 1 };
        let columns = self.columns.max(1) as usize;
        let mut e = 0;
        while e + 1 < self.edges.len() {
            let (ea, eb) = (&self.edges[e], &self.edges[e + 1]);
            for s in 0..columns {
                let (f0, f1) = (s as f32 / columns as f32, (s + 1) as f32 / columns as f32);
                let mut points = Vec::with_capacity(ea.len());
                let mut exact = RibbonExact { ribbon: out.ribbons.len() as u32, uvs: Vec::with_capacity(ea.len()), b_colors: Vec::with_capacity(ea.len()) };
                for (k, (a, b)) in ea.iter().zip(eb).enumerate() {
                    let ua = self.uv(a, b, k, f0, f1);
                    let ub = self.uv(a, b, k, f1, f0);
                    points.push(RibbonPoint { a: node.transform_point3(a.pos.lerp(b.pos, f0)), b: node.transform_point3(a.pos.lerp(b.pos, f1)), u: ua[0], color: mul(particle, a.color, split(s)) });
                    exact.uvs.push([ua[0], ua[1], ub[0], ub[1]]);
                    exact.b_colors.push(mul(particle, b.color, split(s + 1)));
                }
                if points.len() >= 2 {
                    out.ribbon_exact.push(exact);
                    out.ribbons.push(Ribbon { points, material, dynamic, time, blend, tiles: 1.0, v_scale: 1.0, uv_turned: false });
                }
            }
            e += step;
        }
    }
}

fn a_len(edges: &[Vec<EdgePoint>]) -> usize {
    edges.first().map_or(0, Vec::len)
}

/// How one emitter instance draws its spring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// The emitter's particle 0 colour with the draw tint (white x the tint without a particle).
    pub particle: [f32; 4],
    /// Particle 0's material dynamic parameters.
    pub dynamic: [f32; 4],
    pub material: u32,
    /// Seconds the emitter instance has lived.
    pub time: f32,
    pub blend: Blend,
}

impl DrawList {
    /// (VFX-RING S) The exact record of ribbon `index` (`ribbon_exact`, kept in ribbon order), if it is one.
    pub fn ribbon_exact_of(&self, index: usize) -> Option<&RibbonExact> {
        self.ribbon_exact.binary_search_by_key(&(index as u32), |x| x.ribbon).ok().map(|k| &self.ribbon_exact[k])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn curve(keys: &[(f32, f32)]) -> Value {
        let k: Vec<Value> = keys.iter().map(|(t, v)| json!({ "t": t, "interp": "CIM_Linear", "value": [v], "arrive_tangent": [0], "leave_tangent": [0] })).collect();
        let lut: Vec<f32> = (0..=32).map(|i| {
            let t = i as f32 / 32.0;
            let (mut v, mut prev) = (keys[0].1, keys[0]);
            for &(kt, kv) in keys {
                if t >= kt {
                    v = kv;
                    prev = (kt, kv);
                } else {
                    v = prev.1 + (kv - prev.1) * (t - prev.0) / (kt - prev.0);
                    break;
                }
            }
            v
        }).collect();
        json!({ "distribution": { "class": "DistributionFloatConstantCurve", "value_kind": "float", "type": "constant_curve", "curve": { "keys": k },
            "lookup_table": { "op": 1, "op_name": "RDO_None", "entry_count": 33, "entry_stride": 1, "sub_entry_stride": 0, "time_scale": 32, "time_bias": 0, "lock_flag": 0, "values": lut } } })
    }

    fn rasen() -> Value {
        json!({ "DynamicShapeType": "ESQEX_DYNAMIC_SHAPE_TYPE_Spring", "PolygonNumber": 96, "Radius": 65, "Size": 30, "Height": 100, "Spin": 1,
            "RadiusScale": curve(&[(0.0, 1.0), (1.0, 1.0)]), "SizeScale": curve(&[(0.0, 1.0), (1.0, 1.0)]), "HeightScale": curve(&[(0.0, 0.0), (1.0, 1.0)]),
            "PolygonRotation": { "pitch": 0, "yaw": 0, "roll": 45 }, "bIsCrossPolygon": true, "CrossPolygonRotation": 90, "bIsUseCustomPolygon": true, "PolygonSplit": 2,
            "bIsUseCustomUV": true, "UVTiling": { "x": 1, "y": 1 }, "UTilingScale": curve(&[(0.0, 1.0), (1.0, 1.0)]), "VTilingScale": curve(&[(0.0, 0.0), (1.0, 1.0)]),
            "UVType": "SQEX_PARTICLE_UV_ROT_TYPE_LIGHT_ROT" })
    }

    #[test]
    fn the_spring_climbs_and_turns_once_round_its_radius() {
        let d = DynShapeDef::parse(&rasen()).expect("a spring");
        assert_eq!(d.edges.len(), 4, "the cross strip doubles the edges");
        assert!(d.edges.iter().all(|e| e.len() == 97), "N + 1 points");
        // the strip's middle (the two edges' mean) is the ring point: radius 65 round Z, height x HeightScale(t) = 100 t^2
        for k in [0usize, 24, 48, 72, 96] {
            let t = k as f32 / 96.0;
            let mid = (d.edges[0][k].pos + d.edges[1][k].pos) * 0.5;
            let a = (360.0 * t).to_radians();
            let want = Vec3::new(-65.0 * a.sin(), 65.0 * a.cos(), 100.0 * t * t);
            assert!((mid - want).length() < 0.05, "k {k}: {mid:?} vs {want:?}");
            // each edge 30 cm from the middle (Size = the half width), the two strips crossed at 90 degrees
            let (d0, d1) = (d.edges[0][k].pos - mid, d.edges[2][k].pos - (d.edges[2][k].pos + d.edges[3][k].pos) * 0.5);
            assert!((d0.length() - 30.0).abs() < 1.0e-3 && (d1.length() - 30.0).abs() < 1.0e-3);
            assert!(d0.dot(d1).abs() < 1.0e-2, "crossed: {d0:?} . {d1:?}");
        }
        // PolygonRotation roll 45: the first strip's offset leaves the ring plane by 45 degrees
        let d0 = d.edges[0][0].pos - (d.edges[0][0].pos + d.edges[1][0].pos) * 0.5;
        assert!((d0.z.abs() - 30.0 * 45f32.to_radians().sin()).abs() < 1.0e-3, "{d0:?}");
        // custom UV: (0, v) and (u, v), v = t x VTilingScale(t) = t^2
        assert_eq!(d.edges[0][48].uv[0], 0.0);
        assert!((d.edges[1][48].uv[0] - 1.0).abs() < 1.0e-5 && (d.edges[1][48].uv[1] - 0.25).abs() < 1.0e-3);
        assert_eq!((d.columns, d.split.len(), d.uv_type), (2, 3, 2));
        // no SplitColor distribution: black split colours, alpha 1 (the constructor's zero vector)
        assert_eq!(d.split[0], [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn it_draws_one_exact_ribbon_per_strip_column_with_the_fill_uvs_and_colours() {
        let mut p = rasen();
        p["SplitColor"] = json!({ "distribution": { "class": "DistributionVectorConstant", "value_kind": "vector", "type": "constant", "constant": [1, 1, 1],
            "lookup_table": { "op": 1, "op_name": "RDO_None", "entry_count": 1, "entry_stride": 3, "sub_entry_stride": 0, "time_scale": 0, "time_bias": 0, "lock_flag": 0, "values": [1, 1, 1] } } });
        let d = DynShapeDef::parse(&p).expect("a spring");
        let mut out = DrawList::default();
        let node = Affine3A::from_translation(Vec3::new(10.0, 0.0, 100.0));
        d.draw(&node, &Look { particle: [0.5, 1.0, 1.0, 1.0], dynamic: [0.0; 4], material: 7, time: 1.5, blend: Blend::Translucent }, &mut out);
        assert_eq!(out.ribbons.len(), 4, "2 strips x 2 columns");
        assert_eq!(out.ribbon_exact.len(), 4);
        for (i, x) in out.ribbon_exact.iter().enumerate() {
            assert_eq!(x.ribbon as usize, i);
            assert_eq!(x.uvs.len(), 97);
        }
        let r0 = &out.ribbons[0];
        // column 0 runs from edge 0 to the strip's middle; moved by the node
        assert!((r0.points[0].a - (d.edges[0][0].pos + Vec3::from(node.translation))).length() < 1.0e-4);
        assert!((r0.points[0].b - ((d.edges[0][0].pos + d.edges[1][0].pos) * 0.5 + Vec3::from(node.translation))).length() < 1.0e-4);
        // LIGHT_ROT (u, v) -> (-v, u): at k = 48, v = 0.25; edge 0 u = 0, the middle u = 0.5
        let uv = out.ribbon_exact[0].uvs[48];
        assert!((uv[0] + 0.25).abs() < 1.0e-3 && uv[1].abs() < 1.0e-5 && (uv[2] + 0.25).abs() < 1.0e-3 && (uv[3] - 0.5).abs() < 1.0e-5, "{uv:?}");
        // colour: particle x ring colour (no PolygonColor distribution: black) x split
        assert_eq!(r0.points[10].color, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!((r0.material, r0.time, r0.blend), (7, 1.5, Blend::Translucent));
    }

    #[test]
    fn without_cross_or_custom_polygon_it_is_one_strip_of_one_column() {
        let mut p = rasen();
        p["bIsCrossPolygon"] = json!(false);
        p["bIsUseCustomPolygon"] = json!(false);
        p["bIsUseCustomUV"] = json!(false);
        p["UVType"] = json!("SQEX_PARTICLE_UV_ROT_TYPE_NONE");
        let d = DynShapeDef::parse(&p).expect("a spring");
        assert_eq!((d.edges.len(), d.columns, d.split.len()), (2, 1, 0));
        let mut out = DrawList::default();
        d.draw(&Affine3A::IDENTITY, &Look { particle: [1.0; 4], dynamic: [0.0; 4], material: 0, time: 0.0, blend: Blend::Translucent }, &mut out);
        assert_eq!(out.ribbons.len(), 1);
        // not custom: u = the across fraction, v = k / N
        let uv = out.ribbon_exact[0].uvs[96];
        assert_eq!(uv, [0.0, 1.0, 1.0, 1.0]);
        assert!(DynShapeDef::parse(&json!({ "DynamicShapeType": "ESQEX_DYNAMIC_SHAPE_TYPE_Other" })).is_none());
    }
}

