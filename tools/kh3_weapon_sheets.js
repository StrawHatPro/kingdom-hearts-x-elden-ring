// (weapons) KH3's weapon, Keyblade Forge, synthesis, material and Moogle shop tables as port sheets (sheets/port/kh3-*.json).
// Every keyblade (base game + Re Mind), Donald's staves and Goofy's shields, their upgrade rows, form changes and the assets
// they use, the Moogle Workshop (88 recipes, 78 Collector's Goals, 20 Photo Missions), the 60 materials and the enemies that
// drop them, the Moogle / Gummi shop stock, and how KH3 hands each weapon to the player.
//
//   node tools/kh3_weapon_sheets.js     write the 13 sheets and register them (once, additively) in tools/lib/sheet_defs.js
//                                       and tools/lib/port_gen_defs.js; then node tools/extract.js, node tools/preflight.js
//                                       --summary, node tools/gen_port.js
//
// Inputs (read only): sheets/entities/Load/Tres/** (TresItemData, TresEquipItemData, TresAbilityData, TresKeybladeAbilityData,
// ItemSynthesis/*, TresTreasureData*), sheets/entities/Load/Common/{TresCmdData,TresChrInitData}, sheets/entities/DataTable/
// Enemy/Base/* (drops), sheets/reflect/enum-values.json, the decoded packages under assets_converted/packages (equip data,
// weapon / anim / mesh blueprints, projectile sets, wear-form and form-ability sets, UI data assets, the enemy dictionary),
// the English string tables build/sora_pack/ui/strings/en/*.json, and the code / event findings of the 2026-11-02 research
// runs kept in build/sora_tmp/kh3_weapons/{upgrade_shop,synthesis_rules,acquisition}.json (each row carries its own source:
// function addresses of the decompiled exe, level bytecode offsets). Documentation: documentation/KH3_WEAPON_DATA.md.
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const VERSION = '1.0.0';
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const ENT = (f) => readJson(P('sheets', 'entities', f + '.json'));
const EVID = 'build/sora_tmp/kh3_weapons';
const T = 'sheets/entities/Load/Tres/';
const IS = T + 'ItemSynthesis/';

// ------------------------------------------------------------------------------------------------ strings / enums
const STR = new Map();
for (const f of fs.readdirSync(P('build/sora_pack/ui/strings/en'))) {
  const ns = readJson(P('build/sora_pack/ui/strings/en', f)).namespaces;
  for (const n of Object.keys(ns)) for (const k of Object.keys(ns[n])) STR.set(n + '.' + k, ns[n][k]);
}
const txt = (id) => (id && STR.has(id) ? STR.get(id) : '-');
const plain = (s) => String(s).replace(/<br>/g, ' ').replace(/<[^>]*>/g, '').replace(/\s+/g, ' ').trim();
const short = (e) => (e == null ? '-' : String(e).split('::').pop());
const ENUM = new Map(); // enum -> Map(name -> value)
for (const r of readJson(P('sheets/reflect/enum-values.json')).rows) {
  if (!ENUM.has(r[1])) ENUM.set(r[1], new Map());
  ENUM.get(r[1]).set(short(r[2]), r[3]);
}
const enumVal = (en, name) => { const m = ENUM.get(en); const v = m && m.get(short(name)); if (v === undefined) throw new Error(en + ' has no ' + name); return v; };

