//! (INTERACT-MARK, the user 2026-11-05: "Add this green effect from KH3 onto what the interact prompt is focused on (people,
//! etc)") KH3'S REACTION-TARGET MARKER ON WHATEVER THE Y INTERACTION IS ABOUT. State note:
//! build/sora_tmp/weapons/INTERACT-MARK/STATE.md.
//!
//! WHAT KH3 DOES (READ in the decomp and the assets):
//! - The marker is the HUD's type-5 ("action") target cursor: `HudTarget` clip `ActLockonMark` (pool of 2), whose child
//!   `triAct` is the green triangle; `ATresPlayerControllerBase` 0x1413F7410 (the "CheckReactor" search) adds it on the
//!   chosen reaction command's `UTresReactorComponent` (`0x1415759C0(cockpit, reactor, 5, false, -1)`), unless the
//!   reactor's `m_bDisableTargetMarker` (vt+0xBA0 = 0x140F28760; set by no asset of the game) is on, and removes it at once
//!   (`0x141421CA0` -> `0x14157A7A0(.., 5, ..)` -> `0x14166C730`: removeChild, no out animation) when the target changes
//!   or goes. One at a time (the controller's weak pointer +0xC60).
//! - WHERE: the projection pass puts it at the reactor's vt+0xB88 (0x140ACF250: its ComponentToWorld translation). The
//!   reactor is authored ON the thing: every NPC's at the mesh's `center` socket (c_npc blueprints: AttachToName "center",
//!   a few cm off), a chest's 0 / 30 cm above its root, a door's 110 - 120 cm, a save point's 40 cm under its root, the
//!   Moogle's (g_ex_mogli_01) 30 cm above his.
//!
//! THE RULE HERE (no per-object table): the marker follows the FIRST Y interaction (the green event button, erprompt.rs
//! `reactions` order) and sits:
//! - a CHARACTER (Elden Ring's talk / kind-1 region element, Donald at the Finger Reader): its body's middle, feet + half its
//!   capsule height (KH3's `center` socket is the capsule's middle: c_ca001's capsule centre 103 cm, half height 101);
//! - an OBJECT / a POSITION (Elden Ring's kind-3 / kind-0 region element: doors, levers, chests, items, ladders, graces):
//!   the point the game itself anchors its action region at: the element's dummy poly (ActionButtonParam `dummy_poly1` plus
//!   the caller's offset, through the game's own 0xA666B0), else the element's position;
//! - the Moogle: KH3's own reactor height (30 cm over his root).
//! - (the user 2026-11-05: "save points should not have the green marker") NEVER ON A SAVE POINT, by the target's KIND:
//!   the Save command's target is a save point ([`TargetKind::SavePoint`]) and Elden Ring's grace (its own character, the
//!   site of grace's c1000, the kind every grace's talk element is) is one too ([`TargetKind::Grace`]): [`marked`] is false.
//!
//! WHAT ELDEN RING DOES (READ in both executables, build/sora_tmp/weapons/INTERACT-MARK/tools; not seen in the game):
//! every offerer asks `CSActionButtonRegionSystemImp` first ([`REGION_TEST_FN`] R1, (sys, actorElement, targetElement, row,
//! key, dmyOffset, f32) -> bool, true only for the key that won its category last frame; [`REGION_IN_FN`] R3, (sys, actor,
//! target, row) -> bool), then `execute_action_button` with the same row. The target element (`CSActionButtonRegionElement`)
//! is +0x10 kind (0 position, 1 character, 3 field instance), +0x20 position (HavokPosition), +0x40 `ChrIns*` (kind 1). The
//! region system's step [`REGION_STEP_FN`] (sys, frameTime*) ends in the manager's update (tail jump to 0xA64850), which
//! picks the shown node (manager +0x20, node +0x08 its row, +0x2A shown).
//!
//! WHAT WE DO: R1 and R3 are detoured: after the original, a `true` is recorded (row, key, kind, the anchor, the body's middle
//! for a character, read right there while the game holds the element). The step is detoured: after the original (so the
//! manager has just picked), the shown node's row is matched with this frame's records and the result published; the frame
//! task (hud.rs) reads only that copy. Keys: `prompt.mark` (1; 0 = no marker).

