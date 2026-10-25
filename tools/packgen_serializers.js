// (PACKGEN) The JSON layouts the pack tools write, by name: tools/packgen_recipe.js records which one gives a file's bytes,
// tools/packgen_replay.js writes the file with it.
'use strict';
// tools/sora_vfx_pack.js stringify(): objects one member per line, arrays of numbers / small flat objects on one line
function vfx(v, ind = '') {
  if (v === null || typeof v !== 'object') return JSON.stringify(v === undefined ? null : v);
  const flat = (x) => x === null || typeof x !== 'object' || (Array.isArray(x) && x.every((y) => y === null || typeof y !== 'object'));
  if (Array.isArray(v)) {
    if (!v.length) return '[]';
    if (v.every(flat)) return JSON.stringify(v);
    return '[\n' + v.map((x) => ind + ' ' + vfx(x, ind + ' ')).join(',\n') + '\n' + ind + ']';
  }
  const keys = Object.keys(v).filter((k) => v[k] !== undefined);
  if (!keys.length) return '{}';
  if (keys.every((k) => flat(v[k]))) { const t = JSON.stringify(v); if (t.length <= 240) return t; }
  return '{\n' + keys.map((k) => ind + ' ' + JSON.stringify(k) + ': ' + vfx(v[k], ind + ' ')).join(',\n') + '\n' + ind + '}';
}
// a sheet / row list: one row per line
const rows = (v) => (Array.isArray(v) ? '[\n' + v.map((r) => JSON.stringify(r)).join(',\n') + '\n]' : null);
const sheet = (v) => (v && Array.isArray(v.rows) && Array.isArray(v.columns) ? '{' + Object.keys(v).filter((k) => k !== 'rows').map((k) => JSON.stringify(k) + ':' + JSON.stringify(v[k])).join(',') + ',"rows":[\n' + v.rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}' : null);
const FORMATS = {
  vfx: (v) => vfx(v) + '\n',
  json1n: (v) => JSON.stringify(v, null, 1) + '\n',
  json1: (v) => JSON.stringify(v, null, 1),
  json2n: (v) => JSON.stringify(v, null, 2) + '\n',
  json2: (v) => JSON.stringify(v, null, 2),
  json0n: (v) => JSON.stringify(v) + '\n',
  json0: (v) => JSON.stringify(v),
  rowsn: (v) => { const t = rows(v); return t === null ? null : t + '\n'; },
  rows: (v) => rows(v),
  sheetn: (v) => { const t = sheet(v); return t === null ? null : t + '\n'; },
  vfx0: (v) => vfx(v),
};
function serialize(v, fmt) { const f = FORMATS[fmt]; if (!f) throw new Error('unknown layout ' + fmt); return f(v); }
function detect(v, text) { for (const [name, f] of Object.entries(FORMATS)) { let t; try { t = f(v); } catch { continue; } if (t === text) return name; } return null; }
module.exports = { serialize, detect, FORMATS };
