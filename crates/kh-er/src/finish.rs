//! (IMPL_ER22) KH3'S BOSS FINISH IN ELDEN RING (docs/sora-bossfinish-spec.md; key `finish.kh`, sheet rows
//! `finish.*` of sora-er-host). UNTESTED IN THE GAME: written by reading, the engine-free parts unit-tested.
//!
//! WHAT IT IS: when Sora's blow kills a boss, KH3's script runs (`kh_core::combat::finish::FinishSequence`): the
//! world at 0.05 for ~4 s, three camera cuts (the boss, Sora, a wide shot of the boss), the 2D sound se00000_047,
//! every other sound of Sora's silenced (KH3's BOSS_DEAD sound mode), the white-out, then normal speed, a second
//! of white, and the fade back.
//!
//! WHO IS A BOSS: a character one of Elden Ring's three boss bars (`CSFeManImp::boss_health_displays`) pointed at
//! within the last `finish.boss_memory` seconds (the entry may be cleared with the death: it is remembered), when
//! no other such character is still alive (KH3 runs the script on the WIPE-OUT of the boss's group: a duo's first
//! death is not a finish). `finish.last_enemy = 1`: the last engaged hostile of an ordinary fight gets the port's
//! short slow-down (NOT a KH3 effect). `finish.test = 1` (testing aid): every kill is a boss finish.
//!
//! (ER party S7) ANY PARTY MEMBER'S KILLING BLOW: Donald's, Goofy's and the team attacks' hits are the player's bullets
//! through the same `ErTargets::apply_hit` (the same hit memory), so a boss they kill gets the finish exactly as by
//! Sora's blow (KH3 runs it at the boss's death, whoever struck); the "player" cut frames the LEADER (cam.rs).
//!
//! WHEN: ON THE CONFIRMED DEATH, never on the core's prediction. Sora's damage is a bullet the game resolves
//! frames later (targets.rs): the trigger is the game's own "it is dead" (`hp <= 0` or its death flag,
//! `ErTargets::deaths`) arriving within `finish.hit_window` seconds of Sora's last hit on it, still true
//! `finish.confirm_frames` frames later. A phase change is not a death (the HP does not reach 0, or the body is
//! alive again the next frames: nothing starts); a body that comes back to life while the sequence runs ends it
//! at once. A boss that "dies" into a cutscene (Radagon, a first phase that ends in a scene) may start the
//! sequence for the frames before the scene: the scene's start ends it at once (see `Scene`).
//!
//! THE WORLD'S TIME: `CSFlipper::game_speed` (the game's own speed multiplier; pause.rs is NOT used) is written as
//! `its value before x rate` every frame of the sequence and put back EXACTLY: at the script's end, on every
//! abort (Sora's death, a cutscene, a menu or popup, the game owning the body, the target gone, the watchdog of
//! `port.watchdog` seconds), in `ErFinish::stop` (the payload's stop, a load), and by a thread of its own when no
//! frame of ours arrives for 0.7 s. While the menus' old pause route holds the speed (`menu.pause = speed`) nothing
//! is written. SORA'S OWN TIME: his fixed steps keep coming 60 times a real second (the accumulator is fed from
//! the wall clock while the sequence runs, because the task delta may itself be scaled by the game speed) and each
//! is `1/60 x rate` long: he, his effects, his prizes and the enemy puppets slow exactly as the script says.
//! His audio engine keeps real time: the finish sound is not slowed (KH3: a 2D sound at pitch 1).
//!
//! WHAT ELDEN RING DOES MEANWHILE: its own death animation, dissolve, "ENEMY FELLED" banner and the rune award
//! are its own and run on the slowed clock (they simply come ~4 s later). The KH3 prize burst of the finished
//! target and its EXP are HELD until the white-out is full (sora.rs), then every orb flies to him (KH3's
//! `AllPrizeCollection`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use glam::Vec3;
use kh_camera::finish::FinishSubject;
use kh_core::combat::finish::{FinishCue, FinishFrame, FinishPhase, FinishSequence, FinishShot, FinishTuning, P};
use kh_core::host::{FinishKind, TargetId};

use crate::control::Control;
use crate::erlog;

