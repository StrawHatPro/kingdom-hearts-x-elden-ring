// Stage-1 asset pack for the player character Sora (p_ex001): mesh + skeleton + materials + locomotion/jump animations
// as glTF 2.0 binaries in glTF conventions (metres, +Y up, right-handed, character facing +Z), PNG textures and a manifest.
//
//   node tools/sora_pack.js            build build/sora_pack/, validate it, write sheets/port/sora-assets.json
//   node tools/sora_pack.js --check    validate the pack on disk only (no writing)
//
// Inputs (nothing under assets/ or assets_converted/ is modified):
//   assets_converted/packages/...      decoded packages: pawn blueprint, AnimBlueprint, TresAnimSet, BlendSpace1D, materials
//   assets_converted/meshes/...        p_ex001_0.glb (tools/mesh_convert.js): LOD 0 is repacked
//   assets_converted/animations/...    one .glb + .anim.json per AnimSequence (tools/anim_convert.js)
//   assets_converted/textures/...      .dds per texture (tools/texture_convert.js), decoded by tools/lib/bcdec.js
//   assets/...                         only hashed (sha256 of the cooked files each item comes from)
//   audit/sora_port/*.json             data extracted by the research passes (anim graph, leg IK, KineDriver, Bonamik,
//                                      physics asset); copied into the pack's data/ folder and described in the manifest
// Everything that names an asset is read from the game's data (the mesh component of the pawn blueprint, the anim
// set of the AnimBlueprint, the samples of the blend spaces, the texture parameters of the material instances);
// the only authored table is STAGE1 below: the play-call facts (mode, blend-in) of the logical animations stage 1 plays.
// Since 1.5.0 EVERY row of the main anim set (p_ex001_AnimSet: sequences, BlendSpace1D and 2D BlendSpace assets) is packed:
// one anims/<sequence>.glb per sequence, all notifies at full precision; rows outside STAGE1 have play.mode null (the
// play mode is the argument of the state that plays them). sora_all.glb, the KineDriver baked-track check and the
// per-frame root tracks of clips WITHOUT bEnableRootMotion stay limited to the stage-1 clips (size and build time).
// Findings and the pack format: docs/sora-animation-spec.md. The pack is local build output; it is never distributed.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const bc = require('./lib/bcdec');
const kd = require('./lib/kd_eval');
const bnm = require('./lib/bonamik/bnm_eval.js');
const bnmNotify = require('./lib/bonamik/notify.js');

const VERSION = '1.6.0';
const ROOT = path.resolve(__dirname, '..');
const OUT = path.join(ROOT, 'build', 'sora_pack');
const CONTENT = 'KINGDOM HEARTS III/Content';
const PKG = path.join(ROOT, 'assets_converted', 'packages', CONTENT);
const P = (...a) => path.join(...a);

const PAWN_BP = 'Blueprints/Player/p_ex001/p_ex001_Pawn';
// Logical animation names (rows of the TresAnimSet) needed for stage 1. `loop` and `use` restate what the player
// states do with the name (docs/sora-animation-spec.md gives the function addresses); `optional` rows are packed
// because they are cheap and belong to the same states, but stage 1 works without them.
const STAGE1 = [
  // mode = the bit field the state passes to the play call (bit 0 loop, bit 1 root motion); blend = the blend-in it passes (-1 = the asset's m_InterpolationTime)
  { name: 'IdleN', mode: 2, blend: -1, by: 'Idle begin 0x141386050 / tick 0x1413944D0', use: 'idle (no weapon drawn); not looped: the Idle state plays it again each time it ends' },
  { name: 'IdleBreakN', mode: 2, blend: -1, by: 'Idle tick 0x1413944D0', use: 'idle fidget, once after the idle animation has ended 3 times', optional: true },
  { name: 'WalkLoopN', mode: 1, blend: -1, by: 'Run tick 0x1413AED70', use: 'walk loop (stick magnitude < 0.8)' },
  { name: 'WalkEndR', mode: 2, blend: -1, by: 'Run tick 0x1413AED70', use: 'stop from walk when m_bReverseLeg is clear' },
  { name: 'WalkEndL', mode: 2, blend: -1, by: 'Run tick 0x1413AED70', use: 'stop from walk when m_bReverseLeg is set (TresReverseLeg notify window of the loop)' },
  { name: 'RunStartN', mode: 2, blend: -1, by: 'Run tick 0x1413AED70', use: 'start running from idle, and sharp turn (|delta| >= 75 degrees) while running; blend-space X = delta = target yaw - actor yaw in degrees, negative = left, set once' },
  { name: 'RunLoopN', mode: 1, blend: -1, by: 'Run tick 0x1413AED70', use: 'run loop; blend-space X = clamp(delta, -10, 10) every tick' },
  { name: 'RunEndR', mode: 2, blend: -1, by: 'Run tick 0x1413AED70', use: 'stop from run when m_bReverseLeg is clear' },
  { name: 'RunEndL', mode: 2, blend: -1, by: 'Run tick 0x1413AED70', use: 'stop from run when m_bReverseLeg is set' },
  { name: 'TurnN', mode: null, blend: null, by: 'not played by the player Idle/Run/Jump states (selector 0x141435690 is called from cinematic and AI states)', use: 'turn in place; blend space over the turn angle', optional: true },
  { name: 'RunSwayR', mode: 2, blend: 0.1, by: 'TresPlayerState_Swaying 0x1413CC7B0 (not re-read)', use: 'sway right; separate state entered with a target object', optional: true },
  { name: 'RunSwayL', mode: 2, blend: 0.1, by: 'TresPlayerState_Swaying 0x1413CC7B0 (not re-read)', use: 'sway left', optional: true },
  { name: 'JumpNormalStart', mode: 0, blend: -1, by: 'Jump SetSubStep 0x1413A0BF0', use: 'jump take-off; the launch impulse is applied at its TresJumpStart notify' },
  { name: 'JumpNormalRiseLoop', mode: 1, blend: -1, by: 'Jump tick 0x141395450', use: 'rising, from the end of the take-off animation until vertical velocity <= 0' },
  { name: 'JumpNormalDownLoop', mode: 1, blend: -1, by: 'Jump tick 0x141395450', use: 'falling (also the entry when walking off a ledge)' },
  { name: 'Land', mode: 2, blend: -1, by: 'Jump tick 0x141395450', use: 'landing after a fall of more than 75 cm' },
  { name: 'LightLand', mode: 2, blend: -1, by: 'Jump tick 0x141395450', use: 'landing after a fall of at most 75 cm' },
  { name: 'LightLandR', mode: 2, blend: -1, by: 'Jump begin 0x1413868C0 / Idle begin 0x141386050 through the pawn land pose', use: 'only when a TresAnimNotify_LandAnimPose stored pose LIGHT_R on the pawn; not chosen by the normal landing code', optional: true },
  { name: 'LightLandL', mode: 2, blend: -1, by: 'Jump begin 0x1413868C0 / Idle begin 0x141386050 through the pawn land pose', use: 'as LightLandR, pose LIGHT_L', optional: true },
];
// Face clips the stage-1 body clips start through their TresFaceAnim* notifies (rows of the face anim set, docs/sora-animation-spec.md "Face").
const FACE_ROWS = ['NO_0_EL', 'NO_6_EL', 'NO_21_EL', 'NO_50_Lip', 'NO_51_Lip', 'NO_52_Lip'];
// Research data (audit/sora_port/<source>) copied into the pack as data/<file>.
const DATA_FILES = [
  { file: 'data/animgraph.json', source: 'animgraph.json', what: 'the 34 nodes of the AnimBlueprint p_ex001_Anim: link_index (= LinkID), struct, resolved links, every serialized property, bound inputs; schema inside the file', notes: 'build/sora_tmp/native_C_graph.md' },
  { file: 'data/legik.json', source: 'legik.json', what: 'leg IK: the TresAnimNode_TwoBoneIKMultiped node (the one that runs) with its IKDefs and hips controller, the two unused CollTwoBoneIK nodes, footstep units, native defaults of unserialized properties; schema inside the file', notes: 'build/sora_tmp/native_D_legik.md' },
  { file: 'data/kinedriver.json', source: 'kinedriver.json', what: 'KineDriver: 313 operators in evaluation order with resolved bones and all parameters, and per bone what the driver writes; schema inside the file. Reference evaluator: tools/lib/kd_eval.js', notes: 'build/sora_tmp/native_F_kinedriver.md' },
  { file: 'data/bonamik.json', source: 'bonamik.json', what: 'Bonamik secondary motion, the asset data: scene, groups, bodies (particles), links, cones, planes, chains, driven bones, notify-state effects per clip; schema inside the file (_schema). This is the file the reference evaluator loads', notes: 'build/sora_tmp/native_G_bonamik.md' },
  { file: 'data/bonamik2.json', source: 'bonamik2.json', what: 'Bonamik: bonamik.json plus the block runtime2 (schema in _schema2) = the runtime records right after setup on the reference pose: gravity, iterations, per-group state, 252 particles, 164 links, 49 cones with their quaternions, 18 anchors, 37 chain roots, 57 dynamic collision objects, collider lists, 24 self-collision pairs, the KDI record. Reference evaluator: tools/lib/bonamik/', notes: 'build/sora_tmp/native_H_bonamik2.md' },
  { file: 'data/physics_asset.json', source: 'physics_asset.json', what: 'PhysicsAsset p_ex001_0_Physics: 11 bodies, 9 constraints. It moves no bone for the player in field play; included for completeness', notes: 'build/sora_tmp/native_G_bonamik.md section 7' },
];
const ROOT_MOTION_BONES = ['kh_sk', 'trans'];
const MAX_JOINTS = 256; // limit some loaders put on one skin

// ------------------------------------------------------------------------------------------------ helpers
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const rel = (f) => path.relative(ROOT, f).replace(/\\/g, '/');
const f32 = (x) => Math.fround(x);
function gamePathToContent(p) { // "/Game/A/B.B" -> "A/B"
  const m = /^\/Game\/(.+?)(?:\.[^./]+)?$/.exec(p);
  if (!m) throw new Error('not a /Game path: ' + p);
  return m[1];
}
function loadPkg(contentPath) {
  const f = P(PKG, contentPath + '.uasset.json');
  const d = readJson(f);
  d._file = f; d._content = contentPath;
  return d;
}
function importPath(d, i) { // full object path of an import index (negative)
  let im = d.imports[-i - 1], s = im.objectName;
  while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; s = im.objectName + '.' + s; }
  return s;
}
const propsOf = (e) => Object.fromEntries(((e.data && e.data.props) || []).map((p) => [p.name + (p.arrayIndex ? `[${p.arrayIndex}]` : ''), p]));
const structOf = (items) => Object.fromEntries(items.map((p) => [p.name + (p.arrayIndex ? `[${p.arrayIndex}]` : ''), p]));
function sourceFiles(contentPath) { // cooked files of a package, with hashes
  const out = {};
  for (const ext of ['uasset', 'uexp', 'ubulk']) {
    const f = P(ROOT, 'assets', CONTENT, contentPath + '.' + ext);
    if (fs.existsSync(f)) out[ext] = sha256(fs.readFileSync(f));
  }
  if (!out.uasset) throw new Error('source package missing: ' + contentPath);
  return out;
}
const assetId = (contentPath) => `${CONTENT}/${contentPath}.uasset`;

// ---- glb reading / writing
function readGlb(file) {
  const b = fs.readFileSync(file);
  if (b.readUInt32LE(0) !== 0x46546c67) throw new Error('not a glb: ' + file);
  const jl = b.readUInt32LE(12);
  const json = JSON.parse(b.toString('utf8', 20, 20 + jl));
  const bo = 20 + jl;
  const bin = bo < b.length ? b.subarray(bo + 8, bo + 8 + b.readUInt32LE(bo)) : Buffer.alloc(0);
  return { json, bin, bytes: b };
}
const COMPS = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };
const CT = { 5120: [1, 'Int8'], 5121: [1, 'UInt8'], 5122: [2, 'Int16LE'], 5123: [2, 'UInt16LE'], 5125: [4, 'UInt32LE'], 5126: [4, 'FloatLE'] };
function readAccessor(g, i) { // -> { n, comps, data: number[] (flat), normalized, componentType }
  const a = g.json.accessors[i], bv = g.json.bufferViews[a.bufferView];
  const comps = COMPS[a.type], [size, kind] = CT[a.componentType];
  const stride = bv.byteStride || comps * size, off = (bv.byteOffset || 0) + (a.byteOffset || 0);
  const data = new Array(a.count * comps);
  const rd = 'read' + kind;
  for (let k = 0; k < a.count; k++) for (let c = 0; c < comps; c++) data[k * comps + c] = g.bin[rd](off + k * stride + c * size);
  return { n: a.count, comps, data, normalized: !!a.normalized, componentType: a.componentType };
}
class GlbWriter {
  constructor(generatorNote) {
    this.json = { asset: { version: '2.0', generator: `kh3-decomp tools/sora_pack.js ${VERSION}${generatorNote ? ' ' + generatorNote : ''}` }, scene: 0, scenes: [], nodes: [], buffers: [{ byteLength: 0 }], bufferViews: [], accessors: [] };
    this.chunks = []; this.length = 0;
  }
  view(buf, target) {
    const pad = (4 - (this.length % 4)) % 4;
    if (pad) { this.chunks.push(Buffer.alloc(pad)); this.length += pad; }
    const v = { buffer: 0, byteOffset: this.length, byteLength: buf.length };
    if (target) v.target = target;
    this.chunks.push(buf); this.length += buf.length;
    return this.json.bufferViews.push(v) - 1;
  }
  // values: flat number array. type: glTF accessor type. ct: component type. minmax: write min/max.
  accessor(values, type, ct, opts = {}) {
    const comps = COMPS[type], [size, kind] = CT[ct], n = values.length / comps;
    const buf = Buffer.alloc(values.length * size), wr = 'write' + kind;
    for (let i = 0; i < values.length; i++) buf[wr](values[i], i * size);
    const a = { bufferView: this.view(buf, opts.target), componentType: ct, count: n, type };
    if (opts.normalized) a.normalized = true;
    if (opts.minmax) {
      const mn = new Array(comps).fill(Infinity), mx = new Array(comps).fill(-Infinity);
      for (let i = 0; i < values.length; i++) { const c = i % comps, v = ct === 5126 ? f32(values[i]) : values[i]; if (v < mn[c]) mn[c] = v; if (v > mx[c]) mx[c] = v; }
      a.min = mn; a.max = mx;
    }
    return this.json.accessors.push(a) - 1;
  }
  finish() {
    const bin = Buffer.concat(this.chunks);
    const binPad = (4 - (bin.length % 4)) % 4;
    this.json.buffers[0].byteLength = bin.length;
    let js = Buffer.from(JSON.stringify(this.json), 'utf8');
    const jsPad = (4 - (js.length % 4)) % 4;
    if (jsPad) js = Buffer.concat([js, Buffer.alloc(jsPad, 0x20)]);
    const total = 12 + 8 + js.length + 8 + bin.length + binPad;
    const h = Buffer.alloc(12); h.writeUInt32LE(0x46546c67, 0); h.writeUInt32LE(2, 4); h.writeUInt32LE(total, 8);
    const jh = Buffer.alloc(8); jh.writeUInt32LE(js.length, 0); jh.writeUInt32LE(0x4e4f534a, 4);
    const bh = Buffer.alloc(8); bh.writeUInt32LE(bin.length + binPad, 0); bh.writeUInt32LE(0x004e4942, 4);
    return Buffer.concat([h, jh, js, bh, bin, Buffer.alloc(binPad)]);
  }
}

// ---- the coordinate conversion (see CONVENTIONS): a reflection that swaps Y and Z, and centimetres -> metres
const UNIT = 0.01;
const cvPos = (x, y, z) => [x * UNIT, z * UNIT, y * UNIT];
const cvDir = (x, y, z) => [x, z, y];
const cvQuat = (x, y, z, w) => [-x, -z, -y, w];
const cvScale = (x, y, z) => [x, z, y];
const CONVENTIONS = {
  source: 'Unreal Engine: centimetres, +Z up, left-handed; the mesh component is yawed -90 degrees in the pawn, so in mesh space the character faces +Y and its right hand is on -X',
  pack: 'glTF: metres, +Y up, right-handed; the character faces +Z, its left hand is on +X',
  position: '(x, y, z)_UE cm -> (x, z, y) * 0.01 m',
  direction: '(x, y, z)_UE -> (x, z, y)   (normals, tangent xyz)',
  tangent_w: 'w_pack = -w of assets_converted/meshes TANGENT (a reflection flips the bitangent sign)',
  rotation_quaternion: '(x, y, z, w)_UE -> (-x, -z, -y, w)',
  scale: '(x, y, z)_UE -> (x, z, y)',
  matrix_form: 'M_pack = C * M_UE * C^-1 with C = 0.01 * [[1,0,0],[0,0,1],[0,1,0]] (C is its own inverse up to the unit factor: apply the same rules with * 100 to go back)',
  triangle_winding: 'counter-clockwise is front in the pack. assets_converted/meshes stores a UE triangle (a, b, c) as (a, c, b) under a mirrored root node; with the mirror baked into the vertices the pack stores (a, b, c) again',
  uv: 'unchanged (origin top-left in both)',
  baked: 'the conversion is baked into every vertex, bone and animation key; no file has a conversion node',
};

