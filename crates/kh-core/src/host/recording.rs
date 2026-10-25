//! A host for tests: it has a list of simple targets, takes every call and writes it down.
//! Deterministic, engine-free; `kh-app`'s training dummies are the drawn counterpart.

use super::{AttackShape, PushBody, TargetBody, Audio, EffectHandle, EffectSpawn, Effects, HitRecord, HitResult, Host, HurtShape, SoundHandle, SoundPlay, TargetFlags, TargetId, TargetInfo, Targets, UiSink};
use crate::ui_model::UiSnapshot;
use crate::world::World;
use std::cell::RefCell;

/// One call the core made on the host, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    Play(SoundHandle, SoundPlay),
    StopSound(SoundHandle, f32),
    Spawn(EffectHandle, EffectSpawn),
    StopEffect(EffectHandle, bool),
    Hit(HitRecord, HitResult),
    /// `UiSink::publish` (the snapshot itself is kept in `RecordingHost::ui`).
    Publish(u64),
}

/// Records everything. Its targets are the `TargetInfo`s put into `targets`: one hurt shape each
/// (the bounds capsule), and a hit simply takes `HitRecord::damage` off `hp` when the target is
/// `HITTABLE` and alive (it loses `ALIVE` at 0). That is a test fixture, not a damage rule.
#[derive(Clone, Debug, Default)]
pub struct RecordingHost {
    pub targets: Vec<TargetInfo>,
    pub calls: Vec<Call>,
    /// The latest published snapshot.
    pub ui: Option<UiSnapshot>,
    /// What the targets answer to the hit classification (combat strand C 8.2.6); a target
    /// without an entry is a DEFAULT body.
    pub bodies: Vec<(TargetId, TargetBody)>,
    /// Armour left / reaction endurance left per target (`take_armor` / `take_reaction_power`
    /// subtract the power and answer what is left).
    pub armor: Vec<(TargetId, i32)>,
    pub endurance: Vec<(TargetId, i32)>,
    /// Push levels: a target with an entry has a push body equal to its bounds.
    pub push_levels: Vec<(TargetId, u8)>,
    /// Attack shapes for the clash pass.
    pub attack_shapes: Vec<AttackShape>,
    /// Hits that reached a target without `TakeDamage` (notify slot 28).
    pub touched: Vec<HitRecord>,
    /// `Targets::reaction_state`: the damage kind of the latest hit that took HP, per target
    /// (a fixture: the target stays "in that reaction" until the test clears the entry).
    pub reactions: Vec<(TargetId, u8)>,
    /// (combat, fourth pass) Targets that answer `Targets::rush_suppression` with yes.
    pub rush_suppression: Vec<TargetId>,
    /// (XSI) `Targets::foe_level` answers (powerful foes and their battle level) and the targets in their revenge.
    pub foe_levels: Vec<(TargetId, u8)>,
    pub revenge: Vec<TargetId>,
}

impl RecordingHost {
    pub fn new() -> RecordingHost {
        RecordingHost::default()
    }

    /// Runs `run` with the bundle for one step over `world`: this one object plays all four
    /// roles, and every call lands in `calls` in the order it was made.
    pub fn step<R>(&mut self, world: &dyn World, run: impl FnOnce(&mut Host<'_>) -> R) -> R {
        let calls = RefCell::new(std::mem::take(&mut self.calls));
        let ui = RefCell::new(None);
        let mut targets = RecordedTargets { targets: &mut self.targets, calls: &calls, bodies: &self.bodies, armor: &mut self.armor, endurance: &mut self.endurance, push_levels: &self.push_levels, attack_shapes: &self.attack_shapes, touched: &mut self.touched, reactions: &mut self.reactions, rush_suppression: &self.rush_suppression, foe_levels: &self.foe_levels, revenge: &mut self.revenge };
        let (mut audio, mut effects, mut sink) = (RecordedCalls { calls: &calls, ui: &ui }, RecordedCalls { calls: &calls, ui: &ui }, RecordedCalls { calls: &calls, ui: &ui });
        let result = run(&mut Host { world, targets: &mut targets, audio: &mut audio, effects: &mut effects, ui: &mut sink });
        self.calls = calls.into_inner();
        if let Some(snapshot) = ui.into_inner() {
            self.ui = Some(snapshot);
        }
        result
    }

    /// The hits delivered so far.
    pub fn hits(&self) -> Vec<(HitRecord, HitResult)> {
        self.calls.iter().filter_map(|c| if let Call::Hit(record, result) = c { Some((*record, *result)) } else { None }).collect()
    }
}

struct RecordedTargets<'a> {
    targets: &'a mut Vec<TargetInfo>,
    calls: &'a RefCell<Vec<Call>>,
    bodies: &'a [(TargetId, TargetBody)],
    armor: &'a mut Vec<(TargetId, i32)>,
    endurance: &'a mut Vec<(TargetId, i32)>,
    push_levels: &'a [(TargetId, u8)],
    attack_shapes: &'a [AttackShape],
    touched: &'a mut Vec<HitRecord>,
    reactions: &'a mut Vec<(TargetId, u8)>,
    rush_suppression: &'a [TargetId],
    foe_levels: &'a [(TargetId, u8)],
    revenge: &'a mut Vec<TargetId>,
}

