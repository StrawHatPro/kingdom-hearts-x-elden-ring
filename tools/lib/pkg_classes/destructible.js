// UDestructibleMesh: a USkeletalMesh export followed by the APEX destructible asset.
// Docs: docs/formats/misc_classes.md.
//
//   [USkeletalMesh export data: properties, guid flag, render data]   read and re-written by tools/lib/mesh.js
//   uint32 name length, name bytes                                     name of the APEX asset
//   uint32 size, size bytes                                            APEX asset, NvParameterized binary serialization
//
// The skeletal part has no length field. The boundary is the only position where the remaining
// bytes are exactly "length-prefixed name, length-prefixed blob up to the end of the export" and
// tools/lib/mesh.js reads the part before it to its last byte and writes the same bytes back.
const crypto = require('crypto');
const { T, register } = require('../package');
const mesh = require('../mesh');

const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const TOOL = 'tools/lib/mesh.js (SkeletalMesh serializer)';

register('DestructibleMesh', {
  wholeExport: true,
  r: (r, c) => {
    const b = r.b, end = b.length;
    let found = null;
    const MAGIC = Buffer.from('5a5b5c5d', 'hex'); // NvParameterized binary header
    for (let m = b.indexOf(MAGIC, 8); m >= 0 && !found; m = b.indexOf(MAGIC, m + 1)) {
      // q: position of the asset size; the name precedes it; sized blobs must then run to the end
      const q = m - 4;
      const size = b.readUInt32LE(q);
      if (q + 4 + size > end) continue;
      const rest = [];
      let t = q + 4 + size, okChain = true;
      while (t < end) { if (t + 4 > end) { okChain = false; break; } const n2 = b.readUInt32LE(t); if (t + 4 + n2 > end) { okChain = false; break; } rest.push([t + 4, n2]); t += 4 + n2; }
      if (!okChain) continue;
      for (let n = 0; n <= 256 && q - 4 - n >= 0; n++) {
        if (b.readUInt32LE(q - 4 - n) !== n) continue;
        const p = q - 4 - n;
        const name = b.subarray(p + 4, q);
        if (![...name].every((ch) => ch === 0 || (ch >= 0x20 && ch < 0x7f))) continue;
        try {
          const fake = { serialOffset: c.pkg.summary.totalHeaderSize, serialSize: p };
          const { doc, blobs } = mesh.parseExport(c.pkg, b.subarray(0, p), fake, 'SkeletalMesh');
          if (!mesh.serialize(c.pkg, 'SkeletalMesh', doc, blobs).equals(b.subarray(0, p))) continue;
          found = { p, q, name, size, rest };
        } catch (e) { continue; }
        break;
      }
    }
    if (!found) throw new Error('DestructibleMesh: skeletal mesh / APEX boundary not found');
    const skel = b.subarray(0, found.p);
    const apex = b.subarray(found.q + 4, found.q + 4 + found.size);
    const more = found.rest.map(([o, n]) => b.subarray(o, o + n));
    if (c.stats) {
      c.stats.delegated += skel.length;
      c.stats.thirdParty = c.stats.thirdParty || {};
      const k = 'NVIDIA APEX destructible asset (NvParameterized binary, magic 5A 5B 5C 5D)';
      c.stats.thirdParty[k] = (c.stats.thirdParty[k] || 0) + apex.length;
      const k2 = 'NVIDIA APEX destructible collision cache ("DestructibleAsset.10", PhysX cooked convex meshes)';
      for (const x of more) c.stats.thirdParty[k2] = (c.stats.thirdParty[k2] || 0) + x.length;
    }
    r.p = end;
    return {
      skeletalMesh: { delegated: TOOL, size: skel.length, sha256: sha256(skel) },
      apexAssetName: found.name.toString('latin1'),
      apexAsset: apex.toString('hex'),
      // further length-prefixed blobs up to the end of the export
      trailingBlobs: more.map((x) => x.toString('hex')),
    };
  },
  w: (w, v, c) => {
    if (!c.opts || !c.opts.delegatedBytes) throw new Error('DestructibleMesh needs opts.delegatedBytes for its skeletal mesh part');
    const whole = c.opts.delegatedBytes(c.exportIndex, null);
    const skel = whole.subarray(0, v.skeletalMesh.size);
    if (sha256(skel) !== v.skeletalMesh.sha256) throw new Error('DestructibleMesh skeletal part does not match its recorded hash');
    w.push(skel);
    const name = Buffer.from(v.apexAssetName, 'latin1');
    w.u32(name.length); w.push(name);
    w.u32(v.apexAsset.length / 2); w.hex(v.apexAsset);
    for (const x of v.trailingBlobs) { w.u32(x.length / 2); w.hex(x); }
  },
}, 'destructible');
void T;
