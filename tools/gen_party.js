#!/usr/bin/env node
// tools/gen_party.js (party stream A): the anim tables of DONALD and GOOFY as Rust data, from their packs
// (build/sora_pack/party/n_ex00x/manifest.json, tools/party_pack.js), in the types tools/gen_port.js emits for Sora
// (sora_anim.rs: RootKey, NotifyClass, EffectAttach, NotifyData, NotifyDef, SequenceData, BlendSpace2d, AnimData). The NPC
// sets use notify classes Sora's enum lacks, so the types are written ONCE for both characters (the notify classes of both
// sets and their weapons' clips) and every character file uses them:
//
//   build/gen/rust/party_anim_types.rs     the types + AnimTables (one character's tables as one value)
//   build/gen/rust/party_anim_n_ex001.rs   Donald: enum NpcAnimId (EVERY row of n_ex001_AnimSet, in set order), SEQUENCES,
//   build/gen/rust/party_anim_n_ex002.rs   ANIMS, FACE_ROWS, NOTIFY_ASSETS, TABLES (Goofy likewise)
//
// The character files expect the types in scope (`use super::types::*;` in the including module): docs/party-contracts.md A.2.
//
//   node tools/gen_party.js [--check] [--out-dir DIR]
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const { BANNER_MARK, GEN_ROOT } = require('./lib/genout');

const VERSION = '1.0.0';
const CHECK = process.argv.includes('--check');
const arg = (name) => { const i = process.argv.indexOf(name); return i > 0 && process.argv[i + 1] ? path.resolve(process.argv[i + 1]) : null; };
const OUT_DIR = arg('--out-dir') || path.join(ROOT, GEN_ROOT, 'rust');
const PARTY = path.join(ROOT, 'build', 'sora_pack', 'party');
const NPCS = ['n_ex001', 'n_ex002'];
const RESERVED = new Set(['as', 'break', 'const', 'continue', 'crate', 'else', 'enum', 'extern', 'false', 'fn', 'for', 'if', 'impl', 'in', 'let', 'loop', 'match', 'mod', 'move', 'mut', 'pub', 'ref', 'return', 'self', 'static', 'struct', 'super', 'trait', 'true', 'type', 'unsafe', 'use', 'where', 'while', 'async', 'await', 'dyn', 'abstract', 'become', 'box', 'do', 'final', 'macro', 'override', 'priv', 'typeof', 'unsized', 'virtual', 'yield', 'try']);
const fail = (msg) => { console.log(`gen_party: FAILED  ${msg}`); process.exit(1); };
const banner = (from) => `${BANNER_MARK}${from} by tools/gen_party.js (emitter ${VERSION}) - DO NOT EDIT: change the input and run node tools/gen_party.js\n`;
const lit = (s) => JSON.stringify(String(s));
const f32 = (n) => { n = Number(n); const s = String(Math.fround(n) === n ? n : Number(n.toPrecision(9))); return /[.eE]/.test(s) || /inf|nan/i.test(s) ? s : s + '.0'; };
function camel(name) {
  let c = String(name).split(/[^A-Za-z0-9]+/).filter(Boolean).map((p) => (/[a-z]/.test(p) ? p[0].toUpperCase() + p.slice(1) : p[0] + p.slice(1).toLowerCase())).join('');
  if (!c) c = 'Value'; if (/^[0-9]/.test(c)) c = '_' + c; if (c === 'Self' || RESERVED.has(c)) c += '_';
  return c;
}