/// The sheet's rows `finish.*` (sora-er-host); every one also a key of kh-er.ini.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinishRules {
    /// `finish.kh`: the whole feature.
    pub on: bool,
    /// `finish.last_enemy`: the port's short slow-down on the last engaged hostile of an ordinary fight.
    pub last_enemy: bool,
    /// `finish.rate` / `finish.interp`: the world's rate and the real seconds of the ramp (KH3: 0.05 / 0.1).
    pub rate: f32,
    pub interp: f32,
    /// `finish.er_rate_min`: Elden Ring's game speed is never written below `its own x this`.
    pub er_rate_min: f32,
    /// `finish.speed`: 0 = Elden Ring's game speed is not touched (only Sora and the puppets slow).
    pub speed: bool,
    /// `finish.hit_window`: seconds between Sora's last hit on the body and the game's death of it.
    pub hit_window: f32,
    /// `finish.confirm_frames`: frames the body must stay dead before the sequence starts.
    pub confirm_frames: u32,
    /// `finish.boss_memory`: seconds a boss bar's character is remembered as a boss.
    pub boss_memory: f32,
    /// `finish.cuts`: how many of the three camera cuts are shown (0 = our normal camera stays).
    pub cuts: u8,
    /// `finish.distance`: scales the cuts' distances on top of the body's size.
    pub distance: f32,
    /// `finish.ref_half_height` / `finish.distance_max`: a body taller than this (cm, half height) pushes the
    /// cameras back in proportion, at most by this factor.
    pub ref_half_height: f32,
    pub distance_max: f32,
    /// `finish.player_height`: cm above Sora's feet of his camera bone t_camera (measured in the test app).
    pub player_height: f32,
    /// `finish.collision`: the cuts are kept out of the level.
    pub collision: bool,
    /// `finish.white`: the white-out, its hold (`finish.white_hold`) and fade in (`finish.fade_in`).
    pub white: bool,
    pub white_hold: f32,
    pub fade_in: f32,
    /// `finish.shake`: the script's camera shake's scale (0 = none).
    pub shake: f32,
    /// `finish.se`: ETresStaticSE of the 2D sound (22 = se00000_047, 23 = se00000_048, -1 = none).
    pub se: i32,
    /// `finish.sound_mode`: KH3's BOSS_DEAD sound mode (Sora's other sounds are silenced for the sequence).
    pub sound_mode: bool,
    /// `finish.lock_input`: no movement, no attack, no camera stick during the sequence.
    pub lock_input: bool,
    /// `finish.cooldown`: seconds after a sequence in which no other starts.
    pub cooldown: f32,
}

impl FinishRules {
    pub const DEFAULT: FinishRules = FinishRules {
        on: true,
        last_enemy: false,
        rate: 0.05,
        interp: 0.1,
        er_rate_min: 0.05,
        speed: true,
        hit_window: 1.5,
        confirm_frames: 2,
        boss_memory: 10.0,
        cuts: 3,
        distance: 1.0,
        ref_half_height: 90.0,
        distance_max: 4.0,
        player_height: 102.0,
        collision: true,
        white: true,
        white_hold: 1.0,
        fade_in: 0.7,
        shake: 1.0,
        se: 22,
        sound_mode: true,
        lock_input: true,
        cooldown: 5.0,
    };

    /// One row of the sheet (or one key of kh-er.ini). False: not a row of ours, or out of range.
    pub fn apply_row(&mut self, id: &str, v: f32) -> bool {
        match id {
            "finish.kh" => self.on = v != 0.0,
            "finish.last_enemy" => self.last_enemy = v != 0.0,
            "finish.rate" if (0.01..=1.0).contains(&v) => self.rate = v,
            "finish.interp" if (0.0..=2.0).contains(&v) => self.interp = v,
            "finish.er_rate_min" if (0.01..=1.0).contains(&v) => self.er_rate_min = v,
            "finish.speed" => self.speed = v != 0.0,
            "finish.hit_window" if (0.1..=10.0).contains(&v) => self.hit_window = v,
            "finish.confirm_frames" if (0.0..=60.0).contains(&v) => self.confirm_frames = v as u32,
            "finish.boss_memory" if (0.0..=120.0).contains(&v) => self.boss_memory = v,
            "finish.cuts" if (0.0..=3.0).contains(&v) => self.cuts = v as u8,
            "finish.distance" if (0.1..=10.0).contains(&v) => self.distance = v,
            "finish.ref_half_height" if (10.0..=1000.0).contains(&v) => self.ref_half_height = v,
            "finish.distance_max" if (1.0..=20.0).contains(&v) => self.distance_max = v,
            "finish.player_height" if (0.0..=300.0).contains(&v) => self.player_height = v,
            "finish.collision" => self.collision = v != 0.0,
            "finish.white" => self.white = v != 0.0,
            "finish.white_hold" if (0.0..=5.0).contains(&v) => self.white_hold = v,
            "finish.fade_in" if (0.0..=5.0).contains(&v) => self.fade_in = v,
            "finish.shake" if (0.0..=4.0).contains(&v) => self.shake = v,
            "finish.se" if (-1.0..=43.0).contains(&v) => self.se = v as i32,
            "finish.sound_mode" => self.sound_mode = v != 0.0,
            "finish.lock_input" => self.lock_input = v != 0.0,
            "finish.cooldown" if (0.0..=120.0).contains(&v) => self.cooldown = v,
            _ => return false,
        }
        true
    }

