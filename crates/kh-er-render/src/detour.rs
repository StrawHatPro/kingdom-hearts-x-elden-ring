//! A minimal inline hook for x64: the first instructions of a function are replaced by a jump to
//! ours; a trampoline holds what was there and continues the function.
//!
//! WHY (measured, IMPL_ER2_STATE.md "crash 2"): the Steam overlay hooks `Present` too, with an
//! inline jump at the real function's start, and its detour re-hooks whatever function it finds in
//! a swap chain's vtable when that is not the one it knows. A vtable patch of ours on `Present`
//! therefore ends in an endless recursion (their detour -> our function -> the real function =
//! their jump -> their detour ...). An inline hook leaves the vtable alone, so they see nothing
//! new, and it chains cleanly with a jump of theirs that is already there: our trampoline simply
//! jumps to where their jump went.
//!
//! Only what a function prologue is made of is decoded (pushes, moves to the stack, `sub rsp`,
//! existing hook jumps). Anything else makes `install` refuse: no hook rather than a wrong one.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use windows::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows::Win32::System::Memory::{VirtualAlloc, VirtualProtect, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS};
use windows::Win32::System::Threading::GetCurrentProcess;

/// What one instruction at a function's start is, for moving it elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Insn {
    /// `n` bytes that mean the same at any address.
    Plain(usize),
    /// `jmp rel32` (5 bytes) to this displacement from its end.
    JumpRel(i32),
    /// `jmp [rip+0]` followed by its 8-byte target (14 bytes).
    JumpAbs(u64),
    Unknown,
}

fn modrm(code: &[u8], at: usize, immediate: usize) -> Insn {
    let Some(&m) = code.get(at) else { return Insn::Unknown };
    let (mode, rm) = (m >> 6, m & 7);
    let mut n = at + 1;
    if mode != 3 && rm == 4 {
        let Some(&sib) = code.get(n) else { return Insn::Unknown };
        n += 1;
        if mode == 0 && sib & 7 == 5 {
            n += 4;
        }
    }
    match mode {
        // (rip-relative: would have to be re-aimed; no prologue we met has one)
        0 if rm == 5 => return Insn::Unknown,
        1 => n += 1,
        2 => n += 4,
        _ => {}
    }
    if n + immediate > code.len() {
        return Insn::Unknown;
    }
    Insn::Plain(n + immediate)
}

/// Decodes the instruction at the start of `code`.
pub fn decode(code: &[u8]) -> Insn {
    let mut i = 0;
    let byte = |k: usize| code.get(k).copied();
    let operand_size = byte(i) == Some(0x66);
    if operand_size {
        i += 1;
    }
    let rex = match byte(i) {
        Some(r) if (0x40..=0x4F).contains(&r) => {
            i += 1;
            Some(r)
        }
        _ => None,
    };
    let Some(op) = byte(i) else { return Insn::Unknown };
    i += 1;
    let wide = rex.is_some_and(|r| r & 8 != 0);
    match op {
        0x50..=0x5F | 0x90 => Insn::Plain(i),
        0xE9 if rex.is_none() && !operand_size => match code.get(i..i + 4) {
            Some(d) => Insn::JumpRel(i32::from_le_bytes([d[0], d[1], d[2], d[3]])),
            None => Insn::Unknown,
        },
        0xFF if rex.is_none() && byte(i) == Some(0x25) => match (code.get(i + 1..i + 5), code.get(i + 5..i + 13)) {
            (Some([0, 0, 0, 0]), Some(t)) => Insn::JumpAbs(u64::from_le_bytes([t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7]])),
            _ => Insn::Unknown,
        },
        _ if operand_size && op != 0x0F => Insn::Unknown,
        0x01 | 0x03 | 0x29 | 0x2B | 0x31 | 0x33 | 0x39 | 0x3B | 0x63 | 0x84 | 0x85 | 0x88 | 0x89 | 0x8A | 0x8B | 0x8D => modrm(code, i, 0),
        0x80 | 0x83 | 0xC1 | 0xC6 => modrm(code, i, 1),
        0x69 | 0x81 | 0xC7 => modrm(code, i, 4),
        0xB8..=0xBF => {
            let n = i + if wide { 8 } else { 4 };
            if n <= code.len() {
                Insn::Plain(n)
            } else {
                Insn::Unknown
            }
        }
        0x0F => match byte(i) {
            // nop, movups / movaps to and from memory, movzx / movsx, movdqa / movdqu
            Some(0x1F | 0x10 | 0x11 | 0x28 | 0x29 | 0x6F | 0x7F | 0xB6 | 0xB7 | 0xBE | 0xBF) => modrm(code, i + 1, 0),
            _ => Insn::Unknown,
        },
        _ => Insn::Unknown,
    }
}

