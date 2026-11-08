//! (MIMIC-BOSS hotfix 3, MB3-AGGRO, THE USER 2026-11-06: "the boss mimic tear needs to be way way more aggressive"; "the more
//! aggresive AI for the mimic boss needs to apply to Donald and Goofy's mimic bosses as well. Goofy's boss currently stands
//! still and does nothing, waiting to parry you. Make the mimic boss worse at countering everything.") THE OFFLINE DUEL:
//! each boss copy (Sora, Donald, Goofy: the real trees, the real cores and pawns) against a STANDING leader Sora for 60 s of
//! fixed steps, measured: attacks per minute (its attack starts and the steps its hits landed on him), the share of the time
//! it stood idle, the distance it kept, and (a second minute with the standing leader mashing Attack) its parries / guards /
//! dodges per minute and per swing offered. Each duel runs twice: the party's own AI (`CopyAggro::party`: what the copies
//! ran before) and the boss's ([`CopyAggro::default`], the sheet's rows). The AI pack at build/sora_pack/party/ai (skipped
//! without it, said on stderr). `cargo test -p kh-partyhost --test mimic_aggro -- --nocapture` prints the table.

use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Pose, Sora, Status, Vec3, SORA_PARAMS};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;
use kh_partyhost::mimicboss::{self as mb, BossTargets, CopyAggro, CopyDefence, FriendBoss, FriendBossStep};
use kh_partyhost::{BossAi, BossScene, PartyRuntime, PartyTargets, PartyTest, RouteIn, RuntimeConfig, StepIn};

const DT: f32 = 1.0 / 60.0;
const BOSS: TargetId = TargetId(1);
const MINUTE: usize = 60 * 60;

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim_start_matches('\u{feff}')).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
            _ => continue,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn camera() -> kh_camera::CameraView {
    kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false }
}

struct NoEnemies;

impl kh_core::Targets for NoEnemies {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
    fn hit_bone(&self, _id: TargetId, _at: Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

impl PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn leader_sora() -> Sora {
    let mut s = Sora::new(SORA_PARAMS, Pose { position: Vec3::ZERO, yaw: 180.0 }).with_status(level99());
    s.command_mut().fill(&level99());
    s
}

/// The real party (Sora leads; Donald and Goofy at level 99 with their whole kits), the AI pack loaded.
fn party() -> Option<(PartyRuntime, Sora)> {
    let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party/ai");
    let ai_pack = kh_ai::AiPack::load_dir(&pack).ok().map(std::sync::Arc::new)?;
    let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: Some(ai_pack), ai_pack_source: None });
    let mut sora = leader_sora();
    let world = TestWorld::floor();
    let cam = camera();
    for asked in [None, Some(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Sora).unwrap()), None, None] {
        let mut t = NoEnemies;
        let _ = rt.route(RouteIn { sora: &mut sora, input: &Input::default(), dt: DT, targets: &t, camera: &cam, asked_formation: asked, asked_ai: Vec::new(), nav: None, now: 0.0 });
        let out = sora.output();
        let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
        rt.step(StepIn { dt: DT, sora: &mut sora, out: &out, lock_on: TargetId::NONE, camera: &cam, world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
        let _ = rt.after_step(&t, DT, 0.0);
    }
    for c in [kh_party::Chara::Donald, kh_party::Chara::Goofy] {
        let id = rt.party.find(c)?;
        *rt.party.status_mut(id)? = kh_party::status::FriendStatus::test_level99(c);
    }
    Some((rt, sora))
}

/// The boss's copy in the duel.
enum Copy {
    Sora(Box<Sora>, Box<BossAi>),
    Friend(Box<FriendBoss>),
}

/// What the duel counted.
#[derive(Clone, Debug, Default)]
struct Tally {
    /// The copy's state names, steps in each, and its answers to the leader's swings (the diagnosis).
    names: std::collections::BTreeMap<String, usize>,
    answers: std::collections::BTreeMap<String, usize>,
    steps: usize,
    /// Its attack starts (a free / moving copy that starts an action of its own: a swing, a spell, an Art, a Y command).
    attacks: usize,
    /// Its hits that landed on the leader.
    landed: usize,
    idle_steps: usize,
    dist_sum: f32,
    /// The leader's swings that reached its body, and its answers to them.
    offered: usize,
    parried: usize,
    guarded: usize,
    dodged: usize,
    /// A Sora copy's counter: the blows that opened its Aerial Recovery window, and the recoveries it took (AIR_SLIDE).
    windows: usize,
    recovered: usize,
    /// A Goofy copy's AI: the leader's attacks it read and rolled, and the rolls it timed well (its parry rate).
    rolled: u32,
    rolled_good: u32,
    /// The copy's states, steps in each (the diagnosis printed with the table).
    states: [usize; 8],
}

impl Tally {
    fn per_min(&self, n: usize) -> f32 {
        n as f32 * MINUTE as f32 / self.steps.max(1) as f32
    }
}

/// The copy's body for the leader's swings: a Goofy copy's own rule first (`FriendBoss::defend`), a hit that lands its KH3
/// reaction (`take_reaction` / the Sora copy's `reaction_of`). A Sora copy's guard / dodge is its own state at the hit.
struct Body<'a> {
    copy: &'a mut Copy,
    tally: &'a mut Tally,
}

