//! (MIMIC-BOSS hotfix 3, MB3-FORMLEAK; THE USER 2026-11-06: "changing forms myself made the mimic's model also change to that
//! form so make sure the boss (and summons) only change when they do a form change themselves") EVERY COPY OF SORA WEARS HIS
//! OWN FORM. Pure state and rules (no GPU): gfx/sora_copy_draw.rs binds what they give.
//!
//! THE LEAK, as found: one set of Sora's assets is loaded, and a form's look lives in the SHARED library (gfx.rs, the costume
//! change, every frame from the PLAYER's wear form):
//! 1. the wear form's textures (kh-look `step_wear`: `FormChangeTex0/1`, `FormChangePointTex0/1`, an outfit's direct colour
//!    replacements, Rage Form's bakes) are written into the SLOTS' DESCRIPTOR TABLES, and every copy's draw (the KH route's
//!    colour, early depth and G-buffer records; the Mimic route's early depth and its Mimic Tear G-buffer, whose albedo keeps
//!    20 % of the material's base colour under WetAspectParam 18) bound those very tables;
//! 2. the slide `FormChangeSlide` on the slots' scalars;
//! 3. Rage Form's look (rage.rs `frame`): `AntiVolume` and the `AOChannel` occlusion on every slot, the anti-mode TWIN slots
//!    drawn in place of the normal ones (the Mimic route took `rage::draw_slot`, the player's), and the glow rows;
//! 4. the effects' material drives (fixed before: vfx.rs `copy_drive_slot`).
//!
//! THE FIX: a copy (by his drives' owner, kh-host `char_drives`: copy member m = m + 1) keeps his OWN wear state ([`CopyWear`]):
//! his own slot textures stepped by kh-look's own rule (`step_wear_in`) from HIS core's wear form, his own Rage fade (rage.rs
//! `Rage`), and from them his slot copy ([`CopyWear::apply`]: his slide, AntiVolume and occlusion), his twin slots
//! ([`CopyWear::draw_slot`]) and, where his textures differ from the table the player's slot holds now, a descriptor table of
//! his own ([`TablePool`], gfx/sora_copy_draw.rs). The player's library and tables are never written by a copy.

use std::collections::HashMap;

use kh_look::library::{step_wear_in, Library, Slot, TextureRef, WearForms, WEAR_MESH};

/// The textures every slot of a library was loaded with (before any wear form): a copy's wear starts from them.
pub type Pristine = Vec<Vec<Option<TextureRef>>>;

/// The textures of every slot of a library as it is now.
pub fn pristine_of(lib: &Library) -> Pristine {
    lib.slots.iter().map(|s| s.textures.clone()).collect()
}

/// The slots a wear form may give other textures (the wear mesh's instances named by a form's costume or a direct
/// replacement): the only ones a copy can need a table of his own for.
pub fn wear_slots(lib: &Library) -> Vec<usize> {
    let named = |inst: &str| lib.wear.forms.values().flatten().any(|(m, _, _)| m.eq_ignore_ascii_case(inst)) || lib.wear.direct.values().flatten().any(|(m, _, _)| m.eq_ignore_ascii_case(inst));
    lib.slots.iter().enumerate().filter(|(_, s)| s.mesh == WEAR_MESH && named(&s.instance_name)).map(|(k, _)| k).collect()
}

/// ONE COPY'S OWN LOOK: his slot textures and wear state, his Rage fade.
#[derive(Clone, Debug)]
pub struct CopyWear {
    /// His slots (index = the library's): only their textures and `FormChangeSlide` are his (no other parameter is kept).
    slots: Vec<Slot>,
    wear: WearForms,
    rage: crate::rage::Rage,
    /// The frame he was last stepped in (a frame steps him once).
    frame: Option<u64>,
}

