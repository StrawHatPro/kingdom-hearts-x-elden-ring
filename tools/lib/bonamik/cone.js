// Bonamik pass 2, part "cone": instruction-level transcription (KH3.exe, image base 0x140000000) of
//   fn_143b166d0  cone constraint solve (only call site 0x143b192f9 in ChainLink 0x143b192a0)
//   fn_143af6960  helper called by it (attachment point of a particle that has +0x238)
//   fn_143aec370  cone record allocation from a SQEX_BonamikLinkDesc  ->  fn_143ae6800  cone record setup
//   fn_143aee280_cones  the cone refresh loop inside PreStep 0x143aee280 (0x143aee603 .. 0x143aeebc6)
// Field naming: hex offsets (see H_cone.md for the layouts). Everything is float32 in the exe's operation order.
// The module was checked bit-for-bit against an instruction interpreter running the exe's code (see H_cone.md 6).
'use strict';
const { f, sqrt, rsqrt, qmul, qconj } = require('./m.js');

const F = h => new Float32Array(new Uint32Array([h]).buffer)[0];   // float from its bit pattern
const SMALL = F(0x322bcc77);        // 1e-8        0x145C9C488
const TENTH = F(0x3dcccccd);        // 0.1         0x145C87D00
const ARC_MIN = F(0xbf7ff972);      // -0.9999     0x14750BAA8
const P01 = F(0x3c23d70a);          // 0.01        0x145CADFC4
const P001 = F(0x3a83126f);         // 0.001       0x145C9C4A8
const P0001 = F(0x38d1b717);        // 0.0001      0x145CF35A8
const PI = F(0x40490fdb);           // 0x145C9C4F8
const NEG_PI = F(0xc0490fdb);       // 0x145C9C5A0
const HALF_PI = F(0x3fc90fdb);      // 0x145C9C4F0
const NEG_HALF_PI = F(0xbfc90fdb);  // 0x145C9C598
const TWO_PI = F(0x40c90fdb);       // 0x145C9C508
const INV_TWO_PI = F(0x3e22f983);   // 0.15915494  0x145C9C4C4
const DEG2RAD = F(0x3c8efa35);      // 0.017453292 0x145C9C4B4
// acos polynomial 0x14750BAA4 / 0x14750BA98 / 0x14750BA90 / 0x14750BA8C
const AC0 = F(0x3fc90da4), AC1 = F(0x3e593484), AC2 = F(0x3d981627), AC3 = F(0x3c996e30);
// stock FMath::SinCos coefficients 0x145C9C490.. (sin) / 0x145C9C494.. (cos)
const S0 = F(0x32cd365b), S1 = F(0x3638b88e), S2 = F(0x39500bf1), S3 = F(0x3c088886), S4 = F(0x3e2aaaab);
const C0 = F(0x348bdd11), C1 = F(0x37cfb4c2), C2 = F(0x3ab609aa), C3 = F(0x3d2aaaa3);

