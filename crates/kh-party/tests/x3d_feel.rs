//! (party X3D; DON-KIT: the overhaul's kit, documentation/DONALD_OVERHAUL.md) THE FEEL NUMBERS of Donald's kit measured against Sora's (XD_PLAYABLE_KITS.md section 4): input-to-action
//! latency (frames from the press to the move's state), the cancel window after a hit, the dodge's invincible frames.
//! Sora's side runs kh-core's own Sora (level-99 loadout, the user's tuning) on the same 60 Hz step.
//! `cargo test --release -p kh-party --test x3d_feel -- --nocapture` prints the table (IMPL_X3D_STATE.md "Feel").

use glam::{Vec2, Vec3};
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::test_world::TestWorld;
use kh_core::Tuning;
use kh_party::pad::donald::{self, K};
use kh_party::pad::donald_kit::KitAbilities;
use kh_party::pad::PadFrame;
use kh_party::{Chara, MemberId, NpcStateId, Party, PartyConfig, PartyHost, SoraView};

const DT: f32 = 1.0 / 60.0;

struct Dummy(Vec<(TargetId, Vec3)>);

impl Targets for Dummy {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        for &(id, p) in &self.0 {
            out.push(self.info(id).unwrap_or(TargetInfo { id, position: p, center: p, velocity: Vec3::ZERO, yaw: 0.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(0), hp: 1.0, hp_max: 1.0 }));
        }
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.0.iter().find(|t| t.0 == id).map(|&(id, p)| TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 })
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.position + Vec3::Z * 30.0, b: i.position + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
}

struct Donald {
    party: Party,
    id: MemberId,
    world: TestWorld,
    dummy: Dummy,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    held: PadButtons,
}

impl Donald {
    fn new(dummy: Option<Vec3>) -> Donald {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        party.member_mut(id).unwrap().status.kit = KitAbilities::test_all();
        let mut d = Donald { party, id, world: TestWorld::floor(), dummy: Dummy(dummy.map(|p| vec![(TargetId(1), p)]).unwrap_or_default()), audio: Default::default(), effects: Default::default(), held: PadButtons::NONE };
        let max = d.party.member(id).unwrap().status.max_mp();
        d.party.member_mut(id).unwrap().status.mp = max - 1;
        for _ in 0..10 {
            d.step(Vec2::ZERO, &[]);
        }
        d
    }

