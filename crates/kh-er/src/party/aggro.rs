//! (AGGRO) WHOM ELDEN RING'S ENEMIES ATTACK: THE WHOLE PARTY, BY THE AGGRO RULES (stream AGGRO, 2026-10-23; state note
//! build/sora_tmp/party/IMPL_AGGRO_STATE.md). The user: "Currently Elden Ring enemies are ALWAYS aggroing to the party
//! leader, change their behaviour so they respect the aggro rules set by abilities and also do target party members
//! even if there are no aggro-based abilities in play".
//!
//! WHY THEY ONLY EVER WENT FOR THE LEADER (found by this stream; the fixes are in their own files, marked "(AGGRO)"):
//! 1. THE FRIENDS HAD NO ELDEN RING BODY ON 2.7.2.0: the SDK's `WorldChrMan.debug_chr_creator` is the 2.7.1.0 place; the
//!    2.7.2.0 constructor has two more fields before it, so the read gave 0 and every body route said "the game has no
//!    debug character creator object" (the user's logs, 50 times). game.rs `debug_chr_creator` (version table
//!    gamever.rs `wcm`, vtable-checked; the same shift moved `chr_cam`).
//! 2. THE SPIRIT ASH PICK COULD NEVER SUCCEED: it asked for no immune ailment and every Spirit Ash of the game is immune to
//!    blood loss and frost (allybody.rs `auto_pick`: the fewest immunities now; the Spirit Ash model id rule fixed too).
//! 3. EVERY FRIEND'S HIT WAS THE PLAYER'S: Elden Ring's enemies weigh damage into their choice of target (NpcThinkParam
//!    `targetSysDmgEffectRate` 100 % on 2114 of 2215 rows), so each of Donald's / Goofy's hits pulled the enemy back to
//!    the leader. Their hits are now fired by their own bodies (allybody_live.rs `party.bullet_owner`, default `all`).
//! 4. A BODY'S BULLET CARRIED NO ATTACK: the game reads AtkParam_Npc for a bullet an NPC owns, and none of the borrowed
//!    attack ids has a row there; body-owned bullets have their own unused rows now (bullet.rs `pick_owned`).
//!
//! THIS FILE: THE DIRECTOR. Every engaged hostile is given a party member to attack, by rules that hold for any enemy
//! and boss (no per-enemy table; sheet `party-er-aggro`):
//! - ABILITIES FIRST: Goofy's Over Here! (a taunt: every enemy in its reach goes to him while it lasts; taunt.rs fires
//!   its hits), Donald's Taunt Pass (an ally guarding takes the enemies near Donald), Low Profile / Hide Behind (his
//!   weight), Distraction (an enemy that chose him switches to his nearest ally), Out of Sight (weight 0 at range),
//!   Zettaflare (no enemy chooses him while it runs). Donald's rules are kh-party's own (`donald::aggro`).
//! - THE NATURAL THREAT MODEL otherwise: a score per member from who is near and in front of the enemy, who hurt it
//!   lately and how much, who hit it last, who is casting, the leader's fair share, a bonus for its current target
//!   (no flip-flop) and a crowding penalty (attention spread over the party); the choice is a weighted roll, held for
//!   `aggro.hold_min` - `aggro.hold_max` s, re-made early only when another member hurts it hard (`aggro.provoke`) and
//!   never sooner than `aggro.switch_min` s after the last switch.
//! - ENFORCED THROUGH THE GAME'S OWN TARGETING: (a) a 1-point hate hit from the chosen member's body (the player's for the
//!   leader) when the choice changes and every `aggro.refresh` s; (b) the CREDIT of party hits: an enemy held by an
//!   ability takes the hate of every party hit to the member the ability names (allybody_live.rs `owner_of` asks
//!   [`credit`]); a hit that may kill stays the player's (the game's kill credit).
//! Never a hit moved: where a hit lands is untouched (an enemy's on a friend's body lands on him: HP, reaction, guard).
//! Frame-rate independent: every timer is seconds of the frame's `dt`. `party.aggro = 0`: off (donaggro.rs's hate route
//! runs as before). UNTESTED IN GAME.

use std::collections::HashMap;
use std::sync::Mutex;

use glam::Vec3;
use kh_core::TargetId;
use kh_host::party::PartyChara;

/// THE PURE RULES (no game types): the sheet, the threat model, the ability rules, the director's book. Unit-tested.
pub mod rules {
    use super::*;
    use kh_party::donald::aggro::{self as don, AggroCtx};
    use kh_party::donald::DonAbilities;

    /// The sheet `party-er-aggro` (defaults = its rows as authored).
    #[derive(Clone, Debug, PartialEq)]
    pub struct Sheet {
        /// cm: a member farther than this from an enemy is no candidate.
        pub reach: f32,
        /// The score every candidate starts with.
        pub base: f32,
        pub w_near: f32,
        /// cm: "near" falls from 1 at the enemy to 0 at this distance.
        pub near_range: f32,
        pub w_front: f32,
        pub w_damage: f32,
        /// s: the hits remembered for "who hurt it" (each counts less as it ages).
        pub damage_window: f32,
        pub w_last_hit: f32,
        /// s: "who hit it last" counts this long.
        pub last_hit_window: f32,
        pub w_cast: f32,
        pub w_leader: f32,
        pub w_current: f32,
        pub w_crowd: f32,
        /// s: a choice is held for a time between these two (a roll).
        pub hold_min: f32,
        pub hold_max: f32,
        /// s: never two switches closer than this.
        pub switch_min: f32,
        /// Share of the enemy's maximum HP one member dealt within the window that re-makes the choice early.
        pub provoke: f32,
        /// s: the hate hit is fired again this often while the choice holds.
        pub refresh: f32,
        /// s: at least this between two hate hits of the director (the bullet slots are shared).
        pub gap: f32,
        /// s: an enemy stays engaged this long after the last sign (a hit either way, an attack seen near the party).
        pub engaged_for: f32,
        /// cm: an enemy seen attacking within this of a member counts as engaged.
        pub engage_near: f32,
        /// Attack value of a hate hit.
        pub hit_phys: u16,
        /// A hit counts as possibly lethal (the player's: kill credit) when its Elden Ring value times this reaches the
        /// enemy's HP.
        pub lethal_margin: f32,
    }

    impl Default for Sheet {
        fn default() -> Sheet {
            Sheet {
                reach: 3000.0,
                base: 1.0,
                w_near: 1.5,
                near_range: 1500.0,
                w_front: 0.75,
                w_damage: 1.5,
                damage_window: 10.0,
                w_last_hit: 1.0,
                last_hit_window: 3.0,
                w_cast: 1.0,
                w_leader: 0.5,
                w_current: 1.0,
                w_crowd: 1.0,
                hold_min: 5.0,
                hold_max: 10.0,
                switch_min: 2.0,
                provoke: 0.08,
                refresh: 4.0,
                gap: 0.1,
                engaged_for: 8.0,
                engage_near: 1500.0,
                hit_phys: 1,
                lethal_margin: 2.0,
            }
        }
    }

