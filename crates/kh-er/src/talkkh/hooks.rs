//! (CV2) THE GAME'S TALK FUNCTIONS, DETOURED (each found by pattern and used only at its 2.7.1.0 address; undone with
//! every other inline hook by the renderer's shutdown). UNTESTED IN THE GAME.
//!
//! | function (RVA)                         | what the detour does                                                      |
//! |----------------------------------------|---------------------------------------------------------------------------|
//! | talk command, `CSEzStateTalkEvent` slot 1 (0xea7100) | reads the command; a menu / dialog the bubble takes is NOT run (rules.rs `Decision::Swallow`) |
//! | talk query, `CSEzStateTalkEnv` slot 1 (0xea24a0)     | the game answers; while a choice is ours "is it open" / the result read ours |
//! | talk update, `CSTalkIns` (0xe9ba20)    | the game's skip byte set for the line A skipped, right before the game reads it, put back after; the line's state read after the update |
//! | talk subtitle (0xe9c780)               | not posted while the bubble shows the character's line                   |
//!
//! Every read of the game's objects here is of the object the game's own call hands us (its fields at the offsets the
//! decompiled functions use), on the game's thread; the shared state is `try_lock`ed (busy: the game's behaviour).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use super::rules::{Decision, LineSnap, Talks, Texts, Val};
use crate::erlog;
use crate::game::plausible;
use crate::havok::{locate, Located};

pub(crate) const EVENT_FN: Located = Located {
    name: "talk-script command (CSEzStateTalkEvent::invoke)",
    pattern: "48 8b c4 55 56 57 41 54 41 55 41 56 41 57 48 8d 68 b8 48 81 ec 10 01 00 00 48 c7 45 98 fe ff ff ff 48 89 58 08 0f 29 70 b8",
    capture: 0,
    rva_2_7_1_0: 0x00ea_7100, rva_2_7_2_0: 0x00ea_87f0 /* (PORT) */,
};
pub(crate) const ENV_FN: Located = Located {
    name: "talk-script query (CSEzStateTalkEnv::invoke)",
    pattern: "48 8b c4 55 56 57 41 54 41 55 41 56 41 57 48 8d a8 38 ff ff ff 48 81 ec 90 01 00 00 48 c7 45 80 fe ff ff ff 48 89 58 20",
    capture: 0,
    rva_2_7_1_0: 0x00ea_24a0, rva_2_7_2_0: 0x00ea_3b90 /* (PORT) */,
};
pub(crate) const UPDATE_FN: Located = Located {
    name: "talk line update (CSTalkIns)",
    pattern: "48 89 5c 24 10 56 48 83 ec 30 80 79 2d 00 48 8b f2 48 8b d9 0f 84 ? ? ? ? 0f b6 49 3b f3 0f 10 0d ? ? ? ? 84 c9",
    capture: 0,
    rva_2_7_1_0: 0x00e9_ba20, rva_2_7_2_0: 0x00e9_d110 /* (PORT) */,
};
pub(crate) const SUBTITLE_FN: Located = Located {
    name: "talk subtitle post",
    pattern: "40 55 48 8d 6c 24 b1 48 81 ec a0 00 00 00 48 c7 45 e7 fe ff ff ff 48 89 9c 24 c0 00 00 00 48 8b 05 ? ? ? ? 48 33 c4",
    capture: 0,
    rva_2_7_1_0: 0x00e9_c780, rva_2_7_2_0: 0x00e9_de70 /* (PORT) */,
};
/// In the update: `mov rax, [rip + G]` at + 0xbd, `cmp byte [rax + 0x92c0], 0` at + 0xc4 (READ in the copy).
pub(crate) const SKIP_LOAD_IN_FN: usize = 0xbd;
pub(crate) const SKIP_GLOBAL_RVA: usize = 0x0458_fc08;
/// (PORT) The same global in 2.7.2.0 (22 of 24 references carried over, all to this one; the load at + 0xbd is unchanged).
pub(crate) const SKIP_GLOBAL_RVA_2720: usize = 0x0459_2c28;
const SKIP_BYTE: usize = 0x92c0;

