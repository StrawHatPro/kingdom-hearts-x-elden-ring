//! (ER-HITSFX round 2, THE USER 2026-11-07: "I'm still hearing the characters make Elden Ring noises, I even heard the
//! tarnished death moan when I teleported") WHO STANDS IN FOR A KH CHARACTER: every hidden Elden Ring body that is drawn,
//! heard and moved as one of ours, so that NO Elden Ring sound of it is heard (ersound.rs mutes them).
//!
//! THE STAND-INS:
//! - the MAIN PLAYER (the hidden Tarnished, whoever leads), read live by ersound.rs and remembered here each frame;
//! - every body of the ALLY POOL (allybody: Donald's, Goofy's, AI Sora's, Mimic Sora's, a staged friend's; all of them
//!   `buddy c0000` copies of the player's model: the Tarnished's voice and steps);
//! - the MIMIC TEAR BOSS'S bodies (the Nokron / Stray boss behind the boss's KH copy) and the boss-summoned Mimic's body:
//!   MIMIC-BOSS's own lookup (`mimic_boss::is_held`: the fight's body by handle and its summoned Mimic foes), walked every
//!   [`MIMIC_EVERY`] frames on the game thread.
//!
//! ROBUST OVER SPAWN, DESPAWN, DEATH, WARP AND LOAD: a stand-in that LEAVES (a body given up, dead or parked out of the pool,
//! the main player object replaced at a load, a boss body no longer held) is remembered for [`GRACE_MS`] by its object AND
//! its handle: a dying or despawning body (its death animation's cry, a fade on teleport) is still ours while the game still
//! plays it. The handle is read from the object the game hands the sound call (a live object), so an object reused for
//! another character is never taken for ours. Everything here is lock-free: the sound hooks run on the game's task threads.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

/// How long a stand-in that left is still ours.
pub const GRACE_MS: u64 = 20_000;
/// The Mimic boss bodies are looked up every this many frames.
pub const MIMIC_EVERY: u32 = 15;
/// Remembered stand-ins that left (a ring: the oldest is overwritten).
const RING: usize = 32;
/// Mimic boss bodies held at once.
const MIMIC_MAX: usize = 8;

/// Which kind of stand-in a character is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    /// The hidden Tarnished (the main player).
    Player,
    /// A body of the ally pool.
    Body,
    /// A Mimic Tear boss body or the boss-summoned Mimic's.
    MimicBoss,
}

impl Who {
    fn code(self) -> u32 {
        match self {
            Who::Player => 1,
            Who::Body => 2,
            Who::MimicBoss => 3,
        }
    }

    fn of_code(code: u32) -> Option<Who> {
        match code {
            1 => Some(Who::Player),
            2 => Some(Who::Body),
            3 => Some(Who::MimicBoss),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Who::Player => "the hidden Tarnished",
            Who::Body => "an ally-pool body",
            Who::MimicBoss => "a Mimic boss body",
        }
    }
}

/// One remembered stand-in that left.
struct Left {
    ptr: AtomicUsize,
    handle: AtomicU64,
    until: AtomicU64,
    who: AtomicU32,
}

static LEFT: [Left; RING] = [const { Left { ptr: AtomicUsize::new(0), handle: AtomicU64::new(0), until: AtomicU64::new(0), who: AtomicU32::new(0) } }; RING];
static NEXT: AtomicUsize = AtomicUsize::new(0);

/// The main player as last seen on the game thread (object, handle bits).
static PLAYER: AtomicUsize = AtomicUsize::new(0);
static PLAYER_HANDLE: AtomicU64 = AtomicU64::new(0);

/// The Mimic boss bodies now (object, handle bits).
static MIMIC: [(AtomicUsize, AtomicU64); MIMIC_MAX] = [const { (AtomicUsize::new(0), AtomicU64::new(0)) }; MIMIC_MAX];
static FRAMES: AtomicU32 = AtomicU32::new(0);

/// Milliseconds since the payload's first question.
pub fn now_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// A stand-in LEFT (its object and handle bits): it stays ours for [`GRACE_MS`].
pub fn left(ptr: usize, handle: u64, who: Who) {
    left_at(ptr, handle, who, now_ms());
}

fn left_at(ptr: usize, handle: u64, who: Who, now: u64) {
    if ptr == 0 {
        return;
    }
    let k = NEXT.fetch_add(1, Ordering::Relaxed) % RING;
    let e = &LEFT[k];
    // (written "until" last: a reader that sees the new time sees the new object; an old time is simply expired)
    e.until.store(0, Ordering::Release);
    e.ptr.store(ptr, Ordering::Release);
    e.handle.store(handle, Ordering::Release);
    e.who.store(who.code(), Ordering::Release);
    e.until.store(now + GRACE_MS, Ordering::Release);
}

/// THE PURE RULE OF A REMEMBERED STAND-IN: the same object with the same handle, within its grace.
pub fn matches_left(entry: (usize, u64, u64), chr: usize, handle: u64, now: u64) -> bool {
    let (ptr, h, until) = entry;
    ptr != 0 && ptr == chr && h == handle && now < until
}

