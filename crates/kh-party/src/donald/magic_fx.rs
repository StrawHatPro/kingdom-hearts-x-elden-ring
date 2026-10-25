//! (DON-MAGIC) THE SPELLS IN THE WORLD: every actor Donald's casts put out, the one route their hits take, the damage
//! side of the Elemental Reactions, and the statuses' visuals (documentation/DONALD_OVERHAUL.md 4.3, 4.5, 6.1 - 6.5).
//!
//! WHO DOES WHAT
//! - `cast.rs` starts a cast on the pawn (clip, MP, `commit_cast`) and, at its shot, calls [`launch`]: the spell's
//!   actors per its row of `party-don-spells` and the spell-specific numbers of `party-don-magic` ([`magic`]).
//! - LINE 1 tiers 1-3 are SORA'S OWN ACTORS (kh-core `magic::caster::SpellCaster`, one per Donald: Firaga is Firaga), the
//!   -za tier Sora's Grand Magic "-gan" actor (`S_FIRAGAN` 112 ...), the Cure line Sora's Cure actors (KH3's heal rules;
//!   Curaza `S_CURAGAN` 125). Their hits pass [`L1Targets`]: the reaction rule on the target's statuses, the row's
//!   statuses (`applies`), the damage x the reaction / Exposed / Boost and x (row power / Sora's row power).
//! - Everything else (line 2, the ultimates, the hybrids, the Y Arcane, the Releases) is one of the actors here
//!   ([`ActorKind`]) whose hits go through [`deliver`]: `Party::don_hit` (the reactions, the statuses), the damage
//!   function with Donald's Magic, KH3's hit record (a launch = PARABOLA, a freeze = the FREEZE bad status that kh-react
//!   holds the body in, a hold = STUN), then the reactions' damage side: Conduct chains to every Wet enemy within
//!   `conduct_radius`, Wildfire spreads the Burn, Steam Burst's Blind, Exposed (x1.25 for everyone: [`PartyFx`]).
//! - Burn ticks (`party.don.status_events`) are dealt here ([`burn_ticks`]) as hits without a reaction.
//!
//! Every hit of this module carries [`DON_HIT_FLAG`] in `HitRecord::flags`: Exposed is already in it (the party-wide
//! wrapper [`PartyFx`] and the Elden Ring route multiply only the hits without it).
//!
//! Frame-rate independent: every timer counts the seconds of the step; positions move by speed x dt.

use super::element::{ElemSet, Element};
use super::react::{resolve, HitOutcome, HitTag, Reaction};
use super::spells::SpellRef;
use super::status::{Apply, FullFlags, Size, StatusBook, StatusEvent};
use super::tune::{data, DonData, DonSpellRow};
use crate::host::PartyHost;
use crate::ids::MemberId;
use crate::party::Party;
use glam::Vec3;
use kh_core::enums::{ETresDamageAttribute as Attr, ETresDamageKind as Kind};
use kh_core::host::targets::{AttackShape, FinishKind, Friend, FriendLinkRequest, PushBody, TargetBody, TargetDefense};
use kh_core::host::{Attach, EffectHandle, EffectId, EffectSpawn, HitAttacker, HitInfo, HitRecord, HitResult, HurtShape, PartyMember, ReactionResult, TargetFlags, TargetId, TargetInfo, Targets, Tint};
use kh_core::status::{compute_damage, DamageAttack, DamageContext, Defender};
use std::sync::{Arc, RwLock};

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_don_magic.rs"));

/// The spell-specific numbers (sheet `party-don-magic`).
pub type MagicTune = DonMagicParams;

macro_rules! magic_fields {
    ($($f:ident),* $(,)?) => {
        /// One number by its row id (`flare.shrink`) or field name; false when it names nothing.
        pub fn set_magic(t: &mut MagicTune, key: &str, v: f32) -> bool {
            let k = key.trim();
            let k = k.strip_prefix("don.magic.").unwrap_or(k).replace(['.', '-'], "_").to_ascii_lowercase();
            match k.as_str() {
                $(stringify!($f) => { t.$f = v; true })*
                _ => false,
            }
        }
    };
}

magic_fields!(
    cast_recover, cast_guard_chance, cast_shot_fire, cast_shot_air_fire, cast_shot_cast, cast_shot_slam, cast_rate_min, cast_rate_max, cast_turn_max,
    cast_voice_big, boost_element, boost_cure, boost_arcane, size_medium_radius, size_large_radius, size_large_half_height, bolt_speed, bolt_turn,
    bolt_life, bolt_hit_radius, hit_default_radius, l1_za_scale_floor, flare_speed, flare_shrink, flare_burn_bonus, flare_after_delay, flare_stagger,
    freeze_shard_radius, freeze_shard_chill, freeze_shard_power, orbs_orbit_radius, orbs_orbit_speed, orbs_height, orbs_ball_scale, orbs_crackle_gap, orbs_recharge,
    orbs_chain_range, orbs_hurl_power, orbs_knockback, flood_wave_speed, flood_wall_mult, flood_wall_stun, flood_wall_probe, flood_puddle_radius,
    flood_puddle_tick, flood_interrupt_size, barrier_contact_interval, banish_ray_gap, inferno_tick, dust_delay, judgment_gap, tsunami_speed,
    aerial_width, holy_delay, thermal_def_ignore, plasma_speed, plasma_arc_range, scald_tick, whirl_speed, whirl_pull, whirl_tick, sunburst_tick,
    sunburst_height, hail_tick, hail_chain_range, glacier_width, glacier_height, whiteout_tick, storm_tick, tempest_speed, tempest_tick,
    spark_jump_range, spark_gap, maelstrom_tick, rain_tick, gale_launch_reveal, comet_height, comet_fall, comet_gap, gravity_pull, gravity_min_damage, gravity_share_base_1, gravity_share_base_2,
    gravity_share_base_3, gravity_share_per_magic, gravity_share_cap, gravity_boss_resist, gravity_flat_height, gravity_flat_width,
    gravity_restore_delay, gravity_restore_time, gravity_fx_appear, gravity_press_at, gravity_press_time, gravity_press_width,
    gravity_fx_floor_thin, gravity_fx_thin, gravity_fx_fade, gravity_fx_centre, gravity_fx_centre_step, gravity_fx_diameter_1, gravity_fx_diameter_2,
    gravity_fx_diameter_3, gravity_fx_ring,
    meteor_height, meteor_fall, meteor_gap, meteor_blast, ultima_delay, zetta_delay, rel_sanctuary_heal, rel_cyclone_pull, steam_blind_on_hit,
    // (DON-SPELLB)
    barrier_fx_width, barrier_fx_height, barrier_fx_alpha, barrier_fx_spin, barrier_fx_fade, barrier_fx_lv3_density, barrier_fx_scale_donald, barrier_fx_scale_goofy, release_gesture, gravity_fx_form, gravity_fx_total, gravity_rocks_count, gravity_rocks_size, gravity_rocks_height,
    conduct_power, wildfire_power, burn_min_damage, blind_swing_reach,
);

static MAGIC: RwLock<Option<Arc<MagicTune>>> = RwLock::new(None);

/// The numbers every spell reads (the compiled sheet until a host [`install_magic`]s its own).
pub fn magic() -> Arc<MagicTune> {
    if let Some(m) = MAGIC.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return m.clone();
    }
    let mut w = MAGIC.write().unwrap_or_else(|e| e.into_inner());
    w.get_or_insert_with(|| Arc::new(DON_MAGIC_PARAMS)).clone()
}

/// The host's numbers (the sheet read again, `don.magic.<row> = value` ctl keys).
pub fn install_magic(t: MagicTune) {
    *MAGIC.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(t));
}

/// Every row id of the sheet (a host's key listing).
pub fn magic_keys() -> impl Iterator<Item = &'static str> {
    DON_MAGIC_PARAM_ROWS.iter().map(|r| r.1)
}

/// `HitRecord::flags` bit of every hit this module (and `cast.rs`) delivers: the reaction rule ran, Exposed is in it.
pub const DON_HIT_FLAG: u32 = 0x4000_0000;

// ================================================================================================ the effects

/// The KH3 effect names of every spell (build/sora_tmp/party/DON_PLAN.md 4.2; [R] = recoloured / recombined, [V] = read
/// from the name and checked by the harness `cargo test -p kh-party donald::magic_tests::effects_are_packed`).
pub mod fx {
    pub const HIT_FIRE: &str = "vs_fir_hit0";
    pub const HIT_BLIZZARD: &str = "vm_blz_hit0";
    pub const HIT_THUNDER: &str = "vs_thd_hit0";
    pub const HIT_WATER: &str = "vs_wat_hit0";
    pub const HIT_AERO: &str = "vm_air_hit0";
    /// [R] gold (tint_of): the Fairy Godmother's holy hit draws with a substitute material in the port (no shader map)
    pub const HIT_LIGHT: &str = "vs_thd_hit0";
    pub const HIT_ARCANE: &str = "vn_ex001_fff_hit00";
    // line 2
    /// (AUD) the small white-hot star (AUD tools/make_l2.js; Xehanort's flare shot drew a 3 m pink ring round it in flight and
    /// no star)
    pub const FLARE_STAR: &str = "vd_don_aud_flare_star";
    /// (AUD) the detonation's blast: KH3's Flare explosion (Xehanort's), over Sora's fire burst (FLARE_BOOM alone was a puff)
    pub const FLARE_BLAST: &str = "ve_ex771_flr4_exp_040";
    /// (KITFX) the user 2026-10-23: "Flare currently has no sound effects, fix that". Its detonation's SE (FLARE_BOOM's) was a
    /// looping asset refused as a one-shot (see `extra_se`); its star (AUD's) carries none. KH3 Fire-family sounds:
    /// the launch = Firaga's projectile-spawn SE (TresProj_MagFigaNml), the flight = Firaga's bolt loop, the SeAttach of
    /// Xehanort's flare shot (ve_ex773_flre_sht_fir0) itself: played looping where the star leaves the wand and faded out at
    /// the detonation (`kitfx_hums`); the detonation = FLARE_BOOM's Sora's biggest Fire burst (vm_fir_han_070).
    pub const FLARE_LAUNCH_SE: &str = "se00002_017";
    pub const FLARE_HUM_SE: &str = "se00002_005";
    pub const FLARE_HUM_VOLUME: f32 = 0.8;
    pub const FLARE_HUM_FADE: f32 = 0.2;
    // THE TEST-LEVEL RUN (DON-MAGIC runs/r1, 2026-10-22): many [V] picks of the plan from KH3's DLC / event folders draw
    // with a SUBSTITUTE material in the port ("no shader map in its parent chain": white slabs, nothing at all). Those are
    // replaced here by KH3 effects whose materials the port draws, recoloured [R] where the plan names a colour; the
    // working [V] picks stay (Flare's star, Tsunami, Whiteout, Tempest, the Ultima / Zettaflare calls, the pole orbs).
    /// Sora's Fire landing on the ground (map hit) in its own colours (Xehanort's flare detonations draw with substitutes;
    /// Sora's vm_fir_zan1 is his fire's lingering put effect and draws as a tall slab when spawned alone).
    pub const FLARE_BOOM: &str = "vm_fir_wht0";
    pub const FLARE_HIT: &str = "vs_fir_hit0";
    pub const FLARE_AFTER: &str = "vm_fir_fir0";
    /// [R] Sora's Blizzard bolt burst, frost-white, as the 1 s warning (the event frost draws with a substitute; KH3's
    /// freeze-in status effect has no material: light and sound only).
    pub const FREEZE_FROST: &str = "vm_blz_blt1";
    pub const FREEZE_PRISON: &str = "vs_abn_frz4";
    /// [R] Sora's Blizzard landing on the ground as the Lv 3 shards (KH3's freeze shatter has no material).
    pub const FREEZE_SHARDS: &str = "vm_blz_wht0";
    /// BALL LIGHTNING (the user's change of 2026-10-22: real balls of lightning orbiting the wearer): a ball's glow core is
    /// KH3's thunder glow (the electric enemy e_ex028's `ve_ex028_thd_glw00`, looping), [R] electric white-blue, moved every
    /// step; it crackles with KH3's thunder sparks in turn (Sora's Thunder hit, the Thunder-cloud status bolt); a touch
    /// pops it with the Thunder hit; Lv 3's arc to a second enemy is the status bolt.
    /// (AUD) a crackling ball of lightning (AUD tools/make_orb.js: core, flickering halo, arcs crawling over it, sparks; KH3's
    /// lone glow sprite read as a 20 cm white dot), and its crackle with the point light at 20 % (the ground flashed white)
    pub const ORB_CORE: &str = "vd_don_aud_orb";
    pub const ORB_CRACKLE: [&str; 2] = ["vs_thd_hit0", "vd_don_aud_orb_zap"];
    pub const ORB_POP: &str = "vs_thd_hit0";
    pub const ORB_ARC: &str = "vs_abn_cld_thd0";
    /// The Conduct chain's bolt on each Wet enemy around (4.5): Sora's Thunder strike.
    /// (AUD) the strike without its strobe (AUD tools/make_soft.js: no screen veil / scene darkening / radial blur): the chain
    /// strikes every Wet enemy at once and again on every Thunder hit, so KH3's full flash strobed (the user on Judgment Bolt:
    /// "less seizure inducing")
    pub const CONDUCT_ARC: &str = "vd_don_aud_str0s";
    pub const FLOOD_WAVE: &str = "vl_wat_psh0";
    pub const FLOOD_PUDDLE: &str = "vl_bt07_wat0";
    /// (DON-SPELLB) the barrier's KH1 wind ribbons (authored: DON-SPELLB tools/make_aero.js), one per level, endless on the
    /// wearer until the barrier ends; and the gust when it is put on (Sora's Aero call).
    pub const BARRIER_LOOK: [&str; 3] = ["vd_don_aero1", "vd_don_aero2", "vd_don_aero3"];
    pub const BARRIER_GUST: &str = "vm_air_cal0";
    pub const BARRIER_CAST: &str = "vn_ex001_air_han_00";
    pub const BARRIER_CONTACT: &str = "vm_air_hit0";
    /// [R] a gold ray from the sky: Sora's Thundara strike recoloured (the holy beam of the final battle and the plan's
    /// fallback vm_bem_lne0 both draw their beam with a substitute material: a white slab).
    /// (AUD) the ray from the sky in gold: the Thundara strike without its strobe (make_soft.js; KH3's darkened the whole scene
    /// for every ray of Banish II / III)
    pub const BANISH_RAY: &str = "vd_don_aud_str1s";
    /// (AUD) Banish II / III's later rays (0.1-0.2 s apart): the strike without its strobe; the first ray keeps KH3's flash
    pub const BANISH_RAY_SOFT: &str = "vd_don_aud_str1s";
    // ultimates
    /// [R] Sora's Fire ground bursts and Firaga-gan bolt over the field (the Volcano Lord's fire field draws nothing).
    /// (AUD) INFERNO[0] is the field's body for its whole life: the burning ground (AUD tools/make_ult.js; was Sora's one-shot
    /// ground burst, so the field burned with nothing to see); vm_fir_blt3 (Firaga's endless bolt body) dropped
    pub const INFERNO: [&str; 2] = ["vd_don_aud_inferno", "vm_fir_fir0"];
    /// [R] Sora's Blizzard ground burst, frost-white, and the Blizzard hit's shatter.
    pub const DIAMOND_DUST: [&str; 2] = ["vm_blz_wht0", "vm_blz_hit0"];
    pub const JUDGMENT_BOLT: &str = "vm_thd_str3";
    /// (DON-SPELLB) the user, 2026-10-22: "Judgement Bolt should be less seizure inducing ... don't change anything else except
    /// how violently it flashes": its strikes are KH3's Thundaga strike with the scene flash softened (DON-SPELLB make_jb.js):
    /// every 3rd strike a mild scene dim (<= 3 large flashes a second), the others none, the point light and hit glow lower.
    pub const JUDGMENT_SOFT: [&str; 2] = ["vd_don_jb_str3a", "vd_don_jb_str3b"];
    pub const TSUNAMI: [&str; 2] = ["vl_wat_psh0", "vm_wat_cal0"];
    /// [R] Sora's Aeroza and the wind strike (the Lich's tornado draws with a substitute).
    pub const AERIAL_BLAST: [&str; 2] = ["vm_air_zan0", "vm_wnd_str00"];
    /// The Fairy Godmother's holy call, then [R] gold: Sora's Thundaga strike as the beam, the Thunder burst, the hit.
    /// (AUD) HOLY[1] the beam: Judgment Bolt's no-dim strike in gold (one; KH3's strike strobed); HOLY[2] the burst: the
    /// strobe-free Thundaga burst
    pub const HOLY: [&str; 4] = ["vn_ex003_hly_cal0", "vd_don_jb_str3b", "vd_don_aud_str2s", "vs_thd_hit0"];
    // hybrids
    pub const THERMAL: [&str; 2] = ["vs_fir_hit0", "vm_blz_hit0"];
    pub const PLASMA: &str = "vm_fir_blt3";
    pub const SCALD: [&str; 2] = ["vp_wfi_blt0", "vs_abn_cld0"];
    pub const FIRE_WHIRL: &str = "vn_ex002_trn_loc00";
    /// Sora's Fire burst in its own colours, with Simba's light (vm_fns_sky0 draws with a substitute).
    pub const SUNBURST: [&str; 2] = ["vm_fir_wht0", "vm_lgt_2"];
    /// (DON-SPELLB, 2026-10-23) the strike: KH3's vm_thd_str0 without its white screen veil and scene change (it strobed)
    pub const HAIL: [&str; 2] = ["vm_blz_blt1", "vd_don_thd_str0s"];
    /// [R] Sora's Blizzard ground bursts along the wall's line (the event ice wall draws with substitutes).
    pub const GLACIER: [&str; 2] = ["vm_blz_wht0", "vm_blz_blt2"];
    /// (DON-SPELLB, 2026-10-23) a blizzard round him he stays visible in (KH3's Arendelle whiteout turned the whole screen
    /// white for the field's 8 s)
    pub const WHITEOUT: &str = "vd_don_whiteout";
    pub const PRISM: &str = "vm_blz_blt2";
    /// (HYB) the Thunder strike without its veil and scene change (vm_thd_str1 strobed the screen black on every strike)
    pub const THUNDERSTORM: [&str; 2] = ["vp_wth_blt0", "vd_don_thd_str0s"]; // (HYB) the strike without the veil (five Wet enemies strike at once)
    /// [R] yellow: Donald's own tornado (the Lich's spark tornado draws its lines with a substitute).
    pub const TEMPEST: &str = "vn_ex002_trn_awa00";
    pub const DIVINE_SPARK: &str = "vd_don_thd_str0s"; // (DON-SPELLB) the strike without its strobe (see HAIL)
    pub const MAELSTROM: &str = "vp_wai_han0";
    pub const SHINING_RAIN: [&str; 2] = ["vm_cur_win0", "vs_wat_hit1"];
    /// [R] gold wind (vm_wnd_kze00 draws with a substitute).
    pub const RADIANT_GALE: &str = "vm_wnd_str00";
    // Y
    /// [R] Donald's own Call Meteor, star-white (Stitch's star draws with substitutes).
    pub const COMET: [&str; 2] = ["vn_ex001_fcm_flt00", "vn_ex001_fcm_hit00"];
    /// GRAVITY (the user, 2026-10-22: "Gravity needs to look like this (you will have to create the effect yourself)", the
    /// KH1 reference ref/gravity ref.png): Donald's own effects authored from KH3 parts by DON-MAGIC make_gravity.js, one per
    /// tier (bigger each): a near-black indigo sphere with swirling blue-violet wisps, a violet-magenta lightning band with a
    /// white-hot core across its middle, a pink-red ring on the ground; it swells in, pulses for the hold and implodes.
    /// Spawned on the ground under the aim at the cast; the crush's hit spark is [R] violet.
    pub const GRAVITY: [&str; 4] = ["vd_don_grv1", "vd_don_grv2", "vd_don_grv3", "vs_thd_hit0"];
    pub const METEOR: [&str; 2] = ["vn_ex001_fcm_flt00", "vn_ex001_fcm_hit00"];
    /// The Keyblade Graveyard call, then [R] a storm of pure magic: Sora's Thundaga strike in cyan-white (the c36 dome
    /// draws with substitutes).
    pub const ULTIMA: [&str; 2] = ["vd_kg878_c34_ult_cal00", "vm_thd_str3"];
    /// The call, then Sora's Fire ground burst (the c38 call has no material: light and sound only).
    pub const ZETTAFLARE: [&str; 2] = ["vd_kg878_c34_ult_cal00", "vm_fir_wht0"];
    // Releases
    /// Sora's Fire ground burst (the Volcano Lord's pillar draws nothing).
    pub const FIRESTORM: &str = "vm_fir_wht0";
    /// KH3's freeze shatter (light and sound) with [R] Sora's Blizzard hit and ground burst as the visible ice.
    pub const SHATTER: [&str; 3] = ["vm_blz_hit0", "vs_abn_frz_out1", "vm_blz_wht0"];
    pub const OVERLOAD: &str = "vm_thd_str2";
    /// (AUD) Overload's strike on each enemy (all at once, on top of the burst's own flash): strobe-free
    pub const OVERLOAD_HIT: &str = "vd_don_aud_str2s";
    pub const DELUGE: &str = "vl_wat_psh0";
    pub const CYCLONE: &str = "vn_ex002_trn_awa00";
    pub const SANCTUARY: [&str; 2] = ["vm_cur_win0", "vs_com_grd01"];
    // statuses (on the target while they last)
    // (DON-SPELLB) the statuses' looping looks (DON-SPELLB tools/make_status.js; KH3's own were one-shots, drew nothing, or
    // were Slow's clock rings: the "dark ring")
    // (DON-SPELLB) STATUS LOOKS ROUND 2 (the user, 2026-10-23): each status its own look on the enemy's body, built round a
    // capsule from KH3's status / element emitters (KH3's own vs_abn_* spawn on the victim's skeletal mesh, and its freeze
    // in / out draw an ice copy of SORA's mesh wherever they are spawned: not usable on enemies)
    pub const WET: &str = "vd_don_st_wet";
    /// A Wet enemy moving or hit: a splash off it.
    pub const WET_SPLASH: &str = "vd_don_st_wet_splash";
    /// Chill by its stacks (1-4): more frost with each; crystals at 4.
    pub const CHILL_STACKS: [&str; 4] = ["vd_don_st_chill1", "vd_don_st_chill2", "vd_don_st_chill3", "vd_don_st_chill4"];
    pub const CHILL: &str = CHILL_STACKS[0];
    pub const FROZEN: &str = "vd_don_st_frozen";
    /// The moment it freezes: frost drawn in, a cold flash (the Frozen look's own sound plays with it).
    pub const FROZEN_IN: &str = "vd_don_st_freeze_in";
    /// The ice breaking: the Frozen's end and the Shatter reaction.
    pub const FROZEN_OUT: &str = "vd_don_st_shatter";
    pub const BURN: &str = "vd_don_st_burn";
    pub const REVEAL: &str = "vd_don_st_reveal";
    /// The Reveal turned into Exposed: the aura stronger, a pulsing ring at the feet.
    pub const EXPOSED: &str = "vd_don_st_exposed";
    pub const BLIND: &str = "vd_don_st_blind";
    /// A large enemy's slow (its freeze turned into a slow): an ice swirl under it.
    pub const SLOW: &str = "vd_don_st_slow";

