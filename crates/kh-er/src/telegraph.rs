//! (GF-SENSOR) WHAT THE ENEMIES ARE DOING, READ EVERY FRAME (build/sora_tmp/party/GOOFY/SENSOR_NOTES.md):
//! - [`frame`]: each hostile near the leader: its animation, the TAE windows of that animation (taebook.rs), the live flags.
//!   Gives [`list`], the telegraph list of R5 (`Party::set_telegraphs`).
//! - [`hit_read`]: who threw the hit that just landed and which row it was (R4): the enemy whose hit window is open beside the
//!   body, else a bullet that vanished at the body, else what the flags say, else nothing (the caller keeps its guess).
//! - THE LEARNED BOOK (R5 step 3): a kind of enemy without a TAE window for an animation (no TAE found): (behaviour variation,
//!   animation) -> time of its first hit, learned at each identified hit, run-time only, never saved.
//!
//! Fails soft: no param repository, no book or no flags only lose the telegraph, and `hit_read` answers `None`.

use std::collections::HashMap;
use std::sync::Mutex;

use eldenring::cs::{AtkParam_Npc, BehaviorParam, Bullet, CSBulletManager, ChrIns, NpcParam, PlayerIns, SoloParamRepository};
use fromsoftware_shared::FromStatic;
use glam::Vec3;
use kh_core::host::{AttackClass, AttackInfo};
use kh_core::TargetId;
use kh_erfile::tae;

use crate::attackread::{classify, RowFacts};
use crate::control::Control;
use crate::erlog;
use crate::game::plausible;
use crate::targets::{Enemy, ErTargets};
use crate::taebook::{self, Window};
use crate::word;

/// One attack seen coming: the shape of `kh_party::goofy::Telegraph`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub attacker: TargetId,
    pub class: AttackClass,
    pub perilous: bool,
    pub eta: f32,
    pub active: bool,
    /// A bullet in the air (else an attack animation's window).
    pub flight: bool,
}

/// What a landed hit was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitRead {
    pub info: AttackInfo,
    pub attacker: TargetId,
    /// The AtkParam_Npc row, -1 when not known.
    pub row: i32,
    pub how: &'static str,
}

/// Hostiles further than this from the leader (HUD-REVIVE-COMBAT: and from every other party place) are not read.
const READ_RANGE: f32 = 3000.0;
/// A hit is applied before our frame sees it: a window closed this long ago still counts (seconds).
const WINDOW_GRACE: f32 = 0.15;
/// `parryable` opens just before the hit window on the rows measured (seconds it stays good for a hit).
const PARRY_GRACE: f32 = 0.3;
/// A bullet gone within this long, this close to the body (m), is the hit's.
const BULLET_GRACE: f32 = 0.2;
const BULLET_NEAR: f32 = 3.0;
/// Learned first-hit times match within this (seconds).
const LEARNED_SLACK: f32 = 0.25;
/// The nearest enemy in an attack animation is believed up to this far (cm).
const ATTACK_REACH: f32 = 800.0;

/// Windows this close (seconds) are one swing's.
const SAME_SWING: f32 = 0.1;
/// Frames between two reads of an enemy's AI target (spread over the enemies by their ids).
const AIM_EVERY: u32 = 12;

/// What is known of one attack window.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Attack {
    class: AttackClass,
    perilous: bool,
    row: i32,
    /// The row's attack, all elements.
    power: f32,
    /// A row with no damage at all is a shove, a sound or an effect's carrier, no attack (MEASURED: Margit's 2130010 on his steps).
    harmless: bool,
}

const UNKNOWN: Attack = Attack { class: AttackClass::Normal, perilous: false, row: -1, power: 0.0, harmless: false };

#[derive(Clone, Copy, Debug)]
struct Foe {
    id: TargetId,
    anim: i32,
    time: f32,
    variation: i32,
    pos: Vec3,
    yaw: f32,
    radius: f32,
    in_attack: bool,
    /// The melee window open now, and the last one that was (its clock).
    open: Option<Attack>,
    /// Which swing is open: (animation, its start in ms).
    swing: (i32, i32),
    last: Option<(Attack, f32)>,
    /// The clock when `parryable` was last on.
    parry_at: f32,
    /// Where its AI has its enemy (`ai_target`), refreshed every [`AIM_EVERY`] frames, and the countdown.
    aim: Option<Vec3>,
    aim_in: u32,
}

#[derive(Clone, Copy, Debug)]
struct Shot {
    owner: TargetId,
    row: i32,
    /// Its place (Elden Ring, m) as last read, and its velocity.
    at: Vec3,
    velocity: Vec3,
}

