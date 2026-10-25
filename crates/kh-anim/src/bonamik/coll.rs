//! `tools/lib/bonamik/coll.js`: Bonamik collision, transcription of KH3.exe. float32 everywhere,
//! operation order of the exe.
//!
//! Collision objects live in `Scene::objs` (the JS's object identities); the lists hold indices:
//! `Scene::f50` dynamic-side objects, `Scene::f60` self-collision pairs, `Scene::f70[c]` collider
//! lists (0..47 group lists, 48.. collision-tag lists). The particle functions take the particle
//! array and particle indices and read / write the positions at the moments the JS does, so that
//! two arguments naming the same particle behave as in the JS.
//!
//! The capacity tests of the JS (`scene.f79C`, `scene.f7A0`) compare against fields the driver
//! never sets and are not reproduced.

use super::m::{add, qmul, qrot, rsqrt, sqrt, V3};
use super::records::{CollObj, Core, Particle, Scene};
use super::plane::{fn_143b1cb50, fn_143b1d720};

const C_1E8: f32 = 9.99999993922529e-9_f64 as f32; // 0x145C9C488
const C_1E4: f32 = 0.00009999999747378752_f64 as f32; // 0x145CF35A8
const C_1E9: f32 = 9.999999717180685e-10_f64 as f32; // 0x146467DB8
const C_THIRD: f32 = 0.3333333432674408_f64 as f32; // 0x145D3A010
/// global vec3 at 0x149B5C798 (FVector::ZeroVector by usage)
const Z: V3 = [0.0, 0.0, 0.0];

/// 0x143b10190 (rcx = o): bound centre of a sphere / capsule object
pub fn fn_143b10190(ps: &[Particle], o: &mut CollObj) {
    let t = o.f18;
    if ((t - 2) as u32 & 0xFFFF_FFFD) == 0 {
        // 2 capsule, 4 inverse capsule
        let (b, a) = (ps[o.f8.expect("capsule end")].f0, ps[o.f0].f0);
        o.f74 = [(b[0] + a[0]) * 0.5, (b[1] + a[1]) * 0.5, (b[2] + a[2]) * 0.5];
    } else if ((t - 1) as u32 & 0xFFFF_FFFD) == 0 {
        // 1 sphere, 3 inverse sphere
        o.f74 = ps[o.f0].f0;
    } else {
        o.f74 = [0.0, 0.0, 0.0];
    }
}

/// 0x143afc140 (rcx = o): plane update (type 6)
pub fn fn_143afc140(ps: &[Particle], o: &mut CollObj) {
    o.f68 = o.f2c;
    if let (Some(bi), Some(ci)) = (o.f8, o.f10) {
        let (a, b, c) = (ps[o.f0].f0, ps[bi].f0, ps[ci].f0);
        let pz = ((b[2] + a[2]) + c[2]) * C_THIRD;
        let py = ((b[1] + a[1]) + c[1]) * C_THIRD;
        let px = ((b[0] + a[0]) + c[0]) * C_THIRD;
        o.f2c = [px, py, pz];
        let (e1x, e1y, e1z) = (b[0] - a[0], b[1] - a[1], b[2] - a[2]); // B - A
        let (e2x, e2y, e2z) = (c[0] - b[0], c[1] - b[1], c[2] - b[2]); // C - B
        let mut nx = (e2y * e1z) - (e2z * e1y);
        let mut ny = (e2z * e1x) - (e2x * e1z);
        let mut nz = (e2x * e1y) - (e2y * e1x);
        let l2 = ((nx * nx) + (ny * ny)) + (nz * nz);
        if l2 > C_1E8 {
            let y = rsqrt(l2);
            nx = nx * y;
            ny = ny * y;
            nz = nz * y;
        }
        o.f20 = [nx, ny, nz];
    } else {
        let a = &ps[o.f0];
        o.f2c = a.f0;
        let (qx, qy, qz, qw) = (a.f20[0], a.f20[1], a.f20[2], a.f20[3]);
        let z2 = qz + qz;
        let my2 = (-qy) + (-qy);
        let nx = ((qy * my2) - (qz * z2)) + 1.0;
        let ny = (qw * z2) - (qx * my2);
        let nz = (qw * my2) + (qx * z2);
        o.f20 = [nx, ny, nz]; // = rot(q, +X)
    }
    let (k, n, p) = (o.f60, o.f20, o.f2c);
    o.f74 = [(k * n[0]) + p[0], (k * n[1]) + p[1], (k * n[2]) + p[2]];
}

/// 0x143ae62b0 (rcx = o, edx = type, r8 = A, r9 = B, [rsp+0x28] = C): constructor
pub fn fn_143ae62b0(ps: &[Particle], ty: i32, a: usize, b: Option<usize>, c: Option<usize>) -> CollObj {
    let mut o = CollObj { f0: a, f8: b, f10: c, f18: ty, f1c: -1, f20: [0.0; 3], f2c: [0.0; 3], f38: 0, f3c: [-1; 8], f5c: 0.009999999776482582_f64 as f32, f60: 0.0, f64: 0.0, f68: [0.0; 3], f74: [0.0; 3], f80: 0.0 };
    fn_143b10190(ps, &mut o);
    let r;
    if ((ty - 2) as u32 & 0xFFFF_FFFD) == 0 {
        let (pa, pb) = (&ps[o.f0], &ps[o.f8.expect("capsule end")]);
        let (dx, dz, dy) = (pa.f0[0] - pb.f0[0], pa.f0[2] - pb.f0[2], pa.f0[1] - pb.f0[1]);
        let m = pa.f25c.max(pb.f25c); // fmaxf (import at 0x145C0E380)
        r = m + (sqrt(((dy * dy) + (dx * dx)) + (dz * dz)) * 0.5);
    } else if ((ty - 1) as u32 & 0xFFFF_FFFD) == 0 {
        r = ps[o.f0].f25c;
    } else if ty == 6 {
        fn_143afc140(ps, &mut o);
        let a = ps[o.f0].f0;
        let (dz, dy, dx) = (a[2] - o.f2c[2], a[1] - o.f2c[1], a[0] - o.f2c[0]);
        r = sqrt(((dy * dy) + (dx * dx)) + (dz * dz)) + o.f5c;
    } else {
        r = 0.0;
    }
    o.f80 = r;
    if o.f18 == 6 {
        fn_143afc140(ps, &mut o);
        o.f68 = o.f2c;
    }
    o
}

fn idx(i: i32) -> Option<usize> {
    if i == -1 {
        None
    } else {
        Some(i as usize)
    }
}

fn list(sc: &mut Scene, c: usize) -> &mut Vec<usize> {
    if sc.f70.len() <= c {
        sc.f70.resize(c + 1, Vec::new());
    }
    &mut sc.f70[c]
}

