// Record an external tool run (Ghidra scripts, compilers) in audit/tool_log.jsonl.
//   node tools/logrun.js <tool> <version> "<command as run>" "<note>" [output files...]
const fs = require('fs');
const path = require('path');
const { ROOT, sha256File, rel } = require('./lib/log');

const [tool, version, command, note, ...outputs] = process.argv.slice(2);
if (!tool || !version || !command) { console.error('usage: logrun.js <tool> <version> "<command>" "<note>" [outputs...]'); process.exit(2); }
const entry = {
  timestamp: new Date().toISOString(), tool, version, command,
  outputs: outputs.map((f) => ({ path: rel(path.resolve(f)), sha256: sha256File(path.resolve(f)) })),
};
if (note) entry.note = note;
fs.appendFileSync(path.join(ROOT, 'audit', 'tool_log.jsonl'), JSON.stringify(entry) + '\n');
console.log(`tool_log:${entry.timestamp}#${tool}`);
