// (weapons P2, 2026-11-03) Writes sheets/port/sora-keyblades.json: Sora's 17 Keyblades as the port uses them (the plan's
// documentation/KH3_WEAPONS_PLAN.md 1.1 / 1.3; the KH3 tables: documentation/KH3_WEAPON_DATA.md).
//
// One table, five kinds of row (column `kind`):
//   kb.<KEY>               keyblade  item id, row, name / help ids, icons, mesh / blueprint / equip asset, projectile set,
//                                    base Strength / Magic, abilities, shotlocks, formchange commands, wear forms,
//                                    init level (m_bInitAchieved), max level
//   kb.<KEY>.lv<n>         level     a Keyblade Forge level 1..10: its OWN +Strength / +Magic (m_AttackPlus / m_MagicPlus),
//                                    the added ability, materials and counts, m_bInitAchieved. KH3's rule (WD 3, Forge):
//                                    stats = base + every done row; each done row's ability is added to the weapon.
//   kb.<KEY>.fx.<name>     fx        the weapon blueprint's effects and appear / disappear timings (`asset` / `number`)
//   kb.<KEY>.sound.<name>  sound     its swing / hit sound sets and sound alias
//   kb.<KEY>.anim.<name>   anim      its anim blueprint and anim set
// The fx / sound / anim rows generalise sora-weapon.json's `kingdom_key.*` rows to every Keyblade: the same fields, read
// from each Keyblade's own `w_soNNN_Wep` class default object (chain: native default -> w_so_KeybladeBase -> w_soNNN_Wep;
// the native defaults are sora-weapon.json's `kingdom_key.*` rows whose source is "native default").
//
// THE PAIRING (WD 2): the item keys WEP_KEYBLADE_SO_nn are paired with the ETresItemDefWeapon names through
// kh3-keyblades.weapon_def, never by the number in the name. The Forge costs no munny (WD 3).
//
//   node tools/sora_keyblade_sheet.js          write the sheet
//   node tools/sora_keyblade_sheet.js --check  exit 1 if the sheet on disk differs
const fs = require('fs'), path = require('path');
const VERSION = '1.0.0';
const R = path.resolve(__dirname, '..');
const CONTENT = path.join(R, 'assets_converted/packages/KINGDOM HEARTS III/Content');
const sheet = (name) => JSON.parse(fs.readFileSync(path.join(R, 'sheets/port', name + '.json'), 'utf8'));
const objects = (s) => s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]])));
const kbs = objects(sheet('kh3-keyblades'));
const levels = objects(sheet('kh3-keyblade-levels'));
const forms = objects(sheet('kh3-keyblade-forms'));
const weapon = objects(sheet('sora-weapon'));
const kk = new Map(weapon.filter((r) => r.id.startsWith('kingdom_key.')).map((r) => [r.id.slice('kingdom_key.'.length), r]));

