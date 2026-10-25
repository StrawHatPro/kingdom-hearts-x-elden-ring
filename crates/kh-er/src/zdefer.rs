//! (ZETTA 2026-10-23) ZETTAFLARE OWNS THE PRESENTATION UNTIL THE HAND-BACK, on every Elden Ring character (the user: "Zettaflare
//! should overrule the "beat boss" cutscene and camera so that it's the only one that plays and doesn't get interrupted";
//! "Beating Melania first phase with level 1 Zettaflare made me clip through the map, ensure that if a boss has a second
//! phase that this does not happen"; "Zettaflare should completely overrule any sort of animation for an enemy dying, I had
//! enemies doing their normal death animations during Zettaflare and ragdolling which is not correct"). GENERIC, no
//! per-enemy data:
//!
//! - THE DAMAGE OF ZETTAFLARE'S HITS (the forced hits, `HIT_FLAG_FORCE`: Zettaflare's only) IS OWED, NOT TAKEN, while its
//!   presentation runs: the game's HP of the body is untouched, so nothing of the game's own fires mid-sequence (its death
//!   animation and ragdoll, a boss's "felled" sequence and camera, a phase change's cutscene and warp, our own boss finish);
//!   KH3's side sees the HP it would have ([`Book::kh_hp`]): the finale (white, silhouette, crumble) plays on a body the
//!   game still holds alive, held in place by us (never its death).
//! - AT THE HAND-BACK ([`Owner`]: Zettaflare's sequence over, or its party no longer stepped) the owed HP is taken at once:
//!   a killed body dies hidden (the crumble hid it, `zfinale`; a body with no finale is hidden as it dies), a boss whose
//!   game refuses the death (a phase change: its HP floored, its cutscene) is shown again for its next phase, and every
//!   cinematic of the game's plays after Zettaflare's, never inside it.
use std::collections::HashMap;

/// The HP owed per character (target id -> (its handle's bits, Elden Ring HP owed)).
#[derive(Default, Debug, Clone)]
pub struct Book {
    owed: HashMap<u32, (u64, i32)>,
    /// (2026-10-23, the user: "mod detects the second phase and just throws them back") the bodies whose death would be a
    /// phase change: KH3's side keeps them alive (1 HP at least): the survivor's path, the throw; the game's phase change after
    /// the hand-back.
    phase: std::collections::HashSet<u32>,
}

/// (2026-10-23) A BOSS'S DEATH THAT IS A PHASE CHANGE, told at the hit, generically: the game's own "no death" state on the
/// body (`immortal`: a special effect of its with SpEffectParam `noDead`, the HP floor that stopped Malenia's phase 1 at 1 HP),
/// or (the hint) more rows of the same character standing on it (her phase-2 instances).
pub fn phase_change(immortal: bool, twins: bool) -> bool {
    immortal || twins
}

/// (ZETTA 2026-10-23, the user's log: Malenia's phase 1 floored at 1 HP at the hand-back, none of the special effects said
/// `noDead`) WHICH OF THE GAME'S NO-DEATH STATES THE CHARACTER IS IN (None: mortal): its data module's no-death switch
/// (`CSChrDataModule` + 0x19B bit 0, the switch the debug menus' "No Dead" and the events' immortality write; bit 1 is
/// "no damage"), its TimeAct's immortality (ChrActionFlag 96 SET_IMMORTALITY), a special effect whose SpEffectParam row
/// says `noDead`. Game thread.
pub fn no_dead_of(chr: &eldenring::cs::ChrIns) -> Option<&'static str> {
    use fromsoftware_shared::FromStatic;
    if data_no_dead(chr) {
        return Some("its no-death switch (data module + 0x19B bit 0: the events' immortality)");
    }
    if chr.modules.action_flag.action_modifiers_flags.set_immortality() {
        return Some("its animation's immortality (TimeAct SET_IMMORTALITY)");
    }
    // SAFETY: game thread; the game's own param repository.
    let Ok(repo) = (unsafe { eldenring::cs::SoloParamRepository::instance() }) else { return None };
    chr.special_effect.entries().take(128).any(|e| repo.get::<eldenring::cs::SpEffectParam>(e.param_id as u32).is_some_and(|r| r.no_dead())).then_some("a special effect of it with SpEffectParam noDead")
}

/// (ZETTA 2026-10-23) `CSChrDataModule`'s flag byte (private in the SDK): `recoverable_hp_time` (0x168) + 4 + 4 + 0x28 + 3.
pub const DATA_FLAGS: usize = 0x19B;
const _: () = assert!(std::mem::offset_of!(eldenring::cs::CSChrDataModule, recoverable_hp_time) == 0x168);
const _: () = assert!(std::mem::offset_of!(eldenring::cs::CSChrDataModule, character_behavior_name) == DATA_FLAGS + 1 + 0x8c);

