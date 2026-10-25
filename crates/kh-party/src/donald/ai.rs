//! (DON-PROG) AI DONALD (documentation/DONALD_OVERHAUL.md 11): what the AI Donald casts, attunes to and where he
//! stands. [`decide`] is the pure brain; [`party_pass`] (run first in `Party::step`) feeds it and carries it out through
//! DON-MAGIC's `cast::begin` and the attune cast, and keeps the party's DON-PROG state ([`DonProgState`]: the auras,
//! Out of Sight's clocks, Taunt Pass, the AI's memory).
//!
//! HOW HE IS DRIVEN: his KH3 behaviour trees stay (follow, battle positioning, the MetaAI's Cure choice: KH3's healing
//! rules, kh-ai cure.rs). Their requests for his five KH3 spells (DONALD_FIRE / BLIZZARD / THUNDER / AERO / CURE) and his
//! KH3 melee (Beat / Swing / Jump, removed by doc 13) are REPLACED here by the overhaul's choice before the pawn takes
//! them: [`AiAsk::Cure`] (the MetaAI chose to heal someone) or [`AiAsk::Attack`] (the tree wants to attack its target).
//! Besides, every `ai.move_every` s he steps back when an enemy comes within `ai.retreat` (doc: "keeps 8-20 m from
//! enemies, and retreats if one gets close").
//!
//! THE RULES (doc 11, in priority order):
//! 1. Heals: the Cure line by KH3's healing rules (the MetaAI picks whom and when); the tier by the share missing.
//! 2. Aero Barrier, KH1's way: on himself when an enemy closes in, on the leader / a friend who is being attacked or is
//!    low on HP (below `ai.ally_low`), renewed with `ai.barrier_renew` s left during a fight. One barrier per member.
//! 3. Ball Lightning: on himself when cornered (`ai.cornered_count` enemies within `ai.cornered`), else on the friend
//!    fighting up close (Goofy, the leader in melee) who has no orbs.
//! 4. Attunement by situation: Light when the party is low (below `ai.party_low`), Water when Sora uses Thunder (Conduct),
//!    else the target's weakness, else what he has. NEVER A DELIBERATE RELEASE: he never switches while the element he
//!    would let go is at Resonance 3 (`DonCore::would_release`).
//! 5. The ultimate at full power, once per attunement period (the core's rule) when `ai.ultimate` is on.
//! 6. Reactions when easy: Thunder on a Wet target (Conduct), Aero / Thunder on a Frozen one (Shatter), Fire on a
//!    Chilled one (Steam Burst); never Fire on a Wet one (Fizzle).
//! 7. Otherwise the attuned element's spell (the menu's pick: -za at full power), MP allowing; below `ai.mp_keep` of
//!    his MP only heals and protection.
//! The aggro rules (5.4) are the same for him (aggro.rs; the hosts apply them).

use glam::Vec3;

use super::ability_info::prog;
use super::aggro::Stillness;
use super::aura::PartyAura;
use super::cast::{CastTarget, SORA_MEMBER};
use super::combat::{CastPlan, UltMark};
use super::element::{ElemSet, Element};
use super::spells::SpellRef;
use super::DonaldState;
use crate::ids::MemberId;
use kh_core::host::TargetId;

/// An enemy as the AI sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiEnemy {
    pub id: u32,
    pub pos: Vec3,
    /// Its weak and resisted elements (resist < 0 / > 0 by `ETresDamageAttribute`).
    pub weak: ElemSet,
    pub resist: ElemSet,
    /// Its statuses (crate::donald::status).
    pub wet: bool,
    pub chilled: bool,
    pub frozen: bool,
    pub burned: bool,
    /// A boss / large enemy.
    pub big: bool,
}

/// A party member (not Donald) as the AI sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiAlly {
    /// As a cast target ([`SORA_MEMBER`] for Sora).
    pub member: MemberId,
    pub pos: Vec3,
    pub hp: i32,
    pub hp_max: i32,
    pub ko: bool,
    /// Fighting up close (Goofy, the leader).
    pub melee: bool,
    /// Being attacked now (hurt within the last seconds).
    pub attacked: bool,
    /// Seconds left on his Aero Barrier (0 = none), his Ball Lightning orbs.
    pub barrier_left: f32,
    pub orbs: u8,
    /// He used Thunder within `ai.thunder_memory` s (Sora's Thunder: Water attunement for Conduct).
    pub thunder: bool,
}

