#!/usr/bin/env node
// (KEYBLADES, 2026-11-02) EVERY KEYBLADE OF KH3 (BASE GAME + RE MIND), THEIR FORMCHANGES, DONALD'S AND GOOFY'S WEAPONS AND THE
// MOOGLE, AS PACK FILES. The user: "Add all of the keyblades from KH3 including the DLC and all of their form changes and sound
// effects and visual effects and everything else ... Also add Donald and Goofy's weapons." Plus a KH3 Moogle at Roundtable Hold
// as the synthesis shop. Assets only: the game code that equips them is another wave; it queries keyblades/manifest.json.
//
//   node tools/sora_keyblades_pack.js [--only PHASE[,PHASE]] [--force] [--list] [--out DIR]
//     --out      (KBFX) the keyblades folder written to DIR instead of build/sora_pack/keyblades (a staged rebuild: DIR
//                should hold a copy of the folder, its manifest.json / sora_forms_manifest.json name effects the vfx
//                phase needs); the Moogle (npc) and the audio rows are written where they always are
//     phases (default all, in this order):
//       models     every weapon model the keyblades' equip data names (base keyblade, form weapons, transform ("Deform")
//                  units, "Special" units, the finisher StylePawns of m_AccompanyPawnAssets) -> keyblades/<model>/:
//                  weapon.glb (LOD 0, one skin, the Kingdom Key's conventions), textures/*.png (base colour, normal map),
//                  skeleton.json (kh_anim::Skeleton::from_manifest format, as PACK/xsi/weapons/<model>/skeleton_xw.json),
//                  anims/<clip>.glb (every AnimSequence row of the model's own anim sets, on its own skeleton)
//       materials  the models' translated materials (tools/sora_material_pack.js outfit mode, as tools/party_pack.js and the
//                  XS form weapons do): keyblades/materials/{materials.json, materials.wgsl, textures/, uv2_*.bin}, owner ids
//                  500+, meshes `kb:<model>`; kh-shaderc builds keyblades/materials/er (tools/packgen/recipes/shaders.json)
//       forms      Sora's own clips of the form anim sets (m_ReplaceAnimSets: p_ex001_AnimSet2..23) that the main pack and the
//                  XS overlay do not have -> keyblades/sora_forms/anims/<clip>.glb + keyblades/sora_forms_manifest.json
//                  (an overlay of manifest.json in the xsi_manifest.json format: items, logical_animations, root_motion,
//                  blend spaces; kh-anim Pack::load reads it beside xsi_manifest.json)
//       audio      audit/sora_port/audio/keyblade_rows.json: every keyblade weapon's swing / hit / alias set entries, the
//                  effect-attached SE of the keyblades' effects, the sounds the form clips' notifies name (read by
//                  tools/sora_audio_pack.js, which packs their banks into PACK/audio with the rest)
//       vfx        the keyblades' and the shop's particle systems the main / party / XS effect roots lack -> keyblades/vfx/
//                  (tools/sora_vfx_pack.js --list --no-seed --out: effects/, materials/, textures/, meshes/, manifest.json)
//       npc        the Moogle shop NPCs (g_ex_mogli_01: n_ex014, TresShopActor; g_ex_mogli_02: n_ex028, TresShopActorMogli)
//                  -> npc/<id>/: model.glb, textures/, skeleton.json, anims/<clip>.glb, materials/ (owner ids 600+)
//       manifest   keyblades/manifest.json: per keyblade id (w_soNNN) its name, item, forms (weapons on Sora's sockets, Sora's
//                  anim sets, magic / projectile sets, finisher pawns), sound sets, effects and the root that holds each
//                  effect; per model its files; Donald's and Goofy's 14 + 14 weapons (PACK/party/weapons, indexed only);
//                  the Moogle; the KH3 folders the build reads (for setup/pack_inputs.json)
//       anim_instances (DRILL-IDLE) keyblades/anim_instances.json: per model its anim blueprint's default clip and loop /
//                  check-same bits (what a form weapon shows while Sora plays a clip it has no row of: idle, walk, jump ...)
//     --force    rebuild every model / clip / material even when its fingerprint says it is up to date
//     --list     print the inventory and stop (nothing written)
//
// INCREMENTAL: every model and every form clip carries a fingerprint (this tool's version + the sha256 of every cooked file it
// was made from); a model whose fingerprint did not change is not written again, a model that is new is added, a model no
// longer named is left alone (never deleted). The material library is made again only when its slot list or a source changes.
// A pack build that adds a keyblade therefore writes that keyblade's files only.
//
// Inputs (nothing under assets/ or assets_converted/ is modified): the decoded packages, converted meshes / animations / textures,
// the cooked files (hashed), Content/Localization/Game/en/kh3_item.locres (names), sheets/entities/Load/Tres/TresItemData.json and
// TresEquipItemData.json (item rows). Helpers: tools/sora_pack.js. Everything that names an asset is read from the game's data.
// Local build output from the user's own copy of the game; never distributed.
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const sp = require('./sora_pack.js');
const { ROOT, CONTENT, sha256, readGlb, readAccessor, loadPkg, importPath, gamePathToContent, buildSkeleton, materialInfo, texturePng, GlbWriter, addSkeletonNodes, addMesh, loadAnimation, addAnimation, f32, sourceFiles, rel } = sp;

const VERSION = '1.0.3';
// (KBFX) the effect root's own version (its fingerprint only: the models' fingerprints keep VERSION, so a change here
// rewrites keyblades/vfx and not the 68 models). 1.1.0: names reserved against the main effect root, the root's own table
// (tools/sora_vfx_shaders.js --root), the engine textures (tools/sora_vfx_extra_textures.js --pack).
const VFX_VERSION = '1.1.0';
const P = (...a) => path.join(...a);
const PACK = P(ROOT, 'build', 'sora_pack');
// (KBFX) `--out DIR`: the keyblades folder written somewhere else (a staged rebuild); default PACK/keyblades
const OUT = process.argv.indexOf('--out') > 0 ? path.resolve(process.argv[process.argv.indexOf('--out') + 1]) : P(PACK, 'keyblades');
const NPC_OUT = P(PACK, 'npc');
const STAGE = P(ROOT, 'build', 'sora_tmp', 'keyblades');
const PKG = P(ROOT, 'assets_converted', 'packages', CONTENT);
const KB_ID_BASE = 500; // owner ids: Sora 0.., Donald 100.., Goofy 200.., team props 300.., XS form weapons 400..
const NPC_ID_BASE = 600;
const PLAYER = 'Blueprints/Player/p_ex001';
const SORA_SKELETON = 'Character/pc/p_ex001/mdl/p_ex001_0_Skeleton';
const SORA_BODY = '/Game/Character/pc/p_ex001/anm/rt/body/';
// the Moogle shops (Blueprints/Gimmick/ex): their own mesh and anim blueprint, the shop actor class, its effects
const SHOPS = ['Blueprints/Gimmick/ex/g_ex_mogli_01', 'Blueprints/Gimmick/ex/g_ex_mogli_02'];
const SWING_NAMES = ['NOTHING', 'SWING1', 'SWING2', 'SWING3', 'SWING4', 'SWING5', 'SWING6', 'SWING7', 'SWING8', 'SWING9', 'SWING10', 'SWING11', 'SWING12', 'SWING13'];
const HIT_NAMES = ['NOTHING', 'HIT1', 'HIT2', 'HIT3', 'HIT4', 'HIT5', 'HIT6', 'HIT7', 'HIT8', 'HIT9', 'HIT10', 'HIT11'];

const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const PHASES = ['models', 'materials', 'forms', 'audio', 'vfx', 'npc', 'manifest', 'anim_instances']; // (DRILL-IDLE) + anim_instances
const only = opt('--only') ? new Set(opt('--only').split(',')) : null;
const want = (p) => !only || only.has(p);
const FORCE = args.includes('--force');
const J = (o) => Buffer.from(JSON.stringify(o, null, 1) + '\n');
const exists = (f) => fs.existsSync(f);
const readJsonIf = (f) => { try { return JSON.parse(fs.readFileSync(f, 'utf8')); } catch { return null; } };
const pkgExists = (content) => exists(P(PKG, content + '.uasset.json'));
const hashOf = (...parts) => { const h = crypto.createHash('sha256'); for (const p of parts) h.update(typeof p === 'string' ? p : JSON.stringify(p)).update('\0'); return h.digest('hex'); };
const srcHash = (content) => { try { return sourceFiles(content); } catch { return null; } };
let written = 0, writtenBytes = 0, kept = 0;
function put(file, buf) { // write only when the bytes differ (no churn for unchanged files)
  if (exists(file) && fs.statSync(file).size === buf.length && fs.readFileSync(file).equals(buf)) { kept++; return; }
  // (AUDIT-A SHIELD) a staged rebuild (--out) may hold hard links into the live pack: never write through one
  if (exists(file) && fs.statSync(file).nlink > 1) fs.unlinkSync(file);
  fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, buf); written++; writtenBytes += buf.length;
}

// ------------------------------------------------------------------------------------------------ decoded-package helpers
const objPath = (d, i) => (!i ? null : i < 0 ? importPath(d, i) : d.exports[i - 1].objectName);
const isProps = (v) => Array.isArray(v) && v.length > 0 && v.every((e) => e && typeof e === 'object' && !Array.isArray(e) && 'name' in e && 'type' in e);
function plainProps(d, props) {
  const o = {};
  for (const p of props || []) {
    const v = plainValue(d, p);
    if (p.arrayIndex !== undefined) { if (!Array.isArray(o[p.name])) o[p.name] = o[p.name] === undefined ? [] : [o[p.name]]; o[p.name][p.arrayIndex] = v; } else o[p.name] = v;
  }
  return o;
}
function plainValue(d, p) {
  const v = p.value;
  if (p.type === 'ObjectProperty') return objPath(d, v);
  if (p.type === 'ArrayProperty') {
    if (p.innerType === 'ObjectProperty') return (v || []).map((i) => objPath(d, i));
    if (v && Array.isArray(v.items)) return v.items.map((it) => (isProps(it) || (Array.isArray(it) && !it.length) ? plainProps(d, it) : it));
    return v;
  }
  if (p.type === 'MapProperty') return (v.entries || []).map((e) => ({ key: isProps(e.key) ? plainProps(d, e.key) : e.key, value: isProps(e.value) ? plainProps(d, e.value) : p.valueType === 'ObjectProperty' ? objPath(d, e.value) : e.value }));
  if (p.type === 'StructProperty') return isProps(v) ? plainProps(d, v) : v;
  if (v && typeof v === 'object' && 's' in v && 'enc' in v) return v.s;
  return v;
}
const stripObj = (p) => (p ? String(p).replace(/_C$/, '').replace(/\.[^./]+$/, '') : null); // "/Game/A/B.B_C" -> "/Game/A/B"
const contentOf = (p) => gamePathToContent(stripObj(p));
const imports = (d) => d.imports.map((im, i) => importPath(d, -(i + 1)));
const short = (e) => (e ? String(e).split('::').pop() : e);