// ---- small matrix helpers (column-major 4x4, doubles)
function trsMatrix(t, q, s) {
  const [x, y, z, w] = q;
  const xx = x * x, yy = y * y, zz = z * z, xy = x * y, xz = x * z, yz = y * z, wx = w * x, wy = w * y, wz = w * z;
  return [
    (1 - 2 * (yy + zz)) * s[0], (2 * (xy + wz)) * s[0], (2 * (xz - wy)) * s[0], 0,
    (2 * (xy - wz)) * s[1], (1 - 2 * (xx + zz)) * s[1], (2 * (yz + wx)) * s[1], 0,
    (2 * (xz + wy)) * s[2], (2 * (yz - wx)) * s[2], (1 - 2 * (xx + yy)) * s[2], 0,
    t[0], t[1], t[2], 1];
}
function mul(a, b) { const o = new Array(16); for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) { let s = 0; for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k]; o[c * 4 + r] = s; } return o; }
function invertAffine(m) {
  const a = m[0], b = m[4], c = m[8], d = m[1], e = m[5], f = m[9], g = m[2], h = m[6], i = m[10];
  const det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
  if (Math.abs(det) < 1e-12) throw new Error('singular bind matrix');
  const r = [(e * i - f * h) / det, (c * h - b * i) / det, (b * f - c * e) / det, (f * g - d * i) / det, (a * i - c * g) / det, (c * d - a * f) / det, (d * h - e * g) / det, (b * g - a * h) / det, (a * e - b * d) / det];
  const tx = m[12], ty = m[13], tz = m[14];
  return [r[0], r[3], r[6], 0, r[1], r[4], r[7], 0, r[2], r[5], r[8], 0,
    -(r[0] * tx + r[1] * ty + r[2] * tz), -(r[3] * tx + r[4] * ty + r[5] * tz), -(r[6] * tx + r[7] * ty + r[8] * tz), 1];
}
const xform = (m, p) => [m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12], m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13], m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14]];

// ------------------------------------------------------------------------------------------------ 1. identification
function identify() {
  const pawn = loadPkg(PAWN_BP);
  const cls = pawn.exports.find((e) => e.class === 'BlueprintGeneratedClass');
  const baseClassPath = importPath(pawn, cls.superIndex);
  const meshComp = pawn.exports.find((e) => e.class === 'TresSkeletalMeshComponent');
  const mp = propsOf(meshComp);
  const meshPath = importPath(pawn, mp.SkeletalMesh.value), animClassPath = importPath(pawn, mp.AnimClass.value);
  // the parent blueprint holds the transform of the mesh component and the collision capsule of the root component
  const base = loadPkg(gamePathToContent(baseClassPath));
  const baseCls = base.exports.find((e) => e.class === 'BlueprintGeneratedClass');
  const nativeParent = importPath(base, baseCls.superIndex);
  const bMesh = propsOf(base.exports.find((e) => e.class === 'TresSkeletalMeshComponent'));
  const bRoot = propsOf(base.exports.find((e) => e.class === 'TresRootComponent'));
  const shape = structOf(bRoot.CollisionShapesSrc.value.items[0]);
  const move = propsOf(base.exports.find((e) => e.class === 'TresAICharMovementComponent'));
  const meshPkg = loadPkg(gamePathToContent(meshPath));
  const imp = (cn) => meshPkg.imports.map((im, i) => ({ im, i })).filter((x) => x.im.className === cn).map((x) => importPath(meshPkg, -(x.i + 1)));
  const anim = loadPkg(gamePathToContent(animClassPath));
  const animCls = anim.exports.find((e) => e.class === 'AnimBlueprintGeneratedClass');
  const cdo = propsOf(anim.exports.find((e) => /^Default__/.test(e.objectName)));
  const objList = (p) => (p ? p.value.filter((v) => v).map((v) => importPath(anim, v)) : []);
  return {
    pawn_blueprint: `/Game/${PAWN_BP}`, pawn_parent_blueprint: baseClassPath, pawn_native_class: nativeParent,
    skeletal_mesh: meshPath, anim_class: animClassPath, anim_native_parent: importPath(anim, animCls.superIndex),
    anim_target_skeleton: importPath(anim, propsOf(animCls).TargetSkeleton.value),
    skeleton: imp('Skeleton')[0], physics_asset: imp('PhysicsAsset')[0],
    material_instances: imp('MaterialInstanceConstant'),
    other_mesh_user_data: { bonamik: imp('SQEX_BonamikAsset'), kine_driver: imp('SQEX_KineDriverData') },
    anim_sets: objList(cdo.MyAnimSets), sub_anim_sets: objList(cdo.m_SubAnimSets), cache_start_pose_anim: cdo.m_CacheStartPoseAnimName && cdo.m_CacheStartPoseAnimName.value,
    mesh_component: {
      name: meshComp.objectName, relative_location_ue_cm: bMesh.RelativeLocation ? bMesh.RelativeLocation.value : { x: 0, y: 0, z: 0 },
      relative_location_note: bMesh.RelativeLocation ? 'set in the parent blueprint' : 'not serialized in p_ex001_Pawn or p_ex001_PawnBase, so it is the native default: the constructor that creates TresCharMesh0 (0x14059F950, verified) attaches it to the root and never writes its RelativeLocation, and neither do the TresPlayerPawnBase (0x141423210) and TresPlayerPawnSora (0x1413E7580) constructors; the engine default is (0, 0, 0)',
      relative_rotation_ue_deg: bMesh.RelativeRotation.value, attach_parent: 'TresRoot (TresRootComponent, the actor root)',
    },
    collision_capsule: { component: 'TresRoot', shape: shape.ShapeType.value, size_ue_cm: shape.Size.value, relative_location_ue_cm: shape.RelativeLocation.value,
      note: 'TresCollShapeAssetUnit of TresRootComponent.CollisionShapesSrc. The shape builder 0x1405DF310 makes a capsule with radius = max(Size.X, 1), half height = max(Size.Z, 1) (cylinder length 2 * (half height - radius)), centre = RelativeLocation: radius 60, half height 89, centre 91 cm above the actor origin, i.e. bottom 2 cm above the origin (feet level), top at 180 cm' },
    movement_component_overrides: Object.fromEntries(Object.entries(move).filter(([k]) => k !== 'None').map(([k, v]) => [k, v.value])),
  };
}

// ------------------------------------------------------------------------------------------------ 2. animation set
function loadAnimSet(gamePath) {
  const d = loadPkg(gamePathToContent(gamePath));
  const e = d.exports.find((x) => x.class === 'TresAnimSet');
  const rows = propsOf(e).Anims.value.items.map((it) => { const s = structOf(it); return { name: s.AnimName.value, asset: s.AnimData.value ? importPath(d, s.AnimData.value) : null }; });
  return { content: d._content, rows };
}
function describeAnimAsset(gamePath) {
  const content = gamePathToContent(gamePath);
  const d = loadPkg(content);
  const main = d.exports.find((e) => e.outerIndex === 0);
  const p = propsOf(main);
  if (main.class === 'AnimSequence') return { class: 'AnimSequence', content };
  if (main.class === 'BlendSpace1D') {
    const samples = p.SampleData.value.items.map((it) => { const s = structOf(it); return { value: s.SampleValue.value.x, asset: importPath(d, s.Animation.value), rate_scale: s.RateScale ? s.RateScale.value : 1 }; }).sort((a, b) => a.value - b.value);
    const bp0 = p.BlendParameters ? structOf(p.BlendParameters.value) : {};
    const ip = p.InterpolationParam ? structOf(p.InterpolationParam.value) : {};
    const ipType = ip.InterpolationType ? ip.InterpolationType.value : 'BSIT_Linear', win = ip.InterpolationTime ? ip.InterpolationTime.value : 0;
    const grid = (p.GridSamples ? p.GridSamples.value.items : []).map((it) => { const g = structOf(it); const e = []; for (let k = 0; k < 3; k++) { const ix = g['Indices' + (k ? `[${k}]` : '')], wv = g['Weights' + (k ? `[${k}]` : '')]; if (ix && ix.value >= 0 && wv && wv.value > 0) e.push({ sample_index: ix.value, weight: wv.value }); } return e; });
    return { class: 'BlendSpace1D', content, axis: { min: bp0.Min ? bp0.Min.value : 0, max: bp0.Max ? bp0.Max.value : 100, name: bp0.DisplayName ? bp0.DisplayName.value : null, grid_num: bp0.GridNum ? bp0.GridNum.value : 4 },
      grid_samples: grid, sample_order_in_asset: p.SampleData.value.items.map((it) => path.basename(gamePathToContent(importPath(d, structOf(it).Animation.value)))),
      sample_weight_rule: 'stock UBlendSpace1D: n = (clamp(x, min, max) - min) * grid_num / (max - min); between grid points g = floor(n) and g + 1 the grid entries (grid_samples, indices into sample_order_in_asset) are mixed with 1 - frac and frac; entries of the same sequence are merged, weights below 1e-5 dropped, the rest normalised',
      input_filter: win > 0 ? { type: ipType, type_note: 'InterpolationType is not serialized: constructor default BSIT_Linear (0x143F5A5C0)', window_s: win,
        algorithm: 'FFIRFilterTimeBased as in this exe (0x143F425B0): one entry per animation update; entry stamp = clock before this update\'s dt + window; an entry is dropped when clock - stamp > window; output = sum(w * input) / sum(w) over entries with w = 1 - |clock - stamp| / window (Linear) and w > 0. In terms of the age a of an input (time since the update that added it started): triangular weight peaking at a = window, zero at a = 0 and a = 2 * window. At play the filter is reset to one entry holding the play-time parameter with stamp = clock (weight 1 - elapsed / window). Not weighted by dt.' }
        : { type: 'none', window_s: 0, algorithm: 'no InterpolationParam: the parameter is used unfiltered on the update after it is set' },
      target_weight_interpolation_speed_per_sec: 0, per_bone_blend: [], time_sync: 'one normalised position for all samples; sample time = position * its SequenceLength (all samples have the same length)',
      root_motion_rule: 'weighted blend of every sample\'s root delta (weights clamped 0..1; samples without bEnableRootMotion contribute identity)',
      m_InterpolationTime: p.m_InterpolationTime ? p.m_InterpolationTime.value : null,
      notify_trigger_mode: p.NotifyTriggerMode ? p.NotifyTriggerMode.value : null, samples };
  }
  if (main.class === 'BlendSpace') { // 2D: two axes, samples at (x, y), the asset's own grid (triangulated weights per grid point)
    const order = p.SampleData.value.items.map((it) => { const s = structOf(it); return { value: s.SampleValue.value.x, value_y: s.SampleValue.value.y, asset: importPath(d, s.Animation.value), rate_scale: s.RateScale ? s.RateScale.value : 1 }; });
    const ax = (k) => { const b = p['BlendParameters' + (k ? '[' + k + ']' : '')]; const o = b ? structOf(b.value) : {}; return { min: o.Min ? o.Min.value : 0, max: o.Max ? o.Max.value : 100, name: o.DisplayName ? o.DisplayName.value : null, grid_num: o.GridNum ? o.GridNum.value : 4 }; };
    const win = (k) => { const b = p['InterpolationParam' + (k ? '[' + k + ']' : '')]; const o = b ? structOf(b.value) : {}; return o.InterpolationTime ? o.InterpolationTime.value : 0; };
    const grid = (p.GridSamples ? p.GridSamples.value.items : []).map((it) => { const g = structOf(it); const e = []; for (let k = 0; k < 3; k++) { const ix = g['Indices' + (k ? `[${k}]` : '')], wv = g['Weights' + (k ? `[${k}]` : '')]; if (ix && ix.value >= 0 && wv && wv.value > 0) e.push({ sample_index: ix.value, weight: wv.value }); } return e; });
    return { class: 'BlendSpace', content, axis: ax(0), axis_y: ax(1), grid_samples: grid, sample_order_in_asset: order.map((x) => path.basename(gamePathToContent(x.asset))),
      sample_weight_rule: 'stock UBlendSpace (2D): grid of (axis.grid_num + 1) x (axis_y.grid_num + 1) points, index = x_index * (axis_y.grid_num + 1) + y_index (stock UBlendSpace::GetEditorElement: X outer, Y inner); the four grid points around the input are mixed bilinearly, each contributing its grid_samples entries (indices into sample_order_in_asset); entries of the same sequence are merged, the result normalised. NOT verified against the exe for this fork',
      input_filter: win(0) > 0 || win(1) > 0 ? { type: 'BSIT_Linear', window_s: win(0), window_y_s: win(1), algorithm: 'as the 1D blend spaces (FFIRFilterTimeBased per axis)' } : { type: 'none', window_s: 0, window_y_s: 0, algorithm: 'no InterpolationParam: the parameters are used unfiltered' },
      target_weight_interpolation_speed_per_sec: 0, per_bone_blend: [], m_InterpolationTime: p.m_InterpolationTime ? p.m_InterpolationTime.value : null,
      notify_trigger_mode: p.NotifyTriggerMode ? p.NotifyTriggerMode.value : null, samples: order };
  }
  throw new Error(`animation asset class not handled: ${main.class} (${gamePath})`);
}

