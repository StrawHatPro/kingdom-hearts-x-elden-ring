//! (ER party S6) THE PARTY'S SCREENS, SAVE AND PROGRESSION IN ELDEN RING: the HUD's party slots, the leader in the
//! main slot (KH-style shrink and grow: kh-host `Hud::set_leader`), the friends' level-up windows, the leader's command
//! menu, the camp menu's party pages and their edits, Customize's dynamic AI list, the Party Formation screen (save
//! points only; the characters not unlocked yet locked), the save's party record (format 2 + the v5 friends record +
//! Zero EXP + the unlocks), the save point's party rest, the party's EXP and prizes by the P2 rules, the friends'
//! victory bonuses, the game over of a fallen friend leader. Plan: build/sora_tmp/er/party/ER_PARTY_PLAN.md stream S6;
//! state note build/sora_tmp/er/party/IMPL_S6_STATE.md.
//!
//! ONE MAILBOX BETWEEN THE PARTY'S STEP AND THE SCREENS. The party runtime lives in S1's host (party/glue.rs
//! `PartyCore`, kept across Sora hosts); the screens are hud.rs, save.rs, savepoint.rs, prize.rs, bossreward.rs,
//! loot.rs and void.rs, each on its own lock and its own moment of the frame. [`sync`] (S1 calls it once per party step,
//! right after `PartyCore::step`) takes what the screens asked for (edits, a formation, a loaded record, prizes,
//! EXP, the save point's Recover, rewards) into the runtime and publishes a [`Feed`] of what they show. Nothing here
//! touches the game; no lock of this file is held across a game call or across the HUD's lock (the HUD's frame reads
//! the feed under its own lock: plain data).
//!
//! OFF SWITCHES: the cargo feature `party` (this file is not built without it) and the key `party.er = 0`; with either
//! off, or while S1 has never synced (no feed), every hook below is a no-op and the screens are exactly as before.

use std::sync::Mutex;

use glam::Vec3;
use kh_core::status::PrizeKind;
use kh_core::{Sora, UiSnapshot};
use kh_host::formation::Formation;
use kh_host::party::{FriendAiSettings, PartyChara, PartyHudSlot, PartyMenuEdit, PartyMenuMember};
use kh_partyhost::{FriendReward, PartyRuntime};
use serde_json::{json, Value};

use crate::control::Control;
use crate::erlog;

/// What the screens show of the party (published by [`sync`] after every party step).
#[derive(Clone, Debug)]
pub struct Feed {
    /// The friends' slots of the status HUD (contract F.1).
    pub slots: Vec<PartyHudSlot>,
    /// (GOOFY-GG) Goofy's GUARD bar while he leads (kh-partyhost `leader_guard_hud`).
    pub guard: Option<(kh_host::party::GuardValues, kh_host::party::GuardLook)>,
    /// (DON-UI) Donald's HUD while he leads (kh-partyhost `don_hud_view`): ring, pips, stars, tracker, statuses, menu extras.
    pub don: Option<kh_host::party::DonHudView>,
    /// The leader (the HUD's main slot) and whether Sora is in the active party.
    pub leader: PartyChara,
    pub sora_in: bool,
    /// The formation as the last route applied it.
    pub formation: Formation,
    /// The camp menu's party pages (contract F.2).
    pub menu: Vec<PartyMenuMember>,
    /// Customize's dynamic list: the members the AI runs now (Sora while a friend leads).
    pub ai_members: Vec<(PartyChara, FriendAiSettings)>,
    /// A friend leading: his command menu, lock and soft target over Sora's snapshot (`PartyRuntime::hud_snapshot`).
    pub led: Option<UiSnapshot>,
    /// The friend leader is down: the game is over (as Sora's death).
    pub leader_down: bool,
    /// The leader's feet (KH3 cm) and yaw, for the game over's camera.
    pub leader_feet: Option<(Vec3, f32)>,
    /// A friend leader's capsule (centre, radius, half height): the save point tests it.
    pub leader_capsule: Option<(Vec3, f32, f32)>,
    /// The save point's "needs recovery" with the party (P2).
    pub needs: bool,
    /// The friends as prize getters (P2; Sora's slot is the host's, with his live status).
    pub getters: Vec<(PartyChara, kh_host::prize::PrizeMember)>,
    /// Sora is stepped (in the active party).
    pub sora_active: bool,
    /// The party's abilities for the drop (PRIZE_FEEVER, LUCK_UP, the converters).
    pub mods: kh_prize::SpawnMods,
    /// The friends in the party now.
    pub present: Vec<PartyChara>,
    /// The save's party record as it stands.
    pub record: Value,
    /// The friends' weapons as Equipment pieces (built once).
    pub weapons: Vec<kh_ui::camp_equip::Piece>,
    /// The characters not unlocked yet (S8; empty = everyone).
    pub locked: Vec<PartyChara>,
    /// Counts the syncs (the HUD gives the weapons once per change).
    pub generation: u64,
    /// (EQ) The camp-menu edits the runtime had taken when this feed was made (`EDITS_APPLIED`): a feed older than the
    /// HUD's last edit (`EDITS_SENT`) is not handed to the camp menu (its local copy is newer).
    pub applied: u64,
}

/// What the screens asked of the party since the last sync.
#[derive(Default)]
struct Asks {
    edits: Vec<PartyMenuEdit>,
    formation: Option<Formation>,
    load: Option<Value>,
    prizes: Vec<(PartyChara, PrizeKind)>,
    exp: Vec<(i32, f32, Option<Vec3>)>,
    recover: i32,
    cure_bodies: bool,
    rewards: Vec<(PartyChara, FriendReward, String)>,
    /// (IT) permanent stat boosts received (for the whole roster, unlocked or not), with why.
    boosts: Vec<(kh_party::boosts::Boost, String)>,
}

static FEED: Mutex<Option<Feed>> = Mutex::new(None);
/// (EQ) The camp-menu edits the HUD put into the mailbox, and those the runtime took (both counted under the mailbox's
/// lock): while the second is behind, the feed is older than what the menu shows.
static EDITS_SENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static EDITS_APPLIED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// (PFY) The tests that use the process-wide mailbox run one at a time.
#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());
/// `party.er` as the HUD's last frame read it (the screens without a control file at hand: the save slots' pictures).
static KEY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// `party.er` is on (as of the HUD's last frame).
pub fn key_on() -> bool {
    KEY.load(std::sync::atomic::Ordering::Relaxed)
}
static ASKS: Mutex<Option<Asks>> = Mutex::new(None);
/// The friends' level-ups not yet shown by the HUD: (who, level, Strength, Magic, Defense, AP gained).
static LEVEL_UPS: Mutex<Vec<kh_partyhost::LevelUp>> = Mutex::new(Vec::new());
/// (GOOFY-GG) The level-ups that raised GUARD's max (character, level), for their windows' GUARD line.
static GUARD_UPS: Mutex<Vec<(PartyChara, i32)>> = Mutex::new(Vec::new());
/// (BN) The kit abilities level-ups taught (character, level, names), for their LEVEL UP! windows.
static LEVEL_ABILITIES: Mutex<Vec<(PartyChara, i32, Vec<String>)>> = Mutex::new(Vec::new());
/// (BN) The friends' BONUS! panels not yet shown (character, lines with their KH3 icons).
static FRIEND_BONUSES: Mutex<Vec<(PartyChara, Vec<String>)>> = Mutex::new(Vec::new());