use std::sync::Mutex;

use glam::Vec3;

/// What the marker is put on this frame, in Elden Ring's physics space (metres, Y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ErTarget {
    /// ActionButtonParam row of the shown prompt.
    pub row: i32,
    /// The region element's kind (0 position, 1 character, 3 field instance).
    pub kind: u32,
    /// The offerer's key (its character / object; changes when the target does).
    pub key: u64,
    /// Where the marker goes.
    pub at: Vec3,
    /// What kind of thing it is (a grace gets no marker).
    pub what: TargetKind,
}

/// One region test the game answered `true` this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub row: i32,
    pub key: u64,
    pub kind: u32,
    /// The game's anchor of the region (dummy poly, else the element's position).
    pub anchor: Vec3,
    /// (kind 1) the character's feet, capsule height (m) and character id.
    pub body: Option<(Vec3, f32, i32)>,
    /// The actor's (the player's) distance to the anchor, m.
    pub dist: f32,
    pub what: TargetKind,
}

/// The kind of thing a Y interaction is about, for the save-point rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    /// A character, an object, an item, a position.
    Thing,
    /// KH3's save point (the Save command).
    SavePoint,
    /// Elden Ring's site of grace (its own character c1000: the save point's grace).
    Grace,
}

/// The grace's own character (savepoint.rs / targets.rs: the invisible helper every site of grace has).
pub const GRACE_NPC_ID: i32 = 1000;

/// (pure) The kind of an Elden Ring region target: a character whose NpcParam character id is the grace's is a grace.
pub fn kind_of(element_kind: u32, npc_id: Option<i32>) -> TargetKind {
    if element_kind == 1 && npc_id == Some(GRACE_NPC_ID) {
        TargetKind::Grace
    } else {
        TargetKind::Thing
    }
}

/// (pure) THE USER'S RULE: no marker on a save point (ours or the game's grace).
pub fn marked(what: TargetKind) -> bool {
    what == TargetKind::Thing
}

/// KH3's reactor heights the rule uses (cm, READ in the blueprints). (Our own talks are the party build's.)
#[cfg(any(feature = "party", test))]
pub const MOOGLE_REACTOR_CM: f32 = 30.0; // g_ex_mogli_01 TresReactor0 RelativeLocation z

/// (pure) THE PLACEMENT RULE for an Elden Ring target: a character's body middle (KH3's `center` socket), else the game's
/// own anchor of the region.
pub fn marker_point(s: &Seen) -> Vec3 {
    match s.body {
        Some((feet, height, _)) if s.kind == 1 && feet.is_finite() && height.is_finite() && height > 0.0 => feet + Vec3::Y * (height * 0.5),
        _ => s.anchor,
    }
}

/// (pure) The record the shown row stands for: one per category wins in the game (R1); for R3's tests the nearest.
pub fn pick(seen: &[Seen], row: i32) -> Option<&Seen> {
    seen.iter().filter(|s| s.row == row).min_by(|a, b| a.dist.total_cmp(&b.dist))
}

/// What the first Y interaction is about (erprompt.rs `reactions` order).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum First {
    Save,
    PartyTalk,
    ErPrompt,
}

/// (pure) Which source the marker follows: the first offered of Save, a party talk, Elden Ring's prompt.
pub fn first(save: bool, party_talks: usize, er_prompt: bool) -> Option<First> {
    if save {
        Some(First::Save)
    } else if party_talks > 0 {
        Some(First::PartyTalk)
    } else if er_prompt {
        Some(First::ErPrompt)
    } else {
        None
    }
}