// FVector::GetSafeNormal shape as inlined everywhere in the module:
// sq = (y*y + x*x) + z*z ; sq == 1 (or unordered) -> unchanged ; sq < 1e-8 -> ZeroVector ; else v * rsqrt2(sq)
function safeNormal(v) {
  const sq = f(f(f(v[1] * v[1]) + f(v[0] * v[0])) + f(v[2] * v[2]));
  if (sq === 1 || Number.isNaN(sq)) return [v[0], v[1], v[2]];          // ucomiss + je (taken when equal or unordered)
  if (!(sq >= SMALL)) return [0, 0, 0];                                  // FVector::ZeroVector 0x149B5C798
  const y = rsqrt(sq);
  return [f(y * v[0]), f(y * v[1]), f(y * v[2])];
}
// shortest arc from +X to the unit vector m: (x0, -(m.z/s), m.y/s, s/2), s = sqrt(2*m.x + 2); m.x < -0.9999 -> (0,1,0,0)
// x0 is the bit pattern the exe leaves in the x slot: -0.0 at every site except one (+0.0), see H_cone.md
function arcFromX(m, x0) {
  if (m[0] >= ARC_MIN) {
    const s = sqrt(f(f(m[0] + m[0]) + 2));
    const inv = f(1 / s);
    return [x0, f(-f(m[2] * inv)), f(m[1] * inv), f(s * 0.5)];
  }
  return [0, 1, 0, 0];                                                    // 0x145D4A2E0
}
// rot(q, +X) = (1 - 2y^2 - 2z^2, 2wz + 2xy, 2xz - 2wy) in the exe's order (m2y = (-y)+(-y), p2z = z+z)
function xAxis(q) {
  const m2y = f(-q[1] + -q[1]), p2z = f(q[2] + q[2]);
  return [f(f(f(q[1] * m2y) - f(q[2] * p2z)) + 1), f(f(q[3] * p2z) - f(q[0] * m2y)), f(f(q[0] * p2z) + f(q[3] * m2y))];
}
// the same with the x component of q taken as zero (sites that work from the registers of arcFromX)
function xAxis0(q) {
  const m2y = f(-q[1] + -q[1]), p2z = f(q[2] + q[2]);
  return [f(f(f(q[1] * m2y) - f(q[2] * p2z)) + 1), f(q[3] * p2z), f(q[3] * m2y)];
}
// rot(q, +Y) and rot(q, +Z) as coded at 0x143b17d4d..0x143b17e4e
function yAxis(q) {
  const p2x = f(q[0] + q[0]), m2z = f(-q[2] + -q[2]);
  return [f(f(q[1] * p2x) + f(q[3] * m2z)), f(f(f(q[2] * m2z) - f(q[0] * p2x)) + 1), f(f(q[3] * p2x) - f(q[1] * m2z))];
}
function zAxis(q) {
  const m2x = f(-q[0] + -q[0]), p2y = f(q[1] + q[1]);
  return [f(f(p2y * q[3]) - f(m2x * q[2])), f(f(m2x * q[3]) + f(p2y * q[2])), f(f(f(m2x * q[0]) - f(p2y * q[1])) + 1)];
}
// VectorNormalizeQuaternion shape (stock UE SSE): sq = (x^2+z^2) + (y^2+w^2); sq >= 1e-8 ? q*rsqrt2(sq) : (0,0,0,1)
function qnormalize(q) {
  const sq = f(f(f(q[1] * q[1]) + f(q[3] * q[3])) + f(f(q[0] * q[0]) + f(q[2] * q[2])));
  if (!(sq >= SMALL)) return [0, 0, 0, 1];
  const y = rsqrt(sq);
  return [f(y * q[0]), f(y * q[1]), f(y * q[2]), f(y * q[3])];
}
// FQuat::RotateVector shape: T = 2*(q x v) ; r = (q x T) + (T*w + v)
function rotate(q, v) {
  let tx = f(f(q[1] * v[2]) - f(q[2] * v[1])), ty = f(f(q[2] * v[0]) - f(q[0] * v[2])), tz = f(f(q[0] * v[1]) - f(q[1] * v[0]));
  tx = f(tx + tx); ty = f(ty + ty); tz = f(tz + tz);
  return [f(f(f(tz * q[1]) - f(ty * q[2])) + f(f(tx * q[3]) + v[0])),
          f(f(f(tx * q[2]) - f(tz * q[0])) + f(f(ty * q[3]) + v[1])),
          f(f(f(ty * q[0]) - f(tx * q[1])) + f(f(tz * q[3]) + v[2]))];
}
// "swing with the reference twist removed": T = qs * refConj ; p = dot(T.xyz, X) ; tw = normalize(p*X, T.w) ; conj(tw) * qs
function twistFix(qs, X, refConj) {
  const T = qmul(qs, refConj);
  const p = f(f(f(T[1] * X[1]) + f(T[0] * X[0])) + f(T[2] * X[2]));
  const twn = qnormalize([f(p * X[0]), f(p * X[1]), f(p * X[2]), T[3]]);
  return qmul(qconj(twn), qs);
}
// polynomial acos (Abramowitz-Stegun 4.4.45), 0x143b17865.. : sqrt(1-|c|) * (1.5707288 - 0.2121144a + 0.074261a^2 - 0.0187293a^3)
function acosPoly(c) {
  const a = Math.abs(c);
  const root = sqrt(f(1 - a));
  const a2 = f(a * a);
  let t = f(AC0 - f(a * AC1));
  t = f(t + f(a2 * AC2));
  t = f(t - f(f(a * a2) * AC3));
  let r = f(root * t);
  if (!(c >= 0)) r = f(PI - r);
  return r;
}
// stock FMath::SinCos (UE 4.17 UnrealMathUtility.h) as inlined; returns [sin, cos]
function sinCos(v) {
  let q = f(v * INV_TWO_PI);
  q = (v >= 0) ? f(q + 0.5) : f(q - 0.5);
  let n = Math.trunc(q); if (!(n >= -2147483648 && n <= 2147483647)) n = -2147483648;   // cvttss2si
  let y = f(v - f(f(n) * TWO_PI));
  let sign;
  if (y > HALF_PI) { y = f(PI - y); sign = -1; }
  else if (y >= NEG_HALF_PI) sign = 1;
  else { y = f(NEG_PI - y); sign = -1; }
  const y2 = f(y * y);
  let s = f(S1 - f(y2 * S0));
  s = f(f(s * y2) - S2); s = f(f(s * y2) + S3); s = f(f(s * y2) - S4); s = f(f(s * y2) + 1); s = f(s * y);
  let c = f(C1 - f(y2 * C0));
  c = f(f(c * y2) - C2); c = f(f(c * y2) + C3); c = f(f(c * y2) - 0.5); c = f(f(c * y2) + 1); c = f(c * sign);
  return [s, c];
}

