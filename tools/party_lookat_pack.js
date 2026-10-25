#!/usr/bin/env node
// tools/party_lookat_pack.js (party I2): the friends' FACE / HEAD LOOK-AT data -> build/sora_pack/party/n_ex00x/data/lookat.json
// (read by kh-anim Pack::load as `look_at`; kh-anim src/lookat.rs runs it).
//
//   node tools/party_lookat_pack.js            both members
//
// Read from the game's data (decoded packages under assets_converted/packages):
//   Character/npc/n_ex00x/anm/n_ex00x_Anim (the AnimBlueprint): its one TresAnimNode_ExLookAt node (m_LookAtBaseBone,
//   m_LookAtBaseOffset, m_RotationLimitDegMin / Max, m_RotationBackMarginDeg, m_RotationSpeedDeg, m_RotBones, m_EyeBones,
//   m_LodLimit, m_bIsDirectLook) and the anim-instance CDO's look-at location buffer (m_LookAtLocationBufNum,
//   m_bUseLookAtLocationUpdateInterval, m_LookAtLocationUpdateInterval; class defaults from the UTresCharAnimInstance
//   ctor 0x140570C60: false / 0.3 / 0).
//   Every TresAnimNotifyState_LookAt of the body clips is already in the pack's manifest (notifies, with
//   m_StartInterpolationTime / m_EndInterpolationTime / m_ReverseTrg).
// Local build output, never distributed.
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const CONTENT = path.join(ROOT, 'assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
const OUT = path.join(ROOT, 'build', 'sora_pack', 'party');

function props(list) {
  const o = {};
  for (const p of list || []) o[p.name] = p;
  return o;
}
function val(p, d) { return p === undefined ? d : p.value; }
function boneName(p) { const v = props(val(p, [])); return val(v.BoneName, null); }
function rot(p, d) { const v = val(p, null) || d; return [v.pitch, v.yaw, v.roll]; }

function walk(o, f) {
  if (Array.isArray(o)) { for (const x of o) walk(x, f); return; }
  if (o && typeof o === 'object') { f(o); for (const k of Object.keys(o)) walk(o[k], f); }
}

for (const chara of ['n_ex001', 'n_ex002']) {
  const rel = `Character/npc/${chara}/anm/${chara}_Anim`;
  const pkg = JSON.parse(fs.readFileSync(path.join(CONTENT, rel + '.uasset.json'), 'utf8'));
  const nodes = [];
  walk(pkg, o => { if (o.structName === 'TresAnimNode_ExLookAt' && Array.isArray(o.value)) nodes.push(props(o.value)); });
  if (nodes.length !== 1) throw new Error(`${chara}: ${nodes.length} TresAnimNode_ExLookAt nodes (expected 1)`);
  const n = nodes[0];
  // The anim instance's CDO properties (Default__n_ex00x_Anim_C): any export carrying the look-at buffer names.
  let bufNum = 0, useInterval = false, interval = 0.3;
  walk(pkg, o => {
    if (o.name === 'm_LookAtLocationBufNum' && o.type === 'IntProperty') bufNum = o.value;
    if (o.name === 'm_bUseLookAtLocationUpdateInterval' && o.type === 'BoolProperty') useInterval = o.value;
    if (o.name === 'm_LookAtLocationUpdateInterval' && o.type === 'FloatProperty') interval = o.value;
  });
  const items = (val(n.m_RotBones, {}) || {}).items || [];
  const eyes = (val(n.m_EyeBones, {}) || {}).items || [];
  const off = val(n.m_LookAtBaseOffset, { x: 0, y: 0, z: 0 });
  const out = {
    format: 'kh3-party-lookat/1',
    source: `/Game/${rel} (TresAnimNode_ExLookAt + anim-instance CDO), written by tools/party_lookat_pack.js (party I2)`,
    base_bone: boneName(n.m_LookAtBaseBone),
    base_offset: [off.x, off.y, off.z],
    rotation_limit_min_deg: rot(n.m_RotationLimitDegMin, { pitch: 0, yaw: 0, roll: 0 }),
    rotation_limit_max_deg: rot(n.m_RotationLimitDegMax, { pitch: 0, yaw: 0, roll: 0 }),
    back_margin_deg: val(n.m_RotationBackMarginDeg, 0),
    rotation_speed_deg: val(n.m_RotationSpeedDeg, 0),
    direct_look: val(n.m_bIsDirectLook, false),
    lod_limit: val(n.m_LodLimit, -1),
    rot_bones: items.map(it => { const p = props(it); return { bone: boneName(p.m_Bone), yaw: val(p.m_YawCoefficientData, 0), pitch: val(p.m_PitchCoefficientData, 0), param_a: val(p.m_ParamACoefficientData, 0) }; }),
    eye_bones: eyes.map(it => { const p = props(it); return { bone: boneName(p.m_Bone), min_deg: rot(p.m_RotLimitMin, { pitch: 0, yaw: 0, roll: 0 }), max_deg: rot(p.m_RotLimitMax, { pitch: 0, yaw: 0, roll: 0 }) }; }),
    anim_instance: { location_buf_num: bufNum, use_location_update_interval: useInterval, location_update_interval: interval },
    pawn: {
      note: 'LookAtRate = pawn vt+0x10C8 0x1405BA830 = +0x788 (focus interpolator) x +0x770 (notify interpolator), both FUN_143f0aa50 interpolators ticked by 0x1405B2190; AI BeginPlay 0x14054B1C0 sets the notify one to 1 / 1; ATresAIController_BT 0x140527460 (NPC controller vtable 0x14684C4E8 slot 0xA48) every tick: GetFocalPoint valid -> SetLookAt(1, 0.5) + SetLookAtLocation(focal point) else SetLookAt(0, 0.5); TresAnimNotifyState_LookAt 0x140563C90 / 0x140565C90 -> pawn anim interface +0x440 slots 0x288 (on, time) / 0x290 (off, time), m_ReverseTrg swaps them',
      focus_time: 0.5,
      notify_initial: 1.0,
    },
  };
  // The EYE-LOOK blend spaces of the face anim set (the face rows whose asset is a BlendSpace, e.g. NO_0_Eye =
  // n_ex00x_BS_id): the face montage of such a row is fed the anim instance's LookAtEyeRot as (-yaw, pitch) with
  // LookAtRate as its alpha (0x14056BC10 -> 0x143F4D9B0: montage instance +0x1B0 / +0x1E0). Samples in asset order,
  // the stock 2D grid ((grid_num + 1) x (grid_num_y + 1) points, X outer) with up to 3 (sample, weight) per point.
  const manifest = JSON.parse(fs.readFileSync(path.join(OUT, chara, 'manifest.json'), 'utf8'));
  const eyeSpaces = [];
  for (const row of manifest.face_animations || []) {
    if (!row.blend_space || row.blend_space.class !== 'BlendSpace') continue;
    const bsRel = row.blend_space.asset.replace(/^\/Game\//, '').replace(/\.[^/.]+$/, '');
    const bs = JSON.parse(fs.readFileSync(path.join(CONTENT, bsRel + '.uasset.json'), 'utf8'));
    let grid = null, samples = null;
    walk(bs, o => {
      if (o.name === 'GridSamples' && o.value && o.value.items) grid = o.value.items.map(it => { const ind = [], w = []; for (const p of it) { if (p.name === 'Indices') ind[p.arrayIndex || 0] = p.value; if (p.name === 'Weights') w[p.arrayIndex || 0] = p.value; } return ind.map((i, k) => [i, w[k] || 0]); });
      if (o.name === 'SampleData' && o.value && o.value.items) samples = o.value.items.map(it => { const a = it.find(p => p.name === 'Animation'); return a; });
    });
    // the sample clips by the manifest's own sample list (same order as the asset's SampleData)
    const stems = (row.blend_space.samples || []).map(s => s.asset.split('/').pop().split('.')[0]);
    if (!grid || !samples || samples.length !== stems.length) throw new Error(`${chara} ${row.name}: blend space grid / samples not read`);
    const ax = row.blend_space.axis, ay = row.blend_space.axis_y;
    eyeSpaces.push({ row: row.name, axis: [ax.min, ax.max, ax.grid_num], axis_y: [ay.min, ay.max, ay.grid_num], samples: stems, grid });
  }
  out.eye_blend_spaces = eyeSpaces;
  out.eye_input_note = 'input = (-LookAtEyeRot.yaw, LookAtEyeRot.pitch) (0x14056BC10), mixed from the default (0, 0) by LookAtRate [inf: the montage advance that reads +0x1B0 / +0x1E0 was not read]';
  const dir = path.join(OUT, chara, 'data');
  if (!fs.existsSync(dir)) throw new Error(`${dir}: no party pack (run tools/party_pack.js first)`);
  fs.writeFileSync(path.join(dir, 'lookat.json'), JSON.stringify(out, null, 1) + '\n');
  console.log(`${chara}: data/lookat.json base ${out.base_bone}, ${out.rot_bones.length} rot bones, ${out.eye_bones.length} eye bones, speed ${out.rotation_speed_deg} deg/s, buffer ${bufNum}`);
}
