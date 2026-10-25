//! (FACE) THE FRIENDS' FACES IN THE TEST LEVEL'S SHAPE: K1's harness (`PartyRuntime` as kh-app's player.rs drives it)
//! with Donald leading on the pad and his poser from the play pack, the face of every step measured on his posed
//! skeleton: the eyelid gap (both eyes, cm), the beak opening (CU_jaw_b - CD_jaw_b, cm), the clips on the Face / Mouth
//! slots. `cargo test -p kh-partyhost --lib --release face_ -- --nocapture --test-threads=1`.

use kh_core::host::TargetId;
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::{Input, Vec2, Vec3};
use kh_host::party::PartyChara;

use crate::k1_tests::Harness;
use crate::*;

const DT: f32 = 1.0 / 60.0;

fn project(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

/// The party pack: `KH_FACE_PACK` (a pack root), else build/play/donpack, else build/sora_pack (and the ws' parents).
pub fn party_root() -> Option<std::path::PathBuf> {
    let mut list: Vec<std::path::PathBuf> = std::env::var("KH_FACE_PACK").ok().map(std::path::PathBuf::from).into_iter().collect();
    for up in ["", "../../../../"] {
        list.push(project(&format!("{up}build/play/donpack")));
        list.push(project(&format!("{up}build/sora_pack")));
    }
    list.into_iter().map(|r| r.join("party")).find(|p| p.join("n_ex001/manifest.json").is_file())
}

pub fn load_member(root: &std::path::Path, id: u8) -> Option<PosedMember> {
    let read = |p: &std::path::Path| {
        std::fs::read_to_string(p)
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok())
    };
    let index = read(&root.join("party.json")).unwrap_or(serde_json::Value::Null);
    let dir = root.join(poser::npc_of(id)?);
    let manifest = read(&dir.join("manifest.json"))?;
    let entry = index["members"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|m| m["member"].as_u64() == Some(id as u64))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let animator = kh_anim::Animator::new(
        kh_anim::Pack::load(&dir).ok()?,
        kh_anim::Settings {
            leg_ik: false,
            bonamik: false,
            ..kh_anim::Settings::default()
        },
    )
    .ok()?;
    PosedMember::new(id, animator, &manifest, &entry).ok()
}

/// The eyelid gap (cm, both eyes) and the beak / mouth opening (cm) of the latest step.
pub fn face_numbers(m: &PosedMember) -> (f32, f32) {
    let s = m.animator.skeleton();
    let c = &m.current.component;
    let d = |a: &str, b: &str| match (s.index_of(a), s.index_of(b)) {
        (Some(a), Some(b)) => (c[a].translation - c[b].translation).length(),
        _ => 0.0,
    };
    (d("RU_eyelid_e", "RD_eyelid_b") + d("LU_eyelid_e", "LD_eyelid_b"), d("CU_jaw_b", "CD_jaw_b"))
}

