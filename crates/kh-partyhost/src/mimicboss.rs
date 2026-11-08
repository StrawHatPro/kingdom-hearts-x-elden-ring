//! (MIMIC-BOSS, THE USER 2026-11-05: "for the mimic tear boss fight, it needs to mimic whoever is party leader and actually
//! fight the party"; "make it so that the mimic tear boss is a solo fight for the party leader, removing their allies upon
//! going thru the fog gate, and the allies only come back if the player dies or wins"; "summoning spirit ashes works in the
//! mimc tear's boss fight, but any summoned outside of the boss room get wiped as well entering the fog gate with the rest of
//! the party (they don't return when your allies return) and the mimic tear boss is able to use the spirit ashes you have
//! equipped and can summon them itself. This is only true for the boss"; "this all needs to apply to the Stray Mimic Tear as
//! well in the Haligtree") THE MIMIC TEAR BOSSES, THE ENGINE-FREE HALF. kh-er's mimic_boss.rs is the Elden Ring side (the
//! boss's body, its flags, the draw); this module holds the rules that need no game:
//!
//! - [`MIMIC_BOSSES`]: THE ONE SHARED LIST of the two bosses (every rule here is gated to it; no other enemy has any of it),
//!   as traced from Elden Ring 2.7.2.0's own data (build/sora_tmp/weapons/MIMIC-BOSS/STATE.md section 1);
//! - THE SOLO FIGHT ([`SoloHold`], `PartyRuntime::solo_begin` / `solo_end`): the party's own formation path takes every friend
//!   out at the fog gate (their statuses kept as they were) and puts them back on the leader's death or the boss's defeat;
//!   while it holds, no formation change is taken (Leadership::set_formation refuses it) and a save keeps the formation from
//!   before the fight (save.rs `to_save`); a load drops the hold (the save's own formation is the party then);
//! - THE COPY ([`copy_sora`]): the leader Sora as he is at the fight's start (a clone of his core: his outfit, wear form, form,
//!   Keyblade, abilities, magic, shotlocks, items), whole again, never a live link; a friend leader is copied by
//!   [`FriendBoss`] (a second party runtime holding one AI member with the leader's own status);
//! - THE HOSTILE SIDE ([`BossTargets`]): the party as the copy's only enemies (its targets, its AI's enemy list), its hits
//!   collected for the host's damage path (Sora's `take_hit`, a friend's `receive_hit`), x the boss's Elden Ring attack rate;
//! - ITS SUMMON ([`summon_due`]): the player's equipped ash, once a fight, at Elden Ring's own phase point of these bosses.

use kh_core::host::{HitResult, HurtShape, TargetDefense, TargetFlags, TargetInfo};
use kh_core::{HitRecord, Pose, Sora, TargetId, Vec3};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;

use crate::aiworld::PartyTargets;
use crate::runtime::PartyRuntime;
use crate::say;

// ============================================================================================ the two bosses (traced)

/// One Mimic Tear boss as Elden Ring's data has it (regulation and maps 2.7.2.0, read offline; STATE.md section 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MimicBossRow {
    pub key: &'static str,
    pub name: &'static str,
    /// The map (area, block) of its arena: m12_02_00_00 / m30_20_00_00.
    pub area: u8,
    pub block: u8,
    /// Its MSB part's NpcParam (unique to this part in every map: build/sora_tmp/weapons/P9a/enemies.json).
    pub npc_param: i32,
    /// Its MSB part's entity id, which is also its defeat flag (its event's SetEventFlag of the same number).
    pub entity: u32,
    pub defeat_flag: u32,
    /// The flag Elden Ring sets when the player has passed the fog gate (the map's own copy of common event 9005800).
    pub fog_flag: u32,
    /// The fog's region (the passage) and, Nokron only, the region of the first encounter that starts the fight without a fog.
    pub fog_region: u32,
    pub first_region: Option<u32>,
    /// (MIMIC-BOSS) the first encounter's flag its event sets when the leader walks into that region (12022860: 12020851).
    pub first_flag: Option<u32>,
    /// The silver pool it rises from (c3320, its own entity) and where the pool stands (MSB, map-local metres).
    pub pool_entity: u32,
    pub pool_at: [f32; 3],
    /// Its own MSB place (its nest, where the game keeps the body disabled before the fight): the host's calibration of the
    /// physics frame against the MSB's places.
    pub boss_at: [f32; 3],
    /// The arena about the pool: horizontal radius and height band (m), OUR measure of "in the arena" (the game has no
    /// region for the whole room of either boss; Nokron's first-encounter box is 52 x 65 m about a point 20 m from the pool;
    /// the Stray's room runs from its fog, 22.7 m from the pool, to its own place 35 m beyond it). The fight also needs the
    /// fog flag, so the gate's outside (within the radius) never starts it.
    pub arena_radius: f32,
    pub arena_height: f32,
    /// Its boss bar's name (2003[11] DisplayBossHealthBar's NpcName id).
    pub bar_name: i32,
    /// Its NpcParam's GameClearSpEffectID (journey 2+) and that row's physicsAttackPowerRate (= magicAttackPowerRate).
    pub game_clear_speffect: i32,
    pub game_clear_attack: f32,
}

/// THE ONE SHARED LIST (the user: "only true for the boss"; "this all needs to apply to the Stray Mimic Tear as well"). Both
/// copy the player with the same instruction (EMEVD 2004[67] CopyPlayerCharacterData(10000, entity)), rise from a c3320 pool
/// (its SpEffect 16307 starts them, anim 63010, SpEffect 16316 the 3 s liquid silver), have NpcParam hp 1551 and SpEffect
/// 19397 resident (maxHpRate 3: three times the copied player's HP).
pub const MIMIC_BOSSES: [MimicBossRow; 2] = [
    MimicBossRow {
        key: "mimic_tear",
        name: "Mimic Tear",
        area: 12,
        block: 2,
        npc_param: 526_100_965,
        entity: 12_020_850,
        defeat_flag: 12_020_850,
        fog_flag: 12_022_855,
        fog_region: 12_022_850,
        first_region: Some(12_022_851),
        first_flag: Some(12_020_851),
        pool_entity: 12_020_860,
        pool_at: [1001.327, -617.556, 1142.989],
        boss_at: [1029.647, -641.311, 1174.018],
        arena_radius: 40.0,
        arena_height: 15.0,
        bar_name: 903_320_000,
        game_clear_speffect: 19_509,
        game_clear_attack: 1.2,
    },
    MimicBossRow {
        key: "stray_mimic_tear",
        name: "Stray Mimic Tear",
        area: 30,
        block: 20,
        npc_param: 526_100_052,
        entity: 30_200_800,
        defeat_flag: 30_200_800,
        fog_flag: 30_202_805,
        fog_region: 30_202_800,
        first_region: None,
        first_flag: None,
        pool_entity: 30_200_801,
        pool_at: [-89.689, 1278.319, -264.391],
        boss_at: [-88.65, 1276.79, -299.61],
        arena_radius: 45.0,
        arena_height: 12.0,
        bar_name: 903_320_300,
        game_clear_speffect: 19_516,
        game_clear_attack: 1.134,
    },
];

/// The boss of an NpcParam id (the gate: None for every other character of the game).
pub fn boss_of_npc(npc_param: i32) -> Option<&'static MimicBossRow> {
    MIMIC_BOSSES.iter().find(|b| b.npc_param == npc_param)
}

/// The boss of an entity id.
pub fn boss_of_entity(entity: u32) -> Option<&'static MimicBossRow> {
    MIMIC_BOSSES.iter().find(|b| b.entity == entity)
}

/// (MIMIC-BOSS, the user's run 2026-11-06 at Nokron: "SORA DID NOT APPEAR", the friends stayed) Beyond the arena's radius by
/// this much more (m) the leader has LEFT the fight (the margin keeps a fight that runs from ending at the edge).
pub const AWAY_MARGIN_M: f32 = 15.0;

impl MimicBossRow {
    /// THE ARENA, LIVE (the rule in force): the leader is in the fight's room when he stands within the arena's radius and
    /// height band of the BOSS'S OWN BODY, both places read from the game in the same frame (Elden Ring's physics places, m,
    /// y up). The user's run 2026-11-06: the leader at (10.0, -1.5, -14.0), the boss at (17.33, -1.50, -17.01), both
    /// map-local, while the MSB's places (the nest 1029.65, -641.31, 1174.02) are in another frame: the MSB box said "out".
    pub fn near_boss(&self, boss: [f32; 3], leader: [f32; 3]) -> bool {
        let (dx, dy, dz) = (leader[0] - boss[0], leader[1] - boss[1], leader[2] - boss[2]);
        (dx * dx + dz * dz).sqrt() <= self.arena_radius && dy.abs() <= self.arena_height
    }

    /// The leader has left the fight (a respawn elsewhere, a warp): beyond the arena's measure of the live boss body by
    /// [`AWAY_MARGIN_M`].
    pub fn away_from_boss(&self, boss: [f32; 3], leader: [f32; 3]) -> bool {
        let (dx, dy, dz) = (leader[0] - boss[0], leader[1] - boss[1], leader[2] - boss[2]);
        (dx * dx + dz * dz).sqrt() > self.arena_radius + AWAY_MARGIN_M || dy.abs() > self.arena_height + AWAY_MARGIN_M
    }

    /// A point (MSB metres, Elden Ring axes: y up) is in its arena by the MSB's own places. NOT the host's rule (the game's
    /// physics frame is not the MSB's: [`MimicBossRow::near_boss`]); kept for the data's own checks.
    pub fn in_arena(&self, at: [f32; 3]) -> bool {
        let (dx, dy, dz) = (at[0] - self.pool_at[0], at[1] - self.pool_at[1], at[2] - self.pool_at[2]);
        (dx * dx + dz * dz).sqrt() <= self.arena_radius && dy.abs() <= self.arena_height
    }
}

// ============================================================================================ the fight's state

/// Where the fight stands, from the game's facts of one frame (the host reads them; pure so that every edge is tested).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FightFacts {
    /// The boss's defeat flag.
    pub defeated: bool,
    /// The fog-pass flag (or, Nokron, the first encounter's start flag 12020851 seen set this load while the leader is in).
    pub fog_passed: bool,
    /// The leader's body is in the boss's map block and in its arena.
    pub in_arena: bool,
    /// The leader is alive (HP above 0, no game over running).
    pub leader_alive: bool,
}

/// The fight runs: the leader passed the fog (or started the first encounter), stands in the arena, lives, the boss is not
/// beaten. Level-triggered on purpose: a death, a win, a warp, a load or a quit each makes it false by itself.
pub fn fight_on(f: &FightFacts) -> bool {
    f.fog_passed && f.in_arena && f.leader_alive && !f.defeated
}

/// Why a solo fight ended (the log, the tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoloEnd {
    /// The leader died / the game over: the friends come back at the respawn point.
    Death,
    /// The boss's defeat flag is set.
    Win,
    /// The leader is no longer in the arena (a warp, a load, a quit, a fall): never stranded.
    Away,
}

impl SoloEnd {
    pub fn of(f: &FightFacts) -> SoloEnd {
        if f.defeated {
            SoloEnd::Win
        } else if !f.leader_alive {
            SoloEnd::Death
        } else {
            SoloEnd::Away
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SoloEnd::Death => "the leader died",
            SoloEnd::Win => "the boss is defeated",
            SoloEnd::Away => "the leader left the arena (a warp, a load, a quit)",
        }
    }
}

// ============================================================================================ the solo fight

/// THE PARTY AS IT WAS AT THE FOG: the formation (who was in, who led), every friend's status (HP, MP, equipment, gauges,
/// exactly), AI Sora in or out. Held in `Leadership::solo` while the fight runs.
#[derive(Clone, Debug, PartialEq)]
pub struct SoloHold {
    pub boss: &'static str,
    pub before: Formation,
    pub leader: PartyChara,
    pub friends: Vec<(PartyChara, kh_party::FriendStatus)>,
    /// A friend led with AI Sora in the party (Sora's own core keeps his state while he is out: it is not stepped).
    pub sora_was_in: bool,
}

impl SoloHold {
    /// The formation of the solo fight: the leader alone.
    pub fn solo_formation(&self) -> Formation {
        Formation::new(&[self.leader], self.leader).unwrap_or_default()
    }

    /// Everyone who was taken out (for the log and the HUD's lock).
    pub fn taken_out(&self) -> Vec<PartyChara> {
        let mut out: Vec<PartyChara> = self.friends.iter().map(|f| f.0).collect();
        if self.sora_was_in && self.leader != PartyChara::Sora {
            out.push(PartyChara::Sora);
        }
        out
    }
}

impl PartyRuntime {
    /// The solo fight held now (None: the party is as usual).
    pub fn solo(&self) -> Option<&SoloHold> {
        self.lead.solo.as_ref()
    }

    /// THE FOG GATE: every friend out (Donald, Goofy, AI Sora, any later member: the formation's own leave), the leader alone.
    /// Their statuses are kept in the hold. False: a solo fight is held already.
    pub fn solo_begin(&mut self, sora: &mut Sora, boss: &'static str, now: f64) -> bool {
        if self.lead.solo.is_some() {
            return false;
        }
        let leader = self.leader_chara();
        let friends: Vec<(PartyChara, kh_party::FriendStatus)> = PartyChara::FRIENDS.into_iter().filter(|c| *c != leader).filter_map(|c| self.id(c).and_then(|id| self.party.status(id)).map(|s| (c, s.clone()))).collect();
        let hold = SoloHold { boss, before: self.lead.formation.clone(), leader, friends, sora_was_in: leader != PartyChara::Sora && self.lead.sora_active };
        let solo = hold.solo_formation();
        let out = hold.taken_out();
        self.lead.formation = solo;
        self.lead.solo = Some(hold);
        self.apply_formation(sora, now);
        say!("party-play: t={now:.3}s (MIMIC-BOSS) {boss}: THE FOG GATE: a solo fight for {} ({} out of the field, their state kept)", leader.name(), if out.is_empty() { "nobody".to_string() } else { out.iter().map(|c| c.name()).collect::<Vec<_>>().join(", ") });
        true
    }

