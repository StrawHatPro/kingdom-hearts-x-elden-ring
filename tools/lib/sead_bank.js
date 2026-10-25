// Square Enix SEAD sound bank ("sabf", .sab): snd / seq / trk / mtrl sections.
// Pure Node, read-only. Layouts and evaluation rules: build/sora_tmp/audio/bank/NOTES.md and
// FINDINGS.json (each field there carries its confidence and the exe address that reads it).
// Field names below are this project's; unknown bytes are kept as hex (`raw`, `unk_*`, `tail`).
//
//   parseBank(buffer)        whole .sab in memory
//   parseBankFile(path)      reads only the definition part and the material headers (low memory)
//   resolveSound(bank, i)    the tree "sound -> sequences -> commands -> tracks -> materials"
//   describeSelection(sound) the rule the driver applies when the sound is started
'use strict';
const fs = require('fs');

const SOUND_TYPES = { 0: 'none', 1: 'Sound', 2: 'RandomSound', 3: 'OrderSound', 4: 'CycleSound', 5: 'SwitchSound' };
const TRACK_TYPES = { 1: 'MaterialTrack', 2: 'SoundTrack' };
const CURVES = { 0: 'linear', 1: '1-(1-t^2)^3', 2: '1-(1-t)^2', 3: 't^2', 4: '2^t-1', 5: '2-2^(1-t)' };
const SEQ_OPS = {
  0: 'nop', 1: 'end', 2: 'keyOn', 3: 'wait', 4: 'keyOff', 5: 'waitRandom', 6: 'loopStart', 7: 'loopEnd', 8: 'waitZeroOne',
};
const TRK_OPS = {
  0: 'nop', 1: 'end', 2: 'wait', 3: 'volume', 4: 'pitch', 5: 'pan', 6: 'panDepth', 7: 'randomVolume', 8: 'randomPitch',
  9: 'randomPan', 10: 'randomPanDepth', 11: 'lfoVolume', 12: 'lfoPitch', 13: 'lfoPan', 14: 'lfoPanDepth', 15: 'lfeSend',
  16: 'centerMix', 17: 'panPath',
};
const ZEROONE_TARGETS = { 1: 'volume', 2: 'pitch', 3: 'trackParam', 5: 'lowpass', 6: 'auxSendDb' };
const CODECS = { 0: 'none', 1: 'pcm16', 2: 'msadpcm', 3: 'oggvorbis', 4: 'atrac9', 7: 'hca' };

const hex = (d, a, b) => d.toString('hex', a, Math.min(b, d.length));
const f32 = (d, o) => Math.fround(d.readFloatLE(o));
const cstr = (d, o, max) => { let e = o; const lim = Math.min(d.length, o + (max || 256)); while (e < lim && d[e] !== 0) e++; return d.toString('latin1', o, e); };

function parseHeader(d) {
  const magic = d.toString('latin1', 0, 4);
  if (magic !== 'sabf') throw new Error('not a sabf bank: ' + JSON.stringify(magic));
  const h = {
    magic, version: d[4], flags: d[5], headerSize: d.readUInt16LE(6), sectionCount: d[8], nameLength: d[9],
    number: d.readUInt16LE(10), fileSize: d.readUInt32LE(12),
  };
  h.name = d.toString('latin1', 16, 16 + h.nameLength);
  // exe 14596a570: table = headerSize (+ ((nameLength + 0x10) & 0xf0) when version >= 2)
  h.tableOffset = h.headerSize + (h.version >= 2 ? ((h.nameLength + 0x10) & 0xf0) : 0);
  h.sections = {};
  h.sectionList = [];
  for (let i = 0, o = h.tableOffset; i < h.sectionCount; i++, o += 16) {
    const s = { tag: d.toString('latin1', o, o + 4), word4: d.readUInt32LE(o + 4), offset: d.readUInt32LE(o + 8), word12: d.readUInt32LE(o + 12) };
    h.sectionList.push(s);
    h.sections[s.tag.trim()] = s;
  }
  return h;
}

// chunk header: u8 version, u8 ?, u16 headerSize (= offset of the u32 offset table), u16 count
function chunkOffsets(d, base) {
  const c = { version: d[base], byte1: d[base + 1], headerSize: d.readUInt16LE(base + 2), count: d.readUInt16LE(base + 4), raw: hex(d, base, base + d.readUInt16LE(base + 2)) };
  c.offsets = [];
  const t = base + c.headerSize;
  for (let i = 0; i < c.count; i++) c.offsets.push(d.readUInt32LE(t + i * 4));
  c.tableEnd = c.headerSize + c.count * 4;
  return c;
}

