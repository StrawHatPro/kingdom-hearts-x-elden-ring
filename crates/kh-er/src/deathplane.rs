//! (DEATHPLANE, THE USER 2026-11-03: "KH characters should die from Elden Ring's death barriers (like the ocean)") ONE RULE
//! FOR EVERY KH CHARACTER: the leader (Sora, Donald, Goofy or anyone added later), the AI friends, AI Sora, Mimic Sora.
//! Spirit ashes are Elden Ring's own characters: the game's own barrier kills them, nothing of ours stands in its way.
//!
//! ELDEN RING'S OWN DEATH BARRIERS, READ IN THE EXE (2.7.2.0; 2.7.1.0 in brackets, the same bytes with rel32 / RIP operands
//! wildcarded). Not an event script: none of the 470 maps' scripts kills the player by region (every `2004[4]`
//! ForceCharacterDeath takes its entity from a parameter, and every caller passes an enemy). The barriers are COLLISION:
//! - Every character's physics module (`CSChrPhysicsModule`, +0x10) carries a CONTACT TRACKER, made by 0x45a5e0
//!   [0x45a500]: `{ chr, physics module, data module, shape (+0x18), list (+0x20 / +0x28), FILTER (+0x30) = 0x66, flags }`.
//!   The physics step 0x4614f0 [0x461410] asks for its query (`+0x35 = 1`) in the proxy modes 2 / 3; the physics update
//!   0x4612c0 [0x4611e0] runs it, 0x45ab40 [0x45aa60]: 0x45ac70 [0x45ab90] sweeps the tracker's shape from the module's
//!   last position (+0x80) to its position (+0x70) with the game's shape cast 0x1880f50 (havok.rs's cast) and filter 0x66.
//! - Each contact's body id goes through 0xc68bc0 [0xc68700] to its collision record: kind 0 = a collision object at
//!   record + 0x10, whose SPECIAL KIND is 0x707420 [0x706f70] (`obj + 0x18`: property 5 of its Havok body, one byte).
//! - The decider 0x45b200 [0x45b120] keeps a contact by its kind (13 .. 28). The three DEATH kinds ask the game's no-death
//!   rule 0x437a60 [0x4379d0] (`IsNoDead(data module)`: +0x19B bit 0, `all_no_dead`, `player_no_dead` for the main player)
//!   BEFORE the contact is kept: with the hidden body's no-death switch held (sora.rs `fall_switches`) the contact is
//!   dropped, the HP never moves. That is why a KH leader never died in the sea.
//! - What the game does with them, 0x45af00 [0x45ae20] (jump table at 0x45b1bc):
//!   - 13 FALL-DEATH CAMERA (the player only): `ChrIns + 0x1C8 |= 4` (the SDK's `request_falldeath_camera`), the ride too;
//!   - 14 LETHAL FALL: the fall module's `force_max_fall_height` (+0x1D = 1): his landing is a fall of the maximum height
//!     (0x44e320 [0x44e240] measures a fall: the maximum when the flag is set; IsNoDead caps it);
//!   - 15 KILL: unless dead already, 0x3ee360 and 0x44e170 [0x44e090]: `SetHp(0)` (0x4388a0, the HP clamp asks IsNoDead
//!     again through 0x437d30), the fall module's death kind; the same on the ride.
//!   - the other kinds (17, 20, 21: the water's surface, 23, 27 / 28: no Torrent) are no deaths.
//!
//! THE SIGNAL WE USE: THE GAME'S OWN QUERY at each KH character's place, every frame: its shape cast (havok.rs
//! `cast_sphere`, the same function and parameter block) with the TRACKER'S OWN FILTER, read out of the tracker's
//! constructor (0x66), from where he was the frame before to where he is (his body from head to feet, his own radius), and
//! every contact classified by the game's own two functions (record, kind). Kinds 13 / 14 / 15 are the barriers.
//! - The leader's hidden body has the game's test, but the game drops its death contacts while we hold the no-death switch;
//!   the same query at the leader's feet is that test without the switch (the body stands where he stands).
//! - The AI friends', AI Sora's and Mimic Sora's Elden Ring bodies are kept by the ally pool (allybody.rs): written to their
//!   feet, but passive and without map collision; their Havok proxies do not follow our writes (measured: allybody_live
//!   "the proxy's own place reads (-32, 0, 0)"), parked ones lie under the leader. So the same query runs at each one's
//!   KH place: the same regions and planes, in world space, by the game's own classification.
//!
//! THE RULE ([`Doom`], the same for every kind of character):
//! - KILL (15): his death at once, given again every frame until he is down (a Second Chance, a dodge frame, a guard never
//!   saves him from it), at most [`HOLD`] s after the last touch while he still stands.
//! - LETHAL FALL (14): his death at his next landing (as the game's maximum-height landing), or at once if the leader's net
//!   finds no ground under him (a pit); a touch while he stands that is not followed by a fall within [`FLOOR_CLEAR`] s lapses.
//! - FALL-DEATH CAMERA (13): no death by itself; while he is in the air after it, the leader's net gives no put-back: the
//!   fall into the void is his death (the game's own camera zone above a pit).
//! - THE GAME'S FALL DAMAGE STAYS DISCARDED (KH3 has none: guards.rs `FallGuard`, `BodyFall`, `blame`): a fall without a
//!   barrier touch is never a death here, whatever its height.
//!
//! THE DEATHS ((KILLPLANE, THE USER 2026-11-06: "kill planes in Elden Ring need to ISNTANTLY kill the party leader or party
//! members, it should ignore any abilities you have") INSTANT AND UNCONDITIONAL, one route per kind of character, never a
//! hit): a Sora core (the leader Sora, AI Sora, Mimic Sora): [`kill_core`] (kh-core `Sora::kill_unconditional`: HP 0 and
//! KH3's knockdown at his next step, past Second Chance, Withstand Combo, Leaf Bracer, i-frames, guards and the auto-revive
//! item); a friend (leader or AI): [`knock_out`] (kh-party `NpcPawn::kill_unconditional`: HP 0 and the KO at once, HELD: no
//! revive until the next Sora host, its respawn). The leader's death is the party's (death.rs: KH3's game over, Continue =
//! the last save, or Elden Ring's own death and respawn), always a NORMAL one ([`Deathplane::leader_killed`]); Mimic Sora
//! then leaves as Died (KH3's knockdown and the silver dissolve).
//!
//! KEYS: `barrier.death` (1; 0 = none of this), `log.barrier` (1). Logs: `barrier:` lines.

