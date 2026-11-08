// An f32 literal for generated Rust (2026-11-01, the user: "use Clippy and treat it as Bible"): the same f32 as the old
// printer's text, in the shortest form (Clippy's excessive_precision). A game value that merely looks like a math constant
// keeps its exact bits through f32::from_bits (approx_constant): it is not that constant, and must not become it.
// Clippy's list and rule (clippy_lints/src/approx_const.rs): a value matches when it is longer than the constant's minimum
// digits and is a prefix of the constant's text, or equals the constant rounded to the same number of decimals.
const KNOWN = [
  [Math.E, 4], [1 / Math.PI, 4], [Math.SQRT1_2, 5], [2 / Math.PI, 5], [2 / Math.sqrt(Math.PI), 5], [Math.PI / 2, 5],
  [Math.PI / 3, 5], [Math.PI / 4, 5], [Math.PI / 6, 5], [Math.PI / 8, 5], [Math.LN2, 5], [Math.LN10, 5],
  [Math.log10(2), 5], [Math.LOG10E, 5], [Math.log2(10), 5], [Math.LOG2E, 5], [Math.PI, 3], [Math.SQRT2, 5], [Math.PI * 2, 3],
];
const looksConstant = (text) => {
  const t = text.replace(/^-/, '');
  if (!t.includes('.') || /e/i.test(t)) return false;
  return KNOWN.some(([c, digits]) => t.length > digits && (String(c).startsWith(t) || c.toFixed(t.length - 2) === t));
};
const shortest = (v) => {
  for (let p = 1; p <= 9; p++) {
    const t = Number(v.toPrecision(p));
    if (Math.fround(t) === v) return String(t);
  }
  return String(v);
};
const bits = (v) => {
  const b = Buffer.alloc(4);
  b.writeFloatLE(v, 0);
  return `0x${b.readUInt32LE(0).toString(16).padStart(8, '0')}`;
};
function f32(n) {
  n = Number(n);
  const v = Math.fround(Math.fround(n) === n ? n : Number(n.toPrecision(9)));
  const s = shortest(v);
  if (looksConstant(s)) return `f32::from_bits(${bits(v)})`;
  return /[.eE]/.test(s) ? s : `${s}.0`;
}
module.exports = { f32 };
