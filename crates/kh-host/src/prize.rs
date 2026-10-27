//! (prize) THE PRIZE ORBS OF A HOST: one `kh_prize::Prizes`, stepped against the host's world with
//! Sora as the getter; the sound and status half of its events goes through the core's seams
//! (`kh_prize::host::apply_to_sora`), the picture half through the host's effect side as FREE
//! effects (one particle system per orb: the prize blueprint's own, moved with the orb, shrunk
//! while it flies to him, stopped when it is collected or vanishes).
//!
//! Both hosts use this (the test app and the Elden Ring mod); they differ only in WHEN they ask
//! for a drop and in the effect side they pass to `show`.

use std::collections::HashMap;

use glam::Vec3;
use kh_core::{Sora, World};
use kh_prize::{host as prize_host, Getter, PrizeClass, PrizeEvent, Prizes};

/// The four calls of an effect side that draws things the core does not own
/// (`crate::vfx::EffectHost::free_*`, the test app's `Vfx::free_*`).
pub trait FreeEffects {
    fn free_spawn(&mut self, name: &str, position: Vec3, yaw: f32, scale: f32) -> Option<u64>;
    fn free_move(&mut self, id: u64, position: Vec3, yaw: f32, scale: f32);
    fn free_stop(&mut self, id: u64, immediate: bool);
    /// (IMPL_ER19) A factor on everything the effect draws (its colour multiplier: emissive and base colour alike);
    /// 1 = as authored. An effect side that cannot do it draws as authored.
    fn free_gain(&mut self, _id: u64, _gain: f32) {}
    /// (LDI) The effect as if it had been running for `seconds` (a looping idle effect first drawn full). An effect side
    /// that cannot do it starts it from nothing as before.
    fn free_prewarm(&mut self, _id: u64, _seconds: f32) {}
}

impl FreeEffects for crate::vfx::EffectHost {
    fn free_spawn(&mut self, name: &str, position: Vec3, yaw: f32, scale: f32) -> Option<u64> {
        crate::vfx::EffectHost::free_spawn(self, name, position, yaw, scale)
    }

    fn free_move(&mut self, id: u64, position: Vec3, yaw: f32, scale: f32) {
        crate::vfx::EffectHost::free_move(self, id, position, yaw, scale)
    }

    fn free_stop(&mut self, id: u64, immediate: bool) {
        crate::vfx::EffectHost::free_stop(self, id, immediate)
    }

    fn free_gain(&mut self, id: u64, gain: f32) {
        crate::vfx::EffectHost::free_gain(self, id, gain)
    }

    fn free_prewarm(&mut self, id: u64, seconds: f32) {
        crate::vfx::EffectHost::free_prewarm(self, id, seconds)
    }
}

/// Counters for a host's log line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrizeStats {
    pub spawned: u64,
    pub collected: u64,
    pub expired: u64,
    pub bounces: u64,
    /// Orbs whose effect is not in the pack (nothing is drawn for them).
    pub undrawn: u64,
    /// (IMPL_ER19) Munny that vanished with uncollected orbs (KH3: lost), and munny of vanished orbs that was
    /// paid all the same (`PrizeHost::pay_expired_munny`).
    pub munny_lost: i64,
    pub munny_paid_late: i64,
    /// (loot) Item boxes that arrived, and those among them whose item is not in the item table (nothing given).
    pub items: u64,
    pub unknown_items: u64,
}

/// (IMPL_ER19) What an orb did before it was collected or vanished, for a host's log (where it burst out, where it
/// first met the ground, where it came to rest: the look of the throw in a world nobody could watch).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Track {
    spawn: Vec3,
    first_bounce: Option<(f32, Vec3)>,
    rest: Option<(f32, Vec3)>,
    lowest: f32,
}

pub struct PrizeHost {
    pub prizes: Prizes,
    /// Prize id -> its free effect.
    effects: HashMap<u32, u64>,
    /// Events whose picture half is still to do.
    pending: Vec<PrizeEvent>,
    pub stats: PrizeStats,
    /// One line per event worth telling (spawn, get, vanish), for the host's console / log.
    pub log: Vec<String>,
    /// (IMPL_ER19) A factor on the brightness of the orbs' effects (1 = as authored; `FreeEffects::free_gain`).
    pub glow: f32,
    /// (IMPL_ER19) A factor on the SIZE of the orbs' effects (1 = the blueprints' component scales: 0.5 / 0.75 / 1);
    /// the picture only: the orb's own sphere against the world and its flight are untouched.
    pub size: f32,
    /// (IMPL_ER19) A HOST CHOICE, not KH3 (whose uncollected orbs are simply lost): the munny of a munny orb that
    /// vanishes uncollected is given all the same (a host whose munny is another game's currency).
    pub pay_expired_munny: bool,
    /// (IMPL_ER19) Each orb's flight is told in `log` when it ends (a line per orb).
    pub log_flight: bool,
    tracks: HashMap<u32, Track>,
    /// Sora's place at the latest step (the flight lines are told relative to him).
    getter: Vec3,
    glow_shown: f32,
    /// (party P2) The party as the latest `step_party` gave it (getter slots); empty = Sora alone.
    members: Vec<PrizeMember>,
    /// (party P2) The random pick of Share Prizes (0x14147A070 draws from the engine's xorshift128; the port's own).
    share_rng: kh_prize::Rng,
    /// (MUNNYSE, the user 2026-10-27: "make picking up munny from the world give a sound") A HOST THAT SOUNDS THE MUNNY
    /// ORBS' GET ITSELF: their get sound (KH3's `m_SoundGet`, the moment the orb is 0.2 s from him) is not queued on Sora
    /// (his next step's seam) but put in `munny_sounds` (class, place) for the host to play now. False (the default):
    /// through Sora as every other orb sound.
    pub host_munny_sound: bool,
    pub munny_sounds: Vec<(PrizeClass, Vec3)>,
}

