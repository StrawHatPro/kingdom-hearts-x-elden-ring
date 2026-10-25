//! (savepoint) KINGDOM HEARTS III'S SAVE POINT AT ELDEN RING'S SITES OF GRACE (`savepoint.kh`).
//! State note: build/sora_tmp/er/IMPL_ER9_STATE.md; the save point itself is `kh_host::savepoint`
//! (docs/sora-savepoint-spec.md), its menu `kh_ui::savepoint` through the HUD (hud.rs).
//!
//! WHERE: a site of grace is a character of the game (c1000, one per grace, with the grace's
//! entity id): every one the target list reads (`ErTargets::list`, 40 m) gets a save point at its
//! feet, discovered or not. Nothing is learnt or stored: the game's own objects of the loaded area
//! are the list. Whether a grace is discovered is its BonfireWarpParam row's event flag.
//!
//! WHAT IT DOES:
//! - the KH3 effects (base + body; the heal burst while he recovers) as free effects of the effect side;
//! - standing in the recover capsule heals HP, MP and focus by KH3's rule (+180 a second each),
//!   without resting;
//! - Y while Elden Ring's own prompt is on screen and he stands in the reactor of a DISCOVERED
//!   grace opens the KH3 save-point menu instead of the game's rest (Y is hidden from the game);
//!   at an undiscovered grace Y is the game's ("Touch grace": its discovery, banner and map entry).
//!   The menu: SAVE = the game's own rest (the world resets, the game saves; its grace menu is
//!   left at once) + Sora's save file + the pouch refill of every unpause; SELECT AN AREA = the
//!   game's map (fast travel); SITE OF GRACE = the game's rest with its grace menu left open (pass
//!   time, sort chest, flasks ...). Level Up is not ours to offer, and while he rests the wallet
//!   shows the game 0 runes (prize.rs), so the game's own Level Up can raise nothing.
//!
//! (IMPL_ER26, user decisions 2026-10-13) NO ATTUNING, NO SITTING, PASS TIME, RESPAWN ENEMIES (rules: savepoint_rules.rs,
//! menu: kh_host::savepoint_menu; state note build/sora_tmp/er/IMPL_ER26_STATE.md). What is said above about Y at an
//! undiscovered grace and about the entry SITE OF GRACE holds only with `savepoint.sit = 1`. By default:
//! - RUNNING INTO a save point the game presents unlocks it at once: its grace's discovery flag (the BonfireWarpParam
//!   row's event flag) is set and read back, as the game's "Touch grace" sets it, without the kneel, without its
//!   "LOST GRACE DISCOVERED" banner. The map shows it and travels to it from then on (the map reads that flag).
//! - Sora never sits: the menu is Save / Select an Area / Pass Time / Respawn Enemies / Title Screen; Y near a save
//!   point is never Elden Ring's (neither "Touch grace" nor "Rest at site of grace" can be pressed).
//! - PASS TIME and RESPAWN ENEMIES ask the game's own time-transition control (`CSEventWorldAreaTimeCtrl`, the one its
//!   rest and its "Pass time" go through: its fade, the clock, the world reset) with the SDK's
//!   `fade_out_and_pass_time`: no rest, no animation, nothing saved, the character not reset.
//! - STORY EVENTS THAT ONLY THE GAME'S REST STARTS (`savepoint.rest_events`, default `sit`): until the Roundtable
//!   Hold's grace is found, the first Save at each save point is followed by the game's real rest (its grace menu or
//!   Melina's scene is then the player's). LB + Y at a save point asks for that rest at any time (`savepoint.rest_chord`).
//!
//! | key | default | meaning |
//! |---|---|---|
//! | `savepoint.autounlock` | 1 | 0 = a grace is discovered only by the game's own "Touch grace" (needs `savepoint.sit = 1` to be pressable) |
//! | `savepoint.sit` | 0 | 1 = the rules before IMPL_ER26: the Site of Grace entry, the game's own touch, Y by its prompt |
//! | `savepoint.load` | 1 | (SV) the Load entry (KH3's Load screen; needs `saveload.kh`); `saveload.confirm_load` 1 = its Yes / No question, `saveload.delete` 1 = Y deletes a slot (Yes / No) on the Save and Load screens, `saveload.cover` 1 = the load goes through black (`saveload.fade_in` 0.25 s, `saveload.fade_out` 0.5 s) |
//! | `savepoint.passtime` | 1 | the Pass Time entry; `savepoint.passtime.morning / .noon / .nightfall` = `H:M` (7:00, 12:00, 20:00); `savepoint.passtime.reset = 1`: the world is reset with it |
//! | `savepoint.respawn` | 1 | the Respawn Enemies entry |
//! | `savepoint.respawn_on_save` | 0 | 1 = the world is reset after every completed Save |
//! | `savepoint.rest_events` | 0 | (IMPL_ER33: OFF BY DEFAULT) `sit` / `always`: the game's real rest after a Save, the rule of IMPL_ER26 (debugging) |
//! | `savepoint.rest_chord` | 0 | (IMPL_ER33: OFF BY DEFAULT) 1 = LB + Y at a save point = the game's own rest and grace menu (a hidden fallback for what only Elden Ring's menu offers) |
//! | `savepoint.sit_pose` | 0 | (IMPL_ER33) 1 = Sora takes the body's pose while it attunes / rests (he kneels and sits); 0 = he stands |
//! | `cut.savepoint` | 1 | (IMPL_ER33) in a real-time cutscene the save points are still drawn (the scene's own grace is cutgrace.rs's) |
//!
//! (IMPL_ER33, user decision 2026-10-13: "Remove it completely. Make the cutscenes happen automatically when you unlock
//! the correct amount of save points") THE STORY EVENTS: story.rs (the table sheets/port/sora-er-story.json, the keys
//! `story.auto`, `story.talk`, `story.cover`, `story.log`). A Save is never followed by a rest. When a row of the table is
//! met at the save point he stands in (at the unlock by touch, or once a session for a character already past it), the
//! game's REAL rest is made for the body under a black cover and Sora does not take the body's pose: nothing of the
//! attuning or the sitting is seen; the scene / Melina's talk that follows is the game's. The menu shows "Talk to
//! Melina" only while a row says the game offers a talk.
//! (IMPL_ER36, the user: "The Talk to Melina option in the save point menu should just talk to Melina and thats it,
//! not open grace menu") EVERY STORY REST IS DRIVEN TO ITS END (talkdrive.rs): the game's grace menu is never shown and
//! never left to the player: her entry is selected for him under the cover, the cover is off while she speaks, and
//! when the game's menu comes back it is covered at once and the rest is left (B, until the body stands). Only the
//! accord is made unasked. Keys: `story.menu.ups` (the cursor steps, default derived), `story.diag = 0` (no flag line).
//! | `savepoint.guard` | 2.0 | m around a save point within which a prompt of Elden Ring's cannot be answered with Y (NOT MEASURED: how far out the game offers its rest; the line `!!! savepoint: THE GAME'S OWN REST STARTED WITHOUT US` names the distance when it was too small) |
//!
//! HOW THE GAME IS ASKED: there is no function for "rest" or "open the map" in the SDK: the press
//! is given to the game alone (`inputs::press_for_er`: the pad hook when it reads a pad, else the
//! keyboard key of its default layout).
//!
//! | key | default | meaning |
//! |---|---|---|
//! | `savepoint.kh` | 1 | KH3 save points at the sites of grace |
//! | `savepoint.heal` | 1 | 0 = standing on one does not heal |
//! | `savepoint.sfx` | sheet savepoint.sfx (1) | (IMPL_ER19) the save point's sound (the one its heal burst carries, se00000_016): 0 = none, 1 = KH3: each time the heal burst starts, 2 = also at every entry with nothing to heal (not KH3) |
//! | `savepoint.first` | 0 | (IMPL_ER19) 1 = every save point that comes into reach plays its 3 s first-visit burst again (the behaviour before IMPL_ER19); 0 = KH3: only a grace that is not discovered yet |
//! | `savepoint.menu` | 1 | 0 = Y is always the game's (its own rest and grace menu) |
//! | `savepoint.height` | 92 | cm of the save point's root above the grace's feet |
//! | `savepoint.ground` | 1 | (IMPL_ER52) 0 = the effects at the root (before IMPL_ER52); 1 = KH3: on the floor under the root (sheet rows `savepoint.ground_offset / _reach / _miss`) |
//!
//! (IMPL_ER52, the user 2026-10-13: "move the save so it's in the same position on the ground as KH3") KH3 spawns the
//! three effects at the floor its BeginPlay trace meets 140 cm under the actor root, else 90 cm under it (0x140F9AA40,
//! verified at instruction level: kh_host::savepoint::ground_offset); the root stays 92 cm above the floor (the capsules
//! are placed from it). Here: a line straight down from the root (`savepoint.height` above the grace's character)
//! through Sora's own collision world with the graces' mounds left out when they are hidden (graceflame.rs), once a
//! second per save point; the effects are made anew when the floor found moves by more than 2 cm. Log lines `grace:`.
//! | `savepoint.glow` | keep | `render` / `alpha`: the grace character's own drawing switched off (EXPERIMENT, see the note) |
//! | `dbg.lua` | - | ONE-SHOT: the game's rest state and bonfire conditions are logged |

use eldenring::cs::{CSEventFlagMan, CSEventManImp, CSEventWorldAreaTimeCtrl, CSLuaEventManImp, FieldInsHandle, PlayerIns, SoloParamRepository, TimeTransitionParams, WorldAreaTime, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::Vec3;
use kh_core::Sora;
use kh_host::savepoint::{SavePoint, SavePointData, SavePointSound, SavePointStep};
use kh_host::savepoint_menu::{GraceAction, GraceMenu, TimeChoice};
use kh_ui::savepoint::SavePointResult;

use crate::control::{Control, OneShot};
use crate::coords::Frame;
use crate::erlog;
use crate::graceflame::Presence;
use crate::game::plausible;
use crate::inputs::{self, ErUi};
use crate::savepoint_rules::{CtrlSnapshot, Ground, RestEvents, Unlock, UnlockStep, YIn};
use crate::story::{Blocked, Ctx, Offer, Story};
use crate::talkdrive::{self, Act, Drive, End, Goal, Seen};
use crate::targets::ErTargets;
use crate::word;

/// The character id of a site of grace (c1000).
const NPC_GRACE: i32 = crate::targets::NPC_HELPER;

/// (SP2, the user 2026-10-20: "There also needs to be a proper prompt for using the save point when inside it") KH3's
/// Save reaction command is up (written by `frame`, read by hud.rs every HUD frame, whoever leads).
static PROMPT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (SP2) The save point's "Save" command is to be shown in the command menu now.
pub fn prompt_shown() -> bool {
    PROMPT.load(std::sync::atomic::Ordering::Relaxed)
}
/// (IMPL_ER26) The menu's entries, their order, texts and icons: `kh_host::savepoint_menu` (the offline harness shows
/// the same). Which KH3 entry stands for what: Select an Area = KH3's World Map (the globe), Pass Time = the Tutorials'
/// cell, Respawn Enemies = the Bistro's cell, Site of Grace (`savepoint.sit = 1`) = KH3's Select an Area (the ring).
struct Point {
    /// The grace's own character (c1000), once it was in reach.
    handle: Option<FieldInsHandle>,
    entity: u32,
    flag: u32,
    discovered: Option<bool>,
    er_pos: Vec3,
    sp: SavePoint,
    unseen: f32,
    step: SavePointStep,
    /// (IMPL_ER21) What the game shows of this grace (graceflame.rs: its map object plays its flame or not), when the
    /// point was made (our clock, the search's rounds), and whether no other grace shares its tile / block.
    presence: Presence,
    since: f32,
    rounds_at: u32,
    alone: bool,
    /// (IMPL_ER21) THE SAVE POINT EXISTS: drawn, heals, offers Save. False for a grace the game hides.
    shown: bool,
    told: Option<(Presence, bool)>,
    /// (IMPL_ER21) The game offered its own prompt ("Touch grace" / "Rest") while he stood in this point's capsule on
    /// this visit: the game lets him rest here now. Seconds in the capsule without it; the refusal was sounded.
    allowed: bool,
    waited: f32,
    refused_told: bool,
    /// (IMPL_ER26) The unlock rule's state for this save point.
    unlock: Unlock,
    /// (IMPL_ER52) The effects' height relative to the root as applied (None = not traced yet), when the next trace is
    /// due (our clock), the `grace:` lines said about it.
    ground: Option<f32>,
    ground_next: f32,
    ground_said: u8,
    /// (LDI) the per-point load-in rules (savepoint_rules.rs `Arrival`): taken as where he stands at its first step, shown
    /// at once as the grace he arrives at, its idle effects warmed at their next spawn
    arrival: crate::savepoint_rules::Arrival,
}

/// (IMPL_ER21) Seconds a new save point waits for its grace's map object to be found before the grace is taken as
/// present all the same (the search is hurried meanwhile: a whole round takes a dozen frames).
const UNKNOWN_WAIT: f32 = 3.0;
/// (IMPL_ER21) Seconds in the capsule, needing recovery, without the game's prompt before the refusal is sounded.
const REFUSE_AFTER: f32 = 0.75;

/// (IMPL_ER21) No other grace of the game's table belongs to the same tile / block as `entity`.
fn alone_in_its_home(entity: u32) -> bool {
    // SAFETY: game thread; the singleton lives as long as the game.
    let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return false };
    let home = crate::graceflame::home_of_entity(entity);
    repo.bonfire_warps.iter().take(8192).filter(|e| crate::graceflame::home_of_entity(e.bonfire_entity_id) == home).count() == 1
}

/// What the menu asked of the game and how far it is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Flow {
    Idle,
    /// The game's rest: stage 0 = the press is still to make, 1 = waiting for him to sit, 2 = the
    /// game's grace menu is being left again (`close`).
    Rest { close: bool, stage: u8, since: f32, tries: u8 },
    Map { since: f32 },
    /// (IMPL_ER26) Pass Time / Respawn Enemies: the game's time transition. `asked`: the request was made;
    /// `seen`: the game took it up (its "requested" switch was seen on).
    World { what: WorldAsk, since: f32, asked: bool, seen: bool },
}

