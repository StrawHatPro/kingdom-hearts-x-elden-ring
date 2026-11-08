//! (HM-MAGIC) KH3's turn-type magic set `p_ex001_hm_MagicSet` through the real core (build/sora_tmp/weapons/HM-MAGIC/STATE.md):
//! for each of the six formchanges that install it (Favorite Deputy's Hyper Hammer, Happy Gear's Agile Claws, Crystal Snow's
//! Blizzard Claws and Blizzard Blades, Wheel of Fate's Storm Flag, Classic Tone's Boom Hammer) x each Grand Magic
//! (S_FIRAGAN, S_BLIZZAGAN, S_THUNDAGAN, S_WATAGAN, S_AEROGAN): the set swap at StartStyle, the MagicSpin clip, the shot at
//! its Shot notify, the `TresProj_Mag*ganCL` actor at the form weapon's ATK_01 with its effects and sounds, its hits (the
//! MAG_C_*GAN row, again every interval), no MP; a normal spell's MagicTurn and its MP; Cure kept from the base set; and the
//! base set back after EndStyle. Each pair writes its trace to build/sora_tmp/weapons/HM-MAGIC/traces/.

use kh_core::host::Call;
use kh_core::magic::hm_set;
use kh_core::magic::projectile::FIRE_EFFECTS_ID;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};
use std::fmt::Write as _;

const DT: f32 = 1.0 / 60.0;

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

fn holding(key: &str) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let k = row[0].as_str().unwrap();
        let value = if k == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(k, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut s = Status::from_loadout(&loadout);
    s.weapon = key.to_string();
    s
}

fn dummy(id: u32, at: Vec3) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: at, center: at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e9, hp_max: 1.0e9 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    log: Vec<(StateId, &'static str, f32, i32)>,
}

impl Bench {
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut b = Bench { sora, host: RecordingHost::new(), world: TestWorld::floor(), log: Vec::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.log.push((out.state, out.anim.name.name(), out.anim.time, self.sora.combat().form.cmd));
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    /// The offer of `cmd`, Triangle, the transformation: (the step StartStyle ran in, the set before it, the set after it).
    fn transform(&mut self, cmd: i32) -> (usize, &'static str, &'static str) {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<i32> = self.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert_eq!(list, [cmd], "the formchange {cmd:#x} is offered");
        let before = hm_set::magic_set(self.sora.combat().form.cmd);
        let at = self.log.len();
        self.step(&[PadButton::SITUATION]);
        assert_eq!(self.sora.combat().form.cmd, cmd, "StartStyle ran in the Triangle step: the form is held");
        let after = hm_set::magic_set(self.sora.combat().form.cmd);
        for _ in 0..600 {
            if self.step(&[]).state != StateId::Stylechange {
                return (at, before, after);
            }
        }
        panic!("STYLECHANGE did not end");
    }

    /// Executes a command (the menu's ExecuteCommand); steps until the cast is back to an idle state. Returns the trace.
    fn cast(&mut self, cmd: u8) -> Cast {
        let calls = self.host.calls.len();
        let from = self.log.len();
        let mp = self.sora.status().mp;
        self.sora.command_request(kh_core::command::CommandRequest::Execute(cmd));
        let mut spawned: Vec<(u32, &'static str, Vec3, f32, String)> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut shot_time = None;
        let mut cast_seen = false;
        let mut hit_steps: Vec<(u32, usize)> = Vec::new();
        for k in 0..400 {
            let before = self.host.calls.len();
            let out = self.step(&[]);
            for c in &self.host.calls[before..] {
                if let Call::Hit(r, _) = c {
                    hit_steps.push((r.target.0, k));
                }
            }
            if out.state == StateId::Firemagic {
                cast_seen = true;
                if shot_time.is_none() && self.sora.cast().is_some_and(|c| c.bullets > 0) {
                    shot_time = Some(out.anim.time);
                }
            }
            for p in self.sora.magic().projectile_views() {
                if seen.insert(p.id) {
                    spawned.push((p.id, p.class, p.position, p.shape.1, format!("{:?}", out.anim.name)));
                }
            }
            if cast_seen && out.state != StateId::Firemagic && self.sora.magic().projectile_views().count() == 0 {
                break;
            }
        }
        assert!(cast_seen, "the command {cmd} was cast: state {:?}, MP {}, menu {:?}, form {:#x}", self.sora.output().state, self.sora.status().mp, self.sora.ui_snapshot().command.events, self.sora.combat().form.cmd);
        let clips: Vec<&'static str> = self.log[from..].iter().filter(|l| l.0 == StateId::Firemagic).map(|l| l.1).fold(Vec::new(), |mut v, c| {
            if v.last() != Some(&c) {
                v.push(c);
            }
            v
        });
        Cast { calls: self.host.calls[calls..].to_vec(), clips, spawned, shot_time, mp_before: mp, mp_after: self.sora.status().mp, hit_steps }
    }
}