fn recent(chr: usize, handle: u64, now: u64) -> Option<Who> {
    LEFT.iter().find_map(|e| {
        let until = e.until.load(Ordering::Acquire);
        let entry = (e.ptr.load(Ordering::Acquire), e.handle.load(Ordering::Acquire), until);
        matches_left(entry, chr, handle, now).then(|| Who::of_code(e.who.load(Ordering::Acquire))).flatten()
    })
}

/// A body of the ally pool now (lock-free; none without the party feature).
fn pool_body(chr: usize) -> bool {
    #[cfg(feature = "party")]
    {
        crate::allybody::is_body(chr)
    }
    #[cfg(not(feature = "party"))]
    {
        const NO_BODIES: [usize; 0] = [];
        NO_BODIES.contains(&chr)
    }
}

/// The handle bits of a LIVE character object (the one the game hands a sound call).
///
/// # Safety
/// `chr` is a live `ChrIns` (checked plausible here).
pub unsafe fn handle_of(chr: usize) -> u64 {
    if !crate::game::plausible(chr) {
        return 0;
    }
    // (CRASH 2026-11-08) copied by probe.rs: a word that only looked like a character gives 0, never a fault.
    crate::probe::word(chr + std::mem::offset_of!(eldenring::cs::ChrIns, field_ins_handle)).map_or(0, |w| w as u64)
}

/// WHO THIS CHARACTER STANDS IN FOR (None: not ours: an enemy, an NPC, another player, nothing). `live_player`: the main
/// player read live now (0: none, a load).
///
/// # Safety
/// `chr` is the live character object the game hands a sound call (or 0).
pub unsafe fn classify(chr: usize, live_player: usize) -> Option<Who> {
    if chr == 0 {
        return None;
    }
    if chr == live_player || chr == PLAYER.load(Ordering::Relaxed) {
        return Some(Who::Player);
    }
    if pool_body(chr) {
        return Some(Who::Body);
    }
    if MIMIC.iter().any(|(p, _)| p.load(Ordering::Relaxed) == chr) {
        return Some(Who::MimicBoss);
    }
    recent(chr, handle_of(chr), now_ms())
}

/// A stand-in by HANDLE bits (the hit apply's attacker): the main player, a pool body, a Mimic boss body, or one that left.
pub fn classify_handle(bits: u64) -> Option<Who> {
    if bits == 0 || bits as u32 == u32::MAX {
        return None;
    }
    if bits == PLAYER_HANDLE.load(Ordering::Relaxed) {
        return Some(Who::Player);
    }
    #[cfg(feature = "party")]
    if crate::allybody::is_body_handle(bits) {
        return Some(Who::Body);
    }
    if MIMIC.iter().any(|(_, h)| h.load(Ordering::Relaxed) == bits) {
        return Some(Who::MimicBoss);
    }
    let now = now_ms();
    LEFT.iter().find_map(|e| {
        let until = e.until.load(Ordering::Acquire);
        (e.ptr.load(Ordering::Acquire) != 0 && e.handle.load(Ordering::Acquire) == bits && now < until).then(|| Who::of_code(e.who.load(Ordering::Acquire))).flatten()
    })
}

/// The main player's handle bits as last seen (0: none).
pub fn player_handle() -> u64 {
    PLAYER_HANDLE.load(Ordering::Relaxed)
}

/// (ersound.rs frame, game thread) The main player now (a replaced or vanished one is remembered), and every
/// [`MIMIC_EVERY`] frames the Mimic boss bodies.
pub fn frame(live_player: usize) {
    let was = PLAYER.load(Ordering::Relaxed);
    if live_player != was {
        if was != 0 {
            left(was, PLAYER_HANDLE.load(Ordering::Relaxed), Who::Player);
        }
        // SAFETY: the live main player object (or 0), game thread.
        let h = if live_player == 0 { 0 } else { unsafe { handle_of(live_player) } };
        PLAYER.store(live_player, Ordering::Relaxed);
        if live_player != 0 {
            PLAYER_HANDLE.store(h, Ordering::Relaxed);
        }
    }
    if FRAMES.fetch_add(1, Ordering::Relaxed).is_multiple_of(MIMIC_EVERY) {
        mimic_bodies();
    }
}

/// The Mimic boss bodies: every character of the game's sets whose handle MIMIC-BOSS holds (its fight's body, its summoned
/// Mimic); those no longer held are remembered.
fn mimic_bodies() {
    let mut now: Vec<(usize, u64)> = Vec::new();
    each_chr(&mut |p| {
        if now.len() < MIMIC_MAX {
            // SAFETY: a character-set entry pointer of the game thread's walk, checked plausible.
            let bits = unsafe { handle_of(p) };
            if bits != 0 && crate::mimic_boss::is_held(&from_bits(bits)) {
                now.push((p, bits));
            }
        }
    });
    for (p, h) in &MIMIC {
        let (op, oh) = (p.load(Ordering::Relaxed), h.load(Ordering::Relaxed));
        if op != 0 && !now.iter().any(|(np, _)| *np == op) {
            left(op, oh, Who::MimicBoss);
        }
    }
    for (k, (p, h)) in MIMIC.iter().enumerate() {
        let (np, nh) = now.get(k).copied().unwrap_or((0, 0));
        p.store(np, Ordering::Relaxed);
        h.store(nh, Ordering::Relaxed);
    }
}

