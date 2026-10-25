#!/usr/bin/env node
// tools/party_pack.js (party stream A): DONALD (n_ex001) AND GOOFY (n_ex002) as packs in the format of the Sora pack, so that
// kh_anim::Pack::load + kh_anim::Animator pose them with the game's rules, and the test app draws them with the game's own
// material shaders (tools/sora_material_pack.js, outfit mode) at Sora's quality.
//
//   node tools/party_pack.js                 build build/sora_pack/party/ (both members, their 14 + 14 weapons, materials)
//   node tools/party_pack.js --only n_ex001  one member
//   node tools/party_pack.js --no-materials  skip the shader translation (models, clips, data only)
//   node tools/party_pack.js --no-validate   skip gltf-validator (the independent read-back still runs)
//
// Everything that names an asset is read from the game's data (decoded packages under assets_converted/packages):
//   the pawn blueprint n_ex00x_Pawn (+ PawnBase): mesh component (mesh, AnimClass, yaw), root capsule, movement overrides;
//   the AnimBlueprint n_ex00x_Anim: MyAnimSets (body), m_SubAnimSets (face), m_CacheStartPoseAnimName, its graph nodes;
//   the mesh n_ex00x_0: the DEFAULT element of its SQEX_KBDAssetUserData (the KBD component registers the element named
//   "default": build/sora_tmp/native_F_kinedriver.md 5) = which KineDriver data / Bonamik asset run, with their settings;
//   the skeleton asset: sockets (R_wep_skt, R_wep, vfx_*);
//   the weapons: Blueprints/Npc/n_ex00x/w_xx###_Equip (TresEquipWeaponData: AttachBoneName = the socket, RsrcData = the weapon
//   blueprint) -> w_xx###_Wep (+ w_xx_WepBase): TresWeaponMesh0.SkeletalMesh / AnimClass; the 14 rows of TresEquipItemData
//   per character (build/sora_tmp/party/inventory.json, read from Load/Tres/TresEquipItemData).
// Output: build/sora_pack/party/ (layout: docs/party-contracts.md section A.1). Local build output, never distributed.
// Helpers (glb writer, conversion, animation loading, blend-space description, root tracks, validation): tools/sora_pack.js.
'use strict';
const fs = require('fs'), path = require('path'), cp = require('child_process'), crypto = require('crypto');
const sp = require('./sora_pack.js');
const { bonamikExport } = require('./sora_weapon_pack.js');
const { ROOT, CONTENT, sha256, readJson, rel, f32, gamePathToContent, loadPkg, importPath, propsOf, structOf, readGlb, readAccessor, GlbWriter, cvPos, cvDir,
  loadAnimSet, describeAnimAsset, buildSkeleton, addSkeletonNodes, globalBind, invertAffine, loadAnimation, addAnimation, validateGlb, checkGlb, sampleChan, buildRootMotion, footHeight } = sp;

const VERSION = '1.0.0';
const OUT = path.join(sp.OUT, 'party');
const P = (...a) => path.join(...a);
const MEMBERS = [
  { member: 1, npc: 'n_ex001', name: 'Donald', unique: 'TRES_CHR_UID_DONALD', unique_id: 11, key: 'donald', idBase: 100 },
  { member: 2, npc: 'n_ex002', name: 'Goofy', unique: 'TRES_CHR_UID_GOOFY', unique_id: 12, key: 'goofy', idBase: 200 },
];
const args = process.argv.slice(2);
const only = (() => { const k = args.indexOf('--only'); return k >= 0 ? args[k + 1].split(',') : null; })();
const NO_MATERIALS = args.includes('--no-materials'), NO_VALIDATE = args.includes('--no-validate');
const NL = String.fromCharCode(10);

// ------------------------------------------------------------------------------------------------ plain properties
const objPath = (d, i) => (!i ? null : i < 0 ? importPath(d, i) : d.exports[i - 1] ? d.exports[i - 1].objectName : null);
const isProps = (v) => Array.isArray(v) && v.length > 0 && v.every((e) => e && typeof e === 'object' && !Array.isArray(e) && 'name' in e && 'type' in e);
const unbits = (v) => { const b = Buffer.alloc(4); b.writeUInt32LE(v >>> 0); return b.readFloatLE(0); };
function plainProps(d, props) { // (as tools/sora_weapon_pack.js) tagged properties -> plain JSON, references as object paths
  const o = {}, statics = new Set();
  for (const p of props || []) if (p.arrayIndex !== undefined) statics.add(p.name);
  for (const p of props || []) { const v = plainValue(d, p); if (statics.has(p.name)) { if (!(p.name in o)) o[p.name] = []; o[p.name][p.arrayIndex || 0] = v; } else o[p.name] = v; }
  return o;
}
function plainValue(d, p) {
  const v = p.value;
  if (p.type === 'ObjectProperty') return objPath(d, v);
  if (p.type === 'BoolProperty' && p.boolVal !== undefined) return !!p.boolVal;
  if (p.type === 'ArrayProperty') {
    if (p.innerType === 'ObjectProperty') return (Array.isArray(v) ? v : []).map((i) => objPath(d, i));
    if (v && Array.isArray(v.items)) return v.items.map((it) => (isProps(it) || (Array.isArray(it) && !it.length) ? plainProps(d, it) : plainAny(d, it)));
    return plainAny(d, v);
  }
  if (p.type === 'MapProperty') return ((v && v.entries) || []).map((e) => ({ key: plainAny(d, e.key), value: isProps(e.value) ? plainProps(d, e.value) : p.valueType === 'ObjectProperty' ? objPath(d, e.value) : plainAny(d, e.value) }));
  if (p.type === 'StructProperty') return isProps(v) || (Array.isArray(v) && !v.length) ? plainProps(d, v) : plainAny(d, v);
  return plainAny(d, v);
}
function plainAny(d, v) {
  if (v === null || typeof v !== 'object') return v;
  if (isProps(v)) return plainProps(d, v);
  if (Array.isArray(v)) return v.map((x) => plainAny(d, x));
  if ('f32bits' in v) return unbits(v.f32bits);
  if ('s' in v && 'enc' in v) return v.s;
  const o = {}; for (const k of Object.keys(v)) o[k] = plainAny(d, v[k]); return o;
}
const meshGlbFile = (content) => P(ROOT, 'assets_converted', 'meshes', CONTENT, content + '.glb');
const animGlbFile = (content) => P(ROOT, 'assets_converted', 'animations', CONTENT, content + '.glb');
const pkgExists = (content) => fs.existsSync(P(ROOT, 'assets_converted', 'packages', CONTENT, content + '.uasset.json'));

