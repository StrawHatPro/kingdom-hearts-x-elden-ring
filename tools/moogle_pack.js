#!/usr/bin/env node
// tools/moogle_pack.js (P8 MOOGLE NPC, documentation/KH3_WEAPONS_PLAN.md 1.5): KH3'S SYNTHESIS MOOGLE (n_ex014, the standing
// Moogle of /Game/Blueprints/Gimmick/ex/g_ex_mogli_01, class TresShopActor) as a pack in the friends' layout, so that the
// Elden Ring renderer's party path draws him (kh-er-render gfx/party.rs: PACK/party/<npc>/model.glb + manifest.json +
// materials/) and kh_anim::Pack::load + Animator pose him (his idle clips, his Bonamik on the pom-pom and the wings).
//
//   node tools/moogle_pack.js                    build build/sora_pack/party/n_ex014/ (+ audio/), from the packed KH3 Moogle
//   node tools/moogle_pack.js --pack DIR         another pack root (default build/sora_pack)
//   node tools/moogle_pack.js --out DIR          another output folder (default PACK/party/n_ex014)
//   node tools/moogle_pack.js --no-audio         skip the sound root (PACK/party/n_ex014/audio)
//
// Inputs (read only): the asset agent's Moogle (PACK/npc/n_ex014: model.glb, skeleton.json, model.json, anims/*.glb, written by
// tools/sora_keyblades_pack.js) and its translated materials (PACK/npc/shop/materials); the decoded packages of the Moogle's
// Bonamik asset (Character/npc/n_ex014/mdl/n_ex014_Bonamik) and mesh (n_ex014_0); the shop data asset UI/Data/ShopDataAsset
// (the Moogle shop's TalkWelcome / TalkNewArrival voice rows, se00000_moogle1..5).
// Outputs (NEW files only): PACK/party/n_ex014/{manifest.json, model.glb, anims/001.glb, anims/002.glb, data/bonamik.json,
// materials/materials.json, materials/materials.wgsl, materials/textures/*.png, audio/manifest.json, audio/wav/...}.
// The DXIL of the materials is built after this by kh-shaderc (party mode):
//   target/release/kh-shaderc --pack build/sora_pack/party/n_ex014 --party --main-pack build/sora_pack
//   target/release/kh-shaderc --pack build/sora_pack/party/n_ex014 --party --main-pack build/sora_pack --only velo88
// Nothing else of the pack is written (PACK/party/party.json stays as tools/party_pack.js wrote it: the renderer knows the
// Moogle's folder by its draw id, kh-er-render party.rs `MOOGLE`).
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const sp = require('./sora_pack.js');
const { bonamikExport } = require('./sora_weapon_pack.js');

