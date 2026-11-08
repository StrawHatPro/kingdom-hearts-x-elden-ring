//! (MIMIC-FX) A SORA COPY IN THE EFFECTS HARNESS (`examples/effects.rs --copy-fx X,Y`): a second real Sora core (the same
//! loadout, his own animator and training dummy, `X,Y` cm from the player), stepped with the player's input every step (the same
//! script, casts and `--style-at` gauges), whose effects go to the effect side as a SORA INSTANCE (`EffectHost::step_instance`,
//! his own scene: his bones, his core, his Keyblade) under his own member id (kh_audio::copies::MIMIC_SORA: the route every Mimic
//! Tear copy takes in kh-er, copyfx.rs). Draw his body with the renderer's copy aid at the same offset
//! (`KH_SORA_COPY_TEST=X,Y`: the player's pose moved there, which is his: the two cores play the same moves): his effects must
//! sit on it.

use glam::Vec3;
use kh_core::host::party::{PARTY_HANDLE_BASE, PARTY_HANDLE_SPAN};
use kh_core::{EffectHandle, EffectSpawn, Input, Pose};
use kh_host::audio::PoseTap;
use kh_host::rig::{component_to_world, Dummy, Rig};
use kh_host::seam::EffectQueue;
use kh_host::vfx::{EffectHost, Scene};

const MEMBER: u8 = kh_audio::copies::MIMIC_SORA;

fn own(raw: u32) -> u32 {
    PARTY_HANDLE_BASE + u32::from(MEMBER) * PARTY_HANDLE_SPAN + (raw & kh_host::vfx::CORE_BITS)
}

/// kh-er copyfx.rs `SoraCopyFx`: his own handles, `Attach::Sora` = on HIM.
struct OwnFx<'a>(&'a mut EffectQueue);

impl kh_core::Effects for OwnFx<'_> {
    fn spawn(&mut self, h: EffectHandle, s: &EffectSpawn) {
        self.0.spawn(EffectHandle(own(h.0)), s);
    }
    fn timing(&mut self, h: EffectHandle, t: kh_core::host::EffectTiming) {
        self.0.timing(EffectHandle(own(h.0)), t);
    }
    fn tint(&mut self, h: EffectHandle, t: kh_core::host::Tint) {
        self.0.tint(EffectHandle(own(h.0)), t);
    }
    fn color(&mut self, h: EffectHandle, rgba: [f32; 4]) {
        self.0.color(EffectHandle(own(h.0)), rgba);
    }
    fn place(&mut self, h: EffectHandle, at: Vec3) {
        self.0.place(EffectHandle(own(h.0)), at);
    }
    fn transform(&mut self, h: EffectHandle, at: Vec3, r: Vec3, s: Vec3) {
        self.0.transform(EffectHandle(own(h.0)), at, r, s);
    }
    fn stop(&mut self, h: EffectHandle, immediate: bool) {
        self.0.stop(EffectHandle(own(h.0)), immediate);
    }
}

pub struct CopyFx {
    rig: Rig,
    tap: PoseTap,
    commands: Vec<kh_host::seam::EffectCommand>,
    casts: usize,
    steps: u64,
}

impl CopyFx {
    /// `--copy-fx X,Y` (cm): the copy there (his dummy as far ahead of him as the player's of the player). None: no copy.
    pub fn from_arg(arg: Option<String>, pack: &std::path::Path, loadout: &std::path::Path, dummy_cm: f32) -> Option<CopyFx> {
        let v: Vec<f32> = arg?.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        let offset = Vec3::new(*v.first()?, *v.get(1)?, 0.0);
        let mut rig = Rig::new(pack, loadout, (dummy_cm > 0.0).then(|| Vec3::new(dummy_cm, 0.0, 0.0) + offset)).ok()?;
        let z = rig.sora.output().position.z;
        rig.sora.teleport(Pose { position: Vec3::new(offset.x, offset.y, z), yaw: 0.0 });
        rig.out = rig.sora.output();
        println!("effects: (MIMIC-FX) a Sora copy at ({:.0}, {:.0}) cm, member {MEMBER}: his effects through the effect side's Sora instance", offset.x, offset.y);
        Some(CopyFx { rig, tap: PoseTap::default(), commands: Vec::new(), casts: 0, steps: 0 })
    }

    /// His step with the player's input of this step (`player`: the player's rig after his step, for his casts); `style`: a full
    /// form gauge this step (`--style-at`).
    pub fn step(&mut self, dt: f32, input: &Input, player: &Rig, style: bool) {
        for (_, name) in &player.casts[self.casts..] {
            if let Some(kind) = kh_core::magic::spells::command_by_name(name) {
                self.rig.sora.command_request(kh_core::command::CommandRequest::Execute(kind));
            }
        }
        self.casts = player.casts.len();
        if style {
            self.rig.sora.debug_add_style(100.0);
        }
        let mut q = EffectQueue::default();
        let mut audio = kh_core::host::audio::NullAudio;
        self.rig.step_with_dt(dt, input, &mut audio, &mut OwnFx(&mut q));
        self.steps += 1;
        self.tap.record(self.steps, &self.rig.out, self.rig.animator.skeleton(), &self.rig.pose, &component_to_world(&self.rig.out));
        self.tap.samples.clear();
        self.commands = q.0;
    }

    /// His effects on HIS scene (before the player's effect step, as kh-er's copies step before `Fx::after_step`).
    pub fn effect_step(&mut self, effects: &mut EffectHost, eye: Vec3) {
        let dummy = self.rig.dummy;
        let target = move |_: kh_core::TargetId| dummy.map(|d: Dummy| d.info.center);
        let commands = std::mem::take(&mut self.commands);
        effects.step_instance(MEMBER, &commands, &Scene { out: &self.rig.out, sora: &self.rig.sora, tap: &self.tap, target: &target, ground: Some(0.0), eye });
        if self.steps.is_multiple_of(30) {
            let live = effects.instance_live(MEMBER);
            let places: Vec<String> = effects.places().into_iter().filter(|p| p.member == Some(MEMBER) && !p.stopped).take(6).map(|p| format!("{} {:.0} cm from him", p.effect, p.position.distance(self.rig.out.position))).collect();
            println!("effects: (MIMIC-FX) step {}: the copy at ({:.0}, {:.0}) {:?}, {live} effect(s) of his live: {}", self.steps, self.rig.out.position.x, self.rig.out.position.y, self.rig.out.state, places.join(", "));
        }
    }
}
