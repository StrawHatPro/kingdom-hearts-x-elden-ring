// Bonamik collision - transcription of KH3.exe (see H_coll.md). float32 everywhere, operation order of the exe.
// Records: particle p.fXX, collision object o.fXX (0x88 bytes), scene.fXX, core.fXX (hex offsets).
//   o.f0/o.f8/o.f10 particle refs (or null), o.f18 type, o.f1C batch no, o.f20 plane normal, o.f2C plane point,
//   o.f38 group, o.f3C int[8] colId (planes), o.f5C thickness, o.f60 plane offset, o.f64 concave ratio,
//   o.f68 previous plane point, o.f74 bound centre, o.f80 bound radius.
//   scene.f50 = array of dynamic-side objects (count scene.f58), scene.f60 = self-collision pairs (count scene.f68),
//   scene.f70 = array indexed by colId c (0..47 group lists, 48.. collision-tag lists) of arrays of objects
//   (exe: pointer array at scene+0x70+16c, count at scene+0x78+16c).
const { f, add, qmul, qrot, sqrt, rsqrt } = require('./m.js');

const C_1E8 = f(9.99999993922529e-9);    // 0x145C9C488
const C_1E4 = f(0.00009999999747378752); // 0x145CF35A8
const C_1E9 = f(9.999999717180685e-10);  // 0x146467DB8
const C_THIRD = f(0.3333333432674408);   // 0x145D3A010
const Z = [0, 0, 0];                     // global vec3 at 0x149B5C798 (.bss, runtime; FVector::ZeroVector by usage) [?]
const cnt = a => (a ? a.length : 0);

// ---------------------------------------------------------------------------------------------------------------
// 0x143b10190 (rcx = o): bound centre of a sphere / capsule object
function fn_143b10190(o) {
  const t = o.f18;
  if (((t - 2) & 0xFFFFFFFD) === 0) {                 // 2 capsule, 4 inverse capsule
    const b = o.f8.f0, a = o.f0.f0;
    o.f74 = [f(f(b[0] + a[0]) * 0.5), f(f(b[1] + a[1]) * 0.5), f(f(b[2] + a[2]) * 0.5)];
  } else if (((t - 1) & 0xFFFFFFFD) === 0) {          // 1 sphere, 3 inverse sphere
    o.f74 = o.f0.f0.slice();
  } else o.f74 = [0, 0, 0];
}

// 0x143afc140 (rcx = o): plane update (type 6)
function fn_143afc140(o) {
  o.f68 = o.f2C.slice();
  const B = o.f8, Cc = o.f10;
  if (B && Cc) {
    const a = o.f0.f0, b = B.f0, c = Cc.f0;
    const pz = f(f(f(b[2] + a[2]) + c[2]) * C_THIRD);
    const py = f(f(f(b[1] + a[1]) + c[1]) * C_THIRD);
    const px = f(f(f(b[0] + a[0]) + c[0]) * C_THIRD);
    o.f2C = [px, py, pz];
    const e1x = f(b[0] - a[0]), e1y = f(b[1] - a[1]), e1z = f(b[2] - a[2]);     // B - A
    const e2x = f(c[0] - b[0]), e2y = f(c[1] - b[1]), e2z = f(c[2] - b[2]);     // C - B
    let nx = f(f(e2y * e1z) - f(e2z * e1y));
    let ny = f(f(e2z * e1x) - f(e2x * e1z));
    let nz = f(f(e2x * e1y) - f(e2y * e1x));
    const l2 = f(f(f(nx * nx) + f(ny * ny)) + f(nz * nz));
    if (l2 > C_1E8) { const y = rsqrt(l2); nx = f(nx * y); ny = f(ny * y); nz = f(nz * y); }
    o.f20 = [nx, ny, nz];
  } else {
    const A = o.f0; o.f2C = A.f0.slice();
    const qx = A.f20[0], qy = A.f20[1], qz = A.f20[2], qw = A.f20[3];
    const z2 = f(qz + qz), my2 = f(f(-qy) + f(-qy));
    const nx = f(f(f(qy * my2) - f(qz * z2)) + 1);
    const ny = f(f(qw * z2) - f(qx * my2));
    const nz = f(f(qw * my2) + f(qx * z2));
    o.f20 = [nx, ny, nz];                                                          // = rot(q, +X)
  }
  const k = o.f60, n = o.f20, p = o.f2C;
  o.f74 = [f(f(k * n[0]) + p[0]), f(f(k * n[1]) + p[1]), f(f(k * n[2]) + p[2])];
}

// 0x143ae62b0 (rcx = o, edx = type, r8 = A, r9 = B, [rsp+0x28] = C): constructor
function fn_143ae62b0(o, type, A, B, Cc) {
  o.f10 = Cc || null; o.f38 = 0; o.f60 = 0; o.f64 = 0; o.f0 = A; o.f8 = B || null; o.f1C = -1; o.f18 = type;
  o.f3C = [-1, -1, -1, -1, -1, -1, -1, -1]; o.f5C = f(0.009999999776482582);   // 0x3C23D70A
  o.f20 = [0, 0, 0]; o.f2C = [0, 0, 0]; o.f68 = [0, 0, 0];                          // (memory not initialised by the exe)
  fn_143b10190(o);
  let r;
  if (((type - 2) & 0xFFFFFFFD) === 0) {
    const a = o.f0, b = o.f8;
    const dx = f(a.f0[0] - b.f0[0]), dz = f(a.f0[2] - b.f0[2]), dy = f(a.f0[1] - b.f0[1]);
    const m = Math.max(a.f25C, b.f25C);                                              // fmaxf (import at 0x145C0E380)
    r = f(m + f(sqrt(f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz))) * 0.5));
  } else if (((type - 1) & 0xFFFFFFFD) === 0) r = o.f0.f25C;
  else if (type === 6) {
    fn_143afc140(o);
    const a = o.f0.f0;
    const dz = f(a[2] - o.f2C[2]), dy = f(a[1] - o.f2C[1]), dx = f(a[0] - o.f2C[0]);
    r = f(sqrt(f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz))) + o.f5C);
  } else r = 0;
  o.f80 = r;
  if (o.f18 === 6) { fn_143afc140(o); o.f68 = o.f2C.slice(); }
  return o;
}