const VERSION = '1.0.0';
const P = (...a) => path.join(...a);
const NL = String.fromCharCode(10);
const args = process.argv.slice(2);
const value = (name) => { const k = args.indexOf(name); return k >= 0 && args[k + 1] && !args[k + 1].startsWith('--') ? args[k + 1] : null; };
const PACK = path.resolve(value('--pack') || sp.OUT);
// (P8 fix, the user 2026-11-03: "the moogle in round table hold is spazzing out, his actual position should be stationary, floating in
// the air with his floating animation") TWO KH3 MOOGLES: n_ex028 (g_ex_mogli_02, TresShopActorMogli) is KH3's FIELD Moogle shop, the
// one every world places (he_03, fz_01, bx_03, ca_02 .. gameflow levels): his mesh component sits 75 cm above the actor
// (TresGimmickMesh0 RelativeLocation z 75) and his loop M_010 floats (hara1 / koshi bob, CharaWind); M_001 is his arrival (the root
// rises 43 cm into place, Bonamik_ReinitializeSimulationPose). n_ex014 (g_ex_mogli_01, TresShopActor) stands on the floor. The
// default is n_ex028 (`--npc n_ex014` builds the standing one).
const MODELS = {
  // (P8 fix 2, THE USER 2026-11-03: "Remove the balloon") the SHOP balloon and its string: material slot n_ex028_mat0_inst (its
  // primitive skins only balloon1, himo_up1..3, himo_dawn1: nothing of the body) and the Bonamik group "balloon" (himo_*,
  // balloon1 / 2) are left out; the string hangs from R_buki (his right hand)
  n_ex028: { drop_slots: ['n_ex028_mat0_inst'], drop_groups: ['balloon'], gimmick: 'Blueprints/Gimmick/ex/g_ex_mogli_02', native: 'TresShopActorMogli', idle: 'M_010', idle_why: 'his looping float (CharaWind, hara1 / koshi bob); M_001 is the arrival (root +43 cm into place, Bonamik reinit): g_ex_mogli_02 serializes no m_DefaultIdlingMotName' },
  n_ex014: { gimmick: 'Blueprints/Gimmick/ex/g_ex_mogli_01', native: 'TresShopActor', idle: null, idle_why: 'the m_DefaultIdlingMotName of the gimmick blueprint' },
};
const NPC = value('--npc') || 'n_ex028';
if (!MODELS[NPC]) throw new Error(`--npc ${NPC}: n_ex028 or n_ex014`);
const OUT = path.resolve(value('--out') || P(PACK, 'party', NPC));
const NO_AUDIO = args.includes('--no-audio');
const SRC = P(PACK, 'npc', NPC);
const SHOP = P(PACK, 'npc', 'shop', 'materials');
const MESH_CONTENT = `Character/npc/${NPC}/mdl/${NPC}_0`;
const BONAMIK_CONTENT = `Character/npc/${NPC}/mdl/${NPC}_Bonamik`;
const GIMMICK = '/Game/' + MODELS[NPC].gimmick;
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8').replace(/^﻿/, ''));
const rel = (f) => path.relative(sp.ROOT, f).split(path.sep).join('/');

const written = [];
function put(file, buf) {
  const at = P(OUT, file);
  fs.mkdirSync(path.dirname(at), { recursive: true });
  fs.writeFileSync(at, buf);
  written.push({ file: file.split(path.sep).join('/'), bytes: buf.length, sha256: sha256(buf) });
}
const putJson = (file, v) => put(file, Buffer.from(JSON.stringify(v, null, 1) + NL));

// ---------------------------------------------------------------------------------------------- 1. the KH3 Moogle's facts
// the gimmick blueprint: its idle row (m_DefaultIdlingMotName), its mesh component's turn, its reactor (the talk spot)
function gimmickFacts() {
  const d = sp.loadPkg(MODELS[NPC].gimmick);
  const props = (e) => Object.fromEntries(((e && e.data && e.data.props) || []).map((p) => [p.name, p.value]));
  const cdo = props(d.exports.find((e) => /^Default__/.test(e.objectName)));
  const mesh = props(d.exports.find((e) => e.class === 'TresSkeletalMeshComponent'));
  const reactor = props(d.exports.find((e) => e.class === 'TresReactorComponent'));
  return {
    blueprint: GIMMICK, native_class: MODELS[NPC].native, shop_id: cdo.m_ShopID || null, default_idling: cdo.m_DefaultIdlingMotName || MODELS[NPC].idle, default_idling_why: MODELS[NPC].idle_why,
    mesh_component: { name: 'TresGimmickMesh0', relative_rotation_ue_deg: mesh.RelativeRotation || { pitch: 0, yaw: 0, roll: 0 }, relative_location_ue_cm: mesh.RelativeLocation || { x: 0, y: 0, z: 0 } },
    reactor_relative_location_ue_cm: reactor.RelativeLocation || null,
  };
}

// a clip's sequence properties as the sequence package serializes them (absent = class default)
function sequenceProps(asset) {
  const d = sp.loadPkg(sp.gamePathToContent(asset));
  const main = d.exports.find((e) => e.class === 'AnimSequence') || d.exports[0];
  const p = sp.propsOf(main);
  const v = (k, dflt) => (p[k] ? p[k].value : dflt);
  return { bEnableRootMotion: v('bEnableRootMotion', false), m_InterpolationTime: v('m_InterpolationTime', 0), RateScale: v('RateScale', 1), defaults_note: 'absent properties = class defaults' };
}

