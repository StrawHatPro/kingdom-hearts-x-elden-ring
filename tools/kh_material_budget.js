// (P10 MATERIALS, 2026-11-03) THE KH3 MATERIAL BUDGET of the Elden Ring port: READ-ONLY (it writes nothing but an optional report).
//
// documentation/KH3_WEAPONS_PLAN.md 3 / 3.1. It reads Elden Ring's regulation (a copy: build/sora_tmp/er/conv/game/reg/regulation.bin)
// and the port's sheets, and answers: can every KH3 synthesis recipe be made once, every Keyblade raised to +10, every
// Collector's Goal reached, from what the loot gate turns Elden Ring's items into?
//
// - THE SUPPLY. Every lot slot of ItemLotParam_map / ItemLotParam_enemy (the enemy rows an NpcParam's chain reaches), every
//   unlimited rune-paid ShopLineupParam line the KH shop resells (the gloveworts, the Smithing Stones: kh-er shopkh.rs), the
//   Collector's Goals' GET_ITEM rewards and the fixed ore placements (sheet kh-ore-placements) are run through THE SAME RULES as
//   kh-er's loot gate (sheet sora-er-items: the r2.* rows, the id.* overrides, the value tier = rarity + source + region depth +
//   sell value, the `chance:` alternatives as probabilities, sources outside `source.usable` giving a synthesis material only).
//   A slot is FIXED (once per game: a flagged lot row or slot, an event award) or REPEATABLE (an unflagged map lot = a gathered
//   node, an enemy's plain drop, a shop line). An enemy's region depth is not in the params: a repeatable enemy drop counts for
//   every depth 0 .. 4 (the grades it can give somewhere); a map lot's depth is read off its id (mAA_BB, m60 tiles, m61 = the DLC).
// - THE NEED. Every recipe of kh3-synthesis once; every Forge level of kh3-keyblade-levels that a Keyblade does not have when
//   obtained (m_bInitAchieved); the Collector's Goals of kh3-collect-goals (KIND_NUM up to IC_24's 58 kinds and IC_25's 60,
//   KIND_ALL ranges, GET_RANK, SPKIND_NUM buy unlocks, TOTAL_NUM). (P6a's kh-workshop.json replaces the WD sheets once it exists.)
// - THE VERDICT per material: REACHABLE when it has a repeatable source, or its fixed supply covers the need. Orichalcum and
//   Orichalcum+ never have a repeatable source (the user, 2026-11-03: "orichalcum must be a fixed item drop (never random)"): only
//   the verified base-game placements of kh-ore-placements count; Elden Ring's own flagged Ancient Dragon stones, the Larval Tears
//   and the goals' Orichalcum are reported as margin only. A FAMILY SHORT of a grade (no repeatable source, fixed supply below the
//   need) is reported with the fix the plan names: its share of the stable hash weighted (an economy rule, not an enemy table).
//
// Usage: node tools/kh_material_budget.js [--reg <regulation.bin>] [--json <report.json>] [--all]
//   --all  every material's line (default: the short ones, the ores and a summary)
// Exit code 1 when a recipe, a Forge level or a goal is not reachable.
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const zlib = require('zlib');

const ROOT = path.resolve(__dirname, '..');
const arg = (name) => { const k = process.argv.indexOf(name); return k > 0 ? process.argv[k + 1] : null; };
const REG = arg('--reg') || path.join(ROOT, 'build/sora_tmp/er/conv/game/reg/regulation.bin');
const ALL = process.argv.includes('--all');
const sheet = (name) => JSON.parse(fs.readFileSync(path.join(ROOT, 'sheets/port', name + '.json'), 'utf8'));
const rowsOf = (s) => new Map(s.rows.map((r) => [r[0], r]));

// ------------------------------------------------------------------------------------------------ the regulation (read only)
// Elden Ring's regulation.bin: AES-256-CBC (the public key of the game's regulation), a DCX (ZSTD or DEFLATE) around a BND4 of
// .param files. The same reading as build/sora_tmp/party/SHOP/goods.js.
const KEY = Buffer.from('99BFFC366A6BC8C6F5827D093602D676C42892A01C207FB024D3AF4E493FEF99', 'hex');
function openRegulation(file) {
  const b = fs.readFileSync(file);
  const dc = crypto.createDecipheriv('aes-256-cbc', KEY, b.subarray(0, 16));
  dc.setAutoPadding(false);
  const dcx = Buffer.concat([dc.update(b.subarray(16)), dc.final()]);
  const csize = dcx.readUInt32BE(0x20);
  const fmt = dcx.toString('latin1', 0x28, 0x2c);
  const body = dcx.subarray(0x4c, 0x4c + csize);
  const bnd = fmt === 'ZSTD' ? zlib.zstdDecompressSync(body) : zlib.inflateSync(body);
  const files = {};
  const n = bnd.readUInt32LE(0x0c);
  const entry = Number(bnd.readBigUInt64LE(0x20));
  for (let i = 0; i < n; i++) {
    const o = 0x40 + i * entry;
    const size = Number(bnd.readBigUInt64LE(o + 8));
    const off = bnd.readUInt32LE(o + 0x18);
    let e = bnd.readUInt32LE(o + 0x20);
    const start = e;
    while (bnd.readUInt16LE(e)) e += 2;
    files[bnd.toString('utf16le', start, e).split('\\').pop()] = bnd.subarray(off, off + size);
  }
  return (name) => {
    const p = files[name];
    if (!p) throw new Error(`regulation: no ${name}`);
    const count = p.readUInt16LE(0x0a);
    const out = new Map();
    for (let i = 0; i < count; i++) {
      const o = 0x40 + i * 0x18;
      out.set(p.readUInt32LE(o), p.subarray(Number(p.readBigUInt64LE(o + 8))));
    }
    return out;
  };
}

