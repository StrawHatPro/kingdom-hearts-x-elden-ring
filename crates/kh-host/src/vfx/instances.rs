//! (MIMIC-FX, THE USER 2026-11-06: "all of the mimic tears (this goes for bosses and summons) need to have their VFX visible
//! and working") THE SORA INSTANCES OF THE EFFECT SIDE: a second (third ...) Sora core whose effects are HIS OWN, drawn on
//! his own bones and sockets by every Sora rule of this host exactly as the player's are.
//!
//! WHY: every Sora-only rule of `EffectHost` reads the step's `Scene` (his output, his core, his bones) and the host's own
//! Sora facts (the Keyblade in his hand `kb`, his place of the step before `int_before`, his right-hand form weapon, his
//! heal tier, his Blizzard rail, his animation's play id): the Keyblade trail (`key_trail0` -> `vfx_top`), the cast socket
//! (`_cal0` -> `vfx_call`), the heal (`sora_kaihuku`), the form weapons' trails and attach-data rows, the `kb:` / `pawn:`
//! anchors (the form weapons' and pawns' own rigs under HIS clip), the spell actors' / gun bullets' / staff beams' bodies
//! (`Anchor::Projectile` looked up in HIS core), the other-keyblade rule. A copy's effects fed into the player's step land
//! on the player, are refused, or never find their actor.
//!
//! THE ROUTE: a copy is a member of the party's handle ranges (`kh_audio::copies`: 10 the boss's Sora copy, 11 its summoned
//! Mimic, 12 the player's Mimic Sora; kh_core::host::party `PARTY_HANDLE_BASE + id * PARTY_HANDLE_SPAN`), and after his own
//! core's step and pose his host calls [`EffectHost::step_instance`] with HIS scene (his output, his core, his bones) and his
//! commands (`Attach::Sora` = on HIM). For that call the host's Sora facts are HIS (`SoraInst`, swapped in and out), and every
//! effect started is tagged with him (`Live::inst`): later steps place it on his scene only, his animation change ends his
//! endless one-shots, a new core of his (his handles counted from 1 again) ends what his last one started. The same commands
//! may also arrive in the shared queue (their audio side: `Attach::Party { member }` on his taps): the main step ignores every
//! command of a member that is an instance. `world.step` stays the main step's (one evaluator step for everything).
//! [`EffectHost::set_instance_keyblade`] gives him the Keyblade in HIS hand; [`EffectHost::stop_member`] ends everything
//! of his (he left); `char_drives` names him `owner = id + 1`.

use std::collections::HashMap;

use glam::Vec3;
use kh_core::{EffectHandle, Output};
use serde_json::Value;

use super::{right_weapon_of, EffectHost, KbFx, Scene};
use crate::audio::PoseTap;
use crate::seam::EffectCommand;

/// The low bits of a handle in a member's range (kh_core::host::party: each member's range is `PARTY_HANDLE_SPAN` long).
const LOW: u32 = kh_core::host::party::PARTY_HANDLE_SPAN - 1;

/// (MIMIC-FX) One Sora instance's own facts of the effect side (the host's Sora facts while his step runs).
#[derive(Default)]
pub(super) struct SoraInst {
    pub(super) kb: Option<KbFx>,
    pub(super) int_before: Option<(Output, PoseTap)>,
    pub(super) play_id: u32,
    pub(super) cure_group: u32,
    pub(super) right_weapon: Option<String>,
    pub(super) rail_last: Option<(Vec3, f32)>,
    /// His newest handle's low bits (a new core of his counts from 1 again).
    last_handle: u32,
    steps: u64,
}

impl SoraInst {
    /// His root as of his latest step (the in-light centre of his drives).
    pub(super) fn root(&self) -> Vec3 {
        self.int_before.as_ref().map_or(Vec3::ZERO, |(o, _)| o.position)
    }

    /// His next core counts its handles from 1 and has no step before (a new summon of him, the host cleared); his
    /// Keyblade stays known.
    pub(super) fn restart(&mut self) {
        self.last_handle = 0;
        self.int_before = None;
        self.rail_last = None;
    }
}

/// (MIMIC-FX, measuring aid) A live effect of the host: its seam handle, the instance it belongs to (None: the player's Sora,
/// the party's, a host's own), its effect file, where its system stands after the latest step, a trail.
#[derive(Clone, Debug, PartialEq)]
pub struct LivePlace {
    pub handle: u32,
    pub member: Option<u8>,
    pub effect: String,
    pub position: Vec3,
    pub trail: bool,
    pub stopped: bool,
}

/// The party member a handle belongs to (`kh_core::host::party::handle_member`).
fn member_of(handle: EffectHandle) -> Option<u8> {
    kh_core::host::party::handle_member(handle.0).map(|m| m.0)
}

fn handle_of(c: &EffectCommand) -> EffectHandle {
    match c {
        EffectCommand::Spawn(h, _) | EffectCommand::Stop(h, _) | EffectCommand::Tint(h, _) | EffectCommand::Color(h, _) | EffectCommand::Place(h, _) | EffectCommand::Transform(h, ..) | EffectCommand::Timing(h, _) => *h,
    }
}