/// What the tree asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AiAsk {
    /// KH3's MetaAI: heal this member ([`SORA_MEMBER`] = Sora; Donald's own id = himself).
    Cure(MemberId),
    /// Attack this enemy (or the nearest when None).
    Attack(Option<u32>),
}

/// Everything [`decide`] reads.
#[derive(Clone, Copy, Debug)]
pub struct AiInput<'a> {
    pub me: &'a DonaldState,
    pub id: MemberId,
    pub pos: Vec3,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub mp_charging: bool,
    pub in_battle: bool,
    pub barrier_left: f32,
    pub orbs: u8,
    pub enemies: &'a [AiEnemy],
    pub allies: &'a [AiAlly],
    /// Spells a cast of which was refused in the last [`REFUSED_MEMORY`] s (not chosen again meanwhile: the next rule
    /// gets its turn).
    pub refused: &'a [(SpellRef, CastTarget)],
}

/// Seconds a refused spell is left out of his choices.
pub const REFUSED_MEMORY: f32 = 3.0;

/// What AI Donald does now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AiChoice {
    /// Attune to this element (the attune cast).
    Attune(Element),
    /// Cast this plan at this target (`cast::begin`).
    Cast { plan: CastPlan, target: CastTarget },
    /// Move to keep his range (8-20 m): away from a close enemy.
    Move { to: Vec3, retreat: bool },
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, 0.0)
}

impl AiInput<'_> {
    /// The cast of `s` at `target` when he can pay it and it was not refused lately.
    fn cast(&self, s: SpellRef, target: CastTarget) -> Option<AiChoice> {
        if self.refused.contains(&(s, target)) {
            return None;
        }
        let plan = self.plan(s)?;
        // (DON-SPELLB cure reserve) an attack keeps a Cura's price while someone is low (see DON-SPELLB tools/apply_onefx.js)
        if !matches!(s, SpellRef::Line1(Element::Light, _)) && !s.ally_target() && self.me.learned.tier(Element::Light) > 0 {
            let low = prog().value("ai.ally_low");
            let someone_low = (self.hp as f32) < low * self.hp_max.max(1) as f32 || self.allies.iter().any(|a| !a.ko && (a.hp as f32) < low * a.hp_max.max(1) as f32);
            if someone_low {
                let tier = self.me.learned.tier(Element::Light).min(2);
                let reserve = self.plan(SpellRef::Line1(Element::Light, tier)).map_or(0, |c| c.mp);
                if self.mp - plan.mp < reserve {
                    return None;
                }
            }
        }
        Some(AiChoice::Cast { plan, target })
    }

    fn plan(&self, s: SpellRef) -> Option<CastPlan> {
        let d = super::data();
        let dual = self.me.abilities.mods().dual;
        self.me.core.check_cast(s, &self.me.learned, self.mp, self.mp_max, dual, &d).ok()
    }

    fn enemy(&self, id: u32) -> Option<&AiEnemy> {
        self.enemies.iter().find(|e| e.id == id)
    }

    fn nearest_enemy(&self) -> Option<(&AiEnemy, f32)> {
        self.enemies.iter().map(|e| (e, flat(e.pos - self.pos).length())).min_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn enemies_within(&self, r: f32) -> usize {
        self.enemies.iter().filter(|e| flat(e.pos - self.pos).length() <= r).count()
    }

    /// The party's HP share (him and every living member).
    fn party_hp(&self) -> f32 {
        let (mut hp, mut max) = (self.hp.max(0) as f32, self.hp_max.max(1) as f32);
        for a in self.allies.iter().filter(|a| !a.ko) {
            hp += a.hp.max(0) as f32;
            max += a.hp_max.max(1) as f32;
        }
        hp / max
    }

    fn primary(&self) -> Option<Element> {
        self.me.core.attune.primary()
    }

    fn knows_any(&self, e: Element) -> bool {
        self.me.learned.attune_unlocked(e)
    }
}

/// The Cure-line tier for a member missing `share` of his HP: the highest tier he knows up to the need (Curaza at
/// full-power Light only, the core's rule), falling back to lower tiers he can pay.
fn cure_pick(input: &AiInput, share: f32, target: CastTarget) -> Option<AiChoice> {
    let want = if share >= 0.7 { 4 } else if share >= 0.45 { 3 } else if share >= 0.25 { 2 } else { 1 };
    let top = input.me.learned.tier(Element::Light).min(want);
    // the highest tier he can PAY; a refused cast of it (he was busy) waits for the next request rather than dropping to a
    // weaker tier (a lower tier only for the MP)
    let spell = (1..=top).rev().map(|t| SpellRef::Line1(Element::Light, t)).find(|s| input.plan(*s).is_some())?;
    input.cast(spell, target)
}