    /// THE FIGHT'S END (`why`): the formation from before the fog, every friend back beside the leader with the status he
    /// had (HP, MP, equipment). False: no solo fight was held.
    pub fn solo_end(&mut self, sora: &mut Sora, why: SoloEnd, now: f64) -> bool {
        let Some(hold) = self.lead.solo.take() else { return false };
        self.lead.formation = hold.before.clone();
        self.apply_formation(sora, now);
        // (the formation's own join adds a friend only beside other members: with the leader alone, each is joined here)
        let (at, yaw) = self.leader.and_then(|l| self.party.view(l)).map_or_else(|| (sora.output().position, sora.output().yaw), |v| (v.position, v.yaw));
        for (c, _) in &hold.friends {
            if hold.before.is_active(*c) && self.id(*c).is_none() {
                self.spawn_one(*c, at, yaw);
            }
        }
        let mut back = Vec::new();
        for (c, status) in &hold.friends {
            if let Some(s) = self.id(*c).and_then(|id| self.party.status_mut(id)) {
                *s = status.clone();
                back.push(c.name());
            }
        }
        if hold.sora_was_in && self.lead.sora_active {
            back.push("AI Sora");
        }
        self.current = self.views();
        self.previous = self.current.clone();
        self.ai_dirty = true;
        say!("party-play: t={now:.3}s (MIMIC-BOSS) {}: the solo fight ends ({}): {} back beside {}", hold.boss, why.name(), if back.is_empty() { "nobody".to_string() } else { back.join(", ") }, hold.leader.name());
        true
    }

    /// A load drops the hold (the save's own formation is the party now; a save made during the fight kept the formation
    /// from before it). True: one was held.
    pub fn solo_drop(&mut self, why: &str) -> bool {
        let Some(hold) = self.lead.solo.take() else { return false };
        say!("party-play: (MIMIC-BOSS) {}: the solo fight's hold is dropped ({why}): the loaded formation stands", hold.boss);
        true
    }
}

// ============================================================================================ the copy

/// THE COPY OF THE LEADER SORA at the fight's start (Elden Ring's CopyPlayerCharacterData copies once; never a live link): a
/// clone of his core (his outfit, his wear form and a form he is in with its clock, the Keyblade and its upgrade, armour,
/// accessories, abilities, magic and slots, shotlock, items, the command options), put at `at`, WHOLE (full HP, MP and focus,
/// no MP Charge), nothing of the moment (a damage state, invincibility, a buff's time, a meal, munny), the menu at its top,
/// and NO SUMMON of its own (its ashes are the boss's summon: [`summon_due`], the host's).
pub fn copy_sora(leader: &Sora, at: Pose) -> Sora {
    let mut core = leader.clone();
    core.teleport(at);
    // (HUD-REVIVE consistency, MIMIC-BOSS) a party member's KO from its first step (KO'd before its AI's first step it never
    // takes the leader's Die); the lying KO (M_0230), never AI Sora's game-over hover (a clone of AI Sora carries his flag)
    core.combat_mut().friend.sora_is_ai = true;
    core.combat_mut().friend.ko_game_over_pose = false;
    let st = core.status_mut();
    st.status_immunity = 0.0;
    st.in_damage_state = false;
    st.damage_invalid = false;
    st.invincible = false;
    st.hp_limit_one = false;
    st.combo_leave_timer = 0.0;
    st.mp_charging = false;
    st.mp_charge_timer = 0.0;
    st.focus_empty = false;
    st.item_effects = [0; 12];
    st.munny = 0;
    st.notes.clear();
    st.meal = None;
    st.abilities.set_meal(None);
    st.buff_level = 0;
    st.hp = st.max_hp();
    st.mp = st.max_mp();
    st.focus = st.focus_max;
    let c = core.command_mut();
    c.state = kh_core::command::MenuState::MainSel;
    (c.main_cursor, c.sub_cursor, c.list) = (0, 0, 0);
    (c.lock_timer, c.reaction_block) = (0.0, 0.0);
    (c.pending, c.last_executed, c.last_shoot_flow, c.executed) = (0, 0, 0, 0);
    c.slow_applied = false;
    c.requests.clear();
    c.events.clear();
    c.spirit_ashes = [0; kh_core::command::summon::SLOTS];
    c.summon = crate::mimic::summon_view();
    c.calling = false;
    core
}

/// One line for the log: what the copy took.
pub fn describe_sora(core: &Sora) -> String {
    let s = core.status();
    let form = &core.combat().form;
    format!(
        "Sora: level {}, HP {}, MP {}, Str {} Mag {} Def {}, Keyblade {} +{}, form cmd {} ({:.1} s left), wear form {}, {} magic slots",
        s.level,
        s.hp_max,
        s.mp_max,
        s.stats.strength,
        s.stats.magic,
        s.stats.defense,
        s.weapon,
        s.weapon_upgrade,
        form.cmd,
        form.time_left,
        form.wear_form,
        core.command().magic_slots.iter().filter(|k| **k != 0).count()
    )
}

/// THE COPY'S HP MIRRORS ELDEN RING'S: the boss's real HP is the game's (its bar, its death, its flag); the copy's KH3 core
/// shows the same share of its own maximum, never 0 while the game's body lives (its KO is the game's death).
pub fn mirrored_hp(er_hp: i32, er_max: i32, kh_max: i32) -> i32 {
    if er_hp <= 0 {
        return 0;
    }
    let share = er_hp as f32 / er_max.max(1) as f32;
    ((share * kh_max as f32).ceil() as i32).clamp(1, kh_max.max(1))
}

/// THE BOSS'S SUMMON, OUR RULE (Elden Ring's Mimic Tears never summon): once a fight, when its HP first falls to
/// [`SUMMON_AT`] of its maximum or below. 60 % is Elden Ring's own phase point of these bosses: the Stray Mimic Tear's event
/// 30202811 (4[2] IfCharacterHPRatio(boss, <=, 0.6)) sets its second-phase flag 30202802 there (the music's second part).
pub const SUMMON_AT: f32 = 0.6;

/// The summon is due now (`hp_share`: the boss's HP / its maximum; `done`: it summoned this fight already).
pub fn summon_due(hp_share: f32, done: bool) -> bool {
    !done && hp_share > 0.0 && hp_share <= SUMMON_AT
}

// ============================================================================================ the hostile side

/// The copy's victim ids (never an Elden Ring target id: those are small numbers of ErTargets' list).
pub const VICTIM_BASE: u32 = 0x4D42_0000;

/// The target id of a party member as the copy's victim.
pub fn victim_id(c: PartyChara) -> TargetId {
    TargetId(VICTIM_BASE | c as u32)
}

/// The party member of a victim id.
pub fn victim_chara(id: TargetId) -> Option<PartyChara> {
    if id.0 & 0xFFFF_0000 != VICTIM_BASE {
        return None;
    }
    [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy].into_iter().find(|c| *c as u32 == id.0 & 0xFFFF)
}

/// A party member as the copy sees it (the host fills it every step from the party: the leader of the solo fight).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Victim {
    pub chara: PartyChara,
    pub feet: Vec3,
    pub yaw: f32,
    pub velocity: Vec3,
    pub radius: f32,
    pub half_height: f32,
    pub hp: i32,
    pub hp_max: i32,
    /// His Defense (KH3's `Defense(victim)`): the copy's damage formula takes it (`Targets::defense`).
    pub defense: i32,
    pub airborne: bool,
    /// Down / KO / invincible now: no target. (MIMIC-STANCE) NOT while his stance is broken: a broken victim stays the
    /// copy's target (its Breaker's), as a BROKEN enemy stays the party's.
    pub down: bool,
    /// (MIMIC-STANCE) His attack as the copy sees it coming this step (its Goofy's parry reads it): [`victim_tell_of_sora`],
    /// [`victim_tell_of_friend`].
    pub tell: VictimTell,
    /// (MIMIC-STANCE) His stance now (the copy's `stagger_ask` answers from it): [`victim_stance_of_sora`],
    /// [`victim_stance_of_friend`].
    pub stance: VictimStance,
}

impl Victim {
    fn center(&self) -> Vec3 {
        self.feet + Vec3::Z * self.half_height
    }
}

/// THE COPY'S TARGETS: the party (its only enemies; Elden Ring's characters are not its business), its hits collected for
/// the host (`hits`), each x `attack_rate` (the boss's Elden Ring attack rate: 1, or its GameClearSpEffect's on a later
/// journey).
#[derive(Clone, Debug, Default)]
pub struct BossTargets {
    pub victims: Vec<Victim>,
    pub hits: Vec<HitRecord>,
    pub attack_rate: f32,
    /// (MIMIC-STANCE) What the copy's Stagger book asked of the party members this step (a break, its hold and end, a
    /// rebound off its Goofy's shield), for the host to carry out: [`stance_to_sora`], [`stance_to_party`].
    pub orders: Vec<StanceOrder>,
}

impl BossTargets {
    fn victim(&self, id: TargetId) -> Option<&Victim> {
        let c = victim_chara(id)?;
        self.victims.iter().find(|v| v.chara == c && !v.down && v.hp > 0)
    }
}