/// 0x143aec160 (rcx = scene, edx = a, r8d = b, r9d = c): add a dynamic-side object to scene+0x50
pub fn fn_143aec160(sc: &mut Scene, ia: i32, ib: i32, ic: i32) -> usize {
    let o = if ib == -1 {
        fn_143ae62b0(&sc.f0, 1, ia as usize, None, None)
    } else if ic == -1 {
        fn_143ae62b0(&sc.f0, 2, ia as usize, idx(ib), None)
    } else {
        fn_143ae62b0(&sc.f0, 6, ia as usize, idx(ib), idx(ic))
    };
    sc.objs.push(o);
    sc.f50.push(sc.objs.len() - 1);
    sc.objs.len() - 1
}

/// 0x143aec560 (rcx = scene, edx = group, r8d = a, r9d = b, [rsp+0x28] = c): collision plane into the group list
pub fn fn_143aec560(sc: &mut Scene, g: usize, ia: i32, ib: i32, ic: i32) -> usize {
    let o = if ib == -1 || ic == -1 { fn_143ae62b0(&sc.f0, 6, ia as usize, None, None) } else { fn_143ae62b0(&sc.f0, 6, ia as usize, idx(ib), idx(ic)) };
    sc.objs.push(o);
    let oi = sc.objs.len() - 1;
    list(sc, g).push(oi);
    oi
}

/// 0x143aec680 (rcx = scene, edx = type, r8d = a, r9d = b): collider into the list of particle a's group
pub fn fn_143aec680(sc: &mut Scene, ty: i32, ia: i32, ib: i32) -> usize {
    let g = sc.f0[ia as usize].f2b8;
    let o = if ib == -1 { fn_143ae62b0(&sc.f0, ty, ia as usize, None, None) } else { fn_143ae62b0(&sc.f0, ty, ia as usize, idx(ib), None) };
    sc.objs.push(o);
    let oi = sc.objs.len() - 1;
    list(sc, g).push(oi);
    oi
}

/// 0x143aedb50 (rcx = scene, edx = tag-1, r8 = o)
pub fn fn_143aedb50(sc: &mut Scene, tag_m1: usize, o: usize) {
    list(sc, 48 + tag_m1).push(o);
}

/// 0x143ae4440 (rcx = array, edx = count): stable sort of scene+0x50 by o.f1C ascending (signed in the exe;
/// the JS sorts the numbers as the asset file has them, where 0xFFFFFFFF is 4294967295: kept. No
/// dynamic-side object of Sora has such a batch number.)
pub fn fn_143ae4440(sc: &mut Scene) {
    let objs = &sc.objs;
    sc.f50.sort_by_key(|o| objs[*o].f1c); // `sort_by_key` is stable
}

// ---- setup fragments of 0x143b07550
pub fn init_collision_lists(sc: &mut Scene) {
    sc.objs = Vec::new();
    sc.f50 = Vec::new();
    sc.f60 = Vec::new();
    sc.f70 = vec![Vec::new(); 96];
}

/// A plane row of the asset.
#[derive(Clone, Debug)]
pub struct PlaneDesc {
    pub m_group_id: usize,
    pub m_particle_a: i32,
    pub m_particle_b: i32,
    pub m_particle_c: i32,
    pub m_thickness: f32,
    pub m_offset: f32,
    pub m_is_collision: bool,
    pub m_collision_batch_no: i64,
    pub m_col_id: [i32; 8],
}

pub fn setup_planes(core: &mut Core, descs: &[PlaneDesc], index_base: i32) {
    for d in descs {
        if d.m_is_collision {
            let o = fn_143aec560(&mut core.f3778, d.m_group_id, d.m_particle_a + index_base, d.m_particle_b + index_base, d.m_particle_c + index_base);
            core.f3778.objs[o].f38 = d.m_group_id;
            if (core.f744[d.m_group_id] as u32) > 0 {
                fn_143aedb50(&mut core.f3778, (core.f744[d.m_group_id] - 1) as usize, o);
            }
        } else {
            let o = fn_143aec160(&mut core.f3778, d.m_particle_a + index_base, d.m_particle_b + index_base, d.m_particle_c + index_base);
            let o = &mut core.f3778.objs[o];
            o.f38 = d.m_group_id;
            o.f1c = d.m_collision_batch_no;
            o.f60 = d.m_offset;
            o.f5c = d.m_thickness;
            o.f3c = d.m_col_id;
        }
    }
}

/// bodies: uses p.f190 / p.f198 as they are at that point of setup (from m_ParentId / m_ChildId)
pub fn setup_body_objects(core: &mut Core) {
    let n_id = if core.f658 > 0x960 { 8 } else { 4 };
    for i in 0..core.f3778.f0.len() {
        let sc = &mut core.f3778;
        let p = sc.f0[i].clone();
        if p.f277 != 0 {
            let s = p.f27c;
            let mut made = None;
            if ((s - 2) as u32 & 0xFFFF_FFFD) == 0 {
                let child = sc.f0[p.f198.expect("a capsule collider has a child body")].f24c;
                let o = fn_143aec680(sc, s, p.f24c as i32, child as i32);
                sc.objs[o].f64 = p.f284;
                made = Some(o);
            } else if ((s - 1) as u32 & 0xFFFF_FFFD) == 0 {
                made = Some(fn_143aec680(sc, s, p.f24c as i32, -1));
            } else if s == 6 {
                made = Some(fn_143aec560(sc, p.f2b8, p.f24c as i32, -1, -1));
            } else if s == 8 {
                let o = fn_143ae62b0(&sc.f0, 8, i, None, None);
                sc.objs.push(o);
                let oi = sc.objs.len() - 1;
                list(sc, p.f2b8).push(oi);
                made = Some(oi);
            }
            if made.is_some() {
                sc.f0[i].f2a0 = made;
            }
            if let Some(o) = sc.f0[i].f2a0 {
                sc.objs[o].f38 = p.f2b8;
                let tag = core.f744[p.f2b8];
                if tag != 0 {
                    list(sc, (tag + 47) as usize).push(o);
                }
            }
        } else {
            let mut any = false;
            for k in 0..n_id {
                if p.f2bc[k] != -1 {
                    any = true;
                    break;
                }
            }
            if !any {
                continue;
            }
            let s = p.f27c;
            if s == 2 {
                match p.f198 {
                    None => sc.f0[i].f2a0 = Some(fn_143aec160(sc, p.f24c as i32, -1, -1)),
                    Some(c) => {
                        if !(p.f276 != 0 && sc.f0[c].f276 != 0) {
                            let cc = sc.f0[c].f24c;
                            let o = fn_143aec160(sc, p.f24c as i32, cc as i32, -1);
                            sc.objs[o].f64 = p.f284;
                            sc.f0[i].f2a0 = Some(o);
                        }
                    }
                }
            } else if s == 1 && p.f276 == 0 {
                let alone = match p.f190 {
                    None => true,
                    Some(par) => sc.f0[par].f27c == 1 || (sc.f0[par].f27c == 2 && sc.f0[par].f198 != Some(i)),
                };
                if alone {
                    sc.f0[i].f2a0 = Some(fn_143aec160(sc, p.f24c as i32, -1, -1));
                }
            }
            if let Some(o) = sc.f0[i].f2a0 {
                sc.objs[o].f38 = p.f2b8;
                sc.objs[o].f1c = p.f280;
            }
        }
    }
}