/// (party P2) ONE PARTY MEMBER AS KH3'S PRIZE CODE SEES HIM (a getter slot of `PrizeHost::step_party`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrizeMember {
    /// For the host's log ("Sora", "Donald", "Goofy").
    pub name: &'static str,
    /// His point, whether he can take a prize at all (alive), his Treasure Magnet radius (`Getter::sora` for Sora;
    /// a friend's from HIS OWN PRIZE_DRAW count: FindGetter 0x1414794C0 reads vt+0x1500(0x29) of each candidate).
    pub getter: Getter,
    /// This slot is Sora's status (the core applies his prizes: `apply_to_sora`).
    pub sora: bool,
    /// The player pawn (FUN_140E57EE0): the controlled leader.
    pub player: bool,
    /// A friend (an NPC pawn): his CanTakePrize is 0x141259A50 (no item boxes, kinds 9..16; focus 7 / 8 only with
    /// SHARE_PRIZE; nothing in the states of `refuses`).
    pub friend: bool,
    /// SHARE_PRIZE (198, Goofy's "Share Prizes") equipped.
    pub share_prize: bool,
    /// A friend in a state that takes no prize (0x141259A50's list: Die 59, Hidden 140, the team attacks' FR states,
    /// the attractions).
    pub refuses: bool,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
}

/// (party P2) Where a host applies an HP / MP prize that KH3 gives to a FRIEND (`ApplyPrize` 0x1405AB950 on his pawn:
/// HP +1 / +5 by `Recover(.., kind 2)`; MP +3 / +5, or while his MP charges the charge time -2 / -6 s).
pub trait PrizeParty {
    fn apply(&mut self, slot: usize, kind: kh_core::status::PrizeKind);
}

/// (party P2) CanTakePrize of a getter slot: Sora's as `Getter::sora` (alive: 0x14142C600); a friend's 0x141259A50.
/// (PU, the user 2026-10-18: "make it a universal thing that characters can pick up items, even allies who aren't party
/// leader can pick up items. The inventory is shared") ITEM BOXES (kinds 9 .. 16) ARE TAKEN BY EVERY MEMBER: KH3's
/// 0x141259A50 refuses them to a friend, which left a friend LEADER (and a party without Sora) unable to collect any box.
/// OUR RULE: any member that can take a prize takes a box, leader or AI ally, whoever he is (no name list); the box's
/// item goes to the one shared stock through [`crate::items::give`] (the HUD's item popup), whoever collected it. KH3's
/// other refusals stand: a friend in a refusing state (KO, the team attacks' FR states) takes nothing, focus orbs only
/// with Share Prizes.
pub fn member_takes(m: &PrizeMember, class: PrizeClass) -> bool {
    if !m.getter.can_take {
        return false;
    }
    if !m.friend {
        return true;
    }
    let kind = class.kind_value();
    !m.refuses && (m.share_prize || !(7..=8).contains(&kind))
}

/// (party P2) WHO GETS AN ORB the getter `slot` collected: 0x14147A070 (verified at instruction level).
/// - kinds 7 .. 16 (focus; boxes do not come here): the player pawn (FUN_140E57EE0) - IN THE PORT Sora's status, the
///   focus gauge's only owner (the friends have none; OUR DESIGN for a friend leader, decisions dec028);
/// - munny: the getter (the game's munny is global: the port's is Sora's status);
/// - HP / MP without SHARE_PRIZE on the getter: the getter;
/// - with it: the getter while he is not full (HP: vt+0x17B0 < vt+0x1820; MP: vt+0x17D8 < vt+0x1828); else the player
///   pawn when his HP is above 0 and he is not full; else one of the party slots that is alive and not full, drawn at
///   random (`(int)(unit x n)`, at most n - 1); with none, the getter (or the full player) all the same.
pub fn prize_recipient(members: &[PrizeMember], slot: usize, kind: kh_core::status::PrizeKind, rng: &mut kh_prize::Rng) -> usize {
    use kh_core::status::PrizeKind as K;
    let mp = match kind {
        K::HpSmall | K::HpLarge => false,
        K::MpSmall | K::MpLarge => true,
        _ => return slot,
    };
    let Some(getter) = members.get(slot) else { return slot };
    if !getter.share_prize {
        return slot;
    }
    let full = |m: &PrizeMember| if mp { m.mp >= m.mp_max } else { m.hp >= m.hp_max };
    let mut pick = slot;
    if full(getter) {
        if let Some(p) = members.iter().position(|m| m.player).filter(|p| members[*p].hp > 0) {
            pick = p;
        }
    }
    if !full(&members[pick]) {
        return pick;
    }
    let open: Vec<usize> = members.iter().enumerate().filter(|(_, m)| !m.player && m.hp > 0 && !full(m)).map(|(i, _)| i).collect();
    if open.is_empty() {
        return pick;
    }
    let i = ((rng.unit() * open.len() as f32) as usize).min(open.len() - 1);
    open[i]
}

/// (MUNNYSE) The get sound of a munny orb.
fn is_get_of_munny(event: &PrizeEvent) -> bool {
    matches!(*event, PrizeEvent::GetSound { class, .. } if is_munny(class))
}

fn is_munny(class: PrizeClass) -> bool {
    matches!(class, PrizeClass::Orb(kh_core::status::PrizeKind::Munny1 | kh_core::status::PrizeKind::Munny5 | kh_core::status::PrizeKind::Munny10))
}

fn describe(class: PrizeClass) -> &'static str {
    prize_host::class_name(class)
}

impl PrizeHost {
    pub fn new(seed: u32) -> PrizeHost {
        PrizeHost { prizes: Prizes::new(seed), effects: HashMap::new(), pending: Vec::new(), stats: PrizeStats::default(), log: Vec::new(), glow: 1.0, size: 1.0, pay_expired_munny: false, log_flight: false, tracks: HashMap::new(), getter: Vec3::ZERO, glow_shown: 1.0, members: Vec::new(), share_rng: kh_prize::Rng::new(seed ^ 0x5348_4152), host_munny_sound: false, munny_sounds: Vec::new() /* (MUNNYSE) */ }
    }

