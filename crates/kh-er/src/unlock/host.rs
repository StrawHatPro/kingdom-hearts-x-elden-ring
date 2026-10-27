//! (ER party S8) THE UNLOCKS, GAME HALF. Everything game-side is UNTESTED IN THE GAME (agents never launch Elden Ring);
//! every step logs a `unlock:` line so the user's log shows what happened and why.
//!
//! WHAT RUNS (all with `party.er` on and `party.unlocks` (default 1) on):
//! - THE CHARACTER SELECT: the new game's choice menu asks "Whose journey will you begin?" first (kh_ui::newgame
//!   StarterQuestion, KH3's own window, sounds and keys; the pad: D-pad / stick, A, B). The pick is kept beside the
//!   opening's pair (`<saves>\newgame-starter.txt`) and taken when the new character first loads (save.rs). The starter
//!   alone is unlocked (level 1 by S6's rule: the shared EXP of a new game is 0).
//! - WHO IS UNLOCKED goes to S1's host every party step (`PartyCore::set_unlocked`: the locked leave the formation, the
//!   newly unlocked join, up to 3) and into the save's party record (`unlocked`, `starter`; save.rs `capture` -> [`stamp`]).
//!   A save without the record is Sora's alone (he was the only one played before).
//! - THE STAGED BOSS FIGHTS, ONE UNIVERSAL RULE (the user, binding: "The boss should not be paused. Load the boss and the
//!   character the boss is fighting at the same time and make them fight though both characters do 0 damage to each
//!   other until the player enters the arena this is a universal rule for when a not yet obtained friend is fighting a
//!   boss"), data-driven: every sheet row `scene.<id>` of kind `fight` (today Goofy vs Leonine Misbegotten at Castle
//!   Morne, Sora vs Margit at Stormveil's gate when Sora is not the starter). The moment the boss is loaded (found by its
//!   entity id) the friend appears beside it (Goofy in a party runtime of his own whose stand-in "player" stands by the
//!   boss; Sora, out of the party, by the stage pilot) and the fight is on: THE REAL BOSS'S OWN AI fights him (ER boss,
//!   aiswitch.rs: the event keeps its AI off until the fight starts; its switch, the one bit EMEVD 2004[1] writes, is
//!   written on directly, with the fight start's update rate, re-asserted every frame: no boss bar, fog or music; the
//!   boss's double, double.rs, is only a logged fallback behind `unlock.double_fallback = 1`), and the friend has an Elden Ring body (S3's pool, `stage_wants`) its AI targets.
//!   0 DAMAGE BOTH WAYS: the boss's blows on his body become his reaction with 0 HP (`stage_body_hit`); his hits on the
//!   boss are bullets OWNED BY HIS BODY (a non-player owner: no player hit for the boss's event, no hate on the player)
//!   whose HP loss is given back at once (the boss never loses HP). The game's fog flag (or its boss bar) = the player
//!   is in: the same boss fights on (its event's fight start takes the switch, no swap), the friend JOINS the party where he stands, the game's own damage.
//!   The game's defeat flag = he is UNLOCKED. A death / a reload / quitting: re-staged at the next approach.
//! - MARGIT'S INTRO is skipped while Sora is locked: the game's own "first encounter done" flag 10000851 is set so that
//!   his fight event (m10 EVENT 10002860) takes its fog-wall branch (the fight starts as the player is through).
//! - DONALD AT THE LIMGRAVE FINGER READER (m60_41_39_00, the crone entity 1041390700): within `show_range` he stands
//!   in front of her (her live facing) playing his angry rows; the leader within `talk_range` and facing him: the prompt
//!   "[Y] Talk"; Y: his lines in the KH talk window over his head (A shows the rest of a line, then goes on), his
//!   gesture and voice per line; the last line unlocks him.
//! - THE UNLOCK MOMENT (rules.rs `Celebration`): KH3's `vs_com_lv1` burst at his feet, the BONUS! fanfare (static SE 38),
//!   his voice, the banner "<Name> joined the party!", his rows (weapon away, KH3's dance; Sora: his selfie pose), his AI
//!   held meanwhile; S6's join level (the shared EXP) when he spawns.
//! - (ALLYJOIN, the user 2026-10-27: "when gaining a new ally, have a popup ask if the player wants them to join the party?
//!   If yes, open up the party creation menu usually accessed from save points. If no, they are not added but can still be
//!   added at a save point like normal.") RIGHT AFTER HIS OBTAINED POPUP (the unlock moment) the HUD asks the sheet's
//!   `text.join.question` in KH3's Dialog window (kh-host hud_ally.rs; the world held; the pad: D-pad / stick, A, B = No).
//!   YES: he stays in the party where the unlock put him (up to 3) and the save point's Party Formation screen opens at
//!   once (he is unlocked on it: placed, or there to place when the party is full); closing it goes back to the play.
//!   NO: he stays unlocked and leaves the active party (rules.rs `decline_join`; his celebration ends); a save point's
//!   Party Formation adds him as for anyone. For whoever is unlocked (any character the sheet unlocks), not per name. A
//!   pack without the Dialog movie, `unlock.join_ask = 0`, or a load before the answer: he joins as before.
//!
//! KEYS (`kh-er.ini`; (INI) once `kh-er.ctl`): `party.unlocks` (1; 0 = everyone available, no scenes, nothing written), `unlock.skip_intro` (1),
//! (ALLYJOIN) `unlock.join_ask` (1; 0 = a new ally joins at once, no question, as before),
//! (ER boss) `unlock.double_fallback` (0; 1 = the boss's double when the real boss's AI switch is refused),
//! `unlock.log` (0; 1 = the facts of every scene once a second). ONE-SHOTS (testing aids; a change of value fires):
//! `unlock.state` (the whole state in the log, the doubles too), `unlock.give = sora|donald|goofy` (unlock with the celebration),
//! `unlock.reset = sora|donald|goofy|all` (lock again, the starter never; its scene starts afresh), `unlock.starter =
//! sora|donald|goofy` (a new game's state with that starter), `unlock.jump = goofy|sora:stage|fight|won` / `donald:talk|
//! won` (a scene to that step at once), `unlock.goto = goofy|sora|donald` (hop the player to the scene's approach point:
//! same map area only, 28 m a hop).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use eldenring::cs::{CSEventFlagMan, ChrIns, ChrSet, PlayerIns, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::{Vec2, Vec3};
use kh_core::host::{Ally, AttackShape, FinishKind, Friend, FriendLinkRequest, HitResult, HurtShape, PushBody, TargetBody, TargetDefense, TargetInfo};
use kh_core::{Attach, EffectHandle, EffectId, EffectSpawn, HitRecord, Input, Output, PadButton, PadButtons, PadState, Pose, Sora, TargetFlags, TargetId, Targets};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;
use kh_party::{CommandStatus, CommandTicket, MemberCommand};
use serde_json::Value;

use super::aiswitch::{AiWake, BossAi, Fields, Note, Rate};
use super::double::{Double, Rows};
use super::rules::*;
use super::sheet::*;
use crate::control::{Control, OneShot};
use crate::coords;
use crate::erlog;
use crate::game::{self, plausible};
use crate::party::glue::{FrameIn, HostRules, PartyCore};
use crate::targets::ErTargets;
use crate::word;
use crate::world::ErWorld;

/// The staged boss's target id in the stage's own target list (never one of ErTargets', which count up from 1).
pub const STAGE_BOSS: TargetId = TargetId(0x7E00_0001);

// ------------------------------------------------------------------------------------------------- the game's reads

fn flag(id: u32) -> Option<bool> {
    if id == 0 {
        return None;
    }
    // SAFETY: game thread; the singleton lives as long as the game; the flag blocks are the game's own tables.
    let man = unsafe { CSEventFlagMan::instance() }.ok()?;
    if man.virtual_memory_flag.flag_blocks.is_null() {
        return None;
    }
    Some(man.virtual_memory_flag.get_flag(id))
}

fn set_flag(id: u32) -> bool {
    if id == 0 {
        return false;
    }
    // SAFETY: as savepoint.rs `set_flag`: the SDK's own writer (it does nothing for a flag without a block).
    let Ok(man) = (unsafe { CSEventFlagMan::instance_mut() }) else { return false };
    if man.virtual_memory_flag.flag_blocks.is_null() {
        return false;
    }
    man.virtual_memory_flag.set_flag(id, true);
    true
}

/// The game's constant flags 6000 / 6001 read off / on: the flags can be trusted (bossreward.rs's check).
fn flags_readable() -> bool {
    flag(6000) == Some(false) && flag(6001) == Some(true)
}

/// Every character pointer of the game's sets.
fn each_chr(f: &mut dyn FnMut(usize)) {
    // SAFETY: game thread; the singleton lives as long as the game; every pointer is checked before it is followed.
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    unsafe fn each<T>(set: &ChrSet<T>, f: &mut dyn FnMut(usize))
    where
        T: fromsoftware_shared::Subclass<ChrIns> + 'static,
    {
        let entries = word!(set.entries);
        let capacity = set.capacity as usize;
        if !plausible(entries) || capacity == 0 || capacity > 8192 {
            return;
        }
        for k in 0..capacity {
            // an entry is (pointer, ...) of 16 bytes (the walk of targets.rs `each_chr_of`)
            let p = std::ptr::read_unaligned((entries + k * 16) as *const usize);
            if plausible(p) {
                f(p);
            }
        }
    }
    // SAFETY: as above.
    unsafe {
        for k in 0..wcm.chr_sets.len() {
            let w = word!(wcm.chr_sets[k]);
            if plausible(w) {
                each(&*(w as *const ChrSet<ChrIns>), f);
            }
        }
        each(&wcm.open_field_chr_set.base, f);
    }
}

/// The character of an event entity id (complete), and its pointer.
fn chr_of_entity(entity: u32) -> Option<(usize, &'static mut ChrIns)> {
    if entity == 0 {
        return None;
    }
    let mut found = 0usize;
    each_chr(&mut |p| {
        if found == 0 {
            // SAFETY: a character-set entry pointer, checked plausible; the field is read only after completeness.
            let chr = unsafe { &*(p as *const ChrIns) };
            if game::chr_complete(chr) && chr.event_entity_id == entity {
                found = p;
            }
        }
    });
    if found == 0 {
        return None;
    }
    // SAFETY: found complete in this walk; game thread.
    Some((found, unsafe { &mut *(found as *mut ChrIns) }))
}

/// The player's place: physics position, block-local position, block (area, x, z), open world.
#[derive(Clone, Copy, Debug)]
struct PlayerPlace {
    physics: Vec3,
    local: Vec3,
    block: (u8, i32, i32),
    overworld: bool,
}

fn player_place(player: &PlayerIns) -> Option<PlayerPlace> {
    let b = player.current_block_id;
    let local = Vec3::new(player.block_position.x, player.block_position.y, player.block_position.z);
    let physics = game::v3(&player.chr_ins.modules.physics.position);
    (local.is_finite() && physics.is_finite()).then_some(PlayerPlace { physics, local, block: (b.area(), b.block() as i32, b.region() as i32), overworld: b.is_overworld() })
}

/// A scene's map place in this frame's physics space (None: another area, or too far).
fn scene_physics(pp: &PlayerPlace, map: &str, local: Vec3, reach: f32) -> Option<Vec3> {
    let m = parse_map(map)?;
    local_to_physics(local, m, pp.block, pp.overworld, pp.local, pp.physics, reach)
}

/// The boss bar shows this character.
fn on_boss_bar(chr: &ChrIns) -> bool {
    crate::erhud::boss_bar_handles().iter().any(|h| *h == chr.field_ins_handle)
}

/// The current animation of a character is an attack (3000 - 3999 of its own numbering).
fn attacking(chr: &ChrIns) -> Option<i32> {
    let now = crate::chranim::now(chr);
    let id = now.id.rem_euclid(1_000_000);
    (3000..4000).contains(&id).then_some(id)
}

/// A world point (ER physics) on the 1920 x 1080 stage (the HUD's), None behind the camera.
fn to_stage(p: Vec3) -> Option<(f32, f32)> {
    let cam = game::camera()?;
    let d = p - cam.position;
    let z = d.dot(cam.forward);
    if z < 0.2 {
        return None;
    }
    let t = (cam.fov * 0.5).tan().max(1e-3);
    let x = d.dot(cam.right) / (z * t * cam.aspect.max(0.01));
    let y = d.dot(cam.up) / (z * t);
    Some(((x + 1.0) * 0.5 * 1920.0, (1.0 - y) * 0.5 * 1080.0))
}

/// The ground under an ER point (a cast down from 3 m above it), or the point.
fn ground(world: &ErWorld, p: Vec3) -> Vec3 {
    let mut hits = Vec::new();
    crate::havok::cast_sphere(p + Vec3::Y * 3.0, Vec3::NEG_Y * 8.0, 0.2, world.filter, false, &mut hits);
    hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
    hits.first().map_or(p, |h| Vec3::new(p.x, h.position.y, p.z))
}

/// Interned anim-set row names (kh-party takes `&'static str`).
fn intern(s: &str) -> &'static str {
    static KEPT: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(k) = kept.iter().find(|k| **k == s) {
        return k;
    }
    let k: &'static str = Box::leak(s.to_string().into_boxed_str());
    kept.push(k);
    k
}

fn chara_word(text: &str) -> Option<PartyChara> {
    match text.trim().to_ascii_lowercase().as_str() {
        "sora" => Some(PartyChara::Sora),
        "donald" => Some(PartyChara::Donald),
        "goofy" => Some(PartyChara::Goofy),
        _ => None,
    }
}

// ------------------------------------------------------------------------------------------------- the stage's targets

/// THE STAGED BOSS AS THE ONLY TARGET of a staged friend (his AI's enemy and his hits' target). Its place is the boss's
/// (KH3 space); a hit on it is ACCEPTED (the friend's own hit effects and pauses play) and goes NOWHERE: no player
/// bullet, no damage, nothing the game's fight event could take for the player's attack.
pub struct StageTargets {
    pub boss: Option<TargetInfo>,
    pub swallowed: u32,
    /// Whose hits these are (they are fired by his body); None = only counted (the offline harness).
    pub who: Option<PartyChara>,
}