/// (MIMIC-FX) The low bits of a Sora instance's handle that are his CORE's own number (kh-er copyfx.rs `own_handle` keeps them):
/// his core knows its spell actors, bullets, beams and avatars by it.
pub const CORE_BITS: u32 = 0x007F_FFFF;

impl super::Live {
    /// (MIMIC-FX) The handle the owning core knows this effect by: an instance's own number, the player's handle as it is.
    pub(super) fn core_handle(&self) -> EffectHandle {
        match self.inst {
            Some(_) => EffectHandle(self.handle.0 & CORE_BITS),
            None => self.handle,
        }
    }
}

impl EffectHost {
    /// (MIMIC-FX) The Sora facts of the host and of instance `i` trade places (his step runs on his own).
    fn swap_instance(&mut self, i: &mut SoraInst) {
        std::mem::swap(&mut self.kb, &mut i.kb);
        std::mem::swap(&mut self.int_before, &mut i.int_before);
        std::mem::swap(&mut self.play_id, &mut i.play_id);
        std::mem::swap(&mut self.cure_group, &mut i.cure_group);
        std::mem::swap(&mut self.right_weapon, &mut i.right_weapon);
        std::mem::swap(&mut self.rail_last, &mut i.rail_last);
    }

    /// (MIMIC-FX) A command of the shared queue that belongs to a Sora instance (his own step carries it out).
    pub(super) fn instance_owned(&self, c: &EffectCommand) -> bool {
        member_of(handle_of(c)).is_some_and(|m| self.instances.contains_key(&m))
    }

    /// (MIMIC-FX) Member `member` (a party handle range of his own) is a Sora instance of this host.
    pub fn is_instance(&self, member: u8) -> bool {
        self.instances.contains_key(&member)
    }

    /// (MIMIC-FX) THE KEYBLADE IN HIS HAND (as `set_keyblade` for the player): its trail, its sockets, the prefixes of its
    /// effects that pass the other-keyblade rule. Makes him an instance.
    pub fn set_instance_keyblade(&mut self, member: u8, trail: Option<&str>, sockets: Option<&Value>, allowed: &[String]) {
        let mut inst = self.instances.remove(&member).unwrap_or_default();
        std::mem::swap(&mut self.kb, &mut inst.kb);
        self.set_keyblade(trail, sockets, allowed);
        std::mem::swap(&mut self.kb, &mut inst.kb);
        self.instances.insert(member, inst);
    }

    /// (MIMIC-FX) ONE STEP OF SORA INSTANCE `member`, after his core's step and his pose, BEFORE the host's main `step` of the
    /// same simulation step (which evaluates everything once): his commands (`Attach::Sora` = on him; his handles in his own
    /// range), his animation change, the places of his live effects, all on scene `s` (HIS output, core, bones; `s.target`
    /// answers his targets). Makes him an instance.
    pub fn step_instance(&mut self, member: u8, commands: &[EffectCommand], s: &Scene) {
        let mut inst = self.instances.remove(&member).unwrap_or_default();
        inst.steps += 1;
        self.swap_instance(&mut inst);
        self.inst_now = Some(member);
        self.right_weapon = right_weapon_of(&s.sora.combat().form);
        let first = self.live.len();
        let mine: Vec<EffectCommand> = commands.iter().filter(|c| member_of(handle_of(c)) == Some(member)).copied().collect();
        for at in 0..mine.len() {
            if let EffectCommand::Spawn(h, _) = &mine[at] {
                // a new core of his counts its handles from 1 again: his old one's effects have no owner
                let low = h.0 & LOW;
                if low <= inst.last_handle {
                    for l in self.live.iter_mut().filter(|l| l.inst == Some(member)) {
                        self.world.stop(l.id, true);
                        l.stopped = true;
                    }
                }
                inst.last_handle = low;
            }
            self.apply_command(at, &mine, s);
        }
        // his endless one-shots end with HIS animation (the party's anim-set rule is not his)
        for l in self.live.iter_mut().skip(first).filter(|l| l.inst == Some(member)) {
            l.party_anim = None;
        }
        let play_id = s.out.anim.play_id;
        if play_id != self.play_id {
            self.play_id = play_id;
            for l in self.live.iter().filter(|l| l.ends_with_animation && l.inst == Some(member)) {
                self.world.stop(l.id, false);
            }
        }
        for k in 0..self.live.len() {
            if self.live[k].inst == Some(member) && self.world.is_alive(self.live[k].id) {
                self.place_live(k, s);
            }
        }
        self.remember_step(s);
        self.inst_now = None;
        self.swap_instance(&mut inst);
        self.instances.insert(member, inst);
    }

