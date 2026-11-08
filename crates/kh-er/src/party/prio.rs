//! (GF-THREAT, R15) THE GAME'S OWN TARGET SEARCH: `SpEffectParam.targetPriority`, A BORROWED ROW PER MEMBER.
//!
//! READ FROM eldenring.exe 2.7.1.0 (a copy, iced-x86; the same code in 2.7.2.0: the getter at file offset 0x4f7550, the score's tail
//! at 0x3397fb). Every enemy keeps a list of target records and goes for the record with the highest score (0x140340f60):
//!
//! ```text
//! score (0x14033a090) = ((forced ? 10000 : 0) + max(0, 1 - distance / 100 m))
//!                       x (1 + hit_timer / 10 x targetSysDmgEffectRate / 100)     a hit sets hit_timer = 10 s (0x140339600)
//!                       x two other factors (0x140344eb0, 0x140344db0)            not read
//!                       x (1 + sound_timer / 8 x 0.1)
//!                       + targetPriority                                          the sum over the TARGET's live effects
//!                       + sticky                                                  5 from an AI call (0x14033bbe0), else 0
//! ```
//!
//! - THE FIELD DOES WHAT ITS NAME SAYS. `SpecialEffect::target_priority` (0x1404f7e70) adds `[row + 0xF8]` over the live entries
//!   (entry flags & 0x800C0003 = 0); `CSChrInsTargetAccessorBase` slot 29 (+0xE8) calls it on the candidate's character, and the
//!   score adds it after the multipliers. So it is on the BODY an enemy looks at, for every enemy at once: it cannot name an
//!   enemy. The director's per-enemy choices stay with hate hits.
//! - A HIT'S AMOUNT DOES NOT COUNT: any hit (1 point or 1000) sets the same 10-s clock, x2 falling to x1 at rate 100. That is why a
//!   1-point hate hit works, why it never beats a sticky +5 or a forced target, and why it does nothing on the 99 think rows
//!   with rate 0. Scores near 1 are the rule, so a priority of 0.3 tips a close call, and 12 beats all but a forced target.
//! - A SCORE OVER 100 IS NOT SCORED AGAIN (0x140339f45), so a priority must stay under that.
//! - The game's own rows use it the same way: 8204 (+1.5, 20 s) differs from a blank row only in endurance and priority; others
//!   are 8440 and 3380 (+10), 13925 (+0.5), 5150 (-1).
//!
//! THE ROWS: nothing is added to the game's tables. Three rows that are blank and alike are borrowed (the first from
//! `aggro.prio_row`), in memory only; each gets an endurance and, live, its member's value (the game reads the row each time it
//! scores). A row that lapses on its own: while a member wants a value it is re-applied when it is gone, so a mod that stops
//! leaves nothing behind. Hate hits stay: this is the route where they fail, and it spends no bullet. `party.prio = 0`: off (to
//! try the hate hits alone; `party.taunt = 0` tries this alone).

use kh_host::party::PartyChara;

use super::aggro::rules::Sheet;

/// One row per member.
pub const ROWS: usize = 3;

/// A member's row in the pool.
pub fn slot(c: PartyChara) -> usize {
    match c {
        PartyChara::Sora => 0,
        PartyChara::Donald => 1,
        PartyChara::Goofy => 2,
    }
}

/// What the value of a member's body depends on.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PrioIn {
    /// His Taunt runs with an enemy in its reach.
    pub taunting: bool,
    /// He is a Goofy (the base bias of the party's shield).
    pub goofy: bool,
    /// At Resolve 2+.
    pub strong: bool,
    /// In Zettaflare's sequence.
    pub zetta: bool,
}

/// THE RULE: the value on a member's body now (0: no effect on it).
pub fn value(s: &Sheet, i: PrioIn) -> f32 {
    if i.zetta {
        s.prio_zetta
    } else if i.taunting {
        s.prio_taunt
    } else if i.goofy && i.strong {
        s.prio_resolve
    } else if i.goofy {
        s.prio_base
    } else {
        0.0
    }
}

/// THE BORROW: `n` rows from `rows` (id, bytes, its fields say it does nothing; in id order), the first such row at or after `start`
/// and those byte-identical to it. Fewer than `n`: none.
pub fn pick_rows<'a>(rows: impl Iterator<Item = (u32, &'a [u8], bool)>, start: u32, n: usize) -> Vec<u32> {
    let mut first: Option<&[u8]> = None;
    let mut out = Vec::new();
    for (id, bytes, quiet) in rows {
        if id < start || !quiet {
            continue;
        }
        if *first.get_or_insert(bytes) == bytes {
            out.push(id);
            if out.len() == n {
                return out;
            }
        }
    }
    Vec::new()
}

