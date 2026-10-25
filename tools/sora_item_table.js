#!/usr/bin/env node
// (loot) KH3's item universe for the port: every battle item, camp item, armour piece, accessory and synthesis
// material of the game's item table (TresItemData), with the item id the prize box and the stock use.
// Writes sheets/port/sora-item-table.json and registers it in tools/lib/sheet_defs.js (once).
// Docs: docs/sora-prize-spec.md section 11, docs/sora-er-items.md section 2.
'use strict';
const fs = require('fs');
const path = require('path');
const root = path.join(__dirname, '..');
const table = JSON.parse(fs.readFileSync(path.join(root, 'sheets', 'entities', 'Load', 'Tres', 'TresItemData.json'), 'utf8'));
const strings = JSON.parse(fs.readFileSync(path.join(root, 'build', 'sora_pack', 'ui', 'strings', 'en', 'kh3_item.json'), 'utf8')).namespaces;
const text = (id) => { if (!id) return '-'; const k = id.lastIndexOf('.'); const ns = strings[id.slice(0, k)]; return (ns && ns[id.slice(k + 1)]) || '-'; };
const KINDS = { BATTLEITEM: 'battle', CAMPITEM: 'camp', PROTECTOR: 'armour', ACCESSORY: 'accessory', MATERIAL: 'material' };
// ETresItemRank: RANK03 is the lowest material grade (Shard), RANK00 the highest (Crystal and the ores).
const GRADE = { NOTHING: 0, RANK03: 1, RANK02: 2, RANK01: 3, RANK00: 4 };
const BOX = { 1: 'heal', 2: 'heal', 7: 'material', 8: 'foodstuff' };
const rows = [];
for (const r of table.rows) {
  const kind = KINDS[r.m_Category.split('::')[1]];
  if (!kind) continue;
  const m = /^I(\d\d)(\d\d\d)$/.exec(r._row_name);
  if (!m) throw new Error('row name ' + r._row_name);
  const cat = parseInt(m[1], 10), index = parseInt(m[2], 10);
  const itemId = (cat << 8) | index;
  const name = text(r.m_NameTextID);
  const family = kind === 'material' ? name.replace(/ (Shard|Stone|Gem|Crystal)$/, '').replace(/\+$/, '') : '-';
  rows.push([r.m_KeyName, itemId, kind, GRADE[r.m_ItemRank.split('::')[1]], name, family, r.m_BuyPrice, r.m_SellPrice, r.m_MaxNum, BOX[cat] || 'rare',
    r.m_bCanSell ? 1 : 0, r._row_name, 'confirmed', '-']);
}
const columns = ['id', 'item_id', 'kind', 'grade', 'name', 'family', 'buy', 'sell', 'max', 'box', 'can_sell', 'row', 'ref_conf', 'note'];
fs.writeFileSync(path.join(root, 'sheets', 'port', 'sora-item-table.json'),
  '{"sheet":"sora-item-table","group":"port","columns":' + JSON.stringify(columns) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
console.log('sheets/port/sora-item-table.json: ' + rows.length + ' rows');

const defs = path.join(root, 'tools', 'lib', 'sheet_defs.js');
let s = fs.readFileSync(defs, 'utf8');
if (!s.includes("'sora-item-table'")) {
  const anchor = "  'sora-er-fx': {";
  const at = s.indexOf(anchor);
  if (at < 0) throw new Error('sheet_defs.js: anchor not found');
  const entry =
    "  // (loot) KH3's items a Sora can hold: battle and camp items, armour, accessories, synthesis materials, with the item id a prize box carries. Written by tools/sora_item_table.js; both hosts read the file itself (kh-host/src/items.rs)\n" +
    "  'sora-item-table': { group: 'port', file: 'sora-item-table.json', by: 'tools/sora_item_table.js', from: 'sheets/entities/Load/Tres/TresItemData.json (categories BATTLEITEM, CAMPITEM, PROTECTOR, ACCESSORY, MATERIAL) and the English string table kh3_item', external: true, cols: [\n" +
    "    ['id', K, 'str', '-', 'm_KeyName (ITEM_POTION, PRT_ITEM01, ACC_ITEM01, MAT_ITEM01 ...): for armour and accessories the id of the sora-equipment row'], ['item_id', V, 'int', '-', 'the item id: category x 256 + index of the row name I<cc><nnn> (0x101 Potion, 0x701 Blazing Shard); what a prize box holds'],\n" +
    "    ['kind', V, 'enum:battle|camp|armour|accessory|material', '-', 'ETresItemCategory'], ['grade', V, 'int', '-', 'm_ItemRank as a grade: 0 none, 1 RANK03 (Shard), 2 RANK02 (Stone), 3 RANK01 (Gem), 4 RANK00 (Crystal, the ores)'],\n" +
    "    ['name', V, 'str', '-', 'English name (m_NameTextID); \"-\" = no string'], ['family', V, 'str', '-', 'materials: the name without its grade word (Blazing, Mythril ...); \"-\" otherwise'],\n" +
    "    ['buy', V, 'int', '-', 'm_BuyPrice, munny'], ['sell', V, 'int', '-', 'm_SellPrice, munny'], ['max', V, 'int', '-', 'm_MaxNum: the stock maximum'],\n" +
    "    ['box', V, 'enum:heal|material|foodstuff|rare', '-', 'the prize box an item of this id comes in (item box spawn 0x141473110: by the id\x27s high byte)'], ['can_sell', V, 'int', '-', 'm_bCanSell'],\n" +
    "    ['row', V, 'str', '-', 'row name of TresItemData', true], ['ref_conf', V, 'enum:confirmed|inferred|guess', '-', 'confirmed = read from the data table'], ['note', V, 'str', '-', '-']] },\n";
  s = s.slice(0, at) + entry + s.slice(at);
  fs.writeFileSync(defs, s);
  console.log('registered sora-item-table in tools/lib/sheet_defs.js');
}
