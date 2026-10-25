//! (ER party S3) The game half of `allybody.rs` (declared there through `#[path]`): the pool of invisible Elden Ring
//! bodies, the debug character creator, the per-frame writes, the body's damage-in, the stage-2
//! bullet owner and the go / no-go test's evidence. Game thread only. UNTESTED IN GAME.
//!
//! SAFETY RULES OF THIS FILE: a body's pointer is followed only after it was found THIS frame in the game's debug
//! character set (or, for a body the game keeps elsewhere, in its character sets by handle) with the handle it had
//! when it was made, and `game::chr_complete` holds. A body that fails that is `Lost` and never touched again. Param
//! tables are read behind `catch_unwind`. The debug creator is only written when its pointer is plausible and it is
//! not busy (its `spawn` flag clear).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{BuddyParam, CSDebugChrCreator, ChrDebugSpawnRequest, ChrIns, FieldInsHandle, NpcParam, PlayerIns, SoloParamRepository, SpEffectParam, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::Vec3;
use kh_core::TargetId;
use kh_host::party::PartyChara;
use kh_partyhost::AllyWant;

use super::bodytest::{Ev, Outcome, St, Test};
use super::rules::{self, Action, AllyHit, BodyRoute, BuddyRow, HpRead, Pick, Sheet, SlotState, SlotView, WantView};
use crate::control::{Control, OneShot};
use crate::coords::{self, Frame, CM_PER_M};
use crate::erlog;
use crate::game::{self, plausible};
use crate::havok::{locate, Located};
use crate::passive::{self, Switches};
use crate::status_rules::{self as sr, Dot, Loss};
use crate::targets::{Enemy, ErTargets};
use crate::word;

/// The game's own "may this character target that one" (erfps2's pattern; targets.rs locates the same function for
/// Sora's targets: located here again so that targets.rs keeps one marked line of this stream).
pub(crate) const CAN_TARGET: Located = Located { name: "chr can target (ally bodies)", pattern: "56 48 83 ec 30 48 8b f2 48 85 c9 74 ? 48 85 d2 74 ? 48 3b ca", capture: 0, rva_2_7_1_0: 0x0051_b611, rva_2_7_2_0: 0x0051_b8b1 /* (PORT) */ };
static CAN_TARGET_FN: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();

/// `can_target(a, b)`: may `a` target `b` (`None`: the function was not located).
fn can_target(a: &ChrIns, b: &ChrIns) -> Option<bool> {
    let f = (*CAN_TARGET_FN.get_or_init(|| locate(&CAN_TARGET)))?;
    // SAFETY: the game's own function on two live, complete characters (erfps2 and targets.rs call it the same way).
    let raw = unsafe {
        let f: unsafe extern "C" fn(*const ChrIns, *const ChrIns) -> u32 = std::mem::transmute(f);
        f(a, b)
    };
    Some(raw & 0xFF != 0)
}

/// What the host hands the pool each frame (sora.rs pre_physics, after Sora's steps and his damage-in).
pub struct Ctx<'a> {
    pub player: &'a PlayerIns,
    pub frame: Frame,
    pub targets: &'a mut ErTargets,
    pub control: &'a Control,
    /// The leader's feet (KH3 space, cm) and yaw: Sora's while he leads (S2 hands in the leading friend's).
    pub leader_feet: Vec3,
    pub leader_yaw: f32,
    pub dt: f32,
    /// Sora's simulation is paused (a menu, the game's pause): no spawns.
    pub paused: bool,
    /// S1's want list of the latest party step (`PartyCore::feed.wants`); `None` = no party runs (party.er = 0, the
    /// party host not made): the go/no-go test then uses two stand-ins.
    pub wants: Option<Vec<AllyWant>>,
}

/// `party.bodies`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Nothing at all (`party.er = 0` or `party.bodies = 0`).
    Off,
    /// The bodies (default). (A hit on the leader always stays the leader's: the F1 redirection fallback and its
    /// `party.bodies = redirect` value were REMOVED BY THE USER, 2026-10-15.)
    Bodies,
}

impl Mode {
    fn of(c: &Control) -> Mode {
        if c.flag("party.er") == Some(false) {
            return Mode::Off;
        }
        match c.str("party.bodies").map(str::trim) {
            Some("0") | Some("off") | Some("false") => Mode::Off,
            _ => Mode::Bodies,
        }
    }
}

/// `party.bullet_owner` (stage 2, U14): whose hits are fired by their body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Off,
    Donald,
    Friends,
    All,
}

impl Owner {
    fn of(c: &Control) -> Owner {
        // (AGGRO) ON BY DEFAULT (`all`: every AI ally's hits are fired by his own body; the leader's stay the player's):
        // Elden Ring's enemies weigh damage into their target choice (NpcThinkParam `targetSysDmgEffectRate`, 100 % on 2114
        // of 2215 rows), so while every friend's hit was the PLAYER's bullet each of them pulled the enemy back onto the
        // leader. `0` / `off`: as before (all player bullets).
        match c.str("party.bullet_owner").map(str::trim) {
            Some("1") | Some("on") | Some("true") | Some("friends") => Owner::Friends,
            Some("donald") => Owner::Donald,
            Some("0") | Some("off") | Some("false") => Owner::Off,
            _ => Owner::All,
        }
    }
    fn takes(self, c: PartyChara) -> bool {
        match self {
            Owner::Off => false,
            Owner::Donald => c == PartyChara::Donald,
            Owner::Friends => c != PartyChara::Sora,
            Owner::All => true,
        }
    }
}

/// What a body is made from.
#[derive(Clone, Debug, PartialEq)]
struct Spec {
    route: BodyRoute,
    chr_id: i32,
    npc_param: i32,
    think: i32,
    init: i32,
    team: u8,
    buddy: Option<u32>,
}

impl Spec {
    fn label(&self) -> String {
        format!("{} c{:04} (NpcParam {}, think {}, ChrInit {}{}, team {})", self.route.name(), self.chr_id, self.npc_param, self.think, self.init, self.buddy.map_or(String::new(), |b| format!(", BuddyParam {b}")), self.team)
    }
}

struct Slot {
    chara: Option<PartyChara>,
    state: SlotState,
    ptr: usize,
    handle: u64,
    /// Found in the debug set (else in the block sets by handle).
    in_debug_set: bool,
    spec: Spec,
    own_base: i32,
    own_max: i32,
    written_base: i32,
    settle: u8,
    raise_checked: bool,
    hp_written: Option<i32>,
    /// What pre_physics wrote (Elden Ring space) and the yaw: post_physics writes the same.
    target: Option<(Vec3, f32)>,
    /// The last position written (for the drift measure).
    written: Option<Vec3>,
    validated: u64,
    born: f32,
    check_at: f32,
    target_no: u32,
    can: Option<bool>,
    dots: Vec<i32>,
    dots_at: f32,
    hits: u32,
    resets: u32,
    throw_on: bool,
    throw_drift: f32,
    drift_max: f32,
    /// (the user's KO rule) knocked out since (clock), at his feet (KH3); the enemies' can_target asked after the switch.
    ko: Option<(f32, Vec3)>,
    ko_checked: bool,
    revived_at: Option<f32>,
    /// (ER party I) the ground under the body (his footsteps' surface; S5's request)
    ground: rules::GroundRead,
    /// (ER party I) the AI switch: what we last wanted, how often the game put it back, the byte as read this frame
    ai_said: Option<bool>,
    ai_resets: u32,
    ai_now: Option<u8>,
}

impl Slot {
    fn placeholder(chara: Option<PartyChara>, spec: Spec, now: f32) -> Slot {
        Slot { chara, state: SlotState::Spawning, ptr: 0, handle: 0, in_debug_set: true, spec, own_base: 0, own_max: 0, written_base: 0, settle: 4, raise_checked: false, hp_written: None, target: None, written: None, validated: 0, born: now, check_at: now + 0.5, target_no: 0, can: None, dots: Vec::new(), dots_at: 0.0, hits: 0, resets: 0, throw_on: false, throw_drift: 0.0, drift_max: 0.0, ko: None, ko_checked: false, revived_at: None, ground: rules::GroundRead::default() /* (ER party I) */, ai_said: None, ai_resets: 0, ai_now: None }
    }
    fn who(&self) -> &'static str {
        self.chara.map_or("(free)", |c| c.name())
    }
}

struct Pending {
    chara: Option<PartyChara>,
    spec: Spec,
    before_last: usize,
    before_set: Vec<usize>,
    asked: f32,
    frames: u64,
}

/// An event item 6 watches, waiting for its outcome.
struct Watch {
    ev: Ev,
    at: f32,
    losses: u32,
}

struct Pool {
    sheet: Sheet,
    slots: Vec<Slot>,
    pending: Option<Pending>,
    wants: Vec<AllyWant>,
    /// A party runs (its wants are real friends).
    real: bool,
    hits: Vec<AllyHit>,
    clock: f32,
    frame_no: u64,
    mode: Mode,
    owner: Owner,
    shot: OneShot,
    test: Option<Test>,
    show: bool,
    /// Per route: failed this session (until the retry).
    route_failed: [bool; 3],
    /// Every route failed (logged loudly, said in the test's report): no friend can be hit until a body is made;
    /// spawning is tried again at `retry_at`.
    all_failed: bool,
    retry_at: f32,
    next_spawn_at: f32,
    buddy: Option<Vec<BuddyRow>>,
    team: Option<u8>,
    losses: u32,
    body_hits: u32,
    /// (player object, block id, frame origin, in a cutscene) as last seen: the events item 6 watches.
    player_obj: usize,
    block: Option<u32>,
    origin: Option<Vec3>,
    cutscene: bool,
    /// The frame origin moved this frame (no drift is measured across it).
    shifted_now: bool,
    watches: Vec<Watch>,
    /// Handles of bodies that died or were given up: never adopted again.
    dead: Vec<u64>,
    /// Characters of the debug set after a timed-out spawn, until `adopt_until`: a late spawn is adopted, not lost.
    adopt: Vec<usize>,
    adopt_until: f32,
    adopt_spec: Option<Spec>,
    /// Stage 2 / item 10: (enemy target id, its HP before, when fired, the body's handle).
    bullet_watch: Option<(TargetId, i32, f32, u64)>,
    /// Item 4: per enemy, the last thing it faced when an attack began (0 = player, k = body k + 1) and the switches.
    faced: Vec<(TargetId, usize, bool)>,
    switches: u32,
    face_body: u32,
    capacity: Option<u32>,
    /// Item 13 (the user's KO rule): the drop part (Ok text / Err failure), the revive part, the latest attack aimed at a
    /// knocked-out friend (s after his KO).
    ko_drop: Option<Result<String, String>>,
    ko_revive: Option<String>,
    ko_last_aim: Option<f32>,
    told_mode: Option<Mode>,
    told_off: bool,
    cost_us: f32,
}

static POOL: Mutex<Option<Pool>> = Mutex::new(None);

