//! (ZETTA round 5) THE TEST LEVEL'S ENEMY CHOICE DURING ZETTAFLARE (the user: "enemies don't ever target Donald while he
//! is performing Zettaflare"; "Donald is invincible during Zettaflare's cutscene"): from the channel's start until control
//! returns no fixture enemy chooses the leading Donald (it takes the nearest other member), no hit lands on him (his body
//! is marked invulnerable for the hosts); afterwards he is chosen and hit as ever.

use kh_core::Vec3;
use kh_host::party::PartyChara;

use crate::k1_tests::Harness;
use crate::runtime::Victim;

/// (round 5) one enemy 7 m before the leader (Zettaflare needs a target): the harness's step with it
#[derive(Clone, Copy)]
struct OneEnemy {
    at: Vec3,
}

impl kh_core::Targets for OneEnemy {
    fn enumerate(&self, out: &mut Vec<kh_core::host::TargetInfo>) {
        out.extend(self.info(kh_core::TargetId(1)));
    }
    fn info(&self, id: kh_core::TargetId) -> Option<kh_core::host::TargetInfo> {
        use kh_core::host::TargetFlags;
        (id.0 == 1).then(|| kh_core::host::TargetInfo { id, position: self.at, center: self.at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 })
    }
    fn hurt_shapes(&self, _id: kh_core::TargetId, _out: &mut Vec<kh_core::host::HurtShape>) {}
    fn apply_hit(&mut self, _hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
        kh_core::host::HitResult { accepted: true, ..Default::default() }
    }
}