impl kh_core::Targets for BossTargets {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.victims.iter().filter(|v| !v.down && v.hp > 0).filter_map(|v| self.info(victim_id(v.chara))));
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        let v = self.victim(id)?;
        let mut flags = TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY | TargetFlags::PRIORITY_HIGH;
        if v.airborne {
            flags |= TargetFlags::AIRBORNE;
        }
        Some(TargetInfo { id, position: v.feet, center: v.center(), velocity: v.velocity, yaw: v.yaw, radius: v.radius, half_height: v.half_height, flags: TargetFlags(flags), hp: v.hp as f32, hp_max: v.hp_max as f32 })
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(v) = self.victim(id) {
            let reach = (v.half_height - v.radius).max(0.0);
            out.push(HurtShape { a: v.center() - Vec3::Z * reach, b: v.center() + Vec3::Z * reach, radius: v.radius, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        if self.victim(hit.target).is_none() {
            return HitResult::default();
        }
        let mut h = *hit;
        h.damage *= if self.attack_rate > 0.0 { self.attack_rate } else { 1.0 };
        self.hits.push(h);
        // (MIMIC-BOSS-KIT) the HP the hit takes (the host's damage path takes it next): KH3 fills the copy's own formchange
        // gauge only from a hit that dealt damage (kh-core combat/hit.rs: `damage_dealt > 0` -> style::add_hit); with 0 here
        // its forms, grand magic and finish levels never came
        let hp = self.victim(hit.target).map_or(0.0, |v| v.hp as f32);
        let dealt = h.damage.clamp(0.0, hp);
        HitResult { accepted: true, damage_dealt: dealt, hp_after: hp - dealt, killed: hp > 0.0 && dealt >= hp, ..HitResult::default() }
    }
    fn defense(&self, id: TargetId) -> Option<TargetDefense> {
        let v = self.victim(id)?;
        // the player's side of KH3's formula: no damage floor, the player's cap 255 x ... (Status::as_defender's numbers)
        Some(TargetDefense { defense: v.defense, max_hp: v.hp_max, hp_percent: 100.0 * v.hp as f32 / v.hp_max.max(1) as f32, enemy_kind: false, damage_min: 0, damage_max: 255, resist: [0; 8], bio_type: 0 })
    }
    fn engaged_count(&self) -> u32 {
        self.victims.iter().filter(|v| !v.down && v.hp > 0).count() as u32
    }
    /// (MIMIC-STANCE) A party member is a Small body for a Goofy's rules (`Size::Small`, kh-er's weight class of a human-sized
    /// character): no boss, no large body (`stagger.gauge` 100, `stagger.window` 2.5 s; Launch Breaker may throw him).
    fn size_class(&self, _id: TargetId) -> u8 {
        1
    }
    /// (MIMIC-STANCE) THE COPY'S STAGGER BOOK ASKS THE PARTY MEMBER, answered from his real stance ([`Victim::stance`]) and
    /// carried out by the host ([`BossTargets::orders`]): a break only on a member who is up and not broken yet; its hold and
    /// its end always; a Breaker never defeats him outright (his HP goes through his own damage path, Second Chance and all);
    /// Pin Down has no mind of his to hold; a perfect deflect's rebound is his KH3 recoil.
    fn stagger_ask(&mut self, id: TargetId, ask: kh_core::host::StaggerAsk) -> bool {
        use kh_core::host::StaggerAsk;
        let Some(v) = self.victim(id).copied() else { return false };
        let chara = v.chara;
        let order = match ask {
            StaggerAsk::Break { window } => {
                let ordered = self.orders.iter().any(|o| matches!(o, StanceOrder::Break { chara: c, .. } if *c == chara));
                if !v.stance.breakable || v.stance.broken > 0.0 || ordered || window <= 0.0 {
                    return false;
                }
                StanceOrder::Break { chara, seconds: window }
            }
            StaggerAsk::Hold { seconds } => StanceOrder::Hold { chara, seconds },
            StaggerAsk::Release { .. } => StanceOrder::Release { chara },
            StaggerAsk::MayDefeat | StaggerAsk::Delay { .. } => return false,
            StaggerAsk::Rebound { .. } => StanceOrder::Rebound { chara },
        };
        self.orders.push(order);
        true
    }
    fn hit_bone(&self, _id: TargetId, _at: Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

impl PartyTargets for BossTargets {
    fn ai_actors(&self, out: &mut Vec<kh_ai::ActorView>) {
        for v in &self.victims {
            out.push(kh_ai::ActorView {
                actor: kh_ai::ActorRef::Enemy(victim_id(v.chara).0),
                uid: -1,
                position: v.feet,
                center: v.center(),
                yaw: v.yaw,
                velocity: v.velocity,
                radius: v.radius,
                half_height: v.half_height,
                alive: v.hp > 0,
                hostile: !v.down,
                lockable: !v.down,
                airborne: v.airborne,
                hp: v.hp as f32,
                hp_max: v.hp_max as f32,
                ..kh_ai::ActorView::default()
            });
        }
    }
    fn attacked_by_enemies(&self) -> bool {
        false
    }
    fn fighting_party(&self, _id: u32) -> bool {
        true
    }
    /// (MIMIC-STANCE) The party members' attacks the copy sees coming ([`Victim::tell`]).
    fn telegraphs(&self, out: &mut Vec<kh_party::Telegraph>) {
        out.extend(self.victims.iter().filter(|v| !v.down && v.hp > 0 && v.tell.on).map(|v| v.tell.telegraph(v.chara)));
    }
}

/// A party member's hit on the boss as the copy's own KH3 reaction (Sora's damage states: flinch, blow, the air hit; his
/// HP is not taken here: the copy's HP mirrors the game's, [`mirrored_hp`]).
pub fn reaction_of(hit: &HitRecord) -> kh_core::states::HitOnPlayer {
    let i = &hit.info;
    kh_core::states::HitOnPlayer {
        direction: hit.direction,
        causer_yaw: hit.attacker.yaw,
        causer_radius: if hit.attacker.valid { hit.attacker.radius } else { 40.0 },
        power_scale: if i.power_scale > 0.0 { i.power_scale } else { 1.0 },
        brake: i.brake,
        parabola_angle: i.parabola_angle,
        move_length: i.move_length,
        effect_time: i.effect_time,
        src_power: if i.power > 0.0 { i.power } else { 1.0 },
        ignore_guard: i.flags50 & kh_core::host::HitInfo::FLAG50_IGNORE_GUARD != 0,
        rapid_fire: i.flags50 & kh_core::host::HitInfo::FLAG50_RAPID_FIRE != 0,
        reflect_level: i.reflect_level,
        reflect_reaction: i.reflect_reaction,
        // (FREEZE-FX, the user 2026-11-07: "make sure frozen effects are applied to anything that gets frozen by blizzard") the
        // row's bad status reaches the Sora copy as it reaches the friend copies (`copy_reaction_hit` keeps it): a party Blizzard
        // freezes him by KH3's own player rule (kh-core badstat, the freeze state: the row's time), wearing KH3's frozen look
        bad_status: i.bad_status,
        bad_time: i.bad_time,
        bad_param: i.bad_param,
        ..kh_core::states::HitOnPlayer::new(hit.damage_kind, 0.0, TargetId::NONE, if hit.attacker.valid { hit.attacker.position } else { hit.position - hit.direction * 100.0 })
    }
}

/// (MIMIC-BOSS hotfix 3, MB3-FREEZE; THE USER 2026-11-06: "I also froze in place for awhile after getting hit by the mimic
/// tear's blizzard spell") THE LONGEST TIME KH3 EVER PUTS EACH BAD STATUS ON THE PLAYER, by kind: the maximum
/// `m_BadStatusEffectTime` of the damaging rows (`m_Power` > 0, rate and time > 0) of KH3's player-facing attack tables,
/// `Content/DataTable/{Enemy, Enemy_Ship, Gimmick, Vehicle}` (sheets/entities/DataTable; the test
/// `mimicboss_status_tests::status_the_longest_player_side_times_are_kh3s_own_data` recomputes them from those files).
///
/// Why: the player's freeze is `TresPlayerState_BadStatFreeze` (123; Begin 0x14134E7A0, Tick 0x14150EDF0): the reaction
/// gate (iface slot 88 0x141439A50) writes the HIT ROW's own time into state+0x88; no input shortens it, only another
/// flinching hit ends it early, and nothing guards him afterwards (docs/sora-combat-spec.md 9.1, 9.9.3; kh-core
/// states/badstat_freeze.rs). So the time is the attacker's row. A copy attacks with SORA'S, DONALD'S and GOOFY'S rows
/// (p_ex001 / n_* tables), which KH3 wrote for hitting ENEMIES: MAG_BLIZZARD 5 s, MAG_BLIZZARA 7 s, MAG_BLIZZAGA /
/// BLIZZAGAN 10 s (Donald's book adds its own enemy-side freeze). KH3's rows that hit the PLAYER freeze him 0.5 - 3 s
/// (e_he902 IceBreath 0.5, e_ex360 Attack4_Blizzard 2, e_ex047 Attack3_Blizzaga 2.5, e_ex301 SwordThrow_Blizzagan 3).
/// e_ex053's power-0 `Attack1_BlizzaraRef_T` (10 s) is not a hit on the player (the Frozen flan minigame's reflected
/// Blizzara); its own Blizzara rows carry no status.
pub const PLAYER_STATUS_LONGEST: [(u8, f32); 8] = {
    use kh_core::combat::badstat::kind;
    [
        // (MIMIC-BOSS hotfix 3, the coordinator's decision 2026-11-07 on the user's "froze in place for awhile": the ENEMY-side
        // values of a Keyblade-wielding enemy; sheets/port/mimic-boss.json rows status.freeze_s / status.stun_s)
        (kind::FREEZE, 2.0),      // e_ex360_AttackDataTable Attack4_Blizzard (a Keyblade enemy's Blizzard; the longest on the player: 3, e_ex301)
        (kind::STUN, 8.0),        // e_ex721 Action1_Flash, the enemies' longest (a gimmick's: 10, g_bx_blowingcar GIM_ATTACK)
        (kind::BURN, 5.0),        // e_ex054_AttackDataTable Attack7_BerserkFireBless
        (kind::DISCHARGE, 10.0),  // e_ex018 Attack5_Cannon_Impact and 11 more, all 10
        (kind::CLOUD, 20.0),      // e_ex352 Attack14_SneezeChaosFiraga
        (kind::SNEEZE, 15.0),     // e_ex061 Attack2_Bomb and 6 more, all 15
        (kind::HP_BREAK, 30.0),   // e_ex367 Attack17_KBChangeSlashUp
        (kind::TIME_BREAK, 10.0), // e_ex367 Attack17_KBChangeSlash
    ]
};

/// (MIMIC-BOSS hotfix 3, MB3-FREEZE) A COPY'S BAD STATUS AS KH3 PUTS IT ON THE PLAYER (Sora's `take_hit` and a friend
/// leader's `receive_hit` then run their own KH3 rules: the freeze state, a stun's stick flicks and its cure by any damaging
/// hit, the abilities' guards, a form's block). The time is at most KH3's longest of that kind on the player
/// ([`PLAYER_STATUS_LONGEST`]); a status no KH3 attack ever puts on the player (HONEY, STOP, ENERGYBURST: only Sora's own
/// rows carry them) is not put on him; the kinds that are no pawn status (1 - 5, 9 - 11; ApplyBadStatus 0x1405B7C50 drops
/// them) pass as they are. Returns the status time before and after when it changed (for the host's log).
pub fn player_side_status(hit: &mut HitRecord) -> Option<(f32, f32)> {
    let k = kh_core::combat::badstat::kind_of(hit.info.bad_status);
    if k == 0 || matches!(k, 1..=5 | 9..=11) {
        return None;
    }
    let before = hit.info.bad_time;
    match PLAYER_STATUS_LONGEST.iter().find(|(kk, _)| *kk == k) {
        Some(&(_, longest)) => hit.info.bad_time = before.min(longest),
        None => hit.info.clear_bad_status(),
    }
    (hit.info.bad_time != before).then_some((before, hit.info.bad_time))
}

/// (MIMIC-BOSS hotfix 3, MB3-FREEZE) The copy's hit on a FRIEND leader (his `receive_hit`): its status as on the player.
pub fn party_hit_of(hit: &HitRecord) -> HitRecord {
    let mut h = *hit;
    let _ = player_side_status(&mut h);
    h
}

/// The copy's hit on the leader Sora (the party's damage path for Sora: his own `take_hit`, his guard, Second Chance ...).
/// (MIMIC-BOSS hotfix 3, MB3-FREEZE) its status as KH3 puts it on the player ([`player_side_status`]).
pub fn player_hit_of(hit: &HitRecord, boss_at: Vec3, boss_yaw: f32) -> kh_core::states::HitOnPlayer {
    let hit = &party_hit_of(hit);
    let i = &hit.info;
    kh_core::states::HitOnPlayer {
        attribute: hit.attribute,
        direction: hit.direction,
        causer_yaw: boss_yaw,
        causer_radius: 34.0,
        power_scale: if i.power_scale > 0.0 { i.power_scale } else { 1.0 },
        brake: i.brake,
        parabola_angle: i.parabola_angle,
        move_length: i.move_length,
        effect_time: i.effect_time,
        src_power: if i.power > 0.0 { i.power } else { 1.0 },
        ignore_guard: i.flags50 & kh_core::host::HitInfo::FLAG50_IGNORE_GUARD != 0,
        rapid_fire: i.flags50 & kh_core::host::HitInfo::FLAG50_RAPID_FIRE != 0,
        reflect_level: i.reflect_level,
        reflect_reaction: i.reflect_reaction,
        bad_status: i.bad_status,
        bad_time: i.bad_time,
        bad_param: i.bad_param,
        ..kh_core::states::HitOnPlayer::new(hit.damage_kind, hit.damage, TargetId(VICTIM_BASE | 0xFFFF), boss_at)
    }
}

// ============================================================================================ a friend leader's copy

/// A FRIEND LEADER'S COPY: a second party runtime that holds one AI member, the copied friend, with the leader's own status
/// (level, equipment, abilities, the roster's boosts and learned magic, his AI settings): his own moveset and tree (Donald's
/// spells, Goofy's moves), the party AI's brain; its "player" is a stand-in Sora kept at the copy's own feet (so the formation
/// never pulls him anywhere; his battle is the stand-in's forced battle flag), its enemies the party ([`BossTargets`]).
pub struct FriendBoss {
    pub rt: PartyRuntime,
    pub stand_in: Sora,
    pub chara: PartyChara,
    /// (MIMIC-BOSS hotfix 3, MB3-AGGRO) The hostile tuning given ([`FriendBoss::set_aggro`]); None: not given yet.
    pub aggro: Option<CopyAggro>,
}

impl FriendBoss {
    /// The copy of friend `chara` of the party `real` (its status as it is now) at `at` facing `yaw`; `sora`: the party's Sora
    /// core (the shared EXP; the stand-in is made of it). None: `chara` is not in the party.
    pub fn new(real: &PartyRuntime, sora: &Sora, chara: PartyChara, at: Vec3, yaw: f32) -> Option<FriendBoss> {
        let real_id = real.id(chara)?;
        let status = real.party.status(real_id)?.clone();
        let settings = real.ai_settings(chara);
        let mut rt = PartyRuntime::new(crate::RuntimeConfig { wanted: true, test: real.test.clone(), weapons: real.weapons.clone(), ai_pack: real.ai_pack.clone(), ai_pack_source: None });
        rt.progression = real.progression;
        rt.roster_boosts = real.roster_boosts;
        rt.don_learned = real.don_learned;
        rt.friend_growth = real.friend_growth;
        rt.customs = real.customs.clone();
        rt.last_exp = real.last_exp;
        rt.active_friends = [chara == PartyChara::Donald, chara == PartyChara::Goofy];
        rt.lead.formation = Formation::new(&[PartyChara::Sora, chara], PartyChara::Sora).unwrap_or_default();
        rt.lead.current = Some(rt.lead.formation.clone());
        let mut stand_in = Sora::new(*sora.params(), Pose { position: at, yaw }).with_status(sora.status().clone());
        stand_in.actions_mut().battle = true;
        stand_in.actions_mut().battle_forced = true;
        rt.spawn_one(chara, at, yaw);
        let id = rt.id(chara)?;
        if let Some(s) = rt.party.status_mut(id) {
            *s = status;
            s.hp = s.max_hp();
            s.mp = s.max_mp();
        }
        if let Some(m) = rt.party.member_mut(id) {
            m.cmc.location = at;
            m.yaw = yaw;
        }
        mark_copy(&mut rt, chara); // (MIMIC-BOSS-KIT) his full kit (Zettaflare), his party's Zettaflare kept to itself
        rt.set_ai_settings(chara, settings);
        rt.current = rt.views();
        rt.previous = rt.current.clone();
        Some(FriendBoss { rt, stand_in, chara, aggro: None })
    }

    pub fn id(&self) -> Option<kh_party::MemberId> {
        self.rt.id(self.chara)
    }

    /// His view after the last step.
    pub fn view(&self) -> Option<kh_party::MemberView> {
        let id = self.id()?;
        self.rt.current.iter().find(|v| v.id == id).copied()
    }

    /// ONE STEP: the stand-in at his feet in battle, the runtime's step (his tree, his pawn) against the party.
    pub fn step(&mut self, s: FriendBossStep<'_>) -> crate::StepOut {
        let FriendBossStep { dt, world, targets, audio, effects, camera, nav, now } = s;
        let (at, yaw, velocity) = self.view().map_or((self.stand_in.output().position, 0.0, Vec3::ZERO), |v| (v.position, v.yaw, v.velocity));
        self.stand_in.actions_mut().battle = true;
        self.stand_in.actions_mut().battle_forced = true;
        let out = kh_core::Output { position: at, yaw, velocity, on_floor: true, ..self.stand_in.output() };
        // (MIMIC-STANCE) the party's swings it sees coming, as the Elden Ring host's sensors feed the real party's Goofy
        let mut seen = Vec::new();
        targets.telegraphs(&mut seen);
        self.rt.party.set_telegraphs(&seen);
        self.rt.step(crate::StepIn { dt, sora: &mut self.stand_in, out: &out, lock_on: TargetId::NONE, camera, world, targets, audio, effects, nav, now })
    }

    /// A party member's hit on the boss: his own KH3 reaction (kh-party's hurt rules), no HP taken (it mirrors the game's).
    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT) judged on HIS OWN body ([`copy_reaction_hit`]), never on the Elden Ring boss body's.
    pub fn take_reaction(&mut self, hit: &HitRecord) -> Option<HitResult> {
        let h = copy_reaction_hit(hit);
        self.rt.receive_hit(self.chara, &h)
    }

    /// (MIMIC-STANCE, THE USER 2026-11-05: "make sure Goofy's mimic tear boss is able to parry you properly") A PARTY MEMBER'S
    /// HIT ON THE BOSS, ASKED BEFORE THE GAME TAKES IT: the copy's own rule for a hit on him, Goofy's (kh-party pad/goofy.rs
    /// `on_hit`: the hop's invincibility, over a sweep, Tornado's spin, the guard, every parry technique by the one judgement
    /// `goofy::parry::judge` against his tap clock, his Resolve and ranks), exactly as a hit of an Elden Ring enemy on the party's
    /// Goofy. `from`: who threw it. [`CopyDefence::negates`]: the host lets NONE of it through to the boss's Elden Ring body
    /// (no HP, no reaction) and answers the attacker's hit as guarded; a parry's Stagger lands on `from`'s stance (the copy's
    /// book, next step) and a perfect deflect of a swing bounces him ([`StanceOrder::Rebound`], next step). `Through`: the
    /// game's damage as before, then [`FriendBoss::take_reaction`]. A copy that is no Goofy (Donald) answers `Through`.
    pub fn defend(&mut self, from: PartyChara, hit: &HitRecord) -> CopyDefence {
        let seen: Vec<kh_party::Telegraph> = self.rt.party.telegraphs().to_vec();
        let Some(m) = self.id().and_then(|id| self.rt.party.member_mut(id)) else { return CopyDefence::Through };
        if m.status.goofy.is_none() || m.state == kh_party::NpcStateId::Die || m.hurt.dying || matches!(m.state, kh_party::NpcStateId::Fr(_)) {
            return CopyDefence::Through;
        }
        let mut h = *hit;
        let attacker = victim_id(from);
        h.attack_info.attacker = Some(attacker);
        if h.attack_info.class == kh_core::host::AttackClass::Normal {
            h.attack_info.class = party_hit_class(hit, attacker, &seen);
        }
        h.attack_info.perilous |= matches!(h.attack_info.class, kh_core::host::AttackClass::Grab | kh_core::host::AttackClass::Heavy);
        let before = m.status.goofy.as_ref().map_or(0, |s| s.core.events.len());
        let Some(r) = kh_party::pad::goofy::on_hit(m, &h) else { return CopyDefence::Through };
        let parried = m.status.goofy.as_ref().and_then(|s| {
            s.core.events.iter().skip(before).find_map(|e| match e {
                kh_party::goofy::GoofyEvent::Parried { tech, right, .. } => Some((*tech, *right)),
                _ => None,
            })
        });
        match parried {
            Some((tech, right)) => CopyDefence::Parried { tech, right },
            None if !r.accepted => CopyDefence::Evaded,
            None => CopyDefence::Blocked,
        }
    }

    /// (MIMIC-STANCE) The party member `chara`'s stance as the copy's Goofy has filled it (its Stagger book: the HUD's gauge
    /// under the leader's HP, `StaggerView::share` / `broken`). None: nothing on him.
    pub fn victim_stagger(&self, chara: PartyChara) -> Option<kh_party::goofy::StaggerView> {
        self.rt.party.stagger_view(victim_id(chara))
    }

    /// His HP set to the game's share ([`mirrored_hp`]).
    pub fn mirror_hp(&mut self, er_hp: i32, er_max: i32) {
        if let Some(s) = self.id().and_then(|id| self.rt.party.status_mut(id)) {
            s.hp = mirrored_hp(er_hp, er_max, s.max_hp());
        }
    }
}

/// What the host lends a friend copy's step.
pub struct FriendBossStep<'a> {
    pub dt: f32,
    pub world: &'a dyn kh_core::World,
    pub targets: &'a mut dyn PartyTargets,
    pub audio: &'a mut dyn kh_core::Audio,
    pub effects: &'a mut dyn kh_core::Effects,
    pub camera: &'a kh_camera::CameraView,
    pub nav: Option<&'a dyn kh_ai::Nav>,
    pub now: f64,
}

/// THE GAME'S DEATH OF THE BOSS'S BODY AS THE COPY'S KO: KH3's Kill kind, from `from`.
pub fn lethal_blow(from: Vec3) -> kh_core::states::HitOnPlayer {
    kh_core::states::HitOnPlayer::new(kh_core::enums::ETresDamageKind::Kill, 1.0e6, TargetId::NONE, from)
}

/// THE COPY'S KO NOW (the game's body is dead whatever the copied abilities say): its HP to 1 with no Second Chance (it
/// needs more than 1 HP), no Withstand Combo (its window and damage flag cleared), no "1 HP" hold, then the lethal blow. The
/// host gives it again every few steps until the copy is in its KO (a hit in invincible frames is not taken).
pub fn knock_out(core: &mut Sora, from: Vec3) {
    let st = core.status_mut();
    if st.hp <= 0 {
        return;
    }
    st.hp = 1;
    st.hp_limit_one = false;
    st.combo_leave_timer = 0.0;
    st.in_damage_state = false;
    core.take_hit(lethal_blow(from));
}

// ============================================================================================ (MIMIC-BOSS-KIT) the copies' full kit

/// (MIMIC-BOSS-KIT, the coordinator's rule: "a COPY has the copied leader's FULL kit, under the same requirements the real
/// character has"; THE USER 2026-11-05: "mimic tear boss Donald should be able to use Zettaflare if it manages to reach the
/// requirements during combat just like Donald") MARKS A FRIEND COPY'S MEMBER AS A COPY (kh-party `PawnZetta::copy`): a
/// copied Donald's AI may cast his Y Arcane magic and Zettaflare under Donald's own requirements (kh-party donald/ai.rs
/// `copy_arcane`, zettaflare.rs `may_cast_zettaflare`), never with the cinematic camera; and the copy's party (any chara)
/// writes none of Zettaflare's process-wide words and reads no fed wand tip / staff (kh-party zettaflare.rs `copy_party`).
/// Called by [`FriendBoss::new`]; true when the member was there.
pub fn mark_copy(rt: &mut PartyRuntime, chara: PartyChara) -> bool {
    let Some(m) = rt.id(chara).and_then(|id| rt.party.member_mut(id)) else { return false };
    m.don_zetta.copy = true;
    true
}

/// (MIMIC-BOSS-KIT) The friend copy is marked as a copy ([`mark_copy`]).
pub fn is_marked_copy(boss: &FriendBoss) -> bool {
    boss.id().and_then(|id| boss.rt.party.member(id)).is_some_and(|m| m.don_zetta.copy)
}

// ============================================================================================ (MIMIC-STANCE) Goofy against the party
//
// (THE USER 2026-11-05: "make sure Goofy's mimic tear boss is able to parry you properly, can stagger you by breaking your stance
// and can use his Breaker on you") A GOOFY COPY (either boss's, or a boss-summoned Mimic copying Goofy: all `FriendBoss`) PLAYS
// THE PARTY AS THE PARTY'S GOOFY PLAYS ELDEN RING'S ENEMIES, by the same code and rows:
// - HE SEES THE SWINGS COMING: each party member's attack as a telegraph (`kh_party::Telegraph`), read from the member as kh-er's
//   sensor reads an enemy (telegraph.rs: the playing animation's next attack window, else the open one; a bullet closing in);
//   his AI answers it by his own rules (goofy/ai.rs: read delay, timing roll by level, the technique for the class).
// - THE PARRY: [`FriendBoss::defend`], asked before the game takes a party hit on the boss: Goofy's own judgement of a hit.
// - THE STANCE: his Stagger book fills the member's gauge (`stagger.*` rows: 100, a deflect 25 ...); full, it asks the member's
//   real stance ([`Victim::stance`]) and the host breaks him (kh-core combat/stance.rs: KH3's STUN state for 2.5 s).
// - THE BREAKER: his AI's rule 1 / 3 on the BROKEN member (walk in at his run speed within `ai.breaker_chase`, Y after
//   `ai.breaker_delay`), the slam through the member's own damage path (`player_hit_of` / `receive_hit`), as every hit of his.
// - (NOT MIRRORED, the coordinator's default) "Protect the slam" (kh-er stance.rs: while a Breaker winds up on an enemy, the
//   party's other hits leave it 1 HP) has no player-side twin: nothing caps another hit on a broken party member.

/// (MIMIC-STANCE) A party member's attack as the copy sees it coming this step: the shape of `kh_party::Telegraph` (and of
/// kh-er telegraph.rs `Seen`, the enemy side's).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct VictimTell {
    /// An attack of his is coming or open.
    pub on: bool,
    pub class: kh_core::host::AttackClass,
    /// The danger cue's rule (an unblockable heavy, a grab ...).
    pub perilous: bool,
    /// Seconds until its hit window opens (0 when `active`).
    pub eta: f32,
    /// Its hit window is open now.
    pub active: bool,
}

