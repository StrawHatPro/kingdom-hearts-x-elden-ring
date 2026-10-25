//! DAMAGE OUT THROUGH THE GAME'S OWN PIPELINE (IMPL_ER5_STATE.md, route (a)): every hit of Sora's
//! becomes an invisible, instant, tiny bullet owned by the player, spawned at the contact point
//! with the game's own `CSBulletManager::spawn_bullet`. Its Bullet row points at an AtkParam_Pc row
//! that is patched in memory for this one hit (physical attack value, poise damage, `dmgLevel` =
//! the hit reaction asked for), so the game itself computes the damage against the target's
//! defences, takes poise, plays the hit reaction, turns the target hostile and credits the kill.
//!
//! THE ROWS: nothing is added to the game's tables. `SLOTS` existing Bullet rows and as many
//! AtkParam_Pc rows are BORROWED: chosen at the first use by a rule (plain, single, physical player
//! bullets: `pick`), their whole contents saved, overwritten while a bullet of ours may still read
//! them (`HOLD` seconds after the spawn), and put back as they were when the slot has been idle for
//! `IDLE_RESTORE` seconds, when Sora stops, and when the payload is replaced or shut down (`Drop`).
//! The param tables are memory only: `regulation.bin` is never touched.
//!
//! THE SPAWN DATA (0x110 bytes, private in the SDK: built as bytes). Offsets read from the game's
//! own copy routine (exe 2.7.1.0, the callee of the spawn function that takes the caller's block):
//! 0x00 owner handle, 0x08 behaviour id, 0x0c magic id, 0x10 (word), 0x14 bullet id, 0x18 goods id,
//! 0x1c dummy poly, 0x20 target handle, 0x40 (word), 0x44 flag bits, 0x50 / 0x60 / 0x70 / 0x80 four
//! vectors read with ALIGNED moves (acceleration direction, ?, forward, position), 0x90, 0x98 and
//! 0xa0 two reference-counted pointers (must be null), 0xa8 a byte (its own default: 0xff), 0x100 a byte.

use eldenring::cs::{AtkParam_Pc, Bullet, CSBulletManager, FieldInsHandle, SoloParamRepository};
use eldenring::param::{ATK_PARAM_ST, BULLET_PARAM_ST};
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::control::Control;
use crate::erlog;
use crate::game;

/// `blt.dump = 1`: the discovery listing of attack rows is logged even when rows were found.
pub static DUMP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const SLOTS: usize = 12;
/// A slot's rows stay ours this long after its bullet was spawned (the bullet lives `life` s).
const HOLD: f32 = 0.2;
/// Rows of slots unused for this long are put back as the game had them.
const IDLE_RESTORE: f32 = 2.0;

/// What one hit asks of the game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attack {
    /// `atkPhys` (the game applies the target's defence and absorption to it).
    pub phys: u16,
    /// `atkSuperArmor`: poise / stance damage.
    pub poise: f32,
    /// `atkStam`: stamina damage (against a guard).
    pub stamina: u16,
    /// `dmgLevel`: the hit reaction asked for.
    pub level: u8,
    /// (IMPL_ER32) WHICH DAMAGE TYPE CARRIES `phys` (the number is one total, never split).
    pub carrier: Carrier,
    /// (IMPL_ER32) `spAttribute` of the attack row AND of the bullet row (the game's "this is fire" mark); `None` = the shape's / the donor's.
    pub sp_attribute: Option<u8>,
    /// (IMPL_ER32) `atkObj`: the attack value against map objects; `None` = the donor row's own.
    pub obj: Option<u16>,
}

/// (IMPL_ER32) The damage type of the attack row that gets the number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Carrier {
    /// What `Shape::element` says (0 physical, 1 magic, 2 fire): the rows of every build before IMPL_ER32.
    #[default]
    Shape,
    Physical,
    Magic,
    Fire,
    Lightning,
    /// Elden Ring's holy (`atkDark` in the row).
    Holy,
}

impl Carrier {
    /// The sheet's code (rows `damage.element.*`): 0 physical, 1 magic, 2 fire, 3 lightning, 4 holy.
    pub fn from_code(code: u8) -> Carrier {
        match code {
            1 => Carrier::Magic,
            2 => Carrier::Fire,
            3 => Carrier::Lightning,
            4 => Carrier::Holy,
            _ => Carrier::Physical,
        }
    }

    /// The index into (physical, magic, fire, lightning, holy); `None`: no type takes the number (`atk.element` above 2, as before).
    fn index(self, shape_element: u8) -> Option<usize> {
        match self {
            Carrier::Shape => (shape_element <= 2).then_some(shape_element as usize),
            Carrier::Physical => Some(0),
            Carrier::Magic => Some(1),
            Carrier::Fire => Some(2),
            Carrier::Lightning => Some(3),
            Carrier::Holy => Some(4),
        }
    }
}

impl Attack {
    /// No number, and the three IMPL_ER32 fields at "as before": `Attack { phys, .., ..Attack::PLAIN }`.
    pub const PLAIN: Attack = Attack { phys: 0, poise: 0.0, stamina: 0, level: 0, carrier: Carrier::Shape, sp_attribute: None, obj: None };

    /// (physical, magic, fire, lightning, holy) of the row: the whole number in ONE of them.
    pub fn split(&self, shape_element: u8) -> [u16; 5] {
        let mut out = [0u16; 5];
        if let Some(k) = self.carrier.index(shape_element) {
            out[k] = self.phys;
        }
        out
    }
}

/// (IMPL_ER32) What the game's own player attack rows say (read once, `Bullets::survey`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Survey {
    pub rows: u32,
    /// `atkObj`: rows with a value, the commonest value, the highest.
    pub obj_rows: u32,
    pub obj_common: u16,
    pub obj_max: u16,
    /// The commonest non-zero `spAttribute` of the rows whose only damage type is magic / fire / lightning / holy
    /// (index 1..=4 as `Attack::split`; index 0 unused), with how many rows carry it of how many.
    pub sp: [Option<(u8, u32, u32)>; 5],
    /// `atkObj` of the rows we borrow, as the game has them.
    pub donors: Vec<(u32, u16)>,
}

impl Survey {
    pub fn sp_of(&self, carrier: Carrier) -> Option<u8> {
        carrier.index(0).filter(|k| *k > 0).and_then(|k| self.sp[k]).map(|s| s.0)
    }
}

/// (IMPL_ER32) One row for `sp_marks`: (physical, magic, fire, lightning, holy) each "flat value + correction", and its `spAttribute`.
pub type ElementRow = ([u32; 5], u8);