    /// The row ids, for the keys of kh-er.ini.
    pub const KEYS: [&'static str; 23] = [
        "finish.kh", "finish.last_enemy", "finish.rate", "finish.interp", "finish.er_rate_min", "finish.speed", "finish.hit_window", "finish.confirm_frames", "finish.boss_memory", "finish.cuts", "finish.distance",
        "finish.ref_half_height", "finish.distance_max", "finish.player_height", "finish.collision", "finish.white", "finish.white_hold", "finish.fade_in", "finish.shake", "finish.se", "finish.sound_mode", "finish.lock_input", "finish.cooldown",
    ];

    /// These rules with the keys of kh-er.ini over them (a key that goes away gives the sheet's value back).
    pub fn with_keys(self, key: &dyn Fn(&str) -> Option<f32>) -> FinishRules {
        let mut rules = self;
        for id in Self::KEYS {
            if let Some(v) = key(id) {
                rules.apply_row(id, v);
            }
        }
        rules
    }

    /// The script for a body of `half_height` cm: KH3's numbers, the cameras pushed back for a large body.
    pub fn tuning(&self, half_height: f32) -> FinishTuning {
        let size = (half_height / self.ref_half_height.max(1.0)).clamp(1.0, self.distance_max.max(1.0));
        FinishTuning {
            rate: self.rate,
            interp: self.interp,
            cut_count: self.cuts.min(3),
            distance_scale: size * self.distance,
            white: self.white,
            hold: self.white_hold,
            fade_in: self.fade_in,
            shake_scale: self.shake,
            static_se: self.se,
            sound_mode: self.sound_mode,
            lock_input: self.lock_input,
            ..FinishTuning::default()
        }
    }
}

impl Default for FinishRules {
    fn default() -> FinishRules {
        FinishRules::DEFAULT
    }
}

/// A death the game confirmed (targets.rs), with what the rule needs to know about it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Death {
    /// Seconds since Sora's last hit on the body.
    pub since_hit: f32,
    /// A boss bar pointed at it within the memory.
    pub boss: bool,
    /// Another boss-bar character is still alive.
    pub other_boss_alive: bool,
    /// Engaged hostiles left after it.
    pub engaged_left: u32,
}

/// (ZETTA 2026-10-23) Zettaflare's death overrules every KH3 finish. Pure.
pub fn zetta_overrules(zetta_death: bool, kind: FinishKind) -> bool {
    zetta_death && kind != FinishKind::None
}

/// THE RULE: what the death is. Pure.
pub fn kind_of(rules: &FinishRules, test_all: bool, death: &Death) -> FinishKind {
    if !rules.on || death.since_hit > rules.hit_window {
        return FinishKind::None;
    }
    if test_all || (death.boss && !death.other_boss_alive) {
        FinishKind::Boss
    } else if rules.last_enemy && !death.boss && death.engaged_left == 0 {
        FinishKind::Battle
    } else {
        FinishKind::None
    }
}

/// What ends a running sequence at once (and keeps one from starting).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scene {
    /// Sora is dead or dying.
    pub sora_dead: bool,
    /// A cutscene of the game plays (or it moves the body itself).
    pub cutscene: bool,
    /// A menu or popup has the screen (the game's or ours), or the world is paused.
    pub menu: bool,
}

impl Scene {
    fn reason(&self) -> Option<&'static str> {
        if self.sora_dead {
            Some("Sora is dead")
        } else if self.cutscene {
            Some("a cutscene of the game began (or it moves the body)")
        } else if self.menu {
            Some("a menu or popup has the screen")
        } else {
            None
        }
    }
}

struct Pending {
    target: TargetId,
    kind: FinishKind,
    frames: u32,
    half_height: f32,
    /// (IMPL_ER31) The body had one of the game's boss bars: (chr model id, NpcParam id, max HP).
    bar: Option<(i32, i32, i32)>,
}

