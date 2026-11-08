//! (OF) SORA'S HUD FACE FOLLOWS HIS OUTFIT (THE USER 2026-11-03: "make sure Sora's HUD changes for his costumes and also
//! when he is in battle his battle HUD picture (for each costume) is used"). KH3 loads the status HUD's portrait by the
//! pawn's data asset (UI spec 4): each of Sora's pawns names its own face movie in `UI/Data/Character/<pawn>_StatusData`
//! (read from KH3's data 2026-11-03), and every one of those movies carries KH3's own looks (`faceHyoj` labels `normal`,
//! `battle`, `dying`, the `n2b` / `b2n` turns; the damage flash; the forms' frames), which the status driver picks by
//! KH3's order (KO, low HP, battle, normal: kh-ui hud_status.rs). So the worn outfit only chooses the movie; the battle
//! face, the red low-HP face and the rest come from that movie as they do for his own. KH3's face of the KH1 outfit
//! (FacePcEx1Sora01) is one picture for every look, and so it is here (THE USER 2026-11-03: "just use KH3's KH1 sora face").
//!
//! The host says which outfit is worn ([`Hud::set_sora_outfit`], Elden Ring: kh-er boc.rs every HUD frame, the outfit
//! drawn, after a Boc change and after a load alike); the slots Sora sits in get the new face at once (the slot's face
//! is loaded again, its look and gauges sent again). Sora in a friend slot: KH3's NPC face (`FaceNpcExSora`) with his
//! own outfit, as before; KH3 has no NPC face of a costume, so a costume's own PC face stands there (scaled to the
//! slot by kh-ui's face loader). The Party Formation screen's bust takes the outfit's face sheet too. An outfit the table
//! does not name gets his own face (KH3's default).

use std::sync::Arc;

use kh_ui::def::{MovieDef, Texture};
use kh_ui::Pack;

use super::Hud;
use crate::party::PartyChara;

/// The id of Sora's own outfit (kh-er boc.rs, `PACK/outfits/outfits.json`).
pub const KH3: &str = "kh3";

/// KH3's face movie of each outfit (outfit id of `outfits.json`, its pawn, the movie its `_StatusData` names).
pub const SORA_OUTFIT_FACES: [(&str, &str); 8] = [
    ("kh3", "FacePcExSora01"),          // p_ex001
    ("kh2", "FacePcEx2Sora01"),         // p_ex011
    ("kh1", "FacePcEx1Sora01"),         // p_ex005: one picture for every look, as in KH3
    ("toybox", "FacePcTsSora01"),       // p_ts001
    ("monstropolis", "FacePcMiSora01"), // p_mi001
    ("caribbean", "FacePcCaSora01"),    // p_ca001
    ("sanfransokyo", "FacePcBxSora01"), // p_bx001
    ("verumrex", "FacePcExSora01"),     // p_dl001 (Quadratum): KH3's data gives it his own face
];

/// The face movie of an outfit (his own for one the table does not name).
pub fn sora_face_movie(outfit: &str) -> &'static str {
    SORA_OUTFIT_FACES.iter().find(|f| f.0 == outfit).map_or(SORA_OUTFIT_FACES[0].1, |f| f.1)
}

/// A face movie of the table and its face sheet (the movie's own texture, `01_hud/face/<movie without "Face">.png`: its
/// cell 0 is the Party Formation screen's bust).
pub(super) type OutfitFace = (&'static str, Arc<MovieDef>, Option<Arc<Texture>>);

/// The face movies of the table that the UI pack has (`node tools/sora_ui_pack.js --only=FacePcTsSora01,FacePcMiSora01,
/// FacePcCaSora01,FacePcBxSora01` packs the world costumes' into a pack made before them).
pub(super) fn load_faces(pack: &mut Pack) -> Vec<OutfitFace> {
    let mut out: Vec<OutfitFace> = Vec::new();
    for (outfit, name) in SORA_OUTFIT_FACES {
        if out.iter().any(|f| f.0 == name) {
            continue;
        }
        match pack.movie(name) {
            Ok(def) => out.push((name, def, pack.texture(&format!("textures/Scaleform/01_hud/face/{}.png", name.trim_start_matches("Face"))).ok())),
            Err(error) => println!("hud: the HUD face of Sora's outfit {outfit} ({name}) is NOT in the UI pack: his own face stands in ({error})"),
        }
    }
    out
}

impl Hud {
    /// (OF) The outfit Sora wears now (an id of `outfits.json`; `kh3` = his own). A change puts the outfit's face into every
    /// status slot he sits in at once (its look, battle included, sent again at the next frame).
    pub fn set_sora_outfit(&mut self, id: &str) {
        if id == self.sora_outfit {
            return;
        }
        let before = sora_face_movie(&self.sora_outfit);
        self.sora_outfit = id.to_string();
        (self.log)(&format!("hud: Sora's outfit {id}: his HUD face {}", sora_face_movie(id)));
        if sora_face_movie(id) == before {
            return;
        }
        self.refresh_sora_face();
    }

    /// (OF) Sora's face put again into every slot he sits in, and the Party Formation bust, from the worn outfit's movie.
    fn refresh_sora_face(&mut self) {
        for slot in 0..self.seats.len() {
            if self.seats[slot] == Some(PartyChara::Sora) {
                self.status_driver.faces[slot] = self.face_of(PartyChara::Sora, slot);
                self.status_driver.reload_face(slot);
            }
        }
        // the Party Formation screen's bust: the outfit's face sheet
        if let Some(sheet) = self.sora_outfit_faces.iter().find(|f| f.0 == sora_face_movie(&self.sora_outfit)).and_then(|f| f.2.clone()) {
            self.camp_view.set_sora_bust(sheet);
        }
    }

    /// (OF) The outfit the HUD shows Sora in.
    pub fn sora_outfit(&self) -> &str {
        &self.sora_outfit
    }

    /// (OF) Sora's face by his outfit, in any slot; `None` = his own face's rule (his PC face, his NPC face in a friend slot).
    pub(super) fn outfit_face(&self) -> Option<Arc<MovieDef>> {
        let name = sora_face_movie(&self.sora_outfit);
        if name == SORA_OUTFIT_FACES[0].1 {
            return None;
        }
        self.sora_outfit_faces.iter().find(|f| f.0 == name).map(|f| f.1.clone())
    }
}
