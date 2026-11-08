//! (MIMIC-BOSS, THE USER 2026-11-05: "for the mimic tear boss fight, it needs to mimic whoever is party leader and actually
//! fight the party"; "make it so that the mimic tear boss is a solo fight for the party leader, removing their allies upon
//! going thru the fog gate, and the allies only come back if the player dies or wins"; "summoning spirit ashes works in the
//! mimc tear's boss fight, but any summoned outside of the boss room get wiped as well entering the fog gate with the rest of
//! the party (they don't return when your allies return) and the mimic tear boss is able to use the spirit ashes you have
//! equipped and can summon them itself. This is only true for the boss"; "this all needs to apply to the Stray Mimic Tear as
//! well in the Haligtree"; "make sure it's possible that the mimic tear bosses can summon their own mimic tear if you have
//! that spirit ash equipped ... the mimic tear summoned by the boss cannot summon another spirit ash and must fight the player
//! and be allied to the boss it was summoned by"; "Non-Sora mimic bosses won't summon spirit ashes") THE MIMIC TEAR BOSSES,
//! THE ELDEN RING SIDE. UNTESTED IN THE GAME (the user tests it: build/sora_tmp/weapons/MIMIC-BOSS/STATE.md section 6).
//!
//! The engine-free rules are kh-partyhost `mimicboss` (the shared list of the two bosses, the solo fight, the copy, its
//! hostile targets); the Sora copies' pose and drawing are spirit_mimic.rs's own (its rig, pose, keychain, effect and sound
//! seams and the Mimic Tear's draw, made shareable); the friend copies' are the party's (kh-partyhost's runtime and poser,
//! kh-er-render's party drawing).
//!
//! WHAT RUNS (every Sora frame and fixed step; nothing at all unless one of the two bosses' characters is in the loaded
//! world, by its NpcParam: kh-partyhost `mimicboss::boss_of_npc`, the one gate):
//! 1. THE FIGHT (`frame`): Elden Ring's own facts: its defeat flag, its fog-pass flag (or, the first visit without a fog,
//!    its body enabled by the game), the leader in its arena (the pool's MSB place, calibrated on the boss's own MSB place
//!    the first time the body is seen at its nest), the leader alive. `mimicboss::fight_on`.
//! 2. THE SOLO FIGHT: at the fight's start every friend leaves through the party's formation (`PartyRuntime::solo_begin`,
//!    their state kept) and a spirit summoned before the gate is dismissed by the spirit's own leave (spirit.rs `leave`: the
//!    vanish, then Elden Ring's dismiss; Mimic Sora melts away); it is not brought back. They come back on the boss's
//!    defeat flag, or when the leader stands outside the arena again (a respawn after a death, a warp, a load:
//!    `PartyRuntime::solo_end`, then the glue's reseat beside the leader). A spirit summoned inside the arena stays.
//! 3. THE COPY: when the game enables the boss's body (its pool has formed it), the leader as he is then (`copy_sora`: a
//!    clone of the Sora core; a friend leader: `FriendBoss`) at the body's place, forming out of the silver as Mimic Sora
//!    arrives (spirit_mimic `phase`), then driven by the hostile KH AI (`BossAi` on AI Sora's brain; a friend's own tree),
//!    its only enemies the party (`BossTargets`). NO TIMER: it lives until the game's body dies or the fight ends.
//! 4. ITS BODY: Elden Ring's boss character itself, held: hidden (its render switch), no AI, no movement, no actions of its
//!    own, written to the copy's feet in both physics phases. Its HP, its defences, its boss bar, its death, its runes and
//!    its flags stay the game's (the party's hits reach it as any enemy: ErTargets' bullets); a party hit on it is the copy's
//!    KH3 reaction too (a Goofy copy may parry it first: then nothing reaches the game). Puppet (launches by the enemy
//!    reaction rules) leaves it alone: the copy's own KH3 reactions launch and juggle it.
//! 5. ITS HITS: through the party's damage path (the leader Sora's `take_hit`, a friend leader's `receive_hit`), x the boss's
//!    attack rate (its GameClearSpEffect's physicsAttackPowerRate while that SpEffect is on it: a later journey).
//! 6. ITS DEATH: the game's (its HP 0): the copy's KH3 KO, then the silver dissolve; the boss's own summon goes with it.
//! 7. ITS SUMMON: once a fight, when its HP first falls to 60 % (`mimicboss::summon_due`), the ash in the first equipped slot
//!    of the leader's Summon command when the copied leader has one (`boss_summon_of`: Sora only): its own Mimic Tear (a
//!    hostile Mimic Sora: the spirit route's copy of the same leader at the ash's level, its spirit time, on a hidden body of
//!    the boss's team) or Elden Ring's spirit of the ash (its BuddyParam rows, the boss's team, the ash's level), at no cost
//!    to the player. Neither summons anything itself.
//!
//! KEYS: `mimic.boss` (1; 0 = Elden Ring's own Mimic Tears as before: no copy, no solo rule, no summon). Logs: `mimicboss:`.

#[cfg(feature = "party")]
pub use live::*;

#[cfg(not(feature = "party"))]
mod off {
    /// No party host in this build: the bosses are Elden Ring's own.
    pub fn is_held(_handle: &eldenring::cs::FieldInsHandle) -> bool {
        false
    }

    pub fn before_party_hit(_hit: &kh_core::HitRecord, _kh_hp: f32, _body: u64) -> Option<kh_core::host::HitResult> {
        None
    }

    /// No party host: nothing of ours holds a Mimic Tear.
    pub fn forming(_handle_bits: u64) -> bool {
        false
    }
}

#[cfg(not(feature = "party"))]
pub use off::*;

#[cfg(feature = "party")]
mod live {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use eldenring::cs::{ChrIns, FieldInsHandle};
    use glam::Vec3;
    use kh_anim::Trs;
    use kh_core::command::summon::LeaveReason;
    use kh_core::{Attach, EventKind, HitRecord, Input, Output, Pose, Sora, TargetId};
    use kh_host::party::PartyChara;
    use kh_partyhost::mimicboss::{self as mb, BossTargets, FightFacts, MimicBossRow, SoloEnd};
    use kh_partyhost::{BossAi, BossScene, PartyRuntime};

    use crate::control::Control;
    use crate::coords::Frame;
    use crate::erlog;
    use crate::game;
    use crate::spirit_mimic::{self as sm, Rig};
    use crate::targets::ErTargets;
    use crate::world::ErWorld;

    const STEP: f32 = 1.0 / 60.0;
    /// (MIMIC-FX) The copies' own member ids (kh_audio::copies; Mimic Sora is 12): the boss's Sora copy 10, its summoned Mimic
    /// 11 (a friend copy: Donald's 8, Goofy's 9, `friend_member`): their handles, taps, effects and audio state are their own.
    const BOSS_COPY: u8 = kh_audio::copies::BOSS_SORA;
    const SUMMON_COPY: u8 = kh_audio::copies::BOSS_MIMIC;

    /// (MIMIC-FX) A friend copy's own member id (Donald's copy 8, Goofy's 9).
    fn friend_member(c: PartyChara) -> u8 {
        if c == PartyChara::Goofy {
            kh_audio::copies::GOOFY
        } else {
            kh_audio::copies::DONALD
        }
    }
    /// The copy's lethal blow is given again every this many steps until it lies in its KO.
    const KO_EVERY: u64 = 10;
    /// A friend copy's KO is shown this long before he is gone (s; the spirit's KH3 knockdown hold).
    const FRIEND_GONE_AFTER: f32 = sm::phase::KO_HOLD + sm::phase::DEATH_FADE;

    // ------------------------------------------------------------------------------------------------ the copies

    /// A Sora copy (the boss's own, or its summoned Mimic Tear): its core, its hostile AI, its pose side.
    struct SoraCopy {
        core: Sora,
        ai: Option<BossAi>,
        rig: Box<Rig>,
        out: Output,
        out_before: Output,
        pose: kh_anim::Pose,
        pose_before: kh_anim::Pose,
        chain: Vec<Trs>,
        chain_before: Vec<Trs>,
        weapon: (bool, f32),
        weapon_shown: bool,
        tall: Option<f32>,
        /// (MIMIC-FX) its own member id (`BOSS_COPY` / `SUMMON_COPY`)
        member: u8,
        phase: sm::phase::Phase,
        /// Its hits x this (1, the boss's later-journey rate, a summoned Mimic's upgrade) and the damage taken / this.
        attack: f32,
        ko_steps: u64,
        /// (MIMIC-RENDER) seconds since it stood whole out of its pool (None: still forming): the boss copy is silver for
        /// SpEffect 16316's 3 s from then, then his KH3 look (kh-er-render sora_copy.rs `silver_after_formed`)
        formed_for: Option<f32>,
        /// (MIMIC-BOSS hotfix 3, THE USER: "anything the mimic boss or summon does that would heal it should heal it for the same %
        /// of HP that Sora would get healed by") the share of its KH3 maximum its own core healed it by in its last step
        /// (mb::heal_share), for its Elden Ring body ([`heal_body`]).
        healed: f32,
        /// (REVENGE) its KH3 revenge (mimicrevenge.rs): the meter at the boss body's limit, its armour, its counter
        revenge: crate::mimicrevenge::CopyRevenge,
    }

    impl SoraCopy {
        fn new(core: Sora, rig: Box<Rig>, member: u8, attack: f32) -> SoraCopy {
            let out = core.output();
            let weapon_out = core.actions().weapon_out;
            SoraCopy { core, ai: None, rig, out_before: out.clone(), out, pose: kh_anim::Pose::default(), pose_before: kh_anim::Pose::default(), chain: Vec::new(), chain_before: Vec::new(), weapon: (weapon_out, 10.0) /* (hotfix 2) a clone of the leader keeps his Keyblade out: no WeaponVisible comes for it */, weapon_shown: false, tall: None, member, phase: sm::phase::Phase::Arriving(0.0), attack, ko_steps: 0, formed_for: None /* (MIMIC-RENDER) */, healed: 0.0 /* (hotfix 3) */, revenge: Default::default() /* (REVENGE) */ }
        }
    }

    /// A friend leader's copy: its runtime (one AI member) and its own poser.
    struct FriendCopy {
        boss: mb::FriendBoss,
        poser: kh_partyhost::PartyPoser,
        ko_for: Option<f32>,
        mounted: String,
        /// (REVENGE) its KH3 revenge (mimicrevenge.rs)
        revenge: crate::mimicrevenge::CopyRevenge,
    }

    enum Copy {
        Sora(Box<SoraCopy>),
        Friend(Box<FriendCopy>),
    }

    /// The boss's summon in the field.
    enum Foe {
        /// Asked of the creator (the creator's last character before it, when, the spec).
        Asked { before: usize, waited: f32, spec: FoeSpec },
        /// Its hostile Mimic Tear on its body.
        /// (REVENGE) its target id and stance as the targets read them this frame (its stance break, its revenge limit)
        Mimic { copy: Box<SoraCopy>, handle: FieldInsHandle, left: f32, full: f32, defense: f32, id: TargetId, stance: f32 },
        /// Elden Ring's spirit of the ash on its body.
        Spirit { handle: FieldInsHandle, what: String },
        /// (MIMIC-ASH) the WHOLE ash on the boss's side: every BuddyParam row of its trigger, the game's own offsets, arrival
        /// animation, level doping and team rule (mimic_boss_ash.rs; Elden Ring's own summon refuses a non-player owner).
        Ash(Box<crate::mimic_boss_ash::Ash>),
    }

    #[derive(Clone, Debug)]
    struct FoeSpec {
        chr_id: i32,
        npc_param: i32,
        think: i32,
        init: i32,
        doping: i32,
        mimic: Option<(u8, bool)>,
        what: String,
    }

    /// One fight.
    struct Fight {
        boss: &'static MimicBossRow,
        id: TargetId,
        handle: FieldInsHandle,
        leader: PartyChara,
        /// The leader Sora as he was when the boss copied him (its own summoned Mimic copies him too).
        leader_sora: Box<Sora>,
        copy: Copy,
        summon: Option<kh_core::command::summon::AshSlot>,
        summoned: bool,
        foes: Vec<Foe>,
        /// The party's hits on its body this step, to become the copy's reactions.
        reactions: Vec<HitRecord>,
        hp: (i32, i32),
        attack_rate: f32,
        dead: bool,
        steps: u64,
        stopped: Vec<kh_core::EffectHandle>,
        /// (MIMIC-BOSS hotfix 2) the boss's maximum HP wanted (30 x the copied leader's KH3 maximum: `mb::boss_hp`), the base
        /// written for it, whether its HP was filled to the new maximum, and whether the body's no-death switch is ours.
        hp_want: i32,
        base_want: i32,
        hp_filled: bool,
        no_dead_ours: bool,
        /// (hotfix 2, THE USER: "the mimic summoned by the boss needs to use the boss' HP") its summoned Mimic's HP filled to the boss's maximum.
        summon_hp_filled: bool,
        /// (REVENGE) the boss body's stance (its poise maximum as the targets read it): its copy's revenge limit
        stance: f32,
    }

