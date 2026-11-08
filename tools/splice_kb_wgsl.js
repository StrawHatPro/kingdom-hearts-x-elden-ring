#!/usr/bin/env node
// (KBFX) The keyblades' material shader source: PACK/keyblades/materials/materials.wgsl (their KH_C5xx blocks only, written by
// tools/sora_keyblades_pack.js) with the main pack's world post-process material (its KHS block) spliced in after them, as
// kh-shaderc's party mode does and as tools/packgen_shaders.js does for the XS form weapons. The keyblades are drawn in
// SORA's library (kh-er-render keyblade.rs: the `kb:` slots, his vertex layout), so their pixel shaders are built in
// kh-shaderc's normal mode from this file (tools/kb_shader_jobs.js, `--rebuild`), never with --party.
// From P5 (build/sora_tmp/weapons/P5/tools/splice_kb_wgsl.js), the folders as arguments.
//
//   node tools/splice_kb_wgsl.js [KB_DIR] [MAIN_PACK] [OUT]
//     KB_DIR     the keyblades folder (default build/sora_pack/keyblades)
//     MAIN_PACK  the main pack (default build/sora_pack): its materials/materials.wgsl gives the KHS block
//     OUT        the spliced file (default build/packgen/kb/materials.wgsl); written only when its bytes change
'use strict';
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const kbDir = path.resolve(process.argv[2] || path.join(ROOT, 'build', 'sora_pack', 'keyblades'));
const mainPack = path.resolve(process.argv[3] || path.join(ROOT, 'build', 'sora_pack'));
const out = path.resolve(process.argv[4] || path.join(ROOT, 'build', 'packgen', 'kb', 'materials.wgsl'));
const kb = fs.readFileSync(path.join(kbDir, 'materials', 'materials.wgsl'), 'utf8');
const main = fs.readFileSync(path.join(mainPack, 'materials', 'materials.wgsl'), 'utf8');
if (kb.includes('fn kh_khs(')) throw new Error('the keyblades materials.wgsl already has a KHS block');
const at = main.indexOf('// /Game/Shader/KHS_set');
if (at < 0) throw new Error('the main materials.wgsl has no KHS block');
const text = `${kb}\n${main.slice(at)}`;
fs.mkdirSync(path.dirname(out), { recursive: true });
if (!fs.existsSync(out) || fs.readFileSync(out, 'utf8') !== text) fs.writeFileSync(out, text);
console.log(`splice_kb_wgsl: ${path.relative(ROOT, out).split(path.sep).join('/')}: ${kb.length} + ${main.length - at} bytes`);