impl VictimTell {
    fn swing(class: kh_core::host::AttackClass, eta: f32) -> VictimTell {
        let perilous = matches!(class, kh_core::host::AttackClass::Heavy | kh_core::host::AttackClass::Grab);
        let active = eta <= 0.0;
        VictimTell { on: true, class, perilous, eta: eta.max(0.0), active }
    }

    /// The one that opens first (a perilous one on a tie): what one telegraph per attacker carries.
    fn sooner(self, other: VictimTell) -> VictimTell {
        match (self.on, other.on) {
            (false, _) => other,
            (_, false) => self,
            _ if other.eta < self.eta || (other.eta == self.eta && other.perilous && !self.perilous) => other,
            _ => self,
        }
    }

    /// As the copy's party reads it.
    pub fn telegraph(&self, chara: PartyChara) -> kh_party::Telegraph {
        kh_party::Telegraph { attacker: victim_id(chara), class: self.class, perilous: self.perilous, eta: self.eta, active: self.active }
    }
}

/// kh-er telegraph.rs `read_bullets`, the enemy side's rule for a shot: closing in from within 40 m at 1 m/s or more, coming in
/// under 3 s; open under 0.1 s. (cm, cm/s)
const SHOT_RANGE: f32 = 4000.0;
const SHOT_CLOSING_MIN: f32 = 100.0;
const SHOT_ETA_MAX: f32 = 3.0;
const SHOT_OPEN: f32 = 0.1;

fn shot_tell(at: Vec3, velocity: Vec3, copy_at: Vec3, class: kh_core::host::AttackClass) -> VictimTell {
    let to = copy_at - at;
    let d = to.length();
    let closing = velocity.dot(to.normalize_or_zero());
    if d > SHOT_RANGE || closing < SHOT_CLOSING_MIN {
        return VictimTell::default();
    }
    let eta = d / closing;
    if eta >= SHOT_ETA_MAX {
        return VictimTell::default();
    }
    VictimTell { on: true, class, perilous: false, eta: if eta < SHOT_OPEN { 0.0 } else { eta }, active: eta < SHOT_OPEN }
}

/// The class of one of Sora's attack rows (`sora-combat-attack-data`): its `invalid_guard` is the unblockable heavy Goofy reads
/// from the ignore-guard flag (goofy/parry.rs `class_of`), its `magic` a spell.
fn sora_row_class(row: &str) -> kh_core::host::AttackClass {
    use kh_core::host::AttackClass;
    match kh_core::combat::tables::attack_row(row) {
        Some((_, r)) if r.invalid_guard => AttackClass::Heavy,
        Some((_, r)) if r.magic => AttackClass::Magic,
        _ => AttackClass::Normal,
    }
}

/// (MIMIC-STANCE) THE LEADER SORA'S ATTACK AS THE COPY SEES IT (fill [`Victim::tell`] with it every step; `copy_at`: the copy's
/// feet): an attack window of his open now (`combat().windows`), else the next `TresAtk` window of the clip he plays (its notify
/// time against the clip's clock), else the soonest of his spells / shotlock shots closing in on the copy (`magic().projectiles`,
/// a shotlock's cmd 0xFD a projectile, a spell magic). Nothing: `on` false.
pub fn victim_tell_of_sora(sora: &Sora, copy_at: Vec3) -> VictimTell {
    let mut best = VictimTell::default();
    for w in &sora.combat().windows {
        best = best.sooner(VictimTell::swing(sora_row_class(w.row_name), 0.0));
    }
    let anim = sora.output().anim;
    for n in anim.name.notifies() {
        if let kh_core::NotifyData::Attack { row, .. } = n.data {
            if anim.time <= n.end {
                best = best.sooner(VictimTell::swing(sora_row_class(row), n.time - anim.time));
            }
        }
    }
    for p in &sora.magic().projectiles {
        let class = if p.cmd == 0xFD { kh_core::host::AttackClass::Projectile } else { kh_core::host::AttackClass::Magic };
        best = best.sooner(shot_tell(p.position, p.velocity, copy_at, class));
    }
    best
}

/// (MIMIC-STANCE) A FRIEND LEADER'S ATTACK AS THE COPY SEES IT (`chara` of the real party `rt`): his clip's attack window (KH3's
/// `ClipDef::attack_windows`, open or next, by his clip's rate), his kit's shapes on his body (Goofy's shoves and Arts:
/// `goofy_shots::Area`, a shape not arrived yet by its age), his projectiles closing in (the shield a projectile, a spell magic).
pub fn victim_tell_of_friend(rt: &PartyRuntime, chara: PartyChara, copy_at: Vec3) -> VictimTell {
    use kh_core::host::AttackClass;
    let Some(id) = rt.id(chara) else { return VictimTell::default() };
    let Some(p) = rt.party.member(id) else { return VictimTell::default() };
    let mut best = VictimTell::default();
    let (time, speed) = (p.anim.time, p.anim.speed.max(0.01));
    for (start, end, _) in p.anim.clip.attack_windows() {
        if time <= end {
            best = best.sooner(VictimTell::swing(AttackClass::Normal, (start - time) / speed));
        }
    }
    if let Some(g) = p.status.goofy.as_ref() {
        // his string's step before its hit sphere is out: the step's own hit time (rows `step.<id>.hit`, pad/goofy_combo.rs; the
        // shove clips carry no Attack window of their own), at the wind-up's rate
        if let Some(d) = kh_party::pad::goofy_combo::current(p).filter(|_| !g.run.combo.hit_out) {
            best = best.sooner(VictimTell::swing(AttackClass::Normal, (d.hit - time) / kh_party::pad::goofy_combo::rate_at(&d, time).max(0.01)));
        }
        for a in g.run.shots.iter().filter(|a| a.follow.is_some()) {
            let class = if a.row.invalid_guard { AttackClass::Heavy } else if a.row.magic { AttackClass::Magic } else { AttackClass::Normal };
            best = best.sooner(VictimTell::swing(class, -a.age));
        }
    }
    for pr in rt.party.projectiles.iter().filter(|pr| pr.owner == id && pr.alive) {
        let class = if pr.kind == kh_party::projectile::ProjectileKind::Discus { AttackClass::Projectile } else { AttackClass::Magic };
        best = best.sooner(shot_tell(pr.position, pr.velocity, copy_at, class));
    }
    best
}