    /// Every effect name the spells spawn (the pack harness checks each one).
    pub fn all() -> Vec<&'static str> {
        let mut v = vec![
            HIT_FIRE, HIT_BLIZZARD, HIT_THUNDER, HIT_WATER, HIT_AERO, HIT_LIGHT, HIT_ARCANE, FLARE_STAR, FLARE_BLAST, FLARE_BOOM, FLARE_HIT, FLARE_AFTER, FREEZE_FROST, FREEZE_PRISON,
            FREEZE_SHARDS, ORB_CORE, ORB_POP, ORB_ARC, CONDUCT_ARC, FLOOD_WAVE, FLOOD_PUDDLE, BARRIER_GUST, BARRIER_CAST, BARRIER_CONTACT, BANISH_RAY, BANISH_RAY_SOFT, OVERLOAD_HIT, JUDGMENT_BOLT, PLASMA, FIRE_WHIRL, WHITEOUT, PRISM,
            TEMPEST, DIVINE_SPARK, MAELSTROM, RADIANT_GALE, FIRESTORM, OVERLOAD, DELUGE, CYCLONE, WET, WET_SPLASH, FROZEN, FROZEN_IN, FROZEN_OUT, BURN, REVEAL, EXPOSED, BLIND, SLOW,
        ];
        v.extend_from_slice(&CHILL_STACKS);
        for a in [&JUDGMENT_SOFT[..], &BARRIER_LOOK[..], &ORB_CRACKLE[..], &INFERNO[..], &DIAMOND_DUST[..], &TSUNAMI[..], &AERIAL_BLAST[..], &HOLY[..], &THERMAL[..], &SCALD[..], &SUNBURST[..], &HAIL[..], &GLACIER[..], &THUNDERSTORM[..], &SHINING_RAIN[..], &COMET[..], &GRAVITY[..], &METEOR[..], &ULTIMA[..], &ZETTAFLARE[..], &SHATTER[..], &SANCTUARY[..]] {
            v.extend_from_slice(a);
        }
        for e in super::Element::ALL {
            v.push(super::cast_effect_name(e));
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    /// THE SOUND OF AN EFFECT THAT OTHER CHARACTERS ALSO SPAWN (Sora's thunder strikes, Goofy's tornado, the statuses):
    /// played through the audio seam by name ("effect:<SeAttach>", kh-audio's route) next to the effect, so that their own
    /// sound is never changed by an alias. The effects only Donald's spells spawn carry theirs through the party audio
    /// pack (tools/sora_audio_pack.js --party-extra, the (DON-MAGIC) list).
    pub fn extra_se(name: &str) -> Option<&'static str> {
        Some(match name {
            "vm_thd_str0" | "vd_don_thd_str0s" => "effect:vm_thd_str00",
            // (DON-SPELLB) Whiteout's blizzard: Aeroga's wind
            "vd_don_whiteout" => "effect:vm_air_han_080",
            "vm_thd_str1" => "effect:vm_thd_str01",
            // (AUD) the strobe-free copies sound as KH3's strikes
            "vd_don_aud_str0s" => "effect:vm_thd_str00",
            "vd_don_aud_str1s" => "effect:vm_thd_str01",
            "vd_don_aud_str2s" => "effect:vm_thd_str02",
            "vd_don_aud_orb_zap" => "effect:vs_abn_cld_thd0",
            // (AUD) the ultimates' storm parts (make_ult.js): the KH3 effect's own attached sound
            "vd_don_aud_fir_erupt" => "effect:vm_fir_han_050",
            "vd_don_aud_blz_dome" => "effect:vm_blz_han080",
            "vd_don_aud_wat_dome" => "effect:vm_wat_han080",
            "vd_don_aud_ddust" => "effect:vm_air_han_080",
            "vm_thd_str2" => "effect:vm_thd_str02",
            "vm_thd_str4" => "effect:vm_thd_str02", // (HYB) silent alone in KH3: Thundaga's strike sound
            "vm_thd_str3" | "vd_don_jb_str3a" | "vd_don_jb_str3b" => "effect:vm_thd_str03",
            "vm_pole_thd0" | "vm_pole_thd1" | "vm_pole_thd2" | "vm_pole_thd3" => "effect:vn_pole_thd0",
            "vl_wat_psh0" => "effect:vm_wat_bar1",
            "vn_ex002_trn_loc00" | "vn_ex002_trn_awa00" => "effect:vm_air_han_140",
            "vm_cur_win0" => "effect:vm_cur_cal0",
            "vs_abn_frz0" | "vs_abn_frz_in" | "vd_don_st_frozen" => "effect:vs_abn_frz1",
            // (DON-SPELLB status looks) the ice breaking: KH3's freeze-out sound; the burn's own loop sound
            "vd_don_st_shatter" => "effect:vs_abn_frz3",
            // (KITFX) Burn's look named KH3's burn status LOOP (se00003_003), refused as a one-shot: the ignition is KH3's Fire
            // smallest burst SE (vm_fir_han_010: se00002_006, one-shot) instead
            "vd_don_st_burn" => "effect:vm_fir_han_010",
            // (YFX) Donald's own Gravity has its own layered sounds on the ball's clock (yfx.rs gravity), no single SE here
            // (the stand-ins of the test-level run of 2026-10-22, silent alone in KH3, whose spell carries the sound: the sound of
            // the KH3 effect they stand in for, packed by sora_audio_pack.js --party-extra)
            // (KITFX) the three stand-ins below named KH3 effects whose SeAttach is a LOOP (ve_ex773_flre_bom / bom2: se00002_014,
            // Xehanort's flare bomb's burning loop; vd_fz452_ice_spa_1st: se00002_103, Blizzara's flight loop): a one-shot of a
            // looping asset is refused ("a looping asset from an instant notify is refused"), so Flare's detonation, the after-flare,
            // the Fire Release, Inferno, Sunburst, Freeze's shards, Diamond Dust, Glacier and Shatter were silent. Now one-shot
            // KH3 bursts of the same families: Sora's biggest Fire burst, a Fira burst, a Blizzaga burst.
            "vm_fir_wht0" => "effect:vm_fir_han_070",
            "vm_fir_fir0" => "effect:vm_fir_han_040",
            "vm_blz_wht0" => "effect:vm_blz_han040",
            "vs_abn_frz4" => "effect:vs_abn_frz2",
            "vs_abn_frz_out0" | "vs_abn_frz_out1" | "vs_abn_frz_out2" => "effect:vs_abn_frz3",
            _ => return None,
        })
    }

    /// KH3's own effects that are silent in KH3 too (no SeAttach of their own) and stay so: the hit sparks (the spell's
    /// burst carries the sound), the status loops, the barrier's swirl (its cast effect sounds), the puddle, the target
    /// mark, the guard flash of Sanctuary (its heal light sounds).
    pub fn silent_by_design(name: &str) -> bool {
        matches!(name, "vd_don_st_wet" | "vd_don_st_chill" | "vd_don_st_reveal" |
            // (DON-SPELLB status looks) the status loops and their small one-shots: the spell's hit carries the sound
            "vd_don_st_wet_splash" | "vd_don_st_chill1" | "vd_don_st_chill2" | "vd_don_st_chill3" | "vd_don_st_chill4" | "vd_don_st_freeze_in" | "vd_don_st_exposed" | "vd_don_st_blind" | "vd_don_st_slow" | "vs_fir_hit0" | "vm_blz_hit0" | "vs_thd_hit0" | "vs_wat_hit0" | "vs_wat_hit1" | "vs_abn_slw00" | "vp_ath_tgt0" | "vm_air_win0" | "vl_bt07_wat0" | "vs_com_grd01"
            // Ball Lightning's glow core: no SeAttach in KH3; its crackles (the Thunder-cloud bolt) carry the sound
            | "ve_ex028_thd_glw00" | "vd_don_aud_orb" /* (AUD) */
            // (DON-SPELLB) Aero Barrier's ribbons: worn for up to 40 s, the cast's gust (Sora's Aero call) carries the sound
            | "vd_don_aero1" | "vd_don_aero2" | "vd_don_aero3"
            // (AUD) Inferno's burning ground (the field's 8 s): the finisher and the eruptions carry the sound
            | "vd_don_aud_inferno"
            // (AUD) Flare's star in flight: the cast carries the sound; Flare's blast: its fire burst (FLARE_BOOM, played as KH3's
            // flare bomb) carries it
            | "vd_don_aud_flare_star" | "ve_ex771_flr4_exp_040")
    }