#[derive(Default)]
struct Sensor {
    clock: f32,
    foes: HashMap<u32, Foe>,
    rows: HashMap<(i32, i32), Attack>,
    bullets: HashMap<i32, Attack>,
    variations: HashMap<i32, i32>,
    /// Manipulator vtables: is it a `ComManipulator`.
    coms: HashMap<usize, bool>,
    shots: HashMap<u64, Shot>,
    vanished: Vec<(Shot, f32)>,
    learned: HashMap<(i32, i32), f32>,
    list: Vec<Seen>,
    /// What already struck a friend's body: (body, attacker or bullet, swing) with the clock.
    struck: Vec<(u64, u64, i32, f32)>,
    log: bool,
    /// `atk.nobook = 1` (test key): no TAE windows, as for a character without any (the learned book's test).
    no_book: bool,
    /// `atk.perf = 1` (test key): the cost of a frame: the largest and the sum since the last report (microseconds), frames since.
    worst_us: u128,
    sum_us: u128,
    frames: u32,
    /// The most telegraphs the party held in one step since the last report.
    party_most: usize,
    /// (GF-POLISH) The enemy whose dropped telegraph was logged last.
    dropped: Option<TargetId>,
}

static SENSOR: Mutex<Option<Sensor>> = Mutex::new(None);

fn repo() -> Option<&'static SoloParamRepository> {
    // SAFETY: game thread; the singleton lives as long as the game.
    unsafe { SoloParamRepository::instance() }.ok()
}

fn facts_of(a: &eldenring::param::ATK_PARAM_ST, bullet: bool) -> RowFacts {
    RowFacts {
        throw_type: a.throw_type_id(),
        throw_flag: a.throw_flag(),
        disable_guard: a.disable_guard(),
        guard_rate: a.guard_atk_rate(),
        attribute: a.atk_attribute(),
        level: a.dmg_level(),
        arrow: a.is_arrow_atk(),
        bullet,
        element: a.atk_mag() > 0 || a.atk_fire() > 0 || a.atk_thun() > 0 || a.atk_dark() > 0,
        physical: a.atk_phys() > 0,
    }
}

fn attack_of(row: i32, bullet: bool) -> Option<Attack> {
    let a = repo()?.get::<AtkParam_Npc>(row as u32)?;
    let facts = facts_of(a, bullet);
    let (class, perilous) = classify(&facts);
    let power = [a.atk_phys(), a.atk_mag(), a.atk_fire(), a.atk_thun(), a.atk_dark()].iter().map(|v| *v as f32).sum();
    Some(Attack { class, perilous, row, power, harmless: !facts.physical && !facts.element && class != AttackClass::Grab })
}

/// The row's facts for the log.
fn row_text(row: i32) -> String {
    match repo().and_then(|r| r.get::<AtkParam_Npc>(row as u32)) {
        Some(a) => format!("attr {} level {} vs_player {} guard {} atk {}/{}/{}/{}/{}", a.atk_attribute(), a.dmg_level(), a.dmg_level_vs_player(), a.guard_atk_rate(), a.atk_phys(), a.atk_mag(), a.atk_fire(), a.atk_thun(), a.atk_dark()),
        None => "none".to_string(),
    }
}

/// A bullet's attack row: its own, else that of the bullet it spawns on hit (a carrier has none of its own).
fn bullet_attack(row: i32) -> Option<Attack> {
    let repo = repo()?;
    let mut id = row;
    for _ in 0..4 {
        let b = repo.get::<Bullet>(id as u32)?;
        let atk = b.atk_id_bullet();
        if atk > 0 {
            return attack_of(atk, true);
        }
        id = b.hit_bullet_id();
        if id <= 0 {
            break;
        }
    }
    Some(Attack { class: AttackClass::Projectile, perilous: false, row: -1, power: 0.0, harmless: true })
}

impl Sensor {
    fn variation(&mut self, npc_param: i32) -> i32 {
        *self.variations.entry(npc_param).or_insert_with(|| repo().and_then(|r| r.get::<NpcParam>(npc_param as u32).map(|n| n.behavior_variation_id())).unwrap_or(0))
    }

    /// The class of a TAE window of a character of this behaviour variation (a judge id the params do not resolve: a normal attack).
    fn window_attack(&mut self, variation: i32, w: &Window) -> Attack {
        let key = (variation, w.judge);
        if let Some(a) = self.rows.get(&key) {
            return *a;
        }
        let resolved = repo().and_then(|repo| {
            let id = 200_000_000 + variation.wrapping_mul(1000) + w.judge;
            let b = repo.get::<BehaviorParam>(id as u32)?;
            match b.ref_type() {
                0 => attack_of(b.ref_id(), false),
                1 => bullet_attack(b.ref_id()),
                _ => None,
            }
        });
        let mut a = resolved.unwrap_or(UNKNOWN);
        if w.kind == tae::BULLET && !matches!(a.class, AttackClass::Magic | AttackClass::Grab | AttackClass::Heavy) {
            a.class = AttackClass::Projectile;
        }
        self.rows.insert(key, a);
        a
    }