/// (MIMIC-STANCE) A party member's stance as the copy's Stagger book is answered from it.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct VictimStance {
    /// A break may start now (he is up). The default, a host that says nothing: no.
    pub breakable: bool,
    /// Seconds left of a running break (0: none).
    pub broken: f32,
}

/// (MIMIC-STANCE) The leader Sora's stance (fill [`Victim::stance`] with it every step).
pub fn victim_stance_of_sora(sora: &Sora) -> VictimStance {
    VictimStance { breakable: sora.stance_breakable(), broken: sora.stance().left }
}

/// (MIMIC-STANCE) A friend's stance (`chara` of the real party `rt`).
pub fn victim_stance_of_friend(rt: &PartyRuntime, chara: PartyChara) -> VictimStance {
    rt.id(chara).and_then(|id| rt.party.member(id)).map_or_else(VictimStance::default, |p| VictimStance { breakable: p.stance_breakable(), broken: p.stance.left })
}

/// (MIMIC-STANCE) What the copy's Stagger book asked of a party member ([`BossTargets::orders`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StanceOrder {
    /// His gauge is full: BROKEN for `seconds` (`stagger.window`).
    Break { chara: PartyChara, seconds: f32 },
    /// A Breaker winds up on him: the break lasts at least `seconds` more.
    Hold { chara: PartyChara, seconds: f32 },
    /// The break is over (its window, or the Breaker's slam about to land).
    Release { chara: PartyChara },
    /// The copy's perfect deflect turned his swing (goofy/parry.rs `bounces`, seams.rs `on_bounce`): his KH3 recoil off the
    /// shield, as an Elden Ring enemy's rebound.
    Rebound { chara: PartyChara },
}

impl StanceOrder {
    pub fn chara(&self) -> PartyChara {
        match *self {
            StanceOrder::Break { chara, .. } | StanceOrder::Hold { chara, .. } | StanceOrder::Release { chara } | StanceOrder::Rebound { chara } => chara,
        }
    }
}

/// (MIMIC-STANCE) The copy's answer to a party hit ([`FriendBoss::defend`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyDefence {
    /// It lands: the game's damage of the boss's body, then [`FriendBoss::take_reaction`].
    Through,
    /// Parried by technique `tech` (`right`: the right answer to the attack, a perfect parry): 0 damage.
    Parried { tech: kh_party::goofy::Tech, right: bool },
    /// His shield took it (GUARD paid): 0 damage.
    Blocked,
    /// It found him out of reach (the hop's invincibility, over a sweep): 0 damage.
    Evaded,
}

impl CopyDefence {
    /// None of the hit reaches the boss's Elden Ring body.
    pub fn negates(self) -> bool {
        self != CopyDefence::Through
    }

    /// What the host's `apply_hit` answers the attacker for a negated hit (no HP, guarded: Sora's core and kh-party treat it
    /// as a blocked hit: no style points, no hit-stop of damage).
    pub fn result(self, hp: f32) -> HitResult {
        HitResult { accepted: self != CopyDefence::Evaded, damage_dealt: 0.0, hp_after: hp, killed: false, guarded: self != CopyDefence::Evaded }
    }
}

/// (MIMIC-STANCE) The class of a party member's hit on the copy, as kh-er's `hit_read` names an enemy's landed hit: the record's
/// own (a grab kind, the ignore-guard flag: goofy/parry.rs `class_of`), a hit of another attribute than physical a spell; a plain
/// one takes the class of the attacker's telegraph open now (the swing it came from).
pub fn party_hit_class(hit: &HitRecord, attacker: TargetId, seen: &[kh_party::Telegraph]) -> kh_core::host::AttackClass {
    use kh_core::host::AttackClass;
    match kh_party::goofy::parry::class_of(hit) {
        AttackClass::Normal if hit.attribute != kh_core::enums::ETresDamageAttribute::Physical => AttackClass::Magic,
        AttackClass::Normal => seen.iter().find(|t| t.attacker == attacker && t.active).map_or(AttackClass::Normal, |t| t.class),
        c => c,
    }
}

/// (MIMIC-STANCE) The rebound off the copy's shield as a hit on Sora: reaction type 3, REFLECT, no damage (kh-core damage.rs
/// `take_hit`: R 3 -> TresPlayerState_Reflect, the weapon recoil `AttackReflect`, knocked away from the copy).
pub fn rebound_hit(copy_at: Vec3) -> kh_core::states::HitOnPlayer {
    kh_core::states::HitOnPlayer { reaction_type: 3, ..kh_core::states::HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 0.0, TargetId(VICTIM_BASE | 0xFFFF), copy_at) }
}

fn take_orders(t: &mut BossTargets, mine: impl Fn(PartyChara) -> bool) -> Vec<StanceOrder> {
    let (taken, kept): (Vec<StanceOrder>, Vec<StanceOrder>) = t.orders.drain(..).partition(|o| mine(o.chara()));
    t.orders = kept;
    taken
}

/// (MIMIC-STANCE) THE HOST CARRIES OUT THE COPY'S ORDERS ON THE LEADER SORA, right after the copy's step and BEFORE its hits
/// are given to him (`copy_at`: the copy's feet): a break, its hold and end on his stance (`Sora::stance_*`), a rebound his
/// weapon recoil ([`rebound_hit`]). Returns what it did.
pub fn stance_to_sora(t: &mut BossTargets, sora: &mut Sora, copy_at: Vec3) -> Vec<StanceOrder> {
    let taken = take_orders(t, |c| c == PartyChara::Sora);
    for o in &taken {
        match *o {
            StanceOrder::Break { seconds, .. } => {
                sora.stance_break(seconds);
            }
            StanceOrder::Hold { seconds, .. } => sora.stance_hold(seconds),
            StanceOrder::Release { .. } => {
                sora.stance_release();
            }
            StanceOrder::Rebound { .. } => sora.take_hit(rebound_hit(copy_at)),
        }
    }
    taken
}

/// (MIMIC-STANCE) The same for a friend of the real party `rt` (the leading friend; any member the orders name): his pawn's
/// stance (`NpcPawn::stance_*`), a rebound his KH3 repel (kh-party repel.rs `after_swing`: AttackReflect of his own set).
pub fn stance_to_party(t: &mut BossTargets, rt: &mut PartyRuntime, copy_at: Vec3) -> Vec<StanceOrder> {
    let taken = take_orders(t, |c| c != PartyChara::Sora && rt.id(c).is_some());
    for o in &taken {
        let Some(p) = rt.id(o.chara()).and_then(|id| rt.party.member_mut(id)) else { continue };
        match *o {
            StanceOrder::Break { seconds, .. } => {
                p.stance_break(seconds);
            }
            StanceOrder::Hold { seconds, .. } => p.stance_hold(seconds),
            StanceOrder::Release { .. } => {
                p.stance_release();
            }
            StanceOrder::Rebound { .. } => {
                kh_party::repel::after_swing(p, kh_core::host::ReactionResult::Reflect, copy_at);
            }
        }
    }
    taken
}

// ============================================================================================ (MIMIC-BOSS) the host's helpers

/// THE LEADER SORA AS THE COPY'S VICTIM this step (`copy_at`: the copy's feet, for his attack's tell).
pub fn victim_of_sora(sora: &Sora, copy_at: Vec3) -> Victim {
    let o = sora.output();
    let st = sora.status();
    Victim { chara: PartyChara::Sora, feet: o.position, yaw: o.yaw, velocity: o.velocity, radius: 34.0, half_height: 90.0, hp: st.hp, hp_max: st.max_hp(), defense: st.defense(), airborne: !o.on_floor, down: st.hp < 1 || st.invincible, tell: victim_tell_of_sora(sora, copy_at), stance: victim_stance_of_sora(sora) }
}

/// A FRIEND OF THE PARTY (the leading friend) AS THE COPY'S VICTIM this step; None: he is not in the party.
pub fn victim_of_friend(rt: &PartyRuntime, chara: PartyChara, copy_at: Vec3) -> Option<Victim> {
    let id = rt.id(chara)?;
    let v = rt.current.iter().find(|v| v.id == id)?;
    let defense = rt.party.status(id).map_or(0, |s| s.defense());
    Some(Victim { chara, feet: v.position, yaw: v.yaw, velocity: v.velocity, radius: v.capsule.0.max(30.0), half_height: v.capsule.1.max(60.0), hp: v.hp, hp_max: v.hp_max, defense, airborne: !v.on_ground, down: v.ko, tell: victim_tell_of_friend(rt, chara, copy_at), stance: victim_stance_of_friend(rt, chara) })
}

/// (THE USER 2026-11-05: "Non-Sora mimic bosses won't summon spirit ashes since neither of them can summon spirit ashes,
/// correct?") THE BOSS SUMMONS ONLY WHAT ITS LEADER COULD: the shared gate is the leader's own Summon command, as the
/// command menu has it at the fight's start (kh-core `SummonView::allowed`, which the host opens for the player's own Sora
/// only: AI Sora, Mimic Sora and the friends' menus have no Summon row). A copied leader without it (Donald, Goofy, any
/// character whose kit has no Summon command) never summons. The ash: the first equipped slot of that menu (resolved to
/// the highest level the player owns: `SummonView::slots`); None = nothing equipped, nothing summoned.
pub fn boss_summon_of(leader: PartyChara, sora: &Sora) -> Option<kh_core::command::summon::AshSlot> {
    let view = &sora.command().summon;
    if leader != PartyChara::Sora || !view.allowed {
        return None;
    }
    view.slots.iter().flatten().next().cloned()
}

/// What a boss summons with that ash (the host spawns it): its own Mimic Tear (a hostile Mimic Sora: the spirit route's
/// copy of the same leader, `crate::mimic::copy_kit` at the ash's level, its spirit time) or Elden Ring's spirit of the ash
/// (its BuddyParam rows, hostile, at the ash's level).
#[derive(Clone, Debug, PartialEq)]
pub enum BossSummon {
    MimicTear { level: u8, renowned: bool, goods: u32 },
    Spirit { goods: u32, level: u8 },
}

pub fn boss_summon_kind(ash: &kh_core::command::summon::AshSlot) -> BossSummon {
    if ash.mimic {
        BossSummon::MimicTear { level: ash.level, renowned: ash.renowned, goods: ash.goods }
    } else {
        BossSummon::Spirit { goods: ash.goods, level: ash.level }
    }
}

// ============================================================================================ (MIMIC-BOSS hotfix 2) HP, resistances, map

/// (THE USER 2026-11-06: "x30 the health then"; before: "the HP the boss uses to x3 MUST be the party leaders, not Elden RIng's
/// tarnished's health") THE BOSS'S HP: this many times the COPIED LEADER's own KH3 maximum HP, taken as it is (no KH <-> ER
/// conversion). The sheet `sheets/port/mimic-boss.json` row `boss.hp_x_leader` tunes it (this is its default).
pub const BOSS_HP_X_LEADER: f32 = 30.0;

/// The boss's resident SpEffect 19397 (both bosses' NpcParam `spEffectID3`): maxHpRate 3 (regulation 2.7.2.0). The game makes
/// the body's maximum = its base x the rates of its SpEffects, so the base written is the wanted maximum / 3, and a later
/// journey's GameClearSpEffect (19509 maxHpRate 1.461 / 19516 1.015) still goes on top, as Elden Ring applies it.
pub const BOSS_RESIDENT_HP_RATE: f32 = 3.0;

/// The boss's wanted maximum HP (before a later journey's rate) for a leader whose KH3 maximum is `leader_max`.
pub fn boss_hp(leader_max: i32, x: f32) -> i32 {
    ((leader_max.max(1) as f32) * x.clamp(1.0, 1000.0)).round() as i32
}

/// The base HP written on the boss's body for that maximum.
pub fn boss_base_hp(wanted_max: i32) -> i32 {
    ((wanted_max as f32) / BOSS_RESIDENT_HP_RATE).round().max(1.0) as i32
}

/// A body whose game maximum is `max_now` over its base `base_now` (its own SpEffect rates): the base that makes its maximum
/// `wanted` (the boss-summoned Mimic Tear's body takes the boss's maximum: "the mimic summoned by the boss needs to use the
/// boss' HP").
pub fn base_for(wanted: i32, base_now: i32, max_now: i32) -> i32 {
    let rate = if base_now > 0 && max_now > 0 { max_now as f32 / base_now as f32 } else { 1.0 };
    ((wanted as f32) / rate.max(0.01)).round().max(1.0) as i32
}

/// (THE USER 2026-11-06: "Mimic tear needs to copy your abilities, both boss and summon") A BOSS COPY'S SECOND CHANCE AND
/// WITHSTAND COMBO ON ELDEN RING'S HP (the copy's HP is the game's): the body's no-death switch is held while KH3's own rule
/// would leave 1 HP (kh-core status `apply_damage_to_sora`: LastLeave while HP > 1; ComboLeave within its combo window or
/// in a damage state), so a lethal blow leaves the body at 1 HP exactly as it leaves the real character.
pub fn holds_no_dead(abilities: &kh_core::status::AbilitySet, combo_window: bool, er_hp: i32) -> bool {
    use kh_core::enums::ETresAbilityKind as A;
    let second_chance = abilities.count(A::LastLeave) > 0 && er_hp > 1;
    let withstand = abilities.count(A::ComboLeave) > 0 && combo_window;
    er_hp > 0 && (second_chance || withstand)
}

/// A Sora copy's Withstand Combo window (KH3's: its timer runs or it is in a damage state).
pub fn combo_window(status: &kh_core::Status) -> bool {
    status.combo_leave_timer > 0.0 || status.in_damage_state
}

