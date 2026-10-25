// Bonamik pass 2, part "wind": Integrate, Aero, xorshift stream, wind assembly, Anchors, SphereSnap, LateralLinks,
// VelocityUpdate. Transcribed from the disassembly of KH3.exe (image base 0x140000000); see H_wind.md.
// Field naming: f<HEX OFFSET>. Fields reached through the pointer core+0x658 (r9 of Integrate, r8 of VelocityUpdate)
// are named by their CORE offset (pointer+0x1AC -> core.f804, +0x83C -> core.fE94, +0x77C -> core.fDD4,
// +0x2FC -> core.f954, +0x29C -> core.f8F4, +0x18 -> core.f670, +0 -> core.f658): pass the core object for that argument.
'use strict';
const { f, sqrt, rsqrt, qrot } = require('./m.js');

const C_0_01 = f(0.01);            // 0x145CADFC4 (0x3c23d70a)
const C_1EM8 = f(1e-8);            // 0x145C9C488 (0x322bcc77)
const C_1EM8_LO = 9.99999905104687e-9; // 0x14750BA88 (0x322bcc76) - one ulp below, used once in VelocityUpdate
const C_0_05 = f(0.05);            // 0x145C9C4BC (0x3d4ccccd)
const C_0_7 = f(0.7);              // 0x145D11AB0 (0x3f333333)
const C_PI = f(3.1415927410125732);// 0x145C9C4F8 (0x40490fdb)
const C_2M24 = f(5.960464477539063e-8); // 0x145C87CFC (0x33800000) = 2^-24
const C_0_1 = f(0.1);              // 0x145C87D00 (0x3dcccccd)
const C_0_96 = f(0.96);            // 0x14687B7F4 (0x3f75c28f)
const C_0_98 = f(0.98);            // 0x145C9C4E4 (0x3f7ae148)
const C_M0_01 = f(-0.01);          // 0x145CD7804 (0xbc23d70a)
const ZERO = () => [0, 0, 0];      // global at 0x149B5C798 (bss; FVector::ZeroVector)

// ---------------------------------------------------------------------------------------------------------------
// Random stream. 0x142cff460 returns a pointer to a thread-local uint32[4] (TLS block +0x10..+0x1C; initialised on
// first use on each thread, flag bit 0 at +0x20). The update is inlined at every use site (Marsaglia xorshift128):
//   t = x ^ (x << 11); x = y; y = z; z = w; w = (((w >> 11) ^ t) >> 8) ^ t ^ w   (== w ^ (w>>19) ^ t ^ (t>>8))
// next() returns the new w (uint32).
function makeRng(seed) {
  const state = new Uint32Array(seed || [0x075BCD15, 0x159A55E5, 0x1F123BB5, 0x05491333]);
  return {
    state,
    next() {
      const s = state;
      const t = (s[0] ^ (s[0] << 11)) >>> 0;
      s[0] = s[1]; s[1] = s[2];
      const w = s[3];
      s[2] = w;
      const r = (((((w >>> 11) ^ t) >>> 0) >>> 8) ^ t ^ w) >>> 0;
      s[3] = r;
      return r;
    },
  };
}
// conversion used by Integrate: (float)(w >> 8) * 2^-24 - 0.5   -> [-0.5, 0.5), 24-bit resolution
const rngToJitter = w => f(f((w >>> 8) * C_2M24) - 0.5);

