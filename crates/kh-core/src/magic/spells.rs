//! The spells as data: the units of `p_ex001_MagicSet` (magic spec A 3.1), the command rows'
//! MP costs (C 2), and the attack rows the spell actors name (I 6.2). Everything is read from
//! the generated sheet tables (`tables.rs`); the only authored thing here is the list of which
//! command id is which sheet key (checked against the sheet's own `cmd` rows by a test).

use super::tables::Sheet;
use crate::enums::{ETresDamageAttribute, ETresDamageKind};
use crate::status::runtime::MpCost;

/// `ETresCommandKind` values this side uses (row ids of `TresCmdData`).
pub mod cmd {
    pub const ATTACK: u8 = 27;
    pub const FIRE: u8 = 29;
    pub const FIRA: u8 = 30;
    pub const FIRAGA: u8 = 31;
    pub const BLIZZARD: u8 = 33;
    pub const BLIZZARA: u8 = 34;
    pub const BLIZZAGA: u8 = 35;
    pub const THUNDER: u8 = 37;
    pub const THUNDARA: u8 = 38;
    pub const THUNDAGA: u8 = 39;
    pub const WATER: u8 = 41;
    pub const WATARA: u8 = 42;
    pub const WATAGA: u8 = 43;
    pub const AERO: u8 = 45;
    pub const AERORA: u8 = 46;
    pub const AEROGA: u8 = 47;
    pub const CURE: u8 = 49;
    pub const CURA: u8 = 50;
    pub const CURAGA: u8 = 51;
    pub const POTION: u8 = 58;
    pub const HIPOTION: u8 = 59;
    pub const MEGAPOTION: u8 = 60;
    pub const ETHER: u8 = 61;
    pub const HIETHER: u8 = 62;
    pub const MEGAETHER: u8 = 63;
    pub const ELIXIR: u8 = 64;
    pub const MEGALIXIR: u8 = 65;
    pub const REFOCUSER: u8 = 66;
    pub const HIREFOCUSER: u8 = 67;
    pub const PANACEA: u8 = 68;
    pub const S_FIRA: u8 = 110;
    pub const S_FIRAGA: u8 = 111;
    pub const S_FIRAGAN: u8 = 112;
    pub const S_BLIZZARA: u8 = 113;
    pub const S_THUNDARA: u8 = 116;
    pub const S_WATARA: u8 = 119;
    pub const S_AERORA: u8 = 122;
    pub const S_CURAGAN: u8 = 125;
    /// `TRES_CMD_KIND_SHOOTFLOW` (0xFD): the command id of every shotlock.
    pub const SHOOTFLOW: u8 = 0xFD;
}

/// The element of a spell = the `Attr` column of its command row (part I 7.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Element {
    Fire = 1,
    Blizzard = 2,
    Thunder = 3,
    Water = 4,
    Aero = 5,
    Cure = 7,
}

impl Element {
    pub const ALL: [Element; 6] = [Element::Fire, Element::Blizzard, Element::Thunder, Element::Water, Element::Aero, Element::Cure];

    /// The key of the loadout's `magic.known` map and of the `cmd.<name>` rows of tier 1.
    pub const fn name(self) -> &'static str {
        match self {
            Element::Fire => "fire",
            Element::Blizzard => "blizzard",
            Element::Thunder => "thunder",
            Element::Water => "water",
            Element::Aero => "aero",
            Element::Cure => "cure",
        }
    }

    pub fn from_name(name: &str) -> Option<Element> {
        Element::ALL.into_iter().find(|e| e.name().eq_ignore_ascii_case(name))
    }

    /// The command of this element's tier 1..=3 (Fire 29, Fira 30, Firaga 31 ...).
    pub fn command(self, tier: u8) -> u8 {
        let base = match self {
            Element::Fire => cmd::FIRE,
            Element::Blizzard => cmd::BLIZZARD,
            Element::Thunder => cmd::THUNDER,
            Element::Water => cmd::WATER,
            Element::Aero => cmd::AERO,
            Element::Cure => cmd::CURE,
        };
        base + tier.clamp(1, 3) - 1
    }
}