/// (IMPL_ER32) THE GAME'S OWN MARK OF AN ELEMENT: among the rows whose ONLY damage type is one element (no physical part,
/// no other element), the commonest non-zero `spAttribute`. Index as `Attack::split`; `None`: no such row carries a mark.
pub fn sp_marks(rows: impl Iterator<Item = ElementRow>) -> [Option<(u8, u32, u32)>; 5] {
    let mut tally: [std::collections::BTreeMap<u8, u32>; 5] = Default::default();
    let mut total = [0u32; 5];
    for (values, sp) in rows {
        let mut carriers = (0..5).filter(|k| values[*k] > 0);
        let (Some(k), None) = (carriers.next(), carriers.next()) else { continue };
        if k == 0 {
            continue;
        }
        total[k] += 1;
        if sp != 0 {
            *tally[k].entry(sp).or_insert(0) += 1;
        }
    }
    let mut out = [None; 5];
    for k in 1..5 {
        // (the commonest; of two equally common the lower value)
        out[k] = tally[k].iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(sp, n)| (*sp, *n, total[k]));
    }
    out
}

/// (IMPL_ER32) The attack row of one hit: the donor row with everything that matters overwritten (pure; `fire` writes it).
pub fn patch_attack(a: &mut ATK_PARAM_ST, attack: Attack, shape: Shape) {
    let [phys, mag, fire, thun, dark] = attack.split(shape.element);
    a.set_atk_phys(phys);
    a.set_atk_mag(mag);
    a.set_atk_fire(fire);
    a.set_atk_thun(thun);
    a.set_atk_dark(dark);
    a.set_atk_stam(attack.stamina);
    a.set_atk_super_armor(attack.poise);
    a.set_dmg_level(attack.level);
    a.set_knockback_dist(0.0);
    a.set_hit_stop_time(0.0);
    a.set_sp_effect_id0(-1);
    a.set_sp_effect_id1(-1);
    a.set_sp_effect_id2(-1);
    a.set_sp_effect_id3(-1);
    a.set_sp_effect_id4(-1);
    a.set_is_arrow_atk(false);
    a.set_is_add_base_atk(shape.add_base);
    // (a player row's numbers are its flat value plus the weapon's times these: none of the weapon's)
    a.set_atk_phys_correction(shape.corr);
    a.set_atk_mag_correction(shape.corr);
    a.set_atk_fire_correction(shape.corr);
    a.set_atk_thun_correction(shape.corr);
    a.set_atk_dark_correction(shape.corr);
    a.set_atk_stam_correction(shape.corr);
    match shape.extra {
        1 => a.set_is_disable_no_damage(true),
        n @ 20..=29 => a.set_hit_source_type((n - 20) as u8),
        n @ 30..=39 => a.set_atk_behavior_id((n - 30) as u8),
        n @ 40..=49 => a.set_sub_category1((n - 40) as u8),
        4 => a.set_overwrite_attack_element_correct_id(-1),
        5 => a.set_final_damage_rate_id(0),
        _ => {}
    }
    a.set_atk_super_armor_correction(0.0);
    // Standard physical, no element.
    a.set_atk_attribute(shape.attribute);
    a.set_sp_attribute(attack.sp_attribute.unwrap_or(shape.sp_attribute));
    // (IMPL_ER32) the value against map objects
    if let Some(obj) = attack.obj {
        a.set_atk_obj(obj);
    }
}

/// The bullet's shape (control keys `blt.*`; the defaults are what was measured to land).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub life: f32,
    pub speed: f32,
    pub radius: f32,
    /// Metres the bullet starts in front of the contact point (against the blow's direction).
    pub back: f32,
    /// (experiments, keys `atk.*`) overrides of the attack row: the corrections, "add base attack",
    /// the attribute bytes, and which damage type carries the number (0 physical, 1 magic, 2 fire).
    pub corr: u16,
    pub add_base: bool,
    pub attribute: u8,
    pub sp_attribute: u8,
    pub element: u8,
    pub extra: i64,
    /// The flag word of the spawn data (0x44) and its magic / goods ids (0x0c / 0x18).
    pub flags: u32,
    pub magic: i32,
    pub goods: i32,
    /// (experiments, IMPL_ER6) `blt.sfx = 1`: the donor row keeps its own effects; `blt.pen = 0`: the bullet does not pass through the map.
    pub keep_sfx: bool,
    pub penetrate: bool,
}

impl Default for Shape {
    fn default() -> Shape {
        Shape { life: 0.12, speed: 20.0, radius: 0.12, back: 0.35, corr: 0, add_base: false, attribute: 3, sp_attribute: 0, element: 0, extra: 0, flags: FLAT_ATTACK | ONE_SHOT, magic: -1, goods: -1, keep_sfx: false, penetrate: true }
    }
}

impl Shape {
    pub fn from_control(c: &Control) -> Shape {
        let d = Shape::default();
        Shape { life: c.f32("blt.life").unwrap_or(d.life), speed: c.f32("blt.speed").unwrap_or(d.speed), radius: c.f32("blt.radius").unwrap_or(d.radius), back: c.f32("blt.back").unwrap_or(d.back), corr: c.i64("atk.corr").map_or(d.corr, |v| v as u16), add_base: c.on("atk.addbase"), attribute: c.i64("atk.attr").map_or(d.attribute, |v| v as u8), sp_attribute: c.i64("atk.spattr").map_or(d.sp_attribute, |v| v as u8), element: c.i64("atk.element").map_or(d.element, |v| v as u8), extra: c.i64("atk.extra").unwrap_or(0), flags: c.i64("blt.flags").map_or(d.flags, |v| v as u32), magic: c.i64("blt.magic").map_or(d.magic, |v| v as i32), goods: c.i64("blt.goods").map_or(d.goods, |v| v as i32), keep_sfx: c.on("blt.sfx"), penetrate: c.flag("blt.pen") != Some(false) }
    }
}

struct Slot {
    bullet_id: u32,
    atk_id: u32,
    bullet_saved: BULLET_PARAM_ST,
    atk_saved: ATK_PARAM_ST,
    patched: bool,
    used_at: f32,
}

#[derive(Default)]
pub struct Bullets {
    slots: Vec<Slot>,
    /// The rows were looked for (once); `None` afterwards = the route is not available.
    picked: bool,
    clock: f32,
    next: usize,
    pub spawned: u32,
    pub failed: u32,
    /// `blt.bullet` / `blt.atk`: explicit donor rows (experiments).
    pub log_rows: bool,
    /// The emitters an older build left alive were looked for (once).
    swept: bool,
    raw: Option<(u32, f32)>,
    /// (ER party S3) stage 2: the AtkParam_Npc rows of the slots' attack ids, saved as the game had them (id, row, last use).
    #[cfg(feature = "party")]
    npc_saved: Vec<(u32, ATK_PARAM_ST, f32, bool)>,
    /// (AGGRO) THE BODY-OWNED BULLETS' OWN ROWS: Bullet rows whose attack id has a row in BOTH AtkParam_Pc and AtkParam_Npc
    /// (the game reads the NPC table for a bullet an NPC owns; none of the 12 player slots' attack ids has an NPC row in
    /// the 2.7.1.0 / 2.7.2.0 regulation, read offline, so a body's bullet carried no attack at all). Picked at the first
    /// owned bullet; (slot, the NPC row as the game had it). The player's slots are untouched.
    #[cfg(feature = "party")]
    owned: Vec<(Slot, ATK_PARAM_ST)>,
    #[cfg(feature = "party")]
    owned_picked: bool,
    #[cfg(feature = "party")]
    owned_next: usize,
}