    struct State {
        on: bool,
        /// The boss whose body is in the loaded world, and the frame's offset of its MSB places (Elden Ring's physics place of
        /// its nest minus the MSB's: 0 when the physics is map-local).
        here: Option<(&'static MimicBossRow, Vec3)>,
        fight: Option<Fight>,
        /// The solo fight is held by us (the party's hold is ours), and the leader died in it.
        solo: bool,
        died: bool,
        rigs: Vec<Box<Rig>>,
        loading: Option<std::thread::JoinHandle<Result<Rig, String>>>,
        posers: Option<kh_partyhost::PartyPoser>,
        poser_failed: bool,
        told: String,
        told_wait: String,
        /// (REVENGE) bossrevenge.rs's rules of this frame (the puppets' own: off with `react.guard = 0`) and react.revenge.armour
        revenge_rules: (crate::bossrevenge::BossRules, f32),
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);

    fn say(slot: &mut String, line: String) {
        if *slot != line {
            erlog!("{line}");
            *slot = line;
        }
    }

    /// The boss's body is held by us (puppet.rs leaves it alone; targets.rs asks `before_party_hit`).
    pub fn is_held(handle: &FieldInsHandle) -> bool {
        let bits = game::handle_bits(handle);
        let Ok(g) = STATE.try_lock() else { return false };
        g.as_ref().and_then(|s| s.fight.as_ref()).is_some_and(|f| game::handle_bits(&f.handle) == bits || f.foes.iter().any(|x| matches!(x, Foe::Mimic { handle, .. } if game::handle_bits(handle) == bits)))
    }

    // ------------------------------------------------------------------------------------------------ the loaders

    /// Two Sora rigs (the boss's copy, its summoned Mimic) and the friends' posers, read on threads while a boss is near.
    fn poll_loads(s: &mut State) {
        if let Some(h) = s.loading.take() {
            if h.is_finished() {
                match h.join() {
                    Ok(Ok(rig)) => s.rigs.push(Box::new(rig)),
                    Ok(Err(e)) => erlog!("mimicboss: !!! a Sora copy's pose side could not be read ({e})"),
                    Err(_) => erlog!("mimicboss: !!! the rig thread panicked"),
                }
            } else {
                s.loading = Some(h);
            }
        }
        if s.loading.is_none() && s.rigs.len() < 2 {
            if let Some(pack) = sm::pack() {
                s.loading = Some(std::thread::spawn(move || sm::load_rig(&pack)));
            }
        }
        if s.posers.is_none() && !s.poser_failed {
            match load_posers() {
                Ok(p) => {
                    erlog!("mimicboss: the friend copies' posers are read ({} member(s))", p.members.len());
                    s.posers = Some(p);
                }
                Err(e) => {
                    erlog!("mimicboss: !!! the friend copies' posers could not be read ({e}): a friend leader's copy stands undrawn");
                    s.poser_failed = true;
                }
            }
        }
    }

    fn party_pack() -> Option<PathBuf> {
        sm::pack()
    }

