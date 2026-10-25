// party_team_pack.js (party stream D): the team-attack camera data, read from the decoded packages.
//
//   node tools/party_team_pack.js
//
// Reads the seven CameraAnim assets the three state-event camera blueprints start
// (Camera/sora/Skill/{donald,goofy_shoot,shield}) and the two camera shakes of the goofy_shoot
// blueprint (Camera/shake/{shake_kokizami_8sec,tebure_shake01_10sec}), and writes
//   build/sora_pack/party/team/camera_anims/<name>.json   one per anim (readable copy)
//   build/sora_pack/party/team/camera_shakes.json
//   build/gen/rust/party_team_cameras.rs                   static tables for kh-camera's camera_anim.rs
// Every value is the asset's own (key times, values, tangents, interpolation modes); UE 4.17 defaults
// where the asset leaves a property out: CameraAnim.BaseFOV 90, FInterpCurvePoint.InterpMode CIM_Linear,
// InterpTrackMove.RotMode IMR_Keyframed [U].
'use strict';
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const CONTENT = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content');
const OUT_PACK = path.join(ROOT, 'build/sora_pack/party/team');
const OUT_RS = path.join(ROOT, 'build/gen/rust/party_team_cameras.rs');
// (party D5) `--gshoot`: only the Goofy Bombardier link-pawn socket tables (build/gen/rust/party_team_gshoot.rs).
if (process.argv.includes('--gshoot')) { writeGShootSockets(); process.exit(0); }

const ANIMS = [
  'Camera/sora/Skill/donald/CallMeteo_Finish',
  'Camera/sora/Skill/donald/dona_flair',
  'Camera/sora/Skill/goofy_shoot/goofy_shoot_01',
  'Camera/sora/Skill/goofy_shoot/goofy_shoot_03_30f',
  'Camera/sora/Skill/goofy_shoot/goofy_shoot_05',
  'Camera/sora/Skill/goofy_shoot/goofy_shoot_tarl',
  'Camera/sora/Skill/shield/shield_start',
];
const SHAKES = ['Camera/shake/shake_kokizami_8sec', 'Camera/shake/tebure_shake01_10sec'];

const MODES = { CIM_Linear: 'Linear', CIM_CurveAuto: 'CurveAuto', CIM_Constant: 'Constant', CIM_CurveUser: 'CurveUser', CIM_CurveBreak: 'CurveBreak', CIM_CurveAutoClamped: 'CurveAutoClamped' };

function load(rel) {
  return JSON.parse(fs.readFileSync(path.join(CONTENT, rel + '.uasset.json'), 'utf8'));
}
const props = (e) => Object.fromEntries(((e.data && e.data.props) || []).map((p) => [p.name, p.value]));
const exportOf = (j, i) => (i > 0 ? j.exports[i - 1] : null);

// An InterpCurve (InterpCurveFloat / InterpCurveVector) value -> [{t, v, a, l, m}].
function curve(value, vector) {
  if (!value) return [];
  // The decoder gives {Points: [...]} or [{name: 'Points', value: {items: [[props...]]}}].
  let points = value.Points;
  if (!points && Array.isArray(value)) {
    const p = value.find((x) => x.name === 'Points');
    points = p && p.value && p.value.items ? p.value.items.map((item) => Object.fromEntries(item.map((q) => [q.name, q.value]))) : [];
  }
  return (points || []).map((p) => {
    const vec = (x) => (vector ? [x ? x.x || 0 : 0, x ? x.y || 0 : 0, x ? x.z || 0 : 0] : (x || 0));
    const mode = MODES[p.InterpMode || 'CIM_Linear'];
    if (!mode) throw new Error('unknown interp mode ' + p.InterpMode);
    return { t: p.InVal || 0, v: vec(p.OutVal), a: vec(p.ArriveTangent), l: vec(p.LeaveTangent), m: mode };
  });
}

function anim(rel) {
  const j = load(rel);
  const root = j.exports.find((e) => e.class === 'CameraAnim');
  const rp = props(root);
  const name = root.objectName;
  const group = exportOf(j, rp.CameraInterpGroup);
  const interestGroup = rp.m_CameraInterestInterpGroup ? exportOf(j, rp.m_CameraInterestInterpGroup) : null;
  const tracks = (g) => ((g && props(g).InterpTracks) || []).map((i) => exportOf(j, i));
  let pos = [], euler = [], fov = [], interest = [], lookAt = null, fovProperty = null, rotMode = 'IMR_Keyframed';
  for (const t of tracks(group)) {
    const p = props(t);
    if (t.class === 'InterpTrackMove') {
      pos = curve(p.PosTrack, true);
      euler = curve(p.EulerTrack, true);
      if (p.RotMode) rotMode = p.RotMode;
      if (p.LookAtGroupName) lookAt = p.LookAtGroupName;
      if (p.MoveFrame && p.MoveFrame !== 'IMF_World') throw new Error(name + ': MoveFrame ' + p.MoveFrame + ' is not handled');
    } else if (t.class === 'InterpTrackFloatProp') {
      fovProperty = p.PropertyName;
      if (!/FOVAngle|FieldOfView/.test(fovProperty || '')) throw new Error(name + ': float track ' + fovProperty);
      fov = curve(p.FloatTrack, false);
    } else {
      throw new Error(name + ': track class ' + t.class + ' is not handled');
    }
  }
  if (interestGroup) {
    for (const t of tracks(interestGroup)) {
      if (t.class !== 'InterpTrackMove') throw new Error(name + ': interest track ' + t.class);
      interest = curve(props(t).PosTrack, true);
    }
  }
  const interestName = interestGroup ? props(interestGroup).GroupName : null;
  const lookAtInterest = rotMode === 'IMR_LookAtGroup' && lookAt != null && lookAt === interestName;
  if (rotMode === 'IMR_LookAtGroup' && !lookAtInterest) throw new Error(name + ': looks at group ' + lookAt + ' which is not the interest group');
  return {
    name, asset: '/Game/' + rel, length: rp.AnimLength, base_fov: rp.BaseFOV != null ? rp.BaseFOV : 90,
    base_fov_from_asset: rp.BaseFOV != null, rot_mode: rotMode, look_at_group: lookAt, interest_group: interestName,
    look_at_interest: lookAtInterest, fov_property: fovProperty, pos, euler, fov, interest,
  };
}