// (P8 fix 2) the model without the primitives of the slots named (glb JSON chunk only; the binary chunk is kept as it is: its
// unused views stay unread)
function dropPrimitives(glb, slots) {
  if (!slots.length) return glb;
  const len = glb.readUInt32LE(12);
  const json = JSON.parse(glb.subarray(20, 20 + len).toString('utf8'));
  const drop = new Set(json.materials.map((m, i) => (slots.some((s) => (m.name || '').startsWith(s + '@')) ? i : -1)).filter((i) => i >= 0));
  let n = 0;
  for (const mesh of json.meshes) { const before = mesh.primitives.length; mesh.primitives = mesh.primitives.filter((p) => !drop.has(p.material)); n += before - mesh.primitives.length; }
  if (!n) throw new Error(`no primitive of ${slots.join(', ')} in the model`);
  let text = Buffer.from(JSON.stringify(json), 'utf8');
  if (text.length % 4) text = Buffer.concat([text, Buffer.alloc(4 - (text.length % 4), 0x20)]);
  const rest = glb.subarray(20 + len);
  const head = Buffer.alloc(20);
  head.writeUInt32LE(0x46546c67, 0); head.writeUInt32LE(2, 4); head.writeUInt32LE(20 + text.length + rest.length, 8); head.writeUInt32LE(text.length, 12); head.writeUInt32LE(0x4e4f534a, 16);
  console.log(`moogle_pack: ${n} primitive(s) of ${slots.join(', ')} left out of the model`);
  return Buffer.concat([head, text, rest]);
}

