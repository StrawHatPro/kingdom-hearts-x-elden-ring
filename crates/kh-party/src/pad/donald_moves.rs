//! (DON-KIT) DONALD'S DEFENCE AND MOVEMENT MOVES (documentation/DONALD_OVERHAUL.md 4.1, 4.2, 5.1, 5.2, 8; plan
//! DON_PLAN.md 3 / 4.1 / 4.2). "Defense and survival - the weak side, on purpose."
//!
//! - THE DODGE (X + stick) is set by his attunement (4.2, the primary one with Dual Attunement): none = the Hop (300 cm,
//!   brief invincibility); Fire = Flame Step (a dash leaving a fire trail); Blizzard = Ice Glide (a long low slide that
//!   Chills what it touches; it replaces the Superslide); Thunder = Blink (a short invincible teleport); Water = Bubble
//!   Hop (the hop inside a bubble that absorbs one hit); Aero = Air Step (a mid-air dash, two per airtime; the Hop on
//!   the ground); Light = Ward Step (a short dodge leaving a barrier: a party member inside has one hit absorbed).
//!   0.9 s COOLDOWN from a dodge's start; a press within `dodge.buffer` of its end is kept and dodges when it ends, an
//!   earlier one gets KH3's refused SE (GK's Hop rule).
//! - MAGIC BLOCK (X, stick neutral): AttackReflect, frontal, blocks ONE hit in its window; PERFECT in the first 0.15 s:
//!   a projectile (a hit from beyond `block.reflect_range`) is sent back as a missile, `DonCore::on_perfect_block` (+1
//!   pip, +1 Arcane Flow). A block that catches nothing leaves him OPEN for `block.miss_open`. No Counter Blast.
//! - AERO FLOAT (B held falling, Red Wolf's ability, as before) and AERO GLIDE with Aero attuned (faster, longer).
//! - THE ATTUNE CAST (4.1): MagicCast for the cast's length with the element's cast effect; the pad's and AI Donald's.

use super::donald::{self as kit, has, note, KitEvent, AIRSTEP, ATTUNE, BLOCK, DODGE, FLOAT, K};
use super::donald_shots::{self as shots, Area, AreaKind, MissileKind, MissileSpec};
use crate::donald::{data as don_data, DonAbility, Element};
use crate::host::PartyHost;
use crate::pawn::{forward, horizontal, wrap180, yaw_of, NpcPawn, StepCtx};

use glam::{Vec2, Vec3};
use kh_core::host::{Attach, EffectHandle, EffectId, EffectSpawn, HitRecord, HitResult, SoundHandle, SoundId, SoundPlay, TargetId};
use kh_core::input::PadButton;

/// The dodges (4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dodge {
    Hop,
    Flame,
    Glide,
    Blink,
    Bubble,
    Ward,
}

impl Dodge {
    pub fn of(e: Option<Element>) -> Dodge {
        match e {
            Some(Element::Fire) => Dodge::Flame,
            Some(Element::Blizzard) => Dodge::Glide,
            Some(Element::Thunder) => Dodge::Blink,
            Some(Element::Water) => Dodge::Bubble,
            Some(Element::Light) => Dodge::Ward,
            Some(Element::Aero) | None => Dodge::Hop,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Dodge::Hop => "hop",
            Dodge::Flame => "flame_step",
            Dodge::Glide => "ice_glide",
            Dodge::Blink => "blink",
            Dodge::Bubble => "bubble_hop",
            Dodge::Ward => "ward_step",
        }
    }

    fn index(self) -> u8 {
        self as u8
    }

    fn from_index(i: u8) -> Dodge {
        [Dodge::Hop, Dodge::Flame, Dodge::Glide, Dodge::Blink, Dodge::Bubble, Dodge::Ward].get(i as usize).copied().unwrap_or(Dodge::Hop)
    }

    /// (distance cm, move seconds, invincible to, hand-over at).
    pub fn numbers(self) -> (f32, f32, f32, f32) {
        match self {
            Dodge::Hop | Dodge::Bubble => (K.hop_distance, K.hop_time, K.hop_invincible_to, K.hop_recover),
            Dodge::Flame => (K.flame_distance, K.flame_time, K.flame_invincible_to, K.flame_time + 0.08),
            Dodge::Glide => (K.glide_distance, K.glide_time, K.glide_invincible_to, K.glide_time + 0.12),
            Dodge::Blink => (K.blink_distance, K.blink_time, K.blink_time, K.blink_time + 0.1),
            Dodge::Ward => (K.ward_distance, K.ward_time, K.hop_invincible_to.min(K.ward_time), K.ward_time + 0.08),
        }
    }
}

