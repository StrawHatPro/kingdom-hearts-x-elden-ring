//! (DON-SPELLB) THE BIG PAYLOADS: the visual layer of Donald's line-3 ultimates and Ultima (the user, 2026-10-22: "make sure the
//! ultimate spells look, feel and sound extremely powerful. Same with Ultima"; "all of the ultimate magic needs to look as
//! powerful and amazing as the thunder ultimate"). Their rules (damage, statuses, area) stay in `cast.rs` `launch`; this
//! adds, at the release, a timed volley of KH3's biggest effects of the element round the target:
//! - THE CORE: KH3's Grand-Magic-class finisher of the element (`vm_*c_rng3`: Sora's magic combo finishers, which darken the
//!   scene, shake the camera, throw debris and carry their own KH3 SE through the effect-attached sound route);
//! - A STORM OF STRIKES (Judgment Bolt's recipe, the user's bar): many element bursts staggered over the field;
//! - THE AFTERMATH: lingering element bursts and sparks.
//! Every effect here is a KH3 effect in the packs; the recolours are the element's colour (spell_look `element_tint`).

use super::element::Element;
use super::magic_fx::{self, Ctx, MagicFx};
use super::spells::{ArcaneLine, SpellRef};
use crate::ids::MemberId;
use glam::Vec3;
use kh_core::host::{Attach, TargetId, Tint};

/// One scheduled spawn (`MagicFx::timed`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timed {
    pub left: f32,
    pub caster: MemberId,
    pub spell: SpellRef,
    pub name: &'static str,
    pub at: Vec3,
    pub scale: f32,
    pub tint: Option<Tint>,
    /// Its yaw (deg; 0 = as authored).
    pub yaw: f32,
    /// (AUD) spawned on this target's ground point as it stands when the time comes (the storms strike the enemies, as
    /// Judgment Bolt's do), else at `at`.
    pub on: Option<TargetId>,
}

/// A point of the golden-angle spiral `k` of `n` over a disc of `r` round `c` (deterministic, even cover).
fn spot(c: Vec3, k: usize, n: usize, r: f32) -> Vec3 {
    let f = ((k as f32 + 0.5) / n.max(1) as f32).sqrt() * r;
    let a = k as f32 * 2.399_963;
    c + Vec3::new(a.cos() * f, a.sin() * f, 0.0)
}

fn push(fxw: &mut MagicFx, caster: MemberId, spell: SpellRef, name: &'static str, at: Vec3, delay: f32, scale: f32, tint: Option<Tint>) {
    fxw.timed.push(Timed { left: delay, caster, spell, name, at, scale, tint, yaw: 0.0, on: None });
}

/// (AUD) a spawn on a target (its ground point when it comes; `at` if it is gone).
#[allow(clippy::too_many_arguments)]
fn push_on(fxw: &mut MagicFx, caster: MemberId, spell: SpellRef, name: &'static str, on: Option<TargetId>, at: Vec3, delay: f32, scale: f32, tint: Option<Tint>) {
    fxw.timed.push(Timed { left: delay, caster, spell, name, at, scale, tint, yaw: 0.0, on });
}

