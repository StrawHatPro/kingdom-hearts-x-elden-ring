//! The generated combat tables (`tools/gen_port.js` from `sheets/port/sora-combat-*.json`,
//! registered in `tools/lib/port_gen_defs.js`).

use crate::enums::{ETresDamageAttribute, ETresDamageKind};

include!(concat!(env!("KH_GEN_DIR"), "/sora_combat_params.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_combat_attacks.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_combat_hits.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_combat_collisions.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_combat_attack_data.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_combat_rules.rs"));
// (party stream D6b) the team finishers' shot actors (sheets/port/sora-combat-shots.json)
include!(concat!(env!("KH_GEN_DIR"), "/sora_combat_shots.rs"));

/// The combat constants.
pub const P: SoraCombatParams = SORA_COMBAT_PARAMS;

/// The row of an attack kind (the value of Attack state +0x1B0).
pub fn attack_kind(kind: u8) -> Option<&'static AttackKindRow> {
    ATTACK_KINDS.iter().find(|row| row.kind == kind as i32)
}

/// A row of the attack-data table by its name, with its index.
pub fn attack_row(name: &str) -> Option<(usize, &'static AttackDataRow)> {
    ATTACK_DATA.iter().enumerate().find(|(_, row)| row.id.eq_ignore_ascii_case(name))
}

/// The shapes of a keyblade shape group (`w_so_KeybladeBase` `TresAtkColl0`).
pub fn keyblade_shapes(group: &str) -> impl Iterator<Item = &'static AtkCollisionRow> + '_ {
    ATK_COLLISIONS.iter().filter(move |row| row.owner == "w_so_KeybladeBase" && row.group == group)
}
include!(concat!(env!("KH_GEN_DIR"), "/sora_combat_reaction_rules.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_combat_form_commands.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_combat_form_abilities.rs"));

/// The row of a command kind in `TresCmdData` (the rows the sheet carries).
pub fn command_row(kind: i32) -> Option<&'static CommandRow> {
    COMMAND_ROWS.iter().find(|row| row.kind == kind)
}

/// The shapes of a shape group: the keyblade's (`w_so_KeybladeBase` `TresAtkColl0`), else the
/// pawn's own (`p_ex001_Pawn` `TresAtkColl0`: `StyleChange` ...). Both components receive every
/// activation (strand C 8.2.1).
pub fn group_shapes(group: &str) -> impl Iterator<Item = &'static AtkCollisionRow> + '_ {
    ATK_COLLISIONS.iter().filter(move |row| row.group == group)
}