// ---------------------------------------------------------------------------------------------- 2. the pack
function build() {
  const model = readJson(P(SRC, 'model.json'));
  const skel = readJson(P(SRC, 'skeleton.json'));
  const facts = gimmickFacts();
  const glb = dropPrimitives(fs.readFileSync(P(SRC, model.glb)), MODELS[NPC].drop_slots || []);
  put('model.glb', glb);
  // the body clips of the Moogle's own anim sets (n_ex014_AnimSet M_001 / M_002, n_ex014_GM_Body_AnimSet Idle_GM); the
  // photo-hologram set's (rt_holo_dlc) are the same sequences or a 2-frame pose: not packed
  const clips = model.clips.filter((c) => !/rt_holo_dlc/.test(c.asset));
  const items = [{ file: 'model.glb', kind: 'mesh', bytes: glb.length, sha256: sha256(glb), source_asset: model.source_mesh, vertices: model.stats.vertices, triangles: model.stats.triangles, primitives: model.stats.primitives }];
  for (const c of clips) {
    const buf = fs.readFileSync(P(SRC, c.file));
    put(c.file, buf);
    items.push({ file: c.file, kind: 'animation', bytes: buf.length, sha256: sha256(buf), source_asset: c.asset, animation_name: c.sequence, logical_names: c.logical_names, duration_s: c.duration_s, num_frames: c.frames, frame_rate: 30, loop: null, root_motion_enabled: false, sequence_properties: sequenceProps(c.asset), notifies: c.notifies });
  }
  const rowsOf = (seq) => model.rows.filter((r) => !/rt_holo_dlc/.test(r.anim_set) && path.basename(r.asset) === seq);
  const logical = [];
  for (const c of clips) for (const r of rowsOf(c.sequence)) logical.push({ name: r.name, anim_set: r.anim_set, asset: r.asset, class: r.class, file: c.file, samples: [{ animation_name: c.sequence }], stage1: true, use: r.name === facts.default_idling ? 'the gimmick blueprint\'s m_DefaultIdlingMotName: the idle the shop actor plays' : '' });
  // the Animator's saved pose starts as the row named IdleN (kh-anim pose.rs): the default idling row's clip
  const idle = logical.find((l) => l.name === facts.default_idling);
  if (idle) logical.unshift({ ...idle, name: 'IdleN', use: `alias of ${facts.default_idling} (kh-anim starts the saved pose on the row named IdleN)` });

  // Bonamik: the Moogle's asset (pom-pom fusen1/2, wings, hair, ears) by the friends' export rule
  const bones = skel.skeleton.bones.map((b) => ({ name: b.name, parent: b.parent }));
  const soraBonamik = readJson(P(PACK, 'data', 'bonamik.json'));
  const bnm = bonamikExport(BONAMIK_CONTENT, MESH_CONTENT, bones, soraBonamik);
  // (P8 fix 2) groups left out: not simulated (enable false: "state 0, animation weight 1"), their bones not driven
  for (const g of bnm.groups.filter((g) => (MODELS[NPC].drop_groups || []).includes(g.name))) {
    g.enable = false;
    g.dropped_by = 'tools/moogle_pack.js (P8 fix 2: the user removed the balloon)';
    for (const b of bnm.bones.filter((b) => b.group === g.name)) Object.assign(b, { bonamik: 'none', body: -1, group: null });
  }
  bnm._schema.about = `KH3's Moogle (${NPC}) Bonamik data. Source: Content/${BONAMIK_CONTENT}. Same schema as build/sora_pack/data/bonamik.json (asset part); written by tools/moogle_pack.js through tools/sora_weapon_pack.js bonamikExport.`;
  putJson('data/bonamik.json', bnm);
  const driven = bnm.bones.filter((b) => b.bonamik !== 'none');

  const manifest = {
    format: 'kh3-party-pack/1', generator: `tools/moogle_pack.js ${VERSION}`, character: 'Moogle', member: null, npc: NPC,
    note: 'KH3\'s Synthesis Moogle (P8 MOOGLE NPC): a pack in the friends\' layout (docs/party-contracts.md A.1) for the renderer\'s party path and kh_anim::Pack::load. No AI, no weapon, no face slots, no leg IK, no KineDriver (its *_spo bones follow the clips).',
    identification: { ...facts, skeletal_mesh: model.source_mesh, anim_sets: model.anim_sets, mesh_fingerprint: model.fingerprint, bonamik_asset: `/Game/${BONAMIK_CONTENT}` },
    conventions: { skeleton: 'skeleton.bones: translation_ue cm, rotation_ue xyzw, scale_ue (UE space); a parent before its children', component: 'the mesh component = the actor origin turned by yaw - 90 (the blueprint\'s TresGimmickMesh0 RelativeRotation yaw -90), as the friends\' (kh-partyhost poser component_to_world)' },
    skeleton: skel.skeleton, sockets: [],
    data_files: [{ file: 'data/bonamik.json', what: `Bonamik ${BONAMIK_CONTENT}: ${bnm.bodies.length} bodies, ${driven.length} driven bones (${[...new Set(driven.filter((b) => b.group).map((b) => b.group))].join(', ')})` }],
    logical_animations: logical, items,
  };
  putJson('manifest.json', manifest);
  console.log(`moogle_pack: ${bones.length} bones, ${clips.length} clips (${clips.map((c) => c.sequence).join(', ')}), rows ${logical.map((l) => l.name).join(', ')}; Bonamik ${bnm.bodies.length} bodies, ${bnm.links.length} links, groups ${bnm.groups.map((g) => `${g.name}${g.enable ? '' : ' (off)'}`).join(', ')}; driven bones: ${driven.map((b) => `${b.name}:${b.bonamik}`).join(' ')}`);

  // materials: the shop library's n_ex014 slots, their mesh named as the folder (the party loader's rule: slot.mesh == npc)
  const lib = readJson(P(SHOP, 'materials.json'));
  const dropped = (k) => (MODELS[NPC].drop_slots || []).some((s) => k.startsWith(s + '@'));
  const mine = Object.fromEntries(Object.entries(lib.materials).filter(([k, m]) => m.mesh === `npc:${NPC}` && !dropped(k)).map(([k, m]) => [k, { ...m, mesh: NPC }]));
  const owners = Object.fromEntries(Object.entries(lib.owners).filter(([k]) => Object.values(mine).some((m) => m.owner === k)));
  const meshFacts = lib.mesh_facts && lib.mesh_facts[`npc:${NPC}`] ? { [NPC]: lib.mesh_facts[`npc:${NPC}`] } : {};
  const uv2 = Object.fromEntries(Object.entries(lib.uv2 || {}).filter(([k]) => k.startsWith(`npc:${NPC}:`)).map(([k, v]) => [k.replace(`npc:${NPC}:`, `${NPC}:`), v]));
  const out = { ...lib, generator: `${lib.generator} -> tools/moogle_pack.js ${VERSION} (the ${NPC} slots of PACK/npc/shop/materials, mesh npc:${NPC} -> ${NPC})`, owners, materials: mine, uv2, mesh_facts: meshFacts, wear_forms: lib.wear_forms || {} };
  delete out.keyblades;
  putJson('materials/materials.json', out);
  put('materials/materials.wgsl', fs.readFileSync(P(SHOP, 'materials.wgsl')));
  const files = new Set();
  for (const m of Object.values(mine)) for (const t of m.textures || []) if (t && t.file) files.add(t.file);
  for (const u of Object.values(uv2)) if (u && u.file) files.add(u.file);
  for (const f of files) put(P('materials', f), fs.readFileSync(P(SHOP, f)));
  console.log(`moogle_pack: materials ${Object.keys(mine).join(', ')} (owners ${Object.values(owners).map((o) => o.id).join(', ')}), ${files.size} texture files`);
  return { manifest, bnm };
}

