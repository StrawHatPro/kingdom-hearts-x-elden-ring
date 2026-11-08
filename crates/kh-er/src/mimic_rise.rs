//! (MIMIC-BOSS hotfix 3, MB3-FOGRISE; THE USER 2026-11-06: "Have the boss itself activate as soon as you enter the fog gate
//! opposed to having to walk up to it") THE MIMIC TEAR'S RISE AT THE FOG GATE, the Elden Ring side (the rules: kh-partyhost
//! `mimicboss::PoolWake`; the trace: build/sora_tmp/weapons/MIMIC-BOSS/STATE.md, MB3-FOGRISE). UNTESTED IN THE GAME.
//!
//! WHAT ELDEN RING DOES (2.7.2.0, its own scripts): Nokron's event 12022860 starts the rise (the pool's anim 20010,
//! CopyPlayerCharacterData, the body enabled at the pool's SpEffect 16307, 8 s later its AI and boss bar) on one of two
//! branches, chosen once by its first-encounter flag 12020851:
//! - set (a return visit): the fog flag 12022855 AND the player in 12022850, the slab just inside the fog: at the fog pass;
//! - not set (the FIRST visit, the user's test save): the player in 12022851, the first encounter's 52 x 65 m box about the
//!   pool (its edge 33 m inside the fog: the walk-up, 9-13 s in the user's runs), OR the pool woken: any damage on it or a
//!   state info 436 / 2 / 5 / 6 / 260 on it. It then sets 12020851 itself.
//!
//! The Stray Mimic Tear's 30202810 has only the fog branch (its fog flag 30202805 AND its region 30202800 just inside the
//! fog): it rises at the fog pass by itself, and nothing is done for it here. (MB3-FOGRISE 2) Its fog flag is set by the fog
//! event (9005800) only once the walk (anim 60060) has ended with the leader IN 30202800 (within 3 s, else the event starts
//! over without the flag), so the frame the flag is on he stands where 30202810 wants him. Both bosses log where the leader
//! stood when the fog flag came on, and a rise still waiting 6 s after it ("THE RISE IS LATE", with his place against the
//! regions).
//!
//! WHAT THIS DOES: on Nokron's first visit, the frame the fog flag is seen set, the pool (entity 12020860) is given
//! SpEffectParam 3051 by the game's own apply-SpEffect function: stateInfo 6 for 0.1 s, the bleed meter's reset, no vfx, no
//! HP change. That is the event's own "state info 6 on the pool", so ER's event runs its rise from the fog pass: the pool,
//! the copy of the player, the body, its AI, its bar and its flags all stay Elden Ring's. Asked again every 0.5 s (10 at most)
//! until the event's first-encounter flag is on. NO FLAG IS WRITTEN by us: 12020851 is the event's own (persistent, as on
//! any first encounter); the fog flag is reset by a reload, and the wake starts over with it.
//!
//! Called once a frame from `mimic_boss::frame` (the boss row of the leader's map or of the body; its flags as read there).
//! Logs: `mimicrise:`.

use std::sync::Mutex;

use eldenring::cs::ChrInsExt;
use kh_partyhost::mimicboss::{self as mb, MimicBossRow, PoolWake, RiseFacts, RiseRoute, WakeAsk};

use crate::erlog;

struct State {
    key: &'static str,
    wake: PoolWake,
    told: String,
    /// (MB3-FOGRISE 2) when the fog flag was first seen on (this pass), and whether the late rise was told
    fog_at: Option<f64>,
    late_told: bool,
}

impl State {
    fn new(key: &'static str) -> State {
        State { key, wake: PoolWake::default(), told: String::new(), fog_at: None, late_told: false }
    }
}

/// (MB3-FOGRISE 2) The leader's MSB place by the pool's own place (None: the pool is not in the loaded world).
fn leader_msb(boss: &MimicBossRow, leader_er: [f32; 3]) -> Option<[f32; 3]> {
    let pool = crate::zphase::chr_of_entity(boss.pool_entity)?;
    let pool_er = crate::game::v3(&pool.modules.physics.position).to_array();
    Some(mb::msb_place(pool_er, boss.pool_at, leader_er))
}

