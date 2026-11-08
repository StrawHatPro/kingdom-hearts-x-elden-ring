//! (HAIR-PULSE) NO FORM LEAVES SORA PULSING (build/sora_tmp/weapons/HAIR-PULSE/STATE.md). The user (2026-11-05): "Sora's hair
//! should not pulse like it does currently when dual wielding" and "some transformations are just making Sora pulse after he
//! transforms ... ensure it never happens on ANY form!".
//!
//! THE CAUSES (one shared rule each, no per-form code):
//! 1. Every Keyblade form holds KH3's form charge glow `vs_com_lv2` (group 3001 of `ead_wep_chrg0`). Its
//!    `SQEX_Material_InLight` is a SUB module: KH3 writes it as `Sub_InLight*` (0x14436FCA0 takes the name table +0x40 of
//!    0x140387190 when ParamType != 0), which only the WEAPON materials read. The port wrote `InLight*`, which every body /
//!    hair / face material reads: his whole model (the hair brightest) flashed white-gold (InLightColor 10, 0 -> 0.5 -> 0
//!    every 2 s) for the whole form. Fix: kh_vfx::screen::parse_char / `in_light_names`.
//! 2. The core spawned that glow for EVERY wear form; KH3 only for a form whose weapon carries m_CmnFormChangeChargeEffect
//!    (the row's `charge_effect`): Ultimate Form and Rage Form have none. Fix: kh-core form.rs `apply_wear_form`.
//! 3. A drive reached Sora's body from effects not hung on him (owner 0 by default): KH3 writes a SQEX_Material_* drive on the
//!    mesh the component was attached to (0x1442871A0: "AttachOwnerMeshComp"), so Blizzard Claws' transform unit w_so081's
//!    looping `vw_so082_cng_mat0` aura (a row the core places in the world) lit his body blue after the transformation. Fix:
//!    kh-host EffectHost::char_drives (and kh-app's mapping): only Sora- / party-anchored effects drive a character.
//! 4. And the glow itself is not KH3's: KH3 spawns the form's weapons with charge level -1 and never raises it (no caller of
//!    the setter; STATE.md section 6), so no form shows vs_com_lv2 / vs_com_kyp01 on his body or his weapon. Fix: kh-core
//!    `apply_wear_form` spawns neither (the golden trace re-blessed for it). The SUB-name rule, the right-hand weapon route of a
//!    SUB drive (kh-host `right_weapon`) and the weapon-frame centre (`weapon_in_light`) stay: KH3's rules for any SUB in-light.
//!
//! THIS TEST runs EVERY row of sora-keyblade-forms.json (both stages, Rage Form, Ultimate Form; the grow levels of the
//! shields) through the real core and the host's effect side (kh_host::audit_a::Session), hands each step's character drives
//! to the renderer's own `apply_char_drives` and wear-form slide (`Library::step_wear`) on the real material library of
//! EVERY outfit (KH3, KH1, KH2, Toy Box, Monstropolis, Caribbean, San Fransokyo, Verum Rex), and checks the uniform blocks
//! of his body slots (what the shaders read): from KH3's settle point (the transformation's end and the 1 s slide) for 6 s
//! held in the form, and from the end's 0.2 s slide back for 2 s more, they are EXACTLY those of the same library without any
//! drive (nothing of an effect is left on his model: flat), and the wear slide itself is over within KH3's 1 s / 0.2 s; no
//! weapon of his carries any Sub_InLight (no charge glow), and no charge effect runs in any form.

use glam::{Vec3, Vec4};
use kh_core::combat::form::{KeybladeFormRow, KEYBLADE_FORMS};
use kh_core::{PadButton, StateId};
use kh_host::audit_a::Session;
use kh_host::rig::STEP;
use kh_look::library::{Library, LookFrame, ObjectRows};

use super::{apply_char_drives, set_char_drives, CharDrive};

const OUTFITS: [&str; 8] = ["kh3", "kh1", "kh2", "toybox", "monstropolis", "caribbean", "sanfransokyo", "verumrex"];
/// KH3's wear-form slide in (the equipment component's FormChangeSlide, 1 s) and out (0.2 s): kh-look `step_wear`.
const SLIDE_IN: f32 = 1.0;
const SLIDE_OUT: f32 = 0.2;
const HOLD: usize = 360; // 6 s in the form
const AFTER: usize = 120; // 2 s after it

