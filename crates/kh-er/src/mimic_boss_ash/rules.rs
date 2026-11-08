//! (MIMIC-ASH) The pure rules of a whole spirit ash, as Elden Ring's own summon routine makes it (static RE of 2.7.2.0 /
//! 2.7.1.0: build/sora_tmp/party/SUMMON/IMPL_MIMIC_ASH_RE.md). No game types: unit-tested offline.
//!
//! What the game does, in the order it does it (2.7.2.0 RVAs; 2.7.1.0 in the RE note):
//! - THE ROWS: the trigger map (built at 0x4bd040 from every BuddyParam row, keyed by the row's `triggerSpEffectId`, rows
//!   chained in param order) gives ALL rows of the ash's trigger (`0x4bcf60`); the generator (0x4bbeb0) places one spirit per
//!   row (its loop 0x4bc070..0x4bc3f8) and creates one character per row, in that order, in the same frame (CreateBuddy
//!   0x4baf80, its loop 0x4bc490..0x4bc822). There is no other count: the number of spirits IS the number of rows (Lone Wolf
//!   Ashes, goods 232000: rows 23200000..23200002 = its three wolves).
//! - THE PLACE of a row: `origin + R_y(base_yaw) * (-x_offset, 0, -z_offset)` (0x4bc17e..0x4bc293: sinf / cosf of the
//!   manager's yaw +0xb0 as the rotation's X and Z columns, the offsets negated with the sign mask 0x32a49f0, the origin the
//!   manager's +0xa0); the row's facing is `base_yaw + y_angle * pi / 180` (0x4bc0ef..0x4bc0fe, 0x32a3bac = pi / 180).
//! - THE LEVEL: `request % 100` (0x4bbfc2..0x4bbfda); its doping `dopingSpEffect_lv{level}`, none above +10 (0xd295c0).
//! - THE ARRIVAL: `generateAnimId` of the row (CreateBuddy 0x4bb7c9 -> the group entry; played when the character is ready,
//!   0x4be170).
//! - THE TEAM of a spirit whose owner is another player (the Colosseum's opponent spirits): the owner's effective team
//!   (0x3f1c90) adopted unless the spirit's own team is 5 (0x3f7800), CreateBuddy 0x4bb63a..0x4bb6c6.
//! - THE ARENA MARKS (Update 0x4b8c86..0x4b8e92, every frame, every spirit of every group): `arenaHostileSpEffect_1..5` while
//!   the local player's team is hostile to the spirit's (0x51b870), `arenaFriendlySpEffect_1..5` while friendly (0x51b9e0),
//!   each only when the spirit does not have it yet. `arenaDopingSpEffect_1..5` (0x4be1bf..0x4be1f0) go on every spirit of
//!   both sides once it is ready: the Colosseum's balance (1410 sight, 1420 max HP x0.5 and damage taken x2), not a
//!   hostility rule.

use glam::{Quat, Vec3};

/// The generator's marker row of the Mimic Tear (0x4bc464: `cmp dword [rax+50h], 13BDB60h` = BuddyParam 20700000): the
/// player's equipment is copied onto the spirits of that ash (0x654ac0).
pub const MIMIC_MARKER_ROW: u32 = 20_700_000;

/// The highest level with a doping row (`dopingSpEffect_lv10`; 0xd295c0 gives -1 above it).
pub const MAX_LEVEL: u8 = 10;

/// FindSpawnPointAroundPlayer (0x4b9f50): its first candidate is this far from the summoner (0x32a3c08 = 3.0, loaded at
/// 0x4ba17e), taken when nothing is in the way.
pub const SPAWN_AHEAD: f32 = 3.0;

/// The team type CreateBuddy never replaces (0x3fa1e0 writes 5; 0x3f7800 keeps the spirit's team when it equals it).
pub const KEPT_TEAM: u8 = 5;

