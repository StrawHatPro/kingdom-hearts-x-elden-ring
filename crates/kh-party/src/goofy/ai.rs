//! (GF-AI) AI GOOFY (documentation/GOOFY_OVERHAUL.md 12): the shield of the party when he does not lead. [`think`] is the pure
//! brain; [`party_pass`] (run in `Party::step` beside Donald's) feeds it and carries its answer out by pressing the same buttons
//! a player would (`GoofyRun::ai`, `pad::goofy::ai_drive`). So the moves, their ranks and the one judgement of a hit
//! (`pad::goofy::on_hit`) are the leader's: he has no move of his own and no rule of his own (ONESTAT).
//!
//! HOW HE IS DRIVEN: his KH3 behaviour trees stay for following, formation and pathing. In battle a tree move toward anything
//! out of the leash (`ai.leash` from the leader) is cut to a guard post beside the leader. The trees' attack requests (Push,
//! Sweep, Jump, Tornado, Turbo, Discus) are REPLACED before the pawn takes them: a melee request only lets his next string
//! start, a Tornado / Discus request becomes that Art when it is one of his two equipped. Turbo stays unused: the rushes would
//! carry him off the leader.
//!
//! (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique, Goofy's big damage should solely come from his
//! guard break attacks from 6.3" and "Make sure Goofy as an AI party member uses his new tools properly") HIS LOOP IS THE
//! PLAYER'S: parry for 0 damage and Stagger, break the enemy, Goofy Breaker on it. No counter window is waited for.
//!
//! THE RULES, in priority order:
//! 1. Goofy Breaker when one is offered (`ai.breaker_delay`): only he can cash a break (Sora and Donald have no Breaker; KH3's
//!    friends finish too), so he takes every one he can reach.
//! 2. What the telegraphs show coming at him: he reads each attack once (`ai.read_delay_*`), rolls his timing by level
//!    (`ai.parry_rate_*`), and answers with the technique for its class among those he has learned ([`choose`]). A roll he
//!    fails he leaves unanswered (COMBAT-FIX, `ai.miss_blocks` 0: KH3's Goofy tree has no guard; he carries on), or with the
//!    row at 1 it is a mistimed answer ([`mistimed`]): the guard takes it while GUARD can pay, else the hop, or it lands. At Resolve
//!    `ai.safe_pips` he deflects a sweep rather than jump it, and In the Zone every deflect is the right one, so he keeps it.
//! 3. A BROKEN enemy out of his reach: he walks in to it (`ai.breaker_chase`, Finish the Job `ai.breaker_chase_focus`) while
//!    enough of its window is left (`ai.breaker_min_left` past the walk); not with Hands Off.
//! 4. Interpose when the leader is low, Taunt when a friend has `ai.taunt_count` enemies on him; then his own two Arts when they
//!    pay off ([`own_art`]: Goofy Bash on an enemy whose Stagger is nearly full, Tornado on a pack), never one that waits on its
//!    cooldown, by the Customize "Abilities" setting (Go Wild / Watch MP keeps `ai.mp_reserve` / Conserve: Taunt and Interpose
//!    only).
//! 5. GUARD under `ai.guard_low`: no string, back to the leader until it refills (no guard into a Guard Break).
//! 6. Back to the leader when a blow threw him out of the leash, else basic strings on what stands near (`ai.poke_*`; at once into
//!    the opening of an enemy his perfect deflect bounced, `ai.follow_up`), and only what is near the party. Hands Off strikes
//!    nothing; Finish the Job strikes the most staggered. Never inside his string's end lag.

use super::learn::{self, GoofyMove, RankBank};
use super::parry::Tech;
use super::{Resolve, Telegraph, K};
use crate::status::AiSettings;
use crate::command::{MemberCommand, MoveGoal};
use crate::donald::cast::SORA_MEMBER;
use crate::ids::{Chara, MemberId};
use crate::pad::goofy::AiDrive;
use glam::Vec3;
use kh_core::host::{AttackClass, TargetId};
use kh_core::input::{PadButton, PadButtons, PadState};

/// An enemy as he sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiEnemy {
    pub id: TargetId,
    pub pos: Vec3,
    pub radius: f32,
    /// Its Stagger gauge's fill, 0 ..= 1 (the party's book).
    pub stagger: f32,
    /// BROKEN now: the seconds of its window left (0: not broken).
    pub broken: f32,
}

/// A friend as he sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiFriend {
    pub member: MemberId,
    pub pos: Vec3,
    pub hp: i32,
    pub hp_max: i32,
    pub ko: bool,
    pub leader: bool,
    /// The enemies on him.
    pub foes: u32,
}

impl AiFriend {
    fn share(&self) -> f32 {
        self.hp.max(0) as f32 / self.hp_max.max(1) as f32
    }
}

/// (GF-POLISH) What the host read of an enemy's aim: `on` the party member its AI has as its target, None: nobody of the party.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnemyAim {
    pub enemy: TargetId,
    pub on: Option<MemberId>,
}

/// What he can do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    /// Standing or walking: everything.
    Free,
    /// In a shove (X still cuts it).
    Striking,
    /// Shield up, or a parry's recoil.
    Guarding,
    /// In the air.
    Airborne,
    /// A reaction, an Art, the Breaker, a team attack, KO ...
    Busy,
}

/// Everything [`think`] reads.
#[derive(Clone, Copy, Debug)]
pub struct AiInput<'a> {
    pub pos: Vec3,
    pub level: u32,
    pub ranks: RankBank,
    pub resolve: Resolve,
    pub stance: Stance,
    /// X would act now.
    pub can_x: bool,
    pub can_guard: bool,
    /// The share of GUARD left.
    pub guard: f32,
    pub hop_cd: f32,
    pub in_battle: bool,
    /// Y would be the Breaker.
    pub breaker: bool,
    /// Ground shoves done in this string.
    pub combo: u8,
    /// His string's end lag holds a new one (pad/goofy_combo.rs `restart_held`).
    pub end_lag: bool,
    /// The two Arts he has equipped and may use now (learned, not cooling down, payable: `goofy_arts::ai_art_moves`).
    pub arts: [Option<GoofyMove>; 2],
    /// Their MP prices at his rank (0 for an empty slot).
    pub art_cost: [i32; 2],
    /// Interpose is learned and not cooling down (equipped or not: `ai.interpose_unequipped`).
    pub interpose_ready: bool,
    pub mp: i32,
    pub mp_max: i32,
    /// The Customize AI settings the player chose (Combat Style, Abilities).
    pub style: AiSettings,
    /// His perfect deflect bounced this enemy last step (the rebound's opening).
    pub rebound: Option<TargetId>,
    /// His tree asked for this attack this step (the enemy it named).
    pub asked: Option<TargetId>,
    /// His tree asked for this Art this step.
    pub art_asked: Option<GoofyMove>,
    pub enemies: &'a [AiEnemy],
    pub friends: &'a [AiFriend],
    pub telegraphs: &'a [Telegraph],
}