/// The collision fields of a link row.
#[derive(Clone, Copy, Debug)]
pub struct LinkCollisionDesc {
    pub m_parent_id: i32,
    pub m_child_id: i32,
    pub m_is_self_collision: bool,
    pub m_is_collision_receiver: bool,
    pub m_is_collision_repulsor: bool,
    pub m_collision_batch_no: i64,
    pub m_concave_ratio: f32,
}

/// per LinkDesc d, called where 0x143b07550 handles it. link = the link record just created by 0x143aec770 (or none).
pub fn setup_link_collision(core: &mut Core, d: &LinkCollisionDesc, link: Option<usize>, index_base: i32) {
    let sc = &mut core.f3778;
    let (ia, ib) = (d.m_parent_id + index_base, d.m_child_id + index_base);
    if d.m_is_self_collision {
        let o = fn_143ae62b0(&sc.f0, 9, ia as usize, idx(ib), None);
        sc.objs.push(o);
        sc.f60.push(sc.objs.len() - 1);
        return;
    }
    if let (true, Some(link)) = (d.m_is_collision_receiver, link) {
        if sc.f0[ia as usize].f198 == Some(ib as usize) && sc.f0[ia as usize].f27c == 2 {
            return; // jumps past the repulsor part as well
        }
        let o = fn_143aec160(sc, ia, ib, -1);
        sc.f40[link].f30 = Some(o);
        sc.objs[o].f1c = d.m_collision_batch_no;
        sc.objs[o].f64 = d.m_concave_ratio;
    }
    if d.m_is_collision_repulsor {
        let o = fn_143aec680(sc, 2, ia, ib);
        sc.objs[o].f64 = d.m_concave_ratio;
        let tag = core.f744[sc.objs[o].f38];
        if tag != 0 {
            fn_143aedb50(sc, (tag - 1) as usize, o);
        }
    }
}

/// end of setup: sort scene+0x50
pub fn setup_finish_collision(core: &mut Core) {
    fn_143ae4440(&mut core.f3778);
}

/// core+0x400: per bone "driver particle index + 1" (0 none, 9999 static) - end of 0x143b07550
pub fn setup_bone_map(core: &mut Core, n_bones: usize) {
    let mut m = vec![0usize; n_bones];
    let ps = &core.f3778.f0;
    for (i, p) in ps.iter().enumerate() {
        if p.f276 == 0 {
            m[p.f248] = i + 1;
        } else if p.f277 == 0 {
            match p.f198 {
                Some(c) if ps[c].f276 == 0 => m[p.f248] = i + 1,
                _ => {
                    if m[p.f248] == 0 {
                        m[p.f248] = 9999;
                    }
                }
            }
        }
    }
    core.f400 = m;
}

// ---------------------------------------------------------------------------------------------------------------
/// 0x143b1b9f0 (rcx = p, rdx = plane object o): particle vs plane
pub fn fn_143b1b9f0(ps: &mut [Particle], pi: usize, o: &CollObj) {
    if pi == o.f0 || Some(pi) == o.f8 || Some(pi) == o.f10 {
        return;
    }
    let (n, q) = (o.f20, o.f2c);
    let p = &mut ps[pi];
    let d = (((p.f0[1] - q[1]) * n[1]) + ((p.f0[0] - q[0]) * n[0])) + ((p.f0[2] - q[2]) * n[2]);
    let r = p.f25c;
    if d >= r {
        return;
    }
    let pen = r - d;
    let (cx, cy, cz) = (n[0] * pen, n[1] * pen, n[2] * pen);
    if p.f276 != 0 {
        return;
    }
    p.f0[0] = cx + p.f0[0];
    p.f0[2] = cz + p.f0[2];
    p.f0[1] = cy + p.f0[1];
    p.f279 = 1;
    p.f64 = [cx + p.f64[0], cy + p.f64[1], cz + p.f64[2]];
    let (vy, vz, vx) = (p.f0[1] - p.fc[1], p.f0[2] - p.fc[2], p.f0[0] - p.fc[0]);
    let vn = ((vx * n[0]) + (vy * n[1])) + (vz * n[2]);
    let (tx, ty, tz) = (vx - (n[0] * vn), vy - (n[1] * vn), vz - (n[2] * vn));
    let fr = p.f268;
    let lim = fr * pen;
    let tt = ((ty * ty) + (tx * tx)) + (tz * tz);
    if tt >= lim * lim {
        p.f0[0] = p.f0[0] - (tx * fr);
        p.f0[1] = p.f0[1] - (fr * ty);
        p.f0[2] = p.f0[2] - (fr * tz);
    } else {
        p.f0[0] = p.f0[0] - tx;
        p.f0[1] = p.f0[1] - ty;
        p.f0[2] = p.f0[2] - tz;
    }
}

#[inline]
fn min1(s: f32) -> f32 {
    if s < 1.0 {
        s
    } else {
        1.0
    }
}