// ------------------------------------------------------------------------------------------------ 3. textures / materials
const texCache = new Map();
function texturePng(gamePath, mode, extra = {}) { // mode: color | normal | alpha_tint | linear
  const content = gamePathToContent(gamePath);
  const key = content + '|' + mode;
  if (texCache.has(key)) return texCache.get(key);
  const dds = P(ROOT, 'assets_converted', 'textures', CONTENT, content + '.dds');
  const side = readJson(P(ROOT, 'assets_converted', 'textures', CONTENT, content + '.texture.json'));
  const sp = Object.fromEntries(side.props.map((x) => [x.tag.name, x.value]));
  const buf = fs.readFileSync(dds);
  const t = bc.decodeDds(buf, 0);
  // decoder check against the next mip of the same file (other blocks, other BC7 shapes)
  let mipCheck = null;
  if (t.mipCount > 1 && t.width >= 8 && t.height >= 8) {
    const m1 = bc.decodeDds(buf, 1); let sum = 0, worst = 0;
    for (let y = 0; y < m1.height; y++) for (let x = 0; x < m1.width; x++) for (let c = 0; c < 4; c++) {
      const s = t.rgba[((y * 2) * t.width + x * 2) * 4 + c] + t.rgba[((y * 2) * t.width + x * 2 + 1) * 4 + c] + t.rgba[((y * 2 + 1) * t.width + x * 2) * 4 + c] + t.rgba[((y * 2 + 1) * t.width + x * 2 + 1) * 4 + c];
      const e = Math.abs(s / 4 - m1.rgba[(y * m1.width + x) * 4 + c]); sum += e; if (e > worst) worst = e;
    }
    mipCheck = { mean_abs_diff: +(sum / (m1.width * m1.height * 4)).toFixed(3), max_abs_diff: +worst.toFixed(1) };
  }
  const px = Buffer.from(t.rgba);
  let alpha = false, note, srgb = sp.SRGB === undefined ? true : !!sp.SRGB;
  if (mode === 'color') note = 'RGB of the top mip; the source alpha channel is not opacity in these materials (it feeds other shader inputs) and is dropped';
  else if (mode === 'normal') {
    for (let i = 0; i < px.length; i += 4) {
      const x = px[i] / 127.5 - 1, y = px[i + 1] / 127.5 - 1; const z = Math.sqrt(Math.max(0, 1 - x * x - y * y));
      px[i + 1] = 255 - px[i + 1]; px[i + 2] = Math.round((z * 0.5 + 0.5) * 255);
    }
    srgb = false; note = 'BC5 two-channel normal map: X kept, Y inverted (Direct3D -> OpenGL convention of glTF), Z = sqrt(1 - x^2 - y^2)';
  } else if (mode === 'alpha_tint') {
    const enc = (v) => Math.round(255 * (v <= 0.0031308 ? 12.92 * v : 1.055 * Math.pow(v, 1 / 2.4) - 0.055));
    const [r, g, b] = extra.tint.map(enc);
    for (let i = 0; i < px.length; i += 4) { px[i] = r; px[i + 1] = g; px[i + 2] = b; }
    alpha = true; srgb = true; note = `alpha = source alpha (the mask), RGB = the material's colour parameter ${extra.tintName} ${JSON.stringify(extra.tint)} (linear) encoded as sRGB`;
  }
  const name = path.basename(content) + (mode === 'alpha_tint' ? '_tinted' : '') + '.png';
  const png = bc.encodePng(t.width, t.height, px, { alpha, srgb });
  const r = { file: 'textures/' + name, png, content, mode, width: t.width, height: t.height, pixel_format: t.format, srgb, note, mip_check: mipCheck,
    bc7_modes: t.format === 'PF_BC7' ? t.stats.modes : undefined, compression_settings: sp.CompressionSettings || 'TC_Default' };
  texCache.set(key, r);
  return r;
}
// Material instance -> glTF material. The game's shaders are not reproduced: the pack carries the base colour
// (texture or a colour parameter), optionally the normal map, and says in `approximation` what was simplified.
function materialInfo(gamePath) {
  const chain = []; const tex = {}, scal = {}, vec = {};
  let p = gamePath, parentMaterial = null, blend = null, twoSided = null;
  while (p && p.startsWith('/Game/')) {
    const d = loadPkg(gamePathToContent(p));
    const e = d.exports.find((x) => x.outerIndex === 0 && /^Material/.test(x.class));
    const pr = propsOf(e);
    chain.push(p);
    if (e.class === 'Material') {
      parentMaterial = p; blend = pr.BlendMode ? pr.BlendMode.value : 'BLEND_Opaque'; twoSided = pr.TwoSided ? !!pr.TwoSided.value : false;
      // texture defaults of the root material: parameters, and unparameterised samples by export name
      for (const x of d.exports) { if (!/MaterialExpressionTextureSample/.test(x.class)) continue; const xp = propsOf(x); if (!xp.Texture || !xp.Texture.value) continue; const n = xp.ParameterName ? xp.ParameterName.value : '#' + path.basename(gamePathToContent(importPath(d, xp.Texture.value))); if (!(n in tex)) tex[n] = importPath(d, xp.Texture.value); }
      break;
    }
    const grab = (prop, into, val) => { for (const it of (pr[prop] ? pr[prop].value.items : [])) { const s = structOf(it); const n = s.ParameterName.value; if (!(n in into)) into[n] = val(s.ParameterValue.value); } };
    grab('TextureParameterValues', tex, (v) => (v ? importPath(d, v) : null));
    grab('ScalarParameterValues', scal, (v) => v);
    grab('VectorParameterValues', vec, (v) => [v.r, v.g, v.b, v.a]);
    p = pr.Parent && pr.Parent.value ? importPath(d, pr.Parent.value) : null;
  }
  return { chain, root_material: parentMaterial, blend_mode: blend, two_sided: twoSided, tex, scal, vec };
}
const RULES = [ // by root material; first match wins
  { root: /p_ex001_Basemat$/, base: 'FormChangeTex0', normal: 'NormalTex', approx: 'base colour = FormChangeTex0 (FormChangeSlide is 0 in the default form); cloth/leather detail tiling, masks, AO and subsurface shading are left out' },
  { root: /pc_skinbasemat$/, base: 'BaseColorTex', normal: 'NormalTex', approx: 'skin shader (subsurface, AO, lip/nail tint) left out' },
  { root: /SQEX_KHHairBaseMat$/, color: ['HairColor', 'HairPointEndColor'], normal: 'Normalmap', doubleSided: true, approx: 'the hair shader has no base colour texture: it grades from HairColor (roots) to HairPointEndColor (tips) with hair-line masks; the pack uses the mean of the two as a flat colour' },
  { root: /SQEX_Mouth$/, base: '#Mouth_M_0d00', normal: '#Mouth_M_0n00', approx: 'the mouth material samples fixed textures of the shader library' },
  { root: /SQEX_WhiteEyeMaterial$/, base: 'EyeTexture', approx: 'eye white: the base texture of the shader; eye shadowing left out' },
  { root: /SQEX_IrisTexEyeMaterial$/, color: ['Color', 'Color_Dark'], approx: 'the iris is procedural in the game (mask texture EyeG_ColorMask coloured by Color/Color_Dark, parallax pupil, highlight); the pack uses the mean of Color and Color_Dark as a flat colour' },
  { root: /SQEX_EyeLens$/, hidden: true, approx: 'translucent eye highlight layer: exported fully transparent (alphaMode BLEND, alpha 0); a consumer may skip the primitive' },
  { root: /SQEX_BaseEyelashes$/, tinted: ['EyelashesMask', 'EyelashesColor'], doubleSided: true, blend: true, approx: 'eyelash colour parameter with the mask texture alpha as opacity' },
];

// ------------------------------------------------------------------------------------------------ 4. build
function buildSkeleton(srcJson) { // bones of a converted glb (node 0 = UE_to_glTF, bones 1..N) -> converted bone list
  const bones = [];
  const parent = new Map();
  srcJson.nodes.forEach((n, i) => (n.children || []).forEach((c) => parent.set(c, i)));
  for (let i = 1; i < srcJson.nodes.length; i++) {
    const n = srcJson.nodes[i];
    if (n.mesh !== undefined) break;
    const t = n.translation || [0, 0, 0], r = n.rotation || [0, 0, 0, 1], s = n.scale || [1, 1, 1];
    bones.push({ name: n.name, parent: parent.get(i) - 1, translation: cvPos(...t), rotation: cvQuat(...r), scale: cvScale(...s), ue: { translation: t, rotation: r, scale: s } });
  }
  if (bones[0].parent !== -1) throw new Error('bone 0 is not the root');
  for (let i = 1; i < bones.length; i++) if (!(bones[i].parent >= 0 && bones[i].parent < i)) throw new Error('bone order: parent after child at ' + i);
  return bones;
}
function addSkeletonNodes(w, bones) {
  bones.forEach((b, i) => { w.json.nodes.push({ name: b.name, translation: b.translation.map(f32), rotation: b.rotation.map(f32), scale: b.scale.map(f32) }); });
  bones.forEach((b, i) => { if (b.parent >= 0) (w.json.nodes[b.parent].children = w.json.nodes[b.parent].children || []).push(i); });
}
function globalBind(bones) {
  const g = [];
  bones.forEach((b, i) => { const l = trsMatrix(b.translation.map(f32), b.rotation.map(f32), b.scale.map(f32)); g.push(b.parent < 0 ? l : mul(g[b.parent], l)); });
  return g;
}

function addMesh(w, meshGlb, bones, materials) {
  const j = meshGlb.json;
  const lod0 = j.meshes[0];
  const cache = new Map(); const rd = (i) => { if (!cache.has(i)) cache.set(i, readAccessor(meshGlb, i)); return cache.get(i); };
  // The skin lists only the bones that carry a vertex weight at LOD 0 (some loaders limit a skin to 256 joints; the
  // skeleton has more bones than that). The other bones stay in the node hierarchy, so animations target them as before.
  const usedBones = new Set();
  for (const pr of lod0.primitives) { const jn0 = rd(pr.attributes.JOINTS_0).data, wt0 = rd(pr.attributes.WEIGHTS_0).data; for (let i = 0; i < jn0.length; i++) if (wt0[i] > 0) usedBones.add(jn0[i]); }
  const joints = [...usedBones].sort((a, b) => a - b);
  if (joints.length > MAX_JOINTS) throw new Error(`${joints.length} weighted bones: more than ${MAX_JOINTS}; a split into several skins is needed and is not implemented`);
  const jointOf = new Map(joints.map((b, k) => [b, k]));
  const allIbm = globalBind(bones).map(invertAffine);
  const skinAcc = w.accessor(joints.map((b) => allIbm[b]).flat(), 'MAT4', 5126);
  w.json.skins = [{ name: 'p_ex001_0', joints, skeleton: 0, inverseBindMatrices: skinAcc }];
  const prims = []; const stats = { vertices: 0, triangles: 0, primitives: [] };
  for (const pr of lod0.primitives) {
    const idx = rd(pr.indices).data;
    let lo = Infinity, hi = -1; for (const v of idx) { if (v < lo) lo = v; if (v > hi) hi = v; }
    const n = hi - lo + 1;
    const slice = (acc, comps) => rd(acc).data.slice(lo * comps, (hi + 1) * comps);
    const A = pr.attributes;
    const pos = slice(A.POSITION, 3), nor = slice(A.NORMAL, 3), tan = slice(A.TANGENT, 4);
    const P2 = new Array(n * 3), N2 = new Array(n * 3), T2 = new Array(n * 4);
    for (let i = 0; i < n; i++) {
      const p = cvPos(pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]); P2[i * 3] = p[0]; P2[i * 3 + 1] = p[1]; P2[i * 3 + 2] = p[2];
      const nn = cvDir(nor[i * 3], nor[i * 3 + 1], nor[i * 3 + 2]); N2[i * 3] = nn[0]; N2[i * 3 + 1] = nn[1]; N2[i * 3 + 2] = nn[2];
      const tt = cvDir(tan[i * 4], tan[i * 4 + 1], tan[i * 4 + 2]); T2[i * 4] = tt[0]; T2[i * 4 + 1] = tt[1]; T2[i * 4 + 2] = tt[2]; T2[i * 4 + 3] = -tan[i * 4 + 3];
    }
    const jn = slice(A.JOINTS_0, 4), wa = rd(A.WEIGHTS_0), wr = slice(A.WEIGHTS_0, 4);
    if (A.JOINTS_1) throw new Error('more than 4 influences: not handled');
    const div = wa.componentType === 5121 ? 255 : wa.componentType === 5123 ? 65535 : 1;
    const W2 = new Array(n * 4);
    for (let i = 0; i < n; i++) { // float weights, renormalised in double precision so that the float32 sum is 1 within 1 ulp
      let s = 0; for (let c = 0; c < 4; c++) s += wr[i * 4 + c] / div;
      for (let c = 0; c < 4; c++) W2[i * 4 + c] = wr[i * 4 + c] / div / s;
      for (let c = 0; c < 4; c++) jn[i * 4 + c] = W2[i * 4 + c] === 0 ? 0 : jointOf.get(jn[i * 4 + c]); // bone index -> joint index; unused slots point at joint 0
    }
    const attributes = {
      POSITION: w.accessor(P2, 'VEC3', 5126, { minmax: true, target: 34962 }), NORMAL: w.accessor(N2, 'VEC3', 5126, { target: 34962 }), TANGENT: w.accessor(T2, 'VEC4', 5126, { target: 34962 }),
      JOINTS_0: w.accessor(jn, 'VEC4', 5123, { target: 34962 }), WEIGHTS_0: w.accessor(W2, 'VEC4', 5126, { target: 34962 }),
    };
    for (let u = 0; A['TEXCOORD_' + u] !== undefined; u++) attributes['TEXCOORD_' + u] = w.accessor(slice(A['TEXCOORD_' + u], 2), 'VEC2', 5126, { target: 34962 });
    const ix = new Array(idx.length); // the reflection is baked into the vertices, so the winding is reversed here (the source file relies on its mirrored root node for that)
    for (let i = 0; i < idx.length; i += 3) { ix[i] = idx[i] - lo; ix[i + 1] = idx[i + 2] - lo; ix[i + 2] = idx[i + 1] - lo; }
    const indices = w.accessor(ix, 'SCALAR', n > 65535 ? 5125 : 5123, { target: 34963 });
    prims.push({ attributes, indices, mode: 4, material: pr.material });
    stats.vertices += n; stats.triangles += ix.length / 3;
    stats.primitives.push({ material_slot: pr.material, slot_name: j.materials[pr.material].name, vertices: n, triangles: ix.length / 3 });
  }
  w.json.meshes = [{ name: 'p_ex001_0_LOD0', primitives: prims }];
  w.json.materials = materials.map((m) => m.gltf);
  const texList = []; // unique textures in order of use
  for (const m of materials) for (const t of m.textures) if (!texList.includes(t.file)) texList.push(t.file);
  if (texList.length) {
    w.json.samplers = [{ magFilter: 9729, minFilter: 9987, wrapS: 10497, wrapT: 10497 }];
    w.json.images = texList.map((f) => ({ uri: f, name: path.basename(f, '.png') }));
    w.json.textures = texList.map((f, i) => ({ source: i, sampler: 0, name: path.basename(f, '.png') }));
    for (const m of w.json.materials) {
      const fix = (o) => { if (o && typeof o.index === 'string') o.index = texList.indexOf(o.index); };
      fix(m.pbrMetallicRoughness.baseColorTexture); fix(m.normalTexture);
    }
  }
  const node = w.json.nodes.push({ name: 'sora_mesh', mesh: 0, skin: 0 }) - 1;
  stats.skin_joints = joints.length;
  return { node, stats, joints };
}

// One AnimSequence of assets_converted/animations -> channels in pack conventions, targeting bone indices.
function loadAnimation(gamePath, bones) {
  const content = gamePathToContent(gamePath);
  const base = P(ROOT, 'assets_converted', 'animations', CONTENT, content);
  const g = readGlb(base + '.glb'), side = readJson(base + '.anim.json');
  const sp = Object.fromEntries(side.data.props.map((x) => [x.tag.name, x.tag.type === 'BoolProperty' ? x.tag.boolVal === 1 : x.value])); // a bool's value is in its tag
  if (side.class !== 'AnimSequence') throw new Error('not an AnimSequence: ' + gamePath);
  if (sp.AdditiveAnimType && sp.AdditiveAnimType !== 'AAT_None') throw new Error('additive sequence: not handled: ' + gamePath);
  // bones are matched by name: the skeleton asset and the mesh list the same bones (checked in main)
  const a = g.json.animations[0];
  const chans = [];
  const cache = new Map(); const rd = (i) => { if (!cache.has(i)) cache.set(i, readAccessor(g, i)); return cache.get(i); };
  const boneIndex = new Map(bones.map((b, i) => [b.name, i]));
  for (const c of a.channels) {
    const s = a.samplers[c.sampler];
    const bi = boneIndex.get(g.json.nodes[c.target.node].name);
    if (bi === undefined) throw new Error(`animation bone ${g.json.nodes[c.target.node].name} not in the mesh skeleton (${gamePath})`);
    const times = rd(s.input).data, v = rd(s.output).data, out = new Array(v.length);
    if (c.target.path === 'translation') for (let i = 0; i < v.length; i += 3) { const p = cvPos(v[i], v[i + 1], v[i + 2]); out[i] = p[0]; out[i + 1] = p[1]; out[i + 2] = p[2]; }
    else if (c.target.path === 'scale') for (let i = 0; i < v.length; i += 3) { const p = cvScale(v[i], v[i + 1], v[i + 2]); out[i] = p[0]; out[i + 1] = p[1]; out[i + 2] = p[2]; }
    else if (c.target.path === 'rotation') {
      for (let i = 0; i < v.length; i += 4) { // converted, renormalised in float32 terms, and kept on one hemisphere with the previous key
        let q = cvQuat(v[i], v[i + 1], v[i + 2], v[i + 3]); const l = Math.hypot(...q); q = q.map((x) => x / l);
        if (i >= 4 && q[0] * out[i - 4] + q[1] * out[i - 3] + q[2] * out[i - 2] + q[3] * out[i - 1] < 0) q = q.map((x) => -x);
        out[i] = q[0]; out[i + 1] = q[1]; out[i + 2] = q[2]; out[i + 3] = q[3];
      }
    } else throw new Error('channel path ' + c.target.path);
    chans.push({ bone: bi, path: c.target.path, times, values: out, interpolation: s.interpolation || 'LINEAR', ue_values: v });
  }
  // notifies: every FAnimNotifyEvent with full-precision values and the notify object's own serialized properties
  const pkg = loadPkg(content);
  const objRef = (v) => (v ? (v < 0 ? importPath(pkg, v) : (pkg.exports[v - 1] ? pkg.exports[v - 1].objectName : null)) : null);
  // nested values keep their serialized shape; every ObjectProperty inside (struct members, arrays of objects) becomes the object's path
  const deep = (v, type, inner) => {
    if (type === 'ObjectProperty') return typeof v === 'number' ? objRef(v) : v;
    if (Array.isArray(v)) return v.map((x) => (x && typeof x === 'object' && !Array.isArray(x) && 'name' in x && 'type' in x ? { ...x, value: deep(x.value, x.type, x.innerType) } : deep(x, inner === 'ObjectProperty' ? 'ObjectProperty' : null, null)));
    if (v && typeof v === 'object' && Array.isArray(v.items)) return { ...v, items: v.items.map((it) => deep(it, null, null)) };
    return v;
  };
  const plainProps = (e) => Object.fromEntries(((e.data && e.data.props) || []).map((p) => [p.name + (p.arrayIndex ? `[${p.arrayIndex}]` : ''), deep(p.value, p.type, p.innerType)]));
  const notifies = (sp.Notifies ? sp.Notifies.items : []).map((it, order) => {
    const o = Object.fromEntries(it.props.map((x) => [x.tag.name, x.tag.type === 'BoolProperty' ? x.tag.boolVal === 1 : x.value]));
    const objIndex = (o.Notify && o.Notify.object) || (o.NotifyStateClass && o.NotifyStateClass.object) || 0;
    const exp = objIndex > 0 ? pkg.exports[objIndex - 1] : null;
    const isState = !!(o.NotifyStateClass && o.NotifyStateClass.object);
    const time = +(o.LinkValue ?? o.DisplayTime ?? 0), duration = +(o.Duration ?? 0), trig = +(o.TriggerTimeOffset ?? 0), endTrig = +(o.EndTriggerTimeOffset ?? 0);
    const r = { order, name: o.NotifyName, kind: isState ? 'state' : 'instant', time, trigger_time_offset: trig, trigger_time: time + trig };
    if (isState) Object.assign(r, { duration, end_trigger_time_offset: endTrig, end_time: time + duration, end_trigger_time: time + duration + endTrig });
    Object.assign(r, { trigger_weight_threshold: o.TriggerWeightThreshold ?? null, notify_trigger_chance: o.NotifyTriggerChance ?? 1, class: exp ? exp.class : null, object: exp ? exp.objectName : null, properties: exp ? plainProps(exp) : {} });
    return r;
  }).sort((x, y) => x.time - y.time || x.order - y.order);
  const keyCounts = { translation: 0, rotation: 0, scale: 0, total: 0, max_per_channel: 0 };
  for (const c of chans) { keyCounts[c.path] += c.times.length; keyCounts.total += c.times.length; if (c.times.length > keyCounts.max_per_channel) keyCounts.max_per_channel = c.times.length; }
  const numFrames = sp.NumFrames, len = sp.SequenceLength;
  // root motion: what the root bone's own track does (pack units)
  const rootInfo = {};
  for (const bn of ROOT_MOTION_BONES) {
    const bi = boneIndex.get(bn); const tr = chans.find((c) => c.bone === bi && c.path === 'translation'), ro = chans.find((c) => c.bone === bi && c.path === 'rotation');
    if (!tr || !ro) { rootInfo[bn] = { tracked: false }; continue; }
    const first = tr.values.slice(0, 3), last = tr.values.slice(-3); let path_len = 0;
    for (let i = 3; i < tr.values.length; i += 3) path_len += Math.hypot(tr.values[i] - tr.values[i - 3], tr.values[i + 1] - tr.values[i - 2], tr.values[i + 2] - tr.values[i - 1]);
    const q0 = ro.values.slice(0, 4), q1 = ro.values.slice(-4);
    // rotation of the last key relative to the first, as a yaw about +Y (pack), degrees; positive = turning left (towards +X) seen from above
    const inv0 = [-q0[0], -q0[1], -q0[2], q0[3]];
    const qm = (a2, b2) => [a2[3] * b2[0] + a2[0] * b2[3] + a2[1] * b2[2] - a2[2] * b2[1], a2[3] * b2[1] - a2[0] * b2[2] + a2[1] * b2[3] + a2[2] * b2[0], a2[3] * b2[2] + a2[0] * b2[1] - a2[1] * b2[0] + a2[2] * b2[3], a2[3] * b2[3] - a2[0] * b2[0] - a2[1] * b2[1] - a2[2] * b2[2]];
    const dq = qm(q1, inv0); // parent-space delta: q1 * q0^-1
    const yaw = 2 * Math.atan2(dq[1], dq[3]) * 180 / Math.PI;
    rootInfo[bn] = { tracked: true, translation_keys: tr.times.length, rotation_keys: ro.times.length,
      translation_delta_m: last.map((x, i) => +(x - first[i]).toFixed(5)), translation_path_length_m: +path_len.toFixed(5),
      rotation_delta_yaw_deg: +(((yaw + 540) % 360) - 180).toFixed(3), rotation_delta_quat: dq.map((x) => +x.toFixed(6)) };
  }
  const rk = rootInfo.kh_sk;
  const moves = rk.tracked && (rk.translation_path_length_m > 0.005 || Math.abs(rk.rotation_delta_yaw_deg) > 0.5 || rk.rotation_keys > 1);
  return {
    content, sp, chans, notifies, numFrames, length: len, fps: numFrames > 1 ? +((numFrames - 1) / len).toFixed(4) : null,
    props: { bEnableRootMotion: sp.bEnableRootMotion ?? false, RootMotionRootLock: sp.RootMotionRootLock ?? 'ERootMotionRootLock::RefPose', bForceRootLock: sp.bForceRootLock ?? false, RateScale: sp.RateScale ?? 1,
      m_InterpolationTime: sp.m_InterpolationTime ?? 0, Interpolation: sp.Interpolation ?? 'EAnimInterpolationType::Linear', m_DisableInterpolation: sp.m_DisableInterpolation ?? false,
      defaults_note: 'values of properties the sequence does not serialize are the class defaults of the constructors 0x143F5A030 / 0x143F31D90' },
    tracks: new Set(chans.map((c) => c.bone)).size, root: rootInfo, root_track_moves: moves, key_encoding: side.data.keyEncodingFormat ?? null, keyCounts,
    tracked_bones: [...new Set(chans.map((c) => c.bone))].sort((x, y) => x - y),
  };
}
function addAnimation(w, name, anim) {
  const timeAcc = new Map();
  const samplers = [], channels = [];
  for (const c of anim.chans) {
    const key = c.times.join(',');
    if (!timeAcc.has(key)) timeAcc.set(key, w.accessor(c.times, 'SCALAR', 5126, { minmax: true }));
    samplers.push({ input: timeAcc.get(key), output: w.accessor(c.values, c.path === 'rotation' ? 'VEC4' : 'VEC3', 5126), interpolation: c.interpolation });
    channels.push({ sampler: samplers.length - 1, target: { node: c.bone, path: c.path } });
  }
  (w.json.animations = w.json.animations || []).push({ name, samplers, channels });
}