/// The element he wants attuned now (rule 4), None = keep what he has.
fn wanted_element(input: &AiInput, target: Option<&AiEnemy>) -> Option<Element> {
    let p = prog();
    if input.party_hp() < p.value("ai.party_low") && input.knows_any(Element::Light) {
        return Some(Element::Light);
    }
    if input.allies.iter().any(|a| a.thunder) && input.knows_any(Element::Water) {
        return Some(Element::Water);
    }
    if let Some(t) = target {
        if let Some(e) = t.weak.iter().find(|e| input.knows_any(*e)) {
            return Some(e);
        }
        // attuned to what it resists: change to something it does not
        if input.primary().is_some_and(|e| t.resist.has(e)) {
            return Element::ALL.into_iter().filter(|e| *e != Element::Light).find(|e| input.knows_any(*e) && !t.resist.has(*e));
        }
    }
    if input.primary().is_none() {
        // nothing attuned: the element he knows best (the highest line-1 tier), Light last
        return Element::ALL.into_iter().filter(|e| *e != Element::Light && input.knows_any(*e)).max_by_key(|e| (input.me.learned.tier(*e), std::cmp::Reverse(e.index()))).or(input.knows_any(Element::Light).then_some(Element::Light));
    }
    None
}

/// Attuning to `e` is allowed now (never a Release, not mid-cast, unlocked, not already attuned).
fn may_attune(input: &AiInput, e: Element) -> bool {
    let d = super::data();
    let m = input.me.abilities.mods();
    input.me.core.attune.cast.is_none() && !input.me.core.attuned(e) && input.knows_any(e) && input.me.core.would_release(e, &m, &d).is_none()
}

/// The protection casts (rules 2 / 3).
fn protect(input: &AiInput) -> Option<AiChoice> {
    let p = prog();
    let renew = p.value("ai.barrier_renew");
    let barrier = input.me.learned.line2_spell(Element::Aero);
    let orbs = input.me.learned.line2_spell(Element::Thunder);
    let cornered = input.enemies_within(p.value("ai.cornered")) as f32 >= p.value("ai.cornered_count").max(1.0);
    let closed_in = input.enemies_within(p.value("ai.cornered")) > 0;
    // Ball Lightning on himself when cornered
    if let Some(s) = orbs {
        if cornered && input.orbs == 0 && input.barrier_left <= 0.0 { // (DON-SPELLB one worn) never over his barrier
            if let Some(c) = input.cast(s, CastTarget::Member(input.id)) {
                return Some(c);
            }
        }
    }
    if let Some(s) = barrier {
        // himself, an enemy closing in
        if closed_in && input.barrier_left <= renew && input.orbs == 0 { // (DON-SPELLB one worn) never over his balls
            if let Some(c) = input.cast(s, CastTarget::Member(input.id)) {
                return Some(c);
            }
        }
        // a friend attacked or low, then any barrier about to end in a fight
        let low = p.value("ai.ally_low");
        let need = input.allies.iter().filter(|a| !a.ko && a.barrier_left <= renew && a.orbs == 0 /* (DON-SPELLB one worn) */).filter(|a| a.attacked || (a.hp as f32) < low * a.hp_max.max(1) as f32).min_by(|a, b| (a.hp as f32 / a.hp_max.max(1) as f32).total_cmp(&(b.hp as f32 / b.hp_max.max(1) as f32)));
        let renewal = || input.allies.iter().filter(|a| !a.ko && a.barrier_left > 0.0 && a.barrier_left <= renew).min_by(|a, b| a.barrier_left.total_cmp(&b.barrier_left));
        let who = need.or_else(|| if input.in_battle { renewal() } else { None });
        if let Some(a) = who {
            if let Some(c) = input.cast(s, CastTarget::Member(a.member)) {
                return Some(c);
            }
        }
        if input.in_battle && input.barrier_left > 0.0 && input.barrier_left <= renew && input.orbs == 0 { // (DON-SPELLB one worn)
            if let Some(c) = input.cast(s, CastTarget::Member(input.id)) {
                return Some(c);
            }
        }
    }
    // Ball Lightning on the friend fighting up close
    if let Some(s) = orbs {
        if input.in_battle {
            if let Some(a) = input.allies.iter().find(|a| !a.ko && a.melee && a.orbs == 0 && a.barrier_left <= 0.0 && input.enemies.iter().any(|e| flat(e.pos - a.pos).length() <= 400.0)) {
                if let Some(c) = input.cast(s, CastTarget::Member(a.member)) {
                    return Some(c);
                }
            }
        }
    }
    None
}

