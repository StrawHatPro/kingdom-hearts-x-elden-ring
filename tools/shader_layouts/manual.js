const { W, ROOT_SLASH } = require('./work');
// Hand-read serializers (function VA : calling form) -> ops [kind, member offset, extra]
// 'call' descends into another function with the automatic reader.
const seq = (kind, base, n, step) => Array.from({ length: n }, (_, i) => [kind, base + i * step]);
// Parameter-list handlers registered by func_14360BBE0 into the table at 149D0C090 (index = registration order);
// each entry is [vtable, Serialize (vtable slot 6), ops]. Read from the handlers' decompiled Serialize.
const HANDLERS = [
  ['1473A6640', '1436009F0', [['param', 0]]],
  ['1473A8A40', '143600B00', seq('res', 0, 4, 4)],
  ['1473A8DA8', '143600B80', [['param', 0], ['param', 6], ...seq('res', 12, 11, 4)]],
  ['1473A8678', '143600A00', [...seq('res', 0, 4, 4), ...seq('param', 16, 11, 6), ...seq('res', 82, 4, 4)]],
  ['1473A5B78', '143600960', seq('res', 0, 2, 4)],
  ['1473A5C50', '1436009F0', [['param', 0]]],
  ['1473A8C28', '143600B50', [['ub', 0], ['ub', 8]]], // built by func_14360BF90; two uniform-buffer parameters (func_1436108F0)
  ['1473A9948', '143600C40', [['res', 0], ['res', 8], ['res', 4], ['res', 12]]],
  ['1473A6490', '1436009A0', seq('param', 0, 4, 6)],
  ['1473A5A08', '143600900', [['param', 0], ...seq('res', 6, 4, 4)]],
];
module.exports = {
  HANDLERS,
  // FMaterialShader::Serialize
  '1435B4200:method': [
    ['ub', 0xc0], ['ubArr', 0xc8], ['call', 0x118, { f: '14350BC20', mode: 'op' }], ['res', 0x224], ['res', 0x228], ['dbgSet', 0x230],
    ['name', 0x260], ['u32', 0x248], ['u32', 0x24c], ['bytes', 0x250], ['fstr', 0x270], ['res', 0x22c],
    ['paramArr', 0xd8], ['paramArr', 0xe8], ['paramArr', 0xf8], ['paramArr', 0x108], ['typedList', 0x280],
  ],
  // FLocalVertexFactory shader parameters: FShaderParameter at +10, then a bool at +8
  '1441E5260:method': [['param', 10], ['b32', 8]],
  // adjustor thunk: mov rax,[rcx+8]; add rcx,8; jmp [rax+8] -> Serialize of the sub-object at +8 (vtable 147744E90, slot 1)
  '14302C5D0:method': [['call', 8, { f: '1443BAD90', mode: 'method' }]],
  // Scaleform vertex shader: 28 FShaderParameter from +200, then four uint32 from +0x170
  '1438BB980:method': [...seq('param', 200, 28, 6), ...seq('u32', 0x170, 4, 4)],
};