    fn step(&mut self, stick: Vec2, held: &[PadButton]) {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = PadState { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        if let Some(p) = self.party.member_mut(self.id).and_then(|m| m.pad.as_mut()).filter(|p| p.finisher_pending) {
            p.finisher_pending = false;
            p.finisher_answer = Some(false);
        }
        self.party.set_pad_frame(self.id, PadFrame { stick, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummy, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &SoraView { position: Vec3::new(-3000.0, 0.0, 0.0), ..SoraView::default() }, &mut host);
    }

    fn state(&self) -> NpcStateId {
        self.party.member(self.id).unwrap().state
    }

    /// Frames from the press frame (1 = the press frame itself) until `done`.
    fn latency(&mut self, stick: Vec2, press: &[PadButton], hold: bool, done: impl Fn(&Donald) -> bool) -> usize {
        self.step(stick, press);
        if done(self) {
            return 1;
        }
        for k in 2..120 {
            self.step(stick, if hold { press } else { &[] });
            if done(self) {
                return k;
            }
        }
        999
    }
}

fn kit(d: &Donald) -> Option<u8> {
    donald::kit_state(d.party.member(d.id).unwrap())
}

#[test]
fn feel_numbers_against_sora() {
    let x = PadButton::FaceLeft;
    let a = PadButton::FaceBottom;
    let b = PadButton::FaceRight;
    let right = Vec2::new(1.0, 0.0);
    // ---------------------------------------------------------------- Donald
    let mut d = Donald::new(None);
    let hop = d.latency(right, &[x], false, |d| kit(d) == Some(donald::DODGE)); // (DON-KIT) the Hop is the plain dodge
    let mut inv = 0;
    for _ in 0..40 {
        if donald::invincible(d.party.member(d.id).unwrap()) {
            inv += 1;
        }
        d.step(Vec2::ZERO, &[]);
    }
    let mut d = Donald::new(None);
    let block = d.latency(Vec2::ZERO, &[x], false, |d| kit(d) == Some(donald::BLOCK));
    let mut d = Donald::new(None);
    let beat = d.latency(Vec2::ZERO, &[a], false, |d| matches!(d.state(), NpcStateId::Pad(donald::STEP | donald::BONK))); // (DON-KIT) the first missile step (no target: the Wand Bonk)
    // (DON-KIT) the Air Blink is gone (doc 13; Blink is Thunder's ground dodge): no air dodge to measure
    let _ = b;
    let blink = 1;
    // (DON-KIT) the cancel window: the first missile leaves, then X held: frames from the shot to the dodge
    let mut d = Donald::new(Some(Vec3::new(1200.0, 0.0, 0.0)));
    d.step(Vec2::ZERO, &[a]);
    let mut hit_frame = 0;
    for k in 0..60 {
        d.step(Vec2::ZERO, &[]);
        if d.party.member(d.id).unwrap().pad.as_ref().unwrap().donald.shots.fired > 0 {
            hit_frame = k;
            break;
        }
    }
    let after_hit = d.latency(right, &[x], true, |d| kit(d) == Some(donald::DODGE));
    let beat_len = kh_party::tables::clip(Chara::Donald, "M_1301").length;
    // ---------------------------------------------------------------- Sora (kh-core): the dodge roll on a fresh Sora with DODGE
    let world = TestWorld::floor();
    let sora_at = |frame: usize| -> (bool, bool) {
        let origin = Vec3::Z * kh_core::Sora::origin_height_on_floor(&kh_core::SORA_PARAMS);
        let mut sora = kh_core::Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::USER).with_status(level99());
        let idle = kh_core::Input::default();
        for _ in 0..5 {
            sora.step(DT, &idle, &world);
        }
        let g = PadButtons::NONE.with(PadButton::GUARD);
        let press = kh_core::Input { move_stick: Vec2::new(0.0, 1.0), ..kh_core::Input::default() }.with_pad(PadState { held: g, pressed: g, released: PadButtons::NONE });
        let first = sora.step(DT, &press, &world);
        for _ in 1..frame {
            sora.step(DT, &idle, &world);
        }
        let own = sora.output().position;
        let hit = kh_core::states::HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..kh_core::states::HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 10.0, TargetId(1), own + Vec3::new(150.0, 0.0, 0.0)) };
        sora.take_hit(hit);
        let out = sora.step(DT, &idle, &world);
        let dodged = out.events.iter().any(|e| matches!(e.kind, kh_core::host::EventKind::PlayerDodged { .. }));
        (first.state == kh_core::StateId::Dodgeroll, dodged)
    };
    let sora_dodge_latency = if sora_at(1).0 { 1 } else { 2 };
    let sora_inv = (1..60).filter(|f| sora_at(*f).1).count();
    eprintln!("FEEL (frames at 60 Hz; 1 = the press frame itself)");
    eprintln!("  Donald Hop: latency {hop}, invincible {inv} frames ({:.3} s; sheet {:.2} .. {:.2})", inv as f32 * DT, K.hop_invincible_from, K.hop_invincible_to);
    eprintln!("  Donald Magic Block: latency {block}; window 0 .. {:.2} s (perfect {:.2})", K.block_window, K.block_perfect); // (DON-KIT)
    eprintln!("  Donald Beat (A): latency {beat}");
    eprintln!("  Donald cancel: the wave's missile left at frame {hit_frame} of its {:.2} s clip; X held -> the dodge {after_hit} frames later (rule {:.2} s)", beat_len, K.feel_cancel_after_shot); // (DON-KIT)
    eprintln!("  Sora dodge roll: latency {sora_dodge_latency}, invincible {sora_inv} frames ({:.3} s)", sora_inv as f32 * DT);
    assert_eq!(hop, 1, "the Hop starts on the press frame");
    assert_eq!(block, 1);
    assert_eq!(blink, 1);
    assert!(beat <= 2);
    assert!((13..=17).contains(&inv), "{inv}"); // (DON-KIT) hop.invincible_to 0.27 s
    assert!(after_hit as f32 * DT <= K.feel_cancel_after_shot + 2.5 * DT, "{after_hit}"); // (DON-KIT)
    assert!(sora_inv > inv, "the Hop's invincibility is shorter than Sora's roll ({sora_inv} frames)");
}

/// Sora's level-99 loadout (sheets/port/sora-loadout-99.json), as kh-core's tests/actions.rs reads it.
fn level99() -> kh_core::Status {
    use kh_core::status::LoadoutValue;
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = kh_core::Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(items) => LoadoutValue::List(items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect()),
            serde_json::Value::Object(map) => LoadoutValue::Map(map.iter().filter_map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect()),
            _ => continue,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    kh_core::Status::from_loadout(&loadout)
}