// 0x143aec160 (rcx = scene, edx = a, r8d = b, r9d = c): add a dynamic-side object to scene+0x50 [C]
function fn_143aec160(scene, ia, ib, ic) {
  const P = scene.f0;
  if (scene.f79C !== undefined && scene.f79C === scene.f50.length) return null;
  const o = {};
  if (ib === -1) fn_143ae62b0(o, 1, P[ia], null, null);
  else if (ic === -1) fn_143ae62b0(o, 2, P[ia], P[ib], null);
  else fn_143ae62b0(o, 6, P[ia], P[ib], P[ic]);
  scene.f50.push(o); scene.f58 = scene.f50.length;
  return o;
}
// 0x143aec560 (rcx = scene, edx = group, r8d = a, r9d = b, [rsp+0x28] = c): collision plane into the group list [C]
function fn_143aec560(scene, g, ia, ib, ic) {
  const P = scene.f0, o = {};
  if (ib === -1 || ic === -1) fn_143ae62b0(o, 6, P[ia], null, null); else fn_143ae62b0(o, 6, P[ia], P[ib], P[ic]);
  (scene.f70[g] = scene.f70[g] || []).push(o);
  return o;
}
// 0x143aec680 (rcx = scene, edx = type, r8d = a, r9d = b): collider into the list of particle a's group [C]
function fn_143aec680(scene, type, ia, ib) {
  const P = scene.f0, o = {}, g = P[ia].f2B8;
  if (ib === -1) fn_143ae62b0(o, type, P[ia], null, null); else fn_143ae62b0(o, type, P[ia], P[ib], null);
  (scene.f70[g] = scene.f70[g] || []).push(o);
  return o;
}
// 0x143aedb50 (rcx = scene, edx = tag-1, r8 = o) [C]
function fn_143aedb50(scene, tagM1, o) { (scene.f70[48 + tagM1] = scene.f70[48 + tagM1] || []).push(o); }
// 0x143ae4440 (rcx = array, edx = count): stable sort of scene+0x50 by o.f1C ascending (signed) [C]
function fn_143ae4440(arr) {
  const s = arr.map((o, k) => [o, k]).sort((x, y) => (x[0].f1C - y[0].f1C) || (x[1] - y[1])).map(x => x[0]);
  for (let i = 0; i < arr.length; i++) arr[i] = s[i];
}

