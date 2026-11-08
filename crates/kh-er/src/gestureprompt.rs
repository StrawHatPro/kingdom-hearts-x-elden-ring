//! (EMOTE-PROMPTS, THE USER 2026-11-07: "for elden ring quests that require the use of an emote, if the emote is known, give
//! a Y prompt in the area that the emote would work in Elden Ring (use Elden Ring's own data) that when activated triggers
//! the even as if the emote was used. There doesn't need to be any actual animation played for the emote. ... If you
//! haven't learnt the emote from Merchant Kale, you cannot do anything, but if you learn the emote from Merchant Kale then
//! the prompt appears in the same area that the emote would work in and pressing the propmpt triggers the event that the
//! emote would. No need to actually make emotes an item or anything like that, nor does there need to be a popup") ELDEN
//! RING'S GESTURE-GATED EVENTS AS A KH3 Y COMMAND. State note: build/sora_tmp/emote_prompts/STATE.md.
//!
//! WHAT ELDEN RING DOES (READ in the data and both executables, read-only 2.7.1.0 / 2.7.2.0 copies; not seen in the game):
//! - A gesture's animation puts a SpEffect on the player in a window of its TAE (c0000 a00.tae, event 67 / 66), and THAT is
//!   what the scripts wait on: EMEVD `4[5] IfCharacterHasSpEffect(cond, 10000, sp, 1, ..)` in the condition group of a
//!   wait, beside the player's region (`3[2]`), a radius around a character (`3[3]`), flags (`3[0]`), other SpEffects of
//!   the player (a Glintstone Crown's 486), the own-world test (`3[26]`); a talk ESD's function 61 (player has SpEffect).
//!   Every such event is a row of `sheets/port/kh-er-gesture-events.json` (tools/kh_gesture_events.js walks every event
//!   instance; no row is written by hand).
//! - WHAT THE PLAYER KNOWS: `PlayerGameData + 0x8D8` -> `CS::GestureGameData` (RTTI; vtable slot 2 = its load, which puts
//!   every GestureParam row in): `i32 entries[64]` at +8, each `gesture id * 2 | acquired`, an empty slot `-2 / -1`. The
//!   game's SetGesture (0x25BD50 in both executables) finds the id (`sar eax, 1`) and writes the low bit; the award path
//!   (`mov rcx, [rdx+8D8h]` .. `mov r8b, 1` .. `call SetGesture`) gives a gesture with the bit set. Read only here.
//!
//! WHAT WE DO: every frame the first row whose gesture is acquired and whose every condition holds right now (the leader
//! is the player body ER tests: its block-local place in the row's map, the row's shape; the live character of a radius
//! row; the flags; the player's other SpEffects) is offered as KH3's green Y command named as the gesture (erprompt.rs
//! `YOffers::gesture`, after Elden Ring's own prompt in Y's order, so a talk prompt keeps its Y). Y (the interact button of
//! the user's remap table, the pad's FaceTop as the HUD sees it) puts the row's SpEffect on the player body through the
//! game's own "apply special effect" (the function status.rs / savepoint.rs use) every frame, in the frame phase after
//! the characters' update (POST_PHYSICS), for the gesture's own TAE window but at least `MIN_HOLD_S`: the event's own
//! script sees exactly what the gesture gives it and runs unchanged. No flag is written, no animation, nothing granted.
//! A row about a character (a radius row: Patches, Dryleaf Dane) puts KH3's reaction-target marker on that character
//! (reactmark.rs); a region row has none (nothing to mark). Keys: `gesture.prompt` (1; 0 = off), `gesture.log` (1).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

const SHEET: &str = include_str!("../../../sheets/port/kh-er-gesture-events.json");

pub mod rules {
    use glam::{Vec2, Vec3};

    /// `PlayerGameData` field of the `CS::GestureGameData` pointer (READ: the award site `mov rcx, [rdx+8D8h]` in both
    /// executables; the SDK's private `gesture_game_data`, right after its `storage` at 0x8D0).
    pub const GESTURE_GAME_DATA: usize = 0x8D8;
    /// `GestureGameData` entries (after its vtable) and their count (its load copies 0x100 bytes: 64 x i32).
    pub const GESTURE_ENTRIES: usize = 0x8;
    pub const GESTURE_SLOTS: usize = 64;
    /// The shortest time the SpEffect is held (s): the gesture's own window is 0.03 .. 1.7 s; the scripts poll once a frame
    /// in a later phase, so a held half second is seen whatever the order of the phases (the events fire once).
    pub const MIN_HOLD_S: f32 = 0.5;
    /// After a press the prompt stays away this long past the hold (s): no second press while the event answers.
    pub const COOLDOWN_S: f32 = 1.5;
    /// The most SpEffects one press holds (the gesture's plus the row's given ones).
    pub const PRESS_MAX: usize = 4;

    /// (pure) One `GestureGameData` entry says gesture `id` is acquired.
    pub fn entry_acquired(entry: i32, id: i32) -> bool {
        (entry & !1) != -2 && (entry >> 1) == id && entry & 1 == 1
    }

    /// (pure) The player knows gesture `id` (ER's own record).
    pub fn knows(entries: &[i32], id: i32) -> bool {
        entries.iter().any(|e| entry_acquired(*e, id))
    }

