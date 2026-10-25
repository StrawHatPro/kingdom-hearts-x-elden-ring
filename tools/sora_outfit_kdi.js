#!/usr/bin/env node
// tools/sora_outfit_kdi.js (ER73): AN OUTFIT'S OWN KINEDRIVER (its SQEX_KineDriverData, Character/pc/<model>/mdl/<model>_0_kdi)
// in the schema of build/sora_pack/data/kinedriver.json (the builder build/sora_tmp/kd/build_json.js, here for any
// skeleton): build/sora_pack/outfits/<id>/kinedriver.json (a NEW file). The renderer (crates/kh-er-render outfit.rs) runs
// it on the outfit's pose: its helper bones (knee, hip, elbow, shoulder volumes) follow ITS rules, not Sora's.
//
//   node tools/sora_outfit_kdi.js
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const PACK = path.join(ROOT, 'build/sora_pack');
const PKG = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content');
const VERSION = '1.0.0';

// the tagged properties as plain JSON (build/sora_tmp/kd/extract_raw.js)
const fbuf = Buffer.alloc(4);
function num(x) { if (x && typeof x === 'object' && 'f32bits' in x) { fbuf.writeUInt32LE(x.f32bits >>> 0, 0); return fbuf.readFloatLE(0); } return x; }
function val(p) {
  if (p.type === 'StructProperty') { const v = p.value; if (Array.isArray(v)) return props(v); const o = {}; for (const k in v) o[k] = num(v[k]); return o; }
  if (p.type === 'ArrayProperty') { const v = p.value; if (Array.isArray(v)) return v.map((x) => (x && typeof x === 'object' && !('f32bits' in x) ? (Array.isArray(x) ? props(x) : Object.fromEntries(Object.entries(x).map(([k, y]) => [k, num(y)]))) : num(x)));
    if (v && v.items) return v.items.map((it) => (Array.isArray(it) ? props(it) : Object.fromEntries(Object.entries(it).map(([k, y]) => [k, num(y)])))); return v; }
  if (p.type === 'ByteProperty' && typeof p.value === 'string') return p.value.replace(/^ESQEX_KD_[A-Za-z]+Type_/, '');
  return num(p.value);
}
function props(a) { const o = {}; for (const p of a) o[p.name] = val(p); return o; }