/// (IMPL_ER26) What is asked of the game's time-transition control.
#[derive(Clone, Copy, Debug, PartialEq)]
enum WorldAsk {
    PassTime(TimeChoice),
    Respawn,
}

/// (IMPL_ER33) A rest made for a story event (story.rs): the row (`None` = the menu's "Talk to Melina"), the save point,
/// the cover's stage.
#[derive(Clone, Copy, Debug, PartialEq)]
struct StoryRest {
    rule: Option<usize>,
    /// (IMPL_ER36) The row of kind `self` this rest is for (not offered again in the session when she does not speak).
    own: Option<usize>,
    grace: u32,
    /// The whole rest, driven (talkdrive.rs).
    drive: Drive,
    cover: bool,
    /// The popup's talk id while the rest was being asked for (a 3000 left over from an earlier talk is no new talk),
    /// and whether flag 4652 was seen off since (one that is on from the start is no new talk either).
    id_before: i32,
    talk_flag_off: bool,
    /// The popup's talk id was seen other than Melina's since the body rests (her id staying from an earlier talk is
    /// no new talk: MEASURED, it stays 3000 after her talk until the next window).
    id_left: bool,
    presses: u8,
}

/// The game's rest state (its Lua event flags 4079 / 4083 / 4084 and the bonfire of the rest).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Rest {
    flags: u32,
    bonfire: u32,
}

pub struct ErSavePoints {
    /// (IMPL_ER57) the save points hide in combat (guards.rs `CombatHide`; `savepoint.hide_in_combat`).
    combat: crate::guards::CombatHide,
    /// (IMPL_ER19) sheet row `savepoint.sfx` (the key of that name overrides it).
    sfx: u8,
    data: SavePointData,
    points: Vec<Point>,
    on: bool,
    /// The point whose reactor he stands in (index).
    at: Option<usize>,
    flow: Flow,
    y_before: bool,
    rest: Rest,
    resting: bool,
    lua_shot: OneShot,
    seconds: f32,
    check_in: f32,
    healed: i64,
    healing: bool,
    glow: String,
    told_on: Option<bool>,
    /// Y is the save point's this frame (offered and no flow running).
    offered: bool,
    /// (savegate) The game was asked to save (`save.points_only`): Sora's own file and the notice follow its write.
    saving: bool,
    /// c1000 characters looked at already: (handle, the grace's entity id and discovery flag when it is one).
    known: Vec<(u64, Option<(u32, u32)>)>,
    /// (IMPL_ER21) `savepoint.rest_rule` as read this frame.
    rest_rule: bool,
    /// (IMPL_ER26) `savepoint.sit` / the unlock being on, as read this frame.
    sit: bool,
    autounlock: bool,
    /// The save point a Save was chosen at (entity), until the Save screen wrote or the menu went; the one whose Save
    /// was written and is still to be followed (the rest for story events, `savepoint.respawn_on_save`).
    save_at: Option<u32>,
    saved_at: Option<u32>,
    save_status: Option<crate::slots::SaveStatus>,
    /// Save points the game's real rest was made at in this session (entities).
    rested: Vec<u32>,
    /// The Roundtable Hold's grace flags (BonfireWarpParam rows of map 11_10), looked up once.
    roundtable: Option<Vec<u32>>,
    /// The save point (entity) the rest under way is made at for the story events: remembered once the game rested.
    rest_for: Option<u32>,
    /// A world reset was asked for: the orbs on the field are the host's to clear (sora.rs takes it).
    world_reset: bool,
    ctrl_told: Option<CtrlSnapshot>,
    /// The save point of the open menu (entity).
    menu_at: Option<u32>,
    /// (IMPL_ER33) The story events' table and what this session did about it; the rest under way for one; the open
    /// menu's entries; `savepoint.sit_pose` as read this frame.
    story: Story,
    story_rest: Option<StoryRest>,
    /// (SV) a story event waits for the game's rest to be offered here (gracelock lets it through meanwhile)
    story_probe: bool,
    menu_shown: GraceMenu,
    sit_pose: bool,
    /// (IMPL_ER36) What the open menu's "Talk to Melina" stands for; the player's own pad is kept from the game this
    /// frame (only our presses reach its menu).
    talk_offer: Option<Offer>,
    pad_mask: bool,
    /// (FX5) The body's special effects the open menu's "Talk to Melina" takes off for its rest (`Story::talk_offer_lifting`), and
    /// the blocked rows last told (the log).
    talk_lift: Vec<i32>,
    held_told: Vec<(String, Vec<i32>)>,
    /// (FX5) The special effects the running driven rest took off the body (given back when it ends).
    rest_lifted: Vec<i32>,
    /// (CV3 round 4) the leader's fight and life (sora.rs `set_leader`; None: Sora's own)
    leader: Option<(bool, bool)>,
    /// (IMPL_ER52) Sheet rows `savepoint.ground_*`; the sheet's `collision.floor_layer` (Sora's second cast).
    ground: Ground,
    floor_layer: u32,
    /// (WALLS) the sheet's `collision.layer`: the layer of Sora's own casts (the Tarnished's, 2)
    cast_layer: u32,
    /// (SP2) the save points' sounds of this step, for the host to play now (sora.rs `take_sounds`): the entry sound must not
    /// wait for Sora's next step (he is not stepped while out of the party).
    sounds: Vec<kh_core::SoundPlay>,
    /// (LDI) The load-in window (loadin.rs) as sora.rs gave it this frame, the frame before, and its numbers (`loadin.warm`
    /// s, `loadin.arrive` m), the leader's feet (Elden Ring). See `load_in`.
    load_in: crate::savepoint_rules::LoadInPoint,
}

/// The discovery flag of a grace by its entity id, when BonfireWarpParam has a row for it.
fn grace_flag(entity: u32) -> Option<u32> {
    // SAFETY: game thread; the singleton lives as long as the game.
    let repo = unsafe { SoloParamRepository::instance() }.ok()?;
    repo.get_by_bonfire_warp_param_by_entity_id(entity).map(|row| row.eventflag_id()).filter(|f| *f != 0)
}

/// A site of grace of the game's table (BonfireWarpParam) near the player (THE MAP ICON'S PLACE: testing aid `dbg.lua`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Grace {
    entity: u32,
    /// The event flag of its discovery.
    flag: u32,
    /// Where the row says it is, in the physics space of this frame.
    er_pos: Vec3,
}

/// THE SITES OF GRACE WITHIN 60 m, from the game's own table: every BonfireWarpParam row of the
/// player's map area whose block is his or a neighbour (the open world's 256 m tiles) or his
/// own block (elsewhere); a row's place is block-local, the player's block-local place
/// (`PlayerIns::block_position`) ties it to the physics space.
fn graces_within(player: &PlayerIns, reach: f32) -> Vec<Grace> {
    let mut out = Vec::new();
    // SAFETY: game thread; the singleton lives as long as the game.
    let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return out };
    let block = player.current_block_id;
    let (area, gx, gz) = (block.area(), block.block() as i32, block.region() as i32);
    let here = Vec3::new(player.block_position.x, player.block_position.y, player.block_position.z);
    let physics = crate::game::v3(&player.chr_ins.modules.physics.position);
    if !here.is_finite() || !physics.is_finite() {
        return out;
    }
    for entry in repo.bonfire_warps.iter().take(8192) {
        let Some(row) = repo.get_by_bonfire_warp_param_by_entity_id(entry.bonfire_entity_id) else { continue };
        if row.area_no() != area || row.eventflag_id() == 0 {
            continue;
        }
        let (dx, dz) = (row.grid_x_no() as i32 - gx, row.grid_z_no() as i32 - gz);
        if (block.is_overworld() && (dx.abs() > 1 || dz.abs() > 1)) || (!block.is_overworld() && (dx != 0 || dz != 0)) {
            continue;
        }
        let local = Vec3::new(row.pos_x() + 256.0 * dx as f32, row.pos_y(), row.pos_z() + 256.0 * dz as f32);
        let er_pos = physics + (local - here);
        if er_pos.is_finite() && (er_pos - physics).length() < reach {
            out.push(Grace { entity: entry.bonfire_entity_id, flag: row.eventflag_id(), er_pos });
        }
    }
    out
}

fn flag_set(flag: u32) -> Option<bool> {
    // SAFETY: game thread; the singleton lives as long as the game; the flag blocks are the game's own tables.
    let man = unsafe { CSEventFlagMan::instance() }.ok()?;
    if man.virtual_memory_flag.flag_blocks.is_null() {
        return None;
    }
    Some(man.virtual_memory_flag.get_flag(flag))
}

/// (IMPL_ER26) THE DISCOVERY FLAG OF A GRACE IS SET (what the game's "Touch grace" sets; the map and the travel read it).
/// False: no event flag manager. The caller reads it back.
/// (FX5) The body carries special effect `id` now (its first 128 entries, as the story diag reads them).
fn body_has(player: &PlayerIns, id: i32) -> bool {
    player.chr_ins.special_effect.entries().take(128).any(|e| e.param_id == id)
}

/// (FX5) Takes the special effects `ids` off the body (`back` false) or gives them back (`back` true) through the game's
/// own functions (the SDK's `ChrInsExt::remove_speffect` / `apply_speffect`, its 2.7.1.0 RVAs 0x3ee2e0 / 0x3e8dc0; the
/// apply RVA is the one status.rs finds by pattern). Taking off: only those on the body. Returns the ids acted on.
/// UNTESTED IN THE GAME.
fn lift_effects(ids: &[i32], back: bool) -> Vec<i32> {
    use eldenring::cs::ChrInsExt;
    let Some(player) = crate::game::player_mut() else { return Vec::new() };
    let mut done = Vec::new();
    for &id in ids {
        if back {
            player.chr_ins.apply_speffect(id, false);
            done.push(id);
        } else if body_has(player, id) {
            player.chr_ins.remove_speffect(id);
            done.push(id);
        }
    }
    done
}

/// (FX5) For the log: what the game's SpEffectParam rows of `ids` say (duration, state info), to tell what keeps her silent.
fn effect_facts(ids: &[i32]) -> String {
    let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return String::new() };
    let rows: Vec<String> = ids
        .iter()
        .map(|id| match repo.get::<eldenring::cs::SpEffectParam>(*id as u32) {
            Some(r) => format!("{id}: duration {} s, state info {}", r.effect_endurance(), r.state_info()),
            None => format!("{id}: no row"),
        })
        .collect();
    format!(" | their rows: {}", rows.join("; "))
}

fn set_flag(flag: u32) -> bool {
    if flag == 0 {
        return false;
    }
    // SAFETY: game thread; the singleton lives as long as the game; the flag blocks are the game's own tables (the
    // SDK's own writer: it looks the flag's block up and does nothing when there is none).
    let Ok(man) = (unsafe { CSEventFlagMan::instance_mut() }) else { return false };
    if man.virtual_memory_flag.flag_blocks.is_null() {
        return false;
    }
    man.virtual_memory_flag.set_flag(flag, true);
    true
}

/// (IMPL_ER26) The game's time-transition control (its rest's world reset and its "Pass time" go through it).
fn world_ctrl() -> Option<&'static mut CSEventWorldAreaTimeCtrl> {
    // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked.
    let man = unsafe { CSEventManImp::instance_mut() }.ok()?;
    if !plausible(word!(man.world_area_time)) {
        return None;
    }
    Some(&mut *man.world_area_time)
}

/// Its fields as plain numbers (the booleans as bytes: a wrong layout must not make an invalid `bool`).
fn snapshot(c: &CSEventWorldAreaTimeCtrl) -> CtrlSnapshot {
    macro_rules! byte {
        ($f:ident) => {
            // SAFETY: one byte of a live object.
            unsafe { std::ptr::read(std::ptr::addr_of!(c.$f) as *const u8) }
        };
    }
    CtrlSnapshot {
        target: [c.target_hours, c.target_minutes, c.target_seconds],
        bools: [byte!(fade_transition), byte!(reset_world), byte!(reset_main_character), byte!(reset_magic_charges), byte!(restore_estus), byte!(show_clock), byte!(fade_out_requested), byte!(respawn_wait_flag)],
        floats: [c.black_screen_time, c.clock_startup_delay_s, c.clock_move_time_s, c.clock_finish_delay_s, c.fade_out_time, c.fade_in_time, c.update_elapsed_time, c.black_screen_elapsed_time, c.total_elapsed_time, c.black_screen_timeout],
    }
}

/// The clock time of a Pass Time choice (`savepoint.passtime.morning` / `.noon` / `.nightfall` over the defaults).
fn clock_of(control: &Control, choice: TimeChoice) -> (u32, u32) {
    control.str(choice.key()).and_then(kh_host::savepoint_menu::parse_clock).unwrap_or(choice.clock())
}

/// The game's rest state now.
fn rest_now() -> Option<Rest> {
    // SAFETY: game thread; the singleton lives as long as the game; the pointers are checked.
    let man = unsafe { CSLuaEventManImp::instance() }.ok()?;
    if !plausible(word!(man.lua_event_proxy)) {
        return None;
    }
    // SAFETY: a u32 bit field inside the live proxy object.
    let flags = unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(man.lua_event_proxy.control_flags) as *const u32) };
    let bonfire = if plausible(word!(man.lua_event_script_imitation)) { man.lua_event_script_imitation.as_ref().map_or(0, |i| i.bonfire_entity_id) } else { 0 };
    Some(Rest { flags, bonfire })
}

/// Bits 5 (sit-down started), 7 (the sitting loop), 8 (standing up) of the game's Lua control flags.
const SIT: u32 = 1 << 5;
const LOOP: u32 = 1 << 7;
const STAND: u32 = 1 << 8;