/// The running dodge.
pub fn dodge_of(k: &kit::DonaldKit) -> Dodge {
    Dodge::from_index(k.run.sub)
}

fn member(p: &NpcPawn) -> kh_core::host::PartyMember {
    kh_core::host::PartyMember(p.id.0)
}

fn spawn_on(p: &mut NpcPawn, host: &mut PartyHost, name: &'static str, bone: Option<&'static str>, persistent: bool) -> u32 {
    if name.is_empty() || name == "-" {
        return 0;
    }
    let at = Attach::party(member(p), bone);
    p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at, rotation: Vec3::ZERO, scale: 1.0, persistent }).0
}

/// (KITFX) A sound on him at a volume (the float's soft wind).
fn sound_vol(p: &mut NpcPawn, host: &mut PartyHost, name: &'static str, looping: bool, volume: f32) -> u32 {
    if name.is_empty() || name == "-" {
        return 0;
    }
    let m = member(p);
    p.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(name)), name, at: Attach::Party { member: m, bone: None, offset: Vec3::ZERO }, volume, pitch: 1.0, looping, spatial: true }).0
}

fn sound_on(p: &mut NpcPawn, host: &mut PartyHost, name: &'static str, looping: bool) -> u32 {
    if name.is_empty() || name == "-" {
        return 0;
    }
    let m = member(p);
    p.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(name)), name, at: Attach::Party { member: m, bone: None, offset: Vec3::ZERO }, volume: 1.0, pitch: 1.0, looping, spatial: true }).0
}

// ================================================================================================ the dodge

/// THE AERO AURA'S EVASION (4.2; DON-PROG `aura.aero_evasion`): invincible windows x this, dodge cooldowns / this.
pub fn evasion(p: &NpcPawn) -> f32 {
    1.0 + p.don_aura.evasion.max(0.0)
}

/// The cooldown is over.
pub fn dodge_ready(p: &NpcPawn) -> bool {
    kit::kit_ref(p).is_some_and(|k| k.clock + 1e-4 >= k.dodge_ready_at)
}

/// Invincible in the running dodge's window.
pub fn dodge_invincible(p: &NpcPawn, k: &kit::DonaldKit) -> bool {
    let d = dodge_of(k);
    let (_, _, to, _) = d.numbers();
    let to = to * evasion(p); // Donald's Aero aura: the window x (1 + evasion)
    let from = if matches!(d, Dodge::Hop | Dodge::Bubble | Dodge::Ward) { K.hop_invincible_from } else { 0.0 };
    k.run.t >= from && k.run.t <= to
}

/// X (from pre-move or a cancel point): the dodge with the stick, Magic Block without; Air Step in the air (Aero). True =
/// the press was used (a move, a kept press, or the refused cue).
pub fn press_x(p: &mut NpcPawn, host: &mut PartyHost) -> bool {
    let (_, _, active) = kit::stick(p);
    if let Some(k) = kit::kit(p) {
        k.buffered_x = 0.0;
    }
    if !p.on_ground() {
        if active && kit::primary(p) == Some(Element::Aero) {
            let used = kit::kit_ref(p).map_or(0, |k| k.air_dodges);
            if (used as f32) < K.airstep_count {
                start_airstep(p, host);
                return true;
            }
            kit::refused(p, "air_step");
            return true;
        }
        if !active {
            start_block(p, host);
            return true;
        }
        // no air dodge without Aero
        return false;
    }
    if !active {
        start_block(p, host);
        return true;
    }
    if !dodge_ready(p) {
        let left = kit::kit_ref(p).map_or(0.0, |k| k.dodge_cooldown_left());
        if left <= K.dodge_buffer {
            if let Some(k) = kit::kit(p) {
                k.dodge_buffered = true;
            }
        } else {
            kit::refused(p, "dodge_cooldown");
        }
        return true;
    }
    start_dodge(p, host)
}

