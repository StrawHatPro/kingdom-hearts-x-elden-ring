// (GOOFY-BAL) THE FRIEND LEADERS' STAT TABLE: sheets/port/party-balance-levels.json, built from Sora's level table
// (sheets/port/sora-levels.json: attack / magic / defense at each level) times the ratio rows of
// sheets/port/party-balance.json (n_ex00x.lead.strength_ratio / magic_ratio / defense_ratio). The ratio is of SORA'S
// STAT WITH HIS STARTING KEYBLADE (Kingdom Key, sora-equipment WEP_KEYBLADE_SO_00) and a row stores it minus the
// friend's starting weapon (party-weapons WEP_DONALD_00 / WEP_GOOFY_00): with the starting weapons the leader's stat is
// ratio x Sora's (rounded), and a better weapon adds on top as it does for Sora. A ratio of 0 is 0 (no such stat).
// The table is what the code reads (crates/kh-party/src/balance.rs through tools/gen_port.js).
//   node tools/party_balance_sheet.js           check: the table on disk equals what the ratios give (exit 1 if not)
//   node tools/party_balance_sheet.js --write   write it
// Design and numbers: build/sora_tmp/party/BAL_DESIGN.md (stream BAL; the user, 2026-10-16 23:50).
const fs = require('fs');
const path = require('path');
const ROOT = path.join(__dirname, '..');
const PORT = path.join(ROOT, 'sheets', 'port');
const read = (f) => JSON.parse(fs.readFileSync(path.join(PORT, f), 'utf8'));
const params = Object.fromEntries(read('party-balance.json').rows.map((r) => [r[0], r[1]]));
const sora = read('sora-levels.json');
const col = (name) => sora.columns.indexOf(name);
const [cLv, cAtk, cMag, cDef] = ['id', 'attack', 'magic', 'defense'].map(col);
const equip = read('sora-equipment.json');
const ecol = (name) => equip.columns.indexOf(name);
const kk = equip.rows.find((r) => r[0] === 'WEP_KEYBLADE_SO_00');
const KK = [kk[ecol('attack')], kk[ecol('magic')], kk[ecol('defense')]];
const weapons = read('party-weapons.json');
const wcol = (name) => weapons.columns.indexOf(name);
const start = (key) => {
  const w = weapons.rows.find((r) => r[wcol('key')] === key);
  return [w[wcol('strength')], w[wcol('magic')], w[wcol('defense')]];
};
const START = { n_ex001: start('WEP_DONALD_00'), n_ex002: start('WEP_GOOFY_00') };
const rows = [];
for (const chara of ['n_ex001', 'n_ex002']) {
  const ratio = (stat) => {
    const v = params[`${chara}.lead.${stat}_ratio`];
    if (typeof v !== 'number') throw new Error(`party-balance has no number ${chara}.lead.${stat}_ratio`);
    return v;
  };
  const [rs, rm, rd] = ['strength', 'magic', 'defense'].map(ratio);
  const w = START[chara];
  const stat = (r, value, i) => (r === 0 ? 0 : Math.max(0, Math.round(r * (value + KK[i])) - w[i]));
  for (let lv = 1; lv <= 99; lv++) {
    const s = sora.rows.find((r) => String(r[cLv]) === String(lv));
    if (!s) throw new Error(`sora-levels has no row ${lv}`);
    const src = lv === 1
      ? `PORT DESIGN (stream BAL; THE USER 2026-10-16 23:50, ER_PHASE_NOTES "balance Donald and Goofy like Sora"): round(ratio x (Sora's sora-levels row + the Kingdom Key ${KK.join(' / ')})) - his starting weapon ${w.join(' / ')} (Strength / Magic / Defense), with party-balance ${chara}.lead.strength_ratio ${rs} / magic_ratio ${rm} / defense_ratio ${rd}; built by tools/party_balance_sheet.js`
      : `PORT DESIGN (stream BAL, as row ${chara}.1): Sora's row ${lv} = ${s[cAtk]} / ${s[cMag]} / ${s[cDef]}`;
    rows.push([`${chara}.${lv}`, chara, lv, stat(rs, s[cAtk], 0), stat(rm, s[cMag], 1), stat(rd, s[cDef], 2), src]);
  }
}
const sheet = { sheet: 'party-balance-levels', group: 'port', columns: ['id', 'chara', 'level', 'strength', 'magic', 'defense', 'source'], rows };
const out = path.join(PORT, 'party-balance-levels.json');
const text = JSON.stringify(sheet, null, 1) + '\n';
if (process.argv.includes('--write')) {
  fs.writeFileSync(out, text);
  console.log(`party_balance_sheet: wrote ${path.relative(ROOT, out)} (${rows.length} rows)`);
} else {
  const now = fs.existsSync(out) ? fs.readFileSync(out, 'utf8') : '';
  if (now !== text) {
    console.log('party_balance_sheet: sheets/port/party-balance-levels.json is NOT what the ratios give: run with --write');
    process.exit(1);
  }
  console.log('party_balance_sheet: the table matches the ratios');
}