impl Targets for RecordedTargets<'_> {
    fn foe_level(&self, id: TargetId) -> Option<u8> {
        self.foe_levels.iter().find(|f| f.0 == id).map(|f| f.1)
    }

    fn in_revenge(&self, id: TargetId) -> bool {
        self.revenge.contains(&id)
    }

    fn end_revenge(&mut self, id: TargetId) {
        self.revenge.retain(|r| *r != id);
    }

    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.targets.iter().copied());
    }

    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.targets.iter().copied().find(|t| t.id == id)
    }

    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(t) = self.info(id) {
            let axis = glam::Vec3::Z * (t.half_height - t.radius).max(0.0);
            out.push(HurtShape { a: t.center - axis, b: t.center + axis, radius: t.radius, part: 0 });
        }
    }

    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        let result = match self.targets.iter_mut().find(|t| t.id == hit.target) {
            Some(t) if t.flags.has(TargetFlags::HITTABLE | TargetFlags::ALIVE) => {
                // (Guarded / repelled / armour-break results take no HP: strand E 8.4.)
                let dealt = if hit.takes_hp() { hit.damage.max(0.0).min(t.hp) } else { 0.0 };
                t.hp -= dealt;
                let killed = t.hp <= 0.0;
                if killed {
                    t.flags = TargetFlags(t.flags.0 & !TargetFlags::ALIVE);
                }
                if hit.takes_hp() {
                    self.reactions.retain(|r| r.0 != hit.target);
                    self.reactions.push((hit.target, hit.damage_kind.value() as u8));
                }
                HitResult { accepted: true, damage_dealt: dealt, hp_after: t.hp, killed, guarded: !hit.takes_hp() }
            }
            _ => HitResult::default(),
        };
        self.calls.borrow_mut().push(Call::Hit(*hit, result));
        result
    }

    fn body(&self, id: TargetId, _part: u32) -> TargetBody {
        self.bodies.iter().find(|b| b.0 == id).map_or(TargetBody::DEFAULT, |b| b.1)
    }

    fn take_armor(&mut self, id: TargetId, power: i32, _weak: bool) -> i32 {
        match self.armor.iter_mut().find(|a| a.0 == id) {
            Some(a) => {
                a.1 -= power;
                a.1
            }
            None => 0,
        }
    }

    fn take_reaction_power(&mut self, id: TargetId, power: i32) -> i32 {
        match self.endurance.iter_mut().find(|a| a.0 == id) {
            Some(a) => {
                a.1 -= power;
                a.1
            }
            None => 0,
        }
    }

    fn push_body(&self, id: TargetId) -> Option<PushBody> {
        let level = self.push_levels.iter().find(|p| p.0 == id)?.1;
        let t = self.info(id)?;
        Some(PushBody { center: t.center, radius: t.radius, half_height: t.half_height, level })
    }

    fn attack_shapes(&self, out: &mut Vec<AttackShape>) {
        out.extend_from_slice(self.attack_shapes);
    }

    fn touched(&mut self, hit: &HitRecord) {
        self.touched.push(*hit);
    }

    fn reaction_state(&self, id: TargetId) -> u8 {
        self.reactions.iter().find(|r| r.0 == id).map_or(0, |r| r.1)
    }

    fn rush_suppression(&self, id: TargetId) -> bool {
        self.rush_suppression.contains(&id)
    }
}

struct RecordedCalls<'a> {
    calls: &'a RefCell<Vec<Call>>,
    ui: &'a RefCell<Option<UiSnapshot>>,
}

impl Audio for RecordedCalls<'_> {
    fn play(&mut self, handle: SoundHandle, play: &SoundPlay) {
        self.calls.borrow_mut().push(Call::Play(handle, *play));
    }

    fn stop(&mut self, handle: SoundHandle, fade_out: f32) {
        self.calls.borrow_mut().push(Call::StopSound(handle, fade_out));
    }

    /// (party stream E) Recorded as the play that carries it (party::voice_group_no_play).
    fn party_voice_group_no(&mut self, member: super::PartyMember, number: i32) {
        self.calls.borrow_mut().push(Call::Play(SoundHandle(0), super::party::voice_group_no_play(member, number)));
    }
}

impl Effects for RecordedCalls<'_> {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
        self.calls.borrow_mut().push(Call::Spawn(handle, *spawn));
    }

    fn stop(&mut self, handle: EffectHandle, immediate: bool) {
        self.calls.borrow_mut().push(Call::StopEffect(handle, immediate));
    }
}

impl UiSink for RecordedCalls<'_> {
    fn publish(&mut self, snapshot: &UiSnapshot) {
        self.calls.borrow_mut().push(Call::Publish(snapshot.step));
        *self.ui.borrow_mut() = Some(snapshot.clone());
    }
}