/// Starts the dodge of his attunement now.
pub fn start_dodge(p: &mut NpcPawn, host: &mut PartyHost) -> bool {
    let kind = Dodge::of(kit::primary(p));
    let (stick_yaw, _, active) = kit::stick(p);
    let yaw = if active { stick_yaw } else { p.yaw };
    let lock = kit::target_info(p, host).filter(|_| p.pad.as_ref().is_some_and(|d| d.lock != TargetId::NONE));
    let start = p.position();
    let ev = evasion(p);
    kit::enter_kit(p, DODGE);
    p.cmc.stop_movement();
    let clock = kit::kit_ref(p).map_or(0.0, |k| k.clock);
    if let Some(k) = kit::kit(p) {
        k.dodge_ready_at = clock + K.dodge_cooldown / ev;
        k.dodge_buffered = false;
        k.run.sub = kind.index();
        k.run.dir = forward(yaw);
        k.run.start = start;
    }
    match kind {
        Dodge::Glide => {
            p.yaw = yaw;
            p.anim.play(super::donald_combo::clip_copy("SonicSlide", true), 0.05);
            let fx = spawn_on(p, host, K.glide_fx, None, true);
            if let Some(t) = crate::donald::magic_fx::fx::kit_tint("glide") {
                host.effects.tint(EffectHandle(fx), t); // (DON-MAGIC tint) [R] ice blue
            }
            let snd = sound_on(p, host, K.glide_se, true);
            let mut a = Area::new(AreaKind::Hit, start, K.glide_radius, K.glide_time, "DON_ICE_GLIDE");
            (a.follow, a.power, a.element) = (true, K.glide_power, Some(Element::Blizzard));
            a.hit_fx = K.missile_blizzard_hit_fx;
            a.hit_se = K.missile_blizzard_hit_se;
            shots::add_area(p, host, a);
            if let Some(k) = kit::kit(p) {
                (k.owned_fx, k.owned_snd) = (fx, snd);
            }
        }
        Dodge::Blink => {
            p.yaw = yaw;
            let h = shots::effect(p, host, K.blink_in_fx, start + Vec3::Z * 90.0, 1.0);
            if let Some(t) = crate::donald::magic_fx::fx::kit_tint("blink") {
                host.effects.tint(EffectHandle(h), t); // (DON-MAGIC tint) [R] yellow
            }
            shots::sound(p, host, K.blink_se, start);
        }
        _ => {
            // the hop: sideways (HopL / HopR) keeping the facing (the lock's), else turned to the stick
            let facing = lock.map_or(p.yaw, |t| yaw_of(horizontal(t.position - p.position())));
            let off = wrap180(yaw - facing);
            let side = off.abs() > K.hop_side_angle && off.abs() < 180.0 - K.hop_side_angle;
            let clip = if side {
                p.yaw = facing;
                if off < 0.0 { "HopL" } else { "HopR" }
            } else {
                p.yaw = yaw;
                "HopR"
            };
            p.play(clip, 0.05);
            if kind == Dodge::Bubble {
                if kit::kit_ref(p).is_some_and(|k| k.bubble_fx != 0) {
                    end_bubble(p, host);
                }
                let fx = spawn_on(p, host, K.bubble_fx, None, true);
                if let Some(k) = kit::kit(p) {
                    k.bubble = K.bubble_life;
                    k.bubble_fx = fx;
                }
            }
            if kind == Dodge::Ward {
                let mut a = Area::new(AreaKind::Ward, start, K.ward_radius, K.ward_life, "DON_WARD");
                a.fx = K.ward_fx;
                shots::add_area(p, host, a);
            }
        }
    }
    note(p, KitEvent::Started(kind.name()));
    true
}

fn start_airstep(p: &mut NpcPawn, host: &mut PartyHost) {
    let (stick_yaw, _, active) = kit::stick(p);
    let yaw = if active { stick_yaw } else { p.yaw };
    let ev = evasion(p);
    kit::enter_kit(p, AIRSTEP);
    p.yaw = yaw;
    p.play("HopR", 0.05);
    p.cmc.velocity = Vec3::ZERO;
    p.cmc.gravity_off = true;
    p.cmc.set_flying();
    let clock = kit::kit_ref(p).map_or(0.0, |k| k.clock);
    if let Some(k) = kit::kit(p) {
        k.air_dodges += 1;
        k.dodge_ready_at = clock + K.dodge_cooldown / ev;
        k.run.dir = forward(yaw);
    }
    spawn_on(p, host, K.airstep_fx, None, false);
    note(p, KitEvent::Started("air_step"));
}