// A command (sequence and track streams share the frame): u8 version, u8 headerSize, u8 opcode, u8 bodySize.
function readCommands(d, start, end, kind) {
  const out = [];
  let p = start;
  let ended = false;
  while (p + 4 <= end) {
    const ver = d[p], hs = d[p + 1], op = d[p + 2], bs = d[p + 3];
    if (hs < 4 || p + hs + bs > end) break;
    const c = { at: p - start, version: ver, op, name: (kind === 'seq' ? SEQ_OPS : TRK_OPS)[op] || 'op' + op, size: hs + bs };
    if (hs !== 4) c.headerSize = hs;
    const b = p + 4; // operands are addressed from the command start in the exe (cmd+4 ...)
    const u32 = (o) => (p + o + 4 <= p + hs + bs ? d.readUInt32LE(p + o) : null);
    const fl = (o) => (p + o + 4 <= p + hs + bs ? f32(d, p + o) : null);
    const u8 = (o) => (p + o < p + hs + bs ? d[p + o] : null);
    if (kind === 'seq') {
      if (op === 2) { c.track = u32(4); c.loopFlag = u8(8); c.duration = fl(0xc); }
      else if (op === 3) c.ms = u32(4);
      else if (op === 5) { c.maxMs = u32(4); c.minMs = u32(8); }
      else if (op === 6) { c.maxCount = d.readUInt16LE(p + 4); c.minCount = d.readUInt16LE(p + 6); }
      else if (op === 8) { c.enable = u8(4); c.nameOffset = u8(5); c.compare = u8(8); c.threshold = fl(0xc); c.param = cstr(d, p + d[p + 5], 64); }
    } else {
      if (op === 2) c.ms = u32(4);
      else if ((op >= 3 && op <= 6) || op === 15 || op === 16) { c.target = fl(4); c.ms = u32(8); c.curve = ver !== 0 || op >= 15 ? u8(0xc) : 0; }
      else if (op >= 7 && op <= 10) { c.max = fl(4); c.min = fl(8); c.mode = u8(0xc); }
      else if (op >= 11 && op <= 14) { c.period = fl(4); c.depth = fl(8); c.wave = u8(0xc); }
      else if (op === 17) {
        c.p0 = [fl(4), fl(8)]; c.c1 = [fl(0xc), fl(0x10)]; c.c2 = [fl(0x14), fl(0x18)]; c.p3 = [fl(0x1c), fl(0x20)];
        c.ms = u32(0x24); c.mode = u8(0x28);
      }
    }
    c.raw = hex(d, b, p + hs + bs);
    out.push(c);
    p += hs + bs;
    if (op === 1) { ended = true; break; }
  }
  return { commands: out, end: p, ended };
}

function parseZeroOne(d, o, limit) {
  const z = { offset: o, version: d[o], byte1: d[o + 1], headerSize: d.readUInt16LE(o + 2), pointCount: d.readUInt16LE(o + 4), target: d[o + 6], targetIndex: d[o + 7] };
  z.targetName = ZEROONE_TARGETS[z.target] || 'target' + z.target;
  z.inMin = f32(d, o + 8); z.inMax = f32(d, o + 12);
  z.paramId = d.readUInt16LE(o + 0x10);
  z.flags = z.version >= 5 ? d[o + 0x14] : null;
  // input: version < 2 -> slot index byte1; 2..3 -> name at +0x20; >= 4 -> name at +headerSize (byte1 = name length)
  if (z.version < 2) z.slot = z.byte1;
  else z.name = cstr(d, o + (z.version < 4 ? 0x20 : z.headerSize), 64);
  const pts = o + z.headerSize + (z.version >= 4 ? ((z.byte1 + 0x10) & 0xf0) : 0);
  z.points = [];
  for (let i = 0; i < z.pointCount && pts + i * 16 + 16 <= limit; i++) {
    const q = pts + i * 16;
    z.points.push({ x: f32(d, q), y: f32(d, q + 4), curve: d[q + 8], raw: hex(d, q + 9, q + 16) });
  }
  z.raw = hex(d, o, o + Math.min(z.headerSize, 0x40));
  z.end = pts + z.pointCount * 16;
  return z;
}