/// `CSTalkIns` fields (the update / line functions, read in the copy): talking, the block, the index in it, the
/// subtitle's TalkMsg id, the TalkToPlayer flag (the game's skip refused), the first update of a line, elapsed, length,
/// the character's handle.
pub(crate) const INS_TALKING: usize = 0x2d;
pub(crate) const INS_BLOCK: usize = 0x04;
pub(crate) const INS_INDEX: usize = 0x08;
pub(crate) const INS_MSG: usize = 0x34;
pub(crate) const INS_NO_SKIP: usize = 0x2f;
pub(crate) const INS_ELAPSED: usize = 0x54;
pub(crate) const INS_LENGTH: usize = 0x58;
pub(crate) const INS_HANDLE: usize = 0x40;
/// `CSEzStateTalkEvent`: the talk object at + 0x10, the talk id at + 0x18; `CSEzStateTalkEnv`: the talk id at + 0x10
/// (SDK layouts; the command function reads + 0x10 / + 0x18 so).
const EVENT_INS: usize = 0x10;
const EVENT_TALK: usize = 0x18;
const ENV_TALK: usize = 0x10;

static ORIG_EVENT: AtomicUsize = AtomicUsize::new(0);
static ORIG_ENV: AtomicUsize = AtomicUsize::new(0);
static ORIG_UPDATE: AtomicUsize = AtomicUsize::new(0);
static ORIG_SUBTITLE: AtomicUsize = AtomicUsize::new(0);
/// The address of the game's pointer to the object holding the skip byte (0: not found).
static SKIP_GLOBAL: AtomicUsize = AtomicUsize::new(0);
/// The talk object whose line A skips at its next update (0: none).
pub(crate) static SKIP_INS: AtomicUsize = AtomicUsize::new(0);
/// The talk object inside its update right now (the subtitle post is made from there).
static CUR_INS: AtomicUsize = AtomicUsize::new(0);
/// Character talks' subtitles are not posted (the bubble shows them, held or ambient: game.rs sets it while it can).
pub(crate) static SUPPRESS: AtomicBool = AtomicBool::new(false);
/// `talk.kh` on (the frame sets it; off: every detour passes everything).
pub(crate) static ENABLED: AtomicBool = AtomicBool::new(false);

/// The shared state: the talk scripts' (rules.rs) and the lines the updates left.
pub(crate) struct Shared {
    pub talks: Talks,
    /// (talk object, its line) as the latest update left it; `age` counted up by the frame.
    pub lines: Vec<LineSnap>,
    /// The command objects the pending choice came from (to give the command back): (talk object, command object).
    pub events: Vec<(usize, usize)>,
    /// (SHOP) the merchants' windows and the smiths' prompts (talkkh/shop.rs).
    pub shop: super::shop::ShopTalk,
}

pub(crate) static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

pub(crate) fn with_shared<R>(f: impl FnOnce(&mut Shared) -> R) -> Option<R> {
    let mut g = SHARED.try_lock().ok()?;
    let s = g.get_or_insert_with(|| Shared { talks: Talks::new(), lines: Vec::new(), events: Vec::new(), shop: Default::default() /* (SHOP) */ });
    Some(f(s))
}

type EventFn = unsafe extern "C" fn(*mut u8, *mut u8);
type EnvFn = unsafe extern "C" fn(*mut u8, *mut u8, *mut u8) -> *mut u8;
type UpdateFn = unsafe extern "C" fn(*mut u8, *mut f32);
type SubtitleFn = unsafe extern "C" fn(usize, usize, usize, usize, usize);