/// The bubble absorbed a hit: it pops.
pub fn pop_bubble(p: &mut NpcPawn) {
    if let Some(k) = kit::kit(p) {
        k.bubble = 0.0;
        k.run.blocked = true;
    }
}

/// The bubble's effect ends (its life is over or it popped).
pub fn end_bubble(p: &mut NpcPawn, host: &mut PartyHost) {
    let (fx, popped) = kit::kit_ref(p).map_or((0, false), |k| (k.bubble_fx, k.bubble <= 0.0));
    if fx != 0 {
        host.effects.stop(EffectHandle(fx), false);
    }
    if popped {
        let at = p.capsule_centre();
        shots::sound(p, host, K.bubble_pop_se, at);
    }
    if let Some(k) = kit::kit(p) {
        k.bubble_fx = 0;
        k.bubble = 0.0;
    }
}

/// The running dodge / block hands over (the next move, a cast, the jump).
pub fn cancel_open(p: &NpcPawn) -> bool {
    let Some(k) = kit::kit_ref(p) else { return false };
    match kit::kit_state(p) {
        Some(DODGE) => k.run.t >= dodge_of(k).numbers().3,
        Some(BLOCK) => k.run.blocked && k.run.t >= K.block_recover,
        Some(AIRSTEP) => k.run.t >= K.airstep_time,
        _ => false,
    }
}

// ================================================================================================ Magic Block

fn start_block(p: &mut NpcPawn, host: &mut PartyHost) {
    let t = kit::target_info(p, host);
    let air = !p.on_ground();
    kit::enter_kit(p, BLOCK);
    if let Some(t) = t {
        let d = horizontal(t.position - p.position());
        if d.length_squared() > 1.0 {
            p.yaw = yaw_of(d);
        }
    }
    p.cmc.stop_movement();
    if air {
        p.cmc.velocity = Vec3::ZERO;
        p.cmc.gravity_off = true;
        p.cmc.set_flying();
    }
    p.play(if air { "AttackAirReflect" } else { K.block_clip }, 0.05);
    spawn_on(p, host, K.block_raise_fx, Some("vfx_call"), false);
    note(p, KitEvent::Started("magic_block"));
}

/// A hit during Magic Block (`on_hit`): frontal and inside the window -> blocked (perfect in the first 0.15 s). Some =
/// the kit answered.
pub fn block_hit(p: &mut NpcPawn, hit: &HitRecord) -> Option<HitResult> {
    let k = kit::kit_ref(p)?;
    let t = k.run.t;
    if k.run.blocked || t > K.block_window {
        return None;
    }
    let from = if hit.attacker.valid { hit.attacker.position } else { p.position() - hit.direction * 100.0 };
    let d = horizontal(from - p.position());
    let front = d.length_squared() < 1.0 || wrap180(yaw_of(d) - p.yaw).abs() <= K.block_angle * 0.5;
    if !front {
        return None;
    }
    let perfect = t <= K.block_perfect;
    let far = horizontal(from - p.position()).length() > K.block_reflect_range;
    if perfect {
        let d = don_data();
        if let Some(st) = kit::don_mut(p) {
            let mods = st.abilities.mods();
            st.core.on_perfect_block(&mods, &d);
        }
    }
    if let Some(k) = kit::kit(p) {
        k.run.blocked = true;
        // the effects at the next tick (receive_hit has no effect sink); the reflect's aim
        k.run.done = perfect;
        k.run.fx_pending = true;
        k.run.start = from;
        k.run.element = None;
        k.run.trail = if perfect && far { 1.0 } else { 0.0 };
        k.note(KitEvent::Blocked { perfect });
    }
    Some(HitResult { accepted: true, damage_dealt: 0.0, hp_after: p.status.hp as f32, killed: false, guarded: true })
}