/// (THE USER 2026-11-06: "make sure the proper resistences are applied to the mimic boss") THE COPY'S OWN KH3 DEFENCE as the
/// party's damage formula reads it (`Targets::defense`): the copied leader's Defense, his elemental resistances from his
/// equipment, accessories and abilities (kh-core `Status::as_defender`: exactly the real character's), so a party hit is
/// worked out against the copy as it would be against the leader. Elden Ring's own resistances of the body (its NpcParam's
/// status resistances, its SpEffects) stay the game's, on top.
pub fn copy_defense(status: &kh_core::Status) -> TargetDefense {
    let d = status.as_defender();
    TargetDefense { defense: d.defense, max_hp: d.max_hp, hp_percent: d.hp_percent, enemy_kind: false, damage_min: d.damage_min, damage_max: d.damage_max, resist: d.resist, bio_type: d.bio_type }
}

/// The boss whose arena map is the map block `block` (`(area << 24) | (block << 16) | ...`, Elden Ring's BlockId): its rules
/// start at the fog gate there, before its body is in the world.
pub fn boss_of_block(block: i32) -> Option<&'static MimicBossRow> {
    let b = block as u32;
    (block > 0).then_some(()).and_then(|_| MIMIC_BOSSES.iter().find(|r| (b >> 24) & 0xFF == u32::from(r.area) && (b >> 16) & 0xFF == u32::from(r.block)))
}

/// THE ABILITIES A COPY HAS BUT CANNOT USE (kept equipped, inert: nothing of a copy reaches the systems they act on): the
/// prize / munny / EXP / item-drop boosts and the field and HUD abilities. Everything else of the leader's equipment acts
/// through the character's own code, on the copy's own state.
pub const INERT_ON_A_COPY: [&str; 9] = ["LUCKY_STRIKE (item drops: a copy kills nothing that drops)", "MUNNY_CONVERTER / JACKPOT (munny: a copy has no purse)", "EXP_BOOST / EXP_ZERO (EXP: a copy earns none)", "PRIZE abilities, SHARE_PRIZE (prizes: none for a copy)", "SCAN (the enemy gauge: the player's HUD)", "TREASURE_MAGNET (prize pick-up: the player's)", "FIELD abilities (camp / menu / map rules: the player's)", "AUTO_FINISH-type HUD conveniences (the player's command menu)", "SYNCH / team abilities (the party's team attacks: excluded for copies)"];

// ==================================================== (MIMIC-BOSS hotfix 3, MB3-FOGRISE) the rise at the fog gate (traced)

/// (MIMIC-BOSS hotfix 3, MB3-FOGRISE; THE USER 2026-11-06: "Have the boss itself activate as soon as you enter the fog gate
/// opposed to having to walk up to it") HOW ELDEN RING STARTS THE RISE (the pool's anim 20010, CopyPlayerCharacterData, the
/// body enabled at the pool's SpEffect 16307, anim 63010, the boss bar), as its own event waits for it. Read offline from
/// 2.7.2.0's scripts: Nokron event 12022860 (`MIMIC-BOSS/trace/m12_02_00_00.txt`), the Stray event 30202810
/// (`trace/m30_20_00_00.txt`); the regions from the MSBs (`trace/msb_m12_02.js`, `msb_m30_20.js`); STATE.md, MB3-FOGRISE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RiseRoute {
    /// The event waits on its fog flag AND the player in the fog's own region, a slab just inside the fog (Nokron's 12022850,
    /// 11.6 x 2 m; the Stray's 30202800, 5 x 3 m): the rise starts at the fog pass by itself.
    FogPass,
    /// Nokron's FIRST visit (its first-encounter flag 12020851 off): the event never reads the fog flag. It waits on the
    /// player in the first-encounter region 12022851 (a 52 x 65 x 15 m box about the pool, its edge some 33 m inside the fog:
    /// the walk-up) OR on the pool's own wake: any damage on it (3[23]) or a state info 436 / 2 / 5 / 6 / 260 on it (4[34] x 5:
    /// sleep, poison, rot, bleed, frost). Then it sets 12020851 itself and the rise runs.
    FirstVisit,
}

/// The route Elden Ring's event takes for this boss now (`first_done`: its first-encounter flag as the game has it).
pub fn rise_route(row: &MimicBossRow, first_done: bool) -> RiseRoute {
    if row.first_flag.is_some() && !first_done {
        RiseRoute::FirstVisit
    } else {
        RiseRoute::FogPass
    }
}

/// The state infos that wake Nokron's pool on the first visit (event 12022860, its five 4[34] conditions on entity 12020860).
pub const POOL_WAKE_STATE_INFOS: [u16; 5] = [436, 2, 5, 6, 260];

/// THE WAKE: SpEffectParam 3051 (regulation 2.7.2.0): stateInfo 6, effectEndurance 0.1 s, bloodAttackPower -99999 (the bleed
/// meter's reset), no vfx, no HP change. On the pool it is the event's own "a state info 6 on the pool" for six frames and
/// nothing else. Given by the game's own apply-SpEffect function (the one EMEVD SetSpEffect uses). No flag is written.
pub const POOL_WAKE_SPEFFECT: i32 = 3051;
/// SpEffectParam 3051's stateInfo.
pub const POOL_WAKE_STATE_INFO: u16 = 6;
/// Asked again this often (s) while the event has not taken it (its first-encounter flag still off) ...
pub const POOL_WAKE_RETRY_S: f64 = 0.5;
/// ... at most this many times.
pub const POOL_WAKE_TRIES: u32 = 10;

/// What Elden Ring's event reads (the tests' model of 12022860 / 30202810's condition groups, both branches).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RiseInputs {
    pub first_done: bool,
    pub fog_flag: bool,
    pub in_fog_region: bool,
    pub in_first_region: bool,
    pub pool_damaged: bool,
    pub pool_state_info: Option<u16>,
}

/// Elden Ring's event starts the rise this frame (its condition group as the trace has it).
pub fn er_rise_starts(row: &MimicBossRow, i: &RiseInputs) -> bool {
    match rise_route(row, i.first_done) {
        RiseRoute::FogPass => i.fog_flag && i.in_fog_region,
        RiseRoute::FirstVisit => i.in_first_region || i.pool_damaged || i.pool_state_info.is_some_and(|s| POOL_WAKE_STATE_INFOS.contains(&s)),
    }
}

/// An MSB box region (shape 5): its place is the bottom's centre, `dims` = width (x), depth (z), height (y), turned about Y
/// by `rot_y_deg` (the right-handed Y rotation: the fit to the user's log, where Nokron's first-encounter box takes the leader
/// in the second Elden Ring started the rise, and both fog objects stand just outside their own fog slab).
pub fn in_msb_box(center: [f32; 3], rot_y_deg: f32, dims: [f32; 3], p: [f32; 3]) -> bool {
    let (s, c) = rot_y_deg.to_radians().sin_cos();
    let (dx, dy, dz) = (p[0] - center[0], p[1] - center[1], p[2] - center[2]);
    let lx = dx * c - dz * s;
    let lz = dx * s + dz * c;
    lx.abs() <= dims[0] * 0.5 && lz.abs() <= dims[1] * 0.5 && (0.0..=dims[2]).contains(&dy)
}

/// The facts the wake reads (all Elden Ring's: its flags and the boss's body).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RiseFacts {
    pub defeated: bool,
    pub fog_flag: bool,
    /// The first-encounter flag (Nokron's 12020851) as the game has it; nothing for a boss without one.
    pub first_done: bool,
    /// The game has enabled the boss's body (the rise has happened).
    pub body_enabled: bool,
}

/// Wake the pool now: the leader has passed the fog, the boss lives and has not risen, and its event is parked in the
/// first-visit wait (it would rise only at the walk-up). Never on the fog-pass route (the event starts at the fog by itself).
pub fn pool_wake_due(row: &MimicBossRow, f: &RiseFacts) -> bool {
    !f.defeated && f.fog_flag && !f.body_enabled && rise_route(row, f.first_done) == RiseRoute::FirstVisit
}

/// What the wake does this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WakeAsk {
    /// Nothing to do: the fog not passed, the fog-pass route, the boss beaten or risen, the wake over.
    Idle,
    /// Give the pool [`POOL_WAKE_SPEFFECT`] now (the try's number, from 1).
    Apply(u32),
    /// Due, but the pool's character is not in the loaded world.
    NoPool,
    /// Asked; waiting for the event to take it (its first-encounter flag) or for the next try.
    Wait,
    /// The event took it: the first-encounter flag is on (or the body enabled) `after` s after the first ask.
    Took { after: f64 },
    /// Asked [`POOL_WAKE_TRIES`] times without the event taking it: Elden Ring's own walk-up start remains.
    GaveUp,
}

/// THE WAKE'S STATE (one boss; the fog flag off again, e.g. a reload, starts it over).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolWake {
    pub tries: u32,
    pub first_at: Option<f64>,
    pub last_at: Option<f64>,
    pub over: bool,
}

impl PoolWake {
    /// One frame at `now` (s); `pool_there`: the pool's character is in the loaded world.
    pub fn step(&mut self, row: &MimicBossRow, f: &RiseFacts, pool_there: bool, now: f64) -> WakeAsk {
        if !f.fog_flag {
            *self = PoolWake::default();
            return WakeAsk::Idle;
        }
        if self.over {
            return WakeAsk::Idle;
        }
        if let Some(first_at) = self.first_at.filter(|_| f.first_done || f.body_enabled) {
            self.over = true;
            return WakeAsk::Took { after: now - first_at };
        }
        if !pool_wake_due(row, f) {
            return WakeAsk::Idle;
        }
        if !pool_there {
            return WakeAsk::NoPool;
        }
        if self.last_at.is_some_and(|t| now - t < POOL_WAKE_RETRY_S) {
            return WakeAsk::Wait;
        }
        if self.tries >= POOL_WAKE_TRIES {
            self.over = true;
            return WakeAsk::GaveUp;
        }
        self.tries += 1;
        self.first_at.get_or_insert(now);
        self.last_at = Some(now);
        WakeAsk::Apply(self.tries)
    }
}

// ------------------------------------------------------------------------------- (MIMIC-BOSS hotfix 3) the fight's body

/// (MIMIC-BOSS hotfix 3, THE USER 2026-11-06: "after the mimic boss summoned it's own mimic tear, the boss mimic disappeared
/// after it's mimic summon died/timed out"; "For mimic Sora, it's AI seemed to completely break when the mimic sora summoned by
/// the mimic Sora boss died or despawned") What the running fight makes of its Elden Ring body this frame. The user's logs
/// (2026-11-06, 246.013 s and 4114.322 s): the leader ran more than 40 m from the boss copy, the targets (read within 40 m of
/// the leader, `target.range`) no longer listed its body, and the fight took the body for gone: the copy AND its summoned
/// Mimic ended together (it looked like the summon's death ended the boss), and the body was handed back to the game with
/// its own AI on, unseen ("its AI broke"). The body is the fight's while the world has it by its handle, at any distance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FightBody {
    /// Alive in the world (listed or not): the fight runs.
    Alive,
    /// The game's death of it: the copy's KO, then its summon goes with it.
    Dead,
    /// Not in the loaded world at all (a load, another map): the fight ends.
    Gone,
}

/// (MIMIC-BOSS hotfix 3) The fight's body from the targets' entry (`listed`: (alive, HP)) or, when they do not list it, its
/// handle's body in the whole world (`by_handle`). A summon's coming or going never enters it.
pub fn fight_body(listed: Option<(bool, i32)>, by_handle: Option<(bool, i32)>) -> FightBody {
    match listed.or(by_handle) {
        Some((alive, hp)) if alive && hp > 0 => FightBody::Alive,
        Some(_) => FightBody::Dead,
        None => FightBody::Gone,
    }
}

impl MimicBossRow {
    /// (MIMIC-BOSS hotfix 3) THE LEADER IN THE ARENA during the solo fight: within its reach of the boss's body, OR held in
    /// its map behind the fog (the fog flag on, the leader in the boss's map block). The fog keeps him in, so a copy left
    /// behind (its body more than the arena's radius + margin from him) never ends the fight; a warp, a load or a death does.
    pub fn solo_holds(&self, boss_body: Option<[f32; 3]>, leader: [f32; 3], fog_flag: bool, leader_block: i32) -> bool {
        let fogged = fog_flag && boss_of_block(leader_block).is_some_and(|b| b.key == self.key);
        fogged || boss_body.is_some_and(|b| !self.away_from_boss(b, leader))
    }
}

// ============================================================================ (MIMIC-BOSS hotfix 3, MB3-AGGRO) the copies' aggression

/// (MIMIC-BOSS hotfix 3, MB3-AGGRO; THE USER 2026-11-06: "the boss mimic tear needs to be way way more aggressive"; "the more
/// aggresive AI for the mimic boss needs to apply to Donald and Goofy's mimic bosses as well. Goofy's boss currently stands still
/// and does nothing, waiting to parry you. Make the mimic boss worse at countering everything.") THE BOSS COPIES' HOSTILE
/// TUNING, for every copy of either boss (the Sora copy's `BossAi`, a Donald / Goofy copy's `FriendBoss`, the boss-summoned
/// Mimic Tear: the same two routes). OUR DESIGN (KH3 has no hostile Sora / Donald / Goofy): only the AI's CHOICES change: how
/// soon, how often, how near. Every move is still the character's own through his own core / pawn, with its cooldowns, MP rule,
/// MP Charge, GUARD, gauges and ranks: no cheating. The engine-free defaults are here; the host reads the sheet
/// `sheets/port/mimic-boss.json` rows `aggro.*` ([`CopyAggro::from_rows`], ids [`CopyAggro::ROWS`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CopyAggro {
    /// False: the party's own AI, untouched (what the copies ran before; the offline duel's "before").
    pub on: bool,
    // ---- the Sora copy (AI Sora's brain, lead.rs `SoraAi`)
    /// The longest the tree's Idle may hold it (s).
    pub sora_idle_max: f32,
    /// While the tree leaves it standing, turning or walking elsewhere, it runs at the leader and starts its own combo.
    pub sora_press: bool,
    /// Attack presses chain this long from the first (s; the party's 1.2): the combo plays on to its finisher.
    pub sora_combo_for: f32,
    /// Seconds between two presses of a chain (the party's 0.2).
    pub sora_press_every: f32,
    /// From this gap (cm) it presses Attack at the leader instead of running in: the core's own soft target and dashes
    /// (KH3's MyAutoLockonLimit is 900; Flash Step etc. when equipped).
    pub sora_gap_close: f32,
    /// Seconds between its attack spells, the low and the high end (the party's Go Wild 4 - 6); MP and MP Charge are the core's.
    pub sora_magic_every: (f32, f32),
    /// Seconds between its shotlocks (the party's Go Wild 15); its focus gauge is the core's.
    pub sora_shot_every: f32,
    /// Seconds between two situation commands (formchanges, finishes, grand magic; the party's 1).
    pub sora_situation_every: f32,
    /// Seconds between its Keyblade switches (the copies' 12; + up to lead.rs `COPY_SWITCH_SPREAD` x this / 12 at random).
    pub sora_switch_every: f32,
    /// The share of blows it takes an Aerial Recovery out of (the party's copies: every one).
    pub sora_recover_chance: f32,
    /// Seconds after the recovery window opens before it jumps (the party's: at once).
    pub sora_recover_delay: f32,
    // ---- a Goofy copy (kh-party goofy/ai.rs)
    pub goofy: kh_party::goofy::ai::CopyTune,
    // ---- a Donald copy (kh-party donald/ai.rs)
    /// Seconds between the spells he starts on his own; he moves in past `donald_approach` cm, back inside `donald_retreat` cm.
    pub donald_cast_every: f32,
    pub donald_approach: f32,
    pub donald_retreat: f32,
}