/// (BN) A FRIEND'S BONUS! PANEL (a boss reward, a roster boost): shown in his own window at the HUD's next frame
/// (`Hud::party_bonus`: his roster row's colour and portrait). `lines`: KH3's texts, icons in front.
pub fn friend_bonus(c: PartyChara, lines: Vec<String>) {
    if lines.is_empty() {
        return;
    }
    if let Ok(mut b) = FRIEND_BONUSES.lock() {
        b.push((c, lines));
        let n = b.len();
        if n > 16 {
            b.drain(..n - 16);
        }
    }
}

/// (BN) The BONUS! panel's lines of a roster boost as KH3 words them: a boost item (Strength / Magic / Defense Boost, a
/// small boss's AP Boost) its name after the camp-item icon, then the stat line; an Elden Ring item's permanent AP / max
/// HP / max MP the KH3 stat line alone ("Maximum HP increased!"), no icon.
pub fn boost_panel(b: kh_party::boosts::Boost, why: &str) -> Vec<String> {
    use kh_host::bossrewards::Stat;
    use kh_party::boosts::Boost;
    let side = kh_ui::hud_side::SideStrings::default();
    let item = |s: Stat| {
        let (name, line) = s.panel_texts();
        vec![name.to_string(), line.to_string()]
    };
    match b {
        Boost::Strength => item(Stat::Strength),
        Boost::Magic => item(Stat::Magic),
        Boost::Defense => item(Stat::Defense),
        Boost::Ap(_) if why.starts_with("small boss") => item(Stat::Ap),
        Boost::Ap(_) => vec![side.level_up_lines[3].clone()],
        Boost::Bonus(k) => {
            use kh_core::status::BonusCounts as B;
            let hp = matches!(k, B::HP_UP3 | B::HP_UP5 | B::HP_UP10 | B::HP_UP15 | B::HP_UP30);
            vec![side.bonus_lines[if hp { 0 } else { 1 }].clone()]
        }
    }
}

fn asks<R>(f: impl FnOnce(&mut Asks) -> R) -> R {
    let mut guard = ASKS.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Asks::default))
}

/// The latest feed, read under its lock (`None`: no party step has synced: the screens stay Sora's alone).
pub fn feed<R>(f: impl FnOnce(&Feed) -> R) -> Option<R> {
    FEED.lock().ok()?.as_ref().map(f)
}

/// The party is live for the screens: the key is on and S1 has synced at least once since the last [`reset`].
pub fn live(control: &Control) -> bool {
    control.flag("party.er") != Some(false) && FEED.lock().is_ok_and(|f| f.is_some())
}

/// (S1) THE RUNTIME IS MADE: the real progression (the friends start from KH3's init data and keep what they earn;
/// kh_partyhost progression.rs). Call right after `PartyRuntime::new`, before its first step.
pub fn configure(rt: &mut PartyRuntime) {
    rt.set_progression(true);
}

/// (S1) The runtime went away or `party.er` went off: the screens are Sora's alone again.
pub fn reset() {
    if let Ok(mut f) = FEED.lock() {
        if f.take().is_some() {
            erlog!("party: the screens are Sora's alone again (no party runtime)");
        }
    }
}

/// What a sync hands back to S1 / S8 / S3.
#[derive(Debug, Default)]
pub struct SyncOut {
    /// A loaded save's unlocks (S8 restores them: `PartyCore::set_unlocked` WITHOUT the "newly unlocked join" rule).
    pub restore_unlocked: Option<Vec<PartyChara>>,
    /// A loaded save's unlock record as S8 gave it ([`set_unlock_record`]: the starter, the scenes seen ...; `None`: the
    /// save has none, an older save or a new character).
    pub restore_unlock_record: Option<Value>,
    /// The save point rested the party: every active member's Elden Ring ailments are cured (S3: `AllyBodies::cure`
    /// on each AI ally's body; the leader's body is status.rs's own rest).
    pub cure_bodies: bool,
}