/// What he answers an attack with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Tap X, stick neutral.
    Deflect,
    /// Tap X, stick toward the enemy (a thrust).
    Pinning,
    /// Tap X, stick toward the shooter.
    Reflect,
    /// Hold X through the cast, let go and press on impact.
    Ward,
    /// Hold X, stick away.
    Brace,
    /// B over it.
    JumpOver,
    /// The sideways hop as the grab closes.
    Slip,
    /// The sideways hop: he has no answer of his own to this.
    Dodge,
    /// The shield up too early: the guard takes it.
    Block,
    /// Nothing in time.
    Stand,
}

/// What he did last (the harness, the trace).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiWord {
    Answer(Answer),
    Breaker,
    /// Walking in to a BROKEN enemy for the Breaker.
    Chase,
    /// GUARD low: back to the leader, no string.
    Recover,
    /// A string into a rebound's opening.
    FollowUp,
    Taunt,
    Interpose,
    Art(GoofyMove),
    Poke,
}

/// The share of attacks he times well at `level`.
pub fn parry_rate(level: u32) -> f32 {
    let t = (level.clamp(1, 99) - 1) as f32 / 98.0;
    K.ai_parry_rate_low + (K.ai_parry_rate_high - K.ai_parry_rate_low) * t
}

/// (COMBAT-FIX) A Mimic Tear boss copy's base share at `level` (`ai.copy_parry_rate_*`, x its `CopyTune::parry_x`): the ally's
/// rates moved (THE USER 2026-11-07: "goofy as an ai ally at level 1 should have a 5% chance, and a 65% change at level 99"),
/// the boss copy keeps its own.
pub fn copy_parry_rate(level: u32) -> f32 {
    let t = (level.clamp(1, 99) - 1) as f32 / 98.0;
    K.ai_copy_parry_rate_low + (K.ai_copy_parry_rate_high - K.ai_copy_parry_rate_low) * t
}

/// Seconds from seeing an attack until he can answer it, at `level`.
pub fn read_delay(level: u32) -> f32 {
    let t = (level.clamp(1, 99) - 1) as f32 / 98.0;
    K.ai_read_delay_low + (K.ai_read_delay_high - K.ai_read_delay_low) * t
}

fn learned(i: &AiInput, t: Tech) -> bool {
    learn::rank(&i.ranks, t.mv(), i.level) > 0
}

/// THE TECHNIQUE for an attack class, among those he has learned (4.3). Without his own answer a blockable attack is a plain
/// deflect; an unblockable one can only be dodged.
pub fn choose(class: AttackClass, i: &AiInput) -> Answer {
    // In the Zone every deflect counts as the right technique (7)
    if i.resolve.in_zone() {
        return Answer::Deflect;
    }
    let or_plain = |t: Tech, a: Answer| if learned(i, t) { a } else { Answer::Deflect };
    match class {
        AttackClass::Normal => Answer::Deflect,
        AttackClass::Thrust => or_plain(Tech::PinningGuard, Answer::Pinning),
        AttackClass::Projectile => or_plain(Tech::ShieldReflect, Answer::Reflect),
        AttackClass::Magic => or_plain(Tech::SpellWard, Answer::Ward),
        // the jump is the risky answer: a miss costs Resolve, a mistimed deflect only GUARD
        AttackClass::Sweep if (i.resolve.pips() as f32) < K.ai_safe_pips => or_plain(Tech::HopCounter, Answer::JumpOver),
        AttackClass::Sweep => Answer::Deflect,
        AttackClass::Grab => if learned(i, Tech::SlipCounter) { Answer::Slip } else { Answer::Dodge },
        AttackClass::Heavy => if learned(i, Tech::BullBrace) { Answer::Brace } else { Answer::Dodge },
    }
}

/// A timing roll that failed. A blockable attack meets the shield too early, while GUARD can pay for it; an unblockable one
/// lands: he was late.
pub fn mistimed(a: Answer, i: &AiInput) -> Answer {
    match a {
        Answer::Slip | Answer::Brace | Answer::Dodge | Answer::Stand => Answer::Stand,
        _ if i.can_guard && i.guard >= K.ai_guard_low => Answer::Block,
        _ if i.hop_cd <= 0.0 => Answer::Dodge,
        _ => Answer::Stand,
    }
}

/// An answer once he has read the attack: what the time left allows.
fn fit(a: Answer, open: f32, i: &AiInput) -> Answer {
    let brace = super::parry::brace_set(learn::rank(&i.ranks, Tech::BullBrace.mv(), i.level));
    match a {
        Answer::Ward if open < K.ward_hold_min + K.ai_ward_release => Answer::Deflect,
        Answer::Brace if open < brace => if i.hop_cd <= open { Answer::Dodge } else { Answer::Stand },
        Answer::JumpOver if open <= 0.0 => Answer::Deflect,
        Answer::Slip | Answer::Dodge if i.hop_cd > open => Answer::Stand,
        a => a,
    }
}

/// One attack he has seen coming.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeenAttack {
    pub attacker: TargetId,
    pub class: AttackClass,
    /// Seconds until he has read it.
    pub read_left: f32,
    pub answer: Answer,
    /// He timed it well.
    pub good: bool,
    /// This one tap's error, seconds.
    pub jitter: f32,
    /// The side he hops to (-1, 1).
    pub side: f32,
    pub fitted: bool,
    pub seen: bool,
    pub taps: u8,
    pub since_tap: f32,
    /// Seconds its hit window has stood open.
    pub open_for: f32,
    /// How far a held answer has gone.
    pub phase: u8,
}

