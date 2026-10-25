//! (IMPL_ER31) BOSSES GIVE MAGIC, ABILITIES, SLOTS, MAX HP / MP; SMALL BOSSES GIVE AN ITEM (`boss.rewards`, default
//! ON; `boss.rewards = 0` switches it off). The user (2026-10-13): "also add magic unlocks to bosses. Make Margit give
//! you thunder, for example, and Godrick the grafted gives you heal"; "Glide should go to Maliketh"; "small bosses
//! should give meaningful item rewards, either buff items or equipment or accessories. Make sure you don't get too OP
//! too fast though". The table, the once-only record and the grant are `kh_host::bossrewards` (sheet
//! sheets/port/sora-er-boss-rewards.json, docs/sora-er-boss-rewards.md); this file is how Elden Ring says "that boss
//! is beaten". UNTESTED IN THE GAME: written by reading, the rules unit-tested through `Env`.
//!
//! HOW A NAMED BOSS IS RECOGNISED (either is enough; the record makes the second one a refusal):
//! 1. ITS DEFEAT EVENT FLAG (`CSEventFlagMan::virtual_memory_flag.get_flag`, the reader savepoint.rs and travel.rs
//!    use in the user's game), polled twice a second: a row whose flag is set and whose reward is not recorded is due.
//!    The flag is the game's own "this boss is dead for good": it is not set by a phase change, it tells Margit at
//!    Stormveil from his re-encounter outside Leyndell and from Morgott (three different flags), and it is the same
//!    in every session. EVERY FLAG ID OF THE SHEET IS FROM MEMORY (`guess`): the log verifies them (below).
//! 2. A CHR ID, for the rows that have one (Margit c2130 in map area 10, seen in the user's session; Godrick c4750,
//!    a guess): the boss finish's own confirmation (finish.rs: the last boss-bar character, dead by the game's word
//!    within `finish.hit_window` of Sora's hit and still dead `finish.confirm_frames` later) of a body of that model
//!    in that map area. It needs `finish.kh = 1` and a kill by Sora's own hit.
//!
//! WHEN IT IS GIVEN: when nothing else has the screen for 1.5 s: no boss finish running or pending, no cutscene, no
//! menu or pause, Sora alive. Then the reward seam (`Sora::grant`), the game's "BONUS!" panel(s) with its sound, the
//! record (`boss:<row id>` in the HUD's inventory, kept by Sora's save file at its next write).
//!
//! THE CATCH-UP: the first flag poll after Sora starts (1 s in) finds the rows whose flag is ALREADY set and not
//! recorded (an existing character): they are given together, 3 s in at the earliest, with ONE panel and one log
//! line per boss. When the game's constant flags 6000 / 6001 do not read off / on the flags are taken as NOT
//! readable: no flag rule, no catch-up (said in the log); `boss.grant` remains.
//!
//! `boss.grant = <row id>` (one-shot per change of value, never a value present at start): that row's reward by
//! hand (refused when recorded). `boss.grant = list`: every row with its flag's state and its record, in the log.
//!
//! SMALL BOSSES (every boss that is not a row): a boss-bar character whose death the boss finish confirmed and whom
//! no row claims within `small.wait` seconds (by chr id at once, or by a row's flag rising) gives ONE reward, once
//! per boss per character (`small:<NpcParam id>@<map block>`): an armour piece or an accessory of the item code's
//! tier model (lootmap.rs), its tier capped by the region and rising with the boss's maximum HP, or, every third
//! small boss, a +1 stat boost in turn. WHAT THIS TEST MISSES: a boss not killed by Sora's own hit, a boss whose
//! death starts a cutscene at once, any boss while `finish.kh = 0`, a boss the game gives no bar. WHAT IT MAY
//! WRONGLY INCLUDE: a named boss whose flag guess is wrong (it then gives an item instead of its row's reward:
//! the log says so, `boss.grant` gives the row), any non-boss the game shows a boss bar for.
//!
//! THE LOG (prefix `boss:`): the table's summary at start; every boss-bar death with its chr id, NpcParam id, max
//! HP and map block; every flag of the block's own range (`<map>0800` .. `<map>0899`) that BECOMES SET within 30 s
//! of it (this is what turns a `guess` into `verified`); every grant, refusal and held-back reward.

/// (XSI, 2026-10-16) The region depth of the player's map block (0 Limgrave .. 4 the DLC; 255 = not known yet), as the
/// boss rewards read it: kh-core's XS abilities ask for it through `ErTargets::foe_level` (Hidden Potential).
static REGION_DEPTH: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(255);

/// (XSI) The region depth, when known.
pub fn region_depth() -> Option<u8> {
    let d = REGION_DEPTH.load(std::sync::atomic::Ordering::Relaxed);
    (d != 255).then_some(d)
}

use std::collections::VecDeque;

use kh_core::Sora;
use kh_host::bossrewards::{self, Claim, SmallReward, Table};

use crate::control::{Control, OneShot};
use crate::erlog;
use crate::lootmap::LootRules;

/// Seconds without a finish / cutscene / menu before anything is given.
const CALM: f32 = 1.5;
/// Seconds after Sora's start before the first flag poll / before the catch-up is given.
const FIRST_POLL: f32 = 1.0;
const CATCH_UP_AFTER: f32 = 3.0;
const POLL_EVERY: f32 = 0.5;
/// Seconds the flags of a dead boss's map block are watched for one becoming set.
const WATCH: f32 = 30.0;

/// What one frame knows of the game (a test supplies its own).
pub struct Env<'a> {
    /// An event flag (`None`: the flag manager is not there).
    pub flag: &'a dyn Fn(u32) -> Option<bool>,
    /// The player's map block (`BlockId` as a number; 0: unknown).
    pub block: i32,
    /// A boss finish runs or is pending, a cutscene, a menu, the pause, Sora dead.
    pub busy: bool,
    pub dt: f32,
    /// The boss finish's confirmed boss-bar deaths of this frame: (chr model id, NpcParam id, max HP).
    pub deaths: &'a [(i32, i32, i32)],
    /// How many of a KH piece Sora holds.
    pub held: &'a dyn Fn(&str) -> u32,
    /// Hands Sora a KH item (the item popup, the stock). False: no such item.
    pub give: &'a mut dyn FnMut(u32) -> bool,
}

struct Candidate {
    key: String,
    npc_id: i32,
    max_hp: i32,
    depth: u8,
    left: f32,
}

struct Watch {
    who: String,
    base: u32,
    set: Vec<bool>,
    age: f32,
}

/// The first flag of the defeat-flag range of a map block (`<map>0800`): legacy maps mAA_BB -> AABB0800, the open
/// world m60_XX_ZZ -> 10XXZZ0800, the DLC's m61_XX_ZZ -> 20XXZZ0800.
pub fn flag_base(block: i32) -> Option<u32> {
    if block <= 0 {
        return None;
    }
    let (area, x, z) = ((block as u32 >> 24) & 0xFF, (block as u32 >> 16) & 0xFF, (block as u32 >> 8) & 0xFF);
    match area {
        60 => Some(1_000_000_000 + x * 1_000_000 + z * 10_000 + 800),
        61 => Some(2_000_000_000 + x * 1_000_000 + z * 10_000 + 800),
        1..=59 => Some(area * 1_000_000 + x * 10_000 + 800),
        _ => None,
    }
}

fn block_text(block: i32) -> String {
    let b = block as u32;
    format!("m{:02}_{:02}_{:02}_{:02}", (b >> 24) & 0xFF, (b >> 16) & 0xFF, (b >> 8) & 0xFF, b & 0xFF)
}

