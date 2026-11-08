#!/usr/bin/env node
// (loot) KH3's item universe for the port: every battle item, camp item, armour piece, accessory and synthesis
// material of the game's item table (TresItemData), with the item id the prize box and the stock use; (weapons P1) and
// the 45 weapons (17 Keyblades, Donald's 14 staves, Goofy's 14 shields: kind 'weapon', read by kh-host/src/weapons.rs).
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
// (weapons P1, the user 2026-11-02: "Add all of the keyblades from KH3 ... Also add Donald and Goofy's weapons";
// documentation/KH3_WEAPONS_PLAN.md 1.1 / 1.2) THE WEAPONS AS ITEMS, kind 'weapon': the 17 Keyblades of the WD sheet
// kh3-keyblades (WEP_NOEQUIP, the friends' weapons of other characters and the item-less w_so100 / 120 / 160 / 180 are
// not there) and Donald's 14 staves / Goofy's 14 shields of kh3-party-weapons. In TresItemData's own order (category 3
// sits between the camp items and the armour). chara: whose weapon; init_level: the Forge level a Keyblade has when it is
// obtained (kh3-keyblades.init_level = the highest m_bInitAchieved row; 0 for the friends' weapons, which have no Forge).
const sheetRows = (name) => { const s = JSON.parse(fs.readFileSync(path.join(root, 'sheets', 'port', name + '.json'), 'utf8')); const c = (n) => s.columns.indexOf(n); return s.rows.map((r) => ({ id: r[c('id')], chara: c('chara') >= 0 ? r[c('chara')] : 'sora', init: c('init_level') >= 0 ? r[c('init_level')] : 0 })); };
const WEAPONS = new Map([...sheetRows('kh3-keyblades'), ...sheetRows('kh3-party-weapons')].map((w) => [w.id, w]));
if (WEAPONS.size !== 17 + 28) throw new Error('kh3-keyblades + kh3-party-weapons: ' + WEAPONS.size + ' weapons, not 45');
const rows = [];
for (const r of table.rows) {
  if (r.m_Category === 'ETresItemCategory::WEAPON') {
    const w = WEAPONS.get(r.m_KeyName);
    if (!w) continue;
    const m = /^I03(\d\d\d)$/.exec(r._row_name);
    if (!m) throw new Error('weapon row name ' + r._row_name);
    rows.push([r.m_KeyName, 0x300 | parseInt(m[1], 10), 'weapon', 0, text(r.m_NameTextID), '-', r.m_BuyPrice, r.m_SellPrice, r.m_MaxNum, 'rare',
      r.m_bCanSell ? 1 : 0, r._row_name, 'confirmed', '-', '-', 0, w.chara, w.init]);
    WEAPONS.delete(r.m_KeyName);
    continue;
  }
  const kind = KINDS[r.m_Category.split('::')[1]];
  if (!kind) continue;
  const m = /^I(\d\d)(\d\d\d)$/.exec(r._row_name);
  if (!m) throw new Error('row name ' + r._row_name);
  const cat = parseInt(m[1], 10), index = parseInt(m[2], 10);
  const itemId = (cat << 8) | index;
  const name = text(r.m_NameTextID);
  const family = kind === 'material' ? name.replace(/ (Shard|Stone|Gem|Crystal)$/, '').replace(/\+$/, '') : '-';
  rows.push([r.m_KeyName, itemId, kind, GRADE[r.m_ItemRank.split('::')[1]], name, family, r.m_BuyPrice, r.m_SellPrice, r.m_MaxNum, BOX[cat] || 'rare',
    r.m_bCanSell ? 1 : 0, r._row_name, 'confirmed', '-', '-', 0, '-', 0]);
  // (THE USER 2026-11-02: "Re-add Grave Gloveworts and Ghost Gloveworts into Elden Ring ... Treat them as upgrade materials and
  // have them appear alongside upgrade materials in the camp menu's item menus") right after KH3's ores (the mod's upgrade
  // materials: Smithing Stones become them), before the two crystals: the Items screen lists materials in this order
  if (r.m_KeyName === 'MAT_ITEM58') rows.push(...gloveworts());
}

