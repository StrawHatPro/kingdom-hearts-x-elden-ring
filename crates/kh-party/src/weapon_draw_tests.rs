//! (PARTY-WEAPONS) THE TRACES: every party member (Donald, Goofy; run by the AI or leading on the pad) keeps his weapon away
//! out of combat, draws it with its own appear effect at the battle start and on an attack, puts it away with the same
//! effect after KH3's delay, never draws it in a no-combat place, and holds it for the hero light only while he carries
//! it (a leader swap moves the hold). Every staff and shield resolves to its look and socket.

use super::*;
use crate::command::{ActionDef, MemberCommand, TargetRef};
use crate::host::PartyHost;
use crate::{MemberId, Party, PartyConfig, SoraView};
use kh_core::host::{EffectHandle, HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::nocombat;
use kh_core::test_world::TestWorld;

const DT: f32 = 1.0 / 60.0;

struct Dummy(Vec3);

impl Targets for Dummy {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        let p = self.0;
        out.push(TargetInfo { id: TargetId(1), position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 });
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        let mut all = Vec::new();
        self.enumerate(&mut all);
        all.into_iter().find(|t| t.id == id)
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
    fn hit_bone(&self, _id: TargetId, _at: Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

/// The appear / disappear effects spawned (name, where).
#[derive(Default)]
struct Spawns(Vec<(&'static str, Attach)>);

impl kh_core::host::Effects for Spawns {
    fn spawn(&mut self, _handle: EffectHandle, spawn: &EffectSpawn) {
        if spawn.name.ends_with("_app00") {
            self.0.push((spawn.name, spawn.at));
        }
    }
    fn stop(&mut self, _handle: EffectHandle, _immediate: bool) {}
    fn tint(&mut self, _handle: EffectHandle, _tint: kh_core::host::Tint) {}
    fn place(&mut self, _handle: EffectHandle, _at: Vec3) {}
}

struct Rig {
    world: TestWorld,
    dummy: Dummy,
    audio: kh_core::host::audio::NullAudio,
    effects: Spawns,
    party: Party,
    sora: SoraView,
    ids: Vec<(Chara, MemberId)>,
    /// (time, member, out, opacity, why): the trace
    trace: Vec<String>,
    t: f32,
}

impl Rig {
    fn new(pad: Option<Chara>) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let mut ids = Vec::new();
        for (k, chara) in Chara::ALL.into_iter().enumerate() {
            let id = party.spawn(chara, Vec3::new(0.0, 200.0 * k as f32, 0.0), 0.0);
            if pad == Some(chara) {
                party.set_pad(id, Some(kh_core::Tuning::USER));
            }
            ids.push((chara, id));
        }
        let mut rig = Rig { world: TestWorld::floor(), dummy: Dummy(Vec3::new(3000.0, 0.0, 0.0)), audio: Default::default(), effects: Spawns::default(), party, sora: SoraView::default(), ids, trace: Vec::new(), t: 0.0 };
        rig.sora.position = Vec3::new(-300.0, 0.0, 0.0);
        rig
    }

    fn id(&self, c: Chara) -> MemberId {
        self.ids.iter().find(|x| x.0 == c).unwrap().1
    }

    fn view(&self, c: Chara) -> WeaponView {
        self.party.weapon_view(self.id(c)).unwrap()
    }

    fn step(&mut self, battle: bool) {
        self.party.set_in_battle(battle);
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummy, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
        self.t += DT;
        for (c, id) in self.ids.clone() {
            let m = self.party.member(id).unwrap();
            let v = m.weapon.view(m.status.weapon, c);
            self.trace.push(format!("{:.3} {c:?} out={} opacity={:.2} state={:?} clip={}@{:.2} seq={:?} last={:?}", self.t, v.out as u8, v.opacity, m.state, m.anim.clip.name, m.anim.time, m.weapon.put_away, m.weapon.last));
        }
    }

    fn run(&mut self, battle: bool, seconds: f32) {
        for _ in 0..(seconds / DT).round() as usize {
            self.step(battle);
        }
    }

    fn appears(&self, c: Chara) -> usize {
        let m = PartyMember(self.id(c).0);
        self.effects.0.iter().filter(|(_, at)| matches!(at, Attach::Party { member, .. } if *member == m)).count()
    }
}

fn effect_of(c: Chara) -> &'static str {
    match c {
        Chara::Donald => "vw_don_app00",
        Chara::Goofy => "vw_gof_app00",
    }
}

#[test]
fn hidden_out_of_combat_drawn_at_the_battle_start_put_away_after_kh3s_delay() {
    let _out = nocombat::scope(false);
    for pad in [None, Some(Chara::Donald), Some(Chara::Goofy)] {
        let mut rig = Rig::new(pad);
        rig.run(false, 3.0);
        for c in Chara::ALL {
            assert!(!rig.view(c).out && rig.view(c).opacity == 0.0, "{c:?} (pad {pad:?}): hidden out of combat");
            assert_eq!(rig.appears(c), 0, "{c:?}: nothing spawned out of combat");
        }
        rig.step(true);
        for c in Chara::ALL {
            let m = rig.party.member(rig.id(c)).unwrap();
            assert!(m.weapon.out, "{c:?} (pad {pad:?}): drawn at the battle start");
            assert_eq!(m.weapon.last, Some((true, Why::BattleStart)));
            let row = weapon_row(m.status.weapon).unwrap();
            let (name, at) = rig.effects.0.iter().rev().find(|(_, at)| matches!(at, Attach::Party { member, .. } if member.0 == m.id.0)).copied().unwrap();
            assert_eq!(name, effect_of(c), "{c:?}: his weapon's own m_AppearEffect");
            assert!(matches!(at, Attach::Party { bone: Some(b), .. } if b == row.socket), "{c:?}: at the weapon's socket {}", row.socket);
        }
        // the 0.1 s dither: 0 -> 1
        rig.run(true, 0.05);
        assert!(rig.view(Chara::Donald).opacity > 0.3 && rig.view(Chara::Donald).opacity < 0.8, "{}", rig.view(Chara::Donald).opacity);
        rig.run(true, 2.0);
        for c in Chara::ALL {
            assert!(rig.view(c).out && rig.view(c).opacity == 1.0, "{c:?}: stays out in battle");
            assert_eq!(rig.appears(c), 1, "{c:?}: one draw, one effect");
        }
        // the battle ends: still out (BattleEnd keeps it), then put away with the effect after KH3's idle count
        rig.step(false);
        for c in Chara::ALL {
            assert!(rig.view(c).out, "{c:?}: out right after the battle (the battle end keeps it)");
        }
        let (stand, _) = put_away_delays(Chara::Donald);
        assert!(stand > 1.0, "Donald's IdleN x 2 + IdleBreakW: {stand}");
        let longest = Chara::ALL.iter().map(|c| put_away_delays(*c).0.max(put_away_delays(*c).1)).fold(0.0, f32::max);
        rig.run(false, longest + 0.5);
        if pad.is_none() {
            write_trace("battle_start_and_put_away", &rig.trace);
        }
        for c in Chara::ALL {
            let m = rig.party.member(rig.id(c)).unwrap();
            assert!(!m.weapon.out && rig.view(c).opacity == 0.0, "{c:?} (pad {pad:?}): put away out of battle ({:?})", m.weapon.last);
            assert!(matches!(m.weapon.last, Some((false, Why::Idle | Why::Run))), "{c:?}: {:?}", m.weapon.last);
            assert_eq!(rig.appears(c), 2, "{c:?}: the put-away plays the disappear effect");
        }
    }
}

#[test]
fn an_attack_out_of_battle_draws_it_with_the_effect() {
    let _out = nocombat::scope(false);
    for (c, def) in [(Chara::Donald, ActionDef::DonaldBeat), (Chara::Donald, ActionDef::DonaldFire), (Chara::Goofy, ActionDef::GoofyPush), (Chara::Goofy, ActionDef::GoofyTornado)] {
        let mut rig = Rig::new(None);
        // (AI Goofy's melee is his own tree's pad presses in battle: KH3's definitions run with KH3's switch)
        rig.party.gof.kh3_ai = true;
        rig.dummy.0 = rig.party.member(rig.id(c)).unwrap().position() + Vec3::new(150.0, 0.0, 0.0);
        rig.run(false, 1.0);
        assert!(!rig.view(c).out);
        let id = rig.id(c);
        let ticket = rig.party.command(id, MemberCommand::Action { def, target: TargetRef::Enemy(TargetId(1)) });
        let mut drawn_in = None;
        for k in 0..30 {
            rig.step(false);
            if rig.view(c).out {
                drawn_in = Some(k);
                break;
            }
        }
        let m = rig.party.member(id).unwrap();
        assert!(drawn_in.is_some(), "{c:?} {def:?}: the attack draws it ({:?}, the command {:?})", m.state, rig.party.command_status(id, ticket));
        assert!(matches!(m.weapon.last, Some((true, Why::Act(_)))), "{c:?} {def:?}: {:?}", m.weapon.last);
        assert_eq!(rig.appears(c), 1, "{c:?} {def:?}: with its appear effect");
        if (c, def) == (Chara::Goofy, ActionDef::GoofyPush) {
            write_trace("attack_out_of_battle", &rig.trace);
        }
    }
}

#[test]
fn a_leaders_attack_on_the_pad_draws_it_with_the_effect() {
    let _out = nocombat::scope(false);
    use kh_core::input::{PadButton, PadButtons};
    for c in Chara::ALL {
        let mut rig = Rig::new(Some(c));
        rig.dummy.0 = rig.party.member(rig.id(c)).unwrap().position() + Vec3::new(150.0, 0.0, 0.0);
        rig.run(false, 1.0);
        assert!(!rig.view(c).out, "{c:?}: away before");
        let id = rig.id(c);
        let mut drawn = false;
        for k in 0..40 {
            let held = if k < 2 { PadButtons::NONE.with(PadButton::ATTACK) } else { PadButtons::NONE };
            let pressed = if k == 0 { held } else { PadButtons::NONE };
            rig.party.set_pad_frame(id, crate::pad::PadFrame { stick: glam::Vec2::ZERO, camera_yaw: 0.0, pad: kh_core::input::PadState { held, pressed, released: PadButtons::NONE } });
            rig.step(false);
            if rig.view(c).out {
                drawn = true;
                break;
            }
        }
        let m = rig.party.member(id).unwrap();
        assert!(drawn, "{c:?}: A on the pad draws it ({:?})", m.state);
        assert!(matches!(m.weapon.last, Some((true, Why::Act(_)))), "{c:?}: {:?}", m.weapon.last);
        assert_eq!(rig.appears(c), 1, "{c:?}: with its appear effect");
    }
}

#[test]
fn hurt_draws_it_without_the_effect_and_a_team_or_pad_state_counts_as_an_act() {
    let _out = nocombat::scope(false);
    let mut rig = Rig::new(None);
    rig.run(false, 0.5);
    let id = rig.id(Chara::Goofy);
    let mut fx = Spawns::default();
    let m = rig.party.member_mut(id).unwrap();
    m.state = NpcStateId::Damage;
    step(m, DT, false, &mut fx);
    assert!(m.weapon.out && m.weapon.seq.opacity(&GOOFY_LOOK) == 1.0 && fx.0.is_empty(), "the Damage state's Draw(1, 0): at once, no effect");
    for s in [NpcStateId::Fr(1), NpcStateId::Pad(pg::GUARD), NpcStateId::Pad(pgc::COMBO), NpcStateId::Pad(pd::STEP), NpcStateId::Pad(pd::ATTUNE), NpcStateId::Pad(crate::pad::AIR_ATTACK)] {
        assert!(act_of_state(s).is_some(), "{s:?}");
    }
    for s in [NpcStateId::Idle, NpcStateId::Move, NpcStateId::Pad(crate::pad::RUN), NpcStateId::Pad(crate::pad::JUMP), NpcStateId::Pad(pg::HOP), NpcStateId::Pad(pd::DODGE), NpcStateId::UseItem] {
        assert!(act_of_state(s).is_none(), "{s:?}");
    }
}

#[test]
fn the_no_combat_zone_keeps_every_weapon_away() {
    let mut rig = Rig::new(Some(Chara::Donald));
    {
        let _out = nocombat::scope(false);
        rig.run(true, 0.5);
        assert!(Chara::ALL.iter().all(|c| rig.view(*c).out));
    }
    let _zone = nocombat::scope(true);
    rig.step(true);
    for c in Chara::ALL {
        let m = rig.party.member(rig.id(c)).unwrap();
        assert!(!m.weapon.out && m.weapon.last == Some((false, Why::Zone)), "{c:?}: put away in the zone (Roundtable Hold)");
    }
    for (c, id) in rig.ids.clone() {
        rig.party.set_weapon_hold(id, true);
        rig.run(true, 1.0);
        assert!(!rig.view(c).out, "{c:?}: not even a battle or the hero light's hold draws it there");
        let mut fx = Spawns::default();
        let m = rig.party.member_mut(id).unwrap();
        m.state = NpcStateId::Action;
        step(m, DT, false, &mut fx);
        assert!(!m.weapon.out, "{c:?}: nor an attack");
    }
    write_trace("roundtable_zone", &rig.trace);
}

#[test]
fn the_hero_lights_hold_is_the_leaders_and_moves_with_a_leader_swap() {
    let _out = nocombat::scope(false);
    // Donald leads in the dark: his staff is held out of battle, Goofy's shield is not
    let mut rig = Rig::new(Some(Chara::Donald));
    let (don, gof) = (rig.id(Chara::Donald), rig.id(Chara::Goofy));
    let hold = |rig: &mut Rig, leader: Chara| {
        for (c, id) in rig.ids.clone() {
            rig.party.set_weapon_hold(id, kh_core::weapon_gate::hold(c == leader, true));
        }
    };
    hold(&mut rig, Chara::Donald);
    rig.run(false, 1.0);
    assert!(rig.view(Chara::Donald).out && rig.view(Chara::Donald).opacity == 1.0, "the leader draws for the light");
    assert_eq!(rig.party.member(don).unwrap().weapon.last, Some((true, Why::Hold)));
    assert!(!rig.view(Chara::Goofy).out, "an AI friend does not");
    rig.run(false, 20.0);
    assert!(rig.view(Chara::Donald).out, "held: no put-away while the light wants it");
    // the swap: Goofy leads (pad moves), the hold follows the leader
    rig.party.set_pad(don, None);
    rig.party.set_pad(gof, Some(kh_core::Tuning::USER));
    hold(&mut rig, Chara::Goofy);
    rig.step(false);
    assert!(!rig.party.member(don).unwrap().weapon.out && rig.party.member(don).unwrap().weapon.last == Some((false, Why::HoldReleased)), "the old leader puts it away (out of battle)");
    assert!(rig.view(Chara::Goofy).out && rig.party.member(gof).unwrap().weapon.last == Some((true, Why::Hold)), "the new leader draws it");
    // Sora leads again (neither friend carries it): both away
    for (_, id) in rig.ids.clone() {
        rig.party.set_weapon_hold(id, false);
    }
    rig.step(false);
    assert!(!rig.view(Chara::Goofy).out && !rig.view(Chara::Donald).out);
    // in battle a release keeps it (KH3: in battle it stays out)
    hold(&mut rig, Chara::Goofy);
    rig.run(true, 0.3);
    rig.party.set_weapon_hold(gof, false);
    rig.run(true, 0.3);
    assert!(rig.view(Chara::Goofy).out, "released in battle: stays out");
    write_trace("hero_light_leader_swap", &rig.trace);
}

#[test]
fn every_staff_and_shield_has_its_look_and_socket() {
    let _out = nocombat::scope(false);
    assert_eq!(WEAPONS.iter().filter(|w| w.chara == Chara::Donald).count(), 14);
    assert_eq!(WEAPONS.iter().filter(|w| w.chara == Chara::Goofy).count(), 14);
    // KH3's put-away delays from each friend's own clip rows (STATE.md lists them)
    let delays: Vec<String> = Chara::ALL.iter().map(|c| { let len = |n: &str| tables::clip(*c, n).length; let (stand, run) = put_away_delays(*c); format!("{c:?}: IdleN {:.3} s, IdleBreakW {:.3} s, RunBattleEnd {:.3} s, RunLoopW {:.3} s -> standing {stand:.3} s, moving {run:.3} s", len("IdleN"), len("IdleBreakW"), len("RunBattleEnd"), len("RunLoopW")) }).collect();
    for c in Chara::ALL {
        for n in ["IdleN", "IdleBreakW", "RunBattleEnd", "RunLoopW"] {
            assert!(tables::anim_set(c).get(n).is_some(), "{c:?}: no {n} row");
        }
    }
    write_trace("put_away_delays", &delays);
    for w in WEAPONS {
        assert_eq!(weapon_row(w.key), Some(&w));
        assert_eq!(weapon_row(&w.key.replace('_', "")).map(|r| r.model), Some(w.model), "the def spelling {} too", w.key);
        assert_eq!(weapon_row(w.model).map(|r| r.key), Some(w.key));
        let mut rig = Rig::new(None);
        let id = rig.id(w.chara);
        rig.party.member_mut(id).unwrap().status.weapon = w.key;
        rig.step(true);
        let v = rig.view(w.chara);
        assert!(v.out, "{}", w.key);
        assert_eq!(v.look, w.look);
        let (name, at) = *rig.effects.0.iter().find(|(_, at)| matches!(at, Attach::Party { member, .. } if member.0 == id.0)).unwrap();
        assert_eq!(name, effect_of(w.chara), "{}: {}", w.key, w.model);
        assert!(matches!(at, Attach::Party { bone: Some(b), .. } if b == w.socket), "{}", w.key);
    }
}

/// The trace files (build/sora_tmp/weapons/PARTY-WEAPONS/traces/<name>.txt) for STATE.md.
fn write_trace(name: &str, lines: &[String]) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/PARTY-WEAPONS/traces");
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(dir.join(format!("{name}.txt")), lines.join("\n"));
    }
}

/// One step of a member's clip trace: (time, clip, clip time, put-away sequence, weapon out).
type ClipRow = (f32, &'static str, f32, PutAway, bool);

/// One member's clip sequence after the battle (a [`ClipRow`] per step) and the step its effect spawned.
fn clip_trace(rig: &mut Rig, c: Chara, seconds: f32, each: &mut dyn FnMut(&mut Rig)) -> (Vec<ClipRow>, Option<usize>) {
    let mut rows = Vec::new();
    let mut effect_at = None;
    let before = rig.appears(c);
    for k in 0..(seconds / DT).round() as usize {
        each(rig);
        rig.step(false);
        let m = rig.party.member(rig.id(c)).unwrap();
        rows.push((rig.t, m.anim.clip.name, m.anim.time, m.weapon.put_away, m.weapon.out));
        if effect_at.is_none() && rig.appears(c) > before {
            effect_at = Some(k);
        }
        if !m.weapon.out && effect_at.is_some() {
            break;
        }
    }
    (rows, effect_at)
}

#[test]
fn standing_after_a_battle_plays_battle_end_two_idles_and_the_break_then_puts_it_away_at_the_breaks_end() {
    let _out = nocombat::scope(false);
    let mut lines = Vec::new();
    for c in Chara::ALL {
        let mut rig = Rig::new(None);
        rig.run(true, 1.0);
        let (rows, effect_at) = clip_trace(&mut rig, c, 40.0, &mut |_| {});
        // the clips in order (each row's first step)
        let mut order: Vec<&str> = Vec::new();
        for r in &rows {
            if order.last() != Some(&r.1) {
                order.push(r.1);
            }
        }
        assert_eq!(&order[..3], &[BATTLE_END, "IdleN", IDLE_BREAK][..], "{c:?}: BattleEnd -> IdleN -> IdleBreakW ({order:?})");
        // two idle loops before the break: the IdleN stretch is two of its lengths
        let idle_len = tables::clip(c, "IdleN").length;
        let idle_steps = rows.iter().filter(|r| r.1 == "IdleN" && r.3 != PutAway::None).count() as f32 * DT;
        assert!((idle_steps - 2.0 * idle_len).abs() < 3.0 * DT, "{c:?}: two IdleN loops ({idle_steps} s vs 2 x {idle_len})");
        // the weapon goes at the break's end: the step its effect spawned is the first after IdleBreakW ran its length
        let k = effect_at.unwrap_or_else(|| panic!("{c:?}: never put away ({:?})", rows.last()));
        let brk = tables::clip(c, IDLE_BREAK).length;
        let prev = rows[k - 1];
        assert_eq!(prev.1, IDLE_BREAK, "{c:?}: the step before the put-away still plays the break");
        assert!(prev.2 >= brk - 2.0 * DT, "{c:?}: the break ran to its end ({} of {brk})", prev.2);
        assert!(!rows[k].4 && rows[k].1 == "IdleN", "{c:?}: put away, the idle again ({:?})", rows[k]);
        let m = rig.party.member(rig.id(c)).unwrap();
        assert_eq!(m.weapon.last, Some((false, Why::Idle)));
        let total = rows[k].0 - rows[0].0 + DT;
        let (stand, _) = put_away_delays(c);
        assert!((total - stand).abs() < 0.1, "{c:?}: {total} s after the battle vs KH3's clips {stand} s");
        lines.push(format!("{c:?}: {order:?}; put away {total:.3} s after the battle (BattleEnd {:.3} + 2 x IdleN {idle_len:.3} + IdleBreakW {brk:.3}), the effect on the break's last frame", tables::clip(c, BATTLE_END).length));
    }
    write_trace("clips_standing", &lines);
}

#[test]
fn moving_after_a_battle_plays_run_battle_end_then_puts_it_away_past_80_percent_of_the_loop() {
    use crate::command::{LocomotionDef, MoveGoal};
    let _out = nocombat::scope(false);
    let mut lines = Vec::new();
    for c in Chara::ALL {
        let mut rig = Rig::new(None);
        rig.party.gof.kh3_ai = true; // (AI Goofy walks the MoveTo with KH3's tree switch)
        rig.run(true, 1.0);
        let id = rig.id(c);
        let far = rig.party.member(id).unwrap().position() + Vec3::new(4000.0, 0.0, 0.0);
        rig.party.command(id, MemberCommand::MoveTo { goal: MoveGoal::Point(far), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 100.0, follow_path: false });
        // into the run while the battle lasts, then the battle ends while he runs
        rig.run(true, 1.5);
        let mut held = 0;
        let (rows, effect_at) = clip_trace(&mut rig, c, 10.0, &mut |rig: &mut Rig| {
            if rig.party.member(id).is_some_and(|m| m.anim.clip.name == RUN_BATTLE_END) && rig.party.ai_loco(id).is_none() {
                held += 1;
            }
        });
        let ran = rows.iter().filter(|r| r.1 == RUN_BATTLE_END).count() as f32 * DT;
        let end_len = tables::clip(c, RUN_BATTLE_END).length;
        assert!(ran >= end_len - 3.0 * DT, "{c:?}: RunBattleEnd played out ({ran} s of {end_len}; {:?})", rows.iter().map(|r| r.1).collect::<std::collections::BTreeSet<_>>());
        assert!(held > 0, "{c:?}: the speed-matched loop yielded to it (drawn from the clip)");
        let k = effect_at.unwrap_or_else(|| panic!("{c:?}: never put away ({:?})", rows.last()));
        let loop_row = rows[k];
        let len = tables::clip(c, loop_row.1).length.max(1e-4);
        assert!(loop_row.1 != RUN_BATTLE_END && rows[..k].iter().any(|r| r.1 == RUN_BATTLE_END), "{c:?}: after RunBattleEnd, on the loop");
        assert!(loop_row.2 / len > RUN_LOOP_SHARE, "{c:?}: past 80 % of {} ({} / {len})", loop_row.1, loop_row.2);
        assert_eq!(rig.party.member(id).unwrap().weapon.last, Some((false, Why::Run)));
        lines.push(format!("{c:?}: RunBattleEnd {ran:.3} s (the loop held off {held} steps), then {} put away at {:.3} of {len:.3} s ({:.0} %), {:.3} s after the battle", loop_row.1, loop_row.2, 100.0 * loop_row.2 / len, loop_row.0 - rows[0].0 + DT));
    }
    write_trace("clips_moving", &lines);
}

/// The sequences of a loco pose (walk + the run samples).
fn pose_seqs(p: &crate::pad::LocoPose) -> Vec<&'static str> {
    let mut v = vec![p.walk];
    v.extend(p.run.iter().filter(|r| r.1 > 0.0).map(|r| r.0));
    v
}

#[test]
fn out_of_battle_with_the_weapon_out_the_locomotion_is_the_w_set_and_back_to_n_once_put_away() {
    use crate::command::{LocomotionDef, MoveGoal};
    let _out = nocombat::scope(false);
    let mut lines = Vec::new();
    for c in Chara::ALL {
        // AI: the weapon held out (the hero light's hold: out of battle, drawn) while he runs a MoveTo
        let mut rig = Rig::new(None);
        rig.party.gof.kh3_ai = true;
        let id = rig.id(c);
        rig.party.set_weapon_hold(id, true);
        rig.run(false, 0.5);
        let far = rig.party.member(id).unwrap().position() + Vec3::new(6000.0, 0.0, 0.0);
        rig.party.command(id, MemberCommand::MoveTo { goal: MoveGoal::Point(far), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 100.0, follow_path: false });
        let mut starts = std::collections::BTreeSet::new();
        let mut w_pose = None;
        for _ in 0..120 {
            rig.step(false);
            let m = rig.party.member(id).unwrap();
            if m.anim.clip.name.starts_with("RunStart") {
                starts.insert(m.anim.clip.name);
            }
            if let Some(p) = rig.party.ai_loco(id) {
                w_pose = Some((pose_seqs(&p), m.anim.clip.name));
            }
        }
        let (seqs, dominant) = w_pose.unwrap_or_else(|| panic!("{c:?}: never in the loop"));
        assert!(rig.view(c).out, "{c:?}: held out");
        assert!(seqs.iter().all(|s| s.starts_with("100")), "{c:?}: WalkRunW's samples 1002 / 1003* ({seqs:?})");
        assert!(dominant.ends_with('W') || dominant == "WalkLoopN", "{c:?}: the slot's row of the W set ({dominant})");
        assert!(starts.iter().all(|s| *s == "RunStartW"), "{c:?}: the start of the W set ({starts:?})");
        // the start / stop / loop rows of the W set (KH3's name builders 0x1412645F0 / 0x141264350 / 0x141264520)
        assert_eq!(crate::pad::start_clip(c, crate::pad::Set::W, 0.0).name, "RunStartW", "{c:?}");
        assert!(crate::pad::start_length(c, crate::pad::Set::W, 0.0).is_some(), "{c:?}: RunStartW's samples 1007* have lengths");
        let m = rig.party.member(id).unwrap();
        assert_eq!(crate::pad::set_of(m), crate::pad::Set::W, "{c:?}");
        assert_eq!(crate::pad::row(m, "RunEndR", "RunEndRB"), "RunEndRW", "{c:?}");
        assert_eq!(crate::pad::row(m, "WalkEndL", "WalkEndLB"), "WalkEndLW", "{c:?}");
        assert_eq!(crate::pad::row(m, "RunLoopN", "RunLoopB"), "RunLoopW", "{c:?}");
        assert_eq!(crate::pad::row(m, "IdleN", "IdleB"), "IdleN", "{c:?}: no IdleW (slot 909)");
        // the hold released out of battle: put away at once, the N set again
        rig.party.set_weapon_hold(id, false);
        rig.run(false, 1.0);
        assert!(!rig.view(c).out);
        let p = rig.party.ai_loco(id).unwrap_or_else(|| panic!("{c:?}: still in the loop"));
        let back = pose_seqs(&p);
        assert!(back.iter().all(|s| s.starts_with("000")), "{c:?}: WalkRunN's samples 0002 / 0003* again ({back:?})");
        // in battle: B whatever the weapon (the battle pose wins)
        rig.run(true, 1.0);
        let b = pose_seqs(&rig.party.ai_loco(id).unwrap());
        assert!(b.iter().all(|s| s.starts_with("101")), "{c:?}: WalkRunB in battle ({b:?})");
        lines.push(format!("{c:?} AI: weapon out of battle {seqs:?} (slot {dominant}, starts {starts:?}); put away {back:?}; battle {b:?}"));
        // the pad leader: the same rows on his run
        let mut rig = Rig::new(Some(c));
        let id = rig.id(c);
        rig.party.set_weapon_hold(id, true);
        rig.run(false, 0.3);
        let mut pad_pose = None;
        for _ in 0..90 {
            rig.party.set_pad_frame(id, crate::pad::PadFrame { stick: glam::Vec2::new(0.0, 1.0), camera_yaw: 0.0, pad: Default::default() });
            rig.step(false);
            if let Some(p) = rig.party.pad_state(id).and_then(|s| s.loco) {
                pad_pose = Some(pose_seqs(&p));
            }
        }
        let seqs = pad_pose.unwrap_or_else(|| panic!("{c:?}: the pad never ran the loop"));
        assert!(seqs.iter().all(|s| s.starts_with("100")), "{c:?} on the pad: WalkRunW ({seqs:?})");
        lines.push(format!("{c:?} leading: weapon out of battle {seqs:?}"));
    }
    // KH3's rows: the W set has no idle (slot 909 IdleN / IdleB): the standing friend keeps IdleN with the weapon out
    for c in Chara::ALL {
        let set = tables::anim_set(c);
        assert!(set.get("IdleW").is_none() && set.get("RunLoopW").is_some() && set.get("RunStartW").is_some() && set.get("WalkRunW").is_some(), "{c:?}");
    }
    write_trace("w_set", &lines);
}

/// (PARTY-WEAPONS, the user: "does the animation they play after battle make it impossible for them to move as party
/// leader?") A pad input held for `steps` (pressed on its first): the member's state, horizontal speed and yaw after it.
fn drive(rig: &mut Rig, id: MemberId, stick: glam::Vec2, button: Option<kh_core::input::PadButton>, steps: usize) -> (NpcStateId, f32, f32, Vec<NpcStateId>) {
    use kh_core::input::{PadButtons, PadState};
    let mut seen = Vec::new();
    for k in 0..steps {
        let held = button.filter(|_| k < 3).map_or(PadButtons::NONE, |b| PadButtons::NONE.with(b));
        let pressed = if k == 0 { held } else { PadButtons::NONE };
        let released = if k == 3 { button.map_or(PadButtons::NONE, |b| PadButtons::NONE.with(b)) } else { PadButtons::NONE };
        rig.party.set_pad_frame(id, crate::pad::PadFrame { stick, camera_yaw: 0.0, pad: PadState { held, pressed, released } });
        rig.step(false);
        let m = rig.party.member(id).unwrap();
        if seen.last() != Some(&m.state) {
            seen.push(m.state);
        }
        assert!(!holds_clip(m, false) || m.anim.clip.name == RUN_BATTLE_END, "only RunBattleEnd may hold the loop off while he moves");
    }
    let m = rig.party.member(id).unwrap();
    (m.state, m.cmc.velocity.truncate().length(), m.yaw, seen)
}

/// Which put-away phase a test stops in.
type PhaseTest = fn(PutAway) -> bool;

/// A pad-led friend after a battle, stopped on the first step of `phase` (BattleEnd, the IdleN loops, IdleBreakW).
fn leader_in(c: Chara, phase: PhaseTest) -> (Rig, MemberId) {
    let mut rig = Rig::new(Some(c));
    let id = rig.id(c);
    rig.run(true, 1.0);
    for _ in 0..(40.0 / DT) as usize {
        rig.party.set_pad_frame(id, crate::pad::PadFrame { stick: glam::Vec2::ZERO, camera_yaw: 0.0, pad: Default::default() });
        rig.step(false);
        if phase(rig.party.member(id).unwrap().weapon.put_away) {
            return (rig, id);
        }
    }
    panic!("{c:?}: the phase never came ({:?})", rig.party.member(id).unwrap().weapon.put_away);
}

#[test]
fn a_pad_leader_moves_turns_jumps_attacks_and_guards_in_every_put_away_phase() {
    use kh_core::input::PadButton;
    let _out = nocombat::scope(false);
    let phases: [(&str, PhaseTest); 3] = [("BattleEnd", |p| p == PutAway::BattleEnd), ("IdleN loops", |p| matches!(p, PutAway::Idle { .. })), ("IdleBreakW", |p| p == PutAway::Break)];
    let mut lines = Vec::new();
    for c in Chara::ALL {
        // the baseline: the same leader standing with nothing to put away
        let base = |stick: glam::Vec2, button: Option<PadButton>, steps: usize| {
            let mut rig = Rig::new(Some(c));
            let id = rig.id(c);
            rig.run(false, 1.0);
            let yaw0 = rig.party.member(id).unwrap().yaw;
            let (s, v, yaw, seen) = drive(&mut rig, id, stick, button, steps);
            (s, v, kh_core::leap::wrap180(yaw - yaw0), seen)
        };
        let run = base(glam::Vec2::new(0.0, 1.0), None, 40);
        let turn = base(glam::Vec2::new(1.0, 0.0), None, 40);
        let buttons = [PadButton::JUMP, PadButton::ATTACK, PadButton::GUARD];
        let acts: Vec<_> = buttons.iter().map(|b| base(glam::Vec2::ZERO, Some(*b), 12).3).collect();
        for (name, phase) in phases {
            // the stick on the phase's first step: he runs at once, at the same speed, and the clip yields
            let (mut rig, id) = leader_in(c, phase);
            let clip = rig.party.member(id).unwrap().anim.clip.name;
            let (state, v, _, _) = drive(&mut rig, id, glam::Vec2::new(0.0, 1.0), None, 40);
            assert_eq!(state, NpcStateId::Pad(crate::pad::RUN), "{c:?} {name}: he runs");
            assert!((v - run.1).abs() <= 0.05 * run.1.max(1.0), "{c:?} {name}: full speed ({v} vs {})", run.1);
            let m = rig.party.member(id).unwrap();
            // (the battle pose still on, KH3's Land plays RunBattleEnd over the run: at full speed, see below)
            assert!(m.anim.clip.name != clip && (rig.party.pad_state(id).is_some_and(|p| p.loco.is_some()) || m.anim.clip.name == RUN_BATTLE_END), "{c:?} {name}: {clip} gave way to the run ({})", m.anim.clip.name);
            let (mut rig, id) = leader_in(c, phase);
            let yaw0 = rig.party.member(id).unwrap().yaw;
            let (_, _, yaw, _) = drive(&mut rig, id, glam::Vec2::new(1.0, 0.0), None, 40);
            assert!((kh_core::leap::wrap180(yaw - yaw0) - turn.2).abs() < 5.0, "{c:?} {name}: he turns as ever ({} vs {})", kh_core::leap::wrap180(yaw - yaw0), turn.2);
            // jump, attack, guard / dodge: what the same press does standing
            for (b, want) in buttons.iter().zip(&acts) {
                let (mut rig, id) = leader_in(c, phase);
                let (_, _, _, seen) = drive(&mut rig, id, glam::Vec2::ZERO, Some(*b), 12);
                assert_eq!(&seen, want, "{c:?} {name}: {b:?} does what it does standing");
            }
            lines.push(format!("{c:?} leading, {name} ({clip}): runs at {v:.0} cm/s (baseline {:.0}), turns, {:?} -> {:?}", run.1, buttons, acts.iter().map(|s| s.iter().find(|x| !matches!(x, NpcStateId::Idle)).copied()).collect::<Vec<_>>()));
        }
        // RUN BATTLE END: the leader running when the battle ends keeps his speed and his steering through it (no root lock)
        let mut rig = Rig::new(Some(c));
        let id = rig.id(c);
        for _ in 0..90 {
            rig.party.set_pad_frame(id, crate::pad::PadFrame { stick: glam::Vec2::new(0.0, 1.0), camera_yaw: 0.0, pad: Default::default() });
            rig.party.set_in_battle(true);
            let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummy, audio: &mut rig.audio, effects: &mut rig.effects };
            rig.party.step(DT, &rig.sora, &mut host);
        }
        let v_battle = rig.party.member(id).unwrap().cmc.velocity.truncate().length();
        let mut during = Vec::new();
        let yaw0 = rig.party.member(id).unwrap().yaw;
        for k in 0..40 {
            let stick = if k < 10 { glam::Vec2::new(0.0, 1.0) } else { glam::Vec2::new(1.0, 1.0).normalize() };
            rig.party.set_pad_frame(id, crate::pad::PadFrame { stick, camera_yaw: 0.0, pad: Default::default() });
            rig.step(false);
            let m = rig.party.member(id).unwrap();
            if m.anim.clip.name == RUN_BATTLE_END {
                during.push(m.cmc.velocity.truncate().length());
            }
        }
        let turned = kh_core::leap::wrap180(rig.party.member(id).unwrap().yaw - yaw0);
        assert!(!during.is_empty(), "{c:?}: RunBattleEnd played on the leader's run");
        let slowest = during.iter().copied().fold(f32::MAX, f32::min);
        assert!(slowest >= 0.9 * v_battle, "{c:?}: full speed through RunBattleEnd ({slowest} vs {v_battle})");
        assert!(turned.abs() > 20.0, "{c:?}: he steers through it ({turned} deg)");
        lines.push(format!("{c:?} leading, RunBattleEnd: {} steps at >= {slowest:.0} cm/s (running {v_battle:.0}), steered {turned:.0} deg", during.len()));
    }
    write_trace("leader_free_in_put_away", &lines);
}