// ---- notifies: the decoders of tools/gen_port.js (same NotifyData variants, same defaults)
const plain = (v) => {
  if (Array.isArray(v)) {
    if (v.length && v.every((x) => x && typeof x === 'object' && !Array.isArray(x) && 'name' in x && 'type' in x)) return Object.fromEntries(v.map((x) => [x.name + (x.arrayIndex ? `[${x.arrayIndex}]` : ''), plain(x.value)]));
    return v.map(plain);
  }
  if (v && typeof v === 'object') {
    if (Array.isArray(v.items) && 'structName' in v) return v.items.map(plain);
    if ('s' in v && 'enc' in v) return v.s;
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, plain(x)]));
  }
  return v;
};
const rstr = (s) => JSON.stringify(String(s));
const opt = (v, make) => (v === undefined || v === null || v === 'None' ? 'None' : `Some(${make(v)})`);
const label = (v) => String(v).replace(/^.*::/, '');
const num = (v, d) => f32(typeof v === 'number' ? v : d);
const bool = (v, d) => String(typeof v === 'boolean' ? v : d);
const classVariant = (c) => camel(String(c).replace(/_/g, ' ')).replace(/^Sqexsead/, 'SqexSead');
function makeDecoders(assetNames) {
  const asset = (v) => { assetNames.add(String(v)); return rstr(v); };
  const face = (p, used, slot) => { ['m_bIsLoop', 'm_AnimType', 'm_AnimName', 'm_bSameCheck', 'm_BlendInTime'].forEach((k) => used.add(k));
    return `NotifyData::Face { slot: ${slot}, anim_type: ${opt(p.m_AnimType, (v) => rstr(label(v)))}, anim_name: ${opt(p.m_AnimName, rstr)}, blend_in: ${num(p.m_BlendInTime, -1)}, looped: ${bool(p.m_bIsLoop, false)}, same_check: ${bool(p.m_bSameCheck, true)}, battle_face_end: false }`; };
  const voice = (p, used) => { ['m_RandomVoiceAssets', 'm_VoiceGroupName', 'm_VoiceAsset', 'FadeOutTime'].forEach((k) => used.add(k));
    const random = (plain(p.m_RandomVoiceAssets) || []).filter((r) => r && r.Voice);
    return `NotifyData::Voice { group: ${opt(p.m_VoiceGroupName, rstr)}, asset: ${opt(p.m_VoiceAsset, asset)}, random: &[${random.map((r) => `(${asset(r.Voice)}, ${Number.isInteger(r.RandomWeights) ? r.RandomWeights : 0})`).join(', ')}], fade_out: ${num(p.FadeOutTime, 0)} }`; };
  const sound = (p, used) => { ['Sound', 'AttachPointName', 'VolumeMultiplier', 'PitchMultiplier', 'FadeInDuration', 'FadeOutDuration', 'LoopEndSound', 'LoopEndSoundPitchMultiplier', 'bFollowOwnerVisible'].forEach((k) => used.add(k));
    return `NotifyData::Sound { sound: ${p.Sound ? asset(p.Sound) : '""'}, attach_point: ${opt(p.AttachPointName, rstr)}, volume: ${num(p.VolumeMultiplier, 1)}, pitch: ${num(p.PitchMultiplier, 1)}, fade_in: ${num(p.FadeInDuration, 0)}, fade_out: ${num(p.FadeOutDuration, 0)}, loop_end_sound: ${opt(p.LoopEndSound, asset)}, loop_end_pitch: ${num(p.LoopEndSoundPitchMultiplier, 1)}, follow_owner_visible: ${bool(p.bFollowOwnerVisible, false)} }`; };
  const autoSe = (p, used) => { used.add('FadeOutDuration'); const s = plain(p.PlaySettings) || {};
    return `NotifyData::AutoSe { motion: ${opt(s.MotionSoundType, (v) => rstr(label(v)))}, source: ${opt(s.SourceType, (v) => rstr(label(v)))}, fade_out: ${num(p.FadeOutDuration, 0)} }`; };
  const effect = (p) => {
    const entries = [];
    for (const g of plain(p.MetaAttachDataList) || []) for (const m of g.MetaAttachData || []) for (const e of m.EffectDataList || []) entries.push(`EffectAttach { effect: ${opt(e.AttachEffect, asset)}, attach_type: ${rstr(label(g.AttachType ?? 'None'))}, socket: ${rstr(m.SocketName && m.SocketName !== 'None' ? m.SocketName : (g.SocketName ?? 'None'))}, group_id: ${Number.isInteger(e.GroupID) ? e.GroupID : 0}, common: ${bool(m.bUseCommonEffect, false)} }`);
    for (const g of plain(p.AttachDataList) || []) { const datas = (g.AttachDatas || []).filter((x) => typeof x === 'string'); for (const d of datas.length ? datas : [null]) entries.push(`EffectAttach { effect: ${opt(d, asset)}, attach_type: ${rstr(label(g.AttachType ?? 'None'))}, socket: ${rstr(g.SocketName ?? 'None')}, group_id: ${Number.isInteger(g.GroupID) ? g.GroupID : 0}, common: ${bool(g.bUseCommonAttachData, false)} }`); }
    return `NotifyData::Effect { entries: &[${entries.join(', ')}] }`;
  };
  const D = {
    TresAnimNotifyState_Attack: (p, used) => { ['CollisionGrpName', 'm_AttackDataIDName', 'm_WeaponHitSEAliasID', 'AttackInterval', 'm_HitSEAsset', 'm_bOverrideHitEffect', 'm_HitEffectGrpName'].forEach((k) => used.add(k));
      return `NotifyData::Attack { group: ${rstr(p.CollisionGrpName ?? 'None')}, row: ${rstr(p.m_AttackDataIDName ?? 'None')}, hit_se: ${opt(p.m_WeaponHitSEAliasID, (v) => rstr(label(v)))}, interval: ${opt(p.AttackInterval, f32)}, hit_se_asset: ${opt(p.m_HitSEAsset, asset)}, override_hit_effect: ${bool(p.m_bOverrideHitEffect, false)}, hit_effect_group: ${opt(p.m_HitEffectGrpName, rstr)} }`; },
    TresAnimNotifyState_IK: (p, used) => { ['m_StartInterpolationTime', 'm_EndInterpolationTime', 'm_TargetAlpha'].forEach((k) => used.add(k));
      return `NotifyData::Ik { start_interpolation_time: ${num(p.m_StartInterpolationTime, 0)}, end_interpolation_time: ${num(p.m_EndInterpolationTime, 0)}, target_alpha: ${num(p.m_TargetAlpha, 1)} }`; },
    TresAnimNotify_Timing: (p, used) => { used.add('m_Param'); return `NotifyData::Param { param: ${opt(p.m_Param, String)} }`; },
    TresAnimNotifyState_BpEvent: (p, used) => { used.add('m_Param'); return `NotifyData::Param { param: ${opt(p.m_Param, String)} }`; },
    TresAnimNotify_Shot: (p, used) => { ['LocationIndex', 'm_bAttachFireEffect'].forEach((k) => used.add(k)); return `NotifyData::Shot { location_index: ${opt(p.LocationIndex, String)}, attach_fire_effect: ${bool(p.m_bAttachFireEffect, false)} }`; },
    TresAnimNotify_LandAnimPose: (p, used) => { used.add('m_LandAnimPose'); return `NotifyData::LandAnimPose { pose: ${opt(p.m_LandAnimPose, (v) => rstr(label(v)))} }`; },
    TresAnimNotify_LastAnimPose: (p, used) => { used.add('m_LastAnimPose'); return `NotifyData::LastAnimPose { pose: ${opt(p.m_LastAnimPose, (v) => rstr(label(v)))} }`; },
    TresAnimNotify_PlayVoice: voice, TresAnimNotifyState_PlayVoice: voice,
    SQEXSEADAnimNotify_PlaySound: sound, SQEXSEADAnimNotifyState_AttachSound: sound,
    SQEXSEADAnimNotify_PlayAutoSe: autoSe, SQEXSEADAnimNotifyState_PlayAutoSe: autoSe,
    TresAnimNotify_AttachEffect: (p) => effect(p), TresAnimNotifyState_AttachEffect: (p) => effect(p),
    TresAnimNotify_FaceAnim: (p, used) => { ['IsLoop', 'FaceAnimType', 'FaceAnimName', 'SameCheck', 'BlendInTime', 'm_bIsBattleFaceEnd'].forEach((k) => used.add(k));
      return `NotifyData::Face { slot: 0, anim_type: ${opt(p.FaceAnimType, (v) => rstr(label(v)))}, anim_name: ${opt(p.FaceAnimName, rstr)}, blend_in: ${num(p.BlendInTime, -1)}, looped: ${bool(p.IsLoop, false)}, same_check: ${bool(p.SameCheck, true)}, battle_face_end: ${bool(p.m_bIsBattleFaceEnd, false)} }`; },
    TresAnimNotify_FaceAnimEye: (p, used) => face(p, used, 1), TresAnimNotify_FaceAnimLip: (p, used) => face(p, used, 2),
  };
  for (const c of ['TresAnimNotify_PlayWeaponSwingSE', 'TresAnimNotifyState_PlayWeaponSwingSE', 'TresAnimNotify_PlayWeaponHitSE']) D[c] = (p, used) => {
    ['m_SoundAliasID', 'm_SoundAliasUnit', 'm_SocketName', 'm_bFollow', 'm_VolumeMultiplier', 'm_PitchMultiplier', 'm_FadeOutDuration'].forEach((k) => used.add(k));
    return `NotifyData::WeaponSe { hit: ${/Hit/.test(c)}, alias: ${rstr(label(p.m_SoundAliasID ?? 'None'))}, unit: ${rstr(label(p.m_SoundAliasUnit ?? 'None'))}, socket: ${opt(p.m_SocketName, rstr)}, follow: ${bool(p.m_bFollow, false)}, volume: ${num(p.m_VolumeMultiplier, 1)}, pitch: ${num(p.m_PitchMultiplier, 1)}, fade_out: ${num(p.m_FadeOutDuration, 0)} }`;
  };
  return D;
}
const trig = (n) => (n.trigger_time !== undefined ? n.trigger_time : n.time);
const endTrig = (n) => (n.end_trigger_time !== undefined ? n.end_trigger_time : n.time + (n.duration || 0));