// ---- Elden Ring's game speed: held while a sequence runs, put back exactly (see the module text).
/// The address of `CSFlipper::game_speed` while we hold it (0 = not held), the value to put back, the time of our
/// last write (ms).
static SPEED_FIELD: AtomicUsize = AtomicUsize::new(0);
static SPEED_BASE: AtomicU32 = AtomicU32::new(0);
static SPEED_BEAT: AtomicU64 = AtomicU64::new(0);
/// No frame of ours for this long while the speed is held: the thread puts it back.
const SPEED_LAPSE_MS: u64 = 700;

fn now_ms() -> u64 {
    // SAFETY: plain getter.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

/// Puts the game's speed back if we hold it. Any thread.
pub fn speed_restore(why: &str) {
    let field = SPEED_FIELD.swap(0, Ordering::SeqCst);
    if field != 0 {
        let base = f32::from_bits(SPEED_BASE.load(Ordering::SeqCst));
        // SAFETY: the field of a singleton that lives as long as the game; a plain f32 write.
        unsafe { (field as *mut f32).write_volatile(base) };
        erlog!("finish: Elden Ring's game speed is back at {base} ({why})");
    }
}

fn speed_watchdog() {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if SPEED_FIELD.load(Ordering::SeqCst) == 0 {
            return;
        }
        if crate::hud::stopping() || now_ms().saturating_sub(SPEED_BEAT.load(Ordering::SeqCst)) > SPEED_LAPSE_MS {
            speed_restore("THE WATCHDOG: no frame of the sequence for 0.7 s");
            return;
        }
    }
}

/// Writes `the game's own speed x rate` (game thread). Nothing while the menus' pause holds the speed.
fn speed_write(rate: f32) {
    use eldenring::cs::CSFlipper;
    use fromsoftware_shared::FromStatic;
    if crate::hud::speed_held() {
        return;
    }
    // SAFETY: game thread; the singleton lives as long as the game.
    let Ok(flipper) = (unsafe { CSFlipper::instance_mut() }) else { return };
    if SPEED_FIELD.load(Ordering::SeqCst) == 0 {
        let base = flipper.game_speed;
        // (a speed that is not a plain running game's is somebody else's hold: left alone)
        if !(0.5..=2.0).contains(&base) {
            return;
        }
        SPEED_BASE.store(base.to_bits(), Ordering::SeqCst);
        SPEED_BEAT.store(now_ms(), Ordering::SeqCst);
        SPEED_FIELD.store(std::ptr::addr_of_mut!(flipper.game_speed) as usize, Ordering::SeqCst);
        erlog!("finish: Elden Ring's game speed {base} is held by the sequence (put back at its end, on any abort, and by a watchdog thread)");
        match std::thread::Builder::new().name("kh-finish-speed".into()).spawn(speed_watchdog) {
            Ok(handle) => crate::hud::adopt_thread(handle),
            Err(e) => {
                erlog!("finish: the watchdog thread could not be started ({e}): the game speed is NOT touched");
                SPEED_FIELD.store(0, Ordering::SeqCst);
                return;
            }
        }
    }
    let base = f32::from_bits(SPEED_BASE.load(Ordering::SeqCst));
    flipper.game_speed = base * rate;
    SPEED_BEAT.store(now_ms(), Ordering::SeqCst);
}

/// The host's side of the sequence.
pub struct ErFinish {
    pub rules: FinishRules,
    test_all: bool,
    sequence: FinishSequence,
    frame: FinishFrame,
    target: TargetId,
    /// Where the target's camera point was last seen (the body may leave the list when it dies).
    enemy: FinishSubject,
    pending: Option<Pending>,
    /// Target id -> the frame a boss bar last pointed at it.
    bars: HashMap<u32, u64>,
    frame_no: u64,
    wall: Instant,
    wall_dt: f32,
    cooldown: f32,
    plate: bool,
    /// For the camera (taken by sora.rs each step): the script's shake request, "stop the shakes".
    shake: Option<f32>,
    stop_shakes: bool,
    collect: bool,
    /// Kills of the finished target held back until the white is full: (id, max HP, where).
    pub held_kills: Vec<(TargetId, i32, Vec3)>,
    last_phase: FinishPhase,
    told_off: bool,
    /// The kill `expects` held last, and in which frame (it is held for one frame only).
    expect: Option<(TargetId, u64)>,
    /// (IMPL_ER31, boss rewards) THE CONFIRMED DEATHS OF BOSS-BAR CHARACTERS (the sequence's own confirmation: the last
    /// boss-bar body, dead by the game's word and still dead `finish.confirm_frames` later): (chr model id, NpcParam
    /// id, max HP). Taken by sora.rs each frame.
    pub boss_deaths: Vec<(i32, i32, i32)>,
}