/// THE COLOSSEUM'S BALANCE: `arenaDopingSpEffect_*` (1410: sight, 1420: max HP x0.5, damage taken x2) is applied by the game
/// to every spirit of both sides while `IsArena` (0x4b77f0), never outside the Colosseum and never as a hostility rule. A
/// spirit the player summons in the Mimic Tear's arena does not get it, so the boss's does not either. (Flip to apply it.)
pub const APPLY_ARENA_DOPING: bool = false;

/// One BuddyParam row of the ash (the fields the game's summon reads).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AshRow {
    pub id: u32,
    pub npc_param: i32,
    pub think: i32,
    /// `npcPlayerInitParamId` (> 0: a spirit on the player's model, c0000).
    pub init: i32,
    pub x_offset: f32,
    pub z_offset: f32,
    /// Degrees.
    pub y_angle: f32,
    pub generate_anim: i32,
    pub doping: [i32; 11],
    pub arena_doping: [i32; 5],
    pub arena_friendly: [i32; 5],
    pub arena_hostile: [i32; 5],
}

#[cfg(test)]
impl AshRow {
    /// A row with no SpEffects (the tests').
    pub fn plain(id: u32, npc_param: i32, x_offset: f32, z_offset: f32, y_angle: f32) -> AshRow {
        AshRow { id, npc_param, think: npc_param, init: -1, x_offset, z_offset, y_angle, generate_anim: -1, doping: [-1; 11], arena_doping: [-1; 5], arena_friendly: [-1; 5], arena_hostile: [-1; 5] }
    }
}

/// One spirit of the ash, placed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub row: AshRow,
    /// The spirit's place (the game's physics space, the space of `origin`).
    pub at: Vec3,
    /// Its facing: the rotation about +Y (radians) of its orientation.
    pub yaw: f32,
}

/// The rows of a trigger, in the game's order (the param's own order: ascending row id).
pub fn rows_of_trigger(all: &[(i32, AshRow)], trigger: i32) -> Vec<AshRow> {
    let mut rows: Vec<AshRow> = all.iter().filter(|(t, _)| *t == trigger).map(|(_, r)| *r).collect();
    rows.sort_by_key(|r| r.id);
    rows
}

/// The ash is the Mimic Tear (the generator's own marker row is among its rows).
pub fn is_mimic(rows: &[AshRow]) -> bool {
    rows.iter().any(|r| r.id == MIMIC_MARKER_ROW)
}

/// The level doping of a row (none above +10, none when the row has none).
pub fn level_doping(row: &AshRow, level: u8) -> Option<i32> {
    if level > MAX_LEVEL {
        return None;
    }
    let id = row.doping[usize::from(level)];
    (id >= 0).then_some(id)
}

/// The character id the debug creator is asked for: c0000 for a spirit on the player's model, else the NpcParam id's
/// four chr digits (the Spirit Ash NpcParam ids carry a leading 1: 140700000 = c4070), the rule of allybody's `chr_of`.
pub fn chr_of(npc_param: i32, init: i32) -> i32 {
    if init > 0 {
        0
    } else {
        (npc_param / 10_000) % 10_000
    }
}

/// The summoner's facing direction for a yaw (a character model faces -Z: coords.rs `MODEL_FORWARD`).
pub fn forward_of(yaw: f32) -> Vec3 {
    Quat::from_rotation_y(yaw) * Vec3::NEG_Z
}

/// The spawn origin of the game's first candidate: `SPAWN_AHEAD` before the summoner, at its height. (The game then casts
/// for walls and the ground there; the direction of its candidate 0 is the summoner's model forward: see the RE note.)
pub fn origin_ahead(summoner: Vec3, yaw: f32) -> Vec3 {
    summoner + forward_of(yaw) * SPAWN_AHEAD
}

/// THE GENERATOR'S PLACE of one row around the spawn origin and the spawn yaw.
pub fn place(origin: Vec3, base_yaw: f32, row: &AshRow) -> Placed {
    let (s, c) = base_yaw.sin_cos();
    let (vx, vz) = (-row.x_offset, -row.z_offset);
    // columns: X = (cos, 0, -sin), Z = (sin, 0, cos) (0x4bc1af / 0x4bc1b6 and the 1, 1, -1, 1 mask 0x327e9d0)
    let at = origin + Vec3::new(vx * c + vz * s, 0.0, -vx * s + vz * c);
    Placed { row: *row, at, yaw: base_yaw + row.y_angle.to_radians() }
}