    impl Sheet {
        /// The sheet's rows (`{"rows":[[id, value, ...], ...]}`) over the defaults; the ids not understood are returned.
        pub fn from_text(text: &str) -> Result<(Sheet, Vec<String>), String> {
            let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
            let rows = v.get("rows").and_then(|r| r.as_array()).ok_or("no rows")?;
            let mut s = Sheet::default();
            let mut bad = Vec::new();
            for row in rows {
                let (Some(id), Some(value)) = (row.get(0).and_then(|v| v.as_str()), row.get(1)) else { continue };
                let f = value.as_f64().map(|v| v as f32);
                let ok = match id {
                    "aggro.reach" => f.map(|v| s.reach = v).is_some(),
                    "aggro.base" => f.map(|v| s.base = v).is_some(),
                    "aggro.w_near" => f.map(|v| s.w_near = v).is_some(),
                    "aggro.near_range" => f.map(|v| s.near_range = v).is_some(),
                    "aggro.w_front" => f.map(|v| s.w_front = v).is_some(),
                    "aggro.w_damage" => f.map(|v| s.w_damage = v).is_some(),
                    "aggro.damage_window" => f.map(|v| s.damage_window = v).is_some(),
                    "aggro.w_last_hit" => f.map(|v| s.w_last_hit = v).is_some(),
                    "aggro.last_hit_window" => f.map(|v| s.last_hit_window = v).is_some(),
                    "aggro.w_cast" => f.map(|v| s.w_cast = v).is_some(),
                    "aggro.w_leader" => f.map(|v| s.w_leader = v).is_some(),
                    "aggro.w_current" => f.map(|v| s.w_current = v).is_some(),
                    "aggro.w_crowd" => f.map(|v| s.w_crowd = v).is_some(),
                    "aggro.hold_min" => f.map(|v| s.hold_min = v).is_some(),
                    "aggro.hold_max" => f.map(|v| s.hold_max = v).is_some(),
                    "aggro.switch_min" => f.map(|v| s.switch_min = v).is_some(),
                    "aggro.provoke" => f.map(|v| s.provoke = v).is_some(),
                    "aggro.refresh" => f.map(|v| s.refresh = v).is_some(),
                    "aggro.gap" => f.map(|v| s.gap = v).is_some(),
                    "aggro.engaged_for" => f.map(|v| s.engaged_for = v).is_some(),
                    "aggro.engage_near" => f.map(|v| s.engage_near = v).is_some(),
                    "aggro.hit_phys" => value.as_u64().map(|v| s.hit_phys = v.min(50) as u16).is_some(),
                    "aggro.lethal_margin" => f.map(|v| s.lethal_margin = v).is_some(),
                    _ => false,
                };
                if !ok {
                    bad.push(id.to_string());
                }
            }
            Ok((s, bad))
        }

        /// The sheet as shipped (the defaults, checked equal by a test).
        pub fn shipped() -> Sheet {
            Sheet::from_text(include_str!("../../../../sheets/port/party-er-aggro.json")).map(|s| s.0).unwrap_or_default()
        }
    }

