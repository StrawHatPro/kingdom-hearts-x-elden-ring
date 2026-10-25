//! `tools/lib/bonamik/wind.js`: Integrate, Aero, the xorshift stream, wind assembly, Anchors,
//! SphereSnap, LateralLinks, VelocityUpdate. Transcribed from the disassembly of KH3.exe.
//!
//! `sinf` (CRT import, thunk 0x1458719A0) is computed in double and rounded, as the JS does.

use super::m::{qrot, rsqrt, sqrt, V3};
use super::records::{Core, Particle, Scene};

const C_0_01: f32 = 0.01_f64 as f32; // 0x145CADFC4 (0x3c23d70a)
const C_1EM8: f32 = 1e-8_f64 as f32; // 0x145C9C488 (0x322bcc77)
/// 0x14750BA88 (0x322bcc76) - one ulp below, used once in VelocityUpdate (a double in the JS)
const C_1EM8_LO: f64 = 9.99999905104687e-9;
const C_0_05: f32 = 0.05_f64 as f32; // 0x145C9C4BC (0x3d4ccccd)
const C_0_7: f32 = 0.7_f64 as f32; // 0x145D11AB0 (0x3f333333)
const C_PI: f32 = 3.1415927410125732_f64 as f32; // 0x145C9C4F8 (0x40490fdb)
const C_2M24: f32 = 5.960464477539063e-8_f64 as f32; // 0x145C87CFC (0x33800000) = 2^-24
const C_0_1: f32 = 0.1_f64 as f32; // 0x145C87D00 (0x3dcccccd)
const C_0_96: f32 = 0.96_f64 as f32; // 0x14687B7F4 (0x3f75c28f)
const C_0_98: f32 = 0.98_f64 as f32; // 0x145C9C4E4 (0x3f7ae148)
const C_M0_01: f32 = -0.01_f64 as f32; // 0x145CD7804 (0xbc23d70a)
const ZERO: V3 = [0.0, 0.0, 0.0];

/// The seed of the JS's `makeRng()` (the per-thread initial state of 0x142cff460).
pub const DEFAULT_SEED: [u32; 4] = [0x075BCD15, 0x159A55E5, 0x1F123BB5, 0x05491333];

/// The random stream of Integrate. 0x142cff460 returns a pointer to a thread-local uint32[4];
/// the update is inlined at every use site (Marsaglia xorshift128).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rng {
    /// `zeroJitter`: every draw is 0x80000000, which makes the jitter 0.
    Zero,
    /// `makeRng(seed)`: xorshift128.
    Xorshift([u32; 4]),
    /// A logged sequence of draws (three per dynamic particle per sub-step, in particle order);
    /// once it is exhausted every draw is 0x80000000.
    Logged { draws: Vec<u32>, at: usize },
}

impl Rng {
    /// The new w (uint32).
    pub fn next(&mut self) -> u32 {
        match self {
            Rng::Zero => 0x8000_0000,
            Rng::Xorshift(s) => {
                let t = s[0] ^ (s[0] << 11);
                s[0] = s[1];
                s[1] = s[2];
                let w = s[3];
                s[2] = w;
                let r = (((w >> 11) ^ t) >> 8) ^ t ^ w;
                s[3] = r;
                r
            }
            Rng::Logged { draws, at } => {
                let value = draws.get(*at).copied().unwrap_or(0x8000_0000);
                *at += 1;
                value
            }
        }
    }
}

/// conversion used by Integrate: (float)(w >> 8) * 2^-24 - 0.5 -> [-0.5, 0.5), 24-bit resolution
pub fn rng_to_jitter(w: u32) -> f32 {
    (((w >> 8) as f32) * C_2M24) - 0.5
}

