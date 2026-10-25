// (IMPL_ER43) ELDEN RING'S ITEM ICONS -> the UI pack, so the camp menu's Key Items pane shows an Elden Ring key item's picture.
//
//   node tools/er_icons.js [--game DIR] [--work DIR] [--pack DIR]
//     --game  the Elden Ring install's Game folder (READ ONLY: nothing is ever written there; regulation.bin is never read)
//     --work  scratch folder for the decrypted archive headers and the two extracted files (default build/sora_tmp/er/er43/game)
//     --pack  the UI pack (default build/sora_pack/ui)
//
// What it does (every step reads the install only; the outputs stay local under build/, which git ignores):
//  1. The five archive headers Data0..3.bhd / DLC.bhd are decrypted with the RSA public keys stored as PEM text in
//     eldenring.exe (cached in <work>/_bhd/*.bhd5; the method of build/sora_tmp/er/er33/tools/bhd.js, IMPL_ER33 section 7).
//  2. Two files are taken out of the Data*.bdt archives by their path hash (64-bit, x133), AES-128-ECB ranges undone:
//       /menu/hi/01_common.tpf.dcx       the menu's common textures: the item icon atlases SB_Icon_00 .. SB_Icon_08_dlc
//                                        (base game and Shadow of the Erdtree in one file; BC7 4096x2048 / 2048x2048 ...)
//       /menu/hi/01_common.sblytbnd.dcx  their layouts: SB_Icon_*.layout = <SubTexture name="MENU_ItemIcon_NNNNN.png"
//                                        x y width height/> - NNNNN is EquipParamGoods.iconId (the exe formats
//                                        "MENU_ItemIcon_%05d"); every icon is 160 x 160 in the hi atlases
//  3. DCX (KRAK) is undone with the game's own Oodle library, run from a COPY in <work>/oodle (PowerShell, one call).
//  4. Each atlas is decoded (tools/lib/bcdec.js) and every MENU_ItemIcon cut out at its own size (160 x 160: the item
//     pane's picture is drawn at about 150 px on a 1080p stage, so no rescale) -> <pack>/er_icons/NNNNN.png, and the index
//     <pack>/data/er_icons.json: { icons: { "<iconId>": "er_icons/NNNNN.png" }, ... } read by crates/kh-ui/src/item_art.rs.
// (/menu/hi/00_solo.tpfbhd holds MENU_Knowledge_NNNNN, the large pictures of Elden Ring's own item detail, 1.4 GB: not used.)
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { spawnSync } = require('child_process');
const bc = require('./lib/bcdec');

const VERSION = '1.0.0';
const ROOT = path.join(__dirname, '..');
const arg = (name, dflt) => { const k = process.argv.indexOf(name); return k > 0 && process.argv[k + 1] ? process.argv[k + 1] : dflt; };
const GAME = arg('--game', 'C:/Program Files (x86)/Steam/steamapps/common/ELDEN RING/Game');
const WORK = path.resolve(arg('--work', path.join(ROOT, 'build/sora_tmp/er/er43/game')));
const PACK = path.resolve(arg('--pack', path.join(ROOT, 'build/sora_pack/ui')));
const ARCHIVES = ['Data0', 'Data1', 'Data2', 'Data3', 'DLC'];
const FILES = ['/menu/hi/01_common.tpf.dcx', '/menu/hi/01_common.sblytbnd.dcx'];
if (path.resolve(WORK).toLowerCase().startsWith(path.resolve(GAME).toLowerCase()) || PACK.toLowerCase().startsWith(path.resolve(GAME).toLowerCase())) throw new Error('refusing to write inside the game folder');

// ---------------------------------------------------------------- 1-2: archive headers, single files
function rsaKeys() {
  const exe = fs.readFileSync(path.join(GAME, 'eldenring.exe'));
  const begin = Buffer.from('-----BEGIN RSA PUBLIC KEY-----'), end = Buffer.from('-----END RSA PUBLIC KEY-----');
  const out = [];
  for (let i = 0; (i = exe.indexOf(begin, i)) >= 0;) { const e = exe.indexOf(end, i); out.push(exe.slice(i, e + end.length).toString('latin1') + '\n'); i = e; }
  return out;
}