impl Targets for StageTargets {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.boss);
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.boss.filter(|b| b.id == id)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(b) = self.info(id) {
            let up = Vec3::Z * (b.half_height - b.radius).max(0.0);
            out.push(HurtShape { a: b.center - up, b: b.center + up, radius: b.radius, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        let Some(b) = self.info(hit.target) else { return HitResult::default() };
        self.swallowed += 1;
        if let Some(who) = self.who {
            let power = match hit.damage_kind.value() {
                0 => 1,
                k => (k as i32).clamp(1, 6),
            };
            if let Ok(mut q) = HITS_OUT.lock() {
                if q.len() < 8 {
                    q.push(Shot { who, at: hit.position, dir: hit.direction, power, name: hit.attack_name });
                }
            }
        }
        HitResult { accepted: true, damage_dealt: 0.0, hp_after: b.hp, killed: false, guarded: false }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        None
    }
    fn allies(&self, _out: &mut Vec<Ally>) {}
    fn lock_on_range(&self, _id: TargetId) -> f32 {
        3000.0
    }
    fn engaged_count(&self) -> u32 {
        u32::from(self.boss.is_some())
    }
    fn body(&self, _id: TargetId, _part: u32) -> TargetBody {
        TargetBody::default()
    }
    fn push_body(&self, _id: TargetId) -> Option<PushBody> {
        None
    }
    fn attack_shapes(&self, _out: &mut Vec<AttackShape>) {}
    fn finish_kind(&self, _id: TargetId) -> FinishKind {
        FinishKind::default()
    }
    fn friends(&self, _out: &mut Vec<Friend>) {}
    fn friend_link(&mut self, _req: FriendLinkRequest) {}
}

impl kh_partyhost::PartyTargets for StageTargets {
    fn ai_actors(&self, out: &mut Vec<kh_ai::ActorView>) {
        if let Some(b) = self.boss {
            out.push(kh_ai::ActorView {
                actor: kh_ai::ActorRef::Enemy(b.id.0),
                uid: -1,
                position: b.position,
                center: b.center,
                yaw: b.yaw,
                radius: b.radius,
                half_height: b.half_height,
                alive: true,
                hostile: true,
                lockable: true,
                hp: b.hp,
                hp_max: b.hp_max,
                ..kh_ai::ActorView::default()
            });
        }
    }
    fn attacked_by_enemies(&self) -> bool {
        self.boss.is_some()
    }
}

// ------------------------------------------------------------------------------------------------- the director

/// What S4 draws of the stage: a staged friend's pose (KH3 space).
#[derive(Clone)]
pub struct StageDraw {
    pub chara: PartyChara,
    pub posed: kh_partyhost::PosedChar,
    /// (INT) his pose of the step before (drawn between the two: `at`)
    pub before: Option<kh_partyhost::PosedChar>,
}

impl StageDraw {
    /// (INT) As drawn `alpha` of the way from the step before to the latest one (sora.rs's alpha, as the party's friends).
    pub fn at(&self, alpha: f32) -> kh_partyhost::PosedChar {
        match &self.before {
            Some(b) if alpha < 1.0 => crate::interp::posed_between(b, &self.posed, alpha),
            _ => self.posed.clone(),
        }
    }
}

/// A friend in a party runtime of his own (Goofy at Leonine, Donald at the crone): the runtime, its stand-in leader.
struct Stage {
    chara: PartyChara,
    core: Box<PartyCore>,
    stand_in: Sora,
    anchor: Vec3,
    /// The place he was put (KH3), his yaw.
    spot: Vec3,
    yaw: f32,
    fresh: bool,
    /// The row he plays (Donald) and its ticket.
    row: usize,
    ticket: Option<CommandTicket>,
}

struct FightSlot {
    chara: PartyChara,
    scene: FightScene,
    dir: FightDirector,
    /// (ER boss) THE REAL BOSS'S AI SWITCH while staged (aiswitch.rs): the event's held boss woken by its own field.
    wake: AiWake,
    /// (ER boss) HP given back to the real boss while staged, and the refusal said.
    kept_hp: i64,
    refusal_said: bool,
    /// The boss's double (double.rs): ONLY the logged fallback when the switch's write is refused and
    /// `unlock.double_fallback = 1` (default 0).
    double: Double,
    /// The double's attack animation last said.
    attack_said: Option<i32>,
    /// The boss character found by entity id: its pointer and when it was last looked for.
    boss: Option<usize>,
    look_in: u32,
    said: Option<String>,
}

struct Celebrating {
    c: Celebration,
    ticket: Option<CommandTicket>,
    started: bool,
    /// Seconds before it starts.
    wait: f32,
}

struct Shots {
    state: OneShot,
    give: OneShot,
    reset: OneShot,
    starter: OneShot,
    jump: OneShot,
    goto: OneShot,
}

pub struct Director {
    sheet: UnlockSheet,
    unlocks: Option<Unlocks>,
    /// A new character whose starter was not chosen on the title: the in-game choice menu asks it.
    ask_starter: bool,
    fights: Vec<FightSlot>,
    talk: TalkDirector,
    /// In the party for a running fight (not unlocked yet).
    guests: Vec<PartyChara>,
    staged: Option<Stage>,
    donald: Option<Stage>,
    /// Sora at Margit (the main Sora): his spot and yaw (KH3), placed yet.
    sora_stage: Option<(Vec3, f32, bool)>,
    pilot: Pilot,
    celebrate: Option<Celebrating>,
    /// A friend who joins the main party where his stage put him: (who, KH3 place, yaw, steps to wait).
    joins: Vec<(PartyChara, Vec3, f32, u32)>,
    intro_said: bool,
    sec: f32,
    talk_advanced: bool,
    talk_line: Option<usize>,
    /// Seconds since the director was made (the doubles' clock).
    sec_total: f32,
    /// Stage runtimes not in use (their packs are read once a game run: an approach does not read them again).
    spare: Vec<Stage>,
    /// The crone (pointer) and the steps to her next lookup.
    crone: Option<usize>,
    crone_in: u32,
    prompt_up: bool,
    /// (CV1) Donald's talk as a conversation of crate::conv (its id while it runs).
    conv: Option<crate::conv::ConvId>,
    /// (ALLYJOIN) New allies whose "join the party?" the HUD asks (their answer not taken yet).
    join_ask: Vec<PartyChara>,
}

static DIRECTOR: Mutex<Option<Director>> = Mutex::new(None);
static ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);
static SORA_STAGED: AtomicBool = AtomicBool::new(false);
static TALK_HOLD: AtomicBool = AtomicBool::new(false);
/// (CV3) Donald's talk's owner name in crate::conv (its logs, `running`, the party-talk priority `offer_talk`).
pub const DONALD_OWNER: &str = "S8 Donald at the Finger Reader";
static CHOSEN: Mutex<Option<usize>> = Mutex::new(None);
static DRAWS: Mutex<Vec<StageDraw>> = Mutex::new(Vec::new());
/// The staged members' bones / sockets of the latest party step (S5: `Attach::Party` of a staged friend's sounds / effects).
static TAPS: Mutex<Vec<(PartyChara, kh_partyhost::MemberTap)>> = Mutex::new(Vec::new());
/// The staged friends' bodies wanted of S3's pool (`stage_wants`), and who is staged (bit = PartyChara).
static WANTS: Mutex<Vec<kh_partyhost::AllyWant>> = Mutex::new(Vec::new());
static STAGED_MASK: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
/// The staged friends' hits on the boss (fired by their bodies at the next party step) and the boss's blows on their
/// bodies (applied at 0 damage at the next party step).
static HITS_OUT: Mutex<Vec<Shot>> = Mutex::new(Vec::new());
static BODY_HITS: Mutex<Vec<(PartyChara, HitRecord, kh_core::states::HitOnPlayer, i32, i32)>> = Mutex::new(Vec::new());

/// A staged friend's hit on the boss, to be fired by his body.
#[derive(Clone, Copy, Debug)]
struct Shot {
    who: PartyChara,
    /// The contact point and the blow's direction (KH3), its KH3 reaction power.
    at: Vec3,
    dir: Vec3,
    power: i32,
    name: &'static str,
}
static HOP: Mutex<Option<(Vec3, u32)>> = Mutex::new(None);
/// unlock.goto's hops run until this many milliseconds of the system clock (0 = none).
static HOP_UNTIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// (sora.rs) unlock.goto's hops are under way (the hop mover keeps them without a dbg.hop key).
pub fn hop_active() -> bool {
    now_ms() < HOP_UNTIL.load(Ordering::Relaxed)
}
/// Y went down since the last party step (`note_pad`), and the pad of the frame before.
static Y_LATCH: AtomicBool = AtomicBool::new(false);
static Y_BEFORE: AtomicBool = AtomicBool::new(false);
/// The prompt "[Y] Talk" is up: Y is the talk's.
static PROMPT_UP: AtomicBool = AtomicBool::new(false);

fn sheet_path() -> Option<PathBuf> {
    let root = ROOT.lock().ok()?.clone()?;
    let (_, loadout) = crate::sora::data_paths(&root);
    Some(loadout.with_file_name("party-er-unlocks.json"))
}

fn load_sheet() -> UnlockSheet {
    let Some(path) = sheet_path() else { return UnlockSheet::default() };
    match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| UnlockSheet::from_text(&t)) {
        Ok((s, unknown)) => {
            erlog!("unlock: sheet {} read ({} lines of Donald's, {} rows not understood{})", path.display(), s.lines.len(), unknown.len(), if unknown.is_empty() { String::new() } else { format!(": {}", unknown.join(", ")) });
            s
        }
        Err(e) => {
            erlog!("unlock: sheet {} NOT read ({e}): the scenes have no places; only the starter and the unlock record work", path.display());
            UnlockSheet::default()
        }
    }
}

fn with<R>(f: impl FnOnce(&mut Director) -> R) -> R {
    let mut guard = DIRECTOR.lock().unwrap_or_else(|e| e.into_inner());
    let d = guard.get_or_insert_with(Director::new);
    f(d)
}

impl Director {
    fn new() -> Director {
        let sheet = load_sheet();
        // THE UNIVERSAL RULE: one slot per sheet row of kind `fight` (its friend, its boss)
        let fights = sheet.fights.iter().filter_map(|s| s.chara.map(|c| FightSlot { chara: c, scene: s.clone(), dir: FightDirector::new(sheet.lost_after), wake: AiWake::default(), kept_hp: 0, refusal_said: false, double: Double::default(), attack_said: None, boss: None, look_in: 0, said: None })).collect();
        let talk = TalkDirector::new(sheet.lines.len(), sheet.donald.talk_range);
        Director { sheet, unlocks: None, ask_starter: false, fights, talk, guests: Vec::new(), staged: None, donald: None, sora_stage: None, pilot: Pilot::default(), celebrate: None, joins: Vec::new(), intro_said: false, sec: 0.0, talk_advanced: false, talk_line: None, sec_total: 0.0, prompt_up: false, spare: Vec::new(), crone: None, crone_in: 0, conv: None /* (CV1) */, join_ask: Vec::new() /* (ALLYJOIN) */ }
    }

    /// What goes to S1's host: the unlocked and the fight guests.
    fn present(&self) -> Option<Vec<PartyChara>> {
        let u = self.unlocks.as_ref()?;
        let mut list = u.list();
        for g in &self.guests {
            if !list.contains(g) {
                list.push(*g);
            }
        }
        list.sort_by_key(|c| *c as u8);
        Some(list)
    }
}

// ------------------------------------------------------------------------------------------------- entry points

/// `party.unlocks` (default on).
pub fn on(control: &Control) -> bool {
    control.flag("party.unlocks") != Some(false) && control.flag("party.er") != Some(false)
}

/// (newgame.rs, every frame at the title and in game) Where the DLL's root is (the sheet is found from it).
pub fn set_root(root: &Path) {
    if let Ok(mut r) = ROOT.lock() {
        if r.is_none() {
            *r = Some(root.to_path_buf());
        }
    }
}

/// (hud.rs, before the new game's choice menu opens) THE STARTER QUESTION (None: the unlock rule is off: the game's menu).
pub fn starter_question(control: &Control) -> Option<kh_ui::newgame::StarterQuestion> {
    // (CV3, ERT4 fault 7) ALWAYS ASKED ON A NEW GAME: who you start as is the player's choice, also with the unlock rule
    // switched off (`party.unlocks = 0`, a testing switch, kept it from the menu); `newgame.starter = 0` alone leaves it out
    if control.flag("newgame.starter") == Some(false) {
        return None;
    }
    with(|d| {
        let s = &d.sheet;
        Some(kh_ui::newgame::StarterQuestion { question: s.select_question.clone(), options: s.select_options.clone(), describe: s.select_describe.clone(), confirm: s.select_confirm.clone() })
    })
}

/// (hud.rs) The in-game choice menu asks the starter too (a new character whose starter was not chosen on the title).
pub fn asks_starter(control: &Control) -> bool {
    control.flag("newgame.starter") != Some(false) && with(|d| d.ask_starter) // (CV3: not behind `party.unlocks`)
}

/// (hud.rs, after `take_newgame_choice`) The starter the menu confirmed (`Hud::take_newgame_starter`). On the title it
/// waits for the new character; in game (the menu after the intro) it is applied at once.
pub fn chosen_starter(index: Option<usize>) {
    let Some(i) = index else { return };
    let starter = starter_of_index(i);
    erlog!("unlock: THE STARTER IS {} (the new game's character select)", starter.name());
    *CHOSEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(i);
    with(|d| {
        if d.ask_starter {
            d.ask_starter = false;
            d.unlocks = Some(Unlocks::new_game(starter));
            d.reset_scenes();
            *CHOSEN.lock().unwrap_or_else(|e| e.into_inner()) = None;
            erlog!("unlock: the new character starts as {} (chosen in game, after the intro): {}", starter.name(), d.unlocks.as_ref().map_or(String::new(), Unlocks::describe));
        }
    });
}