pub struct ErBossRewards {
    table: Table,
    loot: Option<LootRules>,
    told: bool,
    off_told: bool,
    age: f32,
    calm: f32,
    poll_in: f32,
    polled: bool,
    flags_ok: bool,
    seen: Vec<Option<bool>>,
    /// Seconds since a row's flag last rose (large: never).
    since_rise: f32,
    catch_up: Vec<String>,
    /// (XSI, 2026-10-16) the XS catch-up (kh_host::bossrewards::xs_catch_up) was made for this session's character
    xs_caught: bool,
    /// (row id, how it was recognised).
    due: Vec<(String, String)>,
    small: Vec<Candidate>,
    watch: Option<Watch>,
    depth: u8,
    grant: Option<OneShot>,
    /// The latest log lines (the tests read them).
    pub said: VecDeque<String>,
}

impl ErBossRewards {
    pub fn new(control: &Control) -> ErBossRewards {
        let mut out = ErBossRewards::with_table(Table::load());
        out.grant = Some(OneShot::new("boss.grant", control));
        out
    }

    pub fn with_table(table: Table) -> ErBossRewards {
        let seen = vec![None; table.rows.len()];
        ErBossRewards { table, loot: None, told: false, off_told: false, age: 0.0, calm: 0.0, poll_in: 0.0, polled: false, flags_ok: false, seen, since_rise: 1.0e6, catch_up: Vec::new(), xs_caught: false, due: Vec::new(), small: Vec::new(), watch: None, depth: 0, grant: None, said: VecDeque::new() }
    }

    fn say(&mut self, line: String) {
        erlog!("boss: {line}");
        if self.said.len() >= 256 {
            self.said.pop_front();
        }
        self.said.push_back(line);
    }

    /// ONCE A FRAME while Sora runs (game thread).
    pub fn frame(&mut self, control: &Control, sora: &mut Sora, block: i32, deaths: &[(i32, i32, i32)], busy: bool, dt: f32) {
        let on = control.flag("boss.rewards") != Some(false);
        let grant = self.grant.as_mut().and_then(|g| g.fired(control));
        let mut records = crate::hud::records();
        let before = records.len();
        let env = Env { flag: &game_flag, block, busy, dt, deaths, held: &kh_host::items::held, give: &mut |item_id| kh_host::items::give(item_id, 1) };
        self.frame_with(on, grant, &mut records, sora, env);
        // (ER party S6, U3) the friends' victory bonuses of the bosses this character has beaten (party rows)
        #[cfg(feature = "party")]
        party::frame(control, on && !busy, &mut records, sora);
        if records.len() != before {
            crate::hud::set_records(records);
        }
    }