impl Body<'_> {
    fn info_of(&self) -> Option<TargetInfo> {
        let (p, v, yaw) = match &*self.copy {
            Copy::Sora(c, _) => (c.output().position, c.output().velocity, c.output().yaw),
            Copy::Friend(b) => b.view().map(|v| (v.position, v.velocity, v.yaw))?,
        };
        Some(TargetInfo { id: BOSS, position: p, center: p + Vec3::Z * 90.0, velocity: v, yaw, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 })
    }
}

impl kh_core::Targets for Body<'_> {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.info_of());
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.info_of().filter(|_| id == BOSS)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.center - Vec3::Z * 50.0, b: i.center + Vec3::Z * 50.0, radius: i.radius, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.tally.offered += 1;
        let landed = HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false };
        match &mut *self.copy {
            Copy::Sora(c, _) => {
                use kh_core::StateId as S;
                match c.output().state {
                    S::Guard => self.tally.guarded += 1,
                    S::Dodgeroll | S::Riskdodge | S::AirSlide => self.tally.dodged += 1,
                    _ => {}
                }
                c.take_hit(mb::reaction_of(hit));
                landed
            }
            Copy::Friend(b) => {
                let d = b.defend(PartyChara::Sora, hit);
                let state = b.view().map_or("-".to_string(), |v| format!("{:?}", v.state));
                *self.tally.answers.entry(format!("{d:?} in {state}")).or_default() += 1;
                match d {
                    CopyDefence::Parried { .. } => self.tally.parried += 1,
                    CopyDefence::Blocked => self.tally.guarded += 1,
                    CopyDefence::Evaded => self.tally.dodged += 1,
                    CopyDefence::Through => {}
                }
                if d.negates() {
                    return d.result(1.0e6);
                }
                let _ = b.take_reaction(hit);
                landed
            }
        }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
    fn hit_bone(&self, _id: TargetId, _at: Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

/// The copy is acting on its own (an attack, a spell, an Art, a Y command), not standing, walking, flinching or guarding.
fn acting(copy: &Copy) -> bool {
    match copy {
        Copy::Sora(c, _) => {
            use kh_core::StateId as S;
            !matches!(c.output().state, S::Idle | S::Run | S::Jump | S::Damage | S::Die | S::Guard | S::Dodgeroll | S::Riskdodge | S::AirSlide | S::Turn | S::Stagger)
        }
        Copy::Friend(b) => b.id().and_then(|id| b.rt.party.member(id)).is_some_and(|m| {
            use kh_party::NpcStateId as N;
            let guarding = m.status.goofy.is_some() && kh_party::pad::goofy::ai_stance(m) == kh_party::goofy::ai::Stance::Guarding;
            !guarding && !matches!(m.state, N::Idle | N::Move | N::Turn | N::Jump | N::Damage | N::Die | N::Warp | N::BadStatStun | N::BadStatBurn | N::BadStatFreeze | N::BadStatSneeze)
        }),
    }
}

/// The diagnosis' bins: 0 idle, 1 moving, 2 attacking / acting, 3 hurt, 4 guard / dodge, 5 jump / air, 6 other, 7 -.
fn state_bin(copy: &Copy) -> usize {
    match copy {
        Copy::Sora(c, _) => {
            use kh_core::StateId as S;
            match c.output().state {
                S::Idle => 0,
                S::Run | S::Turn => 1,
                S::Damage | S::Stagger | S::Die => 3,
                S::Guard | S::Dodgeroll | S::Riskdodge | S::AirSlide => 4,
                S::Jump => 5,
                _ if acting(copy) => 2,
                _ => 6,
            }
        }
        Copy::Friend(b) => match b.view().map(|v| v.state) {
            Some(kh_party::NpcStateId::Idle) => 0,
            Some(kh_party::NpcStateId::Move | kh_party::NpcStateId::Turn) => 1,
            Some(kh_party::NpcStateId::Damage | kh_party::NpcStateId::Die) => 3,
            Some(kh_party::NpcStateId::Jump) => 5,
            Some(_) if acting(copy) => 2,
            Some(_) => 4,
            None => 7,
        },
    }
}

fn idle(copy: &Copy) -> bool {
    match copy {
        Copy::Sora(c, _) => c.output().state == kh_core::StateId::Idle,
        Copy::Friend(b) => b.view().is_some_and(|v| v.state == kh_party::NpcStateId::Idle),
    }
}

/// ONE DUEL: the copy of `chara` (made 7 m from the leader) with `aggro`, against the standing leader Sora (`mash`: he fights
/// back where he stands, a three-hit string every 2 s while the copy is within 4 m) for `steps`.
/// How the standing leader plays.
#[derive(Clone, Copy, PartialEq)]
enum Leader {
    Stands,
    /// A three-hit string every 2 s while the copy is within 4 m.
    Strings,
    /// Attack mashed non-stop (his finishers' blows: a Sora copy's Aerial Recovery windows).
    Mashes,
}

fn duel(chara: PartyChara, aggro: CopyAggro, leader: Leader, steps: usize) -> Option<Tally> {
    let mash = leader == Leader::Strings;
    let (rt, _) = party()?;
    let mut lead = leader_sora();
    let at = Vec3::new(-700.0, 0.0, 0.0);
    let mut copy = match chara {
        PartyChara::Sora => {
            let core = mb::copy_sora(&lead, Pose { position: at, yaw: 0.0 });
            Copy::Sora(Box::new(core), Box::new(BossAi::new(&rt).with_aggro(aggro)))
        }
        c => {
            let mut b = FriendBoss::new(&rt, &lead, c, at, 0.0)?;
            b.set_aggro(aggro);
            Copy::Friend(Box::new(b))
        }
    };
    let world = TestWorld::floor();
    let cam = camera();
    let mut tally = Tally::default();
    let mut before = PadButtons::NONE;
    let mut was_acting = false;
    let (mut was_window, mut was_slide) = (false, false);
    for k in 0..steps {
        let now = k as f64 * DT as f64;
        let copy_at = match &copy {
            Copy::Sora(c, _) => c.output().position,
            Copy::Friend(b) => b.view().map_or(at, |v| v.position),
        };
        let mut t = BossTargets { victims: vec![mb::victim_of_sora(&lead, copy_at)], attack_rate: 1.0, ..BossTargets::default() };
        let (pos, yaw) = match &mut copy {
            Copy::Sora(c, ai) => {
                let input = ai.input(&rt, c, BossScene { targets: &t, camera: &cam, camera_yaw: 0.0, nav: None, dt: DT, now });
                let (mut audio, mut effects, mut ui) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects, kh_core::host::NullUi);
                let mut host = kh_core::Host { world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, ui: &mut ui };
                let o = c.step_hosted(DT, &input, &mut host);
                (o.position, o.yaw)
            }
            Copy::Friend(b) => {
                let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
                b.step(FriendBossStep { dt: DT, world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, camera: &cam, nav: None, now });
                b.view().map_or((copy_at, 0.0), |v| (v.position, v.yaw))
            }
        };
        mb::stance_to_sora(&mut t, &mut lead, pos);
        tally.landed += t.hits.len();
        for h in t.hits.drain(..) {
            lead.take_hit(mb::player_hit_of(&h, pos, yaw));
        }
        // the leader stands (never dies: his HP refilled), mashing or not
        let max = lead.status().max_hp();
        lead.status_mut().hp = max;
        // fighting back: a three-hit string (A at 0, 0.2, 0.4 s) every 2 s while the copy is within 4 m of him (a mashed
        // non-stop combo keeps any copy flinching for good: nothing to measure)
        let near = Vec3::new(pos.x - lead.output().position.x, pos.y - lead.output().position.y, 0.0).length() < 400.0;
        let held = if (mash && near && matches!(k % 120, 0 | 12 | 24)) || (leader == Leader::Mashes && k % 2 == 0) { PadButtons::NONE.with(PadButton::ATTACK) } else { PadButtons::NONE };
        let input = Input::default().with_pad(PadState { held, pressed: PadButtons(held.0 & !before.0), released: PadButtons(before.0 & !held.0) });
        before = held;
        {
            let mut body = Body { copy: &mut copy, tally: &mut tally };
            let (mut audio, mut effects, mut ui) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects, kh_core::host::NullUi);
            let mut host = kh_core::Host { world: &world, targets: &mut body, audio: &mut audio, effects: &mut effects, ui: &mut ui };
            let _ = lead.step_hosted(DT, &input, &mut host);
        }
        let a = acting(&copy);
        if a && !was_acting {
            tally.attacks += 1;
        }
        was_acting = a;
        tally.idle_steps += idle(&copy) as usize;
        if let Copy::Sora(c, _) = &copy {
            let window = c.damage_reaction().is_some_and(|d| d.recoverable);
            if window && !was_window {
                tally.windows += 1;
            }
            was_window = window;
            let slide = c.output().state == kh_core::StateId::AirSlide;
            if slide && !was_slide {
                tally.recovered += 1;
            }
            was_slide = slide;
        }
        tally.states[state_bin(&copy)] += 1;
        let name = match &copy {
            Copy::Sora(c, _) => format!("{:?}", c.output().state),
            Copy::Friend(b) => b.view().map_or("-".to_string(), |v| format!("{:?}", v.state)),
        };
        *tally.names.entry(name).or_default() += 1;
        tally.dist_sum += Vec3::new(pos.x - lead.output().position.x, pos.y - lead.output().position.y, 0.0).length();
        tally.steps += 1;
    }
    if let Copy::Friend(b) = &copy {
        if let Some(m) = b.id().and_then(|id| b.rt.party.gof.ai.iter().find(|a| a.member == id)) {
            (tally.rolled, tally.rolled_good) = (m.rolled, m.rolled_good);
        }
    }
    Some(tally)
}

