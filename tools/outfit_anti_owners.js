#!/usr/bin/env node
// (SETUP-TOOLS 2026-11-07) THE RELEASE PACK STEP OF AUDIT-C ANLOOK: an outfit's face / skin / hair drawn with KH3's own anti-mode
// owners in Rage Form (L8 / L9), made IN the pack the build is making (build/sora_pack/outfits/<o>/materials/{materials.json,
// materials.wgsl}), with the kh-shaderc job list of the new owners written to build/packgen/anti_jobs_<o>.json (the shader phase
// builds them: tools/packgen_shaders.js `anti`, recipe anti_jobs). From build/sora_tmp/weapons/AUDIT-C/ANLOOK/tools/
// stage_anti_owners.js (the development tool wrote a test pack's copy from the live pack); the logic is that tool's. Two
// differences: it runs once on the pack it changes (an outfit that has `@anti` materials already is left as it is), and a texture
// of the main pack is written as a path RELATIVE to the outfit's materials folder (the development pack has the developer's
// absolute path there; kh-look joins a relative file to the folder, so the same file is read).
//   node tools/outfit_anti_owners.js [kh1]
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const MAIN = path.join(ROOT, 'build/sora_pack/materials');
const o = process.argv[2] || 'kh1';
const dir = path.join(ROOT, 'build/sora_pack/outfits', o, 'materials');
for (const f of ['materials.json', 'materials.wgsl']) if (fs.lstatSync(path.join(dir, f)).isSymbolicLink()) throw new Error(`${f} is a link: refusing`);
const LIVE = dir;
const lib = JSON.parse(fs.readFileSync(path.join(LIVE, 'materials.json'), 'utf8').replace(/^﻿/, ''));
let wgsl = fs.readFileSync(path.join(LIVE, 'materials.wgsl'), 'utf8');
if (Object.keys(lib.materials).some((n) => n.endsWith('@anti'))) { console.log(`outfit_anti_owners: ${o} has its anti-mode materials already`); process.exit(0); }
const main = JSON.parse(fs.readFileSync(path.join(MAIN, 'materials.json'), 'utf8').replace(/^﻿/, ''));
const mainWgsl = fs.readFileSync(path.join(MAIN, 'materials.wgsl'), 'utf8');