/// (ZETTA 2026-10-23) The flag byte of the character's data module (for the log too).
pub fn data_flags(chr: &eldenring::cs::ChrIns) -> u8 {
    let data: &eldenring::cs::CSChrDataModule = &chr.modules.data;
    // SAFETY: one byte inside the live data module (offset checked against the SDK's neighbours at compile time).
    unsafe { std::ptr::read_volatile((data as *const eldenring::cs::CSChrDataModule as *const u8).add(DATA_FLAGS)) }
}

fn data_no_dead(chr: &eldenring::cs::ChrIns) -> bool {
    data_flags(chr) & 1 != 0
}

/// (ZETTA 2026-10-24, the user, 2026-10-24: "killing a boss with a second phase during it's first phase with Zettaflare should just kill the boss entirely skipping the second phase") THE SHAPE OF A TWO-PHASE BOSS, from what the game holds at the hit (no per-boss table): its
/// final form already in the world as more rows of its own character (`Duplicates`: Malenia's c2120 rows), or not (the same
/// character changing state, or a later character the event makes: `SameCharacter`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Duplicates(usize),
    SameCharacter,
}

pub fn shape_of(final_rows: usize) -> Shape {
    if final_rows > 0 { Shape::Duplicates(final_rows) } else { Shape::SameCharacter }
}

/// (ZETTA 2026-10-24) THE FINAL FORM'S ROWS: every other living row of the body's own character (`rows`: id, character,
/// alive), wherever it stands (hidden or not): killed with it.
pub fn final_rows(body: u32, npc_id: i32, rows: &[(u32, i32, bool)]) -> Vec<u32> {
    rows.iter().filter(|r| r.0 != body && r.1 == npc_id && r.2).map(|r| r.0).collect()
}

/// (ZETTA 2026-10-24) One row of a whole kill, watched after its lethal hit: dead (`Done`); still alive after the bullet's
/// time (the game's floor held): its HP written 0 (`WriteHp`, once); still alive after that: the game cannot be made to
/// finish it (`FallBack`: the old route, its own phase change).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    Wait,
    WriteHp,
    Done,
    FallBack,
}

pub const WHOLE_BULLET_S: f32 = 1.5;
pub const WHOLE_WRITE_S: f32 = 3.0;

pub fn whole_watch(alive: bool, since: f32, written: bool) -> Finish {
    if !alive {
        Finish::Done
    } else if !written && since >= WHOLE_BULLET_S {
        Finish::WriteHp
    } else if written && since >= WHOLE_WRITE_S {
        Finish::FallBack
    } else {
        Finish::Wait
    }
}

/// (ZETTA 2026-10-24) The game's no-death states taken off the character (the data module's switch, its TimeAct's
/// immortality: written again every frame of the watch, a clip may set it again). Game thread.
pub fn clear_no_death(chr: &mut eldenring::cs::ChrIns) {
    let data: &eldenring::cs::CSChrDataModule = &chr.modules.data;
    let at = (data as *const eldenring::cs::CSChrDataModule as *mut u8).wrapping_add(DATA_FLAGS);
    // SAFETY: one byte inside the live data module (offset checked at compile time); game thread.
    unsafe {
        let b = std::ptr::read_volatile(at);
        if b & 1 != 0 {
            std::ptr::write_volatile(at, b & !1);
        }
    }
    if chr.modules.action_flag.action_modifiers_flags.set_immortality() {
        chr.modules.action_flag.as_mut().action_modifiers_flags.set_set_immortality(false);
    }
}

/// (ZETTA 2026-10-23) THE HAND-BACK'S DEATH WATCHED: a body Zettaflare killed takes its owed HP while hidden; the game
/// either kills it (it stays hidden for good) or keeps it alive (a no-death state the hit did not see: a phase change, the
/// game's own: shown again). `since` s after the hand-back, `alive` / `hp` as the game has it now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeathWatch {
    /// Still waiting for the game's word.
    Wait,
    /// Dead: hidden for good.
    Died,
    /// Alive past the wait: a phase change (shown again).
    Survived,
}

/// How long (s) the hand-back's damage may take to land and kill (its bullet lands in a frame or few).
pub const DEATH_WAIT_S: f32 = 1.5;

pub fn death_watch(alive: bool, since: f32) -> DeathWatch {
    if !alive {
        DeathWatch::Died
    } else if since >= DEATH_WAIT_S {
        DeathWatch::Survived
    } else {
        DeathWatch::Wait
    }
}

