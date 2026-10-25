// Evidence log (spec 1.1): every audit value traces to a line in audit/tool_log.jsonl.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const ROOT = path.resolve(__dirname, '..', '..');
const LOG = path.join(ROOT, 'audit', 'tool_log.jsonl');
const CONFIG = JSON.parse(fs.readFileSync(path.join(ROOT, 'tools', 'config.json'), 'utf8'));

// Streaming so the multi-GB archives don't need to fit in memory.
function sha256File(file) {
  const h = crypto.createHash('sha256');
  const fd = fs.openSync(file, 'r');
  const buf = Buffer.allocUnsafe(1 << 24);
  try {
    for (let n; (n = fs.readSync(fd, buf, 0, buf.length, null)) > 0;) h.update(buf.subarray(0, n));
  } finally {
    fs.closeSync(fd);
  }
  return h.digest('hex');
}

function rel(p) {
  return path.relative(ROOT, p).replace(/\\/g, '/');
}

// One line per invocation. `outputs` are files the tool wrote; their hashes pin the evidence.
function logRun({ tool, version, outputs = [], note, timestamp }) {
  const entry = {
    timestamp: timestamp || new Date().toISOString(),
    tool,
    version,
    runtime: `node ${process.version}`,
    command: ['node', rel(process.argv[1]), ...process.argv.slice(2)].join(' '),
    outputs: outputs.map((f) => ({ path: rel(f), sha256: sha256File(f) })),
  };
  if (note) entry.note = note;
  fs.appendFileSync(LOG, JSON.stringify(entry) + '\n');
  return `tool_log:${entry.timestamp}#${tool}`;
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n');
}

const hex = (n, width = 0) => '0x' + n.toString(16).toUpperCase().padStart(width, '0');

module.exports = { ROOT, CONFIG, sha256File, logRun, writeJson, rel, hex };