    /// THE [R] RECOLOURS (DON_PLAN 4.2; kh_core::Tint, kh-vfx `World::set_tint`): a spell's effect that stands in for
    /// one KH3 has not, in the colour the plan names.
    pub const GOLD: super::Tint = super::Tint::rgb(1.0, 0.8, 0.35);
    pub const VIOLET: super::Tint = super::Tint::rgb(0.72, 0.38, 1.0);
    pub const ORANGE: super::Tint = super::Tint::rgb(1.0, 0.55, 0.18);
    pub const GREEN: super::Tint = super::Tint::rgb(0.45, 1.0, 0.5);
    pub const ICE_BLUE: super::Tint = super::Tint::rgb(0.55, 0.85, 1.0);
    pub const YELLOW: super::Tint = super::Tint::rgb(1.0, 0.92, 0.35);
    pub const FROST: super::Tint = super::Tint::rgb(0.8, 0.95, 1.0);
    pub const STAR: super::Tint = super::Tint::rgb(0.75, 0.85, 1.0);
    pub const PURE: super::Tint = super::Tint::rgb(0.6, 0.95, 1.0);
    /// Ball Lightning's glow core: electric white-blue, brighter than the warm glow it recolours.
    /// (DON-SPELLB) the barrier's wind: white to pale blue (KH1's Aero).
    pub const WIND_WHITE: super::Tint = super::Tint { color: [0.76, 0.89, 1.0], intensity: 1.0 };
    pub const ELECTRIC: super::Tint = super::Tint { color: [0.5, 0.72, 1.0], intensity: 1.4 };
    /// Prism Frost's rainbow: one shard spray per colour.
    pub const RAINBOW: [super::Tint; 3] = [super::Tint::rgb(1.0, 0.45, 0.55), super::Tint::rgb(0.5, 1.0, 0.6), super::Tint::rgb(0.55, 0.6, 1.0)];

    /// The recolour of an effect of a spell (None: its own KH3 colours).
    pub fn tint_of(spell: Option<super::SpellRef>, name: &str) -> Option<super::Tint> {
        use super::{Element as E, SpellRef as S};
        Some(match (spell, name) {
            (Some(S::Hybrid(E::Fire, E::Thunder)), PLASMA) => VIOLET,
            (Some(s), "vs_thd_hit0" | "vm_thd_str1" | "vm_thd_str2" | "vm_thd_str3" | "vd_don_aud_str1s" | "vd_don_aud_str2s" | "vd_don_jb_str3a" | "vd_don_jb_str3b" /* (AUD) */) if s.elements().has(E::Light) && !s.elements().has(E::Thunder) && !s.elements().has(E::Fire) => GOLD,
            (Some(S::Line2(E::Blizzard, _)), FREEZE_FROST | FREEZE_SHARDS) => FROST,
            (Some(S::Hybrid(E::Thunder, E::Aero)), TEMPEST) => YELLOW,
            (Some(S::Ultimate(E::Blizzard)), "vm_blz_wht0") => FROST,
            (Some(S::Arcane(super::super::spells::ArcaneLine::Comet, _)), _) if COMET.contains(&name) => STAR,
            (Some(S::Arcane(super::super::spells::ArcaneLine::Gravity, _)), "vs_thd_hit0") => VIOLET,
            (Some(S::Arcane(super::super::spells::ArcaneLine::Ultima, _)), "vm_thd_str3") => PURE,
            (Some(S::Line2(E::Thunder, _)), ORB_CORE) => ELECTRIC,
            (Some(S::Hybrid(E::Fire, E::Aero)), FIRE_WHIRL) => ORANGE,
            (Some(S::Hybrid(E::Fire, E::Light)), "vm_lgt_2") => GOLD,
            (Some(S::Hybrid(E::Thunder, E::Light)), DIVINE_SPARK) => GOLD,
            (Some(S::Hybrid(E::Aero, E::Light)), RADIANT_GALE) => GOLD,
            (Some(S::Release(E::Aero)), CYCLONE) => GREEN,
            (_, "vp_ath_tgt0") => GOLD,
            _ => return None,
        })
    }

    /// DON-KIT's [R] recolours by the move's tag (pad/donald_moves.rs, donald_shots.rs, marked (DON-MAGIC tint)): the Ice
    /// Glide's slide (ice blue), Blink's warp (yellow), Ward Step's barrier (gold), the Aero slam's tornado (green).
    pub fn kit_tint(tag: &str) -> Option<super::Tint> {
        Some(match tag {
            "glide" | "DON_ICE_GLIDE" => ICE_BLUE,
            "blink" => YELLOW,
            "DON_WARD" => GOLD,
            "DON_SLAM_AERO" => GREEN,
            _ => return None,
        })
    }

    /// The hit effect of an element (none: the plain arcane one).
    pub fn hit_of(e: Option<super::Element>) -> &'static str {
        use super::Element as E;
        match e {
            Some(E::Fire) => HIT_FIRE,
            Some(E::Blizzard) => HIT_BLIZZARD,
            Some(E::Thunder) => HIT_THUNDER,
            Some(E::Water) => HIT_WATER,
            Some(E::Aero) => HIT_AERO,
            Some(E::Light) => HIT_LIGHT,
            None => HIT_ARCANE,
        }
    }
}

/// Donald's own cast effect of an element (his `n_ex001_Magic_ead` rows; Water: the Keyblade's `vm_wat_cal0`, as X2).
pub fn cast_effect_name(e: Element) -> &'static str {
    match e {
        Element::Fire => "vn_ex001_fir_cal00",
        Element::Blizzard => "vn_ex001_blz_cal00",
        Element::Thunder => "vn_ex001_thd_cal00",
        Element::Water => "vm_wat_cal0",
        Element::Aero => "vn_ex001_air_cal00",
        Element::Light => "vn_ex001_cur_cal00",
    }
}

// ================================================================================================ sizes, attributes

/// How big an enemy is: the host's word (`Targets::size_class`), else its bounds (sheet `size.*`).
pub fn size_of(targets: &dyn Targets, t: &TargetInfo) -> Size {
    match targets.size_class(t.id) {
        1 => Size::Small,
        2 => Size::Medium,
        3 => Size::Large,
        4 => Size::Boss,
        _ => {
            let m = magic();
            if t.radius >= m.size_large_radius || t.half_height >= m.size_large_half_height {
                Size::Large
            } else if t.radius >= m.size_medium_radius {
                Size::Medium
            } else {
                Size::Small
            }
        }
    }
}

/// KH3's damage attribute of an element (Light: holy = KH3's NOTYPE; the Y magic too).
pub fn attr_of(e: Option<Element>) -> Attr {
    match e {
        Some(Element::Fire) => Attr::Fire,
        Some(Element::Blizzard) => Attr::Blizzard,
        Some(Element::Thunder) => Attr::Thunder,
        Some(Element::Water) => Attr::Water,
        Some(Element::Aero) => Attr::Aero,
        Some(Element::Light) | None => Attr::Notype,
    }
}

/// The element of a KH3 damage attribute (for hits that are not Donald's: Sora's Thunder on a Wet enemy conducts).
pub fn element_of_attr(a: Attr) -> Option<Element> {
    match a {
        Attr::Fire => Some(Element::Fire),
        Attr::Blizzard => Some(Element::Blizzard),
        Attr::Thunder => Some(Element::Thunder),
        Attr::Water => Some(Element::Water),
        Attr::Aero => Some(Element::Aero),
        _ => None,
    }
}

/// The ability Boost of a spell's elements (9: +20 % for that element; a hybrid: either of its pair), Arcane Boost for
/// Comet / Gravity / Meteor / Ultima.
pub fn boost_of(spell: SpellRef, abilities: &super::abilities::DonAbilities) -> f32 {
    use super::abilities::DonAbility as A;
    let m = magic();
    if let SpellRef::Arcane(line, _) = spell {
        use super::spells::ArcaneLine as L;
        return if matches!(line, L::Comet | L::Gravity | L::Meteor | L::Ultima) && abilities.has(A::ArcaneBoost) { m.boost_arcane } else { 1.0 };
    }
    let has = |e: Element| match e {
        Element::Fire => abilities.has(A::FireBoost),
        Element::Blizzard => abilities.has(A::BlizzardBoost),
        Element::Thunder => abilities.has(A::ThunderBoost),
        Element::Water => abilities.has(A::WaterBoost),
        Element::Aero => abilities.has(A::AeroBoost),
        Element::Light => false,
    };
    if spell.elements().iter().any(has) {
        m.boost_element
    } else {
        1.0
    }
}

// ================================================================================================ one hit

/// What a hit of a spell is.
#[derive(Clone, Debug, PartialEq)]
pub struct HitSpec {
    pub spell: SpellRef,
    /// The motion value (Sora's scale) the damage function takes with Donald's Magic.
    pub power: f32,
    pub tag: HitTag,
    pub applies: Vec<Apply>,
    pub kind: Kind,
    /// Knockback (cm, `move_length`); with `toward` the blow carries the target to that point instead.
    pub push: f32,
    pub toward: Option<Vec3>,
    pub attr: Attr,
    /// Ultima / Zettaflare: no resistance applies.
    pub ignore_resist: bool,
    /// Thermal Shock: this share of the defence is ignored; it breaks guards.
    pub def_ignore: f32,
    /// (ZETTA) A forced hit (`kh_core::host::HIT_FLAG_FORCE`): past armour, poise, guard and every limit (Zettaflare).
    pub force: bool,
    /// A bad status of the hit itself (STUN for a hold) and its seconds.
    pub bad: Option<(&'static str, f32)>,
    /// The hit plays no reaction (a puddle's wetting, a tick).
    pub no_reaction: bool,
    /// Flare: a Burned target's Burn is consumed for +flare.burn_bonus.
    pub flare: bool,
    /// Conduct may chain from this hit (false for the chained hits, Thunderstorm's strikes).
    pub chains: bool,
    /// The Boost (and other multipliers) already known at the cast.
    pub mult: f32,
    pub name: &'static str,
    pub hit_fx: &'static str,
    /// Gravity: the damage is this share of the target's HP (capped on bosses at `gravity_cap` of its max).
    pub hp_share: Option<(f32, f32)>,
}

impl HitSpec {
    /// A hit of `spell` from its row: power, elements, statuses.
    pub fn of(spell: SpellRef, row: &DonSpellRow) -> HitSpec {
        let e = spell.element();
        let elements = spell.elements();
        HitSpec {
            spell,
            power: row.power,
            tag: HitTag::of(elements),
            applies: Apply::parse_list(row.applies),
            kind: Kind::Small,
            push: 0.0,
            toward: None,
            attr: attr_of(if matches!(spell, SpellRef::Hybrid(..)) { spell.elements().iter().find(|x| *x != Element::Light).or(e) } else { e }),
            ignore_resist: false,
            def_ignore: 0.0,
            force: false, // (ZETTA)
            bad: None,
            no_reaction: false,
            flare: false,
            chains: true,
            mult: 1.0,
            name: row.name,
            hit_fx: fx::hit_of(e),
            hp_share: None,
        }
    }

    pub fn power(mut self, p: f32) -> HitSpec {
        self.power = p;
        self
    }

    pub fn kind(mut self, k: Kind) -> HitSpec {
        self.kind = k;
        self
    }

    pub fn applies(mut self, a: Vec<Apply>) -> HitSpec {
        self.applies = a;
        self
    }
}

/// One hit landed (tests and the log).
#[derive(Clone, Debug, PartialEq)]
pub struct Landed {
    pub target: TargetId,
    pub damage: f32,
    pub killed: bool,
    pub outcome: HitOutcome,
    pub kind: Kind,
}

/// What the module did this step (tests, the harness, the debug text): effects spawned by name, hits.
#[derive(Clone, Debug, PartialEq)]
pub enum FxLog {
    Effect { spell: Option<SpellRef>, name: &'static str },
    Hit { spell: SpellRef, target: TargetId, damage: f32, reactions: Vec<Reaction>, kind: Kind },
    Heal { member: MemberId, amount: i32 },
    Sora { line: u8, cmd: u8 },
}

/// The context of a step's hits.
pub struct Ctx<'p, 'h, 'a> {
    pub party: &'p mut Party,
    pub host: &'h mut PartyHost<'a>,
    pub log: &'p mut Vec<FxLog>,
}

/// The enemies (alive and hittable) the spells' areas test.
pub fn enemies(host: &PartyHost) -> Vec<TargetInfo> {
    let mut list = Vec::new();
    host.targets.enumerate(&mut list);
    list.retain(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE));
    list
}

/// The enemies a spell may CHOOSE (Sora's rule: alive and lockable; never a pot or a friendly NPC).
pub fn foes(host: &PartyHost) -> Vec<TargetInfo> {
    let mut list = Vec::new();
    host.targets.enumerate(&mut list);
    list.retain(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE));
    list
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, 0.0)
}

/// The caster's pawn as the attacker (his Magic, abilities, level); None when he is gone.
fn caster_pos(party: &Party, caster: MemberId) -> Option<(Vec3, f32, f32)> {
    party.member(caster).map(|m| (m.capsule_centre(), m.yaw, m.params.capsule_radius))
}

/// Spawns an effect through the caster's handles (none when he is gone).
pub fn spawn_fx(cx: &mut Ctx, caster: MemberId, spell: Option<SpellRef>, name: &'static str, at: Attach, scale: f32, persistent: bool) -> Option<EffectHandle> {
    spawn_fx_tint(cx, caster, spell, name, at, scale, persistent, fx::tint_of(spell, name))
}

/// [`spawn_fx`] with its recolour given (None: its own colours).
#[allow(clippy::too_many_arguments)]
pub fn spawn_fx_tint(cx: &mut Ctx, caster: MemberId, spell: Option<SpellRef>, name: &'static str, at: Attach, scale: f32, persistent: bool, tint: Option<Tint>) -> Option<EffectHandle> {
    if name.is_empty() {
        return None;
    }
    let host = &mut *cx.host;
    let m = cx.party.member_mut(caster)?;
    let h = m.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at, rotation: Vec3::ZERO, scale, persistent });
    if let Some(t) = tint {
        host.effects.tint(h, t);
    }
    if let Some(se) = fx::extra_se(name) {
        m.seam.play(host.audio, &kh_core::host::SoundPlay { id: kh_core::host::SoundId(kh_core::notify::notify_asset_id(se)), name: se, at, volume: 1.0, pitch: 1.0, looping: false, spatial: true });
    }
    push_log(cx.log, FxLog::Effect { spell, name });
    Some(h)
}

fn push_log(log: &mut Vec<FxLog>, e: FxLog) {
    if log.len() >= 512 {
        log.drain(..128);
    }
    log.push(e);
}