/// (MB3-FOGRISE 2) Where the leader stands against the rise's regions, for the log.
fn regions_words(boss: &MimicBossRow, leader_er: [f32; 3]) -> String {
    match leader_msb(boss, leader_er) {
        Some(p) => {
            let first = boss.first_region.and_then(mb::rise_region).map_or(String::new(), |(c, r, d)| format!(", in the first-encounter region {}: {}", boss.first_region.unwrap_or(0), mb::in_msb_box(c, r, d, p)));
            format!("the leader at MSB ({:.2}, {:.2}, {:.2}): in the fog's region {}: {}{first}", p[0], p[1], p[2], boss.fog_region, mb::in_fog_region(boss, p))
        }
        None => format!("the pool (entity {}) not in the loaded world: the leader's MSB place unknown", boss.pool_entity),
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// One line once (the same line is not written again until another one was).
fn say(told: &mut String, line: String) {
    if *told != line {
        erlog!("{line}");
        *told = line;
    }
}

/// ONCE A FRAME (`mimic_boss::frame`, the boss's row known): `first` = its first-encounter flag as the game has it (false
/// for a boss without one), `body_enabled` = the game has enabled its body (risen), `leader_er` = the leader's Elden Ring
/// physics place.
pub fn frame(boss: &'static MimicBossRow, defeated: bool, fog_flag: bool, first: bool, body_enabled: bool, leader_er: [f32; 3], now: f64) {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = g.get_or_insert_with(|| State::new(boss.key));
    if s.key != boss.key {
        *s = State::new(boss.key);
    }
    // (MB3-FOGRISE 2, THE USER 2026-11-07: "the other mimic tear boss must start as soon as you enter the fog gate as well")
    // THE FOG PASS AS ELDEN RING CHECKS IT: its fog event sets the fog flag only with the leader in the fog's region, and the
    // rise's event waits on the same two facts; the log shows where he stood, and a rise still waiting 6 s later is told
    if !fog_flag {
        (s.fog_at, s.late_told) = (None, false);
    } else if s.fog_at.is_none() {
        s.fog_at = Some(now);
        if !defeated && !body_enabled {
            erlog!("mimicrise: {}: THE FOG FLAG {} IS ON ({:?} route): {}", boss.name, boss.fog_flag, mb::rise_route(boss, first), regions_words(boss, leader_er));
        }
    }
    if let Some(at) = s.fog_at.filter(|_| !s.late_told) {
        if mb::rise_late(defeated, now - at, body_enabled) {
            s.late_told = true;
            erlog!("!!! mimicrise: {}: THE RISE IS LATE: the fog flag on for {:.1} s and the body not enabled ({:?} route; Elden Ring's event waits on its fog flag AND the leader in region {}{}): {}", boss.name, now - at, mb::rise_route(boss, first), boss.fog_region, boss.first_region.map_or(String::new(), |r| format!(", or on the first visit region {r} / the pool's wake")), regions_words(boss, leader_er));
        }
    }
    let facts = RiseFacts { defeated, fog_flag, first_done: first, body_enabled };
    // the pool is looked up only while a wake is due (a walk of the character sets)
    let mut pool = if mb::pool_wake_due(boss, &facts) { crate::zphase::chr_of_entity(boss.pool_entity) } else { None };
    match s.wake.step(boss, &facts, pool.is_some(), now) {
        WakeAsk::Apply(n) => {
            if let Some(chr) = pool.as_mut() {
                chr.apply_speffect(mb::POOL_WAKE_SPEFFECT, false);
                let on = chr.special_effect.entries().take(256).any(|e| e.param_id == mb::POOL_WAKE_SPEFFECT);
                erlog!("mimicrise: {}: THE FOG IS PASSED on the first visit (fog flag {} on, first-encounter flag {} off): the pool (entity {}) woken by Elden Ring's own condition, SpEffect {} (state info {}) given (try {n}; on it now: {on}); the event rises the boss from here instead of the walk-up", boss.name, boss.fog_flag, boss.first_flag.unwrap_or(0), boss.pool_entity, mb::POOL_WAKE_SPEFFECT, mb::POOL_WAKE_STATE_INFO);
            }
        }
        WakeAsk::Took { after } => erlog!("mimicrise: {}: Elden Ring's event took the wake {after:.2} s after it was given (its first-encounter flag {} on, or the body enabled): the rise runs", boss.name, boss.first_flag.unwrap_or(0)),
        WakeAsk::GaveUp => erlog!("mimicrise: {}: the event did not take the wake after {} tries ({} s apart): Elden Ring's own walk-up start remains (its region {})", boss.name, mb::POOL_WAKE_TRIES, mb::POOL_WAKE_RETRY_S, boss.first_region.unwrap_or(0)),
        WakeAsk::NoPool => say(&mut s.told, format!("mimicrise: {}: the wake is due but the pool (entity {}) is not in the loaded world", boss.name, boss.pool_entity)),
        WakeAsk::Wait | WakeAsk::Idle => {}
    }
    if fog_flag && !defeated && mb::rise_route(boss, first) == RiseRoute::FogPass {
        say(&mut s.told, format!("mimicrise: {}: the fog is passed; Elden Ring's own event starts the rise at the fog pass by itself (its fog flag {} and the fog's region {}): nothing to wake", boss.name, boss.fog_flag, boss.fog_region));
    }
}