// Field offsets of the param rows: the struct layouts of the pinned SDK (fromsoftware-rs eldenring/src/param/generated.rs,
// repr(C)); the same fields kh-er loot.rs `describe` reads.
const F = {
  goods: { ref: 0x04, sell: 0x14, type: 0x3e, consume: [0x48, 7], sort: 0x72, rarity: 0x78 },
  sp: { hpRate: 0x9c, hpPoint: 0xa0, mpRate: 0xa4, mpPoint: 0xa8, soul: 0xec },
  weapon: { sell: 0x20, matSet: 0x5c, magic: 0xca, fire: 0xcc, thunder: 0xce, dark: 0x18c, type: 0x1a6, rarity: 0x240 },
  protector: { sell: 0x20, rarity: 0xd7 },
  accessory: { sell: 0x18, rarity: 0x41 },
  gem: { sell: 0x20, rarity: 0x32 },
  lot: { id: 0x00, cat: 0x20, points: 0x40, slotFlag: 0x60, flag: 0x80, num: 0x8a },
  npc: { lot: 0x30 },
  shop: { id: 0x00, value: 0x04, release: 0x10, qty: 0x14, type: 0x17, cost: 0x18 },
};

// ------------------------------------------------------------------------------------------------ the rules (kh-er lootmap.rs)
const ITEMS = sheet('sora-er-items');
const TABLE = sheet('sora-item-table');
const SHOP = sheet('sora-er-shop');
const ic = (s, name) => s.columns.indexOf(name);
const kh = new Map(TABLE.rows.map((r) => [r[0], { key: r[0], kind: r[ic(TABLE, 'kind')], grade: r[ic(TABLE, 'grade')], family: r[ic(TABLE, 'family')], name: r[ic(TABLE, 'name')] }]));
const val = (id, dflt) => { const r = ITEMS.rows.find((x) => x[0] === id); return r ? r[1] : dflt; };
const families = val('material.families', []);
const byFamilyGrade = new Map();
for (const i of kh.values()) if (i.kind === 'material' && i.family !== '-' && i.grade > 0 && !byFamilyGrade.has(`${i.family}:${i.grade}`)) byFamilyGrade.set(`${i.family}:${i.grade}`, i.key);
const CATS = ['weapon', 'protector', 'accessory', 'goods', 'gem'];
const lotCats = val('lot.categories', {});
const tierCount = val('tier.count', 6);
const rarityPoints = val('tier.rarity_points', [0, 1, 2, 3]);
const sourcePoints = val('tier.source_points', {});
const regionPoints = val('tier.region_points', [0]);
const sellPoints = [...val('tier.sell_points', [])].sort((a, b) => a[0] - b[0]);
const somberPoints = val('tier.somber_points', 0);
const somberSets = val('weapon.somber_material_sets', []);
const wepKinds = new Map();
for (const [k, list] of Object.entries(val('weapon.kinds', {}))) for (const t of list) wepKinds.set(t, k);
const regionAreas = val('region.areas', {});
const regionBoxes = val('region.overworld', []);
const regionDefault = val('region.default', 1);
const usable = val('source.usable', null);
const hiddenTypes = val('goods.hidden_types', []);
const convert = val('progression.convert', 1) !== 0;
const onlineGroups = (SHOP.rows.find((r) => r[0] === 'online.sort_groups') || [0, []])[1];
const keep = new Set((SHOP.rows.find((r) => r[0] === 'required.keep') || [0, []])[1]);
const overrides = new Map(ITEMS.rows.filter((r) => r[0].startsWith('id.')).map((r) => [r[0].slice(3), r[1]]));
const rules = ITEMS.rows.filter((r) => r[0].startsWith('r2.')).map((r) => ({ id: r[0], ...r[1] })).sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));

