//! (GF-SENSOR) TAE, a character's "time act" file: for each animation, the timed events the game runs while it plays. Read
//! from the game's own `/chr/cNNNN.anibnd.dcx` at run time (no table of it is shipped anywhere). Only what the attack
//! sensor needs is kept: `ATTACK`, `BULLET`, `THROW` windows and the flag event's parry window.
//!
//! The layout (little-endian, 64-bit; MEASURED on 2.7.2.0's files, see build/sora_tmp/party/GOOFY/SENSOR_NOTES.md):
//! - header: "TAE " at 0, file id at 0x50, animation count at 0x54, the offset of the animation list at 0x58;
//! - animation list: `{ id: u64, offset: u64 }`, sorted by id;
//! - animation: event list offset at 0, the mini header's offset at 0x18, event count at 0x20;
//! - mini header: kind u32 at 0 (1 = this animation's events are those of another animation of the file: id at 0x18);
//! - event list: `{ start: u64, end: u64, data: u64 }` (offsets of the two f32 times, of the event's data);
//! - event data: kind u32 at 0, the arguments at the offset stored at 8.

use std::collections::BTreeMap;

/// Event kinds kept (the SDK's `CSTaeAnimEventId`).
pub const FLAG: u32 = 0;
pub const ATTACK: u32 = 1;
pub const BULLET: u32 = 2;
pub const THROW: u32 = 304;

/// `FLAG` argument 0 that opens the parry window.
pub const FLAG_PARRYABLE: u32 = 5;

/// One event of an animation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub kind: u32,
    pub start: f32,
    pub end: f32,
    /// The first argument words (`ATTACK`: judge id at 2; `BULLET`: judge id at 2; `THROW`: judge id at 0).
    pub args: [i32; 4],
}

/// The events of one animation, by start time; `import`: the animation whose events these are, when not its own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Anim {
    pub events: Vec<Event>,
    pub import: Option<i64>,
}

/// One TAE file: its id (`aNNN.tae`) and animations by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tae {
    pub id: i32,
    pub anims: BTreeMap<i64, Anim>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at.checked_add(4)?).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
}

fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    u32_at(b, at).map(|v| v as i32)
}

fn f32_at(b: &[u8], at: usize) -> Option<f32> {
    u32_at(b, at).map(f32::from_bits)
}

fn u64_at(b: &[u8], at: usize) -> Option<usize> {
    let x = b.get(at..at.checked_add(8)?)?;
    Some(u64::from_le_bytes([x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]]) as usize)
}

fn is_kept(kind: u32, args: &[i32; 4]) -> bool {
    matches!(kind, ATTACK | BULLET | THROW) || (kind == FLAG && args[0] as u32 == FLAG_PARRYABLE)
}

/// One TAE file's kept events.
pub fn read(b: &[u8]) -> Result<Tae, String> {
    read_at(b, 0)
}

/// The same for a copy of the file as the game holds it (MEASURED on 2.7.2.0, a dump of the loaded c4310.tae): at `base`, its
/// offsets turned into addresses, an event's two times in the low words of the head's first two words, and the
/// animation's event count in the low half of its word.
pub fn read_loaded(b: &[u8], base: usize) -> Result<Tae, String> {
    let list = u64_at(b, 0x58).ok_or("short TAE")?;
    read_at(b, if list >= b.len() { base } else { 0 })
}

