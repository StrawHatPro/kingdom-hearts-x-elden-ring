// How much of .text does the exception table explain, and what is in the gaps between entries?
const path = require('path');
const { ROOT, logRun, writeJson, hex } = require('./lib/log');
const pe = require('./lib/pe');

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'audit', 'triage', 'text_gaps.json');
const img = pe.load();
const text = img.sections[0];
const fns = pe.runtimeFunctions(img);
const b = img.b;

const stat = { pdata_entries: fns.length, chained_fragments: fns.filter((f) => f.root !== f.begin).length,
  root_functions: new Set(fns.map((f) => f.root)).size, gaps: 0, gap_bytes: 0,
  gap_kinds: { int3: { n: 0, bytes: 0 }, zero: { n: 0, bytes: 0 }, int3_then_other: { n: 0, bytes: 0 }, other: { n: 0, bytes: 0 } },
  other_gap_size_histogram: {}, largest_other_gaps: [] };
const others = [];
let prev = text.rva;
const end = text.rva + text.vsize;
function gap(s, e) {
  if (e <= s) return;
  stat.gaps++;
  stat.gap_bytes += e - s;
  const o = img.rvaToOff(s);
  let cc = 0;
  while (cc < e - s && b[o + cc] === 0xcc) cc++;
  let zero = 0;
  while (zero < e - s && b[o + zero] === 0) zero++;
  const kind = cc === e - s ? 'int3' : zero === e - s ? 'zero' : cc > 0 ? 'int3_then_other' : 'other';
  stat.gap_kinds[kind].n++;
  stat.gap_kinds[kind].bytes += e - s;
  if (kind === 'other' || kind === 'int3_then_other') {
    const bucket = 1 << Math.ceil(Math.log2(e - s));
    stat.other_gap_size_histogram[bucket] = (stat.other_gap_size_histogram[bucket] || 0) + 1;
    others.push([e - s, s]);
  }
}
for (const f of fns) {
  gap(prev, f.begin);
  prev = Math.max(prev, f.end);
}
gap(prev, end);
others.sort((a, c) => c[0] - a[0]);
stat.largest_other_gaps = others.slice(0, 20).map(([size, rva]) => ({ rva: hex(rva), size, head: b.subarray(img.rvaToOff(rva), img.rvaToOff(rva) + 16).toString('hex') }));
writeJson(OUT, stat);
logRun({ tool: 'text_gaps', version: VERSION, outputs: [OUT] });
console.log(JSON.stringify(stat, null, 1));