const MAX_PTRS: usize = 8;
static BODY_PTRS: [AtomicUsize; MAX_PTRS] = [const { AtomicUsize::new(0) }; MAX_PTRS];

/// targets.rs: this character object is one of our bodies (never one of Sora's targets).
pub fn is_body(ptr: usize) -> bool {
    ptr != 0 && BODY_PTRS.iter().any(|p| p.load(Ordering::Relaxed) == ptr)
}

/// The hits the bodies took since the last call.
pub fn take_hits() -> Vec<AllyHit> {
    POOL.lock().ok().and_then(|mut g| g.as_mut().map(|p| std::mem::take(&mut p.hits))).unwrap_or_default()
}

/// THE HITS INTO KH3 (sora.rs, right after the pool's frame): each one made final with its friend's own maximum (the
/// share rule, U8) and queued on S1's core (`PartyCore::queue_hit`: applied at the start of the next party step,
/// `PartyRuntime::receive_hit`; AI Sora's through `Sora::take_hit`). Returns how many were queued.
pub fn deliver(core: &mut crate::party::glue::PartyCore, sora_hp: i32, sora_max: i32, params: &crate::ermap::Params) -> usize {
    use crate::party::glue::AllyHitIn;
    let hits = take_hits();
    let mut n = 0;
    for h in &hits {
        // (ER party S8) a STAGED friend's body (a not yet unlocked friend fighting a boss): his hits are S8's, at 0 damage
        if crate::unlock::is_staged(h.chara) {
            crate::unlock::stage_body_hit(h, params);
            n += 1;
            continue;
        }
        match h.chara {
            // (DON-MAGIC glacier) a hit (not a status tick) whose attacker's line passes through Donald's ice wall (attacker
            // unknown: every hostile's) is taken by the ice: dropped, no damage (the bodies are topped up as always).
            // UNTESTED IN GAME.
            c if !matches!(h.class, crate::status_rules::Loss::Tick { .. }) && core.rt.view(c).is_some_and(|v| crate::donstatus::wall_shields(v.position, (h.known || h.causer != kh_core::TargetId::NONE).then_some(h.from), &crate::donstatus::hostiles())) => {
                erlog!("party-body: {}: Donald's Glacier wall stands between the attacker and him: the ice takes the hit, no damage", c.name());
            }
            PartyChara::Sora => {
                if sora_hp > 0 && sora_max > 0 {
                    let hit = h.sora_hit(params, sora_max);
                    core.queue_hit(AllyHitIn::Sora(hit));
                    n += 1;
                    erlog!("party-body: AI Sora is hit for {:.1} ({}) from {} of {} Elden Ring HP ({})", hit.amount, h.kind(params).map_or("no reaction".to_string(), |k| format!("{k:?}")), h.lost, h.ref_max, "his body");
                }
            }
            c => match core.rt.view(c) {
                Some(v) if !v.ko => {
                    let record = h.record(params, v.hp_max, v.position);
                    core.queue_hit(AllyHitIn::Friend(c, record));
                    n += 1;
                    erlog!("party-body: {} is hit for {:.1} of his {} HP ({}) from {} of {} Elden Ring HP ({})", c.name(), record.damage, v.hp_max, h.kind(params).map_or("no reaction".to_string(), |k| format!("{k:?}")), h.lost, h.ref_max, "his body");
                }
                Some(_) => erlog!("party-body: {} is KO: a late hit of his body is dropped", c.name()),
                None => erlog!("party-body: {} is not in the party: a hit of his body is dropped", c.name()),
            },
        }
    }
    n
}

/// The body of a friend (live or parked), for other streams (status cures, team-attack invulnerability).
pub fn handle(chara: PartyChara) -> Option<FieldInsHandle> {
    let g = POOL.lock().ok()?;
    let p = g.as_ref()?;
    p.slots.iter().find(|s| s.chara == Some(chara) && matches!(s.state, SlotState::Live | SlotState::Parked)).map(|s| from_bits(s.handle))
}

/// (AGGRO) The body of a friend while it is LIVE (placed at him, hittable: an enemy can attack him), validated this frame.
pub fn live_handle(chara: PartyChara) -> Option<FieldInsHandle> {
    let g = POOL.lock().ok()?;
    let p = g.as_ref()?;
    p.slots.iter().find(|s| s.chara == Some(chara) && s.state == SlotState::Live && s.validated == p.frame_no).map(|s| from_bits(s.handle))
}

/// (ER party I, S5's request) THE GROUND UNDER A FRIEND'S BODY (Elden Ring `hit_material`) for his footsteps
/// (`FxParams::member_surface(own, leader)`): `None` while his body is parked, missing, or its reading never changed.
pub fn ground_material(chara: PartyChara) -> Option<i32> {
    let g = POOL.try_lock().ok()?;
    let p = g.as_ref()?;
    p.slots.iter().find(|s| s.chara == Some(chara) && s.state == SlotState::Live).and_then(|s| s.ground.ground())
}

/// One line on the pool (the `party:` status of S1, the test's report).
pub fn status() -> String {
    let Ok(g) = POOL.lock() else { return "party-body: (busy)".to_string() };
    let Some(p) = g.as_ref() else { return "party-body: not started".to_string() };
    p.summary()
}

/// Stage 2 (targets.rs `apply_hit`): a friend's hit is fired by his own body when `party.bullet_owner` takes him;
/// every other hit (and any failure) is the player's bullet as before.
#[allow(clippy::too_many_arguments)]
pub fn fire_hit(bullets: &mut crate::bullet::Bullets, owner: FieldInsHandle, hit: &kh_core::HitRecord, target: Option<FieldInsHandle>, from: Vec3, dir: Vec3, attack: crate::bullet::Attack, shape: crate::bullet::Shape) -> Result<u32, String> {
    if let Some((chara, body)) = owner_of(hit) {
        match bullets.fire_owned(body, target, from, dir, attack, shape) {
            Ok(row) => {
                static N: AtomicUsize = AtomicUsize::new(0);
                if N.fetch_add(1, Ordering::Relaxed) < 20 {
                    erlog!("party-body: stage 2: {}'s hit '{}' fired by HIS BODY (owner {:#x}, bullet row {row}): the enemy's hate should go to him", chara.name(), hit.attack_name, game::handle_bits(&body));
                }
                return Ok(row);
            }
            Err(why) => erlog!("party-body: stage 2: {}'s body could not fire ({why}): the player's bullet instead", chara.name()),
        }
    }
    bullets.fire(owner, target, from, dir, attack, shape)
}

fn owner_of(hit: &kh_core::HitRecord) -> Option<(PartyChara, FieldInsHandle)> {
    let g = POOL.try_lock().ok()?;
    let p = g.as_ref()?;
    if p.owner == Owner::Off || p.mode != Mode::Bodies {
        return None;
    }
    let m = p.sheet.owner_match;
    // (AGGRO) who made the hit: Donald's magic carries his flag (its attacker point is the spell's, not his capsule);
    // else the AI ally whose capsule the attacker point is in; none = the leader (the player's own body)
    let donald = hit.flags & kh_party::donald::magic_fx::DON_HIT_FLAG != 0 && p.wants.iter().any(|w| w.chara == PartyChara::Donald);
    let hitter = if donald {
        Some(PartyChara::Donald)
    } else if hit.attacker.valid {
        p.wants
            .iter()
            .filter(|w| (hit.attacker.radius - w.capsule.0).abs() < 2.0 && (hit.attacker.position - (w.feet + Vec3::Z * w.capsule.2)).length() <= m)
            .min_by(|a, b| (hit.attacker.position - a.feet).length().total_cmp(&(hit.attacker.position - b.feet).length()))
            .map(|w| w.chara)
    } else {
        None
    };
    let live = |c: PartyChara| p.slots.iter().find(|s| s.chara == Some(c) && s.state == SlotState::Live && s.validated == p.frame_no).map(|s| (c, from_bits(s.handle)));
    // (AGGRO) an enemy held by an ability (Over Here!, Taunt Pass, Donald's aggro abilities, Zettaflare: party/aggro.rs)
    // takes the hate of every party hit to the member the ability names; a hit that may kill stays the player's (the
    // game's own kill credit)
    match crate::party::aggro::credit(hit.target, hitter, hit.damage) {
        crate::party::aggro::Credit::Leader => None,
        crate::party::aggro::Credit::To(c) => live(c),
        crate::party::aggro::Credit::Own => hitter.filter(|c| p.owner.takes(*c)).and_then(live),
    }
}

fn from_bits(bits: u64) -> FieldInsHandle {
    // SAFETY: FieldInsHandle is 8 plain bytes (game::handle_bits is the inverse).
    unsafe { std::mem::transmute_copy::<u64, FieldInsHandle>(&bits) }
}

/// The characters of the game's debug set, and its capacity.
fn debug_set() -> (Vec<usize>, Option<u32>) {
    let mut out = Vec::new();
    // SAFETY: game thread; the set's entry array is read word by word, every pointer checked.
    let Ok(wcm) = (unsafe { WorldChrMan::instance() }) else { return (out, None) };
    let set = &wcm.debug_chr_set;
    let entries = word!(set.entries);
    let capacity = set.capacity;
    if !plausible(entries) || capacity == 0 || capacity > 4096 {
        return (out, Some(capacity));
    }
    for k in 0..capacity as usize {
        // SAFETY: inside the set's entry array (ChrSetEntry is 0x38 bytes, the character pointer first).
        let p = unsafe { std::ptr::read_unaligned((entries + k * std::mem::size_of::<eldenring::cs::ChrSetEntry<ChrIns>>()) as *const usize) };
        if plausible(p) {
            out.push(p);
        }
    }
    (out, Some(capacity))
}

/// The game's debug creator, when it exists.
fn creator() -> Option<&'static mut CSDebugChrCreator> {
    // (AGGRO) at its place for the running version, vtable-checked (game.rs): the SDK's field is 2.7.1.0's place, so on
    // 2.7.2.0 it read 0 and every route failed with "the game has no debug character creator object"
    game::debug_chr_creator()
}

/// The Spirit Ash rows (BuddyParam + their NpcParam), read once; `None` when the tables cannot be read.
fn read_buddy_rows() -> Option<Vec<BuddyRow>> {
    std::panic::catch_unwind(|| {
        // SAFETY: game thread; the singleton lives as long as the game. The SDK's own table checks; a panic is caught.
        let repo = unsafe { SoloParamRepository::instance() }.ok()?;
        let mut rows = Vec::new();
        for (id, b) in repo.rows::<BuddyParam>().take(4096) {
            let npc = b.npc_param_id();
            if npc <= 0 {
                continue;
            }
            let Some(n) = repo.get::<NpcParam>(npc as u32) else { continue };
            let resist = [n.resist_poison(), n.resist_desease(), n.resist_blood(), n.resist_curse(), n.resist_freeze(), n.resist_sleep(), n.resist_madness()].into_iter().max().unwrap_or(0);
            let resists = [n.resist_poison(), n.resist_desease(), n.resist_blood(), n.resist_curse(), n.resist_freeze(), n.resist_sleep(), n.resist_madness()]; // (AGGRO)
            rows.push(BuddyRow { id, npc_param: npc, think: b.npc_think_param_id(), init: b.npc_player_init_param_id(), team: n.team_type(), height: n.hit_height(), radius: n.hit_radius(), hp: n.hp(), resist, resists });
        }
        Some(rows)
    })
    .ok()
    .flatten()
}