impl ErFinish {
    pub fn new(rules: FinishRules) -> ErFinish {
        ErFinish {
            rules,
            test_all: false,
            sequence: FinishSequence::default(),
            frame: FinishFrame::idle(),
            target: TargetId::NONE,
            enemy: FinishSubject { point: Vec3::ZERO, yaw: 0.0 },
            pending: None,
            bars: HashMap::new(),
            frame_no: 0,
            wall: Instant::now(),
            wall_dt: 0.0,
            cooldown: 0.0,
            plate: false,
            shake: None,
            stop_shakes: false,
            collect: false,
            held_kills: Vec::new(),
            last_phase: FinishPhase::Idle,
            told_off: false,
            expect: None,
            boss_deaths: Vec::new(),
        }
    }

    pub fn active(&self) -> bool {
        self.sequence.active()
    }

    /// (IMPL_ER31) A sequence runs or waits for its confirmation.
    pub fn busy(&self) -> bool {
        self.sequence.active() || self.pending.is_some()
    }

    /// The world's rate of the latest step (1.0 outside a sequence).
    pub fn rate(&self) -> f32 {
        self.frame.rate
    }

    pub fn locked(&self) -> bool {
        self.frame.input_locked
    }

    pub fn shot(&self) -> Option<FinishShot> {
        self.frame.shot
    }

    pub fn enemy(&self) -> FinishSubject {
        self.enemy
    }

    /// The time the fixed-step accumulator is fed with: the task's delta, or the wall clock's while a sequence
    /// runs (the task delta may be scaled by the game speed we wrote).
    pub fn accumulator_dt(&self, task_dt: f32) -> f32 {
        if self.active() {
            self.wall_dt
        } else {
            task_dt
        }
    }

    /// The kill of `id` waits for the white (its prize burst, its EXP).
    pub fn holds(&self, id: TargetId) -> bool {
        let running = self.sequence.kind() == FinishKind::Boss && self.target == id && matches!(self.sequence.phase(), FinishPhase::Slow | FinishPhase::White);
        running || self.pending.as_ref().is_some_and(|p| p.target == id && p.kind == FinishKind::Boss)
    }

    /// A kill of this frame that the next frame's rule will most likely call a boss finish (the body has a boss
    /// bar, nothing runs): its burst waits one frame for that word instead of coming before the sequence.
    pub fn expects(&self, id: TargetId) -> bool {
        self.rules.on && !self.active() && self.pending.is_none() && self.cooldown <= 0.0 && (self.test_all || self.bars.contains_key(&id.0)) && self.expect != Some((id, self.frame_no.saturating_sub(1)))
    }

    /// The held kills whose wait is over.
    pub fn release_kills(&mut self) -> Vec<(TargetId, i32, Vec3)> {
        let frame_no = self.frame_no;
        let expect = self.expect.filter(|(_, at)| *at == frame_no);
        let (keep, go): (Vec<_>, Vec<_>) = std::mem::take(&mut self.held_kills).into_iter().partition(|k| self.holds(k.0) || expect.is_some_and(|(id, _)| id == k.0));
        self.held_kills = keep;
        go
    }

    /// `expects` held this kill now.
    pub fn note_expected(&mut self, id: TargetId) {
        self.expect = Some((id, self.frame_no));
    }

    pub fn take_shake(&mut self) -> (bool, Option<f32>) {
        (std::mem::take(&mut self.stop_shakes), self.shake.take())
    }

    pub fn take_collect(&mut self) -> bool {
        std::mem::take(&mut self.collect)
    }