#[repr(C, align(16))]
struct SpawnData([u8; 0x110]);

fn put<T: Copy>(data: &mut SpawnData, at: usize, value: T) {
    assert!(at + std::mem::size_of::<T>() <= data.0.len());
    // SAFETY: inside the block (checked); unaligned write of a plain value.
    unsafe { std::ptr::write_unaligned(data.0.as_mut_ptr().add(at) as *mut T, value) };
}

fn repo() -> Option<&'static mut SoloParamRepository> {
    // SAFETY: game thread; the singleton lives as long as the game. Its holder table is checked by the SDK.
    unsafe { SoloParamRepository::instance_mut() }.ok()
}

/// A player attack row we can borrow: no throw, not an arrow (arrows take the bow's numbers).
/// Everything else that matters is overwritten per hit (`fire`).
fn plain_attack(a: &ATK_PARAM_ST) -> bool {
    a.throw_type_id() == 0 && !a.is_arrow_atk()
}

/// A plain bullet row: one shot, no child bullets, no homing, no gravity.
fn plain_bullet(b: &BULLET_PARAM_ST) -> bool {
    b.atk_id_bullet() > 0 && b.num_shoot() == 1 && b.hit_bullet_id() <= 0 && b.interval_create_bullet_id() <= 0 && b.gravity_in_range() == 0.0 && b.homing_angle() == 0 && b.follow_type() == 0 && !b.is_endless_hit() && b.sp_effect_id0() <= 0
}

impl Bullets {
    pub fn available(&self) -> bool {
        !self.slots.is_empty()
    }

    /// A row pair is free for a bullet now.
    pub fn free_slot(&self) -> bool {
        self.slots.iter().any(|s| self.clock - s.used_at > HOLD)
    }

    pub fn picked(&self) -> bool {
        self.picked
    }

    pub fn describe(&self) -> String {
        self.slots.iter().map(|s| format!("{}>{}", s.bullet_id, s.atk_id)).collect::<Vec<_>>().join(" ")
    }

    /// Chooses the rows to borrow (once). The SDK's param access panics on a table it does not
    /// recognise: that is caught here and leaves the route unavailable.
    pub fn pick(&mut self, control: &Control) {
        if self.picked {
            return;
        }
        self.picked = true;
        let wanted_bullet = control.i64("blt.bullet").map(|v| v as u32);
        let found = std::panic::catch_unwind(|| {
            let repo = repo()?;
            // Bullet rows whose attack row (taken as a player attack) is a plain physical one.
            let mut pairs: Vec<(bool, u32, u32)> = Vec::new();
            let mut plain = 0;
            for (id, b) in repo.rows::<Bullet>() {
                if !plain_bullet(b) && wanted_bullet != Some(id) {
                    continue;
                }
                plain += 1;
                let atk = b.atk_id_bullet() as u32;
                if repo.get::<AtkParam_Pc>(atk).is_some_and(plain_attack) && !pairs.iter().any(|(_, _, a)| *a == atk) {
                    // (rows that already have no visuals first)
                    pairs.push((b.sfx_id_bullet() > 0 || b.sfx_id_hit() > 0, id, atk));
                }
            }
            erlog!("bullet: {} plain bullet row(s), {} of them with a plain physical AtkParam_Pc row of their own", plain, pairs.len());
            if pairs.is_empty() || crate::bullet::DUMP.load(std::sync::atomic::Ordering::Relaxed) {
                // (discovery) what the player attack rows of plain bullets look like.
                let mut shown = 0;
                let mut seen: Vec<u32> = Vec::new();
                for (id, b) in repo.rows::<Bullet>() {
                    let atk = b.atk_id_bullet() as u32;
                    if !plain_bullet(b) || seen.contains(&atk) {
                        continue;
                    }
                    seen.push(atk);
                    if let Some(a) = repo.get::<AtkParam_Pc>(atk) {
                        if seen.len() % 12 == 0 && shown < 60 {
                            shown += 1;
                            erlog!(
                                "bullet:   row {id} life {} v {} r {} sfx {} > atk {atk}: phys {} mag {} fire {} thun {} dark {} stam {} poise {} lvl {} corr {}/{}/{} sp {} {} throw {} arrow {} addbase {} attr {} spattr {} type {}",
                                b.life(), b.init_vellocity(), b.hit_radius(), b.sfx_id_bullet(), a.atk_phys(), a.atk_mag(), a.atk_fire(), a.atk_thun(), a.atk_dark(), a.atk_stam(), a.atk_super_armor(), a.dmg_level(),
                                a.atk_phys_correction(), a.atk_mag_correction(), a.atk_fire_correction(), a.sp_effect_id0(), a.sp_effect_id1(), a.throw_type_id(), a.is_arrow_atk(), a.is_add_base_atk(), a.atk_attribute(), a.sp_attribute(), a.atk_type()
                            );
                        }
                    }
                }
                erlog!("bullet:   ({} distinct attack ids named by plain bullets; every 12th shown)", seen.len());
            }
            // Rows without visuals of their own first, then by id; an explicit `blt.bullet` before all.
            pairs.sort();
            if let Some(w) = wanted_bullet {
                pairs.sort_by_key(|(_, id, _)| *id != w);
            }
            pairs.truncate(SLOTS);
            let pairs: Vec<(u32, u32)> = pairs.into_iter().map(|(_, b, a)| (b, a)).collect();
            let slots: Vec<Slot> = pairs
                .into_iter()
                .filter_map(|(bullet_id, atk_id)| Some(Slot { bullet_id, atk_id, bullet_saved: repo.get::<Bullet>(bullet_id)?.clone(), atk_saved: repo.get::<AtkParam_Pc>(atk_id)?.clone(), patched: false, used_at: -100.0 }))
                .collect();
            Some(slots)
        });
        match found {
            Ok(Some(slots)) if !slots.is_empty() => {
                self.slots = slots;
                erlog!("bullet: rows borrowed per hit (Bullet > AtkParam_Pc): {}", self.describe());
                if let Some(s) = self.slots.first() {
                    let (b, a) = (&s.bullet_saved, &s.atk_saved);
                    erlog!(
                        "bullet: the first pair as the game has it: bullet {} life {} dist {} v {} r {} sfx {} | atk {} phys {} stam {} poise {} dmgLevel {} attribute {} sp {} type {} material {} hit source {} behaviour {}",
                        s.bullet_id, b.life(), b.dist(), b.init_vellocity(), b.hit_radius(), b.sfx_id_bullet(), s.atk_id, a.atk_phys(), a.atk_stam(), a.atk_super_armor(), a.dmg_level(), a.atk_attribute(), a.sp_attribute(), a.atk_type(), a.atk_material(), a.hit_source_type(), a.atk_behavior_id()
                    );
                }
            }
            Ok(_) => erlog!("bullet: no usable Bullet / AtkParam_Pc rows were found: the bullet route is NOT available"),
            Err(_) => erlog!("bullet: the SDK's param access failed (see the panic line): the bullet route is NOT available"),
        }
    }

