// setup.exe: the mod's setup as ONE program (Node.js's single executable application, SEA: the pinned Node of
// release-tools/node_pin.json with this file injected; built by release-tools/make_release.js). A player needs no Node.
//
//   setup.exe [setup options]                 runs setup/setup.js (next to setup.exe) with those options
//   setup.exe --run <script.js> [args]        runs that tool script from disk, as `node <script.js> [args]` would
//   setup.exe <script.js> [args]              the same (how the tools start each other: process.execPath + a script)
//   setup.exe --max-old-space-size=N <script.js> [args]
//                                             a Node option before the script (the tools' workers ask for more heap):
//                                             setup.exe starts itself again with it in NODE_OPTIONS, which it honours
//
// The script is loaded as the main module with Node's own CommonJS loader (Module._load: `require.main === module`,
// __filename, a real `require` of the script's own files, the bundled node_modules and their wasm), so every tool runs
// under setup.exe exactly as under node.exe; the same scripts still run with a normal `node` in a source checkout.
// Only built-in modules are used here (the injected main script of a SEA can require nothing else).
'use strict';
const path = require('node:path');
const fs = require('node:fs');
const Module = require('node:module');
const cp = require('node:child_process');

const EXE = process.execPath;
const ROOT = path.dirname(EXE);
// (a SEA's argv: [exe, exe, ...the arguments])
let args = process.argv.slice(2);

// Node options the tools pass before a script when they start a worker: applied through NODE_OPTIONS by a restart
const NODE_OPTION = /^--(max-old-space-size|max-semi-space-size|stack-size)=\d+$/;
const nodeOpts = [];
while (args.length && NODE_OPTION.test(args[0])) nodeOpts.push(args.shift());

const isScript = (a) => typeof a === 'string' && /\.(c?js)$/i.test(a) && fs.existsSync(path.resolve(a));
let script, rest;
if (args[0] === '--run') {
  if (!isScript(args[1])) { console.error(`setup.exe --run: no script ${args[1] || ''}`); process.exit(2); }
  script = args[1]; rest = args.slice(2);
} else if (isScript(args[0])) {
  script = args[0]; rest = args.slice(1);
} else {
  script = path.join(ROOT, 'setup', 'setup.js'); rest = args;
  if (!fs.existsSync(script)) { console.error(`setup.exe: ${script} is missing: setup.exe must stay in the mod's folder, next to setup\\`); process.exit(2); }
}
script = path.resolve(script);

// double-clicked (the setup itself, not a tool): the folder picker for a game it cannot find, and the window stays open
// at the end (setup\setup.bat, which pauses itself, sets KH_SETUP_NO_PAUSE)
if (!args.length || script === path.join(ROOT, 'setup', 'setup.js') && args[0] !== '--run' && !isScript(args[0])) {
  if (!process.env.KH_SETUP_NO_PAUSE) {
    const env = { ...process.env, KH_SETUP_PICKER: process.env.KH_SETUP_PICKER || '1', KH_SETUP_NO_PAUSE: '1', ...(nodeOpts.length ? { NODE_OPTIONS: [process.env.NODE_OPTIONS, ...nodeOpts].filter(Boolean).join(' ') } : {}) };
    const r = cp.spawnSync(EXE, ['--run', script, ...rest], { stdio: 'inherit', env, windowsHide: false });
    cp.spawnSync('cmd.exe', ['/d', '/c', 'pause'], { stdio: 'inherit' });
    process.exit(r.status === null ? 1 : r.status);
  }
}

if (nodeOpts.length) {
  const env = { ...process.env, NODE_OPTIONS: [process.env.NODE_OPTIONS, ...nodeOpts].filter(Boolean).join(' ') };
  const r = cp.spawnSync(EXE, ['--run', script, ...rest], { stdio: 'inherit', env, windowsHide: true });
  if (r.error) { console.error(`setup.exe: ${r.error.message}`); process.exit(1); }
  process.exit(r.status === null ? 1 : r.status);
}

process.argv = [EXE, script, ...rest];
Module._load(script, null, true);