// ---- setup fragments of 0x143b07550 (instruction-checked at 143b0889c..143b089e1, 143b094b0..143b097e3,
//      143b09bda..143b09ca3, 143b0b11b..143b0b13b; the helper functions above are [C]) ----
function initCollisionLists(scene) { scene.f50 = []; scene.f58 = 0; scene.f60 = []; scene.f68 = 0; scene.f70 = []; }
// planes: descs = [{m_GroupId, m_ParticleA, m_ParticleB, m_ParticleC, m_Thickness, m_Offset, m_IsCollision, m_CollisionBatchNo, m_ColId[8]}]
function setupPlanes(core, descs, indexBase) {
  const scene = core.f3778; indexBase = indexBase || 0;
  for (const d of descs) {
    if (d.m_IsCollision) {
      const o = fn_143aec560(scene, d.m_GroupId, d.m_ParticleA + indexBase, d.m_ParticleB + indexBase, d.m_ParticleC + indexBase);
      if (o) { o.f38 = d.m_GroupId; if ((core.f744[d.m_GroupId] >>> 0) > 0) fn_143aedb50(scene, core.f744[d.m_GroupId] - 1, o); }
    } else {
      const o = fn_143aec160(scene, d.m_ParticleA + indexBase, d.m_ParticleB + indexBase, d.m_ParticleC + indexBase);
      if (o) { o.f38 = d.m_GroupId; o.f1C = d.m_CollisionBatchNo; o.f60 = d.m_Offset; o.f5C = d.m_Thickness; o.f3C = d.m_ColId.slice(0, 8); }
    }
  }
}
// bodies: uses p.f190 / p.f198 as they are at that point of setup (from m_ParentId / m_ChildId)
function setupBodyObjects(core) {
  const scene = core.f3778;
  const nId = (core.f658 >>> 0) > 0x960 ? 8 : 4;
  for (const p of scene.f0) {
    if (p.f277) {
      const s = p.f27C;
      if (((s - 2) & 0xFFFFFFFD) === 0) { p.f2A0 = fn_143aec680(scene, s, p.f24C, p.f198.f24C); p.f2A0.f64 = p.f284; }
      else if (((s - 1) & 0xFFFFFFFD) === 0) p.f2A0 = fn_143aec680(scene, s, p.f24C, -1);
      else if (s === 6) p.f2A0 = fn_143aec560(scene, p.f2B8, p.f24C, -1, -1);
      else if (s === 8) { const o = fn_143ae62b0({}, 8, p, null, null); (scene.f70[p.f2B8] = scene.f70[p.f2B8] || []).push(o); p.f2A0 = o; }
      if (p.f2A0) { p.f2A0.f38 = p.f2B8; const tag = core.f744[p.f2B8]; if (tag !== 0) (scene.f70[tag + 47] = scene.f70[tag + 47] || []).push(p.f2A0); }
    } else {
      let any = false; for (let k = 0; k < nId; k++) if (p.f2BC[k] !== -1) { any = true; break; }
      if (!any) continue;
      const s = p.f27C;
      if (s === 2) {
        const c = p.f198;
        if (!c) p.f2A0 = fn_143aec160(scene, p.f24C, -1, -1);
        else if (!(p.f276 && c.f276)) { p.f2A0 = fn_143aec160(scene, p.f24C, c.f24C, -1); p.f2A0.f64 = p.f284; }
      } else if (s === 1 && !p.f276) {
        const par = p.f190;
        if (!par || par.f27C === 1 || (par.f27C === 2 && par.f198 !== p)) p.f2A0 = fn_143aec160(scene, p.f24C, -1, -1);
      }
      if (p.f2A0) { p.f2A0.f38 = p.f2B8; p.f2A0.f1C = p.f280; }
    }
  }
}
// per LinkDesc d, called where 0x143b07550 handles it. link = the link record just created by 0x143aec770 (or null).
// d = {m_ParentId, m_ChildId, m_IsSelfCollision, m_IsCollisionReceiver, m_IsCollisionRepulsor, m_CollisionBatchNo, m_ConcaveRatio}
// Only for links that reach the "ordinary link" branch (not LRA, type not 3/4/5) or the self-collision branch.
function setupLinkCollision(core, d, link, indexBase) {
  const scene = core.f3778, P = scene.f0; indexBase = indexBase || 0;
  const ia = d.m_ParentId + indexBase, ib = d.m_ChildId + indexBase;
  if (d.m_IsSelfCollision) {
    if (scene.f7A0 === undefined || scene.f7A0 !== scene.f60.length) { scene.f60.push(fn_143ae62b0({}, 9, P[ia], P[ib], null)); scene.f68 = scene.f60.length; }
    return;
  }
  if (d.m_IsCollisionReceiver && link) {
    if (P[ia].f198 === P[ib] && P[ia].f27C === 2) return;                 // jumps past the repulsor part as well
    const o = fn_143aec160(scene, ia, ib, -1); link.f30 = o;
    if (o) { o.f1C = d.m_CollisionBatchNo; o.f64 = d.m_ConcaveRatio; }
  }
  if (d.m_IsCollisionRepulsor) {
    const o = fn_143aec680(scene, 2, ia, ib);
    if (o) { o.f64 = d.m_ConcaveRatio; const tag = core.f744[o.f38]; if (tag !== 0) fn_143aedb50(scene, tag - 1, o); }
  }
}
// end of setup: sort scene+0x50 (NOTE: p.f2A0 / link.f30 of dynamic-side objects are raw addresses into that array in
// the exe and are not fixed up by the sort; here they stay object references)
function setupFinishCollision(core) { fn_143ae4440(core.f3778.f50); }
// core+0x400: per bone "driver particle index + 1" (0 none, 9999 static) - end of 0x143b07550 [C]
function setupBoneMap(core, nBones) {
  const m = core.f400 = new Array(nBones).fill(0); const P = core.f3778.f0;
  for (let i = 0; i < P.length; i++) {
    const p = P[i];
    if (!p.f276) m[p.f248] = i + 1;
    else if (!p.f277) { const c = p.f198; if (!c || c.f276) { if (m[p.f248] === 0) m[p.f248] = 9999; } else m[p.f248] = i + 1; }
  }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x143b1b9f0 (rcx = p, rdx = plane object o): particle vs plane
function fn_143b1b9f0(p, o) {
  if (p === o.f0 || p === o.f8 || p === o.f10) return;
  const n = o.f20, q = o.f2C, x = p.f0;
  const d = f(f(f(f(x[1] - q[1]) * n[1]) + f(f(x[0] - q[0]) * n[0])) + f(f(x[2] - q[2]) * n[2]));
  const r = p.f25C;
  if (d >= r) return;
  const pen = f(r - d);
  const cx = f(n[0] * pen), cy = f(n[1] * pen), cz = f(n[2] * pen);
  if (p.f276) return;
  x[0] = f(cx + x[0]); x[2] = f(cz + x[2]); x[1] = f(cy + x[1]);
  p.f279 = 1;
  p.f64 = [f(cx + p.f64[0]), f(cy + p.f64[1]), f(cz + p.f64[2])];
  const vy = f(x[1] - p.fC[1]), vz = f(x[2] - p.fC[2]), vx = f(x[0] - p.fC[0]);
  const vn = f(f(f(vx * n[0]) + f(vy * n[1])) + f(vz * n[2]));
  const tx = f(vx - f(n[0] * vn)), ty = f(vy - f(n[1] * vn)), tz = f(vz - f(n[2] * vn));
  const fr = p.f268, lim = f(fr * pen);
  const tt = f(f(f(ty * ty) + f(tx * tx)) + f(tz * tz));
  if (tt >= f(lim * lim)) { x[0] = f(x[0] - f(tx * fr)); x[1] = f(x[1] - f(fr * ty)); x[2] = f(x[2] - f(fr * tz)); }
  else { x[0] = f(x[0] - tx); x[1] = f(x[1] - ty); x[2] = f(x[2] - tz); }
}

// 0x143b1c700 (rcx = p1, rdx = p2, r8 = off vec3*, xmm3 = R): sphere vs sphere
function fn_143b1c700(p1, p2, off, R) {
  if (!(R >= C_1E4)) R = p2.f25C;
  const sum = f(p1.f25C + R);
  const a = p1.f0, b = p2.f0;
  const p2x = b[0], p2y = b[1], p2z = b[2], p1y = a[1], p1z = a[2];
  let nx = f(a[0] - f(p2x + off[0])), ny = f(p1y - f(p2y + off[1])), nz = f(p1z - f(p2z + off[2]));
  const dd = f(f(f(ny * ny) + f(nx * nx)) + f(nz * nz));
  if (dd >= f(sum * sum)) return;
  if (!(dd > 0)) return;
  const k1 = p1.f276;
  const L = sqrt(dd); let pen = f(sum - L); const inv = f(1 / L);
  ny = f(ny * inv); nz = f(nz * inv); nx = f(nx * inv);
  if (!k1 && !p2.f276) pen = f(pen * 0.5);
  let vx = Z[0], vy = Z[1], vz = Z[2];
  if (!k1) { vx = f(vx + f(a[0] - p1.fC[0])); vy = f(vy + f(p1y - p1.fC[1])); vz = f(vz + f(p1z - p1.fC[2])); }
  if (!p2.f276) { vx = f(vx - f(p2x - p2.fC[0])); vy = f(vy - f(p2y - p2.fC[1])); vz = f(vz - f(p2z - p2.fC[2])); }
  const vn = f(f(f(nx * vx) + f(ny * vy)) + f(nz * vz));
  const tx = f(vx - f(nx * vn)), ty = f(vy - f(ny * vn)), tz = f(vz - f(nz * vn));
  const tt = f(f(f(ty * ty) + f(tx * tx)) + f(tz * tz));
  if (!k1) {
    const cy = f(ny * pen), cx = f(nx * pen), cz = f(nz * pen);
    a[0] = f(cx + a[0]); a[2] = f(cz + a[2]); const y1 = f(cy + a[1]); a[1] = y1;
    p1.f279 = 1;
    p1.f64 = [f(cx + p1.f64[0]), f(cy + p1.f64[1]), f(cz + p1.f64[2])];
    const g = f(pen * p1.f268);
    if (tt >= f(g * g)) {
      if (tt > 0) { let s = f(g / sqrt(tt)); s = s < 1 ? s : 1; a[0] = f(a[0] - f(s * tx)); a[1] = f(a[1] - f(ty * s)); a[2] = f(a[2] - f(tz * s)); }
    } else { a[1] = f(y1 - ty); a[0] = f(a[0] - tx); a[2] = f(a[2] - tz); }
  }
  if (!p2.f276) {
    const cx = f(nx * pen), cy = f(ny * pen), cz = f(nz * pen);
    b[0] = f(b[0] - cx); b[1] = f(b[1] - cy); b[2] = f(b[2] - cz);
    p2.f279 = 1;
    p2.f64 = [f(p2.f64[0] - cx), f(p2.f64[1] - cy), f(p2.f64[2] - cz)];
    const g = f(pen * p2.f268);
    if (tt >= f(g * g)) {
      if (!(tt > 0)) return;
      let s = f(g / sqrt(tt)); s = s < 1 ? s : 1;
      b[0] = f(f(s * tx) + b[0]); b[2] = f(f(tz * s) + b[2]); b[1] = f(f(ty * s) + b[1]);
    } else { b[0] = f(tx + b[0]); b[2] = f(tz + b[2]); b[1] = f(ty + b[1]); }
  }
}

// radius of a capsule at parameter t with concave ratio c (inlined in 0x143b1b110 / 0x143b15850)
function capRadius(t, rA, rB, c) {
  let R = f(f(f(1 - t) * rA) + f(t * rB));
  if (Math.abs(c) > C_1E4) { const u = f(t - 0.5); R = f(f(f(f(f(u * 4) * u) * R) * c) + f(f(1 - c) * R)); }
  return R;
}

// 0x143b1b110 (rcx = p, rdx = a, r8 = b, xmm3 = concave, [rsp+0x28] = off vec3*, [rsp+0x30] = R): sphere p vs capsule a-b
function fn_143b1b110(p, a, b, concave, off, R) {
  if (p === a || p === b) return;
  const x = p.f0, A = a.f0, B = b.f0;
  const Ax = f(off[0] + A[0]), Ay = f(off[1] + A[1]), Az = f(off[2] + A[2]);
  const ex = f(f(off[0] + B[0]) - Ax), ey = f(f(off[1] + B[1]) - Ay), ez = f(f(off[2] + B[2]) - Az);
  let t = f(f(f(f(x[1] - Ay) * ey) + f(f(x[0] - Ax) * ex)) + f(f(x[2] - Az) * ez));
  const ee = f(f(f(ey * ey) + f(ex * ex)) + f(ez * ez));
  if (t > 0) { if (ee > t) t = f(t / ee); else t = 1; } else t = 0;
  let dx = f(x[0] - f(f(ex * t) + Ax)), dy = f(x[1] - f(f(ey * t) + Ay)), dz = f(x[2] - f(f(ez * t) + Az));
  const dd = f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz));
  if (!(R >= C_1E4)) {
    R = f(f(f(1 - t) * a.f25C) + f(t * b.f25C));
    if (Math.abs(concave) > C_1E4) { const u = f(t - 0.5); R = f(f(f(f(f(u * 4) * u) * R) * concave) + f(f(1 - concave) * R)); }
  }
  const sum = f(R + p.f25C);
  if (dd >= f(sum * sum)) return;
  if (!(dd > 0)) return;
  const k = p.f276;
  const L = sqrt(dd); let pen = f(sum - L); const inv = f(1 / L);
  const nx = f(inv * dx), ny = f(inv * dy), nz = f(inv * dz);
  if (!k) { if (!a.f276) pen = f(pen * 0.5); }
  else pen = f(pen / f(Math.abs(f(0.5 - t)) + 0.5));
  let vx = Z[0], vy = Z[1], vz = Z[2];
  if (!k) { vx = f(vx + f(x[0] - p.fC[0])); vy = f(vy + f(x[1] - p.fC[1])); vz = f(vz + f(x[2] - p.fC[2])); }
  if (!a.f276) {
    const w = f(1 - t);
    vx = f(vx - f(f(f(A[0] - a.fC[0]) * w) + f(f(B[0] - b.fC[0]) * t)));
    vy = f(vy - f(f(f(A[1] - a.fC[1]) * w) + f(f(B[1] - b.fC[1]) * t)));
    vz = f(vz - f(f(f(A[2] - a.fC[2]) * w) + f(f(B[2] - b.fC[2]) * t)));
  }
  const vn = f(f(f(vy * ny) + f(vx * nx)) + f(vz * nz));
  let tx = f(vx - f(nx * vn)), ty = f(vy - f(ny * vn)), tz = f(vz - f(nz * vn));
  const tt = f(f(f(ty * ty) + f(tx * tx)) + f(tz * tz));
  if (!k) {
    const cy = f(ny * pen), cx = f(nx * pen), cz = f(nz * pen);
    x[0] = f(cx + x[0]); x[2] = f(cz + x[2]); const y1 = f(cy + x[1]); x[1] = y1;
    p.f279 = 1;
    p.f64 = [f(cx + p.f64[0]), f(cy + p.f64[1]), f(cz + p.f64[2])];
    const g = f(pen * p.f268);
    if (tt >= f(g * g)) {
      if (tt > 0) { let s = f(g / sqrt(tt)); s = s < 1 ? s : 1; x[0] = f(x[0] - f(s * tx)); x[1] = f(x[1] - f(ty * s)); x[2] = f(x[2] - f(tz * s)); }
    } else { x[1] = f(y1 - ty); x[0] = f(x[0] - tx); x[2] = f(x[2] - tz); }
  }
  if (!a.f276) {
    const w = f(1 - t); let g = f(w * pen);
    const cx = f(nx * g), cy = f(ny * g), cz = f(nz * g);
    A[0] = f(A[0] - cx); A[1] = f(A[1] - cy); A[2] = f(A[2] - cz);
    a.f279 = 1;
    a.f64 = [f(a.f64[0] - cx), f(a.f64[1] - cy), f(a.f64[2] - cz)];
    g = f(g * a.f268);
    if (tt >= f(g * g)) {
      if (tt > 0) { let s = f(g / sqrt(tt)); s = s < 1 ? s : 1; const sx = f(tx * s), sy = f(s * ty); A[2] = f(f(s * tz) + A[2]); A[1] = f(sy + A[1]); A[0] = f(sx + A[0]); }
    } else { const sx = f(tx * w), sy = f(ty * w); A[2] = f(f(w * tz) + A[2]); A[1] = f(sy + A[1]); A[0] = f(sx + A[0]); }
  }
  if (!b.f276) {
    let g = f(pen * t);
    const cx = f(nx * g), cy = f(ny * g), cz = f(nz * g);
    B[0] = f(B[0] - cx); B[1] = f(B[1] - cy); B[2] = f(B[2] - cz);
    b.f279 = 1;
    b.f64 = [f(b.f64[0] - cx), f(b.f64[1] - cy), f(b.f64[2] - cz)];
    g = f(g * a.f268);                                   // friction of a (rdx), as coded
    if (tt >= f(g * g)) {
      if (!(tt > 0)) return;
      let s = f(g / sqrt(tt)); s = s < 1 ? s : 1; tx = f(tx * s); ty = f(ty * s); tz = f(tz * s);
    } else { tx = f(tx * t); ty = f(ty * t); tz = f(tz * t); }
    B[2] = f(tz + B[2]); B[1] = f(ty + B[1]); B[0] = f(tx + B[0]);
  }
}

