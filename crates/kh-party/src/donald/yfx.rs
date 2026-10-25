//! (YFX) DONALD'S Y ARCANE MAGIC, ITS LOOK AND SOUND (the user, 2026-10-23: "ALL of the Y effects need unique visuals, they
//! look EXTREMELY plain. FIX THAT"; "gravity needs to look better and have unique gravity sound effects"). The rules (damage,
//! area, statuses, the hit times) stay in `cast.rs` `arcane`; this module only shows and sounds them, on the same clock:
//! - COMET: a star twinkles high over the target, then streaks down on a diagonal (a white-blue star, a sparkle trail) and
//!   bursts in a star blast (flash, shock rings, a fountain of stardust, a glowing crater); Comet II drops a second, bigger
//!   star from the other side. KH3's own Comet sounds (Donald's comet team attack: the summon, the dive, the impact).
//! - METEOR: a burning rift opens in the sky, warning glows mark each landing spot, and burning rocks (KH3's comet fire head
//!   round a rock, smoke and embers trailing) rain on them in a shower from one side; each lands in KH3's comet explosion
//!   (fire, flying rubble, smoke) with a scorch crater, a dust ring and a shake. More, bigger rocks each level; Lv 3 ends on
//!   a huge one. Stitch's meteor call, the gummi meteor hits and rock rubble.
//! - ULTIMA: light streams in from all round into a growing core over the target, then a 10 m sphere of pure magic bursts
//!   out of it (flash, rings, rays, thrown rock, a huge shake), holds and breaks into motes. Aqua's ultima cannon call and
//!   finisher with the gummi Ultima hit.
//! - GRAVITY: its ball (vd_don_grv1-3, `make_gravity.js`) scaled up to fit a tall target, and its own layered sounds: a deep
//!   hole hum as it forms, crackles on the band, a press as it squeezes, an implosion thud at the bottom.
//! Every effect is a KH3 effect or authored from KH3 emitters (YFX tools/make_yfx.js); every sound is a KH3 sound played by
//! its asset path (the party audio root, YFX pack MERGE_NOTE).

use super::magic_fx::{Ctx, MagicFx};
use super::spells::SpellRef;
use crate::ids::MemberId;
use glam::Vec3;
use kh_core::host::{Attach, EffectHandle, EffectId, EffectSpawn, SoundHandle, SoundId, SoundPlay, Tint};

// ----------------------------------------------------------------------------------------------------- the effects
pub const COMET_SKY: &str = "vd_yfx_comet_sky";
pub const COMET_MARK: &str = "vd_yfx_comet_mark";
pub const COMET_STAR: &str = "vd_yfx_comet_star";
pub const COMET_HIT: &str = "vd_yfx_comet_hit";
pub const COMET_AFTER: &str = "vd_yfx_comet_after";
pub const METEOR_SKY: &str = "vd_yfx_meteor_sky";
pub const METEOR_MARK: &str = "vd_yfx_meteor_mark";
pub const METEOR_ROCK: &str = "vd_yfx_meteor_rock";
/// KH3's comet body (Donald's comet team attack: the fire head, its tail), round the rock.
pub const METEOR_BODY: &str = "vn_ex001_fcm_emi00";
/// KH3's comet explosion (Donald's comet team attack): fire, flying rubble, smoke, its crater decal.
pub const METEOR_BLAST: &str = "vn_ex001_fcm_hit00";
pub const METEOR_HIT: &str = "vd_yfx_meteor_hit";
/// KH3's Sora shot burst (vp_vld_sho00: a flash, a ring burst, sparks, a ring decal) and the light pillar falling on a spot
/// (vp_flh_sho00), the star blast's KH3 core; the KH3 Ultima dome (the Keyblade Graveyard's vd_kg878_c36_ult_dom00) and a
/// ring burst (vw_lmt_lac_hit00), Ultima's.
pub const COMET_BURST: &str = "vp_vld_sho00";
pub const COMET_PILLAR: &str = "vp_flh_sho00";
pub const ULTIMA_DOME: &str = "vd_kg878_c36_ult_dom00";
pub const ULTIMA_RING: &str = "vw_lmt_lac_hit00";
/// The star's gold-white, Ultima's pure teal-white (KH3 effects recoloured at the spawn).
pub const STAR_TINT: Tint = Tint { color: [1.0, 0.88, 0.62], intensity: 1.25 };
/// Gravity's violet (the crush's KH3 vortex decal is red: recoloured by its brightness).
pub const GRAVITY_TINT: Tint = Tint { color: [0.72, 0.32, 1.0], intensity: 1.3 };
pub const ULTIMA_TINT: Tint = Tint { color: [0.72, 1.0, 0.92], intensity: 1.35 };
pub const ULTIMA_GATHER: &str = "vd_yfx_ult_gather";
pub const ULTIMA_BURST: &str = "vd_yfx_ult_burst";
pub const ULTIMA_AFTER: &str = "vd_yfx_ult_after";
/// Gravity's crush at the bottom of the press: KH3's dark hole and cracked-ground decals in violet, a shake.
pub const GRAVITY_CRUSH: &str = "vd_yfx_grv_crush";

