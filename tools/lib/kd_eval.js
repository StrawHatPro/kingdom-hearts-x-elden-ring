// KineDriver reference evaluator (Square Enix helper-bone driver of the KineDriverRt module), as specified in docs/sora-animation-spec.md.
// Written by the research pass build/sora_tmp/native_F_kinedriver.md; used by tools/sora_pack.js to keep the packed driver data verified.
const VERSION = '1.0.0';
// Reference evaluator of the KineDriver model (native_F_kinedriver.md) in Node, double precision.
// pose: array of {r:[x,y,z,w], t:[x,y,z], s:[x,y,z]} component-space UE transforms (cm, UE axes), modified in place.
// local: array of local-space UE transforms (animation output), read only.
const S = 0.70710677; // 0x3f3504f3 as used by the exe
const qmul = (a, b) => [ // Hamilton product a*b (UE FQuat operator*: b applied first)
  a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
  a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
  a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
  a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qconj = q => [-q[0], -q[1], -q[2], q[3]];
const qnorm = q => { const l = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]; if (l < 1e-8) return [0, 0, 0, 1]; const k = 1 / Math.sqrt(l); return [q[0] * k, q[1] * k, q[2] * k, q[3] * k]; };
const qnormIfNeeded = q => { const l = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]; return Math.abs(1 - l) >= 0.01 ? qnorm(q) : q; };
const qrot = (q, v) => { // FQuat::RotateVector
  const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]);
  return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)];
};
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const vnorm = v => { const l = dot(v, v); if (l === 1) return v; if (l < 1e-8) return [0, 0, 0]; const k = 1 / Math.sqrt(l); return [v[0] * k, v[1] * k, v[2] * k]; };
const C = [S, 0, 0, S], CINV = [-S, 0, 0, S];
// UE component-space quaternion -> KineDriver space
const toKD = q => { const r = qmul(C, q); return [r[0], -r[1], r[2], -r[3]]; };
// KineDriver space quaternion -> UE component space (normalized)
const fromKD = q => qnorm(qmul(CINV, [q[0], -q[1], q[2], -q[3]]));
const swapYZ = v => [v[0], v[2], v[1]];
function slerpNN(a, b, alpha) { // FQuat::Slerp_NotNormalized
  const raw = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]; const c = Math.abs(raw); let s0, s1;
  if (c < 0.9999) { const om = Math.acos(Math.max(-1, Math.min(1, c))), inv = 1 / Math.sin(om); s0 = Math.sin((1 - alpha) * om) * inv; s1 = Math.sin(alpha * om) * inv; }
  else { s0 = 1 - alpha; s1 = alpha; }
  if (raw < 0) s1 = -s1;
  return [s0 * a[0] + s1 * b[0], s0 * a[1] + s1 * b[1], s0 * a[2] + s1 * b[2], s0 * a[3] + s1 * b[3]];
}
function atan2Fast(y, x) { // FMath::Atan2 (0x142ce9900)
  const ax = Math.abs(x), ay = Math.abs(y); const big = ay > ax; const t0 = big ? ay : ax, t1 = big ? ax : ay; if (t0 === 0) return 0;
  let t3 = t1 / t0; const t4 = t3 * t3;
  let p = 7.2128853633444123e-03; p = p * t4 + -3.5059680836411644e-02; p = p * t4 + 8.1675882859940430e-02; p = p * t4 + -1.3374657325451267e-01; p = p * t4 + 1.9856563505717162e-01; p = p * t4 + -3.3324998579202170e-01; p = p * t4 + 1.0;
  t3 = p * t3; if (big) t3 = 1.5707963267948966 - t3; if (x < 0) t3 = 3.141592653589793 - t3; if (y < 0) t3 = -t3; return t3;
}
function sinCosFast(v) { // FMath::SinCos
  let q = 0.15915494 * v; q = v >= 0 ? Math.trunc(q + 0.5) : Math.trunc(q - 0.5); let y = v - 6.2831855 * q; let sign = 1;
  if (y > 1.5707964) { y = 3.1415927 - y; sign = -1; } else if (y < -1.5707964) { y = -3.1415927 - y; sign = -1; }
  const y2 = y * y;
  const s = (((((-2.3889859e-08 * y2 + 2.7525562e-06) * y2 - 0.00019840874) * y2 + 0.0083333310) * y2 - 0.16666667) * y2 + 1.0) * y;
  const c = sign * (((((-2.6051615e-07 * y2 + 2.4760495e-05) * y2 - 0.0013888378) * y2 + 0.041666638) * y2 - 0.5) * y2 + 1.0);
  return [s, c];
}
function findBetween(a, b) { // 0x143ac2a10, a and b unit vectors
  const d = dot(a, b);
  if (d >= 1) return [0, 0, 0, 1];
  let q;
  if (d > -1) { const s = Math.sqrt(d + d + 2), k = 1 / s; const c = cross(a, b); q = [c[0] * k, c[1] * k, c[2] * k, s * 0.5]; }
  else { let l = Math.sqrt(a[1] * a[1] + a[0] * a[0]); let x = 0, y = a[0], z = -a[1]; if (l < 0.0001) { x = -a[2]; y = 0; z = a[0]; l = Math.sqrt(a[2] * a[2] + a[0] * a[0]); } q = [x / l, y / l, z / l, 0]; }
  return qnorm(q);
}
const baseScale = s => (s[0] === 0 && s[1] === 0 && s[2] === 0) ? [1, 1, 1] : s;
const PARAM_SLOTS = { SourceRotate: ['RotateAngle', 'BendingAngle', 'BendS', 'BendT', 'Roll'], TargetTranslate: ['TranslateX', 'TranslateY', 'TranslateZ'], TargetScale: ['ScaleX', 'ScaleY', 'ScaleZ'], TargetRotate: ['QuatAngle'], TargetBendRoll: ['Roll', 'QuatAngle'], TargetBendSTRoll: ['BendS', 'BendT', 'Roll'] };

