// Global variables whose run-time invariant the differential harness reproduces in its rescue
// vectors (kinds 16 and up of the model list, see tools/native/harness.c and docs/l2_testing.md).
//
// The engine's object table is reached through one inlined idiom, compiled about 17,000 times:
//     mov eax,[obj+0Ch] / add eax,eax / sar eax,1     index stored in the object
//     cmp eax,[COUNT] / jge null                      32-bit element count
//     lea rcx,[rax+rax*2] / mov rax,[TABLE] / lea rdx,[rax+rcx*8]
//     null: xor edx,edx                               ... and [rdx+8] is read without a check
// Both globals are found from the code itself: COUNT is the operand of the compare that follows
// a "sar r32,1", TABLE the operand of the 64-bit load that follows that compare.
const pe = require('./pe');

const KIND_INDEX_BOUND = 16, KIND_POINTER = 17;

function findDataModels(img = pe.load()) {
  const b = img.b, t = img.sections.find((s) => s.name === '.text');
  const base = Number(img.imageBase);
  const counts = new Map(), tables = new Map();
  for (let o = t.rawPtr + 16; o < t.rawPtr + t.rawSize - 40; o++) {
    if (b[o] !== 0x3B || (b[o + 1] & 0xC7) !== 0x05) continue;            // cmp r32,[rip+disp32]
    let sar = false;
    for (let k = 2; k <= 16 && !sar; k++) if (b[o - k] === 0xD1 && (b[o - k + 1] & 0xF8) === 0xF8) sar = true; // sar r32,1
    if (!sar) continue;
    const va = base + t.rva + (o - t.rawPtr);
    const count = va + 6 + b.readInt32LE(o + 2);
    counts.set(count, (counts.get(count) || 0) + 1);
    for (let k = 6; k <= 30; k++) {                                         // mov r64,[rip+disp32]
      if ((b[o + k] & 0xFB) !== 0x48 || b[o + k + 1] !== 0x8B || (b[o + k + 2] & 0xC7) !== 0x05) continue;
      const key = count + ':' + (va + k + 7 + b.readInt32LE(o + k + 3));
      tables.set(key, (tables.get(key) || 0) + 1);
      break;
    }
  }
  const models = [], evidence = {};
  const hex = (v) => '0x' + v.toString(16).toUpperCase();
  for (const [count, n] of counts) {
    if (n < 1000) continue; // only an idiom the compiler inlined everywhere
    models.push({ addr: hex(count), kind: KIND_INDEX_BOUND });
    evidence[hex(count)] = `element count of a global table: compared with an object's index at ${n} sites (add r,r / sar r,1 / cmp r,[count])`;
    const best = [...tables].filter(([k]) => k.startsWith(count + ':')).sort((x, y) => y[1] - x[1])[0];
    if (best && best[1] * 2 > n) {
      const table = +best[0].split(':')[1];
      models.push({ addr: hex(table), kind: KIND_POINTER });
      evidence[hex(table)] = `pointer to that table: loaded right after the compare at ${best[1]} sites`;
    }
  }
  return { models, evidence };
}

module.exports = { findDataModels, KIND_INDEX_BOUND, KIND_POINTER };