// 0x143af6960 (rcx = a, rdx = out): out = Q.pos + rot(Q.quat(+0x20), Q.scale(+0x140) * a.f10), Q = a.f0 (particle)
function fn_143af6960(a, out) {
  const Q = a.f0, s = Q.f140, l = a.f10;
  const rv = rotate(Q.f20, [f(s[0] * l[0]), f(s[1] * l[1]), f(s[2] * l[2])]);
  out[0] = f(rv[0] + Q.f0[0]); out[1] = f(rv[1] + Q.f0[1]); out[2] = f(rv[2] + Q.f0[2]);
  return out;
}

// 0x143b166d0 (xmm0 = r, rdx = cone, xmm2 = innerConeScale, xmm3 = outerConeScale, [rsp+0x20] byte = enforceLength; rcx unused)
function fn_143b166d0(r, c, innerScale, outerScale, enforceLength) {
  r = f(r); innerScale = f(innerScale); outerScale = f(outerScale);   // (inputs are float32 in the exe)
  const P1 = c.f0, P2 = c.f8;                       // rbx, rdi
  let w = c.f18;                                    // xmm12: parent share
  if (P1.f31D) w = 0.5;
  if (TENTH > c.f44) {                              // 143b1673c: degenerate segment
    P2.f20 = P1.f20.slice(); P2.f0 = P1.f0.slice();
    return;
  }
  let S = P1;                                       // rsi: particle whose chain parent defines the reference frame
  if (P1.f238) {
    P1.f0 = fn_143af6960(P1.f238, [0, 0, 0]);
    if (!P1.f276) S = P1.f238.f0;
  }
  let QA, Qr;                                       // [rbp-0x40] (w also at [rbp-0x78]) ; xmm11 = [rsp+0x50]
  const G = S.f190;
  if (!G || !S.fBC) {                               // 143b17066
    QA = S.f160.slice(); Qr = S.f160.slice();
  } else {
    QA = (G.f190 ? G.f20 : G.f160).slice();         // 143b16817: cmp [G+0x190],0 ; cmove
    Qr = (P1.f238 ? G : S).f20.slice();
    if (P1.f238) {                                  // 143b16856 .. 143b17058 (attached particle)
      const qs = arcFromX(safeNormal([f(S.f0[0] - G.f0[0]), f(S.f0[1] - G.f0[1]), f(S.f0[2] - G.f0[2])]), 0);
      Qr = twistFix(qs, xAxis0(qs), qconj(QA));
      const A0 = P1.f238.f0, ch = A0.f198;
      if (ch) {
        const q2 = arcFromX(safeNormal([f(ch.f0[0] - A0.f0[0]), f(ch.f0[1] - A0.f0[1]), f(ch.f0[2] - A0.f0[2])]), -0);
        A0.f20 = q2;
        A0.f20 = twistFix(q2, xAxis(q2), qconj(QA));
      }
      P1.f0 = fn_143af6960(P1.f238, [0, 0, 0]);
    }
  }
  // 143b17086: hard length
  {
    const dx = f(P2.f0[0] - P1.f0[0]), dy = f(P2.f0[1] - P1.f0[1]), dz = f(P2.f0[2] - P1.f0[2]);
    const len = sqrt(f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz)));
    if (enforceLength) {
      const inv = f(1 / len), e = f(c.f44 - len);
      const cx = f(f(e * dx) * inv), cy = f(f(e * dy) * inv), cz = f(f(e * dz) * inv);
      if (!P2.f276) P2.f0 = [f(cx + P2.f0[0]), f(cy + P2.f0[1]), f(cz + P2.f0[2])];
      if (!P1.f276) {
        const n = P1.f1A8 >>> 0;
        if (n > 1) {
          const k = f(1 / f(n));
          P1.f58 = [f(f(k * cx) + P1.f58[0]), f(f(k * cy) + P1.f58[1]), f(f(k * cz) + P1.f58[2])];
        } else P1.f58 = [f(cx + P1.f58[0]), f(cy + P1.f58[1]), f(cz + P1.f58[2])];
      }
    }
  }
  // 143b171ad: inner spring towards rot(Qr * cone.f60, +X) * restLength
  const QC = qmul(Qr, c.f50);                       // [rsp+0x60]: limit frame
  const QI = qmul(Qr, c.f60);                       // xmm7: inner target frame
  const L = c.f44;
  let k = f(f(c.f28 * innerScale) * r);             // xmm14
  k = (k >= 0) ? (k < 1 ? k : 1) : 0;               // comiss/jae ; minss
  {
    const XI = xAxis(QI);
    const cx = f(f(f(P1.f0[0] + f(XI[0] * L)) - P2.f0[0]) * k);
    const cy = f(f(f(P1.f0[1] + f(XI[1] * L)) - P2.f0[1]) * k);
    const cz = f(f(f(P1.f0[2] + f(XI[2] * L)) - P2.f0[2]) * k);
    if (P1.f276) {
      if (!P2.f276) P2.f0 = [f(cx + P2.f0[0]), f(cy + P2.f0[1]), f(cz + P2.f0[2])];
    } else if (P2.f276) {
      P1.f0 = [f(P1.f0[0] - cx), f(P1.f0[1] - cy), f(P1.f0[2] - cz)];
    } else {
      const om = f(1 - w);
      P1.f0 = [f(P1.f0[0] - f(cx * om)), f(P1.f0[1] - f(cy * om)), f(P1.f0[2] - f(cz * om))];
      P2.f0 = [f(f(cx * w) + P2.f0[0]), f(f(cy * w) + P2.f0[1]), f(f(cz * w) + P2.f0[2])];
    }
  }
  // 143b173fc: current segment direction n = (P2 - P1) / |P2 - P1| (plain divide, no zero guard)
  const p1 = P1.f0.slice(), p2 = P2.f0.slice();
  let nx = f(p2[0] - p1[0]), ny = f(p2[1] - p1[1]), nz = f(p2[2] - p1[2]);
  {
    const inv = f(1 / sqrt(f(f(f(ny * ny) + f(nx * nx)) + f(nz * nz))));
    nx = f(nx * inv); ny = f(ny * inv); nz = f(nz * inv);
  }
  const n = [nx, ny, nz];
  // 143b1746e .. 143b177cd computes normalize(twist of arcFromX(safeNormal(n)) * conj(Qr)) and discards it (dead store
  // at 143b177cd/143b177d2); it has no effect and is not reproduced here.
  const A = xAxis(QC);                              // cone axis: [rbp-0x7c], [rbp-0x5c], [rbp+0xa8]
  const limY = c.f30;
  // distribution of an outer-limit correction e (the same code twice: 143b17bbf.. and 143b186b0..)
  const distribute = (ex, ey, ez) => {
    if (P1.f276) {
      if (!P2.f276) P2.f0 = [f(ex + p2[0]), f(ey + p2[1]), f(ez + p2[2])];
    } else if (P2.f276) {
      P1.f0 = [f(p1[0] - ex), f(p1[1] - ey), f(p1[2] - ez)];
    } else {
      const om = f(1 - w);
      P1.f0 = [f(p1[0] - f(om * ex)), f(p1[1] - f(om * ey)), f(p1[2] - f(om * ez))];
      P2.f0 = [f(f(ex * w) + p2[0]), f(f(ey * w) + p2[1]), f(f(ez * w) + p2[2])];
      if (w > 0.5) {
        const g = f(f(f(w - 0.5) + w) - 0.5);                         // = 2w - 1
        P1.f58 = [f(f(g * ex) + P1.f58[0]), f(f(g * ey) + P1.f58[1]), f(f(g * ez) + P1.f58[2])];
      } else if (!(w >= 0.5)) {
        const g = f(f(0.5 - w) + f(0.5 - w));                         // = 1 - 2w
        P2.f58 = [f(P2.f58[0] - f(g * ex)), f(P2.f58[1] - f(g * ey)), f(P2.f58[2] - f(g * ez))];
      }
    }
  };
  if (!(Math.abs(f(limY - c.f34)) >= P01)) {
    // 143b17865: circular limit
    const ang = acosPoly(f(f(f(A[1] * ny) + f(A[0] * nx)) + f(A[2] * nz)));
    if (ang > limY) {
      const cx = f(f(A[1] * nz) - f(A[2] * ny)), cy = f(f(A[2] * nx) - f(A[0] * nz)), cz = f(f(A[0] * ny) - f(A[1] * nx));   // A x n
      const clen = sqrt(f(f(f(cy * cy) + f(cx * cx)) + f(cz * cz)));
      if (clen > P001) {
        const inv = f(1 / clen);
        const sc = sinCos(f(f(limY - ang) * 0.5));                    // half angle (negative)
        const s = sc[0];
        const rot = rotate([f(f(inv * cx) * s), f(f(cy * inv) * s), f(s * f(cz * inv)), sc[1]], n);
        let ko = f(f(outerScale * c.f2C) * r);
        ko = (ko >= 0) ? (ko < 1 ? ko : 1) : 0;
        distribute(f(f(f(f(rot[0] * L) + p1[0]) - p2[0]) * ko),
                   f(f(f(f(rot[1] * L) + p1[1]) - p2[1]) * ko),
                   f(f(f(f(rot[2] * L) + p1[2]) - p2[2]) * ko));
      }
    }
  } else {
    // 143b17d4d: elliptical limit (limY about the frame's Y axis test, limZ about its Z axis)
    const Y = yAxis(QC), Z = zAxis(QC);
    let D = [0, 0, 0, 1];                                             // FQuat::Identity 0x14871E410
    const dY = f(f(f(Y[1] * ny) + f(Y[0] * nx)) + f(Y[2] * nz));
    const p = safeNormal([f(nx - f(dY * Y[0])), f(ny - f(dY * Y[1])), f(nz - f(dY * Y[2]))]);
    const ang1 = acosPoly(f(f(f(p[1] * A[1]) + f(p[0] * A[0])) + f(p[2] * A[2])));
    if (ang1 > limY) {
      const dd = f(f(f(Z[1] * p[1]) + f(p[0] * Z[0])) + f(Z[2] * p[2]));
      const sg = (dd >= 0) ? 1 : -1;
      const sc = sinCos(f(f(ang1 - limY) * 0.5));
      const s = sc[0];
      D = qmul([f(f(Y[0] * sg) * s), f(s * f(Y[1] * sg)), f(s * f(Y[2] * sg)), sc[1]], D);
    }
    const dZ = f(f(f(Z[1] * ny) + f(Z[0] * nx)) + f(Z[2] * nz));
    const p2n = safeNormal([f(nx - f(Z[0] * dZ)), f(ny - f(Z[1] * dZ)), f(nz - f(Z[2] * dZ))]);
    const ang2 = acosPoly(f(f(f(p2n[0] * A[0]) + f(p2n[1] * A[1])) + f(p2n[2] * A[2])));
    const limZ = c.f34;
    if (ang2 > limZ) {
      const dd = f(f(f(p2n[1] * Y[1]) + f(p2n[0] * Y[0])) + f(p2n[2] * Y[2]));
      const sg = (dd >= 0) ? 1 : -1;
      const sc = sinCos(f(f(limZ - ang2) * 0.5));
      const s = sc[0];
      D = qmul([f(s * f(Z[0] * sg)), f(f(Z[1] * sg) * s), f(f(Z[2] * sg) * s), sc[1]], D);
    }
    const rot = rotate(D, n);
    const ko = f(r * c.f2C);                                          // 143b18706: no outer scale, no clamp (as coded)
    distribute(f(f(f(f(rot[0] * L) + p1[0]) - p2[0]) * ko),
               f(f(f(f(rot[1] * L) + p1[1]) - p2[1]) * ko),
               f(f(f(f(rot[2] * L) + p1[2]) - p2[2]) * ko));
  }
  // 143b1893b: orientation
  const m = safeNormal([f(P2.f0[0] - P1.f0[0]), f(P2.f0[1] - P1.f0[1]), f(P2.f0[2] - P1.f0[2])]);
  const qs = arcFromX(m, -0);
  if (P1.f190 && !P1.f238 && P1.fBC && !P1.f240) {
    P2.f20 = qs;                                                      // 143b18abf (then replaced below)
    const res = twistFix(qs, xAxis(qs), qconj(c.f20 ? P1.f160 : QA));
    P2.f20 = res;
    if (P1.f1A8 === 1) P1.f20 = res.slice();
  } else {
    P1.f20 = qs;                                                      // 143b18fc0 (then replaced below)
    const res = twistFix(qs, xAxis(qs), qconj(P1.f160));
    P1.f20 = res; P2.f20 = res.slice();
  }
}