// ------------------------------------------------------------------------------------------------ math (UE units)
const qmul = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
// child (q, t, s) under parent: UE FTransform product child * parent
const under = (c, p) => { const v = qrot(p.q, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { q: qmul(p.q, c.q), t: [v[0] + p.t[0], v[1] + p.t[1], v[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
const point = (T, v) => { const r = qrot(T.q, [T.s[0] * v[0], T.s[1] * v[1], T.s[2] * v[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
function rotatorQuat(r) { // FRotator (degrees) -> FQuat, stock UE 4.17 FRotator::Quaternion()
  const h = Math.PI / 360, sp2 = Math.sin(r.pitch * h), cp2 = Math.cos(r.pitch * h), sy = Math.sin(r.yaw * h), cy = Math.cos(r.yaw * h), sr = Math.sin(r.roll * h), cr = Math.cos(r.roll * h);
  return [cr * sp2 * sy - sr * cp2 * cy, -cr * sp2 * cy - sr * cp2 * sy, cr * cp2 * sy - sr * sp2 * cy, cr * cp2 * cy + sr * sp2 * sy];
}

// ------------------------------------------------------------------------------------------------ 1. identification
function identify(m) {
  const pawnContent = `Blueprints/Npc/${m.npc}/${m.npc}_Pawn`;
  const pawn = loadPkg(pawnContent);
  const cls = pawn.exports.find((e) => e.class === 'BlueprintGeneratedClass');
  const baseClassPath = importPath(pawn, cls.superIndex);
  const base = loadPkg(gamePathToContent(baseClassPath));
  const baseCls = base.exports.find((e) => e.class === 'BlueprintGeneratedClass');
  const nativeParent = importPath(base, baseCls.superIndex);
  const comp = (d, c) => d.exports.find((e) => e.class === c);
  const meshP = plainProps(pawn, comp(pawn, 'TresSkeletalMeshComponent').data.props), meshB = plainProps(base, comp(base, 'TresSkeletalMeshComponent').data.props);
  const meshEff = { ...meshB, ...meshP };
  const rootB = plainProps(base, comp(base, 'TresRootComponent').data.props);
  const moveB = plainProps(base, comp(base, 'TresAICharMovementComponent').data.props);
  const shape = (rootB.CollisionShapesSrc || [])[0] || {};
  const meshPath = meshEff.SkeletalMesh, animClassPath = meshEff.AnimClass;
  const meshContent = gamePathToContent(meshPath);
  const meshPkg = loadPkg(meshContent);
  const imp = (cn) => meshPkg.imports.map((im, i) => ({ im, i })).filter((x) => x.im.className === cn).map((x) => importPath(meshPkg, -(x.i + 1)));
  const anim = loadPkg(gamePathToContent(animClassPath));
  const animCls = anim.exports.find((e) => e.class === 'AnimBlueprintGeneratedClass');
  const cdoE = anim.exports.find((e) => /^Default__/.test(e.objectName));
  const cdo = propsOf(cdoE);
  const objList = (p) => (p ? p.value.filter((v) => v).map((v) => importPath(anim, v)) : []);
  const graphNodes = Object.keys(cdo).filter((k) => /AnimGraphNode_/.test(k)).map((k) => k.replace(/_[0-9A-F]{32}$/, ''));
  return {
    pawn_blueprint: `/Game/${pawnContent}`, pawn_parent_blueprint: baseClassPath, pawn_native_class: nativeParent,
    skeletal_mesh: meshPath, mesh_content: meshContent, anim_class: animClassPath, anim_native_parent: importPath(anim, animCls.superIndex),
    skeleton: imp('Skeleton')[0], physics_asset: imp('PhysicsAsset')[0], material_instances: imp('MaterialInstanceConstant'),
    other_mesh_user_data: { bonamik: imp('SQEX_BonamikAsset'), kine_driver: imp('SQEX_KineDriverData') },
    anim_sets: objList(cdo.MyAnimSets), sub_anim_sets: objList(cdo.m_SubAnimSets), cache_start_pose_anim: cdo.m_CacheStartPoseAnimName && cdo.m_CacheStartPoseAnimName.value,
    anim_graph_nodes: graphNodes,
    leg_ik_nodes: { TwoBoneIKMultiped: graphNodes.filter((n) => /TwoBoneIKMultiped/.test(n)).length, CollTwoBoneIK: graphNodes.filter((n) => /CollTwoBoneIK/.test(n)).length },
    mesh_component: { name: 'TresCharMesh0', relative_location_ue_cm: meshEff.RelativeLocation || { x: 0, y: 0, z: 0 }, relative_rotation_ue_deg: meshEff.RelativeRotation || { pitch: 0, yaw: 0, roll: 0 },
      relative_rotation_note: meshEff.RelativeRotation ? 'serialized in the pawn blueprints' : 'not serialized: engine default', foot_steps: meshEff.m_CheckFootSteps || null, enable_collision_foot_step: meshEff.m_bEnableCollisionFootStep ?? null },
    collision_capsule: { component: 'TresRoot', shape: shape.ShapeType || null, size_ue_cm: shape.Size || null, relative_location_ue_cm: shape.RelativeLocation || null, raw: shape },
    movement_component_overrides: moveB,
    leg_ik_data: legIkData(anim, cdoE, animClassPath),
  };
}

// (party A2) The leg-IK nodes of the character's AnimBlueprint in the schema of the Sora pack's data/legik.json
// (build/sora_tmp/legik_json.js: tagged properties flattened, node_index = LinkID space, bound inputs). Donald
// (n_ex001_Anim): two TresAnimGraphNode_CollTwoBoneIK (R_ashi1 node 8 <- L2C 24; L_ashi1 node 9 <- node 8), no Multiped.
// Goofy (n_ex002_Anim): one TwoBoneIKMultiped (node 23, bUseMayaSolver false). coll_two_bone_ik is written in
// EVALUATION order (a node fed by another coll node comes after it).
function legIkData(d, cdoE, animClassPath) {
  const full = (i) => { if (!i) return null; if (i < 0) { let im = d.imports[-i - 1], s = im.objectName; while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; s = im.objectName + '.' + s; } return s; } return 'E' + i + ':' + d.exports[i - 1].objectName; };
  const v = (p) => { if (p === null || typeof p !== 'object') return p; if (Array.isArray(p)) return p.map(v); if ('name' in p && 'type' in p) { let val = p.value; if (p.type === 'ObjectProperty') val = full(val); else val = v(val); return { [p.name + (p.arrayIndex ? '[' + p.arrayIndex + ']' : '')]: val }; } const o = {}; for (const k in p) o[k] = v(p[k]); return o; };
  const flat = (x) => { if (Array.isArray(x)) { if (x.length && x.every((e) => e && typeof e === 'object' && !Array.isArray(e) && Object.keys(e).length === 1)) { const o = {}; let ok = true; for (const e of x) { const k = Object.keys(e)[0]; if (k in o) { ok = false; break; } o[k] = flat(e[k]); } if (ok) return o; } return x.map(flat); } if (x && typeof x === 'object') { if (x.structName && Array.isArray(x.items)) return x.items.map(flat); const o = {}; for (const k in x) o[k] = flat(x[k]); return o; } return x; };
  const p = flat(v(cdoE.data)).props || {};
  const keys = Object.keys(p).filter((k) => /AnimGraphNode_/.test(k));
  const node = (k, i) => { const n = Object.assign({}, p[k]); const b = n.EvaluateGraphExposedInputs; delete n.EvaluateGraphExposedInputs; const bind = ((b && b.CopyRecords) || []).map((r) => ({ from: r.SourcePropertyName, to: String(r.DestProperty).split('.').pop() })); return { graph_node: k, node_index: i, bound_inputs: bind, properties: n }; };
  let coll = [], multi = [];
  keys.forEach((k, i) => { if (/CollTwoBoneIK/.test(k)) coll.push(node(k, i)); else if (/TwoBoneIKMultiped/.test(k)) multi.push(node(k, i)); });
  const feeds = (a, b) => a.properties.ComponentPose && a.properties.ComponentPose.LinkID === b.node_index; // a's input is b
  coll.sort((a, b) => (feeds(a, b) ? 1 : feeds(b, a) ? -1 : a.node_index - b.node_index));
  if (!coll.length && !multi.length) return null;
  return {
    schema: { note: 'Leg-IK nodes of the friend\'s AnimBlueprint (schema of build/sora_pack/data/legik.json: tagged properties, missing = native default; node_index = LinkID; bound_inputs = anim-instance variable copied into the node before every update). coll_two_bone_ik in evaluation order.', source: animClassPath, written_by: 'tools/party_pack.js legIkData (party A2)',
      native_defaults: { TresAnimNode_CollTwoBoneIK: { ctor: '0x14052ABE0', StartStretchRatio: 1, MaxStretchScale: 1.2, m_MaintainEffectorLimitRot: 25, m_MaintainEffectorStartInterpTime: 0.1, m_MaintainEffectorEndInterpTime: 0.2, CheckMin: 0, CheckMax: 30, IkOffset: 0 },
        TwoBoneIKMultiped: { ctor: '0x14052B380' }, TresAnimNode_TwoBoneIKMultiped: { HipsController: { Gain: 140, damping: 23.6643, maxVelocity: 6000, note: 'Gain forced to 140 by Initialize 0x1405453D0; damping / maxVelocity not reflected (native_D_legik.md 4.4)' } } },
      solver: multi.length ? (multi[0].properties.bUseMayaSolver ? 'Maya (0x140551F90)' : 'stock two-bone (0x14054FC10 -> AnimationCore::SolveTwoBoneIK 0x1437C6D50, RemoveTwist 0x14052C9B0)') : 'CollTwoBoneIK EvaluateSkeletalControl 0x140536200 (stock SolveTwoBoneIK)' },
    coll_two_bone_ik: coll, two_bone_ik_multiped: multi,
  };
}

// The DEFAULT element of the mesh's SQEX_KBDAssetUserData: which KineDriver data and Bonamik asset the KBD component runs
// (its registration takes the element named "default": native_F_kinedriver.md 5), and the other elements (variants).
function kbdElements(meshContent) {
  const d = loadPkg(meshContent);
  const kbd = d.exports.filter((e) => e.class === 'SQEX_KBDAssetUserData');
  const elements = new Map();
  for (const e of kbd) {
    const p = plainProps(d, e.data.props);
    for (const el of p.AssetElements || []) {
      const refs = (el.AssetUserDatas || []).map((i) => (typeof i === 'number' ? i : null));
      const raw = (e.data.props.find((x) => x.name === 'AssetElements') || {}).value;
      // the plain form resolves the export indices to names; read the indices again from the raw value
      elements.set(el.ElementName, elements.get(el.ElementName) || el);
    }
  }
  // raw indices per element (the first KBD object that names the element)
  const out = {};
  for (const e of kbd) {
    const arr = (e.data.props.find((x) => x.name === 'AssetElements') || {}).value;
    for (const it of (arr && arr.items) || []) {
      const s = Object.fromEntries(it.map((x) => [x.name, x.value]));
      const name = s.ElementName; if (out[name]) continue;
      const list = (s.AssetUserDatas || []).map((i) => {
        const x = d.exports[i - 1]; const p = plainProps(d, x.data.props);
        if (x.class === 'SQEX_KineDriver_AssetUserData') { const el = (p.KineDriver_AssetUserDataElement || [])[0] || {}; return { class: x.class, export: x.objectName, kine_driver: el.KineDriverData && el.KineDriverData[0] !== undefined ? el.KineDriverData : el.KineDriverData, settings: el }; }
        if (x.class === 'SQEX_BonamikAssetUserData') return { class: x.class, export: x.objectName, bonamik: p.BonamikAssets, settings: p };
        return { class: x.class, export: x.objectName, props: p };
      });
      out[name] = list;
    }
  }
  return out;
}

// SkeletalMeshSocket exports of a Skeleton package (UE units, the socket's local transform on its bone)
function socketsOf(skeletonContent, bones) {
  const d = loadPkg(skeletonContent);
  return d.exports.filter((e) => e.class === 'SkeletalMeshSocket').map((e) => {
    const p = plainProps(d, e.data.props);
    const loc = p.RelativeLocation || { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation || { pitch: 0, yaw: 0, roll: 0 }, scl = p.RelativeScale || { x: 1, y: 1, z: 1 };
    return { name: p.SocketName, bone: p.BoneName ?? null, bone_index: bones.findIndex((b) => b.name === p.BoneName), relative_location_ue_cm: [loc.x, loc.y, loc.z], relative_rotation_ue_deg: rot, relative_scale: [scl.x, scl.y, scl.z], quaternion_ue_xyzw: rotatorQuat(rot).map((x) => +x.toFixed(9)) };
  });
}
function skeletonBones(skeletonContent) {
  const sk = loadPkg(skeletonContent).exports.find((e) => e.class === 'Skeleton');
  return sk.data.native.referenceSkeleton.refBoneInfo.map((b) => ({ name: b.name, parent: b.parentIndex }));
}

// ------------------------------------------------------------------------------------------------ 2. the model
// LOD 0 skinned to the character's OWN skeleton (sora_pack.js addMesh, any influence count up to 4 sets, own names).
function buildModel(meshGlb, bones, slotNames, label) {
  const j = meshGlb.json; const lod0 = j.meshes[0];
  const cache = new Map(); const rd = (i) => { if (!cache.has(i)) cache.set(i, readAccessor(meshGlb, i)); return cache.get(i); };
  const prims = []; const used = new Set(); let cut = 0, most = 0;
  for (const pr of lod0.primitives) {
    const idx = rd(pr.indices).data; let lo = Infinity, hi = -1; for (const v of idx) { if (v < lo) lo = v; if (v > hi) hi = v; }
    const n = hi - lo + 1; const A = pr.attributes; const sets = [];
    for (let s = 0; A['JOINTS_' + s] !== undefined; s++) { const ja = rd(A['JOINTS_' + s]), wa = rd(A['WEIGHTS_' + s]); sets.push({ j: ja.data, w: wa.data, div: wa.componentType === 5121 ? 255 : wa.componentType === 5123 ? 65535 : 1 }); }
    const J = new Array(n * 4).fill(0), W = new Array(n * 4).fill(0);
    for (let v = 0; v < n; v++) {
      const acc = new Map();
      for (const s of sets) for (let c = 0; c < 4; c++) { const w = s.w[(lo + v) * 4 + c] / s.div; if (w <= 0) continue; const b = s.j[(lo + v) * 4 + c]; acc.set(b, (acc.get(b) || 0) + w); }
      const list = [...acc].sort((a, b) => b[1] - a[1]); if (list.length > most) most = list.length; if (list.length > 4) cut++;
      const top = list.slice(0, 4); let sum = top.reduce((s, x) => s + x[1], 0); if (!(sum > 0)) { top.length = 0; top.push([0, 1]); sum = 1; }
      top.forEach(([b, w], c) => { J[v * 4 + c] = b; W[v * 4 + c] = w / sum; used.add(b); });
    }
    prims.push({ pr, idx, lo, n, J, W });
  }
  const joints = [...used].sort((a, b) => a - b);
  if (joints.length > 256) throw new Error(`${label}: ${joints.length} weighted bones, more than one 256-joint skin`);
  const jointOf = new Map(joints.map((b, k) => [b, k]));
  const w = new GlbWriter(`(party ${label}: tools/party_pack.js ${VERSION})`);
  addSkeletonNodes(w, bones);
  const ibm = globalBind(bones).map(invertAffine);
  w.json.skins = [{ name: label, joints, skeleton: 0, inverseBindMatrices: w.accessor(joints.map((b) => ibm[b]).flat(), 'MAT4', 5126) }];
  const out = []; const stats = { vertices: 0, triangles: 0, primitives: [] };
  for (const p of prims) {
    const A = p.pr.attributes; const slice = (acc, comps) => rd(acc).data.slice(p.lo * comps, (p.lo + p.n) * comps);
    const pos = slice(A.POSITION, 3), nor = slice(A.NORMAL, 3), tan = A.TANGENT !== undefined ? slice(A.TANGENT, 4) : null;
    const P2 = new Array(p.n * 3), N2 = new Array(p.n * 3), T2 = new Array(p.n * 4);
    for (let i = 0; i < p.n; i++) {
      const q = cvPos(pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]); P2[i * 3] = q[0]; P2[i * 3 + 1] = q[1]; P2[i * 3 + 2] = q[2];
      const nn = cvDir(nor[i * 3], nor[i * 3 + 1], nor[i * 3 + 2]); const l = Math.hypot(...nn) || 1; N2[i * 3] = nn[0] / l; N2[i * 3 + 1] = nn[1] / l; N2[i * 3 + 2] = nn[2] / l;
      if (tan) { const tt = cvDir(tan[i * 4], tan[i * 4 + 1], tan[i * 4 + 2]); T2[i * 4] = tt[0]; T2[i * 4 + 1] = tt[1]; T2[i * 4 + 2] = tt[2]; T2[i * 4 + 3] = -tan[i * 4 + 3]; } else { T2[i * 4] = 1; T2[i * 4 + 1] = 0; T2[i * 4 + 2] = 0; T2[i * 4 + 3] = 1; }
    }
    const JJ = p.J.map((b, k) => (p.W[k] === 0 ? 0 : jointOf.get(b)));
    const attributes = { POSITION: w.accessor(P2, 'VEC3', 5126, { minmax: true, target: 34962 }), NORMAL: w.accessor(N2, 'VEC3', 5126, { target: 34962 }), TANGENT: w.accessor(T2, 'VEC4', 5126, { target: 34962 }),
      JOINTS_0: w.accessor(JJ, 'VEC4', 5123, { target: 34962 }), WEIGHTS_0: w.accessor(p.W, 'VEC4', 5126, { target: 34962 }) };
    for (let u = 0; A['TEXCOORD_' + u] !== undefined; u++) attributes['TEXCOORD_' + u] = w.accessor(slice(A['TEXCOORD_' + u], 2), 'VEC2', 5126, { target: 34962 });
    const ix = new Array(p.idx.length); // the reflection is baked into the vertices: the winding is reversed (as tools/sora_pack.js)
    for (let i = 0; i < p.idx.length; i += 3) { ix[i] = p.idx[i] - p.lo; ix[i + 1] = p.idx[i + 2] - p.lo; ix[i + 2] = p.idx[i + 1] - p.lo; }
    out.push({ attributes, indices: w.accessor(ix, 'SCALAR', p.n > 65535 ? 5125 : 5123, { target: 34963 }), mode: 4, material: p.pr.material });
    stats.vertices += p.n; stats.triangles += ix.length / 3; stats.primitives.push({ material_slot: p.pr.material, slot_name: slotNames[p.pr.material], vertices: p.n, triangles: ix.length / 3, has_uv2: A.TEXCOORD_2 !== undefined });
  }
  w.json.meshes = [{ name: label + '_LOD0', primitives: out }];
  // plain glTF materials named by slot (the app replaces them with the translated game shaders; --plain-materials shows these)
  w.json.materials = slotNames.map((n, k) => ({ name: n, pbrMetallicRoughness: { baseColorFactor: [0.6, 0.6, 0.6, 1], metallicFactor: 0, roughnessFactor: 0.8 }, extras: { ue_material: j.materials[k].extras && j.materials[k].extras.ue_material } }));
  const node = w.json.nodes.push({ name: label + '_mesh', mesh: 0, skin: 0 }) - 1;
  w.json.scenes = [{ name: label, nodes: [0, node] }];
  return { glb: w.finish(), stats: { ...stats, skin_joints: joints.length, vertices_cut_to_four: cut, most_influences: most }, joints };
}
// The third UV set of each primitive (Bevy meshes carry two): f32 pairs in vertex order, as sora_material_pack.js writes them.
function uv2Of(glbBytesOrFile, mesh) {
  const g = typeof glbBytesOrFile === 'string' ? readGlb(glbBytesOrFile) : glbBytesOrFile;
  const out = [];
  g.json.meshes[0].primitives.forEach((p, k) => {
    if (p.attributes.TEXCOORD_2 === undefined) return;
    const a = readAccessor(g, p.attributes.TEXCOORD_2); const buf = Buffer.alloc(a.data.length * 4); a.data.forEach((v, i) => buf.writeFloatLE(v, i * 4));
    out.push({ key: `${mesh}:${g.json.materials[p.material].name}`, file: `uv2_${mesh}_${k}.bin`, buf, vertices: a.data.length / 2, primitive: k });
  });
  return out;
}

// ------------------------------------------------------------------------------------------------ 3. clips
// One sequence on the character's skeleton. The Skeleton asset may list bones the MESH does not have (Donald: 252 vs 249):
// their tracks cannot move a vertex and are dropped (UE evaluates the mesh's required bones only) and counted.
function loadClip(gp, bones, skelBones, dropped) {
  const extra = skelBones.filter((b) => !bones.some((x) => x.name === b.name)).map((b) => ({ name: b.name, parent: -1 }));
  const a = loadAnimation(gp, extra.length ? [...bones, ...extra] : bones);
  if (extra.length) {
    const before = a.chans.length;
    a.chans = a.chans.filter((c) => c.bone < bones.length);
    if (a.chans.length !== before) { dropped.clips++; dropped.channels += before - a.chans.length; }
    a.tracked_bones = [...new Set(a.chans.map((c) => c.bone))].sort((x, y) => x - y); a.tracks = a.tracked_bones.length;
  }
  return a;
}
function glbOfClip(bones, name, a, note) { const w = new GlbWriter(note); addSkeletonNodes(w, bones); addAnimation(w, name, a); w.json.scenes = [{ name, nodes: [0] }]; return w.finish(); }

// ------------------------------------------------------------------------------------------------ 4. KineDriver (tools/sora_outfit_kdi.js `build`, any skeleton)
const fbuf = Buffer.alloc(4);
function kdNum(x) { if (x && typeof x === 'object' && 'f32bits' in x) { fbuf.writeUInt32LE(x.f32bits >>> 0, 0); return fbuf.readFloatLE(0); } return x; }
function kdVal(p) {
  if (p.type === 'StructProperty') { const v = p.value; if (Array.isArray(v)) return kdProps(v); const o = {}; for (const k in v) o[k] = kdNum(v[k]); return o; }
  if (p.type === 'ArrayProperty') { const v = p.value; if (Array.isArray(v)) return v.map((x) => (x && typeof x === 'object' && !('f32bits' in x) ? (Array.isArray(x) ? kdProps(x) : Object.fromEntries(Object.entries(x).map(([k, y]) => [k, kdNum(y)]))) : kdNum(x)));
    if (v && v.items) return v.items.map((it) => (Array.isArray(it) ? kdProps(it) : Object.fromEntries(Object.entries(it).map(([k, y]) => [k, kdNum(y)])))); return v; }
  if (p.type === 'ByteProperty' && typeof p.value === 'string') return p.value.replace(/^ESQEX_KD_[A-Za-z]+Type_/, '');
  if (p.type === 'BoolProperty' && p.boolVal !== undefined) return !!p.boolVal;
  return kdNum(p.value);
}
function kdProps(a) { const o = {}; for (const p of a) o[p.name] = kdVal(p); return o; }
function kdBuild(raw, bones) {
  const bn = (i) => (i < 0 ? null : bones[i].name);
  const byName = new Map(bones.map((b, i) => [b.name, i]));
  const chk = (i, name, ctx) => { if (i < 0 || name === undefined || name === null || name === 'None') return i < 0 ? -1 : (bones[i] ? i : -1); const k = byName.get(name); if (k === undefined) throw new Error('bone not in the skeleton: ' + name + ' (' + ctx + ')'); return k; };
  const v3 = (v) => [v.x, v.y, v.z], q4 = (q) => [q.x, q.y, q.z, q.w];
  const bodyOf = { ConnectEquals: 'ConnectEqualsBody', TargetPoscns: 'TargetPoscnsBody', TargetDircns: 'TargetDircnsBody', ComputeSpaceBases: 'ComputeSpaceBasesBody', SourceRotate: 'SourceRotateBody', TargetTranslate: 'TargetTranslateBody', TargetScale: 'TargetScaleBody', TargetRotate: 'TargetRotateBody', TargetBendRoll: 'TargetBendRollBody', TargetBendSTRoll: 'TargetBendSTRollBody', ConnectEZParamLink: 'ConnectEZParamLinkBody', ConnectEZParamLinkLinear: 'ConnectEZParamLinkLinearBody', SourceOther: 'SourceOtherBody' };
  const ops = raw.Operators.map((h, i) => {
    const b = (raw[bodyOf[h.OpType]] || [])[h.OperatorBody]; if (!b) throw new Error('no body ' + i + ' (' + h.OpType + ')');
    const o = { index: i, type: h.OpType, asset: { owner: h.Owner, work_index: h.WorkIndex, body_index: h.OperatorBody } };
    const base = () => { o.base = chk(b.BaseSpace, b.BaseSpaceBoneName, 'base' + i); o.base_name = bn(o.base); };
    const tgt = () => { o.target = chk(b.TargetObject, b.TargetObjectBoneName, 'tgt' + i); o.target_name = bn(o.target); };
    switch (h.OpType) {
      case 'ComputeSpaceBases': tgt(); o.parent = bones[o.target].parent; o.parent_name = bn(o.parent); o.is_scale_op_child = !!b.IsScaleOpChild; break;
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
      case 'ConnectEquals': o.in_op = b.InOperator; o.in_param = b.InParameterType; o.out_op = b.OutOperator; o.out_param = b.OutParameterType; o.coef = b.Coef ?? 1; break;
      case 'TargetPoscns': tgt(); o.sources = b.SourceArray.map((x, k) => chk(x, b.SourceBoneNameArray[k], 'psrc' + i)); o.source_names = o.sources.map(bn); o.weights = b.WeightArray; o.offsets = (b.OffsetArray || []).map(v3); o.offset_translate = v3(b.OffsetTranslate); o.orient_affect = !!b.OrientAffect; o.scale_affect = !!b.ScaleAffect; break;
      case 'TargetDircns': tgt(); o.parent = bones[o.target].parent; o.aim_object = chk(b.AimObject, b.AimObjectBoneName, 'aim' + i); o.up_object = b.UpObject >= 0 ? chk(b.UpObject, b.UpObjectBoneName, 'up' + i) : -1;
        o.neutral_rotate = q4(b.NeutralRotate); o.aim = v3(b.AimVector); o.up = v3(b.UpVector); o.cross = v3(b.CrossVector); o.aim_offset = v3(b.AimSrcOffset); o.up_offset = v3(b.UpSrcOffset);
        // (party A2) the two flags 0x143AB97B0 reads at +0x6C / +0x6D (up-object path: offsets turned by the object)
        o.aim_transform_affect = !!b.AimTransformAffect; o.up_transform_affect = !!b.UpTransformAffect; break;
      // (party A) SourceOther (vtable 0x1474FE310): Compute 0x143AB3580 copies CS[source].Scale3D into its parameters, which
      // GetParamPtr 0x143AC1C80 exposes as ScaleX / ScaleY / ScaleZ (types 19, 20, 21) [C]
      case 'SourceOther': o.source = chk(b.SourceObject, b.SourceObjectBoneName, 'other' + i); o.source_name = bn(o.source); o.sources = [o.source]; o.source_names = [o.source_name]; break;
      default: throw new Error('KineDriver operator type not handled: ' + h.OpType);
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
function kineDriverJson(kdiPath, bones, settings) {
  const content = gamePathToContent(kdiPath);
  const d = loadPkg(content);
  const raw = kdProps(d.exports.find((e) => e.class === 'SQEX_KineDriverData').data.props);
  const built = kdBuild(raw, bones);
  return { format: 'sora_kinedriver/1', generator: 'tools/party_pack.js ' + VERSION, source: { asset: kdiPath, class: 'SQEX_KineDriverData' },
    schema: 'as build/sora_pack/data/kinedriver.json; bone indices into this pack\'s manifest.json skeleton.bones',
    settings: { EnableScaleOpChildSSC: !!settings.EnableScaleOpChildSSC, TickGroup: settings.TickGroup ?? null, from: 'the SQEX_KineDriver_AssetUserData of the mesh\'s KBD element (KineDriver_AssetUserDataElement[0])' }, ...built };
}

// ------------------------------------------------------------------------------------------------ 5. weapons
function weaponOf(equipPath) {
  const eqContent = gamePathToContent(equipPath);
  const eq = loadPkg(eqContent);
  const eqp = plainProps(eq, eq.exports.find((e) => e.class === 'TresEquipWeaponData').data.props);
  const unit = eqp.m_Equips[0];
  const bpPath = unit.RsrcData; const bpContent = gamePathToContent(bpPath.replace(/_C$/, ''));
  const bp = loadPkg(bpContent);
  const cls = bp.exports.find((e) => e.class === 'BlueprintGeneratedClass');
  const parentPath = importPath(bp, cls.superIndex), parent = loadPkg(gamePathToContent(parentPath));
  const pcls = parent.exports.find((e) => e.class === 'BlueprintGeneratedClass');
  const comp = (d, name) => { const e = d.exports.find((x) => x.objectName === name && !/^Default__/.test(x.objectName)); return e ? plainProps(d, e.data.props) : {}; };
  const cdo = (d) => plainProps(d, d.exports.find((e) => /^Default__/.test(e.objectName)).data.props);
  const mesh = { ...comp(parent, 'TresWeaponMesh0'), ...comp(bp, 'TresWeaponMesh0') };
  return { equip: `/Game/${eqContent}`, attach_part: unit.m_AttachPart, attach_socket: unit.AttachBoneName, apply_parent_scale: !!unit.m_bApplyParentScale, blueprint: `/Game/${bpContent}`, parent_blueprint: parentPath, native_class: importPath(parent, pcls.superIndex),
    mesh_path: mesh.SkeletalMesh, anim_class: mesh.AnimClass || null, cdo: { ...cdo(parent), ...cdo(bp) }, atk_coll: { ...comp(parent, 'TresAtkColl0'), ...comp(bp, 'TresAtkColl0') }, movement: { ...comp(parent, 'TresWeaponMovement0'), ...comp(bp, 'TresWeaponMovement0') }, mesh_component: mesh };
}
function physicsJson(physPath, bones) {
  if (!physPath) return null;
  const content = gamePathToContent(physPath); const d = loadPkg(content);
  const pa = d.exports.find((e) => e.class === 'PhysicsAsset'); const raw = propsOf(pa);
  const bodies = ((raw.SkeletalBodySetups || {}).value || []).map((ei) => { const e = d.exports[ei - 1]; const p = plainProps(d, e.data.props); const g = p.AggGeom || {}; return { export: e.objectName, bone: p.BoneName, boneIndex: bones.findIndex((b) => b.name === p.BoneName), physicsType: p.PhysicsType || '(default PhysType_Default)', aggGeom: { spheres: g.SphereElems || [], boxes: g.BoxElems || [], sphyls: g.SphylElems || [], convex: (g.ConvexElems || []).length }, raw: p }; });
  const constraints = ((raw.ConstraintSetup || {}).value || []).map((ei) => { const e = d.exports[ei - 1]; const p = plainProps(d, e.data.props); const di = p.DefaultInstance || {}; return { export: e.objectName, jointName: di.JointName, bone1: di.ConstraintBone1, bone2: di.ConstraintBone2, raw: p }; });
  return { _schema: `PhysicsAsset ${physPath}: bodies (SkeletalBodySetups order) and constraints, every tagged property under raw (tools/party_pack.js). Absent properties = class defaults (stock UE 4.17).`, source: physPath, physicsAsset: plainProps(d, pa.data.props), bodies, constraints };
}

// ------------------------------------------------------------------------------------------------ 6. attack sockets
// The weapon's ATK_00 / ATK_01 per animation frame in ACTOR space (UE cm), for every clip with a hit window or a Shot
// notify (as tools/sora_pack.js attackSockets for the keyblade): hand bone -> the character's weapon socket -> the weapon's
// root and wep bones at their reference pose -> the weapon socket. The root bone sits at its reference transform for a
// clip with bEnableRootMotion (the pose's root lock).
function attackSockets(anims, bones, id, handSocket, weapon) {
  const hand = bones.findIndex((b) => b.name === handSocket.bone);
  if (hand < 0) throw new Error('attack sockets: no bone ' + handSocket.bone);
  const chain = []; for (let k = hand; k >= 0; k = bones[k].parent) chain.unshift(k);
  const socket = { q: handSocket.quaternion_ue_xyzw, t: handSocket.relative_location_ue_cm, s: handSocket.relative_scale };
  const wb = weapon.bones; const wroot = wb[0];
  const wepIdx = wb.findIndex((b) => b.name === 'wep');
  const wbT = (b) => ({ q: b.ue.rotation, t: b.ue.translation, s: b.ue.scale });
  const wchain = []; for (let k = wepIdx; k >= 0; k = wb[k].parent) wchain.unshift(k);
  const sk = (name) => { const x = weapon.sockets.find((y) => y.name === name); return x && x.bone === 'wep' ? x.relative_location_ue_cm : null; };
  const a0 = sk('ATK_00'), a1 = sk('ATK_01');
  const yaw = id.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cy = Math.cos(yaw), sy = Math.sin(yaw);
  const actor = (v) => [v[0] * cy - v[1] * sy, v[0] * sy + v[1] * cy, v[2]];
  const out = { note: `ATK_00 and ATK_01 of the default weapon ${weapon.model} per animation frame (time = frame * SequenceLength / (NumFrames - 1)), ACTOR space, UE cm: [x0, y0, z0, x1, y1, z1]. Clips with a TresAnimNotifyState_Attack or a TresAnimNotify_Shot. Weapon at its reference pose on ${handSocket.name} (bone ${handSocket.bone}).`,
    weapon: weapon.model, attach_socket: handSocket.name, attach_bone: handSocket.bone, atk_00_on_wep_cm: a0, atk_01_on_wep_cm: a1, clips: {} };
  if (!a0 || !a1 || wepIdx < 0) { out.note += ' NOT COMPUTED: the weapon lacks ATK_00 / ATK_01 on bone wep.'; return out; }
  for (const a of anims.values()) {
    if (!a.notifies.some((n) => n.class === 'TresAnimNotifyState_Attack' || n.class === 'TresAnimNotify_Shot')) continue;
    const byBone = new Map(); for (const c of a.chans) { if (!byBone.has(c.bone)) byBone.set(c.bone, {}); byBone.get(c.bone)[c.path] = c; }
    const frames = [];
    for (let i = 0; i < a.numFrames; i++) {
      const time = a.numFrames > 1 ? i * a.length / (a.numFrames - 1) : 0;
      let T = null;
      for (const k of chain) {
        const b = bones[k], tr = (k === 0 && a.props.bEnableRootMotion === true) ? {} : (byBone.get(k) || {});
        const l = { q: tr.rotation ? sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale };
        T = T ? under(l, T) : l;
      }
      let W = under(socket, T);
      for (const k of wchain) W = under(wbT(wb[k]), W);
      frames.push([...actor(point(W, a0)), ...actor(point(W, a1))].map((x) => +x.toFixed(4)));
    }
    out.clips[path.basename(a.content)] = { frames: a.numFrames, duration_s: a.length, sockets: frames };
  }
  return out;
}

// ------------------------------------------------------------------------------------------------ 7. one member
async function packMember(m, inventory) {
  const dir = P(OUT, m.npc);
  const files = new Map(); // relative path in dir -> Buffer
  const id = identify(m);
  const meshGlb = readGlb(meshGlbFile(id.mesh_content));
  const bones = buildSkeleton(meshGlb.json);
  const skelBones = skeletonBones(gamePathToContent(id.skeleton));
  const meshOnlyInSkeleton = skelBones.filter((b) => !bones.some((x) => x.name === b.name)).map((b) => b.name);
  // the mesh's bones in the skeleton asset's order with the same parents (a subset of it)
  for (const b of bones) { const s = skelBones.find((x) => x.name === b.name); if (!s) throw new Error(`${m.npc}: mesh bone ${b.name} not in the skeleton asset`); }
  const sockets = socketsOf(gamePathToContent(id.skeleton), bones);
  const slotNames = (() => { const names = []; meshGlb.json.materials.forEach((mm, k) => { let n = mm.name && mm.name !== 'None' ? mm.name : `slot${k}`; if (names.includes(n)) n = `${n}_${k}`; names.push(n); }); return names; })();
  const model = buildModel(meshGlb, bones, slotNames, m.npc);
  files.set('model.glb', model.glb);
  console.log(`${m.name} (${m.npc}): ${bones.length} bones (skeleton asset ${skelBones.length}: ${meshOnlyInSkeleton.join(' ') || 'same'}), ${model.stats.vertices} vertices, ${model.stats.triangles} triangles, ${model.stats.skin_joints} skin joints, ${slotNames.length} material slots`);

  // ---- anim sets: body (MyAnimSets), face (m_SubAnimSets), camp (CM_*), gummi (GM_*)
  const dropped = { clips: 0, channels: 0 };
  // an AnimName serialized as a numbered FName { index, number } (Donald's "M_830") is resolved through the package's names
  const setOf = (gp) => {
    const s = loadAnimSet(gp);
    const names = loadPkg(gamePathToContent(gp)).names || [];
    for (const r of s.rows) if (r.name && typeof r.name === 'object') { const base = names[r.name.index]; const b = Array.isArray(base) ? base[0] : base; r.name = r.name.number > 0 ? `${b}_${r.name.number - 1}` : String(b); }
    return s;
  };
  const bodySets = id.anim_sets.map(setOf), faceSets = id.sub_anim_sets.map(setOf);
  const extraSet = (suffix) => { const c = `Character/npc/${m.npc}/anm/${m.npc}_${suffix}`; return pkgExists(c) ? setOf(`/Game/${c}`) : null; };
  const camp = { body: extraSet('CM_Body_AnimSet'), face: extraSet('CM_Face_AnimSet') }, gummi = { body: extraSet('GM_Body_AnimSet'), face: extraSet('GM_Face_AnimSet') };
  const seqs = new Map(); const need = (gp, why, kind) => { if (!seqs.has(gp)) seqs.set(gp, { used_by: [], kind }); seqs.get(gp).used_by.push(why); };
  const logical = []; const rowsMissing = [];
  const setRows = new Map(); for (const s of bodySets) for (const r of s.rows) if (!setRows.has(r.name)) setRows.set(r.name, { ...r, set: s.content });
  for (const row of setRows.values()) {
    if (!row.asset) { rowsMissing.push(row.name); continue; }
    const d = describeAnimAsset(row.asset);
    const e = { name: row.name, loop: null, play: { mode: null, loop_bit: null, root_motion_bit: null, blend_in_argument: null, play_rate: 1, root_translation_scale: [1, 1], played_by: 'the NPC state that plays the row gives the mode and blend-in' }, use: '', optional: true, stage1: false, anim_set: `/Game/${row.set}`, asset: row.asset, class: d.class,
      same_asset_as: [...setRows.values()].filter((r) => r.asset === row.asset && r.name !== row.name).map((r) => r.name) };
    if (d.class === 'BlendSpace') { Object.assign(e, { axis: d.axis, axis_y: d.axis_y, grid_samples: d.grid_samples, sample_order_in_asset: d.sample_order_in_asset, sample_weight_rule: d.sample_weight_rule, input_filter: d.input_filter, target_weight_interpolation_speed_per_sec: 0, m_InterpolationTime: d.m_InterpolationTime, notify_trigger_mode: d.notify_trigger_mode, samples: d.samples }); for (const s of d.samples) need(s.asset, `${row.name}@${s.value},${s.value_y}`, 'animation'); }
    else if (d.class === 'BlendSpace1D') { Object.assign(e, { axis: d.axis, grid_samples: d.grid_samples, sample_order_in_asset: d.sample_order_in_asset, sample_weight_rule: d.sample_weight_rule, input_filter: d.input_filter, target_weight_interpolation_speed_per_sec: 0, time_sync: d.time_sync, root_motion_rule: d.root_motion_rule, m_InterpolationTime: d.m_InterpolationTime, notify_trigger_mode: d.notify_trigger_mode, samples: d.samples }); for (const s of d.samples) need(s.asset, `${row.name}@${s.value}`, 'animation'); }
    else need(row.asset, row.name, 'animation');
    logical.push(e);
  }
  const extraRows = (set, label, kind) => (set ? set.rows.filter((r) => r.asset).map((r) => { const d = describeAnimAsset(r.asset); if (d.class !== 'AnimSequence') throw new Error(`${label} row ${r.name}: ${d.class} not handled`); need(r.asset, `${label}:${r.name}`, kind); return { name: r.name, anim_set: `/Game/${set.content}`, asset: r.asset }; }) : []);
  const campRows = { body: extraRows(camp.body, 'CM_Body', 'animation'), face: extraRows(camp.face, 'CM_Face', 'face-animation') };
  const gummiRows = { body: extraRows(gummi.body, 'GM_Body', 'animation'), face: extraRows(gummi.face, 'GM_Face', 'face-animation') };
  const faceRows = new Map(); for (const s of faceSets) for (const r of s.rows) if (!faceRows.has(r.name)) faceRows.set(r.name, { ...r, set: s.content });
  // a face row may name a 2D blend space (Donald's NO_0_Eye / BA_00_Eye: n_ex001_BS_id / _BS_Bid): its samples are packed;
  // the face slot plays the sample at the axes' origin (or the nearest): the inputs that drive its axes are not ported [?]
  const faceSpaces = {};
  for (const r of faceRows.values()) {
    if (!r.asset) continue;
    const d = describeAnimAsset(r.asset);
    if (d.class === 'AnimSequence') { need(r.asset, `face:${r.name}`, 'face-animation'); continue; }
    for (const s of d.samples) need(s.asset, `face:${r.name}@${s.value},${s.value_y ?? ''}`, 'face-animation');
    const at0 = [...d.samples].sort((a, b) => Math.hypot(a.value, a.value_y || 0) - Math.hypot(b.value, b.value_y || 0))[0];
    faceSpaces[r.name] = { class: d.class, asset: r.asset, axis: d.axis, axis_y: d.axis_y || null, samples: d.samples, played_sample: at0.asset, note: 'the face slot plays the sample nearest the axes\' origin; what drives the axes in the game (eye look-at) is not ported [?]' };
    r.played = at0.asset;
  }
  // a face sequence whose asset name is also a body sequence's (0811..0814 localisation, 0001CM / 0002CM): <name>_face
  const bodyNames = new Set([...seqs].filter(([, v]) => v.kind === 'animation').map(([gp]) => path.basename(gamePathToContent(gp))));
  const nameOf = (gp) => { const b = path.basename(gamePathToContent(gp)); return seqs.get(gp).kind === 'face-animation' && bodyNames.has(b) ? b + '_face' : b; };
  const fileOf = (gp) => (seqs.get(gp).kind === 'face-animation' ? 'face/' : 'anims/') + nameOf(gp) + '.glb';
  { const names = [...seqs.keys()].map(fileOf); const dup = names.filter((x, i) => names.indexOf(x) !== i); if (dup.length) throw new Error(`${m.npc}: two sequences with one file name: ${dup.slice(0, 5).join(', ')}`); }
  const anims = new Map();
  for (const gp of seqs.keys()) {
    const a = loadClip(gp, bones, skelBones, dropped);
    anims.set(gp, a);
    files.set(fileOf(gp), glbOfClip(bones, nameOf(gp), a,`(party ${m.npc} clip)`));
  }
  const bodyAnims = new Map([...anims].filter(([gp]) => seqs.get(gp).kind === 'animation'));
  console.log(`  ${setRows.size} body rows (${logical.filter((l) => l.samples).length} blend spaces), ${faceRows.size} face rows, camp ${campRows.body.length}+${campRows.face.length}, gummi ${gummiRows.body.length}+${gummiRows.face.length}; ${anims.size} sequences; ${dropped.clips} clips had tracks of ${meshOnlyInSkeleton.length} skeleton-only bones (${dropped.channels} channels dropped)`);
  for (const l of logical) {
    const seqsOf = l.samples ? l.samples.map((x) => x.asset) : [l.asset];
    const it = l.samples ? (l.m_InterpolationTime ?? 0) : (anims.get(l.asset).props.m_InterpolationTime ?? 0);
    l.play.blend_in_s = null; l.play.asset_m_InterpolationTime = it;
    l.play.sequences_with_bEnableRootMotion = seqsOf.filter((g) => anims.get(g).props.bEnableRootMotion === true).map((g) => path.basename(gamePathToContent(g)));
    l.play.root_motion_extracted = null;
  }

  // ---- face runtime (the Animator's face slots): subtrees, rows, type -> row
  const subtree = (root) => bones.map((b, i) => i).filter((i) => { for (let k = i; k >= 0; k = bones[k].parent) if (bones[k].name === root) return true; return false; });
  const faceTypes = readJson(P(ROOT, 'tools', 'lib', 'face_type_rows.json')).rows;
  const faceLogical = [...faceRows.values()].filter((r) => r.asset).map((r) => ({ name: r.name, anim_set: `/Game/${r.set}`, asset: r.asset, file: fileOf(r.played || r.asset), ...(faceSpaces[r.name] ? { blend_space: faceSpaces[r.name] } : {}) }));
  const faceTypeToRow = Object.fromEntries(Object.entries(faceTypes).filter(([, v]) => v.names.length).map(([k, v]) => [k, v.names[v.names.length - 1]]));
  const faceTypeToRowBattle = Object.fromEntries(Object.entries(faceTypes).filter(([, v]) => v.names.length === 2).map(([k, v]) => [k, v.names[0]]));
  const rowLower = new Map([...faceRows.keys()].map((k) => [k.toLowerCase(), k]));
  const faceUnresolved = {};
  for (const a of bodyAnims.values()) for (const x of a.notifies) {
    const mm = /^TresAnimNotify_FaceAnim(Eye|Lip)?$/.exec(x.class || ''); if (!mm) continue;
    const pr = x.properties || {}; const named = pr.FaceAnimName ?? pr.m_AnimName; const type = pr.FaceAnimType ?? pr.m_AnimType ?? 'NORMAL_AUTO';
    const bases = named && named !== 'None' ? [named] : ((faceTypes[type] || {}).names || []);
    if (!bases.length) { faceUnresolved['type ' + type] = (faceUnresolved['type ' + type] || 0) + 1; continue; }
    const slots = mm[1] === 'Eye' ? ['_EYE'] : mm[1] === 'Lip' ? ['_LIP'] : ['_EYE', '_LIP'];
    for (const b of bases) for (const last of slots) if (![b, b + '_EL', b + last].some((c) => rowLower.has(c.toLowerCase()))) faceUnresolved[b + last] = (faceUnresolved[b + last] || 0) + 1;
  }

  // ---- KineDriver and Bonamik: the mesh's KBD elements
  const kbd = kbdElements(id.mesh_content);
  const dataFiles = [];
  const kdOf = (el) => (kbd[el] || []).find((x) => x.class === 'SQEX_KineDriver_AssetUserData');
  const bnmOf = (el) => (kbd[el] || []).find((x) => x.class === 'SQEX_BonamikAssetUserData');
  const kdJson = {}; const bnmJson = {};
  const soraBonamik = readJson(P(sp.OUT, 'data', 'bonamik.json'));
  for (const [el, suffix] of [['default', ''], ['cs', '_cs'], ['shortEar', '_shortEar']]) {
    const kd = kdOf(el), bn = bnmOf(el);
    if (kd && kd.kine_driver && kd.kine_driver[0] && !(suffix && kdOf('default') && kdOf('default').kine_driver[0] === kd.kine_driver[0])) {
      const j = kineDriverJson(kd.kine_driver[0], bones, kd.settings); j.kbd_element = el;
      kdJson[el] = j; const f = `data/kinedriver${suffix}.json`; files.set(f, Buffer.from(JSON.stringify(j, null, 1) + NL)); dataFiles.push({ file: f, what: `KineDriver of the KBD element "${el}": ${kd.kine_driver[0]} (${j.operators.length} operators, ${j.bones.filter((b) => b.written).length} bones written; EnableScaleOpChildSSC ${j.settings.EnableScaleOpChildSSC})` });
    }
    if (bn && bn.bonamik && bn.bonamik[0] && !(suffix && bnmOf('default') && bnmOf('default').bonamik[0] === bn.bonamik[0])) {
      const content = gamePathToContent(bn.bonamik[0]);
      const x = bonamikExport(content, id.mesh_content, bones, soraBonamik);
      // the element's own user data (TeleportDistance 800 for both friends; the asset part is bonamikExport's)
      const def = bnmOf('default') ? bnmOf('default').settings : {};
      const tele = bn.settings.TeleportDistance ?? def.TeleportDistance;
      x.user_data.TeleportDistance = tele ?? 0; x.user_data.TeleportDistance_evidence = tele !== undefined ? `serialized in the SQEX_BonamikAssetUserData of the KBD element "${el}" of ${id.mesh_content} (or the mesh's default one)` : x.user_data.TeleportDistance_evidence;
      x.user_data.PreRollStepOnResume = bn.settings.PreRollStepOnResume ?? def.PreRollStepOnResume ?? null; x.user_data.OutOfCamera = bn.settings.OutOfCamera ?? def.OutOfCamera ?? null;
      x._schema.about = `${m.name} (${m.npc}) Bonamik data, KBD element "${el}". Source: Content/${content}. Same schema as build/sora_pack/data/bonamik.json (asset part); written by tools/party_pack.js through tools/sora_weapon_pack.js bonamikExport.`;
      x.kbd_element = el; bnmJson[el] = x;
      const f = `data/bonamik${suffix}.json`; files.set(f, Buffer.from(JSON.stringify(x, null, 1) + NL)); dataFiles.push({ file: f, what: `Bonamik of the KBD element "${el}": ${bn.bonamik[0]} (${x.bodies.length} bodies, ${x.bones.filter((b) => b.bonamik !== 'none').length} driven bones)` });
    }
  }
  { const ph = physicsJson(id.physics_asset, bones); if (ph) { files.set('data/physics_asset.json', Buffer.from(JSON.stringify(ph, null, 1) + NL)); dataFiles.push({ file: 'data/physics_asset.json', what: `PhysicsAsset ${id.physics_asset}: ${ph.bodies.length} bodies, ${ph.constraints.length} constraints` }); } }
  // (party A2) leg IK: the AnimBlueprint's node data (kh-anim: legik.rs Multiped with the stock solver for Goofy, legik_coll.rs for Donald)
  if (id.leg_ik_data) { files.set('data/legik.json', Buffer.from(JSON.stringify(id.leg_ik_data, null, 1) + NL)); dataFiles.push({ file: 'data/legik.json', what: `leg IK of ${id.anim_class}: ${id.leg_ik_data.coll_two_bone_ik.length} CollTwoBoneIK, ${id.leg_ik_data.two_bone_ik_multiped.length} TwoBoneIKMultiped (${id.leg_ik_data.schema.solver})` }); }

  // ---- weapons (14 per character, TresEquipItemData rows of inventory.json)
  const weaponRows = inventory[m.key].weapons;
  const weapons = [];
  for (const row of weaponRows) {
    const wv = weaponOf(row.equip);
    const wContent = gamePathToContent(wv.mesh_path); const model = path.basename(wContent).replace(/_0$/, '');
    const wGlb = readGlb(meshGlbFile(wContent)); const wBones = buildSkeleton(wGlb.json);
    const wSkel = loadPkg(wContent).imports.map((im, i) => ({ im, i })).filter((x) => x.im.className === 'Skeleton').map((x) => importPath(loadPkg(wContent), -(x.i + 1)))[0];
    const wSockets = wSkel ? socketsOf(gamePathToContent(wSkel), wBones) : [];
    const wSlots = wGlb.json.materials.map((mm, k) => (mm.name && mm.name !== 'None' ? mm.name : `${model}_slot${k}`));
    const wModel = buildModel(wGlb, wBones, wSlots, model);
    const wDir = P(OUT, 'weapons', model); fs.mkdirSync(P(wDir, 'data'), { recursive: true });
    fs.writeFileSync(P(wDir, 'weapon.glb'), wModel.glb);
    const wPkg = loadPkg(wContent); const wImp = (cn) => wPkg.imports.map((im, i) => ({ im, i })).filter((x) => x.im.className === cn).map((x) => importPath(wPkg, -(x.i + 1)));
    const wData = [];
    const bnmPath = wImp('SQEX_BonamikAsset')[0];
    if (bnmPath) { const x = bonamikExport(gamePathToContent(bnmPath), wContent, wBones, soraBonamik); fs.writeFileSync(P(wDir, 'data/bonamik.json'), JSON.stringify(x, null, 1) + NL); wData.push('data/bonamik.json'); }
    const ph = physicsJson(wImp('PhysicsAsset')[0], wBones); if (ph) { fs.writeFileSync(P(wDir, 'data/physics_asset.json'), JSON.stringify(ph, null, 1) + NL); wData.push('data/physics_asset.json'); }
    // the weapon's own anim set (Goofy's shields: w_gu010_Anim), every row
    const wAnims = [];
    if (wv.anim_class) {
      const ab = loadPkg(gamePathToContent(wv.anim_class.replace(/_C$/, '')));
      const c = plainProps(ab, ab.exports.find((e) => /^Default__/.test(e.objectName)).data.props);
      fs.mkdirSync(P(wDir, 'anims'), { recursive: true });
      for (const setPath of c.MyAnimSets || []) { if (!setPath) continue; const set = loadAnimSet(setPath); for (const r of set.rows) { if (!r.asset) continue; const d = describeAnimAsset(r.asset); if (d.class !== 'AnimSequence') { wAnims.push({ name: r.name, asset: r.asset, class: d.class, packed: false }); continue; }
        const a = loadAnimation(r.asset, wBones); const nm = path.basename(a.content); fs.writeFileSync(P(wDir, 'anims', nm + '.glb'), glbOfClip(wBones, nm, a, `(party weapon ${model} clip)`)); wAnims.push({ name: r.name, anim_set: setPath, asset: r.asset, file: `anims/${nm}.glb`, duration_s: a.length, num_frames: a.numFrames, notifies: a.notifies }); } }
    }
    const item = String(row.key).replace(/_/g, '').replace(/^WEP(DONALD|GOOFY)/, 'WEP_$1'); // WEP_DONALD_00 -> WEP_DONALD00
    const wMan = { format: 'kh3-party-weapon/1', generator: 'tools/party_pack.js ' + VERSION, character: `${m.name} (${m.npc})`, model, item_row: row.row, item_key: row.key, item, name_ja: row.name_ja ?? null,
      stats: { ap: row.ap, str: row.str, mag: row.mag, def: row.def, abilities: row.abilities },
      equip: wv.equip, attach: { part: wv.attach_part, socket: wv.attach_socket, socket_on_character: sockets.find((s) => s.name === wv.attach_socket) || null, apply_parent_scale: wv.apply_parent_scale, note: 'the weapon actor\'s root sits on the character\'s socket (bone + socket transform); its mesh component at the actor root (RelativeLocation / Rotation of TresWeaponMesh0 not serialized: identity)' },
      blueprint: wv.blueprint, parent_blueprint: wv.parent_blueprint, native_class: wv.native_class, mesh: wv.mesh_path, anim_class: wv.anim_class,
      skeleton: { source_asset: wSkel || null, bone_count: wBones.length, bones: wBones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, translation: b.translation.map(f32), rotation: b.rotation.map(f32), scale: b.scale.map(f32), translation_ue: b.ue.translation, rotation_ue: b.ue.rotation, scale_ue: b.ue.scale })) },
      sockets: wSockets, materials: wSlots.map((n, k) => ({ slot: k, slot_name: n, material_instance: wGlb.json.materials[k].extras && wGlb.json.materials[k].extras.ue_material })), model_stats: wModel.stats, data_files: wData, animations: wAnims,
      blueprint_data: { cdo: wv.cdo, atk_coll: wv.atk_coll, movement: wv.movement, mesh_component: wv.mesh_component } };
    fs.writeFileSync(P(wDir, 'manifest.json'), JSON.stringify(wMan, null, 1) + NL);
    weapons.push({ item, item_key: row.key, row: row.row, model, dir: `weapons/${model}`, socket: wv.attach_socket, slots: wMan.materials, glb: wModel.glb, bones: wBones, sockets: wSockets, uv2: uv2Of({ json: readGlb(P(wDir, 'weapon.glb')).json, bin: readGlb(P(wDir, 'weapon.glb')).bin }, model) });
  }
  console.log(`  weapons: ${weapons.map((w) => w.model).join(' ')} (sockets ${[...new Set(weapons.map((w) => w.socket))].join(', ')})`);

  // ---- root tracks, feet, attack sockets
  const rootMotion = buildRootMotion(bodyAnims, bones, logical, id, new Set());
  const idleRow = setRows.get('IdleN');
  const feet = footHeight(meshGlb, bones, anims.get(idleRow.asset));
  const defaultWeapon = weapons[0];
  const handSocket = sockets.find((s) => s.name === defaultWeapon.socket);
  const atkSockets = attackSockets(bodyAnims, bones, id, handSocket, { model: defaultWeapon.model, bones: defaultWeapon.bones, sockets: defaultWeapon.sockets });

  // ---- census: notify classes, KDI operators, Bonamik notifies
  const notifyClasses = {};
  for (const [gp, a] of anims) for (const n of a.notifies) { if (!n.class) continue; const k = n.class; notifyClasses[k] = notifyClasses[k] || { count: 0, state: false, instant: false, sets: new Set() }; notifyClasses[k].count++; notifyClasses[k][n.kind === 'state' ? 'state' : 'instant'] = true; notifyClasses[k].sets.add(seqs.get(gp).kind); }
  const bonamikNotifyClips = {};
  for (const [, a] of anims) { const b = a.notifies.filter((n) => /Bonamik/.test(n.class || '')); if (b.length) bonamikNotifyClips[path.basename(a.content)] = b.map((n) => ({ class: n.class, start: n.trigger_time, end: n.end_trigger_time ?? n.trigger_time, properties: n.properties })); }
  const census = {
    notify_classes: Object.fromEntries(Object.entries(notifyClasses).sort().map(([k, v]) => [k, { count: v.count, state: v.state, instant: v.instant, in: [...v.sets] }])),
    kine_driver_operator_kinds: Object.fromEntries(Object.entries(kdJson).map(([el, j]) => [el, j.operator_counts])),
    kine_driver_settings: Object.fromEntries(Object.entries(kdJson).map(([el, j]) => [el, j.settings])),
    bonamik_assets: Object.fromEntries(Object.entries(bnmJson).map(([el, x]) => [el, { source: x.source.package, bodies: x.bodies.length, links: x.links.length, planes: x.planes.length, groups: x.groups.map((g) => g.name), counts: x.runtime.counts }])),
    bonamik_notify_clips: bonamikNotifyClips,
    leg_ik_nodes: id.leg_ik_nodes,
    face_unresolved: faceUnresolved,
    anim_rows_without_asset: rowsMissing,
    skeleton_only_bones: meshOnlyInSkeleton, dropped_tracks: dropped,
  };

  // ---- manifest
  const item = (file, kind, content, more) => ({ file, kind, bytes: files.get(file).length, sha256: sha256(files.get(file)), source_asset: content ? `/Game/${content}` : null, ...more });
  const items = [item('model.glb', 'mesh', id.mesh_content, { lod: 0, ...model.stats, skeleton_bones: bones.length })];
  for (const [gp, a] of anims) {
    const s = seqs.get(gp); const f = fileOf(gp);
    items.push(item(f, s.kind, a.content, { animation_name: nameOf(gp), logical_names: s.used_by, duration_s: a.length, num_frames: a.numFrames, frame_rate: a.fps, loop: false, root_motion_enabled: a.props.bEnableRootMotion === true, tracks: a.tracks,
      interpolation: a.chans.length ? a.chans[0].interpolation : 'LINEAR', sequence_properties: a.props, key_counts: a.keyCounts, root_bone: a.root.kh_sk, root_track_moves: a.root_track_moves, notifies: a.notifies }));
  }
  for (const d of dataFiles) items.push({ file: d.file, kind: 'data', bytes: files.get(d.file).length, sha256: sha256(files.get(d.file)), content: d.what });
  const ikWindows = {}; for (const a of bodyAnims.values()) { const w = a.notifies.filter((x) => x.class === 'TresAnimNotifyState_IK'); if (w.length) ikWindows[path.basename(a.content)] = w.map((x) => ({ start: x.trigger_time, end: x.end_trigger_time, target_alpha: x.properties.m_TargetAlpha ?? 1 })); }
  const yaw = id.mesh_component.relative_rotation_ue_deg.yaw;
  const manifest = {
    format: 'kh3-party-pack/1', generator: `tools/party_pack.js ${VERSION}`, spec: 'docs/party-port-plan.md 2.1, 2.2; schema = the Sora pack\'s (tools/sora_pack.js)', legal: 'Converted from the user\'s own copy of the game for local use; nothing in this folder is distributed.',
    character: `${m.name} (${m.npc}, ${m.unique} = ${m.unique_id})`, member: m.member, npc: m.npc, identification: id, conventions: sp.CONVENTIONS,
    placement: { note: 'As the Sora pack: the actor origin on the ground, the mesh component turned by its relative yaw (pack +Z = actor forward).', mesh_component_relative_location_ue_cm: id.mesh_component.relative_location_ue_cm, mesh_component_relative_rotation_ue_deg: id.mesh_component.relative_rotation_ue_deg,
      pack_model_forward: (((yaw % 360) + 360) % 360) === 270 ? '+Z' : `UNEXPECTED mesh yaw ${yaw}`, pack_model_up: '+Y', pack_model_left: '+X', capsule_size_ue_cm: id.collision_capsule.size_ue_cm, capsule_relative_location_ue_cm: id.collision_capsule.relative_location_ue_cm },
    skeleton: { source_asset: id.skeleton, bone_count: bones.length, skeleton_asset_bone_count: skelBones.length, skeleton_only_bones: meshOnlyInSkeleton, root: bones[0].name, node_index_equals_bone_index: true,
      bones: bones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, translation: b.translation.map(f32), rotation: b.rotation.map(f32), scale: b.scale.map(f32), translation_ue: b.ue.translation, rotation_ue: b.ue.rotation, scale_ue: b.ue.scale })) },
    sockets,
    // body slots, then every weapon's slots (tools/sora_material_pack.js outfit mode translates them all into materials/)
    materials: [...slotNames.map((n, k) => ({ mesh: m.npc, slot: k, slot_name: n, material_instance: meshGlb.json.materials[k].extras && meshGlb.json.materials[k].extras.ue_material })), ...weapons.flatMap((w) => w.slots.map((s) => ({ mesh: w.model, ...s })))],
    mesh_content: id.mesh_content, wearform_set: null,
    animation_set: { source_assets: id.anim_sets, rows_total: setRows.size, structure: 'TresAnimSet: Anims = array of { AnimName, AnimData }; the NPC states play by AnimName' },
    skin: { files: ['model.glb'], joint_count: model.joints.length, joints: model.joints.map((b, k) => ({ joint: k, bone: b, name: bones[b].name })) },
    root_motion: rootMotion, feet, attack_sockets: atkSockets,
    runtime: {
      note: 'What the game runs on top of the clip poses (the Sora pack\'s runtime block, with this character\'s data). The NPC anim instance (TresNpcAnimInstance) runs the same slot / saved-pose graph (its AnimBlueprint: identification.anim_graph_nodes).',
      saved_pose: { face_subtrees: { G_eye: subtree('G_eye'), G_mouth: subtree('G_mouth') }, rule: 'as the Sora pack (docs/sora-animation-spec.md 6)' },
      leg_ik: { nodes: id.leg_ik_nodes, windows_by_clip: ikWindows, data: id.leg_ik_data ? 'data/legik.json' : null, state: id.leg_ik_nodes.TwoBoneIKMultiped ? 'TresAnimNode_TwoBoneIKMultiped (node-level bUseMayaSolver false: the stock per-leg solver 0x14054FC10), kh-anim legik.rs; alpha = m_LegIKAlpha from the TresAnimNotifyState_IK windows (NativeUpdateAnimation 0x14058B450, called first by UTresAIAnimInstance 0x14055F5E0)' : 'two TresAnimNode_CollTwoBoneIK (R_ashi1 then L_ashi1, BOX sweeps), kh-anim legik_coll.rs; same alpha' },
      face: { slots: { Face: 'G_eye subtree', Mouth: 'G_mouth subtree' }, rows: faceLogical, type_to_row: faceTypeToRow, type_to_row_battle: faceTypeToRowBattle, type_source: 'tools/lib/face_type_rows.json (the switch of 0x14056D3E0, decompiled C)', unresolved: faceUnresolved },
      kine_driver: kdJson.default ? { data: 'data/kinedriver.json', operators: kdJson.default.operators.length, bones_written: kdJson.default.bones.filter((b) => b.written).length, settings: kdJson.default.settings, variants: Object.keys(kdJson).filter((k) => k !== 'default') } : null,
      bonamik: bnmJson.default ? { data: ['data/bonamik.json'], driven_bones: bnmJson.default.bones.filter((b) => b.bonamik !== 'none').map((b) => ({ bone: b.index, name: b.name, role: b.bonamik })), user_data: bnmJson.default.user_data, variants: Object.keys(bnmJson).filter((k) => k !== 'default'), clip_notifies: bonamikNotifyClips } : null,
      kbd_elements: Object.fromEntries(Object.entries(kbd).map(([k, v]) => [k, v.map((x) => ({ class: x.class, export: x.export, asset: (x.kine_driver || x.bonamik || [null])[0] }))])),
    },
    face_animations: faceLogical,
    camp_animations: Object.fromEntries(Object.entries(campRows).map(([k, v]) => [k, v.map((r) => ({ ...r, sequence: nameOf(r.asset), file: fileOf(r.asset) }))])),
    gummi_animations: Object.fromEntries(Object.entries(gummiRows).map(([k, v]) => [k, v.map((r) => ({ ...r, sequence: nameOf(r.asset), file: fileOf(r.asset) }))])),
    data_files: dataFiles,
    logical_animations: logical.map((l) => ({ ...l, file: l.class === 'AnimSequence' ? fileOf(l.asset) : undefined, samples: l.samples ? l.samples.map((s) => ({ ...s, file: fileOf(s.asset), animation_name: path.basename(gamePathToContent(s.asset)) })) : undefined })),
    weapons: weapons.map((w) => ({ item: w.item, item_key: w.item_key, row: w.row, model: w.model, dir: w.dir, socket: w.socket })), default_weapon: defaultWeapon.model,
    census, items,
  };
  files.set('manifest.json', Buffer.from(JSON.stringify(manifest, null, 1) + NL));

  // ---- write (stale clip files are removed)
  for (const [f, b] of files) { const o = P(dir, f); fs.mkdirSync(path.dirname(o), { recursive: true }); fs.writeFileSync(o, b); }
  for (const sub of ['anims', 'face', 'data']) { if (!fs.existsSync(P(dir, sub))) continue; for (const f of fs.readdirSync(P(dir, sub))) { const r = sub + '/' + f; if (!files.has(r)) fs.unlinkSync(P(dir, r)); } }

  // ---- materials: the game's shaders of the body and the 14 weapons (tools/sora_material_pack.js, outfit mode)
  let materialsReport = 'not built (--no-materials)';
  if (!NO_MATERIALS) materialsReport = buildMaterials(m, dir, weapons, slotNames);

  // ---- validation
  const validation = { generator: `tools/party_pack.js ${VERSION}`, files: {}, failed: 0 };
  const check = async (f, abs, full) => {
    const c = checkGlb(abs, f.endsWith('weapon.glb') ? null : bones); delete c.weighted_bone_names;
    const v = full && !NO_VALIDATE ? await validateGlb(abs) : null;
    validation.files[f] = { independent: c, validator: v };
    if (c.bone_names_match === false || c.joints_out_of_range || c.index_out_of_range || c.non_finite || c.time_errors || c.non_unit_quaternions || (c.weight_sum_max_dev !== undefined && c.weight_sum_max_dev > 1e-5) || (c.bind_pose_skinning_max_dev_m !== undefined && c.bind_pose_skinning_max_dev_m > 1e-4) || (v && v.errors)) validation.failed++;
  };
  await check('model.glb', P(dir, 'model.glb'), true);
  for (const f of files.keys()) if (/^(anims|face)\//.test(f)) await check(f, P(dir, f), false);
  for (const w of weapons) await check(`../weapons/${w.model}/weapon.glb`, P(OUT, w.dir, 'weapon.glb'), true);
  validation.rows = { body_rows: setRows.size, body_rows_resolved: logical.length, face_rows: faceRows.size, face_rows_resolved: faceLogical.length, rows_without_asset: rowsMissing };
  validation.materials = materialsReport;
  fs.writeFileSync(P(dir, 'validation.json'), JSON.stringify(validation, null, 1) + NL);
  const mv = validation.files['model.glb'];
  console.log(`  model.glb: validator ${mv.validator ? mv.validator.errors + 'E ' + mv.validator.warnings + 'W' : 'skipped'}, bind-pose skinning dev ${mv.independent.bind_pose_skinning_max_dev_m} m, weight dev ${mv.independent.weight_sum_max_dev}; ${Object.keys(validation.files).length} glb read back, ${validation.failed} failed`);
  console.log(`  rows: body ${logical.length}/${setRows.size}, face ${faceLogical.length}/${faceRows.size}; KD ${Object.entries(kdJson).map(([k, j]) => `${k} ${j.operators.length} ops ${JSON.stringify(j.operator_counts)} SSC ${j.settings.EnableScaleOpChildSSC}`).join('; ')}; Bonamik ${Object.entries(bnmJson).map(([k, x]) => `${k} ${x.bodies.length} bodies`).join('; ')}`);
  console.log(`  notify classes: ${Object.keys(census.notify_classes).length}; Bonamik notify clips: ${Object.keys(bonamikNotifyClips).length}; face unresolved: ${JSON.stringify(faceUnresolved)}`);
  return { m, dir, manifest, files, weapons, validation, defaultWeapon };
}

// ------------------------------------------------------------------------------------------------ 8. materials
// The outfit mode of tools/sora_material_pack.js writes materials/ for the slots of manifest.materials (every slot as mesh
// "sora") and appends the Kingdom Key's. Afterwards: slot -> its own mesh (n_ex00x or w_xx###), the Kingdom Key's records
// dropped, owner ids renumbered (Donald 100.., Goofy 200..: one shader source holds Sora's and both friends' owners), the
// post-process function removed from the WGSL (Sora's library has it), the weapons' third UV sets written.
function buildMaterials(m, dir, weapons, bodySlots) {
  const r = cp.spawnSync(process.execPath, [path.join(__dirname, 'sora_material_pack.js')], { env: { ...process.env, KH_OUTFIT_PACK: dir }, encoding: 'utf8', maxBuffer: 1 << 28 });
  const lines = (r.stdout || '').split(NL).filter((l) => /NOT TRANSLATED|NOT DRAWN|stand-in|renumbered|missing|BAD|rror|outfit/.test(l));
  if (r.status !== 0) throw new Error('sora_material_pack.js failed for ' + m.npc + ': ' + r.stderr + (r.stdout || '').slice(-2000));
  const jf = P(dir, 'materials', 'materials.json'), wf = P(dir, 'materials', 'materials.wgsl');
  const json = readJson(jf); let wgsl = fs.readFileSync(wf, 'utf8');
  const meshOfSlot = new Map([...bodySlots.map((n) => [n, m.npc]), ...weapons.flatMap((w) => w.slots.map((s) => [s.slot_name, w.model]))]);
  // drop the Kingdom Key's slots and the owners only they use
  for (const [k, v] of Object.entries(json.materials)) { if (v.mesh === 'kingdom_key') delete json.materials[k]; else v.mesh = meshOfSlot.get(k) || v.mesh; }
  const usedOwners = new Set(Object.values(json.materials).map((v) => v.owner));
  const idMap = new Map();
  for (const [name, o] of Object.entries(json.owners)) { if (!usedOwners.has(name)) { delete json.owners[name]; continue; } idMap.set(o.id, m.idBase + o.id); o.id = m.idBase + o.id; }
  for (const v of Object.values(json.materials)) v.id = json.owners[v.owner] ? json.owners[v.owner].id : v.id;
  // WGSL: keep the #ifdef KH_C<id> blocks of the kept owners, renumbered; nothing else (kh_khs is Sora's library's)
  const blocks = [...wgsl.matchAll(/#ifdef KH_C(\d+)\n([\s\S]*?)\n#endif\n/g)];
  const kept = blocks.filter((b) => idMap.has(+b[1])).map((b) => `#ifdef KH_C${idMap.get(+b[1])}\n${b[2]}\n#endif\n`);
  wgsl = `// GENERATED by tools/party_pack.js from tools/sora_material_pack.js (outfit mode) for ${m.name} (${m.npc}) and his weapons: owner ids ${m.idBase}+. Do not edit.\n\n` + kept.join('\n');
  fs.writeFileSync(wf, wgsl);
  // uv2: the body's key "sora:<slot>" -> "<npc>:<slot>"; the weapons' own
  const uv2 = {};
  for (const [k, v] of Object.entries(json.uv2 || {})) { const [mesh, slot] = k.split(':'); if (mesh === 'kingdom_key') { const f = P(dir, 'materials', v.file); if (fs.existsSync(f)) fs.unlinkSync(f); continue; } uv2[`${meshOfSlot.get(slot) || m.npc}:${slot}`] = v; }
  for (const w of weapons) for (const u of w.uv2) { fs.writeFileSync(P(dir, 'materials', u.file), u.buf); uv2[u.key] = { file: u.file, vertices: u.vertices, primitive: u.primitive }; }
  json.uv2 = uv2;
  // the mesh facts: the body's under its own name (bounds, bone -> material parameter table)
  if (json.mesh_facts) { const f = json.mesh_facts; json.mesh_facts = {}; if (f.sora) json.mesh_facts[m.npc] = f.sora; }
  json.khs = null; json.wear_forms = null;
  json.party = { member: m.member, npc: m.npc, id_base: m.idBase, generator: 'tools/party_pack.js ' + VERSION, note: 'merged into the app\'s material library by kh-app char_material.rs (party A): owners, materials (by slot name), uv2, mesh_facts' };
  fs.writeFileSync(jf, JSON.stringify(json));
  const owners = Object.values(json.owners); const ok = owners.filter((o) => o.base && o.base.ok).length;
  const over = owners.filter((o) => o.base && o.base.ok && (o.base.textures || []).length > 14).map((o) => o.asset.split('.').pop());
  const report = `${Object.keys(json.materials).length} material slots (${Object.values(json.materials).filter((v) => v.stand_in_for).length} stand-ins), ${owners.length} shader owners, ${ok} translated; owners over 14 textures: ${over.join(', ') || 'none'}`;
  console.log('  materials: ' + report); for (const l of lines) console.log('   ' + l);
  return report;
}

// ------------------------------------------------------------------------------------------------ main
async function main() {
  const inventory = readJson(P(ROOT, 'build/sora_tmp/party/inventory.json'));
  fs.mkdirSync(OUT, { recursive: true });
  const done = [];
  for (const m of MEMBERS) { if (only && !only.includes(m.npc)) continue; done.push(await packMember(m, inventory)); }
  // ---- index
  const indexFile = P(OUT, 'party.json');
  const old = fs.existsSync(indexFile) ? readJson(indexFile) : { members: [] };
  const members = MEMBERS.map((m) => { const d = done.find((x) => x.m === m); if (!d) return old.members.find((x) => x.member === m.member) || null;
    return { member: m.member, npc: m.npc, name: m.name, unique: m.unique, unique_id: m.unique_id, dir: m.npc, default_weapon: d.defaultWeapon.model, material_id_base: m.idBase, weapons: d.weapons.map((w) => ({ item: w.item, item_key: w.item_key, row: w.row, model: w.model, dir: w.dir, socket: w.socket })) }; }).filter(Boolean);
  fs.writeFileSync(indexFile, JSON.stringify({ format: 'kh3-party-index/1', generator: 'tools/party_pack.js ' + VERSION, note: 'Donald and Goofy packs (docs/party-contracts.md A.1). member = the party slot (1 Donald, 2 Goofy).', members }, null, 1) + NL);
  // ---- sheet rows (sheets/port/party-assets.json): one per packed file of the members built now
  const sheetFile = P(ROOT, 'sheets/port/party-assets.json');
  const cols = ['id', 'kind', 'logical', 'source', 'source_sha256', 'bytes', 'sha256', 'duration', 'frames', 'loop', 'root_motion', 'validation', 'evidence'];
  const prev = fs.existsSync(sheetFile) ? readJson(sheetFile).rows : [];
  const rows = prev.filter((r) => !done.some((d) => String(r[0]).startsWith(d.m.npc + '/') || (String(r[0]).startsWith('weapons/') && d.weapons.some((w) => String(r[0]).startsWith(w.dir + '/')))) && r[0] !== 'party.json');
  const srcSha = new Map(); const srcOf = (content) => { if (!content) return '-'; if (!srcSha.has(content)) { try { const s = sp.sourceFiles(content); srcSha.set(content, s.uexp || s.uasset); } catch (e) { srcSha.set(content, '-'); } } return srcSha.get(content); };
  for (const d of done) {
    for (const it of d.manifest.items) {
      const content = it.source_asset ? it.source_asset.replace(/^\/Game\//, '') : null;
      const v = d.validation.files[it.file];
      rows.push([`${d.m.npc}/${it.file}`, it.kind, it.logical_names || '-', content ? `${CONTENT}/${content}.uasset` : '-', srcOf(content), it.bytes, it.sha256, it.duration_s ?? '-', it.num_frames ?? '-', it.loop === undefined ? '-' : it.loop ? 'yes' : 'no', it.root_track_moves === undefined ? '-' : it.root_track_moves ? 'yes' : 'no',
        v ? `independent read-back ${v.independent.bone_names_match === false ? 'BONES DIFFER' : 'ok'}${v.validator ? `; gltf-validator ${v.validator.errors} errors ${v.validator.warnings} warnings` : ''}` : (it.kind === 'data' ? 'written from the asset' : '-'),
        it.kind === 'mesh' ? `SkeletalMesh of TresCharMesh0 in /Game/Blueprints/Npc/${d.m.npc}/${d.m.npc}_Pawn (+ PawnBase)` : it.kind === 'data' ? it.content : `rows ${(it.logical_names || []).slice(0, 4).join(', ')} of the anim sets of /Game/Character/npc/${d.m.npc}/anm/${d.m.npc}_Anim`]);
    }
    const mb = d.files.get('manifest.json'); rows.push([`${d.m.npc}/manifest.json`, 'manifest', '-', '-', '-', mb.length, sha256(mb), '-', '-', '-', '-', '-', 'written by tools/party_pack.js from the items above']);
    const mj = P(d.dir, 'materials/materials.json'); if (fs.existsSync(mj)) { const b = fs.readFileSync(mj); rows.push([`${d.m.npc}/materials/materials.json`, 'data', '-', '-', '-', b.length, sha256(b), '-', '-', '-', '-', d.validation.materials, 'tools/sora_material_pack.js outfit mode over the body and weapon material instances, renumbered by tools/party_pack.js']); }
    for (const w of d.weapons) { const b = fs.readFileSync(P(OUT, w.dir, 'weapon.glb')); const v = d.validation.files[`../weapons/${w.model}/weapon.glb`];
      rows.push([`${w.dir}/weapon.glb`, 'mesh', [w.item], `${CONTENT}/Character/wep/${w.model}/mdl/${w.model}_0.uasset`, srcOf(`Character/wep/${w.model}/mdl/${w.model}_0`), b.length, sha256(b), '-', '-', '-', '-', v ? `gltf-validator ${v.validator ? v.validator.errors : '-'} errors; read-back ok` : '-', `TresWeaponMesh0.SkeletalMesh of the weapon blueprint of ${w.item_key} (TresEquipItemData ${w.row})`]); }
  }
  { const b = fs.readFileSync(indexFile); rows.push(['party.json', 'manifest', '-', '-', '-', b.length, sha256(b), '-', '-', '-', '-', '-', 'index of the members (tools/party_pack.js)']); }
  rows.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
  fs.writeFileSync(sheetFile, `{"sheet":"party-assets","group":"port","columns":${JSON.stringify(cols)},"rows":[${NL}${rows.map((r) => JSON.stringify(r)).join(',' + NL)}${NL}]}${NL}`);
  console.log(`sheets/port/party-assets.json: ${rows.length} rows`);
  const bad = done.reduce((s, d) => s + d.validation.failed, 0);
  console.log(`party_pack ${VERSION}: ${done.map((d) => d.m.name).join(', ')}; failed checks ${bad}`);
  if (bad) process.exitCode = 1;
}
main().catch((e) => { console.error(e); process.exit(1); });
