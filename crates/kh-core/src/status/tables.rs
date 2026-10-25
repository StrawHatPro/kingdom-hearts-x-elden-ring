//! The status side's data, generated from the sheets by `node tools/gen_port.js` (registry
//! `tools/lib/port_gen_defs.js`): the level table, the ability catalogue, the difficulty rows,
//! the equipment rows and the rule constants of docs/sora-stats-spec.md. Plain statics: the core
//! reads no file. Lookups only; no rule lives here.

use crate::enums::ETresAbilityKind;

include!(concat!(env!("KH_GEN_DIR"), "/sora_levels.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_abilities.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_difficulty.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_equipment.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_stats_rules.rs"));
// (DF, 2026-10-15) KH3 Critical's start abilities (sheet sora-critical-start, tools/sora_critical_sheet.js)
include!(concat!(env!("KH_GEN_DIR"), "/sora_critical_start.rs"));

/// The rule constants (`sheets/port/sora-stats-rules`).
pub const RULES: StatsRules = STATS_RULES_PARAMS;

/// Highest level (`level.max`; the table has that many rows).
pub const LEVEL_MAX: u32 = 99;

/// `ETresGameLevelID` (GW+0x7DF0): the difficulty. Stats spec section 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(i32)]
pub enum GameLevel {
    Beginner = 0,
    #[default]
    Standard = 1,
    Proud = 2,
    Critical = 3,
}

impl GameLevel {
    pub const ALL: [GameLevel; 4] = [GameLevel::Beginner, GameLevel::Standard, GameLevel::Proud, GameLevel::Critical];

    /// By the sheet's name (BEGINNER, STANDARD, PROUD, CRITICAL), any case.
    pub fn from_name(name: &str) -> Option<GameLevel> {
        let name = name.trim().to_ascii_uppercase();
        GameLevel::ALL.into_iter().find(|g| g.row().id == name)
    }

    pub fn name(self) -> &'static str {
        self.row().id
    }

    /// Its row of `sheets/port/sora-difficulty` (row index = the enum value).
    pub fn row(self) -> &'static DifficultyRow {
        let row = &DIFFICULTY[self as usize];
        debug_assert_eq!(row.game_level, self as i32);
        row
    }

    pub fn is_critical(self) -> bool {
        self == GameLevel::Critical
    }
}

/// `LevelFromExp` 0x140582FD0 (stats spec 2.2): walk the rows 1..99, stop at the first whose
/// `m_Exp` is above the EXP; the level is the last row passed (at least 1).
pub fn level_from_exp(exp: i32) -> u32 {
    let mut level = 1;
    for (i, row) in LEVELS.iter().enumerate() {
        if row.exp_total > exp {
            break;
        }
        level = i as u32 + 1;
    }
    level
}

/// The row of a level (`ReadLevelRow` 0x1405832D0; the level is clamped into the table).
pub fn level_row(level: u32) -> &'static LevelRow {
    &LEVELS[(level.clamp(1, LEVEL_MAX) - 1) as usize]
}

/// The ability a level grants on an ability path (0 = `m_AbilityKind1`, 1 = Kind2, 2 = Kind3;
/// GW+0x7E0D), `None` for "-". (X1) `None` too for an ability the port excludes ([`is_excluded`]):
/// such a level grants nothing and its level-up panel has no ability line.
pub fn level_ability(level: u32, path: u8) -> Option<ETresAbilityKind> {
    let row = level_row(level);
    let name = match path {
        0 => row.ability_a,
        1 => row.ability_b,
        _ => row.ability_c,
    };
    ETresAbilityKind::from_game_name(name).filter(|&kind| !is_excluded(kind))
}

/// (X1, 2026-10-15) AN ABILITY THE PORT DOES NOT HAVE: column `port` = excluded of
/// `sheets/port/sora-abilities` (decision dec029, the user: "remove all attraction related abilities on
/// Sora, they are useless since we don't even have attractions": ATTRACTION_TIME 47, ATTRACTION_UP 62).
/// [`super::AbilitySet`] never holds one, so nothing grants, lists, equips or loads it.
pub fn is_excluded(kind: ETresAbilityKind) -> bool {
    static EXCLUDED: std::sync::OnceLock<[bool; super::ABILITY_SLOTS]> = std::sync::OnceLock::new(); // (XSI) 512 slots
    let table = EXCLUDED.get_or_init(|| {
        let mut out = [false; super::ABILITY_SLOTS];
        for row in ABILITIES.iter().filter(|row| row.port == "excluded") {
            if let Ok(slot) = usize::try_from(row.kind.value()) {
                if slot < out.len() {
                    out[slot] = true;
                }
            }
        }
        out
    });
    usize::try_from(kind.value()).ok().and_then(|slot| table.get(slot).copied()).unwrap_or(false)
}