/// (S1) ONCE PER PARTY STEP, RIGHT AFTER `PartyCore::step`: the screens' asks into the runtime, the feed out. `unlocked`
/// = S8's list (None = everyone).
pub fn sync(rt: &mut PartyRuntime, sora: &mut Sora, unlocked: Option<&[PartyChara]>, now: f64) -> SyncOut {
    let mut out = SyncOut::default();
    if !rt.progression {
        // (configure was not called: the friends would take the test level's level-99 loadout) said once
        static TOLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !TOLD.swap(true, std::sync::atomic::Ordering::Relaxed) {
            erlog!("party: S6: the runtime was made without partyui::configure: the real progression is switched on now (friends spawned before keep the test loadout until they are slotted out)");
        }
        configure(rt);
    }
    let asked = asks(|a| {
        EDITS_APPLIED.fetch_add(a.edits.len() as u64, std::sync::atomic::Ordering::SeqCst); // (EQ)
        std::mem::take(a)
    });
    if let Some(record) = asked.load {
        // (the loaded Sora's total is the shared EXP: the friends start again from it and the record)
        rt.reset_friends_for_load(sora.status().exp);
        rt.load_party_record(&record);
        out.restore_unlocked = unlocked_of(&record);
        out.restore_unlock_record = record.get("unlocks").filter(|v| !v.is_null()).cloned();
        if let Ok(mut r) = UNLOCK_RECORD.lock() {
            *r = out.restore_unlock_record.clone();
        }
        erlog!("party: t={now:.3}s the save's party record is loaded: leader {}, {} friend record(s){}, Zero EXP {}", record["leader"].as_str().unwrap_or("sora (no record)"), record["friends"].as_array().map_or(0, |a| a.len()), if record["friends"].is_array() { "" } else { " (an older save: KH3's init data)" }, record["zero_exp"].as_bool().map_or("as Sora had it".to_string(), |z| z.to_string()));
    }
    if let Some(f) = asked.formation {
        erlog!("party: t={now:.3}s Party Formation: {:?}, leader {} (applied at the next step)", f.active().iter().map(|c| c.name()).collect::<Vec<_>>(), f.leader().name());
        rt.set_formation(f);
    }
    for edit in asked.edits {
        // (party P) AI Sora's settings (Customize's "Sora" entry while a friend leads)
        if let PartyMenuEdit::Ai { chara: PartyChara::Sora, settings } = edit {
            rt.set_sora_ai_settings(settings);
            continue;
        }
        rt.apply_edit(&edit);
    }
    for (c, kind) in asked.prizes {
        rt.apply_prize(c, kind);
    }
    for (base, rate, at) in asked.exp {
        let given = rt.give_exp(sora, base, rate, at);
        if let kh_partyhost::ExpGiven::Direct(added) = given {
            erlog!("party: t={now:.3}s EXP {base} x {rate} with {} as player 0 (Sora is not in the party): +{added} into the shared total ({})", rt.leader_chara().name(), sora.status().exp);
        }
    }
    if asked.recover > 0 {
        for c in rt.savepoint_recover(sora, asked.recover) {
            erlog!("savepoint: t={now:.3}s {} was down (KO): the save point's Recover stands him up", c.name());
        }
    }
    out.cure_bodies = asked.cure_bodies;
    // (IT) an older save (no roster total in its party record): seeded once from what Sora received
    if rt.roster_unrecorded {
        let ap = small_ap_boosts(&crate::hud::records());
        if rt.migrate_roster(sora.status().boosts, ap) {
            erlog!("party: t={now:.3}s an older save: the roster's permanent boosts seeded from Sora's (Strength / Magic / Defense bytes, {ap} AP Boost(s) of the small bosses): {:?}", rt.roster_boosts);
        }
    }
    for (b, why) in asked.boosts {
        match rt.receive_boost(b) {
            Some(line) => {
                erlog!("party: t={now:.3}s ROSTER BOOST {line} ({why}): for every character, unlocked or not; total {:?}", rt.roster_boosts);
                // (BN) every friend in the party sees it on his own BONUS! panel (Sora's own panel is raised by his path)
                for c in rt.members.iter().map(|m| m.0).filter(|c| *c != PartyChara::Sora).collect::<Vec<_>>() {
                    friend_bonus(c, boost_panel(b, &why));
                }
            }
            None => erlog!("party: t={now:.3}s roster boost {b:?} ({why}): not a stat or full, nothing added"),
        }
    }
    for (c, reward, why) in asked.rewards {
        match rt.friend_reward(c, &reward) {
            Some(text) => erlog!("boss: t={now:.3}s {} gets {text} ({why})", c.name()),
            None => erlog!("boss: t={now:.3}s {}'s {} ({why}) is not given: he is not in the party (it is caught up at his join)", c.name(), reward.label()),
        }
    }
    publish(rt, sora, unlocked);
    out
}

fn unlocked_of(record: &Value) -> Option<Vec<PartyChara>> {
    let list = record["unlocked"].as_array()?;
    Some(list.iter().filter_map(|k| k.as_str().and_then(kh_host::formation::chara_of)).collect())
}

fn publish(rt: &mut PartyRuntime, sora: &Sora, unlocked: Option<&[PartyChara]>) {
    let ups: Vec<_> = std::mem::take(&mut rt.level_ups);
    let guard_ups: Vec<_> = std::mem::take(&mut rt.guard_ups); // (GOOFY-GG)
    let learned: Vec<_> = std::mem::take(&mut rt.level_abilities); // (BN)
    if !learned.is_empty() {
        if let Ok(mut g) = LEVEL_ABILITIES.lock() {
            g.extend(learned);
            let n = g.len();
            if n > 16 {
                g.drain(..n - 16);
            }
        }
    }
    if !guard_ups.is_empty() {
        if let Ok(mut g) = GUARD_UPS.lock() {
            g.extend(guard_ups);
            let n = g.len();
            if n > 16 {
                g.drain(..n - 16);
            }
        }
    }
    if !ups.is_empty() {
        if let Ok(mut l) = LEVEL_UPS.lock() {
            l.extend(ups);
            let n = l.len();
            if n > 16 {
                l.drain(..n - 16);
            }
        }
    }
    let leader = rt.leader_chara();
    let capsule_point = sora.output().position + Vec3::Z * kh_partyhost::SORA_CAPSULE.2;
    let (members, _) = rt.prize_members(sora, capsule_point);
    let getters = members.into_iter().filter(|m| m.friend).filter_map(|m| PartyChara::from_name(m.name).map(|c| (c, m))).collect();
    let mut record = rt.party_record();
    // (S8) the unlocks' own record (the starter, the scenes seen): kept as S8 gave it
    if let Some(v) = UNLOCK_RECORD.lock().ok().and_then(|r| r.clone()) {
        record["unlocks"] = v;
    }
    let locked: Vec<PartyChara> = match unlocked {
        Some(list) => {
            record["unlocked"] = json!(list.iter().map(|c| kh_host::formation::key_of(*c)).collect::<Vec<_>>());
            kh_host::formation::ROSTER.iter().map(|r| r.chara).filter(|c| !list.contains(c)).collect()
        }
        None => Vec::new(),
    };
    let leader_feet = match rt.leader.and_then(|l| rt.party.view(l)) {
        Some(v) => Some((v.position, v.yaw)),
        None => Some((sora.output().position, sora.output().yaw)),
    };
    let generation = FEED.lock().ok().and_then(|f| f.as_ref().map(|f| f.generation)).unwrap_or(0) + 1;
    let weapons = FEED.lock().ok().and_then(|f| f.as_ref().map(|f| f.weapons.clone())).filter(|w| !w.is_empty()).unwrap_or_else(|| weapon_pieces(rt));
    let feed = Feed {
        slots: rt.hud_slots(),
        guard: rt.leader_guard_hud(), // (GOOFY-GG)
        don: rt.don_hud_view(sora), // (DON-UI)
        leader,
        sora_in: rt.lead.sora_in,
        formation: rt.lead.current_formation(),
        menu: rt.menu_members(),
        ai_members: rt.ai_members(),
        led: rt.hud_snapshot(&UiSnapshot::default()),
        leader_down: rt.lead.leader_state.0 != PartyChara::Sora && rt.lead.leader_state.1,
        leader_feet,
        leader_capsule: rt.leader_capsule(),
        needs: rt.savepoint_needs(sora),
        getters,
        sora_active: rt.lead.sora_active,
        mods: rt.prize_mods(kh_prize::SpawnMods::default()),
        present: rt.members.iter().map(|m| m.0).collect(),
        record,
        weapons,
        locked,
        generation,
        applied: EDITS_APPLIED.load(std::sync::atomic::Ordering::SeqCst), // (EQ)
    };
    if let Ok(mut f) = FEED.lock() {
        *f = Some(feed);
    }
}