function parseSound(d, o, end, index) {
  const v = d[o];
  const s = { index, offset: o, size: end - o, version: v, lastPlayed: d[o + 1], headerSize: d.readUInt16LE(o + 2), type: d[o + 4], typeName: SOUND_TYPES[d[o + 4]] || 'type' + d[o + 4], sequenceCount: d[o + 5] };
  s.category = d[o + 6]; s.priority = d[o + 7];
  s.number = d.readUInt16LE(o + 8);
  s.macroStart = d[o + 0xa]; s.macro1 = d[o + 0xb];
  s.volume = f32(d, o + 0xc);
  s.cycleIntervalMinMs = d.readUInt32LE(o + 0x10); s.cycleIntervalMaxMs = d.readUInt32LE(o + 0x14);
  s.cycleMaxActive = d[o + 0x18];
  s.flags = d[o + 0x19];
  s.sequenceInfoOffset = v < 6 ? s.headerSize : d.readUInt16LE(o + 0x1a);
  s.float1c = f32(d, o + 0x1c);
  s.mainBus = d[o + 0x20]; s.byte21 = d[o + 0x21]; s.byte22 = d[o + 0x22]; s.nameLength = d[o + 0x23];
  s.duration = f32(d, o + 0x24);
  s.float28 = f32(d, o + 0x28); s.float2c = f32(d, o + 0x2c);
  s.auxSendCount = d[o + 0x44];
  s.auxSends = [];
  for (let i = 0; i < 4; i++) s.auxSends.push({ bus: d[o + 0x30 + i], volume: f32(d, o + 0x34 + i * 4) });
  s.byte45 = d[o + 0x45];
  s.effectTableOffset = d.readUInt16LE(o + 0x46);
  s.unk48 = hex(d, o + 0x48, o + 0x4c);
  s.zeroOneParamOffset = d.readUInt16LE(o + 0x4c);
  s.zeroOneParamCount = d[o + 0x4e];
  s.macro3 = d[o + 0x4f];
  if (s.headerSize > 0x50) { s.panType = d[o + 0x50]; s.byte51 = d[o + 0x51]; s.unk52 = hex(d, o + 0x52, o + 0x54); s.panParam = f32(d, o + 0x54); s.unk58 = hex(d, o + 0x58, o + s.headerSize); }
  s.name = v > 8 ? cstr(d, o + s.headerSize, 256) : v > 2 ? cstr(d, o + 0x50, 16) : '';
  s.sequences = [];
  for (let i = 0; i < s.sequenceCount; i++) {
    const q = o + s.sequenceInfoOffset + i * 16;
    if (q + 16 > end) { s.error = 'sequence info outside the sound entry'; break; }
    s.sequences.push({ version: d[q], byte1: d[q + 1], size: d.readUInt16LE(q + 2), sequence: d.readUInt16LE(q + 4), number: d.readUInt16LE(q + 6), raw: hex(d, q + 8, q + 16) });
  }
  // per-sound zeroone slot parameters (0x20 bytes each), exe 14596acc0 / 145952f00
  s.zeroOneParams = [];
  for (let i = 0; i < s.zeroOneParamCount; i++) {
    const q = o + s.zeroOneParamOffset + i * 0x20;
    if (q + 0x20 > end) { s.error = 'zeroone param outside the sound entry'; break; }
    s.zeroOneParams.push({ byte0: d[q], enable: d[q + 1], upTime: f32(d, q + 0x10), downTime: f32(d, q + 0x14), raw: hex(d, q, q + 0x20) });
  }
  s.raw = hex(d, o, o + s.headerSize);
  const nameEnd = s.headerSize + (v > 8 ? ((s.nameLength + 0x10) & 0xf0) : 0);
  // SwitchSound (type 5, version > 9): the switch variable block follows the name (exe 145952b10)
  let switchEnd = nameEnd;
  if (s.type === 5 && v > 9 && o + nameEnd + 4 <= end) {
    const w = o + nameEnd;
    const wn = d[w] === 0 ? 0x10 : d.readUInt16LE(w + 2);
    s.switchParam = { version: d[w], nameLength: d[w + 1], headerSize: d.readUInt16LE(w + 2), name: cstr(d, w + wn, 64), raw: hex(d, w, w + wn) };
    switchEnd = nameEnd + wn + ((s.switchParam.nameLength + 0x10) & 0xf0);
  }
  // whatever lies between the sequence infos and the end of the entry besides the zeroone params
  const used = [[0, switchEnd], [s.sequenceInfoOffset, s.sequenceInfoOffset + 16 * s.sequenceCount], [s.zeroOneParamOffset, s.zeroOneParamOffset + 0x20 * s.zeroOneParamCount]];
  s.extra = gaps(d, o, end, used);
  return s;
}