use glam::Vec3;
use kh_host::party::PartyChara;

/// The special collision kinds of the game's contact tracker (2.7.2.0 0x45af00's switch, 13 .. 28).
pub const KIND_FALL_CAMERA: i32 = 13;
pub const KIND_LETHAL_FALL: i32 = 14;
pub const KIND_KILL: i32 = 15;
/// The lowest and highest kind the tracker acts on (the rest is ordinary collision).
pub const KIND_SPECIAL: (i32, i32) = (13, 28);

/// A KILL doom given up this long after its last touch while he still stands (s).
pub const HOLD: f32 = 5.0;
/// A LETHAL FALL touched while standing lapses after this long on the floor without a fall (s).
pub const FLOOR_CLEAR: f32 = 0.5;
/// A move longer than this between two frames is a warp: no sweep across it (m).
pub const WARP_M: f32 = 5.0;

/// One of the game's death barriers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Barrier {
    /// 13: the fall-death camera's zone (above a pit).
    FallCamera,
    /// 14: the landing after it is a fall of the maximum height.
    LethalFall,
    /// 15: the instant kill (the sea's kill plane, a pit's floor).
    Kill,
}

impl Barrier {
    pub fn of_kind(kind: i32) -> Option<Barrier> {
        match kind {
            KIND_FALL_CAMERA => Some(Barrier::FallCamera),
            KIND_LETHAL_FALL => Some(Barrier::LethalFall),
            KIND_KILL => Some(Barrier::Kill),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Barrier::FallCamera => "the fall-death camera zone (13)",
            Barrier::LethalFall => "a lethal-fall zone (14)",
            Barrier::Kill => "a kill plane (15)",
        }
    }
}