/// A value object of a command or query (16 bytes: the value, the type at + 8: 1 float, 2 int, 3 string).
unsafe fn value_at(p: *const u8) -> Val {
    if !plausible(p as usize & !7) {
        return Val::Other;
    }
    match std::ptr::read_unaligned(p.add(8) as *const i32) {
        1 => Val::Float(std::ptr::read_unaligned(p as *const f32)),
        2 => Val::Int(std::ptr::read_unaligned(p as *const i32)),
        _ => Val::Other,
    }
}

/// The commands the adapter reads (others pass without a look at their arguments).
fn wanted_command(id: i32) -> bool {
    use super::rules::cmd::*;
    matches!(id, SHOW_SHOP_MESSAGE | GENERIC_DIALOG | CLOSE_DIALOG | ADD_LIST | CLEAR_LIST | CLOSE_MENU | CHOICES | CAMERA | ADD_LIST_ALT)
}

/// (SHOP) The commands of the merchants' windows and of the reinforcement menu (sheet sora-er-shop `talk.commands`).
fn shop_wanted(id: i32) -> bool {
    let n = kh_host::shop::rules().talk;
    id == n.open_buy || id == n.open_sell || id == n.open_enhance
}

/// The game's texts and flags for the rules (game thread).
pub(crate) struct GameTexts;

impl Texts for GameTexts {
    fn flag(&self, id: u32) -> Option<bool> {
        use fromsoftware_shared::FromStatic;
        // SAFETY: game thread; the singleton lives as long as the game; the SDK reads its own flag blocks.
        let man = unsafe { eldenring::cs::CSEventFlagMan::instance() }.ok()?;
        if man.virtual_memory_flag.flag_blocks.is_null() {
            return None;
        }
        Some(man.virtual_memory_flag.get_flag(id))
    }
    fn option_text(&self, msg: i32) -> Option<String> {
        super::game::msg(&[33, 366], msg)
    }
    fn dialog_text(&self, msg: i32) -> Option<String> {
        super::game::msg(&[33, 366, 204, 372, 200, 368], msg)
    }
}

/// (nothing in here may panic: plain reads of the objects the game hands us, a try_lock, the original call)
unsafe extern "C" fn event_hook(this: *mut u8, ev: *mut u8) {
    let original: EventFn = std::mem::transmute(ORIG_EVENT.load(Ordering::Relaxed));
    if !ENABLED.load(Ordering::Relaxed) || !plausible(this as usize) || !plausible(ev as usize) {
        return original(this, ev);
    }
    let vt = std::ptr::read(ev as *const *const usize);
    if !plausible(vt as usize) {
        return original(this, ev);
    }
    let id_fn: unsafe extern "C" fn(*mut u8) -> i32 = std::mem::transmute(std::ptr::read(vt.add(2)));
    let id = id_fn(ev);
    if !wanted_command(id) && !shop_wanted(id) /* (SHOP) */ {
        return original(this, ev);
    }
    let count_fn: unsafe extern "C" fn(*mut u8) -> i32 = std::mem::transmute(std::ptr::read(vt.add(3)));
    let arg_fn: unsafe extern "C" fn(*mut u8, i32) -> *const u8 = std::mem::transmute(std::ptr::read(vt.add(4)));
    let n = count_fn(ev).clamp(0, 9);
    let args: Vec<Val> = (1..n).map(|k| value_at(arg_fn(ev, k))).collect();
    let talk_id = std::ptr::read_unaligned(this.add(EVENT_TALK) as *const i32);
    let ins = std::ptr::read_unaligned(this.add(EVENT_INS) as *const usize);
    let decision = with_shared(|s| {
        // (SHOP) a merchant's buy / sell window -> the KH shop; a smith's reinforcement menu never opens
        if let Some(d) = s.shop.on_event(talk_id, ins, id, &args) {
            return d;
        }
        let d = s.talks.on_event(talk_id, ins, this as usize, id, &args, &GameTexts);
        if d == Decision::Swallow {
            s.events.retain(|(i, _)| *i != ins);
            s.events.push((ins, this as usize));
        }
        d
    })
    .unwrap_or(Decision::Pass);
    if decision == Decision::Pass {
        original(this, ev);
    }
}