/// The friends' weapons as the camp menu's Equipment pieces (the runtime's sheet rows, else kh-party's table).
fn weapon_pieces(rt: &PartyRuntime) -> Vec<kh_ui::camp_equip::Piece> {
    let piece = |key: &str, strength: i32, magic: i32, defense: i32, abilities: Vec<String>| kh_ui::camp_equip::Piece { id: key.to_string(), name: String::new(), kind: kh_ui::camp_equip::KIND_WEAPON, attack: strength, magic, defense, abilities, ..Default::default() };
    if !rt.weapons.is_empty() {
        return rt.weapons.iter().map(|w| piece(&w.key, w.strength, w.magic, w.defense, w.abilities.clone())).collect();
    }
    let mut out = Vec::new();
    for prefix in ["WEP_DONALD", "WEP_GOOFY"] {
        for n in 0..14 {
            let key = format!("{prefix}_{n:02}");
            if let Some(w) = kh_party::tables::weapon(&key) {
                out.push(piece(w.1, w.3, w.4, w.5, w.6.iter().map(|a| a.to_string()).collect()));
            }
        }
    }
    out
}

// ------------------------------------------------------------------------------------------------- the HUD (hud.rs)

/// (hud.rs, under the HUD's lock, before `Hud::frame`) THE PARTY INTO THE HUD: the friends' slots, the leader in the main
/// slot (the shrink / grow swap is the HUD's), the camp menu's party pages, Customize's AI list, the Party Formation
/// screen's formation and locked rows, the friends' weapons, their level-up windows. Returns the snapshot the HUD shows
/// while a friend leads (his command menu, lock and soft target over Sora's), `None` = Sora's own.
pub fn hud_frame(hud: &mut kh_host::hud::Hud, base: &UiSnapshot, control: &Control) -> Option<UiSnapshot> {
    let on = control.flag("party.er") != Some(false);
    KEY.store(on, std::sync::atomic::Ordering::Relaxed);
    // (U4) the friends' weapons drop only for the friends the party has unlocked
    crate::lootmap::set_party_friends(&if on && control.flag("party.weapons") != Some(false) { unlocked_friends(control) } else { Vec::new() });
    // (ERT4) NO PARTY (`party.er = 0`, or no runtime synced): THE HUD IS SORA'S ALONE. Before, the early return left the
    // HUD as the party last set it: ERT4's run switched `party.er = 0` after a Goofy-led game over and Sora stood in the
    // world under Goofy's face, his GUARD bar and a friend's slots.
    let guard = FEED.lock().ok();
    let Some(f) = guard.as_ref().and_then(|g| g.as_ref()).filter(|_| on) else {
        hud_alone(hud);
        return None;
    };
    if !hud.has_party_weapons() && !f.weapons.is_empty() {
        hud.set_party_weapons(f.weapons.clone());
    }
    hud.set_party(&f.slots);
    hud.set_guard(f.guard.clone()); // (GOOFY-GG) Goofy's GUARD bar above MP
    hud.set_don(f.don.clone()); // (DON-UI) Donald's ring, stars, statuses, menu extras
    // (EQ) THE CAMP MENU'S PAGES ONLY FROM A FEED THAT HAS THE MENU'S OWN EDITS: an older one (made before the runtime
    // took them; the world is paused under the menu, so before the fix it stayed old until the menu closed) put the old
    // values back every frame: a toggled ability lit off again, a weapon swap or a shortcut showed the old one, the
    // stock moved twice when the player tried again (pieces lost or doubled)
    let current = f.applied >= EDITS_SENT.load(std::sync::atomic::Ordering::SeqCst);
    if current {
        hud.set_party_menu(f.menu.clone());
    }
    hud.set_leader(f.leader, f.sora_in);
    hud.set_formation(&f.formation);
    hud.set_formation_locked(&f.locked);
    if current {
        hud.set_ai_members(f.ai_members.clone());
    }
    let led = f.led.as_ref().map(|l| {
        let mut s = base.clone();
        s.command = l.command.clone();
        s.lock_on = l.lock_on;
        s.soft_target = l.soft_target;
        s.prompt = None;
        s.aim = Default::default();
        s.sneeze = false;
        s
    });
    drop(guard);
    for (chara, level, d) in LEVEL_UPS.lock().map(|mut l| std::mem::take(&mut *l)).unwrap_or_default() {
        #[allow(unused_mut)]
        let mut lines = hud.level_up_lines(d[0], d[1], d[2], d[3], None);
        // (GOOFY-GG) the level raised GUARD's max: its line
        if let Ok(mut g) = GUARD_UPS.lock() {
            if let Some(i) = g.iter().position(|u| *u == (chara, level)) {
                g.remove(i);
                lines.push(kh_party::guard::GP.hud_level_up_line.to_string());
            }
        }
        // (BN) the kit abilities this level taught, after the ability sparkle (LEVEL UP!: they come from levelling)
        if let Ok(mut l) = LEVEL_ABILITIES.lock() {
            if let Some(i) = l.iter().position(|a| a.0 == chara && a.1 == level) {
                for name in l.remove(i).2 {
                    lines.push(kh_ui::hud_side::icon_line(kh_ui::hud_side::LineIcon::Ability, &name));
                }
            }
        }
        hud.party_level_up(chara, level, &lines);
    }
    // (BN) the friends' BONUS! panels (boss rewards, roster boosts)
    for (c, lines) in FRIEND_BONUSES.lock().map(|mut b| std::mem::take(&mut *b)).unwrap_or_default() {
        hud.party_bonus(c, &lines);
    }
    led
}

/// (ERT4) The HUD with no party: no friend slots, no GUARD bar, Sora leading and in, the default formation, no party pages
/// in the camp menu (its roster falls back to Sora alone). Idempotent: every setter keeps an unchanged value.
fn hud_alone(hud: &mut kh_host::hud::Hud) {
    hud.set_party(&[]);
    hud.set_guard(None);
    hud.set_don(None); // (DON-UI)
    hud.set_party_menu(Vec::new());
    hud.set_formation(&kh_host::formation::Formation::default());
    hud.set_leader(PartyChara::Sora, true);
}

static UNLOCK_RECORD: Mutex<Option<Value>> = Mutex::new(None);

