// (GF-STATS) THE FRIENDS' STAT TABLE: sheets/port/party-balance-levels.json, built from each character's own anchors in
// sheets/port/party-balance.json (n_ex00x.<strength|magic|defense>.<l1|l50|l99>: the status-screen values with his
// starting weapon). A stat grows in straight lines between its anchors; a row stores it minus the friend's starting weapon
// (party-weapons WEP_DONALD_00 / WEP_GOOFY_00), so a better weapon adds on top. NOTHING HERE READS SORA'S SHEETS (2026-11-01,
// the user: "Donald and Goofy need to have independent stats from Sora, those multipliers are just for reference from level 1").
// The rows of chara `reference` are the fixed curve of the damage-taken rule (incoming.reference_hp / _defense anchors,
// build/sora_tmp/party/GOOFY/PLAN.md R8): one curve for every friend, not a live link to anyone.
// The table is what the code reads (crates/kh-party/src/balance.rs through tools/gen_port.js).
//   node tools/party_balance_sheet.js           check: the table on disk equals what the anchors give (exit 1 if not)
//   node tools/party_balance_sheet.js --write   write it
const fs = require('fs');
const path = require('path');
const ROOT = path.join(__dirname, '..');
const PORT = path.join(ROOT, 'sheets', 'port');
const read = (f) => JSON.parse(fs.readFileSync(path.join(PORT, f), 'utf8'));
const params = Object.fromEntries(read('party-balance.json').rows.map((r) => [r[0], r[1]]));
const weapons = read('party-weapons.json');
const wcol = (name) => weapons.columns.indexOf(name);
const start = (key) => {
  const w = weapons.rows.find((r) => r[wcol('key')] === key);
  return [w[wcol('strength')], w[wcol('magic')], w[wcol('defense')]];
};
const START = { n_ex001: start('WEP_DONALD_00'), n_ex002: start('WEP_GOOFY_00') };
const anchors = (base) => ['l1', 'l50', 'l99'].map((l) => {
  const v = params[`${base}.${l}`];
  if (typeof v !== 'number') throw new Error(`party-balance has no number ${base}.${l}`);
  return v;
});
// straight lines between the anchors at levels 1, 50 and 99
const curve = ([a, b, c], lv) => (lv <= 50 ? a + (b - a) * (lv - 1) / 49 : b + (c - b) * (lv - 50) / 49);
const rows = [];
for (const chara of ['n_ex001', 'n_ex002']) {
  const stats = ['strength', 'magic', 'defense'];
  const lines = stats.map((stat) => anchors(`${chara}.${stat}`));
  const w = START[chara];
  for (let lv = 1; lv <= 99; lv++) {
    const [s, m, d] = lines.map((line, i) => Math.max(0, Math.round(curve(line, lv)) - w[i]));
    const src = lv === 1
      ? `PORT DESIGN (2026-11-01, the user: "Donald and Goofy need to have independent stats from Sora"): his own anchors of party-balance ${lines.map((l, i) => `${stats[i]} ${l.join(' / ')}`).join(', ')} (status screen at levels 1 / 50 / 99 with his starting weapon), minus that weapon's ${w.join(' / ')}; built by tools/party_balance_sheet.js`
      : `PORT DESIGN, as row ${chara}.1`;
    rows.push([`${chara}.${lv}`, chara, lv, s, m, d, 0, src]);
  }
}
const [refHp, refDef] = ['hp', 'defense'].map((stat) => anchors(`incoming.reference_${stat}`));
for (let lv = 1; lv <= 99; lv++) {
  const src = lv === 1
    ? `PORT DESIGN (build/sora_tmp/party/GOOFY/PLAN.md R8): the damage-taken curve of every friend: party-balance incoming.reference_hp ${refHp.join(' / ')}, incoming.reference_defense ${refDef.join(' / ')} at levels 1 / 50 / 99; built by tools/party_balance_sheet.js`
    : 'PORT DESIGN, as row reference.1';
  rows.push([`reference.${lv}`, 'reference', lv, 0, 0, Math.round(curve(refDef, lv)), Math.round(curve(refHp, lv)), src]);
}
const sheet = { sheet: 'party-balance-levels', group: 'port', columns: ['id', 'chara', 'level', 'strength', 'magic', 'defense', 'hp', 'source'], rows };
const out = path.join(PORT, 'party-balance-levels.json');
const text = JSON.stringify(sheet, null, 1) + '\n';
if (process.argv.includes('--write')) {
  fs.writeFileSync(out, text);
  console.log(`party_balance_sheet: wrote ${path.relative(ROOT, out)} (${rows.length} rows)`);
} else {
  const now = fs.existsSync(out) ? fs.readFileSync(out, 'utf8') : '';
  if (now !== text) {
    console.log('party_balance_sheet: sheets/port/party-balance-levels.json is NOT what the anchors give: run with --write');
    process.exit(1);
  }
  console.log('party_balance_sheet: the table matches the anchors');
}