/// Copies of `kind` granted by the levels `from + 1 ..= to` of a path.
pub fn level_grants(kind: ETresAbilityKind, from: u32, to: u32, path: u8) -> u8 {
    (from + 1..=to.min(LEVEL_MAX)).filter(|&l| level_ability(l, path) == Some(kind)).count() as u8
}

/// (DF, 2026-10-15) KH3 CRITICAL'S START ABILITIES: the victory-bonus rows flagged `m_InitAbilityEquipOnCriticalMode`
/// (sheet `sora-critical-start`, row order): (ability, equipped at the start). On Critical such a row gives only its stats
/// when it is won (0x140586280 case 1); the ability comes at the start [inferred: the grant code is not located]. A kind
/// that is not an `ETresAbilityKind` or that the port excludes is left out.
pub fn critical_start() -> Vec<(ETresAbilityKind, bool)> {
    // (dec032) only the victory-bonus rows the port keeps (the init rows are Status::from_loadout's)
    CRITICAL_START.iter().filter(|row| row.group == "vbonus" && row.port == "in").filter_map(|row| ETresAbilityKind::from_game_name(row.ability).filter(|k| !is_excluded(*k)).map(|k| (k, row.mode == "EQUIP"))).collect()
}

/// (DF) Whether `kind` is one of [`critical_start`]'s abilities (a reward of it gives nothing on Critical).
pub fn is_critical_start(kind: ETresAbilityKind) -> bool {
    CRITICAL_START.iter().any(|row| row.group == "vbonus" && row.port == "in" && ETresAbilityKind::from_game_name(row.ability) == Some(kind))
}

/// (DF, 2026-10-16; decision dec032, the user: "don't start with critical counter") Whether a KH3 Critical start ability
/// (a `sora-critical-start` row of either group) is EXCLUDED from the port's Critical start: neither owned nor equipped at a
/// new game, nor taken away when the difficulty changes (Sora gets it by other means). Critical Counter.
pub fn critical_start_excluded(kind: ETresAbilityKind) -> bool {
    CRITICAL_START.iter().any(|row| row.port == "excluded" && ETresAbilityKind::from_game_name(row.ability) == Some(kind))
}

/// The catalogue row of an ability (`sheets/port/sora-abilities`).
pub fn ability_row(kind: ETresAbilityKind) -> Option<&'static AbilityRow> {
    ABILITIES.iter().find(|row| row.kind == kind)
}

/// AP one equipped copy costs (`TresAbilityData.m_UseAP`); 0 for a kind without a data row.
pub fn ability_ap(kind: ETresAbilityKind) -> i32 {
    ability_row(kind).and_then(|row| row.ap).unwrap_or(0)
}

/// An equipment row by its id (`WEP_KEYBLADE_SO_00`, `WEP_KEYBLADE_SO_00.upgrade.4`, `PRT_ITEM01` ...).
pub fn equipment_row(id: &str) -> Option<&'static EquipmentRow> {
    EQUIPMENT.iter().find(|row| row.id == id)
}

/// The row of a keyblade at an upgrade level: 0 = the base row; 1..=10 = the sheet's
/// `<id>.upgrade.<n>` rows (cumulative totals; the sheet has them for the Kingdom Key). `None`
/// when the sheet has no such row.
pub fn weapon_row(id: &str, upgrade: u8) -> Option<&'static EquipmentRow> {
    if upgrade == 0 {
        return equipment_row(id);
    }
    let prefix_len = id.len();
    EQUIPMENT.iter().find(|row| row.kind == "keyblade-upgrade" && row.id.len() > prefix_len && row.id.starts_with(id) && row.id[prefix_len..].strip_prefix(".upgrade.").and_then(|n| n.parse::<u8>().ok()) == Some(upgrade))
}