    fn read_json(path: &std::path::Path) -> Result<serde_json::Value, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The friends' posers (party/mod.rs `create`'s own reading of the pack).
    fn load_posers() -> Result<kh_partyhost::PartyPoser, String> {
        let pack = party_pack().ok_or("no pack yet")?;
        let index = read_json(&pack.join("party").join("party.json")).unwrap_or(serde_json::Value::Null);
        let mut poser = kh_partyhost::PartyPoser::default();
        for id in [kh_partyhost::poser::DONALD, kh_partyhost::poser::GOOFY] {
            let Some(npc) = kh_partyhost::poser::npc_of(id) else { continue };
            let dir = pack.join("party").join(npc);
            let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(id as u64)).cloned().unwrap_or(serde_json::Value::Null);
            let manifest = read_json(&dir.join("manifest.json"))?;
            let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir)?, kh_anim::Settings::default())?;
            poser.members.insert(id, kh_partyhost::PosedMember::new(id, animator, &manifest, &entry)?);
        }
        Ok(poser)
    }

    // ------------------------------------------------------------------------------------------------ the frame

    /// What sora.rs hands over once a frame (after the spirit's frame, before Sora's steps).
    pub struct BossFrame<'a> {
        pub control: &'a Control,
        pub sora: &'a mut Sora,
        pub rt: Option<&'a mut PartyRuntime>,
        pub targets: &'a ErTargets,
        pub frame: Frame,
        /// The leader's feet (KH3) and whether he is alive (no death latched).
        pub leader_kh: Vec3,
        pub leader_alive: bool,
        pub now: f64,
    }

    /// ONCE A FRAME: the fight's state, the solo fight, the copy made when the game enables the body. True: the party's
    /// formation changed (the glue reseats the friends beside the leader).
    pub fn frame(f: BossFrame<'_>) -> bool {
        let BossFrame { control, sora, mut rt, targets, frame, leader_kh, leader_alive, now } = f;
        let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let s = g.get_or_insert_with(|| State { on: true, here: None, fight: None, solo: false, died: false, rigs: Vec::new(), loading: None, posers: None, poser_failed: false, told: String::new(), told_wait: String::new(), revenge_rules: (crate::bossrevenge::BossRules::DEFAULT, crate::bossguard::GuardRules::DEFAULT.revenge_armour) });
        s.on = control.flag("mimic.boss") != Some(false);
        // (REVENGE) the copies' revenge rules: the puppets' of this frame (sora.rs sets them; `react.guard = 0`: off)
        s.revenge_rules = (targets.puppets.boss.unwrap_or(targets.params.boss), targets.puppets.guard.unwrap_or(targets.params.guard).revenge_armour);
        let found = targets.list.iter().find_map(|e| mb::boss_of_npc(e.npc_param).map(|b| (b, e)));
        // (MIMIC-BOSS hotfix 2, the user: "the boss fight and your allies leaving and everything needs to start as soon as you
        // enter the fog gate") the leader's map block: the boss's rules start at its fog gate there, before the game has its
        // body in the world (the user's run: the fog's walk at 155.96 s, the body in the world at 168.56 s)
        let block: i32 = crate::game::player_mut().map_or(0, |p| p.current_block_id.into());
        // (MIMIC-BOSS hotfix 3, THE USER: "the boss mimic disappeared after it's mimic summon died/timed out") the targets read
        // only 40 m about the leader (target.range): the user ran 40 m from the boss copy (the log, 245-246 s) and the fight took
        // its body for gone, ending the copy AND its Mimic. The fight's own body is found by its handle in the whole world.
        let far = if found.is_none() { s.fight.as_ref().and_then(|f| far_body(&f.handle)) } else { None };
        let row = found.map(|f| f.0).or_else(|| s.fight.as_ref().filter(|_| far.is_some()).map(|f| f.boss)).or_else(|| mb::boss_of_block(block));
        let body = found.map(|f| f.1);
        let live_handle = body.map(|e| e.handle); // (MIMIC-BOSS) the boss's body before its copy exists (held while it forms)
        let live = body.map(|e| FarBody { er_pos: e.er_pos, active: e.active, alive: e.alive, hp: e.hp_game, hp_max: e.hp_max }).or(far);
        let mut reseat = false;
        let Some(boss) = row.filter(|_| s.on) else {
            // no boss in the world (another map, a load): a solo fight held ends, a fight is dropped
            if s.solo {
                if let Some(rt) = rt.as_deref_mut() {
                    reseat |= rt.solo_end(sora, if s.died { SoloEnd::Death } else { SoloEnd::Away }, now);
                }
                s.solo = false;
                erlog!("mimicboss: the boss is no longer in the loaded world: the solo fight ends{}", if s.died { " (the leader died in it)" } else { "" });
            }
            if let Some(fight) = s.fight.take() {
                end_fight(fight, "the boss is no longer in the loaded world");
            }
            (s.here, s.died) = (None, false);
            publish_forming(Vec::new());
            return reseat;
        };
        if let (Some(e), true) = (body, s.here.map(|h| h.0.key) != Some(boss.key)) {
            s.here = Some((boss, Vec3::ZERO));
            erlog!("mimicboss: THE {} IS IN THE LOADED WORLD (NpcParam {}, target {}, its body {} / {}, at Elden Ring ({:.2}, {:.2}, {:.2}); its MSB nest ({:.2}, {:.2}, {:.2})): its rules are on", boss.name.to_uppercase(), boss.npc_param, e.id.0, if e.active { "enabled" } else { "disabled" }, if e.alive { "alive" } else { "dead" }, e.er_pos.x, e.er_pos.y, e.er_pos.z, boss.boss_at[0], boss.boss_at[1], boss.boss_at[2]);
        }
        poll_loads(s);
        // (MIMIC-BOSS, the user's run 2026-11-06: "SORA DID NOT APPEAR") ONE FRAME FOR EVERY TEST: the leader and the boss's
        // own body, both Elden Ring's physics places of this frame (the MSB's places are another frame: the MSB box kept the
        // fight from ever starting). The fight's own word too: the game's boss bar on this body.
        let leader_er = frame.pos_kh_to_er(leader_kh);
        let defeated = crate::bossreward::game_flag(boss.defeat_flag).unwrap_or(false);
        // (Nokron's first visit has no fog: the first encounter's own flag, set by its event when the leader walks in)
        let fog_flag = crate::bossreward::game_flag(boss.fog_flag).unwrap_or(false);
        let first = boss.first_flag.is_some_and(|f| crate::bossreward::game_flag(f).unwrap_or(false));
        let fog = fog_flag || (first && live.is_some());
        // (MIMIC-BOSS hotfix 3, MB3-FOGRISE) Nokron's first visit: its pool woken by ER's own event condition at the fog pass
        #[cfg(feature = "party")]
        crate::mimic_rise::frame(boss, defeated, fog_flag, first, live.is_some_and(|e| e.active && e.alive), leader_er.to_array(), now); // (MB3-FOGRISE 2: + the leader's place)
        let bar = body.is_some_and(|e| crate::erhud::boss_bar_handles().iter().any(|h| game::handle_bits(h) == game::handle_bits(&e.handle)));
        let leader_at = leader_er.to_array();
        // the arena: the live boss body when the game has it in the world; before that (the fog's walk, the pool rising) the
        // leader in its map with its fog passed (the flag is the game's, reset by a reload)
        let in_arena = match live {
            // (hotfix 3) behind the fog in its map he is in, however far the copy was left behind (mb::solo_holds)
            Some(e) if s.solo => boss.solo_holds(Some(e.er_pos.to_array()), leader_at, fog_flag, block),
            Some(e) => boss.near_boss(e.er_pos.to_array(), leader_at),
            None => fog_flag,
        };
        let facts = FightFacts { defeated, fog_passed: fog || bar || (live.is_some_and(|e| e.active && e.alive) && s.fight.is_some()), in_arena, leader_alive };
        let on = mb::fight_on(&facts);
        crate::arena_walls::fight(boss.key, on); // (NOKRON-WALLS) the arena's invisible walls follow the fight (arena_walls.rs)
        if !leader_alive && facts.in_arena && s.solo {
            s.died = true;
        }
        let body_words = live.map_or_else(|| "not in the world yet".to_string(), |e| format!("{} {}{}", if e.active { "enabled" } else { "disabled" }, if e.alive { "alive" } else { "dead" }, if body.is_none() { " (beyond the targets' 40 m: found by its handle)" } else { "" }));
        let key = format!("mimicboss: {}: defeat flag {} {}, fog flag {} {}, its boss bar {}, the body {body_words}, the leader {} the arena, {}: the fight {}", boss.name, boss.defeat_flag, defeated as u8, boss.fog_flag, fog_flag as u8, if bar { "shown" } else { "not shown" }, if facts.in_arena { "in" } else { "out of" }, if leader_alive { "alive" } else { "down" }, if on { "RUNS" } else { "does not run" });
        if s.told != key {
            match live {
                Some(e) => erlog!("{key} (the leader at Elden Ring ({:.1}, {:.1}, {:.1}), the boss's body at ({:.1}, {:.1}, {:.1}): {:.1} m apart)", leader_er.x, leader_er.y, leader_er.z, e.er_pos.x, e.er_pos.y, e.er_pos.z, leader_er.distance(e.er_pos)),
                None => erlog!("{key} (the leader at Elden Ring ({:.1}, {:.1}, {:.1}) in map block {block:#010x})", leader_er.x, leader_er.y, leader_er.z),
            }
            s.told = key;
        }
        // THE SOLO FIGHT
        if on && !s.solo {
            s.solo = true;
            s.died = false;
            if let Some(rt) = rt.as_deref_mut() {
                rt.solo_begin(sora, boss.name, now);
            }
            // (hotfix 2) KH3's Replicas from the fight's start (the game asks its boss music only with its boss bar, seconds later)
            crate::mimic_music::fight_begins(boss.key);
            if sora.command().summon.out.is_some() {
                erlog!("mimicboss: {}: a spirit summoned before the fog gate is dismissed with the party (its own leave: the vanish, the game's dismiss); it does not come back", boss.name);
                crate::spirit::leave(LeaveReason::SavePoint);
            }
        } else if s.solo && !on && (defeated || !facts.in_arena) {
            // (the leader down in the arena: held until the respawn puts him outside it)
            let why = if defeated { SoloEnd::Win } else if s.died { SoloEnd::Death } else { SoloEnd::Away };
            if let Some(rt) = rt.as_deref_mut() {
                reseat |= rt.solo_end(sora, why, now);
            }
            s.solo = false;
            s.died = false;
        }
        // THE COPY: made when the game has enabled the body (its pool formed it), the fight on
        if let Some(e) = body.filter(|e| s.fight.is_none() && on && e.active && e.alive && e.hp_game > 0) {
            let leader = rt.as_deref().map_or(PartyChara::Sora, |r| r.leader_chara());
            let at = e.pos;
            let yaw = e.yaw;
            let copy = match leader {
                PartyChara::Sora => s.rigs.pop().map(|rig| {
                    let core = mb::copy_sora(sora, Pose { position: at + Vec3::Z * 5.0, yaw });
                    Copy::Sora(Box::new(SoraCopy::new(core, rig, BOSS_COPY, 1.0)))
                }),
                c => match (rt.as_deref(), s.posers.take()) {
                    (Some(r), Some(poser)) => mb::FriendBoss::new(r, sora, c, at, yaw).map(|boss| Copy::Friend(Box::new(FriendCopy { boss, poser, ko_for: None, mounted: String::new(), revenge: Default::default() /* (REVENGE) */ }))),
                    (_, p) => {
                        s.posers = p;
                        None
                    }
                },
            };
            match copy {
                Some(copy) => {
                    let summon = mb::boss_summon_of(leader, sora);
                    erlog!("mimicboss: {}: THE BOSS COPIES {} at its body (target {}, Elden Ring HP {}/{}): {}; its summon: {}; no timer", boss.name, leader.name().to_uppercase(), e.id.0, e.hp_game, e.hp_max, match &copy { Copy::Sora(c) => mb::describe_sora(&c.core), Copy::Friend(c) => format!("{} (level {})", c.boss.chara.name(), c.boss.id().and_then(|id| c.boss.rt.party.status(id)).map_or(0, |s| s.level)) }, summon.as_ref().map_or_else(|| if leader == PartyChara::Sora { "none (no ash equipped, or no Summon command)".to_string() } else { format!("none ({} has no Summon command)", leader.name()) }, |a| format!("{} +{} at 60 % HP", a.name, a.level)));
                    // (hotfix 2, THE USER: "x30 the health then") the boss's HP: 30 x the copied leader's own KH3 maximum
                    let leader_max = match leader {
                        PartyChara::Sora => sora.status().max_hp(),
                        c => rt.as_deref().and_then(|r| r.id(c).and_then(|id| r.party.status(id))).map_or(1, |st| st.max_hp()),
                    };
                    let hp_want = mb::boss_hp(leader_max, hp_x());
                    let base_want = mb::boss_base_hp(hp_want);
                    erlog!("mimicboss: {}: ITS HP: {} x {}'s KH3 maximum {leader_max} = {hp_want} (its base {base_want} x its resident SpEffect 19397's rate 3; a later journey's rate on top); Elden Ring had {}/{}", boss.name, hp_x(), leader.name(), e.hp_game, e.hp_max);
                    s.fight = Some(Fight { boss, id: e.id, handle: e.handle, leader, leader_sora: Box::new(sora.clone()), copy, summon, summoned: false, foes: Vec::new(), reactions: Vec::new(), hp: (e.hp_game, e.hp_max), attack_rate: 1.0, dead: false, steps: 0, stopped: Vec::new(), hp_want, base_want, hp_filled: false, no_dead_ours: false, summon_hp_filled: false, stance: e.poise_max /* (REVENGE) */ });
                    kh_er_render::mimic::preload_mimic();
                }
                None => say(&mut s.told_wait, format!("mimicboss: {}: the copy waits for its pose side (rigs ready {}, posers {})", boss.name, s.rigs.len(), if s.posers.is_some() { "ready" } else { "not ready" })),
            }
        }
        // the fight's body: its id this frame, its HP, its end
        let mut ended = None;
        if let Some(fight) = s.fight.as_mut() {
            // (hotfix 3) the targets' entry, or beyond their range its handle's body: gone only when the world has it no more
            // (kh-partyhost mb::fight_body; its summon's coming or going never enters it)
            let listed = targets.list.iter().find(|x| game::handle_bits(&x.handle) == game::handle_bits(&fight.handle));
            if let Some(b) = listed {
                fight.id = b.id;
                fight.hp = (b.hp_game.max(0), b.hp_max.max(1));
                fight.stance = b.poise_max; // (REVENGE)
            } else if let Some(b) = far {
                fight.hp = (b.hp.max(0), b.hp_max.max(1));
            }
            // (REVENGE) its summoned Mimic Tear's target id and stance (its stance break, its revenge limit)
            for foe in fight.foes.iter_mut() {
                if let Foe::Mimic { handle, id, stance, .. } = foe {
                    if let Some(b) = targets.list.iter().find(|x| game::handle_bits(&x.handle) == game::handle_bits(handle)) {
                        (*id, *stance) = (b.id, b.poise_max);
                    }
                }
            }
            match mb::fight_body(listed.map(|b| (b.alive, b.hp_game)), far.map(|b| (b.alive, b.hp))) {
                mb::FightBody::Alive => {}
                mb::FightBody::Dead if !fight.dead => {
                    fight.dead = true;
                    erlog!("mimicboss: {}: ITS BODY IS DEAD (the game's death: its runes, its flag {}{}): the copy's KO, then the silver; its summon goes with it", fight.boss.name, fight.boss.defeat_flag, if listed.is_none() { "; beyond the targets' range" } else { "" });
                }
                mb::FightBody::Dead => {}
                mb::FightBody::Gone if !fight.dead => ended = Some("its body is gone"),
                mb::FightBody::Gone => {}
            }
            if !on && !fight.dead && !defeated && !facts.in_arena {
                ended = Some("the leader left the arena");
            }
        }
        if let Some(why) = ended {
            if let Some(fight) = s.fight.take() {
                end_fight(fight, why);
            }
        }
        if let Some(fight) = s.fight.as_mut() {
            hold_body_hp(fight);
        }
        // (MIMIC-BOSS, THE USER 2026-11-07: "also mimic tear boss should not be targetable or attackable until it has
        // transformed into Sora") the bodies not yet transformed: no KH lock, no KH hit (targets.rs, invuln.rs Why::Forming)
        let mut held = Vec::new();
        let copy_form = s.fight.as_ref().map(|f| match &f.copy {
            Copy::Sora(c) if matches!(c.phase, sm::phase::Phase::Arriving(_)) => mb::CopyForm::Forming,
            _ => mb::CopyForm::Formed,
        });
        let boss_body = s.fight.as_ref().map(|f| f.handle).or(live_handle);
        if let (true, Some(h)) = (mb::boss_untouchable(s.solo, defeated, copy_form), boss_body) {
            held.push(game::handle_bits(&h));
        }
        if let Some(f) = s.fight.as_ref() {
            for foe in &f.foes {
                if let Foe::Mimic { copy, handle, .. } = foe {
                    let form = if matches!(copy.phase, sm::phase::Phase::Arriving(_)) { mb::CopyForm::Forming } else { mb::CopyForm::Formed };
                    if mb::summon_untouchable(form) {
                        held.push(game::handle_bits(handle));
                    }
                }
            }
        }
        publish_forming(held);
        reseat
    }

    /// (MIMIC-BOSS) The bodies held untouchable now (their handles' bits), read by targets.rs every frame (its own lock: never
    /// the fight's state lock).
    static FORMING: Mutex<Vec<u64>> = Mutex::new(Vec::new());

    fn publish_forming(held: Vec<u64>) {
        let mut f = FORMING.lock().unwrap_or_else(|e| e.into_inner());
        if *f != held {
            if held.len() > f.len() {
                erlog!("mimicboss: {} body(ies) held untouchable until the copy has transformed into the leader (no KH lock-on, no KH hit)", held.len());
            } else if held.is_empty() {
                erlog!("mimicboss: the copy has taken over: its body is a target and hittable");
            }
            *f = held;
        }
    }

    /// (MIMIC-BOSS) This body (its handle's bits) is a Mimic Tear boss or its summoned Mimic not yet transformed into the leader.
    pub fn forming(handle_bits: u64) -> bool {
        FORMING.lock().map(|f| f.contains(&handle_bits)).unwrap_or(false)
    }

    /// (MIMIC-BOSS hotfix 3) The fight's body as the frame reads it (the targets' entry, or its handle's body when the
    /// targets do not list it: beyond their read range of the leader).
    #[derive(Clone, Copy)]
    struct FarBody {
        er_pos: Vec3,
        active: bool,
        alive: bool,
        hp: i32,
        hp_max: i32,
    }

    /// (MIMIC-BOSS hotfix 3) The fight's body by its handle, wherever it is in the loaded world (targets.rs's own rules of
    /// "active" and "alive").
    fn far_body(handle: &FieldInsHandle) -> Option<FarBody> {
        let chr = crate::targets::find_chr(handle)?;
        if !game::chr_complete(chr) {
            return None;
        }
        let er_pos = game::v3(&chr.modules.physics.position);
        let data = &chr.modules.data;
        Some(FarBody { er_pos, active: chr.chr_flags1c8.is_active() && !chr.debug_flags.character_disabled(), alive: data.hp > 0 && !chr.chr_flags1c5.death_flag(), hp: data.hp, hp_max: data.max_hp }).filter(|b| b.er_pos.is_finite())
    }

    /// (MIMIC-BOSS hotfix 2) The sheet's multiplier of the boss's HP (`sheets/port/mimic-boss.json` row `boss.hp_x_leader`; the
    /// built-in `mb::BOSS_HP_X_LEADER` when the sheet is not there).
    fn hp_x() -> f32 {
        static X: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
        *X.get_or_init(|| {
            let path = crate::log::dir().parent().map(|root| crate::sora::data_paths(root).1.with_file_name("mimic-boss.json"));
            let row = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| sheet_value(&t, "boss.hp_x_leader"));
            erlog!("mimicboss: the boss's HP multiplier: {} ({})", row.unwrap_or(mb::BOSS_HP_X_LEADER), if row.is_some() { "sheet mimic-boss.json" } else { "built in: no sheet row" });
            row.unwrap_or(mb::BOSS_HP_X_LEADER)
        })
    }

    /// (MIMIC-BOSS hotfix 3, MB3-AGGRO; THE USER 2026-11-06: "the boss mimic tear needs to be way way more aggressive") THE
    /// COPIES' HOSTILE TUNING: `sheets/port/mimic-boss.json` rows `aggro.*` over the built-in `mb::CopyAggro::default()`, read
    /// each time a copy's AI is made (so an edited sheet counts from the next fight); logged when it changed.
    fn aggro() -> mb::CopyAggro {
        static LAST: std::sync::Mutex<Option<mb::CopyAggro>> = std::sync::Mutex::new(None);
        let path = crate::log::dir().parent().map(|root| crate::sora::data_paths(root).1.with_file_name("mimic-boss.json"));
        let text = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
        let (a, taken) = mb::CopyAggro::from_rows(|id| text.as_deref().and_then(|t| sheet_number(t, id)));
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if *last != Some(a) {
            erlog!("mimicboss: (MB3-AGGRO) the copies' aggression ({} of {} rows from mimic-boss.json, the rest built in): {}", taken.len(), mb::CopyAggro::ROWS.len(), a.describe());
            *last = Some(a);
        }
        a
    }

    /// (MB3-AGGRO) A number row of a port sheet, 0 and below allowed (a switch's 0, a share's 0).
    pub(crate) fn sheet_number(text: &str, id: &str) -> Option<f32> {
        let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
        v["rows"].as_array()?.iter().find(|r| r[0].as_str() == Some(id)).and_then(|r| r[1].as_f64()).map(|x| x as f32).filter(|x| x.is_finite())
    }

    /// A number row of a port sheet (`{"rows": [[id, value, ...], ..]}`).
    pub(crate) fn sheet_value(text: &str, id: &str) -> Option<f32> {
        let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
        v["rows"].as_array()?.iter().find(|r| r[0].as_str() == Some(id)).and_then(|r| r[1].as_f64()).map(|x| x as f32).filter(|x| x.is_finite() && *x > 0.0)
    }

    /// (MIMIC-BOSS hotfix 2) THE BOSS'S BODY HP, every frame of the fight: its base held at the wanted one (the game makes its
    /// maximum = base x its SpEffects' rates), its HP filled once to the new maximum; THE COPY'S SURVIVE ABILITIES on the game's
    /// HP: the body's no-death switch held while Second Chance / Withstand Combo would leave the real character at 1 HP.
    fn hold_body_hp(fight: &mut Fight) {
        if fight.dead {
            return;
        }
        let Some(chr) = crate::targets::find_chr(&fight.handle) else { return };
        if !game::chr_complete(chr) {
            return;
        }
        let data = chr.modules.data.as_mut();
        if data.hp <= 0 {
            return;
        }
        if data.base_hp != fight.base_want {
            erlog!("mimicboss: {}: its body's base HP {} -> {} (maximum {} now)", fight.boss.name, data.base_hp, fight.base_want, data.max_hp);
            data.base_hp = fight.base_want;
        }
        if !fight.hp_filled && data.max_hp >= fight.hp_want * 95 / 100 {
            data.hp = data.max_hp;
            fight.hp_filled = true;
            erlog!("mimicboss: {}: ITS HP FILLED: {}/{} (wanted {} before a later journey's rate)", fight.boss.name, data.hp, data.max_hp, fight.hp_want);
        }
        let er_hp = data.hp;
        let hold = match &fight.copy {
            Copy::Sora(c) => mb::holds_no_dead(&c.core.status().abilities, mb::combo_window(c.core.status()), er_hp),
            Copy::Friend(c) => c.boss.id().and_then(|id| c.boss.rt.party.status(id)).is_some_and(|st| mb::holds_no_dead(&st.abilities, c.boss.view().is_some_and(|v| v.state_name.to_ascii_uppercase().contains("DAMAGE")), er_hp)),
        };
        let at = (data as *mut eldenring::cs::CSChrDataModule as *mut u8).wrapping_add(crate::zdefer::DATA_FLAGS);
        // SAFETY: one byte inside the live data module (zdefer.rs checks the offset against the SDK at compile time); game thread.
        unsafe {
            let b = std::ptr::read_volatile(at);
            if hold && b & 1 == 0 {
                std::ptr::write_volatile(at, b | 1);
                fight.no_dead_ours = true;
            } else if !hold && fight.no_dead_ours && b & 1 != 0 {
                std::ptr::write_volatile(at, b & !1);
                fight.no_dead_ours = false;
            }
        }
    }

    fn end_fight(mut fight: Fight, why: &str) {
        erlog!("mimicboss: {}: the copy is gone ({why}; {} steps)", fight.boss.name, fight.steps);
        for foe in fight.foes.iter_mut() {
            match foe {
                Foe::Mimic { handle, .. } | Foe::Spirit { handle, .. } => kill_body(handle),
                Foe::Ash(a) => crate::mimic_boss_ash::kill_all(a),
                Foe::Asked { .. } => {}
            }
        }
        if let Some(chr) = crate::targets::find_chr(&fight.handle) {
            if fight.no_dead_ours {
                crate::zdefer::clear_no_death(chr);
            }
            release(chr);
        }
        // (MIMIC-FX) everything of its copies ends (their effects fade out, their looping sounds stop, their taps go)
        let copy = match &fight.copy {
            Copy::Sora(c) => c.member,
            Copy::Friend(c) => friend_member(c.boss.chara),
        };
        crate::fx::copyfx::copy_gone(copy, false);
        crate::fx::copyfx::copy_gone(SUMMON_COPY, false);
        kh_er_render::mimic::clear_mimic();
    }

    // ------------------------------------------------------------------------------------------------ the step

    /// What sora.rs lends one fixed step (right after the party's step and Mimic Sora's).
    pub struct BossStep<'a> {
        pub sora: &'a mut Sora,
        pub rt: Option<&'a mut PartyRuntime>,
        pub camera: Option<kh_camera::CameraView>,
        pub world: &'a ErWorld,
        pub fx: &'a mut crate::fx::Fx,
        pub dt: f32,
        pub now: f64,
    }

    /// ONE FIXED STEP of the boss's copy (and its summon).
    pub fn step(m: BossStep<'_>) {
        let BossStep { sora, mut rt, camera, world, fx, dt, now } = m;
        let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = g.as_mut() else { return };
        let revenge_rules = s.revenge_rules; // (REVENGE)
        let Some(fight) = s.fight.as_mut() else { return };
        for h in fight.stopped.drain(..) {
            kh_core::Effects::stop(&mut fx.effects, h, false);
        }
        fight.steps += 1;
        let Some(camera) = camera else { return };
        // the boss's attack rate: its GameClearSpEffect's while it is on its body (a later journey)
        if fight.steps % 60 == 1 {
            let rate = crate::targets::find_chr(&fight.handle).filter(|chr| chr.special_effect.entries().take(128).any(|e| e.param_id == fight.boss.game_clear_speffect)).map_or(1.0, |_| fight.boss.game_clear_attack);
            if rate != fight.attack_rate {
                erlog!("mimicboss: {}: its attack rate x{rate:.3} (SpEffect {} {})", fight.boss.name, fight.boss.game_clear_speffect, if rate > 1.0 { "on: a later journey" } else { "off" });
                fight.attack_rate = rate;
            }
        }
        let (er_hp, er_max) = fight.hp;
        let reactions = std::mem::take(&mut fight.reactions);
        let leader = fight.leader;
        let boss_name = fight.boss.name;
        let attack_rate = fight.attack_rate;
        let dead = fight.dead;
        // the victim: the leader of the solo fight
        let copy_at = match &fight.copy {
            Copy::Sora(c) => c.out.position,
            Copy::Friend(c) => c.boss.view().map_or(Vec3::ZERO, |v| v.position),
        };
        let victim = match leader {
            PartyChara::Sora => Some(mb::victim_of_sora(sora, copy_at)),
            c => rt.as_deref().and_then(|r| mb::victim_of_friend(r, c, copy_at)),
        };
        let mut t = BossTargets { victims: victim.into_iter().collect(), hits: Vec::new(), attack_rate, orders: Vec::new() };
        // (MIMIC-BOSS hotfix 3, MB3-COMBAT) the party's break of its stance as its own character's knockdown
        if !dead && stance_broke(fight.id) {
            let from = t.victims.first().map_or(copy_at - Vec3::X * 100.0, |v| v.feet);
            knock_down(&mut fight.copy, from, boss_name);
        }
        let mut gone = false;
        let held = crate::grabs::body_held(&fight.handle); // (GRABS) the copy's caught gate
        let stance = fight.stance; // (REVENGE)
        match &mut fight.copy {
            Copy::Sora(c) => {
                // (REVENGE) the party's hits through its revenge: its points; its reaction by Sora's own player rules
                let reactions = revenge_gate(&mut c.revenge, &reactions, Gauge { rules: revenge_rules, stance, hp_max: er_max, dt, who: boss_name });
                step_sora_copy(c, SoraCopyStep { rt: rt.as_deref(), world, camera: &camera, targets: &mut t, fx, reactions: &reactions, er_hp: (er_hp, er_max), dead, held, dt, now }, leader);
                heal_body(&fight.handle, std::mem::take(&mut c.healed), boss_name); // (hotfix 3) its heals on its body, the same share
                revenge_step(&mut c.revenge, dead, dt, boss_name); // (REVENGE)
                gone = c.phase_gone();
            }
            Copy::Friend(c) => {
                // (MB3-AGGRO) his hostile tuning, once (the sheet's rows)
                if c.boss.aggro.is_none() {
                    c.boss.set_aggro(aggro());
                }
                c.boss.mirror_hp(if dead { 0 } else { er_hp }, er_max);
                let hp_mirrored = c.boss.id().and_then(|id| c.boss.rt.party.status(id)).map(|s| s.hp); // (hotfix 3) its own heals from here
                // (REVENGE) the party's hits through its revenge: its points; its reaction by its own player rules
                for h in &revenge_gate(&mut c.revenge, &reactions, Gauge { rules: revenge_rules, stance, hp_max: er_max, dt, who: boss_name }) {
                    let _ = c.boss.take_reaction(h);
                }
                if dead && c.ko_for.is_none() {
                    c.ko_for = Some(0.0);
                    let mut lethal = HitRecord::empty(TargetId::NONE);
                    lethal.damage = 1.0e6;
                    let _ = c.boss.rt.receive_hit(c.boss.chara, &lethal);
                }
                if let Some(k) = c.ko_for.as_mut() {
                    *k += dt;
                    gone = *k > FRIEND_GONE_AFTER;
                }
                // (MIMIC-FX) its seams: ITS own member id (Donald's copy 8, Goofy's 9), its handles, its attaches on ITS taps
                // (its runtime's ids are the real party's), its victims the real ones; its tap and spell actors fed after its pose
                let member = friend_member(c.boss.chara);
                let own = c.boss.id().map(|id| id.0);
                let victims = victims_of(&t);
                let retarget = move |a: Attach| victim_attach(&victims, a);
                let stepped = {
                    let mut fxw = crate::fx::copyfx::FriendCopyFx { inner: &mut fx.effects, member, own, retarget: Some(&retarget) };
                    let mut aw = crate::fx::copyfx::FriendCopyAudio { inner: &mut fx.audio, member, own, retarget: Some(&retarget) };
                    c.boss.rt.set_caught(c.boss.chara, held); // (GRABS)
                    c.boss.step(mb::FriendBossStep { dt, world, targets: &mut t, audio: &mut aw, effects: &mut fxw, camera: &camera, nav: None, now })
                };
                // (hotfix 3) its own heal this step (Donald's Cure, Goofy's items and abilities): the same share on its body
                if let (false, Some(before), Some(st)) = (dead, hp_mirrored, c.boss.id().and_then(|id| c.boss.rt.party.status(id))) {
                    heal_body(&fight.handle, mb::heal_share(before, st.hp, st.max_hp()), boss_name);
                }
                // (REVENGE) its revenge: the armour, then its own counter (Donald's next spell at once, Goofy's Bash / Charge)
                if revenge_step(&mut c.revenge, dead, dt, boss_name) || c.revenge.due {
                    let near = t.victims.first().map(|v| (v.feet - copy_at).length());
                    friend_counter(c, near, boss_name);
                }
                mount_friend_weapon(c);
                let ik = sm::Ik(world);
                let taps = c.boss.rt.pose(&mut c.poser, &ik, dt);
                crate::fx::copyfx::friend_feed(member, own, taps, &stepped.projectiles);
            }
        }
        // the copy's orders and hits on the leader, through the party's damage path
        let at = match &fight.copy {
            Copy::Sora(c) => (c.out.position, c.out.yaw),
            Copy::Friend(c) => c.boss.view().map_or((copy_at, 0.0), |v| (v.position, v.yaw)),
        };
        deliver(&mut t, sora, rt.as_deref_mut(), leader, at, boss_name, copy_stats(&fight.copy));
        // ITS SUMMON
        let share = er_hp as f32 / er_max.max(1) as f32;
        if !dead && mb::summon_due(share, fight.summoned) {
            fight.summoned = true;
            match fight.summon.clone() {
                Some(ash) => {
                    erlog!("mimicboss: {boss_name}: ITS HP {:.0} %: IT SUMMONS {} +{} (the leader's equipped ash; no cost to the player)", share * 100.0, ash.name, ash.level);
                    if ash.mimic {
                        if let Some(spec) = foe_spec(&ash) {
                            fight.foes.push(Foe::Asked { before: 0, waited: -1.0, spec });
                        }
                    } else {
                        // (MIMIC-ASH, THE USER: "multi-ashes should work as if you summoned them") every spirit of the ash
                        match crate::mimic_boss_ash::summon_hostile(ash.goods, &fight.handle, world.frame.pos_kh_to_er(at.0), crate::coords::yaw_kh_to_er(at.1)) {
                            Ok(a) => fight.foes.push(Foe::Ash(Box::new(a))),
                            Err(e) => erlog!("mimicboss: !!! the boss's summon {}: {e}", ash.name),
                        }
                    }
                }
                None => erlog!("mimicboss: {boss_name}: its HP {:.0} %: no summon ({})", share * 100.0, if leader == PartyChara::Sora { "no ash equipped, or no Summon command at the fight's start" } else { "its copied leader has no Summon command" }),
            }
        }
        step_foes(fight, &mut s.rigs, FoeStep { sora, rt, world, camera: &camera, fx, dt, now, revenge_rules });
        // (MIMIC-FX) its summoned Mimic Tear has left the field: everything of it ends
        if !fight.foes.iter().any(|f| matches!(f, Foe::Mimic { .. })) {
            crate::fx::copyfx::copy_gone(SUMMON_COPY, false);
        }
        if gone {
            if let Some(fight) = s.fight.take() {
                end_fight(fight, "its KO and the dissolve are over");
            }
        }
    }

    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT) The copy's Strength, Magic and level (`mb::mirror_hit_on_leader`).
    fn copy_stats(c: &Copy) -> (i32, i32, u32) {
        match c {
            Copy::Sora(c) => sora_stats(&c.core),
            Copy::Friend(c) => c.boss.stats(),
        }
    }

    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT) A Sora copy's Strength, Magic and level.
    fn sora_stats(core: &Sora) -> (i32, i32, u32) {
        let st = core.status();
        (st.strength(), st.magic(), st.level)
    }

    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT) The boss body whose stance break the copy has already answered (one knockdown per break).
    /// (REVENGE) one entry per body: the boss's copy and its summoned Mimic Tear (MIMIC-BOSS item 12).
    static KNOCKED: Mutex<Vec<u32>> = Mutex::new(Vec::new());

    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT) A party Stagger book broke the stance of body `id` (kh-er stance.rs: asked, kneeling or
    /// frozen) and the copy has not answered it yet. The body is hidden: without this the break showed nothing.
    fn stance_broke(id: TargetId) -> bool {
        use crate::stance::Phase;
        let on = matches!(crate::stance::state(id), Some(Phase::Asking | Phase::Down { .. } | Phase::Frozen { .. }));
        let mut k = KNOCKED.lock().unwrap_or_else(|e| e.into_inner());
        if !on {
            k.retain(|x| *x != id.0);
            return false;
        }
        if k.contains(&id.0) {
            return false;
        }
        k.push(id.0);
        true
    }

    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT, THE USER 2026-11-06: "It also needs to play Goofy's animation for getting knocked down
    /// when it's stagger breaks") THE COPY'S OWN KNOCKDOWN (`mb::knockdown_hit`): Goofy's blow, down and get-up for a Goofy copy,
    /// Donald's for Donald, Sora's for a Sora copy.
    fn knock_down(copy: &mut Copy, from: Vec3, who: &str) {
        let started = match copy {
            Copy::Sora(c) => {
                // (taken at his next step by his own rules: his i-frames and KO refuse it there)
                c.core.take_hit(mb::knockdown_of_sora(c.out.position, from));
                true
            }
            Copy::Friend(c) => c.boss.knock_down(from),
        };
        erlog!("mimicboss: {who}: the party broke its stance: the copy's own KH3 knockdown {}", if started { "asked (blow, down, get-up)" } else { "refused (it is down, dying or invincible)" });
    }

    /// (REVENGE) What a copy's revenge gate is told of this step.
    struct Gauge<'a> {
        rules: (crate::bossrevenge::BossRules, f32),
        /// The boss body's stance and maximum HP (its revenge limit, bossrevenge.rs rule 2).
        stance: f32,
        hp_max: i32,
        dt: f32,
        who: &'a str,
    }

    /// (REVENGE) THE PARTY'S HITS ON A COPY THROUGH ITS REVENGE (mimicrevenge.rs): every hit adds its revenge points (none in
    /// its revenge armour); the hits whose KH3 reaction may play are returned: all of them (its own player rules decide, THE
    /// USER 2026-11-07: "make the mimic tear boss stagger whenever Sora would stagger"), none in its revenge armour.
    fn revenge_gate(r: &mut crate::mimicrevenge::CopyRevenge, hits: &[HitRecord], g: Gauge<'_>) -> Vec<HitRecord> {
        use crate::mimicrevenge::CopyGate;
        r.sync(&g.rules.0, g.rules.1, g.stance, g.hp_max as f32);
        r.watch(g.dt);
        let mut kept = Vec::new();
        for h in hits {
            match r.hit(h, h.damage > 0.0) {
                CopyGate::React => kept.push(*h),
                CopyGate::Revenge => erlog!("revenge: {}: '{}' lands in its revenge: unflinchable", g.who, h.attack_name),
            }
        }
        kept
    }

    /// (REVENGE) The meter's step. True: its revenge now (logged): the armour runs and its counter is due.
    fn revenge_step(r: &mut crate::mimicrevenge::CopyRevenge, dead: bool, dt: f32, who: &str) -> bool {
        if dead {
            r.due = false;
            return false;
        }
        let fired = r.step(dt);
        if fired {
            erlog!("revenge: {who}: REVENGE no. {} (KH3's revenge value crossed its limit {:.2}): it breaks out, unflinchable for its armour, its own counter next", r.revenges, r.limit());
        }
        fired
    }

    /// (REVENGE) A Sora copy's revenge counter on its input: while a blow can be recovered from, KH3's Aerial Recovery (his
    /// jump; the ability as copied); once he is free, his counter by his form (mimicrevenge.rs `sora_plan`: Sonic Blade
    /// then Zantetsuken with the Keyblade, else the form's own move) through his core's counter request. While it runs his
    /// AI's presses wait (they would chain his normal combo into it).
    fn sora_counter(c: &mut SoraCopy, mut input: Input, dead: bool, leader: PartyChara) -> Input {
        if c.revenge.due && !dead {
            match c.core.damage_reaction() {
                Some(d) if d.recoverable => {
                    input.jump_pressed = true;
                    input.jump_held = true;
                }
                Some(_) => {}
                None => {
                    let cmd = c.core.combat().form.cmd;
                    let (kinds, own) = crate::mimicrevenge::sora_plan(cmd);
                    c.core.request_counter(&kinds, mb::victim_id(leader), own);
                    c.revenge.due = false;
                    erlog!("revenge: a Sora copy (form {cmd:#x}) COUNTERS on {}: kinds {kinds:x?} ({})", leader.name(), if own { "the form's own move" } else { "Sonic Blade, then Zantetsuken" });
                }
            }
        }
        if c.core.counter_state().is_some() {
            input.pad = kh_core::PadState::default();
        }
        input
    }

    /// (REVENGE) A friend copy's revenge counter, once: Donald's next spell at once (his own choice, his own-spell clock
    /// zeroed), Goofy's Bash within reach of the leader (`near`, cm) else his Charge (his AI takes it first once he is free;
    /// his pad's rank, MP and cool-down rules decide). Neither has an Aerial Recovery of his own in the port.
    fn friend_counter(c: &mut FriendCopy, near: Option<f32>, who: &str) {
        if !std::mem::take(&mut c.revenge.due) {
            return;
        }
        match c.boss.chara {
            PartyChara::Donald => match c.boss.rt.party.don_prog.copy.as_mut() {
                Some(cp) => {
                    cp.cast_in = 0.0;
                    erlog!("revenge: {who}: Donald's copy COUNTERS: his next spell at once (his own choice)");
                }
                None => erlog!("revenge: {who}: Donald's copy has no own-spell clock (aggro off): no counter of his own"),
            },
            PartyChara::Goofy => {
                use kh_party::goofy::GoofyMove as G;
                let bash_first = near.is_some_and(|d| crate::mimicrevenge::goofy_counter(d, crate::mimicrevenge::GOOFY_BASH_REACH) == 0xCC);
                let arts = if bash_first { vec![G::GoofyBash, G::GoofyCharge] } else { vec![G::GoofyCharge, G::GoofyBash] };
                let id = c.boss.id();
                match c.boss.rt.party.gof.ai.iter_mut().find(|m| Some(m.member) == id) {
                    Some(m) => {
                        erlog!("revenge: {who}: Goofy's copy COUNTERS: {arts:?} (the first he wears and may use)");
                        m.revenge_arts = arts;
                    }
                    None => erlog!("revenge: {who}: Goofy's copy has no AI memory yet: no counter"),
                }
            }
            PartyChara::Sora => {}
        }
    }

    /// The copy's hits and orders on the leader: Sora's `take_hit`, a friend leader's `receive_hit` (the party's damage path).
    fn deliver(t: &mut BossTargets, sora: &mut Sora, rt: Option<&mut PartyRuntime>, leader: PartyChara, at: (Vec3, f32), who: &str, copy: (i32, i32, u32)) {
        // (MIMIC-BOSS hotfix 3, MB3-COMBAT) the copy hits the leader as KH3's party hits a foe of its level (mb::mirror_hit_on_leader:
        // its party Strength never against his party Defense: Goofy's Breaker on Goofy was 9.6 of 156)
        let leader_defense = t.victims.iter().find(|v| v.chara == leader).map_or(0, |v| v.defense);
        for h in t.hits.iter_mut() {
            let big = h.info.flags50 & kh_core::host::HitInfo::FLAG50_FINISH != 0 || h.info.power >= 2.0; // (the log: finishers and big rows only)
            if let Some((before, after)) = mb::mirror_hit_on_leader(h, (copy.0, copy.1), copy.2, leader_defense).filter(|_| big) {
                erlog!("mimicboss: {who}'s {} on {}: {before:.1} -> {after:.1} (KH3's base against a foe of level {}: Defense {}, floor {}; not his own Defense {leader_defense})", h.attack_name, leader.name(), copy.2, mb::mirror_defense(copy.2), mb::foe_floor(copy.2));
            }
        }
        // (MIMIC-BOSS hotfix 3, MB3-FREEZE) a copy's bad status reaches the leader as KH3 puts it on the PLAYER
        // (`mb::player_side_status`, inside `player_hit_of` / `party_hit_of`), never with its rows' enemy-side times
        for h in &t.hits {
            let mut seen = *h;
            if let Some((row, player)) = mb::player_side_status(&mut seen) {
                erlog!("mimicboss: {who}'s {} on {}: {} {row:.1} s (its row's time, meant for enemies) -> {} (KH3's longest on the player)", h.attack_name, leader.name(), h.info.bad_status, if player > 0.0 { format!("{player:.1} s") } else { "none: KH3 never puts it on the player".to_string() });
            }
        }
        match leader {
            PartyChara::Sora => {
                mb::stance_to_sora(t, sora, at.0);
                for h in t.hits.drain(..) {
                    sora.take_hit(mb::player_hit_of(&h, at.0, at.1));
                }
            }
            c => {
                let Some(rt) = rt else { return };
                mb::stance_to_party(t, rt, at.0);
                for h in t.hits.drain(..) {
                    if let Some(r) = rt.receive_hit(c, &mb::party_hit_of(&h)) {
                        if r.killed {
                            erlog!("mimicboss: {who}'s copy knocked {} out", c.name());
                        }
                    }
                }
            }
        }
    }

    struct SoraCopyStep<'a> {
        rt: Option<&'a PartyRuntime>,
        world: &'a ErWorld,
        camera: &'a kh_camera::CameraView,
        targets: &'a mut BossTargets,
        fx: &'a mut crate::fx::Fx,
        reactions: &'a [HitRecord],
        er_hp: (i32, i32),
        dead: bool,
        /// (GRABS) its Elden Ring body is held by a throw now (crate::grabs::body_held)
        held: bool,
        dt: f32,
        now: f64,
    }

    impl SoraCopy {
        fn phase_gone(&self) -> bool {
            matches!(self.phase, sm::phase::Phase::Leaving { t, death: true } if t > sm::phase::KO_HOLD + sm::phase::MELT_START + sm::phase::DEATH_FADE + sm::phase::POOL_OUT)
        }
    }

    /// A Sora copy's step: its arrival / leaving, its AI (hostile), the party's hits on it as its reactions, its HP mirrored,
    /// its core's step against the party, its pose and keychain.
    fn step_sora_copy(c: &mut SoraCopy, m: SoraCopyStep<'_>, leader: PartyChara) {
        let SoraCopyStep { rt, world, camera, targets, fx, reactions, er_hp, dead, held, dt, now } = m;
        // the arrival (the pool, the forming), the leaving (its KO, the dissolve)
        if dead && !matches!(c.phase, sm::phase::Phase::Leaving { .. }) {
            c.phase = sm::phase::Phase::Leaving { t: 0.0, death: true };
        }
        let before = c.phase;
        let (t0, t1) = (clock_of(c.phase), clock_of(c.phase) + dt);
        for (kind, id) in sm::phase::sounds(c.phase, t0, t1) {
            let _ = crate::ersound::play_at(kind, id, world.frame.pos_kh_to_er(c.out.position));
        }
        if let Some(p) = sm::phase::advance(c.phase, dt) {
            c.phase = p;
        }
        // (MIMIC-RENDER) its clock since it formed (Elden Ring's liquid silver lasts 3 s from then)
        if c.phase == sm::phase::Phase::Out || c.formed_for.is_some() {
            c.formed_for = Some(c.formed_for.map_or(0.0, |t| t + dt));
        }
        if matches!(before, sm::phase::Phase::Arriving(_)) && c.phase == sm::phase::Phase::Out {
            erlog!("mimicboss: the copy has FORMED out of the silver: its AI takes over");
        }
        // its AI: hostile, the party its only enemy list
        let input = match (rt, sm::phase::still(c.phase) || dead) {
            (Some(rt), false) => {
                let ai = c.ai.get_or_insert_with(|| {
                    let a = BossAi::new(rt).with_aggro(aggro()); // (MB3-AGGRO) the sheet's hostile tuning
                    erlog!("mimicboss: the copy's AI: AI Sora's own, turned on the party ({})", if a.has_tree() { "Goofy's tree n_ex002 on its own controller" } else { "NO TREE PACK: it stands" });
                    a
                });
                ai.set_caught(held); // (GRABS)
                ai.input(rt, &mut c.core, BossScene { targets: &*targets, camera, camera_yaw: camera.rotation.yaw, nav: None, dt, now })
            }
            _ => Input { camera_yaw: camera.rotation.yaw, ..Input::default() },
        };
        let input = sora_counter(c, input, dead, leader);
        // the party's hits on its body: its KH3 reactions; its HP the game's
        for h in reactions {
            c.core.take_hit(mb::reaction_of(h));
        }
        if dead {
            if c.ko_steps.is_multiple_of(KO_EVERY) && c.core.status().hp > 0 {
                mb::knock_out(&mut c.core, c.out.position + Vec3::X * 100.0);
            }
            c.ko_steps += 1;
        } else {
            let max = c.core.status().max_hp();
            c.core.status_mut().hp = mb::mirrored_hp(er_hp.0, er_hp.1, max);
        }
        let hp_mirrored = c.core.status().hp; // (hotfix 3) its own heals in this step are measured from here
        // (MIMIC-FX) its seams: ITS own member id, its effects on ITS scene (its bones, Keyblade, form: the effect side's Sora
        // instance; a victim stays a target its scene answers), its sounds on ITS taps (a victim: the real one)
        let victims = victims_of(targets);
        let retarget = {
            let victims = victims.clone();
            move |a: Attach| victim_attach(&victims, a)
        };
        let mut its_fx = kh_host::seam::EffectQueue::default();
        let out = {
            let mut effects = crate::fx::copyfx::SoraCopyFx { queue: &mut its_fx, member: c.member };
            let mut audio = crate::fx::copyfx::SoraCopyAudio { inner: &mut fx.audio, member: c.member, retarget: Some(&retarget) };
            let mut ui = kh_core::host::NullUi;
            let mut host = kh_core::Host { world, targets: &mut *targets, audio: &mut audio, effects: &mut effects, ui: &mut ui };
            c.core.step_hosted(dt, &input, &mut host)
        };
        // (hotfix 3) its own heal this step, as a share of its KH3 maximum (any source of its core's)
        c.healed = if dead { 0.0 } else { mb::heal_share(hp_mirrored, c.core.status().hp, c.core.status().max_hp()) };
        for h in targets.hits.iter_mut() {
            h.damage *= c.attack;
        }
        for e in &out.events {
            if let EventKind::WeaponVisible { visible, instant } = &e.kind {
                if *instant && *visible {
                    c.weapon = (true, 10.0);
                } else if *visible != c.weapon.0 {
                    c.weapon = (*visible, 0.0);
                }
            }
        }
        c.weapon.1 += dt;
        c.out_before = std::mem::replace(&mut c.out, out);
        c.pose_before = std::mem::take(&mut c.pose);
        c.pose = sm::pose_of(&mut c.rig, &c.out, c.core.status().hp > 0, world);
        if c.pose_before.component.is_empty() {
            c.pose_before = c.pose.clone();
        }
        // (MIMIC-FX) its effects on ITS scene, their sounds, its tap
        let target = |id: TargetId| victims.iter().find(|v| Some(v.0) == mb::victim_chara(id)).map(|v| v.1 + Vec3::Z * v.2);
        crate::fx::copyfx::sora_copy_step(fx, crate::fx::copyfx::SoraCopyStep { member: c.member, out: &c.out, core: &c.core, skeleton: c.rig.animator.skeleton(), pose: &c.pose, commands: std::mem::take(&mut its_fx.0), target: &target, retarget: Some(&retarget) });
        let visible = sm::weapon_opacity(c.weapon) > 0.0;
        sm::chain_of(&mut c.rig, &c.core /* (KEYCHAIN-DUAL) */, &c.pose, &c.out, visible, sm::ChainState { shown: &mut c.weapon_shown, chain: &mut c.chain, before: &mut c.chain_before });
        let _ = STEP;
    }

    fn clock_of(p: sm::phase::Phase) -> f32 {
        match p {
            sm::phase::Phase::Arriving(t) | sm::phase::Phase::Leaving { t, .. } => t,
            sm::phase::Phase::Out => 0.0,
        }
    }

    /// A friend copy's weapon on his poser (the glue's `mount_weapons` rule: his status's weapon, once per change).
    fn mount_friend_weapon(c: &mut FriendCopy) {
        let Some(weapon) = c.boss.id().and_then(|id| c.boss.rt.party.status(id)).map(|s| s.weapon.to_string()) else { return };
        if weapon.is_empty() || c.mounted == weapon {
            return;
        }
        c.mounted = weapon.clone();
        let d = kh_partyhost::poser::draw_id(c.boss.chara);
        let Some(pack) = party_pack() else { return };
        let Some(m) = c.poser.members.get_mut(&d) else { return };
        let r = m.weapon_def(&weapon).and_then(|def| read_json(&pack.join("party").join("weapons").join(&def.model).join("manifest.json")).and_then(|man| m.mount_weapon(&def, &man).map(|_| ())));
        erlog!("mimicboss: the {} copy holds {weapon}{}", c.boss.chara.name(), r.err().map_or(String::new(), |e| format!(" (NOT mounted: {e})")));
    }

    /// (MIMIC-FX) A copy's victims (the leader of the solo fight): who, feet, half height.
    fn victims_of(t: &BossTargets) -> Vec<(PartyChara, Vec3, f32)> {
        t.victims.iter().map(|v| (v.chara, v.feet, v.half_height)).collect()
    }

    /// (MIMIC-FX) A copy's attach on a victim of its own targets (`mb::victim_id`), for the shared queues: the victim's centre
    /// (its feet + half its height, as a target's attach is) plus the offset, a world point; anything else as it is.
    fn victim_attach(victims: &[(PartyChara, Vec3, f32)], a: Attach) -> Attach {
        match a {
            Attach::Target { target, offset } => match mb::victim_chara(target).and_then(|c| victims.iter().find(|v| v.0 == c)) {
                Some(v) => Attach::World(v.1 + Vec3::Z * v.2 + offset),
                None => a,
            },
            other => other,
        }
    }

    // ------------------------------------------------------------------------------------------------ its summon

    /// The boss's summon of an ash: its own Mimic Tear (a hidden c0000 body of the Mimic Tear's BuddyParam row for the
    /// hostile Mimic Sora) or Elden Ring's spirit of the ash (its first BuddyParam row of the ash's trigger), with the row's
    /// upgrade SpEffect of the ash's level.
    fn foe_spec(ash: &kh_core::command::summon::AshSlot) -> Option<FoeSpec> {
        use eldenring::cs::{BuddyParam, EquipParamGoods, SoloParamRepository};
        use fromsoftware_shared::FromStatic;
        // SAFETY: game thread; the singleton lives as long as the game.
        let repo = unsafe { SoloParamRepository::instance() }.ok()?;
        let goods = repo.get::<EquipParamGoods>(ash.goods)?;
        let r = crate::spirit::request_of(ash.goods, goods.ref_id_default(), goods.ref_category()).map_err(|e| erlog!("mimicboss: the boss's summon {}: {e}", ash.name)).ok()?;
        // (MIMIC-ASH's trace) the Mimic Tear's trigger has two rows: 20700000 (c3320, the generator's marker for its equipment
        // copy) and 20700001 (c0000 by ChrInit 26050): its body is the c0000 row, the one with a ChrInit
        let (id, row) = repo.rows::<BuddyParam>().filter(|(_, b)| b.trigger_sp_effect_id() == r.trigger).find(|(_, b)| !ash.mimic || b.npc_player_init_param_id() > 0)?;
        let doping = [row.doping_sp_effect_lv0(), row.doping_sp_effect_lv1(), row.doping_sp_effect_lv2(), row.doping_sp_effect_lv3(), row.doping_sp_effect_lv4(), row.doping_sp_effect_lv5(), row.doping_sp_effect_lv6(), row.doping_sp_effect_lv7(), row.doping_sp_effect_lv8(), row.doping_sp_effect_lv9(), row.doping_sp_effect_lv10()][(ash.level as usize).min(10)];
        let npc = row.npc_param_id();
        let spec = FoeSpec { chr_id: if row.npc_player_init_param_id() > 0 { 0 } else { (npc / 10_000) % 10_000 }, npc_param: npc, think: row.npc_think_param_id(), init: row.npc_player_init_param_id(), doping, mimic: ash.mimic.then_some((ash.level, ash.renowned)), what: format!("{} +{} (BuddyParam {id}, NpcParam {npc})", ash.name, ash.level) };
        erlog!("mimicboss: the boss's summon: {} (chr c{:04}, think {}, ChrInit {}, upgrade SpEffect {}){}", spec.what, spec.chr_id, spec.think, spec.init, doping, if ash.mimic { ": ITS OWN MIMIC TEAR (a hostile Mimic Sora of the same leader on that body)" } else { ": Elden Ring's spirit, on the boss's side" });
        Some(spec)
    }

    struct FoeStep<'a> {
        sora: &'a mut Sora,
        rt: Option<&'a mut PartyRuntime>,
        world: &'a ErWorld,
        camera: &'a kh_camera::CameraView,
        fx: &'a mut crate::fx::Fx,
        dt: f32,
        now: f64,
        /// (REVENGE) the copies' revenge rules of this frame
        revenge_rules: (crate::bossrevenge::BossRules, f32),
    }

    /// The boss's summon: asked, adopted (the boss's team, hidden for its Mimic), stepped (its Mimic: the hostile copy with
    /// the spirit's time), gone with the boss.
    fn step_foes(fight: &mut Fight, rigs: &mut Vec<Box<Rig>>, m: FoeStep<'_>) {
        let FoeStep { sora, mut rt, world, camera, fx, dt, now, revenge_rules } = m;
        let boss_team = crate::targets::find_chr(&fight.handle).map_or(6, |c| c.team_type);
        let boss_at = match &fight.copy {
            Copy::Sora(c) => c.out.position,
            Copy::Friend(c) => c.boss.view().map_or(Vec3::ZERO, |v| v.position),
        };
        let leader = fight.leader;
        let mut keep = Vec::new();
        for foe in std::mem::take(&mut fight.foes) {
            match foe {
                Foe::Asked { before, waited, spec } => {
                    if fight.dead {
                        continue;
                    }
                    if waited < 0.0 {
                        // ask now (the creator is shared with the party's bodies: asked again until it is free)
                        let at = world.frame.pos_kh_to_er(boss_at + Vec3::new(150.0, 0.0, 0.0));
                        match crate::allybody::ask_foe(spec.chr_id, spec.npc_param, spec.think, spec.init, at) {
                            Ok(b) => keep.push(Foe::Asked { before: b, waited: 0.0, spec }),
                            Err(e) => {
                                erlog!("mimicboss: the summon waits ({e})");
                                keep.push(Foe::Asked { before, waited, spec });
                            }
                        }
                        continue;
                    }
                    let last = crate::allybody::last_created();
                    if last != before && game::plausible(last) {
                        // SAFETY: the creator's own last character, this frame; completeness checked before any module.
                        let chr = unsafe { &mut *(last as *mut ChrIns) };
                        if game::chr_complete(chr) {
                            chr.team_type = boss_team;
                            if spec.doping > 0 {
                                use eldenring::cs::ChrInsExt;
                                chr.apply_speffect(spec.doping, false);
                            }
                            let handle = chr.field_ins_handle;
                            erlog!("mimicboss: THE BOSS'S SUMMON IS IN THE FIELD: {} (team {boss_team}: the boss's side; HP {}/{})", spec.what, chr.modules.data.hp, chr.modules.data.max_hp);
                            match spec.mimic {
                                Some((level, renowned)) => match rigs.pop() {
                                    Some(r) => {
                                        let mut kit = kh_partyhost::mimic::copy_kit(&fight.leader_sora, level);
                                        kit.outfit = crate::boc::worn();
                                        let place = game::v3(&chr.modules.physics.position);
                                        let mut core = kh_partyhost::mimic::make_core(&kit, Pose { position: world.frame.pos_er_to_kh(place) + Vec3::Z * 5.0, yaw: 0.0 });
                                        core.set_body_scale(crate::boc::body_scale());
                                        let full = kh_core::command::summon::duration(renowned, level);
                                        erlog!("mimicboss: ITS MIMIC TEAR: {} ; the spirit's time {full:.0} s; it never summons", kh_partyhost::mimic::describe(&kit));
                                        let copy = Box::new(SoraCopy::new(core, r, SUMMON_COPY, kit.upgrade.damage));
                                        keep.push(Foe::Mimic { copy, handle, left: full, full, defense: kit.upgrade.defense, id: TargetId::NONE, stance: 0.0 });
                                    }
                                    None => {
                                        erlog!("mimicboss: !!! its Mimic Tear has no pose side (no rig read): its body is given back to the game");
                                        kill_body(&handle);
                                    }
                                },
                                None => keep.push(Foe::Spirit { handle, what: spec.what.clone() }),
                            }
                            continue;
                        }
                    }
                    if waited > 5.0 {
                        erlog!("mimicboss: !!! the summon {} did not appear in 5 s: given up", spec.what);
                        continue;
                    }
                    keep.push(Foe::Asked { before, waited: waited + dt, spec });
                }
                Foe::Ash(mut a) => {
                    if fight.dead {
                        crate::mimic_boss_ash::kill_all(&mut a);
                    }
                    let _ = crate::mimic_boss_ash::poll(&mut a, dt);
                    if a.is_over() {
                        erlog!("mimicboss: the boss's ash {} is over", a.what());
                    } else {
                        keep.push(Foe::Ash(a));
                    }
                }
                Foe::Spirit { handle, what } => {
                    if fight.dead {
                        erlog!("mimicboss: the boss is dead: its spirit {what} dies with it");
                        kill_body(&handle);
                        continue;
                    }
                    if crate::targets::find_chr(&handle).is_some_and(|c| c.modules.data.hp > 0) {
                        keep.push(Foe::Spirit { handle, what });
                    } else {
                        erlog!("mimicboss: the boss's spirit {what} is gone");
                    }
                }
                Foe::Mimic { mut copy, handle, left, full, defense, id, stance } => {
                    // (hotfix 2, THE USER: "the mimic summoned by the boss needs to use the boss' HP") its body's maximum = the boss's
                    // maximum (its own pool): its base written for it (its own SpEffects' rates kept), its HP filled once
                    if let Some(chr) = crate::targets::find_chr(&handle).filter(|c| game::chr_complete(c) && c.modules.data.hp > 0 && !fight.dead) {
                        let boss_max = fight.hp.1.max(1);
                        let data = chr.modules.data.as_mut();
                        let base = mb::base_for(boss_max, data.base_hp, data.max_hp);
                        if !fight.summon_hp_filled && (data.max_hp - boss_max).abs() * 20 > boss_max {
                            data.base_hp = base;
                        } else if !fight.summon_hp_filled {
                            data.hp = data.max_hp;
                            fight.summon_hp_filled = true;
                            erlog!("mimicboss: the boss's Mimic Tear has the boss's HP: {}/{} (the boss's maximum {boss_max})", data.hp, data.max_hp);
                        }
                    }
                    let body = crate::targets::find_chr(&handle);
                    let (hp, max) = body.as_ref().map_or((0, 1), |c| (c.modules.data.hp, c.modules.data.max_hp.max(1)));
                    let left = left - dt;
                    let ending = fight.dead || left <= 0.0 || hp <= 0;
                    if ending && !matches!(copy.phase, sm::phase::Phase::Leaving { .. }) {
                        erlog!("mimicboss: the boss's Mimic Tear LEAVES ({}): {}", if fight.dead { "its boss is dead" } else if hp <= 0 { "its HP are gone" } else { "its time is out" }, if hp <= 0 { "KH3's knockdown, then the silver" } else { "the silver dissolve" });
                        copy.phase = sm::phase::Phase::Leaving { t: 0.0, death: hp <= 0 };
                        kill_body(&handle);
                    }
                    let victim = match leader {
                        PartyChara::Sora => Some(mb::victim_of_sora(sora, copy.out.position)),
                        c => rt.as_deref().and_then(|r| mb::victim_of_friend(r, c, copy.out.position)),
                    };
                    let mut t = BossTargets { victims: victim.into_iter().collect(), hits: Vec::new(), attack_rate: fight.attack_rate, orders: Vec::new() };
                    let reactions = foe_reactions(&handle); // (MB3-COMBAT) the party's hits on it: its reactions
                    // (REVENGE, MIMIC-BOSS item 12) its stance broken by the party: its own KH3 knockdown, as the boss's copy's
                    if !fight.dead && hp > 0 && id != TargetId::NONE && stance_broke(id) {
                        let from = t.victims.first().map_or(copy.out.position - Vec3::X * 100.0, |v| v.feet);
                        copy.core.take_hit(mb::knockdown_of_sora(copy.out.position, from));
                        erlog!("mimicboss: the boss's Mimic Tear: the party broke its stance: its own KH3 knockdown asked (blow, down, get-up)");
                    }
                    // (REVENGE) the party's hits through its revenge: its points; its reaction by Sora's own player rules
                    let reactions = revenge_gate(&mut copy.revenge, &reactions, Gauge { rules: revenge_rules, stance, hp_max: max, dt, who: "the boss's Mimic Tear" });
                    step_sora_copy(&mut copy, SoraCopyStep { rt: rt.as_deref(), world, camera, targets: &mut t, fx, reactions: &reactions, er_hp: (hp, max), dead: false, held: crate::grabs::body_held(&handle), dt, now }, leader);
                    heal_body(&handle, std::mem::take(&mut copy.healed), "the boss's Mimic Tear"); // (hotfix 3) its heals, the same share
                    revenge_step(&mut copy.revenge, ending, dt, "the boss's Mimic Tear"); // (REVENGE)
                    let at = (copy.out.position, copy.out.yaw);
                    deliver(&mut t, sora, rt.as_deref_mut(), leader, at, "the boss's Mimic Tear", sora_stats(&copy.core));
                    let _ = (full, defense);
                    let over = matches!(copy.phase, sm::phase::Phase::Leaving { t, death } if t > if death { sm::phase::KO_HOLD + sm::phase::DEATH_FADE } else { sm::phase::MELT_END } + sm::phase::POOL_OUT + 0.2);
                    if !over {
                        keep.push(Foe::Mimic { copy, handle, left, full, defense, id, stance });
                    }
                }
            }
        }
        fight.foes = keep;
    }

    /// (MIMIC-BOSS hotfix 3, THE USER 2026-11-07: "anything the mimic boss or summon does that would heal it should heal it for
    /// the same % of HP that Sora would get healed by by using it") A copy's own heal of `share` of its KH3 maximum put on its
    /// Elden Ring body as the same share of the body's maximum (mb::healed_er_hp): the boss's 30 x pool, its summoned Mimic's.
    fn heal_body(handle: &FieldInsHandle, share: f32, who: &str) {
        if share <= 0.0 {
            return;
        }
        let Some(chr) = crate::targets::find_chr(handle) else { return };
        if !game::chr_complete(chr) {
            return;
        }
        let data = chr.modules.data.as_mut();
        let healed = mb::healed_er_hp(share, data.hp, data.max_hp);
        if healed != data.hp {
            erlog!("mimicboss: {who} HEALS ITSELF {:.1} % of its maximum (its KH3 core's own heal, the leader's share): Elden Ring HP {} -> {healed} of {}", share * 100.0, data.hp, data.max_hp);
            data.hp = healed;
        }
    }

    /// The game's death of a summon's body (its HP 0: its own death, the spirit's fade).
    fn kill_body(handle: &FieldInsHandle) {
        if let Some(chr) = crate::targets::find_chr(handle) {
            if game::chr_complete(chr) && chr.modules.data.hp > 0 {
                chr.modules.data.hp = 0;
            }
        }
    }

    // ------------------------------------------------------------------------------------------------ the bodies

    /// The boss's body (and its Mimic's) given back to the game: shown again (the game's death hides it itself).
    fn release(chr: &mut ChrIns) {
        let _ = crate::puppet::ai_switch::set_ai_on(chr, true);
        chr.debug_flags.set_disabled_movement(false);
        chr.debug_flags.set_disabled_secondary_actions(false);
        let physics = chr.modules.physics.as_mut();
        physics.gravity_disabled = false;
        physics.motion_multiplier = 1.0;
    }

    /// Holds a body at a copy's feet: hidden, no AI, no movement or action of its own, its place and facing ours.
    fn hold(chr: &mut ChrIns, frame: &Frame, feet_kh: Vec3, yaw: f32) {
        if !game::chr_complete(chr) {
            return;
        }
        chr.chr_flags1c5.set_enable_render(false);
        if crate::puppet::ai_switch::ai_bit(chr) == Some(true) {
            let _ = crate::puppet::ai_switch::set_ai_on(chr, false);
        }
        chr.debug_flags.set_disabled_movement(true);
        chr.debug_flags.set_disabled_secondary_actions(true);
        let er = frame.pos_kh_to_er(feet_kh);
        if !er.is_finite() {
            return;
        }
        let q = game::quaternion(crate::coords::quat_er_from_yaw(yaw));
        let physics = chr.modules.physics.as_mut();
        physics.position = game::hp(er);
        physics.orientation = q;
        physics.interpolated_orientation = q;
        physics.gravity_disabled = true;
        physics.motion_multiplier = 0.0;
    }

    /// BOTH PHYSICS PHASES (sora.rs, as the ally bodies): the boss's body and its Mimic's at their copies.
    pub fn write_bodies(frame: Frame) {
        let Ok(mut g) = STATE.try_lock() else { return };
        let Some(fight) = g.as_mut().and_then(|s| s.fight.as_mut()) else { return };
        if !fight.dead {
            let at = match &fight.copy {
                Copy::Sora(c) => Some((c.out.position, c.out.yaw)),
                Copy::Friend(c) => c.boss.view().map(|v| (v.position, v.yaw)),
            };
            if let (Some((p, y)), Some(chr)) = (at, crate::targets::find_chr(&fight.handle)) {
                if chr.modules.data.hp > 0 {
                    hold(chr, &frame, p, y);
                }
            }
        }
        for foe in &fight.foes {
            if let Foe::Mimic { copy, handle, .. } = foe {
                if let Some(chr) = crate::targets::find_chr(handle).filter(|c| c.modules.data.hp > 0) {
                    hold(chr, &frame, copy.out.position, copy.out.yaw);
                }
            }
        }
    }

    /// (MIMIC-BOSS hotfix 2) THE COPY'S OWN KH3 DEFENCE for a party hit on its body (targets.rs `defense`): a Sora copy's (or its
    /// summoned Mimic's) from its status (`mb::copy_defense`), a friend copy's from its party (kh-party `Party::defender`: the
    /// friend's own Defense and resistances). None: not a held body.
    pub fn copy_defense_of(handle: &FieldInsHandle) -> Option<kh_core::host::TargetDefense> {
        let bits = game::handle_bits(handle);
        let g = STATE.try_lock().ok()?;
        let fight = g.as_ref()?.fight.as_ref()?;
        if game::handle_bits(&fight.handle) == bits {
            return match &fight.copy {
                // (MIMIC-BOSS hotfix 3, MB3-COMBAT) its own resists, no second Defense (mb::copy_as_foe)
                Copy::Sora(c) => Some(mb::copy_as_foe(mb::copy_defense(c.core.status()))),
                Copy::Friend(c) => {
                    let d = c.boss.rt.party.defender(c.boss.id()?)?;
                    Some(mb::copy_as_foe(kh_core::host::TargetDefense { defense: d.defense, max_hp: d.max_hp, hp_percent: d.hp_percent, enemy_kind: false, damage_min: d.damage_min, damage_max: d.damage_max, resist: d.resist, bio_type: d.bio_type }))
                }
            };
        }
        fight.foes.iter().find_map(|f| match f {
            Foe::Mimic { copy, handle, .. } if game::handle_bits(handle) == bits => Some(mb::copy_as_foe(mb::copy_defense(copy.core.status()))), // (MB3-COMBAT)
            _ => None,
        })
    }

    // ------------------------------------------------------------------------------------------------ the party's hits

    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT) The party's hits on the boss's summoned Mimic Tear (its body's handle bits), to become its
    /// reactions at its next step.
    static FOE_REACTIONS: Mutex<Vec<(u64, HitRecord)>> = Mutex::new(Vec::new());

    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT) The hits waiting for the summoned Mimic Tear on `handle` (taken).
    fn foe_reactions(handle: &FieldInsHandle) -> Vec<HitRecord> {
        let bits = game::handle_bits(handle);
        let mut all = FOE_REACTIONS.lock().unwrap_or_else(|e| e.into_inner());
        let mine = all.iter().filter(|(b, _)| *b == bits).map(|(_, h)| *h).collect();
        all.retain(|(b, _)| *b != bits);
        mine
    }

    /// (targets.rs `apply_hit`, before the game's damage) A PARTY HIT ON THE BOSS'S BODY (or its Mimic's): a Goofy copy may
    /// parry it (KH3's Goofy rules: `FriendBoss::defend`): then it is answered here and nothing reaches the game; else it goes
    /// on to the game and becomes the copy's KH3 reaction at its next step.
    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT) `body`: the hit body's handle bits (game::handle_bits): a hit on the boss's summoned
    /// Mimic Tear becomes ITS reaction (it took none before: every party hit, projectiles too, reaches every copy).
    pub fn before_party_hit(hit: &HitRecord, kh_hp: f32, body: u64) -> Option<kh_core::host::HitResult> {
        let Ok(mut g) = STATE.try_lock() else { return None };
        let fight = g.as_mut()?.fight.as_mut()?;
        if fight.dead {
            return None;
        }
        if body != 0 && fight.foes.iter().any(|f| matches!(f, Foe::Mimic { handle, .. } if game::handle_bits(handle) == body)) {
            FOE_REACTIONS.lock().unwrap_or_else(|e| e.into_inner()).push((body, *hit));
            return None;
        }
        if hit.target == fight.id {
            if let Copy::Friend(c) = &mut fight.copy {
                let d = c.boss.defend(fight.leader, hit);
                if d.negates() {
                    erlog!("mimicboss: the {} copy {d:?} a party hit ({}): nothing reaches the game", c.boss.chara.name(), hit.attack_name);
                    return Some(d.result(kh_hp));
                }
            }
            fight.reactions.push(*hit);
        }
        None
    }

    // ------------------------------------------------------------------------------------------------ the drawing

    /// (spirit_mimic.rs `draw`) The Sora copies of this frame. (MIMIC-RENDER, THE USER 2026-11-05: "Sora's copy should only be
    /// silver for three seconds like the rest") handed to kh-er-render's Sora copies (sora_copy.rs): the boss's copy silver while
    /// Elden Ring's liquid silver is on it (forming, then SpEffect 16316's 3 s; the game has no fade of it), then his KH3 look at
    /// full quality with his own Keyblade and form weapon parts; the boss's summoned Mimic Tear silver for good (NpcParam
    /// 100000010 carries 16316 resident, as the player's Mimic). The list returned for the Mimic route is empty.
    pub fn mimic_draws(control: &Control, alpha: f32) -> Vec<kh_er_render::mimic::MimicDraw> {
        let Ok(mut g) = STATE.try_lock() else { return Vec::new() };
        let Some(fight) = g.as_mut().and_then(|s| s.fight.as_mut()) else {
            kh_er_render::sora_copy::submit(Vec::new());
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut one = |c: &mut SoraCopy, silver: f32| {
            if !c.pose.component.is_empty() {
                let m = sm::mimic_draw_of(sm::MimicPose { rig: &c.rig, out: (&c.out_before, &c.out), pose: (&c.pose_before, &c.pose), chain: (&c.chain_before, &c.chain), weapon: c.weapon }, alpha, control, &mut c.tall, sm::phase::form(c.phase));
                out.push(copy_draw_of(c, m, silver, alpha /* (KEYCHAIN-DUAL) */));
            }
        };
        if let Copy::Sora(c) = &mut fight.copy {
            let silver = kh_er_render::sora_copy::silver_after_formed(c.formed_for);
            one(c, silver);
        }
        for foe in fight.foes.iter_mut() {
            if let Foe::Mimic { copy, .. } = foe {
                one(copy, 1.0);
            }
        }
        kh_er_render::sora_copy::submit(out);
        Vec::new()
    }

    /// (MIMIC-RENDER) A Sora copy as the renderer's copy: the Mimic draw's pose, Keyblade and forming, his own keyblade, wear form
    /// and form weapon parts (in place of the Keyblade when he has them), the silver weight.
    fn copy_draw_of(c: &SoraCopy, m: kh_er_render::mimic::MimicDraw, silver: f32, alpha: f32) -> kh_er_render::sora_copy::SoraCopyDraw {
        let keyblade = c.core.status().current_keyblade().map_or("WEP_KEYBLADE_SO_00", |k| k.key()).to_string();
        let mut forms = copy_form_parts(c, &keyblade, &m.body);
        c.rig.chains.parts.apply(&mut forms, alpha); // (KEYCHAIN-DUAL) each Keyblade part with its own keychain (sm::chain_of)
        let weapon = if forms.is_empty() { m.weapon } else { None };
        kh_er_render::sora_copy::SoraCopyDraw { body: m.body, weapon, forms, keyblade: Some(keyblade), outfit: None, wear_form: Some(c.core.combat().form.wear_form), silver, form: m.form, owner: Some(c.member + 1) /* (MIMIC-BOSS drives fix) his own form glow */ }
    }

    /// (MIMIC-RENDER) A copy's form weapon parts on his drawn pose: sora.rs's rule for a form's equipped parts (the units KH3's
    /// equipment holds: the Keyblade until StyleChange's equip, then the transform units, the form's own after), with HIS form.
    /// Empty: none (the Keyblade stays in his hand). A part whose model the renderer has not loaded is not drawn.
    fn copy_form_parts(c: &SoraCopy, keyblade: &str, body: &kh_er_render::Body) -> Vec<kh_er_render::FormWeapon> {
        use kh_core::combat::form::FormEquip;
        let form = &c.core.combat().form;
        // (FORMWEP-FIX) a form just started, its transformation not begun: his hand Keyblade (kh-host formstart.rs)
        let Some(row) = form.row().filter(|r| r.weapon_form_value > 1 && form.equip != FormEquip::Keyblade && !c.rig.form_gate.on()) else { return Vec::new() };
        let grow = form.grow.max(1).to_string();
        let transform = form.equip == FormEquip::Transform;
        let style_change = c.out.state == kh_core::StateId::Stylechange;
        if kh_er_render::keyblade::equipped_parts(keyblade, row.weapon_form, &grow, transform).is_empty() || !(sm::weapon_opacity(c.weapon) > 0.0 || style_change) {
            return Vec::new();
        }
        let skeleton = c.rig.animator.skeleton();
        let index_of = |b: &str| skeleton.index_of(b);
        let socket = sm::weapon_socket();
        let anim = kh_core::combat::forms::weapon_logical(form.cmd, c.out.anim.name).or_else(|| c.out.anim.name.sequences().first().copied());
        let pose = kh_er_render::keyblade::FormPose { index_of: &index_of, component: &body.component, to_world: &body.component_to_world, anim, time: c.out.anim.time, hand_socket: &socket };
        kh_er_render::keyblade::draw_equipped_parts(keyblade, row.weapon_form, &grow, transform, &pose)
    }

    /// (partydraw.rs) A friend leader's copy of this frame for the party's drawing (his own look, as the friends are drawn).
    pub fn friend_draws(alpha: f32, glow_mask: bool) -> Vec<kh_er_render::party::CharDraw> {
        let Ok(g) = STATE.try_lock() else { return Vec::new() };
        let Some(fight) = g.as_ref().and_then(|s| s.fight.as_ref()) else { return Vec::new() };
        let Copy::Friend(c) = &fight.copy else { return Vec::new() };
        let id = kh_partyhost::poser::draw_id(c.boss.chara);
        let Some(posed) = c.poser.pose(id, alpha) else { return Vec::new() };
        if !posed.component_to_world.translation.is_finite() {
            return Vec::new();
        }
        let weapon = posed.weapon.as_ref().filter(|w| w.2).map(|(model, root, _)| kh_er_render::party::PartyWeapon { model: model.clone(), to_world: root.then(&posed.component_to_world), component: Vec::new(), opacity: 1.0 });
        let blood = crate::blood::party::party_look(c.boss.chara).unwrap_or_default();
        vec![kh_er_render::party::CharDraw { chara: id, body: Some(kh_er_render::Body { component_to_world: posed.component_to_world, local: posed.local, component: posed.component }), weapon, props: Vec::new(), glow_mask, blood }]
    }

}