// ------------------------------------------------------------------------------------------------------ the sounds
// (KH3 sound assets, played by path; packed by YFX tools/yfx_audio.js into the party audio root)
/// Donald's comet team attack: the summon (vn_ex001_fcm_spn00), the dive (fcm_flt00), the impact (fcm_hit00).
pub const SE_COMET_SUMMON: &str = "/Game/Sound/SE/NPC/00301_n_ex001/se00301_055";
pub const SE_COMET_DIVE: &str = "/Game/Sound/SE/NPC/00301_n_ex001/se00301_059";
pub const SE_COMET_IMPACT: &str = "/Game/Sound/SE/NPC/00301_n_ex001/se00301_054";
/// Stitch's star point (vm_star_point0): the twinkle.
pub const SE_COMET_TWINKLE: &str = "/Game/Sound/SE/link/stitch/se00005_408";
/// A heavy boom under the star blast (e_bx901's jet impact).
pub const SE_COMET_BOOM: &str = "/Game/Sound/SE/enemy/00510_e_bx901/se00510_059";
/// Stitch's meteor call (vm_inseki_op), its finisher streaks (vm_stch_fns0 / 1).
pub const SE_METEOR_CALL: &str = "/Game/Sound/SE/link/stitch/se00005_414";
pub const SE_METEOR_STREAK: [&str; 2] = ["/Game/Sound/SE/link/stitch/se00005_412", "/Game/Sound/SE/link/stitch/se00005_413"];
/// The rock titan's rock call: the rumble of the rift.
pub const SE_METEOR_RUMBLE: &str = "/Game/Sound/SE/enemy/00531_e_he001/se00531_109";
/// The gummi ship's meteor hit (vt_gm_met_hit01), the rock titan's rock hit, Simba's big boom (vm_fns_bom0), the rubble.
pub const SE_METEOR_HIT: &str = "/Game/Sound/SE/gumi/00801/se00801_102";
pub const SE_METEOR_ROCK: &str = "/Game/Sound/SE/enemy/00531_e_he001/se00531_112";
pub const SE_METEOR_BIG: &str = "/Game/Sound/SE/link/simba/se00005_101";
pub const SE_METEOR_RUBBLE: &str = "/Game/Sound/SE/world/00100_dw/se00100_042";
/// Aqua's ultima cannon: the call (charge), the cinematic swell, the hit, the finisher's tail; the gummi Ultima hit.
pub const SE_ULTIMA_CHARGE: &str = "/Game/Sound/SE/NPC/00318_n_ex011/se00318_038";
pub const SE_ULTIMA_SWELL: &str = "/Game/Sound/SE/NPC/00318_n_ex011/se00318_036";
pub const SE_ULTIMA_HIT: &str = "/Game/Sound/SE/NPC/00318_n_ex011/se00318_041";
pub const SE_ULTIMA_TAIL: &str = "/Game/Sound/SE/NPC/00318_n_ex011/se00318_040";
pub const SE_ULTIMA_BLAST: &str = "/Game/Sound/SE/gumi/00801/se00801_103";
/// GRAVITY: the dark hole's hum (e_dw401 hol0), the electric box crackle, the thunder-status crackle, the press (e_ex011
/// prs0), the gummi gravity bomb (pitched down), the bomb implosion (e_ex721 bom_imp00).
pub const SE_GRAVITY_HUM: &str = "/Game/Sound/SE/enemy/00501_e_dw401/se00501_018";
pub const SE_GRAVITY_CRACKLE: &str = "/Game/Sound/SE/gimmick/mi/se_g_mi_electricbox";
pub const SE_GRAVITY_CRACKLE2: &str = "/Game/Sound/SE/common/00002_magic/se00002_208";
pub const SE_GRAVITY_PRESS: &str = "/Game/Sound/SE/enemy/00457_e_ex011/se00457_026";
pub const SE_GRAVITY_BOMB: &str = "/Game/Sound/SE/gumi/00801/se00801_109";
pub const SE_GRAVITY_IMPLODE: &str = "/Game/Sound/SE/enemy/00504_e_ex721/se00504_054";