    /// (MIMIC-FX) EVERYTHING OF MEMBER `member` ENDS (a copy left, died away, the fight ended): every effect of his handle range
    /// (a Sora instance's, a friend copy's on the party's route). Returns how many effects were stopped. An instance's Keyblade
    /// stays known (a new summon of him starts with it); his next core counts from 1.
    pub fn stop_member(&mut self, member: u8, immediate: bool) -> usize {
        let mut n = 0;
        for l in self.live.iter_mut().filter(|l| (l.inst == Some(member) || member_of(l.handle) == Some(member)) && (immediate || !l.stopped)) {
            l.stopped = true;
            self.world.stop(l.id, immediate);
            n += 1;
        }
        if let Some(i) = self.instances.get_mut(&member) {
            i.restart();
        }
        n
    }

    /// (MIMIC-FX) WHERE HIS SOCKETS ARE on scene `s` (his own): every socket of the host's table, his Keyblade's, the Kingdom
    /// Key's and `extra` names (`kb:` / `pawn:` anchors), as the effect side resolves them for him (world position, rotation).
    /// For his tap: the sounds on his sockets are placed there.
    pub fn instance_sockets(&mut self, member: u8, s: &Scene, extra: &[&str]) -> Vec<(String, Vec3, glam::Quat)> {
        let mut inst = self.instances.remove(&member).unwrap_or_default();
        self.swap_instance(&mut inst);
        let mut names: Vec<String> = self.sockets.keys().cloned().collect();
        if let Some(k) = self.kb.as_ref() {
            names.extend(k.sockets.keys().cloned());
        }
        names.extend(self.weapon_sockets.keys().cloned());
        names.extend(extra.iter().map(|n| n.to_string()));
        names.sort();
        names.dedup();
        let out = names
            .into_iter()
            .map(|n| {
                let x = self.socket_world(s, Some(&n));
                (n, x.position, glam::Quat::from_mat3(&x.rotation).normalize())
            })
            .collect();
        self.swap_instance(&mut inst);
        self.instances.insert(member, inst);
        out
    }

    /// (MIMIC-FX) How many effects of instance `member` are alive and not stopped.
    pub fn instance_live(&self, member: u8) -> usize {
        self.live.iter().filter(|l| l.inst == Some(member) && !l.stopped && self.world.is_alive(l.id)).count()
    }

    /// (MIMIC-FX, measuring aid) Every live effect with its owner and where its system stands after the latest step.
    pub fn places(&self) -> Vec<LivePlace> {
        let names: HashMap<u64, String> = self.world.live().into_iter().map(|l| (l.id.0, l.name)).collect();
        self.live
            .iter()
            .filter(|l| self.world.is_alive(l.id))
            .map(|l| LivePlace {
                handle: l.handle.0,
                member: l.inst,
                effect: names.get(&l.id.0).cloned().unwrap_or_default(),
                position: self.world.latest_nodes(l.id).map_or(Vec3::ZERO, |(x, _)| x.position),
                trail: l.trail,
                stopped: l.stopped,
            })
            .collect()
    }

    /// (MIMIC-FX, harness aid) Member `to` stands where member `from` stood at the latest `set_party`, and `from`'s spell actors
    /// are `to`'s (their handles moved into `to`'s range): an offline stand-in for a copy's own tap and actors (the effects
    /// harness's `--party-copy`).
    pub fn alias_party(&mut self, from: u8, to: u8) {
        if let Some(m) = self.party_members.get(&from).cloned() {
            self.party_members.insert(to, m);
        }
        let span = kh_core::host::party::PARTY_HANDLE_SPAN;
        for p in self.party_projectiles.iter_mut().filter(|p| member_of(EffectHandle(p.0)) == Some(from)) {
            p.0 = kh_core::host::party::PARTY_HANDLE_BASE + u32::from(to) * span + (p.0 & (span - 1));
        }
    }

    /// (MIMIC-FX, measuring aid) Where the live systems of seam handle `handle` stand after the latest step.
    pub fn places_of(&self, handle: u32) -> Vec<Vec3> {
        self.live.iter().filter(|l| l.handle.0 == handle && self.world.is_alive(l.id)).filter_map(|l| self.world.latest_nodes(l.id).map(|(x, _)| x.position)).collect()
    }

    /// (MIMIC-FX, measuring aid) A live trail's newest sample (its head) by seam handle.
    pub fn trail_head_of(&self, handle: u32) -> Option<Vec3> {
        self.live.iter().find(|l| l.handle.0 == handle && l.trail).and_then(|l| self.world.trail_head(l.id)).map(|(p, _)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mimic_fx_a_commands_handle_and_member() {
        let base = kh_core::host::party::PARTY_HANDLE_BASE;
        let span = kh_core::host::party::PARTY_HANDLE_SPAN;
        let h = EffectHandle(base + 12 * span + 5);
        assert_eq!(member_of(h), Some(12));
        assert_eq!(h.0 & LOW, 5);
        assert_eq!(member_of(EffectHandle(5)), None);
        let c = EffectCommand::Stop(h, true);
        assert_eq!(handle_of(&c), h);
    }
}
