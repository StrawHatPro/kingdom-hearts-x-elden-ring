//! (ZETTA) ZETTAFLARE AS THE KH3 CUTSCENE SHOWS IT, AIMED AT WHOEVER THE TARGET IS (the user, 2026-10-22: "I want
//! Zettaflare to look exactly like how it looks in the KH3 cutscene, but aimed at whoever the target is").
//!
//! THE SOURCE: the KH3 event `Cutscene/Localization/ja/kg/cutscene_kg01_kg702.umap`, matinee `kg702b` (the Keyblade
//! Graveyard: Donald casts Zettaflare at Terra-Xehanort), 58.767 .. 79.633 s; the user's GIF is that range. Everything the
//! event does for the spell is a row of `sheets/port/party-don-zetta-fx` (read from the event's data by
//! `build/sora_tmp/party/ZETTA/tools/make_zetta_sheets.js`): Donald's clips (n_ex001_70286wr the channel, n_ex001_70291w
//! the cast), every effect (the attach tracks on him / his staff / the target, the placed emitters with their trigger and
//! move tracks), the event's SE stream cut at its times, his voice lines. Times stay the MATINEE's; places are kept in a
//! frame of Donald (his feet, his facing) or of the target (its centre, facing away from Donald), so the event's layout is
//! rebuilt rigidly around any target at any distance.
//!
//! THE SEQUENCE (seconds from the cast's start, [`seq_time`]): the channel (matinee 58.767 .., the circle, the aura, the
//! channel clip) lasts `zetta.channel_time` (the user: 5 s; the event's channel events fit in `zetta.channel_span`, a
//! shorter channel compresses them); then the cast clip, "Zettaflare!" with his lip track on the face slot (crate::face; en
//! or ja as the voice), the release at the staff (`zetta.release_at`), the beam at the target and the hit (`zetta.hit_at`),
//! Donald free (`zetta.free_at`), the blast to its end (`zetta.end_at`).
//!
//! ROUND 2 (the user, 2026-10-22):
//! - THE CINEMATIC CAMERA ("there is a cinematic camera that mimics the cutscene"): the event's own camera
//!   (`sheets/port/party-don-zetta-cam`, TresMBCameraData_3 of kg702: Donald's shots 860 / 890 / 910, the target's 920 /
//!   930 / 940; the Mickey / Goofy reactions left out) rebuilt in Donald's and the target's frames, on the sequence's time;
//!   both hosts draw it through kh-partyhost `team_camera` ([`Party::don_zetta_view`]); blends in, hands back smoothly at
//!   the end, at once when he is interrupted or KO; a held camera-stick push hands back early (the pad always works); ctl
//!   / sheet `zetta.cinematic` (default on).
//! - ROOTED ("Donald cannot move while casting Zettaflare"): from the channel's start to the cast clip's end no stick
//!   movement, no turning but to face the target, no jump or dodge ([`hold`] from pad::pre_move, [`pin`] for every
//!   member); a hit still interrupts it; control comes back at the cast's end.
//! - THE ULTIMATES ("His ultimate level magic should have the same casting circle and Donald's casting animation as
//!   Zettaflare, without the cinematic camera. The circle and all effects associated with it should be the colour of the
//!   element!"): [`ultimate_begin`]: the circle and its light, the aura glows, the hand light, the staff charge and release
//!   flash, his channel / cast clips and his effort, tinted to the element, the whole cast scaled so the release lands at
//!   `ult.channel_time` (3 s); DON-SPELLB keeps the MP, the payload and its shot (at the returned time) and calls
//!   [`ultimate_cancel`] from its interrupt.
//!
//! THE AIM: his lock-on target, else the soft target, else the target the caller gave (AI Donald), else a point
//! `zetta.ahead` in front of him; re-read every time something is placed, so the beam goes where the target IS.
//!
//! WHO CALLS: `cast::begin` hands a Zettaflare cast to [`begin`] (the one marked edit in DON-MAGIC's cast.rs); `Party::step`
//! calls [`party_step`] right after `cast::party_step` (party.rs, marked); pad::pre_move calls [`hold`] (marked); DON-SPELLB
//! calls [`ultimate_begin`]. Frame-rate independent: every timer counts the step's seconds; moving emitters and the camera
//! are placed from their tracks at the sequence's time.

use super::cast::{CastRun, CastTarget};
use super::combat::CastPlan;
use super::element::Element;
use super::magic_fx::{self, fx, ActorKind, Aim, Burst, FxLog, HitSpec};
use super::spells::{ArcaneLine, SpellRef};
use super::tune::data;
use crate::anim::ClipDef;
use crate::host::PartyHost;
use crate::ids::MemberId;
use crate::party::Party;
use crate::pawn::NpcPawn;
use crate::states::NpcStateId;
use glam::{Vec2, Vec3};
use kh_core::enums::{ETresDamageAttribute as Attr, ETresDamageKind as Kind};
use kh_core::host::{Attach, EffectHandle, EffectId, EffectSpawn, PartyMember, SoundHandle, SoundId, SoundPlay, TargetFlags, TargetId, Tint};
use std::sync::atomic::{AtomicBool, AtomicI8, Ordering};
use std::sync::{Arc, RwLock};

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_don_zetta.rs"));
include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_don_zetta_fx.rs"));
include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_don_zetta_cam.rs"));

/// The numbers (sheet `party-don-zetta`).
pub type ZettaTune = DonZettaParams;

static TUNE: RwLock<Option<Arc<ZettaTune>>> = RwLock::new(None);

/// The numbers the sequence reads (the compiled sheet until a host [`install_zetta`]s its own).
pub fn zetta() -> Arc<ZettaTune> {
    if let Some(t) = TUNE.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return t.clone();
    }
    let mut w = TUNE.write().unwrap_or_else(|e| e.into_inner());
    w.get_or_insert_with(|| Arc::new(DON_ZETTA_PARAMS)).clone()
}

/// The host's numbers (the sheet read again, `don.zetta.<row> = value` ctl keys).
pub fn install_zetta(t: ZettaTune) {
    *TUNE.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(t));
}

/// One number by its row id (`zetta.channel_time`, `ult.channel_time`, also with `don.` before) or field name; false when
/// it names nothing.
pub fn set_zetta(t: &mut ZettaTune, key: &str, v: f32) -> bool {
    let k = key.trim();
    let k = k.strip_prefix("don.").unwrap_or(k).replace(['.', '-'], "_").to_ascii_lowercase();
    let k = if k.starts_with("zetta_") || k.starts_with("ult_") { k } else { format!("zetta_{k}") };
    let f: &mut f32 = match k.as_str() {
        "zetta_channel_start" => &mut t.zetta_channel_start,
        "zetta_cast_start" => &mut t.zetta_cast_start,
        "zetta_channel_time" => &mut t.zetta_channel_time,
        "zetta_channel_span" => &mut t.zetta_channel_span,
        "zetta_release_at" => &mut t.zetta_release_at,
        "zetta_hit_at" => &mut t.zetta_hit_at,
        "zetta_free_at" => &mut t.zetta_free_at,
        "zetta_end_at" => &mut t.zetta_end_at,
        "zetta_target_center_height" => &mut t.zetta_target_center_height,
        "zetta_ahead" => &mut t.zetta_ahead,
        "zetta_ahead_height" => &mut t.zetta_ahead_height,
        "zetta_turn_rate" => &mut t.zetta_turn_rate,
        "zetta_blend_in" => &mut t.zetta_blend_in,
        "zetta_face_blend" => &mut t.zetta_face_blend,
        "zetta_cinematic" => &mut t.zetta_cinematic,
        "zetta_cinematic_ai" => &mut t.zetta_cinematic_ai,
        "zetta_cam_blend_in" => &mut t.zetta_cam_blend_in,
        "zetta_cam_blend_out" => &mut t.zetta_cam_blend_out,
        "zetta_cam_skip_stick" => &mut t.zetta_cam_skip_stick,
        "zetta_cam_skip_hold" => &mut t.zetta_cam_skip_hold,
        "ult_channel_time" => &mut t.ult_channel_time,
        "ult_tint_intensity" => &mut t.ult_tint_intensity,
        "ult_flash_time" => &mut t.ult_flash_time, // (round 5c)
        "zetta_hold_turn_rate" => &mut t.zetta_hold_turn_rate,
        "zetta_react_push" => &mut t.zetta_react_push,
        "zetta_react_kind" => &mut t.zetta_react_kind,
        "zetta_finale_immerse_at" => &mut t.zetta_finale_immerse_at,
        "zetta_finale_immerse_time" => &mut t.zetta_finale_immerse_time,
        "zetta_finale_crumble_at" => &mut t.zetta_finale_crumble_at,
        "zetta_finale_crumble_time" => &mut t.zetta_finale_crumble_time,
        "zetta_finale_dark_time" => &mut t.zetta_finale_dark_time, // (round 5c)
        "zetta_hold_until" => &mut t.zetta_hold_until,
        "zetta_beam_scale_min" => &mut t.zetta_beam_scale_min, // (2026-10-23)
        "zetta_cam_floor_clear" => &mut t.zetta_cam_floor_clear, // (2026-10-23)
        "zetta_frame_body_height" => &mut t.zetta_frame_body_height, // (2026-10-23)
        "zetta_frame_body_width" => &mut t.zetta_frame_body_width, // (2026-10-23)
        "zetta_beam_fill_back" => &mut t.zetta_beam_fill_back, // (2026-10-23)
        "zetta_frame_share_height" => &mut t.zetta_frame_share_height, // (2026-10-23)
        "zetta_frame_share_width" => &mut t.zetta_frame_share_width, // (2026-10-23)
        "zetta_beam_card_floor" => &mut t.zetta_beam_card_floor, // (2026-10-23)
        "zetta_beam_fill_width" => &mut t.zetta_beam_fill_width, // (2026-10-23)
        "zetta_down_time" => &mut t.zetta_down_time, // (2026-10-23)
        "zetta_down_blend" => &mut t.zetta_down_blend, // (2026-10-23)
        "zetta_down_fall_a" => &mut t.zetta_down_fall_a, // (2026-10-23)
        "zetta_down_fall_b" => &mut t.zetta_down_fall_b, // (2026-10-23)
        "zetta_down_up_from" => &mut t.zetta_down_up_from, // (2026-10-23)
        "zetta_down_up_voice" => &mut t.zetta_down_up_voice, // (2026-10-23)
        "zetta_staff_release" => &mut t.zetta_staff_release, // (2026-10-23)
        "zetta_sweep_radius" => &mut t.zetta_sweep_radius, // (2026-10-23)
        "zetta_sweep_push" => &mut t.zetta_sweep_push, // (2026-10-23)
        "zetta_sweep_out" => &mut t.zetta_sweep_out, // (2026-10-23)
        "zetta_down_up_blend" => &mut t.zetta_down_up_blend, // (2026-10-23)
        "zetta_regen_mult" => &mut t.zetta_regen_mult, // (2026-10-23)
        "zetta_end_throw_at" => &mut t.zetta_end_throw_at, // (round 5c)
        "zetta_blast_radius" => &mut t.zetta_blast_radius,
        "zetta_splash_share" => &mut t.zetta_splash_share,
        "zetta_ally_clear" => &mut t.zetta_ally_clear,
        "zetta_ally_beam" => &mut t.zetta_ally_beam,
        "zetta_ally_margin" => &mut t.zetta_ally_margin,
        "zetta_cam_fit_margin" => &mut t.zetta_cam_fit_margin,
        "zetta_cam_fov_max" => &mut t.zetta_cam_fov_max,
        "zetta_cam_sep_max" => &mut t.zetta_cam_sep_max,
        "zetta_cam_block_share" => &mut t.zetta_cam_block_share,
        "zetta_cam_safe_back" => &mut t.zetta_cam_safe_back,
        "zetta_cam_safe_side" => &mut t.zetta_cam_safe_side,
        "zetta_cam_safe_up" => &mut t.zetta_cam_safe_up,
        // (round 5) the ending, the white, the key light
        "zetta_ending" => &mut t.zetta_ending,
        "zetta_beam_beyond" => &mut t.zetta_beam_beyond,
        "zetta_ally_donald" => &mut t.zetta_ally_donald,
        "zetta_cam_target_one_shot" => &mut t.zetta_cam_target_one_shot,
        "zetta_cam_target_low" => &mut t.zetta_cam_target_low,
        "zetta_cam_beam_clear" => &mut t.zetta_cam_beam_clear,
        "zetta_cam_core_clear" => &mut t.zetta_cam_core_clear, // (round 5c)
        "zetta_ally_target_clear" => &mut t.zetta_ally_target_clear, // (round 5c)
        "zetta_ally_wait_back" => &mut t.zetta_ally_wait_back, // (round 5c)
        "zetta_spikes" => &mut t.zetta_spikes, // (round 5c)
        "zetta_spikes_ground_max" => &mut t.zetta_spikes_ground_max, // (round 5c)
        "zetta_spikes_count" => &mut t.zetta_spikes_count, // (round 5c)
        "zetta_spikes_radius" => &mut t.zetta_spikes_radius, // (round 5c)
        "zetta_spikes_spread" => &mut t.zetta_spikes_spread, // (round 5c)
        "zetta_spikes_height" => &mut t.zetta_spikes_height, // (round 5c)
        "zetta_spikes_width" => &mut t.zetta_spikes_width, // (round 5c)
        "zetta_spikes_lean" => &mut t.zetta_spikes_lean, // (round 5c)
        "zetta_spikes_sink" => &mut t.zetta_spikes_sink, // (round 5c)
        "zetta_spikes_clear" => &mut t.zetta_spikes_clear, // (round 5c)
        "zetta_spikes_crumble_at" => &mut t.zetta_spikes_crumble_at, // (round 5c)
        "zetta_spikes_stagger" => &mut t.zetta_spikes_stagger, // (round 5c)
        "zetta_spikes_crumble_time" => &mut t.zetta_spikes_crumble_time, // (round 5c)
        "zetta_spikes_wide_scale" => &mut t.zetta_spikes_wide_scale, // (round 5c)
        "zetta_spikes_crack_time" => &mut t.zetta_spikes_crack_time, // (round 5c)
        "zetta_spikes_fall_time" => &mut t.zetta_spikes_fall_time, // (round 5c)
        "zetta_spikes_fall_out" => &mut t.zetta_spikes_fall_out, // (round 5c)
        "zetta_spikes_bounce" => &mut t.zetta_spikes_bounce, // (round 5c)
        "zetta_spikes_settle_at" => &mut t.zetta_spikes_settle_at, // (round 5c)
        "zetta_spikes_rock_crumble" => &mut t.zetta_spikes_rock_crumble, // (round 5c)
        "zetta_aura_size" => &mut t.zetta_aura_size, // (round 5c)
        "zetta_dust" => &mut t.zetta_dust, // (round 5c)
        "zetta_dust_step" => &mut t.zetta_dust_step, // (round 5c)
        "zetta_dust_lane" => &mut t.zetta_dust_lane, // (round 5c)
        "zetta_dust_max" => &mut t.zetta_dust_max, // (round 5c)
        "zetta_dust_reach" => &mut t.zetta_dust_reach, // (round 5c)
        "zetta_dust_stop_at" => &mut t.zetta_dust_stop_at, // (round 5c)
        "zetta_dust_wide_stop_at" => &mut t.zetta_dust_wide_stop_at, // (round 5c)
        "zetta_dust_near_at" => &mut t.zetta_dust_near_at, // (round 5c)
        "zetta_dust_c100_waves" => &mut t.zetta_dust_c100_waves, // (round 5c)
        "zetta_dust_c100_every" => &mut t.zetta_dust_c100_every, // (round 5c)
        "zetta_launch_travel" => &mut t.zetta_launch_travel, // (round 5c)
        "zetta_launch_kick_deg" => &mut t.zetta_launch_kick_deg, // (round 5c)
        "zetta_launch_kick_time" => &mut t.zetta_launch_kick_time, // (round 5c)
        "zetta_spikes_rise_at" => &mut t.zetta_spikes_rise_at, // (round 5c)
        "zetta_spikes_rise_stagger" => &mut t.zetta_spikes_rise_stagger, // (round 5c)
        "zetta_spikes_rise_time" => &mut t.zetta_spikes_rise_time, // (round 5c)
        "zetta_aura_front" => &mut t.zetta_aura_front, // (round 5c)
        "zetta_shake" => &mut t.zetta_shake, // (round 5c)
        "zetta_shake_deg" => &mut t.zetta_shake_deg, // (round 5c)
        "zetta_shake_cm" => &mut t.zetta_shake_cm, // (round 5c)
        "zetta_finale_beam_glow" => &mut t.zetta_finale_beam_glow, // (round 5c)
        "zetta_sound_fade" => &mut t.zetta_sound_fade, // (round 5c)
        "zetta_spikes_trail_max" => &mut t.zetta_spikes_trail_max, // (round 5c)
        "zetta_spikes_trail_step" => &mut t.zetta_spikes_trail_step, // (round 5c)
        "zetta_spikes_trail_lane" => &mut t.zetta_spikes_trail_lane, // (round 5c)
        "zetta_throw_unveil" => &mut t.zetta_throw_unveil,
        "zetta_beam_girth_margin" => &mut t.zetta_beam_girth_margin,
        "zetta_cam_target_look" => &mut t.zetta_cam_target_look,
        "zetta_beam_width0" => &mut t.zetta_beam_width0,
        "zetta_beam_card_in" => &mut t.zetta_beam_card_in,
        "zetta_beam_card_out" => &mut t.zetta_beam_card_out,
        "zetta_cam_subject_min" => &mut t.zetta_cam_subject_min,
        "zetta_end_start" => &mut t.zetta_end_start,
        "zetta_end_handoff_at" => &mut t.zetta_end_handoff_at,
        "zetta_end_rehold_delay" => &mut t.zetta_end_rehold_delay,
        "zetta_finale_fade_at" => &mut t.zetta_finale_fade_at,
        "zetta_finale_fade_time" => &mut t.zetta_finale_fade_time,
        "zetta_white_in_at" => &mut t.zetta_white_in_at,
        "zetta_white_in_time" => &mut t.zetta_white_in_time,
        "zetta_white_out_time" => &mut t.zetta_white_out_time,
        _ => return false,
    };
    *f = v;
    true
}

/// Every row id of the number sheet (a host's key listing).
pub fn zetta_keys() -> impl Iterator<Item = &'static str> {
    DON_ZETTA_PARAM_ROWS.iter().map(|r| r.1)
}

/// The host's `zetta.cinematic` (ctl key, Elden Ring: kh-er party frame; the test level: `party zetta-cinematic on|off`):
/// `None` = the sheet's row.
static CINEMATIC: AtomicI8 = AtomicI8::new(-1);

pub fn set_cinematic(on: Option<bool>) {
    CINEMATIC.store(on.map_or(-1, |b| b as i8), Ordering::Relaxed);
}

/// The cinematic camera is on (the host's key, else the sheet).
pub fn cinematic_on(z: &ZettaTune) -> bool {
    cinematic_of(CINEMATIC.load(Ordering::Relaxed), z)
}

/// The rule: the host's key (-1 none, 0 off, 1 on) over the sheet's row.
pub fn cinematic_of(key: i8, z: &ZettaTune) -> bool {
    match key {
        -1 => z.zetta_cinematic >= 0.5,
        v => v == 1,
    }
}

/// (round 5) The host's `zetta.ending` (the test level: `party zetta-ending on|off`; tests): `None` = the sheet's row.
static ENDING: AtomicI8 = AtomicI8::new(-1);

pub fn set_ending(on: Option<bool>) {
    ENDING.store(on.map_or(-1, |b| b as i8), Ordering::Relaxed);
}

/// The ending (kg702c) plays after the blast: the host's key, else the sheet's `zetta.ending`.
pub fn ending_on(z: &ZettaTune) -> bool {
    match ENDING.load(Ordering::Relaxed) {
        -1 => z.zetta_ending >= 0.5,
        v => v == 1,
    }
}

/// (round 5) A Zettaflare asked VFXR's cinematic look and has not reset it yet (a party with no run left resets it: a load, a
/// party rebuilt mid-spell).
static LOOK_OWNED: AtomicBool = AtomicBool::new(false);
/// When a run holding the look last stepped (real time: another party stepping meanwhile must not take it away).
static LOOK_BEAT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

fn look_beat() {
    if let Ok(mut b) = LOOK_BEAT.lock() {
        *b = Some(std::time::Instant::now());
    }
}

fn look_stale() -> bool {
    LOOK_BEAT.lock().map_or(true, |b| b.map_or(true, |t| t.elapsed().as_secs_f32() > 0.5))
}

/// (round 5) The run gives the look back (`kh_core::host::cinematic::reset` over `blend` s): its end, an interrupt, his KO, an
/// early hand-back.
fn end_look(run: &mut ZettaRun, blend: f32) {
    if run.look {
        run.look = false;
        run.asked = Some((kh_core::host::cinematic::Look::NEUTRAL, blend.max(0.0)));
        kh_core::host::cinematic::reset(blend.max(0.0));
        LOOK_OWNED.store(false, Ordering::Relaxed);
    }
}

/// (round 5) THE EVENT'S WHITE at a matinee time (the hosts draw [`ZettaView::white`] over the picture): kg702b's
/// TresInterpTrackFade (white, 0 at `zetta.white_in_at` -> 1 a `zetta.white_in_time` later, held) and kg702c's (1 at its
/// start `zetta.end_start` -> 0 over `zetta.white_out_time`); with the ending only (without it the camera hands back
/// from the blast, not from white).
pub fn white_at(mt: f32, z: &ZettaTune) -> f32 {
    if !ending_on(z) || mt < z.zetta_white_in_at {
        return 0.0;
    }
    if mt < z.zetta_end_start {
        return ((mt - z.zetta_white_in_at) / z.zetta_white_in_time.max(1e-3)).clamp(0.0, 1.0);
    }
    1.0 - ((mt - z.zetta_end_start) / z.zetta_white_out_time.max(1e-3)).clamp(0.0, 1.0)
}

/// The game's voice language (the hosts: kh-er `snd.lang`, kh-app `--voice-language`): the lip track follows the voice.
static VOICE_JA: AtomicBool = AtomicBool::new(false);

pub fn set_voice_ja(ja: bool) {
    VOICE_JA.store(ja, Ordering::Relaxed);
}

pub fn voice_ja() -> bool {
    VOICE_JA.load(Ordering::Relaxed)
}

/// The event's channel clip (3.0 s; each of the event's shots restarts it: a loop) and its cast clip (3.1 s), packed on
/// his gameplay skeleton (ZETTA/pack/party/n_ex001/anims, the rows of manifest_items.json).
pub static CHANNEL_CLIP: ClipDef = ClipDef::bare("n_ex001_70286wr", 3.0, true);
pub static CAST_CLIP: ClipDef = ClipDef::bare("n_ex001_70291w", 3.1, false);
/// (round 5) His pose under the shrinking beam in the ending (kg702c [FullBody] n_ex001_70297wr, 4.733 s, played again
/// every 4.7 s: a loop), until the hand-off.
pub static END_CLIP: ClipDef = ClipDef::bare("n_ex001_70297wr", 4.733, true);
/// (round 5) The ending's shots (kg702c 960 the wide shot of the beam, 970 behind Donald): the event's exactly, in the frame
/// `end` (his feet at the ending, X along the beam's line); a wall only pulls them in.
pub const END_SHOTS: [&str; 2] = ["shot960", "shot970"];

/// His face (lip) track of the cast, as the voice that plays: the English (`face/en/n_ex001_70291w`) or the Japanese
/// (`face/ja/n_ex001_70291w`) lip version of the event, packed as face clips (ZETTA/pack/party/n_ex001/face).
pub const FACE_EN: &str = "n_ex001_70291w_face";
pub const FACE_JA: &str = "n_ex001_70291w_face_ja";
pub const FACE_LENGTH: f32 = 3.1;
/// (round 5) HIS FACE THROUGH THE CHANNEL (the user: "Donald's eyes need to be closed when he's charging up the zettaflare"):
/// the event's own face track of the charging shot 860 (kg702b TresInterpGroup_13: the eye row n_ex001_fe202, eyes shut, played
/// again every 0.333 s; the lip row n_ex001_fm200), merged into one face clip (ZETTA/tools/zetta_clips.js), a 1 s loop on his
/// face slot from the channel's start to the cast (the cast's own face track, eyes open, takes over); no blink between.
pub const CHANNEL_FACE: &str = "n_ex001_70286_face";
pub const CHANNEL_FACE_LENGTH: f32 = 1.0;

pub fn face_clip(ja: bool) -> &'static str {
    if ja {
        FACE_JA
    } else {
        FACE_EN
    }
}

/// The event's Donald faces 1.984 deg left of the line to his target (his yaw -57.221, the line -55.236): the Donald
/// frame of the placed emitters and the camera is the line to the target plus this (so a Donald still turning to it does
/// not swing the circle or the shots).
pub const EVENT_DONALD_YAW_OFF: f32 = -1.984;

/// Zettaflare is this module's.
pub fn takes(spell: SpellRef) -> bool {
    matches!(spell, SpellRef::Arcane(ArcaneLine::Zettaflare, _))
}

// ============================================================================================== the pure rules

/// THE SEQUENCE'S TIME of a matinee time: the channel part ([channel_start, cast_start)) keeps its times when the channel
/// is at least `zetta.channel_span` long (later ones happen at its end), else it is compressed by channel / span; the cast
/// part follows the channel.
pub fn seq_time(t: f32, z: &ZettaTune, channel: f32) -> f32 {
    let channel = channel.max(0.0);
    if t < z.zetta_cast_start - 1e-4 {
        ((t - z.zetta_channel_start) * squeeze(z, channel)).clamp(0.0, channel)
    } else {
        channel + (t - z.zetta_cast_start)
    }
}

/// The channel part's time factor: 1, or channel / span for a channel shorter than its events' span.
pub fn squeeze(z: &ZettaTune, channel: f32) -> f32 {
    if z.zetta_channel_span > 1e-3 && channel < z.zetta_channel_span {
        (channel / z.zetta_channel_span).max(1e-3)
    } else {
        1.0
    }
}

/// The matinee time a sequence time stands for (the inverse of [`seq_time`]).
pub fn matinee_time(s: f32, z: &ZettaTune, channel: f32) -> f32 {
    let channel = channel.max(0.0);
    if s < channel {
        z.zetta_channel_start + s / squeeze(z, channel)
    } else {
        z.zetta_cast_start + (s - channel)
    }
}

/// A frame: an origin and a facing (UE axes: X forward, Y right, Z up; yaw about Z from +X toward +Y, degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Vec3,
    pub yaw: f32,
}

impl Frame {
    /// A point given in the frame, in the world.
    pub fn point(&self, local: Vec3) -> Vec3 {
        self.origin + self.vector(local)
    }

    /// A direction given in the frame, in the world.
    pub fn vector(&self, local: Vec3) -> Vec3 {
        let (s, c) = self.yaw.to_radians().sin_cos();
        Vec3::new(local.x * c - local.y * s, local.x * s + local.y * c, local.z)
    }

    /// A rotation given in the frame (pitch, yaw, roll), in the world: the frame's yaw is added (outermost in UE's order).
    pub fn rotation(&self, local: Vec3) -> Vec3 {
        Vec3::new(local.x, crate::pawn::wrap180(local.y + self.yaw), local.z)
    }
}

/// (round 3) THE BEAM FRAME (the user, 2026-10-22: "the zettaflare beam itself and the cinematic camera need to be
/// directed AT the target"): a 3D frame (UE axes) whose origin is the target's centre and whose X runs from Donald's staff
/// (the release point) to it IN 3D, its pitch included; Y is horizontal, to the right of X; Z = X x Y. The event's
/// target-side effects and shots are kept in the event's own beam frame (`make_zetta_sheets.js` / `make_zetta_cam.js`), so
/// in play they turn and tilt with the real line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Basis {
    pub origin: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl Basis {
    /// The beam frame from the staff to the target's centre.
    pub fn beam(staff: Vec3, target: Vec3) -> Basis {
        let d = target - staff;
        let x = if d.length() > 1e-3 { d.normalize() } else { Vec3::X };
        let h = (x.x * x.x + x.y * x.y).sqrt();
        let y = if h > 1e-4 { Vec3::new(-x.y / h, x.x / h, 0.0) } else { Vec3::Y };
        Basis { origin: target, x, y, z: x.cross(y) }
    }

    pub fn point(&self, l: Vec3) -> Vec3 {
        self.origin + self.vector(l)
    }

    pub fn vector(&self, l: Vec3) -> Vec3 {
        self.x * l.x + self.y * l.y + self.z * l.z
    }

    /// A point of the world in the frame.
    pub fn local(&self, p: Vec3) -> Vec3 {
        let d = p - self.origin;
        Vec3::new(d.dot(self.x), d.dot(self.y), d.dot(self.z))
    }

    /// A rotator (pitch, yaw, roll) given in the frame, in the world.
    pub fn rotation(&self, local: Vec3) -> Vec3 {
        let [a, b, c] = rot_axes(local);
        axes_rot(self.vector(a), self.vector(b), self.vector(c))
    }
}

/// UE's FRotationMatrix: the X, Y, Z axes of a rotator (pitch, yaw, roll) in degrees.
pub fn rot_axes(r: Vec3) -> [Vec3; 3] {
    let (sp, cp) = r.x.to_radians().sin_cos();
    let (sy, cy) = r.y.to_radians().sin_cos();
    let (sr, cr) = r.z.to_radians().sin_cos();
    [Vec3::new(cp * cy, cp * sy, sp), Vec3::new(sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp), Vec3::new(-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp)]
}

/// The rotator (pitch, yaw, roll) of three axes (the inverse of [`rot_axes`]).
pub fn axes_rot(x: Vec3, _y: Vec3, z: Vec3) -> Vec3 {
    let pitch = x.z.clamp(-1.0, 1.0).asin().to_degrees();
    let yaw = x.y.atan2(x.x).to_degrees();
    let [_, y0, z0] = rot_axes(Vec3::new(pitch, yaw, 0.0));
    Vec3::new(pitch, yaw, z.dot(y0).atan2(z.dot(z0)).to_degrees())
}

fn v3(a: &[f32]) -> Vec3 {
    Vec3::new(a.first().copied().unwrap_or(0.0), a.get(1).copied().unwrap_or(0.0), a.get(2).copied().unwrap_or(0.0))
}

/// A row's place in its frame at matinee time `t`: its start offset, then its move track (linear between keys, held
/// after the last).
pub fn path_at(row: &DonZettaFxRow, t: f32) -> Vec3 {
    let mut prev = (row.start, v3(row.offset));
    for k in row.path.chunks_exact(4) {
        let next = (k[0], Vec3::new(k[1], k[2], k[3]));
        if t <= next.0 {
            let span = (next.0 - prev.0).max(1e-4);
            let a = ((t - prev.0) / span).clamp(0.0, 1.0);
            return prev.1.lerp(next.1, a);
        }
        prev = next;
    }
    prev.1
}

/// WHERE THE SPELL GOES: the lock-on target, else the soft target, else the caller's target, all only while alive;
/// else `ahead`: a point `zetta.ahead` in front of him at `zetta.ahead_height` (the event's target stood 8.25 m away).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ZettaAim {
    Target(TargetId),
    Point(Vec3),
}

pub fn pick_aim(lock: TargetId, soft: TargetId, given: CastTarget, alive: &dyn Fn(TargetId) -> bool, ahead: Vec3) -> ZettaAim {
    for t in [lock, soft] {
        if t != TargetId::NONE && alive(t) {
            return ZettaAim::Target(t);
        }
    }
    match given {
        CastTarget::Enemy(t) if t != TargetId::NONE && alive(t) => ZettaAim::Target(t),
        // a point the caller chose (its own no-target rule) or an ally: in front of him, at a body's height
        _ => ZettaAim::Point(ahead),
    }
}

/// What a run is: the Zettaflare, or an ultimate cast's presentation in its element's tint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RunKind {
    Zetta,
    Ult { tint: Tint },
}

impl RunKind {
    pub fn is_ult(self) -> bool {
        matches!(self, RunKind::Ult { .. })
    }

    /// The channel's length on the event's clock: Zettaflare's `zetta.channel_time` (5 s); an ultimate's the channel's
    /// whole span (`zetta.channel_span`), the whole sequence then scaled by [`RunKind::scale`].
    pub fn channel(self, z: &ZettaTune) -> f32 {
        match self {
            RunKind::Zetta => z.zetta_channel_time.max(0.0),
            RunKind::Ult { .. } => z.zetta_channel_span.max(0.0),
        }
    }

    /// The sequence's time scale: 1 (Zettaflare); an ultimate's: the whole cast scaled so the release lands at
    /// `ult.channel_time` (the user: 3 s), clips played faster by as much.
    pub fn scale(self, z: &ZettaTune) -> f32 {
        match self {
            RunKind::Zetta => 1.0,
            RunKind::Ult { .. } => {
                let natural = seq_time(z.zetta_release_at, z, self.channel(z)).max(1e-3);
                (z.ult_channel_time.max(0.05) / natural).max(1e-3)
            }
        }
    }

    /// A matinee time on this run's clock (seconds from the cast's start).
    pub fn seq(self, t: f32, z: &ZettaTune, channel: f32) -> f32 {
        seq_time(t, z, channel) * self.scale(z)
    }

    /// The sequence's end: the blast's (Zettaflare), the release flash's (an ultimate: the cast clip's end).
    pub fn end(self, z: &ZettaTune, channel: f32) -> f32 {
        match self {
            // (round 5) with the ending: the hand-off (the shot behind Donald, `zetta.end_handoff_at`)
            RunKind::Zetta if ending_on(z) => seq_time(z.zetta_end_handoff_at.max(z.zetta_end_at), z, channel),
            RunKind::Zetta => seq_time(z.zetta_end_at, z, channel),
            RunKind::Ult { .. } => DON_ZETTA_FX.iter().filter(|r| r.use_ == 1 && r.ult == 1 && r.end >= 0.0).map(|r| self.seq(r.end, z, channel)).fold(self.seq(z.zetta_free_at, z, channel), f32::max),
        }
    }
}

/// The rows a run plays (use = 1; an ultimate: ult = 1 too), with their start / end times on the run's clock.
pub fn schedule(z: &ZettaTune, channel: f32, kind: RunKind) -> Vec<(usize, f32, f32)> {
    DON_ZETTA_FX
        .iter()
        .enumerate()
        .filter(|(_, r)| r.use_ == 1 && (!kind.is_ult() || r.ult == 1))
        // (round 5) the ending's rows (`end.*`, kg702c) with the ending on only
        .filter(|(_, r)| !r.id.starts_with("end.") || (kind == RunKind::Zetta && ending_on(z)))
        .map(|(i, r)| (i, kind.seq(r.start, z, channel), if r.end < 0.0 { f32::INFINITY } else { kind.seq(r.end, z, channel) }))
        .collect()
}

/// An element's colour (sheet party-don-core `color.<element>`, 0xRRGGBB): Fire red-orange, Blizzard ice blue, Thunder
/// yellow, Water aqua blue, Aero green, Light gold-white.
pub fn element_color(e: Element) -> u32 {
    e.color(&data().tune)
}

/// The tint of an ultimate's effects: the colour at full brightness (its largest channel 1: kh_core `Tint` keeps each
/// pixel's brightness and replaces its hue, so the effect keeps its own brightness and contrast), x `ult.tint_intensity`.
/// [`tint_of`] an element's colour (what DON-SPELLB hands [`ultimate_begin`]).
pub fn element_tint(e: Element) -> Tint {
    tint_of(element_color(e), &zetta())
}

pub fn tint_of(color: u32, z: &ZettaTune) -> Tint {
    let c = [((color >> 16) & 0xFF) as f32 / 255.0, ((color >> 8) & 0xFF) as f32 / 255.0, (color & 0xFF) as f32 / 255.0];
    let m = c[0].max(c[1]).max(c[2]).max(1e-3);
    Tint { color: [c[0] / m, c[1] / m, c[2] / m], intensity: z.ult_tint_intensity.max(0.0) }
}

// ============================================================================================== the camera (pure)

/// One shot of the event's camera: its keys (matinee s, position, rotation (pitch, yaw, roll), FOV), its frame
/// (`donald`: his feet and the line's yaw; `beam`: the beam frame; `target`: round 1's yaw-only target frame).
#[derive(Clone, Debug, PartialEq)]
pub struct CamShot {
    pub name: &'static str,
    pub frame: &'static str,
    pub channel: bool,
    pub keys: Vec<(f32, Vec3, Vec3, f32)>,
}

impl CamShot {
    pub fn start(&self) -> f32 {
        self.keys.first().map_or(0.0, |k| k.0)
    }

    pub fn stop(&self) -> f32 {
        self.keys.last().map_or(0.0, |k| k.0)
    }

    /// The camera in the shot's frame at matinee time `t` (linear between keys, held outside).
    pub fn at(&self, t: f32) -> (Vec3, Vec3, f32) {
        let k = &self.keys;
        if k.is_empty() {
            return (Vec3::ZERO, Vec3::ZERO, 45.0);
        }
        if t <= k[0].0 {
            return (k[0].1, k[0].2, k[0].3);
        }
        for w in k.windows(2) {
            if t <= w[1].0 {
                let a = ((t - w[0].0) / (w[1].0 - w[0].0).max(1e-5)).clamp(0.0, 1.0);
                let rot = Vec3::new(w[0].2.x + (w[1].2.x - w[0].2.x) * a, w[0].2.y + crate::pawn::wrap180(w[1].2.y - w[0].2.y) * a, w[0].2.z + (w[1].2.z - w[0].2.z) * a);
                return (w[0].1.lerp(w[1].1, a), rot, w[0].3 + (w[1].3 - w[0].3) * a);
            }
        }
        let l = k.last().unwrap();
        (l.1, l.2, l.3)
    }
}

/// The shots of the camera sheet, in its order (860, 890 the channel; 910, 920, 930, 940 the cast).
pub fn cam_shots() -> Vec<CamShot> {
    let mut out: Vec<CamShot> = Vec::new();
    for r in DON_ZETTA_CAM {
        if out.last().map_or(true, |s| s.name != r.shot) {
            out.push(CamShot { name: r.shot, frame: r.frame, channel: r.part == "channel", keys: Vec::new() });
        }
        if let Some(s) = out.last_mut() {
            s.keys.push((r.time, v3(r.position), v3(r.rotation), r.fov));
        }
    }
    out
}

/// The camera at a sequence time: (world position, rotation (pitch, yaw, roll), horizontal FOV, the subject point the
/// collision pass sweeps from, the shot's name), and the SAFE framing a host uses when the level blocks the shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CamPose {
    pub position: Vec3,
    pub rotation: Vec3,
    pub fov: f32,
    pub pivot: Vec3,
    pub shot: &'static str,
}

/// (round 3) THE ALLIES' ZONE (the user: "allies should stay AWAY from the Zettaflare target"): while the target is held,
/// a disc round it (`zetta.ally_clear` x its size + its radius) and the beam's corridor from 1.5 m past Donald's staff to
/// it (`zetta.ally_beam` each side); horizontal (the AI walks the floor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvoidZone {
    pub at: Vec3,
    pub radius: f32,
    pub from: Vec3,
    pub to: Vec3,
    pub half_width: f32,
    pub margin: f32,
    /// The caster (his own place is never in it).
    pub caster: MemberId,
}

fn flat2(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.y)
}

impl AvoidZone {
    /// How far `p` is inside the zone (> 0 inside), and the horizontal way out of it (unit).
    pub fn depth(&self, p: Vec3) -> (f32, Vec2) {
        let q = flat2(p);
        let c = flat2(self.at);
        let d = q - c;
        let disc = (self.radius - d.length(), if d.length() > 1e-3 { d.normalize() } else { Vec2::X });
        let (a, b) = (flat2(self.from), flat2(self.to));
        let ab = b - a;
        let t = if ab.length_squared() > 1e-3 { ((q - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
        let near = a + ab * t;
        let off = q - near;
        let side = if off.length() > 1e-3 { off.normalize() } else { Vec2::new(-ab.y, ab.x).normalize_or_zero() };
        let lane = if ab.length_squared() > 1e-3 && t > 0.0 && t < 1.0 { (self.half_width - off.length(), side) } else { (-1.0e9, side) };
        if disc.0 >= lane.0 {
            disc
        } else {
            lane
        }
    }

    pub fn inside(&self, p: Vec3) -> bool {
        self.depth(p).0 > 0.0
    }
}

/// THE STEP OUT (the party step, before the members move): an AI member standing in a zone with no move under way is sent
/// out of it with a normal move (a MoveTo to the nearest way out; no teleport); an attack command on an enemy in a zone
/// is dropped (it disengages: the friends' trees pick another).
pub fn step_out(party: &mut Party, zones: &[AvoidZone]) {
    if zones.is_empty() {
        return;
    }
    let ids: Vec<MemberId> = party.members().collect();
    for id in ids {
        let Some(p) = party.member(id) else { continue };
        if p.pad.is_some() || zones.iter().any(|z| z.caster == id) || p.status.ko() {
            continue;
        }
        let moving = p.state == crate::states::NpcStateId::Move;
        if let Some(out) = zone_exit(zones, p.position()) {
            if !moving {
                party.command(id, crate::command::MemberCommand::MoveTo { goal: crate::command::MoveGoal::Point(out), locomotion: crate::command::LocomotionDef::Move1, walk: false, acceptable_radius: 30.0, follow_path: false });
            }
        } else if let Some(at) = wait_place(party, Some(id), p.position()) {
            // (round 5c) no enemy left to them: to their place at the waiting spot (then they stand ready there)
            if !moving && (p.position() - at).truncate().length() > 120.0 {
                // (the move keeps out of the zones and goes round them: the pawn's goal_point)
                party.command(id, crate::command::MemberCommand::MoveTo { goal: crate::command::MoveGoal::Point(at), locomotion: crate::command::LocomotionDef::Move1, walk: false, acceptable_radius: 20.0, follow_path: false });
            }
        }
    }
}

/// (round 5c) An ally's place to wait ([`ZettaWorld::wait_spots`], the host's): the nearest spot it can walk to straight
/// without crossing a zone (else the nearest), and there its own place: the AI members side by side across it (1.5 m
/// apart, facing Donald), AI Sora (`id` None) at its middle. None: not waiting.
pub fn wait_place(party: &Party, id: Option<MemberId>, from: Vec3) -> Option<Vec3> {
    let spots = &party.don_zetta.wait_spots;
    let near = |p: &Vec3| (*p - from).truncate().length();
    let at = spots.iter().filter(|p| party.don_zetta_path_clear(from, **p, 30.0)).min_by(|a, b| near(a).total_cmp(&near(b))).or_else(|| spots.iter().min_by(|a, b| near(a).total_cmp(&near(b)))).copied()?;
    let Some(id) = id else { return Some(at) };
    let dz = party.don_zetta.runs.iter().find(|r| r.kind == RunKind::Zetta && !r.done && !r.handed)?.zone_donald?;
    let to_don = crate::pawn::horizontal(dz.at - at).normalize_or(Vec3::X);
    let across = Vec3::new(-to_don.y, to_don.x, 0.0);
    let ai: Vec<MemberId> = party.members().filter(|m| party.member(*m).is_some_and(|p| p.pad.is_none() && !p.status.ko()) && *m != dz.caster).collect();
    let k = ai.iter().position(|m| *m == id)?;
    // AI Sora keeps the middle: the members on either side of it (+1, -1, +2, -2 ...)
    let slot = (k / 2 + 1) as f32 * if k % 2 == 0 { 1.0 } else { -1.0 };
    Some(at + across * (slot * 150.0))
}

/// The nearest place out of every zone from `p` (`None`: not in one); out by the zone's margin.
pub fn zone_exit(zones: &[AvoidZone], p: Vec3) -> Option<Vec3> {
    let mut q = p;
    let mut moved = false;
    for _ in 0..4 {
        let mut any = false;
        for z in zones {
            let (d, out) = z.depth(q);
            if d > 0.0 {
                q += Vec3::new(out.x, out.y, 0.0) * (d + z.margin);
                any = true;
                moved = true;
            }
        }
        if !any {
            break;
        }
    }
    moved.then_some(q)
}

/// THE ALLIES' GOAL (the friends' movement, kh-party pawn `goal_point`; AI Sora's stick, kh-partyhost lead.rs): a goal in a
/// zone is moved out of it; a member in one goes out first (normal movement: the nearest way out); a straight way that
/// would cross one goes round it (a waypoint past its edge).
pub fn zone_goal(zones: &[AvoidZone], from: Vec3, goal: Vec3) -> Vec3 {
    if zones.is_empty() {
        return goal;
    }
    if let Some(out) = zone_exit(zones, from) {
        return out;
    }
    let goal = zone_exit(zones, goal).unwrap_or(goal);
    let d = goal - from;
    let n = (Vec2::new(d.x, d.y).length() / 50.0).ceil().clamp(1.0, 200.0) as usize;
    for i in 1..n {
        let p = from + d * (i as f32 / n as f32);
        if zones.iter().any(|z| z.inside(p)) {
            // round it: the crossing point pushed out past the edge (and a little further, the side it leans to)
            return zone_exit(zones, p).map_or(goal, |w| w + (w - p).normalize_or_zero() * 50.0);
        }
    }
    goal
}

/// (round 3) THE TARGET AS THE CAMERA SEES IT: where it is now (its centre: its own reaction moves it), its size against
/// Terra-Xehanort's (half height / `zetta.target_center_height`: the framing of its shots is scaled by it), and whether its
/// hold shows an animation (`Targets::held_animated`: the user, "make sure the enemy is never shown during the cutscene
/// without an appropriate animation"; a target held without one is kept out of the shots until its reaction).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CamSubject {
    pub at: Vec3,
    pub size: f32,
    pub animated: bool,
    /// (round 5) its body's radius (cm): the camera keeps out of it
    pub radius: f32,
    /// (2026-10-23) its body's half height (cm; its feet under `at`); 0: the Sora case's scaled by `size`
    pub half: f32,
}

/// What the target plays at a moment of the sequence (the camera's rule: it is only shown playing something).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetAnim {
    /// Held from the channel's start to the hit, its own loop playing (its idle / guard loop; turned to Donald while it
    /// plays): the host's hold, `held_animated`.
    HeldLoop,
    /// Held, but the host has no loop to show for it: a statue. Never on camera.
    Static,
    /// From the hit: its reaction (the host's hit reaction, pushed back along the beam).
    Reaction,
}

/// (round 5c, the user: "the zettaflare only pushes the enemy back only in the shot of the beam when it's shrinking, otherwise
/// hold them in place for the cutscene") THE MATINEE TIME THE HELD TARGET IS THROWN: with the ending, in the wide shot of the
/// beam shrinking (`zetta.end_throw_at`): held in place through every shot before it; without the ending, as the beam
/// ends (`zetta.hold_until`).
pub fn throw_at(z: &ZettaTune) -> f32 {
    if ending_on(z) {
        z.zetta_end_throw_at
    } else {
        z.zetta_hold_until
    }
}

/// The target's animation at sequence time `s` (see [`TargetAnim`]).
pub fn target_anim(s: f32, z: &ZettaTune, channel: f32, animated: bool) -> TargetAnim {
    if !animated {
        TargetAnim::Static
    } else if s + 1e-4 >= seq_time(throw_at(z), z, channel) {
        TargetAnim::Reaction
    } else {
        TargetAnim::HeldLoop
    }
}

/// (round 3) The event's panning close-up of Donald charging (shot 860): the channel's shot, exactly as the event.
pub const PAN_SHOT: &str = "shot860";

/// (round 4) The event's shot of Donald raising his staff ("Zettaflare!", shot 910): exactly as the event.
pub const CAST_SHOT: &str = "shot910";

/// The screen's width / height the framing fits to (16:9).
pub const CAM_ASPECT: f32 = 16.0 / 9.0;

/// Where `p` is seen from a camera at `from` turned `rot`: (right, up) angles off the view axis, degrees (> 90: behind).
pub fn view_angles(from: Vec3, rot: Vec3, p: Vec3) -> (f32, f32) {
    let [x, y, zz] = rot_axes(rot);
    let v = p - from;
    let fwd = v.dot(x);
    (v.dot(y).atan2(fwd).to_degrees(), v.dot(zz).atan2(fwd).to_degrees())
}

/// The vertical FOV of a horizontal one (degrees) on the 16:9 screen.
pub fn vfov_of(hfov: f32) -> f32 {
    2.0 * ((hfov * 0.5).to_radians().tan() / CAM_ASPECT).atan().to_degrees()
}

/// Both subjects inside the view (with the margin)?
pub fn frames_both(from: Vec3, rot: Vec3, hfov: f32, a: Vec3, b: Vec3, margin: f32) -> bool {
    let (hh, vv) = (hfov * 0.5 / margin.max(1.0), vfov_of(hfov) * 0.5 / margin.max(1.0));
    [a, b].iter().all(|p| {
        let (h, v) = view_angles(from, rot, *p);
        h.abs() <= hh && v.abs() <= vv
    })
}

/// THE COMPOSITION ON THE DONALD -> TARGET AXIS (the user: the camera "frames the target throughout"): a shot of the event
/// that already shows both subjects (`a` its own, `b` the other) is kept as it is; one that does not is turned about `a`
/// (orbiting at its height, the smallest turn first, `zetta.cam_sep_max`) and, if no turn will do, pulled back, until the
/// two can share the view; it is then aimed between them with the FOV opened to fit them (`zetta.cam_fit_margin`, at most
/// `zetta.cam_fov_max`). Nothing fits: `safe`.
pub fn compose(raw: CamPose, a: Vec3, b: Vec3, safe: CamPose, z: &ZettaTune) -> CamPose {
    let margin = z.zetta_cam_fit_margin.max(1.0);
    if frames_both(raw.position, raw.rotation, raw.fov, a, b, margin) {
        return raw;
    }
    let max = z.zetta_cam_sep_max.clamp(5.0, 170.0);
    let fov_max = z.zetta_cam_fov_max.max(10.0);
    let d0 = raw.position - a;
    let at = |deg: f32, far: f32| {
        let (s, c) = deg.to_radians().sin_cos();
        a + Vec3::new(d0.x * c - d0.y * s, d0.x * s + d0.y * c, d0.z) * far
    };
    for far in [1.0, 1.4, 2.0, 2.8] {
        for k in 0..=36 {
            for sign in [1.0, -1.0] {
                let p = at(sign * 5.0 * k as f32, far);
                if (a - p).angle_between(b - p).to_degrees() > max {
                    continue;
                }
                if let Some(pose) = fit_between(p, a, b, raw.fov, raw.rotation.z, raw.shot, z).filter(|q| q.fov <= fov_max) {
                    return pose;
                }
                if k == 0 {
                    break;
                }
            }
        }
    }
    safe
}

/// A camera at `p` aimed between `a` and `b` with its FOV opened to fit both (its roll kept), the FOV NOT capped (None:
/// the two cannot share one view from there).
pub fn fit_between(p: Vec3, a: Vec3, b: Vec3, fov: f32, roll: f32, shot: &'static str, z: &ZettaTune) -> Option<CamPose> {
    let margin = z.zetta_cam_fit_margin.max(1.0);
    // aimed nearer its own subject (0.6 / 0.4): the shot stays about `a`, the other in the frame
    let d = ((a - p).normalize_or_zero() * 0.6 + (b - p).normalize_or_zero() * 0.4).normalize_or_zero();
    let d = if d == Vec3::ZERO { (a - p).normalize_or_zero() } else { d };
    if d == Vec3::ZERO {
        return None;
    }
    let rot = Vec3::new(d.z.clamp(-1.0, 1.0).asin().to_degrees(), d.y.atan2(d.x).to_degrees(), roll);
    let mut need_h: f32 = 0.0;
    let mut need_v: f32 = 0.0;
    for q in [a, b] {
        let (h, v) = view_angles(p, rot, q);
        if h.abs() >= 89.0 || v.abs() >= 89.0 {
            return None;
        }
        need_h = need_h.max(h.abs() * 2.0 * margin);
        need_v = need_v.max(v.abs() * 2.0 * margin);
    }
    if need_v >= 170.0 || need_h >= 170.0 {
        return None;
    }
    let from_v = 2.0 * ((need_v * 0.5).to_radians().tan() * CAM_ASPECT).atan().to_degrees();
    Some(CamPose { position: p, rotation: rot, fov: fov.max(need_h).max(from_v).max(10.0), pivot: a, shot })
}

/// A camera at `p` aimed between `a` and `b` with its FOV opened to fit both (its roll kept), at most `zetta.cam_fov_max`.
pub fn aim_between(p: Vec3, a: Vec3, b: Vec3, fov: f32, roll: f32, shot: &'static str, z: &ZettaTune) -> CamPose {
    let fov_max = z.zetta_cam_fov_max.max(10.0);
    match fit_between(p, a, b, fov, roll, shot, z) {
        Some(mut q) => {
            q.fov = q.fov.clamp(10.0, fov_max);
            q
        }
        None => {
            let d = (a - p).normalize_or(Vec3::X);
            CamPose { position: p, rotation: Vec3::new(d.z.clamp(-1.0, 1.0).asin().to_degrees(), d.y.atan2(d.x).to_degrees(), roll), fov: fov_max, pivot: a, shot }
        }
    }
}

/// THE SAFE FRAMING (a shot the level blocks, or one no turn can compose): over Donald's right shoulder toward the target,
/// pulled back until both fit.
pub fn safe_pose(donald_feet: Vec3, target: Vec3, z: &ZettaTune) -> CamPose {
    let chest = donald_feet + Vec3::Z * 60.0;
    let flat = crate::pawn::horizontal(target - donald_feet);
    let fwd = if flat.length() > 1.0 { flat.normalize() } else { Vec3::X };
    let right = Vec3::new(-fwd.y, fwd.x, 0.0);
    let fov_max = z.zetta_cam_fov_max.max(10.0);
    let mut last = None;
    for far in [1.0, 1.5, 2.2, 3.2, 4.5] {
        let p = donald_feet - fwd * (z.zetta_cam_safe_back * far) + right * z.zetta_cam_safe_side + Vec3::Z * (60.0 + z.zetta_cam_safe_up * far);
        let pose = aim_between(p, chest, target, 50.0, 0.0, "safe", z);
        if fit_between(p, chest, target, 50.0, 0.0, "safe", z).is_some_and(|q| q.fov <= fov_max) {
            let mut pose = pose;
            pose.pivot = chest;
            return pose;
        }
        last = Some(pose);
    }
    let mut pose = last.unwrap();
    pose.pivot = chest;
    pose
}

/// The event's shot at a sequence time, in its frame, composed on the axis: the channel plays its Donald shots one after
/// the other from their starts (860 for 3 s, then 890; the last one held), the cast part the shot holding its matinee
/// time. The subjects: Donald's shots Donald (his chest) and the target; the beam's shots the target and the beam just
/// before it (the last one shows the beam hitting it).
/// (round 5) `end`: the ending's frame (his feet at the ending, X along the beam's line; `None` before it: his feet now and
/// the beam's line), for the ending's shots 960 / 970 (with the ending on only).
#[allow(clippy::too_many_arguments)]
pub fn cam_pose(shots: &[CamShot], s: f32, channel: f32, z: &ZettaTune, donald: Frame, target: Frame, beam: Basis, subject: CamSubject, end: Option<Frame>) -> Option<CamPose> {
    // THE CHARGING CLOSE-UP (the user: "the panning closeup of Donald charging like the original cutscene ... frame it as
    // close as you can to the original cutscene"): the channel is the event's shot 860 alone, exactly as the event has it
    // (its position, rotation and FOV relative to Donald's feet and facing, its pan curve key by key), time-scaled onto the
    // channel; it is not recomposed (only a wall moves it: the host's collision pass, nothing more)
    if s < channel {
        let shot = shots.iter().find(|x| x.name == PAN_SHOT).or_else(|| shots.iter().find(|x| x.channel))?;
        let k = (s.max(0.0) / channel.max(1e-3)).clamp(0.0, 1.0);
        let pose = |k: f32| {
            let (p, r, fov) = shot.at(shot.start() + (shot.stop() - shot.start()) * k);
            CamPose { position: donald.point(p), rotation: donald.rotation(r), fov, pivot: donald.origin + Vec3::Z * 60.0, shot: shot.name }
        };
        let mut out = pose(k);
        // a target held without a loop to show: the pull-back stops before it would come into view (the least change)
        if !subject.animated {
            let mut kk = k;
            while kk > 0.0 && frames_both(out.position, out.rotation, out.fov, subject.at, subject.at, 1.0) {
                kk = (kk - 0.01).max(0.0);
                out = pose(kk);
            }
        }
        return Some(out);
    }
    let (shot, t) = {
        let m = z.zetta_cast_start + (s - channel);
        let ending = ending_on(z);
        let cast: Vec<&CamShot> = shots.iter().filter(|x| !x.channel && (ending || x.frame != "end")).collect();
        // the shot that began last (each holds until the next begins)
        let x = cast.iter().rev().find(|x| x.start() <= m + 1e-4).or(cast.first())?;
        // (round 5) ONE CONTINUOUS SHOT OF THE TARGET (the user: "When the camera swaps to the enemy, keep it a persistant shot
        // of the enemy, going from visible to being overcome with the light then their black silhouette if it applies and then
        // them crumbling to nothing"): the event's target cuts (920, 930, 940) folded into its first one, 920, its own drift
        // then held, until the ending's wide shot
        let x = if z.zetta_cam_target_one_shot >= 0.5 && x.frame == "subject" { cast.iter().find(|s| s.frame == "subject").unwrap_or(x) } else { x };
        (*x, m)
    };
    let (p, r, fov) = shot.at(t);
    let chest = donald.origin + Vec3::Z * 60.0;
    let goal = beam.origin;
    // (round 5) THE ENDING'S SHOTS (the user's frames: "The end needs to look exactly like this ... the end shot behind
    // donald is where gameplay resumes"): kg702c's 960 (the wide shot of the beam shrinking in the sky) and 970 (behind
    // him), the event's exactly: position, rotation and lens relative to his feet at the ending and the beam's line, key
    // by key (a wall only pulls them in toward him)
    if shot.frame == "end" {
        let fr = end.unwrap_or(Frame { origin: donald.origin, yaw: crate::pawn::yaw_of(crate::pawn::horizontal(beam.x)) });
        // (2026-10-23, the user: "the beam should be bigger in the zoomed out shot as well, the size it was to cover the enemy")
        // the wide shot (960) scaled about his feet by the beam's girth: the bigger beam framed as the event frames its own
        let g = if shot.name == "shot960" { (subject.size * z.zetta_beam_girth_margin.max(0.1)).clamp(z.zetta_beam_scale_min.max(0.05), SIZE_MAX) } else { 1.0 };
        // (2026-10-23, the user: "the beam MUST start at Donald's wand, and NEVER show the end of it") the ending beam grows
        // across its line only (its length as the event's: [`end_beam_scale`]): the wide shot as the event frames it, so its far
        // end runs out of the frame as the event's does
        let g = if g > 0.0 { 1.0 } else { 1.0 };
        return Some(CamPose { position: fr.point(p * g), rotation: fr.rotation(r), fov, pivot: fr.origin + Vec3::Z * 60.0, shot: shot.name });
    }
    // THE TARGET'S SHOTS (920 / 930 / 940: it is blown back in the beam): the event's camera path relative to its subject
    // (make_zetta_cam.js: Terra-Xehanort in the beam), in the beam's axes, scaled to the target's size, around the target
    // where it is now; aimed at it (the event's 920 / 930 pan off his actor's stand: the data does not show where he is
    // blown to), the event's roll and lens
    if shot.frame == "subject" && !subject.animated {
        // the target not visibly animating (the host's own rule, read at run time): the shot favours Donald (the event's
        // cast shot as it ends: the beam leaving his staff)
        if let Some(cast) = shots.iter().find(|x| x.frame == "donald" && !x.channel) {
            let (p, r, fov) = cast.at(cast.stop());
            return Some(CamPose { position: donald.point(p), rotation: donald.rotation(r), fov, pivot: chest, shot: cast.name });
        }
    }
    if shot.frame == "subject" {
        let mut at = subject.at + beam.vector(p * subject.size.clamp(0.2, SIZE_MAX));
        // (round 5) never inside the target: at least `zetta.cam_subject_min` x its size from its centre (pushed back along the
        // shot's own line)
        // (2026-10-23, the user: "THIS FRAMING IS PERFECT! EVERY ENEMY NEEDS TO BE FRAMED LIKE THIS WITH THE BEAM MATCHING THIS
        // FRAME EXACTLY NO MATTER HOW BIG THE ENEMY IS"): the whole shot scaled about the target by its size (the test
        // level's Sora = 1), every distance with it, so its frame is the reference frame at any size
        let min = z.zetta_cam_subject_min.max(0.0) * subject.size.clamp(0.2, SIZE_MAX);
        // (2026-10-23, the coordinator: "so nothing is ever cropped") never so near that the fitted body (`zetta.frame_body_*`
        // x its fit) takes more than `zetta.frame_share_*` of the view: the event's dolly-in stops there (the reference's
        // moment, the user's PERFECT frame, is that framing); x its size, so every body holds the same framing
        let th = (fov * 0.5).to_radians().tan().max(0.01);
        let tv = (vfov_of(fov) * 0.5).to_radians().tan().max(0.01);
        let fit = (z.zetta_frame_body_height / (z.zetta_frame_share_height.clamp(0.05, 1.0) * 2.0 * tv)).max(z.zetta_frame_body_width / (z.zetta_frame_share_width.clamp(0.05, 1.0) * 2.0 * th));
        let min = min.max(fit * subject.size.clamp(0.2, SIZE_MAX));
        let off = at - subject.at;
        if off.length() < min {
            at = subject.at + off.normalize_or(-beam.x) * min;
        }
        // (round 5) A LOW ANGLE (the user: "the enemy shot for zettaflare should be more of a low angle than a high one"): the
        // camera at `zetta.cam_target_low` x its size above its feet (never under them), looking up at its chest
        // (`zetta.cam_target_look` of its half height above its centre): imposing, against the sky and the light
        let s = subject.size.clamp(0.2, SIZE_MAX);
        // (2026-10-23) its real feet (its own half height; the Sora case's scaled when unknown)
        let half = if subject.half > 0.0 { subject.half } else { z.zetta_target_center_height.max(1.0) * s };
        let feet = subject.at.z - half;
        // (2026-10-23, the coordinator: "In the reference the beam fills the ENTIRE frame") its height over the body's FEET
        // scaled with it (the Sora case's 50 cm x its fit), not about its centre: a standing body's scaled shot would sink
        // under the floor and see the ground in front of the beam; this keeps the reference's look up at it and the floor out
        // of the view
        at.z = (feet + z.zetta_cam_target_low.max(0.0) * s).max(feet + 25.0 * s);
        // (2026-10-23) clear of the floor (the camera's collision probe): a small target's scaled shot never drops into it
        at.z = at.z.max(feet + z.zetta_cam_floor_clear.max(0.0));
        // (2026-10-23) raised off the floor, still no nearer than the fit allows: out along the ground, its height kept
        let off = at - subject.at;
        if off.length() < min {
            let flat = Vec3::new(off.x, off.y, 0.0);
            let need = (min * min - off.z * off.z).max(0.0).sqrt();
            at = subject.at + flat.normalize_or(Vec3::new(-beam.x.x, -beam.x.y, 0.0).normalize_or(Vec3::NEG_X)) * need + Vec3::Z * off.z;
        }
        // out of the beam's core: at least `zetta.cam_beam_clear` from its line (sideways, the height kept), so the target is
        // seen clearly before the light takes it
        let rel = at - subject.at;
        let (ly, lz) = (rel.dot(beam.y), rel.dot(beam.z));
        let clear = z.zetta_cam_beam_clear.max(0.0) * s; // the beam is as many times wider (its girth follows the size)
        if ly.hypot(lz) < clear {
            let want = (clear * clear - lz * lz).max(0.0).sqrt();
            let side = if ly.abs() > 1.0 { ly.signum() } else { 1.0 };
            at += beam.y * (side * want - ly);
        }
        let at = clear_of_core(at, chest, beam, z, &subject);
        let look = subject.at + Vec3::Z * (z.zetta_cam_target_look * half);
        let aim = |at: Vec3| {
            let d = (look - at).normalize_or(beam.x);
            Vec3::new(d.z.clamp(-1.0, 1.0).asin().to_degrees(), d.y.atan2(d.x).to_degrees(), r.z)
        };
        return Some(CamPose { position: at, rotation: aim(at), fov, pivot: subject.at, shot: shot.name });
    }
    let (raw, a, b) = match shot.frame {
        "beam" => {
            let near = goal - beam.x * ((goal - chest).length() * 0.6).min(500.0);
            (CamPose { position: beam.point(p), rotation: beam.rotation(r), fov, pivot: goal, shot: shot.name }, goal, near)
        }
        "target" => (CamPose { position: target.point(p), rotation: target.rotation(r), fov, pivot: target.origin, shot: shot.name }, goal, chest),
        _ => (CamPose { position: donald.point(p), rotation: donald.rotation(r), fov, pivot: chest, shot: shot.name }, chest, goal),
    };
    // DONALD'S SHOTS ARE THE EVENT'S EXACTLY (the user: "the camera for Zettaflare for when Donald raises his wand should
    // match the original cutscene"): position, rotation and lens relative to his feet and facing, key by key; only the
    // shots of Terra-Xehanort frame the target
    if shot.frame == "donald" {
        // (round 5c) ... but never inside a giant's beam (its girth: [`clear_of_core`]): moved sideways, its framing kept
        let mut raw = raw;
        // (2026-10-23, the user: "the shot zooming away from Donald as he launches Zettaflare should NOT change regardless of the
        // size of the enemy so Donald is always framed perfectly, use Zettaflare on Sora as a baseline") kept off the beam as
        // against the Sora target (fit 1), whatever the enemy's size
        if t + 1e-3 >= z.zetta_release_at {
            let sora = CamSubject { size: 1.0, radius: TARGET_RADIUS_REF, half: 0.0, ..subject };
            raw.position = clear_of_core(raw.position, chest, beam, z, &sora);
        }
        return Some(raw);
    }
    let safe = safe_pose(donald.origin, goal, z);
    let mut out = compose(raw, a, b, safe, z);
    out.shot = shot.name;
    // (round 5c) never in the beam: a composed shot (910: the beam reaching the target) kept off its core, re-aimed at its
    // subject when moved
    let moved = if t + 1e-3 >= z.zetta_release_at { clear_of_core(out.position, chest, beam, z, &subject) } else { out.position };
    if (moved - out.position).length() > 1e-3 {
        let d = (out.pivot - moved).normalize_or(beam.x);
        out.rotation = Vec3::new(d.z.clamp(-1.0, 1.0).asin().to_degrees(), d.y.atan2(d.x).to_degrees(), out.rotation.z);
        out.position = moved;
    }
    Some(out)
}

/// (2026-10-23, the user: "make sure the camera dynamically zooms out to frame the whole target and the beam and all other
/// effects dynamically scale to cover the target completely just like in the reference photo"; "vice versa, if the target is
/// small then the camera zooms in and the effects scale") THE TARGET'S SIZE against the event's (Terra-Xehanort: the
/// shots' and the beam's own scale), both ways: its height over his centre height or its width over his radius, whichever
/// is larger, from what the host draws of it (`Targets::drawn_extent`) when that is more than its capsule.
///
/// (2026-10-23, the coordinator's review of the golden-frame proof: "The framing must fit the target's FULL bounds (height
/// AND width, with reach) inside the same screen region as the reference ... so nothing is ever cropped") THE FIT: the
/// factor the reference shot (the test level's Sora in his hurt pose, factor 1) is scaled by about the target so its whole
/// body, top to feet and side to side, fills the reference's region: its full height over `zetta.frame_body_height` or its
/// full width over `zetta.frame_body_width`, whichever is larger (the region the Sora case's body fills: 45 % of the shot's
/// height, 63 % of its width at his depth).
pub fn target_size(half_height: f32, radius: f32, z: &ZettaTune) -> f32 {
    (2.0 * half_height.max(1.0) / z.zetta_frame_body_height.max(1.0)).max(2.0 * radius.max(1.0) / z.zetta_frame_body_width.max(1.0)).clamp(0.2, SIZE_MAX)
}

/// (2026-10-23) The largest fit (a 26 m body).
pub const SIZE_MAX: f32 = 40.0;

/// (2026-10-23, the user: "make the size the beam is when it attacks the Sora in the test level the minimum size the beam can
/// be") THE BEAM'S GIRTH for a target: as many times the event's as the target is larger (`target_size` x
/// `zetta.beam_girth_margin`), never under `zetta.beam_scale_min` (the test level's Sora target: the shots still zoom in on
/// a smaller one), at most 12.
pub fn beam_girth(half_height: f32, radius: f32, z: &ZettaTune) -> f32 {
    (target_size(half_height, radius, z) * z.zetta_beam_girth_margin.max(0.1)).clamp(z.zetta_beam_scale_min.max(0.05), SIZE_MAX)
}

/// Terra-Xehanort's radius (cm): the event's beam is as wide as it takes to engulf him.
pub const TARGET_RADIUS_REF: f32 = 50.0;

/// (2026-10-23) A target's measured (radius, half height): what the host draws of it when it knows (its pose's bounds, its
/// reach), else its capsule.
pub fn target_extent(targets: &dyn kh_core::host::Targets, info: &kh_core::host::TargetInfo) -> (f32, f32) {
    match targets.drawn_extent(info.id) {
        Some((r, h)) if r > 0.0 && h > 0.0 => (r, h),
        _ => (info.radius, info.half_height),
    }
}

/// (round 5c) THE CAMERA NEVER IN THE BEAM (the user: "See the bits of light coming towards the camera and it's being cut
/// off?"; the coordinator: "No beam layer may start at or pass through the camera"): a camera within `zetta.cam_core_clear`
/// x the beam's girth (its width follows the target's size: [`ZettaRun::girth`]) of the beam's line, from Donald's
/// chest past the target, is moved sideways off it (its height kept). The event's own shots never come this near.
fn clear_of_core(at: Vec3, chest: Vec3, beam: Basis, z: &ZettaTune, subject: &CamSubject) -> Vec3 {
    // (2026-10-23) the subject's own scale (the reference frame scaled about it: a small one's camera comes in with it)
    let girth = (subject.size * z.zetta_beam_girth_margin.max(0.1)).clamp(0.2, SIZE_MAX);
    let clear = z.zetta_cam_core_clear.max(0.0) * girth;
    let rel = at - beam.origin;
    let along = rel.dot(beam.x);
    if clear <= 0.0 || along < (chest - beam.origin).dot(beam.x) {
        return at;
    }
    let (ly, lz) = (rel.dot(beam.y), rel.dot(beam.z));
    if ly.hypot(lz) >= clear {
        return at;
    }
    let want = (clear * clear - lz * lz).max(0.0).sqrt() + 1.0;
    let side = if ly.abs() > 1.0 { ly.signum() } else { 1.0 };
    at + beam.y * (side * want - ly)
}

// ============================================================================================== the runs

/// A placed effect alive.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LiveFx {
    row: usize,
    handle: EffectHandle,
    /// Sequence time it ends (INFINITY: by itself).
    end: f32,
    /// The frame it was placed in (its move track is replayed in it).
    frame: Frame,
    /// (round 5c) Removed at once at its end, not left to play out (an ultimate's release flash and light: `ult.flash_time`).
    cut: bool,
    /// (round 5c) The per-shot size factor it was last given ([`shot_scale`]).
    shot_scale: f32,
}

/// (round 5c) THE PER-SHOT SIZE OVERRIDES (the user: "you can make things change size with camera cuts, just make sure
/// everything looks cool and correct"): rows of kind `shot_scale` in party-don-zetta-fx (asset = the row id they scale, or
/// a prefix ending in `*`; socket = the camera shot; scale = the factor). The product of those matching `row_id` in `shot`
/// (1 with none). The sequence applies it as the shot cuts in, never mid-shot.
/// (round 5c) The cinematic camera's shot at matinee time `mt` (the shot that began last; the target's shots folded into its
/// first, as [`cam_pose`] does; the ending's only with the ending), for [`shot_scale`] (known as the shot cuts in).
pub fn shot_at(mt: f32, z: &ZettaTune) -> Option<&'static str> {
    // each shot's start (its first key), frame and part, once
    static STARTS: std::sync::OnceLock<Vec<(f32, &'static str, &'static str, &'static str)>> = std::sync::OnceLock::new();
    let starts = STARTS.get_or_init(|| {
        let mut v: Vec<(f32, &'static str, &'static str, &'static str)> = Vec::new();
        for r in DON_ZETTA_CAM {
            match v.iter_mut().find(|x| x.1 == r.shot) {
                Some(x) => x.0 = x.0.min(r.time),
                None => v.push((r.time, r.shot, r.frame, r.part)),
            }
        }
        v
    });
    let ending = ending_on(z);
    let live = starts.iter().filter(|x| x.3 != "channel" && (ending || x.2 != "end") && x.0 <= mt + 1e-4);
    let (_, shot, frame, _) = *live.clone().fold(None, |b: Option<&(f32, &str, &str, &str)>, x| if b.map_or(true, |b| x.0 > b.0) { Some(x) } else { b })?;
    if frame == "subject" && z.zetta_cam_target_one_shot >= 0.5 {
        return live.filter(|x| x.2 == "subject").fold(None, |b: Option<&(f32, &'static str, &str, &str)>, x| if b.map_or(true, |b| x.0 < b.0) { Some(x) } else { b }).map(|x| x.1).or(Some(shot));
    }
    Some(shot)
}

pub fn shot_scale(row_id: &str, shot: Option<&str>) -> f32 {
    let Some(shot) = shot else { return 1.0 };
    DON_ZETTA_FX
        .iter()
        .filter(|r| r.use_ == 1 && r.kind == "shot_scale" && r.socket == shot)
        .filter(|r| r.asset == row_id || r.asset.strip_suffix('*').is_some_and(|p| row_id.starts_with(p)))
        .map(|r| if r.scale > 0.0 { r.scale } else { 1.0 })
        .product()
}

/// What a run did (tests, the harness, the debug text).
#[derive(Clone, Debug, PartialEq)]
pub enum ZettaLog {
    Begun { caster: MemberId, aim: ZettaAim },
    TakenOver { caster: MemberId },
    Clip { name: &'static str, t: f32 },
    Face { name: &'static str, t: f32 },
    Spawn { row: &'static str, t: f32, at: Vec3, rotation: Vec3, handle: EffectHandle },
    Stop { row: &'static str, t: f32 },
    Sound { row: &'static str, t: f32 },
    Released { t: f32 },
    Hit { aim: ZettaAim, at: Vec3, t: f32 },
    Interrupted { t: f32 },
    Ended { t: f32 },
    /// (round 3) The other enemies the blast caught.
    Blast { caught: Vec<TargetId>, t: f32 },
    /// (round 3) The target given back at the beam's end, its reaction asked for.
    Reaction { target: TargetId, t: f32 },
    /// (round 4) The beam killed its target: the finale is its black silhouette crumbling in the white light.
    Killed { target: TargetId, t: f32 },
    /// An ultimate's presentation began (its serial, tint).
    UltBegun { caster: MemberId, serial: u32, tint: Tint },
}

/// An ultimate's presentation on the pawn (from [`ultimate_begin`] until its cast clip ends or [`ultimate_cancel`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UltState {
    /// Its number (the run is this presentation).
    pub serial: u32,
    pub tint: Tint,
    pub target: CastTarget,
    /// The caller stopped it ([`ultimate_cancel`]): the run stops everything at once.
    pub cancel: bool,
}

/// The pawn's side (`NpcPawn::don_zetta`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PawnZetta {
    pub ult: Option<UltState>,
    /// Where he is rooted (feet) and, after the release, the facing he keeps.
    pub anchor: Option<(Vec3, Option<f32>)>,
    pub serials: u32,
    /// (round 5) The ending holds him (his pose under the beam, rooted) until the hand-off: from kg702c's start to
    /// `zetta.end_handoff_at` (with the cinematic camera only).
    pub ending: bool,
    /// (2026-10-23) The down after Zettaflare ([`ZettaDown`]): the cutscene's collapse, down, the get-up.
    pub down: Option<ZettaDown>,
    /// (2026-10-23) His staff out of his hand ([`StaffAway`]): on the ground where it fell, or on its way back.
    pub staff: Option<StaffAway>,
    /// (2026-10-23, the user: "The target that Zettaflare is initiated on is THE one that is focused on in the cutscene") his
    /// lock-on and soft target as the cast began, kept through the sequence ([`hold`]: the pad's lock-on input frozen),
    /// given back to the pad's own rule at the hand-back.
    pub lock_keep: Option<(TargetId, TargetId)>,
}

/// (2026-10-23, the user: "Donald's wand should stay where it falls, and then a similar effect to the keyblade for being
/// transported back to his hand") HIS STAFF OUT OF HIS HAND: the collapse throws it (the cutscene's own clip); from the
/// collapse's end it lies where it fell (`ground`: its world place and turn, the host's own drawing of it at that moment,
/// [`feed_staff`]); it goes back to his hand at the get-up (it dissolves on the ground in his staff's own appear /
/// disappear light, `vw_don_app00` (KH3's m_AppearEffect / m_DisappearEffect of his staves), hidden over the blend into the
/// get-up, `back` s, then appears in his hand in the same light with the Keyblade's own appear sound) or, after a hit ended
/// the down, as the player has him back (dissolve and appear at once: never empty-handed in his control, never held while it
/// lies on the ground). The hosts draw it at `ground` ([`Party::weapon_ground`]) and hide it while it is on its way
/// ([`Party::weapon_hidden`]).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct StaffAway {
    /// Out of his hand (flying, then lying where it fell): its world place (cm) and turn as drawn.
    pub ground: Option<(Vec3, glam::Quat)>,
    /// On its way back: the seconds left until it is in his hand.
    pub back: f32,
    /// (2026-10-23) Its fall ([`StaffFall`]).
    pub fall: StaffFall,
}

/// (2026-10-23, the user: "the wand dropping and then settling for Donald looks a bit jank") HIS STAFF'S FALL: a small rigid
/// body of its own from the moment it leaves his hand (the cutscene's weapon track: `zetta.staff_release` into the
/// collapse's second clip), on a fixed step (`STAFF_STEP`: the same at any frame rate): one arc with a gentle tumble, its two
/// ends on the ground under them (the world's own floor, uneven ground too), one small bounce, its tumble damped and the body
/// turned to lie along the ground at its own length, then at rest (exactly still: no jitter, no slide).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct StaffFall {
    pub pos: Vec3,
    pub vel: Vec3,
    pub rot: glam::Quat,
    /// Its tumble (rad/s, world axis x rate).
    pub spin: Vec3,
    /// It has touched the ground (the settle runs from then).
    pub landed: bool,
    pub resting: bool,
    /// The step's time not yet simulated.
    pub acc: f32,
}

/// (2026-10-23) The staff's fall: its fixed step (s), its ends along its own axis (local X: the grip at 0, cm), its
/// thickness (cm), the bounce kept, the slide and tumble lost per second once down, how fast it turns flat.
pub const STAFF_STEP: f32 = 1.0 / 120.0;
pub const STAFF_ENDS: (f32, f32) = (-25.0, 75.0);
pub const STAFF_RADIUS: f32 = 3.0;
pub const STAFF_BOUNCE: f32 = 0.22;
pub const STAFF_SLIDE_LOSS: f32 = 6.0;
pub const STAFF_FLATTEN: f32 = 7.0;

impl StaffFall {
    /// From his hand: its place and turn, its speed (from the hand's last step), a gentle tumble about its side.
    pub fn from_hand(pos: Vec3, rot: glam::Quat, hand_vel: Vec3) -> StaffFall {
        let mut vel = hand_vel;
        if vel.length() > 600.0 {
            vel = vel.normalize() * 600.0;
        }
        vel.z = vel.z.max(180.0); // (one clean arc)
        let axis = rot * Vec3::X;
        let side = Vec3::Z.cross(axis).normalize_or(Vec3::Y);
        StaffFall { pos, vel, rot, spin: side * 5.0, landed: false, resting: false, acc: 0.0 }
    }

    /// Its two ends now.
    pub fn ends(&self) -> (Vec3, Vec3) {
        (self.pos + self.rot * (Vec3::X * STAFF_ENDS.0), self.pos + self.rot * (Vec3::X * STAFF_ENDS.1))
    }

    /// Its turn lying along the ground: its own axis along the floor between its two ends (ga, gb: the floor's height
    /// under them; level ground: level), its heading kept, its roll about its axis kept.
    pub fn flat_on(&self, ga: f32, gb: f32) -> glam::Quat {
        let x = self.rot * Vec3::X;
        let (a, b) = self.ends();
        let run = Vec3::new(b.x - a.x, b.y - a.y, 0.0);
        let h = Vec3::new(run.x, run.y, gb - ga).normalize_or(Vec3::new(x.x, x.y, 0.0).normalize_or(Vec3::X));
        let swing = glam::Quat::from_rotation_arc(x.normalize_or(Vec3::X), h);
        (swing * self.rot).normalize()
    }

    /// `dt` s more; `ground(xy)`: the floor's height under a point (None: none found: the last one kept).
    pub fn step(&mut self, dt: f32, ground: &dyn Fn(Vec3) -> Option<f32>) {
        if self.resting {
            return;
        }
        // the floor under its two ends, read once a frame
        let (a, b) = self.ends();
        let (ga, gb) = (ground(a), ground(b));
        let fallback = ga.or(gb).unwrap_or(self.pos.z - 200.0);
        let (ga, gb) = (ga.unwrap_or(fallback), gb.unwrap_or(fallback));
        self.acc += dt;
        while self.acc >= STAFF_STEP {
            self.acc -= STAFF_STEP;
            let h = STAFF_STEP;
            self.vel.z -= 980.0 * h;
            self.pos += self.vel * h;
            if self.spin.length_squared() > 1e-8 {
                self.rot = (glam::Quat::from_scaled_axis(self.spin * h) * self.rot).normalize();
            }
            // its ends on the ground: pushed out, the bounce, then the slide and the tumble lost, turned flat
            let (a, b) = self.ends();
            let pen = (ga + STAFF_RADIUS - a.z).max(gb + STAFF_RADIUS - b.z);
            if pen > 0.0 {
                self.pos.z += pen;
                if self.vel.z < 0.0 {
                    self.vel.z = if self.landed { 0.0 } else { -self.vel.z * STAFF_BOUNCE };
                }
                self.landed = true;
            }
            if self.landed {
                let k = (-STAFF_SLIDE_LOSS * h).exp();
                self.vel.x *= k;
                self.vel.y *= k;
                self.spin *= k;
                let f = 1.0 - (-STAFF_FLATTEN * h).exp();
                self.rot = self.rot.slerp(self.flat_on(ga, gb), f).normalize();
            }
        }
        // at rest: lying along the ground at its own length, its middle on the floor under it, exactly still
        let flat_err = self.rot.angle_between(self.flat_on(ga, gb));
        if self.landed && self.vel.length() < 4.0 && flat_err < 0.02 {
            self.rot = self.flat_on(ga, gb);
            let (a, b) = self.ends();
            let lift = (ga + STAFF_RADIUS - a.z).max(gb + STAFF_RADIUS - b.z);
            self.pos.z += lift;
            self.vel = Vec3::ZERO;
            self.spin = Vec3::ZERO;
            self.resting = true;
        }
    }
}

/// (2026-10-23) His staff's appear / disappear light (KH3's m_AppearEffect and m_DisappearEffect of w_do010 / 020 / 030).
pub const STAFF_LIGHT: &str = "vw_don_app00";
/// (2026-10-23) The appear sound (the Keyblade's: se02001_010, the route of its own appear effect VW_SO010_APP0).
pub const STAFF_APPEAR_SE: &str = "/Game/Sound/SE/weapon/02001/se02001_010";
/// (2026-10-23) His staff's socket in his hand (the hosts' weapon socket bone).
pub const STAFF_SOCKET: &str = "R_wep_skt";

/// (2026-10-23) HIS STAFF AS THE HOST DRAWS IT (world place, cm, and turn), fed every frame like the wand's tip ([`feed_tip`]):
/// the collapse leaves it where this says.
static STAFF: std::sync::Mutex<Option<(u8, Vec3, glam::Quat)>> = std::sync::Mutex::new(None);

/// (2026-10-23) See [`STAFF`]: `member` the party member's id (Donald's), `None` stops the feed.
pub fn feed_staff(member: u8, at: Option<(Vec3, glam::Quat)>) {
    if let Ok(mut m) = STAFF.lock() {
        *m = at.map(|(a, r)| (member, a, r));
    }
    if let (Ok(mut p), Some((now, _))) = (STAFF_PREV.lock(), at) {
        let t = std::time::Instant::now();
        *p = Some(match *p {
            Some((id, _, last, at_t, _)) if id == member => (member, last, now, t, t.duration_since(at_t).as_secs_f32()),
            _ => (member, now, now, t, 0.0),
        });
    }
}

fn staff_of(member: MemberId) -> Option<(Vec3, glam::Quat)> {
    STAFF.lock().ok().and_then(|m| *m).filter(|(id, ..)| *id == member.0).map(|(_, a, r)| (a, r))
}

/// (2026-10-23) The staff's place at the two latest feeds and the time between them (its speed as it leaves his hand).
static STAFF_PREV: std::sync::Mutex<Option<(u8, Vec3, Vec3, std::time::Instant, f32)>> = std::sync::Mutex::new(None);

fn staff_vel(member: MemberId) -> Option<Vec3> {
    STAFF_PREV.lock().ok().and_then(|m| *m).filter(|(id, ..)| *id == member.0).and_then(|(_, a, b, _, dt)| (dt > 1e-3).then(|| (b - a) / dt))
}

/// (2026-10-23) His staff's light at a place (its dissolve on the ground, or its appearance in his hand) and, in his hand, the
/// appear sound.
fn staff_light(p: &mut NpcPawn, host: &mut PartyHost, at: Attach, hand: bool) {
    p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(STAFF_LIGHT)), name: STAFF_LIGHT, at, rotation: Vec3::ZERO, scale: 1.0, persistent: false });
    if hand {
        let pm = PartyMember(p.id.0);
        p.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(STAFF_APPEAR_SE)), name: STAFF_APPEAR_SE, at: Attach::party(pm, None), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
    }
}

/// (2026-10-23) HIS STAFF, EVERY STEP: back to his hand once the player has him (a hit ended the down), or at the end of its
/// way back from the get-up.
fn staff_step(party: &mut Party, host: &mut PartyHost, dt: f32) {
    let ids: Vec<MemberId> = party.members().collect();
    for id in ids {
        let Some(p) = party.member_mut(id) else { continue };
        let Some(mut s) = p.don_zetta.staff else { continue };
        let hand = Attach::Party { member: PartyMember(id.0), bone: Some(STAFF_SOCKET), offset: Vec3::ZERO };
        // (2026-10-23) its fall: simulated on its fixed step, drawn where it is
        if s.ground.is_some() {
            let world = host.world;
            // (the floor under each end, traced from just above it: never Donald's own body or anything over the staff)
            let ground = |q: Vec3| world.line_trace(q + Vec3::Z * 20.0, q - Vec3::Z * 400.0).filter(|h| !h.start_penetrating).map(|h| h.position.z);
            s.fall.step(dt, &ground);
            s.ground = Some((s.fall.pos, s.fall.rot));
            p.don_zetta.staff = Some(s);
        }
        if let Some((at, _)) = s.ground {
            if p.don_zetta.down.is_none() && !down_hurt(p) {
                // the player has him (his reaction to a hit over): it dissolves where it lies and is in his hand at once
                staff_light(p, host, Attach::World(at), false);
                staff_light(p, host, hand, true);
                p.don_zetta.staff = None;
            }
            continue;
        }
        s.back -= dt;
        if s.back <= 0.0 {
            staff_light(p, host, hand, true);
            p.don_zetta.staff = None;
        } else {
            p.don_zetta.staff = Some(s);
        }
    }
}

/// (2026-10-23, the user: "have Donald smoothly go back to his normal animations after the Zettaflare animation, there should
/// be something inbetween, maybe he falls down, the player can't control him but can rotate the camera, then 2 seconds later
/// he gets back up and is controllable. Donald can be hurt during this knocked down animation"; "Can you use the cutscene
/// animation where he falls down and drops his staff?"; "getting hit cancels his current animation and plays his usual hurt
/// animation, the player regains control") THE DOWN: from the hand-off, KH3's own collapse after the beam (kg702c: 702101w
/// at 19.267, 702102w at 21.167: he staggers, his staff flies from his hand and lands on the ground, he falls on his back),
/// its last frame held `zetta.down_time` (he lies there, the staff where it fell: the event's own loop after it, 702104er,
/// has the staff back in his lap, and its last clips (702151e / 702153e) place him elsewhere in the event's set), then
/// KH3's own get-up off his back (his down clip M_0213 from `zetta.down_up_from`: the staff back in his hand over the
/// blend into it), blended out to his idle; rooted where he stands, the pad's frame taken ([`hold`]: the camera stays the
/// player's), hittable: a hit (his damage or status reaction, his KO) ends it at once and his usual reaction plays (his
/// staff in his hand with it), the player has him back when it ends.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ZettaDown {
    /// Seconds into it.
    pub t: f32,
    /// Where he lies (his feet) and his facing.
    pub at: Vec3,
    pub yaw: f32,
    /// The get-up's line said.
    pub up_said: bool,
}

/// (2026-10-23) The down's clips (kg702c, Donald's body track): the collapse and its end (the staff dropped).
pub static DOWN_FALL_A: ClipDef = ClipDef::bare("n_ex001_702101w", 1.9, false);
pub static DOWN_FALL_B: ClipDef = ClipDef::bare("n_ex001_702102w", 2.667, false);

/// (2026-10-23) His get-up off his back: his own down clip (M_0213: the landing on his back and the get-up).
pub fn down_getup() -> &'static ClipDef {
    static BARE: ClipDef = ClipDef::bare("M_0213", 1.767, false);
    crate::tables::anim_set(crate::Chara::Donald).get("M_0213").unwrap_or(&BARE)
}

/// (2026-10-23) The down's parts: (clip, seconds of the down it takes, its time at the part's start), in order; a part
/// of 0 s is left out. The collapse's end is held on its last frame through `zetta.down_time`.
pub fn down_parts(z: &ZettaTune) -> [(&'static ClipDef, f32, f32); 3] {
    let up = down_getup();
    let from = z.zetta_down_up_from.clamp(0.0, up.length);
    [(&DOWN_FALL_A, z.zetta_down_fall_a.max(0.0), 0.0), (&DOWN_FALL_B, z.zetta_down_fall_b.max(0.0) + z.zetta_down_time.max(0.0), 0.0), (up, up.length - from, from)]
}

/// (2026-10-23) Where the down is at `t`: (part index, clip, time in it); None past its end.
pub fn down_at(t: f32, z: &ZettaTune) -> Option<(usize, &'static ClipDef, f32)> {
    let mut s = 0.0;
    for (i, (c, len, from)) in down_parts(z).into_iter().enumerate() {
        if len <= 0.0 {
            continue;
        }
        if t < s + len {
            return Some((i, c, (from + t - s).min(c.length)));
        }
        s += len;
    }
    None
}

/// (2026-10-23) The down's whole length (s).
pub fn down_length(z: &ZettaTune) -> f32 {
    down_parts(z).iter().map(|p| p.1).sum()
}

/// (2026-10-23) His staff is out of his hand (dropped, on the ground): from the collapse's throw to the get-up.
pub fn down_staff_dropped(d: &ZettaDown, z: &ZettaTune) -> bool {
    d.t >= z.zetta_down_fall_a.max(0.0) && d.t < down_up_at(z)
}

/// (2026-10-23) The get-up begins (s into the down): the line he says as he rises.
pub fn down_up_at(z: &ZettaTune) -> f32 {
    z.zetta_down_fall_a.max(0.0) + z.zetta_down_fall_b.max(0.0) + z.zetta_down_time.max(0.0)
}

/// (2026-10-23) STARTS THE DOWN on him (the hand-off of a Zettaflare that played to its end): rooted where he stands, the
/// collapse blended in from his pose, his fall's cry (his damage voice).
pub fn down_begin(p: &mut NpcPawn, z: &ZettaTune) {
    if p.status.ko() || p.status.donald.is_none() {
        return;
    }
    p.don_zetta.anchor = None;
    p.don_zetta.down = Some(ZettaDown { t: 0.0, at: p.position(), yaw: p.yaw, up_said: false });
    p.cmc.stop_movement();
    p.cmc.direct_xy = None;
    if !matches!(p.state, NpcStateId::Idle) {
        p.enter(NpcStateId::Idle);
    }
    p.anim.speed = 1.0;
    if let Some((_, c, time)) = down_at(0.0, z) {
        p.anim.play(c, z.zetta_down_blend.max(0.0));
        p.anim.time = time;
    }
    p.queue_voice(kh_core::host::party::VoiceGroup::Damage, 0, kh_core::host::party::VoiceMode::Replace);
    // (2026-10-23, the user: "Donalds eyes need to be closed when he falls down after Zettaflare") his eyes shut (the channel's
    // face clip, over his blink and look) from the fall to the get-up
    p.face.play(CHANNEL_FACE, z.zetta_face_blend, CHANNEL_FACE_LENGTH, true);
}

/// (2026-10-23, the user: "Zettaflare should overrule the \"beat boss\" cutscene and camera so that it's the only one that
/// plays and doesn't get interrupted") ZETTAFLARE OWNS THE PRESENTATION from its cast to the hand-back: a host keeps the
/// game's own deaths, phase changes and their cinematics out of it (kh-er zdefer.rs). (running, the party's step counter: a
/// counter that stops moving says the party is no longer stepped, so its word no longer holds.)
/// (ZETTA 2026-10-23, the user: "please make sure the enemy hurt animations are their heavy hurt animations, and it should
/// be paused around the middle of the animation so they are visibly hurt"; "could the fast forward happen while Donald is
/// charging so it is never actually visible?") THE PRE-POSE WINDOW: the host may bring the target into its heavy hurt's
/// middle now, unseen: the hit is at most [`PREPOSE_LEAD_S`] away and every shot from now to the hit is one of Donald's
/// ([`DONALD_SHOTS`]) with the target's body (its centre, head and feet) out of its frame (sampled every 1/30 s), so it is
/// never seen in that pose before the hit. Where Donald's last shot (910) shows the target up to the cut (its flight past it
/// at the end: a target near Donald, its head at the frame's foot), the window is the last [`PREPOSE_CUT_S`] before the
/// cut instead (the host's fast-forward takes about that). Pure.
pub fn prepose_window(shots: &[CamShot], s: f32, channel: f32, z: &ZettaTune, donald: Frame, target: Frame, beam: Basis, subject: CamSubject, end: Option<Frame>) -> bool {
    let hit = seq_time(z.zetta_hit_at, z, channel);
    if s >= hit || hit - s > PREPOSE_LEAD_S {
        return false;
    }
    if hit - s <= PREPOSE_CUT_S {
        return true;
    }
    let half = if subject.half > 0.0 { subject.half } else { z.zetta_target_center_height.max(1.0) * subject.size.max(0.2) };
    let body = [subject.at, subject.at + Vec3::Z * half, subject.at - Vec3::Z * half];
    let mut t = s;
    loop {
        let Some(pose) = cam_pose(shots, t, channel, z, donald, target, beam, subject, end) else { return false };
        if !DONALD_SHOTS.contains(&pose.shot) || body.iter().any(|b| frames_both(pose.position, pose.rotation, pose.fov, *b, *b, 1.0)) {
            return false;
        }
        if t >= hit - 1e-3 {
            return true;
        }
        t = (t + 1.0 / 30.0).min(hit - 1e-3);
    }
}

/// (ZETTA 2026-10-23) How long before the hit (sequence s) the pre-pose may begin (its fast-forward takes a few frames).
pub const PREPOSE_LEAD_S: f32 = 0.9;

/// (ZETTA 2026-10-23) The last moment before the hit's cut the pre-pose is always allowed (s: three frames).
pub const PREPOSE_CUT_S: f32 = 3.0 / 60.0;

static PREPOSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (ZETTA 2026-10-23) The pre-pose window is open now (the cinematic camera's word; never without it).
pub fn prepose_now() -> bool {
    PREPOSE.load(Ordering::Relaxed)
}

pub fn presentation() -> (bool, u32) {
    (PRESENTING.load(Ordering::Relaxed), PRESENT_STEP.load(Ordering::Relaxed))
}

static PRESENTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PRESENT_STEP: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// (2026-10-23, the user: "I don't like the voiceline he uses when getting back up") HIS LINE AS HE GETS UP, by the sheet's
/// `zetta.down_up_voice`: 1 BattleStart:1, 2 BattleStart:2 (his battle-start calls: short, determined), 3 his damage cry
/// (donald_damage_1, a random grunt), 4 RecoveryHP:1, 5 RecoveryHP:2 (the "healed" lines the user turned down), 0 none.
pub fn down_up_voice(z: &ZettaTune) -> Option<(kh_core::host::party::VoiceGroup, i32)> {
    use kh_core::host::party::VoiceGroup as G;
    match z.zetta_down_up_voice.round() as i32 {
        1 => Some((G::BattleStart, 1)),
        2 => Some((G::BattleStart, 2)),
        3 => Some((G::Damage, 0)),
        4 => Some((G::RecoveryHp, 1)),
        5 => Some((G::RecoveryHp, 2)),
        _ => None,
    }
}

/// (2026-10-23) A reaction or his KO has him: the down is over (his usual reaction plays; the player has him back when it
/// ends).
fn down_hurt(p: &NpcPawn) -> bool {
    p.status.ko() || matches!(p.state, NpcStateId::Damage | NpcStateId::Reflect | NpcStateId::BadStatStun | NpcStateId::BadStatBurn | NpcStateId::BadStatFreeze | NpcStateId::BadStatSneeze | NpcStateId::Die)
}

/// (2026-10-23) THE DOWN, EVERY STEP (the party step): his clips by the time, rooted and turned as he fell; a hit ends it;
/// at its end his idle (blended).
fn down_step(party: &mut Party, host: &mut PartyHost, dt: f32, z: &ZettaTune) {
    let ids: Vec<MemberId> = party.members().collect();
    for id in ids {
        let Some(p) = party.member_mut(id) else { continue };
        let Some(mut d) = p.don_zetta.down else { continue };
        if down_hurt(p) {
            p.don_zetta.down = None;
            p.anim.speed = 1.0;
            p.face.stop(Some(CHANNEL_FACE), z.zetta_face_blend); // (his eyes open with the reaction)
            continue;
        }
        d.t += dt;
        // rooted (the height free) and turned as he fell
        p.cmc.velocity.x = 0.0;
        p.cmc.velocity.y = 0.0;
        p.cmc.direct_xy = None;
        p.cmc.location.x = d.at.x;
        p.cmc.location.y = d.at.y;
        p.yaw = d.yaw;
        if !matches!(p.state, NpcStateId::Idle) {
            p.enter(NpcStateId::Idle);
        }
        // (the staff: on the ground from the collapse's end (where it fell), back to his hand from the get-up)
        // (the staff leaves his hand at the cutscene's weapon key: its fall is its own from then)
        let release = z.zetta_down_fall_a.max(0.0) + z.zetta_staff_release.clamp(0.0, z.zetta_down_fall_b.max(0.0));
        if d.t >= release && d.t < down_up_at(z) && p.don_zetta.staff.is_none() {
            let (pos, rot) = staff_of(id).unwrap_or((p.position() + crate::pawn::forward(p.yaw) * 30.0 + Vec3::Z * 90.0, glam::Quat::from_rotation_z(p.yaw.to_radians())));
            let hand_vel = staff_vel(id).unwrap_or(crate::pawn::forward(p.yaw) * -150.0);
            let fall = StaffFall::from_hand(pos, rot, hand_vel);
            p.don_zetta.staff = Some(StaffAway { ground: Some((fall.pos, fall.rot)), back: 0.0, fall });
        }
        if d.t >= down_up_at(z) {
            if let Some(StaffAway { ground: Some((at, _)), .. }) = p.don_zetta.staff {
                staff_light(p, host, Attach::World(at), false);
                p.don_zetta.staff = Some(StaffAway { ground: None, back: z.zetta_down_up_blend.max(0.0), fall: StaffFall::default() });
            }
        }
        match down_at(d.t, z) {
            Some((part, c, time)) => {
                if p.anim.clip.sequence != c.sequence {
                    // (into the get-up: a longer blend, his staff carried back from the ground to his hand over it)
                    let blend = if part == 2 { z.zetta_down_up_blend } else { z.zetta_down_blend };
                    p.anim.play(c, blend.max(0.0));
                    if part == 2 {
                        p.face.stop(Some(CHANNEL_FACE), z.zetta_face_blend); // (his eyes open as he gets up)
                    }
                }
                p.anim.time = time;
                p.anim.speed = 1.0;
                if !d.up_said && d.t >= down_up_at(z) {
                    d.up_said = true;
                    // his line as he rises (sheet `zetta.down_up_voice`: [`down_up_voice`])
                    if let Some((g, n)) = down_up_voice(z) {
                        p.queue_voice(g, n, kh_core::host::party::VoiceMode::IfIdle);
                    }
                }
                p.don_zetta.down = Some(d);
            }
            None => {
                p.don_zetta.down = None;
                p.face.stop(Some(CHANNEL_FACE), z.zetta_face_blend);
                p.to_idle();
            }
        }
    }
}

/// One Zettaflare (or ultimate) in progress.
#[derive(Clone, Debug, PartialEq)]
pub struct ZettaRun {
    pub caster: MemberId,
    /// The pawn's `don_cast.begun` at the cast's start (the run is that cast).
    pub begun: u32,
    pub spell: SpellRef,
    pub kind: RunKind,
    /// Seconds from the cast's start (an ultimate: on its scaled clock).
    pub t: f32,
    pub channel: f32,
    /// The caller's target (the aim's last fallback before `ahead`).
    pub given: CastTarget,
    fired: u128,
    live: Vec<LiveFx>,
    sounds: Vec<(usize, SoundHandle, f32)>,
    pub released: bool,
    pub hit: bool,
    pub cast_clip: bool,
    /// The face clip it put on his face slot.
    pub face: Option<&'static str>,
    /// Over (ended or interrupted): dropped after the step.
    pub done: bool,
    /// The aim's point when it last had a target (a target that dies keeps the rest of the spell where it stood).
    last_point: Option<Vec3>,
    /// The cinematic camera follows this run.
    pub cinematic: bool,
    /// (round 3) The target held for it (`Targets::script_hold`, from the channel's start to the hit).
    pub held: Option<TargetId>,
    /// (round 3) The beam frame, kept from the hit on (the beam no longer tracks the target).
    pub beam_at_hit: Option<Basis>,
    /// (round 3) The allies' zone of this run while its target is held.
    pub zone: Option<AvoidZone>,
    /// (round 4) The beam killed its target (its finale: the black silhouette crumbling, no throw).
    pub killed: bool,
    /// (round 4) The target whose finale look is being asked (`Targets::script_finale`).
    pub finale: Option<TargetId>,
    /// (round 4) The target the beam hit (its finale follows it, alive or not).
    pub hit_target: Option<TargetId>,
    /// (round 5) When the target was thrown (the sequence's time; once).
    pub thrown_at: Option<f32>,
    /// (round 5) The ending's frames, fixed at its start: his feet and the beam's line (`end`), the ending beam's start at
    /// his staff and its line in 3D (`end_beam`).
    pub end_frame: Option<Frame>,
    pub end_beam: Option<Basis>,
    /// (round 5) The ending holds him (`PawnZetta::ending`) for this run.
    pub ending: bool,
    /// (round 5) The zone round Donald (the allies keep away from him too), and the target the allies leave alone (the whole
    /// sequence).
    pub zone_donald: Option<AvoidZone>,
    pub reserved: Option<TargetId>,
    /// (round 5) Control came back early (the camera stick, a reaction): he is free and vulnerable, the allies free; the
    /// effects play on to their ends.
    pub handed: bool,
    /// (round 5) It rooted him once (an early hand-back is not undone).
    pub rooted: bool,
    /// (round 5c) The rock spikes of a low impact ([`spike_layout`]); laid out once as the ending begins.
    pub spikes: Vec<Spike>,
    pub spikes_laid: bool,
    /// (round 5c) the broken spikes' rocks, flying, at rest, crumbling
    pub rocks: Vec<RockPiece>,
    /// (round 5c) the ground dust the beam kicked up (laid at the ending's start)
    pub dust: Vec<(EffectHandle, &'static str)>,
    /// (round 5c) where the ground dust lies; the near layer was spawned
    pub dust_at: Vec<Vec3>,
    pub dust_near: bool,
    /// (round 5c) the event's dust: from where along the beam, its waves so far
    pub dust_from: Option<Vec3>,
    pub dust_waves: u32,
    pub dust_laid: bool,
    /// (round 5) THE BEAM ENGULFS THE ENEMY (the user: "The Zettaflare beam should encompass the entire enemy, it adapts to the
    /// enemy's size"): the beam's width and its impact effects x this: the target's height over the event's Terra-Xehanort's (or
    /// its radius over his), never under 1 (`zetta.beam_girth_margin` on top); fixed at the hit.
    pub girth: f32,
    /// (2026-10-23, the golden-frame proof: "make the size the beam is when it attacks the Sora in the test level the minimum
    /// size the beam can be") how far the ending beam's aim is raised over a small target's centre: to the Sora case's centre
    /// height over its feet (`zetta.target_center_height` x `zetta.beam_scale_min`), so the wide shot's beam, never thinner
    /// than the Sora case's, runs where that one would and not half under the floor (0 for the Sora case and larger). The
    /// beam into the target keeps its centre: the enemy shot is inside it.
    pub end_lift: f32,
    /// (2026-10-23, the user: "The target that Zettaflare is initiated on is THE one that is focused on in the cutscene") the
    /// aim fixed at the run's first step: no lock-on, soft target, retarget or reordering changes it; a target the game has
    /// no more is aimed at where it last was.
    pub locked: Option<ZettaAim>,
    /// (2026-10-23, the user: "any enemies that in the beam get pushed away with immense damage") the enemies the beam has
    /// swept (hit once each as its head reaches them), and those of them it killed (their finale as the target's).
    pub swept: Vec<TargetId>,
    pub swept_killed: Vec<TargetId>,
    /// (2026-10-23) The target shots' subject as last measured (size, radius, half height): kept when its body is gone.
    pub subject_last: Option<(f32, f32, f32)>,
    /// (2026-10-23) The body's drawn radius at the hit (cm): the beam's cards are laid in front of it in its shots.
    pub body_r: f32,
    /// (round 5) The look it asked this step (copied to `ZettaWorld::look_asked`).
    pub asked: Option<(kh_core::host::cinematic::Look, f32)>,
    /// (round 5) This run asked VFXR's cinematic look (kh_core::host::cinematic): it resets it on every way out.
    pub look: bool,
}

/// The cinematic camera's feed from the run it follows.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CamFeed {
    caster: MemberId,
    begun: u32,
    t: f32,
    channel: f32,
    donald: Frame,
    target: Frame,
    beam: Basis,
    subject: CamSubject,
    /// (round 5) the ending's frame once it began
    end: Option<Frame>,
}

/// The cinematic camera's state (`ZettaWorld::cam`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ZettaCam {
    feed: Option<CamFeed>,
    /// The run it follows ended by itself: blend back from its last view.
    ended: bool,
    /// The run was interrupted / he is KO: hand back at once.
    cut: bool,
    /// The player held the camera stick: blend back.
    pub skipped: bool,
    skip_t: f32,
    /// 0..1: the share of the event's view in the drawn one.
    pub weight: f32,
    last: Option<CamPose>,
    last_safe: Option<CamPose>,
    key: Option<(MemberId, u32)>,
    /// The run of `key` had its shots (blended out, handed back): its later feeds are ignored.
    retired: bool,
    shots: Vec<CamShot>,
    /// The shot drawn last (tests, the debug text).
    pub shot: Option<&'static str>,
    /// (round 5) The event's white at the last live step (eased out with the view).
    last_white: f32,
}

/// What the camera draws now: the event's view, and its share over the gameplay camera (0..1, eased).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZettaView {
    pub position: Vec3,
    /// Pitch, yaw, roll (deg, UE).
    pub rotation: Vec3,
    /// Horizontal degrees.
    pub fov: f32,
    /// The subject (Donald's chest, the target's centre): a host's collision pass sweeps from it to `position`.
    pub pivot: Vec3,
    pub weight: f32,
    pub shot: &'static str,
    /// (round 3) The safe framing (over Donald's shoulder toward the target): a host whose collision pass finds the shot
    /// blocked (pulled in by more than `block_share` of its distance) draws this instead, swept from `safe_pivot`.
    pub safe_position: Vec3,
    pub safe_rotation: Vec3,
    pub safe_fov: f32,
    pub safe_pivot: Vec3,
    pub block_share: f32,
    /// (round 3) The event's shot exactly (the charging close-up): a wall only pulls it in (no safe framing).
    pub exact: bool,
    /// (round 5) The camera is handing back (the run ended or was skipped: the blend-out). The hosts put the gameplay
    /// camera BEHIND DONALD as it begins (the user: "the camera after zettaflare should just be placed behind Donald, not where
    /// it originally was"): the shot behind him eases into it.
    pub handing_back: bool,
    /// (round 5) THE EVENT'S WHITE (0..1, x the view's share): a full-screen white the hosts draw over the picture (kg702b's
    /// fade to white as the blast ends, kg702c's fade from white into the ending's wide shot). The test level draws it
    /// (kh-app finish.rs' white plate); Elden Ring: the renderer's (VFXR / RSF), read through kh-partyhost
    /// `PartyRuntime::zetta_screen`.
    pub white: f32,
    /// (round 5c) THE SHAKE (the user: "Add screenshake to Zettaflare during the shot where the enemy is inside the beam"):
    /// an offset the hosts add to the drawn view (x `weight` already): turn (deg: pitch, yaw, roll) and sway (cm, camera-local:
    /// X forward, Y right, Z up). [`shake_at`].
    pub shake_rot: Vec3,
    pub shake_loc: Vec3,
    /// (ZETTA 2026-10-23) the beam's direction (unit; the hosts' mirrored shot is the other side of it)
    pub beam_dir: Vec3,
}

/// (round 5c) THE SHAKE OF THE TARGET IN THE BEAM at matinee time `mt`: from the hit (a jolt, 1.8 x, settling in 0.35 s) to
/// the fade to white (a steady rumble of `zetta.shake_deg` / `zetta.shake_cm`), easing in over 0.12 s and out over the
/// white's last 0.5 s. Three incommensurate waves under 14 Hz per axis (no beat, no strobe; motion only: flash-safe).
/// Zero with `zetta.shake` off. Returns (turn deg, sway cm).
pub fn shake_at(mt: f32, z: &ZettaTune) -> (Vec3, Vec3) {
    // (round 5c) the release's burst kicks the camera, briefly (the launch shot)
    let kt = mt - z.zetta_release_at;
    if z.zetta_shake >= 0.5 && kt >= 0.0 && kt < z.zetta_launch_kick_time.max(0.0) && mt < z.zetta_hit_at {
        let env = (1.0 - kt / z.zetta_launch_kick_time.max(1e-3)).powi(2);
        let a = z.zetta_launch_kick_deg.max(0.0) * env;
        return (Vec3::new(-a * (kt * 55.0).cos(), 0.5 * a * (kt * 71.0).sin(), 0.3 * a * (kt * 43.0).sin()), Vec3::ZERO);
    }
    if z.zetta_shake < 0.5 || mt < z.zetta_hit_at || mt > z.zetta_white_in_at {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    let smooth = |a: f32, b: f32, x: f32| {
        let t = ((x - a) / (b - a).max(1e-4)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let s = mt - z.zetta_hit_at;
    let env = smooth(0.0, 0.12, s) * (1.0 - smooth(z.zetta_white_in_at - 0.5, z.zetta_white_in_at, mt)) * (1.0 + 0.8 * (-s / 0.35).exp());
    let wave = |k: f32| ((7.3 * s + k) * std::f32::consts::TAU).sin() * 0.5 + ((11.1 * s + 2.0 * k) * std::f32::consts::TAU).sin() * 0.3 + ((13.7 * s + 3.0 * k) * std::f32::consts::TAU).sin() * 0.2;
    let a = z.zetta_shake_deg.max(0.0) * env;
    let c = z.zetta_shake_cm.max(0.0) * env;
    (Vec3::new(a * wave(0.11), 0.8 * a * wave(0.37), 0.4 * a * wave(0.71)), Vec3::new(0.0, c * wave(0.53), c * wave(0.89)))
}

/// The party's Zettaflares (`Party::don_zetta`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ZettaWorld {
    pub runs: Vec<ZettaRun>,
    pub log: Vec<ZettaLog>,
    pub cam: ZettaCam,
    /// TEST / DEBUG (`party don-ult ELEMENT`): the first Donald plays that element's ultimate presentation through
    /// [`ultimate_begin`] at the next step (the presentation only: the payload is DON-SPELLB's).
    pub debug_ults: Vec<Element>,
    /// (round 5) The cinematic look this party's Zettaflare asked last (the look and its blend; tests, the debug text).
    pub look_asked: Option<(kh_core::host::cinematic::Look, f32)>,
    /// (round 5c) WHERE THE ALLIES MAY WAIT: the host's choice of [`Party::don_zetta_wait_spots`] (on its ground, reachable)
    /// while no enemy is left to them through the span (empty: they are not waiting); each walks to its place at the
    /// nearest ([`wait_place`], [`step_out`]) and stands ready.
    pub wait_spots: Vec<Vec3>,
}

impl ZettaWorld {
    fn push(&mut self, e: ZettaLog) {
        if self.log.len() >= 1024 {
            self.log.drain(..256);
        }
        self.log.push(e);
    }

    /// Clears the log (tests).
    pub fn take_log(&mut self) -> Vec<ZettaLog> {
        std::mem::take(&mut self.log)
    }
}

/// STARTS A ZETTAFLARE (from `cast::begin`): the guards of a cast, the MP and `commit_cast` (Zettaflare's set price,
/// Resonance and Arcane Flow: DON1's rule), the channel clip, the cast run DON-MAGIC's step keeps (its shot never fires:
/// this module releases the spell; its clip = the channel clip: the mark of this module's cast). False = refused (as
/// `cast::begin`).
pub fn begin(p: &mut NpcPawn, plan: &CastPlan, target: CastTarget, host: &mut PartyHost) -> bool {
    let d = data();
    let z = zetta();
    let _ = host;
    if !takes(plan.spell) || p.status.donald.is_none() || p.status.ko() {
        return false;
    }
    // (the user, 2026-10-22: "make sure AI donald cannot use Zettaflare") only the Donald the player drives (the leader,
    // `NpcPawn::pad`) may begin it, whatever path asks
    if !may_cast_zettaflare(p) {
        return false;
    }
    if p.don_cast.run.is_some_and(|r| !r.shot) || holding(p) {
        return false;
    }
    // (round 5) the user: "Zettaflare should not be castable without a target": an enemy alive (the lock-on, else the soft
    // target: the pad's cast target), never a point ahead
    let CastTarget::Enemy(tg) = target else { return false };
    if !host.targets.info(tg).is_some_and(|i| i.flags.has(TargetFlags::ALIVE) && i.flags.has(TargetFlags::ENEMY)) {
        return false;
    }
    // (round 5) the user: "Donald can not perform any ultimate magic or zettaflare while up in the air, he must be on the
    // ground": refused in the air (a jump, a fall, a glide; the pad plays KH3's refused sound: pad::donald `refused`)
    if !p.on_ground() {
        return false;
    }
    if plan.mp > 0 && (p.status.mp < plan.mp || p.status.mp_charge_left > 0.0) {
        return false;
    }
    let channel = RunKind::Zetta.channel(&z);
    let air = root(p);
    if plan.mp > 0 {
        spend_all_mp(&mut p.status, plan.mp);
    }
    if let Some(st) = p.status.donald.as_mut() {
        let mods = st.abilities.mods();
        st.core.commit_cast(plan, &mods, &d);
    }
    p.anim.speed = 1.0;
    p.anim.play_loop(&CHANNEL_CLIP, z.zetta_blend_in);
    p.don_cast.begun += 1;
    p.don_cast.run = Some(CastRun { spell: plan.spell, plan: *plan, target, t: 0.0, shot_at: f32::MAX, end_at: seq_time(z.zetta_free_at, &z, channel), shot: false, hover: air, clip: CHANNEL_CLIP.sequence });
    true
}

/// (2026-10-23, the user: "it just needs 100% of your current MP to fire, and it makes your MP regen slower by 30%") Zettaflare
/// spends `mp` (all of it): his MP is 0 and its MP Charge starts (the spend rule only starts it below 0), which then runs
/// at `zetta.regen_mult` (DonCore::zetta_slow) until his MP has recharged.
pub(crate) fn spend_all_mp(s: &mut crate::FriendStatus, mp: i32) {
    crate::leader_magic::use_mp(s, mp);
    if s.mp <= 0 && s.mp_charge_left <= 0.0 {
        s.mp = 0;
        s.mp_charge_left = crate::tables::cparam(s.chara, "mp_charge_time");
    }
}

/// (round 5) DONALD IS INVINCIBLE THROUGH ZETTAFLARE (the user: "Donald is invincible during Zettaflare's cutscene"): from the
/// channel's start until control returns (the run's end: the hand-off) no hit lands on him (`Party::receive_hit` refuses it,
/// kh-partyhost marks his body invulnerable for the hosts' own routes); enemies never choose him (`shields`).
pub fn invincible(party: &Party, id: MemberId) -> bool {
    party.don_zetta.runs.iter().any(|r| r.caster == id && r.kind == RunKind::Zetta && !r.done && !r.handed)
}

/// ZETTAFLARE IS THE PLAYER'S (the user: "make sure AI donald cannot use Zettaflare"): only a Donald the pad drives (the
/// party leader) may cast it; an AI Donald (Sora or Goofy leading) never can.
pub fn may_cast_zettaflare(p: &NpcPawn) -> bool {
    p.pad.is_some() && p.status.donald.is_some()
}

/// The cast's root at its start: no movement from here (the user: "Donald cannot move while casting Zettaflare"), out of
/// a run / Move; in the air the hover (Sora's air cast rule, as cast::begin: gravity off until the release). True = in the
/// air.
fn root(p: &mut NpcPawn) -> bool {
    p.cmc.stop_movement();
    p.cmc.direct_xy = None;
    let air = !p.on_ground();
    if air {
        p.cmc.velocity.z = 0.0;
        p.cmc.gravity_off = true;
        p.cmc.set_flying();
    }
    if matches!(p.state, NpcStateId::Pad(crate::pad::RUN) | NpcStateId::Move) {
        p.enter(NpcStateId::Idle);
    }
    p.don_zetta.anchor = Some((p.position(), None));
    air
}

/// STARTS AN ULTIMATE'S PRESENTATION WITH ZETTAFLARE'S CIRCLE AND DONALD'S CASTING ANIMATION (DON-SPELLB's ultimate casts;
/// the user, 2026-10-22: "the same casting circle and Donald's casting animation as Zettaflare, without the cinematic
/// camera. The circle and all effects associated with it should be the colour of the element!"): the rune circle and its
/// light, the aura glows on his bones, the hand light, the staff charge and the staff's release flash, his channel and cast
/// clips and his effort, every effect tinted `color` ([`element_tint`]); the whole cast scaled so the release lands at
/// `ult.channel_time` (3 s; the clips faster by as much); no cinematic camera; rooted like Zettaflare (he turns to face
/// `target`) until the cast clip's end. The caller keeps the MP, `commit_cast`, the payload and its shot (it launches at
/// the returned time) and calls [`ultimate_cancel`] from its interrupt. Returns the seconds from now to the release;
/// `None` = refused (KO, not Donald, a Zettaflare or another ultimate under way).
pub fn ultimate_begin(p: &mut NpcPawn, color: Tint, target: CastTarget, host: &mut PartyHost) -> Option<f32> {
    let z = zetta();
    let _ = host;
    if p.status.donald.is_none() || p.status.ko() || holding(p) {
        return None;
    }
    // (round 5) the user: no ultimate magic in the air: the presentation refuses there too
    if !p.on_ground() {
        return None;
    }
    let kind = RunKind::Ult { tint: color };
    let k = kind.scale(&z);
    let release = kind.seq(z.zetta_release_at, &z, kind.channel(&z));
    root(p);
    p.anim.speed = 1.0 / k;
    p.anim.play_loop(&CHANNEL_CLIP, z.zetta_blend_in);
    p.don_zetta.serials = p.don_zetta.serials.wrapping_add(1);
    p.don_zetta.ult = Some(UltState { serial: p.don_zetta.serials, tint: color, target, cancel: false });
    Some(release)
}

/// STOPS AN ULTIMATE'S PRESENTATION AT ONCE (DON-SPELLB's interrupt: a hit before the release): its effects and sounds
/// are cut now, he is no longer rooted (the hit's reaction plays). Nothing when none runs.
pub fn ultimate_cancel(p: &mut NpcPawn) {
    if let Some(u) = p.don_zetta.ult.as_mut() {
        u.cancel = true;
        p.don_zetta.anchor = None;
        if p.cmc.gravity_off {
            p.cmc.gravity_off = false;
            if !p.on_ground() {
                p.cmc.set_falling();
            }
        }
        p.anim.speed = 1.0;
    }
}

/// This module's cast is running on him (from the channel's start to the cast clip's end): a Zettaflare, or an ultimate's
/// presentation not cancelled.
pub fn holding(p: &NpcPawn) -> bool {
    p.don_cast.run.is_some_and(|r| r.clip == CHANNEL_CLIP.sequence || r.clip == CAST_CLIP.sequence) || p.don_zetta.ult.is_some_and(|u| !u.cancel) || p.don_zetta.ending || p.don_zetta.down.is_some() // (2026-10-23) and down after it
}

/// ROOTED (pad::pre_move, after targeting, before the kits; marked): while Zettaflare or an ultimate casts (the channel's
/// start to the cast clip's end) the pad's frame moves nothing: no stick movement, no jump, no dodge, no attack, no buffered
/// press (the lock-on still changes; a cast queued in the menu waits). True = the frame is taken.
pub fn hold(p: &mut NpcPawn) -> bool {
    if !holding(p) {
        return false;
    }
    p.cmc.velocity.x = 0.0;
    p.cmc.velocity.y = 0.0;
    p.cmc.direct_xy = None;
    if matches!(p.state, NpcStateId::Pad(crate::pad::RUN) | NpcStateId::Move) {
        p.enter(NpcStateId::Idle);
    }
    // (2026-10-23) the lock-on frozen on the cast's target through the sequence
    if let (Some((l, s)), Some(d)) = (p.don_zetta.lock_keep, p.pad.as_mut()) {
        d.lock = l;
        d.soft = s;
    }
    if let Some(d) = p.pad.as_mut() {
        d.stick_magnitude = 0.0;
        d.frame.stick = Vec2::ZERO;
        d.frame.pad.pressed = kh_core::input::PadButtons::NONE;
        d.frame.pad.held = kh_core::input::PadButtons::NONE;
        d.buffered = 0.0;
        d.air_request = false;
        d.donald.buffered_x = 0.0;
        d.donald.dodge_buffered = false;
    }
    true
}

/// THE ROOT FOR EVERY MEMBER (the party step, after the pawns moved: an AI Donald's Move as well): his feet back on the
/// anchor (the height free: a fall or a hover), out of a Move, after the release his facing held; his clip (at the run's
/// rate) put back if something replaced it. Ends with the cast.
fn pin(p: &mut NpcPawn, run: &ZettaRun, z: &ZettaTune) {
    if !holding(p) {
        p.don_zetta.anchor = None;
        return;
    }
    let (at, yaw) = match p.don_zetta.anchor {
        Some(a) => a,
        None => {
            p.don_zetta.anchor = Some((p.position(), None));
            (p.position(), None)
        }
    };
    let now = p.position();
    if (now.x - at.x).abs() > 0.01 || (now.y - at.y).abs() > 0.01 {
        p.cmc.location.x = at.x;
        p.cmc.location.y = at.y;
    }
    p.cmc.velocity.x = 0.0;
    p.cmc.velocity.y = 0.0;
    if p.state == NpcStateId::Move {
        p.enter(NpcStateId::Idle);
    }
    match yaw {
        Some(y) => p.yaw = y,
        None if run.released => p.don_zetta.anchor = Some((at, Some(p.yaw))),
        None => {}
    }
    // the clip of this point of the cast (an AI's locomotion, a caller's own cast clip may have played another)
    let k = run.kind.scale(z);
    let chan = run.kind.seq(z.zetta_cast_start, z, run.channel);
    let end0 = seq_time(z.zetta_end_start, z, run.channel);
    let (clip, time) = if p.don_zetta.ending && run.ending && run.end_frame.is_some() && run.t + 1e-4 >= end0 {
        // (round 5) the ending: his pose under the beam (kg702c's n_ex001_70297wr, a loop)
        (&END_CLIP, (run.t - end0).max(0.0) % END_CLIP.length.max(1e-3))
    } else if run.t < chan {
        (&CHANNEL_CLIP, (run.t / k) % CHANNEL_CLIP.length.max(1e-3))
    } else {
        (&CAST_CLIP, ((run.t - chan) / k).min(CAST_CLIP.length))
    };
    if p.anim.clip.sequence != clip.sequence {
        if clip.looping {
            p.anim.play_loop(clip, 0.1);
        } else {
            p.anim.play(clip, 0.1);
        }
        p.anim.time = time;
    }
    p.anim.speed = 1.0 / k;
}

/// TEST / DEBUG (the test level's `party don-zetta-ready`, the harness): every Donald knows Zettaflare and meets its
/// condition now: every learned element reached Resonance 3 in this fight, a full Arcane Flow, his MP full (at least
/// Zettaflare's set cost, the cast rule's price: DON1 combat.rs, the user's 300 MP).
pub fn debug_make_ready(party: &mut Party) -> usize {
    let d = data();
    let ids: Vec<MemberId> = party.members().collect();
    let mut n = 0;
    for id in ids {
        let Some(s) = party.status_mut(id) else { continue };
        let max_mp = s.max_mp();
        let Some(st) = s.donald.as_mut() else { continue };
        if st.learned.arcane(ArcaneLine::Zettaflare) == 0 {
            st.learned.apply(super::learn::DonStep::Arcane(ArcaneLine::Zettaflare));
        }
        st.core.in_combat = true;
        st.core.fight_full = super::element::ElemSet(st.core.fight_full.0 | st.learned.elements().0);
        let max = d.tune.arcane_max.max(1.0) as u8;
        let now = st.core.clock;
        for _ in 0..max {
            st.core.arcane.gain(now, d.tune.arcane_life_plus.max(d.tune.arcane_life), max);
        }
        s.mp_charge_left = 0.0;
        // (2026-10-23) it spends all his current MP: any above 0 will do; a full bar here
        s.mp = max_mp.max(1);
        n += 1;
    }
    n
}

/// The aim now (see [`ZettaAim`]).
fn aim_now(party: &Party, host: &PartyHost, caster: MemberId, given: CastTarget) -> ZettaAim {
    let z = zetta();
    let Some(p) = party.member(caster) else { return ZettaAim::Point(Vec3::ZERO) };
    let alive = |t: TargetId| host.targets.info(t).is_some_and(|i| i.flags.has(TargetFlags::ALIVE));
    let (lock, soft) = p.pad.as_ref().map_or((TargetId::NONE, TargetId::NONE), |d| (d.lock, d.soft));
    let ahead = p.position() + crate::pawn::forward(p.yaw) * z.zetta_ahead + Vec3::Z * z.zetta_ahead_height;
    pick_aim(lock, soft, given, &alive, ahead)
}

/// The aim's point: the target's centre (else its last place), or the point.
fn aim_point(host: &PartyHost, aim: ZettaAim, last: Vec3) -> Vec3 {
    match aim {
        ZettaAim::Target(t) => host.targets.info(t).map_or(last, |i| i.center),
        ZettaAim::Point(p) => p,
    }
}

/// The debug ultimates (`party don-ult ELEMENT`): the first Donald, at the nearest foe, the presentation only.
fn debug_ults(party: &mut Party, host: &mut PartyHost) {
    for e in std::mem::take(&mut party.don_zetta.debug_ults) {
        let Some(id) = party.members().find(|i| party.status(*i).is_some_and(|s| s.donald.is_some())) else { break };
        let Some(p) = party.member_mut(id) else { continue };
        let pos = p.position();
        let target = magic_fx::foes(host).into_iter().min_by(|a, b| (a.position - pos).length().total_cmp(&(b.position - pos).length())).map_or(CastTarget::Point(pos + crate::pawn::forward(p.yaw) * 600.0), |f| CastTarget::Enemy(f.id));
        let _ = ultimate_begin(p, element_tint(e), target, host);
    }
}

/// THE PARTY STEP'S PART (party.rs, right after `cast::party_step`): new casts become runs; every run steps; the casters
/// stay rooted.
pub fn party_step(party: &mut Party, dt: f32, host: &mut PartyHost) {
    let z = zetta();
    debug_ults(party, host);
    // (2026-10-23) the host's word: Zettaflare owns the presentation (zettaflare::presentation)
    PRESENTING.store(party.don_zetta.runs.iter().any(|r| r.kind == RunKind::Zetta && !r.done), Ordering::Relaxed);
    if !PRESENTING.load(Ordering::Relaxed) {
        PREPOSE.store(false, Ordering::Relaxed); // (ZETTA 2026-10-23)
    }
    PRESENT_STEP.fetch_add(1, Ordering::Relaxed);
    // (2026-10-23) the lock-on kept from a Zettaflare's cast to its hand-back
    let ids: Vec<MemberId> = party.members().collect();
    for id in ids {
        let running = party.don_zetta.runs.iter().any(|r| r.caster == id && r.kind == RunKind::Zetta && !r.done);
        if let Some(p) = party.member_mut(id) {
            if running && p.don_zetta.lock_keep.is_none() {
                p.don_zetta.lock_keep = p.pad.as_ref().map(|d| (d.lock, d.soft));
            } else if !running && p.don_zetta.lock_keep.is_some() {
                p.don_zetta.lock_keep = None;
            }
        }
    }
    down_step(party, host, dt, &z); // (2026-10-23) the down after a Zettaflare
    staff_step(party, host, dt); // (2026-10-23) his staff where it fell, and back
    // 1. the casts begun: a Zettaflare (by `begin`, or by DON-MAGIC's own begin: taken over here), an ultimate's presentation
    let ids: Vec<MemberId> = party.members().collect();
    for id in ids {
        let Some(p) = party.member(id) else { continue };
        let zetta_cast = p.don_cast.run.filter(|r| takes(r.spell) && !r.shot).map(|r| (r, p.don_cast.begun));
        let ult = p.don_zetta.ult.filter(|u| !u.cancel);
        let (kind, begun, spell, given, mut run) = match (zetta_cast, ult) {
            (Some((r, begun)), _) => (RunKind::Zetta, begun, r.spell, r.target, Some(r)),
            (None, Some(u)) => (RunKind::Ult { tint: u.tint }, u.serial, SpellRef::Arcane(ArcaneLine::Zettaflare, 0), u.target, None),
            _ => continue,
        };
        if party.don_zetta.runs.iter().any(|r| r.caster == id && r.begun == begun && r.kind.is_ult() == kind.is_ult()) {
            continue;
        }
        let channel = kind.channel(&z);
        let Some(p) = party.member_mut(id) else { continue };
        if kind == RunKind::Zetta && !may_cast_zettaflare(p) {
            // an AI Donald never casts Zettaflare (the user): a cast some other path began is stopped before anything shows
            super::cast::interrupt(p, false);
            continue;
        }
        let taken = run.is_some_and(|r| r.clip != CHANNEL_CLIP.sequence);
        if let Some(r) = run.as_mut().filter(|_| taken) {
            // DON-MAGIC's begin ran (no cast.rs hook): its MP / commit_cast stand; the presentation is this module's
            r.shot_at = f32::MAX;
            r.end_at = seq_time(z.zetta_free_at, &z, channel);
            r.clip = CHANNEL_CLIP.sequence;
            p.anim.speed = 1.0;
            p.anim.play_loop(&CHANNEL_CLIP, z.zetta_blend_in);
            p.don_cast.run = Some(*r);
            p.don_zetta.anchor = Some((p.position(), None));
        }
        // (round 5) his eyes shut through the channel (Zettaflare and the ultimates: the same casting)
        p.face.play(CHANNEL_FACE, z.zetta_face_blend, CHANNEL_FACE_LENGTH, true);
        let pad = p.pad.is_some();
        let cinematic = kind == RunKind::Zetta && cinematic_on(&z) && (pad || z.zetta_cinematic_ai >= 0.5);
        let new = ZettaRun { caster: id, begun, spell, kind, t: run.map_or(0.0, |r| r.t), channel, given, fired: 0, live: Vec::new(), sounds: Vec::new(), released: false, hit: false, cast_clip: false, face: Some(CHANNEL_FACE), done: false, last_point: None, cinematic, held: None, beam_at_hit: None, zone: None, killed: false, finale: None, hit_target: None, thrown_at: None, end_frame: None, end_beam: None, ending: false, look: false, rooted: false, spikes: Vec::new(), spikes_laid: false, rocks: Vec::new(), dust: Vec::new(), dust_laid: false, dust_at: Vec::new(), dust_near: false, dust_from: None, dust_waves: 0, zone_donald: None, reserved: None, handed: false, asked: None, girth: 1.0, end_lift: 0.0, subject_last: None, body_r: 0.0, locked: None, swept: Vec::new(), swept_killed: Vec::new() };
        let aim = aim_now(party, host, id, given);
        if taken {
            party.don_zetta.push(ZettaLog::TakenOver { caster: id });
        }
        if let RunKind::Ult { tint } = kind {
            party.don_zetta.push(ZettaLog::UltBegun { caster: id, serial: begun, tint });
        }
        party.don_zetta.push(ZettaLog::Begun { caster: id, aim });
        party.don_zetta.push(ZettaLog::Clip { name: CHANNEL_CLIP.sequence, t: new.t });
        party.don_zetta.runs.push(new);
    }
    // a presentation cancelled before it became a run
    for id in party.members().collect::<Vec<_>>() {
        let serial = party.member(id).and_then(|p| p.don_zetta.ult.filter(|u| u.cancel).map(|u| u.serial));
        if let Some(sn) = serial {
            if !party.don_zetta.runs.iter().any(|r| r.caster == id && r.kind.is_ult() && r.begun == sn) {
                if let Some(p) = party.member_mut(id) {
                    p.don_zetta.ult = None;
                }
            }
        }
    }
    // 2. every run, then the root
    let mut runs = std::mem::take(&mut party.don_zetta.runs);
    for run in runs.iter_mut() {
        step_run(run, party, host, dt, &z);
        if let Some(a) = run.asked.take() {
            party.don_zetta.look_asked = Some(a);
        }
        if run.look {
            look_beat();
        }
        if !run.done {
            if let Some(p) = party.member_mut(run.caster) {
                pin(p, run, &z);
            }
        }
    }
    for run in runs.iter().filter(|r| r.done) {
        if let Some(p) = party.member_mut(run.caster) {
            if run.kind.is_ult() && p.don_zetta.ult.is_some_and(|u| u.serial == run.begun) {
                p.don_zetta.ult = None;
                p.anim.speed = 1.0;
            }
            if !holding(p) {
                p.don_zetta.anchor = None;
            }
        }
    }
    runs.retain(|r| !r.done);
    // (round 5) the look of a Zettaflare no run of this party plays any more (a load, a party rebuilt): back at once
    if LOOK_OWNED.load(Ordering::Relaxed) && !runs.iter().any(|r| r.look) && !party.don_zetta.runs.iter().any(|r| r.look) && look_stale() {
        kh_core::host::cinematic::reset(0.0);
        LOOK_OWNED.store(false, Ordering::Relaxed);
    }
    let mut keep = runs;
    keep.append(&mut party.don_zetta.runs);
    party.don_zetta.runs = keep;
}

/// Ends a run now: its effects stop emitting (and fade as authored), its sounds fade, the face goes back; the camera hands
/// back at once when it was interrupted.
fn stop_all(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, why_interrupted: bool) {
    stop_all_now(run, party, host, why_interrupted, false);
}

/// [`stop_all`]; `now` = cut at once (an ultimate cancelled by its caller): the effects removed, the sounds cut.
fn stop_all_now(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, why_interrupted: bool, now: bool) {
    end_finale(run, host);
    end_look(run, if why_interrupted || now { 0.0 } else { zetta().zetta_cam_blend_out });
    release_hold(run, host);
    end_ending(run, party);
    spikes_stop(run, party, host, !now); // (round 5c)
    for l in run.live.drain(..) {
        host.effects.stop(l.handle, now || l.cut);
        let row = &DON_ZETTA_FX[l.row];
        party.don_zetta.push(ZettaLog::Stop { row: row.id, t: run.t });
    }
    for (row, h, _) in run.sounds.drain(..) {
        host.audio.stop(h, if now { 0.05 } else { DON_ZETTA_FX[row].fade.max(zetta().zetta_sound_fade.max(0.2)) });
    }
    if let Some(name) = run.face.take() {
        let blend = zetta().zetta_face_blend;
        if let Some(p) = party.member_mut(run.caster) {
            p.face.stop(Some(name), blend);
        }
    }
    if run.cinematic {
        party.don_zetta.cam.end(run.caster, run.begun, why_interrupted || party.member(run.caster).map_or(true, |p| p.status.ko()));
    }
    if why_interrupted {
        party.don_zetta.push(ZettaLog::Interrupted { t: run.t });
    }
    run.done = true;
}

fn step_run(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, dt: f32, z: &ZettaTune) {
    if run.done {
        return;
    }
    let pm = PartyMember(run.caster.0);
    // the caster gone / KO: everything stops
    if party.member(run.caster).map_or(true, |p| p.status.ko()) {
        release_hold(run, host);
        let interrupted = !run.released;
        stop_all(run, party, host, interrupted);
        if !interrupted && run.cinematic {
            party.don_zetta.cam.end(run.caster, run.begun, true);
        }
        return;
    }
    if run.kind.is_ult() {
        // an ultimate's own clock; its caller's cancel (its interrupt) cuts it at once
        let state = party.member(run.caster).and_then(|p| p.don_zetta.ult).filter(|u| u.serial == run.begun);
        if state.map_or(true, |u| u.cancel) {
            stop_all_now(run, party, host, true, true);
            return;
        }
        run.t += dt;
    } else {
        // the leader changed under the cast (the pad no longer drives him): it is cancelled cleanly (an AI Donald never
        // casts Zettaflare)
        if !run.released && party.member(run.caster).is_some_and(|p| !may_cast_zettaflare(p)) {
            if let Some(p) = party.member_mut(run.caster) {
                super::cast::interrupt(p, false);
            }
            release_hold(run, host);
            stop_all(run, party, host, true);
            return;
        }
        // the clock: the cast run's until the release (DON-MAGIC's step counts it), then its own
        let cast = party.member(run.caster).and_then(|p| p.don_cast.run.filter(|r| r.spell == run.spell).map(|r| (r, p.don_cast.begun)));
        match cast {
            Some((r, begun)) if begun == run.begun => run.t = r.t,
            _ if !run.released => {
                // DON-MAGIC's interrupt (a hit, Cast Guard failed) or another cast replaced it before the release
                release_hold(run, host);
                stop_all(run, party, host, true);
                return;
            }
            _ => run.t += dt,
        }
    }
    let t = run.t;
    let channel = run.channel;
    let release = run.kind.seq(z.zetta_release_at, z, channel);
    let hit = seq_time(z.zetta_hit_at, z, channel);
    // (round 5) with the ending the run ends at the hand-off; without it the last shot follows the throw a moment
    let ending = run.kind == RunKind::Zetta && ending_on(z);
    let end = run.kind.end(z, channel) + if run.kind == RunKind::Zetta && !ending { z.zetta_throw_follow.max(0.0) } else { 0.0 };
    // (round 5) THE ENDING GIVES HIM BACK EARLY: a hit (his damage / status reactions), his KO (above), the pad no longer
    // driving him, or the camera stick held (the camera hands back): he is free at once, the target let go; the effects
    // play on to their ends
    if run.ending {
        let hurt = party.member(run.caster).map_or(true, |p| !may_cast_zettaflare(p) || matches!(p.state, NpcStateId::Damage | NpcStateId::Reflect | NpcStateId::BadStatStun | NpcStateId::BadStatBurn | NpcStateId::BadStatFreeze | NpcStateId::BadStatSneeze | NpcStateId::Die));
        let skipped = run.cinematic && !party.don_zetta.cam.live_for(run.caster, run.begun);
        if hurt || skipped {
            spikes_stop(run, party, host, true); // (round 5c)
            // (round 5c) the ending's sounds fade out with it (nothing carries on into play)
            for (row, h, _) in run.sounds.drain(..) {
                host.audio.stop(h, DON_ZETTA_FX[row].fade.max(z.zetta_sound_fade.max(0.0)));
            }
            end_ending(run, party);
            release_hold(run, host);
            run.zone = None;
            run.zone_donald = None;
            run.reserved = None;
            run.handed = true;
            end_look(run, if hurt { 0.0 } else { z.zetta_cam_blend_out });
            if hurt && run.cinematic {
                party.don_zetta.cam.end(run.caster, run.begun, true);
            }
        }
    }
    // (round 5) the camera handed back early (the stick): the look eases back with it
    if run.look && !party.don_zetta.cam.live_for(run.caster, run.begun) {
        end_look(run, z.zetta_cam_blend_out);
    }
    // he faces the target until the release (frame-rate independent turn)
    // (2026-10-23) the target of the cast, fixed for the whole sequence
    let first = *run.locked.get_or_insert_with(|| aim_now(party, host, run.caster, run.given));
    let mut aim = match first {
        ZettaAim::Target(tg) if host.targets.info(tg).is_none() => ZettaAim::Point(run.last_point.unwrap_or_else(|| aim_point(host, first, Vec3::ZERO))),
        a => a,
    };
    let ahead = party.member(run.caster).map_or(Vec3::ZERO, |p| p.position() + crate::pawn::forward(p.yaw) * z.zetta_ahead + Vec3::Z * z.zetta_ahead_height);
    let mut goal = aim_point(host, aim, ahead);
    match aim {
        ZettaAim::Target(_) => run.last_point = Some(goal),
        ZettaAim::Point(_) => {
            if let Some(last) = run.last_point {
                // its target died: the rest of the spell stays where it stood
                goal = last;
                aim = ZettaAim::Point(last);
            }
        }
    }
    if !run.released {
        if let Some(p) = party.member_mut(run.caster) {
            let flat = crate::pawn::horizontal(goal - p.position());
            if flat.length() > 1.0 {
                let diff = crate::pawn::wrap180(crate::pawn::yaw_of(flat) - p.yaw);
                let max = z.zetta_turn_rate.max(0.0) * dt;
                p.yaw = crate::pawn::wrap180(p.yaw + diff.clamp(-max, max));
            }
        }
    }
    let (donald, target) = frames(party, run, goal);
    // (round 3) the beam frame: the staff (the release point, Donald's frame) to the target's centre; kept from the hit on
    // (round 5) the wand's tip through the cast clip (the event's release flash track)
    let staff = donald.point(staff_at(z, matinee_time(t / run.kind.scale(z), z, channel)));
    // (round 5c, the user: "the beam has to look like it comes from Donald's wand tip", in the shot where it first appears) up
    // to the hit the beam starts at his wand's tip as the host draws it (`feed_tip`), else at the event's release track
    let staff = if !run.hit && matinee_time(t / run.kind.scale(z), z, channel) + 1e-4 < z.zetta_hit_at { tip_of(run.caster).unwrap_or(staff) } else { staff };
    let beam = run.beam_at_hit.unwrap_or_else(|| Basis::beam(staff, goal));
    // (round 5) the beam engulfs the enemy: its girth from the target's size, until the hit
    if run.kind == RunKind::Zetta && !run.hit {
        if let ZettaAim::Target(tg) = aim {
            if let Some(i) = host.targets.info(tg) {
                // (2026-10-23) both ways: a small target gets a thinner beam (and the effects round it scale down), a giant a
                // wider one; its size from what the host draws of it
                let (r, h) = target_extent(&*host.targets, &i);
                run.girth = beam_girth(h, r, z);
                run.body_r = r;
                // (2026-10-23) the ending beam of a target under the Sora case runs where the Sora case's would (see `end_lift`)
                run.end_lift = (i.position.z + z.zetta_target_center_height.max(1.0) * z.zetta_beam_scale_min.max(0.05) - i.center.z).max(0.0);
            }
        }
    }
    // (round 5) THE ENDING BEGINS (kg702c, in white): its frames fixed now (his feet, the beam's line; the ending beam
    // from his staff up at least `zetta.end_beam_pitch` on that line, or the line to the target's centre where it was hit
    // when that rises more); with the cinematic camera he is held in his pose under the beam until the hand-off
    let end0 = seq_time(z.zetta_end_start, z, channel);
    if ending && run.end_frame.is_none() && t + 1e-4 >= end0 && t + 1e-4 < end {
        let flat = crate::pawn::horizontal(beam.x);
        let feet = party.member(run.caster).map_or(donald.origin, |p| p.position());
        let yaw = if flat.length() > 1e-3 { crate::pawn::yaw_of(flat) } else { party.member(run.caster).map_or(donald.yaw, |p| p.yaw) };
        let fr = Frame { origin: feet, yaw };
        // WHERE THE BEAM WAS AIMED (the user, round 5): the ending's beam is the beam itself, from the wand's tip along the
        // line to the aim point (the target's centre where it was hit, or the last aim), shrinking there
        let mut b = beam;
        b.origin = staff;
        // (2026-10-23) a target under the Sora case: the ending beam on the Sora case's line (`end_lift`)
        if run.end_lift > 0.5 {
            b = Basis::beam(staff, goal + Vec3::Z * run.end_lift);
        }
        run.end_frame = Some(fr);
        run.end_beam = Some(b);
        if run.ending {
            if let Some(p) = party.member_mut(run.caster) {
                p.yaw = yaw;
                p.don_zetta.anchor = Some((feet, Some(yaw)));
            }
        }
    }
    // (round 5c) the rock spikes of a low impact (the ending only: the wide shot and the shot behind him)
    if run.kind == RunKind::Zetta && !run.handed {
        spikes_step(run, party, host, t, z, staff, ending);
        dust_step(run, party, host, t, z);
    }
    // (round 5) HE STAYS ROOTED TO THE END (the coordinator: "Donald stays rooted and the camera stays cinematic for the whole
    // extended span"): from the release on, the sequence holds him in the cast clip's last pose (then the ending's pose) until
    // the hand-off, while the cinematic camera plays it; a hit, his KO, the pad leaving him or the camera stick frees him
    if run.kind == RunKind::Zetta && run.released && !run.rooted && t + 1e-4 < end && run.cinematic && party.don_zetta.cam.live_for(run.caster, run.begun) {
        if let Some(p) = party.member_mut(run.caster).filter(|p| may_cast_zettaflare(p) && !matches!(p.state, NpcStateId::Damage | NpcStateId::Reflect | NpcStateId::BadStatStun | NpcStateId::BadStatBurn | NpcStateId::BadStatFreeze | NpcStateId::BadStatSneeze | NpcStateId::Die)) {
            let at = p.position();
            let yaw = p.yaw;
            p.cmc.stop_movement();
            p.cmc.direct_xy = None;
            if p.don_zetta.anchor.is_none() {
                p.don_zetta.anchor = Some((at, Some(yaw)));
            }
            p.don_zetta.ending = true;
            run.ending = true;
        }
        run.rooted = true;
    }
    // (round 5) THE SURVIVOR WAITS THROUGH THE ENDING: thrown, it lands and gets up (`zetta.end_rehold_delay`), then it is
    // held again (turned to Donald, its own idle) until the hand-off; the allies keep away from it again
    let rehold = ending && run.ending && !run.killed && run.thrown_at.is_some_and(|t0| t + 1e-4 >= t0 + z.zetta_end_rehold_delay.max(0.0)) && t + 1e-4 < end;
    // (round 3) THE TARGET HELD (the user: "the enemy needs to freeze, look at Donald"): from the channel's start to the
    // hit it does not move or attack and turns to face Donald (Zettaflare only)
    let hold_end = seq_time(throw_at(z), z, channel).min(end); // (round 5c) with the ending: the wide shot
    if run.kind == RunKind::Zetta && !run.handed && (t + 1e-4 < hold_end || rehold) {
        let want = if rehold {
            run.hit_target.filter(|tg| host.targets.info(*tg).is_some_and(|i| i.flags.has(TargetFlags::ALIVE) && i.hp > 0.0))
        } else {
            match aim {
                ZettaAim::Target(tg) => Some(tg),
                ZettaAim::Point(_) => None,
            }
        };
        if run.held != want {
            release_hold(run, host);
        }
        if let (Some(tg), Some(p)) = (want, party.member(run.caster)) {
            // (round 5c) from the hit it plays its own hurt reaction (the hosts' damage-reaction route), not its idle
            // (2026-10-23, the user: "make sure the target, if they survive, is thrown BACK towards the direction of the beam,
            // not to the side") from the hit it faces straight back up the beam's line (not at Donald off its axis): a host's
            // own knock-back, relative to the body, then runs along the beam
            let face = match (run.beam_at_hit, host.targets.info(tg)) {
                (Some(b), Some(i)) => i.position - crate::pawn::horizontal(b.x).normalize_or(Vec3::X) * 500.0,
                _ => p.position(),
            };
            host.targets.script_hold(tg, Some(kh_core::host::ScriptHold { face, turn_rate: z.zetta_hold_turn_rate.max(0.0), dt, hurt: run.hit }));
            run.held = Some(tg);
        }
    }
    // (round 5) THE ALLIES STAY OUT OF IT, THE WHOLE SEQUENCE (the user: "Make SURE ally's DO NOT attack the zettaflared enemy
    // or get anywhere close to Donald or the Zettaflare beam while he is casting it or charging it up"): from the channel's
    // start to the hand-off the zone round the target (where it is now, else the last aim) with the beam's corridor, and a
    // zone round Donald (`zetta.ally_donald`); the target is reserved (no ally picks it) the whole time
    if run.kind == RunKind::Zetta && !run.handed {
        let tg_now = run.held.or(run.hit_target).or(match aim {
            ZettaAim::Target(tg) => Some(tg),
            ZettaAim::Point(_) => None,
        });
        run.reserved = tg_now.or(run.reserved);
        let (feet, rad, size) = tg_now.and_then(|tg| host.targets.info(tg)).map_or((crate::pawn::horizontal(goal) + Vec3::Z * (goal.z - z.zetta_target_center_height), 50.0, 1.0), |i| (i.position, i.radius, i.half_height.max(10.0) / z.zetta_target_center_height.max(1.0)));
        let start = staff + crate::pawn::horizontal(goal - staff).normalize_or_zero() * 150.0;
        run.zone = Some(AvoidZone { at: feet, radius: z.zetta_ally_clear * size.clamp(0.5, 4.0) + rad, from: start, to: goal, half_width: z.zetta_ally_beam * run.girth.max(1.0) /* (round 5c) as wide as the beam */, margin: z.zetta_ally_margin, caster: run.caster });
        run.zone_donald = Some(AvoidZone { at: donald.origin, radius: z.zetta_ally_donald, from: donald.origin, to: donald.origin, half_width: 0.0, margin: z.zetta_ally_margin, caster: run.caster });
    }
    // the cinematic camera's feed
    if run.cinematic {
        // the target's animation, read from the host at run time: held, its own loop (`held_animated`); thrown, its
        // reaction (`reaction_state`, else the hold's rule while the host still shows it)
        let tg_now = run.held.or(match aim {
            ZettaAim::Target(tg) => Some(tg),
            ZettaAim::Point(_) => None,
        });
        // (2026-10-23) a body gone (killed, its info dropped): the size it last had (no jump to another frame)
        let last = run.subject_last.unwrap_or((1.0, 50.0, 0.0));
        let (at, size, animated, radius, half) = match tg_now {
            Some(tg) => host.targets.info(tg).map_or((goal, last.0, true, last.1, last.2), |i| {
                let animated = if run.held.is_some() { host.targets.held_animated(tg) } else { host.targets.reaction_state(tg) != 0 || host.targets.held_animated(tg) };
                // (round 5) its finale (the white, the silhouette crumbling) is its look: the target shots stay on it (the user: the
                // camera always shows the target crumbling)
                let animated = animated || run.finale.is_some() || run.killed;
                // (2026-10-23) its size both ways, from what the host draws of it: the shots zoom out for a giant, in for a small one
                let (r, h) = target_extent(&*host.targets, &i);
                (i.center, target_size(h, r, z), animated, r.max(10.0), i.half_height.max(1.0))
            }),
            None => (goal, last.0, true, last.1, last.2),
        };
        run.subject_last = Some((size, radius, half));
        party.don_zetta.cam.feed(CamFeed { caster: run.caster, begun: run.begun, t, channel, donald, target, beam, subject: CamSubject { at, size, animated, radius, half }, end: run.end_frame });
    }
    let tint = match run.kind {
        RunKind::Ult { tint } => Some(tint),
        RunKind::Zetta => None,
    };
    // the rows due
    let sched = schedule(z, channel, run.kind);
    for &(i, start, stop) in &sched {
        if run.fired & (1u128 << i) != 0 || t + 1e-4 < start {
            continue;
        }
        run.fired |= 1u128 << i;
        let row = &DON_ZETTA_FX[i];
        match row.kind {
            "clip" => {
                if row.asset == CAST_CLIP.sequence && !run.cast_clip {
                    run.cast_clip = true;
                    let zetta_face = (run.kind == RunKind::Zetta).then(|| face_clip(voice_ja()));
                    let ult = run.kind.is_ult();
                    let speed = 1.0 / run.kind.scale(z);
                    if let Some(p) = party.member_mut(run.caster).filter(|p| ult || p.don_cast.run.is_some_and(|r| r.spell == run.spell)) {
                        p.anim.speed = speed;
                        p.anim.play(&CAST_CLIP, 0.0);
                        if let Some(r) = p.don_cast.run.as_mut().filter(|_| !ult) {
                            r.clip = CAST_CLIP.sequence;
                        }
                        // the beak: his lip track of "Zettaflare!" with the cast (the event's face track starts with it)
                        if let Some(name) = zetta_face {
                            p.face.play(name, z.zetta_face_blend, FACE_LENGTH, false);
                            run.face = Some(name);
                        } else if run.face == Some(CHANNEL_FACE) {
                            // (round 5) an ultimate's cast: his eyes open again (no lip track of its own)
                            p.face.stop(Some(CHANNEL_FACE), z.zetta_face_blend);
                            run.face = None;
                        }
                    }
                    party.don_zetta.push(ZettaLog::Clip { name: CAST_CLIP.sequence, t });
                    if let Some(name) = run.face {
                        party.don_zetta.push(ZettaLog::Face { name, t });
                    }
                }
            }
            // (round 5c) the silhouette's breaking: only when Zettaflare killed the target, on it
            "se_kill" => {
                if !run.killed {
                    continue;
                }
                let at = run.hit_target.and_then(|tg| host.targets.info(tg)).map_or(goal, |i| i.center);
                spike_sound(run, party, host, row.id, at, 1.0, t);
            }
            "se" | "voice" => {
                let Some(p) = party.member_mut(run.caster) else { continue };
                let h = p.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(row.asset)), name: row.asset, at: Attach::party(pm, None), volume: 1.0, pitch: 1.0, looping: false, spatial: false });
                // (round 5c) every sound is kept (its own end, else the run's): each fades out when it is stopped
                run.sounds.push((i, h, stop));
                party.don_zetta.push(ZettaLog::Sound { row: row.id, t });
            }
            // (round 5) THE EVENT'S LOOK (VFXR's cinematic exposure control): its exposure, key light and bloom threshold
            // as the event's volume and key light change them, while the cinematic camera plays this run
            "look" => {
                if run.cinematic && party.don_zetta.cam.live_for(run.caster, run.begun) {
                    let o = v3(row.offset);
                    kh_core::host::cinematic::set(kh_core::host::cinematic::Look { exposure_ev: o.x, key_ev: o.y, bloom_threshold: o.z }, row.fade.max(0.0));
                    run.asked = Some((kh_core::host::cinematic::Look { exposure_ev: o.x, key_ev: o.y, bloom_threshold: o.z }, row.fade.max(0.0)));
                    LOOK_OWNED.store(true, Ordering::Relaxed);
                    run.look = true;
                }
            }
            "fx" => {
                let Some((at, rotation, frame)) = place(row, run, party, host, aim, goal, donald, beam) else { continue };
                // (round 5c) AN ULTIMATE'S RELEASE FLASH IS BRIEF (DON-SPELLB / the coordinator: a long whiteout after the
                // ultimates' release): the event's flash and its light, made for a cut away, are removed `ult.flash_time` after
                // they start, so the payload reads; Zettaflare's own cinematic keeps the event's
                let cut = run.kind.is_ult() && matches!(row.id, "fx.world.91_zfr_00" | "fx.world.91_rit_00");
                let stop = if cut { stop.min(t + z.ult_flash_time.max(0.0)) } else { stop };
                let persistent = stop.is_finite();
                // (round 5c) the shot's size override as it is born
                let ovr = if run.cinematic { shot_scale(row.id, shot_at(matinee_time(t / run.kind.scale(z), z, channel), z)) } else { 1.0 };
                let Some(p) = party.member_mut(run.caster) else { continue };
                let h = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(row.asset)), name: row.asset, at, rotation, scale: (if row.scale > 0.0 { row.scale } else { 1.0 }) * (if matches!(row.frame, "beam" | "beam_socket" | "target_socket" | "world_target" | "end_beam") { run.girth } else { 1.0 }) * (if row.frame == "beam_stream" { 1.0 } else { ovr }), persistent });
                if row.frame == "end_beam" {
                    // (2026-10-23) the ending beam grown across the line only (its length as authored: [`end_beam_scale`])
                    if let Some(b) = run.end_beam {
                        let s = (if row.scale > 0.0 { row.scale } else { 1.0 }) * run.girth * ovr;
                        let (w, sc) = end_beam_draw(row, b, s, run.girth, (matinee_time(t / run.kind.scale(z), z, channel) - row.start).max(0.0));
                        host.effects.transform(h, w, rotation, sc);
                    }
                }
                if row.frame == "end_impact" {
                    // (round 5) the final cloud: sized by its distance from the shot behind him
                    if let Some((at, s, rot)) = end_impact_place(row, run) {
                        host.effects.transform(h, at, rot, Vec3::splat(s));
                    }
                }
                if row.frame == "beam_stream" {
                    // the beam laid along the line: its length now (it grows from the staff)
                    let eye = party.don_zetta.cam.last.map(|p| p.position); // (round 5c) turned to the camera
                    // (2026-10-23) in the target's shots the beam drawn to fill the frame, in front of the body
                    let _ = eye;
                    let (o, r, sc) = stream_drawn(row, staff, beam, matinee_time(t / run.kind.scale(z), z, channel), z, shot_girth(party, run.girth), party.don_zetta.cam.last, run.hit && run.cinematic && subject_shot_now(party, z), run.body_r, feet_z_of(run, beam));
                    host.effects.transform(h, o, r, sc);
                }
                if row.frame == "beam_head" {
                    // (round 5c) the beam's head: at the beam's reach now
                    host.effects.transform(h, launch_head(staff, beam, matinee_time(t / run.kind.scale(z), z, channel), z), Vec3::ZERO, Vec3::splat(row.scale.max(0.01)));
                }
                if row.frame == "end_aura" {
                    // (round 5c) the aura on him: its place and size now
                    if let Some((o, k)) = aura_place(row, run, party, matinee_time(t / run.kind.scale(z), z, channel), z) {
                        host.effects.transform(h, o, Vec3::ZERO, Vec3::splat(k * ovr));
                    }
                }
                if row.frame == "end_glow" {
                    // (round 5c) the light at the source: its size now
                    if let Some(b) = run.end_beam {
                        let gs = glow_share(row, matinee_time(t / run.kind.scale(z), z, channel)) * run.girth * ovr;
                        host.effects.transform(h, end_glow_at(row, b, gs), v3(row.rotation), Vec3::splat(gs));
                    }
                }
                if let Some(tn) = tint {
                    // the ultimates: the circle and everything with it in the element's colour
                    host.effects.tint(h, tn);
                }
                if persistent {
                    run.live.push(LiveFx { row: i, handle: h, end: stop, frame, cut, shot_scale: ovr });
                }
                party.don_fx.log.push(FxLog::Effect { spell: Some(run.spell), name: row.asset });
                let world = match at {
                    Attach::World(w) => w,
                    _ => frame.origin,
                };
                party.don_zetta.push(ZettaLog::Spawn { row: row.id, t, at: world, rotation, handle: h });
            }
            _ => {}
        }
    }
    // the moving emitters (their tracks, in the frame they were placed in); (round 3) the beam's rows follow the beam frame
    // up to the hit (the target moving: the beam keeps tracking it), the beam itself grows from the staff to the target
    let mt = matinee_time(t / run.kind.scale(z), z, channel);
    // (round 5c) the shot drawn now: its size overrides ([`shot_scale`]) are applied as it cuts in
    let shot = if run.cinematic { shot_at(mt, z) } else { None };
    let mut rescale: Vec<(usize, f32)> = Vec::new();
    for (k, l) in run.live.iter().enumerate() {
        let row = &DON_ZETTA_FX[l.row];
        let ovr = shot_scale(row.id, shot);
        match row.frame {
            "beam_stream" => {
                // (2026-10-23) in the target's shots the beam drawn to fill the frame, in front of the body
                let (o, r, sc) = stream_drawn(row, staff, beam, mt, z, shot_girth(party, run.girth), party.don_zetta.cam.last, run.hit && run.cinematic && subject_shot_now(party, z), run.body_r, feet_z_of(run, beam));
                host.effects.transform(l.handle, o, r, Vec3::new(sc.x * ovr, sc.y * ovr, sc.z));
            }
            "beam" if run.beam_at_hit.is_none() => {
                let at = beam.point(if row.path.is_empty() { v3(row.offset) } else { path_at(row, mt) });
                host.effects.transform(l.handle, at, beam.rotation(v3(row.rotation)), Vec3::splat(if row.scale > 0.0 { row.scale } else { 1.0 }));
            }
            "beam" if !row.path.is_empty() => host.effects.place(l.handle, beam.point(path_at(row, mt))),
            "end_beam" => {
                // (2026-10-23) drawn longer for the wide shot, its back end kept at the tip as its cards lengthen
                if let Some(b) = run.end_beam {
                    let s = (if row.scale > 0.0 { row.scale } else { 1.0 }) * run.girth * ovr;
                    let (w, sc) = end_beam_draw(row, b, s, run.girth, (mt - row.start).max(0.0));
                    host.effects.transform(l.handle, w, b.rotation(v3(row.rotation)), sc);
                }
            }
            "end_glow" => {
                if let Some(b) = run.end_beam {
                    let gs = glow_share(row, mt) * run.girth * ovr;
                    host.effects.transform(l.handle, end_glow_at(row, b, gs), v3(row.rotation), Vec3::splat(gs));
                }
            }
            "beam_head" => host.effects.transform(l.handle, launch_head(staff, beam, mt, z), Vec3::ZERO, Vec3::splat(row.scale.max(0.01))),
            "end_aura" => {
                if let Some((o, k)) = aura_place(row, run, party, mt, z) {
                    host.effects.transform(l.handle, o, Vec3::ZERO, Vec3::splat(k * ovr));
                }
            }
            f if !row.path.is_empty() && f.starts_with("world") => host.effects.place(l.handle, l.frame.point(path_at(row, mt))),
            _ => {
                if (ovr - l.shot_scale).abs() > 1e-3 {
                    rescale.push((k, ovr));
                }
            }
        }
    }
    // (round 5c) a cut changed an effect's size: placed again where it is, at its size x the shot's factor
    for (k, ovr) in rescale {
        let l = run.live[k];
        let row = &DON_ZETTA_FX[l.row];
        if let Some((Attach::World(at), rot, _)) = place(row, run, party, host, aim, goal, donald, beam) {
            let base = (if row.scale > 0.0 { row.scale } else { 1.0 }) * if matches!(row.frame, "beam" | "beam_socket" | "target_socket" | "world_target" | "end_beam") { run.girth } else { 1.0 };
            host.effects.transform(l.handle, at, rot, Vec3::splat(base * ovr));
        }
        run.live[k].shot_scale = ovr;
    }
    // the ends
    let mut i = 0;
    while i < run.live.len() {
        if t + 1e-4 >= run.live[i].end {
            let l = run.live.remove(i);
            host.effects.stop(l.handle, l.cut);
            party.don_zetta.push(ZettaLog::Stop { row: DON_ZETTA_FX[l.row].id, t });
        } else {
            i += 1;
        }
    }
    let mut k = 0;
    while k < run.sounds.len() {
        if t + 1e-4 >= run.sounds[k].2 {
            let (row, h, _) = run.sounds.remove(k);
            host.audio.stop(h, DON_ZETTA_FX[row].fade.max(z.zetta_sound_fade.max(0.0)));
        } else {
            k += 1;
        }
    }
    // the release: the spell left the staff (no hit cancels it now), the hover ends (an ultimate: its caller's payload
    // launches now, by its own shot)
    if !run.released && t + 1e-4 >= release {
        run.released = true;
        let ult = run.kind.is_ult();
        if let Some(p) = party.member_mut(run.caster) {
            if let Some(r) = p.don_cast.run.as_mut().filter(|r| !ult && r.spell == run.spell) {
                r.shot = true;
            }
            if p.cmc.gravity_off {
                p.cmc.gravity_off = false;
                if !p.on_ground() {
                    p.cmc.set_falling();
                }
            }
            if !ult {
                p.don_cast.last = Some((run.spell, 0.0));
            }
        }
        party.don_zetta.push(ZettaLog::Released { t });
    }
    // the hit: the beam reaches the target (DON-MAGIC's Y Arcane hit rule: no element, its hit effect, a Blow, no
    // resistance); Zettaflare only (an ultimate's payload is the caller's)
    if run.kind == RunKind::Zetta && !run.hit && t + 1e-4 >= hit {
        run.hit = true;
        run.hit_target = run.held.or(match aim {
            ZettaAim::Target(tg) => Some(tg),
            ZettaAim::Point(_) => None,
        });
        // (round 3) the target stays held through the beam (the user); the beam stays where it hit
        run.beam_at_hit = Some(beam);
        if let Ok(mut m) = BEAM.lock() {
            *m = Some((beam.origin, beam.x, run.girth)); // (round 5c) for a host's finale look
        }
        if let Ok(mut m) = IMPACT.lock() {
            *m = Some(beam.origin); // (round 5c) for a host's debug overlay
        }
        let d = data();
        if let Some(row) = run.spell.row(&d).copied() {
            let abilities = party.status(run.caster).and_then(|s| s.donald.as_ref()).map(|st| st.abilities);
            let mut spec = HitSpec::of(run.spell, &row);
            // (2026-10-23, the user: "make sure Zettaflare scales with your magic stat", "make level 99 Zettaflare do 20,000
            // damage") its power x his Magic only (y.zettaflare.1's power: 20,000 Elden Ring HP at level 99), no boost on top
            let _ = abilities;
            spec.mult = 1.0;
            spec.attr = Attr::Notype;
            spec.hit_fx = fx::HIT_ARCANE;
            spec.ignore_resist = true;
            // (round 3) UNSTOPPABLE (the user: "Zettaflare ignores all armor, it takes priority over ANYTHING"): defence
            // ignored, the hosts force it past armour, poise, guard and their limits
            spec.def_ignore = 1.0;
            spec.force = true;
            // (round 3) the beam's hit: the target, held, staggers where it stands (its reaction comes as the beam ends);
            // THE BLAST (the user: "Enemies caught in the blast of Zettaflare should also take damage"): every enemy
            // within `zetta.blast_radius` of the impact or in the beam's path takes it too (x `zetta.splash_share`), its
            // own reaction and statuses as any hit; allies are never hit (foes only)
            // (round 5c, the user: "otherwise hold them in place for the cutscene") no push at the hit: held where it stands
            spec.push = 0.0;
            let spec = spec.kind(Kind::Small);
            let point = goal;
            let a = match aim {
                ZettaAim::Target(tg) => Aim::Target(tg, point),
                ZettaAim::Point(p) => Aim::Point(p),
            };
            party.don_fx.add(run.caster, run.spell, ActorKind::Delayed { aim: a, delay: 0.0, burst: Burst::new(spec.clone(), 0.0) });
            let size = match aim {
                ZettaAim::Target(tg) => host.targets.info(tg).map_or(1.0, |i| i.half_height.max(10.0) / z.zetta_target_center_height.max(1.0)),
                ZettaAim::Point(_) => 1.0,
            };
            let reach = z.zetta_blast_radius.max(0.0) * size.clamp(0.5, 4.0);
            let mut splash = spec.clone();
            splash.mult *= z.zetta_splash_share.max(0.0);
            let mut caught = Vec::new();
            for f in magic_fx::foes(host) {
                if matches!(aim, ZettaAim::Target(tg) if tg == f.id) || !f.flags.has(TargetFlags::ENEMY) {
                    continue;
                }
                let blast = (f.center - point).length() - f.radius <= reach;
                // (2026-10-23) the beam's path is the sweep's (its head reaching each: [`beam_sweep`])
                if blast && !run.swept.contains(&f.id) {
                    party.don_fx.add(run.caster, run.spell, ActorKind::Delayed { aim: Aim::Target(f.id, f.center), delay: 0.0, burst: Burst::new(splash.clone(), 0.0) });
                    caught.push(f.id);
                }
            }
            party.don_zetta.push(ZettaLog::Hit { aim, at: point, t });
            if !caught.is_empty() {
                party.don_zetta.push(ZettaLog::Blast { caught, t });
            }
        }
    }
    // (2026-10-23) THE BEAM SWEEPS ITS PATH: every enemy the head reaches, pushed away with the beam's full damage
    if run.kind == RunKind::Zetta && t + 1e-4 >= release {
        beam_sweep(run, party, host, z, staff, beam, matinee_time(t / run.kind.scale(z), z, channel), aim, t);
    }
    // (round 4) THE FINALE (the user: "the enemy to get immersed in the white light, with a black silhouette that crumbles
    // leaving only the white light", the silhouette only when Zettaflare kills it): every target is engulfed in white from
    // `zetta.finale_immerse_at`; a target the beam killed turns to a black silhouette and crumbles away from
    // `zetta.finale_crumble_at` (no throw); its death and rewards stay the game's
    if run.kind == RunKind::Zetta && run.hit {
        if let Some(tg) = run.hit_target {
            if !run.killed && host.targets.info(tg).map_or(true, |i| !i.flags.has(TargetFlags::ALIVE) || i.hp <= 0.0) {
                run.killed = true;
                party.don_zetta.push(ZettaLog::Killed { target: tg, t });
            }
            let mt = matinee_time(t, z, channel);
            if mt + 1e-4 >= z.zetta_hit_at {
                // (round 5) with the ending the white leaves the target with the beam (it shrinks to nothing over
                // `zetta.finale_fade_time` from `zetta.finale_fade_at`): a survivor is itself again, a killed one is gone
                let fade = if ending { ((mt - z.zetta_finale_fade_at) / z.zetta_finale_fade_time.max(1e-3)).clamp(0.0, 1.0) } else { 0.0 };
                // (round 5) a survivor comes out of the light as it is thrown (the throw reads in the target's shot)
                let fade = match run.thrown_at {
                    Some(t0) if !run.killed => fade.max(((t - t0) / z.zetta_throw_unveil.max(1e-3)).clamp(0.0, 1.0)),
                    _ => fade,
                };
                // (round 5c) blended into the beam from the hit: its light on the target (`zetta.finale_beam_glow`), then the white
                let glow = z.zetta_finale_beam_glow.clamp(0.0, 1.0) * ((mt - z.zetta_hit_at) / 0.3).clamp(0.0, 1.0);
                let white = ((mt - z.zetta_finale_immerse_at) / z.zetta_finale_immerse_time.max(1e-3)).clamp(0.0, 1.0);
                let immerse = white.max(glow) * (1.0 - fade);
                let crumble = if run.killed { ((mt - z.zetta_finale_crumble_at) / z.zetta_finale_crumble_time.max(1e-3)).clamp(0.0, 1.0) } else { 0.0 };
                let push = run.beam_at_hit.map_or(Vec3::ZERO, |b| b.x); // (round 5c) the fragments scatter on the beam's push
                // (round 5c) the silhouette darkens out of the white over `zetta.finale_dark_time` (eased), never in a step
                let dark = if run.killed { let k = ((mt - z.zetta_finale_immerse_at) / z.zetta_finale_dark_time.max(1e-3)).clamp(0.0, 1.0); k * k * (3.0 - 2.0 * k) } else { 0.0 };
                host.targets.script_finale(tg, Some(kh_core::host::ScriptFinale { immerse, silhouette: run.killed && mt + 1e-4 >= z.zetta_finale_immerse_at, crumble, push, dark }));
                run.finale = Some(tg);
                // (2026-10-23) the enemies the sweep killed: the target's own kill look (white, silhouette, cracks, break-apart)
                for k in 0..run.swept.len() {
                    let id = run.swept[k];
                    if !run.swept_killed.contains(&id) && host.targets.info(id).map_or(false, |i| !i.flags.has(TargetFlags::ALIVE) || i.hp <= 0.0) {
                        run.swept_killed.push(id);
                    }
                }
                let crumble_k = ((mt - z.zetta_finale_crumble_at) / z.zetta_finale_crumble_time.max(1e-3)).clamp(0.0, 1.0);
                let dark_k = { let k = ((mt - z.zetta_finale_immerse_at) / z.zetta_finale_dark_time.max(1e-3)).clamp(0.0, 1.0); k * k * (3.0 - 2.0 * k) };
                for &id in &run.swept_killed {
                    host.targets.script_finale(id, Some(kh_core::host::ScriptFinale { immerse: white.max(glow), silhouette: mt + 1e-4 >= z.zetta_finale_immerse_at, crumble: crumble_k, push, dark: dark_k }));
                }
            }
        }
    }
    // (round 3) THE BEAM'S END: the target is given back with its reaction (Terra-Xehanort blown back in the beam: a
    // Blow along it, `zetta.react_push`, `zetta.react_kind`), a light hit carrying it (the damage was the beam's)
    if run.kind == RunKind::Zetta && run.hit && run.held.is_some() && run.thrown_at.is_none() && t + 1e-4 >= hold_end {
        let tg = if run.killed { None } else { run.held }; // a killed target is not thrown: it crumbles
        release_hold(run, host);
        run.thrown_at = Some(t);
        let d = data();
        if let (Some(tg), Some(row)) = (tg, run.spell.row(&d).copied()) {
            let mut spec = HitSpec::of(run.spell, &row);
            spec.mult = 0.01;
            spec.attr = Attr::Notype;
            spec.hit_fx = fx::HIT_ARCANE;
            spec.ignore_resist = true;
            spec.def_ignore = 1.0;
            spec.force = true;
            spec.push = z.zetta_react_push.max(0.0);
            // (2026-10-23, the user: "thrown BACK towards the direction of the beam, not to the side") carried to a point
            // straight down the beam's line from where it stands (its direction the beam's, no side component)
            if let (Some(b), Some(i)) = (run.beam_at_hit, host.targets.info(tg)) {
                spec.toward = Some(i.position + crate::pawn::horizontal(b.x).normalize_or(Vec3::X) * spec.push.max(1.0));
            }
            let spec = spec.kind(if z.zetta_react_kind.round() as i32 == 1 { Kind::Small } else { Kind::Blow });
            let at = host.targets.info(tg).map_or(goal, |i| i.center);
            party.don_fx.add(run.caster, run.spell, ActorKind::Delayed { aim: Aim::Target(tg, at), delay: 0.0, burst: Burst::new(spec, 0.0) });
            party.don_zetta.push(ZettaLog::Reaction { target: tg, t });
        }
    }
    if t + 1e-4 >= end {
        // whatever is still alive stops now (an effect with no end of its own)
        spikes_stop(run, party, host, true); // (round 5c)
        for l in run.live.drain(..) {
            host.effects.stop(l.handle, l.cut);
        }
        for (row, h, _) in run.sounds.drain(..) {
            host.audio.stop(h, DON_ZETTA_FX[row].fade.max(z.zetta_sound_fade.max(0.0)));
        }
        if run.cinematic {
            party.don_zetta.cam.end(run.caster, run.begun, false);
        }
        release_hold(run, host);
        end_finale(run, host);
        // (round 5) the hand-off: he is free, back to his idle
        end_ending(run, party);
        // (2026-10-23) ... by way of the cutscene's collapse ([`ZettaDown`])
        if run.kind == RunKind::Zetta && !run.handed {
            if let Some(p) = party.member_mut(run.caster) {
                down_begin(p, z);
            }
        }
        end_look(run, z.zetta_cam_blend_out);
        party.don_zetta.push(ZettaLog::Ended { t });
        run.done = true;
    }
}

/// (round 5) The ending lets him go (the hand-off, an early hand-back, a stop): no longer rooted, his idle again if his
/// ending pose still plays.
fn end_ending(run: &mut ZettaRun, party: &mut Party) {
    if !run.ending {
        return;
    }
    run.ending = false;
    if let Some(p) = party.member_mut(run.caster) {
        p.don_zetta.ending = false;
        p.don_zetta.anchor = None;
        // his idle again unless a reaction plays (the ending's pose, or the cast clip's last pose he was held in)
        let posed = p.anim.clip.sequence == END_CLIP.sequence || (p.anim.clip.sequence == CAST_CLIP.sequence && p.don_cast.run.is_none());
        if posed && !matches!(p.state, NpcStateId::Damage | NpcStateId::Reflect | NpcStateId::BadStatStun | NpcStateId::BadStatBurn | NpcStateId::BadStatFreeze | NpcStateId::BadStatSneeze | NpcStateId::Die) {
            p.to_idle();
        }
    }
}

/// The run's two frames now: Donald's (his feet, the line to the target + the event's 1.98 deg) and the target's (its
/// centre, X along Donald -> it).
fn frames(party: &Party, run: &ZettaRun, goal: Vec3) -> (Frame, Frame) {
    let Some(p) = party.member(run.caster) else { return (Frame { origin: goal, yaw: 0.0 }, Frame { origin: goal, yaw: 0.0 }) };
    let toward = crate::pawn::horizontal(goal - p.position());
    let toward_yaw = if toward.length() > 1.0 { crate::pawn::yaw_of(toward) } else { p.yaw };
    (Frame { origin: p.position(), yaw: crate::pawn::wrap180(toward_yaw + EVENT_DONALD_YAW_OFF) }, Frame { origin: goal, yaw: toward_yaw })
}

/// The staff's release point in Donald's frame (the release flash `fx.world.91_zfr_00` at the release): the beam's start.
pub fn staff_local(z: &ZettaTune) -> Vec3 {
    staff_at(z, z.zetta_release_at)
}

/// (round 5) THE WAND'S TIP at a matinee time, in Donald's frame (the user: "it looks like Zettaflare comes from the tip of
/// the wand"): the event's own release flash `fx.world.91_zfr_00` (magic1, placed by the event ON the tip of his staff, its
/// move track following the tip as he swings it down: 72.033 .. 72.2), held before its start and after its last key. The
/// beam starts here, so the flash, the tip and the beam's start are one point through the cast clip.
pub fn staff_at(z: &ZettaTune, mt: f32) -> Vec3 {
    DON_ZETTA_FX.iter().find(|r| r.id == "fx.world.91_zfr_00").map_or(Vec3::new(120.0, 0.0, 120.0), |r| path_at(r, mt.max(z.zetta_release_at)))
}

/// (round 5) THE FINAL CLOUD OVER THE IMPACT (the user: "the final cloud effect from the final shot of Zettaflare also needs to
/// line up with where Zettaflare was"): a row of frame `end_impact` is centred above the real impact (the aim point where the
/// beam hit), on Donald's line to it, at the height and size that keep the event's look from the shot behind him: seen from that
/// shot's camera (`offset.x` behind his feet, `offset.z` up) at the event's elevation (`offset.y` deg), scaled by its distance
/// from the camera over the event's (`path[0]`). Returns (place, scale).
/// (round 5c) The last Zettaflare beam's impact (game space), for a host's debug overlay (the test level: `KH_ZETTA_MARK=1`
/// draws it on screen, the proof that the ending's clouds have their clear centre on it).
static IMPACT: std::sync::Mutex<Option<Vec3>> = std::sync::Mutex::new(None);

/// (round 5c) The last Zettaflare beam at its hit (game space): its point on the target, its direction (unit), its girth.
/// A host's finale look reads it (the test level: the silhouette inside the beam, its white-hot core across it).
static BEAM: std::sync::Mutex<Option<(Vec3, Vec3, f32)>> = std::sync::Mutex::new(None);

/// (round 5c) The last Zettaflare's ground track under the beam (from under his wand to the impact's foot), for a host's
/// debug overlay (`KH_ZETTA_MARK=1`: the proof that the spikes run its whole length).
static TRACK: std::sync::Mutex<Option<(Vec3, Vec3)>> = std::sync::Mutex::new(None);

/// (round 5c) See [`TRACK`].
pub fn debug_track() -> Option<(Vec3, Vec3)> {
    TRACK.lock().ok().and_then(|m| *m)
}

/// (round 5c) See [`BEAM`].
pub fn last_beam() -> Option<(Vec3, Vec3, f32)> {
    BEAM.lock().ok().and_then(|m| *m)
}

/// (round 5c) See [`IMPACT`].
pub fn debug_impact() -> Option<Vec3> {
    IMPACT.lock().ok().and_then(|m| *m)
}

pub fn end_impact_place(row: &DonZettaFxRow, run: &ZettaRun) -> Option<(Vec3, f32, Vec3)> {
    // (round 5c, the user: "the clouds at the end are STILL not repositioned properly, the clear centre of them needs to be where
    // the beam was") the cloud is two flat rings (jmd_rng_g64x3_01z: an annulus, its hole on the effect's local Z axis,
    // local z +150 and -350): its clear centre is that axis. It is laid ACROSS the line from the shot behind Donald (970's
    // first key: `offset` = its distance behind him and height) through the real impact, at the event's distance from that
    // camera (`path[0]`), its axis on that line: from that camera the hole sits exactly on the impact. Returns (place, scale,
    // rotation).
    let fr = run.end_frame?;
    let impact = run.beam_at_hit.map(|b| b.origin)?;
    let o = v3(row.offset);
    let cam = fr.point(Vec3::new(-o.x, 0.0, o.z));
    let dir = (impact - cam).normalize_or(fr.vector(Vec3::X));
    let reference = row.path.first().copied().unwrap_or(10000.0).max(1.0);
    let at = cam + dir * reference.max((impact - cam).length() + 600.0);
    let side = Vec3::Z.cross(dir).normalize_or(Vec3::Y);
    let rot = axes_rot(side, dir.cross(side), dir);
    Some((at, 1.0, rot))
}

/// Ease out (cubic): fast at first, settling.
fn ease_out(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x) * (1.0 - x) * (1.0 - x)
}

/// The horizontal-and-vertical distance of `p` to the segment `a` .. `b`.
fn seg_dist(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 1e-6 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t - p).length()
}

/// The finale's look is over (`Targets::script_finale(None)`).
fn end_finale(run: &mut ZettaRun, host: &mut PartyHost) {
    if let Some(tg) = run.finale.take() {
        host.targets.script_finale(tg, None);
    }
    for id in std::mem::take(&mut run.swept_killed) {
        host.targets.script_finale(id, None);
    }
}

/// (2026-10-23, the user: "any enemies that in the beam get pushed away with immense damage") THE BEAM'S SWEEP: from the
/// release, every enemy the beam's head has reached (its drawn length along the line from the wand: the launch, then on past
/// the target, `zetta.beam_beyond`) within the beam's width (`zetta.sweep_radius` x its girth, from its axis to the body's
/// surface) is hit once: the beam's own damage (its power x Donald's Magic, x `zetta.splash_share`: 1, the whole), forced
/// past armour as the target's, a Blow that carries it away (`zetta.sweep_push`, scaled with its size as the target's
/// throw): along the beam, away from Donald, and out off its axis (`zetta.sweep_out` of it), landing on the ground (the
/// hosts' Blow: their ground and collision). Never the cast's target (its own hit), never an ally; it never takes the camera.
#[allow(clippy::too_many_arguments)]
fn beam_sweep(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, z: &ZettaTune, staff: Vec3, beam: Basis, mt: f32, aim: ZettaAim, t: f32) {
    let dist = (beam.origin - staff).length().max(1.0);
    let reach = launch_reach(mt, z);
    let len = dist * reach + z.zetta_beam_beyond.max(0.0) * reach.powi(4);
    let width = z.zetta_sweep_radius.max(0.0) * run.girth.max(1.0);
    let d = data();
    let Some(row) = run.spell.row(&d).copied() else { return };
    let target = match aim {
        ZettaAim::Target(tg) => Some(tg),
        ZettaAim::Point(_) => None,
    };
    for f in magic_fx::foes(host) {
        if Some(f.id) == target || run.hit_target == Some(f.id) || run.swept.contains(&f.id) || !f.flags.has(TargetFlags::ENEMY) || !f.flags.has(TargetFlags::ALIVE) {
            continue;
        }
        let rel = f.center - staff;
        let along = rel.dot(beam.x);
        let off = rel - beam.x * along;
        if along < 0.0 || along - f.radius > len || off.length() - f.radius > width {
            continue;
        }
        run.swept.push(f.id);
        let mut spec = HitSpec::of(run.spell, &row);
        spec.mult = z.zetta_splash_share.max(0.0);
        spec.attr = Attr::Notype;
        spec.hit_fx = fx::HIT_ARCANE;
        spec.ignore_resist = true;
        spec.def_ignore = 1.0;
        spec.force = true;
        let size = (f.half_height.max(10.0) / z.zetta_target_center_height.max(1.0)).clamp(0.5, 4.0);
        spec.push = z.zetta_sweep_push.max(0.0) * size;
        let flat_x = crate::pawn::horizontal(beam.x).normalize_or(Vec3::X);
        let out = crate::pawn::horizontal(off).normalize_or(Vec3::Z.cross(flat_x).normalize_or(Vec3::Y));
        spec.toward = Some(f.position + (flat_x + out * z.zetta_sweep_out.max(0.0)).normalize_or(flat_x) * spec.push.max(1.0));
        let spec = spec.kind(Kind::Blow);
        party.don_fx.add(run.caster, run.spell, ActorKind::Delayed { aim: Aim::Target(f.id, f.center), delay: 0.0, burst: Burst::new(spec, 0.0) });
        party.don_zetta.push(ZettaLog::Blast { caught: vec![f.id], t });
    }
}

/// The target held by a run is given back (`Targets::script_hold(None)`).
fn release_hold(run: &mut ZettaRun, host: &mut PartyHost) {
    if let Some(tg) = run.held.take() {
        host.targets.script_hold(tg, None);
    }
}

/// (2026-10-23) The target's feet (height): the beam's aim (its centre at the hit) less its half height as last measured.
fn feet_z_of(run: &ZettaRun, beam: Basis) -> f32 {
    beam.origin.z - run.subject_last.map_or(90.0, |l| if l.2 > 0.0 { l.2 } else { 90.0 })
}

/// (2026-10-23) The cinematic camera shows a target shot now (its last pose's shot frames the subject).
fn subject_shot_now(party: &Party, _z: &ZettaTune) -> bool {
    let c = &party.don_zetta.cam;
    c.last.is_some_and(|p| SUBJECT_SHOTS.contains(&p.shot)) && c.active()
}

/// (2026-10-23, the user: "the shot zooming away from Donald as he launches Zettaflare should NOT change regardless of the
/// size of the enemy so Donald is always framed perfectly, use Zettaflare on Sora as a baseline") DONALD'S SHOTS (frame
/// `donald`: kg702's 860, 890, 910): framed as against the Sora target at any size, the beam in them as wide as there.
pub const DONALD_SHOTS: [&str; 3] = ["shot860", "shot890", "shot910"];

/// (2026-10-23) The beam's girth as the shot on screen draws it: the Sora case's in Donald's shots ([`DONALD_SHOTS`]),
/// the run's elsewhere.
fn shot_girth(party: &Party, girth: f32) -> f32 {
    let c = &party.don_zetta.cam;
    if c.active() && c.last.is_some_and(|p| DONALD_SHOTS.contains(&p.shot)) {
        1.0f32.min(girth.max(1.0))
    } else {
        girth
    }
}

/// (2026-10-23) The event's target shots (frame `subject`: kg702's 920, 930, 940).
pub const SUBJECT_SHOTS: [&str; 3] = ["shot920", "shot930", "shot940"];

/// THE BEAM ITSELF (rows `beam_stream`): a beam card laid along the line FROM THE WAND'S TIP, rolled by the row's roll, its
/// centre on the line, stretched along its length. (round 5, the user: "Zettaflare's beam just suddenly pops in", "it looks
/// like Zettaflare comes from the tip of the wand") it LAUNCHES: from the release it grows out of the tip (ease out) and
/// reaches the target at the hit, then runs on `zetta.beam_beyond` past it; each card builds from `zetta.beam_width0` of its
/// width to all of it over `zetta.beam_card_in` from its own start and thins to nothing over `zetta.beam_card_out` before its
/// end (no pop in, no pop out). (round 5c, the user: "See the bits of light coming towards the camera ... Thats the extra
/// beam I'm talking about that needs to be removed") the effect's own beam axis, local +Z (its glow cards and, since
/// tools/zetta_fx_fix.js, its core cards: all start at local z -450 and run forward), is laid ON the line from the tip, so
/// no card can start behind the tip or run off at an angle (crates/kh-host/tests/zetta_beam_lines.rs); the row's roll turns
/// the effect about the line. Row: `offset` = the beam's start in the effect's own axes, `path[0]` = half its cards'
/// length (cm).
/// Returns the emitter's (world place, rotation, scale).
pub fn stream_place(row: &DonZettaFxRow, staff: Vec3, beam: Basis, mt: f32, z: &ZettaTune) -> (Vec3, Vec3, Vec3) {
    stream_place_girth(row, staff, beam, mt, z, 1.0)
}

/// (round 5) [`stream_place`] for a beam `girth` times the event's width (the user: "The Zettaflare beam should encompass the
/// entire enemy, it adapts to the enemy's size": [`ZettaRun::girth`]).
pub fn stream_place_girth(row: &DonZettaFxRow, staff: Vec3, beam: Basis, mt: f32, z: &ZettaTune, girth: f32) -> (Vec3, Vec3, Vec3) {
    stream_place_facing(row, staff, beam, mt, z, girth, None)
}

/// (round 5c) THE BEAM TURNED TO THE CAMERA (the user: "the edges of Zettaflare's beam look bare, naked and terrible. It MUST look
/// EXACTLY like the cutscene"): every card of the beam is flat (its face across its local X); the event's cards were turned
/// to each of its own cameras, ours meet any camera, so seen from the side of their plane they read as bare lines. With
/// the camera at `eye`, each card is turned about the beam's line to face it: the row's first copy squarely, its crossed
/// copy (the row's roll 90) at 60 deg to it, so both read as a full glowing band and never as a line. `None`: the rows'
/// own rolls about the beam's horizontal side ([`stream_place_girth`]).
#[allow(clippy::too_many_arguments)]
pub fn stream_place_facing(row: &DonZettaFxRow, staff: Vec3, beam: Basis, mt: f32, z: &ZettaTune, girth: f32, eye: Option<Vec3>) -> (Vec3, Vec3, Vec3) {
    stream_place_beyond(row, staff, beam, mt, z, girth, eye, z.zetta_beam_beyond.max(0.0))
}

/// (2026-10-23) [`stream_place_facing`] with the beam running `beyond` past the aim point.
#[allow(clippy::too_many_arguments)]
pub fn stream_place_beyond(row: &DonZettaFxRow, staff: Vec3, beam: Basis, mt: f32, z: &ZettaTune, girth: f32, eye: Option<Vec3>, beyond: f32) -> (Vec3, Vec3, Vec3) {
    let dist = (beam.origin - staff).length().max(1.0);
    let full = dist + beyond.max(0.0);
    // (round 5c) fired: the beam reaches as far as its head (`launch_reach`), its part past the aim point last
    let reach = launch_reach(mt, z);
    let len = (dist * reach + (full - dist) * reach.powi(4)).max(1.0);
    let half = row.path.first().copied().unwrap_or(2000.0).max(1.0);
    let sz = len / (2.0 * half);
    let w0 = z.zetta_beam_width0.clamp(0.0, 1.0);
    // (round 5c) the launch's cards swell behind the head (its travel and then the card's own build)
    let card_in = if (row.start - z.zetta_release_at).abs() < 1e-3 { z.zetta_launch_travel + z.zetta_beam_card_in } else { z.zetta_beam_card_in };
    let built = ease_out((mt - row.start) / card_in.max(1e-3));
    let left = if row.end > 0.0 { ease_out((row.end - mt) / z.zetta_beam_card_out.max(1e-3)) } else { 1.0 };
    let w = ((w0 + (1.0 - w0) * built) * left * girth.max(0.1)).max(1e-3);
    // the effect's axes: Z along the beam, X turned by the row's roll about it from the beam's horizontal side (or from the
    // camera's side of the line)
    let roll = match eye {
        Some(e) => {
            let d = e - (staff + beam.x * (len * 0.5));
            let toward = d.dot(beam.z).atan2(d.dot(beam.y)).to_degrees();
            toward + v3(row.rotation).z * (60.0 / 90.0)
        }
        None => v3(row.rotation).z,
    };
    let (s, c) = roll.to_radians().sin_cos();
    let az = beam.x;
    let ax = (beam.y * c + beam.z * s).normalize_or_zero();
    let ay = az.cross(ax);
    let rot = axes_rot(ax, ay, az);
    let o = v3(row.offset);
    // (2026-10-23, the user: "the beam must NEVER extend behind Donald") a wider beam starts no further back: what its
    // cards' width adds behind the tip (the event's c94 strand leans: [`STREAM_WIDTH_BACK`] per unit of width) is taken forward
    // (2026-10-23, the user: "the beam MUST start at Donald's wand") each card's start put ON the tip at any width
    // ([`stream_start`]: where the event's card begins, and how its lean moves that as it widens)
    let (start, lean) = stream_start(row.asset);
    let origin = staff - (ax * (o.x * w) + ay * (o.y * w) + az * (o.z * sz)) + az * (-start + lean * (w - 1.0).max(0.0));
    (origin, rot, Vec3::new(w, w, sz))
}

/// (2026-10-23, the coordinator's review of the golden-frame proof: "In the reference the beam fills the ENTIRE frame";
/// "Sora is a pale grey figure blended INTO the light, with the beam's streaks visible through and over him") THE BEAM AS
/// THE TARGET'S SHOTS DRAW IT, from the hit while a target shot plays: the shot is the reference's scaled about the body, so
/// the beam is too: it runs back past the staff and on past the target as far as the frame reaches (`zetta.beam_fill_back`
/// x the camera's distance, each way, never shorter than the beam itself), and its cards lie on the camera's side of the
/// body (its drawn radius toward the camera), so the streaks cross over it as the event's do over Sora. Other shots: the
/// beam as it is (from the wand's tip). Returns (the start drawn, the beam's frame drawn, how far it runs past the aim).
/// (2026-10-23) THE CARDS BROUGHT TO THE CAMERA in the target's shots: a card scaled about the eye draws the same picture, so
/// each of the beam's cards is drawn nearer, by the factor returned, until the floor (`feet_z`, under the lowest edge of the
/// view: `cam`'s pitch and lens) can no longer show in front of it (`zetta.beam_card_floor` of the way to where that
/// edge meets it): the beam fills the frame to its bottom edge on the ground, and its streaks cross over the body. 1: as
/// laid (no camera, another shot, the floor out of view).
pub fn card_pull(eye: Vec3, rotation: Vec3, fov: f32, card: Vec3, feet_z: f32, z: &ZettaTune) -> f32 {
    if z.zetta_beam_card_floor <= 0.0 {
        return 1.0; // (off: the reference's cards on the line)
    }
    let below = vfov_of(fov) * 0.5 - rotation.x;
    let high = (eye.z - feet_z).max(1.0);
    let d = (card - eye).length().max(1.0);
    if below <= 0.5 {
        return 1.0;
    }
    let ground = high / below.to_radians().tan();
    (z.zetta_beam_card_floor.clamp(0.05, 1.0) * ground / d).clamp((25.0 / d).min(1.0), 1.0)
}

/// (2026-10-23) [`stream_place_beyond`] in the target's shots: the beam as [`beam_drawn`] lays it, its card brought to the
/// camera ([`card_pull`]); elsewhere as laid.
#[allow(clippy::too_many_arguments)]
pub fn stream_drawn(row: &DonZettaFxRow, staff: Vec3, beam: Basis, mt: f32, z: &ZettaTune, girth: f32, cam: Option<CamPose>, subject_shot: bool, body_r: f32, feet_z: f32) -> (Vec3, Vec3, Vec3) {
    let eye = cam.map(|c| c.position);
    let (st, bm, by) = beam_drawn(staff, beam, eye, subject_shot, body_r, z);
    // (2026-10-23, the coordinator: "the beam fills the ENTIRE frame") in the target's shots its cards are
    // `zetta.beam_fill_width` times as wide: the band covers the frame top to bottom (and the floor in front of it)
    let girth = if subject_shot && cam.is_some() { girth * z.zetta_beam_fill_width.max(1.0) } else { girth };
    let (o, r, sc) = stream_place_beyond(row, st, bm, mt, z, girth, eye, by);
    // (only the first copy of each of the beam's own streams, the ones turned square to the camera: one layer brought forward
    // over the floor and across the body, the rest on the line through it as the reference shows it)
    match cam.filter(|_| subject_shot && row.id.starts_with("fx.beam.stream_") && v3(row.rotation).z.abs() < 45.0) {
        Some(c) => {
            // the card's middle where it crosses the view: the line's point nearest the eye
            let near = bm.origin + bm.x * (c.position - bm.origin).dot(bm.x);
            let k = card_pull(c.position, c.rotation, c.fov, near, feet_z, z);
            (c.position + (o - c.position) * k, r, sc * k)
        }
        None => (o, r, sc),
    }
}

pub fn beam_drawn(staff: Vec3, beam: Basis, eye: Option<Vec3>, subject_shot: bool, body_r: f32, z: &ZettaTune) -> (Vec3, Basis, f32) {
    let beyond = z.zetta_beam_beyond.max(0.0);
    let Some(e) = eye else { return (staff, beam, beyond) };
    if !subject_shot {
        // (2026-10-23, the user: "NEVER show the end of it") every shot: its far end as far past the target as the frame reaches
        return (staff, beam, beyond.max((e - beam.origin).length() * z.zetta_beam_fill_back.max(0.0)));
    }
    let reach = (e - beam.origin).length() * z.zetta_beam_fill_back.max(0.0);
    // (2026-10-23, the user: "the beam must NEVER extend behind Donald during the zoomed out shot") it starts at the wand's
    // tip in every shot: only its far end runs on past the target to fill the frame
    let start = staff;
    // toward the camera, square to the line: the body's front
    let rel = e - beam.origin;
    let side = rel - beam.x * rel.dot(beam.x);
    // (the cards stay on the line, through the body, as in the reference: half of it in front of them, half behind)
    let _ = body_r;
    let lift = side * 0.0;
    let mut b = beam;
    b.origin += lift;
    (start + lift, b, beyond.max(reach))
}

/// (2026-10-23, the user: "the beam must NEVER extend behind Donald during the zoomed out shot"; the coordinator: "anchor the
/// beam's back end at the start point (the wand tip); scale its length and width forward only") HOW FAR BEHIND ITS PLACE an
/// ending effect reaches at its own size (cm, over its whole life; measured with the packed effects by kh-host
/// tests/zetta_beam_lines.rs, which fails if one reaches further): grown `s` times, it is moved forward by as much x `s`, so
/// its back end stays at the tip.
pub const END_BACK: [(&str, f32); 3] = [("vd_kg702b_96_rit_00", 0.0), ("vd_kg702b_96_zfr_00", 0.0), ("vd_zetta_src_glow", 0.0)];

/// (2026-10-23, the user: "the beam MUST start at Donald's wand, and NEVER show the end of it") WHERE EACH BEAM CARD STARTS
/// along the line from the tip at its own width (cm; ahead +), and how much further back it starts per unit of width past 1
/// (its lean): the cards start where the event's do (its own cards begin 0.6 .. 1.1 m ahead of the tip, in the tip's flash)
/// and no further ahead at any size; a leaning one is held at or ahead of Donald (kh-host tests/zetta_beam_lines.rs).
/// (0, 0): not listed.
pub fn stream_start(asset: &str) -> (f32, f32) {
    match asset {
        // (the event's c94 strand leans back past the tip as it widens: held at or ahead of Donald)
        "vd_kg702_94_zfr_01" => (0.0, 13.0),
        _ => (0.0, 0.0),
    }
}

/// (2026-10-23) How far behind the tip a beam card's width reaches per unit of girth past 1 (cm; kh-host
/// tests/zetta_beam_lines.rs measures the event's c94 strand: 87 cm behind at 1, 167 at 10 unmoved): the cards are moved forward by
/// as much, so a wider beam starts where the event's does.
pub const STREAM_WIDTH_BACK: f32 = 14.0;

/// (2026-10-23) [`END_BACK`] of an asset (0: none listed).
pub fn end_back(asset: &str) -> f32 {
    END_BACK.iter().find(|e| e.0 == asset).map_or(0.0, |e| e.1)
}

/// (2026-10-23) The ending beam row's place grown `s` times (its row scale x the girth): where the event puts it (its cards
/// start at the tip), grown across the line only ([`end_beam_scale`]), moved forward by what of it reaches behind
/// ([`END_BACK`]): the back end at the tip at any size.
pub fn end_beam_at(row: &DonZettaFxRow, b: Basis, s: f32) -> Vec3 {
    b.point(v3(row.offset)) + b.x * (end_back(row.asset) * s)
}

/// (2026-10-23, the coordinator: "scale its length and width forward only") THE ENDING BEAM GROWN `s` TIMES: its width
/// only (the effect's local X and Y, across the line); its length (local Z, along the line) as authored. A uniform scale
/// grows each card about its own middle (the evaluator scales a particle's size, not its place in the effect), so the
/// 100 m gradient cards would reach back past Donald by half their length.
pub fn end_beam_scale(s: f32) -> Vec3 {
    Vec3::new(s, s, 1.0)
}

/// (2026-10-23) THE ENDING BEAM'S LONGEST CARD at `t` s into its life (cm, at its own size; kh-host
/// tests/zetta_beam_lines.rs measures the packed cards and fails if this is ever shorter): (asset, [(t, length)]),
/// linear between the samples, the last held.
pub const END_CARD_LEN: [(&str, &[(f32, f32)]); 1] = [("vd_kg702b_96_zfr_00", &[(0.0, 9450.0), (0.5, 9580.0), (1.0, 9830.0), (1.5, 10280.0), (2.0, 10760.0), (2.5, 11270.0), (3.0, 11720.0), (3.5, 12160.0), (4.0, 12670.0), (4.5, 13150.0), (5.0, 13600.0), (5.5, 13870.0), (6.0, 14060.0), (7.5, 14060.0)])];

/// (2026-10-23, the user: "the end of the beam should never be visible from the long shot, it should always extend out of
/// frame") HOW MUCH LONGER the ending beam is drawn for a girth: as the wide shot pulls back (its square root, [`cam_pose`]
/// shot960), so its far end lies as far outside the frame as the event's does; never shorter than the event's.
pub fn end_len_scale(girth: f32) -> f32 {
    // (the user: "the beam MUST start at Donald's wand, and NEVER show the end of it": its length as the event's, the wide
    // shot as the event frames it, so its far end runs out of the frame as the event's does; only its width grows)
    let _ = girth;
    1.0
}

/// (2026-10-23) THE ENDING BEAM ROW'S DRAWING at `life` s into it: (place, scale) for its width `s` (across the line) and
/// its length grown [`end_len_scale`] (its cards' own axis, local Z, starting at the tip: they grow forward only; kh-host
/// tests/zetta_beam_lines.rs holds the back end at the tip and the far end past [`END_CARD_LEN`] x the factor).
pub fn end_beam_draw(row: &DonZettaFxRow, b: Basis, s: f32, girth: f32, life: f32) -> (Vec3, Vec3) {
    let _ = life;
    let k = end_len_scale(girth);
    // (its cards start END_Z_BACK behind the effect's own origin along its axis: grown k times, it is moved forward by as
    // much again, so they still start at the tip)
    (end_beam_at(row, b, s) + b.x * ((k - 1.0) * end_z_back(row.asset)), Vec3::new(s, s, k))
}

/// (2026-10-23) How far behind the ending effect's own origin its cards start, along its axis (cm, at its own size; kh-host
/// tests/zetta_beam_lines.rs holds the drawn beam's start at the tip at every size).
pub fn end_z_back(asset: &str) -> f32 {
    if asset == "vd_kg702b_96_zfr_00" { 1875.0 } else { 0.0 }
}

/// (2026-10-23) [`END_CARD_LEN`] at `t` s (0: not listed).
pub fn end_card_len(asset: &str, t: f32) -> f32 {
    let Some((_, k)) = END_CARD_LEN.iter().find(|e| e.0 == asset) else { return 0.0 };
    let i = k.iter().position(|e| e.0 > t).unwrap_or(k.len());
    match i {
        0 => k.first().map_or(0.0, |e| e.1),
        n if n >= k.len() => k.last().map_or(0.0, |e| e.1),
        n => {
            let (a, b) = (k[n - 1], k[n]);
            a.1 + (b.1 - a.1) * ((t - a.0) / (b.0 - a.0).max(1e-4))
        }
    }
}

/// (2026-10-23) The light at the source (row `end_glow`) at size `s`: on the tip, moved forward by its reach behind
/// ([`END_BACK`] x `s`): its back edge at the tip.
pub fn end_glow_at(row: &DonZettaFxRow, b: Basis, s: f32) -> Vec3 {
    b.origin + v3(row.offset) + b.x * (end_back(row.asset) * s)
}

/// (round 5c) HIS WAND'S TIP AS THE HOST DRAWS IT (game space, cm): a host that poses Donald feeds it every frame (the test
/// level: kh-app zetta_crumble.rs, his weapon's socket vfx_call); the launch starts the beam there. None: not fed (the
/// event's release track is used).
static TIP: std::sync::Mutex<Option<(u8, Vec3)>> = std::sync::Mutex::new(None);

/// (round 5c) See [`TIP`]: `member` the party member's id (Donald's), `None` stops the feed.
pub fn feed_tip(member: u8, at: Option<Vec3>) {
    if let Ok(mut m) = TIP.lock() {
        *m = at.map(|a| (member, a));
    }
}

/// (round 5c) See [`TIP`].
pub fn tip_of(member: MemberId) -> Option<Vec3> {
    TIP.lock().ok().and_then(|m| *m).filter(|(id, _)| *id == member.0).map(|(_, a)| a)
}

/// (round 5c) THE LAUNCH'S REACH at matinee time `mt`: 0 at the release, 1 when its head arrives (`zetta.launch_travel`
/// later); smooth (it leaves the tip visibly, speeds, and settles onto the target).
pub fn launch_reach(mt: f32, z: &ZettaTune) -> f32 {
    let k = ((mt - z.zetta_release_at) / z.zetta_launch_travel.max(0.05)).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// (round 5c) The beam's head at matinee time `mt`: from the tip toward the aim point (`beam.origin` before the hit).
pub fn launch_head(staff: Vec3, beam: Basis, mt: f32, z: &ZettaTune) -> Vec3 {
    staff + (beam.origin - staff) * launch_reach(mt, z)
}

/// (round 5c) THE LIGHT AT THE SOURCE'S SIZE at matinee time `mt` (row `end_glow`): the ending beam's width share, 1 at the
/// row's start to 0 at `path[0]` seconds (the beam thins to nothing: its cards 5.9 -> 0 wide), eased out.
pub fn glow_share(row: &DonZettaFxRow, mt: f32) -> f32 {
    let span = row.path.first().copied().unwrap_or(5.6).max(0.1);
    let k = ((mt - row.start) / span).clamp(0.0, 1.0);
    (1.0 - k) * (1.0 - k * 0.35)
}

/// (round 5c, the user: "Donald needs to be covered by the glow at the long shot of the beam shrinking, the aura covering him
/// slowly shrinks and fades along with the beam shrinking") THE AURA ON DONALD at matinee time `mt` (row `end_aura`): its
/// centre between his body's centre and his wand tip, `zetta.aura_front` toward the camera (in front of him: it covers him
/// and never cuts through his model), and its scale of the effect's unit (100 cm): `zetta.aura_size` x the beam's width
/// share ([`glow_share`]: it shrinks, and fades as it does, with the beam, gone with it).
pub fn aura_place(row: &DonZettaFxRow, run: &ZettaRun, party: &Party, mt: f32, z: &ZettaTune) -> Option<(Vec3, f32)> {
    let p = party.member(run.caster)?;
    let body = p.position() + Vec3::Z * 50.0;
    let mid = run.end_beam.map_or(body, |b| (body + b.origin) * 0.5);
    let toward = party.don_zetta.cam.last.map(|c| (c.position - mid).normalize_or_zero()).unwrap_or(Vec3::ZERO);
    let share = glow_share(row, mt);
    // (2026-10-23, "the beam should be bigger in the zoomed out shot as well") as many times larger as the beam is
    Some((mid + toward * z.zetta_aura_front.max(0.0) * run.girth.max(1.0), z.zetta_aura_size.max(0.0) / 100.0 * share * run.girth.max(1.0)))
}

/// Where a row's effect goes now: (attach, rotation, its frame).
#[allow(clippy::too_many_arguments)]
fn place(row: &DonZettaFxRow, run: &ZettaRun, party: &Party, host: &PartyHost, aim: ZettaAim, goal: Vec3, donald: Frame, beam: Basis) -> Option<(Attach, Vec3, Frame)> {
    let p = party.member(run.caster)?;
    let pm = PartyMember(run.caster.0);
    let off = v3(row.offset);
    let rot = v3(row.rotation);
    let socket: Option<&'static str> = (!row.socket.is_empty() && row.socket != "-" && row.socket != "None").then_some(row.socket);
    let toward = crate::pawn::horizontal(goal - p.position());
    let toward_yaw = if toward.length() > 1.0 { crate::pawn::yaw_of(toward) } else { p.yaw };
    Some(match row.frame {
        // on him / his staff: the socket's own frame (the host follows him)
        "donald_socket" | "weapon" => (Attach::Party { member: pm, bone: socket, offset: off }, rot, donald),
        // on the target, the offset in a frame facing Donald (the event's target faced him); the host adds it in world axes
        "target_socket" => {
            let frame = Frame { origin: goal, yaw: crate::pawn::wrap180(toward_yaw + 180.0) };
            match aim {
                ZettaAim::Target(tg) if host.targets.info(tg).is_some() => (Attach::Target { target: tg, offset: frame.vector(off) }, frame.rotation(rot), frame),
                _ => (Attach::World(frame.point(off)), frame.rotation(rot), frame),
            }
        }
        "world_donald" => (Attach::World(donald.point(off)), donald.rotation(rot), donald),
        // (round 3) in the beam frame; on the target in the beam frame; the beam itself (placed again at once)
        "beam" => (Attach::World(beam.point(off)), beam.rotation(rot), Frame { origin: goal, yaw: toward_yaw }),
        "beam_socket" => match aim {
            ZettaAim::Target(tg) if host.targets.info(tg).is_some() => (Attach::Target { target: tg, offset: beam.vector(off) }, beam.rotation(rot), Frame { origin: goal, yaw: toward_yaw }),
            _ => (Attach::World(beam.point(off)), beam.rotation(rot), Frame { origin: goal, yaw: toward_yaw }),
        },
        "beam_stream" => (Attach::World(beam.origin), beam.rotation(Vec3::new(0.0, 0.0, rot.z)), Frame { origin: goal, yaw: toward_yaw }),
        // (round 5) the ending: his feet at its start, X along the beam's line; the ending beam's own frame (its start at his
        // staff, X along it in 3D)
        "end" => {
            let fr = run.end_frame?;
            (Attach::World(fr.point(off)), fr.rotation(rot), fr)
        }
        "end_beam" => {
            let b = run.end_beam?;
            // (2026-10-23) its back end anchored at the wand's tip, its size grown forward only ([`end_beam_at`])
            let s = (if row.scale > 0.0 { row.scale } else { 1.0 }) * run.girth;
            (Attach::World(end_beam_at(row, b, s)), b.rotation(rot), run.end_frame.unwrap_or(donald))
        }
        // (round 5c) the beam's head (placed again at once: [`launch_head`])
        "beam_head" => (Attach::World(beam.origin), rot, Frame { origin: goal, yaw: toward_yaw }),
        // (round 5c) the aura on him ([`aura_place`])
        "end_aura" => {
            let (o, _) = aura_place(row, run, party, row.start, &zetta())?;
            (Attach::World(o), rot, run.end_frame.unwrap_or(donald))
        }
        // (round 5c) the light at the source: at the ending beam's start (his wand tip), sized every step ([`glow_share`])
        "end_glow" => {
            let b = run.end_beam?;
            (Attach::World(end_glow_at(row, b, glow_share(row, row.start) * run.girth)), rot, run.end_frame.unwrap_or(donald))
        }
        // (round 5) the final cloud centred above the real impact ([`end_impact_place`])
        "end_impact" => {
            let fr = run.end_frame?;
            let (at, _, r) = end_impact_place(row, run)?;
            let _ = rot;
            (Attach::World(at), r, fr)
        }
        // the target frame: its centre, X along Donald -> it
        "world_target" => {
            let frame = Frame { origin: goal, yaw: toward_yaw };
            (Attach::World(frame.point(off)), frame.rotation(rot), frame)
        }
        _ => return None,
    })
}

// ============================================================================================== (round 5c) the rock spikes

/// (round 5c) THE ROCK SPIKES OF A LOW ZETTAFLARE (the user: "is it possible to add huge spikes coming out from the ground if
/// the beam was close to the ground then have them crumble away before control is back for Donald?"; "only show the spikes
/// in the wide shot and the shot behind donald at the end, where they crumble and disintegrate"): one spike, its foot on
/// the ground (sunk a little), its axis leaning out from the impact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spike {
    pub foot: Vec3,
    /// Its axis (unit): the ground's normal leaned away from the impact.
    pub up: Vec3,
    pub height: f32,
    pub width: f32,
    /// Its own turn about its axis (deg): no two alike.
    pub turn: f32,
    /// Seconds after `zetta.spikes_crumble_at` it starts to crumble (they go one after another).
    pub delay: f32,
    pub handle: Option<EffectHandle>,
    /// Its crumble (rubble, grit, dust) was spawned.
    pub dust: bool,
    /// (round 5c) its rocks broke apart (the spike effect stopped, [`RockPiece`]s flying)
    pub broke: bool,
    /// (round 5c) its sounds played (bits: 1 crack, 2 rubble, 4 crumble)
    pub said: u8,
}

/// (round 5c, the user: "the rocks need to physically crumble to the ground, actually breaking apart") A SPIKE'S OWN ROCKS, as
/// make_zetta_spikes.js stacks them in its unit (100 cm tall, 100 across): (mesh b, across, tall, centre up, turn of a full
/// turn, tip of a full turn, out).
pub const SPIKE_ROCKS: [(bool, f32, f32, f32, f32, f32, f32); 5] = [(false, 104.0, 40.0, 18.0, 0.0, 0.0, 0.0), (true, 84.0, 34.0, 40.0, 0.13, 0.01, 2.0), (false, 62.0, 30.0, 60.0, 0.31, -0.01, 4.0), (true, 40.0, 26.0, 78.0, 0.52, 0.02, 6.0), (false, 18.0, 22.0, 94.0, 0.71, 0.0, 8.0)];
/// One of its rocks alone (make_zetta_spikes.js: a unit 100 cm across and tall, centred): fmd_stn_a01, fmd_stn_a03.
pub const SPIKE_PIECE_FX: [&str; 2] = ["vd_zetta_spk_chunk_a", "vd_zetta_spk_chunk_b"];

/// (round 5c) ONE ROCK OF A BROKEN SPIKE: everything a closed-form function of the run's time (frame-rate independent): it
/// falls from `p0` at `v0` under `g`, tumbling at `spin` (axis x rad/s), lands on its ground (traced under where it comes
/// down), bounces (`zetta.spikes_bounce`), slides to rest, then crumbles away from `crumble_at`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RockPiece {
    pub spike: usize,
    pub b: bool,
    /// cm: across, across, tall.
    pub size: Vec3,
    pub t0: f32,
    pub p0: Vec3,
    pub v0: Vec3,
    pub q0: glam::Quat,
    pub spin: Vec3,
    pub g: f32,
    /// Its centre's height at rest on its ground.
    pub ground: f32,
    pub bounce: f32,
    /// It breaks in two at its first landing.
    pub splits: bool,
    pub crumble_at: f32,
    pub crumble_time: f32,
    pub handle: Option<EffectHandle>,
    /// Its first landing was had (its plume, its thud, its split).
    pub landed: bool,
}

/// (round 5c) A rock's flight at `tau` s after it flew: (place, velocity, its first landing, its second).
pub fn rock_flight(p0: Vec3, v0: Vec3, g: f32, ground: f32, bounce: f32, tau: f32) -> (Vec3, Vec3, f32, f32) {
    let g = g.max(1.0);
    let h = (p0.z - ground).max(0.0);
    let land1 = if h <= 1e-3 && v0.z <= 0.0 { 0.0 } else { (v0.z + (v0.z * v0.z + 2.0 * g * h).sqrt()) / g };
    if tau <= land1 {
        return (p0 + v0 * tau - Vec3::Z * (0.5 * g * tau * tau), v0 - Vec3::Z * (g * tau), land1, f32::INFINITY);
    }
    let p1 = Vec3::new(p0.x + v0.x * land1, p0.y + v0.y * land1, ground);
    let vz1 = v0.z - g * land1;
    let vb = Vec3::new(v0.x * 0.45, v0.y * 0.45, -vz1 * bounce.clamp(0.0, 0.9));
    let land2 = land1 + 2.0 * vb.z / g;
    if tau <= land2 {
        let s = tau - land1;
        return (p1 + vb * s - Vec3::Z * (0.5 * g * s * s), vb - Vec3::Z * (g * s), land1, land2);
    }
    let p2 = Vec3::new(p1.x + vb.x * (land2 - land1), p1.y + vb.y * (land2 - land1), ground);
    let k = 5.0;
    let e = (-k * (tau - land2)).exp();
    let vh = Vec3::new(vb.x, vb.y, 0.0) * 0.5;
    (p2 + vh * ((1.0 - e) / k), vh * e, land1, land2)
}

impl RockPiece {
    /// Its place, turn and scale (of its unit) at the run's time `t`; scale 0: gone.
    pub fn at(&self, t: f32) -> (Vec3, glam::Quat, Vec3) {
        let tau = (t - self.t0).max(0.0);
        let (p, _, l1, l2) = rock_flight(self.p0, self.v0, self.g, self.ground, self.bounce, tau);
        let turned = tau.min(l1) + 0.5 * (tau.min(l2) - l1).max(0.0);
        let rate = self.spin.length();
        let q = if rate > 1e-4 { glam::Quat::from_axis_angle(self.spin / rate, rate * turned) * self.q0 } else { self.q0 };
        let k = ((t - self.crumble_at) / self.crumble_time.max(0.05)).clamp(0.0, 1.0);
        let shrink = 1.0 - k * k * (3.0 - 2.0 * k);
        // (crumbling it settles into its rubble: lower as it goes)
        (p - Vec3::Z * (self.size.z * 0.35 * k), q, self.size / 100.0 * shrink)
    }

    /// Its first landing (run time).
    pub fn lands_at(&self) -> f32 {
        self.t0 + rock_flight(self.p0, self.v0, self.g, self.ground, self.bounce, 0.0).2
    }
}

fn quat_rot(q: glam::Quat) -> Vec3 {
    axes_rot(q * Vec3::X, q * Vec3::Y, q * Vec3::Z)
}


/// The spike (ZETTA/tools/make_zetta_spikes.js: a unit 100 cm tall, its foot 100 cm across, local +Z up).
pub const SPIKE_FX: &str = "vd_zetta_spk_rock";
/// Its crumble, at the same unit.
pub const SPIKE_DUST_FX: &str = "vd_zetta_spk_dust";

fn hash01(seed: u32, k: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B).wrapping_add(0x27D4_EB2F);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x & 0xFFFF) as f32 / 65535.0
}

/// (round 5c) WHERE THE SPIKES STAND, decided as the ending begins: only when the impact (the target's centre where the beam
/// hit it) is within `zetta.spikes_ground_max` x the beam's girth of the ground under it; `zetta.spikes_count` of them
/// round its foot at `zetta.spikes_radius` x the girth (each a little nearer or further, `zetta.spikes_spread`), each on the
/// ground at its own spot (its normal: a slope is followed), leaning out by about `zetta.spikes_lean`, `zetta.spikes_height`
/// x the girth tall (0.7 .. 1.3) and `zetta.spikes_width` x the girth's square root across. A spot behind a wall from the
/// impact, off a ledge or on ground far above or below the impact's is skipped; so is one whose spike would come within
/// `zetta.spikes_clear` of Donald, an ally or Donald's zone (`keep_off`: centre, radius), or of a camera of the ending's
/// shots (`cams`). Deterministic (`seed`). Empty: no spikes.
#[allow(clippy::too_many_arguments)]
pub fn spike_layout(impact: Vec3, girth: f32, world: &dyn kh_core::World, keep_off: &[(Vec3, f32)], cams: &[Vec3], z: &ZettaTune, seed: u32) -> Vec<Spike> {
    spike_layout_from(impact, None, girth, world, keep_off, cams, z, seed)
}

/// (round 5c, the user: "The rock spikes from the zettaflare being close to the ground have to travel all the way down the
/// beam") [`spike_layout`] with a TRAIL down the beam: from `from` (the beam's source, his wand tip) to the impact, a spike
/// every `zetta.spikes_trail_step` (x the girth's square root) along the beam's ground track, alternating sides at
/// `zetta.spikes_trail_lane` (x the girth, plus or minus a third) from it, leaning away from the line, growing taller toward
/// the impact; at most `zetta.spikes_trail_max`. Each on its own ground (slopes followed), none through a wall from the
/// track, off a ledge, or near Donald, an ally, his zone or the ending's cameras.
#[allow(clippy::too_many_arguments)]
pub fn spike_layout_from(impact: Vec3, from: Option<Vec3>, girth: f32, world: &dyn kh_core::World, keep_off: &[(Vec3, f32)], cams: &[Vec3], z: &ZettaTune, seed: u32) -> Vec<Spike> {
    if z.zetta_spikes < 0.5 {
        return Vec::new();
    }
    let size = girth.clamp(1.0, 2.5); // a giant's crown grows with it, up to 2.5 x
    let reach = z.zetta_spikes_ground_max.max(0.0) * girth.max(1.0);
    let Some(ground) = world.line_trace(impact + Vec3::Z * 50.0, impact - Vec3::Z * (reach + 50.0)) else { return Vec::new() };
    if impact.z - ground.position.z > reach {
        return Vec::new();
    }
    let foot0 = ground.position;
    let n = z.zetta_spikes_count.round().clamp(0.0, 32.0) as u32;
    let mut out = Vec::new();
    for k in 0..n {
        let ang = ((k as f32 + 0.6 * hash01(seed, k)) / n.max(1) as f32) * std::f32::consts::TAU;
        let dir = Vec3::new(ang.cos(), ang.sin(), 0.0);
        let r = z.zetta_spikes_radius.max(0.0) * size * (1.0 + z.zetta_spikes_spread * (hash01(seed, k + 100) * 2.0 - 1.0));
        let p = foot0 + dir * r;
        // not through a wall from the impact
        if world.line_trace(foot0 + Vec3::Z * 80.0, p + Vec3::Z * 80.0).is_some() {
            continue;
        }
        // its own ground (none: off a ledge), not far above or below the impact's
        let Some(g) = world.line_trace(p + Vec3::Z * 300.0, p - Vec3::Z * 400.0) else { continue };
        if (g.position.z - foot0.z).abs() > 150.0 * size.min(3.0) {
            continue;
        }
        let height = z.zetta_spikes_height.max(0.0) * size * (0.7 + 0.6 * hash01(seed, k + 200));
        let width = z.zetta_spikes_width.max(0.0) * size.sqrt() * (0.8 + 0.4 * hash01(seed, k + 300));
        let normal = if g.impact_normal.z > 0.5 { g.impact_normal.normalize() } else { Vec3::Z };
        let lean = (z.zetta_spikes_lean * (0.7 + 0.6 * hash01(seed, k + 400))).to_radians();
        let up = (normal * lean.cos() + dir * lean.sin()).normalize_or(Vec3::Z);
        let foot = g.position - up * z.zetta_spikes_sink.max(0.0);
        let tip = foot + up * height;
        let clear = z.zetta_spikes_clear.max(0.0) + width * 0.5;
        if keep_off.iter().any(|(c, rad)| seg_dist(*c, foot, tip) < clear + rad) || cams.iter().any(|c| seg_dist(*c, foot, tip) < clear + 100.0) {
            continue;
        }
        out.push(Spike { foot, up, height, width, turn: hash01(seed, k + 500) * 360.0, delay: z.zetta_spikes_stagger.max(0.0) * hash01(seed, k + 600), handle: None, dust: false, broke: false, said: 0 });
    }
    // the trail down the beam
    if let Some(src) = from {
        let a = Vec3::new(src.x, src.y, foot0.z);
        let track = crate::pawn::horizontal(foot0 - a);
        let len = track.length();
        let along = track.normalize_or_zero();
        let across = Vec3::new(-along.y, along.x, 0.0);
        // (round 5c, the user: "the rocks at the end need to travel all the way down the beam") EVENLY the whole way, from just
        // outside his body to the impact: as many as the track's length takes (`zetta.spikes_trail_max` only a safety cap)
        let body = 90.0;
        let step0 = (z.zetta_spikes_trail_step.max(50.0) * size.sqrt()).max(50.0);
        let span = (len - body).max(0.0);
        let n = ((span / step0).floor() as u32 + 1).min(z.zetta_spikes_trail_max.round().clamp(0.0, 128.0) as u32);
        let step = span / n.max(1) as f32;
        for k in 0..n {
            let id = 1000 + k;
            let d = len - (k as f32 + 0.5) * step; // from the impact back toward him
            let c = a + along * d;
            // either side of the line; a wall on one side: the other
            let lane = z.zetta_spikes_trail_lane.max(0.0) * size * (0.67 + 0.66 * hash01(seed, id));
            let first = if k % 2 == 0 { 1.0 } else { -1.0 };
            let mut found = None;
            for side in [first, -first] {
                let p = c + across * (side * lane) + along * (step * 0.3 * (hash01(seed, id + 100) - 0.5));
                if world.line_trace(c + Vec3::Z * 80.0, p + Vec3::Z * 80.0).is_some() {
                    continue;
                }
                // its own ground, however far under the beam (only a true ledge from the impact's ground is skipped)
                let Some(g) = world.line_trace(p + Vec3::Z * 600.0, p - Vec3::Z * 1500.0) else { continue };
                if (g.position.z - foot0.z).abs() > 500.0 * size.min(3.0) {
                    continue;
                }
                found = Some((side, g));
                break;
            }
            let Some((side, g)) = found else { continue };
            // shorter near him, as tall as the crown at the impact
            let grow = 0.45 + 0.55 * (d / len.max(1.0));
            let height = z.zetta_spikes_height.max(0.0) * size * grow * (0.75 + 0.5 * hash01(seed, id + 200));
            let width = z.zetta_spikes_width.max(0.0) * size.sqrt() * (0.7 + 0.4 * hash01(seed, id + 300)) * (0.6 + 0.4 * grow);
            let normal = if g.impact_normal.z > 0.5 { g.impact_normal.normalize() } else { Vec3::Z };
            let lean = (z.zetta_spikes_lean * (0.7 + 0.6 * hash01(seed, id + 400))).to_radians();
            let up = (normal * lean.cos() + across * (side * lean.sin())).normalize_or(Vec3::Z);
            let foot = g.position - up * z.zetta_spikes_sink.max(0.0);
            let tip = foot + up * height;
            // (clear of his and the allies' bodies only: no empty stretch at his end; never into a camera of the ending)
            let clear = 40.0 + width * 0.5;
            if keep_off.iter().filter(|(_, rad)| *rad <= 200.0).any(|(c, rad)| seg_dist(*c, foot, tip) < clear + rad) || cams.iter().any(|c| seg_dist(*c, foot, tip) < z.zetta_spikes_clear.max(0.0) + width * 0.5 + 100.0) {
                continue;
            }
            // those near the impact crumble first, the trail toward him after
            out.push(Spike { foot, up, height, width, turn: hash01(seed, id + 500) * 360.0, delay: z.zetta_spikes_stagger.max(0.0) * (0.3 + 0.7 * (1.0 - d / len.max(1.0))), handle: None, dust: false, broke: false, said: 0 });
        }
    }
    out
}

/// (round 5c) A spike's place now: `crumble` 0..1 (0 standing, 1 gone): it sinks and narrows as it breaks up. Returns the
/// effect's (place, rotation, scale) for its unit (100 cm tall, 100 across).
pub fn spike_transform(s: &Spike, crumble: f32) -> (Vec3, Vec3, Vec3) {
    let k = crumble.clamp(0.0, 1.0);
    let k = k * k;
    let h = s.height * (1.0 - 0.85 * k);
    let w = s.width * (1.0 - 0.55 * k);
    let foot = s.foot - s.up * (s.height * 0.3 * k);
    let side0 = if s.up.z.abs() < 0.999 { Vec3::Z.cross(s.up).normalize() } else { Vec3::X };
    let fwd0 = s.up.cross(side0);
    let (sn, cs) = s.turn.to_radians().sin_cos();
    let ax = side0 * cs + fwd0 * sn;
    let ay = s.up.cross(ax);
    (foot, axes_rot(ax, ay, s.up), Vec3::new(w / 100.0, w / 100.0, h / 100.0))
}

/// (round 5c) The spikes through a run's ending: laid out and standing from its start (under its white: nothing of their
/// rising shows; the shots before never see them), crumbling one after another from `zetta.spikes_crumble_at` over
/// `zetta.spikes_crumble_time` (in the shot behind Donald), each gone with its rubble and dust before the hand-off.
fn spikes_step(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, t: f32, z: &ZettaTune, staff: Vec3, ending: bool) {
    let channel = run.channel;
    // (round 5c, the user: "have the rocks actually burst out from the ground with a sound effect, but they should already be
    // there when the camera cuts to the wide shot of the beam shrinking, no pop in"; heard, not seen) laid out at
    // `zetta.spikes_rise_at`, in the target shot, where the ending will stand (his feet, the beam's line: the frame the
    // ending fixes at its start)
    let rise0 = seq_time(z.zetta_spikes_rise_at, z, channel);
    if run.ending && ending && !run.spikes_laid && run.beam_at_hit.is_some() && t + 1e-4 >= rise0 {
        run.spikes_laid = true;
        let pre = run.beam_at_hit.map(|b| {
            let flat = crate::pawn::horizontal(b.x);
            let feet = party.member(run.caster).map_or(b.origin, |p| p.position());
            Frame { origin: feet, yaw: if flat.length() > 1e-3 { crate::pawn::yaw_of(flat) } else { party.member(run.caster).map_or(0.0, |p| p.yaw) } }
        });
        if let (Some(b), Some(fr)) = (run.beam_at_hit, run.end_frame.or(pre)) {
            let mut keep: Vec<(Vec3, f32)> = party.members().filter_map(|m| party.member(m)).map(|p| (p.position() + Vec3::Z * 60.0, 120.0)).collect();
            if let Some(dz) = run.zone_donald {
                keep.push((dz.at, dz.radius));
            }
            let cams: Vec<Vec3> = cam_shots().iter().filter(|s| s.frame == "end").flat_map(|s| s.keys.iter().map(|k| fr.point(k.1)).collect::<Vec<_>>()).collect();
            let from = run.end_beam.map(|e| e.origin).or(Some(staff));
            run.spikes = spike_layout_from(b.origin, from, run.girth, host.world, &keep, &cams, z, run.begun.wrapping_mul(7919) ^ run.caster.0 as u32);
            // (debug overlay) the beam's ground track
            if let (Some(src), Some(g)) = (from, host.world.line_trace(b.origin + Vec3::Z * 50.0, b.origin - Vec3::Z * 2000.0)) {
                if let Ok(mut m) = TRACK.lock() {
                    *m = Some((Vec3::new(src.x, src.y, g.position.z), g.position));
                }
            }
            for s in run.spikes.iter_mut() {
                // (under the ground until it bursts out)
                let (at, rot, sc) = spike_rise(s, 0.0);
                let sc = sc * 0.001;
                let Some(p) = party.member_mut(run.caster) else { break };
                let h = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(SPIKE_FX)), name: SPIKE_FX, at: Attach::World(at), rotation: rot, scale: 1.0, persistent: true });
                host.effects.transform(h, at, rot, sc);
                s.handle = Some(h);
                // (logged where it will stand)
                party.don_zetta.log.push(ZettaLog::Spawn { row: "spikes.rock", t, at: spike_transform(s, 0.0).0, rotation: rot, handle: h });
            }
        }
    }
    if run.spikes.is_empty() {
        return;
    }
    let from = seq_time(z.zetta_spikes_crumble_at, z, channel);
    let span = z.zetta_spikes_crumble_time.max(0.05);
    // (the user's freedom on cuts) bigger in the wide shot, their own size from the cut behind Donald (970)
    let wide = if t + 1e-4 < seq_time(z.zetta_end_start + 7.1666667, z, channel) { z.zetta_spikes_wide_scale.max(0.1) } else { 1.0 };
    // (round 5c, the user: "the rocks need to physically crumble to the ground, actually breaking apart") each spike CRACKS
    // (`zetta.spikes_crack_time`: it shudders, grit and dust trickle from it, its crack is heard), then BREAKS: its own rocks
    // fly apart one by one (`RockPiece`) and the spike is gone
    let crack = z.zetta_spikes_crack_time.max(0.0);
    let end0 = seq_time(z.zetta_end_start, z, channel);
    for i in 0..run.spikes.len() {
        let s = run.spikes[i];
        let k = ((t - from - s.delay) / span).clamp(0.0, 1.0);
        if k > 0.0 && s.said & 1 == 0 {
            run.spikes[i].said |= 1;
            spike_sound(run, party, host, "spk.se.crack", s.foot + s.up * (s.height * 0.6), 1.0, t);
        }
        if !s.broke && k > 0.0 && t - from - s.delay >= crack {
            run.spikes[i].broke = true;
            if let Some(h) = s.handle {
                host.effects.stop(h, true);
                run.spikes[i].handle = None;
                party.don_zetta.log.push(ZettaLog::Stop { row: "spikes.rock", t });
            }
            let t_break = from + s.delay + crack;
            let pieces = spike_rocks(&s, i, t_break, host.world, z, run.begun.wrapping_mul(31) ^ i as u32);
            for mut r in pieces {
                let (at, q, sc) = r.at(t);
                let Some(p) = party.member_mut(run.caster) else { break };
                let name = SPIKE_PIECE_FX[r.b as usize];
                let h = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), rotation: quat_rot(q), scale: 1.0, persistent: true });
                host.effects.transform(h, at, quat_rot(q), sc);
                r.handle = Some(h);
                party.don_zetta.log.push(ZettaLog::Spawn { row: "spikes.piece", t, at, rotation: quat_rot(q), handle: h });
                run.rocks.push(r);
            }
        }
        let s = run.spikes[i];
        let Some(h) = s.handle else { continue };
        if k > 0.0 && !s.dust {

            run.spikes[i].dust = true;
            let (at, rot, sc) = spike_transform(&s, 0.0);
            if let Some(p) = party.member_mut(run.caster) {
                let d = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(SPIKE_DUST_FX)), name: SPIKE_DUST_FX, at: Attach::World(at), rotation: rot, scale: 1.0, persistent: false });
                host.effects.transform(d, at, rot, sc);
                party.don_zetta.log.push(ZettaLog::Spawn { row: "spikes.dust", t, at, rotation: rot, handle: d });
            }
        }
        if k >= 1.0 {
            host.effects.stop(h, true);
            run.spikes[i].handle = None;
            party.don_zetta.log.push(ZettaLog::Stop { row: "spikes.rock", t });
        } else if k > 0.0 {
            // cracking: it shudders where it stands (growing)
            let (at, rot, sc) = spike_transform(&s, 0.0);
            let c = ((t - from - s.delay) / crack.max(1e-3)).clamp(0.0, 1.0);
            let amp = 1.6 * c;
            let jig = Vec3::new((t * 53.0 + i as f32).sin() * amp, 0.0, (t * 41.0 + 2.0 * i as f32).cos() * amp);
            host.effects.transform(h, at - s.up * (s.height * 0.02 * c), rot + jig, sc * wide);
        } else {
            // bursting out of the ground (one after another), then standing; in the target shot (before the wide shot) one
            // in its view is not drawn: heard, not seen
            let rk = ((t - rise0 - spike_rise_delay(&s, z)) / z.zetta_spikes_rise_time.max(0.05)).clamp(0.0, 1.0);
            if rk > 0.0 && s.said & 8 == 0 {
                run.spikes[i].said |= 8;
                spike_sound(run, party, host, "spk.se.rise", s.foot, 1.0, t);
                if !run.spikes.iter().any(|x| x.said & 32 != 0) {
                    run.spikes[i].said |= 32;
                    spike_sound(run, party, host, "spk.se.rumble", s.foot, 1.0, t);
                }
            }
            let unseen = t + 1e-4 < end0 && party.don_zetta.cam.last.is_some_and(|c| in_view(&c, s.foot + s.up * (s.height * 0.5)) || in_view(&c, s.foot + s.up * s.height) || in_view(&c, s.foot));
            if rk >= 1.0 && s.said & 16 == 0 {
                run.spikes[i].said |= 16;
                spike_sound(run, party, host, "spk.se.clatter", s.foot, 1.0, t);
                if !unseen {
                    spawn_dust(run, party, host, s.foot, (s.width / 100.0).max(0.3) * wide, t);
                }
            }
            let (at, rot, sc) = spike_rise(&s, rk);
            host.effects.transform(h, at, rot, if unseen || rk <= 0.0 { sc * 0.001 } else { sc * wide });
        }
    }
    rocks_step(run, party, host, t);
}

/// (round 5c) Seconds after `zetta.spikes_rise_at` a spike bursts out (its crumble's order, over `zetta.spikes_rise_stagger`).
pub fn spike_rise_delay(s: &Spike, z: &ZettaTune) -> f32 {
    s.delay / z.zetta_spikes_stagger.max(1e-3) * z.zetta_spikes_rise_stagger.max(0.0)
}

/// (round 5c) A spike bursting out of the ground: `k` 0 (under it) .. 1 (standing), fast, overshooting a little and settling.
pub fn spike_rise(s: &Spike, k: f32) -> (Vec3, Vec3, Vec3) {
    let k = k.clamp(0.0, 1.0);
    let c = 1.9;
    let e = 1.0 + (c + 1.0) * (k - 1.0).powi(3) + c * (k - 1.0).powi(2);
    let (at, rot, sc) = spike_transform(s, 0.0);
    (at - s.up * (s.height * (1.0 - e)), rot, sc)
}

/// (round 5c) Is a point in a camera pose's view (its horizontal field and a margin)?
pub fn in_view(c: &CamPose, p: Vec3) -> bool {
    let d = p - c.position;
    let l = d.length();
    if l < 1.0 {
        return true;
    }
    let f = rot_axes(c.rotation)[0];
    d.dot(f) / l > (c.fov * 0.5 + 12.0).min(89.0).to_radians().cos()
}

/// (round 5c) THE GROUND DUST (the user: "Zettaflare should kick up dust from the ground as well, visible at the two end
/// shots"): where it lies, from `from` (his wand tip) to the impact: billows along the beam's ground track every
/// `zetta.dust_step` (x the girth's square root), either side of it (`zetta.dust_lane`), only where the beam ran within
/// `zetta.dust_reach` of the ground, and a ring of them round the impact's foot; each on its ground (traced), none through a
/// wall from the track nor near Donald or an ally (`keep`); at most `zetta.dust_max`.
pub fn dust_layout(impact: Vec3, from: Vec3, girth: f32, world: &dyn kh_core::World, keep: &[(Vec3, f32)], z: &ZettaTune, seed: u32) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::new();
    let size = girth.max(1.0);
    let reach = z.zetta_dust_reach.max(0.0) * size;
    let ground_under = |p: Vec3| world.line_trace(p + Vec3::Z * 300.0, p - Vec3::Z * (reach + 600.0)).map(|h| h.position);
    // (round 5c: "evenly along the whole ground track, from near Donald to the impact") clear of his and the allies' bodies only
    let ok = |p: Vec3, out: &Vec<Vec3>| !keep.iter().filter(|(_, r)| *r <= 200.0).any(|(c, r)| (p - *c).truncate().length() < r + 60.0) && !out.iter().any(|q| (*q - p).length() < 120.0 * size.sqrt());
    let max = z.zetta_dust_max.round().clamp(0.0, 64.0) as usize;
    // round the impact's foot
    if let Some(foot) = ground_under(impact) {
        if impact.z - foot.z <= reach {
            for k in 0..6u32 {
                let a = (k as f32 / 6.0 + 0.1 * hash01(seed, k)) * std::f32::consts::TAU;
                let p = foot + Vec3::new(a.cos(), a.sin(), 0.0) * (z.zetta_dust_lane * 1.6 * size * (0.7 + 0.6 * hash01(seed, k + 20)));
                if world.line_trace(foot + Vec3::Z * 80.0, p + Vec3::Z * 80.0).is_some() {
                    continue;
                }
                if let Some(g) = ground_under(p) {
                    if out.len() < max && ok(g, &out) {
                        out.push(g);
                    }
                }
            }
        }
    }
    // down the beam's ground track
    let flat = Vec3::new(impact.x - from.x, impact.y - from.y, 0.0);
    let len = flat.length();
    let along = flat.normalize_or_zero();
    let across = Vec3::new(-along.y, along.x, 0.0);
    let step = (z.zetta_dust_step.max(50.0) * size.sqrt()).max(50.0);
    let n = (len / step).floor() as u32;
    for k in 0..n {
        if out.len() >= max {
            break;
        }
        let d = len - (k as f32 + 0.5) * step;
        let line = from + (impact - from) * (d / len.max(1.0));
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let p = Vec3::new(line.x, line.y, line.z) + across * (side * z.zetta_dust_lane * size * (0.67 + 0.66 * hash01(seed, k + 40)));
        let Some(g) = ground_under(Vec3::new(p.x, p.y, line.z)) else { continue };
        if line.z - g.z > reach || world.line_trace(Vec3::new(line.x, line.y, g.z + 80.0), g + Vec3::Z * 80.0).is_some() {
            continue;
        }
        if ok(g, &out) {
            out.push(g);
        }
    }
    out
}

/// (round 5c) The ground dust over a run's ending: laid as it begins (under its white), billowing through the wide shot,
/// stopped at `zetta.dust_stop_at` (each billow thins out over its life: gone by the hand-off).
fn dust_step(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, t: f32, z: &ZettaTune) {
    if z.zetta_dust < 0.5 {
        return;
    }
    let spawn = |run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, name: &'static str, row: &'static str, p: Vec3| {
        let Some(m) = party.member_mut(run.caster) else { return };
        let h = m.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(p), rotation: Vec3::ZERO, scale: 1.0, persistent: true });
        host.effects.transform(h, p, Vec3::ZERO, Vec3::ONE);
        run.dust.push((h, row));
        party.don_zetta.log.push(ZettaLog::Spawn { row, t, at: p, rotation: Vec3::ZERO, handle: h });
    };
    // where it lies, and the wide shot's big billows (88 m off), as the ending begins
    if run.ending && !run.dust_laid {
        if let (Some(b), Some(e)) = (run.beam_at_hit, run.end_beam) {
            run.dust_laid = true;
            let mut keep: Vec<(Vec3, f32)> = party.members().filter_map(|m| party.member(m)).map(|p| (p.position(), 150.0)).collect();
            if let Some(dz) = run.zone_donald {
                keep.push((dz.at, dz.radius));
            }
            run.dust_at = dust_layout(b.origin, e.origin, run.girth, host.world, &keep, z, run.begun.wrapping_mul(131) ^ 7);
            run.dust_from = Some(e.origin);
            for p in run.dust_at.clone() {
                spawn(run, party, host, GROUND_DUST_WIDE_FX, "dust.wide", p);
            }
            event_dust(run, party, host, t);
        }
    }
    // (round 5c) the event's own dust again, in waves through the wide shot (each its own 1 s of puffs, 3 s each)
    if run.dust_laid && run.dust_waves > 0 && run.dust_waves < z.zetta_dust_c100_waves.round().max(1.0) as u32 {
        let next = seq_time(z.zetta_end_start + z.zetta_dust_c100_every * run.dust_waves as f32, z, run.channel);
        if t + 1e-4 >= next {
            event_dust(run, party, host, t);
        }
    }
    // the near billows for the shot behind Donald (rising before its cut)
    if run.dust_laid && !run.dust_near && t + 1e-4 >= seq_time(z.zetta_dust_near_at, z, run.channel) {
        run.dust_near = true;
        // (every other place: the shot behind him is near, a haze, not a wall)
        for p in run.dust_at.clone().into_iter().step_by(2) {
            spawn(run, party, host, GROUND_DUST_FX, "dust.ground", p);
        }
    }
    // each layer stops rising in time to thin away (the wide one by the cut, the near one by the hand-off)
    let (wide_stop, near_stop) = (seq_time(z.zetta_dust_wide_stop_at, z, run.channel), seq_time(z.zetta_dust_stop_at, z, run.channel));
    let mut i = 0;
    while i < run.dust.len() {
        let (h, row) = run.dust[i];
        if t + 1e-4 >= if row == "dust.wide" { wide_stop } else { near_stop } {
            host.effects.stop(h, false);
            party.don_zetta.log.push(ZettaLog::Stop { row, t });
            run.dust.remove(i);
        } else {
            i += 1;
        }
    }
}

/// (round 5c, the coordinator: "pack and use kg702c's own vfx_c100_smk_00") THE EVENT'S OWN DUST (kg702c cut 1000: warm tan
/// puffs drifting along the ground, `vd_kg702b_100_smk_00`) at every place of the ground dust, drifting outward from the
/// beam's line (its puffs move along its local -Y); one wave.
fn event_dust(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, t: f32) {
    run.dust_waves += 1;
    let Some(from) = run.dust_from else { return };
    let to = run.beam_at_hit.map_or(from, |b| b.origin);
    let along = Vec3::new(to.x - from.x, to.y - from.y, 0.0).normalize_or(Vec3::X);
    for p in run.dust_at.clone() {
        let rel = Vec3::new(p.x - from.x, p.y - from.y, 0.0);
        let off = rel - along * rel.dot(along);
        let o = off.normalize_or(Vec3::new(-along.y, along.x, 0.0));
        let yaw = o.x.atan2(-o.y).to_degrees();
        let Some(m) = party.member_mut(run.caster) else { return };
        let h = m.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(EVENT_DUST_FX)), name: EVENT_DUST_FX, at: Attach::World(p + Vec3::Z * 40.0), rotation: Vec3::new(0.0, yaw, 0.0), scale: 1.0, persistent: false });
        party.don_zetta.log.push(ZettaLog::Spawn { row: "dust.c100", t, at: p, rotation: Vec3::new(0.0, yaw, 0.0), handle: h });
    }
}

/// (round 5c) kg702c's own dust (cut 1000).
pub const EVENT_DUST_FX: &str = "vd_kg702b_100_smk_00";

/// (round 5c) One billow of the ground dust (make_zetta_spikes.js).
pub const GROUND_DUST_FX: &str = "vd_zetta_ground_dust";
/// (round 5c) Its wide shot's layer (three times the size: an effect's scale never sizes its particles).
pub const GROUND_DUST_WIDE_FX: &str = "vd_zetta_ground_dust_wide";

/// (round 5c) A broken spike's rocks, from its unit's stack ([`SPIKE_ROCKS`]) where it stood: each pushed out along the
/// spike's lean (the top ones the farthest, `zetta.spikes_fall_out`), a little scatter, tumbling; its ground traced under
/// where it comes down (slopes, steps); a giant spike's top lands within `zetta.spikes_fall_time` (more gravity); the three
/// big ones split in two at their first landing; each crumbles from `zetta.spikes_settle_at` after the crack began (a little
/// later each) over `zetta.spikes_rock_crumble`.
pub fn spike_rocks(s: &Spike, index: usize, t_break: f32, world: &dyn kh_core::World, z: &ZettaTune, seed: u32) -> Vec<RockPiece> {
    let (foot, _, sc) = spike_transform(s, 0.0);
    let side0 = if s.up.z.abs() < 0.999 { Vec3::Z.cross(s.up).normalize() } else { Vec3::X };
    let fwd0 = s.up.cross(side0);
    let (sn, cs) = s.turn.to_radians().sin_cos();
    let ax = side0 * cs + fwd0 * sn;
    let ay = s.up.cross(ax);
    let frame = glam::Quat::from_mat3(&glam::Mat3::from_cols(ax, ay, s.up));
    let lean = Vec3::new(s.up.x, s.up.y, 0.0).normalize_or(Vec3::new(ax.x, ax.y, 0.0).normalize_or(Vec3::X));
    let top = foot.z + s.height;
    let g = 980.0f32.max(2.0 * s.height / z.zetta_spikes_fall_time.max(0.3).powi(2));
    let mut out = Vec::new();
    for (j, &(b, across, tall, up, turn, tip, dx)) in SPIKE_ROCKS.iter().enumerate() {
        let k = j as u32 * 17;
        let p0 = foot + ax * (dx * sc.x) + s.up * (up * sc.z);
        let size = Vec3::new(across * sc.x, across * sc.y, tall * sc.z);
        let q0 = frame * glam::Quat::from_rotation_z(turn * std::f32::consts::TAU) * glam::Quat::from_rotation_x(tip * std::f32::consts::TAU);
        let height_share = ((p0.z - foot.z) / s.height.max(1.0)).clamp(0.0, 1.0);
        let scatter = Vec3::new(hash01(seed, k) - 0.5, hash01(seed, k + 1) - 0.5, 0.0) * 2.0;
        let v0 = lean * (z.zetta_spikes_fall_out * (0.33 + 0.67 * height_share)) + scatter * (z.zetta_spikes_fall_out * 0.35) + Vec3::Z * (40.0 * hash01(seed, k + 2));
        // where it comes down: its ground there
        let fall = (2.0 * (p0.z - foot.z).max(0.0) / g).sqrt().max(0.1);
        let down = p0 + Vec3::new(v0.x, v0.y, 0.0) * fall;
        let ground = world.line_trace(Vec3::new(down.x, down.y, top + 200.0), Vec3::new(down.x, down.y, foot.z - 600.0)).map_or(foot.z + z.zetta_spikes_sink, |h| h.position.z) + size.z * 0.4;
        let axis = Vec3::new(hash01(seed, k + 3) - 0.5, hash01(seed, k + 4) - 0.5, hash01(seed, k + 5) - 0.5).normalize_or(Vec3::X);
        let crumble_at = t_break - z.zetta_spikes_crack_time.max(0.0) + z.zetta_spikes_settle_at + 0.25 * hash01(seed, k + 6);
        out.push(RockPiece { spike: index, b, size, t0: t_break, p0, v0, q0, spin: axis * (1.2 + 3.0 * hash01(seed, k + 7)), g, ground, bounce: z.zetta_spikes_bounce, splits: j < 3, crumble_at, crumble_time: z.zetta_spikes_rock_crumble.max(0.05), handle: None, landed: false });
    }
    out
}

/// (round 5c) The rocks every step: placed; at a first landing its dust plume, its thud and (the big ones) its split in two;
/// crumbling, its dust; gone, stopped.
fn rocks_step(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, t: f32) {
    let mut born = Vec::new();
    for i in 0..run.rocks.len() {
        let r = run.rocks[i];
        let Some(h) = r.handle else { continue };
        if !r.landed && t >= r.lands_at() {
            run.rocks[i].landed = true;
            let (at, q, _) = r.at(r.lands_at());
            let ground = at - Vec3::Z * (r.size.z * 0.4);
            if r.splits && t < r.crumble_at {
                // its plume and its thud
                spawn_dust(run, party, host, ground, r.size.x / 100.0 * 0.8, t);
                spike_sound(run, party, host, "spk.se.thud", ground, (r.size.x / 150.0).clamp(0.4, 1.2), t);
                if run.spikes.get(r.spike).is_some_and(|s| s.said & 2 == 0) {
                    run.spikes[r.spike].said |= 2;
                    spike_sound(run, party, host, "spk.se.rubble", ground, 1.0, t);
                }
                // it breaks in two: each half flies on from where it landed
                host.effects.stop(h, true);
                run.rocks[i].handle = None;
                party.don_zetta.log.push(ZettaLog::Stop { row: "spikes.piece", t });
                let (_, v, _, _) = rock_flight(r.p0, r.v0, r.g, r.ground, r.bounce, r.lands_at() - r.t0 - 1e-3);
                let across = (q * Vec3::X).normalize_or(Vec3::X);
                for (n, sign) in [(0u32, 1.0f32), (1, -1.0)] {
                    let seed = (i as u32).wrapping_mul(97) ^ n;
                    let size = Vec3::new(r.size.x * 0.68, r.size.y * 0.68, r.size.z * 0.75);
                    born.push(RockPiece {
                        size,
                        t0: t,
                        p0: at + across * (sign * r.size.x * 0.22) + Vec3::Z * 4.0,
                        v0: Vec3::new(v.x, v.y, 0.0) * 0.5 + across * (sign * (90.0 + 80.0 * hash01(seed, 1))) + Vec3::Z * (80.0 + 90.0 * hash01(seed, 2)),
                        q0: q,
                        spin: Vec3::new(hash01(seed, 3) - 0.5, hash01(seed, 4) - 0.5, hash01(seed, 5) - 0.5).normalize_or(Vec3::Y) * (2.0 + 3.0 * hash01(seed, 6)),
                        g: 980.0,
                        ground: ground.z + size.z * 0.4,
                        splits: false,
                        landed: true,
                        handle: None,
                        ..r
                    });
                }
                continue;
            }
        }
        let k = (t - r.crumble_at) / r.crumble_time;
        if k >= 1.0 {
            host.effects.stop(h, true);
            run.rocks[i].handle = None;
            party.don_zetta.log.push(ZettaLog::Stop { row: "spikes.piece", t });
            continue;
        }
        if k >= 0.0 && run.spikes.get(r.spike).is_some_and(|s| s.said & 4 == 0) {
            // the pile crumbles: its dust and its sound (once a spike)
            run.spikes[r.spike].said |= 4;
            let (at, _, _) = r.at(t);
            let w = run.spikes[r.spike].width;
            spawn_dust(run, party, host, at - Vec3::Z * (r.size.z * 0.4), (w / 100.0).max(0.3), t);
            spike_sound(run, party, host, "spk.se.crumble", at, 1.0, t);
        }
        let (at, q, sc) = r.at(t);
        host.effects.transform(h, at, quat_rot(q), sc);
    }
    for mut r in born {
        let (at, q, sc) = r.at(t);
        let Some(p) = party.member_mut(run.caster) else { break };
        let name = SPIKE_PIECE_FX[r.b as usize];
        let h = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), rotation: quat_rot(q), scale: 1.0, persistent: true });
        host.effects.transform(h, at, quat_rot(q), sc);
        r.handle = Some(h);
        party.don_zetta.log.push(ZettaLog::Spawn { row: "spikes.piece", t, at, rotation: quat_rot(q), handle: h });
        run.rocks.push(r);
    }
}

/// (round 5c) A dust plume (the spike's crumble effect: rubble, grit, a dust cloud) at a ground point, `scale` of its unit.
fn spawn_dust(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, at: Vec3, scale: f32, t: f32) {
    let Some(p) = party.member_mut(run.caster) else { return };
    let d = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(SPIKE_DUST_FX)), name: SPIKE_DUST_FX, at: Attach::World(at), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
    host.effects.transform(d, at, Vec3::ZERO, Vec3::splat(scale.max(0.1)));
    party.don_zetta.log.push(ZettaLog::Spawn { row: "spikes.dust", t, at, rotation: Vec3::ZERO, handle: d });
}

/// (round 5c) One of the breaking's sounds (sheet row `id`, kind se_kill / se_spike: its volume the row's scale x
/// `volume`), 3D at `at` (the host's distance attenuation); kept with the run's sounds, so it fades at the hand-off.
fn spike_sound(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, id: &str, at: Vec3, volume: f32, t: f32) {
    let Some(i) = DON_ZETTA_FX.iter().position(|r| r.id == id && r.use_ == 1) else { return };
    let row = &DON_ZETTA_FX[i];
    let Some(p) = party.member_mut(run.caster) else { return };
    let h = p.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(row.asset)), name: row.asset, at: Attach::World(at), volume: row.scale.max(0.0) * volume, pitch: 1.0, looping: false, spatial: true });
    // (a spike's row: its start = the most seconds it plays, then it fades)
    let stop = if row.kind == "se_spike" && row.start > 0.0 { t + row.start } else { f32::INFINITY };
    run.sounds.push((i, h, stop));
    party.don_zetta.push(ZettaLog::Sound { row: row.id, t });
}


/// (round 5c) Every spike still standing goes at once (the run is over or cut); `dust`: each breaks up in its crumble
/// (an early hand-back: they are gone as control returns, not popped away).
fn spikes_stop(run: &mut ZettaRun, party: &mut Party, host: &mut PartyHost, dust: bool) {
    // (round 5c) the ground dust stops rising and thins out
    for (h, _) in run.dust.drain(..) {
        host.effects.stop(h, false);
    }
    // (round 5c) the broken spikes' rocks too
    for r in run.rocks.iter_mut() {
        if let Some(h) = r.handle.take() {
            host.effects.stop(h, true);
        }
    }
    for i in 0..run.spikes.len() {
        let s = run.spikes[i];
        let Some(h) = s.handle else { continue };
        if dust && !s.dust {
            let (at, rot, sc) = spike_transform(&s, 0.0);
            if let Some(p) = party.member_mut(run.caster) {
                let d = p.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(SPIKE_DUST_FX)), name: SPIKE_DUST_FX, at: Attach::World(at), rotation: rot, scale: 1.0, persistent: false });
                host.effects.transform(d, at, rot, sc);
            }
            run.spikes[i].dust = true;
        }
        host.effects.stop(h, true);
        run.spikes[i].handle = None;
    }
}

// ============================================================================================== (round 5c) the allies' hands off

thread_local! {
    /// The party step's snapshot of what the allies keep off ([`hands_off_begin`]): the zones, the reserved targets, the
    /// reach (`zetta.ally_target_clear`).
    static HANDS_OFF: std::cell::RefCell<Option<(Vec<AvoidZone>, Vec<TargetId>, f32)>> = const { std::cell::RefCell::new(None) };
}

/// (round 5c) Party::step, before its members act: the span's zones and reserved targets for [`hands_off_refuses`].
pub(crate) fn hands_off_begin(party: &Party) {
    let snap = party.don_zetta_span().then(|| (party.don_zetta_zones(), party.don_zetta_reserved(), zetta().zetta_ally_target_clear.max(0.0)));
    HANDS_OFF.with(|g| *g.borrow_mut() = snap);
}

/// (round 5c) Party::step, after its members acted.
pub(crate) fn hands_off_end() {
    HANDS_OFF.with(|g| *g.borrow_mut() = None);
}

/// (round 5c) THE ALLIES NEVER HIT WHAT THEY KEEP OFF (the user: "allies should not target anything close to Donald or the
/// Zettaflare or the Zettaflare's expected path ... This includes using magic on enemies"): during the span a party hit
/// (an ally's swing, spell, area, projectile, team row) on Zettaflare's target or on an enemy near a zone is refused, so
/// no area that reaches into a zone lands there. Zettaflare's own hits (forced: `HIT_FLAG_FORCE`) pass.
pub fn hands_off_refuses(hit: &kh_core::HitRecord, targets: &dyn kh_core::host::Targets) -> bool {
    if hit.flags & kh_core::host::HIT_FLAG_FORCE != 0 {
        return false;
    }
    HANDS_OFF.with(|g| match &*g.borrow() {
        None => false,
        Some((zones, reserved, reach)) => reserved.contains(&hit.target) || targets.info(hit.target).is_some_and(|i| zones.iter().any(|zn| zn.depth(i.position).0 + reach + i.radius.max(0.0) > 0.0)),
    })
}

// ============================================================================================== the camera (state)

impl ZettaCam {
    fn feed(&mut self, f: CamFeed) {
        let key = Some((f.caster, f.begun));
        if self.key == key && self.retired {
            return;
        }
        if self.key != key {
            // a new Zettaflare: its own shots from its start
            *self = ZettaCam { shots: std::mem::take(&mut self.shots), weight: self.weight, last: self.last, last_safe: self.last_safe, ..ZettaCam::default() };
            self.key = key;
        }
        if !self.ended && !self.cut {
            self.feed = Some(f);
        }
    }

    /// The run it follows is over: `cut` = at once (interrupted, KO), else blend back from its last view.
    fn end(&mut self, caster: MemberId, begun: u32, cut: bool) {
        if self.key != Some((caster, begun)) {
            return;
        }
        if cut {
            self.cut = true;
        } else {
            self.ended = true;
        }
    }

    /// One camera step (see [`Party::don_zetta_view`]).
    pub fn step(&mut self, dt: f32, stick: f32, z: &ZettaTune) -> Option<ZettaView> {
        if self.cut {
            *self = ZettaCam { shots: std::mem::take(&mut self.shots), key: self.key, retired: true, ..ZettaCam::default() };
            return None;
        }
        let Some(f) = self.feed else { return None };
        if self.shots.is_empty() {
            self.shots = cam_shots();
        }
        let live = !self.ended && !self.skipped;
        if live {
            if stick >= z.zetta_cam_skip_stick {
                self.skip_t += dt;
                if self.skip_t >= z.zetta_cam_skip_hold {
                    self.skipped = true;
                }
            } else {
                self.skip_t = 0.0;
            }
        }
        let live = !self.ended && !self.skipped;
        // (ZETTA 2026-10-23) the host's pre-pose window of the target's heavy hurt ([`prepose_window`])
        PREPOSE.store(live && prepose_window(&self.shots, f.t, f.channel, z, f.donald, f.target, f.beam, f.subject, f.end), Ordering::Relaxed);
        if live {
            // (ZETTA 2026-10-23, the user: "Can the first shot of Donald charging up just be a cut from gameplay instead of a spin
            // around?") A HARD CUT IN: the first cinematic frame is the first shot's own (no blend from the gameplay camera;
            // `zetta.cam_blend_in` is no longer read); the hand-back's blend out stays
            let _ = z.zetta_cam_blend_in;
            self.weight = 1.0;
            self.last = cam_pose(&self.shots, f.t, f.channel, z, f.donald, f.target, f.beam, f.subject, f.end).or(self.last);
            self.last_safe = Some(safe_pose(f.donald.origin, f.beam.origin, z));
            // (round 5) the event's white at this point of the sequence
            let mt = matinee_time(f.t, z, f.channel);
            self.last_white = white_at(mt, z);
        } else {
            self.weight = if z.zetta_cam_blend_out > 1e-3 { (self.weight - dt / z.zetta_cam_blend_out).max(0.0) } else { 0.0 };
        }
        if self.weight <= 0.0 && !live {
            *self = ZettaCam { shots: std::mem::take(&mut self.shots), key: self.key, retired: true, ..ZettaCam::default() };
            return None;
        }
        let pose = self.last?;
        self.shot = Some(pose.shot);
        // eased (smoothstep): no jolt at either end of a blend
        let w = self.weight.clamp(0.0, 1.0);
        let eased = w * w * (3.0 - 2.0 * w);
        let safe = self.last_safe.unwrap_or(pose);
        let exact = pose.shot == PAN_SHOT || pose.shot == CAST_SHOT || END_SHOTS.contains(&pose.shot);
        // (round 5c) the target's shot shakes while it is in the beam
        let target_shot = self.shots.iter().any(|s| s.name == pose.shot && s.frame == "subject");
        let (shake_rot, shake_loc) = if live && target_shot { shake_at(matinee_time(f.t, z, f.channel), z) } else { (Vec3::ZERO, Vec3::ZERO) };
        Some(ZettaView { position: pose.position, rotation: pose.rotation, fov: pose.fov, pivot: pose.pivot, weight: eased, shot: pose.shot, safe_position: safe.position, safe_rotation: safe.rotation, safe_fov: safe.fov, safe_pivot: safe.pivot, block_share: z.zetta_cam_block_share.clamp(0.0, 1.0), exact, white: self.last_white * eased, handing_back: !live, shake_rot: shake_rot * eased, shake_loc: shake_loc * eased, beam_dir: f.beam.x })
    }

    /// The camera has the view (any share).
    pub fn active(&self) -> bool {
        self.feed.is_some() && !self.cut && !self.retired
    }

    /// (round 5) The camera still plays the shots of this run (not handed back, skipped or cut).
    pub fn live_for(&self, caster: MemberId, begun: u32) -> bool {
        self.key == Some((caster, begun)) && !self.retired && !self.skipped && !self.cut && !self.ended
    }
}

impl Party {
    /// The Zettaflare runs now (the debug text, the harness).
    pub fn don_zetta_runs(&self) -> &[ZettaRun] {
        &self.don_zetta.runs
    }

    /// (round 5) Donald in Zettaflare's sequence: invincible, never an enemy's target ([`invincible`]).
    pub fn don_zetta_invincible(&self, id: MemberId) -> bool {
        invincible(self, id)
    }

    /// (round 3) THE TARGETS ZETTAFLARE HOLDS (the user: "the allies you have target everybody else except for the
    /// zettaflare's target"): the friends' trees leave these alone (kh-partyhost's enemy feed makes them bystanders) from
    /// the channel's start to the hit, a cancel or an interrupt.
    pub fn don_zetta_reserved(&self) -> Vec<TargetId> {
        self.don_zetta.runs.iter().filter(|r| !r.done).filter_map(|r| r.held.or(r.reserved)).collect()
    }

    /// (round 3) THE ALLIES' ZONES now (the user: "allies should stay AWAY from the Zettaflare target"): the friends'
    /// movement (pawn `goal_point`, the idle step-out) and AI Sora's stick keep out of them while the target is held.
    pub fn don_zetta_zones(&self) -> Vec<AvoidZone> {
        self.don_zetta.runs.iter().filter(|r| !r.done).flat_map(|r| [r.zone, r.zone_donald]).flatten().collect()
    }

    /// (round 5c) THE SPAN THE ALLIES KEEP THEIR HANDS OFF (the user: "allies should not target anything close to Donald or
    /// the Zettaflare or the Zettaflare's expected path until after the player has regained control over Donald"): from the
    /// channel's start until the player has Donald back (the hand-off, an early hand-back, a cancel or an interrupt).
    pub fn don_zetta_span(&self) -> bool {
        self.don_zetta.runs.iter().any(|r| r.kind == RunKind::Zetta && !r.done && !r.handed)
    }

    /// (round 5c) An enemy at `at` (its feet) with this body radius is OFF LIMITS to the allies: within
    /// `zetta.ally_target_clear` (plus its radius) of a zone (Donald's, the target's, the beam's corridor from the channel's
    /// start, as wide as the beam's girth) during [`Party::don_zetta_span`]. No ally attacks, casts at, locks on, shoots or
    /// hits it; their spells, aimed only at enemies further out, never reach into a zone.
    pub fn don_zetta_off_limits(&self, at: Vec3, radius: f32) -> bool {
        if !self.don_zetta_span() {
            return false;
        }
        let reach = zetta().zetta_ally_target_clear.max(0.0) + radius.max(0.0);
        self.don_zetta_zones().iter().any(|zn| zn.depth(at).0 + reach > 0.0)
    }

    /// (round 5c) Every target the allies keep off now: Zettaflare's own ([`Party::don_zetta_reserved`]) and, through the
    /// span, every one of `targets` [`Party::don_zetta_off_limits`] (the hosts' enemy feeds, AI Sora's targeting and hits).
    pub fn don_zetta_off_limit_targets(&self, targets: &dyn kh_core::host::Targets) -> Vec<TargetId> {
        let mut out = self.don_zetta_reserved();
        if self.don_zetta_span() {
            let mut all = Vec::new();
            targets.enumerate(&mut all);
            for i in all {
                if !out.contains(&i.id) && self.don_zetta_off_limits(i.position, i.radius) {
                    out.push(i.id);
                }
            }
        }
        out
    }

    /// (round 5c) A shot's way from `from` to `to` keeps out of every zone (sampled every 50 cm; `radius` = the shot's
    /// own): true when clear or outside the span.
    pub fn don_zetta_path_clear(&self, from: Vec3, to: Vec3, radius: f32) -> bool {
        if !self.don_zetta_span() {
            return true;
        }
        let zones = self.don_zetta_zones();
        let n = ((to - from).length() / 50.0).ceil().max(1.0) as usize;
        (0..=n).all(|k| {
            let p = from.lerp(to, k as f32 / n as f32);
            zones.iter().all(|zn| zn.depth(p).0 + radius.max(0.0) <= 0.0)
        })
    }

    /// (round 5c) WHERE THE ALLIES WAIT (the user: "If there is only one target (and they're being targeted by or are in the
    /// zettaflare) then the allies move away and wait for when the player regains control of Donald"): spots
    /// `zetta.ally_wait_back` past the edge of Donald's zone, beside him and a little behind first (out of the shot behind
    /// him, 970, and of the beam's line), then further round; each out of every zone (with `zetta.ally_margin`). A fixed
    /// order (Donald's place and the beam's bearing only), so the spot holds still through the cuts. The host takes the first
    /// on its ground that it can reach. Empty outside the span.
    pub fn don_zetta_wait_spots(&self) -> Vec<Vec3> {
        let Some(run) = self.don_zetta.runs.iter().find(|r| r.kind == RunKind::Zetta && !r.done && !r.handed) else { return Vec::new() };
        let Some(dz) = run.zone_donald else { return Vec::new() };
        let z = zetta();
        let zones = self.don_zetta_zones();
        let ahead = run.beam_at_hit.map(|b| crate::pawn::horizontal(b.x)).or(run.zone.map(|t| crate::pawn::horizontal(t.to - dz.at))).unwrap_or(Vec3::X).normalize_or(Vec3::X);
        let dist = dz.radius + z.zetta_ally_margin.max(0.0) + z.zetta_ally_wait_back.max(0.0);
        let mut out = Vec::new();
        // (never straight behind him: the shot behind Donald, 970, looks along the beam from 15 m back)
        for deg in [100.0f32, -100.0, 80.0, -80.0, 125.0, -125.0, 145.0, -145.0] {
            let (s, c) = deg.to_radians().sin_cos();
            let dir = Vec3::new(ahead.x * c - ahead.y * s, ahead.x * s + ahead.y * c, 0.0);
            let p = dz.at + dir * dist;
            if zones.iter().all(|zn| zn.depth(p).0 + z.zetta_ally_margin.max(0.0) <= 0.0) {
                out.push(p);
            }
        }
        out
    }

    /// THE CINEMATIC CAMERA (both hosts, kh-partyhost `team_camera`, once a step after the field / team cameras): the
    /// event's view and its share (`weight`, 0..1) over the drawn one; `None` = the gameplay camera alone. `stick` = the
    /// camera stick's magnitude (0..1): held past `zetta.cam_skip_stick` for `zetta.cam_skip_hold` it hands the camera
    /// back (blend-out). Interrupted / KO: `None` at once.
    pub fn don_zetta_view(&mut self, dt: f32, stick: f32) -> Option<ZettaView> {
        let z = zetta();
        self.don_zetta.cam.step(dt, stick, &z)
    }
}

#[cfg(test)]
mod zetta_prepose_tests {
    use super::*;

    /// (2026-10-23, the user: "could the fast forward happen while Donald is charging so it is never actually visible?") the
    /// pre-pose window opens only in Donald's last shot before the hit (910) with the target out of its frame, never in the
    /// target's shots, never earlier than PREPOSE_LEAD_S before the hit
    #[test]
    fn zetta_the_pre_pose_lands_inside_a_shot_without_the_target() {
        let z = zetta();
        let shots = cam_shots();
        let channel = 5.0;
        let donald = Frame { origin: Vec3::ZERO, yaw: 0.0 };
        let goal = Vec3::new(700.0, 0.0, 90.0);
        let target = Frame { origin: Vec3::new(700.0, 0.0, 0.0), yaw: 180.0 };
        let beam = Basis::beam(Vec3::new(30.0, 0.0, 150.0), goal);
        let subject = CamSubject { at: goal, size: 1.0, animated: true, radius: 50.0, half: 90.0 };
        let hit = seq_time(z.zetta_hit_at, &z, channel);
        // every frame the window is open, from there to the hit: Donald's shot, the target out of frame (or the last frames
        // before the cut)
        let check = |subject: CamSubject| {
            let body = [subject.at, subject.at + Vec3::Z * subject.half, subject.at - Vec3::Z * subject.half];
            let mut opened = None;
            let mut t = hit - 3.0;
            while t < hit + 1.0 {
                if prepose_window(&shots, t, channel, &z, donald, target, beam, subject, None) {
                    opened.get_or_insert(t);
                    assert!(t < hit && hit - t <= PREPOSE_LEAD_S + 1e-4);
                    if hit - t > PREPOSE_CUT_S {
                        let mut u = t;
                        while u < hit {
                            let pose = cam_pose(&shots, u, channel, &z, donald, target, beam, subject, None).unwrap();
                            assert!(DONALD_SHOTS.contains(&pose.shot), "{u}: in Donald's shot ({})", pose.shot);
                            assert!(body.iter().all(|b| !frames_both(pose.position, pose.rotation, pose.fov, *b, *b, 1.0)), "{u}: never seen before the hit");
                            u += 1.0 / 60.0;
                        }
                    }
                }
                t += 1.0 / 60.0;
            }
            opened
        };
        // far out (20 m): 910 never frames it: the window opens PREPOSE_LEAD_S before the hit, inside Donald's shot
        let far = CamSubject { at: Vec3::new(2000.0, 0.0, 90.0), ..subject };
        let opened = check(far).expect("opens");
        assert!(hit - opened > 0.5, "early in the window ({} s before the hit)", hit - opened);
        let pose = cam_pose(&shots, opened, channel, &z, donald, target, beam, far, None).unwrap();
        assert!(DONALD_SHOTS.contains(&pose.shot) && !frames_both(pose.position, pose.rotation, pose.fov, far.at, far.at, 1.0), "inside a shot without the target");
        // 7 m: 910's end flies past it (its head at the frame's foot): only the last frames before the cut
        let opened = check(subject).expect("opens");
        assert!(hit - opened <= PREPOSE_CUT_S + 1e-4, "{} s before the hit", hit - opened);
        assert!(!prepose_window(&shots, hit + 0.01, channel, &z, donald, target, beam, subject, None), "never after the hit");
    }
}

#[cfg(test)]
mod tests {
    //! (ZETTA) the timing (the matinee's times mapped onto the sequence, frame-rate independent), the frames (the
    //! cutscene's own world places rebuilt exactly), the aim (lock-on, soft, given, ahead), a whole cast in a party; round
    //! 2: the 5 s channel, the camera (its shots, blends, the hand-back on an interrupt, the stick, off), the root, the
    //! face clip, the ultimates (3 s, tints, the release event, no camera).
    use super::*;
    use crate::ids::Chara;
    use crate::pad::PadFrame;
    use crate::{PartyConfig, SoraView};
    use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetInfo, Targets};
    use kh_core::input::{PadButton, PadButtons, PadState as Buttons};
    use kh_core::test_world::TestWorld;

    fn z() -> ZettaTune {
        DON_ZETTA_PARAMS
    }

    fn row(id: &str) -> &'static DonZettaFxRow {
        DON_ZETTA_FX.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("no row {id}"))
    }

    fn close(a: Vec3, b: Vec3, tol: f32) -> bool {
        (a - b).length() <= tol
    }

    #[test]
    fn the_matinee_times_map_onto_the_sequence() {
        let z = z();
        assert_eq!(z.zetta_channel_time, 5.0, "the user's 5 s");
        assert_eq!(z.ult_channel_time, 3.0, "the ultimates' 3 s");
        // the event's own length: a pure shift
        let full = z.zetta_cast_start - z.zetta_channel_start;
        for t in [58.767, 60.667, 63.5, 69.733, 72.033, 72.833, 79.633] {
            assert!((seq_time(t, &z, full) - (t - 58.767)).abs() < 1e-3, "{t}");
        }
        // the play channel (5 s): the channel's events keep their times (they end by 4.733 s), the cast follows it
        assert_eq!(seq_time(58.767, &z, 5.0), 0.0);
        assert!((seq_time(60.667, &z, 5.0) - 1.9).abs() < 1e-3, "the strong aura at 1.9 s");
        assert!((seq_time(63.5, &z, 5.0) - 4.733).abs() < 1e-3, "the hand light ends in the channel");
        assert!((seq_time(69.733, &z, 5.0) - 5.0).abs() < 1e-3, "the cast starts when the channel ends");
        assert!((seq_time(72.033, &z, 5.0) - 7.3).abs() < 1e-3, "the release 2.3 s into the cast clip");
        assert!((seq_time(72.833, &z, 5.0) - 8.1).abs() < 1e-3, "the beam / hit as the cast clip ends");
        assert!((seq_time(79.633, &z, 5.0) - 14.9).abs() < 1e-3, "the end with the white fade");
        // the ultimates' 3 s channel: the channel's events compressed into it
        let k = 3.0 / 4.733;
        assert!((seq_time(60.667, &z, 3.0) - 1.9 * k).abs() < 1e-3, "the aura compressed");
        assert!((seq_time(63.5, &z, 3.0) - 3.0).abs() < 1e-3, "the hand light ends with the channel");
        assert!((seq_time(72.033, &z, 3.0) - 5.3).abs() < 1e-3, "the cast part unscaled");
        // the inverse
        for ch in [3.0, 5.0] {
            for s in [0.5, 1.2, 2.9, 3.0, 4.0, 6.1, 12.9] {
                assert!((seq_time(matinee_time(s, &z, ch), &z, ch) - s).abs() < 1e-3, "{ch} {s}");
            }
        }
    }

    #[test]
    fn the_schedule_is_the_events_rows() {
        let z = z();
        let s = schedule(&z, 5.0, RunKind::Zetta);
        let at = |id: &str| s.iter().find(|x| DON_ZETTA_FX[x.0].id == id).copied().unwrap_or_else(|| panic!("{id} not scheduled"));
        // the circle and its light from the first frame to the beam (the event: OnActive 58.767, OnDective 72.833)
        assert_eq!(at("fx.world.86_zfr_01").1, 0.0);
        assert!((at("fx.world.86_zfr_01").2 - 8.1).abs() < 1e-3);
        assert!((at("fx.world.86_rit_00").2 - 8.1).abs() < 1e-3);
        // the cast clip at the channel's end, the staff charge with it, "Zettaflare!" 1/3 s in
        assert!((at("clip.cast").1 - 5.0).abs() < 1e-3);
        assert!((at("fx.donald.91_zfr_01").1 - 5.0).abs() < 1e-3);
        assert!((at("voice.kg7020250do0").1 - 5.334).abs() < 1e-3);
        // the channel SE (the event's, compressed to 5 s) ends with the channel; the cast SE runs out by itself
        assert!((at("se.channel").2 - 5.0).abs() < 1e-3);
        assert!(DON_ZETTA_FX[at("se.channel").0].asset.ends_with("_zetta_channel5"));
        assert!(at("se.cast").2.is_infinite());
        // the camera cheats of the reaction shots are not played
        assert!(s.iter().all(|x| !DON_ZETTA_FX[x.0].asset.starts_with("vd_kg702_88_")));
        // every effect name is one of the packed event effects
        for x in &s {
            let r = &DON_ZETTA_FX[x.0];
            if r.kind == "fx" {
                // (round 5c) or ZETTA's own from KH3 parts (tools/make_zetta_glow.js)
                assert!(r.asset.starts_with("vd_kg702_") || r.asset.starts_with("vd_kg702b_") || r.asset.starts_with("vd_zetta_"), "{}", r.asset);
            }
        }
        assert!(DON_ZETTA_FX.len() < 128, "the fired mask holds every row");
    }

    #[test]
    fn the_ultimates_play_the_circle_and_the_clips_only() {
        let z = z();
        let kind = RunKind::Ult { tint: Tint::rgb(1.0, 0.0, 0.0) };
        let s = schedule(&z, kind.channel(&z), kind);
        let ids: Vec<&str> = s.iter().map(|x| DON_ZETTA_FX[x.0].id).collect();
        for want in ["clip.channel", "clip.cast", "fx.world.86_zfr_01", "fx.world.86_rit_00", "fx.donald.86_zfr_02", "fx.donald.86_zfr_03", "fx.donald.86_zfr_04", "fx.donald.91_rit_01", "fx.donald.91_zfr_01", "fx.world.91_rit_00", "fx.world.91_zfr_00", "voice.kg7020241do0"] {
            assert!(ids.contains(&want), "{want} missing");
        }
        for not in ["se.channel", "se.cast", "voice.kg7020250do0", "fx.world.93_zfr_00", "fx.target.92_zfr_02", "fx.world.94_exp_00"] {
            assert!(!ids.contains(&not), "{not} is Zettaflare's");
        }
        // the whole cast scaled: the release at ult.channel_time (3 s), the end with the cast clip's end
        let k = kind.scale(&z);
        assert!((kind.seq(z.zetta_release_at, &z, kind.channel(&z)) - 3.0).abs() < 1e-4);
        assert!((kind.end(&z, kind.channel(&z)) - (4.733 + 3.1) * k).abs() < 1e-3);
        assert!(s.iter().all(|x| x.1 <= 3.0 + 1e-3 || DON_ZETTA_FX[x.0].start > 69.7), "every channel row starts before the release");
    }

    #[test]
    fn the_tints_are_the_elements_colours_at_full_brightness() {
        let z = z();
        for e in Element::ALL {
            let t = tint_of(element_color(e), &z);
            let m = t.color[0].max(t.color[1]).max(t.color[2]);
            assert!((m - 1.0).abs() < 1e-5, "{e:?}: brightness kept");
            assert_eq!(t.intensity, z.ult_tint_intensity);
        }
        let fire = tint_of(element_color(Element::Fire), &z).color;
        assert!(fire[0] > fire[1] && fire[1] > fire[2], "red-orange {fire:?}");
        let ice = tint_of(element_color(Element::Blizzard), &z).color;
        assert!(ice[2] >= ice[1] && ice[1] > ice[0], "ice blue {ice:?}");
        let th = tint_of(element_color(Element::Thunder), &z).color;
        assert!(th[0] > 0.9 && th[1] > 0.8 && th[2] < 0.4, "yellow {th:?}");
        let aero = tint_of(element_color(Element::Aero), &z).color;
        assert!(aero[1] > aero[0] && aero[1] > aero[2], "green {aero:?}");
        let water = tint_of(element_color(Element::Water), &z).color;
        assert!(water[2] > water[1] && water[1] > water[0], "aqua blue {water:?}");
        let light = tint_of(element_color(Element::Light), &z).color;
        assert!(light[0] > 0.95 && light[1] > 0.9 && light[2] > 0.7, "gold-white {light:?}");
    }

    /// The event's frames (make_zetta_sheets.js FR): Donald's feet and facing, Terra-Xehanort's centre, the staff.
    fn event_frames() -> (Frame, Vec3, Basis) {
        let don = Frame { origin: Vec3::new(1279.0072, -37333.105, -16.567169), yaw: -57.22063 };
        let xeh = Vec3::new(1749.5466, -38011.043, 126.27189 + 90.0);
        let staff = don.point(staff_local(&z()));
        (don, xeh, Basis::beam(staff, xeh))
    }

    #[test]
    fn the_rotations_and_the_beam_frame_are_ues() {
        // a rotator survives its axes; the beam frame's X runs from the staff to the target in 3D, Y stays level
        for r in [Vec3::new(10.0, 20.0, 30.0), Vec3::new(-45.0, 170.0, -80.0), Vec3::new(80.0, -100.0, 5.0), Vec3::ZERO] {
            let [x, y, zz] = rot_axes(r);
            let back = axes_rot(x, y, zz);
            assert!((back - r).length() < 1e-2, "{r} -> {back}");
            assert!((x.cross(y) - zz).length() < 1e-4, "right-handed in UE's sense: X x Y = Z");
        }
        for (s0, t0) in [(Vec3::ZERO, Vec3::new(800.0, 0.0, 0.0)), (Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 600.0, 500.0)), (Vec3::new(50.0, 50.0, 300.0), Vec3::new(-900.0, -200.0, 0.0))] {
            let b = Basis::beam(s0, t0);
            assert!((b.x - (t0 - s0).normalize()).length() < 1e-5);
            assert!(b.y.z.abs() < 1e-5 && b.z.z > 0.0, "level Y, Z up");
            assert!((b.point(b.local(Vec3::new(3.0, 4.0, 5.0))) - Vec3::new(3.0, 4.0, 5.0)).length() < 1e-3);
            // a rotator of the frame's own axes is the frame
            let r = b.rotation(Vec3::ZERO);
            let [x, y, zz] = rot_axes(r);
            assert!((x - b.x).length() < 1e-4 && (y - b.y).length() < 1e-4 && (zz - b.z).length() < 1e-4);
        }
        // the event's beam frame: 7.5 m from the staff to Terra-Xehanort's centre, 10.9 deg up (he stood on a rise)
        let (_, xeh, b) = event_frames();
        assert!(((xeh - (b.origin - b.x * 748.33)).length()) > 0.0);
        assert!((b.x.z.asin().to_degrees() - 10.879).abs() < 0.05, "{}", b.x.z.asin().to_degrees());
    }

    #[test]
    fn the_beams_are_snapped_onto_the_line_and_the_impacts_sit_on_the_target() {
        // (round 5) every card of the beam is laid along the line FROM THE TIP (frame beam_stream: 92's cards too); the world
        // copies of 93 / 94 are retired; the "guard" shocks and the far glow sit on the target
        for id in ["fx.world.92_zfr_01", "fx.world.92_zfr_03"] {
            let r = row(id);
            assert_eq!(r.frame, "beam_stream", "{id}");
            assert_eq!(r.path.len(), 1, "{id}: its half length");
        }
        for id in ["fx.world.93_zfr_00", "fx.world.94_zfr_01"] {
            assert_eq!(row(id).use_, 0, "{id}: retired (its stream copies are the beam)");
        }
        for id in ["fx.target.92_zfr_02", "fx.target.92_zfr_02_b", "fx.target.94_zfr_00"] {
            assert_eq!(row(id).frame, "beam_socket");
            assert!(v3(row(id).offset).length() < 250.0, "{id} on the target");
        }
        assert_eq!(v3(row("fx.world.94_exp_00").offset), Vec3::ZERO, "the impact glow on the target, not 17.8 m past it");
        assert!(row("fx.world.94_exp_00").scale < 0.5);
        // the beam: two crossed copies of the c93 card from the release, of the c94 card for the blast, 92's two cards
        let streams: Vec<&DonZettaFxRow> = DON_ZETTA_FX.iter().filter(|r| r.frame == "beam_stream" && r.use_ == 1).collect();
        assert_eq!(streams.len(), 6);
        assert!(streams.iter().any(|r| (r.start - z().zetta_release_at).abs() < 1e-3));
    }

    #[test]
    fn the_launch_comes_out_of_the_wand_tip_and_its_head_runs_to_the_target() {
        // (round 5c) the user: "the beam has to look like it comes from Donald's wand tip" (in the shot where it first appears)
        // and "right now it just appears suddenly": up to the hit the beam starts at his wand tip as the host draws it (fed),
        // within 10 cm, nothing of it behind the tip; the tip gathers light before the release and bursts at it (on his wand
        // socket); a head runs from the tip to the aim point over zetta.launch_travel, the beam never ahead of it by more than
        // its part past the aim point, thin behind it and swelling; it bursts again where it arrives
        let z = z();
        feed_tip(200, Some(Vec3::new(12.0, 34.0, 156.0)));
        assert_eq!(tip_of(MemberId(200)), Some(Vec3::new(12.0, 34.0, 156.0)));
        assert_eq!(tip_of(MemberId(201)), None, "the fed member only");
        feed_tip(200, None);
        let tip = Vec3::new(100.0, -20.0, 140.0);
        let goal = Vec3::new(1300.0, 200.0, 90.0);
        let b = Basis::beam(tip, goal);
        let dist = (goal - tip).length();
        for id in ["fx.beam.stream_93_a", "fx.beam.stream_93_b"] {
            let r = DON_ZETTA_FX.iter().find(|r| r.id == id).unwrap();
            let half = r.path[0];
            let mut last = 0.0;
            for n in 0..=48 {
                let mt = z.zetta_release_at + n as f32 / 60.0;
                let (o, rot, sc) = stream_place_facing(r, tip, b, mt, &z, 1.0, Some(Vec3::new(-300.0, -200.0, 200.0)));
                let [x, y, zz] = rot_axes(rot);
                let e0 = o + x * (v3(r.offset).x * sc.x) + y * (v3(r.offset).y * sc.y) + zz * (v3(r.offset).z * sc.z);
                assert!((e0 - tip).length() < 10.0, "{id} {mt}: its start on the tip ({e0})");
                assert!((zz - b.x).length() < 1e-3, "{id}: nothing behind the tip (along the line, forward)");
                let front = 2.0 * half * sc.z;
                let head = (launch_head(tip, b, mt, &z) - tip).length();
                assert!(front + 1.0 >= last && front <= head + z.zetta_beam_beyond + 1.0, "{id} {mt}: with its head ({front} / {head})");
                last = front;
            }
        }
        assert!((launch_head(tip, b, z.zetta_release_at, &z) - tip).length() < 1e-3, "the head leaves the tip");
        assert!((launch_head(tip, b, z.zetta_release_at + z.zetta_launch_travel, &z) - goal).length() < 1e-3, "and arrives at the aim point");
        let mid = (launch_head(tip, b, z.zetta_release_at + z.zetta_launch_travel * 0.5, &z) - tip).length();
        assert!((mid - dist * 0.5).abs() < 1.0, "a readable run, not a pop ({mid} of {dist})");
        assert!(z.zetta_launch_travel >= 0.25 && z.zetta_launch_travel <= 0.4);
        let row = |id: &str| DON_ZETTA_FX.iter().find(|r| r.id == id).unwrap();
        assert!(row("fx.launch.charge").frame == "weapon" && row("fx.launch.charge").socket == "vfx_call" && row("fx.launch.charge").end <= z.zetta_release_at + 1e-3 && z.zetta_release_at - row("fx.launch.charge").start >= 0.3, "the tip gathers light before the release");
        assert!(row("fx.launch.burst").frame == "weapon" && (row("fx.launch.burst").start - z.zetta_release_at).abs() < 1e-3, "it bursts at the tip at the release");
        assert!(row("fx.launch.head").frame == "beam_head" && (row("fx.launch.impact").start - (z.zetta_release_at + z.zetta_launch_travel)).abs() < 2e-3, "the head, its arrival");
        let (kick, _) = shake_at(z.zetta_release_at + 0.02, &z);
        assert!(kick.length() > 0.2 && shake_at(z.zetta_release_at + z.zetta_launch_kick_time + 0.05, &z).0.length() < 1e-6, "a quick kick at the burst");
    }

    #[test]
    fn the_beam_runs_from_the_staff_to_the_target_wherever_it_is() {
        // THE AIM MATHS: the beam card starts AT THE TIP (any direction, height, distance); (round 5) it launches: nothing at
        // the release, growing out of the tip (ease out), reaching the target at the hit and running zetta.beam_beyond past
        // it; each card builds from zetta.beam_width0 of its width and thins to nothing at its end (no pop in or out)
        let z = z();
        let staff = Vec3::new(100.0, -20.0, 140.0);
        for id in ["fx.beam.stream_93_a", "fx.world.92_zfr_01"] {
            let r = DON_ZETTA_FX.iter().find(|r| r.id == id).unwrap();
            let half = r.path[0];
            for target in [Vec3::new(900.0, 0.0, 90.0), Vec3::new(100.0, 700.0, 90.0), Vec3::new(500.0, 300.0, 600.0), Vec3::new(-400.0, -600.0, -300.0), Vec3::new(4000.0, 1000.0, 90.0), Vec3::new(250.0, 0.0, 150.0)] {
                let b = Basis::beam(staff, target);
                let full = (target - staff).length() + z.zetta_beam_beyond;
                let mid = 0.5 * (z.zetta_release_at + z.zetta_hit_at);
                for (mt, share) in [(z.zetta_hit_at, 1.0), (mid, ease_out(0.5)), (z.zetta_hit_at + 0.5, 1.0)] {
                    let mt = mt.max(r.start);
                    let (o, rot, sc) = stream_place(r, staff, b, mt, &z);
                    let [x, y, zz] = rot_axes(rot);
                    // (round 5c) the effect's beam axis (local +Z, its cards from local z offset.z forward) on the line
                    assert!((zz - b.x).length() < 1e-4, "laid along the line");
                    let e0 = o + x * (v3(r.offset).x * sc.x) + y * (v3(r.offset).y * sc.y) + zz * (v3(r.offset).z * sc.z);
                    let (c, e1) = (e0 + zz * half * sc.z, e0 + zz * 2.0 * half * sc.z);
                    // (round 5c) fired: as far as its head (launch_reach), its part past the aim point last
                    let _ = share;
                    let reach = launch_reach(mt, &z);
                    let dist = (target - staff).length();
                    let len = dist * reach + (full - dist) * reach.powi(4);
                    assert!((e0 - staff).length() < 1.0, "{id} {target} {mt}: from the tip ({e0} vs {staff})");
                    assert!((e1 - (staff + b.x * len)).length() < 1.0, "{id} {target} {mt}: its front ({e1} vs {})", staff + b.x * len);
                    assert!(b.local(c).y.abs() < 1.0 && b.local(c).z.abs() < 1.0, "{id}: its centre on the line");
                }
                // it launches: thin and short as it starts, whole once built, thin again as it ends
                let (_, _, s0) = stream_place(r, staff, b, r.start, &z);
                assert!((s0.y - z.zetta_beam_width0).abs() < 1e-3, "{id}: starts thin ({})", s0.y);
                // (round 5c) the launch's cards swell behind the head: over its travel and their own build
                let build = z.zetta_beam_card_in + if (r.start - z.zetta_release_at).abs() < 1e-3 { z.zetta_launch_travel } else { 0.0 };
                let (_, _, s1) = stream_place(r, staff, b, r.start + build + 0.01, &z);
                assert!(s1.y > 0.99, "{id}: built");
                let (_, _, s2) = stream_place(r, staff, b, r.end - 0.001, &z);
                assert!(s2.y < 0.02, "{id}: gone at its end ({})", s2.y);
                if (r.start - z.zetta_release_at).abs() < 1e-3 {
                    assert!(s0.z * 2.0 * half <= 1.5, "nothing at the release");
                }
            }
        }
    }

    #[test]
    fn the_camera_frames_the_target_in_every_shot_wherever_it_is() {
        // the target ahead, to the side, above on a ledge, below, far, very close: every frame of every shot shows the
        // target; Donald's shots show Donald too; the last shot shows the beam reaching the target
        let z = z();
        let shots = cam_shots();
        let feet = Vec3::ZERO;
        for (name, goal) in [("ahead", Vec3::new(800.0, 0.0, 90.0)), ("side", Vec3::new(0.0, 900.0, 90.0)), ("ledge", Vec3::new(700.0, 300.0, 500.0)), ("below", Vec3::new(600.0, -200.0, -350.0)), ("far", Vec3::new(3500.0, 800.0, 90.0)), ("close", Vec3::new(180.0, 40.0, 90.0))] {
            let yaw = crate::pawn::yaw_of(crate::pawn::horizontal(goal - feet));
            let donald = Frame { origin: feet, yaw: yaw + EVENT_DONALD_YAW_OFF };
            let target = Frame { origin: goal, yaw };
            let beam = Basis::beam(donald.point(staff_local(&z)), goal);
            let chest = feet + Vec3::Z * 60.0;
            let mut seen = Vec::new();
            let mut s = 0.0;
            while s < 14.9 {
                let p = cam_pose(&shots, s, 5.0, &z, donald, target, beam, CamSubject { at: goal, size: 1.0, animated: true , radius: 50.0, half: 0.0 }, None).unwrap();
                if s < 5.0 {
                    // the channel: the event's charging close-up exactly (time-scaled), on Donald
                    assert_eq!(p.shot, PAN_SHOT);
                    let pan = shots.iter().find(|x| x.name == PAN_SHOT).unwrap();
                    let (rp, rr, rf) = pan.at(pan.start() + (pan.stop() - pan.start()) * s / 5.0);
                    assert!((p.position - donald.point(rp)).length() < 1e-2 && (p.rotation - donald.rotation(rr)).length() < 1e-2 && (p.fov - rf).abs() < 1e-4);
                    let hips = feet + Vec3::Z * 50.0;
                    assert!(frames_both(p.position, p.rotation, p.fov, hips, hips, 1.0), "{name} {s}: Donald in the close-up");
                    s += 0.1;
                    continue;
                }
                if p.shot == CAST_SHOT {
                    // the wand raise: the event's camera exactly (cast part 1:1)
                    let cast = shots.iter().find(|x| x.name == CAST_SHOT).unwrap();
                    let (rp, rr, rf) = cast.at(z.zetta_cast_start + (s - 5.0));
                    assert!((p.position - donald.point(rp)).length() < 1e-2 && (p.rotation - donald.rotation(rr)).length() < 1e-2 && (p.fov - rf).abs() < 1e-4, "{name} {s}");
                    s += 0.1;
                    continue;
                }
                assert!(p.fov <= z.zetta_cam_fov_max + 1e-3 && p.fov >= 10.0, "{name} {s}: fov {}", p.fov);
                assert!(frames_both(p.position, p.rotation, p.fov, goal, goal, 1.0), "{name} s {s:.1} {}: the target out of frame ({:?})", p.shot, view_angles(p.position, p.rotation, goal));
                if p.shot == "shot860" || p.shot == "shot890" || p.shot == "shot910" || p.shot == "safe" {
                    assert!(frames_both(p.position, p.rotation, p.fov, chest, goal, 1.0), "{name} s {s:.1} {}: Donald and the target", p.shot);
                }
                if !seen.contains(&p.shot) {
                    seen.push(p.shot);
                }
                s += 0.1;
            }
            assert!(seen.contains(&"shot920") && !seen.contains(&"shot930") && !seen.contains(&"shot940"), "{name}: one continuous target shot (round 5): {seen:?}");
            let last = cam_pose(&shots, 14.8, 5.0, &z, donald, target, beam, CamSubject { at: goal, size: 1.0, animated: true , radius: 50.0, half: 0.0 }, None).unwrap();
            assert!(frames_both(last.position, last.rotation, last.fov, goal, goal, 1.0), "{name}: the last shot shows the target in the beam");
        }
        // in the event's own layout the close-up is the event's camera itself (the GIF's frames)
        let (don, xeh, b) = event_frames();
        let tf = Frame { origin: xeh, yaw: don.yaw - EVENT_DONALD_YAW_OFF };
        let p = cam_pose(&shots, 2.5, 5.0, &z, don, tf, b, CamSubject { at: xeh, size: 1.0, animated: true , radius: 50.0, half: 0.0 }, None).unwrap();
        let raw = shots.iter().find(|x| x.name == PAN_SHOT).unwrap().at(58.767 + 1.5);
        assert!((p.position - don.point(raw.0)).length() < 0.1 && (p.fov - raw.2).abs() < 1e-3, "the close-up as the event, time-scaled 3 s -> 5 s");
    }

    #[test]
    fn the_aim_is_lock_on_then_soft_then_given_then_ahead() {
        let alive = |t: TargetId| t.0 == 1 || t.0 == 2 || t.0 == 3;
        let ahead = Vec3::new(800.0, 0.0, 100.0);
        assert_eq!(pick_aim(TargetId(1), TargetId(2), CastTarget::Enemy(TargetId(3)), &alive, ahead), ZettaAim::Target(TargetId(1)));
        assert_eq!(pick_aim(TargetId::NONE, TargetId(2), CastTarget::Enemy(TargetId(3)), &alive, ahead), ZettaAim::Target(TargetId(2)));
        assert_eq!(pick_aim(TargetId(9), TargetId(2), CastTarget::Enemy(TargetId(3)), &alive, ahead), ZettaAim::Target(TargetId(2)), "a dead lock-on is skipped");
        assert_eq!(pick_aim(TargetId::NONE, TargetId::NONE, CastTarget::Enemy(TargetId(3)), &alive, ahead), ZettaAim::Target(TargetId(3)));
        assert_eq!(pick_aim(TargetId::NONE, TargetId::NONE, CastTarget::Point(Vec3::ONE), &alive, ahead), ZettaAim::Point(ahead), "the caller's point: ahead of him");
        assert_eq!(pick_aim(TargetId::NONE, TargetId::NONE, CastTarget::Member(MemberId(1)), &alive, ahead), ZettaAim::Point(ahead));
    }

    // ------------------------------------------------------------------------------------ a whole cast in a party

    /// (2026-10-23) the Sora case's drawn (radius, half height): the body that fills the reference's region (fit 1)
    fn sora_case() -> (f32, f32) {
        (DON_ZETTA_PARAMS.zetta_frame_body_width * 0.5, DON_ZETTA_PARAMS.zetta_frame_body_height * 0.5)
    }

    #[derive(Default)]
    struct Dummies {
        list: Vec<(TargetId, Vec3)>,
        hits: Vec<HitRecord>,
        holds: Vec<(TargetId, Option<kh_core::host::ScriptHold>)>,
        /// (round 4) the targets' hit points (one for all; hits take them), the finales asked
        hp: f32,
        finales: Vec<(TargetId, Option<kh_core::host::ScriptFinale>)>,
        /// (round 5) the target's size (0: the man-sized 90 / 50)
        half: f32,
        radius: f32,
    }

    impl Dummies {
        fn info_of(&self, id: TargetId, pos: Vec3) -> TargetInfo {
            let flags = TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY;
            let flags = if self.hp > 0.0 { flags } else { flags & !TargetFlags::ALIVE };
            TargetInfo { id, position: pos, center: pos + Vec3::Z * if self.half > 0.0 { self.half } else { 90.0 }, velocity: Vec3::ZERO, yaw: 180.0, radius: if self.radius > 0.0 { self.radius } else { 50.0 }, half_height: if self.half > 0.0 { self.half } else { 90.0 }, flags: TargetFlags(flags), hp: self.hp, hp_max: 1.0e6 }
        }
    }

    impl Targets for Dummies {
        /// (2026-10-23) the default target draws as the test level's Sora does in his hurt pose (the reference's body: the fit
        /// 1); a sized one (`half` / `radius` set) as its capsule
        fn drawn_extent(&self, _id: TargetId) -> Option<(f32, f32)> {
            (self.half <= 0.0 && self.radius <= 0.0).then(|| sora_case())
        }

        fn enumerate(&self, out: &mut Vec<TargetInfo>) {
            for (id, p) in &self.list {
                out.push(self.info_of(*id, *p));
            }
        }
        fn info(&self, id: TargetId) -> Option<TargetInfo> {
            self.list.iter().find(|d| d.0 == id).map(|d| self.info_of(d.0, d.1))
        }
        fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
            if let Some(i) = self.info(id) {
                out.push(HurtShape { a: i.position + Vec3::Z * 30.0, b: i.position + Vec3::Z * 150.0, radius: 60.0, part: 0 });
            }
        }
        fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
            self.hits.push(*hit);
            self.hp -= hit.damage;
            HitResult { accepted: true, damage_dealt: hit.damage, hp_after: self.hp.max(0.0), killed: self.hp <= 0.0, guarded: false }
        }
        fn script_finale(&mut self, id: TargetId, finale: Option<kh_core::host::ScriptFinale>) {
            self.finales.push((id, finale));
        }
        fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
            Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
        }
        fn size_class(&self, _id: TargetId) -> u8 {
            1
        }
        fn script_hold(&mut self, id: TargetId, hold: Option<kh_core::host::ScriptHold>) {
            self.holds.push((id, hold));
        }
    }

    type Spawned = (u32, &'static str, Attach, Vec3, bool);

    #[derive(Default)]
    struct Rec {
        spawned: Vec<Spawned>,
        stopped: Vec<u32>,
        placed: Vec<(u32, Vec3)>,
        tinted: Vec<(u32, Tint)>,
        transformed: Vec<(u32, Vec3, Vec3, Vec3)>,
        /// (2026-10-23) each spawn's scale
        scales: Vec<(u32, f32)>,
    }

    impl kh_core::host::Effects for Rec {
        fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
            self.spawned.push((handle.0, spawn.name, spawn.at, spawn.rotation, spawn.persistent));
            self.scales.push((handle.0, spawn.scale));
        }
        fn tint(&mut self, handle: EffectHandle, tint: Tint) {
            self.tinted.push((handle.0, tint));
        }
        fn stop(&mut self, handle: EffectHandle, _immediate: bool) {
            self.stopped.push(handle.0);
        }
        fn place(&mut self, handle: EffectHandle, at: Vec3) {
            self.placed.push((handle.0, at));
        }
        fn transform(&mut self, handle: EffectHandle, at: Vec3, rotation: Vec3, scale: Vec3) {
            self.transformed.push((handle.0, at, rotation, scale));
        }
    }

    #[derive(Default)]
    struct RecAudio {
        played: Vec<&'static str>,
        /// (round 5c) every stop: the handle and its fade-out
        stopped: Vec<(u32, f32)>,
        handles: Vec<(u32, &'static str)>,
    }

    impl kh_core::host::Audio for RecAudio {
        fn play(&mut self, handle: SoundHandle, play: &SoundPlay) {
            self.played.push(play.name);
            self.handles.push((handle.0, play.name));
        }
        fn stop(&mut self, handle: SoundHandle, fade_out: f32) {
            self.stopped.push((handle.0, fade_out));
        }
    }

    /// The module's globals (`set_voice_ja`, `set_cinematic`) are shared by the tests: a rig holds this lock.
    static GLOBALS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct Rig {
        _globals: std::sync::MutexGuard<'static, ()>,
        world: TestWorld,
        dummies: Dummies,
        effects: Rec,
        audio: RecAudio,
        party: Party,
        sora: SoraView,
        don: MemberId,
        dt: f32,
        held: PadButtons,
    }

    impl Rig {
        fn new(enemy: Vec3, dt: f32) -> Rig {
            let mut party = Party::new(PartyConfig::test_level99());
            let don = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
            party.set_pad(don, Some(kh_core::Tuning::USER));
            party.member_mut(don).unwrap().in_battle = true;
            let mut sora = SoraView::default();
            sora.position = Vec3::new(-3000.0, 0.0, 0.0);
            sora.hp_max = 100;
            sora.hp = 100;
            let g = GLOBALS.lock().unwrap_or_else(|e| e.into_inner());
            set_voice_ja(false);
            set_cinematic(None);
            set_ending(None);
            let mut rig = Rig { _globals: g, world: TestWorld::floor(), dummies: Dummies { list: vec![(TargetId(1), enemy)], hits: Vec::new(), holds: Vec::new(), hp: 1.0e6, finales: Vec::new(), half: 0.0, radius: 0.0 }, effects: Rec::default(), audio: RecAudio::default(), party, sora, don, dt, held: PadButtons::NONE };
            for _ in 0..(0.5 / dt) as usize {
                rig.step();
            }
            rig
        }

        fn step(&mut self) {
            let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
            self.party.step(self.dt, &self.sora, &mut host);
        }

        /// A step with the pad: the stick and the buttons held.
        fn step_pad(&mut self, stick: Vec2, held: &[PadButton]) {
            let mut now = PadButtons::NONE;
            for b in held {
                now.set(*b, true);
            }
            let pad = Buttons { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
            self.held = now;
            self.party.set_pad_frame(self.don, PadFrame { stick, camera_yaw: 0.0, pad });
            self.step();
        }

        fn cast(&mut self) {
            assert_eq!(debug_make_ready(&mut self.party), 1);
            self.party.don_fx.debug_casts.push(SpellRef::Arcane(ArcaneLine::Zettaflare, 1));
            self.party.don_zetta.take_log();
            self.step();
        }

        fn run_for(&mut self, seconds: f32) {
            for _ in 0..(seconds / self.dt).round() as usize {
                self.step();
            }
        }

        fn time_of(&self, f: impl Fn(&ZettaLog) -> bool) -> Option<f32> {
            self.party.don_zetta.log.iter().find(|e| f(e)).map(|e| match e {
                ZettaLog::Reaction { t, .. } | ZettaLog::Blast { t, .. } | ZettaLog::Clip { t, .. } | ZettaLog::Face { t, .. } | ZettaLog::Spawn { t, .. } | ZettaLog::Stop { t, .. } | ZettaLog::Sound { t, .. } | ZettaLog::Released { t } | ZettaLog::Hit { t, .. } | ZettaLog::Interrupted { t } | ZettaLog::Ended { t } => *t,
                _ => 0.0,
            })
        }

        fn spawn_of(&self, id: &str) -> Option<(f32, Vec3, Vec3)> {
            self.party.don_zetta.log.iter().find_map(|e| match e {
                ZettaLog::Spawn { row, t, at, rotation, .. } if *row == id => Some((*t, *at, *rotation)),
                _ => None,
            })
        }
    }

    #[test]
    fn a_whole_zettaflare_plays_the_events_sequence_at_the_target() {
        let enemy = Vec3::new(600.0, 400.0, 0.0);
        let mut rig = Rig::new(enemy, 1.0 / 60.0);
        set_ending(Some(false)); // round 3 and 4 as they were (the ending: its own tests)
        assert_eq!(debug_make_ready(&mut rig.party), 1);
        let mp_before = rig.party.status(rig.don).unwrap().mp;
        rig.cast();
        // begun by the cast.rs hook: this module's channel clip, the whole MP spent (DON1's rule for Zettaflare)
        assert!(rig.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::Begun { aim: ZettaAim::Target(TargetId(1)), .. })), "{:?}", rig.party.don_zetta.log.first());
        assert!(!rig.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::TakenOver { .. })), "the cast.rs hook began it");
        assert_eq!(rig.party.member(rig.don).unwrap().anim.clip.sequence, "n_ex001_70286wr");
        {
            // the cast rule priced it (not the debug route's free cast): (2026-10-23) ALL his current MP spent, his MP regen
            // slowed until it has recharged, Arcane Flow emptied
            let s = rig.party.status(rig.don).unwrap();
            assert!(mp_before > 0 && s.mp == 0 && s.mp_charge_left > 0.0, "Zettaflare's price was paid: all his MP ({mp_before} -> {}), his MP Charge running", s.mp);
            assert!(s.donald.as_ref().unwrap().core.zetta_slow, "his MP regen slowed");
            assert_eq!(s.donald.as_ref().unwrap().core.arcane.charges(), 0, "Arcane Flow emptied");
        }
        rig.run_for(16.0);
        let tol = 1.5 / 60.0;
        let at = |rig: &Rig, f: &dyn Fn(&ZettaLog) -> bool| rig.time_of(f).expect("event");
        assert!(at(&rig, &|e| matches!(e, ZettaLog::Spawn { row: "fx.world.86_zfr_01", .. })) < tol, "the circle at once");
        assert!((at(&rig, &|e| matches!(e, ZettaLog::Spawn { row: "fx.donald.86_zfr_03", .. })) - 1.9).abs() < tol, "the strong aura at 1.9 s");
        assert!((at(&rig, &|e| matches!(e, ZettaLog::Clip { name: "n_ex001_70291w", .. })) - 5.0).abs() < tol, "the cast clip at 5 s");
        assert!((at(&rig, &|e| matches!(e, ZettaLog::Face { name: FACE_EN, .. })) - 5.0).abs() < tol, "his lip track with it (en voice)");
        assert!((at(&rig, &|e| matches!(e, ZettaLog::Released { .. })) - 7.3).abs() < tol, "the release");
        assert!((at(&rig, &|e| matches!(e, ZettaLog::Hit { .. })) - 8.1).abs() < tol, "the hit");
        assert!((at(&rig, &|e| matches!(e, ZettaLog::Ended { .. })) - (14.9 + z().zetta_throw_follow)).abs() < tol, "the end: the beam's, then the throw followed");
        let hold_end = seq_time(z().zetta_hold_until, &z(), 5.0);
        let thrown_at = at(&rig, &|e| matches!(e, ZettaLog::Reaction { .. }));
        assert!((thrown_at - hold_end).abs() < tol, "thrown as the beam ends: {thrown_at} vs {hold_end}");
        assert!(hold_end < 14.9 - 0.2, "the throw comes before the beam's end");
        // he faced the target before the release
        let p = rig.party.member(rig.don).unwrap();
        let want = crate::pawn::yaw_of(crate::pawn::horizontal(enemy - p.position()));
        assert!(crate::pawn::wrap180(p.yaw - want).abs() < 1.0, "yaw {} want {}", p.yaw, want);
        // (round 5) every card of the beam is laid from the tip (the world copies of 93 / 94 retired)
        assert!(rig.spawn_of("fx.world.93_zfr_00").is_none() && rig.spawn_of("fx.world.92_zfr_01").is_some());
        // the beam itself was laid along the line and stretched (crossed copies), and placed again as it grew
        assert!(rig.spawn_of("fx.beam.stream_93_a").is_some() && rig.spawn_of("fx.beam.stream_93_b").is_some());
        assert!(rig.effects.transformed.iter().any(|t| t.3.x > 0.1 && t.3.x < 1.0), "stretched to the target's distance");
        // the hit pushes it back by the event's distance (Terra-Xehanort's 25 cm)
        assert!(rig.dummies.hits.iter().any(|h| h.target == TargetId(1) && (h.info.move_length - z().zetta_react_push).abs() < 1e-3), "{:?}", rig.dummies.hits.first().map(|h| h.info.move_length));
        // the target was held from the channel's start, facing Donald, and given back at the hit
        let holds: Vec<_> = rig.dummies.holds.iter().filter(|h| h.0 == TargetId(1)).collect();
        let span = seq_time(z().zetta_hold_until, &z(), 5.0);
        assert!((holds.iter().filter(|h| h.1.is_some()).count() as f32 - span * 60.0).abs() < 3.0, "held every step through the beam: {}", holds.len());
        // unstoppable: the hits are forced and ignore defence
        assert!(rig.dummies.hits.iter().filter(|h| h.target == TargetId(1)).all(|h| h.flags & kh_core::host::HIT_FLAG_FORCE != 0 && h.info.reaction_power >= 999));
        // (2026-10-23) turned to Donald until the hit, then straight back up the beam's line (the throw goes along it)
        assert!(holds.iter().filter_map(|h| h.1).filter(|h| !h.hurt).all(|h| (h.face - p.position()).length() < 1.0 && h.turn_rate > 0.0));
        assert!(holds.iter().filter_map(|h| h.1).filter(|h| h.hurt).all(|h| h.turn_rate > 0.0 && crate::pawn::horizontal(enemy - h.face).normalize().dot(crate::pawn::horizontal(enemy - p.position()).normalize()) > 0.99));
        assert_eq!(holds.last().unwrap().1, None, "given back");
        assert_eq!(holds.iter().filter(|h| h.1.is_none()).count(), 1, "given back once");
        // the hit landed on the target
        assert!(rig.dummies.hits.iter().any(|h| h.target == TargetId(1)), "no hit");
        // every persistent effect was stopped, nothing is left; nothing is tinted (Zettaflare keeps the event's colours)
        for (h, _, _, _, persistent) in &rig.effects.spawned {
            if *persistent {
                assert!(rig.effects.stopped.contains(h), "effect {h} left running");
            }
        }
        assert!(rig.effects.tinted.is_empty());
        assert!(rig.party.don_zetta.runs.is_empty());
        // the sounds: the event's SE (both cuts) and both voice lines
        for s in ["se_kg702b_cut510_1800_zetta_channel5", "se_kg702b_cut510_1800_zetta_cast", "kg7020241do0", "kg7020250do0"] {
            assert!(rig.audio.played.iter().any(|p| p.ends_with(s)), "{s} not played");
        }
        // the moving emitters were placed along their tracks
        assert!(!rig.effects.placed.is_empty());
        // he may act again; his face is his own again (the lip clip ended with its 3.1 s)
        let p = rig.party.member(rig.don).unwrap();
        assert!(p.don_cast.run.is_none());
        // (2026-10-23) unless he is down after it: his eyes shut through the fall and the down
        assert!(p.face.playing().is_none() || (p.don_zetta.down.is_some() && p.face.playing() == Some(CHANNEL_FACE)), "{:?}", p.face.playing());
    }

    #[test]
    fn the_timing_does_not_depend_on_the_frame_rate() {
        for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0] {
            let mut rig = Rig::new(Vec3::new(700.0, 0.0, 0.0), dt);
            rig.cast();
            rig.run_for(16.0);
            let rel = rig.time_of(|e| matches!(e, ZettaLog::Released { .. })).unwrap();
            let hit = rig.time_of(|e| matches!(e, ZettaLog::Hit { .. })).unwrap();
            let c93 = rig.spawn_of("fx.beam.stream_94_a").unwrap().0;
            assert!((rel - 7.3).abs() <= dt + 1e-3, "{dt}: release {rel}");
            assert!((hit - 8.1).abs() <= dt + 1e-3, "{dt}: hit {hit}");
            assert!((c93 - seq_time(75.967, &z(), 5.0)).abs() <= dt + 1e-3, "{dt}: c94 {c93}");
        }
    }

    #[test]
    fn a_hit_before_the_release_stops_it_and_nothing_is_left() {
        // (round 5, the user: "Donald is invincible during Zettaflare's cutscene") a hit in the channel does not land and
        // stops nothing; the leadership leaving him cancels it cleanly: nothing left, the camera back at once, he is free
        let mut rig = Rig::new(Vec3::new(700.0, 0.0, 0.0), 1.0 / 60.0);
        rig.cast();
        rig.run_for(2.0);
        assert!(rig.party.don_zetta_view(1.0 / 60.0, 0.0).is_some(), "the camera has the view");
        let hp = rig.party.member(rig.don).unwrap().status.hp;
        let hit = HitRecord { damage: 50.0, damage_kind: Kind::Blow, ..HitRecord::empty(TargetId(1)) };
        assert!(!rig.party.receive_hit(rig.don, &hit).accepted, "the hit does not land");
        rig.run_for(0.5);
        assert_eq!(rig.party.member(rig.don).unwrap().status.hp, hp);
        assert!(rig.party.don_zetta.log.iter().all(|e| !matches!(e, ZettaLog::Interrupted { .. })), "the channel goes on");
        // the pad leaves him (the leader changed): cancelled cleanly
        rig.party.set_pad(rig.don, None);
        rig.run_for(0.2);
        assert!(rig.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::Interrupted { .. })));
        assert!(rig.party.don_zetta.runs.is_empty());
        for (h, _, _, _, persistent) in &rig.effects.spawned {
            if *persistent {
                assert!(rig.effects.stopped.contains(h), "effect {h} left running");
            }
        }
        assert!(rig.dummies.hits.is_empty(), "no hit after the cancel");
        assert!(rig.party.don_zetta.log.iter().all(|e| !matches!(e, ZettaLog::Released { .. })));
        assert!(rig.party.don_zetta_view(1.0 / 60.0, 0.0).is_none(), "the camera handed back at once");
        assert!(!holding(rig.party.member(rig.don).unwrap()));
    }

    #[test]
    fn with_no_target_it_goes_ahead_of_him() {
        // (round 5) the user: "Zettaflare should not be castable without a target": with no enemy it is refused (nothing
        // begins, nothing is paid); with one it casts
        let mut rig = Rig::new(Vec3::new(700.0, 0.0, 0.0), 1.0 / 60.0);
        let dummies = std::mem::take(&mut rig.dummies.list);
        assert_eq!(debug_make_ready(&mut rig.party), 1);
        let mp = rig.party.status(rig.don).unwrap().mp;
        rig.party.don_fx.debug_casts.push(SpellRef::Arcane(ArcaneLine::Zettaflare, 1));
        rig.party.don_zetta.take_log();
        rig.run_for(0.5);
        assert!(rig.party.don_zetta.runs.is_empty() && rig.party.don_zetta.log.iter().all(|e| !matches!(e, ZettaLog::Begun { .. })), "refused without a target");
        assert_eq!(rig.party.status(rig.don).unwrap().mp, mp, "nothing paid");
        rig.dummies.list = dummies;
        rig.cast();
        assert!(rig.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::Begun { aim: ZettaAim::Target(TargetId(1)), .. })));
    }

    #[test]
    fn the_cinematic_camera_plays_the_shots_and_hands_back_smoothly() {
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.cast();
        let dt = rig.dt;
        let mut shots: Vec<&str> = Vec::new();
        let mut weights: Vec<f32> = Vec::new();
        for _ in 0..(30.0 / dt) as usize {
            rig.step();
            match rig.party.don_zetta_view(dt, 0.0) {
                Some(v) => {
                    if shots.last() != Some(&v.shot) {
                        shots.push(v.shot);
                    }
                    weights.push(v.weight);
                    assert!(v.position.is_finite() && v.fov > 5.0);
                }
                None => weights.push(0.0),
            }
        }
        assert_eq!(shots, ["shot860", "shot910", "shot920", "shot960", "shot970"], "the charging close-up through the channel (890 and the reactions left out), (round 5) the ending: the wide shot, the shot behind him");
        // (ZETTA 2026-10-23, the user: "Can the first shot of Donald charging up just be a cut from gameplay instead of a spin
        // around?") CUT IN (its first frame at full share: the shot's own camera, nothing of the gameplay camera), held at 1,
        // blended out at the end, then gone
        assert_eq!(weights[0], 1.0, "a cut in");
        assert!(weights.iter().any(|w| (*w - 1.0).abs() < 1e-6));
        let last_full = weights.iter().rposition(|w| (*w - 1.0).abs() < 1e-6).unwrap();
        let gone = weights.iter().rposition(|w| *w > 0.0).unwrap();
        assert!(gone > last_full + 10, "a smooth blend out ({last_full} .. {gone})");
        assert!(weights.windows(2).all(|w| (w[1] - w[0]).abs() < 0.2), "no jump in the share");
        assert_eq!(*weights.last().unwrap(), 0.0);
    }

    #[test]
    fn the_camera_stick_hands_back_and_the_key_turns_it_off() {
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.cast();
        let dt = rig.dt;
        for _ in 0..60 {
            rig.step();
            rig.party.don_zetta_view(dt, 0.0);
        }
        // the camera stick held: the shots blend away, the spell plays on
        let mut w = 1.0;
        for _ in 0..90 {
            rig.step();
            w = rig.party.don_zetta_view(dt, 1.0).map_or(0.0, |v| v.weight);
        }
        assert_eq!(w, 0.0, "handed back");
        assert!(!rig.party.don_zetta.runs.is_empty(), "the spell goes on");
        // it does not come back for the same cast
        for _ in 0..60 {
            rig.step();
            assert!(rig.party.don_zetta_view(dt, 0.0).is_none());
        }
        drop(rig);
        // off by the key: no camera at all
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        set_cinematic(Some(false));
        rig.cast();
        let mut any = false;
        for _ in 0..120 {
            rig.step();
            any |= rig.party.don_zetta_view(dt, 0.0).is_some();
        }
        set_cinematic(None);
        assert!(!any, "zetta.cinematic 0: the gameplay camera stays");
        let z = z();
        assert!(cinematic_of(-1, &z) && !cinematic_of(0, &z) && cinematic_of(1, &z), "the key over the sheet's 1");
    }

    #[test]
    fn donald_cannot_move_while_casting_zettaflare() {
        // the user: "Donald cannot move while casting Zettaflare": from the channel's start to the cast clip's end no stick
        // movement, no turning but to face the target, no jump or dodge; then control is his again
        let enemy = Vec3::new(0.0, 800.0, 0.0);
        let mut rig = Rig::new(enemy, 1.0 / 60.0);
        set_cinematic(Some(false)); // (round 5) with the cinematic camera he stays rooted to the hand-off: the ending's tests
        rig.cast();
        let start = rig.party.member(rig.don).unwrap().position();
        let dt = rig.dt;
        let z = z();
        let free = seq_time(z.zetta_free_at, &z, z.zetta_channel_time);
        let mut t = dt;
        let mut max_off = 0.0f32;
        let mut max_up = 0.0f32;
        while t < free - 0.05 {
            // full stick away from the target, jump and dodge pressed every other step
            let buttons: &[PadButton] = if ((t / dt) as i32) % 2 == 0 { &[PadButton::JUMP, PadButton::FaceLeft] } else { &[] };
            rig.step_pad(Vec2::new(0.0, -1.0), buttons);
            let p = rig.party.member(rig.don).unwrap();
            max_off = max_off.max(crate::pawn::horizontal(p.position() - start).length());
            max_up = max_up.max(p.position().z - start.z);
            assert!(holding(p), "rooted at {t}");
            t += dt;
        }
        assert!(max_off < 1.0, "he moved {max_off} cm");
        assert!(max_up < 1.0, "he jumped {max_up} cm");
        let p = rig.party.member(rig.don).unwrap();
        let want = crate::pawn::yaw_of(crate::pawn::horizontal(enemy - p.position()));
        assert!(crate::pawn::wrap180(p.yaw - want).abs() < 1.0, "he turned only to face the target: yaw {} want {want}", p.yaw);
        assert!(rig.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::Released { .. })), "the spell went off");
        // the cast clip's end: control comes back, the stick moves him
        for _ in 0..30 {
            rig.step_pad(Vec2::ZERO, &[]);
        }
        let before = rig.party.member(rig.don).unwrap().position();
        assert!(!holding(rig.party.member(rig.don).unwrap()));
        for _ in 0..60 {
            rig.step_pad(Vec2::new(0.0, 1.0), &[]);
        }
        let moved = crate::pawn::horizontal(rig.party.member(rig.don).unwrap().position() - before).length();
        assert!(moved > 50.0, "free after the cast: moved {moved} cm");
    }

    #[test]
    fn a_hit_still_interrupts_a_rooted_cast() {
        // (round 5, the user: "Donald is invincible during Zettaflare's cutscene") from the channel's start until control
        // returns no hit lands (no damage, no reaction, nothing stops it); the moment control returns he is hit as ever
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.cast();
        let z = z();
        let handoff = seq_time(z.zetta_end_handoff_at, &z, 5.0);
        let hp = rig.party.member(rig.don).unwrap().status.hp;
        let hit = HitRecord { damage: 40.0, damage_kind: Kind::Blow, ..HitRecord::empty(TargetId(1)) };
        let mut t = rig.dt;
        let mut tried = 0;
        while t < handoff - 0.05 {
            if ((t / rig.dt) as i32) % 30 == 0 {
                let r = rig.party.receive_hit(rig.don, &hit);
                assert!(!r.accepted, "invincible at {t}");
                assert!(rig.party.don_zetta_invincible(rig.don));
                tried += 1;
            }
            rig.step();
            t += rig.dt;
        }
        assert!(tried > 30);
        assert_eq!(rig.party.member(rig.don).unwrap().status.hp, hp, "no damage");
        assert!(rig.party.don_zetta.log.iter().all(|e| !matches!(e, ZettaLog::Interrupted { .. })));
        rig.run_for(0.2);
        assert!(!rig.party.don_zetta_invincible(rig.don), "control back: vulnerable again");
        assert!(rig.party.receive_hit(rig.don, &hit).accepted, "hit as ever");
    }

    #[test]
    fn the_face_clip_follows_the_voice_language() {
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        set_voice_ja(true);
        rig.cast();
        rig.run_for(5.5);
        set_voice_ja(false);
        let p = rig.party.member(rig.don).unwrap();
        assert_eq!(p.face.playing(), Some(FACE_JA));
        let v = p.view();
        assert_eq!(v.anim.face.clip.map(|c| c.sequence), Some(FACE_JA), "the view carries it to the host's poser");
        assert!(v.anim.face.clip.unwrap().time > 0.4);
        // an interrupt after the release does not exist; the clip ends by itself at 3.1 s
        rig.run_for(3.0);
        assert_eq!(rig.party.member(rig.don).unwrap().face.playing(), None);
    }

    fn ult_rig(e: Element) -> (Rig, f32) {
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.party.don_zetta.take_log();
        let release = {
            let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
            let p = rig.party.member_mut(rig.don).unwrap();
            ultimate_begin(p, element_tint(e), CastTarget::Enemy(TargetId(1)), &mut host).expect("begun")
        };
        (rig, release)
    }

    #[test]
    fn an_ultimates_release_flash_is_brief_and_asks_no_cinematic_look() {
        // (round 5c) DON-SPELLB / the coordinator: a long whiteout after the ultimates' release. For every element: the
        // event's release flash and its light are removed ult.flash_time after they start (at once, not played out), no
        // cinematic look (exposure / bloom) and no white plate is asked, no cinematic camera
        let z = z();
        for e in [Element::Fire, Element::Blizzard, Element::Thunder, Element::Water, Element::Aero, Element::Light] {
            let (mut rig, release) = ult_rig(e);
            kh_core::host::cinematic::reset(0.0);
            let mut views = 0;
            for _ in 0..((release + 3.0) / rig.dt) as usize {
                rig.step();
                if rig.party.don_zetta_view(rig.dt, 0.0).is_some() {
                    views += 1;
                }
                assert_eq!(rig.party.don_zetta.look_asked, None, "{e:?}: no cinematic look");
            }
            assert_eq!(views, 0, "{e:?}: no cinematic camera");
            for id in ["fx.world.91_zfr_00", "fx.world.91_rit_00"] {
                let on = rig.time_of(|l| matches!(l, ZettaLog::Spawn { row, .. } if *row == id)).unwrap_or_else(|| panic!("{e:?}: {id} plays"));
                let off = rig.time_of(|l| matches!(l, ZettaLog::Stop { row, .. } if *row == id)).unwrap_or_else(|| panic!("{e:?}: {id} ends"));
                assert!(off - on <= z.ult_flash_time + 2.0 * rig.dt, "{e:?}: {id} brief ({:.2} s)", off - on);
            }
        }
    }

    #[test]
    fn an_ultimate_casts_with_the_circle_in_the_elements_colour() {
        let z = z();
        let (mut rig, release) = ult_rig(Element::Thunder);
        assert!((release - 3.0).abs() < 1e-4, "the release 3 s from now (the user's 3 s): {release}");
        let mp = rig.party.status(rig.don).unwrap().mp;
        let dt = rig.dt;
        let mut camera = false;
        let mut moved = 0.0f32;
        for _ in 0..(4.0 / dt) as usize {
            rig.step_pad(Vec2::new(0.0, 1.0), &[PadButton::JUMP]);
            camera |= rig.party.don_zetta_view(dt, 0.0).is_some();
            let p = rig.party.member(rig.don).unwrap();
            if holding(p) {
                moved = moved.max(p.position().length());
            }
        }
        assert!(!camera, "no cinematic camera for the ultimates");
        assert_eq!(rig.party.status(rig.don).unwrap().mp, mp, "the MP is the caller's");
        let k = RunKind::Ult { tint: element_tint(Element::Thunder) }.scale(&z);
        let tol = 1.5 / 60.0;
        assert!(rig.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::UltBegun { tint, .. } if *tint == element_tint(Element::Thunder))));
        assert!((rig.time_of(|e| matches!(e, ZettaLog::Clip { name: "n_ex001_70291w", .. })).unwrap() - 4.733 * k).abs() < tol, "the cast clip after the scaled channel");
        assert!((rig.time_of(|e| matches!(e, ZettaLog::Released { .. })).unwrap() - 3.0).abs() < tol, "the release at 3 s");
        assert!((rig.time_of(|e| matches!(e, ZettaLog::Ended { .. })).unwrap() - (4.733 + 3.1) * k).abs() < tol, "the end with the cast clip");
        assert!(rig.party.don_zetta.log.iter().all(|e| !matches!(e, ZettaLog::Hit { .. } | ZettaLog::Face { .. })), "no Zettaflare hit, no lip track");
        // the circle and every effect with it tinted yellow; the beams are Zettaflare's
        let spawned: Vec<&Spawned> = rig.effects.spawned.iter().filter(|s| s.1.starts_with("vd_kg702_")).collect();
        assert!(spawned.iter().any(|s| s.1 == "vd_kg702_86_zfr_01"), "the circle");
        assert!(spawned.iter().any(|s| s.1 == "vd_kg702_91_zfr_00"), "the release flash");
        assert!(spawned.iter().all(|s| !s.1.starts_with("vd_kg702_92") && !s.1.starts_with("vd_kg702_93") && !s.1.starts_with("vd_kg702_94")), "no beams");
        let want = element_tint(Element::Thunder);
        for s in &spawned {
            assert!(rig.effects.tinted.iter().any(|(h, t)| *h == s.0 && *t == want), "{} not tinted", s.1);
        }
        // his effort, no "Zettaflare!", no event SE
        assert!(rig.audio.played.iter().any(|p| p.ends_with("kg7020241do0")));
        assert!(!rig.audio.played.iter().any(|p| p.ends_with("kg7020250do0") || p.contains("_zetta_")));
        // rooted through it (the stick held forward, B held), free after
        assert!(moved < 1.0, "rooted: {moved}");
        assert!(rig.party.don_zetta.runs.is_empty());
        assert!(!holding(rig.party.member(rig.don).unwrap()));
        assert_eq!(rig.party.member(rig.don).unwrap().don_zetta.ult, None);
    }

    #[test]
    fn an_ultimate_cancelled_stops_at_once_and_frees_him() {
        let (mut rig, _) = ult_rig(Element::Fire);
        rig.run_for(1.0);
        assert!(holding(rig.party.member(rig.don).unwrap()));
        let live: Vec<u32> = rig.effects.spawned.iter().filter(|s| s.4).map(|s| s.0).collect();
        assert!(!live.is_empty());
        ultimate_cancel(rig.party.member_mut(rig.don).unwrap());
        assert!(!holding(rig.party.member(rig.don).unwrap()), "free at once");
        rig.step();
        for h in &live {
            assert!(rig.effects.stopped.contains(h), "effect {h} left");
        }
        assert!(rig.party.don_zetta.runs.is_empty());
        assert!(rig.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::Interrupted { .. })));
        assert!(rig.party.don_zetta.log.iter().all(|e| !matches!(e, ZettaLog::Released { .. })));
        // a new one may begin
        let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
        assert!(ultimate_begin(rig.party.member_mut(rig.don).unwrap(), element_tint(Element::Fire), CastTarget::Enemy(TargetId(1)), &mut host).is_some());
        // and none twice at once
        assert!(ultimate_begin(rig.party.member_mut(rig.don).unwrap(), element_tint(Element::Fire), CastTarget::Enemy(TargetId(1)), &mut host).is_none());
    }

    #[test]
    fn every_element_has_its_ultimate_cast() {
        for e in Element::ALL {
            let (mut rig, release) = ult_rig(e);
            assert!((release - 3.0).abs() < 1e-4);
            rig.run_for(4.0);
            let want = element_tint(e);
            assert!(rig.effects.tinted.iter().filter(|(_, t)| *t == want).count() >= 8, "{e:?}");
            assert!(rig.party.don_zetta.log.iter().any(|x| matches!(x, ZettaLog::Released { .. })), "{e:?}");
        }
    }

    #[test]
    fn the_allies_leave_the_target_alone_until_the_hit() {
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        set_ending(Some(false)); // (the ending holds a survivor again: its own test)
        assert!(rig.party.don_zetta_reserved().is_empty());
        rig.cast();
        rig.run_for(1.0);
        assert_eq!(rig.party.don_zetta_reserved(), vec![TargetId(1)], "held from the channel's start");
        rig.run_for(7.2);
        assert_eq!(rig.party.don_zetta_reserved(), vec![TargetId(1)], "held through the beam (the user), not given back at the hit");
        rig.run_for(8.0);
        assert!(rig.party.don_zetta_reserved().is_empty(), "given back when the sequence ends (round 5: reserved the whole sequence)");
        drop(rig);
        // an interrupt gives it back at once
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.cast();
        rig.run_for(1.0);
        assert!(super::super::cast::interrupt(rig.party.member_mut(rig.don).unwrap(), true));
        rig.step();
        assert!(rig.party.don_zetta_reserved().is_empty());
        assert_eq!(rig.dummies.holds.last().map(|h| h.1), Some(None), "the hold ended with the interrupt");
    }

    #[test]
    fn ai_donald_can_never_cast_zettaflare() {
        // the user: "make sure AI donald cannot use Zettaflare": every condition met, but no pad drives him
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.party.set_pad(rig.don, None);
        assert_eq!(debug_make_ready(&mut rig.party), 1);
        let mp = rig.party.status(rig.don).unwrap().mp;
        rig.party.don_fx.debug_casts.push(SpellRef::Arcane(ArcaneLine::Zettaflare, 1));
        rig.party.don_zetta.take_log();
        rig.run_for(2.0);
        assert!(rig.party.don_zetta.runs.is_empty() && rig.party.don_zetta.log.iter().all(|e| !matches!(e, ZettaLog::Begun { .. })), "refused");
        assert!(rig.party.member(rig.don).unwrap().don_cast.run.map_or(true, |r| !takes(r.spell)));
        assert_eq!(rig.party.status(rig.don).unwrap().mp, mp, "nothing paid");
        assert!(rig.dummies.holds.is_empty() && rig.party.don_zetta_view(1.0 / 60.0, 0.0).is_none());
        // begin itself refuses it
        {
            let d = data();
            let plan = { let s = rig.party.status(rig.don).unwrap(); let st = s.donald.as_ref().unwrap(); st.core.check_cast(SpellRef::Arcane(ArcaneLine::Zettaflare, 1), &st.learned, s.mp, s.max_mp(), false, &d) };
            if let Ok(plan) = plan {
                let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
                assert!(!begin(rig.party.member_mut(rig.don).unwrap(), &plan, CastTarget::Enemy(TargetId(1)), &mut host));
            }
        }
        // the player-led Donald still can
        drop(rig);
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.cast();
        assert!(rig.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::Begun { .. })));
        // and the leader changing under the cast cancels it cleanly
        rig.run_for(1.0);
        rig.party.set_pad(rig.don, None);
        rig.run_for(0.2);
        assert!(rig.party.don_zetta.runs.is_empty());
        assert!(rig.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::Interrupted { .. })));
        assert!(!holding(rig.party.member(rig.don).unwrap()));
        assert_eq!(rig.dummies.holds.last().map(|h| h.1), Some(None));
    }

    #[test]
    fn the_ai_never_decides_zettaflare() {
        // ai::decide over many random fights, with Zettaflare learned and its condition met: never chosen
        use super::super::ai::{decide, AiAsk, AiChoice, AiEnemy, AiInput};
        let mut party = Party::new(PartyConfig::test_level99());
        let don = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
        debug_make_ready(&mut party);
        let me = party.status(don).unwrap().donald.clone().unwrap();
        let mut seed = 0x2545_F491u32;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 10_000) as f32 / 10_000.0
        };
        for _ in 0..3000 {
            let n = 1 + (rnd() * 4.0) as usize;
            let enemies: Vec<AiEnemy> = (0..n).map(|i| AiEnemy { id: i as u32 + 1, pos: Vec3::new(rnd() * 3000.0 - 1500.0, rnd() * 3000.0 - 1500.0, 0.0), weak: Default::default(), resist: Default::default(), wet: rnd() < 0.3, chilled: rnd() < 0.3, frozen: rnd() < 0.1, burned: rnd() < 0.3, big: rnd() < 0.2 }).collect();
            let hp_max = 500;
            let input = AiInput { me: &me, id: don, pos: Vec3::ZERO, hp: (rnd() * 500.0) as i32 + 1, hp_max, mp: (rnd() * 400.0) as i32, mp_max: 400, mp_charging: rnd() < 0.1, in_battle: rnd() < 0.9, barrier_left: 0.0, orbs: 0, enemies: &enemies, allies: &[], refused: &[] };
            let ask = if rnd() < 0.2 { AiAsk::Cure(don) } else { AiAsk::Attack(None) };
            if let Some(AiChoice::Cast { plan, .. }) = decide(&input, ask) {
                assert!(!takes(plan.spell), "the AI chose Zettaflare");
            }
        }
    }

    #[test]
    fn the_target_is_only_shown_playing_an_animation() {
        // THE STATE MACHINE (the user: "make sure the enemy is never shown during the cutscene without an appropriate
        // animation"): at every moment a shot shows the target, it plays its held loop or its reaction; a target whose hold
        // has no loop to show is kept out of every shot until its reaction
        let _g = GLOBALS.lock().unwrap_or_else(|e| e.into_inner()); // (round 5c: the throw's time follows the ending switch)
        let z = z();
        let shots = cam_shots();
        let feet = Vec3::ZERO;
        for animated in [true, false] {
            for goal in [Vec3::new(800.0, 0.0, 90.0), Vec3::new(0.0, 900.0, 90.0), Vec3::new(700.0, 300.0, 500.0), Vec3::new(3500.0, 800.0, 90.0)] {
                let yaw = crate::pawn::yaw_of(crate::pawn::horizontal(goal - feet));
                let donald = Frame { origin: feet, yaw: yaw + EVENT_DONALD_YAW_OFF };
                let target = Frame { origin: goal, yaw };
                let beam = Basis::beam(donald.point(staff_local(&z)), goal);
                let mut s = 0.0;
                while s < 14.9 {
                    let p = cam_pose(&shots, s, 5.0, &z, donald, target, beam, CamSubject { at: goal, size: 1.0, animated , radius: 50.0, half: 0.0 }, None).unwrap();
                    let state = target_anim(s, &z, 5.0, animated);
                    if frames_both(p.position, p.rotation, p.fov, goal, goal, 1.0) {
                        assert_ne!(state, TargetAnim::Static, "animated {animated} s {s:.1} {}: the target shown standing dead still", p.shot);
                    }
                    if s >= seq_time(throw_at(&z), &z, 5.0) + 0.01 && animated {
                        assert_eq!(state, TargetAnim::Reaction);
                    }
                    s += 0.05;
                }
            }
        }
        assert_eq!(target_anim(1.0, &z, 5.0, true), TargetAnim::HeldLoop);
        assert_eq!(target_anim(1.0, &z, 5.0, false), TargetAnim::Static);
    }

    #[test]
    fn the_targets_shots_are_scaled_to_its_size() {
        // a boss twice Terra-Xehanort's height is framed from twice as far, a small enemy from closer: the same framing
        let mut z = z();
        z.zetta_cam_subject_min = 0.0; // (round 5) the keep-out of a target shot left aside: the framing's own scale
        z.zetta_cam_beam_clear = 0.0; // (round 5) the beam clearance and the low angle left aside too
        z.zetta_cam_target_low = 1.0e6;
        let shots = cam_shots();
        let goal = Vec3::new(800.0, 0.0, 90.0);
        let donald = Frame { origin: Vec3::ZERO, yaw: EVENT_DONALD_YAW_OFF };
        let target = Frame { origin: goal, yaw: 0.0 };
        let beam = Basis::beam(donald.point(staff_local(&z)), goal);
        for s in [8.5, 10.0, 11.5, 13.0] {
            let one = cam_pose(&shots, s, 5.0, &z, donald, target, beam, CamSubject { at: goal, size: 1.0, animated: true , radius: 0.0, half: 0.0 }, None).unwrap();
            let two = cam_pose(&shots, s, 5.0, &z, donald, target, beam, CamSubject { at: goal, size: 2.0, animated: true , radius: 0.0, half: 0.0 }, None).unwrap();
            let half = cam_pose(&shots, s, 5.0, &z, donald, target, beam, CamSubject { at: goal, size: 0.5, animated: true , radius: 0.0, half: 0.0 }, None).unwrap();
            let d1 = (one.position - goal).length();
            assert!(((two.position - goal).length() - 2.0 * d1).abs() < 1.0, "{s}");
            assert!(((half.position - goal).length() - 0.5 * d1).abs() < 1.0, "{s}");
            assert!((one.fov - two.fov).abs() < 1e-4, "the same lens");
            assert!(frames_both(one.position, one.rotation, one.fov, goal, goal, 1.0) && frames_both(two.position, two.rotation, two.fov, goal, goal, 1.0));
        }
    }

    #[test]
    fn the_blast_hits_the_enemies_round_the_target_and_in_the_beam_never_allies() {
        // the user: "Enemies caught in the blast of Zettaflare should also take damage"
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.dummies.list.push((TargetId(2), Vec3::new(1000.0, 150.0, 0.0))); // 2.5 m from the target: in the blast
        rig.dummies.list.push((TargetId(3), Vec3::new(400.0, 40.0, 0.0))); // in the beam's path, half way
        rig.dummies.list.push((TargetId(4), Vec3::new(800.0, 2500.0, 0.0))); // far away
        rig.party.member_mut(rig.don).unwrap().pad.as_mut().unwrap().lock = TargetId(1);
        rig.cast();
        rig.run_for(9.0);
        let hit = |id: u32| rig.dummies.hits.iter().filter(|h| h.target == TargetId(id)).count();
        assert!(hit(1) >= 1 && hit(2) >= 1 && hit(3) >= 1, "the target, the one beside it and the one in the beam: {} {} {}", hit(1), hit(2), hit(3));
        assert_eq!(hit(4), 0, "not the far one");
        let full = rig.dummies.hits.iter().find(|h| h.target == TargetId(1)).unwrap().damage;
        let splash = rig.dummies.hits.iter().find(|h| h.target == TargetId(2)).unwrap().damage;
        assert!((splash - full * z().zetta_splash_share).abs() < 1.0, "full damage by default ({splash} vs {full})");
        // (2026-10-23: the beam's path is swept as its head reaches each one: logged one by one)
        let caught: Vec<TargetId> = rig.party.don_zetta.log.iter().filter_map(|e| if let ZettaLog::Blast { caught, .. } = e { Some(caught.clone()) } else { None }).flatten().collect();
        assert!(caught.contains(&TargetId(2)) && caught.contains(&TargetId(3)), "{caught:?}");
    }

    #[test]
    fn the_target_is_thrown_along_the_beam_before_it_ends() {
        let enemy = Vec3::new(600.0, 450.0, 0.0);
        let mut rig = Rig::new(enemy, 1.0 / 60.0);
        rig.cast();
        rig.run_for(16.5);
        let z = z();
        let hold_end = seq_time(z.zetta_hold_until, &z, 5.0);
        let last = rig.dummies.hits.iter().filter(|h| h.target == TargetId(1)).last().copied().expect("the throw");
        assert_eq!(last.damage_kind, Kind::Blow, "a big knock-back");
        assert!((last.info.move_length - z.zetta_react_push).abs() < 1e-3);
        let p = rig.party.member(rig.don).unwrap().position();
        let beam = crate::pawn::horizontal(enemy - p).normalize();
        let thrown = crate::pawn::horizontal(last.direction).normalize(); // (2026-10-23: carried straight down the beam)
        assert!(thrown.dot(beam) > 0.99, "along the beam: {thrown} vs {beam}");
        assert!(hold_end < 14.9, "before the beam's end");
        // the hold ended with it, once
        assert_eq!(rig.dummies.holds.iter().filter(|h| h.1.is_none()).count(), 1);
    }

    #[test]
    fn the_allies_keep_out_of_the_zone() {
        // the user: "allies should stay AWAY from the Zettaflare target": an AI Goofy right beside the target walks out of
        // the zone and keeps out of the beam's corridor while the target is held; the zone goes with the hold
        let mut rig = Rig::new(Vec3::new(900.0, 0.0, 0.0), 1.0 / 60.0);
        set_ending(Some(false));
        let goofy = rig.party.spawn(Chara::Goofy, Vec3::new(850.0, 60.0, 0.0), 0.0);
        rig.cast();
        rig.run_for(0.2);
        let zones = rig.party.don_zetta_zones();
        assert_eq!(zones.len(), 2, "the target's zone and (round 5) Donald's");
        assert!(zones[0].inside(rig.party.member(goofy).unwrap().position()), "he starts in it");
        let start = rig.party.member(goofy).unwrap().position();
        let mut out_at = None;
        let mut last = start;
        for k in 0..(5.0 / rig.dt) as usize {
            rig.step();
            let p = rig.party.member(goofy).unwrap().position();
            assert!((p - last).length() < 30.0, "no teleport");
            last = p;
            if out_at.is_none() && !rig.party.don_zetta_zones().iter().any(|z| z.inside(p)) {
                out_at = Some(k as f32 * rig.dt);
            }
            if let Some(t) = out_at {
                if k as f32 * rig.dt > t + 0.5 {
                    assert!(!rig.party.don_zetta_zones().iter().any(|z| z.inside(p)), "he never walks back in");
                }
            }
        }
        assert!(out_at.is_some_and(|t| t < 5.5), "out within 5.5 s (round 5: the zone is 5 m + its radius: a walk of about 6 m from a standstill): {out_at:?}");
        rig.run_for(12.0);
        assert!(rig.party.don_zetta_zones().is_empty(), "the zone goes with the hold");
    }

    #[test]
    fn the_zone_moves_goals_out_and_routes_round_it() {
        let z = AvoidZone { at: Vec3::new(1000.0, 0.0, 0.0), radius: 400.0, from: Vec3::new(150.0, 0.0, 100.0), to: Vec3::new(1000.0, 0.0, 90.0), half_width: 150.0, margin: 60.0, caster: MemberId(9) };
        let zones = [z];
        // a goal inside is moved out; a member inside goes out first; a straight way across goes round
        let g = zone_goal(&zones, Vec3::new(1000.0, 1500.0, 0.0), Vec3::new(1000.0, 100.0, 0.0));
        assert!(!z.inside(g));
        let out = zone_goal(&zones, Vec3::new(950.0, 50.0, 0.0), Vec3::new(1000.0, 2000.0, 0.0));
        assert!(!z.inside(out) && (out - Vec3::new(950.0, 50.0, 0.0)).length() < 600.0, "the nearest way out: {out}");
        let from = Vec3::new(600.0, -900.0, 0.0);
        let via = zone_goal(&zones, from, Vec3::new(600.0, 900.0, 0.0));
        assert!(!z.inside(via));
        let d = via - from;
        let n = 40;
        assert!((1..n).all(|i| !z.inside(from + d * (i as f32 / n as f32))), "the way to the waypoint does not cross: {via}");
        // the beam's corridor counts; Donald's own place does not
        assert!(z.inside(Vec3::new(500.0, 100.0, 0.0)));
        assert!(!z.inside(Vec3::new(0.0, 0.0, 0.0)));
        assert!(zone_goal(&[], from, Vec3::ONE) == Vec3::ONE);
    }

    #[test]
    fn no_target_keeps_a_finale_or_a_black_look_after_the_sequence() {
        // (round 5c) the user: "the black silhouette isn't crumbling and is in fact remaining after the zettaflare": a killed
        // target's silhouette is crumbled to nothing (1) before its look is given back, its fragments pushed along the beam;
        // a survivor never gets one; after the sequence no target has a finale or a black look, with or without the ending
        for ending in [false, true] {
            for (hp, killed) in [(1.0e6f32, false), (500.0, true), (1.0, true)] {
                let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
                rig.dummies.hp = hp;
                set_ending(Some(ending));
                rig.cast();
                rig.run_for(40.0);
                let looks: Vec<kh_core::host::ScriptFinale> = rig.dummies.finales.iter().filter_map(|f| f.1).collect();
                assert_eq!(looks.iter().any(|f| f.silhouette), killed, "ending {ending}, hp {hp}: the silhouette only for a kill");
                if killed {
                    let last = looks.iter().rev().find(|f| f.silhouette).unwrap();
                    assert!((last.crumble - 1.0).abs() < 1e-4, "ending {ending}, hp {hp}: crumbled to nothing before the end ({})", last.crumble);
                    assert!(looks.iter().filter(|f| f.silhouette).all(|f| (f.push.length() - 1.0).abs() < 1e-3), "the beam's push given");
                }
                let mut ids: Vec<TargetId> = rig.dummies.finales.iter().map(|f| f.0).collect();
                ids.dedup();
                for id in ids {
                    let last = rig.dummies.finales.iter().rev().find(|f| f.0 == id).unwrap();
                    assert_eq!(last.1, None, "ending {ending}, hp {hp}: no finale (no black look) left on {id:?}");
                }
                let holds_after = rig.dummies.holds.iter().rev().take_while(|h| h.1.is_none()).count();
                assert!(holds_after > 0 || rig.dummies.holds.is_empty(), "ending {ending}, hp {hp}: the hold given back");
            }
        }
        set_ending(None);
    }

    #[test]
    fn level_99_zettaflare_does_20000_and_lower_levels_scale_down_by_magic() {
        // (2026-10-23) the user: "make level 99 Zettaflare do 20,000 damage" (Elden Ring HP: the host's x7) and "make sure
        // Zettaflare scales with your magic stat": its power x his Magic only, past defence, resistances and limits
        use kh_core::status::damage::{compute_damage, DamageAttack, DamageContext, Defender};
        let power = SpellRef::Arcane(ArcaneLine::Zettaflare, 1).row(&data()).unwrap().power;
        let kh = |s: &crate::FriendStatus| compute_damage(&s.as_attacker(), &Defender::NONE, &DamageAttack::magic(power, kh_core::enums::ETresDamageAttribute::Notype), &DamageContext::new(s.game_level, 1)).amount;
        let l99 = crate::FriendStatus::test_level99(crate::Chara::Donald);
        assert_eq!(l99.magic(), 71);
        let er = (kh(&l99) * 7.0).round();
        assert!((er - 20000.0).abs() <= 7.0, "level 99: {} KH = {er} Elden Ring HP", kh(&l99));
        let l1 = crate::FriendStatus::new(crate::Chara::Donald, 0);
        assert!(kh(&l1) < kh(&l99) * 0.5, "level 1 scales down by Magic ({} KH)", kh(&l1));
        // the beam's own hit through the party route, on the rig's level-99 Donald: that number, on a plain and a guarded target
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        set_ending(Some(false));
        rig.cast();
        rig.run_for(9.0);
        set_ending(None);
        let m = rig.party.status(rig.don).unwrap().magic();
        let hit = rig.dummies.hits.iter().filter(|h| h.target == TargetId(1)).map(|h| h.damage).fold(0.0f32, f32::max);
        let want = kh(rig.party.status(rig.don).unwrap());
        assert!((hit - want).abs() < 0.5, "the beam's hit is his Magic ({m}) x its power: {hit} vs {want}");
    }

    #[test]
    fn a_small_targets_beam_runs_where_the_sora_cases_would() {
        // (2026-10-23) the beam never under the Sora case's: the beam into a 0.5 m target runs through its centre (the enemy
        // shot inside it), the ending beam (the wide shot) at the Sora case's centre height over its feet (not half under the
        // floor); a target of the Sora case's size or larger: its centre both times
        for (half, radius, want) in [(25.0f32, 15.0f32, 90.0f32), (90.0, 40.0, 90.0), (250.0, 120.0, 250.0)] {
            let mut rig = Rig::new(Vec3::new(900.0, 0.0, 0.0), 1.0 / 60.0);
            rig.dummies.half = half;
            rig.dummies.radius = radius;
            rig.cast();
            let z = z();
            rig.run_for(seq_time(z.zetta_end_start, &z, 5.0) + 0.5);
            let r = rig.party.don_zetta.runs.first().expect("the run");
            let hit = r.beam_at_hit.expect("the beam at the hit");
            let into = hit.origin + hit.x * (crate::pawn::horizontal(Vec3::new(900.0, 0.0, 0.0) - hit.origin).length() / crate::pawn::horizontal(hit.x).length().max(1e-4));
            assert!((into.z - half).abs() < 3.0, "{half}/{radius}: the beam into it runs through its centre ({} cm up)", into.z);
            let b = r.end_beam.expect("the ending beam");
            let d = crate::pawn::horizontal(Vec3::new(900.0, 0.0, 0.0) - b.origin).length() / crate::pawn::horizontal(b.x).length().max(1e-4);
            let at = b.origin + b.x * d;
            assert!((at.z - want).abs() < 3.0, "{half}/{radius}: the ending beam runs {} cm up over it (want {want})", at.z);
        }
    }

    #[test]
    fn the_ending_beams_far_end_is_never_in_the_long_shot() {
        // (2026-10-23) the user: "the end of the beam should never be visible from the long shot, it should always extend out of
        // frame"; the coordinator: for factors 1, 2.5, 5 and 10, at the wide shot's start, middle and end, the far end's
        // projection is outside the screen (its back end at the tip: kh-host tests/zetta_beam_lines.rs)
        let z = z();
        for g in [1.0f32, 2.5, 5.0, 10.0] {
            let mut rig = Rig::new(Vec3::new(1500.0, 0.0, 0.0), 1.0 / 60.0);
            rig.dummies.half = g * z.zetta_frame_body_height * 0.5;
            rig.dummies.radius = 10.0;
            set_ending(Some(true));
            rig.cast();
            for mt in [z.zetta_end_start + 0.2, z.zetta_end_start + 3.5, 86.6] {
                let at = seq_time(mt, &z, 5.0);
                let now = rig.party.don_zetta.runs.first().map_or(0.0, |r| r.t);
                rig.run_for((at - now).max(0.0));
                let run = rig.party.don_zetta.runs.first().expect("the run").clone();
                assert!((run.girth - g).abs() < 0.05, "girth {} for {g}", run.girth);
                let b = run.end_beam.expect("the ending beam");
                let fr = run.end_frame.expect("the ending's frame");
                let cam = cam_pose(&cam_shots(), run.t, run.channel, &z, fr, fr, b, CamSubject { at: b.origin, size: run.girth, animated: true, radius: 10.0, half: 0.0 }, Some(fr)).expect("the camera");
                assert_eq!(cam.shot, "shot960", "{g} at {mt}: the long shot");
                let row = DON_ZETTA_FX.iter().find(|r| r.id == "end.beam.96_zfr_00").unwrap();
                let tip_far = b.origin + b.x * (end_len_scale(g) * end_card_len(row.asset, mt - row.start) * 0.97);
                let (h, v) = view_angles(cam.position, cam.rotation, tip_far);
                let fwd = (tip_far - cam.position).dot(rot_axes(cam.rotation)[0]);
                assert!(fwd < 0.0 || h.abs() > cam.fov * 0.5 || v.abs() > vfov_of(cam.fov) * 0.5, "{g} at {mt}: the far end in the frame ({h:.1}, {v:.1} deg)");
            }
            set_ending(None);
        }
    }

    #[test]
    fn his_staff_falls_in_one_arc_bounces_once_and_comes_to_rest_lying_along_the_ground() {
        // (2026-10-23) the user: "the wand dropping and then settling for Donald looks a bit jank": its own fall on a fixed step,
        // at any frame rate the same rest: both ends on the floor (no hovering, no sinking), lying along it at its own length,
        // then exactly still; on a slope too
        let level = |_: Vec3| Some(0.0f32);
        let slope = |q: Vec3| Some(q.x * 0.15);
        for (name, ground) in [("level", &level as &dyn Fn(Vec3) -> Option<f32>), ("slope", &slope)] {
            let mut rests = Vec::new();
            for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0] {
                let start = StaffFall::from_hand(Vec3::new(30.0, 0.0, 95.0), glam::Quat::from_rotation_y(-0.6), Vec3::new(-180.0, 40.0, 120.0));
                let mut f = start;
                let mut t = 0.0;
                let mut peak = f.pos.z;
                let mut bounces = 0;
                let mut was_up = true;
                while t < 4.0 {
                    f.step(dt, ground);
                    t += dt;
                    peak = peak.max(f.pos.z);
                    let (a, b) = f.ends();
                    let (ga, gb) = (ground(a).unwrap(), ground(b).unwrap());
                    assert!(a.z >= ga + STAFF_RADIUS - 0.5 && b.z >= gb + STAFF_RADIUS - 0.5, "{name} {dt}: never into the floor ({} {})", a.z - ga, b.z - gb);
                    let up = f.vel.z > 0.0;
                    if up && !was_up && f.landed {
                        bounces += 1;
                    }
                    was_up = up;
                }
                assert!(f.resting && f.landed, "{name} {dt}: at rest");
                assert!(peak > start.pos.z + 5.0, "{name} {dt}: one clean arc up out of his hand");
                assert!(bounces <= 1, "{name} {dt}: one small bounce at most ({bounces})");
                let (a, b) = f.ends();
                let (ga, gb) = (ground(a).unwrap(), ground(b).unwrap());
                assert!((a.z - ga - STAFF_RADIUS).abs() < 1.0 && (b.z - gb - STAFF_RADIUS).abs() < 1.0, "{name} {dt}: lying along the ground ({} {})", a.z - ga, b.z - gb);
                assert!(((b - a).length() - (STAFF_ENDS.1 - STAFF_ENDS.0)).abs() < 0.5, "at its own length");
                // still: nothing moves after rest
                let rest = (f.pos, f.rot);
                for _ in 0..60 {
                    f.step(dt, ground);
                }
                assert!(f.pos == rest.0 && f.rot == rest.1, "{name} {dt}: exactly still at rest");
                rests.push(f.pos);
            }
            // the same rest at any frame rate (a fixed step)
            assert!(rests.iter().all(|r| (*r - rests[0]).length() < 6.0), "{name}: frame-rate independent {rests:?}");
        }
    }

    #[test]
    fn after_zettaflare_he_collapses_lies_down_gets_up_and_a_hit_gives_him_back() {
        // (2026-10-23) the user: "have Donald smoothly go back to his normal animations after the Zettaflare animation ... maybe
        // he falls down, the player can't control him but can rotate the camera, then 2 seconds later he gets back up and is
        // controllable. Donald can be hurt during this knocked down animation"; "include proper voice lines for falling down
        // and getting back up"; "Can you use the cutscene animation where he falls down and drops his staff?"; "getting hit
        // cancels his current animation and plays his usual hurt animation, the player regains control"
        let z = z();
        let handoff = seq_time(z.zetta_end_handoff_at, &z, 5.0);
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        set_ending(Some(true));
        rig.cast();
        rig.run_for(handoff + 0.2);
        let p = rig.party.member(rig.don).unwrap();
        let d = p.don_zetta.down.expect("down from the hand-off");
        assert!(holding(p), "the pad's frame taken (the camera stays the player's)");
        assert!(!invincible(&rig.party, rig.don), "he can be hurt");
        assert_eq!(p.anim.clip.sequence, DOWN_FALL_A.sequence, "the cutscene's collapse");
        assert!(rig.audio.played.contains(&"donald_damage_1:0"), "his cry as he falls: {:?}", rig.audio.played);
        let at = p.position();
        // on his back, his staff on the ground, through zetta.down_time
        rig.run_for(down_up_at(&z) - d.t - 0.1);
        let p = rig.party.member(rig.don).unwrap();
        let d = p.don_zetta.down.expect("still down");
        assert_eq!(p.anim.clip.sequence, DOWN_FALL_B.sequence);
        assert!((p.anim.time - DOWN_FALL_B.length).abs() < 1e-3 && down_staff_dropped(&d, &z), "the collapse held on its last frame, the staff where it fell");
        assert!((crate::pawn::horizontal(p.position() - at)).length() < 1.0, "he stays where he fell");
        // (2026-10-23) its fall: down onto the floor and at rest, lying along it
        let (gp, _) = rig.party.weapon_ground(rig.don).expect("out of his hand");
        assert!(gp.z < 20.0, "on the floor, not hanging at his hand ({gp})");
        // (the user: "Donald's wand should stay where it falls") his staff lies where it fell, drawn there, not in his hand
        assert!(rig.party.weapon_ground(rig.don).is_some() && !rig.party.weapon_hidden(rig.don), "the staff on the ground");
        let lights = |rig: &Rig| rig.effects.spawned.iter().filter(|e| e.1 == STAFF_LIGHT).count();
        assert_eq!(lights(&rig), 0);
        // he gets up (his staff back in his hand over the blend), says his line, then is the player's again
        rig.run_for(0.3);
        let p = rig.party.member(rig.don).unwrap();
        assert_eq!(p.anim.clip.sequence, down_getup().sequence, "KH3's get-up");
        assert!(!down_staff_dropped(&p.don_zetta.down.unwrap(), &z));
        // ... it dissolves where it lay and is on its way to his hand (hidden), then appears there in its light
        assert!(rig.party.weapon_ground(rig.don).is_none() && rig.party.weapon_hidden(rig.don) && lights(&rig) == 1, "dissolved on the ground");
        rig.run_for(z.zetta_down_up_blend);
        assert!(!rig.party.weapon_hidden(rig.don) && rig.party.weapon_ground(rig.don).is_none() && lights(&rig) == 2, "in his hand, in its light");
        assert!(rig.audio.played.contains(&STAFF_APPEAR_SE), "with the Keyblade's appear sound");
        assert!(rig.party.member(rig.don).unwrap().don_zetta.down.is_some(), "while he still gets up (not yet the player's)");
        assert!(rig.audio.played.iter().any(|v| v.starts_with("BattleStart:")), "his line as he gets up (zetta.down_up_voice): {:?}", rig.audio.played);
        rig.run_for(down_length(&z));
        let p = rig.party.member(rig.don).unwrap();
        assert!(p.don_zetta.down.is_none() && !holding(p), "the player has him back");
        assert!(p.anim.clip.sequence != down_getup().sequence && p.anim.clip.sequence != DOWN_FALL_B.sequence, "his idle");
        set_ending(None);
        drop(rig); // (its hold on the module's globals)
        // a hit at any point (the collapse, down, the get-up): his usual reaction plays at once, the player has him back
        for after in [0.5, down_up_at(&z) - 0.5, down_up_at(&z) + 0.2] {
            let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
            rig.cast();
            rig.run_for(handoff + after);
            assert!(rig.party.member(rig.don).unwrap().don_zetta.down.is_some());
            let grounded = rig.party.weapon_ground(rig.don).is_some();
            rig.party.member_mut(rig.don).unwrap().enter(NpcStateId::Damage);
            rig.step();
            let p = rig.party.member(rig.don).unwrap();
            assert!(p.don_zetta.down.is_none() && !holding(p), "{after}: the down ends with the hit; nothing holds the pad");
            // the staff stays where it fell through his reaction, and is in his hand the moment the player has him
            assert_eq!(rig.party.weapon_ground(rig.don).is_some(), grounded, "{after}: the staff stays where it is through the reaction");
            rig.party.member_mut(rig.don).unwrap().to_idle();
            rig.step();
            rig.run_for(z.zetta_down_up_blend + 0.05);
            assert!(rig.party.weapon_ground(rig.don).is_none() && !rig.party.weapon_hidden(rig.don), "{after}: in his hand with the player's control");
        }
        set_ending(None);
    }

    #[test]
    fn the_cast_target_is_the_one_for_the_whole_sequence_whatever_the_lock_on_does() {
        // (2026-10-23) the user: "Zettaflare's camera angle was changing if the target changed while playing the animation,
        // DO NOT LET THIS HAPPEN!! The target that Zettaflare is initiated on is THE one that is focused on in the cutscene"
        let z = z();
        let mut rig = Rig::new(Vec3::new(900.0, 0.0, 0.0), 1.0 / 60.0);
        rig.dummies.list.push((TargetId(2), Vec3::new(400.0, 700.0, 0.0)));
        rig.party.member_mut(rig.don).unwrap().pad.as_mut().unwrap().lock = TargetId(1);
        rig.cast();
        rig.run_for(1.0);
        assert_eq!(rig.party.don_zetta.runs[0].locked, Some(ZettaAim::Target(TargetId(1))));
        // the player (or a retarget) moves the lock-on to the other enemy mid-sequence: nothing follows it
        let mut subjects = Vec::new();
        let mut t = 1.0;
        while t < seq_time(z.zetta_white_in_at, &z, 5.0) {
            rig.party.member_mut(rig.don).unwrap().pad.as_mut().unwrap().lock = TargetId(2);
            rig.party.member_mut(rig.don).unwrap().pad.as_mut().unwrap().soft = TargetId(2);
            rig.step();
            t += rig.dt;
            if let Some(f) = rig.party.don_zetta.cam.feed {
                subjects.push(f.subject.at);
            }
            let run = &rig.party.don_zetta.runs[0];
            assert_eq!(run.locked, Some(ZettaAim::Target(TargetId(1))), "the cast's target, fixed");
        }
        let run = &rig.party.don_zetta.runs[0];
        assert_eq!(run.hit_target, Some(TargetId(1)), "the beam hit the cast's target");
        assert!(subjects.iter().all(|s| crate::pawn::horizontal(*s - Vec3::new(900.0, 0.0, 0.0)).length() < 5.0), "the camera on the cast's target throughout");
        assert!(rig.dummies.hits.iter().all(|h| h.target != TargetId(2)), "the other enemy (off the beam) untouched");
        // the lock-on kept on it through the sequence (the pad's own rule again at the hand-back)
        assert_eq!(rig.party.member(rig.don).unwrap().pad.as_ref().unwrap().lock, TargetId(1));
        drop(rig);
        // the target gone mid-sequence (despawned): the shot and the beam stay on where it last was, never another enemy
        let mut rig = Rig::new(Vec3::new(900.0, 0.0, 0.0), 1.0 / 60.0);
        rig.dummies.list.push((TargetId(2), Vec3::new(400.0, 700.0, 0.0)));
        rig.party.member_mut(rig.don).unwrap().pad.as_mut().unwrap().lock = TargetId(1);
        rig.cast();
        rig.run_for(2.0);
        rig.dummies.list.retain(|d| d.0 != TargetId(1));
        rig.run_for(seq_time(z.zetta_hit_at, &z, 5.0) + 1.0 - 2.0);
        let f = rig.party.don_zetta.cam.feed.expect("the camera");
        assert!(crate::pawn::horizontal(f.subject.at - Vec3::new(900.0, 0.0, 0.0)).length() < 5.0, "its last place ({})", f.subject.at);
        assert!(rig.dummies.hits.iter().all(|h| h.target != TargetId(2)), "never another enemy");
    }

    #[test]
    fn an_enemy_in_the_beams_path_is_blasted_away_with_the_full_damage_one_outside_is_untouched() {
        // (2026-10-23) the user: "any enemies that in the beam get pushed away with immense damage"
        let z = z();
        let mut rig = Rig::new(Vec3::new(1500.0, 0.0, 0.0), 1.0 / 60.0);
        rig.dummies.list.push((TargetId(2), Vec3::new(700.0, 60.0, 0.0))); // in the path, a little off its axis
        rig.dummies.list.push((TargetId(3), Vec3::new(700.0, 1200.0, 0.0))); // well outside it
        rig.party.member_mut(rig.don).unwrap().pad.as_mut().unwrap().lock = TargetId(1);
        rig.cast();
        rig.run_for(seq_time(z.zetta_hit_at, &z, 5.0) + 0.5);
        let main = rig.dummies.hits.iter().find(|h| h.target == TargetId(1)).copied().expect("the target's hit");
        let swept = rig.dummies.hits.iter().find(|h| h.target == TargetId(2)).copied().expect("the enemy in the path is hit");
        assert!((swept.damage - main.damage).abs() < 0.5 * main.damage.max(1.0) && swept.damage >= main.damage * 0.99, "the beam's full damage ({} vs {})", swept.damage, main.damage);
        assert!(swept.flags & kh_core::host::HIT_FLAG_FORCE != 0 && swept.damage_kind == Kind::Blow, "forced past armour, a Blow");
        let dir = crate::pawn::horizontal(swept.direction).normalize();
        assert!(dir.x > 0.5 && dir.y > 0.2, "away from Donald along the beam and out off its axis ({dir})");
        assert!(swept.info.move_length > 300.0, "pushed far");
        assert!(rig.dummies.hits.iter().all(|h| h.target != TargetId(3)), "outside the beam: untouched");
        // hit when the beam reached it (before the target's hit), not all at once
        let i_swept = rig.dummies.hits.iter().position(|h| h.target == TargetId(2)).unwrap();
        let i_main = rig.dummies.hits.iter().position(|h| h.target == TargetId(1)).unwrap();
        assert!(i_swept < i_main, "the head reaches it first");
    }

    #[test]
    fn a_survivor_is_thrown_straight_back_along_the_beam() {
        // (2026-10-23) the user: "make sure the target, if they survive, is thrown BACK towards the direction of the beam, not to
        // the side": from the hit it faces straight back up the beam's line, and the throw carries it straight down it, wherever
        // it stands and whichever way it faced
        for enemy in [Vec3::new(900.0, 0.0, 0.0), Vec3::new(300.0, 800.0, 0.0), Vec3::new(-700.0, -500.0, 0.0), Vec3::new(1200.0, 900.0, 0.0)] {
            let mut rig = Rig::new(enemy, 1.0 / 60.0);
            set_ending(Some(false));
            rig.cast();
            rig.run_for(16.5);
            set_ending(None);
            let b = rig.party.don_zetta.runs.first().and_then(|r| r.beam_at_hit).or_else(|| {
                let p = rig.party.member(rig.don).unwrap().position();
                Some(Basis::beam(p, enemy))
            }).unwrap();
            let axis = crate::pawn::horizontal(b.x).normalize();
            let last = rig.dummies.hits.iter().filter(|h| h.target == TargetId(1)).last().copied().expect("the throw");
            let dir = crate::pawn::horizontal(last.direction).normalize();
            assert!(dir.dot(axis) > (3.0f32).to_radians().cos(), "{enemy}: thrown along the beam ({dir} vs {axis})");
            // it faced back up the beam while held after the hit
            let faced: Vec<Vec3> = rig.dummies.holds.iter().filter_map(|h| h.1).filter(|h| h.hurt).map(|h| crate::pawn::horizontal(enemy - h.face).normalize()).collect();
            assert!(!faced.is_empty() && faced.iter().all(|f| f.dot(axis) > (3.0f32).to_radians().cos()), "{enemy}: facing back up the beam");
        }
    }

    #[test]
    fn zettaflare_damage_scales_with_donalds_magic() {
        // (2026-10-23) the user: "make sure Zettaflare scales with your magic stat": the beam's hit goes through his spells'
        // damage path (y.zettaflare.1's power x his Magic, Sora's damage function), past armour, defence and resistances, so
        // only his Magic (and the power) decide the number; more Magic, more damage
        let damage = |magic: u8| {
            let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
            rig.party.status_mut(rig.don).unwrap().roster.magic = magic;
            set_ending(Some(false));
            rig.cast();
            rig.run_for(9.0);
            let m = rig.party.status(rig.don).unwrap().magic();
            let d = rig.dummies.hits.iter().filter(|h| h.target == TargetId(1)).map(|h| h.damage).fold(0.0f32, f32::max);
            (m, d)
        };
        let (m0, d0) = damage(0);
        let (m1, d1) = damage(60);
        set_ending(None);
        assert!(m1 > m0, "more Magic ({m0} -> {m1})");
        assert!(d0 > 0.0 && d1 > d0 * 1.2, "Zettaflare's damage grows with his Magic ({d0} at {m0}, {d1} at {m1})");
    }

    #[test]
    fn after_zettaflare_his_mp_regen_is_slower_until_it_has_recharged() {
        // (2026-10-23) the user: "it makes your MP regen slower by 30%": his MP Charge runs zetta.regen_mult (0.7) as fast (about
        // 1.43 x as long) until his MP bar has recharged; then his regen is normal again
        let z = z();
        assert!((z.zetta_regen_mult - 0.7).abs() < 1e-6);
        let mut party = crate::Party::new(crate::PartyConfig::test_level99());
        let id = party.spawn(crate::Chara::Donald, Vec3::ZERO, 0.0);
        let charge = |party: &mut crate::Party, slow: bool| {
            let s = party.status_mut(id).unwrap();
            s.mp = 1;
            s.mp_charge_left = 0.0;
            s.donald.as_mut().unwrap().core.zetta_slow = slow;
            spend_all_mp(s, 1);
            let mut t = 0.0f32;
            while party.status(id).unwrap().mp_charge_left > 0.0 && t < 200.0 {
                party.status_mut(id).unwrap().tick(1.0 / 60.0);
                crate::donald::hooks::step(party, 1.0 / 60.0);
                t += 1.0 / 60.0;
            }
            t
        };
        let normal = charge(&mut party, false);
        let slowed = charge(&mut party, true);
        assert!((slowed / normal - 1.0 / 0.7).abs() < 0.03, "about 1.43 x as long ({normal} s vs {slowed} s)");
        assert!(!party.status(id).unwrap().donald.as_ref().unwrap().core.zetta_slow, "normal again once his MP has recharged");
    }

    #[test]
    fn the_silhouette_darkens_out_of_the_white_smoothly() {
        // (round 5c) the user: "make the transition from the first to the second much smoother, no full black suddenly
        // popping in": from the white it was washed into, the silhouette's look ramps in over zetta.finale_dark_time, eased,
        // never more than a small step a frame, done before it breaks apart
        let z = z();
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.dummies.hp = 500.0;
        set_ending(Some(false));
        rig.cast();
        rig.run_for(16.5);
        let darks: Vec<f32> = rig.dummies.finales.iter().filter_map(|f| f.1).filter(|f| f.silhouette).map(|f| f.dark).collect();
        assert!(darks.first().is_some_and(|d| *d < 0.05), "it starts from the white ({:?})", darks.first());
        assert!(darks.windows(2).all(|w| w[1] >= w[0] - 1e-6 && w[1] - w[0] < 0.06), "a smooth ramp, no step");
        let ramp = darks.iter().filter(|d| **d > 0.02 && **d < 0.98).count() as f32 / 60.0;
        assert!(ramp > 0.45 && ramp < 1.1, "a visible blend ({ramp} s)");
        assert!(z.zetta_finale_immerse_at + z.zetta_finale_dark_time <= z.zetta_finale_crumble_at, "full before it breaks apart");
        set_ending(None);
    }

    #[test]
    fn a_killed_target_crumbles_and_a_survivor_is_thrown() {
        // the user: immersed in white; "a black silhouette that crumbles" only when Zettaflare kills it, else the throw
        for (hp, killed) in [(1.0e6f32, false), (500.0, true)] {
            let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
            rig.dummies.hp = hp;
            set_ending(Some(false));
            rig.cast();
            rig.run_for(16.5);
            let z = z();
            let log = &rig.party.don_zetta.log;
            assert_eq!(log.iter().any(|e| matches!(e, ZettaLog::Killed { .. })), killed, "hp {hp}");
            assert_eq!(log.iter().any(|e| matches!(e, ZettaLog::Reaction { .. })), !killed, "thrown only when it survives");
            let looks: Vec<kh_core::host::ScriptFinale> = rig.dummies.finales.iter().filter_map(|f| f.1).collect();
            assert!(!looks.is_empty(), "the finale's look was asked");
            let peak = looks.iter().position(|f| f.immerse > 0.9999).expect("fully engulfed");
            assert!(looks[..=peak].windows(2).all(|w| w[1].immerse >= w[0].immerse - 1e-6), "the white rises");
            if killed {
                assert!((looks.last().unwrap().immerse - 1.0).abs() < 1e-4, "it stays in the light as it crumbles");
            } else {
                assert!(looks.last().unwrap().immerse < 1e-3, "(round 5) a survivor comes out of the light as it is thrown");
            }
            assert_eq!(looks.iter().any(|f| f.silhouette), killed);
            if killed {
                assert!((looks.iter().map(|f| f.crumble).fold(0.0, f32::max) - 1.0).abs() < 1e-4, "crumbled away");
                assert!(looks.windows(2).all(|w| w[1].crumble >= w[0].crumble - 1e-6));
            } else {
                assert!(looks.iter().all(|f| f.crumble == 0.0));
            }
            assert_eq!(rig.dummies.finales.last().unwrap().1, None, "the look given back at the end (the restore path)");
            let t0 = seq_time(z.zetta_finale_immerse_at, &z, 5.0);
            assert!(t0 > 8.1 && t0 < 14.9);
        }
    }

    #[test]
    fn an_interrupted_or_cancelled_finale_restores_the_target() {
        // a run that stops for any reason gives the target its own look back
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.cast();
        rig.run_for(12.0);
        assert!(rig.dummies.finales.iter().any(|f| f.1.is_some()));
        rig.party.member_mut(rig.don).unwrap().status.hp = 0;
        rig.run_for(0.5);
        assert_eq!(rig.dummies.finales.last().unwrap().1, None);
    }

    // ------------------------------------------------------------------------------------ (round 5) the ending

    /// A view's look direction (UE rotator) and whether a point is inside its horizontal / vertical half-angles.
    fn sees(v: &ZettaView, p: Vec3) -> bool {
        let (h, vv) = view_angles(v.position, v.rotation, p);
        h.abs() < v.fov * 0.5 && vv.abs() < vfov_of(v.fov) * 0.5
    }

    #[test]
    fn the_ending_plays_the_wide_shot_of_the_beam_then_the_shot_behind_donald_and_hands_back() {
        // the user: "The end needs to look exactly like this, with the beam slowly shrinking and the end shot behind donald
        // is where gameplay resumes": in white the wide shot (960) of the beam running up into the sky from his staff,
        // then the shot behind him (970), then the camera eases back and he is free
        for enemy in [Vec3::new(800.0, 0.0, 0.0), Vec3::new(-300.0, 700.0, 0.0), Vec3::new(1500.0, -1500.0, 0.0)] {
            let mut rig = Rig::new(enemy, 1.0 / 60.0);
            kh_core::host::cinematic::reset(0.0);
            rig.cast();
            let z = z();
            let dt = rig.dt;
            let end0 = seq_time(z.zetta_end_start, &z, 5.0);
            let handoff = seq_time(z.zetta_end_handoff_at, &z, 5.0);
            let mut t = dt;
            let mut seen: Vec<(&str, f32)> = Vec::new();
            let mut whites: Vec<(f32, f32)> = Vec::new();
            let mut looks: Vec<(f32, kh_core::host::cinematic::Look)> = Vec::new();
            let mut feet_at_end: Option<Vec3> = None;
            while t < handoff + 3.0 {
                // the stick pushed through the ending: he stays put (the cast clip's end freed him before it)
                let stick = if t > end0 + 0.1 && t < handoff - 0.1 { Vec2::new(0.0, 1.0) } else { Vec2::ZERO };
                rig.step_pad(stick, &[]);
                // the host's look step (VFXR: once a frame)
                if let Some(a) = rig.party.don_zetta.look_asked { looks.push((t, a.0)); }
                if let Some(v) = rig.party.don_zetta_view(dt, 0.0) {
                    if seen.last().map(|s| s.0) != Some(v.shot) {
                        seen.push((v.shot, t));
                    }
                    whites.push((t, v.white));
                    let p = rig.party.member(rig.don).unwrap();
                    let chest = p.position() + Vec3::Z * 60.0;
                    if (v.shot == "shot960" || v.shot == "shot970") && v.weight > 0.999 {
                        assert!(v.exact, "the event's shot exactly");
                        assert!(sees(&v, chest), "{} sees Donald (enemy {enemy}) at {t}", v.shot);
                        if v.shot == "shot970" {
                            // behind him, looking along the beam's line
                            if let Some(fr) = rig.party.don_zetta.runs.first().and_then(|r| r.end_frame) {
                                let back = fr.vector(Vec3::X).dot(chest - v.position);
                                assert!(back > 1000.0, "behind him: {back}");
                            }
                        }
                    }
                }
                if t > end0 + 0.05 && t < handoff - 0.05 {
                    let p = rig.party.member(rig.don).unwrap();
                    assert!(holding(p) && p.don_zetta.ending, "held in his pose through the ending at {t}");
                    assert_eq!(p.anim.clip.sequence, END_CLIP.sequence, "his kg702c pose");
                    let f = *feet_at_end.get_or_insert(p.position());
                    assert!(crate::pawn::horizontal(p.position() - f).length() < 1.0, "rooted at {t}");
                }
                t += dt;
            }
            let names: Vec<&str> = seen.iter().map(|s| s.0).collect();
            assert_eq!(&names[names.len() - 2..], ["shot960", "shot970"], "{names:?}");
            let at960 = seen.iter().find(|s| s.0 == "shot960").unwrap().1;
            let at970 = seen.iter().find(|s| s.0 == "shot970").unwrap().1;
            assert!((at960 - end0).abs() < 2.0 * dt && (at970 - (end0 + 7.1667)).abs() < 2.0 * dt, "{at960} {at970}");
            // the white: up to 1 as kg702b ends, down to 0 over kg702c's first 0.667 s
            let w = |at: f32| whites.iter().min_by(|a, b| (a.0 - at).abs().total_cmp(&(b.0 - at).abs())).unwrap().1;
            assert!(w(end0 - 1.5) < 1e-3 && w(end0) > 0.97 && w(end0 + 0.8) < 1e-3, "{} {} {}", w(end0 - 1.5), w(end0), w(end0 + 0.8));
            // THE LOOK (VFXR's cinematic API): the event's exposure and key light, the bloom threshold, back after the hand-off
            let l = |at: f32| looks.iter().min_by(|a, b| (a.0 - at).abs().total_cmp(&(b.0 - at).abs())).unwrap().1;
            let near = |a: f32, b: f32| (a - b).abs() < 0.02;
            assert!(near(l(1.0).exposure_ev, -0.6) && near(l(1.0).key_ev, -1.15), "the channel: {:?}", l(1.0));
            assert!(near(l(10.0).exposure_ev, -0.6) && near(l(10.0).key_ev, -0.35) && near(l(10.0).bloom_threshold, 5.0), "the blast: {:?}", l(10.0));
            assert!(near(l(end0 + 1.0).exposure_ev, -1.0), "the wide shot: {:?}", l(end0 + 1.0));
            assert_eq!(l(handoff + 2.5), kh_core::host::cinematic::Look::NEUTRAL, "given back after the hand-off");
            // the run ended at the hand-off; (2026-10-23) he goes down (the cutscene's collapse), then is back to his idle
            let ended = rig.time_of(|e| matches!(e, ZettaLog::Ended { .. })).expect("ended");
            assert!((ended - handoff).abs() < 2.0 * dt, "{ended} vs {handoff}");
            let p = rig.party.member(rig.don).unwrap();
            assert!(p.don_zetta.down.is_some() && !p.don_zetta.ending, "down after the hand-off");
            assert_ne!(p.anim.clip.sequence, END_CLIP.sequence);
            // the ending's effects: the beam and its light in the beam's own frame, the circle at his feet, the sky, his glow
            for row in ["end.beam.96_zfr_00", "end.beam.96_rit_00", "end.world.96_zfr_01", "end.world.97_sky_00", "end.donald.97_zfr_00"] {
                let (ts, _, _) = rig.spawn_of(row).unwrap_or_else(|| panic!("{row}"));
                let want = seq_time(DON_ZETTA_FX.iter().find(|r| r.id == row).unwrap().start, &z, 5.0);
                assert!((ts - want).abs() < 2.0 * dt, "{row} at {ts} want {want}");
            }
            assert!(rig.audio.played.iter().any(|s| s.ends_with("se_kg702c_cut960_zetta_end")), "the ending's sound");
        }
    }

    #[test]
    fn the_ending_beam_runs_from_his_staff_up_the_line_to_the_target() {
        // (round 5, the user: "ALL of the beam effects and aftermath effects need to be repositioned where the beam was
        // actually aimed") the ending's beam IS the beam: from the wand's tip along the line to where it was aimed (low,
        // level or high), its cards starting at the tip
        for enemy in [Vec3::new(800.0, 300.0, 0.0), Vec3::new(700.0, -200.0, 1200.0), Vec3::new(2500.0, 900.0, -300.0)] {
            let mut rig = Rig::new(enemy, 1.0 / 60.0);
            rig.cast();
            let z = z();
            rig.run_for(seq_time(z.zetta_end_start, &z, 5.0) + 0.5);
            let run = rig.party.don_zetta.runs[0].clone();
            let b = run.end_beam.expect("the ending beam");
            let hit = run.beam_at_hit.expect("the beam at the hit");
            assert!(b.x.dot(hit.x) > 0.9999, "{enemy}: on the aim line");
            let aim = hit.origin;
            let off = b.local(aim);
            assert!(off.y.abs() < 1.0 && off.z.abs() < 1.0 && off.x > 0.0, "{enemy}: the aim point lies on it ahead of the tip ({off})");
            let (_, at, rot) = rig.spawn_of("end.beam.96_zfr_00").unwrap();
            let [_, _, zz] = rot_axes(rot);
            assert!(zz.dot(b.x) > 0.999, "its length along the beam");
            assert!(close(at - zz * 450.0, b.origin, 1.0), "its cards start at the tip");
        }
    }

    #[test]
    fn the_survivor_waits_through_the_ending_and_is_let_go_at_the_hand_off() {
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.cast();
        let z = z();
        let handoff = seq_time(z.zetta_end_handoff_at, &z, 5.0);
        rig.run_for(handoff - 0.5);
        let thrown = rig.time_of(|e| matches!(e, ZettaLog::Reaction { .. })).expect("thrown");
        assert!(rig.party.don_zetta_reserved().contains(&TargetId(1)), "held again after the throw");
        assert_eq!(rig.party.don_zetta_zones().len(), 2, "the allies keep away again");
        let last_given = rig.dummies.holds.iter().rposition(|h| h.1.is_none()).unwrap();
        assert!(rig.dummies.holds[last_given + 1..].iter().all(|h| h.1.is_some()));
        assert!(thrown + z.zetta_end_rehold_delay < handoff);
        rig.run_for(1.0);
        assert_eq!(rig.dummies.holds.last().unwrap().1, None, "let go at the hand-off");
        assert!(rig.party.don_zetta_reserved().is_empty() && rig.party.don_zetta_zones().is_empty());
        // the white on it left with the beam
        let looks: Vec<kh_core::host::ScriptFinale> = rig.dummies.finales.iter().filter_map(|f| f.1).collect();
        assert!(looks.iter().any(|f| f.immerse > 0.99), "engulfed");
        assert!(looks.last().unwrap().immerse < 1e-3, "the white gone with the beam");
        assert_eq!(rig.dummies.finales.last().unwrap().1, None);
    }

    #[test]
    fn a_hit_or_the_camera_stick_during_the_ending_frees_him_at_once() {
        for by_hit in [true, false] {
            let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
            rig.cast();
            let z = z();
            let dt = rig.dt;
            let end0 = seq_time(z.zetta_end_start, &z, 5.0);
            for _ in 0..((end0 + 1.0) / dt) as usize {
                rig.step();
                rig.party.don_zetta_view(dt, 0.0);
            }
            assert!(rig.party.member(rig.don).unwrap().don_zetta.ending);
            if by_hit {
                rig.party.member_mut(rig.don).unwrap().enter(NpcStateId::Damage);
            } else {
                for _ in 0..30 {
                    rig.step();
                    rig.party.don_zetta_view(dt, 1.0);
                }
            }
            rig.step();
            let v = rig.party.don_zetta_view(dt, 0.0);
            let p = rig.party.member(rig.don).unwrap();
            assert!(!p.don_zetta.ending && !holding(p), "free (by hit {by_hit})");
            assert!(rig.party.don_zetta_reserved().is_empty());
            if by_hit {
                assert!(v.is_none(), "a hit hands the camera back at once");
                assert_eq!(rig.party.don_zetta.look_asked, Some((kh_core::host::cinematic::Look::NEUTRAL, 0.0)), "and the look at once");
            }
        }
    }

    #[test]
    fn his_eyes_are_shut_through_the_channel_and_open_for_the_cast() {
        // the user: "Donald's eyes need to be closed when he's charging up the zettaflare": the event's channel face (eye row
        // n_ex001_fe202, eyes shut) on his face slot from the first step to the cast, one looping clip (no blink), then the
        // cast's own face track (the event's: eyes open with "Zettaflare!"); an ultimate's cast gives his face back
        for ult in [false, true] {
            let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
            let z = z();
            let cast_at = if ult {
                let p = rig.party.member_mut(rig.don).unwrap();
                ultimate_begin(p, element_tint(Element::Fire), CastTarget::Enemy(TargetId(1)), &mut PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects }).unwrap();
                RunKind::Ult { tint: element_tint(Element::Fire) }.seq(z.zetta_cast_start, &z, z.zetta_channel_span)
            } else {
                rig.cast();
                seq_time(z.zetta_cast_start, &z, 5.0)
            };
            let dt = rig.dt;
            let mut t = if ult { 0.0 } else { dt };
            let mut serials = std::collections::BTreeSet::new();
            while t < cast_at - 2.0 * dt {
                rig.step();
                t += dt;
                let c = rig.party.member(rig.don).unwrap().face.view().clip.expect("a face clip through the channel");
                assert_eq!(c.sequence, CHANNEL_FACE, "eyes shut at {t}");
                assert!(c.looping, "held, no end");
                serials.insert(c.serial);
            }
            assert_eq!(serials.len(), 1, "one play: nothing breaks it (no blink between)");
            rig.run_for(0.2);
            let after = rig.party.member(rig.don).unwrap().face.view().clip.map(|c| c.sequence);
            if ult {
                assert!(after != Some(CHANNEL_FACE), "an ultimate's cast opens them: {after:?}");
            } else {
                assert_eq!(after, Some(FACE_EN), "the cast's face track");
            }
        }
    }

    #[test]
    fn the_camera_holds_on_the_target_through_its_crumble_or_its_throw() {
        // the user: the camera always shows the target crumbling (the kill) or thrown (a survivor): both lie inside a shot of
        // the target and end before the fade to white; in play every frame of them looks at the target, near or far, big or
        // small, high or low
        let z = z();
        let shots = cam_shots();
        // (round 5) the target's shots are ONE continuous shot: from the first one's start to the next shot of another kind
        let first = shots.iter().find(|s| s.frame == "subject").expect("a target shot");
        let until = shots.iter().filter(|s| s.frame != "subject" && s.start() > first.start()).map(|s| s.start()).fold(f32::INFINITY, f32::min);
        let target_shot = |m0: f32, m1: f32| first.start() <= m0 + 1e-3 && until >= m1 - 1e-3;
        let crumble = (z.zetta_finale_crumble_at, z.zetta_finale_crumble_at + z.zetta_finale_crumble_time);
        let throw = (z.zetta_hold_until, z.zetta_hold_until + 0.9);
        assert!(target_shot(crumble.0, crumble.1), "the crumble inside a target shot: {crumble:?}");
        assert!(target_shot(throw.0, throw.1), "the throw's flight inside a target shot: {throw:?}");
        assert!(crumble.1 <= z.zetta_white_in_at && throw.1 <= z.zetta_white_in_at, "both before the white");
        for (enemy, hp) in [(Vec3::new(800.0, 0.0, 0.0), 500.0f32), (Vec3::new(800.0, 0.0, 0.0), 1.0e6), (Vec3::new(250.0, 100.0, 0.0), 500.0), (Vec3::new(2500.0, -800.0, 0.0), 1.0e6), (Vec3::new(600.0, 300.0, 900.0), 500.0)] {
            let mut rig = Rig::new(enemy, 1.0 / 60.0);
            rig.dummies.hp = hp;
            rig.cast();
            let dt = rig.dt;
            let (m0, m1) = if hp < 1.0e5 { crumble } else { throw };
            let (s0, s1) = (seq_time(m0, &z, 5.0), seq_time(m1, &z, 5.0));
            let mut t = dt;
            let mut checked = 0;
            let mut first_at: Option<f32> = None;
            while t < s1 + 0.1 {
                rig.step();
                let v = rig.party.don_zetta_view(dt, 0.0);
                // no cut from the first target shot to the finale's end
                if let Some(vv) = v.as_ref() {
                    if vv.shot == "shot920" && first_at.is_none() {
                        first_at = Some(t);
                    }
                    if first_at.is_some() && t < s1 {
                        assert_eq!(vv.shot, "shot920", "{enemy} hp {hp}: no cut at {t}");
                    }
                }
                if t > s0 + 0.02 && t < s1 - 0.02 {
                    let v = v.expect("the cinematic camera");
                    assert!(v.shot == "shot920", "{enemy} hp {hp}: a target shot at {t} ({})", v.shot);
                    let centre = rig.dummies.info(TargetId(1)).map_or(enemy + Vec3::Z * 90.0, |i| i.center);
                    assert!(sees(&v, centre), "{enemy} hp {hp}: the target in frame at {t}");
                    assert!((v.position - centre).length() > z.zetta_cam_subject_min * 0.99, "not inside it");
                    // (round 5) a low angle: the camera below its centre, above its feet, looking up
                    let feet = rig.dummies.info(TargetId(1)).map_or(enemy.z, |i| i.position.z);
                    assert!(v.position.z < centre.z && v.position.z >= feet + 24.0 && v.rotation.x > 0.0, "{enemy} hp {hp}: a low angle looking up ({} pitch {})", v.position.z - feet, v.rotation.x);
                    checked += 1;
                }
                t += dt;
            }
            assert!(checked > 30, "{checked}");
        }
    }

    #[test]
    fn every_beam_and_aftermath_effect_sits_where_the_beam_was_aimed() {
        // the user: "ALL of the beam effects and aftermath effects need to be repositioned where the beam was actually aimed":
        // from the release on, every effect placed in the world lies on the line from the wand's tip through the aim point
        // (the beam runs on past it), or round the impact; the haze of the ending far out along the aim's bearing
        for enemy in [Vec3::new(800.0, 0.0, 0.0), Vec3::new(250.0, 120.0, 0.0), Vec3::new(3000.0, -900.0, 0.0), Vec3::new(600.0, 300.0, 900.0), Vec3::new(900.0, -400.0, -350.0), Vec3::new(-100.0, 900.0, 0.0)] {
            let mut rig = Rig::new(enemy, 1.0 / 60.0);
            rig.cast();
            let z = z();
            rig.run_for(seq_time(z.zetta_end_handoff_at, &z, 5.0) + 0.5);
            let release = seq_time(z.zetta_release_at, &z, 5.0);
            let p = rig.party.member(rig.don).unwrap();
            let goal = enemy + Vec3::Z * 90.0;
            let toward = crate::pawn::yaw_of(crate::pawn::horizontal(goal - p.position()));
            let tip = Frame { origin: p.position(), yaw: toward + EVENT_DONALD_YAW_OFF }.point(staff_at(&z, z.zetta_hit_at));
            let dir = (goal - tip).normalize();
            let mut checked = 0;
            for e in &rig.party.don_zetta.log {
                let ZettaLog::Spawn { row, t, at, .. } = e else { continue };
                if *row == "dust.ground" || *row == "dust.wide" || *row == "dust.c100" {
                    // (round 5c) the ground dust: round the impact's foot or down the beam's ground track
                    let foot = Vec3::new(goal.x, goal.y, 0.0);
                    let near = (*at - foot).truncate().length() < z.zetta_dust_lane * 2.5 + 50.0 || seg_dist(Vec3::new(at.x, at.y, 0.0), Vec3::new(p.position().x, p.position().y, 0.0), foot) < z.zetta_dust_lane * 1.5 + 50.0;
                    assert!(near, "{enemy}: the dust under the beam ({at})");
                    checked += 1;
                    continue;
                }
                if row.starts_with("spikes.") {
                    // (round 5c) the rock spikes and their crumble: round the impact's foot, or down the beam's ground track
                    let foot = Vec3::new(goal.x, goal.y, 0.0);
                    // (their broken rocks and dust come down a little further out)
                    let fell = if *row == "spikes.rock" { 0.0 } else { 450.0 };
                    let ring = (*at - foot).truncate().length() < z.zetta_spikes_radius * (1.0 + z.zetta_spikes_spread) * 1.05 + fell;
                    let track = seg_dist(Vec3::new(at.x, at.y, 0.0), Vec3::new(p.position().x, p.position().y, 0.0), foot) < z.zetta_spikes_trail_lane * 1.4 + 150.0 + fell;
                    assert!(ring || track, "{enemy}: a spike round the impact or down the beam ({at})");
                    checked += 1;
                    continue;
                }
                let r = DON_ZETTA_FX.iter().find(|x| x.id == *row).unwrap();
                if *t + 1e-3 < release || matches!(r.frame, "donald_socket" | "weapon") {
                    continue;
                }
                let along = (*at - tip).dot(dir);
                let off = (*at - tip - dir * along).length();
                let impact = (*at - goal).length();
                let ok = if r.id == "end.world.97_sky_00" {
                    // (round 5c) the final cloud: its clear centre beyond the impact, on the line from behind Donald through it
                    let back = goal - p.position();
                    let away = *at - goal;
                    crate::pawn::horizontal(away).normalize_or_zero().dot(crate::pawn::horizontal(back).normalize_or_zero()) > 0.98 && away.length() > 1000.0
                } else if r.id == "end.world.96_zfr_01" {
                    (*at - p.position()).length() < 50.0 // the shock and circle at his feet: the beam's source
                } else {
                    impact <= z.zetta_blast_radius || (along > -200.0 && off < 300.0)
                };
                assert!(ok, "{enemy}: {} at {at} (along {along:.0}, off {off:.0}, from the impact {impact:.0})", r.id);
                checked += 1;
            }
            assert!(checked >= 15, "{enemy}: {checked}");
        }
    }

    #[test]
    fn no_zettaflare_or_ultimate_in_the_air() {
        // the user: "Donald can not perform any ultimate magic or zettaflare while up in the air, he must be on the ground"
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        assert_eq!(debug_make_ready(&mut rig.party), 1);
        {
            let p = rig.party.member_mut(rig.don).unwrap();
            p.cmc.location.z += 400.0;
            p.cmc.set_falling();
            assert!(!p.on_ground());
            let r = ultimate_begin(p, element_tint(Element::Fire), CastTarget::Enemy(TargetId(1)), &mut PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects });
            assert!(r.is_none(), "the ultimate's presentation refuses in the air");
        }
        rig.party.don_fx.debug_casts.push(SpellRef::Arcane(ArcaneLine::Zettaflare, 1));
        rig.party.don_zetta.take_log();
        rig.step();
        assert!(rig.party.don_zetta.runs.is_empty() && rig.party.don_zetta.log.iter().all(|e| !matches!(e, ZettaLog::Begun { .. })), "refused in the air");
        // on the ground again: it casts
        rig.run_for(2.0);
        assert!(rig.party.member(rig.don).unwrap().on_ground());
        rig.cast();
        assert!(rig.party.don_zetta.log.iter().any(|e| matches!(e, ZettaLog::Begun { .. })));
    }

    #[test]
    fn the_allies_never_attack_the_target_and_keep_away_from_donald_and_the_beam() {
        // the user: "Make SURE ally's DO NOT attack the zettaflared enemy or get anywhere close to Donald or the Zettaflare
        // beam while he is casting it or charging it up": an AI Goofy starting next to Donald, next to the target or on the
        // beam's line leaves the zones within a short walk and stays out to the end; no hit of his lands on the target
        let enemy = Vec3::new(900.0, 0.0, 0.0);
        for start in [Vec3::new(80.0, 60.0, 0.0), Vec3::new(850.0, 60.0, 0.0), Vec3::new(450.0, 20.0, 0.0)] {
            let mut rig = Rig::new(enemy, 1.0 / 60.0);
            let goofy = rig.party.spawn(Chara::Goofy, start, 0.0);
            rig.party.member_mut(goofy).unwrap().in_battle = true;
            rig.cast();
            let z = z();
            let end = seq_time(z.zetta_end_handoff_at, &z, 5.0);
            let mut t = rig.dt;
            let mut out_at: Option<f32> = None;
            while t < end - 0.1 {
                rig.step();
                let p = rig.party.member(goofy).unwrap().position();
                let zones = rig.party.don_zetta_zones();
                assert_eq!(zones.len(), 2, "the target's zone (with the beam's corridor) and Donald's, the whole sequence");
                let inside = zones.iter().any(|zn| zn.inside(p));
                if out_at.is_none() && !inside {
                    out_at = Some(t);
                }
                if let Some(o) = out_at {
                    if t > o + 0.5 {
                        assert!(!inside, "{start}: back in a zone at {t}");
                    }
                }
                assert!(rig.party.don_zetta_reserved().contains(&TargetId(1)), "the target reserved the whole sequence");
                t += rig.dt;
            }
            assert!(out_at.is_some_and(|o| o < 6.0), "{start}: out within 6 s: {out_at:?}");
            assert!(rig.dummies.hits.iter().filter(|h| h.target == TargetId(1)).all(|h| h.flags & kh_core::host::HIT_FLAG_FORCE != 0), "{start}: only Zettaflare's own (forced) hits on the target");
        }
    }

    #[test]
    fn the_beam_engulfs_the_enemy_whatever_its_size() {
        // the user: "The Zettaflare beam should encompass the entire enemy, it adapts to the enemy's size": a man-sized enemy
        // gets the event's width, a 5 m+ giant a beam as many times wider; (2026-10-23, "vice versa, if the target is small
        // then the camera zooms in and the effects scale") a small one a thinner beam; its impact effects follow it
        // (2026-10-23, "the size the beam is when it attacks the Sora in the test level the minimum size the beam can be") a
        // 0.5 m one gets exactly the Sora target's beam (dummy 1: 1.8 m tall, 40 cm radius)
        // (2026-10-23, the coordinator: the whole body fitted in the reference's region) the Sora case: the test level's Sora
        // in his hurt pose (fit 1); a standing 1.8 m capsule fits its full height: 180 / zetta.frame_body_height
        let sora = beam_girth(sora_case().1, sora_case().0, &z());
        assert!((z().zetta_beam_scale_min - sora).abs() < 1e-4, "zetta.beam_scale_min is the Sora case ({sora})");
        let fh = z().zetta_frame_body_height;
        for (half, radius, want) in [(25.0f32, 15.0f32, sora), (90.0, 50.0, 180.0 / fh), (300.0, 120.0, 600.0 / fh)] {
            let mut rig = Rig::new(Vec3::new(900.0, 0.0, 0.0), 1.0 / 60.0);
            rig.dummies.half = half;
            rig.dummies.radius = radius;
            rig.cast();
            let z = z();
            rig.run_for(seq_time(z.zetta_hit_at, &z, 5.0) + 0.5);
            let run = &rig.party.don_zetta.runs[0];
            assert!((run.girth - want).abs() < 1e-3, "{half}: girth {} want {want}", run.girth);
            // the stream's last placements: its width x the girth (built: 1)
            let (h93, ..) = rig.effects.spawned.iter().find(|s| s.1 == "vd_kg702_93_zfr_00").copied().expect("the stream");
            let w = rig.effects.transformed.iter().rev().find(|t| t.0 == h93).map(|t| t.3.y).unwrap();
            assert!((w - want).abs() < 0.02 * want, "{half}: the beam {w} wide, want {want}");
        }
    }

    #[test]
    fn the_ending_beam_starts_at_the_enemy_shots_size_and_shrinks_to_nothing_on_time() {
        // (2026-10-23) the user: "the beam should be bigger in the zoomed out shot as well, the size it was to cover the enemy
        // but it still shrinks to nothing at the same speed": the ending beam and everything shrinking with it start at the
        // enemy shot's beam size (a multiplier on their own width: their own shrink, same time, same curve), the wide shot
        // framed about his feet by the same factor
        let z = z();
        for (half, radius) in [(90.0f32, 40.0f32), (225.0, 100.0), (450.0, 200.0)] {
            let mut rig = Rig::new(Vec3::new(1500.0, 0.0, 0.0), 1.0 / 60.0);
            rig.dummies.half = half;
            rig.dummies.radius = radius;
            set_ending(Some(true));
            rig.cast();
            rig.run_for(seq_time(z.zetta_end_start, &z, 5.0) + 1.0);
            set_ending(None);
            let g = rig.party.don_zetta.runs[0].girth;
            assert!((g - beam_girth(half, radius, &z)).abs() < 1e-3, "{half}: the enemy shot's girth {g}");
            // the ending beam born at that scale (x its own row scale); its shrink is its own animation (unchanged)
            let ends: Vec<_> = rig.effects.spawned.iter().filter(|e| e.1.starts_with("vd_kg702b_96_zfr_00")).collect();
            assert!(!ends.is_empty(), "{half}: the ending beam");
            for e in &ends {
                let row = DON_ZETTA_FX.iter().find(|r| r.asset == e.1 && r.frame == "end_beam").unwrap();
                let sc = rig.effects.scales.iter().find(|x| x.0 == e.0).unwrap().1;
                let ovr = sc / (if row.scale > 0.0 { row.scale } else { 1.0 } * g);
                assert!((ovr - 1.0).abs() < 1e-3 || DON_ZETTA_FX.iter().any(|o| o.kind == "shot_scale" && o.asset == row.id), "{half}: born x{g} ({sc})");
            }
            // what shrinks with it: the light at the source, the same share at the same times x the girth
            let r = DON_ZETTA_FX.iter().find(|r| r.frame == "end_glow").unwrap();
            assert!((glow_share(r, r.start) - 1.0).abs() < 1e-6 && glow_share(r, r.start + r.path[0]) < 1e-6, "same curve, nothing at the same time");
        }
    }

    #[test]
    fn every_enemy_shot_is_the_reference_frame_scaled_about_the_target() {
        // (2026-10-23) the user: "THIS FRAMING IS PERFECT! EVERY ENEMY NEEDS TO BE FRAMED LIKE THIS WITH THE BEAM MATCHING THIS
        // FRAME EXACTLY NO MATTER HOW BIG THE ENEMY IS" (the test level's Sora target in the beam), "vice versa, if the target
        // is small then the camera zooms in", "make the size the beam is when it attacks the Sora in the test level the
        // minimum size the beam can be": at every moment of the enemy shot the camera is the Sora case's scaled about the
        // target by its size (same angles, same lens, the target the same share of the frame), for 0.5 m to 10 m, a Malenia
        // (2.6 m capsule, her reach); the beam is as many times the Sora case's, never under it
        let _g = GLOBALS.lock().unwrap_or_else(|e| e.into_inner());
        let z = z();
        let shots = cam_shots();
        let feet = Vec3::ZERO;
        // (2026-10-23) the Sora case: the test level's Sora in his hurt pose, the body that fills the reference's region (fit 1)
        let sora = (sora_case().1, sora_case().0);
        let pose = |half: f32, radius: f32, s: f32| {
            // (one aim point for every size: the beam's line the same, so only the scale differs)
            let goal = Vec3::new(1200.0, 0.0, 90.0);
            let _ = half;
            let yaw = crate::pawn::yaw_of(crate::pawn::horizontal(goal - feet));
            let donald = Frame { origin: feet, yaw: yaw + EVENT_DONALD_YAW_OFF };
            let target = Frame { origin: goal, yaw };
            let beam = Basis::beam(donald.point(staff_local(&z)), goal);
            let p = cam_pose(&shots, s, 5.0, &z, donald, target, beam, CamSubject { at: goal, size: target_size(half, radius, &z), animated: true, radius, half: 0.0 }, None).unwrap();
            (p, goal)
        };
        let malenia = (130.0 * 1.25, 57.0 * 2.2); // (her capsule with the reach kh-er adds: targets.rs drawn_extent)
        for (half, radius) in [(25.0f32, 15.0f32), sora, malenia, (250.0, 120.0), (500.0, 200.0)] {
            let k = target_size(half, radius, &z) / target_size(sora.0, sora.1, &z);
            let girth = beam_girth(half, radius, &z);
            assert!((girth - beam_girth(sora.0, sora.1, &z) * k.max(1.0)).abs() < 1e-3, "{half}/{radius}: the beam the Sora case's x {k} (never under it): {girth}");
            let mut t = seq_time(z.zetta_hit_at, &z, 5.0) + 0.05;
            let mut seen = 0;
            while t < seq_time(z.zetta_white_in_at, &z, 5.0) {
                let (p, goal) = pose(half, radius, t);
                let (r, rg) = pose(sora.0, sora.1, t);
                if p.shot == "shot920" && r.shot == "shot920" {
                    seen += 1;
                    // (the Sora case scaled about it; a small target's camera kept clear of the floor: zetta.cam_floor_clear)
                    let scaled = (r.position - rg) * k;
                    let floor = -z.zetta_target_center_height * target_size(half, radius, &z) + z.zetta_cam_floor_clear;
                    let lifted = scaled.z < floor;
                    let want = Vec3::new(scaled.x, scaled.y, scaled.z.max(floor));
                    assert!((p.position - goal - want).length() < 2.0 + 0.01 * want.length(), "{half}/{radius} at {t:.2}: the Sora case scaled x{k} ({} vs {})", p.position - goal, want);
                    assert!(lifted || (p.rotation - r.rotation).length() < 0.5, "{half}/{radius}: the same angles");
                    assert!((p.fov - r.fov).abs() < 1e-3, "{half}/{radius}: the same lens");
                    // (2026-10-23, the coordinator: "so nothing is ever cropped") its whole body (top, feet, both sides as the
                    // camera sees them) inside the frame, in no more than the reference's share of it
                    let th = (p.fov * 0.5).to_radians().tan();
                    let tv = (vfov_of(p.fov) * 0.5).to_radians().tan();
                    let sc = |q: Vec3| {
                        let (h, v) = view_angles(p.position, p.rotation, q);
                        (0.5 + h.to_radians().tan() / (2.0 * th), 0.5 - v.to_radians().tan() / (2.0 * tv))
                    };
                    let [_, side, _] = rot_axes(p.rotation);
                    let pts = [goal + Vec3::Z * half, goal - Vec3::Z * half, goal + side * radius, goal - side * radius];
                    for q in pts {
                        let (x, y) = sc(q);
                        assert!((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y), "{half}/{radius} at {t:.2}: its whole body in the frame ({x:.2}, {y:.2})");
                    }
                    let tall = (sc(pts[1]).1 - sc(pts[0]).1).abs();
                    let wide = (sc(pts[2]).0 - sc(pts[3]).0).abs();
                    assert!(tall < z.zetta_frame_share_height + 0.02 && wide < z.zetta_frame_share_width + 0.02, "{half}/{radius} at {t:.2}: the reference's share of the frame ({tall:.2} high, {wide:.2} wide)");
                }
                t += 0.2;
            }
            assert!(seen > 3, "{half}/{radius}: the enemy shot");
        }
    }

    #[test]
    fn no_camera_ever_sits_in_the_beam() {
        // (round 5c) the user, with a screenshot of the ending's wide shot: "See the bits of light coming towards the camera
        // and it's being cut off? Thats the extra beam I'm talking about that needs to be removed"; the coordinator: "No beam
        // layer may start at or pass through the camera". Every beam effect's cards lie on its line from the wand's tip
        // (crates/kh-host/tests/zetta_beam_lines.rs, the real evaluator); here, through a whole Zettaflare (near, far, to the
        // side, high, very close, a giant; killed or surviving), every frame of the cinematic camera is kept off every live
        // beam card's line: the stream cards (as placed: their start at the tip, their whole length) and the ending's beam
        // (from the tip along the aim line, 135 m), by CLEAR x the beam's girth and never less than CLEAR
        const CLEAR: f32 = 150.0;
        let z = z();
        let half_of = |asset: &str| DON_ZETTA_FX.iter().find(|r| r.asset == asset && r.frame == "beam_stream").map(|r| r.path[0]);
        let dist = |p: Vec3, a: Vec3, b: Vec3| seg_dist(p, a, b);
        for (enemy, half, radius, hp) in [
            (Vec3::new(800.0, 0.0, 0.0), 0.0f32, 0.0f32, 1.0e6f32),
            (Vec3::new(800.0, 0.0, 0.0), 0.0, 0.0, 500.0),
            (Vec3::new(-200.0, 900.0, 0.0), 0.0, 0.0, 1.0e6),
            (Vec3::new(2500.0, -800.0, 0.0), 0.0, 0.0, 500.0),
            (Vec3::new(600.0, 300.0, 900.0), 0.0, 0.0, 1.0e6),
            (Vec3::new(250.0, 100.0, 0.0), 0.0, 0.0, 1.0e6),
            (Vec3::new(1200.0, 0.0, 0.0), 300.0, 120.0, 1.0e6),
        ] {
            let mut rig = Rig::new(enemy, 1.0 / 60.0);
            rig.dummies.hp = hp;
            rig.dummies.half = half;
            rig.dummies.radius = radius;
            rig.cast();
            let dt = rig.dt;
            let handoff = seq_time(z.zetta_end_handoff_at, &z, 5.0);
            let mut t = dt;
            let (mut checked, mut cards) = (0, 0);
            while t < handoff + 1.5 {
                rig.step();
                t += dt;
                let Some(v) = rig.party.don_zetta_view(dt, 0.0) else { continue };
                if v.weight < 0.5 {
                    continue;
                }
                let girth = rig.party.don_zetta.runs.first().map_or(1.0, |r| r.girth);
                // (2026-10-23) Donald's shots: the Sora case's framing and beam width at any size ([`DONALD_SHOTS`])
                let girth = if DONALD_SHOTS.contains(&v.shot) { 1.0 } else { girth };
                let clear = CLEAR * girth.max(1.0);
                for &(h, name, at, rot, _) in &rig.effects.spawned {
                    // the end beam lives on 2 s after its stop (its cards shrink away); a stream card ends at its stop
                    let stopped = rig.effects.stopped.contains(&h);
                    let (a, b) = if let Some(half) = half_of(name) {
                        if stopped {
                            continue;
                        }
                        // (2026-10-23) in the target's shots the front stream layer is drawn near the camera on purpose (scaled
                        // about the eye: in front of it, the same picture: [`stream_drawn`]); its line is not where it is drawn
                        if SUBJECT_SHOTS.contains(&v.shot) && matches!(name, "vd_kg702_93_zfr_00" | "vd_kg702_94_zfr_01") {
                            continue;
                        }
                        let Some(&(_, o, r, s)) = rig.effects.transformed.iter().rev().find(|x| x.0 == h) else { continue };
                        let [_, _, ax] = rot_axes(r);
                        let a = o + ax * (-450.0 * s.z);
                        (a, a + ax * (2.0 * half * s.z))
                    } else if name == "vd_kg702b_96_zfr_00" {
                        if stopped && t > seq_time(z.zetta_end_start, &z, 5.0) + 9.2 {
                            continue;
                        }
                        let Attach::World(o) = at else { continue };
                        let [_, _, ax] = rot_axes(rot);
                        let a = o - ax * 450.0;
                        (a, a + ax * 13500.0)
                    } else {
                        continue;
                    };
                    cards += 1;
                    let d = dist(v.position, a, b);
                    assert!(d >= clear, "{enemy} (half {half}, hp {hp}): the camera ({}) {d:.0} cm from a card of {name} at {t:.2} (shot {})", v.position, v.shot);
                }
                checked += 1;
            }
            assert!(checked > 600 && cards > 300, "{enemy}: {checked} frames, {cards} card checks");
        }
    }

    #[test]
    fn the_allies_walk_to_their_places_at_the_waiting_spot() {
        // (round 5c) the user: "If there is only one target ... then the allies move away and wait for when the player
        // regains control of Donald": the host's waiting spots (ZettaWorld::wait_spots, from don_zetta_wait_spots) take each
        // AI member to its own place there (round the zones), where it stands until the hand-back
        let mut rig = Rig::new(Vec3::new(700.0, 0.0, 0.0), 1.0 / 60.0);
        let goofy = rig.party.spawn(Chara::Goofy, Vec3::new(250.0, -350.0, 0.0), 0.0);
        rig.cast();
        let z = z();
        let handoff = seq_time(z.zetta_end_handoff_at, &z, 5.0);
        let mut t = 0.0;
        let mut at_place = 0;
        while t < handoff - 0.5 {
            rig.party.don_zetta.wait_spots = rig.party.don_zetta_wait_spots();
            rig.step();
            t += rig.dt;
            if t > 1.0 {
                let spots = rig.party.don_zetta_wait_spots();
                assert!(!spots.is_empty(), "somewhere to wait at {t}");
                let zones = rig.party.don_zetta_zones();
                for s in &spots {
                    assert!(zones.iter().all(|zn| zn.depth(*s).0 < 0.0), "a spot out of every zone");
                }
            }
            if t > 8.0 {
                let p = rig.party.view(goofy).unwrap().position;
                let place = wait_place(&rig.party, Some(goofy), p).expect("his place");
                if (p - place).truncate().length() < 150.0 {
                    at_place += 1;
                }
                assert!(rig.party.don_zetta_zones().iter().all(|zn| !zn.inside(p)), "out of every zone at {t}");
            }
        }
        assert!(at_place > 600, "he waits at his place ({at_place} steps)");
    }

    #[test]
    fn a_low_beam_raises_rock_spikes_for_the_ending_that_crumble_before_the_hand_off() {
        // (round 5c) the user: "huge spikes coming out from the ground if the beam was close to the ground then have them
        // crumble away before control is back for Donald"; "only show the spikes in the wide shot and the shot behind donald
        // at the end, where they crumble and disintegrate": near the ground (ahead, to the side, far, a giant) they stand
        // round the impact from the ending's start (under its white: never in an earlier shot), on the ground, leaning out,
        // clear of Donald, his zone and the ending's cameras; each crumbles (its dust) in the shot behind him and is gone
        // before the hand-off; a high impact has none
        let z = z();
        let end0 = seq_time(z.zetta_end_start, &z, 5.0);
        let behind = seq_time(z.zetta_end_start + 7.1666667, &z, 5.0);
        let handoff = seq_time(z.zetta_end_handoff_at, &z, 5.0);
        for (enemy, half, want) in [(Vec3::new(800.0, 0.0, 0.0), 0.0f32, true), (Vec3::new(-200.0, 900.0, 0.0), 0.0, true), (Vec3::new(2500.0, -800.0, 0.0), 0.0, true), (Vec3::new(1400.0, 300.0, 0.0), 300.0, true), (Vec3::new(600.0, 300.0, 900.0), 0.0, false)] {
            let mut rig = Rig::new(enemy, 1.0 / 60.0);
            rig.dummies.half = half;
            rig.cast();
            let mut run = None;
            for _ in 0..((handoff + 0.5) / rig.dt) as usize {
                rig.step();
                rig.party.don_zetta_view(rig.dt, 0.0);
                if let Some(r) = rig.party.don_zetta.runs.iter().find(|r| r.caster == rig.don) {
                    run = Some(r.clone());
                }
            }
            if let Some(r) = run.as_ref() {
                // the run as it ended: its spikes all gone
                assert!(r.spikes.iter().all(|s| s.handle.is_none()) || r.done, "{enemy}: none standing at its end");
            }
            let log = &rig.party.don_zetta.log;
            let spawns: Vec<(f32, Vec3)> = log.iter().filter_map(|e| match e { ZettaLog::Spawn { row: "spikes.rock", t, at, .. } => Some((*t, *at)), _ => None }).collect();
            let dusts: Vec<f32> = log.iter().filter_map(|e| match e { ZettaLog::Spawn { row: "spikes.dust", t, .. } => Some(*t), _ => None }).collect();
            let stops: Vec<f32> = log.iter().filter_map(|e| match e { ZettaLog::Stop { row: "spikes.rock", t } => Some(*t), _ => None }).collect();
            if !want {
                assert!(spawns.is_empty(), "{enemy}: a high impact raises none");
                continue;
            }
            assert!(spawns.len() >= 5 && spawns.len() <= (z.zetta_spikes_count + z.zetta_spikes_trail_max) as usize, "{enemy}: {} spikes", spawns.len());
            let spikes = run.map(|r| r.spikes).unwrap_or_default();
            let foot = Vec3::new(enemy.x, enemy.y, 0.0);
            let don = rig.party.member(rig.don).unwrap().position();
            for (t, at) in &spawns {
                // (round 5c) laid out under the ground in the target shot, bursting out from `zetta.spikes_rise_at`
                let rise0 = seq_time(z.zetta_spikes_rise_at, &z, 5.0);
                assert!(*t + 1e-3 >= rise0 && *t < rise0 + 0.1 && *t < end0, "{enemy}: laid out at the eruption ({t})");
                assert!(at.z.abs() < 80.0, "{enemy}: its foot in the ground ({at})");
                // (round 5c, the user: "the rocks at the end need to travel all the way down the beam") clear of his body only
                assert!((*at - don).truncate().length() > 90.0, "{enemy}: clear of Donald's body");
            }
            // the beam's ground track, from his feet to the impact's foot
            let a = Vec3::new(don.x, don.y, 0.0);
            let track = (foot - a).truncate();
            let mut trail = 0;
            for s in &spikes {
                let rel = (s.foot - a).truncate();
                let t = rel.dot(track.normalize());
                let off = rel - track.normalize() * t;
                let near_impact = (s.foot - foot).truncate().length() < z.zetta_spikes_radius * 1.4 * if half > 0.0 { 2.5 } else { 1.0 };
                if near_impact {
                    let out = (s.foot - foot).truncate().normalize_or_zero();
                    assert!(s.up.truncate().dot(out) > 0.1 || s.up.truncate().dot(off.normalize_or_zero()) > 0.1, "{enemy}: leaning out ({})", s.up);
                } else {
                    // the trail: along the track, either side of it, leaning away from the line
                    trail += 1;
                    assert!(t > 0.0 && t < track.length(), "{enemy}: a trail spike along the track ({t} of {})", track.length());
                    assert!(s.up.truncate().dot(off.normalize_or_zero()) > 0.1, "{enemy}: leaning away from the beam ({})", s.up);
                }
                assert!(s.up.z > 0.7 && s.height > 100.0, "{enemy}: upright and tall ({})", s.height);
            }
            // (round 5c) "they should already be there when the camera cuts to the wide shot of the beam shrinking, no pop in":
            // each has burst out and settled before the wide shot; each eruption heard (in the target shot)
            let rise0 = seq_time(z.zetta_spikes_rise_at, &z, 5.0);
            for s in &spikes {
                assert!(rise0 + spike_rise_delay(s, &z) + z.zetta_spikes_rise_time <= end0 - 0.15, "{enemy}: standing before the wide shot");
            }
            let rises: Vec<f32> = log.iter().filter_map(|e| match e { ZettaLog::Sound { row: "spk.se.rise", t } => Some(*t), _ => None }).collect();
            assert!(rises.len() == spikes.len() && rises.iter().all(|t| *t >= rise0 - 1e-3 && *t < end0), "{enemy}: each eruption heard in the target shot ({rises:?})");
            assert_eq!(rig.audio.played.iter().filter(|p| p.ends_with("se_he02_ms061s_cut06_collapse1")).count(), 1, "{enemy}: the ground rumbles once");
            assert!((spike_rise(&spikes[0], 0.0).0 - spikes[0].foot).dot(spikes[0].up) < -spikes[0].height * 0.9 && (spike_rise(&spikes[0], 1.0).0 - spike_transform(&spikes[0], 0.0).0).length() < 1e-3, "{enemy}: from under the ground to standing");
            // the whole way: from just outside his body to the impact, no stretch empty
            let mut alongs: Vec<f32> = spikes.iter().map(|s| (s.foot - a).truncate().dot(track.normalize())).filter(|t| *t < track.length() - z.zetta_spikes_radius * 0.5).collect();
            alongs.sort_by(|x, y| x.partial_cmp(y).unwrap());
            if track.length() > 900.0 {
                let step = z.zetta_spikes_trail_step * if half > 0.0 { (1.0f32 + half / 90.0).min(2.5).sqrt() } else { 1.0 };
                assert!(alongs.first().is_some_and(|t| *t < 90.0 + step * 1.2), "{enemy}: from right by him ({alongs:?})");
                assert!(alongs.windows(2).all(|w| w[1] - w[0] < step * 2.2), "{enemy}: no empty stretch ({alongs:?})");
            }
            if track.length() > 1500.0 {
                assert!(trail >= 3, "{enemy}: the spikes run down the beam ({trail} in the trail)");
            }
            // (round 5c, the user: "the rocks need to physically crumble to the ground, actually breaking apart") each cracks
            // (its dust), breaks into its own rocks (the big ones split again as they land: their plumes), which crumble away;
            // all in the shot behind Donald, all gone before the hand-off, every sound heard
            let late = dusts.iter().filter(|t| **t >= behind - 1e-3).count();
            assert!(late >= spawns.len() * 3, "{enemy}: each cracks, its rocks land, its pile crumbles ({late} dusts)");
            assert!(dusts.iter().all(|t| (*t >= behind - 1e-3 && *t < handoff) || *t < end0 - 0.15), "{enemy}: its eruption dust in the target shot (settling before the wide shot), the rest in the shot behind Donald");
            assert!(stops.len() == spawns.len() && stops.iter().all(|t| *t <= handoff - 0.2), "{enemy}: all gone before the hand-off: {stops:?}");
            let pieces: Vec<f32> = log.iter().filter_map(|e| match e { ZettaLog::Spawn { row: "spikes.piece", t, .. } => Some(*t), _ => None }).collect();
            let gone: Vec<f32> = log.iter().filter_map(|e| match e { ZettaLog::Stop { row: "spikes.piece", t } => Some(*t), _ => None }).collect();
            assert!(pieces.len() >= spawns.len() * (SPIKE_ROCKS.len() + 3), "{enemy}: each spike's rocks, the big ones in two ({} pieces)", pieces.len());
            assert!(gone.len() == pieces.len() && gone.iter().all(|t| *t <= handoff - 0.2), "{enemy}: every rock gone before the hand-off");
            let heard = |name: &str| rig.audio.played.iter().filter(|p| p.ends_with(name)).count();
            assert_eq!(heard("se_g_bt_break_stone_big_01"), spawns.len(), "{enemy}: each spike's crack");
            assert!(heard("se_g_bt_break_stone_small_01") >= spawns.len() * 2, "{enemy}: its rocks' thuds");
            assert_eq!(heard("se_g_bt_break_stone_mid_01"), spawns.len(), "{enemy}: its rubble");
            assert_eq!(heard("se00100_042"), spawns.len(), "{enemy}: its pile crumbling");
        }
    }

    #[test]
    fn with_the_ending_the_target_is_held_in_place_until_the_wide_shot_and_only_thrown_there() {
        // (round 5c) the user: "the zettaflare only pushes the enemy back only in the shot of the beam when it's shrinking,
        // otherwise hold them in place for the cutscene": held every step from the channel to the wide shot (no push, no
        // drift), the survivor's throw (the beam's push back) inside the wide shot of the beam shrinking; a kill is never
        // thrown; without the ending, the throw stays as the beam ends
        let z = z();
        let end0 = seq_time(z.zetta_end_start, &z, 5.0);
        let behind = seq_time(z.zetta_end_start + 7.1666667, &z, 5.0);
        for (hp, ending) in [(1.0e6f32, true), (500.0, true), (1.0e6, false)] {
            let mut rig = Rig::new(Vec3::new(900.0, 0.0, 0.0), 1.0 / 60.0);
            rig.dummies.hp = hp;
            set_ending(Some(ending));
            rig.cast();
            rig.run_for(if ending { behind + 0.5 } else { 16.5 });
            let thrown = rig.time_of(|e| matches!(e, ZettaLog::Reaction { .. }));
            let holds: Vec<_> = rig.dummies.holds.iter().filter(|h| h.0 == TargetId(1)).collect();
            let first_free = holds.iter().position(|h| h.1.is_none());
            if hp < 1.0e5 {
                assert!(thrown.is_none(), "a kill is never thrown");
                continue;
            }
            let t = thrown.expect("a survivor is thrown");
            if ending {
                assert!(t >= end0 - 1e-3 && t < behind, "thrown in the wide shot ({t}, {end0} .. {behind})");
                // held every step up to the throw: the first release is the throw's
                let held = holds[..first_free.expect("given back")].len() as f32 / 60.0;
                assert!((held - t).abs() < 0.1, "held in place every step until the throw ({held} s vs {t})");
                // the beam's push (the throw's hit) only then
                let pushes = rig.dummies.hits.iter().filter(|h| h.target == TargetId(1) && h.info.move_length > 0.0).count();
                assert!(pushes >= 1);
            } else {
                assert!((t - seq_time(z.zetta_hold_until, &z, 5.0)).abs() < 0.05, "without the ending, as the beam ends");
            }
        }
        set_ending(None);
    }

    #[test]
    fn the_beam_kicks_up_ground_dust_for_the_end_shots_that_thins_by_the_hand_off() {
        // (round 5c) the user: "Zettaflare should kick up dust from the ground as well, visible at the two end shots": laid
        // as the ending begins (under its white), along the beam's ground track and round the impact, on the ground, clear of
        // Donald and his zone; it stops rising in time to thin out before the hand-off; a high beam kicks up none
        let z = z();
        let end0 = seq_time(z.zetta_end_start, &z, 5.0);
        let handoff = seq_time(z.zetta_end_handoff_at, &z, 5.0);
        for (enemy, want) in [(Vec3::new(1600.0, 0.0, 0.0), true), (Vec3::new(-300.0, 1200.0, 0.0), true), (Vec3::new(600.0, 300.0, 2500.0), false)] {
            let mut rig = Rig::new(enemy, 1.0 / 60.0);
            rig.cast();
            rig.run_for(handoff + 0.5);
            let log = &rig.party.don_zetta.log;
            // the wide shot's big billows as the ending begins, thinned away by the cut; the near ones for the shot behind him
            let laid: Vec<(f32, Vec3)> = log.iter().filter_map(|e| match e { ZettaLog::Spawn { row: "dust.wide", t, at, .. } => Some((*t, *at)), _ => None }).collect();
            let wide_stops: Vec<f32> = log.iter().filter_map(|e| match e { ZettaLog::Stop { row: "dust.wide", t } => Some(*t), _ => None }).collect();
            let near: Vec<f32> = log.iter().filter_map(|e| match e { ZettaLog::Spawn { row: "dust.ground", t, .. } => Some(*t), _ => None }).collect();
            let stops: Vec<f32> = log.iter().filter_map(|e| match e { ZettaLog::Stop { row: "dust.ground", t } => Some(*t), _ => None }).collect();
            let behind = seq_time(z.zetta_end_start + 7.1666667, &z, 5.0);
            assert!(wide_stops.len() == laid.len() && wide_stops.iter().all(|t| *t <= behind - 2.4), "{enemy}: the wide billows thin away by the cut");
            assert!(near.len() == laid.len().div_ceil(2) && near.iter().all(|t| *t < behind), "{enemy}: the near billows rise before the cut");
            if !want {
                assert!(laid.is_empty(), "{enemy}: a high beam kicks up none");
                continue;
            }
            assert!(laid.len() >= 6 && laid.len() <= z.zetta_dust_max as usize, "{enemy}: {} billows", laid.len());
            // evenly the whole way: no empty stretch of the track (from right by him to the impact)
            let don0 = rig.party.member(rig.don).unwrap().position();
            let track = Vec3::new(enemy.x - don0.x, enemy.y - don0.y, 0.0);
            let mut along: Vec<f32> = laid.iter().map(|(_, at)| Vec3::new(at.x - don0.x, at.y - don0.y, 0.0).dot(track.normalize())).collect();
            along.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert!(along.first().is_some_and(|a| *a < 450.0), "{enemy}: from near him ({along:?})");
            assert!(along.windows(2).all(|w| w[1] - w[0] < z.zetta_dust_step * 2.2), "{enemy}: no empty stretch ({along:?})");
            // the event's own dust (kg702c cut 1000) at each place, in waves through the wide shot, none after it
            let c100: Vec<f32> = log.iter().filter_map(|e| match e { ZettaLog::Spawn { row: "dust.c100", t, .. } => Some(*t), _ => None }).collect();
            assert_eq!(c100.len(), laid.len() * z.zetta_dust_c100_waves.round() as usize, "{enemy}: the event's dust in waves");
            let behind_cut = seq_time(z.zetta_end_start + 7.1666667, &z, 5.0);
            assert!(c100.iter().all(|t| *t + 4.2 <= behind_cut), "{enemy}: gone by the cut behind him");
            let don = rig.party.member(rig.don).unwrap().position();
            for (t, at) in &laid {
                assert!((*t - end0).abs() < 0.1, "{enemy}: as the ending begins ({t})");
                assert!(at.z.abs() < 5.0, "{enemy}: on the ground ({at})");
                assert!((*at - don).truncate().length() > 150.0, "{enemy}: clear of Donald");
            }
            assert_eq!(stops.len(), near.len());
            assert!(stops.iter().all(|t| *t <= handoff - 3.5), "{enemy}: it stops rising in time to thin out by the hand-off");
        }
    }

    #[test]
    fn a_broken_spike_rock_falls_lands_bounces_and_comes_to_rest_on_its_ground() {
        // (round 5c) closed form: the same place for the same time however it is stepped; never under its ground
        let p0 = Vec3::new(0.0, 0.0, 600.0);
        let v0 = Vec3::new(160.0, 30.0, 20.0);
        let (_, _, l1, l2) = rock_flight(p0, v0, 980.0, 50.0, 0.28, 0.0);
        assert!(l1 > 0.9 && l1 < 1.2 && l2 > l1, "it falls, then bounces ({l1}, {l2})");
        let mut low = f32::MAX;
        for n in 0..400 {
            let (p, _, _, _) = rock_flight(p0, v0, 980.0, 50.0, 0.28, n as f32 * 0.01);
            low = low.min(p.z);
        }
        assert!(low >= 50.0 - 1e-2, "never under its ground ({low})");
        let (p, v, _, _) = rock_flight(p0, v0, 980.0, 50.0, 0.28, 4.0);
        assert!((p.z - 50.0).abs() < 1e-3 && v.length() < 1.0, "at rest on its ground ({p}, {v})");
        assert!(p.x > 150.0, "it fell out along its push ({p})");
    }

    #[test]
    fn the_killed_target_breaking_apart_is_heard_and_a_survivor_is_not() {
        // (round 5c) the user: "there should be a sound effect for the enemy breaking apart": its crack, its break, its ash, on
        // it, only when Zettaflare killed it
        for (hp, killed) in [(1.0e6f32, false), (500.0, true)] {
            let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
            rig.dummies.hp = hp;
            set_ending(Some(false));
            rig.cast();
            rig.run_for(16.5);
            for s in ["se00508_061", "se00002_116" /* (KITFX) was the looping se00002_105, refused */, "se_cs_enm_HLdead1"] {
                assert_eq!(rig.audio.played.iter().any(|p| p.ends_with(s)), killed, "hp {hp}: {s}");
            }
        }
        set_ending(None);
    }

    #[test]
    fn the_spikes_go_at_once_when_he_is_freed_early() {
        // (round 5c) a hit during the ending frees him: the spikes break up at once (their dust), none left standing
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.cast();
        let z = z();
        let dt = rig.dt;
        for _ in 0..((seq_time(z.zetta_end_start, &z, 5.0) + 1.0) / dt) as usize {
            rig.step();
            rig.party.don_zetta_view(dt, 0.0);
        }
        assert!(rig.party.don_zetta.runs[0].spikes.iter().any(|s| s.handle.is_some()), "standing in the wide shot");
        let before = rig.party.don_zetta.log.len();
        rig.party.member_mut(rig.don).unwrap().enter(NpcStateId::Damage);
        rig.step();
        assert!(rig.party.don_zetta.runs.iter().all(|r| r.spikes.iter().all(|s| s.handle.is_none())), "none left standing");
        let dusts = rig.party.don_zetta.log[before..].iter().filter(|e| matches!(e, ZettaLog::Spawn { row: "spikes.dust", .. })).count();
        assert!(dusts == 0 || dusts >= rig.party.don_zetta.runs[0].spikes.len(), "each broke up");
    }

    #[test]
    fn the_target_shot_shakes_while_the_enemy_is_in_the_beam() {
        // (round 5c) the user: "Add screenshake to Zettaflare during the shot where the enemy is inside the beam": only in the
        // target's shot, from the hit to the fade to white, smooth (motion only, no strobe), within its bounds; off with
        // zetta.shake 0
        let z = z();
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        rig.cast();
        let (hit, white) = (seq_time(z.zetta_hit_at, &z, 5.0), seq_time(z.zetta_white_in_at, &z, 5.0));
        let mut t = 0.0;
        let (mut shook, mut last) = (0, Vec3::ZERO);
        while t < white + 2.0 {
            rig.step();
            t += rig.dt;
            let Some(v) = rig.party.don_zetta_view(rig.dt, 0.0) else { continue };
            let on = v.shake_rot.length() > 1e-4;
            if on {
                shook += 1;
                assert!(t > hit - 0.05 && t < white + 0.05, "only with the enemy in the beam ({t})");
                assert!(v.shot == "shot920", "only in the target's shot ({})", v.shot);
                assert!(v.shake_rot.abs().max_element() <= 1.8 * 1.01 * z.zetta_shake_deg + 1e-3 && v.shake_loc.abs().max_element() <= 1.8 * 1.01 * z.zetta_shake_cm + 1e-3, "within its bounds");
                assert!((v.shake_rot - last).length() < 1.2 * z.zetta_shake_deg + 0.2, "no snap step to step ({})", (v.shake_rot - last).length());
            }
            last = v.shake_rot;
        }
        assert!(shook > 60, "it shook ({shook} steps)");
        let mut off = z.clone();
        off.zetta_shake = 0.0;
        assert_eq!(shake_at(z.zetta_hit_at + 1.0, &off), (Vec3::ZERO, Vec3::ZERO), "off with zetta.shake 0");
    }

    #[test]
    fn donald_is_covered_in_light_that_shrinks_with_the_beam() {
        // (round 5c) the user: "Donald, the source of the beam, should be covered in light that shrinks alongside the beam just
        // like in KH3": from the ending's start a blaze at his wand tip (the ending beam's start), its size the beam's width
        // share every step, shrinking to nothing; gone at the cut behind him
        let z = z();
        let mut rig = Rig::new(Vec3::new(900.0, 200.0, 0.0), 1.0 / 60.0);
        rig.cast();
        let end0 = seq_time(z.zetta_end_start, &z, 5.0);
        for _ in 0..((end0 + 7.5) / rig.dt) as usize {
            rig.step();
            rig.party.don_zetta_view(rig.dt, 0.0);
        }
        let (t0, at, _) = rig.spawn_of("end.donald.src_glow").expect("the light at the source");
        assert!((t0 - end0).abs() < 0.05, "from the ending's start ({t0})");
        let b = rig.party.don_zetta.runs[0].end_beam.expect("the ending beam");
        assert!((at - b.origin).length() < 1.0, "at his wand tip");
        let h = rig.effects.spawned.iter().find(|s| s.1 == "vd_zetta_src_glow").map(|s| s.0).unwrap();
        let sizes: Vec<f32> = rig.effects.transformed.iter().filter(|x| x.0 == h).map(|x| x.3.x).collect();
        assert!(sizes.first().copied().unwrap_or(0.0) > 0.95, "whole at first ({:?})", sizes.first());
        assert!(sizes.windows(2).all(|w| w[1] <= w[0] + 1e-4), "it only shrinks");
        assert!(sizes.last().copied().unwrap_or(1.0) < 0.05, "down to nothing ({:?})", sizes.last());
        assert!(rig.time_of(|l| matches!(l, ZettaLog::Stop { row: "end.donald.src_glow", .. })).is_some(), "gone at the cut behind him");
    }

    #[test]
    fn the_waiting_spots_are_out_of_the_shot_behind_donald() {
        // (round 5c) the allies wait out of the ending's last shot (970, from 15 m behind him along the beam): every spot,
        // near, far or to the side, out of every key of that shot (a body 1.8 m tall, its edges too)
        let z = z();
        for enemy in [Vec3::new(700.0, 0.0, 0.0), Vec3::new(2500.0, 200.0, 0.0), Vec3::new(-40.0, 900.0, 0.0)] {
            let mut rig = Rig::new(enemy, 1.0 / 60.0);
            rig.cast();
            for _ in 0..((seq_time(z.zetta_end_start, &z, 5.0) + 1.0) / rig.dt) as usize {
                rig.step();
                rig.party.don_zetta_view(rig.dt, 0.0);
            }
            let fr = rig.party.don_zetta.runs[0].end_frame.expect("the ending");
            let spots = rig.party.don_zetta_wait_spots();
            assert!(spots.len() >= 4, "{enemy}: spots");
            let shot = cam_shots().into_iter().find(|s| s.name == "shot970").unwrap();
            for k in &shot.keys {
                let (at, rot, fov) = (fr.point(k.1), fr.rotation(k.2), k.3);
                for s in &spots {
                    for p in [*s, *s + Vec3::Z * 180.0] {
                        let (h, v) = view_angles(at, rot, p);
                        assert!(h.abs() > fov * 0.5 + 4.0 || v.abs() > vfov_of(fov) * 0.5 + 4.0, "{enemy}: a waiting spot in the shot behind Donald ({s}, {h:.1} deg)");
                    }
                }
            }
        }
    }

    #[test]
    fn a_shots_size_override_applies_in_that_shot_only() {
        // (round 5c) the user: "you can make things change size with camera cuts, just make sure everything looks cool and
        // correct": the override rows (kind shot_scale) by shot; the light at the source and the shock at his feet bigger in
        // the wide shot (960), nothing else touched, nothing in other shots
        let z = z();
        assert!((shot_scale("end.donald.src_glow", Some("shot960")) - 1.4).abs() < 1e-4);
        assert!((shot_scale("end.world.96_zfr_01", Some("shot960")) - 1.25).abs() < 1e-4);
        assert_eq!(shot_scale("end.donald.src_glow", Some("shot970")), 1.0);
        assert_eq!(shot_scale("fx.beam.stream_93_a", Some("shot960")), 1.0);
        assert_eq!(shot_scale("end.donald.src_glow", None), 1.0);
        // the shot by time: the cast's, the target's (one shot), the wide, the one behind him
        assert_eq!(shot_at(z.zetta_cast_start + 0.1, &z), Some(CAST_SHOT));
        assert_eq!(shot_at(z.zetta_hit_at + 0.5, &z), Some("shot920"));
        assert_eq!(shot_at(z.zetta_white_in_at - 0.1, &z), Some("shot920"));
        assert_eq!(shot_at(z.zetta_end_start + 1.0, &z), Some("shot960"));
        assert_eq!(shot_at(z.zetta_end_start + 8.0, &z), Some("shot970"));
    }

    #[test]
    fn every_sound_fades_out_and_none_outlives_the_sequence() {
        // (round 5c) the user: "the zettaflare sound effects need to fade out": through a whole Zettaflare (and one handed
        // back early by the camera stick) every sound it played is stopped by the hand-back, each with a fade-out of at
        // least zetta.sound_fade (no hard cut, nothing carrying on into play)
        let z = z();
        for early in [false, true] {
            let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
            rig.cast();
            let handoff = seq_time(z.zetta_end_handoff_at, &z, 5.0);
            let end0 = seq_time(z.zetta_end_start, &z, 5.0);
            let mut t = 0.0;
            while t < handoff + 1.0 {
                rig.step();
                t += rig.dt;
                let stick = if early && t > end0 + 1.0 { 1.0 } else { 0.0 };
                rig.party.don_zetta_view(rig.dt, stick);
            }
            assert!(!rig.audio.handles.is_empty(), "it played sounds");
            for (h, name) in &rig.audio.handles {
                // (2026-10-23) his voice lines (the down's cry: a voice-set unit, "group:number") are his, not the sequence's
                if name.contains(':') {
                    continue;
                }
                let stop = rig.audio.stopped.iter().find(|s| s.0 == *h).unwrap_or_else(|| panic!("early {early}: {name} never stopped"));
                assert!(stop.1 >= z.zetta_sound_fade - 1e-4, "early {early}: {name} cut with a {} s fade", stop.1);
            }
        }
    }

    #[test]
    fn the_hit_target_plays_its_hurt_reaction_while_held() {
        // (round 5c) the user: "the enemy that is hit should play a hurt animation as a universal rule": the hold asks for its
        // own idle before the hit and its hurt reaction from the hit (hosts: their damage-reaction route), for a survivor and
        // a killed target alike
        for hp in [1.0e6f32, 500.0] {
            let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
            rig.dummies.hp = hp;
            rig.cast();
            let z = z();
            let hit = seq_time(z.zetta_hit_at, &z, 5.0);
            let (mut before, mut after) = (0, 0);
            let mut t = 0.0;
            while t < hit + 3.0 {
                let n = rig.dummies.holds.len();
                rig.step();
                t += rig.dt;
                for (_, h) in rig.dummies.holds[n..].iter() {
                    let Some(h) = h else { continue };
                    if t < hit - 0.05 {
                        assert!(!h.hurt, "hp {hp}: no hurt before the hit ({t})");
                        before += 1;
                    } else if t > hit + 0.05 {
                        assert!(h.hurt, "hp {hp}: its hurt reaction from the hit ({t})");
                        after += 1;
                    }
                }
            }
            // a survivor is held (hurt) until its throw; a killed one is the finale's from the hit (the hosts play its hurt
            // under the beam's glow until its silhouette)
            if hp > 1.0e5 {
                assert!(before > 60 && after > 60, "hp {hp}: held before ({before}) and after ({after}) the hit");
            } else {
                assert!(before > 60 && !rig.dummies.finales.is_empty(), "hp {hp}: the finale from the hit");
            }
        }
    }

    #[test]
    fn without_the_ending_nothing_of_it_plays() {
        let mut rig = Rig::new(Vec3::new(800.0, 0.0, 0.0), 1.0 / 60.0);
        set_ending(Some(false));
        rig.cast();
        let dt = rig.dt;
        let mut shots = Vec::new();
        let mut white = 0.0f32;
        for _ in 0..(18.0 / dt) as usize {
            rig.step();
            if let Some(v) = rig.party.don_zetta_view(dt, 0.0) {
                if !shots.contains(&v.shot) {
                    shots.push(v.shot);
                }
                white = white.max(v.white);
            }
        }
        assert!(!shots.contains(&"shot960") && !shots.contains(&"shot970") && white == 0.0);
        assert!(rig.party.don_zetta.log.iter().all(|e| !matches!(e, ZettaLog::Spawn { row, .. } if row.starts_with("end."))));
        assert!(!rig.party.member(rig.don).unwrap().don_zetta.ending);
    }
}