/// 0x143b13000 Aero(rcx p, rdx &flow, xmm2 drag, xmm3 lift, [rsp+20] h, [rsp+28] 1.0 (never read), [rsp+30] &jitter)
pub fn fn_143b13000(p: &mut Particle, flow: V3, drag: f32, lift: f32, h: f32, jitter: V3) {
    let (mvx, mvy, mvz) = (-flow[0], -flow[1], -flow[2]);
    let s2 = ((mvy * mvy) + (mvx * mvx)) + (mvz * mvz);
    if !(s2 > C_0_01) {
        return; // comiss/jbe (NaN -> return)
    }
    let s2b = ((flow[1] * flow[1]) + (flow[0] * flow[0])) + (flow[2] * flow[2]);
    if !(s2b > C_0_01) {
        return;
    }
    let len = sqrt(s2); // |flow|
    let inv = 1.0 / len;
    let (ux, uy, uz) = (mvx * inv, mvy * inv, mvz * inv); // u = -flow/|flow|
    let (mut x, mut y, mut z);
    if p.f310 == 0 {
        // ---- variant A (143b130f5): particle without joint-twist neighbours; uses the jitter
        let (mut nx, mut ny, mut nz) = (p.f1d0[0], p.f1d0[1], p.f1d0[2]);
        let mut d = ((ny * uy) + (nx * ux)) + (nz * uz);
        if !(d >= 0.0) {
            // comiss d,0 ; jae skips
            nx = -nx;
            ny = -ny;
            nz = -nz;
            d = -d;
            p.f1d0 = [nx, ny, nz]; // the flipped direction is WRITTEN BACK
        }
        let along = d * len;
        let rem = len - along;
        let cx = (ny * uz) - (nz * uy); // c = n x u
        let cy = (ux * nz) - (nx * uz);
        let cz = (nx * uy) - (ux * ny);
        let drag_mag = rem * drag;
        let lift_mag = along * lift;
        let mut lx = ((uy * cz) - (uz * cy)) + jitter[0]; // l = u x (n x u) + jitter
        let mut ly = ((uz * cx) - (ux * cz)) + jitter[1];
        let mut lz = ((ux * cy) - (uy * cx)) + jitter[2];
        let s = ((ly * ly) + (lx * lx)) + (lz * lz);
        if !(s == 1.0 || s.is_nan()) {
            // ucomiss s,1 ; je (equal or unordered) skips
            if !(s >= C_1EM8) {
                lx = 0.0;
                ly = 0.0;
                lz = 0.0;
            } else {
                let k = rsqrt(s);
                lx = k * lx;
                ly = k * ly;
                lz = k * lz;
            }
        }
        let t = (along - len) * drag; // 143b13315 / 143b13324 (= -dragMag)
        lx = lx * lift_mag;
        ly = ly * lift_mag;
        lz = lz * lift_mag;
        z = (t * uz) + lz;
        x = lx - (ux * drag_mag);
        y = ly - (uy * drag_mag);
        z = z * h;
        x = x * h;
        y = y * h;
    } else {
        // ---- variant B (143b1336c): particle with joint-twist neighbours; no jitter, no write-back of the flip
        let (mut nx, mut ny, mut nz) = (p.f1d0[0], p.f1d0[1], p.f1d0[2]);
        let d0 = ((uy * ny) + (ux * nx)) + (uz * nz);
        if !(d0 >= 0.0) {
            nx = -nx;
            ny = -ny;
            nz = -nz;
        }
        let d = (((uy * ny) + (ux * nx)) + (nz * uz)).abs();
        let cx = (ny * uz) - (nz * uy); // c = n x u
        let cy = (ux * nz) - (nx * uz);
        let cz = (uy * nx) - (ux * ny);
        let mut along = d * len;
        let mut rem = len - along;
        let mut lx = (uz * cy) - (uy * cz); // l = (n x u) x u
        let mut ly = (ux * cz) - (uz * cx);
        let mut lz = (uy * cx) - (ux * cy);
        let s = ((ly * ly) + (lx * lx)) + (lz * lz);
        if !(s == 1.0 || s.is_nan()) {
            if !(s >= C_1EM8) {
                lx = 0.0;
                ly = 0.0;
                lz = 0.0;
            } else {
                let k = rsqrt(s);
                lx = lx * k;
                ly = ly * k;
                lz = lz * k;
            }
        }
        if !(h >= C_0_05) {
            along = along * C_0_7;
            rem = rem * C_0_7;
        }
        let l = rem * lift; // NOTE: lift multiplies (|v| - along) here
        let dd = along * drag; //       drag multiplies along
        x = ((lx * l) - (ux * dd)) * h;
        y = ((ly * l) - (dd * uy)) * h;
        z = ((lz * l) - (dd * uz)) * h;
    }
    p.f40 = [x + p.f40[0], y + p.f40[1], z + p.f40[2]];
}