struct Frame {
    cmd: i32,
    wear: u8,
    drives: Vec<CharDrive>,
    /// the effects whose drives these are (their names, for the report)
    sources: Vec<String>,
    /// his right-hand weapon's mesh this frame (the form's RWeapon unit at its grow level; `kingdom_key` for w_so010)
    right: Option<String>,
    /// the effects with a SUB in-light drive this frame (KH3's own, e.g. Storm Flag's vw_so093_fns_ora0 on the right-hand weapon)
    sub_sources: Vec<String>,
    /// a form charge effect (vs_com_lv1 / lv2 / kyp01) is live
    charge: bool,
}

struct Rec {
    frames: Vec<Frame>,
}

impl Rec {
    fn tick(&mut self, s: &mut Session, held: &[PadButton]) -> StateId {
        let state = s.step(held);
        let eye = s.rig.out.position + Vec3::new(-500.0, 0.0, 200.0);
        let params: Vec<(u64, u8)> = s.fx.draw(1.0, &kh_vfx::Camera { position: eye, forward: Vec3::X, right: Vec3::Y, up: Vec3::Z }).0.char_params.iter().map(|p| (p.effect, p.target)).collect();
        let live = s.fx.world.live();
        let names = |sub: bool| -> Vec<String> {
            let mut v: Vec<String> = live.iter().filter(|l| params.iter().any(|(e, t)| *e == l.id.0 && (!sub || *t == 1))).map(|l| l.name.clone()).collect();
            v.dedup();
            v
        };
        let (sources, sub_sources) = (names(false), names(true));
        let charge = live.iter().any(|l| { let n = l.name.to_lowercase(); n.starts_with("vs_com_lv") || n.starts_with("vs_com_kyp") });
        let drives = s.fx.char_drives().into_iter().map(|d| CharDrive { owner: d.owner, name: d.name, value: d.value, vector: d.vector, mesh: d.mesh, equipment: d.equipment }).collect();
        let f = s.rig.sora.combat().form;
        let right = f.weapon_units().iter().find(|u| u.starts_with("RWeapon@")).and_then(|u| model_of(u)).map(|m| mesh_of(&m));
        self.frames.push(Frame { cmd: f.cmd, wear: f.wear_form, drives, sources, right, sub_sources, charge });
        state
    }

    fn idle(&mut self, s: &mut Session, n: usize) {
        for _ in 0..n {
            self.tick(s, &[]);
        }
    }

    fn until(&mut self, s: &mut Session, limit: usize, done: impl Fn(&Session) -> bool) -> bool {
        for _ in 0..limit {
            if done(s) {
                return true;
            }
            self.tick(s, &[]);
        }
        done(s)
    }

    /// The full gauge, Y, STYLECHANGE to its end and back to IDLE (audit_c_ld.rs `transform`). Returns the frame index of the
    /// transformation's end.
    fn transform(&mut self, s: &mut Session, what: &str) -> usize {
        s.rig.sora.debug_add_style(60.0);
        self.idle(s, 2);
        s.rig.sora.debug_add_style(40.0);
        self.idle(s, 15);
        self.tick(s, &[PadButton::SITUATION]);
        self.tick(s, &[]);
        assert!(self.until(s, 400, |s| s.rig.out.state == StateId::Stylechange), "{what}: STYLECHANGE starts");
        assert!(self.until(s, 400, |s| s.rig.out.state != StateId::Stylechange), "{what}: STYLECHANGE ends");
        let end = self.frames.len();
        assert!(self.until(s, 200, |s| s.rig.out.state == StateId::Idle), "{what}: back in IDLE");
        end
    }