// ------------------------------------------------------------------------------------------------ 5. validation
async function validateGlb(file) {
  const validator = require('gltf-validator');
  const dir = path.dirname(file);
  const rep = await validator.validateBytes(new Uint8Array(fs.readFileSync(file)), { uri: path.basename(file), maxIssues: 0, writeTimestamp: false,
    externalResourceFunction: (uri) => new Promise((res, rej) => { try { res(new Uint8Array(fs.readFileSync(path.join(dir, decodeURIComponent(uri))))); } catch (e) { rej(String(e)); } }) });
  const codes = {};
  for (const m of rep.issues.messages) { const k = ['error', 'warning', 'info', 'hint'][m.severity] + ':' + m.code; codes[k] = (codes[k] || 0) + 1; }
  const first = rep.issues.messages.filter((m) => m.severity < 2).slice(0, 5).map((m) => `${m.code} ${m.pointer || ''} ${m.message}`);
  return { errors: rep.issues.numErrors, warnings: rep.issues.numWarnings, infos: rep.issues.numInfos, hints: rep.issues.numHints, codes, first };
}
// Independent reading of a pack glb: skin data and animation data.
function checkGlb(file, expectBones) {
  const g = readGlb(file), j = g.json, r = { file: path.basename(file) };
  const parent = new Map(); j.nodes.forEach((n, i) => (n.children || []).forEach((c) => parent.set(c, i)));
  const glob = []; const G = (i) => { if (!glob[i]) { const n = j.nodes[i]; const l = trsMatrix(n.translation || [0, 0, 0], n.rotation || [0, 0, 0, 1], n.scale || [1, 1, 1]); glob[i] = parent.has(i) ? mul(G(parent.get(i)), l) : l; } return glob[i]; };
  if (expectBones) { r.bones = expectBones.length; r.bone_names_match = expectBones.every((b, i) => j.nodes[i].name === b.name && (parent.has(i) ? parent.get(i) : -1) === b.parent); }
  if (j.skins) {
    const sk = j.skins[0], ibm = readAccessor(g, sk.inverseBindMatrices).data; let worst = 0;
    sk.joints.forEach((jn, k) => { const m = mul(G(jn), ibm.slice(k * 16, k * 16 + 16)); for (let i = 0; i < 16; i++) worst = Math.max(worst, Math.abs(m[i] - (i % 5 === 0 ? 1 : 0))); });
    r.ibm_times_bind_max_dev_from_identity = +worst.toExponential(2);
    let verts = 0, maxSumDev = 0, badJoint = 0, zeroWeightNonZeroJoint = 0, maxInfl = 0, unnormal = 0, bindDev = 0; const used = new Set();
    const mn = [Infinity, Infinity, Infinity], mx = [-Infinity, -Infinity, -Infinity]; let tris = 0, degenerate = 0, idxBad = 0;
    for (const p of j.meshes[0].primitives) {
      const pos = readAccessor(g, p.attributes.POSITION).data, nor = readAccessor(g, p.attributes.NORMAL).data, jn = readAccessor(g, p.attributes.JOINTS_0).data, wt = readAccessor(g, p.attributes.WEIGHTS_0).data, ix = readAccessor(g, p.indices).data;
      const n = pos.length / 3; verts += n;
      for (let i = 0; i < n; i++) {
        let s = 0, infl = 0; const sp = [0, 0, 0]; const pt = [pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]];
        for (let c = 0; c < 4; c++) {
          const wgt = wt[i * 4 + c], b = jn[i * 4 + c]; s += wgt;
          if (b < 0 || b >= sk.joints.length) badJoint++;
          if (wgt > 0) { infl++; used.add(b); const q = xform(mul(G(sk.joints[b]), ibm.slice(b * 16, b * 16 + 16)), pt); sp[0] += wgt * q[0]; sp[1] += wgt * q[1]; sp[2] += wgt * q[2]; } else if (b !== 0) zeroWeightNonZeroJoint++;
        }
        maxSumDev = Math.max(maxSumDev, Math.abs(s - 1)); maxInfl = Math.max(maxInfl, infl);
        bindDev = Math.max(bindDev, Math.hypot(sp[0] - pt[0], sp[1] - pt[1], sp[2] - pt[2]));
        if (Math.abs(Math.hypot(nor[i * 3], nor[i * 3 + 1], nor[i * 3 + 2]) - 1) > 1e-3) unnormal++;
        for (let c = 0; c < 3; c++) { mn[c] = Math.min(mn[c], pt[c]); mx[c] = Math.max(mx[c], pt[c]); }
      }
      for (let i = 0; i < ix.length; i += 3) { tris++; if (ix[i] >= n || ix[i + 1] >= n || ix[i + 2] >= n) idxBad++; if (ix[i] === ix[i + 1] || ix[i + 1] === ix[i + 2] || ix[i] === ix[i + 2]) degenerate++; }
    }
    // facing and winding: summed area-weighted face normal agreement with vertex normals; nose ahead of the head bone; left hand on +X
    let agree = 0, against = 0;
    for (const p of j.meshes[0].primitives) {
      const pos = readAccessor(g, p.attributes.POSITION).data, nor = readAccessor(g, p.attributes.NORMAL).data, ix = readAccessor(g, p.indices).data;
      for (let i = 0; i < ix.length; i += 3) {
        const a = ix[i] * 3, b = ix[i + 1] * 3, c = ix[i + 2] * 3;
        const e1 = [pos[b] - pos[a], pos[b + 1] - pos[a + 1], pos[b + 2] - pos[a + 2]], e2 = [pos[c] - pos[a], pos[c + 1] - pos[a + 1], pos[c + 2] - pos[a + 2]];
        const fn = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        const d = fn[0] * nor[a] + fn[1] * nor[a + 1] + fn[2] * nor[a + 2];
        if (d > 0) agree++; else if (d < 0) against++;
      }
    }
    const bonePos = (name) => { const i = j.nodes.findIndex((n) => n.name === name); return i < 0 ? null : G(i).slice(12, 15); };
    const nose = bonePos('C_nose'), head = bonePos('atama'), lh = bonePos('L_te') || bonePos('L_ude2'), rh = bonePos('R_te') || bonePos('R_ude2');
    Object.assign(r, { vertices: verts, triangles: tris, weight_sum_max_dev: +maxSumDev.toExponential(2), joints_out_of_range: badJoint, zero_weight_with_nonzero_joint: zeroWeightNonZeroJoint, max_influences: maxInfl,
      bones_with_weights: used.size, non_unit_normals: unnormal, index_out_of_range: idxBad, degenerate_triangles: degenerate, bind_pose_skinning_max_dev_m: +bindDev.toExponential(2),
      bounds_m: { min: mn.map((x) => +x.toFixed(4)), max: mx.map((x) => +x.toFixed(4)) }, ccw_triangles_agreeing_with_normals: agree, triangles_against_normals: against,
      nose_minus_head_m: nose && head ? nose.map((x, i) => +(x - head[i]).toFixed(4)) : null, left_hand_x_m: lh ? +lh[0].toFixed(4) : null, right_hand_x_m: rh ? +rh[0].toFixed(4) : null,
      weighted_bone_names: [...used].sort((a, b) => a - b).map((b) => j.nodes[sk.joints[b]].name) });
  }
  if (j.animations) {
    let ch = 0, keys = 0, badQuat = 0, nonFinite = 0, timeBad = 0, maxT = 0;
    for (const a of j.animations) for (const c of a.channels) {
      const s = a.samplers[c.sampler], t = readAccessor(g, s.input).data, v = readAccessor(g, s.output).data; ch++; keys += t.length;
      for (let i = 0; i < t.length; i++) { if (i && !(t[i] > t[i - 1])) timeBad++; if (t[i] > maxT) maxT = t[i]; }
      if (t[0] !== 0) timeBad++;
      for (const x of v) if (!Number.isFinite(x)) nonFinite++;
      if (c.target.path === 'rotation') for (let i = 0; i < v.length; i += 4) if (Math.abs(Math.hypot(v[i], v[i + 1], v[i + 2], v[i + 3]) - 1) > 1e-5) badQuat++;
      if (j.nodes[c.target.node].mesh !== undefined) timeBad++;
    }
    Object.assign(r, { animations: j.animations.length, channels: ch, keys, non_unit_quaternions: badQuat, non_finite: nonFinite, time_errors: timeBad, max_time: +maxT.toFixed(6) });
  }
  return r;
}