/// Every row of the ash placed, in the game's order.
pub fn place_all(origin: Vec3, base_yaw: f32, rows: &[AshRow]) -> Vec<Placed> {
    rows.iter().map(|r| place(origin, base_yaw, r)).collect()
}

/// 0x3f7800's rule: the owner's team, unless the spirit's own is the kept one.
pub fn adopt_team(spirit_team: u8, owner_team: u8) -> u8 {
    if spirit_team == KEPT_TEAM {
        spirit_team
    } else {
        owner_team
    }
}

/// The arena SpEffects the game would put on the spirit now: the hostile ones while hostile to the local player, the
/// friendly ones while friendly, the doping ones once (only with `APPLY_ARENA_DOPING`); `has` = already on it. In the
/// game's order (hostile 1..5, friendly 1..5).
pub fn arena_marks(row: &AshRow, hostile: bool, friendly: bool, has: &dyn Fn(i32) -> bool) -> Vec<i32> {
    let mut out = Vec::new();
    if hostile {
        out.extend(row.arena_hostile.iter().copied().filter(|id| *id >= 0 && !has(*id)));
    }
    if friendly {
        out.extend(row.arena_friendly.iter().copied().filter(|id| *id >= 0 && !has(*id)));
    }
    out
}

/// The doping SpEffects of the ready spirit (once): the level's, then (only with `APPLY_ARENA_DOPING`) the arena's 1..5.
pub fn setup_effects(row: &AshRow, level: u8, arena_doping: bool) -> Vec<i32> {
    let mut out: Vec<i32> = level_doping(row, level).into_iter().collect();
    if arena_doping {
        out.extend(row.arena_doping.iter().copied().filter(|id| *id >= 0));
    }
    out
}