/// The damage-over-time effects on a character (SpEffectParam change_hp rate / point), as status.rs reads the player's.
fn dots_of(chr: &ChrIns) -> Vec<Dot> {
    let mut out = Vec::new();
    if !plausible(word!(chr.special_effect)) {
        return out;
    }
    let found = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: game thread; the SDK's table checks.
        let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return };
        for entry in chr.special_effect.entries().take(128) {
            if let Some(row) = repo.get::<SpEffectParam>(entry.param_id as u32) {
                if row.change_hp_rate() > 0.0 || row.change_hp_point() > 0 {
                    out.push(Dot { id: entry.param_id, rate: row.change_hp_rate(), point: row.change_hp_point() });
                }
            }
        }
    }));
    let _ = found;
    out
}

/// The throw node's state of a character (the SDK's `ThrowNodeState` as a number; 4 = held as a throw's target).
fn throw_state(chr: &ChrIns) -> Option<u32> {
    use eldenring::cs::{CSChrThrowModule, CSThrowNode};
    let module = word!(chr.modules.throw);
    if !plausible(module) {
        return None;
    }
    let node = crate::probe::word(module + std::mem::offset_of!(CSChrThrowModule, throw_node)).filter(|n| plausible(*n))?;
    crate::probe::u32_at(node + std::mem::offset_of!(CSThrowNode, throw_state))
}

fn block_of(player: &PlayerIns) -> u32 {
    // SAFETY: a plain 4-byte field of the live player.
    unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(player.chr_ins.block_id) as *const u32) }
}

impl Pool {
    fn new(control: &Control) -> Pool {
        let (sheet, note) = load_sheet();
        erlog!("party-body: the ally-body pool starts ({note}): {:?}", sheet);
        Pool {
            sheet,
            slots: Vec::new(),
            pending: None,
            wants: Vec::new(),
            real: false,
            hits: Vec::new(),
            clock: 0.0,
            frame_no: 0,
            mode: Mode::of(control),
            owner: Owner::of(control),
            shot: OneShot::new("party.body_test", control),
            test: None,
            show: false,
            route_failed: [false; 3],
            all_failed: false,
            retry_at: 0.0,
            next_spawn_at: 0.0,
            buddy: None,
            team: None,
            losses: 0,
            body_hits: 0,
            player_obj: 0,
            block: None,
            origin: None,
            cutscene: false,
            shifted_now: false,
            watches: Vec::new(),
            dead: Vec::new(),
            adopt: Vec::new(),
            adopt_until: 0.0,
            adopt_spec: None,
            bullet_watch: None,
            faced: Vec::new(),
            switches: 0,
            face_body: 0,
            capacity: None,
            ko_drop: None,
            ko_revive: None,
            ko_last_aim: None,
            told_mode: None,
            told_off: false,
            cost_us: 0.0,
        }
    }

    fn summary(&self) -> String {
        let bodies: Vec<String> = self.slots.iter().map(|s| format!("{} {:?} {}{}", s.who(), s.state, s.spec.label(), if s.can == Some(false) { " (CAN NOT BE TARGETED)" } else { "" })).collect();
        format!(
            "party-body: mode {:?}{}, {} body(ies) [{}], {} hit(s) on bodies, {} body loss(es), bullets owned by {:?}",
            self.mode, if self.all_failed { format!(" (!!! EVERY ROUTE FAILED: NO BODY CAN BE MADE, the friends cannot be hit; retried in {:.0} s)", (self.retry_at - self.clock).max(0.0)) } else { String::new() }, self.slots.len(), bodies.join(" | "), self.body_hits, self.losses, self.owner
        )
    }

    fn push_hit(&mut self, h: AllyHit) {
        if self.hits.len() >= 64 {
            self.hits.remove(0);
        }
        self.hits.push(h);
    }

    fn set_item(&mut self, n: usize, st: St) {
        if let Some(t) = self.test.as_mut() {
            if t.set(n, st) {
                erlog!("{}", t.line(n));
            }
        }
    }

    /// The wants of this frame: S1's, or the test's two stand-ins beside the leader while no party runs or the party
    /// has no AI ally yet (none unlocked).
    fn effective_wants(&self, ctx: &Ctx) -> (Vec<AllyWant>, bool) {
        if self.real && (!self.wants.is_empty() || self.test.is_none()) {
            return (self.wants.clone(), true);
        }
        if self.test.is_none() {
            return (Vec::new(), false);
        }
        let facing = coords::dir_er_to_kh(coords::facing_er(ctx.leader_yaw));
        let right = facing.cross(Vec3::Z).normalize_or_zero();
        let off = self.sheet.test_offset * CM_PER_M;
        let stand_in = |chara, side: f32, capsule| AllyWant { chara, feet: ctx.leader_feet + right * side * off, yaw: ctx.leader_yaw, capsule, hittable: true, invulnerable: false, ko: false };
        (vec![stand_in(PartyChara::Donald, 1.0, (30.0, 49.0, 51.0)), stand_in(PartyChara::Goofy, -1.0, (40.0, 89.0, 91.0))], false)
    }

    fn pre(&mut self, ctx: &mut Ctx, started: Instant) {
        self.frame_no += 1;
        self.clock += ctx.dt.max(0.0);
        self.real = ctx.wants.is_some();
        self.wants = ctx.wants.clone().unwrap_or_default();
        let mode = Mode::of(ctx.control);
        self.owner = Owner::of(ctx.control);
        self.show = ctx.control.on("party.body_test.show");
        if self.told_mode != Some(mode) {
            erlog!("party-body: mode {:?} (party.er / party.bodies){}", mode, "");
            self.told_mode = Some(mode);
        }
        self.mode = mode;
        if let Some(cmd) = self.shot.fired(ctx.control) {
            self.command(cmd.trim(), ctx);
        }
        if mode == Mode::Off && self.test.is_none() {
            // (party.er = 0 / party.bodies = 0: nothing is made; bodies made before are parked, never touched otherwise)
            if !self.slots.is_empty() && !self.told_off {
                erlog!("party-body: switched off: the {} body(ies) made before are parked (unhittable, neutral team)", self.slots.len());
                self.told_off = true;
            }
            if self.slots.is_empty() {
                publish(&[]);
                return;
            }
        } else {
            self.told_off = false;
        }
        let (set, capacity) = debug_set();
        if self.capacity.is_none() && capacity.is_some() {
            self.capacity = capacity;
            erlog!("party-body: the game's debug character set has room for {} character(s), {} in it now", capacity.unwrap_or(0), set.len());
        }
        self.watch_events(ctx);
        // ---- 1. every body is checked before anything of it is read
        for i in 0..self.slots.len() {
            if matches!(self.slots[i].state, SlotState::Lost | SlotState::Spawning) {
                continue;
            }
            if self.valid(i, &set).is_none() {
                let s = &mut self.slots[i];
                erlog!("party-body: {}'s body ({}) WAS DROPPED BY THE GAME (no longer in its character sets with its handle {:#x}): it is remade at once; {} is not affected", s.who(), s.spec.label(), s.handle, s.chara.map_or("nobody", |c| c.name()));
                s.state = SlotState::Lost;
                self.losses += 1;
                self.next_spawn_at = self.clock + self.sheet.respawn_delay;
            }
        }
        // ---- 2. a spawn in flight
        self.poll_pending(ctx, &set);
        // ---- 3. who needs a body
        let (wants, real) = self.effective_wants(ctx);
        let cutscene = crate::hud::in_cutscene();
        let want_views: Vec<WantView> = if mode == Mode::Bodies || self.test.is_some() { wants.iter().map(|w| WantView { chara: w.chara, hittable: w.hittable && !cutscene }).collect() } else { Vec::new() };
        if self.all_failed && self.clock >= self.retry_at {
            self.all_failed = false;
            self.route_failed = [false; 3];
            erlog!("party-body: every body route had failed: trying again now (the friends could not be hit meanwhile)");
        }
        let spawn_ok = self.pending.is_none() && !ctx.paused && !cutscene && game::player_age() >= 60 && self.clock >= self.next_spawn_at && !self.all_failed && (mode == Mode::Bodies || self.test.is_some());
        let views: Vec<SlotView> = self.slots.iter().map(|s| SlotView { chara: s.chara, state: s.state }).collect();
        let actions = rules::plan(&views, &want_views, spawn_ok, self.sheet.max);
        let mut drops = Vec::new();
        for a in actions {
            match a {
                Action::Drop(i) => drops.push(i),
                Action::Release(i) => {
                    let s = &mut self.slots[i];
                    erlog!("party-body: {} left the wanted list: his body is parked and free for any friend (never despawned)", s.who());
                    s.chara = None;
                    if s.state == SlotState::Live {
                        s.state = SlotState::Parked;
                    }
                }
                Action::Reuse(i, c) => {
                    erlog!("party-body: the free body {} is given to {}", self.slots[i].spec.label(), c.name());
                    self.slots[i].chara = Some(c);
                }
                Action::Activate(i) => {
                    if self.slots[i].state == SlotState::Parked {
                        self.slots[i].state = SlotState::Live;
                        erlog!("party-body: {}'s body is placed at him again (hittable)", self.slots[i].who());
                    }
                }
                Action::Park(i) => {
                    self.slots[i].state = SlotState::Parked;
                    erlog!("party-body: {}'s body is parked (KO or a cutscene: unhittable, neutral team)", self.slots[i].who());
                }
                Action::Spawn(c) => self.spawn_for(Some(c), ctx, &set),
            }
        }
        drops.sort_unstable();
        for i in drops.into_iter().rev() {
            self.slots.remove(i);
        }
        // ---- 3b. cures asked for the friends (S2's queue: Donald's Cure on Goofy, a status-clearing item, the save point):
        // the game's own cure effects on the friend's BODY (`status::cure_chr`)
        for (c, reason) in crate::status::take_ally_cures() {
            match self.slots.iter_mut().find(|s| s.chara == Some(c) && matches!(s.state, SlotState::Live | SlotState::Parked) && s.validated == self.frame_no) {
                Some(s) => {
                    // SAFETY: validated in the game's sets this frame (`valid`).
                    let chr = unsafe { &mut *(s.ptr as *mut ChrIns) };
                    let n = crate::status::cure_chr(chr, &format!("{} ({})", reason, c.name()));
                    s.dots.clear();
                    erlog!("party-body: {}'s body cured ({reason}): {n} cure effect(s) given", c.name());
                }
                None => erlog!("party-body: a cure for {} ({reason}): he has no body now (nothing of the game's to clear)", c.name()),
            }
        }
        // ---- 4. each body: its HP (the hits), the switches, the position
        for i in 0..self.slots.len() {
            if matches!(self.slots[i].state, SlotState::Live | SlotState::Parked) && self.slots[i].validated == self.frame_no {
                self.body_frame(i, ctx, &wants, real, cutscene);
            }
        }
        if !real && self.test.is_none() {
            self.hits.clear();
        }
        // ---- 5. the test
        self.cost_us = self.cost_us * 0.95 + started.elapsed().as_secs_f32() * 1.0e6 * 0.05;
        let live = self.slots.iter().filter(|s| s.state == SlotState::Live).count();
        if let Some(t) = self.test.as_mut() {
            t.cost(ctx.dt, started.elapsed().as_secs_f32() * 1.0e6, live, wants.len());
        }
        self.test_frame(ctx);
        publish(&self.slots);
    }