    /// An MSB region shape (dimensions in metres).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Shape {
        Point,
        Circle { r: f32 },
        Sphere { r: f32 },
        /// Bottom at the region's place (MSB cylinders and boxes stand on their position).
        Cylinder { r: f32, h: f32 },
        Rect { w: f32, d: f32 },
        Box { w: f32, d: f32, h: f32 },
        Other,
    }

    /// Where the gesture has to be done.
    #[derive(Clone, Debug, PartialEq)]
    pub enum Where {
        /// The player inside (or outside) an MSB region of `map` (area, BB, CC).
        Region { map: (u8, i32, i32), entity: u32, shape: Shape, pos: Vec3, rot_y_deg: f32, inside: bool },
        /// The player within `radius` m of the live character `entity` (of `map`), and in front of it within the angle.
        Near { map: (u8, i32, i32), entity: u32, radius: f32, max_angle_deg: Option<f32>, inside: bool },
    }

    impl Where {
        fn map(&self) -> (u8, i32, i32) {
            match self {
                Where::Region { map, .. } | Where::Near { map, .. } => *map,
            }
        }
    }

    /// One gesture-gated event of the sheet.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Row {
        pub id: String,
        pub gesture: i32,
        pub name: String,
        pub sp: i32,
        /// The gesture's own TAE window (s).
        pub window_s: f32,
        pub wheres: Vec<Where>,
        pub flags: Vec<(u32, bool)>,
        pub any_flags: Vec<Vec<(u32, bool)>>,
        pub player_sp: Vec<(i32, bool)>,
        /// (the user's rule on single rows, e.g. "yes drop the crown requirement") SpEffects the row's script also waits on
        /// that are GIVEN with the gesture's for the press window instead of being required.
        pub extra_sp: Vec<i32>,
        pub own_world: bool,
        /// Conditions the sheet could not read (taken as met; listed in the log).
        pub unread: Vec<String>,
    }

    impl Row {
        /// The character the row is about (a radius row), for the marker.
        pub fn character(&self) -> Option<u32> {
            self.wheres.iter().find_map(|w| match w {
                Where::Near { entity, inside: true, .. } => Some(*entity),
                _ => None,
            })
        }

        /// What Y puts on the body for the press window: the gesture's SpEffect, then the row's given ones (at most
        /// [`PRESS_MAX`] in all).
        pub fn press_speffects(&self) -> Vec<i32> {
            std::iter::once(self.sp).chain(self.extra_sp.iter().copied()).take(PRESS_MAX).collect()
        }

        /// How long the SpEffect is held after Y (s).
        pub fn hold_s(&self) -> f32 {
            self.window_s.max(MIN_HOLD_S)
        }
    }

    /// `mAA_BB_CC_DD` -> (area, BB, CC).
    pub fn parse_map(name: &str) -> Option<(u8, i32, i32)> {
        let mut p = name.trim_start_matches('m').split('_');
        Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?))
    }

    fn shape_of(kind: &str, d: &[f32]) -> Shape {
        let at = |k: usize| d.get(k).copied().unwrap_or(0.0);
        match kind {
            "point" => Shape::Point,
            "circle" => Shape::Circle { r: at(0) },
            "sphere" => Shape::Sphere { r: at(0) },
            "cylinder" => Shape::Cylinder { r: at(0), h: at(1) },
            "rect" => Shape::Rect { w: at(0), d: at(1) },
            "box" => Shape::Box { w: at(0), d: at(1), h: at(2) },
            _ => Shape::Other,
        }
    }

    /// (pure) The sheet's rows.
    pub fn parse(text: &str) -> Result<Vec<Row>, String> {
        use serde_json::Value;
        let j: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let num = |v: &Value| v.as_f64().map(|n| n as f32);
        let v3 = |v: &Value| Vec3::new(num(&v[0]).unwrap_or(f32::NAN), num(&v[1]).unwrap_or(f32::NAN), num(&v[2]).unwrap_or(f32::NAN));
        let lits = |v: &Value| -> Vec<(u32, bool)> { v.as_array().map(|a| a.iter().filter_map(|l| Some((l[0].as_u64()? as u32, l[1].as_bool()?))).collect()).unwrap_or_default() };
        let mut out = Vec::new();
        for row in j["rows"].as_array().ok_or("no rows")? {
            let id = row[0].as_str().ok_or("a row without an id")?.to_string();
            let v = &row[1];
            let int = |k: &str| v[k].as_i64().map(|n| n as i32).ok_or_else(|| format!("{id}: no {k}"));
            let mut wheres = Vec::new();
            for w in v["where"].as_array().ok_or_else(|| format!("{id}: no where"))? {
                let map = w["map"].as_str().and_then(parse_map).ok_or_else(|| format!("{id}: a place without its map"))?;
                let entity = w["entity"].as_u64().ok_or_else(|| format!("{id}: a place without its entity"))? as u32;
                let inside = w["inside"].as_bool().unwrap_or(true);
                match w["kind"].as_str() {
                    Some("region") if w["missing"].as_bool() != Some(true) => {
                        let dims: Vec<f32> = w["dims"].as_array().map(|a| a.iter().filter_map(num).collect()).unwrap_or_default();
                        let shape = shape_of(w["shape"].as_str().unwrap_or(""), &dims);
                        wheres.push(Where::Region { map, entity, shape, pos: v3(&w["pos"]), rot_y_deg: num(&w["rot"][1]).unwrap_or(0.0), inside });
                    }
                    Some("near") => wheres.push(Where::Near { map, entity, radius: num(&w["radius_m"]).unwrap_or(0.0), max_angle_deg: num(&w["max_angle_deg"]), inside }),
                    _ => return Err(format!("{id}: a place this reader does not know: {w}")),
                }
            }
            let tae = &v["tae"];
            out.push(Row {
                gesture: int("gesture")?,
                name: v["gesture_name"].as_str().unwrap_or("Gesture").to_string(),
                sp: int("sp_effect")?,
                window_s: (num(&tae["end"]).unwrap_or(0.0) - num(&tae["start"]).unwrap_or(0.0)).max(0.0),
                wheres,
                flags: lits(&v["flags"]),
                any_flags: v["any_flags"].as_array().map(|a| a.iter().map(lits).collect()).unwrap_or_default(),
                extra_sp: v["extra_speffects"].as_array().map(|a| a.iter().filter_map(|s| s.as_i64().map(|n| n as i32)).collect()).unwrap_or_default(),
                player_sp: v["player_sp"].as_array().map(|a| a.iter().filter_map(|l| Some((l[0].as_i64()? as i32, l[1].as_bool()?))).collect()).unwrap_or_default(),
                own_world: v["own_world"].as_bool().unwrap_or(false),
                unread: v["unknown"].as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default(),
                id,
            });
        }
        Ok(out)
    }

    /// (pure) An MSB place `p` (the region's map coordinates) is inside the shape at `pos`, turned `rot_y_deg` about Y.
    pub fn in_shape(shape: Shape, pos: Vec3, rot_y_deg: f32, p: Vec3) -> bool {
        let d = p - pos;
        let (s, c) = rot_y_deg.to_radians().sin_cos();
        // into the region's own axes (the inverse of the MSB turn: model -Z turned by +rot about Y)
        let local = Vec2::new(d.x * c - d.z * s, d.x * s + d.z * c);
        let up = d.y >= 0.0;
        match shape {
            Shape::Point | Shape::Other => false,
            Shape::Circle { r } => local.length() <= r,
            Shape::Sphere { r } => d.length() <= r,
            Shape::Cylinder { r, h } => local.length() <= r && up && d.y <= h,
            Shape::Rect { w, d: depth } => local.x.abs() <= w * 0.5 && local.y.abs() <= depth * 0.5,
            Shape::Box { w, d: depth, h } => local.x.abs() <= w * 0.5 && local.y.abs() <= depth * 0.5 && up && d.y <= h,
        }
    }

    /// Where the player body is (Elden Ring's own place of it: what its event scripts test).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Place {
        /// The body's block (area, BB, CC).
        pub block: (u8, i32, i32),
        pub overworld: bool,
        /// Its place in that block (MSB coordinates of the block's map).
        pub local: Vec3,
        /// Its physics place (the space the characters' places are in).
        pub physics: Vec3,
    }

    /// (pure) The player's place in `map`'s MSB coordinates: the same block, or (the open world's 256 m tiles) any tile of
    /// the same area. None: another map.
    pub fn local_in(map: (u8, i32, i32), p: &Place) -> Option<Vec3> {
        if map.0 != p.block.0 {
            return None;
        }
        let (dx, dz) = (p.block.1 - map.1, p.block.2 - map.2);
        if !p.overworld && (dx != 0 || dz != 0) {
            return None;
        }
        let v = p.local + Vec3::new(256.0 * dx as f32, 0.0, 256.0 * dz as f32);
        v.is_finite().then_some(v)
    }

    /// (pure) The row's map is this block or (open world) a neighbouring tile: only then can its character be loaded near.
    fn near_map(map: (u8, i32, i32), p: &Place) -> bool {
        map.0 == p.block.0 && if p.overworld { (p.block.1 - map.1).abs() <= 1 && (p.block.2 - map.2).abs() <= 1 } else { map == p.block }
    }

    /// A live character the row is about.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Body {
        pub at: Vec3,
        /// Its facing (horizontal unit, Elden Ring axes).
        pub facing: Vec3,
        pub alive: bool,
    }

    /// What the rules ask of the game.
    pub trait World {
        /// ER's own record: the player has acquired this gesture.
        fn knows(&self, gesture: i32) -> bool;
        /// An event flag (None: not readable now).
        fn flag(&self, id: u32) -> Option<bool>;
        /// The player body carries this SpEffect now.
        fn player_has(&self, sp: i32) -> bool;
        /// The live character of this event entity id.
        fn body(&self, entity: u32) -> Option<Body>;
    }

    /// (pure) Is the player within `radius` of the body and (with an angle) in front of it within the angle?
    pub fn near(body: &Body, player: Vec3, radius: f32, max_angle_deg: Option<f32>) -> bool {
        let to = player - body.at;
        let dist = to.length();
        if dist.is_nan() || dist > radius {
            return false;
        }
        match max_angle_deg {
            None => true,
            Some(a) => {
                let h = Vec2::new(to.x, to.z);
                let f = Vec2::new(body.facing.x, body.facing.z);
                h.length() < 1e-3 || f.length() < 1e-3 || h.normalize().dot(f.normalize()).clamp(-1.0, 1.0).acos().to_degrees() < a
            }
        }
    }

    /// (pure) EVERY CONDITION ELDEN RING CHECKS for this row holds now, and the player knows the gesture.
    pub fn ready(row: &Row, w: &dyn World, p: &Place) -> bool {
        if !w.knows(row.gesture) || row.wheres.is_empty() || !row.wheres.iter().all(|x| near_map(x.map(), p)) {
            return false;
        }
        if !row.flags.iter().all(|(f, on)| w.flag(*f) == Some(*on)) {
            return false;
        }
        if !row.any_flags.iter().all(|c| c.iter().any(|(f, on)| w.flag(*f) == Some(*on))) {
            return false;
        }
        if !row.player_sp.iter().all(|(sp, has)| w.player_has(*sp) == *has) {
            return false;
        }
        row.wheres.iter().all(|x| match x {
            Where::Region { map, shape, pos, rot_y_deg, inside, .. } => local_in(*map, p).is_some_and(|l| in_shape(*shape, *pos, *rot_y_deg, l) == *inside),
            Where::Near { entity, radius, max_angle_deg, inside, .. } => w.body(*entity).filter(|b| b.alive && b.at.is_finite()).is_some_and(|b| near(&b, p.physics, *radius, *max_angle_deg) == *inside),
        })
    }

    /// (pure) The row offered: the first ready one in the sheet's order.
    pub fn pick(rows: &[Row], w: &dyn World, p: &Place) -> Option<usize> {
        rows.iter().position(|r| ready(r, w, p))
    }
}

