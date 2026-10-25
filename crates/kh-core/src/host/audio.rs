//! The audio seam: the core says WHICH sound, WHERE and with what parameters; the host owns
//! decoding, mixing and attenuation.

use super::Attach;

/// Which sound: the row index of the generated sound table (the sound sheet of
/// docs/sora-audio-spec.md through `tools/gen_port.js`, once it exists). Until then the only ids
/// in use are the test level's debug ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SoundId(pub u32);

/// One playing instance. Allocated by the core, counting up from 1; 0 is never used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SoundHandle(pub u32);

/// A request to play a sound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundPlay {
    pub id: SoundId,
    /// The sound's name in its sheet (for logs, overlays and hosts that key their bank by name);
    /// may be empty.
    pub name: &'static str,
    /// Where it is heard from. `Attach::World` and `Attach::Target` are 3D; a host may treat
    /// `Attach::Sora` as 3D at his position.
    pub at: Attach,
    /// Linear gain, 1 = as authored.
    pub volume: f32,
    /// Playback rate, 1 = as authored.
    pub pitch: f32,
    /// Loops until stopped through its handle.
    pub looping: bool,
    /// False: play it flat (UI and menu sounds), whatever `at` says.
    pub spatial: bool,
}

impl SoundPlay {
    /// A one-shot at Sora's origin with neutral parameters.
    pub const fn at_sora(id: SoundId, name: &'static str) -> SoundPlay {
        SoundPlay { id, name, at: Attach::SORA, volume: 1.0, pitch: 1.0, looping: false, spatial: true }
    }
}

/// The host's audio output. Calls arrive during `Sora::step_hosted`, in order; nothing is
/// returned, so an implementation can do anything (or nothing) without touching the simulation.
pub trait Audio {
    /// Start `play`; `handle` names the instance for a later `stop`.
    fn play(&mut self, handle: SoundHandle, play: &SoundPlay);
    /// Stop an instance, fading over `fade_out` seconds (0 = at once). Unknown and finished
    /// handles are ignored.
    fn stop(&mut self, handle: SoundHandle, fade_out: f32);
    /// (audio implementer) THE PAWN'S VOICE GROUP NUMBER changed (`SetVoiceGroupNo`, the int at
    /// pawn+0x4D8; docs/sora-audio-spec.md 5.2): a voice notify that names a GROUP ("sora_magic",
    /// "damage_dead" ...) plays the unit of the group with this number, and nothing when the
    /// group has no such unit. Arrives in order with `play`; also `EventKind::VoiceGroupNo`.
    fn voice_group_no(&mut self, _number: i32) {}
    /// (party stream E) A PARTY MEMBER'S voice group number changed (the anim interface's
    /// +0x1F0, e.g. Donald's magic voice number before a cast clip): the voice-GROUP notifies of
    /// that member's clips play the unit with this number. In order with `play`.
    fn party_voice_group_no(&mut self, _member: super::party::PartyMember, _number: i32) {}
}

/// (audio implementer) The voice group number an attack sets after it started its animation
/// (sheet sora-sounds, rows `code.voice_group_no`: 0x1413378B0 and the attack state's per-kind
/// cases [C]): kind 0x25 -> 52, 0x4D -> 54, 0x4E -> 55, 0x5D / 0x67 -> 1, 0xB6 / 0xB7 -> 2; every
/// other site passes 0 (the default unit).
pub const fn attack_voice_group_no(kind: i32) -> i32 {
    match kind {
        0x25 => 52,
        0x4D => 54,
        0x4E => 55,
        0x5D | 0x67 => 1,
        0xB6 | 0xB7 => 2,
        _ => 0,
    }
}

/// (audio implementer) THE HIT SOUNDS OF A SPELL / SHOT ACTOR (`ATresProjectileBase`): the seam
/// NAME is the actor's blueprint class ("TresProj_MagFireNml_C") and the id says which of the
/// class's sounds: its `m_HitEffects[0].m_PawnHitSEAsset` or `m_MapHitSEAsset` (audio spec 5.4,
/// rows `code.hit_magic`). The host's audio side owns the table class -> sound.
pub const ACTOR_PAWN_HIT: SoundId = SoundId(0xAC70_0001);
pub const ACTOR_MAP_HIT: SoundId = SoundId(0xAC70_0002);

/// (audio implementer) The seam name of a collected prize's `TresPrizeBase.m_SoundGet` (rows
/// `code.prize`: "<prize class>.get"; ".bound" is the bounce, played by whoever simulates the
/// orb). Index = `status::PrizeKind` as u8.
pub const fn prize_get_sound(kind: u8) -> &'static str {
    match kind {
        0 => "PrizeHpS.get",
        1 => "PrizeHpL.get",
        2 => "PrizeMpS.get",
        3 => "PrizeMpL.get",
        4 => "PrizeMunnyS.get",
        5 => "PrizeMunnyM.get",
        6 => "PrizeMunnyL.get",
        7 => "PrizeFocusS.get",
        _ => "PrizeFocusL.get",
    }
}