    /// ONCE A FRAME, before the steps: the keys, the boss bars, the deaths the game confirmed, what ends a sequence.
    pub fn frame(&mut self, control: &Control, targets: &mut crate::targets::ErTargets, scene: Scene, fps: f32) {
        self.frame_no += 1;
        let now = Instant::now();
        self.wall_dt = now.duration_since(self.wall).as_secs_f32().min(0.1);
        self.wall = now;
        self.cooldown = (self.cooldown - self.wall_dt).max(0.0);
        let rules = targets.sheet.finish.with_keys(&|key| control.f32(key));
        if rules != self.rules {
            erlog!("finish: the rules are now {rules:?}");
            self.rules = rules;
        }
        self.test_all = control.on("finish.test");
        let fps = fps.clamp(20.0, 240.0);
        if !self.rules.on {
            if self.active() || self.pending.is_some() {
                self.abort("finish.kh = 0");
            }
            targets.deaths.clear();
            if !self.told_off {
                self.told_off = true;
                erlog!("finish: KH3's boss finish is OFF (finish.kh = 0; finish.kh = 1 turns it on: UNTESTED IN THE GAME)");
            }
            return;
        }
        self.told_off = false;
        // ---- the boss bars: who is a boss (remembered: the entry may go with the death)
        for handle in crate::erhud::boss_bar_handles() {
            if let Some(id) = targets.id_of(&handle) {
                if self.bars.insert(id.0, self.frame_no).is_none() {
                    erlog!("finish: target {} has one of Elden Ring's boss bars: its death by Sora's hand is a boss finish", id.0);
                }
            }
        }
        let memory = (self.rules.boss_memory * fps) as u64;
        self.bars.retain(|_, seen| self.frame_no.saturating_sub(*seen) <= memory.max(1));
        // ---- what ends a sequence
        if self.active() || self.pending.is_some() {
            let revived = targets.get(self.pending.as_ref().map_or(self.target, |p| p.target)).is_some_and(|e| e.alive);
            if let Some(reason) = scene.reason() {
                self.abort(reason);
            } else if revived {
                self.abort("THE BODY IS ALIVE AGAIN (a phase change or a scripted survival, not a death)");
            }
        }
        // ---- the deaths the game confirmed since the last frame
        for (id, frames_since_hit) in std::mem::take(&mut targets.deaths) {
            if self.active() || self.pending.is_some() || self.cooldown > 0.0 || scene.reason().is_some() {
                continue;
            }
            let boss = self.bars.contains_key(&id.0);
            let other_boss_alive = self.bars.keys().any(|other| *other != id.0 && targets.get(TargetId(*other)).is_some_and(|e| e.alive));
            let engaged_left = kh_core::host::Targets::engaged_count(targets);
            let death = Death { since_hit: frames_since_hit as f32 / fps, boss, other_boss_alive, engaged_left };
            let kind = kind_of(&self.rules, self.test_all, &death);
            let e = targets.get(id);
            // (ZETTA 2026-10-23, the user: "It also played the standard boss kingdom hearts cinematic death instead of just
            // Zettaflare! Zettaflare overrules that remember!!") a death that is Zettaflare's: no KH3 finish, during or after
            // (its boss rewards still given)
            if zetta_overrules(targets.zetta_death(id), kind) {
                if boss && !other_boss_alive {
                    if let Some(e) = e {
                        self.boss_deaths.push((e.npc_id, e.npc_param, e.hp_max));
                    }
                }
                erlog!("finish: target {} DIED by Zettaflare: ZETTAFLARE OVERRULES KH3's {:?} finish (only Zettaflare's own sequence plays)", id.0, kind);
                continue;
            }
            erlog!(
                "finish: target {}{} DIED by the game's own word {:.2} s after the party's last hit (Sora's or a friend's: ER party S7): boss bar {}, another boss alive {}, engaged hostiles left {} -> {:?}",
                id.0, e.map_or(String::new(), |e| format!(" (c{:04}, max HP {})", e.npc_id, e.hp_max)), death.since_hit, boss, other_boss_alive, engaged_left, kind
            );
            if kind == FinishKind::None {
                continue;
            }
            let half_height = e.map_or(self.rules.ref_half_height, |e| e.half_height);
            if let Some(e) = e {
                self.enemy = FinishSubject { point: e.pos + Vec3::Z * e.half_height, yaw: e.yaw };
            }
            self.pending = Some(Pending { target: id, kind, frames: self.rules.confirm_frames, half_height, bar: e.filter(|_| boss && !other_boss_alive).map(|e| (e.npc_id, e.npc_param, e.hp_max)) });
        }
        // ---- the confirmation: still dead after the frames
        if let Some(p) = self.pending.as_mut() {
            if p.frames > 0 {
                p.frames -= 1;
            } else {
                let p = self.pending.take().unwrap();
                self.boss_deaths.extend(p.bar); // (IMPL_ER31) boss rewards: the same confirmation
                self.target = p.target;
                self.sequence.start(p.kind, self.rules.tuning(p.half_height));
                self.last_phase = FinishPhase::Idle;
                erlog!("finish: THE {} FINISH STARTS on target {} (sequence no. {}; half height {:.0} cm: the cuts' distances x {:.2})", if p.kind == FinishKind::Boss { "BOSS" } else { "BATTLE (port option)" }, p.target.0, self.sequence.started, p.half_height, self.rules.tuning(p.half_height).distance_scale);
            }
        }
        // (the target's camera point while it is still read)
        if self.active() {
            if let Some(e) = targets.get(self.target) {
                self.enemy = FinishSubject { point: e.pos + Vec3::Z * e.half_height, yaw: e.yaw };
            }
        }
    }