    /// (IMPL_ER32) WHAT THE GAME'S OWN PLAYER ATTACK ROWS SAY about objects and elements (read once by breakables.rs):
    /// the object attack values in use, and the `spAttribute` the game gives its pure fire / magic / lightning / holy
    /// rows. Read only; the SDK's panic on an unknown table is caught (an empty survey).
    pub fn survey(&self) -> Survey {
        let donors: Vec<(u32, u16)> = self.slots.iter().map(|s| (s.atk_id, s.atk_saved.atk_obj())).collect();
        let found = std::panic::catch_unwind(|| {
            let repo = repo()?;
            let mut s = Survey::default();
            let mut objs: std::collections::BTreeMap<u16, u32> = Default::default();
            let mut rows: Vec<ElementRow> = Vec::new();
            for (_, a) in repo.rows::<AtkParam_Pc>() {
                s.rows += 1;
                let obj = a.atk_obj();
                if obj > 0 {
                    s.obj_rows += 1;
                    s.obj_max = s.obj_max.max(obj);
                    *objs.entry(obj).or_insert(0) += 1;
                }
                let both = |flat: u16, corr: u16| flat as u32 + corr as u32;
                rows.push(([both(a.atk_phys(), a.atk_phys_correction()), both(a.atk_mag(), a.atk_mag_correction()), both(a.atk_fire(), a.atk_fire_correction()), both(a.atk_thun(), a.atk_thun_correction()), both(a.atk_dark(), a.atk_dark_correction())], a.sp_attribute()));
            }
            s.obj_common = objs.iter().max_by_key(|(_, n)| **n).map_or(0, |(v, _)| *v);
            s.sp = sp_marks(rows.into_iter());
            Some(s)
        });
        let mut s = found.ok().flatten().unwrap_or_default();
        s.donors = donors;
        s
    }

