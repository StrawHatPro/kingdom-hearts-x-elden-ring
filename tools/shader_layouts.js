// Derives, from the game executable, the layout of every shader class's parameter bindings
// (what FShader subclasses write in Serialize) and writes tools/lib/pkg_classes/shader_layouts.json,
// which tools/lib/pkg_classes/material.js uses to decode shader maps.
//
//   node tools/shader_layouts.js
//
// Inputs: base/KH3.exe (strings, vtables, static tables), build/decomp/*.jsonl (decompiled C),
// the shader type names present in the data (audit/package_facts.json and the converted global
// shader cache). Steps (tools/shader_layouts/):
//   reg.js       type-name string -> the static initializer that registers the FShaderType
//   vf_reg.js    vertex factory type names -> their FVertexFactoryType registration
//   vf_ctor.js   ConstructShaderParameters -> parameter class vtable per shader frequency
//   scaleform.js uniform tables of the Scaleform fragment shader descriptors
//   expand.js    ConstructSerializedType -> vtable -> Serialize (slot 2) -> flat list of primitive
//                serializer calls with member offsets (manual.js holds the functions read by hand)
//   emit.js      the JSON
// Docs: docs/formats/shader_parameters.md.
const path = require('path');
const { spawnSync } = require('child_process');
const { ROOT, logRun } = require('./lib/log');

const VERSION = '1.0.0';
for (const step of ['reg.js', 'vf_reg.js', 'vf_ctor.js', 'expand.js', 'scaleform.js', 'emit.js']) {
  const r = spawnSync(process.execPath, ['--max-old-space-size=8000', path.join(__dirname, 'shader_layouts', step)], { stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 1 << 26 });
  if (r.status !== 0) { console.error(step + ' failed'); process.exit(1); }
  console.log('--- ' + step + '\n' + r.stdout.split('\n').slice(0, 12).join('\n'));
}
const OUT = path.join(ROOT, 'tools', 'lib', 'pkg_classes', 'shader_layouts.json');
logRun({ tool: 'shader_layouts', version: VERSION, outputs: [OUT] });