/// The attack cast on `t` (rules 5 - 7).
fn attack(input: &AiInput, t: &AiEnemy) -> Option<AiChoice> {
    let p = prog();
    let d = super::data();
    let on = CastTarget::Enemy(TargetId(t.id));
    let cast = |s: SpellRef| input.cast(s, on);
    let low_mp = (input.mp as f32) < p.value("ai.mp_keep") * input.mp_max.max(1) as f32;
    if low_mp {
        return None;
    }
    // 5. the ultimate at full power, once per period
    if p.value("ai.ultimate") != 0.0 {
        for e in input.me.core.elements().iter() {
            if input.me.core.ultimate_mark(e, &input.me.learned, &d) == UltMark::Ready && !(e == Element::Fire && t.wet) {
                if let Some(c) = cast(SpellRef::Ultimate(e)) {
                    return Some(c);
                }
            }
        }
    }
    // 6. a Reaction when it is easy
    let best_line1 = |e: Element| input.me.core.line1_pick(e, &input.me.learned, &d);
    if t.frozen {
        for e in [Element::Aero, Element::Thunder] {
            if let Some(c) = best_line1(e).and_then(cast) {
                return Some(c);
            }
        }
    }
    if t.wet {
        if let Some(c) = best_line1(Element::Thunder).and_then(cast) {
            return Some(c);
        }
    }
    if (t.chilled || t.frozen) && !t.wet {
        if let Some(c) = best_line1(Element::Fire).and_then(cast) {
            return Some(c);
        }
    }
    // 7. the attuned element's spell (a Fire spell never on a Wet target)
    for e in input.me.core.elements().iter().filter(|e| *e != Element::Light) {
        if e == Element::Fire && t.wet {
            continue;
        }
        if let Some(c) = best_line1(e).and_then(cast) {
            return Some(c);
        }
    }
    // a Light attunement: Banish (its attack line)
    if input.me.core.attuned(Element::Light) {
        if let Some(c) = input.me.learned.line2_spell(Element::Light).and_then(cast) {
            return Some(c);
        }
    }
    // not attuned (or nothing castable there): the cheapest base spell that is no Fizzle and not resisted
    Element::ALL.into_iter().filter(|e| *e != Element::Light && !(t.wet && *e == Element::Fire) && !t.resist.has(*e)).filter_map(|e| (input.me.learned.tier(e) >= 1).then(|| SpellRef::Line1(e, 1))).find_map(cast)
}

/// THE DECISION for what the tree asked (`ask`), None = nothing fits (the request fails: no KH3 spell is cast instead).
pub fn decide(input: &AiInput, ask: AiAsk) -> Option<AiChoice> {
    if input.me.core.attune.cast.is_some() {
        return None;
    }
    // 1. KH3's healing rules chose to heal someone: the Cure line
    if let AiAsk::Cure(who) = ask {
        let (hp, max) = if who == input.id { (input.hp, input.hp_max) } else { input.allies.iter().find(|a| a.member == who).map_or((input.hp, input.hp_max), |a| (a.hp, a.hp_max)) };
        let share = 1.0 - hp.max(0) as f32 / max.max(1) as f32;
        let target = CastTarget::Member(who);
        // the party low: Light first (rule 4: its cheaper Cure line, its Regen aura), when that is no Release
        if input.party_hp() < prog().value("ai.party_low") && may_attune(input, Element::Light) {
            return Some(AiChoice::Attune(Element::Light));
        }
        return cure_pick(input, share, target);
    }
    // 2 / 3. protection
    if input.in_battle {
        if let Some(c) = protect(input) {
            return Some(c);
        }
    }
    let target = match ask {
        AiAsk::Attack(Some(id)) => input.enemy(id).or_else(|| input.nearest_enemy().map(|e| e.0)),
        _ => input.nearest_enemy().map(|e| e.0),
    };
    // 4. attunement
    if let Some(e) = wanted_element(input, target) {
        if may_attune(input, e) {
            return Some(AiChoice::Attune(e));
        }
    }
    let t = target?;
    attack(input, t)
}

