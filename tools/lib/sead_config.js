// SEAD global config ("scnf"): categories, category layers, macros, buses, external parameters.
// Source: [SQEXSEAD] ConfigString in assets/KINGDOM HEARTS III/Config/Windows/WindowsEngine.ini
// (3 decimal digits per byte; read by the UE plugin at exe 145814bc0 and handed to SEAD init 14594ba20).
//   node tools/lib/sead_config.js  -> writes build/gen/audio/sead_config.bin and sead_config.json
//   require(...).parseConfig(buf)  -> object
'use strict';
const fs = require('fs'), path = require('path');
const R = path.resolve(__dirname, '../..');
const INI = path.join(R, 'assets/KINGDOM HEARTS III/Config/Windows/WindowsEngine.ini');

function extract() {
  const ini = fs.readFileSync(INI, 'latin1');
  const m = /^ConfigString=(\d+)/m.exec(ini);
  if (!m) throw new Error('ConfigString not found');
  const s = m[1], b = Buffer.alloc(Math.floor(s.length / 3));
  for (let i = 0; i < b.length; i++) b[i] = +s.substr(i * 3, 3);
  return b;
}
const cstr = (d, o, max) => { let e = o; const lim = Math.min(d.length, o + (max || 128)); while (e < lim && d[e] !== 0) e++; return d.toString('latin1', o, e); };
const CMP = ['==', '!=', '>', '>=', '<', '<='];
const MACRO_OPS = {
  0x01: 'end', 0x10: 'resetSet', 0x11: 'filterCategory', 0x12: 'filterSoundId', 0x13: 'filterPriority', 0x14: 'filterMacro', 0x15: 'filterBankNumber',
  0x16: 'filterElapsed', 0x17: 'filterVolume', 0x18: 'filterPan', 0x19: 'filter3dKind', 0x1a: 'filterPosition',
  0xa0: 'stopOthers', 0xa1: 'cancelSelf', 0xa2: 'categoryVolume', 0xa3: 'othersVolume', 0xa4: 'selfVolume', 0xa5: 'selfDelay', 0xa6: 'selfSwitchOrSeek',
  0xa7: 'categoryPitch', 0xa8: 'categoryOpA8', 0xa9: 'categoryOpA9', 0xaa: 'othersOpAA', 0xab: 'othersOpAB',
};
const STOP_TARGET = ['all in set', 'first in set', 'quietest in set (else cancel self)', 'farthest in set (else cancel self)'];

function macroCommands(d, o, end) {
  const out = [];
  let p = o;
  while (p + 4 <= end) {
    const hs = d[p + 1], op = d[p + 2], bs = d[p + 3];
    if (hs < 4) break;
    const c = { op, name: MACRO_OPS[op] || 'op' + op.toString(16), raw: d.toString('hex', p + 4, p + hs + bs) };
    if (op >= 0x11 && op <= 0x15) { c.cmp = CMP[d[p + 4]]; c.relativeToSelf = d[p + 5] === 1; c.value = d.readInt32LE(p + 8); }
    else if (op === 0x16 || op === 0x17) { c.cmp = CMP[d[p + 4]]; c.mode = d[p + 5]; c.value = d.readFloatLE(p + 8); }
    else if (op >= 0xa0) {
      c.ifCount = CMP[d[p + 4]] + ' ' + d[p + 5]; c.arg = d[p + 6];
      if (bs >= 8) c.f1 = d.readFloatLE(p + 8);
      if (bs >= 12) c.f2 = d.readFloatLE(p + 12);
      if (op === 0xa0) { c.target = STOP_TARGET[d[p + 6]] || d[p + 6]; c.fadeSeconds = c.f1; }
    }
    out.push(c);
    p += hs + bs;
    if (op === 1) break;
  }
  return out;
}