/// One Goofy's AI memory.
#[derive(Clone, Debug, PartialEq)]
pub struct AiMemory {
    pub member: MemberId,
    pub seen: Vec<SeenAttack>,
    /// Seconds until the next string, Taunt, Interpose, own step.
    pub poke_in: f32,
    pub taunt_in: f32,
    pub interpose_in: f32,
    pub move_in: f32,
    /// Seconds a Breaker has been on offer.
    pub breaker_wait: f32,
    /// The enemy a perfect deflect bounced, and the seconds its opening is still followed up.
    pub follow: Option<(TargetId, f32)>,
    /// What he did last, and how many attacks he rolled well of how many (the log, the harness).
    pub last: Option<AiWord>,
    pub rolled: u32,
    pub rolled_good: u32,
    /// (FINAL-FIX) The BROKEN enemies he has said something about, and what (one `goofy-ai:` line per change of verdict).
    pub noted: Vec<(TargetId, &'static str)>,
    /// (MIMIC-BOSS hotfix 3, MB3-AGGRO) A Mimic Tear boss's copy of Goofy: his hostile tuning ([`CopyTune`]). None: the party's
    /// own Goofy, untouched.
    pub copy: Option<CopyTune>,
    /// (REVENGE, 2026-11-07) A Mimic Tear boss copy's revenge counter (kh-er mimicrevenge.rs): these Arts in order, the first he
    /// knows, wears and may use now goes before any other choice of his once he is free (his pad's own rank, MP and cool-down
    /// rules still decide whether it starts). Empty: none.
    pub revenge_arts: Vec<GoofyMove>,
}

/// (MIMIC-BOSS hotfix 3, MB3-AGGRO; THE USER 2026-11-06: "the more aggresive AI for the mimic boss needs to apply to Donald and
/// Goofy's mimic bosses as well. Goofy's boss currently stands still and does nothing, waiting to parry you. Make the mimic boss
/// worse at countering everything.") A BOSS COPY OF GOOFY'S AI TUNING: the same moves, ranks, cooldowns, MP and GUARD rules (his
/// pad's), only his CHOICES differ. The kh-partyhost sheet rows `mimic-boss.json` `aggro.goofy.*` fill it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CopyTune {
    /// x his timing roll by level (`ai.parry_rate_*`).
    pub parry_x: f32,
    /// Seconds added to his read of an attack (`ai.read_delay_*`).
    pub read_add: f32,
    /// A failed roll meets the shield (a block) only this share of the time; else it lands.
    pub block_chance: f32,
    /// Seconds from the end of a string to the next (`ai.poke_every`).
    pub poke_every: f32,
    /// Shoves in one string (`ai.poke_hits`; his string's own length still bounds it).
    pub poke_hits: f32,
    /// No string starts with an answered attack due within this long (`ai.poke_clear`).
    pub poke_clear: f32,
    /// He walks in to the nearest enemy within this gap (cm) instead of waiting for it; 0: never.
    pub press_reach: f32,
    /// Goofy Bash (equipped, ready) on an enemy in reach whose Stagger is at least this full (`ai.bash_stagger`).
    pub bash_stagger: f32,
    /// Goofy Tornado (equipped, ready) with this many enemies in its whirl (`ai.tornado_count`).
    pub tornado_count: f32,
}

impl AiMemory {
    pub fn new(member: MemberId) -> AiMemory {
        AiMemory { member, seen: Vec::new(), poke_in: 0.0, taunt_in: 0.0, interpose_in: 0.0, move_in: 0.0, breaker_wait: 0.0, follow: None, last: None, rolled: 0, rolled_good: 0, noted: Vec::new(), copy: None, revenge_arts: Vec::new() }
    }

    fn tick(&mut self, dt: f32) {
        for c in [&mut self.poke_in, &mut self.taunt_in, &mut self.interpose_in, &mut self.move_in] {
            *c = (*c - dt).max(0.0);
        }
        self.follow = self.follow.map(|(t, left)| (t, left - dt)).filter(|f| f.1 > 0.0);
        for a in self.seen.iter_mut() {
            a.read_left -= dt;
            a.since_tap += dt;
        }
    }

    /// The attacks on the list that may be for him: new ones are read and rolled once, gone ones forgotten.
    fn see(&mut self, i: &AiInput, unit: &mut dyn FnMut() -> f32) {
        for a in self.seen.iter_mut() {
            a.seen = false;
        }
        for t in i.telegraphs.iter().filter(|t| meant_for_him(t, i)) {
            if let Some(a) = self.seen.iter_mut().find(|a| a.attacker == t.attacker && a.class == t.class) {
                a.seen = true;
                continue;
            }
            let wanted = choose(t.class, i);
            // (MB3-AGGRO) a boss copy: his roll x `parry_x`, his read `read_add` later
            let base = self.copy.map_or(parry_rate(i.level), |c| copy_parry_rate(i.level) * c.parry_x);
            let rate = base * if wanted == Answer::Deflect { 1.0 } else { K.ai_tech_rate };
            let good = unit() < rate;
            let read = read_delay(i.level) * (1.0 + (unit() * 2.0 - 1.0) * K.ai_read_jitter) + self.copy.map_or(0.0, |c| c.read_add.max(0.0));
            let jitter = (unit() * 2.0 - 1.0) * K.ai_tap_jitter;
            let side = if unit() < 0.5 { -1.0 } else { 1.0 };
            self.rolled += 1;
            self.rolled_good += good as u32;
            // (MB3-AGGRO) a boss copy's failed roll meets the shield only `block_chance` of the time (one more roll, copies only)
            // (COMBAT-FIX) the ally does not answer a failed roll (`ai.miss_blocks` 0): KH3's own Goofy tree has no guard, he
            // carries on with his strings, Arts and abilities (THE USER 2026-11-07: "Goofy should still use his attacks and
            // abilties as well as deflecting")
            let missed = match self.copy {
                Some(c) if unit() >= c.block_chance => Answer::Stand,
                Some(_) => mistimed(wanted, i),
                None if K.ai_miss_blocks == 0.0 => Answer::Stand,
                None => mistimed(wanted, i),
            };
            self.seen.push(SeenAttack { attacker: t.attacker, class: t.class, read_left: read, answer: if good { wanted } else { missed }, good, jitter, side, fitted: false, seen: true, taps: 0, since_tap: 0.0, open_for: 0.0, phase: 0 });
        }
        self.seen.retain(|a| a.seen);
    }
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, 0.0)
}

fn gap(from: Vec3, e: &AiEnemy) -> f32 {
    (flat(e.pos - from).length() - e.radius).max(0.0)
}

/// An attack on the list may be meant for him: its thrower stands in reach of him. Nothing says whom it aims at (and he draws
/// them on purpose), so a swing in reach is answered; a far shot clearly nearer to a friend is that friend's.
fn meant_for_him(t: &Telegraph, i: &AiInput) -> bool {
    let Some(e) = i.enemies.iter().find(|e| e.id == t.attacker) else { return false };
    let shot = matches!(t.class, AttackClass::Projectile | AttackClass::Magic);
    let mine = gap(i.pos, e);
    if !shot {
        return mine <= K.ai_parry_range;
    }
    mine <= K.ai_shot_range && !i.friends.iter().filter(|f| !f.ko).any(|f| gap(f.pos, e) + K.ai_friend_margin < mine)
}

fn leader<'a>(i: &'a AiInput) -> Option<&'a AiFriend> {
    i.friends.iter().find(|f| f.leader && !f.ko)
}

/// The enemy is his to fight: near the party, not somewhere he would have to leave the leader for.
fn near_party(e: &AiEnemy, i: &AiInput) -> bool {
    leader(i).is_none_or(|l| gap(l.pos, e) <= K.ai_leash)
}

/// The buttons, the stick and the lock of this step.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct AiOut {
    pub pressed: PadButtons,
    pub held: PadButtons,
    /// The stick: world yaw (None: neutral).
    pub stick: Option<f32>,
    pub lock: TargetId,
    /// The Art he fires.
    pub art: Option<GoofyMove>,
    /// He has drifted off the leader: back to him.
    pub back: bool,
    /// A BROKEN enemy out of his reach: walk in to it for the Breaker.
    pub chase: Option<TargetId>,
}

impl AiOut {
    fn press(&mut self, b: PadButton) {
        self.pressed.set(b, true);
        self.held.set(b, true);
    }

    fn hold(&mut self, b: PadButton) {
        self.held.set(b, true);
    }