// 0x143ae6800 (rcx = rec, rdx = P1 parent particle, r8 = P2 child particle, xmm3 = limitY [rad], stack: limitZ, limitOffsetY,
// limitOffsetZ, coneOffsetY, coneOffsetZ [all rad], coneOuterStr, coneInnerStr, coneParentStr, byte enableConeAnimation)
function fn_143ae6800(rec, P1, P2, limY, limZ, limOffY, limOffZ, coneOffY, coneOffZ, outerStr, innerStr, parentStr, anim) {
  rec.f34 = f(limZ); rec.f30 = f(limY); rec.f0 = P1; rec.f8 = P2; rec.f10 = -1;
  rec.f38 = 0; rec.f3C = 0;                                           // one qword store of 0 at +0x38
  rec.f50 = [0, 0, 0, 1]; rec.f60 = [0, 0, 0, 1]; rec.f70 = [0, 0, 0, 1]; rec.f80 = [0, 0, 0, 1];
  const qs = arcFromX(safeNormal([f(P2.f0[0] - P1.f0[0]), f(P2.f0[1] - P1.f0[1]), f(P2.f0[2] - P1.f0[2])]), -0);
  const Q0 = twistFix(qs, xAxis0(qs), qconj(P1.f120));                // [rsp]: frame of the segment P1->P2
  let S = P1;
  if (P1.f238 && !P1.f276) S = P1.f238.f0;
  const G = S.f190;
  if (G && !G.f277 && !S.f276 && !P1.f240) {
    let v = [f(S.f0[0] - G.f0[0]), f(S.f0[1] - G.f0[1]), f(S.f0[2] - G.f0[2])];
    const sq = f(f(f(v[1] * v[1]) + f(v[0] * v[0])) + f(v[2] * v[2]));
    if (!(sq >= P01) && G.f190) { const GG = G.f190; v = [f(G.f0[0] - GG.f0[0]), f(G.f0[1] - GG.f0[1]), f(G.f0[2] - GG.f0[2])]; }
    const qg = arcFromX(safeNormal(v), -0);
    const Qg = twistFix(qg, xAxis0(qg), qconj(G.f120));               // frame of the segment G->S
    const rel = qmul(qconj(Qg), Q0);
    rec.f50 = rel; rec.f60 = rel.slice();
  }
  let sc = sinCos(f(limOffY * 0.5)); const qY = [-0, sc[0], -0, sc[1]];
  sc = sinCos(f(limOffZ * 0.5)); const qZ = [-0, -0, sc[0], sc[1]];
  const Lq = qmul(qZ, qY);
  rec.f70 = Lq; rec.f50 = qmul(rec.f50, Lq);
  sc = sinCos(f(coneOffY * 0.5)); const qY2 = [-0, sc[0], -0, sc[1]];
  sc = sinCos(f(coneOffZ * 0.5)); const qZ2 = [-0, -0, sc[0], sc[1]];
  const Cq = qmul(qZ2, qY2);
  rec.f80 = Cq; rec.f60 = qmul(rec.f60, Cq);
  rec.f28 = f(innerStr); rec.f90 = [1, 1, 1]; rec.f14 = f(outerStr); rec.f2C = f(outerStr); rec.f18 = f(parentStr);
  rec.f20 = anim ? 1 : 0;
  const dx = f(P1.f0[0] - P2.f0[0]), dy = f(P1.f0[1] - P2.f0[1]), dz = f(P1.f0[2] - P2.f0[2]);
  rec.f40 = sqrt(f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz)));
  rec.f44 = f(rec.f40 * rec.f90[0]);
  return rec;
}