// ---------------------------------------------------------------------------------------------------------------
// 0x143b13000  Aero(rcx p, rdx &flow, xmm2 drag, xmm3 lift, [rsp+20] h, [rsp+28] 1.0 (never read), [rsp+30] &jitter)
function fn_143b13000(p, flow, drag, lift, h, unused1, jitter) {
  const mvx = -flow[0], mvy = -flow[1], mvz = -flow[2];
  const s2 = f(f(f(mvy * mvy) + f(mvx * mvx)) + f(mvz * mvz));
  if (!(s2 > C_0_01)) return;                              // comiss/jbe (NaN -> return)
  const s2b = f(f(f(flow[1] * flow[1]) + f(flow[0] * flow[0])) + f(flow[2] * flow[2]));
  if (!(s2b > C_0_01)) return;
  const len = sqrt(s2);                                     // |flow|
  const inv = f(1 / len);
  const ux = f(mvx * inv), uy = f(mvy * inv), uz = f(mvz * inv);   // u = -flow/|flow|
  let x, y, z;
  if (p.f310 === 0) {
    // ---- variant A (143b130f5): particle without joint-twist neighbours; uses the jitter
    let nx = p.f1D0[0], ny = p.f1D0[1], nz = p.f1D0[2];
    let d = f(f(f(ny * uy) + f(nx * ux)) + f(nz * uz));
    if (!(d >= 0)) {                                         // comiss d,0 ; jae skips
      nx = -nx; ny = -ny; nz = -nz; d = -d;
      p.f1D0 = [nx, ny, nz];                                 // the flipped direction is WRITTEN BACK
    }
    const along = f(d * len);
    const rem = f(len - along);
    const cx = f(f(ny * uz) - f(nz * uy));                   // c = n x u
    const cy = f(f(ux * nz) - f(nx * uz));
    const cz = f(f(nx * uy) - f(ux * ny));
    const dragMag = f(rem * drag);
    const liftMag = f(along * lift);
    let lx = f(f(f(uy * cz) - f(uz * cy)) + jitter[0]);      // l = u x (n x u) + jitter
    let ly = f(f(f(uz * cx) - f(ux * cz)) + jitter[1]);
    let lz = f(f(f(ux * cy) - f(uy * cx)) + jitter[2]);
    const s = f(f(f(ly * ly) + f(lx * lx)) + f(lz * lz));
    if (!(s === 1 || s !== s)) {                             // ucomiss s,1 ; je (equal or unordered) skips
      if (!(s >= C_1EM8)) { lx = 0; ly = 0; lz = 0; }
      else { const k = rsqrt(s); lx = f(k * lx); ly = f(k * ly); lz = f(k * lz); }
    }
    const t = f(f(along - len) * drag);                      // 143b13315 / 143b13324 (= -dragMag)
    lx = f(lx * liftMag); ly = f(ly * liftMag); lz = f(lz * liftMag);
    z = f(f(t * uz) + lz);
    x = f(lx - f(ux * dragMag));
    y = f(ly - f(uy * dragMag));
    z = f(z * h); x = f(x * h); y = f(y * h);
  } else {
    // ---- variant B (143b1336c): particle with joint-twist neighbours; no jitter, no write-back of the flip
    let nx = p.f1D0[0], ny = p.f1D0[1], nz = p.f1D0[2];
    const d0 = f(f(f(uy * ny) + f(ux * nx)) + f(uz * nz));
    if (!(d0 >= 0)) { nx = -nx; ny = -ny; nz = -nz; }
    const d = Math.abs(f(f(f(uy * ny) + f(ux * nx)) + f(nz * uz)));
    const cx = f(f(ny * uz) - f(nz * uy));                   // c = n x u
    const cy = f(f(ux * nz) - f(nx * uz));
    const cz = f(f(uy * nx) - f(ux * ny));
    let along = f(d * len);
    let rem = f(len - along);
    let lx = f(f(uz * cy) - f(uy * cz));                     // l = (n x u) x u
    let ly = f(f(ux * cz) - f(uz * cx));
    let lz = f(f(uy * cx) - f(ux * cy));
    const s = f(f(f(ly * ly) + f(lx * lx)) + f(lz * lz));
    if (!(s === 1 || s !== s)) {
      if (!(s >= C_1EM8)) { lx = 0; ly = 0; lz = 0; }
      else { const k = rsqrt(s); lx = f(lx * k); ly = f(ly * k); lz = f(lz * k); }
    }
    if (!(h >= C_0_05)) { along = f(along * C_0_7); rem = f(rem * C_0_7); }
    const L = f(rem * lift);                                 // NOTE: lift multiplies (|v| - along) here
    const D = f(along * drag);                               //       drag multiplies along
    x = f(f(f(lx * L) - f(ux * D)) * h);
    y = f(f(f(ly * L) - f(D * uy)) * h);
    z = f(f(f(lz * L) - f(D * uz)) * h);
  }
  p.f40 = [f(x + p.f40[0]), f(y + p.f40[1]), f(z + p.f40[2])];
}

