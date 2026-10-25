const { W, ROOT_SLASH } = require('./work');
// disassemble a function from base/KH3.exe with iced-x86: node dis.js <hexVA> [maxInstr]
const { Decoder, DecoderOptions, Formatter, FormatterSyntax, Instruction } = require('iced-x86');
const X = require('./exe');
function dis(va, max = 400) {
  const off = X.va2off(va);
  const bytes = X.img.b.subarray(off, off + 8192);
  const dec = new Decoder(64, bytes, DecoderOptions.None);
  dec.ip = BigInt(va);
  const fmt = new Formatter(FormatterSyntax.Intel);
  const out = [];
  const ins = new Instruction();
  for (let i = 0; i < max && dec.canDecode; i++) {
    dec.decodeOut(ins);
    const s = fmt.format(ins);
    out.push([Number(ins.ip), s]);
    if (/^(ret|int3)/.test(s)) break;
    if (/^jmp /.test(s) && !/^jmp short|^jmp near ptr 0*14/i.test(s)) break;
  }
  ins.free(); fmt.free(); dec.free();
  return out;
}
module.exports = { dis };
if (require.main === module) for (const [ip, s] of dis(parseInt(process.argv[2], 16), +(process.argv[3] || 400))) console.log(ip.toString(16).toUpperCase(), s);
