#!/usr/bin/env node
// (KBFX) The kh-shaderc `--rebuild` job list of the keyblades' material shaders for SORA'S library (kh-er-render keyblade.rs:
// the `kb:` slots are drawn with his vertex shader, so they are built in kh-shaderc's normal mode, never --party): for every
// variant of KB_DIR/materials/materials.json (owner id, masked, translucent, subsurface, as kh_look Library::from_json reads
// them) every set the renderer may pick: "", _pl, _bl, _bli, _hy2, _gb2, _hy3, _gb3 and the Mimic Tear's _gb3_mm / _gb3_mf
// (opaque / masked only). The defines are kh-shaderc main.rs's for each set. From P5
// (build/sora_tmp/weapons/P5/tools/kb_shader_jobs.js), the folders as arguments; `--split N` also writes the list cut into
// N parts (OUT with _<i> before .json) for parallel kh-shaderc runs.
//
//   node tools/kb_shader_jobs.js [KB_DIR] [OUT.json] [--split N]
//     KB_DIR  the keyblades folder (default build/sora_pack/keyblades)
//     OUT     the job list (default build/packgen/kb/jobs.json)
'use strict';
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const args = process.argv.slice(2);
const si = args.indexOf('--split');
const split = si >= 0 ? Math.max(1, Number(args[si + 1]) || 1) : 0;
const pos = args.filter((a, i) => !(si >= 0 && (i === si || i === si + 1)));
const kbDir = path.resolve(pos[0] || path.join(ROOT, 'build', 'sora_pack', 'keyblades'));
const out = path.resolve(pos[1] || path.join(ROOT, 'build', 'packgen', 'kb', 'jobs.json'));
const lib = JSON.parse(fs.readFileSync(path.join(kbDir, 'materials', 'materials.json'), 'utf8').replace(/^﻿/, ''));
const variants = new Map();
for (const r of Object.values(lib.materials)) {
  const o = lib.owners[r.owner];
  if (!o || !(o.base && o.base.ok)) continue;
  const masked = r.blend_mode === 'BLEND_Masked' && !!(o.mask && o.mask.ok);
  const translucent = r.blend_mode === 'BLEND_Translucent';
  const subsurface = o.lighting_model === 'MSM_Subsurface';
  const name = `c${o.id}${masked ? 'm' : ''}${translucent ? 't' : ''}${subsurface ? 's' : ''}`;
  variants.set(name, { id: o.id, masked, translucent, subsurface });
}
const jobs = [];
for (const [name, v] of [...variants].sort()) {
  const flags = [v.masked && 'KH_MASKED', v.translucent && 'KH_TRANSLUCENT', v.subsurface && 'KH_SUBSURFACE'].filter(Boolean);
  const job = (suffix, entry, defs) => jobs.push({ name: `${name}${suffix}`, stage: 'ps', entry, defines: [...new Set(['ER_FRAGMENT', `KH_C${v.id}`, ...defs, ...flags])].sort() });
  job('', 'fs_main', []);
  job('_pl', 'fs_main', ['KH_EXTRA_LIGHTS']);
  job('_bl', 'fs_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD']);
  job('_bli', 'fs_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_INTERIOR']);
  job('_hy2', 'fs_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_HYBRID']);
  job('_gb2', 'gb_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_HYBRID', 'KH_GBUFFER']);
  job('_hy3', 'fs_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_HYBRID', 'KH_HY3', 'KH_INTERIOR']);
  job('_gb3', 'gb_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_HYBRID', 'KH_HY3', 'KH_INTERIOR', 'KH_GBUFFER']);
  if (!v.translucent) {
    // (kh-shaderc mimic_defines: KH_TRANSLUCENT never; the flags are masked / subsurface)
    const mm = ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_HYBRID', 'KH_HY3', 'KH_INTERIOR', 'KH_GBUFFER', 'KH_MIMIC'];
    jobs.push({ name: `${name}_gb3_mm`, stage: 'ps', entry: 'gb_main', defines: [...new Set(['ER_FRAGMENT', `KH_C${v.id}`, ...mm, ...flags])].sort() });
    jobs.push({ name: `${name}_gb3_mf`, stage: 'ps', entry: 'gb_main', defines: [...new Set(['ER_FRAGMENT', `KH_C${v.id}`, ...mm, 'KH_MIMIC_FORM', ...flags])].sort() });
  }
}
const write = (file, list) => { const text = JSON.stringify({ about: 'kh-shaderc --rebuild list: the keyblades\' materials for Sora (tools/kb_shader_jobs.js)', variants: [...variants.keys()], jobs: list }, null, 1) + '\n'; fs.mkdirSync(path.dirname(file), { recursive: true }); if (!fs.existsSync(file) || fs.readFileSync(file, 'utf8') !== text) fs.writeFileSync(file, text); };
write(out, jobs);
if (split) for (let i = 0; i < split; i++) write(out.replace(/\.json$/, `_${i}.json`), jobs.filter((_, k) => k % split === i));
console.log(`kb_shader_jobs: ${variants.size} variants, ${jobs.length} jobs -> ${path.relative(ROOT, out).split(path.sep).join('/')}${split ? ` (+ ${split} parts)` : ''}`);