    /// Losses to 1 HP until the roll offers Rage Form, then Y (audit_c_an.rs `enter_rage`).
    fn enter_rage(&mut self, s: &mut Session) -> usize {
        s.rig.sora.force_battle(true);
        self.idle(s, 10);
        let max = s.rig.sora.status().max_hp();
        for _ in 0..300 {
            if s.rig.sora.combat().situation.has(0x8B) {
                break;
            }
            s.rig.sora.status_mut().hp = max;
            self.idle(s, 1);
            s.rig.sora.status_mut().hp = 1;
            self.idle(s, 1);
        }
        assert!(s.rig.sora.combat().situation.has(0x8B), "Rage Form offered");
        s.rig.sora.status_mut().hp = max;
        self.idle(s, 60);
        self.tick(s, &[PadButton::SITUATION]);
        assert!(self.until(s, 60, |s| s.rig.out.state == StateId::Stylechange), "Rage: the transformation starts");
        assert!(self.until(s, 400, |s| s.rig.out.state != StateId::Stylechange && s.rig.sora.combat().form.cmd == 0x8B), "Rage Form starts");
        self.frames.len()
    }
}

/// The model of a weapon unit (`RWeapon@RIGHT_HAND:R_wep_skt=/Game/.../w_so200_Wep` -> `w_so200`).
fn model_of(unit: &str) -> Option<String> {
    // (the blueprints are `w_so200_Wep`, the shields' `w_so032_wep`)
    unit.rsplit('/').next().filter(|b| b.len() > 4 && b[b.len() - 4..].eq_ignore_ascii_case("_wep")).map(|b| b[..b.len() - 4].to_string())
}

/// A weapon model's mesh in Sora's library (the Kingdom Key is the main pack's `kingdom_key`).
fn mesh_of(model: &str) -> String {
    if model == crate::keyblade::KINGDOM_KEY { "kingdom_key".to_string() } else { format!("{}{model}", crate::keyblade::PREFIX) }
}

/// The models of a row's weapon units, every grow level and the transform units.
fn models(row: &KeybladeFormRow) -> Vec<String> {
    let grown = row.grow_units.iter().filter_map(|u| u.split_once(':').map(|(_, rest)| rest));
    let mut out: Vec<String> = Vec::new();
    for m in row.units.iter().copied().chain(grown).chain(row.transform_units.iter().copied()).filter_map(model_of) {
        if !out.contains(&m) {
            out.push(m);
        }
    }
    if out.is_empty() {
        out.push(crate::keyblade::KINGDOM_KEY.to_string());
    }
    out
}

/// One form's session: returns the frames, the index where KH3's look must have settled from (the transformation's end and
/// the slide), the index of the form's end.
fn session(row: &KeybladeFormRow) -> Option<(Rec, Marks)> {
    let keyblade = if row.keyblade == "-" { kh_core::combat::form::KINGDOM_KEY } else { row.keyblade };
    let names = models(row);
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut s = Session::new(keyblade, &refs, refs[0], Some(Vec3::new(300.0, 0.0, 0.0)), &format!("hair_pulse_{}", row.id.trim_start_matches("form.")))?;
    s.load_xsi_root();
    let mut r = Rec { frames: Vec::new() };
    r.idle(&mut s, 5);
    let end = if row.cmd == 0x8B {
        r.enter_rage(&mut s)
    } else {
        if row.stage == 2 {
            let first = r.transform(&mut s, row.first);
            assert_ne!(r.frames[first].cmd, 0, "{}: its first stage {}", row.id, row.first);
        }
        r.transform(&mut s, row.id)
    };
    assert_eq!(s.rig.sora.combat().form.cmd, row.cmd, "{}: the form is on", row.id);
    // the wear form's last change up to now (its slide runs from there)
    let changed = (1..r.frames.len()).rev().find(|&i| r.frames[i].wear != r.frames[i - 1].wear).unwrap_or(end);
    let settle_from = end.max(changed + (SLIDE_IN / STEP).ceil() as usize);
    // in the form: 6 s, the grow levels raised on the way (the shields grow by guards: GrowUp 0x1405BD810)
    for k in 0..HOLD {
        if row.grow_levels > 1 && k % (HOLD / row.grow_levels as usize) == 0 && k > 0 {
            let f = &mut s.rig.sora.combat_mut().form;
            f.grow = (f.grow + 1).min(row.grow_levels as u8);
        }
        r.tick(&mut s, &[]);
    }
    assert_eq!(s.rig.sora.combat().form.cmd, row.cmd, "{}: still in the form after 6 s", row.id);
    // the form's end by its time (EndStyle), then 2 s
    s.rig.sora.combat_mut().form.time_left = STEP;
    let held = r.frames.len();
    assert!(r.until(&mut s, 600, |s| s.rig.sora.combat().form.cmd == 0), "{}: the form ends by its time", row.id);
    // the end: the first frame whose wear form (EndStyle puts it back first) or command has left the form
    let worn = r.frames[held - 1].wear;
    let ended = (held..r.frames.len()).find(|&i| r.frames[i].wear != worn || r.frames[i].cmd == 0).unwrap_or(r.frames.len());
    r.idle(&mut s, AFTER + (SLIDE_OUT / STEP).ceil() as usize + 2);
    Some((r, Marks { changed, settle_from, ended }))
}