// ------------------------------------------------------------------------------------------------ root motion, feet
function sampleChan(c, t) { // linear / normalised-linear sample of one channel at time t
  const n = c.path === 'rotation' ? 4 : 3, T = c.times;
  let k = 0; while (k < T.length - 2 && T[k + 1] <= t) k++;
  const k2 = Math.min(k + 1, T.length - 1), f = T.length > 1 && T[k2] > T[k] ? Math.min(1, Math.max(0, (t - T[k]) / (T[k2] - T[k]))) : 0;
  return (vals) => { const v = []; for (let i = 0; i < n; i++) v.push(vals[k * n + i] * (1 - f) + vals[k2 * n + i] * f); if (n === 4) { const l = Math.hypot(...v); return v.map((x) => x / l); } return v; };
}
const qmul = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const r5 = (x) => +x.toFixed(5);
function rootTrackOf(a, bones, meshYawDeg) { // per frame, relative to frame 0
  const tr = a.chans.find((c) => c.bone === 0 && c.path === 'translation'), ro = a.chans.find((c) => c.bone === 0 && c.path === 'rotation');
  const frames = [];
  const cy = Math.cos(meshYawDeg * Math.PI / 180), sy = Math.sin(meshYawDeg * Math.PI / 180);
  let t0, q0, prevYaw = 0;
  for (let i = 0; i < a.numFrames; i++) {
    const t = a.numFrames > 1 ? i * a.length / (a.numFrames - 1) : 0;
    const p = sampleChan(tr, t)(tr.ue_values), q = sampleChan(ro, t)(ro.ue_values);
    if (i === 0) { t0 = p; q0 = q; }
    const d = [p[0] - t0[0], p[1] - t0[1], p[2] - t0[2]]; // UE cm, mesh-component space
    const dq = qmul(q, [-q0[0], -q0[1], -q0[2], q0[3]]); // rotation since frame 0, in the parent (component) space
    let yaw = 2 * Math.atan2(dq[2], dq[3]) * 180 / Math.PI; // UE yaw about +Z: positive turns +X towards +Y (to the right, seen from above: clockwise)
    while (yaw - prevYaw > 180) yaw -= 360; while (yaw - prevYaw < -180) yaw += 360; prevYaw = yaw;
    const actor = [d[0] * cy - d[1] * sy, d[0] * sy + d[1] * cy, d[2]]; // rotated by the mesh component's yaw into actor space
    const pk = cvPos(...d);
    frames.push({ frame: i, time: r5(t), translation_ue_mesh_cm: d.map(r5), translation_ue_actor_cm: actor.map(r5), yaw_ue_deg: r5(yaw), translation_pack_m: pk.map(r5), yaw_pack_deg: r5(-yaw), tilt: r5(Math.hypot(dq[0], dq[1])) });
  }
  return frames;
}
function buildRootMotion(anims, bones, logical, id, stage1) {
  const meshYaw = id.mesh_component.relative_rotation_ue_deg.yaw;
  const byName = new Map([...anims.entries()].map(([gp, a]) => [path.basename(a.content), a]));
  const out = {
    note: 'Track of the root bone kh_sk per animation frame, relative to frame 0 (sampled from the converted keys at t = frame * SequenceLength / (NumFrames - 1)). translation_ue_mesh_cm is in the mesh component\'s space (character faces +Y); translation_ue_actor_cm is the same vector in actor space (mesh component yaw applied: actor forward +X, right +Y); yaw_ue_deg is the UE yaw of the root since frame 0 (positive = turning right / clockwise seen from above; identical in mesh and actor space). Pack values: metres, forward +Z, left +X, yaw about +Y with positive = turning left (yaw_pack_deg = -yaw_ue_deg). tilt = sine of half the non-yaw part of the rotation (0 = pure yaw). The game extracts this motion only from sequences with bEnableRootMotion = true played with the root-motion mode bit.',
    mesh_component_yaw_ue_deg: meshYaw, tracks: {}, clips_without_root_motion: {},
    tracks_not_extracted_note: 'clips outside stage 1 whose root bone moves but whose bEnableRootMotion is false: the game extracts no root motion from them (the motion stays in the pose); summary only, no per-frame data',
    tracks_not_extracted: {} };
  for (const [name, a] of [...byName.entries()].sort()) {
    const fr = rootTrackOf(a, bones, meshYaw), last = fr[fr.length - 1];
    const maxT = Math.max(...fr.map((x) => Math.hypot(...x.translation_ue_mesh_cm))), maxY = Math.max(...fr.map((x) => Math.abs(x.yaw_ue_deg)));
    const tr = a.chans.find((c) => c.bone === 0 && c.path === 'translation'), ro = a.chans.find((c) => c.bone === 0 && c.path === 'rotation');
    const sum = { bEnableRootMotion: a.props.bEnableRootMotion === true, root_translation_keys: tr.times.length, root_rotation_keys: ro.times.length, frames: a.numFrames, duration_s: a.length,
      total_translation_ue_actor_cm: last.translation_ue_actor_cm, total_yaw_ue_deg: last.yaw_ue_deg, max_translation_from_start_cm: r5(maxT), max_abs_yaw_deg: r5(maxY),
      logical_names: logical.filter((l) => path.basename(gamePathToContent(l.asset)) === name || (l.samples || []).some((x) => path.basename(gamePathToContent(x.asset)) === name)).map((l) => l.name + (l.samples ? '@' + l.samples.find((x) => path.basename(gamePathToContent(x.asset)) === name).value : '')) };
    if ((maxT > 1e-4 || maxY > 1e-3) && !sum.bEnableRootMotion && !stage1.has(name)) out.tracks_not_extracted[name] = sum;
    else if (maxT > 1e-4 || maxY > 1e-3) out.tracks[name] = { ...sum, frames_data: fr };
    else out.clips_without_root_motion[name] = { ...sum, statement: 'the root bone track is constant: no translation, no rotation' };
  }
  return out;
}
// Lowest mesh points at the reference pose and at frame 0 of the idle animation (UE cm above the mesh origin = above the actor origin).
function footHeight(meshGlb, bones, idle) {
  const j = meshGlb.json, lod0 = j.meshes[0];
  const bind = globalBind(bones), ibm = bind.map(invertAffine);
  const posed = bones.map((b) => ({ t: b.translation.map(f32), r: b.rotation.map(f32), s: b.scale.map(f32) }));
  for (const c of idle.chans) posed[c.bone][{ translation: 't', rotation: 'r', scale: 's' }[c.path]] = c.values.slice(0, c.path === 'rotation' ? 4 : 3);
  const g = []; bones.forEach((b, i) => { const l = trsMatrix(posed[i].t, posed[i].r, posed[i].s); g.push(b.parent < 0 ? l : mul(g[b.parent], l)); });
  const skinM = g.map((m, i) => mul(m, ibm[i]));
  const res = { bind: { all: Infinity, left: Infinity, right: Infinity }, idle0: { all: Infinity, left: Infinity, right: Infinity } };
  const lowBones = { bind: new Map(), idle0: new Map() };
  for (const pr of lod0.primitives) {
    const P0 = readAccessor(meshGlb, pr.attributes.POSITION).data, J = readAccessor(meshGlb, pr.attributes.JOINTS_0).data, Wa = readAccessor(meshGlb, pr.attributes.WEIGHTS_0), Wt = Wa.data;
    const div = Wa.componentType === 5121 ? 255 : Wa.componentType === 5123 ? 65535 : 1;
    for (let i = 0; i < P0.length / 3; i++) {
      const p = cvPos(P0[i * 3], P0[i * 3 + 1], P0[i * 3 + 2]); let s = 0; const q = [0, 0, 0]; let top = 0, topB = 0;
      for (let c = 0; c < 4; c++) { const wv = Wt[i * 4 + c] / div; if (!wv) continue; s += wv; const x = xform(skinM[J[i * 4 + c]], p); q[0] += wv * x[0]; q[1] += wv * x[1]; q[2] += wv * x[2]; if (wv > top) { top = wv; topB = J[i * 4 + c]; } }
      q[0] /= s; q[1] /= s; q[2] /= s;
      for (const [k, v] of [['bind', p], ['idle0', q]]) { const side = v[0] > 0 ? 'left' : 'right'; if (v[1] < res[k].all) { res[k].all = v[1]; lowBones[k].set('all', bones[topB].name); } if (v[1] < res[k][side]) { res[k][side] = v[1]; lowBones[k].set(side, bones[topB].name); } }
    }
  }
  const cm = (o, k) => ({ lowest_vertex_z_ue_cm: r5(o.all * 100), left_foot_lowest_z_ue_cm: r5(o.left * 100), right_foot_lowest_z_ue_cm: r5(o.right * 100), dominant_bone_of_lowest_vertex: Object.fromEntries(lowBones[k]) });
  return { note: 'Height of the lowest mesh vertices above the mesh origin, which is the actor origin (mesh component location 0). UE cm, +Z up; negative = below the origin. Left/right = the side of the body (+X / -X in the pack). Skinned on the CPU from LOD 0 with the bones posed by the first key of every track of IdleN (0001); bones without a track at the reference pose. The game additionally runs leg IK (not reproduced).',
    reference_pose: cm(res.bind, 'bind'), idle_frame_0: cm(res.idle0, 'idle0') };
}