/// (newgame.rs, when the title's pair is kept) The starter is kept beside it for the character that is being made.
pub fn store_starter(saves: Option<&Path>) {
    let Some(i) = CHOSEN.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let Some(dir) = saves else { return };
    let path = dir.join("newgame-starter.txt");
    let _ = std::fs::create_dir_all(dir);
    match std::fs::write(&path, format!("starter={}\r\n", kh_host::formation::key_of(starter_of_index(i)))) {
        Ok(()) => erlog!("unlock: the starter {} is kept in {} for the character that is being made", starter_of_index(i).name(), path.display()),
        Err(e) => erlog!("unlock: the starter could NOT be kept ({e}): the menu asks it again after the intro"),
    }
}

/// (save.rs, a loaded or respawned character) The unlocks of its party record (none: Sora alone, the old rule).
pub fn on_record(record: &Value) {
    with(|d| {
        let u = Unlocks::from_record(record).unwrap_or_else(|| {
            erlog!("unlock: the save has no unlock record (made before the rule): Sora alone is unlocked");
            Unlocks::legacy()
        });
        if d.unlocks.as_ref() != Some(&u) {
            erlog!("unlock: the character's unlocks: {}", u.describe());
        }
        d.unlocks = Some(u);
        // (CV3, ERT4 fault 7) the starter is still to be asked when the record says so (a reload before the menu)
        d.ask_starter = super::rules::starter_pending(record);
        if d.ask_starter {
            erlog!("unlock: the save's record: THE STARTER IS STILL TO BE ASKED (the choice menu asks it)");
        }
        d.reset_scenes();
    });
}

/// (save.rs, a new character) Its starter: the title's pick (`newgame-starter.txt`, taken), else the in-game choice
/// menu asks it (`ask`: the menu will be shown); Sora until then.
pub fn on_new_game(dir: &Path, ask: bool) {
    let path = dir.join("newgame-starter.txt");
    let picked = std::fs::read_to_string(&path).ok().and_then(|t| t.lines().find_map(|l| l.trim().strip_prefix("starter=").and_then(kh_host::formation::chara_of)));
    if picked.is_some() {
        let _ = std::fs::remove_file(&path);
    }
    with(|d| {
        let starter = picked.unwrap_or(PartyChara::Sora);
        d.unlocks = Some(Unlocks::new_game(starter));
        d.ask_starter = picked.is_none() && ask;
        d.reset_scenes();
        erlog!("unlock: A NEW CHARACTER: {} ({})", d.unlocks.as_ref().map_or(String::new(), Unlocks::describe), if picked.is_some() { "the starter chosen on the title" } else if ask { "the choice menu after the intro asks the starter" } else { "no starter chosen and no choice menu to come: Sora" });
    });
}

/// (save.rs `capture`) The unlocks into the save's party record (the authoritative list: a fight guest is not saved).
pub fn stamp(record: &mut Value) {
    let Ok(guard) = DIRECTOR.lock() else { return };
    if let Some(u) = guard.as_ref().and_then(|d| d.unlocks.as_ref()) {
        u.stamp(record);
    }
    // (CV3, ERT4 fault 7) a starter still to be asked survives a save / reload / respawn before the menu after the intro
    if let Some(obj) = record.as_object_mut() {
        if guard.as_ref().is_some_and(|d| d.ask_starter) {
            obj.insert("starter_pending".into(), Value::Bool(true));
        } else {
            obj.remove("starter_pending");
        }
    }
}

/// (S2 / S4) Sora is staged at Margit: draw the main Sora where he is (he is out of the party).
pub fn sora_staged() -> bool {
    SORA_STAGED.load(Ordering::Relaxed)
}

/// (hud.rs `HudHost::input`) A talk line is up: the pad is the talk window's (Sora takes no stick, no button).
pub fn talk_holds_pad() -> bool {
    TALK_HOLD.load(Ordering::Relaxed)
}

/// (hud.rs `HudHost::input`, every frame) THE PAD FOR THE TALK: Y's down edge is kept for the next party step while the
/// prompt is up (and hidden from Sora: no situation command); while a line is up Sora gets nothing (the window takes A).
pub fn note_pad(stick: &mut glam::Vec2, held: &mut PadButtons) {
    let y = held.has(PadButton::FaceTop);
    let before = Y_BEFORE.swap(y, Ordering::Relaxed);
    if PROMPT_UP.load(Ordering::Relaxed) {
        if y && !before {
            Y_LATCH.store(true, Ordering::Relaxed);
        }
        held.set(PadButton::FaceTop, false);
    }
    if TALK_HOLD.load(Ordering::Relaxed) {
        *stick = glam::Vec2::ZERO;
        *held = PadButtons::NONE;
    }
}

/// (S5) The staged members' taps (bones, sockets, weapon sockets) of the latest step: `Attach::Party { member }` of a
/// staged friend's sound or effect is his (his member id is his runtime's: Donald 0, Goofy 1, as the main party's; a
/// staged friend is never in the main party at the same time).
pub fn stage_taps() -> Vec<(PartyChara, kh_partyhost::MemberTap)> {
    TAPS.lock().map(|t| t.clone()).unwrap_or_default()
}

/// (sora.rs, the ally bodies' wants) THE STAGED FRIENDS' BODIES: S3's pool makes and holds them as for party members
/// (the spirit team: the boss's own AI targets them).
pub fn stage_wants() -> Vec<kh_partyhost::AllyWant> {
    WANTS.lock().map(|w| w.clone()).unwrap_or_default()
}

/// (allybody_live.rs `deliver`) This character is staged (his body's hits are S8's: 0 damage).
pub fn is_staged(c: PartyChara) -> bool {
    STAGED_MASK.load(Ordering::Relaxed) & (1 << c as u8) != 0
}

/// (allybody_live.rs `deliver`) A staged friend's body was hit by the boss: his reaction, 0 damage (applied at the next
/// party step).
pub fn stage_body_hit(h: &crate::allybody::rules::AllyHit, params: &crate::ermap::Params) {
    let mut record = h.record(params, 100, Vec3::ZERO);
    let would = record.damage;
    record.damage = 0.0;
    let mut sora = h.sora_hit(params, 100);
    sora.amount = 0.0;
    if let Ok(mut q) = BODY_HITS.lock() {
        q.push((h.chara, record, sora, h.lost, (would * 100.0) as i32));
    }
}

/// (S4) What to draw of the stage this frame.
pub fn stage_draws() -> Vec<StageDraw> {
    DRAWS.lock().map(|d| d.clone()).unwrap_or_default()
}

/// (sora.rs, the hop of `dbg.hop`) `unlock.goto`'s hops: (metres to go, hops).
pub fn take_hop() -> Option<(Vec3, u32)> {
    HOP.lock().ok()?.take()
}

/// (S1) The list to persist / the friends present (None: no unlock rule yet).
pub fn present() -> Option<Vec<PartyChara>> {
    DIRECTOR.lock().ok()?.as_ref()?.present()
}

/// (party/mod.rs `step_sora`, Sora out of the party) SORA FOUND FIGHTING MARGIT: his stage step (the pilot's pad,
/// the stage's targets: the boss only, nothing reaches the game). None: he is not staged (his output stays).
pub fn stage_sora_step(sora: &mut Sora, dt: f32, world: &ErWorld, fx: &mut crate::fx::Fx) -> Option<Output> {
    if !SORA_STAGED.load(Ordering::Relaxed) {
        // (CV3) THE CAUSE OF "I cannot talk to Donald" (the user's session of 2026-10-16, Goofy leading): Sora out of the
        // party is not stepped, so the battle flag of his staged Margit fight stayed ON for good ("Sora Attack" in every
        // `y:` line; sora-trace.csv: A1313 / A1314 looping), and everything that asked "is the leader fighting?" through
        // it said yes: Donald's prompt was never offered, and every conversation (the crone's) ended "by combat" in the
        // frame it began. Out of the stage nothing of his fight may linger: the flag and its linger go off here.
        let a = sora.actions_mut();
        if a.battle || a.battle_linger > 0.0 {
            a.battle = false;
            a.battle_linger = 0.0;
            erlog!("unlock: Sora is out of the party and not staged: his battle flag (left from his staged fight) is cleared, nobody reads him as fighting (CV3)");
        }
        return None;
    }
    let mut guard = DIRECTOR.lock().ok()?;
    let d = guard.as_mut()?;
    let (spot, yaw, placed) = d.sora_stage?;
    if !placed {
        sora.teleport(Pose { position: spot, yaw });
        d.sora_stage = Some((spot, yaw, true));
        erlog!("unlock: Sora is put at Margit's arena (KH3 ({:.0}, {:.0}, {:.0}), yaw {yaw:.0}): the stage pilot fights him", spot.x, spot.y, spot.z);
    }
    let slot = d.fights.iter().find(|f| f.chara == PartyChara::Sora)?;
    let boss_chr = fight_target(slot).and_then(|p| {
        // SAFETY: the double's pointer (checked by handle) or the original of this frame's walk; checked complete again.
        let chr = unsafe { &*(p as *const ChrIns) };
        game::chr_complete(chr).then_some(chr)
    });
    let info = boss_chr.map(|c| boss_info(c, world));
    let out_before = sora.output();
    let boss = info.map(|b| PilotBoss { position: b.position, radius: b.radius, attacking: boss_chr.is_some_and(|c| attacking(c).is_some()) });
    let pad = d.pilot.pad(dt, out_before.position, out_before.on_floor, boss);
    let mut held = PadButtons::NONE;
    held.set(PadButton::FaceBottom, pad.a);
    held.set(PadButton::FaceRight, pad.b);
    held.set(PadButton::FaceLeft, pad.x);
    // (one-step presses: the pilot's buttons are taps)
    let input = Input { move_stick: pad.stick, camera_yaw: pad.camera_yaw, ..Input::default() }.with_pad(PadState { held, pressed: held, released: PadButtons::NONE });
    let mut targets = StageTargets { boss: info, swallowed: 0, who: Some(PartyChara::Sora) };
    let mut ui = kh_core::host::NullUi;
    let mut host = kh_core::Host { world, targets: &mut targets, audio: &mut fx.audio, effects: &mut fx.effects, ui: &mut ui };
    Some(sora.step_hosted(dt, &input, &mut host))
}

/// The boss as a stage target (KH3 space).
fn boss_info(chr: &ChrIns, world: &ErWorld) -> TargetInfo {
    let physics = &chr.modules.physics;
    let pos = world.frame.pos_er_to_kh(game::v3(&physics.position));
    let (r, h) = (physics.hit_radius.max(0.3) * coords::CM_PER_M, physics.hit_height.max(1.0) * coords::CM_PER_M);
    let yaw = coords::yaw_kh_of_er_quat(game::quat(&physics.orientation));
    let data = &chr.modules.data;
    TargetInfo { id: STAGE_BOSS, position: pos, center: pos + glam::Vec3::Z * h * 0.5, velocity: Vec3::ZERO, yaw, radius: r, half_height: (h * 0.5).max(r), flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY | TargetFlags::PRIORITY_HIGH), hp: data.hp as f32, hp_max: data.max_hp as f32 }
}

/// (party/mod.rs `after_sora_step`, every party step) THE UNLOCK RULE'S STEP: who is in the party, the scenes, the
/// celebration, the testing aids. `pack` / `sheets` = the party host's folders.
#[allow(clippy::too_many_arguments)]
pub fn party_step(core: &mut PartyCore, pack: &Path, sheets: &Path, sora: &mut Sora, out: &Output, dt: f32, world: &ErWorld, targets: &mut ErTargets, fx: &mut crate::fx::Fx, now: f64) {
    let keys = {
        let mut k = KEYS.lock().unwrap_or_else(|e| e.into_inner());
        let taken = k.clone();
        k.shots = ShotValues::default();
        taken
    };
    if !keys.on {
        SORA_STAGED.store(false, Ordering::Relaxed);
        TALK_HOLD.store(false, Ordering::Relaxed);
        PROMPT_UP.store(false, Ordering::Relaxed);
        crate::conv::offer_talk(DONALD_OWNER, false); // (CV3)
        return;
    }
    let mut guard = DIRECTOR.lock().unwrap_or_else(|e| e.into_inner());
    let d = guard.get_or_insert_with(Director::new);
    let ctx = Ctx { pack, sheets, keys, world, dt, now };
    d.step(core, &ctx, sora, out, targets, fx);
}

/// The keys of the latest frame (`frame`), the one-shots fired since the last party step.
#[derive(Clone, Debug, Default)]
struct Keys {
    on: bool,
    log: bool,
    skip_intro: bool,
    leader_down: bool,
    /// (ER boss) `unlock.double_fallback` (0): the boss's double only when the real boss's AI switch is refused.
    double_fallback: bool,
    /// (ALLYJOIN) `unlock.join_ask` (1): a new ally's "join the party?" after his OBTAINED popup.
    join_ask: bool,
    shots: ShotValues,
}

#[derive(Clone, Debug, Default)]
struct ShotValues {
    state: Option<String>,
    give: Option<String>,
    reset: Option<String>,
    starter: Option<String>,
    jump: Option<String>,
    goto: Option<String>,
}

static KEYS: Mutex<Keys> = Mutex::new(Keys { on: false, log: false, skip_intro: true, leader_down: false, double_fallback: false, join_ask: true, shots: ShotValues { state: None, give: None, reset: None, starter: None, jump: None, goto: None } });
static SHOTS: Mutex<Option<Shots>> = Mutex::new(None);