    /// What `pad::goofy::ai_drive` takes.
    pub fn drive(&self) -> AiDrive {
        AiDrive { pad: PadState { held: self.held, pressed: self.pressed, released: PadButtons::NONE }, stick_yaw: self.stick.unwrap_or(0.0), stick_magnitude: if self.stick.is_some() { 1.0 } else { 0.0 }, lock: self.lock, art: self.art }
    }
}

const X: PadButton = PadButton::FaceLeft;

/// One step of the answer to an attack. True: this step is his answer's.
fn answer_step(a: &mut SeenAttack, t: &Telegraph, i: &AiInput, dt: f32, out: &mut AiOut) -> bool {
    let open = if t.active { 0.0 } else { t.eta };
    // a window that never closes (a sensor that lost its enemy) does not keep him behind his shield
    a.open_for += if t.active { dt } else { 0.0 };
    if a.open_for > K.ai_answer_max {
        return false;
    }
    if !a.fitted {
        a.fitted = true;
        a.answer = fit(a.answer, open, i);
    }
    let Some(e) = i.enemies.iter().find(|e| e.id == a.attacker) else { return false };
    let to = flat(e.pos - i.pos);
    let yaw = if to.length() > 1.0 { to.y.atan2(to.x).to_degrees() } else { 0.0 };
    out.lock = a.attacker;
    match a.answer {
        Answer::Deflect | Answer::Pinning | Answer::Reflect => {
            if open > K.ai_tap_lead + a.jitter || !i.can_x {
                return false;
            }
            out.hold(X);
            // a long hit window: the tap is renewed, no faster than the mash rule forgives (4.1)
            if a.taps == 0 || (t.active && a.since_tap >= K.deflect_mash_gap + dt) {
                out.press(X);
                if a.answer != Answer::Deflect {
                    out.stick = Some(yaw);
                }
                a.taps += 1;
                a.since_tap = 0.0;
            }
        }
        Answer::Block => {
            if open > K.ai_block_lead || !i.can_guard {
                return false;
            }
            if a.taps == 0 {
                // later than this the press would be a lucky deflect: a mistimed answer that was also late lands
                if open < K.deflect_window + K.second_deflect_late {
                    a.answer = Answer::Stand;
                }
                if a.answer == Answer::Stand || !i.can_x {
                    return false;
                }
                out.press(X);
                a.taps = 1;
            }
            out.hold(X);
        }
        Answer::Brace => {
            if open > K.ai_brace_lead || !i.can_guard {
                return false;
            }
            out.hold(X);
            out.stick = Some(yaw + 180.0);
            if a.taps == 0 && i.can_x {
                out.press(X);
                a.taps = 1;
            }
        }
        Answer::Ward => {
            if open > K.ai_ward_lead || !i.can_guard {
                return false;
            }
            match a.phase {
                0 => {
                    if i.can_x {
                        out.press(X);
                        a.phase = 1;
                    }
                }
                1 if open > K.ai_ward_release => out.hold(X),
                // let go ...
                1 => a.phase = 2,
                // ... and press on impact
                2 => {
                    out.press(X);
                    a.phase = 3;
                }
                _ => out.hold(X),
            }
        }
        Answer::JumpOver => {
            if open > K.ai_jump_lead {
                return false;
            }
            if a.taps == 0 && i.stance != Stance::Airborne {
                out.press(PadButton::JUMP);
                a.taps = 1;
            }
        }
        Answer::Slip | Answer::Dodge => {
            if open > K.ai_hop_lead || !i.can_x {
                return false;
            }
            if a.taps == 0 {
                out.press(X);
                out.stick = Some(yaw + 90.0 * a.side);
                a.taps = 1;
            }
        }
        Answer::Stand => return false,
    }
    true
}