    /// The frame with the game's side handed in.
    pub fn frame_with(&mut self, on: bool, grant: Option<String>, records: &mut Vec<String>, sora: &mut Sora, env: Env) {
        if !on {
            if !std::mem::replace(&mut self.off_told, true) {
                self.say("boss rewards are OFF (boss.rewards = 0): no boss gives anything; nothing is recorded".to_string());
            }
            (self.due.clear(), self.catch_up.clear(), self.small.clear());
            (self.polled, self.told, self.age) = (false, false, 0.0);
            return;
        }
        self.off_told = false;
        if !std::mem::replace(&mut self.told, true) {
            let small = self.table.small.clone();
            self.say(format!("THE TABLE (sheets/port/sora-er-boss-rewards.json): {}", self.table.summary()));
            self.say(format!("small bosses (every boss-bar character that is no row): {}", if small.on { format!("one item each, tier caps by region {:?}, HP steps {:?}, a stat boost every {} bosses, {} s for a row's flag to claim the boss", small.tier_cap, small.hp_steps, small.boost_every, small.wait) } else { "OFF (small.on = 0)".to_string() }));
        }
        self.age += env.dt;
        self.calm = if env.busy { 0.0 } else { self.calm + env.dt };
        self.since_rise += env.dt;
        let rules = self.loot.get_or_insert_with(LootRules::load);
        if let Some(depth) = rules.depth_of_block(env.block) {
            self.depth = depth;
            REGION_DEPTH.store(depth, std::sync::atomic::Ordering::Relaxed); // (XSI) Hidden Potential's region band
        }
        let area = ((env.block.max(0) as u32 >> 24) & 0xFF) as u8;

        // ---- the boss finish's confirmed boss-bar deaths: the chr rule, the small bosses, the flag watch
        for &(npc_id, npc_param, max_hp) in env.deaths {
            let row = self.table.by_chr(npc_id, area).map(|r| (r.id.clone(), r.name.clone()));
            let place = block_text(env.block);
            match row {
                Some((id, name)) => {
                    if bossrewards::recorded(records, &id) {
                        self.say(format!("A BOSS-BAR CHARACTER DIED: c{npc_id:04} (NpcParam {npc_param}, max HP {max_hp}) in {place} = {name} by its chr id: REFUSED, this character has had its reward (record boss:{id})"));
                    } else {
                        self.say(format!("A BOSS-BAR CHARACTER DIED: c{npc_id:04} (NpcParam {npc_param}, max HP {max_hp}) in {place} = {name} by its chr id: its reward is due"));
                        if !self.due.iter().any(|d| d.0 == id) {
                            self.due.push((id, format!("the confirmed death of c{npc_id:04} in map area {area}")));
                        }
                    }
                }
                None => {
                    let key = format!("{npc_param}@{:08x}", env.block);
                    let wait = self.table.small.wait;
                    let claimed_by_flag = self.since_rise <= wait;
                    self.say(format!(
                        "A BOSS-BAR CHARACTER DIED: c{npc_id:04} (NpcParam {npc_param}, max HP {max_hp}) in {place}, region depth {}: no row by chr id{}",
                        self.depth,
                        if claimed_by_flag { ": a row's defeat flag rose just before: it is that NAMED boss (no small-boss item)".to_string() } else if !self.table.small.on { String::new() } else if bossrewards::small_recorded(records, &key) { format!(": REFUSED as a small boss, this character has had its item (record small:{key})") } else { format!(": a SMALL BOSS (key {key}) unless a row's defeat flag claims it within {wait} s") }
                    ));
                    if !claimed_by_flag && self.table.small.on && !bossrewards::small_recorded(records, &key) && !self.small.iter().any(|c| c.key == key) {
                        self.small.push(Candidate { key, npc_id, max_hp, depth: self.depth, left: wait });
                    }
                }
            }
            if let Some(base) = flag_base(env.block) {
                let set = (0..100).map(|k| (env.flag)(base + k).unwrap_or(false)).collect();
                self.watch = Some(Watch { who: format!("c{npc_id:04} (NpcParam {npc_param}) in {place}"), base, set, age: 0.0 });
            }
        }

        // ---- the flags, twice a second
        self.poll_in -= env.dt;
        if self.age >= FIRST_POLL && self.poll_in <= 0.0 {
            self.poll_in = POLL_EVERY;
            if !self.polled {
                let (off, on) = ((env.flag)(6000), (env.flag)(6001));
                self.flags_ok = off == Some(false) && on == Some(true);
                if !self.flags_ok {
                    self.say(format!("THE EVENT FLAGS ARE NOT READABLE HERE (the game's constant flags 6000 / 6001 read {off:?} / {on:?}, wanted off / on): no boss is recognised by its defeat flag and THERE IS NO CATCH-UP for bosses beaten earlier; rows with a chr id still work, and `boss.grant = <row id>` gives a row by hand (`boss.grant = list` lists them)"));
                }
            }
            if self.flags_ok {
                let mut set_names = Vec::new();
                for k in 0..self.table.rows.len() {
                    let (id, name) = (self.table.rows[k].id.clone(), self.table.rows[k].name.clone());
                    let Some((flag, _)) = self.table.rows[k].flag else { continue };
                    let now = (env.flag)(flag).unwrap_or(false);
                    let before = std::mem::replace(&mut self.seen[k], Some(now));
                    if !now {
                        continue;
                    }
                    set_names.push(name.clone());
                    let rose = before == Some(false);
                    if rose {
                        // (the boss-bar death of these seconds is this named boss: no small-boss item for it)
                        self.since_rise = 0.0;
                        self.small.clear();
                    }
                    let waiting = self.due.iter().any(|d| d.0 == id) || self.catch_up.contains(&id);
                    if bossrewards::recorded(records, &id) {
                        if rose {
                            self.say(format!("the defeat flag {flag} of {name} BECAME SET: REFUSED, this character has had its reward (record boss:{id})"));
                        }
                    } else if !waiting {
                        if self.polled {
                            self.say(format!("DEFEATED: the defeat flag {flag} of {name} BECAME SET: its reward is due"));
                            self.due.push((id, format!("its defeat flag {flag}")));
                        } else {
                            self.catch_up.push(id);
                        }
                    }
                }
                if !self.polled {
                    let had = self.table.rows.iter().filter(|r| bossrewards::recorded(records, &r.id)).count();
                    self.say(format!(
                        "AT SORA'S START: {} of {} defeat flags are set ({}); {} reward(s) recorded for this character; {}",
                        set_names.len(),
                        self.table.rows.iter().filter(|r| r.flag.is_some()).count(),
                        if set_names.is_empty() { "none".to_string() } else { set_names.join("; ") },
                        had,
                        if self.catch_up.is_empty() { "nothing to catch up".to_string() } else { format!("CATCH-UP: {} boss(es) beaten before and not rewarded yet: {}", self.catch_up.len(), self.catch_up.join(", ")) }
                    ));
                }
            }
            self.polled = true;
            // the flags of a dead boss's map block: which one becomes set
            if let Some(mut watch) = self.watch.take() {
                watch.age += POLL_EVERY;
                for k in 0..watch.set.len() {
                    if !watch.set[k] && (env.flag)(watch.base + k as u32) == Some(true) {
                        watch.set[k] = true;
                        let flag = watch.base + k as u32;
                        let known = self.table.rows.iter().find(|r| r.flag.is_some_and(|(f, _)| f == flag)).map(|r| r.name.clone());
                        self.say(format!("flag {flag} BECAME SET {:.1} s after the death of {}: {}", watch.age, watch.who, known.map_or("no row has this flag (if this is a row's boss, its flag in the sheet is wrong: this is the number)".to_string(), |name| format!("the sheet's flag of {name}: VERIFIED by this line"))));
                    }
                }
                if watch.age < WATCH {
                    self.watch = Some(watch);
                }
            }
        }

        // ---- `boss.grant = <row id>` / `list`
        if let Some(value) = grant {
            let value = value.trim().to_string();
            if value == "list" {
                for k in 0..self.table.rows.len() {
                    let row = self.table.rows[k].clone();
                    let state = row.flag.map_or("no flag".to_string(), |(f, _)| format!("flag {f} {}", match (env.flag)(f) { Some(true) if self.flags_ok => "SET", Some(false) if self.flags_ok => "clear", _ => "not readable" }));
                    self.say(format!("row {} = {} ({}): {} | {} | {}", row.id, row.name, row.region, row.reward_text(), state, if bossrewards::recorded(records, &row.id) { "REWARDED" } else { "not rewarded" }));
                }
                self.say(format!("small bosses rewarded for this character: {}", bossrewards::small_count(records)));
            } else {
                match self.table.row(&value).map(|r| (r.id.clone(), r.name.clone())) {
                    None => self.say(format!("boss.grant = {value}: NO SUCH ROW (boss.grant = list lists the row ids)")),
                    Some((id, name)) if bossrewards::recorded(records, &id) => self.say(format!("boss.grant = {value}: REFUSED, this character has had the reward of {name} (record boss:{id})")),
                    Some((id, _)) => {
                        if !self.due.iter().any(|d| d.0 == id) {
                            self.catch_up.retain(|c| *c != id);
                            self.due.push((id, "the key boss.grant".to_string()));
                        }
                    }
                }
            }
        }

        // ---- the small bosses whose wait is over
        for c in &mut self.small {
            c.left -= env.dt;
        }
        if self.calm < CALM {
            return;
        }
        while let Some(at) = self.small.iter().position(|c| c.left <= 0.0) {
            let c = self.small.remove(at);
            let rules = self.table.small.clone();
            let Some(reward) = bossrewards::small_claim(&rules, records, sora, &c.key, c.depth, c.max_hp) else { continue };
            // (IT) the boost (KH3's Strength / Magic / Defense / AP Boost) goes to the whole roster, unlocked or not (the user,
            // 2026-10-17 00:50): the party host's one total; Sora's own bytes took it in small_claim
            #[cfg(all(windows, feature = "party"))]
            if let SmallReward::Boost(stat) = reward {
                let b = match stat {
                    kh_host::bossrewards::Stat::Strength => kh_party::boosts::Boost::Strength,
                    kh_host::bossrewards::Stat::Magic => kh_party::boosts::Boost::Magic,
                    kh_host::bossrewards::Stat::Defense => kh_party::boosts::Boost::Defense,
                    kh_host::bossrewards::Stat::Ap => kh_party::boosts::Boost::Ap(1),
                };
                crate::partyui::roster_boost(b, &format!("small boss {}", c.key));
            }
            let nth = bossrewards::small_count(records);
            let what = match reward {
                SmallReward::Boost(stat) => format!("a stat boost: {} = {} (applied at once; every {} small bosses, in turn)", stat.texts().0, stat.texts().1, rules.boost_every),
                SmallReward::Piece { armour, tier } => {
                    let loot = self.loot.get_or_insert_with(LootRules::load);
                    let seed = bossrewards::key_seed(&c.key);
                    let piece = loot.capped_piece(tier, armour, seed, env.held).or_else(|| loot.capped_piece(tier, !armour, seed, env.held));
                    match piece {
                        Some((item_id, key, got)) => format!("{} {key} of tier {got} (asked: tier {tier} = cap {} of region depth {}, max HP {}): {}", if kh_host::items::by_id(item_id).is_some_and(|i| i.class == kh_host::items::Class::Armour) { "the armour piece" } else { "the accessory" }, rules.tier(c.depth, i32::MAX), c.depth, c.max_hp, if (env.give)(item_id) { "given (the item popup)" } else { "NOT GIVEN: the item table has no such item" }),
                        None => {
                            let elixir = kh_host::items::by_key("ITEM_ELIXIR").map(|i| i.item_id);
                            format!("every piece up to tier {tier} is held {} times: an Elixir instead: {}", loot.dup_max, if elixir.is_some_and(|id| (env.give)(id)) { "given" } else { "NOT GIVEN" })
                        }
                    }
                }
                // (IMPL_ER61) every `small.item_every`th boss without a boost: a recovery item instead of a piece
                SmallReward::Item { tier } => {
                    let item = rules.item_key(tier).and_then(kh_host::items::by_key);
                    format!("a recovery item (every {} small bosses without a boost; row small.item_line, tier {tier}): {}", rules.item_every, match item {
                        Some(i) => format!("{} {}", i.key, if (env.give)(i.item_id) { "given (the item popup)" } else { "NOT GIVEN" }),
                        None => "NOT GIVEN: no such item".to_string(),
                    })
                }
            };
            self.say(format!("SMALL BOSS no. {nth} of this character: c{:04} (key {}): {what}; recorded small:{}", c.npc_id, c.key, c.key));
        }

        // ---- (XSI) the XS rewards of bosses this character was rewarded for before the XS rows existed: once, at the catch-up time
        if !self.xs_caught && self.age >= CATCH_UP_AFTER {
            self.xs_caught = true;
            let claimed = bossrewards::xs_catch_up(&self.table, records, sora);
            for c in &claimed {
                self.say(format!("XS REWARDS GIVEN LATE: {} (rewarded before the XS abilities; row {}): {}; recorded xs:{}", c.name, c.id, c.text(), c.id));
            }
            // (DON-PROG) the Donald overhaul's Sora magic of the extra bosses this character had beaten before it: once
            let claimed = bossrewards::sm_catch_up(&self.table, records, sora);
            for c in &claimed {
                self.say(format!("SORA'S OVERHAUL MAGIC GIVEN LATE: {} (rewarded before the Donald overhaul made it one of its element's bosses; row {}): {}; recorded sora_magic:{}", c.name, c.id, c.text(), c.id));
            }
        }
        // ---- the catch-up, then the rewards that fell due in play
        if !self.catch_up.is_empty() && self.age >= CATCH_UP_AFTER {
            let ids = std::mem::take(&mut self.catch_up);
            let claimed = bossrewards::claim_all(&self.table, records, sora, &ids);
            for c in &claimed {
                self.say(format!("CATCH-UP GRANT: {} (beaten before; row {}): {}; recorded boss:{}", c.name, c.id, c.text(), c.id));
            }
            let lines = bossrewards::summary_lines(&claimed);
            self.say(format!("CATCH-UP: {} boss(es) rewarded together, one panel{}", claimed.len(), if claimed.iter().flat_map(|c| &c.given).filter(|g| g.changed).count() > 2 { format!(" (\"{}\" / \"{}\")", lines[0], lines[1]) } else { String::new() }));
        }
        for (id, why) in std::mem::take(&mut self.due) {
            match bossrewards::claim(&self.table, records, sora, &id, true) {
                Claim::Given(c) => self.say(format!("GRANT: {} (row {}; recognised by {why}): {}; recorded boss:{} (kept by Sora's save file at its next write)", c.name, c.id, c.text(), c.id)),
                Claim::Recorded => self.say(format!("row {id} (recognised by {why}): REFUSED, this character has had its reward")),
                Claim::Unknown => self.say(format!("row {id}: no such row")),
            }
        }
    }
}