/// 0x143b1c700 (rcx = p1, rdx = p2, r8 = off vec3*, xmm3 = R): sphere vs sphere
pub fn fn_143b1c700(ps: &mut [Particle], p1: usize, p2: usize, off: V3, mut r: f32) {
    if !(r >= C_1E4) {
        r = ps[p2].f25c;
    }
    let sum = ps[p1].f25c + r;
    let (p2x, p2y, p2z, p1y, p1z) = (ps[p2].f0[0], ps[p2].f0[1], ps[p2].f0[2], ps[p1].f0[1], ps[p1].f0[2]);
    let mut nx = ps[p1].f0[0] - (p2x + off[0]);
    let mut ny = p1y - (p2y + off[1]);
    let mut nz = p1z - (p2z + off[2]);
    let dd = ((ny * ny) + (nx * nx)) + (nz * nz);
    if dd >= sum * sum {
        return;
    }
    if !(dd > 0.0) {
        return;
    }
    let k1 = ps[p1].f276;
    let l = sqrt(dd);
    let mut pen = sum - l;
    let inv = 1.0 / l;
    ny = ny * inv;
    nz = nz * inv;
    nx = nx * inv;
    if k1 == 0 && ps[p2].f276 == 0 {
        pen = pen * 0.5;
    }
    let (mut vx, mut vy, mut vz) = (Z[0], Z[1], Z[2]);
    if k1 == 0 {
        vx = vx + (ps[p1].f0[0] - ps[p1].fc[0]);
        vy = vy + (p1y - ps[p1].fc[1]);
        vz = vz + (p1z - ps[p1].fc[2]);
    }
    if ps[p2].f276 == 0 {
        vx = vx - (p2x - ps[p2].fc[0]);
        vy = vy - (p2y - ps[p2].fc[1]);
        vz = vz - (p2z - ps[p2].fc[2]);
    }
    let vn = ((nx * vx) + (ny * vy)) + (nz * vz);
    let (tx, ty, tz) = (vx - (nx * vn), vy - (ny * vn), vz - (nz * vn));
    let tt = ((ty * ty) + (tx * tx)) + (tz * tz);
    if k1 == 0 {
        let (cy, cx, cz) = (ny * pen, nx * pen, nz * pen);
        ps[p1].f0[0] = cx + ps[p1].f0[0];
        ps[p1].f0[2] = cz + ps[p1].f0[2];
        let y1 = cy + ps[p1].f0[1];
        ps[p1].f0[1] = y1;
        ps[p1].f279 = 1;
        ps[p1].f64 = [cx + ps[p1].f64[0], cy + ps[p1].f64[1], cz + ps[p1].f64[2]];
        let g = pen * ps[p1].f268;
        if tt >= g * g {
            if tt > 0.0 {
                let s = min1(g / sqrt(tt));
                ps[p1].f0[0] = ps[p1].f0[0] - (s * tx);
                ps[p1].f0[1] = ps[p1].f0[1] - (ty * s);
                ps[p1].f0[2] = ps[p1].f0[2] - (tz * s);
            }
        } else {
            ps[p1].f0[1] = y1 - ty;
            ps[p1].f0[0] = ps[p1].f0[0] - tx;
            ps[p1].f0[2] = ps[p1].f0[2] - tz;
        }
    }
    if ps[p2].f276 == 0 {
        let (cx, cy, cz) = (nx * pen, ny * pen, nz * pen);
        ps[p2].f0[0] = ps[p2].f0[0] - cx;
        ps[p2].f0[1] = ps[p2].f0[1] - cy;
        ps[p2].f0[2] = ps[p2].f0[2] - cz;
        ps[p2].f279 = 1;
        ps[p2].f64 = [ps[p2].f64[0] - cx, ps[p2].f64[1] - cy, ps[p2].f64[2] - cz];
        let g = pen * ps[p2].f268;
        if tt >= g * g {
            if !(tt > 0.0) {
                return;
            }
            let s = min1(g / sqrt(tt));
            ps[p2].f0[0] = (s * tx) + ps[p2].f0[0];
            ps[p2].f0[2] = (tz * s) + ps[p2].f0[2];
            ps[p2].f0[1] = (ty * s) + ps[p2].f0[1];
        } else {
            ps[p2].f0[0] = tx + ps[p2].f0[0];
            ps[p2].f0[2] = tz + ps[p2].f0[2];
            ps[p2].f0[1] = ty + ps[p2].f0[1];
        }
    }
}

/// radius of a capsule at parameter t with concave ratio c (inlined in 0x143b1b110 / 0x143b15850)
pub fn cap_radius(t: f32, r_a: f32, r_b: f32, c: f32) -> f32 {
    let mut r = ((1.0 - t) * r_a) + (t * r_b);
    if c.abs() > C_1E4 {
        let u = t - 0.5;
        r = ((((u * 4.0) * u) * r) * c) + ((1.0 - c) * r);
    }
    r
}

/// 0x143b1b110 (rcx = p, rdx = a, r8 = b, xmm3 = concave, [rsp+0x28] = off vec3*, [rsp+0x30] = R): sphere p vs capsule a-b
pub fn fn_143b1b110(ps: &mut [Particle], p: usize, a: usize, b: usize, concave: f32, off: V3, mut r: f32) {
    if p == a || p == b {
        return;
    }
    let (ax, ay, az) = (off[0] + ps[a].f0[0], off[1] + ps[a].f0[1], off[2] + ps[a].f0[2]);
    let (ex, ey, ez) = ((off[0] + ps[b].f0[0]) - ax, (off[1] + ps[b].f0[1]) - ay, (off[2] + ps[b].f0[2]) - az);
    let mut t = (((ps[p].f0[1] - ay) * ey) + ((ps[p].f0[0] - ax) * ex)) + ((ps[p].f0[2] - az) * ez);
    let ee = ((ey * ey) + (ex * ex)) + (ez * ez);
    if t > 0.0 {
        if ee > t {
            t = t / ee;
        } else {
            t = 1.0;
        }
    } else {
        t = 0.0;
    }
    let (dx, dy, dz) = (ps[p].f0[0] - ((ex * t) + ax), ps[p].f0[1] - ((ey * t) + ay), ps[p].f0[2] - ((ez * t) + az));
    let dd = ((dy * dy) + (dx * dx)) + (dz * dz);
    if !(r >= C_1E4) {
        r = ((1.0 - t) * ps[a].f25c) + (t * ps[b].f25c);
        if concave.abs() > C_1E4 {
            let u = t - 0.5;
            r = ((((u * 4.0) * u) * r) * concave) + ((1.0 - concave) * r);
        }
    }
    let sum = r + ps[p].f25c;
    if dd >= sum * sum {
        return;
    }
    if !(dd > 0.0) {
        return;
    }
    let k = ps[p].f276;
    let l = sqrt(dd);
    let mut pen = sum - l;
    let inv = 1.0 / l;
    let (nx, ny, nz) = (inv * dx, inv * dy, inv * dz);
    if k == 0 {
        if ps[a].f276 == 0 {
            pen = pen * 0.5;
        }
    } else {
        pen = pen / ((0.5 - t).abs() + 0.5);
    }
    let (mut vx, mut vy, mut vz) = (Z[0], Z[1], Z[2]);
    if k == 0 {
        vx = vx + (ps[p].f0[0] - ps[p].fc[0]);
        vy = vy + (ps[p].f0[1] - ps[p].fc[1]);
        vz = vz + (ps[p].f0[2] - ps[p].fc[2]);
    }
    if ps[a].f276 == 0 {
        let w = 1.0 - t;
        vx = vx - (((ps[a].f0[0] - ps[a].fc[0]) * w) + ((ps[b].f0[0] - ps[b].fc[0]) * t));
        vy = vy - (((ps[a].f0[1] - ps[a].fc[1]) * w) + ((ps[b].f0[1] - ps[b].fc[1]) * t));
        vz = vz - (((ps[a].f0[2] - ps[a].fc[2]) * w) + ((ps[b].f0[2] - ps[b].fc[2]) * t));
    }
    let vn = ((vy * ny) + (vx * nx)) + (vz * nz);
    let (mut tx, mut ty, mut tz) = (vx - (nx * vn), vy - (ny * vn), vz - (nz * vn));
    let tt = ((ty * ty) + (tx * tx)) + (tz * tz);
    if k == 0 {
        let (cy, cx, cz) = (ny * pen, nx * pen, nz * pen);
        ps[p].f0[0] = cx + ps[p].f0[0];
        ps[p].f0[2] = cz + ps[p].f0[2];
        let y1 = cy + ps[p].f0[1];
        ps[p].f0[1] = y1;
        ps[p].f279 = 1;
        ps[p].f64 = [cx + ps[p].f64[0], cy + ps[p].f64[1], cz + ps[p].f64[2]];
        let g = pen * ps[p].f268;
        if tt >= g * g {
            if tt > 0.0 {
                let s = min1(g / sqrt(tt));
                ps[p].f0[0] = ps[p].f0[0] - (s * tx);
                ps[p].f0[1] = ps[p].f0[1] - (ty * s);
                ps[p].f0[2] = ps[p].f0[2] - (tz * s);
            }
        } else {
            ps[p].f0[1] = y1 - ty;
            ps[p].f0[0] = ps[p].f0[0] - tx;
            ps[p].f0[2] = ps[p].f0[2] - tz;
        }
    }
    if ps[a].f276 == 0 {
        let w = 1.0 - t;
        let mut g = w * pen;
        let (cx, cy, cz) = (nx * g, ny * g, nz * g);
        ps[a].f0[0] = ps[a].f0[0] - cx;
        ps[a].f0[1] = ps[a].f0[1] - cy;
        ps[a].f0[2] = ps[a].f0[2] - cz;
        ps[a].f279 = 1;
        ps[a].f64 = [ps[a].f64[0] - cx, ps[a].f64[1] - cy, ps[a].f64[2] - cz];
        g = g * ps[a].f268;
        if tt >= g * g {
            if tt > 0.0 {
                let s = min1(g / sqrt(tt));
                let (sx, sy) = (tx * s, s * ty);
                ps[a].f0[2] = (s * tz) + ps[a].f0[2];
                ps[a].f0[1] = sy + ps[a].f0[1];
                ps[a].f0[0] = sx + ps[a].f0[0];
            }
        } else {
            let (sx, sy) = (tx * w, ty * w);
            ps[a].f0[2] = (w * tz) + ps[a].f0[2];
            ps[a].f0[1] = sy + ps[a].f0[1];
            ps[a].f0[0] = sx + ps[a].f0[0];
        }
    }
    if ps[b].f276 == 0 {
        let mut g = pen * t;
        let (cx, cy, cz) = (nx * g, ny * g, nz * g);
        ps[b].f0[0] = ps[b].f0[0] - cx;
        ps[b].f0[1] = ps[b].f0[1] - cy;
        ps[b].f0[2] = ps[b].f0[2] - cz;
        ps[b].f279 = 1;
        ps[b].f64 = [ps[b].f64[0] - cx, ps[b].f64[1] - cy, ps[b].f64[2] - cz];
        g = g * ps[a].f268; // friction of a (rdx), as coded
        if tt >= g * g {
            if !(tt > 0.0) {
                return;
            }
            let s = min1(g / sqrt(tt));
            tx = tx * s;
            ty = ty * s;
            tz = tz * s;
        } else {
            tx = tx * t;
            ty = ty * t;
            tz = tz * t;
        }
        ps[b].f0[2] = tz + ps[b].f0[2];
        ps[b].f0[1] = ty + ps[b].f0[1];
        ps[b].f0[0] = tx + ps[b].f0[0];
    }
}