function shake(rel) {
  const j = load(rel);
  const cdo = j.exports.find((e) => e.objectName.startsWith('Default__'));
  const p = props(cdo);
  const osc = (o) => {
    const q = o ? (Array.isArray(o) ? Object.fromEntries(o.map((x) => [x.name, x.value])) : o) : {};
    return { amplitude: q.Amplitude || 0, frequency: q.Frequency || 0, start_at_zero: q.InitialOffset === 'EOO_OffsetZero' };
  };
  const tri = (v, keys) => { const q = v ? (Array.isArray(v) ? Object.fromEntries(v.map((x) => [x.name, x.value])) : v) : {}; return keys.map((k) => osc(q[k])); };
  return {
    name: cdo.objectName.replace(/^Default__/, '').replace(/_C$/, ''), asset: '/Game/' + rel,
    duration: p.OscillationDuration != null ? p.OscillationDuration : 1, blend_in: p.OscillationBlendInTime != null ? p.OscillationBlendInTime : 0.1,
    blend_out: p.OscillationBlendOutTime != null ? p.OscillationBlendOutTime : 0.2,
    rot: tri(p.RotOscillation, ['Pitch', 'Yaw', 'Roll']), loc: tri(p.LocOscillation, ['X', 'Y', 'Z']), fov: osc(p.FOVOscillation),
    single_instance: !!p.bSingleInstance,
  };
}

const f = (x) => {
  if (!Number.isFinite(x)) throw new Error('not a number: ' + x);
  const s = String(x);
  return /[.eE]/.test(s) ? s : s + '.0';
};
const v3 = (a) => '[' + a.map(f).join(', ') + ']';
function keysRs(keys, vector) {
  return '&[' + keys.map((k) => `K { t: ${f(k.t)}, v: ${vector ? v3(k.v) : f(k.v)}, a: ${vector ? v3(k.a) : f(k.a)}, l: ${vector ? v3(k.l) : f(k.l)}, m: M::${k.m} }`).join(',\n        ') + ']';
}