    /// Once a frame: rows of idle slots go back to the game's values.
    /// EMITTERS LEFT ALIVE by bullets spawned without the one-shot flag (builds before IMPL_ER6, in
    /// this run of the game): idle one-shot emitters of OUR rows are told to stop (the byte the flag
    /// would have set); the game's own update then ends and frees them. Offsets read from the exe:
    /// pool list head at manager + 0x28, next entry + 0x998, in use + 0x328, shooting + 0x329, stop
    /// + 0x4c4, bullet row id + 0x334, timer + 0x990 (-f32::MAX once the single shot is out).
    fn sweep_emitters(&mut self) {
        // SAFETY: game thread; every entry pointer is checked; only the stop byte is written.
        let Ok(manager) = (unsafe { CSBulletManager::instance_mut() }) else { return };
        let base = manager as *const CSBulletManager as usize;
        let (mut entry, mut seen, mut stopped) = (unsafe { std::ptr::read_unaligned((base + 0x28) as *const usize) }, 0, 0);
        while game::plausible(entry) && seen < 300 {
            seen += 1;
            // SAFETY: a live entry of the pool (0x9d0 bytes each).
            unsafe {
                let byte = |at: usize| std::ptr::read_unaligned((entry + at) as *const u8);
                let row = std::ptr::read_unaligned((entry + 0x334) as *const i32);
                let timer = std::ptr::read_unaligned((entry + 0x990) as *const u32);
                if byte(0x328) == 1 && byte(0x329) == 1 && byte(0x4c4) == 0 && timer == 0xFF7F_FFFF && self.slots.iter().any(|s| s.bullet_id as i32 == row) {
                    std::ptr::write_unaligned((entry + 0x4c4) as *mut u8, 1);
                    stopped += 1;
                }
                entry = std::ptr::read_unaligned((entry + 0x998) as *const usize);
            }
        }
        if stopped > 0 {
            erlog!("bullet: {stopped} of {seen} bullet emitter(s) of this game run were left alive by an older build (no one-shot flag): told to stop; the game frees them");
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
        if !self.swept && !self.slots.is_empty() {
            self.swept = true;
            self.sweep_emitters();
        }
        let raw = self.raw.filter(|r| self.clock - r.1 < 1.5).map(|r| r.0 as i32);
        if self.log_rows && (raw.is_some() || self.slots.iter().any(|s| self.clock - s.used_at < 0.1) || (self.clock * 2.0).fract() < 0.02) {
            // (discovery, `blt.log = 1`) the game's live bullets made from our rows.
            // SAFETY: game thread; the list is the game's own, walked as the SDK walks it.
            if let Ok(manager) = unsafe { CSBulletManager::instance_mut() } {
                let mut n = 0;
                for b in manager.bullets() {
                    n += 1;
                    let ids = [b.wait_state.base.param.param_id, b.fly_state.base.param.param_id, b.exp_state.base.param.param_id];
                    if n < 200 && ids.iter().any(|id| Some(*id) == raw || self.slots.iter().any(|s| s.bullet_id as i32 == *id)) {
                        let (p, v) = (&b.physics.position, &b.physics.velocity);
                        erlog!("bullet:   live: rows {ids:?} alive {:.3} s at ({:.2}, {:.2}, {:.2}) velocity ({:.2}, {:.2}, {:.2})", b.time_alive, p.0, p.1, p.2, v.0, v.1, v.2);
                    }
                }
                // (the three pools of the manager: buffer pointer, head, free spot, allocated, allocations; then its counters)
                let base = manager as *const CSBulletManager as usize;
                // SAFETY: inside the manager object (0x280 bytes).
                let w = |at: usize| unsafe { std::ptr::read_unaligned((base + at) as *const u64) };
                let d = |at: usize| unsafe { std::ptr::read_unaligned((base + at) as *const u32) };
                let head2 = w(0x28) as usize;
                if game::plausible(head2) {
                    // SAFETY: a live entry of the pool (0x9d0 bytes).
                    let q: Vec<String> = (0..24).map(|k| format!("{:x}", unsafe { std::ptr::read_unaligned((head2 + k * 8) as *const u64) })).collect();
                    erlog!("bullet:   second pool head entry (exe base {:#x}): {}", crate::entry::exe_base(), q.join(" "));
                }
                erlog!("bullet:   {n} live bullet(s) in the game | pools: bullets head {:#x} free {:#x} allocated {} allocations {} | sfx head {:#x} free {:#x} allocated {} allocations {} | third head {:#x} free {:#x} allocated {} allocations {} | counters {} {} {} {} {} {}", w(0x08), w(0x10), d(0x18), d(0x1c), w(0x28), w(0x30), d(0x38), d(0x3c), w(0x48), w(0x50), d(0x58), d(0x5c), d(0x1cc), d(0x1d0), d(0x1d4), d(0x1d8), d(0x1dc), d(0x1e0));
            }
        }
        #[cfg(feature = "party")]
        self.restore_npc(true); // (ER party S3) stage-2 rows of idle slots back as the game had them
        #[cfg(feature = "party")]
        self.restore_owned(true); // (AGGRO) the body-owned slots' rows likewise
        if self.slots.iter().any(|s| s.patched && self.clock - s.used_at > IDLE_RESTORE) {
            let clock = self.clock;
            self.restore_where(|s| clock - s.used_at > IDLE_RESTORE);
        }
    }

    fn restore_where(&mut self, which: impl Fn(&Slot) -> bool) -> u32 {
        let mut n = 0;
        let Some(repo) = repo() else { return 0 };
        for s in self.slots.iter_mut().filter(|s| s.patched) {
            if !which(s) {
                continue;
            }
            if let Some(row) = repo.get_mut::<Bullet>(s.bullet_id) {
                *row = s.bullet_saved.clone();
            }
            if let Some(row) = repo.get_mut::<AtkParam_Pc>(s.atk_id) {
                *row = s.atk_saved.clone();
            }
            s.patched = false;
            n += 1;
        }
        n
    }

    /// Every borrowed row back as the game had it.
    pub fn restore_all(&mut self) {
        #[cfg(feature = "party")]
        self.restore_npc(false); // (ER party S3)
        #[cfg(feature = "party")]
        self.restore_owned(false); // (AGGRO)
        let n =std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.restore_where(|_| true))).unwrap_or(0);
        if n > 0 {
            erlog!("bullet: {n} borrowed row pair(s) put back as the game had them");
        }
    }

    /// One bullet of the player's at `from` (Elden Ring space) flying along `dir` (unit) with the
    /// attack `attack`. `target`: the character it is meant for (its handle goes into the spawn data).
    pub fn fire(&mut self, owner: FieldInsHandle, target: Option<FieldInsHandle>, from: Vec3, dir: Vec3, attack: Attack, shape: Shape) -> Result<u32, String> {
        if self.slots.is_empty() {
            return Err("no rows".into());
        }
        if !from.is_finite() || !dir.is_finite() || dir.length_squared() < 0.5 {
            return Err("bad position / direction".into());
        }
        // The slot longest unused; one still held by a live bullet is not taken.
        let clock = self.clock;
        let Some(k) = (0..self.slots.len()).map(|i| (self.next + i) % self.slots.len()).find(|k| clock - self.slots[*k].used_at > HOLD) else {
            self.failed += 1;
            return Err(format!("all {} row pairs are held by bullets in flight", self.slots.len()));
        };
        self.next = (k + 1) % self.slots.len();
        let repo = repo().ok_or("no param repository")?;
        let slot = &mut self.slots[k];
        {
            let Some(a) = repo.get_mut::<AtkParam_Pc>(slot.atk_id) else { return Err("attack row gone".into()) };
            *a = slot.atk_saved.clone();
            patch_attack(a, attack, shape);
        }
        {
            let Some(b) = repo.get_mut::<Bullet>(slot.bullet_id) else { return Err("bullet row gone".into()) };
            *b = slot.bullet_saved.clone();
            b.set_atk_id_bullet(slot.atk_id as i32);
            if !shape.keep_sfx {
                b.set_sfx_id_bullet(-1);
                b.set_sfx_id_hit(-1);
                b.set_sfx_id_flick(-1);
                b.set_sfx_id_force_erase(-1);
            }
            b.set_se_id_bullet1(-1);
            b.set_se_id_bullet2(-1);
            b.set_life(shape.life);
            b.set_dist(100.0);
            b.set_init_vellocity(shape.speed);
            b.set_max_vellocity(shape.speed);
            b.set_min_vellocity(shape.speed);
            b.set_accel_in_range(0.0);
            b.set_accel_out_range(0.0);
            b.set_gravity_in_range(0.0);
            b.set_gravity_out_range(0.0);
            b.set_hit_radius(shape.radius);
            b.set_hit_radius_max(-1.0);
            b.set_num_shoot(1);
            b.set_shoot_interval(0.0);
            b.set_exp_delay(0.0);
            b.set_hit_bullet_id(-1);
            b.set_interval_create_bullet_id(-1);
            b.set_sp_effect_id_for_shooter(-1);
            b.set_sp_effect_id0(-1);
            b.set_sp_effect_id1(-1);
            b.set_sp_effect_id2(-1);
            b.set_sp_effect_id3(-1);
            b.set_sp_effect_id4(-1);
            b.set_homing_angle(0);
            b.set_shoot_angle(0);
            b.set_shoot_angle_interval(0);
            b.set_shoot_angle_x_interval(0);
            b.set_damage_damp(0);
            b.set_is_penetrate_map(shape.penetrate);
            b.set_is_endless_hit(false);
            // (IMPL_ER32) the bullet row carries the same element mark as its attack row
            if let Some(sp) = attack.sp_attribute {
                b.set_sp_attribute(sp);
            }
        }
        slot.patched = true;
        slot.used_at = clock;
        if self.log_rows {
            {
                let a = &slot.atk_saved;
                erlog!("bullet: slot {k}: bullet {} > attack {} patched: {attack:?} {shape:?}; the donor row had: add_base {} hit_source {} behaviour {} / {} overwrite_element {} final_rate {} sub {} {} disable_no_damage {} material {} type {} obj {}", slot.bullet_id, slot.atk_id,
                    a.is_add_base_atk(), a.hit_source_type(), a.atk_behavior_id(), a.atk_behavior_id_2(), a.overwrite_attack_element_correct_id(), a.final_damage_rate_id(), a.sub_category1(), a.sub_category2(), a.is_disable_no_damage(), a.atk_material(), a.atk_type(), a.atk_obj());
            }
        }
        let bullet_id = slot.bullet_id;
        match spawn(owner, target, from, dir, bullet_id, shape) {
            Ok(()) => {
                self.spawned += 1;
                Ok(bullet_id)
            }
            Err(why) => {
                self.failed += 1;
                Err(why)
            }
        }
    }

    /// (discovery, `dbg.raw`) A bullet row of the game as it is, from the player.
    pub fn fire_raw(&mut self, owner: FieldInsHandle, target: Option<FieldInsHandle>, from: Vec3, dir: Vec3, bullet_id: u32) -> Result<(), String> {
        self.raw = Some((bullet_id, self.clock));
        spawn(owner, target, from, dir, bullet_id, Shape::default())
    }
}