function build(raw, bones) {
  const bn = (i) => (i < 0 ? null : bones[i].name);
  // (the asset's bone indices are those of ITS skeleton asset, which may list the bones in another order than the
  // mesh: the bone is found by its NAME in the outfit's skeleton)
  const byName = new Map(bones.map((b, i) => [b.name, i]));
  const chk = (i, name, ctx) => { if (i < 0 || name === undefined || name === null || name === 'None') return i < 0 ? -1 : (bones[i] ? i : -1); const k = byName.get(name); if (k === undefined) throw new Error('bone not in the skeleton: ' + name + ' (' + ctx + ')'); return k; };
  const v3 = (v) => [v.x, v.y, v.z], q4 = (q) => [q.x, q.y, q.z, q.w];
  const bodyOf = { ConnectEquals: 'ConnectEqualsBody', TargetPoscns: 'TargetPoscnsBody', TargetDircns: 'TargetDircnsBody', ComputeSpaceBases: 'ComputeSpaceBasesBody', SourceRotate: 'SourceRotateBody', TargetTranslate: 'TargetTranslateBody', TargetScale: 'TargetScaleBody', TargetRotate: 'TargetRotateBody', TargetBendRoll: 'TargetBendRollBody', TargetBendSTRoll: 'TargetBendSTRollBody', ConnectEZParamLink: 'ConnectEZParamLinkBody', ConnectEZParamLinkLinear: 'ConnectEZParamLinkLinearBody' };
  const ops = raw.Operators.map((h, i) => {
    const b = (raw[bodyOf[h.OpType]] || [])[h.OperatorBody]; if (!b) throw new Error('no body ' + i + ' (' + h.OpType + ')');
    const o = { index: i, type: h.OpType, asset: { owner: h.Owner, work_index: h.WorkIndex, body_index: h.OperatorBody } };
    const base = () => { o.base = chk(b.BaseSpace, b.BaseSpaceBoneName, 'base' + i); o.base_name = bn(o.base); };
    const tgt = () => { o.target = chk(b.TargetObject, b.TargetObjectBoneName, 'tgt' + i); o.target_name = bn(o.target); };
    switch (h.OpType) {
      case 'ComputeSpaceBases': tgt(); o.parent = bones[o.target].parent; o.parent_name = bn(o.parent); o.is_scale_op_child = b.IsScaleOpChild; break;
      case 'SourceRotate': o.sources = b.SourceArray.map((s, k) => chk(s, b.SourceBoneNameArray[k], 'src' + i)); o.source_names = o.sources.map(bn); o.weights = b.WeightArray; base();
        o.segment_scale_compensate = b.SegmentScaleCompensate; o.neutral_rotate = q4(b.NeutralRotate); o.aim = v3(b.AimVector); o.up = v3(b.UpVector); o.cross = v3(b.CrossVector); break;
      case 'TargetTranslate': tgt(); o.translate = v3(b.Translate); base(); o.neutral_translate = v3(b.NeutralTranslate); o.neutral_rotate = q4(b.NeutralRotate); break;
      case 'TargetScale': tgt(); o.target_parent = bones[o.target].parent; o.scale = v3(b.Scale); o.clamp_zero = b.ClampZero; base(); o.segment_scale_compensate = b.SegmentScaleCompensate; break;
      case 'TargetRotate': tgt(); o.source_quat_op = b.SourceQuat; o.quat_weight = b.QuatWeight; o.as_quat_angle = b.AsQuatAngle; base(); o.segment_scale_compensate = b.SegmentScaleCompensate; o.neutral_rotate = q4(b.NeutralRotate); break;
      case 'TargetBendRoll': tgt(); o.source_quat_op = b.SourceQuat; o.quat_weight = b.QuatWeight; o.as_quat_angle = b.AsQuatAngle; o.roll = b.Roll; base(); o.segment_scale_compensate = b.SegmentScaleCompensate; o.neutral_rotate = q4(b.NeutralRotate); o.aim = v3(b.AimVector); break;
      case 'TargetBendSTRoll': tgt(); o.bend_s = b.BendS; o.bend_t = b.BendT; o.roll = b.Roll; base(); o.segment_scale_compensate = b.SegmentScaleCompensate; o.neutral_rotate = q4(b.NeutralRotate); o.aim = v3(b.AimVector); o.up = v3(b.UpVector); o.cross = v3(b.CrossVector); break;
      case 'ConnectEZParamLink': o.in_op = b.InOperator; o.in_param = b.InParameterType; o.out_op = b.OutOperator; o.out_param = b.OutParameterType; o.other_target_param_index = b.OtherTargetParamIndex;
        for (const k of ['PX0', 'VX1_0', 'VX2_1', 'Grad0', 'Grad1', 'PY0', 'PY0A', 'PY0B', 'PY1', 'PY1A', 'PY1B', 'PY2']) o[k] = b[k]; o.by_coef = b.ByCoef; break;
      case 'ConnectEZParamLinkLinear': o.in_op = b.InOperator; o.in_param = b.InParameterType; o.out_op = b.OutOperator; o.out_param = b.OutParameterType; o.other_target_param_index = b.OtherTargetParamIndex;
        o.scale = b.Scale; o.offset = b.Offset; o.clamp_min = b.ClampMin; o.clamp_max = b.ClampMax; o.enable_min = b.EnableMin; o.enable_max = b.EnableMax; break;
      // (ER73) the operators of the outfits' assets Sora's lacks (runtime: 0x143ab0f40 Equals, 0x143abc500 Poscns,
      // 0x143ab97b0 Dircns)
      case 'ConnectEquals': o.in_op = b.InOperator; o.in_param = b.InParameterType; o.out_op = b.OutOperator; o.out_param = b.OutParameterType; o.coef = b.Coef ?? 1; break;
      case 'TargetPoscns': tgt(); o.sources = b.SourceArray.map((x, k) => chk(x, b.SourceBoneNameArray[k], 'psrc' + i)); o.source_names = o.sources.map(bn); o.weights = b.WeightArray; o.offsets = (b.OffsetArray || []).map(v3); o.offset_translate = v3(b.OffsetTranslate); o.orient_affect = !!b.OrientAffect; o.scale_affect = !!b.ScaleAffect; break;
      case 'TargetDircns': tgt(); o.parent = bones[o.target].parent; o.aim_object = chk(b.AimObject, b.AimObjectBoneName, 'aim' + i); o.up_object = b.UpObject >= 0 ? chk(b.UpObject, b.UpObjectBoneName, 'up' + i) : -1;
        o.neutral_rotate = q4(b.NeutralRotate); o.aim = v3(b.AimVector); o.up = v3(b.UpVector); o.cross = v3(b.CrossVector); o.aim_offset = v3(b.AimSrcOffset); o.up_offset = v3(b.UpSrcOffset); break;
      default: throw new Error('unhandled ' + h.OpType);
    }
    return o;
  });
  const driven = {}; for (const o of ops) if (o.out_op !== undefined) (driven[o.out_op] = driven[o.out_op] || []).push(o.out_param);
  for (const o of ops) if (/^Target/.test(o.type)) o.connect_driven_params = driven[o.index] || [];
  const role = bones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, written: false, writes: [], read_as: [] }));
  ops.forEach((o) => {
    const w = (i, what) => { role[i].written = true; role[i].writes.push({ op: o.index, what }); }; const r = (i, what) => { if (i >= 0 && !role[i].read_as.includes(what)) role[i].read_as.push(what); };
    if (o.type === 'ComputeSpaceBases') { w(o.target, 'recompute_component_space'); r(o.parent, 'csb_parent'); }
    if (o.type === 'TargetTranslate') w(o.target, 'translation'); if (o.type === 'TargetScale') w(o.target, 'scale');
    if (o.type === 'TargetRotate' || o.type === 'TargetBendRoll' || o.type === 'TargetBendSTRoll' || o.type === 'TargetDircns') w(o.target, 'rotation');
    if (o.type === 'TargetPoscns') w(o.target, 'translation');
    if (o.aim_object !== undefined && o.aim_object >= 0) r(o.aim_object, 'aim');
    if (o.sources) o.sources.forEach((s) => r(s, 'source')); if (o.base !== undefined) r(o.base, 'base_space');
  });
  const types = {}; ops.forEach((o) => (types[o.type] = (types[o.type] || 0) + 1));
  return { work_num: raw.WorkNum, pre_compute_objects: raw.PreComputeObjects, computed_objects: raw.ComputedObjects, operator_counts: types, operators: ops, bones: role };
}