    /// Found in the game's sets this frame, with its own handle: the body (else `None`: lost).
    fn valid(&mut self, i: usize, set: &[usize]) -> Option<&'static mut ChrIns> {
        let s = &mut self.slots[i];
        if !plausible(s.ptr) {
            return None;
        }
        let found = if set.contains(&s.ptr) {
            true
        } else if !s.in_debug_set {
            crate::targets::find_chr(&from_bits(s.handle)).is_some_and(|c| c as *const ChrIns as usize == s.ptr)
        } else {
            false
        };
        if !found {
            return None;
        }
        // SAFETY: the pointer is an entry of the game's character set this frame (a live character object).
        let chr = unsafe { &mut *(s.ptr as *mut ChrIns) };
        if game::handle_bits(&chr.field_ins_handle) != s.handle || !game::chr_complete(chr) {
            return None;
        }
        s.validated = self.frame_no;
        Some(chr)
    }

    /// The spec of a body by `route` for `chara` (`None` = a probe of item 2).
    fn spec_for(&mut self, route: BodyRoute, chara: Option<PartyChara>, ctx: &Ctx) -> Result<Spec, String> {
        if self.buddy.is_none() {
            self.buddy = Some(read_buddy_rows().unwrap_or_default());
            let rows = self.buddy.as_deref().unwrap_or(&[]);
            self.team = self.sheet.team.or_else(|| rules::spirit_team(rows));
            erlog!("party-body: {} Spirit Ash row(s) read from BuddyParam; the spirit team is {} ({})", rows.len(), self.team.map_or("NOT KNOWN".to_string(), |t| t.to_string()), if self.sheet.team.is_some() { "the sheet's body.team" } else { "the commonest team of their NpcParams" });
            self.set_item(3, St::Wait(format!("the Spirit Ash NpcParams carry team {}; waiting for a hostile within 30 m of a body for the can-target check", self.team.map_or("?".to_string(), |t| t.to_string()))));
        }
        let rows = self.buddy.clone().unwrap_or_default();
        let team = self.team.ok_or("no spirit team is known (BuddyParam / NpcParam not readable, no body.team row)")?;
        let who = chara.unwrap_or(PartyChara::Goofy);
        match route {
            BodyRoute::Buddy => {
                let row = match self.sheet.buddy[who as usize] {
                    Pick::Row(id) => rows.iter().find(|r| r.id == id).copied().ok_or(format!("BuddyParam {id} (sheet) not found"))?,
                    Pick::Auto => rules::auto_pick(&rows, self.sheet.height[who as usize], team, self.sheet.immune).ok_or("no Spirit Ash row of the spirit team with normal resistances")?,
                };
                Ok(Spec { route, chr_id: rules::chr_of(&row), npc_param: row.npc_param, think: row.think, init: if row.init > 0 { row.init } else { -1 }, team: if row.team != 0 { row.team } else { team }, buddy: Some(row.id) })
            }
            BodyRoute::C0000 => {
                // (AGGRO) the fewest immune ailments, not none (no Spirit Ash has none: allybody.rs auto_pick)
                let row = rows.iter().filter(|r| r.init > 0).min_by(|a, b| (a.height - self.sheet.height[who as usize]).abs().total_cmp(&(b.height - self.sheet.height[who as usize]).abs()).then(rules::immune_count(a, self.sheet.immune).cmp(&rules::immune_count(b, self.sheet.immune))).then(a.id.cmp(&b.id))).copied();
                let npc = self.sheet.c0000_npc.or(row.map(|r| r.npc_param)).ok_or("no c0000-based Spirit Ash row and no body.c0000.npc_param")?;
                let init = self.sheet.c0000_init.or(row.map(|r| r.init)).unwrap_or(-1);
                Ok(Spec { route, chr_id: 0, npc_param: npc, think: row.map_or(-1, |r| r.think), init, team, buddy: row.map(|r| r.id) })
            }
            BodyRoute::Copy => {
                let e = ctx.targets.hostiles_by_distance().into_iter().find(|(d, _)| *d <= 3000.0).map(|(_, e)| e.clone()).ok_or("no hostile within 30 m to copy")?;
                Ok(Spec { route, chr_id: e.npc_id, npc_param: e.npc_param, think: e.think_param, init: -1, team, buddy: None })
            }
        }
    }

    /// Asks the debug creator for a body (the first route of the sheet's order that has not failed).
    fn spawn_for(&mut self, chara: Option<PartyChara>, ctx: &Ctx, set: &[usize]) {
        let order = self.sheet.order.clone();
        let mut why = Vec::new();
        for route in order {
            if self.route_failed[route.index()] {
                why.push(format!("{}: failed before", route.name()));
                continue;
            }
            let spec = match self.spec_for(route, chara, ctx) {
                Ok(s) => s,
                Err(e) => {
                    why.push(format!("{}: {e}", route.name()));
                    continue;
                }
            };
            let feet = chara.and_then(|c| self.effective_wants(ctx).0.into_iter().find(|w| w.chara == c)).map_or(ctx.leader_feet, |w| w.feet);
            match ask(&spec, ctx.frame.pos_kh_to_er(feet) + Vec3::Y * 0.05) {
                Ok(before_last) => {
                    erlog!("party-body: a body for {} is asked of the game's debug character creator: {} at his feet", chara.map_or("(item 2 probe)", |c| c.name()), spec.label());
                    self.set_item(1, St::Wait(format!("asked for {} at {:.1} s; waiting for the creator", spec.label(), self.clock)));
                    self.slots.push(Slot::placeholder(chara, spec.clone(), self.clock));
                    self.pending = Some(Pending { chara, spec, before_last, before_set: set.to_vec(), asked: self.clock, frames: 0 });
                    return;
                }
                Err(e) => {
                    erlog!("party-body: the debug creator refused {} ({e})", spec.label());
                    if e.contains("no debug character creator") {
                        self.set_item(1, St::Fail(e.clone()));
                        self.route_failed = [true; 3];
                    }
                    why.push(format!("{}: {e}", route.name()));
                    if e.contains("busy") {
                        return; // (next frame)
                    }
                    self.route_failed[route.index()] = true;
                }
            }
        }
        if !self.all_failed {
            self.all_failed = true;
            self.retry_at = self.clock + self.sheet.retry;
            erlog!("party-body: !!! NO BODY CAN BE MADE for {} ({}): until one is made the friends CANNOT BE HIT (hits on the leader stay the leader's); tried again in {:.0} s", chara.map_or("(probe)", |c| c.name()), why.join("; "), self.sheet.retry);
            self.set_item(1, St::Fail(format!("no route made a body: {}; retried every {:.0} s, the friends cannot be hit meanwhile", why.join("; "), self.sheet.retry)));
        }
    }

    fn poll_pending(&mut self, ctx: &Ctx, set: &[usize]) {
        let Some(p) = self.pending.as_mut() else {
            // (a late body of a spawn given up: adopted as a free, parked body)
            if self.clock < self.adopt_until {
                let ours: Vec<usize> = self.slots.iter().map(|s| s.ptr).collect();
                if let Some(&ptr) = set.iter().find(|q| !self.adopt.contains(q) && !ours.contains(q)) {
                    // SAFETY: an entry of the debug set this frame.
                    let chr = unsafe { &mut *(ptr as *mut ChrIns) };
                    if game::chr_complete(chr) && !self.dead.contains(&game::handle_bits(&chr.field_ins_handle)) {
                        let spec = self.adopt_spec.clone().unwrap_or(Spec { route: BodyRoute::Copy, chr_id: chr.npc_id, npc_param: chr.npc_param_id, think: -1, init: -1, team: self.team.unwrap_or(0), buddy: None });
                        erlog!("party-body: a LATE body appeared in the debug set (c{:04}, NpcParam {}): adopted as a free, parked body (a spawn given up earlier came after all)", chr.npc_id, chr.npc_param_id);
                        let mut s = Slot::placeholder(None, spec, self.clock);
                        self.adopt_until = 0.0;
                        setup(&mut s, chr, ptr, set.contains(&ptr));
                        s.state = SlotState::Parked;
                        s.validated = self.frame_no;
                        self.slots.push(s);
                    }
                }
            }
            return;
        };
        p.frames += 1;
        let Some(creator) = creator() else {
            self.pending = None;
            return;
        };
        let last = word!(creator.last_created_chr);
        let ours: Vec<usize> = self.slots.iter().map(|s| s.ptr).collect();
        let new = if last != p.before_last && plausible(last) && !ours.contains(&last) { Some(last) } else { set.iter().copied().find(|q| !p.before_set.contains(q) && !ours.contains(q)) };
        let waited = self.clock - p.asked;
        if let Some(ptr) = new {
            // SAFETY: the creator's own last character, or a new entry of the debug set, this frame; completeness is
            // checked before any module is read.
            let chr = unsafe { &mut *(ptr as *mut ChrIns) };
            if game::chr_complete(chr) {
                let p = self.pending.take().unwrap();
                let in_set = set.contains(&ptr);
                let Some(i) = self.slots.iter().position(|s| s.state == SlotState::Spawning && s.ptr == 0) else { return };
                let s = &mut self.slots[i];
                setup(s, chr, ptr, in_set);
                s.state = if p.chara.is_some() { SlotState::Live } else { SlotState::Parked };
                s.validated = self.frame_no;
                let facts = format!(
                    "c{:04} NpcParam {} team {} (written {}) chr type {} hp {}/{} base {} hit r {:.2} h {:.2} m, block {:#x}, {} | the creator's spawn flag {}",
                    chr.npc_id, chr.npc_param_id, chr.team_type, p.spec.team, chr.chr_type as i32, chr.modules.data.hp, chr.modules.data.max_hp, chr.modules.data.base_hp, chr.modules.physics.hit_radius, chr.modules.physics.hit_height,
                    // SAFETY: a plain 4-byte field
                    unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(chr.block_id) as *const u32) },
                    if in_set { "in the debug set" } else { "NOT in the debug set (found through last_created_chr)" }, creator.spawn
                );
                erlog!("party-body: BODY MADE for {} after {} frame(s) ({waited:.2} s): {} -> {facts}", p.chara.map_or("(item 2 probe)", |c| c.name()), p.frames, p.spec.label());
                self.set_item(1, St::Pass(format!("the first body appeared {} frame(s) ({waited:.2} s) after the request; last_created_chr {:#x} -> {:#x}", p.frames, p.before_last, last)));
                if let Some(t) = self.test.as_mut() {
                    t.route(p.spec.label(), Ok(waited));
                    let n = 2;
                    erlog!("{}", t.line(n));
                }
                // (item 9 is checked once the raise has settled; item 7: the capacity)
                if let Some(c) = self.capacity {
                    self.set_item(7, if c as usize >= self.sheet.max { St::Pass(format!("the debug set has room for {c} (the plan needs {}); {} in it now; the clean-up at quit-to-title shows in the next session's first line", self.sheet.max, set.len())) } else { St::Fail(format!("the debug set has room for {c} only (the plan needs {})", self.sheet.max)) });
                }
                let _ = ctx;
                return;
            }
        }
        if waited > self.sheet.spawn_timeout {
            let p = self.pending.take().unwrap();
            let flag = creator.spawn;
            if flag {
                creator.spawn = false;
            }
            let why = if flag { "the creator never took the request (its spawn flag stayed set: its update may not run in this game)".to_string() } else { format!("the creator took the request but no character appeared within {:.0} s (the model's files may not load here)", self.sheet.spawn_timeout) };
            erlog!("party-body: the body for {} by {} FAILED: {why}; the next route is tried", p.chara.map_or("(probe)", |c| c.name()), p.spec.label());
            self.route_failed[p.spec.route.index()] = true;
            if let Some(i) = self.slots.iter().position(|s| s.state == SlotState::Spawning && s.ptr == 0) {
                self.slots.remove(i);
            }
            if let Some(t) = self.test.as_mut() {
                t.route(p.spec.label(), Err(why));
                erlog!("{}", t.line(2));
            }
            self.adopt = set.to_vec();
            self.adopt_until = self.clock + 10.0;
            self.adopt_spec = Some(p.spec);
        }
    }

    /// One body, one frame (pre-physics).
    fn body_frame(&mut self, i: usize, ctx: &mut Ctx, wants: &[AllyWant], real: bool, cutscene: bool) {
        // SAFETY: validated this frame (`valid`).
        let chr = unsafe { &mut *(self.slots[i].ptr as *mut ChrIns) };
        let want = self.slots[i].chara.and_then(|c| wants.iter().find(|w| w.chara == c)).copied();
        let parked = self.slots[i].state == SlotState::Parked || cutscene || want.is_none() || self.mode == Mode::Off;
        let hittable = !parked && want.is_some_and(|w| w.hittable);
        let invulnerable = want.is_some_and(|w| w.invulnerable);
        // (the user's rule, 2026-10-15: "Enemies should ignore [dead friends] until they recover") AT THE KO FRAME the body
        // is parked (the pool's plan did it this frame: unhittable, neutral team, held `body.park_depth` m below the leader,
        // out of every enemy's sight and reach) and its ailments are cured (no tick reaches him while he is down); ON
        // REVIVE it is placed at him again, hittable, on the spirit team.
        match (want.is_some_and(|w| w.ko), self.slots[i].ko) {
            (true, None) => {
                let feet = want.map_or(ctx.leader_feet, |w| w.feet);
                let n = crate::status::cure_chr(chr, "the knocked-out friend's body (nothing may reach him while he is down)");
                let s = &mut self.slots[i];
                (s.ko, s.ko_checked, s.revived_at) = (Some((self.clock, feet)), false, None);
                s.dots.clear();
                erlog!("party-body: {} IS KNOCKED OUT: his body is parked at once (unhittable, team {}, {:.0} m below the leader: an enemy that held it re-picks among the live party); {n} cure effect(s) given to it", s.who(), self.sheet.park_team, self.sheet.park_depth);
            }
            (false, Some((since, _))) if want.is_some() => {
                let s = &mut self.slots[i];
                (s.ko, s.revived_at, s.can, s.check_at) = (None, Some(self.clock), None, self.clock);
                erlog!("party-body: {} IS UP AGAIN after {:.1} s down: his body is placed at him, hittable, team {} (the spirit team)", s.who(), self.clock - since, s.spec.team);
            }
            _ => {}
        }
        let k = self.sheet.hp_raise;
        // ---- HP: the one-shot rule, the loss, the top-up
        let (hp, max_now, base_now, dead) = {
            let data = &chr.modules.data;
            (data.hp, data.max_hp, data.base_hp, data.hp <= 0 || chr.chr_flags1c5.death_flag())
        };
        if dead {
            let s = &mut self.slots[i];
            erlog!("party-body: {}'s body DIED in the game (HP {hp} of {max_now}, base held at {} = {k} x its own {}): it is remade; {} is not affected", s.who(), s.written_base, s.own_base, s.chara.map_or("nobody", |c| c.name()));
            s.state = SlotState::Lost;
            self.dead.push(s.handle);
            self.losses += 1;
            self.next_spawn_at = self.clock + self.sheet.respawn_delay;
            self.set_item(9, St::Fail(format!("a body died (HP {hp}): a blow got through the raised maximum ({} = {k} x its own {}) or the game killed it; it was remade", max_now, self.slots[i].own_max)));
            return;
        }
        {
            let s = &mut self.slots[i];
            let data = chr.modules.data.as_mut();
            if s.settle > 0 {
                s.settle -= 1;
                if s.settle == 0 {
                    (s.own_base, s.own_max, s.written_base) = (base_now, max_now, base_now);
                }
            } else if base_now != s.written_base {
                erlog!("party-body: the game set {}'s body base HP to {base_now} (ours {}): taken as its own from now", s.who(), s.written_base);
                (s.own_base, s.own_max, s.written_base, s.settle) = (base_now, max_now, base_now, 4);
            } else {
                let want_base = rules::raised_base(s.own_base, k);
                if data.base_hp != want_base {
                    data.base_hp = want_base;
                    s.written_base = want_base;
                }
            }
        }
        if !self.slots[i].raise_checked && self.slots[i].settle == 0 && self.clock - self.slots[i].born > 1.5 {
            let s = &mut self.slots[i];
            s.raise_checked = true;
            let ok = max_now as f32 >= s.own_max as f32 * (k * 0.9).max(1.0) || k <= 1.0;
            let text = format!("{}'s body: own base HP {} / maximum {}, base held at {}: the game makes its maximum {max_now}", s.who(), s.own_base, s.own_max, s.written_base);
            erlog!("party-body: {text}{}", if ok { "" } else { ": THE RAISE DID NOT TAKE (a blow of its whole own HP could kill it; it is remade then)" });
            self.set_item(9, if ok { St::Pass(format!("{text} (no single blow can kill it; its death, if it ever comes, is logged and it is remade)")) } else { St::Fail(text) });
        }
        let read = rules::read_hp(self.slots[i].hp_written, hp, max_now);
        let mut lost_now = 0;
        match read {
            HpRead::Clamp(v) => self.slots[i].hp_written = Some(v),
            HpRead::Lost(n) if self.slots[i].settle == 0 => lost_now = n,
            _ => {}
        }
        if lost_now > 0 {
            if let Some(w) = want.filter(|_| hittable) {
                self.body_loss(i, chr, &w, lost_now, ctx, real);
            } else {
                erlog!("party-body: a parked / unwanted body ({}) lost {lost_now} HP: no hit (topped up)", self.slots[i].spec.label());
            }
        }
        {
            let data = chr.modules.data.as_mut();
            if data.hp > 0 && data.hp != data.max_hp {
                data.hp = data.max_hp;
            }
            self.slots[i].hp_written = Some(data.hp);
        }
        // ---- the switches (hidden, passive, no pushing, unhittable while parked / invulnerable)
        let sw = Switches { ctrl_no_map_hit: Some(true), dbg_no_hit: Some(!hittable || invulnerable), ..Switches::passive(!self.show) };
        let resets = passive::apply_chr(chr, &sw);
        chr.chr_flags1c5.set_is_invincible(!hittable || invulnerable);
        {
            let flags = &mut chr.chr_ctrl.as_mut().flags;
            if !flags.disable_character_capsule_collision() {
                flags.set_disable_character_capsule_collision(true);
            }
            if !flags.disable_player_collision() {
                flags.set_disable_player_collision(true);
            }
        }
        let team = if hittable { self.slots[i].spec.team } else { self.sheet.park_team };
        if chr.team_type != team {
            chr.team_type = team;
        }
        if resets > 0 {
            let s = &mut self.slots[i];
            s.resets += resets;
            if s.resets <= 3 || s.resets % 600 == 0 {
                erlog!("party-body: {resets} switch(es) of {}'s body had been reset by the game (written again; {} so far)", s.who(), s.resets);
            }
        }
        // (ER party I, the BOSS stream's finding) THE BODY'S AI SWITCH (ChrCtrl + 0xE8 bit 5, SetCharacterAIState): the AI
        // manager puts a character into its per-team lists only while it is on (0x140507340), and those are what the
        // enemies' targeting reads; a body is ON while it is out (hittable) and OFF while parked / KO (out of every list:
        // the user's KO rule). It stays passive: no movement and no attack of its own (the debug switches above:
        // `disabled_movement`, `disabled_secondary_actions`, root motion 0) and our writes place it every physics phase.
        // `party.body_ai = 0`: the switch is not touched (as before).
        if ctx.control.flag("party.body_ai") != Some(false) {
            let want_on = hittable;
            let before = crate::sdkx::ai_on(chr);
            let s = &mut self.slots[i];
            if before != Some(want_on) {
                let written = crate::sdkx::set_ai_on(chr, want_on);
                if before.is_some() && s.ai_said == Some(want_on) {
                    s.ai_resets += 1;
                    if s.ai_resets <= 3 || s.ai_resets % 600 == 0 {
                        erlog!("party-body: the game set {}'s body's AI switch back to {:?}: written {} again ({} so far)", s.who(), before, if want_on { "ON" } else { "OFF" }, s.ai_resets);
                    }
                } else if s.ai_said != Some(want_on) {
                    erlog!("party-body: {}'s body's AI switch (ChrCtrl + 0xE8 bit 5) {:?} -> {} ({}){}", s.who(), before, if want_on { "ON" } else { "OFF" }, if want_on { "out: listed for the enemies' targeting" } else { "parked / KO: out of the AI manager's lists" }, if written { "" } else { ": NOT WRITTEN (its ChrCtrl is not readable)" });
                }
            }
            s.ai_said = Some(want_on);
            s.ai_now = crate::sdkx::ai_byte_value(chr);
        }
        // (ER party I, S5's request) THE GROUND UNDER HIS BODY for his footsteps (`ground_material`): the game's reading of
        // the body's physics module, trusted once seen to change (rules::GroundRead); parked / KO = none (the leader's)
        {
            let forced = chr.hit_material_override;
            let read = chr.modules.physics.material_info.hit_material;
            let on = ctx.control.flag("party.body_ground") != Some(false) && hittable;
            let s = &mut self.slots[i];
            let was = s.ground.trusted();
            s.ground.feed(on.then_some(if forced >= 0 { forced } else { read }));
            if !was && s.ground.trusted() {
                erlog!("party-body: {}'s body reads its own ground (hit_material now {read}, override {forced}): his footsteps take it (`party.body_ground = 0`: the leader's)", s.who());
            }
        }
        // ---- the position: the friend's feet (parked: the leader's), his facing
        let (feet, yaw) = match want {
            Some(w) if !parked => (w.feet, w.yaw),
            // (parked: below the leader, out of every enemy's sight and reach; `body.park_depth = 0`: at his feet)
            _ => (ctx.leader_feet - Vec3::Z * self.sheet.park_depth.max(0.0) * CM_PER_M, ctx.leader_yaw),
        };
        let er = ctx.frame.pos_kh_to_er(feet);
        let now = game::v3(&chr.modules.physics.position);
        let shifted = self.shifted_now;
        if let (Some(w), false) = (self.slots[i].written, shifted) {
            let d = (now - w).length();
            let s = &mut self.slots[i];
            s.drift_max = s.drift_max.max(d);
            if s.throw_on {
                s.throw_drift = s.throw_drift.max(d);
            }
        }
        write(chr, er, yaw);
        self.slots[i].target = Some((er, yaw));
        self.slots[i].written = Some(er);
        // ---- throws (item 8)
        let throw = throw_state(chr);
        let held = throw == Some(4);
        if held != self.slots[i].throw_on {
            let s = &mut self.slots[i];
            s.throw_on = held;
            if held {
                s.throw_drift = 0.0;
                erlog!("party-body: {}'s body is HELD BY A THROW (throw node state 4): it is written to him every frame regardless; the throw's blows arrive as HP", s.who());
            } else {
                let text = format!("{}'s body was held by a throw; the largest move the game made against our writes meanwhile: {:.2} m", s.who(), s.throw_drift);
                let fights = s.throw_drift > 0.3;
                erlog!("party-body: {text}");
                self.set_item(8, if fights { St::Fail(format!("{text}: the throw fights our position writes")) } else { St::Pass(text) });
            }
        }
        // ---- (item 13) right after the KO switch: may any hostile near still target the body? (the game's own word)
        if let (Some((since, ko_feet)), false) = (self.slots[i].ko, self.slots[i].ko_checked) {
            self.slots[i].ko_checked = true;
            let near: Vec<Enemy> = ctx.targets.hostiles_by_distance().into_iter().map(|(_, e)| e.clone()).filter(|e| (e.pos - ko_feet).length() <= 3000.0).collect();
            let (mut yes, mut asked) = (0u32, 0u32);
            for e in &near {
                if let Some(enemy) = ctx.targets.chr_mut(e.id) {
                    if let Some(a) = can_target(enemy, chr) {
                        asked += 1;
                        yes += a as u32;
                    }
                }
            }
            let who = self.slots[i].who();
            erlog!("party-body: {who} knocked out: can_target(hostile, his body) after the switch = yes for {yes} of {asked} hostile(s) within 30 m ({:.2} s after the KO)", self.clock - since);
            if yes > 0 {
                self.ko_drop = Some(Err(format!("after {who}'s KO the game still let {yes} of {asked} hostile(s) near target his body (the neutral team {} does not stop it)", self.sheet.park_team)));
            }
        }
        // ---- can it be targeted (the game's own word), every `target_check` s while a hostile is near
        if hittable && self.clock >= self.slots[i].check_at {
            self.slots[i].check_at = self.clock + self.sheet.target_check;
            let near = ctx.targets.hostiles_by_distance().into_iter().map(|(_, e)| e.clone()).filter(|e| (e.pos - feet).length() <= 3000.0).min_by(|a, b| (a.pos - feet).length().total_cmp(&(b.pos - feet).length()));
            if let Some(e) = near {
                if let Some(enemy) = ctx.targets.chr_mut(e.id) {
                    let answer = can_target(enemy, chr);
                    let s = &mut self.slots[i];
                    if answer != s.can {
                        erlog!("party-body: can_target(c{:04} target {}, {}'s body team {}) = {}", e.npc_id, e.id.0, s.who(), chr.team_type, answer.map_or("NOT KNOWN (the function was not located)".to_string(), |a| if a { "YES: enemies may target him".to_string() } else { "NO: no enemy may target this body".to_string() }));
                    }
                    s.can = answer;
                    s.target_no = if answer == Some(false) { s.target_no + 1 } else { 0 };
                    // (item 13) re-targetable after a revive
                    if let Some(at) = s.revived_at {
                        let who = s.who();
                        if answer == Some(true) {
                            self.ko_revive = Some(format!("{who} re-targetable {:.1} s after he got up (can_target yes, team back to {})", self.clock - at, self.slots[i].spec.team));
                            self.slots[i].revived_at = None;
                        } else if self.clock - at > 10.0 {
                            self.ko_drop = Some(Err(format!("{who} was up again for 10 s and the game still would not let enemies target his body ({answer:?})")));
                            self.slots[i].revived_at = None;
                        }
                    }
                    let text = format!("the Spirit Ash NpcParam team {}, written {}; can_target(c{:04} target {}, the body) = {:?}", self.team.map_or("?".to_string(), |t| t.to_string()), chr.team_type, e.npc_id, e.id.0, answer);
                    let st = match answer {
                        Some(true) => St::Pass(text),
                        Some(false) if self.slots[i].target_no >= self.sheet.target_fails => St::Fail(text),
                        Some(false) => St::Wait(format!("{text} (asked again)")),
                        None => St::Fail(format!("{text}: the game's can-target function was not located")),
                    };
                    self.set_item(3, st);
                    // (ER party I) item 14: the body's AI switch as read, how often the game cleared it, the game's own
                    // can_target and the attacks aimed at a body so far (item 4's count)
                    let s = &self.slots[i];
                    let bit = s.ai_now.map(|b| b & 0x20 != 0);
                    let text = format!("{}'s body: ChrCtrl + 0xE8 = {} (AI switch {}), put back {} time(s) after the game cleared it; can_target(c{:04} target {}, the body) = {:?}; attacks aimed at a body so far {}", s.who(), s.ai_now.map_or("NOT READABLE".to_string(), |b| format!("{b:#04x}")), bit.map_or("?", |b| if b { "ON" } else { "OFF" }), s.ai_resets, e.npc_id, e.id.0, answer, self.face_body);
                    let st14 = match (bit, answer) {
                        (None, _) => St::Fail(format!("{text}: the switch cannot be read (party.body_ai = 0, or the ChrCtrl is not ours)")),
                        (Some(false), _) => St::Fail(format!("{text}: the switch does not stay on")),
                        (Some(true), Some(true)) => St::Pass(text),
                        (Some(true), _) => St::Wait(format!("{text} (asked again)")),
                    };
                    self.set_item(14, st14);
                }
            }
        }
        // ---- ailments on the body (item 11)
        if self.clock - self.slots[i].dots_at > 0.5 {
            self.slots[i].dots_at = self.clock;
            let dots: Vec<i32> = dots_of(chr).iter().map(|d| d.id).collect();
            if dots.iter().any(|d| !self.slots[i].dots.contains(d)) {
                erlog!("party-body: damage-over-time effect(s) {:?} on {}'s body: its ticks reach him as hits without a reaction", dots, self.slots[i].who());
                self.set_item(11, St::Pass(format!("effect(s) {dots:?} built up on {}'s body", self.slots[i].who())));
            }
            self.slots[i].dots = dots;
        }
    }

    /// A body lost HP: the hit on its friend (Sora's damage-in rule, by the reference maximum).
    fn body_loss(&mut self, i: usize, chr: &ChrIns, w: &AllyWant, lost: i32, ctx: &mut Ctx, real: bool) {
        let control = ctx.control;
        let s = &self.slots[i];
        let ref_max = if self.sheet.ref_hp > 0 { self.sheet.ref_hp } else { s.own_max.max(1) };
        let body_max = chr.modules.data.max_hp;
        let dots = dots_of(chr);
        let fresh: Vec<Dot> = dots.iter().filter(|d| !s.dots.contains(&d.id)).copied().collect();
        let class = if control.flag("status.ticks") == Some(false) {
            Loss::Hit
        } else {
            let active = (!dots.is_empty()).then_some(0usize);
            sr::classify_with(lost, ref_max, body_max, active, &dots, control.f32("status.tick_cap").unwrap_or(0.05), &sr::ProcSign { fresh: &fresh, ailment: None, guess_pct: None })
        };
        let tick = matches!(class, Loss::Tick { .. });
        let by = ctx.targets.id_of(&chr.last_hit_by).and_then(|id| ctx.targets.get(id)).cloned();
        let known = by.is_some();
        let attacker = if tick { None } else { by.or_else(|| ctx.targets.hostiles_by_distance().into_iter().map(|(_, e)| e.clone()).filter(|e| (e.pos - w.feet).length() <= 800.0).min_by(|a, b| (a.pos - w.feet).length().total_cmp(&(b.pos - w.feet).length()))) };
        let (causer, from, yaw, radius) = match &attacker {
            Some(e) => (e.id, e.pos, e.yaw, e.radius),
            None => (TargetId::NONE, w.feet + coords::dir_er_to_kh(coords::facing_er(w.yaw)) * 100.0, w.yaw + 180.0, 40.0),
        };
        let direction = ((w.feet - from) * Vec3::new(1.0, 1.0, 0.0)).normalize_or_zero();
        let hit = AllyHit { chara: w.chara, lost, ref_max, class, causer, from, causer_yaw: yaw, causer_radius: radius, known, direction, burn_flinch: control.flag("status.burn_flinch") != Some(false) };
        if let Some(e) = attacker.as_ref().filter(|_| known) {
            ctx.targets.note_attack(e.id);
        }
        let who = s.who();
        let hidden = !chr.chr_flags1c5.enable_render() && chr.debug_flags.disabled_movement();
        self.slots[i].hits += 1;
        self.body_hits += 1;
        erlog!(
            "party-body: {who}'s body was hit: Elden Ring took {lost} of its {body_max} HP (reference {ref_max}) {} -> {} ({}){}",
            match &attacker {
                Some(e) if known => format!("by target {} c{:04} (last_hit_by)", e.id.0, e.npc_id),
                Some(e) => format!("(last_hit_by {:#x} names no character we know: taken as the nearest hostile, target {} c{:04})", game::handle_bits(&chr.last_hit_by), e.id.0, e.npc_id),
                None => "(no attacker: a status tick or damage over time)".to_string(),
            },
            if real { "a hit on him" } else { "the test's stand-in (no party runs: logged only)" },
            match class { Loss::Tick { .. } => "a status tick, no reaction".to_string(), Loss::Proc { pct, .. } => format!("a proc, {pct:.1} % of his maximum"), Loss::Hit => format!("{:?}", hit.kind(&ctx.targets.params)) },
            if hidden { "" } else { " [the body is not hidden + passive now]" },
        );
        if !tick {
            self.set_item(5, if hidden { St::Pass(format!("{who}'s body lost {lost} HP while hidden (render off) and passive; last_hit_by {}; the largest move of the game against our writes so far {:.2} m", if known { "named the attacker" } else { "named nobody we know" }, self.slots[i].drift_max)) } else { St::Info(format!("{who}'s body was hit while NOT hidden + passive (party.body_test.show = 1?)")) });
            if let Some(e) = attacker.as_ref().filter(|e| known && e.hostile) {
                self.set_item(4, St::Pass(format!("target {} c{:04} attacked {who}'s body (its HP dropped, last_hit_by names it){}", e.id.0, e.npc_id, if self.switches > 0 { format!("; {} switch(es) between body and player seen", self.switches) } else { String::new() })));
            }
        }
        // (the test's stand-ins have no friend to deliver to: logged only)
        if real {
            self.push_hit(hit);
        }
    }

    /// The events item 6 watches, and their outcomes.
    fn watch_events(&mut self, ctx: &Ctx) {
        let obj = ctx.player as *const PlayerIns as usize;
        let block = block_of(ctx.player);
        let cut = crate::hud::in_cutscene();
        let mut evs = Vec::new();
        if self.player_obj != 0 && obj != self.player_obj {
            evs.push(Ev::Load);
        }
        if self.block.is_some_and(|b| b != block) {
            evs.push(Ev::Block);
        }
        if let Some(o) = self.origin {
            if o != ctx.frame.origin && game::origin_shift(ctx.frame.origin - o) {
                evs.push(Ev::Origin);
            }
        }
        if cut && !self.cutscene {
            evs.push(Ev::Cutscene);
        }
        if obj != self.player_obj && self.player_obj != 0 {
            let (set, _) = debug_set();
            erlog!("party-body: a NEW PLAYER OBJECT (a load screen): the debug set holds {} character(s) now; our bodies are checked before use", set.len());
        }
        self.shifted_now = self.origin.is_some_and(|o| o != ctx.frame.origin);
        (self.player_obj, self.block, self.origin, self.cutscene) = (obj, Some(block), Some(ctx.frame.origin), cut);
        let live = self.slots.iter().filter(|s| s.state == SlotState::Live).count();
        for ev in evs {
            if live > 0 || self.slots.iter().any(|s| s.state == SlotState::Lost) {
                self.watches.push(Watch { ev, at: self.clock, losses: self.losses });
            }
        }
        // outcomes: no loss within 2 s = survived; lost and every wanted body live again = remade; 15 s = not remade
        let wanted = self.slots.iter().filter(|s| s.chara.is_some()).count();
        let all_live = self.slots.iter().filter(|s| s.chara.is_some()).all(|s| s.state == SlotState::Live || s.state == SlotState::Parked) && wanted > 0;
        let mut done = Vec::new();
        for (k, w) in self.watches.iter().enumerate() {
            let age = self.clock - w.at;
            let outcome = if self.losses == w.losses && age >= 2.0 {
                Some(Outcome::Survived)
            } else if self.losses > w.losses && all_live {
                Some(Outcome::Remade(age))
            } else if age > 15.0 {
                Some(Outcome::NotRemade)
            } else {
                None
            };
            if let Some(o) = outcome {
                done.push((k, w.ev, o));
            }
        }
        for (k, ev, o) in done.into_iter().rev() {
            self.watches.remove(k);
            erlog!("party-body: {}: the bodies {}", ev.name(), match o { Outcome::Survived => "SURVIVED".to_string(), Outcome::Remade(s) => format!("were lost and REMADE after {s:.2} s (the friends were not affected)"), Outcome::NotRemade => "were lost and NOT remade within 15 s (the friends cannot be hit until they are; retried)".to_string() });
            if let Some(t) = self.test.as_mut() {
                t.event(ev, o);
                erlog!("{}", t.line(6));
            }
        }
    }

    /// `party.body_test = ...`.
    fn command(&mut self, cmd: &str, ctx: &mut Ctx) {
        if self.mode == Mode::Off && Mode::of(ctx.control) == Mode::Off && ctx.control.flag("party.er") == Some(false) {
            erlog!("party-body-test: party.er = 0: the party is off; the test needs it on");
            return;
        }
        match cmd {
            "stop" | "0" | "off" | "" => {
                if self.test.take().is_some() {
                    erlog!("party-body-test: stopped; the stand-in bodies are parked (never despawned)");
                }
            }
            "report" => match self.test.as_ref() {
                Some(t) => {
                    for line in t.report(self.clock) {
                        erlog!("{line}");
                    }
                    erlog!("{}", self.summary());
                }
                None => erlog!("party-body-test: no test runs: set party.body_test = go"),
            },
            "bullet" => self.test_bullet(ctx),
            "kinds" => {
                if self.test.is_none() {
                    self.start_test();
                }
                // (item 2: the routes not tried yet, each once, as free parked bodies)
                let tried: Vec<String> = self.test.as_ref().map(|t| t.routes.iter().map(|(l, _)| l.clone()).collect()).unwrap_or_default();
                let order = self.sheet.order.clone();
                for r in order {
                    if !tried.iter().any(|l| l.starts_with(r.name())) && self.pending.is_none() {
                        let saved = self.route_failed;
                        self.route_failed = [true; 3];
                        self.route_failed[r.index()] = false;
                        let (set, _) = debug_set();
                        self.spawn_for(None, ctx, &set);
                        self.route_failed = saved;
                        self.all_failed = false;
                        erlog!("party-body-test: item 2: the {} route is tried as a probe (one route at a time; set kinds again for the next)", r.name());
                        break;
                    }
                }
            }
            _ => self.start_test(),
        }
    }

    fn start_test(&mut self) {
        let t = Test::new(self.clock, &self.sheet.test_report.clone());
        self.test = Some(t);
        erlog!("party-body-test: ==== STARTED (party.body_test): {} ====", if self.real && !self.wants.is_empty() { "the party's own bodies are watched" } else { "two stand-in bodies (Donald's left of the leader, Goofy's right) are made because no party runs or no AI ally is in it yet; their hits are logged only" });
        erlog!("party-body-test: what to do: stand near enemies and let them fight for a minute; then rest at a grace or fast travel (a load), walk a few hundred metres in the open world, watch a cutscene if one comes; `party.body_test = bullet` near an enemy (item 10); `party.body_test = report` at any time; `party.body_test.show = 1` shows the bodies (for looking only)");
        // the facts known now
        if let Some(c) = self.capacity {
            self.set_item(7, if c as usize >= self.sheet.max { St::Pass(format!("the debug set has room for {c} (the plan needs {})", self.sheet.max)) } else { St::Fail(format!("the debug set has room for {c} only (the plan needs {})", self.sheet.max)) });
        }
        if let Some(s) = self.slots.iter().find(|s| matches!(s.state, SlotState::Live | SlotState::Parked)) {
            let text = format!("bodies were already made this session ({} first)", s.spec.label());
            self.set_item(1, St::Pass(text));
        }
        if creator().is_none() {
            self.set_item(1, St::Fail("the game has no debug character creator object (WorldChrMan.debug_chr_creator is null)".to_string()));
        }
    }

    /// Item 10: one body-owned bullet at the nearest hostile of a live body.
    fn test_bullet(&mut self, ctx: &mut Ctx) {
        if self.test.is_none() {
            self.start_test();
        }
        let body = self.slots.iter().find(|s| s.state == SlotState::Live && s.validated == self.frame_no).and_then(|s| s.target.map(|t| (s.handle, t.0, s.who())));
        let Some((handle, body_er, who)) = body else {
            erlog!("party-body-test: item 10: no live body yet (wait for the bodies, then set party.body_test = bullet again)");
            return;
        };
        let near = ctx.targets.hostiles_by_distance().into_iter().map(|(_, e)| e.clone()).find(|e| (e.er_pos - body_er).length() <= 15.0);
        let Some(e) = near else {
            erlog!("party-body-test: item 10: no hostile within 15 m of {who}'s body");
            return;
        };
        let to = e.er_pos + Vec3::Y * (e.half_height / CM_PER_M);
        let from = to + (body_er - to).normalize_or_zero() * 0.6;
        let dir = (to - from).normalize_or_zero();
        let attack = crate::bullet::Attack { phys: 40, poise: 0.0, stamina: 10, level: 1, ..crate::bullet::Attack::PLAIN };
        let shape = ctx.targets.shape;
        match ctx.targets.bullets.fire_owned(from_bits(handle), Some(e.handle), from, dir, attack, shape) {
            Ok(row) => {
                erlog!("party-body-test: item 10: a bullet owned by {who}'s body (row {row}, attack 40) fired at target {} c{:04} (HP {})", e.id.0, e.npc_id, e.hp);
                self.bullet_watch = Some((e.id, e.hp, self.clock, handle));
                self.set_item(10, St::Wait(format!("fired at target {} c{:04}; watching its HP and where it turns", e.id.0, e.npc_id)));
            }
            Err(why) => {
                erlog!("party-body-test: item 10: the bullet could not be fired ({why})");
                self.set_item(10, St::Fail(format!("the body-owned bullet could not be fired: {why}")));
            }
        }
    }

    /// The test's per-frame evidence (items 4 and 10) and its report.
    fn test_frame(&mut self, ctx: &Ctx) {
        if self.test.is_none() {
            return;
        }
        // item 4: which of player / bodies each enemy faces when an attack begins
        let bodies: Vec<Vec3> = self.slots.iter().filter(|s| s.state == SlotState::Live).filter_map(|s| s.target.map(|t| ctx.frame.pos_er_to_kh(t.0))).collect();
        let player = ctx.leader_feet;
        for e in ctx.targets.list.iter().filter(|e| e.hostile && e.alive && (e.pos - player).length() < 2500.0) {
            let facing = coords::dir_er_to_kh(coords::facing_er(e.yaw));
            let angle = |p: Vec3| {
                let v = ((p - e.pos) * Vec3::new(1.0, 1.0, 0.0)).normalize_or_zero();
                v.dot(facing).clamp(-1.0, 1.0).acos().to_degrees()
            };
            let k = self.faced.iter().position(|f| f.0 == e.id);
            let was_attacking = k.is_some_and(|k| self.faced[k].2);
            if e.in_attack && !was_attacking {
                let mut best = (0usize, angle(player));
                for (b, p) in bodies.iter().enumerate() {
                    let a = angle(*p);
                    if a < best.1 {
                        best = (b + 1, a);
                    }
                }
                if best.0 > 0 && best.1 < 25.0 && angle(player) > 45.0 {
                    self.face_body += 1;
                }
                // (item 13) an attack that begins aimed at a knocked-out friend's place, nearer it than anyone live
                let downs: Vec<(&'static str, f32, Vec3)> = self.slots.iter().filter_map(|s| s.ko.map(|(t, f)| (s.who(), t, f))).collect();
                for (who, since, ko_feet) in downs {
                    let a = angle(ko_feet);
                    if a < 25.0 && a < best.1 && (ko_feet - e.pos).length() < 600.0 {
                        let after = self.clock - since;
                        self.ko_last_aim = Some(self.ko_last_aim.map_or(after, |l| l.max(after)));
                        if after > self.sheet.ko_grace {
                            self.ko_drop = Some(Err(format!("target {} c{:04} began an attack aimed at {who}'s place {after:.1} s after his KO (grace {:.0} s)", e.id.0, e.npc_id, self.sheet.ko_grace)));
                        }
                    }
                }
                if let Some(k) = k {
                    if self.faced[k].1 != best.0 {
                        self.switches += 1;
                    }
                    self.faced[k].1 = best.0;
                } else {
                    self.faced.push((e.id, best.0, true));
                }
            }
            if let Some(k) = self.faced.iter().position(|f| f.0 == e.id) {
                self.faced[k].2 = e.in_attack;
            }
        }
        // (item 13) the drop part passes after 10 s down with no late attack at him
        if self.ko_drop.is_none() {
            if let Some(who) = self.slots.iter().find_map(|s| s.ko.filter(|(t, _)| self.clock - *t >= 10.0).map(|_| s.who())) {
                self.ko_drop = Some(Ok(format!("{who} down for 10 s: no enemy attack aimed at his place later than {:.1} s after the KO (grace {:.0} s); no hostile near could target his body", self.ko_last_aim.unwrap_or(0.0), self.sheet.ko_grace)));
            }
        }
        let item13 = match (&self.ko_drop, &self.ko_revive) {
            (Some(Err(e)), _) => St::Fail(e.clone()),
            (Some(Ok(a)), Some(b)) => St::Pass(format!("{a}; {b}")),
            (Some(Ok(a)), None) => St::Wait(format!("{a}; now revive him (Cure, an item, or he gets up)")),
            (None, Some(b)) => St::Wait(format!("{b}; still to see: a KO of 10 s near enemies")),
            (None, None) => St::Wait("let Donald or Goofy be knocked out near enemies, wait 10 s, then revive him (Cure, an item, or he gets up)".to_string()),
        };
        self.set_item(13, item13);
        if self.face_body >= 2 {
            let text = format!("{} attack(s) began facing a body and not the player; {} switch(es) between body and player", self.face_body, self.switches);
            self.set_item(4, St::Pass(text));
        }
        // item 10: the enemy's HP after the body's bullet
        if let Some((id, before, at, _)) = self.bullet_watch {
            let row = ctx.targets.get(id);
            let now_hp = row.map(|e| e.hp);
            if now_hp.is_some_and(|h| h < before) {
                let turned = row.is_some_and(|e| bodies.iter().any(|b| {
                    let v = ((*b - e.pos) * Vec3::new(1.0, 1.0, 0.0)).normalize_or_zero();
                    v.dot(coords::dir_er_to_kh(coords::facing_er(e.yaw))) > 0.9
                }));
                self.set_item(10, St::Pass(format!("the body-owned bullet took {} HP off target {}{}", before - now_hp.unwrap_or(before), id.0, if turned { "; it now faces the body (hate moved)" } else { "; it does not face the body (yet)" })));
                if turned || self.clock - at > 3.0 {
                    self.bullet_watch = None;
                }
            } else if self.clock - at > 3.0 {
                self.set_item(10, St::Fail(format!("target {} lost no HP within 3 s of the body-owned bullet (the AtkParam_Npc row may not be read, or the owner's team cannot hurt it)", id.0)));
                self.bullet_watch = None;
            }
        }
        let clock = self.clock;
        let due = self.test.as_mut().is_some_and(|t| t.due(clock));
        if due {
            if let Some(t) = self.test.as_ref() {
                for line in t.report(clock) {
                    erlog!("{line}");
                }
            }
            erlog!("{}", self.summary());
        }
    }

    /// post_physics: the same position again (the physics step may have moved it).
    fn post(&mut self, frame: Frame) {
        let (set, _) = debug_set();
        for s in self.slots.iter_mut() {
            if !matches!(s.state, SlotState::Live | SlotState::Parked) || s.validated != self.frame_no {
                continue;
            }
            let found = if set.contains(&s.ptr) { true } else { !s.in_debug_set && crate::targets::find_chr(&from_bits(s.handle)).is_some_and(|c| c as *const ChrIns as usize == s.ptr) };
            if !found {
                continue;
            }
            // SAFETY: found in the game's sets just now, validated (handle, completeness) in this frame's pre-physics.
            let chr = unsafe { &mut *(s.ptr as *mut ChrIns) };
            if game::handle_bits(&chr.field_ins_handle) != s.handle {
                continue;
            }
            if let Some((er, yaw)) = s.target {
                let _ = frame;
                write(chr, er, yaw);
                s.written = Some(er);
            }
        }
    }
}