// ---------------------------------------------------------------------------------------------------------------
// 0x143b123b0  Integrate(xmm0 r, rdx core, r8 scene, r9 core+0x658, [rsp+20] h, [rsp+28] invH, [rsp+30] invN,
//                        [rsp+38] &gravity, [rsp+40] &globalWind, [rsp+48] presetWind[group] (12 bytes each))
// deps = { rng: makeRng(), sinf: x => float } (the exe calls the CRT import sinf through the thunk 0x1458719A0).
function fn_143b123b0(r, core, scene, coreP658, h, invH, invN, g, wind, presetWind, deps) {
  const rng = deps.rng;
  const sinf = deps.sinf || (x => f(Math.sin(x)));
  const k = f(h / r);
  const gx = f(k * g[0]), gy = f(k * g[1]), gz = f(k * g[2]);
  // chara-wind direction for spaces 1 and 2 (computed once); other spaces: zero vector (space 0 = per particle)
  let dir = ZERO();
  const space = core.f3720;
  if (space === 1) {
    const ref = core.f3730 ? core.f3730 : core;               // cmovne
    dir = qrot(ref.f160, core.f3724);
    if (ref !== core) {
      dir = qrot(ref.f100, dir);                              // ref's component -> world rotation
      dir = qrot(core.fD0, dir);                              // world -> this component
    }
  } else if (space === 2) {
    dir = qrot(core.fD0, core.f3724);
  }
  const P = scene.f0, n = scene.f8;
  const state = coreP658.f804, massScale = coreP658.fE94, worldScale = coreP658.fDD4, charaScale = coreP658.f954,
    aero = coreP658.f8F4;
  for (let i = 0; i < n; i++) {
    const p = P[i];
    const grp = p.f2B8;
    if (state[grp] < 2) continue;
    if (p.f276 !== 0) {                                       // kinematic
      p.f4C = [p.f40[0], p.f40[1], p.f40[2]];
      const vz = f(f(p.f0[2] - p.fC[2]) * invH), vx = f(f(p.f0[0] - p.fC[0]) * invH), vy = f(f(p.f0[1] - p.fC[1]) * invH);
      p.f40 = [vx, vy, vz];
      continue;
    }
    const damp = p.f260 < 1 ? p.f260 : 1;                     // minss damping,1.0 (NaN -> 1)
    p.f279 = 0;
    const inv2N = f(invN + invN);
    p.f58 = ZERO(); p.f64 = ZERO(); p.f70 = ZERO(); p.f88 = ZERO();
    let vx = f(p.f40[0] - f(f(p.f40[0] * damp) * h));
    let vy = f(p.f40[1] - f(f(p.f40[1] * damp) * h));
    let vz = f(p.f40[2] - f(f(p.f40[2] * damp) * h));
    const gm = f(massScale[grp] * p.f250);
    vx = f(f(gx * gm) + vx); vy = f(f(gy * gm) + vy); vz = f(f(gz * gm) + vz);
    p.f40 = [vx, vy, vz];
    const lf = p.f7C;
    p.f0 = [f(f(invN * lf[0]) + p.f0[0]), f(f(lf[1] * invN) + p.f0[1]), f(f(lf[2] * invN) + p.f0[2])];
    const ws = worldScale[grp];
    const pw = presetWind[grp];
    let wx = f(f(pw[0] + wind[0]) * ws), wy = f(f(pw[1] + wind[1]) * ws), wz = f(f(pw[2] + wind[2]) * ws);
    const cs = charaScale[grp];
    const lvx = f(f(inv2N * lf[0]) * invH), lvy = f(f(inv2N * lf[1]) * invH), lvz = f(f(inv2N * lf[2]) * invH);
    if (cs > 0) {                                             // comiss scale,0 ; jbe skips
      let d = dir;
      if (space === 0) d = p.f1DC;
      const t = f(core.f3714 * C_PI);
      const a = f(f(f(t + t) + f(f(i) * 0.25)) * core.f3740);
      const s = f(f(f(sinf(a) * core.f373C) + core.f3738) * charaScale[grp]);
      wx = f(f(d[0] * s) + wx); wy = f(f(d[1] * s) + wy); wz = f(f(d[2] * s) + wz);
    }
    if (aero[grp] !== 0) {
      const d1 = rng.next(), d2 = rng.next(), d3 = rng.next();
      const jit = [rngToJitter(d3), rngToJitter(d2), rngToJitter(d1)];   // x from the THIRD draw, z from the first
      let two = false;
      if ((coreP658.f658 >>> 0) >= 0x960) {
        if (Math.abs(f(p.f26C - p.f294)) >= C_0_01) two = true;
        else if (Math.abs(f(p.f270 - p.f298)) >= C_0_01) two = true;
      }
      if (two) {
        fn_143b13000(p, [wx, wy, wz], p.f26C, p.f270, h, 1, jit);
        fn_143b13000(p, [lvx, lvy, lvz], p.f294, p.f298, h, 1, jit);
      } else {
        fn_143b13000(p, [f(wx + lvx), f(wy + lvy), f(wz + lvz)], p.f26C, p.f270, h, 1, jit);
      }
    }
    p.fC = [p.f0[0], p.f0[1], p.f0[2]];
    p.f0 = [f(f(h * p.f40[0]) + p.f0[0]), f(f(p.f40[1] * h) + p.f0[1]), f(f(p.f40[2] * h) + p.f0[2])];
  }
}