// 0x143b15850 (rcx = a1, rdx = a2, xmm2 = c1, r9 = b1, [rsp+0x28] = b2, [rsp+0x30] = c2, [rsp+0x38] = off vec3*,
//              [rsp+0x40] = R): capsule a1-a2 vs capsule b1-b2 (off is added to b1/b2; R > 0 overrides the b radius)
function fn_143b15850(a1, a2, c1, b1, b2, c2, off, R) {
  if (a1 === b1 || a1 === b2 || b1 === a2) return;
  const A1 = a1.f0, A2 = a2.f0, B1 = b1.f0, B2 = b2.f0;
  const a1x = A1[0];
  const b1x = f(B1[0] + off[0]), b1y = f(B1[1] + off[1]), b1z = f(B1[2] + off[2]);
  const d1x = f(A2[0] - a1x), d1y = f(A2[1] - A1[1]), d1z = f(A2[2] - A1[2]);
  const rx = f(a1x - b1x), ry = f(A1[1] - b1y), rz = f(A1[2] - b1z);
  const d2x = f(f(B2[0] + off[0]) - b1x), d2y = f(f(B2[1] + off[1]) - b1y), d2z = f(f(B2[2] + off[2]) - b1z);
  const a = f(f(f(d1y * d1y) + f(d1x * d1x)) + f(d1z * d1z));
  const b = f(f(f(d2y * d1y) + f(d2x * d1x)) + f(d2z * d1z));
  const e = f(f(f(d2y * d2y) + f(d2x * d2x)) + f(d2z * d2z));
  const c = f(f(f(ry * d1y) + f(rx * d1x)) + f(rz * d1z));
  const ff = f(f(f(ry * d2y) + f(rx * d2x)) + f(rz * d2z));
  const D = f(f(e * a) - f(b * b));
  let sN, sD = D, tN = ff, tD = D;
  if (D >= C_1E9) {
    sN = f(f(ff * b) - f(c * e));
    tN = f(f(ff * a) - f(c * b));
    if (sN >= 0) {
      if (sN > sD) { sN = sD; tN = f(ff + b); tD = e; }
    } else { sN = 0; tN = ff; tD = e; }
  } else { sN = 0; sD = 1; tD = e; }
  if (tN >= 0) {
    if (tN > tD) {
      sN = f(b - c); tN = tD;
      if (sN >= 0) { if (sN > a) sN = sD; else sD = a; } else sN = 0;
    }
  } else {
    tN = 0;
    if (c > 0) sN = 0;                                   // comiss c, -0.0 ; ja
    else { sN = f(-c); if (sN > a) sN = sD; else sD = a; }
  }
  const s = (Math.abs(sN) >= C_1E9) ? f(sN / sD) : 0;
  const t = (Math.abs(tN) >= C_1E9) ? f(tN / tD) : 0;
  const w1 = f(1 - s);
  const px = f(f(f(d1x * s) + rx) - f(d2x * t));
  const py = f(f(f(d1y * s) + ry) - f(d2y * t));
  const pz = f(f(f(d1z * s) + rz) - f(d2z * t));
  const dd = f(f(f(py * py) + f(px * px)) + f(pz * pz));
  let R1 = f(f(s * a2.f25C) + f(w1 * a1.f25C));
  if (Math.abs(c1) > C_1E4) { const u = f(s - 0.5); R1 = f(f(f(f(f(u * 4) * u) * R1) * c1) + f(f(1 - c1) * R1)); }
  if (!(R >= C_1E4)) {
    R = f(f(f(1 - t) * b1.f25C) + f(t * b2.f25C));
    if (Math.abs(c2) > C_1E4) { const u = f(t - 0.5); R = f(f(f(f(f(u * 4) * u) * R) * c2) + f(f(1 - c2) * R)); }
  }
  const sum = f(R1 + R);
  if (dd >= f(sum * sum)) return;
  if (!(dd > 0)) return;
  const k1 = a1.f276;
  const L = sqrt(dd); let pen = f(sum - L); const inv = f(1 / L);
  const nx = f(inv * px), ny = f(inv * py), nz = f(inv * pz);
  if (!k1 && !b1.f276) pen = f(pen * 0.5);
  const k2 = b1.f276;
  const w2 = f(1 - t);
  let g1 = f(w1 * pen), g2 = f(s * pen), g4 = f(t * pen), g3 = f(w2 * pen);
  if (k2) { if (!k1) { g1 = f(g1 + g1); g2 = f(g2 + g2); } }
  else if (k1) { g3 = f(g3 + g3); g4 = f(g4 + g4); }
  let vx = Z[0], vy = Z[1], vz = Z[2];
  if (!k1) {
    vx = f(vx + f(f(f(a1x - a1.fC[0]) * w1) + f(f(A2[0] - a2.fC[0]) * s)));
    vy = f(vy + f(f(f(A1[1] - a1.fC[1]) * w1) + f(f(A2[1] - a2.fC[1]) * s)));
    vz = f(vz + f(f(f(A1[2] - a1.fC[2]) * w1) + f(f(A2[2] - a2.fC[2]) * s)));
  }
  if (!k2) {
    vx = f(vx - f(f(f(B1[0] - b1.fC[0]) * w2) + f(f(B2[0] - b2.fC[0]) * t)));
    vy = f(vy - f(f(f(B1[1] - b1.fC[1]) * w2) + f(f(B2[1] - b2.fC[1]) * t)));
    vz = f(vz - f(f(f(B1[2] - b1.fC[2]) * w2) + f(f(B2[2] - b2.fC[2]) * t)));
  }
  const vn = f(f(f(vy * ny) + f(vx * nx)) + f(vz * nz));
  let tx = f(vx - f(vn * nx)), ty = f(vy - f(vn * ny)), tz = f(vz - f(vn * nz));
  const tt = f(f(f(ty * ty) + f(tx * tx)) + f(tz * tz));
  if (!k1) {
    const cx = f(nx * g1), cy = f(ny * g1), cz = f(nz * g1);
    A1[0] = f(a1x + cx); A1[1] = f(cy + A1[1]); A1[2] = f(cz + A1[2]);
    a1.f279 = 1;
    a1.f64 = [f(cx + a1.f64[0]), f(cy + a1.f64[1]), f(cz + a1.f64[2])];
    const g = f(g1 * a1.f268);
    if (tt >= f(g * g)) {
      if (tt > 0) { let k = f(g / sqrt(tt)); k = k < 1 ? k : 1; A1[0] = f(A1[0] - f(k * tx)); A1[1] = f(A1[1] - f(ty * k)); A1[2] = f(A1[2] - f(tz * k)); }
    } else { A1[0] = f(A1[0] - f(tx * w1)); A1[1] = f(A1[1] - f(ty * w1)); A1[2] = f(A1[2] - f(w1 * tz)); }
  }
  if (!a2.f276) {
    const cx = f(nx * g2), cy = f(ny * g2), cz = f(nz * g2);
    A2[0] = f(cx + A2[0]); A2[1] = f(cy + A2[1]); A2[2] = f(cz + A2[2]);
    a2.f279 = 1;
    a2.f64 = [f(cx + a2.f64[0]), f(cy + a2.f64[1]), f(cz + a2.f64[2])];
    const g = f(g2 * a1.f268);                           // friction of a1 (rcx), as coded
    if (tt >= f(g * g)) {
      if (tt > 0) { let k = f(g / sqrt(tt)); k = k < 1 ? k : 1; A2[0] = f(A2[0] - f(k * tx)); A2[1] = f(A2[1] - f(ty * k)); A2[2] = f(A2[2] - f(tz * k)); }
    } else { A2[0] = f(A2[0] - f(tx * s)); A2[1] = f(A2[1] - f(ty * s)); A2[2] = f(A2[2] - f(tz * s)); }
  }
  if (!b1.f276) {
    const cx = f(nx * g3), cy = f(ny * g3), cz = f(nz * g3);
    B1[0] = f(B1[0] - cx); B1[1] = f(B1[1] - cy); B1[2] = f(B1[2] - cz);
    b1.f279 = 1;
    b1.f64 = [f(b1.f64[0] - cx), f(b1.f64[1] - cy), f(b1.f64[2] - cz)];
    const g = f(g3 * b1.f268);
    if (tt >= f(g * g)) {
      if (tt > 0) { let k = f(g / sqrt(tt)); k = k < 1 ? k : 1; B1[0] = f(f(tx * k) + B1[0]); B1[1] = f(f(ty * k) + B1[1]); B1[2] = f(f(tz * k) + B1[2]); }
    } else { B1[1] = f(f(ty * w2) + B1[1]); B1[0] = f(f(w2 * tx) + B1[0]); B1[2] = f(f(tz * w2) + B1[2]); }
  }
  if (!b2.f276) {
    const cx = f(nx * g4), cy = f(ny * g4), cz = f(nz * g4);
    B2[0] = f(B2[0] - cx); B2[1] = f(B2[1] - cy); B2[2] = f(B2[2] - cz);
    b2.f279 = 1;
    b2.f64 = [f(b2.f64[0] - cx), f(b2.f64[1] - cy), f(b2.f64[2] - cz)];
    const g = f(g4 * b2.f268);
    if (tt >= f(g * g)) {
      if (!(tt > 0)) return;
      let k = f(g / sqrt(tt)); k = k < 1 ? k : 1; tx = f(tx * k); ty = f(ty * k); tz = f(tz * k);
    } else { tx = f(tx * t); ty = f(ty * t); tz = f(tz * t); }
    B2[2] = f(tz + B2[2]); B2[1] = f(ty + B2[1]); B2[0] = f(tx + B2[0]);
  }
}