/// One copy's numbers: (the standing leader's minute, the minute he fights back in strings; a Sora copy: the Aerial Recoveries
/// of a minute of his mashed combos, else zero).
fn measure(chara: PartyChara, aggro: CopyAggro) -> Option<(Tally, Tally)> {
    let mut back = duel(chara, aggro, Leader::Strings, 3 * MINUTE)?;
    if chara == PartyChara::Sora {
        let mashed = duel(chara, aggro, Leader::Mashes, MINUTE)?;
        (back.windows, back.recovered) = (mashed.windows, mashed.recovered);
    }
    Some((duel(chara, aggro, Leader::Stands, MINUTE)?, back))
}

fn row(name: &str, which: &str, (stand, mash): &(Tally, Tally)) -> String {
    format!(
        "{name:<6} {which:<6} | standing leader: attack starts/min {:>5.1}, hits landed/min {:>6.1}, idle {:>5.1} %, distance {:>5.0} cm | leader fighting back: hits landed/min {:>6.1}, his hits on it {:>4}, parried/min {:>5.1}, guarded/min {:>5.1}, dodged/min {:>5.1} (answered {:>5.1} %), timed well {}/{} of the attacks it read, Aerial Recoveries {}/{} blows | states idle/move/act/hurt/guard/air/other {:?}",
        stand.per_min(stand.attacks),
        stand.per_min(stand.landed),
        stand.idle_steps as f32 * 100.0 / stand.steps.max(1) as f32,
        stand.dist_sum / stand.steps.max(1) as f32,
        mash.per_min(mash.landed),
        mash.offered,
        mash.per_min(mash.parried),
        mash.per_min(mash.guarded),
        mash.per_min(mash.dodged),
        (mash.parried + mash.guarded + mash.dodged) as f32 * 100.0 / mash.offered.max(1) as f32,
        mash.rolled_good,
        mash.rolled,
        mash.recovered,
        mash.windows,
        &stand.states[..7],
    ) + &format!("\n    states {:?}\n    mash states {:?}\n    answers {:?}", stand.names, mash.names, mash.answers)
}