// 0x143aec370 (rcx = scene, rdx = SQEX_BonamikLinkDesc, r8d = particle index base of this asset)
// desc fields by reflected offset: f20 m_ParentId, f24 m_ChildId, f2C m_EnableConeAnimation, f38 m_ConeInnerStr,
// f3C m_ConeOuterStr, f40 m_ConeParentStr, f44 m_LimitY, f48 m_LimitZ, f4C m_ConeOffsetY, f50 m_ConeOffsetZ,
// f54 m_LimitOffsetY, f58 m_LimitOffsetZ, f6C m_ConeBatchNo
function fn_143aec370(scene, d, base) {
  const idx = scene.f38;
  if (scene.f78C === idx) return null;
  const P1 = scene.f0[(d.f20 + base) | 0], P2 = scene.f0[(d.f24 + base) | 0];
  scene.f38 = idx + 1;                                                // (array growth 0x1405930B0 when > scene.f3C)
  if (scene.f3C !== undefined && scene.f38 > scene.f3C) scene.f3C = scene.f38;
  const rec = scene.f30[idx] || (scene.f30[idx] = {});
  fn_143ae6800(rec, P1, P2, f(d.f44 * DEG2RAD), f(d.f48 * DEG2RAD), f(d.f54 * DEG2RAD), f(d.f58 * DEG2RAD),
    f(d.f4C * DEG2RAD), f(d.f50 * DEG2RAD), d.f3C, d.f38, d.f40, d.f2C);
  rec.f10 = d.f6C;
  P1.f2A8 = rec;
  rec.f24 = idx;
  return rec;
}