use rules::Row;

/// The sheet's rows (read once; a bad sheet: none, logged).
fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| match rules::parse(SHEET) {
        Ok(r) => r,
        Err(e) => {
            crate::erlog!("gesture: sheets/port/kh-er-gesture-events.json unreadable ({e}): no gesture prompts");
            Vec::new()
        }
    })
}

/// The row offered on Y now and whether it is the first Y command (no Save, party talk or Elden Ring prompt above it).
#[derive(Clone, Debug, Default, PartialEq)]
struct Offer {
    row: Option<usize>,
    top: bool,
}

/// A press being answered: the SpEffect held on the body.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hold {
    row: usize,
    sp: i32,
    /// Every SpEffect held (the gesture's first), how many, and which were on the body before the press (never taken off:
    /// a worn Glintstone Crown's own 486 stays).
    sps: [i32; rules::PRESS_MAX],
    n: usize,
    keep: [bool; rules::PRESS_MAX],
    hold_s: f32,
    since: Option<std::time::Instant>,
    frames: u32,
    seen_on_body: bool,
}

#[derive(Default)]
struct State {
    offer: Offer,
    hold: Option<Hold>,
    cooldown_until: Option<std::time::Instant>,
    logged: Option<Option<usize>>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Y's down edge since the last frame while the gesture command was the first Y command; the pad of the frame before.
static Y_LATCH: AtomicBool = AtomicBool::new(false);
static Y_BEFORE: AtomicBool = AtomicBool::new(false);
static TOP: AtomicBool = AtomicBool::new(false);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(State::default))
}