    /// One party member as the director sees it.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Member {
        pub chara: PartyChara,
        /// KH3 space (cm).
        pub pos: Vec3,
        pub leader: bool,
        /// Up, hittable, with an Elden Ring body the enemy can attack (the leader's is the player's).
        pub up: bool,
        /// Casting a spell now (Donald's: a cast begun lately).
        pub casting: bool,
    }

    /// One engaged hostile.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Foe {
        pub id: TargetId,
        pub pos: Vec3,
        /// Degrees, the core's yaw (forward = (cos, sin)).
        pub yaw: f32,
        pub hp_max: f32,
    }

    /// A party hit on an enemy (KH3 damage), attributed.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Hit {
        pub foe: TargetId,
        pub by: PartyChara,
        pub damage: f32,
        pub at: f32,
    }

    /// Over Here! now.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct TauntIn {
        pub by: PartyChara,
        pub centre: Vec3,
        pub radius: f32,
    }

    /// Donald's side of the rules.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct DonIn {
        pub abilities: DonAbilities,
        pub ctx: AggroCtx,
        /// Zettaflare runs: no enemy chooses him.
        pub zetta: bool,
    }

    /// Why a member was chosen.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum Why {
        /// The threat model.
        Natural,
        /// Goofy's Over Here!.
        Taunt,
        /// Donald's Taunt Pass (an ally guarding).
        TauntPass,
        /// Donald's Distraction.
        Distraction,
    }

    impl Why {
        /// The choice is an ability's: the hate of every party hit on the enemy goes to the member chosen.
        pub fn ability(self) -> bool {
            self != Why::Natural
        }
        pub fn name(self) -> &'static str {
            match self {
                Why::Natural => "the threat model",
                Why::Taunt => "Over Here! (a taunt)",
                Why::TauntPass => "Taunt Pass (an ally guarding)",
                Why::Distraction => "Distraction",
            }
        }
    }

    /// What the director reads each frame.
    pub struct Input<'a> {
        pub now: f32,
        pub members: &'a [Member],
        pub foes: &'a [Foe],
        pub hits: &'a [Hit],
        pub taunts: &'a [TauntIn],
        /// Taunt Pass's ally now (an ally guarding).
        pub guard: Option<PartyChara>,
        pub donald: Option<DonIn>,
    }

    /// A hate hit wanted this frame.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Order {
        pub foe: TargetId,
        pub to: PartyChara,
        pub why: Why,
        /// The choice is new (else a refresh).
        pub first: bool,
    }

    /// One enemy's choice.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Choice {
        pub to: PartyChara,
        pub why: Why,
        pub since: f32,
        pub hold_until: f32,
        pub decisions: u32,
        /// The last hate hit for this choice (`None`: not fired yet).
        pub fired: Option<f32>,
    }

    /// THE DIRECTOR'S BOOK: the choices, the remembered hits.
    #[derive(Clone, Debug, Default)]
    pub struct Director {
        pub choices: HashMap<TargetId, Choice>,
        pub hits: Vec<Hit>,
        pub switches: u64,
    }

    fn forward(yaw: f32) -> Vec3 {
        let r = yaw.to_radians();
        Vec3::new(r.cos(), r.sin(), 0.0)
    }

    fn flat(v: Vec3) -> Vec3 {
        Vec3::new(v.x, v.y, 0.0)
    }

    /// The score parts of one candidate (for the log and the tests).
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Score {
        pub near: f32,
        pub front: f32,
        pub damage: f32,
        pub last: f32,
        pub cast: f32,
        pub crowd: f32,
        pub total: f32,
    }

    impl Director {
        /// The member chosen for `foe` now (and why).
        pub fn choice(&self, foe: TargetId) -> Option<Choice> {
            self.choices.get(&foe).copied()
        }

        /// The score of `m` for `foe` (before Donald's weight).
        pub fn score(&self, s: &Sheet, i: &Input, foe: &Foe, m: &Member, current: Option<PartyChara>) -> Score {
            let to = flat(m.pos - foe.pos);
            let d = to.length();
            let near = (1.0 - d / s.near_range.max(1.0)).clamp(0.0, 1.0);
            let front = if d > 1.0 { forward(foe.yaw).dot(to / d).max(0.0) } else { 1.0 };
            // who hurt it: each hit counts less as it ages, as a share of everything the party dealt it
            let age_w = |h: &Hit| (1.0 - (i.now - h.at) / s.damage_window.max(0.1)).clamp(0.0, 1.0);
            let on_foe = || self.hits.iter().filter(|h| h.foe == foe.id);
            let all: f32 = on_foe().map(|h| h.damage * age_w(h)).sum();
            let mine: f32 = on_foe().filter(|h| h.by == m.chara).map(|h| h.damage * age_w(h)).sum();
            let damage = if all > 0.0 { mine / all } else { 0.0 };
            let last = on_foe().max_by(|a, b| a.at.total_cmp(&b.at)).filter(|h| h.by == m.chara && i.now - h.at <= s.last_hit_window).map_or(0.0, |_| 1.0);
            let cast = if m.casting { 1.0 } else { 0.0 };
            let others = self.choices.iter().filter(|(id, c)| **id != foe.id && c.to == m.chara && i.foes.iter().any(|f| f.id == **id)).count() as f32;
            let crowd = others / (i.foes.len().max(2) - 1) as f32;
            let total = s.base + s.w_near * near + s.w_front * front + s.w_damage * damage + s.w_last_hit * last + s.w_cast * cast + if m.leader { s.w_leader } else { 0.0 } + if current == Some(m.chara) { s.w_current } else { 0.0 } - s.w_crowd * crowd;
            Score { near, front, damage, last, cast, crowd, total: total.max(0.05) }
        }

        /// THE CHOICE for `foe` among `cands` (the abilities over the threat model). `None`: no candidate.
        pub fn choose(&self, s: &Sheet, i: &Input, foe: &Foe, cands: &[Member], current: Option<PartyChara>, decisions: u32) -> Option<(PartyChara, Why)> {
            // Zettaflare: no enemy chooses Donald (unless he is the only one there)
            let zetta = i.donald.is_some_and(|d| d.zetta);
            let pool: Vec<Member> = if zetta && cands.iter().any(|m| m.chara != PartyChara::Donald) { cands.iter().copied().filter(|m| m.chara != PartyChara::Donald).collect() } else { cands.to_vec() };
            let key = foe.id.0 as u64;
            let weights: Vec<f32> = pool
                .iter()
                .map(|m| {
                    let mut w = self.score(s, i, foe, m, current).total;
                    if m.chara == PartyChara::Donald {
                        if let Some(d) = i.donald {
                            w *= don::pick_weight(&d.abilities, &d.ctx, flat(m.pos - foe.pos).length());
                        }
                    }
                    w
                })
                .collect();
            let sum: f32 = weights.iter().sum();
            let picked = if sum <= 0.0 {
                // (every weight 0: Out of Sight on the only candidate) the nearest
                pool.iter().min_by(|a, b| a.pos.distance(foe.pos).total_cmp(&b.pos.distance(foe.pos)))?
            } else {
                let mut r = don::roll(key, decisions as u64, 31) * sum;
                let mut at = &pool[pool.len() - 1];
                for (m, w) in pool.iter().zip(&weights) {
                    if r < *w {
                        at = m;
                        break;
                    }
                    r -= *w;
                }
                at
            };
            if picked.chara == PartyChara::Donald {
                if let Some(d) = i.donald {
                    let others: Vec<&Member> = pool.iter().filter(|m| m.chara != PartyChara::Donald).collect();
                    // Taunt Pass: the ally guarding takes the enemies near him
                    if let Some(g) = i.guard.and_then(|g| others.iter().find(|m| m.chara == g)) {
                        if don::taunt_pass(&d.abilities, &d.ctx, flat(foe.pos - picked.pos).length()) {
                            return Some((g.chara, Why::TauntPass));
                        }
                    }
                    // Distraction: it switches to his nearest ally
                    if don::switch_on_target(&d.abilities, &d.ctx, don::roll(key, decisions as u64, 32)) {
                        if let Some(n) = others.iter().min_by(|a, b| a.pos.distance(picked.pos).total_cmp(&b.pos.distance(picked.pos))) {
                            return Some((n.chara, Why::Distraction));
                        }
                    }
                }
            }
            Some((picked.chara, Why::Natural))
        }

        /// ONE FRAME: the hits remembered, every engaged enemy's choice kept or re-made; the hate hits wanted (new
        /// choices first, then the refreshes due, oldest first). Over Here!'s enemies are held to the taunter without an
        /// order (taunt.rs fires its own hits).
        pub fn step(&mut self, s: &Sheet, i: &Input) -> Vec<Order> {
            self.hits.extend_from_slice(i.hits);
            self.hits.retain(|h| i.now - h.at <= s.damage_window.max(s.last_hit_window));
            self.choices.retain(|id, _| i.foes.iter().any(|f| f.id == *id));
            let mut orders = Vec::new();
            for foe in i.foes {
                let cands: Vec<Member> = i.members.iter().copied().filter(|m| m.up && (m.leader || flat(m.pos - foe.pos).length() <= s.reach)).collect();
                if cands.is_empty() {
                    self.choices.remove(&foe.id);
                    continue;
                }
                let old = self.choices.get(&foe.id).copied();
                let current = old.map(|c| c.to).filter(|c| cands.iter().any(|m| m.chara == *c));
                // Over Here!: the taunter while the enemy is in its reach
                if let Some(t) = i.taunts.iter().find(|t| cands.iter().any(|m| m.chara == t.by) && flat(foe.pos - t.centre).length() <= t.radius) {
                    let fresh = old.is_none_or(|c| c.to != t.by || c.why != Why::Taunt);
                    if fresh {
                        self.switches += 1;
                    }
                    let n = old.map_or(0, |c| c.decisions) + fresh as u32;
                    let since = if fresh { i.now } else { old.map_or(i.now, |c| c.since) };
                    self.choices.insert(foe.id, Choice { to: t.by, why: Why::Taunt, since, hold_until: i.now + s.hold_min, decisions: n, fired: Some(i.now) });
                    continue;
                }
                let zetta_on_him = i.donald.is_some_and(|d| d.zetta) && current == Some(PartyChara::Donald) && cands.len() > 1;
                let provoked = current.is_some_and(|c| {
                    i.members.iter().filter(|m| m.chara != c && cands.contains(m)).any(|m| {
                        let dealt: f32 = self.hits.iter().filter(|h| h.foe == foe.id && h.by == m.chara && i.now - h.at <= s.damage_window).map(|h| h.damage).sum();
                        foe.hp_max > 0.0 && dealt >= s.provoke * foe.hp_max
                    })
                });
                let since_switch = old.map_or(f32::INFINITY, |c| i.now - c.since);
                let due = match (old, current) {
                    (None, _) | (_, None) => true,
                    (Some(c), Some(_)) => zetta_on_him || i.now >= c.hold_until || (provoked && since_switch >= s.switch_min) || (c.why == Why::Taunt),
                };
                if !due {
                    let c = old.unwrap();
                    if c.fired.is_none_or(|t| i.now - t >= s.refresh) {
                        orders.push((c.fired.unwrap_or(f32::NEG_INFINITY), Order { foe: foe.id, to: c.to, why: c.why, first: c.fired.is_none() }));
                    }
                    continue;
                }
                let n = old.map_or(0, |c| c.decisions + 1);
                let Some((to, why)) = self.choose(s, i, foe, &cands, current, n) else { continue };
                let hold = s.hold_min + (s.hold_max - s.hold_min).max(0.0) * don::roll(foe.id.0 as u64, n as u64, 33);
                let changed = old.is_none_or(|c| c.to != to);
                if changed {
                    self.switches += 1;
                }
                let fired = if changed { None } else { old.and_then(|c| c.fired) };
                let since = if changed { i.now } else { old.map_or(i.now, |c| c.since) };
                self.choices.insert(foe.id, Choice { to, why, since, hold_until: i.now + hold, decisions: n, fired });
                if fired.is_none_or(|t| i.now - t >= s.refresh) {
                    orders.push((fired.unwrap_or(f32::NEG_INFINITY), Order { foe: foe.id, to, why, first: fired.is_none() }));
                }
            }
            orders.sort_by(|a, b| b.1.first.cmp(&a.1.first).then(a.0.total_cmp(&b.0)));
            orders.into_iter().map(|o| o.1).collect()
        }

        /// The hate hit of `foe`'s choice was fired now.
        pub fn fired(&mut self, foe: TargetId, now: f32) {
            if let Some(c) = self.choices.get_mut(&foe) {
                c.fired = Some(now);
            }
        }
    }

    /// Whose body a party hit is credited to (allybody_live.rs `owner_of`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Credit {
        /// The hitter's own (the player's for the leader).
        Own,
        /// The player's.
        Leader,
        /// This member's body.
        To(PartyChara),
    }

    /// What [`credit_of`] reads: the enemy's choice, Donald's side, the enemy's Elden Ring HP, the leader.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct CreditIn {
        pub choice: Option<(PartyChara, Why)>,
        pub leader: PartyChara,
        /// Donald's weight for this enemy (1 = unchanged, 0 = Out of Sight / Zettaflare).
        pub don_weight: f32,
        /// The enemy's Elden Ring HP and the Elden Ring value of this hit.
        pub hp: i32,
        pub er_damage: i32,
    }

    /// THE CREDIT RULE: a hit that may kill is the player's (the game's kill credit); an enemy held by an ability takes
    /// the hate of every party hit to the member chosen; Donald's own hits on an enemy that should not choose him go to
    /// its choice with his weight's complement (`roll` uniform in 0..1); else the hitter's own.
    pub fn credit_of(c: &CreditIn, hitter: Option<PartyChara>, lethal_margin: f32, roll: f32) -> Credit {
        if c.hp > 0 && c.er_damage as f32 * lethal_margin.max(1.0) >= c.hp as f32 {
            return Credit::Leader;
        }
        let to_member = |m: PartyChara| if m == c.leader { Credit::Leader } else { Credit::To(m) };
        match c.choice {
            Some((to, why)) if why.ability() && Some(to) != hitter => to_member(to),
            Some((to, _)) if hitter == Some(PartyChara::Donald) && to != PartyChara::Donald && roll >= c.don_weight => to_member(to),
            _ => Credit::Own,
        }
    }
}