struct Cast {
    /// (target, the step of the cast it was hit in).
    hit_steps: Vec<(u32, usize)>,
    calls: Vec<Call>,
    clips: Vec<&'static str>,
    spawned: Vec<(u32, &'static str, Vec3, f32, String)>,
    shot_time: Option<f32>,
    mp_before: i32,
    mp_after: i32,
}

impl Cast {
    fn hits(&self) -> Vec<(TargetId, &'static str, f32)> {
        self.calls.iter().filter_map(|c| if let Call::Hit(r, _) = c { Some((r.target, r.attack_name, r.damage)) } else { None }).collect()
    }
    fn effects(&self) -> Vec<(String, u32, bool)> {
        self.calls.iter().filter_map(|c| if let Call::Spawn(_, s) = c { Some((s.name.to_string(), s.id.0, s.persistent)) } else { None }).collect()
    }
    fn sounds(&self) -> Vec<(String, u32)> {
        self.calls.iter().filter_map(|c| if let Call::Play(_, s) = c { Some((s.name.to_string(), s.id.0)) } else { None }).collect()
    }
}

/// The five Grand Magic ids: (command, the CL class, the unit's voice group, the hit row).
const GRAND: [(u8, &str, i32, &str); 5] = [
    (112, "TresProj_MagFiganCL_C", 3, "MAG_C_FIGAN"),
    (115, "TresProj_MagBlizzaganCL_C", 5, "MAG_C_BLIZZAGAN"),
    (118, "TresProj_MagThundaganCL_C", 7, "MAG_C_THUNDAGAN"),
    (121, "TresProj_MagWataganCL_C", 11, "MAG_C_WATAGAN"),
    (124, "TresProj_MagAeroganCL_C", 9, "MAG_C_AEROGAN"),
];

/// One form: its Keyblade, the first stage to pass through (a second-stage form), the form, its clips' prefix, its code.
fn the_form(key: &str, first: Option<i32>, form: i32, code: &str, spin: &str, turn: &str) {
    let mut trace = String::new();
    // ---- the swap at StartStyle: a fresh transformation for each Grand Magic (the form's 30 s are the form's own)
    let fresh = |trace: &mut String| {
        let mut b = Bench::new(key);
        assert_eq!(hm_set::magic_set(b.sora.combat().form.cmd), hm_set::MAIN_SET);
        if let Some(f) = first {
            let (_, before, after) = b.transform(f);
            assert_eq!((before, after), (hm_set::MAIN_SET, hm_set::magic_set(f)));
            b.idle(60);
        }
        let (at, before, after) = b.transform(form);
        assert_eq!(after, hm_set::HM_SET, "{code}: StartStyle installs p_ex001_hm_MagicSet");
        writeln!(trace, "{code} form {form:#x}: StartStyle at step {at}: set {before} -> {after}").unwrap();
        b.idle(20);
        b
    };
    // ---- the five Grand Magics
    for (cmd, class, voice, row) in GRAND {
        let mut b = fresh(&mut trace);
        // Three dummies inside the ring (the sphere grows 150 + 800 / s to 500 around the weapon's tip), one far outside.
        let o = b.sora.output();
        let a = o.yaw.to_radians();
        let (fwd, side) = (Vec3::new(a.cos(), a.sin(), 0.0), Vec3::new(-a.sin(), a.cos(), 0.0));
        let floor = Vec3::new(o.position.x, o.position.y, 0.0);
        b.host.targets = vec![dummy(1, floor + fwd * 180.0), dummy(2, floor - fwd * 160.0 + side * 120.0), dummy(3, floor + side * 250.0), dummy(4, floor + fwd * 2500.0)];
        let c = b.cast(cmd);
        let (shot, change) = hm_set::clip_times(form, "MagicSpin").unwrap();
        // the clip: MagicSpin of the form's set (a grand cast is a "B" cast), standing
        assert_eq!(c.clips, [spin], "{code} {cmd}: MagicSpin = {spin}");
        assert!(c.shot_time.is_some_and(|t| (t - shot).abs() <= DT * 1.01), "{code} {cmd}: the shot at the Shot notify {shot}: {:?}", c.shot_time);
        // the actor: one, the CL class, at the weapon's ATK_01 at the shot (not Sora's root), static
        assert_eq!(c.spawned.len(), 1, "{code} {cmd}: one actor: {:?}", c.spawned);
        let (_, spawned_class, position, _, _) = c.spawned[0];
        assert_eq!(spawned_class, class);
        let root = o.position;
        assert!((position - root).length() > 30.0, "{code} {cmd}: the actor sits at the form weapon's tip, not the root: {position} vs {root}");
        // its effects: the body (the class, persistent: the ring vm_*c_rng3 on the host) and the fire effects (the class with
        // FIRE_EFFECTS_ID at P)
        let fx = c.effects();
        assert!(fx.iter().any(|e| e.0 == class && e.2), "{code} {cmd}: the ring (persistent class event)");
        assert!(fx.iter().any(|e| e.0 == class && e.1 == FIRE_EFFECTS_ID), "{code} {cmd}: the fire effects");
        // the sounds: the unit's voice group (sora_magic:<voice>), the pawn-hit SE by the class
        let snd = c.sounds();
        let voice_name = format!("sora_magic:{voice}");
        assert!(snd.iter().any(|s| s.0 == voice_name), "{code} {cmd}: voice {voice_name}: {snd:?}");
        assert!(snd.iter().any(|s| s.0 == class && s.1 == kh_core::host::audio::ACTOR_PAWN_HIT.0), "{code} {cmd}: the class's pawn-hit SE");
        // the hits: the -gan row on the three dummies inside, each hit again after the interval; never the far one
        let hits = c.hits();
        assert!(hits.iter().all(|h| h.1 == row), "{code} {cmd}: every hit is {row}: {hits:?}");
        for id in 1..=3 {
            let n = hits.iter().filter(|h| h.0 == TargetId(id)).count();
            assert!(n >= 2, "{code} {cmd}: dummy {id} is hit again every interval ({n} hits)");
        }
        assert!(!hits.iter().any(|h| h.0 == TargetId(4)), "{code} {cmd}: the far dummy is not hit");
        // a grand magic costs nothing
        assert_eq!(c.mp_after, c.mp_before, "{code} {cmd}: no MP");
        let per: Vec<usize> = (1..=3).map(|id| hits.iter().filter(|h| h.0 == TargetId(id)).count()).collect();
        // the re-hits come every interval (TimerTick 0x14062CFC0: the list is emptied when the clock reaches it)
        let interval = hm_set::stage(class).unwrap().hit_interval;
        let gaps: Vec<usize> = c.hit_steps.iter().filter(|h| h.0 == 1).map(|h| h.1).collect::<Vec<_>>().windows(2).map(|w| w[1] - w[0]).collect();
        let expect = (interval / DT).round() as usize;
        assert!(!gaps.is_empty() && gaps.iter().all(|g| g.abs_diff(expect) <= 1), "{code} {cmd}: re-hit gaps {gaps:?} steps, interval {interval} s = {expect} steps");
        writeln!(trace, "{code} {cmd} ({row}): clip {:?}, shot at {:.4} s (notify {shot}, EnableChange {change}), actor {spawned_class} at {:.1} {:.1} {:.1} (root {:.1} {:.1} {:.1}), effects {:?}, sounds {:?}, hits per dummy {per:?}, dummy 1 re-hit every {gaps:?} steps (interval {interval} s), MP {} -> {}", c.clips, c.shot_time.unwrap_or(-1.0), position.x, position.y, position.z, root.x, root.y, root.z, tally(fx.iter().map(|e| format!("{}#{:x}", e.0, e.1))), tally(snd.iter().map(|s| format!("{}#{:x}", s.0, s.1))), c.mp_before, c.mp_after).unwrap();
    }
    // ---- a normal spell: Fira -> MagicTurn, TresProj_MagFiraCL_C, its MP
    let mut b = fresh(&mut trace);
    b.sora.status_mut().set_mp(100);
    let c = b.cast(30);
    assert_eq!(c.clips, [turn], "{code} Fira: MagicTurn = {turn}");
    assert_eq!(c.spawned.first().map(|s| s.1), Some("TresProj_MagFiraCL_C"));
    assert!(c.mp_after < c.mp_before, "{code} Fira costs MP");
    writeln!(trace, "{code} 30 (Fira): clip {:?}, actor {:?}, MP {} -> {}, hits {:?}", c.clips, c.spawned.first().map(|s| s.1), c.mp_before, c.mp_after, c.hits().iter().map(|h| h.1).collect::<Vec<_>>()).unwrap();
    b.idle(30);
    // ---- the swap at EndStyle: the form's time runs out
    let mut ended = None;
    for _ in 0..4000 {
        b.step(&[]);
        if b.sora.combat().form.cmd == 0 {
            ended = Some(b.log.len());
            break;
        }
    }
    let ended = ended.expect("the form ends");
    assert_eq!(hm_set::magic_set(b.sora.combat().form.cmd), hm_set::MAIN_SET, "{code}: EndStyle puts the base set back");
    writeln!(trace, "{code}: EndStyle at step {ended}: set -> {}", hm_set::magic_set(0)).unwrap();
    b.idle(60);
    b.sora.status_mut().set_mp(100);
    let c = b.cast(30);
    assert_eq!(c.spawned.first().map(|s| s.1), Some("TresProj_MagFiraNml_C"), "{code}: after the form, Fira is the base set's");
    assert!(!c.clips.contains(&turn), "{code}: no MagicTurn after the form: {:?}", c.clips);
    writeln!(trace, "{code} 30 (Fira) after the form: clip {:?}, actor {:?}", c.clips, c.spawned.first().map(|s| s.1)).unwrap();
    // ---- Cure in the form: the hm set has no Cure unit, the base set's (cast type, no actor of the CL family)
    let mut b = fresh(&mut trace);
    b.sora.status_mut().set_mp(100);
    let c = b.cast(49);
    assert!(c.clips.iter().all(|cl| *cl != turn && *cl != spin), "{code} Cure: not a turn clip: {:?}", c.clips);
    assert!(c.spawned.iter().all(|s| !s.1.ends_with("CL_C")), "{code} Cure: the base set's actor");
    writeln!(trace, "{code} 49 (Cure): clip {:?}, actor {:?}", c.clips, c.spawned.iter().map(|s| s.1).collect::<Vec<_>>()).unwrap();
    let dir = format!("{}/../../build/sora_tmp/weapons/HM-MAGIC/traces", env!("CARGO_MANIFEST_DIR"));
    if std::fs::create_dir_all(&dir).is_ok() {
        std::fs::write(format!("{dir}/{code}.trace.txt"), &trace).ok();
    }
}

#[test]
fn favorite_deputy_hyper_hammer() {
    the_form("WEP_KEYBLADE_SO_03", None, 0x81, "HM", "3382", "3381");
}

#[test]
fn happy_gear_agile_claws() {
    the_form("WEP_KEYBLADE_SO_05", None, 0x8A, "CL", "C382", "C381");
}

#[test]
fn crystal_snow_blizzard_claws() {
    the_form("WEP_KEYBLADE_SO_06", None, 0x90, "CL2", "C382", "C381");
}

#[test]
fn crystal_snow_blizzard_blades() {
    the_form("WEP_KEYBLADE_SO_06", Some(0x90), 0x85, "SK", "7382", "7381");
}

#[test]
fn wheel_of_fate_storm_flag() {
    the_form("WEP_KEYBLADE_SO_09", Some(0x8C), 0x8D, "FL", "F382", "F381");
}

#[test]
fn classic_tone_boom_hammer() {
    the_form("WEP_KEYBLADE_SO_11", None, 0x92, "HM2", "3382", "3381");
}

/// Every other form keeps the base set: a Grand Magic in Highwind (a `_bz_` form, not ported) and with the plain Keyblade
/// spawns the Nml actor and plays MagicShoot.
#[test]
fn other_forms_keep_the_base_set() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_09");
    b.host.targets = vec![dummy(1, Vec3::new(400.0, 0.0, 0.0))];
    let c = b.cast(112);
    assert_eq!(c.spawned.first().map(|s| s.1), Some("TresProj_MagFiganNml_C"));
    assert!(c.clips.contains(&"MagicShoot"), "{:?}", c.clips);
    b.idle(60);
    b.transform(0x8C);
    assert_eq!(hm_set::magic_set(0x8C), hm_set::MAIN_SET);
    b.idle(30);
    let c = b.cast(112);
    assert!(c.spawned.iter().all(|s| s.1.starts_with("TresProj_MagFiganNml") || s.1.starts_with("TresProj_FiganNml")) && !c.spawned.is_empty(), "Highwind: the base set's actors: {:?}", c.spawned);
}

/// "name x count" in first-seen order.
fn tally(items: impl Iterator<Item = String>) -> Vec<String> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for item in items {
        match out.iter_mut().find(|(n, _)| *n == item) {
            Some(entry) => entry.1 += 1,
            None => out.push((item, 1)),
        }
    }
    out.into_iter().map(|(n, k)| if k > 1 { format!("{n} x{k}") } else { n }).collect()
}