fn block_tick(p: &mut NpcPawn, host: &mut PartyHost) {
    let Some(k) = kit::kit_ref(p).cloned() else { return };
    let t = k.run.t;
    if p.cmc.gravity_off {
        p.cmc.direct_z = Some(0.0);
    }
    if k.run.fx_pending {
        let at = p.capsule_centre() + forward(p.yaw) * 40.0;
        shots::effect(p, host, K.block_fx, at, 1.0);
        shots::sound(p, host, K.block_se, at);
        if k.run.done {
            shots::effect(p, host, K.block_perfect_fx, at, 1.0);
        }
        // a perfect block of a projectile: sent back at its owner (the nearest enemy to where it came from)
        if k.run.trail > 0.0 {
            let from = k.run.start;
            let owner = shots::enemies(host).into_iter().min_by(|a, b| a.position.distance(from).total_cmp(&b.position.distance(from)));
            let origin = at + Vec3::Z * 20.0;
            let aim = owner.map_or(from + Vec3::Z * 90.0, |o| o.center);
            let mut spec = MissileSpec::new(MissileKind::Reflect, None, origin, (aim - origin).normalize_or_zero(), owner.map_or(TargetId::NONE, |o| o.id));
            spec.mult = K.block_reflect_mult;
            spec.aim = Some(aim);
            shots::launch(p, host, spec);
            note(p, KitEvent::Missile { kind: MissileKind::Reflect, element: None, mult: K.block_reflect_mult });
        }
        if let Some(kk) = kit::kit(p) {
            kk.run.fx_pending = false;
            kk.run.trail = 0.0;
        }
    }
    if k.run.blocked {
        if t >= K.block_recover + 0.3 || p.anim.finished {
            end(p);
            kit::back_to_pad(p);
        }
        return;
    }
    // a miss: open
    if t > K.block_window && kit::kit_ref(p).is_some_and(|k| k.open <= 0.0 && k.run.sub == 0) {
        if let Some(kk) = kit::kit(p) {
            kk.open = K.block_miss_open;
            kk.run.sub = 1;
        }
    }
    if t >= K.block_window + K.block_miss_open {
        end(p);
        kit::back_to_pad(p);
    }
}

// ================================================================================================ Aero Float / Aero Glide

fn aero_glide(p: &NpcPawn) -> bool {
    kit::don(p).is_some_and(|d| d.core.attuned(Element::Aero))
}

/// Float seconds left this airtime (Aero Float: 2.5 s + his Aerora / Aeroga 0.5 s each; Aero Glide: `glide_aero.time`).
pub fn float_left(p: &NpcPawn) -> f32 {
    let used = kit::kit_ref(p).map_or(0.0, |k| k.float_used);
    let total = if aero_glide(p) {
        K.glide_aero_time
    } else {
        let tier = kit::don(p).map_or(0, |d| d.learned.tier(Element::Aero));
        K.float_time + if tier >= 2 { K.float_bonus_aerora } else { 0.0 } + if tier >= 3 { K.float_bonus_aeroga } else { 0.0 }
    };
    total - used
}

pub fn begin_float(p: &mut NpcPawn, host: &mut PartyHost) {
    let glide = aero_glide(p);
    kit::enter_kit(p, FLOAT);
    p.play("MagicAirCast", 0.15);
    p.anim.time = K.float_pose;
    p.anim.speed = 0.0;
    p.cmc.gravity_off = true;
    // the user (2026-10-22): "the aero effect should be small at the top of donalds wand for gliding, make it bigger when
    // he attunes to aero". One KH3 Aero wind (his own Aero spell body, trimmed to its twist, cups and specks, looping in
    // local space: make_tip_fx.js) on the wand's tip socket, small for Aero Float, bigger for Aero Glide; nothing on his body.
    let name = K.float_fx;
    let fx = if name.is_empty() || name == "-" {
        0
    } else {
        let scale = if glide { K.glide_aero_fx_scale } else { K.float_fx_scale };
        let at = Attach::party(member(p), Some(K.float_fx_socket));
        p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at, rotation: Vec3::new(K.float_fx_pitch, 0.0, 0.0), scale, persistent: true }).0
    };
    let dir = forward(p.yaw);
    if let Some(k) = kit::kit(p) {
        k.run.sub = glide as u8;
        k.owned_fx = fx;
        k.owned_more.clear();
        k.run.dir = dir;
    }
    // (KITFX) the user 2026-10-23: "aero glide needs a little non-annoying sfx": a gentle whoosh at the start, at most once
    // per `float.se_debounce` (quick toggles never stack it); the quiet loop starts in float_tick
    let clock = kit::kit_ref(p).map_or(0.0, |k| k.clock);
    if kit::kit_ref(p).and_then(|k| k.float_whoosh_at).map_or(true, |t| clock - t >= K.float_se_debounce) {
        sound_vol(p, host, K.float_se_start, false, K.float_se_start_volume);
        if let Some(k) = kit::kit(p) {
            k.float_whoosh_at = Some(clock);
        }
    }
    note(p, KitEvent::Started(if glide { "aero_glide" } else { "aero_float" }));
}