/// (ER party S3) STAGE 2 OF THE ALLY BODIES (U14, key `party.bullet_owner`, OFF until go/no-go item 10 passes): a
/// friend's hit fired by HIS BODY (the bullet's owner = the body's handle), so that the enemy's hate goes to him. One
/// additive function; `fire` is unchanged. For an owner that is not a player the game probably reads the bullet's
/// attack row from AtkParam_Npc instead of AtkParam_Pc [research 1.3, H, medium]: the AtkParam_Npc row with THE SAME
/// ID as the slot's AtkParam_Pc row is borrowed too (saved, patched the same way, put back with the slot), so the
/// bullet carries our numbers whichever table the game reads. When AtkParam_Npc has no such row the bullet goes out
/// anyway (said once: go/no-go item 10 shows what it does). UNTESTED IN THE GAME.
#[cfg(feature = "party")]
impl Bullets {
    pub fn fire_owned(&mut self, owner: FieldInsHandle, target: Option<FieldInsHandle>, from: Vec3, dir: Vec3, attack: Attack, shape: Shape) -> Result<u32, String> {
        use eldenring::cs::AtkParam_Npc;
        // (AGGRO) the body-owned slots first (rows with an AtkParam_Npc twin); none found: the S3 route below
        if !self.owned_picked {
            self.pick_owned();
        }
        if !self.owned.is_empty() {
            return self.fire_owned_slot(owner, target, from, dir, attack, shape);
        }
        let clock = self.clock;
        // (the slot `fire` takes: the same rule)
        let k = (0..self.slots.len()).map(|i| (self.next + i) % self.slots.len().max(1)).find(|k| clock - self.slots[*k].used_at > HOLD);
        if let Some(k) = k {
            let atk_id = self.slots[k].atk_id;
            let saved = &mut self.npc_saved;
            let patched = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<bool, String> {
                let repo = repo().ok_or("no param repository")?;
                let Some(row) = repo.get_mut::<AtkParam_Npc>(atk_id) else { return Ok(false) };
                let at = match saved.iter().position(|(id, ..)| *id == atk_id) {
                    Some(at) => at,
                    None => {
                        saved.push((atk_id, row.clone(), clock, false));
                        saved.len() - 1
                    }
                };
                *row = saved[at].1.clone();
                patch_attack(row, attack, shape);
                saved[at].2 = clock;
                saved[at].3 = true;
                Ok(true)
            }));
            static TOLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            match patched {
                Ok(Ok(true)) => {}
                Ok(Ok(false)) => {
                    if !TOLD.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        erlog!("bullet: (ER party S3) stage 2: AtkParam_Npc has no row {atk_id} (the slot's attack id): a body-owned bullet carries the AtkParam_Pc row only (if the game reads the NPC table for it, it may hit with nothing: go/no-go item 10)");
                    }
                }
                Ok(Err(why)) => erlog!("bullet: (ER party S3) stage 2: the AtkParam_Npc row {atk_id} could not be borrowed ({why})"),
                Err(_) => erlog!("bullet: (ER party S3) stage 2: the SDK's param access failed for AtkParam_Npc (see the panic line)"),
            }
        }
        self.fire(owner, target, from, dir, attack, shape)
    }

    /// (AGGRO) A body-owned slot is free now (the director's / taunts' hate hits ask before they fire).
    pub fn free_owned_slot(&self) -> bool {
        if !self.owned_picked || self.owned.is_empty() {
            return self.free_slot();
        }
        self.owned.iter().any(|(s, _)| self.clock - s.used_at > HOLD)
    }

    /// (AGGRO) Chooses the body-owned rows (once), so that patching them can never change one of the game's own attacks:
    /// - the BULLET row: a plain one (the player slots' rule) that nothing fires (no BehaviorParam / BehaviorParam_PC
    ///   bullet reference, no other bullet's child): 1095 of 15475 rows in the 2.7.2.0 regulation, read offline;
    /// - the ATTACK id (written into the bullet row, as `fire` does): an AtkParam_Npc row that nothing references (no
    ///   bullet's attack id, no behaviour's attack reference): 370 of 12855 rows. The game reads AtkParam_Npc for a
    ///   bullet an NPC owns (2178 bullet attack ids have a row in both tables: the player's spells cast by NPCs); its
    ///   AtkParam_Pc row of the same id, where there is one (1 of the 370), is patched too.
    /// None of the player slots' rows; rows without visuals first.
    fn pick_owned(&mut self) {
        use eldenring::cs::{AtkParam_Npc, BehaviorParam, BehaviorParam_PC};
        self.owned_picked = true;
        let taken: Vec<(u32, u32)> = self.slots.iter().map(|s| (s.bullet_id, s.atk_id)).collect();
        let found = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let repo = repo()?;
            let mut fired = std::collections::HashSet::<i32>::new();
            let mut attacks = std::collections::HashSet::<i32>::new();
            for (_, b) in repo.rows::<BehaviorParam>().chain(repo.rows::<BehaviorParam_PC>()) {
                match b.ref_type() {
                    0 => drop(attacks.insert(b.ref_id())),
                    1 => drop(fired.insert(b.ref_id())),
                    _ => {}
                }
            }
            for (_, b) in repo.rows::<Bullet>() {
                attacks.insert(b.atk_id_bullet());
                fired.insert(b.hit_bullet_id());
                fired.insert(b.interval_create_bullet_id());
            }
            let free_atk: Vec<u32> = repo.rows::<AtkParam_Npc>().filter(|(id, a)| !attacks.contains(&(*id as i32)) && plain_attack(a) && !taken.iter().any(|(_, ta)| ta == id)).map(|(id, _)| id).take(OWNED_SLOTS).collect();
            let mut bullets: Vec<(bool, u32)> = repo.rows::<Bullet>().filter(|(id, b)| plain_bullet(b) && !fired.contains(&(*id as i32)) && !taken.iter().any(|(tb, _)| tb == id)).map(|(id, b)| (b.sfx_id_bullet() > 0 || b.sfx_id_hit() > 0, id)).collect();
            bullets.sort();
            Some(bullets.into_iter().zip(free_atk).filter_map(|((_, bullet_id), atk_id)| {
                let npc = repo.get::<AtkParam_Npc>(atk_id)?.clone();
                // (the AtkParam_Pc twin, else the NPC row stands in for the saved player row: never written then)
                let pc = repo.get::<AtkParam_Pc>(atk_id).cloned().unwrap_or_else(|| npc.clone());
                Some((Slot { bullet_id, atk_id, bullet_saved: repo.get::<Bullet>(bullet_id)?.clone(), atk_saved: pc, patched: false, used_at: -100.0 }, npc))
            }).collect::<Vec<_>>())
        }));
        match found {
            Ok(Some(owned)) if !owned.is_empty() => {
                erlog!("bullet: (AGGRO) {} row pair(s) for the ally bodies' own bullets (an unfired Bullet row > an unreferenced AtkParam_Npc row): {}",owned.len(), owned.iter().map(|(s, _)| format!("{}>{}", s.bullet_id, s.atk_id)).collect::<Vec<_>>().join(" "));
                self.owned = owned;
            }
            Ok(_) => erlog!("bullet: (AGGRO) no unused Bullet / AtkParam_Npc rows were found: body-owned bullets use the player slots (S3's route)"),
            Err(_) => erlog!("bullet: (AGGRO) the SDK's param access failed while choosing the body-owned rows (see the panic line): S3's route"),
        }
    }

    /// (AGGRO) One bullet owned by `owner` (an ally body) on a body-owned slot: both attack rows and the bullet row
    /// patched as `fire` patches the player's.
    fn fire_owned_slot(&mut self, owner: FieldInsHandle, target: Option<FieldInsHandle>, from: Vec3, dir: Vec3, attack: Attack, shape: Shape) -> Result<u32, String> {
        use eldenring::cs::AtkParam_Npc;
        if !from.is_finite() || !dir.is_finite() || dir.length_squared() < 0.5 {
            return Err("bad position / direction".into());
        }
        let clock = self.clock;
        let n = self.owned.len();
        let Some(k) = (0..n).map(|i| (self.owned_next + i) % n).find(|k| clock - self.owned[*k].0.used_at > HOLD) else {
            self.failed += 1;
            return Err(format!("all {n} body-owned row pairs are held by bullets in flight"));
        };
        self.owned_next = (k + 1) % n;
        let (slot, npc_saved) = &mut self.owned[k];
        let patched = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
            let repo = repo().ok_or("no param repository")?;
            if let Some(a) = repo.get_mut::<AtkParam_Pc>(slot.atk_id) {
                *a = slot.atk_saved.clone();
                patch_attack(a, attack, shape);
            }
            let a = repo.get_mut::<AtkParam_Npc>(slot.atk_id).ok_or("NPC attack row gone")?;
            *a = npc_saved.clone();
            patch_attack(a, attack, shape);
            let b = repo.get_mut::<Bullet>(slot.bullet_id).ok_or("bullet row gone")?;
            *b = slot.bullet_saved.clone();
            patch_bullet_row(b, slot.atk_id, attack, shape);
            Ok(())
        }));
        match patched {
            Ok(Ok(())) => {}
            Ok(Err(why)) => return Err(why),
            Err(_) => return Err("the SDK's param access failed".into()),
        }
        slot.patched = true;
        slot.used_at = clock;
        let bullet_id = slot.bullet_id;
        match spawn(owner, target, from, dir, bullet_id, shape) {
            Ok(()) => {
                self.spawned += 1;
                Ok(bullet_id)
            }
            Err(why) => {
                self.failed += 1;
                Err(why)
            }
        }
    }

    /// (AGGRO) The body-owned slots' rows back as the game had them (`idle_only`: those unused for `IDLE_RESTORE` s).
    fn restore_owned(&mut self, idle_only: bool) -> u32 {
        use eldenring::cs::AtkParam_Npc;
        let clock = self.clock;
        if !self.owned.iter().any(|(s, _)| s.patched && (!idle_only || clock - s.used_at > IDLE_RESTORE)) {
            return 0;
        }
        let owned = &mut self.owned;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let Some(repo) = repo() else { return 0 };
            let mut n = 0;
            for (s, npc) in owned.iter_mut().filter(|(s, _)| s.patched && (!idle_only || clock - s.used_at > IDLE_RESTORE)) {
                if let Some(row) = repo.get_mut::<Bullet>(s.bullet_id) {
                    *row = s.bullet_saved.clone();
                }
                if let Some(row) = repo.get_mut::<AtkParam_Pc>(s.atk_id) {
                    *row = s.atk_saved.clone();
                }
                if let Some(row) = repo.get_mut::<AtkParam_Npc>(s.atk_id) {
                    *row = npc.clone();
                }
                s.patched = false;
                n += 1;
            }
            n
        }))
        .unwrap_or(0)
    }

    /// The borrowed AtkParam_Npc rows back as the game had them (`idle_only`: those unused for `IDLE_RESTORE` s).
    fn restore_npc(&mut self, idle_only: bool) -> u32 {
        use eldenring::cs::AtkParam_Npc;
        let clock = self.clock;
        if !self.npc_saved.iter().any(|(_, _, used, patched)| *patched && (!idle_only || clock - used > IDLE_RESTORE)) {
            return 0;
        }
        let saved = &mut self.npc_saved;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let Some(repo) = repo() else { return 0 };
            let mut n = 0;
            for (id, row, used, patched) in saved.iter_mut() {
                if *patched && (!idle_only || clock - *used > IDLE_RESTORE) {
                    if let Some(live) = repo.get_mut::<AtkParam_Npc>(*id) {
                        *live = row.clone();
                    }
                    *patched = false;
                    n += 1;
                }
            }
            n
        }))
        .unwrap_or(0)
    }
}