// ---------------------------------------------------------------------------------------------------------------
// Fragment of Step 0x143b0cb60 (143b0ce63..143b0d033): builds the two wind arguments of Integrate.
// owner.vf38() -> world-space wind [x,y,z] (0x143b57640); owner.vf40(id) -> preset wind object (field f90 = vec3) or null.
// returns { wind:[x,y,z], presetWind:[[x,y,z] per group] } in component space.
function stepWind_143b0ce63(core, owner) {
  const sc = core.fF0;
  const out = [0, 0, 0];
  const w = owner.vf38(out) || out;
  const wind = qrot(core.fD0, [f(w[0] * sc[0]), f(w[1] * sc[1]), f(w[2] * sc[2])]);
  const presetWind = [];
  for (let i = 0; i < core.f410; i++) {
    const o = owner.vf40(core.fAD4[i]);
    if (!o) { presetWind.push(ZERO()); continue; }
    const s = core.fA14[i];
    const v = o.f90;
    presetWind.push(qrot(core.fD0, [f(f(s * v[0]) * sc[0]), f(f(s * v[1]) * sc[1]), f(f(s * v[2]) * sc[2])]));
  }
  return { wind, presetWind };
}

// ---------------------------------------------------------------------------------------------------------------
// 0x143b1b830  SphereSnap(rcx out, rdx x (in/out), r8 &centre, xmm3 radius, [rsp+20] strength) -> rax = out
function fn_143b1b830(out, x, c, radius, strength) {
  let dx = f(c[0] - x[0]), dy = f(c[1] - x[1]), dz = f(c[2] - x[2]);
  out[0] = 0; out[1] = 0; out[2] = 0;
  const l2 = f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz));
  let x0 = x[0], x1 = x[1], x2 = x[2];
  if (l2 > f(radius * radius)) {
    const l = sqrt(l2);
    const inv = f(1 / l);
    const ex = f(l - radius);
    const kk = f(radius * strength);
    dx = f(dx * inv); dy = f(dy * inv); dz = f(dz * inv);
    x0 = f(x0 + f(dx * ex)); x1 = f(x1 + f(dy * ex)); x2 = f(x2 + f(dz * ex));
    dx = f(dx * kk); dy = f(dy * kk); dz = f(dz * kk);
  } else if (l2 > 0) {
    const l = sqrt(l2);
    const inv = f(1 / l);
    const t = f(l * strength);
    dx = f(f(dx * t) * inv); dy = f(f(dy * t) * inv); dz = f(f(dz * t) * inv);
  } else {
    return out;
  }
  out[0] = dx; out[1] = dy; out[2] = dz;                     // only the LAST part of the move is returned
  x[2] = f(x2 + dz); x[1] = f(x1 + dy); x[0] = f(x0 + dx);
  return out;
}