/// THE DECISION of one step.
pub fn think(i: &AiInput, mem: &mut AiMemory, dt: f32, unit: &mut dyn FnMut() -> f32) -> AiOut {
    mem.tick(dt);
    mem.see(i, unit);
    let mut out = AiOut::default();
    // (MB3-AGGRO) a boss copy's "leader" is his stand-in at his own feet: every enemy is his to fight
    let copy = mem.copy;
    let mine = i.enemies.iter().filter(|e| copy.is_some() || near_party(e, i)).min_by(|a, b| gap(i.pos, a).total_cmp(&gap(i.pos, b)));
    if i.in_battle {
        out.lock = mine.map_or(TargetId::NONE, |e| e.id);
    }
    if i.stance == Stance::Busy {
        return out;
    }
    // (REVENGE) a boss copy's revenge counter before any other choice, once he is free: the first of its Arts he knows, wears
    // and may use now (none: the list is dropped and he fights on by his own rules)
    if !mem.revenge_arts.is_empty() && i.stance == Stance::Free {
        let arts = std::mem::take(&mut mem.revenge_arts);
        if let Some(m) = arts.into_iter().find(|m| learn::rank(&i.ranks, *m, i.level) > 0 && i.arts.contains(&Some(*m)) && art_allowed(i, *m)) {
            out.art = Some(m);
            mem.last = Some(AiWord::Art(m));
            return out;
        }
    }
    if let Some(t) = i.rebound {
        mem.follow = Some((t, K.ai_follow_up)); // (THE USER 2026-11-02, the bounce) its opening is his
    }
    // 1. Cashing in: Goofy Breaker, his big damage (THE USER 2026-11-02: "...solely come from his guard break attacks from 6.3")
    mem.breaker_wait = if i.breaker { mem.breaker_wait + dt } else { 0.0 };
    if i.breaker && mem.breaker_wait >= K.ai_breaker_delay {
        out.press(PadButton::FaceTop);
        if let Some(e) = i.enemies.iter().filter(|e| e.broken > 0.0).min_by(|a, b| gap(i.pos, a).total_cmp(&gap(i.pos, b))) {
            out.lock = e.id;
        }
        mem.last = Some(AiWord::Breaker);
        return out;
    }
    // 2. What is coming at him: the soonest attack he has read
    let open_in = |a: &SeenAttack| i.telegraphs.iter().find(|t| t.attacker == a.attacker && t.class == a.class).map_or(f32::MAX, |t| if t.active { 0.0 } else { t.eta });
    let next = mem.seen.iter().enumerate().filter(|(_, a)| a.read_left <= 0.0).min_by(|a, b| open_in(a.1).total_cmp(&open_in(b.1))).map(|(n, _)| n);
    if let Some(n) = next {
        let a = &mut mem.seen[n];
        let t = i.telegraphs.iter().find(|t| t.attacker == a.attacker && t.class == a.class);
        if t.is_some_and(|t| answer_step(a, t, i, dt, &mut out)) {
            mem.last = Some(AiWord::Answer(a.answer));
            return out;
        }
    }
    // (FINAL-FIX) a window that never closes (`ai.answer_max`: a sensor that lost its enemy) is no attack still coming: it no
    // longer holds back the walk to a BROKEN enemy or anything below, as it no longer holds his shield (`answer_step`)
    let poke_clear = copy.map_or(K.ai_poke_clear, |c| c.poke_clear);
    let due = mem.seen.iter().any(|a| a.answer != Answer::Stand && a.open_for <= K.ai_answer_max && open_in(a) <= poke_clear);
    // (FINAL-FIX) the Breaker's beat (`ai.breaker_delay`): Y is on offer and comes next, so nothing of his own starts in between
    // (in a crowd his Tornado started in the beat, he was busy and the window ran out; rule 1 is first). Parries (rule 2) stand.
    if i.breaker {
        if let Some(e) = i.enemies.iter().filter(|e| e.broken > 0.0).min_by(|a, b| gap(i.pos, a).total_cmp(&gap(i.pos, b))) {
            out.lock = e.id;
        }
        return out;
    }
    // 3. A BROKEN enemy out of reach: he walks in for the Breaker (not with Hands Off), while its window will still be open.
    // (THE USER 2026-11-03 follow-up, in game: a soldier broken twice and AI Goofy stood still) in battle or not: a gauge only
    // fills from the party's own blows, so a BROKEN enemy is one the party is fighting even when the leader's battle flag is
    // down or the enemy has not engaged yet; the lock-on's own filter (`goofy_shots::enemies`) keeps it a hostile he can see.
    // KH3's friends finish what the party starts.
    if !due {
        if let Some(e) = chase_target(i) {
            out.chase = Some(e.id);
            out.lock = e.id;
            mem.last = Some(AiWord::Chase);
            return out;
        }
    }
    if !i.in_battle || !matches!(i.stance, Stance::Free | Stance::Striking) || due {
        return out;
    }
    // 4. The bodyguard
    let knows = |m: GoofyMove| learn::rank(&i.ranks, m, i.level) > 0;
    let worn = |m: GoofyMove| knows(m) && i.arts.contains(&Some(m));
    let interpose = i.interpose_ready && if K.ai_interpose_unequipped != 0.0 { knows(GoofyMove::Interpose) } else { worn(GoofyMove::Interpose) };
    if let (true, Some(l)) = (interpose && mem.interpose_in <= 0.0, leader(i)) {
        let danger = i.enemies.iter().any(|e| flat(e.pos - l.pos).length() <= K.art_interpose_threat_close) || i.telegraphs.iter().any(|t| i.enemies.iter().any(|e| e.id == t.attacker && flat(e.pos - l.pos).length() <= K.art_interpose_threat_near));
        if l.share() < K.ai_interpose_low && flat(l.pos - i.pos).length() <= K.art_interpose_range && danger {
            out.art = Some(GoofyMove::Interpose);
            mem.interpose_in = K.ai_interpose_every;
            mem.last = Some(AiWord::Interpose);
            return out;
        }
    }
    if mem.taunt_in <= 0.0 && knows(GoofyMove::Taunt) && i.friends.iter().any(|f| !f.ko && f.foes as f32 >= K.ai_taunt_count) {
        out.press(PadButton::LeftShoulder);
        out.press(X);
        mem.taunt_in = K.ai_taunt_every;
        mem.last = Some(AiWord::Taunt);
        return out;
    }
    // what his tree asked of his Arts (equipped, ready, and the Abilities setting allows), else one of his own that pays off now
    let stays = |m: &GoofyMove| !matches!(m, GoofyMove::GoofyCharge | GoofyMove::GoofyTurbo); // those two would carry him off the leader
    let asked = i.art_asked.filter(|m| worn(*m) && stays(m) && art_allowed(i, *m));
    if let Some((m, on)) = asked.map(|m| (m, None)).or_else(|| own_art(i, copy)) {
        out.art = Some(m);
        if let Some(t) = on {
            out.lock = t;
        }
        mem.last = Some(AiWord::Art(m));
        return out;
    }
    // 5. GUARD low: no string; he keeps by the leader until it refills rather than guard into a Guard Break
    let guard_low = !i.can_guard || i.guard < K.ai_guard_low;
    // 6. Last, a basic string on what stands near, the time between strings counted from the end of the last; never in the
    // bash's end lag (THE USER 2026-11-02: "He needs some end lag at the end of his combo so you can't spam it, like Sora.")
    let (poke_every, poke_hits) = copy.map_or((K.ai_poke_every, K.ai_poke_hits), |c| (c.poke_every, c.poke_hits));
    if i.stance == Stance::Striking {
        mem.poke_in = poke_every;
        if (i.combo as f32) < poke_hits && !i.end_lag && !guard_low {
            out.press(PadButton::FaceBottom);
        }
        return out;
    }
    // a blow threw him off the leader, or an enemy drew him: he goes back before anything else of his own
    let far = leader(i).is_some_and(|l| flat(l.pos - i.pos).length() > K.ai_leash);
    let near_foe = i.enemies.iter().any(|e| gap(i.pos, e) <= K.ai_poke_reach);
    let off_post = leader(i).is_some_and(|l| flat(l.pos - i.pos).length() > K.ai_post + K.ai_post_near);
    if far || (guard_low && near_foe && off_post) {
        out.back = mem.move_in <= 0.0;
        if out.back {
            mem.move_in = K.ai_move_every;
        }
        if !far {
            mem.last = Some(AiWord::Recover);
        }
        return out;
    }
    if guard_low || i.style.battle == STYLE_HANDS_OFF || i.end_lag {
        return out;
    }
    // (MB3-AGGRO) A BOSS COPY PRESSES: the nearest enemy out of his reach but within `press_reach` is walked in to (the walk of
    // rule 3, his own run), never waited for
    if let (Some(c), Some(e), Stance::Free) = (copy, mine, i.stance) {
        if gap(i.pos, e) > K.ai_poke_reach && gap(i.pos, e) <= c.press_reach {
            out.chase = Some(e.id);
            out.lock = e.id;
            mem.last = Some(AiWord::Chase);
            return out;
        }
    }
    if i.asked.is_some() {
        mem.poke_in = 0.0;
    }
    // the rebound's opening: a string at once on the enemy his perfect deflect bounced
    if let Some(e) = mem.follow.and_then(|(t, _)| i.enemies.iter().find(|e| e.id == t)).filter(|e| gap(i.pos, e) <= K.ai_poke_reach) {
        out.lock = e.id;
        out.press(PadButton::FaceBottom);
        mem.follow = None;
        mem.poke_in = poke_every;
        mem.last = Some(AiWord::FollowUp);
        return out;
    }
    let focus = (i.style.battle == STYLE_FOCUS).then(|| i.enemies.iter().filter(|e| near_party(e, i) && gap(i.pos, e) <= K.ai_poke_reach).max_by(|a, b| a.stagger.total_cmp(&b.stagger))).flatten();
    let target = i.asked.and_then(|id| i.enemies.iter().find(|e| e.id == id)).filter(|e| near_party(e, i)).or(focus).or(mine);
    if let Some(e) = target.filter(|e| gap(i.pos, e) <= K.ai_poke_reach) {
        if mem.poke_in <= 0.0 {
            out.lock = e.id;
            out.press(PadButton::FaceBottom);
            mem.poke_in = poke_every;
            mem.last = Some(AiWord::Poke);
        }
    }
    out
}