/// THE ONE ROUTE OF A SPELL HIT on `t` from `caster` (`from`: where the blow comes from).
pub fn deliver(cx: &mut Ctx, caster: MemberId, spec: &HitSpec, t: &TargetInfo, from: Vec3) -> Option<Landed> {
    let d = data();
    let m = magic();
    let size = size_of(cx.host.targets, t);
    let id = t.id.0;
    // Flare: the Burn is consumed for +50 % (6.1)
    let mut mult = spec.mult;
    if spec.flare {
        if let Some(s) = cx.party.don.status.get_mut(id) {
            if s.is_burned() {
                s.burn = 0.0;
                mult *= 1.0 + m.flare_burn_bonus;
            }
        }
    }
    let outcome = cx.party.don_hit(Some(caster), id, size, spec.tag, &spec.applies);
    mult *= outcome.damage_mult;
    // the reaction's record parts
    let launch = outcome.events.iter().any(|e| matches!(e, StatusEvent::Launch));
    let froze = outcome.events.iter().find_map(|e| if let StatusEvent::Froze(s) = e { Some(*s) } else { None });
    let mut kind = spec.kind;
    if launch && !spec.no_reaction {
        kind = Kind::Parabola;
    }
    let mut bad = spec.bad;
    if let Some(s) = froze {
        bad = Some(("FREEZE", s));
    }
    // the damage: the damage function with Donald's Magic, or Gravity's share of the HP
    let (attacker_status, game_level, apos, ayaw, arad) = {
        let p = cx.party.member(caster)?;
        (p.status.clone(), p.status.game_level, p.capsule_centre(), p.yaw, p.params.capsule_radius)
    };
    let mut defender = Defender::of_target(cx.host.targets.defense(t.id).as_ref());
    // (ZETTA 2026-10-23) a forced hit (Zettaflare: "ignores all armor, it takes priority over ANYTHING"; "make sure Zettaflare
    // scales with your magic stat"): no defence, resistance, cut or per-enemy damage limit: his Magic x its power, nothing else
    if spec.force {
        defender = Defender::NONE;
    }
    if spec.ignore_resist {
        defender.resist = [0; 8];
        defender.second_resist = 0;
    }
    if spec.def_ignore > 0.0 {
        defender.defense = (defender.defense as f32 * (1.0 - spec.def_ignore.clamp(0.0, 1.0))).round() as i32;
    }
    let damage = match spec.hp_share {
        // GRAVITY (the user, 2026-10-22, doc 6.3): a share of the enemy's CURRENT HP (so a big, healthy one loses the most and
        // repeat casts take less as the bar shrinks); the share = the tier's base + Donald's Magic x `gravity.share_per_magic`,
        // at most `gravity.share_cap`; a boss resists `gravity.boss_resist` of it (90 %: a tenth); no other cap
        Some((base, _)) => gravity_damage(t.hp, base, attacker_status.magic(), size == Size::Boss),
        None if spec.power <= 0.0 => 0.0,
        None => {
            let attack = DamageAttack::magic(spec.power, spec.attr);
            compute_damage(&attacker_status.as_attacker(), &defender, &attack, &DamageContext::new(game_level, 1)).amount
        }
    } * mult;
    let mut hit = HitRecord::empty(t.id);
    hit.attack_name = spec.name;
    hit.position = t.center;
    let toward_dir = spec.toward.map(|p| flat(p - t.position).normalize_or_zero());
    let dir = toward_dir.filter(|v| *v != Vec3::ZERO).unwrap_or_else(|| {
        let v = flat(t.position - from).normalize_or_zero();
        if v == Vec3::ZERO {
            Vec3::X
        } else {
            v
        }
    });
    hit.direction = dir;
    hit.damage = damage.max(0.0);
    hit.damage_kind = kind;
    hit.attribute = spec.attr;
    hit.flags |= DON_HIT_FLAG;
    hit.attacker = HitAttacker { position: spec.toward.unwrap_or(apos), yaw: ayaw, radius: arad, valid: true };
    let push = match spec.toward {
        Some(p) => flat(p - t.position).length().min(spec.push.max(0.0)),
        None => spec.push,
    };
    let mut info = HitInfo { power: spec.power, power_scale: 1.0, move_length: push, knockback: "NORMAL", reaction_power: 1, armor_attack_power: 2, ..HitInfo::DEFAULT };
    info.flags50 |= HitInfo::FLAG50_MAGIC;
    if spec.def_ignore > 0.0 {
        info.flags50 |= HitInfo::FLAG50_IGNORE_GUARD;
        info.armor_attack_power = 10;
    }
    if spec.force {
        // (ZETTA) unstoppable: the hosts force it past every guard, armour, poise and limit
        hit.flags |= kh_core::host::HIT_FLAG_FORCE;
        info.flags50 |= HitInfo::FLAG50_IGNORE_GUARD;
        info.armor_attack_power = 999;
        info.reaction_power = 999;
    }
    match bad {
        Some((name, secs)) if secs > 0.0 => {
            info.bad_status = name;
            info.bad_time = secs;
        }
        _ => info.clear_bad_status(),
    }
    if spec.no_reaction {
        info.victim_result = ReactionResult::NoReaction;
    }
    hit.info = info;
    classify(cx.host, &mut hit);
    let res = cx.host.targets.apply_hit(&hit);
    // the reactions' damage side, landed or not (the statuses changed already)
    let landed = res.accepted.then(|| Landed { target: t.id, damage: res.damage_dealt, killed: res.killed, outcome: outcome.clone(), kind });
    if res.accepted {
        if let Some(p) = cx.party.member_mut(caster) {
            p.events.push(crate::view::PartyEventKind::Hit { target: t.id, damage: res.damage_dealt, killed: res.killed, attack: spec.name });
        }
        push_log(cx.log, FxLog::Hit { spell: spec.spell, target: t.id, damage: res.damage_dealt, reactions: outcome.reactions.clone(), kind });
        let _ = spawn_fx(cx, caster, Some(spec.spell), spec.hit_fx, Attach::World(t.center), 1.0, false);
        if outcome.reactions.contains(&Reaction::Shatter) {
            let _ = spawn_fx(cx, caster, Some(spec.spell), fx::FROZEN_OUT, Attach::World(t.center), status_scale(t).0, false); // (DON-SPELLB status looks) sized to it
        }
        if res.killed {
            cx.party.don_forget(id);
        }
    }
    after_reactions(cx, caster, spec, t, &outcome, hit.damage, &d);
    landed
}

/// Conduct's chain and Wildfire's spread (4.5).
pub fn after_reactions(cx: &mut Ctx, caster: MemberId, spec: &HitSpec, t: &TargetInfo, outcome: &HitOutcome, damage: f32, d: &DonData) {
    if let (Some(r), true) = (outcome.conduct_radius, spec.chains) {
        let wet: Vec<u32> = cx.party.don.status.wet_ids().filter(|w| *w != t.id.0).collect();
        let list: Vec<TargetInfo> = enemies(cx.host).into_iter().filter(|e| wet.contains(&e.id.0) && flat(e.position - t.position).length() <= r).collect();
        let m = magic();
        for e in list {
            let mut chained = spec.clone();
            chained.chains = false;
            chained.applies.clear();
            chained.tag = HitTag::of(ElemSet::one(Element::Thunder));
            chained.attr = Attr::Thunder;
            chained.flare = false;
            chained.hp_share = None;
            chained.mult = spec.mult * m.conduct_power;
            chained.kind = Kind::Small;
            chained.push = 0.0;
            chained.toward = None;
            chained.hit_fx = fx::HIT_THUNDER;
            let _ = spawn_fx(cx, caster, Some(spec.spell), fx::CONDUCT_ARC, Attach::World(e.center), 0.6, false);
            deliver(cx, caster, &chained, &e, t.center);
        }
        let _ = damage;
    }
    if let Some(r) = outcome.wildfire_radius {
        let full = cx.party.status(caster).and_then(|s| s.donald.as_ref()).map(|st| st.full_flags(d)).unwrap_or_default();
        let list: Vec<TargetInfo> = enemies(cx.host).into_iter().filter(|e| e.id != t.id && flat(e.position - t.position).length() <= r).collect();
        for e in list {
            let size = size_of(cx.host.targets, &e);
            let mut ev = Vec::new();
            cx.party.don.status.entry(e.id.0, size).apply(Apply::Burn, full, &d.tune, &mut ev);
            let _ = spawn_fx(cx, caster, Some(spec.spell), fx::HIT_FIRE, Attach::World(e.center), 0.8, false);
        }
    }
}

/// KH3's victim classification with the target's body (as the kit's `deliver`).
fn classify(host: &mut PartyHost, hit: &mut HitRecord) {
    let target = hit.target;
    let body = host.targets.body(target, hit.part);
    let shape = kh_core::combat::classify::Shape { no_clash: true, ..Default::default() };
    let (mut armor, mut endurance) = (None, None);
    let mut info = hit.info;
    let keep = info.victim_result;
    let _ = kh_core::combat::classify::classify(&body, kh_core::combat::classify::MapResponse::Other, &shape, hit.damage_kind, hit.attribute, &mut info, &mut |pw, weak| {
        armor = Some((pw, weak));
        i32::MIN
    }, &mut |pw| {
        endurance = Some(pw);
        i32::MIN
    });
    if armor.is_some() || endurance.is_some() {
        let armor_left = armor.map(|(pw, weak)| host.targets.take_armor(target, pw, weak));
        let endurance_left = endurance.map(|pw| host.targets.take_reaction_power(target, pw));
        info = hit.info;
        let _ = kh_core::combat::classify::classify(&body, kh_core::combat::classify::MapResponse::Other, &shape, hit.damage_kind, hit.attribute, &mut info, &mut |_, _| armor_left.unwrap_or(0), &mut |_| endurance_left.unwrap_or(0));
    }
    if keep == ReactionResult::NoReaction {
        info.victim_result = ReactionResult::NoReaction;
    }
    hit.info = info;
}

/// Every enemy within `r` of `at` (and in front within `half_angle` of `yaw` from `at` when given).
pub fn within(host: &PartyHost, at: Vec3, r: f32, arc: Option<(f32, f32)>) -> Vec<TargetInfo> {
    enemies(host)
        .into_iter()
        .filter(|e| {
            let d = flat(e.position - at);
            if d.length() > r + e.radius {
                return false;
            }
            match arc {
                Some((yaw, half)) if half < 179.0 && d.length() > 1.0 => {
                    let a = d.y.atan2(d.x).to_degrees();
                    crate::pawn::wrap180(a - yaw).abs() <= half
                }
                _ => true,
            }
        })
        .collect()
}

// ================================================================================================ the actors

/// Where an actor is aimed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    /// A target and its centre when last seen.
    Target(TargetId, Vec3),
    Point(Vec3),
}

impl Aim {
    pub fn point(&self, host: &PartyHost) -> Vec3 {
        match *self {
            Aim::Target(t, last) => host.targets.info(t).filter(|i| i.flags.has(TargetFlags::ALIVE)).map_or(last, |i| i.center),
            Aim::Point(p) => p,
        }
    }

    pub fn target(&self) -> Option<TargetId> {
        match *self {
            Aim::Target(t, _) => Some(t),
            Aim::Point(_) => None,
        }
    }

    pub fn ground(&self, host: &PartyHost) -> Vec3 {
        match *self {
            Aim::Target(t, last) => host.targets.info(t).map_or(last, |i| i.position),
            Aim::Point(p) => p,
        }
    }
}

/// What a burst does besides hitting (its special rule).
#[derive(Clone, Debug, PartialEq)]
pub enum Special {
    None,
    /// Freeze / Diamond Dust: the freeze rule for `seconds` on enemies up to `max` size (larger: the big slow);
    /// `shards`: Freeze Lv 3's shards Chill each freezer's neighbours.
    Freeze { seconds: f32, max: Size, shards: bool },
    /// Gravity: the crush, the pull and the hold.
    Gravity { hold: f32 },
    /// Sanctuary (Release Light): heals and a short invincibility for the party.
    Sanctuary { heal_share: f32, invincible: f32 },
    /// Prism Frost: `count` shards over the enemies in the cone; a freezer stays Revealed until it thaws.
    Prism { count: u32 },
    /// Radiant Gale: a launched enemy's Reveal is renewed.
    Gale,
    /// Plasma: the Burned it hits arc their Burn to one more enemy each `period` for `life`.
    Plasma { period: f32, life: f32 },
    /// Sunburst: Burned and Revealed enemies take x `bonus`.
    Sunburst { bonus: f32 },
}

/// A one-off hit area.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    pub spec: HitSpec,
    /// Radius round the aim (0: the aimed target only).
    pub radius: f32,
    /// A cone / arc from `origin` (yaw, half angle, reach): the area is the arc instead of the circle.
    pub arc: Option<(Vec3, f32, f32, f32)>,
    /// A line from `origin` along yaw (reach, half width).
    pub line: Option<(Vec3, f32, f32, f32)>,
    pub fx: Vec<&'static str>,
    pub fx_scale: f32,
    pub special: Special,
    /// Only enemies up to this size are hit by this burst's spec as given (bigger: no reaction).
    pub react_max: Option<Size>,
}

impl Burst {
    pub fn new(spec: HitSpec, radius: f32) -> Burst {
        Burst { spec, radius, arc: None, line: None, fx: Vec::new(), fx_scale: 1.0, special: Special::None, react_max: None }
    }

    pub fn fx(mut self, list: &[&'static str]) -> Burst {
        self.fx = list.to_vec();
        self
    }
}

/// Where a lasting field is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Follow {
    Fixed,
    /// Follows a target at a speed (Fire Whirl).
    Target(TargetId, f32),
    /// On the caster (Whiteout).
    Caster,
    /// Moves along a direction (Tempest).
    Drift(Vec3),
}

/// What a field does on its tick.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldRule {
    /// Hits everything inside.
    Area,
    /// Strikes every Wet enemy inside (Thunderstorm), wets the rest.
    WetStrikes,
    /// One strike a tick, round robin over the enemies inside, `left` strikes (Judgment Bolt).
    Storm { left: u32, next: usize },
    /// Hail on everything inside; lightning chains between the Chilled (Hailstorm).
    Hail,
    /// A puddle: wets what stands in it, no damage, no reaction (Flood).
    Puddle,
}

/// A spell actor.
#[derive(Clone, Debug, PartialEq)]
pub enum ActorKind {
    /// A burst after `delay` at the aim.
    Delayed { aim: Aim, delay: f32, burst: Burst },
    /// A ball that flies to the aim, then (after `shrink`) bursts (Flare's star, Plasma, a hurled orb).
    Ball { pos: Vec3, aim: Aim, speed: f32, turn: f32, dir: Vec3, life: f32, shrink: f32, arrived: Option<f32>, burst: Burst, after: Option<(f32, Burst)> },
    /// A lasting area.
    Field { centre: Vec3, follow: Follow, radius: f32, life: f32, tick: f32, next: f32, spec: HitSpec, pull: f32, rule: FieldRule, special: Special },
    /// A front that rolls out from `origin` in an arc (Flood, Tsunami).
    Wave { origin: Vec3, yaw: f32, half: f32, reach: f32, speed: f32, front: f32, spec: HitSpec, hit: Vec<TargetId>, flood: bool },
    /// A bolt that leaps from enemy to enemy (Divine Spark).
    Chain { at: TargetId, pos: Vec3, left: u32, range: f32, gap: f32, next: f32, spec: HitSpec, hit: Vec<TargetId> },
}

/// One actor in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct Actor {
    pub id: u32,
    pub caster: MemberId,
    pub spell: SpellRef,
    pub kind: ActorKind,
    pub age: f32,
    pub effect: Option<EffectHandle>,
    pub done: bool,
}

/// Glacier's ice wall: blocks projectiles (a host reads [`Party::don_walls`]) until it melts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wall {
    pub a: Vec3,
    pub b: Vec3,
    pub half_width: f32,
    pub height: f32,
    pub left: f32,
}

/// THE GRAVITY BALL (the user, 2026-10-22: "the gravity ball should squish down while also moving towards the floor, which
/// is what squishes the enemies. It fades away once it gets thin enough"; numbers `gravity.*` of party-don-magic; the
/// effect vd_don_grv1..3 is authored from the same rule by DON-MAGIC make_gravity.js): at `t` seconds from its spawn it
/// swells in, and from `press_at` presses down onto the floor over `press_time` (pressing harder as it goes): its height
/// share falls to `fx_floor_thin`, its width grows to `press_width`, its bottom sinks to the floor. Returns its top above
/// the floor (cm).
pub fn gravity_ball_top(lv: u8, t: f32) -> f32 {
    let m = magic();
    let d = 100.0 * match lv {
        0 | 1 => m.gravity_fx_diameter_1,
        2 => m.gravity_fx_diameter_2,
        _ => m.gravity_fx_diameter_3,
    };
    let c = m.gravity_fx_centre + m.gravity_fx_centre_step * (lv.max(1) - 1) as f32;
    let press = gravity_ball_press(t);
    let tall = 1.0 - (1.0 - m.gravity_fx_floor_thin) * press;
    // (YFX, the user 2026-10-23: "the whole gravity object should try to move into the floor, with the eletrical beam at the
    // midway point of the orb being where it stops") the whole ball sinks as one piece: its centre (the band) from `c` to the
    // floor over the press while it squishes; the ground hides its lower half (make_gravity.js `ball`, the same rule)
    let centre = c * (1.0 - press);
    centre + d * tall / 2.0
}

