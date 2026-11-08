//! (NOCOMBAT) WHERE ELDEN RING FORBIDS THE TARNISHED TO ATTACK, THE KH CHARACTERS CANNOT EITHER.
//!
//! THE USER (2026-11-03): "grey out all action commands except for item in the roundtable hold, you can't do anything
//! offensive there"; "this applies to all characters"; "you can't summon in round table hold either"; "whenever the
//! tarnished wouldn't be able to attack in Elden Ring is when offensive commands are disabled for KH characters".
//!
//! THE GAME'S OWN STATE (read from its files, 2026-11-03, Elden Ring 2.7.2.0, build/sora_tmp/weapons/NOCOMBAT/STATE.md):
//! - the Roundtable Hold's event script (m11_10_00_00.emevd, event 11102600, started at the map's load) puts special effect
//!   9621 on the player (`SetSpEffect(10000, 9621)`) while he is inside the Hold's area 11102600 and clears it when he
//!   leaves (the event restarts and clears it first);
//! - the same effect is put on him by the same kind of event in Rennala's chamber once she is defeated (m14_00_00_00 event
//!   14003500: area 14002700, flag 14000800) and by the common event 90005615 in four more places (m12_05 area 12052699
//!   after flag 12050800; m20_01 area 20019253; m21_00 area 21002690 after flag 21009230; m22_00 area 22002699);
//! - SpEffectParam 9621 (regulation.bin) does nothing else: no stat, no rate, permanent while on, its `stateInfo` (the
//!   game's "state change type", read by the engine itself) is 457, and 9621 is the ONLY row with that type. The
//!   player's behaviour script (action/script/c0000.hks) never names 9621 or 457: the refusal of the attack inputs is the
//!   engine's, keyed on that state type (as EquipParamGoods / Magic `isUseNoAttackRegion` say which items and spells it
//!   still lets through).
//!
//! So the rule is ON exactly while one of the player's special effects is of state type 457 ([`NO_ATTACK_STATE`]),
//! whichever effect and whichever place gives it: the Hold, Rennala's chamber, any other place the game marks so. No map
//! test. Read once a frame ([`frame`]) from the player's special effects and the game's own param table; the answer is
//! handed to the one rule every KH character asks (kh_core::nocombat).
//!
//! NOT READ: the engine's own attack-input gate in eldenring.exe (the function that drops the R1 / R2 request) was not
//! disassembled; what it keys on is the data above.

/// SpEffectParam `stateInfo` of the game's "no attacking here" effect (row 9621, the only one of this type).
pub const NO_ATTACK_STATE: u16 = 457;

/// THE GAME-STATE READER: the `stateInfo` of each special effect on the player now (mocked by the tests).
pub trait PlayerEffects {
    /// (effect id, its state type); None when the player or the param table is not readable this frame.
    fn effects(&mut self) -> Option<Vec<(i32, u16)>>;
}

/// The rule's answer from the reader: Some((on, the effect that forbids)) or None (nothing readable: the last answer stands).
pub fn forbids(reader: &mut dyn PlayerEffects) -> Option<(bool, Option<i32>)> {
    let effects = reader.effects()?;
    let by = effects.iter().find(|(_, state)| *state == NO_ATTACK_STATE).map(|(id, _)| *id);
    Some((by.is_some(), by))
}

/// The answer kept between frames (the log's line on a change).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Watch {
    pub on: bool,
    pub by: Option<i32>,
}

impl Watch {
    /// One frame: the new answer (`enabled` false: `nocombat.on = 0`, the rule is never on). Returns the line to log on a change.
    pub fn step(&mut self, reader: &mut dyn PlayerEffects, enabled: bool) -> Option<String> {
        let (on, by) = if enabled { forbids(reader).unwrap_or((self.on, self.by)) } else { (false, None) };
        let changed = on != self.on;
        (self.on, self.by) = (on, by);
        changed.then(|| match by {
            Some(id) => format!("nocombat: Elden Ring forbids attacks here (special effect {id}, state type {NO_ATTACK_STATE}): every KH character's offensive commands are off, Items stay"),
            None if enabled => "nocombat: attacks are allowed again: the KH characters' commands are back".to_string(),
            None => "nocombat: nocombat.on = 0: the rule is off".to_string(),
        })
    }
}

/// (SYNTH-MOOGLE audit) A reader of the game: the player's (effect id, state type) through the param cache (None: unreadable).
#[cfg(any(windows, test))]
type ReadFn<'a> = dyn FnMut(&mut std::collections::HashMap<i32, u16>) -> Option<Vec<(i32, u16)>> + 'a;

/// The player's special effects and their state types through a reader of the game (`read`, with the param cache).
#[cfg(any(windows, test))]
struct Reader<'a> {
    types: &'a mut std::collections::HashMap<i32, u16>,
    read: &'a mut ReadFn<'a>,
}

#[cfg(any(windows, test))]
impl PlayerEffects for Reader<'_> {
    fn effects(&mut self) -> Option<Vec<(i32, u16)>> {
        (self.read)(self.types)
    }
}

/// (SYNTH-MOOGLE audit, 2026-11-07) THE ONE WATCH OF THE PROCESS and its param cache. It was `thread_local!`, but sora.rs
/// runs its frame on the game's worker threads: the DEPLOY3 log (er/logs/kh-er.log) shows each change logged six times
/// within a few seconds ("forbids attacks here" at 1009.250, .281, .525, 1010.125, 1011.656, 1017.518), one per thread's
/// watch, and a thread whose frame could not read the game handed its own stale answer to kh_core::nocombat. The lock is
/// held only over the step (memory reads of the player's effects and the param table: no game function is called, no
/// hook of ours runs inside, the render thread never takes it); the log line is written after it is let go.
#[cfg(any(windows, test))]
static SHARED: std::sync::Mutex<Option<(Watch, std::collections::HashMap<i32, u16>)>> = std::sync::Mutex::new(None);