/// THE RANGE RULE (doc 11: "keeps 8-20 m from enemies, and retreats if one gets close"): a point `ai.retreat_to` away from
/// the nearest enemy when one is within `ai.retreat`. None otherwise (the tree's own positioning stands).
pub fn range_move(input: &AiInput) -> Option<AiChoice> {
    let p = prog();
    let (e, dist) = input.nearest_enemy()?;
    if dist > p.value("ai.retreat") {
        return None;
    }
    let away = flat(input.pos - e.pos).normalize_or(Vec3::X);
    Some(AiChoice::Move { to: e.pos + away * p.value("ai.retreat_to"), retreat: true })
}

// ============================================================================ the party's DON-PROG state and its pass

/// One Donald's AI memory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AiMemory {
    pub member: MemberId,
    /// Seconds until the next range move may be made.
    pub move_in: f32,
    /// What he chose last and whether it started (the log / the harness).
    pub last: Option<(AiChoice, bool)>,
    /// How many choices he made / that started.
    pub chosen: u32,
    pub started: u32,
}

/// THE PARTY'S DON-PROG STATE (`Party::don_prog`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonProgState {
    pub aura: PartyAura,
    /// Out of Sight's clocks per Donald.
    pub still: Vec<(MemberId, Stillness)>,
    pub ai: Vec<AiMemory>,
    /// The party's clock (s).
    pub clock: f32,
    /// The last Thunder hit of Sora (the clock), fed by the host (`note_sora_thunder`).
    pub sora_thunder: Option<f32>,
    /// TAUNT PASS: the member who blocked / guarded last ([`SORA_MEMBER`] = Sora) and until when the enemies near Donald
    /// take him.
    pub taunt: Option<(MemberId, f32)>,
    /// HP of every member last step (the "being attacked" rule): (member, hp, clock of its last drop).
    pub hp_seen: Vec<(MemberId, i32, f32)>,
    /// The aggro context of each Donald this step (the hosts' target rules read it).
    pub aggro: Vec<(MemberId, super::aggro::AggroCtx)>,
    /// The members present this step (Sora included when he is in the party).
    pub party_size: usize,
    /// (member, spell, clock) of the casts refused lately ([`REFUSED_MEMORY`]).
    pub refused: Vec<(MemberId, (SpellRef, CastTarget), f32)>,
    /// KH3's AI Donald instead of the overhaul's (his five KH3 spells and melee as the trees ask): only for the tests of
    /// that machinery (kh-party tests/party.rs); off in play.
    pub kh3_ai: bool,
}

impl DonProgState {
    pub fn stillness(&self, m: MemberId) -> Stillness {
        self.still.iter().find(|s| s.0 == m).map_or_else(Stillness::new, |s| s.1)
    }

    pub fn aggro_of(&self, m: MemberId) -> super::aggro::AggroCtx {
        self.aggro.iter().find(|a| a.0 == m).map_or(super::aggro::AggroCtx::ALONE, |a| a.1)
    }

    /// Taunt Pass's ally now, when one holds.
    pub fn taunt_now(&self) -> Option<MemberId> {
        self.taunt.filter(|t| self.clock <= t.1).map(|t| t.0)
    }

    fn attacked(&self, m: MemberId) -> bool {
        self.hp_seen.iter().find(|h| h.0 == m).is_some_and(|h| self.clock - h.2 <= 2.0)
    }

    fn see_hp(&mut self, m: MemberId, hp: i32) {
        let clock = self.clock;
        match self.hp_seen.iter_mut().find(|h| h.0 == m) {
            Some(h) => {
                if hp < h.1 {
                    h.2 = clock;
                }
                h.1 = hp;
            }
            None => self.hp_seen.push((m, hp, -1.0e6)),
        }
    }
}

/// The host: Sora's Thunder hit landed (Water attunement, rule 4).
pub fn note_sora_thunder(party: &mut crate::party::Party) {
    party.don_prog.sora_thunder = Some(party.don_prog.clock);
}

/// The host: an ally blocked / guarded (Sora's guard, a friend's GUARD): Taunt Pass's ally for `aggro.taunt_pass_hold` s.
pub fn note_guard(party: &mut crate::party::Party, who: MemberId) {
    let until = party.don_prog.clock + prog().value("aggro.taunt_pass_hold");
    party.don_prog.taunt = Some((who, until));
}

fn is_donald_ai_def(def: crate::ActionDef) -> bool {
    use crate::ActionDef as D;
    matches!(def, D::DonaldBeat | D::DonaldSwing | D::DonaldJump | D::DonaldFire | D::DonaldBlizzard | D::DonaldThunder | D::DonaldAero | D::DonaldCure)
}