/// Whom the rule judges. AI Sora is `Ally(PartyChara::Sora)`. (Without the party feature there is only the leader.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Who {
    Leader,
    #[cfg(feature = "party")]
    Ally(PartyChara),
    #[cfg(feature = "party")]
    Mimic,
}

impl Who {
    pub fn name(self, leader: PartyChara) -> String {
        match self {
            Who::Leader => format!("the leader {}", leader.name()),
            #[cfg(feature = "party")]
            Who::Ally(PartyChara::Sora) => "AI Sora".to_string(),
            #[cfg(feature = "party")]
            Who::Ally(c) => format!("the AI friend {}", c.name()),
            #[cfg(feature = "party")]
            Who::Mimic => "Mimic Sora".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FallDoom {
    /// In the air since the touch.
    airborne: bool,
    /// On the floor this long without having been in the air since the touch (s).
    floor_for: f32,
}

/// ONE CHARACTER'S DOOM: what the barriers he touched still owe him.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Doom {
    /// Seconds since the last KILL touch (or since a lethal fall landed).
    kill: Option<f32>,
    fall: Option<FallDoom>,
    /// Seconds in the air since a fall-death camera touch.
    camera: Option<f32>,
    /// Where he was last frame (Elden Ring metres): the sweep's start.
    prev: Option<Vec3>,
}

impl Doom {
    /// A barrier he touched this frame. True: a doom begins with it (for the log).
    pub fn touch(&mut self, barrier: Barrier, on_floor: bool) -> bool {
        match barrier {
            Barrier::Kill => self.kill.replace(0.0).is_none(),
            Barrier::LethalFall => {
                let new = self.fall.is_none() && self.kill.is_none();
                let f = self.fall.get_or_insert(FallDoom::default());
                f.airborne |= !on_floor;
                if !on_floor {
                    f.floor_for = 0.0;
                }
                new
            }
            Barrier::FallCamera => {
                let new = self.camera.is_none() && self.kill.is_none() && self.fall.is_none();
                if !on_floor || self.camera.is_some() {
                    self.camera.get_or_insert(0.0);
                }
                new && self.camera.is_some()
            }
        }
    }

    /// THE RULE, once a frame after the touches: the death to give him now (again every frame until he is down), if any.
    pub fn frame(&mut self, alive: bool, on_floor: bool, dt: f32) -> Option<Barrier> {
        if !alive {
            // he is down: the barrier's work is done (his next life starts clean)
            *self = Doom { prev: self.prev, ..Doom::default() };
            return None;
        }
        let dt = dt.max(0.0);
        if let Some(f) = self.fall.as_mut() {
            if !on_floor {
                f.airborne = true;
                f.floor_for = 0.0;
            } else if f.airborne {
                // the landing of a lethal fall: his death, from now on given as a kill's
                self.fall = None;
                self.camera = None;
                self.kill = Some(0.0);
                return Some(Barrier::LethalFall);
            } else {
                f.floor_for += dt;
                if f.floor_for > FLOOR_CLEAR {
                    self.fall = None;
                }
            }
        }
        if let Some(t) = self.camera.as_mut() {
            if on_floor {
                self.camera = None;
            } else {
                *t += dt;
            }
        }
        if let Some(t) = self.kill.as_mut() {
            *t += dt;
            if *t > HOLD {
                self.kill = None;
                return None;
            }
            return Some(Barrier::Kill);
        }
        None
    }

    /// A fall into the void (the leader's net: no ground at all under him) is his death, not a put-back.
    pub fn void_is_death(&self) -> bool {
        self.kill.is_some() || self.fall.is_some() || self.camera.is_some()
    }

    /// THE SWEEP OF THIS FRAME: from where he was to where he is (Elden Ring metres); after a warp (or at the first frame)
    /// it starts where he is.
    pub fn sweep(&mut self, at: Vec3) -> (Vec3, Vec3) {
        let from = self.prev.filter(|p| p.is_finite() && (*p - at).length() <= WARP_M).unwrap_or(at);
        self.prev = Some(at);
        (from, at)
    }

    /// He is not tested this frame (a cutscene, a load, the party off): the next sweep starts afresh.
    pub fn pause(&mut self) {
        self.prev = None;
    }
}

/// THE CAST OF ONE CHARACTER (Elden Ring metres, Y up): his body from head to feet, swept from his place of last frame to
/// his place now. Returns (origin, delta, radius). `radius` / `top`: his capsule's radius and height above his feet.
pub fn cast_of(from_feet: Vec3, to_feet: Vec3, radius: f32, top: f32) -> (Vec3, Vec3, f32) {
    let r = if radius.is_finite() { radius.clamp(0.1, 0.6) } else { 0.3 };
    let top = if top.is_finite() { top.clamp(2.0 * r, 4.0) } else { 1.8 };
    let origin = from_feet + Vec3::Y * (top - r);
    let end = to_feet + Vec3::Y * r;
    (origin, end - origin, r)
}

/// What the host knows of one character this frame (KH3 cm): his feet, his capsule's radius and height above his feet,
/// alive, on the floor; `test`: the game's query runs for him this frame (not in a cutscene, a load, a hold of the body).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facts {
    pub feet_kh: Vec3,
    pub radius: f32,
    pub top: f32,
    pub alive: bool,
    pub on_floor: bool,
    pub test: bool,
}

/// EVERY KH CHARACTER'S DOOM (one per Sora host: a respawn, a load or fast travel starts them all clean).
#[derive(Clone, Debug, Default)]
pub struct Deathplane {
    dooms: Vec<(Who, Doom)>,
    /// (KILLPLANE) the leader was killed by a barrier in this host: his death is the party's NORMAL death (Elden Ring's
    /// player dies outright in a kill plane, even at the tutorial boss's place): never the scripted death's 1-HP spare.
    pub leader_killed: bool,
    /// (test aids, test profile only) the values of `test.barrier_probe` / `test.barrier_push` last seen (a one-shot fires
    /// on a CHANGE of its value, never at launch).
    pub aids: [Option<Option<String>>; 2],
}

/// (test aids) A one-shot of `aids[k]`: the new value when it changed since the last frame (never the value at launch).
pub fn aid_fired(seen: &mut Option<Option<String>>, now: Option<&str>) -> Option<String> {
    let now = now.map(str::to_string);
    match seen.replace(now.clone()) {
        Some(before) if before != now => now,
        _ => None,
    }
}

#[cfg(feature = "party")]
/// (test aid `test.barrier_push = who,x,y,z` or `who,+dx,+dy,+dz`) Whom to put where (Elden Ring metres; `+`: relative to
/// the leader's feet). None: not understood.
pub fn push_of(text: &str, leader_feet: Vec3) -> Option<(String, Vec3)> {
    let mut parts = text.split(',').map(str::trim);
    let who = parts.next().filter(|w| !w.is_empty())?.to_ascii_lowercase();
    let rest: Vec<&str> = parts.collect();
    let relative = rest.first().is_some_and(|p| p.starts_with('+'));
    let v: Vec<f32> = rest.iter().filter_map(|p| p.trim_start_matches('+').parse().ok()).collect();
    let [x, y, z] = v[..] else { return None };
    let at = if relative { leader_feet + Vec3::new(x, y, z) } else { Vec3::new(x, y, z) };
    at.is_finite().then_some((who, at))
}

impl Deathplane {
    pub fn doom(&mut self, who: Who) -> &mut Doom {
        let k = match self.dooms.iter().position(|(w, _)| *w == who) {
            Some(k) => k,
            None => {
                self.dooms.push((who, Doom::default()));
                self.dooms.len() - 1
            }
        };
        &mut self.dooms[k].1
    }