/// Every sound asset the Y spells play (the audio pack and its harness).
pub const SOUNDS: [&str; 24] = [
    SE_COMET_SUMMON, SE_COMET_DIVE, SE_COMET_IMPACT, SE_COMET_TWINKLE, SE_COMET_BOOM, SE_METEOR_CALL, SE_METEOR_STREAK[0], SE_METEOR_STREAK[1], SE_METEOR_RUMBLE, SE_METEOR_HIT,
    SE_METEOR_ROCK, SE_METEOR_BIG, SE_METEOR_RUBBLE, SE_ULTIMA_CHARGE, SE_ULTIMA_SWELL, SE_ULTIMA_HIT, SE_ULTIMA_TAIL, SE_ULTIMA_BLAST, SE_GRAVITY_HUM, SE_GRAVITY_CRACKLE,
    SE_GRAVITY_CRACKLE2, SE_GRAVITY_PRESS, SE_GRAVITY_BOMB, SE_GRAVITY_IMPLODE,
];

// ------------------------------------------------------------------------------------------------------ the queue
/// What lands with a falling body (spawned where it arrives).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub fx: [Option<(&'static str, f32, Option<Tint>)>; 4],
    pub se: [Option<(&'static str, f32, f32)>; 3],
    /// An aftermath effect this long after the landing (s).
    pub after: Option<(&'static str, f32, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    /// An effect at a point (`yaw` deg about the vertical).
    Fx { name: &'static str, at: Vec3, scale: f32, yaw: f32, tint: Option<Tint> },
    /// A body flying from `from` down to `lift` cm over the ground point `to` in `time` s (moved every step), then its
    /// landing at `to`.
    Fall { names: [Option<&'static str>; 2], from: Vec3, to: Vec3, lift: f32, time: f32, scale: f32, land: Landing },
    /// A KH3 sound by its asset path at `at`; stopped (faded) after `stop` s when that is finite.
    Se { asset: &'static str, at: Vec3, volume: f32, pitch: f32, stop: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timed {
    pub left: f32,
    pub caster: MemberId,
    pub spell: SpellRef,
    pub cue: Cue,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Flying {
    caster: MemberId,
    spell: SpellRef,
    handles: [Option<EffectHandle>; 2],
    from: Vec3,
    to: Vec3,
    lift: f32,
    t: f32,
    time: f32,
    scale: f32,
    land: Landing,
}

/// The Y spells' presentation state (`MagicFx::yfx`).
#[derive(Clone, Debug, Default)]
pub struct YState {
    pub timed: Vec<Timed>,
    flying: Vec<Flying>,
    playing: Vec<(SoundHandle, f32)>,
    /// TEST: every cue fired (name or asset), in order.
    pub fired: Vec<&'static str>,
}

fn fx(st: &mut YState, caster: MemberId, spell: SpellRef, delay: f32, name: &'static str, at: Vec3, scale: f32) {
    st.timed.push(Timed { left: delay, caster, spell, cue: Cue::Fx { name, at, scale, yaw: 0.0, tint: None } });
}

fn se(st: &mut YState, caster: MemberId, spell: SpellRef, delay: f32, asset: &'static str, at: Vec3, volume: f32, pitch: f32, stop: f32) {
    st.timed.push(Timed { left: delay, caster, spell, cue: Cue::Se { asset, at, volume, pitch, stop } });
}

/// A deterministic jitter in -1..1 for `k`.
fn jit(k: usize, salt: u32) -> f32 {
    let mut x = (k as u32).wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x % 2001) as f32 / 1000.0 - 1.0
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, 0.0).normalize_or_zero()
}

// ------------------------------------------------------------------------------------------------------ COMET
/// COMET (Lv 1 one star, Comet II two): `count` stars, `gap` s apart, each falling `fall` s from `height` cm onto `ground`
/// (the same clock as `cast.rs`'s delayed hits); Donald at `me` facing `yaw` (deg).
#[allow(clippy::too_many_arguments)]
pub fn comet(fxw: &mut MagicFx, caster: MemberId, spell: SpellRef, lv: u8, ground: Vec3, me: Vec3, yaw: f32, count: u32, gap: f32, fall: f32, height: f32) {
    let st = &mut fxw.yfx;
    let fwd = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
    let side = Vec3::new(-fwd.y, fwd.x, 0.0);
    let n = count.max(1) as usize;
    // the marker under the target for the whole volley; the summon
    fx(st, caster, spell, 0.0, COMET_MARK, ground, 1.0 + 0.15 * (lv as f32 - 1.0));
    se(st, caster, spell, 0.0, SE_COMET_SUMMON, me, 1.0, 1.0, f32::INFINITY);
    for k in 0..n {
        let last = k + 1 == n;
        let d = k as f32 * gap;
        // from behind Donald and to one side (the second star from the other side), steep
        let s = if k % 2 == 0 { 1.0 } else { -1.0 };
        let off = flat(-fwd * 0.55 + side * 0.85 * s) * (height * 0.62);
        let from = ground + Vec3::Z * height + off;
        let scale = if n > 1 && last { 1.3 } else { 1.0 } * (1.0 + 0.1 * (lv as f32 - 1.0));
        fx(st, caster, spell, d, COMET_SKY, from, scale);
        se(st, caster, spell, d, SE_COMET_TWINKLE, ground, 0.9, 1.15, f32::INFINITY);
        se(st, caster, spell, d, SE_COMET_DIVE, ground, 0.9, 1.3, fall + 0.12);
        let land = Landing {
            fx: [Some((COMET_HIT, scale, None)), Some((COMET_BURST, 1.1 * scale, Some(STAR_TINT))), Some((COMET_PILLAR, 1.2 * scale, Some(STAR_TINT))), None],
            se: [Some((SE_COMET_IMPACT, 1.0, 1.0)), Some((SE_COMET_BOOM, 0.75, if last { 0.8 } else { 0.9 })), Some((SE_COMET_TWINKLE, 0.7, 1.45))],
            after: Some((COMET_AFTER, 0.25, scale)),
        };
        st.timed.push(Timed { left: d, caster, spell, cue: Cue::Fall { names: [Some(COMET_STAR), None], from, to: ground, lift: 40.0, time: fall, scale, land } });
    }
}

// ------------------------------------------------------------------------------------------------------ METEOR
/// Where the meteors of a volley land: a sunflower over the radius (deterministic), the first on the target.
pub fn meteor_spots(ground: Vec3, n: u32, radius: f32) -> Vec<Vec3> {
    let n = n.max(1);
    (0..n)
        .map(|k| {
            let r = radius * ((k as f32 + 0.5) / n as f32).sqrt();
            let a = k as f32 * 2.399_963;
            ground + Vec3::new(a.cos() * r, a.sin() * r, 0.0)
        })
        .collect()
}

/// METEOR (Lv 1 3 rocks over 6 m, Lv 2 6 over 9 m, Lv 3 10 over 12 m): `spots` (from [`meteor_spots`]), the k-th falling at
/// `k * gap` for `fall` s from `height` cm (the same clock as `cast.rs`'s delayed hits).
#[allow(clippy::too_many_arguments)]
pub fn meteor(fxw: &mut MagicFx, caster: MemberId, spell: SpellRef, lv: u8, ground: Vec3, me: Vec3, yaw: f32, spots: &[Vec3], gap: f32, fall: f32, height: f32) {
    let st = &mut fxw.yfx;
    let fwd = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
    let side = Vec3::new(-fwd.y, fwd.x, 0.0);
    let lvf = lv.clamp(1, 3) as f32;
    // the shower comes from one side of the sky (behind Donald's right), every rock on a near-parallel line
    let dir = flat(-fwd * 0.4 + side * 0.9);
    let n = spots.len().max(1);
    let total = (n - 1) as f32 * gap + fall;
    // the rift in the sky over the field, open for the whole volley; its call and rumble
    fx(st, caster, spell, 0.0, METEOR_SKY, ground + Vec3::Z * height + dir * height * 0.55, 0.8 + 0.25 * lvf);
    se(st, caster, spell, 0.0, SE_METEOR_CALL, me.lerp(ground, 0.5), 1.0, 0.95, f32::INFINITY);
    se(st, caster, spell, 0.1, SE_METEOR_RUMBLE, ground, 0.8, 0.8, total + 0.6);
    for (k, at) in spots.iter().copied().enumerate() {
        let d = k as f32 * gap;
        let last = k + 1 == n;
        let big = lv >= 3 && last;
        let scale = if big { 2.0 } else { (0.85 + 0.15 * lvf) * (1.0 + 0.12 * jit(k, 7)) };
        let wobble = side * (90.0 * jit(k, 3)) + fwd * (90.0 * jit(k, 5));
        let from = at + Vec3::Z * height + dir * (height * 0.55) + wobble;
        // the warning glow where it will land, appearing as it starts to fall
        fx(st, caster, spell, d, METEOR_MARK, at, scale.min(1.6));
        se(st, caster, spell, d, SE_METEOR_STREAK[k % 2], at, 0.55 + 0.1 * scale, 0.85 + 0.1 * jit(k, 9), f32::INFINITY);
        let land = Landing {
            fx: [Some((METEOR_BLAST, 0.65 + 0.3 * scale, None)), Some((METEOR_HIT, scale, None)), None, None],
            se: [Some((SE_METEOR_HIT, 0.85 + 0.1 * scale, 0.92 + 0.08 * jit(k, 11))), if k % 2 == 0 || big { Some((SE_METEOR_ROCK, 0.6, 1.0)) } else { None }, if big { Some((SE_METEOR_BIG, 1.0, 1.0)) } else { None }],
            after: None,
        };
        st.timed.push(Timed { left: d, caster, spell, cue: Cue::Fall { names: [Some(METEOR_BODY), Some(METEOR_ROCK)], from, to: at, lift: 30.0, time: fall, scale, land } });
    }
    se(st, caster, spell, total + 0.3, SE_METEOR_RUBBLE, ground, 0.6, 1.0, f32::INFINITY);
}

// ------------------------------------------------------------------------------------------------------ ULTIMA
/// ULTIMA: the gathering at the release, the sphere bursting `delay` s later (the same clock as `cast.rs`'s hit).
pub fn ultima(fxw: &mut MagicFx, caster: MemberId, spell: SpellRef, ground: Vec3, delay: f32) {
    let st = &mut fxw.yfx;
    fx(st, caster, spell, 0.0, ULTIMA_GATHER, ground, 1.0);
    se(st, caster, spell, 0.0, SE_ULTIMA_CHARGE, ground, 1.0, 1.0, delay + 0.25);
    se(st, caster, spell, 0.0, SE_ULTIMA_SWELL, ground, 0.6, 1.0, delay + 0.1);
    fx(st, caster, spell, delay, ULTIMA_BURST, ground, 1.0);
    st.timed.push(Timed { left: delay, caster, spell, cue: Cue::Fx { name: ULTIMA_DOME, at: ground, scale: 1.3, yaw: 0.0, tint: None } });
    st.timed.push(Timed { left: delay, caster, spell, cue: Cue::Fx { name: ULTIMA_RING, at: ground, scale: 2.6, yaw: 0.0, tint: Some(ULTIMA_TINT) } });
    st.timed.push(Timed { left: delay + 0.05, caster, spell, cue: Cue::Fx { name: COMET_BURST, at: ground, scale: 2.2, yaw: 0.0, tint: Some(ULTIMA_TINT) } });
    se(st, caster, spell, delay, SE_ULTIMA_BLAST, ground, 1.0, 0.85, f32::INFINITY);
    se(st, caster, spell, delay, SE_ULTIMA_HIT, ground, 0.9, 0.9, f32::INFINITY);
    se(st, caster, spell, delay + 0.05, SE_ULTIMA_TAIL, ground, 0.8, 1.0, f32::INFINITY);
    fx(st, caster, spell, delay + 1.3, ULTIMA_AFTER, ground, 1.0);
}

// ------------------------------------------------------------------------------------------------------ GRAVITY
/// GRAVITY'S ball scale: the tier's own size, made bigger only when the target is too tall for it (any enemy, from its
/// run-time capsule height `height` cm), at most x3.
pub fn gravity_scale(lv: u8, height: f32) -> f32 {
    let m = super::magic_fx::magic();
    let d = 100.0
        * match lv {
            0 | 1 => m.gravity_fx_diameter_1,
            2 => m.gravity_fx_diameter_2,
            _ => m.gravity_fx_diameter_3,
        };
    (height * 1.12 / d.max(1.0)).clamp(1.0, 3.0)
}

/// GRAVITY'S SOUNDS on the ball's clock (`gravity.*` of party-don-magic): the hum as it forms, the band's crackles, the
/// press as it squeezes, the implosion as it bottoms out.
pub fn gravity(fxw: &mut MagicFx, caster: MemberId, spell: SpellRef, lv: u8, ground: Vec3, scale: f32) {
    let m = super::magic_fx::magic();
    let st = &mut fxw.yfx;
    let deep = 1.0 - 0.06 * (lv.clamp(1, 3) as f32 - 1.0);
    let at = ground + Vec3::Z * 120.0;
    se(st, caster, spell, 0.0, SE_GRAVITY_HUM, at, 0.9, 0.78 * deep, m.gravity_fx_total + 0.1);
    se(st, caster, spell, m.gravity_fx_form, SE_GRAVITY_CRACKLE, at, 0.55, 0.9, f32::INFINITY);
    se(st, caster, spell, m.gravity_fx_form + 0.35, SE_GRAVITY_CRACKLE2, at, 0.5, 0.8, f32::INFINITY);
    se(st, caster, spell, m.gravity_press_at, SE_GRAVITY_PRESS, at, 1.0, 0.8 * deep, f32::INFINITY);
    se(st, caster, spell, m.gravity_press_at + 0.05, SE_GRAVITY_BOMB, at, 0.6, 0.68 * deep, f32::INFINITY);
    se(st, caster, spell, m.gravity_press_at + m.gravity_press_time * 0.9, SE_GRAVITY_IMPLODE, ground, 1.0, 0.85 * deep, f32::INFINITY);
    // the crater, the cracks and the shake as it bottoms out
    let tint = Some(GRAVITY_TINT);
    st.timed.push(Timed { left: m.gravity_press_at + m.gravity_press_time * 0.85, caster, spell, cue: Cue::Fx { name: GRAVITY_CRUSH, at: ground, scale: scale * (0.42 + 0.06 * lv.clamp(1, 3) as f32), yaw: 0.0, tint } });
}

/// The effects whose sound is this module's layered cues rather than one SE of their own (the pack harness).
pub fn own_sounds(effect: &str) -> Option<&'static [&'static str]> {
    const GRAVITY: [&str; 6] = [SE_GRAVITY_HUM, SE_GRAVITY_CRACKLE, SE_GRAVITY_CRACKLE2, SE_GRAVITY_PRESS, SE_GRAVITY_BOMB, SE_GRAVITY_IMPLODE];
    matches!(effect, "vd_don_grv1" | "vd_don_grv2" | "vd_don_grv3").then_some(&GRAVITY[..])
}

// ------------------------------------------------------------------------------------------------------ the step
fn spawn(cx: &mut Ctx, caster: MemberId, spell: SpellRef, name: &'static str, at: Vec3, yaw: f32, scale: f32, persistent: bool, tint: Option<Tint>) -> Option<EffectHandle> {
    let m = cx.party.member_mut(caster)?;
    let h = m.seam.spawn(cx.host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), rotation: Vec3::new(0.0, yaw, 0.0), scale, persistent });
    if let Some(t) = tint {
        cx.host.effects.tint(h, t);
    }
    if let Some(s) = super::magic_fx::fx::extra_se(name) {
        m.seam.play(cx.host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(s)), name: s, at: Attach::World(at), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
    }
    let _ = spell;
    Some(h)
}

fn play(cx: &mut Ctx, caster: MemberId, asset: &'static str, at: Vec3, volume: f32, pitch: f32) -> Option<SoundHandle> {
    let m = cx.party.member_mut(caster)?;
    Some(m.seam.play(cx.host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(asset)), name: asset, at: Attach::World(at), volume, pitch, looping: false, spatial: true }))
}

