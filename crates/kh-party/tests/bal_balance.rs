//! (GOOFY-BAL) THE BALANCE SIMULATOR: how many hits / combos each leader (Sora, Donald, Goofy) needs to kill a set of
//! reference Elden Ring enemies, at several levels, through the REAL code paths:
//! - Sora: kh-core's own Sora (the base 4-step combo against one dummy: KB_VERTICAL, KB_STAB, KB_TURN, KB_BREAK) for
//!   the rows, `compute_damage` with his status at each level (sora-levels attack + the Kingdom Key) for the numbers;
//!   the level-99 run's own HitRecord damage is checked against that formula.
//! - Donald / Goofy: kh-party's pad (the friend LEADER's own combo, `set_pad`), A mashed against one dummy that answers
//!   `Targets::defense` with None (Elden Ring's targets have no KH3 status object: `Defender::NONE`), from a status at
//!   the shared EXP of each level (the real progression, `PartyConfig::level99` false, no kit abilities, no Combo Plus).
//! - Elden Ring's side is a MODEL (this file, `er_hp`): attack value = KH3 damage x `damage.out.scale` 7 (sheet
//!   sora-er-host), then Elden Ring's defence curve on the target's physical defence and absorption. The curve is the
//!   game's published one; the model is checked against the in-game measurements of the sora-er-host row
//!   `damage.out.scale` (Godrick soldier c4311: attacks 53 / 81 / 132 took 9 / 23 / 69 HP).
//!
//! `cargo test --release -p kh-party --test bal_balance -- --nocapture` prints the tables (build/sora_tmp/party/
//! BAL_DESIGN.md, IMPL_BAL_STATE.md).

use glam::{Vec2, Vec3};
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::status::{compute_damage, Attacker, DamageAttack, DamageContext, Defender, GameLevel, Side};
use kh_core::test_world::TestWorld;
use kh_core::Tuning;
use kh_party::pad::PadFrame;
use kh_party::{Chara, Party, PartyConfig, PartyHost, SoraView};

const DT: f32 = 1.0 / 60.0;
const OUT_SCALE: f32 = 7.0;
const LEVELS: [u32; 6] = [1, 10, 20, 40, 60, 99];

// ------------------------------------------------------------------------------------------- the Elden Ring model

/// Elden Ring's physical defence curve (attack `a`, defence `d`) and the target's absorption.
fn er_hp(a: f32, d: f32, absorption: f32) -> f32 {
    if a <= 0.0 {
        return 0.0;
    }
    let r = a / d;
    let m = if r < 0.125 {
        0.1
    } else if r < 1.0 {
        0.1 + 19.2 / 49.0 * (r - 0.125).powi(2)
    } else if r < 2.5 {
        0.7 - 0.4 / 3.0 * (r - 2.5).powi(2)
    } else if r < 8.0 {
        0.9 - 0.8 / 121.0 * (r - 8.0).powi(2)
    } else {
        0.9
    };
    (a * m * (1.0 - absorption)).max(1.0)
}

/// One HP the game takes for one KH3 hit (attack value = KH3 x 7, at least 1).
fn er_hit(kh: f32, e: &Enemy) -> f32 {
    er_hp(((kh * OUT_SCALE).round()).max(1.0), e.defense, e.absorption)
}

/// A reference enemy (MODEL numbers: Elden Ring's NpcParam values as the wikis give them for Limgrave; the Godrick
/// soldier's HP is the in-game measurement of the sora-er-host row).
#[derive(Clone, Copy, Debug)]
pub struct Enemy {
    pub name: &'static str,
    pub hp: f32,
    pub defense: f32,
    pub absorption: f32,
}

pub const ENEMIES: [Enemy; 4] = [
    Enemy { name: "Soldier of Godrick (tutorial boss)", hp: 384.0, defense: 100.0, absorption: 0.0 },
    Enemy { name: "Godrick Soldier (c4311)", hp: 219.0, defense: 100.0, absorption: 0.0 },
    Enemy { name: "Godrick Knight (Limgrave)", hp: 700.0, defense: 103.0, absorption: 0.10 },
    Enemy { name: "Tree Sentinel (field boss)", hp: 2825.0, defense: 107.0, absorption: 0.10 },
];

/// Hits to kill with the combo repeated (each entry one hit's KH3 damage), and the combos that takes (fractional).
fn to_kill(combo: &[f32], e: &Enemy) -> (u32, f32) {
    let per_combo: f32 = combo.iter().map(|k| er_hit(*k, e)).sum();
    let mut hp = e.hp;
    let mut hits = 0;
    'outer: loop {
        for k in combo {
            hp -= er_hit(*k, e);
            hits += 1;
            if hp <= 0.0 {
                break 'outer;
            }
        }
        if hits > 100_000 {
            break;
        }
    }
    (hits, e.hp / per_combo.max(1e-3))
}

#[test]
fn the_model_matches_the_in_game_measurements() {
    // sora-er-host damage.out.scale evidence: Godrick soldier c4311 (219 HP): 53 -> 9, 81 -> 23, 132 -> 69
    let soldier = ENEMIES[1];
    for (attack, taken) in [(53.0, 9.0), (81.0, 23.0), (132.0, 69.0)] {
        let model = er_hp(attack, soldier.defense, soldier.absorption);
        assert!((model - taken).abs() <= 2.5, "attack {attack}: model {model:.1}, game {taken}");
    }
}