    /// THE SHARED RULE, one frame of one character: the barriers he touched (the game's query, or a mock), whether he is
    /// alive and on the floor. Returns the touches that began a doom (for the log) and the death to give him now.
    pub fn judge(&mut self, who: Who, touches: &[Barrier], alive: bool, on_floor: bool, dt: f32) -> (Vec<Barrier>, Option<Barrier>) {
        let doom = self.doom(who);
        let mut began = Vec::new();
        if alive {
            for t in touches {
                if doom.touch(*t, on_floor) && !began.contains(t) {
                    began.push(*t);
                }
            }
        }
        (began, doom.frame(alive, on_floor, dt))
    }

    pub fn void_is_death(&self, who: Who) -> bool {
        self.dooms.iter().any(|(w, d)| *w == who && d.void_is_death())
    }

    /// The characters not judged this frame are forgotten (a friend left the party, Mimic Sora is gone).
    pub fn keep_only(&mut self, judged: &[Who]) {
        self.dooms.retain(|(w, _)| judged.contains(w));
    }
}

/// (the leader's net, guards.rs `net_verdict`) A doomed leader's fall into the void is his death, never a put-back.
pub fn net(verdict: crate::guards::NetVerdict, void_is_death: bool) -> crate::guards::NetVerdict {
    match verdict {
        crate::guards::NetVerdict::PutBack if void_is_death => crate::guards::NetVerdict::Death,
        v => v,
    }
}

/// (feature `party`) (KILLPLANE) THE PARTY'S KO for a barrier: kh-party's `kill_unconditional` (HP 0 and the Die state at
/// once: the KO clip, the KO event), past every hit rule (a guard, Interpose, a dodge, invincibility, Second Chance, Withstand
/// Combo, a dying reaction first), and HELD: no revive (the timer, a heal, an item, a friend) until the next Sora host stands
/// the party up (death.rs `revive_all`: a respawn, a load, fast travel), as Elden Ring's player stays dead until his respawn.
/// True: he went down now.
#[cfg(feature = "party")]
pub fn knock_out(party: &mut kh_party::Party, id: kh_party::MemberId) -> bool {
    party.member_mut(id).is_some_and(kh_party::pawn::NpcPawn::kill_unconditional)
}

/// (KILLPLANE) A SORA CORE'S DEATH for a barrier (the leader Sora, AI Sora, Mimic Sora): kh-core's `kill_unconditional`, his
/// HP 0 and KH3's knockdown at his next step, whatever he has (Second Chance, Withstand Combo, Leaf Bracer, i-frames, a guard,
/// the auto-revive item). From above his feet (he is thrown back from where he faces).
pub fn kill_core(core: &mut kh_core::Sora, feet_kh: Vec3) {
    core.kill_unconditional(feet_kh + Vec3::Z * 100.0);
}

// =============================================================================================== the game's side

#[cfg(windows)]
pub use game_side::*;

#[cfg(windows)]
mod game_side {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::OnceLock;