// ------------------------------------------------------------------------------------------------ attack sockets
// For every clip with a TresAnimNotifyState_Attack: where the keyblade's hit sockets ATK_00 / ATK_01 are, per animation
// frame, in ACTOR space (UE cm; x forward, y right, z up; origin = the actor origin), from the clip's own pose: bone
// R_buki -> socket R_wep_skt -> the weapon's root bone and bone wep at the weapon's reference pose -> the socket. The
// engine-free simulation sweeps its hit shapes along these tracks (docs/sora-combat-spec.md 8.1; the attack clips blend in
// over 0 s, so the clip pose is the shown pose; leg IK, KineDriver and Bonamik do not write the right arm). The root bone
// is at its reference transform for clips with bEnableRootMotion (the pose's root lock), else follows its track.
function attackSockets(anims, bones, id) {
  const wf = P(OUT, 'weapon', 'manifest.json');
  if (!fs.existsSync(wf)) return { note: 'build/sora_pack/weapon/manifest.json does not exist (node tools/sora_weapon_pack.js): no socket tracks', clips: {} };
  const wm = readJson(wf);
  const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
  const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
  // child (q, t, s) under parent (q, t, s): UE FTransform product child * parent
  const under = (c, p) => { const v = qrot(p.q, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { q: qmulH(p.q, c.q), t: [v[0] + p.t[0], v[1] + p.t[1], v[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
  const hand = bones.findIndex((b) => b.name === wm.attach.parent.bone);
  if (hand < 0) throw new Error('attack sockets: Sora has no bone ' + wm.attach.parent.bone);
  const chain = []; for (let k = hand; k >= 0; k = bones[k].parent) chain.unshift(k);
  const sock = wm.attach.socket_relative_to_bone.ue;
  const socket = { q: sock.quaternion_xyzw, t: sock.location_cm, s: sock.scale };
  const wb = wm.skeleton.bones;
  const wbone = (name) => { const b = wb.find((x) => x.name === name); if (!b) throw new Error('attack sockets: the weapon has no bone ' + name); return { q: b.rotation_ue, t: b.translation_ue, s: b.scale_ue, parent: b.parent }; };
  const wroot = wbone(wb[0].name), wep = wbone('wep');
  if (wep.parent !== 0) throw new Error('attack sockets: weapon bone wep is not a child of the root');
  const sk = (name) => { const x = wm.sockets.sockets.find((y) => y.name === name); if (!x || x.bone !== 'wep') throw new Error('attack sockets: weapon socket ' + name); return x.relative_location_ue_cm; };
  const a0 = sk('ATK_00'), a1 = sk('ATK_01');
  const yaw = id.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cy = Math.cos(yaw), sy = Math.sin(yaw);
  const actor = (v) => [v[0] * cy - v[1] * sy, v[0] * sy + v[1] * cy, v[2]];
  const point = (T, v) => { const r = qrot(T.q, [T.s[0] * v[0], T.s[1] * v[1], T.s[2] * v[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
  const weaponAt = (a, time) => {
    const byBone = new Map(); for (const c of a.chans) { if (!byBone.has(c.bone)) byBone.set(c.bone, {}); byBone.get(c.bone)[c.path] = c; }
    let T = null;
    for (const k of chain) {
      const b = bones[k], tr = (k === 0 && a.props.bEnableRootMotion === true) ? {} : (byBone.get(k) || {});
      const l = { q: tr.rotation ? sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale };
      T = T ? under(l, T) : l;
    }
    return under(wep, under(wroot, under(socket, T)));
  };
  const out = { note: 'ATK_00 and ATK_01 of the keyblade per animation frame (time = frame * SequenceLength / (NumFrames - 1)), ACTOR space, UE cm: [x0, y0, z0, x1, y1, z1]. Clips with a TresAnimNotifyState_Attack or a TresAnimNotify_Shot, and the Kingdom Key shotlock clips. Weapon at its reference pose on R_wep_skt.',
    weapon: wm.weapon, attach_bone: wm.attach.parent.bone, atk_00_on_wep_cm: a0, atk_01_on_wep_cm: a1, clips: {} };
  for (const a of anims.values()) {
    // (magic implementer, 1.5.2: also the clips with a TresAnimNotify_Shot - the cast clips, whose muzzle ATK_01 the spell
    // spawns from - and the Kingdom Key shotlock clips, whose blade tip is the volley origin: docs/sora-magic-spec.md A 1.4, G 1.5)
    if (!a.notifies.some((n) => n.class === 'TresAnimNotifyState_Attack' || n.class === 'TresAnimNotify_Shot') && !/^(1473|1474|1481|1482|13[2-5][0-9][DU])$/.test(path.basename(a.content))) continue;
    const frames = [];
    for (let i = 0; i < a.numFrames; i++) { const t = a.numFrames > 1 ? i * a.length / (a.numFrames - 1) : 0; const W = weaponAt(a, t); frames.push([...actor(point(W, a0)), ...actor(point(W, a1))].map((x) => +x.toFixed(4))); }
    out.clips[path.basename(a.content)] = { frames: a.numFrames, duration_s: a.length, sockets: frames };
  }
  // the wielder's own socket vfx_cure (shape groups FrontImpactS, KB_FixPillar): a socket of the root bone
  { const d = loadPkg(gamePathToContent(id.skeleton)); const e = d.exports.find((x) => x.class === 'SkeletalMeshSocket' && propsOf(x).SocketName && propsOf(x).SocketName.value === 'vfx_cure');
    if (e) { const p = propsOf(e); const b = bones.find((x) => x.name === p.BoneName.value); const loc = p.RelativeLocation ? p.RelativeLocation.value : { x: 0, y: 0, z: 0 }; const r = p.RelativeRotation ? p.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 };
      const h = Math.PI / 360, SP = Math.sin(r.pitch * h), CP = Math.cos(r.pitch * h), SY = Math.sin(r.yaw * h), CY = Math.cos(r.yaw * h), SR = Math.sin(r.roll * h), CR = Math.cos(r.roll * h);
      const q = [CR * SP * SY - SR * CP * CY, -CR * SP * CY - SR * CP * SY, CR * CP * SY - SR * SP * CY, CR * CP * CY + SR * SP * SY];
      if (b && b.parent === -1) { const T = under({ q, t: [loc.x, loc.y, loc.z], s: [1, 1, 1] }, { q: b.ue.rotation, t: b.ue.translation, s: b.ue.scale });
        out.vfx_cure = { bone: b.name, relative_location_ue_cm: [loc.x, loc.y, loc.z], relative_rotation_ue_deg: r, position_actor_cm: actor(point(T, [0, 0, 0])).map((x) => +x.toFixed(4)), plus_200_x_actor_cm: actor(point(T, [200, 0, 0])).map((x) => +x.toFixed(4)), note: 'root bone at its reference transform; FRotator -> quaternion by the stock UE formula. plus_200_x = the socket point moved by the shapes\' m_RelativeLocation1 (200, 0, 0) in the socket frame' }; } } }
  return out;
}

// ------------------------------------------------------------------------------------------------ KineDriver check
// 1. On the reference pose the driver must reproduce the reference transform of every bone it writes (except R_susoB1..3,
//    whose reference pose differs from the driver's neutral by 1 degree in the asset itself).
// 2. Some driver-written helper bones have baked tracks in the clips: at every key time the driver's output must equal them.
function kineDriverCheck(data, bones, anims) {
  const angle = (a, b) => { const d = Math.abs(a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]); return 2 * Math.acos(Math.min(1, d)) * 180 / Math.PI; };
  const diff = (A, B) => ({ t: Math.hypot(A.t[0] - B.t[0], A.t[1] - B.t[1], A.t[2] - B.t[2]), r: angle(A.r, B.r), s: Math.max(...[0, 1, 2].map((i) => Math.abs(A.s[i] - B.s[i]))) });
  const refLocal = () => bones.map((b) => ({ r: b.ue.rotation.slice(), t: b.ue.translation.slice(), s: b.ue.scale.slice() }));
  const run = (local) => { const base = kd.componentSpace(bones, local); const pose = base.map((x) => ({ r: x.r.slice(), t: x.t.slice(), s: x.s.slice() })); kd.evaluate(data, kd.createState(data), pose, local); return { base, pose }; };
  const TOL = { t: 1e-3, r: 1e-2, s: 1e-4 };
  const ref = run(refLocal()); const bad = []; const mx = { t: 0, r: 0, s: 0 };
  for (const b of data.bones) { if (!b.written) continue; const d = diff(ref.base[b.index], ref.pose[b.index]); if (d.t > TOL.t || d.r > TOL.r || d.s > TOL.s) bad.push(b.name); else for (const k of 'trs') mx[k] = Math.max(mx[k], d[k]); }
  const expected = ['R_susoB1', 'R_susoB2', 'R_susoB3'];
  const out = { reference_pose: { written_bones: data.bones.filter((b) => b.written).length, tolerance: { translation_cm: TOL.t, rotation_deg: TOL.r, scale: TOL.s }, bones_outside_tolerance: bad, expected_outside: expected, max_error_of_the_rest: { translation_cm: +mx.t.toExponential(2), rotation_deg: +mx.r.toExponential(2), scale: +mx.s.toExponential(2) }, ok: bad.length === expected.length && expected.every((n) => bad.includes(n)) }, baked_tracks: { tolerance: { translation_cm: 1e-2, rotation_deg: 2e-2, scale: 1e-3, note: 'times 10 for clips with key-reduced channels (the comparison then falls between keys of some channels)' }, clips: {}, ok: true } };
  for (const a of anims.values()) {
    const byBone = new Map(); for (const c of a.chans) { if (!byBone.has(c.bone)) byBone.set(c.bone, {}); byBone.get(c.bone)[c.path] = c; }
    let keyTimes = []; for (const c of a.chans) if (c.times.length > keyTimes.length) keyTimes = c.times;
    const m = { t: 0, r: 0, s: 0 }; let n = 0;
    for (const time of keyTimes) {
      const local = bones.map((b, i) => { const tr = byBone.get(i) || {}; return { r: tr.rotation ? sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation.slice(), t: tr.translation ? sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation.slice(), s: tr.scale ? sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale.slice() }; });
      const r = run(local);
      for (const b of data.bones) { if (!b.written || !byBone.has(b.index)) continue; const d = diff(r.base[b.index], r.pose[b.index]); for (const k of 'trs') m[k] = Math.max(m[k], d[k]); n++; }
    }
    // clips whose channels were key-reduced to different key times are compared between keys of some channels: looser tolerance there
    const reduced = a.chans.some((c) => c.times.length > 1 && c.times.length !== keyTimes.length); const f = reduced ? 10 : 1;
    const ok = m.t <= 1e-2 * f && m.r <= 2e-2 * f && m.s <= 1e-3 * f;
    out.baked_tracks.clips[path.basename(a.content)] = { key_times: keyTimes.length, key_reduced_channels: reduced, tolerance_factor: f, comparisons: n, max_translation_cm: +m.t.toExponential(2), max_rotation_deg: +m.r.toExponential(2), max_scale: +m.s.toExponential(2), ok };
    if (!ok) out.baked_tracks.ok = false;
  }
  out.ok = out.reference_pose.ok && out.baked_tracks.ok;
  return out;
}

// ------------------------------------------------------------------------------------------------ Bonamik check
// The static validations of the reference evaluator (tools/lib/bonamik/validate.js), with their expected numbers:
// rest stability on the reference pose and on IdleN frame 0, the driven-bone set, determinism, and identical 30 Hz
// evaluation states at 45 / 60 / 120 / 144 fps. Also derives the facts the manifest lists.
function bonamikCheck(data, bones, idle, fall) {
  const fr = Math.fround;
  const parents = bones.map((b) => b.parent), names = bones.map((b) => b.name);
  const qm = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
  const qr = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
  const qn = (q) => { const l = Math.hypot(...q); return q.map((x) => x / l); };
  const cs = (local) => { const o = []; for (let i = 0; i < bones.length; i++) { const l = local[i], pi = bones[i].parent; if (pi < 0) o.push({ q: l.q.slice(), t: l.t.slice(), s: l.s.slice() }); else { const Pp = o[pi]; const rt = qr(Pp.q, [Pp.s[0] * l.t[0], Pp.s[1] * l.t[1], Pp.s[2] * l.t[2]]); o.push({ q: qn(qm(Pp.q, l.q)), t: [rt[0] + Pp.t[0], rt[1] + Pp.t[1], rt[2] + Pp.t[2]], s: [l.s[0] * Pp.s[0], l.s[1] * Pp.s[1], l.s[2] * Pp.s[2]] }); } } return o.map((x) => ({ q: x.q.map(fr), t: x.t.map(fr), s: x.s.map(fr) })); };
  const refLocal = () => bones.map((b) => ({ q: b.ue.rotation.slice(), t: b.ue.translation.slice(), s: b.ue.scale.slice() }));
  const clipPose = (a, time) => { const byBone = new Map(); for (const c of a.chans) { if (!byBone.has(c.bone)) byBone.set(c.bone, {}); byBone.get(c.bone)[c.path] = c; } return cs(bones.map((b, i) => { const tr = byBone.get(i) || {}; return { q: tr.rotation ? sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation.slice(), t: tr.translation ? sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation.slice(), s: tr.scale ? sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale.slice() }; })); };
  const clone = (p) => p.map((b) => ({ q: b.q.slice(), t: b.t.slice(), s: b.s.slice() }));
  const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
  const ref = cs(refLocal()), idle0 = clipPose(idle, 0);
  const mk = (o) => bnm.create(Object.assign({ refPose: clone(ref), parents, boneNames: names, data }, o));
  const hash = (sim) => { let h = 0x811c9dc5; const buf = new Float32Array(1), u = new Uint32Array(buf.buffer); for (const p of sim.particles()) for (const v of [...p.f0, ...p.f40, ...p.f20]) { buf[0] = v; h = Math.imul(h ^ u[0], 0x01000193) >>> 0; } return h; };
  function rest(pose, o, n) {
    const sim = mk(o); const Pp = sim.core.f3778.f0; const dyn = Pp.filter((p) => !p.f276); let prev = null, out; const hist = [];
    for (let i = 0; i < n; i++) { out = sim.frame(1 / 30, clone(pose)); const cur = dyn.map((p) => p.f0.slice()); if (prev) hist.push(Math.max(...cur.map((c, k) => dist(c, prev[k])))); prev = cur; }
    let maxNode = 0; for (const p of dyn) { const B = pose[p.f248]; const r = qr(B.q, [B.s[0] * p.f170[0], B.s[1] * p.f170[1], B.s[2] * p.f170[2]]); maxNode = Math.max(maxNode, dist(p.f0, [r[0] + B.t[0], r[1] + B.t[1], r[2] + B.t[2]])); }
    const w = sim.core.f3810; let driven = 0, unchanged = 0, changedUndriven = 0, maxOff = 0;
    for (let i = 0; i < pose.length; i++) { if (w[i] > 0) { driven++; maxOff = Math.max(maxOff, dist(out.pose[i].t, pose[i].t)); } else { if (out.pose[i].t.some((v, k) => v !== fr(pose[i].t[k])) || out.pose[i].q.some((v, k) => v !== fr(pose[i].q[k]))) changedUndriven++; else unchanged++; } }
    return { sim, settle_evaluation: hist.findIndex((x) => x < 1e-3) + 1, max_node_distance_cm: +maxNode.toFixed(3), max_bone_offset_cm: +maxOff.toFixed(3), driven, unchanged, changedUndriven, state_hash: hash(sim) };
  }
  const N = 120;
  const a = rest(ref, { zeroJitter: true }, N), b = rest(idle0, { zeroJitter: true }, N), bSeed = rest(idle0, {}, N), bHard = rest(idle0, { zeroJitter: true, hardResetAtStart: true }, N);
  // determinism and frame-rate independence on the idle clip
  const poseAt = (k) => clipPose(idle, (k / 30) % idle.length);
  const poses = []; for (let k = 0; k < 90; k++) poses.push(poseAt(k));
  const runEval = (fps, nEval, o) => { const sim = mk(o); const hs = []; let k = 0, frames = 0; while (hs.length < nEval) { const out = sim.frame(1 / fps, clone(poses[k])); frames++; if (out.didEval) { hs.push(hash(sim)); k++; } if (frames > 100000) throw new Error('Bonamik: no evaluation'); } return hs; };
  const base = runEval(30, 90, { zeroJitter: true }), again = runEval(30, 90, { zeroJitter: true });
  const rates = {}; for (const fps of [45, 60, 120, 144]) { const h = runEval(fps, 90, { zeroJitter: true }); const d = h.findIndex((x, i) => x !== base[i]); rates[fps] = d < 0 ? 'identical' : 'differs from evaluation ' + d; }
  // clip notifies with an effect (group names that exist in the asset)
  const groupNames = data.groups.map((g) => g.name.toLowerCase());
  const clipNotifies = {};
  for (const [clip, v] of Object.entries(data.notify_states || {})) {
    const eff = [];
    for (const n of v.notifies || []) {
      if (/ChangeGroupParams/.test(n.class)) { const rows = (n.props.m_GroupParams || []).map((r) => ({ groups: r.m_Groups, groups_present: r.m_Groups.filter((g) => groupNames.includes(String(g).toLowerCase())), parameter: String(r.m_Parameter).replace(/^ESQEX_Bonamik_Parameters_/, ''), value: r.m_Value })).filter((r) => r.groups_present.length || r.groups.length === 0); if (rows.length) eff.push({ class: n.class, start: n.start, end: n.end, rows, begin: 'set the value on every joint of the named groups (InnerConeStr / OuterConeStr: on the cone record owned by the joint), immediately', end_effect: 'restore the asset default of the same rows' }); }
      else if (/CharaWind/.test(n.class)) eff.push({ class: n.class, start: n.start, end: n.end, properties: n.props, begin: 'set the character wind (direction normalised, frequency = 1 / WavePeriod, scale 1 on the listed groups; no list = all groups)', tick: 'scale = value of the clip float curve "Wind" (constant 2 in 0033) from the second tick', end_effect: 'clear' });
      else if (/Bonamik_Enable/.test(n.class)) { const present = (n.props.m_Groups || []).filter((g) => groupNames.includes(String(g).toLowerCase())); if (present.length) eff.push({ class: n.class, start: n.start, end: n.end, groups_present: present, on: n.props.m_On !== false }); }
    }
    clipNotifies[clip] = eff.length ? eff : 'no effect with this asset (no Bonamik notify, or every group it names is absent from p_ex001_Bonamik)';
  }
  // the fall loop's notify effects move the simulation (smoke test of tools/lib/bonamik/notify.js)
  let notifyTest = null;
  { const nt = (data.notify_states['0033'] || {}).notifies || []; const gp = nt.find((x) => /ChangeGroupParams/.test(x.class)), cw = nt.find((x) => /CharaWind/.test(x.class));
    if (gp && cw && fall) { const pose = clipPose(fall, 0); const sA = mk({ zeroJitter: true }), sB = mk({ zeroJitter: true }); let oA, oB; for (let i = 0; i < 100; i++) { if (i === 50) { bnmNotify.setGroupParam(sB, gp.props.m_GroupParams); bnmNotify.setCharaWind(sB, cw.props); } if (i === 51) bnmNotify.setCharaWindScale(sB, 2); oA = sA.frame(1 / 30, clone(pose)); oB = sB.frame(1 / 30, clone(pose)); }
      let mxd = 0; oA.pose.forEach((x, i) => { mxd = Math.max(mxd, dist(x.t, oB.pose[i].t)); }); notifyTest = { max_bone_translation_change_cm_after_50_evaluations: +mxd.toFixed(3), ok: mxd > 1 }; } }
  const sim = a.sim, Pp = sim.core.f3778.f0, w = sim.core.f3810;
  const expect = { ref_settle: 35, ref_max: 2.97, idle_settle: 70, idle_max: 2.72, driven: 86, unchanged: 188 };
  const check = {
    expected: expect,
    reference_pose: { settle_evaluation: a.settle_evaluation, max_node_distance_cm: a.max_node_distance_cm, max_bone_offset_cm: a.max_bone_offset_cm },
    idle_frame_0: { settle_evaluation: b.settle_evaluation, max_node_distance_cm: b.max_node_distance_cm, max_bone_offset_cm: b.max_bone_offset_cm },
    driven_bones: a.driven, non_driven_bones_bit_identical: a.unchanged, non_driven_bones_changed: a.changedUndriven + b.changedUndriven,
    rest_state_independent_of_random_stream: b.state_hash === bSeed.state_hash,
    soft_start_and_hard_reset_end_alike: Math.abs(b.max_node_distance_cm - bHard.max_node_distance_cm) < 1e-3,
    determinism_two_runs_identical: base.every((x, i) => x === again[i]),
    evaluation_states_vs_30fps: rates,
    notify_0033: notifyTest,
  };
  check.ok = a.settle_evaluation === expect.ref_settle && Math.abs(a.max_node_distance_cm - expect.ref_max) <= 0.006 && b.settle_evaluation === expect.idle_settle && Math.abs(b.max_node_distance_cm - expect.idle_max) <= 0.006
    && a.driven === expect.driven && a.unchanged === expect.unchanged && check.non_driven_bones_changed === 0 && check.rest_state_independent_of_random_stream && check.soft_start_and_hard_reset_end_alike
    && check.determinism_two_runs_identical && Object.values(rates).every((x) => x === 'identical') && (!notifyTest || notifyTest.ok);
  check.summary = `settle ${a.settle_evaluation}/${b.settle_evaluation}, max ${a.max_node_distance_cm}/${b.max_node_distance_cm} cm, driven ${a.driven}, unchanged ${a.unchanged}, 45/60/120/144 fps ${Object.values(rates).every((x) => x === 'identical') ? 'identical' : 'DIFFER'}`;
  const sc = sim.core.f3778;
  return { check, clip_notifies: clipNotifies,
    counts: { particles: Pp.length, dynamic_particles: Pp.filter((p) => !p.f276).length, links: sc.f48, cones: sc.f38, anchors: sc.f18, chain_roots: sc.f750, driven_bones: a.driven },
    driven_bones: bones.map((bn, i) => ({ bone: i, name: bn.name })).filter((x) => w[x.bone] > 0),
    dynamic_particle_bones: Pp.filter((p) => !p.f276).map((p) => ({ particle: p.name, bone: p.f248, bone_name: names[p.f248], group: data.groups[p.f2B8] ? data.groups[p.f2B8].name : p.f2B8 })) };
}

// ------------------------------------------------------------------------------------------------ main
async function main() {
  const checkOnly = process.argv.includes('--check');
  bc.selfCheck();
  const id = identify();
  const meshContent = gamePathToContent(id.skeletal_mesh);
  const meshGlbFile = P(ROOT, 'assets_converted', 'meshes', CONTENT, meshContent + '.glb');
  const meshGlb = readGlb(meshGlbFile), meshSide = readJson(P(ROOT, 'assets_converted', 'meshes', CONTENT, meshContent + '.mesh.json'));
  const bones = buildSkeleton(meshGlb.json);

  // animation set -> stage-1 rows
  const sets = id.anim_sets.map(loadAnimSet);
  const setRows = new Map(); for (const s of sets) for (const r of s.rows) if (!setRows.has(r.name)) setRows.set(r.name, { ...r, set: s.content });
  const logical = []; const seqs = new Map(); // game path -> { names: [] }
  const needSeq = (gp, why) => { if (!seqs.has(gp)) seqs.set(gp, { used_by: [] }); seqs.get(gp).used_by.push(why); };
  const stage1Names = new Set(STAGE1.map((s) => s.name));
  const EXTRA = [...setRows.values()].filter((r) => !stage1Names.has(r.name)).map((r) => ({ name: r.name, mode: null, blend: null, by: 'not a stage-1 animation: mode and blend-in are the arguments of the state that plays it', use: '', optional: true, extra: true }));
  for (const s of [...STAGE1, ...EXTRA]) {
    const row = setRows.get(s.name);
    if (!row || !row.asset) throw new Error('anim set has no row ' + s.name);
    const d = describeAnimAsset(row.asset);
    const e = { name: s.name, loop: s.mode === null ? null : !!(s.mode & 1), play: { mode: s.mode, loop_bit: s.mode === null ? null : !!(s.mode & 1), root_motion_bit: s.mode === null ? null : !!(s.mode & 2), blend_in_argument: s.blend, play_rate: 1, root_translation_scale: [1, 1], played_by: s.by }, use: s.use, optional: !!s.optional, anim_set: `/Game/${row.set}`, asset: row.asset, class: d.class, same_asset_as: [...setRows.values()].filter((r) => r.asset === row.asset && r.name !== s.name).map((r) => r.name) };
    if (s.extra) e.stage1 = false;
    if (d.class === 'BlendSpace') { Object.assign(e, { axis: d.axis, axis_y: d.axis_y, grid_samples: d.grid_samples, sample_order_in_asset: d.sample_order_in_asset, sample_weight_rule: d.sample_weight_rule, input_filter: d.input_filter, target_weight_interpolation_speed_per_sec: 0, m_InterpolationTime: d.m_InterpolationTime, notify_trigger_mode: d.notify_trigger_mode, samples: d.samples }); for (const sm of d.samples) needSeq(sm.asset, `${s.name}@${sm.value},${sm.value_y}`); }
    else if (d.class === 'BlendSpace1D') { Object.assign(e, { axis: d.axis, grid_samples: d.grid_samples, sample_order_in_asset: d.sample_order_in_asset, sample_weight_rule: d.sample_weight_rule, input_filter: d.input_filter, target_weight_interpolation_speed_per_sec: 0, time_sync: d.time_sync, root_motion_rule: d.root_motion_rule, m_InterpolationTime: d.m_InterpolationTime, m_InterpolationTime_note: 'only the default blend-in time of the play call', notify_trigger_mode: d.notify_trigger_mode, notify_trigger_mode_note: 'SQEX_AllAnimation: on every update the notifies of EVERY sample sequence fire, whatever its weight', samples: d.samples })
      ; for (const sm of d.samples) needSeq(sm.asset, `${s.name}@${sm.value}`); } else needSeq(row.asset, s.name);
    logical.push(e);
  }
  // the skeleton asset of the animations lists the same bones in the same order as the mesh
  const animFileOf = (gp) => 'anims/' + path.basename(gamePathToContent(gp)) + '.glb';
  const anims = new Map();
  for (const gp of seqs.keys()) {
    const a = loadAnimation(gp, bones);
    const src = readGlb(P(ROOT, 'assets_converted', 'animations', CONTENT, a.content + '.glb')).json;
    const sb = buildSkeleton(src);
    if (sb.length !== bones.length || sb.some((b, i) => b.name !== bones[i].name || b.parent !== bones[i].parent)) throw new Error('skeleton of ' + gp + ' differs from the mesh skeleton');
    a.ref_pose_max_dev = Math.max(...sb.map((b, i) => Math.max(...b.translation.map((x, k) => Math.abs(x - bones[i].translation[k])), ...b.rotation.map((x, k) => Math.abs(Math.abs(x) - Math.abs(bones[i].rotation[k]))))));
    anims.set(gp, a);
  }

  // what the play call resolves to: blend-in (-1 = m_InterpolationTime of the asset; class default 0.0, constructors 0x143F5A030 / 0x143F5A5C0) and
  // whether root motion is extracted (mode bit 1 and bEnableRootMotion of the sequence, 0x143F33C00 / 0x143F44DB0 / 0x143F44E10)
  for (const l of logical) {
    const seqsOf = l.samples ? l.samples.map((x) => x.asset) : [l.asset];
    const it = l.samples ? (l.m_InterpolationTime ?? 0) : (anims.get(l.asset).props.m_InterpolationTime ?? 0);
    l.play.blend_in_s = l.play.blend_in_argument === null ? null : l.play.blend_in_argument < 0 ? it : l.play.blend_in_argument;
    l.play.asset_m_InterpolationTime = it;
    l.play.sequences_with_bEnableRootMotion = seqsOf.filter((g) => anims.get(g).props.bEnableRootMotion === true).map((g) => path.basename(gamePathToContent(g)));
    l.play.root_motion_extracted = l.play.root_motion_bit === null ? null : l.play.root_motion_bit && l.play.sequences_with_bEnableRootMotion.length > 0;
  }
  // face clips
  const faceSets = id.sub_anim_sets.map(loadAnimSet);
  const faceRows = new Map(); for (const fs2 of faceSets) for (const r of fs2.rows) if (!faceRows.has(r.name)) faceRows.set(r.name, { ...r, set: fs2.content });
  const faceAnims = new Map(); const faceLogical = [];
  // 1.6.0: every face row the body clips' TresAnimNotify_FaceAnim / _FaceAnimEye / _FaceAnimLip notifies can start
  // (the six stage-1 rows first, in their old order). A notify names a row (FaceAnimName / m_AnimName) or a type
  // (FaceAnimType / m_AnimType, default NORMAL_AUTO); a type gives one or two base names (tools/lib/face_type_rows.json:
  // the switch of 0x14056D3E0, decompiled C; two = [in battle, otherwise]); a base name is looked up as name,
  // name + "_EL", name + "_EYE" (eye slot) / name + "_LIP" (mouth slot), case-insensitive.
  const faceTypes = readJson(P(ROOT, 'tools', 'lib', 'face_type_rows.json')).rows;
  const faceRowByLower = new Map([...faceRows.keys()].map((k) => [k.toLowerCase(), k]));
  const faceWanted = [...FACE_ROWS]; const faceUnresolved = new Map();
  const findFaceRow = (base, last) => { for (const c of [base, base + '_EL', base + last]) { const k = faceRowByLower.get(c.toLowerCase()); if (k && faceRows.get(k).asset) return k; } return null; };
  for (const a of anims.values()) for (const x of a.notifies) {
    const m = /^TresAnimNotify_FaceAnim(Eye|Lip)?$/.exec(x.class || ''); if (!m) continue;
    const pr = x.properties || {};
    const named = pr.FaceAnimName ?? pr.m_AnimName;
    const type = pr.FaceAnimType ?? pr.m_AnimType ?? 'NORMAL_AUTO';
    const bases = named && named !== 'None' ? [named] : ((faceTypes[type] || {}).names || []);
    if (!bases.length) { faceUnresolved.set('type ' + type, (faceUnresolved.get('type ' + type) || 0) + 1); continue; }
    const slots = m[1] === 'Eye' ? ['_EYE'] : m[1] === 'Lip' ? ['_LIP'] : ['_EYE', '_LIP'];
    for (const base of bases) for (const last of slots) {
      const row = findFaceRow(base, last);
      if (row) { if (!faceWanted.includes(row)) faceWanted.push(row); }
      else faceUnresolved.set(base + last, (faceUnresolved.get(base + last) || 0) + 1);
    }
  }
  for (const name of faceWanted) {
    const row = faceRows.get(name); if (!row || !row.asset) throw new Error('face anim set has no row ' + name);
    if (!faceAnims.has(row.asset)) faceAnims.set(row.asset, loadAnimation(row.asset, bones));
    faceLogical.push({ name, anim_set: `/Game/${row.set}`, asset: row.asset, file: 'face/' + path.basename(gamePathToContent(row.asset)) + '.glb' });
  }
  // KineDriver data check (the two validations of the research pass, kept alive here)
  const kdData = readJson(P(ROOT, 'audit', 'sora_port', 'kinedriver.json'));
  // the stage-1 clips only (as before 1.5.0): the baked-track comparison runs the driver at every key of every clip
  const stage1Seqs = new Set(); for (const l of logical) if (l.stage1 !== false) for (const gp of (l.samples ? l.samples.map((x) => x.asset) : [l.asset])) stage1Seqs.add(gp);
  const stage1Anims = new Map([...anims].filter(([gp]) => stage1Seqs.has(gp)));
  const kdCheck = kineDriverCheck(kdData, bones, stage1Anims);
  const bnmData = readJson(P(ROOT, 'audit', 'sora_port', 'bonamik.json'));
  const bnmCheck = bonamikCheck(bnmData, bones, anims.get(setRows.get('IdleN').asset), anims.get(setRows.get('JumpNormalDownLoop').asset));

  // materials
  const mats = meshGlb.json.materials.map((m, slot) => {
    const gp = m.extras.ue_material, info = materialInfo(gp);
    const rule = RULES.find((r) => r.root.test(info.root_material || ''));
    if (!rule) throw new Error('no material rule for ' + info.root_material + ' (' + gp + ')');
    const gl = { name: m.name, pbrMetallicRoughness: { metallicFactor: 0, roughnessFactor: 0.8 }, extras: { ue_material: gp, ue_root_material: info.root_material, ue_blend_mode: info.blend_mode } };
    const textures = []; const rec = { slot, slot_name: m.name, material_instance: gp, parent_chain: info.chain, root_material: info.root_material, ue_blend_mode: info.blend_mode, ue_two_sided: info.two_sided, approximation: rule.approx,
      texture_parameters: info.tex, vector_parameters: info.vec, scalar_parameters: info.scal };
    if (rule.base) { const t = texturePng(info.tex[rule.base], 'color'); textures.push(t); gl.pbrMetallicRoughness.baseColorTexture = { index: t.file }; rec.base_color = { parameter: rule.base, texture: info.tex[rule.base], file: t.file }; }
    if (rule.color) { const a = info.vec[rule.color[0]], b = info.vec[rule.color[1]]; const c = [0, 1, 2].map((k) => +((a[k] + b[k]) / 2).toFixed(6)); gl.pbrMetallicRoughness.baseColorFactor = [...c, 1]; rec.base_color = { parameters: rule.color, values_linear: [a, b], factor_linear: c }; }
    if (rule.tinted) { const t = texturePng(info.tex[rule.tinted[0]], 'alpha_tint', { tint: info.vec[rule.tinted[1]].slice(0, 3), tintName: rule.tinted[1] }); textures.push(t); gl.pbrMetallicRoughness.baseColorTexture = { index: t.file }; rec.base_color = { parameter: rule.tinted[0], colour_parameter: rule.tinted[1], texture: info.tex[rule.tinted[0]], file: t.file }; }
    if (rule.hidden) { gl.pbrMetallicRoughness.baseColorFactor = [1, 1, 1, 0]; gl.alphaMode = 'BLEND'; rec.hidden = true; }
    if (rule.blend) gl.alphaMode = 'BLEND';
    if (rule.doubleSided || info.two_sided) gl.doubleSided = true;
    if (rule.normal && info.tex[rule.normal] && !process.argv.includes('--no-normals')) { const t = texturePng(info.tex[rule.normal], 'normal'); textures.push(t); gl.normalTexture = { index: t.file }; rec.normal = { parameter: rule.normal, texture: info.tex[rule.normal], file: t.file }; }
    return { gltf: gl, textures, rec };
  });

  // ---- assemble the files in memory
  const files = new Map(); // relative path -> Buffer
  for (const t of texCache.values()) files.set(t.file, t.png);
  const plain = (m) => JSON.parse(JSON.stringify(m)); // fresh material objects per file (texture indices are resolved in place)
  const matsFor = () => mats.map((m) => ({ gltf: plain(m.gltf), textures: m.textures, rec: m.rec }));
  let meshStats, skinJoints;
  { const w = new GlbWriter(); addSkeletonNodes(w, bones); const r = addMesh(w, meshGlb, bones, matsFor()); meshStats = r.stats; skinJoints = r.joints; w.json.scenes = [{ name: 'sora', nodes: [0, r.node] }]; files.set('sora.glb', w.finish()); }
  for (const [gp, a] of anims) { const w = new GlbWriter(); addSkeletonNodes(w, bones); addAnimation(w, path.basename(a.content), a); w.json.scenes = [{ name: path.basename(a.content), nodes: [0] }]; files.set(animFileOf(gp), w.finish()); }
  const faceFileOf = (gp) => 'face/' + path.basename(gamePathToContent(gp)) + '.glb';
  for (const [gp, a] of faceAnims) { const w = new GlbWriter('(face clip)'); addSkeletonNodes(w, bones); addAnimation(w, path.basename(a.content), a); w.json.scenes = [{ name: path.basename(a.content), nodes: [0] }]; files.set(faceFileOf(gp), w.finish()); }
  const dataList = [];
  for (const d of DATA_FILES) { const src = P(ROOT, 'audit', 'sora_port', d.source); if (!fs.existsSync(src)) { if (d.optional) continue; throw new Error('missing research data: ' + rel(src)); } files.set(d.file, fs.readFileSync(src)); dataList.push(d); }
  { const w = new GlbWriter('(mesh + every stage-1 animation)'); addSkeletonNodes(w, bones); const r = addMesh(w, meshGlb, bones, matsFor()); for (const a of stage1Anims.values()) addAnimation(w, path.basename(a.content), a); w.json.scenes = [{ name: 'sora', nodes: [0, r.node] }]; files.set('sora_all.glb', w.finish()); }

  // ---- root-motion tracks (the track of the root bone kh_sk, per frame) and foot height
  const rootMotion = buildRootMotion(anims, bones, logical, id, new Set([...stage1Anims.values()].map((a) => path.basename(a.content))));
  const feet = footHeight(meshGlb, bones, anims.get(setRows.get('IdleN').asset));
  const atkSockets = attackSockets(anims, bones, id);

  // ---- manifest
  const srcCache = new Map(); const src = (content) => { if (!srcCache.has(content)) srcCache.set(content, sourceFiles(content)); return srcCache.get(content); };
  const item = (file, kind, content, more) => ({ file, kind, bytes: files.get(file).length, sha256: sha256(files.get(file)), source_asset: content ? `/Game/${content}` : null, source_package: content ? assetId(content) : null, source_sha256: content ? src(content) : null, ...more });
  const items = [];
  items.push(item('sora.glb', 'mesh', meshContent, { lod: 0, source_lods: meshSide.data.lods.length, ...meshStats, skeleton_bones: bones.length, converted_from: rel(meshGlbFile),
    note: 'LOD 0 only; one primitive per mesh section with its own vertex range; one skin whose joints are the bones that carry vertex weights (manifest.skin); weights as float32; TEXCOORD_0..2 kept' }));
  items.push(item('sora_all.glb', 'mesh+animations', meshContent, { note: 'the same mesh plus the STAGE-1 animations of anims/ (animation name = sequence asset name); for loaders that want one file. The other clips of anims/ are not in it (size)', animations: [...stage1Anims.values()].map((a) => path.basename(a.content)) }));
  for (const t of texCache.values()) items.push(item(t.file, 'texture', t.content, { role: t.mode, width: t.width, height: t.height, source_pixel_format: t.pixel_format, srgb: t.srgb, note: t.note, decoder_mip_check: t.mip_check, bc7_mode_histogram: t.bc7_modes, converted_from: rel(P(ROOT, 'assets_converted', 'textures', CONTENT, t.content + '.dds')) }));
  const logicalOf = (gp) => seqs.get(gp).used_by;
  for (const [gp, a] of anims) {
    items.push(item(animFileOf(gp), 'animation', a.content, { animation_name: path.basename(a.content), logical_names: logicalOf(gp), duration_s: a.length, num_frames: a.numFrames, frame_rate: a.fps,
      loop: logical.filter((l) => l.asset === gp || (l.samples || []).some((s) => s.asset === gp)).some((l) => l.loop === true), root_motion_enabled: a.props.bEnableRootMotion === true, tracks: a.tracks, bones_without_track: bones.length - a.tracks,
      interpolation: a.chans[0].interpolation, sequence_properties: a.props, key_counts: a.keyCounts, tracked_bones: a.tracked_bones, root_bone: a.root.kh_sk, trans_bone: a.root.trans, root_track_moves: a.root_track_moves, notifies: a.notifies,
      converted_from: rel(P(ROOT, 'assets_converted', 'animations', CONTENT, a.content + '.glb')) }));
  }
  for (const [gp, a] of faceAnims) {
    items.push(item(faceFileOf(gp), 'face-animation', a.content, { animation_name: path.basename(a.content), logical_names: faceLogical.filter((l) => l.asset === gp).map((l) => l.name), duration_s: a.length, num_frames: a.numFrames, frame_rate: a.fps,
      loop: true, tracks: a.tracks, bones_without_track: bones.length - a.tracks, interpolation: a.chans[0].interpolation, sequence_properties: a.props, key_counts: a.keyCounts, tracked_bones: a.tracked_bones, notifies: a.notifies,
      note: 'played on the slots Face (G_eye subtree) and Mouth (G_mouth subtree) only: tracks of other bones in this clip are discarded by the graph',
      converted_from: rel(P(ROOT, 'assets_converted', 'animations', CONTENT, a.content + '.glb')) }));
  }
  for (const d of dataList) items.push({ file: d.file, kind: 'data', bytes: files.get(d.file).length, sha256: sha256(files.get(d.file)), source_asset: null, source_package: null, source_sha256: null, content: d.what, research_notes: d.notes, copied_from: 'audit/sora_port/' + d.source });
  // leg-IK windows and face plays per clip, from the notifies
  const ikWindows = {}; const facePlays = {};
  for (const a of [...anims.values(), ...faceAnims.values()]) {
    const n = path.basename(a.content);
    ikWindows[n] = a.notifies.filter((x) => x.class === 'TresAnimNotifyState_IK').map((x) => ({ start: x.trigger_time, end: x.end_trigger_time, start_interpolation_time: x.properties.m_StartInterpolationTime ?? 0, end_interpolation_time: x.properties.m_EndInterpolationTime ?? 0, target_alpha: x.properties.m_TargetAlpha ?? 1 }));
    facePlays[n] = a.notifies.filter((x) => /^TresAnimNotify_FaceAnim/.test(x.class || '')).map((x) => ({ time: x.trigger_time, class: x.class, properties: x.properties }));
  }
  const manifest = {
    format: 'kh3-sora-pack/1', generator: `tools/sora_pack.js ${VERSION}`, spec: 'docs/sora-animation-spec.md',
    legal: 'Converted from the user\'s own copy of the game for local use; nothing in this folder is distributed.',
    character: 'Sora, default form (p_ex001)', identification: id, conventions: CONVENTIONS,
    placement: {
      note: 'How the mesh sits in the pawn, in pack conventions. The actor origin is on the ground between the feet; the actor faces UE +X; the mesh component is rotated so that the mesh (which faces UE +Y in its own space, pack +Z) faces actor forward.',
      mesh_component_relative_location_ue_cm: id.mesh_component.relative_location_ue_cm, mesh_component_relative_rotation_ue_deg: id.mesh_component.relative_rotation_ue_deg,
      pack_model_forward: '+Z', pack_model_up: '+Y', pack_model_left: '+X',
      to_place: 'put the pack scene at the actor origin (feet), rotated about +Y so that pack +Z points along the actor\'s facing direction; no vertical offset',
      capsule_centre_above_origin_m: id.collision_capsule.relative_location_ue_cm.z * UNIT, capsule_size_ue_cm: id.collision_capsule.size_ue_cm,
      mesh_origin_relative_to_capsule_centre_m: [0, -id.collision_capsule.relative_location_ue_cm.z * UNIT, 0],
    },
    skeleton: { source_asset: id.skeleton, bone_count: bones.length, root: bones[0].name, node_index_equals_bone_index: true,
      note: 'reference pose from the SkeletalMesh export (identical bone list and order in the Skeleton asset used by the animations). parent -1 = root. translation in metres, rotation quaternion xyzw, pack conventions; the *_ue values are the original cm / left-handed ones.',
      bones: bones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, translation: b.translation.map(f32), rotation: b.rotation.map(f32), scale: b.scale.map(f32), translation_ue: b.ue.translation, rotation_ue: b.ue.rotation, scale_ue: b.ue.scale })) },
    materials: mats.map((m) => m.rec),
    animation_set: { source_assets: id.anim_sets, rows_total: [...setRows.keys()].length, structure: 'TresAnimSet (DataAsset): Anims = array of TresAnimAssetUnit { AnimName: FName, AnimData: AnimationAsset (AnimSequence or BlendSpace1D) }; the game plays animations by AnimName' },
    skin: { files: ['sora.glb', 'sora_all.glb'], skins_per_file: 1, joint_limit_applied: MAX_JOINTS, joint_count: skinJoints.length,
      note: 'JOINTS_0 holds indices into this joint list, not bone indices. The list is the bones that carry a non-zero vertex weight at LOD 0, in bone order; every other bone is a plain node of the same hierarchy (animations still target it) and is not a skin joint. skin.skeleton is node 0 (kh_sk). Inverse bind matrices are those of the listed bones. The skinned result is unchanged.',
      joints: skinJoints.map((b, k) => ({ joint: k, bone: b, name: bones[b].name })),
      bones_not_in_skin: bones.map((b, i) => ({ bone: i, name: b.name })).filter((x) => !skinJoints.includes(x.bone)) },
    root_motion: rootMotion, feet: feet, attack_sockets: atkSockets,
    runtime: {
      note: 'What the game runs on top of the clip poses, with the constants of this character. Algorithms: docs/sora-animation-spec.md sections 5-12 and the research notes they cite.',
      frame_order: ['pose tick (montage weights, positions, notifies, graph update) from inside the movement component tick, with the real frame dt, before the movement is performed', 'graph evaluation: slots blended against the saved pose of the previous frame -> saved pose written -> leg IK -> look-at -> local pose', 'KineDriver (313 operators, stateless) on the component-space pose', 'Bonamik (30 Hz evaluation + interpolation) on the component-space pose'],
      saved_pose: { rule: 'S[n] = per bone: Face slot blend for the G_eye subtree, Mouth slot blend for the G_mouth subtree, FullBody slot blend otherwise; every slot blends its montage with S[n-1] as source: out = blend(montage weight w, source weight 1 - w); S[0] = IdleN at t = 0 (bones without a track: reference pose)', space: 'local (parent-relative)', taken: 'after the face layers, before leg IK and look-at',
        blend: { rotation: 'accumulated nlerp: R = q0 * w0; for i >= 1: R += (dot(R, qi * wi) >= 0 ? +1 : -1) * qi * wi; normalise. Order: montage pose first, source pose last', translation: 'sum of w_i * t_i', scale: 'exp(sum of w_i * ln(s_i)) per component', single_pose: 'when only one pose remains (montage weight >= 1 - 1e-5) the result is pose * w (rotation renormalised)' },
        montage_weight: 'linear: min(1, elapsed real time since the first update after the play call, including that update\'s dt) / blend_in; blend_in 0 -> 1 on the first update', position: 'stays 0 on the first update after the play call (whatever its dt), then advances by dt * play rate', face_subtrees: { G_eye: bones.map((b, i) => i).filter((i) => { for (let k = i; k >= 0; k = bones[k].parent) if (bones[k].name === 'G_eye') return true; return false; }), G_mouth: bones.map((b, i) => i).filter((i) => { for (let k = i; k >= 0; k = bones[k].parent) if (bones[k].name === 'G_mouth') return true; return false; }) } },
      leg_ik: { node: 'TresAnimNode_TwoBoneIKMultiped (data/legik.json -> two_bone_ik_multiped[0])', alpha: 'min(m_LegIKAlpha, 0.99); m_LegIKAlpha = pawn interpolator: linear towards the target over the given time (TickBlendWeight); target and time are set by TresAnimNotifyState_IK begin (target_alpha, start_interpolation_time) and end (0, end_interpolation_time); two states with the same target_alpha merge (no end/begin)', initial_alpha: 0, windows_by_clip: ikWindows, inactive_branch: 'the two TresAnimNode_CollTwoBoneIK nodes never run (IKinemaSW is true)' },
      look_at: 'TresAnimNode_ExLookAt and both ModifyBone nodes are exact pass-throughs for the player without a look-at target (alpha 0)',
      face: { slots: { Face: 'G_eye subtree', Mouth: 'G_mouth subtree' }, rows: faceLogical, type_to_row: Object.fromEntries(Object.entries(faceTypes).filter(([, v]) => v.names.length).map(([k, v]) => [k, v.names[v.names.length - 1]])), type_to_row_battle: Object.fromEntries(Object.entries(faceTypes).filter(([, v]) => v.names.length === 2).map(([k, v]) => [k, v.names[0]])), type_source: 'tools/lib/face_type_rows.json (the switch of 0x14056D3E0, decompiled C); type_to_row = the name outside battle, type_to_row_battle = the name in battle for the types that have two', unresolved: Object.fromEntries([...faceUnresolved].sort()), row_lookup: 'name, then name + "_EL", then name + "_EYE" (eye play) or name + "_LIP" (lip play); so NO_0 / NO_21 play the _EL clip on both slots and NO_50..52 play only on Mouth', plays_by_clip: facePlays, notify_defaults: { m_AnimType: 'NORMAL_AUTO', m_BlendInTime: -1, m_bIsLoop: false, m_Speed: 1, m_bSameCheck: true, m_Priority: 0, note: 'TresAnimNotify_FaceAnim uses IsLoop / FaceAnimName; blend-in -1 = the clip\'s m_InterpolationTime; SameCheck: not restarted when the same row is already the slot\'s last played' },
        default_pose: 'frame 0 of 0000EM on the G_eye and G_mouth subtrees for an implementation that does not animate the face' },
      kine_driver: { data: 'data/kinedriver.json', operators: kdData.operators.length, bones_written: kdData.bones.filter((b) => b.written).length, stateless: true, evaluator: 'tools/lib/kd_eval.js (UE units and axes)', check: kdCheck },
      bonamik: {
        data: ['data/bonamik.json', 'data/bonamik2.json'], evaluator: 'tools/lib/bonamik/bnm_eval.js (UE component space, cm; float32 arithmetic in the exe\'s operation order)', specification: 'docs/sora-animation-spec.md section 14; build/sora_tmp/native_H_bonamik2.md',
        order: 'every frame: KineDriver operators first (real dt), then Bonamik; both after the anim graph (leg IK included), in place on the component-space pose',
        evaluation_rule: { rate_hz: 30, rule: 'a timer of 1/30 s: the solver is evaluated on the frame where the timer expires (the overshoot is carried), with solver step dtSim = 0.05 s (step 1/30 x 1.5) whatever the frame rate >= 30 fps; the first two ticks both evaluate; below 30 fps every frame evaluates and the frame-rate bucket changes the step',
          interpolation: 'two buffers per driven bone hold the DELTA of the simulated local transform against the animated local transform (prev, cur). A frame shows the animated local transform composed with the delta interpolated from prev to cur by the eased alpha a * (a + 1) / 2, a = 1 - remaining / interval; an evaluation frame shows prev (a = 0): the displayed pose lags the simulation by one evaluation',
          frame_pattern_60fps: 'E(alpha 1) E(0) -(0.375) E(0) -(0.375) ...' },
        start_state: 'after setup the nodes sit at the mesh REFERENCE pose with a soft reset pending and no pre-roll; nothing in the player pawn, its states or the blueprints under Content/Blueprints/Player issues a hard reset or pre-roll at spawn (native callers of the pre-roll: the anim notify Bonamik_ResetAndPreRoll, Matinee tracks, the photo hologram actor, the blueprint library function TresBonamikResetAndPreRoll; level scripts were not searched)',
        gravity_world_cm_s2: bnmData.scene && bnmData.scene.gravity ? [bnmData.scene.gravity[0] * 100, bnmData.scene.gravity[2] * 100, bnmData.scene.gravity[1] * 100] : [0, 0, -981],
        counts: bnmCheck.counts,
        driven_bones: bnmCheck.driven_bones, driven_bone_note: 'bones with a Bonamik weight (the simulation may write them); every other bone is bit-identical to the incoming pose',
        dynamic_particle_bones: bnmCheck.dynamic_particle_bones,
        random_stream: 'thread-local xorshift128 shared with other game code; used only by the aero force (variant A) when the relative air flow is not zero; at rest the result does not depend on it. Port policy: zero the jitter by default, optionally feed logged draws',
        world_wind: 'per-map wind manager not specified; 0 unless the host supplies it', ground: 'ground query result of the owner not specified; none unless the host supplies it',
        clip_notifies: bnmCheck.clip_notifies,
        check: bnmCheck.check,
      },
      physics_asset: 'moves no bone for the player in field play',
    },
    face_animations: faceLogical,
    data_files: dataList.map((d) => ({ file: d.file, content: d.what, research_notes: d.notes })),
    logical_animations: logical.map((l) => ({ ...l, file: l.class === 'AnimSequence' ? animFileOf(l.asset) : undefined, samples: l.samples ? l.samples.map((s) => ({ ...s, file: animFileOf(s.asset), animation_name: path.basename(gamePathToContent(s.asset)) })) : undefined })),
    items,
  };
  files.set('manifest.json', Buffer.from(JSON.stringify(manifest, null, 1) + '\n'));

  // ---- write
  if (!checkOnly) {
    for (const [f, b] of files) { const o = P(OUT, f); fs.mkdirSync(path.dirname(o), { recursive: true }); fs.writeFileSync(o, b); }
    for (const sub of ['', 'anims', 'textures', 'face', 'data']) for (const f of fs.readdirSync(P(OUT, sub))) { const relf = (sub ? sub + '/' : '') + f; if (fs.statSync(P(OUT, relf)).isFile() && !files.has(relf) && relf !== 'validation.json') fs.unlinkSync(P(OUT, relf)); }
  }

  // ---- validate what is on disk
  const validation = { generator: `tools/sora_pack.js ${VERSION}`, validator: 'gltf-validator ' + require('gltf-validator/package.json').version, files: {} };
  let bad = 0;
  for (const f of [...files.keys()].filter((x) => x.endsWith('.glb'))) {
    const onDisk = P(OUT, f);
    const v = await validateGlb(onDisk); const c = checkGlb(onDisk, bones);
    if (sha256(fs.readFileSync(onDisk)) !== sha256(files.get(f))) { v.stale = true; bad++; }
    const names = c.weighted_bone_names; delete c.weighted_bone_names;
    if (names && f === 'sora.glb') validation.bones_with_vertex_weights = names;
    validation.files[f] = { validator: v, independent: c };
    if (v.errors || v.warnings) bad++;
    if (c.bone_names_match === false || c.joints_out_of_range || c.index_out_of_range || c.non_finite || c.time_errors || c.non_unit_quaternions || (c.weight_sum_max_dev !== undefined && c.weight_sum_max_dev > 1e-6) || (c.ibm_times_bind_max_dev_from_identity !== undefined && c.ibm_times_bind_max_dev_from_identity > 1e-4) || (c.bind_pose_skinning_max_dev_m !== undefined && c.bind_pose_skinning_max_dev_m > 1e-4)) bad++;
  }
  for (const t of texCache.values()) { const d = bc.decodePng(fs.readFileSync(P(OUT, t.file))); validation.files[t.file] = { png_read_back: `${d.width}x${d.height}x${d.channels}`, ok: d.width === t.width && d.height === t.height, decoder_mip_check: t.mip_check }; if (!validation.files[t.file].ok || (t.mip_check && t.mip_check.mean_abs_diff > 4)) bad++; }
  validation.kine_driver = kdCheck; if (!kdCheck.ok) bad++;
  validation.bonamik = bnmCheck.check; if (!bnmCheck.check.ok) bad++;
  validation.failed_checks = bad;
  if (!checkOnly) fs.writeFileSync(P(OUT, 'validation.json'), JSON.stringify(validation, null, 1) + '\n');

  // ---- sheet: one row per packed item
  if (!checkOnly) {
    const cols = ['id', 'kind', 'logical', 'source', 'source_sha256', 'bytes', 'sha256', 'duration', 'frames', 'loop', 'root_motion', 'validation', 'evidence'];
    const ev = (it) => {
      if (it.kind === 'mesh' || it.kind === 'mesh+animations') return `SkeletalMesh property of ${id.mesh_component.name} in /Game/${PAWN_BP} (class default object of the pawn blueprint; native parent ${id.pawn_native_class})`;
      if (it.kind === 'animation') return `rows ${it.logical_names.map((n) => n.split('@')[0]).filter((x, i, a) => a.indexOf(x) === i).join(', ')} of ${id.anim_sets.join(', ')} (MyAnimSets of the class default object of ${id.anim_class})` + (it.logical_names.some((n) => n.includes('@')) ? '; sample of the BlendSpace1D the row names' : '');
      if (it.kind === 'texture') return `texture parameter of ${mats.filter((m) => m.textures.some((t) => t.file === it.file)).map((m) => m.rec.material_instance.split('.').pop()).join(', ')} (material slots of the mesh), or the default of the root material`;
      return 'written by tools/sora_pack.js from the items above';
    };
    const val = (f) => { const v = validation.files[f]; if (!v) return '-'; if (v.validator) return `gltf-validator ${v.validator.errors} errors ${v.validator.warnings} warnings; independent read ok`; return v.ok ? 'png read back; decoder mip check mean ' + (v.decoder_mip_check ? v.decoder_mip_check.mean_abs_diff : '-') : 'FAILED'; };
    const rows = items.map((it) => [it.file, it.kind, it.logical_names ? it.logical_names : it.kind === 'texture' ? [it.role] : '-', it.source_package || '-', it.source_sha256 ? it.source_sha256.uexp || it.source_sha256.uasset : '-', it.bytes, it.sha256,
      it.duration_s ?? '-', it.num_frames ?? '-', it.loop === undefined ? '-' : it.loop ? 'yes' : 'no', it.root_track_moves === undefined ? '-' : it.root_track_moves ? 'yes' : 'no', val(it.file), ev(it)]);
    for (const r of rows) { if (r[1] === 'data') { r[11] = r[0] === 'data/kinedriver.json' ? (kdCheck.ok ? 'KineDriver model check ok (reference pose, baked tracks)' : 'KineDriver model check FAILED') : /bonamik/.test(r[0]) ? (bnmCheck.check.ok ? 'Bonamik evaluator checks ok (rest stability, driven set, determinism, frame-rate independence)' : 'Bonamik evaluator checks FAILED') : 'copied'; r[12] = 'research data, ' + items.find((x) => x.file === r[0]).research_notes; } }
    rows.push(['manifest.json', 'manifest', '-', '-', '-', files.get('manifest.json').length, sha256(files.get('manifest.json')), '-', '-', '-', '-', '-', 'written by tools/sora_pack.js from the items above']);
    // rows contributed by the other packs under build/sora_pack/ (written by their own tools, same 13 columns; ids prefixed weapon/ and vfx/)
    for (const sub of ['weapon', 'vfx', 'ui', 'audio']) { const xf = P(OUT, sub, 'sheet_rows.json'); if (!fs.existsSync(xf)) continue; for (const r of readJson(xf)) { if (!Array.isArray(r) || r.length !== cols.length || !String(r[0]).startsWith(sub + '/')) throw new Error(`bad extra sheet row in ${rel(xf)}: ${JSON.stringify(r).slice(0, 120)}`); rows.push(r); } }
    const sheet = P(ROOT, 'sheets', 'port', 'sora-assets.json');
    fs.mkdirSync(path.dirname(sheet), { recursive: true });
    fs.writeFileSync(sheet, `{"sheet":"sora-assets","group":"port","columns":${JSON.stringify(cols)},"rows":[\n${rows.map((r) => JSON.stringify(r)).join(',\n')}\n]}\n`);
  }

  // ---- report
  const total = [...files.values()].reduce((s, b) => s + b.length, 0);
  console.log(`sora_pack ${VERSION}: ${files.size} files, ${(total / 1048576).toFixed(1)} MB${checkOnly ? ' (check only, nothing written)' : ' -> ' + rel(OUT)}`);
  console.log(`  mesh ${id.skeletal_mesh}: ${meshStats.vertices} vertices, ${meshStats.triangles} triangles, ${bones.length} bones, ${mats.length} materials, ${texCache.size} textures`);
  console.log(`  animations: ${anims.size} sequences for ${logical.length} logical names (${stage1Anims.size} of them stage-1: sora_all.glb, KineDriver check), ${[...anims.values()].reduce((n, a) => n + a.notifies.length, 0)} notifies, ${(([...files].filter(([f]) => f.startsWith('anims/')).reduce((n, [, b]) => n + b.length, 0)) / 1048576).toFixed(1)} MB in anims/`);
  for (const [f, v] of Object.entries(validation.files)) if (v.validator) console.log(`  ${f.padEnd(22)} validator ${v.validator.errors}E ${v.validator.warnings}W ${v.validator.infos}I  ${Object.keys(v.validator.codes).join(' ')}${v.validator.first.length ? '  ' + v.validator.first[0] : ''}`);
  const s = validation.files['sora.glb'].independent;
  console.log(`  skin: weight sum dev ${s.weight_sum_max_dev}, joints out of range ${s.joints_out_of_range}, IBM*bind dev ${s.ibm_times_bind_max_dev_from_identity}, bind-pose skinning dev ${s.bind_pose_skinning_max_dev_m} m, bones with weights ${s.bones_with_weights}`);
  console.log(`  bounds ${JSON.stringify(s.bounds_m)} nose-head ${JSON.stringify(s.nose_minus_head_m)} left hand x ${s.left_hand_x_m} right hand x ${s.right_hand_x_m} ccw-agree ${s.ccw_triangles_agreeing_with_normals} against ${s.triangles_against_normals}`);
  console.log(`  KineDriver check: reference pose ${kdCheck.reference_pose.ok ? 'ok' : 'FAILED'} (${kdCheck.reference_pose.bones_outside_tolerance.join(', ')} outside, as expected), baked tracks ${kdCheck.baked_tracks.ok ? 'ok' : 'FAILED'} over ${Object.keys(kdCheck.baked_tracks.clips).length} clips`);
  console.log(`  Bonamik check: ${bnmCheck.check.ok ? 'ok' : 'FAILED'} (${bnmCheck.check.summary})`);
  console.log(`  failed checks: ${bad}`);
  if (bad) process.exitCode = 1;
}
// Run as a script: build the pack. Required as a module (tools/sora_weapon_pack.js): only the helpers are exported, nothing runs.
if (require.main === module) main().catch((e) => { console.error(e); process.exit(1); });
else module.exports = { VERSION, ROOT, OUT, CONTENT, UNIT, CONVENTIONS, sha256, readJson, rel, f32, gamePathToContent, loadPkg, importPath, propsOf, structOf, sourceFiles, assetId, readGlb, readAccessor, GlbWriter, cvPos, cvDir, cvQuat, cvScale, trsMatrix, mul, invertAffine, xform, loadAnimSet, texturePng, texCache, materialInfo, buildSkeleton, addSkeletonNodes, globalBind, addMesh, loadAnimation, addAnimation, validateGlb, checkGlb, sampleChan,
  // (party A: tools/party_pack.js packs Donald / Goofy with the same rules)
  describeAnimAsset, buildRootMotion, footHeight,
  // (XSI, 2026-10-16: tools/xsi_pack.js packs the XS abilities' clips with the same socket tracks)
  attackSockets };