impl crate::PartyTargets for OneEnemy {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn step_with(h: &mut Harness, foe: OneEnemy) {
    let (lp, ly) = h.leader_pos();
    let (s, c) = ly.to_radians().sin_cos();
    let camera = kh_camera::CameraView { position: lp - Vec3::new(c, s, 0.0) * 450.0 + Vec3::Z * 220.0, rotation: kh_camera::Rot { pitch: -15.0, yaw: ly, roll: 0.0 }, rotation_target: kh_camera::Rot { pitch: -15.0, yaw: ly, roll: 0.0 }, look_point: lp, distance: 450.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
    let input = kh_core::Input { camera_yaw: ly, ..kh_core::Input::default() };
    let dt = 1.0 / 60.0;
    let input = h.rt.route(crate::RouteIn { sora: &mut h.sora, input: &input, dt, targets: &foe, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: h.t });
    let mut targets = foe;
    let mut audio = kh_core::host::audio::NullAudio;
    let mut effects = kh_core::host::effects::NullEffects;
    let mut ui = kh_core::host::NullUi;
    h.out = {
        let mut st = crate::SoraTargets { targets: &mut targets, runtime: &mut h.rt };
        let mut host = kh_core::Host { world: &h.world, targets: &mut st, audio: &mut audio, effects: &mut effects, ui: &mut ui };
        h.sora.step_hosted(dt, &input, &mut host)
    };
    let _ = h.rt.step(crate::StepIn { dt, sora: &mut h.sora, out: &h.out, lock_on: kh_core::TargetId::NONE, camera: &camera, world: &h.world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: h.t });
    let _ = h.rt.after_step(&foe, dt, h.t);
    h.t += dt as f64;
}

#[test]
fn zetta_enemies_never_choose_donald_and_he_is_invincible_through_the_sequence() {
    let Some(mut h) = Harness::new() else {
        println!("no party AI pack: skipped");
        return;
    };
    h.rt.lead_word(&["leader", PartyChara::Donald.name()]);
    for _ in 0..30 {
        h.step(None);
    }
    let don = h.rt.leader.expect("Donald leads");
    let at = |h: &Harness| h.rt.party.view(don).unwrap().position;
    // before: of fifty enemies beside him some choose him (his aggro's rolls: DON-PROG)
    let chosen = |h: &Harness| (1..=50).filter(|n| h.rt.victim(*n, at(h) + Vec3::new(60.0, 0.0, 0.0), h.out.position) == Victim::Friend(PartyChara::Donald)).count();
    assert!(chosen(&h) > 0, "chosen before");
    let (lp, ly) = h.leader_pos();
    let foe = OneEnemy { at: lp + Vec3::new(ly.to_radians().cos(), ly.to_radians().sin(), 0.0) * 700.0 };
    h.rt.word("party don-zetta-ready");
    h.rt.word("party don-cast y.zettaflare.1");
    let mut during = 0;
    for _ in 0..(40.0 * 60.0) as usize {
        step_with(&mut h, foe);
        let on = h.rt.party.don_zetta_invincible(don);
        if !on && during > 0 {
            break;
        }
        if on {
            during += 1;
            assert_eq!(chosen(&h), 0, "never Donald during the sequence");
            let lv = h.rt.leader_view_of(&h.out, h.sora.status());
            assert!(lv.invulnerable, "his body refuses hits");
            let hp = h.rt.party.status(don).unwrap().hp;
            let r = h.rt.party.receive_hit(don, &kh_core::HitRecord { damage: 30.0, ..kh_core::HitRecord::empty(kh_core::TargetId(1)) });
            assert!(!r.accepted && h.rt.party.status(don).unwrap().hp == hp, "no damage");
        }
    }
    assert!(during > 60 * 20, "the whole sequence: {during} steps");
    // control back: chosen again
    assert!(chosen(&h) > 0, "chosen again after");
}

/// (ZETTA round 5) the user: "the camera after zettaflare should just be placed behind Donald, not where it originally was":
/// the step its camera begins handing back, `team_camera` asks the host for the field camera behind the leader (once), and
/// the leader then faces where the beam went (his facing is what the field camera's reset sits behind)
#[test]
fn zetta_the_camera_goes_behind_donald_as_it_hands_back() {
    let Some(mut h) = Harness::new() else {
        println!("no party AI pack: skipped");
        return;
    };
    h.rt.lead_word(&["leader", PartyChara::Donald.name()]);
    for _ in 0..30 {
        h.step(None);
    }
    let don = h.rt.leader.expect("Donald leads");
    let (lp, ly) = h.leader_pos();
    let foe = OneEnemy { at: lp + Vec3::new(ly.to_radians().cos(), ly.to_radians().sin(), 0.0) * 700.0 };
    h.rt.word("party don-zetta-ready");
    h.rt.word("party don-cast y.zettaflare.1");
    let mut asked = Vec::new();
    let mut beam_yaw = None;
    for k in 0..(40.0 * 60.0) as usize {
        step_with(&mut h, foe);
        if let Some(b) = h.rt.party.don_zetta_runs().first().and_then(|r| r.beam_at_hit) {
            beam_yaw = Some(b.x.y.atan2(b.x.x).to_degrees());
        }
        let (lp, ly) = h.leader_pos();
        let (s, c) = ly.to_radians().sin_cos();
        let mut field = kh_camera::CameraView { position: lp - Vec3::new(c, s, 0.0) * 450.0 + Vec3::Z * 220.0, rotation: kh_camera::Rot { pitch: -15.0, yaw: ly + 120.0, roll: 0.0 }, rotation_target: kh_camera::Rot { pitch: -15.0, yaw: ly + 120.0, roll: 0.0 }, look_point: lp, distance: 450.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let out = h.out.clone();
        let reset = h.rt.team_camera(1.0 / 60.0, &[], &out, &mut field, &h.world, kh_camera::CameraInput::default());
        if h.rt.zetta_behind {
            assert!(reset, "team_camera asks for the reset (Elden Ring's route)");
            asked.push((k, ly));
        }
        if !h.rt.party.don_zetta_invincible(don) && !asked.is_empty() && k > asked[0].0 + 120 {
            break;
        }
    }
    assert_eq!(asked.len(), 1, "once, as the hand-back begins: {asked:?}");
    let want = beam_yaw.expect("the beam");
    let d = ((asked[0].1 - want + 540.0) % 360.0) - 180.0;
    assert!(d.abs() < 3.0, "he faces where the beam went ({} vs {want}): the camera goes behind him there", asked[0].1);
}

/// (round 5c) several enemies: their hits are logged (the target id, the hit's flags)
struct Foes {
    list: Vec<(u32, Vec3)>,
    hits: Vec<(u32, u32)>,
}

impl kh_core::Targets for Foes {
    fn enumerate(&self, out: &mut Vec<kh_core::host::TargetInfo>) {
        out.extend(self.list.iter().filter_map(|(id, _)| self.info(kh_core::TargetId(*id))));
    }
    fn info(&self, id: kh_core::TargetId) -> Option<kh_core::host::TargetInfo> {
        use kh_core::host::TargetFlags;
        let at = self.list.iter().find(|f| f.0 == id.0)?.1;
        Some(kh_core::host::TargetInfo { id, position: at, center: at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 })
    }
    fn hurt_shapes(&self, id: kh_core::TargetId, out: &mut Vec<kh_core::host::HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(kh_core::host::HurtShape { a: i.position + Vec3::Z * 50.0, b: i.position + Vec3::Z * 130.0, radius: 50.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
        self.hits.push((hit.target.0, hit.flags));
        kh_core::host::HitResult { accepted: true, ..Default::default() }
    }
}

impl crate::PartyTargets for Foes {
    fn ai_actors(&self, out: &mut Vec<kh_ai::ActorView>) {
        for (id, at) in &self.list {
            out.push(kh_ai::ActorView { actor: kh_ai::ActorRef::Enemy(*id), position: *at, center: *at + Vec3::Z * 90.0, radius: 50.0, half_height: 90.0, alive: true, hostile: true, lockable: true, ..kh_ai::ActorView::default() });
        }
    }
    fn attacked_by_enemies(&self) -> bool {
        true
    }
}

fn step_foes(h: &mut Harness, foes: &mut Foes) {
    let (lp, ly) = h.leader_pos();
    let (s, c) = ly.to_radians().sin_cos();
    let camera = kh_camera::CameraView { position: lp - Vec3::new(c, s, 0.0) * 450.0 + Vec3::Z * 220.0, rotation: kh_camera::Rot { pitch: -15.0, yaw: ly, roll: 0.0 }, rotation_target: kh_camera::Rot { pitch: -15.0, yaw: ly, roll: 0.0 }, look_point: lp, distance: 450.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
    let input = kh_core::Input { camera_yaw: ly, ..kh_core::Input::default() };
    let dt = 1.0 / 60.0;
    let input = h.rt.route(crate::RouteIn { sora: &mut h.sora, input: &input, dt, targets: &*foes, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: h.t });
    let mut audio = kh_core::host::audio::NullAudio;
    let mut effects = kh_core::host::effects::NullEffects;
    let mut ui = kh_core::host::NullUi;
    h.out = {
        let mut st = crate::SoraTargets { targets: &mut *foes, runtime: &mut h.rt };
        let mut host = kh_core::Host { world: &h.world, targets: &mut st, audio: &mut audio, effects: &mut effects, ui: &mut ui };
        h.sora.step_hosted(dt, &input, &mut host)
    };
    let _ = h.rt.step(crate::StepIn { dt, sora: &mut h.sora, out: &h.out, lock_on: kh_core::TargetId::NONE, camera: &camera, world: &h.world, targets: &mut *foes, audio: &mut audio, effects: &mut effects, nav: None, now: h.t });
    let _ = h.rt.after_step(&*foes, dt, h.t);
    h.t += dt as f64;
}

/// Donald leading: the point `ahead` cm before him and `side` cm to his right
fn place(h: &Harness, ahead: f32, side: f32) -> Vec3 {
    let (lp, ly) = h.leader_pos();
    let (s, c) = ly.to_radians().sin_cos();
    lp + Vec3::new(c, s, 0.0) * ahead + Vec3::new(-s, c, 0.0) * side
}

/// (round 5c) THE ALLIES' HANDS OFF (the user: "allies should not target anything close to Donald or the Zettaflare or the
/// Zettaflare's expected path until after the player has regained control over Donald. This includes using magic on
/// enemies"): enemies by Donald, on the beam's path, by the target, and one far off; from the cast to the hand-back no
/// ally's hit (swing, spell, area, projectile) lands on any but the far one (Zettaflare's own forced hits are its), the
/// hosts' enemy feeds and AI Sora's targeting leave them out; at the hand-back they are fair game again
#[test]
fn zetta_the_allies_never_touch_an_enemy_near_donald_the_target_or_the_beam() {
    let Some(mut h) = Harness::new() else {
        println!("no party AI pack: skipped");
        return;
    };
    h.rt.lead_word(&["leader", PartyChara::Donald.name()]);
    for _ in 0..30 {
        h.step(None);
    }
    let mut foes = Foes { list: vec![(1, place(&h, 700.0, 0.0)), (2, place(&h, -150.0, 250.0)), (3, place(&h, 380.0, 120.0)), (4, place(&h, 850.0, -250.0)), (5, place(&h, -300.0, -1300.0))], hits: Vec::new() };
    h.rt.word("party don-zetta-ready");
    h.rt.word("party don-cast y.zettaflare.1");
    let force = kh_core::host::HIT_FLAG_FORCE;
    let mut span = 0;
    for _ in 0..(40.0 * 60.0) as usize {
        step_foes(&mut h, &mut foes);
        if !h.rt.party.don_zetta_span() {
            if span > 0 {
                break;
            }
            continue;
        }
        span += 1;
        let off = h.rt.party.don_zetta_off_limit_targets(&foes);
        for id in 1..=4 {
            assert!(off.contains(&kh_core::TargetId(id)), "enemy {id} is off limits ({off:?})");
        }
        assert!(!off.contains(&kh_core::TargetId(5)), "the far one is fair game");
        for &(id, flags) in &foes.hits {
            assert!(id == 5 || flags & force != 0, "an ally's hit on enemy {id} during Zettaflare (step {span})");
        }
        foes.hits.clear();
    }
    assert!(span > 60 * 20, "the whole span: {span} steps");
    assert!(h.rt.party.don_zetta_off_limit_targets(&foes).is_empty(), "fair game again at the hand-back");
}

/// (round 5c) ONE ENEMY, ZETTAFLARE'S (the user: "If there is only one target (and they're being targeted by or are in the
/// zettaflare) then the allies move away and wait for when the player regains control of Donald"): the allies go to a
/// waiting spot out of every zone (behind or beside Donald, past his zone) and stand there without a hit on it until the
/// hand-back
#[test]
fn zetta_with_only_its_target_the_allies_move_away_and_wait() {
    let Some(mut h) = Harness::new() else {
        println!("no party AI pack: skipped");
        return;
    };
    h.rt.lead_word(&["leader", PartyChara::Donald.name()]);
    for _ in 0..30 {
        h.step(None);
    }
    let mut foes = Foes { list: vec![(1, place(&h, 700.0, 0.0))], hits: Vec::new() };
    h.rt.word("party don-zetta-ready");
    h.rt.word("party don-cast y.zettaflare.1");
    let don = h.rt.leader.expect("Donald leads");
    let force = kh_core::host::HIT_FLAG_FORCE;
    let mut span = 0;
    let mut settled = 0;
    for _ in 0..(40.0 * 60.0) as usize {
        step_foes(&mut h, &mut foes);
        if !h.rt.party.don_zetta_span() {
            if span > 0 {
                break;
            }
            continue;
        }
        span += 1;
        assert!(foes.hits.iter().all(|h| h.1 & force != 0), "no ally's hit on the only enemy");
        assert!(!h.rt.party.don_zetta_wait_spots().is_empty(), "somewhere to wait");
        if span > 60 * 8 {
            // settled: every AI ally (the friends and AI Sora) out of every zone, away from Donald
            let zones = h.rt.party.don_zetta_zones();
            let dp = h.rt.party.view(don).unwrap().position;
            let mut at: Vec<Vec3> = h.rt.party.members().filter(|m| *m != don).filter_map(|m| h.rt.party.view(m)).filter(|v| !v.ko).map(|v| v.position).collect();
            at.push(h.out.position);
            let spots = h.rt.party.don_zetta.wait_spots.clone();
            assert!(!spots.is_empty(), "they are waiting");
            for p in at {
                let near = spots.iter().map(|s| (p - *s).truncate().length()).fold(f32::MAX, f32::min);
                assert!(near < 400.0, "an ally not at a waiting spot ({p} vs {spots:?})");
                assert!(zones.iter().all(|z| !z.inside(p)), "an ally in a zone at {p}");
                assert!((p - dp).truncate().length() > 400.0, "an ally crowding Donald at {p}");
            }
            settled += 1;
        }
    }
    assert!(span > 60 * 20 && settled > 60 * 10, "{span} {settled}");
}

/// (ZETTA 2026-10-23) the ground lift of a target's low shot (`runtime::zetta_cinematic_view`, after the wall rule): every
/// other shot is drawn as it is handed in.
#[test]
fn zetta_the_cinematic_camera_never_collides_its_distance_is_the_authored_one() {
    use kh_party::donald::zettaflare::ZettaView;
    struct Walled;
    impl kh_core::World for Walled {
        fn sweep_capsule(&self, start: Vec3, end: Vec3, _r: f32, _h: f32) -> Option<kh_core::world::Hit> {
            Some(kh_core::world::Hit { time: 0.05, position: start.lerp(end, 0.05), normal: Vec3::X, impact_normal: Vec3::X, start_penetrating: false, penetration_depth: 0.0, component: 1 })
        }
        fn line_trace(&self, start: Vec3, end: Vec3) -> Option<kh_core::world::Hit> {
            // a slope: the ground at z 100 everywhere under the shots
            (start.z > 100.0 && end.z < 100.0).then(|| { let t = (start.z - 100.0) / (start.z - end.z); kh_core::world::Hit { time: t, position: start.lerp(end, t), normal: Vec3::Z, impact_normal: Vec3::Z, start_penetrating: false, penetration_depth: 0.0, component: 1 } })
        }
    }
    let donald = Vec3::new(0.0, 0.0, 0.0);
    let view = |shot: &'static str, position: Vec3, pivot: Vec3| ZettaView { position, rotation: Vec3::new(-5.0, 30.0, 0.0), fov: 60.0, pivot, weight: 1.0, shot, safe_position: donald + Vec3::new(-80.0, 40.0, 160.0), safe_rotation: Vec3::ZERO, safe_fov: 50.0, safe_pivot: donald, block_share: 0.25, exact: false, handing_back: false, white: 0.0, shake_rot: Vec3::ZERO, shake_loc: Vec3::ZERO, beam_dir: Vec3::X };
    for (shot, at) in [("shot960", Vec3::new(-2400.0, 900.0, 700.0)), ("shot970", Vec3::new(-300.0, 0.0, 180.0))] {
        let v = view(shot, at, donald + Vec3::Z * 60.0);
        let (drawn, rot) = crate::runtime::zetta_cinematic_view(&v, &Walled);
        assert_eq!(drawn, at, "{shot}: drawn where it is authored");
        assert!(((drawn - v.pivot).length() - (at - v.pivot).length()).abs() < 1e-3, "{shot}: its distance the authored one");
        assert_eq!(rot, v.rotation);
    }
    // a target's low shot 40 cm over its feet with the ground rising to 100 under the camera: lifted to 130, its pitch turned
    let target = Vec3::new(500.0, 0.0, 90.0);
    let v = view("shot920", Vec3::new(300.0, 0.0, 40.0), target);
    let (drawn, rot) = crate::runtime::zetta_cinematic_view(&v, &Walled);
    assert!((drawn.z - (100.0 + crate::runtime::ZETTA_CAM_GROUND_CLEAR)).abs() < 1e-3 && drawn.x == 300.0);
    assert!(rot.x < v.rotation.x, "looks down onto the target as much as it was raised");
}

/// (ZETTA 2026-10-23, the user: "Also Zettaflare should obey wall collision again, now you don't see anything cause it's gone
/// through a wall") the hosts' wall rule (`runtime::ZettaSolve`, Elden Ring's and the test level's): a wall behind the
/// authored camera: SWUNG round Donald at the very same distance and lens, clear of it; boxed in on every side: PULLED IN to
/// where the level allows, the lens widened; never inside or behind geometry; the way held through the shot.
#[test]
fn zetta_the_camera_keeps_out_of_walls_swung_first_pulled_in_last() {
    use crate::runtime::{zetta_clear, ZettaSolve, ZettaWay};
    use kh_party::donald::zettaflare::ZettaView;
    // a wall: the plane x = 300 (anything beyond it is behind geometry)
    struct Wall;
    impl kh_core::World for Wall {
        fn sweep_capsule(&self, start: Vec3, end: Vec3, r: f32, _h: f32) -> Option<kh_core::world::Hit> {
            let x = 300.0 - r;
            (end.x > x && start.x < x).then(|| { let t = (x - start.x) / (end.x - start.x); kh_core::world::Hit { time: t, position: start.lerp(end, t), normal: -Vec3::X, impact_normal: -Vec3::X, start_penetrating: false, penetration_depth: 0.0, component: 1 } })
        }
        fn line_trace(&self, _s: Vec3, _e: Vec3) -> Option<kh_core::world::Hit> {
            None
        }
    }
    // boxed in: every way out meets the level at 40 % of the way
    struct Boxed;
    impl kh_core::World for Boxed {
        fn sweep_capsule(&self, start: Vec3, end: Vec3, _r: f32, _h: f32) -> Option<kh_core::world::Hit> {
            ((end - start).length() > 1.0).then(|| kh_core::world::Hit { time: 0.4, position: start.lerp(end, 0.4), normal: Vec3::X, impact_normal: Vec3::X, start_penetrating: false, penetration_depth: 0.0, component: 1 })
        }
        fn line_trace(&self, _s: Vec3, _e: Vec3) -> Option<kh_core::world::Hit> {
            None
        }
    }
    let pivot = Vec3::new(0.0, 0.0, 60.0);
    let at = Vec3::new(400.0, 0.0, 160.0);
    let d = at - pivot;
    let look = -d.normalize();
    let rotation = Vec3::new(look.z.asin().to_degrees(), look.y.atan2(look.x).to_degrees(), 0.0);
    let v = ZettaView { position: at, rotation, fov: 50.0, pivot, weight: 1.0, shot: "shot910", safe_position: Vec3::ZERO, safe_rotation: Vec3::ZERO, safe_fov: 50.0, safe_pivot: pivot, block_share: 0.25, exact: false, handing_back: false, white: 0.0, shake_rot: Vec3::ZERO, shake_loc: Vec3::ZERO, beam_dir: Vec3::Y };
    assert!(!zetta_clear(pivot, at, &Wall), "the authored camera is behind the wall");
    let mut solve = ZettaSolve::default();
    let (p, r, fov) = solve.view(&v, &Wall, 1.0 / 60.0);
    assert!(matches!(solve.way(), ZettaWay::Swung { .. }), "{:?}", solve.way());
    assert!(((p - pivot).length() - d.length()).abs() < 0.5, "the same distance ({} vs {})", (p - pivot).length(), d.length());
    assert_eq!(fov, v.fov, "the same lens");
    assert!(zetta_clear(pivot, p, &Wall) && p.x < 300.0, "clear of the wall: {p}");
    // the view turned with it: it still looks at Donald from there
    let (pp, yy) = (r.x.to_radians(), r.y.to_radians());
    let fwd = Vec3::new(pp.cos() * yy.cos(), pp.cos() * yy.sin(), pp.sin());
    assert!(fwd.dot((pivot - p).normalize()) > 0.999, "Donald where he was on the screen");
    // (the user: "Can the first shot of Donald charging up just be a cut from gameplay") the shot's first frame is its resolved
    // camera exactly: no blend at a cut
    assert_eq!((p, r, fov), ZettaSolve::pose_of(&v, ZettaSolve::choose(&v, &Wall), &Wall), "the first frame: the resolved camera");
    // held through the shot (a later frame: the same way)
    let way = solve.way();
    let _ = solve.view(&v, &Wall, 1.0 / 60.0);
    assert_eq!(solve.way(), way, "held for the shot");
    // boxed in: pulled in to where the level allows, the lens widened, never past it
    let mut solve = ZettaSolve::default();
    let (p, _, fov) = solve.view(&v, &Boxed, 1.0 / 60.0);
    assert!(matches!(solve.way(), ZettaWay::Pulled { .. }), "{:?}", solve.way());
    assert!(((p - pivot).length() - 0.4 * d.length()).abs() < 1.0, "pulled in to the level ({})", (p - pivot).length());
    assert!(fov > v.fov && fov <= 110.0, "the lens widened to keep the subject's size: {fov}");
}



/// (ZETTA 2026-10-23, the user: "Goofy also randomly starts hopping after you do Zettaflare and never stops") after
/// Zettaflare's hand-back the AI friends are on their own follow / combat at once (no waiting spot, no Zettaflare move), and
/// in the 15 s after it none takes off more than once (no repeated hop)
#[test]
fn zetta_the_friends_act_normally_after_the_hand_back() {
    use kh_party::NpcStateId as S;
    let Some(mut h) = Harness::new() else {
        println!("no party AI pack: skipped");
        return;
    };
    h.rt.lead_word(&["leader", PartyChara::Donald.name()]);
    for _ in 0..30 {
        h.step(None);
    }
    let don = h.rt.leader.expect("Donald leads");
    let (lp, ly) = h.leader_pos();
    let foe = OneEnemy { at: lp + Vec3::new(ly.to_radians().cos(), ly.to_radians().sin(), 0.0) * 700.0 };
    h.rt.word("party don-zetta-ready");
    h.rt.word("party don-cast y.zettaflare.1");
    let mut back_at = None;
    let mut takeoffs = 0;
    let mut air_was = false;
    for k in 0..(60.0 * 60.0) as usize {
        step_with(&mut h, foe);
        let span = h.rt.party.don_zetta_span();
        if back_at.is_none() && k > 60 && !span {
            back_at = Some(k);
            assert!(h.rt.party.don_zetta.wait_spots.is_empty(), "no waiting spot after the hand-back");
        }
        if let Some(b) = back_at {
            for id in h.rt.party.members().filter(|i| *i != don).collect::<Vec<_>>() {
                let p = h.rt.party.member(id).unwrap();
                let air = matches!(p.state, S::Jump | S::NavLinkJump | S::Hopping);
                if air && !air_was {
                    takeoffs += 1;
                }
                air_was = air;
                assert!(h.rt.hop_loops.is_empty(), "no move loop");
            }
            if k > b + 15 * 60 {
                break;
            }
        }
    }
    assert!(back_at.is_some(), "the hand-back came");
    assert!(takeoffs <= 1, "no repeated hops after the hand-back ({takeoffs})");
    let goofy = h.rt.party.members().find(|i| h.rt.party.member(*i).is_some_and(|p| p.chara == kh_party::Chara::Goofy)).expect("Goofy");
    let st = h.rt.party.member(goofy).unwrap().state;
    assert!(matches!(st, S::Idle | S::Move | S::Turn) || format!("{st:?}").contains("Action"), "Goofy on his own follow / combat: {st:?}");
}
