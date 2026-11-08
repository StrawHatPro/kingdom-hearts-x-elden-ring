#!/usr/bin/env node
// (PACKGEN) THE PACK'S ELDEN RING SHADERS (DXIL), compiled on the player's PC from the pack's own translated materials (the
// game's shaders, translated by tools/sora_material_pack.js and tools/sora_vfx_shaders.js) and our harness WGSL, by
// kh-shaderc and the DirectX shader compiler. The runs are those of tools/packgen/recipes/shaders.json:
//   1. the hair translations (er87 / er91 genh): our scripts' fixes laid on the folder's translated materials
//   2. per character folder (Sora, his outfits, Donald, Goofy, the team props): kh-shaderc's whole build, then the
//      exclusive modes the folder has (finale, hycap, h91, probe3 / probe4, skincg, velo88, zdraw)
//      2b. (KBFX) the keyblades' materials in Sora's vertex layout (kb_dirs; step `kb`, also run with `char`)
//   3. the rebuild groups: every character pixel shader the DLL can load, with its defines and materials source
//   4. per effect root (vfx, party/vfx, xsi/vfx): kh-shaderc --vfx on the root's shading tables
//   5. the effect tables' overlay shaders (the streams' er/<name>_<s>.*.dxil): kh-shaderc --vfx --only over the overlay
//      translator runs (tools/packgen_overlays.js)
//   (SETUP-TOOLS 2026-11-07) 3b anti (KH1's anti-mode owners), 3c mimic (the Mimic Tear's writers and pool), 3d fxcopy (the
//      effects' fog-copy / lit-kit shaders): the runs the deploys made by hand (recipe anti_jobs, mimic_sets, mimic_pool, fx_copies)
//
//   node tools/packgen_shaders.js [--shaderc EXE] [--dxc EXE] [--jobs N] [--only STEP,...]
//     --shaderc  kh-shaderc.exe (default: KH_SHADERC, tools/bin/kh-shaderc.exe, target/release/kh-shaderc.exe)
//     --dxc      dxc.exe (default: KH_DXC, tools/bin/dxc/bin/x64/dxc.exe, the newest Windows 10/11 SDK's)
// A group whose materials source is missing (a stream's set this build does not make) is skipped and listed.
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const ROOT = path.resolve(__dirname, '..');
const T = path.join(ROOT, 'build', 'sora_tmp');
const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const only = opt('--only') ? new Set(opt('--only').split(',')) : null;
const JOBS = Math.max(1, Number(opt('--jobs')) || Math.max(1, (require('os').availableParallelism ? require('os').availableParallelism() : require('os').cpus().length) - 1));
const recipe = JSON.parse(fs.readFileSync(path.join(ROOT, 'tools', 'packgen', 'recipes', 'shaders.json'), 'utf8'));
const exists = (p) => fs.existsSync(p);
const abs = (p) => path.resolve(ROOT, p);

function findShaderc() {
  for (const c of [opt('--shaderc'), process.env.KH_SHADERC, path.join(ROOT, 'tools', 'bin', 'kh-shaderc.exe'), path.join(ROOT, 'target', 'release', 'kh-shaderc.exe'), path.join(ROOT, 'tools', 'bin', 'kh-shaderc'), path.join(ROOT, 'target', 'release', 'kh-shaderc')]) if (c && exists(c)) return path.resolve(c);
  throw new Error('kh-shaderc.exe not found (the release has it in tools/bin; from source: cargo build --release -p kh-shaderc)');
}
function findDxc() {
  for (const c of [opt('--dxc'), process.env.KH_DXC, path.join(ROOT, 'tools', 'bin', 'dxc', 'bin', 'x64', 'dxc.exe'), path.join(ROOT, 'tools', 'bin', 'dxc', 'bin', 'dxc')]) if (c && exists(c)) return path.resolve(c);
  const kits = 'C:\\Program Files (x86)\\Windows Kits\\10\\bin';
  if (exists(kits)) { const v = fs.readdirSync(kits).map((d) => path.join(kits, d, 'x64', 'dxc.exe')).filter(exists).sort(); if (v.length) return v[v.length - 1]; }
  throw new Error('dxc.exe not found: the setup downloads the DirectX shader compiler into tools/bin/dxc (or install the Windows 10/11 SDK)');
}
const SHADERC = findShaderc(), DXC = findDxc();
const LOG = path.join(ROOT, 'build', 'packgen', 'logs', 'shaders.log');
fs.mkdirSync(path.dirname(LOG), { recursive: true });
const log = (s) => fs.appendFileSync(LOG, s + '\n');
log(`==== ${new Date().toISOString()} kh-shaderc ${SHADERC}, dxc ${DXC}`);

