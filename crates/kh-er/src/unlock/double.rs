//! (ER party S8) THE STAGED BOSS WITH ITS OWN AI (the user, 2026-10-15, binding: "The boss should not be paused. Load the
//! boss and the character the boss is fighting at the same time and make them fight though both characters do 0 damage
//! to each other until the player enters the arena this is a universal rule for when a not yet obtained friend is
//! fighting a boss").
//!
//! (ER boss, 2026-10-15) NOW ONLY THE LOGGED FALLBACK, OFF BY DEFAULT: the real boss's AI switch was found in the exe
//! (EMEVD 2004[1] writes ChrCtrl + 0xE8 bit 5; crate::sdkx, aiswitch.rs, IMPL_BOSS_STATE.md) and the REAL boss is woken
//! by it. A double is made only when that write is refused AND `unlock.double_fallback = 1`.
//!
//! WHY A DOUBLE (the old reasoning, before the switch was found): the boss's own event keeps its AI off until the fight starts (m60_43_30_00 EVENT 1043302810 and m10
//! EVENT 10002860: SetCharacterAIState(boss, off) at load, on together with its boss bar only when the player is through
//! the fog). The switch that instruction writes is not in the pinned SDK (no AI field on ChrIns / ChrCtrl /
//! ChrManipulator), and the executable cannot be searched for it here (its strings are packed; no Ghidra in this
//! project). So the boss is MADE AGAIN beside itself by the game's own debug character creator (the route the ally bodies
//! take, S3) from ITS OWN rows: its model, its NpcParam and its NpcThinkParam (read from the map, checked against the live
//! boss). That character is the boss as the game builds it, with its own AI running from the first frame, and no event
//! holds it: no boss bar, no fog, no music. The event-held original is hidden meanwhile (not drawn, not hittable, no
//! capsule: it stands at its place with its AI off as the event wants). WHEN THE PLAYER IS IN (the game's fog flag / its
//! boss bar), the original is put exactly where the double stands, facing its way, and shown; the double is parked
//! (hidden, unhittable, its movement and actions off, below the arena). The event's own AI and boss bar then run the real
//! fight. A double is never despawned (the debug set has no remove): it is reused at the next staging of the session; a
//! load destroys it with the world.
//!
//! THE CREATOR IS SHARED WITH S3's POOL: a double is asked only while the pool has no request pending or adoptable
//! (`allybody::creator_free`), and the pool refuses to ask while a double is pending (`creator_busy`).

use std::sync::atomic::{AtomicBool, Ordering};

use eldenring::cs::{CSDebugChrCreator, ChrDebugSpawnRequest, ChrIns}; // (AGGRO) WorldChrMan: the creator is game.rs's now
// (AGGRO) FromStatic no longer needed here (the creator is game.rs's)
use glam::Vec3;

use crate::erlog;
use crate::game::{self, plausible};
use crate::word;

/// A double is pending at the creator (S3's pool waits).
static BUSY: AtomicBool = AtomicBool::new(false);

/// (allybody_live.rs `ask`) The creator is S8's now: S3's pool does not ask.
pub fn creator_busy() -> bool {
    BUSY.load(Ordering::Relaxed)
}

fn creator() -> Option<&'static mut CSDebugChrCreator> {
    // (AGGRO) at its place for the running version, vtable-checked (game.rs; the SDK's field is 2.7.1.0's place)
    crate::game::debug_chr_creator()
}

/// What the double is made from (the boss's own rows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rows {
    pub chr_id: i32,
    pub npc_param: i32,
    pub think: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    None,
    /// Asked of the creator: its last character before, when (s of the director's clock), where.
    Asked { before: usize, at: f32, pos: Vec3 },
    /// Made: the pointer and its handle bits; parked (hidden) or fighting.
    Live { ptr: usize, handle: u64, parked: bool },
    /// Not makeable this session (no creator, or three timeouts).
    Failed,
}

#[derive(Clone, Debug)]
pub struct Double {
    pub state: State,
    pub tries: u32,
    pub retry_at: f32,
    /// The original was hidden by us (to be shown again).
    pub original_hidden: bool,
    /// The HP given back to the double (the boss never loses HP before the player is in).
    pub kept_hp: i64,
}

impl Default for Double {
    fn default() -> Double {
        Double { state: State::None, tries: 0, retry_at: 0.0, original_hidden: false, kept_hp: 0 }
    }
}

