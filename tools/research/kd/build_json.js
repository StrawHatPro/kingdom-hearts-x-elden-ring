// builds build/sora_tmp/sora_kinedriver.json from the decoded p_ex001_0_kdi package + pack manifest
const fs = require('fs'), path = require('path');
const raw = require('./extract_raw.js');
const man = require('../../sora_pack/manifest.json');
const bones = man.skeleton.bones; const skinSet = new Set(man.skin.joints.map(j => j.bone));
const bn = i => i < 0 ? null : bones[i].name;
const chk = (i, name, ctx) => { if (i >= 0 && bones[i].name !== name) throw new Error('name mismatch ' + ctx + ' ' + i + ' ' + name + ' vs ' + bones[i].name); return i; };
const v3 = v => [v.x, v.y, v.z], q4 = q => [q.x, q.y, q.z, q.w];
const bodyOf = { ComputeSpaceBases: 'ComputeSpaceBasesBody', SourceRotate: 'SourceRotateBody', TargetTranslate: 'TargetTranslateBody', TargetScale: 'TargetScaleBody', TargetRotate: 'TargetRotateBody', TargetBendRoll: 'TargetBendRollBody', TargetBendSTRoll: 'TargetBendSTRollBody', ConnectEZParamLink: 'ConnectEZParamLinkBody', ConnectEZParamLinkLinear: 'ConnectEZParamLinkLinearBody' };
const ops = raw.Operators.map((h, i) => {
  const b = raw[bodyOf[h.OpType]][h.OperatorBody]; if (!b) throw new Error('no body ' + i);
  const o = { index: i, type: h.OpType, asset: { owner: h.Owner, work_index: h.WorkIndex, body_index: h.OperatorBody } };
  const base = () => { o.base = chk(b.BaseSpace, b.BaseSpaceBoneName, 'base' + i); o.base_name = bn(b.BaseSpace); };
  const tgt = () => { o.target = chk(b.TargetObject, b.TargetObjectBoneName, 'tgt' + i); o.target_name = bn(b.TargetObject); };
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
    default: throw new Error('unhandled ' + h.OpType);
  }
  return o;
});
// structural checks
const warns = [];
for (const o of ops) {
  if (o.in_op !== undefined) {
    if (!(o.in_op < o.index)) warns.push('in_op after connect ' + o.index);
    if (ops[o.in_op].type !== 'SourceRotate') warns.push('in not SourceRotate ' + o.index + ' ' + ops[o.in_op].type);
    if (!/^Target/.test(ops[o.out_op].type)) warns.push('out not Target ' + o.index);
    if (!(o.out_op > o.index)) warns.push('out_op before connect ' + o.index + ' ' + o.out_op);
  }
  if (o.source_quat_op !== undefined && o.source_quat_op >= 0) { if (ops[o.source_quat_op].type !== 'SourceRotate' || o.source_quat_op >= o.index) warns.push('srcquat ' + o.index); }
}
const driven = {}; for (const o of ops) if (o.out_op !== undefined) { (driven[o.out_op] = driven[o.out_op] || []).push(o.out_param); }
const dup = []; for (const k in driven) { const s = new Set(driven[k]); if (s.size !== driven[k].length) dup.push(+k); }
for (const o of ops) if (/^Target/.test(o.type)) o.connect_driven_params = (driven[o.index] || []);
// per bone role
const role = bones.map(b => ({ index: b.index, name: b.name, parent: b.parent, skinned: skinSet.has(b.index), written: false, writes: [], read_as: [] }));
ops.forEach(o => {
  const w = (i, what) => { role[i].written = true; role[i].writes.push({ op: o.index, what }); }; const r = (i, what) => { if (i >= 0 && !role[i].read_as.includes(what)) role[i].read_as.push(what); };
  if (o.type === 'ComputeSpaceBases') { w(o.target, 'recompute_component_space'); r(o.parent, 'csb_parent'); }
  if (o.type === 'TargetTranslate') w(o.target, 'translation'); if (o.type === 'TargetScale') w(o.target, 'scale');
  if (o.type === 'TargetRotate' || o.type === 'TargetBendRoll' || o.type === 'TargetBendSTRoll') w(o.target, 'rotation');
  if (o.sources) o.sources.forEach(s => r(s, 'source')); if (o.base !== undefined) r(o.base, 'base_space');
});
const written = role.filter(r => r.written).map(r => r.index);
const setEq = (a, b) => a.length === b.length && a.every(x => b.includes(x));
const inputs = role.filter(r => r.read_as.length).map(r => r.index);
const info = { computed_equals_written: setEq(raw.ComputedObjects, written), computed_minus_written: raw.ComputedObjects.filter(x => !written.includes(x)).map(bn), written_minus_computed: written.filter(x => !raw.ComputedObjects.includes(x)).map(bn),
  precompute_minus_inputs_and_written: raw.PreComputeObjects.filter(x => !inputs.includes(x) && !written.includes(x)).map(bn), inputs_not_in_precompute: inputs.filter(x => !raw.PreComputeObjects.includes(x)).map(bn), written_not_in_precompute: written.filter(x => !raw.PreComputeObjects.includes(x)).map(bn), warnings: warns, targets_with_duplicate_param_drivers: dup };