/// 0x143b15850: capsule a1-a2 vs capsule b1-b2 (off is added to b1/b2; R > 0 overrides the b radius)
#[allow(clippy::too_many_arguments)]
pub fn fn_143b15850(ps: &mut [Particle], a1: usize, a2: usize, c1: f32, b1: usize, b2: usize, c2: f32, off: V3, mut r: f32) {
    if a1 == b1 || a1 == b2 || b1 == a2 {
        return;
    }
    let a1x = ps[a1].f0[0];
    let (b1x, b1y, b1z) = (ps[b1].f0[0] + off[0], ps[b1].f0[1] + off[1], ps[b1].f0[2] + off[2]);
    let (d1x, d1y, d1z) = (ps[a2].f0[0] - a1x, ps[a2].f0[1] - ps[a1].f0[1], ps[a2].f0[2] - ps[a1].f0[2]);
    let (rx, ry, rz) = (a1x - b1x, ps[a1].f0[1] - b1y, ps[a1].f0[2] - b1z);
    let (d2x, d2y, d2z) = ((ps[b2].f0[0] + off[0]) - b1x, (ps[b2].f0[1] + off[1]) - b1y, (ps[b2].f0[2] + off[2]) - b1z);
    let a = ((d1y * d1y) + (d1x * d1x)) + (d1z * d1z);
    let b = ((d2y * d1y) + (d2x * d1x)) + (d2z * d1z);
    let e = ((d2y * d2y) + (d2x * d2x)) + (d2z * d2z);
    let c = ((ry * d1y) + (rx * d1x)) + (rz * d1z);
    let ff = ((ry * d2y) + (rx * d2x)) + (rz * d2z);
    let d = (e * a) - (b * b);
    let mut s_n: f32;
    let mut s_d = d;
    let mut t_n = ff;
    let mut t_d = d;
    if d >= C_1E9 {
        s_n = (ff * b) - (c * e);
        t_n = (ff * a) - (c * b);
        if s_n >= 0.0 {
            if s_n > s_d {
                s_n = s_d;
                t_n = ff + b;
                t_d = e;
            }
        } else {
            s_n = 0.0;
            t_n = ff;
            t_d = e;
        }
    } else {
        s_n = 0.0;
        s_d = 1.0;
        t_d = e;
    }
    if t_n >= 0.0 {
        if t_n > t_d {
            s_n = b - c;
            t_n = t_d;
            if s_n >= 0.0 {
                if s_n > a {
                    s_n = s_d;
                } else {
                    s_d = a;
                }
            } else {
                s_n = 0.0;
            }
        }
    } else {
        t_n = 0.0;
        if c > 0.0 {
            s_n = 0.0; // comiss c, -0.0 ; ja
        } else {
            s_n = -c;
            if s_n > a {
                s_n = s_d;
            } else {
                s_d = a;
            }
        }
    }
    let s = if s_n.abs() >= C_1E9 { s_n / s_d } else { 0.0 };
    let t = if t_n.abs() >= C_1E9 { t_n / t_d } else { 0.0 };
    let w1 = 1.0 - s;
    let px = ((d1x * s) + rx) - (d2x * t);
    let py = ((d1y * s) + ry) - (d2y * t);
    let pz = ((d1z * s) + rz) - (d2z * t);
    let dd = ((py * py) + (px * px)) + (pz * pz);
    let mut r1 = (s * ps[a2].f25c) + (w1 * ps[a1].f25c);
    if c1.abs() > C_1E4 {
        let u = s - 0.5;
        r1 = ((((u * 4.0) * u) * r1) * c1) + ((1.0 - c1) * r1);
    }
    if !(r >= C_1E4) {
        r = ((1.0 - t) * ps[b1].f25c) + (t * ps[b2].f25c);
        if c2.abs() > C_1E4 {
            let u = t - 0.5;
            r = ((((u * 4.0) * u) * r) * c2) + ((1.0 - c2) * r);
        }
    }
    let sum = r1 + r;
    if dd >= sum * sum {
        return;
    }
    if !(dd > 0.0) {
        return;
    }
    let k1 = ps[a1].f276;
    let l = sqrt(dd);
    let mut pen = sum - l;
    let inv = 1.0 / l;
    let (nx, ny, nz) = (inv * px, inv * py, inv * pz);
    if k1 == 0 && ps[b1].f276 == 0 {
        pen = pen * 0.5;
    }
    let k2 = ps[b1].f276;
    let w2 = 1.0 - t;
    let (mut g1, mut g2, mut g4, mut g3) = (w1 * pen, s * pen, t * pen, w2 * pen);
    if k2 != 0 {
        if k1 == 0 {
            g1 = g1 + g1;
            g2 = g2 + g2;
        }
    } else if k1 != 0 {
        g3 = g3 + g3;
        g4 = g4 + g4;
    }
    let (mut vx, mut vy, mut vz) = (Z[0], Z[1], Z[2]);
    if k1 == 0 {
        vx = vx + (((a1x - ps[a1].fc[0]) * w1) + ((ps[a2].f0[0] - ps[a2].fc[0]) * s));
        vy = vy + (((ps[a1].f0[1] - ps[a1].fc[1]) * w1) + ((ps[a2].f0[1] - ps[a2].fc[1]) * s));
        vz = vz + (((ps[a1].f0[2] - ps[a1].fc[2]) * w1) + ((ps[a2].f0[2] - ps[a2].fc[2]) * s));
    }
    if k2 == 0 {
        vx = vx - (((ps[b1].f0[0] - ps[b1].fc[0]) * w2) + ((ps[b2].f0[0] - ps[b2].fc[0]) * t));
        vy = vy - (((ps[b1].f0[1] - ps[b1].fc[1]) * w2) + ((ps[b2].f0[1] - ps[b2].fc[1]) * t));
        vz = vz - (((ps[b1].f0[2] - ps[b1].fc[2]) * w2) + ((ps[b2].f0[2] - ps[b2].fc[2]) * t));
    }
    let vn = ((vy * ny) + (vx * nx)) + (vz * nz);
    let (mut tx, mut ty, mut tz) = (vx - (vn * nx), vy - (vn * ny), vz - (vn * nz));
    let tt = ((ty * ty) + (tx * tx)) + (tz * tz);
    if k1 == 0 {
        let (cx, cy, cz) = (nx * g1, ny * g1, nz * g1);
        ps[a1].f0[0] = a1x + cx;
        ps[a1].f0[1] = cy + ps[a1].f0[1];
        ps[a1].f0[2] = cz + ps[a1].f0[2];
        ps[a1].f279 = 1;
        ps[a1].f64 = [cx + ps[a1].f64[0], cy + ps[a1].f64[1], cz + ps[a1].f64[2]];
        let g = g1 * ps[a1].f268;
        if tt >= g * g {
            if tt > 0.0 {
                let k = min1(g / sqrt(tt));
                ps[a1].f0[0] = ps[a1].f0[0] - (k * tx);
                ps[a1].f0[1] = ps[a1].f0[1] - (ty * k);
                ps[a1].f0[2] = ps[a1].f0[2] - (tz * k);
            }
        } else {
            ps[a1].f0[0] = ps[a1].f0[0] - (tx * w1);
            ps[a1].f0[1] = ps[a1].f0[1] - (ty * w1);
            ps[a1].f0[2] = ps[a1].f0[2] - (w1 * tz);
        }
    }
    if ps[a2].f276 == 0 {
        let (cx, cy, cz) = (nx * g2, ny * g2, nz * g2);
        ps[a2].f0[0] = cx + ps[a2].f0[0];
        ps[a2].f0[1] = cy + ps[a2].f0[1];
        ps[a2].f0[2] = cz + ps[a2].f0[2];
        ps[a2].f279 = 1;
        ps[a2].f64 = [cx + ps[a2].f64[0], cy + ps[a2].f64[1], cz + ps[a2].f64[2]];
        let g = g2 * ps[a1].f268; // friction of a1 (rcx), as coded
        if tt >= g * g {
            if tt > 0.0 {
                let k = min1(g / sqrt(tt));
                ps[a2].f0[0] = ps[a2].f0[0] - (k * tx);
                ps[a2].f0[1] = ps[a2].f0[1] - (ty * k);
                ps[a2].f0[2] = ps[a2].f0[2] - (tz * k);
            }
        } else {
            ps[a2].f0[0] = ps[a2].f0[0] - (tx * s);
            ps[a2].f0[1] = ps[a2].f0[1] - (ty * s);
            ps[a2].f0[2] = ps[a2].f0[2] - (tz * s);
        }
    }
    if ps[b1].f276 == 0 {
        let (cx, cy, cz) = (nx * g3, ny * g3, nz * g3);
        ps[b1].f0[0] = ps[b1].f0[0] - cx;
        ps[b1].f0[1] = ps[b1].f0[1] - cy;
        ps[b1].f0[2] = ps[b1].f0[2] - cz;
        ps[b1].f279 = 1;
        ps[b1].f64 = [ps[b1].f64[0] - cx, ps[b1].f64[1] - cy, ps[b1].f64[2] - cz];
        let g = g3 * ps[b1].f268;
        if tt >= g * g {
            if tt > 0.0 {
                let k = min1(g / sqrt(tt));
                ps[b1].f0[0] = (tx * k) + ps[b1].f0[0];
                ps[b1].f0[1] = (ty * k) + ps[b1].f0[1];
                ps[b1].f0[2] = (tz * k) + ps[b1].f0[2];
            }
        } else {
            ps[b1].f0[1] = (ty * w2) + ps[b1].f0[1];
            ps[b1].f0[0] = (w2 * tx) + ps[b1].f0[0];
            ps[b1].f0[2] = (tz * w2) + ps[b1].f0[2];
        }
    }
    if ps[b2].f276 == 0 {
        let (cx, cy, cz) = (nx * g4, ny * g4, nz * g4);
        ps[b2].f0[0] = ps[b2].f0[0] - cx;
        ps[b2].f0[1] = ps[b2].f0[1] - cy;
        ps[b2].f0[2] = ps[b2].f0[2] - cz;
        ps[b2].f279 = 1;
        ps[b2].f64 = [ps[b2].f64[0] - cx, ps[b2].f64[1] - cy, ps[b2].f64[2] - cz];
        let g = g4 * ps[b2].f268;
        if tt >= g * g {
            if !(tt > 0.0) {
                return;
            }
            let k = min1(g / sqrt(tt));
            tx = tx * k;
            ty = ty * k;
            tz = tz * k;
        } else {
            tx = tx * t;
            ty = ty * t;
            tz = tz * t;
        }
        ps[b2].f0[2] = tz + ps[b2].f0[2];
        ps[b2].f0[1] = ty + ps[b2].f0[1];
        ps[b2].f0[0] = tx + ps[b2].f0[0];
    }
}