    fn read_foe(&mut self, e: &Enemy, chr: &ChrIns, list: &mut Vec<Seen>) {
        let now = crate::chranim::now(chr);
        let speed = crate::chranim::speed(chr).max(0.01);
        let variation = self.variation(e.npc_param);
        let flag = &chr.modules.action_flag;
        let parry = plausible(word!(chr.modules.action_flag)) && flag.action_modifiers_flags.parryable();
        let clock = self.clock;
        let before = self.foes.get(&e.id.0).copied();
        let mut foe = Foe { id: e.id, anim: now.id, time: now.time, variation, pos: e.pos, yaw: e.yaw, radius: e.radius, in_attack: e.in_attack, open: None, swing: (0, 0), last: before.and_then(|b| b.last), parry_at: before.map_or(-100.0, |b| b.parry_at), aim: before.and_then(|b| b.aim), aim_in: before.map_or(e.id.0 % AIM_EVERY, |b| b.aim_in) };
        if foe.aim_in == 0 {
            foe.aim = ai_target(chr, &mut self.coms);
            foe.aim_in = AIM_EVERY;
        }
        foe.aim_in -= 1;
        if parry {
            foe.parry_at = clock;
        }
        let book = if self.no_book { None } else { taebook::book(e.npc_id, chr) };
        let windows = book.as_ref().and_then(|b| b.anims.get(&(now.id as i64)).or_else(|| b.anims.get(&((now.id % 1_000_000) as i64))));
        let mut next: Option<(Window, Attack)> = None;
        if let Some(windows) = windows {
            // (a bullet event beside a swing is the swing's shock wave or trail: the swing is the attack. MEASURED: c4310 has 394
            // bullet events to 126 attack events. But a bullet of a swing that grabs or cannot be blocked is the swing's point: c4311's
            // grab is such a bullet, 4310920 in 3008. Event 304 is no warning: all 573 of them in the game's characters sit on x4xxx
            // throw animations, after the grab took.)
            let swings = windows.iter().any(|w| w.kind == tae::ATTACK);
            for w in windows.iter().filter(|w| w.kind == tae::ATTACK || w.kind == tae::BULLET) {
                if now.time > w.end {
                    continue;
                }
                let a = self.window_attack(variation, w);
                if a.harmless || (w.kind == tae::BULLET && swings && !matches!(a.class, AttackClass::Grab | AttackClass::Heavy)) {
                    continue;
                }
                if now.time >= w.start && (w.kind != tae::BULLET || a.perilous) {
                    if foe.open.is_none() {
                        foe.swing = (now.id, (w.start * 1000.0) as i32);
                    }
                    // (a grab or a thrust beside a plain row of one swing: the danger is what counts)
                    if foe.open.is_none_or(|o| a.perilous && !o.perilous) {
                        foe.open = Some(a);
                        foe.last = Some((a, clock));
                    }
                }
                if next.is_none_or(|n| w.start < n.0.start - SAME_SWING || (w.start <= n.0.start + SAME_SWING && a.perilous && !n.1.perilous)) {
                    next = Some((*w, a));
                }
            }
            if let Some((w, a)) = next {
                let active = now.time >= w.start;
                list.push(Seen { attacker: e.id, class: a.class, perilous: a.perilous, eta: if active { 0.0 } else { (w.start - now.time) / speed }, active, flight: false });
            }
        } else if e.in_attack {
            // (no TAE window) the learned first hit of this animation; the flags say when it is on
            let learned = self.learned.get(&(variation, now.id)).copied();
            match learned {
                Some(at) if now.time < at + 0.1 => {
                    if self.log && before.is_none_or(|b| b.anim != now.id) {
                        erlog!("telegraph: t{} anim {} warned by the LEARNED book: first hit at {at:.2} s, eta {:.2}", e.id.0, now.id, (at - now.time).max(0.0) / speed);
                    }
                    list.push(Seen { attacker: e.id, class: AttackClass::Normal, perilous: false, eta: (at - now.time).max(0.0) / speed, active: now.time >= at, flight: false });
                }
                _ if parry => list.push(Seen { attacker: e.id, class: AttackClass::Normal, perilous: false, eta: 0.0, active: true, flight: false }),
                _ => {}
            }
        }
        if self.log && before.is_none_or(|b| b.anim != now.id) && foe.in_attack {
            erlog!("telegraph: t{} c{:04} anim {} ({:.2} s, var {variation}): {}", e.id.0, e.npc_id, now.id, now.length, match (&windows, next) {
                (Some(_), Some((w, a))) => format!("next window {:.2}-{:.2} kind {} judge {} row {} {:?} perilous {} facts {}", w.start, w.end, w.kind, w.judge, a.row, a.class, a.perilous, row_text(a.row)),
                (Some(_), None) => "no window left".to_string(),
                (None, _) => format!("no TAE window (book {})", book.as_ref().map_or("not read".to_string(), |b| format!("of {} has {} animations", b.from, b.anims.len()))),
            });
        }
        self.foes.insert(e.id.0, foe);
    }