/** FNV-1a of the table tag and the id (lootmap.rs `stable_hash`): the family of a non-goods item. */
function stableHash(cat, id) {
  let h = 0x811c9dc5;
  const bytes = [CATS.indexOf(cat), id & 0xff, (id >>> 8) & 0xff, (id >>> 16) & 0xff, (id >>> 24) & 0xff];
  for (const b of bytes) h = Math.imul(h ^ b, 0x01000193) >>> 0;
  return h >>> 8;
}
const baseId = (item) => (item.cat === 'weapon' ? Math.floor(item.id / 10000) * 10000 : item.id);
// (P10) the families' shares of the stable pick (row material.weights; 1 each when the row does not name a family)
const familyWeights = families.map((f) => val('material.weights', {})[f] || 1);
const weightTotal = familyWeights.reduce((a, w) => a + w, 0) || 1;
/** The family a `material` give picks for this item (lootmap.rs `family_of`). */
function familyOf(item) {
  const roll = item.cat === 'goods' ? item.id % weightTotal : stableHash(item.cat, baseId(item)) % weightTotal;
  let upto = 0;
  for (let k = 0; k < families.length; k++) { upto += familyWeights[k]; if (roll < upto) return families[k]; }
  return families[families.length - 1];
}

function tierOf(item, source, depth) {
  const rarity = rarityPoints[Math.min(item.rarity, 3)] ?? item.rarity;
  const src = sourcePoints[source] ?? 0;
  const region = regionPoints[Math.min(depth, regionPoints.length - 1)] ?? 0;
  let sell = 0;
  for (const p of sellPoints) if (item.sell >= p[0]) sell = p[1];
  const points = rarity + src + region + sell + (item.somber ? somberPoints : 0);
  return Math.max(1, Math.min(tierCount, 1 + Math.floor(points + 1e-4)));
}

/** One give text without `|`: { key } a KH item, { cls } something that is no material, null = nothing to give (a Skip). */
function resolveOne(give, item, tier) {
  const rarity = Math.min(item.rarity, 3);
  const nth = (keys, at) => { const list = keys.split(',').map((s) => s.trim()); return { key: list[Math.min(at, list.length - 1)] }; };
  const material = (grade) => { const k = byFamilyGrade.get(`${familyOf(item)}:${Math.max(1, Math.min(4, grade))}`); return k ? { key: k } : { cls: 'bad' }; };
  if (give === 'pass') return { cls: 'pass' };
  if (give === 'nothing') return { cls: 'nothing' };
  if (give === 'munny:soul') return item.soul > 0 ? { cls: 'munny' } : null;
  if (give === 'munny:sell') return item.sell > 0 ? { cls: 'munny' } : null;
  if (give === 'material') return material(tier);
  if (give.startsWith('equip')) return { cls: 'equip' };
  if (give.startsWith('reward:')) return { cls: 'reward' };
  if (give.startsWith('item:')) return { key: give.slice(5) };
  if (give.startsWith('tier:')) return nth(give.slice(5), rarity);
  if (give.startsWith('bytier:')) return nth(give.slice(7), tier - 1);
  if (give.startsWith('material:')) return material(Math.max(tier, +give.slice(9)));
  if (give.startsWith('munny:')) return { cls: 'munny' };
  return { cls: 'bad' };
}

/** A give text -> [{ key | cls, p }]: the `chance:` alternatives as probabilities (lootmap.rs `resolve`). */
function resolveDist(give, item, tier) {
  const out = [];
  let left = 1;
  for (const raw of give.split('|').map((s) => s.trim())) {
    const m = raw.match(/^chance:([\d.]+):(.*)$/);
    if (m) {
      const p = Math.min(left, +m[1] / 100);
      const r = resolveOne(m[2], item, tier);
      if (r && p > 0) { out.push({ ...r, p }); left -= p; }
      continue;
    }
    const r = resolveOne(raw, item, tier);
    if (r) { out.push({ ...r, p: left }); left = 0; break; }
  }
  if (left > 1e-9) out.push({ cls: 'nothing', p: left });
  return out;
}