// bound-sphere reject shared by the object-object tests: true = overlapping
function boundsOverlap(S, Cc) {
  const dy = f(S.f74[1] - Cc.f74[1]), dx = f(S.f74[0] - Cc.f74[0]), dz = f(S.f74[2] - Cc.f74[2]);
  const rr = f(S.f80 + Cc.f80);
  return !(f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz)) >= f(rr * rr));
}
// 0x143b13a20 (rcx = S sphere object, rdx = collider C); jump table 0x143b13b50
function fn_143b13a20(S, Cc) {
  switch (Cc.f18) {
    case 1: fn_143b1c700(S.f0, Cc.f0, [0, 0, 0], 0); return;
    case 2: if (boundsOverlap(S, Cc)) fn_143b1b110(S.f0, Cc.f0, Cc.f8, Cc.f64, [0, 0, 0], 0); return;
    case 3: throw new Error('not transcribed: 0x143b1bc20 (inverse sphere)');
    case 4: throw new Error('not transcribed: 0x143b16420 (inverse capsule)');
    case 6: fn_143b1b9f0(S.f0, Cc); return;
    case 8: throw new Error('not transcribed: 0x143b1aec0 (box)');
  }
}
// 0x143b13620 (rcx = S capsule object, rdx = collider C); jump table 0x143b138dc
function fn_143b13620(S, Cc) {
  switch (Cc.f18) {
    case 1: if (boundsOverlap(S, Cc)) fn_143b1b110(Cc.f0, S.f0, S.f8, S.f64, [0, 0, 0], 0); return;
    case 2:
      if (boundsOverlap(S, Cc)) {
        fn_143b1b110(S.f0, Cc.f0, Cc.f8, Cc.f64, [0, 0, 0], 0);
        fn_143b1b110(S.f8, Cc.f0, Cc.f8, Cc.f64, [0, 0, 0], 0);
        fn_143b15850(S.f0, S.f8, S.f64, Cc.f0, Cc.f8, Cc.f64, [0, 0, 0], 0);
      }
      return;
    case 3: throw new Error('not transcribed: 0x143b1bc20 (inverse sphere)');
    case 4: throw new Error('not transcribed: 0x143b16420 (inverse capsule)');
    case 6: { const B = S.f8; fn_143b1b9f0(S.f0, Cc); fn_143b1b9f0(B, Cc); return; }
    case 8: throw new Error('not transcribed: 0x143b1aec0 (box)');
  }
}
// 0x143b13900 (rcx = S plane object, rdx = collider C)
function fn_143b13900(S, Cc) {
  const t = Cc.f18;
  if (t === 1) { if (boundsOverlap(S, Cc)) throw new Error('not transcribed: 0x143b1d720 (limit plane vs sphere)'); }
  else if (t === 2) { if (boundsOverlap(S, Cc)) throw new Error('not transcribed: 0x143b1cb50 (limit plane vs capsule)'); }
  else if (t === 6) { fn_143b1b9f0(S.f0, Cc); fn_143b1b9f0(S.f8, Cc); fn_143b1b9f0(S.f10, Cc); }
}
function vsList(S, list, n, pred) {
  const t = S.f18;
  if (t !== 1 && t !== 2 && t !== 6) return;
  for (let i = 0; i < n; i++) {
    const Cc = list[i];
    if (pred && !pred(Cc)) continue;
    const A = Cc.f0, B = Cc.f8, D = Cc.f10;
    let sa, sb, sd;
    if (A) { sa = A.f276; A.f276 = 1; }
    if (B) { sb = B.f276; B.f276 = 1; }
    if (D) { sd = D.f276; D.f276 = 1; }
    if (t === 1) fn_143b13a20(S, Cc); else if (t === 2) fn_143b13620(S, Cc); else fn_143b13900(S, Cc);
    if (A) A.f276 = sa;
    if (B) B.f276 = sb;
    if (D) D.f276 = sd;
  }
}
// 0x143ae4ef0 (rcx = S, rdx = list, r8d = count, r9 unused): S vs every collider of a group list (colliders forced kinematic)
function fn_143ae4ef0(S, list, n) { vsList(S, list, n, null); }
// 0x143ae4c30 (rcx = S, rdx = list, r8d = count, r9 = core+0x658): same, skipping colliders whose group state < 2
function fn_143ae4c30(S, list, n, core) { vsList(S, list, n, Cc => core.f804[Cc.f38] >= 2); }