// sheets/port/party-team.json: one row per team attack in scope (plan 2.6).
const TEAM_ROWS = [
  ['UNISON_FIRE', 22, 'Attack kind 0x3F (rule 7 of Decide)', 'TresNpcState_n_ex001_FRDonaldSora (fr_id 5)', 'shot UNISON_FIRE (TresProj_UnisonFire)', 'DoUFire (Sora 1761, Donald on vfx_root)', 'none (the field camera)', 'UNISON_FIRE', '0x141430A60', 'roll 0x141430A60 [C]; PlayAttack case 0x3F [C]; slot 137 0x14134CCE0 [C]; NPC 0x1411CACA0 [C]', 'Donald held at Sora\'s actor transform (vfx_root offset not applied) [D]'],
  ['UNISON_BLIZZARD', 23, 'Attack kind 0x40', 'TresNpcState_n_ex001_FRDonaldSora', 'shot UNISON_BLIZZARD', 'DoUBlizzard', 'none', 'UNISON_BLIZZARD', '0x14134CCE0', 'as Unison Fire', 'the END shape and the burn respawn are not ported'],
  ['UNISON_THUNDER', 24, 'Attack kind 0x41', 'TresNpcState_n_ex001_FRDonaldSora', 'shot UNISON_THUNDER', 'DoUThunder', 'none', 'UNISON_THUNDER', '0x14134CCE0', 'as Unison Fire', 'convex shape taken as a 50 cm sphere'],
  ['FUSION_SPIN', 25, 'Attack kind 0x42', 'TresNpcState_n_ex002_FRGoofySora (fr_id 6)', 'shot FUSION_SPIN (slot 138 0x14134C710)', 'GooFSpin', 'none', 'FUSION_SPIN', '0x14134C710', 'PlayAttack case 0x42 [C]; NPC 0x1411CB5D0 [C]', '-'],
  ['FUSION_ROCKET', 26, 'Attack kind 0x43', 'TresNpcState_n_ex002_FRGoofySora', 'none (the clip rushes, +0x1D8 = 1300)', 'GooFRocket', 'none', 'FUSION_ROCKET', '0x1413378B0', 'PlayAttack case 0x43 [C]', 'FUSION_ROCKET row hits not wired (no shot, no hit window in the clip)'],
  ['DONALD_METEOR', 207, 'TresPlayerState_FRCallMeteor (135)', 'TresNpcState_FRCallMeteor (fr_id 1)', 'ATresFriendLinkPawnCallMeteor (Appear / Finish / Disappear)', 'CMeteoFinish (Sora, Donald)', 'action 78 CallMeteo_Finish on Sora; 79 orbit 1000 -> 1200 in 5 s at R(-25,210) on the camera target; 80 end', 'FR_CALLMETEOR_FINISH, FR_CALLMETEOR_FINISH_EXP', '0x1412D5680', 'Sora state 0x141375420 [C]; link Appear 0x1412D5680, Finish 0x1412CD0E0 [C]; camera BP [A]; CDO [A]', '40 meteors as projectiles (CallMeteor_Proj: 150 cm sphere, 1700 x speed, accel 2400 to 4000, life 3) on the link pawn timer: show at start x 4.2, fire at ((start - 0.2) x 2 + 0.4) x 4.2 above the target (FUN_1412CAF60 / FUN_1412E3900 / 0x1412DA0B0 [C]); ExpProj 100 + 1500/s at 4.2; Disappear at 4.8 + 2 s [C]; meteor mesh o_ex210_0 drawn by kh-app with its own translated shader (ymt_mtr_lit_00o, D4; no dither clip); D4: target FUN_1412AC290 (controller target, else nearest lockable with sight, max 10000) [C]; link pawn at the feet of Sora (0x14127A780) [C]; stop at the map [D]'],
  ['FLARE_FORCE', 208, 'TresPlayerState_FRFlareForce (136)', 'TresNpcState_FRFlareForce (fr_id 2)', 'ATresFriendLinkPawnFlareForce', 'FForceFinish (Sora, Donald)', 'action 87 dona_flair on the camera target, autoEnd, out 0.33 s', 'FR_FLAREFORCE_FINISH01..03, FR_FLAREFORCE_FINISH_EXP', '0x1412D5F00', 'Sora state 0x1413755D0 [C]; link 0x1412D5F00 (sends 87) [C]; camera BP [A]', '16 missiles as homing projectiles (CDO appear / fire timers, 1200 cm/s, turn 1000 deg/s, 25 cm sphere, D3: Finish at the TresAnimNotify_Timing 1.2064 s of Sora 1723 (+0xC6E by FUN_1412E1950); missile machine 0x1412E72B0 states 1/2/3/4/5/10 with AppearCurve wobble / FireCurve recoil, homing FUN_1412E82B0, EXP row by height (<= 1000 / 2000) [C]; direct hit FINISH01 without EXP; missile mesh o_ex208_0 drawn with its own translated shader (SQEX_OBJ_Basemat_MNL_Inst, D4) in its reference pose (no AnimClass / clips [A]); D4: main target FUN_1412AC290 every tick, own target FUN_1412E43E0 (facing score x lock weight x 5 for the main target; first on a target = fast homing) [C]; FR_FLAREFORCE_FINISH_EXP: no exe or asset user (never spawned)'],
  ['GOOFY_BOMBARDIER', 195, 'TresPlayerState_FRGoofyShoot (101)', 'TresNpcState_n_ex002_FRGoofyShoot (fr_id 3)', 'ATresFriendLinkPawnGoofyShoot (Appear / Move / Disappear)', 'GShootStart / Rise / Hit / Finish (+ Goofy GShootEnd)', 'BpEvent 1636 goofy_shoot_01 on the link pawn + tebure shake x2; 1638 +1 s CUt03 goofy_shoot_03_30f; 1639 orbit 400 R(-50,-20) FOV 60; dtor 1.0 / 0.3 s', 'FR_GOOFYSHOOT_SORA, FR_GOOFYSHOOT_EXP', '0x1412F9130', 'Sora state 0x141375B30 [C]; link Move tick 0x1412DCE50 [C, partly]; camera BP [A]', 'D3: rise to target + 1000 (FUN_1412C93A0), SORA 350 cm sphere, GShootHit / Finish, Goofy thrown R(-45, yaw) x 2300 with 0.9^(60dt) drag until the ground, GoofyShoot_Proj at the landing [C]; RUSH = Goofy n_ex002 row via TeamParty::fr_hit (B); the arc search FUN_1412C90E0 belongs to e_ex041 / e_ex702 targets only; D4: T from the controller target only (0x1412CC160), the pawn falls at its own gravity 980 x GravityScale 3 (0x140582270), waits on its own o_at001 clips (1636 1.867 s, 1638 1.233 s), the mesh of Goofy pitched to R(velocity pitch + 90) at 6 dt, back at 15 dt (0x1411CB010) [C]'],
  ['TRINITY_GUARD', 193, 'TresPlayerState_FRShieldGuardian (30)', 'TresNpcState_FRShieldGuardian (fr_id 4) x2', 'ATresFriendLinkPawnShieldGuardian (m_Param_MaxTime 20)', 'SGuardStart / Idle / Walk / ShieldBash / ShieldBash2 / Jump* / HeavyBash', 'BpEvent 1621 shield_start on the link pawn; end of 1621: shield_01 orbit 380; action 68 FINISH01 + 0.8 s FINISH02; end of 1632: out 0.3 s', 'FR_SHIELDGUARDIAN_SHIELDBASH, _SHIELDBASH2, _HEAVYBASH', '0x141377AC0', 'Sora state 0x141377AC0 (A bash, B jump, 214 cancel) [C]; camera BP [A]', 'D3: the link pawn is a character: kh-core CMC with MaxWalkSpeed 180 / step 47 [A], JumpZVelocity 420, AirControl 0.05, accel 2048 [C ctor 0x140570EB0], AirAttack; bash hit spheres [D]'],
];