/// Our own talk owners' marker points this frame (KH space, cm, Z up), set by their hosts (moogle.rs, unlock/host.rs).
static KH_POINTS: Mutex<Vec<(&'static str, Vec3)>> = Mutex::new(Vec::new());

/// (moogle.rs, unlock/host.rs, once a frame) where an `offer_talk` owner's marker goes (None: nothing of it to mark).
#[cfg(any(feature = "party", test))]
pub fn set_talk_point(owner: &'static str, at_kh: Option<Vec3>) {
    let mut g = KH_POINTS.lock().unwrap_or_else(|e| e.into_inner());
    g.retain(|(o, _)| *o != owner);
    if let Some(p) = at_kh.filter(|p| p.is_finite()) {
        g.push((owner, p));
    }
}

fn talk_point(owner: &str) -> Option<Vec3> {
    KH_POINTS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(o, _)| *o == owner).map(|(_, p)| *p)
}

/// (EMOTE-PROMPTS) The gesture command's character this frame: (key, Elden Ring physics point), gestureprompt.rs's.
static GESTURE_POINT: Mutex<Option<(u64, Vec3)>> = Mutex::new(None);

/// (EMOTE-PROMPTS, gestureprompt.rs once a frame) where the gesture command's marker goes: the character a radius row is
/// about (Patches, Dryleaf Dane); None for a region row (nothing to mark) or no gesture command.
pub fn set_gesture_point(at_er: Option<(u64, Vec3)>) {
    *GESTURE_POINT.lock().unwrap_or_else(|e| e.into_inner()) = at_er.filter(|(_, p)| p.is_finite());
}

/// The published Elden Ring target (the step's copy).
static SHOWN: Mutex<Option<ErTarget>> = Mutex::new(None);

pub fn er_target() -> Option<ErTarget> {
    *SHOWN.lock().unwrap_or_else(|e| e.into_inner())
}

/// A stable key of our own targets.
fn owner_key(owner: &str) -> u64 {
    owner.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)) | 0x10
}