/// Every step: the cues whose time has come, the bodies in flight (and their landings), the sounds to fade.
pub fn step(fxw: &mut MagicFx, cx: &mut Ctx, dt: f32) {
    let st = &mut fxw.yfx;
    if st.timed.is_empty() && st.flying.is_empty() && st.playing.is_empty() {
        return;
    }
    let mut due = Vec::new();
    for t in st.timed.iter_mut() {
        t.left -= dt;
        if t.left <= 0.0 {
            due.push(*t);
        }
    }
    st.timed.retain(|t| t.left > 0.0);
    for t in due {
        match t.cue {
            Cue::Fx { name, at, scale, yaw, tint } => {
                st.fired.push(name);
                let _ = spawn(cx, t.caster, t.spell, name, at, yaw, scale, false, tint);
            }
            Cue::Se { asset, at, volume, pitch, stop } => {
                st.fired.push(asset);
                if let Some(h) = play(cx, t.caster, asset, at, volume, pitch) {
                    if stop.is_finite() {
                        st.playing.push((h, stop));
                    }
                }
            }
            Cue::Fall { names, from, to, lift, time, scale, land } => {
                // its heading: the yaw of its flight
                let dir = to - from;
                let yaw = dir.y.atan2(dir.x).to_degrees();
                let mut handles = [None; 2];
                for (k, name) in names.iter().enumerate() {
                    if let Some(name) = *name {
                        st.fired.push(name);
                        handles[k] = spawn(cx, t.caster, t.spell, name, from, yaw, scale, true, None);
                    }
                }
                st.flying.push(Flying { caster: t.caster, spell: t.spell, handles, from, to, lift, t: -t.left.min(0.0), time: time.max(0.05), scale, land });
            }
        }
    }
    // the bodies in flight: a straight dive, a little faster at the end; at the end the body stops (its trail fades) and
    // its landing is spawned where it arrived
    let mut landed = Vec::new();
    for f in st.flying.iter_mut() {
        f.t += dt;
        let u = (f.t / f.time).clamp(0.0, 1.0);
        let k = u * (0.75 + 0.25 * u);
        let at = f.from.lerp(f.to + Vec3::Z * f.lift, k);
        let dir = f.to - f.from;
        for h in f.handles.iter().flatten() {
            cx.host.effects.transform(*h, at, Vec3::new(0.0, dir.y.atan2(dir.x).to_degrees(), 0.0), Vec3::splat(f.scale));
            if u >= 1.0 {
                cx.host.effects.stop(*h, false);
            }
        }
        if u >= 1.0 {
            landed.push(*f);
        }
    }
    st.flying.retain(|f| f.t < f.time);
    for f in landed {
        let ground = f.to;
        for (name, scale, tint) in f.land.fx.iter().flatten().copied() {
            st.fired.push(name);
            let _ = spawn(cx, f.caster, f.spell, name, ground, 0.0, scale, false, tint);
        }
        for (asset, volume, pitch) in f.land.se.iter().flatten().copied() {
            st.fired.push(asset);
            let _ = play(cx, f.caster, asset, ground, volume, pitch);
        }
        if let Some((name, delay, scale)) = f.land.after {
            st.timed.push(Timed { left: delay, caster: f.caster, spell: f.spell, cue: Cue::Fx { name, at: ground, scale, yaw: 0.0, tint: None } });
        }
    }
    // the sounds cut short: faded out at their time
    for p in st.playing.iter_mut() {
        p.1 -= dt;
        if p.1 <= 0.0 {
            cx.host.audio.stop(p.0, 0.2);
        }
    }
    st.playing.retain(|p| p.1 > 0.0);
    if st.fired.len() > 256 {
        st.fired.drain(..128);
    }
}

