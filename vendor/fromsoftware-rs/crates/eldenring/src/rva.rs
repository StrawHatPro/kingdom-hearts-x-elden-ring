use pelite::pe64::PeView;
use std::sync::LazyLock;
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::core::PCSTR;

mod bundle;
mod rva_jp;
mod rva_ww;
// (PORT) kh3-decomp: Elden Ring 2.7.2.0 (WW, 2026-10-21 patch), mapped with tools/binary-mapper.
mod rva_ww2720;

pub use bundle::RvaBundle;

use fromsoftware_shared::game_version::{GameVersion, LANG_ID_EN, LANG_ID_JP};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ERGameVersion {
    Ww2710,
    Jp2711,
    // (PORT)
    Ww2720,
}

impl GameVersion for ERGameVersion {
    const NAME: &'static str = "elden ring";

    fn from_lang_version(lang_id: u16, version: &str) -> Option<Self> {
        match (lang_id, version) {
            (LANG_ID_EN, "2.7.1.0") => Some(Self::Ww2710),
            // (PORT)
            (LANG_ID_EN, "2.7.2.0") => Some(Self::Ww2720),
            (LANG_ID_JP, "2.7.1.1") => Some(Self::Jp2711),
            _ => None,
        }
    }
}

impl ERGameVersion {
    const fn rvas(self) -> RvaBundle {
        match self {
            Self::Ww2710 => rva_ww::RVAS,
            Self::Jp2711 => rva_jp::RVAS,
            // (PORT)
            Self::Ww2720 => rva_ww2720::RVAS,
        }
    }
}

/// Returns the RVA bundle for the current executable region and version.
///
/// This will panic if the current executable isn't supported by this package.
pub fn get() -> &'static RvaBundle {
    static RVAS: LazyLock<RvaBundle> = LazyLock::new(|| {
        let module = unsafe {
            PeView::module(GetModuleHandleA(PCSTR(std::ptr::null())).unwrap().0 as *const u8)
        };
        ERGameVersion::detect(&module)
            .unwrap_or_else(|e| panic!("{e}"))
            .rvas()
    });

    &RVAS
}