// ---------------------------------------------------------------------------------------------------------------
// 0x143ae5160  Anchors(xmm0 r (not read), rdx &it {f0: scene, f8: int index, -1 at entry}, xmm2 h, xmm3 invH)
// anchor record (stride 0x40, scene.f10, count scene.f18): f0 particle index, f4 type, f8 strength, fC anchor vec3,
// f24 radius, f28 anchor copy vec3, f34 velocity vec3.
function fn_143ae5160(r, it, h, invH) {
  for (;;) {
    it.f8 = (it.f8 + 1) | 0;
    const scene = it.f0;
    if (it.f8 >= scene.f18) return;
    const a = scene.f10[it.f8];
    const p = scene.f0[a.f0];
    if (p.f276 !== 0) continue;
    if (a.f4 === 2) {
      const kk = f(a.f8 * C_0_1);
      p.f0 = [f(f(f(a.fC[0] - p.f0[0]) * kk) + p.f0[0]), f(f(f(a.fC[1] - p.f0[1]) * kk) + p.f0[1]),
        f(f(f(a.fC[2] - p.f0[2]) * kk) + p.f0[2])];
    } else if (a.f4 === 1) {
      const x = [p.f0[0], p.f0[1], p.f0[2]];
      fn_143b1b830([0, 0, 0], x, [a.fC[0], a.fC[1], a.fC[2]], a.f24, a.f8);
      p.f0 = x;
    } else if (a.f4 === 4) {
      const A = [f(f(h * a.f34[0]) + a.fC[0]), f(f(a.f34[1] * h) + a.fC[1]), f(f(a.f34[2] * h) + a.fC[2])];
      const d = fn_143b1b830([0, 0, 0], A, [a.f28[0], a.f28[1], a.f28[2]], a.f24, a.f8);
      a.fC = A;
      a.f34 = [f(invH * d[0]), f(invH * d[1]), f(invH * d[2])];
      const x = [p.f0[0], p.f0[1], p.f0[2]];
      fn_143b1b830([0, 0, 0], x, [A[0], A[1], A[2]], a.f24, a.f8);
      p.f0 = x;
    }
  }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x143ae53f0  LateralLinks(xmm0 r, rdx &it {f0: scene, f8: -1})
// link record (stride 0x38, scene.f40, count scene.f48): f0 parent particle, f8 child particle, f14 strength,
// f18 parent share, f20 rest length, f26 byte, f28 cone pointer.
function fn_143ae53f0(r, it) {
  for (;;) {
    const idx = (it.f8 + 1) | 0;
    it.f8 = idx;
    const scene = it.f0;
    if (idx >= scene.f48) return;
    const l = scene.f40[idx];
    const p1 = l.f0, p2 = l.f8;
    if (l.f26 !== 0) continue;
    if (l.f28) continue;
    const dx = f(p1.f0[0] - p2.f0[0]), dy = f(p1.f0[1] - p2.f0[1]), dz = f(p1.f0[2] - p2.f0[2]);
    const l2 = f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz));
    if (!(l2 > 0)) continue;
    const len = sqrt(l2);
    const inv = f(1 / len);
    const e = f(f(len - l.f20) * f(r * l.f14));
    const cx = f(f(inv * dx) * e), cy = f(f(inv * dy) * e), cz = f(f(inv * dz) * e);
    if (p1.f276 !== 0) {
      if (p2.f276 !== 0) continue;
      p2.fAC = [f(cx + p2.fAC[0]), f(cy + p2.fAC[1]), f(cz + p2.fAC[2])];
      p2.fB8 = (p2.fB8 + 1) | 0;
      continue;
    }
    if (p2.f276 !== 0) {
      p1.fAC = [f(p1.fAC[0] - cx), f(p1.fAC[1] - cy), f(p1.fAC[2] - cz)];
      p1.fB8 = (p1.fB8 + 1) | 0;
      continue;
    }
    const k1 = f(1 - l.f18);
    p1.fAC = [f(p1.fAC[0] - f(k1 * cx)), f(p1.fAC[1] - f(k1 * cy)), f(p1.fAC[2] - f(k1 * cz))];
    p1.fB8 = (p1.fB8 + 1) | 0;
    const w = l.f18;
    p2.fAC = [f(f(w * cx) + p2.fAC[0]), f(f(w * cy) + p2.fAC[1]), f(f(w * cz) + p2.fAC[2])];
    p2.fB8 = (p2.fB8 + 1) | 0;
    if (w > 0.5) {
      const kk = f(f(f(w - 0.5) + w) - 0.5);
      p1.fA0 = [f(f(kk * cx) + p1.fA0[0]), f(f(kk * cy) + p1.fA0[1]), f(f(kk * cz) + p1.fA0[2])];
    } else if (!(w >= 0.5)) {                                 // jbe ; jae -> skip only when w == 0.5
      const t = f(0.5 - w);
      const kk = f(t + t);
      p2.fA0 = [f(p2.fA0[0] - f(kk * cx)), f(p2.fA0[1] - f(kk * cy)), f(p2.fA0[2] - f(kk * cz))];
    }
  }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x143b1e710  VelocityUpdate(rcx core, rdx scene, r8 core+0x658, xmm3 h (not read), [rsp+20] invH)