// ------------------------------------------------------------------ class default objects
const pkgFile = (gamePath) => path.join(CONTENT, gamePath.replace(/^\/Game\//, '').replace(/\.[^/]*$/, '') + '.uasset.json');
function loadPackage(gamePath) {
  const file = pkgFile(gamePath);
  if (!fs.existsSync(file)) return null;
  const j = JSON.parse(fs.readFileSync(file, 'utf8'));
  const imp = j.imports;
  // An object reference: an import's package path (the asset), an export by name, or 0 = none.
  const ref = (v) => {
    if (!v) return null;
    if (v > 0) return j.exports[v - 1] ? 'export:' + j.exports[v - 1].objectName : null;
    const i = imp[-v - 1];
    if (!i) return null;
    const outer = i.outerIndex ? imp[-i.outerIndex - 1] : null;
    return outer && outer.className === 'Package' ? outer.objectName : i.objectName;
  };
  const exp = (name) => j.exports.find((e) => e.objectName === name);
  return { file: path.relative(R, file).replace(/\\/g, '/'), ref, exp, cdo: j.exports.find((e) => String(e.objectName).startsWith('Default__')) };
}
const props = (e) => (e && e.data && e.data.props) || [];
const prop = (e, name, index = 0) => props(e).find((p) => p.name === name && (p.arrayIndex || 0) === index);

const base = loadPackage('/Game/Blueprints/Player/p_ex001/w_so_KeybladeBase');
if (!base) throw new Error('sora-keyblades: w_so_KeybladeBase is not decoded');

// The CDO fields of TresWeaponBase the port reads (sora-weapon.json kingdom_key rows of the same name).
// [row name, property, kind: asset | number | bool]
const CDO_FIELDS = [
  ['appear.effect', 'm_AppearEffect', 'asset'],
  ['appear.visible_delay', 'm_AppearVisibleDelayTime', 'number'],
  ['appear.scale_anim_enable', 'm_bIsEnableAppearScaleAnim', 'bool'],
  ['appear.scale_anim_time', 'm_AppearScaleAnimTime', 'number'],
  ['appear.scale_anim_start_scale', 'm_AppearScaleAnimStartScale', 'number'],
  ['appear.dither_enable', 'm_bIsEnableAppearDitherAnim', 'bool'],
  ['appear.dither_delayed', 'm_bIsDelayAppearDitherAnim', 'bool'],
  ['appear.dither_time', 'm_AppearDitherAnimTime', 'number'],
  ['appear.dither_start_value', 'm_AppearDitherAnimStartValue', 'number'],
  ['disappear.effect', 'm_DisappearEffect', 'asset'],
  ['disappear.visible_delay', 'm_DisappearVisibleDelayTime', 'number'],
  ['disappear.scale_anim_enable', 'm_bIsEnableDisappearScaleAnim', 'bool'],
  ['disappear.scale_anim_time', 'm_DisappearScaleAnimTime', 'number'],
  ['disappear.scale_anim_end_scale', 'm_DisappearScaleAnimEndScale', 'number'],
  ['disappear.dither_enable', 'm_bIsEnableDisappearDitherAnim', 'bool'],
  ['disappear.dither_time', 'm_DisappearDitherAnimTime', 'number'],
  ['disappear.dither_end_value', 'm_DisappearDitherAnimEndValue', 'number'],
  ['disappear.with_scale_time', 'm_DisappearWithScaleTime', 'number'],
  ['drain', 'm_pDrainEffect', 'asset'],
  ['magic_cast', 'm_CmnMagicCastEffect', 'asset'],
  ['bad_states', 'm_CmnBadStatesEffect', 'asset'],
];
const SOUND_FIELDS = [['swing_set', 'm_SoundAssets_Swing'], ['hit_set', 'm_SoundAssets_Hit'], ['alias', 'm_SoundAliasAsset']];
// The native default of a field: sora-weapon.json's kingdom_key row when it says "native default", else none / 0 / false.
function nativeDefault(row, kind) {
  const r = kk.get(row);
  if (r && /native default/.test(String(r.source))) return { value: r.value, from: 'native default (sora-weapon.json kingdom_key.' + row + ')' };
  return { value: kind === 'asset' ? null : kind === 'bool' ? false : 0, from: 'native default (not serialized)' };
}

// (P2 hitfx) the columns of the `hit` rows: group (m_GrpName), effects (m_PawnHitEffectPerAttr[0..7]), direct
// (m_DirectHitEffect), map (m_MapHitEffect) + change_map (m_bChangeMapHit), sounds (m_PawnHitSEAsset,
// m_PawnHitSEAssetPerAttr[0..7], m_MapHitSEAsset) + change_se_map (m_bChangeSEMapHit), decal (m_bEnableDecal).
const columns = ['id', 'kind', 'keyblade', 'level', 'item_id', 'row', 'name', 'name_id', 'help_id', 'icon', 'forge_icon', 'mesh', 'blueprint', 'equip', 'projectile_set',
  'strength', 'magic', 'abilities', 'shotlocks', 'forms', 'wear_forms', 'init_level', 'max_level', 'init_achieved', 'ability', 'materials', 'counts', 'asset', 'number',
  'group', 'effects', 'direct', 'map', 'change_map', 'sounds', 'change_se_map', 'decal',
  'source', 'ref_conf', 'note'];
const rows = [];
const blank = { level: 0, item_id: 0, row: '-', name: '-', name_id: '-', help_id: '-', icon: '-', forge_icon: '-', mesh: '-', blueprint: '-', equip: '-', projectile_set: '-',
  strength: 0, magic: 0, abilities: [], shotlocks: [], forms: [], wear_forms: [], init_level: 0, max_level: 0, init_achieved: false, ability: '-', materials: [], counts: [],
  asset: '-', number: 0, group: '-', effects: [], direct: '-', map: '-', change_map: false, sounds: [], change_se_map: false, decal: false, ref_conf: 'confirmed', note: '-' };
const push = (o) => rows.push(columns.map((c) => (o[c] !== undefined ? o[c] : blank[c])));
const assetOf = (p) => (p ? String(p).replace(/\.[^/]*$/, '') : '-');

for (const kb of kbs) {
  const key = kb.id;
  const lv = levels.filter((l) => l.keyblade === key).sort((a, b) => a.level - b.level);
  if (lv.length !== kb.max_level) throw new Error('sora-keyblades: ' + key + ' has ' + lv.length + ' Forge rows, max_level ' + kb.max_level);
  // The init level: the highest m_bInitAchieved row (they are a prefix: KH3 offers levels strictly in order, WD 3).
  const init = lv.filter((l) => l.init_achieved).reduce((m, l) => Math.max(m, l.level), 0);
  if (init !== kb.init_level) throw new Error('sora-keyblades: ' + key + ' init level ' + init + ' != kh3-keyblades.init_level ' + kb.init_level);
  lv.forEach((l, i) => { if (l.init_achieved !== (l.level <= init) || l.level !== i + 1) throw new Error('sora-keyblades: ' + l.id + ' breaks the in-order Forge rule'); });
  const formRows = forms.filter((f) => f.keyblade === key);
  push({ id: 'kb.' + key, kind: 'keyblade', keyblade: key, item_id: kb.item_id, row: kb.row, name: kb.name, name_id: kb.name_id, help_id: kb.help_id,
    icon: assetOf(kb.icon), forge_icon: assetOf(kb.forge_icon), mesh: assetOf(kb.mesh), blueprint: kb.weapon_bp, equip: assetOf(kb.equip), projectile_set: assetOf(kb.projectile_set),
    strength: kb.strength, magic: kb.magic, abilities: kb.abilities, shotlocks: kb.shotlocks, forms: kb.formchanges, wear_forms: kb.wear_forms,
    init_level: kb.init_level, max_level: kb.max_level,
    source: 'sheets/port/kh3-keyblades row ' + key + ' (' + kb.weapon_def + ')', ref_conf: kb.ref_conf,
    note: 'ETresItemDefWeapon ' + kb.weapon_def + ' = ' + kb.weapon_def_value + '; ' + formRows.filter((f) => f.formchange).length + ' formchange rows in kh3-keyblade-forms; obtained at +' + kb.init_level + ' (m_bInitAchieved levels count as done, never saved)' });
  for (const l of lv) {
    push({ id: 'kb.' + key + '.lv' + l.level, kind: 'level', keyblade: key, level: l.level, row: l.row, strength: l.strength_plus, magic: l.magic_plus,
      init_achieved: l.init_achieved, ability: l.ability, materials: l.materials, counts: l.counts,
      source: 'sheets/port/kh3-keyblade-levels row ' + l.id + ' (' + l.row + ')', ref_conf: l.ref_conf,
      note: 'cumulative ' + l.strength + ' / ' + l.magic + '; munny ' + l.munny + ' (the Forge takes none); save flag index ' + l.flag_index });
  }
  // fx / sound / anim from the weapon blueprint.
  const bpPath = kb.weapon_bp.replace(/_Wep$/, '_Wep');
  const wep = loadPackage(bpPath);
  if (!wep) throw new Error('sora-keyblades: ' + key + ' blueprint ' + bpPath + ' is not decoded');
  const field = (name) => {
    for (const [pkg, label] of [[wep, wep.file], [base, base.file]]) {
      const p = prop(pkg.cdo, name);
      if (p) return { p, pkg, from: label + ' (' + pkg.cdo.objectName + ') ' + name };
    }
    return null;
  };
  for (const [row, name, kind] of CDO_FIELDS) {
    const f = field(name);
    let value, from;
    if (f) { value = kind === 'asset' ? f.pkg.ref(f.p.value) : f.p.value; from = f.from; } else ({ value, from } = nativeDefault(row, kind));
    push({ id: 'kb.' + key + '.fx.' + row, kind: 'fx', keyblade: key,
      asset: kind === 'asset' ? (value || '-') : '-', number: kind === 'number' ? Number(value) : kind === 'bool' ? (value ? 1 : 0) : 0,
      source: from, note: kind === 'bool' ? 'flag: number 1 = true, 0 = false' : kind === 'number' ? (/time|delay/.test(row) ? 'seconds' : 'value') : '-' });
  }
  // Hit effects: TresAtkColl0 m_HitEffects group "None" (the default group).
  const coll = wep.exp('TresAtkColl0');
  const groups = (prop(coll, 'm_HitEffects') || { value: { items: [] } }).value.items || [];
  const none = groups.find((g) => (g.find((x) => x.name === 'm_GrpName') || {}).value === 'None');
  const gref = (g, name, index = 0) => { const x = g && g.find((p) => p.name === name && (p.arrayIndex || 0) === index); return x ? wep.ref(x.value) : null; };
  const collSrc = wep.file + ' TresAtkColl0.m_HitEffects["None"].';
  push({ id: 'kb.' + key + '.fx.hit.default', kind: 'fx', keyblade: key, asset: gref(none, 'm_PawnHitEffect') || '-', source: collSrc + 'm_PawnHitEffect' });
  for (let a = 0; a < 8; a++) push({ id: 'kb.' + key + '.fx.hit.attr' + a, kind: 'fx', keyblade: key, asset: gref(none, 'm_PawnHitEffectPerAttr', a) || '-', number: a, source: collSrc + 'm_PawnHitEffectPerAttr[' + a + ']', note: 'number = ETresDamageAttribute value' });
  push({ id: 'kb.' + key + '.fx.hit.direct', kind: 'fx', keyblade: key, asset: gref(none, 'm_DirectHitEffect') || '-', source: collSrc + 'm_DirectHitEffect' });
  const changeMap = none && none.find((p) => p.name === 'm_bChangeMapHit');
  push({ id: 'kb.' + key + '.fx.hit.map', kind: 'fx', keyblade: key, asset: gref(none, 'm_MapHitEffect') || '-', number: changeMap && changeMap.value ? 1 : 0, source: collSrc + 'm_MapHitEffect, m_bChangeMapHit', note: 'number = m_bChangeMapHit (1 = the map hit effect replaces the default)' });
  // The trail: TresEffectAttach0 CommonEffectList key_trail0 (the attach data is w_so_KeybladeBase's, sora-weapon.json kingdom_key.effect.trail).
  const att = wep.exp('TresEffectAttach0');
  const list = (prop(att, 'CommonEffectList') || { value: { items: [] } }).value.items || [];
  const trail = list.find((it) => (it.find((p) => p.name === 'CommonKey') || {}).value === 'key_trail0');
  push({ id: 'kb.' + key + '.fx.trail', kind: 'fx', keyblade: key, asset: trail ? (gref(trail, 'CommonEffect') || '-') : '-', source: wep.file + ' TresEffectAttach0.CommonEffectList[key_trail0].CommonEffect', note: 'attach data, sockets and end rules: w_so_KeybladeBase (sora-weapon.json kingdom_key.effect.trail)' });
  const charge = formRows.find((f) => f.weapon_form === 'TRES_WEAPON_FORM_01' && f.grow === 1);
  push({ id: 'kb.' + key + '.fx.form_change_charge', kind: 'fx', keyblade: key, asset: charge ? assetOf(charge.charge_effect) : '-', source: 'sheets/port/kh3-keyblade-forms ' + (charge ? charge.id : '-') + ' charge_effect (m_CmnFormChangeChargeEffect)' });
  for (const [row, name] of SOUND_FIELDS) {
    const f = field(name);
    push({ id: 'kb.' + key + '.sound.' + row, kind: 'sound', keyblade: key, asset: f ? (f.pkg.ref(f.p.value) || '-') : '-', source: f ? f.from : wep.file + ' (not set)' });
  }
  const mesh = wep.exp('TresWeaponMesh0');
  const animClass = mesh && prop(mesh, 'AnimClass') ? wep.ref(prop(mesh, 'AnimClass').value) : null;
  push({ id: 'kb.' + key + '.anim.blueprint', kind: 'anim', keyblade: key, asset: animClass || '-', source: wep.file + ' TresWeaponMesh0.AnimClass' });
  const animSet = animClass ? animClass.replace(/_Anim$/, '_AnimSet') : null;
  const hasSet = animSet && fs.existsSync(pkgFile(animSet));
  push({ id: 'kb.' + key + '.anim.set', kind: 'anim', keyblade: key, asset: hasSet ? animSet : '-', source: hasSet ? path.relative(R, pkgFile(animSet)).replace(/\\/g, '/') : 'no ' + (animSet || 'anim set') + ' package decoded', ref_conf: hasSet ? 'confirmed' : 'inferred', note: hasSet ? '-' : 'the Keyblade has no own anim set package' });
}
// ------------------------------------------------------------------ (P2 hitfx) the hit-effect entries of every weapon component
// KH3's chooser 0x140628960 (docs/sora-combat-spec.md 8.6, sora-vfx-spec.md 2.6) reads the m_HitEffects of the TresAtkColl0
// component the hit's primitive belongs to: the Keyblade's own blueprint, a form's weapon part (w_so052_Wep ...), or Sora's
// pawn (p_ex001_PawnBase). One row per entry: `hit.<blueprint>.<group>`; `keyblade` = the Keyblade that equips the blueprint
// (base weapon or a form unit), '-' for the pawn; `number` = the component's m_fHitEffectRandomOffsetRadius.
{
  const P_DIR = path.join(CONTENT, 'Blueprints/Player/p_ex001');
  const owner = new Map(); // blueprint (lower case) -> keyblade key
  for (const kb of kbs) owner.set(kb.weapon_bp.split('/').pop().toLowerCase(), kb.id);
  for (const f of forms) for (const u of [...f.units, ...f.transform_units]) {
    const bp = String(u).split('=').pop().split('/').pop().replace(/\.[^.]*$/, '').toLowerCase();
    if (!owner.has(bp)) owner.set(bp, f.keyblade);
  }
  const files = fs.readdirSync(P_DIR).filter((f) => /^(w_so\d+_wep|p_ex001_pawnbase)\.uasset\.json$/i.test(f)).sort();
  // (AUDIT-B F-HAMMER) + the style pawns whose own TresAtkColl0 hits (appended after the weapons: no row moves): Clock Drill's
  // Clock StylePawn/w_so114 (m_HitEffects "None" vw_so114_fns_hit00 / vw_so110_str01, "FinishDR2" vw_so112_jwr_hit0 /
  // vw_so110_str00); keyblade = Classic Tone's (its form's)
  const STYLE_PAWNS = [['w_so114', 'WEP_KEYBLADE_SO_11']];
  for (const [bp, key] of STYLE_PAWNS) {
    files.push({ bp, dir: '/Game/Blueprints/StylePawn/', key });
    owner.set(bp, key);
  }
  let n = 0;
  for (const entry of files) {
    const file = typeof entry === 'string' ? entry : entry.bp + '.uasset.json';
    const bp = file.replace(/\.uasset\.json$/, '');
    const pkg = loadPackage((typeof entry === 'string' ? '/Game/Blueprints/Player/p_ex001/' : entry.dir) + bp);
    const coll = pkg && pkg.exp('TresAtkColl0');
    const list = coll && prop(coll, 'm_HitEffects');
    if (!list) continue;
    const radius = prop(coll, 'm_fHitEffectRandomOffsetRadius');
    const key = bp.toLowerCase() === 'p_ex001_pawnbase' ? '-' : (owner.get(bp.toLowerCase()) || '-');
    for (const it of list.value.items || []) {
      const field = (name, index = 0) => it.find((p) => p.name === name && (p.arrayIndex || 0) === index);
      const obj = (name, index = 0) => { const p = field(name, index); return (p && pkg.ref(p.value)) || '-'; };
      const flag = (name) => Boolean(field(name) && field(name).value);
      const group = String(field('m_GrpName').value);
      const set = field('m_HitEffSet');
      if (set && set.value && set.value.items && set.value.items.length) throw new Error('sora-keyblades: ' + bp + ' group ' + group + ' has an m_HitEffSet: not handled');
      const effects = [0, 1, 2, 3, 4, 5, 6, 7].map((a) => obj('m_PawnHitEffectPerAttr', a));
      const sounds = [obj('m_PawnHitSEAsset'), ...[0, 1, 2, 3, 4, 5, 6, 7].map((a) => obj('m_PawnHitSEAssetPerAttr', a)), obj('m_MapHitSEAsset')];
      push({ id: 'hit.' + bp + '.' + group, kind: 'hit', keyblade: key, blueprint: bp, group, asset: obj('m_PawnHitEffect'), effects, direct: obj('m_DirectHitEffect'),
        map: obj('m_MapHitEffect'), change_map: flag('m_bChangeMapHit'), sounds, change_se_map: flag('m_bChangeSEMapHit'), decal: flag('m_bEnableDecal'),
        number: radius ? Number(radius.value) : 0,
        source: pkg.file + ' TresAtkColl0.m_HitEffects["' + group + '"]', note: 'chooser 0x140628960: effect = map && change_map ? map : effects[attribute] or asset; direct on a phys shape' });
      n++;
    }
  }
  if (n === 0) throw new Error('sora-keyblades: no hit-effect entries');
}
if (rows.length === 0) throw new Error('sora-keyblades: no rows');
const out = JSON.stringify({ sheet: 'sora-keyblades', group: 'port', columns }).replace(/\}$/, ',"rows":[\n') + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n';
const file = path.join(R, 'sheets/port/sora-keyblades.json');
if (process.argv.includes('--check')) {
  const same = fs.existsSync(file) && fs.readFileSync(file, 'utf8') === out;
  console.log(same ? 'sora-keyblades: ' + rows.length + ' rows, current' : 'sora-keyblades: sheet differs from tools/sora_keyblade_sheet.js ' + VERSION);
  process.exit(same ? 0 : 1);
}
fs.writeFileSync(file, out);
console.log('sora-keyblades: ' + rows.length + ' rows written (tools/sora_keyblade_sheet.js ' + VERSION + ')');