// byte ranges of [o,end) not covered by `used` (relative ranges) and not all zero
function gaps(d, o, end, used) {
  used = used.filter((u) => u[1] > u[0]).sort((a, b) => a[0] - b[0]);
  const out = [];
  let p = 0;
  const push = (a, b) => { if (b <= a) return; let nz = false; for (let i = o + a; i < o + b && i < d.length; i++) if (d[i] !== 0) { nz = true; break; } if (nz) out.push({ at: a, hex: hex(d, o + a, o + b) }); };
  for (const [a, b] of used) { push(p, a); p = Math.max(p, b); }
  push(p, end - o);
  return out;
}

function parseSequence(d, o, end, index) {
  const v = d[o];
  const q = { index, offset: o, size: end - o, version: v, byte1: d[o + 1], headerSize: d.readUInt16LE(o + 2), number: d.readUInt16LE(o + 4) };
  // exe 14596a390: command stream offset by version
  q.commandOffset = v === 0 ? d.readUInt16LE(o + 2) : v === 1 ? d.readUInt16LE(o + 0xe) : v === 2 ? d.readUInt16LE(o + 0x16) : d.readUInt16LE(o + 6);
  // exe 14596a620: zeroone count (2 fixed below version 3)
  q.zeroOneCount = v < 3 ? 2 : d[o + 8];
  // selection fields share bytes: weight/fixed flag + group (Random/Order/Cycle), or a float range (Switch)
  const sel = v >= 3 ? 0x10 : 4;
  q.weight = d[o + sel]; q.group = d[o + sel + 1];
  q.switchMin = v < 2 ? 0 : f32(d, o + (v === 2 ? 4 : 0x10));
  q.switchMax = v < 2 ? 0 : f32(d, o + (v === 2 ? 8 : 0x14));
  q.raw = hex(d, o, o + Math.min(q.commandOffset, 0x40));
  q.zeroOnes = [];
  const used = [[0, v >= 3 ? 0x20 : q.commandOffset]];
  if (v >= 3) {
    used.push([q.headerSize, q.headerSize + 4 * q.zeroOneCount]);
    for (let i = 0; i < q.zeroOneCount; i++) {
      const zo = d.readUInt32LE(o + q.headerSize + i * 4);
      if (o + zo + 0x18 > end) { q.error = 'zeroone outside the sequence entry'; break; }
      const z = parseZeroOne(d, o + zo, end);
      z.offset = zo;
      used.push([zo, z.end - o]);
      delete z.end;
      q.zeroOnes.push(z);
    }
  }
  const r = readCommands(d, o + q.commandOffset, end, 'seq');
  q.commands = r.commands;
  if (!r.ended) q.error = (q.error ? q.error + '; ' : '') + 'command stream does not reach an end command';
  used.push([q.commandOffset, r.end - o]);
  q.extra = gaps(d, o, end, used);
  return q;
}

function parseTrack(d, o, end, index) {
  const v = d[o];
  const t = { index, offset: o, size: end - o, version: v, type: d[o + 1], typeName: TRACK_TYPES[d[o + 1]] || 'type' + d[o + 1], headerSize: d.readUInt16LE(o + 2) };
  // exe 1459b24a0 (MaterialTrack: +4 material index, +6 bank number); 1459b2b60 (SoundTrack: +4 = index of a
  // sound in the same bank, played as a child); 1459b0450: command offset = +2 (version < 2) or +0xe
  t.ref = d.readUInt16LE(o + 4);
  t.bankNumber = d.readUInt16LE(o + 6);
  t.word8 = d.readUInt16LE(o + 8); t.wordA = d.readUInt16LE(o + 0xa); t.wordC = d.readUInt16LE(o + 0xc);
  t.commandOffset = v < 2 ? d.readUInt16LE(o + 2) : d.readUInt16LE(o + 0xe);
  t.raw = hex(d, o, o + Math.min(t.commandOffset, 0x40));
  const r = readCommands(d, o + t.commandOffset, end, 'trk');
  t.commands = r.commands;
  if (!r.ended) t.error = 'command stream does not reach an end command';
  t.extra = gaps(d, o, end, [[0, t.commandOffset], [t.commandOffset, r.end - o]]);
  return t;
}

