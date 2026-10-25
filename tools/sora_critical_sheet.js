// (DF, 2026-10-15) Writes sheets/port/sora-critical-start.json: the victory-bonus rows whose ABILITY Critical mode gives
// Sora AT THE START instead of with the bonus (KH3 Critical; the user: "the overall difficulty of the mod should be like
// Kingdom Hearts critical mode").
//
// THE DATA [A]: TresVBonusData rows with m_InitAbilityEquipOnCriticalMode != NONE (EQUIP = owned and equipped, HAVE = owned).
// THE CODE [C]: 0x140586280 (decode a bonus row for a character) case 1 skips the ability of such a row when the game level
// is Critical: on Critical the row gives only its stat bonuses (docs/sora-stats-spec.md 2.4). The code that grants the
// abilities at a Critical start was NOT located (spec open question 2): the start grant is INFERRED from the field's name
// ("init ability equip on critical mode") and the skip, which would otherwise leave a Critical player without them.
// One copy per row (two rows each give Air Slide and Superslide). The ap column is the ability's m_UseAP (sora-abilities).
//
//   node tools/sora_critical_sheet.js          write the sheet
//   node tools/sora_critical_sheet.js --check  exit 1 if the sheet on disk differs
const fs = require('fs'), path = require('path');
const VERSION = '1.1.0';
const R = path.resolve(__dirname, '..');
const T_VB = 'Load/Tres/TresVBonusData.json';
const vb = JSON.parse(fs.readFileSync(path.join(R, 'sheets/entities', T_VB), 'utf8')).rows;
const abil = JSON.parse(fs.readFileSync(path.join(R, 'sheets/port/sora-abilities.json'), 'utf8'));
const ai = (n) => abil.columns.indexOf(n);
const byName = new Map(abil.rows.map((r) => [r[0], r]));
const short = (v) => (typeof v === 'string' ? v.replace(/^E[A-Za-z0-9_]+::/, '') : v);
const comment = (c) => (c && typeof c === 'object' ? c.s : c) || '-';

// (DF, 2026-10-16) PORT EXCLUSIONS (decision dec032; the user: "don't start with critical counter"): KH3 data rows kept, column
// port = excluded: the ability is not part of the port's Critical start (neither owned nor equipped); Sora gets it by other means.
const EXCLUDED = new Map([['CRITICAL_COUNTER', 'dec032: the user, 2026-10-16: "don\'t start with critical counter"; Sora gets it later (the new abilities plan)']]);
const T_INIT = 'Load/Common/TresChrInitData.json';
const init = JSON.parse(fs.readFileSync(path.join(R, 'sheets/entities', T_INIT), 'utf8'));
const sora = ((init.rows || init.objects)[0]).m_PlayerSora.props;
const initList = (name) => { const p = sora.find((x) => x.tag.name === name); return p && p.value.items ? p.value.items.map(short) : []; };
const columns = ['id', 'group', 'ability', 'mode', 'ap', 'name', 'order', 'port', 'source', 'ref_addr', 'ref_conf', 'note'];
const rows = [];
const port = (ability) => (EXCLUDED.has(ability) ? 'excluded' : 'in');
const portNote = (ability) => (EXCLUDED.has(ability) ? '; PORT: EXCLUDED (' + EXCLUDED.get(ability) + ')' : '');
for (const [list, mode] of [['m_CriticalEquipAbility', 'EQUIP'], ['m_CriticalHaveAbility', 'HAVE']]) {
  for (const ability of initList(list)) {
    const row = byName.get(ability);
    if (!row) throw new Error('sora-critical-start: ' + list + ' names ' + ability + ', not a row of sora-abilities');
    rows.push([list + '.' + ability, 'init', ability, mode, row[ai('ap')], row[ai('name')], rows.length + 1, port(ability),
      'sheets/entities/' + T_INIT + ' m_PlayerSora.' + list, '0x140E92280', 'inferred',
      'KH3 Critical start set (with m_CriticalUseAPUpNum 30 AP); granted by Status::from_loadout on Critical' + portNote(ability)]);
  }
}
for (const r of vb) {
  const mode = short(r.m_InitAbilityEquipOnCriticalMode);
  if (!mode || mode === 'NONE') continue;
  const ability = short(r.m_AbilitySora1);
  const row = byName.get(ability);
  if (!row) throw new Error('sora-critical-start: ' + r._row_name + ' names ' + ability + ', not a row of sora-abilities');
  if (short(r.m_AbilitySora2) !== 'NONE') throw new Error('sora-critical-start: ' + r._row_name + ' has a second Sora ability; the sheet holds one');
  rows.push([r._row_name, 'vbonus', ability, mode, row[ai('ap')], row[ai('name')], rows.length + 1, port(ability),
    'sheets/entities/' + T_VB + ' ' + r._row_name + ' m_InitAbilityEquipOnCriticalMode ' + mode + ', m_AbilitySora1 (' + comment(r.m_Comment) + ')',
    '0x140586280', 'inferred',
    'Critical: the row\'s ability is skipped when the bonus is won (0x140586280 case 1) and given at the start (' + (mode === 'EQUIP' ? 'owned and equipped while the AP allow, in row order' : 'owned, not equipped') + '); the start grant itself was not located in the code' + portNote(ability)]);
}
if (rows.length === 0) throw new Error('sora-critical-start: no flagged rows');
const out = JSON.stringify({ sheet: 'sora-critical-start', group: 'port', columns }).replace(/\}$/, ',"rows":[\n') + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n';
const file = path.join(R, 'sheets/port/sora-critical-start.json');
if (process.argv.includes('--check')) {
  const same = fs.existsSync(file) && fs.readFileSync(file, 'utf8') === out;
  console.log(same ? 'sora-critical-start: ' + rows.length + ' rows, current' : 'sora-critical-start: sheet differs from tools/sora_critical_sheet.js ' + VERSION);
  process.exit(same ? 0 : 1);
}
fs.writeFileSync(file, out);
console.log('sora-critical-start: ' + rows.length + ' rows written (tools/sora_critical_sheet.js ' + VERSION + ')');
