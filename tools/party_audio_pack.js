#!/usr/bin/env node
// (party stream E) THE PARTY'S SOUND, VOICE AND EFFECT TABLES: Donald (n_ex001) and Goofy (n_ex002) read from the
// game's assets with the same rules the Sora research passes used (build/sora_tmp/audio/triggers/*.js, code/*.js),
// written as research rows that tools/sora_audio_pack.js packs into THE SAME audio pack (one SEAD driver, one set of
// category pools, as in the game), plus the party's host tables for the audio and effect sides.
//
//   node tools/party_audio_pack.js          write the outputs below, print a census
//   node tools/party_audio_pack.js --check  only print the census
//
// Outputs:
//   audit/sora_port/audio/party_rows.json   per member: notify rows (anim notifies of every anim set of the member, with
//                                           the effective properties and the sounds they name), voice group rows
//                                           (n_ex00x_VoiceGroupSet), footstep / cloth rows (FsTable_Donald / _Goofy),
//                                           weapon rows (the swing / hit sets of all 14 weapons), effect-attached SE rows
//                                           (Sound/SE/_Data/SeAttach for every party effect), spell-actor rows
//                                           (MyFireEffects sound, m_HitEffects pawn / map hit SE)
//   build/sora_pack/audio/party_tables.json the party's spell actor classes -> effects by role and their sounds (the
//                                           format of tables.json `projectiles`; read by crates/kh-audio pack.rs)
//   build/sora_pack/party/vfx/party_host.json   the same classes -> effects by role for the effect side (host.json
//                                           `projectiles` format; read by crates/kh-app vfx.rs)
//   build/sora_tmp/party/impl_e/party_effect_list.json   every particle system the party uses (notifies, magic sets,
//                                           spell actors, weapons, team-attack link pawns): the effect list for
//                                           tools/sora_vfx_pack.js (--list)
//   sheets/port/party-sounds.json, party-voice.json, party-effects.json   one row per trigger / unit / effect
// Nothing under assets/ or assets_converted/ is modified. Rows cite assets; a value not read is null.
'use strict';
const fs = require('fs');
const path = require('path');

const VERSION = '1.0.0';
const ROOT = path.resolve(__dirname, '..');
const PK = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content') + '/';
const AN = path.join(ROOT, 'assets_converted/animations/KINGDOM HEARTS III/Content') + '/';
const AU = path.join(ROOT, 'assets_converted/audio/KINGDOM HEARTS III/Content') + '/';
const checkOnly = process.argv.includes('--check');