/// A hate hit is worth this share of its usual rate while the member's priority is on his body (the bullet budget).
pub const BACKUP_SLOWER: f32 = 3.0;

#[cfg(windows)]
pub use live::{frame, live_on, off, Want};

#[cfg(windows)]
mod live {
    use super::*;
    use crate::erlog;
    use eldenring::cs::{ChrIns, ChrInsExt, FieldInsHandle, SoloParamRepository, SpEffectParam};
    use eldenring::param::SP_EFFECT_PARAM_ST;
    use fromsoftware_shared::FromStatic;
    use std::sync::Mutex;

    /// What the game's getter 0x1404f7e70 skips an entry for.
    const ENTRY_FLAGS_AT: usize = 0x60;
    const ENTRY_OFF: u32 = 0x800C_0003;

    /// One member's want this frame.
    #[derive(Clone, Copy, Debug)]
    pub struct Want {
        pub chara: PartyChara,
        /// Its body (`None`: no live body: nothing to put it on).
        pub body: Option<FieldInsHandle>,
        pub value: f32,
    }

    #[derive(Default)]
    struct State {
        rows: Vec<u32>,
        tried: bool,
        /// Member: the value last written to his row, the game's own sum last read, the body it is on (bits).
        written: [f32; ROWS],
        sums: [f32; ROWS],
        told_at: [f32; ROWS],
        clock: f32,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);

