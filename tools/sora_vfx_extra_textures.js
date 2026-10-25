#!/usr/bin/env node
// tools/sora_vfx_extra_textures.js: textures the effect materials sample that the main effect pack run does not
// carry, added to build/sora_pack/vfx (PNG + a manifest.textures row) so that no material slot falls back.
//
//   node tools/sora_vfx_extra_textures.js          add them (idempotent), then run node tools/sora_vfx_shaders.js
//   node tools/sora_vfx_extra_textures.js --check  report only
//
// Why a tool of its own: tools/sora_vfx_pack.js resolves /Game/ paths only (its texture set is what the material
// summaries name); the engine's default textures are named by /Engine/ material functions and by TextureObject
// expressions without a serialized texture (tools/sora_vfx_shaders.js finds them when it rebuilds a material's
// texture list). RUN ORDER: sora_vfx_pack.js -> sora_vfx_host.js -> sora_vfx_extra_textures.js -> sora_vfx_shaders.js
// (a pack run rewrites manifest.json and so drops the rows this tool adds).
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const bc = require('./lib/bcdec');
const PACK = path.join(ROOT, 'build', 'sora_pack', 'vfx');
const TEX = path.join(ROOT, 'assets_converted', 'textures');

// asset path -> converted texture (relative to assets_converted/textures, without extension)
const EXTRA = [
  // the engine's tiling noise: default texture of dither / noise material functions (16 effect materials, e.g. Aeroga's rocks)
  ['/Engine/EngineMaterials/Good64x64TilingNoiseHighFreq.Good64x64TilingNoiseHighFreq', 'Engine/Content/EngineMaterials/Good64x64TilingNoiseHighFreq'],
  ['/Engine/EngineMaterials/DefaultDiffuse.DefaultDiffuse', 'Engine/Content/EngineMaterials/DefaultDiffuse'],
  ['/Engine/EngineMaterials/DefaultNormal.DefaultNormal', 'Engine/Content/EngineMaterials/DefaultNormal'],
  ['/Engine/EngineResources/DefaultTexture.DefaultTexture', 'Engine/Content/EngineResources/DefaultTexture'],
  ['/Engine/EngineResources/WhiteSquareTexture.WhiteSquareTexture', 'Engine/Content/EngineResources/WhiteSquareTexture'],
  ['/Engine/EngineResources/Black.Black', 'Engine/Content/EngineResources/Black'],
  // the icon texture one material samples
  ['/Game/Effects/texcom/tex/icon/jtx_ic_opa_m.jtx_ic_opa_m', 'KINGDOM HEARTS III/Content/Effects/texcom/tex/icon/jtx_ic_opa_m'],
];

function main() {
  const check = process.argv.includes('--check');
  const manifestPath = path.join(PACK, 'manifest.json');
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  const have = new Set((manifest.textures || []).map((t) => t.asset));
  const names = new Set((manifest.textures || []).map((t) => t.file));
  let added = 0;
  for (const [asset, content] of EXTRA) {
    if (have.has(asset)) { console.log('have    ' + asset); continue; }
    const dds = path.join(TEX, content + '.dds'), side = path.join(TEX, content + '.texture.json');
    if (!fs.existsSync(dds) || !fs.existsSync(side)) { console.log('no converted texture: ' + asset); continue; }
    const sc = JSON.parse(fs.readFileSync(side, 'utf8'));
    const sp = {}; for (const x of sc.props || []) sp[x.tag.name] = x.tag.type === 'BoolProperty' ? !!x.tag.boolVal : x.value;
    let t; try { t = bc.decodeDds(fs.readFileSync(dds), 0); } catch (e) { console.log('decode failed: ' + asset + ': ' + e.message); continue; }
    const srgb = sp.SRGB === undefined ? true : !!sp.SRGB;
    let file = 'textures/' + path.basename(content) + '.png';
    if (names.has(file)) file = 'textures/' + path.basename(content) + '__engine.png';
    console.log((check ? 'missing ' : 'added   ') + asset + ' -> ' + file + ` (${t.width}x${t.height} ${t.format}, sRGB ${srgb})`);
    if (check) continue;
    const png = bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb });
    fs.writeFileSync(path.join(PACK, file), png);
    manifest.textures.push({ file, asset, content, class: sc.class, width: t.width, height: t.height, pixel_format: t.format, mip_count: t.mipCount, srgb, address_x: sp.AddressX || 'TA_Wrap', address_y: sp.AddressY || 'TA_Wrap', filter: sp.Filter || 'TF_Default', roles: ['engine default / extra'], used_by: [], added_by: 'tools/sora_vfx_extra_textures.js', converted_from: path.relative(ROOT, dds).replace(/\\/g, '/') });
    names.add(file); added++;
  }
  if (added) fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 1));
  console.log(`${added} texture(s) added to ${path.relative(ROOT, PACK).replace(/\\/g, '/')}; now run: node tools/sora_vfx_shaders.js`);
}
main();