/// One unit of the magic set (`TresPlayerMagicAssetUnit`), plus what the port derives from the
/// command id.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpellUnit {
    /// `m_Command`.
    pub cmd: u8,
    /// The unit's key in `cast.magicset.main.<key>` ("fira", "s_firagan").
    pub key: &'static str,
    /// The key of its first actor in the projectile sheet ("fira", "figa", "figan").
    pub proj: &'static str,
    pub element: Element,
    /// 1..=3; 4 = the "-gan" class.
    pub tier: u8,
    /// A grand-magic situation command (`S_*`).
    pub grand: bool,
    /// `m_bIsCastType`, `m_bIsDirectPos`.
    pub cast_type: bool,
    pub direct_pos: bool,
    /// `m_EffectGroup` (1301 ...), `m_VoiceGroup`, `m_VoiceGroupFinish`.
    pub effect_group: i32,
    pub voice: i32,
    pub voice_finish: i32,
    /// The `Mp` column of the command row (255 = all).
    pub mp: MpCost,
}

/// (command, magic-set key, projectile key, element, tier, grand).
const UNITS: &[(u8, &str, &str, Element, u8, bool)] = &[
    (29, "fire", "fire", Element::Fire, 1, false),
    (30, "fira", "fira", Element::Fire, 2, false),
    (31, "firaga", "figa", Element::Fire, 3, false),
    (110, "s_fira", "fira", Element::Fire, 2, true),
    (111, "s_firaga", "figa", Element::Fire, 3, true),
    (112, "s_firagan", "figan", Element::Fire, 4, true),
    (33, "blizzard", "blizzard", Element::Blizzard, 1, false),
    (34, "blizzara", "blizzara", Element::Blizzard, 2, false),
    (35, "blizzaga", "blizzaga", Element::Blizzard, 3, false),
    (113, "s_blizzara", "blizzara", Element::Blizzard, 2, true),
    (114, "s_blizzaga", "blizzaga", Element::Blizzard, 3, true),
    (115, "s_blizzagan", "blizzagan", Element::Blizzard, 4, true),
    (37, "thunder", "thunder", Element::Thunder, 1, false),
    (38, "thundara", "thundara", Element::Thunder, 2, false),
    (39, "thundaga", "thundaga", Element::Thunder, 3, false),
    (116, "s_thundara", "thundara", Element::Thunder, 2, true),
    (117, "s_thundaga", "thundaga", Element::Thunder, 3, true),
    (118, "s_thundagan", "thundagan", Element::Thunder, 4, true),
    (41, "water", "water", Element::Water, 1, false),
    (42, "watara", "watara", Element::Water, 2, false),
    (43, "wataga", "wataga", Element::Water, 3, false),
    (119, "s_watara", "watara", Element::Water, 2, true),
    (120, "s_wataga", "wataga", Element::Water, 3, true),
    (121, "s_watagan", "watagan", Element::Water, 4, true),
    (45, "aero", "aero", Element::Aero, 1, false),
    (46, "aerora", "aerora", Element::Aero, 2, false),
    (47, "aeroga", "aeroga", Element::Aero, 3, false),
    (122, "s_aerora", "aerora", Element::Aero, 2, true),
    (123, "s_aeroga", "aeroga", Element::Aero, 3, true),
    (124, "s_aerogan", "aerogan", Element::Aero, 4, true),
    (49, "cure", "cure", Element::Cure, 1, false),
    (50, "cura", "cura", Element::Cure, 2, false),
    (51, "curaga", "curaga", Element::Cure, 3, false),
    (125, "s_curagan", "curagan", Element::Cure, 4, true),
];

/// Every command id that has a unit in the Kingdom Key's magic set (the underwater ones left
/// out: no water zone exists in the port).
pub fn unit_commands() -> impl Iterator<Item = u8> {
    UNITS.iter().map(|u| u.0)
}