/// (hud.rs `HudHost::input`, every frame) Y's down edge is the gesture's while its command is the first Y command (and
/// hidden from Sora's situation command), as the Moogle's talk (moogle.rs `note_pad`).
pub fn note_pad(held: &mut kh_core::PadButtons) {
    let y = held.has(kh_core::PadButton::FaceTop);
    let before = Y_BEFORE.swap(y, Ordering::Relaxed);
    if TOP.load(Ordering::Relaxed) {
        if y && !before {
            Y_LATCH.store(true, Ordering::Relaxed);
        }
        held.set(kh_core::PadButton::FaceTop, false);
    }
}

/// (hud.rs, erprompt.rs `YOffers::gesture`) The gesture command offered now (its name), if any.
pub fn offer() -> Option<String> {
    let row = with(|s| s.offer.row)?;
    rows().get(row).map(|r| r.name.clone())
}

/// (pure) The gesture command is the first Y command: nothing of Save, a party talk or Elden Ring's prompt is up.
pub fn is_top(offered: bool, save: bool, party_talks: usize, er_prompt: bool) -> bool {
    offered && !save && party_talks == 0 && !er_prompt
}

#[cfg(windows)]
mod game {
    use super::rules::{self, Body, Place, World};
    use eldenring::cs::PlayerGameData;
    use glam::Vec3;

    /// The game's state the rules read, this frame.
    pub struct GameWorld {
        pub entries: Option<[i32; rules::GESTURE_SLOTS]>,
    }

    impl GameWorld {
        pub fn read() -> Self {
            GameWorld { entries: entries() }
        }
    }

    /// ER's own gesture record (`PlayerGameData + 0x8D8` -> `GestureGameData` + 8: 64 entries). None: not readable.
    pub fn entries() -> Option<[i32; rules::GESTURE_SLOTS]> {
        let data = crate::loot::player_data()?;
        let base = data as *mut PlayerGameData as *const u8;
        // SAFETY: the field is inside the live PlayerGameData (0x8D8 < its size, the SDK's `gesture_game_data`); game thread.
        let ggd = crate::game::read_word(unsafe { base.add(rules::GESTURE_GAME_DATA) } as *const usize);
        if !crate::game::plausible(ggd) {
            return None;
        }
        let mut out = [0i32; rules::GESTURE_SLOTS];
        for (k, e) in out.iter_mut().enumerate() {
            // SAFETY: the object is CS::GestureGameData (0x108 bytes: vtable + 64 x i32), alive with the player data.
            *e = unsafe { std::ptr::read_unaligned((ggd + rules::GESTURE_ENTRIES + k * 4) as *const i32) };
        }
        Some(out)
    }