/// The double's character this frame (found by its pointer and handle, complete).
fn valid(ptr: usize, handle: u64) -> Option<&'static mut ChrIns> {
    if !plausible(ptr) {
        return None;
    }
    // SAFETY: a character pointer we were handed by the creator; only followed while the handle at its second word and
    // completeness still hold (a freed object would not keep both).
    let chr = unsafe { &mut *(ptr as *mut ChrIns) };
    (game::handle_bits(&chr.field_ins_handle) == handle && game::chr_complete(chr)).then_some(chr)
}

impl Double {
    /// The double fighting now (made and not parked).
    pub fn fighting(&self) -> Option<&'static mut ChrIns> {
        match self.state {
            State::Live { ptr, handle, parked: false } => valid(ptr, handle),
            _ => None,
        }
    }

    /// THE STAGED STEP: make it (or take the parked one back) at the original's place; hide the original; hold the
    /// double's HP full. `now` = the director's clock (s). `who` = the scene for the log.
    pub fn stage(&mut self, original: &mut ChrIns, rows: Rows, now: f32, who: &str) {
        let at = game::v3(&original.modules.physics.position);
        let orient = original.modules.physics.orientation;
        match self.state {
            State::Failed => {}
            State::None => {
                if now < self.retry_at || !crate::allybody::creator_free() {
                    return;
                }
                let Some(c) = creator() else {
                    erlog!("unlock: {who}: the game has no debug character creator: the boss cannot fight before the player is in (it stands, AI off, as its event wants)");
                    self.state = State::Failed;
                    return;
                };
                if c.spawn {
                    return;
                }
                let before = word!(c.last_created_chr);
                // (the init block's scale and count, as S3's `ask`: a zero scale / count would make nothing)
                let init = std::ptr::addr_of_mut!(c.init_data) as usize;
                // SAFETY: inside the live creator's init block (the SDK's layout; S3 writes the same two fields).
                unsafe {
                    let scale = std::ptr::read_unaligned((init + 0x30) as *const [f32; 4]);
                    if scale[..3].iter().all(|v| *v == 0.0 || !v.is_finite()) {
                        std::ptr::write_unaligned((init + 0x30) as *mut [f32; 4], [1.0, 1.0, 1.0, 1.0]);
                    }
                    let count = std::ptr::read_unaligned((init + 0xec) as *const u32);
                    if count == 0 || count > 16 {
                        std::ptr::write_unaligned((init + 0xec) as *mut u32, 1);
                    }
                }
                // (AGGRO) the SDK's request writer on the version-aware creator (game.rs)
                let _ = crate::game::spawn_debug_character(&ChrDebugSpawnRequest { chr_id: rows.chr_id, chara_init_param_id: -1, npc_param_id: rows.npc_param, npc_think_param_id: rows.think, event_entity_id: 0, talk_id: 0, pos_x: at.x, pos_y: at.y + 0.05, pos_z: at.z });
                BUSY.store(true, Ordering::Relaxed);
                self.state = State::Asked { before, at: now, pos: at };
                erlog!("unlock: {who}: THE BOSS IS MADE AGAIN WITH ITS OWN AI (debug creator: c{:04}, NpcParam {}, think {}) at the original's place; the event-held original will be hidden", rows.chr_id, rows.npc_param, rows.think);
            }
            State::Asked { before, at: asked, pos } => {
                let last = creator().map_or(0, |c| word!(c.last_created_chr));
                if last != before && plausible(last) {
                    // SAFETY: the creator's newest character; checked complete and of the boss's rows before it is kept.
                    let chr = unsafe { &mut *(last as *mut ChrIns) };
                    if game::chr_complete(chr) && chr.npc_param_id == rows.npc_param && !crate::allybody::is_body(last) {
                        BUSY.store(false, Ordering::Relaxed);
                        let handle = game::handle_bits(&chr.field_ins_handle);
                        chr.modules.physics.as_mut().orientation = orient;
                        chr.modules.physics.as_mut().interpolated_orientation = orient;
                        self.state = State::Live { ptr: last, handle, parked: false };
                        self.tries = 0;
                        erlog!("unlock: {who}: BOSS AI ACTIVE: its double (handle {handle:#x}, team {}, {} / {} HP) stands at the original's place {:.1} s after the request; its own AI picks its target (the staged friend's body is the hostile beside it)", chr.team_type, chr.modules.data.hp, chr.modules.data.max_hp, now - asked);
                        return;
                    }
                }
                if now - asked > 3.0 {
                    BUSY.store(false, Ordering::Relaxed);
                    self.tries += 1;
                    self.retry_at = now + 5.0;
                    self.state = if self.tries >= 3 { State::Failed } else { State::None };
                    erlog!("unlock: {who}: the boss's double did not appear within 3 s at ({:.1}, {:.1}, {:.1}) (try {} of 3){}", pos.x, pos.y, pos.z, self.tries, if self.tries >= 3 { ": GIVEN UP this session (the boss stands, AI off, as its event wants)" } else { "" });
                }
            }
            State::Live { ptr, handle, parked } => {
                let Some(chr) = valid(ptr, handle) else {
                    erlog!("unlock: {who}: the boss's double is gone (the world was reloaded): made again");
                    self.state = State::None;
                    return;
                };
                if parked {
                    // back from its park: at the original's place, shown, its movement and actions given back
                    let p = chr.modules.physics.as_mut();
                    p.position = game::hp(at);
                    p.orientation = orient;
                    p.interpolated_orientation = orient;
                    show(chr, true);
                    self.state = State::Live { ptr, handle, parked: false };
                    erlog!("unlock: {who}: the boss's double is back at the original's place (reused)");
                }
                // the boss must not die or lose HP before the player is in
                let data = &mut chr.modules.data;
                if data.hp > 0 && data.hp < data.max_hp {
                    self.kept_hp += (data.max_hp - data.hp) as i64;
                    erlog!("unlock: {who}: damage suppressed on the boss: {} HP given back ({} / {})", data.max_hp - data.hp, data.max_hp, data.max_hp);
                    data.hp = data.max_hp;
                }
            }
        }
        // the event-held original: not drawn, not hit, no capsule (it stands at its place, its AI off)
        if matches!(self.state, State::Live { parked: false, .. }) {
            if !self.original_hidden {
                erlog!("unlock: {who}: the event-held original is hidden while its double fights");
            }
            show(original, false);
            self.original_hidden = true;
        }
    }

    /// The staging ends without a fight (the player went away, a load): the double is parked, the original shown.
    pub fn unstage(&mut self, original: Option<&mut ChrIns>, who: &str) {
        self.park(who);
        if let Some(o) = original {
            if self.original_hidden {
                show(o, true);
            }
        }
        self.original_hidden = false;
        if matches!(self.state, State::Asked { .. }) {
            BUSY.store(false, Ordering::Relaxed);
            self.state = State::None;
        }
    }

    /// THE PLAYER IS IN: the original takes the double's place and facing and is shown; the double is parked.
    pub fn hand_over(&mut self, original: &mut ChrIns, who: &str) {
        if let Some(d) = self.fighting() {
            let (pos, orient) = (d.modules.physics.position, d.modules.physics.orientation);
            let p = original.modules.physics.as_mut();
            p.position = pos;
            p.orientation = orient;
            p.interpolated_orientation = orient;
            erlog!("unlock: {who}: THE PLAYER IS IN: the event's boss takes its double's place ({:.1}, {:.1}, {:.1}) and is shown; the fight goes on with the game's own damage", game::v3(&pos).x, game::v3(&pos).y, game::v3(&pos).z);
        }
        show(original, true);
        self.original_hidden = false;
        self.park(who);
    }

    fn park(&mut self, who: &str) {
        if let State::Live { ptr, handle, parked: false } = self.state {
            if let Some(chr) = valid(ptr, handle) {
                show(chr, false);
                let p = chr.modules.physics.as_mut();
                let at = game::v3(&p.position);
                p.position = game::hp(at - Vec3::Y * 60.0);
                erlog!("unlock: {who}: the boss's double is parked (hidden, unhittable, still) below the arena");
            }
            self.state = State::Live { ptr, handle, parked: true };
        }
    }

    /// Every frame while parked: it stays parked (the game may reset the switches).
    pub fn hold_parked(&mut self) {
        if let State::Live { ptr, handle, parked: true } = self.state {
            if let Some(chr) = valid(ptr, handle) {
                show(chr, false);
            } else {
                self.state = State::None;
            }
        }
    }
}

/// Shown and fighting (true) or hidden, unhittable and still (false). The original is only hidden / shown: its AI and
/// movement are its event's.
fn show(chr: &mut ChrIns, on: bool) {
    chr.chr_flags1c5.set_enable_render(on);
    chr.chr_ctrl.flags.set_disable_hit(!on);
    chr.chr_ctrl.flags.set_disable_character_capsule_collision(!on);
    chr.debug_flags.set_disabled_movement(!on);
    chr.debug_flags.set_disabled_secondary_actions(!on);
    chr.chr_flags1c4.set_no_gravity(!on);
}