function header(name, keys) {
  const cache = path.join(WORK, '_bhd', name + '.bhd5');
  if (fs.existsSync(cache)) return fs.readFileSync(cache);
  const enc = fs.readFileSync(path.join(GAME, name + '.bhd'));
  for (const pem of keys()) {
    try {
      const blocks = [];
      for (let o = 0; o + 256 <= enc.length; o += 256) blocks.push(crypto.publicDecrypt({ key: pem, padding: crypto.constants.RSA_NO_PADDING }, enc.slice(o, o + 256)).slice(1));
      const dec = Buffer.concat(blocks);
      if (dec.slice(0, 4).toString('latin1') !== 'BHD5') continue;
      fs.mkdirSync(path.dirname(cache), { recursive: true });
      fs.writeFileSync(cache, dec);
      return dec;
    } catch (e) { /* not this key */ }
  }
  throw new Error('no key of eldenring.exe decrypts ' + name + '.bhd');
}

function hashPath(p) {
  let h = 0n;
  for (const ch of p.toLowerCase().replace(/\\/g, '/')) h = (h * 0x85n + BigInt(ch.charCodeAt(0))) & 0xFFFFFFFFFFFFFFFFn;
  return h;
}

function findEntry(b, hash) {
  const bucketCount = b.readInt32LE(0x10), bucketsOffset = b.readInt32LE(0x14);
  for (let k = 0; k < bucketCount; k++) {
    const n = b.readInt32LE(bucketsOffset + k * 8), off = b.readInt32LE(bucketsOffset + k * 8 + 4);
    for (let f = 0; f < n; f++) {
      const o = off + f * 0x28;
      if (b.readBigUInt64LE(o) !== hash) continue;
      const aesOff = Number(b.readBigInt64LE(o + 32));
      let aes = null;
      if (aesOff > 0) {
        const ranges = [];
        for (let r = 0, rc = b.readInt32LE(aesOff + 16); r < rc; r++) {
          const s = b.readBigInt64LE(aesOff + 20 + r * 16), e = b.readBigInt64LE(aesOff + 28 + r * 16);
          if (s >= 0n && e > s) ranges.push([Number(s), Number(e)]);
        }
        aes = { key: b.slice(aesOff, aesOff + 16), ranges };
      }
      return { padded: b.readInt32LE(o + 8), unpadded: b.readInt32LE(o + 12), fileOffset: Number(b.readBigInt64LE(o + 16)), aes };
    }
  }
  return null;
}

function extract(want) {
  const out = path.join(WORK, want.replace(/^[\\/]/, ''));
  if (fs.existsSync(out)) return out;
  let keys = null;
  const lazyKeys = () => keys || (keys = rsaKeys());
  for (const name of ARCHIVES) {
    const e = findEntry(header(name, lazyKeys), hashPath(want));
    if (!e) continue;
    const buf = Buffer.alloc(e.padded);
    const fd = fs.openSync(path.join(GAME, name + '.bdt'), 'r');
    fs.readSync(fd, buf, 0, e.padded, e.fileOffset);
    fs.closeSync(fd);
    for (const [s, en] of e.aes ? e.aes.ranges : []) {
      const d = crypto.createDecipheriv('aes-128-ecb', e.aes.key, null);
      d.setAutoPadding(false);
      Buffer.concat([d.update(buf.slice(s, en)), d.final()]).copy(buf, s);
    }
    fs.mkdirSync(path.dirname(out), { recursive: true });
    fs.writeFileSync(out, e.unpadded > 0 ? buf.slice(0, e.unpadded) : buf);
    console.log(`${want}: ${name}.bdt @${e.fileOffset}, ${e.padded} bytes -> ${out}`);
    return out;
  }
  throw new Error(want + ': in none of the archives');
}