/// (party/mod.rs `PartySlot::frame`, once a frame) The keys and the one-shots of the control file for the next party
/// step (the party step has no control file at hand).
/// (RC) `talk.float_prompt` (default 0): Donald's old floating "Y Talk" over his head as well. By default his talk is
/// KH3's green "Talk" reaction command on the command menu (kh-er hud.rs, erprompt.rs, from `conv::offer_talk`).
static FLOAT_PROMPT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn frame(control: &Control) {
    let mut shots = SHOTS.lock().unwrap_or_else(|e| e.into_inner());
    let s = shots.get_or_insert_with(|| Shots { state: OneShot::new("unlock.state", control), give: OneShot::new("unlock.give", control), reset: OneShot::new("unlock.reset", control), starter: OneShot::new("unlock.starter", control), jump: OneShot::new("unlock.jump", control), goto: OneShot::new("unlock.goto", control) });
    let mut k = KEYS.lock().unwrap_or_else(|e| e.into_inner());
    k.on = on(control);
    k.log = control.on("unlock.log");
    k.skip_intro = control.flag("unlock.skip_intro") != Some(false);
    k.leader_down = crate::partyui::fallen_leader(control).is_some();
    k.double_fallback = control.on("unlock.double_fallback");
    k.join_ask = control.flag("unlock.join_ask") != Some(false); // (ALLYJOIN)
    FLOAT_PROMPT.store(control.on("talk.float_prompt"), std::sync::atomic::Ordering::Relaxed); // (RC)
    let keep = |slot: &mut Option<String>, v: Option<String>| {
        if v.is_some() {
            *slot = v;
        }
    };
    keep(&mut k.shots.state, s.state.fired(control));
    keep(&mut k.shots.give, s.give.fired(control));
    keep(&mut k.shots.reset, s.reset.fired(control));
    keep(&mut k.shots.starter, s.starter.fired(control));
    keep(&mut k.shots.jump, s.jump.fired(control));
    keep(&mut k.shots.goto, s.goto.fired(control));
    // (ER party I) `party.er = 0` / `party.unlocks = 0`: party_step no longer runs, so what a staged fight left is
    // undone here (the boss shown again, its double parked, the staged friend gone from the drawing, the sounds and the
    // bodies' wants); the parked doubles are held hidden while the rule is off
    let off = !k.on;
    drop(k);
    if off {
        if let Ok(mut guard) = DIRECTOR.try_lock() {
            if let Some(d) = guard.as_mut() {
                d.switch_off();
            }
        }
    }
}

struct Ctx<'a> {
    pack: &'a Path,
    sheets: &'a Path,
    keys: Keys,
    world: &'a ErWorld,
    dt: f32,
    now: f64,
}

impl Director {
    /// All scenes back to their start (a load, a new character).
    fn reset_scenes(&mut self) {
        for f in &mut self.fights {
            f.dir.reset();
            f.boss = None;
        }
        self.talk.reset();
        self.guests.clear();
        let (a, b) = (self.staged.take(), self.donald.take());
        self.spare.extend(a.into_iter().chain(b));
        self.sora_stage = None;
        self.celebrate = None;
        self.joins.clear();
        self.join_ask.clear(); // (ALLYJOIN) a load before the answer: he stays as the load has him
        self.intro_said = false;
        SORA_STAGED.store(false, Ordering::Relaxed);
        TALK_HOLD.store(false, Ordering::Relaxed);
        PROMPT_UP.store(false, Ordering::Relaxed);
        self.prompt_up = false;
        crate::conv::offer_talk(DONALD_OWNER, false); // (CV3)
        self.talk_line = None;
        if let Some(id) = self.conv.take() {
            crate::conv::end(id, "the unlock scenes were reset"); // (CV1)
        }
        if let Ok(mut dr) = DRAWS.lock() {
            dr.clear();
        }
    }

    /// (ER party I) THE UNLOCK RULE SWITCHED OFF (`party.er = 0` / `party.unlocks = 0`) WHILE A FIGURE OF IT IS OUT: every
    /// staged fight is undone as when the player walks away (the event's boss shown again where it stands, the double
    /// parked), the scenes reset (staged friends, Donald at the crone, the guests), nothing published for the drawing,
    /// the sounds or S3's pool. Once a frame while off; does nothing when nothing is out (parked doubles are held hidden).
    fn switch_off(&mut self) {
        let mut any = self.staged.is_some() || self.donald.is_some() || self.sora_stage.is_some() || !self.guests.is_empty() || self.celebrate.is_some();
        for k in 0..self.fights.len() {
            let f = &mut self.fights[k];
            // (ER boss) the real boss's AI switch written back as its event left it
            if f.wake.awake() {
                any = true;
                let entity = self.sheet.fight(f.chara).map_or(0, |s| s.boss_entity);
                let name = f.chara.name();
                release_wake(&mut f.wake, chr_of_entity(entity).map(|(_, c)| c), name, "the unlock rule is off");
            }
            let out = f.double.original_hidden || matches!(f.double.state, super::double::State::Asked { .. } | super::double::State::Live { parked: false, .. });
            if out {
                any = true;
                let entity = self.sheet.fight(f.chara).map_or(0, |s| s.boss_entity);
                let original = chr_of_entity(entity).map(|(_, c)| c);
                erlog!("unlock: the unlock rule is off (party.er / party.unlocks = 0): {}'s staged fight is undone", f.chara.name());
                f.double.unstage(original, f.chara.name());
            } else {
                f.double.hold_parked();
            }
            f.boss = None;
        }
        if !any {
            return;
        }
        self.reset_scenes();
        STAGED_MASK.store(0, Ordering::Relaxed);
        if let Ok(mut w) = WANTS.lock() {
            w.clear();
        }
        if let Ok(mut taps) = TAPS.lock() {
            taps.clear();
        }
    }

    fn step(&mut self, core: &mut PartyCore, ctx: &Ctx, sora: &mut Sora, out: &Output, targets: &mut ErTargets, fx: &mut crate::fx::Fx) {
        self.testing_aids(core, ctx);
        let Some(unlocks) = self.unlocks.clone() else { return };
        self.sec += ctx.dt;
        self.sec_total += ctx.dt;
        let log_now = self.sec >= 1.0 && ctx.keys.log;
        if self.sec >= 1.0 {
            self.sec = 0.0;
        }
        // ---- who is in the party
        if let Some(list) = self.present() {
            core.set_unlocked(&list, ctx.now);
        }
        let Some(player) = game::player_mut() else { return };
        let Some(pp) = player_place(player) else { return };
        let readable = flags_readable();
        // ---- Margit's intro skipped while Sora waits for him
        self.margit_intro(&unlocks, &pp, readable, ctx);
        // ---- the hidden / down facts
        let hidden = crate::hud::in_cutscene() || crate::hud::menu_open();
        // the leader down: Sora's death when he leads, a friend leader's KO, the game over S6 saw
        let leader_down = if core.rt.friend_leads() { core.rt.lead.leader_state.1 } else { out.state == kh_core::StateId::Die } || ctx.keys.leader_down;
        // (CV3) the leader's fight whoever leads: Sora's flag only while he is in the active party (conv::leader_combat)
        let in_battle = crate::conv::leader_combat(!core.sora_skipped(), sora.actions().battle, core.rt.current.iter().any(|v| v.in_battle));
        // ---- the fights
        let mut draws = Vec::new();
        for k in 0..self.fights.len() {
            self.fight_step(k, core, ctx, &unlocks, &pp, readable, hidden, leader_down, sora, log_now, fx);
        }
        // ---- the staged friend's stage runtime (his target: the boss's double, else the original)
        if let Some(stage) = self.staged.as_mut() {
            let boss = self.fights.iter().find(|f| f.chara == stage.chara).and_then(fight_target);
            stage_step(stage, boss, ctx, fx);
            draws.extend(stage_draw(stage));
        }
        // ---- 0 damage both ways: the boss's blows on the staged bodies, the staged friends' hits on the boss
        self.body_hits(sora);
        self.fire_shots(ctx, targets);
        // ---- the staged bodies for S3's pool, who is staged
        let mut wants: Vec<kh_partyhost::AllyWant> = Vec::new();
        let mut mask = 0u8;
        if let Some(stage) = self.staged.as_ref() {
            wants.extend(stage.core.feed.wants.iter().filter(|w| w.chara == stage.chara).copied());
            mask |= 1 << stage.chara as u8;
        }
        if self.sora_stage.is_some() {
            let o = sora.output();
            wants.push(kh_partyhost::AllyWant { chara: PartyChara::Sora, feet: o.position, yaw: o.yaw, capsule: kh_partyhost::SORA_CAPSULE, hittable: sora.status().hp > 0, invulnerable: false, ko: sora.status().hp <= 0 });
            mask |= 1 << PartyChara::Sora as u8;
        }
        STAGED_MASK.store(mask, Ordering::Relaxed);
        if let Ok(mut w) = WANTS.lock() {
            *w = wants;
        }
        // ---- Donald at the crone
        self.donald_step(core, ctx, &unlocks, &pp, hidden, in_battle, out, fx, &mut draws, targets);
        // ---- joins where the stage put them
        self.joins_step(core);
        // ---- the celebration
        self.celebrate_step(core, ctx, sora, fx);
        // ---- (ALLYJOIN) the answer to "join the party?"
        self.join_step(core);
        if let Ok(mut dr) = DRAWS.lock() {
            *dr = draws;
        }
        if let Ok(mut taps) = TAPS.lock() {
            taps.clear();
            for s in self.staged.iter().chain(self.donald.iter()) {
                taps.extend(s.core.feed.taps.iter().cloned().map(|tap| (s.chara, tap)));
            }
        }
        SORA_STAGED.store(self.sora_stage.is_some(), Ordering::Relaxed);
        let _ = (ctx.pack, ctx.sheets);
    }