/// The plan for hooking a function that starts with `code` at address `at`: how many of its bytes
/// the trampoline takes over, and the trampoline's bytes (ending in a jump back or onward).
pub fn plan(code: &[u8], at: u64) -> Result<(usize, Vec<u8>), String> {
    let absolute = |target: u64| {
        let mut bytes = vec![0xFF, 0x25, 0, 0, 0, 0];
        bytes.extend_from_slice(&target.to_le_bytes());
        bytes
    };
    let mut taken = 0usize;
    let mut trampoline: Vec<u8> = Vec::new();
    while taken < 5 {
        match decode(&code[taken.min(code.len())..]) {
            Insn::Plain(n) => {
                trampoline.extend_from_slice(&code[taken..taken + n]);
                taken += n;
            }
            // Somebody's hook is already there: go where it goes.
            Insn::JumpRel(displacement) => {
                trampoline.extend(absolute((at + taken as u64 + 5).wrapping_add(displacement as i64 as u64)));
                return Ok((taken + 5, trampoline));
            }
            Insn::JumpAbs(target) => {
                trampoline.extend(absolute(target));
                return Ok((taken + 14, trampoline));
            }
            Insn::Unknown => return Err(format!("an instruction at +{taken} is not one this hook can move (bytes {})", code.iter().take(16).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "))),
        }
    }
    trampoline.extend(absolute(at + taken as u64));
    Ok((taken, trampoline))
}

struct Installed {
    target: usize,
    saved: [u8; 5],
    ours: [u8; 5],
    what: &'static str,
}

static INSTALLED: Mutex<Vec<Installed>> = Mutex::new(Vec::new());

/// Executable memory within reach of a 32-bit jump from `near`.
unsafe fn allocate_near(near: usize) -> Option<usize> {
    let base = near & !0xFFFF;
    for step in 1..0x7000usize {
        for candidate in [base.wrapping_sub(step * 0x10000), base.wrapping_add(step * 0x10000)] {
            if candidate < 0x10000 {
                continue;
            }
            let got = VirtualAlloc(Some(candidate as *const c_void), 0x1000, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
            if !got.is_null() {
                return Some(got as usize);
            }
        }
    }
    None
}

/// Hooks the function at `target`: calls of it arrive at `hook`; `original` receives the address
/// that continues the function (stored before the jump goes in).
///
/// # Safety
/// `target` must be the start of a function and `hook` a function of its signature that stays
/// loaded for the life of the process.
pub unsafe fn install(target: usize, hook: usize, original: &AtomicUsize, what: &'static str) -> Result<(), String> {
    if !crate::hook::readable(target, 32) {
        return Err(format!("{what}: {target:#x} is not readable"));
    }
    let code = std::slice::from_raw_parts(target as *const u8, 32).to_vec();
    let (_taken, trampoline) = plan(&code, target as u64).map_err(|e| format!("{what}: {e}"))?;
    let block = allocate_near(target).ok_or_else(|| format!("{what}: no memory near {target:#x}"))?;
    // The block: [relay: jmp [rip+0]; hook] [trampoline].
    let mut relay: Vec<u8> = vec![0xFF, 0x25, 0, 0, 0, 0];
    relay.extend_from_slice(&(hook as u64).to_le_bytes());
    std::ptr::copy_nonoverlapping(relay.as_ptr(), block as *mut u8, relay.len());
    let trampoline_at = block + 16;
    std::ptr::copy_nonoverlapping(trampoline.as_ptr(), trampoline_at as *mut u8, trampoline.len());
    let _ = FlushInstructionCache(GetCurrentProcess(), Some(block as *const c_void), 0x1000);
    original.store(trampoline_at, Ordering::SeqCst);

    let displacement = block as i64 - (target as i64 + 5);
    if displacement != displacement as i32 as i64 {
        return Err(format!("{what}: the relay is out of reach"));
    }
    let mut ours = [0xE9u8, 0, 0, 0, 0];
    ours[1..].copy_from_slice(&(displacement as i32).to_le_bytes());
    let mut saved = [0u8; 5];
    saved.copy_from_slice(&code[..5]);
    let mut old = PAGE_PROTECTION_FLAGS(0);
    VirtualProtect(target as *const c_void, 5, PAGE_EXECUTE_READWRITE, &mut old).map_err(|e| format!("{what}: VirtualProtect: {e}"))?;
    std::ptr::copy_nonoverlapping(ours.as_ptr(), target as *mut u8, 5);
    let _ = VirtualProtect(target as *const c_void, 5, old, &mut old);
    let _ = FlushInstructionCache(GetCurrentProcess(), Some(target as *const c_void), 5);
    INSTALLED.lock().unwrap().push(Installed { target, saved, ours, what });
    Ok(())
}

/// Puts the functions' first bytes back where they are still ours. (The relay and trampoline
/// memory stays: a thread may be in it.) Returns what could not be undone.
pub fn remove() -> Vec<&'static str> {
    let mut kept = Vec::new();
    let mut installed = INSTALLED.lock().unwrap();
    for h in installed.drain(..).rev() {
        // SAFETY: the same bytes we wrote before; the module stays loaded.
        unsafe {
            let now = std::slice::from_raw_parts(h.target as *const u8, 5);
            if now != h.ours {
                kept.push(h.what);
                continue;
            }
            let mut old = PAGE_PROTECTION_FLAGS(0);
            if VirtualProtect(h.target as *const c_void, 5, PAGE_EXECUTE_READWRITE, &mut old).is_ok() {
                std::ptr::copy_nonoverlapping(h.saved.as_ptr(), h.target as *mut u8, 5);
                let _ = VirtualProtect(h.target as *const c_void, 5, old, &mut old);
                let _ = FlushInstructionCache(GetCurrentProcess(), Some(h.target as *const c_void), 5);
            } else {
                kept.push(h.what);
            }
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prologue_instructions_are_measured() {
        // mov [rsp+8], rbx / mov [rsp+10h], rsi / push rdi / sub rsp, 20h
        assert_eq!(decode(&[0x48, 0x89, 0x5C, 0x24, 0x08, 0x48]), Insn::Plain(5));
        assert_eq!(decode(&[0x48, 0x89, 0x74, 0x24, 0x10]), Insn::Plain(5));
        assert_eq!(decode(&[0x57, 0x48]), Insn::Plain(1));
        assert_eq!(decode(&[0x41, 0x56]), Insn::Plain(2));
        assert_eq!(decode(&[0x48, 0x83, 0xEC, 0x20]), Insn::Plain(4));
        assert_eq!(decode(&[0x48, 0x81, 0xEC, 0x90, 0x00, 0x00, 0x00]), Insn::Plain(7));
        // mov rax, rsp / mov r11, rsp / mov [r11+8], rbx / lea rbp, [rsp-47h] / mov ebp, r8d
        assert_eq!(decode(&[0x48, 0x8B, 0xC4]), Insn::Plain(3));
        assert_eq!(decode(&[0x4C, 0x8B, 0xDC]), Insn::Plain(3));
        assert_eq!(decode(&[0x49, 0x89, 0x5B, 0x08]), Insn::Plain(4));
        assert_eq!(decode(&[0x48, 0x8D, 0x6C, 0x24, 0xB9]), Insn::Plain(5));
        assert_eq!(decode(&[0x41, 0x8B, 0xE8]), Insn::Plain(3));
        // movaps [rsp+20h], xmm6
        assert_eq!(decode(&[0x0F, 0x29, 0x74, 0x24, 0x20]), Insn::Plain(5));
        // hooks of others
        assert_eq!(decode(&[0xE9, 0x10, 0x00, 0x00, 0x00]), Insn::JumpRel(0x10));
        assert_eq!(decode(&[0xFF, 0x25, 0, 0, 0, 0, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]), Insn::JumpAbs(0x1122334455667788));
        // rip-relative and unknown: refused
        assert_eq!(decode(&[0x48, 0x8B, 0x05, 0x10, 0x00, 0x00, 0x00]), Insn::Unknown);
        assert_eq!(decode(&[0xCC]), Insn::Unknown);
        assert_eq!(decode(&[0xE8, 0, 0, 0, 0]), Insn::Unknown);
    }

    #[test]
    fn the_trampoline_continues_or_follows_a_hook() {
        // A plain prologue: 5 + 1 bytes are taken (the first whole instructions covering 5 bytes
        // ... here the first alone), then a jump back to the rest.
        let code = [0x48, 0x89, 0x5C, 0x24, 0x08, 0x57, 0x48, 0x83, 0xEC, 0x20, 0, 0, 0, 0, 0, 0];
        let (taken, t) = plan(&code, 0x1000).unwrap();
        assert_eq!(taken, 5);
        assert_eq!(&t[..5], &code[..5]);
        assert_eq!(&t[5..11], &[0xFF, 0x25, 0, 0, 0, 0]);
        assert_eq!(u64::from_le_bytes(t[11..19].try_into().unwrap()), 0x1005);
        // Short instructions first: push rbx (2) + sub rsp (4) = 6 bytes taken.
        let code = [0x40, 0x53, 0x48, 0x83, 0xEC, 0x20, 0x48, 0x8B, 0xD9, 0, 0, 0, 0, 0, 0, 0];
        let (taken, t) = plan(&code, 0x2000).unwrap();
        assert_eq!((taken, u64::from_le_bytes(t[12..20].try_into().unwrap())), (6, 0x2006));
        // A function another party already hooked with jmp rel32: the trampoline goes where their
        // jump went (0x3000 + 5 + 0x100), nothing of the function's own code is copied.
        let code = [0xE9, 0x00, 0x01, 0x00, 0x00, 0x90, 0x90, 0x90, 0, 0, 0, 0, 0, 0, 0, 0];
        let (taken, t) = plan(&code, 0x3000).unwrap();
        assert_eq!((taken, t.len(), u64::from_le_bytes(t[6..14].try_into().unwrap())), (5, 14, 0x3105));
        // A backward jump.
        let code = [0xE9, 0xFB, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(u64::from_le_bytes(plan(&code, 0x3000).unwrap().1[6..14].try_into().unwrap()), 0x3000);
        // Unknown code: no plan.
        assert!(plan(&[0xCC; 16], 0x4000).is_err());
    }
}