/// bound-sphere reject shared by the object-object tests: true = overlapping
pub fn bounds_overlap(s: &CollObj, c: &CollObj) -> bool {
    let (dy, dx, dz) = (s.f74[1] - c.f74[1], s.f74[0] - c.f74[0], s.f74[2] - c.f74[2]);
    let rr = s.f80 + c.f80;
    !(((dy * dy) + (dx * dx)) + (dz * dz) >= rr * rr)
}

/// 0x143b13a20 (rcx = S sphere object, rdx = collider C); jump table 0x143b13b50
pub fn fn_143b13a20(ps: &mut [Particle], s: &CollObj, c: &CollObj) {
    match c.f18 {
        1 => fn_143b1c700(ps, s.f0, c.f0, [0.0; 3], 0.0),
        2 => {
            if bounds_overlap(s, c) {
                fn_143b1b110(ps, s.f0, c.f0, c.f8.expect("capsule end"), c.f64, [0.0; 3], 0.0);
            }
        }
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 0x143b1bc20 / 0x143b16420 (inverse sphere / capsule): no contact
        3 | 4 => super::unported(8),
        6 => fn_143b1b9f0(ps, s.f0, c),
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 0x143b1aec0 (box): no contact
        8 => super::unported(9),
        _ => {}
    }
}