/// A new body, as it is first seen: its numbers, its switches.
fn setup(s: &mut Slot, chr: &mut ChrIns, ptr: usize, in_set: bool) {
    s.ptr = ptr;
    s.handle = game::handle_bits(&chr.field_ins_handle);
    s.in_debug_set = in_set;
    let data = &chr.modules.data;
    (s.own_base, s.own_max, s.written_base, s.settle) = (data.base_hp, data.max_hp, data.base_hp, 4);
    s.hp_written = None;
    chr.team_type = s.spec.team;
    passive::apply_chr(chr, &Switches { ctrl_no_map_hit: Some(true), dbg_no_hit: Some(true), ..Switches::passive(true) });
    chr.chr_flags1c5.set_is_invincible(true);
}

/// Puts a body at `er` (Elden Ring physics space) facing the KH3 yaw.
fn write(chr: &mut ChrIns, er: Vec3, yaw: f32) {
    if !er.is_finite() {
        return;
    }
    let q = coords::quat_er_from_yaw(yaw);
    let physics = chr.modules.physics.as_mut();
    physics.position = game::hp(er);
    physics.orientation = game::quaternion(q);
    physics.interpolated_orientation = game::quaternion(q);
}

/// Asks the debug creator for `spec` at `at` (Elden Ring space): the creator's last character before the request.
fn ask(spec: &Spec, at: Vec3) -> Result<usize, String> {
    // SAFETY: game thread.
    // (AGGRO) the version-aware creator (game.rs); the SDK's spawn_debug_character writes through 2.7.1.0's place
    let creator = creator().ok_or("the game has no debug character creator object")?;
    if creator.spawn {
        return Err("the creator is busy (its spawn flag is still set)".to_string());
    }
    // (ER party S8) a staged boss's double is being made by the same creator
    if crate::unlock::creator_busy() {
        return Err("the creator is busy (S8 is making a staged boss's double)".to_string());
    }
    if !at.is_finite() {
        return Err("no position".to_string());
    }
    let before = word!(creator.last_created_chr);
    // (the init block's scale and count: a zero scale or count of the debug menu's defaults would make nothing; offsets
    // of the SDK's CSDebugChrCreatorInitData: spawn_scale +0x30, spawn_count +0xec)
    let init = std::ptr::addr_of_mut!(creator.init_data) as usize;
    // SAFETY: inside the live creator's init block (0x100 bytes, the SDK's layout).
    unsafe {
        let scale = std::ptr::read_unaligned((init + 0x30) as *const [f32; 4]);
        if scale[..3].iter().all(|v| *v == 0.0 || !v.is_finite()) {
            std::ptr::write_unaligned((init + 0x30) as *mut [f32; 4], [1.0, 1.0, 1.0, 1.0]);
            erlog!("party-body: the creator's spawn scale was {scale:?}: set to 1");
        }
        let count = std::ptr::read_unaligned((init + 0xec) as *const u32);
        if count == 0 || count > 16 {
            std::ptr::write_unaligned((init + 0xec) as *mut u32, 1);
            erlog!("party-body: the creator's spawn count was {count}: set to 1");
        }
    }
    game::spawn_debug_character(&ChrDebugSpawnRequest { chr_id: spec.chr_id, chara_init_param_id: spec.init, npc_param_id: spec.npc_param, npc_think_param_id: spec.think, event_entity_id: 0, talk_id: 0, pos_x: at.x, pos_y: at.y, pos_z: at.z })?; // (AGGRO)
    Ok(before)
}

