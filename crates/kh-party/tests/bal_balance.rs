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
    // (AUDIT-B) no bones here: a rider attached to a hit follows the target's origin and yaw
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

/// The friend at `level` (the real progression: KH3's init data at the shared EXP of that level), leading. MP one short of
/// full (a full bar turns Donald's finisher into Meteor Drop: not the base combo).
fn leader_party(chara: Chara, level: u32) -> (Party, kh_party::MemberId) {
    let exp = kh_party::tables::level_row(chara, level).0;
    let mut party = Party::new(PartyConfig { level99: false, total_exp: exp, seed: 0x5EED });
    let id = party.spawn(chara, Vec3::ZERO, 0.0);
    party.set_pad(id, Some(Tuning::USER));
    let s = party.status_mut(id).unwrap();
    s.mp = s.max_mp() - 1;
    // the BASE kit: none of Goofy's port abilities worn (the level-learned Combo Plus / Quake ... are switched on by the
    // string variants of round 2, so both sides compare like with like)
    let _ = s.gof_sync_level(); // owned by level first, so the party step does not wear them again
    if let Some(g) = s.goofy.as_mut() {
        for k in kh_party::goofy::GoofyAbility::ALL {
            g.abilities.set_equipped(k, false);
        }
    }
    assert_eq!(s.level, level);
    (party, id)
}

/// One full base ground combo of the friend leader (A mashed until the combo counter has run up and dropped back),
/// each hit's (row, KH3 damage).
fn friend_combo(chara: Chara, level: u32) -> Vec<(&'static str, f32)> {
    friend_combo_timed(chara, level).0
}

/// `friend_combo` and the frames from the first press to the combo's last hit.
fn friend_combo_timed(chara: Chara, level: u32) -> (Vec<(&'static str, f32)>, usize) {
    let (hits, frames) = friend_hits_timed(chara, level);
    (hits.iter().map(|h| (h.attack_name, h.damage)).collect(), frames)
}

/// The combo's hit records (power, magic flag ...) and the frames to its last hit.
fn friend_hits_timed(chara: Chara, level: u32) -> (Vec<HitRecord>, usize) {
    let (mut party, id) = leader_party(chara, level);
    let world = TestWorld::floor();
    let mut dummy = Dummy(Vec::new(), if chara == Chara::Donald { Vec3::new(1200.0, 0.0, 0.0) } else { Dummy::at() }); // (DON-KIT)
    let mut audio = kh_core::host::audio::NullAudio;
    let mut effects = kh_core::host::effects::NullEffects;
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
    let last_hit = 0;
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
    let keep: Vec<usize> = (0..dummy.0.len()).filter(|&i| first_fin.is_none_or(|k| i <= k || is_finisher(dummy.0[i].attack_name))).collect();
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

fn stats_line(chara: Chara, level: u32) -> String {
    let (party, id) = leader_party(chara, level);
    let s = &party.member(id).unwrap().status;
    // an Elden Ring hit that takes 10 % of the Tarnished's max: how many knock him out (Sora: 10)
    let ko = kh_party::balance::hits_to_ko(s, 0.1);
    format!("Str {:3} Mag {:3} Def {:3} HP {:4} MP {:4} | 10 %-hits to KO {ko:5.2}", s.strength(), s.magic(), s.defense(), s.max_hp(), s.max_mp())
}

#[test]
fn hits_to_kill_table() {
    let mut out = String::new();
    out.push_str("BALANCE SIMULATOR (GOOFY-BAL): base ground combo against one target, Standard, Elden Ring model (attack = KH3 x 7, defence curve)\n");
    out.push_str("'Nx' = combos needed / Sora's.\n");
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
            {
                let (combo, frames) = friend_combo_timed(chara, level);
                out.push_str(&format!("  {name:6} {}   combo ({:.2} s to its last hit) {:?}\n", stats_line(chara, level), frames as f32 * DT, combo.iter().map(|(n, k)| format!("{n} {k}")).collect::<Vec<_>>()));
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
                ratios.push((level, name, sum / ENEMIES.len() as f32, 0.0));
                if chara == Chara::Donald {
                    let (party, idd) = leader_party(chara, level);
                    let dm = party.member(idd).unwrap().status.magic();
                    let dfire = compute_damage(&Attacker { has_status: true, strength: 0, magic: dm, abilities: None, side: Side::Player, state_boost: false }, &Defender::NONE, &DamageAttack::magic(fire, kh_core::enums::ETresDamageAttribute::Fire), &DamageContext::new(GameLevel::Standard, 1)).amount;
                    out.push_str(&format!("      Fire (MAG_FIRE {fire}): Sora {sfire} KH -> {:.0} ER HP on the soldier; Donald {dfire} KH -> {:.0} ER HP ({:.2}x)\n", er_hit(sfire, &soldier), er_hit(dfire, &soldier), er_hit(dfire, &soldier) / er_hit(sfire, &soldier)));
                    ratios.last_mut().unwrap().3 = er_hit(dfire, &soldier) / er_hit(sfire, &soldier);
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
                    assert!((1.2..=2.5).contains(combos), "level {level}: Goofy needs {combos:.2}x Sora's combos"); // (2026-10-30, the user: Goofy 0.7x Sora's strength)
                }
            }
            _ => {
                // (DON-KIT) doc 12: the menu-less Donald (his plain missile combo at 12 m) is weaker than Sora: more combos;
                // (DONLINE2, the user 2026-10-27: "make Donald's starting magic 18 at level 1": Magic ratio 1.2, his missiles 5 %
                // weaker, level 1 at 3.12x) up to 3.2x
                assert!((1.2..=3.2).contains(combos), "level {level}: Donald needs {combos:.2}x Sora's combos");
                assert!(*fire > 1.1, "level {level}: Donald's Fire {fire:.2}x Sora's");
            }
        }
    }
}