impl Default for CopyAggro {
    /// THE BOSS'S DEFAULTS (the sheet's rows as written): far more aggressive, far worse at countering.
    fn default() -> CopyAggro {
        CopyAggro {
            on: true,
            sora_idle_max: 0.2,
            sora_press: true,
            sora_combo_for: 2.4,
            sora_press_every: 0.12,
            sora_gap_close: 600.0,
            sora_magic_every: (1.5, 3.0),
            sora_shot_every: 8.0,
            sora_situation_every: 0.3,
            sora_switch_every: 6.0,
            sora_recover_chance: 0.35,
            sora_recover_delay: 0.25,
            goofy: kh_party::goofy::ai::CopyTune { parry_x: 0.6, read_add: 0.1, block_chance: 0.5, poke_every: 0.1, poke_hits: 4.0, poke_clear: 0.1, press_reach: 2000.0, bash_stagger: 0.0, tornado_count: 1.0 },
            donald_cast_every: 1.0,
            donald_approach: 1000.0,
            donald_retreat: 250.0,
        }
    }
}

impl CopyAggro {
    /// The party's own AI (the copies' behaviour before MB3-AGGRO): nothing applied.
    pub fn party() -> CopyAggro {
        CopyAggro { on: false, ..CopyAggro::default() }
    }

    /// The sheet's row ids, in order (`mimic-boss.json`; the host reads each as a number, 0 allowed).
    pub const ROWS: [&'static str; 25] = [
        "aggro.on",
        "aggro.sora.idle_max",
        "aggro.sora.press",
        "aggro.sora.combo_for",
        "aggro.sora.press_every",
        "aggro.sora.gap_close",
        "aggro.sora.magic_every_min",
        "aggro.sora.magic_every_max",
        "aggro.sora.shot_every",
        "aggro.sora.situation_every",
        "aggro.sora.switch_every",
        "aggro.sora.recover_chance",
        "aggro.sora.recover_delay",
        "aggro.goofy.parry_x",
        "aggro.goofy.read_add",
        "aggro.goofy.block_chance",
        "aggro.goofy.poke_every",
        "aggro.goofy.poke_hits",
        "aggro.goofy.poke_clear",
        "aggro.goofy.press_reach",
        "aggro.goofy.bash_stagger",
        "aggro.goofy.tornado_count",
        "aggro.donald.cast_every",
        "aggro.donald.approach",
        "aggro.donald.retreat",
    ];

    /// Each knob by its row id (the sheet's and [`CopyAggro::set`]'s); None: no such row.
    pub fn get(&self, id: &str) -> Option<f32> {
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        let g = &self.goofy;
        Some(match id {
            "aggro.on" => b(self.on),
            "aggro.sora.idle_max" => self.sora_idle_max,
            "aggro.sora.press" => b(self.sora_press),
            "aggro.sora.combo_for" => self.sora_combo_for,
            "aggro.sora.press_every" => self.sora_press_every,
            "aggro.sora.gap_close" => self.sora_gap_close,
            "aggro.sora.magic_every_min" => self.sora_magic_every.0,
            "aggro.sora.magic_every_max" => self.sora_magic_every.1,
            "aggro.sora.shot_every" => self.sora_shot_every,
            "aggro.sora.situation_every" => self.sora_situation_every,
            "aggro.sora.switch_every" => self.sora_switch_every,
            "aggro.sora.recover_chance" => self.sora_recover_chance,
            "aggro.sora.recover_delay" => self.sora_recover_delay,
            "aggro.goofy.parry_x" => g.parry_x,
            "aggro.goofy.read_add" => g.read_add,
            "aggro.goofy.block_chance" => g.block_chance,
            "aggro.goofy.poke_every" => g.poke_every,
            "aggro.goofy.poke_hits" => g.poke_hits,
            "aggro.goofy.poke_clear" => g.poke_clear,
            "aggro.goofy.press_reach" => g.press_reach,
            "aggro.goofy.bash_stagger" => g.bash_stagger,
            "aggro.goofy.tornado_count" => g.tornado_count,
            "aggro.donald.cast_every" => self.donald_cast_every,
            "aggro.donald.approach" => self.donald_approach,
            "aggro.donald.retreat" => self.donald_retreat,
            _ => return None,
        })
    }

    /// One knob by its row id, kept in its sane range (shares 0 ..= 1, times and distances >= 0). False: no such knob.
    pub fn set(&mut self, id: &str, v: f32) -> bool {
        if !v.is_finite() {
            return false;
        }
        let (p, share, on) = (v.max(0.0), v.clamp(0.0, 1.0), v > 0.5);
        let g = &mut self.goofy;
        match id {
            "aggro.on" => self.on = on,
            "aggro.sora.idle_max" => self.sora_idle_max = p,
            "aggro.sora.press" => self.sora_press = on,
            "aggro.sora.combo_for" => self.sora_combo_for = p,
            "aggro.sora.press_every" => self.sora_press_every = p.max(1.0 / 60.0),
            "aggro.sora.gap_close" => self.sora_gap_close = p,
            "aggro.sora.magic_every_min" => self.sora_magic_every.0 = p,
            "aggro.sora.magic_every_max" => self.sora_magic_every.1 = p,
            "aggro.sora.shot_every" => self.sora_shot_every = p,
            "aggro.sora.situation_every" => self.sora_situation_every = p,
            "aggro.sora.switch_every" => self.sora_switch_every = p,
            "aggro.sora.recover_chance" => self.sora_recover_chance = share,
            "aggro.sora.recover_delay" => self.sora_recover_delay = p,
            "aggro.goofy.parry_x" => g.parry_x = share,
            "aggro.goofy.read_add" => g.read_add = p,
            "aggro.goofy.block_chance" => g.block_chance = share,
            "aggro.goofy.poke_every" => g.poke_every = p,
            "aggro.goofy.poke_hits" => g.poke_hits = p,
            "aggro.goofy.poke_clear" => g.poke_clear = p,
            "aggro.goofy.press_reach" => g.press_reach = p,
            "aggro.goofy.bash_stagger" => g.bash_stagger = share,
            "aggro.goofy.tornado_count" => g.tornado_count = p,
            "aggro.donald.cast_every" => self.donald_cast_every = p,
            "aggro.donald.approach" => self.donald_approach = p,
            "aggro.donald.retreat" => self.donald_retreat = p,
            _ => return false,
        }
        if self.sora_magic_every.1 < self.sora_magic_every.0 {
            self.sora_magic_every.1 = self.sora_magic_every.0;
        }
        true
    }

    /// The defaults with every row `row(id)` answers ([`CopyAggro::ROWS`]); the ids it took.
    pub fn from_rows(row: impl Fn(&str) -> Option<f32>) -> (CopyAggro, Vec<&'static str>) {
        let mut a = CopyAggro::default();
        let mut taken = Vec::new();
        for id in CopyAggro::ROWS {
            if let Some(v) = row(id) {
                if a.set(id, v) {
                    taken.push(id);
                }
            }
        }
        (a, taken)
    }

    /// Its Goofy tuning when on.
    pub fn goofy_tune(&self) -> Option<kh_party::goofy::ai::CopyTune> {
        self.on.then_some(self.goofy)
    }

    /// Its Donald tuning when on (the clock at 0: his first own spell at once).
    pub fn donald_tune(&self) -> Option<kh_party::donald::ai::DonCopy> {
        self.on.then_some(kh_party::donald::ai::DonCopy { cast_every: self.donald_cast_every, approach: self.donald_approach, retreat: self.donald_retreat, cast_in: 0.0 })
    }

    /// One log line of it.
    pub fn describe(&self) -> String {
        if !self.on {
            return "OFF (the party's own AI)".to_string();
        }
        let g = &self.goofy;
        format!(
            "Sora: idle <= {:.2} s, press {}, combo {:.1} s x press {:.2} s, gap-close {:.0} cm, magic {:.1}-{:.1} s, shotlock {:.0} s, Y {:.1} s, switch {:.0} s, recovery {:.0} % after {:.2} s; Goofy: parry x{:.2}, read +{:.2} s, block {:.0} %, string every {:.2} s x {:.0}, clear {:.2} s, walks in from {:.0} cm, Bash at {:.0} %, Tornado at {:.0}; Donald: own spell every {:.1} s, in past {:.0} cm, back inside {:.0} cm",
            self.sora_idle_max, self.sora_press, self.sora_combo_for, self.sora_press_every, self.sora_gap_close, self.sora_magic_every.0, self.sora_magic_every.1, self.sora_shot_every, self.sora_situation_every, self.sora_switch_every, self.sora_recover_chance * 100.0, self.sora_recover_delay,
            g.parry_x, g.read_add, g.block_chance * 100.0, g.poke_every, g.poke_hits, g.poke_clear, g.press_reach, g.bash_stagger * 100.0, g.tornado_count, self.donald_cast_every, self.donald_approach, self.donald_retreat
        )
    }
}

impl FriendBoss {
    /// (MB3-AGGRO) His hostile tuning ([`CopyAggro`]): a Goofy copy's AI memory carries the Goofy part, a Donald copy's party the
    /// Donald part, and his Customize settings become the real ones that fight hardest (Combat Style "Be Yourself", Abilities
    /// "Go Wild"; Recovery and its targets kept). `CopyAggro::party()`: all of it taken off again (his copied settings stay).
    pub fn set_aggro(&mut self, a: CopyAggro) {
        self.aggro = Some(a);
        let Some(id) = self.id() else { return };
        let mut mem = self.rt.party.gof.ai.iter().position(|m| m.member == id).map_or_else(|| kh_party::goofy::ai::AiMemory::new(id), |n| self.rt.party.gof.ai.swap_remove(n));
        mem.copy = if self.chara == PartyChara::Goofy { a.goofy_tune() } else { None };
        self.rt.party.gof.ai.push(mem);
        self.rt.party.don_prog.copy = if self.chara == PartyChara::Donald { a.donald_tune() } else { None };
        if a.on {
            let s = self.rt.ai_settings(self.chara);
            self.rt.set_ai_settings(self.chara, kh_host::party::FriendAiSettings { battle: 1, ability: 0, ..s });
        }
    }
}

// ============================================================================ (MIMIC-BOSS hotfix 3, MB3-COMBAT) the copies' combat
//
// (THE USER 2026-11-06: "make sure mimic Donald get's properly hit stunned by Donald's missiles from his standard combo. ... it
// does almost no damage to you when it does it's finisher move on you and goofy does almost no damage to it when he does his
// finisher on it. Fix this. It also needs to play Goofy's animation for getting knocked down when it's stagger breaks.")

/// (MIMIC-BOSS hotfix 3, MB3-COMBAT) A PARTY HIT ON THE BOSS'S BODY AS THE COPY'S OWN REACTION: no HP (the copy's HP mirrors the
/// game's), and the victim's classification made again on the COPY'S OWN body. The hit came classified against the Elden Ring
/// boss body (kh-core `classify` on `ErTargets::body`: a boss of more than `target.hp_armoured` 1500 HP is an armoured STRONG
/// body), so a SMALL hit carried `StrongBodyDamage1/2` and kh-react's gate (0x140DF84F0) refused the copy any reaction: Donald's
/// combo missiles (DON_MISSILE, kind Small) never stunned a Donald copy. A copy is a playable character (Sora, Donald, Goofy):
/// KH3's DEFAULT hurt body, as every hit of an enemy on the party's friends is taken. Every party hit, melee or projectile, goes
/// through this one rule for every copy.
pub fn copy_reaction_hit(hit: &HitRecord) -> HitRecord {
    use kh_core::combat::classify::{classify, DamageClass, MapResponse, Shape};
    let mut h = *hit;
    h.damage = 0.0;
    let mut info = h.info;
    let c = classify(&kh_core::host::TargetBody::DEFAULT, MapResponse::Other, &Shape::default(), DamageClass { kind: h.damage_kind, attribute: h.attribute }, &mut info, &mut |_, _| 0, &mut |_| 0);
    h.info.victim_result = c.victim;
    h
}

/// (MIMIC-BOSS hotfix 3, MB3-COMBAT) THE DEFENSE OF A FOE OF LEVEL `level` IN KH3's CalcDamage: `EnemyBaseStatus[level]
/// .m_BaseDefensePower` (sheets/entities/Load/Common/EnemyBaseStatus.json; kh-core `abilities::xs::ENEMY_BASE_DEFENSE`, pinned to
/// the game's table by its test; docs/sora-stats-spec.md 4.1: an enemy's stats are that table's row of its battle level; the
/// enemy rows' own `m_DefensePower` is 0 in all 271 rows of DataTable/Enemy/Base).
pub fn mirror_defense(level: u32) -> i32 {
    let l = level.clamp(1, 99) as usize;
    kh_core::abilities::xs::ENEMY_BASE_DEFENSE.get(l - 1).copied().unwrap_or(0)
}