// one kh-shaderc run (synchronous)
function shaderc(argv, what) {
  const full = [...argv, '--dxc', DXC];
  const r = cp.spawnSync(SHADERC, full, { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 28, env: { ...process.env, KH_SHADERC_HLSL_DIR: path.join(ROOT, 'build', 'packgen', 'hlsl', 'vfx') } });
  log(`> kh-shaderc ${full.join(' ')}\n${(r.stdout || '') + (r.stderr || '')}`);
  if (r.status !== 0) throw new Error(`kh-shaderc failed (${what}): ${((r.stdout || '') + (r.stderr || '')).split('\n').filter(Boolean).slice(-6).join(' | ')}`);
}
// several runs in parallel (the rebuild groups)
function parallel(tasks) {
  return new Promise((resolve) => {
    let next = 0, running = 0; const failed = [];
    const launch = () => {
      if (next >= tasks.length && !running) return resolve(failed);
      while (running < JOBS && next < tasks.length) {
        const t = tasks[next++]; running++;
        const c = cp.spawn(SHADERC, [...t.argv, '--dxc', DXC], { cwd: ROOT, windowsHide: true });
        let out = ''; c.stdout.on('data', (d) => { out += d; }); c.stderr.on('data', (d) => { out += d; });
        c.on('close', (code) => { log(`> kh-shaderc ${t.argv.join(' ')} (exit ${code})\n${out}`); if (code !== 0) failed.push(t.what); running--; launch(); });
      }
    };
    launch();
  });
}
const step = (id) => !only || only.has(id);
const skipped = [];