    fn margit_intro(&mut self, unlocks: &Unlocks, pp: &PlayerPlace, readable: bool, ctx: &Ctx) {
        // (the scene of Sora's staging: the fight row whose friend is Sora)
        let Some(scene) = self.sheet.fight(PartyChara::Sora) else { return };
        let Some(intro) = scene.intro_flag else { return };
        if !unlocks.skip_margit_intro() || !ctx.keys.skip_intro || !readable {
            return;
        }
        if flag(scene.defeat_flag) == Some(true) {
            return;
        }
        if flag(intro) == Some(false) {
            let in_map = parse_map(&scene.map).is_some_and(|m| m == pp.block);
            if set_flag(intro) && flag(intro) == Some(true) {
                erlog!("unlock: MARGIT'S INTRO IS SKIPPED: flag {intro} (his first encounter, m10 EVENT 10002860) set ON while Sora waits for him{}", if in_map { " -- BUT THE PLAYER IS IN m10 ALREADY: its script read the flag at the map's load, so the skip holds from the next load of Stormveil (rest at a grace, or reload)" } else { " (Stormveil's script reads it when the map loads: the fog-wall branch, the fight as soon as the player is through)" });
            } else {
                erlog!("!!! unlock: flag {intro} could NOT be set: Margit's intro will play");
            }
        } else if !self.intro_said {
            self.intro_said = true;
            erlog!("unlock: Margit's intro flag {intro} is on (his fight starts at the fog wall)");
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fight_step(&mut self, k: usize, core: &mut PartyCore, ctx: &Ctx, unlocks: &Unlocks, pp: &PlayerPlace, readable: bool, hidden: bool, leader_down: bool, sora: &mut Sora, log_now: bool, _fx: &mut crate::fx::Fx) {
        let chara = self.fights[k].chara;
        let Some(scene) = self.sheet.fight(chara).cloned() else { return };
        if scene.boss_entity == 0 {
            return;
        }
        // (nothing to look for once he is unlocked and the scene is over)
        if unlocks.is_unlocked(chara) && self.fights[k].dir.phase == FightPhase::Done {
            self.fights[k].boss = None;
            return;
        }
        // the boss by entity id (looked for 6 times a second; the pointer is checked every step)
        let slot = &mut self.fights[k];
        if slot.look_in == 0 {
            slot.look_in = 10;
            slot.boss = chr_of_entity(scene.boss_entity).map(|(p, _)| p);
        } else {
            slot.look_in -= 1;
            if let Some(p) = slot.boss {
                // SAFETY: a pointer of a walk at most 10 steps old; checked complete and still the same entity.
                let chr = unsafe { &*(p as *const ChrIns) };
                if !(game::chr_complete(chr) && chr.event_entity_id == scene.boss_entity) {
                    slot.boss = None;
                    slot.look_in = 0;
                }
            }
        }
        let boss = slot.boss.map(|p| {
            // SAFETY: checked above in this step.
            unsafe { &mut *(p as *mut ChrIns) }
        });
        let boss_er = boss.as_ref().map(|c| game::v3(&c.modules.physics.position));
        let place = scene_physics(pp, &scene.map, scene.boss.pos, 5000.0);
        // THE UNIVERSAL RULE: the moment the boss is loaded (and within stage.range) the friend is with it
        let near = boss_er.is_some_and(|b| (b - pp.physics).length() <= self.sheet.stage_range);
        let _ = place;
        let facts = FightFacts {
            unlocked: unlocks.is_unlocked(chara),
            defeated: if readable { flag(scene.defeat_flag) } else { None },
            fog_passed: if readable { flag(scene.fog_passed_flag) } else { None },
            boss_bar: boss.as_ref().is_some_and(|c| on_boss_bar(c)),
            boss_found: boss.is_some(),
            boss_alive: boss.as_ref().is_some_and(|c| c.modules.data.hp > 0 && !c.chr_flags1c5.death_flag()),
            near,
            leader_down,
            hidden,
        };
        if log_now {
            erlog!("unlock: {} scene ({}): {:?} facts {:?}", chara.name(), scene.boss_name, self.fights[k].dir.phase, facts);
        }
        let acts = self.fights[k].dir.step(&facts, ctx.dt);
        let phase = self.fights[k].dir.phase;
        let said = format!("{phase:?}");
        if self.fights[k].said.as_deref() != Some(said.as_str()) {
            erlog!("unlock: {} vs {}: {:?} -> {said} (defeat flag {} = {:?}, fog flag {} = {:?}, boss bar {}, boss {} {}, player {:.0} m from it)", chara.name(), scene.boss_name, self.fights[k].said, scene.defeat_flag, facts.defeated, scene.fog_passed_flag, facts.fog_passed, facts.boss_bar, scene.boss_entity, if facts.boss_found { "found" } else { "not loaded" }, boss_er.or(place).map_or(f32::NAN, |b| (b - pp.physics).length()));
            self.fights[k].said = Some(said);
        }
        for act in acts {
            match act {
                FightAct::Stage => self.stage_fight(chara, &scene, core, ctx, boss_er, sora),
                FightAct::Unstage(why) => {
                    erlog!("unlock: {} leaves the arena ({why})", chara.name());
                    self.unstage(chara);
                    // (ER boss) the real boss's AI switch back as its event left it (a dead boss is left alone)
                    let alive = facts.boss_alive && facts.defeated != Some(true);
                    let original = self.fights[k].boss.filter(|_| alive).map(|p| unsafe { &mut *(p as *mut ChrIns) }); // SAFETY: checked complete in this step
                    release_wake(&mut self.fights[k].wake, original, chara.name(), why);
                    if double_out(&self.fights[k].double) {
                        let original = self.fights[k].boss.map(|p| unsafe { &mut *(p as *mut ChrIns) }); // SAFETY: checked complete in this step
                        self.fights[k].double.unstage(original, chara.name());
                    }
                }
                FightAct::Join => {
                    // (ER boss) THE PLAYER IS IN: the same boss fights on, the event's fight start takes the switch
                    for n in self.fights[k].wake.hand_over() {
                        let _ = n;
                        erlog!("unlock: {} vs {}: THE PLAYER IS IN: the real boss's AI switch is handed to its event (its fight start writes the same switch, the update rate and the boss bar); the SAME boss fights on, no swap, the game's own damage from now on ({} HP had been given back while staged)", chara.name(), scene.boss_name, self.fights[k].kept_hp);
                    }
                    // (the fallback) the event's boss takes its double's place
                    if double_out(&self.fights[k].double) {
                        if let Some(p) = self.fights[k].boss {
                            // SAFETY: checked complete in this step.
                            let original = unsafe { &mut *(p as *mut ChrIns) };
                            self.fights[k].double.hand_over(original, chara.name());
                        }
                    }
                    self.join_fight(chara);
                }
                FightAct::Leave(why) => {
                    erlog!("unlock: {} leaves the party again ({why}): staged anew at the next approach", chara.name());
                    self.guests.retain(|g| *g != chara);
                }
                FightAct::Unlock => {
                    // (ER boss) the boss is dead: nothing written back
                    self.fights[k].wake = AiWake::default();
                    if double_out(&self.fights[k].double) {
                        let original = self.fights[k].boss.map(|p| unsafe { &mut *(p as *mut ChrIns) }); // SAFETY: checked complete in this step
                        self.fights[k].double.unstage(original, chara.name());
                    }
                    self.unstage(chara);
                    self.guests.retain(|g| *g != chara);
                    self.unlock(chara, core, &format!("{} is beaten (defeat flag {})", scene.boss_name, scene.defeat_flag), 6.0);
                }
            }
        }
        // THE STAGED DUEL (ER boss): THE REAL BOSS with its own AI, woken by its own switch (aiswitch.rs, sdkx.rs), the
        // switch and its update rate re-asserted every frame (its event writes them back at a load), its HP held full;
        // the boss's double (double.rs) only as the logged fallback when the write is refused and
        // `unlock.double_fallback = 1`
        let now = self.sec_total;
        let fallback_key = ctx.keys.double_fallback;
        let friend_at = friend_er(chara, &self.staged, sora, ctx);
        let slot = &mut self.fights[k];
        if slot.dir.phase == FightPhase::Staged {
            if let Some(p) = slot.boss {
                // SAFETY: checked complete in this step.
                let original = unsafe { &mut *(p as *mut ChrIns) };
                let who = format!("{} vs {}", chara.name(), scene.boss_name);
                if !slot.wake.refused() {
                    let handle = game::handle_bits(&original.field_ins_handle);
                    let notes = slot.wake.hold(&mut Chr(original), handle);
                    say_wake(&who, &notes, original, scene.boss_entity, chara);
                }
                let use_double = slot.wake.refused() && fallback_key;
                if slot.wake.refused() && !slot.refusal_said {
                    slot.refusal_said = true;
                    erlog!("!!! unlock: {who}: THE REAL BOSS'S AI SWITCH WAS REFUSED ({:?}): {}", slot.wake.state, if fallback_key { "unlock.double_fallback = 1: the boss's DOUBLE is made instead (the old path)" } else { "the boss stands as its event holds it (AI off) and the friend fights it alone; set unlock.double_fallback = 1 to try the boss's double instead" });
                }
                if use_double {
                    if original.npc_param_id != scene.boss_npc_param && slot.double.tries == 0 && matches!(slot.double.state, super::double::State::None) {
                        erlog!("unlock: {}: the loaded boss has NpcParam {} (the sheet says {}): its double is made from the sheet's rows", chara.name(), original.npc_param_id, scene.boss_npc_param);
                    }
                    let rows = Rows { chr_id: scene.boss_chr, npc_param: scene.boss_npc_param, think: scene.boss_think };
                    slot.double.stage(original, rows, now, &who);
                } else {
                    // 0 damage on the real boss: what the friend's body-owned bullets took is given back the same frame
                    let data = &mut original.modules.data;
                    if data.hp > 0 && data.hp < data.max_hp {
                        let back = data.max_hp - data.hp;
                        slot.kept_hp += back as i64;
                        data.hp = data.max_hp;
                        erlog!("unlock: {who}: damage suppressed on the REAL boss: {back} HP given back ({} / {})", data.max_hp, data.max_hp);
                    }
                }
                // what its AI does: one line per attack it starts, and at whom it faces
                let fighter: Option<&ChrIns> = if use_double { slot.double.fighting().map(|d| &*d) } else { Some(&*original) };
                let attack = fighter.and_then(attacking);
                if attack != slot.attack_said {
                    slot.attack_said = attack;
                    if let (Some(id), Some(d)) = (attack, fighter) {
                        let at = game::v3(&d.modules.physics.position);
                        let facing = coords::facing_er(coords::yaw_kh_of_er_quat(game::quat(&d.modules.physics.orientation)));
                        let friend = friend_at;
                        let aim = |p: Vec3| facing.dot(Vec3::new(p.x - at.x, 0.0, p.z - at.z).normalize_or_zero());
                        let (f, pl) = (friend.map_or(-1.0, aim), aim(pp.physics));
                        let target = if f >= 0.8 && f >= pl { "the staged friend" } else if pl >= 0.8 { "the player" } else { "neither (turning)" };
                        erlog!("unlock: {} vs {}: BOSS AI ACTIVE, it attacks (animation {id}): target = {target} (facing the friend {:.0} deg off, the player {:.0} deg off)", chara.name(), scene.boss_name, f.clamp(-1.0, 1.0).acos().to_degrees(), pl.clamp(-1.0, 1.0).acos().to_degrees());
                    }
                }
            }
        } else {
            slot.double.hold_parked();
        }
    }

    fn stage_fight(&mut self, chara: PartyChara, scene: &FightScene, core: &mut PartyCore, ctx: &Ctx, boss_er: Option<Vec3>, sora: &mut Sora) {
        let Some(player) = game::player_mut() else { return };
        let Some(pp) = player_place(player) else { return };
        let boss = boss_er.or_else(|| scene_physics(&pp, &scene.map, scene.boss.pos, 5000.0));
        let fog = boss_er.map(|b| b + (scene.fog.pos - scene.boss.pos)).or_else(|| scene_physics(&pp, &scene.map, scene.fog.pos, 5000.0));
        let (Some(boss), Some(fog)) = (boss, fog) else {
            erlog!("unlock: {} cannot be staged: the arena's places are not in this frame's space", chara.name());
            return;
        };
        let spot_er = ground(ctx.world, toward(boss, fog, self.sheet.friend_offset));
        let anchor_er = ground(ctx.world, toward(boss, fog, self.sheet.anchor_offset));
        let frame = ctx.world.frame;
        let spot = frame.pos_er_to_kh(spot_er);
        let anchor = frame.pos_er_to_kh(anchor_er);
        let yaw = coords::yaw_kh_of_er_dir(boss - spot_er);
        if chara == PartyChara::Sora {
            self.sora_stage = Some((spot, yaw, false));
            let _ = sora;
            erlog!("unlock: SORA IS FOUND FIGHTING {} (staged {:.1} m from it toward the fog; no damage either way until the player is through)", scene.boss_name, self.sheet.friend_offset);
            return;
        }
        match self.stage_for(chara, core, ctx, spot, yaw, anchor) {
            Some(stage) => {
                erlog!("unlock: {} IS FOUND FIGHTING {} (his own party runtime; staged {:.1} m from it toward the fog; no damage either way until the player is through)", chara.name(), scene.boss_name, self.sheet.friend_offset);
                self.staged = Some(stage);
            }
            None => erlog!("!!! unlock: {} could not be staged (no AI pack / no pose)", chara.name()),
        }
    }

    /// A stage runtime for him: a kept one (put at the new place) or a new one.
    fn stage_for(&mut self, chara: PartyChara, core: &PartyCore, ctx: &Ctx, spot: Vec3, yaw: f32, anchor: Vec3) -> Option<Stage> {
        if let Some(k) = self.spare.iter().position(|s| s.chara == chara) {
            let mut s = self.spare.remove(k);
            s.spot = spot;
            s.yaw = yaw;
            s.anchor = anchor;
            s.stand_in.teleport(Pose { position: anchor, yaw });
            s.fresh = true;
            s.row = 0;
            s.ticket = None;
            return Some(s);
        }
        make_stage(chara, core, ctx, spot, yaw, anchor)
    }

    fn unstage(&mut self, chara: PartyChara) {
        if self.staged.as_ref().is_some_and(|s| s.chara == chara) {
            let s = self.staged.take();
            self.spare.extend(s);
        }
        if chara == PartyChara::Sora {
            self.sora_stage = None;
        }
    }

    /// The player is in: the staged friend joins the main party where he stands.
    fn join_fight(&mut self, chara: PartyChara) {
        let at = match chara {
            PartyChara::Sora => self.sora_stage.map(|s| (s.0, s.1)),
            c => self.staged.as_ref().filter(|s| s.chara == c).and_then(|s| s.core.rt.view(c).map(|v| (v.position, v.yaw)).or(Some((s.spot, s.yaw)))),
        };
        erlog!("unlock: THE PLAYER IS IN: {} joins the party for the fight{}", chara.name(), at.map_or(String::new(), |a| format!(" where he stands (KH3 ({:.0}, {:.0}, {:.0}))", a.0.x, a.0.y, a.0.z)));
        if !self.guests.contains(&chara) {
            self.guests.push(chara);
        }
        if let (Some((p, yaw)), true) = (at, chara != PartyChara::Sora) {
            self.joins.push((chara, p, yaw, 120));
        }
        self.unstage(chara);
    }

    /// He is unlocked: the record, the party, the celebration.
    /// `wait`: seconds before the unlock moment (after a boss: the game's own "felled" banner first).
    fn unlock(&mut self, chara: PartyChara, core: &mut PartyCore, why: &str, wait: f32) {
        let Some(u) = self.unlocks.as_mut() else { return };
        if !u.unlock(chara) {
            return;
        }
        erlog!("unlock: *** {} IS UNLOCKED ({why}): {} ***", chara.name(), u.describe());
        if let Some(list) = self.present() {
            core.set_unlocked(&list, 0.0);
        }
        let rows = self.sheet.unlock_rows[chara as usize].clone();
        self.celebrate = Some(Celebrating { c: Celebration::new(chara, rows, self.sheet.banner_s, self.sheet.total_s), ticket: None, started: false, wait });
    }

    fn joins_step(&mut self, core: &mut PartyCore) {
        self.joins.retain_mut(|(c, at, yaw, left)| {
            *left = left.saturating_sub(1);
            if let Some(id) = core.rt.id(*c) {
                if let Some(m) = core.rt.party.member_mut(id) {
                    m.cmc.location = *at;
                    m.cmc.stop_movement();
                    m.yaw = *yaw;
                    erlog!("unlock: {} is in the party where his stage put him", c.name());
                    return false;
                }
            }
            if *left == 0 {
                erlog!("unlock: {} did not appear in the party within 2 s: he stays where the party put him", c.name());
            }
            *left > 0
        });
    }

    fn celebrate_step(&mut self, core: &mut PartyCore, ctx: &Ctx, sora: &mut Sora, fx: &mut crate::fx::Fx) {
        let Some(cel) = self.celebrate.as_mut() else { return };
        if cel.wait > 0.0 {
            cel.wait -= ctx.dt;
            return;
        }
        let chara = cel.c.chara;
        // Sora: his own pose and voice are his core's; the friends: their member
        let id = if chara == PartyChara::Sora { None } else { core.rt.id(chara) };
        if chara != PartyChara::Sora && id.is_none() && cel.c.t < 3.0 {
            // (he spawns at the next route)
            cel.c.t += ctx.dt;
            return;
        }
        let row_done = match (id, cel.ticket) {
            (Some(id), Some(t)) => !matches!(core.rt.party.command_status(id, t), CommandStatus::Running),
            _ => true,
        };
        let acts = cel.c.step(ctx.dt, row_done);
        for act in acts {
            match act {
                CelebAct::Start => {
                    cel.started = true;
                    let at = id.and_then(|i| core.rt.party.view(i)).map(|v| v.position).unwrap_or(sora.output().position);
                    use kh_core::Effects as _;
                    fx.effects.spawn(EffectHandle(0x7E00_0000 | chara as u32), &EffectSpawn { id: EffectId(0), name: intern(&self.sheet.fx_effect), at: Attach::World(at), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
                    let voice = match self.sheet.fx_voice.as_str() {
                        "BattleStart" => Some(kh_core::host::party::VoiceGroup::BattleStart),
                        "FriendLink" => Some(kh_core::host::party::VoiceGroup::FriendLink),
                        _ => None,
                    };
                    if let (Some(group), Some(i)) = (voice, id) {
                        // (ER party S5) the voice stays on HIS head (Attach::Party): only his own voice routes resolve his group
                        // lines (a World attach resolved "BattleStart:1" with Sora's routes)
                        if let Some(play) = kh_core::host::party::voice_play(kh_core::PartyMember(i.0), group, 1, kh_core::host::party::VoiceMode::Replace) {
                            use kh_core::Audio as _;
                            fx.audio.play(kh_core::SoundHandle(0x7E00_0100 | chara as u32), &play);
                        }
                    }
                    let text = self.sheet.joined_text(chara);
                    let se = self.sheet.fx_se;
                    let banner = self.sheet.banner_s;
                    // (HUD2, the user: "new characters (replacing the current popup for unlocking a character)") KH3's OBTAINED
                    // popup with his portrait, KH3 name and the sheet's description, its own sound (static SE 10) and VFX; it
                    // pauses the game until A. A pack without the KeyItem movie: the fanfare and banner as before.
                    let caption = self.sheet.obtained[chara as usize].clone();
                    let shown = crate::hud::with_hud(|hud| {
                        if hud.show_obtained(kh_host::hud::ObtainedKind::Character { chara, caption }) {
                            true
                        } else {
                            hud.play_ui(se);
                            hud.talk_banner(&text, banner)
                        }
                    });
                    // (2026-10-28, the user: "remove the dance team members do first joining the party") held for his rows
                    // only: with none (the sheet's default now) he keeps moving with the party
                    if let (Some(i), false) = (id, cel.c.rows.is_empty()) {
                        if let Some(ai) = core.rt.ai_of(i) {
                            ai.set_stop_ai(true);
                        }
                        core.rt.party.command(i, MemberCommand::Stop);
                    }
                    erlog!("unlock: THE UNLOCK MOMENT of {}: {} at his feet, static SE {se}, his voice, the banner \"{text}\" ({}), rows {:?}", chara.name(), self.sheet.fx_effect, if shown == Some(true) { "shown" } else { "NOT shown: the HUD has no talk window" }, cel.c.rows);
                    // (ALLYJOIN) "join the party?" right after his OBTAINED popup (the HUD shows it once that is closed)
                    if ctx.keys.join_ask {
                        let question = self.sheet.join_question_text(chara);
                        let asked = crate::hud::with_hud(|hud| hud.ask_ally_join(chara, &question));
                        if asked == Some(true) {
                            if !self.join_ask.contains(&chara) {
                                self.join_ask.push(chara);
                            }
                            erlog!("unlock: (ALLYJOIN) {} is asked \"{question}\" after his OBTAINED popup (Yes: the Party Formation screen; No: unlocked, out of the party)", chara.name());
                        } else {
                            erlog!("unlock: (ALLYJOIN) {} could NOT be asked to join ({}): he joins as before", chara.name(), if asked.is_none() { "no HUD" } else { "the UI pack has no Dialog movie" });
                        }
                    }
                }
                CelebAct::Row(row) => {
                    if chara == PartyChara::Sora {
                        // Sora's own clip on his full-body slot (his state may replace it: best effort)
                        match kh_core::AnimId::from_name(&row) {
                            Some(anim) => sora.debug_play_anim(anim, kh_core::PlayArgs { looping: false, root_motion: false, blend_in: 0.2 }),
                            None => erlog!("unlock: Sora has no animation {row}"),
                        }
                    }
                    if let Some(i) = id {
                        let t = core.rt.party.command(i, MemberCommand::PlayMotion { name: intern(&row), looping: false });
                        cel.ticket = Some(t);
                    }
                }
                CelebAct::End => {
                    if let Some(i) = id {
                        if let Some(ai) = core.rt.ai_of(i) {
                            ai.set_stop_ai(false);
                        }
                    }
                    erlog!("unlock: {}'s unlock moment is over: he is a member of the party", chara.name());
                    self.celebrate = None;
                    return;
                }
            }
        }
    }

    /// (ALLYJOIN) THE ANSWER TO "JOIN THE PARTY?" (the HUD's, once): Yes = he stays (the HUD opened the Party Formation
    /// screen itself); No = he leaves the active party (still unlocked), his celebration ends. The HUD no longer asking
    /// him and no answer (a load cleared it): he stays as he is.
    fn join_step(&mut self, core: &mut PartyCore) {
        if self.join_ask.is_empty() {
            return;
        }
        let Some((answer, pending)) = crate::hud::with_hud(|hud| (hud.take_ally_answer(), hud.ally_pending())) else { return };
        if let Some((chara, yes)) = answer {
            if !self.join_ask.contains(&chara) {
                erlog!("unlock: (ALLYJOIN) an answer for {} that nobody waits for: ignored", chara.name());
            } else if yes {
                erlog!("unlock: (ALLYJOIN) {} JOINS (Yes): he stays in the party; the Party Formation screen is the HUD's", chara.name());
            } else {
                let base = core.asked_formation.clone().unwrap_or_else(|| core.rt.lead.formation.clone());
                match super::rules::decline_join(&base, chara) {
                    Some(f) => {
                        erlog!("unlock: (ALLYJOIN) {} does NOT join (No): unlocked, out of the party: the formation is {:?}, leader {} (a save point's Party Formation adds him)", chara.name(), f.active().iter().map(|c| c.name()).collect::<Vec<_>>(), f.leader().name());
                        core.asked_formation = Some(f);
                    }
                    None => erlog!("unlock: (ALLYJOIN) {} does NOT join (No): he is not in the formation {:?} (nothing to change)", chara.name(), base.active().iter().map(|c| c.name()).collect::<Vec<_>>()),
                }
                // his celebration ends now (his AI back): he is leaving the party
                if self.celebrate.as_ref().is_some_and(|c| c.c.chara == chara) {
                    if let Some(i) = core.rt.id(chara) {
                        if let Some(ai) = core.rt.ai_of(i) {
                            ai.set_stop_ai(false);
                        }
                    }
                    self.celebrate = None;
                }
            }
            self.join_ask.retain(|c| *c != chara);
        }
        let lost: Vec<PartyChara> = self.join_ask.iter().copied().filter(|c| !pending.contains(c)).collect();
        for c in lost {
            erlog!("unlock: (ALLYJOIN) the HUD no longer asks {} (a load or the HUD going away) and no answer came: he stays as he is", c.name());
            self.join_ask.retain(|x| *x != c);
        }
    }

    // ------------------------------------------------------------------------------------------- Donald at the crone

    #[allow(clippy::too_many_arguments)]
    fn donald_step(&mut self, core: &mut PartyCore, ctx: &Ctx, unlocks: &Unlocks, pp: &PlayerPlace, hidden: bool, in_battle: bool, out: &Output, fx: &mut crate::fx::Fx, draws: &mut Vec<StageDraw>, _targets: &mut ErTargets) {
        let scene = self.sheet.donald.clone();
        if scene.crone_entity == 0 {
            return;
        }
        if unlocks.is_unlocked(PartyChara::Donald) && self.talk.phase == TalkPhase::Done {
            self.crone = None;
            crate::conv::offer_talk(DONALD_OWNER, false); // (CV3)
            return;
        }
        // the crone by entity id (looked for 6 times a second; the pointer is checked every step)
        if self.crone_in == 0 {
            self.crone_in = 10;
            self.crone = chr_of_entity(scene.crone_entity).map(|(p, _)| p);
        } else {
            self.crone_in -= 1;
        }
        let crone_chr = self.crone.and_then(|p| {
            // SAFETY: a pointer of a walk at most 10 steps old; checked complete and still her before it is read.
            let chr = unsafe { &*(p as *const ChrIns) };
            (game::chr_complete(chr) && chr.event_entity_id == scene.crone_entity).then_some(chr)
        });
        if crone_chr.is_none() {
            self.crone = None;
        }
        let crone = crone_chr.map(|c| (game::v3(&c.modules.physics.position), coords::yaw_kh_of_er_quat(game::quat(&c.modules.physics.orientation))));
        let place = crone.map(|c| c.0).or_else(|| scene_physics(pp, &scene.map, scene.crone.pos, 5000.0));
        let near = place.is_some_and(|p| (p - pp.physics).length() <= scene.show_range);
        // the leader (the body's place = the leader's) to Donald
        let frame = ctx.world.frame;
        let leader_kh = core.rt.leader.and_then(|l| core.rt.party.view(l)).map_or(out.position, |v| v.position);
        let leader_yaw = core.rt.leader.and_then(|l| core.rt.party.view(l)).map_or(out.yaw, |v| v.yaw);
        let donald_kh = self.donald.as_ref().and_then(|s| s.core.rt.view(PartyChara::Donald).map(|v| v.position)).or(self.donald.as_ref().map(|s| s.spot));
        // (CV3) whoever leads (Sora: his output; a friend: his view): the same rule (rules.rs talk_geometry)
        let (dist, facing) = super::rules::talk_geometry(leader_kh, leader_yaw, donald_kh);
        // (CV1) THE TALK IS A CONVERSATION OF THE CONVERSATION SYSTEM (crate::conv; build/sora_tmp/er/IMPL_CV1_STATE.md): the
        // speech bubble, the locks, the conversation camera are its; its end is the talk's last advance (the unlock), a
        // fight or a break ends it and he starts over at the next talk
        let leader_c = core.rt.leader_chara();
        let leader_head = leader_kh + Vec3::Z * crate::conv::height_of(leader_c) * 0.93;
        let mut advanced = std::mem::take(&mut self.talk_advanced);
        if let Some(id) = self.conv {
            for e in crate::conv::take_events(id) {
                match e {
                    crate::conv::ConvEvent::Line { index, anim, voice, .. } => {
                        let n = index.saturating_sub(1);
                        self.talk_line = Some(n);
                        if let Some(s) = self.donald.as_mut() {
                            if let Some(row) = anim.as_deref() {
                                play_row(s, row, false);
                            }
                            // (FACE) his talk face for the line (KH3's field-voice face: the mood's eye row with its blink
                            // timer, the talking lip row), as long as the line is said (the text's length: the lines have
                            // no voice of their own but line 6's shout)
                            if let Some(mid) = s.core.rt.id(PartyChara::Donald) {
                                let text = self.sheet.lines.get(n).map_or("", |l| l.text.as_str());
                                kh_party::face::talk(&mut s.core.rt.party, mid, talk_mood(n, text), kh_party::face::speech_length(text));
                            }
                            if let (Some(v), Some(mid)) = (voice.as_ref(), s.core.rt.id(PartyChara::Donald)) {
                                let group = match v.key.as_str() {
                                    "Damage" => Some((kh_core::host::party::VoiceGroup::Damage, 0)),
                                    "BattleStart" => Some((kh_core::host::party::VoiceGroup::BattleStart, 1)),
                                    _ => None,
                                };
                                if let Some((g, k)) = group {
                                    // (ER party S5) on HIS head (Attach::Party, his stage tap): his own voice routes, not Sora's
                                    if let Some(play) = kh_core::host::party::voice_play(kh_core::PartyMember(mid.0), g, k, kh_core::host::party::VoiceMode::Replace) {
                                        use kh_core::Audio as _;
                                        fx.audio.play(kh_core::SoundHandle(0x7E00_0200), &play);
                                    }
                                }
                            }
                        }
                        erlog!("unlock: Donald's line {} of {} (anim {:?}, voice {:?})", n + 1, self.sheet.lines.len(), anim, voice.as_ref().map(|v| v.key.clone()));
                    }
                    crate::conv::ConvEvent::StopVoice { .. } => {
                        use kh_core::Audio as _;
                        fx.audio.stop(kh_core::SoundHandle(0x7E00_0200), 0.05);
                    }
                    crate::conv::ConvEvent::Ended(crate::conv::EndReason::Script) => {
                        self.conv = None;
                        if let Some(s) = self.donald.as_mut() {
                            s.ticket = None; // (round 4) his talk loop ends with the talk
                        }
                        self.talk_line = None;
                        // the last line was advanced: the talk director's unlock
                        self.talk.phase = TalkPhase::Talking(self.sheet.lines.len().saturating_sub(1));
                        advanced = true;
                    }
                    crate::conv::ConvEvent::Ended(why) => {
                        self.conv = None;
                        self.talk_line = None;
                        if let Some(s) = self.donald.as_mut() {
                            s.ticket = None; // (round 4) back to his angry rows at her
                        }
                        if matches!(self.talk.phase, TalkPhase::Talking(_)) {
                            self.talk.phase = if near { TalkPhase::Angry } else { TalkPhase::Away };
                        }
                        erlog!("unlock: Donald's talk ended before its end ({why:?}): he starts over at the next talk");
                    }
                    _ => {}
                }
            }
        }
        let y = Y_LATCH.swap(false, Ordering::Relaxed);
        // (CV3) another held conversation (an Elden Ring NPC's talk) runs: his prompt waits
        let busy = self.conv.is_none() && crate::conv::running().is_some_and(|(id, owner)| owner != DONALD_OWNER && crate::conv::is_held(id));
        let facts = TalkFacts { unlocked: unlocks.is_unlocked(PartyChara::Donald), near, leader_dist: dist, facing, talk_pressed: y, advanced, hidden, in_battle, busy };
        let was = self.talk.phase;
        let acts = self.talk.step(&facts);
        if self.talk.phase != was {
            erlog!("unlock: Donald at the Finger Reader: {was:?} -> {:?} (leader {dist:.1} m from him, facing {facing}, near {near})", self.talk.phase);
        }
        for act in acts {
            match act {
                TalkAct::Show => {
                    let Some(crone_er) = place else { continue };
                    // (CV3, the user 2026-10-17: "He is way too close to the other NPC, put him where Goofy is in the
                    // picture") ~3 m FROM HER ON THE WALKWAY'S MIDDLE, A LITTLE BACK ALONG THE APPROACH, FACING THE PLAYER
                    // COMING UP: relative to her facing (her live one, read off her character; else her MSB rotation's),
                    // the sheet's spot_dist / spot_angle (rules.rs recruit_spot; round 4: he faces her, spot_face removed); never inside her own talk
                    // reach (crone_reach). The stage's stand-in "leader" stays 3 m BEHIND her.
                    let her_yaw = super::rules::her_yaw_kh(crone.map(|c| c.1), scene.crone.rot_y);
                    let front = coords::facing_er(her_yaw);
                    let from_her = scene.spot_from_her();
                    let (spot_raw, _) = super::rules::recruit_spot(crone_er, her_yaw, from_her, scene.spot_angle);
                    let spot_er = ground(ctx.world, spot_raw);
                    let spot = frame.pos_er_to_kh(spot_er);
                    // (round 4) facing HER, from where he really stands
                    let yaw = coords::yaw_kh_of_er_dir(crone_er - spot_er);
                    let anchor = frame.pos_er_to_kh(crone_er - front * 3.0);
                    match self.stage_for(PartyChara::Donald, core, ctx, spot, yaw, anchor) {
                        Some(mut s) => {
                            s.fresh = true;
                            let off = Vec2::new(spot_er.x - crone_er.x, spot_er.z - crone_er.z).length();
                            erlog!("unlock: DONALD STANDS AT THE FINGER READER (crone {} {}, her facing {:.0} deg (MSB {:.0})); him {off:.2} m from her ({:.0} deg off her facing; her own talk reaches {:.2} m: he is OUTSIDE it), at KH3 ({:.0}, {:.0}, {:.0}) facing her (yaw {yaw:.0}), angry (CV3)", scene.crone_entity, if crone.is_some() { "found" } else { "not loaded: her map place" }, her_yaw, scene.crone.rot_y, scene.spot_angle, scene.crone_reach, spot.x, spot.y, spot.z);
                            self.donald = Some(s);
                        }
                        None => erlog!("!!! unlock: Donald could not be put at the crone (no AI pack / no pose)"),
                    }
                }
                TalkAct::Hide => {
                    { let s = self.donald.take(); self.spare.extend(s); }
                    if let Some(id) = self.conv.take() {
                        crate::conv::end(id, "Donald is gone (the player went away)");
                    }
                    TALK_HOLD.store(false, Ordering::Relaxed);
                    PROMPT_UP.store(false, Ordering::Relaxed);
                    self.prompt_up = false;
                }
                TalkAct::PromptOn => {
                    self.prompt_up = true;
                    PROMPT_UP.store(true, Ordering::Relaxed);
                }
                TalkAct::PromptOff => {
                    self.prompt_up = false;
                    PROMPT_UP.store(false, Ordering::Relaxed);
                    crate::hud::with_hud(|hud| hud.talk_close(true));
                }
                TalkAct::Line(_) if self.conv.is_none() => {
                    // (CV1) the six lines as ONE conversation: he turns to the leader, each line his gesture and voice
                    use crate::conv::{Actor, Begin, Body, Directive, Facing, Line, Participant, Role, Step, Voice};
                    const LEADER: u32 = 0;
                    const DONALD: u32 = 1;
                    let name = self.sheet.lines.first().map_or("Donald".to_string(), |l| l.speaker.clone());
                    let parts = vec![
                        Participant { actor: Actor { id: LEADER, name: leader_c.name().to_string(), role: Role::Leader }, body: Body::Leader, height_cm: None },
                        Participant { actor: Actor { id: DONALD, name, role: Role::Npc }, body: Body::Placed, height_cm: Some(crate::conv::height_of(PartyChara::Donald)) },
                    ];
                    let mut steps = vec![Step::Direct { directive: Directive::Turn { actor: DONALD, face: Facing::Actor(LEADER) }, wait: false }];
                    for line in &self.sheet.lines {
                        steps.push(Step::Line(Line { speaker: DONALD, listener: Some(LEADER), text: line.text.clone(), voice: line.voice.as_ref().map(|v| Voice { key: v.clone(), seconds: Some(1.5) }), anim: line.anim.clone(), ..Line::default() }));
                    }
                    match crate::conv::begin(Begin::new(DONALD_OWNER, parts, steps)) {
                        Ok(id) => {
                            self.conv = Some(id);
                            self.talk_line = Some(0);
                            if let Some(dk) = donald_kh {
                                crate::conv::place(id, DONALD, crate::conv::Pose { feet_kh: dk, yaw: self.donald.as_ref().map_or(0.0, |s| s.yaw), height_cm: crate::conv::height_of(PartyChara::Donald) });
                            }
                            erlog!("unlock: Donald's talk begins (conversation #{id}: {} lines)", self.sheet.lines.len());
                        }
                        Err(e) => {
                            erlog!("!!! unlock: Donald's talk could not begin ({e}): he waits");
                            self.talk.phase = TalkPhase::Angry;
                        }
                    }
                }
                TalkAct::Line(_) => {}
                TalkAct::Close => {
                    self.talk_line = None;
                    if let Some(id) = self.conv.take() {
                        crate::conv::end(id, "the talk was broken off");
                    }
                    TALK_HOLD.store(false, Ordering::Relaxed);
                }
                TalkAct::Unlock => {
                    { let s = self.donald.take(); self.spare.extend(s); }
                    self.unlock(PartyChara::Donald, core, "he told the leader about the bogus reading", 0.3);
                }
            }
        }
        // the angry loop at her; (CV1) HELD ON HIS SPOT, facing her and looking at her face before the talk, turned to the
        // leader (body and head) for the whole talk; the prompt follows him; the conversation is told where he is
        let crone_head = place.map(|c| frame.pos_er_to_kh(c) + Vec3::Z * 95.0);
        let talking = self.conv.is_some();
        if let Some(s) = self.donald.as_mut() {
            // (CV3 round 4, the user: "Donald looked fucked while in conversation and his animation was glitching out ...
            // He was also not looking at the old lady") ONE POSE RULE (rules.rs donald_pose), applied BEFORE and AFTER
            // his stage's step. Before, it was applied only after the step, so every drawn frame showed the clip's root
            // motion and the idle turn toward his focus (which pointed at the leader while his body was held toward the
            // approach) and the next step snapped him back: the jitter and the sideways facing of the user's picture.
            // Now: his spot always; facing HER with his eyes on her face before and after the talk; facing the leader
            // with his eyes on him during it.
            let pose = |s: &mut Stage| {
                let Some(mid) = s.core.rt.id(PartyChara::Donald) else { return };
                let (yaw, look) = super::rules::donald_pose(s.spot, s.yaw, talking, leader_kh, leader_head, crone_head);
                let spot = s.spot;
                if let Some(m) = s.core.rt.party.member_mut(mid) {
                    m.cmc.stop_movement();
                    m.cmc.location.x = spot.x;
                    m.cmc.location.y = spot.y;
                    m.yaw = yaw;
                    if let Some(h) = look {
                        m.focus = kh_party::TargetRef::Point(h);
                    }
                }
            };
            pose(s);
            stage_step(s, None, ctx, fx);
            if talking {
                talk_loop(s);
            } else {
                angry_loop(s, &self.sheet.angry);
            }
            pose(s);
            let view = s.core.rt.view(PartyChara::Donald);
            if let (Some(id), Some(v)) = (self.conv, view.as_ref()) {
                crate::conv::place(id, 1, crate::conv::Pose { feet_kh: v.position, yaw: v.yaw, height_cm: crate::conv::height_of(PartyChara::Donald) });
            }
            let head = view.map(|v| v.position + Vec3::Z * 120.0);
            if let Some(at) = head.and_then(|h| to_stage(frame.pos_kh_to_er(h))) {
                if self.prompt_up && FLOAT_PROMPT.load(std::sync::atomic::Ordering::Relaxed) /* (RC) */ {
                    let text = kh_ui::talk::prompt_text(&self.sheet.prompt);
                    crate::hud::with_hud(|hud| hud.talk_prompt(&text, at));
                }
            }
            draws.extend(stage_draw(s));
        }
        // (CV3) TALK PRIORITY: while his prompt is up it wins Y over Elden Ring's own action prompt (the crone's talk)
        crate::conv::offer_talk(DONALD_OWNER, self.prompt_up && self.donald.is_some());
    }

    // ------------------------------------------------------------------------------------------- the testing aids

    fn testing_aids(&mut self, core: &mut PartyCore, ctx: &Ctx) {
        let ShotValues { state, give, reset, starter, jump, goto } = ctx.keys.shots.clone();
        if state.is_some() {
            erlog!("unlock: unlock.state: {} | guests {:?} | fights {} | Donald {:?} | staged {} | Sora staged {} | celebrating {:?} | present {:?}", self.unlocks.as_ref().map_or("not known yet (no character loaded)".to_string(), Unlocks::describe), self.guests, self.fights.iter().map(|f| format!("{} vs {} {:?} (real boss AI switch {:?}, HP given back {}; fallback double {:?}, tries {}, HP given back {}, original hidden {})", f.chara.name(), f.scene.boss_name, f.dir.phase, f.wake.state, f.kept_hp, f.double.state, f.double.tries, f.double.kept_hp, f.double.original_hidden)).collect::<Vec<_>>().join("; "), self.talk.phase, self.staged.as_ref().map_or("none".to_string(), |s| s.chara.name().to_string()), self.sora_stage.is_some(), self.celebrate.as_ref().map(|c| c.c.chara), self.present());
        }
        if let Some(who) = give.as_deref().and_then(chara_word) {
            erlog!("unlock: unlock.give = {}: unlocked by hand", who.name());
            self.unlock(who, core, "unlock.give (testing aid)", 0.0);
        }
        if let Some(text) = reset {
            let list: Vec<PartyChara> = if text.trim() == "all" { ALL.to_vec() } else { chara_word(&text).into_iter().collect() };
            for c in list {
                let locked = self.unlocks.as_mut().is_some_and(|u| u.lock(c));
                self.guests.retain(|g| *g != c);
                self.unstage(c);
                if c == PartyChara::Donald {
                    self.talk.reset();
                    { let s = self.donald.take(); self.spare.extend(s); }
                }
                if let Some(f) = self.fights.iter_mut().find(|f| f.chara == c) {
                    f.dir.reset();
                }
                erlog!("unlock: unlock.reset = {}: {} (its scene starts afresh)", c.name(), if locked { "locked again" } else { "not locked (the starter, or he was locked)" });
            }
        }
        if let Some(c) = starter.as_deref().and_then(chara_word) {
            self.unlocks = Some(Unlocks::new_game(c));
            self.reset_scenes();
            erlog!("unlock: unlock.starter = {}: a new game's unlocks ({}); nothing of the game is changed", c.name(), self.unlocks.as_ref().map_or(String::new(), Unlocks::describe));
        }
        if let Some(text) = jump {
            let (who, step) = text.split_once(':').unwrap_or((text.as_str(), ""));
            match (chara_word(who), step.trim()) {
                (Some(PartyChara::Donald), "talk") => {
                    self.talk.phase = TalkPhase::Prompt;
                    self.prompt_up = true;
                    PROMPT_UP.store(true, Ordering::Relaxed);
                    erlog!("unlock: unlock.jump = donald:talk: the prompt is up (Y talks; he must be near)");
                }
                (Some(c), "won") => {
                    self.unstage(c);
                    self.unlock(c, core, "unlock.jump won (testing aid)", 0.0);
                }
                (Some(c), "stage") | (Some(c), "fight") => {
                    if let Some(f) = self.fights.iter_mut().find(|f| f.chara == c) {
                        f.dir.reset();
                        erlog!("unlock: unlock.jump = {}:{}: the scene decides afresh now (it stages when the player is within range of the loaded boss{})", c.name(), step, if step == "fight" { "; the fight follows the game's own fog flag" } else { "" });
                    }
                }
                _ => erlog!("unlock: unlock.jump = {text}: not understood (goofy|sora:stage|fight|won, donald:talk|won)"),
            }
        }
        if let Some(who) = goto.as_deref().and_then(chara_word) {
            self.goto(who);
        }
    }

    fn goto(&mut self, who: PartyChara) {
        let Some(player) = game::player_mut() else { return };
        let Some(pp) = player_place(player) else { return };
        let (map, at) = match who {
            PartyChara::Donald => (self.sheet.donald.map.clone(), self.sheet.donald.crone.pos + Vec3::new(0.0, 0.0, 3.0)),
            c => match self.sheet.fight(c) {
                Some(s) => (s.map.clone(), s.approach),
                None => return,
            },
        };
        // a loaded boss places its own map's points (the player may be in another area: the open world before Stormveil)
        let live = self.sheet.fight(who).and_then(|s| {
            let boss = self.fights.iter().find(|f| f.chara == who).and_then(|f| f.boss)?;
            // SAFETY: a pointer of the latest step's walk; checked complete and still the boss before it is read.
            let chr = unsafe { &*(boss as *const ChrIns) };
            (game::chr_complete(chr) && chr.event_entity_id == s.boss_entity).then(|| game::v3(&chr.modules.physics.position) + (s.approach - s.boss.pos))
        });
        match live.or_else(|| scene_physics(&pp, &map, at, 3000.0)) {
            Some(p) => {
                let d = Vec3::new(p.x - pp.physics.x, 0.0, p.z - pp.physics.z);
                let hops = (d.length() / 28.0).ceil().max(1.0) as u32;
                *HOP.lock().unwrap_or_else(|e| e.into_inner()) = Some((d / hops as f32, hops));
                HOP_UNTIL.store(now_ms() + hops as u64 * 600 + 2000, Ordering::Relaxed);
                erlog!("unlock: unlock.goto = {}: {:.0} m to the scene's approach point in {hops} hop(s) (dbg.hop's mover)", who.name(), d.length());
            }
            None => erlog!("unlock: unlock.goto = {}: the scene's map {map} is not this area / within 3 km: travel near it first (a grace close to it), then ask again", who.name()),
        }
    }
}


// ------------------------------------------------------------------------------------------------- a stage runtime

/// A staged friend's own party runtime: the friend and a stand-in Sora who leads (never stepped, never drawn) at
/// `anchor`; his AI follows the stand-in and fights what the stage's targets list.
fn make_stage(chara: PartyChara, main: &PartyCore, ctx: &Ctx, spot: Vec3, yaw: f32, anchor: Vec3) -> Option<Stage> {
    let rows = |name: &str| -> Vec<Value> { std::fs::read_to_string(ctx.sheets.join(name)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default() };
    let test = kh_partyhost::PartyTest::from_rows(&rows("party-test.json"));
    let weapons = kh_partyhost::PartyWeapon::from_rows(&rows("party-weapons.json"));
    let config = kh_partyhost::RuntimeConfig { wanted: true, test, weapons, ai_pack: main.rt.ai_pack.clone(), ai_pack_source: None };
    let mut core = Box::new(PartyCore::new(config, HostRules::DEFAULT));
    core.sink = Some(|line: &str| crate::log::write(&format!("unlock-stage: {line}")));
    let id = kh_partyhost::poser::draw_id(chara);
    let npc = kh_partyhost::poser::npc_of(id)?;
    let dir = ctx.pack.join("party").join(npc);
    let read = |p: &Path| -> Option<Value> { std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()) };
    let index = read(&ctx.pack.join("party").join("party.json")).unwrap_or(Value::Null);
    let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(id as u64)).cloned().unwrap_or(Value::Null);
    let manifest = read(&dir.join("manifest.json"))?;
    let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir).ok()?, kh_anim::Settings::default()).ok()?;
    let member = kh_partyhost::PosedMember::new(id, animator, &manifest, &entry).ok()?;
    core.poser.members.insert(id, member);
    core.asked_formation = Formation::new(&[PartyChara::Sora, chara], PartyChara::Sora).ok();
    let mut stand_in = Sora::new(kh_core::SORA_PARAMS, Pose { position: anchor, yaw });
    stand_in.actions_mut().battle = true;
    Some(Stage { chara, core, stand_in, anchor, spot, yaw, fresh: true, row: 0, ticket: None })
}

/// The leg IK of a stage member against the game's world.
struct StageIk<'a>(&'a ErWorld);