/// Customize's Combat Style (`ETresFNpcAIStyle_Battle`): 2 SUPPORT (Hands Off), 3 FOCUS (Finish the Job).
pub const STYLE_HANDS_OFF: u8 = 2;
pub const STYLE_FOCUS: u8 = 3;
/// Customize's Abilities (`ETresFNpcAIStyle_Ability`): 0 FREE (Go Wild), 1 BALANCE (Watch MP), 2 LESS (Conserve).
pub const ABILITY_WATCH_MP: u8 = 1;
pub const ABILITY_CONSERVE: u8 = 2;

/// The Abilities setting lets him use `m` now: Taunt and Interpose always (his bodyguard moves); the attack Arts not with
/// Conserve, and with Watch MP only while they leave `ai.mp_reserve` of his MP.
fn art_allowed(i: &AiInput, m: GoofyMove) -> bool {
    if matches!(m, GoofyMove::Taunt | GoofyMove::Interpose) {
        return true;
    }
    match i.style.ability {
        ABILITY_CONSERVE => false,
        ABILITY_WATCH_MP => {
            let cost = i.arts.iter().zip(i.art_cost).find(|(a, _)| **a == Some(m)).map_or(0, |(_, c)| c);
            (i.mp - cost) as f32 >= K.ai_mp_reserve * i.mp_max as f32
        }
        _ => true,
    }
}

/// (THE USER 2026-11-02: "Make sure Goofy as an AI party member uses his new tools properly") HIS OWN ARTS WHEN THEY PAY OFF, of
/// the two equipped and ready ([`AiInput::arts`]): Goofy Bash on an enemy in reach whose Stagger is at `ai.bash_stagger` or more
/// (the slam's big Stagger breaks it, and the Breaker follows); Goofy Tornado when `ai.tornado_count` enemies stand in its whirl.
/// Shield Toss leaves him bare-handed (no parry) and the rushes carry him off the leader: never on his own. (Art, its target)
/// (MB3-AGGRO) A boss copy (`copy`): its own thresholds, every enemy his.
fn own_art(i: &AiInput, copy: Option<CopyTune>) -> Option<(GoofyMove, Option<TargetId>)> {
    // (COMBAT-FIX) only an Art he has learned (THE USER 2026-11-07: "Goofy should still use his attacks and abilties as well as
    // deflecting"): an equipped Art of rank 0 was asked for on every step, the kit refused it, and the ask stood in front of
    // every string (the offline duel: a level-1 Goofy with Tornado worn stood idle beside three enemies)
    let ready = |m: GoofyMove| learn::rank(&i.ranks, m, i.level) > 0 && i.arts.contains(&Some(m)) && art_allowed(i, m);
    let (bash_stagger, tornado_count) = copy.map_or((K.ai_bash_stagger, K.ai_tornado_count), |c| (c.bash_stagger, c.tornado_count));
    if ready(GoofyMove::GoofyBash) {
        let ripe = i.enemies.iter().filter(|e| e.broken <= 0.0 && e.stagger >= bash_stagger && gap(i.pos, e) <= K.ai_poke_reach && (copy.is_some() || near_party(e, i))).max_by(|a, b| a.stagger.total_cmp(&b.stagger));
        if let Some(e) = ripe {
            return Some((GoofyMove::GoofyBash, Some(e.id)));
        }
    }
    // (COMBAT-FIX) what the spin reaches: KH3's Attack_Tornado prism (circumradius 130), no longer a 280 sphere
    if ready(GoofyMove::GoofyTornado) && i.enemies.iter().filter(|e| gap(i.pos, e) <= crate::actions::TORNADO_HULL.0).count() as f32 >= tornado_count {
        return Some((GoofyMove::GoofyTornado, None));
    }
    None
}

/// A BROKEN enemy out of his Breaker's reach that he can still reach in time: within `ai.breaker_chase` of him (Finish the Job:
/// `ai.breaker_chase_focus`), its window lasting the walk at his run speed and `ai.breaker_min_left` more. None with Hands Off.
/// The nearest. (THE USER 2026-11-03 follow-up) not held to the leader's leash: the walk is short and bounded by the row, and he
/// goes back after the slam; the leash would cut it to a guard post halfway.
fn chase_target<'a>(i: &AiInput<'a>) -> Option<&'a AiEnemy> {
    if i.style.battle == STYLE_HANDS_OFF || i.stance != Stance::Free {
        return None;
    }
    let range = if i.style.battle == STYLE_FOCUS { K.ai_breaker_chase_focus } else { K.ai_breaker_chase };
    let walk = |e: &AiEnemy| (gap(i.pos, e) - K.breaker_reach).max(0.0) / K.move_run.max(1.0);
    i.enemies.iter().filter(|e| e.broken > 0.0 && gap(i.pos, e) > K.breaker_reach && gap(i.pos, e) <= range && e.broken >= walk(e) + K.ai_breaker_min_left).min_by(|a, b| gap(i.pos, a).total_cmp(&gap(i.pos, b)))
}

/// (FINAL-FIX, FINAL 1a: 7 breaks and no word of why) WHAT HE DOES ABOUT THE BROKEN ENEMY `e` THIS STEP, in words, by the
/// rules of [`think`] in their order (the `goofy-ai:` log line, the tests). His target or not makes no difference: the rules
/// never ask which enemy his tree or his lock has.
pub fn break_verdict(i: &AiInput, e: &AiEnemy, out: &AiOut, mem: &AiMemory) -> &'static str {
    let edge = gap(i.pos, e);
    let range = if i.style.battle == STYLE_FOCUS { K.ai_breaker_chase_focus } else { K.ai_breaker_chase };
    let walk = (edge - K.breaker_reach).max(0.0) / K.move_run.max(1.0);
    if mem.last == Some(AiWord::Breaker) && out.lock == e.id {
        "Goofy Breaker pressed (rule 1)"
    } else if out.chase == Some(e.id) {
        "walks in to it for the Breaker (rule 3)"
    } else if i.stance == Stance::Busy {
        "busy (an Art, a reaction, a team attack, KO): nothing until he is free (the user: accepted)"
    } else if i.breaker && edge <= K.breaker_reach {
        "in his reach: Goofy Breaker after the beat (rule 1)"
    } else if edge <= K.breaker_reach {
        "in his reach, but the Breaker cannot start from this move (the string's end lag, the air, the shield out)"
    } else if i.style.battle == STYLE_HANDS_OFF {
        "Hands Off: no walk to it"
    } else if edge > range {
        "out of his walk range: no Breaker (GOOFY_OVERHAUL.md 12)"
    } else if e.broken < walk + K.ai_breaker_min_left {
        "too little of its window left for the walk"
    } else if i.stance != Stance::Free {
        "not free yet (a string, his guard, the air): the walk waits"
    } else {
        "an attack he has read comes first (rule 2)"
    }
}

