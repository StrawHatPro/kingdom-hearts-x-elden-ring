//! What a host needs to SHOW, SOUND and APPLY the orbs: the effect and sound of each prize class
//! (its blueprint under `/Game/Blueprints/Prize/`), and the calls that put a collected prize and
//! the orb sounds through the core's own seams.

use crate::params::P;
use crate::{BoxKind, PrizeClass, PrizeEvent, SpawnMods};
use kh_core::enums::ETresAbilityKind;
use kh_core::status::{ItemKind, PrizeKind};
use kh_core::{Attach, DebugEmit, SoundId, SoundPlay, Sora, StatusRequest};

/// The blueprint class of a prize ("PrizeHpS" ...): the audio routes are "<class>.get" and
/// "<class>.bound" (sheet sora-sounds, rows `code.prize`).
pub fn class_name(class: PrizeClass) -> &'static str {
    match class {
        PrizeClass::Orb(kind) => match kind {
            PrizeKind::HpSmall => "PrizeHpS",
            PrizeKind::HpLarge => "PrizeHpL",
            PrizeKind::MpSmall => "PrizeMpS",
            PrizeKind::MpLarge => "PrizeMpL",
            PrizeKind::Munny1 => "PrizeMunnyS",
            PrizeKind::Munny5 => "PrizeMunnyM",
            PrizeKind::Munny10 => "PrizeMunnyL",
            PrizeKind::FocusSmall => "PrizeFocusS",
            PrizeKind::FocusLarge => "PrizeFocusL",
        },
        PrizeClass::Box(kind, _) => match kind {
            BoxKind::Heal => "PrizeBoxHeal",
            BoxKind::Material => "PrizeBoxMate",
            BoxKind::Foodstuff => "PrizeBoxFstf",
            BoxKind::Rare => "PrizeBoxRare",
        },
    }
}

/// The seam name of its get sound (`m_SoundGet`) or bounce sound (`m_SoundBound`).
pub fn sound_name(class: PrizeClass, bound: bool) -> &'static str {
    macro_rules! pair {
        ($name:literal) => {
            if bound {
                concat!($name, ".bound")
            } else {
                concat!($name, ".get")
            }
        };
    }
    match class_name(class) {
        "PrizeHpS" => pair!("PrizeHpS"),
        "PrizeHpL" => pair!("PrizeHpL"),
        "PrizeMpS" => pair!("PrizeMpS"),
        "PrizeMpL" => pair!("PrizeMpL"),
        "PrizeMunnyS" => pair!("PrizeMunnyS"),
        "PrizeMunnyM" => pair!("PrizeMunnyM"),
        "PrizeMunnyL" => pair!("PrizeMunnyL"),
        "PrizeFocusS" => pair!("PrizeFocusS"),
        "PrizeFocusL" => pair!("PrizeFocusL"),
        "PrizeBoxHeal" => pair!("PrizeBoxHeal"),
        "PrizeBoxMate" => pair!("PrizeBoxMate"),
        "PrizeBoxFstf" => pair!("PrizeBoxFstf"),
        _ => pair!("PrizeBoxRare"),
    }
}

/// The particle system of its blueprint (the effect pack's name) and the component's scale.
pub fn effect(class: PrizeClass) -> (&'static str, f32) {
    match class {
        PrizeClass::Orb(kind) => {
            let name = match kind {
                PrizeKind::HpSmall | PrizeKind::HpLarge => "vs_plz_hpb0",
                PrizeKind::MpSmall | PrizeKind::MpLarge => "vs_plz_mpb0",
                PrizeKind::Munny1 | PrizeKind::Munny5 | PrizeKind::Munny10 => "VS_PLZ_MNY0",
                PrizeKind::FocusSmall | PrizeKind::FocusLarge => "vs_plz_fop0",
            };
            (name, P.kind_effect_scale[kind as usize])
        }
        PrizeClass::Box(kind, _) => (
            match kind {
                BoxKind::Heal => "vs_plz_prb00",
                BoxKind::Rare => "vs_plz_prb01",
                BoxKind::Material => "vs_plz_prb02",
                BoxKind::Foodstuff => "vs_plz_prb03",
            },
            1.0,
        ),
    }
}

/// The spawn modifiers of Sora alone (no party): his own counts stand for the party-wide ones.
pub fn mods_of(sora: &Sora) -> SpawnMods {
    let s = sora.status();
    SpawnMods {
        prize_fever: s.has(ETresAbilityKind::PrizeFeever),
        luck_up: s.count(ETresAbilityKind::LuckUp),
        converters: crate::Converters {
            hp: s.has(ETresAbilityKind::HpConverter),
            mp: s.has(ETresAbilityKind::MpConverter),
            focus: s.has(ETresAbilityKind::FpConverter),
            millionaire: s.has(ETresAbilityKind::Millionaire),
            munny: s.has(ETresAbilityKind::MunnyConverter),
        },
    }
}

/// True when the killing hit draws its prizes from any distance: a magic hit of a Sora with
/// MAGIC_DRAW (`m_bIsEnableMagicDraw`, magic spec row 60).
pub fn magic_draw(sora: &Sora, magic_hit: bool) -> bool {
    magic_hit && sora.status().has(ETresAbilityKind::MagicDraw)
}

/// THE SOUND AND STATUS HALF OF AN EVENT, through the core's own seams: a bounce or get sound is
/// queued as a seam sound at the orb (it reaches the host's `Audio` in the next step, in the
/// event list like every other sound); a collected orb becomes `StatusRequest::PrizeCollected`
/// (HP / MP / focus / munny with its HUD popup), a collected box `StatusRequest::ObtainItem` when
/// the item is one the core knows. Returns true when the event was a collection.
pub fn apply_to_sora(sora: &mut Sora, event: &PrizeEvent) -> bool {
    let sound = |sora: &mut Sora, class: PrizeClass, bound: bool, at: glam::Vec3| {
        sora.debug_emit(DebugEmit::Sound(SoundPlay { at: Attach::World(at), ..SoundPlay::at_sora(SoundId(u32::MAX), sound_name(class, bound)) }));
    };
    match *event {
        PrizeEvent::Bounced { class, position, sound: true, .. } => sound(sora, class, true, position),
        PrizeEvent::GetSound { class, position, .. } => sound(sora, class, false, position),
        PrizeEvent::Collected { class, amount, .. } => {
            match class {
                PrizeClass::Orb(kind) => sora.status_request(StatusRequest::PrizeCollected { kind, amount }),
                // (loot) into the pouch's STOCK with the item popup (`Sora::grant`; it had been
                // `StatusRequest::ObtainItem`, which fills the status list a filled pouch no longer reads).
                // Materials, armour and accessories are not the core's: a host with a HUD stocks every box
                // itself (kh-host `PrizeHost`) and does not come here for them.
                PrizeClass::Box(_, item) => {
                    if let Some(item) = ItemKind::ALL.into_iter().find(|i| *i as u32 == item) {
                        sora.grant(kh_core::status::Reward::Item { item, count: 1 });
                    }
                }
            }
            return true;
        }
        _ => {}
    }
    false
}