function writeSheet() {
  const sheet = { sheet: 'party-team', group: 'port', columns: ['id', 'command', 'sora_state', 'npc_state', 'link_pawn', 'clips', 'cameras', 'rows', 'ref_addr', 'evidence', 'note'], rows: TEAM_ROWS };
  fs.writeFileSync(path.join(ROOT, 'sheets/port/party-team.json'), JSON.stringify(sheet, null, 1) + '\n');
}

function main() {
  writeSheet();
  const anims = ANIMS.map(anim);
  const shakes = SHAKES.map(shake);
  fs.mkdirSync(path.join(OUT_PACK, 'camera_anims'), { recursive: true });
  for (const a of anims) fs.writeFileSync(path.join(OUT_PACK, 'camera_anims', a.name + '.json'), JSON.stringify(a, null, 1));
  fs.writeFileSync(path.join(OUT_PACK, 'camera_shakes.json'), JSON.stringify({ format: 'kh3-party-team-shakes/1', shakes }, null, 1));
  let rs = '// GENERATED by tools/party_team_pack.js from the decoded packages (CameraAnim / CameraShake assets) - DO NOT EDIT.\n';
  rs += '// Included by crates/kh-camera/src/camera_anim.rs (types CameraAnimDef, ShakeData, K = CurveKeyDef, M = CurveMode).\n\n';
  rs += 'pub static TEAM_CAMERA_ANIMS: &[CameraAnimDef] = &[\n';
  for (const a of anims) {
    rs += `    // ${a.asset}: AnimLength ${a.length}, BaseFOV ${a.base_fov}${a.base_fov_from_asset ? '' : ' (default)'}, RotMode ${a.rot_mode}${a.look_at_group ? ' -> ' + a.look_at_group : ''}, FOV track ${a.fov_property}\n`;
    rs += `    CameraAnimDef {\n        name: ${JSON.stringify(a.name)},\n        asset: ${JSON.stringify(a.asset)},\n        length: ${f(a.length)},\n        base_fov: ${f(a.base_fov)},\n        look_at_interest: ${a.look_at_interest},\n`;
    rs += `        pos: ${keysRs(a.pos, true)},\n        euler: ${keysRs(a.euler, true)},\n        fov: ${keysRs(a.fov, false)},\n        interest: ${keysRs(a.interest, true)},\n    },\n`;
  }
  rs += '];\n\npub static TEAM_CAMERA_SHAKES: &[ShakeData] = &[\n';
  const o = (x) => `O { amplitude: ${f(x.amplitude)}, frequency: ${f(x.frequency)}, start_at_zero: ${x.start_at_zero} }`;
  for (const s of shakes) {
    rs += `    // ${s.asset}\n    ShakeData { name: ${JSON.stringify(s.name)}, duration: ${f(s.duration)}, blend_in: ${f(s.blend_in)}, blend_out: ${f(s.blend_out)}, rot: [${s.rot.map(o).join(', ')}], loc: [${s.loc.map(o).join(', ')}], fov: ${o(s.fov)}, single_instance: ${s.single_instance} },\n`;
  }
  rs += '];\n';
  fs.mkdirSync(path.dirname(OUT_RS), { recursive: true });
  fs.writeFileSync(OUT_RS, rs);
  for (const a of anims) console.log(`${a.name.padEnd(20)} length ${a.length.toFixed(3)}  fov ${String(a.base_fov).padEnd(3)} keys pos ${a.pos.length} euler ${a.euler.length} fov ${a.fov.length} interest ${a.interest.length}  ${a.look_at_interest ? 'look-at interest' : 'keyframed rotation'}`);
  for (const s of shakes) console.log(`${s.name.padEnd(22)} ${s.duration} s, blend ${s.blend_in} / ${s.blend_out}, rot ${s.rot.map((x) => x.amplitude + '@' + x.frequency).join(' ')}`);
  console.log('wrote ' + path.relative(ROOT, OUT_RS) + ' and ' + path.relative(ROOT, OUT_PACK));
}

// build/sora_pack/party/team/team_host.json: the team projectile classes -> effects by role, in the
// format of the vfx host's `projectiles` table (kh-app vfx.rs loads it after E's party_host.json).
function writeTeamHost() {
  const C = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content/Blueprints/FriendLink');
  const out = {};
  for (const cls of ['CallMeteor_Proj', 'FlareForce_Proj', 'CallMeteor_Exp_Proj', 'FlareForce_ExpProj', 'GoofyShoot_Proj']) {
    const d = JSON.parse(fs.readFileSync(path.join(C, cls + '.uasset.json'), 'utf8'));
    const nm = (i) => (i < 0 ? (d.imports[-i - 1] || {}).objectName : i > 0 ? (d.exports[i - 1] || {}).objectName : null);
    const exp = (n) => d.exports.find((e) => e.objectName === n);
    const props = (e) => (e && e.data && e.data.props) || [];
    const roles = { body: [], fire: [], hit: [] };
    const tmpl = props(exp('ParticleComp')).find((p) => p.name === 'Template');
    if (tmpl && tmpl.value) roles.body.push(nm(tmpl.value));
    const cdo = d.exports.find((e) => e.objectName.startsWith('Default__'));
    for (const p of props(cdo)) {
      if (p.name !== 'MyFireEffects') continue;
      for (const item of (p.value && p.value.items) || []) for (const q of item) if (q.name === 'MyParticleSystem' && q.value) roles.fire.push(nm(q.value));
    }
    for (const p of props(exp('TresAtkColl0'))) {
      if (p.name !== 'm_HitEffects') continue;
      for (const item of (p.value && p.value.items) || []) for (const q of item) if (q.name === 'm_PawnHitEffect' && q.value) roles.hit.push(nm(q.value));
    }
    out[cls + '_C'] = roles;
  }
  fs.mkdirSync(OUT_PACK, { recursive: true });
  fs.writeFileSync(path.join(OUT_PACK, 'team_host.json'), JSON.stringify({ format: 'kh3-party-team-host/1', generator: 'tools/party_team_pack.js', projectiles: out }, null, 1));
  console.log('team_host.json', JSON.stringify(out));
}