function parseConfig(d) {
  if (d.toString('latin1', 0, 4) !== 'scnf') throw new Error('not scnf');
  const cfg = { magic: 'scnf', version: d[4], headerSize: d.readUInt16LE(6), sectionCount: d[8], fileSize: d.readUInt32LE(12), sections: [] };
  for (let i = 0; i < cfg.sectionCount; i++) { const o = cfg.headerSize + i * 16; cfg.sections.push({ tag: d.toString('latin1', o, o + 4).trim(), offset: d.readUInt32LE(o + 8) }); }
  const sorted = cfg.sections.slice().sort((a, b) => a.offset - b.offset);
  const chunk = (tag) => {
    const s = cfg.sections.find((x) => x.tag === tag);
    if (!s) return null;
    const end = (sorted[sorted.indexOf(s) + 1] || { offset: d.length }).offset;
    const b = s.offset, hs = d.readUInt16LE(b + 2), n = d.readUInt16LE(b + 4);
    const entries = [];
    for (let i = 0; i < n; i++) { const o = b + d.readUInt32LE(b + hs + i * 4); const e = i + 1 < n ? b + d.readUInt32LE(b + hs + (i + 1) * 4) : end; entries.push([o, e]); }
    return { base: b, end, version: d[b], count: n, entries };
  };
  let c = chunk('ctgr');
  // CONFIGCATEGORY: u8 version, u8 number, u16 headerSize, i16 maxSounds, u16 nameLength; name at headerSize
  cfg.categories = c.entries.map(([o]) => {
    const max = d.readInt16LE(o + 4);
    return { number: d[o + 1], name: cstr(d, o + d.readUInt16LE(o + 2)), maxSounds: max > 0 ? max : 0, sharesPoolOf: max < 0 ? -max : null, exists: max !== 0, raw: d.toString('hex', o, o + 16) };
  });
  c = chunk('ctgl');
  cfg.categoryLayers = c ? c.entries.map(([o]) => ({ number: d[o + 1], name: cstr(d, o + d.readUInt16LE(o + 2)) })) : [];
  c = chunk('mcr');
  cfg.macros = c.entries.map(([o, e]) => ({ number: d[o + 1], commands: macroCommands(d, o + d.readUInt16LE(o + 2), e) }));
  c = chunk('bus');
  cfg.buses = c.entries.map(([o, e]) => ({ number: d[o + 1], name: cstr(d, o + d.readUInt16LE(o + 2)), volume: d.readFloatLE(o + 0xc), raw: d.toString('hex', o, Math.min(e, o + 0x20)) }));
  c = chunk('expr');
  cfg.externalParameters = c.entries.map(([o]) => ({ name: cstr(d, o + d.readUInt16LE(o + 2)), smoothing: d[o + 4], upSeconds: d.readFloatLE(o + 0x10), downSeconds: d.readFloatLE(o + 0x14), raw: d.toString('hex', o, o + 0x20) }));
  for (const tag of ['curv', 'zeon', 'evt', 'eps', 'epsp', 'hcam', 'spka', 'pext']) {
    const s = cfg.sections.find((x) => x.tag === tag);
    if (!s) continue;
    const end = (sorted[sorted.indexOf(s) + 1] || { offset: d.length }).offset;
    cfg['raw_' + tag] = { offset: s.offset, size: end - s.offset, head: d.toString('hex', s.offset, Math.min(end, s.offset + 0x50)) };
  }
  return cfg;
}

module.exports = { extract, parseConfig, MACRO_OPS };
if (require.main === module) {
  const b = extract();
  fs.writeFileSync(path.join(R, 'build', 'gen', 'audio', 'sead_config.bin'), b);
  const cfg = parseConfig(b);
  fs.writeFileSync(path.join(R, 'build', 'gen', 'audio', 'sead_config.json'), JSON.stringify(cfg, null, 1));
  console.log('bytes', b.length, 'categories', cfg.categories.filter((x) => x.exists).length + '/' + cfg.categories.length, 'macros', cfg.macros.length, 'layers', cfg.categoryLayers.map((x) => x.name).join(','), 'buses', cfg.buses.map((x) => x.number + ':' + x.name).join(','));
  for (const m of cfg.macros) if (m.commands.length > 1) console.log(String(m.number).padStart(3), m.commands.filter((x) => x.op !== 1).map((x) => x.name + (x.cmp ? `(${x.relativeToSelf ? 'self-other' : 'other'} ${x.cmp} ${typeof x.value === 'number' ? +x.value.toPrecision(4) : ''})` : x.ifCount ? `[if n ${x.ifCount}${x.target ? '; ' + x.target : ''}${x.arg && !x.target ? '; arg ' + x.arg : ''}${x.f1 !== undefined ? '; ' + +x.f1.toPrecision(4) : ''}${x.f2 !== undefined ? ', ' + +x.f2.toPrecision(4) : ''}]` : '')).join(' -> '));
}