// PreStep 0x143aee280, loop 0x143aee64d..0x143aeebc0 over the cone array scene+0x30 (count scene+0x38).
// bones = editable component-space transforms (owner vf60), bones[i] = {q, t, s}
function fn_143aee280_cones(scene, bones) {
  for (let i = 0; i < scene.f38; i++) {
    const c = scene.f30[i];
    if (!c.f20) continue;
    const P1 = c.f0, P2 = c.f8;
    let S = P1;
    if (P1.f238 && !P1.f276) S = P1.f238.f0;
    const q1 = bones[P1.f248].q, q2 = bones[P2.f248].q;
    const G = S.f190;
    if (!G) { c.f50 = c.f70.slice(); c.f60 = c.f80.slice(); }
    else if (!G.f277 && !S.f276) {
      const rel = qmul(qconj(bones[G.f248].q), q1);
      c.f50 = qmul(rel, c.f70); c.f60 = qmul(rel, c.f80);
    }
    // twist angle of the child bone against the parent bone about the child bone's X axis
    const X = xAxis(q2);
    const T = qmul(q2, qconj(q1));
    const p = f(f(f(T[0] * X[0]) + f(T[1] * X[1])) + f(T[2] * X[2]));
    let tw = [f(p * X[0]), f(p * X[1]), f(p * X[2]), T[3]];
    const sq = f(f(f(f(tw[1] * tw[1]) + f(tw[0] * tw[0])) + f(tw[2] * tw[2])) + f(tw[3] * tw[3]));
    if (!(sq > P0001)) tw = [0, 0, 0, 1];
    tw = qnormalize(tw);
    const ang = acosPoly(tw[3]);
    P2.f314 = f(ang + ang);
    const s = bones[P1.f248].s;
    c.f90 = [s[0], s[1], s[2]];
    c.f44 = f(c.f40 * c.f90[0]);
  }
}