    impl World for GameWorld {
        fn knows(&self, gesture: i32) -> bool {
            self.entries.is_some_and(|e| rules::knows(&e, gesture))
        }
        fn flag(&self, id: u32) -> Option<bool> {
            crate::bossreward::game_flag(id)
        }
        fn player_has(&self, sp: i32) -> bool {
            crate::game::player_mut().is_some_and(|p| p.chr_ins.special_effect.entries().take(128).any(|e| e.param_id == sp))
        }
        fn body(&self, entity: u32) -> Option<Body> {
            let chr = crate::zphase::chr_of_entity(entity)?;
            let at = crate::game::v3(&chr.modules.physics.position);
            let yaw = crate::coords::yaw_kh_of_er_quat(crate::game::quat(&chr.modules.physics.orientation));
            Some(Body { at, facing: crate::coords::facing_er(yaw), alive: chr.modules.data.hp > 0 && !chr.chr_flags1c5.death_flag() })
        }
    }

    /// The player body's place (None: no player, dying, or not readable).
    pub fn place() -> Option<Place> {
        let player = crate::game::player_mut()?;
        if !crate::game::player_alive(player) {
            return None;
        }
        let b = player.current_block_id;
        let local = Vec3::new(player.block_position.x, player.block_position.y, player.block_position.z);
        let physics = crate::game::v3(&player.chr_ins.modules.physics.position);
        (local.is_finite() && physics.is_finite()).then_some(Place { block: (b.area(), b.block() as i32, b.region() as i32), overworld: b.is_overworld(), local, physics })
    }

    /// The marker's point of the row's character: its body's middle (feet + half its capsule height, reactmark.rs's rule).
    pub fn mark_point(entity: u32) -> Option<Vec3> {
        let chr = crate::zphase::chr_of_entity(entity)?;
        let feet = crate::game::v3(&chr.modules.physics.position);
        let h = chr.modules.physics.hit_height;
        (feet.is_finite() && h.is_finite() && h > 0.0).then(|| feet + Vec3::Y * (h * 0.5))
    }

    /// The SpEffect put on the body through the game's own function (the SDK's `ChrInsExt::apply_speffect`, the function
    /// status.rs finds by pattern: 0x3E8DC0 in both executables). Returns whether the body carries it right after.
    pub fn apply(sp: i32) -> bool {
        use eldenring::cs::ChrInsExt;
        let Some(player) = crate::game::player_mut() else { return false };
        player.chr_ins.apply_speffect(sp, false);
        player.chr_ins.special_effect.entries().take(128).any(|e| e.param_id == sp)
    }

    /// The SpEffect taken off the body at the end of a hold, if it is still there.
    pub fn take_off(sp: i32) {
        use eldenring::cs::ChrInsExt;
        if let Some(player) = crate::game::player_mut() {
            if player.chr_ins.special_effect.entries().take(128).any(|e| e.param_id == sp) {
                player.chr_ins.remove_speffect(sp);
            }
        }
    }
}

/// What the frame knows besides the game (sora.rs).
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameIn {
    /// A cutscene or a menu is up: nothing offered.
    pub hidden: bool,
}

/// (sora.rs, once a frame) THE GESTURE COMMAND: the row offered, Y answered (the hold starts), the marker's point.
#[cfg(windows)]
pub fn frame(control: &crate::control::Control, input: FrameIn) {
    let on = control.flag("gesture.prompt") != Some(false);
    let log = control.flag("gesture.log") != Some(false);
    let rows = rows();
    let now = std::time::Instant::now();
    let pressed = Y_LATCH.swap(false, Ordering::Relaxed);
    let world = game::GameWorld::read();
    with(|s| {
        // Y on the command offered last frame
        if pressed && s.hold.is_none() {
            if let Some(i) = s.offer.row.filter(|_| s.offer.top) {
                let r = &rows[i];
                let list = r.press_speffects();
                let (mut sps, mut keep) = ([0; rules::PRESS_MAX], [false; rules::PRESS_MAX]);
                for (k, sp) in list.iter().enumerate() {
                    sps[k] = *sp;
                    keep[k] = rules::World::player_has(&world, *sp);
                }
                s.hold = Some(Hold { row: i, sp: r.sp, sps, n: list.len(), keep, hold_s: r.hold_s(), since: None, frames: 0, seen_on_body: false });
                if log && !r.extra_sp.is_empty() {
                    crate::erlog!("gesture: \"{}\" ({}): SpEffect(s) {:?} given with the gesture's for the press window (the user's rule on this row; the script's own condition passes unchanged)", r.name, r.id, r.extra_sp);
                }
                s.cooldown_until = Some(now + std::time::Duration::from_secs_f32(r.hold_s() + rules::COOLDOWN_S));
                if log {
                    crate::erlog!("gesture: Y -> \"{}\" ({}): SpEffect {} held on the player body for {:.2} s (the gesture's own window {:.3} s), as gesture {} gives it; nothing else written{}", r.name, r.id, r.sp, r.hold_s(), r.window_s, r.gesture, if r.unread.is_empty() { String::new() } else { format!(" (conditions taken as met: {})", r.unread.join(", ")) });
                }
            }
        }
        let cooling = s.cooldown_until.is_some_and(|t| now < t);
        let row = if on && !input.hidden && s.hold.is_none() && !cooling { game::place().and_then(|p| rules::pick(rows, &world, &p)) } else { None };
        let top = is_top(row.is_some(), crate::savepoint::prompt_shown(), crate::conv::party_talks_offered(), crate::erprompt::er_up());
        s.offer = Offer { row, top };
        TOP.store(top, Ordering::Relaxed);
        crate::reactmark::set_gesture_point(row.and_then(|i| rows[i].character()).and_then(|e| game::mark_point(e).map(|p| (u64::from(e) | 0x20, p))));
        if log && s.logged != Some(row) {
            match row {
                Some(i) => crate::erlog!("gesture: OFFERED \"{}\" ({}; gesture {} known, every condition of the row holds){}", rows[i].name, rows[i].id, rows[i].gesture, if top { "" } else { ": under Elden Ring's own prompt / a talk in Y's order" }),
                None if s.logged.is_some() => crate::erlog!("gesture: no gesture command now"),
                None => {}
            }
            s.logged = Some(row);
        }
    });
}