/// (FACE) The rig's enemies: none, or one standing at a place (TargetId 1).
#[derive(Clone, Copy)]
pub struct Foes(pub Option<Vec3>);
impl Foes {
    fn info_of(&self) -> Option<kh_core::host::TargetInfo> {
        use kh_core::host::{TargetFlags, TargetInfo};
        let p = self.0?;
        Some(TargetInfo {
            id: TargetId(1),
            position: p,
            center: p + Vec3::Z * 90.0,
            velocity: Vec3::ZERO,
            yaw: 180.0,
            radius: 50.0,
            half_height: 90.0,
            flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY),
            hp: 1.0e6,
            hp_max: 1.0e6,
        })
    }
}
impl kh_core::Targets for Foes {
    fn enumerate(&self, out: &mut Vec<kh_core::host::TargetInfo>) {
        out.extend(self.info_of());
    }
    fn info(&self, id: TargetId) -> Option<kh_core::host::TargetInfo> {
        self.info_of().filter(|_| id == TargetId(1))
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<kh_core::host::HurtShape>) {
        if let (Some(p), true) = (self.0, id == TargetId(1)) {
            out.push(kh_core::host::HurtShape { a: p + Vec3::Z * 30.0, b: p + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &kh_core::host::HitRecord) -> kh_core::host::HitResult {
        kh_core::host::HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
}
impl PartyTargets for Foes {
    fn ai_actors(&self, out: &mut Vec<kh_ai::ActorView>) {
        if let Some(p) = self.0 {
            out.push(kh_ai::ActorView {
                actor: kh_ai::ActorRef::Enemy(1),
                uid: -1,
                position: p,
                center: p + Vec3::Z * 90.0,
                radius: 50.0,
                half_height: 90.0,
                alive: true,
                hostile: true,
                lockable: true,
                hp: 1.0e6,
                hp_max: 1.0e6,
                ..kh_ai::ActorView::default()
            });
        }
    }
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

struct Ik;
impl kh_anim::IkWorld for Ik {
    fn sweep_sphere(&self, start: Vec3, end: Vec3, _radius: f32) -> Option<kh_anim::SweepHit> {
        // the floor at z = 0
        if start.z >= 0.0 && end.z < 0.0 {
            let t = start.z / (start.z - end.z);
            return Some(kh_anim::SweepHit {
                time: t,
                impact_point: start.lerp(end, t),
                impact_normal: Vec3::Z,
                start_penetrating: false,
            });
        }
        None
    }
}

/// The runtime with Donald leading, his poser, the buttons held last step.
pub struct Rig {
    pub h: Harness,
    pub poser: PartyPoser,
    held: PadButtons,
    pub log: Vec<String>,
    pub t: f32,
    /// (FACE) one enemy standing there (None: none)
    pub foe: Option<Vec3>,
}

impl Rig {
    pub fn new() -> Option<Rig> {
        Rig::led_by(true)
    }

    /// (FACE) The rig with Donald leading on the pad (`donald`), else Sora leading and Donald on his AI.
    pub fn led_by(donald: bool) -> Option<Rig> {
        let root = party_root()?;
        let mut h = Harness::new()?;
        let mut poser = PartyPoser {
            members: Default::default(),
            menu: Default::default(),
        };
        for id in [poser::DONALD, poser::GOOFY] {
            poser.members.insert(id, load_member(&root, id)?);
        }
        if donald {
            h.rt.lead_word(&["leader", PartyChara::Donald.name()]);
        }
        Some(Rig {
            h,
            poser,
            held: PadButtons::NONE,
            log: Vec::new(),
            t: 0.0,
            foe: None,
        })
    }

    /// One fixed step with `buttons` held (pressed / released from the last step's).
    pub fn step(&mut self, buttons: &[PadButton], stick: Vec2) {
        let mut now = PadButtons::NONE;
        for b in buttons {
            now.set(*b, true);
        }
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        let foes = Foes(self.foe);
        let h = &mut self.h;
        let (lp, ly) = h.leader_pos();
        let (s, c) = ly.to_radians().sin_cos();
        let camera = kh_camera::CameraView {
            position: lp - Vec3::new(c, s, 0.0) * 450.0 + Vec3::Z * 220.0,
            rotation: kh_camera::Rot { pitch: -15.0, yaw: ly, roll: 0.0 },
            rotation_target: kh_camera::Rot { pitch: -15.0, yaw: ly, roll: 0.0 },
            look_point: lp,
            distance: 450.0,
            auto_power: 0.0,
            stick_wait: 0.0,
            still_wait: 0.0,
            reset_wait: 0.0,
            slope_pitch: 0.0,
            stick_active: false,
            followed: false,
            collided: false,
        };
        let input = Input {
            move_stick: stick,
            camera_yaw: ly,
            ..Input::default()
        }
        .with_pad(pad);
        let input = h.rt.route(RouteIn {
            sora: &mut h.sora,
            input: &input,
            dt: DT,
            targets: &foes,
            camera: &camera,
            asked_formation: None,
            asked_ai: Vec::new(),
            nav: None,
            now: h.t,
        });
        let mut targets = foes;
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        let mut ui = kh_core::host::NullUi;
        h.out = {
            let mut st = SoraTargets {
                targets: &mut targets,
                runtime: &mut h.rt,
            };
            let mut host = kh_core::Host {
                world: &h.world,
                targets: &mut st,
                audio: &mut audio,
                effects: &mut effects,
                ui: &mut ui,
            };
            h.sora.step_hosted(DT, &input, &mut host)
        };
        let _ = h.rt.step(StepIn {
            dt: DT,
            sora: &mut h.sora,
            out: &h.out,
            lock_on: TargetId::NONE,
            camera: &camera,
            world: &h.world,
            targets: &mut targets,
            audio: &mut audio,
            effects: &mut effects,
            nav: None,
            now: h.t,
        });
        let _ = h.rt.pose(&mut self.poser, &Ik, DT);
        let _ = h.rt.after_step(&foes, DT, h.t);
        h.t += DT as f64;
        self.t += DT;
    }

    pub fn donald(&self) -> &PosedMember {
        self.poser.member(poser::DONALD).unwrap()
    }

    pub fn line(&self) -> String {
        let m = self.donald();
        let (eyes, beak) = face_numbers(m);
        let f = m.animator.face_playing().map(|p| p.map_or("-".to_string(), |(c, w)| format!("{c}x{w:.1}")));
        let row = self
            .h
            .rt
            .members
            .iter()
            .find(|x| x.0 == PartyChara::Donald)
            .and_then(|x| self.h.rt.party.view(x.1))
            .map_or("?", |v| v.anim.clip);
        format!("t {:5.2} {row:<18} eyes {eyes:5.1} beak {beak:4.1} | {} / {}", self.t, f[0], f[1])
    }

    /// `secs` with `buttons` held, a log line every 0.1 s.
    pub fn run(&mut self, secs: f32, buttons: &[PadButton], stick: Vec2) {
        let n = (secs / DT).round() as usize;
        for k in 0..n {
            self.step(buttons, stick);
            if k % 6 == 0 {
                let l = self.line();
                self.log.push(l);
            }
        }
    }
}

#[test]
#[ignore]
fn face_audit_donald_leading() {
    let Some(mut r) = Rig::new() else {
        println!("no party pack: skipped");
        return;
    };
    use PadButton::*;
    r.log.push("== idle".into());
    r.run(4.0, &[], Vec2::ZERO);
    r.log.push("== missile combo (A x3)".into());
    for _ in 0..3 {
        r.run(0.1, &[FaceBottom], Vec2::ZERO);
        r.run(0.35, &[], Vec2::ZERO);
    }
    r.run(2.0, &[], Vec2::ZERO);
    r.log.push("== LB + B (shortcut 2)".into());
    r.run(0.1, &[LeftShoulder], Vec2::ZERO);
    r.run(0.1, &[LeftShoulder, FaceRight], Vec2::ZERO);
    r.run(0.1, &[LeftShoulder], Vec2::ZERO);
    r.run(2.5, &[], Vec2::ZERO);
    r.log.push("== run".into());
    r.run(2.0, &[], Vec2::new(0.0, 1.0));
    r.run(1.0, &[], Vec2::ZERO);
    r.log.push("== jump".into());
    r.run(0.1, &[PadButton::JUMP], Vec2::ZERO);
    r.run(2.0, &[], Vec2::ZERO);
    r.log.push("== charge (A held)".into());
    r.run(1.5, &[FaceBottom], Vec2::ZERO);
    r.run(2.0, &[], Vec2::ZERO);
    r.log.push("== dodge (X + stick)".into());
    r.run(0.1, &[FaceLeft], Vec2::new(1.0, 0.0));
    r.run(1.5, &[], Vec2::ZERO);
    for l in &r.log {
        println!("{l}");
    }
}

// ------------------------------------------------------------------------------------------------- (FACE) the tests

use kh_anim::face_layer::VoiceInput;
use kh_party::face::{FaceClip, FaceView, Mood};

thread_local! {
    static TEST_FACE: std::cell::RefCell<FaceView> = std::cell::RefCell::new(FaceView::default());
}

fn set_test_face(v: FaceView) {
    TEST_FACE.with(|f| *f.borrow_mut() = v);
}

fn test_face() -> FaceView {
    TEST_FACE.with(|f| *f.borrow())
}

/// One step's face: (time after the step, eye gap cm, beak opening cm, the face slots' clips).
type FaceSample = (f32, f32, f32, [Option<String>; 2]);

/// Steps member `m` with body row `row` from clip time `t0` for `secs` at `dt`, `per_step` before each step (the test's
/// face view from `set_test_face`); the face after each step.
fn run_row(m: &mut PosedMember, row: &'static str, t0: f32, secs: f32, dt: f32, per_step: &mut dyn FnMut(&mut PosedMember, f32)) -> Vec<FaceSample> {
    let seq: &'static str = Box::leak(
        m.rows
            .get(row)
            .and_then(|r| r["file"].as_str())
            .map(|f| f.rsplit('/').next().unwrap().trim_end_matches(".glb").to_string())
            .unwrap_or_else(|| row.to_string())
            .into_boxed_str(),
    );
    let length = m.animator.clip_info(seq).map_or(1.0, |i| i.length);
    let mut out = Vec::new();
    let n = (secs / dt).round() as usize;
    for k in 0..n {
        let t = k as f32 * dt;
        per_step(m, t);
        let view = kh_party::AnimView {
            clip: row,
            sequence: seq,
            time: (t0 + t) % length,
            weight: 1.0,
            face: test_face(),
            ..Default::default()
        };
        m.set_pose_view(poser::DONALD, Vec3::ZERO, 0.0, dt, &view, true, true, &kh_anim::NoIkWorld);
        let (eyes, beak) = face_numbers(m);
        out.push((t + dt, eyes, beak, m.animator.face_playing().map(|p| p.map(|(c, _)| c.to_string()))));
    }
    out
}

fn donald() -> Option<PosedMember> {
    set_test_face(FaceView::default());
    load_member(&party_root()?, poser::DONALD)
}

/// A voice line of `parts` (seconds, loudness 0..1) at 60 Hz.
fn line(parts: &[(f32, f32)]) -> (std::sync::Arc<[u8]>, f32) {
    let v: Vec<u8> = parts.iter().flat_map(|&(s, l)| std::iter::repeat((l * 255.0) as u8).take((s * 60.0).round() as usize)).collect();
    (v.into(), 60.0)
}

#[test]
fn face_a_voice_line_without_its_clips_lips_opens_the_beak_at_any_frame_rate() {
    let mut runs = Vec::new();
    for hz in [30.0f32, 60.0, 120.0] {
        let Some(mut m) = donald() else {
            println!("no party pack: skipped");
            return;
        };
        let dt = 1.0 / hz;
        let (loud, rate) = line(&[(0.3, 0.0), (0.3, 1.0), (0.15, 0.0), (0.3, 0.45), (0.45, 0.0)]);
        let mut elapsed = -0.3f32;
        let mut step = |m: &mut PosedMember, _t: f32| {
            elapsed += dt;
            let v = (elapsed >= 0.0 && elapsed < 1.5).then(|| VoiceInput {
                id: 7,
                elapsed,
                length: 1.5,
                loudness: Some((loud.clone(), rate)),
            });
            m.set_voice(v);
        };
        // idle (IdleN carries no voice): the line is not the clip's
        let log = run_row(&mut m, "IdleN", 0.5, 2.4, dt, &mut step);
        let at = |t: f32| log.iter().find(|s| s.0 >= t - 1e-4).unwrap().clone();
        let quiet = at(0.25).2;
        let loud_beak = at(0.85).2;
        let pause = at(1.0).2;
        let soft = at(1.25).2;
        let after = at(2.35);
        println!(
            "{hz} Hz: beak quiet {quiet:.2} loud {loud_beak:.2} pause {pause:.2} soft {soft:.2} after {:.2} | mouth {:?} -> {:?}",
            after.2,
            at(0.85).3[1],
            after.3[1]
        );
        assert!(loud_beak > quiet + 3.0, "{hz} Hz: the loud syllable opens the beak wide ({quiet} -> {loud_beak})");
        assert!(soft > quiet + 1.0 && soft < loud_beak, "{hz} Hz: the soft one narrower ({soft})");
        assert!(pause < loud_beak - 2.0, "{hz} Hz: the pause closes it ({pause})");
        assert_eq!(after.3[1].as_deref(), Some("0000EM"), "{hz} Hz: the face's own lip loop after the line");
        assert!(!m.animator.face_held()[1], "the mouth is given back");
        runs.push((loud_beak, soft));
    }
    // frame-rate independent: the same opening at 30 / 60 / 120 Hz
    for r in &runs[1..] {
        assert!((r.0 - runs[0].0).abs() < 0.8 && (r.1 - runs[0].1).abs() < 0.8, "{runs:?}");
    }
}

#[test]
fn face_a_voice_line_its_clip_authored_is_left_to_the_clip() {
    let Some(mut m) = donald() else { return };
    let dt = 1.0 / 60.0;
    let (loud, rate) = line(&[(0.8, 1.0)]);
    // M_1301 from 0.40: its PlayVoice at 0.436, its own lips after it
    let mut step = |m: &mut PosedMember, t: f32| {
        let e = t - 0.04;
        m.set_voice((e >= 0.0 && e < 0.8).then(|| VoiceInput {
            id: 3,
            elapsed: e,
            length: 0.8,
            loudness: Some((loud.clone(), rate)),
        }));
    };
    let log = run_row(&mut m, "M_1301", 0.40, 0.5, dt, &mut step);
    assert!(m.face_layer.lip_state().1, "the layer leaves it to the clip: {}", m.face_layer.said);
    assert!(!m.animator.face_held()[1]);
    assert!(
        log.iter().any(|s| s.3[1].as_deref() == Some("0052M")),
        "KH3's own MOUTH_OPEN_03 of M_1301 at 0.472: {:?}",
        log.iter().map(|s| s.3[1].clone()).collect::<Vec<_>>()
    );
}

#[test]
fn face_a_line_without_loudness_talks_with_kh3s_chatter_lip_row() {
    let Some(mut m) = donald() else { return };
    let dt = 1.0 / 60.0;
    let mut step = |m: &mut PosedMember, t: f32| {
        m.set_voice((t < 1.0).then(|| VoiceInput {
            id: 9,
            elapsed: t,
            length: 1.0,
            loudness: None,
        }))
    };
    let log = run_row(&mut m, "IdleN", 0.0, 1.5, dt, &mut step);
    let mid = &log[30];
    assert_eq!(mid.3[1].as_deref(), Some("n_ex001_fm003"), "TA_DE_M_03 (KH3's field-chatter lip) on the mouth: {:?}", mid.3);
    let beaks: Vec<f32> = log[5..60].iter().map(|s| s.2).collect();
    let (lo, hi) = beaks.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    assert!(hi - lo > 2.0, "the beak flaps ({lo} .. {hi})");
    assert_eq!(log.last().unwrap().3[1].as_deref(), Some("0000EM"), "closed after the line");
}

#[test]
fn face_a_talk_line_plays_kh3s_field_voice_face_with_npc_blinks_at_any_frame_rate() {
    let mut counts = Vec::new();
    for hz in [30.0f32, 60.0, 144.0] {
        let Some(mut m) = donald() else { return };
        let dt = 1.0 / hz;
        let mut slot = kh_party::face::FaceSlot::default();
        slot.talk(Mood::Default, 10.0);
        set_test_face(slot.view());
        let mut step = |_m: &mut PosedMember, _t: f32| {
            set_test_face(slot.view());
            slot.step(dt);
        };
        let log = run_row(&mut m, "TalkLoopN", 0.0, 11.0, dt, &mut step);
        let eyes_seen: std::collections::BTreeSet<String> = log.iter().filter_map(|s| s.3[0].clone()).collect();
        let mid = log.iter().find(|s| s.0 >= 1.0).unwrap();
        assert_eq!(mid.3, [Some("n_ex001_fe000".to_string()), Some("n_ex001_fm001".to_string())], "TA_DE_E_00 / TA_DE_M_01 during the line");
        assert!(eyes_seen.contains("n_ex001_fe001"), "TA_DE_E_01 blinks: {eyes_seen:?}");
        let closed = log.iter().filter(|s| s.1 < 15.0).count();
        assert!(closed > 0, "the lids close at a blink");
        let end = log.last().unwrap();
        assert_eq!(end.3[1].as_deref(), Some("0000EM"), "NORMAL_AUTO after the line (FUN_1414f6ed0)");
        println!("{hz} Hz: {} blinks in 10 s (KH3: every 3 - 4 s), eyes {eyes_seen:?}", m.face_layer.blinks);
        counts.push(m.face_layer.blinks);
        set_test_face(FaceView::default());
    }
    assert!(counts.iter().all(|c| (2..=3).contains(c)), "a blink every 3 - 4 s at every rate: {counts:?}");
    assert!(counts.windows(2).all(|w| w[0] == w[1]), "the same blinks at every frame rate: {counts:?}");
}

#[test]
fn face_the_clip_slot_holds_the_face_against_the_bodys_own_plays() {
    let Some(mut m) = donald() else { return };
    let dt = 1.0 / 60.0;
    // a face clip of the pack on the slot while he runs (RunLoopN plays MOUTH_OPEN on every stride)
    let clip = FaceClip {
        sequence: "0811_face",
        time: 0.0,
        blend_in: 0.1,
        length: 2.0,
        looping: false,
        serial: 5,
    };
    let mut step = |_m: &mut PosedMember, t: f32| {
        set_test_face(FaceView {
            clip: (t < 1.0).then_some(clip),
            blend_out: 0.15,
            ..Default::default()
        })
    };
    let log = run_row(&mut m, "RunLoopN", 0.0, 1.6, dt, &mut step);
    assert!(
        log[10..58].iter().all(|s| s.3 == [Some("0811_face".to_string()), Some("0811_face".to_string())]),
        "the clip keeps both slots: {:?}",
        log[10..58].iter().map(|s| s.3.clone()).collect::<Vec<_>>()
    );
    assert!(
        log[70..].iter().any(|s| s.3[1].as_deref().is_some_and(|c| c != "0811_face")),
        "the stride's own mouth plays come back after it"
    );
    assert_eq!(m.animator.face_held(), [false, false]);
    set_test_face(FaceView::default());
}

#[test]
fn face_a_clip_played_backwards_fires_no_face_plays() {
    let Some(mut m) = donald() else { return };
    let dt = 1.0 / 60.0;
    // RunLoopN (0.6 s, MOUTH_OPEN at 0.07 / 0.21 / 0.39 / 0.49) played backwards from 0.55
    let mut changes = 0;
    let mut t = 0.55f32;
    let mut last: Option<[Option<String>; 2]> = None;
    for _ in 0..20 {
        let view = kh_party::AnimView {
            clip: "RunLoopN",
            sequence: "0003",
            time: t,
            weight: 1.0,
            ..Default::default()
        };
        m.set_pose_view(poser::DONALD, Vec3::ZERO, 0.0, dt, &view, true, true, &kh_anim::NoIkWorld);
        let now = m.animator.face_playing().map(|p| p.map(|(c, _)| c.to_string()));
        if last.as_ref().is_some_and(|l| *l != now) {
            changes += 1;
        }
        last = Some(now);
        t -= dt * 0.5;
    }
    assert!(changes <= 1, "no notify storm while the clip runs backwards ({changes} face changes)");
}

/// THE COVERAGE: every body row of Donald's pack and the face its own notifies give it (KH3's pairing of a body clip with
/// its face). The rows with none are the ones listed: the side samples of blend spaces (their centre sample carries the
/// face), M_0230 (keeps the face of the reaction before it), the talk rows (the talk face), the camp / photo rows (their
/// own face rows).
#[test]
fn face_every_body_row_of_donald_and_goofy_has_a_face_source() {
    let Some(root) = party_root() else { return };
    for npc in ["n_ex001", "n_ex002"] {
        let text = std::fs::read_to_string(root.join(npc).join("manifest.json")).unwrap();
        let m: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap();
        let items: std::collections::HashMap<&str, &serde_json::Value> = m["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["kind"] == "animation")
            .filter_map(|i| Some((i["animation_name"].as_str()?, i)))
            .collect();
        let mut table = Vec::new();
        let mut without = Vec::new();
        for r in m["logical_animations"].as_array().unwrap() {
            let (Some(name), Some(file)) = (r["name"].as_str(), r["file"].as_str()) else { continue };
            let seq = file.rsplit('/').next().unwrap().trim_end_matches(".glb");
            let Some(item) = items.get(seq) else { continue };
            let faces: Vec<String> = item["notifies"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|n| n["class"].as_str().is_some_and(|c| c.starts_with("TresAnimNotify_FaceAnim")))
                .map(|n| {
                    let p = &n["properties"];
                    let what = ["FaceAnimName", "m_AnimName", "FaceAnimType", "m_AnimType"]
                        .iter()
                        .find_map(|k| p[*k].as_str())
                        .unwrap_or("NORMAL_AUTO");
                    let class = n["class"].as_str().unwrap().trim_start_matches("TresAnimNotify_FaceAnim");
                    format!("{}@{:.2}:{what}", if class.is_empty() { "Both" } else { class }, n["trigger_time"].as_f64().unwrap_or(0.0))
                })
                .collect();
            if faces.is_empty() {
                without.push(name.to_string());
            }
            table.push(format!("{name:<24} {seq:<8} {}", if faces.is_empty() { "-".to_string() } else { faces.join(" ") }));
        }
        let _ = std::fs::write(std::env::temp_dir().join(format!("kh_face_coverage_{npc}.txt")), table.join("\n"));
        println!("{npc}: {} rows, {} without a face notify: {without:?}", table.len(), without.len());
        // ZettaCast: its face is ZETTA's face-clip slot (face/n_ex001_70291w_face); ZettaChannel: none (KH3's event has no
        // face track for n_ex001_70286wr either: the face it had stays)
        let known = |n: &str| n.starts_with("TalkAct") || n == "TalkLoopN" || n == "M_0230" || n.ends_with("_CM") || n.ends_with("_GM") || n == "ZettaCast" || n == "ZettaChannel";
        let unexplained: Vec<&String> = without.iter().filter(|n| !known(n)).collect();
        assert!(unexplained.is_empty(), "{npc}: rows with no face source: {unexplained:?}");
    }
}

/// THE VOICE GATE: Aero Float plays MagicAirCast as a pose (its MagicCast line is not said); its own lip plays
/// (MOUTH_OPEN_05 at 0.17, MOUTH_OPEN_03 at 0.57) would move the beak to a silence. With the host feeding his voice
/// lines and none sounding, the beak stays shut after the line's time; without a feed (no audio side) KH3's lips stay.
#[test]
fn face_a_clips_own_lips_wait_for_its_line_to_sound() {
    let mut widest = Vec::new();
    for fed in [true, false] {
        let Some(mut m) = donald() else { return };
        let dt = 1.0 / 60.0;
        let mut step = |m: &mut PosedMember, _t: f32| {
            if fed {
                m.set_voice(None);
            }
        };
        let log = run_row(&mut m, "MagicAirCast", 0.0, 0.9, dt, &mut step);
        let beak = log.iter().filter(|s| s.0 > 0.2).map(|s| s.2).fold(0.0f32, f32::max);
        println!("fed {fed}: widest beak after the line's time {beak:.2}; layer: {}", m.face_layer.said);
        widest.push(beak);
    }
    assert!(widest[0] < 5.0, "fed and silent: the beak stays shut ({})", widest[0]);
    assert!(widest[1] > 8.0, "no audio side: KH3's own lips ({})", widest[1]);
}

/// The named faces a voiced clip plays are its line's face: Flare Force's COO_FF_0_EL (1705EM) and the damage mouth
/// DA_0_Lip of M_0211; the line keeps them (no lips of the layer over them).
#[test]
fn face_a_voiced_clips_named_face_is_its_lines_face() {
    for (row, t0, voice_at) in [("FForceFinish", 0.35f32, 0.05f32), ("M_0211", 0.0, 0.0)] {
        let Some(mut m) = donald() else { return };
        let dt = 1.0 / 60.0;
        let (loud, rate) = line(&[(0.6, 1.0)]);
        let mut step = |m: &mut PosedMember, t: f32| {
            let e = t - voice_at;
            m.set_voice((e >= 0.0 && e < 0.6).then(|| VoiceInput { id: 11, elapsed: e, length: 0.6, loudness: Some((loud.clone(), rate)) }));
        };
        let log = run_row(&mut m, row, t0, 0.5, dt, &mut step);
        assert!(m.face_layer.lip_state().1, "{row}: the clip's own face ({})", m.face_layer.said);
        assert!(!log.iter().any(|s| s.3[1].as_deref().is_some_and(|c| ["0050M", "0051M", "0052M", "0054M"].contains(&c)) && row == "M_0211"), "{row}: no layer lips");
        println!("{row}: {} / mouth {:?}", m.face_layer.said, log.last().unwrap().3[1]);
    }
}

// ------------------------------------------------------------------------------------- (FACE) the head's look-up cap

/// One look-up run of `npc`'s animator (IdleN, the actor at the origin facing +X): the focus at `focus(t)` for `secs`;
/// per step (head pitch deg = the angle of the head bone against the same step without a look, the eye input pitch deg).
fn look_run(npc: u8, cap: Option<f32>, secs: f32, focus: &dyn Fn(f32) -> glam::Vec3) -> Option<Vec<(f32, f32, f32)>> {
    let root = party_root()?;
    let mut with = load_member(&root, npc)?.animator;
    let mut without = load_member(&root, npc)?.animator;
    with.look_at_mut()?.set_head_up_max(cap);
    let head = with.skeleton().index_of("atama")?;
    let dt = 1.0 / 60.0;
    let seq = with.clip_info("0001").map(|_| "0001").unwrap_or("0001");
    let names = [seq];
    let mut out = Vec::new();
    let mut time = 0.0f32;
    let c2w = kh_anim::Trs { translation: Vec3::ZERO, rotation: glam::Quat::from_rotation_z((-90.0f32).to_radians()), scale: Vec3::ONE };
    for k in 0..(secs / dt) as usize {
        let t = k as f32 * dt;
        let from = if k == 0 { -1.0 } else { time };
        time += dt;
        with.set_look_focus(Some(focus(t)));
        without.set_look_focus(None);
        for a in [&mut with, &mut without] {
            let body = kh_anim::BodySlot { samples: [(seq, 1.0), (seq, 0.0)], blend_with: None, notify_sequences: &names, time, weight: 1.0, active: true, notify_window: (from, time) };
            a.step(&kh_anim::Frame { dt, body, component_to_world: c2w }, &kh_anim::NoIkWorld);
        }
        let (h1, h0) = (with.pose().component[head].rotation, without.pose().component[head].rotation);
        out.push((t, h1.angle_between(h0).to_degrees(), with.look_at().map_or(0.0, |l| l.eye.x)));
    }
    Some(out)
}

#[test]
fn face_the_heads_look_up_stops_at_its_cap_and_the_eyes_look_the_rest() {
    for (npc, name) in [(poser::DONALD, "Donald"), (poser::GOOFY, "Goofy")] {
        // a point 6 m above, 1 m in front of him
        let high = |_t: f32| Vec3::new(100.0, 0.0, 600.0);
        let Some(kh3) = look_run(npc, None, 2.0, &high) else {
            println!("no party pack: skipped");
            return;
        };
        let capped = look_run(npc, Some(poser::LOOK_HEAD_UP_MAX_DEG), 2.0, &high).unwrap();
        let (k, c) = (kh3.last().unwrap(), capped.last().unwrap());
        println!("{name}: target 6 m up: KH3's head {:.1} deg (eyes {:.1}), capped head {:.1} deg (eyes {:.1})", k.1, k.2, c.1, c.2);
        assert!(k.1 > poser::LOOK_HEAD_UP_MAX_DEG + 10.0, "{name}: KH3's limit tips the head far up ({})", k.1);
        assert!(capped.iter().all(|s| s.1 <= poser::LOOK_HEAD_UP_MAX_DEG + 1.0), "{name}: the head never passes the cap");
        assert!(c.1 > poser::LOOK_HEAD_UP_MAX_DEG - 3.0, "{name}: the head goes to the cap ({})", c.1);
        assert!(c.2 > k.2 + 15.0, "{name}: the eyes take the rest ({} vs KH3 {})", c.2, k.2);
        // a low target: the cap changes nothing (down / ahead stay KH3's)
        let low = |_t: f32| Vec3::new(300.0, 0.0, 20.0);
        let (a, b) = (look_run(npc, None, 1.5, &low).unwrap(), look_run(npc, Some(poser::LOOK_HEAD_UP_MAX_DEG), 1.5, &low).unwrap());
        assert!((a.last().unwrap().1 - b.last().unwrap().1).abs() < 0.2, "{name}: a target below is as KH3 looks at it");
        // the target rising slowly from his chest to 8 m above: the head stops, the eyes go on, no pop
        let rising = |t: f32| Vec3::new(150.0, 0.0, 60.0 + 250.0 * t);
        let r = look_run(npc, Some(poser::LOOK_HEAD_UP_MAX_DEG), 3.2, &rising).unwrap();
        let jumps = r.windows(2).map(|w| ((w[1].1 - w[0].1).abs(), (w[1].2 - w[0].2).abs())).fold((0.0f32, 0.0f32), |m, d| (m.0.max(d.0), m.1.max(d.1)));
        println!("{name}: rising target: largest step of the head {:.2} deg, of the eye input {:.2} deg; end head {:.1} eyes {:.1}", jumps.0, jumps.1, r.last().unwrap().1, r.last().unwrap().2);
        assert!(jumps.0 < 3.5 && jumps.1 < 3.5, "{name}: a smooth hand-off ({jumps:?})");
        assert!(r.last().unwrap().2 > 20.0, "{name}: the eyes end high");
    }
}

// ------------------------------------------------------------------------- (FACE) the eyes look around (every direction)

/// A run of `npc`'s animator on body clip `seq` (the actor at the origin facing +X), the focus at `focus(t)`, at `dt`:
/// per step (t, head angle deg against no look, eye input (pitch, yaw) deg, the right eyeball's turn deg against no look).
fn eye_run(npc: u8, seq: &str, secs: f32, dt: f32, focus: &dyn Fn(f32) -> Vec3) -> Option<Vec<(f32, f32, glam::Vec2, f32)>> {
    let root = party_root()?;
    let mut with = load_member(&root, npc)?.animator;
    let mut without = load_member(&root, npc)?.animator;
    let head = with.skeleton().index_of("atama")?;
    let eye = with.skeleton().index_of("R_eye")?;
    let names = [seq];
    let c2w = kh_anim::Trs { translation: Vec3::ZERO, rotation: glam::Quat::from_rotation_z((-90.0f32).to_radians()), scale: Vec3::ONE };
    let length = with.clip_info(seq)?.length;
    let mut out = Vec::new();
    let mut time = 0.0f32;
    for k in 0..(secs / dt).round() as usize {
        let t = k as f32 * dt;
        let from = if k == 0 { -1.0 } else { time };
        time = (time + dt) % length;
        with.set_look_focus(Some(focus(t)));
        without.set_look_focus(None);
        for a in [&mut with, &mut without] {
            let body = kh_anim::BodySlot { samples: [(seq, 1.0), (seq, 0.0)], blend_with: None, notify_sequences: &names, time, weight: 1.0, active: true, notify_window: (from, time) };
            a.step(&kh_anim::Frame { dt, body, component_to_world: c2w }, &kh_anim::NoIkWorld);
        }
        let (w, n) = (with.pose(), without.pose());
        let e = with.look_at().map_or(glam::Vec2::ZERO, |l| glam::Vec2::new(l.eye.x, l.eye.y));
        // the eyeball relative to its head: its own turn, not the head's
        let rel = |p: &kh_anim::Pose| p.component[eye].rotation * p.component[head].rotation.inverse();
        out.push((t + dt, w.component[head].rotation.angle_between(n.component[head].rotation).to_degrees(), e, rel(w).angle_between(rel(n)).to_degrees()));
    }
    Some(out)
}

#[test]
fn face_the_eyes_look_around_in_every_direction_with_any_face() {
    // left / right / up / down of him at 3 m (his eyes at about 0.8 m): within the head's limits
    let dirs = [("left", Vec3::new(260.0, 150.0, 80.0)), ("right", Vec3::new(260.0, -150.0, 80.0)), ("up", Vec3::new(260.0, 0.0, 260.0)), ("down", Vec3::new(260.0, 0.0, -60.0))];
    for (npc, name) in [(poser::GOOFY, "Goofy"), (poser::DONALD, "Donald")] {
        // IdleN plays the eye-look grid (NO_0_Eye); IdleB the battle face BA_0_EL (no grid: the eyes' overlay)
        for seq in ["0001", "1015"] {
            let mut finals = Vec::new();
            for (d, p) in dirs {
                let Some(r) = eye_run(npc, seq, 2.0, 1.0 / 60.0, &|_| p) else {
                    println!("no party pack: skipped");
                    return;
                };
                let f = *r.last().unwrap();
                println!("{name} {seq} {d:<5}: head {:5.1} deg, eye input ({:5.1}, {:5.1}), eyeball turned {:4.1} deg", f.1, f.2.x, f.2.y, f.3);
                assert!(f.3 > 2.0, "{name} {seq} {d}: his eyeballs turn toward it ({})", f.3);
                // the battle idle has KH3's LookAt notify state that turns the head's look-at off: the eyes look alone
                if seq == "0001" {
                    assert!(f.1 > 3.0, "{name} {seq} {d}: the head takes its share ({})", f.1);
                }
                finals.push(f.2);
            }
            // the eye input points the right way: left / right opposite yaw, up / down opposite pitch
            assert!(finals[0].y * finals[1].y < 0.0 && finals[2].x * finals[3].x < 0.0, "{name} {seq}: directions {finals:?}");
        }
    }
}

#[test]
fn face_the_eyes_lead_and_the_head_follows_at_any_frame_rate() {
    for (npc, name) in [(poser::GOOFY, "Goofy"), (poser::DONALD, "Donald")] {
        let mut ends = Vec::new();
        for hz in [30.0f32, 60.0, 144.0] {
            // the target jumps from ahead to 50 deg to his left at t = 1 s
            let jump = |t: f32| if t < 1.0 { Vec3::new(300.0, 0.0, 80.0) } else { Vec3::new(190.0, 230.0, 80.0) };
            let Some(r) = eye_run(npc, "0001", 3.0, 1.0 / hz, &jump) else { return };
            let at = |t: f32| r.iter().find(|s| s.0 >= t - 1e-4).copied().unwrap();
            // the eyes' widest turn in the second after the jump (KH3's look-location delay ring comes first)
            let early = r.iter().filter(|s| s.0 > 1.0 && s.0 < 2.0).copied().max_by(|a, b| a.2.y.abs().total_cmp(&b.2.y.abs())).unwrap();
            let (before, late) = (at(0.95), at(2.9));
            println!("{name} {hz} Hz: eye yaw {:.1} -> {:.1} (0.12 s after the jump) -> {:.1}; head {:.1} -> {:.1} -> {:.1}", before.2.y, early.2.y, late.2.y, before.1, early.1, late.1);
            assert!(early.2.y.abs() > late.2.y.abs() + 5.0, "{name} {hz} Hz: the eyes lead (they turn first, more than they keep)");
            assert!(early.1 < late.1 * 0.6, "{name} {hz} Hz: the head follows later");
            ends.push((late.1, late.2));
        }
        for e in &ends[1..] {
            assert!((e.0 - ends[0].0).abs() < 0.5 && (e.1 - ends[0].1).length() < 0.5, "{name}: the same at every frame rate {ends:?}");
        }
    }
}

// ------------------------------------------------------------------------------- (FACE) he looks at his cast's target

/// (FACE) One sample of Donald's look per step: his cast look, the look-at's focus point, the look rate, the head turn
/// (`current`: pitch, yaw deg), the eye input (pitch, yaw deg), the body row, a Zettaflare under way.
#[derive(Clone, Debug)]
struct LookSample {
    t: f32,
    cast: Option<kh_party::donald::cast_look::CastLook>,
    point: Vec3,
    rate: f32,
    head: Vec3,
    eye: Vec3,
    row: String,
    zetta: bool,
}

impl Rig {
    fn donald_id(&self) -> kh_party::MemberId {
        self.h.rt.id(PartyChara::Donald).unwrap()
    }
    fn donald_view(&self) -> kh_party::MemberView {
        let id = self.donald_id();
        self.h.rt.current.iter().find(|v| v.id == id).cloned().unwrap()
    }
    fn look_sample(&self) -> LookSample {
        let p = self.h.rt.party.member(self.donald_id()).unwrap();
        let l = self.donald().animator.look_at().unwrap();
        LookSample {
            t: self.t,
            cast: kh_party::donald::cast_look::cast_look(p),
            point: l.point,
            rate: l.rate,
            head: l.current,
            eye: l.eye,
            row: self.donald_view().anim.clip.to_string(),
            zetta: kh_party::donald::cast_look::zettaflare(p),
        }
    }
    /// `secs` of steps without buttons, one sample per step.
    fn look_run(&mut self, secs: f32, buttons: &[PadButton]) -> Vec<LookSample> {
        let n = (secs / DT).round() as usize;
        (0..n)
            .map(|_| {
                self.step(buttons, Vec2::ZERO);
                self.look_sample()
            })
            .collect()
    }
    /// A place `d` cm to his side (`left` = +90 deg from his facing), on the floor.
    fn beside(&self, d: f32, left: bool) -> Vec3 {
        let v = self.donald_view();
        let y = (v.yaw + if left { 90.0 } else { -90.0 }).to_radians();
        Vec3::new(v.position.x + y.cos() * d, v.position.y + y.sin() * d, 0.0)
    }
}

fn print_looks(name: &str, s: &[LookSample]) {
    for x in s.iter().step_by(6) {
        println!(
            "{name} t {:5.2} {:<22} cast {:<14} point ({:6.0},{:6.0},{:4.0}) rate {:.2} head ({:5.1},{:5.1}) eyes ({:5.1},{:5.1}){}",
            x.t,
            x.row,
            x.cast.map_or("-".to_string(), |c| format!("{c:?}")).chars().take(14).collect::<String>(),
            x.point.x,
            x.point.y,
            x.point.z,
            x.rate,
            x.head.x,
            x.head.y,
            x.eye.x,
            x.eye.y,
            if x.zetta { " ZETTA" } else { "" }
        );
    }
}

#[test]
fn face_donald_looks_at_the_enemy_he_casts_at_pad_and_ai() {
    for pad in [false, true] {
        let Some(mut r) = Rig::led_by(pad) else {
            println!("no party pack: skipped");
            return;
        };
        let who = if pad { "pad" } else { "AI" };
        r.look_run(1.0, &[]);
        // one enemy 4 m to his left (off to the side)
        let foe = r.beside(400.0, true);
        r.foe = Some(foe);
        r.look_run(0.5, &[]);
        println!("{}", r.h.rt.word("party don-cast l1.fire.1"));
        let s = r.look_run(3.0, &[]);
        print_looks(who, &s);
        let during: Vec<&LookSample> = s.iter().filter(|x| x.cast.is_some()).collect();
        assert!(during.len() > 10, "{who}: the cast runs ({})", during.len());
        for x in &during {
            assert_eq!(x.cast, Some(kh_party::donald::cast_look::CastLook::Enemy(TargetId(1))), "{who}: the cast's target is the foe");
            assert!((x.point - (foe + Vec3::Z * 90.0)).length() < 1.0, "{who} t {}: he looks at the foe's centre, nothing else ({:?})", x.t, x.point);
        }
        // the look is on (head and eyes) during the cast
        let last = during.last().unwrap();
        assert!(last.rate > 0.5, "{who}: the look is on during the cast ({})", last.rate);
    }
}

#[test]
fn face_aero_barrier_on_sora_looks_at_sora_not_the_enemy() {
    let Some(mut r) = Rig::led_by(false) else {
        println!("no party pack: skipped");
        return;
    };
    r.look_run(1.0, &[]);
    // a foe on his right; Sora's look point (the host's "t_camera" bone) behind-left of him
    r.foe = Some(r.beside(400.0, false));
    let sora = r.beside(250.0, true) + Vec3::Z * 140.0;
    r.h.rt.sora_look_point = Some(sora);
    r.look_run(0.5, &[]);
    println!("{}", r.h.rt.word("party don-cast l2.aero.1 sora"));
    let s = r.look_run(3.0, &[]);
    print_looks("aero", &s);
    let during: Vec<&LookSample> = s.iter().filter(|x| x.cast.is_some()).collect();
    assert!(during.len() > 10, "the barrier cast runs ({})", during.len());
    for x in &during {
        assert_eq!(x.cast, Some(kh_party::donald::cast_look::CastLook::Sora));
        assert!((x.point - sora).length() < 1.0, "t {}: he looks at Sora, not the enemy ({:?})", x.t, x.point);
    }
    // after the cast the normal look comes back smoothly: no head jump from one step to the next
    let end = s.iter().position(|x| x.cast.is_none() && x.t > during.last().unwrap().t).unwrap();
    for w in s[end.saturating_sub(1)..].windows(2) {
        // the head's turn as drawn: the node's rotation x LookAtRate
        let d = (w[1].head * w[1].rate - w[0].head * w[0].rate).length();
        assert!(d < 6.0, "t {}: the head turns back smoothly ({d:.1} deg in a step)", w[1].t);
    }
}

#[test]
fn face_zettaflare_keeps_its_own_look() {
    // Zettaflare is the player-led Donald's only
    let Some(mut r) = Rig::led_by(true) else {
        println!("no party pack: skipped");
        return;
    };
    r.look_run(1.0, &[]);
    r.foe = Some(r.beside(600.0, true));
    r.look_run(0.5, &[]);
    println!("{}", r.h.rt.word("party don-zetta-ready"));
    println!("{}", r.h.rt.word("party don-cast y.zettaflare.1"));
    let s = r.look_run(6.0, &[]);
    print_looks("zetta", &s);
    let zetta: Vec<&LookSample> = s.iter().filter(|x| x.zetta).collect();
    assert!(!zetta.is_empty(), "Zettaflare runs");
    assert!(zetta.iter().all(|x| x.cast.is_none()), "no cast look over Zettaflare");
}

#[test]
fn face_his_missiles_look_at_their_target() {
    let Some(mut r) = Rig::led_by(true) else {
        println!("no party pack: skipped");
        return;
    };
    r.look_run(1.0, &[]);
    // a foe ahead-left of him (the soft target)
    let v = r.donald_view();
    let y = (v.yaw + 30.0).to_radians();
    let foe = Vec3::new(v.position.x + y.cos() * 350.0, v.position.y + y.sin() * 350.0, 0.0);
    r.foe = Some(foe);
    r.look_run(0.5, &[]);
    let mut s = Vec::new();
    for _ in 0..3 {
        s.extend(r.look_run(0.1, &[PadButton::FaceBottom]));
        s.extend(r.look_run(0.35, &[]));
    }
    print_looks("missile", &s);
    let during: Vec<&LookSample> = s.iter().filter(|x| x.cast.is_some()).collect();
    assert!(during.len() > 10, "the missiles fire ({})", during.len());
    for x in &during {
        assert_eq!(x.cast, Some(kh_party::donald::cast_look::CastLook::Enemy(TargetId(1))), "t {}: the missiles' target", x.t);
        assert!((x.point - (foe + Vec3::Z * 90.0)).length() < 1.0, "t {}: he looks at it", x.t);
    }
}