console.log(JSON.stringify(info));
const stale = []; for (const b of bones) { if (role[b.index].written) continue; let p = b.parent, anc = null; while (p >= 0) { if (role[p].written) { anc = p; break; } p = bones[p].parent; } if (anc !== null) stale.push({ bone: b.index, name: b.name, nearest_written_ancestor: bn(anc), skinned: skinSet.has(b.index) }); }
const types = {}; ops.forEach(o => types[o.type] = (types[o.type] || 0) + 1);
const out = {
  format: 'sora_kinedriver/1',
  source: { asset: '/Game/Character/pc/p_ex001/mdl/p_ex001_0_kdi', class: 'SQEX_KineDriverData', decoded: 'assets_converted/packages/KINGDOM HEARTS III/Content/Character/pc/p_ex001/mdl/p_ex001_0_kdi.uasset.json', extractor: 'build/sora_tmp/kd/build_json.js', spec: 'build/sora_tmp/native_F_kinedriver.md' },
  schema: {
    spaces: 'All numbers are the raw asset values (KineDriver authoring space, see spec section 3). Bone indices = indices into build/sora_pack/manifest.json skeleton.bones (identical to the asset bone indices; names verified at extraction). Quaternions [x,y,z,w], vectors [x,y,z], angles in radians.',
    evaluation: 'Run operators[] in array order once per frame on the component-space pose, in place. Connect* operators map one float from in_op.in_param to out_op.out_param. Target* and ComputeSpaceBases operators write component-space bone transforms.',
    operator_fields: {
      common: 'index (position in evaluation order = operator id referenced by in_op / out_op / source_quat_op), type, asset{owner, work_index, body_index} (not read by the runtime)',
      SourceRotate: 'sources[] + weights[] (bones blended), base (bone or -1), neutral_rotate, aim, up, cross (cross and segment_scale_compensate are not read by the runtime). Outputs: params RotateAngle, BendingAngle, BendS, BendT, Roll; quats RotateQuat, BendingQuat.',
      ConnectEZParamLink: 'two-segment cubic map: PX0, VX1_0, VX2_1, Grad0, Grad1, PY0, PY0A, PY0B, PY1, PY1A, PY1B, PY2, by_coef',
      ConnectEZParamLinkLinear: 'out = scale*in + offset, clamped below by clamp_min when enable_min, above by clamp_max when enable_max',
      TargetBendSTRoll: 'target bone rotation from bend_s, bend_t, roll (values here are the asset defaults; the ones listed in connect_driven_params are overwritten every frame), base, neutral_rotate, aim, up, cross',
      TargetBendRoll: 'target bone rotation from BendingQuat of operator source_quat_op (or identity when -1) slerped by quat_weight, then roll about aim; base, neutral_rotate',
      TargetRotate: 'target bone rotation from RotateQuat of operator source_quat_op slerped by quat_weight; base, neutral_rotate',
      TargetTranslate: 'target bone translation: translate (param), neutral_translate, neutral_rotate, base',
      TargetScale: 'target bone scale: scale (param), clamp_zero, base, segment_scale_compensate, target_parent',
      ComputeSpaceBases: 'component-space transform of target := local(animation) * component-space(parent); is_scale_op_child only matters when EnableScaleOpChildSSC (false for Sora)'
    },
    bones: 'one entry per skeleton bone: written (KineDriver writes its component-space transform), writes[] (operator index + component), read_as[] (roles as input), skinned (bone carries vertex weights in the pack)',
    stale_descendants: 'bones with a KineDriver-written ancestor that no ComputeSpaceBases operator recomputes: their component-space transform stays the one of the animation pose'
  },
  settings: { kbd_element: 'default', KineDriverIndex: 0, EnableScaleOpChildSSC: false, TickGroup: 'TG_PostPhysics', ComponentTag: 'None', EnableLOD: false, MinScreenSize: 0.009999999776482582, EnableFrustumCulling: false, EnableCheckDrawn: false },
  work_num: raw.WorkNum, pre_compute_objects: raw.PreComputeObjects, computed_objects: raw.ComputedObjects,
  operator_counts: types, list_checks: info,
  operators: ops, bones: role, stale_descendants: stale
};
fs.writeFileSync(path.join(__dirname, '../sora_kinedriver.json'), JSON.stringify(out, null, 1));
console.log(JSON.stringify(types), 'written bones', written.length, 'skinned&written', role.filter(r => r.written && r.skinned).length, 'stale', stale.length);
console.log('stale:', stale.map(s => s.name + (s.skinned ? '*' : '') + '<' + s.nearest_written_ancestor).join(' '));
console.log('written:', role.filter(r => r.written).map(r => r.name + (r.skinned ? '*' : '') + '[' + [...new Set(r.writes.map(w => w.what[0]))].join('') + ']').join(' '));
console.log('inputs:', role.filter(r => r.read_as.length).map(r => r.name + ':' + r.read_as.map(x => x[0]).join('')).join(' '));