/// (frame.rs, the POST_PHYSICS phase: after the characters' update, before the event scripts poll) the hold's SpEffect on
/// the body this frame; at its end, taken off if still there.
#[cfg(windows)]
pub fn post_physics() {
    let done = with(|s| {
        let h = s.hold.as_mut()?;
        let started = *h.since.get_or_insert_with(std::time::Instant::now);
        if started.elapsed().as_secs_f32() >= h.hold_s {
            return Some(*h);
        }
        let mut on = false;
        for (k, sp) in h.sps[..h.n].iter().enumerate() {
            let stuck = game::apply(*sp);
            on |= k == 0 && stuck;
        }
        h.frames += 1;
        if on && !h.seen_on_body {
            h.seen_on_body = true;
            crate::erlog!("gesture: SpEffect {} on the player body (frame {} of the hold)", h.sp, h.frames);
        }
        None
    });
    if let Some(h) = done {
        for (sp, keep) in h.sps[..h.n].iter().zip(h.keep) {
            if !keep {
                game::take_off(*sp);
            }
        }
        crate::erlog!("gesture: hold of SpEffect {} ended after {} frame(s) ({} s){}", h.sp, h.frames, h.hold_s, if h.seen_on_body { "" } else { ": NOT SEEN ON THE BODY after the game's apply" });
        with(|s| s.hold = None);
    }
}

#[cfg(test)]
mod tests {
    use super::rules::*;
    use super::*;
    use glam::Vec3;
    use std::collections::{HashMap, HashSet};

    /// A mock game: known gestures, flags, the player's SpEffects, live characters.
    #[derive(Default)]
    struct Mock {
        known: HashSet<i32>,
        flags: HashMap<u32, bool>,
        sp: HashSet<i32>,
        bodies: HashMap<u32, Body>,
    }

    impl World for Mock {
        fn knows(&self, g: i32) -> bool {
            self.known.contains(&g)
        }
        fn flag(&self, id: u32) -> Option<bool> {
            Some(self.flags.get(&id).copied().unwrap_or(false))
        }
        fn player_has(&self, sp: i32) -> bool {
            self.sp.contains(&sp)
        }
        fn body(&self, e: u32) -> Option<Body> {
            self.bodies.get(&e).copied()
        }
    }

    fn all() -> Vec<Row> {
        parse(SHEET).expect("the sheet parses")
    }

    fn row(id: &str) -> Row {
        all().into_iter().find(|r| r.id == id).unwrap_or_else(|| panic!("no row {id}"))
    }

    /// A mock where every condition of `r` holds and the player stands at the middle of its place.
    fn satisfied(r: &Row) -> (Mock, Place) {
        let mut m = Mock::default();
        m.known.insert(r.gesture);
        for (f, on) in &r.flags {
            m.flags.insert(*f, *on);
        }
        for c in &r.any_flags {
            if !c.iter().any(|(f, on)| r.flags.contains(&(*f, *on))) {
                let (f, on) = c[0];
                m.flags.insert(f, on);
            }
        }
        for (sp, has) in &r.player_sp {
            if *has {
                m.sp.insert(*sp);
            }
        }
        let mut place = Place { block: (0, 0, 0), overworld: false, local: Vec3::ZERO, physics: Vec3::new(10.0, 0.0, 10.0) };
        for w in &r.wheres {
            match w {
                Where::Region { map, shape, pos, .. } => {
                    let up = match shape {
                        Shape::Cylinder { h, .. } | Shape::Box { h, .. } => h * 0.5,
                        _ => 0.0,
                    };
                    place.block = *map;
                    place.overworld = (50..89).contains(&map.0);
                    place.local = *pos + Vec3::Y * up;
                }
                Where::Near { map, entity, radius, .. } => {
                    place.block = *map;
                    place.overworld = (50..89).contains(&map.0);
                    m.bodies.insert(*entity, Body { at: place.physics + Vec3::new(0.0, 0.0, radius * 0.5), facing: Vec3::new(0.0, 0.0, -1.0), alive: true });
                }
            }
        }
        (m, place)
    }