// A blueprint class and its blueprint parents: the class default object and every component template, merged parent first.
const bpCache = new Map();
function blueprint(bpPath) {
  const key = stripObj(bpPath);
  if (bpCache.has(key)) return bpCache.get(key);
  const chain = []; let c = gamePathToContent(key), native = null;
  while (c && pkgExists(c) && chain.length < 8) {
    const d = loadPkg(c); const cls = d.exports.find((e) => e.class === 'BlueprintGeneratedClass');
    chain.push(d); if (!cls) break;
    const sup = importPath(d, cls.superIndex);
    if (!sup.startsWith('/Game/')) { native = sup; break; }
    c = gamePathToContent(stripObj(sup));
  }
  let cdo = {}; const comps = {};
  for (const d of [...chain].reverse()) {
    const cd = d.exports.find((e) => /^Default__/.test(e.objectName)); if (!cd) continue;
    cdo = { ...cdo, ...plainProps(d, cd.data.props) };
    const ci = d.exports.indexOf(cd) + 1;
    for (const e of d.exports.filter((x) => x.outerIndex === ci)) comps[e.objectName] = { class: e.class, ...(comps[e.objectName] || {}), ...plainProps(d, e.data.props) };
    // components of the simple construction script (StylePawns / gimmicks): templates named *_GEN_VARIABLE
    for (const e of d.exports.filter((x) => /_GEN_VARIABLE$/.test(x.objectName))) { const n = e.objectName.replace(/_GEN_VARIABLE$/, ''); comps[n] = { class: e.class, ...(comps[n] || {}), ...plainProps(d, e.data.props) }; }
  }
  // the mesh component: the weapon's TresWeaponMesh0, a pawn's TresCharMesh0, a gimmick's TresGimmickMesh0; else any with a mesh
  const meshComp = comps.TresWeaponMesh0 || comps.TresCharMesh0 || comps.TresGimmickMesh0 || Object.values(comps).find((x) => x.SkeletalMesh);
  // a component the data does not serialize the mesh of: the mesh import of the blueprint package itself
  let mesh = meshComp && meshComp.SkeletalMesh ? meshComp.SkeletalMesh : null;
  let animClass = meshComp && meshComp.AnimClass ? meshComp.AnimClass : null;
  if (!mesh && chain[0]) { const m = JSON.stringify(imports(chain[0])).match(/\/Game\/Character\/[A-Za-z0-9_]+\/[A-Za-z0-9_]+\/mdl\/[A-Za-z0-9_]+_0\b/); if (m) mesh = m[0]; }
  if (!animClass && chain[0]) { const m = JSON.stringify(imports(chain[0])).match(/\/Game\/Character\/[A-Za-z0-9_]+\/[A-Za-z0-9_]+\/anm\/[A-Za-z0-9_]+_Anim\b/); if (m) animClass = m[0]; }
  const effects = {};
  for (const [k, v] of Object.entries(cdo)) if (typeof v === 'string' && /\/Game\/Effects\//.test(v)) effects[k] = v;
  const r = { blueprint: '/Game/' + gamePathToContent(key), chain: chain.map((d) => '/Game/' + d._content), native, cdo, comps, mesh: mesh ? stripObj(mesh) : null, anim_class: animClass ? stripObj(animClass) : null, effects,
    sounds: { hit: stripObj(cdo.m_SoundAssets_Hit), swing: stripObj(cdo.m_SoundAssets_Swing), alias: stripObj(cdo.m_SoundAliasAsset) } };
  bpCache.set(key, r);
  return r;
}
// every /Game/Effects/ path inside a value tree
function effectsIn(v, out = new Set()) {
  if (typeof v === 'string') { if (/^\/Game\/Effects\//.test(v)) out.add(stripObj(v)); return out; }
  if (Array.isArray(v)) { for (const x of v) effectsIn(x, out); return out; }
  if (v && typeof v === 'object') for (const x of Object.values(v)) effectsIn(x, out);
  return out;
}
function animSetRows(gamePath) {
  const d = loadPkg(contentOf(gamePath)); const e = d.exports.find((x) => x.class === 'TresAnimSet'); if (!e) return [];
  const anims = (e.data.props || []).find((p) => p.name === 'Anims'); if (!anims) return [];
  return anims.value.items.map((it) => { const n = it.find((x) => x.name === 'AnimName'), a = it.find((x) => x.name === 'AnimData'); return { name: n ? n.value : null, asset: a && a.value ? importPath(d, a.value) : null }; });
}
function animBlueprint(animClass) {
  if (!animClass || !pkgExists(contentOf(animClass))) return { sets: [], default_anim: null, instance: null };
  const d = loadPkg(contentOf(animClass)); const cdo = d.exports.find((e) => /^Default__/.test(e.objectName));
  const p = cdo ? plainProps(d, cdo.data.props) : {};
  // (DRILL-IDLE) the default-clip fields of UTresAnimInstance (+0x420 .. +0x430, read by its play by name 0x14056ECE0). A
  // blueprint CDO stores only what differs from its parent's, and the native defaults are None / false (the CDOs that set
  // them all set true), so a missing field is false.
  const instance = { anim_class: stripObj(animClass), default_anim: p.m_DefaultAnimName || null, default_anim_loop: p.m_bDefaultAnimAlwaysLoopPlay === true, default_anim_check_same: p.m_bDefaultAnimCheckSame === true, stop_if_asset_none: p.m_bAnimStopIfAnimAssetIsNone === true };
  return { sets: (p.MyAnimSets || []).filter(Boolean).map(stripObj), default_anim: p.m_DefaultAnimName || null, instance };
}
const mainClass = (content) => { const d = loadPkg(content); const e = d.exports.find((x) => x.outerIndex === 0) || d.exports[0]; return e.class; };

// ------------------------------------------------------------------------------------------------ the inventory
function soraSockets() {
  const d = loadPkg(SORA_SKELETON); const out = {};
  for (const e of d.exports) {
    if (e.class !== 'SkeletalMeshSocket') continue;
    const p = Object.fromEntries((e.data.props || []).map((x) => [x.name, x.value]));
    out[String(p.SocketName).toLowerCase()] = { name: p.SocketName, bone: p.BoneName, location_ue: p.RelativeLocation || { x: 0, y: 0, z: 0 }, rotation_ue: p.RelativeRotation || { pitch: 0, yaw: 0, roll: 0 }, scale_ue: p.RelativeScale || { x: 1, y: 1, z: 1 } };
  }
  return out;
}
function locres() {
  const f = P(ROOT, 'assets', CONTENT, 'Localization', 'Game', 'en', 'kh3_item.locres');
  if (!exists(f)) return null;
  const L = require('./lib/locres.js'); const d = L.decodeLocres(fs.readFileSync(f)); const map = new Map();
  for (const ns of d.namespaces) for (const k of ns.keys) { const text = k.text !== undefined ? k.text : d.strings[k.stringIndex]; map.set(`${ns.namespace.s}.${k.key.s}`, text && typeof text === 'object' ? text.s : text); }
  return map;
}
function itemRows() { // equip asset -> its item rows (TresEquipItemData m_EquipSubclass) with the item name
  const eq = readJsonIf(P(ROOT, 'sheets', 'entities', 'Load', 'Tres', 'TresEquipItemData.json'));
  const it = readJsonIf(P(ROOT, 'sheets', 'entities', 'Load', 'Tres', 'TresItemData.json'));
  const names = locres(); const byEquip = new Map();
  if (!eq) return { byEquip, note: 'sheets/entities/Load/Tres/TresEquipItemData.json missing: names / item rows not determined' };
  const itemOf = new Map((it ? it.rows : []).map((r) => [r._row_name, r]));
  for (const r of eq.rows) {
    if (!r.m_EquipSubclass) continue;
    const item = itemOf.get(r._row_name) || {};
    const name = names && item.m_NameTextID ? names.get(item.m_NameTextID) || null : null;
    const k = stripObj(r.m_EquipSubclass).toLowerCase();
    if (!byEquip.has(k)) byEquip.set(k, []);
    byEquip.get(k).push({ row: r._row_name, key: r.m_KeyName, name, name_text_id: item.m_NameTextID || null, jp_comment: typeof r.m_Comment === 'object' && r.m_Comment ? r.m_Comment.s : r.m_Comment || null, icon: item.m_IconAsset || null,
      stats: { ap: r.m_AP, str: r.m_AttackPlus, mag: r.m_MagicPlus, def: r.m_DefensePlus } });
  }
  return { byEquip, note: names ? null : 'kh3_item.locres missing: English names not determined' };
}

// (KBFX) the projectile blueprints a projectile / magic set (or a pawn) names, and those they name in turn (a projectile that
// respawns as another class, a finisher pawn that spawns another: w_so036 -> StylePawn/w_so035, ProjSHThunder),
// Blueprints/Projectile and Blueprints/StylePawn only, at most 4 deep
function projectileBlueprints(set) {
  const out = []; const seen = new Set();
  const visit = (gp, depth) => {
    const c = gamePathToContent(stripObj(gp)); if (!c || seen.has(c) || !pkgExists(c) || depth > 4) return; seen.add(c);
    if (depth > 0) out.push('/Game/' + c);
    for (const x of imports(loadPkg(c))) if (/^\/Game\/Blueprints\/(Projectile|StylePawn)\//.test(x) && !/\.|Default__/.test(x.slice(6))) visit(x, depth + 1);
  };
  if (set) visit(set, 0);
  return out;
}

function inventory() {
  const sockets = soraSockets();
  const items = itemRows();
  const dir = P(PKG, PLAYER);
  const equips = fs.readdirSync(dir).filter((f) => /^w_so\d+_Equip\.uasset\.json$/i.test(f)).map((f) => f.replace(/\.uasset\.json$/, '')).sort();
  const keyblades = []; const models = new Map(); const effects = new Map(); // effect -> Set(used_by)
  const useFx = (e, by) => { if (!e) return; const k = stripObj(e); if (!effects.has(k)) effects.set(k, new Set()); effects.get(k).add(by); };
  const addModel = (bpPath, role, kb) => {
    const b = blueprint(bpPath);
    if (!b.mesh) return null;
    const model = path.basename(gamePathToContent(b.mesh)).replace(/_0$/, '');
    if (!models.has(model)) models.set(model, { model, mesh: b.mesh, blueprints: new Set(), roles: new Set(), keyblades: new Set(), anim_class: b.anim_class });
    const m = models.get(model); m.blueprints.add(b.blueprint); m.roles.add(role); m.keyblades.add(kb);
    if (!m.anim_class && b.anim_class) m.anim_class = b.anim_class;
    for (const [k, v] of Object.entries(b.effects)) useFx(v, `${kb} ${path.basename(b.blueprint)} ${k}`);
    for (const c of Object.values(b.comps)) for (const e of effectsIn(c)) useFx(e, `${kb} ${path.basename(b.blueprint)} component`);
    return { model, blueprint: b.blueprint, mesh: b.mesh, anim_class: b.anim_class, sounds: b.sounds };
  };
  for (const eq of equips) {
    const d = loadPkg(`${PLAYER}/${eq}`); const e = d.exports.find((x) => /TresEquip.*Data/.test(x.class)); if (!e) continue;
    const p = plainProps(d, e.data.props);
    const id = eq.replace(/_Equip$/i, '');
    const rows = items.byEquip.get(`/game/${PLAYER}/${eq}`.toLowerCase()) || [];
    const kb = { id, equip: `/Game/${PLAYER}/${eq}`, class: e.class, items: rows, name: rows.length ? rows[0].name : null, form_change_charge_effect: stripObj(p.m_CmnFormChangeChargeEffect), forms: [], base: null };
    useFx(p.m_CmnFormChangeChargeEffect, `${id} m_CmnFormChangeChargeEffect`);
    for (const en of p.m_EquipMap || []) {
      for (const l of en.value.m_List || []) {
        const unit = (u, transform) => {
          const sock = u.AttachBoneName && u.AttachBoneName !== 'None' ? sockets[String(u.AttachBoneName).toLowerCase()] || null : null;
          const role = transform ? 'transform' : u.UnitName === 'Special' ? 'special' : short(en.key) === 'TRES_WEAPON_FORM_01' ? 'keyblade' : 'form_weapon';
          const m = u.RsrcData ? addModel(u.RsrcData, role, id) : null;
          return { unit: u.UnitName, part: short(u.m_AttachPart), socket: u.AttachBoneName && u.AttachBoneName !== 'None' ? u.AttachBoneName : null, socket_found_on_sora: sock ? true : u.AttachBoneName === 'None' ? null : false,
            bone: sock ? sock.bone : null, socket_location_ue: sock ? sock.location_ue : null, socket_rotation_ue: sock ? sock.rotation_ue : null, socket_scale_ue: sock ? sock.scale_ue : null, apply_parent_scale: !!u.m_bApplyParentScale,
            blueprint: m ? m.blueprint : stripObj(u.RsrcData), model: m ? m.model : null, sounds: m ? m.sounds : null };
        };
        const pawns = (l.m_AccompanyPawnAssets || []).map((a) => { const m = a.m_AssetData ? addModel(a.m_AssetData, 'pawn', id) : null; return { command: short(a.m_Command), blueprint: stripObj(a.m_AssetData), model: m ? m.model : null }; });
        const projSet = stripObj(l.m_ReplaceProjectileSet), magicSet = stripObj(l.m_ReplaceMagicSet);
        for (const s of [projSet, magicSet]) if (s && pkgExists(gamePathToContent(s))) { const sd = loadPkg(gamePathToContent(s)); for (const x of imports(sd)) if (/^\/Game\/Effects\//.test(x)) useFx(x, `${id} ${path.basename(s)}`); }
        // (KBFX) the projectiles of those sets: their blueprints (Blueprints/Projectile/..., and the ones they respawn as:
        // TresProjGM_RocketPunch0 -> TresProjGM_RocketPunch) with their own meshes (Nano Arms' Rocket Fist w_so173) and effects
        // (its attach data ead_so170_com0, its hit effect): before, only the sets' own effect imports were read
        const projectiles = [];
        for (const s of [projSet, magicSet, ...pawns.map((p) => p.blueprint)]) for (const bp of projectileBlueprints(s)) {
          const b = blueprint(bp);
          const m = b.mesh && pkgExists(gamePathToContent(b.mesh)) ? addModel(bp, /\/StylePawn\//.test(bp) ? 'pawn' : 'projectile', id) : null;
          if (!m) { for (const v of Object.values(b.effects)) useFx(v, `${id} ${path.basename(b.blueprint)}`); for (const c of Object.values(b.comps)) for (const e of effectsIn(c)) useFx(e, `${id} ${path.basename(b.blueprint)} component`); }
          for (const x of imports(loadPkg(gamePathToContent(stripObj(bp))))) if (/^\/Game\/Effects\//.test(x)) useFx(stripObj(x), `${id} ${path.basename(b.blueprint)}`);
          projectiles.push({ set: s, blueprint: b.blueprint, model: m ? m.model : null });
        }
        const form = { form: short(en.key), grow_level: short(l.m_GrowLevel), wear_form: short(l.m_OwnerWearForm), sora_anim_sets: (l.m_ReplaceAnimSets || []).filter(Boolean).map(stripObj), magic_set: magicSet, projectile_set: projSet,
          ik_offset_ue: l.m_ReplaceIKOffset || null, ui_data: stripObj(l.m_UIDataAsset), weapons: (l.m_Equips || []).map((u) => unit(u, false)), transform: (l.m_TransformSetData || []).map((u) => unit(u, true)), accompany_pawns: pawns, projectiles };
        kb.forms.push(form);
        if (form.form === 'TRES_WEAPON_FORM_01' && form.grow_level === 'TRES_WEAPON_GROW_LVL01' && form.weapons[0]) kb.base = form.weapons[0].model;
      }
    }
    keyblades.push(kb);
  }
  // every model's own effect folders (Effects/wep/<model>/par, Effects/dlc01/wep/<model>/par) and its anim sets
  for (const m of models.values()) {
    for (const root of ['Effects/wep', 'Effects/dlc01/wep', 'Effects/dlc02/wep']) {
      const dd = P(PKG, root, m.model, 'par');
      if (exists(dd)) for (const f of fs.readdirSync(dd)) if (f.endsWith('.uasset.json')) { const c = `${root}/${m.model}/par/${f.replace(/\.uasset\.json$/, '')}`; if (mainClass(c) === 'ParticleSystem') useFx('/Game/' + c, `${m.model} effect folder`); }
    }
    const ab = animBlueprint(m.anim_class); m.anim_sets = ab.sets; m.default_anim = ab.default_anim; m.anim_instance = ab.instance; // (DRILL-IDLE)
  }
  return { sockets, keyblades, models, effects, item_note: items.note };
}

// ------------------------------------------------------------------------------------------------ models
const pick = (tex, re) => Object.keys(tex || {}).find((k) => re.test(k));
// the files of one model (weapon.glb + textures + skeleton + its own clips) and its fingerprint
function modelSources(meshContent, clipAssets) {
  const srcs = { mesh: srcHash(meshContent) };
  const g = readGlb(P(ROOT, 'assets_converted', 'meshes', CONTENT, meshContent + '.glb'));
  for (const m of g.json.materials) { const gp = m.extras && m.extras.ue_material; if (!gp) continue; try { const info = materialInfo(gp); srcs[gp] = info.chain.map((c) => srcHash(gamePathToContent(c))); for (const t of Object.values(info.tex)) if (t) srcs[t] = srcHash(gamePathToContent(t)); } catch (e) { srcs[gp] = 'unreadable: ' + e.message; } }
  for (const a of clipAssets) srcs[a] = srcHash(gamePathToContent(a));
  return { srcs, glb: g };
}
// A converted mesh with more than four influences per vertex (JOINTS_1 / WEIGHTS_1): the four largest weights of every vertex,
// renormalised, as new JOINTS_0 / WEIGHTS_0 accessors (the renderer's vertex has four). Returns the glb unchanged otherwise and
// the number of vertices that lost an influence.
function fourInfluences(g) {
  const prims = g.json.meshes[0].primitives;
  if (!prims.some((p) => p.attributes.JOINTS_1 !== undefined)) return { g, trimmed: 0 };
  const json = JSON.parse(JSON.stringify(g.json)); const chunks = [g.bin]; let len = g.bin.length; let trimmed = 0;
  const addView = (buf) => { const pad = (4 - (len % 4)) % 4; if (pad) { chunks.push(Buffer.alloc(pad)); len += pad; } json.bufferViews.push({ buffer: 0, byteOffset: len, byteLength: buf.length }); chunks.push(buf); len += buf.length; return json.bufferViews.length - 1; };
  for (const p of json.meshes[0].primitives) {
    const A = p.attributes; if (A.JOINTS_1 === undefined) continue;
    const j0 = readAccessor(g, A.JOINTS_0), j1 = readAccessor(g, A.JOINTS_1), w0 = readAccessor(g, A.WEIGHTS_0), w1 = readAccessor(g, A.WEIGHTS_1);
    const div = (a) => (a.componentType === 5121 ? 255 : a.componentType === 5123 ? 65535 : 1);
    const n = j0.n; const J4 = Buffer.alloc(n * 8), W4 = Buffer.alloc(n * 16);
    for (let i = 0; i < n; i++) {
      const inf = []; for (let c = 0; c < 4; c++) { inf.push([j0.data[i * 4 + c], w0.data[i * 4 + c] / div(w0)]); inf.push([j1.data[i * 4 + c], w1.data[i * 4 + c] / div(w1)]); }
      inf.sort((a, b) => b[1] - a[1]); if (inf[4][1] > 0) trimmed++;
      const top = inf.slice(0, 4); const s = top.reduce((x, y) => x + y[1], 0) || 1;
      top.forEach(([j, w], c) => { J4.writeUInt16LE(w > 0 ? j : 0, i * 8 + c * 2); W4.writeFloatLE(w / s, i * 16 + c * 4); });
    }
    json.accessors.push({ bufferView: addView(J4), componentType: 5123, count: n, type: 'VEC4' }); A.JOINTS_0 = json.accessors.length - 1;
    json.accessors.push({ bufferView: addView(W4), componentType: 5126, count: n, type: 'VEC4' }); A.WEIGHTS_0 = json.accessors.length - 1;
    delete A.JOINTS_1; delete A.WEIGHTS_1;
  }
  return { g: { json, bin: Buffer.concat(chunks), bytes: null }, trimmed };
}
function weaponGlb(name, meshGlb0, bones, generator) {
  const { g: meshGlb, trimmed } = fourInfluences(meshGlb0);
  const textures = new Map();
  const mats = meshGlb.json.materials.map((m, slot) => {
    const gp = m.extras && m.extras.ue_material; let info = { tex: {} };
    try { if (gp) info = materialInfo(gp); } catch { info = { tex: {} }; }
    // the glTF material (= the library's slot) name is unique in the pack: <the game's slot name>@<model>#<slot index> (a transform
    // unit reuses its keyblade's material slots, a model may name one slot twice; the translated library keys slots by name)
    const unique = `${m.name}@${name}#${slot}`;
    const gl = { name: unique, pbrMetallicRoughness: { metallicFactor: 0, roughnessFactor: 0.8, baseColorFactor: [0.6, 0.6, 0.6, 1] }, extras: { ue_slot_name: m.name, ue_material: gp || null, ue_root_material: info.root_material || null, ue_blend_mode: info.blend_mode || null } };
    const own = [];
    const base = pick(info.tex, /^ba[sc]e.?col|^basetex$|base_col|diffuse|albedo|^color/i) || pick(info.tex, /^#.*_0d0[0-9]$/i); // (BaceColorTex: the fur shader's spelling; #..._0d00: a root's unparameterised base colour sample)
    if (base && info.tex[base]) { try { const t = texturePng(info.tex[base], 'color'); own.push(t); textures.set(t.file, t); gl.pbrMetallicRoughness.baseColorTexture = { index: t.file }; delete gl.pbrMetallicRoughness.baseColorFactor; } catch { /* a texture the build did not convert: the flat factor stays */ } }
    const normal = pick(info.tex, /normal/i);
    if (normal && info.tex[normal]) { try { const t = texturePng(info.tex[normal], 'normal'); own.push(t); textures.set(t.file, t); gl.normalTexture = { index: t.file }; } catch { /* left out */ } }
    if (info.two_sided) gl.doubleSided = true;
    return { gltf: gl, textures: own, rec: { slot, slot_name: unique, ue_slot_name: m.name, material_instance: gp || null, root_material: info.root_material || null, blend_mode: info.blend_mode || null, two_sided: !!info.two_sided, base_color_parameter: base || null, normal_parameter: normal || null } };
  });
  const w = new GlbWriter(`(KEYBLADES) via ${generator}`); addSkeletonNodes(w, bones);
  const r = addMesh(w, meshGlb, bones, mats.map((m) => ({ gltf: JSON.parse(JSON.stringify(m.gltf)), textures: m.textures, rec: m.rec })));
  w.json.skins[0].name = name; w.json.meshes[0].name = name + '_LOD0'; w.json.nodes[r.node].name = name + '_mesh'; w.json.scenes = [{ name, nodes: [0, r.node] }];
  return { glb: w.finish(), textures, stats: { ...r.stats, vertices_with_more_than_4_influences_trimmed: trimmed }, materials: mats.map((m) => m.rec), joints: r.joints };
}
const skeletonJson = (meshPath, bones) => ({ format: 'kh3-sora-weapon-skeleton-xw/1', source_mesh: meshPath, note: 'kh_anim::Skeleton::from_manifest reads skeleton.bones (translation_ue cm, rotation_ue xyzw, scale_ue); bone order = the mesh\'s, a parent before its children; translation / rotation / scale = the pack conventions',
  skeleton: { bone_count: bones.length, root: bones[0].name, bones: bones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, translation: b.translation.map(f32), rotation: b.rotation.map(f32), scale: b.scale.map(f32), translation_ue: b.ue.translation, rotation_ue: b.ue.rotation, scale_ue: b.ue.scale })) } });
// one clip on a model's skeleton: tracks of bones the mesh does not have are dropped (UE binds tracks by name)
// (FORMWEP-FIX) ... by FName, which compares WITHOUT case: the clip's `Center` / `ring_a` tracks drive the mesh's `center` /
// `ring_A` (w_so021's StyleChange2 2401 carries the whole Double Arrowguns transformation on `Center`; dropped, the unit
// stayed at his feet while he flew 8.2 m). A track binds to the mesh bone of its name in any case; only a name no bone has
// in any case is dropped.
function clipOnSkeleton(asset, bones) {
  const content = gamePathToContent(stripObj(asset));
  const gf = P(ROOT, 'assets_converted', 'animations', CONTENT, content + '.glb');
  if (!exists(gf)) return { error: 'not converted (assets_converted/animations has no ' + content + '.glb)' };
  const extra = []; const g = readGlb(gf);
  const named = bones.map((b) => ({ ...b })); const lower = new Map(bones.map((b, i) => [b.name.toLowerCase(), i]));
  for (const c of g.json.animations[0].channels) {
    const n = g.json.nodes[c.target.node].name; const i = lower.get(n.toLowerCase());
    if (i !== undefined) named[i].name = n; // the clip's spelling, so loadAnimation's exact match binds it
    else if (!extra.some((b) => b.name === n)) extra.push({ name: n });
  }
  let a; try { a = loadAnimation(stripObj(asset), [...named, ...extra]); } catch (e) { return { error: e.message }; }
  const dropped = [...new Set(a.chans.filter((c) => c.bone >= bones.length).map((c) => extra[c.bone - bones.length].name))];
  a.chans = a.chans.filter((c) => c.bone < bones.length);
  return { a, dropped };
}
function packModel(name, meshPath, animSets, defaultAnim, outDir, generator, kind) {
  const meshContent = gamePathToContent(meshPath);
  const meshFile = P(ROOT, 'assets_converted', 'meshes', CONTENT, meshContent + '.glb');
  if (!exists(meshFile)) return { model: name, error: `mesh not converted: assets_converted/meshes/.../${meshContent}.glb` };
  // its clips: every row of its anim sets (logical name -> sequence); a row naming a blend space is listed, not packed
  const rows = []; const seqs = new Map();
  for (const s of animSets) { if (!pkgExists(gamePathToContent(s))) { rows.push({ anim_set: s, error: 'anim set package missing' }); continue; } for (const r of animSetRows(s)) { if (!r.asset) continue; const c = gamePathToContent(stripObj(r.asset)); const cls = pkgExists(c) ? mainClass(c) : 'missing'; rows.push({ anim_set: s, name: r.name, asset: stripObj(r.asset), class: cls }); if (cls === 'AnimSequence' && !seqs.has(stripObj(r.asset))) seqs.set(stripObj(r.asset), path.basename(c)); } }
  // (AUDIT-A SHIELD) a row naming a BLEND SPACE: its samples are packed too (each under its own sequence name), so a host can
  // draw the sample a core blend picks (Pegasus's / the Chariot's M_B425: B425, B425DL, B425DR, B425UL, B425UR; kh-core
  // shield::finisher_pawns names the heaviest one). The blend space row itself stays listed in rows_not_sequences.
  for (const r of rows.filter((x) => /^BlendSpace/.test(x.class || ''))) {
    const d = loadPkg(gamePathToContent(r.asset)); const e = d.exports.find((x) => /^BlendSpace/.test(x.class)); if (!e) continue;
    const sd = (e.data.props || []).find((p) => p.name === 'SampleData'); if (!sd) continue;
    for (const it of sd.value.items) { const a = it.find((x) => x.name === 'Animation'); const s = a && a.value ? importPath(d, a.value) : null; if (!s) continue; const sa = stripObj(s); const c = gamePathToContent(sa); if (pkgExists(c) && mainClass(c) === 'AnimSequence' && !seqs.has(sa)) seqs.set(sa, path.basename(c)); }
  }
  const { srcs, glb: meshGlb } = modelSources(meshContent, [...seqs.keys()]);
  const fingerprint = hashOf(VERSION, kind, meshPath, animSets, srcs);
  const dir = P(outDir, name); const recFile = P(dir, 'model.json');
  const old = readJsonIf(recFile);
  if (!FORCE && old && old.fingerprint === fingerprint && exists(P(dir, old.glb || 'weapon.glb')) && (old.clips || []).every((c) => !c.file || exists(P(dir, c.file)))) { kept++; return { ...old, unchanged: true }; }
  const bones = buildSkeleton(meshGlb.json);
  const g = weaponGlb(name, meshGlb, bones, generator);
  const glbName = kind === 'npc' ? 'model.glb' : 'weapon.glb';
  put(P(dir, glbName), g.glb);
  for (const t of g.textures.values()) put(P(dir, t.file), t.png);
  put(P(dir, 'skeleton.json'), J(skeletonJson(meshPath, bones)));
  const clips = [];
  for (const [asset, clipName] of seqs) {
    const r = clipOnSkeleton(asset, bones);
    if (r.error) { clips.push({ sequence: clipName, asset, error: r.error }); continue; }
    const w = new GlbWriter(`(KEYBLADES) via ${generator}`); addSkeletonNodes(w, bones); addAnimation(w, clipName, r.a); w.json.scenes = [{ name: clipName, nodes: [0] }];
    const buf = w.finish(); const file = `anims/${clipName}.glb`; put(P(dir, file), buf);
    clips.push({ sequence: clipName, asset, file, bytes: buf.length, duration_s: r.a.length, frames: r.a.numFrames, notifies: r.a.notifies.map((n) => ({ class: n.class, kind: n.kind, trigger_time: n.trigger_time, end_trigger_time: n.end_trigger_time ?? null, properties: n.properties })), dropped_tracks_of_bones_not_in_this_mesh: r.dropped,
      logical_names: rows.filter((x) => x.asset === asset).map((x) => x.name) });
  }
  const files = [glbName, 'skeleton.json', ...[...g.textures.keys()], ...clips.filter((c) => c.file).map((c) => c.file)];
  const bytes = files.reduce((s, f) => s + fs.statSync(P(dir, f)).size, 0);
  const rec = { format: 'kh3-sora-keyblade-model/1', generator: `tools/sora_keyblades_pack.js ${VERSION}`, model: name, kind, fingerprint, source_mesh: meshPath, glb: glbName, glb_sha256: sha256(g.glb), skeleton: 'skeleton.json', bones: bones.length, skin_joints: g.joints.length,
    stats: { vertices: g.stats.vertices, triangles: g.stats.triangles, primitives: g.stats.primitives, vertices_with_more_than_4_influences_trimmed: g.stats.vertices_with_more_than_4_influences_trimmed }, materials: g.materials, textures: [...g.textures.keys()], anim_sets: animSets, default_anim: defaultAnim, rows, clips, bytes, sources: srcs };
  put(recFile, J(rec));
  return rec;
}

// ------------------------------------------------------------------------------------------------ materials
// tools/sora_material_pack.js in outfit mode over a staged folder that lists every slot of the models (as tools/party_pack.js and
// the XS form weapons' tool do); then slot -> mesh `<prefix><model>`, the Kingdom Key's records dropped, owner ids ID_BASE + n,
// the KHS post-process block left out (Sora's library has it), the third UV sets as uv2_*.bin.
// A slot the character translator can take: its shader owner (the first material of the chain with a shader map) has GPU-skin
// base-pass shaders (tools/sora_material_pack.js VF / BASE_PS). Effect materials (unlit, translucent particles' materials
// put on a mesh) have none: those slots are listed, not translated.
function translatable(instance) {
  try {
    for (const c of materialInfo(instance).chain) {
      const d = loadPkg(gamePathToContent(c)); const e = d.exports.find((x) => x.outerIndex === 0 && /^Material(InstanceConstant)?$/.test(x.class));
      const maps = e && e.data && e.data.native && e.data.native.shaderMaps;
      if (!maps || !maps.length) continue;
      const msm = (maps[0].shaderMap.meshShaderMaps || []).find((m) => m.vertexFactoryType === 'TGPUSkinVertexFactoryfalse');
      if (!msm) return { ok: false, why: `no GPU-skin shaders in ${c}` };
      if (!msm.shaders.some((s) => s.type === 'TBasePassPSFCachedPointIndirectLightingPolicy')) return { ok: false, why: `no lit base-pass pixel shader in ${c} (an unlit / effect material)` };
      return { ok: true };
    }
    return { ok: false, why: 'no shader map in the chain' };
  } catch (e) { return { ok: false, why: e.message }; }
}
function buildMaterials(models, outDir, prefix, idBase, label) {
  const slots = []; const meshOfSlot = new Map(); const dupes = []; const untranslatable = [];
  for (const m of models) for (const s of m.materials) {
    if (!s.material_instance) continue;
    const tr = translatable(s.material_instance);
    if (!tr.ok) { untranslatable.push({ model: m.model, slot: s.slot_name, material: s.material_instance, why: tr.why }); continue; }
    if (meshOfSlot.has(s.slot_name)) { dupes.push(`${s.slot_name} (${meshOfSlot.get(s.slot_name)} and ${prefix}${m.model})`); continue; }
    meshOfSlot.set(s.slot_name, `${prefix}${m.model}`); slots.push({ slot: s.slot, slot_name: s.slot_name, material_instance: s.material_instance });
  }
  const srcs = slots.map((s) => { try { const i = materialInfo(s.material_instance); return [s.slot_name, i.chain.map((c) => srcHash(gamePathToContent(c)))]; } catch (e) { return [s.slot_name, e.message]; } });
  const fingerprint = hashOf(VERSION, label, idBase, slots, srcs, fs.readFileSync(P(ROOT, 'tools', 'sora_material_pack.js')));
  const old = readJsonIf(P(outDir, 'materials', 'materials.json'));
  if (!FORCE && old && old.keyblades && old.keyblades.fingerprint === fingerprint) { console.log(`  ${label} materials: unchanged (${Object.keys(old.materials).length} slots)`); return { report: old.keyblades.report, untranslatable: old.keyblades.untranslatable || [], unchanged: true }; }
  const stage = P(STAGE, 'matstage_' + label);
  fs.rmSync(stage, { recursive: true, force: true }); fs.mkdirSync(stage, { recursive: true });
  const first = models.find((m) => m.glbFile);
  fs.copyFileSync(first.glbFile, P(stage, 'model_stage.glb'));
  fs.writeFileSync(P(stage, 'manifest.json'), JSON.stringify({ format: 'keyblades-stage', materials: slots, items: [{ kind: 'mesh', file: 'model_stage.glb' }], mesh_content: gamePathToContent(first.source_mesh), wearform_set: null }));
  const t0 = Date.now();
  const r = cp.spawnSync(process.execPath, [P(ROOT, 'tools', 'sora_material_pack.js')], { env: { ...process.env, KH_OUTFIT_PACK: stage, KH_XSI_WEAPONS: '1' }, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error('sora_material_pack.js failed: ' + (r.stderr || '').slice(-3000) + (r.stdout || '').slice(-3000));
  const lines = (r.stdout || '').split(/\r?\n/).filter((l) => l.trim());
  const json = JSON.parse(fs.readFileSync(P(stage, 'materials', 'materials.json'), 'utf8')); const wgsl = fs.readFileSync(P(stage, 'materials', 'materials.wgsl'), 'utf8');
  for (const [k, v] of Object.entries(json.materials)) { if (v.mesh === 'kingdom_key' || !meshOfSlot.has(k)) delete json.materials[k]; else v.mesh = meshOfSlot.get(k); }
  const used = new Set(Object.values(json.materials).map((v) => v.owner));
  const idMap = new Map();
  for (const [name, o] of Object.entries(json.owners)) { if (!used.has(name)) { delete json.owners[name]; continue; } idMap.set(o.id, idBase + o.id); o.id = idBase + o.id; }
  for (const v of Object.values(json.materials)) v.id = json.owners[v.owner] ? json.owners[v.owner].id : v.id;
  const blocks = [...wgsl.matchAll(/#ifdef KH_C(\d+)\n([\s\S]*?)\n#endif\n/g)];
  const outWgsl = `// GENERATED by tools/sora_keyblades_pack.js ${VERSION} from tools/sora_material_pack.js (outfit mode) for the ${label}: owner ids ${idBase}+. Do not edit.\n\n` + blocks.filter((b) => idMap.has(+b[1])).map((b) => `#ifdef KH_C${idMap.get(+b[1])}\n${b[2]}\n#endif\n`).join('\n');
  const uv2 = {}; const uvFiles = [];
  for (const m of models) {
    if (!m.glbFile) continue;
    const g = readGlb(m.glbFile);
    g.json.meshes[0].primitives.forEach((p, k) => {
      if (p.attributes.TEXCOORD_2 === undefined) return;
      const a = readAccessor(g, p.attributes.TEXCOORD_2); const buf = Buffer.alloc(a.data.length * 4); a.data.forEach((v, i) => buf.writeFloatLE(v, i * 4));
      const name = `uv2_${m.model}_${k}.bin`; uvFiles.push([name, buf]);
      uv2[`${prefix}${m.model}:${g.json.materials[p.material].name}`] = { file: name, vertices: a.data.length / 2, primitive: k };
    });
  }
  json.uv2 = uv2;
  json.mesh_facts = {};
  for (const m of models) { const f = P(ROOT, 'assets_converted', 'meshes', CONTENT, gamePathToContent(m.source_mesh) + '.mesh.json'); if (!exists(f)) continue; const mm = /"importedBounds":\[([^\]]+)\]/.exec(fs.readFileSync(f, 'utf8')); const b = mm ? mm[1].split(',').map(Number) : null; if (b && b.length === 7) json.mesh_facts[`${prefix}${m.model}`] = { source: rel(f), bone_params: [], bounds: { origin: b.slice(0, 3), extent: b.slice(3, 6), radius: b[6] } }; }
  json.khs = null; json.wear_forms = null;
  const owners = Object.values(json.owners); const ok = owners.filter((o) => o.base && o.base.ok).length;
  const standIns = Object.entries(json.materials).filter(([, v]) => v.stand_in_for).map(([k, v]) => `${k} (${v.owner})`);
  const texFiles = new Set(); for (const v of Object.values(json.materials)) for (const t of v.textures || []) if (t && t.file) texFiles.add(t.file);
  const report = `${Object.keys(json.materials).length} material slots, ${owners.length} shader owners, ${ok} translated; stand-ins: ${standIns.join(', ') || 'none'}; ${texFiles.size} textures; ${uvFiles.length} uv2 files; slot names shared by two models (the first model's record serves both): ${dupes.join(', ') || 'none'}; slots not translatable (effect / unlit materials): ${untranslatable.length}`;
  json.keyblades = { generator: `tools/sora_keyblades_pack.js ${VERSION}`, id_base: idBase, mesh_prefix: prefix, fingerprint, report, shared_slots: dupes, untranslatable, note: `the ${label}' translated materials (meshes ${prefix}<model>); texture and uv2 files relative to this folder; shaders: ${prefix === 'kb:' ? 'tools/packgen_shaders.js (recipe kb_dirs: tools/splice_kb_wgsl.js + tools/kb_shader_jobs.js, kh-shaderc --rebuild in Sora\'s vertex layout, then --only tex)' : `kh-shaderc --pack ${rel(outDir)} --party --main-pack build/sora_pack`} -> materials/er` };
  put(P(outDir, 'materials', 'materials.json'), Buffer.from(JSON.stringify(json)));
  put(P(outDir, 'materials', 'materials.wgsl'), Buffer.from(outWgsl));
  for (const f of texFiles) put(P(outDir, 'materials', f), fs.readFileSync(P(stage, 'materials', f)));
  for (const [name, buf] of uvFiles) put(P(outDir, 'materials', name), buf);
  console.log(`  ${label} materials (${((Date.now() - t0) / 1000).toFixed(0)} s): ${report}`);
  for (const l of lines.filter((x) => /NOT TRANSLATED|NOT DRAWN|stand-in|missing|BAD/.test(x)).slice(0, 20)) console.log('    [material pack] ' + l);
  return { report, untranslatable, problems: lines.filter((x) => /NOT TRANSLATED|NOT DRAWN|stand-in|missing|BAD/.test(x)) };
}

// ------------------------------------------------------------------------------------------------ Sora's form clips
function buildForms(inv) {
  const mainMf = readJsonIf(P(PACK, 'manifest.json'));
  if (!mainMf) return { error: 'build/sora_pack/manifest.json missing: run tools/sora_pack.js first' };
  const id = mainMf.identification;
  const meshGlb = readGlb(P(ROOT, 'assets_converted', 'meshes', CONTENT, gamePathToContent(id.skeletal_mesh) + '.glb'));
  const bones = buildSkeleton(meshGlb.json);
  const have = new Set(mainMf.items.filter((i) => i.kind === 'animation').map((i) => i.animation_name));
  const xsi = readJsonIf(P(PACK, 'xsi_manifest.json')) || readJsonIf(P(ROOT, 'build', 'sora_tmp', 'xsi', 'pack', 'xsi_manifest.json'));
  const inXsi = new Set(xsi ? xsi.items.map((i) => i.animation_name) : []);
  // + the anim sets Sora's wear forms replace (p_ex001_WearformSet m_Forms[].m_ReplaceAnimSets: Anti = Rage Form's AnimSet13)
  const wearSets = [];
  { const d = loadPkg(`${PLAYER}/p_ex001_WearformSet`); const e = d.exports.find((x) => /WearformSet/.test(x.class)); for (const p of (e && e.data.props) || []) if (p.name === 'm_Forms') for (const s of plainProps(d, p.value).m_ReplaceAnimSets || []) if (s) wearSets.push(stripObj(s)); }
  const sets = [...new Set([...inv.keyblades.flatMap((k) => k.forms.flatMap((f) => f.sora_anim_sets)), ...wearSets])].sort((a, b) => a.localeCompare(b, 'en', { numeric: true }));
  const outDir = P(OUT, 'sora_forms');
  const old = readJsonIf(P(OUT, 'sora_forms_manifest.json'));
  const oldItems = new Map((old ? old.items : []).map((i) => [i.animation_name, i]));
  const setRows = {}; const seqs = new Map(); const blends = {};
  for (const s of sets) {
    const rows = animSetRows(s); setRows[s] = [];
    for (const r of rows) {
      if (!r.asset) { setRows[s].push({ name: r.name, asset: null }); continue; }
      const c = gamePathToContent(stripObj(r.asset)); const cls = pkgExists(c) ? mainClass(c) : 'missing';
      const row = { name: r.name, asset: stripObj(r.asset), class: cls, clip: path.basename(c) };
      if (cls === 'AnimSequence') { if (!seqs.has(row.asset)) seqs.set(row.asset, { set: s, names: [] }); seqs.get(row.asset).names.push(r.name); }
      else if (/^BlendSpace/.test(cls)) { try { const b = sp.describeAnimAsset(row.asset); blends[row.clip] = b; row.blend_samples = b.samples.map((x) => path.basename(gamePathToContent(stripObj(x.asset)))); for (const x of b.samples) { const a = stripObj(x.asset); if (!seqs.has(a)) seqs.set(a, { set: s, names: [] }); seqs.get(a).names.push(`${r.name} (sample of ${row.clip})`); } } catch (e) { row.error = e.message; } }
      setRows[s].push(row);
    }
  }
  const items = []; const logical = []; const anims = new Map(); const skipped = []; let made = 0, reused = 0;
  for (const [asset, info] of seqs) {
    const clip = path.basename(gamePathToContent(asset));
    if (have.has(clip) || inXsi.has(clip)) { skipped.push({ clip, in: have.has(clip) ? 'manifest.json' : 'xsi_manifest.json' }); continue; }
    const content = gamePathToContent(asset); const src = srcHash(content);
    const file = `sora_forms/anims/${clip}.glb`;
    const prev = oldItems.get(clip);
    const fp = hashOf(VERSION, asset, src);
    let a = null;
    if (!FORCE && prev && prev.fingerprint === fp && exists(P(OUT, file))) { items.push({ ...prev, logical_names: info.names }); reused++; continue; }
    try { a = loadAnimation(asset, bones); } catch (e) { skipped.push({ clip, error: e.message }); continue; }
    const w = new GlbWriter(`(KEYBLADES: tools/sora_keyblades_pack.js ${VERSION})`); addSkeletonNodes(w, bones); addAnimation(w, clip, a); w.json.scenes = [{ name: clip, nodes: [0] }];
    const buf = w.finish(); put(P(OUT, file), buf); made++;
    anims.set(asset, a);
    items.push({ file, kind: 'animation', fingerprint: fp, bytes: buf.length, sha256: sha256(buf), source_asset: '/Game/' + content, source_sha256: src, animation_name: clip, logical_names: info.names, anim_set: info.set, duration_s: a.length, num_frames: a.numFrames, frame_rate: a.fps, loop: null,
      root_motion_enabled: a.props.bEnableRootMotion === true, tracks: a.tracks, interpolation: a.chans.length ? a.chans[0].interpolation : 'LINEAR', sequence_properties: a.props, key_counts: a.keyCounts, root_bone: a.root.kh_sk, root_track_moves: a.root_track_moves, notifies: a.notifies });
  }
  for (const s of sets) for (const r of setRows[s]) if (r.asset) logical.push({ name: r.name, anim_set: s, asset: r.asset, class: r.class, clip: r.clip, file: r.class === 'AnimSequence' ? (items.find((i) => i.animation_name === r.clip) || {}).file || null : null, in_main_pack: have.has(r.clip), in_xsi_overlay: inXsi.has(r.clip), optional: true, stage1: false });
  let rootTracks = old && old.root_motion ? old.root_motion.tracks : {};
  if (anims.size) { try { const rm = sp.buildRootMotion(anims, bones, logical.filter((l) => anims.has(l.asset)).map((l) => ({ ...l, play: { root_translation_scale: [1, 1] } })), id, new Set()); rootTracks = { ...rootTracks, ...rm.tracks }; } catch (e) { console.log('  forms: root motion not extracted: ' + e.message); } }
  items.sort((x, y) => x.animation_name.localeCompare(y.animation_name, 'en', { numeric: true }));
  const manifest = { format: 'kh3-sora-pack-overlay/1', generator: `tools/sora_keyblades_pack.js ${VERSION}`,
    note: 'KEYBLADES overlay of manifest.json: Sora\'s own clips of the form anim sets (TresEquipKeybladeData m_ReplaceAnimSets) that manifest.json and xsi_manifest.json lack. items / logical_animations / root_motion.tracks in the main manifest\'s formats; files relative to build/sora_pack/keyblades. kh-anim Pack::load reads it (merge_overlay). A row of a set whose clip the main pack or the XS overlay has is listed with in_main_pack / in_xsi_overlay. Blend spaces: describeAnimAsset records (their samples are items). Local build output, never distributed.',
    legal: mainMf.legal, anim_sets: Object.fromEntries(sets.map((s) => [s, setRows[s]])), blend_spaces: blends, items, logical_animations: logical, root_motion: { tracks: rootTracks }, skipped };
  put(P(OUT, 'sora_forms_manifest.json'), J(manifest));
  const bytes = items.reduce((s, i) => s + (i.bytes || 0), 0);
  console.log(`  forms: ${sets.length} anim sets, ${items.length} clips (${made} written, ${reused} unchanged), ${(bytes / 1048576).toFixed(1)} MB; ${skipped.filter((x) => x.in).length} rows already in the main pack / XS overlay; ${skipped.filter((x) => x.error).length} not packed`);
  return { sets, clips: items.length, bytes, skipped, blend_spaces: Object.keys(blends).length, items };
}

// ------------------------------------------------------------------------------------------------ audio rows
const AU = P(ROOT, 'assets_converted', 'audio', CONTENT);
function soundBrief(asset) {
  if (!asset) return { asset: null };
  const a = stripObj(asset), c = gamePathToContent(a), sf = P(AU, c + '.sead.json');
  const o = { asset: a, exists: exists(sf) };
  if (o.exists) { const j = JSON.parse(fs.readFileSync(sf, 'utf8')); const pr = Object.fromEntries(j.props.map((p) => [p.tag.name, p.value])); o.sound_index = pr.SoundIndex ?? 0; o.duration = pr.Duration ?? null; }
  return o;
}
function enumSet(setPath) {
  if (!setPath || !pkgExists(gamePathToContent(setPath))) return null;
  const d = loadPkg(gamePathToContent(setPath)); const p = plainProps(d, d.exports[0].data.props);
  return { set: setPath, refs: (p.References || []).map((r) => ({ sound: stripObj(r.Sound), bFollow: r.bFollow ?? null, AttachPointName: r.AttachPointName ?? null })) };
}
function aliasSet(setPath) { // setable_w_so010 (TresSoundAliasSet): the static array m_SoundCueAlias[i] -> sound
  if (!setPath || !pkgExists(gamePathToContent(setPath))) return null;
  const d = loadPkg(gamePathToContent(setPath)); const p = plainProps(d, d.exports[0].data.props);
  const arr = Array.isArray(p.m_SoundCueAlias) ? p.m_SoundCueAlias : p.m_SoundCueAlias ? [p.m_SoundCueAlias] : [];
  return { set: setPath, entries: arr.map((s, i) => ({ key: `m_SoundCueAlias[${i}]`, index: i, sound: stripObj(s) })).filter((e) => e.sound) };
}
function seAttachIndex() { // effect name (lower) -> SeAttach packages
  const idx = new Map(); const root = P(PKG, 'Sound', 'SE', '_Data', 'SeAttach');
  const walk = (dir) => { if (!exists(dir)) return; for (const f of fs.readdirSync(dir, { withFileTypes: true })) { const p = P(dir, f.name); if (f.isDirectory()) walk(p); else if (f.name.endsWith('.uasset.json')) { const relp = path.relative(PKG, p).split(path.sep).join('/').replace(/\.uasset\.json$/, ''); const k = path.basename(relp).toLowerCase(); if (!idx.has(k)) idx.set(k, []); idx.get(k).push(relp); } } };
  walk(root);
  return idx;
}
function buildAudio(inv, models, forms) {
  const weaponRows = []; const sets = {};
  for (const m of inv.models.values()) {
    for (const bpPath of m.blueprints) {
      const b = blueprint(bpPath);
      const sw = enumSet(b.sounds.swing), ht = enumSet(b.sounds.hit), al = aliasSet(b.sounds.alias);
      sets[m.model] = sets[m.model] || { swing: null, hit: null, alias: null, blueprint: b.blueprint };
      if (sw && !sets[m.model].swing) { sets[m.model].swing = sw.set; sw.refs.forEach((r, i) => weaponRows.push({ kind: 'weapon_swing', weapon: m.model, set: sw.set, set_from: b.blueprint, index: i, enum_name: SWING_NAMES[i] || null, bFollow: r.bFollow, AttachPointName: r.AttachPointName, ...soundBrief(r.sound) })); }
      if (ht && !sets[m.model].hit) { sets[m.model].hit = ht.set; ht.refs.forEach((r, i) => weaponRows.push({ kind: 'weapon_hit', weapon: m.model, set: ht.set, set_from: b.blueprint, index: i, enum_name: HIT_NAMES[i] || null, bFollow: r.bFollow, AttachPointName: r.AttachPointName, ...soundBrief(r.sound) })); }
      if (al && !sets[m.model].alias) { sets[m.model].alias = al.set; for (const e of al.entries) weaponRows.push({ kind: 'weapon_alias', weapon: m.model, set: al.set, set_from: b.blueprint, index: e.index, enum_name: e.key, ...soundBrief(e.sound) }); }
    }
  }
  // the effect-attached SE of every keyblade effect (Sound/SE/_Data/SeAttach/<...>/<effect name>)
  const idx = seAttachIndex(); const seRows = [];
  // effects whose SE the audio pack has already (Sora's code rows, the party's): not again (a second row would play it twice)
  const known = new Set();
  const pr = readJsonIf(P(ROOT, 'audit', 'sora_port', 'audio', 'party_rows.json'));
  for (const r of [...(readJsonIf(P(ROOT, 'audit', 'sora_port', 'audio', 'code_rows.json')) || []), ...((pr && pr.code_rows) || [])]) { const m = /^Effect (\S+)/.exec(r.trigger || ''); if (m && /^effect_se/.test(r.category || '')) known.add(m[1].toLowerCase()); }
  for (const [eff] of [...inv.effects].sort()) {
    const name = path.basename(gamePathToContent(eff)).toLowerCase();
    if (known.has(name)) continue;
    for (const relp of idx.get(name) || []) {
      const d = loadPkg(relp);
      for (const e of d.exports) {
        if (!/SeAttach/i.test(e.class)) continue;
        const pr = plainProps(d, e.data.props);
        const list = Array.isArray(pr.SeAttachDataList) ? pr.SeAttachDataList : Array.isArray(pr.m_SeAttachDataList) ? pr.m_SeAttachDataList : [pr];
        for (const x of list) { const snd = x.Sound || x.m_Sound || null; if (!snd && !x.SoundAsset) continue; seRows.push({ category: 'effect_se_keyblade', trigger: `Effect ${path.basename(gamePathToContent(eff))} ${x.bPlayAtEnd ? 'ends' : 'starts'}`, ref: `/Game/${relp} :: ${e.objectName}`, verdict: 'same-partial', condition: `GroupID ${x.GroupID ?? 0}`, sound_id: 'SQEX_EffectSeAttachData.Sound', sound_asset: stripObj(snd || x.SoundAsset), spatial: '3D', attach: 'attached to the particle component', volume: 'asset default', pitch: null, loop_stop: `FadeOutTime ${x.FadeOutTime ?? null}${x.bNoSeFadeAtTheEffectEnd ? '; bNoSeFadeAtTheEffectEnd' : ''}${x.bPlayAtEnd ? '; bPlayAtEnd' : ''}`, evidence: 'asset', note: 'keyblade effect (tools/sora_keyblades_pack.js)' }); }
      }
    }
  }
  // the sounds Sora's form clips and the weapons' own clips name in their notifies (voices, sounds, random voices)
  const requests = new Set();
  const noteSounds = (n) => { for (const v of [JSON.stringify(n.properties || {})]) for (const m of v.matchAll(/\/Game\/(?:Sound|ActorVoice)\/[A-Za-z0-9_/.-]+/g)) requests.add(stripObj(m[0])); };
  for (const it of forms && forms.items ? forms.items : []) for (const n of it.notifies || []) noteSounds(n);
  for (const m of models) for (const c of m.clips || []) for (const n of c.notifies || []) noteSounds(n);
  const out = { format: 'kh3-keyblade-audio-rows/1', generator: `tools/sora_keyblades_pack.js ${VERSION}`, note: 'read by tools/sora_audio_pack.js: weapon_rows (as party_rows.json weapon_rows, without an owner: Sora\'s routes, per weapon model), code_rows (effect-attached SE, the code_rows.json format) and requests (sounds the form clips\' and the weapons\' notifies name: their banks are packed)',
    sets, weapon_rows: weaponRows, code_rows: seRows, requests: [...requests].sort() };
  put(P(ROOT, 'audit', 'sora_port', 'audio', 'keyblade_rows.json'), J(out));
  console.log(`  audio: ${Object.keys(sets).length} weapon models, ${weaponRows.length} set entries (${weaponRows.filter((r) => r.asset && !r.exists).length} naming a sound without a converted SEAD export), ${seRows.length} effect SE rows, ${requests.size} notify sounds -> audit/sora_port/audio/keyblade_rows.json`);
  return out;
}

// ------------------------------------------------------------------------------------------------ effects
function effectRoots() { // effect name (lower) -> the root that has it
  const have = new Map();
  for (const [root, dir] of [['vfx', P(PACK, 'vfx', 'effects')], ['party/vfx', P(PACK, 'party', 'vfx', 'effects')], ['xsi/vfx', P(PACK, 'xsi', 'vfx', 'effects')]]) if (exists(dir)) for (const f of fs.readdirSync(dir)) if (f.endsWith('.json')) { const k = f.replace(/\.json$/, '').toLowerCase(); if (!have.has(k)) have.set(k, root); }
  return have;
}
function buildVfx(inv) {
  // effects named by the form clips' / weapon clips' AttachEffect notifies are in inv.effects already when the caller added them
  const have = effectRoots();
  const list = []; const where = {}; const attach = {};
  // an effect attach data asset (SQEX_ParticleAttachDataAsset, the ead/ folders: the form-change charge, the key trail, the
  // magic cast effect) lists its particle systems: they join the list, the asset is recorded with them
  for (const [eff, by] of [...inv.effects]) {
    const c = gamePathToContent(eff); if (!pkgExists(c) || mainClass(c) !== 'SQEX_ParticleAttachDataAsset') continue;
    const systems = imports(loadPkg(c)).filter((x) => /^\/Game\/Effects\//.test(x)).map(stripObj).filter((x, i, a) => a.indexOf(x) === i && pkgExists(gamePathToContent(x)) && mainClass(gamePathToContent(x)) === 'ParticleSystem');
    attach[eff] = systems;
    for (const s of systems) { if (!inv.effects.has(s)) inv.effects.set(s, new Set()); inv.effects.get(s).add(`${path.basename(c)} (attach data of: ${[...by].slice(0, 2).join('; ')})`); }
  }
  for (const [eff, by] of [...inv.effects].sort()) {
    if (attach[eff]) { where[eff] = { attach_data: true, systems: attach[eff] }; continue; }
    const name = path.basename(gamePathToContent(eff)).toLowerCase();
    const c = gamePathToContent(eff); const cls = pkgExists(c) ? mainClass(c) : null;
    if (have.has(name)) { where[eff] = have.get(name); continue; }
    if (cls !== 'ParticleSystem') { where[eff] = cls ? `not a particle system (${cls})` : 'no such package'; continue; }
    where[eff] = 'keyblades/vfx';
    list.push({ effect: eff, package: `${CONTENT}/${c}.uasset`, exists: true, used_by: [...by].sort(), class: cls });
  }
  fs.mkdirSync(STAGE, { recursive: true });
  const listFile = P(STAGE, 'effect_list.json'); fs.writeFileSync(listFile, JSON.stringify(list, null, 1));
  // (KBFX) the main effect root's material / mesh names are RESERVED (sora_vfx_pack.js --reserve): a keyblade material or mesh
  // whose name the main root has for another asset is packed under a name of its own (tmi_loc_00sa__wep_w_so080_mat, Crystal
  // Snow's, beside the main root's tmi_loc_00sa = Wheel of Fate's), so the host's kbfx: namespace (kh-host vfx.rs
  // namespace_def: names the main pack lacks) always reaches this root's own record; a name both have for the SAME asset
  // keeps it (the main pack's record is that asset's). The reserved names are part of the fingerprint.
  const mainVfx = P(PACK, 'vfx');
  const reserved = (() => { const m = readJsonIf(P(mainVfx, 'manifest.json')) || {}; const names = (k) => (m[k] || []).filter((x) => x.file).map((x) => [x.file, x.asset]).sort(); const dir = (k) => (exists(P(mainVfx, k)) ? fs.readdirSync(P(mainVfx, k)).sort() : []); return hashOf(names('materials'), names('meshes'), dir('materials'), dir('meshes')); })();
  const fp = hashOf(VERSION, VFX_VERSION, reserved, list.map((l) => l.effect), list.map((l) => srcHash(gamePathToContent(l.effect))), fs.readFileSync(P(ROOT, 'tools', 'sora_vfx_pack.js')));
  const outDir = P(OUT, 'vfx'); const stamp = readJsonIf(P(outDir, 'keyblades_fp.json'));
  let ran = false, log = '';
  if (list.length && (FORCE || !stamp || stamp.fingerprint !== fp || !exists(P(outDir, 'manifest.json')))) {
    const t0 = Date.now();
    const r = cp.spawnSync(process.execPath, [P(ROOT, 'tools', 'sora_vfx_pack.js'), '--list', listFile, '--no-seed', '--reserve', mainVfx, '--out', outDir], { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 28 });
    log = (r.stdout || '') + (r.stderr || '');
    fs.writeFileSync(P(STAGE, 'vfx_pack.log'), log);
    if (r.status !== 0 && !exists(P(outDir, 'manifest.json'))) throw new Error('sora_vfx_pack.js failed: ' + log.split('\n').slice(-12).join('\n'));
    // (KBFX) + the engine's default textures the materials sample (dither noise, DefaultTexture: as the main root's)
    const x = cp.spawnSync(process.execPath, [P(ROOT, 'tools', 'sora_vfx_extra_textures.js'), '--pack', outDir], { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 28 });
    fs.writeFileSync(P(STAGE, 'vfx_extra_textures.log'), (x.stdout || '') + (x.stderr || ''));
    if (x.status !== 0) throw new Error('sora_vfx_extra_textures.js failed for keyblades/vfx: ' + ((x.stdout || '') + (x.stderr || '')).split('\n').slice(-6).join('\n'));
    put(P(outDir, 'keyblades_fp.json'), J({ generator: `tools/sora_keyblades_pack.js ${VERSION} (vfx ${VFX_VERSION})`, fingerprint: fp, effects: list.length, exit: r.status }));
    ran = true; console.log(`  vfx: sora_vfx_pack.js over ${list.length} effects (${((Date.now() - t0) / 1000).toFixed(0)} s, exit ${r.status}) -> ${rel(outDir)}`);
  } else console.log(`  vfx: ${list.length} effects for keyblades/vfx, unchanged`);
  // (KBFX) the root's OWN shading table (tools/sora_vfx_shaders.js --root: this root's materials and texture records only,
  // owner ids 0.. of its own owners; --xw-break: a loop's break translated), from which kh-shaderc --vfx --pack
  // PACK/keyblades builds keyblades/vfx/er (the shader phase: tools/packgen_shaders.js, which checks that every translated
  // owner was built). Remade when the effects or the table tools change.
  const tablesFp = hashOf(fp, fs.readFileSync(P(ROOT, 'tools', 'sora_vfx_shaders.js')), fs.readFileSync(P(ROOT, 'tools', 'sora_vfx_extra_textures.js')));
  const tstamp = readJsonIf(P(outDir, 'shaders', 'keyblades_tables_fp.json'));
  if (FORCE || ran || !exists(P(outDir, 'shaders', 'shading.json')) || !tstamp || tstamp.fingerprint !== tablesFp) {
    const t1 = Date.now();
    const r = cp.spawnSync(process.execPath, [P(ROOT, 'tools', 'sora_vfx_shaders.js'), '--root', outDir, '--xw-break'], { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 28 });
    fs.writeFileSync(P(STAGE, 'vfx_shaders.log'), (r.stdout || '') + (r.stderr || ''));
    if (r.status !== 0) throw new Error('sora_vfx_shaders.js failed for keyblades/vfx: ' + ((r.stdout || '') + (r.stderr || '')).split('\n').slice(-8).join('\n'));
    put(P(outDir, 'shaders', 'keyblades_tables_fp.json'), J({ generator: `tools/sora_keyblades_pack.js ${VERSION} (vfx ${VFX_VERSION})`, fingerprint: tablesFp, note: 'kh-shaderc --vfx --pack PACK/keyblades must run after this table is made (the built er/<s|m|d|v><id> files are keyed by its owner ids)' }));
    console.log(`  vfx: own shading table (${((Date.now() - t1) / 1000).toFixed(0)} s): ${((r.stdout || '').match(/\d+ of \d+ pixel shaders translated/) || ['?'])[0]}; ${((r.stdout || '').match(/\d+ referenced textures are not in the pack/) || ['?'])[0]}`);
  }
  const mf = readJsonIf(P(outDir, 'manifest.json'));
  const packed = new Set(mf && Array.isArray(mf.effects) ? mf.effects.map((e) => String(e.name || e.effect || e.file || e).toLowerCase().replace(/^effects\//, '').replace(/\.json$/, '')) : []);
  const failures = mf ? mf.failures || [] : [];
  return { where, list: list.length, ran, packed: packed.size, failures, log_tail: log.split('\n').slice(-6).join('\n') };
}

// ------------------------------------------------------------------------------------------------ the Moogle
function buildNpcs(inv) {
  const out = [];
  for (const shop of SHOPS) {
    if (!pkgExists(shop)) { out.push({ shop: '/Game/' + shop, error: 'blueprint missing' }); continue; }
    const b = blueprint('/Game/' + shop);
    if (!b.mesh) { out.push({ shop: b.blueprint, error: 'no skeletal mesh' }); continue; }
    const npc = path.basename(gamePathToContent(b.mesh)).replace(/_0$/, '');
    const ab = animBlueprint(b.anim_class);
    // the NPC's other anim sets in its anm folder (the GM set, the holo set): the clips a talk / idle loop may want
    const anmDir = P(PKG, path.dirname(path.dirname(gamePathToContent(b.mesh))), 'anm');
    const extraSets = [];
    const walk = (dir) => { if (!exists(dir)) return; for (const f of fs.readdirSync(dir, { withFileTypes: true })) { if (f.isDirectory()) walk(P(dir, f.name)); else if (/AnimSet[^/]*\.uasset\.json$/.test(f.name)) { const c = path.relative(PKG, P(dir, f.name)).split(path.sep).join('/').replace(/\.uasset\.json$/, ''); if (mainClass(c) === 'TresAnimSet') extraSets.push('/Game/' + c); } } };
    walk(anmDir);
    const sets = [...new Set([...ab.sets, ...extraSets])];
    const rec = packModel(npc, b.mesh, sets, ab.default_anim, NPC_OUT, `tools/sora_keyblades_pack.js ${VERSION}`, 'npc');
    const effects = [...effectsIn({ cdo: b.cdo, comps: b.comps }), ...imports(loadPkg(shop)).filter((x) => /^\/Game\/Effects\//.test(x)).map(stripObj)];
    for (const e of new Set(effects)) { if (!inv.effects.has(e)) inv.effects.set(e, new Set()); inv.effects.get(e).add(`shop ${path.basename(shop)}`); }
    out.push({ shop: b.blueprint, native_class: b.native, npc, mesh: b.mesh, anim_class: b.anim_class, anim_sets: sets, default_anim: ab.default_anim, effects: [...new Set(effects)], rec });
  }
  return out;
}

// ------------------------------------------------------------------------------------------------ main
function main() {
  const t0 = Date.now();
  const inv = inventory();
  console.log(`sora_keyblades_pack ${VERSION}: ${inv.keyblades.length} equip data, ${inv.models.size} weapon models, ${inv.effects.size} effects named${inv.item_note ? ' (' + inv.item_note + ')' : ''}`);
  if (args.includes('--list')) {
    for (const k of inv.keyblades) { console.log(`${k.id} ${k.name || '(no item row)'} base ${k.base}`); for (const f of k.forms) console.log(`   ${f.form} ${f.grow_level} wear ${f.wear_form} sets ${f.sora_anim_sets.map((s) => path.basename(s)).join(',')} weapons ${f.weapons.map((w) => `${w.model}@${w.socket}${w.socket_found_on_sora === false ? '(NOT ON SORA)' : ''}`).join(' ')} transform ${f.transform.map((w) => w.model).join(' ')} pawns ${f.accompany_pawns.map((p) => p.model).join(' ')}`); }
    for (const m of inv.models.values()) console.log(`model ${m.model} ${[...m.roles].join('/')} sets ${m.anim_sets.map((s) => path.basename(s)).join(',')}`);
    return;
  }
  fs.mkdirSync(OUT, { recursive: true });
  // ---- (DRILL-IDLE) anim_instances.json: every model's anim instance (its anim blueprint's default clip and how KH3 plays it:
  // UTresAnimInstance 0x14056ECE0 plays m_DefaultAnimName when the weapon's set has no row of Sora's name, looped when
  // m_bDefaultAnimAlwaysLoopPlay, kept running when m_bDefaultAnimCheckSame). Read by kh-er-render keyblade.rs and kh-host
  // vfx/kbweapon.rs; a CDO read only, so this phase is cheap.
  if (want('anim_instances')) {
    const rows = Object.fromEntries([...inv.models.values()].sort((a, b) => a.model.localeCompare(b.model, 'en')).map((m) => [m.model, m.anim_instance || { anim_class: m.anim_class ? stripObj(m.anim_class) : null, default_anim: null, default_anim_loop: false, default_anim_check_same: false, stop_if_asset_none: false }]));
    put(P(OUT, 'anim_instances.json'), J({ format: 'kh3-keyblades-anim-instances/1', generator: `tools/sora_keyblades_pack.js ${VERSION}`, note: 'per weapon model: its anim blueprint CDO fields of UTresAnimInstance (m_DefaultAnimName +0x420, m_bDefaultAnimAlwaysLoopPlay +0x428, m_bDefaultAnimCheckSame +0x42C, m_bAnimStopIfAnimAssetIsNone +0x430), read by the play by name 0x14056ECE0', models: rows }));
    console.log(`  anim_instances.json: ${Object.keys(rows).length} model(s), ${Object.values(rows).filter((r) => r.default_anim_loop).length} looping default(s)`);
  }
  const prevManifest = readJsonIf(P(OUT, 'manifest.json'));
  // ---- models
  const models = [];
  if (want('models') || want('materials') || want('manifest') || want('audio')) {
    for (const m of [...inv.models.values()].sort((a, b) => a.model.localeCompare(b.model, 'en'))) {
      let r;
      try { r = packModel(m.model, m.mesh, m.anim_sets, m.default_anim, OUT, `tools/sora_keyblades_pack.js ${VERSION}`, [...m.roles].sort().join('+')); } catch (e) { r = { model: m.model, error: e.message }; }
      r.glbFile = r.error ? null : P(OUT, m.model, r.glb);
      models.push(r);
      if (want('models')) console.log(`  ${m.model.padEnd(8)} ${r.error ? 'ERROR ' + r.error : `${r.unchanged ? 'unchanged' : 'written  '} ${r.stats.vertices} v ${r.stats.triangles} t, ${r.bones} bones, ${r.materials.length} slot(s), ${r.clips.filter((c) => c.file).length} clips${r.clips.some((c) => c.error) ? ` (${r.clips.filter((c) => c.error).length} not packed)` : ''}, ${(r.bytes / 1024).toFixed(0)} KB`}`);
    }
  }
  // ---- materials
  let materials = prevManifest ? prevManifest.materials : null;
  if (want('materials')) materials = buildMaterials(models.filter((m) => !m.error), OUT, 'kb:', KB_ID_BASE, 'keyblades');
  // ---- Sora's form clips (their AttachEffect notifies name effects: added to the effect list)
  let forms = prevManifest ? prevManifest.sora_forms : null; let formItems = null;
  if (want('forms')) { const f = buildForms(inv); formItems = f.items; forms = { manifest: 'sora_forms_manifest.json', anim_sets: f.sets, clips: f.clips, bytes: f.bytes, blend_spaces: f.blend_spaces, not_packed: f.skipped.filter((x) => x.error), already_packed: f.skipped.filter((x) => x.in).length }; }
  else { const fm = readJsonIf(P(OUT, 'sora_forms_manifest.json')); if (fm) formItems = fm.items; }
  for (const it of formItems || []) for (const n of it.notifies || []) for (const e of effectsIn(n.properties)) { if (!inv.effects.has(e)) inv.effects.set(e, new Set()); inv.effects.get(e).add(`Sora form clip ${it.animation_name} ${n.class}`); }
  for (const m of models) for (const c of m.clips || []) for (const n of c.notifies || []) for (const e of effectsIn(n.properties)) { if (!inv.effects.has(e)) inv.effects.set(e, new Set()); inv.effects.get(e).add(`${m.model} clip ${c.sequence} ${n.class}`); }
  // ---- the Moogle (before the effects: its shop effects join the list)
  let npcs = prevManifest ? prevManifest.npcs : null;
  if (want('npc')) {
    const list = buildNpcs(inv);
    for (const n of list) if (n.rec && !n.rec.error) console.log(`  npc ${n.npc} (${path.basename(n.shop)}, ${short(n.native_class)}): ${n.rec.unchanged ? 'unchanged' : 'written'} ${n.rec.stats.vertices} v, ${n.rec.bones} bones, ${n.rec.clips.filter((c) => c.file).length} clips, ${n.effects.length} effects`);
    const npcModels = list.filter((n) => n.rec && !n.rec.error).map((n) => ({ ...n.rec, glbFile: P(NPC_OUT, n.npc, n.rec.glb) }));
    let npcMat = null;
    if (npcModels.length) { fs.mkdirSync(P(NPC_OUT, 'shop'), { recursive: true }); npcMat = buildMaterials(npcModels, P(NPC_OUT, 'shop'), 'npc:', NPC_ID_BASE, 'Moogle shop NPCs'); }
    npcs = { shops: list.map((n) => ({ shop: n.shop, native_class: n.native_class, npc: n.npc, dir: n.npc ? `../npc/${n.npc}` : null, mesh: n.mesh, anim_class: n.anim_class, anim_sets: n.anim_sets, default_anim: n.default_anim, clips: n.rec && n.rec.clips ? n.rec.clips.map((c) => ({ sequence: c.sequence, file: c.file || null, logical_names: c.logical_names, duration_s: c.duration_s ?? null, error: c.error || null })) : [], effects: n.effects, error: n.error || (n.rec && n.rec.error) || null, bytes: n.rec ? n.rec.bytes : 0 })),
      materials: npcMat ? { dir: '../npc/shop/materials', report: npcMat.report, id_base: NPC_ID_BASE, mesh_prefix: 'npc:' } : null };
  } else if (prevManifest && prevManifest.npcs) for (const s of prevManifest.npcs.shops) for (const e of s.effects || []) { if (!inv.effects.has(e)) inv.effects.set(e, new Set()); inv.effects.get(e).add('shop'); }
  // ---- effects
  let vfx = prevManifest ? prevManifest.vfx : null;
  if (want('vfx') || want('manifest')) vfx = buildVfx(inv); // (unchanged effects: no run, only where each effect is)
  // ---- audio rows
  let audio = prevManifest ? prevManifest.audio : null;
  if (want('audio')) { const a = buildAudio(inv, models, { items: formItems }); audio = { rows: 'audit/sora_port/audio/keyblade_rows.json', sets: a.sets, entries: a.weapon_rows.length, missing_sounds: a.weapon_rows.filter((r) => r.asset && !r.exists).map((r) => r.asset), effect_se_rows: a.code_rows.length, notify_sounds: a.requests.length }; }
  // ---- manifest
  if (want('manifest')) {
    const modelRec = Object.fromEntries(models.filter((m) => !m.error).map((m) => [m.model, { dir: m.model, glb: `${m.model}/${m.glb}`, skeleton: `${m.model}/skeleton.json`, kind: m.kind, mesh: `kb:${m.model}`, source_mesh: m.source_mesh, bones: m.bones, vertices: m.stats.vertices, triangles: m.stats.triangles, slots: m.materials.map((s) => s.slot_name), textures: m.textures.map((t) => `${m.model}/${t}`),
      anim_sets: m.anim_sets, default_anim: m.default_anim, clips: Object.fromEntries(m.clips.filter((c) => c.file).flatMap((c) => (c.logical_names.length ? c.logical_names : [c.sequence]).map((n) => [n, `${m.model}/${c.file}`]))), clips_not_packed: m.clips.filter((c) => c.error).map((c) => ({ sequence: c.sequence, error: c.error })), rows_not_sequences: (m.rows || []).filter((r) => r.class && r.class !== 'AnimSequence').map((r) => ({ name: r.name, asset: r.asset, class: r.class })),
      bytes: m.bytes, fingerprint: m.fingerprint, blueprints: [...(inv.models.get(m.model) || { blueprints: [] }).blueprints], roles: [...(inv.models.get(m.model) || { roles: [] }).roles] }]));
    const fxOf = (e) => ({ effect: e, root: vfx && vfx.where ? vfx.where[e] || null : null });
    const kbs = Object.fromEntries(inv.keyblades.map((k) => {
      const fx = new Set(); for (const m of new Set(k.forms.flatMap((f) => [...f.weapons, ...f.transform, ...f.accompany_pawns, ...(f.projectiles || [])].map((w) => w.model).filter(Boolean)))) for (const [e, by] of inv.effects) if ([...by].some((b) => b.startsWith(k.id + ' ') || b.startsWith(m + ' '))) fx.add(e);
      const baseBp = k.base ? blueprint([...inv.models.get(k.base).blueprints][0]) : null;
      const dlc = [...fx].some((e) => /\/dlc0\d\//.test(e)) || (audio && audio.sets && k.base && /_DLC/.test(String((audio.sets[k.base] || {}).hit || '')));
      return [k.id, { name: k.name, items: k.items, equip: k.equip, dlc: !!dlc, base_model: k.base, models: [...new Set(k.forms.flatMap((f) => [...f.weapons, ...f.transform, ...f.accompany_pawns, ...(f.projectiles || [])].map((w) => w.model).filter(Boolean)))],
        form_change_charge_effect: k.form_change_charge_effect, forms: k.forms, sounds: k.base && audio && audio.sets ? audio.sets[k.base] || null : null,
        effects: baseBp ? { appear: stripObj(baseBp.cdo.m_AppearEffect), disappear: stripObj(baseBp.cdo.m_DisappearEffect), magic_cast: stripObj(baseBp.cdo.m_CmnMagicCastEffect), bad_states: stripObj(baseBp.cdo.m_CmnBadStatesEffect) } : null,
        all_effects: [...fx].sort().map(fxOf), usable: !!k.base && k.forms.length > 0 }];
    }));
    const party = readJsonIf(P(PACK, 'party', 'party.json'));
    const partyRows = readJsonIf(P(ROOT, 'audit', 'sora_port', 'audio', 'party_rows.json'));
    const partyWeapons = {};
    for (const mem of party ? party.members : []) for (const w of mem.weapons || []) {
      const wm = readJsonIf(P(PACK, 'party', w.dir, 'manifest.json'));
      const rows = partyRows && partyRows.members[mem.npc] ? partyRows.members[mem.npc].weapon_rows.filter((r) => r.weapon === w.model) : [];
      partyWeapons[w.model] = { owner: mem.npc, member: mem.name, item: w.item, item_key: w.item_key, row: w.row, name_ja: wm ? wm.name_ja : null, dir: `../party/${w.dir}`, glb: exists(P(PACK, 'party', w.dir, 'weapon.glb')) ? `../party/${w.dir}/weapon.glb` : null, socket: w.socket, mesh: wm ? wm.mesh : null,
        materials: `../party/${mem.dir}/materials (library of ${mem.name}, meshes of his weapons)`, stats: wm ? wm.stats : null, swing_set: (rows.find((r) => r.kind === 'weapon_swing') || {}).set || null, hit_set: (rows.find((r) => r.kind === 'weapon_hit') || {}).set || null, sound_entries: rows.filter((r) => r.asset).length };
    }
    const folders = new Set();
    const addFolder = (gp) => { if (!gp) return; const c = gamePathToContent(stripObj(gp)); folders.add(`KINGDOM HEARTS III/Content/${path.dirname(c)}`); };
    for (const m of models.filter((x) => !x.error)) { addFolder(m.source_mesh); for (const s of m.materials) if (s.material_instance) { try { for (const c of materialInfo(s.material_instance).chain) addFolder(c); for (const t of Object.values(materialInfo(s.material_instance).tex)) addFolder(t); } catch { /* listed by the model */ } } for (const c of m.clips) addFolder(c.asset); for (const s of m.anim_sets) addFolder(s); for (const b of (inv.models.get(m.model) || { blueprints: [] }).blueprints) addFolder(b); }
    for (const k of inv.keyblades) { addFolder(k.equip); for (const f of k.forms) { for (const s of f.sora_anim_sets) addFolder(s); addFolder(f.projectile_set); addFolder(f.magic_set); addFolder(f.ui_data); } }
    for (const it of formItems || []) addFolder(it.source_asset);
    for (const e of inv.effects.keys()) addFolder(e);
    for (const s of Object.values(audio && audio.sets ? audio.sets : {})) for (const x of [s.swing, s.hit, s.alias]) addFolder(x);
    const ar = readJsonIf(P(ROOT, 'audit', 'sora_port', 'audio', 'keyblade_rows.json'));
    for (const r of ar ? [...ar.weapon_rows, ...ar.code_rows.map((x) => ({ asset: x.sound_asset }))] : []) addFolder(r.asset);
    if (npcs) for (const s of npcs.shops) { addFolder(s.shop); addFolder(s.mesh); for (const x of s.anim_sets || []) addFolder(x); for (const c of s.clips || []) if (c.file) addFolder(`${path.dirname(s.anim_sets[0] || '')}/x`); for (const e of s.effects || []) addFolder(e); }
    const manifest = {
      format: 'kh3-sora-keyblades/1', generator: `tools/sora_keyblades_pack.js ${VERSION}`, legal: 'Converted from the user\'s own copy of the game for local use; nothing in this folder is distributed.',
      note: 'THE KEYBLADES (every TresEquipKeybladeData of Blueprints/Player/p_ex001: base game + Re Mind), their formchanges, Donald\'s and Goofy\'s weapons, the Moogle shop. Query a keyblade by its id (keyblades.w_soNNN): forms[] (TresEquipKeybladeData m_EquipMap: form, grow level, Sora\'s wear form, Sora\'s anim sets, magic / projectile sets, weapons on Sora\'s sockets with the socket transform, transform units, finisher pawns), the models (models.<model>: glb, skeleton, clips by logical name, material slots `kb:<model>` of materials/materials.json), sounds (enumset / setable sets; the entries are triggers weapon.swing_set / weapon.hit_set with weapon = <model> in PACK/audio/manifest.json), effects with the root that holds each (vfx = PACK/vfx, party/vfx, xsi/vfx, keyblades/vfx). Paths are relative to this folder.',
      conventions: sp.CONVENTIONS,
      dlc_found: { re_mind: inv.keyblades.filter((k) => kbs[k.id].dlc).map((k) => k.id), folders: ['Effects/dlc01', 'Effects/dlc02', 'Sound/_DLC'].filter((f) => exists(P(PKG, f))), note: 'Re Mind content is in the installed paks (Effects/dlc01: Oathkeeper w_so200, Oblivion w_so210, w_so220 finisher effects; Sound/_DLC/SE/weapon/02020..02022; Effects/dlc02: Re Mind\'s other characters and enemies)' },
      keyblades: kbs, models: modelRec, materials: materials ? { dir: 'materials', report: materials.report, id_base: KB_ID_BASE, mesh_prefix: 'kb:', shaders: 'materials/er (tools/packgen_shaders.js kb_dirs: Sora\'s vertex layout, kh-shaderc --rebuild over tools/splice_kb_wgsl.js / tools/kb_shader_jobs.js, then --only tex)' } : null,
      sora_forms: forms, audio, vfx: vfx ? { root: 'vfx', effects_in_this_root: vfx.list, where: vfx.where, failures: vfx.failures } : null,
      party_weapons: partyWeapons, npcs, pack_inputs_folders: [...folders].sort(),
      sora_sockets: Object.fromEntries(Object.values(inv.sockets).filter((s) => /wep|buki|ashi2|ude2/i.test(s.name)).map((s) => [s.name, s])),
    };
    put(P(OUT, 'manifest.json'), J(manifest));
    // the picture tests' plans (crates/kh-er-render/examples/kb_pics.rs): every model seen flat; the Moogles from the front
    const nameOf = (model) => { const k = Object.entries(kbs).find(([, v]) => v.models.includes(model)); return k ? `${k[0]} ${k[1].name || ''}`.trim() : ''; };
    fs.mkdirSync(STAGE, { recursive: true });
    fs.writeFileSync(P(STAGE, 'kb_pics_plan.json'), JSON.stringify({ columns: 10, tile: [240, 240], view: 'flat', panels: Object.entries(modelRec).map(([m, r]) => ({ label: `${m} ${r.kind} (${nameOf(m)})`, glb: `build/sora_pack/keyblades/${r.glb}` })) }, null, 1));
    if (npcs) fs.writeFileSync(P(STAGE, 'npc_pics_plan.json'), JSON.stringify({ columns: 2, tile: [360, 420], view: 'front', panels: npcs.shops.filter((s) => s.npc && !s.error).map((s) => ({ label: `${s.npc} (${path.basename(s.shop)})`, glb: `build/sora_pack/npc/${s.npc}/model.glb` })) }, null, 1));
    console.log(`  manifest: ${Object.keys(kbs).length} keyblades, ${Object.keys(modelRec).length} models, ${Object.keys(partyWeapons).length} party weapons, ${folders.size} KH3 folders read`);
  }
  console.log(`sora_keyblades_pack: ${written} file(s) written (${(writtenBytes / 1048576).toFixed(1)} MB), ${kept} unchanged, ${((Date.now() - t0) / 1000).toFixed(0)} s`);
}
if (require.main === module) { try { main(); } catch (e) { console.error(e); process.exit(1); } }
else module.exports = { inventory, blueprint, VERSION };