fn from_bits(bits: u64) -> eldenring::cs::FieldInsHandle {
    // SAFETY: a field-ins handle is 8 plain bytes (game::handle_bits reads it the same way).
    unsafe { std::mem::transmute::<u64, eldenring::cs::FieldInsHandle>(bits) }
}

/// Every character pointer of the game's sets (conv.rs's walk, made fault-proof).
///
/// (CRASH 2026-11-08, the user's game at 124 s: an access violation reading 0xbed714f0 here, an entry word that passed
/// `plausible` but pointed at unmapped memory while the game streamed enemies in.) Nothing the sets point at is followed
/// directly: a set's header, its entry block and each character's handle are copied by probe.rs (`ReadProcessMemory`),
/// so a stale, half-written or garbage word is "not read", never a fault. One copy per set, one per character.
fn each_chr(f: &mut dyn FnMut(usize)) {
    use eldenring::cs::{ChrIns, ChrSet, WorldChrMan};
    use fromsoftware_shared::FromStatic;
    const CAPACITY: usize = std::mem::offset_of!(ChrSet<ChrIns>, capacity);
    const ENTRIES: usize = std::mem::offset_of!(ChrSet<ChrIns>, entries);
    fn walk(set: usize, buf: &mut Vec<u8>, f: &mut dyn FnMut(usize)) {
        let (Some(capacity), Some(entries)) = (crate::probe::u32_at(set + CAPACITY), crate::probe::word(set + ENTRIES)) else {
            return;
        };
        let capacity = capacity as usize;
        if !crate::game::plausible(entries) || capacity == 0 || capacity > 8192 {
            return;
        }
        buf.clear();
        buf.resize(capacity * 16, 0);
        if !crate::probe::read(entries, buf) {
            return;
        }
        let (entries, _) = buf.as_chunks::<16>();
        for entry in entries {
            let p = entry.first_chunk::<8>().map_or(0, |w| usize::from_le_bytes(*w));
            if crate::game::plausible(p) {
                f(p);
            }
        }
    }
    // SAFETY: game thread; the singleton lives as long as the game. Only addresses inside it are taken here; every read
    // past it goes through probe.rs.
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    let mut buf = Vec::new();
    walk(std::ptr::addr_of!(wcm.summon_buddy_chr_set) as usize, &mut buf, f);
    walk(std::ptr::addr_of!(wcm.debug_chr_set) as usize, &mut buf, f);
    for k in 0..wcm.chr_sets.len() {
        let w = crate::word!(wcm.chr_sets[k]);
        if crate::game::plausible(w) {
            walk(w, &mut buf, f);
        }
    }
    walk(std::ptr::addr_of!(wcm.open_field_chr_set.base) as usize, &mut buf, f);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in that left stays ours for its grace, by the same object AND the same handle only.
    #[test]
    fn a_stand_in_that_left_is_ours_for_its_grace_by_object_and_handle() {
        let (chr, h) = (0x7ff0_1000_usize, 0x0000_0002_1000_0007_u64);
        let until = 1_000 + GRACE_MS;
        assert!(matches_left((chr, h, until), chr, h, 1_000), "its death cry right after it left");
        assert!(matches_left((chr, h, until), chr, h, until - 1), "still dying at the end of the grace");
        assert!(!matches_left((chr, h, until), chr, h, until), "the grace is over");
        assert!(!matches_left((chr, h, until), chr, 0x0000_0003_1000_0011, 1_000), "the object reused for another character");
        assert!(!matches_left((chr, h, until), 0x7ff0_2000, h, 1_000), "another object");
        assert!(!matches_left((0, h, until), 0, h, 1_000), "an empty entry");
    }

    /// The ring: a body that left, a replaced main player and a Mimic boss body are found again with their kind; the ring
    /// keeps the newest.
    #[test]
    fn the_ring_remembers_who_left_with_their_kind() {
        let base = 0x7ff7_0000_0000_usize;
        left_at(base + 0x100, 0x11, Who::Body, 5_000);
        left_at(base + 0x200, 0x22, Who::Player, 5_000);
        left_at(base + 0x300, 0x33, Who::MimicBoss, 5_000);
        assert_eq!(recent(base + 0x100, 0x11, 6_000), Some(Who::Body));
        assert_eq!(recent(base + 0x200, 0x22, 6_000), Some(Who::Player));
        assert_eq!(recent(base + 0x300, 0x33, 6_000), Some(Who::MimicBoss));
        assert_eq!(recent(base + 0x300, 0x34, 6_000), None);
        assert_eq!(recent(base + 0x100, 0x11, 5_000 + GRACE_MS), None);
    }

    /// The kinds' codes round-trip; 0 is none.
    #[test]
    fn the_kinds_round_trip() {
        for w in [Who::Player, Who::Body, Who::MimicBoss] {
            assert_eq!(Who::of_code(w.code()), Some(w));
        }
        assert_eq!(Who::of_code(0), None);
    }
}
