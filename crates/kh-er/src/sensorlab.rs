//! (GF-SENSOR) THE ATTACK SENSOR'S TEST BENCH (control keys, off unless written; never in a shipped ini):
//! - `atk.log = 1`: per frame, what the sensor can read of the attackers near the leader, logged at each change.
//! - `atk.find = n,Text`: the objects under its modules whose class name holds Text, five levels down (n = p: the player).
//! - `atk.peek = n,off,off..[,+size]`: the object reached from character n (or p) through the offsets (hex), its pointers with their class names.
//! - `atk.refs = n`: the memory holding a pointer to character n, read as `CSAiTargetingSystemOwner`s, and what points at those.
//! - `atk.aim = 1`: each hostile's AI target logged at each change (telegraph.rs `ai_target`).
//! - `atk.shoot = n`: a game bullet from hostile n at the body, owned by that enemy (R18: the sensor must name it when it lands).
//! - `atk.wake = cNNNN`: that character's AI switched on the way its boss event does (a boss asleep before its fight).
//! - `atk.sponge = 1`: hostiles within 30 m are put back to full HP when under half (a long look at one enemy's attacks).
//! - `atk.classes = n`: the non-plain attacks in the book of hostile n (`dbg.fire = who,W_AttackNNNN` plays one).
//! - `atk.nodead = cNNNN`: that character gets the game's no-death switch (data module + 0x19B bit 0), for the no-death rule.
//! - `atk.fire = who,W_Name[,tag]`: that event on the nearest hostile of `who`: an animation set (0..5), `cNNNN`, `cNNNN/set`, or `*` for any.
//!   (`tag`: anything, so the same event can be asked for twice.)
//! - `atk.keep = 1`: Sora kept at full health (a long look at what enemies do to the body).
//! - `atk.break = n,seconds`: the stance ladder (stance.rs) on hostile number n by distance, or `cNNNN,seconds`: the nearest of that id.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use eldenring::cs::{CSBulletManager, ChrIns, PlayerIns};
use fromsoftware_shared::FromStatic;

use crate::chranim::Reader;
use crate::control::{Control, OneShot};
use crate::erlog;
use crate::game::plausible;
use crate::targets::{Enemy, ErTargets};
use crate::word;

#[derive(Clone, Copy, Default, PartialEq)]
struct Seen {
    anim: i32,
    parry: bool,
    signal: bool,
    possible: bool,
}

#[derive(Clone)]
struct Shot {
    rows: [i32; 3],
    owner: Option<u32>,
    pos: [f32; 3],
    age: f32,
}

struct Lab {
    find: OneShot,
    brk: OneShot,
    shoot: OneShot,
    fire: OneShot,
    nodead: OneShot,
    classes: OneShot,
    wake: OneShot,
    peek: OneShot,
    refs: OneShot,
    seen: HashMap<u32, Seen>,
    damage: (u8, u32),
    shots: HashMap<u64, Shot>,
    aim: HashMap<u32, u8>,
    clock: f32,
}

static LAB: Mutex<Option<Lab>> = Mutex::new(None);
static LOG: AtomicBool = AtomicBool::new(false);