// ---------------------------------------------------------------------------------------------- 3. the sounds
// KH3's Moogle shop voice rows (UI/Data/ShopDataAsset, the ShopInfos entry whose Staff is n_ex014_UIActor): TalkWelcome
// se00000_moogle1..3, TalkNewArrival se00000_moogle4..5. The main audio pack carries none of them (its se00000 bank holds the
// UI sounds only): an extra root by the audio tool's own rules (sora_audio_pack.js --party-extra with KH_AUDIO_EXTRA_SOUNDS,
// into a scratch folder), then only the Moogle's sounds and their materials are kept (kh-audio Pack::merge_party_root adds them
// to the main pack's se00000 bank).
function shopVoices() {
  const d = sp.loadPkg('UI/Data/ShopDataAsset');
  const simp = (v) => { if (Array.isArray(v)) { if (v.length && v[0] && v[0].name !== undefined && v[0].type) { const o = {}; for (const p of v) o[p.name] = simp(p.value); return o; } return v.map(simp); } if (v && typeof v === 'object') { if (v.items) return v.items.map(simp); const o = {}; for (const k in v) o[k] = simp(v[k]); return o; } return v; };
  const infos = d.exports[0].data.props.filter((p) => p.name === 'ShopInfos').map((p) => simp(p.value));
  const moogle = infos.find((i) => /n_ex014_UIActor/.test(i.Staff || ''));
  if (!moogle) throw new Error('ShopDataAsset: no ShopInfos entry with the n_ex014 staff');
  const row = (r) => ({ asset: r.VoiceSound.replace(/\.[A-Za-z0-9_]+$/, ''), subtitle: r.SubtitleTextID, subtitle_seconds: r.FixedSubtitleDisplayTime });
  return { name: moogle.NameTextID, welcome: (moogle.TalkWelcome || []).map(row), new_arrival: (moogle.TalkNewArrival || []).map(row) };
}