unsafe extern "C" fn env_hook(this: *mut u8, result: *mut u8, query: *mut u8) -> *mut u8 {
    let original: EnvFn = std::mem::transmute(ORIG_ENV.load(Ordering::Relaxed));
    // (SHOP) A REFUSED SMITH'S PROMPT (`npc.smith_talk = 0`): the game's query is not run (no prompt is put up), the
    // script reads 0. The query object's result is the call's hidden return value (16 bytes: the value, its type at + 8).
    if ENABLED.load(Ordering::Relaxed) && plausible(this as usize) && plausible(query as usize) && plausible(result as usize) {
        let vt = std::ptr::read(query as *const *const usize);
        if plausible(vt as usize) {
            let id_fn: unsafe extern "C" fn(*mut u8) -> i32 = std::mem::transmute(std::ptr::read(vt.add(1)));
            let id = id_fn(query);
            if id == kh_host::shop::rules().talk.action_button {
                let talk_id = std::ptr::read_unaligned(this.add(ENV_TALK) as *const i32);
                if let Some(Some(v)) = with_shared(|s| s.shop.before_env(talk_id, id)) {
                    std::ptr::write_unaligned(result as *mut i32, v);
                    std::ptr::write_unaligned(result.add(4) as *mut i32, 0);
                    std::ptr::write_unaligned(result.add(8) as *mut i32, 2);
                    return result;
                }
            }
        }
    }
    let r = original(this, result, query);
    if !ENABLED.load(Ordering::Relaxed) || !plausible(this as usize) || !plausible(query as usize) || !plausible(result as usize) {
        return r;
    }
    use super::rules::env::*;
    let vt = std::ptr::read(query as *const *const usize);
    if !plausible(vt as usize) {
        return r;
    }
    let id_fn: unsafe extern "C" fn(*mut u8) -> i32 = std::mem::transmute(std::ptr::read(vt.add(1)));
    let id = id_fn(query);
    if !matches!(id, DIALOG_RESULT | LIST_RESULT | DIALOG_OPEN | MENU_OPEN) {
        return r;
    }
    let mut args = Vec::new();
    if id == MENU_OPEN {
        let count_fn: unsafe extern "C" fn(*mut u8) -> i32 = std::mem::transmute(std::ptr::read(vt.add(2)));
        let arg_fn: unsafe extern "C" fn(*mut u8, *mut u8, i32) -> *const u8 = std::mem::transmute(std::ptr::read(vt.add(3)));
        if count_fn(query) >= 2 {
            let mut buf = [0u8; 16];
            let p = arg_fn(query, buf.as_mut_ptr(), 1);
            let v = value_at(p);
            // (an int is expected; anything else reads as "any menu")
            args.push(if matches!(v, Val::Int(_) | Val::Float(_)) { v } else { Val::Int(-1) });
        }
    }
    let talk_id = std::ptr::read_unaligned(this.add(ENV_TALK) as *const i32);
    if let Some(Some(v)) = with_shared(|s| s.shop.on_env(talk_id, id, &args).or_else(|| s.talks.on_env(talk_id, id, &args)) /* (SHOP) the KH shop stands for the window */) {
        std::ptr::write_unaligned(result as *mut i32, v);
        std::ptr::write_unaligned(result.add(8) as *mut i32, 2);
    }
    r
}

unsafe fn skip_byte() -> Option<*mut u8> {
    let g = SKIP_GLOBAL.load(Ordering::Relaxed);
    if g == 0 {
        return None;
    }
    let obj = std::ptr::read(g as *const usize);
    plausible(obj).then(|| (obj + SKIP_BYTE) as *mut u8)
}