/// Once a frame, before the hits are read. True: `atk.keep`.
pub fn frame(control: &Control, player: &PlayerIns, targets: &mut ErTargets, dt: f32) -> bool {
    LOG.store(control.on("atk.log"), Ordering::Relaxed);
    let mut guard = LAB.lock().unwrap_or_else(|e| e.into_inner());
    let lab = guard.get_or_insert_with(|| Lab { find: OneShot::new("atk.find", control), brk: OneShot::new("atk.break", control), shoot: OneShot::new("atk.shoot", control), fire: OneShot::new("atk.fire", control), nodead: OneShot::new("atk.nodead", control), classes: OneShot::new("atk.classes", control), wake: OneShot::new("atk.wake", control), peek: OneShot::new("atk.peek", control), refs: OneShot::new("atk.refs", control), seen: HashMap::new(), damage: (0, 0), shots: HashMap::new(), aim: HashMap::new(), clock: 0.0 });
    lab.clock += dt;
    if let Some(text) = lab.peek.fired(control) {
        let parts: Vec<&str> = text.split(',').map(str::trim).collect();
        if let Some(chr) = pick(player, targets, parts.first().copied().unwrap_or("")) {
            peek(chr, &parts[1..]);
        }
    }
    if let Some(text) = lab.shoot.fired(control) {
        let (who, at_friend) = text.split_once(',').unwrap_or((&text, ""));
        let list = targets.hostiles_by_distance();
        let shooter = list.get(who.trim().parse::<usize>().unwrap_or(0)).map(|(_, e)| (e.handle, e.er_pos, e.id, e.npc_id));
        drop(list);
        // (`fire_owned` is the party build's)
        #[cfg(not(feature = "party"))]
        let _ = (shooter, at_friend);
        #[cfg(feature = "party")]
        if let Some((owner, at, id, npc)) = shooter {
            // (`n,goofy`: at his body)
            let friend = (at_friend.trim() == "goofy").then(|| crate::allybody::live_handle(kh_host::party::PartyChara::Goofy)).flatten();
            let friend_chr = friend.and_then(|h| crate::targets::find_chr(&h));
            let body = friend_chr.as_ref().map_or_else(|| crate::game::v3(&player.chr_ins.modules.physics.position), |c| crate::game::v3(&c.modules.physics.position)) + glam::Vec3::Y;
            // (a borrowed row flies a short way: it starts close enough to land, still the enemy's own)
            let toward = (at + glam::Vec3::Y * 1.4 - body).normalize_or_zero();
            let from = body + toward * 1.5;
            let dir = -toward;
            let attack = crate::bullet::Attack { phys: 60, ..crate::bullet::Attack::PLAIN };
            let shape = targets.shape;
            let target = friend.or(targets.player_handle);
            let shot = targets.bullets.fire_owned(owner, target, from, dir, attack, shape);
            erlog!("atk: shoot: target {} c{npc:04} fires at the body from {:.1} m: {shot:?}", id.0, (body - from).length());
        }
    }
    if let Some(text) = lab.wake.fired(control) {
        let (who, bar) = text.split_once(',').unwrap_or((&text, ""));
        let want = who.trim().strip_prefix('c').and_then(|n| n.parse::<i32>().ok());
        for (_, e) in targets.hostiles_by_distance().iter().filter(|(_, e)| Some(e.npc_id) == want) {
            if bar.trim() == "bar" {
                crate::erhud::wake_boss_bar(e.handle, 900_000_000 + e.npc_id * 1000);
            }
            if let Some(chr) = targets.chr_mut(e.id) {
                let was = crate::puppet::ai_switch::ai_bit(chr);
                let on = crate::puppet::ai_switch::set_ai_on(chr, true);
                #[cfg(feature = "party")]
                crate::sdkx::set_update_rate(chr, crate::sdkx::EVENT_FIGHT_RATE);
                erlog!("atk: wake: target {} c{:04}: AI {was:?} -> on ({on})", e.id.0, e.npc_id);
            }
        }
    }
    if let Some(text) = lab.classes.fired(control) {
        let list = targets.hostiles_by_distance();
        if let Some((_, e)) = list.get(text.trim().parse::<usize>().unwrap_or(0)) {
            if let Some(chr) = targets.chr_mut(e.id) {
                crate::telegraph::list_classes(e, chr);
            }
        }
    }
    if let Some(text) = lab.nodead.fired(control) {
        let want = text.trim().strip_prefix('c').and_then(|n| n.parse::<i32>().ok());
        for (_, e) in targets.hostiles_by_distance().iter().filter(|(_, e)| Some(e.npc_id) == want) {
            if let Some(chr) = targets.chr_mut(e.id) {
                let data: &eldenring::cs::CSChrDataModule = &chr.modules.data;
                let at = data as *const eldenring::cs::CSChrDataModule as usize + 0x19B;
                // SAFETY: inside the live data module, the byte zdefer.rs reads.
                unsafe { *(at as *mut u8) |= 1 };
                erlog!("atk: nodead: target {} c{:04}: {:?}", e.id.0, e.npc_id, crate::zdefer::no_dead_of(chr));
            }
        }
    }
    if let Some(text) = lab.fire.fired(control) {
        if let Some((set, name)) = text.split_once(',') {
            let who = set.trim();
            let name = name.split(',').next().unwrap_or(name).trim();
            let list = targets.hostiles_by_distance();
            let (kind, set) = match who.split_once('/') {
                Some((k, s)) => (k, s),
                None if who.starts_with('c') => (who, ""),
                None => ("", who),
            };
            let kind = kind.strip_prefix('c').and_then(|n| n.parse::<i32>().ok());
            let set = set.parse::<i32>().ok();
            let found = list.iter().find(|(_, e)| kind.is_none_or(|k| e.npc_id == k) && set.is_none_or(|s| e.anim / 1_000_000 == s));
            match found {
                Some((d, e)) => {
                    let done = targets.chr_mut(e.id).map(|chr| crate::chranim::fire(chr, name));
                    erlog!("atk: fire: {name} on target {} c{:04} ({:.1} m), anim set {}: {done:?}", e.id.0, e.npc_id, d / 100.0, e.anim / 1_000_000);
                }
                None => erlog!("atk: fire: no hostile of {who} read now"),
            }
        }
    }
    if let Some(text) = lab.refs.fired(control) {
        if let Some(chr) = pick(player, targets, &text) {
            refs(chr);
        }
    }
    if let Some(text) = lab.brk.fired(control) {
        let (who, window) = text.split_once(',').unwrap_or((&text, "0"));
        let window = window.trim().parse::<f32>().unwrap_or(0.0);
        let want = who.trim().strip_prefix('c').and_then(|n| n.parse::<i32>().ok());
        let list = targets.hostiles_by_distance();
        let pick = match want {
            Some(id) => list.iter().find(|(_, e)| e.npc_id == id),
            None => list.get(who.trim().parse::<usize>().unwrap_or(0)),
        };
        match pick {
            // (PARTY-AI 2026-11-03) the kneel alone: the party's Stagger book is not broken, so no Goofy Breaker is offered (a
            // real KH break, `stagger: ... BROKEN`: `party.word = x3g stagger near 999`)
            Some((d, e)) => erlog!("stance: test on target {} c{:04} ({:.1} m, poise {:.0}/{:.0}, HP {}/{}): asked {} (the game's kneel only, NOT a KH Stagger break: no Goofy Breaker; for one use party.word = x3g stagger near 999)", e.id.0, e.npc_id, d / 100.0, e.poise, e.poise_max, e.hp, e.hp_max, crate::stance::begin(targets, e.id, window)),
            None => erlog!("stance: test: no such hostile near ({who})"),
        }
    }
    if let Some(text) = lab.find.fired(control) {
        if let Some((who, name)) = text.split_once(',') {
            if let Some(chr) = pick(player, targets, who) {
                find_class(chr, name.trim());
            }
        }
    }
    if control.on("atk.log") {
        lab.watch(player, targets);
    }
    if control.on("atk.sponge") {
        for (d, e) in targets.hostiles_by_distance() {
            if d <= 3000.0 && e.hp * 2 < e.hp_max {
                if let Some(chr) = targets.chr_mut(e.id) {
                    chr.modules.data.as_mut().hp = chr.modules.data.max_hp;
                }
            }
        }
    }
    if control.on("atk.aim") {
        lab.aims(player, targets);
    }
    control.on("atk.keep")
}