// ---------------------------------------------------------------- 3: DCX (KRAK) through the game's Oodle (a copy)
function undcx(files) {
  const todo = files.filter((f) => !fs.existsSync(f.replace(/\.dcx$/, '')));
  if (!todo.length) return;
  const oodle = path.join(WORK, 'oodle');
  fs.mkdirSync(oodle, { recursive: true });
  const dll = path.join(oodle, 'oo2core_6_win64.dll');
  if (!fs.existsSync(dll)) fs.copyFileSync(path.join(GAME, 'oo2core_6_win64.dll'), dll);
  const ps1 = path.join(oodle, 'undcx.ps1');
  fs.writeFileSync(ps1, `param([string]$OodleDir, [string]$Files)
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class Oo {
    [DllImport("kernel32", SetLastError=true, CharSet=CharSet.Unicode)] public static extern IntPtr LoadLibrary(string p);
    [DllImport("oo2core_6_win64.dll")] public static extern long OodleLZ_Decompress(byte[] buf, long bufSize, byte[] outBuf, long outSize, int fuzz, int crc, int verbose, IntPtr a, long b, IntPtr c, IntPtr d, IntPtr e, long f, int threadPhase);
}
"@
[void][Oo]::LoadLibrary((Join-Path $OodleDir 'oo2core_6_win64.dll'))
function BE([byte[]]$b, [int]$o) { return ([int]$b[$o] -shl 24) -bor ([int]$b[$o+1] -shl 16) -bor ([int]$b[$o+2] -shl 8) -bor [int]$b[$o+3] }
foreach ($f in $Files.Split("|")) {
    $b = [IO.File]::ReadAllBytes($f)
    if ([Text.Encoding]::ASCII.GetString($b, 0x28, 4) -ne 'KRAK') { "$($f): not KRAK"; exit 2 }
    $usize = BE $b 0x1C; $csize = BE $b 0x20
    $data = New-Object byte[] $csize; [Array]::Copy($b, 0x4C, $data, 0, $csize)
    $o = New-Object byte[] $usize
    $n = [Oo]::OodleLZ_Decompress($data, $csize, $o, $usize, 1, 0, 0, [IntPtr]::Zero, 0, [IntPtr]::Zero, [IntPtr]::Zero, [IntPtr]::Zero, 0, 3)
    if ($n -ne $usize) { "$($f): OODLE FAILED ($n of $usize)"; exit 3 }
    [IO.File]::WriteAllBytes($f.Substring(0, $f.Length - 4), $o); "$($f): $csize -> $usize"
}
`);
  const r = spawnSync('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', ps1, '-OodleDir', oodle, '-Files', todo.join("|")], { encoding: 'utf8' });
  process.stdout.write(r.stdout || '');
  if (r.status !== 0) throw new Error('DCX: ' + (r.stderr || r.stdout));
}

// ---------------------------------------------------------------- BND4 / TPF / layouts
function bnd4(b) {
  if (b.slice(0, 4).toString('latin1') !== 'BND4') throw new Error('not BND4');
  const count = b.readInt32LE(0x0C), entry = Number(b.readBigInt64LE(0x20)), unicode = b[0x30] !== 0;
  const out = new Map();
  for (let k = 0; k < count; k++) {
    const o = 0x40 + k * entry; // flags, -1, compressed size (i64), uncompressed size (i64), offset (u32), id (i32), name offset (u32)
    const size = Number(b.readBigInt64LE(o + 8)), off = b.readUInt32LE(o + 24), no = b.readUInt32LE(o + 32);
    let e = no, name;
    if (unicode) { while (b.readUInt16LE(e) !== 0) e += 2; name = b.slice(no, e).toString('utf16le'); } else { while (b[e] !== 0) e++; name = b.slice(no, e).toString('latin1'); }
    out.set(name.replace(/\\/g, '/').split('/').pop(), b.slice(off, off + size));
  }
  return out;
}