impl kh_anim::IkWorld for StageIk<'_> {
    fn sweep_sphere(&self, start: Vec3, end: Vec3, radius: f32) -> Option<kh_anim::SweepHit> {
        let hit = crate::sweep::sweep_first(self.0, start, end, radius, radius, &self.0.opts.get())?;
        Some(kh_anim::SweepHit { time: hit.time, impact_point: hit.position, impact_normal: hit.impact_normal, start_penetrating: hit.start_penetrating })
    }
}

/// One step of a stage runtime (ER.0's order with the stand-in as "the player").
fn stage_step(s: &mut Stage, boss: Option<*mut ChrIns>, ctx: &Ctx, fx: &mut crate::fx::Fx) {
    let fresh = std::mem::take(&mut s.fresh);
    s.core.frame(FrameIn { on: true, fresh, relocated: false, hidden: None, body_action: false, body: None, pose: true, trace: false, dt: ctx.dt, now: ctx.now });
    let boss_info = boss.and_then(|p| {
        // SAFETY: a pointer of this step's check (fight_step).
        let chr = unsafe { &*p };
        game::chr_complete(chr).then(|| boss_info(chr, ctx.world))
    });
    let mut targets = StageTargets { boss: boss_info, swallowed: 0, who: Some(s.chara) };
    let out = s.stand_in.output();
    let _ = s.core.route(&mut s.stand_in, &Input::default(), ctx.dt, ctx.world, &targets, ctx.now);
    s.core.step(&mut s.stand_in, &out, ctx.dt, TargetId::NONE, ctx.world, &StageIk(ctx.world), &mut targets, &mut fx.audio, &mut fx.effects, None, ctx.now);
    // the first step put him beside the stand-in: he goes to his spot
    if fresh {
        if let Some(id) = s.core.rt.id(s.chara) {
            if let Some(m) = s.core.rt.party.member_mut(id) {
                m.cmc.location = s.spot;
                m.cmc.stop_movement();
                m.yaw = s.yaw;
            }
        }
    }
    let _ = s.anchor;
}

