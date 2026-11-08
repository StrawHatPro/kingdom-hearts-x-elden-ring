//! (GOOFY) The kit's numbers: one row of `sheets/port/party-kit-goofy.json` per field (`node tools/gen_port.js`).

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_kit_goofy.rs"));

/// The kit's numbers (sheet `party-kit-goofy`).
pub const K: PartyKitGoofyParams = PARTY_KIT_GOOFY_PARAMS;