    /// ONE SIMULATION STEP, after Sora's: the orbs move against `world`; bounce and get sounds and
    /// collected prizes are queued on `sora` (they are in his NEXT step's events).
    pub fn step(&mut self, dt: f32, world: &dyn World, sora: &mut Sora, capsule_center: Vec3) {
        if self.prizes.is_empty() && self.pending.is_empty() {
            return;
        }
        let getter = Getter::sora(sora, capsule_center);
        self.getter = capsule_center;
        self.members.clear();
        self.prizes.step(dt, world, &getter);
        self.take(sora);
        self.after_step();
    }

    /// (party P2) ONE STEP WITH THE PARTY AS GETTERS (KH3's FindGetter over the player and his party slots, the
    /// friends' CanTakePrize, Share Prizes' redirect, `ApplyPrize` on the one who gets it): `members` in slot order (each
    /// step the host's current party; Sora's slot, if any, with `sora`), `party` applies a friend's HP / MP prize.
    /// Munny, focus and item boxes go where they go with Sora alone (`take`). `sora_point`: where the flight lines are
    /// told from (the player's centre).
    pub fn step_party(&mut self, dt: f32, world: &dyn World, sora: &mut Sora, members: &[PrizeMember], sora_point: Vec3, party: &mut dyn PrizeParty) {
        if self.prizes.is_empty() && self.pending.is_empty() {
            return;
        }
        self.getter = sora_point;
        self.members = members.to_vec();
        let getters: Vec<Getter> = members.iter().map(|m| m.getter).collect();
        self.prizes.step_party(dt, world, &getters, &|slot, class| members.get(slot).is_some_and(|m| member_takes(m, class)));
        self.take_with(sora, Some(party));
        self.after_step();
    }

    fn after_step(&mut self) {
        if self.log_flight {
            for prize in self.prizes.list() {
                if let Some(track) = self.tracks.get_mut(&prize.id) {
                    track.lowest = track.lowest.min(prize.position.z);
                    if track.rest.is_none() && prize.resting() {
                        track.rest = Some((prize.age, prize.position));
                    }
                }
            }
        }
    }

    /// (loot) How many of its item a box holds (`kh_prize::Prizes::set_amount`).
    pub fn set_amount(&mut self, id: u32, amount: i32) -> bool {
        self.prizes.set_amount(id, amount)
    }

    /// The events of a drop the host just asked for (so that the orbs are drawn from their first frame).
    pub fn take(&mut self, sora: &mut Sora) {
        self.take_with(sora, None);
    }

    /// (party P2) `take` with the party: an HP / MP orb goes to the member KH3 gives it to (`prize_recipient`).
    fn take_with(&mut self, sora: &mut Sora, mut party: Option<&mut dyn PrizeParty>) {
        for event in self.prizes.take_events() {
            // (loot) A BOX THAT ARRIVED: its item goes through the hosts' one door (the HUD stocks it and shows the
            // game's item popup); everything else through the core's seams.
            if let PrizeEvent::Collected { class: PrizeClass::Box(_, item), amount, .. } = event {
                if !crate::items::give(item, amount.max(1) as u32) {
                    self.stats.unknown_items += 1;
                }
                self.stats.items += 1;
            } else if let (PrizeEvent::Collected { class: PrizeClass::Orb(kind), getter, .. }, Some(party)) = (event, party.as_deref_mut()) {
                // (party P2) the getter, or whom Share Prizes passes it to
                let to = prize_recipient(&self.members, getter as usize, kind, &mut self.share_rng);
                match self.members.get(to) {
                    Some(m) if !m.sora && !matches!(kind, kh_core::status::PrizeKind::Munny1 | kh_core::status::PrizeKind::Munny5 | kh_core::status::PrizeKind::Munny10 | kh_core::status::PrizeKind::FocusSmall | kh_core::status::PrizeKind::FocusLarge) => {
                        party.apply(to, kind);
                        if to != getter as usize && self.log.len() < 256 {
                            self.log.push(format!("Share Prizes: {} passes it to {}", self.members.get(getter as usize).map_or("?", |g| g.name), m.name));
                        }
                    }
                    _ => {
                        prize_host::apply_to_sora(sora, &event);
                    }
                }
            } else if let (PrizeEvent::GetSound { class, position, .. }, true) = (event, self.host_munny_sound && is_get_of_munny(&event)) {
                // (MUNNYSE) a munny orb's get sound: the host plays it (bounded, so a burst does not pile up while nobody takes them)
                if self.munny_sounds.len() < 64 {
                    self.munny_sounds.push((class, position));
                }
            } else {
                prize_host::apply_to_sora(sora, &event);
            }
            match event {
                PrizeEvent::Spawned { id, position, .. } => {
                    self.stats.spawned += 1;
                    if self.log_flight && self.tracks.len() < 1024 {
                        self.tracks.insert(id, Track { spawn: position, first_bounce: None, rest: None, lowest: position.z });
                    }
                }
                PrizeEvent::Bounced { id, position, count, .. } => {
                    self.stats.bounces += 1;
                    if count == 1 {
                        if let (Some(track), Some(age)) = (self.tracks.get_mut(&id), self.prizes.get(id).map(|p| p.age)) {
                            track.first_bounce = Some((age, position));
                        }
                    }
                }
                PrizeEvent::Collected { id, class, amount, getter, .. } => {
                    self.stats.collected += 1;
                    self.tracks.remove(&id);
                    if self.log.len() < 256 {
                        // (party P2) by whom, when the party takes part
                        let by = self.members.get(getter as usize).map_or(String::new(), |m| format!(" by {}", m.name));
                        self.log.push(format!("prize {id} ({}) collected{by}{}", describe(class), if amount > 0 { format!(": worth {amount}") } else { String::new() }));
                    }
                }
                PrizeEvent::Expired { id, class, amount, position } => {
                    self.stats.expired += 1;
                    let munny = is_munny(class);
                    let mut what = String::new();
                    if munny && amount > 0 {
                        if self.pay_expired_munny {
                            // (the status path, like a collected orb: the HUD's munny popup comes with it; no get sound)
                            if let PrizeClass::Orb(kind) = class {
                                sora.status_request(kh_core::StatusRequest::PrizeCollected { kind, amount });
                            }
                            self.stats.munny_paid_late += amount as i64;
                            what = format!(": its {amount} munny are given all the same (a host choice)");
                        } else {
                            self.stats.munny_lost += amount as i64;
                            what = format!(": its {amount} munny are LOST (KH3's rule)");
                        }
                    }
                    let flight = self.tracks.remove(&id).map_or(String::new(), |t| format!("; it lay {:.0} cm from him, {:+.0} cm from his centre in height{}", (position - self.getter).truncate().length(), position.z - self.getter.z, if position.z < t.spawn.z - 1000.0 { " (IT FELL more than 10 m: no ground under it)" } else { "" }));
                    if self.log.len() < 256 {
                        self.log.push(format!("prize {id} ({}) vanished uncollected{what}{flight}", describe(class)));
                    }
                }
                PrizeEvent::GetStarted { id, class, getter } => {
                    if self.log.len() < 256 {
                        let flight = match (self.tracks.get(&id), self.prizes.get(id)) {
                            (Some(t), Some(p)) => format!(
                                " from {:.0} cm away ({:+.0} cm in height), {:.2} s old, {} bounce(s){}{}; it burst out {:+.0} cm from his centre in height and fell as low as {:+.0}",
                                (p.position - self.getter).truncate().length(),
                                p.position.z - self.getter.z,
                                p.age,
                                p.bounces,
                                t.first_bounce.map_or(String::new(), |(age, at)| format!(", first ground contact at {age:.2} s {:+.0} cm from his centre in height", at.z - self.getter.z)),
                                t.rest.map_or(String::new(), |(age, _)| format!(", at rest since {age:.2} s")),
                                t.spawn.z - self.getter.z,
                                t.lowest - self.getter.z,
                            ),
                            _ => String::new(),
                        };
                        // (party P2) the getter by name when the party takes part
                        let who = self.members.get(getter as usize).map_or("Sora", |m| m.name);
                        self.log.push(format!("prize {id} ({}) flies to {who}{flight}", describe(class)));
                    }
                }
                PrizeEvent::GetSound { .. } => {}
            }
            if matches!(event, PrizeEvent::Spawned { .. } | PrizeEvent::Collected { .. } | PrizeEvent::Expired { .. }) {
                self.pending.push(event);
            }
        }
    }