    #[test]
    fn gesture_sheet_has_every_event_found_in_the_data() {
        let rows = all();
        let ids: HashSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids.len(), rows.len(), "ids unique");
        // the events the brief names, read from Elden Ring's scripts: Blaidd's howl (Kale's Finger Snap), Patches' pardon
        // (Extreme Repentance), the two Erudition spots, the Shadow Keep statue (O Mother), Dryleaf Dane (May the Best Win)
        for (event, gesture, sp) in [(1_045_370_711, 73, 9672), (31_003_701, 40, 9700), (31_003_711, 40, 9700), (1_037_423_711, 40, 9700), (1_037_543_702, 40, 9700), (1_034_432_580, 103, 485), (1_039_482_610, 103, 485), (21_012_576, 115, 102_334), (425_006_100, 111, 9912)] {
            assert!(rows.iter().any(|r| r.id.contains(&event.to_string()) && r.gesture == gesture && r.sp == sp), "event {event}");
        }
        for r in &rows {
            assert!(!r.wheres.is_empty(), "{}: a place", r.id);
            assert!(r.hold_s() >= MIN_HOLD_S && r.window_s >= 0.0, "{}", r.id);
        }
    }

    #[test]
    fn gesture_blaidd_row_is_mistwoods_sphere_and_his_flags() {
        let r = row("gesture.emevd.1045370711");
        assert_eq!((r.gesture, r.name.as_str(), r.sp), (73, "Finger Snap", 9672));
        assert_eq!(r.wheres, vec![Where::Region { map: (60, 45, 37), entity: 1_045_372_701, shape: Shape::Sphere { r: 25.0 }, pos: Vec3::new(-71.884, 23.492, -99.82), rot_y_deg: 0.0, inside: true }]);
        assert_eq!(r.flags, vec![(3600, true), (3605, true), (1_045_370_711, false)]);
        assert!(r.own_world && r.character().is_none(), "a region row: no marker");
    }

    /// For EVERY row: the prompt shows only when the gesture is known, the player is in the place and every condition
    /// holds; any one of them missing and it does not show.
    #[test]
    fn gesture_every_row_shows_only_when_known_in_place_and_conditions_hold() {
        for r in all() {
            let (m, p) = satisfied(&r);
            assert!(ready(&r, &m, &p), "{}: everything holds", r.id);
            // not known (ER's own record): nothing
            let mut unknown = Mock { known: HashSet::new(), ..Default::default() };
            unknown.flags.clone_from(&m.flags);
            unknown.sp.clone_from(&m.sp);
            unknown.bodies.clone_from(&m.bodies);
            assert!(!ready(&r, &unknown, &p), "{}: unknown gesture", r.id);
            // a flag in the wrong state
            for (f, on) in &r.flags {
                let mut bad = Mock { known: m.known.clone(), flags: m.flags.clone(), sp: m.sp.clone(), bodies: m.bodies.clone() };
                bad.flags.insert(*f, !*on);
                assert!(!ready(&r, &bad, &p), "{}: flag {f} wrong", r.id);
            }
            // none of an either-or set
            for c in &r.any_flags {
                let mut bad = Mock { known: m.known.clone(), flags: m.flags.clone(), sp: m.sp.clone(), bodies: m.bodies.clone() };
                for (f, on) in c {
                    bad.flags.insert(*f, !*on);
                }
                if r.flags.iter().all(|(f, on)| bad.flags.get(f) == Some(on)) {
                    assert!(!ready(&r, &bad, &p), "{}: none of {c:?}", r.id);
                }
            }
            // a SpEffect the player must carry (a Glintstone Crown's 486)
            for (sp, has) in r.player_sp.iter().filter(|x| x.1) {
                let mut bad = Mock { known: m.known.clone(), flags: m.flags.clone(), sp: m.sp.clone(), bodies: m.bodies.clone() };
                bad.sp.remove(sp);
                assert!(!ready(&r, &bad, &p) && *has, "{}: without SpEffect {sp}", r.id);
            }
            // out of the place
            let mut away = p;
            away.local += Vec3::new(300.0, 0.0, 0.0);
            away.physics += Vec3::new(300.0, 0.0, 0.0);
            assert!(!ready(&r, &m, &away), "{}: far away", r.id);
            // another map
            let mut elsewhere = p;
            elsewhere.block.0 = elsewhere.block.0.wrapping_add(1);
            assert!(!ready(&r, &m, &elsewhere), "{}: another map", r.id);
            // the character dead or not loaded
            if let Some(e) = r.character() {
                let mut gone = Mock { known: m.known.clone(), flags: m.flags.clone(), sp: m.sp.clone(), bodies: HashMap::new() };
                assert!(!ready(&r, &gone, &p), "{}: no character", r.id);
                gone.bodies.insert(e, Body { alive: false, ..m.bodies[&e] });
                assert!(!ready(&r, &gone, &p), "{}: dead character", r.id);
            }
            assert_eq!(pick(std::slice::from_ref(&r), &m, &p), Some(0));
        }
    }

    #[test]
    fn gesture_shapes_and_turns() {
        // a cylinder stands on its place
        let c = Shape::Cylinder { r: 8.0, h: 15.0 };
        assert!(in_shape(c, Vec3::ZERO, 0.0, Vec3::new(7.9, 14.9, 0.0)));
        assert!(!in_shape(c, Vec3::ZERO, 0.0, Vec3::new(0.0, -0.1, 0.0)));
        assert!(!in_shape(c, Vec3::ZERO, 0.0, Vec3::new(0.0, 15.1, 0.0)));
        // a box turned 90 degrees: its width runs along z
        let b = Shape::Box { w: 10.0, d: 2.0, h: 5.0 };
        assert!(in_shape(b, Vec3::ZERO, 0.0, Vec3::new(4.9, 1.0, 0.0)));
        assert!(!in_shape(b, Vec3::ZERO, 90.0, Vec3::new(4.9, 1.0, 0.0)));
        assert!(in_shape(b, Vec3::ZERO, 90.0, Vec3::new(0.0, 1.0, 4.9)));
        assert!(in_shape(Shape::Sphere { r: 25.0 }, Vec3::ZERO, 0.0, Vec3::new(0.0, -24.0, 0.0)));
        assert!(!in_shape(Shape::Point, Vec3::ZERO, 0.0, Vec3::ZERO));
        // the open world's neighbouring tile: 256 m
        let p = Place { block: (60, 45, 38), overworld: true, local: Vec3::new(-71.0, 23.0, -356.0), physics: Vec3::ZERO };
        assert_eq!(local_in((60, 45, 37), &p), Some(Vec3::new(-71.0, 23.0, -100.0)));
        let legacy = Place { block: (21, 1, 0), overworld: false, ..p };
        assert_eq!(local_in((21, 1, 0), &legacy), Some(legacy.local));
        assert_eq!(local_in((21, 0, 0), &legacy), None);
        // the angle: behind a character within its 10 degrees is out (Dryleaf Dane's 170)
        let body = Body { at: Vec3::ZERO, facing: Vec3::Z, alive: true };
        assert!(near(&body, Vec3::new(1.0, 0.0, -3.0), 4.0, Some(170.0)), "18 degrees off straight behind: in");
        assert!(!near(&body, Vec3::new(0.5, 0.0, -3.0), 4.0, Some(170.0)), "9.5 degrees off straight behind: out");
        assert!(!near(&body, Vec3::new(0.0, 0.0, -3.0), 4.0, Some(170.0)));
        assert!(!near(&body, Vec3::new(0.0, 0.0, 4.1), 4.0, Some(170.0)));
    }

    #[test]
    fn gesture_record_is_er_gesture_game_data() {
        // `id * 2 | acquired`; an empty slot -2 / -1 (GestureGameData's SetGesture / load, both executables)
        let mut e = [-2i32; GESTURE_SLOTS];
        e[0] = 73 * 2; // listed, not acquired (the load puts every GestureParam row in)
        assert!(!knows(&e, 73));
        e[0] = (73 * 2) | 1;
        assert!(knows(&e, 73) && !knows(&e, 40));
        assert!(!knows(&[-1; 4], -1), "an empty slot is no gesture");
        assert!(!entry_acquired(0, 0) && entry_acquired(1, 0));
    }

    #[test]
    fn gesture_y_is_the_gestures_only_as_the_first_command() {
        assert!(is_top(true, false, 0, false));
        assert!(!is_top(true, true, 0, false), "the save point first");
        assert!(!is_top(true, false, 1, false), "a party talk first");
        assert!(!is_top(true, false, 0, true), "Elden Ring's own prompt (a talk) keeps its Y");
        assert!(!is_top(false, false, 0, false));
        // the latch: Y's down edge while top, hidden from the rest of the pad
        TOP.store(true, Ordering::Relaxed);
        Y_BEFORE.store(false, Ordering::Relaxed);
        Y_LATCH.store(false, Ordering::Relaxed);
        let mut held = kh_core::PadButtons::NONE;
        held.set(kh_core::PadButton::FaceTop, true);
        note_pad(&mut held);
        assert!(!held.has(kh_core::PadButton::FaceTop) && Y_LATCH.swap(false, Ordering::Relaxed));
        TOP.store(false, Ordering::Relaxed);
        let mut held = kh_core::PadButtons::NONE;
        held.set(kh_core::PadButton::FaceTop, true);
        Y_BEFORE.store(false, Ordering::Relaxed);
        note_pad(&mut held);
        assert!(held.has(kh_core::PadButton::FaceTop) && !Y_LATCH.load(Ordering::Relaxed), "not top: Y is not ours");
    }

    /// (THE USER 2026-11-07, verbatim: "yes drop the crown requirement") Erudition near Frenzied Flame Village: the prompt
    /// shows without a Glintstone Crown, and Y gives Erudition's 485 AND the crown's 486 for the press window, so the
    /// script's own condition (485 and 486 on the player) passes unchanged.
    #[test]
    fn gesture_erudition_needs_no_crown_and_the_press_gives_485_and_486() {
        let r = row("gesture.emevd.1039482610");
        assert!(r.player_sp.is_empty(), "the crown is not required");
        assert_eq!(r.extra_sp, vec![486]);
        assert_eq!(r.press_speffects(), vec![485, 486]);
        let (mut m, p) = satisfied(&r);
        assert!(!m.player_has(486) && ready(&r, &m, &p), "known + in the cylinder is enough");
        // what the press puts on the body is exactly what the script waits on
        for sp in r.press_speffects() {
            m.sp.insert(sp);
        }
        assert!(m.player_has(485) && m.player_has(486));
        // every other row holds the gesture's SpEffect alone
        for o in all().into_iter().filter(|o| o.id != r.id) {
            assert_eq!(o.press_speffects(), vec![o.sp], "{}", o.id);
        }
    }

    /// Pressing produces the gesture's condition: the row's SpEffect, held at least the gesture's own window, nothing else.
    #[test]
    fn gesture_press_holds_the_gestures_own_speffect() {
        for r in all() {
            assert!(r.hold_s() >= r.window_s && r.hold_s() >= MIN_HOLD_S);
            // what the scripts wait on (4[5] / talk function 61) is exactly this SpEffect on the player
            let (mut m, p) = satisfied(&r);
            m.sp.insert(r.sp);
            assert!(m.player_has(r.sp) && ready(&r, &m, &p));
        }
        assert_eq!(row("gesture.emevd.1045370711").hold_s(), MIN_HOLD_S, "Finger Snap's window is 0.1 s");
        assert!((row("gesture.emevd.1034432580").hold_s() - 1.7).abs() < 0.01, "Erudition's window is 1.7 s");
    }
}