fn pick<'a>(player: &'a PlayerIns, targets: &ErTargets, text: &str) -> Option<&'a ChrIns> {
    if text.trim() == "p" {
        return Some(&player.chr_ins);
    }
    nth_hostile(targets, text).map(|c| &*c)
}

fn nth_hostile(targets: &ErTargets, text: &str) -> Option<&'static mut ChrIns> {
    let n = text.trim().parse::<usize>().unwrap_or(0);
    let list = targets.hostiles_by_distance();
    let (_, e) = list.get(n)?;
    erlog!("atk: hostile {n}: target {} c{:04} npc {} anim {}", e.id.0, e.npc_id, e.npc_param, e.anim);
    targets.chr_mut(e.id)
}

fn describe(e: &Enemy, chr: &ChrIns, time: f32) -> String {
    let w = crate::taebook::book(e.npc_id, chr).and_then(|b| b.anims.get(&(e.anim as i64)).cloned());
    format!("t{} c{:04} anim {} @{time:.2}{}", e.id.0, e.npc_id, e.anim, w.map_or(String::new(), |w| format!(" tae {}", w.iter().map(|x| format!("[{} {:.2}-{:.2} j{}]", x.kind, x.start, x.end, x.judge)).collect::<Vec<_>>().join(""))))
}