    /// THE PICTURE HALF: starts the effect of every new orb, moves every orb's effect to where the
    /// orb is, stops the effects of the orbs that are gone. Call once per step or frame, before the
    /// effect side steps.
    pub fn show(&mut self, fx: &mut dyn FreeEffects) {
        // (the brightness factor was changed while orbs are out: they follow)
        if self.glow != self.glow_shown {
            self.glow_shown = self.glow;
            for effect in self.effects.values() {
                fx.free_gain(*effect, self.glow);
            }
        }
        for event in std::mem::take(&mut self.pending) {
            match event {
                PrizeEvent::Spawned { id, class, position } => {
                    let (name, scale) = prize_host::effect(class);
                    match fx.free_spawn(name, position, 0.0, scale * self.size) {
                        Some(effect) => {
                            if self.glow != 1.0 {
                                fx.free_gain(effect, self.glow);
                            }
                            self.effects.insert(id, effect);
                        }
                        None => self.stats.undrawn += 1,
                    }
                }
                PrizeEvent::Collected { id, .. } | PrizeEvent::Expired { id, .. } => {
                    if let Some(effect) = self.effects.remove(&id) {
                        // It stops emitting; what is in the air runs out as authored.
                        fx.free_stop(effect, false);
                    }
                }
                _ => {}
            }
        }
        for prize in self.prizes.list() {
            if let Some(&effect) = self.effects.get(&prize.id) {
                let (_, scale) = prize_host::effect(prize.class);
                fx.free_move(effect, prize.position, prize.yaw, scale * self.size * prize.scale.max(0.02));
            }
        }
    }

    /// Every orb and its effect gone at once (a level reset, the host shutting down).
    pub fn clear(&mut self, fx: &mut dyn FreeEffects) {
        for (_, effect) in self.effects.drain() {
            fx.free_stop(effect, true);
        }
        self.pending.clear();
        self.tracks.clear();
        self.prizes.clear();
    }

    /// (IMPL_ER19) Every orb gone WITHOUT an effect side at hand (their effects are the caller's to stop).
    pub fn forget(&mut self) {
        self.effects.clear();
        self.pending.clear();
        self.tracks.clear();
        self.prizes.clear();
    }

    /// (IMPL_ER20) EVERY ORB TAKEN BACK AT ONCE (the host can no longer let him collect them: his death, a cutscene,
    /// the host stopping): their effects stop immediately (with an effect side at hand; else they are the caller's),
    /// nothing is applied. Returns (orbs taken back, the munny the munny orbs among them held): what the host does
    /// with that munny is its rule.
    pub fn recall(&mut self, fx: Option<&mut dyn FreeEffects>) -> (usize, i64) {
        let out = (self.prizes.list().len(), self.munny_out());
        match fx {
            Some(fx) => self.clear(fx),
            None => self.forget(),
        }
        out
    }

    /// The munny still lying or flying in orbs.
    pub fn munny_out(&self) -> i64 {
        self.prizes.list().iter().filter(|p| is_munny(p.class)).map(|p| p.amount.max(0) as i64).sum()
    }