/// The float's input: B let go ends it, the stick steers.
pub fn float_input(p: &mut NpcPawn, host: &mut PartyHost) {
    if !kit::held(p, PadButton::JUMP) {
        end_float(p, host);
        return;
    }
    let (yaw, _, active) = kit::stick(p);
    if let Some(k) = kit::kit(p) {
        k.run.done = active;
        if active {
            k.run.dir = forward(yaw);
        }
    }
}

fn end_float(p: &mut NpcPawn, host: &mut PartyHost) {
    // the wand-tip wind ends at once (the user 2026-10-22: "The Aero effect for Aero glide should end as SOON as he stops
    // using it, instead of lingering on his wand"): its live particles go with it
    let (h, more) = kit::kit_ref(p).map_or((0, Vec::new()), |k| (k.owned_fx, k.owned_more.clone()));
    for h in std::iter::once(h).chain(more) {
        if h != 0 {
            host.effects.stop(EffectHandle(h), true);
        }
    }
    // (KITFX) the wind's loop fades softly the same step
    let snd = kit::kit_ref(p).map_or(0, |k| k.owned_snd);
    if snd != 0 {
        host.audio.stop(SoundHandle(snd), K.float_se_fade);
    }
    p.cmc.gravity_off = false;
    p.trav.pitch = 0.0;
    if let Some(k) = kit::kit(p) {
        k.owned_fx = 0;
        k.owned_snd = 0; // (KITFX)
        k.owned_more.clear();
    }
    super::enter_jump(p, true);
}

/// The float's sway at state time `t`: (the held MagicAirCast time, the body pitch in degrees).
pub fn sway(t: f32, glide: bool) -> (f32, f32) {
    let hz = if glide { K.glide_aero_sway_hz } else { K.float_sway_hz };
    let ph = std::f32::consts::TAU * hz * t;
    (K.float_pose + K.float_sway_pose * ph.sin(), K.float_sway_pitch * (ph - K.float_sway_lag).sin())
}

fn float_tick(p: &mut NpcPawn, host: &mut PartyHost, dt: f32) {
    let Some(k) = kit::kit_ref(p).cloned() else { return };
    let glide = k.run.sub == 1;
    let (speed, sink) = if glide { (K.glide_aero_speed, K.glide_aero_sink) } else { (K.float_speed, K.float_sink) };
    if k.run.done {
        let want = yaw_of(k.run.dir);
        p.yaw = wrap180(p.yaw + wrap180(want - p.yaw) * (K.float_turn * dt * 60.0).clamp(0.0, 1.0));
    }
    let f = forward(p.yaw);
    p.cmc.velocity = Vec3::new(f.x * speed, f.y * speed, -sink);
    // (KITFX) the quiet airy loop: ONE instance, on him, started once the float has lasted `float.se_loop_delay` (a quick
    // toggle starts none), stopped with a soft fade when the float is left (left_state)
    if k.owned_snd == 0 && p.state_time >= K.float_se_loop_delay {
        let vol = if glide { K.glide_aero_se_loop_volume } else { K.float_se_loop_volume };
        let h = sound_vol(p, host, K.float_se_loop, true, vol);
        if let Some(kk) = kit::kit(p) {
            kk.owned_snd = h;
        }
    }
    // THE SWAY (8, the user 2026-10-22: "Donald should sway while holding his staff during Aero glide"): the staff arm
    // swings about the held MagicAirCast frame and his body rocks about his feet behind it, on the state's clock
    // (frame-rate independent); the wand-tip wind follows the wand
    let (pose, pitch) = sway(p.state_time, glide);
    p.anim.time = pose;
    p.trav.pitch = pitch;
    if let Some(kk) = kit::kit(p) {
        kk.float_used += dt;
    }
    if p.on_ground() || float_left(p) <= 0.0 {
        end_float(p, host);
    }
}