fn read_at(b: &[u8], base: usize) -> Result<Tae, String> {
    let loaded = base != 0;
    let off = |at: usize| u64_at(b, at).map(|v| v.wrapping_sub(base));
    if !b.starts_with(b"TAE ") || b.get(7) != Some(&0xFF) {
        return Err("not a 64-bit TAE".to_string());
    }
    let id = i32_at(b, 0x50).ok_or("short TAE")?;
    let count = u32_at(b, 0x54).ok_or("short TAE")? as usize;
    let list = off(0x58).ok_or("short TAE")?;
    if count > 1 << 20 {
        return Err(format!("TAE {id}: {count} animations"));
    }
    let mut anims = BTreeMap::new();
    for k in 0..count {
        let (Some(anim_id), Some(at)) = (u64_at(b, list.wrapping_add(k * 16)), off(list.wrapping_add(k * 16 + 8))) else {
            return Err(format!("TAE {id}: animation {k} lies past the end"));
        };
        let events_at = off(at).ok_or("short animation")?;
        let mini = off(at + 0x18).ok_or("short animation")?;
        let n = u32_at(b, at + 0x20).ok_or("short animation")? as usize;
        let n = if loaded { n & 0xFFFF } else { n };
        let import = (u32_at(b, mini) == Some(1)).then(|| i32_at(b, mini + 0x18).map(i64::from)).flatten().filter(|v| *v >= 0);
        let mut events = Vec::new();
        for e in 0..n.min(4096) {
            let head = events_at.wrapping_add(e * 24);
            let (Some(s), Some(t), Some(data)) = (u64_at(b, head), u64_at(b, head.wrapping_add(8)), off(head.wrapping_add(16))) else { break };
            let (start, end) = if loaded { (Some(f32::from_bits(s as u32)), Some(f32::from_bits(t as u32))) } else { (f32_at(b, s), f32_at(b, t)) };
            let (Some(start), Some(end), Some(kind), Some(args_at)) = (start, end, u32_at(b, data), off(data.wrapping_add(8))) else { break };
            let mut args = [0i32; 4];
            for (i, a) in args.iter_mut().enumerate() {
                *a = i32_at(b, args_at.wrapping_add(i * 4)).unwrap_or(0);
            }
            if is_kept(kind, &args) {
                events.push(Event { kind, start, end, args });
            }
        }
        events.sort_by(|x, y| x.start.total_cmp(&y.start));
        anims.insert(anim_id as i64, Anim { events, import });
    }
    Ok(Tae { id, anims })
}

/// The  names a file carries, in order of first sight (a character without a TAE of its own names the character whose
/// animations it uses in its skeleton: MEASURED c4311 > c4310).
pub fn chr_names(b: &[u8]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in b.windows(5) {
        if w[0] == b'c' && w[1..].iter().all(u8::is_ascii_digit) {
            let name = String::from_utf8_lossy(w).to_string();
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file with one animation of one attack event, laid out as the module's head says.
    fn sample() -> Vec<u8> {
        let mut b = vec![0u8; 0x200];
        b[..4].copy_from_slice(b"TAE ");
        b[7] = 0xFF;
        let put32 = |b: &mut Vec<u8>, at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let put64 = |b: &mut Vec<u8>, at: usize, v: u64| b[at..at + 8].copy_from_slice(&v.to_le_bytes());
        put32(&mut b, 0x50, 3); // id
        put32(&mut b, 0x54, 1); // count
        put64(&mut b, 0x58, 0x80); // list
        put64(&mut b, 0x80, 3001); // animation id
        put64(&mut b, 0x88, 0xa0); // its offset
        put64(&mut b, 0xa0, 0xe0); // events
        put64(&mut b, 0xb8, 0xd0); // mini header
        put32(&mut b, 0xc0, 1); // one event
        put64(&mut b, 0xe0, 0x100); // start time's offset
        put64(&mut b, 0xe8, 0x104); // end time's offset
        put64(&mut b, 0xf0, 0x110); // data
        b[0x100..0x104].copy_from_slice(&0.5f32.to_le_bytes());
        b[0x104..0x108].copy_from_slice(&0.75f32.to_le_bytes());
        put32(&mut b, 0x110, ATTACK);
        put64(&mut b, 0x118, 0x120);
        put32(&mut b, 0x128, 100); // the judge id
        b
    }

    #[test]
    fn reads_an_attack_window() {
        let t = read(&sample()).unwrap();
        assert_eq!(t.id, 3);
        let a = &t.anims[&3001];
        assert_eq!(a.import, None);
        assert_eq!(a.events, vec![Event { kind: ATTACK, start: 0.5, end: 0.75, args: [0, 0, 100, 0] }]);
    }

    #[test]
    fn reads_a_copy_of_the_loaded_file() {
        let mut b = sample();
        let base = 0x7ff1_0000_0000u64;
        for at in [0x58, 0x88, 0xa0, 0xb8, 0xf0, 0x118] {
            let v = u64::from_le_bytes(b[at..at + 8].try_into().unwrap());
            b[at..at + 8].copy_from_slice(&(v + base).to_le_bytes());
        }
        b[0xe0..0xe4].copy_from_slice(&0.5f32.to_le_bytes());
        b[0xe8..0xec].copy_from_slice(&0.75f32.to_le_bytes());
        b[0xc2] = 0x24; // (the count's word has more in its high half)
        assert_eq!(read_loaded(&b, base as usize).unwrap(), read(&sample()).unwrap());
        assert_eq!(read_loaded(&sample(), base as usize).unwrap(), read(&sample()).unwrap(), "an untouched file reads the same");
    }

    #[test]
    fn refuses_what_is_no_tae() {
        assert!(read(b"BND4").is_err());
    }
}
