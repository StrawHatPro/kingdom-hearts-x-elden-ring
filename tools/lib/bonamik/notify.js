// Effects of the Bonamik anim-notify states on a running evaluator (sim = bnm_eval.create(...)).
//
//   setGroupParam(sim, rows) / restoreGroupParam(sim, rows)     TresAnimNotifyState_Bonamik_ChangeGroupParams begin / end
//   setCharaWind(sim, props) / setCharaWindScale(sim, scale) / clearCharaWind(sim)   TresAnimNotifyState_Bonamik_CharaWind
//
// rows = the notify's m_GroupParams: [{ m_Groups: [names], m_Parameter: 'ESQEX_Bonamik_Parameters_<Name>', m_Value }].
// Source: instance functions 0x143B064B0 (set) / 0x143B018B0 (restore), branch inst+0x33E8 == 0, read in decompiled C
// (build/sora_tmp/bnmsub_notifies_physics.md); chara wind: 0x143B05F10 (build/sora_tmp/native_G_bonamik.md section 6).
// A group name that matches no group of the asset is ignored (as in the game). The set is immediate and absolute; the
// restore writes the default, not the value before the set.
// ASSUMPTION: the defaults restored (per-joint table core+0x37B0, per-cone table core+0x37A0) are the values the
// records have right after setup; this module snapshots them on first use. Who fills those tables was not read.
// Parameters Mass (with its link side effect) and the +0x320.. restores of LFCoeff / ParkRate / MaxLF are implemented
// as the table says; only WindLift and InnerConeStr occur in Sora's stage-1 clips (fall loop 0033).
const { f } = require('./m.js');

const FIELD = { Friction: 'f268', Damping: 'f260', Bounciness: 'f264', WindDrag: 'f26C', WindLift: 'f270', LFCoeff: 'f288', ParkRate: 'f28C', MaxLF: 'f290', LFDrag: 'f294', LFLift: 'f298' };
const paramName = (p) => String(p).replace(/^ESQEX_Bonamik_Parameters_/, '');

function defaultsOf(sim) {
  if (!sim._notifyDefaults) {
    const P = sim.core.f3778.f0;
    sim._notifyDefaults = P.map((p) => ({ f250: p.f250, f254: p.f254, f268: p.f268, f260: p.f260, f264: p.f264, f26C: p.f26C, f270: p.f270, f288: p.f288, f28C: p.f28C, f290: p.f290, f294: p.f294, f298: p.f298, cone: p.f2A8 ? { f28: p.f2A8.f28, f2C: p.f2A8.f2C } : null }));
  }
  return sim._notifyDefaults;
}
function jointsOf(sim, groups) {
  const P = sim.core.f3778.f0; const names = sim.data.groups.map((g) => g.name);
  if (!groups || groups.length === 0) return P.map((p, i) => i);
  const out = [];
  for (const n of groups) { const g = names.findIndex((x) => x.toLowerCase() === String(n).toLowerCase()); if (g < 0) continue; P.forEach((p, i) => { if (p.f2B8 === g) out.push(i); }); }
  return out;
}
function apply(sim, rows, restore) {
  const P = sim.core.f3778.f0, D = defaultsOf(sim); const touched = [];
  for (const row of rows) {
    const name = paramName(row.m_Parameter);
    for (const i of jointsOf(sim, row.m_Groups)) {
      const J = P[i], d = D[i];
      if (name === 'Mass') { const v = restore ? d.f250 : f(row.m_Value); if (restore || v > 1e-4) { J.f250 = v; J.f254 = restore ? d.f254 : f(1 / v); } }
      else if (name === 'InnerConeStr') { if (J.f2A8) { J.f2A8.f28 = restore ? d.cone.f28 : f(row.m_Value); touched.push(J.name); } }
      else if (name === 'OuterConeStr') { if (J.f2A8) { J.f2A8.f2C = restore ? d.cone.f2C : f(row.m_Value); touched.push(J.name); } }
      else if (FIELD[name]) { J[FIELD[name]] = restore ? d[FIELD[name]] : f(row.m_Value); touched.push(J.name); }
      else throw new Error('Bonamik parameter not handled: ' + row.m_Parameter);
    }
  }
  return touched;
}
const setGroupParam = (sim, rows) => apply(sim, rows, false);
const restoreGroupParam = (sim, rows) => apply(sim, rows, true);

// props: { m_DirectionSpace: enum name or number (UpVec 0, ModelSpace 1, WorldSpace 2), m_DirectionVec: [x,y,z], m_BaseStrength, m_WaveAmplitude, m_WavePeriod, m_Groups: [] (empty = all) }
function setCharaWind(sim, props) {
  const c = sim.core; const dv = props.m_DirectionVec; const v = Array.isArray(dv) ? dv : [dv.x, dv.y, dv.z]; const l = Math.hypot(v[0], v[1], v[2]) || 1;
  const sp = props.m_DirectionSpace; // ESQEX_Bonamik_EmissiveDirection: UpVec 0, ModelSpace 1, WorldSpace 2
  c.f3720 = typeof sp === 'number' ? sp : /ModelSpace/.test(String(sp)) ? 1 : /WorldSpace/.test(String(sp)) ? 2 : 0;
  c.f3724 = [f(v[0] / l), f(v[1] / l), f(v[2] / l)]; c.f3730 = c;
  c.f3738 = f(props.m_BaseStrength); c.f373C = f(props.m_WaveAmplitude); c.f3740 = props.m_WavePeriod > 0 ? f(1 / props.m_WavePeriod) : 0; c.f3744 = 1;
  sim._charaWindGroups = (props.m_Groups && props.m_Groups.length) ? jointsOf(sim, props.m_Groups).map((i) => sim.core.f3778.f0[i].f2B8).filter((g, k, a) => a.indexOf(g) === k) : sim.data.groups.map((g, i) => i);
  setCharaWindScale(sim, 1);
}
function setCharaWindScale(sim, scale) { for (const g of sim._charaWindGroups || []) sim.core.f954[g] = f(scale); }
function clearCharaWind(sim) { setCharaWindScale(sim, 0); sim.core.f3744 = 0; sim._charaWindGroups = []; }

module.exports = { setGroupParam, restoreGroupParam, setCharaWind, setCharaWindScale, clearCharaWind };