/// A session's frame marks: the wear form's last change before the hold, KH3's settle point (the transformation's end and
/// the 1 s slide), the form's end.
#[derive(Clone, Copy)]
struct Marks {
    changed: usize,
    settle_from: usize,
    ended: usize,
}

/// The uniform blocks of his body slots (mesh "sora"), as the renderer evaluates them (clock held: only the parameters move).
fn body_blocks(lib: &Library) -> Vec<Vec4> {
    let object = ObjectRows::new(&kh_anim::Trs::IDENTITY, (Vec3::new(0.0, 0.0, 90.0), Vec3::new(60.0, 60.0, 90.0), 110.0));
    let rig = kh_look::stand_in::rig();
    let frame = LookFrame { clock: 0.0, exposure: 1.0, debug: 0.0 };
    lib.slots.iter().filter(|s| s.mesh == "sora").flat_map(|s| lib.params(s, &object, &rig, None, frame).m).collect()
}

/// The hair's in-light as its material reads it (its MAIN `InLight` x the brightest channel of `InLightColor`), the strip's
/// brightness measure.
fn hair_in_light(lib: &Library) -> f32 {
    lib.slots
        .iter()
        .filter(|s| s.mesh == "sora" && s.owner.to_lowercase().contains("hair"))
        .map(|s| s.scalars.get("InLight").copied().unwrap_or(0.0) * s.vectors.get("InLightColor").map_or(0.0, |c| c[0].max(c[1]).max(c[2])))
        .fold(0.0, f32::max)
}

struct Outcome {
    /// the largest difference of the driven blocks from the undriven twin's (the same library, the same wear steps, no drive):
    /// held in the form (from `settle_from`) and after the end's slide back. 0 = nothing of an effect is left on his model.
    drive_in_form: f32,
    drive_after: f32,
    /// the undriven twin's last change, seconds after the wear change into the form / after the form's end: KH3's wear
    /// slide (1 s in, 0.2 s out, then the costume swap) is over by then and nothing moves after it
    slide_in_form: f32,
    slide_after: f32,
    /// the drives on his body at the frames that differ (for the report)
    left: Vec<String>,
    /// the hair's in-light during the transformation (KH3's own clip effects) and held in the form
    hair_transform: f32,
    hair_held: f32,
    /// the first frame off KH3's blocks (for the report)
    first_off: Option<String>,
    /// his weapons' charge glow held in the form
    weapon: WeaponGlow,
}

/// KH3's form charge glow on his right-hand weapon (vs_com_lv2's SUB in-light on that weapon's materials): its `Sub_InLight`
/// per held frame, its colours, and any `Sub_InLight` on another weapon or left after the form.
#[derive(Default)]
struct WeaponGlow {
    /// the right-hand weapon's slots in the library (0: it is not drawn from this library)
    slots: usize,
    series: Vec<f32>,
    colour: Option<([f32; 3], [f32; 3])>,
    other_lit: f32,
    after_lit: f32,
    /// held frames with one of KH3's own SUB in-light effects on his weapon (not counted above)
    kh3_own_frames: usize,
}

impl WeaponGlow {
    /// (peak, trough, period s) of the held series; the period from its rises through 0.25 (None: under two)
    fn shape(&self) -> (f32, f32, Option<f32>) {
        let peak = self.series.iter().copied().fold(0.0, f32::max);
        let trough = self.series.iter().copied().fold(f32::MAX, f32::min);
        let rises: Vec<usize> = (1..self.series.len()).filter(|&i| self.series[i - 1] < 0.25 && self.series[i] >= 0.25).collect();
        let period = (rises.len() >= 2).then(|| (rises[rises.len() - 1] - rises[0]) as f32 * STEP / (rises.len() - 1) as f32);
        (peak, if self.series.is_empty() { 0.0 } else { trough }, period)
    }
}