function audio(voices) {
  const all = [...voices.welcome, ...voices.new_arrival];
  const scratch = P(sp.ROOT, 'build', 'sora_tmp', 'weapons', 'P8', 'audio_scratch');
  fs.rmSync(scratch, { recursive: true, force: true });
  fs.mkdirSync(scratch, { recursive: true });
  const list = P(scratch, 'sounds.json');
  fs.writeFileSync(list, JSON.stringify(all.map((v) => ({ asset: v.asset, trigger: 'P8 MOOGLE NPC: the Moogle\'s talk (KH3 ShopDataAsset ' + (voices.welcome.includes(v) ? 'TalkWelcome' : 'TalkNewArrival') + ')', spatial: '3D', note: '(P8) the Synthesis Moogle\'s voice' })), null, 1));
  const run = cp.spawnSync(process.execPath, [P(__dirname, 'sora_audio_pack.js'), '--party-extra', '--out', P(scratch, 'root')], { env: { ...process.env, KH_AUDIO_EXTRA_SOUNDS: list }, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  if (run.status !== 0) throw new Error('sora_audio_pack.js --party-extra failed: ' + (run.stderr || run.stdout).slice(-2000));
  const m = readJson(P(scratch, 'root', 'manifest.json'));
  const keep = new Set(all.map((v) => v.asset));
  const assets = Object.fromEntries(Object.entries(m.sound_assets).filter(([k]) => keep.has(k)));
  if (Object.keys(assets).length !== keep.size) throw new Error(`the extra root packed ${Object.keys(assets).length} of the ${keep.size} Moogle sounds`);
  const banks = {};
  for (const a of Object.values(assets)) {
    const b = m.banks[a.bank];
    const nb = banks[a.bank] || (banks[a.bank] = { ...b, sounds: {}, materials: {} });
    const s = b.sounds[a.sound_index];
    nb.sounds[a.sound_index] = s;
    const mats = new Set();
    const walk = (x) => { if (!x || typeof x !== 'object') return; if (Array.isArray(x)) { x.forEach(walk); return; } if (x.type === 'MaterialTrack' && x.material !== null && x.material !== undefined) mats.add(x.material); for (const k of Object.keys(x)) walk(x[k]); };
    walk(s);
    for (const mi of mats) nb.materials[mi] = b.materials[mi];
  }
  for (const b of Object.values(banks)) for (const mt of Object.values(b.materials)) put(P('audio', mt.file), fs.readFileSync(P(scratch, 'root', mt.file)));
  const triggers = m.triggers.filter((t) => t.sounds.some((s) => keep.has(s.asset)));
  const usedCat = new Set(); for (const b of Object.values(banks)) for (const s of Object.values(b.sounds)) usedCat.add(s.category.number);
  const manifest = { ...m, generator: `${m.generator} -> tools/moogle_pack.js ${VERSION} (the Moogle's voices only)`, counts: { triggers: triggers.length, sound_assets: Object.keys(assets).length, banks: Object.keys(banks).length, sounds: Object.values(banks).reduce((n, b) => n + Object.keys(b.sounds).length, 0), materials: Object.values(banks).reduce((n, b) => n + Object.keys(b.materials).length, 0), unresolved: 0 }, banks, sound_assets: assets, triggers, unresolved: [], requests_seen: 0, validation: { note: 'the materials are the extra root\'s files, read back by sora_audio_pack.js' } };
  putJson('audio/manifest.json', manifest);
  console.log(`moogle_pack: audio root ${Object.keys(assets).length} sounds (${[...keep].map((a) => path.basename(a)).join(', ')}), banks ${Object.keys(banks).join(', ')} (${manifest.counts.materials} materials)`);
  return manifest;
}

function main() {
  if (!fs.existsSync(P(SRC, 'model.json'))) throw new Error(`${rel(SRC)}: no Moogle pack (tools/sora_keyblades_pack.js)`);
  build();
  const voices = shopVoices();
  if (!NO_AUDIO) audio(voices);
  // the facts kh-er reads (the sheet's moogle.sounds rows quote them)
  putJson('moogle.json', { format: 'kh3-moogle/1', generator: `tools/moogle_pack.js ${VERSION}`, shop_name_text: voices.name, voices: { welcome: voices.welcome, new_arrival: voices.new_arrival }, note: 'KH3 UI/Data/ShopDataAsset ShopInfos[n_ex014_UIActor]: the voice rows the shop plays when it opens (TalkWelcome) and when it has something new (TalkNewArrival)' });
  const index = { format: 'kh3-moogle-files/1', generator: `tools/moogle_pack.js ${VERSION}`, root: rel(OUT), files: written };
  fs.writeFileSync(P(OUT, 'files.json'), JSON.stringify(index, null, 1) + NL);
  console.log(`moogle_pack ${VERSION}: ${written.length} files -> ${rel(OUT)} (+ files.json)`);
}
try { main(); } catch (e) { console.error(e); process.exit(1); }