// ================================================================================================ the attune cast

fn attune_fx(e: Element) -> &'static str {
    match e {
        Element::Fire => K.attune_fire_fx,
        Element::Blizzard => K.attune_blizzard_fx,
        Element::Thunder => K.attune_thunder_fx,
        Element::Water => K.attune_water_fx,
        Element::Aero => K.attune_aero_fx,
        Element::Light => K.attune_light_fx,
    }
}

/// The attune cast's move (`kit::begin_attune`, after `DonCore::begin_attune` accepted it): the cast clip; the cast
/// effect at its first tick (the move may start without the host: AI Donald, the menu's decide).
pub fn start_attune(p: &mut NpcPawn, e: Element, seconds: f32) {
    let air = !p.on_ground();
    if p.pad.is_some() && kit::kit_state(p).is_some() {
        kit::end_kit(p, "attune");
    }
    kit::enter_kit(p, ATTUNE);
    p.cmc.stop_movement();
    if air {
        p.cmc.velocity = Vec3::ZERO;
        p.cmc.gravity_off = true;
        p.cmc.set_flying();
    }
    p.play(if air { K.attune_air_clip } else { K.attune_clip }, 0.08);
    if let Some(k) = kit::kit(p) {
        k.run.element = Some(e);
        k.run.fx_pending = true;
        k.run.trail = seconds;
    }
    // MagicCast callout, short (DON_PLAN 4.3)
    let n = kh_core::host::party::magic_voice_number(member(p), p.rng.next_u32());
    p.queue_voice(kh_core::host::party::VoiceGroup::MagicCast, n, kh_core::host::party::VoiceMode::Replace);
}

fn attune_tick(p: &mut NpcPawn, host: &mut PartyHost, dt: f32) {
    let first = p.state_time <= dt * 1.5 + 1e-4;
    let casting = kit::don(p).and_then(|d| d.core.attune.cast).map(|c| c.element);
    if p.cmc.gravity_off {
        p.cmc.direct_z = Some(0.0);
    }
    if first || kit::kit_ref(p).is_some_and(|k| k.run.fx_pending) {
        let e = kit::kit_ref(p).and_then(|k| k.run.element).or(casting).or_else(|| kit::primary(p));
        if let Some(e) = e {
            spawn_on(p, host, attune_fx(e), Some("vfx_call"), false);
        }
        if let Some(k) = kit::kit(p) {
            k.run.fx_pending = false;
        }
    }
    // the cast runs while the core's attune cast does; then a short recovery
    let total = kit::kit_ref(p).map_or(0.6, |k| k.run.trail);
    let over = casting.is_none() && p.state_time >= total.min(3.0);
    if over && p.state_time >= total + K.attune_recover || p.state_time > 3.0 {
        end(p);
        kit::back_to_pad(p);
    }
}

// ================================================================================================ MainProc

pub fn tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    if let Some(k) = kit::kit(p) {
        k.run.t += dt;
    }
    match kit::kit_state(p) {
        Some(DODGE) => dodge_tick(p, host, dt),
        Some(AIRSTEP) => airstep_tick(p),
        Some(BLOCK) => block_tick(p, host),
        Some(FLOAT) => float_tick(p, host, dt),
        Some(ATTUNE) => attune_tick(p, host, dt),
        _ => {}
    }
}