// ------------------------------------------------------------------------------------------------- (ER boss) the AI switch

/// The real boss's switch and update rate through `crate::sdkx` (aiswitch.rs's `BossAi`).
struct Chr<'a>(&'a mut ChrIns);

impl Chr<'_> {
    fn rate_of(chr: &ChrIns) -> Rate {
        let r = crate::sdkx::update_rate(chr);
        Rate { mode: r.mode, override_: r.override_, fixed: r.fixed }
    }
}

impl BossAi for Chr<'_> {
    fn read(&self) -> Option<Fields> {
        let ai_on = crate::sdkx::ai_on(self.0)?;
        Some(Fields { ai_on, rate: Chr::rate_of(self.0) })
    }
    fn write_ai(&mut self, on: bool) -> bool {
        crate::sdkx::set_ai_on(self.0, on)
    }
    fn write_rate(&mut self, r: Rate) {
        crate::sdkx::set_update_rate(self.0, crate::sdkx::UpdateRate { mode: r.mode, override_: r.override_, fixed: r.fixed });
    }
}

fn rate_text(r: &Rate) -> String {
    format!("+0xB4 = {}, +0xB8 = {}, +0x1C4 bit 0 = {}", r.mode, r.override_, r.fixed as u8)
}

/// The switch's notes in the log (a fight with the game: the first 5, then every 100th).
fn say_wake(who: &str, notes: &[Note], boss: &ChrIns, entity: u32, friend: PartyChara) {
    for n in notes {
        match n {
            Note::On { before, after } => {
                let byte = crate::sdkx::ai_byte_value(boss).map_or("?".to_string(), |b| format!("{b:#04x}"));
                // the friend's body as the boss's AI sees it (team, its own switch): S3's pool
                let body = crate::allybody::handle(friend).map(|h| game::handle_bits(&h));
                let mut body_text = "no Elden Ring body yet (S3's pool makes it)".to_string();
                if let Some(bits) = body {
                    each_chr(&mut |p| {
                        // SAFETY: a character-set entry pointer, checked plausible; read after completeness.
                        let c = unsafe { &*(p as *const ChrIns) };
                        if game::chr_complete(c) && game::handle_bits(&c.field_ins_handle) == bits {
                            body_text = format!("his body {bits:#x}: team {}, its own AI switch {}", c.team_type, crate::sdkx::ai_on(c).map_or("not readable".to_string(), |o| (o as u8).to_string()));
                        }
                    });
                }
                erlog!("unlock: {who}: REAL BOSS AI ON (field ChrIns(entity {entity}).chr_ctrl + 0xE8 bit 5 (0x20) = {} (the byte now {byte}), was {}; update rate {} (was {}): the writes of EMEVD 2004[1] SetCharacterAIState(on) + 2004[34] SetNetworkUpdateRate, its event NOT run: no boss bar, no fog lock, no music); the boss's own AI fights {}'s spirit-team body ({body_text}); re-asserted every frame while staged", after.ai_on as u8, before.ai_on as u8, rate_text(&after.rate), rate_text(&before.rate), friend.name());
            }
            Note::AiFought(n) if *n <= 5 || n % 100 == 0 => erlog!("unlock: {who}: THE GAME TURNED THE REAL BOSS'S AI OFF AGAIN ({n} times so far: its event's SetCharacterAIState(off) at a load, or another writer): written on again"),
            Note::RateFought(n) if *n <= 5 || n % 100 == 0 => erlog!("unlock: {who}: the game changed the real boss's update rate again ({n} times so far, now {}): written to the fight start's again", rate_text(&Chr::rate_of(boss))),
            Note::AiFought(_) | Note::RateFought(_) => {}
            Note::Refused(why) => erlog!("!!! unlock: {who}: the real boss's AI switch: REFUSED ({why})"),
            Note::NewBoss => erlog!("unlock: {who}: a new character stands behind entity {entity} (a load made it again): its AI switch is written afresh"),
            Note::HandedOver => {}
            Note::Released(f) => erlog!("unlock: {who}: the real boss's AI switch is written back as its event left it (bit 5 = {}, update rate {})", f.ai_on as u8, rate_text(&f.rate)),
        }
    }
}