/// (ER party S8) THE CREATOR IS FREE for S8's staged-boss double: no body request of ours is pending, none adoptable.
pub fn creator_free() -> bool {
    let Ok(g) = POOL.try_lock() else { return false };
    g.as_ref().is_none_or(|p| p.pending.is_none() && (p.adopt.is_empty() || p.clock > p.adopt_until))
}

/// The bodies' pointers for `is_body`.
fn publish(slots: &[Slot]) {
    for (k, p) in BODY_PTRS.iter().enumerate() {
        let v = slots.get(k).filter(|s| matches!(s.state, SlotState::Live | SlotState::Parked)).map_or(0, |s| s.ptr);
        p.store(v, Ordering::Relaxed);
    }
}

/// The sheet (`<project>\sheets\port\party-er-bodies.json`, beside the loadout), else the built-in rows.
fn load_sheet() -> (Sheet, String) {
    let log = crate::log::dir();
    let path = log.parent().map(|root| crate::sora::data_paths(root).1.with_file_name("party-er-bodies.json"));
    let text = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
    match text.as_deref().map(Sheet::from_text) {
        Some(Ok((s, bad))) => (s, format!("sheet {} ({} row(s) not understood: {bad:?})", path.map(|p| p.display().to_string()).unwrap_or_default(), bad.len())),
        Some(Err(e)) => (Sheet::default(), format!("sheet not read ({e}): built-in rows")),
        None => (Sheet::default(), "no sheet file: built-in rows".to_string()),
    }
}

/// SoraHost pre_physics, after Sora's steps and his damage-in: the pool's frame.
pub fn pre_physics(ctx: &mut Ctx) {
    let started = Instant::now();
    let Ok(mut g) = POOL.lock() else { return };
    let pool = g.get_or_insert_with(|| Pool::new(ctx.control));
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pool.pre(ctx, started)));
    if run.is_err() {
        erlog!("party-body: the pool's frame panicked (see the panic line): its bodies are given up (parked by nobody; the friends are not affected)");
        for s in pool.slots.iter_mut() {
            s.state = SlotState::Lost;
        }
        publish(&[]);
    }
}

/// SoraHost post_physics, after the player's write: the bodies are written again.
pub fn post_physics(frame: Frame) {
    let Ok(mut g) = POOL.lock() else { return };
    if let Some(pool) = g.as_mut() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pool.post(frame)));
    }
}