/// 0x143b13620 (rcx = S capsule object, rdx = collider C); jump table 0x143b138dc
pub fn fn_143b13620(ps: &mut [Particle], s: &CollObj, c: &CollObj) {
    let s8 = s.f8.expect("capsule end");
    match c.f18 {
        1 => {
            if bounds_overlap(s, c) {
                fn_143b1b110(ps, c.f0, s.f0, s8, s.f64, [0.0; 3], 0.0);
            }
        }
        2 => {
            if bounds_overlap(s, c) {
                let c8 = c.f8.expect("capsule end");
                fn_143b1b110(ps, s.f0, c.f0, c8, c.f64, [0.0; 3], 0.0);
                fn_143b1b110(ps, s8, c.f0, c8, c.f64, [0.0; 3], 0.0);
                fn_143b15850(ps, s.f0, s8, s.f64, c.f0, c8, c.f64, [0.0; 3], 0.0);
            }
        }
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 0x143b1bc20 / 0x143b16420 (inverse sphere / capsule): no contact
        3 | 4 => super::unported(8),
        6 => {
            fn_143b1b9f0(ps, s.f0, c);
            fn_143b1b9f0(ps, s8, c);
        }
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 0x143b1aec0 (box): no contact
        8 => super::unported(9),
        _ => {}
    }
}

/// 0x143b13900 (rcx = S plane object, rdx = collider C)
pub fn fn_143b13900(ps: &mut [Particle], s: &CollObj, c: &CollObj) {
    let t = c.f18;
    if t == 1 {
        if bounds_overlap(s, c) {
            fn_143b1d720(ps, s, c.f0);
        }
    } else if t == 2 {
        if bounds_overlap(s, c) {
            fn_143b1cb50(ps, s, c.f0, c.f8.expect("capsule end"));
        }
    } else if t == 6 {
        fn_143b1b9f0(ps, s.f0, c);
        fn_143b1b9f0(ps, s.f8.expect("plane particle B"), c);
        fn_143b1b9f0(ps, s.f10.expect("plane particle C"), c);
    }
}

/// `state`: the per-group state array when colliders whose group state < 2 are skipped.
fn vs_list(sc: &mut Scene, si: usize, c: usize, state: Option<&[i32]>) {
    let s = sc.objs[si];
    let t = s.f18;
    if t != 1 && t != 2 && t != 6 {
        return;
    }
    let n = sc.f70.get(c).map_or(0, |l| l.len());
    for i in 0..n {
        let cc = sc.objs[sc.f70[c][i]];
        if let Some(state) = state {
            if !(state[cc.f38] >= 2) {
                continue;
            }
        }
        let ps = &mut sc.f0;
        let (a, b, d) = (cc.f0, cc.f8, cc.f10);
        let sa = ps[a].f276;
        ps[a].f276 = 1;
        let sb = b.map(|b| {
            let v = ps[b].f276;
            ps[b].f276 = 1;
            v
        });
        let sd = d.map(|d| {
            let v = ps[d].f276;
            ps[d].f276 = 1;
            v
        });
        if t == 1 {
            fn_143b13a20(ps, &s, &cc);
        } else if t == 2 {
            fn_143b13620(ps, &s, &cc);
        } else {
            fn_143b13900(ps, &s, &cc);
        }
        ps[a].f276 = sa;
        if let (Some(b), Some(sb)) = (b, sb) {
            ps[b].f276 = sb;
        }
        if let (Some(d), Some(sd)) = (d, sd) {
            ps[d].f276 = sd;
        }
    }
}

/// 0x143ae4ef0: S vs every collider of a group list (colliders forced kinematic)
pub fn fn_143ae4ef0(sc: &mut Scene, s: usize, c: usize) {
    vs_list(sc, s, c, None);
}

/// 0x143ae4c30: same, skipping colliders whose group state < 2
pub fn fn_143ae4c30(sc: &mut Scene, s: usize, c: usize, state: &[i32]) {
    vs_list(sc, s, c, Some(state));
}

/// 0x143b1aba0 (rcx = scene): self-collision pairs (scene+0x60)
pub fn fn_143b1aba0(sc: &mut Scene) {
    for i in 0..sc.f60.len() {
        let o = sc.objs[sc.f60[i]];
        let a = o.f0;
        let Some(b) = o.f8 else { continue };
        let ps = &mut sc.f0;
        let (sa, sb) = (ps[a].f27c, ps[b].f27c);
        let z = [0.0f32; 3];
        if sa == 1 {
            if sb == 1 {
                fn_143b1c700(ps, a, b, z, 0.0);
            } else if sb == 2 {
                if let Some(bc) = ps[b].f198 {
                    let cv = ps[b].f284;
                    fn_143b1b110(ps, a, b, bc, cv, z, 0.0);
                }
            } else if sb == 6 {
                if let Some(bo) = ps[b].f2a0 {
                    let plane = sc.objs[bo];
                    fn_143b1b9f0(ps, a, &plane);
                }
            }
        } else if sa == 2 {
            if sb == 1 {
                if let Some(ac) = ps[a].f198 {
                    let cv = ps[a].f284;
                    fn_143b1b110(ps, b, a, ac, cv, z, 0.0);
                }
            } else if sb == 2 {
                if let (Some(ac), Some(bc)) = (ps[a].f198, ps[b].f198) {
                    let (ca, cb) = (ps[a].f284, ps[b].f284);
                    fn_143b15850(ps, a, ac, ca, b, bc, cb, z, 0.0);
                }
            } else if sb == 6 {
                if let Some(bo) = ps[b].f2a0 {
                    let plane = sc.objs[bo];
                    fn_143b1b9f0(ps, a, &plane);
                    if let Some(ac) = ps[a].f198 {
                        fn_143b1b9f0(ps, ac, &plane);
                    }
                }
            }
        } else if sa == 6 {
            if sb == 1 {
                if let Some(ao) = ps[a].f2a0 {
                    let plane = sc.objs[ao];
                    fn_143b1b9f0(ps, b, &plane);
                }
            } else if sb == 2 {
                if let Some(ao) = ps[a].f2a0 {
                    let plane = sc.objs[ao];
                    fn_143b1b9f0(ps, b, &plane);
                    if let Some(bc) = ps[b].f198 {
                        fn_143b1b9f0(ps, bc, &plane);
                    }
                }
            }
        }
    }
}