/// (MIMIC-BOSS hotfix 3, MB3-COMBAT) KH3's damage floor on an ENEMY victim at battle level `level`: `1 + ceil(0.1 x level)`
/// (CalcDamage 0x1406325A0..0x14063261A, defender kinds 1 / 5 with `m_DamageMin` 0; sheet sora-stats-rules `damage.min_rule`).
pub fn foe_floor(level: u32) -> i32 {
    1 + kh_core::status::math::ue_ceil(level.clamp(1, 99) as f32 * 0.1)
}

/// (MIMIC-BOSS hotfix 3, MB3-COMBAT) THE COPY AS A PARTY HIT'S VICTIM (`ErTargets::defense` of its body).
///
/// THE CAUSE (the user's log, kh-er.prev.log, the Goofy-led Nokron fight): MIMIC-BOSS hotfix 2 gave the boss's body the copy's
/// whole PARTY Defense in the party's KH3 formula, ON TOP OF Elden Ring's own body defence that every bullet still meets (the
/// `x0.27 after its defences` of each LANDED line): the defence was counted twice. KH3's base is `max(S - D, 1)`
/// (0x140632240 step 4) and Goofy's port balance is a tank's (level 50: Strength 28, Defense 48, party-balance-levels), so
/// Goofy against his own copy was base 1, every hit its power x 1: `[712.458] hit: Sora -> target 42 (c0000 team 6)
/// 'GOOFY_BREAKER' kind Blow 8.0 KH` -> `CAPPED ... the KH side expected 8` on a 4680 HP boss (30 x 156): 0.17 % for his
/// finisher. Every other Elden Ring body the party hits has NO KH3 Defense (`ErTargets::defense` None: `Defender::NONE`, D 0)
/// and its own Elden Ring defence once; the user's "x30 the health" was asked against that (hotfix 2: the copies died in
/// 10 - 17 s at 1188 HP).
///
/// THE RULE: as every Elden Ring body: Defense 0 in KH3's formula (Elden Ring's own body defence takes the bullet once, as on
/// every boss); the copy's OWN resists (the user's "proper resistances", hotfix 2), Damage Control's HP share and bio type are
/// kept; KH3's enemy kind and an enemy row's cap 255 (`m_DamageMax` of the boss rows; a friend's 24 is the party's own row).
pub fn copy_as_foe(own: TargetDefense) -> TargetDefense {
    TargetDefense { defense: 0, enemy_kind: true, damage_min: 0, damage_max: 255, ..own }
}

/// (MIMIC-BOSS hotfix 3, MB3-COMBAT) THE COPY'S HIT ON THE LEADER (the host's `deliver`, before his damage path).
///
/// THE CAUSE (kh-er.prev.log, the same fight): the copy's own kh-core / kh-party formula took the LEADER's party Defense
/// (`BossTargets::defense`) against the copy's PARTY Strength (the same character's): Goofy 28 - 48 -> base 1, so the copy's
/// Breaker (8.0x) was `Goofy: Hit { target: TargetId(1296171010), damage: 9.6, ... "GOOFY_BREAKER" }` (8 x the x1.200 later
/// journey rate) -> `Damaged { amount: 9, hp: 95 }` of 156, its pushes 1.2. KH3 never pairs a party Strength with a party
/// Defense: the party's Strength meets a FOE's Defense and floor.
///
/// THE MIRROR RULE: the copy hits the leader as KH3's party hits a foe of the copied leader's level: the base is
/// `max(S - EnemyBaseStatus[level] Defense, 1 + ceil(0.1 x level))` ([`mirror_defense`], [`foe_floor`]) in place of
/// `max(S - D_leader, 1)`, S its own Strength (Magic for a magic row, `info+0x50` bit 0). Everything else of the hit stays as its
/// own formula made it (the row's power, abilities, forms, the boss's attack rate); his own resists, guards, Second Chance and
/// the MB3-FREEZE status rule run after, in his damage path. A forced hit (`HIT_FLAG_FORCE`: Zettaflare) and a hit that takes no
/// HP are left alone. Returns the damage before and after when it changed (for the host's log).
pub fn mirror_hit_on_leader(hit: &mut HitRecord, copy_stats: (i32, i32), level: u32, leader_defense: i32) -> Option<(f32, f32)> {
    if hit.flags & kh_core::host::HIT_FLAG_FORCE != 0 || hit.damage <= 0.0 || !hit.takes_hp() {
        return None;
    }
    let s = if hit.info.flags50 & 1 != 0 { copy_stats.1 } else { copy_stats.0 };
    let old = (s - leader_defense).max(1);
    let new = (s - mirror_defense(level)).max(foe_floor(level));
    if new == old {
        return None;
    }
    let before = hit.damage;
    hit.damage = (before * new as f32 / old as f32).ceil();
    Some((before, hit.damage))
}

/// (MIMIC-BOSS hotfix 3, MB3-COMBAT) A COPY'S STANCE BROKEN BY THE PARTY (a Goofy's Stagger book on the boss's body: kh-er
/// targets.rs `stagger_break_now`) AS ITS CHARACTER'S OWN KNOCKDOWN: KH3's BLOW damage state from `from` with no HP, on the
/// copy's own body ([`copy_reaction_hit`] rules), through its own hurt rules: a friend's blow -> down -> get-up clips (kh-party
/// hurt.rs `Blow(index, phase)` 0x14125FC00: M_0227 / M_0228, the down M_0234 / M_0238 / M_0213 of HIS set: Goofy's for Goofy,
/// Donald's for Donald), Sora's own blow and down for a Sora copy ([`knockdown_of_sora`]). Not guardable (the break opened him).
pub fn knockdown_hit(at: Vec3, from: Vec3) -> HitRecord {
    let d = Vec3::new(at.x - from.x, at.y - from.y, 0.0).normalize_or_zero();
    let mut h = HitRecord { position: at, direction: d, damage: 0.0, damage_kind: kh_core::enums::ETresDamageKind::Blow, attack_name: "MIMIC_STANCE_BREAK", ..HitRecord::empty(TargetId::NONE) };
    h.attacker = kh_core::host::HitAttacker { position: from, yaw: 0.0, radius: 40.0, valid: true };
    h.info.flags50 |= kh_core::host::HitInfo::FLAG50_IGNORE_GUARD;
    copy_reaction_hit(&h)
}

/// (MIMIC-BOSS hotfix 3, MB3-COMBAT) [`knockdown_hit`] for a Sora copy's core (`take_hit`).
pub fn knockdown_of_sora(at: Vec3, from: Vec3) -> kh_core::states::HitOnPlayer {
    reaction_of(&knockdown_hit(at, from))
}

impl FriendBoss {
    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT) His stance broken by the party: his own knockdown ([`knockdown_hit`]). True: it started.
    pub fn knock_down(&mut self, from: Vec3) -> bool {
        let Some(at) = self.view().map(|v| v.position) else { return false };
        let before = self.id().and_then(|id| self.rt.party.member(id)).map_or(0, |m| m.damage_reactions);
        let _ = self.rt.receive_hit(self.chara, &knockdown_hit(at, from));
        self.id().and_then(|id| self.rt.party.member(id)).is_some_and(|m| m.damage_reactions != before)
    }

    /// (MIMIC-BOSS hotfix 3, MB3-COMBAT) His Strength, Magic and level ([`mirror_hit_on_leader`]).
    pub fn stats(&self) -> (i32, i32, u32) {
        self.id().and_then(|id| self.rt.party.status(id)).map_or((0, 0, 1), |s| (s.strength(), s.magic(), s.level))
    }
}

// ------------------------------------------------------------------------------- (MIMIC-BOSS hotfix 3) the copies' heals

/// (MIMIC-BOSS hotfix 3, THE USER 2026-11-07: "anything the mimic boss or summon does that would heal it should heal it for
/// the same % of HP that Sora would get healed by by using it") THE SHARE OF ITS MAXIMUM A COPY HEALED ITSELF BY in one step
/// of its own KH3 core (any source: Cure / Cura / Curaga, items, Leaf Bracer's cast, drain, a form's or finisher's heal,
/// Donald's or Goofy's own heals on a friend copy): its KH3 HP after the step minus before it (after the game's HP was
/// mirrored in), over its KH3 maximum, which is the copied leader's. 0 for no gain (its losses are the game's: hits land on
/// its Elden Ring body).
pub fn heal_share(kh_before: i32, kh_after: i32, kh_max: i32) -> f32 {
    if kh_after <= kh_before || kh_max <= 0 || kh_before <= 0 {
        return 0.0;
    }
    (kh_after - kh_before) as f32 / kh_max as f32
}

/// (MIMIC-BOSS hotfix 3) The copy's Elden Ring HP after a heal of `share` of its maximum: the same share of the body's own
/// maximum (the boss's 30 x pool, its summoned Mimic's boss-sized pool), so a heal is never tiny or huge in absolute terms;
/// rounded, at least 1 for any heal, never past the maximum, never on a dead body.
pub fn healed_er_hp(share: f32, er_hp: i32, er_max: i32) -> i32 {
    if er_hp <= 0 || share <= 0.0 || !share.is_finite() {
        return er_hp;
    }
    let gain = ((share * er_max as f32).round() as i32).max(1);
    er_hp.saturating_add(gain).min(er_max.max(er_hp))
}

// ====================================== (MIMIC-BOSS hotfix 3, MB3-FOGRISE 2) the fog pass as Elden Ring checks it (both bosses)

/// (MIMIC-BOSS hotfix 3, MB3-FOGRISE 2; THE USER 2026-11-07: "the other mimic tear boss must start as soon as you enter the
/// fog gate as well, not just the Nokron City one") THE REGIONS OF THE RISE as the MSBs have them (entity, the box's place =
/// its bottom's centre, Y rotation in degrees, width / depth / height; `trace/msb_m12_02.js`, `msb_m30_20.js`):
/// - 12022850 "前座ボス開始領域": Nokron's slab just inside its fog (the fog event 12022869's walk target and region);
/// - 12022851 "前座ボス初戦開始領域": Nokron's first-encounter box;
/// - 30202800 "ボス戦開始領域": the Stray's box just inside its fog (9005800's walk target and region, and 30202810's).
pub const RISE_REGIONS: [(u32, [f32; 3], f32, [f32; 3]); 3] = [
    (12_022_850, [965.491, -617.627, 1163.893], 29.895, [2.0, 11.6, 2.0]),
    (12_022_851, [1020.755, -618.381, 1129.591], -60.0, [52.0, 65.0, 15.0]),
    (30_202_800, [-89.134, 1278.265, -243.392], 0.0, [5.0, 3.0, 2.0]),
];

/// A region of the rise by its entity.
pub fn rise_region(entity: u32) -> Option<([f32; 3], f32, [f32; 3])> {
    RISE_REGIONS.iter().find(|r| r.0 == entity).map(|r| (r.1, r.2, r.3))
}

/// The MSB place of an Elden Ring physics place, by the pool: the physics frame is the MSB's moved (the user's Nokron run:
/// the body at its pool's dummy and at its nest both fit one translation, no turn).
pub fn msb_place(pool_er: [f32; 3], pool_msb: [f32; 3], p_er: [f32; 3]) -> [f32; 3] {
    [p_er[0] - pool_er[0] + pool_msb[0], p_er[1] - pool_er[1] + pool_msb[1], p_er[2] - pool_er[2] + pool_msb[2]]
}

/// The leader is in the fog's own region (the one the fog event walks him into and the rise's event reads with the fog flag).
pub fn in_fog_region(row: &MimicBossRow, leader_msb: [f32; 3]) -> bool {
    rise_region(row.fog_region).is_some_and(|(c, r, d)| in_msb_box(c, r, d, leader_msb))
}

/// ELDEN RING'S FOG EVENT (common 9005800; Nokron's own copy 12022869), read for the timing: the fog's action, then the walk
/// (2004[14]: the player turned to the fog's region and anim 60060, the event waiting for its end), then a wait of at most
/// 3 s (1[0] 3.0) for the player IN that region; only then is the fog flag set (2003[69]). If the 3 s pass first, the event
/// ends and starts over without the flag. So the frame the fog flag is set, the leader stands in the fog's region: the same
/// two facts the rise's event (30202810; Nokron's return visit 12022860) waits on. It starts on its next check.
pub const FOG_EVENT_REGION_WAIT_S: f32 = 3.0;

/// The pool's own rise (anim 20010 to its SpEffect 16307, when the body is enabled), from the user's first Nokron run: the
/// music ask at 173.25 s less the event's 8 s gives the start at 165.25 s, the body at 168.56 s.
pub const POOL_RISE_S: f64 = 3.31;

/// The rise is LATE: the fog flag on this long (s) and still no body (the pool's rise is 3.31 s): the event is waiting on
/// something (the log then says where the leader stands against its regions).
pub const RISE_LATE_S: f64 = 6.0;

/// The rise is late `since` s after the fog flag (`body_enabled`: the game has enabled the body).
pub fn rise_late(defeated: bool, since: f64, body_enabled: bool) -> bool {
    !defeated && !body_enabled && since > RISE_LATE_S
}

// ------------------------------------------------------------------------------- (MIMIC-BOSS) untouchable while it forms

/// (MIMIC-BOSS, THE USER 2026-11-07: "also mimic tear boss should not be targetable or attackable until it has transformed into
/// Sora") Where a copy is in its forming: the leader's copy (Sora, Donald or Goofy) not yet out of the silver and in control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyForm {
    /// Rising out of the pool / forming: no AI of its own yet.
    Forming,
    /// It has taken over (its AI acts).
    Formed,
}

/// (MIMIC-BOSS) THE BOSS'S BODY IS UNTOUCHABLE (no KH lock, soft target, AI pick or hit of any route: kh-er invuln.rs's
/// `Why::Forming`) from the fight's start (the solo fight: the fog passed) while its copy of the leader is not formed: no
/// copy yet (the pool rising, the copying) or one still forming. Never before the fight, never once beaten.
pub fn boss_untouchable(solo: bool, defeated: bool, copy: Option<CopyForm>) -> bool {
    solo && !defeated && copy != Some(CopyForm::Formed)
}

/// (MIMIC-BOSS) The boss-summoned Mimic Tear's body is untouchable until its copy has formed.
pub fn summon_untouchable(copy: CopyForm) -> bool {
    copy == CopyForm::Forming
}