impl Book {
    /// A hit of Zettaflare's on `id`: `amount` Elden Ring HP owed.
    pub fn add(&mut self, id: u32, key: u64, amount: i32) {
        let e = self.owed.entry(id).or_insert((key, 0));
        e.0 = key;
        e.1 = e.1.saturating_add(amount.max(0));
    }

    /// The HP owed by `id` (0: none).
    pub fn owed(&self, id: u32) -> i32 {
        self.owed.get(&id).map_or(0, |o| o.1)
    }

    /// Is anything owed?
    pub fn any(&self) -> bool {
        !self.owed.is_empty()
    }

    /// KH3's side of the body's HP: the game's less what is owed (0: killed by Zettaflare, still alive in the game).
    pub fn kh_hp(game_hp: i32, owed: i32) -> i32 {
        (game_hp - owed.max(0)).max(0)
    }

    /// (2026-10-23) A body whose death would be a phase change ([`phase_change`]): told at the hit.
    pub fn set_phase(&mut self, id: u32) {
        self.phase.insert(id);
    }

    /// (ZETTA 2026-10-23) Was `id`'s death told a phase change at the hit?
    pub fn is_phase(&self, id: u32) -> bool {
        self.phase.contains(&id)
    }

    /// KH3's side of `id`'s HP: [`Book::kh_hp`], at least 1 for a body whose death would be a phase change (a survivor).
    pub fn kh_hp_of(&self, id: u32, game_hp: i32) -> i32 {
        let hp = Self::kh_hp(game_hp, self.owed(id));
        if self.phase.contains(&id) && game_hp > 0 { hp.max(1) } else { hp }
    }

    /// A body Zettaflare has killed on KH3's side that the game still holds alive (never a phase change's body).
    pub fn killed(&self, id: u32, game_hp: i32) -> bool {
        let o = self.owed(id);
        o > 0 && game_hp > 0 && self.kh_hp_of(id, game_hp) == 0
    }

    /// The hand-back: everything owed, to be taken now (id, handle bits, HP); the book empties.
    pub fn due(&mut self) -> Vec<(u32, u64, i32)> {
        self.phase.clear();
        let mut v: Vec<(u32, u64, i32)> = self.owed.drain().map(|(id, (k, hp))| (id, k, hp)).collect();
        v.sort_by_key(|x| x.0);
        v
    }

    /// A body gone from the game (despawned / unloaded) owes nothing.
    pub fn forget(&mut self, id: u32) {
        self.owed.remove(&id);
        self.phase.remove(&id);
    }
}

/// WHETHER ZETTAFLARE OWNS THE PRESENTATION NOW, from kh-party's word (`zettaflare::presentation`: its runs, and its step
/// counter): owned while a run is up and its party is stepped; a party not stepped for `STALE_FRAMES` frames (hidden by a
/// cutscene of the game's, a load, Sora stopped) owns nothing (its sequence cannot end by itself there).
#[derive(Default, Debug, Clone, Copy)]
pub struct Owner {
    last_step: u32,
    still: u32,
}

/// Frames without a party step after which its word no longer holds.
pub const STALE_FRAMES: u32 = 30;

impl Owner {
    /// Once a frame: kh-party's word (`running`, its step counter) -> owned now.
    pub fn frame(&mut self, running: bool, step: u32) -> bool {
        if step != self.last_step {
            self.last_step = step;
            self.still = 0;
        } else {
            self.still = self.still.saturating_add(1);
        }
        running && self.still < STALE_FRAMES
    }
}

