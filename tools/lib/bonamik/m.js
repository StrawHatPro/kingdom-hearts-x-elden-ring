// Shared float32 math helpers for the Bonamik transcription (build/sora_tmp/bnm). Every scalar op is rounded to
// float32 with Math.fround so that the operation ORDER of the exe can be reproduced. Vectors are plain arrays
// [x,y,z], quaternions [x,y,z,w]. If the exe's order differs from a helper here, inline the arithmetic instead.
const f = Math.fround;
const add = (a, b) => [f(a[0] + b[0]), f(a[1] + b[1]), f(a[2] + b[2])];
const sub = (a, b) => [f(a[0] - b[0]), f(a[1] - b[1]), f(a[2] - b[2])];
const mul = (a, s) => [f(a[0] * s), f(a[1] * s), f(a[2] * s)];
// dot as ((x*x) + (y*y)) + (z*z)  -- CHECK the exe's order at each site; many sites use (y*y + x*x) + z*z (same value) 
const dot = (a, b) => f(f(f(a[0] * b[0]) + f(a[1] * b[1])) + f(a[2] * b[2]));
const cross = (a, b) => [f(f(a[1] * b[2]) - f(a[2] * b[1])), f(f(a[2] * b[0]) - f(a[0] * b[2])), f(f(a[0] * b[1]) - f(a[1] * b[0]))];
const sqrt = x => f(Math.sqrt(x));                      // sqrtss (correctly rounded)
// rsqrtss + two Newton steps y += y*(0.5 - 0.5*x*y*y) as the module does; rsqrtss itself (11-bit table) is emulated
// by a correctly rounded 1/sqrt -> last-bit differences possible.
const rsqrt = x => { let y = f(1 / Math.sqrt(x)); const h = f(x * 0.5); y = f(y + f(y * f(0.5 - f(h * f(y * y))))); y = f(y + f(y * f(0.5 - f(h * f(y * y))))); return y; };
// Hamilton product a*b
const qmul = (a, b) => [
  f(f(f(f(a[3] * b[0]) + f(a[0] * b[3])) + f(a[1] * b[2])) - f(a[2] * b[1])),
  f(f(f(f(a[3] * b[1]) - f(a[0] * b[2])) + f(a[1] * b[3])) + f(a[2] * b[0])),
  f(f(f(f(a[3] * b[2]) + f(a[0] * b[1])) - f(a[1] * b[0])) + f(a[2] * b[3])),
  f(f(f(f(a[3] * b[3]) - f(a[0] * b[0])) - f(a[1] * b[1])) - f(a[2] * b[2]))];
const qconj = q => [f(-q[0]), f(-q[1]), f(-q[2]), q[3]];
// FQuat::RotateVector shape: v + 2w(q x v) + 2 q x (q x v)
const qrot = (q, v) => { const t = mul(cross(q, v), 2); const c2 = cross(q, t); return [f(f(v[0] + f(q[3] * t[0])) + c2[0]), f(f(v[1] + f(q[3] * t[1])) + c2[1]), f(f(v[2] + f(q[3] * t[2])) + c2[2])]; };
module.exports = { f, add, sub, mul, dot, cross, sqrt, rsqrt, qmul, qconj, qrot };
