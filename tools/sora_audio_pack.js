// Audio part of the Sora asset pack: every sound the player character can trigger, decoded to WAV, with the
// SEAD playback data a host needs to play it as the game does, and the table trigger -> sound.
//
//   node tools/sora_audio_pack.js            build build/sora_pack/audio/, validate, write the sheets
//   node tools/sora_audio_pack.js --check    validate the pack on disk only (nothing written)
//   node tools/sora_audio_pack.js --import   first copy the research passes' outputs from
//                                            build/sora_tmp/ into audit/sora_port/audio/, then build
//   node tools/sora_audio_pack.js --party-extra   (party D5) a SECOND, party-only pack in build/sora_pack/party/audio/:
//                                            the effect-attached SE of the effects in PARTY_EXTRA_EFFECTS (effects the
//                                            port plays for the party that the main pack does not carry); nothing else
//                                            is written (not build/sora_pack/audio, not the sheets). kh-audio
//                                            Pack::merge_party_root merges it (kh-app audio.rs).
//
// Inputs (nothing under assets/ or assets_converted/ is modified):
//   audit/sora_port/audio/*.json        research data: notify_rows, footstep_rows, voice_rows, weapon_cloth_rows
//                                       (animation notifies, auto-SE table, voice groups, weapon sets of p_ex001),
//                                       code_rows (sounds played from native code and blueprints),
//                                       requests_<strand>.json (sound ids other research passes asked for)
//   assets_converted/audio/...          .sab banks and .sead.json sidecars (tools/sead_convert.js)
//   assets_converted/packages/...       decoded packages (object references of the sound assets, attenuation
//                                       and sound class assets)
//   assets/.../WindowsEngine.ini        the SEAD configuration (categories, buses, macros; tools/lib/sead_config.js)
// Outputs:
//   build/sora_pack/audio/wav/<bank>/m<NNN>.wav   one file per material (stream) used, PCM s16, loop in a smpl chunk.
//                                       Every source stream of these sounds is CRI HCA; none is Vorbis, so nothing
//                                       is kept as Ogg (the tool stops if that ever changes).
//   build/sora_pack/audio/manifest.json sounds (playback data per bank + sound index), sound assets (the UE-side
//                                       properties), triggers, configuration; format in docs/sora-audio-spec.md
//   build/sora_pack/audio/sheet_rows.json   rows for sheets/port/sora-assets.json (merged by tools/sora_pack.js)
//   sheets/port/sora-sounds.json        one row per trigger -> sound
// The pack is local build output; it is never distributed. Spec: docs/sora-audio-spec.md.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const sead = require('./lib/sead');
const seadBank = require('./lib/sead_bank');
const seadConfig = require('./lib/sead_config');
const mat = require('./lib/sead_material');
const wav = require('./lib/wav');

const VERSION = '1.1.0';
const ROOT = path.resolve(__dirname, '..');
const P = (...a) => path.join(...a);
const CONTENT = 'KINGDOM HEARTS III/Content/';
const AUDIO = P(ROOT, 'assets_converted', 'audio', CONTENT);
const PACKAGES = P(ROOT, 'assets_converted', 'packages', CONTENT);
const RESEARCH = P(ROOT, 'audit', 'sora_port', 'audio');
// (party D5) --party-extra writes ONLY build/sora_pack/party/audio (the main pack is read by the live Elden Ring build).
// (XSI, 2026-10-16) --xsi-extra: a THIRD root, build/sora_tmp/xsi/pack/xsi/audio (deployed to PACK/xsi/audio): the sounds
// the XS abilities' clips (tools/xsi_pack.js overlay, build/sora_tmp/xsi/pack/xsi_manifest.json) name that the main pack
// lacks. Same rules as --party-extra: nothing else is written; kh-audio Pack::merge_party_root merges it (kh-app audio.rs).
const XSI_EXTRA = process.argv.includes('--xsi-extra');
const PARTY_EXTRA = process.argv.includes('--party-extra') || XSI_EXTRA;
// (DON-MAGIC) `--out <dir>`: the extra pack written elsewhere (a stream's own pack folder, never build/sora_pack behind a DLL)
const OUT_AT = process.argv.indexOf('--out');
const OUT = OUT_AT > 0 ? path.resolve(process.argv[OUT_AT + 1]) : XSI_EXTRA ? P(ROOT, 'build', 'sora_tmp', 'xsi', 'pack', 'xsi', 'audio') : PARTY_EXTRA ? P(ROOT, 'build', 'sora_pack', 'party', 'audio') : P(ROOT, 'build', 'sora_pack', 'audio');
// (party D5) Effects the port spawns for the party whose effect-attached SE the main pack lacks:
//   vm_cur_bds1: Donald's Cure (heal kinds 14 / 15 of ATresProj_MagicCure 0x1414742A0) -> Recover 0x1405B57D0 ->
//   0x1405B8C30(+0x440, 4, kind): kinds 0xE / 0xF -> group 0x58F = 1423 of m_CmnBadStatesEffect (ead_com_abn0
//   EffectDataList[20]: vm_cur_bds1 at vfx_abn_center) on the healed pawn [C, A]; its SeAttach
//   Sound/SE/_Data/SeAttach/NPC/n_ex001/vm_cur_bds1 (se00301_040, FadeOutTime 0.7, bNoSeFadeAtTheEffectEnd) [A].
const PARTY_EXTRA_EFFECTS = ['vm_cur_bds1'];
// (GOOFY-GK) Goofy's Bombardier (now his Y move): the effects the port spawns are vn_ex002_gsh_lnc0 (GShootRise's launch,
// its AttachEffect at 0) and vn_ex002_gsh_sho0 (the explosion, GoofyShoot_Proj); their SeAttach packages carry the short
// names vn_gsh_lnc0 (se00302_008) / vn_gsh_sho0 (se00302_010) [A]. Entries 'EFFECT=SEATTACH' alias the package name.
PARTY_EXTRA_EFFECTS.push('vn_ex002_gsh_lnc0=vn_gsh_lnc0', 'vn_ex002_gsh_sho0=vn_gsh_sho0');
// (DON-MAGIC) Donald's spells (crates/kh-party/src/donald/magic_fx.rs fx): the effects only his spells spawn, with their own
// SeAttach where KH3 has one, else (marked [R] in DON_PLAN 4.3: no silent stand-in) the closest KH3 element SE of the same
// kind. Effects other characters also spawn are NOT aliased here (that would change their sound): magic_fx plays theirs
// through the audio seam ("effect:<SeAttach>"), and vn_pole_thd0 (Donald's own pole thunder) is packed for that use.
PARTY_EXTRA_EFFECTS.push(
  've_ex731_ded_ice00', 've_ex711_tor2', 'vn_ex003_hly_fir2', 'vn_ex003_hly_hit0', 'vn_ex003_hly_cal0', 'vt_grv_bom01', 'vt_grv_bom02', 'vt_grv_bom03', 'vt_grv_hit1', 'vn_pole_thd0',
  've_ex773_flre_sht_fir0=vm_fir_blt3', 've_ex773_flre_bom=vm_fir_zan1', 've_ex773_flre_hit0=vm_fir_blt2', 've_ex773_flre_bom2=vm_fir_zan1',
  'vd_fz451_13_smk_frz_01=vm_blz_cal0', 'vd_fz452_ice_spa_1st=vm_blz_blt1', 'vr_kg06_ms130_c04_lst_holy_bem01=vn_ex003_hly_fir2', 'vr_kg06_ms130_c02_holy_cal0=vn_ex003_hly_cal0',
  've_ex043_fir_zan0=vm_fir_zan1', 've_ex043_fir_zan1=vm_fir_zan1', 've_ex043_fir_zan2=vm_fir_zan1', 've_ex731_ded_ice01=vm_blz_blt3',
  'vd_ew751_wave_set=vm_wat_bar3', 'vd_ew751_wave_head=vm_wat_han060', 'vm_fns_sky0=vm_fns_fir0', 'vm_lgt_2=vm_cur_hed0', 'vm_fns_star=vm_fns_bom0', 'vm_fns_inseki=vm_fns_bom0',
  'vd_fz453_19_ice_wal00=vm_blz_blt3', 'vd_fz453_19_ice_wal01=vm_blz_blt3', 'vr_he_ms100s_whiteout00=vm_air_han_140', 've_ex711_tor_spk0=vm_thd_str01',
  'vm_wnd_kze00=vm_air_blt2', 'vm_wnd_str00=vm_air_blt3', 'vd_kg878_c36_ult_dom00=vt_gm_ult_hit01', 'vd_kg878_c36_ult_blt00=vt_gm_ult_hit01', 'vd_kg878_c34_ult_cal00=vm_fir_zan1', 'vd_kg878_c38_ult_cal00=vt_gm_ult_hit01',
);
const SHEET = P(ROOT, 'sheets', 'port', 'sora-sounds.json');
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const rel = (f) => path.relative(ROOT, f).split(path.sep).join('/');
const round = (x, n = 6) => (typeof x === 'number' ? +x.toFixed(n) : x);
const checkOnly = process.argv.includes('--check');