pub use rules::Credit;

/// A party hit as targets.rs sees it (attributed in the director's frame, where the members' feet are known).
#[derive(Clone, Copy, Debug)]
struct RawHit {
    foe: TargetId,
    attacker: Option<Vec3>,
    donald: bool,
    damage: f32,
}

/// What the credit rule needs between frames.
#[derive(Default)]
struct CreditBook {
    on: bool,
    leader: Option<PartyChara>,
    choices: HashMap<TargetId, (PartyChara, rules::Why, f32 /* Donald's weight */)>,
    hp: HashMap<TargetId, i32>,
    out_scale: f32,
    lethal_margin: f32,
    counter: u64,
}

static LEDGER: Mutex<Vec<RawHit>> = Mutex::new(Vec::new());
static BOOK: Mutex<Option<CreditBook>> = Mutex::new(None);

/// (targets.rs `apply_hit`) A party hit on target `foe`: its attacker point (KH3 space), Donald's magic flag, its KH3
/// damage. Kept for the director's next frame (at most 256).
pub fn note_hit(foe: TargetId, attacker: Option<Vec3>, flags: u32, damage: f32) {
    if let Ok(mut l) = LEDGER.try_lock() {
        if l.len() < 256 && damage > 0.0 {
            l.push(RawHit { foe, attacker, donald: flags & kh_party::donald::magic_fx::DON_HIT_FLAG != 0, damage });
        }
    }
}

/// (allybody_live.rs `owner_of`) Whose body party hit on `foe` by `hitter` (`None`: the leader) is credited to.
pub fn credit(foe: TargetId, hitter: Option<PartyChara>, kh_damage: f32) -> Credit {
    let Ok(mut g) = BOOK.try_lock() else { return Credit::Own };
    let Some(b) = g.as_mut().filter(|b| b.on) else { return Credit::Own };
    b.counter += 1;
    let leader = b.leader.unwrap_or(PartyChara::Sora);
    let c = b.choices.get(&foe).copied();
    let input = rules::CreditIn { choice: c.map(|c| (c.0, c.1)), leader, don_weight: c.map_or(1.0, |c| c.2), hp: b.hp.get(&foe).copied().unwrap_or(0), er_damage: (kh_damage * b.out_scale).round() as i32 };
    let hitter = hitter.or(Some(leader));
    let roll = kh_party::donald::aggro::roll(foe.0 as u64, b.counter, 41);
    rules::credit_of(&input, hitter, b.lethal_margin, roll)
}

/// Is the director on (donaggro.rs leaves its hate route to it).
pub fn on(control: &crate::control::Control) -> bool {
    control.flag("party.aggro") != Some(false) && control.flag("party.er") != Some(false)
}