// ------------------------------------------------------------------------------------------------ packages
const PKG = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
const pkgCache = new Map();
const contentOf = (gamePath) => { const m = /^\/Game\/(.+?)(?:\.[^./]+)?$/.exec(gamePath); if (!m) throw new Error('not a /Game path: ' + gamePath); return m[1]; };
function loadPkg(content) {
  if (pkgCache.has(content)) return pkgCache.get(content);
  const f = path.join(PKG, content + '.uasset.json');
  const d = fs.existsSync(f) ? readJson(f) : null;
  if (d) d._content = content;
  pkgCache.set(content, d);
  return d;
}
function importPath(d, i) { let im = d.imports[-i - 1], s = im.objectName; while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; s = im.objectName + '.' + s; } return s; }
const objPath = (d, i) => (!i ? null : i < 0 ? importPath(d, i) : d.exports[i - 1].objectName);
const isProps = (v) => Array.isArray(v) && v.length > 0 && v.every((e) => e && typeof e === 'object' && !Array.isArray(e) && 'name' in e && 'type' in e);
function plainProps(d, props) {
  const o = {}, st = new Set();
  for (const p of props || []) if (p.arrayIndex !== undefined) st.add(p.name);
  for (const p of props || []) { const v = plainValue(d, p); if (st.has(p.name)) { if (!(p.name in o)) o[p.name] = []; o[p.name][p.arrayIndex || 0] = v; } else o[p.name] = v; }
  return o;
}
function plainValue(d, p) {
  const v = p.value;
  if (p.type === 'ObjectProperty') return objPath(d, v);
  if (p.type === 'ArrayProperty') {
    if (p.innerType === 'ObjectProperty') return v.map((i) => objPath(d, i));
    if (v && Array.isArray(v.items)) return v.items.map((it) => (isProps(it) || (Array.isArray(it) && !it.length) ? plainProps(d, it) : plainAny(d, it)));
    return plainAny(d, v);
  }
  if (p.type === 'MapProperty') return (v.entries || []).map((e) => ({ key: plainAny(d, e.key), value: isProps(e.value) ? plainProps(d, e.value) : p.valueType === 'ObjectProperty' ? objPath(d, e.value) : plainAny(d, e.value) }));
  if (p.type === 'StructProperty') return isProps(v) || (Array.isArray(v) && !v.length) ? plainProps(d, v) : plainAny(d, v);
  return plainAny(d, v);
}
function plainAny(d, v) {
  if (v === null || typeof v !== 'object') return v;
  if (isProps(v)) return plainProps(d, v);
  if (Array.isArray(v)) return v.map((x) => plainAny(d, x));
  if ('s' in v && 'enc' in v) return v.s;
  const o = {}; for (const k of Object.keys(v)) o[k] = plainAny(d, v[k]); return o;
}
const exportProps = (d, pred) => { const e = d.exports.find(pred); return e ? plainProps(d, e.data && e.data.props) : null; };
const cdoProps = (d) => exportProps(d, (e) => /^Default__/.test(e.objectName));
// every '/Game/...' string inside a plain value (soft references and resolved hard references)
function gameRefs(v, out = new Set()) {
  if (typeof v === 'string') { if (/^\/Game\//.test(v)) out.add(v); } else if (Array.isArray(v)) v.forEach((x) => gameRefs(x, out)); else if (v && typeof v === 'object') Object.values(v).forEach((x) => gameRefs(x, out));
  return out;
}
const importsByClass = (d, re) => d.imports.map((im, i) => ({ im, i })).filter((x) => re.test(x.im.className)).map((x) => importPath(d, -(x.i + 1)));
const uniq = (a) => [...new Set(a)];
const sortU = (a) => uniq(a).sort();

// ------------------------------------------------------------------------------------------------ the game's tables
const ITEMS = ENT('Load/Tres/TresItemData').rows;
const EQUIP = new Map(ENT('Load/Tres/TresEquipItemData').rows.map((r) => [r._row_name, r]));
const ABIL = ENT('Load/Tres/TresAbilityData').rows;
const ABIL_BY_KEY = new Map(ABIL.map((r) => [r.m_KeyName, r]));
const abilName = (k) => { const r = ABIL_BY_KEY.get(short(k)); return r ? plain(txt(r.m_NameTextID)) : '-'; };
const itemByKey = new Map(ITEMS.map((r) => [r.m_KeyName, r]));
const itemByRow = new Map(ITEMS.map((r) => [r._row_name, r]));
const itemId = (r) => { const m = /^I(\d\d)(\d\d\d)$/.exec(r._row_name); return (parseInt(m[1], 10) << 8) | parseInt(m[2], 10); };
const itemName = (k) => { const r = itemByKey.get(k); return r ? plain(txt(r.m_NameTextID)) : '-'; };
const KIND = { WEAPON: 'weapon', BATTLEITEM: 'battle', CAMPITEM: 'camp', PROTECTOR: 'armour', ACCESSORY: 'accessory', MATERIAL: 'material', KEYITEM: 'key', FOODSTUFF: 'foodstuff' };
const kindOf = (r) => KIND[short(r.m_Category)] || short(r.m_Category).toLowerCase();
const ENH = ENT('Load/Tres/ItemSynthesis/TresItemWeaponEnhanceData').rows;
const ICONS = ENT('Load/Tres/ItemSynthesis/TresItemWeaponIconData').rows;
const SYN = ENT('Load/Tres/ItemSynthesis/TresItemSynthesisData').rows;
const COL = ENT('Load/Tres/ItemSynthesis/TresItemCollectData').rows;
const MOT = ENT('Load/Tres/ItemSynthesis/TresItemMotifData').rows;
const CMD = ENT('Load/Common/TresCmdData').rows;
const cmdKind = new Map([...ENUM.get('ETresCommandKind')].map(([n, v]) => [v, n.replace(/^TRES_CMD_KIND_/, '')]));
const CMD_BY_NAME = new Map(CMD.map((r) => [cmdKind.get(+r._row_name), r]));
const KBA = (() => { // TresKeybladeAbilityData m_Datas: weapon def -> { shotlocks, wear_forms, forms: Map(weapon form -> text abilities) }
  const o = ENT('Load/Tres/TresKeybladeAbilityData').objects[0];
  const m = new Map();
  for (const e of o.m_Datas.entries) {
    const p = Object.fromEntries(e.value.props.map((x) => [x.tag.name, x.value]));
    const forms = new Map((p.m_FormAbilityMap ? p.m_FormAbilityMap.entries : []).map((f) => [short(f.key), f.value.props.find((x) => x.tag.name === 'm_TextAbilitys').value.items.map(short)]));
    m.set(short(e.key), { shotlocks: (p.m_ShootflowAbilitys ? p.m_ShootflowAbilitys.items : []).map(short), wear: (p.m_WearForms ? p.m_WearForms.items : []).map(short), forms });
  }
  return m;
})();
const evid = (f) => readJson(P(EVID, f + '.json'));
const ACQ = evid('acquisition'), UPG = evid('upgrade_shop'), SYR = evid('synthesis_rules');

// material helpers
const mats = (r) => { const m = [], n = []; for (let i = 0; i < 6; i++) { const k = r['m_Material' + i]; if (k && k !== 'None' && r['m_MatNum' + i] > 0) { m.push(k); n.push(r['m_MatNum' + i]); } } return { m, n }; };
const RANK_GRADE = { NOTHING: 0, RANK03: 1, RANK02: 2, RANK01: 3, RANK00: 4 };

// ------------------------------------------------------------------------------------------------ weapon ids
// ETresItemDefWeapon value v = the item index: row I03<v>, item id 0x300 + v (upgrade-code finding upgrade.rule.listed_weapons /
// item_id_encoding; the 19 TresItemWeaponIconData rows agree: e.g. WEP_KEYBLADE02 (value 2) has keyblade02, row I03002's icon is weapon002).
const defOfIndex = new Map([...ENUM.get('ETresItemDefWeapon')].map(([n, v]) => [v, n]));
const weaponRows = ITEMS.filter((r) => kindOf(r) === 'weapon');
const defOf = (r) => defOfIndex.get(itemId(r) & 0xff);
const SORA = weaponRows.filter((r) => /^WEP_KEYBLADE_SO_/.test(r.m_KeyName));
const PARTY = weaponRows.filter((r) => /^WEP_(DONALD|GOOFY)_/.test(r.m_KeyName));

// ------------------------------------------------------------------------------------------------ sources of each weapon
const WORLD = { BT: 'Scala ad Caelum / Keyblade Graveyard (bt)', BX: 'San Fransokyo', CA: 'The Caribbean', DW: 'Dark World', EW: 'Final World', FZ: 'Arendelle', HE: 'Olympus', KG: 'Keyblade Graveyard', MI: 'Monstropolis', RA: 'Kingdom of Corona', TS: 'Toy Box', TT: 'Twilight Town', PO: '100 Acre Wood', SF: 'Scala ad Caelum', RG: 'Radiant Garden (Re Mind)' };
const CHESTS = [];
for (const f of fs.readdirSync(P(T)).filter((f) => /^TresTreasureData[A-Z]+\.json$/.test(f))) {
  const t = readJson(P(T, f));
  for (const r of t.rows) CHESTS.push({ world: f.replace(/^TresTreasureData|\.json$/g, ''), row: r._row_name, item: r.m_TreasureName, area: short(r.m_WorldAreaCode), file: T + f });
}
const SOURCES = []; // [id, item, how, where, source, evidence, ref_conf, note]
{
  const n = new Map();
  const add = (item, how, where, source, evidence, conf, note) => { const k = (n.get(item) || 0) + 1; n.set(item, k); SOURCES.push([item + '.' + k, item, how, where, source, evidence, conf, note || '-']); };
  for (const r of [...SORA, ...PARTY]) {
    const k = r.m_KeyName;
    for (const a of ACQ.rows.filter((x) => x[0] === k && !/^(chest|shop|synthesis)$/.test(x[1]))) add(k, a[1], a[2], a[3], a[4], a[5], a[6]);
    for (const c of CHESTS.filter((x) => x.item === k)) add(k, 'chest', (WORLD[c.world] || c.world) + ', chest ' + c.row + ' (' + c.area + ')', c.file + ' row ' + c.row + ' m_TreasureName', 'treasure table of the world', 'confirmed');
    if (r.m_ShopLevel > 0) add(k, 'shop', 'Moogle shop from shop level ' + r.m_ShopLevel + ', ' + r.m_BuyPrice + ' munny', T + 'TresItemData.json row ' + r._row_name + ' m_ShopLevel / m_BuyPrice', 'stock rule shop.level.rule of sheets/port/kh3-workshop-rules (item listed when 1 <= m_ShopLevel <= shop level); shop levels raised by the story: rows shop.level.N.when', 'confirmed');
    if (r.m_GumiShopLevel > 0) add(k, 'gummi shop', 'Gummi shop (Huey, Dewey, Louie) from Gummi level ' + r.m_GumiShopLevel, T + 'TresItemData.json row ' + r._row_name + ' m_GumiShopLevel', 'rule shop.gumi.rule', 'confirmed');
    if (r.m_bNeedMogCard) add(k, 'shop (M.O.G. Card)', 'Moogle shop once the M.O.G. Card (KEY_ITEM05) is held', T + 'TresItemData.json row ' + r._row_name + ' m_bNeedMogCard', 'rule shop.mogcard.rule', 'confirmed');
    for (const s of SYN.filter((x) => x.m_RewardItem === k)) add(k, 'synthesis', 'Moogle Workshop recipe ' + s._row_name + ' (' + short(s.m_Condition) + (s.m_ConditionName !== 'None' ? ' ' + s.m_ConditionName : '') + ')', IS + 'TresItemSynthesisData.json row ' + s._row_name, 'materials in sheets/port/kh3-synthesis', 'confirmed');
  }
}
const sourcesOf = (k) => SOURCES.filter((s) => s[1] === k);

// ------------------------------------------------------------------------------------------------ assets of a weapon blueprint
const ASSETS = new Map(); // bp game path (no class suffix) -> row
const AS_COLS = ['id', 'kind', 'used_by', 'parent', 'mesh', 'materials', 'skeleton', 'physics', 'bonamik', 'anim_class', 'anim_sets', 'effects', 'sounds', 'projectiles', 'other', 'source'];
function weaponAsset(bpPath, kind, user) {
  const id = bpPath.replace(/\.[^./]+$/, '');
  if (ASSETS.has(id)) { const r = ASSETS.get(id); if (!r[2].includes(user)) r[2].push(user); return r; }
  const content = contentOf(bpPath), d = loadPkg(content);
  if (!d) { const r = [id, kind, [user], '-', '-', [], '-', '-', '-', '-', [], [], [], [], [], 'package not decoded: assets_converted/packages/.../' + content + '.uasset.json missing']; ASSETS.set(id, r); return r; }
  const cls = d.exports.find((e) => /GeneratedClass$/.test(e.class));
  const parent = cls && cls.superIndex ? objPath(d, cls.superIndex) : '-';
  const all = new Set(); for (const e of d.exports) gameRefs(plainProps(d, e.data && e.data.props), all);
  const mesh = importsByClass(d, /^SkeletalMesh$/)[0] || '-';
  let materials = [], skeleton = '-', physics = '-', bonamik = '-';
  if (mesh !== '-') { const m = loadPkg(contentOf(mesh)); if (m) { materials = sortU(importsByClass(m, /^(MaterialInstanceConstant|Material)$/)); skeleton = importsByClass(m, /^Skeleton$/)[0] || '-'; physics = importsByClass(m, /^PhysicsAsset$/)[0] || '-'; bonamik = importsByClass(m, /^SQEX_BonamikAsset$/)[0] || '-'; } }
  const animClass = importsByClass(d, /^AnimBlueprintGeneratedClass$/)[0] || '-';
  let animSets = [];
  if (animClass !== '-') { const a = loadPkg(contentOf(animClass)); const c = a && cdoProps(a); if (c && c.MyAnimSets) animSets = c.MyAnimSets.filter(Boolean); }
  const effects = sortU([...importsByClass(d, /Particle|Niagara|Effect(?!Attach)|SQEX_ParticleAttachDataAsset/), ...[...all].filter((x) => /^\/Game\/Effects\//.test(x))]);
  const sounds = sortU([...importsByClass(d, /Sound|SEAD/), ...[...all].filter((x) => /^\/Game\/Sound\//.test(x))]);
  const projectiles = sortU([...all].filter((x) => /^\/Game\/Blueprints\/Projectile\//.test(x)));
  const known = new Set([mesh, animClass, ...effects, ...sounds, ...projectiles, parent]);
  const other = sortU([...importsByClass(d, /^(StaticMesh|Texture2D|DataAsset|CurveFloat|SQEX_.*Asset)$/), ...[...all]].filter((x) => !known.has(x) && !/^\/Script\//.test(x) && x !== id && !x.startsWith(id + '.')));
  const r = [id, kind, [user], parent || '-', mesh, materials, skeleton, physics, bonamik, animClass, animSets, effects, sounds, projectiles, other,
    'assets_converted/packages/KINGDOM HEARTS III/Content/' + content + '.uasset.json: imports by class (SkeletalMesh, AnimBlueprintGeneratedClass, effect and sound classes) + every /Game/ reference in the exports\' properties; materials / skeleton / physics / bonamik = imports of the mesh package; anim_sets = MyAnimSets of the anim blueprint\'s class default object'];
  ASSETS.set(id, r);
  return r;
}

// ------------------------------------------------------------------------------------------------ sheet: kh3-keyblades + kh3-keyblade-forms + kh3-keyblade-levels
const WEARSET = (() => { const d = loadPkg('Blueprints/Player/p_ex001/p_ex001_WearformSet'); return exportProps(d, (e) => e.class === 'TresWearformSet').m_Forms; })();
const FORMABIL = (() => { const d = loadPkg('Blueprints/Player/p_ex001/p_ex001_FormAbility'); return new Map(exportProps(d, (e) => e.class === 'TresFormAbilitySet').m_Data.map((x) => [short(x.key), x.value])); })();
const helpForms = (help) => [...String(help).matchAll(/<color\(strong\)>([^<]+)<color>/g)].map((m) => m[1]);
// a formchange command by its English name (LocKey text of the CHANGE_* rows of TresCmdData)
const CHANGE_BY_TEXT = new Map(); for (const [n, r] of CMD_BY_NAME) if (/^CHANGE_/.test(n) && r.LocKey) { const t = txt(r.LocKey); if (t !== '-' && !CHANGE_BY_TEXT.has(t)) CHANGE_BY_TEXT.set(t, n); }
const KB = [], FORMS = [], LEVELS = [];
for (const r of SORA) {
  const k = r.m_KeyName, row = r._row_name, def = short(defOf(r)), e = EQUIP.get(row), kba = KBA.get(def) || { shotlocks: [], wear: [], forms: new Map() };
  const icon = ICONS.find((x) => short(x.m_WeaponID) === def);
  const enh = ENH.filter((x) => short(x.m_WeaponID) === def).sort((a, b) => a.m_WeaponLevel - b.m_WeaponLevel);
  const equipPath = e.m_EquipSubclass || '-';
  const eqd = equipPath !== '-' ? loadPkg(contentOf(equipPath)) : null;
  const eqp = eqd ? exportProps(eqd, (x) => x.class === 'TresEquipKeybladeData') : null;
  const help = txt(r.m_HelpTextID);
  const named = helpForms(help);
  let baseBp = '-', baseProj = '-'; const formCmds = [], formNames = [];
  const changeForms = []; // the weapon forms with a formchange, in m_EquipMap order (= the order the help text names them)
  for (const fe of (eqp && eqp.m_EquipMap) || []) {
    const wf = short(fe.key);
    for (const l of fe.value.m_List) {
      const grow = short(l.m_GrowLevel).replace('TRES_WEAPON_GROW_LVL', ''), owner = short(l.m_OwnerWearForm);
      const isChange = owner !== 'TRES_WEAR_FORM_NORMAL';
      const ui = l.m_UIDataAsset ? l.m_UIDataAsset.replace(/\.[^./]+$/, '') : '-';
      const code = ui !== '-' ? ui.split('/').pop().replace('UIDataAssetWeapon', '') : '-';
      for (const u of [...l.m_Equips, ...l.m_TransformSetData]) if (u.RsrcData) weaponAsset(u.RsrcData, 'sora', k);
      if (wf === 'TRES_WEAPON_FORM_01' && grow === '01') { baseBp = (l.m_Equips[0] && l.m_Equips[0].RsrcData) || '-'; baseProj = l.m_ReplaceProjectileSet || '-'; }
      let cmd = '-', how = '-';
      if (isChange) {
        // the formchange's command: by its UI code (CHANGE_<code>), checked against the Keyblade's help text (it names its formchanges in order)
        const byCode = CMD_BY_NAME.has('CHANGE_' + code) ? 'CHANGE_' + code : null;
        if (!changeForms.includes(wf)) changeForms.push(wf);
        const nameHere = named[Math.min(changeForms.indexOf(wf), named.length - 1)];
        const byName = nameHere ? CHANGE_BY_TEXT.get(nameHere) : null;
        if (byCode && (!byName || byCode === byName)) { cmd = byCode; how = 'command CHANGE_' + code + ' named by the form\'s UI data asset ' + ui + (byName ? '; its LocKey text "' + txt(CMD_BY_NAME.get(cmd).LocKey) + '" is the formchange the Keyblade\'s help text names' : ''); }
        else if (byName) { cmd = byName; how = 'command whose LocKey text is "' + nameHere + '", the formchange the Keyblade\'s help text ' + r.m_HelpTextID + ' names (the UI data asset ' + ui + (byCode ? ' would name ' + byCode : ' names no CHANGE_' + code + ' command') + ')'; }
        if (cmd !== '-' && !formCmds.includes(cmd)) { formCmds.push(cmd); formNames.push(txt(CMD_BY_NAME.get(cmd).LocKey)); }
      }
      const c = cmd !== '-' ? CMD_BY_NAME.get(cmd) : null;
      const code2 = cmd !== '-' ? cmd.replace(/^CHANGE_/, '') : '-';
      const fin = cmd !== '-' && CMD_BY_NAME.has('FINISH_' + code2) ? 'FINISH_' + code2 : '-';
      const wearIdx = enumVal('ETresCharWearForm', owner);
      const wearSet = WEARSET[wearIdx] || {};
      const fab = FORMABIL.get(owner) || { m_Ability: [], m_TextAbility: [] };
      const textAb = kba.forms.get(wf) || [];
      let projectiles = [];
      if (l.m_ReplaceProjectileSet) { const pd = loadPkg(contentOf(l.m_ReplaceProjectileSet)); const pp = pd && exportProps(pd, (x) => x.class === 'TresProjectileSet'); projectiles = ((pp && pp.Projectiles) || []).map((x) => x.ProjectileName + '=' + x.ProjectileData); for (const x of (pp && pp.Projectiles) || []) if (x.ProjectileData) weaponAsset(x.ProjectileData, 'projectile', k); }
      let logo = [];
      if (ui !== '-') { const ud = loadPkg(contentOf(ui)); const up = ud && exportProps(ud, (x) => x.class === 'TresUIDataAsset'); if (up) logo = [...(up.SwfAssets || []), ...(up.SwfRefAssets || [])].filter(Boolean); }
      FORMS.push([k + '.' + wf.replace('TRES_WEAPON_FORM_', 'F') + '.G' + grow, k, wf, enumVal('ETresWeaponForm', wf), +grow, owner, wearSet.m_FormName || '-', isChange, cmd, cmd !== '-' ? txt(c.LocKey) : '-',
        c ? c.Param1 : 0, c ? c.Param3 : 0, c ? c.Param0 : 0, fin, ui, logo,
        l.m_Equips.map((u) => u.UnitName + '@' + short(u.m_AttachPart) + ':' + u.AttachBoneName + '=' + (u.RsrcData || '-').replace(/\.[^./]+$/, '')),
        l.m_TransformSetData.map((u) => u.UnitName + '@' + short(u.m_AttachPart) + ':' + u.AttachBoneName + '=' + (u.RsrcData || '-').replace(/\.[^./]+$/, '')),
        l.m_ReplaceAnimSets.filter(Boolean), l.m_ReplaceMagicSet || '-', l.m_ReplaceProjectileSet || '-', projectiles,
        textAb, textAb.map(abilName), isChange ? (fab.m_Ability || []).map(short) : [], isChange ? (fab.m_TextAbility || []).map(short) : [],
        isChange && wearSet.m_TextureReplaceData ? wearSet.m_TextureReplaceData.map((t) => t.m_ReplaceMaterialName + '=' + t.m_ImageTexture) : [],
        (eqp && eqp.m_CmnFormChangeChargeEffect) || '-',
        'assets_converted/packages/KINGDOM HEARTS III/Content/' + contentOf(equipPath) + '.uasset.json (TresEquipKeybladeData m_EquipMap[' + wf + '].m_List[grow ' + grow + ']); ' + (c ? 'sheets/entities/Load/Common/TresCmdData.json row ' + c._row_name + ' (' + cmd + ': Param1 form level, Param3 form time s, Param0 command lifetime s; docs/sora-combat-spec.md 10.1)' : 'no formchange command') + '; ' + T + 'TresKeybladeAbilityData.json ' + def + ' m_FormAbilityMap[' + wf + ']; Blueprints/Player/p_ex001/p_ex001_WearformSet m_Forms[' + wearIdx + '], p_ex001_FormAbility m_Data[' + owner + ']',
        cmd === '-' ? 'confirmed' : how.startsWith('command CHANGE_') ? 'confirmed' : 'inferred', isChange ? how : 'the Keyblade itself (no formchange)']);
    }
  }
  const baseAsset = baseBp !== '-' ? ASSETS.get(baseBp.replace(/\.[^./]+$/, '')) : null;
  let atk = e.m_AttackPlus, mag = e.m_MagicPlus; const abil = (e.m_AppendAbility.items || []).map(short);
  for (const x of enh) {
    atk += x.m_AttackPlus; mag += x.m_MagicPlus;
    const { m, n } = mats(x);
    LEVELS.push([k + '.lv' + x.m_WeaponLevel, k, x.m_WeaponLevel, x._row_name, x.m_FlagIndex, !!x.m_bInitAchieved, x.m_AttackPlus, x.m_MagicPlus, atk, mag, short(x.m_AppendAbility) === 'NONE' ? '-' : short(x.m_AppendAbility), short(x.m_AppendAbility) === 'NONE' ? '-' : abilName(x.m_AppendAbility),
      m, n, m.map(itemName), 0, IS + 'TresItemWeaponEnhanceData.json row ' + x._row_name + ' (' + short(x.m_WeaponID) + ' = ' + k + '); strength / magic = TresEquipItemData ' + row + ' base + the sum of every level up to this one; munny 0: kh3-workshop-rules upgrade.munny.*', 'confirmed']);
  }
  const init = Math.max(0, ...enh.filter((x) => x.m_bInitAchieved).map((x) => x.m_WeaponLevel));
  const src = sourcesOf(k);
  KB.push([k, itemId(r), row, def, enumVal('ETresItemDefWeapon', def), plain(txt(r.m_NameTextID)), r.m_NameTextID, plain(help), r.m_HelpTextID, e.m_AttackPlus, e.m_MagicPlus, abil, abil.map(abilName),
    Math.max(0, ...enh.map((x) => x.m_WeaponLevel)), init, kba.shotlocks, kba.shotlocks.map(abilName), kba.wear, formCmds, formNames,
    r.m_IconAsset || '-', icon ? icon.KeybladeIconAsset : '-', icon ? icon.PickerIconAsset : '-', equipPath, baseBp.replace(/\.[^./]+$/, ''), baseAsset ? baseAsset[4] : '-', baseProj, r.m_MaxNum, !!r.m_bCanSell,
    uniq(src.map((s) => s[2])), src.map((s) => s[3]),
    T + 'TresItemData.json / TresEquipItemData.json row ' + row + '; ' + IS + 'TresItemWeaponEnhanceData.json (' + def + ': ' + enh.length + ' levels) and TresItemWeaponIconData.json; ' + T + 'TresKeybladeAbilityData.json ' + def + '; equip data ' + equipPath + '; acquisition: sheets/port/kh3-weapon-sources rows ' + k + '.*',
    'confirmed', def === 'WEP_KEYBLADE00' ? 'start weapon' : '-']);
}

// ------------------------------------------------------------------------------------------------ sheet: kh3-party-weapons
const PW = [];
for (const r of PARTY) {
  const k = r.m_KeyName, row = r._row_name, def = short(defOf(r)), e = EQUIP.get(row), abil = (e.m_AppendAbility.items || []).map(short);
  const equipPath = e.m_EquipSubclass || '-';
  const eqd = equipPath !== '-' ? loadPkg(contentOf(equipPath)) : null;
  const bps = []; if (eqd) for (const x of eqd.exports) { const pr = plainProps(eqd, x.data && x.data.props); (function walk(v) { if (Array.isArray(v)) v.forEach(walk); else if (v && typeof v === 'object') for (const [kk, vv] of Object.entries(v)) { if (kk === 'RsrcData' && typeof vv === 'string') bps.push(vv); else walk(vv); } })(pr); }
  const chara = /DONALD/.test(k) ? 'donald' : 'goofy';
  for (const b of bps) weaponAsset(b, chara, k);
  const a = bps[0] ? ASSETS.get(bps[0].replace(/\.[^./]+$/, '')) : null;
  const src = sourcesOf(k);
  PW.push([k, itemId(r), row, def, chara, plain(txt(r.m_NameTextID)), plain(txt(r.m_HelpTextID)), e.m_AttackPlus, e.m_MagicPlus, e.m_DefensePlus, abil, abil.map(abilName), r.m_BuyPrice, r.m_SellPrice, !!r.m_bCanSell, r.m_MaxNum, r.m_ShopLevel, r.m_GumiShopLevel,
    r.m_IconAsset || '-', equipPath, uniq(bps.map((b) => b.replace(/\.[^./]+$/, ''))), a ? a[4] : '-', uniq(src.map((s) => s[2])), src.map((s) => s[3]),
    T + 'TresItemData.json / TresEquipItemData.json row ' + row + '; equip data ' + equipPath + ' (RsrcData of its units); acquisition: sheets/port/kh3-weapon-sources rows ' + k + '.*', 'confirmed', '-']);
}

// ------------------------------------------------------------------------------------------------ sheets: kh3-synthesis, kh3-collect-goals, kh3-photo-missions
const SY = SYN.map((s) => { const { m, n } = mats(s); const it = itemByKey.get(s.m_RewardItem);
  return [s._row_name, s.m_FlagIndex, s.m_RewardItem, itemName(s.m_RewardItem), it ? kindOf(it) : '-', it ? itemId(it) : -1, it ? it.m_MaxNum : 0, short(s.m_Condition).replace('TISC_', ''), s.m_ConditionName === 'None' ? '-' : s.m_ConditionName, m, n, m.map(itemName),
    IS + 'TresItemSynthesisData.json row ' + s._row_name, 'confirmed', '-']; });
const matKey = (e) => (short(e) === 'NOTHING' ? '-' : 'MAT_' + short(e)); // ETresItemDefMaterial ITEMnn = item MAT_ITEMnn (id 0x700 + nn; synthesis finding synth.collect.material_ids)
const goalText = (c) => { const n = short(c.m_Condition); const r = short(c.m_ConditionR);
  if (n === 'TICC_MAT_KIND_NUM') return c.m_ConditionN >= 60 ? 'kh3_menu.synthesis.collection_0020' : 'kh3_menu.synthesis.collection_0010';
  if (n === 'TICC_MAT_TOTAL_NUM') return 'kh3_menu.synthesis.collection_0030';
  if (n === 'TICC_MAT_SPKIND_NUM') return 'kh3_menu.synthesis.collection_0040';
  if (n === 'TICC_MAT_GET_RANK') return 'kh3_menu.synthesis.collection_00' + (90 - 10 * enumVal('ETresItemRank', r));
  return c.m_HelptTextID || '-'; };
const CG = COL.map((c) => { const gt = goalText(c);
  return [c._row_name, c.m_FlagIndex, short(c.m_Condition).replace('TICC_', ''), c.m_ConditionN, matKey(c.m_ConditionX), matKey(c.m_ConditionY), short(c.m_ConditionR), short(c.m_RewardKind).replace('TICR_', ''), c.m_RewardItem === 'None' ? '-' : c.m_RewardItem,
    c.m_RewardItem === 'None' ? '-' : itemName(c.m_RewardItem), gt, gt !== '-' && STR.has(gt) ? plain(txt(gt)) : '-',
    SYN.filter((s) => short(s.m_Condition) === 'TISC_COLLECT_LIST' && s.m_ConditionName === c._row_name).map((s) => s._row_name),
    short(c.m_RewardKind) === 'TICR_ENABLE_BUY' ? ITEMS.filter((i) => i.m_UnlockFlagIndex === c.m_FlagIndex).map((i) => i.m_KeyName) : [],
    IS + 'TresItemCollectData.json row ' + c._row_name + '; goal text by condition: kh3-workshop-rules synth.collect.* (decompiled C)', 'confirmed', '-']; });
const PM = MOT.map((m) => [m._row_name, m.m_FlagIndex, plain(txt(m.m_NameTextID)), m.m_NameTextID, plain(txt(m.m_HelptTextID)), m.m_HelptTextID, m.m_GameFlagName, m.m_GameFlagLabel, !!m.m_bSevenPrincessFlag, m.m_PrevMotifName === 'None' ? '-' : m.m_PrevMotifName, m.m_UIPriority,
  SYN.filter((s) => short(s.m_Condition) === 'TISC_MOTIF_LIST' && s.m_ConditionName === m._row_name).map((s) => s._row_name), IS + 'TresItemMotifData.json row ' + m._row_name, 'confirmed', (m.m_Comment && typeof m.m_Comment === 'object' ? m.m_Comment.s : m.m_Comment) || '-']);

// ------------------------------------------------------------------------------------------------ sheets: kh3-materials, kh3-material-drops
const DICT = (() => { // enemy code (e_ex001) -> English name, from the Gummiphone enemy dictionary (MainEnemyID / SubEnemyIDs)
  const d = loadPkg('UI/Data/MobilePortal/DictionaryEnemyData'); const m = new Map();
  for (const row of d.exports[0].data.native.rows) { const p = plainProps(d, row.props); const name = plain(txt(p.NameTextID));
    for (const u of [p.MainEnemyID, ...(p.SubEnemyIDs || [])]) if (u) { const code = 'e_' + short(u).replace('TRES_ENEMY_UID_', '').toLowerCase(); if (!m.has(code)) m.set(code, { name, row: row.name }); } }
  return m; })();
const dropVal = new Map([...ENUM.get('ETresDropItemID')]);
const dropItem = (e) => { const n = short(e); const m = /^ITEM_Material(\d\d)$/.exec(n); if (m) return 'MAT_ITEM' + m[1]; return n; };
const DROPS = [];
const DROP_DIR = 'sheets/entities/DataTable/Enemy/Base';
for (const f of fs.readdirSync(P(DROP_DIR)).filter((x) => x.endsWith('.json')).sort()) {
  const t = readJson(P(DROP_DIR, f)); const table = f.replace(/\.json$/, ''); const code = table.replace(/_BaseDataTable.*$/, '');
  const en = DICT.get(code);
  for (const r of t.rows) for (const [slot, a, b] of [[1, 'm_DropItemID', 'm_DropItemRate'], [2, 'm_DropItemID2', 'm_DropItemRate2'], [3, 'm_DropItemID3', 'm_DropItemRate3']]) {
    const it = r[a]; if (!it || short(it) === 'None' || short(it) === 'NONE' || !(r[b] > 0)) continue;
    const key = dropItem(it); const isMat = /^MAT_ITEM/.test(key);
    DROPS.push([table + '.' + r._row_name + '.' + slot, table, r._row_name, code, en ? en.name : '-', slot, short(it), dropVal.get(short(it)) ?? -1, isMat ? key : '-', isMat ? itemName(key) : '-', r[b],
      DROP_DIR + '/' + f + ' row ' + r._row_name + ' ' + a + ' / ' + b + '; enemy name: UI/Data/MobilePortal/DictionaryEnemyData ' + (en ? en.row : '(no entry)'),
      isMat ? 'inferred' : 'confirmed', isMat ? 'ETresDropItemID ' + short(it) + ' = material ' + key + ' (ids 23..82 are the 60 materials in order: docs/sora-prize-spec.md, prize man 0x141489120, decompiled C); rate = percent per kill (docs/sora-prize-spec.md)' : 'not a material (heal item / ingredient)']);
  }
}
const LUCKY = ENT('Load/Tres/TresLuckyMarkMilestoneRewardData').rows;
const MATS = ITEMS.filter((r) => kindOf(r) === 'material').map((r) => { const k = r.m_KeyName; const goal = COL.find((c) => short(c.m_RewardKind) === 'TICR_ENABLE_BUY' && c.m_FlagIndex === r.m_UnlockFlagIndex);
  const drops = DROPS.filter((d) => d[8] === k);
  return [k, itemId(r), r._row_name, short(r.m_ItemRank), RANK_GRADE[short(r.m_ItemRank)], plain(txt(r.m_NameTextID)), plain(txt(r.m_HelpTextID)), r.m_IconAsset || '-', r.m_BuyPrice, r.m_SellPrice, !!r.m_bCanSell, r.m_MaxNum, r.m_ShopLevel, r.m_UnlockFlagIndex, goal ? goal._row_name : '-',
    uniq(drops.map((d) => d[3] + (d[4] !== '-' ? ' (' + d[4] + ')' : '') + ' ' + d[2] + ': ' + d[10] + '%')),
    CHESTS.filter((c) => c.item === k).map((c) => (WORLD[c.world] || c.world) + ' ' + c.row), COL.filter((c) => c.m_RewardItem === k && short(c.m_RewardKind) === 'TICR_GET_ITEM').map((c) => c._row_name), SYN.filter((s) => s.m_RewardItem === k).map((s) => s._row_name), LUCKY.filter((x) => x.RewardTreasureName === k).map((x) => 'Lucky Emblems ' + x.MarkCount),
    SYN.filter((s) => mats(s).m.includes(k)).map((s) => s._row_name), uniq(ENH.filter((x) => mats(x).m.includes(k)).map((x) => short(x.m_WeaponID))).length,
    T + 'TresItemData.json row ' + r._row_name + '; buyable after the Collector\'s Goal whose flag index = m_UnlockFlagIndex (TICR_ENABLE_BUY); drops: sheets/port/kh3-material-drops; chests: ' + T + 'TresTreasureData*.json; milestones: ' + T + 'TresLuckyMarkMilestoneRewardData.json; made_by / recipes: ' + IS + 'TresItemSynthesisData.json', 'confirmed', '-']; });

// ------------------------------------------------------------------------------------------------ sheet: kh3-shop-stock
const STOCK = ITEMS.filter((r) => r.m_ShopLevel > 0 || r.m_GumiShopLevel > 0 || r.m_bNeedMogCard || r.m_UnlockFlagIndex >= 0).map((r) => {
  const goal = COL.find((c) => short(c.m_RewardKind) === 'TICR_ENABLE_BUY' && c.m_FlagIndex === r.m_UnlockFlagIndex);
  return [r.m_KeyName, itemId(r), kindOf(r), plain(txt(r.m_NameTextID)), r.m_BuyPrice, r.m_SellPrice, !!r.m_bCanSell, r.m_ShopLevel, r.m_GumiShopLevel, !!r.m_bNeedMogCard, r.m_UnlockFlagIndex, goal ? goal._row_name : '-', !!r.m_bCanbeSoldOut, r.m_MaxNum,
    T + 'TresItemData.json row ' + r._row_name + ' (m_ShopLevel, m_GumiShopLevel, m_bNeedMogCard, m_UnlockFlagIndex, m_bCanbeSoldOut); rules: kh3-workshop-rules shop.*', 'confirmed', '-']; });

// ------------------------------------------------------------------------------------------------ sheet: kh3-workshop-rules (parameter sheet)
const RULES = [];
const seenRule = new Set();
for (const r of [...UPG.rows, ...SYR.rows]) { if (seenRule.has(r[0])) continue; seenRule.add(r[0]); RULES.push([r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7]]); }
RULES.push(['data.counts', [SORA.length, PARTY.filter((r) => /DONALD/.test(r.m_KeyName)).length, PARTY.filter((r) => /GOOFY/.test(r.m_KeyName)).length, ENH.length, SYN.length, COL.length, MOT.length, MATS.length], 'count',
  'sheets/port/kh3-*.json (this tool)', 'Sora keyblades, Donald staves, Goofy shields, upgrade rows, recipes, Collector\'s Goals, Photo Missions, materials', '-', 'confirmed', 'order of the list as in the evidence column']);

// ------------------------------------------------------------------------------------------------ sheet definitions
// [name, sheet_defs type, gen_port type (null = not generated), doc]
const S = (n, t, g, doc) => [n, t, g, doc];
const SHEETS = {
  'kh3-keyblades': { row: 'Kh3KeybladeRow', table: 'KH3_KEYBLADES', prefix: 'KH3_KB', doc: 'One Keyblade of KH3 (base game + Re Mind): item, stats, abilities, Forge levels, shotlocks, formchanges, assets and how KH3 gives it.', rows: KB, cols: [
    S('id', 'str', 'str', 'm_KeyName (WEP_KEYBLADE_SO_00 ..)'), S('item_id', 'int', 'u32', 'item id 0x300 + index'), S('row', 'str', 'str', 'TresItemData row I03nnn'), S('weapon_def', 'str', 'str', 'ETresItemDefWeapon name (the upgrade / ability tables key by it)'), S('weapon_def_value', 'int', 'u32', 'its value = the item index'),
    S('name', 'str', 'str', 'English name'), S('name_id', 'str', 'str', 'm_NameTextID'), S('help', 'str', 'str', 'English description (markup stripped)'), S('help_id', 'str', 'str', 'm_HelpTextID'),
    S('strength', 'int', 'i32', 'm_AttackPlus at level 0'), S('magic', 'int', 'i32', 'm_MagicPlus at level 0'), S('abilities', 'list', 'str[]', 'm_AppendAbility (ETresAbilityKind)'), S('ability_names', 'list', 'str[]', 'English'),
    S('max_level', 'int', 'u32', 'highest m_WeaponLevel of its Forge rows'), S('init_level', 'int', 'u32', 'highest level with m_bInitAchieved (the level it has when obtained)'),
    S('shotlocks', 'list', 'str[]', 'm_ShootflowAbilitys (ETresTextAbilityKind)'), S('shotlock_names', 'list', 'str[]', 'English'), S('wear_forms', 'list', 'str[]', 'm_WearForms (Sora\'s formchange costumes)'),
    S('formchanges', 'list', 'str[]', 'CHANGE_* commands of its formchanges (TresCmdData), see kh3-keyblade-forms'), S('formchange_names', 'list', 'str[]', 'English'),
    S('icon', 'str', 'str', 'm_IconAsset (item icon)'), S('forge_icon', 'str', 'str', 'TresItemWeaponIconData KeybladeIconAsset'), S('picker_icon', 'str', 'str', 'TresItemWeaponIconData PickerIconAsset'),
    S('equip', 'str', 'str', 'TresEquipItemData m_EquipSubclass (TresEquipKeybladeData)'), S('weapon_bp', 'str', 'str', 'the Keyblade\'s own weapon blueprint (form 01, grow 01)'), S('mesh', 'str', 'str', 'its skeletal mesh'), S('projectile_set', 'str', 'str', 'm_ReplaceProjectileSet of form 01 (shotlock projectiles)'),
    S('max_num', 'int', 'u32', 'm_MaxNum'), S('can_sell', 'json', 'bool', 'm_bCanSell'), S('how', 'list', 'str[]', 'how KH3 gives it (kh3-weapon-sources)'), S('where', 'list', 'str[]', 'where / when'),
    S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, '-')] },
  'kh3-keyblade-levels': { row: 'Kh3KeybladeLevelRow', table: 'KH3_KEYBLADE_LEVELS', prefix: 'KH3_KBLV', doc: 'One Keyblade Forge level of KH3 (TresItemWeaponEnhanceData): stat gains, the added ability and the materials it costs (no munny).', rows: LEVELS, cols: [
    S('id', 'str', 'str', '<keyblade>.lv<n>'), S('keyblade', 'str', 'str', 'kh3-keyblades id'), S('level', 'int', 'u32', 'm_WeaponLevel 1..10'), S('row', 'str', 'str', 'IW_n'), S('flag_index', 'int', 'u32', 'm_FlagIndex: the save bit of the level'),
    S('init_achieved', 'json', 'bool', 'm_bInitAchieved: the level is done when the Keyblade is obtained'), S('strength_plus', 'int', 'i32', 'm_AttackPlus'), S('magic_plus', 'int', 'i32', 'm_MagicPlus'),
    S('strength', 'int', 'i32', 'base + every level up to this one'), S('magic', 'int', 'i32', 'base + every level up to this one'), S('ability', 'str', 'str', 'm_AppendAbility or -'), S('ability_name', 'str', 'str', 'English'),
    S('materials', 'list', 'str[]', 'm_Material0..5 (MAT_ITEMnn)'), S('counts', 'list', 'i32[]', 'm_MatNum0..5'), S('material_names', 'list', 'str[]', 'English'), S('munny', 'int', 'u32', 'munny cost (0: the Forge takes none)'),
    S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-')] },
  'kh3-keyblade-forms': { row: 'Kh3KeybladeFormRow', table: 'KH3_KEYBLADE_FORMS', prefix: 'KH3_KBFORM', doc: 'One weapon form (and grow level) of a KH3 Keyblade (TresEquipKeybladeData m_EquipMap): the formchange command, its time and level, finisher, the weapon parts and where they attach, replacement anim / magic / projectile sets, the moves the Abilities screen lists, the form\'s abilities and Sora\'s costume textures.', rows: FORMS, cols: [
    S('id', 'str', 'str', '<keyblade>.F<form>.G<grow>'), S('keyblade', 'str', 'str', 'kh3-keyblades id'), S('weapon_form', 'str', 'str', 'ETresWeaponForm'), S('weapon_form_value', 'int', 'u32', 'its value'), S('grow', 'int', 'u32', 'ETresWeaponGrowLvl 1..3'),
    S('wear_form', 'str', 'str', 'm_OwnerWearForm (ETresCharWearForm; NORMAL = no formchange)'), S('wear_form_name', 'str', 'str', 'p_ex001_WearformSet m_Forms[wear form].m_FormName'), S('formchange', 'json', 'bool', 'a formchange (wear form not NORMAL)'),
    S('command', 'str', 'str', 'the CHANGE_* command (TresCmdData) or -'), S('command_name', 'str', 'str', 'English'), S('form_level', 'int', 'i32', 'command Param1 (1 first form, 2 second-stage form)'), S('form_time', 'int', 'i32', 'command Param3, s'), S('command_time', 'int', 'i32', 'command Param0: s the formchange command stays offered'),
    S('finisher', 'str', 'str', 'FINISH_<code> command if TresCmdData has one, else -'), S('ui_asset', 'str', 'str', 'm_UIDataAsset'), S('act_logo', 'list', 'str[]', 'its Scaleform movies (the formchange banner)'),
    S('units', 'list', 'str[]', 'm_Equips: unit@part:socket=weapon blueprint'), S('transform_units', 'list', 'str[]', 'm_TransformSetData (the Keyblade\'s transform part)'), S('anim_sets', 'list', 'str[]', 'm_ReplaceAnimSets'), S('magic_set', 'str', 'str', 'm_ReplaceMagicSet'),
    S('projectile_set', 'str', 'str', 'm_ReplaceProjectileSet'), S('projectiles', 'list', 'str[]', 'its Projectiles (name=blueprint)'), S('moves', 'list', 'str[]', 'TresKeybladeAbilityData m_FormAbilityMap[form] (ETresTextAbilityKind)'), S('move_names', 'list', 'str[]', 'English'),
    S('form_abilities', 'list', 'str[]', 'p_ex001_FormAbility m_Ability of the wear form'), S('form_text_abilities', 'list', 'str[]', 'p_ex001_FormAbility m_TextAbility'), S('wear_textures', 'list', 'str[]', 'p_ex001_WearformSet texture replacements (material=texture)'),
    S('charge_effect', 'str', 'str', 'm_CmnFormChangeChargeEffect'), S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, '-')] },
  'kh3-weapon-assets': { row: 'Kh3WeaponAssetRow', table: 'KH3_WEAPON_ASSETS', prefix: 'KH3_WASSET', doc: 'One weapon (or shotlock projectile) blueprint of KH3 that a Keyblade, staff or shield uses, with the meshes, materials, anim sets, effects and sounds it references.', rows: null, cols: [
    S('id', 'str', 'str', 'blueprint path'), S('kind', 'enum:sora|donald|goofy|projectile', 'str', 'whose'), S('used_by', 'list', 'str[]', 'weapon ids that use it'), S('parent', 'str', 'str', 'parent class'), S('mesh', 'str', 'str', 'SkeletalMesh'),
    S('materials', 'list', 'str[]', 'material instances of the mesh'), S('skeleton', 'str', 'str', '-'), S('physics', 'str', 'str', 'PhysicsAsset'), S('bonamik', 'str', 'str', 'SQEX_BonamikAsset (chain)'), S('anim_class', 'str', 'str', 'anim blueprint'),
    S('anim_sets', 'list', 'str[]', 'MyAnimSets of the anim blueprint'), S('effects', 'list', 'str[]', 'particle systems / attach data'), S('sounds', 'list', 'str[]', 'sound sets'), S('projectiles', 'list', 'str[]', 'projectile blueprints it names'), S('other', 'list', 'str[]', 'other references'),
    S('source', 'str', null, '-')] },
  'kh3-party-weapons': { row: 'Kh3PartyWeaponRow', table: 'KH3_PARTY_WEAPONS', prefix: 'KH3_PW', doc: 'One of Donald\'s staves or Goofy\'s shields: stats, abilities, prices, shop levels, assets and how KH3 gives it.', rows: PW, cols: [
    S('id', 'str', 'str', 'm_KeyName'), S('item_id', 'int', 'u32', '-'), S('row', 'str', 'str', '-'), S('weapon_def', 'str', 'str', 'ETresItemDefWeapon'), S('chara', 'enum:donald|goofy', 'str', '-'), S('name', 'str', 'str', '-'), S('help', 'str', 'str', '-'),
    S('strength', 'int', 'i32', 'm_AttackPlus'), S('magic', 'int', 'i32', 'm_MagicPlus'), S('defense', 'int', 'i32', 'm_DefensePlus'), S('abilities', 'list', 'str[]', 'm_AppendAbility'), S('ability_names', 'list', 'str[]', '-'),
    S('buy', 'int', 'u32', 'm_BuyPrice'), S('sell', 'int', 'u32', 'm_SellPrice'), S('can_sell', 'json', 'bool', '-'), S('max_num', 'int', 'u32', '-'), S('shop_level', 'int', 'u32', 'm_ShopLevel (0 = not sold)'), S('gumi_level', 'int', 'u32', 'm_GumiShopLevel'),
    S('icon', 'str', 'str', '-'), S('equip', 'str', 'str', 'm_EquipSubclass'), S('weapon_bps', 'list', 'str[]', '-'), S('mesh', 'str', 'str', '-'), S('how', 'list', 'str[]', '-'), S('where', 'list', 'str[]', '-'),
    S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, '-')] },
  'kh3-synthesis': { row: 'Kh3SynthesisRow', table: 'KH3_SYNTHESIS', prefix: 'KH3_SYN', doc: 'One recipe of the Moogle Workshop (TresItemSynthesisData): the item, its unlock condition and materials.', rows: SY, cols: [
    S('id', 'str', 'str', 'IS_n'), S('flag_index', 'int', 'u32', 'm_FlagIndex (save bits: made / seen / count)'), S('reward', 'str', 'str', 'm_RewardItem'), S('reward_name', 'str', 'str', '-'), S('reward_kind', 'str', 'str', 'item category'), S('reward_item_id', 'int', 'i32', '-'), S('reward_max', 'int', 'u32', 'm_MaxNum (carry cap)'),
    S('condition', 'enum:NONE|COLLECT_LIST|MOTIF_LIST', 'str', 'ETresItemSynthesisCondition'), S('condition_row', 'str', 'str', 'IC_n / IM_n or -'), S('materials', 'list', 'str[]', '-'), S('counts', 'list', 'i32[]', '-'), S('material_names', 'list', 'str[]', '-'),
    S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, '-')] },
  'kh3-collect-goals': { row: 'Kh3CollectGoalRow', table: 'KH3_COLLECT_GOALS', prefix: 'KH3_GOAL', doc: 'One Collector\'s Goal of the Moogle Workshop (TresItemCollectData): the condition, its reward, and what it unlocks.', rows: CG, cols: [
    S('id', 'str', 'str', 'IC_n'), S('flag_index', 'int', 'u32', '-'), S('condition', 'enum:MAT_KIND_NUM|MAT_TOTAL_NUM|MAT_KIND_ALL|MAT_SPKIND_NUM|MAT_GET_RANK', 'str', 'ETresItemCollectCondition'), S('n', 'int', 'u32', 'm_ConditionN'),
    S('x', 'str', 'str', 'm_ConditionX as MAT_ITEMnn or -'), S('y', 'str', 'str', 'm_ConditionY'), S('rank', 'str', 'str', 'm_ConditionR (ETresItemRank)'), S('reward_kind', 'enum:GET_ITEM|ENABLE_SYNTHESIS|ENABLE_BUY', 'str', '-'), S('reward_item', 'str', 'str', '-'), S('reward_name', 'str', 'str', '-'),
    S('text_id', 'str', 'str', 'the goal\'s list text'), S('text', 'str', 'str', 'English template ({0} = n / material)'), S('unlocks_recipes', 'list', 'str[]', 'recipes with this condition row'), S('unlocks_buy', 'list', 'str[]', 'items whose m_UnlockFlagIndex = this flag index (ENABLE_BUY)'),
    S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, '-')] },
  'kh3-photo-missions': { row: 'Kh3PhotoMissionRow', table: 'KH3_PHOTO_MISSIONS', prefix: 'KH3_PHOTO', doc: 'One Photo Mission of the Moogle Workshop (TresItemMotifData): its story gate and the recipes it unlocks.', rows: PM, cols: [
    S('id', 'str', 'str', 'IM_n'), S('flag_index', 'int', 'u32', '-'), S('name', 'str', 'str', '-'), S('name_id', 'str', 'str', '-'), S('help', 'str', 'str', '-'), S('help_id', 'str', 'str', '-'), S('flag_name', 'str', 'str', 'm_GameFlagName (listed when this flag >= the label)'),
    S('flag_label', 'str', 'str', 'm_GameFlagLabel'), S('seven_princess', 'json', 'bool', 'm_bSevenPrincessFlag (all 12 teammates in the album)'), S('prev', 'str', 'str', 'm_PrevMotifName or -'), S('ui_priority', 'int', 'i32', '-'), S('unlocks_recipes', 'list', 'str[]', '-'),
    S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, 'm_Comment (Japanese)')] },
  'kh3-materials': { row: 'Kh3MaterialRow', table: 'KH3_MATERIALS', prefix: 'KH3_MAT', doc: 'One of KH3\'s 60 synthesis materials: grade, prices, when it becomes buyable, which enemies drop it, what uses it.', rows: MATS, cols: [
    S('id', 'str', 'str', 'MAT_ITEMnn'), S('item_id', 'int', 'u32', '0x700 + nn'), S('row', 'str', 'str', '-'), S('rank', 'str', 'str', 'ETresItemRank (RANK03 shard .. RANK00 crystal / ores)'), S('grade', 'int', 'u32', '1 shard, 2 stone, 3 gem, 4 crystal / ore'), S('name', 'str', 'str', '-'), S('help', 'str', 'str', '-'), S('icon', 'str', 'str', '-'),
    S('buy', 'int', 'u32', 'm_BuyPrice (once buyable)'), S('sell', 'int', 'u32', 'm_SellPrice'), S('can_sell', 'json', 'bool', '-'), S('max_num', 'int', 'u32', 'carry cap (the Workshop stock holds up to 999)'), S('shop_level', 'int', 'u32', '-'), S('unlock_flag', 'int', 'i32', 'm_UnlockFlagIndex (-1 = never buyable)'),
    S('buy_goal', 'str', 'str', 'the Collector\'s Goal that makes it buyable, or -'), S('drops', 'list', 'str[]', 'enemy (name) variant: rate %'), S('chests', 'list', 'str[]', 'treasure chests holding it (TresTreasureData*)'), S('goal_rewards', 'list', 'str[]', 'Collector\'s Goals that give it (GET_ITEM)'), S('made_by', 'list', 'str[]', 'recipes that produce it'), S('milestones', 'list', 'str[]', 'Lucky Emblem milestone rewards (TresLuckyMarkMilestoneRewardData)'), S('recipes', 'list', 'str[]', 'recipes that use it'), S('forge_keyblades', 'int', 'u32', 'Keyblades whose Forge uses it'),
    S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, '-')] },
  'kh3-material-drops': { row: 'Kh3DropRow', table: 'KH3_DROPS', prefix: 'KH3_DROP', doc: 'One item slot of a KH3 enemy\'s base data table (m_DropItemID / m_DropItemRate): what it drops and how often.', rows: DROPS, cols: [
    S('id', 'str', 'str', '<table>.<row>.<slot>'), S('table', 'str', 'str', 'DataTable/Enemy/Base table'), S('variant', 'str', 'str', 'row name'), S('enemy', 'str', 'str', 'enemy code'), S('enemy_name', 'str', 'str', 'English (enemy dictionary) or -'), S('slot', 'int', 'u32', '1..3'),
    S('drop_id', 'str', 'str', 'ETresDropItemID'), S('drop_value', 'int', 'i32', 'its value'), S('material', 'str', 'str', 'MAT_ITEMnn or -'), S('material_name', 'str', 'str', '-'), S('rate', 'num', 'f32', 'percent per kill'),
    S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, '-')] },
  'kh3-shop-stock': { row: 'Kh3ShopStockRow', table: 'KH3_SHOP_STOCK', prefix: 'KH3_STOCK', doc: 'One item a KH3 shop can sell: Moogle shop level, Gummi shop level, M.O.G. Card, Collector\'s Goal unlock, price, sold-out rule.', rows: STOCK, cols: [
    S('id', 'str', 'str', 'm_KeyName'), S('item_id', 'int', 'u32', '-'), S('kind', 'str', 'str', 'item category'), S('name', 'str', 'str', '-'), S('buy', 'int', 'u32', '-'), S('sell', 'int', 'u32', '-'), S('can_sell', 'json', 'bool', '-'),
    S('shop_level', 'int', 'u32', 'Moogle shop level it appears at (0 = not by level)'), S('gumi_level', 'int', 'u32', 'Gummi shop level (0 = not there)'), S('mog_card', 'json', 'bool', 'stocked once the M.O.G. Card is held'), S('unlock_flag', 'int', 'i32', 'm_UnlockFlagIndex'), S('unlock_goal', 'str', 'str', 'Collector\'s Goal that unlocks it'),
    S('sold_out', 'json', 'bool', 'm_bCanbeSoldOut (hidden once held count >= max)'), S('max_num', 'int', 'u32', '-'), S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, '-')] },
  'kh3-weapon-sources': { row: 'Kh3WeaponSourceRow', table: 'KH3_WEAPON_SOURCES', prefix: 'KH3_WSRC', doc: 'One way KH3 gives a weapon to the player: start, story event, chest, shop, synthesis, minigame, entitlement.', rows: SOURCES, cols: [
    S('id', 'str', 'str', '<item>.<n>'), S('item', 'str', 'str', 'weapon key'), S('how', 'str', 'str', 'start | event | event+moogle | chest | shop | synthesis | minigame | entitlement | event (+entitlement)'), S('where', 'str', 'str', 'world / story point / recipe'),
    S('source', 'str', 'str', 'asset + export / bytecode offset, table row, or function address'), S('evidence', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, '-')] },
};
SHEETS['kh3-weapon-assets'].rows = [...ASSETS.values()].map((r) => AS_COLS.map((c, i) => r[i]));
for (const r of SHEETS['kh3-weapon-assets'].rows) r[2] = sortU(r[2]);

// ------------------------------------------------------------------------------------------------ checks
const must = (c, m) => { if (!c) throw new Error(m); };
must(KB.length === 17, 'expected 17 Sora keyblades, got ' + KB.length);
must(PW.length === 28, 'expected 28 party weapons, got ' + PW.length);
must(LEVELS.length === ENH.length, 'every Forge row is a keyblade level');
must(SY.length === 88 && CG.length === 78 && PM.length === 20 && MATS.length === 60, 'synthesis table sizes');
for (const [name, s] of Object.entries(SHEETS)) for (const r of s.rows) must(r.length === s.cols.length, name + ' row ' + r[0] + ': ' + r.length + ' cells, ' + s.cols.length + ' columns');
for (const r of [...KB, ...PW]) must(sourcesOf(r[0]).length > 0, 'no source for ' + r[0]);
for (const r of RULES) must(r.length === 8 && r[1] !== null && r[1] !== undefined, 'rule row ' + r[0]);

// ------------------------------------------------------------------------------------------------ write
const write = (name, columns, rows) => fs.writeFileSync(P('sheets', 'port', name + '.json'), '{"sheet":"' + name + '","group":"port","columns":' + JSON.stringify(columns) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
for (const [name, s] of Object.entries(SHEETS)) { write(name, s.cols.map((c) => c[0]), s.rows); console.log('sheets/port/' + name + '.json: ' + s.rows.length + ' rows'); }
write('kh3-workshop-rules', ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'], RULES);
console.log('sheets/port/kh3-workshop-rules.json: ' + RULES.length + ' rows');

// ------------------------------------------------------------------------------------------------ register (only the kh3-* entries; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const BY = "'tools/kh3_weapon_sheets.js'";
const NL = '\n';
// Replaces this tool's own entry for a sheet (its "(weapons)" comment line through the end marker) or inserts it at pos(s).
function upsert(s, name, text, end, pos) {
  const at = s.indexOf("'" + name + "': {");
  if (at < 0) { const p = pos(s); return s.slice(0, p) + text + s.slice(p); }
  const c = s.lastIndexOf(NL, at - 1) + 1, cs = s.lastIndexOf(NL, c - 2) + 1;
  must(s.slice(cs, c).includes('(weapons)'), 'the entry of ' + name + ' is not this tool\'s (no "(weapons)" comment above it)');
  const e = s.indexOf(end, at); must(e > at, 'end of the entry of ' + name);
  return s.slice(0, cs) + text + s.slice(e + end.length);
}
const save = (f, before, after, what) => { if (after !== before) { fs.writeFileSync(f, after); console.log('registered the kh3-* sheets in ' + what); } };
{
  const f = P('tools', 'lib', 'sheet_defs.js'); const before = fs.readFileSync(f, 'utf8'); let s = before;
  const pos = (t) => { const at = t.indexOf("  'sora-item-table': {"); must(at >= 0, 'sheet_defs.js: anchor not found'); return t.lastIndexOf(NL, t.lastIndexOf('  //', at)) + 1; };
  const END = '] },' + NL;
  for (const [name, d] of Object.entries(SHEETS)) {
    const text = '  // (weapons) ' + d.doc + ' Written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)' + NL +
      '  ' + q(name) + ": { group: 'port', file: " + q(name + '.json') + ', by: ' + BY + ', from: ' + q('KH3 data tables and decoded packages; see documentation/KH3_WEAPON_DATA.md') + ', external: true, cols: [' + NL +
      '    ' + d.cols.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['source', 'evidence', 'note'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
    s = upsert(s, name, text, END, pos);
  }
  const rules = "  // (weapons) KH3's Moogle shop, Workshop, Keyblade Forge and synthesis RULES (code findings with their function addresses). Written by tools/kh3_weapon_sheets.js" + NL +
    "  'kh3-workshop-rules': { group: 'port', file: 'kh3-workshop-rules.json', by: " + BY + ", from: 'decompiled KH3.exe (re/decomp), level bytecode and UI strings; research rows in build/sora_tmp/kh3_weapons/*.json', external: true, cols: [" + NL +
    "    ['id', K, 'str', '-', 'upgrade.* shop.* synth.* data.*'], ['value', V, 'json', '-', 'a number, string or list'], ['unit', V, 'str', '-', '-'], ['source', V, 'str', '-', '-', true], ['evidence', V, 'str', '-', '-', true], ['ref_addr', V, 'str', '-', 'function address(es)'], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = instruction level or asset; inferred = decompiled C only'], ['note', V, 'str', '-', '-', true]" + END;
  s = upsert(s, 'kh3-workshop-rules', rules, END, pos);
  save(f, before, s, 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8'); let s = before;
  const after = (anchor) => (t) => { const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor ' + anchor.trim()); return at + anchor.length; };
  const END = NL + '    },' + NL;
  for (const [name, d] of Object.entries(SHEETS)) {
    const cols = d.cols.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
    const text = '    // (weapons) ' + name + ': written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)' + NL +
      '    ' + q(name) + ': {' + NL +
      '      file: ' + q(name.replace(/-/g, '_') + '.rs') + ', row: ' + q(d.row) + ', table: ' + q(d.table) + ", key: 'id', key_prefix: " + q(d.prefix) + ',' + NL +
      '      doc: ' + q(d.doc) + ',' + NL +
      '      columns: { ' + cols + ' },' + END;
    s = upsert(s, name, text, END, after('  tables: {' + NL));
  }
  const rules = "    // (weapons) KH3's Moogle shop / Workshop / Keyblade Forge rules: written by tools/kh3_weapon_sheets.js (documentation/KH3_WEAPON_DATA.md)" + NL +
    "    'kh3-workshop-rules': {" + NL +
    "      file: 'kh3_workshop_rules.rs', struct: 'Kh3WorkshopRules', prefix: 'KH3_WORKSHOP', from: 'port/kh3-workshop-rules'," + NL +
    '      doc: ' + q("KH3's Moogle shop, Workshop (synthesis, Collector's Goals, Photo Missions) and Keyblade Forge rules, one row per rule, from the decompiled exe and the level scripts.") + ',' + NL +
    '      informational: {},' + END;
  s = upsert(s, 'kh3-workshop-rules', rules, END, after('  params: {' + NL));
  save(f, before, s, 'tools/lib/port_gen_defs.js');
}
console.log('kh3_weapon_sheets ' + VERSION + ': ' + KB.length + ' keyblades, ' + PW.length + ' party weapons, ' + FORMS.length + ' form rows, ' + ASSETS.size + ' weapon blueprints, ' + SY.length + ' recipes, ' + CG.length + ' goals, ' + PM.length + ' photo missions, ' + MATS.length + ' materials, ' + DROPS.length + ' drop slots, ' + STOCK.length + ' stock items, ' + SOURCES.length + ' sources');