function fn_143b1e710(core, scene, coreP658, h, invH) {
  const P = scene.f0;
  for (let i = 0; i < scene.f8; i++) {
    const p = P[i];
    if (p.f276 !== 0) { p.fC = [p.f0[0], p.f0[1], p.f0[2]]; continue; }   // kinematic: BEFORE the group-state test
    if (coreP658.f804[p.f2B8] < 2) continue;
    const b = p.f264;
    let dx = f(p.f0[0] - p.fC[0]), dy = f(p.f0[1] - p.fC[1]), dz = f(p.f0[2] - p.fC[2]);
    let bx = 0, by = 0, bz = 0;
    if (b > C_0_01) {
      const c = p.f64;
      const n2 = f(f(f(c[0] * c[0]) + f(c[1] * c[1])) + f(c[2] * c[2]));
      if (n2 > C_1EM8_LO) {
        let nx = c[0], ny = c[1], nz = c[2];
        const s = f(f(f(ny * ny) + f(nx * nx)) + f(nz * nz));
        if (s > C_1EM8) { const kk = rsqrt(s); nx = f(nx * kk); ny = f(ny * kk); nz = f(nz * kk); }
        const vn = f(f(f(ny * p.f40[1]) + f(nx * p.f40[0])) + f(nz * p.f40[2]));
        if (!(vn >= C_M0_01)) {
          const nb = -b;
          bx = f(f(nx * vn) * nb); by = f(f(ny * vn) * nb); bz = f(f(nz * vn) * nb);
        }
      }
    }
    const acc = p.f58;
    if (p.f31D !== 0) {
      dx = f(f(dx * C_0_96) - f(acc[0] * 0.75));
      dy = f(f(dy * C_0_96) - f(acc[1] * 0.75));
      dz = f(f(dz * C_0_96) - f(acc[2] * 0.75));
    } else {
      dx = f(dx - f(acc[0] * C_0_98));
      dy = f(dy - f(acc[1] * C_0_98));
      dz = f(dz - f(acc[2] * C_0_98));
    }
    const vs = coreP658.f670;
    p.f40 = [f(f(f(dx * invH) + bx) * vs), f(f(f(dy * invH) + by) * vs), f(f(f(dz * invH) + bz) * vs)];
    const q = p.f88;
    const s = f(f(f(q[0] * q[0]) + f(q[1] * q[1])) + f(q[2] * q[2]));
    if (!(s === 1 || s !== s)) {                              // ucomiss ; jne
      if (!(s >= C_1EM8)) p.f88 = ZERO();
      else { const kk = rsqrt(s); p.f88 = [f(kk * q[0]), f(kk * q[1]), f(kk * q[2])]; }
    }
    if (p.f31C !== 0) {
      const sc = core.f120;
      const w = qrot(core.f100, [f(p.f0[0] * sc[0]), f(p.f0[1] * sc[1]), f(p.f0[2] * sc[2])]);
      p.f94 = [f(w[0] + core.f110[0]), f(w[1] + core.f110[1]), f(w[2] + core.f110[2])];
    }
  }
}

module.exports = {
  makeRng, rngToJitter, fn_143b13000, fn_143b123b0, stepWind_143b0ce63, fn_143b1b830, fn_143ae5160, fn_143ae53f0,
  fn_143b1e710,
};