#[cfg(windows)]
pub use live::frame;

#[cfg(windows)]
mod live {
    use super::rules::{self, DonIn, Foe, Hit, Input, Member, Sheet, TauntIn, Why};
    use super::*;
    use crate::control::Control;
    use crate::erlog;

    struct State {
        sheet: Sheet,
        gen: Option<u64>,
        dir: rules::Director,
        clock: f32,
        engaged: HashMap<TargetId, f32>,
        last_fire: f32,
        fired: u64,
        refreshed: u64,
        failed: u64,
        report_at: f32,
        told_no_body: Vec<PartyChara>,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);

    fn load_sheet() -> (Sheet, String) {
        let log = crate::log::dir();
        let path = log.parent().map(|root| crate::sora::data_paths(root).1.with_file_name("party-er-aggro.json"));
        match path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).as_deref().map(Sheet::from_text) {
            Some(Ok((s, bad))) => (s, format!("sheet {} ({} row(s) not understood: {bad:?})", path.map(|p| p.display().to_string()).unwrap_or_default(), bad.len())),
            Some(Err(e)) => (Sheet::shipped(), format!("sheet not read ({e}): the shipped rows")),
            None => (Sheet::shipped(), "no sheet file beside the loadout: the shipped rows".to_string()),
        }
    }

    /// ONCE A FRAME (sora.rs pre_physics, after the ally bodies, the taunt and Donald's aggro): the choices and at most
    /// one hate hit. `sora_pos`: Sora's KH3 feet (leader or AI Sora).
    pub fn frame(core: Option<&crate::party::glue::PartyCore>, player: eldenring::cs::FieldInsHandle, targets: &mut crate::targets::ErTargets, control: &Control, sora_pos: Vec3, dt: f32) {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let raw: Vec<RawHit> = LEDGER.lock().map(|mut l| std::mem::take(&mut *l)).unwrap_or_default();
        let Some(core) = core.filter(|_| on(control)) else {
            if let Ok(mut b) = BOOK.lock() {
                *b = None;
            }
            return;
        };
        let st = guard.get_or_insert_with(|| {
            let (sheet, note) = load_sheet();
            erlog!("aggro: the director starts ({note}): {sheet:?}");
            State { sheet, gen: None, dir: Default::default(), clock: 0.0, engaged: HashMap::new(), last_fire: -1.0e6, fired: 0, refreshed: 0, failed: 0, report_at: 30.0, told_no_body: Vec::new() }
        });
        if st.gen.is_some_and(|g| g != control.generation) {
            let (sheet, note) = load_sheet();
            if sheet != st.sheet {
                erlog!("aggro: the sheet read again ({note})");
                st.sheet = sheet;
            }
        }
        st.gen = Some(control.generation);
        st.clock += dt.max(0.0);
        let now = st.clock;
        let s = st.sheet.clone();
        let rt = &core.rt;
        let leader = rt.leader_chara();
        // the members: the leader (the player's body), the AI allies with a live body
        let mut members: Vec<Member> = Vec::new();
        let don_id = rt.current.iter().find(|v| v.chara == kh_party::Chara::Donald && !v.ko).map(|v| v.id);
        let don_ctx = don_id.map(|id| rt.party.don_prog.aggro_of(id));
        for v in &rt.current {
            let c = kh_partyhost::party_chara(v.chara);
            let is_leader = c == leader;
            members.push(Member { chara: c, pos: v.position, leader: is_leader, up: !v.ko && (is_leader || crate::allybody::live_handle(c).is_some()), casting: Some(v.id) == don_id && don_ctx.is_some_and(|x| x.casting) });
        }
        if rt.lead.sora_active && !members.iter().any(|m| m.chara == PartyChara::Sora) {
            let is_leader = leader == PartyChara::Sora;
            members.push(Member { chara: PartyChara::Sora, pos: sora_pos, leader: is_leader, up: is_leader || crate::allybody::live_handle(PartyChara::Sora).is_some(), casting: false });
        }
        for m in &members {
            if !m.up && !m.leader && !st.told_no_body.contains(&m.chara) {
                st.told_no_body.push(m.chara);
                erlog!("aggro: {} has no live Elden Ring body now: enemies cannot choose him until his body is there (party-body lines)", m.chara.name());
            }
        }
        // the party's hits of the last frame, attributed: Donald's magic by its flag, else the member nearest the attacker point
        let hits: Vec<Hit> = raw
            .iter()
            .map(|h| {
                let by = if h.donald && members.iter().any(|m| m.chara == PartyChara::Donald) {
                    PartyChara::Donald
                } else {
                    h.attacker.and_then(|p| members.iter().filter(|m| m.pos.distance(p) <= 300.0).min_by(|a, b| a.pos.distance(p).total_cmp(&b.pos.distance(p))).map(|m| m.chara)).unwrap_or(leader)
                };
                Hit { foe: h.foe, by, damage: h.damage, at: now }
            })
            .collect();
        // the engaged hostiles: fought lately, or seen attacking near a member; kept `engaged_for` s
        let mut foes: Vec<Foe> = Vec::new();
        let mut hp = HashMap::new();
        for (_, e) in targets.hostiles_by_distance() {
            hp.insert(e.id, e.hp);
            let near = members.iter().any(|m| m.pos.distance(e.pos) <= s.engage_near);
            if targets.fought_lately(e.id) || (e.in_attack && near) || hits.iter().any(|h| h.foe == e.id) {
                st.engaged.insert(e.id, now + s.engaged_for);
            }
            if st.engaged.get(&e.id).is_some_and(|until| now <= *until) && !targets.passive_now(e) {
                foes.push(Foe { id: e.id, pos: e.pos, yaw: e.yaw, hp_max: e.hp_max as f32 });
            }
        }
        st.engaged.retain(|id, until| now <= *until && hp.contains_key(id));
        // the abilities
        let taunts: Vec<TauntIn> = rt.members.iter().filter_map(|(c, id)| rt.party.goofy_taunt(*id).filter(|t| t.left > 0.0).map(|t| TauntIn { by: *c, centre: t.centre, radius: t.radius })).collect();
        let guard_ally = rt.party.don_prog.taunt_now().and_then(|m| if m == kh_party::donald::cast::SORA_MEMBER { Some(PartyChara::Sora) } else { rt.chara_of_id(m) });
        let donald = don_id.and_then(|id| rt.party.status(id).and_then(|st| st.donald.as_ref()).map(|d| DonIn { abilities: d.abilities, ctx: rt.party.don_prog.aggro_of(id), zetta: rt.party.don_zetta_invincible(id) }));
        let input = Input { now, members: &members, foes: &foes, hits: &hits, taunts: &taunts, guard: guard_ally, donald };
        let before: HashMap<TargetId, PartyChara> = st.dir.choices.iter().map(|(k, v)| (*k, v.to)).collect();
        let orders = st.dir.step(&s, &input);
        // the log: every changed choice
        for f in &foes {
            if let Some(c) = st.dir.choice(f.id) {
                if before.get(&f.id) != Some(&c.to) {
                    let npc = targets.get(f.id).map_or(0, |e| e.npc_id);
                    let parts = members.iter().find(|m| m.chara == c.to).map(|m| st.dir.score(&s, &input, f, m, Some(c.to)));
                    erlog!("aggro: target {} c{npc:04} -> {} by {} (held {:.1} s{})", f.id.0, c.to.name(), c.why.name(), c.hold_until - now, parts.map_or(String::new(), |p| format!("; near {:.2} front {:.2} damage {:.2} last {:.0} cast {:.0} crowd {:.2}", p.near, p.front, p.damage, p.last, p.cast, p.crowd)));
                }
            }
        }
        // the credit rule's book (allybody_live.rs `owner_of`, during the next party step)
        if let Ok(mut b) = BOOK.lock() {
            let book = b.get_or_insert_with(Default::default);
            book.on = true;
            book.leader = Some(leader);
            book.out_scale = targets.params.out_scale;
            book.lethal_margin = s.lethal_margin;
            book.hp = hp;
            book.choices = st
                .dir
                .choices
                .iter()
                .map(|(id, c)| {
                    let w = match (donald, members.iter().find(|m| m.chara == PartyChara::Donald), foes.iter().find(|f| f.id == *id)) {
                        (Some(d), Some(m), _) if d.zetta => {
                            let _ = m;
                            0.0
                        }
                        (Some(d), Some(m), Some(f)) => kh_party::donald::aggro::pick_weight(&d.abilities, &d.ctx, (m.pos - f.pos).truncate().length()),
                        _ => 1.0,
                    };
                    (*id, (c.to, c.why, w))
                })
                .collect();
        }
        // the summary every 30 s
        if now >= st.report_at {
            st.report_at = now + 30.0;
            let mut by: HashMap<PartyChara, usize> = HashMap::new();
            for c in st.dir.choices.values() {
                *by.entry(c.to).or_insert(0) += 1;
            }
            if !st.dir.choices.is_empty() || st.fired > 0 {
                erlog!("aggro: {} engaged enemy(ies) on {} ({} switch(es) so far); hate hits {} new + {} refresh, {} not fired", st.dir.choices.len(), by.iter().map(|(c, n)| format!("{} {n}", c.name())).collect::<Vec<_>>().join(", "), st.dir.switches, st.fired, st.refreshed, st.failed);
            }
        }
        // at most one hate hit per `gap` s (Over Here!'s enemies: taunt.rs's own hits)
        if now - st.last_fire < s.gap || !targets.bullets.available() {
            return;
        }
        for o in orders.iter().filter(|o| o.why != Why::Taunt) {
            let owned = o.to != leader;
            if (owned && !targets.bullets.free_owned_slot()) || (!owned && !targets.bullets.free_slot()) {
                return;
            }
            let Some(e) = targets.get(o.foe).cloned() else { continue };
            let owner = if owned { crate::allybody::live_handle(o.to) } else { Some(player) };
            let Some(owner) = owner else { continue };
            let from_pos = members.iter().find(|m| m.chara == o.to).map_or(e.pos, |m| m.pos);
            let towards = crate::coords::dir_kh_to_er((e.pos - from_pos) * Vec3::new(1.0, 1.0, 0.0)).normalize_or(Vec3::Z);
            let contact = e.er_pos + Vec3::Y * (e.half_height / crate::coords::CM_PER_M);
            let from = contact - towards * targets.shape.back;
            let attack = crate::bullet::Attack { phys: s.hit_phys.max(1), poise: 0.0, stamina: 0, level: 0, carrier: Default::default(), sp_attribute: None, obj: None };
            let shape = targets.shape;
            let fired = if owned { targets.bullets.fire_owned(owner, Some(e.handle), from, towards, attack, shape) } else { targets.bullets.fire(owner, Some(e.handle), from, towards, attack, shape) };
            st.last_fire = now;
            match fired {
                Ok(row) => {
                    st.dir.fired(o.foe, now);
                    if o.first {
                        st.fired += 1;
                        erlog!("aggro: hate hit on target {} c{:04} from {} ({}, bullet row {row}): it should turn to {}", o.foe.0, e.npc_id, if owned { "his ally body" } else { "the leader body (the player's)" }, o.why.name(), o.to.name());
                    } else {
                        st.refreshed += 1;
                    }
                }
                Err(why) => {
                    st.failed += 1;
                    if st.failed <= 20 {
                        erlog!("aggro: the hate hit on target {} could not be fired ({why})", o.foe.0);
                    }
                }
            }
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::rules::*;
    use super::*;
    use kh_party::donald::aggro::AggroCtx;
    use kh_party::donald::{DonAbilities, DonAbility};

    const SORA: PartyChara = PartyChara::Sora;
    const DONALD: PartyChara = PartyChara::Donald;
    const GOOFY: PartyChara = PartyChara::Goofy;

    fn party(leader: PartyChara) -> Vec<Member> {
        // a formation: the leader in front, the two others 2 m behind to the sides
        [(SORA, Vec3::new(0.0, 0.0, 0.0)), (DONALD, Vec3::new(-200.0, 150.0, 0.0)), (GOOFY, Vec3::new(-200.0, -150.0, 0.0))]
            .into_iter()
            .map(|(c, p)| Member { chara: c, pos: p, leader: c == leader, up: true, casting: false })
            .collect()
    }

    /// Enemies in a ring of `r` cm round the party, facing it.
    fn ring(n: u32, r: f32) -> Vec<Foe> {
        (0..n)
            .map(|k| {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                let pos = Vec3::new(a.cos() * r - 100.0, a.sin() * r, 0.0);
                let yaw = (-pos.y).atan2(-100.0 - pos.x).to_degrees();
                Foe { id: TargetId(100 + k), pos, yaw, hp_max: 1000.0 }
            })
            .collect()
    }

    fn don(list: &[DonAbility], zetta: bool) -> DonIn {
        let mut a = DonAbilities::default();
        for k in list {
            a.give(*k);
            a.set_equipped(*k, true);
        }
        DonIn { abilities: a, ctx: AggroCtx { others: 2, nearest_ally: 250.0, still: false, casting: false }, zetta }
    }

    /// Runs the director for `secs` at 60 fps; returns each member's share of enemy-seconds and the switches.
    fn run(members: &[Member], foes: &[Foe], secs: f32, mut each: impl FnMut(f32) -> (Vec<Hit>, Vec<TauntIn>, Option<PartyChara>, Option<DonIn>)) -> (HashMap<PartyChara, f32>, rules::Director, Vec<Order>) {
        let s = Sheet::default();
        let mut d = rules::Director::default();
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        let mut time: HashMap<PartyChara, f32> = HashMap::new();
        let mut orders = Vec::new();
        while t < secs {
            let (hits, taunts, guard, donald) = each(t);
            let i = Input { now: t, members, foes, hits: &hits, taunts: &taunts, guard, donald };
            let o = d.step(&s, &i);
            if let Some(first) = o.first() {
                d.fired(first.foe, t);
                orders.push(*first);
            }
            for c in d.choices.values() {
                *time.entry(c.to).or_insert(0.0) += dt;
            }
            t += dt;
        }
        let all: f32 = time.values().sum();
        (time.into_iter().map(|(c, v)| (c, v / all)).collect(), d, orders)
    }

    #[test]
    fn aggro_the_sheet_rows_are_all_understood_and_are_the_defaults() {
        let (s, bad) = Sheet::from_text(include_str!("../../../../sheets/port/party-er-aggro.json")).unwrap();
        assert!(bad.is_empty(), "rows not understood: {bad:?}");
        assert_eq!(s, Sheet::default());
    }

    /// No ability in play: every member is attacked regularly, the leader keeps a fair share (at least a third, not
    /// all), and nobody flip-flops (a switch at most every `hold_min` s per enemy on average).
    #[test]
    fn aggro_without_abilities_the_attention_is_spread_and_the_leader_keeps_a_fair_share() {
        for leader in [SORA, DONALD, GOOFY] {
            let members = party(leader);
            let foes = ring(6, 600.0);
            let (share, d, _) = run(&members, &foes, 300.0, |_| (Vec::new(), Vec::new(), None, None));
            println!("AGGRO shares, {} leading: {}; {} switches / 6 enemies / 5 min", leader.name(), members.iter().map(|m| format!("{} {:.2}", m.chara.name(), share.get(&m.chara).copied().unwrap_or(0.0))).collect::<Vec<_>>().join(", "), d.switches);
            for m in &members {
                let v = share.get(&m.chara).copied().unwrap_or(0.0);
                assert!(v >= 0.15, "{leader:?} leading: {} has {v:.2} of the attention: {share:?}", m.chara.name());
            }
            let l = share[&leader];
            assert!((0.33..=0.7).contains(&l), "{leader:?} leading: the leader's share {l:.2}: {share:?}");
            let per_enemy_minute = d.switches as f32 / foes.len() as f32 / 5.0;
            assert!(per_enemy_minute <= 60.0 / Sheet::default().hold_min, "{per_enemy_minute} switches per enemy per minute");
            assert!(per_enemy_minute >= 2.0, "never a permanent lock: {per_enemy_minute}");
        }
    }

    /// Who hurts an enemy draws it: Goofy pounding every enemy takes most of them; Donald's casting draws more too.
    #[test]
    fn aggro_damage_and_casting_draw_attention() {
        let members = party(SORA);
        let foes = ring(6, 600.0);
        let (base, ..) = run(&members, &foes, 300.0, |_| (Vec::new(), Vec::new(), None, None));
        let goofy_hits = |t: f32| foes.iter().map(|f| Hit { foe: f.id, by: GOOFY, damage: 30.0, at: t }).collect::<Vec<_>>();
        let (hurt, ..) = run(&members, &foes, 300.0, |t| (if (t * 60.0).round() as u32 % 60 == 0 { goofy_hits(t) } else { Vec::new() }, Vec::new(), None, None));
        assert!(hurt[&GOOFY] > base[&GOOFY] + 0.1, "Goofy's damage: {:.2} -> {:.2}", base[&GOOFY], hurt[&GOOFY]);
        let mut casting = party(SORA);
        casting[1].casting = true;
        let (cast, ..) = run(&casting, &foes, 300.0, |_| (Vec::new(), Vec::new(), None, None));
        assert!(cast[&DONALD] > base[&DONALD] + 0.05, "Donald casting: {:.2} -> {:.2}", base[&DONALD], cast[&DONALD]);
    }

    /// A member far away (beyond the reach), KO'd or without a body is never chosen; alone, the leader takes them all.
    #[test]
    fn aggro_only_members_that_can_be_attacked_are_chosen() {
        let mut members = party(SORA);
        members[1].up = false; // Donald KO / no body
        members[2].pos = Vec3::new(9000.0, 0.0, 0.0); // Goofy 90 m away
        let foes = ring(4, 600.0);
        let (share, ..) = run(&members, &foes, 60.0, |_| (Vec::new(), Vec::new(), None, None));
        assert_eq!(share.get(&SORA).copied(), Some(1.0), "{share:?}");
    }

    /// Over Here!: every enemy in its reach goes to Goofy at once and stays while it lasts (no order: taunt.rs fires);
    /// the credit rule sends every party hit's hate on them to him.
    #[test]
    fn aggro_over_here_holds_the_enemies_in_reach_on_goofy() {
        let members = party(SORA);
        let foes = ring(6, 600.0);
        let taunt = TauntIn { by: GOOFY, centre: Vec3::new(-200.0, -150.0, 0.0), radius: 1000.0 };
        let (share, d, orders) = run(&members, &foes, 20.0, |t| (Vec::new(), if t >= 5.0 { vec![taunt] } else { Vec::new() }, None, None));
        assert!(d.choices.values().all(|c| c.to == GOOFY && c.why == Why::Taunt));
        assert!(share[&GOOFY] > 0.7, "{share:?}");
        assert!(orders.iter().all(|o| o.why != Why::Taunt));
        let c = CreditIn { choice: Some((GOOFY, Why::Taunt)), leader: SORA, don_weight: 1.0, hp: 1000, er_damage: 20 };
        assert_eq!(credit_of(&c, Some(DONALD), 2.0, 0.5), Credit::To(GOOFY));
        assert_eq!(credit_of(&c, Some(SORA), 2.0, 0.5), Credit::To(GOOFY), "the leader's own hit too");
        assert_eq!(credit_of(&c, Some(GOOFY), 2.0, 0.5), Credit::Own);
        assert_eq!(credit_of(&CreditIn { hp: 30, ..c }, Some(DONALD), 2.0, 0.5), Credit::Leader, "a hit that may kill is the player's");
    }

    /// Donald's abilities: Low Profile lowers his share; Out of Sight (still, casting, enemies past 15 m) and Zettaflare
    /// take him out; Taunt Pass gives the enemies near him to the guarding ally; Distraction moves them to his nearest ally.
    #[test]
    fn aggro_donalds_abilities_are_obeyed() {
        let members = party(SORA);
        let foes = ring(8, 600.0);
        let (none, ..) = run(&members, &foes, 400.0, |_| (Vec::new(), Vec::new(), None, Some(don(&[], false))));
        let (lp, ..) = run(&members, &foes, 400.0, |_| (Vec::new(), Vec::new(), None, Some(don(&[DonAbility::LowProfile, DonAbility::HideBehind], false))));
        assert!(lp[&DONALD] < none[&DONALD] - 0.05, "Low Profile + Hide Behind: {:.2} -> {:.2}", none[&DONALD], lp[&DONALD]);
        let (zetta, ..) = run(&members, &foes, 60.0, |_| (Vec::new(), Vec::new(), None, Some(don(&[], true))));
        assert_eq!(zetta.get(&DONALD).copied().unwrap_or(0.0), 0.0, "Zettaflare: {zetta:?}");
        let mut quiet = don(&[DonAbility::OutOfSight], false);
        quiet.ctx.still = true;
        quiet.ctx.casting = true;
        let far = ring(6, 2500.0);
        let (oos, ..) = run(&members, &far, 60.0, |_| (Vec::new(), Vec::new(), None, Some(quiet)));
        assert_eq!(oos.get(&DONALD).copied().unwrap_or(0.0), 0.0, "Out of Sight at 25 m: {oos:?}");
        let (tp, d, _) = run(&members, &foes, 120.0, |_| (Vec::new(), Vec::new(), Some(GOOFY), Some(don(&[DonAbility::TauntPass], false))));
        assert_eq!(tp.get(&DONALD).copied().unwrap_or(0.0), 0.0, "Taunt Pass with Goofy guarding: {tp:?}");
        assert!(d.choices.values().any(|c| c.why == Why::TauntPass));
        let (ds, d, _) = run(&members, &foes, 400.0, |_| (Vec::new(), Vec::new(), None, Some(don(&[DonAbility::Distraction], false))));
        assert!(ds[&DONALD] < none[&DONALD], "Distraction: {:.2} -> {:.2}", none[&DONALD], ds[&DONALD]);
        let _ = d;
        // Donald's own hits on an enemy that should not choose him: credited to its choice by his weight's complement
        let c = CreditIn { choice: Some((GOOFY, Why::Natural)), leader: SORA, don_weight: 0.0, hp: 1000, er_damage: 20 };
        assert_eq!(credit_of(&c, Some(DONALD), 2.0, 0.3), Credit::To(GOOFY));
        assert_eq!(credit_of(&CreditIn { don_weight: 1.0, ..c }, Some(DONALD), 2.0, 0.3), Credit::Own, "no ability: his own");
        assert_eq!(credit_of(&CreditIn { choice: Some((SORA, Why::Natural)), ..c }, Some(DONALD), 2.0, 0.3), Credit::Leader);
        assert_eq!(credit_of(&CreditIn { don_weight: 1.0, ..c }, Some(GOOFY), 2.0, 0.3), Credit::Own, "a natural choice never moves a hit's hate");
    }

    /// The choice holds (no flip-flop) and is re-made early only by a hard hit from another member, never sooner than
    /// `switch_min`; the hate hits: one when the choice is new, then every `refresh` s.
    #[test]
    fn aggro_choices_hold_switch_on_provocation_and_refresh_their_hate_hit() {
        let s = Sheet::default();
        let members = party(SORA);
        let foes = [Foe { id: TargetId(1), pos: Vec3::new(500.0, 0.0, 0.0), yaw: 180.0, hp_max: 1000.0 }];
        let mut d = rules::Director::default();
        let i = |t: f32, hits: &'static [Hit]| Input { now: t, members: &members, foes: &foes, hits, taunts: &[], guard: None, donald: None };
        let o = d.step(&s, &i(0.0, &[]));
        assert_eq!(o.len(), 1);
        assert!(o[0].first);
        let first = d.choice(TargetId(1)).unwrap();
        d.fired(TargetId(1), 0.0);
        // held: no order until the refresh, the same member
        let mut t = 0.0;
        while t < first.hold_until - 0.05 {
            let o = d.step(&s, &i(t, &[]));
            assert_eq!(d.choice(TargetId(1)).unwrap().to, first.to);
            if let Some(o) = o.first() {
                assert!(!o.first && t >= s.refresh - 1e-3, "a refresh at {t}");
                d.fired(TargetId(1), t);
            }
            t += 1.0 / 60.0;
        }
        // a provocation: someone else deals 10 % of its HP; re-made at once (the hold had lasted past switch_min)
        let other = if first.to == GOOFY { DONALD } else { GOOFY };
        let hard: &'static [Hit] = Box::leak(vec![Hit { foe: TargetId(1), by: other, damage: 100.0, at: t }].into_boxed_slice());
        let mut d2 = d.clone();
        if let Some(c) = d2.choices.get_mut(&TargetId(1)) {
            c.hold_until = t + 100.0;
            c.since = t - s.switch_min - 0.1;
        }
        let before = d2.choice(TargetId(1)).unwrap().decisions;
        d2.step(&s, &i(t, hard));
        assert_eq!(d2.choice(TargetId(1)).unwrap().decisions, before + 1, "re-made after a hard hit");
        // but not within switch_min of the last switch
        let mut d3 = d.clone();
        if let Some(c) = d3.choices.get_mut(&TargetId(1)) {
            c.hold_until = t + 100.0;
            c.since = t - 0.5;
        }
        let before = d3.choice(TargetId(1)).unwrap().decisions;
        d3.step(&s, &i(t, hard));
        assert_eq!(d3.choice(TargetId(1)).unwrap().decisions, before, "never within switch_min");
    }

    /// Frame-rate independence: the shares at 30 and 144 fps are those at 60 within a few points.
    #[test]
    fn aggro_the_director_is_frame_rate_independent() {
        let members = party(SORA);
        let foes = ring(6, 600.0);
        let share_at = |fps: f32| {
            let s = Sheet::default();
            let mut d = rules::Director::default();
            let mut time: HashMap<PartyChara, f32> = HashMap::new();
            let mut t = 0.0;
            while t < 600.0 {
                let i = Input { now: t, members: &members, foes: &foes, hits: &[], taunts: &[], guard: None, donald: None };
                if let Some(o) = d.step(&s, &i).first() {
                    d.fired(o.foe, t);
                }
                for c in d.choices.values() {
                    *time.entry(c.to).or_insert(0.0) += 1.0 / fps;
                }
                t += 1.0 / fps;
            }
            let all: f32 = time.values().sum();
            time.into_iter().map(|(c, v)| (c, v / all)).collect::<HashMap<_, _>>()
        };
        let (a, b, c) = (share_at(30.0), share_at(60.0), share_at(144.0));
        for m in [SORA, DONALD, GOOFY] {
            assert!((a[&m] - b[&m]).abs() < 0.08 && (c[&m] - b[&m]).abs() < 0.08, "{m:?}: {:.2} / {:.2} / {:.2}", a[&m], b[&m], c[&m]);
        }
    }
}