/// (YFX) THE BALL'S SQUISH CURVE: 0 before the press, rising to 1 when it is pressed flat (`t` s from its spawn); the
/// ball's height share is `1 - (1 - fx_floor_thin) * press` (make_gravity.js `ball`) and a flattened enemy's
/// `1 - (1 - flat_height) * press` ([`Squash::scale`]): the same curve on the same clock.
pub fn gravity_ball_press(t: f32) -> f32 {
    let m = magic();
    let p = ((t - m.gravity_press_at) / m.gravity_press_time.max(1e-3)).clamp(0.0, 1.0);
    p * p
}

/// GRAVITY'S FLATTEN (the user, 2026-10-22: "when it's used on an enemy, it flattens them, then about half a second after
/// the spell ends they return to normal"; "gravity does not do the flatten effect on bosses, however"; "the gravity ball
/// should squish down ... which is what squishes the enemies"): from the crush (`press_at` after the cast) each enemy is
/// squashed BY THE BALL: while the ball's top ([`gravity_ball_top`]) is below the enemy's own height, its height share is
/// that top over its height (at least `gravity.flat_height`), a little wider as it flattens (`gravity.flat_width` when
/// flat); once the press is over it is flat for the rest of the spell's hold (its row's `duration`: Lv 1 2 s, Lv 2 3 s,
/// Lv 3 4 s), and `gravity.restore_delay` seconds after the hold springs back over `gravity.restore_time`. Never a boss
/// (the ball still presses down on it). The hosts scale the drawn model by [`Party::don_squash`]: the test level its
/// capsule, Elden Ring the character's model scale (kh-er donstatus.rs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Squash {
    pub target: u32,
    /// Seconds since it was flattened.
    pub age: f32,
    /// The spell's hold (s).
    pub hold: f32,
    /// Its height (cm) and the tier of the ball pressing on it.
    pub height: f32,
    pub lv: u8,
}

impl Squash {
    /// When it is back to normal (and dropped).
    pub fn end(&self) -> f32 {
        let m = magic();
        self.flat_until() + m.gravity_restore_delay + m.gravity_restore_time
    }

    /// (DON-SPELLB gif) It stays pressed until the effect ends (`gravity.fx_total` after the cast, the GIF's length), counted
    /// from the crush; the spell's hold (STUN) is the row's own and may be longer.
    pub fn flat_until(&self) -> f32 {
        let m = magic();
        (m.gravity_fx_total - m.gravity_press_at).max(m.gravity_press_time)
    }

    /// The drawn scale now: (width share, height share); (1, 1) = normal.
    pub fn scale(&self) -> (f32, f32) {
        let m = magic();
        // (YFX) never left poking out of the sunk ball: a body so tall that the ball was grown for it is pressed down to the
        // pressed ball's top (the ball's end top / its height) when that is lower than `flat_height` (the same curve)
        let ball_d = 100.0 * match self.lv { 0 | 1 => m.gravity_fx_diameter_1, 2 => m.gravity_fx_diameter_2, _ => m.gravity_fx_diameter_3 } * super::yfx::gravity_scale(self.lv, self.height);
        let flat_h = m.gravity_flat_height.min(ball_d * m.gravity_fx_floor_thin / 2.0 / self.height.max(1.0)).clamp(0.05, 1.0);
        let back = self.flat_until() + m.gravity_restore_delay;
        // (YFX, the user 2026-10-23: "The enemys also need to be squeezed down from it in time with the ball squeezing") the
        // ball's own squish curve on its clock (the press began at the crush, age 0): any enemy, whatever its height, goes
        // from its own height to `flat_height` of it exactly as the ball goes from round to `fx_floor_thin`, and stays so
        // (the old rule, the ball's top over the enemy's height, never touched a small enemy and popped a tall one at once)
        let pressed = 1.0 - (1.0 - flat_h) * gravity_ball_press(m.gravity_press_at + self.age);
        let h = if self.age < back {
            pressed
        } else if self.age < back + m.gravity_restore_time {
            // springs back (ease out)
            let t = ((self.age - back) / m.gravity_restore_time.max(1e-3)).clamp(0.0, 1.0);
            flat_h + (1.0 - flat_h) * (1.0 - (1.0 - t) * (1.0 - t))
        } else {
            1.0
        };
        let k = if flat_h < 1.0 { ((1.0 - h) / (1.0 - flat_h)).clamp(0.0, 1.0) } else { 0.0 };
        (1.0 + (m.gravity_flat_width - 1.0) * k, h)
    }
}

/// GRAVITY'S CRUSH (pure): what an enemy with `hp` HP left loses to a tier of base share `base` cast with Magic `magic`.
pub fn gravity_share(base: f32, magic_stat: i32, boss: bool) -> f32 {
    let m = magic();
    let share = (base + magic_stat.max(0) as f32 * m.gravity_share_per_magic).clamp(0.0, m.gravity_share_cap);
    if boss {
        share * (1.0 - m.gravity_boss_resist).clamp(0.0, 1.0)
    } else {
        share
    }
}

/// The HP Gravity takes off an enemy with `hp` HP left (at least `gravity.min_damage`).
pub fn gravity_damage(hp: f32, base: f32, magic_stat: i32, boss: bool) -> f32 {
    (hp.max(0.0) * gravity_share(base, magic_stat, boss)).max(magic_min_damage())
}

fn magic_min_damage() -> f32 {
    magic().gravity_min_damage
}

/// The base share of Gravity's tier `lv` (1 Gravity, 2 Gravira, 3 Graviga).
pub fn gravity_base(lv: u8) -> f32 {
    let m = magic();
    match lv {
        0 | 1 => m.gravity_share_base_1,
        2 => m.gravity_share_base_2,
        _ => m.gravity_share_base_3,
    }
}

/// One step of Gravity's flattens: they age; a restored one is dropped.
pub fn step_squash(fxw: &mut MagicFx, dt: f32) {
    for q in fxw.squash.iter_mut() {
        q.age += dt;
    }
    fxw.squash.retain(|q| q.age < q.end());
}

impl Party {
    /// Gravity's flatten of an enemy as the hosts draw it: (width share, height share); None = drawn as it is.
    pub fn don_squash(&self, target: u32) -> Option<(f32, f32)> {
        self.don_fx.squash.iter().find(|q| q.target == target).map(Squash::scale)
    }

    /// Every flattened enemy now: (target, width share, height share).
    pub fn don_squashes(&self) -> Vec<(u32, f32, f32)> {
        self.don_fx.squash.iter().map(|q| { let (w, h) = q.scale(); (q.target, w, h) }).collect()
    }
}

/// A Plasma arc: a Burned enemy passes its Burn on.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ArcRun {
    caster: MemberId,
    from: TargetId,
    left: f32,
    next: f32,
    period: f32,
}

/// A status's effect shown on an enemy.
#[derive(Clone, Copy, Debug, PartialEq)]
struct StatusFx {
    target: u32,
    /// (DON-SPELLB) slot x 8 + variant (Chill's stacks, Reveal / Exposed): a new variant swaps the look.
    which: u8,
    handle: EffectHandle,
    /// (DON-SPELLB) Wet: seconds to the next splash.
    clock: f32,
}

/// The party's spell world (`Party::don_fx`).
#[derive(Clone, Debug, Default)]
pub struct MagicFx {
    pub actors: Vec<Actor>,
    /// Sora's actors cast by each Donald (line 1, the Cure line), and what each cast's hits multiply by.
    pub casters: Vec<(MemberId, kh_core::magic::caster::SpellCaster)>,
    /// The spell cast by each member's caster last (its row's statuses and power scale for the actor's hits).
    pub caster_spell: Vec<(MemberId, SpellRef)>,
    pub worn: super::allyfx::Worn,
    pub walls: Vec<Wall>,
    pub log: Vec<FxLog>,
    arcs: Vec<ArcRun>,
    status_fx: Vec<StatusFx>,
    /// Heals and effects for Sora (the step's output carries them).
    pub sora_heals: Vec<(MemberId, i32, u8)>,
    /// BLIND (the user, 2026-10-22): each Blind enemy and the spot its target stood at when it was blinded.
    pub blind_at: Vec<(u32, Vec3)>,
    /// TEST / DEBUG (the test level's `party don-cast ID` word): spells a Donald casts at the next step, priced by the
    /// rules when they allow it, else free (to look at every spell's effects).
    pub debug_casts: Vec<SpellRef>,
    /// TEST / DEBUG (`party don-cast ID sora`): the next debug casts' ally spells go on Sora.
    pub debug_on_sora: bool,
    /// (the user, 2026-10-22) GRAVITY'S FLATTEN: the enemies it squashed (never a boss), see [`Squash`].
    pub squash: Vec<Squash>,
    /// (DON-SPELLB) the ultimates' and Releases' timed volleys of effects (spell_payload.rs).
    pub timed: Vec<super::spell_payload::Timed>,
    /// (YFX) the Y spells' presentation: their cues, falling bodies, sounds (yfx.rs).
    pub yfx: super::yfx::YState,
    /// (HYB) the hybrids' bodies, cues, movers and repeaters (hybrid_look.rs).
    pub hyb: super::hybrid_look::HybLook,
    /// (KITFX) the looping flight sounds of actors (Flare's star): faded out when the actor is gone.
    pub kitfx_hums: Vec<(u32, kh_core::host::SoundHandle)>,
    next: u32,
}

impl MagicFx {
    pub fn add(&mut self, caster: MemberId, spell: SpellRef, kind: ActorKind) -> u32 {
        self.next += 1;
        self.actors.push(Actor { id: self.next, caster, spell, kind, age: 0.0, effect: None, done: false });
        self.next
    }

    /// The actors of one spell alive now (tests, the debug text).
    pub fn alive(&self, spell: SpellRef) -> usize {
        self.actors.iter().filter(|a| a.spell == spell && !a.done).count()
    }

    /// Clears the log (tests).
    pub fn take_log(&mut self) -> Vec<FxLog> {
        std::mem::take(&mut self.log)
    }
}

impl Party {
    /// (DON-MAGIC) BLIND (the user, 2026-10-22: "blind should make enemies attack the position they last saw you before
    /// they were blinded"): the host feeds, every step, where each enemy's current target stands; a Blind enemy keeps
    /// the spot of the step it went Blind ([`Party::don_blind_point`]) until it sees again. Nothing is redirected:
    /// whoever stands in that spot is hit ([`blind_swing_hits`]).
    pub fn don_blind_track(&mut self, enemy: u32, target_at: Vec3) {
        let blind = self.don.status.get(enemy).is_some_and(|s| s.is_blind());
        let known = self.don_fx.blind_at.iter().any(|b| b.0 == enemy);
        if blind && !known {
            self.don_fx.blind_at.push((enemy, target_at));
        } else if !blind && known {
            self.don_fx.blind_at.retain(|b| b.0 != enemy);
        }
    }

    /// (DON-MAGIC) THE SPEED SHARE an enemy at `pos` runs at: Frozen 0, Chill / the large-enemy slow (4.2, 6.1), and
    /// DON-PROG's Blizzard attunement aura (enemies near Donald -15 %, `aura.enemy_speed`); 1 = normal.
    pub fn don_enemy_speed(&self, enemy: u32, pos: Vec3) -> f32 {
        let t = data().tune;
        let own = self.don.status.get(enemy).map_or(1.0, |s| if s.is_frozen() { 0.0 } else { 1.0 - s.slow(&t) });
        own * self.don_prog.aura.enemy_speed(pos)
    }

    /// (DON-MAGIC) The spot a Blind enemy attacks (None: it sees).
    pub fn don_blind_point(&self, enemy: u32) -> Option<Vec3> {
        if !self.don.status.get(enemy).is_some_and(|s| s.is_blind()) {
            return None;
        }
        self.don_fx.blind_at.iter().find(|b| b.0 == enemy).map(|b| b.1)
    }

    /// (DON-MAGIC) Glacier's walls standing now (a host stops enemy projectiles at them).
    pub fn don_walls(&self) -> &[Wall] {
        &self.don_fx.walls
    }
}

// ================================================================================================ the step

fn burst_now(cx: &mut Ctx, fxw: &mut MagicFx, caster: MemberId, spell: SpellRef, at: Vec3, aim_target: Option<TargetId>, b: &Burst) -> Vec<Landed> {
    let mut out = Vec::new();
    for name in &b.fx {
        let _ = spawn_fx(cx, caster, Some(spell), name, Attach::World(at), b.fx_scale, false);
    }
    let from = caster_pos(cx.party, caster).map_or(at, |c| c.0);
    let mut list: Vec<TargetInfo> = if let Some((o, yaw, half, reach)) = b.arc {
        within(cx.host, o, reach, Some((yaw, half)))
    } else if let Some((o, yaw, reach, half_w)) = b.line {
        let f = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
        enemies(cx.host)
            .into_iter()
            .filter(|e| {
                let d = flat(e.position - o);
                let along = d.dot(f);
                along >= -e.radius && along <= reach + e.radius && (d - f * along).length() <= half_w + e.radius
            })
            .collect()
    } else if b.radius > 0.0 {
        within(cx.host, at, b.radius, None)
    } else {
        Vec::new()
    };
    // the aimed target always (a single-target spell); first in the list
    if let Some(t) = aim_target.and_then(|t| cx.host.targets.info(t)).filter(|i| i.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE)) {
        list.retain(|e| e.id != t.id);
        list.insert(0, t);
    }
    match &b.special {
        Special::Sanctuary { heal_share, invincible } => {
            sanctuary(cx, fxw, caster, *heal_share, *invincible);
            return out;
        }
        Special::Prism { count } => {
            if list.is_empty() {
                return out;
            }
            let per = b.spec.power / (*count).max(1) as f32;
            for i in 0..(*count).max(1) {
                let e = list[i as usize % list.len()];
                let spec = b.spec.clone().power(per);
                if let Some(l) = deliver(cx, caster, &spec, &e, from) {
                    // a freezer stays Revealed until it thaws (6.4 Prism Frost)
                    if let Some(s) = cx.party.don.status.get_mut(e.id.0) {
                        if s.is_frozen() {
                            s.reveal = s.reveal.max(s.frozen);
                        }
                    }
                    out.push(l);
                }
            }
            return out;
        }
        _ => {}
    }
    for e in list {
        let mut spec = b.spec.clone();
        if let Some(max) = b.react_max {
            if size_of(cx.host.targets, &e) > max {
                spec.kind = Kind::Small;
                spec.push = 0.0;
                spec.toward = None;
            }
        }
        match &b.special {
            Special::Freeze { seconds, max, .. } => {
                // the freeze rule with its own length; bigger than `max`: the slow (status.rs freeze_for)
                let size = size_of(cx.host.targets, &e);
                let d = data();
                let freezes = size <= *max || size.freeze_proof();
                // what the freeze will do (on a copy), so the light hit carries it; then the hit; then the freeze itself
                // (a hit on ice chips it: the ice comes after its own hit)
                let mut ev = Vec::new();
                let mut copy = *cx.party.don.status.entry(e.id.0, size);
                if freezes {
                    copy.freeze_for(*seconds, &d.tune, &mut ev);
                }
                let froze = ev.iter().find_map(|x| if let StatusEvent::Froze(s) = x { Some(*s) } else { None });
                if let Some(s) = froze {
                    spec.bad = Some(("FREEZE", s));
                    let _ = spawn_fx(cx, caster, Some(spell), fx::FREEZE_PRISON, Attach::Target { target: e.id, offset: Vec3::ZERO }, 1.0, false);
                }
                spec.applies.retain(|a| !matches!(a, Apply::Frozen));
                if let Some(l) = deliver(cx, caster, &spec, &e, from) {
                    out.push(l);
                }
                if freezes {
                    let mut ev = Vec::new();
                    cx.party.don.status.entry(e.id.0, size).freeze_for(*seconds, &d.tune, &mut ev);
                }
                if froze.is_some() {
                    if let Special::Freeze { shards: true, .. } = b.special {
                        shards(cx, caster, spell, &e);
                    }
                }
                continue;
            }
            Special::Gravity { hold } => {
                spec.toward = Some(at);
                spec.bad = Some(("STUN", *hold));
                spec.kind = Kind::Blow;
            }
            Special::Sunburst { bonus } => {
                if cx.party.don.status.get(e.id.0).is_some_and(|s| s.is_burned() && s.is_revealed()) {
                    spec.mult *= *bonus;
                }
            }
            _ => {}
        }
        let landed = deliver(cx, caster, &spec, &e, from);
        if let Some(l) = landed {
            match &b.special {
                Special::Gale if l.kind == Kind::Parabola && magic().gale_launch_reveal > 0.0 => {
                    if let Some(s) = cx.party.don.status.get_mut(e.id.0) {
                        s.reveal = s.reveal.max(data().tune.reveal_duration);
                    }
                }
                Special::Plasma { period, life } => {
                    if cx.party.don.status.get(e.id.0).is_some_and(|s| s.is_burned()) {
                        fxw.arcs.retain(|a| a.from != e.id);
                        fxw.arcs.push(ArcRun { caster, from: e.id, left: *life, next: *period, period: *period });
                    }
                }
                // (the user, 2026-10-22) Gravity flattens what it hits, never a boss (it still takes the damage and the hold)
                Special::Gravity { hold } if size_of(cx.host.targets, &e) != Size::Boss => {
                    let lv = match spell {
                        SpellRef::Arcane(_, lv) => lv,
                        _ => 1,
                    };
                    fxw.squash.retain(|q| q.target != e.id.0);
                    fxw.squash.push(Squash { target: e.id.0, age: 0.0, hold: *hold, height: (2.0 * e.half_height).max(1.0), lv });
                }
                _ => {}
            }
            out.push(l);
        }
    }
    out
}