// ------------------------------------------------------------------------------------------------- Sora

/// Sora's base ground combo rows against one near dummy (kh-core's own run; tests/combat.rs section 15 row 1).
const SORA_ROWS: [&str; 4] = ["KB_VERTICAL", "KB_STAB", "KB_TURN", "KB_BREAK"];

fn sora_row_power(name: &str) -> f32 {
    kh_core::combat::tables::attack_row(name).map(|(_, r)| r.power).unwrap_or_else(|| panic!("no Sora row {name}"))
}

/// Sora's Strength / Magic / Defense at `level`: the sora-levels row + the Kingdom Key (WEP_KEYBLADE_SO_00 4 / 3 / 0).
fn sora_stats(level: u32) -> (i32, i32, i32) {
    let path = format!("{}/../../sheets/port/sora-levels.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let row = json["rows"].as_array().unwrap().iter().find(|r| r[0].as_str() == Some(&level.to_string())).unwrap();
    (row[4].as_i64().unwrap() as i32 + 4, row[5].as_i64().unwrap() as i32 + 3, row[6].as_i64().unwrap() as i32)
}

fn sora_hit(level: u32, power: f32, magic: bool) -> f32 {
    let (s, m, _) = sora_stats(level);
    let attacker = Attacker { has_status: true, strength: s, magic: m, abilities: None, side: Side::Player, state_boost: false };
    let attack = if magic { DamageAttack::magic(power, kh_core::enums::ETresDamageAttribute::Fire) } else { DamageAttack::physical(power) };
    compute_damage(&attacker, &Defender::NONE, &attack, &DamageContext::new(GameLevel::Standard, 1)).amount
}

fn sora_combo(level: u32) -> Vec<f32> {
    SORA_ROWS.iter().map(|r| sora_hit(level, sora_row_power(r), false)).collect()
}

/// Kh-core's Sora (level-99 loadout, no abilities) mashing attack at one dummy: each hit (row, damage) and the frames
/// from the first press to the 4th hit.
fn sora_run() -> (Vec<(String, f32)>, usize) {
    sora_run_with(&[], 4)
}

/// Sora with `equipped` abilities (e.g. COMBO_PLUS x2): his combo's hits and the frames to its `n`th hit.
fn sora_run_with(equipped: &[&str], n: usize) -> (Vec<(String, f32)>, usize) {
    use kh_core::{Input, Pose, RecordingHost, Sora, StateId, SORA_PARAMS};
    // the level-99 loadout with only `equipped`: the base 4-step combo (+1 per Combo Plus) against one dummy at 200 cm
    let status = level99_sora_with(equipped);
    let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
    let mut host = RecordingHost::new();
    host.targets.push(TargetInfo { id: TargetId(1), position: Vec3::new(200.0, 0.0, 0.0), center: Vec3::new(200.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 });
    let world = TestWorld::floor();
    for _ in 0..5 {
        host.step(&world, |h| sora.step_hosted(DT, &Input::default(), h));
    }
    let attack = PadButtons::NONE.with(PadButton::ATTACK);
    let (mut frames, mut at_fourth, mut seen) = (0, 0, 0);
    for i in 0..400 {
        let input = Input::default().with_pad(PadState { held: attack, pressed: attack, released: PadButtons::NONE });
        let out = host.step(&world, |h| sora.step_hosted(DT, &input, h));
        let release = Input::default();
        host.step(&world, |h| sora.step_hosted(DT, &release, h));
        frames += 2;
        // the frame of the string's last hit (its `n`th, or the last one of a shorter ability string)
        if host.hits().len() > seen && seen < n {
            seen = host.hits().len();
            at_fourth = frames;
        }
        if i > 2 && out.state == StateId::Idle {
            break;
        }
    }
    (host.hits().iter().map(|(r, _)| (r.attack_name.to_string(), r.damage)).collect(), at_fourth)
}

#[test]
fn sora_runs_the_combo_the_simulator_uses() {
    let (hits, _) = sora_run();
    let names: Vec<&str> = hits.iter().map(|h| h.0.as_str()).take(4).collect();
    assert_eq!(names, SORA_ROWS, "{hits:?}");
    // the dummy answers no defence (RecordingHost: no status object): the same numbers as the formula at level 99
    let formula = sora_combo(99);
    for (i, (name, dmg)) in hits.iter().take(4).enumerate() {
        assert_eq!(*dmg, formula[i], "{name}: run {dmg} vs formula {}", formula[i]);
    }
}

fn level99_sora() -> kh_core::Status {
    level99_sora_with(&[])
}

fn level99_sora_with(equipped: &[&str]) -> kh_core::Status {
    use kh_core::status::LoadoutValue;
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = kh_core::Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        // no abilities equipped: the base 4-step combo (tests/combat.rs section 15 row 1)
        if row[0].as_str() == Some("abilities.equipped") {
            loadout.set("abilities.equipped", &LoadoutValue::List(equipped.iter().map(|s| s.to_string()).collect())).unwrap();
            continue;
        }
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

// ------------------------------------------------------------------------------------------- the friend leaders

/// (DON-KIT) the dummy's place: Goofy's at 200 cm; Donald's missiles at 12 m (his range, doc 3.1: under 2 m A is the
/// Wand Bonk, under 6 m the missiles deal x0.6).
struct Dummy(Vec<HitRecord>, Vec3);

impl Dummy {
    fn at() -> Vec3 {
        Vec3::new(200.0, 0.0, 0.0)
    }
}

impl Targets for Dummy {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.push(self.info(TargetId(1)).unwrap());
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        (id == TargetId(1)).then(|| {
            let p = self.1; // (DON-KIT)
            TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
        })
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.position + Vec3::Z * 30.0, b: i.position + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.0.push(*hit);
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
    /// Elden Ring's targets: no KH3 status object (`Defender::NONE`), as kh-er's ErTargets answers.
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        None
    }
}

/// The friend at `level` (the real progression: KH3's init data at the shared EXP of that level), leading. `balanced`
/// false: the pad still drives him but his status is KH3's (the rule before stream BAL: `set_leading(false)`). MP one
/// short of full (a full bar turns Donald's finisher into Meteor Drop: not the base combo). Sora's reference: his base
/// (Standard 90 HP / 100 MP), as the host would feed it at the start of a game.
fn leader_party(chara: Chara, level: u32, balanced: bool) -> (Party, kh_party::MemberId) {
    let exp = kh_party::tables::level_row(chara, level).0;
    let mut party = Party::new(PartyConfig { level99: false, total_exp: exp, seed: 0x5EED });
    let id = party.spawn(chara, Vec3::ZERO, 0.0);
    party.set_pad(id, Some(Tuning::USER));
    party.set_lead_reference(Some(kh_party::balance::sora_base(GameLevel::Standard)));
    let s = party.status_mut(id).unwrap();
    if !balanced {
        s.set_leading(false);
    }
    s.mp = s.max_mp() - 1;
    // the BASE kit: none of Goofy's port abilities worn (the level-learned Combo Plus / Quake ... are switched on by the
    // string variants of round 2, so both sides compare like with like)
    let _ = s.kit_goofy_sync_level(); // owned by level first, so the party step does not wear them again
    s.goofy_kit.equipped = 0;
    assert_eq!(s.level, level);
    (party, id)
}

/// One full base ground combo of the friend leader (A mashed until the combo counter has run up and dropped back),
/// each hit's (row, KH3 damage).
fn friend_combo(chara: Chara, level: u32, balanced: bool) -> Vec<(&'static str, f32)> {
    friend_combo_timed(chara, level, balanced).0
}

/// `friend_combo` and the frames from the first press to the combo's last hit.
fn friend_combo_timed(chara: Chara, level: u32, balanced: bool) -> (Vec<(&'static str, f32)>, usize) {
    let (hits, frames) = friend_hits_timed(chara, level, balanced);
    (hits.iter().map(|h| (h.attack_name, h.damage)).collect(), frames)
}

/// The combo's hit records (power, magic flag ...) and the frames to its last hit.
fn friend_hits_timed(chara: Chara, level: u32, balanced: bool) -> (Vec<HitRecord>, usize) {
    let (mut party, id) = leader_party(chara, level, balanced);
    let world = TestWorld::floor();
    let mut dummy = Dummy(Vec::new(), if chara == Chara::Donald { Vec3::new(1200.0, 0.0, 0.0) } else { Dummy::at() }); // (DON-KIT)
    let mut audio = kh_core::host::audio::NullAudio::default();
    let mut effects = kh_core::host::effects::NullEffects::default();
    let sora = SoraView { position: Vec3::new(-3000.0, 0.0, 0.0), ..SoraView::default() };
    let mut held = PadButtons::NONE;
    let mut step = |party: &mut Party, dummy: &mut Dummy, press: bool| {
        let now = if press { PadButtons::NONE.with(PadButton::FaceBottom) } else { PadButtons::NONE };
        let pad = PadState { held: now, pressed: PadButtons(now.0 & !held.0), released: PadButtons(held.0 & !now.0) };
        held = now;
        if let Some(p) = party.member_mut(id).and_then(|m| m.pad.as_mut()).filter(|p| p.finisher_pending) {
            p.finisher_pending = false;
            p.finisher_answer = Some(false);
        }
        party.set_pad_frame(id, PadFrame { stick: Vec2::ZERO, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &world, targets: dummy, audio: &mut audio, effects: &mut effects };
        party.step(DT, &sora, &mut host);
    };
    for _ in 0..10 {
        step(&mut party, &mut dummy, false);
    }
    let combo = |party: &Party| party.member(id).and_then(|m| m.pad.as_ref()).map_or(0, |p| p.combo);
    let mut top = 0;
    let mut last_hit = 0;
    let mut frame = 0;
    let mut hits_seen = 0;
    let mut hit_frames: Vec<usize> = Vec::new();
    for f in 0..1200 {
        step(&mut party, &mut dummy, f % 4 == 0);
        frame += 1;
        while hit_frames.len() < dummy.0.len() {
            hit_frames.push(frame);
        }
        hits_seen = dummy.0.len();
        let c = combo(&party);
        top = top.max(c);
        // the combo's end: its finisher landed (stream GK's string has its own step counter), or the pad's counter ran
        // up and dropped back
        if dummy.0.iter().any(|h| is_finisher(h.attack_name)) || (top >= 3 && c == 0) {
            break;
        }
    }
    let _ = (hits_seen, last_hit);
    for _ in 0..180 {
        step(&mut party, &mut dummy, false);
        frame += 1;
        while hit_frames.len() < dummy.0.len() {
            hit_frames.push(frame);
        }
    }
    // ONE combo: everything up to the finisher's first hit, then only the finisher's own hits (a press buffered during
    // the finisher may have started the next combo's first step)
    let first_fin = dummy.0.iter().position(|h| is_finisher(h.attack_name));
    let keep: Vec<usize> = (0..dummy.0.len()).filter(|&i| first_fin.map_or(true, |k| i <= k || is_finisher(dummy.0[i].attack_name))).collect();
    let last = keep.last().map_or(0, |&i| hit_frames[i]);
    (keep.iter().map(|&i| dummy.0[i]).collect(), last)
}

/// A finisher's row: Goofy's Big Bash / Shield Quake / the Fusions (stream GK's and the pre-GK combo's), Donald's _FIN
/// Fire bolt and his team finishers.
fn is_finisher(row: &str) -> bool {
    matches!(row, "FR_SHIELDGUARDIAN_SHIELDBASH2" | "FR_SHIELDGUARDIAN_HEAVYBASH" | "FUSION_SPIN" | "FUSION_ROCKET" | "UNISON_FIRE" | "UNISON_BLIZZARD" | "UNISON_THUNDER") || row.ends_with("_FIN") || row.starts_with("FR_CALLMETEOR") || row.starts_with("FR_FLAREFORCE")
        || row.starts_with("DON_SLAM") // (DON-KIT) Donald's slam (his missile combo's finisher)
}

// --------------------------------------------------------------------------------------------- the table

fn stats_line(chara: Chara, level: u32, balanced: bool) -> String {
    let (party, id) = leader_party(chara, level, balanced);
    let s = &party.member(id).unwrap().status;
    // an Elden Ring hit that takes 10 % of the Tarnished's max: how many knock him out (Sora: 10)
    let ko = kh_party::balance::hits_to_ko(s, kh_party::balance::sora_base(GameLevel::Standard).0, 0.1);
    format!("Str {:3} Mag {:3} Def {:3} HP {:4} MP {:4} | 10 %-hits to KO {ko:5.2}", s.strength(), s.magic(), s.defense(), s.max_hp(), s.max_mp())
}

#[test]
fn hits_to_kill_table() {
    let mut out = String::new();
    out.push_str("BALANCE SIMULATOR (GOOFY-BAL): base ground combo against one target, Standard, Elden Ring model (attack = KH3 x 7, defence curve)\n");
    out.push_str("KH3 = the friend leader's status before stream BAL (KH3's friend tables); BAL = the balanced leader. 'Nx' = combos needed / Sora's.\n");
    let mut ratios: Vec<(u32, &str, f32, f32)> = Vec::new();
    let (_, sora_frames) = sora_run();
    out.push_str(&format!("Sora's base combo: 4th hit {sora_frames} frames after the first press ({:.2} s)\n", sora_frames as f32 * DT));
    for level in LEVELS {
        let (s, m, d) = sora_stats(level);
        let sora = sora_combo(level);
        out.push_str(&format!("\n== level {level}\n"));
        out.push_str(&format!("  Sora       Str {s:3} Mag {m:3} Def {d:3} | 10 %-hits to KO 10.00   combo {:?}\n", SORA_ROWS.iter().zip(&sora).map(|(n, k)| format!("{n} {k}")).collect::<Vec<_>>()));
        let fire = kh_party::pad::donald_shots::SoloRow::sora("MAG_FIRE").map(|r| r.power).unwrap_or(1.0);
        let soldier = ENEMIES[1];
        let sfire = sora_hit(level, fire, true);
        for (chara, name) in [(Chara::Donald, "Donald"), (Chara::Goofy, "Goofy")] {
            for balanced in [false, true] {
                let tag = if balanced { "BAL" } else { "KH3" };
                let (combo, frames) = friend_combo_timed(chara, level, balanced);
                out.push_str(&format!("  {name:6} {tag} {}   combo ({:.2} s to its last hit) {:?}\n", stats_line(chara, level, balanced), frames as f32 * DT, combo.iter().map(|(n, k)| format!("{n} {k}")).collect::<Vec<_>>()));
                let ks: Vec<f32> = combo.iter().map(|h| h.1).collect();
                let mut line = String::new();
                let mut sum = 0.0;
                let mut time = 0.0;
                for e in ENEMIES {
                    let (sh, sc) = to_kill(&sora, &e);
                    let (fh, fc) = to_kill(&ks, &e);
                    sum += fc / sc;
                    time += fc * frames as f32 / (sc * sora_frames as f32);
                    line.push_str(&format!("{}: {fh} hits / {fc:.2} combos ({:.2}x; Sora {sh} / {sc:.2}) | ", e.name.split(' ').next().unwrap_or(""), fc / sc));
                }
                out.push_str(&format!("      {line}mean {:.2}x the combos, {:.2}x the time\n", sum / ENEMIES.len() as f32, time / ENEMIES.len() as f32));
                if balanced {
                    ratios.push((level, name, sum / ENEMIES.len() as f32, 0.0));
                }
                if chara == Chara::Donald {
                    let (party, idd) = leader_party(chara, level, balanced);
                    let dm = party.member(idd).unwrap().status.magic();
                    let dfire = compute_damage(&Attacker { has_status: true, strength: 0, magic: dm, abilities: None, side: Side::Player, state_boost: false }, &Defender::NONE, &DamageAttack::magic(fire, kh_core::enums::ETresDamageAttribute::Fire), &DamageContext::new(GameLevel::Standard, 1)).amount;
                    out.push_str(&format!("      Fire (MAG_FIRE {fire}): Sora {sfire} KH -> {:.0} ER HP on the soldier; Donald {dfire} KH -> {:.0} ER HP ({:.2}x)\n", er_hit(sfire, &soldier), er_hit(dfire, &soldier), er_hit(dfire, &soldier) / er_hit(sfire, &soldier)));
                    if balanced {
                        ratios.last_mut().unwrap().3 = er_hit(dfire, &soldier) / er_hit(sfire, &soldier);
                    }
                }
            }
        }
    }
    eprintln!("{out}");
    let dir = format!("{}/../../build/sora_tmp/party/bal", env!("CARGO_MANIFEST_DIR"));
    let _ = std::fs::create_dir_all(&dir);
    let name = std::env::var("BAL_OUT").unwrap_or_else(|_| "last.txt".to_string());
    let _ = std::fs::write(format!("{dir}/{name}"), &out);
    // THE TARGETS (BAL_DESIGN.md). Melee per physical hit: Sora > Goofy > Donald at every level (Elden Ring HP of the mean
    // physical hit). Goofy's base combo with stream GK's Sora-scale moves: 1.1 - 1.45x Sora's combos (the user: "a bit MORE
    // than Sora's two combos"); with the pre-GK KH3 moves (power 1.0 rows, the old 1.8 finisher) the table only reports.
    // (DON-KIT) Donald's base combo (his missiles at 12 m, doc 3.1): 1.2 - 3.0x (weaker without the menu, doc 12); his Fire above
    // Sora's.
    for (level, name, combos, fire) in &ratios {
        match *name {
            "Goofy" => {
                if goofy_is_sora_scale() {
                    // (round 2, the coordinator 2026-10-17: time to kill 1.15 - 1.25x, combos above 1.2x)
                    assert!((1.2..=1.7).contains(combos), "level {level}: Goofy needs {combos:.2}x Sora's combos");
                }
            }
            _ => {
                // (DON-KIT) doc 12: the menu-less Donald (his plain missile combo at 12 m) is weaker than Sora: more combos
                assert!((1.2..=3.0).contains(combos), "level {level}: Donald needs {combos:.2}x Sora's combos");
                assert!(*fire > 1.1, "level {level}: Donald's Fire {fire:.2}x Sora's");
            }
        }
    }
}

/// Goofy's real combo is stream GK's (his hits on Sora's scale: a base combo of at most 3.5 power).
fn goofy_is_sora_scale() -> bool {
    let (hits, _) = friend_hits_timed(Chara::Goofy, 30, true);
    hits.iter().map(|h| h.info.power).sum::<f32>() <= 3.5
}

/// Mean Elden Ring HP of a physical (non-magic) hit on the soldier.
fn physical_mean(hits: &[HitRecord]) -> f32 {
    let e = ENEMIES[1];
    let phys: Vec<f32> = hits.iter().filter(|h| h.info.flags50 & kh_core::host::HitInfo::FLAG50_MAGIC == 0 && h.attribute == kh_core::enums::ETresDamageAttribute::Physical).map(|h| er_hit(h.damage, &e)).collect();
    phys.iter().sum::<f32>() / phys.len().max(1) as f32
}

#[test]
fn melee_order_sora_goofy_donald_at_every_level() {
    for level in LEVELS {
        let sora: f32 = sora_combo(level).iter().map(|k| er_hit(*k, &ENEMIES[1])).sum::<f32>() / 4.0;
        let goofy = physical_mean(&friend_hits_timed(Chara::Goofy, level, true).0);
        let donald = physical_mean(&friend_hits_timed(Chara::Donald, level, true).0);
        eprintln!("level {level}: mean physical hit on the soldier: Sora {sora:.1} Goofy {goofy:.1} Donald {donald:.1} ER HP");
        assert!(sora > donald && goofy > donald, "level {level}: Sora {sora:.1} Goofy {goofy:.1} Donald {donald:.1}");
        if goofy_is_sora_scale() {
            assert!(sora > goofy * 0.9, "level {level}: Goofy's hits are heavier but fewer: Sora {sora:.1} vs Goofy {goofy:.1}");
        }
    }
}

/// Goofy's base string as stream GK's sheet gives it (party-kit-goofy step.push / step.sweep / step.big power), with the
/// balanced Strength: the cross-check while GK's code is not in the build.
#[test]
fn goofy_gk_sheet_string_needs_a_bit_more_than_soras_combos() {
    let path = format!("{}/../../sheets/port/party-kit-goofy.json", env!("CARGO_MANIFEST_DIR"));
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let get = |id: &str| json["rows"].as_array().unwrap().iter().find(|r| r[0].as_str() == Some(id)).and_then(|r| r[1].as_f64()).map(|v| v as f32);
    let (Some(push), Some(sweep), Some(big)) = (get("step.push.power"), get("step.sweep.power"), get("step.big.power")) else {
        eprintln!("party-kit-goofy has no step.* rows yet (stream GK): skipped");
        return;
    };
    let mut lines = String::new();
    for level in LEVELS {
        let (party, id) = leader_party(Chara::Goofy, level, true);
        let st = party.member(id).unwrap().status.strength();
        let hit = |p: f32| compute_damage(&Attacker { has_status: true, strength: st, magic: 0, abilities: None, side: Side::Player, state_boost: false }, &Defender::NONE, &DamageAttack::physical(p), &DamageContext::new(GameLevel::Standard, 1)).amount;
        let combo = [hit(push), hit(sweep), hit(big)];
        let sora = sora_combo(level);
        let mean: f32 = ENEMIES.iter().map(|e| to_kill(&combo, e).1 / to_kill(&sora, e).1).sum::<f32>() / ENEMIES.len() as f32;
        lines.push_str(&format!("  level {level}: Goofy Str {st} string {combo:?} -> {mean:.2}x Sora's combos\n"));
        assert!((1.2..=1.7).contains(&mean), "level {level}: {mean:.2}x"); // (round 2: lower Strength, time 1.15 - 1.25x)
    }
    eprintln!("GOOFY (GK sheet MVs {push} / {sweep} / {big}, BAL Strength):\n{lines}");
}

#[test]
fn no_goofy_hit_is_a_magic_hit() {
    // Goofy has no magic (Magic 0 while he leads): none of his combo rows may use the Magic stat
    for level in [1, 50] {
        for (row, dmg) in friend_combo(Chara::Goofy, level, true) {
            assert!(dmg > 1.0, "level {level}: {row} {dmg}: a magic row would do ~1 with Magic 0");
        }
        let (party, id) = leader_party(Chara::Goofy, level, true);
        assert_eq!(party.member(id).unwrap().status.magic(), 0);
    }
}

// ============================================================================ (GOOFY-BAL round 2) GK's real strings
// Goofy's final kit (stream GK, IMPL_GK_STATE.md): the base string (Push, Sweep, Big Bash), Combo Plus x2 (+ Shield Bash,
// Shove), the finisher variants Shield Quake (A held) and Shield Upper (stick away), Bombardier (Y), and the guard bashes
// with Brace (blocks stored, +25 % each) and Counter Bash (A after a Perfect Guard, x1.5). Each runs through the real
// pad against the same dummy; Sora's matching strings run through kh-core's Sora.

use kh_party::pad::goofy_kit::GoofyAbility as GA;

/// How a case is played: per frame, the stick, the buttons held, and whether the dummy hits him now (from the front).
type CaseInput<'a> = dyn FnMut(usize, &[HitRecord]) -> (Vec2, Vec<PadButton>, bool) + 'a;

/// Goofy leading at `level` with `kit` worn, played by `input` for `frames` frames: each hit with its frame.
fn goofy_case(level: u32, kit: &[GA], frames: usize, input: &mut CaseInput) -> Vec<(HitRecord, usize)> {
    let (mut party, id) = leader_party(Chara::Goofy, level, true);
    {
        let s = party.status_mut(id).unwrap();
        for k in kit {
            s.goofy_kit.give(*k);
            s.goofy_kit.set_equipped(*k, true);
        }
    }
    let world = TestWorld::floor();
    let mut dummy = Dummy(Vec::new(), Dummy::at()); // (DON-KIT)
    let mut audio = kh_core::host::audio::NullAudio::default();
    let mut effects = kh_core::host::effects::NullEffects::default();
    let sora = SoraView { position: Vec3::new(-3000.0, 0.0, 0.0), ..SoraView::default() };
    let mut held = PadButtons::NONE;
    let mut frames_of: Vec<usize> = Vec::new();
    for _ in 0..10 {
        let pad = PadState { held: PadButtons::NONE, pressed: PadButtons::NONE, released: PadButtons::NONE };
        party.set_pad_frame(id, PadFrame { stick: Vec2::ZERO, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &world, targets: &mut dummy, audio: &mut audio, effects: &mut effects };
        party.step(DT, &sora, &mut host);
    }
    for f in 0..frames {
        let (stick, buttons, hit_me) = input(f, &dummy.0);
        let mut now = PadButtons::NONE;
        for b in &buttons {
            now.set(*b, true);
        }
        let pad = PadState { held: now, pressed: PadButtons(now.0 & !held.0), released: PadButtons(held.0 & !now.0) };
        held = now;
        if let Some(p) = party.member_mut(id).and_then(|m| m.pad.as_mut()).filter(|p| p.finisher_pending) {
            p.finisher_pending = false;
            p.finisher_answer = Some(false);
        }
        party.set_pad_frame(id, PadFrame { stick, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &world, targets: &mut dummy, audio: &mut audio, effects: &mut effects };
        party.step(DT, &sora, &mut host);
        if hit_me {
            let me = party.member(id).unwrap().position();
            let from = Dummy::at();
            let mut h = HitRecord::empty(TargetId(1));
            h.damage = 5.0;
            h.damage_kind = kh_core::enums::ETresDamageKind::Small;
            h.direction = (me - from).normalize_or_zero();
            h.attacker = kh_core::host::HitAttacker { position: from, yaw: 180.0, radius: 50.0, valid: true };
            h.position = me + Vec3::Z * 90.0;
            let _ = party.receive_hit(id, &h);
        }
        while frames_of.len() < dummy.0.len() {
            frames_of.push(f + 1);
        }
    }
    dummy.0.iter().copied().zip(frames_of).collect()
}

/// A finisher of the string variants (Shield Upper borrows Attack3_JumpAscent's shape).
fn is_string_finisher(row: &str) -> bool {
    is_finisher(row) || row == "Attack3_JumpAscent"
}

/// One ground string: A mashed (a press every 4 frames) until its finisher's first hit; `regular` steps before the
/// finisher; `finisher_hold`: A held from the finisher's press (Shield Quake); `finisher_stick`: the stick on the finisher
/// press (Shield Upper: away from the target). Returns the string's hits (up to the finisher's own) and the frame of the
/// last kept hit.
fn goofy_string(level: u32, kit: &[GA], regular: usize, finisher_hold: bool, finisher_stick: Vec2) -> (Vec<HitRecord>, usize) {
    let mut holding = false;
    let mut input = move |f: usize, hits: &[HitRecord]| -> (Vec2, Vec<PadButton>, bool) {
        let fin_seen = hits.iter().any(|h| is_string_finisher(h.attack_name));
        let reg = hits.iter().filter(|h| !is_string_finisher(h.attack_name)).count();
        if fin_seen {
            return (Vec2::ZERO, if holding { vec![PadButton::FaceBottom] } else { vec![] }, false);
        }
        if reg >= regular && (finisher_hold || finisher_stick != Vec2::ZERO) {
            // the finisher press: a fresh press, then held (Quake) / with the stick (Upper)
            let press = holding || f % 4 == 0;
            if press {
                holding = finisher_hold;
            }
            return (finisher_stick, if press { vec![PadButton::FaceBottom] } else { vec![] }, false);
        }
        (Vec2::ZERO, if f % 4 == 0 { vec![PadButton::FaceBottom] } else { vec![] }, false)
    };
    let all = goofy_case(level, kit, 600, &mut input);
    let first_fin = all.iter().position(|(h, _)| is_string_finisher(h.attack_name));
    let keep: Vec<&(HitRecord, usize)> = all.iter().enumerate().filter(|(i, (h, _))| first_fin.map_or(true, |k| *i <= k || is_string_finisher(h.attack_name))).map(|(_, x)| x).collect();
    (keep.iter().map(|(h, _)| *h).collect(), keep.last().map_or(0, |(_, f)| *f))
}

/// Sora's string with `n_plus` Combo Plus: rows from kh-core's run, damage at `level` by the formula, frames to its end.
fn sora_string(level: u32, n_plus: usize) -> (Vec<f32>, usize) {
    let eq: Vec<&str> = (0..n_plus).map(|_| "COMBO_PLUS").collect();
    let (hits, frames) = sora_run_with(&eq, 4 + n_plus);
    let rows: Vec<String> = hits.iter().take(4 + n_plus).map(|h| h.0.clone()).collect();
    (rows.iter().map(|r| sora_hit(level, sora_row_power(r), false)).collect(), frames)
}

/// (combos ratio, time ratio) of a Goofy string against a Sora string over the reference enemies.
fn ratios_of(goofy: &[f32], g_frames: usize, sora: &[f32], s_frames: usize) -> (f32, f32) {
    let mut c = 0.0;
    for e in ENEMIES {
        c += to_kill(goofy, &e).1 / to_kill(sora, &e).1;
    }
    c /= ENEMIES.len() as f32;
    (c, c * g_frames as f32 / s_frames.max(1) as f32)
}

#[test]
fn goofy_round2_strings_and_specials() {
    let mut out = String::from("ROUND 2 (GOOFY-BAL): GK's final kit through the real pad; ratios = Goofy / Sora over the 4 reference enemies\n");
    let mut fails = Vec::new();
    let soldier = ENEMIES[1];
    for level in [1, 5, 10, 20, 30, 40, 60, 80, 99] {
        let (sb, sbf) = sora_string(level, 0);
        let (sp, spf) = sora_string(level, 2);
        let (lp, lid) = leader_party(Chara::Goofy, level, true);
        out.push_str(&format!("\n== level {level} (Goofy Str {}, Sora Str {})\n", lp.status(lid).unwrap().strength(), sora_stats(level).0));
        let (s1, s1f) = sora_string(level, 1);
        // Sora's ability finishers (his own reward for an ability slot, as Quake / Upper are Goofy's): Magic Flash, Final Heaven
        let sora_ability = |eq: &str| {
            let (hits, frames) = sora_run_with(&[eq], 5);
            // the whole first string (Final Heaven has two rows: KB_FINALHEAVEN + its _END)
            let rows: Vec<String> = hits.iter().map(|h| h.0.clone()).collect();
            (rows.iter().map(|r| sora_hit(level, sora_row_power(r), false)).collect::<Vec<f32>>(), frames, rows)
        };
        let (sf_flash, sf_flash_f, flash_rows) = sora_ability("MAGICFLUSH");
        let (sf_fh, sf_fh_f, fh_rows) = sora_ability("FINAL_HEAVEN");
        out.push_str(&format!("  Sora Magic Flash string {flash_rows:?} {sf_flash:?} in {:.2} s; Final Heaven string {fh_rows:?} {sf_fh:?} in {:.2} s
", sf_flash_f as f32 * DT, sf_fh_f as f32 * DT));
        let cases: [(&str, Vec<GA>, usize, bool, Vec2, &[f32], usize); 5] = [
            ("base string", vec![], 2, false, Vec2::ZERO, &sb, sbf),
            ("Combo Plus x1", vec![GA::ComboPlus], 3, false, Vec2::ZERO, &s1, s1f),
            ("Combo Plus x2", vec![GA::ComboPlus, GA::ComboPlus2], 4, false, Vec2::ZERO, &sp, spf),
            ("Shield Quake finisher", vec![GA::ShieldQuake], 2, true, Vec2::ZERO, &sb, sbf),
            ("Shield Upper finisher", vec![GA::ShieldUpper], 2, false, Vec2::new(0.0, -1.0), &sb, sbf),
        ];
        for (name, kit, regular, hold, stick, sora, sf) in cases {
            let (hits, gf) = goofy_string(level, &kit, regular, hold, stick);
            let ks: Vec<f32> = hits.iter().map(|h| h.damage).collect();
            let (c, t) = ratios_of(&ks, gf, sora, sf);
            out.push_str(&format!("  {name:22} {:?} in {:.2} s (Sora {:.2} s {:?}): {c:.2}x the combos, {t:.2}x the time\n", hits.iter().map(|h| format!("{} {}", h.attack_name, h.damage)).collect::<Vec<_>>(), gf as f32 * DT, sf as f32 * DT, sora));
            if name.starts_with("Shield") {
                for (sname, sv, sfr) in [("Magic Flash", &sf_flash, sf_flash_f), ("Final Heaven", &sf_fh, sf_fh_f)] {
                    let (c2, t2) = ratios_of(&ks, gf, sv, sfr);
                    out.push_str(&format!("      vs Sora's {sname} string: {c2:.2}x the combos, {t2:.2}x the time
"));
                }
            }
            if name.starts_with("base") || name.starts_with("Combo Plus") {
                if !(c > 1.2) || !(1.1..=1.4).contains(&t) {
                    fails.push(format!("level {level} {name}: {c:.2}x combos, {t:.2}x time"));
                }
            }
        }
        // Bombardier (Y; 8 s cooldown): one use
        let mut bomb = |f: usize, _: &[HitRecord]| -> (Vec2, Vec<PadButton>, bool) { (Vec2::ZERO, if f < 2 { vec![PadButton::FaceTop] } else { vec![] }, false) };
        let b = goofy_case(level, &[], 240, &mut bomb);
        let bhp: f32 = b.iter().map(|(h, _)| er_hit(h.damage, &soldier)).sum();
        // the guard bashes: Brace x3 (three blocked hits, then A), Counter Bash (a Perfect Guard, then A), both together
        let mut brace = |f: usize, _: &[HitRecord]| -> (Vec2, Vec<PadButton>, bool) {
            let mut b = vec![PadButton::FaceLeft];
            if f == 80 || f == 81 {
                b.push(PadButton::FaceBottom);
            }
            (Vec2::ZERO, b, matches!(f, 30 | 45 | 60))
        };
        let br = goofy_case(level, &[], 200, &mut brace);
        let mut counter = |f: usize, _: &[HitRecord]| -> (Vec2, Vec<PadButton>, bool) {
            let mut b = vec![PadButton::FaceLeft];
            if f == 8 || f == 9 {
                b.push(PadButton::FaceBottom);
            }
            (Vec2::ZERO, b, f == 3)
        };
        let co = goofy_case(level, &[], 200, &mut counter);
        let mut both = |f: usize, _: &[HitRecord]| -> (Vec2, Vec<PadButton>, bool) {
            // two blocks, shield down, up again, a Perfect Guard (the 3rd stored block), then A
            let up = !(46..52).contains(&f);
            let mut b = if up { vec![PadButton::FaceLeft] } else { vec![] };
            if f == 58 || f == 59 {
                b.push(PadButton::FaceBottom);
            }
            (Vec2::ZERO, b, matches!(f, 30 | 40 | 54))
        };
        let bo = goofy_case(level, &[], 200, &mut both);
        let one = |v: &[(HitRecord, usize)]| v.first().map(|(h, _)| (h.attack_name, h.damage, h.info.power)).unwrap_or(("-", 0.0, 0.0));
        let sora_revenge = sora_hit(level, sora_row_power("REVENGE_SLASH"), false);
        let sora_break = sora_hit(level, sora_row_power("KB_BREAK"), false);
        out.push_str(&format!("  Bombardier (Y, 8 s cooldown): {:?} -> {bhp:.0} ER HP on the soldier\n", b.iter().map(|(h, _)| format!("{} {}", h.attack_name, h.damage)).collect::<Vec<_>>()));
        let bp = goofy_case(level, &[GA::PaybackBash], 200, &mut brace);
        let bop = goofy_case(level, &[GA::PaybackBash], 200, &mut both);
        for (name, v) in [("Brace x3 guard bash", &br), ("Counter Bash", &co), ("Counter Bash + Brace x3", &bo), ("Brace x3 + Payback Bash", &bp), ("Counter+Brace3+Payback", &bop)] {
            let (row, dmg, power) = one(v);
            out.push_str(&format!("  {name:24} {row} power {power:.2} -> {dmg} KH, {:.0} ER HP (Sora's Revenge Slash {sora_revenge} KH {:.0}; KB_BREAK {sora_break} KH {:.0})\n", er_hit(dmg, &soldier), er_hit(sora_revenge, &soldier), er_hit(sora_break, &soldier)));
        }
    }
    eprintln!("{out}");
    let dir = format!("{}/../../build/sora_tmp/party/bal", env!("CARGO_MANIFEST_DIR"));
    let _ = std::fs::write(format!("{dir}/round2.txt"), &out);
    assert!(fails.is_empty(), "time-to-kill targets (combos > 1.2x, time 1.1 - 1.4x):\n{}", fails.join("\n"));
}
