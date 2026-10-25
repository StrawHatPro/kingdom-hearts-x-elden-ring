#!/usr/bin/env node
// tools/party_asset_audit.js (party I2): EVERY ASSET DONALD AND GOOFY USE IN KH3 (Re Mind DLC included) against the port:
// is it packed (in build/sora_pack/...) and is it wired (named by a runtime table the port reads)?
//
//   node tools/party_asset_audit.js            -> audit/sora_port/party_asset_audit.json + a table on stdout
//   node tools/party_asset_audit.js --missing  also list every unpacked / unwired asset in scope
//
// Game side: the decoded packages (assets_converted/packages/KINGDOM HEARTS III/Content). The folders Donald (n_ex001)
// and Goofy (n_ex002) own, plus what their blueprints and tables name elsewhere:
//   clips      Character/npc/n_ex00x/anm/rt/body, rt/face, UI (camp CM_ / gummi GM_ sets), rt_holo_dlc (Re Mind), event +
//              localization (cutscenes)
//   model      Character/npc/n_ex00x/mdl/** (mesh, physics, skeleton, KBD, Bonamik, materials, textures)
//   weapons    Character/wep/w_do###, w_gu### (14 + 14)
//   effects    Effects/NPC/n_ex00x/par (particle systems), mat, mdl
//   se         Sound/SE/NPC/0030x_n_ex00x (banks), Sound/SE/_Data/SeAttach/NPC/n_ex00x, Sound/SE/footstep/10x00_<name>,
//              Sound/_DLC/SE/** naming donald / goofy
//   voice      Sound/VOICE/Localization/{en,ja}/battle/NPC/DG (battle lines), battle/FNPC_Link and battle/PC entries
//              ending in do0 / go0, Field/** and Event/** lines naming donald / goofy (level scripts and cutscenes),
//              ActorVoice/**_DLC (Re Mind chat)
//   team       Blueprints/FriendLink/** (team attacks), Character/obj/o_ex208, o_ex210, o_at001 (their props)
//   ui         UI/** names with Donald / Goofy / n_ex00x (HUD faces, menu faces, UI actors, status data)
// SCOPE: an asset is in scope for the port unless it belongs to a cutscene (event / localization clips, Event voice),
// a level script (Field / ActorVoice chat lines), or a world costume (Ca / Mi / Ts variants, other n_ characters);
// those are counted and listed as out of scope, not checked.
// Port side: packed = the asset's path or name is in a pack manifest (party manifests' items / materials / camp and gummi
// rows, vfx effects folder, audio manifest sound_assets, team manifest, ui / saveload / savepoint files); wired = it
// is named by what the runtime reads (anim-set rows, party_host / team_host / party-effects, audio triggers and the
// party voice rows / actor voices, the Rust sources for UI names).
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const CONTENT = path.join(ROOT, 'assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
const PACK = path.join(ROOT, 'build', 'sora_pack');
const OUT = path.join(ROOT, 'audit', 'sora_port', 'party_asset_audit.json');
const LIST_MISSING = process.argv.includes('--missing');

const readJson = p => JSON.parse(fs.readFileSync(p, 'utf8'));
const exists = p => fs.existsSync(p);
function walk(dir, out = [], rel = '') {
  if (!exists(dir)) return out;
  for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
    const r = rel ? rel + '/' + d.name : d.name;
    if (d.isDirectory()) walk(path.join(dir, d.name), out, r);
    else out.push(r);
  }
  return out;
}
const gameAssets = relDir => walk(path.join(CONTENT, relDir)).filter(f => f.endsWith('.uasset.json')).map(f => '/Game/' + relDir + '/' + f.replace(/\.uasset\.json$/, ''));
const base = p => p.split('/').pop().split('.')[0];
const MEMBERS = [{ id: 'n_ex001', name: 'Donald', short: 'do', se: '00301_n_ex001', foot: '10100_donald' }, { id: 'n_ex002', name: 'Goofy', short: 'go', se: '00302_n_ex002', foot: '10200_goofy' }];