// ---- read both packs
const packs = [];
for (const npc of NPCS) {
  const file = path.join(PARTY, npc, 'manifest.json');
  if (!fs.existsSync(file)) fail(`${path.relative(ROOT, file)} does not exist (node tools/party_pack.js)`);
  packs.push({ npc, man: JSON.parse(fs.readFileSync(file, 'utf8')) });
}
// the union of the notify classes (body, face, camp and gummi clips of both)
const classes = new Map();
for (const { man } of packs) for (const it of man.items) for (const n of it.notifies || []) { if (!n.class) continue; if (!classes.has(n.class)) classes.set(n.class, { variant: classVariant(n.class), count: 0, state: false, instant: false }); const c = classes.get(n.class); c.count++; if (n.kind === 'state') c.state = true; else c.instant = true; }
{ const seen = new Map(); for (const [game, c] of classes) { if (seen.has(c.variant)) fail(`notify classes ${game} and ${seen.get(c.variant)} both become ${c.variant}`); seen.set(c.variant, game); } }
const classList = [...classes].sort((a, b) => (a[0] < b[0] ? -1 : 1));

const outputs = [];
// ================================================================ the types
let types = banner('port/party-assets (build/sora_pack/party/n_ex00x/manifest.json: the notify classes of both packs)');
types += `\n/// The pack generators the tables were read from.\npub const PARTY_ANIM_SOURCES: &[&str] = &[${packs.map((p) => lit(`${p.npc}: ${p.man.generator}`)).join(', ')}];\n\n`;
types += '/// One frame of a root-motion track: seconds; translation since frame 0 in actor space at frame 0\n/// (cm: x forward, y right, z up); yaw since frame 0 (degrees, positive = turning right).\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct RootKey {\n    pub time: f32,\n    pub x: f32,\n    pub y: f32,\n    pub z: f32,\n    pub yaw: f32,\n}\n\n';
types += `/// The notify classes that occur in Donald's and Goofy's packed sequences (${classList.length}), by the game's class names without\n/// their underscores. \`game_name()\` is the class name as the asset has it.\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]\npub enum NotifyClass {\n`;
for (const [game, c] of classList) types += `    /// \`${game}\`: ${c.count} instance(s)${c.state && c.instant ? ' (as a state and as an instant notify)' : c.state ? ' (a notify state: begin / tick / end)' : ''}.\n    ${c.variant},\n`;
types += `}\n\nimpl NotifyClass {\n    pub const ALL: &'static [NotifyClass] = &[${classList.map(([, c]) => `NotifyClass::${c.variant}`).join(', ')}];\n\n    /// The class name in the game's assets.\n    pub const fn game_name(self) -> &'static str {\n        match self {\n`;
for (const [game, c] of classList) types += `            NotifyClass::${c.variant} => ${lit(game)},\n`;
types += '        }\n    }\n}\n\n';
types += `/// One effect of a \`TresAnimNotify(State)_AttachEffect\` (as kh_core::anim::EffectAttach).\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct EffectAttach {\n    pub effect: Option<&'static str>,\n    pub attach_type: &'static str,\n    pub socket: &'static str,\n    pub group_id: i32,\n    pub common: bool,\n}\n\n`;
types += `/// The decoded properties of a notify (the variants and defaults of kh_core::anim::NotifyData, tools/gen_port.js).\n#[derive(Clone, Copy, Debug, PartialEq)]\npub enum NotifyData {\n    None,\n    Attack { group: &'static str, row: &'static str, hit_se: Option<&'static str>, interval: Option<f32>, hit_se_asset: Option<&'static str>, override_hit_effect: bool, hit_effect_group: Option<&'static str> },\n    Ik { start_interpolation_time: f32, end_interpolation_time: f32, target_alpha: f32 },\n    Param { param: Option<i32> },\n    Shot { location_index: Option<i32>, attach_fire_effect: bool },\n    LandAnimPose { pose: Option<&'static str> },\n    LastAnimPose { pose: Option<&'static str> },\n    WeaponSe { hit: bool, alias: &'static str, unit: &'static str, socket: Option<&'static str>, follow: bool, volume: f32, pitch: f32, fade_out: f32 },\n    Sound { sound: &'static str, attach_point: Option<&'static str>, volume: f32, pitch: f32, fade_in: f32, fade_out: f32, loop_end_sound: Option<&'static str>, loop_end_pitch: f32, follow_owner_visible: bool },\n    AutoSe { motion: Option<&'static str>, source: Option<&'static str>, fade_out: f32 },\n    Voice { group: Option<&'static str>, asset: Option<&'static str>, random: &'static [(&'static str, i32)], fade_out: f32 },\n    Face { slot: u8, anim_type: Option<&'static str>, anim_name: Option<&'static str>, blend_in: f32, looped: bool, same_check: bool, battle_face_end: bool },\n    Effect { entries: &'static [EffectAttach] },\n}\n\n`;
types += `/// One notify of a sequence: \`time\` the trigger time, \`end\` the end trigger of a notify state (= \`time\` for an instant one), seconds.\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct NotifyDef {\n    pub class: NotifyClass,\n    pub state: bool,\n    pub time: f32,\n    pub end: f32,\n    pub data: NotifyData,\n    /// The serialized properties the decoder did not read: (name, value as JSON text).\n    pub props: &'static [(&'static str, &'static str)],\n}\n\n`;
types += `/// One animation sequence of a pack (as kh_core::anim::SequenceData). The legacy notify times are seconds; -1.0 = none.\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct SequenceData {\n    /// Asset name = the pack file \`anims/<name>.glb\` (or \`face/<name>.glb\`).\n    pub name: &'static str,\n    pub length: f32,\n    pub frames: u32,\n    pub root_motion_enabled: bool,\n    pub interpolation_time: f32,\n    pub jump_start: f32,\n    pub enable_change: f32,\n    pub reverse_leg: (f32, f32),\n    /// Per-frame root track where \`bEnableRootMotion\`; empty otherwise.\n    pub root: &'static [RootKey],\n    pub notifies: &'static [NotifyDef],\n    /// Clips with a hit window or a Shot notify: the default weapon's ATK_00 / ATK_01 per frame, ACTOR space, cm.\n    pub sockets: &'static [[f32; 6]],\n}\n\n`;
types += `/// A 2D blend space (as kh_core::anim::BlendSpace2d).\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct BlendSpace2d {\n    pub x_min: f32,\n    pub x_max: f32,\n    pub x_grid: usize,\n    pub y_min: f32,\n    pub y_max: f32,\n    pub y_grid: usize,\n    pub y_smoothing: f32,\n    pub samples: &'static [(f32, f32, usize)],\n    pub grid: &'static [&'static [(usize, f32)]],\n}\n\n`;
types += `/// One row of an NPC anim set (as kh_core::anim::AnimData; an NPC has no stage-1 play calls: stage1_mode is None, looping /\n/// root_motion_bit / root_motion false, blend_in = the asset's m_InterpolationTime).\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct AnimData {\n    pub name: &'static str,\n    pub stage1_mode: Option<u8>,\n    pub looping: bool,\n    pub root_motion_bit: bool,\n    pub root_motion: bool,\n    pub root_motion_enabled: bool,\n    pub blend_in: f32,\n    pub length: f32,\n    pub sample_lengths_differ: bool,\n    pub blend_space: bool,\n    pub axis_min: f32,\n    pub axis_max: f32,\n    pub axis_smoothing: f32,\n    pub all_animation: bool,\n    pub samples: &'static [(f32, usize)],\n    pub space2d: Option<&'static BlendSpace2d>,\n}\n\n`;
types += `/// One character's tables as one value (\`TABLES\` of party_anim_n_ex00x.rs).\n#[derive(Clone, Copy, Debug)]\npub struct AnimTables {\n    pub npc: &'static str,\n    pub sequences: &'static [SequenceData],\n    pub anims: &'static [AnimData],\n    /// Face anim-set row -> the sequence the face slot plays (a face blend-space row: its sample at the axes' origin).\n    pub face_rows: &'static [(&'static str, &'static str)],\n    /// Camp-menu (CM_Body / CM_Face) and gummi (GM_*) rows -> sequence.\n    pub camp_rows: &'static [(&'static str, &'static str)],\n    pub notify_assets: &'static [&'static str],\n}\n\nimpl AnimTables {\n    /// A row by the game's name.\n    pub fn anim(&self, name: &str) -> Option<&'static AnimData> {\n        self.anims.iter().find(|a| a.name == name)\n    }\n\n    /// A sequence by its asset name.\n    pub fn sequence(&self, name: &str) -> Option<&'static SequenceData> {\n        self.sequences.iter().find(|s| s.name == name)\n    }\n}\n`;
outputs.push(['party_anim_types.rs', types, `${classList.length} notify classes`]);