/// The staging ended without a fight: the real boss as its event left it (`boss`: the same entity's character now).
fn release_wake(wake: &mut AiWake, boss: Option<&mut ChrIns>, who: &str, why: &str) {
    let handle = boss.as_ref().map(|c| game::handle_bits(&c.field_ins_handle));
    let notes = match boss {
        Some(c) => wake.release(Some(&mut Chr(c)), handle),
        None => wake.release(None, None),
    };
    for n in notes {
        if let Note::Released(f) = n {
            erlog!("unlock: {who}: the real boss's AI switch is written back as its event left it ({why}): bit 5 = {}, update rate {}", f.ai_on as u8, rate_text(&f.rate));
        }
    }
}

/// The fallback double is out (asked, fighting, or its original hidden).
fn double_out(d: &Double) -> bool {
    d.original_hidden || matches!(d.state, super::double::State::Asked { .. } | super::double::State::Live { parked: false, .. })
}

/// The staged boss a friend fights: the REAL boss (its own AI, woken by its switch), or the fallback double fighting.
fn fight_target(slot: &FightSlot) -> Option<*mut ChrIns> {
    slot.double.fighting().map(|d| d as *mut ChrIns).or(slot.boss.map(|p| p as *mut ChrIns))
}

/// The staged friend's place (ER space).
fn friend_er(chara: PartyChara, staged: &Option<Stage>, sora: &Sora, ctx: &Ctx) -> Option<Vec3> {
    let kh = match chara {
        PartyChara::Sora => Some(sora.output().position),
        c => staged.as_ref().filter(|s| s.chara == c).and_then(|s| s.core.rt.view(c)).map(|v| v.position),
    }?;
    Some(ctx.world.frame.pos_kh_to_er(kh))
}

impl Director {
    /// THE BOSS'S BLOWS ON A STAGED BODY: his reaction, 0 damage (the game's HP loss of his body suppressed).
    fn body_hits(&mut self, sora: &mut Sora) {
        let hits: Vec<_> = BODY_HITS.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
        for (c, record, sora_hit, lost, would) in hits {
            match c {
                PartyChara::Sora if self.sora_stage.is_some() => sora.take_hit(sora_hit),
                c => match self.staged.as_mut().filter(|s| s.chara == c) {
                    Some(s) => {
                        let mut record = record;
                        if let Some(v) = s.core.rt.view(c) {
                            record.position = v.position + Vec3::Z * 60.0;
                        }
                        s.core.rt.receive_hit(c, &record);
                    }
                    None => continue,
                },
            }
            erlog!("unlock: the boss hits {}'s body ({lost} Elden Ring HP): damage suppressed -> 0 (it would have been {:.0}% of his HP); his reaction plays", c.name(), would as f32 / 100.0);
        }
    }

    /// THE STAGED FRIENDS' HITS ON THE BOSS: fired by their own bodies (a non-player owner: no player hit for its event,
    /// no hate on the player), one bullet a frame, the HP it takes given back by the double's rule.
    fn fire_shots(&mut self, ctx: &Ctx, targets: &mut ErTargets) {
        let mut q = match HITS_OUT.lock() {
            Ok(q) => q,
            Err(_) => return,
        };
        if q.is_empty() {
            return;
        }
        if !targets.bullets.available() || !targets.bullets.free_slot() {
            if q.len() > 4 {
                q.remove(0);
            }
            return;
        }
        let shot = q.remove(0);
        drop(q);
        let Some(slot) = self.fights.iter().find(|f| f.chara == shot.who) else { return };
        let Some(boss) = fight_target(slot) else { return };
        // SAFETY: the double (checked by handle) or the original checked in this step.
        let boss = unsafe { &*boss };
        if !game::chr_complete(boss) {
            return;
        }
        let Some(body) = crate::allybody::handle(shot.who) else {
            static TOLD: AtomicBool = AtomicBool::new(false);
            if !TOLD.swap(true, Ordering::Relaxed) {
                erlog!("unlock: {}'s hit on the boss: he has no Elden Ring body yet (S3's pool): the hit stays KH3's only (no player bullet ever)", shot.who.name());
            }
            return;
        };
        let frame = ctx.world.frame;
        let dir = coords::dir_kh_to_er(shot.dir).normalize_or(Vec3::Z);
        let contact = frame.pos_kh_to_er(shot.at);
        let from = contact - dir * targets.shape.back;
        let s = &self.sheet;
        let attack = crate::bullet::Attack { phys: s.bullet_phys, poise: s.bullet_poise * shot.power as f32, stamina: s.bullet_stamina, level: (shot.power.clamp(1, 3) as u8) + 1, carrier: Default::default(), sp_attribute: None, obj: None };
        match targets.bullets.fire_owned(body, Some(boss.field_ins_handle), from, dir, attack, targets.shape) {
            Ok(row) => erlog!("unlock: {}'s hit '{}' on the staged boss is fired by HIS BODY (bullet row {row}, attack {}, poise {:.0}): damage suppressed (its HP is given back), the reaction is the game's", shot.who.name(), shot.name, attack.phys, attack.poise),
            Err(why) => erlog!("unlock: {}'s hit on the staged boss could not be fired by his body ({why}): no player bullet instead", shot.who.name()),
        }
    }
}

/// Donald's angry rows one after the other (his AI held).
fn angry_loop(s: &mut Stage, rows: &[String]) {
    if rows.is_empty() {
        return;
    }
    let Some(id) = s.core.rt.id(s.chara) else { return };
    if let Some(ai) = s.core.rt.ai_of(id) {
        ai.set_stop_ai(true);
    }
    let done = s.ticket.is_none_or(|t| !matches!(s.core.rt.party.command_status(id, t), CommandStatus::Running));
    if done {
        let row = rows[s.row % rows.len()].clone();
        s.row += 1;
        play_row(s, &row, false);
    }
}

/// (CV3 round 4) Between his lines' gestures Donald keeps KH3's talk loop (a gesture ends -> the loop blends in; the
/// next line's gesture blends over it), never an idle snap.
fn talk_loop(s: &mut Stage) {
    let Some(id) = s.core.rt.id(s.chara) else { return };
    let done = s.ticket.is_none_or(|t| !matches!(s.core.rt.party.command_status(id, t), CommandStatus::Running));
    if done {
        play_row(s, super::rules::DONALD_TALK_LOOP, true);
    }
}

/// (FACE) The mood of Donald's recruit line `n` (0-based) for his talk face (HOST CHOICE over KH3's rows, the lines are
/// ours: KH3's own talk lines use DE / SM): an indignant line ("phooey", "bogus") is Angry, a cheerful one ("Shucks",
/// "comin' with ya") Joy, the others Default.
fn talk_mood(_n: usize, text: &str) -> kh_party::face::Mood {
    let t = text.to_ascii_lowercase();
    if ["phooey", "bogus", "fake", "hmph"].iter().any(|w| t.contains(w)) && !t.contains("comin'") {
        kh_party::face::Mood::Angry
    } else if ["shucks", "comin' with", "ha!", "hooray"].iter().any(|w| t.contains(w)) {
        kh_party::face::Mood::Joy
    } else {
        kh_party::face::Mood::Default
    }
}

fn play_row(s: &mut Stage, row: &str, looping: bool) {
    let Some(id) = s.core.rt.id(s.chara) else { return };
    if let Some(ai) = s.core.rt.ai_of(id) {
        ai.set_stop_ai(true);
    }
    s.ticket = Some(s.core.rt.party.command(id, MemberCommand::PlayMotion { name: intern(row), looping }));
}

fn stage_draw(s: &Stage) -> Option<StageDraw> {
    let id = kh_partyhost::poser::draw_id(s.chara);
    s.core.poser.pose(id, 1.0).map(|posed| StageDraw { chara: s.chara, posed, before: s.core.poser.pose(id, 0.0) /* (INT) */ })
}