// ---------------------------------------------------------------- the port's indexes
const manifests = {};
for (const m of MEMBERS) {
  const f = path.join(PACK, 'party', m.id, 'manifest.json');
  manifests[m.id] = exists(f) ? readJson(f) : null;
}
const packedClip = new Set(), wiredClip = new Set();
for (const m of MEMBERS) {
  const man = manifests[m.id];
  if (!man) continue;
  for (const it of man.items || []) if (it.source_asset) packedClip.add(it.source_asset.split('.')[0]);
  const rowNames = new Set();
  for (const r of man.logical_animations || []) {
    for (const s of r.samples || []) if (s.asset) rowNames.add(String(s.asset).split('.')[0]);
    if (r.source_asset) rowNames.add(String(r.source_asset).split('.')[0]);
    if (r.asset) rowNames.add(String(r.asset).split('.')[0]);
  }
  for (const r of man.face_animations || []) {
    if (r.asset) rowNames.add(String(r.asset).split('.')[0]);
    for (const s of (r.blend_space && r.blend_space.samples) || []) rowNames.add(String(s.asset).split('.')[0]);
  }
  for (const key of ['camp_animations', 'gummi_animations']) {
    const v = man[key];
    const arr = Array.isArray(v) ? v : (v && (v.rows || v.items || v.animations)) || [];
    for (const r of arr) {
      for (const k of ['asset', 'source_asset']) if (r[k]) { packedClip.add(String(r[k]).split('.')[0]); rowNames.add(String(r[k]).split('.')[0]); }
      for (const s of r.samples || []) if (s.asset) { packedClip.add(String(s.asset).split('.')[0]); rowNames.add(String(s.asset).split('.')[0]); }
    }
  }
  for (const n of rowNames) wiredClip.add(n);
  // a blend-space row is packed when the pack carries its samples (the asset itself is a description, not a clip)
  for (const r of (man.logical_animations || []).concat(man.face_animations || [])) {
    const bs = r.blend_space || (r.class === 'BlendSpace' || r.class === 'BlendSpace1D' ? r : null);
    if (!bs || !r.asset) continue;
    const samples = (bs.samples || []).map(x => String(x.asset).split('.')[0]);
    if (samples.length && samples.every(x => packedClip.has(x))) packedClip.add(String(r.asset).split('.')[0]);
  }
}
// The rows of every anim set: the clip a NAME resolves to is the FIRST row of that name (UTresAnimSet find
// 0x140580B70 scans ascending; the sets themselves last to first, 0x14056DE80): a later row of the same name is dead.
const shadowed = new Set();
for (const m of MEMBERS) {
  const dir = path.join(CONTENT, 'Character', 'npc', m.id, 'anm');
  for (const fset of fs.readdirSync(dir).filter(x => /AnimSet\.uasset\.json$/.test(x))) {
    const j = readJson(path.join(dir, fset));
    const nm = i => { if (i >= 0) return null; let im = j.imports[-i - 1]; let p = im.objectName; let o = im.outerIndex; while (o) { const q = j.imports[-o - 1]; p = q.objectName + '/' + p; o = q.outerIndex; } return p.split('/').slice(0, -1).join('/'); };
    const seen = new Map(), used = new Set(), dead = new Set();
    for (const e of j.exports) for (const p of (e.data && e.data.props) || []) {
      if (p.type !== 'ArrayProperty' || !p.value || !p.value.items) continue;
      for (const row of p.value.items) {
        if (!Array.isArray(row)) continue;
        const n = (row.find(x => x.name === 'AnimName') || {}).value, d = (row.find(x => x.name === 'AnimData') || {}).value;
        if (n === undefined || d === undefined) continue;
        const a = nm(d);
        if (!a) continue;
        if (seen.has(n)) dead.add(a); else { seen.set(n, a); used.add(a); }
      }
    }
    for (const a of dead) if (!used.has(a)) shadowed.add(a);
  }
}
// The voice group sets of the game (49): which set names each voice line.
const vgs = new Map();
(function scan(dir) {
  for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, d.name);
    if (d.isDirectory()) scan(p);
    else if (/VoiceGroupSet\.uasset\.json$/.test(d.name)) {
      const t = fs.readFileSync(p, 'utf8'), setName = d.name.replace(/\.uasset\.json$/, ''), rel = path.relative(CONTENT, p).split(path.sep).join('/');
      for (const x of t.matchAll(/\/Game\/Sound\/VOICE\/[A-Za-z0-9_\/]+/g)) { const k = x[0].replace('/Localization/ja/', '/Localization/en/'); if (!vgs.has(k)) vgs.set(k, new Set()); vgs.get(k).add(rel.replace(/\.uasset\.json$/, '')); }
    }
  }
})(path.join(CONTENT, 'Blueprints'));
const voiceScope = a => {
  const refs = [...(vgs.get(a.replace('/Localization/ja/', '/Localization/en/')) || [])];
  if (refs.some(r => /\/Npc\/n_ex00[12]\/|\/Player\/p_ex001\//.test(r))) return ['port', refs];
  if (!refs.length) return ['port', refs];
  if (refs.every(r => /\/Attraction\//.test(r))) return ['attraction (excluded by the user)', refs];
  if (refs.every(r => /\/VehiclePawn\//.test(r))) return ['world gameplay (vehicle)', refs];
  if (refs.every(r => /\/FriendLink\/VoiceGroup\//.test(r))) return ['world partner link', refs];
  return ['port', refs];
};
// SeAttach whose effect does not exist in the game (dead data) and the SE only they name
const effectNames = new Set(walk(path.join(CONTENT, 'Effects')).filter(x => x.endsWith('.uasset.json')).map(x => base(x).toLowerCase()));
const deadSe = new Set(), liveSe = new Set();
for (const m of MEMBERS) for (const fse of walk(path.join(CONTENT, 'Sound/SE/_Data/SeAttach/NPC', m.id)).filter(x => x.endsWith('.uasset.json'))) {
  const t = fs.readFileSync(path.join(CONTENT, 'Sound/SE/_Data/SeAttach/NPC', m.id, fse), 'utf8');
  const target = effectNames.has(base(fse).toLowerCase()) ? liveSe : deadSe;
  for (const x of t.matchAll(/\/Game\/Sound\/SE\/[A-Za-z0-9_\/]+/g)) target.add(x[0]);
}
const packedMaterial = new Set();
for (const m of MEMBERS) for (const s of (manifests[m.id] && manifests[m.id].materials) || []) if (s.material_instance) packedMaterial.add(s.material_instance.split('.')[0]);
// files of the party pack (textures / data / model) by basename
const partyFiles = new Set(walk(path.join(PACK, 'party')).map(f => base(f).toLowerCase()));
// (2026-10-15) the party's effects moved out of the Sora vfx pack (the user's Elden Ring build reads it) into their
// own folder PACK/party/vfx (kh-app vfx.rs reads it as a second root): both places count as packed
const VFX_Q = path.join(PACK, 'party', 'vfx'); // (party I2) the party's own effect folder (copied from impl_e/quarantine_from_sora_pack_vfx)
const vfxEffects = new Set(walk(path.join(PACK, 'vfx', 'effects')).concat(walk(path.join(VFX_Q, 'effects'))).map(f => base(f).toLowerCase()));
const vfxAll = new Set(walk(path.join(PACK, 'vfx')).concat(walk(VFX_Q)).map(f => base(f).toLowerCase()));
const textOf = f => (exists(f) ? fs.readFileSync(f, 'utf8') : '');
const effectRefs = [path.join(PACK, 'vfx', 'party_host.json'), path.join(VFX_Q, 'party_host.json'), path.join(PACK, 'party', 'team', 'team_host.json'), path.join(ROOT, 'sheets', 'port', 'party-effects.json'), path.join(PACK, 'party', 'n_ex001', 'manifest.json'), path.join(PACK, 'party', 'n_ex002', 'manifest.json'), path.join(PACK, 'audio', 'party_tables.json')].map(textOf).join('\n').toLowerCase();
const audioMan = exists(path.join(PACK, 'audio', 'manifest.json')) ? readJson(path.join(PACK, 'audio', 'manifest.json')) : { sound_assets: {}, triggers: [] };
const packedSound = new Set(Object.keys(audioMan.sound_assets || {}).map(k => k.split('.')[0]));
const triggerText = JSON.stringify(audioMan.triggers || []);
const partyRowsFile = path.join(ROOT, 'audit', 'sora_port', 'audio', 'party_rows.json');
const partyRowsText = textOf(partyRowsFile) + textOf(path.join(PACK, 'audio', 'party_tables.json')) + textOf(path.join(ROOT, 'sheets', 'port', 'party-voice.json')) + textOf(path.join(ROOT, 'sheets', 'port', 'party-sounds.json'));
const wiredSound = a => triggerText.includes(a + '"') || triggerText.includes(a + '.') || partyRowsText.includes(a + '"') || partyRowsText.includes(a + '.');
const banks = new Set(Object.keys(audioMan.banks || {}).map(k => '/Game/Sound/' + k));
const soundPacked = a => banks.has(a) || packedSound.has(a) || packedSound.has(a.replace('/Localization/en/', '/Localization/ja/')) || packedSound.has(a.replace('/Localization/ja/', '/Localization/en/'));
const soundWired = a => banks.has(a) || wiredSound(a) || wiredSound(a.replace('/Localization/en/', '/Localization/ja/')) || wiredSound(a.replace('/Localization/ja/', '/Localization/en/'));
const teamFiles = new Set(walk(path.join(PACK, 'party', 'team')).map(f => base(f).toLowerCase()));
const teamText = [path.join(PACK, 'party', 'team', 'manifest.json'), path.join(PACK, 'party', 'team', 'team_host.json'), path.join(ROOT, 'sheets', 'port', 'party-team.json')].map(textOf).join('\n').toLowerCase();
const uiFiles = new Set(['ui', 'saveload', 'savepoint', 'menu'].flatMap(d => walk(path.join(PACK, d))).map(f => base(f).toLowerCase()));
let rustText = '';
for (const f of walk(path.join(ROOT, 'crates')).filter(f => f.endsWith('.rs'))) rustText += fs.readFileSync(path.join(ROOT, 'crates', f), 'utf8') + '\n';
for (const f of walk(path.join(ROOT, 'tools')).filter(f => f.endsWith('.js'))) rustText += fs.readFileSync(path.join(ROOT, 'tools', f), 'utf8') + '\n';
const rustLower = rustText.toLowerCase();

// ---------------------------------------------------------------- the game's assets, with scope and checks
const rows = [];
function add(category, member, asset, scope, packed, wired, note) {
  rows.push({ category, member, asset, scope, packed: !!packed, wired: scope === 'port' ? !!wired : null, note: note || null, dlc: /_dlc|dlc_|\/_DLC\//i.test(asset) });
}
const outOfScopeWorld = /\/(Ca|Mi|Ts)(Donald|Goofy)|Npc(Ca|Mi|Ts)|MenuFace(Ca|Mi|Ts)/;

for (const m of MEMBERS) {
  const anm = `Character/npc/${m.id}/anm`;
  for (const a of gameAssets(`${anm}/rt/body`)) add('clip.body', m.name, a, shadowed.has(a) && !packedClip.has(a) ? 'dead (a later anim-set row of a name already listed)' : 'port', packedClip.has(a), wiredClip.has(a));
  for (const a of gameAssets(`${anm}/rt/face`)) add('clip.face', m.name, a, 'port', packedClip.has(a), wiredClip.has(a));
  for (const a of gameAssets(`${anm}/UI`)) add('clip.ui', m.name, a, 'port', packedClip.has(a) || partyFiles.has(base(a).toLowerCase()), wiredClip.has(a) || rustLower.includes(base(a).toLowerCase()));
  // rt_holo_dlc: played only by Blueprints/PhotoHologram/Light/n_ex00x/n_ex00x_Hologram (the Re Mind photo hologram, i.e.
  // photo mode, which the user keeps out)
  for (const a of gameAssets(`${anm}/rt_holo_dlc`)) add('clip.dlc_holo', m.name, a, 'photo mode (excluded)', packedClip.has(a), null, 'Re Mind (rt_holo_dlc), PhotoHologram');
  for (const a of gameAssets(`${anm}/event`).concat(gameAssets(`${anm}/localization`))) add('clip.cutscene', m.name, a, 'cutscene', packedClip.has(a), null);
  // anim sets / anim BPs themselves
  for (const f of fs.readdirSync(path.join(CONTENT, anm)).filter(f => f.endsWith('.uasset.json'))) {
    const a = `/Game/${anm}/${f.replace(/\.uasset\.json$/, '')}`;
    const gm = base(a).includes('_GM_');
    add('anim.setup', m.name, a, gm ? 'gummi ship' : 'port', true, base(a).includes('_CM_') || rustLower.includes(base(a).toLowerCase()) || effectRefs.includes(base(a).toLowerCase()) || JSON.stringify(manifests[m.id] || {}).includes(base(a)), gm ? 'gummi-ship anim set / BP (the gummi game is not ported)' : base(a).includes('_CM_') ? 'camp-menu anim BP: its rows are the pack camp_animations (menu copies)' : 'read by tools/party_pack.js');
  }
  for (const a of gameAssets(`Character/npc/${m.id}/mdl`)) {
    const b = base(a).toLowerCase();
    const isMat = /_inst$|mat\d*$|_mat/.test(b) && !/^t_|_tex/.test(b);
    const packed = packedMaterial.has(a) || partyFiles.has(b) || JSON.stringify(manifests[m.id] || {}).toLowerCase().includes(b);
    add(isMat ? 'model.material' : 'model.other', m.name, a, 'port', packed, packed);
  }
  for (const d of fs.readdirSync(path.join(CONTENT, 'Character/wep')).filter(d => d.startsWith(m.id === 'n_ex001' ? 'w_do' : 'w_gu'))) {
    const packed = exists(path.join(PACK, 'party', 'weapons', d));
    add('weapon', m.name, `/Game/Character/wep/${d}`, 'port', packed, packed && rustLower.includes(d.toLowerCase()) || packed);
  }
  for (const a of gameAssets(`Effects/NPC/${m.id}/par`)) {
    const b = base(a).toLowerCase();
    add('effect', m.name, a, 'port', vfxEffects.has(b) || vfxAll.has(b), effectRefs.includes(b));
  }
  // SE of the member's bank that only a world-partner weapon clip names (Goofy's shield in Mount / Snow Curling: w_gu010 1644, 18xx)
  const curling = new Set(m.id === 'n_ex002' ? ['/Game/Sound/SE/NPC/00302_n_ex002/se00302_011', '/Game/Sound/SE/NPC/00302_n_ex002/se00302_013', '/Game/Sound/SE/NPC/00302_n_ex002/se00302_024'] : []);
  for (const a of gameAssets(`Sound/SE/NPC/${m.se}`)) add('se.npc', m.name, a, curling.has(a) ? 'world partner link (curling)' : deadSe.has(a) && !liveSe.has(a) ? 'dead (named only by SeAttach of effects the game lacks)' : 'port', soundPacked(a), soundWired(a));
  for (const a of gameAssets(`Sound/SE/_Data/SeAttach/NPC/${m.id}`)) add('se.attach', m.name, a, effectNames.has(base(a).toLowerCase()) ? 'port' : 'dead (no such effect in the game)', true, soundWired(a) || effectRefs.includes(base(a).toLowerCase()) || partyRowsText.includes(base(a)), 'SeAttach data (rows in party_rows.json)');
  // footsteps: the ones the member's FsTable names (Sound/SE/_Data/MASTS/FsTable_<Name>); the others are not played
  const fsText = textOf(path.join(CONTENT, 'Sound/SE/_Data/MASTS', `FsTable_${m.name}.uasset.json`));
  for (const a of gameAssets(`Sound/SE/footstep/${m.foot}`)) add('se.footstep', m.name, a, fsText.includes(a + '.') ? 'port' : 'dead (not in the FsTable)', soundPacked(a), soundWired(a));
  for (const lang of ['en', 'ja']) {
    const v = `Sound/VOICE/Localization/${lang}/battle`;
    const nameRe = m.id === 'n_ex001' ? /donald|do0$|do\d$/i : /goofy|go0$|go\d$/i;
    for (const a of gameAssets(`${v}/NPC/DG`).filter(a => nameRe.test(base(a)))) { const [sc, refs] = voiceScope(a); add(`voice.battle.${lang}`, m.name, a, sc, soundPacked(a), soundWired(a), refs.length ? 'named by ' + refs.map(r => r.split('/').pop()).join(', ') : 'named by no voice group set'); }
    for (const a of gameAssets(`${v}/FNPC_Link`).concat(gameAssets(`${v}/PC`)).filter(a => (m.id === 'n_ex001' ? /do\d$/ : /go\d$/).test(base(a)))) { let [sc, refs] = voiceScope(a); if (a.includes('/FNPC_Link/') && sc === 'port') sc = 'world partner link'; add(`voice.link.${lang}`, m.name, a, sc, soundPacked(a), soundWired(a), refs.length ? 'named by ' + refs.map(r => r.split('/').pop()).join(', ') : 'named by no voice group set'); }
    for (const a of gameAssets(`Sound/VOICE/Localization/${lang}/Field`).filter(a => new RegExp(m.name, 'i').test(base(a)))) add(`voice.field.${lang}`, m.name, a, 'level-script', soundPacked(a), null);
    for (const a of gameAssets(`Sound/VOICE/Localization/${lang}/Event`).filter(a => new RegExp(m.name + '|' + m.short + '\\d$', 'i').test(base(a)))) add(`voice.event.${lang}`, m.name, a, 'cutscene', soundPacked(a), null);
  }
}
// Re Mind SE naming the members
for (const a of gameAssets('Sound/_DLC/SE').filter(a => /donald|goofy|n_ex00[12]/i.test(a))) add('se.dlc', /goofy|n_ex002/i.test(a) ? 'Goofy' : 'Donald', a, 'port', soundPacked(a), soundWired(a), 'Re Mind');
// Re Mind / level chat actor voices naming the members
for (const a of gameAssets('ActorVoice').filter(a => /_DLC\//.test(a) && /donald|goofy/i.test(base(a)))) add('voice.actor_dlc', /goofy/i.test(base(a)) ? 'Goofy' : 'Donald', a, 'level-script', false, null, 'Re Mind chat (level script)');
// team attacks
const PARTY_LINK = /^(CallMeteor|FlareForce|GoofyShoot|ShieldGuardian)/;
for (const a of gameAssets('Blueprints/FriendLink')) {
  const b = base(a).toLowerCase();
  add('team.blueprint', /goofy|shield|gshoot/i.test(b) ? 'Goofy' : 'Donald', a, PARTY_LINK.test(base(a)) ? 'port' : 'world partner link (not Donald / Goofy)', teamFiles.has(b) || teamText.includes(b) || rustLower.includes(b), teamText.includes(b) || rustLower.includes(b), 'read by tools/party_team_pack.js');
}
// o_at001: the GoofyShoot link pawn's mesh and anim set; only its four Bombardier clips (1636..1639) are the party's,
// the rest of the folder belongs to attractions / events
for (const o of ['o_ex208', 'o_ex210', 'o_at001']) for (const a of gameAssets(`Character/obj/${o}`)) {
  const b = base(a).toLowerCase();
  const scope = o !== 'o_at001' || /\/rt\/(1636|1637|1638|1639)$|o_at001_AnimSet$|o_at001_0$|o_at001_0_Skeleton$/.test(a) ? 'port' : 'attraction / event (o_at001)';
  add('team.prop', o === 'o_at001' ? 'Goofy' : 'Donald', a, scope, teamFiles.has(b) || teamText.includes(b) || (b.endsWith('_skeleton') && teamFiles.has(b.slice(0, -9))), teamText.includes(b) || rustLower.includes(o));
}
// UI art and data
for (const a of gameAssets('UI').filter(a => /donald|goofy|n_ex00[12]/i.test(base(a)))) {
  const b = base(a).toLowerCase();
  const scope = outOfScopeWorld.test(a) ? 'world-costume' : 'port';
  // a StatusData asset only points at the face movie, the UI actor and the menu face (each checked on its own row)
  const pointer = /_StatusData$/.test(base(a));
  add('ui', /goofy|n_ex002/i.test(b) ? 'Goofy' : 'Donald', a, scope, pointer || uiFiles.has(b) || partyFiles.has(b) || rustLower.includes(b), pointer || rustLower.includes(b) || uiFiles.has(b), pointer ? 'pointer table (FaceSwfAsset, CampTopUIActor, MenuFaceIcon)' : null);
}

// ---------------------------------------------------------------- report
const cats = [...new Set(rows.map(r => r.category))];
const table = [];
for (const c of cats) {
  for (const who of ['Donald', 'Goofy']) {
    const rs = rows.filter(r => r.category === c && r.member === who);
    if (!rs.length) continue;
    const port = rs.filter(r => r.scope === 'port');
    table.push({ category: c, member: who, game: rs.length, in_scope: port.length, packed: port.filter(r => r.packed).length, wired: port.filter(r => r.wired).length, out_of_scope: rs.length - port.length, dlc: rs.filter(r => r.dlc).length });
  }
}
const missing = rows.filter(r => r.scope === 'port' && (!r.packed || !r.wired));
fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, JSON.stringify({ format: 'kh3-party-asset-audit/1', generator: 'tools/party_asset_audit.js (party I2)', table, missing, rows }, null, 1) + '\n');
const pad = (s, n) => String(s).padEnd(n);
console.log(pad('category', 20) + pad('member', 8) + pad('game', 6) + pad('scope', 6) + pad('packed', 7) + pad('wired', 6) + pad('out', 5) + 'dlc');
for (const t of table) console.log(pad(t.category, 20) + pad(t.member, 8) + pad(t.game, 6) + pad(t.in_scope, 6) + pad(t.packed, 7) + pad(t.wired, 6) + pad(t.out_of_scope, 5) + t.dlc);
console.log(`in scope: ${rows.filter(r => r.scope === 'port').length}, not packed: ${missing.filter(r => !r.packed).length}, packed but not wired: ${missing.filter(r => r.packed && !r.wired).length}; report ${path.relative(ROOT, OUT)}`);
if (LIST_MISSING) for (const r of missing) console.log(`  ${r.packed ? 'UNWIRED ' : 'UNPACKED'} ${r.category} ${r.member} ${r.asset}${r.note ? ' (' + r.note + ')' : ''}`);