/// (hud.rs, once a frame) THE MARKER'S TARGET for the HUD: (key, KH-space point), from the first Y interaction.
/// `pos_er_to_kh`: this frame's space change; `talk_owners`: CV3's offer order.
pub fn target(save: bool, talk_owners: &[&'static str], er_prompt: bool, pos_er_to_kh: impl Fn(Vec3) -> Vec3) -> Option<(u64, Vec3)> {
    let Some(first) = first(save, talk_owners.len(), er_prompt) else {
        // (EMOTE-PROMPTS) the gesture command comes after all of them in Y's order
        return GESTURE_POINT.lock().unwrap_or_else(|e| e.into_inner()).map(|(k, p)| (k, pos_er_to_kh(p)));
    };
    match first {
        // (the user 2026-11-05) the Save command is about a save point: no marker
        First::Save => marked(TargetKind::SavePoint).then_some((0, Vec3::ZERO)),
        First::PartyTalk => {
            let owner = talk_owners.first()?;
            talk_point(owner).map(|p| (owner_key(owner), p))
        }
        First::ErPrompt => er_target().filter(|t| marked(t.what)).map(|t| (t.key, pos_er_to_kh(t.at))),
    }
}

static LAST_LOGGED: Mutex<Option<u64>> = Mutex::new(None);

/// (hud.rs) The marker's target as the HUD gets it is logged when it changes (`reactmark:` lines).
pub fn log_change(mark: Option<(u64, Vec3)>, first_name: Option<&str>) {
    let mut last = LAST_LOGGED.lock().unwrap_or_else(|e| e.into_inner());
    let key = mark.map(|m| m.0);
    if *last == key {
        return;
    }
    *last = key;
    match mark {
        Some((k, p)) => {
            let er = er_target().filter(|t| t.key == k).map(|t| format!("; Elden Ring's row {} element kind {} at ({:.2}, {:.2}, {:.2})", t.row, t.kind, t.at.x, t.at.y, t.at.z)).unwrap_or_default();
            crate::erlog!("reactmark: (INTERACT-MARK) KH3's marker ON \"{}\" (key {k:#x}) at KH ({:.0}, {:.0}, {:.0}){er}", first_name.unwrap_or("?"), p.x, p.y, p.z);
        }
        None => crate::erlog!("reactmark: (INTERACT-MARK) no marker{}", first_name.map(|n| format!(" (the first Y interaction is \"{n}\": a save point or nothing to mark)")).unwrap_or_default()),
    }
}

#[cfg(windows)]
pub(crate) use game::{install, uninstalled};
#[cfg(all(windows, test))]
pub(crate) use game::LOCATED;

#[cfg(windows)]
mod game {
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    use std::sync::Mutex;

    use glam::Vec3;

    use super::{pick, Seen, SHOWN};
    use crate::erlog;
    use crate::havok::{locate, Located};

    /// R1: the region test every offerer asks (talk, items, objects, bloodstains, signs; 18 call sites).
    pub(crate) const REGION_TEST_FN: Located = Located {
        name: "action-button region test (CSActionButtonRegionSystemImp)",
        pattern: "48 8b c4 57 41 56 41 57 48 83 ec 70 48 c7 40 c0 fe ff ff ff 48 89 58 08 48 89 68 10 48 89 70 18 4d 8b f0 48 8b ea 48 8b d9 40 32 ff 4c 8d 3d ? ? ? ? 4c 89 78 c8 83 ce ff 89 70 d0 48 c7 40 d8 00 00 00 00",
        capture: 0,
        rva_2_7_1_0: 0x00a6_7160,
        rva_2_7_2_0: 0x00a6_7620,
    };
    /// R3: the plain in-region test (two characters at 0x3F9630, Sos signs at 0x5B1BC0).
    pub(crate) const REGION_IN_FN: Located = Located {
        name: "action-button in-region test",
        pattern: "48 8b c4 41 54 41 56 41 57 48 83 ec 70 48 c7 40 c0 fe ff ff ff 48 89 58 08 48 89 68 10 48 89 70 18 48 89 78 20 49 8b e8 48 8b f2 4c 8b f1 40 32 ff 4c 8d 25 ? ? ? ? 4c 89 60 c8 83 cb ff 89 58 d0",
        capture: 0,
        rva_2_7_1_0: 0x00a6_6cc0,
        rva_2_7_2_0: 0x00a6_7180,
    };
    /// S: the region system's frame step (resets the categories, then the manager's update; its one caller 0x58F254).
    pub(crate) const REGION_STEP_FN: Located = Located {
        name: "action-button region step (then the manager's update)",
        pattern: "40 53 48 83 ec 20 48 83 c1 18 48 8b da 4c 8d 81 38 04 00 00 49 3b c8 74 25 48 83 c1 08 33 d2 90 48 8b 01 48 89 41 08 c7 41 f8 ff ff 7f 7f",
        capture: 0,
        rva_2_7_1_0: 0x00a6_7560,
        rva_2_7_2_0: 0x00a6_7a20,
    };
    /// The element's dummy-poly point (element, dummy poly id, out position, out direction) -> found; the region tests'
    /// own call (0xA67B54: id = the row's `dummy_poly1` + the caller's offset, skipped for -1).
    pub(crate) const DUMMY_POINT_FN: Located = Located {
        name: "action-button element dummy-poly point",
        pattern: "40 55 53 56 57 41 56 48 8d 6c 24 c9 48 81 ec d0 00 00 00 48 c7 45 d7 fe ff ff ff 0f 29 b4 24 c0 00 00 00 49 8b f9 49 8b d8 44 8b f2 48 8b f1 8b 51 10 85 d2 0f 84 ? ? ? ? 83 ea 01 0f 84",
        capture: 0,
        rva_2_7_1_0: 0x00a6_66b0,
        rva_2_7_2_0: 0x00a6_6b70,
    };
    #[cfg(test)]
    pub(crate) const LOCATED: [Located; 4] = [REGION_TEST_FN, REGION_IN_FN, REGION_STEP_FN, DUMMY_POINT_FN];

    /// `CSActionButtonRegionElement` (READ: 0xA66610 / 0xA66400 / 0xA66500 build it; 0xA67AC0 reads +0x20 / +0x30).
    const ELEM_KIND: usize = 0x10;
    const ELEM_POS: usize = 0x20;
    const ELEM_CHR: usize = 0x40;
    /// `CSActionButtonManImp`: +0x20 the shown node (its +0x08 row), +0x2A shown (READ: 0xA65AB0, 0xA64E16).
    const MAN_SHOWN_NODE: usize = 0x20;
    const MAN_SHOWN: usize = 0x2A;
    const NODE_ROW: usize = 0x08;

    type TestFn = unsafe extern "C" fn(*mut u8, *const u8, *const u8, i32, u64, i32, f32) -> bool;
    type InFn = unsafe extern "C" fn(*mut u8, *const u8, *const u8, i32) -> bool;
    type StepFn = unsafe extern "C" fn(*mut u8, *const u8);
    type DummyFn = unsafe extern "C" fn(*const u8, i32, *mut [f32; 4], *mut [f32; 4]) -> bool;

    static TEST_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
    static IN_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
    static STEP_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
    static DUMMY: AtomicUsize = AtomicUsize::new(0);
    /// This frame's `true` answers (cleared by the step after the match).
    static CURRENT: Mutex<Vec<Seen>> = Mutex::new(Vec::new());
    /// Shown rows with no record, told once each (player-local prompts have no target).
    static UNTOLD: Mutex<Vec<i32>> = Mutex::new(Vec::new());
    static LAST_ROW: AtomicU32 = AtomicU32::new(u32::MAX);

    #[repr(C, align(16))]
    struct V4([f32; 4]);

    fn v3(p: &[f32; 4]) -> Vec3 {
        Vec3::new(p[0], p[1], p[2])
    }

    /// Reads what the marker needs from the target element the game just answered for (nothing here can panic).
    ///
    /// # Safety
    /// `elem` / `actor` are the region test's live elements (or null), on the game's thread inside its own call.
    unsafe fn record(actor: *const u8, elem: *const u8, row: i32, key: u64, dmy_offset: Option<i32>) {
        if !crate::game::plausible(elem as usize) {
            return;
        }
        let kind = std::ptr::read_unaligned(elem.add(ELEM_KIND) as *const u32);
        let pos = std::ptr::read_unaligned(elem.add(ELEM_POS) as *const [f32; 4]);
        let mut anchor = v3(&pos);
        // the game's own anchor: the row's dummy poly + the caller's offset (R1's rule; R3 has none)
        let dummy = DUMMY.load(Ordering::Relaxed);
        if let (Some(offset), true) = (dmy_offset, dummy != 0) {
            let dmy = dummy_poly_of(row);
            if dmy != -1 {
                let (mut at, mut dir) = (V4([0.0; 4]), V4([0.0; 4]));
                let f: DummyFn = std::mem::transmute(dummy);
                if f(elem, dmy.wrapping_add(offset), &mut at.0, &mut dir.0) && v3(&at.0).is_finite() {
                    anchor = v3(&at.0);
                }
            }
        }
        let body = if kind == 1 {
            let chr = std::ptr::read_unaligned(elem.add(ELEM_CHR) as *const usize);
            body_of(chr, v3(&pos))
        } else {
            None
        };
        let dist = if crate::game::plausible(actor as usize) { (v3(&std::ptr::read_unaligned(actor.add(ELEM_POS) as *const [f32; 4])) - anchor).length() } else { 0.0 };
        if !anchor.is_finite() {
            return;
        }
        let mut g = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
        if g.len() < 64 {
            g.push(Seen { row, key, kind, anchor, body, dist, what: super::kind_of(kind, body.map(|b| b.2)) });
        }
    }

    /// The row's `dummy_poly1` (-1: none, or no param).
    fn dummy_poly_of(row: i32) -> i32 {
        use eldenring::cs::{ActionButtonParam, SoloParamRepository};
        use fromsoftware_shared::FromStatic;
        if row < 0 {
            return -1;
        }
        // SAFETY: the game's param repository, read only (the region test read the same row a moment ago).
        unsafe { SoloParamRepository::instance() }.ok().and_then(|repo| repo.get::<ActionButtonParam>(row as u32).map(|r| r.dummy_poly1())).unwrap_or(-1)
    }

    /// A character's feet (the element's position: its physics position, or its mount's), capsule height (m), character id.
    ///
    /// # Safety
    /// `chr` is the element's `ChrIns*` (or 0), live while the game's region test runs.
    unsafe fn body_of(chr: usize, feet: Vec3) -> Option<(Vec3, f32, i32)> {
        if !crate::game::plausible(chr) {
            return None;
        }
        let c = &*(chr as *const eldenring::cs::ChrIns);
        if !crate::game::chr_complete(c) {
            return None;
        }
        let h = c.modules.physics.hit_height;
        (h.is_finite() && h > 0.05 && h < 50.0).then_some((feet, h, c.npc_id))
    }

    unsafe extern "C" fn test_hook(sys: *mut u8, actor: *const u8, target: *const u8, row: i32, key: u64, dmy_offset: i32, f: f32) -> bool {
        let original: TestFn = std::mem::transmute(TEST_ORIGINAL.load(Ordering::Relaxed));
        let yes = original(sys, actor, target, row, key, dmy_offset, f);
        if yes {
            record(actor, target, row, key, Some(dmy_offset));
        }
        yes
    }

    unsafe extern "C" fn in_hook(sys: *mut u8, actor: *const u8, target: *const u8, row: i32) -> bool {
        let original: InFn = std::mem::transmute(IN_ORIGINAL.load(Ordering::Relaxed));
        let yes = original(sys, actor, target, row);
        if yes {
            let key = if crate::game::plausible(target as usize) { std::ptr::read_unaligned(target.add(ELEM_CHR) as *const u64) } else { 0 };
            record(actor, target, row, if key != 0 { key } else { target as u64 }, None);
        }
        yes
    }

    unsafe extern "C" fn step_hook(sys: *mut u8, time: *const u8) {
        let original: StepFn = std::mem::transmute(STEP_ORIGINAL.load(Ordering::Relaxed));
        original(sys, time);
        // the manager has just picked what it shows
        let row = shown_row();
        let mut current = CURRENT.lock().unwrap_or_else(|e| e.into_inner());
        let found = row.and_then(|r| pick(&current, r).copied());
        current.clear();
        drop(current);
        let target = found.map(|s| super::ErTarget { row: s.row, kind: s.kind, key: s.key, at: super::marker_point(&s), what: s.what });
        *SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = target;
        let r = row.unwrap_or(-1);
        if LAST_ROW.swap(r as u32, Ordering::Relaxed) != r as u32 && r >= 0 && target.is_none() {
            let mut told = UNTOLD.lock().unwrap_or_else(|e| e.into_inner());
            if !told.contains(&r) && told.len() < 64 {
                told.push(r);
                erlog!("reactmark: (INTERACT-MARK) Elden Ring shows ActionButtonParam row {r} with no region target this frame: no marker for it");
            }
        }
    }

    /// The shown node's row (None: nothing shown).
    unsafe fn shown_row() -> Option<i32> {
        use fromsoftware_shared::FromStatic;
        let man = eldenring::cs::CSActionButtonMan::instance().ok()?;
        let base = man as *const eldenring::cs::CSActionButtonMan as *const u8;
        if *base.add(MAN_SHOWN) == 0 {
            return None;
        }
        let node = std::ptr::read_unaligned(base.add(MAN_SHOWN_NODE) as *const usize);
        crate::game::plausible(node).then(|| std::ptr::read_unaligned((node + NODE_ROW) as *const i32))
    }

    /// Finds the functions and detours the two tests and the step (once per payload). False: no Elden Ring target is known
    /// (the marker then shows only on our own talks and save points).
    pub(crate) fn install() -> bool {
        let (test, inside, step, dummy) = (locate(&REGION_TEST_FN), locate(&REGION_IN_FN), locate(&REGION_STEP_FN), locate(&DUMMY_POINT_FN));
        if let Some(d) = dummy {
            DUMMY.store(d, Ordering::SeqCst);
        }
        let hook = |at: Option<usize>, detour: usize, original: &'static AtomicUsize, what: &'static str| -> bool {
            match at {
                Some(f) if original.load(Ordering::SeqCst) == 0 => {
                    // SAFETY: the function's start, found by its unique pattern at the recorded address; the detour has its signature.
                    match unsafe { kh_er_render::inline_hook(f, detour, original, what) } {
                        Ok(()) => true,
                        Err(e) => {
                            original.store(0, Ordering::SeqCst);
                            erlog!("reactmark: {what} was NOT detoured ({e})");
                            false
                        }
                    }
                }
                Some(_) => true,
                None => false,
            }
        };
        // the step first: a test record is only of use when the step matches it
        let s = hook(step, step_hook as *const () as usize, &STEP_ORIGINAL, "action-button region step");
        let t = s && hook(test, test_hook as *const () as usize, &TEST_ORIGINAL, "action-button region test");
        let i = s && hook(inside, in_hook as *const () as usize, &IN_ORIGINAL, "action-button in-region test");
        erlog!(
            "reactmark: (INTERACT-MARK) region step {}, region test {}, in-region test {}, dummy-poly point {}: {}",
            if s { "detoured" } else { "NOT FOUND" },
            if t { "detoured" } else { "NOT FOUND" },
            if i { "detoured" } else { "NOT FOUND" },
            if dummy.is_some() { "found" } else { "NOT FOUND (element positions)" },
            if s && (t || i) { "KH3's marker follows Elden Ring's prompt" } else { "NO MARKER ON ELDEN RING'S PROMPTS (our own talks and save points keep theirs)" }
        );
        s && (t || i)
    }

    /// The hooks are gone (the renderer's shutdown removed every inline hook).
    pub(crate) fn uninstalled() {
        for a in [&TEST_ORIGINAL, &IN_ORIGINAL, &STEP_ORIGINAL] {
            a.store(0, Ordering::SeqCst);
        }
        CURRENT.lock().unwrap_or_else(|e| e.into_inner()).clear();
        *SHOWN.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(row: i32, kind: u32, anchor: Vec3, body: Option<(Vec3, f32)>, dist: f32) -> Seen {
        let body = body.map(|(f, h)| (f, h, 3000));
        Seen { row, key: 0x1_0000_0000 + row as u64, kind, anchor, body, dist, what: kind_of(kind, body.map(|b| b.2)) }
    }

    /// The placement rule: a character's body middle (KH3's `center` socket), an object's / a position's game anchor.
    #[test]
    fn interact_mark_placement_rule() {
        // an NPC 1.8 m tall standing at (10, 2, -5): the middle of his capsule
        let npc = seen(6000, 1, Vec3::new(10.0, 2.0, -5.0), Some((Vec3::new(10.0, 2.0, -5.0), 1.8)), 1.5);
        assert_eq!(marker_point(&npc), Vec3::new(10.0, 2.9, -5.0));
        // a door: the game's dummy-poly anchor as it is (no body)
        let door = seen(9000, 3, Vec3::new(1.0, 1.1, 0.0), None, 2.0);
        assert_eq!(marker_point(&door), Vec3::new(1.0, 1.1, 0.0));
        // an item on the ground (position-only / field): the anchor on the ground
        let item = seen(4000, 0, Vec3::new(3.0, 0.0, 3.0), None, 1.0);
        assert_eq!(marker_point(&item), Vec3::new(3.0, 0.0, 3.0));
        // a character whose capsule could not be read: its anchor
        let blind = seen(6000, 1, Vec3::new(0.0, 5.0, 0.0), None, 1.0);
        assert_eq!(marker_point(&blind), Vec3::new(0.0, 5.0, 0.0));
        // a body on a kind that is not a character is ignored; a nonsense height too
        assert_eq!(marker_point(&seen(1, 3, Vec3::ONE, Some((Vec3::ZERO, 2.0)), 0.0)), Vec3::ONE);
        assert_eq!(marker_point(&seen(1, 1, Vec3::ONE, Some((Vec3::ZERO, f32::NAN)), 0.0)), Vec3::ONE);
        // the size follows the target's own capsule: a 4 m giant gets it 2 m up
        let giant = seen(6000, 1, Vec3::ZERO, Some((Vec3::ZERO, 4.0)), 3.0);
        assert_eq!(marker_point(&giant), Vec3::new(0.0, 2.0, 0.0));
    }

    #[test]
    fn interact_mark_the_shown_row_picks_its_record() {
        let list = [seen(6000, 1, Vec3::X, None, 3.0), seen(4000, 0, Vec3::Y, None, 1.0), seen(6000, 1, Vec3::Z, None, 2.0)];
        assert_eq!(pick(&list, 6000).map(|s| s.anchor), Some(Vec3::Z), "the nearest of a row (R3 has no winner bookkeeping)");
        assert_eq!(pick(&list, 4000).map(|s| s.anchor), Some(Vec3::Y));
        assert_eq!(pick(&list, 1234), None, "a player-local prompt has no target");
    }

    /// (the user 2026-11-05: "save points should not have the green marker") by the target's KIND: the Save command's save
    /// point and Elden Ring's grace (its own character c1000) are never marked; every other thing is.
    #[test]
    fn interact_mark_no_marker_on_save_points_by_kind() {
        assert_eq!(kind_of(1, Some(GRACE_NPC_ID)), TargetKind::Grace);
        assert_eq!(kind_of(1, Some(3000)), TargetKind::Thing, "an NPC");
        assert_eq!(kind_of(3, None), TargetKind::Thing, "an object");
        assert_eq!(kind_of(0, None), TargetKind::Thing, "a position");
        assert_eq!(kind_of(3, Some(GRACE_NPC_ID)), TargetKind::Thing, "only a character element is a grace's character");
        assert!(!marked(TargetKind::SavePoint) && !marked(TargetKind::Grace) && marked(TargetKind::Thing));
        let to_kh = |p: Vec3| crate::coords::Frame::at(Vec3::ZERO).pos_er_to_kh(p);
        // the Save command first (he stands inside the save point): no marker, even with a party talk and a prompt behind it
        set_talk_point("save rule owner", Some(Vec3::ONE));
        assert_eq!(target(true, &["save rule owner"], true, to_kh), None);
        set_talk_point("save rule owner", None);
    }

    /// The marker follows the first Y interaction (Save, a party talk, Elden Ring's prompt), as the green bar does.
    #[test]
    fn interact_mark_follows_the_first_y_interaction() {
        assert_eq!(first(true, 1, true), Some(First::Save));
        assert_eq!(first(false, 2, true), Some(First::PartyTalk));
        assert_eq!(first(false, 0, true), Some(First::ErPrompt));
        assert_eq!(first(false, 0, false), None);
        let to_kh = |p: Vec3| crate::coords::Frame::at(Vec3::ZERO).pos_er_to_kh(p);
        // a party talk: its owner's point (the Moogle's: KH3's reactor 30 cm over his root), whoever leads
        set_talk_point("test owner", Some(Vec3::new(100.0, 0.0, MOOGLE_REACTOR_CM)));
        let (k, p) = target(false, &["test owner"], true, to_kh).expect("the owner's point");
        assert_eq!((k, p), (owner_key("test owner"), Vec3::new(100.0, 0.0, 30.0)));
        assert!(k & 0x10 != 0);
        set_talk_point("test owner", None);
        assert_eq!(target(false, &["test owner"], true, to_kh), None, "no point: no marker (never another target's)");
    }
}