/// (FINAL-FIX) The `goofy-ai:` lines of this step: one per BROKEN enemy whose verdict changed; the memory forgets an enemy whose
/// break ended.
fn break_notes(i: &AiInput, out: &AiOut, mem: &mut AiMemory) -> Vec<String> {
    let mut notes = Vec::new();
    mem.noted.retain(|(t, _)| i.enemies.iter().any(|e| e.id == *t && e.broken > 0.0));
    for e in i.enemies.iter().filter(|e| e.broken > 0.0) {
        let verdict = break_verdict(i, e, out, mem);
        if mem.noted.contains(&(e.id, verdict)) {
            continue;
        }
        mem.noted.retain(|(t, _)| *t != e.id);
        mem.noted.push((e.id, verdict));
        let range = if i.style.battle == STYLE_FOCUS { K.ai_breaker_chase_focus } else { K.ai_breaker_chase };
        notes.push(format!(
            "goofy-ai: target {} BROKEN ({:.2} s left) {:.1} m from AI Goofy (edge; his reach {:.1} m, his walk range {:.1} m), stance {:?}, this step's lock {}: {verdict}",
            e.id.0,
            e.broken,
            gap(i.pos, e) / 100.0,
            K.breaker_reach / 100.0,
            range / 100.0,
            i.stance,
            out.lock.0
        ));
    }
    notes
}

// ============================================================================ the party's side

/// The moves of his KH3 tree the overhaul answers.
fn art_of(def: crate::ActionDef) -> Option<GoofyMove> {
    use crate::ActionDef as D;
    match def {
        D::GoofyTornado => Some(GoofyMove::GoofyTornado),
        D::GoofyTurbo => Some(GoofyMove::GoofyTurbo),
        D::GoofyDiscus => Some(GoofyMove::ShieldToss),
        _ => None,
    }
}

fn is_goofy_melee(def: crate::ActionDef) -> bool {
    use crate::ActionDef as D;
    matches!(def, D::GoofyPush | D::GoofySweep | D::GoofyJump)
}

/// The enemies he may answer or choose (what the lock-on accepts, nothing Zettaflare keeps the allies off).
fn enemies_of(party: &crate::party::Party, pos: Vec3, host: &crate::PartyHost) -> Vec<AiEnemy> {
    let reach = K.ai_shot_range + K.ai_leash;
    let gauge = |t: TargetId| party.gof.book.view(t).map_or((0.0, 0.0), |v| (v.share(), v.broken.map_or(0.0, |b| b.0)));
    crate::pad::goofy_shots::enemies(host)
        .into_iter()
        .filter(|t| flat(t.position - pos).length() <= reach && !party.don_zetta_off_limits(t.position, t.radius))
        .map(|t| {
            let (stagger, broken) = gauge(t.id);
            AiEnemy { id: t.id, pos: t.position, radius: t.radius, stagger, broken }
        })
        .collect()
}

/// The party as he sees it: the other members and Sora, the leader marked, and who has enemies on him.
fn friends_of(party: &crate::party::Party, me: MemberId, pos: Vec3, sora: &crate::SoraView, enemies: &[AiEnemy]) -> Vec<AiFriend> {
    let lead = party.members().find(|m| party.member(*m).is_some_and(|p| p.pad.is_some()));
    let mut out = Vec::new();
    for id in party.members().filter(|m| *m != me) {
        let Some(m) = party.member(id) else { continue };
        out.push(AiFriend { member: id, pos: m.position(), hp: m.status.hp, hp_max: m.status.max_hp(), ko: m.status.hp < 1, leader: lead == Some(id), foes: 0 });
    }
    if lead.is_none() {
        out.push(AiFriend { member: SORA_MEMBER, pos: sora.position, hp: sora.hp, hp_max: sora.hp_max, ko: sora.dead, leader: true, foes: 0 });
    } else if let Some((_, at)) = sora.ai_sora {
        out.push(AiFriend { member: SORA_MEMBER, pos: at, hp: 1, hp_max: 1, ko: false, leader: false, foes: 0 });
    }
    count_foes(&mut out, pos, enemies, &party.gof.aims);
    out
}

/// (GF-POLISH) WHO HAS ENEMIES ON HIM. Where the host read an enemy's aim (`Party::set_enemy_aims`, Elden Ring) that is the answer: at
/// a friend, at him, or at nobody of the party. Otherwise, and always in the test level, the estimate: an enemy is on the friend
/// it stands nearest to, as long as that is not him.
fn count_foes(friends: &mut [AiFriend], pos: Vec3, enemies: &[AiEnemy], aims: &[EnemyAim]) {
    for e in enemies {
        if let Some(aim) = aims.iter().find(|a| a.enemy == e.id) {
            if let Some(f) = friends.iter_mut().find(|f| !f.ko && Some(f.member) == aim.on) {
                f.foes += 1;
            }
            continue;
        }
        let mine = gap(pos, e);
        let near = friends.iter_mut().filter(|f| !f.ko).min_by(|a, b| gap(a.pos, e).total_cmp(&gap(b.pos, e)));
        if let Some(f) = near.filter(|f| gap(f.pos, e) <= K.ai_taunt_near && gap(f.pos, e) < mine) {
            f.foes += 1;
        }
    }
}

/// THE LEASH: in battle a move of his tree toward something beyond `ai.leash` from the leader is cut to the guard post, a point
/// near the leader on that thing's side. His following (the leader himself) and his formation (spots beside the leader) pass.
fn leash(party: &mut crate::party::Party, id: MemberId, lead: Vec3, host: &crate::PartyHost) {
    let Some(m) = party.member_mut(id) else { return };
    let Some(MemberCommand::MoveTo { goal, .. }) = m.command_running() else { return };
    let beyond = match goal {
        MoveGoal::Target(t) => host.targets.info(t).filter(|i| flat(i.position - lead).length() - i.radius > K.ai_leash).map(|i| i.position),
        MoveGoal::Point(p) => Some(p).filter(|p| flat(*p - lead).length() > K.ai_leash),
        _ => None,
    };
    // (a move that brings him nearer the leader is never cut: he is on his way back)
    let Some(far) = beyond.filter(|p| flat(*p - lead).length() > flat(m.position() - lead).length()) else { return };
    let post = lead + flat(far - lead).normalize_or_zero() * K.ai_post;
    if flat(post - m.position()).length() <= K.ai_post_near {
        m.answer_move(true);
    } else {
        m.set_move_goal(MoveGoal::Point(post));
    }
}