impl Lab {
    /// `atk.aim`: whom each hostile's AI has as its enemy.
    fn aims(&mut self, player: &PlayerIns, targets: &ErTargets) {
        let me = crate::game::v3(&player.chr_ins.modules.physics.position);
        let aims = crate::telegraph::aims();
        for s in crate::telegraph::list().iter().filter(|s| s.active && !s.flight) {
            if let (Some((_, p)), Some(e)) = (aims.iter().find(|(id, _)| *id == s.attacker), targets.get(s.attacker)) {
                let d = (e.er_pos - *p).length();
                let body = (*p - me).length();
                if d < 4.0 && body > 1.5 {
                    erlog!("atk.aim: t{} window open {:?} on something {d:.1} m from it, not the leader ({body:.1} m from the leader's body)", s.attacker.0, s.class);
                }
            }
        }
        for (d, e) in targets.hostiles_by_distance() {
            if d > 3000.0 {
                break;
            }
            let aim = aims.iter().find(|(id, _)| *id == e.id).map(|(_, p)| *p);
            let state = match aim {
                None => 0,
                Some(p) if (p - me).length() < 1.5 => 1,
                Some(_) => 2,
            };
            if self.aim.insert(e.id.0, state) != Some(state) {
                erlog!("atk.aim: t{} c{:04} ({:.1} m): {} (body at ({:.1}, {:.1}, {:.1}))", e.id.0, e.npc_id, d / 100.0, aim.map_or("no target read".to_string(), |p| format!("its target stands at ({:.1}, {:.1}, {:.1}), {:.1} m from the body", p.x, p.y, p.z, (p - me).length())), me.x, me.y, me.z);
            }
        }
    }

    fn watch(&mut self, player: &PlayerIns, targets: &ErTargets) {
        let clock = self.clock;
        for (d, e) in targets.hostiles_by_distance() {
            if d > 3000.0 {
                break;
            }
            let Some(chr) = targets.chr_mut(e.id) else { continue };
            if !plausible(word!(chr.modules.action_flag)) {
                continue;
            }
            let f = &chr.modules.action_flag.action_modifiers_flags;
            let now = crate::chranim::now(chr);
            let s = Seen { anim: now.id, parry: f.parryable(), signal: f.ai_parry_signal(), possible: f.ai_parry_possible_state() };
            let before = self.seen.insert(e.id.0, s).unwrap_or_default();
            if before == s {
                continue;
            }
            let attacky = (3000..4000).contains(&now.id.rem_euclid(1_000_000));
            if s.anim != before.anim && attacky {
                erlog!("atk: [{clock:.3}] {} starts ({:.1} m, length {:.2}, speed {:.2})", describe(e, chr, now.time), d / 100.0, now.length, crate::chranim::speed(chr));
            }
            if s.parry != before.parry || s.signal != before.signal || s.possible != before.possible {
                erlog!("atk: [{clock:.3}] {} parryable {} signal {} possible {}", describe(e, chr, now.time), s.parry, s.signal, s.possible);
            }
        }
        let flag = &player.chr_ins.modules.action_flag;
        let damage = (flag.damage_level, flag.received_damage_type);
        if damage != self.damage {
            erlog!("atk: [{clock:.3}] the body's received: level {} type {} (was {:?})", damage.0, damage.1, self.damage);
            self.damage = damage;
        }
        self.watch_bullets(player, targets);
    }

    fn watch_bullets(&mut self, player: &PlayerIns, targets: &ErTargets) {
        // SAFETY: game thread; the list is the game's own, walked as the SDK walks it.
        let Ok(manager) = (unsafe { CSBulletManager::instance_mut() }) else { return };
        let me = &player.chr_ins.modules.physics.position;
        let (mx, my, mz) = (me.0, me.1, me.2);
        let mut now: HashMap<u64, Shot> = HashMap::new();
        for b in manager.bullets().take(256) {
            let p = &b.physics.position;
            let d2 = (p.0 - mx).powi(2) + (p.1 - my).powi(2) + (p.2 - mz).powi(2);
            let rows = [b.wait_state.base.param.param_id, b.fly_state.base.param.param_id, b.exp_state.base.param.param_id];
            let owner = targets.id_of(&b.targeting_owner.owner_chr_handle).map(|t| t.0);
            if d2 < 40.0 * 40.0 && owner.is_some() {
                now.insert(crate::game::handle_bits(&b.field_ins_handle), Shot { rows, owner, pos: [p.0, p.1, p.2], age: b.time_alive });
            }
        }
        let clock = self.clock;
        for (k, s) in &now {
            if !self.shots.contains_key(k) {
                erlog!("atk: [{clock:.3}] bullet {k:#x} appears: rows {:?} owner {:?}", s.rows, s.owner);
            }
        }
        for (k, s) in &self.shots {
            if !now.contains_key(k) {
                let d = ((s.pos[0] - mx).powi(2) + (s.pos[1] - my).powi(2) + (s.pos[2] - mz).powi(2)).sqrt();
                erlog!("atk: [{clock:.3}] bullet {k:#x} vanished: rows {:?} owner {:?} age {:.2}, last {d:.1} m from the body", s.rows, s.owner, s.age);
            }
        }
        self.shots = now;
    }
}