/** WHAT THE GATE GIVES for an item from a source at a depth: [{ key | cls, p, rule }] (lootmap.rs `classify`). */
function classify(item, source, depth) {
  if (!item || !item.known) return [{ cls: 'pass', p: 1, rule: 'unknown' }];
  if (item.hidden) return [{ cls: 'pass', p: 1, rule: 'er.hidden' }];
  if (item.cat === 'goods' && onlineGroups.includes(item.sort)) return [{ cls: 'pass', p: 1, rule: 'shop.online_er' }];
  let tier = tierOf(item, source, depth);
  const over = overrides.get(`${item.cat}.${item.id}`) ?? overrides.get(`${item.cat}.${baseId(item)}`);
  if (over !== undefined) return resolveDist(over, item, tier).map((d) => ({ ...d, rule: `id.${item.cat}.${item.id}` }));
  for (const rule of rules) {
    if (rule.proposal && !convert) continue;
    const ok = (!rule.cat || rule.cat.includes(item.cat))
      && (!rule.goods_type || (item.cat === 'goods' && rule.goods_type.includes(item.type)))
      && (!rule.rarity || rule.rarity.includes(item.rarity))
      && (!rule.effect || rule.effect.includes(item.effect))
      && (rule.consumable === undefined || (item.cat === 'goods' && rule.consumable === item.consume))
      && (!rule.ids || rule.ids.some(([lo, hi]) => item.id >= lo && item.id <= hi))
      && (!rule.source || rule.source.includes(source))
      && (!rule.tier || rule.tier.includes(tier))
      && (!rule.wep_kind || (item.cat === 'weapon' && rule.wep_kind.includes(item.wepKind)));
    if (!ok) continue;
    if (rule.as_tier) tier = Math.max(1, Math.min(tierCount, rule.as_tier));
    else if (rule.tier_min && rule.tier_min > tier) tier = Math.max(1, Math.min(tierCount, rule.tier_min));
    let dist = resolveDist(rule.give, item, tier);
    // (PU) a source outside `source.usable` gives a synthesis material for anything that is not one
    if (usable && !usable.includes(source) && !rule.any_source) {
      dist = dist.map((d) => {
        const restricted = d.key ? (kh.get(d.key) || {}).kind !== 'material' : d.cls === 'munny' || d.cls === 'equip';
        return restricted ? { ...resolveOne('material', item, tier), p: d.p } : d;
      });
    }
    const decided = rule.id.startsWith('r2.0');
    const passes = dist.every((d) => d.cls === 'pass');
    if (!decided && !passes && (keep.has(`${CATS_EVENT[item.cat]}:${item.id}`) || keep.has(`${CATS_EVENT[item.cat]}:${baseId(item)}`))) return [{ cls: 'pass', p: 1, rule: 'shop.script_keep' }];
    return dist.map((d) => ({ ...d, rule: rule.id }));
  }
  return [{ cls: 'pass', p: 1, rule: 'unknown' }];
}
const CATS_EVENT = { weapon: 0, protector: 1, accessory: 2, goods: 3, gem: 4 };

// ------------------------------------------------------------------------------------------------ Elden Ring's items
const param = openRegulation(REG);
const P = {
  goods: param('EquipParamGoods.param'), sp: param('SpEffectParam.param'), weapon: param('EquipParamWeapon.param'),
  protector: param('EquipParamProtector.param'), accessory: param('EquipParamAccessory.param'), gem: param('EquipParamGem.param'),
};
const names = new Map();
const TXT = path.join(ROOT, 'build/sora_tmp/er/conv/game/txt/all');
for (const [cat, files] of [['goods', ['GoodsName', 'GoodsName_dlc01']], ['weapon', ['WeaponName', 'WeaponName_dlc01']], ['protector', ['ProtectorName', 'ProtectorName_dlc01']], ['accessory', ['AccessoryName', 'AccessoryName_dlc01']], ['gem', ['GemName', 'GemName_dlc01']]]) {
  for (const f of files) {
    const file = path.join(TXT, f + '.tsv');
    if (!fs.existsSync(file)) continue;
    for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
      const t = line.indexOf('\t');
      const text = t > 0 ? line.slice(t + 1).trim() : '';
      if (t > 0 && text && !names.has(`${cat}:${line.slice(0, t)}`)) names.set(`${cat}:${line.slice(0, t)}`, text);
    }
  }
}
const haveNames = names.size > 0;
const nameOf = (cat, id) => names.get(`${cat}:${id}`) || names.get(`${cat}:${Math.floor(id / 10000) * 10000}`) || `${cat} ${id}`;

const described = new Map();
/** An Elden Ring item as kh-er loot.rs `describe` reads it. */
function describe(cat, id) {
  const k = `${cat}:${id}`;
  if (described.has(k)) return described.get(k);
  let item = null;
  if (cat === 'goods') {
    const d = P.goods.get(id);
    if (d) {
      item = { cat, id, known: true, type: d.readUInt8(F.goods.type), rarity: d.readUInt8(F.goods.rarity), sort: d.readUInt8(F.goods.sort), consume: (d.readUInt8(F.goods.consume[0]) & (1 << F.goods.consume[1])) !== 0, sell: d.readInt32LE(F.goods.sell), effect: 'none', soul: 0 };
      item.hidden = hiddenTypes.includes(item.type) || (haveNames && !names.has(`goods:${id}`));
      const ref = d.readInt32LE(F.goods.ref);
      const sp = ref > 0 ? P.sp.get(ref) : null;
      if (sp) {
        const hp = sp.readInt32LE(F.sp.hpPoint) + sp.readFloatLE(F.sp.hpRate);
        const mp = sp.readInt32LE(F.sp.mpPoint) + sp.readFloatLE(F.sp.mpRate);
        const soul = sp.readInt32LE(F.sp.soul);
        item.effect = soul > 0 ? 'runes' : hp < 0 && mp < 0 ? 'both' : hp < 0 ? 'hp' : mp < 0 ? 'fp' : 'none';
        item.soul = Math.max(0, soul);
      }
    }
  } else if (cat === 'weapon') {
    const d = P.weapon.get(Math.floor(id / 100) * 100) || P.weapon.get(Math.floor(id / 10000) * 10000) || P.weapon.get(id);
    if (d) item = { cat, id, known: true, rarity: d.readUInt8(F.weapon.rarity), sell: d.readInt32LE(F.weapon.sell), wepKind: wepKinds.get(d.readUInt16LE(F.weapon.type)) || 'melee', somber: somberSets.includes(d.readInt32LE(F.weapon.matSet)) };
  } else if (P[cat]) {
    const d = P[cat].get(id);
    if (d) item = { cat, id, known: true, rarity: d.readUInt8(F[cat].rarity), sell: d.readInt32LE(F[cat].sell) };
  }
  described.set(k, item);
  return item;
}