// ---------------------------------------------------------------------------------------------------------------
if (require.main === module) {
  const rng = makeRng();
  const first = [rng.next(), rng.next(), rng.next(), rng.next()];
  console.log('xorshift128 first draws', first.map(x => x.toString(16)).join(' '),
    '(Marsaglia reference: 3701687786 458299110 2500872618 3633119408 ->', first.join(' '), ')');
  const mkP = (i, kin, tw) => ({
    f0: [i, 0, 100 - i], fC: [i, 0, 100 - i], f40: [0, 0, 0], f4C: [0, 0, 0], f58: [0, 0, 0], f64: [0, 0, 0],
    f70: [0, 0, 0], f7C: [0.3, -0.1, 0.05], f88: [0, 0, 0], f94: [0, 0, 0], fA0: [0, 0, 0], fAC: [0, 0, 0], fB8: 0,
    f1D0: [0, 0, -1], f1DC: [0, 0, 1], f250: 1, f260: 0.5, f264: 0, f26C: 0.3, f270: 0.5, f276: kin, f279: 1,
    f294: 0.5, f298: 4, f2B8: 0, f310: tw, f31C: 0, f31D: 0,
  });
  const P = [mkP(0, 1, 0), mkP(1, 0, 0), mkP(2, 0, 1)];
  const scene = {
    f0: P, f8: 3,
    f10: [{ f0: 1, f4: 1, f8: 0.5, fC: [1, 0, 90], f24: 4.5, f28: [1, 0, 90], f34: [0, 0, 0] },
      { f0: 2, f4: 2, f8: 0.06, fC: [2, 0, 95], f24: 3, f28: [2, 0, 95], f34: [0, 0, 0] },
      { f0: 2, f4: 4, f8: 0.2, fC: [2, 1, 95], f24: 1, f28: [2, 0, 99], f34: [0, 0, 1] }],
    f18: 3,
    f40: [{ f0: P[0], f8: P[1], f14: 0.8, f18: 0.5, f20: 1.2, f26: 0, f28: null },
      { f0: P[1], f8: P[2], f14: 0.8, f18: 0.7, f20: 1.2, f26: 0, f28: null }],
    f48: 2,
  };
  const core = {
    f658: 2401, f670: 1, f804: [2], fE94: [1], fDD4: [1], f954: [2], f8F4: [1], fA14: [1], fAD4: [11], f410: 1,
    fD0: [0, 0, 0, 1], fF0: [1, 1, 1], f100: [0, 0, 0, 1], f110: [0, 0, 0], f120: [1, 1, 1], f160: [0, 0, 0, 1],
    f3714: 0.35, f3720: 1, f3724: [-0.3419, -0.2279, 0.9117], f3730: null, f3738: 600, f373C: 4, f3740: f(1 / 0.03),
  };
  core.f3730 = core;
  const owner = { vf38: out => { out[0] = 10; out[1] = 0; out[2] = 0; return out; }, vf40: () => null };
  const sw = stepWind_143b0ce63(core, owner);
  console.log('stepWind', JSON.stringify(sw));
  const h = f(0.05), invH = f(1 / h);
  fn_143b123b0(1, core, scene, core, h, invH, 1, [0, 0, -981], sw.wind, sw.presetWind, { rng });
  console.log('Integrate', JSON.stringify(P.map(p => ({ pos: p.f0, vel: p.f40 }))));
  fn_143ae5160(1, { f0: scene, f8: -1 }, h, invH);
  console.log('Anchors', JSON.stringify(P.map(p => p.f0)), JSON.stringify(scene.f10[2]));
  fn_143ae53f0(1, { f0: scene, f8: -1 });
  console.log('LateralLinks', JSON.stringify(P.map(p => ({ aC: p.fAC, a0: p.fA0, n: p.fB8 }))));
  const o = [0, 0, 0], x = [0, 0, 0];
  console.log('SphereSnap', JSON.stringify(fn_143b1b830(o, x, [10, 0, 0], 4, 0.5)), JSON.stringify(x));
  fn_143b1e710(core, scene, core, h, invH);
  console.log('VelocityUpdate', JSON.stringify(P.map(p => ({ vel: p.f40, prev: p.fC }))));
}