function createState(data) { // per-instance operator parameter storage; values persist between frames exactly like the runtime objects
  return data.operators.map(o => {
    const p = {};
    if (o.type === 'TargetTranslate') { p.TranslateX = o.translate[0]; p.TranslateY = o.translate[1]; p.TranslateZ = o.translate[2]; }
    if (o.type === 'TargetScale') { p.ScaleX = o.scale[0]; p.ScaleY = o.scale[1]; p.ScaleZ = o.scale[2]; }
    if (o.type === 'TargetRotate') { p.QuatAngle = o.quat_weight; }
    if (o.type === 'TargetBendRoll') { p.QuatAngle = o.quat_weight; p.Roll = o.roll; }
    if (o.type === 'TargetBendSTRoll') { p.BendS = o.bend_s; p.BendT = o.bend_t; p.Roll = o.roll; }
    if (o.type === 'SourceRotate') { p.RotateAngle = 0; p.BendingAngle = 0; p.BendS = 0; p.BendT = 0; p.Roll = 0; p.RotateQuat = [0, 0, 0, 0]; p.BendingQuat = [0, 0, 0, 0]; }
    return p;
  });
}
function evaluate(data, state, pose, local, trace) {
  for (const o of data.operators) {
    const p = state[o.index];
    switch (o.type) {
      case 'SourceRotate': {
        let acc = [0, 0, 0, 1], wsum = 0;
        for (let i = 0; i < o.sources.length; i++) { const w = o.weights[i]; wsum += w; acc = qnorm(slerpNN(acc, toKD(pose[o.sources[i]].r), w / wsum)); }
        const B = o.base < 0 ? o.neutral_rotate : qmul(qnormIfNeeded(toKD(pose[o.base].r)), o.neutral_rotate);
        const L = qmul(qconj(B), acc); p.RotateQuat = L;
        p.RotateAngle = 2 * Math.acos(Math.min(Math.abs(L[3]), 1));
        const v = vnorm(qrot(L, o.aim));
        const c = Math.max(-1, Math.min(1, dot(v, o.aim)));
        p.BendingAngle = Math.acos(c);
        const BQ = findBetween(o.aim, v); p.BendingQuat = BQ;
        const N = vnorm(cross(o.aim, o.up));
        p.BendS = 2 * atan2Fast(-dot(N, v), c + 1);
        p.BendT = 2 * atan2Fast(dot(o.up, v), c + 1);
        const tw = qmul(qconj(BQ), L);
        const ax = Math.abs(o.aim[0]), ay = Math.abs(o.aim[1]), az = Math.abs(o.aim[2]); let k;
        if (ax <= ay) k = az < ay ? 1 : 2; else k = az < ax ? 0 : 2;
        let roll = 2 * atan2Fast(tw[k] / o.aim[k], tw[3]);
        if (roll < -3.1415927) roll += 6.2831855; else if (roll > 3.1415927) roll -= 6.2831855;
        p.Roll = roll; break;
      }
      case 'ConnectEZParamLink': {
        const src = state[o.in_op], dst = state[o.out_op]; if (!(o.in_param in src) || !(o.out_param in dst)) break;
        const x = src[o.in_param]; let d = x - o.PX0, y;
        const a0 = ((o.PY0A * 3 - o.PY0) - o.PY0B * 3) + o.PY1, b0 = (o.PY0 * 3 - o.PY0A * 6) + o.PY0B * 3, c0 = o.PY0A * 3 - o.PY0 * 3;
        const a1 = ((o.PY1A * 3 - o.PY1) - o.PY1B * 3) + o.PY2, b1 = (o.PY1 * 3 - o.PY1A * 6) + o.PY1B * 3, c1 = o.PY1A * 3 - o.PY1 * 3;
        if (!(d > 0)) y = d * o.Grad0 + o.PY0;
        else { const d2 = d - o.VX1_0; if (!(d2 > 0)) { const t = d / o.VX1_0; y = ((t * a0 + b0) * t + c0) * t + o.PY0; } else { const d3 = d2 - o.VX2_1; if (d3 >= 0) y = d3 * o.Grad1 + o.PY2; else { const t = d2 / o.VX2_1; y = ((t * a1 + b1) * t + c1) * t + o.PY1; } } }
        if (o.by_coef) y *= x; dst[o.out_param] = y; break;
      }
      case 'ConnectEZParamLinkLinear': {
        const src = state[o.in_op], dst = state[o.out_op]; if (!(o.in_param in src) || !(o.out_param in dst)) break;
        let y = o.scale * src[o.in_param] + o.offset; if (o.enable_min) y = Math.max(o.clamp_min, y); if (o.enable_max) y = Math.min(o.clamp_max, y); dst[o.out_param] = y; break;
      }
      case 'TargetBendSTRoll': {
        const ts = Math.tan(p.BendS * 0.5), tt = Math.tan(p.BendT * 0.5); const k = 2 / (tt * tt + ts * ts + 1), m = k - 1;
        const dir = [0, 1, 2].map(i => (tt * o.up[i] * k + m * o.aim[i]) - ts * o.cross[i] * k);
        let q = findBetween(o.aim, dir);
        if (Math.abs(p.Roll) > 0.0001) { const [s, c] = sinCosFast(p.Roll * 0.5); q = qmul(q, [o.aim[0] * s, o.aim[1] * s, o.aim[2] * s, c]); }
        let r = qmul(o.neutral_rotate, q); if (o.base >= 0) r = qmul(qnormIfNeeded(toKD(pose[o.base].r)), r);
        pose[o.target].r = fromKD(r); break;
      }
      case 'TargetBendRoll': case 'TargetRotate': {
        let q = [0, 0, 0, 1];
        if (o.source_quat_op >= 0) { const sq = o.type === 'TargetRotate' ? state[o.source_quat_op].RotateQuat : state[o.source_quat_op].BendingQuat; if (o.as_quat_angle) throw new Error('AsQuatAngle path (0x143ac2c30) not modelled'); q = qnorm(slerpNN([0, 0, 0, 1], sq, p.QuatAngle)); }
        if (o.type === 'TargetBendRoll' && Math.abs(p.Roll) > 0.0001) { const [s, c] = sinCosFast(p.Roll * 0.5); q = qmul(q, [o.aim[0] * s, o.aim[1] * s, o.aim[2] * s, c]); }
        let r = qmul(o.neutral_rotate, q); if (o.base >= 0) r = qmul(qnormIfNeeded(toKD(pose[o.base].r)), r);
        pose[o.target].r = fromKD(r); break;
      }
      case 'TargetTranslate': {
        const T = [p.TranslateX, p.TranslateY, p.TranslateZ]; let out;
        if (o.base < 0) { const a = qrot(o.neutral_rotate, T); out = [o.neutral_translate[0] + a[0], o.neutral_translate[1] + a[1], o.neutral_translate[2] + a[2]]; }
        else { const b = pose[o.base]; const q = qnormIfNeeded(toKD(b.r)); const s = baseScale(b.s); const tm = swapYZ(b.t);
          const a = qrot(q, [s[0] * o.neutral_translate[0], s[1] * o.neutral_translate[1], s[2] * o.neutral_translate[2]]);
          const c = qrot(qmul(q, o.neutral_rotate), [s[0] * T[0], s[1] * T[1], s[2] * T[2]]);
          out = [tm[0] + a[0] + c[0], tm[1] + a[1] + c[1], tm[2] + a[2] + c[2]]; }
        pose[o.target].t = swapYZ(out); break;
      }
      case 'TargetScale': {
        let s = [p.ScaleX, p.ScaleY, p.ScaleZ]; if (o.clamp_zero) s = s.map(x => x < 0.0001 ? 0.0001 : x);
        if (o.base >= 0) { const b = baseScale(pose[o.base].s); s = [s[0] * b[0], s[1] * b[1], s[2] * b[2]]; }
        if (o.segment_scale_compensate && o.target_parent >= 0 && o.base >= 0) { const b = pose[o.base].s; s = [s[0] / b[0], s[1] / b[1], s[2] / b[2]]; }
        pose[o.target].s = s; break;
      }
      case 'ComputeSpaceBases': { // FTransform::Multiply(local, parent) + normalize (positive scales assumed; EnableScaleOpChildSSC false)
        const l = local[o.target], P = pose[o.parent];
        const sl = [P.s[0] * l.t[0], P.s[1] * l.t[1], P.s[2] * l.t[2]]; const rt = qrot(P.r, sl);
        pose[o.target] = { r: qnorm(qmul(P.r, l.r)), t: [rt[0] + P.t[0], rt[1] + P.t[1], rt[2] + P.t[2]], s: [l.s[0] * P.s[0], l.s[1] * P.s[1], l.s[2] * P.s[2]] }; break;
      }
      default: throw new Error('operator type not modelled: ' + o.type);
    }
    if (trace) trace(o, p);
  }
}
// helper: component-space pose from local pose (stock FillComponentSpaceTransforms)
function componentSpace(bones, local) {
  const cs = [];
  for (let i = 0; i < bones.length; i++) {
    const l = local[i], pi = bones[i].parent;
    if (pi < 0) { cs.push({ r: l.r.slice(), t: l.t.slice(), s: l.s.slice() }); continue; }
    const P = cs[pi]; const rt = qrot(P.r, [P.s[0] * l.t[0], P.s[1] * l.t[1], P.s[2] * l.t[2]]);
    cs.push({ r: qnorm(qmul(P.r, l.r)), t: [rt[0] + P.t[0], rt[1] + P.t[1], rt[2] + P.t[2]], s: [l.s[0] * P.s[0], l.s[1] * P.s[1], l.s[2] * P.s[2]] });
  }
  return cs;
}
module.exports = { createState, evaluate, componentSpace, qmul, qconj, qrot, qnorm, toKD, fromKD, PARAM_SLOTS };