/// 0x143b123b0 Integrate(xmm0 r, rdx core, r8 scene, r9 core+0x658, [rsp+20] h, [rsp+28] invH, [rsp+30] invN,
///                       [rsp+38] &gravity, [rsp+40] &globalWind, [rsp+48] presetWind[group])
#[allow(clippy::too_many_arguments)]
pub fn fn_143b123b0(r: f32, core: &mut Core, h: f32, inv_h: f32, inv_n: f32, g: V3, wind: V3, preset_wind: &[V3], rng: &mut Rng) {
    let k = h / r;
    let (gx, gy, gz) = (k * g[0], k * g[1], k * g[2]);
    // chara-wind direction for spaces 1 and 2 (computed once); other spaces: zero vector (space 0 = per particle)
    let mut dir = ZERO;
    let space = core.f3720;
    if space == 1 {
        // ref = core.f3730 ? core.f3730 : core; the reference core is this core (notify.js sets
        // core.f3730 = core), so the two extra rotations of the `ref !== core` case do not apply.
        dir = qrot(core.f160, core.f3724);
    } else if space == 2 {
        dir = qrot(core.fd0, core.f3724);
    }
    let version = core.f658;
    let (t_wave, f3740, f373c, f3738) = (core.f3714, core.f3740, core.f373c, core.f3738);
    let (state, mass_scale, world_scale, chara_scale, aero) = (&core.f804, &core.fe94, &core.fdd4, &core.f954, &core.f8f4);
    for (i, p) in core.f3778.f0.iter_mut().enumerate() {
        let grp = p.f2b8;
        if state[grp] < 2 {
            continue;
        }
        if p.f276 != 0 {
            // kinematic
            p.f4c = [p.f40[0], p.f40[1], p.f40[2]];
            let (vz, vx, vy) = ((p.f0[2] - p.fc[2]) * inv_h, (p.f0[0] - p.fc[0]) * inv_h, (p.f0[1] - p.fc[1]) * inv_h);
            p.f40 = [vx, vy, vz];
            continue;
        }
        let damp = if p.f260 < 1.0 { p.f260 } else { 1.0 }; // minss damping,1.0 (NaN -> 1)
        p.f279 = 0;
        let inv2n = inv_n + inv_n;
        p.f58 = ZERO;
        p.f64 = ZERO;
        p.f70 = ZERO;
        p.f88 = ZERO;
        let mut vx = p.f40[0] - ((p.f40[0] * damp) * h);
        let mut vy = p.f40[1] - ((p.f40[1] * damp) * h);
        let mut vz = p.f40[2] - ((p.f40[2] * damp) * h);
        let gm = mass_scale[grp] * p.f250;
        vx = (gx * gm) + vx;
        vy = (gy * gm) + vy;
        vz = (gz * gm) + vz;
        p.f40 = [vx, vy, vz];
        let lf = p.f7c;
        p.f0 = [(inv_n * lf[0]) + p.f0[0], (lf[1] * inv_n) + p.f0[1], (lf[2] * inv_n) + p.f0[2]];
        let ws = world_scale[grp];
        let pw = preset_wind[grp];
        let (mut wx, mut wy, mut wz) = ((pw[0] + wind[0]) * ws, (pw[1] + wind[1]) * ws, (pw[2] + wind[2]) * ws);
        let cs = chara_scale[grp];
        let (lvx, lvy, lvz) = ((inv2n * lf[0]) * inv_h, (inv2n * lf[1]) * inv_h, (inv2n * lf[2]) * inv_h);
        if cs > 0.0 {
            // comiss scale,0 ; jbe skips
            let mut d = dir;
            if space == 0 {
                d = p.f1dc;
            }
            let t = t_wave * C_PI;
            let a = ((t + t) + ((i as f32) * 0.25)) * f3740;
            let s = ((((a as f64).sin() as f32) * f373c) + f3738) * chara_scale[grp];
            wx = (d[0] * s) + wx;
            wy = (d[1] * s) + wy;
            wz = (d[2] * s) + wz;
        }
        if aero[grp] != 0 {
            let (d1, d2, d3) = (rng.next(), rng.next(), rng.next());
            let jit = [rng_to_jitter(d3), rng_to_jitter(d2), rng_to_jitter(d1)]; // x from the THIRD draw, z from the first
            let mut two = false;
            if version >= 0x960 {
                if (p.f26c - p.f294).abs() >= C_0_01 {
                    two = true;
                } else if (p.f270 - p.f298).abs() >= C_0_01 {
                    two = true;
                }
            }
            let (f26c, f270, f294, f298) = (p.f26c, p.f270, p.f294, p.f298);
            if two {
                fn_143b13000(p, [wx, wy, wz], f26c, f270, h, jit);
                fn_143b13000(p, [lvx, lvy, lvz], f294, f298, h, jit);
            } else {
                fn_143b13000(p, [wx + lvx, wy + lvy, wz + lvz], f26c, f270, h, jit);
            }
        }
        p.fc = [p.f0[0], p.f0[1], p.f0[2]];
        p.f0 = [(h * p.f40[0]) + p.f0[0], (p.f40[1] * h) + p.f0[1], (p.f40[2] * h) + p.f0[2]];
    }
}