/// A hit reached the body (`who`: the body's name for the log).
pub fn on_hit(chr: &ChrIns, lost: i32, who: &str) {
    if !LOG.load(Ordering::Relaxed) {
        return;
    }
    let flag: &eldenring::cs::CSChrActionFlagModule = &chr.modules.action_flag;
    let base = flag as *const eldenring::cs::CSChrActionFlagModule as usize;
    // SAFETY: inside the live module (0x258 bytes, SDK size test).
    let raw: Vec<String> = (0..12).map(|k| format!("{:08x}", unsafe { std::ptr::read_unaligned((base + 0x10 + k * 4) as *const u32) })).collect();
    erlog!("atk: HIT on {who}: {lost} HP; level {} type {} last_hit_by {:#x}; flag words +0x10: {}", flag.damage_level, flag.received_damage_type, crate::game::handle_bits(&chr.last_hit_by), raw.join(" "));
}

/// The class name of an object with a vtable in the executable's image.
pub(crate) fn rtti(r: &mut Reader, object: usize) -> Option<String> {
    let base = crate::entry::exe_base();
    let in_image = |v: usize| (base..base + 0x1000_0000).contains(&v);
    let vtable = r.peek(object)?;
    if !in_image(vtable) {
        return None;
    }
    let col = r.peek(vtable.checked_sub(8)?)?;
    if !in_image(col) {
        return None;
    }
    let td = (r.peek(col + 8)? >> 32) as u32 as usize;
    let mut bytes = [0u8; 96];
    if !crate::chranim::read_bytes(base + td + 0x10, &mut bytes) {
        return None;
    }
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(96);
    std::str::from_utf8(&bytes[..end]).ok().filter(|s| s.starts_with(".?A")).map(str::to_string)
}

/// `atk.peek`: what lies at the end of a chain of offsets from a character.
fn peek(chr: &ChrIns, chain: &[&str]) {
    let mut r = Reader::default();
    let mut at = chr as *const ChrIns as usize;
    let mut size = 0x300usize;
    for hop in chain {
        if let Some(n) = hop.strip_prefix('+') {
            size = usize::from_str_radix(n.trim_start_matches("0x"), 16).unwrap_or(size);
            continue;
        }
        if let Some(n) = hop.strip_prefix('~') {
            at += usize::from_str_radix(n.trim_start_matches("0x"), 16).unwrap_or(0);
            continue;
        }
        let Ok(off) = usize::from_str_radix(hop.trim_start_matches("0x"), 16) else { continue };
        match r.peek(at + off) {
            Some(next) => at = next,
            None => {
                erlog!("atk.peek: {at:#x}+{off:#x} is not readable");
                return;
            }
        }
    }
    erlog!("atk.peek: {:?} = {at:#x} {}", chain, rtti(&mut r, at).unwrap_or_default());
    let mut line = String::new();
    for k in (0..size).step_by(8) {
        let Some(w) = r.peek(at + k) else { break };
        if let Some(name) = rtti(&mut r, at + k) {
            erlog!("atk.peek:   +{k:#x}: EMBEDDED {name}");
        } else if plausible(w) {
            let name = rtti(&mut r, w).unwrap_or_default();
            let pointee = r.peek(w).unwrap_or(0);
            erlog!("atk.peek:   +{k:#x}: {w:#x} {name}{}", if name.is_empty() { format!(" > {pointee:#x}") } else { String::new() });
        } else if w != 0 {
            line.push_str(&format!(" +{k:#x}={w:#x}"));
        }
    }
    erlog!("atk.peek: words:{line}");
}

