//! (XSI) SPECTRAL RAYS (XS 2.8; ability SHOTLOCK_SPECTRAL_RAYS 263, Glintstone Dragon Smarag). Ever After's
//! shotlock (`TresShootFlowData` row 11 "ラプ反射" = `kh3_system.command.sf_laser_0` "Spectral Rays": up to 28
//! locks, sheet sora-magic-shotlocks `aim2.table.k11.*`; rows `SF_REFLECTIVELASER` 0.8 SMALL magic and
//! `SF_REFLECTIVELASER_RANGE` 0.5 TOSS magic) as a SECOND SHOTLOCK:
//! - the Customize screen's Shotlock choice (`CommandState::shotlock`: 0 = the Keyblade's own, King of Hearts with
//!   its Ragnarok upgrade at MAX; 11 = Spectral Rays), stored per character (Sora's save record; Donald and Goofy
//!   keep their kits' own); it takes effect while the ability is equipped;
//! - RB held as before: the aim mode locks with row 11's count and interval; no Ragnarok upgrade at MAX;
//! - the attack: (SHOT-A laser) KH3's own ShootFlow update of kind 11 (0x1413B9600, magic/shotlocks/laser.rs: Mirage
//!   Staff's `M_A481`, one SF_REFLECTIVELASER per lock at the locked targets 0.1 s apart, the SF_Range window), the
//!   same code as Ever After's MAX kind. XSI's own beam schedule (M_1483, refracted beams) is gone with it.
//! - the Elden Ring shotlock nerf applies unchanged (kh-er ermap.rs: rows `SF_*` are shotlock hits).

use super::has;
use crate::enums::ETresAbilityKind as A;
use crate::pawn::Pawn;

/// `ETresShootFlowKind` 11 (sheet row `spectral.kind`).
pub const KIND: u8 = 11;

/// The shotlock Sora's aim mode uses: Spectral Rays when it is chosen on the Customize screen AND equipped.
/// (SHOT-A laser) Not while the held Keyblade fires Spectral Rays itself (Ever After's MAX kind): the Customize screen
/// does not offer it then (`choices_for`), and the Keyblade's own rule (Shimmering Drops, Spectral Rays at MAX) applies.
pub(crate) fn chosen(s: &Pawn) -> bool {
    s.command.shotlock == KIND && has(s, A::ShotlockSpectralRays) && !s.status.current_keyblade().is_some_and(native)
}

/// (SHOT-A laser) The Keyblade's own shotlocks include Spectral Rays (KH3: Ever After's MAX kind 11).
pub fn native(id: crate::status::KeybladeId) -> bool {
    crate::magic::shotlocks::keyblade_kinds(id).contains(&KIND)
}

/// The kinds the Customize screen offers: the Keyblade's own (0) and, when the ability is owned, Spectral Rays.
pub fn choices(owned: bool) -> Vec<u8> {
    let mut out = vec![0];
    if owned {
        out.push(KIND);
    }
    out
}

/// (SHOT-A laser) `choices` for the held Keyblade: Spectral Rays is not offered a second time when the Keyblade's own
/// entry (0) already fires it (Ever After), so the menu never lists it twice.
pub fn choices_for(owned: bool, held: Option<crate::status::KeybladeId>) -> Vec<u8> {
    choices(owned && !held.is_some_and(native))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::KeybladeId;

    #[test]
    fn spectral_rays_is_never_offered_twice() {
        let id = |key: &str| KeybladeId::from_key(key);
        // Ever After fires it as its MAX kind: only the Keyblade's own entry.
        assert!(native(id("WEP_KEYBLADE_SO_04").unwrap()));
        assert_eq!(choices_for(true, id("WEP_KEYBLADE_SO_04")), vec![0]);
        // Every other Keyblade: the XS choice as before.
        assert_eq!(choices_for(true, id("WEP_KEYBLADE_SO_00")), vec![0, KIND]);
        assert_eq!(choices_for(true, None), vec![0, KIND]);
        assert_eq!(choices_for(false, id("WEP_KEYBLADE_SO_01")), vec![0]);
    }
}
