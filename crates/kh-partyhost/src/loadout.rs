//! (moved from kh-app loadout.rs) TEST-LEVEL CHOICE (sheet `party-test`, authored; plan open note N2): the friends'
//! loadout on top of the level-99 status kh-party builds, the spare staffs / shields of the camp menu's stock, where
//! they stand at start. The friends' weapon rows themselves are sheet `party-weapons` (stream B, TresEquipItemData).
//! The host reads the sheet files and hands their `rows` here (no files in this crate).

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartyLoadout {
    pub weapon: String,
    pub items: Vec<String>,
    /// Record bytes +0x158 .. +0x15B.
    pub ai: [u8; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartyTest {
    pub donald: PartyLoadout,
    pub goofy: PartyLoadout,
    pub stock_weapons: Vec<(String, u32)>,
    /// (length cm, angle deg from Sora's facing) for Donald, Goofy.
    pub offsets: [(f32, f32); 2],
    /// Sora's team abilities to equip, and the copies taken off to make the AP (row sora.team_abilities).
    pub sora_team: (Vec<String>, Vec<String>),
}

/// One weapon row of sheet `party-weapons`: (item key, owner npc, Strength, Magic, Defense, abilities, model).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartyWeapon {
    pub key: String,
    pub npc: String,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    pub abilities: Vec<String>,
    pub model: String,
}

impl PartyTest {
    /// Sheet `party-test`'s rows (`rows` of `sheets/port/party-test.json`).
    pub fn from_rows(rows: &[serde_json::Value]) -> PartyTest {
        let mut out = PartyTest { offsets: [(180.0, 70.0), (180.0, -70.0)], ..PartyTest::default() };
        for row in rows {
            let (id, value) = (row[0].as_str().unwrap_or(""), &row[1]);
            let loadout = |v: &serde_json::Value| PartyLoadout {
                weapon: v["weapon"].as_str().unwrap_or("").to_string(),
                items: v["items"].as_array().into_iter().flatten().filter_map(|i| i.as_str().map(str::to_string)).collect(),
                ai: {
                    let a: Vec<u8> = v["ai"].as_array().into_iter().flatten().filter_map(|x| x.as_u64().map(|x| x as u8)).collect();
                    [a.first().copied().unwrap_or(0), a.get(1).copied().unwrap_or(0), a.get(2).copied().unwrap_or(1), a.get(3).copied().unwrap_or(7)]
                },
            };
            match id {
                "donald.loadout" => out.donald = loadout(value),
                "goofy.loadout" => out.goofy = loadout(value),
                "stock.weapons" => out.stock_weapons = value.as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(1) as u32)).collect(),
                "sora.team_abilities" => {
                    let list = |k: &str| value[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>();
                    out.sora_team = (list("equip"), list("make_room"));
                }
                "spawn.offsets" => {
                    for (i, who) in ["donald", "goofy"].into_iter().enumerate() {
                        if let (Some(l), Some(a)) = (value[who][0].as_f64(), value[who][1].as_f64()) {
                            out.offsets[i] = (l as f32, a as f32);
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
}

impl PartyWeapon {
    /// Sheet `party-weapons`'s rows.
    pub fn from_rows(rows: &[serde_json::Value]) -> Vec<PartyWeapon> {
        rows.iter()
            .map(|r| PartyWeapon {
                key: r[1].as_str().unwrap_or("").to_string(),
                npc: r[2].as_str().unwrap_or("").to_string(),
                strength: r[4].as_i64().unwrap_or(0) as i32,
                magic: r[5].as_i64().unwrap_or(0) as i32,
                defense: r[6].as_i64().unwrap_or(0) as i32,
                abilities: r[7].as_str().unwrap_or("").split([',', ' ']).filter(|s| !s.is_empty()).map(str::to_string).collect(),
                model: r[8].as_str().unwrap_or("").to_string(),
            })
            .collect()
    }
}
