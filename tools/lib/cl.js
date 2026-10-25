// Run the installed MSVC x64 compiler without a developer prompt: the vcvars environment is
// captured once into build/vcvars.json.
const fs = require('fs');
const path = require('path');
const { execFileSync, spawnSync } = require('child_process');
const { ROOT } = require('./log');

const CACHE = path.join(ROOT, 'build', 'vcvars.json');
// /TC C, /O2 optimise, /GS- no stack cookie (would reference CRT symbols), /Zl no default libs,
// /Oi intrinsics, /Gy- keep the function in a plain .text section, /w no warnings (decompiler
// output is deliberately loosely typed).
const CFLAGS = ['/nologo', '/c', '/TC', '/std:c11', '/O2', '/GS-', '/Zl', '/Oi', '/Gy-', '/w', '/fp:precise'];

function env() {
  if (fs.existsSync(CACHE)) return JSON.parse(fs.readFileSync(CACHE, 'utf8'));
  const vswhere = path.join(process.env['ProgramFiles(x86)'], 'Microsoft Visual Studio', 'Installer', 'vswhere.exe');
  const vs = execFileSync(vswhere, ['-latest', '-property', 'installationPath'], { encoding: 'utf8' }).trim();
  const vcvars = path.join(vs, 'VC', 'Auxiliary', 'Build', 'vcvars64.bat');
  const out = execFileSync('cmd.exe', ['/s', '/c', `"call "${vcvars}" >nul 2>&1 && set"`], { encoding: 'utf8', windowsVerbatimArguments: true });
  const e = {};
  for (const line of out.split(/\r?\n/)) { const i = line.indexOf('='); if (i > 0) e[line.slice(0, i)] = line.slice(i + 1); }
  const pathKey = Object.keys(e).find((k) => k.toLowerCase() === 'path');
  e.__cl = e[pathKey].split(';').map((d) => path.join(d, 'cl.exe')).find((p) => fs.existsSync(p));
  const ver = spawnSync(e.__cl, [], { env: e, encoding: 'utf8' });
  e.__version = (/Version ([\d.]+)/.exec(ver.stderr + ver.stdout) || [])[1] || 'unknown';
  fs.mkdirSync(path.dirname(CACHE), { recursive: true });
  fs.writeFileSync(CACHE, JSON.stringify(e));
  return e;
}

// Compile `files` (absolute .c paths) into objDir. Returns { failed: Set<basename>, output }.
function compile(files, objDir, extraFlags = []) {
  const e = env();
  fs.mkdirSync(objDir, { recursive: true });
  const rsp = path.join(objDir, `cl_${process.pid}_${Date.now()}.rsp`);
  fs.writeFileSync(rsp, [...CFLAGS, ...extraFlags, `/Fo"${objDir}\\\\"`, ...files.map((f) => `"${f}"`)].join('\n'));
  const r = spawnSync(e.__cl, ['@' + rsp], { env: e, encoding: 'utf8', cwd: objDir, maxBuffer: 1 << 28 });
  fs.rmSync(rsp, { force: true });
  const output = (r.stdout || '') + (r.stderr || '');
  const failed = new Set();
  for (const m of output.matchAll(/^(.*?)\((\d+)\)\s*:\s*(?:fatal )?error /gm)) failed.add(path.basename(m[1]).toLowerCase());
  return { failed, output, version: e.__version };
}

module.exports = { env, compile, CFLAGS };