/// (S8) THE UNLOCKS' OWN RECORD in the save's party record (`party.unlocks`: the starter, which unlock scenes ran, any
/// JSON S8 needs): written with every save from now on, handed back at a load (`SyncOut::restore_unlock_record`).
/// S8's list of who is unlocked is `sync`'s `unlocked` (saved as `party.unlocked`).
pub fn set_unlock_record(v: Value) {
    if let Ok(mut r) = UNLOCK_RECORD.lock() {
        *r = Some(v);
    }
}

/// (S8) The unlock record as it stands (the last set or loaded).
pub fn unlock_record() -> Option<Value> {
    UNLOCK_RECORD.lock().ok().and_then(|r| r.clone())
}

/// A Party Formation change for the next party step (the HUD's screen; S8's unlocks may use it too).
pub fn ask_formation(f: Formation) {
    asks(|a| a.formation = Some(f));
}

/// A camp-menu edit of a friend (or of AI Sora's settings) for the next party step.
pub fn ask_edit(edit: PartyMenuEdit) {
    asks(|a| {
        EDITS_SENT.fetch_add(1, std::sync::atomic::Ordering::SeqCst); // (EQ)
        a.edits.push(edit)
    });
}

/// (EQ) THE CAMP MENU'S EDITS IN FORCE AT ONCE, AS SORA'S (party/mod.rs `PartySlot::frame`, every game frame, the world
/// paused or not): the edits the HUD put into the mailbox are applied to the runtime now (not at the first party step
/// after the menu closes) and the feed's menu pages, Customize's AI list and the save's record are made again, so the
/// open screen shows the runtime's own answer (his new stats, an item's HP, a refused edit) and a save right after has
/// them. The rest of the mailbox (formation, a load, prizes ...) stays for `sync`. Returns the edits applied.
pub fn menu_sync(rt: &mut PartyRuntime) -> usize {
    let edits = asks(|a| {
        EDITS_APPLIED.fetch_add(a.edits.len() as u64, std::sync::atomic::Ordering::SeqCst);
        std::mem::take(&mut a.edits)
    });
    if edits.is_empty() {
        return 0;
    }
    let n = edits.len();
    for edit in edits {
        if let PartyMenuEdit::Ai { chara: PartyChara::Sora, settings } = edit {
            rt.set_sora_ai_settings(settings);
            continue;
        }
        rt.apply_edit(&edit);
    }
    let applied = EDITS_APPLIED.load(std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut f) = FEED.lock() {
        if let Some(f) = f.as_mut() {
            f.menu = rt.menu_members();
            f.ai_members = rt.ai_members();
            let mut record = rt.party_record();
            for key in ["unlocks", "unlocked"] {
                if !f.record[key].is_null() {
                    record[key] = f.record[key].clone();
                }
            }
            f.record = record;
            f.applied = applied;
        }
    }
    n
}

/// (hud.rs, under the HUD's lock, after `Hud::apply_actions`) What the camp menu and the Party Formation screen
/// changed: into the mailbox for the next party step.
pub fn hud_after(hud: &mut kh_host::hud::Hud, control: &Control) {
    if !live(control) {
        return;
    }
    let edits = hud.take_party_edits();
    let formation = hud.take_formation_change();
    if edits.is_empty() && formation.is_none() {
        return;
    }
    asks(|a| {
        EDITS_SENT.fetch_add(edits.len() as u64, std::sync::atomic::Ordering::SeqCst); // (EQ)
        a.edits.extend(edits);
        if formation.is_some() {
            a.formation = formation;
        }
    });
}

/// (hud.rs) The friend leader fell: the game is over as at Sora's death (the user: "the death screen should show the
/// party leader in their death pose").
pub fn leader_down(control: &Control) -> bool {
    live(control) && feed(|f| f.leader_down).unwrap_or(false)
}

/// (hud.rs / void.rs) The fallen leader and his feet (KH3 cm) for the game over: `None` = Sora's own death.
pub fn fallen_leader(control: &Control) -> Option<(PartyChara, Vec3)> {
    if !live(control) {
        return None;
    }
    feed(|f| f.leader_down.then(|| (f.leader, f.leader_feet.map_or(Vec3::ZERO, |l| l.0)))).flatten()
}

/// (DTH) The leader's feet (KH3 cm) as of the last party step, whoever leads (the game over's camera).
pub fn leader_feet(control: &Control) -> Option<Vec3> {
    if !live(control) {
        return None;
    }
    feed(|f| f.leader_feet.map(|l| l.0)).flatten()
}

/// (DTH) A new Sora host stood every member up (death.rs `revive_all`): the published "leader down" of the host before
/// is void until the next party step publishes again (the screens must not see the old KO).
pub fn clear_leader_down() {
    if let Ok(mut f) = FEED.lock() {
        if let Some(f) = f.as_mut() {
            f.leader_down = false;
        }
    }
}

// ------------------------------------------------------------------------------------------- the save (save.rs, slots)

/// (save.rs `capture`) THE SAVE'S PARTY RECORD: format 2 + the v5 friends record + Zero EXP + the unlocks
/// (`None`: no party: the HUD's formation record as before).
pub fn party_record(control: &Control) -> Option<Value> {
    if !live(control) {
        return None;
    }
    feed(|f| f.record.clone())
}

/// (save.rs `capture`, no control file at hand) The party record when the party is live (`party.er` as of the HUD's
/// last frame).
pub fn party_record_now() -> Option<Value> {
    if !key_on() {
        return None;
    }
    // a loaded record not taken by the party yet is the truth until then (a capture must not write the runtime's
    // state of before the load into the loaded character's file)
    if let Some(pending) = asks(|a| a.load.clone()) {
        return feed(|_| ()).map(|_| pending);
    }
    feed(|f| f.record.clone())
}

/// (save.rs, a loaded or respawned character) The save's party record for the next party step (any format; null =
/// a save from before the party: the default formation, KH3's init data).
pub fn load_record(record: &Value) {
    asks(|a| a.load = Some(record.clone()));
}

/// The leader a save's party record names (the save-slot picture: kh_host `slot_summary_of` reads the same field).
pub fn record_leader(record: &Value) -> PartyChara {
    record.get("leader").and_then(|l| l.as_str()).and_then(kh_host::formation::chara_of).unwrap_or(PartyChara::Sora)
}

// --------------------------------------------------------------------------------- EXP, prizes (sora.rs, prize.rs)

/// (sora.rs, a kill) THE PARTY'S EXP AWARD by the P2 rules (the leader as player 0, his Zero EXP; the shared total):
/// queued for the next party step. False = no party: the caller gives it to Sora as before.
pub fn give_exp(control: &Control, base_exp: i32, rate: f32, at: Option<Vec3>) -> bool {
    // (Sora leading: his own award, at once, exactly as before the party)
    if !live(control) || control.flag("party.exp") == Some(false) || feed(|f| f.leader == PartyChara::Sora).unwrap_or(true) {
        return false;
    }
    asks(|a| a.exp.push((base_exp, rate, at)));
    true
}