/// (GF-AI) THE PARTY PASS, beside Donald's in `Party::step`: every AI Goofy's tree requests, leash, decision and buttons.
pub fn party_pass(party: &mut crate::party::Party, sora: &crate::SoraView, host: &mut crate::PartyHost, dt: f32) {
    if party.gof.kh3_ai {
        return;
    }
    let dt = dt.max(0.0);
    let ids: Vec<MemberId> = party.members().filter(|id| party.member(*id).is_some_and(|m| m.chara == Chara::Goofy && m.pad.is_none() && m.status.goofy.is_some())).collect();
    let telegraphs = party.telegraphs().to_vec();
    for id in ids {
        let mut mem = match party.gof.ai.iter().position(|a| a.member == id) {
            Some(n) => party.gof.ai.swap_remove(n),
            None => AiMemory::new(id),
        };
        let Some(m) = party.member(id) else { continue };
        let pos = m.position();
        let enemies = enemies_of(party, pos, host);
        let friends = friends_of(party, id, pos, sora, &enemies);
        // the tree's attack requests
        let (mut asked, mut art_asked) = (None, None);
        let mut answer = None;
        if let Some(MemberCommand::Action { def, target }) = m.don_pending() {
            if is_goofy_melee(def) {
                asked = match target {
                    crate::TargetRef::Enemy(t) => Some(t),
                    _ => enemies.first().map(|e| e.id),
                };
                answer = Some(true);
            } else if let Some(art) = art_of(def) {
                art_asked = Some(art);
                answer = Some(crate::pad::goofy_arts::art_moves(&m.status).contains(&Some(art)));
            }
        }
        // (THE USER 2026-11-02) an Art cooling down is not asked for; nor one MP cannot pay ("let Goofy use his last MP and go into
        // MP reserve": any MP above 0 pays, none in MP Charge), nor one that would take his last MP while a friend is low enough
        // for Interpose (that MP is the bodyguard's)
        let arts = crate::pad::goofy_arts::ai_art_moves(&m.status, friends.iter().any(|f| !f.ko && f.share() < K.ai_interpose_low));
        let input = AiInput {
            pos,
            level: m.status.level,
            ranks: m.status.growth.ranks,
            resolve: m.status.goofy.as_ref().map_or_else(Resolve::default, |s| s.core.resolve),
            stance: crate::pad::goofy::ai_stance(m),
            can_x: crate::pad::goofy::ai_can_x(m),
            can_guard: m.status.guard.is_some_and(|g| g.can_guard()),
            guard: m.status.guard.map_or(0.0, |g| g.value / g.max.max(1.0)),
            hop_cd: m.status.goofy.as_ref().map_or(0.0, |s| s.core.hop_cd),
            in_battle: m.in_battle,
            breaker: crate::pad::goofy_breaker::breaker_offered(m),
            combo: m.status.goofy.as_ref().map_or(0, |s| s.run.combo.n),
            end_lag: crate::pad::goofy_combo::restart_held(m),
            arts,
            art_cost: arts.map(|a| a.map_or(0, |mv| crate::pad::goofy_arts::move_mp(&m.status, mv))),
            interpose_ready: m.status.gof_rank(GoofyMove::Interpose) > 0 && crate::pad::goofy_arts::move_cooldown_left(&m.status, GoofyMove::Interpose) <= 0.0,
            mp: m.status.mp,
            mp_max: m.status.max_mp(),
            style: m.status.ai,
            // (THE USER 2026-11-02, the bounce) the rebound his perfect deflect asked last step
            rebound: party.gof.events.iter().find_map(|(who, e)| match e {
                super::GoofyEvent::Bounce { target } if *who == id => Some(*target),
                _ => None,
            }),
            asked,
            art_asked,
            enemies: &enemies,
            friends: &friends,
            telegraphs: &telegraphs,
        };
        let mut rng = m.rng;
        let out = think(&input, &mut mem, dt, &mut || rng.unit());
        let notes = break_notes(&input, &out, &mut mem); // (FINAL-FIX) what he does about each BROKEN enemy, for the log
        let lead = friends.iter().find(|f| f.leader && !f.ko).map(|f| f.pos);
        let zetta = party.don_zetta_span(); // Zettaflare walks the allies itself
        let Some(m) = party.member_mut(id) else { continue };
        m.rng = rng;
        if let Some(ok) = answer {
            m.don_take_pending(ok);
        }
        // (THE USER 2026-11-02: "Make sure Goofy as an AI party member uses his new tools properly") in to the BROKEN enemy, to stand
        // in his Breaker's reach of it (a run of his tree that goes elsewhere gives way, and his tree's next asks are refused
        // while the walk lasts: `pad::goofy::ai_chase_refuses`); a walk whose break is over (or taken) ends
        let chase = out.chase.filter(|_| !zetta);
        let was = m.status.goofy.as_mut().and_then(|s| std::mem::replace(&mut s.run.ai_chase, chase));
        if let Some(old) = was.filter(|old| Some(*old) != chase) {
            if matches!(m.command_running(), Some(MemberCommand::MoveTo { goal: MoveGoal::Target(g), .. }) if g == old) {
                m.abort_command_pub();
            }
        }
        crate::pad::goofy::ai_drive(m, out.drive());
        if out.back && !zetta && m.command_running().is_none() {
            // (`MoveGoal::Sora` is the leader: a friend who leads stands in `SoraView`)
            m.command(MemberCommand::MoveTo { goal: MoveGoal::Sora, locomotion: crate::LocomotionDef::Move1, walk: false, acceptable_radius: K.ai_post, follow_path: true });
        }
        if let Some(t) = chase {
            m.in_battle = true; // (his walk at his own run speed, not KH3's follow speed of a standing leader)
            let going = matches!(m.command_running(), Some(MemberCommand::MoveTo { goal: MoveGoal::Target(g), .. }) if g == t);
            if !going && matches!(m.command_running(), Some(MemberCommand::MoveTo { .. })) {
                // a walk of his tree already runs: it is turned to the enemy (an abort and a new MoveTo in the same step left his
                // locomotion standing in its stop)
                m.set_move_goal(MoveGoal::Target(t));
            } else if !going {
                if m.command_running().is_some() {
                    m.abort_command_pub();
                }
                let radius = enemies.iter().find(|e| e.id == t).map_or(0.0, |e| e.radius);
                m.command(MemberCommand::MoveTo { goal: MoveGoal::Target(t), locomotion: crate::LocomotionDef::Move1, walk: false, acceptable_radius: radius + K.breaker_reach * 0.5, follow_path: true });
            }
        }
        party.gof.ai.push(mem);
        for note in notes {
            party.gof.note(note); // (FINAL-FIX) said by the host (kh-partyhost runtime.rs)
        }
        // (the walk to a Breaker is not cut by the leash: `chase_target`)
        if let (Some(lead), true, false, None) = (lead, party.member(id).is_some_and(|m| m.in_battle), zetta, chase) {
            leash(party, id, lead, host);
        }
    }
}

/// DEBUG (`party-word x3g ai`): what he is doing and thinking.
pub fn word(party: &crate::party::Party) -> String {
    let Some(id) = party.find(Chara::Goofy) else { return "goofy: not in the party".to_string() };
    let Some(m) = party.member(id) else { return "goofy: not in the party".to_string() };
    let Some(a) = party.gof.ai.iter().find(|a| a.member == id) else { return "goofy: he leads, or KH3's AI runs him".to_string() };
    let seen: Vec<_> = a.seen.iter().map(|s| (s.attacker.0, s.class, s.answer, s.good, s.read_left.max(0.0), s.taps)).collect();
    format!("goofy ai: at {:.0} state {:?} {:?} telegraphs {} seen {seen:?} last {:?} rolled {}/{} next string {:.1} s", m.position(), m.state, crate::pad::goofy::ai_stance(m), party.telegraphs().len(), a.last, a.rolled_good, a.rolled, a.poke_in)
}

#[cfg(test)]
#[path = "ai_tests.rs"]
mod tests;
