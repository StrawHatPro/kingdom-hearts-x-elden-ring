//! The game's reflected enums that the port uses, with the game's own values: generated from
//! `sheets/reflect/enum-values` by `node tools/gen_port.js` into `build/gen/rust/tres_enums.rs`.
//! Which enums are generated is the `enums` list of `tools/lib/port_gen_defs.js`: add a name
//! there when code needs it. Each has `ALL`, `name()` (the Rust variant), `game_name()` (the
//! game's), `value()`, `from_value()`, `from_game_name()`.
//! Never edit the generated file.

include!(concat!(env!("KH_GEN_DIR"), "/tres_enums.rs"));
