//! (party stream D) Sora's side of the team attacks against a scripted friends' side: the friend
//! gauge and a friend's offer (team branch of NotifySetActionCommand: 30 s x (1 + 0.5 x COMBI_TIME)),
//! Triangle runs the FR state, the link pawn's specific actions reach the event list, the end clip;
//! the unison roll and finisher of a combo next to Donald.

use kh_core::host::effects::NullEffects;
use kh_core::host::audio::NullAudio;
use kh_core::host::{EventKind, Friend, FriendLinkRequest, FriendLinkView, Host, HitRecord, HitResult, HurtShape, NullUi, Targets};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
            _ => continue,
        };
        let v = match (row[0].as_str().unwrap(), v) {
            // The five team finishers equipped (the sheet owns them, equips them not).
            ("abilities.equipped", LoadoutValue::List(mut l)) => {
                l.extend(["UNISON_FIRE", "UNISON_BLIZZARD", "UNISON_THUNDER", "FUSION_SPIN", "FUSION_ROCKET"].map(String::from));
                LoadoutValue::List(l)
            }
            (_, v) => v,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

/// A scripted party: Donald and Goofy beside Sora, a dummy enemy, and a link pawn that holds Sora
/// for `link_steps` steps, sends `action` on its third step, then finishes.
#[derive(Default)]
struct Party {
    dummy: Option<TargetInfo>,
    offer: Option<(u8, i32, f32)>,
    requests: Vec<FriendLinkRequest>,
    link: Option<(i32, u32)>,
    link_steps: u32,
    action: u8,
}

impl Targets for Party {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.dummy);
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.dummy.filter(|d| d.id == id)
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
    fn friends(&self, out: &mut Vec<Friend>) {
        for (uid, y) in [(11u8, 150.0f32), (12, -150.0)] {
            let mut link = FriendLinkView::default();
            if let Some((kind, n)) = self.link {
                link = FriendLinkView { kind, phase: n as u8, finished: n >= self.link_steps, sora: Some((Vec3::new(0.0, 0.0, 300.0), 0.0)), sora_motion: Some("GShootRise"), sora_action: (n == 3).then_some(self.action), ..FriendLinkView::default() };
            }
            out.push(Friend { uid, position: Vec3::new(0.0, y, 0.0), yaw: 0.0, ko: false, combi_time: 1, combi_up: 0, offer: self.offer.filter(|o| o.0 == uid).map(|o| (o.1, o.2)), link });
        }
    }
    fn friend_link(&mut self, req: FriendLinkRequest) {
        if let FriendLinkRequest::Start { kind, .. } = req {
            self.link = Some((kind, 0));
        }
        if let FriendLinkRequest::End { .. } = req {
            self.link = None;
        }
        self.requests.push(req);
    }
}

struct Bench {
    sora: Sora,
    party: Party,
    world: TestWorld,
    events: Vec<EventKind>,
}

impl Bench {
    fn new() -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut b = Bench { sora, party: Party::default(), world: TestWorld::floor(), events: Vec::new() };
        for _ in 0..5 {
            b.step(&[], &[]);
        }
        b
    }

    fn step(&mut self, held: &[PadButton], before: &[PadButton]) -> kh_core::Output {
        let set = |l: &[PadButton]| l.iter().fold(PadButtons::NONE, |a, b| a.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set(held), set(before)));
        let (mut audio, mut effects, mut ui) = (NullAudio, NullEffects, NullUi);
        let mut host = Host { world: &self.world, targets: &mut self.party, audio: &mut audio, effects: &mut effects, ui: &mut ui };
        let out = self.sora.step_hosted(DT, &input, &mut host);
        if let Some((_, n)) = &mut self.party.link {
            *n += 1;
        }
        self.events.extend(out.events.iter().map(|e| e.kind));
        out
    }
}