/// The unit of a command; `None` for a command the magic set has no unit for (the plain "-gan"
/// ids 32, 36, 40, 44, 48: A 3.1).
pub fn unit(cmd: u8) -> Option<SpellUnit> {
    let &(cmd, key, proj, element, tier, grand) = UNITS.iter().find(|u| u.0 == cmd)?;
    let id = |property: &str| format!("cast.magicset.main.{key}.{property}");
    // The cost: the `Mp` column (menu2.cmddata.magic.<id>.mp: 255 = all); grand rows have none.
    let mp = if grand { MpCost::Points(0) } else { MpCost::from_row(Sheet::Items.num_or(&format!("menu2.cmddata.magic.{cmd}.mp"), 0.0) as u8) };
    Some(SpellUnit {
        cmd,
        key,
        proj,
        element,
        tier,
        grand,
        cast_type: Sheet::Spells.flag(&id("cast_type")),
        direct_pos: Sheet::Spells.flag(&id("direct_pos")),
        effect_group: Sheet::Spells.int(&id("effect_group")),
        voice: Sheet::Spells.int(&id("voice_group")),
        voice_finish: Sheet::Spells.int(&id("voice_group_finish")),
        mp,
    })
}

/// A magic command of the normal list (29..31, 33..35, 37..39, 41..43, 45..47, 49..51).
pub fn is_magic(cmd: u8) -> bool {
    matches!(cmd, 29..=31 | 33..=35 | 37..=39 | 41..=43 | 45..=47 | 49..=51)
}

/// A grand-magic command (32, 36, 40, 44, 48, 110..=126, 231: A 2.2).
pub fn is_grand(cmd: u8) -> bool {
    matches!(cmd, 32 | 36 | 40 | 44 | 48 | 110..=126 | 231)
}

/// The Cure family (49..51, 125).
pub fn is_cure(cmd: u8) -> bool {
    matches!(cmd, 49..=51 | 125)
}

/// The Fire family that chains at once (29..31: A 1.3).
pub fn is_fire_chain(cmd: u8) -> bool {
    matches!(cmd, 29..=31)
}

/// An item command (rows of category ITEM, 58..=68).
pub fn is_item(cmd: u8) -> bool {
    matches!(cmd, 58..=68)
}

/// The display name of a command for the menu model (the text tables are the HUD's: this is
/// the English name of the game's `LocKey` text, for the debug HUD and logs).
pub fn command_name(cmd: u8) -> &'static str {
    match cmd {
        27 => "Attack",
        29 => "Fire",
        30 => "Fira",
        31 => "Firaga",
        33 => "Blizzard",
        34 => "Blizzara",
        35 => "Blizzaga",
        37 => "Thunder",
        38 => "Thundara",
        39 => "Thundaga",
        41 => "Water",
        42 => "Watera",
        43 => "Waterga",
        45 => "Aero",
        46 => "Aerora",
        47 => "Aeroga",
        49 => "Cure",
        50 => "Cura",
        51 => "Curaga",
        58 => "Potion",
        59 => "Hi-Potion",
        60 => "Mega-Potion",
        61 => "Ether",
        62 => "Hi-Ether",
        63 => "Mega-Ether",
        64 => "Elixir",
        65 => "Megalixir",
        66 => "Refocuser",
        67 => "Hi-Refocuser",
        68 => "Panacea",
        110 => "Fira",
        111 => "Firaga",
        112 => "Firaza",
        113 => "Blizzara",
        114 => "Blizzaga",
        115 => "Blizzaza",
        116 => "Thundara",
        117 => "Thundaga",
        118 => "Thundaza",
        119 => "Watera",
        120 => "Waterga",
        121 => "Waterza",
        122 => "Aerora",
        123 => "Aeroga",
        124 => "Aeroza",
        125 => "Curaza",
        _ => "",
    }
}

/// A command by a name: the display name, the game's enum tail ("FIRAGA", "WATAGA", "HIPOTION")
/// or its number.
pub fn command_by_name(name: &str) -> Option<u8> {
    let key: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).flat_map(char::to_lowercase).collect();
    if let Ok(n) = key.parse::<u8>() {
        return Some(n);
    }
    let by_game = crate::enums::ETresCommandKind::from_game_name(&format!("TRES_CMD_KIND_{}", name.trim().to_ascii_uppercase())).map(|k| k.value() as u8);
    by_game.or_else(|| {
        (27u8..=68).find(|&c| {
            let display: String = command_name(c).chars().filter(|ch| ch.is_ascii_alphanumeric()).flat_map(char::to_lowercase).collect();
            !display.is_empty() && display == key
        })
    })
}