    fn read_bullets(&mut self, player: &PlayerIns, targets: &ErTargets, list: &mut Vec<Seen>) {
        // SAFETY: game thread; the list is the game's own, walked as the SDK walks it.
        let Ok(manager) = (unsafe { CSBulletManager::instance_mut() }) else { return };
        let me = crate::game::v3(&player.chr_ins.modules.physics.position);
        let mut now = HashMap::new();
        for b in manager.bullets().take(256) {
            let Some(owner) = targets.id_of(&b.targeting_owner.owner_chr_handle) else { continue };
            if !targets.get(owner).is_some_and(|e| e.hostile) {
                continue;
            }
            let at = crate::game::v3(&b.physics.position);
            let v = &b.physics.velocity;
            let velocity = Vec3::new(v.0, v.1, v.2);
            let row = [b.fly_state.base.param.param_id, b.wait_state.base.param.param_id, b.exp_state.base.param.param_id].into_iter().find(|r| *r > 0).unwrap_or(-1);
            now.insert(crate::game::handle_bits(&b.field_ins_handle), Shot { owner, row, at, velocity });
        }
        let clock = self.clock;
        for (k, s) in &self.shots {
            if !now.contains_key(k) {
                self.vanished.push((*s, clock));
            }
        }
        self.vanished.retain(|(_, at)| clock - *at < 1.0);
        let shots: Vec<Shot> = now.values().copied().collect();
        self.shots = now;
        for s in shots {
            let to_me = me - s.at;
            let d = to_me.length();
            let closing = s.velocity.dot(to_me.normalize_or_zero());
            if d > 40.0 || closing < 1.0 {
                continue;
            }
            let a = self.bullet_row(s.row);
            let eta = d / closing;
            if eta < 3.0 {
                list.push(Seen { attacker: s.owner, class: a.class, perilous: a.perilous, eta, active: eta < 0.1, flight: true });
            }
        }
    }

    fn bullet_row(&mut self, row: i32) -> Attack {
        *self.bullets.entry(row).or_insert_with(|| bullet_attack(row).unwrap_or(Attack { class: AttackClass::Projectile, ..UNKNOWN }))
    }

    /// Who hit the body at `at` (KH3 cm, feet) just now.
    fn read_hit(&mut self, at: Vec3, er_at: Vec3) -> Option<HitRead> {
        let clock = self.clock;
        let dist = |f: &Foe| (f.pos - at).length();
        let open = self.foes.values().filter_map(|f| f.last.filter(|(_, t)| clock - *t <= WINDOW_GRACE).map(|(a, _)| (dist(f), f.id, a))).min_by(|a, b| a.0.total_cmp(&b.0));
        // (MEASURED: a bullet that lands can stay in the list for a few frames after the HP went, so the live ones at the body count too)
        let gone = self.vanished.iter().filter(|(s, t)| clock - *t <= BULLET_GRACE && (s.at - er_at).length() <= BULLET_NEAR).map(|(s, _)| *s);
        let live = self.shots.values().filter(|s| (s.at - er_at).length() <= BULLET_NEAR).copied();
        let shot = gone.chain(live).min_by(|a, b| (a.at - er_at).length().total_cmp(&(b.at - er_at).length()));
        let answer = |id: TargetId, a: Attack, how: &'static str| Some(HitRead { info: AttackInfo { class: a.class, perilous: a.perilous, attacker: Some(id) }, attacker: id, row: a.row, how });
        let found = match (open, shot) {
            // a bullet gone at the body beats a swing that cannot reach it
            (Some((d, id, a)), None) if d <= ATTACK_REACH => answer(id, a, "open window"),
            (Some((d, id, a)), Some(_)) if d <= ATTACK_REACH / 2.0 => answer(id, a, "open window"),
            (_, Some(s)) => {
                let a = self.bullet_row(s.row);
                answer(s.owner, a, "bullet")
            }
            _ => {
                // no window known: the flags, then the learned time, then an attack animation close by
                let near = |ok: &dyn Fn(&Foe) -> bool, foes: &HashMap<u32, Foe>| foes.values().filter(|f| ok(f) && dist(f) <= ATTACK_REACH).min_by(|a, b| dist(a).total_cmp(&dist(b))).copied();
                let learned = self.learned.clone();
                let by = near(&|f| clock - f.parry_at <= PARRY_GRACE, &self.foes).map(|f| (f, "parry flag")).or_else(|| near(&|f| learned.get(&(f.variation, f.anim)).is_some_and(|t| (f.time - t).abs() <= LEARNED_SLACK), &self.foes).map(|f| (f, "learned"))).or_else(|| near(&|f| f.in_attack, &self.foes).map(|f| (f, "attack animation")));
                let (f, how) = by?;
                self.learned.entry((f.variation, f.anim)).or_insert(f.time);
                answer(f.id, UNKNOWN, how)
            }
        };
        found
    }
}