#[test]
fn an_offer_enters_the_queue_with_combi_time_and_y_runs_the_team_attack() {
    let mut b = Bench::new();
    // Gauge: points of a hit go to pending, the tick moves them in, capped at 200.
    b.sora.combat_mut().friend.gauge_add(500.0);
    b.step(&[], &[]);
    assert!(b.sora.combat().friend.gauge_full());
    assert!(b.party.requests.iter().any(|r| matches!(r, FriendLinkRequest::Sora { gauge_full: true, .. })));
    // Donald offers Flare Force (208) with 30 s; his COMBI_TIME 1 makes it 45 s; the gauge resets.
    b.party.offer = Some((11, 208, 30.0));
    b.step(&[], &[]);
    b.party.offer = None;
    let entry = b.sora.combat().situation.list.iter().find(|e| e.kind == 208).copied().expect("the entry");
    assert_eq!(entry.total, 45.0);
    assert!(!b.sora.combat().friend.gauge_full(), "slot 901");
    // Past the appear time, Y executes it: the FR state begins and asks for the link pawn.
    for _ in 0..40 {
        b.step(&[], &[]);
    }
    b.party.link_steps = 30;
    b.party.action = 87;
    b.step(&[PadButton::SITUATION], &[]);
    let out = b.step(&[], &[]);
    assert_eq!(out.state, StateId::FrFlareforce);
    assert!(b.party.requests.iter().any(|r| matches!(r, FriendLinkRequest::Start { kind: 208, .. })));
    assert!(!b.sora.combat().situation.list.iter().any(|e| e.kind == 208), "used");
    // The link pawn holds him (custom move at its point) and sends action 87 (the dona_flair camera).
    for _ in 0..10 {
        b.step(&[], &[]);
    }
    assert!(b.events.iter().any(|e| matches!(e, EventKind::PlayerSpecificAction { state: StateId::FrFlareforce, action: 87, .. })));
    assert!(b.events.iter().any(|e| matches!(e, EventKind::PlayerSpecificAction { state: StateId::FrFlareforce, action: 0, .. })), "Begin's SpecificAction(0)");
    // Link finished -> M_1501 -> Idle; the End request.
    let mut state = StateId::FrFlareforce;
    for _ in 0..200 {
        let o = b.step(&[], &[]);
        state = o.state;
        if state != StateId::FrFlareforce {
            break;
        }
    }
    assert_ne!(state, StateId::FrFlareforce);
    assert!(b.party.requests.iter().any(|r| matches!(r, FriendLinkRequest::End { kind: 208, interrupted: false })));
    assert!(b.events.iter().any(|e| matches!(e, EventKind::TeamAttack { kind: 208, phase: 255 })));
}

#[test]
fn a_combo_next_to_donald_ends_with_a_unison_finisher() {
    let mut b = Bench::new();
    b.party.dummy = Some(TargetInfo { id: TargetId(1), position: Vec3::new(200.0, 0.0, 0.0), center: Vec3::new(200.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 });
    // Donald 150 cm away (rate 60), all five unison abilities equipped at level 99: base 1000 +
    // 600 against chance 100 -> the roll cannot fail.
    for i in 0..600 {
        let out = b.step(&[PadButton::ATTACK], &[]);
        if i > 2 && out.state == StateId::Idle {
            break;
        }
    }
    let kinds = b.sora.combat().history.clone();
    let last = *kinds.last().unwrap();
    assert!((0x3F..=0x43).contains(&last), "the finisher is a unison / fusion kind: {kinds:x?}");
    assert!(b.party.requests.iter().any(|r| matches!(r, FriendLinkRequest::Unison { .. })));
    assert_eq!(b.sora.combat().friend.unison_chance, 50, "halved by the success");
    assert_eq!(b.sora.combat().friend.unison, 0, "cleared when the chain ended");
}

#[test]
fn a_friend_leaders_combo_rolls_the_unison_and_ai_sora_plays_it() {
    // (D3) Donald leads 150 cm from Sora: the leader's roll (cannot fail at level 99, chance 100)
    // latches the kind; Sora, idle (no pad press), plays it as his attack and asks for Donald.
    let mut b = Bench::new();
    b.party.dummy = Some(TargetInfo { id: TargetId(1), position: Vec3::new(200.0, 0.0, 0.0), center: Vec3::new(200.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 });
    let abilities = b.sora.status().abilities.clone();
    let sora_at = b.sora.output().position;
    let ok = b.sora.combat_mut().friend.roll_leader_unison(&abilities, 11, sora_at + Vec3::new(0.0, 150.0, 0.0), sora_at);
    assert!(ok, "rolled");
    let kind = b.sora.combat().friend.unison;
    assert!((22..=24).contains(&kind), "only Donald's kinds: {kind}");
    for _ in 0..120 {
        b.step(&[], &[]);
    }
    assert!(b.sora.combat().history.iter().any(|k| (0x3F..=0x41).contains(k)), "{:x?}", b.sora.combat().history);
    assert!(b.party.requests.iter().any(|r| matches!(r, FriendLinkRequest::Unison { uid: 11, .. })));
    // Goofy leading never gets Donald's kinds; out of range nothing is rolled.
    assert!(!b.sora.combat_mut().friend.roll_leader_unison(&abilities, 12, sora_at + Vec3::new(900.0, 0.0, 0.0), sora_at));
}