/// (AUD) THE STORM'S SPOTS (Judgment Bolt's recipe: the strikes go to the enemies): `n` spots, at least half of them on the
/// foes inside `r` of `c` (nearest first, round again), the rest spread over the field, interleaved so the storm moves
/// over the whole field.
pub fn storm_spots(c: Vec3, r: f32, n: usize, foes: &[(TargetId, Vec3)]) -> Vec<(Option<TargetId>, Vec3)> {
    let flat = |p: Vec3| Vec3::new(p.x - c.x, p.y - c.y, 0.0).length();
    let mut inside: Vec<(TargetId, Vec3)> = foes.iter().copied().filter(|(_, p)| flat(*p) <= r).collect();
    inside.sort_by(|a, b| flat(a.1).total_cmp(&flat(b.1)));
    let on_foes = if inside.is_empty() { 0 } else { (n / 2).max(inside.len().min(n)) };
    let f: Vec<(Option<TargetId>, Vec3)> = (0..on_foes).map(|k| (Some(inside[k % inside.len()].0), inside[k % inside.len()].1)).collect();
    let left = n - on_foes;
    let g: Vec<(Option<TargetId>, Vec3)> = (0..left).map(|k| (None, spot(c, k, left, r * 0.85))).collect();
    let mut out = Vec::with_capacity(n);
    for k in 0..f.len().max(g.len()) {
        if let Some(x) = f.get(k) {
            out.push(*x);
        }
        if let Some(x) = g.get(k) {
            out.push(*x);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn push_yaw(fxw: &mut MagicFx, caster: MemberId, spell: SpellRef, name: &'static str, at: Vec3, delay: f32, scale: f32, yaw: f32) {
    fxw.timed.push(Timed { left: delay, caster, spell, name, at, scale, tint: None, yaw, on: None });
}

/// FLOOD's look (the user, 2026-10-22: "Flood doesn't have any VFX": its KH3 picks were level water pushes that draw
/// almost nothing): the slam's water burst at Donald's feet (Sora's Water detonations, bigger each level), the wave thrown
/// across the arc (the water enemies' forward splash, one per 30 deg, turned outward, a second rank further out) and the
/// puddle's splashes for its life.
pub fn flood(fxw: &mut MagicFx, caster: MemberId, lv: u8, me: Vec3, yaw: f32, half: f32, reach: f32, puddle: Vec3, puddle_r: f32, puddle_life: f32) {
    let spell = SpellRef::Line2(Element::Water, lv);
    let burst = match lv { 1 => "vm_wat_han040", 2 => "vm_wat_han060", _ => "vm_wat_han080" };
    push(fxw, caster, spell, burst, me, 0.0, 0.9, None);
    let n = ((2.0 * half / 30.0).round() as usize).max(3);
    for k in 0..n {
        let a = if half >= 179.0 { k as f32 * 360.0 / n as f32 } else { -half + (k as f32 + 0.5) * 2.0 * half / n as f32 };
        let dir = (yaw + a).to_radians();
        let d = Vec3::new(dir.cos(), dir.sin(), 0.0);
        push_yaw(fxw, caster, spell, "ve_ca004_wat_atk1", me + d * 150.0, 0.0, 1.2, yaw + a);
        push_yaw(fxw, caster, spell, "ve_ex004_wat_atk1", me + d * (reach * 0.45), 0.18, 1.4, yaw + a);
    }
    let m = (puddle_life / 0.5) as usize;
    for k in 0..m {
        push(fxw, caster, spell, "vs_wat_hit1", spot(puddle, k % 7, 7, puddle_r * 0.8), 0.4 + k as f32 * 0.5, 1.3, None);
    }
}

/// THE PAYLOAD of an ultimate / Ultima released at `ground` (the target's ground point), `me` (Donald's feet), `yaw` (deg).
pub fn launch(fxw: &mut MagicFx, caster: MemberId, spell: SpellRef, ground: Vec3, me: Vec3, yaw: f32, radius: f32) {
    launch_foes(fxw, caster, spell, ground, me, yaw, radius, radius, &[]);
}

/// (AUD) [`launch`] with the field's own radius (`field`: the sheet row's) and the foes standing then (id, ground point): the
/// ultimates' storms strike them first, as Judgment Bolt's do.
#[allow(clippy::too_many_arguments)]
pub fn launch_foes(fxw: &mut MagicFx, caster: MemberId, spell: SpellRef, ground: Vec3, me: Vec3, yaw: f32, radius: f32, field: f32, foes: &[(TargetId, Vec3)]) {
    let fwd = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
    match spell {
        // (AUD) INFERNO, rebuilt to Judgment Bolt's bar (the review of 2026-10-23: after the finisher only small puffs, and the
        // field burned with nothing to see): the Fire finisher (KH3's darkening, the ring of fire), then a storm of ten big fire
        // eruptions (Sora's grand Fire burst without its screen veil) striking the enemies in the field first, the field itself
        // burning for its whole life (vd_don_aud_inferno, the field actor's body: cast.rs), the flare explosion rising
        SpellRef::Ultimate(Element::Fire) => {
            push(fxw, caster, spell, "vm_fic_rng3", ground, 0.0, 1.5, None);
            for (k, (on, at)) in storm_spots(ground, field, 10, foes).into_iter().enumerate() {
                push_on(fxw, caster, spell, "vd_don_aud_fir_erupt", on, at, 0.3 + k as f32 * 0.16, if k % 2 == 0 { 1.0 } else { 0.85 }, None);
            }
            push(fxw, caster, spell, "ve_ex771_flr4_exp_040", ground + Vec3::Z * 60.0, 0.6, 1.6, None);
            push(fxw, caster, spell, "vm_fic_rng0", ground, 1.95, 1.4, None);
        }
        // (AUD) DIAMOND DUST, rebuilt to Judgment Bolt's bar (was: the finisher's blue veil twice, then small bursts): the
        // Blizzard finisher (once), a blizzard of glittering ice over the whole area (vd_don_aud_ddust), the ice dome with its
        // giant crystals bursting up on the target as everything freezes (dust.delay 0.6 s), and ten clusters of ice spikes
        // erupting on the enemies first, then over the field
        SpellRef::Ultimate(Element::Blizzard) => {
            push(fxw, caster, spell, "vm_blc_rng3", ground, 0.0, 1.6, None);
            push(fxw, caster, spell, "vd_don_aud_ddust", ground, 0.05, (field / 1200.0).max(0.5), None);
            push(fxw, caster, spell, "vd_don_aud_blz_dome", ground, 0.45, 1.3, None);
            for (k, (on, at)) in storm_spots(ground, field, 10, foes).into_iter().enumerate() {
                push_on(fxw, caster, spell, "vm_blz_han060", on, at, 0.2 + k as f32 * 0.13, if k % 2 == 0 { 1.15 } else { 0.95 }, None);
            }
        }
        // JUDGMENT BOLT: the user's bar, kept as it is (its own 12 strikes, magic_fx's Storm rule; only their flash softened)
        SpellRef::Ultimate(Element::Thunder) => {}
        // (AUD) TSUNAMI, rebuilt to Judgment Bolt's bar (was: the finisher's vortex and a few splashes; no wave could be seen): the
        // Water finisher round Donald, the giant water dome swelling out of him (Sora's grand Water without its screen veil), and
        // THE WAVE: rings of breaking water rolling out at the spell's own speed (tsunami.speed) to the field's edge, each splash
        // turned outward, a burst of water on every enemy the moment the wave reaches it
        SpellRef::Ultimate(Element::Water) => {
            push(fxw, caster, spell, "vm_wac_rng3", me, 0.0, 1.6, None);
            // (the dome a little ahead of him and the splashes kept off the arc behind him: the player's camera stands there,
            // and a wave through it whited the screen out for a second, review 2026-10-23)
            push(fxw, caster, spell, "vd_don_aud_wat_dome", me + fwd * 250.0, 0.15, 1.15, None);
            let speed = super::magic_fx::magic().tsunami_speed.max(100.0);
            let rings = 6usize;
            for k in 0..rings {
                let r = 220.0 + (field - 220.0).max(0.0) * k as f32 / (rings - 1) as f32;
                let n = 8 + 2 * k;
                for i in 0..n {
                    let a = (i as f32 + 0.5 * (k % 2) as f32) * 360.0 / n as f32;
                    if (a - 180.0).abs() < 62.0 && r < 1100.0 {
                        continue;
                    }
                    let a = a + yaw;
                    let d = Vec3::new(a.to_radians().cos(), a.to_radians().sin(), 0.0);
                    push_yaw(fxw, caster, spell, if (i + k) % 2 == 0 { "ve_ex004_wat_atk1" } else { "ve_ca004_wat_atk1" }, me + d * r, 0.2 + r / speed, 1.5 + 0.1 * k as f32, a);
                }
            }
            for (t, p) in foes.iter().filter(|(_, p)| Vec3::new(p.x - me.x, p.y - me.y, 0.0).length() <= field) {
                let dist = Vec3::new(p.x - me.x, p.y - me.y, 0.0).length();
                push_on(fxw, caster, spell, "vm_wat_han060", Some(*t), *p, 0.2 + dist / speed, 1.0, None);
            }
        }
        // AERIAL BLAST: the Aero finisher at the target, the cannon's line of tornadoes rising one after another, Aeroga's storm
        SpellRef::Ultimate(Element::Aero) => {
            push(fxw, caster, spell, "vm_aic_rng3", ground, 0.0, 1.4, None);
            for k in 0..5 {
                push(fxw, caster, spell, "vm_air_min_2", me + fwd * (300.0 + k as f32 * 350.0), 0.05 + k as f32 * 0.12, 1.2, None);
            }
            push(fxw, caster, spell, "vm_air_han_140", ground, 0.5, 1.3, None);
            push(fxw, caster, spell, "vm_air_zan0", ground, 0.9, 1.6, None);
        }
        // HOLY: a column of light on the target, then a ring of golden strikes round it and the flare's flash in gold
        // (AUD: the ring drawn in close round the target, 6 single strikes without any scene dimming: the 8 Thundaga strikes
        // 4-8 m out whited the whole screen out for a second and a half, review of 2026-10-23)
        SpellRef::Ultimate(Element::Light) => {
            let gold = Some(super::magic_fx::fx::GOLD);
            // (AUD) the centre is the spell's own beam and burst (cast.rs HOLY[1] / HOLY[2]); KH3's Thundaga strike spawns a forest of
            // columns, and eight of them round the target whited the screen out
            for k in 0..6 {
                let a = (k as f32 * 60.0).to_radians();
                push(fxw, caster, spell, "vd_don_aud_str1s", ground + Vec3::new(a.cos(), a.sin(), 0.0) * 320.0, 0.4 + k as f32 * 0.14, 1.0, gold);
            }
            push(fxw, caster, spell, "vm_cur_cal0", ground, 0.2, 2.5, None);
            push(fxw, caster, spell, "ve_ex771_flr4_exp_040", ground + Vec3::Z * 80.0, 1.3, 1.5, gold);
        }
        // ULTIMA: the Keyblade Graveyard's dome of pure magic swelling over the target, strikes of pure magic all round it,
        // the final flare
        // (YFX, the user 2026-10-23: the Y effects "look EXTREMELY plain") ULTIMA has its own look and sound now, on its hit's
        // clock: kh-party donald/yfx.rs `ultima` (called from cast.rs `arcane`); nothing more here (its old strike ring
        // reused Judgment Bolt's strikes)
        SpellRef::Arcane(ArcaneLine::Ultima, _) => {}
        _ => {}
    }
}

/// THE RELEASE BURSTS' look (4.3: "the switch itself is a move"), at `at` (the burst's ground point), Donald at `me`.
pub fn release(fxw: &mut MagicFx, caster: MemberId, e: Element, at: Vec3, me: Vec3, radius: f32) {
    let spell = SpellRef::Release(e);
    match e {
        // (AUD) FIRESTORM: a fire pillar (the review of 2026-10-23: the Releases were small puffs beside Overload's lightning):
        // the big fire eruption (make_ult.js) as the pillar, Fira's ring of eruptions, the flare explosion rising, bursts round it
        Element::Fire => {
            push(fxw, caster, spell, "vd_don_aud_fir_erupt", at, 0.0, 1.25, None);
            push(fxw, caster, spell, "vm_fic_rng0", at, 0.0, 1.3, None);
            push(fxw, caster, spell, "ve_ex771_flr4_exp_040", at + Vec3::Z * 90.0, 0.12, 1.0, None);
            for k in 0..5 {
                push(fxw, caster, spell, "vm_fir_wht0", spot(at, k, 5, radius * 0.6), 0.2 + k as f32 * 0.08, 1.1, None);
            }
        }
        // SHATTER: an ice burst: Blizzaga's burst, the freeze shatter, frost bursts in a ring
        Element::Blizzard => {
            // (AUD) the ice bursting up: Blizzaga's spike cluster
            push(fxw, caster, spell, "vm_blz_han060", at, 0.0, 1.6, None);
            push(fxw, caster, spell, "vm_blb_fir0", at + Vec3::Z * 60.0, 0.0, 2.2, None);
            push(fxw, caster, spell, "vs_abn_frz_out1", at + Vec3::Z * 60.0, 0.0, 2.5, None);
            for k in 0..6 {
                push(fxw, caster, spell, "vm_blz_wht0", spot(at, k, 6, radius * 0.6), 0.08 + k as f32 * 0.06, 1.1, Some(super::magic_fx::fx::FROST));
            }
        }
        // OVERLOAD: its strikes on every enemy (cast.rs) and the Thunder burst round Donald
        Element::Thunder => push(fxw, caster, spell, "vd_don_aud_str1s", me, 0.0, 1.2, None), // (AUD) strobe-free: the burst at the target keeps the flash
        // DELUGE: a flood wave ring and Waterga's bursts
        Element::Water => {
            // (AUD) the flood bursting out: Waterga's water burst, and a ring of the wave's splashes turned outward
            push(fxw, caster, spell, "vm_wat_han060", at, 0.0, 1.1, None);
            for k in 0..8 {
                let a = k as f32 * 45.0;
                let d = Vec3::new(a.to_radians().cos(), a.to_radians().sin(), 0.0);
                push_yaw(fxw, caster, spell, "ve_ex004_wat_atk1", at + d * radius * 0.45, 0.12, 1.3, a);
            }
            push(fxw, caster, spell, "vm_waw_fir3", at, 0.0, 1.6, None);
            push(fxw, caster, spell, "vl_wat_psh0", at, 0.05, 1.5, None);
            for k in 0..5 {
                push(fxw, caster, spell, "vm_wat_bar3", spot(at, k, 5, radius * 0.5), 0.15 + k as f32 * 0.08, 0.8, None);
            }
        }
        // CYCLONE: a whirlwind gathering them (Aero's tornado) and Donald's own tornado
        Element::Aero => {
            // (AUD) the whirlwind itself: Aerora's tornado
            push(fxw, caster, spell, "vm_air_han_080", at, 0.0, 1.2, None);
            push(fxw, caster, spell, "vm_air_min_2", at, 0.0, 1.1, None);
            push(fxw, caster, spell, "vn_ex002_trn_awa00", at, 0.1, 1.5, Some(super::magic_fx::fx::GREEN));
        }
        // SANCTUARY: a burst of healing light round Donald, the holy call
        Element::Light => {
            push(fxw, caster, spell, "vm_cur_cal0", me, 0.0, 3.2, None);
            push(fxw, caster, spell, "vn_ex003_hly_cal0", me + Vec3::Z * 60.0, 0.05, 2.4, None);
            push(fxw, caster, spell, "vm_cur_hed2", me, 0.2, 2.2, None);
            // (AUD) bigger, and the heal's light on the party round him
            push(fxw, caster, spell, "vm_cur_bds2", me, 0.25, 2.0, None);
        }
    }
}

/// Every step: the scheduled spawns whose time has come.
pub fn step(fxw: &mut MagicFx, cx: &mut Ctx, dt: f32) {
    if fxw.timed.is_empty() {
        return;
    }
    let mut due = Vec::new();
    for t in fxw.timed.iter_mut() {
        t.left -= dt;
        if t.left <= 0.0 {
            due.push(*t);
        }
    }
    fxw.timed.retain(|t| t.left > 0.0);
    for mut t in due {
        // (AUD) on a target: its ground point now (gone: where it was)
        if let Some(id) = t.on {
            if let Some(i) = cx.host.targets.info(id).filter(|i| i.flags.has(kh_core::host::TargetFlags::ALIVE)) {
                t.at = i.position;
            }
        }
        let tint = t.tint.or_else(|| magic_fx::fx::tint_of(Some(t.spell), t.name));
        if t.yaw != 0.0 {
            let Some(m) = cx.party.member_mut(t.caster) else { continue };
            let h = m.seam.spawn(cx.host.effects, &kh_core::host::EffectSpawn { id: kh_core::host::EffectId(kh_core::notify::notify_asset_id(t.name)), name: t.name, at: Attach::World(t.at), rotation: Vec3::new(0.0, t.yaw, 0.0), scale: t.scale, persistent: false });
            if let Some(tt) = tint {
                cx.host.effects.tint(h, tt);
            }
            continue;
        }
        let _ = magic_fx::spawn_fx_tint(cx, t.caster, Some(t.spell), t.name, Attach::World(t.at), t.scale, false, tint);
    }
}

/// Every effect name the payloads spawn (the pack harness checks each one).
pub fn all_names() -> Vec<&'static str> {
    let mut fxw = MagicFx::default();
    for s in [SpellRef::Ultimate(Element::Fire), SpellRef::Ultimate(Element::Blizzard), SpellRef::Ultimate(Element::Thunder), SpellRef::Ultimate(Element::Water), SpellRef::Ultimate(Element::Aero), SpellRef::Ultimate(Element::Light), SpellRef::Arcane(ArcaneLine::Ultima, 1)] {
        launch(&mut fxw, MemberId(0), s, Vec3::ZERO, Vec3::ZERO, 0.0, 800.0);
    }
    for lv in 1..=3 {
        flood(&mut fxw, MemberId(0), lv, Vec3::ZERO, 0.0, 60.0, 800.0, Vec3::ZERO, 300.0, 6.0);
    }
    for e in Element::ALL {
        release(&mut fxw, MemberId(0), e, Vec3::ZERO, Vec3::ZERO, 600.0);
    }
    let mut v: Vec<&'static str> = fxw.timed.iter().map(|t| t.name).collect();
    v.extend(super::yfx::all_names()); // (YFX) the Y spells' effects
    v.sort_unstable();
    v.dedup();
    v
}