/// One row of `p_ex001_AttackData` as a spell actor or shotlock projectile names it: the fields
/// a hit carries (magic spec B 4, C 4, I 6.2, G 3.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackRow {
    /// The row's name as the actors give it ("MAG_FIRA_FIN").
    pub name: &'static str,
    pub power: f32,
    pub kind: ETresDamageKind,
    pub attribute: ETresDamageAttribute,
    pub power_scale: f32,
    pub parabola_angle: f32,
    /// `m_DamageEffectTime` (the SWIRL_UP hold).
    pub effect_time: f32,
    /// `m_BadStatusKind` without its prefix ("NONE", "FREEZE", "DISCHARGE"), chance in percent, seconds.
    pub bad_status: &'static str,
    pub bad_rate: f32,
    pub bad_time: f32,
    pub reaction_power: i32,
    pub armor_attack: i32,
    pub form_point: i32,
    pub revenge_point: f32,
    pub magic: bool,
    pub invalid_guard: bool,
    pub rapid_fire: bool,
    pub killer: bool,
    pub attraction: bool,
    /// `m_PhysForceLv` ("LV_1").
    pub phys_force: &'static str,
}

fn strip(value: &'static str, prefix: &str) -> &'static str {
    value.strip_prefix(prefix).unwrap_or(value)
}

fn damage_kind(name: &str) -> ETresDamageKind {
    let full = if name.starts_with("TRES_DMG_KIND_") { name.to_string() } else { format!("TRES_DMG_KIND_{name}") };
    ETresDamageKind::from_game_name(&full).unwrap_or(ETresDamageKind::Small)
}

fn attribute(name: &str) -> ETresDamageAttribute {
    let full = if name.starts_with("TRES_DMG_ATTR_") { name.to_string() } else { format!("TRES_DMG_ATTR_{name}") };
    ETresDamageAttribute::from_game_name(&full).unwrap_or(ETresDamageAttribute::Physical)
}

/// An attack row by name: the `MAG_*` rows (sheet `sora-magic-mp`, `atk.<row>.*`), the two
/// Kingdom Key shotlock rows (`sora-magic-shotlocks`, `kc2.attack.<row>.*`) and the airstep
/// kick (`airstep2.row.ff_athletic_kick.*`). `None`: no such row (the static default row then
/// applies: power 1, nothing else).
pub fn attack_row(name: &'static str) -> Option<AttackRow> {
    let lower = name.to_ascii_lowercase();
    if Sheet::Mp.find(&format!("atk.{lower}.power")).is_some() {
        let id = |property: &str| format!("atk.{lower}.{property}");
        let flags = Sheet::Mp.text_or(&id("flags"), "");
        let has = |flag: &str| flags.split('|').any(|f| f == flag);
        return Some(AttackRow {
            name,
            power: Sheet::Mp.num(&id("power")),
            kind: damage_kind(Sheet::Mp.text(&id("damagekind"))),
            attribute: attribute(Sheet::Mp.text(&id("damageattribute"))),
            power_scale: Sheet::Mp.num(&id("damagepowerscale")),
            parabola_angle: Sheet::Mp.num(&id("damageparabolaangle")),
            effect_time: Sheet::Mp.num(&id("damageeffecttime")),
            bad_status: strip(Sheet::Mp.text(&id("badstatuskind")), "TRES_BADSTAT_"),
            bad_rate: Sheet::Mp.num(&id("badstatusgenrate")),
            bad_time: Sheet::Mp.num(&id("badstatuseffecttime")),
            reaction_power: Sheet::Mp.int(&id("reactionpowervalue")),
            armor_attack: Sheet::Mp.int(&id("armorattackpower")),
            form_point: Sheet::Mp.int(&id("formpoint")),
            revenge_point: Sheet::Mp.num(&id("revengepoint")),
            magic: has("MagicAttack"),
            invalid_guard: has("IvalidGuard"),
            rapid_fire: has("RapidFire"),
            killer: has("KillerAttack"),
            attraction: has("AttractionDamage"),
            phys_force: strip(Sheet::Mp.text_or(&id("physforcelv"), "TPDF_LV_1"), "TPDF_"),
        });
    }
    for group in ["kc2.attack", "airstep2.row"] {
        if Sheet::Shot.find(&format!("{group}.{lower}.power")).is_some() {
            let id = |property: &str| format!("{group}.{lower}.{property}");
            return Some(AttackRow {
                name,
                power: Sheet::Shot.num(&id("power")),
                kind: damage_kind(Sheet::Shot.text(&id("damage_kind"))),
                attribute: attribute(Sheet::Shot.text_or(&id("damage_attribute"), "TRES_DMG_ATTR_PHYSICAL")),
                power_scale: Sheet::Shot.num_or(&id("power_scale"), 1.0),
                parabola_angle: 0.0,
                effect_time: 0.0,
                bad_status: "NONE",
                bad_rate: 0.0,
                bad_time: 0.0,
                reaction_power: Sheet::Shot.num_or(&id("reaction_power_value"), 0.0) as i32,
                armor_attack: Sheet::Shot.num_or(&id("armor_attack_power"), 0.0) as i32,
                form_point: Sheet::Shot.num_or(&id("form_point"), 0.0) as i32,
                revenge_point: Sheet::Shot.num_or(&id("revenge_point"), 0.0),
                magic: Sheet::Shot.flag_or(&id("is_magic_attack"), false),
                invalid_guard: false,
                rapid_fire: Sheet::Shot.flag_or(&id("is_rapid_fire"), false),
                killer: Sheet::Shot.flag_or(&id("is_killer_attack"), false),
                attraction: false,
                phys_force: strip(Sheet::Shot.text_or(&id("phys_force_lv"), "TPDF_LV_1"), "TPDF_"),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unit_list_agrees_with_the_sheet() {
        for id in unit_commands() {
            let u = unit(id).unwrap();
            assert_eq!(Sheet::Spells.int(&format!("cast.magicset.main.{}.cmd", u.key)), id as i32, "{}", u.key);
            let class = Sheet::Spells.text(&format!("cast.magicset.main.{}.projectile", u.key));
            let first = if u.element == Element::Water { format!("{}.barrier_class", u.proj) } else { format!("{}.class", u.proj) };
            assert!(class.ends_with(Sheet::Proj.text(&first)), "{}: {class} vs {first}", u.key);
        }
        // Costs (C 2): Fire 10 / 14 / 20, Blizzard 12 / 17 / 24, Thunder 14 / 20 / 28, Water 11 / 15 / 22,
        // Aero 13 / 18 / 26, Cure all.
        let cost = |c: u8| unit(c).unwrap().mp;
        assert_eq!([cost(29), cost(30), cost(31)], [MpCost::Points(10), MpCost::Points(14), MpCost::Points(20)]);
        assert_eq!([cost(33), cost(39), cost(43), cost(47)], [MpCost::Points(12), MpCost::Points(28), MpCost::Points(22), MpCost::Points(26)]);
        assert_eq!([cost(49), cost(51), cost(110)], [MpCost::All, MpCost::All, MpCost::Points(0)]);
        assert!(unit(32).is_none());
    }

    #[test]
    fn attack_rows() {
        let fire = attack_row("MAG_FIRE").unwrap();
        assert_eq!((fire.power, fire.kind, fire.attribute, fire.form_point, fire.magic), (1.0, ETresDamageKind::Small, ETresDamageAttribute::Fire, 15, true));
        let fin = attack_row("MAG_FIRE_FIN").unwrap();
        assert_eq!((fin.power, fin.kind, fin.form_point), (1.2, ETresDamageKind::Parabola, 18));
        let blizzard = attack_row("MAG_BLIZZARD").unwrap();
        assert_eq!((blizzard.bad_status, blizzard.bad_rate, blizzard.bad_time), ("FREEZE", 100.0, 5.0));
        let kc = attack_row("SF_KCSHOT").unwrap();
        assert_eq!((kc.power, kc.magic, kc.rapid_fire, kc.attribute), (0.8, true, true, ETresDamageAttribute::Notype));
        assert_eq!(attack_row("SF_KCRAGNAROK").unwrap().kind, ETresDamageKind::Toss);
        assert!(attack_row("NO_SUCH_ROW").is_none());
        assert_eq!((command_by_name("FIRAGA"), command_by_name("Hi-Potion"), command_by_name("61")), (Some(31), Some(59), Some(61)));
    }
}
