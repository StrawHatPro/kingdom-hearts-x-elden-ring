//! (AUDIT-B F-CLAWS) THE FORM WEAPONS' OWN CLIP NOTIFIES through the real core: KH3's form weapons play their own clip of Sora's
//! play and that clip's AttachEffect / PlaySound / AttachSound notifies fire on the weapon (and the Chandelier's on the
//! Chandelier). The rows `wfx.*` of sheets/port/sora-form-claws.json (tools/sora_form_claws.js), played by
//! `combat::forms::claws::on_notifies` / `chandelier_fx`. Sessions: Happy Gear (Agile Claws' combo, Twin Yo-Yos' combo, FINISH_YO's
//! giant yo-yo) and Crystal Snow (Blizzard Claws' combo with the ice group 0x7DA, FINISH_SK's Chandelier with its sounds).

use kh_core::host::recording::Call;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

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

fn dummy(x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(1), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

/// What the seam got in one step: (step, Sora's clip, effects spawned (name, place), sounds played (name)).
type StepLog = (usize, &'static str, Vec<(&'static str, Vec3)>, Vec<&'static str>);

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    log: Vec<StepLog>,
}

impl Bench {
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(200.0));
        let mut b = Bench { sora, host, world: TestWorld::floor(), log: Vec::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let before = self.host.calls.len();
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        let mut fx = Vec::new();
        let mut se = Vec::new();
        for c in &self.host.calls[before..] {
            match c {
                Call::Spawn(_, s) => fx.push((s.name, match s.at { kh_core::Attach::World(p) => p, _ => Vec3::NAN })),
                Call::Play(_, p) => se.push(p.name),
                _ => {}
            }
        }
        self.log.push((self.log.len(), out.anim_ticked.body_samples()[0].0, fx, se));
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn transform(&mut self, cmd: i32) {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        for _ in 0..120 {
            if self.sora.combat().situation.list.iter().any(|e| e.kind == cmd && e.appear <= 0.0 && e.start_delay <= 0.0) {
                break;
            }
            self.step(&[]);
        }
        self.step(&[PadButton::SITUATION]);
        for _ in 0..400 {
            if self.step(&[]).state != StateId::Stylechange && self.sora.combat().form.cmd == cmd {
                return;
            }
        }
        panic!("the form {cmd:#X} did not start");
    }

    fn combo(&mut self) {
        for k in 0..900 {
            let out = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
    }

    fn finish(&mut self) {
        self.step(&[PadButton::SITUATION]);
        for _ in 0..3000 {
            let out = self.step(&[]);
            if out.state != StateId::Stylefinish && self.sora.finish().is_none() && self.sora.combat().form.cmd == 0 {
                break;
            }
        }
    }

    /// The (step, clip, effect, place) of every spawn from `from` whose name starts with `prefix`.
    fn spawns(&self, from: usize, prefix: &str) -> Vec<(usize, &'static str, &'static str, Vec3)> {
        self.log[from..].iter().flat_map(|(k, clip, fx, _)| fx.iter().filter(|(n, _)| n.starts_with(prefix)).map(move |(n, p)| (*k, *clip, *n, *p))).collect()
    }

    fn sounds(&self, from: usize, part: &str) -> Vec<(usize, &'static str)> {
        self.log[from..].iter().flat_map(|(k, _, _, se)| se.iter().filter(|n| n.contains(part)).map(move |_| (*k, ""))).collect()
    }

    /// A text trace of the weapon / pawn rows' seam commands.
    fn trace(&self, from: usize) -> String {
        let mut s = String::new();
        for (k, clip, fx, se) in &self.log[from..] {
            for (n, p) in fx.iter().filter(|(n, _)| n.starts_with("vw_so06") || n.starts_with("vw_so08")) {
                s.push_str(&format!("{k:5} {clip:>14} effect {n} at ({:.0}, {:.0}, {:.0})\n", p.x, p.y, p.z));
            }
            for n in se.iter().filter(|n| n.contains("/weapon/0200")) {
                s.push_str(&format!("{k:5} {clip:>14} sound {n}\n"));
            }
        }
        s
    }
}

fn write_trace(name: &str, text: &str) {
    let dir = format!("{}/../../build/sora_tmp/weapons/AUDIT-B/F-CLAWS/traces", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(format!("{dir}/{name}"), text).ok();
}

#[test]
fn happy_gear_weapons_play_their_own_clips() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_05");
    b.transform(0x8A);
    b.idle(30);
    let from = b.log.len();
    b.combo();
    // Agile Claws: the claws' own sounds (w_so062's C411 .. C415 PlaySound notifies, se02006_*) with Sora's swings
    let wanted: Vec<&str> = kh_core::combat::forms::claws::weapon_fx_rows(0x8A).map(|(_, r)| r).filter(|r| r.flags.get(1) == Some(&"sound")).map(|r| r.logical).collect();
    assert!(b.log[from..].iter().any(|(_, _, _, se)| se.iter().any(|n| wanted.contains(n))), "the claws' own clip sounds {wanted:?}:\n{}", b.trace(from));
    let claws = b.trace(from);
    b.idle(30);
    b.transform(0x82);
    b.idle(30);
    let combo = b.log.len();
    b.combo();
    let yoyo = b.spawns(combo, "vw_so064");
    assert!(yoyo.iter().any(|(_, clip, n, _)| *clip == "4411" && *n == "vw_so064_bem_lin00"), "the yo-yo string of M_4411 (w_so064 vfx_himo00):\n{}", b.trace(combo));
    assert!(yoyo.iter().any(|(_, clip, ..)| *clip == "4411") && yoyo.iter().any(|(_, clip, ..)| *clip == "4412"), "both yo-yos' clips play");
    b.idle(10);
    let fin = b.log.len();
    let start = b.sora.output().position;
    b.finish();
    let giant = b.spawns(fin, "vw_so064_fsh");
    let names: Vec<&str> = giant.iter().map(|g| g.2).collect();
    for n in ["vw_so064_fsh_sho00", "vw_so064_fsh_clw00", "vw_so064_fsh_clw01", "vw_so064_fsh_clw02", "vw_so064_fsh_flh01", "vw_so064_fsh_sho01", "vw_so064_fsh_flh03"] {
        assert!(names.contains(&n), "{n} of the giant yo-yo's M_4420: {names:?}");
    }
    // the giant yo-yo stands 3 - 6 m in front of him: its effects are there, not on his hand
    let sho = giant.iter().find(|g| g.2 == "vw_so064_fsh_sho00").expect("sho00");
    assert!((sho.3 - start).truncate().length() > 250.0, "on the giant yo-yo: {:?} (he stood at {start})", sho.3);
    // the flashes at the three YO_Chew windows (3.333 / 3.781 / 4.207 s), in order
    // (both yo-yos' own 4420 carry them: w_so064's and w_so065's)
    let mut flh: Vec<usize> = giant.iter().filter(|g| g.2.starts_with("vw_so064_fsh_flh")).map(|g| g.0).collect();
    assert_eq!(flh.len(), 6, "{flh:?}");
    flh.dedup();
    assert_eq!(flh.len(), 3, "three moments: {flh:?}");
    write_trace("wfx_happy_gear.trace", &format!("# Agile Claws combo\n{claws}# Twin Yo-Yos combo\n{}# FINISH_YO\n{}", b.trace(combo), b.trace(fin)));
    // every live weapon row ended with the form
    b.idle(20);
    assert_eq!(b.sora.combat().form.cmd, 0);
}

#[test]
fn crystal_snow_ice_claws_and_the_chandelier_play_their_own_clips() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_06");
    b.transform(0x90);
    b.idle(30);
    let combo = b.log.len();
    b.combo();
    // Blizzard Claws: the ice weapons' (w_so082 / w_so083 I412 / I415 / I418) effects of group 0x7DA, which the claws' plays pass
    let ice = b.spawns(combo, "vw_so082");
    assert!(ice.iter().any(|(_, clip, n, _)| *clip == "C415" && n.starts_with("vw_so082_dwc_")), "C415's ice claws (I415):\n{}", b.trace(combo));
    let claws = b.trace(combo);
    b.idle(30);
    b.transform(0x85);
    b.idle(30);
    b.combo();
    b.idle(10);
    let fin = b.log.len();
    b.finish();
    let pawn = b.spawns(fin, "vw_so088_fin");
    let names: Vec<&str> = pawn.iter().map(|p| p.2).collect();
    for n in ["vw_so088_fin_rsn01", "vw_so088_fin_kira00", "vw_so088_fin_kira01", "vw_so088_fin_han00", "vw_so088_fin_han01", "vw_so088_fin_hahen00", "vw_so088_fin_hahen01", "vw_so088_fin_hahen02", "vw_so088_fin_hahen03", "vw_so088_fin_mat"] {
        assert!(names.contains(&n), "{n} of the Chandelier's 7423: {names:?}");
    }
    assert_eq!(names.iter().filter(|n| n.starts_with("vw_so088_fin_kira")).count(), 7, "{names:?}");
    // its two sounds, from the data (XS's hand list had none)
    assert_eq!(b.sounds(fin, "se02008_041").len(), 1, "{}", b.trace(fin));
    assert_eq!(b.sounds(fin, "se02008_042").len(), 1, "{}", b.trace(fin));
    // the sparkles are on its body (its vfx_center sockets, 2 - 6 m up), not at its foot
    let kira = pawn.iter().find(|p| p.2 == "vw_so088_fin_kira00").expect("kira");
    let rsn = pawn.iter().find(|p| p.2 == "vw_so088_fin_rsn01").expect("rsn");
    assert!(kira.3.z - rsn.3.z > 150.0, "{:?} {:?}", kira.3, rsn.3);
    write_trace("wfx_crystal_snow.trace", &format!("# Blizzard Claws combo\n{claws}# FINISH_SK\n{}", b.trace(fin)));
}