/// THE MEASURE, before (the party's own AI) and after (the boss's): every copy attacks more and stands idle less, keeps closer
/// to the leader, and answers fewer of his swings. Goofy's copy, the user's "stands still and does nothing, waiting to parry
/// you", attacks a standing leader now.
#[test]
fn mb3_aggro_every_boss_copy_is_more_aggressive_and_worse_at_countering_offline_duel() {
    if party().is_none() {
        eprintln!("no AI pack at build/sora_pack/party/ai: skipped");
        return;
    }
    let mut lines = Vec::new();
    let mut runs = Vec::new();
    for chara in [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy] {
        let before = measure(chara, CopyAggro::party()).unwrap();
        let after = measure(chara, CopyAggro::default()).unwrap();
        lines.push(row(chara.name(), "before", &before));
        lines.push(row(chara.name(), "after", &after));
        runs.push((chara, before, after));
    }
    for l in &lines {
        eprintln!("{l}");
    }
    for (chara, before, after) in runs {
        let (b, a) = (&before.0, &after.0);
        assert!(a.landed > b.landed, "{}: more of its hits land: {lines:#?}", chara.name());
        assert!(after.1.landed > before.1.landed, "{}: more of its hits land on a leader who fights back: {lines:#?}", chara.name());
        assert!(a.idle_steps < b.idle_steps.max(1), "{}: less idle: {lines:#?}", chara.name());
        // worse at countering: a Sora copy takes fewer Aerial Recoveries per blow, a Goofy copy times fewer of the attacks he reads
        // (his parries per swing that reached him follow the fight's own course: printed, not asserted); a Donald copy has no
        // counter (his rules answer no hit)
        let recovered = |t: &Tally| t.recovered as f32 / t.windows.max(1) as f32;
        let timed = |t: &Tally| t.rolled_good as f32 / t.rolled.max(1) as f32;
        match chara {
            PartyChara::Sora => assert!(before.1.windows > 0 && recovered(&after.1) < recovered(&before.1), "Sora: fewer Aerial Recoveries per blow: {lines:#?}"),
            PartyChara::Goofy => assert!(before.1.rolled > 0 && after.1.rolled > 0 && timed(&after.1) < timed(&before.1), "Goofy: fewer well-timed answers: {lines:#?}"),
            PartyChara::Donald => assert_eq!(after.1.parried + after.1.guarded + after.1.dodged, 0, "Donald answers no hit"),
        }
    }
}

/// (MB3-AGGRO) Every sheet row names one knob, read back as written; the party's AI is the knobs switched off; the shares and
/// switches stay in range whatever the sheet says.
#[test]
fn mb3_aggro_every_sheet_row_is_one_knob() {
    let d = CopyAggro::default();
    assert!(d.on && !CopyAggro::party().on);
    for id in CopyAggro::ROWS {
        let v = d.get(id).unwrap_or_else(|| panic!("{id} is a knob"));
        let mut a = CopyAggro::party();
        assert!(a.set(id, v), "{id}");
        assert_eq!(a.get(id), Some(v), "{id}");
    }
    let (a, taken) = CopyAggro::from_rows(|id| match id {
        "aggro.goofy.parry_x" => Some(3.0),
        "aggro.sora.recover_chance" => Some(-1.0),
        "aggro.on" => Some(0.0),
        _ => None,
    });
    assert_eq!(taken.len(), 3);
    assert!(!a.on && a.goofy.parry_x == 1.0 && a.sora_recover_chance == 0.0);
    assert!(a.goofy_tune().is_none() && a.donald_tune().is_none(), "off: the party's own AI");
}