/// (AGGRO) How many body-owned row pairs are borrowed (the ally bodies' own hits and the hate hits).
#[cfg(feature = "party")]
const OWNED_SLOTS: usize = 6;

/// (AGGRO) The bullet row of a body-owned slot, made as `fire` makes the player's (the same writes, in the same order).
#[cfg(feature = "party")]
fn patch_bullet_row(b: &mut BULLET_PARAM_ST, atk_id: u32, attack: Attack, shape: Shape) {
    b.set_atk_id_bullet(atk_id as i32);
    if !shape.keep_sfx {
        b.set_sfx_id_bullet(-1);
        b.set_sfx_id_hit(-1);
        b.set_sfx_id_flick(-1);
        b.set_sfx_id_force_erase(-1);
    }
    b.set_se_id_bullet1(-1);
    b.set_se_id_bullet2(-1);
    b.set_life(shape.life);
    b.set_dist(100.0);
    b.set_init_vellocity(shape.speed);
    b.set_max_vellocity(shape.speed);
    b.set_min_vellocity(shape.speed);
    b.set_accel_in_range(0.0);
    b.set_accel_out_range(0.0);
    b.set_gravity_in_range(0.0);
    b.set_gravity_out_range(0.0);
    b.set_hit_radius(shape.radius);
    b.set_hit_radius_max(-1.0);
    b.set_num_shoot(1);
    b.set_shoot_interval(0.0);
    b.set_exp_delay(0.0);
    b.set_hit_bullet_id(-1);
    b.set_interval_create_bullet_id(-1);
    b.set_sp_effect_id_for_shooter(-1);
    b.set_sp_effect_id0(-1);
    b.set_sp_effect_id1(-1);
    b.set_sp_effect_id2(-1);
    b.set_sp_effect_id3(-1);
    b.set_sp_effect_id4(-1);
    b.set_homing_angle(0);
    b.set_shoot_angle(0);
    b.set_shoot_angle_interval(0);
    b.set_shoot_angle_x_interval(0);
    b.set_damage_damp(0);
    b.set_is_penetrate_map(shape.penetrate);
    b.set_is_endless_hit(false);
    if let Some(sp) = attack.sp_attribute {
        b.set_sp_attribute(sp);
    }
}