/// (prize.rs) THE GETTER SLOTS (P2): Sora (when in the party) and every friend, the leader as the player pawn; the
/// friends' slots in the returned list are matched by name in [`prize_to`]. `None` = no party: Sora alone as before.
pub fn prize_members(control: &Control, sora: &Sora, sora_point: Vec3) -> Option<Vec<kh_host::prize::PrizeMember>> {
    if !live(control) || control.flag("party.prizes") == Some(false) {
        return None;
    }
    feed(|f| {
        let mut members = Vec::new();
        if f.sora_active {
            let s = sora.status();
            use kh_core::enums::ETresAbilityKind as A;
            members.push(kh_host::prize::PrizeMember { name: "Sora", getter: kh_prize::Getter::sora(sora, sora_point), sora: true, player: f.leader == PartyChara::Sora, friend: false, share_prize: s.count(A::SharePrize) > 0, refuses: false, hp: s.hp, hp_max: s.max_hp(), mp: s.mp, mp_max: s.max_mp() });
        }
        members.extend(f.getters.iter().map(|g| g.1));
        members
    })
    .filter(|m| m.iter().any(|x| x.friend))
}

/// (prize.rs) The drop's modifiers with the friends' abilities (P2): `mods` of Sora, the party's added.
pub fn prize_mods(control: &Control, mut mods: kh_prize::SpawnMods) -> kh_prize::SpawnMods {
    if let Some(m) = live(control).then(|| feed(|f| f.mods)).flatten() {
        mods.prize_fever |= m.prize_fever;
        mods.luck_up += m.luck_up;
        mods.converters.hp |= m.converters.hp;
        mods.converters.mp |= m.converters.mp;
        mods.converters.focus |= m.converters.focus;
        mods.converters.millionaire |= m.converters.millionaire;
        mods.converters.munny |= m.converters.munny;
    }
    mods
}

/// (prize.rs) An HP / MP orb a friend collected (`ApplyPrize` on him at the next party step).
pub struct PartyPrizes {
    pub members: Vec<kh_host::prize::PrizeMember>,
}

impl kh_host::prize::PrizeParty for PartyPrizes {
    fn apply(&mut self, slot: usize, kind: PrizeKind) {
        prize_to(self.members.get(slot).map_or("", |m| m.name), kind);
    }
}

/// An orb for the friend of this name (queued for the next party step).
pub fn prize_to(name: &str, kind: PrizeKind) {
    if let Some(c) = PartyChara::from_name(name).filter(|c| *c != PartyChara::Sora) {
        asks(|a| a.prizes.push((c, kind)));
    }
}

// ------------------------------------------------------------------------------------------ the save point (savepoint.rs)

/// (savepoint.rs) THE SAVE POINT WITH THE PARTY (P2): the pawn its capsules test (the leader: a friend leader's
/// capsule) and his "needs recovery" with the party. `None` = no party: Sora alone as before.
pub fn savepoint_pawn(control: &Control) -> Option<(Option<(Vec3, f32, f32)>, bool)> {
    if !live(control) {
        return None;
    }
    feed(|f| (f.leader_capsule.filter(|_| f.leader != PartyChara::Sora), f.needs))
}

/// (savepoint.rs) The save point's `Recover(n, n, n, kind 1)` on the leader and every member (P2): at the next party
/// step (Sora's own too: the runtime gives it to him when he is in the party).
pub fn savepoint_recover(n: i32) {
    if n > 0 {
        asks(|a| a.recover += n);
    }
}

/// (savepoint.rs / status.rs, the user's rule: "save points should also cure all status effects") THE SAVE POINT
/// CURES EVERY ACTIVE MEMBER: the AI allies' bodies are cured at the next party step (`SyncOut::cure_bodies`, S3).
pub fn savepoint_cure_all() {
    asks(|a| a.cure_bodies = true);
}

/// (savepoint.rs, after status.rs's own rest of the leader's body) THE PARTY'S REST: while the leader stands in the
/// recovery area of a save point that may heal, the AI allies' bodies are cured on entering and once a second after
/// (status.rs's cadence for the leader's body, `cure.er_status`).
pub fn savepoint_rest(control: &Control, in_recover: bool, dt: f32) {
    static STATE: Mutex<(bool, f32)> = Mutex::new((false, 0.0));
    if !live(control) || control.flag("cure.er_status") == Some(false) || control.flag("party.cure") == Some(false) {
        return;
    }
    let Ok(mut s) = STATE.lock() else { return };
    let entered = in_recover && !s.0;
    s.0 = in_recover;
    s.1 = (s.1 - dt).max(0.0);
    let again = in_recover && s.1 <= 0.0;
    if entered || again {
        s.1 = 1.0;
        drop(s);
        // every member but the leader (his body is the player's: status.rs's own save-point rest): S2's ally-cure queue,
        // which S3's pool turns into `cure_chr` on each ally body
        let (leader, sora_in, present) = feed(|f| (f.leader, f.sora_in, f.present.clone())).unwrap_or((PartyChara::Sora, true, Vec::new()));
        let mut members = present;
        if sora_in {
            members.push(PartyChara::Sora);
        }
        for m in members.into_iter().filter(|m| *m != leader) {
            crate::status::request_member(m, leader, "the save point (every member's ailments, the user's rule)");
        }
        if entered {
            erlog!("savepoint: the party rests: every AI ally's Elden Ring ailments are cured with the leader's (the user's rule)");
        }
        savepoint_cure_all();
    }
}

// ------------------------------------------------------------------------------------- rewards (bossreward.rs, loot.rs)

/// (bossreward.rs) A friend's victory bonus or ability (applied at the next party step; `why` for the log).
pub fn give_friend_reward(c: PartyChara, reward: FriendReward, why: &str) {
    asks(|a| a.rewards.push((c, reward, why.to_string())));
}

/// (IT) A PERMANENT STAT BOOST RECEIVED (a small boss's Strength / Magic / Defense / AP Boost, an Elden Ring item mapped to
/// AP or max HP / MP): into the roster's one total at the next party sync, onto every friend, unlocked or not (the
/// user, 2026-10-17 00:50). Sora's own status takes it where it is received, as before.
pub fn roster_boost(b: kh_party::boosts::Boost, why: &str) {
    asks(|a| a.boosts.push((b, why.to_string())));
}