/// Freeze Lv 3: the freezer throws ice shards that Chill its neighbours.
fn shards(cx: &mut Ctx, caster: MemberId, spell: SpellRef, e: &TargetInfo) {
    let m = magic();
    let _ = spawn_fx(cx, caster, Some(spell), fx::FREEZE_SHARDS, Attach::World(e.center), 1.0, false);
    let near: Vec<TargetInfo> = within(cx.host, e.position, m.freeze_shard_radius, None).into_iter().filter(|x| x.id != e.id).collect();
    for n in near {
        let spec = HitSpec {
            spell,
            power: m.freeze_shard_power,
            tag: HitTag::of(ElemSet::one(Element::Blizzard)),
            applies: vec![Apply::Chill(m.freeze_shard_chill.max(1.0) as u8)],
            kind: Kind::Small,
            push: 0.0,
            toward: None,
            attr: Attr::Blizzard,
            ignore_resist: false,
            def_ignore: 0.0,
            force: false, // (ZETTA)
            bad: None,
            no_reaction: false,
            flare: false,
            chains: true,
            mult: 1.0,
            name: "Freeze shard",
            hit_fx: fx::HIT_BLIZZARD,
            hp_share: None,
        };
        deliver(cx, caster, &spec, &n, e.center);
    }
}

/// Sanctuary (Release Light): heals the party and grants a short invincibility (4.3).
fn sanctuary(cx: &mut Ctx, fxw: &mut MagicFx, caster: MemberId, share: f32, invincible: f32) {
    let ids: Vec<MemberId> = cx.party.members().collect();
    for id in ids {
        let Some(max) = cx.party.status(id).filter(|s| !s.ko()).map(|s| s.max_hp()) else { continue };
        let amount = ((max as f32) * share).round().max(1.0) as i32;
        cx.party.heal(id, amount, 1);
        push_log(cx.log, FxLog::Heal { member: id, amount });
        fxw.worn.grant_invincible(id, invincible);
        let at = Attach::party(PartyMember(id.0), None);
        for name in fx::SANCTUARY {
            let _ = spawn_fx(cx, caster, None, name, at, 1.0, false);
        }
    }
    // Sora (the player or AI Sora): the host heals him from the step's output
    fxw.sora_heals.push((caster, -1, 1));
    fxw.worn.grant_invincible(super::cast::SORA_MEMBER, invincible);
}

/// One step of every actor (after the members' steps).
pub fn step_actors(fxw: &mut MagicFx, party: &mut Party, host: &mut PartyHost, dt: f32) {
    let m = magic();
    let mut log = std::mem::take(&mut fxw.log);
    let mut actors = std::mem::take(&mut fxw.actors);
    {
        let mut cx = Ctx { party, host, log: &mut log };
        for a in actors.iter_mut() {
            if a.done {
                continue;
            }
            a.age += dt;
            let (caster, spell) = (a.caster, a.spell);
            match &mut a.kind {
                ActorKind::Delayed { aim, delay, burst } => {
                    *delay -= dt;
                    if *delay <= 0.0 {
                        let at = aim.point(cx.host);
                        let b = burst.clone();
                        let t = aim.target();
                        burst_now(&mut cx, fxw, caster, spell, at, t, &b);
                        a.done = true;
                    }
                }
                ActorKind::Ball { pos, aim, speed, turn, dir, life, shrink, arrived, burst, after } => {
                    match arrived {
                        Some(left) => {
                            *left -= dt;
                            if *left <= 0.0 {
                                let at = *pos;
                                let b = burst.clone();
                                let t = aim.target();
                                burst_now(&mut cx, fxw, caster, spell, at, t, &b);
                                if let Some((delay, ab)) = after.take() {
                                    // the after-flare (Flare Lv 3)
                                    let id_aim = t.map_or(Aim::Point(at), |t| Aim::Target(t, at));
                                    let next = ActorKind::Delayed { aim: id_aim, delay, burst: ab };
                                    fxw.next += 1;
                                    fxw.actors.push(Actor { id: fxw.next, caster, spell, kind: next, age: 0.0, effect: None, done: false });
                                }
                                a.done = true;
                            }
                        }
                        None => {
                            let goal = aim.point(cx.host);
                            let to = goal - *pos;
                            let dist = to.length();
                            let step = *speed * dt;
                            // soft homing: the heading turns at most `turn` deg/s
                            let want = to.normalize_or_zero();
                            if want != Vec3::ZERO {
                                let max = (*turn * dt).to_radians();
                                let ang = dir.angle_between(want);
                                *dir = if ang <= max || ang.is_nan() { want } else { dir.lerp(want, max / ang).normalize_or_zero() };
                            }
                            if dist <= step.max(m.bolt_hit_radius) {
                                *pos = goal;
                                *arrived = Some(*shrink);
                            } else {
                                *pos += *dir * step;
                            }
                            // (the user's change of 2026-10-22) a ball with a body (Ball Lightning's hurled balls) carries it
                            if let Some(h) = a.effect {
                                cx.host.effects.place(h, *pos);
                            }
                            *life -= dt;
                            if *life <= 0.0 && arrived.is_none() {
                                *arrived = Some(0.0);
                            }
                            // Glacier's walls stop balls too (they are projectiles)
                            if fxw.walls.iter().any(|w| wall_blocks(w, *pos)) && arrived.is_none() {
                                *arrived = Some(0.0);
                            }
                        }
                    }
                }
                ActorKind::Field { centre, follow, radius, life, tick, next, spec, pull, rule, special } => {
                    match *follow {
                        Follow::Fixed => {}
                        Follow::Caster => {
                            if let Some(p) = cx.party.member(caster) {
                                *centre = p.position();
                            }
                        }
                        Follow::Target(t, v) => {
                            if let Some(i) = cx.host.targets.info(t).filter(|i| i.flags.has(TargetFlags::ALIVE)) {
                                let d = flat(i.position - *centre);
                                let s = v * dt;
                                *centre += if d.length() <= s { d } else { d.normalize_or_zero() * s };
                            }
                        }
                        Follow::Drift(v) => *centre += v * dt,
                    }
                    *life -= dt;
                    *next -= dt;
                    if *next <= 0.0 {
                        *next += tick.max(0.05);
                        let c = *centre;
                        let r = *radius;
                        let inside = within(cx.host, c, r, None);
                        let from = c;
                        match rule {
                            FieldRule::Area | FieldRule::Puddle => {
                                for e in inside {
                                    let mut s = spec.clone();
                                    if *pull > 0.0 {
                                        s.toward = Some(c);
                                        s.push = *pull;
                                        s.kind = Kind::Blow;
                                    }
                                    if let Special::Sunburst { bonus } = special {
                                        if cx.party.don.status.get(e.id.0).is_some_and(|st| st.is_burned() && st.is_revealed()) {
                                            s.mult *= *bonus;
                                        }
                                    }
                                    deliver(&mut cx, caster, &s, &e, from);
                                }
                            }
                            FieldRule::WetStrikes => {
                                let mut wet_pts = Vec::new(); // (HYB) the arcs chaining between the Wet
                                for e in inside {
                                    let wet = cx.party.don.status.get(e.id.0).is_some_and(|s| s.is_wet());
                                    let mut s = spec.clone();
                                    s.chains = false;
                                    if wet {
                                        s.applies.clear();
                                        s.tag = HitTag::of(ElemSet::one(Element::Thunder));
                                        s.attr = Attr::Thunder;
                                        let _ = spawn_fx(&mut cx, caster, Some(spell), fx::THUNDERSTORM[1], Attach::World(e.center), 1.0, false);
                                        wet_pts.push(e.center); // (HYB)
                                    } else {
                                        s.power = 0.0;
                                        s.no_reaction = true;
                                        s.tag = HitTag::of(ElemSet::one(Element::Water));
                                        s.attr = Attr::Water;
                                    }
                                    deliver(&mut cx, caster, &s, &e, from);
                                }
                                super::hybrid_look::chain(&mut cx, caster, spell, &wet_pts); // (HYB)
                            }
                            FieldRule::Storm { left, next: k } => {
                                if *left == 0 || inside.is_empty() {
                                    if *left == 0 {
                                        *life = 0.0;
                                    }
                                } else {
                                    let e = inside[*k % inside.len()];
                                    *k += 1;
                                    *left -= 1;
                                    // (DON-SPELLB) the softened strike: a mild flash on every 3rd, none on the others
                                    let bolt = fx::JUDGMENT_SOFT[usize::from(*k % 3 != 1)];
                                    let _ = spawn_fx(&mut cx, caster, Some(spell), bolt, Attach::World(e.position), 1.0, false);
                                    deliver(&mut cx, caster, spec, &e, e.center + Vec3::Z * 500.0);
                                }
                            }
                            FieldRule::Hail => {
                                let mut chilled = Vec::new();
                                for e in inside.iter() {
                                    let mut s = spec.clone();
                                    s.tag = HitTag::of(ElemSet::one(Element::Blizzard));
                                    s.attr = Attr::Blizzard;
                                    deliver(&mut cx, caster, &s, e, from);
                                    if cx.party.don.status.get(e.id.0).is_some_and(|st| st.is_chilled() || st.is_frozen()) {
                                        chilled.push(*e);
                                    }
                                }
                                // lightning chains between the Chilled (each Chilled one struck once per tick)
                                let mut done: Vec<TargetId> = Vec::new();
                                for i in 0..chilled.len() {
                                    let a = chilled[i];
                                    if done.contains(&a.id) {
                                        continue;
                                    }
                                    let group: Vec<TargetInfo> = chilled.iter().copied().filter(|b| !done.contains(&b.id) && flat(b.position - a.position).length() <= m.hail_chain_range).collect();
                                    if group.len() < 2 {
                                        continue;
                                    }
                                    for b in group {
                                        done.push(b.id);
                                        let mut s = spec.clone();
                                        s.applies.clear();
                                        s.tag = HitTag::of(ElemSet::one(Element::Thunder));
                                        s.attr = Attr::Thunder;
                                        s.chains = false;
                                        let _ = spawn_fx(&mut cx, caster, Some(spell), fx::HAIL[1], Attach::World(b.center), 0.8, false);
                                        super::hybrid_look::chain(&mut cx, caster, spell, &[a.center, b.center]); // (HYB)
                                        deliver(&mut cx, caster, &s, &b, a.center);
                                    }
                                }
                            }
                        }
                    }
                    if *life <= 0.0 {
                        a.done = true;
                    }
                }
                ActorKind::Wave { origin, yaw, half, reach, speed, front, spec, hit, flood } => {
                    let before = *front;
                    *front = (*front + *speed * dt).min(*reach);
                    let list = within(cx.host, *origin, *front, Some((*yaw, *half)));
                    for e in list {
                        if hit.contains(&e.id) {
                            continue;
                        }
                        let dist = flat(e.position - *origin).length();
                        if dist + e.radius < before.min(*front) - *speed * dt * 2.0 && before > 0.0 {
                            // already behind the front when the wave began this step (it hit them then)
                        }
                        hit.push(e.id);
                        let mut s = spec.clone();
                        let size = size_of(cx.host.targets, &e);
                        if *flood {
                            if size as u8 + 1 > m.flood_interrupt_size as u8 {
                                // large / boss: no interruption, no push
                                s.kind = Kind::Small;
                                s.push = 0.0;
                            } else if matches!(spell, SpellRef::Line2(Element::Water, 3)) && size == Size::Small {
                                // Lv 3 knocks small enemies down
                                s.kind = Kind::Parabola;
                            }
                            // pushed into a wall: extra damage and a 1 s stun
                            if s.push > 0.0 {
                                let away = flat(e.position - *origin).normalize_or_zero();
                                let probe = e.center + away * (s.push + e.radius + m.flood_wall_probe);
                                if cx.host.world.line_trace(e.center, probe).is_some() {
                                    s.mult *= m.flood_wall_mult;
                                    s.bad = Some(("STUN", m.flood_wall_stun));
                                }
                            }
                        }
                        let _ = spawn_fx(&mut cx, caster, Some(spell), fx::HIT_WATER, Attach::World(e.center), 1.0, false);
                        deliver(&mut cx, caster, &s, &e, *origin);
                    }
                    if *front >= *reach {
                        a.done = true;
                    }
                }
                ActorKind::Chain { at, pos, left, range, gap, next, spec, hit } => {
                    *next -= dt;
                    if *next <= 0.0 {
                        *next = *gap;
                        let cur = cx.host.targets.info(*at).filter(|i| i.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE));
                        match cur {
                            Some(t) if !hit.contains(&t.id) => {
                                let revealed_before = cx.party.don.status.get(t.id.0).is_some_and(|s| s.is_revealed());
                                super::hybrid_look::spark_hop(&mut cx, caster, spell, *pos, t.center); // (HYB) the golden bolt from the last one, the holy burst
                                deliver(&mut cx, caster, spec, &t, *pos);
                                hit.push(t.id);
                                *pos = t.center;
                                if revealed_before {
                                    // it jumps once more for every enemy already Revealed (6.4)
                                    *left += 1;
                                }
                                if *left == 0 {
                                    a.done = true;
                                } else {
                                    *left -= 1;
                                    let p = *pos;
                                    let nxt = foes(cx.host).into_iter().filter(|e| !hit.contains(&e.id) && (e.center - p).length() <= *range).min_by(|x, y| (x.center - p).length().total_cmp(&(y.center - p).length()));
                                    match nxt {
                                        Some(n) => *at = n.id,
                                        None => a.done = true,
                                    }
                                }
                            }
                            _ => a.done = true,
                        }
                    }
                }
            }
        }
        // the Plasma arcs
        let mut arcs = std::mem::take(&mut fxw.arcs);
        for arc in arcs.iter_mut() {
            arc.left -= dt;
            arc.next -= dt;
            if arc.next <= 0.0 && arc.left > 0.0 {
                arc.next += arc.period;
                let Some(src) = cx.host.targets.info(arc.from) else {
                    arc.left = 0.0;
                    continue;
                };
                let burned: Vec<u32> = cx.party.don.status.entries.iter().filter(|e| e.1.is_burned()).map(|e| e.0).collect();
                let nxt = foes(cx.host).into_iter().filter(|e| e.id != src.id && !burned.contains(&e.id.0) && flat(e.position - src.position).length() <= m.plasma_arc_range).min_by(|x, y| (x.position - src.position).length().total_cmp(&(y.position - src.position).length()));
                if let Some(n) = nxt {
                    let d = data();
                    let size = size_of(cx.host.targets, &n);
                    let full = cx.party.status(arc.caster).and_then(|s| s.donald.as_ref()).map(|st| st.full_flags(&d)).unwrap_or_default();
                    let mut ev = Vec::new();
                    cx.party.don.status.entry(n.id.0, size).apply(Apply::Burn, full, &d.tune, &mut ev);
                    super::hybrid_look::plasma_arc(&mut cx, arc.caster, src.center, n.center); // (HYB) the violet arc and the strike
                    arc.from = n.id;
                }
            }
        }
        arcs.retain(|a| a.left > 0.0);
        fxw.arcs.extend(arcs);
        // walls melt
        for w in fxw.walls.iter_mut() {
            w.left -= dt;
        }
        fxw.walls.retain(|w| w.left > 0.0);
    }
    // persistent body effects of finished actors end
    for a in actors.iter().filter(|a| a.done) {
        if let Some(h) = a.effect {
            host.effects.stop(h, false);
        }
    }
    actors.retain(|a| !a.done);
    actors.append(&mut fxw.actors);
    fxw.actors = actors;
    // (KITFX) the flight hums of the actors that are gone (Flare's star detonated) fade out
    for (id, h) in std::mem::take(&mut fxw.kitfx_hums) {
        if fxw.actors.iter().any(|a| a.id == id && !a.done) {
            fxw.kitfx_hums.push((id, h));
        } else {
            host.audio.stop(h, fx::FLARE_HUM_FADE);
        }
    }
    fxw.log = log;
}