/// Fragment of Step 0x143b0cb60 (143b0ce63..143b0d033): builds the two wind arguments of Integrate:
/// (wind, presetWind per group) in component space. owner.vf38() = the world wind; owner.vf40()
/// (the preset wind object of a group) is null for this host.
pub fn step_wind_143b0ce63(core: &Core) -> (V3, Vec<V3>) {
    let sc = core.ff0;
    let w = core.f3848.wind;
    let wind = qrot(core.fd0, [w[0] * sc[0], w[1] * sc[1], w[2] * sc[2]]);
    let preset_wind = vec![ZERO; core.f410];
    (wind, preset_wind)
}

/// 0x143b1b830 SphereSnap(rcx out, rdx x (in/out), r8 &centre, xmm3 radius, [rsp+20] strength) -> out
pub fn fn_143b1b830(x: &mut V3, c: V3, radius: f32, strength: f32) -> V3 {
    let (mut dx, mut dy, mut dz) = (c[0] - x[0], c[1] - x[1], c[2] - x[2]);
    let l2 = ((dy * dy) + (dx * dx)) + (dz * dz);
    let (mut x0, mut x1, mut x2) = (x[0], x[1], x[2]);
    if l2 > radius * radius {
        let l = sqrt(l2);
        let inv = 1.0 / l;
        let ex = l - radius;
        let kk = radius * strength;
        dx = dx * inv;
        dy = dy * inv;
        dz = dz * inv;
        x0 = x0 + (dx * ex);
        x1 = x1 + (dy * ex);
        x2 = x2 + (dz * ex);
        dx = dx * kk;
        dy = dy * kk;
        dz = dz * kk;
    } else if l2 > 0.0 {
        let l = sqrt(l2);
        let inv = 1.0 / l;
        let t = l * strength;
        dx = (dx * t) * inv;
        dy = (dy * t) * inv;
        dz = (dz * t) * inv;
    } else {
        return [0.0, 0.0, 0.0];
    }
    // only the LAST part of the move is returned
    x[2] = x2 + dz;
    x[1] = x1 + dy;
    x[0] = x0 + dx;
    [dx, dy, dz]
}