/// Once a frame, before the hits are read.
pub fn frame(control: &Control, player: &PlayerIns, targets: &ErTargets, leader: Vec3, dt: f32) {
    let mut guard = SENSOR.lock().unwrap_or_else(|e| e.into_inner());
    let started = std::time::Instant::now();
    let s = guard.get_or_insert_with(Sensor::default);
    s.clock += dt;
    s.log = control.on("atk.log");
    s.no_book = control.on("atk.nobook");
    let mut list = Vec::new();
    let mut seen = Vec::new();
    // (HUD-REVIVE-COMBAT) near the leader or any other party place (`ErTargets::party_at`): a far member's enemy is read too
    for e in targets.list.iter().filter(|e| e.hostile && e.alive && e.active && std::iter::once(leader).chain(targets.party_at.iter().copied()).any(|p| (e.pos - p).length() <= READ_RANGE)) {
        let Some(chr) = targets.chr_mut(e.id) else { continue };
        s.read_foe(e, chr, &mut list);
        seen.push(e.id.0);
    }
    s.foes.retain(|id, _| seen.contains(id));
    s.read_bullets(player, targets, &mut list);
    s.list = list;
    let took = started.elapsed().as_micros();
    s.worst_us = s.worst_us.max(took);
    s.sum_us += took;
    s.frames += 1;
    if s.frames >= 600 {
        if control.on("atk.perf") {
            erlog!("telegraph: {} foes read, {} seen coming; of the last {} frames: mean {} us, worst {} us; {} learned; the party held up to {} telegraphs", s.foes.len(), s.list.len(), s.frames, s.sum_us / s.frames as u128, s.worst_us, s.learned.len(), s.party_most);
        }
        s.frames = 0;
        s.worst_us = 0;
        s.sum_us = 0;
        s.party_most = 0;
    }
}

/// `ChrCtrl` + 0x18 = its manipulator (the SDK's `ComManipulator`, RTTI on 2.7.2.0), + 0xC0 = the AI's think block.
const CTRL_MANIPULATOR: usize = 0x18;
const MANIPULATOR_THINK: usize = 0xC0;
/// The think block + this = its `CSAiTargetingSystemOwner` (MEASURED 2.7.2.0).
const THINK_OWNER: usize = 0xC938;

/// (R15) Where this character's AI has its enemy now (`get_target_ene0_position` of its `CSAiTargetingSystemOwner`, the game's
/// own getter). None: no AI block, or the block is not laid out as measured (the owner must name this very character).
/// The manipulator's class is checked once per class (`coms`: a checked read costs a system call, MEASURED 35 ms a read of
/// this function with a fresh `Reader`); behind a `ComManipulator` the rest is the game's own fixed layout.
fn ai_target(chr: &ChrIns, coms: &mut HashMap<usize, bool>) -> Option<Vec3> {
    use eldenring::cs::CSAiTargetingSystemOwner;
    use eldenring::position::HavokPosition;
    let word = |at: usize| {
        // SAFETY: a pointer-sized field of a live object of the layout just checked.
        unsafe { std::ptr::read_unaligned(at as *const usize) }
    };
    let manipulator = word(word!(chr.chr_ctrl) + CTRL_MANIPULATOR);
    if !plausible(manipulator) {
        return None;
    }
    let is_com = *coms.entry(word(manipulator)).or_insert_with(|| crate::sensorlab::rtti(&mut crate::chranim::Reader::default(), manipulator).is_some_and(|n| n.contains("ComManipulator")));
    if !is_com {
        return None;
    }
    let think = word(manipulator + MANIPULATOR_THINK);
    if !plausible(think) {
        return None;
    }
    let owner = think + THINK_OWNER;
    if word(owner + 0x10) != chr as *const ChrIns as usize || word(owner + 8) != think {
        return None;
    }
    let getter = word(word(owner) + 3 * 8);
    if !plausible(getter) {
        return None;
    }
    // SAFETY: the game's own virtual getter of an object that names this character and its think block; game thread.
    unsafe {
        let call: extern "C" fn(*const CSAiTargetingSystemOwner, *mut HavokPosition) -> *mut HavokPosition = std::mem::transmute(getter);
        let mut out: HavokPosition = std::mem::zeroed();
        call(owner as *const CSAiTargetingSystemOwner, &mut out);
        let p = crate::game::v3(&out);
        (p.is_finite() && p != Vec3::ZERO).then_some(p)
    }
}