/// The objects under a character's modules whose class name holds `text`, breadth first.
fn find_class(chr: &ChrIns, text: &str) {
    let started = std::time::Instant::now();
    let mut r = Reader::default();
    let mut level = vec![("chr".to_string(), chr as *const ChrIns as usize)];
    let mut seen = std::collections::HashSet::new();
    let mut found = 0;
    for depth in 0..5 {
        let mut next = Vec::new();
        for (path, object) in &level {
            if let Some(name) = rtti(&mut r, *object) {
                if name.contains(text) && found < 60 {
                    found += 1;
                    erlog!("atk.find: {path} {object:#x} {name}");
                }
            }
            let span = if depth < 3 { 0x4000usize } else { 0x500 };
            for k in (0..span).step_by(8) {
                let Some(w) = r.peek(object + k) else { continue };
                // (an object embedded in this one: its vtable word)
                if k > 0 && (crate::entry::exe_base()..crate::entry::exe_base() + 0x1000_0000).contains(&w) {
                    if let Some(name) = rtti(&mut r, object + k).filter(|n| n.contains(text) && found < 60) {
                        found += 1;
                        erlog!("atk.find: {path}+{k:#x} (embedded) {name}");
                    }
                }
                if plausible(w) && seen.insert(w) && next.len() < 60_000 {
                    next.push((format!("{path}>{k:#x}"), w));
                }
            }
        }
        level = next;
        if started.elapsed().as_secs_f32() > 4.0 {
            erlog!("atk.find: cut short at depth {depth}");
            break;
        }
    }
    erlog!("atk.find: {found} object(s) named {text}, {:.2} s", started.elapsed().as_secs_f32());
}

/// Every 8-byte aligned place in read-write memory holding one of `values`.
fn holders(values: &[usize]) -> Vec<(usize, usize)> {
    use windows_sys::Win32::System::Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS, PAGE_READWRITE};
    let started = std::time::Instant::now();
    let mut at = 0x10000usize;
    let mut found = Vec::new();
    while at < 0x7FFF_0000_0000 && started.elapsed().as_secs_f32() < 20.0 {
        // SAFETY: VirtualQuery only describes the address.
        let info = unsafe {
            let mut info: MEMORY_BASIC_INFORMATION = std::mem::zeroed();
            if VirtualQuery(at as *const _, &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) == 0 {
                break;
            }
            info
        };
        let (base, size) = (info.BaseAddress as usize, info.RegionSize);
        if info.State == MEM_COMMIT && info.Protect & (PAGE_GUARD | PAGE_NOACCESS) == 0 && info.Protect == PAGE_READWRITE {
            for p in (base..base + size).step_by(8) {
                // SAFETY: inside a committed read-write region.
                let w = unsafe { std::ptr::read_unaligned(p as *const usize) };
                if values.contains(&w) {
                    found.push((p, w));
                }
            }
        }
        at = base + size;
    }
    erlog!("atk.refs: scanned in {:.2} s, {} place(s)", started.elapsed().as_secs_f32(), found.len());
    found
}

/// `atk.refs`: from the character to its AI's targeting owner, by what points at what.
fn refs(chr: &ChrIns) {
    let mut r = Reader::default();
    let me = chr as *const ChrIns as usize;
    let places = holders(&[me]);
    let mut owners = Vec::new();
    for (p, _) in &places {
        let (base, rest) = (p - 0x10, p - 8);
        if rtti(&mut r, base).is_some_and(|n| n.contains("CSAiTargetingSystemOwner")) {
            let ai = r.peek(rest).unwrap_or(0);
            erlog!("atk.refs: owner object at {base:#x}, ai {ai:#x} {}", rtti(&mut r, ai).unwrap_or_default());
            owners.push(base);
            owners.push(ai);
        }
    }
    erlog!("atk.refs: {} pointer(s) to the character; first: {}", places.len(), places.iter().take(40).map(|(p, _)| format!("{p:#x}")).collect::<Vec<_>>().join(" "));
    if owners.is_empty() {
        return;
    }
    for (p, v) in holders(&owners) {
        // what object holds it: the nearest vtable word within 0x4000 before it
        let mut name = String::new();
        for back in (0..0x4000usize).step_by(8) {
            if let Some(n) = rtti(&mut r, p - back) {
                name = format!("{} + {back:#x}", n);
                break;
            }
        }
        erlog!("atk.refs: {p:#x} holds {v:#x}: inside {name}");
    }
}