/// The game's own spawn call with a block built as the module comment says.
fn spawn(owner: FieldInsHandle, target: Option<FieldInsHandle>, from: Vec3, dir: Vec3, bullet_id: u32, shape: Shape) -> Result<(), String> {
    {
        let mut data = SpawnData([0; 0x110]);
        put(&mut data, 0x00, game::handle_bits(&owner));
        put(&mut data, 0x08, -1i32);
        put(&mut data, 0x0c, shape.magic);
        put(&mut data, 0x14, bullet_id as i32);
        put(&mut data, 0x18, shape.goods);
        put(&mut data, 0x44, shape.flags);
        put(&mut data, 0x1c, -1i32);
        put(&mut data, 0x20, target.map_or(u64::MAX, |t| game::handle_bits(&t)));
        put(&mut data, 0x50, [dir.x, dir.y, dir.z, 0.0f32]);
        put(&mut data, 0x70, [dir.x, dir.y, dir.z, 0.0f32]);
        put(&mut data, 0x80, [from.x, from.y, from.z, 1.0f32]);
        put(&mut data, 0xa8, 0xffu8);
        // SAFETY: game thread; the singleton is checked; the block is the size and alignment the
        // game's routine reads (see the module comment).
        let result = unsafe {
            let Ok(manager) = CSBulletManager::instance_mut() else { return Err("no bullet manager".into()) };
            manager.spawn_bullet(&*(&data as *const SpawnData as *const eldenring::cs::BulletSpawnData))
        };
        result.map_err(|code| format!("the game refused the bullet (code {code})"))
    }
}

impl Drop for Bullets {
    fn drop(&mut self) {
        self.restore_all();
    }
}

/// THE FLAG OF THE SPAWN DATA THAT MAKES THE ATTACK ROW'S OWN NUMBER COUNT (bit 2 of the word at
/// 0x44; MEASURED: without it a player's bullet deals his weapon's attack times the row's
/// correction and the flat `atkPhys` is ignored: 0 HP at correction 0; with it `atkPhys` 100 took
/// 41 HP and 200 took 95 off the same enemy). The game's copy routine sets the same internal byte
/// when the spawn data names a magic or a goods id: "this bullet is not a weapon swing".
pub const FLAT_ATTACK: u32 = 4;

/// THE FLAG OF THE SPAWN DATA THAT ENDS THE EMITTER AFTER ITS SHOT (bit 0 of the word at 0x44; read
/// from the exe, IMPL_ER6_STATE.md: the game's copy routine 0x38e550 puts it into the emitter's
/// "stop" byte, which its update 0x3900c0 needs to end a one-shot emitter). WITHOUT IT EVERY BULLET
/// LEFT ITS EMITTER ALIVE FOR EVER: the manager's emitter pool (64 + 192) was full after 256 bullets
/// and the game refused every further one (code 4) until it was restarted.
pub const ONE_SHOT: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(a: &ATK_PARAM_ST) -> Vec<u8> {
        // SAFETY: a plain parameter row, read as bytes.
        unsafe { std::slice::from_raw_parts(a as *const ATK_PARAM_ST as *const u8, std::mem::size_of::<ATK_PARAM_ST>()) }.to_vec()
    }

    fn donor() -> ATK_PARAM_ST {
        // SAFETY: a parameter row is plain numbers; all zero is a valid one.
        let mut a: ATK_PARAM_ST = unsafe { std::mem::zeroed() };
        a.set_atk_obj(37);
        a.set_sp_attribute(9);
        a.set_atk_phys(500);
        a.set_atk_thun(11);
        a
    }

    /// (IMPL_ER32) `break.kh = 0` = `Attack::PLAIN`'s three fields: the row of every build before (the number on the type
    /// `atk.element` names, lightning and holy zero, the shape's `spAttribute`, the donor's `atkObj`).
    #[test]
    fn a_plain_attack_patches_the_row_as_before() {
        let attack = Attack { phys: 321, poise: 12.0, stamina: 160, level: 3, ..Attack::PLAIN };
        for (element, want) in [(0u8, [321u16, 0, 0]), (1, [0, 321, 0]), (2, [0, 0, 321]), (3, [0, 0, 0])] {
            let mut a = donor();
            patch_attack(&mut a, attack, Shape { element, ..Shape::default() });
            assert_eq!([a.atk_phys(), a.atk_mag(), a.atk_fire()], want);
            assert_eq!((a.atk_thun(), a.atk_dark(), a.atk_obj(), a.sp_attribute(), a.dmg_level(), a.atk_stam()), (0, 0, 37, 0, 3, 160));
        }
    }

    /// (IMPL_ER32) An element moves the WHOLE number to one damage type (the total is the attack value asked for), marks
    /// the row, and the object value is set; the saved donor row put back is the game's row again, byte for byte.
    #[test]
    fn an_element_carries_the_whole_number_and_the_row_is_restored() {
        let saved = donor();
        for (carrier, at) in [(Carrier::Physical, 0), (Carrier::Magic, 1), (Carrier::Fire, 2), (Carrier::Lightning, 3), (Carrier::Holy, 4)] {
            let attack = Attack { phys: 777, carrier, sp_attribute: Some(2), obj: Some(450), ..Attack::PLAIN };
            let mut a = saved.clone();
            // (whatever `atk.element` says: an explicit carrier overrules it)
            patch_attack(&mut a, attack, Shape { element: 1, ..Shape::default() });
            let parts = [a.atk_phys(), a.atk_mag(), a.atk_fire(), a.atk_thun(), a.atk_dark()];
            assert_eq!(parts.iter().map(|v| *v as u32).sum::<u32>(), 777, "{carrier:?}: the total is unchanged");
            assert_eq!(parts[at], 777, "{carrier:?}");
            assert_eq!(attack.split(1), parts);
            assert_eq!((a.atk_obj(), a.sp_attribute()), (450, 2));
            assert_ne!(bytes(&a), bytes(&saved));
            a = saved.clone();
            assert_eq!(bytes(&a), bytes(&saved), "restored exactly");
        }
        assert_eq!(Carrier::from_code(2), Carrier::Fire);
        assert_eq!(Carrier::from_code(0), Carrier::Physical);
        assert_eq!(Carrier::from_code(200), Carrier::Physical);
    }

    /// (IMPL_ER32) The element marks are read off the game's own rows: only rows with ONE elemental damage type count.
    #[test]
    fn the_element_marks_come_from_pure_rows() {
        let rows: Vec<ElementRow> = vec![
            ([100, 100, 100, 100, 100], 7), // a weapon swing: every type scaled, no evidence
            ([0, 0, 100, 0, 0], 2),
            ([0, 0, 80, 0, 0], 2),
            ([0, 0, 80, 0, 0], 0), // an unmarked fire row
            ([0, 0, 80, 0, 0], 5),
            ([0, 120, 0, 0, 0], 3),
            ([50, 0, 100, 0, 0], 4), // physical + fire: not pure
            ([0, 0, 0, 90, 0], 0),
        ];
        let marks = sp_marks(rows.into_iter());
        assert_eq!(marks[2], Some((2, 2, 4)));
        assert_eq!(marks[1], Some((3, 1, 1)));
        assert_eq!(marks[3], None, "lightning rows exist but none is marked");
        assert_eq!((marks[0], marks[4]), (None, None));
        let survey = Survey { sp: marks, ..Survey::default() };
        assert_eq!((survey.sp_of(Carrier::Fire), survey.sp_of(Carrier::Magic), survey.sp_of(Carrier::Lightning), survey.sp_of(Carrier::Physical), survey.sp_of(Carrier::Shape)), (Some(2), Some(3), None, None, None));
    }
}
