// Builds the final machine-readable outputs from the scan results:
//   node scan_notifies.js ; node scan_footsteps.js ; node build_outputs.js
// -> notify_rows.json (final), bp_event_rows.json, voice_rows.json, weapon_cloth_rows.json, sound_assets.json
const L = require('./lib.js'); const fs = L.fs;
const raw = JSON.parse(fs.readFileSync(__dirname + '/notify_rows.raw.json', 'utf8'));
const E = 'ESQEXSEAD';
// class defaults read from the constructors (instruction level; addresses in NOTES.md)
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
const strip = p => p ? p.replace(/\.[^./]+$/, '') : p;
const sounds = new Map(); // asset -> {r, by:Set}
function use(asset, by) { if (!asset) return null; const a = strip(asset); const r = L.resolveSound(asset); if (!sounds.has(a)) sounds.set(a, { r, by: new Set() }); sounds.get(a).by.add(by); return r; }
function short(r, asset) { return { asset: strip(asset), exists: r.exists, bank_file: r.bank_file || null, sound_index: r.sound_index === undefined ? null : r.sound_index, volume: r.volume === undefined ? null : r.volume, duration: r.duration === undefined ? null : r.duration }; }
function brief(asset, by, extra) {
  const r = use(asset, by); if (!r) return null;
  const o = Object.assign(short(r, asset), extra || {});
  if (/\/Localization\/ja\//.test(asset)) { const en = L.enVariant(asset); const re = use(en, by + ' (en)'); o.en = short(re, en); }
  return o;
}
// --- data tables --------------------------------------------------------------------------
const wd = L.loadPkg('Blueprints/Player/p_ex001/w_so010_Wep');
const wcdo = L.simp(wd, wd.exports.find(e => e.objectName === 'Default__w_so010_Wep_C').data.props);
function enumset(path) { const d = L.loadPkg(L.gamePathToRel(path)); return L.simp(d, d.exports[0].data.props).References; }
const SW = enumset(wcdo.m_SoundAssets_Swing), HT = enumset(wcdo.m_SoundAssets_Hit);
const ad = L.loadPkg(L.gamePathToRel(wcdo.m_SoundAliasAsset)); const ALIAS = L.simp(ad, ad.exports[0].data.props);
const vd = L.loadPkg('Blueprints/Player/p_ex001/p_ex001_VoiceGroupSet'); const VG = L.simp(vd, vd.exports[0].data.props).m_VoiceGroups;
const foot = JSON.parse(fs.readFileSync(__dirname + '/footstep_rows.json', 'utf8'));
const cloth = JSON.parse(fs.readFileSync(__dirname + '/cloth_autose_rows.json', 'utf8'));
const enumNum = (s, names) => names.indexOf(String(s).split('::').pop());
const SWN = ['NOTHING', 'SWING1', 'SWING2', 'SWING3', 'SWING4', 'SWING5', 'SWING6', 'SWING7', 'SWING8', 'SWING9', 'SWING10', 'SWING11', 'SWING12', 'SWING13'];
const HTN = ['NOTHING', 'HIT1', 'HIT2', 'HIT3', 'HIT4', 'HIT5', 'HIT6', 'HIT7', 'HIT8', 'HIT9', 'HIT10', 'HIT11'];
const MOT = ['None', 'Walk', 'Run', 'Jump', 'LandNormal', 'LandHard', 'FootShuffle', 'RustleShort', 'RustleLong', 'RustleFootShort', 'RustleFootLong', 'WingFlapUp', 'WingFlapDown', 'SwingKnock', 'SwingRub', 'CrouchDown', 'StandUp'];
const TABLE = '/Game/Sound/SE/_Data/MASTS/FsTable_Sora';
// --- notify rows --------------------------------------------------------------------------
const out = [], bp = [];
for (const x of raw) {
  const c = x.notify_class; const p = Array.isArray(x.props) ? {} : x.props; const by = c + ' @ ' + x.anim_asset.split('/').pop();
  if (c === 'TresAnimNotifyState_BpEvent' || /_C$/.test(c)) { bp.push(x); continue; }
  const d = DEF[c]; let eff = null;
  if (d) { eff = Object.assign({}, d, p); if (d.PlaySettings) eff.PlaySettings = Object.assign({}, d.PlaySettings, p.PlaySettings || {}); }
  const row = Object.assign({}, x, { props_serialized: p, props_effective: eff }); delete row.props;
  row.sounds = []; row.route = null; row.plays = true; row.note = null;
  if (c === 'SQEXSEADAnimNotify_PlaySound') { row.route = 'sead_play_sound'; row.sounds.push(brief(p.Sound, by, { role: 'Sound' })); }
  else if (c === 'SQEXSEADAnimNotifyState_AttachSound') {
    row.route = 'sead_attach_sound'; row.sounds.push(brief(p.Sound, by, { role: 'Sound' }));
    if (p.LoopEndSound) row.sounds.push(brief(p.LoopEndSound, by, { role: 'LoopEndSound' }));
  }
  else if (c === 'SQEXSEADAnimNotify_PlayAutoSe' || c === 'SQEXSEADAnimNotifyState_PlayAutoSe') {
    const s = eff.PlaySettings; const src = s.SourceType.split('::').pop(); row.route = 'auto_se';
    if (src === 'MotionSoundType') {
      const m = enumNum(s.MotionSoundType, MOT);
      row.auto_se = { source: 'MotionSoundType', motion_sound_type: MOT[m], motion_sound_type_value: m, table: TABLE,
        lookup: m >= 1 && m <= 6 ? 'PerSurfaceInfos[surface].SoundAssetPaths[' + (m - 1) + ']' : m >= 7 ? 'NonSurfaceInfos.SoundAssetPaths[' + (m - 7) + ']' : 'none',
        join: m >= 1 && m <= 6 ? { file: 'footstep_rows.json', table: TABLE, route: 'per_surface', slot: m - 1 } : { file: 'weapon_cloth_rows.json', table: TABLE, kind: 'cloth_auto_se', slot: m - 7 } };
    } else if (src === 'AuxSurfaceTable') {
      row.auto_se = { source: 'AuxSurfaceTable', record: s.SurfaceAssetTableRecordName, table: TABLE,
        lookup: 'AuxSurfaceAssetTable.Records[Name==' + s.SurfaceAssetTableRecordName + '].AssetRefs[surface]',
        join: { file: 'footstep_rows.json', table: TABLE, route: 'aux_surface', record: s.SurfaceAssetTableRecordName } };
    } else row.auto_se = { source: src, table: s.SurfaceAssetPathTable };
    row.auto_se.parts_type = s.AutoSePartsType.split('::').pop(); row.auto_se.parts_index = s.AutoSePartsIndex;
  }
  else if (c === 'SQEXSEADAnimNotifyState_ChangeAutoSeEnable') {
    row.route = 'auto_se_enable'; row.plays = false;
    row.note = 'sets the auto-SE enable flag to bEnableSetting=' + eff.bEnableSetting + ' for the window; at end: ' + eff.NotifyEndBehavior.split('::').pop();
  }
  else if (c === 'TresAnimNotify_PlayVoice' || c === 'TresAnimNotifyState_PlayVoice') {
    if (p.m_VoiceAsset) { row.route = 'voice_direct'; row.sounds.push(brief(p.m_VoiceAsset, by, { role: 'm_VoiceAsset' })); }
    else if ((p.m_RandomVoiceAssets || []).length) {
      row.route = 'voice_random'; row.random_total_weight = p.m_RandomVoiceAssets.reduce((a, v) => a + (v.RandomWeights || 0), 0);
      p.m_RandomVoiceAssets.forEach((v, i) => row.sounds.push(brief(v.Voice, by, { role: 'm_RandomVoiceAssets[' + i + ']', weight: v.RandomWeights || 0 })));
    }
    else if (p.m_VoiceGroupName) {
      row.route = 'voice_group'; row.voice_group = p.m_VoiceGroupName; const g = VG.find(g => g.m_GroupName === p.m_VoiceGroupName);
      if (!g) row.note = 'group not in p_ex001_VoiceGroupSet';
      else g.m_Table.forEach(u => row.sounds.push(brief(u.m_pVoice, by, { role: 'group ' + g.m_GroupName + ' no ' + u.m_GroupNo, group_no: u.m_GroupNo })));
    }
    else { row.route = 'voice_none'; row.plays = false; row.note = 'no asset, no random list, no group'; }
  }
  else if (c === 'TresAnimNotify_PlayActorVoice') {
    row.route = p.m_ActorVoice ? 'actor_voice' : 'actor_voice_null'; row.plays = !!p.m_ActorVoice;
    if (!p.m_ActorVoice) row.note = 'm_ActorVoice is null: Notify 0x1405604A0 returns without playing'; else row.actor_voice = p.m_ActorVoice;
  }
  else if (c === 'TresAnimNotify_PlayWeaponSwingSE' || c === 'TresAnimNotifyState_PlayWeaponSwingSE') {
    row.route = 'weapon_swing_alias'; const i = enumNum(eff.m_SoundAliasID, SWN); row.alias_index = i; row.alias_unit = eff.m_SoundAliasUnit.split('::').pop();
    const ref = SW[i]; row.resolved_with = 'Kingdom Key w_so010_Wep.m_SoundAssets_Swing.References[' + i + ']';
    if (ref && ref.Sound) row.sounds.push(brief(ref.Sound, by, { role: 'Kingdom Key swing ' + i }));
    else { row.plays = false; row.note = 'reference ' + i + ' has no sound on the Kingdom Key'; }
  }
  else if (c === 'TresAnimNotify_PlayWeaponHitSE') {
    row.route = 'weapon_hit_alias'; const i = enumNum(eff.m_SoundAliasID, HTN); row.alias_index = i; row.alias_unit = eff.m_SoundAliasUnit.split('::').pop();
    const ref = HT[i]; row.resolved_with = 'Kingdom Key w_so010_Wep.m_SoundAssets_Hit.References[' + i + ']';
    if (ref && ref.Sound) row.sounds.push(brief(ref.Sound, by, { role: 'Kingdom Key hit ' + i }));
    else { row.plays = false; row.note = 'reference ' + i + ' missing or null on the Kingdom Key'; }
  }
  else if (c === 'TresAnimNotifyState_Attack') {
    row.route = 'attack_hit_se'; row.plays = false;
    row.note = 'attack window: the hit SE is played by the attack-collision code when the attack connects (not traced in this strand)';
    if (p.m_HitSEAsset) row.sounds.push(brief(p.m_HitSEAsset, by, { role: 'm_HitSEAsset' }));
    if (p.m_WeaponHitSEAliasID) {
      const i = enumNum(p.m_WeaponHitSEAliasID, HTN); row.alias_index = i; const ref = HT[i];
      row.resolved_with = 'Kingdom Key w_so010_Wep.m_SoundAssets_Hit.References[' + i + ']';
      if (ref && ref.Sound) row.sounds.push(brief(ref.Sound, by, { role: 'Kingdom Key hit ' + i }));
      else row.note += '; alias ' + i + ' has no Kingdom Key sound';
    }
  }
  else row.route = 'other';
  row.sounds = row.sounds.filter(Boolean); out.push(row);
}
fs.writeFileSync(__dirname + '/notify_rows.json', JSON.stringify(out, null, 1));
fs.writeFileSync(__dirname + '/bp_event_rows.json', JSON.stringify(bp, null, 1));
// --- voice rows ---------------------------------------------------------------------------
const voice = [];
for (const g of VG) for (const u of g.m_Table) {
  const b = brief(u.m_pVoice, 'VoiceGroupSet ' + g.m_GroupName) || {};
  voice.push(Object.assign({ kind: 'voice_group', table: '/Game/Blueprints/Player/p_ex001/p_ex001_VoiceGroupSet', group: g.m_GroupName, group_no: u.m_GroupNo, play_on_group_not_found: g.m_bPlayOnGroupNotFound, default_voice: strip(g.m_pDefaultVoice) }, b));
}
const agg = new Map();
for (const r of out) if (/^voice_(direct|random)/.test(r.route)) for (const s of r.sounds) {
  if (!agg.has(s.asset)) { const o = Object.assign({ kind: 'notify_voice' }, s, { anims: [] }); delete o.role; delete o.weight; agg.set(s.asset, o); }
  agg.get(s.asset).anims.push({ anim: r.anim_asset.split('/').pop(), logical: r.logical[0], time_s: r.time_s, route: r.route, weight: s.weight === undefined ? null : s.weight, total: r.random_total_weight || null });
}
for (const v of agg.values()) voice.push(v);
fs.writeFileSync(__dirname + '/voice_rows.json', JSON.stringify(voice, null, 1));
// --- weapon / cloth rows ------------------------------------------------------------------
const wc = []; const WEP = '/Game/Blueprints/Player/p_ex001/w_so010_Wep';
SW.forEach((r, i) => wc.push(Object.assign({ kind: 'weapon_swing', weapon: WEP, set: strip(wcdo.m_SoundAssets_Swing), index: i, enum_name: SWN[i] || null, bFollow: r.bFollow, AttachPointName: r.AttachPointName }, r.Sound ? brief(r.Sound, 'Kingdom Key swing set') : { asset: null })));
HT.forEach((r, i) => wc.push(Object.assign({ kind: 'weapon_hit', weapon: WEP, set: strip(wcdo.m_SoundAssets_Hit), index: i, enum_name: HTN[i] || null, bFollow: r.bFollow, AttachPointName: r.AttachPointName }, r.Sound ? brief(r.Sound, 'Kingdom Key hit set') : { asset: null })));
const AL = ['SEAL_NOTHING', 'SEAL_HIT1', 'SEAL_HIT2', 'SEAL_HIT3', 'SEAL_HIT4', 'SEAL_HIT5', 'SEAL_HIT6', 'SEAL_HIT7', 'SEAL_SWING1', 'SEAL_SWING2', 'SEAL_SWING3', 'SEAL_SWING4'];
for (const k in ALIAS) { const m = /\[(\d+)\]$/.exec(k); const i = m ? +m[1] : 0; wc.push(Object.assign({ kind: 'weapon_alias_set', weapon: WEP, set: strip(wcdo.m_SoundAliasAsset), index: i, enum_name: AL[i] || null }, brief(ALIAS[k], 'Kingdom Key alias set'))); }
const wdir = L.AU + 'Sound/SE/weapon/02001/';
for (const f of fs.readdirSync(wdir).filter(f => /^se02001_\d+\.sead\.json$/.test(f))) {
  const a = '/Game/Sound/SE/weapon/02001/' + f.replace('.sead.json', '');
  if (!sounds.has(a)) wc.push(Object.assign({ kind: 'weapon_bank_other', note: 'in the Kingdom Key bank folder, not referenced by the weapon blueprint sets' }, brief(a + '.x', 'weapon/02001 folder')));
}
for (const c of cloth) wc.push(Object.assign({ kind: 'cloth_auto_se', table: c.table, slot: c.slot, slot_name: c.slot_name, motion_sound_type_value: c.slot + 7 }, brief(c.sound + '.x', 'auto-SE ' + c.table.split('/').pop())));
fs.writeFileSync(__dirname + '/weapon_cloth_rows.json', JSON.stringify(wc, null, 1));
for (const f of foot) use(f.sound + '.x', 'footstep ' + f.table.split('/').pop() + ' ' + (f.route === 'aux_surface' ? 'aux:' + f.record : f.slot_name) + ' surface ' + f.surface_type);
// --- sound assets -------------------------------------------------------------------------
const nz = v => v === undefined ? null : v;
const sa = [...sounds.entries()].map(([a, v]) => {
  const r = v.r; const by = [...v.by];
  return { asset: a, exists: r.exists, class: r.class || null, note: r.note || null, reference_bank: strip(r.reference_bank || null), bank_file: r.bank_file || null, bank_file_exists: nz(r.bank_file_exists), own_bank: !!r.own_bank, sound_index: nz(r.sound_index), sound_index_serialized: nz(r.sound_index_serialized), volume: nz(r.volume), duration: nz(r.duration), attenuation: strip(r.attenuation || null), sound_class: strip(r.sound_class || null), props: r.props || null, used_by_count: by.length, used_by: by.slice(0, 12) };
}).sort((a, b) => a.asset < b.asset ? -1 : 1);
fs.writeFileSync(__dirname + '/sound_assets.json', JSON.stringify(sa, null, 1));
// --- summary ------------------------------------------------------------------------------
const cnt = (arr, f) => { const o = {}; for (const x of arr) { const k = f(x); o[k] = (o[k] || 0) + 1; } return o; };
console.log('notify rows', out.length, cnt(out, x => x.notify_class));
console.log('routes', cnt(out, x => x.route)); console.log('not playing by itself', cnt(out.filter(x => !x.plays), x => x.route));
console.log('bp rows', bp.length, 'voice rows', voice.length, cnt(voice, x => x.kind), 'weapon/cloth', wc.length, cnt(wc, x => x.kind));
console.log('sound assets', sa.length, 'missing', sa.filter(s => !s.exists).length, 'bank missing', sa.filter(s => s.exists && !s.bank_file_exists).length, cnt(sa, s => s.asset.split('/').slice(3, 6).join('/')));
const unres = out.filter(r => r.sounds.some(s => !s.exists || !s.bank_file));
console.log('notify rows with unresolved sounds', unres.length, unres.slice(0, 5).map(r => r.anim_asset + ' ' + JSON.stringify(r.sounds.filter(s => !s.exists))));
console.log('en missing', [...new Set(out.flatMap(r => r.sounds).filter(s => s.en && !s.en.exists).map(s => s.en.asset))]);