// 0x143b1aba0 (rcx = scene): self-collision pairs (scene+0x60)
function fn_143b1aba0(scene) {
  for (let i = 0; i < scene.f68; i++) {
    const o = scene.f60[i], A = o.f0, B = o.f8;
    if (!A || !B) continue;
    const sa = A.f27C, sb = B.f27C;
    if (sa === 1) {
      if (sb === 1) fn_143b1c700(A, B, [0, 0, 0], 0);
      else if (sb === 2) { if (B.f198) fn_143b1b110(A, B, B.f198, B.f284, [0, 0, 0], 0); }
      else if (sb === 6) { if (B.f2A0) fn_143b1b9f0(A, B.f2A0); }
    } else if (sa === 2) {
      if (sb === 1) { if (A.f198) fn_143b1b110(B, A, A.f198, A.f284, [0, 0, 0], 0); }
      else if (sb === 2) { if (A.f198 && B.f198) fn_143b15850(A, A.f198, A.f284, B, B.f198, B.f284, [0, 0, 0], 0); }
      else if (sb === 6) { if (B.f2A0) { fn_143b1b9f0(A, B.f2A0); if (A.f198) fn_143b1b9f0(A.f198, B.f2A0); } }
    } else if (sa === 6) {
      if (sb === 1) { if (A.f2A0) fn_143b1b9f0(B, A.f2A0); }
      else if (sb === 2) { if (A.f2A0) { fn_143b1b9f0(B, A.f2A0); if (B.f198) fn_143b1b9f0(B.f198, A.f2A0); } }
    }
  }
}

