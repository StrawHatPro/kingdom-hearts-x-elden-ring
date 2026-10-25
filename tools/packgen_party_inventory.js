#!/usr/bin/env node
// (PACKGEN) build/sora_tmp/party/inventory.json for tools/party_pack.js on a player's PC.
//
//   node tools/packgen_party_inventory.js
//
// The development file is the party survey's whole inventory (build/sora_tmp/party/tools/inventory.js, which also reads the
// decompilation's function index). tools/party_pack.js reads one part of it: per member the weapon rows (`weapons`: the
// TresEquipItemData rows whose m_KeyName holds WEP_DONALD / WEP_GOOFY). This tool writes that part the same way, from
// sheets/entities/Load/Tres/TresEquipItemData.json (tools/sheets_build.js, from the player's own paks).
'use strict';
const fs = require('fs');
const path = require('path');
const { ROOT } = require('./lib/log');

const SHEET = path.join(ROOT, 'sheets', 'entities', 'Load', 'Tres', 'TresEquipItemData.json');
if (!fs.existsSync(SHEET)) { console.error(`${path.relative(ROOT, SHEET)} is missing: run node tools/sheets_build.js first`); process.exit(1); }
const rows = JSON.parse(fs.readFileSync(SHEET, 'utf8')).rows;
const inv = { generated: 'tools/packgen_party_inventory.js (the weapon rows of the party survey inventory)', note: 'paths relative to assets_converted/packages/KINGDOM HEARTS III/Content unless stated' };
for (const k of ['donald', 'goofy']) {
  const eq = rows.filter((r) => new RegExp('WEP_' + k.toUpperCase()).test(r.m_KeyName));
  inv[k] = { weapons: eq.map((r) => ({ row: r._row_name, key: r.m_KeyName, name_ja: r.m_Comment && r.m_Comment.s, ap: r.m_AP, str: r.m_AttackPlus, mag: r.m_MagicPlus, def: r.m_DefensePlus, abilities: r.m_AppendAbility.items, equip: r.m_EquipSubclass })) };
}
const OUT = path.join(ROOT, 'build', 'sora_tmp', 'party', 'inventory.json');
fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, JSON.stringify(inv, null, 1));
console.log(`packgen_party_inventory: ${inv.donald.weapons.length} + ${inv.goofy.weapons.length} weapon rows -> build/sora_tmp/party/inventory.json`);