fn wall_blocks(w: &Wall, p: Vec3) -> bool {
    let ab = flat(w.b - w.a);
    let ap = flat(p - w.a);
    let t = if ab.length_squared() > 0.0 { (ap.dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    let q = w.a + ab * t;
    flat(p - q).length() <= w.half_width && p.z >= w.a.z - 50.0 && p.z <= w.a.z + w.height
}

/// A Blind enemy's swing at its recorded spot reaches someone standing at `at` (sheet `blind.swing_reach`).
pub fn blind_swing_hits(spot: Vec3, at: Vec3) -> bool {
    flat(at - spot).length() <= magic().blind_swing_reach
}

/// A point on a wall's line is inside it (a host's projectile test).
pub fn blocked_by_walls(walls: &[Wall], p: Vec3) -> bool {
    walls.iter().any(|w| wall_blocks(w, p))
}

/// GLACIER'S WALLS BLOCK PROJECTILES (hosts): the first point of the path `p0` -> `p1` (cm) that is inside a wall (None:
/// the path is clear). The path is sampled finer than half a wall's thickness, so a fast shot cannot skip through.
pub fn wall_crossing(walls: &[Wall], p0: Vec3, p1: Vec3) -> Option<Vec3> {
    if walls.is_empty() {
        return None;
    }
    let thin = walls.iter().map(|w| w.half_width).fold(f32::MAX, f32::min).max(1.0);
    let len = (p1 - p0).length();
    let n = ((len / (thin * 0.5)).ceil() as usize).clamp(1, 4096);
    (0..=n).map(|i| p0 + (p1 - p0) * (i as f32 / n as f32)).find(|p| blocked_by_walls(walls, *p))
}

/// The height a hit's line is taken at above both ends' feet (cm): a body's chest.
pub const WALL_LINE_HEIGHT: f32 = 100.0;

/// GLACIER'S WALLS SHIELD WHO STANDS BEHIND THEM (the Elden Ring hosts' hit intake, where a game projectile cannot be
/// stopped in flight): a hit on the party member at `victim` (feet, KH3 space) is taken by the ice when the line from its
/// attacker (`attacker`, feet) passes through a wall; with the attacker unknown (an Elden Ring bullet names nobody), when
/// every hostile in `hostiles` (feet) has its line through a wall (none has a clear shot). Never a redirection: the hit is
/// simply not taken.
pub fn wall_shields(walls: &[Wall], victim: Vec3, attacker: Option<Vec3>, hostiles: &[Vec3]) -> bool {
    if walls.is_empty() {
        return false;
    }
    let up = Vec3::Z * WALL_LINE_HEIGHT;
    let through = |from: Vec3| wall_crossing(walls, from + up, victim + up).is_some();
    match attacker {
        Some(a) => through(a),
        None => !hostiles.is_empty() && hostiles.iter().all(|h| through(*h)),
    }
}

// ================================================================================================ Burn ticks

/// The step's Burn ticks (`party.don.status_events`): a magic hit of `burn.tick_power` x the burner's Magic each, no
/// reaction, no reaction rule (4.2 Fire). The burner is the party's first Donald (the book does not keep who burned).
pub fn burn_ticks(cx: &mut Ctx) {
    let ticks: Vec<(u32, f32)> = cx.party.don.status_events.iter().filter_map(|(id, e)| if let StatusEvent::BurnTick(p) = e { Some((*id, *p)) } else { None }).collect();
    if ticks.is_empty() {
        return;
    }
    let Some(burner) = cx.party.members().find(|id| cx.party.status(*id).is_some_and(|s| s.donald.is_some())) else { return };
    let m = magic();
    for (id, power) in ticks {
        let Some(t) = cx.host.targets.info(TargetId(id)).filter(|i| i.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE)) else { continue };
        let Some(p) = cx.party.member(burner) else { return };
        let defender = Defender::of_target(cx.host.targets.defense(t.id).as_ref());
        let dmg = compute_damage(&p.status.as_attacker(), &defender, &DamageAttack::magic(power, Attr::Fire), &DamageContext::new(p.status.game_level, 1)).amount.max(m.burn_min_damage);
        let mult = cx.party.don_damage_taken(id);
        let mut hit = HitRecord::empty(t.id);
        hit.attack_name = "Burn";
        hit.position = t.center;
        hit.direction = Vec3::X;
        hit.damage = dmg * mult;
        hit.damage_kind = Kind::Small;
        hit.attribute = Attr::Fire;
        hit.flags |= DON_HIT_FLAG;
        let mut info = HitInfo { power, ..HitInfo::DEFAULT };
        info.flags50 |= HitInfo::FLAG50_MAGIC;
        info.victim_result = ReactionResult::NoReaction;
        info.clear_bad_status();
        hit.info = info;
        let res = cx.host.targets.apply_hit(&hit);
        if res.accepted {
            push_log(cx.log, FxLog::Hit { spell: SpellRef::Line1(Element::Fire, 1), target: t.id, damage: res.damage_dealt, reactions: Vec::new(), kind: Kind::Small });
            if res.killed {
                cx.party.don_forget(id);
            }
        }
    }
}

// ================================================================================================ the statuses' visuals

/// Keeps one effect per status on each enemy while it lasts (Wet, Chill, Frozen, Burn, Reveal, Blind).
pub fn sync_status_fx(fxw: &mut MagicFx, party: &mut Party, host: &mut PartyHost, dt: f32, step_log: &[FxLog]) {
    // (DON-SPELLB status looks, the user 2026-10-23) one look per status SLOT on each enemy; a slot's variant (Chill's stacks,
    // Reveal -> Exposed) swaps the look; each sized to the enemy's capsule; a Wet enemy splashes when it moves or is hit;
    // the Frozen breaks with a shatter burst when it thaws (the Shatter reaction spawns its own at the hit)
    let Some(owner) = party.members().find(|id| party.status(*id).is_some_and(|s| s.donald.is_some())) else {
        for s in fxw.status_fx.drain(..) {
            host.effects.stop(s.handle, false);
        }
        return;
    };
    let mut want: Vec<(u32, u8)> = Vec::new();
    for (id, s) in party.don.status.entries.iter() {
        let on = [
            s.is_wet().then_some(0),
            s.is_chilled().then(|| s.chill.clamp(1, 4) - 1),
            s.is_frozen().then_some(0),
            s.is_burned().then_some(0),
            s.is_revealed().then_some(u8::from(s.exposed)),
            s.is_blind().then_some(0),
            (s.big_slow > 0.0).then_some(0),
        ];
        for (slot, v) in on.iter().enumerate() {
            if let Some(v) = v {
                want.push((*id, slot as u8 * 8 + v));
            }
        }
    }
    let thawed = |id: u32| party.don.status_events.iter().any(|(t, e)| *t == id && matches!(e, StatusEvent::Thawed));
    // stop what ended (a short fade: the looks' particles are short-lived); a thawed Frozen breaks
    let mut keep = Vec::new();
    let mut breaks: Vec<(u32, f32)> = Vec::new();
    for s in fxw.status_fx.drain(..) {
        let info = host.targets.info(TargetId(s.target));
        if want.contains(&(s.target, s.which)) && info.is_some() {
            keep.push(s);
        } else {
            // a Frozen's ice goes at once (it breaks: the burst below when it thawed, the Shatter reaction's own at the hit);
            // any other look fades
            host.effects.stop(s.handle, s.which / 8 == 2);
            if s.which / 8 == 2 && thawed(s.target) {
                if let Some(t) = info {
                    breaks.push((s.target, status_scale(&t).0));
                }
            }
        }
    }
    fxw.status_fx = keep;
    let Some(m) = party.member_mut(owner) else { return };
    let spawn1 = |m: &mut crate::pawn::NpcPawn, host: &mut PartyHost, name: &'static str, id: u32, scale: f32| {
        let at = Attach::Target { target: TargetId(id), offset: Vec3::ZERO };
        let h = m.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at, rotation: Vec3::ZERO, scale, persistent: false });
        if let Some(se) = fx::extra_se(name) {
            m.seam.play(host.audio, &kh_core::host::SoundPlay { id: kh_core::host::SoundId(kh_core::notify::notify_asset_id(se)), name: se, at, volume: 1.0, pitch: 1.0, looping: false, spatial: true });
        }
        h
    };
    for (id, scale) in breaks {
        spawn1(m, host, fx::FROZEN_OUT, id, scale);
    }
    // start what began
    for (id, which) in want {
        if fxw.status_fx.iter().any(|s| s.target == id && s.which == which) {
            continue;
        }
        let Some(t) = host.targets.info(TargetId(id)) else { continue };
        let name = status_look(which);
        let at = Attach::Target { target: TargetId(id), offset: Vec3::ZERO };
        // authored round a 180 cm, 80 cm wide body: scaled by the enemy's height, widened / narrowed by its radius
        let (scale, wide) = status_scale(&t);
        if name == fx::FROZEN {
            spawn1(m, host, fx::FROZEN_IN, id, scale);
        }
        let h = m.seam.spawn(host.effects, &EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at, rotation: Vec3::ZERO, scale, persistent: true });
        if (wide / scale - 1.0).abs() > 0.02 {
            host.effects.transform(h, Vec3::ZERO, Vec3::ZERO, Vec3::new(wide, wide, scale));
        }
        if let Some(se) = fx::extra_se(name) {
            m.seam.play(host.audio, &kh_core::host::SoundPlay { id: kh_core::host::SoundId(kh_core::notify::notify_asset_id(se)), name: se, at, volume: 1.0, pitch: 1.0, looping: false, spatial: true });
        }
        if let Some(tint) = fx::tint_of(None, name) {
            host.effects.tint(h, tint);
        }
        fxw.status_fx.push(StatusFx { target: id, which, handle: h, clock: 0.0 });
    }
    // a Wet enemy moving or hit: a splash off it (at most every 0.3 s)
    for s in fxw.status_fx.iter_mut().filter(|s| s.which / 8 == 0) {
        s.clock = (s.clock - dt).max(0.0);
        if s.clock > 0.0 {
            continue;
        }
        let Some(t) = host.targets.info(TargetId(s.target)) else { continue };
        let moving = Vec3::new(t.velocity.x, t.velocity.y, 0.0).length() > 140.0 || t.velocity.z.abs() > 250.0;
        let hit = step_log.iter().any(|e| matches!(e, FxLog::Hit { target, .. } if target.0 == s.target));
        if moving || hit {
            spawn1(m, host, fx::WET_SPLASH, s.target, status_scale(&t).0);
            s.clock = 0.3;
        }
    }
}

/// (DON-SPELLB status looks) A status slot x 8 + variant -> its look.
pub fn status_look(which: u8) -> &'static str {
    let v = (which % 8) as usize;
    match which / 8 {
        0 => fx::WET,
        1 => fx::CHILL_STACKS[v.min(3)],
        2 => fx::FROZEN,
        3 => fx::BURN,
        4 if v == 1 => fx::EXPOSED,
        4 => fx::REVEAL,
        5 => fx::BLIND,
        _ => fx::SLOW,
    }
}

/// (DON-SPELLB status looks) The looks' scale on an enemy: (height scale, width scale). Authored round a 180 cm tall body of
/// 40 cm radius; the width follows the radius within 0.6x .. 2.2x of the height scale (a squat or a tall thin enemy).
pub fn status_scale(t: &TargetInfo) -> (f32, f32) {
    let s = (t.half_height / 90.0).clamp(0.5, 4.0);
    let w = if t.radius > 1.0 { (t.radius / 40.0).clamp(s * 0.6, s * 2.2) } else { s };
    (s, w)
}

// ================================================================================================ line 1: Sora's actors

/// Sora's main attack row (name, power) of a line-1 tier: the scale of a Donald cast's hits is row power / this.
pub fn sora_row(e: Element, tier: u8) -> Option<(&'static str, f32)> {
    let names: [&str; 4] = match e {
        Element::Fire => ["MAG_FIRE", "MAG_FIRA", "MAG_FIGA", "MAG_FIGAN"],
        Element::Blizzard => ["MAG_BLIZZARD", "MAG_BLIZZARA", "MAG_BLIZZAGA", "MAG_BLIZZAGAN"],
        Element::Thunder => ["MAG_THUNDER", "MAG_THUNDARA", "MAG_THUNDAGA", "MAG_THUNDAGAN"],
        Element::Water => ["MAG_WATER", "MAG_WATARA", "MAG_WATAGA", "MAG_WATAGAN"],
        Element::Aero => ["MAG_AERO", "MAG_AERORA", "MAG_AEROGA", "MAG_AEROGAN"],
        Element::Light => return None,
    };
    let name = names[(tier.clamp(1, 4) - 1) as usize];
    kh_core::magic::spells::attack_row(name).map(|r| (name, r.power))
}

/// Sora's command of a line-1 tier (tiers 1-3: 29 Fire ..; the -za tier: the Grand Magic "-gan" `S_*` id).
pub fn sora_cmd(e: Element, tier: u8) -> u8 {
    use kh_core::magic::spells::Element as S;
    if tier >= 4 {
        return match e {
            Element::Fire => 112,
            Element::Blizzard => 115,
            Element::Thunder => 118,
            Element::Water => 121,
            Element::Aero => 124,
            Element::Light => 125,
        };
    }
    let s: S = e.to_sora();
    s.command(tier)
}