/// (audio implementer) THE HEAL EFFECT ON THE PAWN, which carries the heal's sound (audio spec
/// 5.5): the entries of the pawn's `m_CmnBadStatesEffect` table (ead_com_abn0) by group id.
/// Cure / Cura / Curaga on himself: the spell's own effect group 1420 / 1422 / 1424 ->
/// `vm_cur_hed0..2` (se00002_302..304) [A for the groups; that the heal of tier n shows the
/// group of tier n is inf]. `tier` 0..2.
pub const fn cure_heal_effect(tier: usize) -> &'static str {
    match tier {
        0 => "vm_cur_hed0",
        1 => "vm_cur_hed1",
        _ => "vm_cur_hed2",
    }
}

/// (audio implementer) As [`cure_heal_effect`] for an item (`status::ItemKind` value).
/// (GOOFY-IT) NOW TRACED [C]: ItemEffect 0x1405BE820 hands the item's Recover kind ([`item_recover_kind`]) to Recover
/// (vt+0x1878 -> base 0x1405B57D0, the SAME for Sora (player override 0x141445F40 -> 0x14054C770 -> 0x1405B57D0) and the
/// friends), which calls 0x1405B8C30(pawn+0x440, 4, kind): its switch picks the group of the pawn's m_CmnBadStatesEffect
/// (ead_com_abn0) ([`recover_kind_effect`]). Before (inferred from asset names): Mega-Potion / Mega-Ether had no effect and
/// Megalixir played vs_itm_mgn0 (group 3155, the Mognet medal's burst, "モグネット出現・破裂"); the traced switch gives
/// Mega-Potion kind 4 -> 3130 vs_itm_hpo01, Mega-Ether 0x12 -> 3141 vs_itm_het0, Megalixir kind 6 -> 3150 vs_itm_elx0.
/// `None`: no effect for the item (Tent: kind 0).
pub const fn item_heal_effect(item: u32) -> Option<&'static str> {
    match item_recover_kind(item) {
        Some(kind) => match recover_kind_effect(kind) {
            Some((name, _, _)) => Some(name),
            None => None,
        },
        None => None,
    }
}

/// (GOOFY-IT) ItemEffect 0x1405BE820's Recover kind of an item (its local bVar12, passed to Recover when bShow) [C]:
/// 3 Potion, 4 Hi- / Mega-Potion, 5 Ether, 0x12 Hi- / Mega-Ether, 6 Elixir / Megalixir, 0x13 Refocuser, 0x14 Hi-Refocuser,
/// 0x15 Panacea. Tent (0x201) passes 0: no effect. (`status::ItemKind` value.)
pub const fn item_recover_kind(item: u32) -> Option<u8> {
    Some(match item {
        0x101 => 3,
        0x102 | 0x103 => 4,
        0x104 => 5,
        0x105 | 0x10B => 0x12,
        0x106 | 0x107 => 6,
        0x108 => 0x13,
        0x109 => 0x14,
        0x10A => 0x15,
        _ => return None,
    })
}

/// (GOOFY-IT) The item part of 0x1405B8C30(pawn+0x440, 4, kind)'s switch [C], with the group's row of ead_com_abn0 [A,
/// build/sora_tmp/vfx/fxinv/spec.md rows pawn.badstate.g31xx: every one "component owner @vfx_abn_center"]: (effect, socket,
/// group). Each effect carries its own SeAttach (/Game/Sound/SE/_Data/SeAttach/common/<name>): the sound rides the effect
/// seam. The same call also starts group 0xC12 (3090 vs_abn_cur0 at vfx_lit_mune, a looping glow with a 1.0 s timer at
/// +0x4D4) when it is not already running: NOT PORTED (for Sora either; its stop tick was not found).
pub const fn recover_kind_effect(kind: u8) -> Option<(&'static str, &'static str, u32)> {
    Some(match kind {
        3 => ("vs_itm_pot0", "vfx_abn_center", 0xC30),
        4 => ("vs_itm_hpo01", "vfx_abn_center", 0xC3A),
        5 => ("vs_itm_etr0", "vfx_abn_center", 0xC44),
        6 => ("vs_itm_elx0", "vfx_abn_center", 0xC4E),
        0x12 => ("vs_itm_het0", "vfx_abn_center", 0xC45),
        0x13 => ("vs_itm_fcr0", "vfx_abn_center", 0xC46),
        0x14 => ("vs_itm_hfr0", "vfx_abn_center", 0xC47),
        0x15 => ("vs_itm_acu0", "vfx_abn_center", 0xC48),
        _ => return None,
    })
}

/// (audio implementer) The voice group number of the death state: group "damage_dead" unit 1.
pub const DEATH_VOICE_GROUP_NO: i32 = 1;

/// Silence.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullAudio;

impl Audio for NullAudio {
    fn play(&mut self, _handle: SoundHandle, _play: &SoundPlay) {}
    fn stop(&mut self, _handle: SoundHandle, _fade_out: f32) {}
}
