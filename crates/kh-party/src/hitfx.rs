//! (AUD) A FRIEND'S MELEE HIT ON CONTACT: its hit sound and its hit effect, as Sora's own hit does (kh-core
//! combat/hit.rs step 20: `HitEffects` 0x1406294F0, chooser 0x140628960). Found by the Donald / Goofy audit
//! (build/sora_tmp/party/AUD_CHECKLIST.md): every friend hit delivered by `actions::deliver` (Donald's Beat / Swing /
//! Jump Strike, the AI friends' clip attacks) landed with NO sound and NO effect in both hosts; the clips' attack
//! windows carry the sound (`TresAnimNotifyState_Attack` `m_HitSE`: "HIT1" .. of the weapon's hit set, the data of
//! `build/gen/rust/party_anim_n_ex00x.rs`) and every friend weapon has its own hit effect in the party pack
//! (`vw_do010_hit0` .. `vw_gu140_hit00`, party-effects.json "wNNN effect folder").
//!
//! THE RULE (Sora's, with the friend's pieces): on a hit the host took, the window's sound (its asset, else the
//! alias, resolved in the member's EQUIPPED weapon's hit set by the audio host) and the effect: a physical row the
//! weapon's own hit effect, an element row the element's (Sora's table: vs_fir_hit0 ...); a guarded hit the common
//! guard spark (vs_com_haj00), no sound. Always on (the
//! user: every sound and effect working).

use crate::pawn::NpcPawn;
use glam::Vec3;
use kh_core::host::{Attach, EffectId, EffectSpawn, HitResult, SoundId, SoundPlay};

/// Every friend weapon's hit effect (the party pack's vfx/effects, one per weapon model).
const WEAPON_HIT: [(&str, &str); 28] = [
    ("w_do010", "vw_do010_hit0"),
    ("w_do020", "vw_do020_hit0"),
    ("w_do030", "vw_do030_hit00"),
    ("w_do040", "vw_do040_hit00"),
    ("w_do050", "vw_do050_hit0"),
    ("w_do060", "vw_do060_hit00"),
    ("w_do070", "vw_do070_hit0"),
    ("w_do080", "vw_do080_hit00"),
    ("w_do090", "vw_do090_hit0"),
    ("w_do100", "vw_do100_hit00"),
    ("w_do110", "vw_do110_hit00"),
    ("w_do120", "vw_do120_hit00"),
    ("w_do130", "vw_do130_hit0"),
    ("w_do140", "vw_do140_hit00"),
    ("w_gu010", "vw_gu010_hit0"),
    ("w_gu020", "vw_gu020_hit00"),
    ("w_gu030", "vw_gu030_hit0"),
    ("w_gu040", "vw_gu040_hit00"),
    ("w_gu050", "vw_gu050_hit00"),
    ("w_gu060", "vw_gu060_hit00"),
    ("w_gu070", "vw_gu070_hit0"),
    ("w_gu080", "vw_gu080_hit00"),
    ("w_gu090", "vw_gu090_hit0"),
    ("w_gu100", "vw_gu100_hit0"),
    ("w_gu110", "vw_gu110_hit0"),
    ("w_gu120", "vw_gu120_hit0"),
    ("w_gu130", "vw_gu130_hit0"),
    ("w_gu140", "vw_gu140_hit00"),
];

/// The hit effect of a weapon model ("w_do010" -> "vw_do010_hit0").
pub fn weapon_hit_effect(model: &str) -> Option<&'static str> {
    WEAPON_HIT.iter().find(|(m, _)| *m == model).map(|w| w.1)
}

/// The effect of a hit by attribute (Sora's chooser; physical = the member's weapon's).
pub fn effect_of(attribute: &str, weapon_model: &str) -> &'static str {
    match attribute {
        "TRES_DMG_ATTR_FIRE" => "vs_fir_hit0",
        "TRES_DMG_ATTR_BLIZZARD" => "vs_blz_hit0",
        "TRES_DMG_ATTR_THUNDER" => "vs_thd_hit0",
        "TRES_DMG_ATTR_WATER" => "vs_wat_hit0",
        "TRES_DMG_ATTR_AERO" => "vs_win_hit0",
        "TRES_DMG_ATTR_DARK" => "vs_dak_hit1",
        _ => weapon_hit_effect(weapon_model).unwrap_or("vw_so010_hit0"),
    }
}

/// The sound of the running clip's attack window for `row` (`m_HitSE` / `m_HitSEAsset`), when A's full tables exist.
pub fn window_hit_se(p: &NpcPawn, row: &str) -> Option<&'static str> {
    crate::notify_seam::attack_hit_se(p.chara, p.anim.clip.sequence, row)
}

/// On a melee hit `deliver` handed to the host: the sound and the effect at the hit point `at`.
pub fn on_contact(p: &mut NpcPawn, row: &crate::tables::AttackRow, at: Vec3, res: &HitResult, host: &mut crate::host::PartyHost) {
    if !res.accepted {
        return;
    }
    let rotation = Vec3::new(0.0, p.yaw, 0.0);
    let effect = |name: &'static str| EffectSpawn { id: EffectId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), rotation, scale: 1.0, persistent: false };
    if res.guarded {
        p.seam.spawn(host.effects, &effect(kh_core::combat::hit::EFFECT_GUARD_REFLECT));
        return;
    }
    let model = crate::tables::weapon(p.status.weapon).map_or("", |w| w.7);
    p.seam.spawn(host.effects, &effect(effect_of(row.attribute, model)));
    // the window's own sound, else the weapon set's first hit alias (Sora's rule: HIT1 when the notify names none)
    let name = window_hit_se(p, row.name).unwrap_or("HIT1");
    // Attach::World at the impact, on the member's handle range: the audio host resolves it in HIS weapon's hit set
    p.seam.play(host.audio, &SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_friend_weapon_has_its_hit_effect_and_the_elements_theirs() {
        for w in crate::tables::PARTY_WEAPONS {
            assert!(weapon_hit_effect(w.7).is_some(), "weapon {} ({}) has no hit effect", w.1, w.7);
        }
        assert_eq!(effect_of("TRES_DMG_ATTR_PHYSICAL", "w_do010"), "vw_do010_hit0");
        assert_eq!(effect_of("TRES_DMG_ATTR_FIRE", "w_do010"), "vs_fir_hit0");
        assert_eq!(effect_of("TRES_DMG_ATTR_PHYSICAL", "w_gu030"), "vw_gu030_hit0");
    }

    #[cfg(kh_party_anim_full)]
    #[test]
    fn donalds_beat_window_names_its_hit_sound() {
        // Attack1_Beat (M_1301): TresAnimNotifyState_Attack m_HitSE "HIT1"
        let seq = crate::tables::clip(crate::ids::Chara::Donald, "M_1301").sequence;
        assert_eq!(crate::notify_seam::attack_hit_se(crate::ids::Chara::Donald, seq, "Attack1_Beat"), Some("HIT1"));
    }
}