/// 0x143b1a5c0 (rcx = core): colliders of other Bonamik cores (list core+0x3748, count core+0x3750)
pub fn fn_143b1a5c0(core: &Core) {
    if core.f3750 == 0 {
        return;
    }
    // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 0x143b1a665..0x143b1ab03 (external core colliders): none
    super::unported(5);
}

/// 0x143b12da0 (rcx = p, rdx = point vec3*, r8 = normal vec3*): ground plane clamp; returns 1 if the particle was moved
pub fn fn_143b12da0(p: &mut Particle, pt: V3, n: V3) -> i32 {
    let d = (((p.f0[1] - pt[1]) * n[1]) + ((p.f0[0] - pt[0]) * n[0])) + ((p.f0[2] - pt[2]) * n[2]);
    let r = p.f25c;
    if d >= r {
        return 0;
    }
    let (ox, oy, oz) = (p.f0[0], p.f0[1], p.f0[2]);
    let pen = r - d;
    let (cx, cy, cz) = (n[0] * pen, n[1] * pen, n[2] * pen);
    p.f0[0] = cx + ox;
    p.f0[2] = cz + p.f0[2];
    p.f0[1] = cy + p.f0[1];
    p.f64 = [cx + p.f64[0], cy + p.f64[1], cz + p.f64[2]];
    let (my, mz, mx) = (p.f0[1] - oy, p.f0[2] - oz, p.f0[0] - ox);
    let mn = ((mx * n[0]) + (my * n[1])) + (mz * n[2]);
    let (tx, ty, tz) = (mx - (n[0] * mn), my - (n[1] * mn), mz - (n[2] * mn));
    let fr = p.f268;
    let lim = fr * pen;
    let tt = ((ty * ty) + (tx * tx)) + (tz * tz);
    let (y, z);
    if tt >= lim * lim {
        p.f0[0] = p.f0[0] - (tx * fr);
        y = p.f0[1] - (fr * ty);
        z = p.f0[2] - (fr * tz);
    } else {
        p.f0[0] = p.f0[0] - tx;
        y = p.f0[1] - ty;
        z = p.f0[2] - tz;
    }
    p.f0[2] = z;
    p.f0[1] = y;
    p.f70 = [(p.f0[0] - ox) + p.f70[0], (y - oy) + p.f70[1], (z - oz) + p.f70[2]];
    1
}

/// 0x143b19d80 (rcx = core, rdx = scene, r8b = queryOk, r9 = pos vec3*, [rsp+0x28] = normal vec3*)
pub fn fn_143b19d80(core: &mut Core, query_ok: bool, pos: V3, normal: V3) {
    let f7ac = core.f3778.f7ac;
    for p in core.f3778.f0.iter_mut() {
        if p.f276 != 0 {
            continue;
        }
        if core.f924[p.f2b8] == 0 {
            continue;
        }
        if p.f31c != 0 && f7ac == 0 {
            // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 0x143b19e6b..0x143b1a3ca (ground-attach sphere sweep): the plain ground clamp below
            super::unported(6);
        }
        if !query_ok {
            continue;
        }
        fn_143b12da0(p, pos, normal);
        p.f88 = [p.f88[0] + normal[0], normal[1] + p.f88[1], normal[2] + p.f88[2]];
    }
}

/// What the ground query fragment of Step returns: `valid` = r12b, `ok` = [rsp+0x51].
#[derive(Clone, Copy, Debug)]
pub struct Ground {
    pub valid: bool,
    pub ok: bool,
    pub pos: V3,
    pub normal: V3,
}

/// Fragment of Step 0x143b0cb60 (143b0cc4c..143b0ce5d): ground query + world -> component conversion.
pub fn step_ground_query(core: &Core) -> Ground {
    let mut pos = Z;
    let mut normal: V3 = [0.0, 0.0, 1.0];
    // owner.vf10(posOut, normalOut)
    let ok = match core.f3848.ground {
        Some((point, n)) => {
            pos = point;
            normal = n;
            true
        }
        None => false,
    };
    let mut valid = core.f3718 != 0;
    let (mut pc, mut nc) = (pos, normal);
    if ok {
        valid = true;
        let (q, s, t) = (core.fd0, core.ff0, core.fe0);
        pc = add(qrot(q, [pos[0] * s[0], pos[1] * s[1], pos[2] * s[2]]), t);
        nc = qrot(q, normal);
    }
    Ground { valid, ok, pos: pc, normal: nc }
}

/// Fragment of Step 0x143b0cb60 (143b0d65b..143b0d7ed): the collision iterations of one sub-step
pub fn step_collision(core: &mut Core, g: &Ground) {
    for _it in 0..core.f680 {
        fn_143b1a5c0(core);
        let sc = &mut core.f3778;
        for c in 0..48 {
            for i in 0..sc.f70.get(c).map_or(0, |l| l.len()) {
                let oi = sc.f70[c][i];
                let mut o = sc.objs[oi];
                if o.f18 == 6 {
                    fn_143afc140(&sc.f0, &mut o);
                } else {
                    fn_143b10190(&sc.f0, &mut o);
                }
                sc.objs[oi] = o;
            }
        }
        for i in 0..sc.f50.len() {
            let si = sc.f50[i];
            let mut s = sc.objs[si];
            if core.f804[s.f38] < 2 {
                continue;
            }
            if s.f18 == 6 {
                fn_143afc140(&sc.f0, &mut s);
            } else {
                fn_143b10190(&sc.f0, &mut s);
            }
            sc.objs[si] = s;
            for k in 0..8 {
                let c = if s.f18 == 6 { s.f3c[k] } else { sc.f0[s.f0].f2bc[k] }; // re-evaluated per k as in the exe
                if c == -1 {
                    continue;
                }
                if c < 48 {
                    if core.f804[c as usize] >= 2 {
                        fn_143ae4ef0(sc, si, c as usize);
                    }
                } else {
                    fn_143ae4c30(sc, si, c as usize, &core.f804);
                }
            }
        }
        fn_143b1aba0(sc);
        if g.valid && core.f65d != 0 {
            fn_143b19d80(core, g.ok, g.pos, g.normal);
        }
    }
}

/// 0x143b10230 (rcx = core, xmm1 = dt): Attach. Part 1: multiple-collision-object particles follow their driver particle.
pub fn fn_143b10230(core: &mut Core, _dt: f32) {
    let ps = &mut core.f3778.f0;
    for i in 0..ps.len() {
        if ps[i].f278 == 0 {
            continue;
        }
        let m = core.f400[ps[i].f248];
        if m == 0 || m == 9999 {
            continue;
        }
        let (q20, q0) = (ps[m - 1].f20, ps[m - 1].f0);
        ps[i].f0 = add(qrot(q20, ps[i].f170), q0);
        ps[i].f20 = qmul(q20, ps[i].f180);
    }
    if core.f37e8 > 0 {
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 0x143b104ef..0x143b107fb (ground attach list): none
        super::unported(7);
    }
}