    use glam::Vec3;

    use super::{Barrier, KIND_SPECIAL};
    use crate::erlog;
    use crate::havok::{locate, Located};

    /// The Torrent-zone check's loop (2.7.2.0 0x45aa30 [0x459950 .. ]): each contact's body id -> its collision record
    /// (capture 1) -> kind 0 = an object at +0x10 -> its special kind (capture 2), compared with 27 / 28.
    const PAT_LOOKUP_AND_KIND: &str = "8b 4c 24 54 e8 $ { ' } 48 85 c0 74 23 66 83 78 08 00 75 1c 48 8b 48 10 48 85 c9 74 13 48 83 c1 18 e8 $ { ' } 83 f8 1b 74 16 83 f8 1c";

    /// The collision record of a Havok body id (`c68bc0(u32 id) -> record`, null for none).
    pub(crate) const RECORD_FN: Located = Located { name: "collision record of a body id (barrier kinds)", pattern: PAT_LOOKUP_AND_KIND, capture: 1, rva_2_7_1_0: 0x00c6_8700, rva_2_7_2_0: 0x00c6_8bc0 };
    /// The special kind of a collision object (`707420(obj + 0x18) -> u8`).
    pub(crate) const KIND_FN: Located = Located { name: "special collision kind (barrier kinds)", pattern: PAT_LOOKUP_AND_KIND, capture: 2, rva_2_7_1_0: 0x0070_6f70, rva_2_7_2_0: 0x0070_7420 };
    /// The contact tracker's constructor (its filter word is its last store: `mov qword [rcx+30h], imm32`).
    pub(crate) const TRACKER_CTOR: Located = Located {
        name: "chr contact tracker constructor (barrier filter)",
        pattern: "33 c0 48 89 11 48 89 41 08 48 89 41 10 48 89 41 18 48 89 41 20 48 89 41 28 48 8b c1 48 c7 41 30 ? 00 00 00 c3",
        capture: 0,
        rva_2_7_1_0: 0x0045_a500,
        rva_2_7_2_0: 0x0045_a5e0,
    };
    /// Where the filter's immediate is in the constructor.
    pub(crate) const FILTER_AT: usize = 32;
    /// The filter the constructor writes in both versions (read from the code at run time; this is the check).
    pub(crate) const FILTER_RECORDED: u32 = 0x66;