main();
writeTeamHost();

// (D3) build/sora_pack/party/team/props/: the team props' meshes and diffuse textures for kh-app's minimal
// prop draw (party_draw.rs, "team props"): the converted glb (tools/lib/mesh.js: UE_to_glTF root, cm under
// it) as is, its first material's diffuse (d00) decoded to PNG, and the mesh component's relative rotation
// of the projectile blueprint (UE rotator) [A].
function writeProps() {
  const bc = require('./lib/bcdec');
  const MESH = path.join(ROOT, 'assets_converted/meshes/KINGDOM HEARTS III/Content/Character/obj');
  const TEX = path.join(ROOT, 'assets_converted/textures/KINGDOM HEARTS III/Content/Character/obj');
  const props = [
    { class: 'CallMeteor_Proj_C', model: 'o_ex210_0', obj: 'o_ex210', component: 'TresProjectileStaticMesh0', bp: 'CallMeteor_Proj' },
    { class: 'FlareForce_Proj_C', model: 'o_ex208_0', obj: 'o_ex208', component: 'TresCharMesh0', bp: 'FlareForce_Proj' },
  ];
  const dir = path.join(OUT_PACK, 'props');
  fs.mkdirSync(dir, { recursive: true });
  const out = {};
  for (const p of props) {
    fs.copyFileSync(path.join(MESH, p.obj, 'mdl', p.model + '.glb'), path.join(dir, p.model + '.glb'));
    const dds = path.join(TEX, p.obj, 'mdl', 'mat0', p.model + 'd00.dds');
    const t = bc.decodeDds(fs.readFileSync(dds), 0);
    fs.writeFileSync(path.join(dir, p.model + 'd00.png'), bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb: true }));
    const d = JSON.parse(fs.readFileSync(path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content/Blueprints/FriendLink', p.bp + '.uasset.json'), 'utf8'));
    const comp = d.exports.find((e) => e.objectName === p.component);
    const rr = ((comp && comp.data && comp.data.props) || []).find((q) => q.name === 'RelativeRotation');
    const v = rr && rr.value ? (Array.isArray(rr.value) ? Object.fromEntries(rr.value.map((q) => [q.name, q.value])) : rr.value) : {};
    out[p.class] = { glb: `party/team/props/${p.model}.glb`, diffuse: `party/team/props/${p.model}d00.png`, relative_rotation: [v.pitch || v.Pitch || 0, v.yaw || v.Yaw || 0, v.roll || v.Roll || 0], source: `Character/obj/${p.obj}/mdl/${p.model} (${p.bp} ${p.component})` };
  }
  fs.writeFileSync(path.join(dir, 'props.json'), JSON.stringify({ format: 'kh3-party-team-props/1', generator: 'tools/party_team_pack.js', props: out }, null, 1));
  console.log('props.json', JSON.stringify(out));
}
writeProps();