/** The region depth of a map lot by its id: mAA_BB (8 digits AABBnnnn), the m60 tiles (10XXYYnnnn), the DLC (20XXYYnnnn). */
function depthOfLot(id) {
  const s = String(id);
  if (s.length === 10 && s.startsWith('10')) {
    const x = +s.slice(2, 4), z = +s.slice(4, 6);
    const box = regionBoxes.find((b) => x >= b[0] && x <= b[1] && z >= b[2] && z <= b[3]);
    return box ? box[4] : regionDefault;
  }
  if (s.length === 10 && s.startsWith('20')) return 4;
  if (s.length === 8) return regionAreas[String(Math.floor(id / 1e6))] ?? regionDefault;
  return null;
}

// ------------------------------------------------------------------------------------------------ the supply
/** material key -> { fixed: expected count, fixedBy: source -> count, repeat: Set of source texts, margin: count } */
const supply = new Map();
const sup = (key) => { if (!supply.has(key)) supply.set(key, { fixed: 0, fixedBy: {}, repeat: new Set(), repeatGrades: new Set(), margin: 0, marginBy: {} }); return supply.get(key); };
const ORE = { orichalcum: 'MAT_ITEM56', orichalcum_plus: 'MAT_ITEM57' };
const ORES = new Set(Object.values(ORE));
const randomOre = [];

function addSlot({ cat, id, n, p, fixed, source, depths, where }) {
  const item = describe(cat, id);
  if (!item) return;
  for (const depth of depths) {
    for (const d of classify(item, source, depth)) {
      if (!d.key || (kh.get(d.key) || {}).kind !== 'material') continue;
      const s = sup(d.key);
      const count = n * p * d.p;
      if (count <= 0) continue;
      if (ORES.has(d.key)) {
        // the never-random rule: an ore from a repeatable source, or by chance, is a fault of the rules
        if (!fixed || d.p < 1 - 1e-9) randomOre.push(`${where}: ${nameOf(cat, id)} -> ${d.key} (${d.rule}, ${source}, p ${(p * d.p).toFixed(3)})`);
        if (depth === depths[0]) { s.margin += count; s.marginBy[d.rule] = (s.marginBy[d.rule] || 0) + count; }
        continue;
      }
      if (fixed) {
        if (depth === depths[0]) { s.fixed += count; s.fixedBy[source] = (s.fixedBy[source] || 0) + count; }
      } else {
        s.repeat.add(`${source}:${nameOf(cat, id)}`);
      }
    }
  }
}

function lotSlots(d) {
  const slots = [];
  for (let k = 0; k < 8; k++) slots.push({ id: d.readInt32LE(F.lot.id + k * 4), cat: d.readInt32LE(F.lot.cat + k * 4), points: d.readUInt16LE(F.lot.points + k * 2), flag: d.readUInt32LE(F.lot.slotFlag + k * 4), n: d.readUInt8(F.lot.num + k) });
  const total = slots.reduce((a, s) => a + s.points, 0);
  return { slots, total, flag: d.readUInt32LE(F.lot.flag) };
}

const mapLots = param('ItemLotParam_map.param');
const unflaggedOre = [];
for (const [lot, d] of mapLots) {
  const { slots, total, flag } = lotSlots(d);
  if (!total) continue;
  const depth = depthOfLot(lot);
  for (const s of slots) {
    const cat = lotCats[String(s.cat)];
    if (!cat || s.id <= 0 || !s.points) continue;
    const fixed = (flag || s.flag) !== 0;
    // an event's award (a boss, a gift: ids outside the map numbering) counts as the event's, a flagged map lot as a fixed
    // pickup (a chest or a corpse: the gate tells them by the call site; the tier takes the lower, a world pickup), an
    // unflagged one as a gathered node
    const source = depth === null ? 'npc' : fixed ? 'world' : 'gather';
    if (cat === 'goods' && (s.id === 10140 || s.id === 10168) && !fixed) unflaggedOre.push(`map lot ${lot} (${nameOf('goods', s.id)}: unflagged; ${lot >= 990000 && lot < 1000000 ? 'the 99xxxx item test rows no map places' : 'CHECK'})`);
    if (!fixed && lot >= 990000 && lot < 1000000) continue; // (the item test rows: no map places them)
    addSlot({ cat, id: s.id, n: s.n || 1, p: s.points / total, fixed, source, depths: [depth ?? regionDefault], where: `map lot ${lot}` });
  }
}