    fn repo() -> Option<&'static mut SoloParamRepository> {
        // SAFETY: game thread; the singleton lives as long as the game; the SDK checks its holder table.
        unsafe { SoloParamRepository::instance_mut() }.ok()
    }

    fn bytes(row: &SP_EFFECT_PARAM_ST) -> &[u8] {
        // SAFETY: the row is plain data of this size.
        unsafe { std::slice::from_raw_parts(row as *const SP_EFFECT_PARAM_ST as *const u8, size_of::<SP_EFFECT_PARAM_ST>()) }
    }

    fn quiet(r: &SP_EFFECT_PARAM_ST) -> bool {
        r.icon_id() == -1 && r.effect_endurance() == 0.0 && r.target_priority() == 0.0 && r.state_info() == 0 && r.replace_sp_effect_id() <= 0 && r.cycle_occurrence_sp_effect_id() <= 0 && r.vfx_id() <= 0 && r.behavior_id() <= 0
    }

    fn borrow(st: &mut State, s: &Sheet) {
        st.tried = true;
        let Some(repo) = repo() else {
            erlog!("aggro: targetPriority: the param repository is not there: hate hits only");
            return;
        };
        let rows = pick_rows(repo.rows::<SpEffectParam>().map(|(id, r)| (id, bytes(r), quiet(r))), s.prio_row, ROWS);
        if rows.len() < ROWS {
            erlog!("aggro: targetPriority: no {ROWS} blank alike rows from {}: hate hits only", s.prio_row);
            return;
        }
        for id in &rows {
            if let Some(r) = repo.get_mut::<SpEffectParam>(*id) {
                r.set_effect_endurance(s.prio_time);
            }
        }
        erlog!("aggro: targetPriority: rows {rows:?} borrowed (Sora, Donald, Goofy), {:.1} s each, in memory only: the game adds a row's value to that body's score in every enemy's own target search", s.prio_time);
        st.rows = rows;
    }

    /// What the game's own getter answers for this body, and the flags of our entry (`None`: it is not on the body).
    fn read(chr: &ChrIns, row: u32) -> (f32, Option<u32>) {
        let mut sum = 0.0;
        let mut ours = None;
        for e in chr.special_effect.entries().take(128) {
            // SAFETY: inside the live entry (the SDK's `unk50`).
            let flags = unsafe { *(e as *const _ as *const u8).add(ENTRY_FLAGS_AT).cast::<u32>() };
            if e.param_id == row as i32 {
                ours = Some(flags);
            }
            if flags & ENTRY_OFF == 0 {
                // SAFETY: the game's own row pointer of a live entry.
                sum += e.param_data.map_or(0.0, |p| unsafe { p.as_ref() }.target_priority());
            }
        }
        (sum, ours)
    }

    /// ONCE A FRAME (aggro.rs): every member's value on his body. `dt`: seconds of this frame.
    pub fn frame(s: &Sheet, wants: &[Want], dt: f32) {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let st = guard.get_or_insert_with(State::default);
        st.clock += dt.max(0.0);
        if !st.tried {
            borrow(st, s);
        }
        if st.rows.len() < ROWS {
            return;
        }
        for w in wants {
            let k = slot(w.chara);
            let row = st.rows[k];
            if st.written[k] != w.value {
                if let Some(r) = repo().and_then(|repo| repo.get_mut::<SpEffectParam>(row)) {
                    r.set_target_priority(w.value);
                }
                st.written[k] = w.value;
                st.told_at[k] = -1.0e6;
            }
            if w.value == 0.0 && st.sums[k] == 0.0 {
                continue; // nothing wanted, nothing there: no walk over the characters
            }
            let Some(chr) = w.body.and_then(|h| crate::targets::find_chr(&h)) else { continue };
            let (sum, on) = read(chr, row);
            st.sums[k] = sum;
            match (w.value != 0.0, on.is_some()) {
                (true, false) => chr.apply_speffect(row as i32, true),
                (false, true) => chr.remove_speffect(row as i32),
                _ => {}
            }
            if w.value != 0.0 && st.clock - st.told_at[k] >= 10.0 {
                st.told_at[k] = st.clock;
                erlog!("aggro: targetPriority: {}'s body wants {:.2}: row {row} {}, the game's own sum on the body {sum:.2}{}", w.chara.name(), w.value, on.map_or("just applied".to_string(), |f| format!("is on it (entry flags {f:#x})")), if on.is_some_and(|f| f & ENTRY_OFF != 0) { " (FLAGGED OFF: the game skips it)" } else { "" });
            }
        }
    }

    /// The member's value is on his body by the game's own sum (his hate hits may slow down).
    pub fn live_on(c: PartyChara, at_least: f32) -> bool {
        STATE.try_lock().ok().and_then(|g| g.as_ref().map(|st| st.rows.len() == ROWS && st.sums[slot(c)] >= at_least)).unwrap_or(false)
    }

    /// The party stopped: the rows are blank again (the effects lapse on their own).
    pub fn off() {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(st) = guard.as_mut() else { return };
        for (k, row) in st.rows.iter().enumerate() {
            if st.written[k] != 0.0 {
                if let Some(r) = repo().and_then(|repo| repo.get_mut::<SpEffectParam>(*row)) {
                    r.set_target_priority(0.0);
                }
                st.written[k] = 0.0;
            }
        }
        st.sums = [0.0; ROWS];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_value_of_a_body_follows_its_state() {
        let s = Sheet::default();
        assert_eq!(value(&s, PrioIn::default()), 0.0, "a friend with nothing on");
        assert_eq!(value(&s, PrioIn { goofy: true, ..Default::default() }), s.prio_base);
        assert_eq!(value(&s, PrioIn { goofy: true, strong: true, ..Default::default() }), s.prio_resolve);
        assert_eq!(value(&s, PrioIn { goofy: true, strong: true, taunting: true, ..Default::default() }), s.prio_taunt);
        assert_eq!(value(&s, PrioIn { zetta: true, ..Default::default() }), s.prio_zetta);
    }

    /// The numbers against the score read from the exe: the taunt beats a sticky +5 and stays under the 100 where a record stops being
    /// scored; the base bias does not undo a director's hate hit 4 s old (rate 100: x1.6 on a score near 1).
    #[test]
    fn the_shipped_values_fit_the_games_score() {
        let s = Sheet::default();
        assert!(s.prio_taunt > 5.0 + 2.0 && s.prio_taunt < 100.0);
        let held = 1.6 * 0.98;
        let goofy_near = 0.95;
        assert!(goofy_near + s.prio_resolve < held && goofy_near + s.prio_base < held, "a held choice survives until its refresh");
        assert!(goofy_near + s.prio_resolve > 1.0, "but tips an even choice");
        assert!(s.prio_zetta < -2.0);
    }

    #[test]
    fn the_borrow_takes_alike_blank_rows_from_the_start() {
        let blank = [1u8, 2, 3];
        let other = [1u8, 2, 4];
        let rows = [(10, &blank, true), (20, &other, false), (21, &blank, false), (22, &blank, true), (23, &other, true), (24, &blank, true), (25, &blank, true), (26, &blank, true)];
        let it = || rows.iter().map(|(id, b, q)| (*id, &b[..], *q));
        assert_eq!(pick_rows(it(), 21, 3), vec![22, 24, 25], "the one row it does nothing with is skipped, the odd one too");
        assert_eq!(pick_rows(it(), 25, 3), Vec::<u32>::new(), "not enough: none");
    }
}