/// 0x143ae5160 Anchors(xmm0 r (not read), rdx &it {f0: scene, f8: int index, -1 at entry}, xmm2 h, xmm3 invH)
pub fn fn_143ae5160(scene: &mut Scene, h: f32, inv_h: f32) {
    for i in 0..scene.f10.len() {
        let a = scene.f10[i];
        let p = &mut scene.f0[a.f0];
        if p.f276 != 0 {
            continue;
        }
        if a.f4 == 2 {
            let kk = a.f8 * C_0_1;
            p.f0 = [((a.fc[0] - p.f0[0]) * kk) + p.f0[0], ((a.fc[1] - p.f0[1]) * kk) + p.f0[1], ((a.fc[2] - p.f0[2]) * kk) + p.f0[2]];
        } else if a.f4 == 1 {
            let mut x = [p.f0[0], p.f0[1], p.f0[2]];
            fn_143b1b830(&mut x, [a.fc[0], a.fc[1], a.fc[2]], a.f24, a.f8);
            p.f0 = x;
        } else if a.f4 == 4 {
            let mut anchor = [(h * a.f34[0]) + a.fc[0], (a.f34[1] * h) + a.fc[1], (a.f34[2] * h) + a.fc[2]];
            let d = fn_143b1b830(&mut anchor, [a.f28[0], a.f28[1], a.f28[2]], a.f24, a.f8);
            let mut x = [p.f0[0], p.f0[1], p.f0[2]];
            fn_143b1b830(&mut x, [anchor[0], anchor[1], anchor[2]], a.f24, a.f8);
            p.f0 = x;
            scene.f10[i].fc = anchor;
            scene.f10[i].f34 = [inv_h * d[0], inv_h * d[1], inv_h * d[2]];
        }
    }
}

/// 0x143ae53f0 LateralLinks(xmm0 r, rdx &it {f0: scene, f8: -1})
pub fn fn_143ae53f0(r: f32, scene: &mut Scene) {
    for idx in 0..scene.f40.len() {
        let l = scene.f40[idx];
        let (p1, p2) = (l.f0, l.f8);
        if l.f26 != 0 {
            continue;
        }
        if l.f28.is_some() {
            continue;
        }
        let ps = &mut scene.f0;
        let (dx, dy, dz) = (ps[p1].f0[0] - ps[p2].f0[0], ps[p1].f0[1] - ps[p2].f0[1], ps[p1].f0[2] - ps[p2].f0[2]);
        let l2 = ((dy * dy) + (dx * dx)) + (dz * dz);
        if !(l2 > 0.0) {
            continue;
        }
        let len = sqrt(l2);
        let inv = 1.0 / len;
        let e = (len - l.f20) * (r * l.f14);
        let (cx, cy, cz) = ((inv * dx) * e, (inv * dy) * e, (inv * dz) * e);
        if ps[p1].f276 != 0 {
            if ps[p2].f276 != 0 {
                continue;
            }
            ps[p2].fac = [cx + ps[p2].fac[0], cy + ps[p2].fac[1], cz + ps[p2].fac[2]];
            ps[p2].fb8 += 1;
            continue;
        }
        if ps[p2].f276 != 0 {
            ps[p1].fac = [ps[p1].fac[0] - cx, ps[p1].fac[1] - cy, ps[p1].fac[2] - cz];
            ps[p1].fb8 += 1;
            continue;
        }
        let k1 = 1.0 - l.f18;
        ps[p1].fac = [ps[p1].fac[0] - (k1 * cx), ps[p1].fac[1] - (k1 * cy), ps[p1].fac[2] - (k1 * cz)];
        ps[p1].fb8 += 1;
        let w = l.f18;
        ps[p2].fac = [(w * cx) + ps[p2].fac[0], (w * cy) + ps[p2].fac[1], (w * cz) + ps[p2].fac[2]];
        ps[p2].fb8 += 1;
        if w > 0.5 {
            let kk = ((w - 0.5) + w) - 0.5;
            ps[p1].fa0 = [(kk * cx) + ps[p1].fa0[0], (kk * cy) + ps[p1].fa0[1], (kk * cz) + ps[p1].fa0[2]];
        } else if !(w >= 0.5) {
            // jbe ; jae -> skip only when w == 0.5
            let t = 0.5 - w;
            let kk = t + t;
            ps[p2].fa0 = [ps[p2].fa0[0] - (kk * cx), ps[p2].fa0[1] - (kk * cy), ps[p2].fa0[2] - (kk * cz)];
        }
    }
}