/// The hits of one Donald's line-1 actors (Sora's) through the reaction rule: see the module text.
pub struct L1Targets<'a> {
    pub inner: &'a mut dyn Targets,
    pub book: &'a mut StatusBook,
    pub full: FullFlags,
    pub abilities: super::abilities::DonAbilities,
    /// The cast that shot (its tier when a hit's own row name says none).
    pub spell: SpellRef,
    /// (target, reaction) for the caster's meters, and the outcomes for the chains.
    pub rewards: Vec<(u32, Reaction)>,
    pub outcomes: Vec<(TargetInfo, HitOutcome, f32)>,
    pub log: Vec<FxLog>,
}

/// The element and tier of a hit of Sora's actors by its attack row ("MAG_FIRA_FIN" -> Fire 2, "MAG_B_BLIZZAGAN" ->
/// Blizzard 4, "MAG_WATER_BARRIER" -> Water 1); None for a row that is no line-1 spell.
pub fn tier_of_attack(name: &str) -> Option<(Element, u8)> {
    let n = name.strip_prefix("MAG_")?;
    let n = n.strip_prefix("B_").or_else(|| n.strip_prefix("C_")).or_else(|| n.strip_prefix("D_")).unwrap_or(n);
    let word = n.split('_').next()?;
    Some(match word {
        "FIRE" => (Element::Fire, 1),
        "FIRA" => (Element::Fire, 2),
        "FIGA" => (Element::Fire, 3),
        "FIGAN" => (Element::Fire, 4),
        "BLIZZARD" => (Element::Blizzard, 1),
        "BLIZZARA" => (Element::Blizzard, 2),
        "BLIZZAGA" => (Element::Blizzard, 3),
        "BLIZZAGAN" => (Element::Blizzard, 4),
        "THUNDER" => (Element::Thunder, 1),
        "THUNDARA" => (Element::Thunder, 2),
        "THUNDAGA" => (Element::Thunder, 3),
        "THUNDAGAN" => (Element::Thunder, 4),
        "WATER" | "WATERFIRE" | "WATERBLIZZARD" | "WATERTHUNDER" | "WATERAERO" => (Element::Water, 1),
        "WATARA" => (Element::Water, 2),
        "WATAGA" => (Element::Water, 3),
        "WATAGAN" => (Element::Water, 4),
        "AERO" => (Element::Aero, 1),
        "AERORA" => (Element::Aero, 2),
        "AEROGA" => (Element::Aero, 3),
        "AEROGAN" => (Element::Aero, 4),
        "CURE" => (Element::Light, 1),
        "CURA" => (Element::Light, 2),
        "CURAGA" => (Element::Light, 3),
        _ => return None,
    })
}

/// The damage scale of a Donald line-1 hit: his row's power / Sora's row's power for that tier (x the Boost).
pub fn l1_scale(e: Element, tier: u8, d: &DonData) -> f32 {
    let Some(row) = d.row(&SpellRef::Line1(e, tier).id()) else { return 1.0 };
    match sora_row(e, tier) {
        Some((_, p)) if p > 0.0 => (row.power / p).max(magic().l1_za_scale_floor),
        _ => 1.0,
    }
}

impl<'a> L1Targets<'a> {
    pub fn new(inner: &'a mut dyn Targets, book: &'a mut StatusBook, full: FullFlags, abilities: super::abilities::DonAbilities, spell: SpellRef) -> L1Targets<'a> {
        L1Targets { inner, book, full, abilities, spell, rewards: Vec::new(), outcomes: Vec::new(), log: Vec::new() }
    }

    pub fn into_parts(self) -> (Vec<(u32, Reaction)>, Vec<(TargetInfo, HitOutcome, f32)>, Vec<FxLog>) {
        (self.rewards, self.outcomes, self.log)
    }
}

impl Targets for L1Targets<'_> {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        self.inner.enumerate(out)
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.inner.info(id)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        self.inner.hurt_shapes(id, out)
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        if !hit.takes_hp() || hit.flags & DON_HIT_FLAG != 0 {
            return self.inner.apply_hit(hit);
        }
        let Some(info) = self.inner.info(hit.target) else { return self.inner.apply_hit(hit) };
        let size = size_of(&*self.inner, &info);
        let d = data();
        // the hit's own spell: its row name (an earlier cast's actor may still be out), else the cast's
        let (e, tier) = tier_of_attack(hit.attack_name).or_else(|| match self.spell {
            SpellRef::Line1(e, t) => Some((e, t)),
            _ => None,
        }).unwrap_or((Element::Fire, 1));
        let spell = SpellRef::Line1(e, tier);
        let applies = d.row(&spell.id()).map_or_else(Vec::new, |r| Apply::parse_list(r.applies));
        let scale = l1_scale(e, tier, &d) * boost_of(spell, &self.abilities);
        let out = resolve(self.book.entry(hit.target.0, size), HitTag::of(ElemSet::one(e)), &applies, self.full, &d.tune, self.abilities.mods().reaction_plus);
        let mut h = *hit;
        h.damage *= scale * out.damage_mult;
        h.flags |= DON_HIT_FLAG;
        if out.events.iter().any(|ev| matches!(ev, StatusEvent::Launch)) {
            h.damage_kind = Kind::Parabola;
        }
        if let Some(s) = out.events.iter().find_map(|ev| if let StatusEvent::Froze(s) = ev { Some(*s) } else { None }) {
            h.info.bad_status = "FREEZE";
            h.info.bad_time = s;
        }
        let res = self.inner.apply_hit(&h);
        for r in &out.reactions {
            self.rewards.push((hit.target.0, *r));
        }
        if res.accepted {
            push_log(&mut self.log, FxLog::Hit { spell, target: hit.target, damage: res.damage_dealt, reactions: out.reactions.clone(), kind: h.damage_kind });
        }
        if out.conduct_radius.is_some() || out.wildfire_radius.is_some() {
            self.outcomes.push((info, out, h.damage));
        }
        if res.killed {
            self.book.forget(hit.target.0);
        }
        res
    }
    fn defense(&self, id: TargetId) -> Option<TargetDefense> {
        self.inner.defense(id)
    }
    fn allies(&self, out: &mut Vec<kh_core::host::targets::Ally>) {
        self.inner.allies(out)
    }
    fn heal_ally(&mut self, id: TargetId, amount: i32, effect: u8) {
        self.inner.heal_ally(id, amount, effect)
    }
    fn lock_on_range(&self, id: TargetId) -> f32 {
        self.inner.lock_on_range(id)
    }
    fn engaged_count(&self) -> u32 {
        self.inner.engaged_count()
    }
    fn body(&self, id: TargetId, part: u32) -> TargetBody {
        self.inner.body(id, part)
    }
    fn take_armor(&mut self, id: TargetId, power: i32, weak: bool) -> i32 {
        self.inner.take_armor(id, power, weak)
    }
    fn take_reaction_power(&mut self, id: TargetId, power: i32) -> i32 {
        self.inner.take_reaction_power(id, power)
    }
    fn rush_suppression(&self, id: TargetId) -> bool {
        self.inner.rush_suppression(id)
    }
    fn terrain_dz(&self, id: TargetId) -> f32 {
        self.inner.terrain_dz(id)
    }
    fn push_body(&self, id: TargetId) -> Option<PushBody> {
        self.inner.push_body(id)
    }
    fn reaction_state(&self, id: TargetId) -> u8 {
        self.inner.reaction_state(id)
    }
    fn attack_shapes(&self, out: &mut Vec<AttackShape>) {
        self.inner.attack_shapes(out)
    }
    fn touched(&mut self, hit: &HitRecord) {
        self.inner.touched(hit)
    }
    fn finish_kind(&self, id: TargetId) -> FinishKind {
        self.inner.finish_kind(id)
    }
    fn foe_level(&self, id: TargetId) -> Option<u8> {
        self.inner.foe_level(id)
    }
    fn damage_to_hp(&self) -> f32 {
        self.inner.damage_to_hp()
    }
    fn in_revenge(&self, id: TargetId) -> bool {
        self.inner.in_revenge(id)
    }
    fn end_revenge(&mut self, id: TargetId) {
        self.inner.end_revenge(id)
    }
    fn friends(&self, out: &mut Vec<Friend>) {
        self.inner.friends(out)
    }
    fn friend_link(&mut self, req: FriendLinkRequest) {
        self.inner.friend_link(req)
    }
    fn size_class(&self, id: TargetId) -> u8 {
        self.inner.size_class(id)
    }
    fn script_hold(&mut self, id: TargetId, hold: Option<kh_core::host::ScriptHold>) {
        self.inner.script_hold(id, hold) // (ZETTA) Zettaflare's held target
    }
    fn held_animated(&self, id: TargetId) -> bool {
        self.inner.held_animated(id) // (ZETTA)
    }
    fn script_finale(&mut self, id: TargetId, finale: Option<kh_core::host::ScriptFinale>) {
        self.inner.script_finale(id, finale) // (ZETTA)
    }
    fn drawn_extent(&self, id: TargetId) -> Option<(f32, f32)> {
        self.inner.drawn_extent(id) // (ZETTA 2026-10-23) what the host draws of it (the framing and the beam's cover)
    }
}

// ================================================================================================ the party-wide wrapper

/// EVERYONE'S HITS on a Revealed + Exposed enemy deal x`exposed.mult` (4.5): the wrapper of `host.targets` for the
/// party's own step (Goofy's swings, the friends' KH3 actions, the team attacks): a hit without [`DON_HIT_FLAG`] is
/// multiplied by the target's [`Party::don_damage_taken`] (a snapshot taken at the step's start) and remembered, so
/// that the reaction rule sees it after the step (`after_party_hits`: a non-Donald hit turns a Reveal into Exposed, a
/// friend's elemental hit reacts). Sora's own hits are the host's (kh-er targets.rs, kh-app).
pub struct PartyFx<'a> {
    pub inner: &'a mut dyn Targets,
    pub mults: &'a [(u32, f32)],
    pub seen: Vec<(u32, Attr)>,
    /// DON-KIT's x on Donald's team-attack rows at Resonance 3 (`team::link::don_team_hit_mult`).
    pub team: f32,
}

impl Targets for PartyFx<'_> {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        self.inner.enumerate(out)
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.inner.info(id)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        self.inner.hurt_shapes(id, out)
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        // (ZETTA round 5c) the allies never hit Zettaflare's target or an enemy near its zones until the player has Donald back
        if crate::donald::zettaflare::hands_off_refuses(hit, &*self.inner) {
            return HitResult::default();
        }
        if hit.flags & DON_HIT_FLAG != 0 || !hit.takes_hp() {
            return self.inner.apply_hit(hit);
        }
        let team = if crate::team::link::is_donald_team_row(hit.attack_name) { self.team } else { 1.0 };
        let mult = self.mults.iter().find(|m| m.0 == hit.target.0).map_or(1.0, |m| m.1) * team;
        self.seen.push((hit.target.0, hit.attribute));
        if mult == 1.0 {
            return self.inner.apply_hit(hit);
        }
        let mut h = *hit;
        h.damage *= mult;
        self.inner.apply_hit(&h)
    }
    fn defense(&self, id: TargetId) -> Option<TargetDefense> {
        self.inner.defense(id)
    }
    fn allies(&self, out: &mut Vec<kh_core::host::targets::Ally>) {
        self.inner.allies(out)
    }
    fn heal_ally(&mut self, id: TargetId, amount: i32, effect: u8) {
        self.inner.heal_ally(id, amount, effect)
    }
    fn lock_on_range(&self, id: TargetId) -> f32 {
        self.inner.lock_on_range(id)
    }
    fn engaged_count(&self) -> u32 {
        self.inner.engaged_count()
    }
    fn body(&self, id: TargetId, part: u32) -> TargetBody {
        self.inner.body(id, part)
    }
    fn take_armor(&mut self, id: TargetId, power: i32, weak: bool) -> i32 {
        self.inner.take_armor(id, power, weak)
    }
    fn take_reaction_power(&mut self, id: TargetId, power: i32) -> i32 {
        self.inner.take_reaction_power(id, power)
    }
    fn rush_suppression(&self, id: TargetId) -> bool {
        self.inner.rush_suppression(id)
    }
    fn terrain_dz(&self, id: TargetId) -> f32 {
        self.inner.terrain_dz(id)
    }
    fn push_body(&self, id: TargetId) -> Option<PushBody> {
        self.inner.push_body(id)
    }
    fn reaction_state(&self, id: TargetId) -> u8 {
        self.inner.reaction_state(id)
    }
    fn attack_shapes(&self, out: &mut Vec<AttackShape>) {
        self.inner.attack_shapes(out)
    }
    fn touched(&mut self, hit: &HitRecord) {
        self.inner.touched(hit)
    }
    fn finish_kind(&self, id: TargetId) -> FinishKind {
        self.inner.finish_kind(id)
    }
    fn foe_level(&self, id: TargetId) -> Option<u8> {
        self.inner.foe_level(id)
    }
    fn damage_to_hp(&self) -> f32 {
        self.inner.damage_to_hp()
    }
    fn in_revenge(&self, id: TargetId) -> bool {
        self.inner.in_revenge(id)
    }
    fn end_revenge(&mut self, id: TargetId) {
        self.inner.end_revenge(id)
    }
    fn friends(&self, out: &mut Vec<Friend>) {
        self.inner.friends(out)
    }
    fn friend_link(&mut self, req: FriendLinkRequest) {
        self.inner.friend_link(req)
    }
    fn size_class(&self, id: TargetId) -> u8 {
        self.inner.size_class(id)
    }
    fn script_hold(&mut self, id: TargetId, hold: Option<kh_core::host::ScriptHold>) {
        self.inner.script_hold(id, hold) // (ZETTA) Zettaflare's held target
    }
    fn held_animated(&self, id: TargetId) -> bool {
        self.inner.held_animated(id) // (ZETTA)
    }
    fn script_finale(&mut self, id: TargetId, finale: Option<kh_core::host::ScriptFinale>) {
        self.inner.script_finale(id, finale) // (ZETTA)
    }
    fn drawn_extent(&self, id: TargetId) -> Option<(f32, f32)> {
        self.inner.drawn_extent(id) // (ZETTA 2026-10-23) what the host draws of it (the framing and the beam's cover)
    }
}

impl Party {
    /// (DON-MAGIC) The Exposed multipliers of the enemies now (the snapshot [`PartyFx`] reads).
    pub fn don_exposed(&self) -> Vec<(u32, f32)> {
        let t = data().tune;
        self.don.status.entries.iter().map(|(id, s)| (*id, s.damage_taken_mult(&t))).filter(|m| m.1 != 1.0).collect()
    }

    /// (DON-MAGIC) The reaction rule for the hits that were NOT Donald's spells (a friend's swing, Sora's spell the host
    /// reports): the first such hit turns a Reveal into Exposed, an elemental one reacts (Sora's Thunder conducts ...).
    /// Returns the damage multiplier the reaction asks (the host applies it to that hit when it can).
    pub fn don_other_hit(&mut self, target: u32, size: Size, attr: Attr) -> f32 {
        let tag = HitTag::of(element_of_attr(attr).map_or(ElemSet::NONE, ElemSet::one));
        // only targets that carry a status: a hit on a clean enemy changes nothing
        if self.don.status.get(target).is_none() {
            return 1.0;
        }
        let out = self.don_hit(None, target, size, tag, &[]);
        out.damage_mult
    }
}

/// The step's non-Donald hits through the reaction rule (statuses only: the damage already went out with Exposed).
pub fn after_party_hits(party: &mut Party, host: &PartyHost, seen: &[(u32, Attr)]) {
    for (id, attr) in seen {
        if party.don.status.get(*id).is_none() {
            continue;
        }
        let size = host.targets.info(TargetId(*id)).map_or(Size::Small, |i| size_of(host.targets, &i));
        let tag = HitTag::of(element_of_attr(*attr).map_or(ElemSet::NONE, ElemSet::one));
        let _ = party.don_hit(None, *id, size, tag, &[]);
    }
}