/// One frame of the shared watch: (the answer, the line to log on a change).
#[cfg(any(windows, test))]
fn shared_step(enabled: bool, read: &mut ReadFn<'_>) -> (bool, Option<String>) {
    let mut guard = SHARED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (watch, types) = guard.get_or_insert_with(Default::default);
    let line = watch.step(&mut Reader { types, read }, enabled);
    (watch.on, line)
}

#[cfg(windows)]
mod live {
    use std::collections::HashMap;

    use eldenring::cs::{SoloParamRepository, SpEffectParam};
    use fromsoftware_shared::FromStatic;

    use crate::control::Control;
    use crate::erlog;

    /// The player's special effects and their state types from the game's param table (cached per effect id).
    fn game_effects(types: &mut HashMap<i32, u16>) -> Option<Vec<(i32, u16)>> {
        let player = crate::game::player_mut()?;
        let ids: Vec<i32> = player.chr_ins.special_effect.entries().take(128).map(|e| e.param_id).collect();
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let state = match types.get(&id) {
                Some(s) => *s,
                None => {
                    // SAFETY: the singleton lives as long as the game; the SDK checks its holder table.
                    let repo = unsafe { SoloParamRepository::instance() }.ok()?;
                    let s = u32::try_from(id).ok().and_then(|i| repo.get::<SpEffectParam>(i)).map_or(0, |row| row.state_info());
                    types.insert(id, s);
                    s
                }
            };
            out.push((id, state));
        }
        Some(out)
    }

    /// ONCE A FRAME (sora.rs, before any KH character steps): the game's state -> kh_core::nocombat.
    pub fn frame(control: &Control) {
        let enabled = control.flag("nocombat.on") != Some(false);
        let (on, line) = super::shared_step(enabled, &mut game_effects);
        if let Some(line) = line {
            erlog!("{line}");
        }
        kh_core::nocombat::set_zone(on);
    }
}

#[cfg(windows)]
pub use live::frame;

#[cfg(test)]
mod tests {
    use super::*;

    /// A mocked game: the effects on the player, or nothing readable.
    struct Mock(Option<Vec<(i32, u16)>>);

    impl PlayerEffects for Mock {
        fn effects(&mut self) -> Option<Vec<(i32, u16)>> {
            self.0.clone()
        }
    }

    #[test]
    fn nocombat_reader_on_exactly_while_an_effect_of_the_no_attack_type_is_on() {
        let mut w = Watch::default();
        assert_eq!(w.step(&mut Mock(Some(vec![(4300, 0), (100380, 12)])), true), None, "ordinary effects: off, no line");
        assert!(!w.on);
        let line = w.step(&mut Mock(Some(vec![(4300, 0), (9621, NO_ATTACK_STATE)])), true);
        assert!(w.on && w.by == Some(9621) && line.is_some_and(|l| l.contains("9621")), "the Hold's effect: on");
        // any effect of that type, not only 9621 (the rule is the game's state, not a list of places)
        assert!(forbids(&mut Mock(Some(vec![(1234567, NO_ATTACK_STATE)]))) == Some((true, Some(1234567))));
        // a frame the game cannot be read keeps the answer
        assert_eq!(w.step(&mut Mock(None), true), None);
        assert!(w.on);
        let line = w.step(&mut Mock(Some(vec![(4300, 0)])), true);
        assert!(!w.on && line.is_some(), "the effect cleared (he left): off again");
    }

    #[test]
    fn nocombat_reader_switched_off_by_the_control_key() {
        let mut w = Watch::default();
        w.step(&mut Mock(Some(vec![(9621, NO_ATTACK_STATE)])), true);
        assert!(w.on);
        w.step(&mut Mock(Some(vec![(9621, NO_ATTACK_STATE)])), false);
        assert!(!w.on, "nocombat.on = 0: never on");
    }

    /// (SYNTH-MOOGLE audit) ONE WATCH WHATEVER THREAD RUNS THE FRAME: six threads take turns at the frame (as the game's
    /// workers do) while the player walks into the Hold and out again: each change is logged once, and a frame whose reader
    /// sees nothing keeps the one shared answer (a thread's own watch kept its own, stale one).
    #[test]
    fn nocombat_one_watch_for_every_game_thread() {
        let lines = std::sync::Mutex::new(Vec::new());
        let answers = std::sync::Mutex::new(Vec::new());
        // frames 0..60 outside, 60..120 in the Hold, 120..180 outside; every 7th frame unreadable
        for round in 0..30usize {
            std::thread::scope(|sc| {
                for th in 0..6usize {
                    let k = round * 6 + th;
                    let (lines, answers) = (&lines, &answers);
                    sc.spawn(move || {
                        let in_hold = (60..120).contains(&k);
                        let mut read = |_: &mut std::collections::HashMap<i32, u16>| (k % 7 != 3).then(|| if in_hold { vec![(4300, 0), (9621, NO_ATTACK_STATE)] } else { vec![(4300, 0)] });
                        let (on, line) = shared_step(true, &mut read);
                        lines.lock().unwrap().extend(line);
                        answers.lock().unwrap().push((k, on));
                    })
                    .join()
                    .unwrap();
                }
            });
        }
        let lines = lines.into_inner().unwrap();
        assert_eq!(lines.len(), 2, "each change logged once: {lines:?}");
        assert!(lines[0].contains("forbids") && lines[1].contains("allowed again"));
        for (k, on) in answers.into_inner().unwrap() {
            let want = if k % 7 == 3 { (61..121).contains(&k) } else { (60..120).contains(&k) };
            assert_eq!(on, want, "frame {k}");
        }
    }
}
