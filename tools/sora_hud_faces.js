// (OF) KH1 SORA'S HUD FACE MOVIE (THE USER 2026-11-03: "make sure Sora's HUD changes for his costumes and also when he is in
// battle his battle HUD picture (for each costume) is used"; then "render the KH1 model's own head for every face", shipped to
// everyone). A step of the UI pack, after tools/sora_ui_pack.js (tools/packgen.js step `ui-faces`); Node only, from the
// player's own KH3 data; no picture of ours is in the release.
//
// KH3's face of the KH1 outfit (pawn p_ex005, its UI/Data/Character/p_ex005_StatusData names FacePcEx1Sora01) is ONE picture
// (the KH1 illustration, texture PcEx1Sora01) shown for every look. The game renders KH1 Sora's own head for every look at
// run time (kh-er hudface.rs, kh-host face_render.rs, the renderer's one-shot capture) into the cells of KH3's look layout.
// This step makes the movie for it: FacePcEx1Sora01Made = a copy of FacePcEx2Sora01 (KH3's face movie of the KH2 outfit:
// cells normal / battle / damage / dying / Anti in one 1024 x 512 texture) on a sheet of its own, the STAND-IN the HUD shows
// until (or without: `hud.kh1_render = 0`, a failed capture) the rendered one:
//   - normal, battle, damage, dying: KH3's KH1 illustration in each (placed where its own movie places it: as KH3 shows it);
//   - the Anti face: KH3's KH2 outfit's (the dark silhouette).
// Writes build/sora_pack/ui/movies/FacePcEx1Sora01Made.json, textures/Scaleform/01_hud/face/PcEx1Sora01Made.png and their
// manifest.json / textures/atlas.json entries. Usage: node tools/sora_hud_faces.js [--check]
'use strict';
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const bc = require('./lib/bcdec.js');
const ROOT = path.resolve(__dirname, '..');
const UI = path.join(ROOT, 'build/sora_pack/ui');
const VERSION = '2.0.0'; // 2.0.0: the KH1 picture in every cell (the game renders the looks); 1.0.0 recoloured KH2 faces
const checkOnly = process.argv.includes('--check');
const FACE = 'textures/Scaleform/01_hud/face/';
const SRC_MOVIE = 'FacePcEx2Sora01', NAME = 'FacePcEx1Sora01Made';
const SRC_TEX = FACE + 'PcEx2Sora01.png', KH1_TEX = FACE + 'PcEx1Sora01.png', OUT_TEX = FACE + 'PcEx1Sora01Made.png';
// the cells of the KH2 movie's texture (x, y, w, h): normal, battle, damage, dying, Anti (from its shapes' bitmap fills)
const CELLS = { normal: [0, 0, 256, 276], battle: [256, 0, 256, 276], damage: [512, 0, 256, 276], dying: [768, 0, 256, 276] }; // the Anti face (the dark silhouette, x 620.., y 232..) is kept as KH3 draws it
// the KH1 picture's place: its movie puts texture pixel (u, v) at stage (u - 130, v - 146); the KH2 movie's normal cell
// puts texture pixel (x, y) at stage (x - 128, y - 144): KH1 (u, v) -> (u - 2, v - 2)
const KH1_SHIFT = [-2, -2];

const read = (rel) => bc.decodePng(fs.readFileSync(path.join(UI, rel)));

function main() {
  const src = read(SRC_TEX), kh1 = read(KH1_TEX);
  if (src.width !== 1024 || src.height !== 512) throw new Error(`${SRC_TEX}: ${src.width} x ${src.height}, the cells expect 1024 x 512`);
  const out = Buffer.from(src.data);
  // every look's cell: KH3's KH1 picture (the Anti face, beside them, stays the KH2 outfit's)
  for (const [nx, ny, nw, nh] of Object.values(CELLS)) {
    for (let y = ny; y < ny + nh; y++) for (let x = nx; x < nx + nw; x++) {
      const i = (y * 1024 + x) * 4; const u = x - nx - KH1_SHIFT[0], v = y - ny - KH1_SHIFT[1];
      if (u < 0 || v < 0 || u >= kh1.width || v >= kh1.height) { out.fill(0, i, i + 4); continue; }
      const j = (v * kh1.width + u) * 4; kh1.data.copy(out, i, j, j + 4);
    }
  }
  const png = bc.encodePng(1024, 512, out, { alpha: true, srgb: true });
  // the movie: KH3's KH2 face movie on the new texture
  const movie = JSON.parse(fs.readFileSync(path.join(UI, 'movies', SRC_MOVIE + '.json'), 'utf8'));
  movie.name = NAME;
  movie.made_by = `tools/sora_hud_faces.js ${VERSION}: ${SRC_MOVIE} (KH3's look layout) on ${OUT_TEX} (stand-in cells: KH3's KH1 face ${KH1_TEX} in each; the game renders the looks into it)`;
  const text = JSON.stringify(movie).split(SRC_TEX).join(OUT_TEX);
  // manifest + atlas entries
  const manifest = JSON.parse(fs.readFileSync(path.join(UI, 'manifest.json'), 'utf8'));
  const atlas = JSON.parse(fs.readFileSync(path.join(UI, 'textures/atlas.json'), 'utf8'));
  const srcEntry = manifest.movies.find((m) => m.name === SRC_MOVIE);
  const entry = Object.assign({}, srcEntry, { name: NAME, source: movie.made_by, source_sha256: crypto.createHash('sha256').update(text).digest('hex'), files: { movie: `movies/${NAME}.json`, as3: srcEntry.files.as3, layout: srcEntry.files.layout } });
  const at = manifest.movies.findIndex((m) => m.name === NAME); if (at >= 0) manifest.movies[at] = entry; else manifest.movies.push(entry);
  const srcAtlas = atlas.textures[SRC_TEX] || { regions: [] };
  atlas.textures[OUT_TEX] = { size: [1024, 512], pixel_format: 'made', source: `tools/sora_hud_faces.js from ${SRC_TEX} + ${KH1_TEX}`, used_by: [NAME], regions: srcAtlas.regions };
  const tex = { file: OUT_TEX, source: atlas.textures[OUT_TEX].source, size: [1024, 512], pixel_format: 'made', used_by: [NAME], regions: srcAtlas.regions.length };
  const ti = manifest.textures.findIndex((t) => t.file === OUT_TEX); if (ti >= 0) manifest.textures[ti] = tex; else manifest.textures.push(tex);
  manifest.counts.movies = manifest.movies.length; manifest.counts.textures = manifest.textures.length;
  if (!checkOnly) {
    fs.writeFileSync(path.join(UI, OUT_TEX), png);
    fs.writeFileSync(path.join(UI, 'movies', NAME + '.json'), text);
    fs.writeFileSync(path.join(UI, 'textures/atlas.json'), JSON.stringify(atlas));
    fs.writeFileSync(path.join(UI, 'manifest.json'), JSON.stringify(manifest, null, 1) + '\n');
  }
  console.log(`sora_hud_faces ${VERSION}: ${NAME} (the KH1 picture in ${Object.keys(CELLS).length} cells)${checkOnly ? ' (check only)' : ' -> build/sora_pack/ui'}`);
}
main();