impl CopyWear {
    /// A copy in his normal clothes: the library's slots with the textures they were loaded with, the library's costumes, no
    /// form worn yet (his first step puts his wear form on at once, as the player's first frame does).
    pub fn new(lib: &Library, pristine: &[Vec<Option<TextureRef>>]) -> CopyWear {
        let slots = lib
            .slots
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let mut own = s.clone();
                own.scalars.clear();
                own.vectors.clear();
                if let Some(t) = pristine.get(k) {
                    own.textures.clone_from(t);
                }
                own
            })
            .collect();
        let wear = WearForms { forms: lib.wear.forms.clone(), direct: lib.wear.direct.clone(), plain: lib.wear.plain, ..WearForms::default() };
        CopyWear { slots, wear, rage: crate::rage::Rage::default(), frame: None }
    }

    /// ONE FRAME OF HIS LOOK (once a frame: `frame`): `wanted` = HIS core's wear form (kh-core `combat.form.wear_form`),
    /// `clock` = the frame's game clock (s). His Rage fade and his costume, by the player's own rules (rage.rs, kh-look).
    pub fn step(&mut self, lib: &Library, wanted: u8, clock: f32, frame: u64) {
        if self.frame == Some(frame) {
            return;
        }
        self.frame = Some(frame);
        let dt = self.rage.clock.map_or(0.0, |before| (clock - before).clamp(0.0, 0.1));
        self.rage.step(wanted, clock);
        let form = self.rage.library_form(wanted);
        self.wear.plain = lib.wear.plain;
        step_wear_in(&mut self.slots, &mut self.wear, form, dt);
    }

    /// The frame he was last stepped in.
    pub fn frame(&self) -> Option<u64> {
        self.frame
    }

    /// His AntiVolume (Rage Form's look, 0..1).
    pub fn volume(&self) -> f32 {
        self.rage.volume
    }

    /// (the costume worn, the one sliding in)
    pub fn worn(&self) -> (u8, u8) {
        (self.wear.worn, self.wear.target)
    }

    /// His textures of a slot (None: a slot he does not know: drawn as the library has it).
    pub fn textures(&self, index: usize) -> Option<&[Option<TextureRef>]> {
        self.slots.get(index).map(|s| s.textures.as_slice())
    }

    /// His textures of a slot differ from those the player's table holds now: he needs a table of his own for it.
    pub fn differs(&self, lib: &Library, index: usize) -> bool {
        match (self.textures(index), lib.slots.get(index)) {
            (Some(own), Some(theirs)) => own != theirs.textures.as_slice(),
            _ => false,
        }
    }

    /// The slot he draws for `index`: its KH3 anti-mode twin while HIS Rage look is on (rage.rs `twin_of`), never by the
    /// player's.
    pub fn draw_slot(&self, index: usize) -> usize {
        if self.rage.volume > 0.0 {
            crate::rage::twin_of(index).unwrap_or(index)
        } else {
            index
        }
    }

    /// His `FormChangeSlide` on a slot (None: the slot has no slide).
    pub fn slide(&self, index: usize) -> Option<f32> {
        self.slots.get(index).and_then(|s| s.scalars.get("FormChangeSlide").copied())
    }

    /// `base` (the library's slot `index`, or its copy with his drives) differs from his version of it: his slide, his Rage
    /// fade (`player_volume`: the player's, rage.rs `volume`, which the library's slots carry) or his textures.
    pub fn needs(&self, lib: &Library, index: usize, base: &Slot, player_volume: f32) -> bool {
        self.rage.volume != player_volume || self.slide(index).is_some_and(|s| base.scalars.get("FormChangeSlide") != Some(&s)) || self.differs(lib, index)
    }

    /// HIS VERSION OF A SLOT: `base` with his textures, his `FormChangeSlide`, and Rage Form's parameters at HIS fade
    /// (rage.rs `apply_slot`: AntiVolume, the occlusion).
    pub fn apply(&self, index: usize, base: &mut Slot) {
        if let Some(own) = self.slots.get(index) {
            base.textures.clone_from(&own.textures);
            if let Some(s) = own.scalars.get("FormChangeSlide") {
                base.scalars.insert("FormChangeSlide".to_string(), *s);
            }
        }
        crate::rage::apply_slot(base, self.rage.volume);
    }
}

/// A copy not stepped for this many frames is gone: his state is dropped (a new copy of the same member starts afresh).
pub const GONE_AFTER_FRAMES: u64 = 30;