const enemyLots = param('ItemLotParam_enemy.param');
const npcs = param('NpcParam.param');
const reached = new Set();
const chainMax = val('lot.chain_max', 16);
for (const [, d] of npcs) {
  const lot = d.readInt32LE(F.npc.lot);
  if (lot <= 0) continue;
  for (let k = 0; k < chainMax && enemyLots.has(lot + k); k++) reached.add(lot + k);
}
const enemyOre = [];
for (const lot of reached) {
  const { slots, total, flag } = lotSlots(enemyLots.get(lot));
  if (!total) continue;
  for (const s of slots) {
    const cat = lotCats[String(s.cat)];
    if (!cat || s.id <= 0 || !s.points) continue;
    const fixed = (flag || s.flag) !== 0;
    if (cat === 'goods' && (s.id === 10140 || s.id === 10168)) enemyOre.push(`enemy lot ${lot}${fixed ? ' (flagged)' : ''}`);
    addSlot({ cat, id: s.id, n: s.n || 1, p: s.points / total, fixed, source: 'enemy', depths: fixed ? [2] : [0, 1, 2, 3, 4], where: `enemy lot ${lot}` });
  }
}

// the KH shop's resale of the merchants' own lines (kh-er spirittune.rs `merchant_lines`: the gloveworts; shopkh.rs `ore_lines`:
// the Smithing Stones): unlimited, paid in runes, priced
const shopLines = param('ShopLineupParam.param');
const resale = [];
for (const [line, d] of shopLines) {
  if (d.readUInt8(F.shop.type) !== 3 || d.readUInt8(F.shop.cost) !== 0 || d.readInt16LE(F.shop.qty) >= 0 || d.readInt32LE(F.shop.value) <= 0) continue;
  const id = d.readInt32LE(F.shop.id);
  const item = describe('goods', id);
  if (!item) continue;
  for (const r of classify(item, 'shop', 0)) {
    // (shopkh.rs `ore_of`: a Smithing Stone's own row r2.30x giving Fluorite / Damascus / Adamantite / Electrum; never r2.309)
    const ok = r.key && ((r.rule.startsWith('r2.30') && r.rule !== 'r2.309.reinforce_other' && /^MAT_ITEM5[3-58]$/.test(r.key)) || r.key.startsWith('MAT_GLOVE_'));
    if (!ok) continue;
    sup(r.key).repeat.add(`shop:${nameOf('goods', id)}`);
    resale.push(`${line} ${nameOf('goods', id)} -> ${r.key} ${d.readInt32LE(F.shop.value)}`);
  }
}

// the Collector's Goals' item rewards (fixed, once)
const GOALS = sheet('kh3-collect-goals');
for (const r of GOALS.rows) {
  const reward = r[ic(GOALS, 'reward_item')];
  if (r[ic(GOALS, 'reward_kind')] !== 'GET_ITEM' || !kh.has(reward) || kh.get(reward).kind !== 'material') continue;
  const s = sup(reward);
  if (ORES.has(reward)) { s.margin += 1; s.marginBy[`goal ${r[0]}`] = 1; } else { s.fixed += 1; s.fixedBy.goal = (s.fixedBy.goal || 0) + 1; }
}

// the fixed ore placements (sheet kh-ore-placements; the only Orichalcum the budget counts)
const placements = { base: {}, dlc: {}, pending: [] };
const ORESHEET = path.join(ROOT, 'sheets/port/kh-ore-placements.json');
if (fs.existsSync(ORESHEET)) {
  for (const r of JSON.parse(fs.readFileSync(ORESHEET, 'utf8')).rows) {
    if (!r[0].startsWith('ore.')) continue;
    const v = r[1];
    const m = /^item:([A-Z0-9_+]+)(?::(\d+))?$/.exec(v.give || '');
    if (!m) continue;
    const n = m[2] ? +m[2] : 1;
    if (!(v.verified && v.flag)) { placements.pending.push(`${r[0]} (${v.place || '-'})`); continue; }
    const into = v.dlc ? placements.dlc : placements.base;
    into[m[1]] = (into[m[1]] || 0) + n;
  }
}
for (const [key, n] of Object.entries(placements.base)) { const s = sup(key); s.fixed += n; s.fixedBy.placement = (s.fixedBy.placement || 0) + n; }
for (const [key, n] of Object.entries(placements.dlc)) { const s = sup(key); s.margin += n; s.marginBy['placement (DLC)'] = n; }

