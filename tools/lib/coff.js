// Minimal COFF (x64, as written by cl) reader and single-object linker. Produces a flat blob
// for a fixed load address so the harness can run a recompiled function without link.exe.
const SKIP_NAME = /^(\.drectve|\.debug|\.pdata|\.xdata|\.chks64|\.voltbl|\.llvm|\.gfids|\.gehcont|\.giats)/;

function parse(buf) {
  const nSections = buf.readUInt16LE(2);
  const symPtr = buf.readUInt32LE(8), nSyms = buf.readUInt32LE(12);
  if (buf.readUInt16LE(0) !== 0x8664) throw new Error('not an x64 COFF object');
  const strTab = symPtr + nSyms * 18;
  const str = (o) => { let e = strTab + o; while (buf[e]) e++; return buf.toString('latin1', strTab + o, e); };
  const name8 = (o) => (buf.readUInt32LE(o) === 0 ? str(buf.readUInt32LE(o + 4)) : buf.toString('latin1', o, o + 8).replace(/\0+$/, ''));
  const sections = [];
  for (let i = 0; i < nSections; i++) {
    const o = 20 + i * 40;
    let name = buf.toString('latin1', o, o + 8).replace(/\0+$/, '');
    if (name[0] === '/') name = str(parseInt(name.slice(1), 10));
    const s = { index: i + 1, name, rawSize: buf.readUInt32LE(o + 16), rawPtr: buf.readUInt32LE(o + 20), relPtr: buf.readUInt32LE(o + 24), nRel: buf.readUInt16LE(o + 32), flags: buf.readUInt32LE(o + 36) };
    s.align = 1 << Math.max(0, ((s.flags >>> 20) & 0xf) - 1);
    s.keep = !SKIP_NAME.test(name) && !(s.flags & 0x02000a00) && (s.flags & 0xe0) !== 0 && s.rawSize > 0;
    sections.push(s);
  }
  const symbols = [];
  for (let i = 0; i < nSyms; i++) {
    const o = symPtr + i * 18;
    const aux = buf[o + 17];
    symbols[i] = { name: name8(o), value: buf.readUInt32LE(o + 8), section: buf.readInt16LE(o + 12), storage: buf[o + 16] };
    i += aux;
  }
  // "/alternatename:a=b" in the linker directives: use b when nothing defines a. The compiler
  // writes one for __isa_available (tested by loops it vectorises), naming a zero in this object.
  const alternate = new Map();
  for (const s of sections) if (s.name === '.drectve') for (const m of buf.toString('latin1', s.rawPtr, s.rawPtr + s.rawSize).matchAll(/\/alternatename:([^\s=]+)=(\S+)/gi)) alternate.set(m[1], m[2]);
  return { buf, sections, symbols, alternate };
}

// Lay the kept sections out from `loadAddr` (code first) and apply relocations.
// resolve(name) -> absolute address (BigInt or number) for undefined externals, or throws.
function link(obj, entryName, loadAddr, imageBase, resolve) {
  const { buf, sections, symbols } = obj;
  const kept = sections.filter((s) => s.keep).sort((a, b) => ((b.flags & 0x20) - (a.flags & 0x20)) || a.index - b.index);
  let size = 0;
  for (const s of kept) {
    size = Math.ceil(size / s.align) * s.align;
    s.base = size;
    size += s.rawSize;
  }
  const out = Buffer.alloc(size, 0xcc);
  for (const s of kept) {
    if (s.flags & 0x80) out.fill(0, s.base, s.base + s.rawSize); // .bss
    else buf.copy(out, s.base, s.rawPtr, s.rawPtr + s.rawSize);
  }
  const bySection = new Map(kept.map((s) => [s.index, s]));
  const addrOf = (sym) => {
    if (sym.section > 0) {
      const s = bySection.get(sym.section);
      if (!s) throw new Error(`symbol ${sym.name} is in a dropped section`);
      return BigInt(loadAddr) + BigInt(s.base + sym.value);
    }
    if (sym.section === 0 && obj.alternate && obj.alternate.has(sym.name)) {
      const alt = symbols.find((x) => x && x.name === obj.alternate.get(sym.name) && x.section > 0 && bySection.has(x.section));
      if (alt) return addrOf(alt);
    }
    if (sym.section === 0) return BigInt(resolve(sym.name));
    return BigInt(sym.value); // absolute
  };
  const externals = new Set();
  for (const s of kept) {
    for (let i = 0; i < s.nRel; i++) {
      const r = s.relPtr + i * 10;
      const at = s.base + buf.readUInt32LE(r);
      const sym = symbols[buf.readUInt32LE(r + 4)];
      const type = buf.readUInt16LE(r + 8);
      if (sym.section === 0) externals.add(sym.name);
      const S = addrOf(sym);
      const P = BigInt(loadAddr) + BigInt(at);
      if (type === 1) out.writeBigUInt64LE(BigInt.asUintN(64, S + out.readBigInt64LE(at)), at);
      else if (type >= 4 && type <= 9) {
        const v = S + BigInt(out.readInt32LE(at)) - (P + 4n + BigInt(type - 4));
        if (v < -0x80000000n || v > 0x7fffffffn) throw new Error(`REL32 to ${sym.name} out of range`);
        out.writeInt32LE(Number(v), at);
      } else if (type === 3) {
        const v = S - BigInt(imageBase) + BigInt(out.readInt32LE(at));
        if (v < 0n || v > 0xffffffffn) throw new Error(`ADDR32NB to ${sym.name} out of range`);
        out.writeUInt32LE(Number(v), at);
      } else throw new Error(`unsupported relocation type ${type} against ${sym.name}`);
    }
  }
  const entry = symbols.find((x) => x && x.name === entryName && x.section > 0 && bySection.has(x.section) && (bySection.get(x.section).flags & 0x20));
  if (!entry) throw new Error(`entry symbol ${entryName} not found`);
  return { bytes: out, entry: bySection.get(entry.section).base + entry.value, externals: [...externals] };
}

module.exports = { parse, link };