    /// ONE FIXED STEP of the script (real 1/60 s): returns the world's rate for this step.
    pub fn step(&mut self, real_dt: f32) -> f32 {
        if !self.sequence.active() {
            if self.frame.active {
                self.frame = FinishFrame::idle();
            }
            return 1.0;
        }
        self.frame = self.sequence.step(real_dt);
        let cues = self.frame.cues.clone();
        self.carry_out(&cues);
        if self.frame.phase != self.last_phase {
            erlog!("finish: phase {:?} -> {:?}: world rate {:.3}, white {:.2}", self.last_phase, self.frame.phase, self.frame.rate, self.frame.white);
            self.last_phase = self.frame.phase;
        }
        self.frame.rate
    }

    fn carry_out(&mut self, cues: &[FinishCue]) {
        for cue in cues {
            match *cue {
                FinishCue::Sound { static_se } => {
                    let played = crate::fx::finish_sound(Some(static_se), None);
                    erlog!("finish: SOUND static SE {static_se} (2D, real time): {}", if played { "started" } else { "NOT started (the audio side is not there, or the id is not in the pack)" });
                }
                FinishCue::SoundState(name) => {
                    let known = crate::fx::finish_sound(None, Some(name));
                    erlog!("finish: sound state {name}{}", if known { "" } else { " (NOT sent: the audio side is not there, or the pack has no such event)" });
                }
                FinishCue::BgmFade { seconds } => erlog!("finish: KH3 fades its battle music out over {seconds} s here: Elden Ring's music is its own (not touched)"),
                FinishCue::StopShakes => self.stop_shakes = true,
                FinishCue::Shake { scale } => self.shake = Some(scale),
                FinishCue::CamerasEnd => erlog!("finish: the cuts end: our normal camera shows"),
                FinishCue::CollectPrizes => self.collect = true,
                FinishCue::InputBack => erlog!("finish: control is back"),
                FinishCue::End => {
                    self.cooldown = self.rules.cooldown;
                    erlog!("finish: THE SEQUENCE IS OVER");
                }
            }
        }
    }

    /// ONCE A FRAME, after the steps: Elden Ring's game speed, the white plate.
    pub fn post(&mut self) {
        if self.active() && self.rules.speed {
            let rate = self.frame.rate.max(self.rules.er_rate_min).min(1.0);
            if rate < 1.0 || SPEED_FIELD.load(Ordering::SeqCst) != 0 {
                speed_write(rate);
            }
            if rate >= 1.0 {
                speed_restore("the script's speed is 1 again");
            }
        } else {
            speed_restore("no sequence runs");
        }
        let white = if self.active() { self.frame.white } else { 0.0 };
        // (ZFIN round 7) + Zettaflare's event white (kh-partyhost PartyRuntime::zetta_white), as the test level's plate
        #[cfg(feature = "party")]
        let white = white.max(crate::party::team::zetta_white());
        if white > 0.0 {
            let c = P.white_colour;
            // (renewed every frame; it lapses by itself 250 ms after our last frame)
            kh_er_render::hud::set_plate(Some((crate::hud::PLATE_LAYER, [c[0], c[1], c[2], white], 250)));
            self.plate = true;
        } else if std::mem::take(&mut self.plate) {
            kh_er_render::hud::set_plate(None);
        }
    }

    /// Ends a running (or pending) sequence now and puts everything back.
    pub fn abort(&mut self, why: &str) {
        if self.pending.take().is_some() {
            erlog!("finish: the pending finish is dropped: {why}");
        }
        if self.sequence.active() {
            erlog!("finish: THE SEQUENCE IS ENDED EARLY: {why}");
            let frame = self.sequence.abort();
            self.carry_out(&frame.cues);
        }
        self.frame = FinishFrame::idle();
        speed_restore(why);
        if std::mem::take(&mut self.plate) {
            kh_er_render::hud::set_plate(None);
        }
    }

    /// Sora stops (a load, the payload's stop).
    pub fn stop(&mut self) {
        self.abort("Sora stopped");
        speed_restore("Sora stopped");
    }
}

