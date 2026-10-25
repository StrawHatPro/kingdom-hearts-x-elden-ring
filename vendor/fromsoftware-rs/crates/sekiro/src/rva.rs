use pelite::pe64::PeView;
use std::sync::LazyLock;
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::core::PCSTR;

mod bundle;
mod rva_data;

pub use bundle::RvaBundle;

use fromsoftware_shared::game_version::{GameVersion, LANG_ID_EN, LANG_ID_JP};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SekiroGameVersion {
    Ww1600,
    Jp1600,
}

impl GameVersion for SekiroGameVersion {
    const NAME: &'static str = "sekiro: shadows die twice";

    fn from_lang_version(lang_id: u16, version: &str) -> Option<Self> {
        match (lang_id, version) {
            (LANG_ID_EN, "1.6.0.0") => Some(Self::Ww1600),
            (LANG_ID_JP, "1.6.0.0") => Some(Self::Jp1600),
            _ => None,
        }
    }
}

impl SekiroGameVersion {
    const fn rvas(self) -> RvaBundle {
        // For lack of a better option, we're assuming that (unlike Elden Ring)
        // the RVAs are the same between the worldwide and Japanese
        // versions, but we haven't actually tested that yet. It's possible
        // we'll need to generate separate Japanese RVAs from a memory dump of
        // the decrypted Japanese executable.
        match self {
            Self::Ww1600 | Self::Jp1600 => rva_data::RVAS,
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
        SekiroGameVersion::detect(&module)
            .unwrap_or_else(|e| panic!("{e}"))
            .rvas()
    });

    &RVAS
}