// Material entry header (same layout as tools/lib/sead.js surveyBank); `m` holds at least 0x60 bytes.
function parseMaterial(m, index, entryOffset) {
  const e = {
    index, entryOffset, version: m[0], byte1: m[1], headerSize: m.readUInt16LE(2), channels: m[4], codec: m[5], codecName: CODECS[m[5]] || 'codec' + m[5],
    number: m.readUInt16LE(6), sampleRate: m.readUInt32LE(8), loopStart: m.readUInt32LE(12), loopEnd: m.readUInt32LE(16),
    extraSize: m.readUInt32LE(20), streamSize: m.readUInt32LE(24), word28: m.readUInt16LE(28), word30: m.readUInt16LE(30),
  };
  e.looping = e.loopEnd > e.loopStart; // exe 145982010: loop taken when loopStart < loopEnd
  e.dataOffset = entryOffset + e.headerSize + e.extraSize;
  if (e.codec === 7 && m.length >= 0x60 && m.toString('latin1', 0x30, 0x33) === 'HCA') {
    e.hca = { extraHead: hex(m, 0x20, 0x30), version: m.readUInt16BE(0x34), headerSize: m.readUInt16BE(0x36) };
    // fmt chunk: channels(1) rate(3) blockCount(4) encoderDelay(2) padding(2)
    if (m.toString('latin1', 0x38, 0x3b) === 'fmt') {
      e.hca.channels = m[0x3c]; e.hca.sampleRate = m.readUIntBE(0x3d, 3); e.hca.blockCount = m.readUInt32BE(0x40);
      e.hca.encoderDelay = m.readUInt16BE(0x44); e.hca.padding = m.readUInt16BE(0x46);
      e.hca.samples = e.hca.blockCount * 1024 - e.hca.encoderDelay - e.hca.padding;
      e.duration = e.hca.samples / (e.hca.sampleRate || 1);
    }
  }
  return e;
}

function parseDefinitions(d, h) {
  const bank = { header: h, sounds: [], sequences: [], tracks: [], materials: [], problems: [] };
  const sec = (t) => h.sections[t];
  const order = h.sectionList.slice().sort((a, b) => a.offset - b.offset);
  const secEnd = (s) => { const i = order.indexOf(s); return i + 1 < order.length ? order[i + 1].offset : h.fileSize; };
  for (const [tag, list, fn] of [['snd', 'sounds', parseSound], ['seq', 'sequences', parseSequence], ['trk', 'tracks', parseTrack]]) {
    const s = sec(tag);
    if (!s) { bank.problems.push('no ' + tag + ' section'); continue; }
    const c = chunkOffsets(d, s.offset);
    bank[tag + 'Chunk'] = { version: c.version, byte1: c.byte1, headerSize: c.headerSize, count: c.count, raw: c.raw };
    const end = secEnd(s);
    // an entry ends where the next larger offset begins (entries are stored in index order in every bank seen)
    const sorted = c.offsets.slice().sort((a, b) => a - b);
    for (let i = 0; i < c.count; i++) {
      const off = c.offsets[i];
      const k = sorted.indexOf(off);
      let e = end - s.offset;
      for (let j = k + 1; j < sorted.length; j++) if (sorted[j] > off) { e = sorted[j]; break; }
      try {
        const x = fn(d, s.offset + off, s.offset + e, i);
        if (x.error) bank.problems.push(`${tag} ${i}: ${x.error}`);
        bank[list].push(x);
      } catch (err) { bank.problems.push(`${tag} ${i}: ${err.message}`); bank[list].push({ index: i, offset: s.offset + off, error: err.message }); }
    }
    if (c.count && sorted[0] !== ((c.tableEnd + 15) & ~15)) bank.problems.push(`${tag}: first entry at ${sorted[0]}, table ends at ${c.tableEnd}`);
  }
  return bank;
}