impl Drop for ErFinish {
    fn drop(&mut self) {
        speed_restore("the host is gone");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_rows_are_the_defaults_and_every_row_is_a_key() {
        let text = include_str!("../../../sheets/port/sora-er-host.json");
        let json: serde_json::Value = serde_json::from_str(text).unwrap();
        let mut rules = FinishRules { on: true, rate: 0.5, ..FinishRules::DEFAULT };
        let mut seen = Vec::new();
        for row in json["rows"].as_array().unwrap() {
            let id = row[0].as_str().unwrap();
            if id.starts_with("finish.") {
                assert!(rules.apply_row(id, row[1].as_f64().unwrap() as f32), "{id}");
                seen.push(id.to_string());
            }
        }
        assert_eq!(rules, FinishRules::DEFAULT);
        let mut keys: Vec<String> = FinishRules::KEYS.iter().map(|k| k.to_string()).collect();
        keys.sort();
        seen.sort();
        assert_eq!(seen, keys);
        // ON by default (the user, 2026-10-13: "the boss finish must be on by default"); the last-enemy option stays off.
        assert!(FinishRules::DEFAULT.on && !FinishRules::DEFAULT.last_enemy);
        // a key of kh-er.ini over the sheet; an out-of-range value is refused
        let keyed = FinishRules::DEFAULT.with_keys(&|k| match k { "finish.kh" => Some(1.0), "finish.rate" => Some(0.2), "finish.cuts" => Some(9.0), _ => None });
        assert!(keyed.on && keyed.rate == 0.2 && keyed.cuts == 3);
    }

    #[test]
    fn zetta_a_zettaflare_death_gets_no_kh3_finish() {
        assert!(zetta_overrules(true, FinishKind::Boss) && zetta_overrules(true, FinishKind::Battle));
        assert!(!zetta_overrules(false, FinishKind::Boss), "any other death of a boss: KH3's finish as ever");
        assert!(!zetta_overrules(true, FinishKind::None));
    }

    #[test]
    fn who_gets_a_finish() {
        let on = FinishRules { on: true, ..FinishRules::DEFAULT };
        let death = Death { since_hit: 0.2, boss: true, other_boss_alive: false, engaged_left: 0 };
        assert_eq!(kind_of(&on, false, &death), FinishKind::Boss);
        // off; too long after Sora's hit (somebody else's kill, a fall); a duo's first death
        assert_eq!(kind_of(&FinishRules { on: false, ..FinishRules::DEFAULT }, false, &death), FinishKind::None);
        assert_eq!(kind_of(&on, false, &Death { since_hit: 1.6, ..death }), FinishKind::None);
        assert_eq!(kind_of(&on, false, &Death { other_boss_alive: true, ..death }), FinishKind::None);
        // an ordinary enemy: nothing, unless the port option is on and it was the last one
        let plain = Death { boss: false, engaged_left: 0, ..death };
        assert_eq!(kind_of(&on, false, &plain), FinishKind::None);
        let last = FinishRules { last_enemy: true, ..on };
        assert_eq!(kind_of(&last, false, &plain), FinishKind::Battle);
        assert_eq!(kind_of(&last, false, &Death { engaged_left: 2, ..plain }), FinishKind::None);
        // the testing aid
        assert_eq!(kind_of(&on, true, &Death { engaged_left: 3, ..plain }), FinishKind::Boss);
    }

    #[test]
    fn the_script_for_a_large_body_keeps_kh3s_times_and_moves_the_cameras_back() {
        let rules = FinishRules { on: true, ..FinishRules::DEFAULT };
        let human = rules.tuning(90.0);
        assert_eq!((human.rate, human.interp, human.white_delay, human.white_time, human.distance_scale, human.static_se), (0.05, 0.1, 0.15, 0.08, 1.0, 22));
        assert_eq!(rules.tuning(40.0).distance_scale, 1.0);
        assert!((rules.tuning(270.0).distance_scale - 3.0).abs() < 1.0e-5);
        assert_eq!(rules.tuning(2000.0).distance_scale, 4.0);
        let tuned = FinishRules { distance: 1.5, cuts: 0, white: false, se: -1, ..rules }.tuning(180.0);
        assert_eq!((tuned.distance_scale, tuned.cut_count, tuned.white, tuned.static_se), (3.0, 0, false, -1));
    }

    #[test]
    fn what_ends_a_sequence() {
        assert_eq!(Scene::default().reason(), None);
        assert!(Scene { cutscene: true, ..Scene::default() }.reason().unwrap().contains("cutscene"));
        assert!(Scene { sora_dead: true, cutscene: true, menu: true }.reason().unwrap().contains("dead"));
        assert!(Scene { menu: true, ..Scene::default() }.reason().is_some());
    }
}