module.exports = { fn_143b166d0, fn_143af6960, fn_143ae6800, fn_143aec370, fn_143aee280_cones,
  // helpers (shared shapes, exported for the other parts and for tests)
  safeNormal, arcFromX, xAxis, xAxis0, yAxis, zAxis, qnormalize, rotate, twistFix, acosPoly, sinCos };

if (require.main === module) {
  const mk = (pos, o) => Object.assign({ f0: pos, f20: [0, 0, 0, 1], f58: [0, 0, 0], f120: [0, 0, 0, 1], f130: pos.slice(), f140: [1, 1, 1],
    f160: [0, 0, 0, 1], f190: null, f198: null, f1A8: 0, f238: null, f240: null, f248: 0, f276: 0, f277: 0, fBC: 0, f31D: 0, f314: 0, f2A8: null }, o);
  const root = mk([0, 0, 100], { f276: 1, f1A8: 1, f248: 0 });
  const a = mk([10, 0, 100], { f190: root, fBC: 1, f1A8: 1, f248: 1 });
  const b = mk([20, 0, 100], { f190: a, fBC: 1, f248: 2 });
  root.f198 = a; a.f198 = b;
  const scene = { f0: [root, a, b], f8: 3, f30: [], f38: 0, f3C: 0, f78C: 8 };
  const desc = (p, ch) => ({ f20: p, f24: ch, f2C: 1, f38: 0.05, f3C: 1, f40: 0.2, f44: 20, f48: 35, f4C: 0, f50: 0, f54: 0, f58: 0, f6C: 0 });
  const c0 = fn_143aec370(scene, desc(0, 1), 0), c1 = fn_143aec370(scene, desc(1, 2), 0);
  console.log('setup cone0', { f30: c0.f30, f34: c0.f34, f44: c0.f44, f50: c0.f50, f60: c0.f60 });
  console.log('setup cone1', { f30: c1.f30, f34: c1.f34, f44: c1.f44, f50: c1.f50, f60: c1.f60 });
  const bones = [0, 1, 2].map(i => ({ q: [0, 0, 0, 1], t: [10 * i, 0, 100], s: [1, 1, 1] }));
  fn_143aee280_cones(scene, bones);
  console.log('prestep cone1', { f50: c1.f50, f60: c1.f60, f44: c1.f44, twist: b.f314 });
  a.f0 = [9, 0, 96]; b.f0 = [12, 8, 90];          // disturbed state
  for (let it = 0; it < 3; it++) { fn_143b166d0(1, c0, 1, 1, 1); fn_143b166d0(1, c1, 1, 1, 1); }
  console.log('solve a', a.f0, a.f20, a.f58);
  console.log('solve b', b.f0, b.f20, b.f58);
  const att = { f0: root, f10: [1, 2, 3] };
  console.log('fn_143af6960', fn_143af6960(att, [0, 0, 0]));
}