/// The largest `Sub_InLight` on the slots of `mesh` (a weapon's), 0 without one.
fn sub_in_light(lib: &Library, mesh: &str) -> f32 {
    lib.slots.iter().filter(|s| s.mesh == mesh).filter_map(|s| s.scalars.get("Sub_InLight").copied()).fold(0.0, f32::max)
}

/// The weapon meshes of a library (his hand's Kingdom Key, the keyblades' `kb:` models).
fn weapon_meshes(lib: &Library) -> Vec<String> {
    let mut out: Vec<String> = lib.slots.iter().filter(|s| s.mesh == "kingdom_key" || s.mesh.starts_with(crate::keyblade::PREFIX)).map(|s| s.mesh.clone()).collect();
    out.sort();
    out.dedup();
    out
}

/// The frames through the renderer's drive and wear path on one outfit's library.
fn replay(pack: &std::path::Path, outfit: &str, frames: &[Frame], marks: Marks, weapons: &[String]) -> Option<Outcome> {
    let Marks { changed, settle_from, ended } = marks;
    let dir = crate::outfit::look_dir(pack, outfit);
    let load = || -> Option<Library> {
        let mut l = Library::load(&dir).ok()?;
        crate::rage::prepare(&dir, &mut l);
        Some(l)
    };
    let (mut lib, mut twin) = (load()?, load()?);
    // (his form's weapons' own slots, merged as the renderer does: keyblade::add_slots_with; the Kingdom Key is the main pack's)
    if let Some(t) = crate::keyblade::Table::read(pack) {
        let kb: Vec<String> = weapons.iter().filter(|m| m.as_str() != crate::keyblade::KINGDOM_KEY).cloned().collect();
        crate::keyblade::add_slots_with(&t, &mut lib, &kb);
    }
    let all_weapons = weapon_meshes(&lib);
    let diff = |a: &[Vec4], b: &[Vec4]| a.iter().zip(b).map(|(x, y)| (*x - *y).abs().max_element()).fold(0.0, f32::max);
    let mut o = Outcome { drive_in_form: 0.0, drive_after: 0.0, slide_in_form: 0.0, slide_after: 0.0, left: Vec::new(), hair_transform: 0.0, hair_held: 0.0, first_off: None, weapon: WeaponGlow::default() };
    let after_from = ended + (SLIDE_OUT / STEP).ceil() as usize + 1;
    let mut twin_before: Option<Vec<Vec4>> = None;
    for (i, f) in frames.iter().enumerate() {
        let drives: Vec<CharDrive> = f.drives.clone();

        let names: Vec<String> = drives.iter().filter(|d| d.owner == 0 && d.mesh.is_none()).map(|d| d.name.clone()).collect();
        set_char_drives(drives);
        apply_char_drives(&mut lib, 0);
        lib.step_wear(f.wear, STEP);
        twin.step_wear(f.wear, STEP);
        let hair = hair_in_light(&lib);
        if i < changed {
            o.hair_transform = o.hair_transform.max(hair);
            continue;
        }
        let steady = body_blocks(&twin);
        // the twin moved this frame: the wear slide (its last change is timed from the wear change / the form's end)
        if twin_before.as_deref().is_some_and(|b| diff(&steady, b) > 0.0) {
            if i < ended {
                o.slide_in_form = (i - changed) as f32 * STEP;
            } else {
                o.slide_after = (i - ended) as f32 * STEP;
            }
        }
        if i < settle_from {
            o.hair_transform = o.hair_transform.max(hair);
            twin_before = Some(steady);
            continue;
        }
        let blocks = body_blocks(&lib);
        let drive = diff(&blocks, &steady);
        if drive > 0.0 && o.first_off.is_none() {
            // the first differing frame: its offset from the settle point, the block row, the drives on the body then
            let row = blocks.iter().zip(&steady).position(|(x, y)| (*x - *y).abs().max_element() > 0.0).unwrap_or(0);
            o.first_off = Some(format!("frame +{} (settle point), block row {} ({:?} vs {:?}), drives {names:?} from {:?}", i as i64 - settle_from as i64, row, blocks[row], steady[row], f.sources));
        }
        twin_before = Some(steady);
        if i < ended {
            o.hair_held = o.hair_held.max(hair);
            o.drive_in_form = o.drive_in_form.max(drive);
            // his right-hand weapon's glow, and none on any other weapon
            // (a frame with one of KH3's own SUB in-light effects on his weapon, e.g. Storm Flag's vw_so093_fns_ora0, is KH3's look:
            // not counted, but told)
            let own = !f.sub_sources.is_empty();
            if own {
                o.weapon.kh3_own_frames += 1;
            }
            if let Some(right) = f.right.as_deref().filter(|_| !own) {
                o.weapon.slots = lib.slots.iter().filter(|s| s.mesh == right).count();
                let v = sub_in_light(&lib, right);
                o.weapon.series.push(v);
                if v > 0.0 && o.weapon.colour.is_none() {
                    let slot = lib.slots.iter().find(|s| s.mesh == right && s.scalars.contains_key("Sub_InLight"));
                    let c = |n: &str| slot.and_then(|s| s.vectors.get(n)).map_or([0.0; 3], |v| [v[0], v[1], v[2]]);
                    o.weapon.colour = Some((c("Sub_InLightColor"), c("Sub_InLightOutColor")));
                }
            }
            for m in all_weapons.iter().filter(|m| !own && f.right.as_deref() != Some(m.as_str())) {
                o.weapon.other_lit = o.weapon.other_lit.max(sub_in_light(&lib, m));
            }
        } else if i >= after_from {
            o.drive_after = o.drive_after.max(drive);
        }
        if i + 1 == frames.len() && f.sub_sources.is_empty() {
            // the form is over: nothing of the glow left on any weapon (its emitter ran out after the loop end)
            o.weapon.after_lit = all_weapons.iter().map(|m| sub_in_light(&lib, m)).fold(0.0, f32::max);
        }
        if drive > 0.0 && (i < ended || i >= after_from) {
            for n in names {
                if !o.left.contains(&n) {
                    o.left.push(n);
                }
            }
        }
    }
    set_char_drives(Vec::new());
    apply_char_drives(&mut lib, 0);
    Some(o)
}