fn dodge_tick(p: &mut NpcPawn, host: &mut PartyHost, dt: f32) {
    let Some(k) = kit::kit_ref(p).cloned() else { return };
    let kind = dodge_of(&k);
    let (dist, time, _, recover) = kind.numbers();
    let t = k.run.t;
    if t <= time + 1e-4 && dt > 0.0 {
        let v = k.run.dir * (dist / time.max(0.01));
        p.cmc.direct_xy = Some(Vec2::new(v.x, v.y));
    } else if k.run.sub < 200 && !k.run.done {
        p.cmc.stop_movement();
        p.cmc.direct_xy = Some(Vec2::ZERO);
        if kind == Dodge::Blink {
            let at = p.position() + Vec3::Z * 90.0;
            let h = shots::effect(p, host, K.blink_out_fx, at, 1.0);
            if let Some(t) = crate::donald::magic_fx::fx::kit_tint("blink") {
                host.effects.tint(EffectHandle(h), t); // (DON-MAGIC tint) [R] yellow
            }
        }
        if kind == Dodge::Glide {
            p.anim.play(super::donald_combo::clip_copy("SlideEnd", true), 0.08);
        }
        if let Some(kk) = kit::kit(p) {
            kk.run.done = true;
        }
    }
    match kind {
        Dodge::Glide => {
            if t > 0.15 && t <= time && p.anim.clip.name == "SonicSlide" && p.anim.finished {
                p.anim.play_loop(super::donald_combo::clip_copy("SonicSlideLoop", true), 0.05);
            }
        }
        Dodge::Flame => {
            // the fire trail: a patch every `flame.trail_every` along the dash
            let trail = k.run.trail + dt;
            if t <= time && trail >= K.flame_trail_every {
                let mut a = Area::new(AreaKind::Hit, p.position(), K.flame_trail_radius, K.flame_trail_life, "DON_FLAME_STEP");
                (a.power, a.element, a.fx) = (K.flame_trail_power, Some(Element::Fire), K.flame_fx);
                a.hit_fx = K.missile_fire_hit_fx;
                shots::add_area(p, host, a);
                if let Some(kk) = kit::kit(p) {
                    kk.run.trail = 0.0;
                }
            } else if let Some(kk) = kit::kit(p) {
                kk.run.trail = trail;
            }
        }
        _ => {}
    }
    let end_at = if kind == Dodge::Glide { time + 0.53 } else { recover + 0.07 };
    if t >= end_at || (kind != Dodge::Glide && p.anim.finished && t >= recover) {
        end(p);
        kit::back_to_pad(p);
    }
}

fn airstep_tick(p: &mut NpcPawn) {
    let Some(k) = kit::kit_ref(p).cloned() else { return };
    let t = k.run.t;
    if t <= K.airstep_time {
        let v = k.run.dir * (K.airstep_distance / K.airstep_time.max(0.01));
        p.cmc.velocity = Vec3::new(v.x, v.y, 0.0);
        p.cmc.direct_z = Some(0.0);
    } else {
        end(p);
        p.cmc.velocity *= 0.3;
        if p.on_ground() {
            kit::back_to_pad(p);
        } else {
            super::enter_jump(p, true);
        }
    }
}

/// A move state ends (whatever took him out): hover off, rate back.
pub fn end(p: &mut NpcPawn) {
    if !matches!(kit::kit_state(p), Some(DODGE | AIRSTEP | BLOCK | FLOAT | ATTUNE)) {
        return;
    }
    p.anim.speed = 1.0;
    p.cmc.direct_xy = None;
    p.cmc.direct_z = None;
    if p.cmc.gravity_off {
        p.cmc.gravity_off = false;
        if !p.on_ground() {
            p.cmc.set_falling();
        }
    }
}

/// The state `n` was left: its looping sound and persistent effect stop.
pub fn left_state(p: &mut NpcPawn, host: &mut PartyHost, n: u8) {
    let (fx, snd) = kit::kit_ref(p).map_or((0, 0), |k| (k.owned_fx, k.owned_snd));
    if n == FLOAT {
        p.trav.pitch = 0.0; // the sway's rock off
    }
    if matches!(n, DODGE | FLOAT) {
        if fx != 0 {
            host.effects.stop(EffectHandle(fx), n == FLOAT); // the float's wind at once (as end_float)
        }
        if snd != 0 {
            host.audio.stop(SoundHandle(snd), if n == FLOAT { K.float_se_fade } else { 0.15 }); // (KITFX) the float's wind fades softly
        }
        for h in kit::kit_ref(p).map_or(Vec::new(), |k| k.owned_more.clone()) {
            if h != 0 {
                host.effects.stop(EffectHandle(h), n == FLOAT);
            }
        }
        if let Some(k) = kit::kit(p) {
            k.owned_fx = 0;
            k.owned_snd = 0;
            k.owned_more.clear();
        }
    }
    let _ = has(p, DonAbility::AeroFloat);
}