    /// Every address of this file (gamever.rs's offline check runs them against both executables).
    #[cfg(test)]
    pub(crate) const LOCATED: [Located; 3] = [RECORD_FN, KIND_FN, TRACKER_CTOR];

    #[derive(Clone, Copy)]
    struct Fns {
        record: unsafe extern "C" fn(u32) -> usize,
        kind: unsafe extern "C" fn(usize) -> u32,
        filter: u32,
    }

    static FNS: OnceLock<Option<Fns>> = OnceLock::new();
    /// The special kinds seen so far (bit = kind - 13), for the log.
    static SEEN: AtomicU32 = AtomicU32::new(0);

    fn fns() -> Option<Fns> {
        *FNS.get_or_init(|| {
            let (record, kind, ctor) = (locate(&RECORD_FN), locate(&KIND_FN), locate(&TRACKER_CTOR));
            let (Some(record), Some(kind), Some(ctor)) = (record, kind, ctor) else {
                erlog!("barrier: the game's barrier functions are NOT all found (record {record:?}, kind {kind:?}, tracker {ctor:?}): no KH character dies of Elden Ring's death barriers");
                return None;
            };
            // SAFETY: the constructor's own bytes, found by its unique pattern; `FILTER_AT` lies inside the matched bytes.
            let filter = unsafe { std::ptr::read_unaligned((ctor + FILTER_AT) as *const u32) };
            if filter == 0 || filter > 0x7f {
                erlog!("barrier: the contact tracker's filter reads {filter:#x} (not a layer): no barrier deaths");
                return None;
            }
            erlog!(
                "barrier: THE GAME'S DEATH BARRIERS are read by its own contact query: the tracker's filter {filter:#x}{} (its constructor at {ctor:#x}), body -> record {record:#x}, record -> kind {kind:#x}; kinds 13 / 14 / 15 are barriers",
                if filter == FILTER_RECORDED { " (as recorded)" } else { " (NOT the recorded 0x66: used as the code says)" }
            );
            // SAFETY: the two functions found by the pattern above, with the signatures the game's own callers use.
            Some(Fns { record: unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(u32) -> usize>(record) }, kind: unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(usize) -> u32>(kind) }, filter })
        })
    }

    pub fn ready() -> bool {
        fns().is_some()
    }

    /// One contact of the query with a special collision.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Touch {
        pub kind: i32,
        pub barrier: Option<Barrier>,
        /// Where (Elden Ring metres).
        pub at: Vec3,
    }

    /// THE GAME'S CONTACT QUERY for one character (game thread): his body swept from `from` to `to` (feet, Elden Ring
    /// metres), every contact with a special collision (kinds 13 .. 28) by the game's own classification.
    pub fn touches(from: Vec3, to: Vec3, radius: f32, top: f32) -> Vec<Touch> {
        let Some(f) = fns() else { return Vec::new() };
        if !from.is_finite() || !to.is_finite() {
            return Vec::new();
        }
        let (origin, delta, r) = super::cast_of(from, to, radius, top);
        let mut hits = Vec::new();
        crate::havok::cast_sphere(origin, delta, r, f.filter, false, &mut hits);
        let mut out: Vec<Touch> = Vec::new();
        let mut bodies: Vec<u32> = Vec::new();
        for h in &hits {
            if bodies.contains(&h.body) || h.body == 0x00ff_ffff {
                continue;
            }
            bodies.push(h.body);
            // SAFETY: the game's own lookup of a body id its own cast reported (it masks the id and returns null for none);
            // the record's kind word and object are read as the game's tracker reads them (kind 0 = an object at +0x10),
            // and the kind function is called only on such an object, as the game calls it.
            let kind = unsafe {
                let rec = (f.record)(h.body);
                if rec == 0 || std::ptr::read_unaligned((rec + 8) as *const u16) != 0 {
                    continue;
                }
                let obj = std::ptr::read_unaligned((rec + 0x10) as *const usize);
                if obj == 0 {
                    continue;
                }
                ((f.kind)(obj + 0x18) & 0xff) as i32
            };
            if !(KIND_SPECIAL.0..=KIND_SPECIAL.1).contains(&kind) {
                continue;
            }
            let bit = 1u32 << (kind - KIND_SPECIAL.0);
            if SEEN.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
                erlog!("barrier: the game's special collision kind {kind} met for the first time at Elden Ring ({:.2}, {:.2}, {:.2}){}", h.position.x, h.position.y, h.position.z, Barrier::of_kind(kind).map_or(String::new(), |b| format!(": {}, A DEATH BARRIER", b.name())));
            }
            out.push(Touch { kind, barrier: Barrier::of_kind(kind), at: h.position });
        }
        out
    }

    /// (test aid `test.barrier_probe = metres`) WHERE THE GAME'S SPECIAL COLLISION IS AROUND HIM: columns straight down
    /// (from 40 m above his feet to 120 m below) on rings every 8 m out to `reach` (16 ways), each one's special contacts.
    /// Returns (the column's top, its touches).
    pub fn probe(center: Vec3, reach: f32) -> Vec<(Vec3, Vec<Touch>)> {
        let mut out = Vec::new();
        let reach = if reach.is_finite() { reach.clamp(8.0, 120.0) } else { 40.0 };
        let mut ring = 0.0f32;
        while ring <= reach {
            let ways = if ring == 0.0 { 1 } else { 16 };
            for k in 0..ways {
                let a = k as f32 * std::f32::consts::TAU / ways as f32;
                let top = center + Vec3::new(a.cos() * ring, 40.0, a.sin() * ring);
                let found = touches(top, top - Vec3::Y * 160.0, 0.3, 0.6);
                if !found.is_empty() {
                    out.push((top, found));
                }
            }
            ring += 8.0;
        }
        out
    }
}

#[cfg(test)]
#[path = "deathplane_tests.rs"]
mod tests;