/// A one-line description of the placed ash (the log).
pub fn describe(placed: &[Placed]) -> String {
    placed.iter().map(|p| format!("row {} NpcParam {} at ({:.2}, {:.2}, {:.2}) yaw {:.1} deg anim {}", p.row.id, p.row.npc_param, p.at.x, p.at.y, p.at.z, p.yaw.to_degrees(), p.row.generate_anim)).collect::<Vec<_>>().join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    /// Lone Wolf Ashes (goods 232000), regulation 2.7.2.0: three rows of NpcParam 140700000.
    fn lone_wolf() -> Vec<(i32, AshRow)> {
        vec![(232_000, AshRow::plain(23_200_002, 140_700_000, -1.5, -1.3, -25.0)), (200_000, AshRow::plain(20_000_000, 121_000_000, 1.3, 0.0, 0.0)), (232_000, AshRow::plain(23_200_000, 140_700_000, -1.1, 1.3, 0.0)), (232_000, AshRow::plain(23_200_001, 140_700_000, 1.4, -1.3, 10.0))]
    }

    #[test]
    fn mimic_ash_every_row_of_the_trigger_is_one_spirit_in_param_order() {
        let rows = rows_of_trigger(&lone_wolf(), 232_000);
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![23_200_000, 23_200_001, 23_200_002], "Lone Wolf: three wolves");
        assert_eq!(rows_of_trigger(&lone_wolf(), 200_000).len(), 1, "Black Knife Tiche: one row");
        assert!(rows_of_trigger(&lone_wolf(), 999_000).is_empty());
        assert!(rows.iter().all(|r| chr_of(r.npc_param, r.init) == 4070), "the wolves are c4070");
    }

    #[test]
    fn mimic_ash_the_generators_place_and_facing() {
        let o = Vec3::new(10.0, 2.0, -5.0);
        // yaw 0: the rotation is the identity, the offsets negated
        let p = place(o, 0.0, &AshRow::plain(1, 1, -1.1, 1.3, 0.0));
        assert!(close(p.at, o + Vec3::new(1.1, 0.0, -1.3)), "{:?}", p.at);
        assert_eq!(p.yaw, 0.0);
        // yaw 90 deg: X column (0, 0, -1), Z column (1, 0, 0)
        let q = place(o, std::f32::consts::FRAC_PI_2, &AshRow::plain(2, 1, 1.4, -1.3, 10.0));
        assert!(close(q.at, o + Vec3::new(1.3, 0.0, 1.4)), "{:?}", q.at);
        assert!((q.yaw - (90.0f32 + 10.0).to_radians()).abs() < 1e-5);
        // the same as the rotation about +Y of the negated offset
        for yaw in [-2.0f32, -0.3, 0.7, 2.9] {
            let r = AshRow::plain(3, 1, -1.5, -1.3, -25.0);
            let want = o + Quat::from_rotation_y(yaw) * Vec3::new(1.5, 0.0, 1.3);
            assert!(close(place(o, yaw, &r).at, want), "yaw {yaw}");
        }
        // every spirit stays at the origin's height (the game's ground cast settles it)
        assert!(place_all(o, 1.0, &rows_of_trigger(&lone_wolf(), 232_000)).iter().all(|p| (p.at.y - o.y).abs() < 1e-6));
    }

    #[test]
    fn mimic_ash_origin_is_ahead_of_the_summoner() {
        let s = Vec3::new(1.0, 0.0, 1.0);
        assert!(close(origin_ahead(s, 0.0), Vec3::new(1.0, 0.0, -2.0)), "a model faces -Z");
        assert!(close(origin_ahead(s, std::f32::consts::PI), Vec3::new(1.0, 0.0, 4.0)));
        assert!(((origin_ahead(s, 0.4) - s).length() - SPAWN_AHEAD).abs() < 1e-5);
    }

    #[test]
    fn mimic_ash_level_doping_and_team() {
        let mut r = AshRow::plain(1, 1, 0.0, 0.0, 0.0);
        r.doping = [290_000, 290_001, 290_002, 290_003, 290_004, 290_005, 290_006, 290_007, 290_008, 290_009, 290_010];
        assert_eq!(level_doping(&r, 0), Some(290_000));
        assert_eq!(level_doping(&r, 10), Some(290_010));
        assert_eq!(level_doping(&r, 11), None, "none above +10 (0xd295c0)");
        r.doping[3] = -1;
        assert_eq!(level_doping(&r, 3), None);
        assert_eq!(adopt_team(47, 6), 6, "the owner's team");
        assert_eq!(adopt_team(KEPT_TEAM, 6), KEPT_TEAM, "team 5 is kept");
        r.arena_doping = [1410, 1420, -1, -1, -1];
        assert_eq!(setup_effects(&r, 4, false), vec![290_004]);
        assert_eq!(setup_effects(&r, 4, true), vec![290_004, 1410, 1420]);
    }

    #[test]
    fn mimic_ash_arena_marks_follow_the_relation_and_are_given_once() {
        let mut r = AshRow::plain(1, 1, 0.0, 0.0, 0.0);
        r.arena_hostile = [1400, -1, -1, -1, -1];
        r.arena_friendly = [1500, -1, 1501, -1, -1];
        assert_eq!(arena_marks(&r, true, false, &|_| false), vec![1400]);
        assert!(arena_marks(&r, true, false, &|id| id == 1400).is_empty(), "already on it");
        assert_eq!(arena_marks(&r, false, true, &|_| false), vec![1500, 1501]);
        assert!(arena_marks(&r, false, false, &|_| false).is_empty());
    }

    #[test]
    fn mimic_ash_the_mimic_tear_is_known_by_the_generators_marker() {
        assert!(is_mimic(&[AshRow::plain(20_700_000, 133_201_000, 1.5, -0.5, 0.0), AshRow::plain(20_700_001, 100_000_010, 1.5, -0.5, 0.0)]));
        assert!(!is_mimic(&rows_of_trigger(&lone_wolf(), 232_000)));
        assert_eq!(chr_of(100_000_010, 1), 0, "a c0000 spirit");
    }
}