function parseBank(d) {
  const h = parseHeader(d);
  const bank = parseDefinitions(d, h);
  const m = h.sections.mtrl;
  if (m) {
    const c = chunkOffsets(d, m.offset);
    bank.mtrlChunk = { version: c.version, byte1: c.byte1, headerSize: c.headerSize, count: c.count, raw: c.raw };
    for (let i = 0; i < c.count; i++) { const e = m.offset + c.offsets[i]; bank.materials.push(parseMaterial(d.subarray(e, Math.min(d.length, e + 0x60)), i, e)); }
  }
  return bank;
}

function parseBankFile(file) {
  const fd = fs.openSync(file, 'r');
  try {
    const rd = (pos, len) => { const b = Buffer.alloc(len); const n = fs.readSync(fd, b, 0, len, pos); return n < len ? b.subarray(0, n) : b; };
    let head = rd(0, 0x100);
    const h = parseHeader(head);
    const m = h.sections.mtrl;
    const defEnd = m ? m.offset + 16 : h.fileSize;
    head = rd(0, defEnd);
    const bank = parseDefinitions(head, h);
    if (m) {
      const ch = rd(m.offset, 16);
      const count = ch.readUInt16LE(4), hs = ch.readUInt16LE(2);
      const tab = rd(m.offset + hs, count * 4);
      bank.mtrlChunk = { version: ch[0], byte1: ch[1], headerSize: hs, count, raw: ch.toString('hex', 0, hs) };
      for (let i = 0; i < count; i++) { const e = m.offset + tab.readUInt32LE(i * 4); bank.materials.push(parseMaterial(rd(e, 0x60), i, e)); }
    }
    return bank;
  } finally { fs.closeSync(fd); }
}

// The rule the driver applies when a sound of this type is started (see NOTES.md "Evaluation").
function describeSelection(bank, s) {
  const seqs = s.sequences.map((r) => bank.sequences[r.sequence]).filter(Boolean);
  const groups = [];
  for (const q of seqs) { const g = groups[groups.length - 1]; if (g && g.group === q.group) g.members.push(q.index); else groups.push({ group: q.group, weight: q.weight, members: [q.index] }); }
  switch (s.type) {
    case 1: return { rule: 'all', start: seqs.map((q) => q.index) };
    case 2: return { rule: 'random', always: groups.filter((g) => g.weight === 0), choose: groups.filter((g) => g.weight !== 0), noRepeat: !(s.flags & 0x20) };
    case 3: return { rule: 'order', always: groups.filter((g) => g.weight !== 0), rotate: groups.filter((g) => g.weight === 0) };
    case 4: return { rule: 'cycle', always: groups.filter((g) => g.weight === 0), choose: groups.filter((g) => g.weight !== 0), intervalMs: [s.cycleIntervalMinMs, s.cycleIntervalMaxMs], maxActive: s.cycleMaxActive };
    case 5: return { rule: 'switch', ranges: seqs.map((q) => ({ sequence: q.index, min: q.switchMin, max: q.switchMax })) };
    default: return { rule: 'none' };
  }
}

function resolveSound(bank, i) {
  const s = bank.sounds[i];
  if (!s) return null;
  const tree = { index: i, name: s.name, type: s.typeName, selection: describeSelection(bank, s), sequences: [] };
  for (const r of s.sequences || []) {
    const q = bank.sequences[r.sequence];
    if (!q) { tree.sequences.push({ sequence: r.sequence, missing: true }); continue; }
    const node = { sequence: q.index, weight: q.weight, group: q.group, zeroOnes: q.zeroOnes, commands: [] };
    for (const c of q.commands || []) {
      const cc = { ...c };
      if (c.op === 2) {
        const t = bank.tracks[c.track];
        if (t) {
          cc.trackInfo = { type: t.typeName, ref: t.ref, bankNumber: t.bankNumber, commands: t.commands };
          if (t.type === 1 && t.bankNumber === bank.header.number) cc.trackInfo.material = bank.materials[t.ref] || null;
          if (t.type === 2) cc.trackInfo.sound = t.ref; // SoundTrack: child sound index in the same bank (exe 1459b2b60)
        }
      }
      node.commands.push(cc);
    }
    tree.sequences.push(node);
  }
  return tree;
}

module.exports = { parseBank, parseBankFile, parseHeader, parseMaterial, resolveSound, describeSelection, SOUND_TYPES, TRACK_TYPES, SEQ_OPS, TRK_OPS, CURVES, ZEROONE_TARGETS, CODECS };