// (party D4) THE TEAM PROPS' REAL MATERIALS: the game's own translated base-pass shaders, as Donald's and Goofy's (tools/
// party_pack.js buildMaterials): tools/sora_material_pack.js in outfit mode over the props' two material instances
//   o_ex208_mat0_Inst (missile; parent SQEX_OBJ_Basemat_MNL_Inst: BaseColorTex d00, NormalTex n00, MaskTexture m00,
//                      AOTexture o00, MetalMaskChannell G, NunoMaskChannell B, index scalars) [A]
//   o_ex210_mat0_Inst (meteor; parent ymt_mtr_lit_00o, masked: Diffuse d00, Normal n00, TripleMask s00) [A]
// into build/sora_pack/party/team/materials/ (merged by kh-app char_material.rs merge_party as "team"), then: the Kingdom
// Key's records dropped, slot -> its own mesh (o_ex208_0 / o_ex210_0), owner ids renumbered from 300 (Sora 0.., Donald 100..,
// Goofy 200.. share one shader source), the KHS post-process function removed (Sora's library has it), mesh facts per mesh.
// The meteor's depth-only (mask) shader reads an indexable temp (x0[], the SQEXDitherMatrixTemporalAA matrix of its Fade
// dither) the translator does not take: it draws without the dither clip (Fade = CameraFade = 1: nothing is clipped) [D].
// The missile has no animation of its own: FlareForce_Proj's TresCharMesh0 has no AnimClass and Character/obj/o_ex208 has
// no anm folder [A], so its reference pose (the glb's bind pose) is the game's pose.
function writePropMaterials() {
  const cp = require('child_process');
  const ID_BASE = 300;
  const props = [
    { mesh: 'o_ex208_0', slot: 'o_ex208_mat0', instance: '/Game/Character/obj/o_ex208/mdl/mat0/o_ex208_mat0_Inst.o_ex208_mat0_Inst', content: 'Character/obj/o_ex208/mdl/o_ex208_0', glb: 'props/o_ex208_0.glb' },
    { mesh: 'o_ex210_0', slot: 'o_ex210_mat0', instance: '/Game/Character/obj/o_ex210/mdl/mat0/o_ex210_mat0_Inst.o_ex210_mat0_Inst', content: 'Character/obj/o_ex210/mdl/o_ex210_0', glb: 'props/o_ex210_0.glb' },
  ];
  const manifest = { format: 'kh3-party-team-props/1', generator: 'tools/party_team_pack.js', note: 'outfit-mode input of tools/sora_material_pack.js for the team props (D4)',
    mesh_content: props[0].content, wearform_set: null, items: [{ kind: 'mesh', file: props[0].glb }], materials: props.map((p) => ({ slot: 0, slot_name: p.slot, material_instance: p.instance })) };
  fs.writeFileSync(path.join(OUT_PACK, 'manifest.json'), JSON.stringify(manifest, null, 1));
  const r = cp.spawnSync(process.execPath, [path.join(__dirname, 'sora_material_pack.js')], { env: { ...process.env, KH_OUTFIT_PACK: OUT_PACK }, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error('sora_material_pack.js failed for the team props: ' + r.stderr + (r.stdout || '').slice(-2000));
  for (const l of (r.stdout || '').split(/\r?\n/)) if (/ok \(|NOT|mask no|stand-in|missing|outfit/.test(l)) console.log('  ' + l.trim());
  const jf = path.join(OUT_PACK, 'materials', 'materials.json'), wf = path.join(OUT_PACK, 'materials', 'materials.wgsl');
  const json = JSON.parse(fs.readFileSync(jf, 'utf8')); let wgsl = fs.readFileSync(wf, 'utf8');
  const meshOf = new Map(props.map((p) => [p.slot, p.mesh]));
  for (const [k, v] of Object.entries(json.materials)) { if (v.mesh === 'kingdom_key' || !meshOf.has(k)) delete json.materials[k]; else v.mesh = meshOf.get(k); }
  const used = new Set(Object.values(json.materials).map((v) => v.owner));
  const idMap = new Map();
  for (const [name, o] of Object.entries(json.owners)) { if (!used.has(name)) { delete json.owners[name]; continue; } idMap.set(o.id, ID_BASE + o.id); o.id = ID_BASE + o.id; }
  for (const v of Object.values(json.materials)) v.id = json.owners[v.owner] ? json.owners[v.owner].id : v.id;
  const blocks = [...wgsl.matchAll(/#ifdef KH_C(\d+)\n([\s\S]*?)\n#endif\n/g)];
  wgsl = '// GENERATED by tools/party_team_pack.js (party D4) from tools/sora_material_pack.js (outfit mode) for the team props: owner ids ' + ID_BASE + '+. Do not edit.\n\n' +
    blocks.filter((b) => idMap.has(+b[1])).map((b) => `#ifdef KH_C${idMap.get(+b[1])}\n${b[2]}\n#endif\n`).join('\n');
  fs.writeFileSync(wf, wgsl);
  const uv2 = {};
  for (const [k, v] of Object.entries(json.uv2 || {})) { const [mesh, slot] = k.split(':'); if (mesh === 'kingdom_key') { const f = path.join(OUT_PACK, 'materials', v.file); if (fs.existsSync(f)) fs.unlinkSync(f); continue; } uv2[`${meshOf.get(slot) || mesh}:${slot}`] = v; }
  json.uv2 = uv2;
  // mesh facts per mesh: the missile's from the tool (bone table, bounds), the static meteor's bounds from its mesh asset
  const facts = {};
  if (json.mesh_facts && json.mesh_facts.sora) facts[props[0].mesh] = json.mesh_facts.sora;
  const sm = JSON.parse(fs.readFileSync(path.join(ROOT, 'assets_converted/meshes/KINGDOM HEARTS III/Content', props[1].content + '.mesh.json'), 'utf8'));
  const b = sm.data && sm.data.bounds;
  facts[props[1].mesh] = { source: 'assets_converted/meshes/KINGDOM HEARTS III/Content/' + props[1].content + '.mesh.json (data.bounds)', bone_params: [], bounds: Array.isArray(b) && b.length === 7 ? { origin: b.slice(0, 3), extent: b.slice(3, 6), radius: b[6] } : null };
  json.mesh_facts = facts;
  json.khs = null; json.wear_forms = null;
  json.party = { member: 0, npc: 'team', id_base: ID_BASE, generator: 'tools/party_team_pack.js (party D4)', note: 'the team props (meteor o_ex210_0, missile o_ex208_0): merged into the app\'s material library by kh-app char_material.rs (party A merge_party); drawn as Instance::Party(3) by party_draw.rs team_props' };
  fs.writeFileSync(jf, JSON.stringify(json));
  // the textures only the dropped records used (the Kingdom Key's, the KHS tables)
  const keep = new Set(Object.values(json.materials).flatMap((v) => (v.textures || []).map((t) => t && t.file).filter(Boolean)));
  const tdir = path.join(OUT_PACK, 'materials', 'textures');
  for (const t of fs.existsSync(tdir) ? fs.readdirSync(tdir) : []) if (!keep.has('textures/' + t)) fs.unlinkSync(path.join(tdir, t));
  const owners = Object.values(json.owners);
  console.log(`team prop materials: ${Object.keys(json.materials).length} slots, ${owners.length} shader owners, ${owners.filter((o) => o.base && o.base.ok).length} translated; masks ${owners.map((o) => (o.mask ? (o.mask.ok ? 'ok' : 'no') : '-')).join('/')}`);
}
writePropMaterials();

// (party D5) GOOFY BOMBARDIER: THE LINK PAWN'S ANIMATED SOCKETS. ATresFriendLinkPawnGoofyShoot's Appear tick 0x1412D6F90
// (sub 1) attaches every participant with vt+0x23A8 (0x1405A3FA0 -> AActor::AttachToComponent 0x143E8FC60, rules
// DAT_148750ED4 = 02 02 01 00 = SnapToTarget location / rotation, KeepWorld scale) to the pawn's mesh (vt+0xB58) at
// "X_Link01_skt", Goofy (vt+0xD70 == 12) at "X_Link02_skt" [C]. The pawn's mesh o_at001_0 (GoofyShoot_Pawn TresCharMesh0,
// RelativeRotation yaw 270 [A]) plays its own o_at001 clips 1636..1639 (GShootStart / GSHootRise / GShootHit / GShootFinish)
// in play modes 2 / 3 = with root motion (docs/sora-actions-spec.md, state interface vt+0x18) [C]; the clips have
// bEnableRootMotion and the default RootMotionRootLock RefPose (UE 4.17) [A]: the root kh_sk is held at its reference pose
// in the drawn pose and its track moves the ACTOR. The proof in the data: under that rule the sockets join 1638's last
// frame to 1639's first exactly (with kh_sk left in the pose they would jump 11 m) [A].
// Per clip frame (the clip's own frame grid, NumFrames over SequenceLength) in the pawn ACTOR frame (the mesh's default
// relative rotation applied, UE cm, no extra mesh pitch): `root` = the kh_sk track's translation (the root motion source),
// `s1` / `q1` = X_Link01_skt location / rotation (Sora), `s2` / `q2` = X_Link02_skt (Goofy), the pose's kh_sk at its
// reference pose. Key interpolation: linear, quaternions normalised-linear (the keys sit on the frame grid) [A].
function writeGShootSockets() {
  const sp = require('./sora_pack.js');
  const OBJ = 'assets_converted/animations/KINGDOM HEARTS III/Content/Character/obj/o_at001/anm/rt/';
  const mesh = sp.readGlb(path.join(ROOT, 'assets_converted/meshes/KINGDOM HEARTS III/Content/Character/obj/o_at001/mdl/o_at001_0.glb'));
  const bones = [], par = new Map();
  mesh.json.nodes.forEach((n, i) => (n.children || []).forEach((c) => par.set(c, i)));
  for (let i = 1; i < mesh.json.nodes.length; i++) {
    const n = mesh.json.nodes[i];
    if (n.mesh !== undefined) break;
    bones.push({ name: n.name, parent: par.get(i) - 1, t: n.translation || [0, 0, 0], r: n.rotation || [0, 0, 0, 1], s: n.scale || [1, 1, 1] });
  }
  const qmul = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
  const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
  // FTransform child * parent (UE): location = parent.R (parent.S x child.T) + parent.T, rotation = parent.R child.R
  const under = (c, p) => { const v = qrot(p.r, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { r: qmul(p.r, c.r), t: [v[0] + p.t[0], v[1] + p.t[1], v[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
  const rotQ = (pitch, yaw, roll) => { const h = Math.PI / 360, sp2 = Math.sin(pitch * h), cp2 = Math.cos(pitch * h), sy = Math.sin(yaw * h), cy = Math.cos(yaw * h), sr = Math.sin(roll * h), cr = Math.cos(roll * h); return [cr * sp2 * sy - sr * cp2 * cy, -cr * sp2 * cy - sr * cp2 * sy, cr * cp2 * sy - sr * sp2 * cy, cr * cp2 * cy + sr * sp2 * sy]; };
  // the sockets (o_at001_0_Skeleton SkeletalMeshSocket exports) and the mesh component's default relative rotation [A]
  const skel = JSON.parse(fs.readFileSync(path.join(CONTENT, 'Character/obj/o_at001/mdl/o_at001_0_Skeleton.uasset.json'), 'utf8'));
  const sockets = {};
  for (const e of skel.exports.filter((x) => x.class === 'SkeletalMeshSocket')) {
    const p = Object.fromEntries(e.data.props.map((q) => [q.name, q.value]));
    const r = p.RelativeRotation || { pitch: 0, yaw: 0, roll: 0 }, l = p.RelativeLocation || { x: 0, y: 0, z: 0 };
    sockets[p.SocketName] = { bone: bones.findIndex((b) => b.name === p.BoneName), t: [l.x, l.y, l.z], r: rotQ(r.pitch, r.yaw, r.roll), src: r };
  }
  const pawn = JSON.parse(fs.readFileSync(path.join(CONTENT, 'Blueprints/FriendLink/GoofyShoot_Pawn.uasset.json'), 'utf8'));
  const meshComp = pawn.exports.find((e) => e.objectName === 'TresCharMesh0');
  const rr = ((meshComp.data && meshComp.data.props) || []).find((q) => q.name === 'RelativeRotation').value;
  const def = { t: [0, 0, 0], r: rotQ(rr.pitch, rr.yaw, rr.roll), s: [1, 1, 1] };
  const num = (x) => { if (!Number.isFinite(x)) throw new Error('nan'); const s = String(+x.toFixed(4)); return /[.eE]/.test(s) ? s : s + '.0'; };
  const vec = (a) => '[' + a.map(num).join(', ') + ']';
  const clips = [['GShootStart', 1636], ['GSHootRise', 1637], ['GShootHit', 1638], ['GShootFinish', 1639]];
  let rs = '// GENERATED by tools/party_team_pack.js (party D5, writeGShootSockets) from o_at001_0 / its Skeleton sockets / rt/1636..1639 /\n';
  rs += '// GoofyShoot_Pawn TresCharMesh0 - DO NOT EDIT. Included by crates/kh-party/src/team/link.rs (types GShootClip / GShootFrame).\n';
  rs += `// Mesh default relative rotation R(${rr.pitch}, ${rr.yaw}, ${rr.roll}); sockets X_Link01_skt / X_Link02_skt R(${JSON.stringify(sockets.X_Link01_skt.src)}); actor frame, UE cm.\n\n`;
  rs += 'pub static GSHOOT_LINK_CLIPS: &[GShootClip] = &[\n';
  const report = [];
  for (const [name, id] of clips) {
    const g = sp.readGlb(path.join(ROOT, OBJ + id + '.glb'));
    const side = JSON.parse(fs.readFileSync(path.join(ROOT, OBJ + id + '.anim.json'), 'utf8'));
    const pr = Object.fromEntries(side.data.props.map((x) => [x.tag.name, x.tag.type === 'BoolProperty' ? x.tag.boolVal === 1 : x.value]));
    if (pr.bEnableRootMotion !== true || pr.RootMotionRootLock) throw new Error(`${id}: root-motion settings changed`);
    const a = g.json.animations[0];
    const tracks = new Map(); // bone -> { translation / rotation / scale: { times, values } }
    for (const ch of a.channels) {
      const s = a.samplers[ch.sampler];
      const bi = bones.findIndex((b) => b.name === g.json.nodes[ch.target.node].name);
      if (!tracks.has(bi)) tracks.set(bi, {});
      tracks.get(bi)[ch.target.path] = { times: sp.readAccessor(g, s.input).data, values: sp.readAccessor(g, s.output).data };
    }
    const sample = (tr, n, t) => {
      const T = tr.times; let k = 0; while (k < T.length - 2 && T[k + 1] <= t) k++;
      const k2 = Math.min(k + 1, T.length - 1), f = T.length > 1 && T[k2] > T[k] ? Math.min(1, Math.max(0, (t - T[k]) / (T[k2] - T[k]))) : 0;
      const A = [...tr.values.slice(k * n, k * n + n)], B = [...tr.values.slice(k2 * n, k2 * n + n)];
      if (n === 4 && A[0] * B[0] + A[1] * B[1] + A[2] * B[2] + A[3] * B[3] < 0) for (let i = 0; i < 4; i++) B[i] = -B[i];
      const v = A.map((x, i) => x * (1 - f) + B[i] * f);
      if (n === 4) { const l = Math.hypot(...v); return v.map((x) => x / l); }
      return v;
    };
    const frames = [];
    const N = pr.NumFrames, L = pr.SequenceLength;
    for (let i = 0; i < N; i++) {
      const t = N > 1 ? (i * L) / (N - 1) : 0;
      const comp = [];
      let rootT = null;
      bones.forEach((b, bi) => {
        const tr = tracks.get(bi) || {};
        const local = { t: tr.translation ? sample(tr.translation, 3, t) : b.t, r: tr.rotation ? sample(tr.rotation, 4, t) : b.r, s: tr.scale ? sample(tr.scale, 3, t) : b.s };
        if (bi === 0) { rootT = local.t; local.t = b.t; local.r = b.r; local.s = b.s; } // RootMotionRootLock::RefPose
        comp.push(b.parent < 0 ? local : under(local, comp[b.parent]));
      });
      const sock = (nm) => under(under({ t: sockets[nm].t, r: sockets[nm].r, s: [1, 1, 1] }, comp[sockets[nm].bone]), def);
      const s1 = sock('X_Link01_skt'), s2 = sock('X_Link02_skt');
      frames.push({ root: qrot(def.r, rootT), s1: s1.t, q1: s1.r, s2: s2.t, q2: s2.r });
    }
    const last = frames[frames.length - 1];
    report.push(`${id} ${name}: ${N} frames ${L.toFixed(4)} s, root ${frames[0].root.map((x) => x.toFixed(0))} -> ${last.root.map((x) => x.toFixed(0))}, X_Link01 ${frames[0].s1.map((x) => x.toFixed(0))} -> ${last.s1.map((x) => x.toFixed(0))}, X_Link02 ${frames[0].s2.map((x) => x.toFixed(0))} -> ${last.s2.map((x) => x.toFixed(0))}, q2 ${frames[0].q2.map((x) => x.toFixed(3))} -> ${last.q2.map((x) => x.toFixed(3))}`);
    rs += `    // rt/${id} ${name}: NumFrames ${N}, SequenceLength ${L}\n    GShootClip {\n        name: ${JSON.stringify(name)},\n        length: ${num(L)},\n        frames: &[\n`;
    for (const fr of frames) rs += `            GShootFrame { root: ${vec(fr.root)}, s1: ${vec(fr.s1)}, q1: ${vec(fr.q1)}, s2: ${vec(fr.s2)}, q2: ${vec(fr.q2)} },\n`;
    rs += '        ],\n    },\n';
  }
  rs += '];\n';
  const out = path.join(ROOT, 'build/gen/rust/party_team_gshoot.rs');
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, rs);
  for (const r of report) console.log(r);
  console.log('wrote ' + path.relative(ROOT, out));
}
writeGShootSockets();