// --------------------------------------------------------------------------- (ER party S6) the friends' rewards

/// (ER party S6, U3: "yes, using a draft table the user can edit") THE FRIENDS' VICTORY BONUSES: rows
/// `party.<boss row id>` of sheets/port/sora-er-boss-rewards.json (a DRAFT: KH3's own friend rows of TresVBonusData in
/// KH3's order, spread over the base game's bosses; docs/sora-er-boss-rewards.md "The friends"). A friend IN THE PARTY
/// gets the rewards of every boss this character has beaten (record `boss:<id>`), once (record `party:<id>:<friend>`):
/// at once after the boss, or at his join for bosses beaten before (the user: an unlocked friend is "where they should
/// be"). Applied through the party's step (partyui.rs); the bonus panel names him. `party.rewards = 0`: none.
#[cfg(feature = "party")]
pub mod party {
    use kh_host::party::PartyChara;
    use kh_partyhost::FriendReward;

    use crate::control::Control;
    use crate::erlog;

    /// One boss's friend rewards.
    #[derive(Clone, Debug, PartialEq)]
    pub struct PartyRow {
        pub boss: String,
        pub name: String,
        pub rewards: Vec<(PartyChara, FriendReward)>,
    }

    /// The party rows of the sheet's text, and what was not understood.
    pub fn parse(text: &str) -> (Vec<PartyRow>, Vec<String>) {
        let mut rows = Vec::new();
        let mut problems = Vec::new();
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return (rows, vec!["the sheet is not JSON".to_string()]) };
        for row in json["rows"].as_array().map_or(&[][..], |r| r.as_slice()) {
            let (Some(id), value) = (row[0].as_str(), &row[1]) else { continue };
            let Some(boss) = id.strip_prefix("party.").filter(|b| !b.is_empty()) else { continue };
            let mut rewards = Vec::new();
            for c in PartyChara::FRIENDS {
                for token in value[c.name().to_ascii_lowercase()].as_array().map_or(&[][..], |a| a.as_slice()) {
                    match token.as_str().and_then(FriendReward::parse) {
                        Some(r) => rewards.push((c, r)),
                        None => problems.push(format!("{id}: {} reward {token} not understood (bonus:<ETresVictoryBonusKind> | ability:<game name> | kit:<kit ability> | don:<Donald's ability>)", c.name())),
                    }
                }
            }
            rows.push(PartyRow { boss: boss.to_string(), name: value["name"].as_str().unwrap_or(boss).to_string(), rewards });
        }
        (rows, problems)
    }

    pub fn record_key(boss: &str, c: PartyChara) -> String {
        format!("party:{boss}:{}", kh_host::formation::key_of(c))
    }

    /// THE RULE (pure): the rewards due to the friends `present`: rows of bosses beaten (`boss:<id>`) whose
    /// `party:<id>:<friend>` is not recorded yet.
    pub fn due<'a>(rows: &'a [PartyRow], records: &[String], present: &[PartyChara]) -> Vec<(&'a PartyRow, PartyChara)> {
        let mut out = Vec::new();
        for row in rows {
            if !kh_host::bossrewards::recorded(records, &row.boss) {
                continue;
            }
            for c in present.iter().copied().filter(|c| *c != PartyChara::Sora) {
                if row.rewards.iter().any(|r| r.0 == c) && !records.iter().any(|k| *k == record_key(&row.boss, c)) {
                    out.push((row, c));
                }
            }
        }
        out
    }

    /// The bonus panel's words for a reward ("Max HP +10", "Fire Boost").
    pub fn label(r: &FriendReward) -> String {
        match r {
            FriendReward::Ability(k) => kh_core::status::tables::ABILITIES.iter().find(|row| row.kind == *k).map_or_else(|| k.name().to_string(), |row| row.name.to_string()),
            FriendReward::Kit(_) => r.label(), // (party X3D) a kit ability: its own name ("Aero Float")
            FriendReward::Don(_) => r.label(), // (DON-PROG) one of Donald's overhaul abilities ("Aero Float")
            FriendReward::Bonus(_) => {
                let raw = r.label();
                let n = raw.rsplit("UP").next().and_then(|n| n.parse::<i32>().ok()).unwrap_or(1);
                match raw.split('_').next().unwrap_or("") {
                    "HP" => format!("Max HP +{n}"),
                    "MP" => format!("Max MP +{n}"),
                    "ITEM" => "Item Slot +1".to_string(),
                    "DEF" => "Armor Slot +1".to_string(),
                    "ACC" => "Accessory Slot +1".to_string(),
                    _ => raw,
                }
            }
        }
    }

    /// (BN) THE BONUS! PANEL'S LINE OF A FRIEND'S REWARD AS KH3 WRITES IT (0x141596150): an ability (KH3's or his kit's)
    /// its name after the ability sparkle; a max HP / MP bonus or a slot KH3's own stat line, no icon ("Maximum HP
    /// increased!", "Item slot unlocked!").
    pub fn panel_line(r: &FriendReward) -> String {
        use kh_ui::hud_side::{icon_line, LineIcon};
        match r {
            FriendReward::Ability(_) | FriendReward::Kit(_) | FriendReward::Don(_) /* (DON-PROG) */ => icon_line(LineIcon::Ability, &label(r)),
            FriendReward::Bonus(_) => {
                let side = kh_ui::hud_side::SideStrings::default();
                let raw = r.label();
                match raw.split('_').next().unwrap_or("") {
                    "HP" => side.bonus_lines[0].clone(),
                    "MP" => side.bonus_lines[1].clone(),
                    "DEF" => side.bonus_lines[2].clone(),
                    "ACC" => side.bonus_lines[3].clone(),
                    "ITEM" => side.bonus_lines[4].clone(),
                    _ => label(r),
                }
            }
        }
    }

    fn rows() -> &'static (Vec<PartyRow>, Vec<String>) {
        static ROWS: std::sync::OnceLock<(Vec<PartyRow>, Vec<String>)> = std::sync::OnceLock::new();
        ROWS.get_or_init(|| {
            let path = kh_gendata::project_file("sheets/port/sora-er-boss-rewards.json");
            let read = std::fs::read_to_string(path).ok().map(|t| parse(&t)).filter(|(r, _)| !r.is_empty());
            let out = read.unwrap_or_else(|| parse(kh_host::bossrewards::SHEET));
            erlog!("boss: the friends' reward rows (party.*): {} boss(es), {} reward(s){}", out.0.len(), out.0.iter().map(|r| r.rewards.len()).sum::<usize>(), if out.1.is_empty() { String::new() } else { format!(" | {} PROBLEM(S): {}", out.1.len(), out.1.join("; ")) });
            out
        })
    }

    /// Once a frame (`ok`: the rewards are on and nothing else has the screen).
    pub fn frame(control: &Control, ok: bool, records: &mut Vec<String>, sora: &mut kh_core::Sora) {
        if !ok || control.flag("party.rewards") == Some(false) {
            return;
        }
        let present = crate::partyui::present(control);
        if present.is_empty() {
            return;
        }
        let rows = &rows().0;
        for (row, c) in due(rows, records, &present) {
            let mine: Vec<&FriendReward> = row.rewards.iter().filter(|r| r.0 == c).map(|r| &r.1).collect();
            for r in &mine {
                crate::partyui::give_friend_reward(c, (*r).clone(), &format!("{} beaten (row party.{})", row.name, row.boss));
            }
            records.push(record_key(&row.boss, c));
            let words = mine.iter().map(|r| label(r)).collect::<Vec<_>>().join(", ");
            erlog!("boss: {}'s reward for {} (row party.{}): {words}; recorded {}", c.name(), row.name, row.boss, record_key(&row.boss, c));
            // (BN) HIS OWN BONUS! panel(s) (his roster row's colour and portrait), KH3's lines, two a panel; not Sora's panel
            // with his name as its first line
            let _ = &sora;
            crate::partyui::friend_bonus(c, mine.iter().map(|r| panel_line(r)).collect());
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_sheets_party_rows_are_all_understood() {
            let (rows, problems) = parse(kh_host::bossrewards::SHEET);
            assert!(problems.is_empty(), "{problems:?}");
            assert!(rows.len() >= 20, "{}", rows.len());
            // (party X3D) the playable kits' port abilities (`kit:` tokens) are counted apart from KH3's rows
            let count = |c: PartyChara| rows.iter().flat_map(|r| &r.rewards).filter(|r| r.0 == c && !matches!(r.1, FriendReward::Kit(_) | FriendReward::Don(_))).count(); // (DON-PROG) + Donald's overhaul abilities
            // KH3's own friend rows: Donald 13 rows (16 rewards), Goofy 14 rows (15 rewards)
            assert_eq!((count(PartyChara::Donald), count(PartyChara::Goofy)), (16, 15));
            // (party X3D) Red Wolf of Radagon gives Donald Aero Float (XD decision 7); (DON-PROG) his overhaul ability now
            let red = rows.iter().find(|r| r.boss == "red_wolf").expect("party.red_wolf");
            assert_eq!(red.rewards, vec![(PartyChara::Donald, FriendReward::Don(kh_party::donald::DonAbility::AeroFloat))]);
            let table = kh_host::bossrewards::Table::parse(kh_host::bossrewards::SHEET);
            assert!(table.problems.is_empty(), "{:?}", table.problems);
            for r in &rows {
                assert!(table.row(&r.boss).is_some(), "party.{} names no boss row", r.boss);
            }
        }

        #[test]
        fn a_friend_gets_a_beaten_bosss_reward_once_and_a_late_joiner_catches_up() {
            let (rows, _) = parse(kh_host::bossrewards::SHEET);
            let mut records = vec!["boss:tree_sentinel".to_string(), "boss:leonine".to_string()];
            let first = due(&rows, &records, &[PartyChara::Donald]);
            assert_eq!(first.iter().map(|(r, c)| (r.boss.as_str(), *c)).collect::<Vec<_>>(), vec![("tree_sentinel", PartyChara::Donald), ("leonine", PartyChara::Donald)]);
            for (r, c) in &first {
                records.push(record_key(&r.boss, *c));
            }
            assert!(due(&rows, &records, &[PartyChara::Donald]).is_empty(), "once");
            // Goofy joins later (unlocked by Leonine): both bosses' rewards for him now
            let late = due(&rows, &records, &[PartyChara::Donald, PartyChara::Goofy]);
            assert_eq!(late.iter().map(|(r, c)| (r.boss.as_str(), *c)).collect::<Vec<_>>(), vec![("tree_sentinel", PartyChara::Goofy), ("leonine", PartyChara::Goofy)]);
            assert!(due(&rows, &records, &[PartyChara::Sora]).is_empty(), "Sora has his own rows");
            assert_eq!(label(&FriendReward::parse("bonus:HP_UP10").unwrap()), "Max HP +10");
            assert_eq!(label(&FriendReward::parse("bonus:ITEM_SLOT_UP1").unwrap()), "Item Slot +1");
        }

        /// (BN) A friend's BONUS! lines as KH3 writes them: abilities (KH3's and his kit's) after the sparkle; max HP / MP
        /// and slots KH3's own stat lines with no icon. Roster boosts: the boost item after the camp-item icon + the
        /// stat line; an Elden Ring item's permanent stat the KH3 line alone.
        #[test]
        fn bn_friend_panel_lines_carry_kh3_icons() {
            let p = |t: &str| panel_line(&FriendReward::parse(t).unwrap());
            assert_eq!(p("ability:FIRE_UP"), "<icon(Ability)>Fire Boost");
            assert!(p("kit:AERO_FLOAT").starts_with("<icon(Ability)>"), "{}", p("kit:AERO_FLOAT"));
            assert_eq!(p("don:AERO_FLOAT"), "<icon(Ability)>Aero Float"); // (DON-PROG)
            assert_eq!(p("bonus:HP_UP10"), "Maximum HP increased!");
            assert_eq!(p("bonus:ITEM_SLOT_UP1"), "Item slot unlocked!");
            assert_eq!(p("bonus:ACC_SLOT_UP1"), "Accessory slot unlocked!");
            assert_eq!(p("bonus:DEF_SLOT_UP1"), "Armor slot unlocked!");
            use kh_party::boosts::Boost;
            assert_eq!(crate::partyui::boost_panel(Boost::Strength, "small boss x"), vec!["<icon(CampItem)>Strength Boost".to_string(), "Strength +1".to_string()]);
            assert_eq!(crate::partyui::boost_panel(Boost::Ap(1), "the Elden Ring item's reward ap:1"), vec!["Maximum AP increased!".to_string()]);
            assert_eq!(crate::partyui::boost_panel(Boost::Bonus(kh_core::status::BonusCounts::MP_UP5), "a Memory Stone"), vec!["Maximum MP increased!".to_string()]);
        }
    }
}