// ------------------------------------------------------------------ research data
// Where each research file comes from (--import). The strand folders are scratch space; the copies under
// audit/sora_port/audio are what this tool builds from.
const IMPORTS = [
  ['notify_rows.json', 'build/sora_tmp/audio/triggers/notify_rows.json'],
  ['footstep_rows.json', 'build/sora_tmp/audio/triggers/footstep_rows.json'],
  ['voice_rows.json', 'build/sora_tmp/audio/triggers/voice_rows.json'],
  ['weapon_cloth_rows.json', 'build/sora_tmp/audio/triggers/weapon_cloth_rows.json'],
  ['code_rows.json', 'build/sora_tmp/audio/code/code_rows.json'],
  ['code_helpers.json', 'build/sora_tmp/audio/code/helpers.json'],
  ['code_tables.json', 'build/sora_tmp/audio/code/tables.json'],
  ['plugin_findings.json', 'build/sora_tmp/audio/plugin/FINDINGS.json'],
  ['bank_findings.json', 'build/sora_tmp/audio/bank/FINDINGS.json'],
  ['requests_actions.json', 'build/sora_tmp/actions/asset_requests.json'],
  ['requests_combat.json', 'build/sora_tmp/combat/asset_requests.json'],
  ['requests_magic.json', 'build/sora_tmp/magic/asset_requests.json'],
  ['requests_ui.json', 'build/sora_tmp/ui/asset_requests.json'],
];
if (process.argv.includes('--import')) {
  fs.mkdirSync(RESEARCH, { recursive: true });
  for (const [name, src] of IMPORTS) {
    const s = P(ROOT, src);
    if (!fs.existsSync(s)) { console.log(`import: ${src} absent`); continue; }
    fs.copyFileSync(s, P(RESEARCH, name));
    console.log(`import: ${src} -> ${rel(P(RESEARCH, name))}`);
  }
}
const research = (name, dflt) => (fs.existsSync(P(RESEARCH, name)) ? readJson(P(RESEARCH, name)) : dflt);
// (party E) Donald's and Goofy's rows (tools/party_audio_pack.js -> audit/sora_port/audio/party_rows.json): packed into
// this same pack (one SEAD driver and one set of category pools for everybody, as in the game). Absent file = Sora only.
const PARTY = (() => {
  const j = research('party_rows.json', null);
  const all = (k) => (j ? Object.entries(j.members).flatMap(([owner, m]) => (m[k] || []).map((r) => Object.assign({ owner }, r))) : []);
  return { notify_rows: all('notify_rows'), voice_rows: all('voice_rows'), footstep_rows: all('footstep_rows'), cloth_rows: all('cloth_rows'), weapon_rows: all('weapon_rows'), code_rows: j ? j.code_rows || [] : [] };
})();

