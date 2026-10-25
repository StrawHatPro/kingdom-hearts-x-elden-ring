// Where generated code goes, and how it is marked (doctrine D6 of the Spreadsheet Method).
// Generated files are disposable build output: they live under build/gen/, never under src/,
// are never edited, and `node tools/gen.js` recreates all of them from the sheets. Every file
// starts with banner(): the sheets its rows come from, the emitter and the emitter version.
// EMITTER_VERSION changes whenever the emitted text changes; sheets/doctrine.json records it and
// tools/preflight.js compares the two.
const EMITTER_VERSION = '2.0.0';
const GEN_ROOT = 'build/gen';
const GEN_INCLUDE = `${GEN_ROOT}/include`;            // the /I directory
const GEN = `${GEN_INCLUDE}/generated`;               // #include "generated/reflection.h"
const BANNER_MARK = '// GENERATED from sheets/';      // first bytes of every generated file
const banner = (sheets) => `${BANNER_MARK}${sheets} by tools/gen.js (emitter ${EMITTER_VERSION}) - DO NOT EDIT: change the sheet and run node tools/gen.js\n`;
const REFLECT_BANNER = banner('reflect: classes, structs, properties, property-types');
const DATA_BANNER = banner('data: structs, fields');
module.exports = { EMITTER_VERSION, GEN_ROOT, GEN_INCLUDE, GEN, BANNER_MARK, banner, REFLECT_BANNER, DATA_BANNER };