fn game_flag(id: u32) -> Option<bool> {
    use eldenring::cs::CSEventFlagMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game; the flag blocks are the game's own tables
    // (the reader travel.rs and savepoint.rs use).
    let man = unsafe { CSEventFlagMan::instance() }.ok()?;
    (!man.virtual_memory_flag.flag_blocks.is_null()).then(|| man.virtual_memory_flag.get_flag(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::magic::spells::Element;
    use kh_core::status::Status;
    use std::cell::RefCell;
    use std::collections::HashSet;

    const DT: f32 = 1.0 / 60.0;
    const STORMVEIL: i32 = 10 << 24;
    const LIMGRAVE: i32 = (60 << 24) | (42 << 16) | (36 << 8);

    fn new_sora() -> Sora {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sheets/port/sora-loadout-99.json");
        let base = kh_host::rig::loadout::parse(path, &std::fs::read_to_string(path).unwrap()).unwrap().0;
        let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: glam::Vec3::ZERO, yaw: 0.0 }).with_status(Status::from_loadout(&kh_host::save::new_game_loadout(&base, None)));
        kh_core::command::customize::ensure(&mut sora);
        sora
    }

    /// A game: the flags that are set, what was handed over.
    struct Game {
        flags: RefCell<HashSet<u32>>,
        readable: bool,
        given: RefCell<Vec<u32>>,
        rewards: ErBossRewards,
        records: Vec<String>,
        sora: Sora,
    }

    impl Game {
        fn new(set: &[u32]) -> Game {
            let mut flags: HashSet<u32> = set.iter().copied().collect();
            flags.insert(6001);
            Game { flags: RefCell::new(flags), readable: true, given: RefCell::new(Vec::new()), rewards: ErBossRewards::with_table(Table::parse(bossrewards::SHEET)), records: Vec::new(), sora: new_sora() }
        }

        fn run(&mut self, seconds: f32, block: i32, busy: bool, deaths: &[(i32, i32, i32)], grant: Option<&str>) {
            let frames = (seconds / DT).round().max(1.0) as u32;
            for k in 0..frames {
                let (flags, readable, given) = (&self.flags, self.readable, &self.given);
                let env = Env { flag: &|id| readable.then(|| flags.borrow().contains(&id)), block, busy, dt: DT, deaths: if k == 0 { deaths } else { &[] }, held: &|_| 0, give: &mut |id| { given.borrow_mut().push(id); true } };
                self.rewards.frame_with(true, grant.filter(|_| k == 0).map(str::to_string), &mut self.records, &mut self.sora, env);
            }
        }

        fn said(&self, what: &str) -> usize {
            self.rewards.said.iter().filter(|l| l.contains(what)).count()
        }

        fn thunder(&self) -> u32 {
            self.sora.status().magic_tier(Element::Thunder)
        }
    }

    #[test]
    fn the_flag_ranges_of_the_map_blocks() {
        assert_eq!(flag_base(STORMVEIL), Some(10000800));
        assert_eq!(flag_base((14 << 24) | 0), Some(14000800));
        assert_eq!(flag_base((39 << 24) | (20 << 16)), Some(39200800));
        assert_eq!(flag_base(LIMGRAVE), Some(1042360800));
        assert_eq!(flag_base((61 << 24) | (48 << 16) | (44 << 8)), Some(2048440800));
        assert_eq!((flag_base(0), flag_base(-1)), (None, None));
        assert_eq!(block_text(LIMGRAVE), "m60_42_36_00");
    }

    /// KILL MARGIT: the chr rule at the confirmed death, held while the finish runs, given once; the flag that rises
    /// afterwards, the next session and the re-encounter give nothing more.
    #[test]
    fn margit_gives_thunder_once_after_the_finish() {
        let mut game = Game::new(&[]);
        game.run(5.0, STORMVEIL, false, &[], None);
        assert_eq!(game.said("AT SORA'S START: 0 of 54 defeat flags are set"), 1);
        // the confirmed death; the boss finish runs (busy): nothing yet
        game.run(4.0, STORMVEIL, true, &[(2130, 21300014, 4174)], None);
        assert_eq!((game.thunder(), game.records.len(), game.said("= Margit, the Fell Omen by its chr id: its reward is due")), (0, 0, 1));
        // the game sets its flag meanwhile: not a second reward
        game.flags.borrow_mut().insert(10000850);
        game.run(1.0, STORMVEIL, true, &[], None);
        assert_eq!(game.said("flag 10000850 BECAME SET"), 1, "{:?}", game.rewards.said);
        assert!(game.rewards.said.iter().any(|l| l.contains("flag 10000850 BECAME SET") && l.contains("VERIFIED")));
        // the finish is over: 1.5 s of calm, then Thunder and MP Safety
        game.run(1.0, STORMVEIL, false, &[], None);
        assert_eq!(game.thunder(), 0);
        game.run(1.0, STORMVEIL, false, &[], None);
        assert_eq!((game.thunder(), game.records.clone()), (1, vec!["boss:margit".to_string()]));
        assert_eq!(game.said("GRANT: Margit, the Fell Omen"), 1);
        assert!(game.sora.command().magic_slots.contains(&Element::Thunder.command(1)));
        game.run(10.0, STORMVEIL, false, &[], None);
        assert_eq!((game.thunder(), game.records.len(), game.given.borrow().len()), (1, 1, 0), "no small-boss item for a named boss");
        // THE NEXT SESSION (the record is in the save, the flag is set): nothing
        let (records, flags, sora) = (game.records.clone(), game.flags.borrow().clone(), std::mem::replace(&mut game.sora, new_sora()));
        let mut next = Game::new(&[]);
        (next.records, next.sora) = (records, sora);
        *next.flags.borrow_mut() = flags;
        next.run(8.0, STORMVEIL, false, &[], None);
        assert_eq!((next.thunder(), next.records.len(), next.said("nothing to catch up")), (1, 1, 1));
        // the re-encounter outside Leyndell (the same model, the open world) and Morgott's arena (map area 11): the
        // chr rule does not match; Morgott's own flag gives Morgott's own row
        next.run(20.0, (60 << 24) | (43 << 16) | (53 << 8), false, &[(2130, 21300534, 4000)], None);
        assert_eq!(next.thunder(), 1);
        assert_eq!(next.given.borrow().len(), 1, "an unnamed boss-bar death is a small boss: one item");
        next.flags.borrow_mut().insert(11000800);
        next.run(4.0, 11 << 24, false, &[(2130, 21300000, 10000)], None);
        assert_eq!((next.thunder(), next.said("GRANT: Morgott, the Omen King"), next.given.borrow().len()), (1, 1, 1), "{:?}", next.rewards.said);
    }

    /// A boss identified by its flag only (no chr id in the sheet): the flag rising is the defeat.
    #[test]
    fn a_flag_that_rises_is_a_defeat_and_claims_the_boss_bar_death() {
        let mut game = Game::new(&[]);
        game.run(3.0, 14 << 24, false, &[], None);
        // Rennala's body dies (an unknown chr id), the finish runs, the flag rises 3 s later
        game.run(3.0, 14 << 24, true, &[(2030, 20300000, 4097)], None);
        assert_eq!(game.said("a SMALL BOSS (key 20300000@0e000000) unless"), 1);
        game.flags.borrow_mut().insert(14000800);
        game.run(4.0, 14 << 24, true, &[], None);
        assert_eq!(game.said("DEFEATED: the defeat flag 14000800 of Rennala, Queen of the Full Moon BECAME SET"), 1);
        assert_eq!(game.sora.status().magic_tier(Element::Blizzard), 0, "held while the finish runs");
        game.run(20.0, 14 << 24, false, &[], None);
        assert_eq!((game.sora.status().magic_tier(Element::Blizzard), game.records.clone(), game.given.borrow().len()), (1, vec!["boss:rennala".to_string()], 0));
        // not during a cutscene / menu either: Godrick's flag while busy for a long time
        game.flags.borrow_mut().insert(10000800);
        game.run(30.0, STORMVEIL, true, &[], None);
        assert_eq!(game.sora.status().magic_tier(Element::Cure), 0);
        game.run(2.0, STORMVEIL, false, &[], None);
        assert_eq!(game.sora.status().magic_tier(Element::Cure), 1);
    }

    /// (XSI, 2026-10-16) A CHARACTER REWARDED BEFORE THE XS ROWS: at the catch-up time the XS abilities of the bosses his record
    /// already has are given once (record xs:<row>); a second session gives nothing.
    #[test]
    fn xsi_the_xs_catch_up_gives_the_xs_rewards_of_bosses_rewarded_before() {
        let mut game = Game::new(&[1042360800]);
        game.records.push("boss:tree_sentinel".to_string());
        game.run(2.0, LIMGRAVE, false, &[], None);
        assert_eq!(game.sora.status().abilities.owned(kh_core::enums::ETresAbilityKind::SpiralRush), 0, "not before 3 s");
        game.run(2.0, LIMGRAVE, false, &[], None);
        assert_eq!(game.sora.status().abilities.owned(kh_core::enums::ETresAbilityKind::SpiralRush), 1);
        assert_eq!(game.said("XS REWARDS GIVEN LATE: Tree Sentinel"), 1, "{:?}", game.rewards.said);
        assert!(game.records.contains(&"xs:tree_sentinel".to_string()));
        // the next session of the same character: nothing again
        let records = game.records.clone();
        let mut next = Game::new(&[1042360800]);
        next.records = records;
        next.sora.status_mut().grant_ability(kh_core::enums::ETresAbilityKind::SpiralRush, false);
        next.run(5.0, LIMGRAVE, false, &[], None);
        assert_eq!((next.said("XS REWARDS GIVEN LATE"), next.sora.status().abilities.owned(kh_core::enums::ETresAbilityKind::SpiralRush)), (0, 1));
    }

    /// (DON-PROG) AN OLD SAVE: a DLC boss beaten before the Donald overhaul made it its element's -za boss (shared with Sora):
    /// at the catch-up time Sora gets his share once (record sora_magic:<row>); the next session nothing.
    #[test]
    fn don_prog_an_old_save_gets_soras_overhaul_magic_once() {
        // the Dancing Lion (Aero's DLC boss) beaten before the overhaul: Aero for Sora at the catch-up time, once
        let mut game = Game::new(&[20000800]);
        game.records.push("boss:dancing_lion".to_string());
        game.run(2.0, LIMGRAVE, false, &[], None);
        assert_eq!(game.sora.status().magic_tier(Element::Aero), 0, "not before 3 s");
        game.run(2.0, LIMGRAVE, false, &[], None);
        assert_eq!(game.sora.status().magic_tier(Element::Aero), 1);
        assert_eq!(game.said("SORA'S OVERHAUL MAGIC GIVEN LATE: Divine Beast Dancing Lion"), 1, "{:?}", game.rewards.said);
        assert!(game.records.contains(&"sora_magic:dancing_lion".to_string()));
        let (records, sora) = (game.records.clone(), std::mem::replace(&mut game.sora, new_sora()));
        let mut next = Game::new(&[20000800]);
        (next.records, next.sora) = (records, sora);
        next.run(5.0, LIMGRAVE, false, &[], None);
        assert_eq!((next.said("SORA'S OVERHAUL MAGIC GIVEN LATE"), next.sora.status().magic_tier(Element::Aero)), (0, 1));
    }

    /// AN EXISTING CHARACTER: the flags already set at Sora's start are caught up once, together.
    #[test]
    fn the_catch_up_at_start() {
        let mut game = Game::new(&[10000850, 10000800, 1042360800]);
        game.records.push("boss:tree_sentinel".to_string());
        game.run(2.0, LIMGRAVE, false, &[], None);
        assert_eq!(game.said("AT SORA'S START: 3 of 54 defeat flags are set"), 1);
        assert_eq!(game.said("CATCH-UP: 2 boss(es) beaten before and not rewarded yet: margit, godrick"), 1, "{:?}", game.rewards.said);
        assert_eq!(game.thunder(), 0, "not before 3 s");
        game.run(2.0, LIMGRAVE, false, &[], None);
        // (XSI) + xs:tree_sentinel: the Tree Sentinel's XS reward (Spiral Rush), given late to the record boss:tree_sentinel
        assert_eq!((game.thunder(), game.sora.status().magic_tier(Element::Cure), game.records.len()), (1, 1, 4));
        assert_eq!((game.said("CATCH-UP GRANT:"), game.said("CATCH-UP: 2 boss(es) rewarded together, one panel (\"Rewards of 2 bosses obtained\" / \"Thunder, Cure +3 more\")")), (2, 1), "{:?}", game.rewards.said);
        game.run(10.0, LIMGRAVE, false, &[], None);
        assert_eq!((game.records.len(), game.said("GRANT:")), (4, 2)); // (XSI) xs:tree_sentinel
        // FLAGS NOT READABLE: said plainly, no catch-up; the key gives a row by hand
        let mut blind = Game::new(&[10000850]);
        blind.readable = false;
        blind.run(6.0, LIMGRAVE, false, &[], None);
        assert_eq!((blind.said("THE EVENT FLAGS ARE NOT READABLE HERE"), blind.thunder()), (1, 0));
        blind.run(3.0, LIMGRAVE, false, &[], Some("margit"));
        assert_eq!((blind.thunder(), blind.said("recognised by the key boss.grant")), (1, 1));
        blind.run(3.0, LIMGRAVE, false, &[], Some("margit"));
        assert_eq!((blind.thunder(), blind.said("boss.grant = margit: REFUSED")), (1, 1));
        blind.run(1.0, LIMGRAVE, false, &[], Some("nobody"));
        blind.run(1.0, LIMGRAVE, false, &[], Some("list"));
        assert_eq!((blind.said("NO SUCH ROW"), blind.said("row margit = Margit, the Fell Omen") , blind.said("| REWARDED")), (1, 1, 1));
    }

    /// (DON-PROG) DONALD'S STEPS OF THE BOSSES THROUGH THE SAME RECORDS: an existing character's beaten bosses (the catch-up
    /// at Sora's start) and a boss beaten in play give the roster's Donald his steps once (records don:<row>, the party
    /// host's rule kh_partyhost::donald::don_claim_due, run by party/donaggro.rs); an older save (no donald_magic) gets
    /// every beaten boss's steps once; a second session nothing.
    #[cfg(feature = "party")]
    #[test]
    fn don_prog_donalds_steps_ride_the_boss_records_once() {
        use kh_party::donald::{DonLearned, Element};
        let table = Table::parse(bossrewards::SHEET);
        let mut game = Game::new(&[10000850, 10000800]);
        game.run(5.0, LIMGRAVE, false, &[], None);
        assert_eq!(game.records, vec!["boss:margit".to_string(), "boss:godrick".to_string()]);
        // the party host of an older save (no donald_magic: the start, flagged)
        let mut rt = kh_partyhost::PartyRuntime::new(kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        rt.set_progression(true);
        rt.load_party_record(&serde_json::Value::Null);
        assert!(rt.don_unrecorded && rt.don_learned() == DonLearned::start());
        let got = rt.don_claim_due(&table.don, &mut game.records);
        assert_eq!(got.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["margit", "godrick"]);
        assert_eq!((rt.don_learned().tier(Element::Thunder), rt.don_learned().tier(Element::Light)), (1, 1));
        assert!(game.records.contains(&"don:margit".to_string()) && game.records.contains(&"don:godrick".to_string()));
        // Rennala in play (its flag rises): Sora's Blizzard and then Donald's
        game.flags.borrow_mut().insert(14000800);
        game.run(4.0, 14 << 24, false, &[], None);
        let got = rt.don_claim_due(&table.don, &mut game.records);
        assert_eq!(got.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["rennala"]);
        assert_eq!((game.sora.status().magic_tier(Element::Blizzard.to_sora()), rt.don_learned().tier(Element::Blizzard)), (1, 1));
        // the next session: the records hold don:, nothing again
        assert!(rt.don_claim_due(&table.don, &mut game.records).is_empty());
    }

    /// SMALL BOSSES: one item per boss, once; the tier capped by the region; a boost every third.
    #[test]
    fn a_small_boss_gives_one_item_once() {
        // (IT) its Strength Boost also goes to the party's process-wide mailbox (partyui::roster_boost): one at a time with
        // the tests that read it
        #[cfg(all(windows, feature = "party"))]
        let _serial = crate::partyui::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut game = Game::new(&[]);
        game.run(3.0, LIMGRAVE, false, &[], None);
        // a Limgrave cave boss: the finish runs, then the wait (12 s) for a row's flag, then the item
        game.run(6.0, LIMGRAVE, true, &[(3000, 30000010, 900)], None);
        game.run(5.0, LIMGRAVE, false, &[], None);
        assert_eq!(game.given.borrow().len(), 0, "the wait for a row's flag is not over");
        game.run(3.0, LIMGRAVE, false, &[], None);
        assert_eq!((game.given.borrow().len(), game.records.clone()), (1, vec![format!("small:30000010@{LIMGRAVE:08x}")]));
        let loot = LootRules::load();
        let tier = |id: u32| loot.ranked().iter().find(|r| r.item_id == id).map(|r| r.tier);
        assert_eq!(tier(game.given.borrow()[0]), Some(1), "a 900-HP boss in Limgrave: tier 1");
        // the same boss again (it cannot die twice, but a second confirmed death must not pay twice)
        game.run(15.0, LIMGRAVE, false, &[(3000, 30000010, 900)], None);
        assert_eq!((game.given.borrow().len(), game.said("REFUSED as a small boss")), (1, 1));
        // a 40000-HP body in Limgrave is still capped at tier 2; (IMPL_ER61) the second small boss gives the recovery item of
        // that tier (small.item_every 2, small.item_line: an Elixir) instead of a piece; the third is a Strength Boost, no item
        game.run(15.0, LIMGRAVE, false, &[(4500, 45000010, 40000)], None);
        let elixir = kh_host::items::by_key("ITEM_ELIXIR").unwrap().item_id;
        assert_eq!((game.given.borrow().len(), game.given.borrow()[1]), (2, elixir));
        assert_eq!(game.said("a recovery item"), 1);
        let strength = game.sora.status().boosts.strength;
        game.run(15.0, LIMGRAVE, false, &[(3010, 30100010, 900)], None);
        assert_eq!((game.given.borrow().len(), game.sora.status().boosts.strength - strength, game.records.len()), (2, 1, 3));
        assert_eq!(game.said("SMALL BOSS no. 3 of this character"), 1);
    }

    /// `boss.rewards` is ON without the key; `boss.grant` acts once per change and never for a value that is in
    /// the file when Sora starts.
    #[test]
    fn the_keys() {
        let path = std::env::temp_dir().join(format!("kh-er-bossgrant-{}.ctl", std::process::id()));
        std::fs::write(&path, "boss.grant = margit\n").unwrap();
        let mut control = Control::new(path.clone());
        control.poll();
        assert_eq!(control.flag("boss.rewards"), Some(true), "the shipped default");
        let mut shot = OneShot::new("boss.grant", &control);
        assert_eq!(shot.fired(&control), None, "a value present at start is not carried out");
        let mut change = |text: &str| {
            for _ in 0..200 {
                std::thread::sleep(std::time::Duration::from_millis(15));
                std::fs::write(&path, text).unwrap();
                if control.poll() {
                    break;
                }
            }
            let fired = shot.fired(&control);
            (fired, shot.fired(&control), control.flag("boss.rewards"))
        };
        assert_eq!(change("boss.grant = godrick\n"), (Some("godrick".to_string()), None, Some(true)));
        assert_eq!(change("boss.grant = godrick\nboss.rewards = 0\n"), (None, None, Some(false)));
        assert_eq!(change("boss.rewards = 0\n"), (None, None, Some(false)), "the key removed: nothing");
        let _ = std::fs::remove_file(&path);
        // switched off: nothing is recognised, given or recorded, whatever the flags say
        let mut game = Game::new(&[10000850]);
        for _ in 0..600 {
            let flags = &game.flags;
            let env = Env { flag: &|id| Some(flags.borrow().contains(&id)), block: STORMVEIL, busy: false, dt: DT, deaths: &[(2130, 21300014, 4174)], held: &|_| 0, give: &mut |_| true };
            game.rewards.frame_with(false, Some("margit".to_string()), &mut game.records, &mut game.sora, env);
        }
        assert_eq!((game.thunder(), game.records.len(), game.said("boss rewards are OFF")), (0, 0, 1));
    }

    #[test]
    fn capped_pieces_never_exceed_the_cap_and_step_past_held_ones() {
        let loot = LootRules::load();
        for tier in 1..=6u8 {
            for armour in [false, true] {
                for seed in [0u32, 7, 12345, 0xDEAD_BEEF] {
                    if let Some((id, key, got)) = loot.capped_piece(tier, armour, seed, &|_| 0) {
                        let ranked = loot.ranked().iter().find(|r| r.item_id == id).unwrap();
                        assert!(got <= tier && ranked.tier == got && ranked.armour == armour, "{key}");
                        // deterministic; held twice: another piece, still within the cap
                        assert_eq!(loot.capped_piece(tier, armour, seed, &|_| 0).unwrap().1, key);
                        if let Some((_, other, got)) = loot.capped_piece(tier, armour, seed, &|k| if k == key { loot.dup_max } else { 0 }) {
                            assert!(other != key && got <= tier);
                        }
                    }
                }
            }
        }
        assert!(loot.capped_piece(2, false, 1, &|_| 0).is_some() && loot.capped_piece(2, true, 1, &|_| 0).is_some());
        assert_eq!(loot.capped_piece(6, false, 1, &|_| 99), None);
    }
}