// ------------------------------------------------------------------ sound asset -> bank + index
const pkgCache = new Map();
function loadPackage(relPath) {
  if (pkgCache.has(relPath)) return pkgCache.get(relPath);
  const f = P(PACKAGES, relPath + '.uasset.json');
  const d = fs.existsSync(f) ? readJson(f) : null;
  if (pkgCache.size > 200) pkgCache.clear();
  pkgCache.set(relPath, d);
  return d;
}
function objectPath(pkg, i) {
  if (!i || !pkg) return null;
  if (i > 0) return 'export:' + pkg.exports[i - 1].objectName;
  let im = pkg.imports[-i - 1], s = im.objectName;
  while (im.outerIndex) { im = pkg.imports[-im.outerIndex - 1]; s = im.objectName + '.' + s; }
  return s;
}
const gameRel = (p) => { let s = String(p).replace(/^\/Game\//, ''); const i = s.lastIndexOf('.'); if (i > s.lastIndexOf('/')) s = s.slice(0, i); return s; };
function simplifySidecar(v, pkg) {
  if (v === null || typeof v !== 'object') return v;
  if (Array.isArray(v)) return v.map((x) => simplifySidecar(x, pkg));
  if (v.props) { const r = {}; for (const p of v.props) r[p.tag.name] = p.tag.type === 'ObjectProperty' ? objectPath(pkg, p.value && p.value.object) : p.tag.type === 'BoolProperty' ? !!p.tag.boolVal : simplifySidecar(p.value, pkg); return r; }
  if ('object' in v && Object.keys(v).length === 1) return objectPath(pkg, v.object);
  if (v.items) return v.items.map((x) => simplifySidecar(x, pkg));
  const r = {}; for (const k of Object.keys(v)) r[k] = simplifySidecar(v[k], pkg); return r;
}
function simplifyProps(list, pkg) { // decoded-package form: [{name,type,value}]
  const one = (e) => {
    if (e.type === 'ObjectProperty') return objectPath(pkg, e.value);
    if (e.type === 'StructProperty' && Array.isArray(e.value) && e.value.every((x) => x && typeof x === 'object' && 'name' in x && 'type' in x)) return simplifyProps(e.value, pkg);
    if (e.type === 'ArrayProperty' && Array.isArray(e.value)) return e.value.map((x) => (Array.isArray(x) ? simplifyProps(x, pkg) : e.innerType === 'ObjectProperty' ? objectPath(pkg, x) : x));
    return e.value;
  };
  const r = {};
  for (const e of list || []) r[e.name + (e.arrayIndex ? `[${e.arrayIndex}]` : '')] = one(e);
  return r;
}
const assetCache = new Map();
// '/Game/Sound/.../x' -> { asset, bank (path under Content/, no extension), ext, index, props... } or { asset, missing }
function resolveAsset(gamePath) {
  const r0 = gameRel(gamePath), key = r0.toLowerCase();
  if (assetCache.has(key)) return assetCache.get(key);
  const out = { asset: '/Game/' + r0 };
  assetCache.set(key, out);
  const sf = P(AUDIO, r0 + '.sead.json');
  if (!fs.existsSync(sf)) { out.missing = fs.existsSync(P(PACKAGES, r0 + '.uasset.json')) ? 'package without a SEAD export' : 'no such package'; return out; }
  const sc = readJson(sf), pkg = loadPackage(r0);
  if (sc.class !== 'SQEXSEADSound' && sc.class !== 'SQEXSEADMusic') { out.missing = 'class ' + sc.class; return out; }
  const props = simplifySidecar({ props: sc.props }, pkg);
  out.class = sc.class; out.package = sc.package;
  if (props.ReferenceBank) { out.bank = gameRel('/Game/' + String(props.ReferenceBank).replace(/^\/Game\//, '')); out.index = props.SoundIndex || 0; }
  else if (sc.bank) { out.bank = r0; out.index = props.SoundIndex || 0; }
  else { out.missing = 'neither an embedded bank nor a ReferenceBank'; return out; }
  out.ext = fs.existsSync(P(AUDIO, out.bank + '.sab')) ? 'sab' : fs.existsSync(P(AUDIO, out.bank + '.mab')) ? 'mab' : null;
  if (!out.ext) { out.missing = 'bank file not found: ' + out.bank; return out; }
  out.volume = 'Volume' in props ? props.Volume : null; // unset: the class default applies (docs/sora-audio-spec.md)
  out.duration = props.Duration === undefined ? null : props.Duration;
  out.looping = props.Duration === 10000; // the plugin's marker for a looping sound
  out.attenuation = props.AttenuationSettings || null;
  out.sound_class = props.SoundClassObject || null;
  out.props = props;
  return out;
}
const enVariant = (p) => String(p).replace('/Localization/ja/', '/Localization/en/');

// ------------------------------------------------------------------ banks
const bankCache = new Map();
function loadBank(bankRel, ext) {
  if (bankCache.has(bankRel)) return bankCache.get(bankRel);
  const file = P(AUDIO, bankRel + '.' + ext);
  const data = fs.readFileSync(file);
  const b = { rel: bankRel, file, data, parsed: seadBank.parseBank(data), survey: sead.surveyBank(data), sha256: sha256(data), usedMaterials: new Set(), sounds: new Map() };
  if (b.parsed.problems && b.parsed.problems.length) throw new Error(`bank ${bankRel}: ${b.parsed.problems.join('; ')}`);
  bankCache.set(bankRel, b);
  return b;
}
const bankId = (bankRel) => bankRel.replace(/^Sound\//, '');
const STRIP = new Set(['raw', 'offset', 'at', 'size', 'headerSize', 'version', 'byte1']);
const strip = (o) => { if (o === null || typeof o !== 'object') return round(o); if (Array.isArray(o)) return o.map(strip); const r = {}; for (const k of Object.keys(o)) if (!STRIP.has(k)) r[k] = strip(o[k]); return r; };

// The playback data of sound `index`: header fields, selection rule, sequences with their commands, the tracks
// they key on (with commands) and the materials those play. Child sounds (SoundTrack) are pulled in too.
function soundEntry(b, index, cfg, depth = 0) {
  if (b.sounds.has(index)) return b.sounds.get(index);
  const s = b.parsed.sounds[index];
  if (!s || !s.type) { b.sounds.set(index, null); return null; }
  const res = seadBank.resolveSound(b.parsed, index);
  const cat = cfg.categories.find((c) => c.number === s.category);
  const pool = cat && cat.sharesPoolOf ? cfg.categories.find((c) => c.number === cat.sharesPoolOf) : cat;
  const bus = (n) => { const x = cfg.buses.find((q) => q.number === n); return { number: n, name: x ? x.name : null }; };
  const e = {
    index, number: s.number, name: s.name, type: s.typeName, format_version: s.version,
    category: { number: s.category, name: cat ? cat.name : null, pool: pool ? pool.name : null, pool_size: pool ? pool.maxSounds : null },
    priority: s.priority, volume: round(s.volume), duration: round(s.duration),
    main_bus: bus(s.mainBus), aux_sends: s.auxSends.slice(0, s.auxSendCount).map((a) => ({ bus: bus(a.bus), volume: round(a.volume) })),
    flags: s.flags, flag_names: [[0x01, 'no_3d_pan'], [0x02, 'no_3d_volume'], [0x04, 'bit2'], [0x08, 'centre_only_2d'], [0x20, 'random_may_repeat'], [0x80, 'stop_breaks_loops']].filter(([m]) => s.flags & m).map(([, n]) => n),
    macro_start: s.macroStart, macro_stop: s.macro1, macro_event3: s.macro3,
    pan_type: s.panType, pan_param: round(s.panParam), flags2: s.byte51, release_seconds: round(Buffer.from(s.unk58 || '0000000000000000', 'hex').readFloatLE(0)),
    cycle: s.type === 4 ? { interval_min_ms: s.cycleIntervalMinMs, interval_max_ms: s.cycleIntervalMaxMs, max_active: s.cycleMaxActive } : undefined,
    selection: strip(res.selection), sequences: [], tracks: {}, child_sounds: [],
  };
  b.sounds.set(index, e);
  for (const si of s.sequences) {
    const q = b.parsed.sequences[si.sequence];
    const seq = { sequence: si.sequence, number: q.number, weight: q.weight, group: q.group, switch_min: round(q.switchMin), switch_max: round(q.switchMax), zero_ones: strip(q.zeroOnes).map((z) => ({ name: z.name, target: z.targetName, target_index: z.targetIndex, in_min: z.inMin, in_max: z.inMax, flags: z.flags, param_id: z.paramId, points: z.points })), commands: strip(q.commands) };
    for (const c of q.commands) {
      if (c.name !== 'keyOn') continue;
      const t = b.parsed.tracks[c.track];
      if (!t) throw new Error(`bank ${b.rel} sound ${index}: keyOn of a missing track ${c.track}`);
      const tr = { type: t.typeName, commands: strip(t.commands) };
      if (t.type === 1) {
        const m = b.survey.materials[t.ref];
        if (!m || m.codec === 0) tr.material = null; // empty slot: silent
        else { tr.material = t.ref; b.usedMaterials.add(t.ref); }
      } else if (t.type === 2) {
        tr.child_sound = t.ref;
        if (depth < 4) { soundEntry(b, t.ref, cfg, depth + 1); if (!e.child_sounds.includes(t.ref)) e.child_sounds.push(t.ref); }
      }
      e.tracks[c.track] = tr;
    }
    e.sequences.push(seq);
  }
  return e;
}

// ------------------------------------------------------------------ triggers
const NOTIFY_FN = { // class -> [function a host hooks (Notify / NotifyBegin), verdict in sheets/impl/progress as the research pass read it]
  TresAnimNotify_PlayVoice: '0x140560960', TresAnimNotifyState_PlayVoice: '0x140563E80', TresAnimNotify_PlayActorVoice: '0x1405604A0',
  TresAnimNotify_PlayWeaponSwingSE: '0x1405615F0', TresAnimNotifyState_PlayWeaponSwingSE: '0x140564040', TresAnimNotify_PlayWeaponHitSE: '0x1405612A0',
  TresAnimNotifyState_Attack: '0x140561F40', SQEXSEADAnimNotify_PlaySound: '0x145815C90', SQEXSEADAnimNotifyState_AttachSound: '0x145815F40',
  SQEXSEADAnimNotify_PlayAutoSe: '0x145815C10', SQEXSEADAnimNotifyState_PlayAutoSe: '0x145816230', SQEXSEADAnimNotifyState_ChangeAutoSeEnable: '0x145816110',
};
function buildTriggers() {
  const T = [];
  const add = (t) => { T.push(t); return t; };
  const snd = (asset, how, extra = {}) => ({ asset: asset ? '/Game/' + gameRel(asset) : null, how, ...extra });
  const clipOf = (r) => gameRel(r.anim_asset).split('/').pop();
  const logical = (r) => (r.logical || []).map((x) => String(x).split(' ')[0]);
  // -- animation notifies
  const perClip = new Map();
  // (party E) the party's notify rows (tools/party_audio_pack.js) go through the same rules; their triggers carry
  // `owner` (n_ex001 / n_ex002) and ids prefixed party.<owner>.
  for (const r of [...research('notify_rows.json', []), ...PARTY.notify_rows]) {
    const clip = clipOf(r), cls = r.notify_class;
    const k = (r.owner || '') + clip + '|' + cls; const n = perClip.get(k) || 0; perClip.set(k, n + 1);
    const pe = r.props_effective || r.props_serialized || {};
    const ps = pe.PlaySettings || {};
    const t = {
      id: `${r.owner ? 'party.' + r.owner + '.' : ''}notify.${gameRel(r.anim_asset).includes('/localization/') ? 'loc.' : ''}${clip}.${cls.replace(/^TresAnimNotify(State)?_|^SQEXSEADAnimNotify(State)?_/, (m) => (/State/.test(m) ? 'S' : ''))}.${n}`, owner: r.owner || undefined,
      route: 'notify.' + r.route, trigger: `${cls} in ${gameRel(r.anim_asset)}`, clip, logical: logical(r), notify_class: cls,
      time: round(r.time_s, 4), duration: r.is_state ? round(r.duration_s, 4) : null, anim_length: round(r.sequence_length, 4),
      plays: !!r.plays, sounds: [], spatial: '3D', attach: null, follow: null, volume: null, pitch: null, condition: null,
      evidence: 'asset', ref_addr: NOTIFY_FN[cls] || null, note: r.note || null, params: pe,
    };
    if (/Voice/.test(cls)) { t.attach = pe.m_SocketName && pe.m_SocketName !== 'None' ? pe.m_SocketName : null; t.follow = pe.m_bFollow ?? null; t.volume = pe.m_VolumeMultiplier ?? null; t.pitch = pe.m_PitchMultiplier ?? null; }
    else if (/Weapon(Swing|Hit)SE/.test(cls)) { t.attach = pe.m_SocketName && pe.m_SocketName !== 'None' ? pe.m_SocketName : null; t.follow = pe.m_bFollow ?? null; t.volume = pe.m_VolumeMultiplier ?? null; t.pitch = pe.m_PitchMultiplier ?? null; }
    else if (/PlaySound|AttachSound/.test(cls)) { t.attach = pe.AttachPointName && pe.AttachPointName !== 'None' ? pe.AttachPointName : null; t.follow = pe.bFollow ?? null; t.volume = pe.VolumeMultiplier ?? null; t.pitch = pe.PitchMultiplier ?? null; }
    else if (/PlayAutoSe/.test(cls)) { t.attach = `auto-SE part ${String(ps.AutoSePartsType || '').split('::').pop()}[${ps.AutoSePartsIndex ?? 0}]`; t.follow = ps.bAttachToParts ?? null; t.volume = ps.SoundVolume ?? null; t.pitch = ps.SoundPitchMultiplier ?? null; }
    if (r.route === 'voice_random') { const ws = pe.m_RandomVoiceAssets || []; for (const s of r.sounds) { const w = ws.find((x) => gameRel(x.Voice) === gameRel(s.asset)); t.sounds.push(snd(s.asset, 'weighted random, no immediate repeat', { weight: w ? w.RandomWeights : null })); } }
    else if (r.route === 'voice_group') for (const s of r.sounds) t.sounds.push(snd(s.asset, `voice group ${pe.m_VoiceGroupName}: the unit whose number equals the pawn's voice group number`, { group: pe.m_VoiceGroupName, group_no: s.group_no }));
    else if (r.route === 'auto_se' && r.auto_se) { t.auto_se = { source: r.auto_se.source, motion_sound_type: r.auto_se.motion_sound_type || null, record: r.auto_se.record || r.auto_se.record_name || null, lookup: r.auto_se.lookup }; t.sound_ref = r.auto_se.motion_sound_type ? `autose.<surface>.${r.auto_se.motion_sound_type}` : `autose.<surface>.aux.${t.auto_se.record}`; }
    else for (const s of r.sounds || []) t.sounds.push(snd(s.asset, s.role || 'direct', r.alias_index !== undefined ? { alias_index: r.alias_index } : {}));
    if (r.route === 'attack_hit_se') t.condition = 'when the attack window connects';
    add(t);
  }
  // -- auto-SE table of the default costume
  const SLOT_TYPES = ['Walk', 'Run', 'Jump', 'LandNormal', 'LandHard', 'FootShuffle'];
  for (const r of research('footstep_rows.json', [])) {
    if (r.table !== '/Game/Sound/SE/_Data/MASTS/FsTable_Sora') continue;
    const aux = r.route !== 'per_surface';
    if (!r.sound) continue;
    add({
      id: aux ? `autose.${r.surface_type}.aux.${r.record}` : `autose.${r.surface_type}.${r.slot_name || SLOT_TYPES[r.slot]}`,
      route: aux ? 'autose.aux_surface' : 'autose.per_surface', trigger: aux ? `auto-SE record "${r.record}" on surface ${r.surface_type} (${r.surface_name})` : `auto-SE motion ${r.slot_name} on surface ${r.surface_type} (${r.surface_name})`,
      clip: null, logical: [], notify_class: null, time: null, duration: null, plays: true, sounds: [snd(r.sound, 'table entry')], spatial: '3D', attach: 'auto-SE part (foot)', follow: null, volume: null, pitch: null,
      condition: `physical surface type ${r.surface_type}`, evidence: 'asset', ref_addr: '0x145811C50', note: null, surface: r.surface_type, surface_name: r.surface_name, slot: aux ? r.record : r.slot_name,
    });
  }
  for (const r of research('weapon_cloth_rows.json', [])) {
    if (!r.asset) continue;
    if (r.kind === 'cloth_auto_se') add({ id: `autose.cloth.${r.slot_name}`, route: 'autose.non_surface', trigger: `auto-SE motion ${r.slot_name} (cloth rustle, detector driven)`, clip: null, logical: [], notify_class: null, time: null, duration: null, plays: true, sounds: [snd(r.asset, 'table entry')], spatial: '3D', attach: 'auto-SE part', follow: null, volume: null, pitch: null, condition: null, evidence: 'asset', ref_addr: '0x145811C50', note: null, slot: r.slot_name });
    else if (r.kind === 'weapon_swing' || r.kind === 'weapon_hit') add({ id: `weapon.kingdom_key.${r.kind === 'weapon_swing' ? 'swing' : 'hit'}.${r.index}`, route: 'weapon.' + (r.kind === 'weapon_swing' ? 'swing_set' : 'hit_set'), trigger: `${r.set} [${r.index}] ${r.enum_name}`, clip: null, logical: [], notify_class: null, time: null, duration: null, plays: true, sounds: [snd(r.asset, 'set entry', { alias_index: r.index })], spatial: '3D', attach: null, follow: null, volume: null, pitch: null, condition: 'weapon = Kingdom Key (w_so010_Wep)', evidence: 'asset', ref_addr: r.kind === 'weapon_swing' ? '0x14178BED0' : '0x14178BEB0', note: null });
    else add({ id: `weapon.kingdom_key.${r.kind.replace('weapon_', '')}.${r.index ?? gameRel(r.asset).split('/').pop()}`, route: 'weapon.' + r.kind.replace('weapon_', ''), trigger: r.set ? `${r.set} [${r.index}] ${r.enum_name}` : `sound of the Kingdom Key bank folder (${r.note || 'no reference found'})`, clip: null, logical: [], notify_class: null, time: null, duration: null, plays: r.kind !== 'weapon_bank_other', sounds: [snd(r.asset, 'set entry')], spatial: '3D', attach: null, follow: null, volume: null, pitch: null, condition: null, evidence: 'asset', ref_addr: null, note: r.note || null });
  }
  // -- voice groups
  for (const r of research('voice_rows.json', [])) {
    if (r.kind !== 'voice_group') continue;
    add({ id: `voice.group.${r.group}.${r.group_no}`, route: 'voice.group', trigger: `voice group "${r.group}" unit ${r.group_no} of ${r.table}`, clip: null, logical: [], notify_class: null, time: null, duration: null, plays: true, sounds: [snd(r.asset, 'group unit', { group: r.group, group_no: r.group_no })], spatial: '3D', attach: 'atama', follow: true, volume: null, pitch: null, condition: `pawn voice group number = ${r.group_no}`, evidence: 'asset', ref_addr: '0x14150DC50', note: r.play_on_group_not_found ? 'plays the default voice when no unit matches' : null });
  }
  // -- (party E) the party's tables: FsTable_Donald / _Goofy, the 14 + 14 weapons' sets, n_ex00x_VoiceGroupSet
  for (const r of PARTY.footstep_rows) {
    if (!r.sound) continue;
    const aux = r.route !== 'per_surface';
    add({ id: `party.${r.owner}.` + (aux ? `autose.${r.surface_type}.aux.${r.record}` : `autose.${r.surface_type}.${r.slot_name}`), owner: r.owner, route: aux ? 'autose.aux_surface' : 'autose.per_surface', trigger: `${r.table.split('/').pop()}: ` + (aux ? `auto-SE record "${r.record}" on surface ${r.surface_type} (${r.surface_name})` : `auto-SE motion ${r.slot_name} on surface ${r.surface_type} (${r.surface_name})`),
      clip: null, logical: [], notify_class: null, time: null, duration: null, plays: true, sounds: [snd(r.sound, 'table entry')], spatial: '3D', attach: 'auto-SE part (foot)', follow: null, volume: null, pitch: null, condition: `physical surface type ${r.surface_type}`, evidence: 'asset', ref_addr: '0x145811C50', note: null, surface: r.surface_type, surface_name: r.surface_name, slot: aux ? r.record : r.slot_name });
  }
  for (const r of PARTY.cloth_rows) if (r.sound) add({ id: `party.${r.owner}.autose.cloth.${r.slot_name}`, owner: r.owner, route: 'autose.non_surface', trigger: `${r.table.split('/').pop()}: auto-SE motion ${r.slot_name}`, clip: null, logical: [], notify_class: null, time: null, duration: null, plays: true, sounds: [snd(r.sound, 'table entry')], spatial: '3D', attach: 'auto-SE part', follow: null, volume: null, pitch: null, condition: null, evidence: 'asset', ref_addr: '0x145811C50', note: null, slot: r.slot_name });
  for (const r of PARTY.weapon_rows) {
    if (!r.asset) continue;
    const sw = r.kind === 'weapon_swing';
    add({ id: `party.${r.owner}.weapon.${r.weapon}.${sw ? 'swing' : 'hit'}.${r.index}`, owner: r.owner, weapon: r.weapon, route: sw ? 'weapon.swing_set' : 'weapon.hit_set', trigger: `${r.set} [${r.index}] ${r.enum_name}`, clip: null, logical: [], notify_class: null, time: null, duration: null, plays: true, sounds: [snd(r.asset, 'set entry', { alias_index: r.index })], spatial: '3D', attach: r.AttachPointName && r.AttachPointName !== 'None' ? r.AttachPointName : null, follow: r.bFollow ?? null, volume: null, pitch: null, condition: `weapon = ${r.weapon}`, evidence: 'asset', ref_addr: sw ? '0x14178BED0' : '0x14178BEB0', note: null });
  }
  for (const r of PARTY.voice_rows) {
    if (r.kind !== 'voice_group' || !r.asset) continue;
    add({ id: `party.${r.owner}.voice.group.${r.group}.${r.group_no}`, owner: r.owner, route: 'voice.group', trigger: `voice group "${r.group}" unit ${r.group_no} of ${r.table}`, clip: null, logical: [], notify_class: null, time: null, duration: null, plays: true, sounds: [snd(r.asset, 'group unit', { group: r.group, group_no: r.group_no })], spatial: '3D', attach: 'atama', follow: true, volume: null, pitch: null, condition: `unit number ${r.group_no} (PlayVoiceGroup FUN_141275AA0, or the pawn's voice group number for a notify)`, evidence: 'asset', ref_addr: '0x141275AA0', note: r.play_on_group_not_found ? 'plays the default voice when no unit matches' : null });
  }
  // -- native code and blueprints
  const seen = new Map();
  for (const r of [...research('code_rows.json', []), ...PARTY.code_rows]) {
    const base = `code.${r.category}.${String(r.sound_id || r.trigger || '').replace(/[^A-Za-z0-9]+/g, '_').replace(/^_|_$/g, '').slice(0, 48)}`;
    const n = seen.get(base) || 0; seen.set(base, n + 1);
    const addr = /0x1[0-9A-Fa-f]{8}/.exec(String(r.ref || ''));
    add({
      id: n ? `${base}.${n}` : base, route: 'code.' + r.category, trigger: r.trigger, clip: null, logical: [], notify_class: null, time: null, duration: null, plays: true,
      sounds: r.sound_asset ? [snd(r.sound_asset, r.sound_id || 'direct')] : [], sound_id: r.sound_id || null, spatial: r.spatial || null, attach: r.attach || null, follow: null,
      volume: typeof r.volume === 'number' ? r.volume : null, pitch: typeof r.pitch === 'number' ? r.pitch : null, volume_text: typeof r.volume === 'number' ? null : r.volume || null,
      condition: r.condition || null, loop_stop: r.loop_stop || null, evidence: r.evidence || null, verdict: r.verdict || null, ref: r.ref || null, ref_addr: addr ? '0x' + addr[0].slice(2).toUpperCase() : null, note: r.note || null,
    });
  }
  return T;
}

// (party D5) --party-extra: the effect-attached SE rows of PARTY_EXTRA_EFFECTS, read from every SeAttach package of
// that effect name (the lookup is by name, 0x14425F080), in the row shape of tools/party_audio_pack.js's seRows.
function partyExtraTriggers() {
  const T = [];
  const index = new Map();
  // (Windows' folder order on every system: tools/lib/ntfs_order.js)
  const { ntfsCompare } = require('./lib/ntfs_order');
  (function walk(dir) { for (const f of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => ntfsCompare(a.name, b.name))) { const p = P(dir, f.name); if (f.isDirectory()) walk(p); else if (f.name.endsWith('.uasset.json')) { const r = path.relative(PACKAGES, p).split(path.sep).join('/').replace(/\.uasset\.json$/, ''); const k = r.split('/').pop().toLowerCase(); if (!index.has(k)) index.set(k, []); index.get(k).push(r); } } })(P(PACKAGES, 'Sound', 'SE', '_Data', 'SeAttach'));
  for (const entry of PARTY_EXTRA_EFFECTS) {
    const [name, seName] = entry.includes('=') ? entry.split('=') : [entry, entry]; // (GOOFY-GK) an aliased SeAttach name
    for (const r of index.get(seName.toLowerCase()) || []) {
      const pkg = loadPackage(r); if (!pkg) continue;
      pkg.exports.forEach((e, i) => {
        if (e.class !== 'SQEX_EffectSeAttachData') return;
        const pr = simplifyProps((e.data && e.data.props) || [], pkg);
        const a = pr.Sound ? '/Game/' + gameRel(pr.Sound) : null;
        T.push({
          id: `party.d5.code.effect_se_npc.${name}.${i}`, route: 'code.effect_se_npc', trigger: 'Effect ' + name + (pr.bPlayAtEnd ? ' ends' : ' starts'), clip: null, logical: [], notify_class: null, time: null, duration: null, plays: !!a,
          sounds: a ? [{ asset: a, how: 'SQEX_EffectSeAttachData.Sound' }] : [], sound_id: 'SQEX_EffectSeAttachData.Sound', spatial: '3D', attach: 'attached to the particle component (bAttach default true)', follow: null,
          volume: null, pitch: null, volume_text: 'asset default', condition: 'GroupID ' + (pr.GroupID ?? 0), loop_stop: 'FadeOutTime ' + (pr.FadeOutTime ?? null) + (pr.bNoSeFadeAtTheEffectEnd ? '; bNoSeFadeAtTheEffectEnd' : '') + (pr.bPlayAtEnd ? '; bPlayAtEnd' : ''),
          evidence: 'asset', verdict: 'same-partial', ref: '/Game/' + r + ' :: ' + e.objectName + ' (played by 0x14425F080)', ref_addr: '0x14425F080', note: 'party D5: an effect the port spawns for the party (see PARTY_EXTRA_EFFECTS)',
        });
      });
    }
  }
  return T;
}

// (XSI) --xsi-extra: one row per sound a notify of an XS overlay clip names that the main pack's manifest lacks.
function xsiExtraTriggers() {
  const T = [];
  const overlay = readJson(P(ROOT, 'build', 'sora_tmp', 'xsi', 'pack', 'xsi_manifest.json'));
  const main = readJson(P(ROOT, 'build', 'sora_pack', 'audio', 'manifest.json'));
  const have = new Set(Object.keys(main.sound_assets || {}).map((k) => k.toLowerCase()));
  for (const it of overlay.items) {
    const clip = path.basename(it.file, '.glb');
    (it.notifies || []).forEach((n, i) => {
      const found = [...new Set((JSON.stringify(n).match(/\/Game\/Sound\/[A-Za-z0-9_/]+/g) || []).map((s) => s.replace(/\.[A-Za-z0-9_]+$/, '')))];
      const missing = found.filter((a) => !have.has(a.toLowerCase()) && !a.includes('/Localization/'));
      if (!missing.length) return;
      T.push({
        id: `xsi.notify.${clip}.${i}`, route: 'notify.xsi_overlay', trigger: `${n.class} in ${clip}`, clip, logical: [], notify_class: n.class || null, time: n.time ?? null, duration: n.duration ?? null, plays: true,
        sounds: missing.map((asset) => ({ asset, how: 'named by the notify' })), sound_id: 'notify', spatial: '3D', attach: null, follow: null,
        volume: null, pitch: null, volume_text: 'asset default', condition: '-', loop_stop: '-',
        evidence: 'asset', verdict: 'same-partial', ref: it.source_asset + ' notify ' + i, ref_addr: '-', note: 'XSI: a sound of an XS clip the main pack does not carry',
      });
    });
  }
  return T;
}

// Sound ids other research passes asked for, as asset paths where they gave one.
function requestedAssets() {
  const out = [];
  const push = (strand, p, by) => { if (p && /^\/Game\/Sound\/|Content\/Sound\//.test(p)) out.push({ strand, asset: '/Game/' + gameRel(String(p).replace(/^.*Content\//, '/Game/').replace(/\.uasset$/, '')), by }); };
  const walk = (strand, v, by) => {
    if (typeof v === 'string') push(strand, v, by);
    else if (Array.isArray(v)) v.forEach((x) => walk(strand, x, by));
    else if (v && typeof v === 'object') for (const k of Object.keys(v)) walk(strand, v[k], v.name || v.id || v.referenced_by || by);
  };
  for (const strand of ['actions', 'combat', 'magic', 'ui']) { const j = research(`requests_${strand}.json`, null); if (j) walk(strand, j, null); }
  return out;
}

// ------------------------------------------------------------------ build
function build() {
  const cfg = seadConfig.parseConfig(seadConfig.extract());
  const triggers = XSI_EXTRA ? xsiExtraTriggers() : PARTY_EXTRA ? partyExtraTriggers() : buildTriggers();
  // (ZETTA) KH_AUDIO_EXTRA_SOUNDS=<file.json> with --party-extra: [{ asset: "/Game/Sound/...", trigger, note }] sound assets a
  // party module plays BY PATH through the audio seam (Zettaflare's cutscene SE stream and Donald's event voice lines); one
  // trigger row each (route code.party_by_path). Without the variable nothing changes.
  if (PARTY_EXTRA && process.env.KH_AUDIO_EXTRA_SOUNDS) {
    for (const r of readJson(path.resolve(process.env.KH_AUDIO_EXTRA_SOUNDS))) {
      triggers.push({ id: 'party.by_path.' + r.asset.split('/').pop(), route: 'code.party_by_path', trigger: r.trigger || 'played by path', clip: null, logical: [], notify_class: null, time: null, duration: null, plays: true,
        sounds: [{ asset: r.asset, how: 'SoundPlay name = the asset path' }], sound_id: 'by path', spatial: r.spatial || '2D', attach: null, follow: null, volume: null, pitch: null, volume_text: 'asset default',
        condition: '-', loop_stop: '-', evidence: 'asset', verdict: 'same-partial', ref: r.ref || '-', ref_addr: '-', note: r.note || '(ZETTA) a sound a party module plays by path' });
    }
  }
  const requests = PARTY_EXTRA ? [] : requestedAssets();
  // every asset path in play, with the English twin of each Japanese voice
  const want = new Map();
  const need = (p, why) => { if (!p) return; for (const q of p.includes('/Localization/ja/') ? [p, enVariant(p)] : [p]) { const k = '/Game/' + gameRel(q); if (!want.has(k)) want.set(k, new Set()); want.get(k).add(why); } };
  for (const t of triggers) for (const s of t.sounds) need(s.asset, t.route);
  for (const r of requests) need(r.asset, 'request.' + r.strand);
  // 1.1.0: every row of the static SE table (ETresStaticSE -> TresStaticSESet.m_Assets): 8 of its 43 ids are
  // never passed as constants by native code, so no trigger row names them, but the HUD plays them by id.
  if (!PARTY_EXTRA) for (const r of (research('code_tables.json', {}).static_se || { rows: [] }).rows) need(r.asset, 'static_se_table');

  const assets = {}, unresolved = [];
  for (const [p, why] of [...want].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
    const a = resolveAsset(p);
    if (a.missing) { unresolved.push({ asset: p, reason: a.missing, wanted_by: [...why].sort() }); continue; }
    const b = loadBank(a.bank, a.ext);
    const e = soundEntry(b, a.index, cfg);
    assets[p] = {
      bank: bankId(a.bank), sound_index: a.index, empty_sound: !e, volume: round(a.volume), duration: round(a.duration), looping: a.looping,
      attenuation: a.attenuation, sound_class: a.sound_class, concurrency: a.props.bOverrideConcurrency ? a.props.ConcurrencyOverrides || {} : null,
      occlusion_trace_channel: a.props.SEADOcclusionTraceChannel || null,
      other_props: Object.fromEntries(Object.entries(a.props).filter(([k]) => !['ReferenceBank', 'SoundIndex', 'Volume', 'Duration', 'AttenuationSettings', 'SoundClassObject', 'bOverrideConcurrency', 'ConcurrencyOverrides', 'SEADOcclusionTraceChannel'].includes(k))),
      source_package: a.package, wanted_by: [...why].sort(),
    };
  }
  // referenced UE assets (attenuation, sound class): their properties as stored
  const ueAssets = {};
  for (const a of Object.values(assets)) for (const ref of [a.attenuation, a.sound_class]) {
    if (!ref || ueAssets[ref]) continue;
    const pkg = loadPackage(gameRel(ref));
    ueAssets[ref] = pkg ? { class: pkg.exports[0].class, props: simplifyProps((pkg.exports[0].data || {}).props, pkg) } : { missing: true };
  }

  // decode the used materials
  const files = new Map(), banks = {}, materialRows = [];
  let seconds = 0;
  for (const b of [...bankCache.values()].sort((x, y) => (x.rel < y.rel ? -1 : 1))) {
    const id = bankId(b.rel);
    const entry = { source: CONTENT + b.rel + path.extname(b.file), sha256: b.sha256, bank_number: b.parsed.header.number ?? b.survey.word10, name: b.survey.name, sounds: {}, materials: {} };
    for (const [i, e] of [...b.sounds].sort((x, y) => x[0] - y[0])) if (e) entry.sounds[i] = e;
    for (const mi of [...b.usedMaterials].sort((x, y) => x - y)) {
      const m = b.survey.materials[mi];
      const r = mat.decodeMaterial(b.data, m);
      if (r.kind !== 'pcm') throw new Error(`bank ${b.rel} material ${mi}: codec ${r.info.codecName} is not expected among Sora's sounds`);
      if (r.issues.length) throw new Error(`bank ${b.rel} material ${mi}: ${r.issues.join('; ')}`);
      const w = wav.encode({ channels: r.channels, sampleRate: r.sampleRate, loop: r.loop });
      const file = `wav/${id}/m${String(mi).padStart(3, '0')}.wav`;
      files.set(file, w.buffer);
      let peak = 0; for (const c of r.channels) for (let k = 0; k < c.length; k++) { const v = Math.abs(c[k]); if (v > peak) peak = v; }
      entry.materials[mi] = { file, codec: r.info.codecName, obfuscated: !!r.info.obfuscated, channels: r.channels.length, sample_rate: r.sampleRate, samples: r.sampleCount, seconds: round(r.sampleCount / r.sampleRate), loop: r.loop ? { start: r.loop.start, end: r.loop.end } : null, peak: round(peak, 5), clipped_samples: w.clipped, notes: r.notes && r.notes.length ? r.notes : undefined, bytes: w.buffer.length, sha256: sha256(w.buffer) };
      seconds += r.sampleCount / r.sampleRate;
      materialRows.push({ file, bank: b, mi, m: entry.materials[mi] });
    }
    banks[id] = entry;
  }
  // which categories / buses / macros these sounds use
  const usedCat = new Set(), usedMacro = new Set(), usedBus = new Set();
  for (const b of Object.values(banks)) for (const s of Object.values(b.sounds)) { usedCat.add(s.category.number); [s.macro_start, s.macro_stop, s.macro_event3].forEach((m) => m && usedMacro.add(m)); usedBus.add(s.main_bus.number); s.aux_sends.forEach((a) => usedBus.add(a.bus.number)); }
  const config = {
    source: 'assets/KINGDOM HEARTS III/Config/Windows/WindowsEngine.ini [SQEXSEAD] ConfigString',
    categories: cfg.categories.filter((c) => c.exists).map((c) => ({ number: c.number, name: c.name, max_sounds: c.maxSounds, shares_pool_of: c.sharesPoolOf, used: usedCat.has(c.number) })),
    category_layers: cfg.categoryLayers, buses: cfg.buses.map((x) => ({ number: x.number, name: x.name, volume: x.volume, used: usedBus.has(x.number) })),
    macros: cfg.macros.filter((m) => usedMacro.has(m.number)).map((m) => ({ number: m.number, commands: strip(m.commands) })),
    external_parameters: strip(cfg.externalParameters),
  };
  // trigger -> packed sound
  for (const t of triggers) for (const s of t.sounds) { const a = s.asset && assets[s.asset]; s.packed = !!a; if (a) { s.bank = a.bank; s.sound_index = a.sound_index; if (s.asset.includes('/Localization/ja/')) s.en = enVariant(s.asset); } }
  const manifest = {
    format: 'kh3-sora-audio-pack/1', generator: `tools/sora_audio_pack.js ${VERSION}`, spec: 'docs/sora-audio-spec.md',
    conventions: { wav: 'PCM s16 little-endian, channels and sample rate as stored in the bank, loop in a smpl chunk (start, end inclusive); manifest loop = {start, end exclusive} in samples', volume: 'linear', pitch: 'frequency ratio', time: 'seconds unless a field says ms', voice_language: 'assets are referenced by their /Localization/ja/ path; the English twin (same bank layout and index under /Localization/en/) is packed beside it' },
    counts: { triggers: triggers.length, sound_assets: Object.keys(assets).length, banks: Object.keys(banks).length, sounds: Object.values(banks).reduce((n, b) => n + Object.keys(b.sounds).length, 0), materials: materialRows.length, seconds: round(seconds, 1), unresolved: unresolved.length },
    config, ue_assets: ueAssets, banks, sound_assets: assets, triggers, unresolved, requests_seen: requests.length,
  };
  return { manifest, files, materialRows, triggers, assets, banks };
}

// ------------------------------------------------------------------ validate (reads the pack back)
function validate(dir, manifest) {
  const v = { files: 0, bad: [], sounds_checked: 0, duration_mismatch: [], silent_tracks: 0 };
  for (const [id, b] of Object.entries(manifest.banks)) {
    for (const [mi, m] of Object.entries(b.materials)) {
      const f = P(dir, m.file);
      if (!fs.existsSync(f)) { v.bad.push(`${m.file}: missing`); continue; }
      const buf = fs.readFileSync(f), w = wav.parse(buf);
      v.files++;
      if (sha256(buf) !== m.sha256) v.bad.push(`${m.file}: sha256 differs from the manifest`);
      if (w.format !== 1 || w.bits !== 16 || w.channels !== m.channels || w.sampleRate !== m.sample_rate || w.frames !== m.samples) v.bad.push(`${m.file}: header differs (${w.channels}ch ${w.sampleRate} Hz ${w.frames} frames)`);
      if (!!w.loop !== !!m.loop || (w.loop && (w.loop.start !== m.loop.start || w.loop.end !== m.loop.end))) v.bad.push(`${m.file}: loop differs`);
      if (m.samples <= 0) v.bad.push(`${m.file}: empty`);
      void mi;
    }
    // a sound with one sequence and one key-on of a one-shot material: its header duration is the material's length
    for (const [si, s] of Object.entries(b.sounds)) {
      const tracks = Object.values(s.tracks);
      v.silent_tracks += tracks.filter((t) => t.type === 'MaterialTrack' && t.material === null).length;
      for (const t of tracks) if (t.material !== null && t.material !== undefined && !b.materials[t.material]) v.bad.push(`${id} sound ${si}: material ${t.material} not packed`);
      const mats = tracks.filter((t) => t.material !== null && t.material !== undefined).map((t) => b.materials[t.material]).filter(Boolean);
      if (!mats.length || mats.some((m) => m.loop)) continue;
      v.sounds_checked++;
      const longest = Math.max(...mats.map((m) => m.seconds));
      // the header duration is rounded up to the millisecond by the authoring tool; sequences may add waits
      if (s.sequences.length === 1 && tracks.length === 1 && Math.abs(s.duration - longest) > 0.0015) v.duration_mismatch.push(`${id} sound ${si}: header ${s.duration} s, material ${longest} s`);
    }
  }
  for (const [p, a] of Object.entries(manifest.sound_assets)) if (!manifest.banks[a.bank] || (!a.empty_sound && !manifest.banks[a.bank].sounds[a.sound_index])) v.bad.push(`${p}: bank or sound not in the manifest`);
  v.ok = v.bad.length === 0;
  return v;
}

// ------------------------------------------------------------------ main
if (checkOnly) {
  const manifest = readJson(P(OUT, 'manifest.json'));
  const v = validate(OUT, manifest);
  console.log(`sora_audio_pack ${VERSION} --check: ${v.files} wav files, ${v.bad.length} problems, ${v.duration_mismatch.length} duration mismatches`);
  for (const x of v.bad.slice(0, 20)) console.log('  ' + x);
  process.exitCode = v.ok ? 0 : 1;
} else {
  const { manifest, files, materialRows, triggers, assets, banks } = build();
  fs.rmSync(P(OUT, 'wav'), { recursive: true, force: true });
  for (const [f, buf] of files) { fs.mkdirSync(path.dirname(P(OUT, f)), { recursive: true }); fs.writeFileSync(P(OUT, f), buf); }
  const v = validate(OUT, manifest);
  manifest.validation = { wav_files_read_back: v.files, problems: v.bad, single_material_sounds_checked: v.sounds_checked, duration_mismatches: v.duration_mismatch, key_ons_of_empty_material_slots: v.silent_tracks };
  const mtext = JSON.stringify(manifest, null, 1) + '\n';
  fs.writeFileSync(P(OUT, 'manifest.json'), mtext);
  if (PARTY_EXTRA) {
    // (party D5) the party-only root: no sheets, no sheet_rows (the main pack's own outputs stay untouched)
    const c = manifest.counts;
    console.log(`sora_audio_pack ${VERSION} --party-extra: ${files.size} wav files, ${c.triggers} triggers, ${c.sound_assets} sound assets, ${c.banks} banks -> ${rel(OUT)}; validation ${v.ok ? 'ok' : 'FAILED'}`);
    for (const t of triggers) console.log(`  ${t.trigger}: ${t.sounds.map((s) => s.asset + (s.packed ? '' : ' (NOT PACKED)')).join(', ') || 'no sound'}`);
    for (const x of v.bad.slice(0, 20)) console.log('    ' + x);
    process.exit(v.ok ? 0 : 1);
  }

  // rows for sheets/port/sora-assets.json (13 columns, ids prefixed audio/)
  const usersOf = new Map(); // "bank#index" -> asset names
  for (const [p, a] of Object.entries(assets)) { const k = a.bank + '#' + a.sound_index; if (!usersOf.has(k)) usersOf.set(k, []); usersOf.get(k).push(p.split('/').pop()); }
  const rows = materialRows.map(({ file, bank, mi, m }) => {
    const id = bankId(bank.rel);
    const used = Object.entries(banks[id].sounds).filter(([, s]) => Object.values(s.tracks).some((t) => t.material === mi)).map(([si]) => si);
    const names = used.flatMap((si) => usersOf.get(id + '#' + si) || [`${id.split('/').pop()}#${si}`]);
    return ['audio/' + file, 'sound', names.slice(0, 12), CONTENT + bank.rel + '.uasset', '-', m.bytes, m.sha256, m.seconds, m.samples, m.loop ? 'yes' : 'no', '-',
      `HCA decode: all frame CRCs ok; wav read back (${m.channels} ch, ${m.sample_rate} Hz, ${m.samples} frames)${m.clipped_samples ? `; ${m.clipped_samples} samples clipped at s16` : ''}`,
      `material ${mi} of the bank, keyed on by sound ${used.join(', ')} (tools/lib/sead_bank.js); the sounds are named by the triggers of sheets/port/sora-sounds`];
  });
  rows.push(['audio/manifest.json', 'manifest', '-', '-', '-', Buffer.byteLength(mtext), sha256(Buffer.from(mtext)), '-', '-', '-', '-', v.ok ? `pack check ok (${v.files} wav files read back, ${v.sounds_checked} single-material sounds: header duration = material length on all but ${v.duration_mismatch.length})` : 'pack check FAILED', 'written by tools/sora_audio_pack.js from the items above']);
  fs.writeFileSync(P(OUT, 'sheet_rows.json'), '[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]\n');

  // sheets/port/sora-sounds.json: one row per trigger -> sound (a trigger without a sound keeps one row)
  const cols = ['id', 'route', 'trigger', 'clip', 'time', 'duration', 'sound', 'bank', 'sound_index', 'selection', 'spatial', 'attach', 'volume', 'pitch', 'condition', 'packed', 'evidence', 'ref_addr', 'ref_conf', 'note'];
  const codeIds = new Set();
  // (PACKGEN) the decompilation's function list (sheets/native/code.json); without it (a release checkout) the function
  // addresses the sheet already cites are the known ones (the sheet is in the repository), so the sheet comes out the same
  if (fs.existsSync(P(ROOT, 'sheets', 'native', 'code.json'))) { const text = fs.readFileSync(P(ROOT, 'sheets', 'native', 'code.json'), 'utf8'); const re = /^\["(0x[0-9A-Fa-f]+)"/gm; let m; while ((m = re.exec(text))) codeIds.add(m[1].toUpperCase().replace('0X', '0x')); }
  else if (fs.existsSync(SHEET)) { const s = readJson(SHEET); const ci = s.columns.indexOf('ref_addr'); for (const r of s.rows) if (/^0x[0-9A-F]+$/i.test(String(r[ci]))) codeIds.add(String(r[ci]).toUpperCase().replace('0X', '0x')); }
  const conf = (t) => (t.evidence === 'asset' ? 'confirmed' : /instruction/.test(t.evidence || '') ? 'confirmed' : 'inferred');
  const srows = [];
  // (party E) the party's triggers have their own sheet (sheets/port/party-sounds.json, tools/party_audio_pack.js)
  for (const t of triggers.filter((x) => !x.owner && !/^code\.(effect_se_npc)$/.test(x.route) && !/^n_ex00\d_/.test(String(x.trigger)))) {
    const addr = t.ref_addr && codeIds.has(t.ref_addr) ? t.ref_addr : '-';
    const common = (s, k) => [t.sounds.length > 1 ? `${t.id}#${k}` : t.id, t.route, t.trigger, t.clip || '-', t.time ?? '-', t.duration ?? '-',
      s ? s.asset || '-' : t.sound_ref || '-', s && s.bank ? s.bank : '-', s && s.sound_index !== undefined ? s.sound_index : '-',
      s ? s.how + (s.weight !== undefined && s.weight !== null ? ` (weight ${s.weight})` : '') : t.plays ? (t.sound_ref ? 'looked up per surface in the auto-SE table' : t.sound_id || '-') : 'plays nothing',
      t.spatial || '-', t.attach === null || t.attach === undefined ? '-' : String(t.attach) + (t.follow === true ? ' (follows)' : t.follow === false ? ' (not following)' : ''),
      t.volume ?? (t.volume_text || '-'), t.pitch ?? '-', t.condition || '-', s ? (s.packed ? 'yes' : 'no') : '-',
      t.evidence || '-', addr, conf(t), [t.note, t.loop_stop && t.loop_stop !== 'one-shot' ? 'loop/stop: ' + t.loop_stop : null, t.ref && addr === '-' ? 'ref: ' + t.ref : null, t.verdict ? 'verdict ' + t.verdict : null].filter(Boolean).join(' | ') || '-'];
    if (!t.sounds.length) srows.push(common(null, 0)); else t.sounds.forEach((s, k) => srows.push(common(s, k)));
  }
  const idSeen = new Set(); for (const r of srows) { let id = r[0], n = 1; while (idSeen.has(id)) id = `${r[0]}~${++n}`; r[0] = id; idSeen.add(id); }
  fs.writeFileSync(SHEET, `{"sheet":"sora-sounds","group":"port","columns":${JSON.stringify(cols)},"rows":[\n${srows.map((r) => JSON.stringify(r)).join(',\n')}\n]}\n`);

  const total = [...files.values()].reduce((s, b) => s + b.length, 0);
  const c = manifest.counts;
  console.log(`sora_audio_pack ${VERSION}: ${files.size} wav files, ${(total / 1048576).toFixed(1)} MB, ${c.seconds} s -> ${rel(OUT)}`);
  console.log(`  ${c.triggers} triggers, ${c.sound_assets} sound assets, ${c.banks} banks, ${c.sounds} sounds, ${c.materials} materials; ${c.unresolved} unresolved asset paths`);
  const byRoute = {}; for (const t of triggers) byRoute[t.route] = (byRoute[t.route] || 0) + 1;
  console.log('  triggers by route: ' + Object.entries(byRoute).sort().map(([k, n]) => `${k} ${n}`).join(', '));
  console.log(`  sheet rows: sora-sounds ${srows.length}, sora-assets ${rows.length} (through build/sora_pack/audio/sheet_rows.json, merged by tools/sora_pack.js)`);
  console.log(`  validation: ${v.ok ? 'ok' : 'FAILED'}; ${v.bad.length} problems, ${v.duration_mismatch.length} duration mismatches of ${v.sounds_checked} single-material sounds, ${v.silent_tracks} key-ons of empty slots`);
  for (const x of v.bad.slice(0, 20)) console.log('    ' + x);
  process.exitCode = v.ok ? 0 : 1;
}