// 0x143b1a5c0 (rcx = core): colliders of other Bonamik cores (list core+0x3748, count core+0x3750, stride 0x30)
function fn_143b1a5c0(core) {
  if (!core.f3750) return;
  throw new Error('not transcribed: 0x143b1a665..0x143b1ab03 (external core colliders)');
}

// ---------------------------------------------------------------------------------------------------------------
// 0x143b12da0 (rcx = p, rdx = point vec3*, r8 = normal vec3*): ground plane clamp; returns 1 if the particle was moved
function fn_143b12da0(p, pt, n) {
  const x = p.f0;
  const d = f(f(f(f(x[1] - pt[1]) * n[1]) + f(f(x[0] - pt[0]) * n[0])) + f(f(x[2] - pt[2]) * n[2]));
  const r = p.f25C;
  if (d >= r) return 0;
  const ox = x[0], oy = x[1], oz = x[2];
  const pen = f(r - d);
  const cx = f(n[0] * pen), cy = f(n[1] * pen), cz = f(n[2] * pen);
  x[0] = f(cx + ox); x[2] = f(cz + x[2]); x[1] = f(cy + x[1]);
  p.f64 = [f(cx + p.f64[0]), f(cy + p.f64[1]), f(cz + p.f64[2])];
  const my = f(x[1] - oy), mz = f(x[2] - oz), mx = f(x[0] - ox);
  const mn = f(f(f(mx * n[0]) + f(my * n[1])) + f(mz * n[2]));
  const tx = f(mx - f(n[0] * mn)), ty = f(my - f(n[1] * mn)), tz = f(mz - f(n[2] * mn));
  const fr = p.f268, lim = f(fr * pen);
  const tt = f(f(f(ty * ty) + f(tx * tx)) + f(tz * tz));
  let y, z;
  if (tt >= f(lim * lim)) { x[0] = f(x[0] - f(tx * fr)); y = f(x[1] - f(fr * ty)); z = f(x[2] - f(fr * tz)); }
  else { x[0] = f(x[0] - tx); y = f(x[1] - ty); z = f(x[2] - tz); }
  x[2] = z; x[1] = y;
  p.f70 = [f(f(x[0] - ox) + p.f70[0]), f(f(y - oy) + p.f70[1]), f(f(z - oz) + p.f70[2])];
  return 1;
}

// 0x143b19d80 (rcx = core, rdx = scene, r8b = queryOk, r9 = pos vec3*, [rsp+0x28] = normal vec3*, [rsp+0x30] = core+0x658)
function fn_143b19d80(core, scene, queryOk, pos, normal) {
  for (let i = 0; i < scene.f8; i++) {
    const p = scene.f0[i];
    if (p.f276) continue;
    if (!core.f924[p.f2B8]) continue;
    if (p.f31C && !scene.f7AC) throw new Error('not transcribed: 0x143b19e6b..0x143b1a3ca (ground-attach sphere sweep)');
    if (!queryOk) continue;
    fn_143b12da0(p, pos, normal);
    p.f88 = [f(p.f88[0] + normal[0]), f(normal[1] + p.f88[1]), f(normal[2] + p.f88[2])];
  }
}

// Fragment of Step 0x143b0cb60 (143b0cc4c..143b0ce5d): ground query + world -> component conversion.
// owner.vf10(posOut, normalOut) -> bool (world space). Returns {valid, ok, pos, normal} (valid = r12b, ok = [rsp+0x51]).
function stepGroundQuery(core) {
  const pos = Z.slice(), normal = [0, 0, 1];
  const ok = core.f3848.vf10(pos, normal) ? 1 : 0;
  let valid = core.f3718 ? 1 : 0;
  let pc = pos, nc = normal;
  if (ok) {
    valid = 1;
    const q = core.fD0, s = core.fF0, t = core.fE0;
    pc = add(qrot(q, [f(pos[0] * s[0]), f(pos[1] * s[1]), f(pos[2] * s[2])]), t);
    nc = qrot(q, normal);
  }
  return { valid, ok, pos: pc, normal: nc };
}
// Fragment of Step 0x143b0cb60 (143b0d65b..143b0d7ed): the collision iterations of one sub-step
function stepCollision(core, scene, g) {
  for (let it = 0; it < core.f680; it++) {
    fn_143b1a5c0(core);
    for (let c = 0; c < 48; c++) { const L = scene.f70[c]; for (let i = 0; i < cnt(L); i++) { if (L[i].f18 === 6) fn_143afc140(L[i]); else fn_143b10190(L[i]); } }
    for (let i = 0; i < scene.f58; i++) {
      const S = scene.f50[i];
      if (core.f804[S.f38] < 2) continue;
      if (S.f18 === 6) fn_143afc140(S); else fn_143b10190(S);
      for (let k = 0; k < 8; k++) {
        const c = (S.f18 === 6) ? S.f3C[k] : S.f0.f2BC[k];      // re-evaluated per k as in the exe
        if (c === -1) continue;
        if (c < 48) { if (core.f804[c] >= 2) fn_143ae4ef0(S, scene.f70[c] || [], cnt(scene.f70[c])); }
        else fn_143ae4c30(S, scene.f70[c] || [], cnt(scene.f70[c]), core);
      }
    }
    fn_143b1aba0(scene);
    if (g.valid && core.f65D) fn_143b19d80(core, scene, g.ok, g.pos, g.normal);
  }
}

// 0x143b10230 (rcx = core, xmm1 = dt): Attach. Part 1: multiple-collision-object particles follow their driver particle.
function fn_143b10230(core, dt) {
  const scene = core.f3778, P = scene.f0;
  for (let i = 0; i < scene.f8; i++) {
    const p = P[i];
    if (!p.f278) continue;
    const m = core.f400[p.f248];
    if (m === 0 || m === 9999) continue;
    const Q = P[m - 1];
    p.f0 = add(qrot(Q.f20, p.f170), Q.f0);      // (q x t + (w*t + L)) + Q.pos, t = 2 (q x L): same value as qrot + add
    p.f20 = qmul(Q.f20, p.f180);                // SSE quaternion product, sign vectors 0x149D52870/60/50 (.bss) [U shape]
  }
  if (core.f37E8 > 0) throw new Error('not transcribed: 0x143b104ef..0x143b107fb (ground attach list)');
}