/// (HAIR-PULSE) THE FULL SWEEP: every form row x every outfit (216 rows, about 7 min in release, far longer in debug), so it is
/// left out of normal runs. Run it with `cargo test --release -p kh-er-render --lib form_pulse -- --ignored --nocapture`, or
/// any rows of it with `KH_FORM_PULSE_ONLY=form.TF,form.CL2` (then every outfit).
#[test]
#[ignore = "the full 216-row sweep (minutes): --ignored, or KH_FORM_PULSE_ONLY=form.X"]
fn no_form_leaves_sora_pulsing_in_any_outfit() {
    sweep(&[], &OUTFITS);
}

/// (HAIR-PULSE) The quick one of every normal run: Double Form (the dual-wield report: causes A and D, the weapon's glow),
/// Blizzard Claws (cause C: its transform unit's aura) and Ultimate Form (cause B: no glow) on KH3's outfit. With
/// `KH_FORM_PULSE_ONLY` set, those rows on every outfit instead.
#[test]
fn double_form_blizzard_claws_and_ultimate_form_leave_sora_flat() {
    if std::env::var("KH_FORM_PULSE_ONLY").is_ok_and(|v| !v.is_empty()) {
        sweep(&[], &OUTFITS);
    } else {
        sweep(&["form.TF", "form.CL2", "form.SW"], &["kh3"]);
    }
}