/// (DON-PROG) THE PARTY PASS, first in `Party::step` after the foundation's clocks: the auras (Water / Light on the
/// friends), Out of Sight's clocks, Taunt Pass from Goofy's GUARD, the aggro contexts, and AI Donald's choices for the
/// requests his tree made since the last step (replacing his KH3 spells and melee).
pub fn party_pass(party: &mut crate::party::Party, sora: &crate::SoraView, host: &mut crate::PartyHost, dt: f32) {
    use crate::NpcStateId as S;
    let dt = dt.max(0.0);
    party.don_prog.clock += dt;
    let clock = party.don_prog.clock;
    let ids: Vec<MemberId> = party.members().collect();
    let friend_leads = ids.iter().any(|id| party.member(*id).is_some_and(|m| m.pad.is_some()));
    let sora_in = !friend_leads || sora.ai_sora.is_some();
    let party_size = ids.len() + sora_in as usize;
    party.don_prog.party_size = party_size;
    // ---- the auras
    let donalds: Vec<(MemberId, Vec3, DonaldState)> = ids.iter().filter_map(|id| party.member(*id).and_then(|m| m.status.donald.clone().map(|d| (*id, m.position(), d)))).collect();
    {
        let refs: Vec<(MemberId, Vec3, &DonaldState)> = donalds.iter().map(|(i, p, d)| (*i, *p, d)).collect();
        let mut aura = std::mem::take(&mut party.don_prog.aura);
        aura.make(&refs, party_size);
        let mut members: Vec<(MemberId, Vec3, &mut crate::status::FriendStatus)> = Vec::new();
        // (one borrow per member: the statuses are disjoint)
        let positions: Vec<(MemberId, Vec3)> = ids.iter().filter_map(|id| party.member(*id).map(|m| (*id, m.position()))).collect();
        let mut statuses: Vec<(MemberId, crate::status::FriendStatus)> = ids.iter().filter_map(|id| party.status(*id).cloned().map(|s| (*id, s))).collect();
        for (id, s) in statuses.iter_mut() {
            let pos = positions.iter().find(|p| p.0 == *id).map_or(Vec3::ZERO, |p| p.1);
            members.push((*id, pos, s));
        }
        aura.apply_friends(members.into_iter(), dt);
        for (id, s) in statuses {
            if let Some(st) = party.status_mut(id) {
                (st.mp_charge_left, st.mp, st.hp) = (s.mp_charge_left, s.mp, s.hp);
            }
        }
        party.don_prog.aura = aura;
    }
    // ---- HP seen (being attacked), Out of Sight's clocks, Taunt Pass from a friend's GUARD
    for id in &ids {
        let Some(m) = party.member(*id) else { continue };
        let (hp, speed, guarding) = (m.status.hp, flat(m.cmc.velocity).length(), m.chara == crate::Chara::Goofy && matches!(m.state, S::Pad(crate::pad::goofy::GUARD | crate::pad::goofy::GUARD_HIT | crate::pad::goofy::AIR_GUARD)));
        let casting = m.status.donald.is_some() && super::cast::casting(m);
        party.don_prog.see_hp(*id, hp);
        if guarding {
            note_guard(party, *id);
        }
        if party.member(*id).is_some_and(|m| m.status.donald.is_some()) {
            let mut st = party.don_prog.stillness(*id);
            let began = casting && !st.casting_now;
            st.tick(speed, began, dt);
            st.casting_now = casting;
            match party.don_prog.still.iter_mut().find(|s| s.0 == *id) {
                Some(s) => s.1 = st,
                None => party.don_prog.still.push((*id, st)),
            }
        }
    }
    party.don_prog.see_hp(SORA_MEMBER, sora.hp);
    // ---- the aggro contexts
    let mut aggro = Vec::new();
    for (id, pos, _) in &donalds {
        let mut nearest = f32::INFINITY;
        for other in ids.iter().filter(|o| *o != id) {
            if let Some(m) = party.member(*other) {
                nearest = nearest.min(m.position().distance(*pos));
            }
        }
        if sora_in {
            let sp = sora.ai_sora.map_or(sora.position, |a| a.1);
            if !friend_leads || sora.ai_sora.is_some() {
                nearest = nearest.min(sp.distance(*pos));
            }
        }
        let st = party.don_prog.stillness(*id);
        aggro.push((*id, super::aggro::AggroCtx { others: party_size.saturating_sub(1), nearest_ally: nearest, still: st.still(), casting: st.casting() }));
    }
    party.don_prog.aggro = aggro;
    // ---- AI Donald
    for (id, pos, state) in &donalds {
        let Some(m) = party.member(*id) else { continue };
        if m.pad.is_some() || party.don_prog.kh3_ai {
            continue;
        }
        let pending = m.don_pending();
        let melee = matches!(pending, Some(crate::MemberCommand::Action { def: crate::ActionDef::DonaldBeat | crate::ActionDef::DonaldSwing | crate::ActionDef::DonaldJump, .. }));
        let ask = match pending {
            Some(crate::MemberCommand::Cure { target }) => Some(AiAsk::Cure(member_of(target, *id))),
            Some(crate::MemberCommand::Action { def: crate::ActionDef::DonaldCure, target }) => Some(AiAsk::Cure(member_of(target, *id))),
            Some(crate::MemberCommand::Action { def, target }) if is_donald_ai_def(def) => Some(AiAsk::Attack(match target {
                crate::TargetRef::Enemy(t) => Some(t.0),
                _ => None,
            })),
            _ => None,
        };
        let input_parts = gather(party, *id, *pos, sora, sora_in, host);
        let (enemies, allies) = input_parts;
        let (hp, hp_max, mp, mp_max, charging, in_battle) = (m.status.hp, m.status.max_hp(), m.status.mp, m.status.max_mp(), m.status.mp_charge_left > 0.0, m.in_battle);
        let (barrier_left, orbs) = (super::allyfx::barrier_of(party, *id).map_or(0.0, |b| b.left), super::allyfx::orbs_of(party, *id).map_or(0, |o| o.orbs));
        party.don_prog.refused.retain(|r| clock - r.2 <= REFUSED_MEMORY);
        let refused: Vec<(SpellRef, CastTarget)> = party.don_prog.refused.iter().filter(|r| r.0 == *id).map(|r| r.1).collect();
        let input = AiInput { me: state, id: *id, pos: *pos, hp, hp_max, mp, mp_max, mp_charging: charging, in_battle, barrier_left, orbs, enemies: &enemies, allies: &allies, refused: &refused };
        let mut done = None;
        if let Some(ask) = ask {
            let choice = if charging && !matches!(ask, AiAsk::Cure(_)) { None } else { decide(&input, ask) };
            let ok = match choice {
                Some(c) => execute(party, *id, c, host),
                None => false,
            };
            if let (Some(AiChoice::Cast { plan, target }), false) = (choice, ok) {
                party.don_prog.refused.push((*id, (plan.spell, target), clock));
            }
            // (a melee request the overhaul has nothing for: his KH3 staff hit stands, the last resort up close)
            if ok || !melee {
                if let Some(m) = party.member_mut(*id) {
                    m.don_take_pending(ok);
                }
            }
            done = choice.map(|c| (c, ok));
        } else if in_battle {
            let mem = party.don_prog.ai.iter().find(|a| a.member == *id).copied();
            let ready = mem.is_none_or(|a| a.move_in <= 0.0);
            let free = party.member(*id).is_some_and(|m| matches!(m.state, S::Idle | S::Move));
            if ready && free {
                if let Some(c @ AiChoice::Move { .. }) = range_move(&input) {
                    let ok = execute(party, *id, c, host);
                    done = Some((c, ok));
                }
            }
        }
        let every = prog().value("ai.move_every");
        let mem = match party.don_prog.ai.iter_mut().find(|a| a.member == *id) {
            Some(a) => a,
            None => {
                party.don_prog.ai.push(AiMemory { member: *id, move_in: 0.0, last: None, chosen: 0, started: 0 });
                party.don_prog.ai.last_mut().unwrap()
            }
        };
        mem.move_in = if matches!(done, Some((AiChoice::Move { .. }, true))) { every } else { (mem.move_in - dt).max(0.0) };
        if let Some((_, ok)) = done {
            mem.last = done;
            mem.chosen += 1;
            mem.started += ok as u32;
        }
    }
    let _ = clock;
}