/// (`atk.log`) What Goofy's rules did this step.
pub fn log_events<T: std::fmt::Debug>(events: &[T]) {
    if events.is_empty() || !SENSOR.try_lock().is_ok_and(|g| g.as_ref().is_some_and(|s| s.log)) {
        return;
    }
    for e in events {
        erlog!("goofy: {e:?}");
    }
}

/// (the party's route) How many telegraphs the party holds this step, for the `atk.perf` report.
pub fn note_party(n: usize) {
    if let Ok(mut g) = SENSOR.try_lock() {
        if let Some(s) = g.as_mut() {
            s.party_most = s.party_most.max(n);
        }
    }
}

/// (GF-POLISH, `atk.log`) This enemy aims at nobody of the party: its telegraph is dropped. One line per enemy in a row.
pub fn note_dropped(attacker: TargetId) {
    if let Ok(mut g) = SENSOR.try_lock() {
        if let Some(s) = g.as_mut().filter(|s| s.log && s.dropped != Some(attacker)) {
            s.dropped = Some(attacker);
            erlog!("telegraph: t{} aims at nobody of the party: no warning", attacker.0);
        }
    }
}

/// (test key `atk.classes`) The attacks of this enemy's book that are not plain: `W_AttackNNNN` fires one on demand.
pub fn list_classes(e: &Enemy, chr: &ChrIns) {
    let Ok(mut g) = SENSOR.lock() else { return };
    let Some(s) = g.as_mut() else { return };
    let Some(book) = taebook::book(e.npc_id, chr) else {
        erlog!("telegraph: classes: no book for c{:04}", e.npc_id);
        return;
    };
    let variation = s.variation(e.npc_param);
    let set = e.anim / 1_000_000;
    let mut lines = Vec::new();
    for (id, windows) in &book.anims {
        if *id / 1_000_000 != set as i64 && *id >= 1_000_000 {
            continue;
        }
        for w in windows.iter().filter(|w| w.kind == tae::ATTACK || w.kind == tae::BULLET) {
            let a = s.window_attack(variation, w);
            if a.class != AttackClass::Normal && !a.harmless && a.class != AttackClass::Sweep {
                lines.push(format!("{id}:{:?}:{:.2}:r{}", a.class, w.start, a.row));
            }
        }
    }
    lines.sort();
    lines.dedup();
    erlog!("telegraph: classes of c{:04} var {variation} set {set}: {}", e.npc_id, lines.join(" "));
}

/// (R15) Every read hostile's enemy now: (attacker, the place its AI aims at, Elden Ring space). Read a few times a second.
pub fn aims() -> Vec<(TargetId, Vec3)> {
    SENSOR.lock().ok().and_then(|g| g.as_ref().map(|s| s.foes.values().filter_map(|f| f.aim.map(|p| (f.id, p))).collect())).unwrap_or_default()
}

/// How a swing or a bullet reaches a friend's body (rows `body.swing.*`; KH3 cm, degrees; `bullet` in m).
#[derive(Clone, Copy, Debug)]
pub struct SwingRules {
    pub reach: f32,
    pub arc: f32,
    pub height: f32,
    pub aim: f32,
    pub bullet: f32,
}

/// (GF-INGAME) A swing of an enemy that aims at this friend, or an enemy bullet, reaches his body now, once each: its row's attack.
/// `body`: the body's handle bits; `er_at`, `at`: his place in Elden Ring and KH3 space.
pub fn body_hit(body: u64, er_at: Vec3, at: Vec3, rules: &SwingRules) -> Option<f32> {
    let mut guard = SENSOR.lock().ok()?;
    guard.as_mut()?.reach_body(body, er_at, at, rules)
}