// ------------------------------------------------------------------------------------------------ the need
const SYN = sheet('kh3-synthesis');
const LV = sheet('kh3-keyblade-levels');
const need = new Map();
const needBy = new Map();
const add = (key, n, why) => { need.set(key, (need.get(key) || 0) + n); if (!needBy.has(key)) needBy.set(key, []); needBy.get(key).push(why); };
let source = 'the WD sheets kh3-synthesis / kh3-keyblade-levels';
const WORKSHOP = path.join(ROOT, 'sheets/port/kh-workshop.json');
let fromWorkshop = false;
if (fs.existsSync(WORKSHOP)) {
  // (P6a) its `syn.IS_<n>` rows, when they carry the materials as KH3's
  try {
    const w = JSON.parse(fs.readFileSync(WORKSHOP, 'utf8'));
    const syn = w.rows.filter((r) => /^syn\.IS_\d+$/.test(r[0]) && r[1] && Array.isArray(r[1].materials));
    if (syn.length === SYN.rows.length) {
      for (const r of syn) r[1].materials.forEach((m, k) => (Array.isArray(m) ? add(m[0], m[1], r[0]) : add(m, r[1].counts[k], r[0])));
      fromWorkshop = true;
      source = 'P6a\'s kh-workshop.json (syn.*) and kh3-keyblade-levels';
    }
  } catch (e) { /* the WD sheets stand */ }
}
if (!fromWorkshop) for (const r of SYN.rows) r[ic(SYN, 'materials')].forEach((m, k) => add(m, r[ic(SYN, 'counts')][k], r[0]));
const forge = new Map();
for (const r of LV.rows) {
  if (r[ic(LV, 'init_achieved')]) continue;
  r[ic(LV, 'materials')].forEach((m, k) => { add(m, r[ic(LV, 'counts')][k], r[0]); forge.set(m, (forge.get(m) || 0) + r[ic(LV, 'counts')][k]); });
}

// ------------------------------------------------------------------------------------------------ the verdict
const MATS = sheet('kh3-materials');
const materials = MATS.rows.map((r) => ({ key: r[0], name: r[ic(MATS, 'name')], rank: r[ic(MATS, 'rank')] }));
const report = { regulation: REG, need_from: source, materials: [], short: [], goals: [], ores: {}, random_ore: randomOre, enemy_ore_lots: enemyOre, unflagged_ore_lots: unflaggedOre, pending_placements: placements.pending, resale };
for (const m of materials) {
  const s = sup(m.key);
  const n = need.get(m.key) || 0;
  const ore = ORES.has(m.key);
  const repeat = !ore && s.repeat.size > 0;
  const ok = repeat || s.fixed + 1e-9 >= n;
  const line = { key: m.key, name: m.name, need: n, forge: forge.get(m.key) || 0, fixed: +s.fixed.toFixed(2), repeatable: ore ? 0 : s.repeat.size, margin: +s.margin.toFixed(2), reachable: ok, any_source: repeat || s.fixed > 0 };
  report.materials.push(line);
  if (!ok) report.short.push(line);
}
const any = new Set(report.materials.filter((l) => l.any_source || l.fixed > 0).map((l) => l.key));
for (const r of GOALS.rows) {
  const cond = r[ic(GOALS, 'condition')], nn = r[ic(GOALS, 'n')], x = r[ic(GOALS, 'x')], y = r[ic(GOALS, 'y')], rank = r[ic(GOALS, 'rank')];
  let ok = true, why = '';
  if (cond === 'MAT_KIND_NUM') { ok = any.size >= nn; why = `${any.size} of ${nn} kinds`; }
  else if (cond === 'MAT_KIND_ALL') { const keys = materials.map((m) => m.key).filter((k) => k >= x && k <= y); const miss = keys.filter((k) => !any.has(k)); ok = !miss.length; why = miss.length ? `missing ${miss.join(', ')}` : `${keys.length} kinds`; }
  else if (cond === 'MAT_GET_RANK') { const miss = materials.filter((m) => m.rank === rank && !any.has(m.key)).map((m) => m.key); ok = !miss.length; why = miss.length ? `missing ${miss.join(', ')}` : `every ${rank}`; }
  else if (cond === 'MAT_SPKIND_NUM') { const l = report.materials.find((m) => m.key === x); ok = l.repeatable > 0 || l.fixed >= nn; why = `${x}: ${l.repeatable ? 'repeatable' : l.fixed.toFixed(1) + ' fixed'} (needs ${nn})`; }
  else if (cond === 'MAT_TOTAL_NUM') { const rep = report.materials.some((m) => m.repeatable > 0); ok = rep; why = rep ? 'repeatable sources exist' : 'no repeatable source'; }
  report.goals.push({ id: r[0], condition: cond, ok, why });
}
for (const [name, key] of Object.entries(ORE)) {
  const s = sup(key);
  report.ores[name] = { key, need: need.get(key) || 0, base_placements: placements.base[key] || 0, dlc_placements: placements.dlc[key] || 0, margin_er_stones_larval_goals: +(s.margin - (placements.dlc[key] || 0)).toFixed(2), margin_by: s.marginBy };
}
// the families: which grades of each family come from a repeatable source; THE ECONOMY RULE: a family whose share of the need
// is more than twice its share of the stable pick is SHORT (every material comes 1 in 13 of the pick, KH3's Forge eats
// Wellspring): its weight in row material.weights should be about 13 x its need share
const famRows = [];
const famNeed = families.map((f) => [1, 2, 3, 4].reduce((a, g) => a + (need.get(byFamilyGrade.get(`${f}:${g}`)) || 0), 0));
const needTotal = famNeed.reduce((a, n) => a + n, 0) || 1;
report.families = [];
families.forEach((f, k) => {
  const grades = [1, 2, 3, 4].map((g) => { const key = byFamilyGrade.get(`${f}:${g}`); const l = report.materials.find((m) => m.key === key); return l ? `${l.repeatable ? 'R' + l.repeatable : 'F' + l.fixed.toFixed(0)}/${l.need}` : '-'; });
  const needShare = famNeed[k] / needTotal, pickShare = familyWeights[k] / weightTotal;
  const suggested = Math.max(1, Math.round(families.length * needShare));
  const short = needShare > 2 * pickShare;
  report.families.push({ family: f, need: famNeed[k], need_share: +needShare.toFixed(3), weight: familyWeights[k], pick_share: +pickShare.toFixed(3), suggested_weight: suggested, short });
  famRows.push(`${f.padEnd(11)} ${grades.map((g) => g.padEnd(12)).join('')} need ${String(famNeed[k]).padStart(4)} = ${(needShare * 100).toFixed(1).padStart(4)} %, pick ${(pickShare * 100).toFixed(1).padStart(4)} % (weight ${familyWeights[k]})${short ? ` SHORT: weight ${suggested}` : ''}`);
});