/// EVERY COPY'S OWN LOOK for one library (by his drives' owner).
#[derive(Clone, Debug, Default)]
pub struct CopyWears {
    pristine: Pristine,
    by_owner: HashMap<u8, CopyWear>,
}

impl CopyWears {
    /// For a library as loaded (no wear form stepped yet).
    pub fn new(lib: &Library) -> CopyWears {
        CopyWears { pristine: pristine_of(lib), by_owner: HashMap::new() }
    }

    /// One frame of the copy `owner` (made at his first).
    pub fn step(&mut self, lib: &Library, owner: u8, wanted: u8, clock: f32, frame: u64) -> &CopyWear {
        let pristine = &self.pristine;
        let w = self.by_owner.entry(owner).or_insert_with(|| CopyWear::new(lib, pristine));
        w.step(lib, wanted, clock, frame);
        w
    }

    pub fn get(&self, owner: u8) -> Option<&CopyWear> {
        self.by_owner.get(&owner)
    }

    /// The copies seen lately only.
    pub fn forget_gone(&mut self, frame: u64) {
        self.by_owner.retain(|_, w| w.frame().is_some_and(|f| frame.saturating_sub(f) <= GONE_AFTER_FRAMES));
    }
}

/// Copies whose look is drawn at most in one frame (sora_copy.rs `MAX_COPIES` on the KH route + the Mimic route's four).
pub const MAX_LOOKS: usize = crate::sora_copy::MAX_COPIES + 4;

/// What a table of the pool holds: the slot and its texture files, and the frame it was last bound in.
#[derive(Clone, Debug, PartialEq)]
pub struct TableUse {
    pub slot: usize,
    pub files: Vec<Option<String>>,
    pub last: u64,
}

/// What [`TablePool::choose`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// A table already holds these textures (index into the pool).
    Hit(usize),
    /// This table is to be written with them.
    Write(usize),
    /// None free (every table bound within the frames in flight): the player's table is used.
    Full,
}

/// THE COPIES' OWN DESCRIPTOR TABLES (gfx.rs puts them after the slots' and the frame's three): each holds one slot's
/// textures as one copy wears them; a table is reused while its textures are wanted, and written again only once no frame
/// in flight can still read it (`in_flight` frames).
#[derive(Clone, Debug, Default)]
pub struct TablePool {
    pub tables: Vec<Option<TableUse>>,
    pub in_flight: u64,
}

impl TablePool {
    /// `count` tables, `in_flight` frames that may still read a table bound before.
    pub fn new(count: usize, in_flight: u64) -> TablePool {
        TablePool { tables: vec![None; count], in_flight }
    }

    /// The tables a library's copies may need: each drawn copy, each wear slot, twice (one set may still be read in flight).
    pub fn count_for(lib: &Library) -> usize {
        2 * MAX_LOOKS * wear_slots(lib).len()
    }

    /// The table for `slot` with `files` in `frame` (marked as bound in it).
    pub fn choose(&mut self, slot: usize, files: &[Option<String>], frame: u64) -> Choice {
        if let Some(k) = self.tables.iter().position(|u| u.as_ref().is_some_and(|u| u.slot == slot && u.files == files)) {
            if let Some(u) = self.tables[k].as_mut() {
                u.last = frame;
            }
            return Choice::Hit(k);
        }
        let free = |u: &Option<TableUse>| u.as_ref().is_none_or(|u| u.last.saturating_add(self.in_flight) < frame);
        let Some(k) = (0..self.tables.len()).filter(|k| free(&self.tables[*k])).min_by_key(|k| self.tables[*k].as_ref().map_or(0, |u| u.last.saturating_add(1))) else { return Choice::Full };
        self.tables[k] = Some(TableUse { slot, files: files.to_vec(), last: frame });
        Choice::Write(k)
    }
}

/// A slot's texture files (what a table holds).
pub fn files_of(textures: &[Option<TextureRef>]) -> Vec<Option<String>> {
    textures.iter().map(|t| t.as_ref().map(|t| t.file.clone())).collect()
}

#[cfg(test)]
#[path = "copy_wear_tests.rs"]
mod tests;