// ================================================================ per character
for (const { npc, man } of packs) {
  const items = new Map(man.items.filter((i) => i.kind === 'animation' || i.kind === 'face-animation').map((i) => [i.animation_name, i]));
  const tracks = (man.root_motion && man.root_motion.tracks) || {};
  const seqNames = [];
  const seqIndex = (name) => { if (!items.has(name)) fail(`${npc}: no animation item ${name}`); let i = seqNames.indexOf(name); if (i < 0) { i = seqNames.length; seqNames.push(name); } return i; };
  const anims = []; const variantNames = new Map();
  for (const a of man.logical_animations) {
    const p = a.play || {}; const two = a.class === 'BlendSpace';
    const samples = a.samples ? a.samples.map((s) => [s.value, seqIndex(s.animation_name), s.value_y]) : [[0, seqIndex(path.basename(a.file, '.glb'))]];
    if (!two) samples.sort((x, y) => x[0] - y[0]);
    const lengths = samples.map((s) => items.get(seqNames[s[1]]).duration_s);
    const differ = Math.max(...lengths) - Math.min(...lengths) > 1e-4;
    const enabled = samples.some((s) => items.get(seqNames[s[1]]).root_motion_enabled === true);
    let variant = String(a.name).replace(/[^A-Za-z0-9_]/g, '_'); if (/^[0-9]/.test(variant)) variant = 'A' + variant; if (RESERVED.has(variant) || variant === 'Self') variant += '_';
    if (variantNames.has(variant)) fail(`${npc}: rows ${a.name} and ${variantNames.get(variant)} both become ${variant}`); variantNames.set(variant, a.name);
    anims.push({ name: a.name, variant, rootEnabled: enabled, blendIn: p.asset_m_InterpolationTime || 0, length: differ ? Math.max(...lengths) : lengths[0], differ, blendSpace: !!a.samples, two,
      axisMin: a.axis ? a.axis.min : 0, axisMax: a.axis ? a.axis.max : 0, axisSmoothing: (a.input_filter && a.input_filter.window_s) || 0, samples, axisY: a.axis_y || null, axisGrid: a.axis ? a.axis.grid_num : 0, ySmoothing: (a.input_filter && a.input_filter.window_y_s) || 0, grid: a.grid_samples || [], allAnimation: /SQEX_AllAnimation/.test(a.notify_trigger_mode || '') });
  }
  // every other packed sequence (face, camp, gummi) gets an index too, so that its notifies are tables as well
  for (const name of items.keys()) seqIndex(name);
  const assetNames = new Set(); const DECODE = makeDecoders(assetNames);
  let notifyCount = 0;
  let out = banner(`port/party-assets (build/sora_pack/party/${npc}/manifest.json: logical_animations, items[].notifies, root_motion.tracks, attack_sockets)`);
  out += `\n/// The pack generator the tables were read from.\npub const ANIM_SOURCE: &str = ${lit(man.generator)};\npub const NPC: &str = ${lit(npc)};\n\n`;
  const rootTrack = (name, it) => { const tr = tracks[name] && tracks[name].bEnableRootMotion ? tracks[name].frames_data : []; return tr.map((k, i) => `RootKey { time: ${f32(i === tr.length - 1 ? it.duration_s : k.time)}, x: ${f32(k.translation_ue_actor_cm[0])}, y: ${f32(k.translation_ue_actor_cm[1])}, z: ${f32(k.translation_ue_actor_cm[2])}, yaw: ${f32(k.yaw_ue_deg)} }`).join(', '); };
  const find = (it, cls) => (it.notifies || []).find((n) => n.class === cls);
  out += 'pub static SEQUENCES: &[SequenceData] = &[\n';
  for (const name of seqNames) {
    const it = items.get(name);
    const js = find(it, 'TresAnimNotify_JumpStart'), ec = find(it, 'TresAnimNotify_EnableChange'), rl = find(it, 'TresAnimNotifyState_ReverseLeg');
    const sp = it.sequence_properties || {};
    out += `    SequenceData { name: ${lit(name)}, length: ${f32(it.duration_s)}, frames: ${it.num_frames || 0}, root_motion_enabled: ${it.root_motion_enabled === true}, interpolation_time: ${f32(sp.m_InterpolationTime || 0)}, jump_start: ${js ? f32(trig(js)) : '-1.0'}, enable_change: ${ec ? f32(trig(ec)) : '-1.0'}, reverse_leg: ${rl ? `(${f32(trig(rl))}, ${f32(endTrig(rl))})` : '(-1.0, -1.0)'}, root: &[${rootTrack(name, it)}],\n        notifies: &[\n`;
    for (const n of it.notifies || []) {
      if (!n.class) continue;
      const props = n.properties || {}; const used = new Set();
      const data = DECODE[n.class] ? DECODE[n.class](props, used) : 'NotifyData::None';
      const bag = Object.entries(props).filter(([k]) => !used.has(k)).map(([k, v]) => `(${rstr(k)}, ${rstr(JSON.stringify(plain(v)))})`);
      const state = n.kind === 'state';
      out += `            NotifyDef { class: NotifyClass::${classes.get(n.class).variant}, state: ${state}, time: ${f32(trig(n))}, end: ${f32(state ? endTrig(n) : trig(n))}, data: ${data}, props: &[${bag.join(', ')}] },\n`;
      notifyCount++;
    }
    const sk = ((man.attack_sockets || {}).clips || {})[name];
    out += `        ], sockets: &[${sk ? sk.sockets.map((r) => `[${r.map(f32).join(', ')}]`).join(', ') : ''}] },\n`;
  }
  out += '];\n\n';
  const spaces = anims.filter((a) => a.two);
  spaces.forEach((a, k) => {
    out += `static SPACE2D_${k}: BlendSpace2d = BlendSpace2d { x_min: ${f32(a.axisMin)}, x_max: ${f32(a.axisMax)}, x_grid: ${a.axisGrid}, y_min: ${f32(a.axisY.min)}, y_max: ${f32(a.axisY.max)}, y_grid: ${a.axisY.grid_num}, y_smoothing: ${f32(a.ySmoothing)}, samples: &[${a.samples.map((s) => `(${f32(s[0])}, ${f32(s[2])}, ${s[1]})`).join(', ')}], grid: &[${a.grid.map((g) => `&[${g.map((e) => `(${e.sample_index}, ${f32(e.weight)})`).join(', ')}]`).join(', ')}] };\n`;
    a.space = k;
  });
  out += '\n/// Every row of the body anim set, in the order of `NpcAnimId`.\npub static ANIMS: &[AnimData] = &[\n';
  for (const a of anims) out += `    AnimData { name: ${lit(a.name)}, stage1_mode: None, looping: false, root_motion_bit: false, root_motion: false, root_motion_enabled: ${a.rootEnabled}, blend_in: ${f32(a.blendIn)}, length: ${f32(a.length)}, sample_lengths_differ: ${a.differ}, blend_space: ${a.blendSpace}, axis_min: ${f32(a.axisMin)}, axis_max: ${f32(a.axisMax)}, axis_smoothing: ${f32(a.axisSmoothing)}, all_animation: ${a.allAnimation}, samples: &[${a.samples.map((s) => `(${f32(s[0])}, ${s[1]})`).join(', ')}], space2d: ${a.two ? `Some(&SPACE2D_${a.space})` : 'None'} },\n`;
  out += '];\n\n';
  out += `/// The rows of ${npc}_AnimSet by the game's names (a name that starts with a digit has the prefix \`A\`), in the set's order.\n#[allow(non_camel_case_types)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]\n#[repr(u16)]\npub enum NpcAnimId {\n`;
  anims.forEach((a, i) => { out += `    ${a.variant} = ${i},\n`; });
  out += `}\n\nimpl NpcAnimId {\n    pub const ALL: [NpcAnimId; ${anims.length}] = [${anims.map((a) => `NpcAnimId::${a.variant}`).join(', ')}];\n\n    /// By the game's row name ("IdleN", "M_1301").\n    pub fn from_name(name: &str) -> Option<NpcAnimId> {\n        ANIMS.iter().position(|a| a.name == name).map(|i| NpcAnimId::ALL[i])\n    }\n\n    pub fn data(self) -> &'static AnimData {\n        &ANIMS[self as usize]\n    }\n\n    pub fn name(self) -> &'static str {\n        ANIMS[self as usize].name\n    }\n}\n\n`;
  const stem = (f) => path.basename(String(f), '.glb');
  out += `/// Face anim-set rows -> the sequence the face slot plays.\npub static FACE_ROWS: &[(&str, &str)] = &[${(man.face_animations || []).map((f) => `(${lit(f.name)}, ${lit(stem(f.file))})`).join(', ')}];\n\n`;
  const campRows = [...(man.camp_animations.body || []), ...(man.camp_animations.face || []), ...(man.gummi_animations.body || []), ...(man.gummi_animations.face || [])].map((r) => [r.name + '@' + path.basename(String(r.anim_set).split('.')[0]), r.sequence || path.basename(String(r.asset).split('.')[0])]);
  out += `/// Camp-menu and gummi rows ("<row>@<anim set>") -> sequence.\npub static CAMP_ROWS: &[(&str, &str)] = &[${campRows.map(([a, b]) => `(${lit(a)}, ${lit(b)})`).join(', ')}];\n\n`;
  const assetList = [...assetNames].sort();
  out += `/// Every sound, voice and effect asset a notify names (sorted).\npub static NOTIFY_ASSETS: &[&str] = &[\n${assetList.map((n) => `    ${rstr(n)},\n`).join('')}];\n\n`;
  out += `pub static TABLES: AnimTables = AnimTables { npc: NPC, sequences: SEQUENCES, anims: ANIMS, face_rows: FACE_ROWS, camp_rows: CAMP_ROWS, notify_assets: NOTIFY_ASSETS };\n`;
  // ---- (B's request, contract B.7) the same rows in kh-party's own types: party_clips_<npc>.rs, `pub static
  // N_EX00x_ANIMS: AnimSetTable` (expects ClipDef, ClipNotify, ClipNotifyData, RootKey, SocketKey, AnimSetTable, Chara in
  // scope). Notify data as tools/party_sheet.js maps it (Attack / State / Raw); trigger times with their offsets; the root
  // track where bEnableRootMotion; the socket the weapon's attack collision uses (TresAtkColl0 of w_do_WepBase: ATK_01,
  // of w_gu_WepBase: ATK_00) per animation frame. Looping: the row-name convention of party_sheet.js [D].
  {
    const atk = npc === 'n_ex001' ? 1 : 0;
    const UP = npc.toUpperCase();
    let clips = banner(`port/party-assets (build/sora_pack/party/${npc}/manifest.json, in kh-party's B.7 types)`);
    clips += `\n/// ${npc}_AnimSet: every row (${anims.length}) as kh-party's ClipDef (contract B.7). A blend-space row: its first sample's sequence and the longest sample's length.\npub static ${UP}_CLIPS: &[ClipDef] = &[\n`;
    const tailState = (c) => c.replace(/^.*NotifyState_/, ''), tailRaw = (c) => c.replace(/^.*Notify_/, '');
    for (const a of anims) {
      const seq = seqNames[a.samples[0][1]]; const it = items.get(seq);
      const ns = (it.notifies || []).filter((n) => n.class).map((n) => {
        const state = n.kind === 'state'; const p = n.properties || {};
        const data = n.class === 'TresAnimNotifyState_Attack' ? `ClipNotifyData::Attack { attack: ${lit(p.m_AttackDataIDName ?? 'None')}, collision: ${lit(p.CollisionGrpName ?? p.m_AttackCollisionGrpName ?? 'None')} }` : state ? `ClipNotifyData::State { name: ${lit(tailState(n.class))} }` : `ClipNotifyData::Raw(${lit(tailRaw(n.class))})`;
        const s = trig(n), e = state ? Math.max(endTrig(n), s + 1e-4) : s;
        return [s, `ClipNotify { class: ${lit(n.class)}, start: ${f32(s)}, end: ${f32(e)}, data: ${data} }`];
      }).sort((x, y) => x[0] - y[0]).map((x) => x[1]);
      const tr = tracks[seq] && tracks[seq].bEnableRootMotion ? tracks[seq].frames_data : [];
      const root = tr.map((k, i) => `RootKey { time: ${f32(i === tr.length - 1 ? it.duration_s : k.time)}, x: ${f32(k.translation_ue_actor_cm[0])}, y: ${f32(k.translation_ue_actor_cm[1])}, z: ${f32(k.translation_ue_actor_cm[2])}, yaw: ${f32(k.yaw_ue_deg)} }`).join(', ');
      const sk = ((man.attack_sockets || {}).clips || {})[seq];
      const socket = sk ? sk.sockets.map((r, i) => `SocketKey { time: ${f32(sk.frames > 1 ? i * sk.duration_s / (sk.frames - 1) : 0)}, x: ${f32(r[atk * 3])}, y: ${f32(r[atk * 3 + 1])}, z: ${f32(r[atk * 3 + 2])} }`).join(', ') : '';
      const looping = /Loop|Idle|^Swim|Dive|Walk|RunLoop/.test(a.name) && !/End|Start/.test(a.name);
      const rate = (it.sequence_properties && it.sequence_properties.RateScale) ?? 1;
      clips += `    ClipDef { name: ${lit(a.name)}, sequence: ${lit(seq)}, length: ${f32(a.length)}, rate: ${f32(rate)}, blend_in: ${f32(a.blendIn)}, looping: ${looping}, root_motion: ${it.root_motion_enabled === true}, notifies: &[${ns.join(', ')}], root: &[${root}], socket: &[${socket}] },\n`;
    }
    clips += `];\n\npub static ${UP}_ANIMS: AnimSetTable = AnimSetTable { chara: Chara::${npc === 'n_ex001' ? 'Donald' : 'Goofy'}, clips: ${UP}_CLIPS };\n`;
    outputs.push([`party_clips_${npc}.rs`, clips, `${anims.length} ClipDefs (kh-party B.7 types), socket ${atk ? 'ATK_01' : 'ATK_00'}`]);
  }
  outputs.push([`party_anim_${npc}.rs`, out, `${anims.length} rows (${anims.filter((a) => a.blendSpace && !a.two).length} 1D, ${spaces.length} 2D blend spaces), ${seqNames.length} sequences, ${seqNames.filter((n) => tracks[n] && tracks[n].bEnableRootMotion).length} root tracks, ${notifyCount} notifies, ${Object.keys(((man.attack_sockets || {}).clips) || {}).length} socket tracks, ${assetList.length} notify assets`]);
}

// ================================================================ write
let differs = 0;
fs.mkdirSync(OUT_DIR, { recursive: true });
for (const [name, body, what] of outputs) {
  const file = path.join(OUT_DIR, name);
  const old = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : null;
  const same = old === body;
  if (!same) differs++;
  if (!CHECK && !same) fs.writeFileSync(file, body);
  console.log(`gen_party: ${CHECK ? (same ? 'same     ' : 'DIFFERS  ') : same ? 'unchanged' : 'written  '} ${name}: ${what}`);
}
if (CHECK && differs) process.exit(1);