impl Sensor {
    fn reach_body(&mut self, body: u64, er_at: Vec3, at: Vec3, rules: &SwingRules) -> Option<f32> {
    let s = self;
    let clock = s.clock;
    s.struck.retain(|k| clock - k.3 < 4.0);
    let mut found = None;
    for f in s.foes.values() {
        let (Some(a), Some(aim)) = (f.open, f.aim) else { continue };
        let to = at - f.pos;
        let flat = Vec3::new(to.x, to.y, 0.0);
        let off = (flat.y.atan2(flat.x).to_degrees() - f.yaw + 540.0).rem_euclid(360.0) - 180.0;
        if (aim - er_at).length() * 100.0 > rules.aim || flat.length() - f.radius > rules.reach || to.z.abs() > rules.height || off.abs() > rules.arc {
            continue;
        }
        let key = (body, f.id.0 as u64, f.swing.0.wrapping_mul(1000).wrapping_add(f.swing.1));
        if s.struck.iter().any(|k| (k.0, k.1, k.2) == key) {
            continue;
        }
        found = Some((key, a.power));
        break;
    }
    if found.is_none() {
        let bullets = s.shots.iter().filter(|(_, b)| (b.at - er_at).length() <= rules.bullet).map(|(k, b)| (*k, b.row)).collect::<Vec<_>>();
        for (k, row) in bullets {
            let key = (body, k, 0);
            let a = s.bullet_row(row);
            if !a.harmless && !s.struck.iter().any(|x| (x.0, x.1, x.2) == key) {
                found = Some((key, a.power));
                break;
            }
        }
    }
    let (key, power) = found?;
    s.struck.push((key.0, key.1, key.2, clock));
    if s.log {
        erlog!("telegraph: a swing or a bullet reaches a friend's body: power {power}");
    }
    Some(power)
    }
}

/// The attacks seen coming, this frame.
pub fn list() -> Vec<Seen> {
    SENSOR.lock().ok().and_then(|g| g.as_ref().map(|s| s.list.clone())).unwrap_or_default()
}

/// (R4) Who threw the hit that just landed on the body at `at` (KH3 cm; `er_at`: the body's Elden Ring place), and which row it was. `None`: nothing to say (the
/// caller keeps its nearest-hostile guess, and no class).
pub fn hit_read(at: Vec3, er_at: Vec3) -> Option<HitRead> {
    let mut guard = SENSOR.lock().ok()?;
    let s = guard.as_mut()?;
    let read = s.read_hit(at, er_at);
    if s.log {
        erlog!("telegraph: HIT read: {read:?}");
    }
    read
}

#[cfg(test)]
mod tests {
    use super::*;

    const SWING: Attack = Attack { class: AttackClass::Thrust, perilous: true, row: 77, power: 100.0, harmless: false };

    fn foe(id: u32, x: f32) -> Foe {
        Foe { id: TargetId(id), anim: 3000, time: 0.5, variation: 9, pos: Vec3::new(x, 0.0, 0.0), yaw: 0.0, radius: 40.0, in_attack: false, open: None, swing: (0, 0), last: None, parry_at: -100.0, aim: None, aim_in: 0 }
    }

    fn sensor(foes: Vec<Foe>) -> Sensor {
        Sensor { clock: 10.0, foes: foes.into_iter().map(|f| (f.id.0, f)).collect(), ..Sensor::default() }
    }

    fn shot(owner: u32, at: Vec3) -> Shot {
        Shot { owner: TargetId(owner), row: 5, at, velocity: Vec3::ZERO }
    }

    const RULES: SwingRules = SwingRules { reach: 230.0, arc: 80.0, height: 260.0, aim: 200.0, bullet: 1.2 };

    fn swinging(x: f32) -> Foe {
        Foe { open: Some(SWING), aim: Some(Vec3::ZERO), swing: (3000, 500), ..foe(1, x) }
    }

    #[test]
    fn a_swing_reaches_the_friend_it_aims_at_once() {
        let mut s = sensor(vec![swinging(-100.0)]);
        assert_eq!(s.reach_body(9, Vec3::ZERO, Vec3::ZERO, &RULES), Some(100.0));
        assert_eq!(s.reach_body(9, Vec3::ZERO, Vec3::ZERO, &RULES), None, "one swing, one hit");
        assert_eq!(s.reach_body(8, Vec3::ZERO, Vec3::ZERO, &RULES), Some(100.0), "each body has its own");
    }

    #[test]
    fn a_swing_misses_a_friend_it_does_not_aim_at_or_cannot_reach() {
        let far = RULES.reach + 41.0;
        for (foe, er_at) in [(swinging(-100.0), Vec3::X * 3.0), (swinging(-far), Vec3::ZERO), (swinging(100.0), Vec3::ZERO), (Foe { aim: None, ..swinging(-100.0) }, Vec3::ZERO)] {
            assert_eq!(sensor(vec![foe]).reach_body(9, er_at, Vec3::ZERO, &RULES), None);
        }
    }