/// The renderer's drive state (`CHAR_DRIVES` / `CHAR_HELD`) is global: one sweep at a time (the quick test and the full sweep
/// run in parallel with `--include-ignored`).
static SWEEP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Runs `rows` (empty: every row, or `KH_FORM_PULSE_ONLY`'s) on `outfits` and asserts the rules above.
fn sweep(rows: &[&str], outfits: &[&str]) {
    let _one_at_a_time = SWEEP_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let pack = kh_host::audit_a::project().join("build").join("sora_pack");
    if !pack.join("vfx").join("effects").join("vs_com_lv2.json").is_file() || !pack.join("materials").join("materials.json").is_file() {
        eprintln!("SKIPPED: no pack under {}", pack.display());
        return;
    }
    let mut table = vec![format!("{:<9} {:<13} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} | weapon {:<14} {:>5} {:<5} | {:<6}", "form", "outfit", "drive", "after", "slide", "slide out", "hair trf", "hair held", "(right hand)", "glow", "col", "result")];
    let mut fails = Vec::new();
    // `KH_FORM_PULSE_ONLY=form.CL2,form.TF`: those rows alone (a quick look); unset: `rows` (empty: every row)
    let only = std::env::var("KH_FORM_PULSE_ONLY").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| rows.join(","));
    for row in KEYBLADE_FORMS.iter().filter(|r| only.is_empty() || only.split(',').any(|o| o == r.id)) {
        let Some((rec, marks)) = session(row) else { return };
        // (HAIR-PULSE) no SUB in-light drive (the form charge glow vs_com_lv2) and no charge effect in any form: KH3 never raises
        // the form charge level (STATE.md section 6)
        let charged = rec.frames.iter().filter(|f| f.charge).count();
        assert_eq!(charged, 0, "{}: a form charge effect (vs_com_lv1 / lv2 / kyp01) was live in {charged} frame(s)", row.id);
        for outfit in outfits.iter().copied() {
            let weapons = models(row);
            let Some(o) = replay(&pack, outfit, &rec.frames, marks, &weapons) else {
                eprintln!("(no {outfit} library in the pack)");
                continue;
            };
            // no form glows its weapon either, on any weapon, at any time (the right hand's series, every other weapon, after)
            let (peak, ..) = o.weapon.shape();
            let glow_ok = peak == 0.0 && o.weapon.other_lit == 0.0 && o.weapon.after_lit == 0.0 && o.weapon.colour.is_none();
            let pass = glow_ok && o.drive_in_form == 0.0 && o.drive_after == 0.0 && o.slide_in_form <= SLIDE_IN + 2.0 * STEP && o.slide_after <= SLIDE_OUT + 2.0 * STEP;
            table.push(format!(
                "{:<9} {:<13} {:>9.2e} {:>9.2e} {:>8.2}s {:>8.2}s {:>9.3} {:>9.3} | weapon {:<14} {:>5.3} {:<5} | {:<6}",
                row.id,
                outfit,
                o.drive_in_form,
                o.drive_after,
                o.slide_in_form,
                o.slide_after,
                o.hair_transform,
                o.hair_held,
                rec.frames[marks.settle_from].right.as_deref().unwrap_or("-"),
                peak.max(o.weapon.other_lit).max(o.weapon.after_lit),
                if o.weapon.colour.is_some() { "LIT".to_string() } else if o.weapon.kh3_own_frames > 0 { format!("kh3:{}", o.weapon.kh3_own_frames) } else { "none".to_string() },
                if pass { "PASS" } else { "FAIL" }
            ));
            if !pass {
                fails.push(format!("{} {outfit}: drive {:.3e} in the form / {:.3e} after, slide over {:.2} s / {:.2} s, drives on the body {:?}, first off: {}; weapon glow peak {peak:.3} other {:.3} after {:.3} colour {:?}", row.id, o.drive_in_form, o.drive_after, o.slide_in_form, o.slide_after, o.left, o.first_off.as_deref().unwrap_or("-"), o.weapon.other_lit, o.weapon.after_lit, o.weapon.colour));
            }
        }
    }
    eprintln!("(HAIR-PULSE) per form and outfit, from KH3's settle point (the transformation's end and the 1 s slide) for 6 s and from");
    eprintln!("the end's 0.2 s slide for 2 s: drive / after = the largest difference of his body's uniform blocks from the same library");
    eprintln!("without drives (0 = flat, nothing of an effect on his model), slide / slide out = the undriven blocks' last change after");
    eprintln!("the wear change / the form's end (KH3: 1 s / 0.2 s), hair trf / held = the hair's in-light (InLight x colour) during the");
    eprintln!("transformation / held in the form; weapon glow = the largest Sub_InLight on any of his weapons (KH3: none, 0)");
    for line in &table {
        eprintln!("{line}");
    }
    assert!(fails.is_empty(), "forms that leave Sora pulsing or off KH3's look:\n{}", fails.join("\n"));
}
/// (HAIR-PULSE) A weapon slot's in-light centre handed in the world (w 1) is measured from its own object position at its draw
/// (`p8` = `kh.obj`): `world - obj`, w 0; his body's slots and centres already relative (w 0) are left as they are.
#[test]
fn a_weapon_slot_takes_its_glow_centre_in_its_own_object_frame() {
    let slot = |mesh: &str, centre: [f32; 4]| kh_look::library::Slot {
        mesh: mesh.to_string(),
        slot: "m".to_string(),
        owner: "o".to_string(),
        owner_id: 0,
        instance_name: "i".to_string(),
        root: String::new(),
        masked: false,
        translucent: false,
        two_sided: false,
        subsurface: false,
        textures: Vec::new(),
        texture_names: Vec::new(),
        uv2: None,
        scalars: std::collections::HashMap::new(),
        vectors: [("Sub_InLightCenterPosition".to_string(), centre)].into_iter().collect(),
    };
    let at = kh_anim::Trs { translation: Vec3::new(100.0, 20.0, 90.0), ..kh_anim::Trs::IDENTITY };
    let object = ObjectRows::new(&at, (Vec3::new(0.0, 0.0, 40.0), Vec3::splat(10.0), 50.0));
    let out = super::weapon_in_light(&slot("kb:w_so200", [110.0, 20.0, 150.0, 1.0]), &object).expect("a weapon centre in the world");
    assert_eq!(out.vectors["Sub_InLightCenterPosition"], [10.0, 0.0, 20.0, 0.0]);
    assert!(super::weapon_in_light(&slot("kb:w_so200", [1.0, 2.0, 3.0, 0.0]), &object).is_none(), "already relative");
    assert!(super::weapon_in_light(&slot("sora", [110.0, 20.0, 150.0, 1.0]), &object).is_none(), "his body: the host's rule");
}
/// (MIMIC-BOSS drives fix) A copy of Sora draws HIS drives (owner = member + 1), never the player's: the player's in-light is
/// taken back to the slot's own value on the copy's slot copy, the copy's own glow goes on his body / his form weapon only,
/// and the library the player draws keeps the player's.
#[test]
fn a_copy_of_sora_shows_its_own_drives_not_the_players() {
    let _one_at_a_time = SWEEP_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let pack = kh_host::audit_a::project().join("build").join("sora_pack");
    let Ok(mut lib) = Library::load(&crate::outfit::look_dir(&pack, "kh3")) else {
        eprintln!("SKIPPED: no kh3 library under {}", pack.display());
        return;
    };
    let body = lib.slots.iter().position(|s| s.mesh == "sora").expect("a body slot");
    let name = "MimicDrivesFixProbe".to_string();
    let drive = |owner: u8, value: f32, mesh: Option<&str>| CharDrive { owner, name: name.clone(), value: [value, 0.0, 0.0, 0.0], vector: false, mesh: mesh.map(str::to_string), equipment: false };
    let before = lib.slots[body].scalars.get(&name).copied();
    set_char_drives(vec![drive(0, 1.0, None), drive(11, 0.25, None), drive(12, 0.5, Some(crate::keyblade::KINGDOM_KEY))]);
    apply_char_drives(&mut lib, 0);
    assert_eq!(lib.slots[body].scalars.get(&name).copied(), Some(1.0), "the player's own drive on the library");
    let copy = super::copy_drive_slot(&lib, body, 11).expect("his slot copy");
    assert_eq!(copy.scalars.get(&name).copied(), Some(0.25), "the copy's own glow");
    // a copy with no drive of its own: the player's taken back (what the slot held before it)
    let bare = super::copy_drive_slot(&lib, body, 12).expect("the player's drive undone");
    assert_eq!(bare.scalars.get(&name).copied(), before);
    // a drive on the copy's Kingdom Key reaches its slots, not his body
    if let Some(key) = lib.slots.iter().position(|s| s.mesh == "kingdom_key") {
        assert_eq!(super::copy_drive_slot(&lib, key, 12).and_then(|s| s.scalars.get(&name).copied()), Some(0.5));
    }
    assert_eq!(lib.slots[body].scalars.get(&name).copied(), Some(1.0), "the player's library untouched");
    set_char_drives(Vec::new());
    apply_char_drives(&mut lib, 0);
    assert_eq!(lib.slots[body].scalars.get(&name).copied(), before, "the drive ended");
    assert!(super::copy_drive_slot(&lib, body, 11).is_none(), "nothing driven: the slot as it is");
}