/// 0x143b1e710 VelocityUpdate(rcx core, rdx scene, r8 core+0x658, xmm3 h (not read), [rsp+20] invH)
pub fn fn_143b1e710(core: &mut Core, inv_h: f32) {
    let vs = core.f670;
    let (c100, c110, c120) = (core.f100, core.f110, core.f120);
    for p in core.f3778.f0.iter_mut() {
        if p.f276 != 0 {
            p.fc = [p.f0[0], p.f0[1], p.f0[2]]; // kinematic: BEFORE the group-state test
            continue;
        }
        if core.f804[p.f2b8] < 2 {
            continue;
        }
        let b = p.f264;
        let (mut dx, mut dy, mut dz) = (p.f0[0] - p.fc[0], p.f0[1] - p.fc[1], p.f0[2] - p.fc[2]);
        let (mut bx, mut by, mut bz) = (0.0f32, 0.0f32, 0.0f32);
        if b > C_0_01 {
            let c = p.f64;
            let n2 = ((c[0] * c[0]) + (c[1] * c[1])) + (c[2] * c[2]);
            if (n2 as f64) > C_1EM8_LO {
                let (mut nx, mut ny, mut nz) = (c[0], c[1], c[2]);
                let s = ((ny * ny) + (nx * nx)) + (nz * nz);
                if s > C_1EM8 {
                    let kk = rsqrt(s);
                    nx = nx * kk;
                    ny = ny * kk;
                    nz = nz * kk;
                }
                let vn = ((ny * p.f40[1]) + (nx * p.f40[0])) + (nz * p.f40[2]);
                if !(vn >= C_M0_01) {
                    let nb = -b;
                    bx = (nx * vn) * nb;
                    by = (ny * vn) * nb;
                    bz = (nz * vn) * nb;
                }
            }
        }
        let acc = p.f58;
        if p.f31d != 0 {
            dx = (dx * C_0_96) - (acc[0] * 0.75);
            dy = (dy * C_0_96) - (acc[1] * 0.75);
            dz = (dz * C_0_96) - (acc[2] * 0.75);
        } else {
            dx = dx - (acc[0] * C_0_98);
            dy = dy - (acc[1] * C_0_98);
            dz = dz - (acc[2] * C_0_98);
        }
        p.f40 = [((dx * inv_h) + bx) * vs, ((dy * inv_h) + by) * vs, ((dz * inv_h) + bz) * vs];
        let q = p.f88;
        let s = ((q[0] * q[0]) + (q[1] * q[1])) + (q[2] * q[2]);
        if !(s == 1.0 || s.is_nan()) {
            // ucomiss ; jne
            if !(s >= C_1EM8) {
                p.f88 = ZERO;
            } else {
                let kk = rsqrt(s);
                p.f88 = [kk * q[0], kk * q[1], kk * q[2]];
            }
        }
        if p.f31c != 0 {
            let w = qrot(c100, [p.f0[0] * c120[0], p.f0[1] * c120[1], p.f0[2] * c120[2]]);
            p.f94 = [w[0] + c110[0], w[1] + c110[1], w[2] + c110[2]];
        }
    }
}
