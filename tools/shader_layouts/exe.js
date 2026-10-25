const { W, ROOT_SLASH } = require('./work');
// exe helpers: string search, VA read. usage:
//   node exe.js str <text> [ansi]      -> VAs of the string
//   node exe.js q <va> [n]             -> n qwords at va
//   node exe.js s <va>                 -> string at va (utf16 then ansi)
const pe = require('../lib/pe');
const img = pe.load();
const base = Number(img.imageBase);
const off2va = (off) => { for (const s of img.sections) if (off >= s.rawPtr && off < s.rawPtr + s.rawSize) return base + s.rva + (off - s.rawPtr); return -1; };
const va2off = (va) => img.rvaToOff(va - base);
function findStr(text, ansi) {
  const needle = ansi ? Buffer.from(text + '\0', 'latin1') : Buffer.from(text + '\0', 'utf16le');
  const out = [];
  for (let p = 0; (p = img.b.indexOf(needle, p)) >= 0; p++) {
    // must start a string: previous char is NUL
    const ok = ansi ? img.b[p - 1] === 0 : img.b[p - 1] === 0; // the previous string ended (it may be an 8-bit string)
    if (ok) out.push(off2va(p));
  }
  return out;
}
const q = (va) => Number(img.b.readBigUInt64LE(va2off(va)));
function str(va) {
  const o = va2off(va); if (o < 0) return null;
  let e = o; while (img.b.readUInt16LE(e) !== 0 && e - o < 600) e += 2;
  const u = img.b.toString('utf16le', o, e);
  if (u.length > 1 && /^[\x20-\x7e]+$/.test(u)) return u;
  e = o; while (img.b[e] !== 0 && e - o < 300) e++;
  return 'A:' + img.b.toString('latin1', o, e);
}
module.exports = { img, base, off2va, va2off, findStr, q, str };
if (require.main === module) {
  const [cmd, a, b] = process.argv.slice(2);
  if (cmd === 'str') console.log(findStr(a, !!b).map((v) => v.toString(16).toUpperCase()));
  if (cmd === 'q') { const va = parseInt(a, 16); for (let i = 0; i < +(b || 8); i++) console.log((va + i * 8).toString(16), q(va + i * 8).toString(16)); }
  if (cmd === 's') console.log(str(parseInt(a, 16)));
}