// ------------------------------------------------------------------------------------------------ the report
const recipesOk = SYN.rows.filter((r) => r[ic(SYN, 'materials')].every((m) => report.materials.find((l) => l.key === m)?.reachable)).length;
const levels = LV.rows.filter((r) => !r[ic(LV, 'init_achieved')]);
const levelsOk = levels.filter((r) => r[ic(LV, 'materials')].every((m) => report.materials.find((l) => l.key === m)?.reachable)).length;
const goalsOk = report.goals.filter((g) => g.ok).length;
console.log(`KH3 MATERIAL BUDGET (read-only): regulation ${path.relative(ROOT, REG)}; need from ${source}`);
console.log(`recipes reachable: ${recipesOk} / ${SYN.rows.length}; Forge levels reachable: ${levelsOk} / ${levels.length}; Collector's Goals reachable: ${goalsOk} / ${report.goals.length}`);
for (const [name, o] of Object.entries(report.ores)) console.log(`${name.padEnd(16)} need ${o.need}, fixed placements base game ${o.base_placements} (DLC +${o.dlc_placements}${placements.pending.length ? `, ${placements.pending.length} unverified placement row(s) not counted` : ''}); margin not counted: ${o.margin_er_stones_larval_goals} (${Object.entries(o.margin_by).filter(([k]) => !k.startsWith('placement')).map(([k, v]) => `${k} ${(+v).toFixed(1)}`).join(', ') || 'none'})`);
console.log(`ore never random: ${randomOre.length ? `NO: ${randomOre.length} random source(s): ${randomOre.slice(0, 8).join('; ')}` : 'yes (no repeatable or chance source gives Orichalcum / Orichalcum+)'}`);
if (enemyOre.length) console.log(`  enemy lots an NpcParam reaches holding an Ancient Dragon / Somber Ancient stone: ${enemyOre.join(', ')} (they give Electrum: r2.306)`);
if (unflaggedOre.length) console.log(`  unflagged map lots holding one: ${unflaggedOre.join('; ')}`);
console.log(`KH shop resale lines (merchants' own unlimited rune lines): ${resale.length}${ALL ? ': ' + resale.join('; ') : ''}`);
console.log('families (grade 1 Shard .. 4 Crystal): R<n> = n repeatable sources, F<n> = fixed supply; /<need>');
for (const r of famRows) console.log('  ' + r);
const shown = ALL ? report.materials : report.materials.filter((l) => !l.reachable || ORES.has(l.key) || /^MAT_ITEM(5[3-9]|60)$/.test(l.key));
for (const l of shown) console.log(`  ${l.reachable ? 'ok   ' : 'SHORT'} ${l.key} ${l.name.padEnd(20)} need ${String(l.need).padStart(4)} (Forge ${l.forge}) | fixed ${String(l.fixed).padStart(7)} | repeatable sources ${l.repeatable}${l.margin ? ` | margin ${l.margin}` : ''}`);
for (const g of report.goals.filter((x) => !x.ok || ALL)) console.log(`  goal ${g.id} ${g.condition}: ${g.ok ? 'ok' : 'NOT REACHABLE'} (${g.why})`);
if (report.short.length) console.log(`SHORT: ${report.short.map((l) => l.key + ' ' + l.name).join(', ')}: the fix is the family's share of the stable hash weighted (sheet sora-er-items material.weights)`);
const shortFamilies = report.families.filter((f) => f.short);
console.log(`families short by the economy rule: ${shortFamilies.length ? shortFamilies.map((f) => `${f.family} (weight ${f.weight} -> ${f.suggested_weight})`).join(', ') : 'none'}`);
const out = arg('--json');
if (out) fs.writeFileSync(out, JSON.stringify(report, null, 1));
process.exitCode = recipesOk === SYN.rows.length && levelsOk === levels.length && goalsOk === report.goals.length && !randomOre.length && !shortFamilies.length ? 0 : 1;