impl ErSavePoints {
    pub fn new(pack: &std::path::Path, control: &Control, sfx: u8) -> ErSavePoints {
        ErSavePoints { combat: crate::guards::CombatHide::default(), sfx, data: SavePointData::load(pack), points: Vec::new(), on: false, at: None, flow: Flow::Idle, y_before: false, rest: Rest::default(), resting: false, lua_shot: OneShot::new("dbg.lua", control), seconds: 0.0, check_in: 0.0, healed: 0, healing: false, glow: String::new(), told_on: None, offered: false, saving: false, known: Vec::new(), rest_rule: true, sit: false, autounlock: true, save_at: None, saved_at: None, save_status: None, rested: Vec::new(), roundtable: None, rest_for: None, world_reset: false, ctrl_told: None, menu_at: None, story: Story::new(crate::story::default_rules()), story_rest: None, story_probe: false, menu_shown: GraceMenu::DEFAULT, sit_pose: false, talk_offer: None, pad_mask: false, ground: Ground::DEFAULT, floor_layer: 0, cast_layer: 2 /* (WALLS) */, talk_lift: Vec::new(), held_told: Vec::new(), rest_lifted: Vec::new(), leader: None /* (CV3) */, sounds: Vec::new() /* (SP2) */, load_in: Default::default() /* (LDI) */ }
    }

    /// (IMPL_ER52) The sheet's rows `savepoint.ground_*` (sora.rs passes ermap's `Params::ground`).
    pub fn with_ground(mut self, ground: Ground, floor_layer: u32) -> ErSavePoints {
        self.ground = ground;
        self.floor_layer = floor_layer;
        self
    }

    /// (WALLS) The sheet's `collision.layer` (sora.rs passes ermap's `Params::collision_layer`): the ground trace casts as Sora does.
    pub fn with_layer(mut self, layer: u32) -> ErSavePoints {
        self.cast_layer = layer;
        self
    }

    /// (LDI) THE LOAD-IN WINDOW (loadin.rs), once a frame before `frame`: in it the save points made or shown appear settled
    /// (savepoint_rules.rs `Arrival`): taken as where he stands (no entry burst / sound), the grace he arrives at standing
    /// at once, the idle effects warmed. `leader`: the leader's feet (Elden Ring). Keys `loadin`, `loadin.warm`,
    /// `loadin.arrive`.
    pub fn load_in(&mut self, control: &Control, window: bool, leader: Vec3) {
        let on = control.flag("loadin") != Some(false);
        let window = window && on;
        let li = &mut self.load_in;
        li.was = li.window;
        li.window = window;
        li.warm = if on { control.f32("loadin.warm").filter(|v| v.is_finite()).map_or(3.0, |v| v.clamp(0.0, 10.0)) } else { 0.0 };
        li.arrive = if on { control.f32("loadin.arrive").filter(|v| v.is_finite()).map_or(4.0, |v| v.clamp(0.0, 50.0)) } else { 0.0 };
        li.leader = leader;
        if window && !li.was {
            // (a warp in the same host: the save points already there may now be around him)
            for p in &mut self.points {
                p.arrival.settle = true;
            }
        }
    }

    /// (IMPL_ER36, sora.rs asks) A story rest is under way and the body rests: the pad is the game's (Melina's talk,
    /// her choice boxes), Sora gets none.
    pub fn holds_pad(&self) -> bool {
        self.story_rest.is_some_and(|sr| sr.drive.holds_pad())
    }

    /// (IMPL_ER36, sora.rs asks) Under the cover the game's menu is being worked for the player: his own pad is kept
    /// from the game (a stick held down would move the cursor we count on).
    pub fn masks_pad(&self) -> bool {
        self.pad_mask
    }

    /// (IMPL_ER33) The story table from the sheet beside the loadout (the compiled-in rows = the sheet as it was at the
    /// build, when it cannot be read).
    pub fn with_story(mut self, sheet: &std::path::Path) -> ErSavePoints {
        let count = |rules: &[crate::story::Rule]| (rules.iter().filter(|r| r.kind == crate::story::Kind::Auto).count(), rules.iter().filter(|r| r.kind == crate::story::Kind::Menu).count());
        match std::fs::read_to_string(sheet).map_err(|e| e.to_string()).and_then(|text| crate::story::parse(&text)) {
            Ok((rules, problems)) if !rules.is_empty() => {
                let (auto, menu) = count(&rules);
                erlog!("story: the table {}: {auto} automatic row(s) [{}], {menu} menu row(s){}", sheet.display(), rules.iter().filter(|r| r.kind == crate::story::Kind::Auto).map(|r| r.id.as_str()).collect::<Vec<_>>().join(", "), if problems.is_empty() { String::new() } else { format!("; NOT UNDERSTOOD: {problems:?}") });
                self.story = Story::new(rules);
            }
            other => {
                let (auto, menu) = count(&self.story.rules);
                erlog!("story: the table {} was not read ({}): the rows compiled in are used ({auto} automatic, {menu} menu)", sheet.display(), match other { Err(e) => e, Ok(_) => "no rows".to_string() });
            }
        }
        self
    }

    /// (IMPL_ER33, sora.rs asks) Sora does not take the body's pose while it attunes to or rests at a site of grace.
    pub fn hides_grace_pose(&self) -> bool {
        self.on && !self.sit && !self.sit_pose
    }

    /// (CV3 round 4, sora.rs tells before `frame`) THE LEADER'S FIGHT AND LIFE, whoever leads (conv.rs
    /// `leader_facts`): Sora out of the party is not stepped and his battle flag / HP are frozen (the user's FINAL5
    /// session: no Save offered after Donald's unlock, Goofy leading). `frame`'s `on_floor` is the leader's too.
    pub fn set_leader(&mut self, battle: bool, alive: bool) {
        self.leader = Some((battle, alive));
    }

    /// He rests at a site of grace (the game's own rest: sitting, or its grace menu up).
    pub fn resting(&self) -> bool {
        self.resting
    }

    /// (SP2) The save points' sounds asked for since the last call (sora.rs plays them through the effect side's audio
    /// queue in the same step, so that a friend leading with Sora out of the party hears them too).
    pub fn take_sounds(&mut self) -> Vec<kh_core::SoundPlay> {
        std::mem::take(&mut self.sounds)
    }

    /// The menu's command is being carried out by the game: the pad is the game's meanwhile.
    pub fn busy(&self) -> bool {
        self.flow != Flow::Idle || self.story_rest.is_some()
    }