/// (IT) The AP Boosts the small bosses of `records` gave (an older save's migration): every `small.boost_every`th small
/// boss gives a boost, Strength -> Magic -> Defense -> AP in turn (kh_host::bossrewards::SmallRules::reward).
fn small_ap_boosts(records: &[String]) -> u8 {
    let every = kh_host::bossrewards::Table::load().small.boost_every;
    if every == 0 {
        return 0;
    }
    let boosts = kh_host::bossrewards::small_count(records) / every;
    (boosts / 4).min(255) as u8
}

/// The friends in the party now (empty without a party).
pub fn present(control: &Control) -> Vec<PartyChara> {
    if !live(control) {
        return Vec::new();
    }
    feed(|f| f.present.clone()).unwrap_or_default()
}

/// The friends unlocked (S8's list; everyone without one) for rewards that wait for a join.
pub fn unlocked_friends(control: &Control) -> Vec<PartyChara> {
    if !live(control) {
        return Vec::new();
    }
    feed(|f| PartyChara::FRIENDS.into_iter().filter(|c| !f.locked.contains(c)).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;


    /// No enemies (the party's step needs a target list).
    struct NoEnemies;
    impl kh_core::Targets for NoEnemies {
        fn enumerate(&self, _out: &mut Vec<kh_core::host::TargetInfo>) {}
        fn info(&self, _id: kh_core::TargetId) -> Option<kh_core::host::TargetInfo> {
            None
        }
        fn hurt_shapes(&self, _id: kh_core::TargetId, _out: &mut Vec<kh_core::host::HurtShape>) {}
        fn apply_hit(&mut self, _hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
            kh_core::host::HitResult::default()
        }
    }
    impl kh_partyhost::PartyTargets for NoEnemies {
        fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
        fn attacked_by_enemies(&self) -> bool {
            false
        }
    }

    /// One party step as S1's glue makes it (route, the party's step, after-step) and the sync after it.
    fn step(rt: &mut PartyRuntime, sora: &mut Sora, world: &kh_core::test_world::TestWorld, t: f64) -> SyncOut {
        let camera = kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let mut targets = NoEnemies;
        let input = kh_core::Input::default();
        let _ = rt.route(kh_partyhost::RouteIn { sora, input: &input, dt: 1.0 / 60.0, targets: &targets, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: t });
        let out = sora.output();
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        rt.step(kh_partyhost::StepIn { dt: 1.0 / 60.0, sora, out: &out, lock_on: kh_core::TargetId::NONE, camera: &camera, world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: t });
        let _ = rt.after_step(&NoEnemies, 1.0 / 60.0, t);
        sync(rt, sora, None, t)
    }

    /// (ERT4) `party.er = 0` (or no runtime) hands the HUD back to Sora alone: the formation the default, the camp
    /// roster Sora, no friend slots. ERT4's run: after a Goofy-led game over, `party.er = 0` left Goofy's face and GUARD.
    #[test]
    fn ert4_no_party_hands_the_hud_back_to_sora() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let Ok((mut hud, _fonts)) = kh_host::hud::Hud::load(&root.join("build/sora_pack/ui")) else {
            eprintln!("no UI pack: skipped");
            return;
        };
        // the party as a Goofy start left it
        let goofy = Formation::new(&[PartyChara::Goofy], PartyChara::Goofy).unwrap();
        hud.set_formation(&goofy);
        hud.set_leader(PartyChara::Goofy, false);
        assert_eq!(hud.formation(), &goofy);
        let ctl = std::env::temp_dir().join(format!("kh-er-ert4-{}.ctl", std::process::id()));
        std::fs::write(&ctl, "party.er=0\n").unwrap();
        let mut control = Control::new(ctl.clone());
        control.poll();
        assert_eq!(control.flag("party.er"), Some(false));
        let base = UiSnapshot::default();
        assert!(hud_frame(&mut hud, &base, &control).is_none(), "no party: Sora's own snapshot");
        assert_eq!(hud.formation(), &Formation::default(), "the default formation");
        assert_eq!(hud.camp_roster(), vec![PartyChara::Sora], "the camp menu is Sora's alone");
        // no runtime at all (the key on, nothing synced): the same
        hud.set_formation(&goofy);
        reset();
        let on = Control::new(std::env::temp_dir().join("kh-er-partyui-test-no-such.ctl"));
        assert!(hud_frame(&mut hud, &base, &on).is_none());
        assert_eq!(hud.camp_roster(), vec![PartyChara::Sora]);
        let _ = std::fs::remove_file(ctl);
    }

    fn runtime() -> PartyRuntime {
        let mut rt = PartyRuntime::new(kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        configure(&mut rt);
        rt
    }

    fn new_game_sora() -> Sora {
        let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
        *sora.status_mut() = kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None));
        sora
    }

    /// THE MAILBOX END TO END (one test: the mailbox is process-wide): the Party Formation's change, the leader's
    /// HUD feed, the camp menu's edit, the EXP with a friend as player 0, the save point's party Recover and cure, the
    /// save's record and its load into a new runtime, the reset.
    #[test]
    fn the_screens_reach_the_party_through_the_mailbox() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner()); // (PFY)
        let world = kh_core::test_world::TestWorld::floor();
        let control = Control::new(std::env::temp_dir().join("kh-er-partyui-test-no-such.ctl"));
        reset();
        assert!(!live(&control), "no sync yet: the screens are Sora's alone");
        assert!(!give_exp(&control, 100, 1.0, None), "no party: the caller gives Sora his EXP as before");
        let mut rt = runtime();
        let mut sora = new_game_sora();
        for k in 0..2 {
            step(&mut rt, &mut sora, &world, k as f64 / 60.0);
        }
        assert!(live(&control));
        assert_eq!(feed(|f| (f.leader, f.slots.len(), f.present.clone())), Some((PartyChara::Sora, 2, vec![PartyChara::Donald, PartyChara::Goofy])));
        assert!(!give_exp(&control, 100, 1.0, None), "Sora leads: his own award, at once");
        // the save point's Party Formation: Donald leads, Sora out of the party
        ask_formation(Formation::new(&[PartyChara::Donald, PartyChara::Goofy], PartyChara::Donald).unwrap());
        for k in 2..5 {
            step(&mut rt, &mut sora, &world, k as f64 / 60.0);
        }
        let (leader, sora_in, led, ai) = feed(|f| (f.leader, f.sora_in, f.led.is_some(), f.ai_members.iter().map(|a| a.0).collect::<Vec<_>>())).unwrap();
        assert_eq!((leader, sora_in, led), (PartyChara::Donald, false, true), "the HUD's main slot and command menu are Donald's");
        assert_eq!(ai, vec![PartyChara::Goofy], "Customize lists the members the AI runs");
        // a camp-menu edit: Goofy's AI settings
        let mine = FriendAiSettings { battle: 2, ability: 1, cure: 0, cure_targets: 3 };
        ask_edit(PartyMenuEdit::Ai { chara: PartyChara::Goofy, settings: mine });
        step(&mut rt, &mut sora, &world, 0.1);
        assert_eq!(rt.ai_settings(PartyChara::Goofy), mine);
        // EXP with Donald as player 0 (Sora out of the party: straight into the shared total)
        let before = sora.status().exp;
        assert!(give_exp(&control, 300, 1.0, None));
        step(&mut rt, &mut sora, &world, 0.12);
        assert!(sora.status().exp > before, "{} -> {}", before, sora.status().exp);
        // the save point: Goofy hurt -> the party needs it; Recover reaches him; the rest cures the bodies
        let goofy = rt.id(PartyChara::Goofy).unwrap();
        rt.party.status_mut(goofy).unwrap().hp = 3;
        step(&mut rt, &mut sora, &world, 0.14);
        let (capsule, needs) = savepoint_pawn(&control).unwrap();
        assert!(needs && capsule.is_some(), "the leader's own capsule, the party's need");
        savepoint_recover(1000);
        savepoint_rest(&control, true, 1.0 / 60.0);
        let out = step(&mut rt, &mut sora, &world, 0.16);
        assert!(out.cure_bodies, "the AI allies' bodies are cured with the leader's");
        assert_eq!(rt.view(PartyChara::Goofy).map(|v| v.hp == v.hp_max), Some(true));
        // a prize orb Goofy took
        rt.party.status_mut(goofy).unwrap().hp = 3;
        prize_to("Goofy", PrizeKind::HpLarge);
        step(&mut rt, &mut sora, &world, 0.18);
        assert_eq!(rt.party.status(goofy).unwrap().hp, 8);
        // the save's record, and its load into another runtime (a reload after a death)
        let record = party_record_now().expect("live: the record");
        assert_eq!((record["leader"].as_str(), record["friends"].as_array().map(|a| a.len()), record["friends_version"].as_u64()), (Some("donald"), Some(2), Some(5)));
        assert_eq!(record_leader(&record), PartyChara::Donald);
        let mut rt2 = runtime();
        let mut sora2 = new_game_sora();
        load_record(&record);
        assert_eq!(party_record_now().map(|r| r["leader"].clone()), Some(json!("donald")), "a pending load is the truth until it is taken");
        for k in 0..4 {
            step(&mut rt2, &mut sora2, &world, k as f64 / 60.0);
        }
        assert_eq!(rt2.leader_chara(), PartyChara::Donald);
        assert_eq!(rt2.ai_settings(PartyChara::Goofy), mine);
        reset();
        assert!(!live(&control));
    }
    /// (IT) THE ROSTER'S PERMANENT BOOSTS through the mailbox (the user, 2026-10-17 00:50): a boost received while
    /// Donald is out of the party reaches Goofy at once and Donald at his join; the save's record keeps the total once and
    /// a load gives it back; an older record (no total) is seeded once from Sora's boost bytes.
    #[test]
    fn it_roster_boosts_reach_everyone_and_survive_the_save() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let world = kh_core::test_world::TestWorld::floor();
        reset();
        asks(|a| a.boosts.clear()); // boosts another test (bossreward's small boss) left in the process-wide mailbox
        let mut rt = runtime();
        let mut sora = new_game_sora();
        ask_formation(Formation::new(&[PartyChara::Sora, PartyChara::Goofy], PartyChara::Sora).unwrap());
        for k in 0..4 {
            step(&mut rt, &mut sora, &world, k as f64 / 60.0);
        }
        assert!(rt.id(PartyChara::Donald).is_none(), "Donald is out of the party");
        let goofy = rt.id(PartyChara::Goofy).unwrap();
        let g0 = rt.party.status(goofy).unwrap().clone();
        roster_boost(kh_party::boosts::Boost::Strength, "test: a small boss");
        roster_boost(kh_party::boosts::Boost::Ap(1), "test: a Whetblade");
        roster_boost(kh_party::boosts::Boost::Bonus(kh_core::status::BonusCounts::MP_UP5), "test: a Memory Stone");
        step(&mut rt, &mut sora, &world, 0.1);
        let g1 = rt.party.status(goofy).unwrap();
        assert_eq!((g1.strength() - g0.strength(), g1.max_ap() - g0.max_ap(), g1.max_mp() - g0.max_mp()), (1, 1, 5));
        // Donald joins later: he arrives with all of it
        ask_formation(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Sora).unwrap());
        for k in 0..4 {
            step(&mut rt, &mut sora, &world, 0.2 + k as f64 / 60.0);
        }
        let d = rt.party.status(rt.id(PartyChara::Donald).expect("Donald joined")).unwrap();
        assert_eq!(d.roster, rt.roster_boosts);
        assert_eq!((d.roster.strength, d.roster.ap), (1, 1));
        // the save's record, then a load into another runtime
        let record = party_record_now().expect("the record");
        assert_eq!(record[kh_party::boosts::RECORD_KEY]["strength"], 1);
        let mut rt2 = runtime();
        let mut sora2 = new_game_sora();
        load_record(&record);
        for k in 0..4 {
            step(&mut rt2, &mut sora2, &world, k as f64 / 60.0);
        }
        assert_eq!(rt2.roster_boosts, rt.roster_boosts);
        assert!(rt2.members.iter().all(|(_, id)| rt2.party.status(*id).is_some_and(|s| s.roster == rt.roster_boosts)));
        // an older save: no total in its record -> seeded once from Sora's bytes
        let mut old = record.clone();
        old.as_object_mut().unwrap().remove(kh_party::boosts::RECORD_KEY);
        sora2.status_mut().boosts.defense = 2;
        load_record(&old);
        for k in 0..3 {
            step(&mut rt2, &mut sora2, &world, 0.2 + k as f64 / 60.0);
        }
        assert!(!rt2.roster_unrecorded);
        assert_eq!(rt2.roster_boosts.defense, 2);
        reset();
    }
    #[test]
    fn a_records_leader_and_unlocks_are_read() {
        let v = json!({ "format": 2, "leader": "goofy", "active": ["goofy", "sora"], "unlocked": ["goofy", "sora", "riku"] });
        assert_eq!(record_leader(&v), PartyChara::Goofy);
        assert_eq!(record_leader(&Value::Null), PartyChara::Sora);
        assert_eq!(unlocked_of(&v), Some(vec![PartyChara::Goofy, PartyChara::Sora]));
        assert_eq!(unlocked_of(&json!({ "leader": "sora" })), None);
    }
}