    pub fn summary(&self) -> String {
        let s = &self.stats;
        format!(
            "{} orb(s) out{} | spawned {} collected {} vanished {} bounces {}{}{}{}",
            self.prizes.list().len(),
            if self.munny_out() > 0 { format!(" holding {} munny", self.munny_out()) } else { String::new() },
            s.spawned,
            s.collected,
            s.expired,
            s.bounces,
            if s.munny_lost > 0 { format!(" | {} munny LOST with vanished orbs", s.munny_lost) } else { String::new() },
            if s.munny_paid_late > 0 { format!(" | {} munny of vanished orbs given all the same", s.munny_paid_late) } else { String::new() },
            if s.undrawn > 0 { format!(" | {} WITHOUT AN EFFECT in the pack", s.undrawn) } else { String::new() }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::status::PrizeKind;
    use kh_core::test_world::TestWorld;
    use kh_prize::SpawnMods;

    /// (party P2) A party member for the tests: at x cm on the floor, 300 cm magnet.
    fn member(name: &'static str, x: f32, sora: bool, friend: bool, share: bool, hp: (i32, i32), mp: (i32, i32)) -> PrizeMember {
        PrizeMember { name, getter: Getter { point: Vec3::new(x, 0.0, 90.0), can_take: hp.0 > 0, radius: Some(300.0) }, sora, player: sora, friend, share_prize: share, refuses: false, hp: hp.0, hp_max: hp.1, mp: mp.0, mp_max: mp.1 }
    }

    /// (party P2) CanTakePrize of a friend (0x141259A50): HP / MP / munny; focus only with Share Prizes; never a box;
    /// nothing while he refuses (Die, FR states); Sora takes everything while alive.
    #[test]
    fn friends_take_what_kh3_lets_them() {
        let donald = member("Donald", 0.0, false, true, false, (100, 100), (50, 50));
        let goofy = member("Goofy", 0.0, false, true, true, (100, 100), (50, 50));
        let sora = member("Sora", 0.0, true, false, false, (100, 100), (50, 50));
        let focus = PrizeClass::Orb(PrizeKind::FocusSmall);
        let item = PrizeClass::Box(kh_prize::BoxKind::Heal, 0x101);
        assert!(member_takes(&donald, PrizeClass::Orb(PrizeKind::HpSmall)) && member_takes(&donald, PrizeClass::Orb(PrizeKind::Munny5)));
        assert!(!member_takes(&donald, focus) && member_takes(&goofy, focus), "focus only with SHARE_PRIZE");
        // (PU) the user's rule (2026-10-18): every member takes an item box, leader or AI ally (KH3 refused it to friends)
        assert!(member_takes(&goofy, item) && member_takes(&donald, item) && member_takes(&sora, item), "every member takes a box");
        assert!(!member_takes(&PrizeMember { refuses: true, ..goofy }, PrizeClass::Orb(PrizeKind::HpSmall)));
        assert!(!member_takes(&PrizeMember { refuses: true, ..goofy }, item), "(PU) a KO'd / FR-state friend takes no box");
        assert!(!member_takes(&member("Goofy", 0.0, false, true, false, (0, 100), (0, 50)), item), "(PU) nor one at 0 HP");
    }

    /// (PU, the user 2026-10-18: "make it a universal thing that characters can pick up items, even allies who aren't
    /// party leader ... The inventory is shared") AN ITEM BOX GOES TO WHOEVER REACHES IT and its item to the one shared
    /// stock: (a) Goofy LEADING a party without Sora (the user's case: before, nobody could take it); (b) an AI ally
    /// (Donald) nearer than the leader Sora; (c) a future character no code names (a mocked "Riku" slot, AI).
    #[test]
    fn every_member_collects_item_boxes_into_the_shared_stock() {
        let _door = crate::items::TEST_DOOR.lock().unwrap_or_else(|e| e.into_inner());
        struct Nobody;
        impl PrizeParty for Nobody {
            fn apply(&mut self, _: usize, _: PrizeKind) {}
        }
        let world = TestWorld::floor();
        let leader = |m: PrizeMember| PrizeMember { player: true, ..m };
        let cases: [(&str, Vec<PrizeMember>); 3] = [
            ("Goofy", vec![leader(member("Goofy", 100.0, false, true, false, (100, 100), (0, 0)))]),
            ("Donald", vec![member("Sora", -2000.0, true, false, false, (100, 100), (50, 50)), member("Donald", 100.0, false, true, false, (100, 100), (50, 50))]),
            ("Riku", vec![leader(member("Donald", -2000.0, false, true, false, (100, 100), (50, 50))), member("Riku", 100.0, false, true, false, (100, 100), (50, 50))]),
        ];
        crate::items::take_given();
        for (who, party) in cases {
            let mut sora = sora_with_hp();
            let mut host = PrizeHost::new(9);
            let id = host.prizes.drop_item_box(Vec3::new(150.0, 0.0, 120.0), 0x101, false).unwrap();
            assert!(host.set_amount(id, 2));
            host.take(&mut sora);
            let mut log = Vec::new();
            for _ in 0..400 {
                host.step_party(1.0 / 60.0, &world, &mut sora, &party, party[0].getter.point, &mut Nobody);
                log.extend(host.log.drain(..));
                if host.prizes.is_empty() {
                    break;
                }
            }
            assert!(host.prizes.is_empty(), "{who}: the box was not collected: {log:?}");
            assert!(log.iter().any(|l| l.contains(&format!("flies to {who}"))), "{who}: {log:?}");
            assert_eq!((host.stats.items, host.stats.unknown_items), (1, 0), "{who}");
            assert_eq!(crate::items::take_given(), vec![(0x101, 2)], "{who}: the Potions went to the shared stock");
        }
    }

    /// (party P2) 0x14147A070: without Share Prizes the getter keeps it; with it a full getter passes HP / MP to the
    /// player, and when the player is full too, to a hurt party member; focus goes to Sora's status; munny stays.
    #[test]
    fn share_prizes_passes_on_what_the_getter_cannot_use() {
        let mut rng = kh_prize::Rng::new(1);
        let sora = member("Sora", 0.0, true, false, false, (80, 100), (50, 50));
        let donald = member("Donald", 0.0, false, true, false, (60, 100), (10, 50));
        let goofy = member("Goofy", 0.0, false, true, true, (100, 100), (50, 50));
        let party = [sora, donald, goofy];
        assert_eq!(prize_recipient(&party, 1, PrizeKind::HpLarge, &mut rng), 1, "Donald has no Share Prizes");
        assert_eq!(prize_recipient(&party, 2, PrizeKind::HpLarge, &mut rng), 0, "full Goofy -> the player");
        let full_sora = [PrizeMember { hp: 100, ..sora }, donald, goofy];
        assert_eq!(prize_recipient(&full_sora, 2, PrizeKind::HpLarge, &mut rng), 1, "player full -> the hurt friend");
        assert_eq!(prize_recipient(&party, 2, PrizeKind::MpSmall, &mut rng), 1, "MP: Sora is full, Donald is not");
        let hurt_goofy = [sora, donald, PrizeMember { hp: 50, ..goofy }];
        assert_eq!(prize_recipient(&hurt_goofy, 2, PrizeKind::HpSmall, &mut rng), 2, "a hurt getter keeps it");
        assert_eq!(prize_recipient(&party, 2, PrizeKind::Munny10, &mut rng), 2);
    }

    /// (party P2) FindGetter over the party: the nearest member inside his own radius gets the orb and it flies to
    /// HIM; an HP orb a friend gets heals the friend (PrizeParty::apply), not Sora.
    #[test]
    fn the_nearest_member_gets_the_orb() {
        struct Applied(Vec<(usize, PrizeKind)>);
        impl PrizeParty for Applied {
            fn apply(&mut self, slot: usize, kind: PrizeKind) {
                self.0.push((slot, kind));
            }
        }
        let world = TestWorld::floor();
        let mut sora = crate::savepoint::tests::sora99();
        let mut host = PrizeHost::new(3);
        host.prizes.drop_orbs(Vec3::new(500.0, 0.0, 100.0), PrizeKind::HpSmall, 1, &[], &SpawnMods::default(), false);
        host.take(&mut sora);
        let party = [member("Sora", -2000.0, true, false, false, (100, 100), (50, 50)), member("Donald", 500.0, false, true, false, (60, 100), (50, 50))];
        let mut applied = Applied(Vec::new());
        let mut log = Vec::new();
        for _ in 0..400 {
            host.step_party(1.0 / 60.0, &world, &mut sora, &party, party[0].getter.point, &mut applied);
            log.extend(host.log.drain(..));
            if host.prizes.is_empty() {
                break;
            }
        }
        assert!(log.iter().any(|l| l.contains("flies to Donald")), "{log:?}");
        assert_eq!(applied.0, vec![(1, PrizeKind::HpSmall)]);
    }

    #[derive(Default)]
    struct Fake {
        next: u64,
        live: HashMap<u64, (String, Vec3, f32)>,
        stopped: Vec<u64>,
        gains: Vec<(u64, f32)>,
    }

    impl FreeEffects for Fake {
        fn free_spawn(&mut self, name: &str, position: Vec3, _yaw: f32, scale: f32) -> Option<u64> {
            self.next += 1;
            self.live.insert(self.next, (name.to_string(), position, scale));
            Some(self.next)
        }

        fn free_move(&mut self, id: u64, position: Vec3, _yaw: f32, scale: f32) {
            if let Some(e) = self.live.get_mut(&id) {
                (e.1, e.2) = (position, scale);
            }
        }

        fn free_stop(&mut self, id: u64, _immediate: bool) {
            self.live.remove(&id);
            self.stopped.push(id);
        }

        fn free_gain(&mut self, id: u64, gain: f32) {
            self.gains.push((id, gain));
        }
    }

    /// (MUNNYSE) With `host_munny_sound` every munny orb's get sound comes to the host once (class, place), the munny is
    /// still paid through Sora's status; an HP orb's get sound is not the host's; without the switch nothing comes.
    #[test]
    fn munnyse_munny_get_sounds_go_to_the_host() {
        let world = TestWorld::floor();
        for own in [true, false] {
            let mut sora = sora_with_hp();
            let mut host = PrizeHost::new(5);
            host.host_munny_sound = own;
            let at = Vec3::new(150.0, 0.0, 200.0);
            for kind in [PrizeKind::Munny5, PrizeKind::Munny1, PrizeKind::Munny10, PrizeKind::HpSmall] {
                let amount = if kind == PrizeKind::HpSmall { 0 } else { 5 };
                host.prizes.spawn(at, PrizeClass::Orb(kind), Vec3::new(0.0, 60.0, 300.0), &SpawnMods::default(), false, amount);
            }
            host.take(&mut sora);
            for _ in 0..600 {
                host.step(1.0 / 60.0, &world, &mut sora, Vec3::new(0.0, 0.0, 91.0));
                sora.step(1.0 / 60.0, &kh_core::Input::default(), &world);
                if host.prizes.is_empty() {
                    break;
                }
            }
            assert!(host.prizes.is_empty(), "every orb was collected");
            let sounds = std::mem::take(&mut host.munny_sounds);
            if own {
                assert_eq!(sounds.len(), 3, "one per munny orb: {sounds:?}");
                assert!(sounds.iter().all(|(c, _)| is_munny(*c)));
            } else {
                assert!(sounds.is_empty());
            }
            assert_eq!(sora.status().munny, 15, "the munny is paid either way");
        }
    }

    /// A Sora with a real status (the level-99 loadout sheet), his munny at 0.
    fn sora_with_hp() -> Sora {
        let mut sora = crate::savepoint::tests::sora99();
        sora.status_mut().munny = 0;
        sora
    }

    /// (IMPL_ER19) A MUNNY ORB NOBODY COLLECTS: KH3's rule = it vanishes after its living time and its munny is lost
    /// (counted, said in the log); with the host's choice its munny is given all the same, once, through the status
    /// path. Either way the wallet rises only by what orbs (or that choice) bring: never at the drop.
    #[test]
    fn a_vanished_munny_orb_loses_its_munny_unless_the_host_pays_it() {
        let world = TestWorld::floor();
        for pay in [false, true] {
            let mut sora = sora_with_hp();
            let mut host = PrizeHost::new(7);
            host.pay_expired_munny = pay;
            host.log_flight = true;
            let mut fx = Fake::default();
            // 20 m from him: outside every magnet radius.
            host.prizes.spawn(Vec3::new(2000.0, 0.0, 150.0), PrizeClass::Orb(PrizeKind::Munny5), Vec3::new(0.0, 100.0, 300.0), &SpawnMods::default(), false, 77);
            host.take(&mut sora);
            host.show(&mut fx);
            assert_eq!((host.munny_out(), sora.status().munny), (77, 0));
            let centre = Vec3::new(0.0, 0.0, 91.0);
            for _ in 0..(17 * 60) {
                host.step(1.0 / 60.0, &world, &mut sora, centre);
                host.show(&mut fx);
                sora.step(1.0 / 60.0, &kh_core::Input::default(), &world);
            }
            assert!(host.prizes.is_empty() && fx.live.is_empty(), "it vanished and its effect ended");
            assert_eq!((host.stats.expired, host.stats.collected), (1, 0));
            assert_eq!(sora.status().munny, if pay { 77 } else { 0 });
            assert_eq!((host.stats.munny_lost, host.stats.munny_paid_late), if pay { (0, 77) } else { (77, 0) });
            assert!(host.log.iter().any(|l| l.contains("vanished uncollected") && l.contains("77 munny")), "{:?}", host.log);
        }
    }

    /// (IMPL_ER19) A collected munny orb carrying a host's amount raises the wallet by exactly that amount, when it
    /// ARRIVES (not when it is thrown), with the HUD's munny popup of the same amount; the stat orbs give KH3's values
    /// (HP +1 / +5, MP +3 / +5).
    #[test]
    fn collected_orbs_give_their_amounts_when_they_arrive() {
        let world = TestWorld::floor();
        let mut sora = sora_with_hp();
        (sora.status_mut().hp, sora.status_mut().mp) = (50, 40);
        let (hp_max, mp_max) = (sora.status().max_hp(), sora.status().max_mp());
        assert!(hp_max >= 60 && mp_max >= 50 && !sora.status().mp_charging, "the test needs room: {hp_max} {mp_max}");
        let mut host = PrizeHost::new(9);
        host.glow = 3.0;
        host.log_flight = true;
        let mut fx = Fake::default();
        let centre = Vec3::new(0.0, 0.0, 91.0);
        let at = Vec3::new(150.0, 0.0, 200.0);
        for (kind, amount) in [(PrizeKind::Munny1, 38), (PrizeKind::Munny5, 0), (PrizeKind::HpSmall, 0), (PrizeKind::HpLarge, 0), (PrizeKind::MpSmall, 0), (PrizeKind::MpLarge, 0)] {
            host.prizes.spawn(at, PrizeClass::Orb(kind), Vec3::new(0.0, 60.0, 300.0), &SpawnMods::default(), false, amount);
        }
        host.take(&mut sora);
        host.show(&mut fx);
        // (every orb's effect got the host's brightness factor)
        assert_eq!(fx.gains.len(), 6);
        // (and the host's size factor is on the picture alone: an HP orb's effect at 0.5 x 1)
        assert!(fx.live.values().any(|e| e.0 == "vs_plz_hpb0" && e.2 == 0.5) && fx.live.values().any(|e| e.0 == "VS_PLZ_MNY0" && e.2 == 0.75));
        assert!(fx.gains.iter().all(|g| g.1 == 3.0));
        let mut popups = Vec::new();
        let mut munny_while_out = Vec::new();
        for _ in 0..240 {
            host.step(1.0 / 60.0, &world, &mut sora, centre);
            host.show(&mut fx);
            let out = sora.step(1.0 / 60.0, &kh_core::Input::default(), &world);
            for e in &out.events {
                if let kh_core::EventKind::Popup(p) = &e.kind {
                    if p.kind == kh_core::UiPopup::KIND_MUNNY {
                        popups.push(p.value);
                    }
                }
            }
            if host.munny_out() > 0 {
                munny_while_out.push(sora.status().munny);
            }
        }
        assert!(host.prizes.is_empty());
        assert!(!munny_while_out.is_empty() && munny_while_out.iter().all(|m| *m == 0), "nothing before the orbs arrive");
        // 38 (the host's amount) + 5 (a KH3 medium munny orb)
        assert_eq!(sora.status().munny, 43);
        popups.sort_by(|a, b| a.total_cmp(b));
        assert_eq!(popups, vec![5.0, 38.0]);
        assert_eq!((sora.status().hp, sora.status().mp), (50 + 1 + 5, 40 + 3 + 5));
        assert!(host.log.iter().any(|l| l.contains("flies to Sora from")), "{:?}", host.log);
    }

    /// (IMPL_ER20) AN HP ORB CARRYING A HOST'S AMOUNT gives that many hit points (the Elden Ring host: +4 small,
    /// +20 large) instead of KH3's +1 / +5, capped at his maximum; without an amount KH3's values (the test above);
    /// an amount on an HP orb is not munny. And `recall`: every orb and its effect gone at once, nothing applied,
    /// the munny orbs' munny told to the host.
    #[test]
    fn an_hp_orb_gives_the_hosts_amount_and_recalled_orbs_tell_their_munny() {
        let world = TestWorld::floor();
        let mut sora = sora_with_hp();
        sora.status_mut().hp = 50;
        let hp_max = sora.status().max_hp();
        assert!(hp_max >= 80, "the test needs room: {hp_max}");
        let mut host = PrizeHost::new(11);
        let mut fx = Fake::default();
        let centre = Vec3::new(0.0, 0.0, 91.0);
        let at = Vec3::new(150.0, 0.0, 200.0);
        for (kind, amount) in [(PrizeKind::HpSmall, 4), (PrizeKind::HpLarge, 20)] {
            host.prizes.spawn(at, PrizeClass::Orb(kind), Vec3::new(0.0, 60.0, 300.0), &SpawnMods::default(), false, amount);
        }
        host.take(&mut sora);
        host.show(&mut fx);
        assert_eq!(host.munny_out(), 0, "an HP orb's amount is not munny");
        for _ in 0..240 {
            host.step(1.0 / 60.0, &world, &mut sora, centre);
            host.show(&mut fx);
            sora.step(1.0 / 60.0, &kh_core::Input::default(), &world);
        }
        assert!(host.prizes.is_empty());
        assert_eq!((sora.status().hp, sora.status().munny), (50 + 4 + 20, 0));
        // (capped at the maximum)
        sora.status_mut().hp = hp_max - 3;
        host.prizes.spawn(at, PrizeClass::Orb(PrizeKind::HpLarge), Vec3::new(0.0, 60.0, 300.0), &SpawnMods::default(), false, 20);
        host.take(&mut sora);
        for _ in 0..240 {
            host.step(1.0 / 60.0, &world, &mut sora, centre);
            sora.step(1.0 / 60.0, &kh_core::Input::default(), &world);
        }
        assert_eq!(sora.status().hp, hp_max);
        // ---- recall: two munny orbs (30 + 12), an HP orb with an amount, far from him
        let far = Vec3::new(3000.0, 0.0, 200.0);
        for (kind, amount) in [(PrizeKind::Munny1, 30), (PrizeKind::Munny10, 12), (PrizeKind::HpSmall, 4)] {
            host.prizes.spawn(far, PrizeClass::Orb(kind), Vec3::new(0.0, 60.0, 300.0), &SpawnMods::default(), false, amount);
        }
        host.take(&mut sora);
        host.show(&mut fx);
        assert_eq!((fx.live.len(), host.munny_out()), (3, 42));
        let (hp, munny) = (sora.status().hp, sora.status().munny);
        assert_eq!(host.recall(Some(&mut fx)), (3, 42));
        assert!(host.prizes.is_empty() && fx.live.is_empty() && host.munny_out() == 0);
        for _ in 0..60 {
            host.step(1.0 / 60.0, &world, &mut sora, centre);
            sora.step(1.0 / 60.0, &kh_core::Input::default(), &world);
        }
        assert_eq!((sora.status().hp, sora.status().munny), (hp, munny), "a recall applies nothing itself");
        assert_eq!(host.recall(None), (0, 0));
    }

    /// (loot) AN ITEM BOX THAT ARRIVES hands its item to the hosts' one door (`crate::items::give`), as many as it
    /// holds; its effect is the box of the item's category (the rare box for armour) and ends with it; nothing is
    /// asked of the core's status path for it.
    #[test]
    fn an_arrived_box_hands_its_item_to_the_door() {
        let world = TestWorld::floor();
        let mut sora = sora_with_hp();
        let mut host = PrizeHost::new(21);
        let mut fx = Fake::default();
        let centre = Vec3::new(0.0, 0.0, 91.0);
        let _door = crate::items::TEST_DOOR.lock().unwrap_or_else(|e| e.into_inner()); // (PU) the door's queue is shared
        crate::items::take_given();
        let armour = host.prizes.drop_item_box(Vec3::new(150.0, 0.0, 120.0), 0x404, false).unwrap();
        let shard = host.prizes.drop_item_box(Vec3::new(-150.0, 0.0, 120.0), 0x701, false).unwrap();
        assert!(host.set_amount(shard, 3));
        host.take(&mut sora);
        host.show(&mut fx);
        let names: Vec<&str> = [armour, shard].iter().map(|id| fx.live[&(*id as u64)].0.as_str()).collect();
        assert_eq!(names, ["vs_plz_prb01", "vs_plz_prb02"], "the rare box and the material box");
        for _ in 0..120 {
            host.step(1.0 / 60.0, &world, &mut sora, centre);
            host.show(&mut fx);
            sora.step(1.0 / 60.0, &kh_core::Input::default(), &world);
        }
        assert!(host.prizes.is_empty() && fx.live.is_empty());
        let mut given = crate::items::take_given();
        given.sort();
        assert_eq!(given, vec![(0x404, 1), (0x701, 3)]);
        assert_eq!((host.stats.items, host.stats.unknown_items), (2, 0));
    }

    /// An orb gets the effect of its blueprint at its scale, the effect follows it and shrinks on
    /// the way to Sora, and is stopped when the orb is collected.
    #[test]
    fn every_orb_has_one_effect_that_follows_it_and_ends_with_it() {
        let world = TestWorld::floor();
        let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&kh_core::SORA_PARAMS)), yaw: 0.0 });
        // (A Sora without a loadout has an empty status: give him the one hit point a getter needs.)
        sora.status_mut().hp = 1;
        let mut host = PrizeHost::new(4);
        let mut fx = Fake::default();
        let centre = Vec3::new(0.0, 0.0, 91.0);
        host.prizes.spawn(Vec3::new(120.0, 0.0, 200.0), PrizeClass::Orb(PrizeKind::HpSmall), Vec3::new(0.0, 100.0, 300.0), &SpawnMods::default(), false, 0);
        host.take(&mut sora);
        host.show(&mut fx);
        assert_eq!(fx.live.len(), 1);
        assert_eq!(fx.live[&1].0, "vs_plz_hpb0");
        assert_eq!(fx.live[&1].2, 0.5);
        let mut smallest = 1.0f32;
        for _ in 0..200 {
            host.step(1.0 / 60.0, &world, &mut sora, centre);
            host.show(&mut fx);
            if let (Some(p), Some(e)) = (host.prizes.list().first(), fx.live.get(&1)) {
                assert_eq!(e.1, p.position);
                smallest = smallest.min(e.2);
            }
        }
        assert!(host.prizes.is_empty() && fx.live.is_empty() && fx.stopped == vec![1]);
        assert!(smallest < 0.1, "it shrank on the way: {smallest}");
        assert_eq!((host.stats.spawned, host.stats.collected), (1, 1));
    }
}