module.exports = {
  fn_143b10190, fn_143afc140, fn_143ae62b0, fn_143aec160, fn_143aec560, fn_143aec680, fn_143aedb50, fn_143ae4440,
  fn_143b1b9f0, fn_143b1c700, fn_143b1b110, fn_143b15850, fn_143b13a20, fn_143b13620, fn_143b13900,
  fn_143ae4ef0, fn_143ae4c30, fn_143b1aba0, fn_143b1a5c0, fn_143b12da0, fn_143b19d80, fn_143b10230,
  initCollisionLists, setupPlanes, setupBodyObjects, setupLinkCollision, setupFinishCollision, setupBoneMap,
  stepGroundQuery, stepCollision, capRadius, boundsOverlap,
};

// ---------------------------------------------------------------------------------------------------------------
if (require.main === module) {
  const mk = (i, pos, o) => Object.assign({
    f0: pos.map(f), fC: pos.map(f), f20: [0, 0, 0, 1], f64: [0, 0, 0], f70: [0, 0, 0], f88: [0, 0, 0],
    f170: [0, 0, 0], f180: [0, 0, 0, 1], f190: null, f198: null, f248: i, f24C: i, f25C: 1, f268: f(0.05), f276: 0, f277: 0,
    f278: 0, f279: 0, f27C: 1, f280: 0, f284: 0, f2A0: null, f2B8: 0, f2BC: [-1, -1, -1, -1, -1, -1, -1, -1], f31C: 0 }, o || {});
  // 0 dyn sphere, 1-2 dyn capsule, 3-4 kinematic capsule collider (group 1), 5 kinematic sphere collider (group 1),
  // 6 kinematic plane collider (group 2), 7 multi-collision sphere on bone 1
  const P = [
    mk(0, [0, 0.5, 0.2], { f2BC: [1, 2, -1, -1, -1, -1, -1, -1], fC: [0, 0.6, 0.3] }),
    mk(1, [3, 0.2, 0], { f27C: 2, f2BC: [1, 2, -1, -1, -1, -1, -1, -1], f284: f(0.1), fC: [3.1, 0.2, 0.1] }),
    mk(2, [5, 0.2, 0.5], { f2BC: [1, -1, -1, -1, -1, -1, -1, -1] }),
    mk(3, [-1, 0, 0], { f276: 1, f277: 1, f27C: 2, f2B8: 1 }),
    mk(4, [6, 0, 0], { f276: 1, f277: 0, f2B8: 1 }),
    mk(5, [0, 1.2, 0.4], { f276: 1, f277: 1, f27C: 1, f2B8: 1 }),
    mk(6, [0, 0, -0.5], { f276: 1, f277: 1, f27C: 6, f2B8: 2, f20: [0, f(-Math.SQRT1_2), 0, f(Math.SQRT1_2)] }),
    mk(7, [0, 0, 0], { f276: 1, f277: 1, f278: 1, f27C: 1, f2B8: 3, f248: 1, f170: [0, 1, 0] }),
  ];
  P[1].f198 = P[2]; P[2].f190 = P[1]; P[3].f198 = P[4];
  const scene = { f0: P, f8: P.length, f7AC: 0 };
  const core = { f3778: scene, f658: 2401, f680: 1, f65D: 1, f3718: 0, f3750: 0, f37E8: 0,
    f804: [2, 2, 2, 2], f744: [0, 0, 0, 0], f924: [1, 1, 1, 1],
    fD0: [0, 0, 0, 1], fE0: [0, 0, 0], fF0: [1, 1, 1],
    f3848: { vf10: (pos, n) => { pos[0] = 0; pos[1] = 0; pos[2] = f(-0.2); n[0] = 0; n[1] = 0; n[2] = 1; return true; } } };
  initCollisionLists(scene);
  setupPlanes(core, []);
  setupBodyObjects(core);
  setupLinkCollision(core, { m_ParentId: 0, m_ChildId: 6, m_IsSelfCollision: true }, null);
  setupLinkCollision(core, { m_ParentId: 0, m_ChildId: 2, m_IsCollisionReceiver: true, m_IsCollisionRepulsor: false, m_CollisionBatchNo: 1, m_ConcaveRatio: 0 }, {});
  setupFinishCollision(core);
  setupBoneMap(core, 8);
  console.log('dynamic-side objects:', scene.f50.map(o => `type${o.f18} batch${o.f1C} r${o.f80}`).join(' | '));
  console.log('group lists:', scene.f70.map((l, g) => l ? g + ':' + l.map(o => 'type' + o.f18).join(',') : '').filter(Boolean).join(' | '));
  console.log('bone map:', core.f400.join(','));
  fn_143b10230(core, f(0.05));
  console.log('multi-collision particle 7 ->', P[7].f0, P[7].f20);
  const g = stepGroundQuery(core);
  console.log('ground', g);
  stepCollision(core, scene, g);
  for (const p of P.slice(0, 3)) console.log('p' + p.f24C, p.f0, 'f64', p.f64, 'f279', p.f279, 'f70', p.f70, 'f88', p.f88);
  // direct calls
  const a = mk(10, [0, 0, 0]), b = mk(11, [0.5, 0.1, 0]);
  fn_143b1c700(a, b, [0, 0, 0], 0); console.log('1c700', a.f0, b.f0);
  const c1 = mk(12, [0, 0, 0], { fC: [0.1, 0, 0] }), c2 = mk(13, [4, 0, 0]), e1 = mk(14, [2, -3, 0.5], { f276: 1 }), e2 = mk(15, [2, 3, 0.5], { f276: 1 });
  fn_143b15850(c1, c2, f(0.1), e1, e2, 0, [0, 0, 0], 0); console.log('15850', c1.f0, c2.f0, e1.f0, e2.f0);
  const s1 = mk(16, [1, 0.5, 0.2], { f276: 1 }), k1 = mk(17, [0, 0, 0]), k2 = mk(18, [3, 0, 0]);
  fn_143b1b110(s1, k1, k2, f(0.1), [0, 0, 0], 0); console.log('1b110', s1.f0, k1.f0, k2.f0);
  const pl = fn_143ae62b0({}, 6, mk(19, [0, 0, 0]), mk(20, [1, 0, 0]), mk(21, [0, 1, 0]));
  const q = mk(22, [0.2, 0.2, -0.3], { fC: [0.1, 0.2, -0.3] }); fn_143b1b9f0(q, pl); console.log('1b9f0 n', pl.f20, 'p', q.f0);
  const gp = mk(23, [0, 0, 0.5]); console.log('12da0', fn_143b12da0(gp, [0, 0, 0], [0, 0, 1]), gp.f0, gp.f70);
  console.log('capRadius', capRadius(f(0.25), 1, 2, f(0.5)));
}