function tpf(b) {
  if (b.slice(0, 4).toString('latin1') !== 'TPF\0') throw new Error('not TPF');
  const out = new Map();
  for (let k = 0, n = b.readInt32LE(8); k < n; k++) {
    const o = 0x10 + k * 0x14; // data offset, size, format, cube, mips, flags, name offset (UTF-16), 0
    const off = b.readUInt32LE(o), size = b.readInt32LE(o + 4), no = b.readUInt32LE(o + 12);
    let e = no; while (b.readUInt16LE(e) !== 0) e += 2;
    out.set(b.slice(no, e).toString('utf16le'), b.slice(off, off + size));
  }
  return out;
}

function layoutIcons(xml) {
  const atlas = /imagePath="([^".]+)/.exec(xml)[1];
  const icons = [];
  for (const m of xml.matchAll(/<SubTexture\s+name="MENU_ItemIcon_(\d+)\.png"\s+x="(\d+)"\s+y="(\d+)"\s+width="(\d+)"\s+height="(\d+)"/g)) {
    icons.push({ atlas, id: parseInt(m[1], 10), name: m[1], x: +m[2], y: +m[3], w: +m[4], h: +m[5] });
  }
  return icons;
}

// ---------------------------------------------------------------- main
function main() {
  const [tpfDcx, lytDcx] = FILES.map(extract);
  undcx([tpfDcx, lytDcx]);
  const textures = tpf(fs.readFileSync(tpfDcx.replace(/\.dcx$/, '')));
  const layouts = bnd4(fs.readFileSync(lytDcx.replace(/\.dcx$/, '')));
  const icons = [];
  for (const [name, data] of layouts) if (/^SB_Icon_.*\.layout$/.test(name)) icons.push(...layoutIcons(data.toString('utf8')));
  const byAtlas = new Map();
  for (const ic of icons) byAtlas.set(ic.atlas, [...(byAtlas.get(ic.atlas) || []), ic]);
  fs.mkdirSync(path.join(PACK, 'er_icons'), { recursive: true });
  const index = {
    format: 'kh3-ui-data/1',
    generator: 'tools/er_icons.js ' + VERSION,
    source: 'ELDEN RING Data0.bdt /menu/hi/01_common.tpf.dcx (SB_Icon_* atlases, BC7) + /menu/hi/01_common.sblytbnd.dcx (SB_Icon_*.layout)',
    note: 'icons[iconId] = the picture of Elden Ring\'s MENU_ItemIcon_<iconId> (EquipParamGoods.iconId), cut from its atlas at its own size (RGBA PNG). ' +
      'Read by crates/kh-ui/src/item_art.rs for the camp menu\'s Key Items pane (host key items HOSTKEY_n.icon<iconId>). ' +
      'Local only: made from the user\'s own install by `node tools/er_icons.js` (read-only; never regulation.bin); not in git.',
    size: null,
    icons: {},
  };
  const sizes = new Set();
  for (const [atlas, list] of [...byAtlas].sort()) {
    const dds = textures.get(atlas);
    if (!dds) { console.log(`${atlas}: not in 01_common.tpf (${list.length} icons skipped)`); continue; }
    const t = bc.decodeDds(dds, 0);
    for (const ic of list) {
      const rgba = Buffer.alloc(ic.w * ic.h * 4);
      for (let y = 0; y < ic.h; y++) {
        const sy = ic.y + y;
        if (sy >= t.height) break;
        t.rgba.copy(rgba, y * ic.w * 4, (sy * t.width + ic.x) * 4, (sy * t.width + Math.min(ic.x + ic.w, t.width)) * 4);
      }
      const file = `er_icons/${ic.name}.png`;
      fs.writeFileSync(path.join(PACK, file), bc.encodePng(ic.w, ic.h, rgba, { alpha: true }));
      index.icons[String(ic.id)] = file;
      sizes.add(`${ic.w}x${ic.h}`);
    }
    console.log(`${atlas}: ${t.width}x${t.height} ${t.format}, ${list.length} icons`);
  }
  index.size = [...sizes];
  fs.writeFileSync(path.join(PACK, 'data/er_icons.json'), JSON.stringify(index, null, 1));
  console.log(`${Object.keys(index.icons).length} icons -> ${path.join(PACK, 'er_icons')} (sizes ${index.size.join(', ')}), index data/er_icons.json`);
}

main();