/// Every effect the Y spells spawn here (the pack harness checks each one).
pub fn all_names() -> Vec<&'static str> {
    vec![GRAVITY_CRUSH, COMET_SKY, COMET_MARK, COMET_STAR, COMET_HIT, COMET_AFTER, COMET_BURST, COMET_PILLAR, METEOR_SKY, METEOR_MARK, METEOR_ROCK, METEOR_BODY, METEOR_BLAST, METEOR_HIT, ULTIMA_GATHER, ULTIMA_BURST, ULTIMA_AFTER, ULTIMA_DOME, ULTIMA_RING]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::donald::spells::ArcaneLine;

    fn cues(f: impl FnOnce(&mut MagicFx)) -> Vec<Timed> {
        let mut fxw = MagicFx::default();
        f(&mut fxw);
        fxw.yfx.timed.clone()
    }

    fn sounds(t: &[Timed]) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = t.iter().filter_map(|c| if let Cue::Se { asset, .. } = c.cue { Some(asset) } else { None }).collect();
        for c in t {
            if let Cue::Fall { land, .. } = c.cue {
                v.extend(land.se.iter().flatten().map(|s| s.0));
            }
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    /// (YFX) the bodies land exactly when cast.rs's hits land (the same sheet numbers), one per star / rock.
    #[test]
    fn falling_bodies_land_on_the_hits_clock() {
        let m = crate::donald::magic_fx::magic();
        let comet = cues(|f| comet(f, MemberId(0), SpellRef::Arcane(ArcaneLine::Comet, 2), 2, Vec3::ZERO, Vec3::new(-600.0, 0.0, 0.0), 0.0, 2, m.comet_gap, m.comet_fall, m.comet_height));
        let lands: Vec<f32> = comet.iter().filter_map(|c| if let Cue::Fall { time, .. } = c.cue { Some(c.left + time) } else { None }).collect();
        assert_eq!(lands.len(), 2);
        for (k, t) in lands.iter().enumerate() {
            assert!((t - (m.comet_fall + k as f32 * m.comet_gap)).abs() < 1e-5, "star {k} lands at {t}");
        }
        for (lv, n) in [(1u8, 3u32), (2, 6), (3, 10)] {
            let spots = meteor_spots(Vec3::ZERO, n, 600.0);
            let rain = cues(|f| meteor(f, MemberId(0), SpellRef::Arcane(ArcaneLine::Meteor, lv), lv, Vec3::ZERO, Vec3::new(-600.0, 0.0, 0.0), 0.0, &spots, m.meteor_gap, m.meteor_fall, m.meteor_height));
            let falls: Vec<(f32, Vec3)> = rain.iter().filter_map(|c| if let Cue::Fall { time, to, .. } = c.cue { Some((c.left + time, to)) } else { None }).collect();
            assert_eq!(falls.len(), n as usize, "Meteor Lv {lv}");
            for (k, (t, to)) in falls.iter().enumerate() {
                assert!((t - (k as f32 * m.meteor_gap + m.meteor_fall)).abs() < 1e-5);
                assert!(to.distance(spots[k]) < 1e-3, "it lands where its hit is");
            }
        }
    }

    /// (YFX, the user: "unique gravity sound effects", "its own sound design ... never a generic reuse") every Y spell has
    /// layered sounds of its own: no sound asset is shared between two of them.
    #[test]
    fn every_y_spell_has_its_own_sounds() {
        let m = crate::donald::magic_fx::magic();
        let spots = meteor_spots(Vec3::ZERO, 10, 1200.0);
        let sets = [
            sounds(&cues(|f| comet(f, MemberId(0), SpellRef::Arcane(ArcaneLine::Comet, 2), 2, Vec3::ZERO, Vec3::ZERO, 0.0, 2, m.comet_gap, m.comet_fall, m.comet_height))),
            sounds(&cues(|f| gravity(f, MemberId(0), SpellRef::Arcane(ArcaneLine::Gravity, 3), 3, Vec3::ZERO, 1.0))),
            sounds(&cues(|f| meteor(f, MemberId(0), SpellRef::Arcane(ArcaneLine::Meteor, 3), 3, Vec3::ZERO, Vec3::ZERO, 0.0, &spots, m.meteor_gap, m.meteor_fall, m.meteor_height))),
            sounds(&cues(|f| ultima(f, MemberId(0), SpellRef::Arcane(ArcaneLine::Ultima, 1), Vec3::ZERO, m.ultima_delay))),
        ];
        for (i, a) in sets.iter().enumerate() {
            assert!(a.len() >= 4, "spell {i}: a layered sound design ({} sounds)", a.len());
            for b in sets.iter().skip(i + 1) {
                assert!(a.iter().all(|s| !b.contains(s)), "a sound shared between two Y spells: {a:?} / {b:?}");
            }
            for s in a {
                assert!(SOUNDS.contains(s), "{s} is packed (SOUNDS)");
            }
        }
        // Gravity: the hum as it forms, the press as it squeezes and the implosion at the bottom, on the ball's clock
        let g = cues(|f| gravity(f, MemberId(0), SpellRef::Arcane(ArcaneLine::Gravity, 1), 1, Vec3::ZERO, 1.0));
        let at = |a: &str| g.iter().find(|c| matches!(c.cue, Cue::Se { asset, .. } if asset == a)).map(|c| c.left).unwrap();
        assert!(at(SE_GRAVITY_HUM) < m.gravity_fx_form);
        assert!((at(SE_GRAVITY_PRESS) - m.gravity_press_at).abs() < 1e-5);
        assert!(at(SE_GRAVITY_IMPLODE) > m.gravity_press_at && at(SE_GRAVITY_IMPLODE) <= m.gravity_press_at + m.gravity_press_time);
    }

    /// (YFX) the ball keeps its tier size for an ordinary enemy and grows to fit a tall one (any enemy, from its height).
    #[test]
    fn gravity_ball_fits_any_enemy() {
        assert_eq!(gravity_scale(2, 180.0), 1.0);
        assert_eq!(gravity_scale(1, 0.0), 1.0);
        assert!(gravity_scale(2, 400.0) > 1.4 && gravity_scale(2, 400.0) < 1.5);
        assert_eq!(gravity_scale(3, 5000.0), 3.0);
    }
}