// ------------------------------------------------------------------ package helpers (as build/sora_tmp/audio/triggers/lib.js)
const cache = new Map();
const gamePathToRel = (p) => { let s = String(p).replace(/^\/Game\//, ''); const i = s.lastIndexOf('.'); if (i > s.lastIndexOf('/')) s = s.slice(0, i); return s; };
function loadPkg(rel) { if (cache.has(rel)) return cache.get(rel); const f = PK + rel + '.uasset.json'; let d = null; if (fs.existsSync(f)) d = JSON.parse(fs.readFileSync(f, 'utf8')); if (cache.size > 60) cache.clear(); cache.set(rel, d); return d; }
function full(d, i) { if (!i) return null; if (i < 0) { let im = d.imports[-i - 1], s = im.objectName; while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; s = im.objectName + '.' + s; } return s; } return 'E' + i + ':' + d.exports[i - 1].objectName; }
function simp(d, p) {
  if (p === null || typeof p !== 'object') return p;
  if (Array.isArray(p)) { if (p.length && p.every((e) => e && typeof e === 'object' && 'name' in e && 'type' in e)) { const o = {}; for (const e of p) o[e.name + (e.arrayIndex ? '[' + e.arrayIndex + ']' : '')] = sv(d, e); return o; } return p.map((x) => simp(d, x)); }
  if ('name' in p && 'type' in p) return sv(d, p);
  if ('items' in p && Object.keys(p).length <= 3) return p.items.map((x) => simp(d, x));
  const o = {}; for (const k in p) o[k] = simp(d, p[k]); return o;
}
function sv(d, e) { if (e.type === 'ObjectProperty') return full(d, e.value); if (e.type === 'ArrayProperty' && e.innerType === 'ObjectProperty' && Array.isArray(e.value)) return e.value.map((x) => full(d, x)); return simp(d, e.value); }
function ssimp(v) { if (v === null || typeof v !== 'object') return v; if (Array.isArray(v)) return v.map(ssimp); if (v.props) { const r = {}; for (const p of v.props) r[p.tag.name] = ssimp(p.value); return r; } if (v.items) return v.items.map(ssimp); return v; }
function exportProps(d, idx) { const e = d.exports[idx - 1]; return { name: e.objectName, class: e.class, props: simp(d, (e.data && e.data.props) || []) }; }
const cdo = (rel) => { const d = loadPkg(rel); if (!d) return null; const e = d.exports.find((x) => /^Default__/.test(x.objectName)); return e ? { d, props: simp(d, (e.data && e.data.props) || []), super: superOf(d) } : null; };
function superOf(d) { // the parent blueprint class (package path) of a BlueprintGeneratedClass package
  const cls = d.exports.find((e) => e.class === 'BlueprintGeneratedClass'); if (!cls || !cls.superIndex || cls.superIndex > 0) return null;
  let im = d.imports[-cls.superIndex - 1]; while (im.outerIndex) im = d.imports[-im.outerIndex - 1];
  return /^\/Game\//.test(im.objectName) ? gamePathToRel(im.objectName + '.x') : null;
}
// a property along the blueprint chain (own CDO first)
function chainProp(rel, name) { for (let r = rel, n = 0; r && n < 8; n++) { const c = cdo(r); if (!c) return null; if (c.props[name] !== undefined && c.props[name] !== null) return { value: c.props[name], from: r }; r = c.super; } return null; }
const strip = (p) => (p ? String(p).replace(/\.[^./]+$/, '') : p);
const objPath = (s) => (s ? strip(String(s).replace(/^</, '').replace(/>$/, '')) : null);
function resolveSound(gamePath) { // as lib.js resolveSound, reduced to what the rows keep
  if (!gamePath) return null;
  const rel = gamePathToRel(gamePath); const out = { asset: '/Game/' + rel, exists: false };
  const sf = AU + rel + '.sead.json'; if (!fs.existsSync(sf)) { out.note = fs.existsSync(PK + rel + '.uasset.json') ? 'package exists but no SEAD sidecar' : 'no such package'; return out; }
  const j = JSON.parse(fs.readFileSync(sf, 'utf8')); out.exists = true; out.class = j.class;
  const d = fs.existsSync(PK + rel + '.uasset.json') ? JSON.parse(fs.readFileSync(PK + rel + '.uasset.json', 'utf8')) : null;
  const props = {}; for (const p of j.props) props[p.tag.name] = p.tag.type === 'ObjectProperty' ? (d ? full(d, p.value.object) : p.value) : ssimp(p.value);
  out.reference_bank = props.ReferenceBank || null; out.sound_index = 'SoundIndex' in props ? props.SoundIndex : 0;
  out.volume = 'Volume' in props ? props.Volume : null; out.duration = props.Duration === undefined ? null : props.Duration;
  out.attenuation = props.AttenuationSettings || null;
  return out;
}
const enVariant = (p) => p.replace('/Localization/ja/', '/Localization/en/');
function brief(asset, extra) {
  if (!asset) return null; const a = strip(objPath(asset)); const r = resolveSound(a + '.x');
  const o = Object.assign({ asset: a, exists: r.exists, sound_index: r.sound_index ?? null, volume: r.volume ?? null, duration: r.duration ?? null }, extra || {});
  if (/\/Localization\/ja\//.test(a)) { const e = resolveSound(enVariant(a) + '.x'); o.en = { asset: enVariant(a), exists: e.exists, sound_index: e.sound_index ?? null, volume: e.volume ?? null }; }
  return o;
}

// ------------------------------------------------------------------ the members
const MEMBERS = [
  { member: 0, owner: 'n_ex001', name: 'Donald', bp: 'Blueprints/Npc/n_ex001', weaponPrefix: 'w_do', fstable: 'FsTable_Donald', seattach: 'Sound/SE/_Data/SeAttach/NPC/n_ex001' },
  { member: 1, owner: 'n_ex002', name: 'Goofy', bp: 'Blueprints/Npc/n_ex002', weaponPrefix: 'w_gu', fstable: 'FsTable_Goofy', seattach: 'Sound/SE/_Data/SeAttach/NPC/n_ex002' },
];
const SETS = ['AnimSet', 'FaceAnimSet', 'CM_Body_AnimSet', 'CM_Face_AnimSet', 'GM_Body_AnimSet', 'GM_Face_AnimSet'];

// ------------------------------------------------------------------ notify defaults (constructors read by the Sora strand, NOTES.md)
const E = 'ESQEXSEAD';
const AUTOSE = { SourceType: E + 'ANPlayAutoSeSourceType::MotionSoundType', MotionSoundType: E + 'AutoSeMotionSoundType::None', SurfaceAssetPathTable: null, SurfaceAssetTableRecordName: 'None', AutoSePartsType: E + 'AutoSePartsType::Invalid', AutoSePartsIndex: 0, bCheckIsGrounded: true, bAttachToParts: true, SoundVolume: 1, SoundPitchMultiplier: 1, bFollowOwnerVisible: true };
const DEF = {
  SQEXSEADAnimNotify_PlaySound: { Sound: null, VolumeMultiplier: 1, PitchMultiplier: 1, FadeInDuration: 0, bFollow: true, AttachPointName: 'None', bFollowOwnerVisible: true },
  SQEXSEADAnimNotifyState_AttachSound: { Sound: null, VolumeMultiplier: 1, PitchMultiplier: 1, StartWaitTime: 0, FadeInDuration: 0, FadeOutDuration: 0, bFollow: true, AttachPointName: 'None', LoopEndSound: null, LoopEndSoundVolumeMultiplier: 1, LoopEndSoundPitchMultiplier: 1, bFollowOwnerVisible: false },
  SQEXSEADAnimNotify_PlayAutoSe: { PlaySettings: AUTOSE },
  SQEXSEADAnimNotifyState_PlayAutoSe: { PlaySettings: AUTOSE, FadeOutDuration: 0.5 },
  SQEXSEADAnimNotifyState_ChangeAutoSeEnable: { bEnableSetting: false, NotifyEndBehavior: E + 'ChangeAutoSeEnableNotifyEndBehavior::RevertState', bEnableContinuousDisabling: false },
  TresAnimNotify_PlayVoice: { m_VoiceAsset: null, m_RandomVoiceAssets: [], m_VoiceGroupName: 'None', m_VolumeMultiplier: 1, m_PitchMultiplier: 1, m_SocketName: 'atama', m_bFollow: true },
  TresAnimNotifyState_PlayVoice: { m_VoiceAsset: null, m_RandomVoiceAssets: [], m_VoiceGroupName: 'None', m_StartWaitTime: 0, m_VolumeMultiplier: 1, m_PitchMultiplier: 1, FadeOutTime: 0, m_SocketName: 'atama', m_bFollow: true },
  TresAnimNotify_PlayActorVoice: { m_ActorVoice: null },
  TresAnimNotify_PlayWeaponSwingSE: { m_SoundAliasID: 'ETresSoundAliasLabel_WeaponSwing::NOTHING', m_SoundAliasUnit: 'ETresSoundAliasUnit::NOTHING', m_VolumeMultiplier: 1, m_PitchMultiplier: 1, m_SocketName: 'None', m_bFollow: false },
  TresAnimNotify_PlayWeaponHitSE: { m_SoundAliasID: 'ETresSoundAliasLabel_WeaponHit::NOTHING', m_SoundAliasUnit: 'ETresSoundAliasUnit::NOTHING', m_VolumeMultiplier: 1, m_PitchMultiplier: 1, m_SocketName: 'None', m_bFollow: false },
  TresAnimNotifyState_PlayWeaponSwingSE: { m_SoundAliasID: 'ETresSoundAliasLabel_WeaponSwing::NOTHING', m_SoundAliasUnit: 'ETresSoundAliasUnit::NOTHING', m_StartWaitTime: 0, m_VolumeMultiplier: 1, m_PitchMultiplier: 1, m_SocketName: 'None', m_bFollow: false, m_FadeOutDuration: 0 },
};
const SOUND_CLASS = /^(SQEXSEAD|AnimNotify_PlaySound)|Voice|PlaySE|SwingSE|HitSE|FootStep/;
const EFFECT_CLASS = /AttachEffect|PlayEffect|SpawnEffect|Particle/;
const SWN = ['NOTHING', 'SWING1', 'SWING2', 'SWING3', 'SWING4', 'SWING5', 'SWING6', 'SWING7', 'SWING8', 'SWING9', 'SWING10', 'SWING11', 'SWING12', 'SWING13'];
const HTN = ['NOTHING', 'HIT1', 'HIT2', 'HIT3', 'HIT4', 'HIT5', 'HIT6', 'HIT7', 'HIT8', 'HIT9', 'HIT10', 'HIT11'];
const MOT = ['None', 'Walk', 'Run', 'Jump', 'LandNormal', 'LandHard', 'FootShuffle', 'RustleShort', 'RustleLong', 'RustleFootShort', 'RustleFootLong', 'WingFlapUp', 'WingFlapDown', 'SwingKnock', 'SwingRub', 'CrouchDown', 'StandUp'];
const PER = ['Walk', 'Run', 'Jump', 'LandNormal', 'LandHard', 'FootShuffle'];
const NON = ['RustleShort', 'RustleLong', 'RustleFootShort', 'RustleFootLong', 'WingFlapUp', 'WingFlapDown', 'SwingKnock', 'SwingRub', 'CrouchDown', 'StandUp'];
const SURF = { 0: 'Default', 1: 'mud', 2: 'soil', 4: 'sand', 6: 'grass', 8: 'fallen leaves', 9: 'snow', 10: 'rock/concrete', 11: 'marble/ice', 12: 'carpet', 13: 'wood', 14: 'cloth', 15: 'glass', 16: 'plastic', 17: 'roof tile', 18: 'net/mesh', 19: 'metal', 20: 'iron plate', 21: 'treasure', 22: 'cardboard box', 25: 'gravel', 30: 'frost', 31: 'snow (2)', 32: 'puddle', 33: 'water knee', 34: 'water waist', 35: 'water 3', 36: 'water 4', 37: 'water 5' };
const enumNum = (s, names) => names.indexOf(String(s).split('::').pop());
const idxOf = (k) => { const m = /\[(\d+)\]$/.exec(k); return m ? +m[1] : 0; };
const str = (v) => (typeof v === 'string' ? v : v && v.s !== undefined ? v.s : null);

// ------------------------------------------------------------------ per member
const effects = new Map(); // particle system path -> Set(used_by)
const useEffect = (p, by) => { const a = objPath(p); if (!a || !/\/Effects\//.test(a)) return; if (!effects.has(a)) effects.set(a, new Set()); effects.get(a).add(by); };
const out = { format: 'kh3-party-audio-rows/1', generator: `tools/party_audio_pack.js ${VERSION}`, members: {} };
const partyTables = { format: 'kh3-sora-audio-tables/1', generator: `tools/party_audio_pack.js ${VERSION}`, note: 'the party spell actor classes (tables.json projectiles format)', projectiles: [] };
const partyHost = { format: 'kh3-sora-vfx-host/1', generator: `tools/party_audio_pack.js ${VERSION}`, note: 'party spell actor classes -> effects by role (host.json projectiles format)', projectiles: {} };
const census = {};
const actorVoices = {}; // TresActorVoice asset name -> its sound sections' assets
partyTables.actor_voices = actorVoices;

for (const M of MEMBERS) {
  const rows = { notify_rows: [], voice_rows: [], footstep_rows: [], cloth_rows: [], weapon_rows: [], seattach_rows: [], projectile_rows: [] };
  out.members[M.owner] = Object.assign({ member: M.member, name: M.name }, rows);
  const C = (census[M.owner] = { notify_classes: {}, routes: {} });
  // --- weapons: the 14 models and their swing / hit sets (the class chain: Goofy's sets sit on w_gu_WepBase)
  const weapons = [];
  for (let n = 10; n <= 140; n += 10) {
    const model = M.weaponPrefix + String(n).padStart(3, '0'); const rel = `${M.bp}/${model}_Wep`;
    if (!loadPkg(rel)) continue;
    const enumset = (name) => { const p = chainProp(rel, name); if (!p) return null; const d = loadPkg(gamePathToRel(objPath(p.value) + '.x')); return d ? { set: objPath(p.value), from: p.from, refs: simp(d, d.exports[0].data.props).References || [] } : null; };
    const w = { model, rel, swing: enumset('m_SoundAssets_Swing'), hit: enumset('m_SoundAssets_Hit') };
    weapons.push(w);
    for (const [kind, set, names] of [['weapon_swing', w.swing, SWN], ['weapon_hit', w.hit, HTN]]) {
      if (!set) continue;
      set.refs.forEach((r, i) => rows.weapon_rows.push(Object.assign({ kind, weapon: model, set: set.set, set_from: '/Game/' + set.from, index: i, enum_name: names[i] || null, bFollow: r.bFollow, AttachPointName: r.AttachPointName }, r.Sound ? brief(r.Sound) : { asset: null })));
    }
    for (const k of ['m_AppearEffect', 'm_DisappearEffect']) { const p = chainProp(rel, k); if (p) useEffect(p.value, `${model} ${k}`); }
  }
  const W0 = weapons[0];
  // --- voice group set
  const vd = loadPkg(`${M.bp}/DataTable/${M.owner}_VoiceGroupSet`);
  const VG = vd ? simp(vd, vd.exports[0].data.props).m_VoiceGroups : [];
  for (const g of VG) for (const u of g.m_Table) rows.voice_rows.push(Object.assign({ kind: 'voice_group', table: `/Game/${M.bp}/DataTable/${M.owner}_VoiceGroupSet`, group: g.m_GroupName, group_no: u.m_GroupNo, play_on_group_not_found: g.m_bPlayOnGroupNotFound, default_voice: objPath(g.m_pDefaultVoice) }, brief(u.m_pVoice) || {}));
  // --- notifies of every anim set
  const seqs = new Map();
  for (const s of SETS) {
    const setRel = `Character/npc/${M.owner}/anm/${M.owner}_${s}`; const d = loadPkg(setRel); if (!d) continue;
    const e = d.exports.find((x) => x.class === 'TresAnimSet'); const anims = simp(d, e.data.props).Anims || [];
    for (const r of anims) {
      if (!r.AnimData) continue; const rel = gamePathToRel(r.AnimData); const p = loadPkg(rel); if (!p) continue;
      const main = p.exports.find((x) => x.objectName === rel.split('/').pop()) || p.exports[0];
      const add = (srel, via) => { if (!seqs.has(srel)) seqs.set(srel, []); seqs.get(srel).push({ set: s, name: r.AnimName, via }); };
      if (main.class === 'AnimSequence') add(rel, null);
      else { const seen = new Set(); for (const im of p.imports) if (im.className === 'AnimSequence' && !/^Default__/.test(im.objectName)) { const pk = p.imports[-im.outerIndex - 1]; const srel = gamePathToRel(pk.objectName + '.x'); if (!seen.has(srel)) { seen.add(srel); add(srel, main.class + ' ' + r.AnimData); } } }
    }
  }
  for (const [rel, infos] of seqs) {
    const sf = AN + rel + '.anim.json'; const p = loadPkg(rel); if (!fs.existsSync(sf) || !p) continue;
    const j = JSON.parse(fs.readFileSync(sf, 'utf8')); const P = {}; for (const q of j.data.props) P[q.tag.name] = ssimp(q.value);
    const len = P.SequenceLength, nf = P.NumFrames;
    for (const n of P.Notifies || []) {
      const isState = !!(n.NotifyStateClass && n.NotifyStateClass.object); const idx = isState ? n.NotifyStateClass.object : n.Notify && n.Notify.object; if (!idx) continue;
      const ex = exportProps(p, idx); C.notify_classes[ex.class] = (C.notify_classes[ex.class] || 0) + 1;
      // effects of AttachEffect notifies (for the effect list): their asset references
      if (EFFECT_CLASS.test(ex.class)) { const walk = (v) => { if (typeof v === 'string') useEffect(v, `${M.owner} notify ${ex.class} @ ${rel.split('/').pop()}`); else if (v && typeof v === 'object') Object.values(v).forEach(walk); }; walk(ex.props); }
      if (!SOUND_CLASS.test(ex.class) && ex.class !== 'TresAnimNotifyState_Attack') continue;
      const c = ex.class; const pr = Array.isArray(ex.props) ? {} : ex.props;
      if (c === 'TresAnimNotifyState_Attack' && !('m_HitSEAsset' in pr) && !('m_WeaponHitSEAliasID' in pr)) continue;
      const d0 = DEF[c]; let eff = null; if (d0) { eff = Object.assign({}, d0, pr); if (d0.PlaySettings) eff.PlaySettings = Object.assign({}, d0.PlaySettings, pr.PlaySettings || {}); }
      const t = n.LinkValue;
      const row = { owner: M.owner, anim_asset: '/Game/' + rel, sequence_length: len, num_frames: nf, logical: infos.map((i) => (i.set === 'AnimSet' && !i.via ? i.name : (i.set === 'AnimSet' ? '' : i.set + ':') + i.name + (i.via ? ' (sample of ' + i.via + ')' : ''))), notify_class: c, notify_object: ex.name, is_state: isState, time_s: t, duration_s: isState ? n.Duration : null, props_serialized: pr, props_effective: eff, sounds: [], route: null, plays: true, note: null };
      if (c === 'SQEXSEADAnimNotify_PlaySound') { row.route = 'sead_play_sound'; row.sounds.push(brief(pr.Sound, { role: 'Sound' })); }
      else if (c === 'SQEXSEADAnimNotifyState_AttachSound') { row.route = 'sead_attach_sound'; row.sounds.push(brief(pr.Sound, { role: 'Sound' })); if (pr.LoopEndSound) row.sounds.push(brief(pr.LoopEndSound, { role: 'LoopEndSound' })); }
      else if (/PlayAutoSe$/.test(c)) {
        const s = eff.PlaySettings; const src = s.SourceType.split('::').pop(); row.route = 'auto_se';
        if (src === 'MotionSoundType') { const m = enumNum(s.MotionSoundType, MOT); row.auto_se = { source: 'MotionSoundType', motion_sound_type: MOT[m], table: '/Game/Sound/SE/_Data/MASTS/' + M.fstable }; }
        else if (src === 'AuxSurfaceTable') row.auto_se = { source: 'AuxSurfaceTable', record: s.SurfaceAssetTableRecordName, table: '/Game/Sound/SE/_Data/MASTS/' + M.fstable };
        else row.auto_se = { source: src, table: s.SurfaceAssetPathTable };
      } else if (c === 'SQEXSEADAnimNotifyState_ChangeAutoSeEnable') { row.route = 'auto_se_enable'; row.plays = false; row.note = 'auto-SE enable ' + eff.bEnableSetting + ' for the window'; }
      else if (/PlayVoice$/.test(c)) {
        if (pr.m_VoiceAsset) { row.route = 'voice_direct'; row.sounds.push(brief(pr.m_VoiceAsset, { role: 'm_VoiceAsset' })); }
        else if ((pr.m_RandomVoiceAssets || []).length) { row.route = 'voice_random'; pr.m_RandomVoiceAssets.forEach((v, i) => row.sounds.push(brief(v.Voice, { role: 'm_RandomVoiceAssets[' + i + ']', weight: v.RandomWeights || 0 }))); }
        else if (pr.m_VoiceGroupName) {
          row.route = 'voice_group'; row.voice_group = pr.m_VoiceGroupName; const g = VG.find((x) => x.m_GroupName === pr.m_VoiceGroupName);
          if (!g) row.note = `group not in ${M.owner}_VoiceGroupSet`; else g.m_Table.forEach((u) => row.sounds.push(brief(u.m_pVoice, { role: 'group ' + g.m_GroupName + ' no ' + u.m_GroupNo, group_no: u.m_GroupNo })));
        } else { row.route = 'voice_none'; row.plays = false; row.note = 'no asset, no random list, no group'; }
      } else if (c === 'TresAnimNotify_PlayActorVoice') {
        // TresActorVoice: tracks of sound sections (m_Sound, m_Duration) and subtitle sections (m_SubtitleKey) [A]
        row.route = pr.m_ActorVoice ? 'actor_voice' : 'actor_voice_null'; row.plays = !!pr.m_ActorVoice;
        if (pr.m_ActorVoice) {
          const av = objPath(pr.m_ActorVoice); const ad = loadPkg(gamePathToRel(av + '.x')); row.actor_voice = av;
          for (const e of ad ? ad.exports : []) { const q = simp(ad, (e.data && e.data.props) || []); if (e.class === 'TresActorVoiceSoundSection' && q.m_Sound) row.sounds.push(brief(q.m_Sound, { role: 'TresActorVoiceSoundSection.m_Sound', duration_section: q.m_Duration })); if (e.class === 'TresActorVoiceSubtitleSection') row.subtitle = q.m_SubtitleKey; }
          actorVoices[av.split('/').pop()] = row.sounds.map((s) => s.asset);
        }
      }
      else if (/PlayWeaponSwingSE$/.test(c)) {
        row.route = 'weapon_swing_alias'; const i = enumNum(eff.m_SoundAliasID, SWN); row.alias_index = i; row.alias_unit = eff.m_SoundAliasUnit.split('::').pop();
        const ref = W0 && W0.swing && W0.swing.refs[i]; row.resolved_with = `${W0 ? W0.model : '?'} swing set [${i}] (the equipped weapon's set decides at run time)`;
        if (ref && ref.Sound) row.sounds.push(brief(ref.Sound, { role: `${W0.model} swing ${i}` })); else { row.note = `reference ${i} has no sound on ${W0 ? W0.model : '?'}`; }
      } else if (c === 'TresAnimNotify_PlayWeaponHitSE') {
        row.route = 'weapon_hit_alias'; const i = enumNum(eff.m_SoundAliasID, HTN); row.alias_index = i;
        const ref = W0 && W0.hit && W0.hit.refs[i]; if (ref && ref.Sound) row.sounds.push(brief(ref.Sound, { role: `${W0.model} hit ${i}` })); else row.note = `reference ${i} has no sound on ${W0 ? W0.model : '?'}`;
      } else if (c === 'TresAnimNotifyState_Attack') {
        row.route = 'attack_hit_se'; row.plays = false; row.note = 'attack window: the hit SE plays when the attack connects';
        if (pr.m_HitSEAsset) row.sounds.push(brief(pr.m_HitSEAsset, { role: 'm_HitSEAsset' }));
        if (pr.m_WeaponHitSEAliasID) { const i = enumNum(pr.m_WeaponHitSEAliasID, HTN); row.alias_index = i; const ref = W0 && W0.hit && W0.hit.refs[i]; if (ref && ref.Sound) row.sounds.push(brief(ref.Sound, { role: `${W0.model} hit ${i}` })); }
      } else row.route = 'other';
      row.sounds = row.sounds.filter(Boolean);
      C.routes[row.route] = (C.routes[row.route] || 0) + 1;
      rows.notify_rows.push(row);
    }
  }
  // --- footstep table
  const fd = loadPkg('Sound/SE/_Data/MASTS/' + M.fstable);
  if (fd) {
    const p = simp(fd, fd.exports[0].data.props); const tbl = '/Game/Sound/SE/_Data/MASTS/' + M.fstable;
    for (const k in p) {
      if (/^PerSurfaceInfos/.test(k)) { const s = idxOf(k); for (const kk in p[k]) { const a = str(p[k][kk]); if (!a) continue; const m = idxOf(kk); rows.footstep_rows.push({ table: tbl, route: 'per_surface', surface_type: s, surface_name: SURF[s] || null, slot: m, slot_name: PER[m], sound: strip(a), resolved: brief(a) }); } }
      else if (k === 'NonSurfaceInfos') { for (const kk in p[k]) { const a = str(p[k][kk]); if (!a) continue; const m = idxOf(kk); rows.cloth_rows.push({ table: tbl, route: 'non_surface', slot: m, slot_name: NON[m], sound: strip(a), resolved: brief(a) }); } }
      else if (k === 'AuxSurfaceAssetTable') { for (const rec of p[k].Records || []) for (const kk in rec) { if (!/^AssetRefs/.test(kk)) continue; const a = str(rec[kk]); if (!a) continue; const s = idxOf(kk); rows.footstep_rows.push({ table: tbl, route: 'aux_surface', record: rec.Name, surface_type: s, surface_name: SURF[s] || null, sound: strip(a), resolved: brief(a) }); } }
    }
  }
  // --- the member's own particle systems (Effects/NPC/n_ex00x/par) and weapon hit effects
  for (const dir of [`Effects/NPC/${M.owner}/par`]) {
    const abs = PK + dir; if (!fs.existsSync(abs)) continue;
    for (const f of fs.readdirSync(abs)) if (f.endsWith('.uasset.json')) useEffect(`/Game/${dir}/${f.replace('.uasset.json', '')}`, `${M.owner} effect folder`);
  }
  for (const w of weapons) { const dir = `Effects/wep/${w.model}/par`; if (fs.existsSync(PK + dir)) for (const f of fs.readdirSync(PK + dir)) if (f.endsWith('.uasset.json')) useEffect(`/Game/${dir}/${f.replace('.uasset.json', '')}`, `${w.model} effect folder`); }
  // --- magic set / ead (the spells' effect groups)
  for (const extra of [`${M.bp}/${M.owner}_MagicSet`, `${M.bp}/${M.owner}_Magic_ead`]) {
    const d = loadPkg(extra); if (!d) continue;
    for (const e of d.exports) { const walk = (v) => { if (typeof v === 'string') useEffect(v, extra.split('/').pop()); else if (v && typeof v === 'object') Object.values(v).forEach(walk); }; walk(simp(d, (e.data && e.data.props) || [])); }
  }
  // --- spell actors: Blueprints/Npc/n_ex00x/n_ex00x_Projectile*
  const bpDir = PK + M.bp;
  for (const f of fs.readdirSync(bpDir).filter((x) => /_Projectile.*\.uasset\.json$/.test(x))) {
    const rel = `${M.bp}/${f.replace('.uasset.json', '')}`; const d = loadPkg(rel); const cls = rel.split('/').pop() + '_C';
    const roles = {}; const role = (r, p) => { const a = objPath(p); if (!a) return; useEffect(a, cls + ' ' + r); const n = a.split('/').pop(); (roles[r] = roles[r] || []).includes(n) || roles[r].push(n); };
    let fireSe = null, pawnHit = null, mapHit = null;
    for (const e of d.exports) {
      const pr = simp(d, (e.data && e.data.props) || []);
      if (/^Default__/.test(e.objectName)) { for (const fe of pr.MyFireEffects || []) { role('fire', fe.MyParticleSystem); if (fe.MySoundCue && !fireSe) fireSe = objPath(fe.MySoundCue); } if (pr.m_PutEffect) role('PutEffect', pr.m_PutEffect); }
      if (e.class === 'ParticleSystemComponent' && pr.Template) role('body', pr.Template);
      if (e.class === 'TresAtkCollComponent') for (const h of pr.m_HitEffects || []) { role('hit', h.m_PawnHitEffect); role('map_hit', h.m_MapHitEffect); if (h.m_PawnHitSEAsset && !pawnHit) pawnHit = objPath(h.m_PawnHitSEAsset); if (h.m_MapHitSEAsset && !mapHit) mapHit = objPath(h.m_MapHitSEAsset); }
    }
    rows.projectile_rows.push({ class: cls, blueprint: '/Game/' + rel, effects: roles, fire_se: fireSe, pawn_hit_se: pawnHit, map_hit_se: mapHit, sounds: [fireSe, pawnHit, mapHit].filter(Boolean).map((a) => brief(a)) });
    partyTables.projectiles.push({ class: cls, chain: [rel.split('/').pop()], command: null, effects: roles, fire_se: fireSe, pawn_hit_se: pawnHit, map_hit_se: mapHit });
    partyHost.projectiles[cls] = roles;
  }
}

// ------------------------------------------------------------------ team-attack link pawns' effects (stream D draws them through E's packs)
for (const rel of ['Blueprints/FriendLink/CallMeteor_Pawn', 'Blueprints/FriendLink/CallMeteor_Proj', 'Blueprints/FriendLink/CallMeteor_Exp_Proj', 'Blueprints/FriendLink/FlareForce_Pawn', 'Blueprints/FriendLink/FlareForce_Proj', 'Blueprints/FriendLink/FlareForce_ExpProj', 'Blueprints/FriendLink/GoofyShoot_Pawn', 'Blueprints/FriendLink/GoofyShoot_Proj', 'Blueprints/FriendLink/ShieldGuardian_Pawn']) {
  const d = loadPkg(rel); if (!d) { census['missing ' + rel] = 1; continue; }
  for (const e of d.exports) { const walk = (v) => { if (typeof v === 'string') useEffect(v, rel.split('/').pop()); else if (v && typeof v === 'object') Object.values(v).forEach(walk); }; walk(simp(d, (e.data && e.data.props) || [])); }
  for (const im of d.imports) if (im.className === 'ParticleSystem') { let o = im; while (o.outerIndex) o = d.imports[-o.outerIndex - 1]; useEffect(o.objectName + '.x', rel.split('/').pop() + ' (import)'); }
}

// ------------------------------------------------------------------ effect-attached SE of every party effect (every SeAttach folder)
const seIndex = new Map(); // effect name (lower) -> [seattach package rel]
// (Windows' folder order on every system: tools/lib/ntfs_order.js)
(function walk(dir) { for (const f of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => require('./lib/ntfs_order').ntfsCompare(a.name, b.name))) { const p = path.join(dir, f.name); if (f.isDirectory()) walk(p); else if (f.name.endsWith('.uasset.json')) { const rel = path.relative(PK, p).split(path.sep).join('/').replace(/\.uasset\.json$/, ''); const k = rel.split('/').pop().toLowerCase(); if (!seIndex.has(k)) seIndex.set(k, []); seIndex.get(k).push(rel); } } })(PK + 'Sound/SE/_Data/SeAttach');
const seRows = [];
for (const [eff, by] of [...effects].sort()) {
  const name = eff.split('/').pop();
  for (const rel of seIndex.get(name.toLowerCase()) || []) {
    const d = loadPkg(rel); if (!d) continue;
    for (let i = 1; i <= d.exports.length; i++) {
      const e = d.exports[i - 1]; if (e.class !== 'SQEX_EffectSeAttachData') continue;
      const pr = exportProps(d, i).props; const a = pr.Sound ? objPath(pr.Sound) : null;
      seRows.push({ category: 'effect_se_npc', trigger: 'Effect ' + name + (pr.bPlayAtEnd ? ' ends' : ' starts'), ref: '/Game/' + rel + ' :: ' + e.objectName + ' (played by 0x14425F080)', verdict: 'same-partial', condition: 'GroupID ' + (pr.GroupID ?? 0) + (pr.AttachEmitterName ? '; emitter ' + pr.AttachEmitterName : ''), sound_id: 'SQEX_EffectSeAttachData.Sound', sound_asset: a, spatial: '3D', attach: 'attached to the particle component (bAttach default true)', volume: 'asset default', pitch: null, loop_stop: 'FadeOutTime ' + (pr.FadeOutTime ?? null) + (pr.bNoSeFadeAtTheEffectEnd ? '; bNoSeFadeAtTheEffectEnd' : '') + (pr.bPlayAtEnd ? '; bPlayAtEnd' : ''), evidence: 'asset', note: a ? 'party effect (' + [...by].slice(0, 3).join('; ') + ')' : 'entry without a Sound (silent)' });
    }
  }
}
out.code_rows = seRows;
// spell actors' fire / hit SE as code rows (the classes are named by the effect seam; spec 5.4 / 5.6)
for (const M of MEMBERS) for (const p of out.members[M.owner].projectile_rows) {
  if (p.fire_se) seRows.push({ category: 'projectile', trigger: `${p.class.replace(/_C$/, '')}: fire effect SE (projectile spawn)`, ref: p.blueprint + ' :: MyFireEffects[].MySoundCue', verdict: 'same / same-partial', sound_id: 'MyFireEffects[0].MySoundCue', sound_asset: p.fire_se, spatial: '3D', volume: 1, pitch: 1, loop_stop: 'one-shot', evidence: 'asset', note: 'party spell actor' });
  if (p.pawn_hit_se) seRows.push({ category: 'hit_magic', trigger: `${p.class.replace(/_C$/, '')}: pawn hit SE`, ref: p.blueprint + ' :: m_HitEffects[0].m_PawnHitSEAsset', verdict: 'same-partial', sound_id: 'm_PawnHitSEAsset', sound_asset: p.pawn_hit_se, spatial: '3D', volume: 1, pitch: 1, loop_stop: 'one-shot', evidence: 'asset', note: 'party spell actor' });
  if (p.map_hit_se) seRows.push({ category: 'hit_magic', trigger: `${p.class.replace(/_C$/, '')}: map hit SE`, ref: p.blueprint + ' :: m_HitEffects[0].m_MapHitSEAsset', verdict: 'same-partial', sound_id: 'm_MapHitSEAsset', sound_asset: p.map_hit_se, spatial: '3D', volume: 1, pitch: 1, loop_stop: 'one-shot', evidence: 'asset', note: 'party spell actor' });
}

// ------------------------------------------------------------------ effect list (tools/sora_vfx_pack.js --list format)
const effectList = [...effects].sort().map(([eff, by]) => {
  const rel = gamePathToRel(eff + '.x'); const d = loadPkg(rel);
  return { effect: eff, package: 'KINGDOM HEARTS III/Content/' + rel + '.uasset', exists: !!d, used_by: [...by].sort(), class: d ? (d.exports.find((e) => e.objectName === rel.split('/').pop()) || d.exports[0]).class : null };
}).filter((e) => e.exists && e.class === 'ParticleSystem');

// ------------------------------------------------------------------ census + write
for (const M of MEMBERS) {
  const m = out.members[M.owner]; const c = census[M.owner];
  c.counts = { notify_rows: m.notify_rows.length, voice_rows: m.voice_rows.length, footstep_rows: m.footstep_rows.length, cloth_rows: m.cloth_rows.length, weapon_rows: m.weapon_rows.length, projectiles: m.projectile_rows.length };
  c.unresolved_sounds = [...new Set(m.notify_rows.flatMap((r) => r.sounds).concat(m.voice_rows).filter((s) => s && s.asset && !s.exists).map((s) => s.asset))];
}
census.effects = effectList.length; census.effect_se_rows = seRows.filter((r) => r.category === 'effect_se_npc').length;
console.log(JSON.stringify(census, null, 1));
if (!checkOnly) {
  const W = (f, v) => { fs.mkdirSync(path.dirname(path.join(ROOT, f)), { recursive: true }); fs.writeFileSync(path.join(ROOT, f), typeof v === 'string' ? v : JSON.stringify(v, null, 1) + '\n'); console.log('wrote ' + f); };
  W('audit/sora_port/audio/party_rows.json', out);
  W('build/sora_pack/audio/party_tables.json', partyTables);
  W('build/sora_pack/party/vfx/party_host.json', partyHost); // (party I2) the party's own effect folder, never PACK/vfx (the Elden Ring build reads it)
  W('build/sora_tmp/party/impl_e/party_effect_list.json', effectList);
  // sheets: one row per trigger (sounds), per voice unit, per effect
  const cell = (v) => (v === undefined ? null : v);
  const soundRows = [];
  for (const M of MEMBERS) {
    const m = out.members[M.owner];
    m.notify_rows.forEach((r, i) => soundRows.push([`${M.owner}.notify.${r.anim_asset.split('/').pop()}.${i}`, M.owner, 'notify.' + r.route, `${r.notify_class} in ${r.anim_asset}`, r.logical.join(' | ') || '-', +r.time_s.toFixed(4), r.sounds.map((s) => s.asset).join(' | ') || (r.auto_se ? `auto-SE ${r.auto_se.motion_sound_type || r.auto_se.record}` : '-'), r.plays ? 'yes' : 'no', 'asset', r.note || '-']));
    m.footstep_rows.forEach((r) => soundRows.push([`${M.owner}.autose.${r.surface_type}.${r.route === 'aux_surface' ? 'aux.' + r.record : r.slot_name}`, M.owner, 'autose.' + r.route, `${M.fstable} surface ${r.surface_type} (${r.surface_name})`, '-', '-', r.sound, r.resolved && r.resolved.exists ? 'yes' : 'no', 'asset', '-']));
    m.cloth_rows.forEach((r) => soundRows.push([`${M.owner}.autose.cloth.${r.slot_name}`, M.owner, 'autose.non_surface', `${M.fstable} NonSurfaceInfos[${r.slot}]`, '-', '-', r.sound, r.resolved && r.resolved.exists ? 'yes' : 'no', 'asset', '-']));
    m.weapon_rows.forEach((r) => soundRows.push([`${M.owner}.${r.weapon}.${r.kind === 'weapon_swing' ? 'swing' : 'hit'}.${r.index}`, M.owner, r.kind === 'weapon_swing' ? 'weapon.swing_set' : 'weapon.hit_set', `${r.set} [${r.index}] ${r.enum_name}`, '-', '-', r.asset || '-', r.asset ? (r.exists ? 'yes' : 'no') : 'empty', 'asset', '-']));
  }
  for (const r of seRows) soundRows.push([`code.${r.category}.${r.trigger.replace(/[^A-Za-z0-9]+/g, '_').slice(0, 60)}`, '-', 'code.' + r.category, r.trigger, '-', '-', r.sound_asset || '-', r.sound_asset ? 'yes' : 'no', r.evidence, r.loop_stop]);
  const seen = new Set(); for (const r of soundRows) { let id = r[0], n = 1; while (seen.has(id)) id = `${r[0]}~${++n}`; r[0] = id; seen.add(id); }
  W('sheets/port/party-sounds.json', `{"sheet":"party-sounds","group":"port","columns":["id","owner","route","trigger","logical","time","sound","packable","evidence","note"],"rows":[\n${soundRows.map((r) => JSON.stringify(r)).join(',\n')}\n]}\n`);
  const voiceRows = [];
  for (const M of MEMBERS) for (const v of out.members[M.owner].voice_rows) voiceRows.push([`${M.owner}.${v.group}.${v.group_no}`, M.owner, v.group, v.group_no, v.asset || '-', v.en ? v.en.asset : '-', v.exists ? 'yes' : 'no', v.play_on_group_not_found ? 'yes' : 'no', 'asset', v.table]);
  W('sheets/port/party-voice.json', `{"sheet":"party-voice","group":"port","columns":["id","owner","group","number","voice_ja","voice_en","exists","play_on_group_not_found","evidence","source"],"rows":[\n${voiceRows.map((r) => JSON.stringify(r)).join(',\n')}\n]}\n`);
  const effRows = effectList.map((e) => [e.effect.split('/').pop(), e.effect, e.used_by.slice(0, 6).join(' | '), e.used_by.length, 'asset']);
  for (const M of MEMBERS) for (const p of out.members[M.owner].projectile_rows) effRows.push([p.class, p.blueprint, Object.entries(p.effects).map(([r, n]) => `${r}: ${n.join(' ')}`).join(' | '), Object.keys(p.effects).length, 'asset']);
  W('sheets/port/party-effects.json', `{"sheet":"party-effects","group":"port","columns":["id","asset","used_by","uses","evidence"],"rows":[\n${effRows.map((r) => JSON.stringify(r)).join(',\n')}\n]}\n`);
}