unsafe extern "C" fn update_hook(ins: *mut u8, dt: *mut f32) {
    let original: UpdateFn = std::mem::transmute(ORIG_UPDATE.load(Ordering::Relaxed));
    let p = ins as usize;
    if !ENABLED.load(Ordering::Relaxed) || !plausible(p) {
        return original(ins, dt);
    }
    // A skipped this object's line: the game's own skip byte for this update only
    let mut put_back = None;
    // (ZETTA 2026-10-23, zboss.rs) a character Zettaflare killed speaks no more: the game's own skip on each update of its talk
    let zboss_quiet = crate::zboss::any_recent() && crate::zboss::ins_silenced(p, || super::game::npc_of(p).map(|(h, _)| crate::game::handle_bits(&h)));
    if zboss_quiet {
        if let Some(b) = skip_byte() {
            put_back = Some((b, *b));
            *b = 1;
        }
    }
    if put_back.is_none() && SKIP_INS.load(Ordering::Relaxed) == p {
        SKIP_INS.store(0, Ordering::Relaxed);
        if let Some(b) = skip_byte() {
            put_back = Some((b, *b));
            *b = 1;
        }
    }
    CUR_INS.store(p, Ordering::Relaxed);
    original(ins, dt);
    CUR_INS.store(0, Ordering::Relaxed);
    if let Some((b, v)) = put_back {
        *b = v;
    }
    let rd_i = |o: usize| std::ptr::read_unaligned(ins.add(o) as *const i32);
    let rd_f = |o: usize| std::ptr::read_unaligned(ins.add(o) as *const f32);
    let snap = LineSnap {
        ins: p,
        talk_id: rd_i(0),
        row: rd_i(INS_BLOCK).wrapping_add(rd_i(INS_INDEX)),
        msg: rd_i(INS_MSG),
        active: *ins.add(INS_TALKING) != 0,
        unskippable: *ins.add(INS_NO_SKIP) != 0,
        elapsed: rd_f(INS_ELAPSED),
        duration: rd_f(INS_LENGTH),
        age: 0,
    };
    let _ = with_shared(|s| match s.lines.iter_mut().find(|l| l.ins == p) {
        Some(l) => *l = snap,
        None if snap.active => s.lines.push(snap),
        None => {}
    });
}

unsafe extern "C" fn subtitle_hook(a: usize, b: usize, c: usize, d: usize, e: usize) {
    let original: SubtitleFn = std::mem::transmute(ORIG_SUBTITLE.load(Ordering::Relaxed));
    let ins = CUR_INS.load(Ordering::Relaxed);
    // (ZETTA 2026-10-23, zboss.rs) no subtitle of a character Zettaflare killed
    if plausible(ins) && crate::zboss::ins_known_silenced(ins) {
        return;
    }
    if ENABLED.load(Ordering::Relaxed) && SUPPRESS.load(Ordering::Relaxed) && plausible(ins) {
        // (CV2-b) every character's line is the bubble's (held or ambient) while the bubble can show
        let talk_id = std::ptr::read_unaligned(ins as *const i32);
        if super::rules::is_character_talk(talk_id, &super::game::skip_ids()) {
            return;
        }
    }
    original(a, b, c, d, e)
}

/// GIVES A CHOICE BACK to the game: its command run now as the script gave it (Elden Ring's own menu / dialog opens).
/// `event_obj`: the command object it came from (the talk script's own, alive while its talk object updates).
pub(crate) fn run_for_the_game(event_obj: usize, id: i32, args: &[i32]) -> bool {
    let f = ORIG_EVENT.load(Ordering::Relaxed);
    if f == 0 || !plausible(event_obj) {
        return false;
    }
    use eldenring::ez_state::{EzStateEvent, EzStateValue};
    let ev = EzStateEvent::from((id, args.iter().map(|a| EzStateValue::Int32(*a)).collect::<Vec<_>>()));
    let original: EventFn = unsafe { std::mem::transmute(f) };
    // SAFETY: the game's own command function on the script's own command object, with an event of the SDK's own
    // layout (as `TalkScript::event` makes it), on the game's thread.
    unsafe { original(event_obj as *mut u8, &ev as *const EzStateEvent as *mut u8) };
    true
}