#[cfg(all(test, feature = "party"))]
mod tests {
    /// (MIMIC-BOSS hotfix 2) the sheet row of the boss's HP (THE USER: "x30 the health then") is read as the code reads it.
    #[test]
    fn mimicboss_the_sheet_row_of_the_boss_hp_is_30() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sheets/port/mimic-boss.json");
        let text = std::fs::read_to_string(path).expect("the sheet");
        assert_eq!(super::sheet_value(&text, "boss.hp_x_leader"), Some(30.0));
        assert_eq!(super::sheet_value(&text, "no.such"), None);
    }

    /// (MIMIC-BOSS hotfix 3, MB3-AGGRO) every aggression / counter row of the sheet is read as the host reads it (0 allowed),
    /// and the sheet as written gives the built-in boss defaults.
    #[test]
    fn mimicboss_the_sheet_rows_of_the_copies_aggression_are_read() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sheets/port/mimic-boss.json");
        let text = std::fs::read_to_string(path).expect("the sheet");
        let (a, taken) = kh_partyhost::mimicboss::CopyAggro::from_rows(|id| super::sheet_number(&text, id));
        assert_eq!(taken.len(), kh_partyhost::mimicboss::CopyAggro::ROWS.len(), "every row is in the sheet");
        assert_eq!(a, kh_partyhost::mimicboss::CopyAggro::default());
        assert_eq!(super::sheet_number(r#"{"rows":[["aggro.on",0]]}"#, "aggro.on"), Some(0.0), "a switch's 0 is read");
    }

    use kh_host::party::PartyChara;
    use kh_partyhost::mimicboss::{self as mb, FightFacts, SoloEnd};

    /// (MIMIC-BOSS) the host's own rule over the engine-free one: the leader down in the arena keeps the solo fight until the
    /// respawn puts him outside it; a win ends it at once; leaving ends it.
    #[test]
    fn mimicboss_host_ends_the_solo_fight_on_win_or_outside_never_on_the_death_screen() {
        let ends = |f: &FightFacts| !mb::fight_on(f) && (f.defeated || !f.in_arena);
        let on = FightFacts { defeated: false, fog_passed: true, in_arena: true, leader_alive: true };
        assert!(!ends(&on));
        assert!(!ends(&FightFacts { leader_alive: false, ..on }), "dead in the arena: held (the death screen)");
        assert!(ends(&FightFacts { leader_alive: false, in_arena: false, ..on }), "respawned outside: the friends come back");
        assert!(ends(&FightFacts { defeated: true, ..on }), "a win: at once");
        assert_eq!(SoloEnd::of(&FightFacts { defeated: true, ..on }), SoloEnd::Win);
        let _ = PartyChara::Sora;
    }
}