    #[test]
    fn a_harmless_bullet_passes_a_friend_and_a_real_one_does_not() {
        let mut s = sensor(Vec::new());
        s.shots.insert(1, shot(7, Vec3::X));
        s.bullets.insert(5, Attack { harmless: true, ..SWING });
        assert_eq!(s.reach_body(9, Vec3::ZERO, Vec3::ZERO, &RULES), None);
        s.bullets.insert(5, SWING);
        assert_eq!(s.reach_body(9, Vec3::ZERO, Vec3::ZERO, &RULES), Some(100.0));
        assert_eq!(s.reach_body(9, Vec3::ZERO, Vec3::ZERO, &RULES), None, "once");
    }

    #[test]
    fn the_nearest_open_window_names_the_attacker_not_the_nearest_enemy() {
        let (mut idle, mut far, mut nearer) = (foe(1, 100.0), foe(2, 500.0), foe(3, 300.0));
        idle.in_attack = false;
        far.last = Some((Attack { row: 8, ..SWING }, 10.0));
        nearer.last = Some((SWING, 9.95));
        let r = sensor(vec![idle, far, nearer]).read_hit(Vec3::ZERO, Vec3::ZERO).unwrap();
        assert_eq!((r.attacker, r.row, r.how), (TargetId(3), 77, "open window"));
        assert_eq!(r.info, AttackInfo { class: AttackClass::Thrust, perilous: true, attacker: Some(TargetId(3)) });
    }

    #[test]
    fn a_window_long_closed_or_out_of_reach_names_nobody() {
        let mut old = foe(1, 100.0);
        old.last = Some((SWING, 10.0 - WINDOW_GRACE - 0.1));
        let mut far = foe(2, ATTACK_REACH + 100.0);
        far.last = Some((SWING, 10.0));
        assert_eq!(sensor(vec![old, far]).read_hit(Vec3::ZERO, Vec3::ZERO), None);
    }

    #[test]
    fn a_bullet_gone_at_the_body_beats_a_swing_that_cannot_reach() {
        let mut swinger = foe(1, 600.0);
        swinger.last = Some((SWING, 10.0));
        let mut s = sensor(vec![swinger]);
        s.vanished.push((shot(7, Vec3::X), 10.0));
        s.bullets.insert(5, Attack { class: AttackClass::Magic, perilous: false, row: 55, power: 100.0, harmless: false });
        let r = s.read_hit(Vec3::ZERO, Vec3::ZERO).unwrap();
        assert_eq!((r.attacker, r.how, r.info.class), (TargetId(7), "bullet", AttackClass::Magic));
    }

    #[test]
    fn a_bullet_still_listed_at_the_body_is_the_hit() {
        let mut s = sensor(Vec::new());
        s.shots.insert(1, shot(7, Vec3::X));
        s.bullets.insert(5, Attack { class: AttackClass::Projectile, perilous: false, row: 55, power: 100.0, harmless: false });
        let r = s.read_hit(Vec3::ZERO, Vec3::ZERO).unwrap();
        assert_eq!((r.attacker, r.row, r.how), (TargetId(7), 55, "bullet"));
    }

    #[test]
    fn a_bullet_gone_far_from_the_body_is_not_the_hit() {
        let mut s = sensor(Vec::new());
        s.vanished.push((shot(7, Vec3::X * (BULLET_NEAR + 1.0)), 10.0));
        s.vanished.push((shot(8, Vec3::X), 10.0 - BULLET_GRACE - 0.1));
        assert_eq!(s.read_hit(Vec3::ZERO, Vec3::ZERO), None);
    }

    #[test]
    fn without_a_window_the_flag_answers_and_the_book_learns() {
        let mut f = foe(1, 100.0);
        f.parry_at = 9.9;
        let mut s = sensor(vec![f, foe(2, 50.0)]);
        let r = s.read_hit(Vec3::ZERO, Vec3::ZERO).unwrap();
        assert_eq!((r.attacker, r.row, r.how), (TargetId(1), -1, "parry flag"));
        assert_eq!(s.learned.get(&(9, 3000)), Some(&0.5), "the first hit's time is kept for the next sighting");
        // the same animation again, no flag: the learned time names it
        let mut again = foe(1, 100.0);
        again.time = 0.6;
        s.foes.insert(1, again);
        s.clock = 20.0;
        assert_eq!(s.read_hit(Vec3::ZERO, Vec3::ZERO).unwrap().how, "learned");
    }
}