(async () => {
  const t0 = Date.now();
  // ---- 1. the hair translations
  if (step('hair')) {
    const node = (script, a) => { const r = cp.spawnSync(process.execPath, [path.join(T, script), ...a], { cwd: ROOT, encoding: 'utf8' }); log(`> node ${script} ${a.join(' ')}\n${r.stdout || ''}${r.stderr || ''}`); if (r.status !== 0) throw new Error(`${script} failed: ${r.stderr}`); };
    for (const k of ['er87', 'er91']) fs.mkdirSync(path.join(T, 'er', k, 'genh'), { recursive: true });
    fs.mkdirSync(path.join(T, 'er', 'er87', 'gen'), { recursive: true }); fs.mkdirSync(path.join(T, 'er', 'er91', 'gen'), { recursive: true });
    for (const h of recipe.hair.er87) {
      if (!exists(abs(h.wgsl))) { skipped.push(`hair er87 ${h.name}: no ${h.wgsl}`); continue; }
      const mid = path.join(T, 'er', 'er87', 'gen', h.name + '.wgsl');
      node('er/er87/tools/patch_udiv.js', [abs(h.wgsl), mid]);
      node('er/er87/tools/hair_fade.js', [mid, path.join(T, 'er', 'er87', 'genh', h.name + '.wgsl')]);
    }
    for (const h of recipe.hair.er91) {
      if (!exists(abs(h.wgsl))) { skipped.push(`hair er91 ${h.name}: no ${h.wgsl}`); continue; }
      const mid = path.join(T, 'er', 'er91', 'gen', h.name + '.wgsl');
      node('er/er87/tools/patch_udiv.js', [abs(h.wgsl), mid]);
      node('er/er91/tools/hair91.js', [mid, abs(h.json), path.join(T, 'er', 'er91', 'genh', h.name + '.wgsl'), '--phase', '--margin', '--fade', 'keep']);
    }
    console.log(`packgen_shaders: hair translations ${recipe.hair.er87.length + recipe.hair.er91.length - skipped.length}`);
  }
  // ---- 2. per character folder: the whole build, then its exclusive modes
  if (step('char')) {
    for (const d of recipe.char_dirs) {
      if (!exists(abs(d.pack))) { skipped.push(`char ${d.pack}: no pack folder`); continue; }
      const hlsl = path.join(ROOT, 'build', 'packgen', 'hlsl', d.er.replace(/[\\/]/g, '_'));
      const base = ['--pack', abs(d.pack), '--hlsl', hlsl, ...d.args.map((a) => (/^build\//.test(a) ? abs(a) : a))];
      shaderc(base, d.er);
      // (zdraw, FREEZE-FX iceshell: kh-shaderc writes them to its own default folder unless --out names the folder's er/)
      for (const m of d.modes) shaderc(m === 'zdraw' || m === 'iceshell' ? [...base, '--only', m, '--out', abs(path.join('build/sora_pack', d.er))] : [...base, '--only', m], `${d.er} --only ${m}`);
      console.log(`packgen_shaders: ${d.er}: whole build${d.modes.length ? ' + ' + d.modes.join(', ') : ''}`);
    }
  }
  // ---- 2b. (KBFX) the keyblades' own materials (recipe kb_dirs): drawn in SORA's library (kh-er-render keyblade.rs: the `kb:`
  // slots take his vertex shader), so built in kh-shaderc's normal mode, never --party: their KH_C5xx blocks with the main
  // pack's KHS block (tools/splice_kb_wgsl.js), every set the renderer may pick (tools/kb_shader_jobs.js, `--rebuild`, in
  // parallel parts), then the block-compressed textures (`--only tex`)
  if (step('char') || step('kb')) {
    const node = (a) => { const r = cp.spawnSync(process.execPath, a, { cwd: ROOT, encoding: 'utf8' }); log(`> node ${a.join(' ')}\n${r.stdout || ''}${r.stderr || ''}`); if (r.status !== 0) throw new Error(`${a[0]} failed: ${r.stderr || r.stdout}`); };
    for (const d of recipe.kb_dirs || []) {
      if (!exists(abs(path.join(d.pack, 'materials', 'materials.json')))) { skipped.push(`kb ${d.pack}: no materials/materials.json`); continue; }
      const work = abs(d.work), wgsl = path.join(work, 'materials.wgsl'), jobs = path.join(work, 'jobs.json'), out = abs(path.join('build/sora_pack', d.er));
      const hlsl = path.join(ROOT, 'build', 'packgen', 'hlsl', d.er.replace(/[\\/]/g, '_'));
      node([path.join(ROOT, 'tools', 'splice_kb_wgsl.js'), abs(d.pack), abs(d.main), wgsl]);
      node([path.join(ROOT, 'tools', 'kb_shader_jobs.js'), abs(d.pack), jobs, '--split', String(JOBS)]);
      fs.mkdirSync(out, { recursive: true });
      const base = ['--pack', abs(d.pack), '--materials', wgsl, '--out', out];
      const failed = await parallel(Array.from({ length: JOBS }, (_, i) => ({ what: `${d.er} part ${i}`, argv: [...base, '--rebuild', jobs.replace(/\.json$/, `_${i}.json`), '--hlsl', `${hlsl}_${i}`] })));
      if (failed.length) throw new Error(`kh-shaderc failed (${failed.join(', ')}): build/packgen/logs/shaders.log`);
      shaderc([...base, '--only', 'tex', '--hlsl', hlsl], `${d.er} --only tex`);
      const want = JSON.parse(fs.readFileSync(jobs, 'utf8')).jobs.map((j) => `${j.name}.${j.stage}.dxil`).filter((f) => !exists(path.join(out, f)));
      if (want.length) throw new Error(`${d.er}: ${want.length} shader(s) of the job list not built: ${want.slice(0, 6).join(', ')}`);
      console.log(`packgen_shaders: ${d.er}: ${JSON.parse(fs.readFileSync(jobs, 'utf8')).jobs.length} shaders in Sora's layout + the textures`);
    }
  }
  // (outfit forms) an outfit that wears KH3 Sora's own wear-form clothes has copies of their block-compressed textures
  // (tools/sora_outfit_forms.js copies them when the main folder has them: they exist only now, after kh-shaderc)
  if (step('char')) {
    const mainJson = abs('build/sora_pack/materials/materials.json'), mainTex = abs('build/sora_pack/materials/er/tex');
    const forms = exists(mainJson) ? (JSON.parse(fs.readFileSync(mainJson, 'utf8')).wear_forms || {}) : {};
    const names = new Set(); for (const f of ['0', '5']) for (const m of (forms[f] || {}).materials || []) for (const k of ['image', 'point']) if (m[k] && m[k].file) names.add(path.basename(m[k].file));
    let n = 0;
    const outfits = abs('build/sora_pack/outfits');
    for (const o of exists(outfits) ? fs.readdirSync(outfits) : []) for (const name of names) {
      const png = path.join(outfits, o, 'materials', 'textures', name), dds = path.join(mainTex, name.replace(/\.png$/, '.dds')), to = path.join(outfits, o, 'materials', 'er', 'tex', name.replace(/\.png$/, '.dds'));
      if (exists(png) && exists(dds) && exists(path.dirname(to)) && !exists(to)) { fs.copyFileSync(dds, to); n++; }
    }
    if (n) console.log(`packgen_shaders: ${n} wear-form texture(s) copied into the outfits`);
  }
  // ---- 3. the rebuild groups (every character shader the DLL can load)
  if (step('groups')) {
    // (HY) the XS form weapons' and Chandelier's pawn's materials as HY built them: their translated table with the main
    // pack's post-process (KHS) block appended, and the pawn's library as a folder of its own (build/sora_tmp/er/hy/scratch)
    {
      const main = abs('build/sora_pack/materials/materials.wgsl'), S = abs('build/sora_tmp/er/hy/scratch');
      const put = (f, data) => { if (exists(f) && fs.readFileSync(f).equals(Buffer.from(data))) return; fs.mkdirSync(path.dirname(f), { recursive: true }); fs.writeFileSync(f, data); };
      if (exists(main)) {
        const m = fs.readFileSync(main, 'utf8'); const at = m.indexOf('// /Game/Shader/KHS_set');
        for (const [src, dst] of [['xsi/weapons/materials/materials.wgsl', 'xsi/materials.wgsl'], ['xsi/weapons/materials_xwb/materials.wgsl', 'xsi/materials_xwb.wgsl']]) {
          const s = abs('build/sora_pack/' + src); if (exists(s) && at >= 0) put(path.join(S, dst), fs.readFileSync(s, 'utf8') + '\n' + m.slice(at));
        }
        const lib = abs('build/sora_pack/xsi/weapons/materials_xwb/materials.json'); if (exists(lib)) put(path.join(S, 'xwb_lib', 'materials', 'materials.json'), fs.readFileSync(lib));
      }
    }
    const tasks = [];
    const planDir = path.join(ROOT, 'build', 'packgen', 'plan'); fs.mkdirSync(planDir, { recursive: true });
    for (const g of recipe.nf_groups) {
      if (!exists(abs(g.materials)) || !exists(abs(g.pack))) { skipped.push(`group ${g.id}: no ${!exists(abs(g.pack)) ? g.pack : g.materials}`); continue; }
      const list = path.join(planDir, g.id + '.json'); fs.writeFileSync(list, JSON.stringify({ jobs: g.jobs }));
      const out = abs(path.join('build/sora_pack', g.er)); fs.mkdirSync(out, { recursive: true });
      tasks.push({ what: g.id, argv: ['--pack', abs(g.pack), '--materials', abs(g.materials), ...g.args.map((a) => (/^build\//.test(a) ? abs(a) : a)), '--rebuild', list, '--out', out, '--hlsl', path.join(ROOT, 'build', 'packgen', 'hlsl', 'g_' + g.id)] });
    }
    const failed = await parallel(tasks);
    // the copies of a group (the chandelier's VF-b / XW-d folders take the XW-b files)
    for (const g of recipe.nf_groups) for (const c of g.copies || []) { const src = abs(path.join('build/sora_pack', g.er)); if (!exists(src)) continue; const dst = abs(path.join('build/sora_pack', c)); fs.mkdirSync(dst, { recursive: true }); for (const f of fs.readdirSync(src)) if (f.endsWith('.dxil')) fs.copyFileSync(path.join(src, f), path.join(dst, f)); }
    console.log(`packgen_shaders: ${tasks.length - failed.length} of ${recipe.nf_groups.length} rebuild groups${failed.length ? '; FAILED: ' + failed.join(', ') : ''}`);
    if (failed.length) process.exitCode = 1;
  }
  // ---- 3b. (SETUP-TOOLS; AUDIT-C ANLOOK, FINAL's final_post.ps1) the anti-mode owners an outfit got (tools/outfit_anti_owners.js):
  // their every set, then the hair twin's ER91 hair path over the er91 genh translation (after the hair step and the folder's
  // whole build, as the deploys ran them)
  if (step('anti')) {
    for (const a of recipe.anti_jobs || []) {
      if (!exists(abs(a.jobs)) || !exists(abs(a.materials))) { skipped.push(`anti ${a.jobs}: no ${!exists(abs(a.jobs)) ? a.jobs : a.materials}`); continue; }
      shaderc(['--pack', abs(a.pack), '--materials', abs(a.materials), '--rebuild', abs(a.jobs), '--out', abs(path.join('build/sora_pack', a.er)), '--hlsl', path.join(ROOT, 'build', 'packgen', 'hlsl', 'anti_' + path.basename(a.jobs, '.json'))], `anti ${a.jobs}`);
    }
    if ((recipe.anti_jobs || []).length) console.log(`packgen_shaders: ${recipe.anti_jobs.length} anti-mode job list(s)`);
  }
  // ---- 3c. (SETUP-TOOLS; MIMIC-BOSS, DEPLOY3 mimic_writers.ps1) the Mimic Tear's G-buffer writers of every translation that has
  // them (<variant>[_S]_gb3_mm / _gb3_mf and their shaders_mimic*.json indexes) and the pool (mmpool.vs / .ps), from the current
  // harness (the char / groups steps leave them as they were)
  if (step('mimic')) {
    let n = 0;
    for (const m of recipe.mimic_sets || []) {
      if (!exists(abs(m.pack))) { skipped.push(`mimic ${m.pack}: no pack folder`); continue; }
      for (const mode of ['mimic', 'mimicform']) {
        const a = ['--pack', abs(m.pack), '--only', mode, '--out', abs(path.join('build/sora_pack', m.er)), '--hlsl', path.join(ROOT, 'build', 'packgen', 'hlsl', 'mimic')];
        if (m.suffix) a.push('--materials', abs(path.join(m.pack, 'materials', `materials_${m.suffix}.wgsl`)), '--suffix', m.suffix);
        shaderc(a, `${m.er}${m.suffix ? ' [' + m.suffix + ']' : ''} --only ${mode}`); n++;
      }
    }
    if (recipe.mimic_pool) { shaderc(['--pack', abs(recipe.mimic_pool.pack), '--only', 'mimicpool', '--out', abs(path.join('build/sora_pack', recipe.mimic_pool.er)), '--hlsl', path.join(ROOT, 'build', 'packgen', 'hlsl', 'mimic')], 'mimicpool'); n++; }
    if (n) console.log(`packgen_shaders: the Mimic Tear's writers (${n} run(s))`);
  }
  // ---- 3d. (SETUP-TOOLS; PARTY-NATIVE) the effects' fog-copy and lit-kit shaders (fxfog.vs / fxfog_rows / fxfog_vol, fxlit_*)
  if (step('fxcopy')) {
    for (const c of recipe.fx_copies || []) shaderc(['--only', c.only, '--out', abs(path.join('build/sora_pack', c.er)), '--hlsl', path.join(ROOT, 'build', 'packgen', 'hlsl', c.only)], `--only ${c.only}`);
    if ((recipe.fx_copies || []).length) console.log(`packgen_shaders: ${recipe.fx_copies.map((c) => c.only).join(', ')}`);
  }
  // ---- 4. the effect roots
  if (step('vfx')) {
    for (const p of recipe.vfx_roots) {
      if (!exists(abs(path.join(p, 'vfx', 'shaders', 'shading.json')))) { skipped.push(`vfx ${p}: no shading table`); continue; }
      shaderc(['--vfx', '--pack', abs(p), '--jobs', String(JOBS)], 'vfx ' + p);
      // (KBFX) a root of vfx_check_roots: every owner class its table translated must now be built (the er/<s|m|d><id> files
      // are keyed by the table's owner ids: a table made after the build leaves materials on another owner's shader)
      if ((recipe.vfx_check_roots || []).includes(p)) {
        const table = JSON.parse(fs.readFileSync(abs(path.join(p, 'vfx', 'shaders', 'shading.json')), 'utf8'));
        const built = new Set(JSON.parse(fs.readFileSync(abs(path.join(p, 'vfx', 'er', 'shaders.json')), 'utf8')).shaders.map((s) => s.file));
        const missing = Object.values(table.owners).flatMap((o) => [['sprite', 's'], ['mesh', 'm'], ['decal', 'd']].filter(([c]) => o[c] && o[c].ok).map(([, l]) => `${l}${o.id}.ps.dxil`)).filter((f) => !built.has(f));
        if (missing.length) throw new Error(`vfx ${p}: ${missing.length} translated owner shader(s) not built: ${missing.slice(0, 6).join(', ')}`);
      }
      console.log(`packgen_shaders: ${p}/vfx/er`);
    }
  }
  // ---- 5. the effect tables' overlay shaders: kh-shaderc --vfx --only <name> over the overlay translator run of that table
  // (tools/packgen_overlays.js: build/packgen/vfx_stage/<run>), the file copied under the overlay's name
  if (step('vfx-overlays') && (recipe.vfx_overlays || []).length) {
    const STAGE = abs('build/packgen/vfx_stage'), SP = abs('build/packgen/vfx_stagepack');
    const packs = new Map(); let built = 0;
    for (const o of recipe.vfx_overlays) {
      const key = o.stage + '/' + o.table;
      if (!packs.has(key)) {
        const src = path.join(STAGE, o.stage); if (!exists(path.join(src, o.table))) { skipped.push(`vfx overlay ${o.file}: no run ${o.stage} (tools/packgen_overlays.js)`); continue; }
        const d = path.join(SP, o.stage + '_' + o.table.replace(/\.wgsl$/, '')); fs.rmSync(d, { recursive: true, force: true }); fs.mkdirSync(path.join(d, 'vfx', 'shaders'), { recursive: true });
        fs.copyFileSync(path.join(src, 'shading.json'), path.join(d, 'vfx', 'shaders', 'shading.json')); fs.copyFileSync(path.join(src, o.table), path.join(d, 'vfx', 'shaders', 'materials.wgsl'));
        packs.set(key, d);
      }
      const d = packs.get(key);
      shaderc(['--vfx', '--pack', d, '--only', o.only], `vfx overlay ${o.file}`);
      const ext = o.file.slice(o.file.indexOf('.'));
      const made = path.join(d, 'vfx', 'er', o.only + ext);
      if (!exists(made)) { skipped.push(`vfx overlay ${o.file}: kh-shaderc made no ${o.only}${ext}`); continue; }
      const out = path.join(abs(o.root), 'er'); fs.mkdirSync(out, { recursive: true }); fs.copyFileSync(made, path.join(out, o.file)); built++;
    }
    console.log(`packgen_shaders: ${built} effect overlay shader(s)`);
  }
  for (const s of skipped) console.log('   skipped: ' + s);
  fs.writeFileSync(path.join(ROOT, 'build', 'packgen', 'shaders_skipped.json'), JSON.stringify(skipped, null, 1) + '\n');
  console.log(`packgen_shaders: done in ${((Date.now() - t0) / 60000).toFixed(1)} min (log: build/packgen/logs/shaders.log)`);
})().catch((e) => { console.error(String(e.message || e)); process.exit(1); });