const ROOT_FACE = '/Game/Shader/Material_set/SQEX_BaseMaterialSet/SQEX_CHRBaseMat/pc_skinbasemat.pc_skinbasemat';
const ROOT_HAIR = '/Game/Shader/Material_set/SQEX_KHHairBaseMat.SQEX_KHHairBaseMat';
// the KH3 owner for each outfit slot: KH3's anti-mode owner of the SAME root material (pc_skinbasemat: the face's
// p_ex001_F_mat0 for a face slot, the skin's p_ex001_mat3 otherwise); a root KH3 has no owner of (the outfits' cloth bases)
// keeps the bake
const pick = (name, rec) => {
  if (lib.owners[rec.owner] && (lib.owners[rec.owner].static_switches_true || []).includes('UseAntiMode')) return null;
  if (rec.root === ROOT_FACE) return /_F_mat/.test(name) ? 'p_ex001_F_mat0_Inst' : 'p_ex001_mat3_Inst';
  const same = Object.entries(main.materials).filter(([, r]) => r.root === rec.root && (main.owners[r.owner].static_switches_true || []).includes('UseAntiMode')).map(([, r]) => r.owner);
  return same[0] || null;
};
const block = (text, id) => { const a = text.indexOf(`#ifdef KH_C${id}\n`); const b = text.indexOf('#endif', a); if (a < 0 || b < 0) throw new Error(`no KH_C${id} block`); return text.slice(a, b + '#endif'.length); };
const mainMaterials = Object.entries(main.materials);
const kh3Instance = (owner) => mainMaterials.find(([, r]) => r.owner === owner)[1];
let nextId = Math.max(...Object.values(lib.owners).map((x) => x.id)) + 1;
const added = new Map(); // KH3 owner name -> new id
const blocks = [];
const anti = (n) => /^(Anti|UseAnti)/.test(n);
const defaults = (owner) => { const sc = {}, ve = {}; const walk = (t) => { if (!t || typeof t !== 'object') return; if (Array.isArray(t)) { t.forEach(walk); return; } if (typeof t.s === 'string' && 'd' in t) sc[t.s] ??= t.d; if (typeof t.v === 'string' && Array.isArray(t.d)) ve[t.v] ??= t.d; Object.values(t).forEach(walk); }; walk(owner.vec); walk(owner.sca); return { sc, ve }; };
const report = [];
for (const [name, rec] of Object.entries(lib.materials)) {
  if (name.endsWith('@anti')) continue;
  const kh3 = pick(name, rec);
  if (!kh3) continue;
  if (!added.has(kh3)) {
    const id = nextId++;
    added.set(kh3, id);
    lib.owners[`${kh3}@kh3`] = { ...JSON.parse(JSON.stringify(main.owners[kh3])), id };
    blocks.push(block(mainWgsl, main.owners[kh3].id).replace(`#ifdef KH_C${main.owners[kh3].id}\n`, `#ifdef KH_C${id}\n// (AUDIT-C ANLOOK) KH3's ${kh3} (main pack KH_C${main.owners[kh3].id}, UseAntiMode compiled), for ${o}'s Rage Form\n`));
  }
  const kOwner = lib.owners[`${kh3}@kh3`];
  const own = lib.owners[rec.owner];
  // the outfit's textures by parameter name (its owner's slot list gives each texture's parameter); unnamed slots by the
  // root default's file name in the outfit, else KH3's (absolute, the main pack)
  const byParam = new Map(); own.slots.forEach((s, i) => { if (s.parameter && rec.textures[i]) byParam.set(s.parameter, rec.textures[i]); });
  const fileOf = (asset) => asset ? asset.split('/').pop().split('.')[0] + '.png' : null;
  const textures = kOwner.slots.map((s) => {
    if (s.parameter && byParam.has(s.parameter)) return byParam.get(s.parameter);
    const f = fileOf(s.root_default);
    if (f && fs.existsSync(path.join(LIVE, 'textures', f))) return { file: `textures/${f}`, srgb: false, mips: 2 };
    if (f && fs.existsSync(path.join(MAIN, 'textures', f))) return { file: path.relative(dir, path.join(MAIN, 'textures', f)).split(path.sep).join('/'), srgb: /RGBTable|Color/.test(f), mips: 2 };
    // (HQMayugeTex: its root default BaseColorTex_A_Plus is in no pack) the slot's own base colour
    return byParam.get('BaseColorTex') || rec.textures[0];
  });
  const d = defaults(own);
  const k = kh3Instance(kh3);
  const scalars = { ...d.sc, ...rec.scalars, ...Object.fromEntries(Object.entries(k.scalars || {}).filter(([n]) => anti(n))) };
  const vectors = { ...d.ve, ...rec.vectors, ...Object.fromEntries(Object.entries(k.vectors || {}).filter(([n]) => anti(n))) };
  lib.materials[`${name}@anti`] = { ...rec, owner: `${kh3}@kh3`, id: added.get(kh3), scalars, vectors, textures, note: `(AUDIT-C ANLOOK) ${name} drawn with KH3's ${kh3} (UseAntiMode) in Rage Form` };
  const uv = lib.uv2 && lib.uv2[`${rec.mesh}:${name}`];
  if (uv) lib.uv2[`${rec.mesh}:${name}@anti`] = uv;
  report.push(`${name} -> ${kh3} (c${added.get(kh3)}): ${textures.map((t) => path.basename(t.file)).join(', ')}`);
}
const at = wgsl.indexOf('\nfn kh_khs(');
if (at < 0) throw new Error('no KHS block in the outfit wgsl');
wgsl = wgsl.slice(0, at) + '\n' + blocks.join('\n\n') + '\n' + wgsl.slice(at);
fs.writeFileSync(path.join(dir, 'materials.json'), JSON.stringify(lib));
fs.writeFileSync(path.join(dir, 'materials.wgsl'), wgsl);
// the kh-shaderc job list (tools/kb_shader_jobs.js's sets) for the new owners' variants
const jobs = [];
const variants = new Map();
for (const [name, r] of Object.entries(lib.materials).filter(([n]) => n.endsWith('@anti'))) {
  const ow = lib.owners[r.owner];
  const masked = r.blend_mode === 'BLEND_Masked' && !!(ow.mask && ow.mask.ok);
  const translucent = r.blend_mode === 'BLEND_Translucent';
  const subsurface = ow.lighting_model === 'MSM_Subsurface';
  variants.set(`c${ow.id}${masked ? 'm' : ''}${translucent ? 't' : ''}${subsurface ? 's' : ''}`, { id: ow.id, masked, translucent, subsurface, hair: r.root === ROOT_HAIR });
  void name;
}
for (const [vname, v] of variants) {
  const flags = [v.masked && 'KH_MASKED', v.translucent && 'KH_TRANSLUCENT', v.subsurface && 'KH_SUBSURFACE'].filter(Boolean);
  const job = (suffix, entry, defs) => jobs.push({ name: `${vname}${suffix}`, stage: 'ps', entry, defines: [...new Set(['ER_FRAGMENT', `KH_C${v.id}`, 'KH_JOINTS_EXTRA', ...defs, ...flags])].sort() });
  job('', 'fs_main', []);
  job('_pl', 'fs_main', ['KH_EXTRA_LIGHTS']);
  job('_bl', 'fs_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD']);
  job('_bli', 'fs_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_INTERIOR']);
  job('_hy2', 'fs_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_HYBRID']);
  job('_gb2', 'gb_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_HYBRID', 'KH_GBUFFER']);
  job('_hy3', 'fs_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_HYBRID', 'KH_HY3', 'KH_INTERIOR']);
  job('_gb3', 'gb_main', ['KH_EXTRA_LIGHTS', 'KH_BLOOD', 'KH_HYBRID', 'KH_HY3', 'KH_INTERIOR', 'KH_GBUFFER']);
}
fs.mkdirSync(path.join(ROOT, 'build', 'packgen'), { recursive: true });
fs.writeFileSync(path.join(ROOT, 'build', 'packgen', `anti_jobs_${o}.json`), JSON.stringify({ jobs }, null, 1));
console.log(report.join('\n'));
console.log(`owners added: ${[...added].map(([k, id]) => `${k} c${id}`).join(', ')}; ${jobs.length} jobs -> build/packgen/anti_jobs_${o}.json`);