/// The four detours (once; the frame switches them on with `talk.kh`).
pub fn install() {
    let mut ok = 0;
    for (item, hook, original) in [
        (&EVENT_FN, event_hook as *const () as usize, &ORIG_EVENT),
        (&ENV_FN, env_hook as *const () as usize, &ORIG_ENV),
        (&UPDATE_FN, update_hook as *const () as usize, &ORIG_UPDATE),
        (&SUBTITLE_FN, subtitle_hook as *const () as usize, &ORIG_SUBTITLE),
    ] {
        let Some(f) = locate(item) else {
            erlog!("talk: {} not found for this executable (PORT: 2.7.1.0 / 2.7.2.0 table): the KH conversation bubbles are off", item.name);
            continue;
        };
        if item.rva_2_7_1_0 == UPDATE_FN.rva_2_7_1_0 {
            // the skip byte's global, from the update's own load (must be the 2.7.1.0 one)
            // SAFETY: bytes of the function just located, inside the mapped executable.
            unsafe {
                let at = f + SKIP_LOAD_IN_FN;
                let code = std::slice::from_raw_parts(at as *const u8, 14);
                if code[..3] == [0x48, 0x8b, 0x05] && code[7..11] == [0x80, 0xb8, 0xc0, 0x92] {
                    let target = (at + 7).wrapping_add(i32::from_le_bytes([code[3], code[4], code[5], code[6]]) as isize as usize);
                    // (PORT) the running version's recorded global
                    if crate::gamever::pick(SKIP_GLOBAL_RVA, SKIP_GLOBAL_RVA_2720).map(|r| crate::entry::exe_base() + r) == Some(target) {
                        SKIP_GLOBAL.store(target, Ordering::SeqCst);
                    }
                }
            }
            if SKIP_GLOBAL.load(Ordering::Relaxed) == 0 {
                erlog!("talk: the game's line-skip byte was NOT found where this executable has it (PORT): A cannot skip a line");
            }
        }
        // SAFETY: the function's start, found by pattern at the recorded address; our hook has its signature.
        match unsafe { kh_er_render::inline_hook(f, hook, original, item.name) } {
            Ok(()) => ok += 1,
            Err(e) => {
                original.store(0, Ordering::SeqCst);
                erlog!("talk: {} was NOT detoured ({e})", item.name);
            }
        }
    }
    // all four or none of the adapter: the command and query detours must agree (a menu not run but not answered
    // would hang its script)
    let complete = [&ORIG_EVENT, &ORIG_ENV, &ORIG_UPDATE, &ORIG_SUBTITLE].iter().all(|o| o.load(Ordering::Relaxed) != 0);
    COMPLETE.store(complete, Ordering::SeqCst);
    erlog!("talk: {ok} of 4 talk functions detoured{}", if complete { ": Elden Ring's conversations can be KH bubbles (talk.kh)" } else { ": INCOMPLETE, the adapter stays off (Elden Ring's own subtitles and menus)" });
}

/// All four detours are in.
pub(crate) static COMPLETE: AtomicBool = AtomicBool::new(false);

/// The hooks are gone (the renderer's shutdown removed every inline hook).
pub fn uninstalled() {
    for o in [&ORIG_EVENT, &ORIG_ENV, &ORIG_UPDATE, &ORIG_SUBTITLE] {
        o.store(0, Ordering::SeqCst);
    }
    COMPLETE.store(false, Ordering::SeqCst);
    ENABLED.store(false, Ordering::SeqCst);
    SUPPRESS.store(false, Ordering::SeqCst);
}