// THE GLOVEWORTS: COMPOSED rows (not KH3 items). One KH3 material per Elden Ring grade: Grave Glovewort [1] .. [9] and Great
// Grave Glovewort (Elden Ring goods 10900 .. 10909), Ghost Glovewort [1] .. [9] and Great Ghost Glovewort (10910 .. 10919).
// Names: Elden Ring's own (GoodsName). Help: COMPOSED in KH3's material style (a short line, then the tag in KH3's markup).
// Item ids 0x781 .. 0x78A / 0x791 .. 0x79A (category 7 = a material prize box; KH3's materials end at 0x73C). buy: Elden Ring's
// own price at the Twin Maiden Husks (ShopLineupParam 101829 .. 101846, regulation 2.7.2.0, read 2026-11-02); the Great ones
// are sold nowhere: 1.5 x the [9]'s. grade: the glovewort's own ([n]; 10 = Great). (THE USER 2026-11-03: "gloveworts should use
// their Elden Ring icon in all of the KH menus and notifications") er_icon: the goods row's EquipParamGoods.iconId (2100 ..
// 2109 Grave, 2110 .. 2119 Ghost, READ in regulation 2.7.2.0): the picture is Elden Ring's own MENU_ItemIcon of that number
// from the pack's er_icons (tools/er_icons.js, the player's own install), kh-ui item_art.
function gloveworts() {
  const out = [];
  const families = [
    { key: 'GRAVE', family: 'Grave Glovewort', base: 0x780, er: 10900, icon: 2100, prices: [800, 1200, 1600, 2400, 3200, 4000, 5000, 6000, 8000],
      line: 'A white flower that blooms in catacombs.', great: 'An especially large catacomb flower.', /* (each first line short: the pane's three lines hold the tag) */ tunes: 'Tunes spirit ashes to +' },
    { key: 'GHOST', family: 'Ghost Glovewort', base: 0x790, er: 10910, icon: 2110, prices: [1200, 1800, 2400, 3600, 4800, 6000, 7500, 9000, 12000],
      line: 'A flower a spirit nestles close to.', great: 'A grand flower a spirit nestles to.', tunes: 'Tunes renowned spirit ashes to +' },
  ];
  for (const f of families) {
    for (let g = 1; g <= 10; g++) {
      const great = g === 10;
      const key = 'MAT_GLOVE_' + f.key + '_' + (great ? 'GREAT' : String(g).padStart(2, '0'));
      const name = great ? 'Great ' + f.family : f.family + ' [' + g + ']';
      const buy = great ? Math.round(f.prices[8] * 1.5) : f.prices[g - 1];
      const help = (great ? f.great : f.line) + '<br>' + f.tunes + g + '. <color(strong)>(Upgrade material)<color>';
      out.push([key, f.base + g, 'material', g, name, f.family, buy, 0, 99, 'material', 0, '-', 'guess',
        'COMPOSED (THE USER 2026-11-02: "Re-add Grave Gloveworts and Ghost Gloveworts ... Treat them as upgrade materials"): not a KH3 item; Elden Ring goods ' + (f.er + g - 1) + ' (sora-er-items id.goods.' + (f.er + g - 1) + ')', help, f.icon + g - 1, '-', 0]);
    }
  }
  return out;
}
if (WEAPONS.size) throw new Error('weapons of the WD sheets without a TresItemData row: ' + [...WEAPONS.keys()].join(' '));
const columns = ['id', 'item_id', 'kind', 'grade', 'name', 'family', 'buy', 'sell', 'max', 'box', 'can_sell', 'row', 'ref_conf', 'note', 'help', 'er_icon', 'chara', 'init_level'];
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