    /// ONCE PER GAME FRAME (`ChrIns_PrePhysics`, after the targets were read and Elden Ring's
    /// interface was looked at). `y_held`: the pad's Y as the player holds it (before any masking).
    /// Returns true when Y belongs to the save point this frame (hidden from Elden Ring and from Sora).
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, control: &Control, player: &PlayerIns, targets: &ErTargets, frame: Frame, sora: &mut Sora, on_floor: bool, prompt: bool, ui: ErUi, y_held: bool, lb_held: bool, dt: f32) -> bool {
        let dt = dt.clamp(0.0, 0.1);
        self.seconds += dt;
        self.on = control.on("savepoint.kh");
        if self.told_on != Some(self.on) {
            self.told_on = Some(self.on);
            erlog!("savepoint: KH3 save points at the sites of grace are {}", if self.on { "ON (savepoint.kh)" } else { "off" });
        }
        // ---- the game's rest state (also without the save points: the wallet asks)
        if let Some(rest) = rest_now() {
            if rest != self.rest {
                erlog!("savepoint: the game's rest state: Lua control flags {:#x} -> {:#x} (sit {} loop {} stand {}), bonfire entity {} -> {}", self.rest.flags, rest.flags, rest.flags >> 5 & 1, rest.flags >> 7 & 1, rest.flags >> 8 & 1, self.rest.bonfire, rest.bonfire);
                self.rest = rest;
            }
        }
        // (the game's own flags: MEASURED in the game, IMPL_ER9_STATE.md; the map also reads hud_state 0 at a grace,
        // so the interface state cannot stand in for them)
        // MEASURED: 0 before the session's first rest; 0x20 when he sits down, 0xa0 while he sits (the grace menu),
        // 0x180 from the moment he stands up, AND IT STAYS 0x180 afterwards (until the next rest starts with 0x20).
        let resting = self.rest.flags & SIT != 0 || (self.rest.flags & LOOP != 0 && self.rest.flags & STAND == 0);
        if resting != self.resting {
            erlog!("savepoint: he {} at a site of grace (the game's rest){}", if resting { "RESTS" } else { "no longer rests" }, if resting { ": the wallet shows the game 0 runes meanwhile" } else { "" });
            self.resting = resting;
            // (IMPL_ER26) a rest nobody of ours asked for although Sora is not to sit
            if resting && !matches!(self.flow, Flow::Rest { .. }) && !control.on("savepoint.sit") && self.on {
                erlog!("!!! savepoint: THE GAME'S OWN REST STARTED WITHOUT US (savepoint.sit = 0): its prompt was answered outside the guard radius (savepoint.guard, m; the nearest save point is {:.2} m from him) or by another route; its grace menu is the player's (B leaves it)", { let at = crate::game::v3(&player.chr_ins.modules.physics.position); self.points.iter().map(|p| (p.er_pos - at).length()).fold(f32::INFINITY, f32::min) });
            }
        }
        if self.lua_shot.fired(control).is_some() {
            self.dump_lua();
            // (IMPL_ER17) the grace characters themselves: their special effects and effect modules (the flame's owner is looked for)
            if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
                for p in &self.points {
                    if let Some(chr) = p.handle.as_ref().and_then(|h| wcm.chr_ins_by_handle_mut(h)) {
                        if !crate::game::chr_complete(chr) {
                            continue;
                        }
                        let modules = word!(chr.modules);
                        let effects: Vec<i32> = chr.special_effect.entries().take(64).map(|e| e.param_id).collect();
                        let ta = &chr.modules.time_act;
                        if let Ok(repo) = unsafe { SoloParamRepository::instance() } {
                            for id in &effects {
                                let Some(row) = repo.get::<eldenring::cs::SpEffectParam>(*id as u32) else { continue };
                                let vfx = [row.vfx_id(), row.vfx_id1(), row.vfx_id2(), row.vfx_id3(), row.vfx_id4(), row.vfx_id5(), row.vfx_id6(), row.vfx_id7()];
                                let rows: Vec<String> = vfx.iter().filter(|v| **v > 0).map(|v| match repo.get::<eldenring::cs::SpEffectVfxParam>(*v as u32) {
                                    Some(r) => { let w = crate::probe::floats::<1>(r as *const _ as usize).map(|_| ()); let _ = w; let raw: [i32; 6] = unsafe { std::ptr::read_unaligned(r as *const _ as *const [i32; 6]) }; format!("{v}: midst sfx {} init sfx {} finish sfx {} (row at {:#x})", raw[0], raw[2], raw[4], r as *const _ as usize) }
                                    None => format!("{v}: no row"),
                                }).collect();
                                erlog!("lua:   special effect {id}: vfx {rows:?}");
                            }
                        }
                        erlog!("lua: grace {} chr {:#x} modules {modules:#x} sfx module {:#x} vfx module {:#x} special effects {effects:?} animation {} model {:#x}{}", p.entity, chr as *const _ as usize, crate::probe::word(modules + 0xB0).unwrap_or(0), crate::probe::word(modules + 0xB8).unwrap_or(0), ta.anim_queue[(ta.read_idx as usize) % 10].anim_id, word!(chr.chr_model_ins), crate::probe::model_facts(word!(chr.chr_model_ins)));
                    }
                }
            }
            let here = crate::game::v3(&player.chr_ins.modules.physics.position);
            for g in graces_within(player, 600.0) {
                let d = g.er_pos - here;
                erlog!("lua: grace {} (flag {}: {:?}) is ({:+.1}, {:+.1}, {:+.1}) m from him in Elden Ring's axes ({:.0} m)", g.entity, g.flag, flag_set(g.flag), d.x, d.y, d.z, d.length());
            }
        }
        if !self.on {
            self.release(true);
            self.offered = false;
            PROMPT.store(false, std::sync::atomic::Ordering::Relaxed); // (SP2)
            self.y_before = y_held;
            return false;
        }
        // ---- the graces in reach: one save point each
        let height = control.f32("savepoint.height").unwrap_or(92.0);
        for p in &mut self.points {
            p.unseen += dt;
        }
        let (load_window, load_warm) = (self.load_in.window, self.load_in.warm); // (LDI)
        // A GRACE = a c1000 character whose entity id + 1000 is a row of BonfireWarpParam (MEASURED: The First Step's
        // character is 1042360951 and its grace 1042361951, the Church of Elleh's 1042360950 / 1042361950; c1000 is also
        // the game's generic invisible helper, with entity ids of no row). The row's own position is the MAP ICON's (a
        // cave's row lies in the open world), so the character is the place.
        for e in targets.list.iter().filter(|e| e.npc_id == NPC_GRACE) {
            let key = crate::game::handle_bits(&e.handle);
            let known = match self.known.iter().find(|k| k.0 == key) {
                Some(k) => k.1,
                None => {
                    let entity = targets.chr_mut(e.id).map_or(0, |chr| chr.event_entity_id);
                    let grace = [entity.wrapping_add(1000), entity].into_iter().filter(|id| *id != 0).find_map(|id| grace_flag(id).map(|flag| (id, flag)));
                    if self.known.len() < 4096 {
                        self.known.push((key, grace));
                    }
                    grace
                }
            };
            let Some((entity, flag)) = known else { continue };
            let sunk = self.glow == "sink";
            let er_pos = if sunk { self.points.iter().find(|p| p.entity == entity).map_or(e.er_pos, |p| p.er_pos) } else { e.er_pos };
            let root = frame.pos_er_to_kh(er_pos) + Vec3::Z * height;
            match self.points.iter_mut().find(|p| p.entity == entity) {
                Some(p) => {
                    p.unseen = 0.0;
                    p.er_pos = er_pos;
                    p.handle = Some(e.handle);
                    if (p.sp.root - root).length() > 3.0 {
                        // (the frame moved under him: a testing warp; the height key)
                        // (LDI) a save point that stood is made again where it now is in its idle state, not from nothing
                        if p.shown && load_warm > 0.0 {
                            p.sp.warm = load_warm;
                        }
                        p.sp.active = false;
                        crate::fx::with_effects(|fx| p.sp.show(fx));
                        p.sp.root = root;
                        p.sp.active = true;
                    }
                }
                None => {
                    let discovered = flag_set(flag);
                    erlog!("savepoint: a site of grace (entity {entity}, discovery flag {flag}: {}) at Elden Ring ({:.2}, {:.2}, {:.2}): a KH3 save point stands there", match discovered { Some(true) => "DISCOVERED", Some(false) => "not discovered yet", None => "unknown" }, er_pos.x, er_pos.y, er_pos.z);
                    let mut sp = SavePoint::new(self.data.clone(), root);
                    // (IMPL_ER19) KH3 keeps "accessed" per save point: its 3 s first-visit burst plays once. The save points
                    // here are made anew whenever a grace comes into reach: a DISCOVERED grace is one he was at.
                    if discovered == Some(true) && !control.on("savepoint.first") {
                        sp.set_accessed();
                    }
                    // (IMPL_ER21) it exists for the player only when the game shows its grace: decided below, every frame
                    sp.active = false;
                    // (IMPL_ER26) the unlock happens in the reactor: the first-visit burst (KH3's own feedback) starts there too
                    sp.first_in_reactor = control.flag("savepoint.autounlock") != Some(false) && !control.on("savepoint.sit");
                    // (SP2) its sounds are played by the host at once (whoever leads), not queued on Sora
                    sp.host_sounds = Some(Vec::new());
                    let alone = alone_in_its_home(entity);
                    self.points.push(Point { handle: Some(e.handle), entity, flag, discovered, er_pos, sp, unseen: 0.0, step: SavePointStep::default(), presence: Presence::Unknown, since: self.seconds, rounds_at: u32::MAX, alone, shown: false, told: None, allowed: false, waited: 0.0, refused_told: false, unlock: Unlock::default(), ground: None, ground_next: 0.0, ground_said: 0, arrival: crate::savepoint_rules::Arrival::made(load_window) /* (LDI) */ });
                }
            }
        }
        // ---- (IMPL_ER52) THE SAVE POINT ON THE GROUND: KH3's trace down from the root (kh_host::savepoint::ground_offset)
        self.ground_phase(control, frame);
        // ---- (IMPL_ER21) HIDDEN GRACES ARE HIDDEN SAVE POINTS: the game's own state of each grace (graceflame.rs: its map
        // object AEG099_060 plays its flame 1010 / 1011, or nothing: a boss's grace before the boss is defeated, one an
        // event has not enabled yet). A grace whose object is not found within `UNKNOWN_WAIT` seconds and a whole search
        // round is taken as present (as before this rule). `savepoint.hidden = 0`: every grace has its save point.
        let rule = control.flag("savepoint.hidden") != Some(false);
        let block: i32 = player.current_block_id.into();
        let physics = crate::game::v3(&player.chr_ins.modules.physics.position);
        let anchor = (block as u32, [player.block_position.x, player.block_position.y, player.block_position.z], [physics.x, physics.y, physics.z]);
        for p in &mut self.points {
            let (presence, rounds) = crate::graceflame::presence(p.entity, [p.er_pos.x, p.er_pos.y, p.er_pos.z], anchor, p.alone);
            if p.rounds_at == u32::MAX {
                p.rounds_at = rounds;
            }
            p.presence = presence;
            let waited = self.seconds - p.since >= UNKNOWN_WAIT && rounds >= p.rounds_at.saturating_add(2);
            if presence == Presence::Unknown && !waited && rule {
                crate::graceflame::hurry();
            }
            let shown = !rule || presence == Presence::Present || (presence == Presence::Unknown && waited);
            if p.told != Some((presence, shown)) {
                erlog!(
                    "savepoint: grace {} ({}): {} -> {}",
                    p.entity,
                    match p.discovered { Some(true) => "discovered", Some(false) => "not discovered", None => "discovery unknown" },
                    match presence {
                        Presence::Present => "THE GAME SHOWS IT (its map object plays its flame)".to_string(),
                        Presence::Hidden => "THE GAME HIDES IT (its map object plays no flame: a boss's grace before the boss is defeated, or one an event has not enabled)".to_string(),
                        Presence::Unknown => format!("its map object is NOT KNOWN ({:.1} s, {} search round(s) since it came into reach; {})", self.seconds - p.since, rounds.saturating_sub(p.rounds_at), if p.alone { "alone in its tile / block" } else { "other graces share its tile / block" }),
                    },
                    if !rule { "the save point stands (savepoint.hidden = 0)" } else if shown && presence == Presence::Unknown { "TAKEN AS PRESENT: the save point stands" } else if shown { "the save point STANDS: drawn, heals, offers Save" } else if presence == Presence::Unknown { "no save point yet (its object is looked for)" } else { "A HIDDEN SAVE POINT: nothing drawn, no heal, no prompt, no Save" }
                );
                p.told = Some((presence, shown));
            }
            // (LDI) the grace he arrives at with a load stands at once (savepoint_rules.rs `Arrival`)
            let here = rule && self.load_in.arrive > 0.0 && p.discovered == Some(true) && (p.er_pos - self.load_in.leader).length() <= self.load_in.arrive;
            let standing = p.arrival.standing;
            p.shown = p.arrival.shown(shown, presence == Presence::Present, presence == Presence::Hidden, here, load_window, dt);
            if p.arrival.standing != standing {
                erlog!("savepoint: grace {}: {}", p.entity, if p.arrival.standing { format!("(LDI) THE GRACE HE ARRIVES AT WITH THE LOAD ({:.1} m from him, discovered): its save point stands at once, before the game's flame is seen", (p.er_pos - self.load_in.leader).length()) } else if presence == Presence::Present { "(LDI) the game's flame is seen: the arrival rule is done".to_string() } else { format!("(LDI) the arrival rule is over ({}): the game's word holds again", if here { "the flame never came" } else { "he left it" }) });
            }
        }
        // ---- (IMPL_ER57, the user: "Save points need to hide while in combat") IN COMBAT the save points are not there: their
        // effects stop emitting and run out (KH3's own Deactivate: the fade), no heal, no unlock, no Save offered; they come
        // back `savepoint.combat_hold` seconds (default 3) after the fight. COMBAT = SORA'S BATTLE FLAG (KH3's battle mode,
        // fed by the host's engaged count: a hostile, living, active, drawn enemy within the sheet's engage_range, 8 m, or
        // one that hit him / that he hit in the last 5 s; + KH3's 1 s linger). Never while the menu or one of its commands
        // is in progress. `savepoint.hide_in_combat = 0`: as before.
        {
            let leader_battle = self.leader.map_or(sora.actions().battle, |l| l.0); // (CV3) the leader's fight
            // (LDI) not in the load-in window: a battle flag left from before the load does not take the save point he arrives
            // at away under the fade-in (fix6's log: "IN COMBAT" in the new host's first frame after a game over)
            let battle = control.flag("savepoint.hide_in_combat") != Some(false) && leader_battle && !self.busy() && !crate::hud::menu_open() && !load_window;
            let hold = control.f32("savepoint.combat_hold").filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(3.0);
            let before = self.combat.hidden;
            let hidden = self.combat.frame(battle, hold, dt);
            if hidden != before && !self.points.is_empty() {
                erlog!("savepoint: {}", if hidden { format!("IN COMBAT (Sora's battle flag is up): the {} save point(s) in reach FADE OUT and offer nothing until {hold:.1} s after the fight", self.points.len()) } else { "the fight is over: the save points are back".to_string() });
            }
            if hidden {
                for p in &mut self.points {
                    p.shown = false;
                }
            }
        }
        // (LDI) the frame's final word: a save point that comes to be shown in the window, or that was always there, is drawn
        // in its idle state at once (its effects warmed); one that appears in play or comes back after a fight starts as KH3's
        for p in &mut self.points {
            match p.arrival.edge(p.shown, load_window) {
                Some(true) if load_warm > 0.0 => p.sp.warm = load_warm,
                Some(_) => p.sp.warm = 0.0,
                None if !p.shown => p.sp.warm = 0.0,
                None => {}
            }
        }
        // (gone from the list for two seconds: out of reach or unloaded)
        let mut k = 0;
        while k < self.points.len() {
            if self.points[k].unseen > 2.0 {
                let mut p = self.points.remove(k);
                p.sp.active = false;
                crate::fx::with_effects(|fx| p.sp.show(fx));
                erlog!("savepoint: the site of grace {} is out of reach: its save point is taken down", p.entity);
                self.at = None;
            } else {
                k += 1;
            }
        }
        // ---- discovery (the flag is the game's; looked at twice a second)
        self.check_in -= dt;
        if self.check_in <= 0.0 {
            self.check_in = 0.5;
            for p in &mut self.points {
                if p.flag != 0 {
                    let now = flag_set(p.flag);
                    if now != p.discovered {
                        erlog!("savepoint: the site of grace {} is {} (event flag {})", p.entity, if now == Some(true) { "NOW DISCOVERED: the game registered it" } else { "no longer flagged as discovered" }, p.flag);
                        p.discovered = now;
                    }
                }
            }
        }
        // ---- (IMPL_ER26) NO ATTUNING: running into a save point the game presents unlocks it (savepoint_rules::Unlock)
        self.sit = control.on("savepoint.sit");
        self.autounlock = control.flag("savepoint.autounlock") != Some(false) && !self.sit;
        for p in &mut self.points {
            // (presented = its flame was SEEN; a grace merely "taken as present" is not unlocked blind. With
            // `savepoint.hidden = 0` every grace counts, as that key says.)
            // A grace whose map object was never matched ("taken as present") counts once the game itself offers a prompt
            // while he stands in its capsule: else, with Y kept from the game, it could never be discovered at all.
            let presented = p.shown && (p.presence == Presence::Present || !rule || (prompt && !ui.menu));
            match p.unlock.step(self.autounlock, presented, p.discovered, p.step.in_reactor, dt) {
                UnlockStep::Nothing => {}
                UnlockStep::Write => {
                    let wrote = set_flag(p.flag);
                    let now = flag_set(p.flag);
                    erlog!(
                        "savepoint: grace {} UNLOCKED BY RUNNING INTO ITS SAVE POINT (try {}): discovery flag {} {} and reads back {}: {}",
                        p.entity,
                        p.unlock.tries(),
                        p.flag,
                        if wrote { "written" } else { "COULD NOT BE WRITTEN (no event flag manager)" },
                        match now { Some(true) => "SET", Some(false) => "NOT SET", None => "unreadable" },
                        if now == Some(true) { "the map shows it and travels to it; no \"Touch grace\", no kneel, no banner of Elden Ring's (the respawn point is NOT written: a death loads the last Save)" } else { "tried again in a second" }
                    );
                    if now == Some(true) {
                        p.discovered = now;
                        self.story.unlocked(p.entity); // (IMPL_ER33) a row this unlock fulfils is made at once
                    }
                }
                UnlockStep::GaveUp => erlog!("!!! savepoint: grace {}: its discovery flag {} did not hold after {} writes: NOT UNLOCKED (savepoint.sit = 1 gives the game's own \"Touch grace\" back)", p.entity, p.flag, Unlock::TRIES),
            }
        }
        self.glow_phase(control);
        // ---- (IMPL_ER21) THE REST RULE: the save point heals and offers Save only where the game itself would let him
        // rest NOW: its own prompt ("Touch grace" / "Rest at site of grace") came up while he stood in the capsule on
        // this visit (the game shows none in a boss fight or while enemies are alerted nearby). Kept until he leaves.
        // `savepoint.rest_rule = 0`: it heals whenever he stands in it (as before).
        // (SV) ... while gracelock.rs keeps the game's grace actions switched off its prompt never comes: "the game lets him rest
        // here" is then what the grace itself shows (the game puts out its flame in a boss fight / with enemies alerted near:
        // the save point is HIDDEN then, `shown`) together with our combat rule (IMPL_ER57): a shown save point is usable
        self.rest_rule = control.flag("savepoint.rest_rule") != Some(false) && !crate::gracelock::locked();
        for p in &mut self.points {
            // (IMPL_ER26) kept from the Save-command capsule on (the menu no longer needs the prompt at the press itself)
            if !p.shown || !(p.step.in_recover || p.step.in_reactor) {
                (p.allowed, p.waited, p.refused_told) = (false, 0.0, false);
            } else if prompt && !ui.menu {
                if !p.allowed && self.rest_rule {
                    erlog!("savepoint: grace {}: the game offers its prompt here: the save point is usable on this visit (after {:.2} s in its capsule)", p.entity, p.waited);
                }
                p.allowed = true;
            } else if !p.allowed && p.step.in_recover {
                p.waited += dt;
            }
        }
        // ---- (IMPL_ER33) IN A REAL-TIME CUTSCENE Sora's steps do not run (`after_step` neither): the save points are kept
        // drawn from here (`cut.savepoint = 0`: as before, whatever state the scene's start left them in)
        if ui.cutscene && control.flag("cut.savepoint") != Some(false) {
            let draw = control.flag("savepoint.draw") != Some(false);
            let points = &mut self.points;
            crate::fx::with_effects(|fx| {
                for p in points.iter_mut() {
                    p.sp.active = draw && p.shown;
                    p.sp.show(fx);
                }
            });
        }
        // ---- (IMPL_ER33) THE STORY EVENTS (story.rs): a row of the table met at the save point he stands in
        self.sit_pose = control.on("savepoint.sit_pose");
        {
            let here = self.at.and_then(|k| self.points.get(k)).filter(|p| p.shown && p.discovered == Some(true));
            let blocked = Blocked {
                cutscene: ui.cutscene,
                menu: ui.menu || crate::hud::menu_open(),
                busy: self.flow != Flow::Idle || self.story_rest.is_some() || self.saving || self.resting || self.saved_at.is_some(),
                no_rest: !(prompt && here.is_some_and(|p| p.allowed || !self.rest_rule)),
                in_air: !on_floor,
                dead: !self.leader.map_or(sora.status().hp > 0, |l| l.1), // (CV3) the leader's life
            };
            let ctx = Ctx { at: here.map(|p| p.entity), blocked, auto: control.flag("story.auto") != Some(false) && !self.sit };
            let verbose = control.on("story.log") && self.seconds.fract() < dt;
            self.story.set_on_y(control.flag("story.on_y") != Some(false)); // (RC) Melina's events wait for Y at the save point
            let decision = self.story.frame(ctx, &flag_set, verbose);
            // (SV) a story event waits only for the game's rest to be offered here: gracelock lets it be offered now
            if decision.waits_for_prompt != self.story_probe {
                erlog!("savepoint: (SV) {}", if decision.waits_for_prompt { "a story event is due here and waits for the game's rest: its grace actions are let through for it (the press stays ours: the story's driven rest)" } else { "no story event waits for the game's rest any more" });
            }
            self.story_probe = decision.waits_for_prompt;
            for line in &decision.log {
                erlog!("story: {line}");
            }
            if let Some((rule, grace)) = decision.rest {
                self.story_rest = Some(StoryRest { rule: Some(rule), own: None, grace, drive: Drive::new(Goal { scene: true, own: false, ups: None }), cover: control.flag("story.cover") != Some(false), id_before: 0, talk_flag_off: false, id_left: false, presses: 0 });
            }
        }
        self.story_step(control, player, ui, dt);
        // (DON-UI) B on the Party Formation screen: back to this menu (the user's rule, B = the previous menu)
        crate::hud::reopen_after_formation(self.menu_shown);
        // ---- the menu's answer
        if let Some(result) = crate::hud::take_savepoint_result() {
            // Leaving the pause refills the used auto-reload pouch slots (magic spec J 7.5).
            let refilled = sora.on_unpause();
            let action = match result {
                SavePointResult::Chosen(command) => kh_host::savepoint_menu::action_in(self.menu_shown, command),
                SavePointResult::Closed => None,
            };
            match action {
                // (IMPL_ER14) with KH3's Save screen (saveload.kh): the slot list comes first; the save is made from there
                // (no rest: the save point's own healing stays)
                Some(GraceAction::Save) if crate::hud::saveload_on() => {
                    erlog!("savepoint: SAVE chosen: KH3's Save screen is asked for (the slot is chosen there)");
                    self.save_at = self.menu_at;
                    crate::hud::ask_save_screen();
                }
                Some(GraceAction::Save) => {
                    erlog!("savepoint: SAVE chosen ({refilled} pouch slot(s) refilled; saveload.kh = 0): the game's own rest is asked for (THE ONLY WAY TO SAVE WITHOUT KH3'S SAVE SCREEN: he sits); its grace menu is left again at once");
                    self.flow = Flow::Rest { close: true, stage: 0, since: 0.0, tries: 0 };
                }
                // (SV, the user 2026-10-17: "Have a menu to load save data at save points") LOAD: KH3's Load screen; a used slot
                // asks "Load the save data in this slot?" (cursor on No); Yes = the game's own load of that slot (hud.rs)
                Some(GraceAction::Load) => {
                    erlog!("savepoint: LOAD chosen ({refilled} pouch slot(s) refilled): KH3's Load screen is asked for (the slot is chosen and confirmed there; B leads back to this menu)");
                    crate::hud::ask_load_screen();
                }
                Some(GraceAction::Map) => {
                    erlog!("savepoint: SELECT AN AREA chosen ({refilled} pouch slot(s) refilled): the game's map is asked for");
                    self.flow = Flow::Map { since: 0.0 };
                    crate::travel::from_save_point(); // (IMPL_ER23A) this map may travel
                }
                // (IMPL_ER26) PASS TIME: the choice window's button says until when
                Some(GraceAction::PassTime) => match crate::hud::take_savepoint_choice().and_then(TimeChoice::from_button) {
                    Some(choice) => {
                        erlog!("savepoint: PASS TIME chosen: until {} ({refilled} pouch slot(s) refilled): the game's time transition is asked for when the menu is off", choice.label());
                        self.flow = Flow::World { what: WorldAsk::PassTime(choice), since: 0.0, asked: false, seen: false };
                    }
                    None => erlog!("savepoint: PASS TIME chosen without a time of day: nothing is done"),
                },
                // (IMPL_ER26) RESPAWN ENEMIES: the world reset of the game's rest, without the rest
                Some(GraceAction::Respawn) => {
                    erlog!("savepoint: RESPAWN ENEMIES chosen ({refilled} pouch slot(s) refilled): the game's world reset is asked for when the menu is off; NOTHING IS SAVED, Sora's state stays");
                    self.flow = Flow::World { what: WorldAsk::Respawn, since: 0.0, asked: false, seen: false };
                }
                Some(GraceAction::Rest) => {
                    erlog!("savepoint: SITE OF GRACE chosen ({refilled} pouch slot(s) refilled; savepoint.sit = 1): the game's own rest and grace menu");
                    self.flow = Flow::Rest { close: false, stage: 0, since: 0.0, tries: 0 };
                }
                // (IMPL_ER36) TALK TO MELINA: "just talk to Melina and that's it": the driven rest (talkdrive.rs)
                Some(GraceAction::Talk) => match (self.menu_at, self.talk_offer.take()) {
                    (Some(grace), Some(offer)) => {
                        let ups = offer.entry.then(|| talkdrive::entry_ups(&flag_set, control.i64("story.menu.ups").map(|n| n.clamp(1, 20) as u8)));
                        let goal = Goal { scene: offer.scene, own: offer.own.is_some(), ups };
                        erlog!("story: TALK TO MELINA chosen at save point {grace} (row {}; {refilled} pouch slot(s) refilled): THE DRIVEN REST: black cover, the game's rest for the body, {}; the cover is off while she speaks; the game's grace menu is never shown, the rest is left for the player at the end", offer.row, match (goal.scene, goal.own, goal.ups) {
                            (true, ..) => "the event's scene is expected".to_string(),
                            (_, true, Some(n)) => format!("she is expected to speak by herself, else her entry of the game's menu is selected ({n} x up from the top, confirm)"),
                            (_, true, None) => "she is expected to speak by herself (the game's menu holds no entry of hers now)".to_string(),
                            (_, false, Some(n)) => format!("her entry of the game's menu is selected ({n} x up from the top, confirm)"),
                            (_, false, None) => "nothing is expected".to_string(),
                        });
                        // (FX5) the effects that keep her silent come off the body for this rest (given back at its end)
                        let lifted = lift_effects(&std::mem::take(&mut self.talk_lift), false);
                        if !lifted.is_empty() {
                            erlog!("story: (FX5) the body's special effects {lifted:?} are taken off for Melina's talk (the talk script tests them); they are given back when the rest ends");
                        }
                        self.story_rest = Some(StoryRest { rule: None, own: offer.own, grace, drive: Drive::new(goal), cover: control.flag("story.cover") != Some(false), id_before: 0, talk_flag_off: false, id_left: false, presses: 0 });
                        self.rest_lifted = lifted;
                    }
                    _ => erlog!("story: TALK TO MELINA chosen but the menu's save point or its offer is not known: nothing is done"),
                },
                // (party P) PARTY FORMATION: the HUD's screen (who is in the party, who leads); kept with Sora's save. UNTESTED
                // IN ELDEN RING (code and unit tests only): the Elden Ring host has no party members to spawn yet.
                Some(GraceAction::Party) => {
                    let opened = crate::hud::open_party_formation();
                    erlog!("savepoint: PARTY FORMATION chosen ({refilled} pouch slot(s) refilled): its screen {}", if opened { "opens" } else { "does not open (another menu is up)" });
                }
                // (IMPL_ER21) TITLE SCREEN (asked and confirmed in the menu): Elden Ring's title screen, nothing saved
                Some(GraceAction::Title) => {
                    let asked = crate::savegate::quit_to_title("the save-point menu's Title Screen");
                    erlog!("savepoint: TITLE SCREEN chosen and confirmed: {}", if asked { "Elden Ring's title screen is asked for; NOTHING IS SAVED (the last Save at a save point stands)" } else { "the game's title request could not be made" });
                }
                None => erlog!("savepoint: the save menu is closed ({refilled} pouch slot(s) refilled)"),
            }
        }
        self.after_save(control);
        // (savegate) the game's save asked for by Save is written: Sora's own file follows at once, then the notice
        if self.saving {
            // (IMPL_ER13) through the one slot path (slots.rs): the game's "both" save into the current slot, Sora's file
            // and the slot's marker with it
            match crate::slots::take_save_result() {
                Some(Ok(slot)) => {
                    self.saving = false;
                    crate::hud::say("item Game_saved.");
                    erlog!("savepoint: GAME SAVED (slot {slot}): Elden Ring's save and Sora's save were written together");
                }
                Some(Err((slot, why))) => {
                    self.saving = false;
                    crate::hud::say("item Could_not_save.");
                    erlog!("savepoint: THE SAVE INTO SLOT {slot} FAILED ({why})");
                }
                None => {}
            }
        }
        self.flow_step(control, sora, prompt, ui, dt);
        // ---- Y at a save point: the KH3 menu (savepoint_rules::y_rule)
        // (IMPL_ER26) the save point he stands in; else, while the game's prompt is up, the nearest one within the guard
        // radius (the game offers its action farther out than the Save-command capsule reaches)
        let guard = control.f32("savepoint.guard").filter(|g| g.is_finite()).unwrap_or(2.0).clamp(0.0, 20.0);
        let nearest = self.points.iter().enumerate().filter(|(_, p)| p.shown).map(|(k, p)| ((p.er_pos - physics).length(), k)).filter(|(d, _)| *d <= guard).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, k)| k);
        let near_any = self.points.iter().any(|p| (p.er_pos - physics).length() <= guard.max(2.0)); // (SV)
        let at = self.at.filter(|k| self.points.get(*k).is_some_and(|p| p.shown));
        let here_at = at.or(if prompt { nearest } else { None });
        let here = here_at.and_then(|k| self.points.get(k));
        // (SV) the game's grace actions are switched off (gracelock.rs) except while one of our flows needs its rest
        crate::gracelock::set_allow(matches!(self.flow, Flow::Rest { .. }) || self.story_rest.is_some() || self.story_probe);
        let y = crate::savepoint_rules::y_rule(YIn {
            sit: self.sit,
            menu_key: control.flag("savepoint.menu") != Some(false),
            at_point: at.is_some(),
            prompt_near: at.is_none() && prompt && nearest.is_some(),
            // (SV) any grace in reach: also one whose save point is hidden in combat, an undiscovered one, one the game hides
            near: nearest.is_some() || at.is_some() || near_any,
            discovered: here.is_some_and(|p| p.discovered == Some(true)),
            allowed: here.is_some_and(|p| p.allowed || prompt) || !self.rest_rule,
            prompt,
            game_menu: ui.menu,
            resting: self.resting,
            busy: self.flow != Flow::Idle || self.story_rest.is_some(),
            on_floor,
            alive: self.leader.map_or(sora.status().hp > 0, |l| l.1), // (CV3) the leader's life
        });
        let offered = y.offered;
        // (SP2) KH3's "Save" reaction command in the command menu while the Save is offered (hud.rs shows it)
        let shown = crate::savepoint_rules::prompt_shows(offered, crate::hud::menu_open(), ui.cutscene, control.flag("savepoint.prompt") != Some(false));
        if PROMPT.swap(shown, std::sync::atomic::Ordering::Relaxed) != shown {
            erlog!("savepoint: (SP2) KH3's \"Save\" command (the green reaction command over the command menu, Y) is {}", if shown { "SHOWN" } else { "taken off" });
        }
        if offered != self.offered {
            erlog!("savepoint: the Save command is {} (he {} grace {}; Elden Ring's prompt {})", if offered { "OFFERED: Y opens the KH3 save menu" } else { "gone" }, if at.is_some() { "stands in the reactor of" } else if here.is_some() { "is in the game's prompt of" } else { "left" }, here.map_or(0, |p| p.entity), if prompt { "is up" } else { "is not up" });
            self.offered = offered;
        }
        let menu_open = crate::hud::menu_open();
        // (RC, the user 2026-10-20: "Melina cutscenes should play only when you press activate on the save point, not just by
        // touching it") A STORY EVENT DUE AT THE SAVE POINT HE STANDS IN (the accord: story.rs automatic rows) IS STARTED BY
        // HIS Y, BEFORE THE MENU: the game's own trigger is the player resting at the grace (event 950 waits for flag 9000,
        // which its talk script sets while he rests), and Y is our rest. The press asks for it; the driven rest follows as
        // before (black cover, the grace actions let through only for it, SV). A second Y while it has not come yet (blocked)
        // opens the menu as usual. `story.on_y = 0`: the events come by themselves on entry again (IMPL_ER36).
        let melina_due = at
            .filter(|_| control.flag("story.on_y") != Some(false) && control.flag("story.auto") != Some(false) && !self.sit)
            .and_then(|k| self.points.get(k))
            .filter(|p| p.discovered == Some(true) && self.story.asked() != Some(p.entity))
            .and_then(|p| self.story.due_on_y(p.entity, &flag_set).map(|row| (row, p.entity)));
        // (IMPL_ER26) LB + Y at a save point: THE GAME'S OWN REST (its grace menu: whatever of Elden Ring only that menu
        // offers, e.g. "Talk to Melina"), for the player who needs it; `savepoint.rest_chord = 0` switches it off
        if offered && y_held && !self.y_before && !menu_open && lb_held && !self.sit && control.flag("savepoint.rest_chord") == Some(true) {
            erlog!("savepoint: LB + Y at the save point of grace {}: THE GAME'S OWN REST is asked for (the body sits; its grace menu is the player's, B leaves it)", here.map_or(0, |p| p.entity));
            self.rest_for = here.map(|p| p.entity);
            self.flow = Flow::Rest { close: false, stage: 0, since: 0.0, tries: 0 };
        } else if let Some((row, grace)) = melina_due.filter(|_| offered && y_held && !self.y_before && !menu_open) {
            self.story.ask(grace);
            erlog!("story: (RC) Y at the save point of grace {grace}: {} IS DUE HERE AND STARTS NOW (Melina first; the save menu comes with the next Y)", self.story.rule_id(row));
        } else if offered && y_held && !self.y_before && !menu_open {
            // (IMPL_ER26) Save / Select an Area / Pass Time / Respawn Enemies / [Site of Grace] / Title Screen
            // (IMPL_ER33) "Talk to Melina" only while a row of the story table says the game offers a talk here
            // (IMPL_ER36) ... and only where the game's own menu can hold her entry (talkdrive::menu_gate), or she is
            // expected to speak by herself (a row of kind `self`, once a session)
            let gate = talkdrive::menu_gate(&flag_set, player.current_block_id.area() as u32);
            // (FX5) the talk script's whole condition: the body's special effects it tests too (story.rs `no_effects`); a
            // blocked `self` row is offered only with `story.lift_blockers` 1 (its rest takes them off and back). (FINAL5) DEFAULT 0:
            // the removal and re-application of the game's effects never ran in game, so the entry is offered only when the
            // game's own talk condition holds (faithful to Elden Ring); `story.lift_blockers 1` keeps the lifting to try
            let has = |id: i32| body_has(player, id);
            let lift = crate::story::lift_blockers(control); // (FINAL5) default 0
            if let Some(p) = here {
                let held = self.story.held_by_effects(p.entity, &flag_set, &has);
                if held != self.held_told {
                    for (row, effects) in &held {
                        erlog!("story: (FX5) {row} is ready at grace {} but the body carries {effects:?}, which the talk script tests (she does not speak by herself while any is on): {}{}", p.entity, if lift { "\"Talk to Melina\" is offered and its rest takes them off the body first, then gives them back (story.lift_blockers = 1)" } else { "NOT OFFERED (story.lift_blockers = 0, the default; 1 = its rest takes them off and gives them back)" }, effect_facts(effects));
                    }
                    self.held_told = held;
                }
            }
            let talk = here.filter(|_| control.flag("story.talk") != Some(false) && !self.sit).and_then(|p| self.story.talk_offer_lifting(p.entity, &flag_set, gate.is_ok(), &has, lift));
            let (talk, lift_now) = match talk {
                Some((o, l)) => (Some(o), l),
                None => (None, Vec::new()),
            };
            self.talk_lift = lift_now;
            let menu = GraceMenu { passtime: control.flag("savepoint.passtime") != Some(false), respawn: control.flag("savepoint.respawn") != Some(false), sit: self.sit, title: control.flag("menu.title") != Some(false), talk: talk.is_some(), party: control.flag("savepoint.party") != Some(false) /* (party P) */, load: control.flag("savepoint.load") != Some(false) && crate::hud::saveload_on() /* (SV) */ };
            if let Some(offer) = &talk {
                erlog!("story: the save menu at grace {} shows \"Talk to Melina\" (row {}: {}{}; the game's menu can hold her entry here: {})", here.map_or(0, |p| p.entity), offer.row, if offer.scene { "its scene is expected" } else if offer.own.is_some() { "by the talk script she speaks by herself at a rest; offered once a session" } else { "an entry of the game's grace menu" }, if offer.own.is_some() && offer.entry { ", and the game's menu holds an entry of hers as well" } else { "" }, match gate { Ok(()) => "yes".to_string(), Err(why) => format!("no, {why}") });
            }
            self.talk_offer = talk;
            let opened = crate::hud::open_savepoint_grace(menu);
            self.menu_shown = menu;
            self.menu_at = here.map(|p| p.entity);
            self.save_at = None;
            erlog!("savepoint: Y at the save point of grace {}: the save menu {} ({:?})", self.menu_at.unwrap_or(0), if opened { "OPENS (the world is paused)" } else { "does not open (the HUD is not up, or another menu is)" }, kh_host::savepoint_menu::entries(menu).iter().map(|e| e.action).collect::<Vec<_>>());
        }
        // (IMPL_ER21) Y at a discovered save point the game does not let him rest at: KH3's refusal sound
        if !offered && y_held && !self.y_before && self.rest_rule && self.flow == Flow::Idle && !menu_open && here.is_some_and(|p| p.shown && p.discovered == Some(true) && !p.allowed) && !prompt {
            crate::hud::say(&format!("se {}", kh_ui::menu::SE_MENU_REFUSED));
            erlog!("savepoint: Y at the save point of grace {}: REFUSED (the game offers no rest here now: enemies near, a boss fight)", here.map_or(0, |p| p.entity));
        }
        self.y_before = y_held;
        y.ours
    }

    /// (IMPL_ER26) hook in sora.rs: a world reset was asked of the game in this frame (Respawn Enemies): the orbs on
    /// the field are to be cleared. True once.
    pub fn take_world_reset(&mut self) -> bool {
        std::mem::take(&mut self.world_reset)
    }

    /// (IMPL_ER26) AFTER A COMPLETED SAVE (the Save screen's write of the game's slot and Sora's file): the game's real
    /// rest when a story event may wait for one (`savepoint.rest_events`), the world reset with `savepoint.respawn_on_save`.
    fn after_save(&mut self, control: &Control) {
        let status = crate::slots::save_status();
        if self.save_status != Some(status) {
            if self.save_status.is_some() {
                match status {
                    crate::slots::SaveStatus::Done(_) => self.saved_at = self.save_at.take().or(self.saved_at),
                    crate::slots::SaveStatus::Failed(..) => self.save_at = None,
                    _ => {}
                }
            }
            self.save_status = Some(status);
        }
        if self.saved_at.is_none() || self.flow != Flow::Idle || crate::hud::menu_open() {
            return;
        }
        let Some(entity) = self.saved_at.take() else { return };
        if self.sit {
            return; // (the player has the Site of Grace entry)
        }
        let mode = RestEvents::parse(control.str("savepoint.rest_events"));
        let found = self.roundtable_found();
        match crate::savepoint_rules::rest_after_save(mode, found, self.rested.contains(&entity)) {
            Some(why) => {
                erlog!("savepoint: AFTER THE SAVE at grace {entity}: THE GAME'S REAL REST IS ASKED FOR ({why}); the body sits, the game's grace menu or its scene is the player's (B leaves the menu). savepoint.rest_events = 0 switches this off");
                self.rest_for = Some(entity);
                self.flow = Flow::Rest { close: false, stage: 0, since: 0.0, tries: 0 };
            }
            None if control.on("savepoint.respawn_on_save") => {
                erlog!("savepoint: AFTER THE SAVE at grace {entity}: the world reset is asked for (savepoint.respawn_on_save)");
                self.flow = Flow::World { what: WorldAsk::Respawn, since: 0.0, asked: false, seen: false };
            }
            None => erlog!("savepoint: after the Save at grace {entity}: no rest (rest_events {mode:?}; the Roundtable Hold's grace found: {found:?}; rested here this session: {})", self.rested.contains(&entity)),
        }
    }

    /// (IMPL_ER26) The Roundtable Hold's own site of grace is discovered. Its rows are those of BonfireWarpParam whose
    /// map is 11_10 (the Roundtable Hold: one grace, the Table of Lost Grace), found in the game's table, not by a
    /// flag number from memory. `None`: no such row, or the flags cannot be read.
    fn roundtable_found(&mut self) -> Option<bool> {
        if self.roundtable.is_none() {
            // SAFETY: game thread; the singleton lives as long as the game.
            let repo = unsafe { SoloParamRepository::instance() }.ok()?;
            let flags: Vec<u32> = repo.bonfire_warps.iter().take(8192).filter_map(|e| repo.get_by_bonfire_warp_param_by_entity_id(e.bonfire_entity_id).map(|row| (e.bonfire_entity_id, row))).filter(|(_, row)| row.area_no() == 11 && row.grid_x_no() == 10 && row.eventflag_id() != 0).map(|(entity, row)| { erlog!("savepoint: the Roundtable Hold's grace: entity {entity}, discovery flag {}", row.eventflag_id()); row.eventflag_id() }).collect();
            if flags.is_empty() {
                erlog!("savepoint: NO grace of map 11_10 (the Roundtable Hold) in BonfireWarpParam: taken as not found yet");
            }
            self.roundtable = Some(flags);
        }
        let flags = self.roundtable.as_ref()?;
        if flags.is_empty() {
            return None;
        }
        let mut any = Some(false);
        for flag in flags {
            match flag_set(*flag) {
                Some(true) => return Some(true),
                Some(false) => {}
                None => any = None,
            }
        }
        any
    }
    /// (IMPL_ER36) THE REST MADE FOR A STORY EVENT, DRIVEN (talkdrive.rs `Drive`), a step a frame: what is seen is
    /// read (the game's rest state, the cutscene, Elden Ring's popup menu = its grace menu / another window, the talk
    /// script's flags), the machine says what to do (the cover, the presses that only the game sees, the end).
    fn story_step(&mut self, control: &Control, player: &PlayerIns, ui: ErUi, dt: f32) {
        let Some(mut sr) = self.story_rest.take() else {
            self.pad_mask = false;
            return;
        };
        let name = sr.rule.map_or("Talk to Melina".to_string(), |k| self.story.rule_id(k).to_string());
        // (our menu is still closing: the cover waits with it, so the menu is not cut off)
        if crate::hud::menu_open() && matches!(sr.drive.stage, talkdrive::Stage::Up { .. }) {
            self.story_rest = Some(sr);
            return;
        }
        let popup = inputs::popup_state();
        let (window, id) = popup.map_or((false, 0), |p| (p.top_job != 0, p.talk_id));
        let on = |f: u32| flag_set(f) == Some(true);
        let talk_flag = on(talkdrive::FLAG_TALKING);
        if !sr.drive.holds_pad() {
            sr.id_before = id; // (until the body rests)
        }
        sr.talk_flag_off |= !talk_flag;
        sr.id_left |= sr.drive.holds_pad() && id != talkdrive::TALK_MELINA;
        let seen = Seen {
            resting: self.resting,
            cutscene: ui.cutscene,
            grace_menu: window && id == talkdrive::TALK_GRACE,
            other_window: window && id != talkdrive::TALK_GRACE,
            talking: (talk_flag && sr.talk_flag_off) || (id == talkdrive::TALK_MELINA && (sr.id_left || sr.id_before != talkdrive::TALK_MELINA)),
            warp: on(talkdrive::FLAG_WARP) && !on(talkdrive::FLAG_ROUNDTABLE),
            pressing: matches!(self.flow, Flow::Rest { .. }),
        };
        let before = sr.drive.stage;
        let step = sr.drive.step(dt, seen);
        if sr.cover && step.cover {
            crate::story::cover(true);
        } else {
            crate::story::cover(false);
        }
        self.pad_mask = step.mask;
        if let Some(line) = step.log {
            erlog!("story: {name} at save point {}: {line}{}", sr.grace, if step.cover && !sr.cover { " (NO COVER: story.cover = 0)" } else { "" });
            // (what the talk script looks at, once per decision at the grace menu: for the next reading of a log)
            if matches!(before, talkdrive::Stage::Resting { .. }) && seen.grace_menu && control.flag("story.diag") != Some(false) {
                let flags: Vec<String> = [9000u32, 9001, 951, 953, 4651, 4652, 4653, 4655, 4657, 4680, 4698, 4699, 10000851, 10009655, 104, 105, 108, 110].iter().map(|f| format!("{f}={}", match flag_set(*f) { Some(true) => "1", Some(false) => "0", None => "?" })).collect();
                let effects: Vec<i32> = player.chr_ins.special_effect.entries().take(128).map(|e| e.param_id).filter(|id| [4270, 4271, 4272, 4280, 4282, 4286, 9600, 9603].contains(id)).collect();
                erlog!("story: at the grace menu ({:.1} s into the rest; popup talk id {id}, before the rest {}): flags {} | the body's special effects among those the talk script tests (4270 4271 4272 4280 4282 4286 9600 9603): {effects:?}", match before { talkdrive::Stage::Resting { since } => since, _ => 0.0 }, sr.id_before, flags.join(" "));
            }
        }
        match step.act {
            Act::None => {}
            Act::Rest => {
                self.rest_for = Some(sr.grace);
                self.flow = Flow::Rest { close: false, stage: 0, since: 0.3, tries: 0 };
            }
            Act::Up | Act::Confirm | Act::Back => {
                let (buttons, scan, what) = match step.act {
                    Act::Up => (0x0001, 0, "UP"),
                    Act::Confirm => (inputs::XI_A, inputs::SCAN_E, "CONFIRM (A / E)"),
                    _ => (inputs::XI_B, inputs::SCAN_Q, "BACK (B / Q)"),
                };
                // (the pad hook first; a repeated B alternates with the keyboard when that route is open)
                let how = inputs::press_for_er_try(buttons, scan, if step.act == Act::Back { sr.presses } else { 0 });
                sr.presses = sr.presses.saturating_add(1);
                erlog!("{}story: {name}: {what} pressed for the game's menu ({}); seen: grace menu {} other window {} resting {} talk id {id}", if how.is_none() { "!!! " } else { "" }, how.unwrap_or("NO WAY: the game reads no pad and this key has no keyboard route"), seen.grace_menu, seen.other_window, seen.resting);
            }
        }
        match step.end {
            None => self.story_rest = Some(sr),
            Some(end) => {
                self.pad_mask = false;
                crate::story::cover(false);
                // (FX5) what the rest took off the body is given back (was it already back? the game holds it then)
                let lifted = std::mem::take(&mut self.rest_lifted);
                if !lifted.is_empty() {
                    let back_already: Vec<i32> = lifted.iter().copied().filter(|e| body_has(player, *e)).collect();
                    let missing: Vec<i32> = lifted.iter().copied().filter(|e| !back_already.contains(e)).collect();
                    let given = lift_effects(&missing, true);
                    erlog!("story: (FX5) the body's special effects {given:?} are given back after the rest{}", if back_already.is_empty() { String::new() } else { format!(" ({back_already:?} were on again already: the game puts them back by itself)") });
                }
                let rest_failed = matches!(end, End::Failed(step) if step.starts_with("the rest:"));
                match end {
                    End::Talked(how) => erlog!("story: {name} at save point {}: DONE: {how}", sr.grace),
                    End::Nothing(why) => erlog!("story: {name} at save point {}: NOTHING OF MELINA'S CAME: {why}; he stands, the cover is down{}", sr.grace, if sr.own.is_some() { "; this row is not offered again in this session (the game's own flags decide when she speaks)" } else { "" }),
                    End::Failed(step) => erlog!("!!! story: {name} at save point {}: FAILED AT: {step}; the cover is down; the entry is offered again", sr.grace),
                }
                if let (Some(k), false) = (sr.own, matches!(end, End::Talked(_)) || rest_failed) {
                    self.story.own_tried(k);
                }
                if rest_failed && matches!(self.flow, Flow::Rest { stage: 0, .. }) {
                    self.rest_for = None;
                    self.flow = Flow::Idle;
                }
            }
        }
    }

    /// What the menu asked of the game, a step a frame.
    fn flow_step(&mut self, control: &Control, sora: &Sora, prompt: bool, ui: ErUi, dt: f32) {
        match self.flow {
            Flow::Idle => {}
            // (IMPL_ER26) PASS TIME / RESPAWN ENEMIES: asked once our menu is off and the world runs; then the game's
            // own transition is watched to its end (its fade out, the black, the reset, its fade in)
            Flow::World { what, since, asked, seen } => {
                let since = since + dt;
                self.flow = Flow::World { what, since, asked, seen };
                if !asked {
                    if since < 0.3 || crate::hud::menu_open() {
                        return;
                    }
                    if ui.menu || since > 6.0 {
                        erlog!("savepoint: {what:?}: NOT ASKED (a menu of the game's has the pad, or our menu did not go)");
                        self.flow = Flow::Idle;
                        return;
                    }
                    match self.world_ask(control, what) {
                        Ok(line) => {
                            erlog!("savepoint: {what:?}: {line}");
                            self.world_reset = matches!(what, WorldAsk::Respawn) || control.on("savepoint.passtime.reset");
                            self.flow = Flow::World { what, since: 0.0, asked: true, seen: false };
                        }
                        Err(why) => {
                            erlog!("!!! savepoint: {what:?}: THE GAME'S TIME TRANSITION CANNOT BE USED ({why})");
                            match what {
                                // (the proven route of `time.set`: the clock's target, no fade, no reset)
                                WorldAsk::PassTime(choice) => {
                                    let (h, m) = clock_of(control, choice);
                                    // SAFETY: game thread; the singleton lives as long as the game.
                                    if let Ok(time) = unsafe { WorldAreaTime::instance_mut() } {
                                        time.request_time(h, m, 0);
                                        erlog!("savepoint: Pass Time: the world clock's target is set to {h:02}:{m:02} instead (as `time.set`: no fade of the game's)");
                                    }
                                }
                                WorldAsk::Respawn => crate::hud::say(&format!("se {}", kh_ui::menu::SE_MENU_REFUSED)),
                            }
                            self.flow = Flow::Idle;
                        }
                    }
                    return;
                }
                let now = world_ctrl().map(|c| snapshot(c));
                // (the elapsed times change every frame: only the switches and the target are compared)
                let key = now.map(|n| CtrlSnapshot { floats: [0.0; 10], ..n });
                if key != self.ctrl_told {
                    if let Some(n) = now {
                        erlog!("savepoint: {what:?}: the game's time transition: requested {} fading {} respawn wait {} | elapsed {:.2} s (black {:.2} of {:.2}) | target {:02}:{:02}:{:02}", n.bools[CtrlSnapshot::FADE_OUT_REQUESTED], n.bools[0], n.bools[CtrlSnapshot::RESPAWN_WAIT], n.floats[8], n.floats[7], n.floats[0], n.target[0], n.target[1], n.target[2]);
                    }
                    self.ctrl_told = key;
                }
                let requested = now.is_some_and(|n| n.requested());
                let seen = seen || requested;
                self.flow = Flow::World { what, since, asked, seen };
                if seen && !requested {
                    erlog!("savepoint: {what:?}: DONE after {since:.1} s: the game's transition is over");
                    self.flow = Flow::Idle;
                } else if since > 30.0 || now.is_none() {
                    erlog!("!!! savepoint: {what:?}: no end of the game's transition seen after {since:.1} s (taken up by the game: {seen}): let go; its request switch is left as the game has it");
                    self.flow = Flow::Idle;
                }
            }
            Flow::Map { since } => {
                let since = since + dt;
                self.flow = Flow::Map { since };
                // (our menu has to be off and the world running again before the game takes a press)
                if since > 0.3 && !crate::hud::menu_open() {
                    if ui.menu {
                        erlog!("savepoint: the game's map is up");
                        self.flow = Flow::Idle;
                    } else if since > 4.0 {
                        erlog!("savepoint: the game's map did not open (no pad read by the game and its window not in front?)");
                        self.flow = Flow::Idle;
                    } else if (since - 0.3) % 1.2 < dt {
                        let how = inputs::press_for_er_try(inputs::XI_BACK, inputs::SCAN_G, ((since - 0.3) / 1.2) as u8);
                        crate::travel::from_save_point(); // (IMPL_ER23A) the press that opens it
                        erlog!("savepoint: the map: View / G pressed for the game ({})", how.unwrap_or("NO WAY: neither a pad it reads nor its window in front"));
                    }
                }
            }
            Flow::Rest { close, stage, since, tries } => {
                let since = since + dt;
                self.flow = Flow::Rest { close, stage, since, tries };
                match stage {
                    0 => {
                        if since < 0.3 || crate::hud::menu_open() {
                            return;
                        }
                        if self.resting {
                            self.flow = Flow::Rest { close, stage: 1, since: 0.0, tries };
                        } else if tries >= if self.story_rest.is_some() { 5 } else { 3 } || since > if self.story_rest.is_some() { 8.0 } else { 6.0 } {
                            // (IMPL_ER33) a story rest: the game may play its own "touch" first (4.7 s, MEASURED) and rest at the next press
                            erlog!("{}savepoint: THE GAME DID NOT REST (its prompt {}; {tries} press(es) made): nothing more is tried{}", if self.rest_for.is_some() { "!!! " } else { "" }, if prompt { "is up" } else { "is not up" }, if self.story_rest.is_some() { " at this save point in this session (the story table tries at the next one; the menu's \"Talk to Melina\" asks again here)" } else if self.rest_for.is_some() { " NOW; the next Save here asks again (savepoint.rest_events)" } else { "" });
                            self.rest_for = None;
                            self.flow = Flow::Idle;
                        } else if prompt && !ui.menu && (tries == 0 || since > 1.5) {
                            let how = inputs::press_for_er_try(inputs::XI_Y, inputs::SCAN_E, tries);
                            erlog!("savepoint: rest: Y / E pressed for the game ({})", how.unwrap_or("NO WAY: neither a pad it reads nor its window in front"));
                            self.flow = Flow::Rest { close, stage: 0, since: 0.31, tries: tries + 1 };
                        }
                    }
                    1 => {
                        // (IMPL_ER26) the rest was made here: not asked again at this save point in this session
                        if let Some(entity) = self.rest_for.take() {
                            if !self.rested.contains(&entity) && self.rested.len() < 4096 {
                                self.rested.push(entity);
                            }
                        }
                        // He sits: the game rested (the world is reset, the game saves): Sora's own save follows.
                        if crate::savegate::mode_on() {
                            // SAVE: the gate opens and the game writes its save now; SITE OF GRACE (the rest alone): nothing is saved.
                            if close {
                                let asked = crate::savegate::current_slot().ok_or("the game has no current save slot").and_then(|slot| crate::slots::save_to_slot(slot as usize, sora, "the save point's Save"));
                                self.saving = asked.is_ok();
                                match asked {
                                    Ok(()) => crate::hud::say("item Saving..."),
                                    Err(why) => {
                                        crate::hud::say("item Could_not_save.");
                                        erlog!("savepoint: the Save could not be started ({why})");
                                    }
                                }
                            }
                            erlog!("savepoint: THE GAME RESTED at grace {} (its world reset){}", self.rest.bonfire, if close { "; ITS SAVE IS ASKED FOR NOW, Sora's follows it; its grace menu is left again" } else { "; NOTHING IS SAVED (save.points_only: only Save saves); its grace menu is the player's now" });
                        } else {
                            crate::save::write_now(sora, "the save point's Save");
                            erlog!("savepoint: THE GAME RESTED at grace {} (its world reset and save); Sora's save was asked for{}", self.rest.bonfire, if close { "; its grace menu is left again" } else { "; its grace menu is the player's now" });
                        }
                        self.flow = if close { Flow::Rest { close, stage: 2, since: 0.0, tries: 0 } } else { Flow::Idle };
                    }
                    _ => {
                        if !self.resting && !ui.menu {
                            erlog!("savepoint: Save done: he is back in play");
                            self.flow = Flow::Idle;
                        } else if tries >= 8 {
                            erlog!("savepoint: the game's grace menu could not be left for him ({tries} presses): it is the player's (B leaves it)");
                            self.flow = Flow::Idle;
                        } else if since > 1.6 && ui.menu {
                            let how = inputs::press_for_er_try(inputs::XI_B, inputs::SCAN_Q, tries);
                            erlog!("savepoint: leaving the game's grace menu: B / Q pressed for the game ({})", how.unwrap_or("NO WAY"));
                            self.flow = Flow::Rest { close, stage: 2, since: 0.6, tries: tries + 1 };
                        } else if since > 12.0 {
                            self.flow = Flow::Idle;
                        }
                    }
                }
            }
        }
    }

    /// (IMPL_ER26) THE GAME IS ASKED for a time transition: Pass Time = the time to the chosen clock time, forward;
    /// Respawn Enemies = no time, the world reset of its rest. Never the character's reset, the flasks or the spell
    /// charges (Sora's state is his own), no bonfire named. The line for the log.
    fn world_ask(&mut self, control: &Control, what: WorldAsk) -> Result<String, &'static str> {
        let ctrl = world_ctrl().ok_or("the event manager or its time control is not there")?;
        let before = snapshot(ctrl);
        crate::savepoint_rules::ctrl_usable(&before)?;
        // SAFETY: game thread; the singleton lives as long as the game.
        let time = unsafe { WorldAreaTime::instance() }.map_err(|_| "no world clock")?;
        let now = (time.clock.hours() as u32, time.clock.minutes() as u32, time.clock.seconds() as u32);
        let (params, line) = match what {
            WorldAsk::PassTime(choice) => {
                let target = clock_of(control, choice);
                let (h, m, sec) = kh_host::savepoint_menu::time_to_pass(now, target);
                let reset = control.on("savepoint.passtime.reset");
                (TimeTransitionParams { reset_world: reset, ..TimeTransitionParams::time_skip(h, m, sec) }, format!("until {} = {:02}:{:02}: {h} h {m} min {sec} s pass from {:02}:{:02}:{:02}{}", choice.label(), target.0, target.1, now.0, now.1, now.2, if reset { ", the world is reset with it (savepoint.passtime.reset)" } else { "" }))
            }
            WorldAsk::Respawn => (TimeTransitionParams { reset_world: true, ..TimeTransitionParams::default() }, format!("the world reset of the game's rest (enemies come back; no time passes: {:02}:{:02}:{:02}), the character NOT reset, nothing saved", now.0, now.1, now.2)),
        };
        let fades = (params.fade_out_time, params.black_screen_time, params.fade_in_time);
        ctrl.fade_out_and_pass_time(params);
        self.ctrl_told = None;
        Ok(format!("THE GAME'S TIME TRANSITION IS ASKED FOR (CSEventWorldAreaTimeCtrl, as its own rest / Pass time): {line}; its own fade out {:.2} s, black {:.2} s, fade in {:.2} s; no rest, no animation of the body", fades.0, fades.1, fades.2))
    }

    /// AFTER EACH SIMULATION STEP of Sora: the recover rule of every save point in reach.
    pub fn after_step(&mut self, control: &Control, dt: f32, sora: &mut Sora, capsule_center: Vec3) {
        if !self.on {
            return;
        }
        let params = *sora.params();
        let capsule = (capsule_center, params.capsule_radius, params.capsule_half_height);
        // (ER party S6) THE SAVE POINT WITH THE PARTY (P2): its capsules test the leader (a friend leader's own capsule),
        // "needs recovery" counts the party, and its Recover goes to the leader and every member (partyui.rs)
        #[cfg(feature = "party")]
        let party_pawn = crate::partyui::savepoint_pawn(control);
        #[cfg(feature = "party")]
        let capsule = party_pawn.and_then(|p| p.0).unwrap_or(capsule);
        let heal = control.flag("savepoint.heal") != Some(false);
        // (IMPL_ER19) the save point's sound: `savepoint.sfx` over the sheet's row
        let sound = match control.i64("savepoint.sfx").unwrap_or(self.sfx as i64) {
            0 => SavePointSound::Off,
            2 => SavePointSound::EveryEntry,
            _ => SavePointSound::WithHeal,
        };
        let mut at = None;
        let mut healing = false;
        let rest_rule = self.rest_rule;
        let needs = {
            let s = sora.status();
            s.hp < s.max_hp() || s.mp < s.max_mp() || s.focus < s.focus_max
        };
        #[cfg(feature = "party")]
        let needs = party_pawn.map_or(needs, |p| p.1);
        // (SP2) `savepoint.entry` (default 1): every entry plays KH3's burst and its sound (kh_host::savepoint::EntryBurst)
        let entry_on = control.flag("savepoint.entry") != Some(false);
        let load_window = self.load_in.window; // (LDI)
        for (k, p) in self.points.iter_mut().enumerate() {
            p.sp.entry.on = entry_on;
            // (IMPL_ER21) a hidden save point does nothing; one the game does not let him rest at does not heal
            if !p.shown {
                p.sp.idle();
                p.step = SavePointStep::default();
                continue;
            }
            // (LDI) HE WAS PUT HERE BY THE LOAD: in the load-in window (the game may still move him in the dark), and at the
            // first step of a save point made in it, his overlap is where he stands: no entry burst, no entry sound
            if load_window || p.arrival.settle {
                let first = std::mem::take(&mut p.arrival.settle);
                if p.sp.arrive(capsule) && first {
                    erlog!("savepoint: grace {}: (LDI) he was put in it by the load: no entry burst, no entry sound (the save point stands in its idle state)", p.entity);
                }
            }
            if heal && (p.allowed || !rest_rule) {
                p.sp.sound = sound;
                let sounded = p.sp.sounded;
                #[cfg(feature = "party")]
                {
                    p.step = match party_pawn {
                        Some((_, party_needs)) => {
                            let step = p.sp.step_checked(dt, sora, capsule, party_needs);
                            crate::partyui::savepoint_recover(step.recovered);
                            step
                        }
                        None => p.sp.step(dt, sora, capsule),
                    };
                }
                #[cfg(not(feature = "party"))]
                {
                    p.step = p.sp.step(dt, sora, capsule);
                }
                // (SP2) the sound goes to the host now (sora.rs plays it this step, whoever leads)
                let entries = p.sp.entry.entries;
                self.sounds.extend(p.sp.take_sounds());
                if p.sp.entry.entries != entries {
                    erlog!("savepoint: grace {}: (SP2) the leader STEPS INTO the save point (entry {}): KH3's burst {} for {:.1} s{}", p.entity, p.sp.entry.entries, p.sp.data.heal, p.sp.data.first_access_time, if p.sp.sounded != sounded { format!(" and its sound (\"effect:{}\", 3D at the save point)", p.sp.data.heal) } else { " (savepoint.sfx = 0: no sound)".to_string() });
                }
                if p.step.heal_started || p.sp.sounded != sounded {
                    let s = sora.status();
                    erlog!("savepoint: grace {}: {} (HP {}/{} MP {}/{} focus {}/{}){}", p.entity, if p.step.heal_started { "the heal burst STARTS" } else { "he steps on it with nothing to heal" }, s.hp, s.max_hp(), s.mp, s.max_mp(), s.focus, s.focus_max, if p.sp.sounded != sounded { format!(": its sound is asked for (seam \"effect:{}\", 3D at the save point; {} time(s) here)", p.sp.data.heal, p.sp.sounded) } else { " (savepoint.sfx = 0: no sound)".to_string() });
                }
            } else {
                // (the reactor test without the recover rule)
                let d = capsule.0 - (p.sp.root + p.sp.data.reactor.2);
                p.sp.idle();
                let r = capsule.0 - (p.sp.root + p.sp.data.recover.2);
                p.step = SavePointStep { in_reactor: d.truncate().length() <= capsule.1 + p.sp.data.reactor.0 && d.z.abs() <= capsule.2 + p.sp.data.reactor.1 * 0.5, in_recover: r.truncate().length() <= capsule.1 + p.sp.data.recover.0 && r.z.abs() <= capsule.2 + p.sp.data.recover.1 * 0.5, ..SavePointStep::default() };
                // (IMPL_ER21) he stands in it needing recovery and the game offers no rest: KH3's refusal sound, once a visit
                if heal && rest_rule && p.step.in_recover && needs && p.waited > REFUSE_AFTER && !p.refused_told {
                    p.refused_told = true;
                    crate::hud::say(&format!("se {}", kh_ui::menu::SE_MENU_REFUSED));
                    let s = sora.status();
                    erlog!("savepoint: grace {}: NO HEAL (HP {}/{} MP {}/{}): the game offers no rest here now (no prompt of its own for {:.2} s: enemies alerted nearby, a boss fight): the refusal sound", p.entity, s.hp, s.max_hp(), s.mp, s.max_mp(), p.waited);
                }
            }
            self.healed += p.step.recovered as i64;
            healing |= p.step.healing && p.step.recovered > 0;
            if p.step.in_reactor {
                at = Some(k);
            }
        }
        // (IMPL_ER48, the user: "save points should also cure all status effects") in the recovery area of one that may heal him
        crate::status::save_point(self.points.iter().any(|p| p.shown && heal && (p.allowed || !rest_rule) && p.step.in_recover), dt);
        // (ER party S6, the user's rule) ... and every active member's: the AI allies' bodies are cured with it
        #[cfg(feature = "party")]
        crate::partyui::savepoint_rest(control, self.points.iter().any(|p| p.shown && heal && (p.allowed || !rest_rule) && p.step.in_recover), dt);
        if healing != self.healing {
            let s = sora.status();
            erlog!("savepoint: the save point {} him (HP {}/{} MP {}/{} focus {}/{}; {} point(s) of each given so far)", if healing { "HEALS" } else { "has healed" }, s.hp, s.max_hp(), s.mp, s.max_mp(), s.focus, s.focus_max, self.healed);
            self.healing = healing;
        }
        self.at = at;
        // (IMPL_ER17, testing aid) `savepoint.draw = 0`: the save points work but KH3's effect is not drawn (to look at what the game draws there)
        let draw = control.flag("savepoint.draw") != Some(false);
        let points = &mut self.points;
        crate::fx::with_effects(|fx| {
            for p in points.iter_mut() {
                p.sp.active = draw && p.shown;
                p.sp.show(fx);
            }
        });
    }

    /// (IMPL_ER52) Once a second per save point: the floor under its root, through Sora's collision world (the graces'
    /// mounds left out while they are not drawn); the effects are made anew where it moved by more than `Ground::MOVE_AT`.
    fn ground_phase(&mut self, control: &Control, frame: Frame) {
        let on = control.flag("savepoint.ground") != Some(false);
        let mounds_hidden = crate::savepoint_rules::mound_plan(true, control.flag("grace.hide_mound"), None).hide;
        let rules = self.ground;
        let seconds = self.seconds;
        // (the filter and the floor layer Sora's own casts use: sora.rs pre_physics, keys w.filter / w.floor)
        let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
        let filter = control.str("w.filter").and_then(hex).unwrap_or(crate::havok::filter_of_layer(self.cast_layer)); // (WALLS)
        let layer = control.str("w.floor").and_then(hex).unwrap_or(self.floor_layer) & 0x7f;
        let mut world: Option<crate::world::ErWorld> = None;
        let warm = self.load_in.warm; // (LDI)
        for p in &mut self.points {
            if seconds < p.ground_next {
                continue;
            }
            p.ground_next = seconds + 1.0;
            let root = p.sp.root;
            let (z, floor) = if on {
                let w = world.get_or_insert_with(|| {
                    let mut w = crate::world::ErWorld::new(frame);
                    w.filter = filter;
                    w.floor_filter.set(if layer == 0 || layer == filter & 0x7f { 0 } else { (filter & !0xFF) | layer });
                    w.mounds.set(Some(mounds_hidden));
                    w
                });
                let floor = kh_core::World::line_trace(&*w, root, root - Vec3::Z * rules.reach).map(|h| h.position.z);
                (rules.effect_z(root.z, floor), floor)
            } else {
                (0.0, None)
            };
            if p.ground.is_some_and(|g| (g - z).abs() <= Ground::MOVE_AT) {
                continue;
            }
            if p.ground_said < 6 {
                p.ground_said += 1;
                erlog!(
                    "grace: the save point of grace {}: {} -> its effects (base, body, heal) {:+.1} cm from the root (= {:+.1} cm from the grace's feet){}",
                    p.entity,
                    match (on, floor) {
                        (false, _) => "savepoint.ground = 0: AT THE ROOT (as before IMPL_ER52)".to_string(),
                        (true, Some(f)) => format!("GROUND FOUND {:.1} cm under the root (the trace {:.0} cm down{})", root.z - f, rules.reach, if mounds_hidden { ", the graces' mounds left out" } else { "" }),
                        (true, None) => format!("NO FLOOR within {:.0} cm under the root: KH3's {:.0} cm", rules.reach, rules.miss),
                    },
                    z,
                    z + control.f32("savepoint.height").unwrap_or(92.0),
                    if p.ground.is_some() { " (MOVED: the effects are made anew)" } else { "" }
                );
            }
            p.ground = Some(z);
            p.sp.effect_offset = Vec3::Z * z;
            // the effects are taken down; the next show makes them at the new place
            let was = p.sp.active;
            // (LDI) ...in their idle state when they stood (MEASURED: the floor of a loaded area is found again 1 s later,
            // 12 cm off: the effects were made anew and filled up from nothing in front of him)
            if was && warm > 0.0 {
                p.sp.warm = warm;
            }
            p.sp.active = false;
            crate::fx::with_effects(|fx| p.sp.show(fx));
            p.sp.active = was;
        }
    }

    /// `savepoint.glow = render | alpha` (EXPERIMENT): the grace character's own drawing off, to see
    /// what of the grace's light goes with it (its golden glow, its guidance ray).
    fn glow_phase(&mut self, control: &Control) {
        let want = control.str("savepoint.glow").unwrap_or("keep").to_string();
        if want != self.glow {
            erlog!("savepoint: the grace's own drawing: {want} (was {})", if self.glow.is_empty() { "keep" } else { &self.glow });
            self.release(false);
            self.glow = want;
        }
        if self.glow != "render" && self.glow != "alpha" && self.glow != "sink" && self.glow != "model" {
            return;
        }
        // SAFETY: game thread; the game's own lookup by handle (it checks the handle).
        let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
        for p in &self.points {
            if let Some(chr) = p.handle.as_ref().and_then(|h| wcm.chr_ins_by_handle_mut(h)) {
                if !crate::game::chr_complete(chr) {
                    continue;
                }
                if self.glow == "model" {
                    // (IMPL_ER17, EXPERIMENT) bit 0 of the character model's display flags, as the cutscene figure is hidden
                    if let Some(w) = crate::probe::word(word!(chr.chr_model_ins) + 0x18).filter(|d| *d != 0).map(|d| d + 0x20) {
                        if let Some(now) = crate::probe::u32_at(w) {
                            if now & 1 != 0 {
                                // SAFETY: a flag word of a live model's display entity, just read; game thread.
                                unsafe { std::ptr::write_volatile(w as *mut u32, now & !1) };
                            }
                        }
                    }
                } else if self.glow == "sink" {
                    // (IMPL_ER17, EXPERIMENT) the character 30 m under its grace: what of the grace's light goes down with it?
                    let want = p.er_pos - Vec3::Y * 30.0;
                    chr.modules.physics.position = crate::game::hp(want);
                } else if self.glow == "render" {
                    if chr.chr_flags1c5.enable_render() {
                        chr.chr_flags1c5.set_enable_render(false);
                    }
                } else if chr.base_transparency != 0.0 {
                    chr.base_transparency = 0.0;
                }
            }
        }
    }

    /// The graces get their own drawing back; with `all` the save points go too.
    fn release(&mut self, all: bool) {
        if self.glow == "render" || self.glow == "alpha" {
            // SAFETY: as in `glow_phase`.
            if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
                for p in &self.points {
                    if let Some(chr) = p.handle.as_ref().and_then(|h| wcm.chr_ins_by_handle_mut(h)) {
                        if crate::game::chr_complete(chr) {
                            if self.glow == "render" {
                                chr.chr_flags1c5.set_enable_render(true);
                            } else {
                                chr.base_transparency = 1.0;
                            }
                        }
                    }
                }
            }
            self.glow.clear();
        }
        if all && !self.points.is_empty() {
            let points = &mut self.points;
            for p in points.iter_mut() {
                p.sp.active = false;
            }
            crate::fx::with_effects(|fx| {
                for p in points.iter_mut() {
                    p.sp.show(fx);
                }
            });
            self.points.clear();
            self.at = None;
        }
    }

    /// Sora stops (death, payload unload).
    pub fn stop(&mut self) {
        self.release(true);
        PROMPT.store(false, std::sync::atomic::Ordering::Relaxed); // (SP2) no Save command without him
        // (IMPL_ER26) a death or a load puts the game back to its last Save: what a rest after that Save started (Melina's
        // accord) is gone with it, so the rests made are forgotten and the next Save rests again where one may be needed
        self.rested.clear();
        (self.rest_for, self.save_at, self.saved_at) = (None, None, None);
        // (IMPL_ER33) the session's story tries are forgotten with it; the cover never outlives him
        self.story.reset();
        // (FX5) what a driven rest had taken off the body is given back (a load / death cut the rest short)
        let lifted = std::mem::take(&mut self.rest_lifted);
        if !lifted.is_empty() {
            if let Some(player) = crate::game::player_mut() {
                let missing: Vec<i32> = lifted.iter().copied().filter(|e| !body_has(player, *e)).collect();
                let given = lift_effects(&missing, true);
                erlog!("story: (FX5) the rest was cut short: the body's special effects {given:?} are given back");
            }
        }
        self.story_rest = None;
        (self.talk_offer, self.pad_mask) = (None, false);
        crate::story::cover(false);
        if matches!(self.flow, Flow::World { .. }) {
            self.flow = Flow::Idle;
        }
    }

    /// The once-a-second line.
    pub fn summary(&self, sora_er: Vec3) -> Option<String> {
        self.on.then(|| {
            let nearest = self.points.iter().map(|p| ((p.er_pos - sora_er).length(), p)).min_by(|a, b| a.0.total_cmp(&b.0));
            format!(
                "savepoint: {} save point(s) in reach, {} hidden by the game{} | {} | resting {} (flags {:#x}) | flow {:?} | healed {} point(s)",
                self.points.len(),
                self.points.iter().filter(|p| !p.shown).count(),
                nearest.map_or(String::new(), |(d, p)| format!(" (nearest: grace {} at {:.1} m = Elden Ring ({:.2}, {:.2}, {:.2}), {}, drawn base / body / heal {:?})", p.entity, d, p.er_pos.x, p.er_pos.y, p.er_pos.z, match p.discovered { Some(true) => "discovered", Some(false) => "NOT discovered", None => "discovery unknown" }, p.sp.drawn()) + &format!(", {:?}{}{}", p.presence, if p.shown { "" } else { " = HIDDEN SAVE POINT" }, if p.step.in_recover { if p.allowed { ", the game lets him rest" } else { ", the game offers NO rest" } } else { "" })),
                if self.offered { "Save OFFERED" } else if self.at.is_some() { "in a reactor" } else { "not at one" },
                self.resting,
                self.rest.flags,
                self.flow,
                self.healed,
            )
        })
    }

    /// `dbg.lua` (one-shot): the game's rest state and its bonfire conditions.
    fn dump_lua(&self) {
        // SAFETY: game thread; the singleton lives as long as the game; pointers are checked before use.
        let Ok(man) = (unsafe { CSLuaEventManImp::instance() }) else {
            erlog!("lua: no CSLuaEventMan");
            return;
        };
        erlog!("lua: rest state {:?}", rest_now());
        if !plausible(word!(man.lua_event_observer)) {
            return;
        }
        let observer = &man.lua_event_observer;
        let mut n = 0;
        for condition in observer.bonfire_event_observees.iter() {
            if !plausible(word!(*condition)) || n >= 64 {
                break;
            }
            erlog!("lua: bonfire condition {n}: id {} group {} stop {} deleted {}", condition.condition_id, condition.event_group, condition.stop_execution, condition.is_deleted);
            n += 1;
        }
        erlog!("lua: {n} bonfire condition(s) listed");
    }
}