/// Goofy's real combo is stream GK's (his hits on Sora's scale: a base combo of at most 3.5 power).
fn goofy_is_sora_scale() -> bool {
    let (hits, _) = friend_hits_timed(Chara::Goofy, 30);
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
        let goofy = physical_mean(&friend_hits_timed(Chara::Goofy, level).0);
        let donald = physical_mean(&friend_hits_timed(Chara::Donald, level).0);
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
        let (party, id) = leader_party(Chara::Goofy, level);
        let st = party.member(id).unwrap().status.strength();
        let hit = |p: f32| compute_damage(&Attacker { has_status: true, strength: st, magic: 0, abilities: None, side: Side::Player, state_boost: false }, &Defender::NONE, &DamageAttack::physical(p), &DamageContext::new(GameLevel::Standard, 1)).amount;
        let combo = [hit(push), hit(sweep), hit(big)];
        let sora = sora_combo(level);
        let mean: f32 = ENEMIES.iter().map(|e| to_kill(&combo, e).1 / to_kill(&sora, e).1).sum::<f32>() / ENEMIES.len() as f32;
        lines.push_str(&format!("  level {level}: Goofy Str {st} string {combo:?} -> {mean:.2}x Sora's combos\n"));
        assert!((1.2..=2.5).contains(&mean), "level {level}: {mean:.2}x"); // (round 2: lower Strength, time 1.15 - 1.25x) (2026-10-30, the user: Goofy 0.7x Sora's strength)
    }
    eprintln!("GOOFY (GK sheet MVs {push} / {sweep} / {big}, BAL Strength):\n{lines}");
}

#[test]
fn no_goofy_hit_is_a_magic_hit() {
    // Goofy has no magic (Magic 0 while he leads): none of his combo rows may use the Magic stat
    for level in [1, 50] {
        for (row, dmg) in friend_combo(Chara::Goofy, level) {
            assert!(dmg > 1.0, "level {level}: {row} {dmg}: a magic row would do ~1 with Magic 0");
        }
        let (party, id) = leader_party(Chara::Goofy, level);
        assert_eq!(party.member(id).unwrap().status.magic(), 0);
    }
}

// ============================================================================ (GOOFY-BAL round 2) GK's real strings
// Goofy's final kit (stream GK, IMPL_GK_STATE.md): the base string (Push, Sweep, Big Bash), Combo Plus x2 (+ Shield Bash,
// Shove), the finisher variants Shield Quake (A held) and Shield Upper (stick away), Bombardier (Y), and the guard bashes
// with Brace (blocks stored, +25 % each) and Counter Bash (A after a Perfect Guard, x1.5). Each runs through the real
// pad against the same dummy; Sora's matching strings run through kh-core's Sora.