fn member_of(t: crate::TargetRef, me: MemberId) -> MemberId {
    match t {
        crate::TargetRef::Member(m) => m,
        crate::TargetRef::Sora => SORA_MEMBER,
        _ => me,
    }
}

/// The enemies (the host's targets with their weaknesses and the party's statuses) and the allies of one Donald.
fn gather(party: &crate::party::Party, me: MemberId, pos: Vec3, sora: &crate::SoraView, sora_in: bool, host: &crate::PartyHost) -> (Vec<AiEnemy>, Vec<AiAlly>) {
    use kh_core::enums::ETresDamageAttribute as At;
    let mut infos = Vec::new();
    host.targets.enumerate(&mut infos);
    let attr = |e: Element| -> Option<usize> {
        Some(match e {
            Element::Fire => At::Fire as usize,
            Element::Blizzard => At::Blizzard as usize,
            Element::Thunder => At::Thunder as usize,
            Element::Water => At::Water as usize,
            Element::Aero => At::Aero as usize,
            Element::Light => 7,
        })
    };
    let reach = prog().value("ai.range_max") * 2.0;
    let enemies = infos
        .iter()
        .filter(|t| t.flags.0 & kh_core::host::TargetFlags::ALIVE != 0 && t.flags.0 & kh_core::host::TargetFlags::HITTABLE != 0)
        .filter(|t| t.position.distance(pos) <= reach.max(3000.0))
        .map(|t| {
            // the host's elements (Targets::element_affinity: the KH3 resists, Elden Ring's NpcParam), Light in slot 7
            let def = host.targets.element_affinity(t.id);
            let (mut weak, mut resist) = (ElemSet::NONE, ElemSet::NONE);
            for e in Element::ALL {
                if let (Some(d), Some(i)) = (def.as_ref(), attr(e)) {
                    match d.get(i).copied().unwrap_or(0) {
                        r if r < 0 => weak.insert(e),
                        r if r > 0 => resist.insert(e),
                        _ => {}
                    }
                }
            }
            let st = party.don.status.get(t.id.0);
            AiEnemy { id: t.id.0, pos: t.position, weak, resist, wet: st.is_some_and(|s| s.is_wet()), chilled: st.is_some_and(|s| s.is_chilled()), frozen: st.is_some_and(|s| s.is_frozen()), burned: st.is_some_and(|s| s.is_burned()), big: st.is_some_and(|s| s.size.freeze_proof()) }
        })
        .collect();
    let memory = prog().value("ai.thunder_memory");
    let thunder = party.don_prog.sora_thunder.is_some_and(|t| party.don_prog.clock - t <= memory);
    let mut allies = Vec::new();
    for id in party.members().filter(|m| *m != me) {
        let Some(m) = party.member(id) else { continue };
        allies.push(AiAlly {
            member: id,
            pos: m.position(),
            hp: m.status.hp,
            hp_max: m.status.max_hp(),
            ko: m.status.hp < 1,
            melee: m.chara == crate::Chara::Goofy || m.pad.is_some(),
            attacked: party.don_prog.attacked(id) || m.state == crate::NpcStateId::Damage,
            barrier_left: super::allyfx::barrier_of(party, id).map_or(0.0, |b| b.left),
            orbs: super::allyfx::orbs_of(party, id).map_or(0, |o| o.orbs),
            thunder: false,
        });
    }
    if sora_in {
        let (spos, hp, hp_max) = match sora.ai_sora {
            Some((_, p)) => (p, 1, 1),
            None => (sora.position, sora.hp, sora.hp_max),
        };
        allies.push(AiAlly { member: SORA_MEMBER, pos: spos, hp, hp_max, ko: sora.ai_sora.is_none() && sora.dead, melee: true, attacked: party.don_prog.attacked(SORA_MEMBER), barrier_left: super::allyfx::barrier_of(party, SORA_MEMBER).map_or(0.0, |b| b.left), orbs: super::allyfx::orbs_of(party, SORA_MEMBER).map_or(0, |o| o.orbs), thunder });
    }
    (enemies, allies)
}

/// Carries a choice out on member `id`. True when it started.
fn execute(party: &mut crate::party::Party, id: MemberId, c: AiChoice, host: &mut crate::PartyHost) -> bool {
    match c {
        AiChoice::Cast { plan, target } => {
            let Some(m) = party.member_mut(id) else { return false };
            super::cast::begin(m, &plan, target, host)
        }
        AiChoice::Attune(e) => {
            let Some(m) = party.member_mut(id) else { return false };
            attune(m, e)
        }
        AiChoice::Move { to, .. } => {
            let t = party.command(id, crate::MemberCommand::MoveTo { goal: crate::MoveGoal::Point(to), locomotion: crate::LocomotionDef::Move1, walk: false, acceptable_radius: 100.0, follow_path: false });
            party.command_status(id, t) != crate::CommandStatus::Failed
        }
    }
}

/// THE ATTUNE CAST for AI Donald: DON-KIT's `pad::donald::begin_attune` (the clip + the core's cast, the AI's too).
fn attune(p: &mut crate::pawn::NpcPawn, e: Element) -> bool {
    crate::pad::donald::begin_attune(p, e)
}