/// (ZETTA 2026-10-23) IS THIS A HIT ZETTAFLARE'S PRESENTATION OWNS: its forced hits (`HIT_FLAG_FORCE`, Zettaflare's only).
pub fn owned_hit(flags: u32) -> bool {
    flags & kh_core::host::HIT_FLAG_FORCE != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zdefer_a_kill_is_owed_until_the_hand_back_then_taken_in_full() {
        // the user's Malenia (phase 1: 958 HP left) hit by a level-1 Zettaflare (9940): KH3's side sees her dead, the game
        // still holds her alive (no death, no phase change mid-sequence); at the hand-back the whole number is taken
        let mut b = Book::default();
        b.add(4, 77, 9940);
        assert_eq!(Book::kh_hp(958, b.owed(4)), 0);
        assert!(b.killed(4, 958), "dead on KH3's side, alive in the game");
        // a survivor (a level-99 boss with 30000 HP): owed, not killed
        b.add(5, 78, 20000);
        assert!(!b.killed(5, 30000) && Book::kh_hp(30000, b.owed(5)) == 10000);
        // the throw's hit adds to it
        b.add(5, 78, 99);
        assert_eq!(b.owed(5), 20099);
        let due = b.due();
        assert_eq!(due, vec![(4, 77, 9940), (5, 78, 20099)]);
        assert!(!b.any());
    }

    #[test]
    /// (the user, 2026-10-24: "killing a boss with a second phase during it's first phase with Zettaflare should just kill the boss entirely skipping the second phase") the shapes, modelled: Malenia (her final form's rows already in the world: all killed with her), a boss changing
    /// state on its own body and one whose final form a later event makes (no rows: the body alone, the game's death watched,
    /// its floor written past, the old route only when nothing will do)
    fn zdefer_a_two_phase_boss_is_killed_whole() {
        // Malenia: phase-1 body 6 (c2120), her rows 7, 8, 9 (c2120, hidden), a dead one, another character
        let rows = [(6, 2120, true), (7, 2120, true), (8, 2120, true), (9, 2120, true), (10, 2120, false), (11, 4311, true)];
        let fin = final_rows(6, 2120, &rows);
        assert_eq!(fin, vec![7, 8, 9]);
        assert_eq!(shape_of(fin.len()), Shape::Duplicates(3));
        // Radahn-like (its own body changes) / Radagon -> Elden Beast (a later character): nothing else in the world
        assert_eq!(final_rows(1, 4710, &[(1, 4710, true), (2, 4311, true)]), Vec::<u32>::new());
        assert_eq!(shape_of(0), Shape::SameCharacter);
        // the watch: dead at once; floored: written 0 once; still alive: the old route
        assert_eq!(whole_watch(false, 0.2, false), Finish::Done);
        assert_eq!(whole_watch(true, 0.2, false), Finish::Wait);
        assert_eq!(whole_watch(true, WHOLE_BULLET_S, false), Finish::WriteHp);
        assert_eq!(whole_watch(true, WHOLE_BULLET_S + 0.1, true), Finish::Wait);
        assert_eq!(whole_watch(true, WHOLE_WRITE_S, true), Finish::FallBack);
        assert_eq!(whole_watch(false, WHOLE_WRITE_S, true), Finish::Done);
        // a whole kill is a kill on KH3's side (no survivor's floor)
        let mut b = Book::default();
        b.add(6, 1, 30000);
        assert_eq!(b.kh_hp_of(6, 14340), 0, "killed: the frozen hurt, the silhouette, the crumble, hidden for good");
    }

    #[test]
    fn zdefer_the_hand_backs_death_is_watched() {
        assert_eq!(death_watch(true, 0.1), DeathWatch::Wait);
        assert_eq!(death_watch(false, 0.1), DeathWatch::Died);
        assert_eq!(death_watch(true, DEATH_WAIT_S), DeathWatch::Survived, "floored by the game: its phase change");
    }

    #[test]
    fn zdefer_a_phase_change_death_is_a_survivor_a_mortal_one_a_kill() {
        // (2026-10-23) the user: "mod detects the second phase and just throws them back": phase-1 Malenia (the game's no-death
        // state on her, her phase-2 rows on her spot) hit for more than her HP: KH3's side keeps her alive (the survivor's
        // path: held, thrown in the wide shot); a mortal body is a kill; the debt is taken at the hand-back either way
        assert!(phase_change(true, false) && phase_change(false, true) && !phase_change(false, false));
        let mut b = Book::default();
        b.add(4, 7, 9940);
        b.set_phase(4);
        assert_eq!(b.kh_hp_of(4, 958), 1, "a survivor on KH3's side");
        assert!(!b.killed(4, 958), "no crumble for a phase-1 boss");
        b.add(5, 8, 9940);
        assert_eq!(b.kh_hp_of(5, 958), 0);
        assert!(b.killed(5, 958), "a mortal one: the kill (its crumble)");
        assert_eq!(b.due(), vec![(4, 7, 9940), (5, 8, 9940)], "both owed in full at the hand-back (the game floors hers: its phase change)");
        assert_eq!(b.kh_hp_of(4, 958), 958, "after the hand-back nothing is owed");
    }

    #[test]
    fn zdefer_the_presentation_is_owned_only_while_its_party_steps() {
        let mut o = Owner::default();
        assert!(o.frame(true, 1));
        assert!(o.frame(true, 2));
        // the party hidden by a cutscene of the game's: its run cannot end there; after STALE_FRAMES it owns nothing
        let mut owned = true;
        for _ in 0..STALE_FRAMES + 1 {
            owned = o.frame(true, 2);
        }
        assert!(!owned, "a party not stepped owns nothing: what it owes is taken");
        assert!(!o.frame(false, 3), "the sequence over");
        assert!(owned_hit(kh_core::host::HIT_FLAG_FORCE) && !owned_hit(0));
    }
}