function main() {
  const index = JSON.parse(fs.readFileSync(path.join(PACK, 'outfits/outfits.json'), 'utf8'));
  for (const o of index.outfits) {
    if (!o.dir) continue;
    const file = path.join(PKG, `Character/pc/${o.model}/mdl/${o.model}_0_kdi.uasset.json`);
    if (!fs.existsSync(file)) { console.log(`${o.id}: no KineDriver asset`); continue; }
    const bones = JSON.parse(fs.readFileSync(path.join(PACK, o.dir, 'manifest.json'), 'utf8')).skeleton.bones;
    const d = JSON.parse(fs.readFileSync(file, 'utf8'));
    const raw = props(d.exports[0].data.props);
    let built;
    try { built = build(raw, bones); } catch (e) { console.log(`${o.id}: NOT BUILT (${e.stack.split(String.fromCharCode(10)).slice(0, 3).join(' | ')})`); continue; }
    const out = { format: 'sora_kinedriver/1', generator: 'tools/sora_outfit_kdi.js ' + VERSION, source: { asset: `/Game/Character/pc/${o.model}/mdl/${o.model}_0_kdi`, class: 'SQEX_KineDriverData' },
      schema: 'as build/sora_pack/data/kinedriver.json; bone indices into this outfit\'s manifest.json skeleton.bones', settings: { EnableScaleOpChildSSC: false }, ...built };
    fs.writeFileSync(path.join(PACK, o.dir, 'kinedriver.json'), JSON.stringify(out, null, 1) + '\n');
    console.log(`${o.id}: ${built.operators.length} operators, ${built.bones.filter((b) => b.written).length} bones written (${built.bones.filter((b) => b.written).map((b) => b.name).slice(0, 12).join(' ')} ...)`);
  }
}
main();
