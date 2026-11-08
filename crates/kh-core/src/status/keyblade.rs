//! (weapons P2, 2026-11-03) SORA'S KEYBLADES: the 17 Keyblades of KH3 (base game + Re Mind), their Keyblade Forge
//! levels, the three Keyblade slots with the current one, and KH3's Keyblade switch (documentation/KH3_WEAPONS_PLAN.md
//! 1.3; the data: documentation/KH3_WEAPON_DATA.md).
//!
//! THE DATA: `sheets/port/sora-keyblades` (`tools/sora_keyblade_sheet.js`, built from the `kh3-keyblade*` sheets and each
//! `w_soNNN_Wep` class default object), generated into `SORA_KEYBLADES`:
//! - `kb.<KEY>` (kind keyblade): base Strength / Magic, abilities, shotlocks, formchange commands, init / max level;
//! - `kb.<KEY>.lv<n>` (kind level): a Forge level's OWN +Strength / +Magic and the ability it adds;
//! - `kb.<KEY>.fx|sound|anim.<name>`: the weapon blueprint's appear / disappear effects and timings, hit effects, trail,
//!   swing / hit sound sets, anim blueprint (the `kingdom_key.*` rows of `sora-weapon` for every Keyblade).
//!
//! THE RULES (WD 3, Keyblade Forge; [I] decompiled C):
//! - stats = the Keyblade's base + every done level row (KH3 rebuilds them from the rows, 0x14112EBD0); every done row's
//!   ability is added to the weapon's ability list (equipment copies, no AP);
//! - levels go strictly in order 1..=max (10 for all); the `m_bInitAchieved` levels count as done the moment the
//!   Keyblade is held (Ultima Weapon +10, Hunny Spout +3 ...): a level below the init level is never held here;
//! - three slots; the active one is `record+6`. The switch (docs/sora-ui-spec.md 10.5 "Keyblade slots and switching"
//!   [C]): D-pad right / left with the command cursor on Attack asks request-interface slots 234 / 235 (0x141446590 /
//!   0x141446640); next = the following NON-EMPTY slot in the order 0 -> 1 -> 2 -> 0 (0x141782070), previous the other
//!   way (0x1417820E0); nothing happens with one Keyblade. KH3's game help (`kh3_gamehelp.system.detail_01400_00`):
//!   "move the cursor to Attack on the Command Menu and then use <left / right>". The switch itself is the state
//!   WEAPONCHANGE (32, states/weapon_change.rs): the combo carried 0.5 s out of a hit combo, the glide / battle-stance
//!   resume, and a formchange kept by its Keyblade slot across switches.
//!
//! THE KINGDOM KEY ALONE behaves exactly as before this file: its rows are the same numbers as `sora-equipment`'s
//! `WEP_KEYBLADE_SO_00.upgrade.<n>` rows (test `keyblades.rs`), and with one Keyblade there is no switch.
//!
//! `Status` keeps `weapon` / `weapon_upgrade` as the CURRENT Keyblade's key and level (every existing reader and writer
//! goes on working); [`Keyblades`] holds the slots and the level of every Keyblade. `Status::rebuild` reconciles the
//! two (mod.rs `sync_keyblades`).

use crate::enums::ETresAbilityKind;
use std::sync::OnceLock;

include!(concat!(env!("KH_GEN_DIR"), "/sora_keyblades.rs"));

/// Sora's Keyblade slots (`TresEquipmentComponent.m_EquipSlots`, 3; sora-weapon `kingdom_key.equip.slots`).
pub const SLOTS: usize = 3;
/// The Keyblades of the sheet (17: SO_00 .. SO_14, SO_17, SO_18).
pub const COUNT: usize = 17;

/// One Keyblade: its index in the sheet's `kind = keyblade` rows (sheet order = KH3's item order). Never the number in
/// `WEP_KEYBLADEnn` (WD 2: those enum names are in another order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeybladeId(u8);

/// What the catalogue keeps per Keyblade: its row, its level rows (index = level - 1) and its other rows.
struct Entry {
    row: &'static SoraKeybladeRow,
    levels: Vec<&'static SoraKeybladeRow>,
    fields: Vec<&'static SoraKeybladeRow>,
}

fn catalogue() -> &'static [Entry] {
    static CATALOGUE: OnceLock<Vec<Entry>> = OnceLock::new();
    CATALOGUE.get_or_init(|| {
        let mut out: Vec<Entry> = SORA_KEYBLADES.iter().filter(|r| r.kind == "keyblade").map(|row| Entry { row, levels: Vec::new(), fields: Vec::new() }).collect();
        for r in SORA_KEYBLADES.iter().filter(|r| r.kind != "keyblade") {
            if let Some(entry) = out.iter_mut().find(|e| e.row.keyblade == r.keyblade) {
                if r.kind == "level" {
                    entry.levels.push(r);
                } else {
                    entry.fields.push(r);
                }
            }
        }
        for entry in &mut out {
            entry.levels.sort_by_key(|r| r.level);
        }
        out
    })
}

/// A Keyblade's Strength / Magic at a level (base + every level row up to it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct KeybladeStats {
    pub strength: i32,
    pub magic: i32,
}

impl KeybladeId {
    /// The start weapon (`TresChrInitData` m_PlayerSora.m_Weapons): the sheet's first row.
    pub const KINGDOM_KEY: KeybladeId = KeybladeId(0);

    /// Every Keyblade, in the sheet's order.
    pub fn all() -> impl Iterator<Item = KeybladeId> {
        (0..catalogue().len()).map(|i| KeybladeId(i as u8))
    }

    /// By the item key (`WEP_KEYBLADE_SO_00`); `None` for anything else (an empty weapon, a friend's weapon ...).
    pub fn from_key(key: &str) -> Option<KeybladeId> {
        catalogue().iter().position(|e| e.row.keyblade == key).map(|i| KeybladeId(i as u8))
    }

    /// By the item id (0x301 ..).
    pub fn from_item_id(item_id: u32) -> Option<KeybladeId> {
        catalogue().iter().position(|e| e.row.item_id == item_id).map(|i| KeybladeId(i as u8))
    }

    /// Its index 0..COUNT (the position in [`Keyblades`]' level array).
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    fn entry(self) -> &'static Entry {
        &catalogue()[self.index()]
    }

    /// Its `kb.<KEY>` row.
    pub fn row(self) -> &'static SoraKeybladeRow {
        self.entry().row
    }

    /// The item key (`WEP_KEYBLADE_SO_00`): the `sora-equipment` / item-table id and `Status::weapon`.
    pub fn key(self) -> &'static str {
        self.row().keyblade
    }

    pub fn name(self) -> &'static str {
        self.row().name
    }

    /// The level it has when obtained (the highest `m_bInitAchieved` row).
    pub fn init_level(self) -> u8 {
        self.row().init_level.min(u8::MAX as u32) as u8
    }

    /// The highest Forge level (10 for all).
    pub fn max_level(self) -> u8 {
        self.entry().levels.len().min(u8::MAX as usize) as u8
    }

    /// A level as KH3 can hold it: `init_level ..= max_level`.
    pub fn clamp_level(self, level: u8) -> u8 {
        level.clamp(self.init_level(), self.max_level())
    }

    /// The row of Forge level `level` (1..=max).
    pub fn level_row(self, level: u8) -> Option<&'static SoraKeybladeRow> {
        let levels = &self.entry().levels;
        (level >= 1).then(|| levels.get(level as usize - 1)).flatten().copied()
    }

    /// Strength / Magic at `level` (clamped into the table): base + the own plus of every level row up to it.
    pub fn stats(self, level: u8) -> KeybladeStats {
        let row = self.row();
        let mut stats = KeybladeStats { strength: row.strength, magic: row.magic };
        for r in self.entry().levels.iter().take(level as usize) {
            stats.strength += r.strength;
            stats.magic += r.magic;
        }
        stats
    }

    /// The abilities the Keyblade gives at `level`: its own (`m_AppendAbility`), then the ability of every level row up
    /// to it, in that order, as game names (`PRIZE_DRAW`, `FORM_TIME` ...). A name that is not an `ETresAbilityKind` is
    /// left out by [`Self::abilities`].
    pub fn ability_names(self, level: u8) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = self.row().abilities.to_vec();
        out.extend(self.entry().levels.iter().take(level as usize).map(|r| r.ability).filter(|a| *a != "-"));
        out
    }

    /// [`Self::ability_names`] as kinds.
    pub fn abilities(self, level: u8) -> Vec<ETresAbilityKind> {
        self.ability_names(level).into_iter().filter_map(ETresAbilityKind::from_game_name).collect()
    }

    /// Its formchange commands (`CHANGE_L1`, `CHANGE_SH` ...), KH3's order: what the form framework (P3) offers.
    pub fn forms(self) -> &'static [&'static str] {
        self.row().forms
    }

    /// Its wear forms (`TRES_WEAR_FORM_05` ...).
    pub fn wear_forms(self) -> &'static [&'static str] {
        self.row().wear_forms
    }

    /// Its shotlocks (`m_ShootflowAbilitys`, `ETresTextAbilityKind` names).
    pub fn shotlocks(self) -> &'static [&'static str] {
        self.row().shotlocks
    }

    /// One of its fx / sound / anim rows by the part of the id after the key (`fx.appear.effect`, `sound.swing_set`,
    /// `anim.blueprint` ...).
    pub fn field(self, name: &str) -> Option<&'static SoraKeybladeRow> {
        let key = self.key();
        self.entry().fields.iter().copied().find(|r| r.id.strip_prefix("kb.").and_then(|s| s.strip_prefix(key)).and_then(|s| s.strip_prefix('.')) == Some(name))
    }

    /// The asset of a field (`fx.appear.effect` -> `/Game/Effects/wep/w_so010/par/VW_SO010_APP0`); `None` for "-".
    pub fn asset(self, name: &str) -> Option<&'static str> {
        self.field(name).map(|r| r.asset).filter(|a| *a != "-")
    }

    /// The number of a field (a time in s, a scale, or a flag 1 / 0); 0 when the field is missing.
    pub fn number(self, name: &str) -> f32 {
        self.field(name).map_or(0.0, |r| r.number)
    }

    /// The effect name of an fx asset as the effect sheets key it (the asset's last path part: `VW_SO010_APP0`).
    pub fn effect_name(self, name: &str) -> Option<&'static str> {
        self.asset(name).and_then(|a| a.rsplit('/').next())
    }
}

/// THE KEYBLADE SAVE STATE: the three slots, the current slot and a level per Keyblade. Plain data (`Copy`): part of
/// `Status`, copied with it (Mimic Sora). The level of a Keyblade is kept even while it is in no slot; it is always
/// within `init_level ..= max_level`. A Keyblade is in at most one slot.
///
/// THE STORAGE INTERFACE FOR THE SAVE (P1, `kh-host/src/weapons.rs` / `save.rs` v6; agreed through
/// `build/sora_tmp/weapons/P2/STATE.md`): write [`Self::slot_keys`], [`Self::current_slot`], [`Self::levels_by_key`];
/// read back with [`Self::from_keys`]; hand the result to `Status::set_keyblades`. The slots' stored forms
/// ([`SlotForm`]) are run-time only (KH3 keeps them in the equipment component's slot work, not in the save).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyblades {
    slots: [Option<KeybladeId>; SLOTS],
    current: u8,
    levels: [u8; COUNT],
    /// The formchange each slot's Keyblade was in when it was switched away from (`TresEquipWeaponSlotWork`: weapon
    /// form, grow level, time left, finish level; states/weapon_change.rs).
    forms: [Option<SlotForm>; SLOTS],
}

/// (weapons P2) A formchange held by a Keyblade slot while another Keyblade is in hand (states/weapon_change.rs): the
/// controller's form fields as they were when the switch took the form off, put back when the slot is current again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotForm {
    /// controller+0xB38 / +0xB34: the form's command and level.
    pub cmd: i32,
    pub level: i32,
    /// controller+0xB54 / +0xB58: time left / full.
    pub time_left: f32,
    pub time_full: f32,
    /// controller+0xB64 / +0xB68: the finish level and its command.
    pub finish_level: i32,
    pub finish_cmd: i32,
    /// The slot's grow level (`TresEquipWeaponSlotWork` +0x0D) and the wear form of its weapon form.
    pub grow: u8,
    pub wear_form: u8,
}

impl Default for Keyblades {
    /// No Keyblade in any slot; every level at its init level.
    fn default() -> Keyblades {
        let mut levels = [0; COUNT];
        for id in KeybladeId::all() {
            if let Some(level) = levels.get_mut(id.index()) {
                *level = id.init_level();
            }
        }
        Keyblades { slots: [None; SLOTS], current: 0, levels, forms: [None; SLOTS] }
    }
}

impl Keyblades {
    /// Only the Kingdom Key, in slot 0 (a new game).
    pub fn kingdom_key() -> Keyblades {
        let mut k = Keyblades::default();
        k.slots[0] = Some(KeybladeId::KINGDOM_KEY);
        k
    }

    /// The Keyblade in a slot (0..3).
    pub fn slot(&self, slot: usize) -> Option<KeybladeId> {
        self.slots.get(slot).copied().flatten()
    }

    pub fn slots(&self) -> [Option<KeybladeId>; SLOTS] {
        self.slots
    }

    /// The active slot (`record+6`).
    pub fn current_slot(&self) -> usize {
        self.current as usize
    }

    /// The Keyblade in the active slot.
    pub fn current(&self) -> Option<KeybladeId> {
        self.slot(self.current_slot())
    }

    /// How many slots hold a Keyblade.
    pub fn filled(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// The slot holding `id`.
    pub fn slot_of(&self, id: KeybladeId) -> Option<usize> {
        self.slots.iter().position(|s| *s == Some(id))
    }

    /// The level of a Keyblade (at least its init level).
    pub fn level(&self, id: KeybladeId) -> u8 {
        self.levels.get(id.index()).copied().map_or(0, |l| id.clamp_level(l))
    }

    /// Sets a Keyblade's level, clamped into `init_level ..= max_level` (the Forge raises it one level at a time; a
    /// save may set it directly). Returns the level held.
    pub fn set_level(&mut self, id: KeybladeId, level: u8) -> u8 {
        let level = id.clamp_level(level);
        if let Some(l) = self.levels.get_mut(id.index()) {
            *l = level;
        }
        level
    }

    /// Puts a Keyblade into a slot (`None` empties it). A Keyblade already in another slot changes places with what
    /// this slot held (KH3 has one of each Keyblade). The active slot cannot be emptied (KH3's equip menu refuses
    /// "Remove" on the current Keyblade with `kh3_camp.equip.dialog_0001`): `Err` then, nothing changes.
    pub fn set_slot(&mut self, slot: usize, id: Option<KeybladeId>) -> Result<(), &'static str> {
        if slot >= SLOTS {
            return Err("no such Keyblade slot");
        }
        if id.is_none() && slot == self.current_slot() && self.current().is_some() {
            return Err("the current Keyblade cannot be removed");
        }
        if let Some(other) = id.and_then(|id| self.slot_of(id)).filter(|&other| other != slot) {
            self.slots[other] = self.slots[slot];
            self.forms.swap(other, slot);
        } else if self.slots[slot] != id {
            // Another Keyblade in this slot: the old one's stored form goes with it.
            self.forms[slot] = None;
        }
        self.slots[slot] = id;
        Ok(())
    }

    /// The form a slot holds while another Keyblade is in hand (states/weapon_change.rs).
    pub fn slot_form(&self, slot: usize) -> Option<SlotForm> {
        self.forms.get(slot).copied().flatten()
    }

    pub(crate) fn set_slot_form(&mut self, slot: usize, form: Option<SlotForm>) {
        if let Some(f) = self.forms.get_mut(slot) {
            *f = form;
        }
    }

    /// Empties a slot, the active one included (`Status` unarmed: an empty `weapon`).
    pub(crate) fn clear_slot(&mut self, slot: usize) {
        if let Some(s) = self.slots.get_mut(slot) {
            *s = None;
        }
        self.set_slot_form(slot, None);
    }

    /// Makes a slot the active one; `Err` for an empty slot.
    pub fn set_current(&mut self, slot: usize) -> Result<(), &'static str> {
        match self.slot(slot) {
            Some(_) => {
                self.current = slot as u8;
                Ok(())
            }
            None => Err("an empty Keyblade slot cannot be the current one"),
        }
    }

    /// THE SLOT SEARCH of the switch (0x141782070 next / 0x1417820E0 previous): the following NON-EMPTY slot after the
    /// active one, in the order 0 -> 1 -> 2 -> 0 (`forward`) or the other way. `None` when no other slot holds one.
    pub fn next_slot(&self, forward: bool) -> Option<usize> {
        (1..SLOTS).map(|step| if forward { (self.current_slot() + step) % SLOTS } else { (self.current_slot() + SLOTS - step) % SLOTS }).find(|&s| self.slot(s).is_some())
    }

    /// (save) The slots as item keys.
    pub fn slot_keys(&self) -> [Option<&'static str>; SLOTS] {
        self.slots.map(|s| s.map(KeybladeId::key))
    }

    /// (save) Every Keyblade's level by item key, sheet order.
    pub fn levels_by_key(&self) -> Vec<(&'static str, u8)> {
        KeybladeId::all().map(|id| (id.key(), self.level(id))).collect()
    }

    /// (save) Builds the state back from item keys. Unknown keys are left out and said in the returned notes; a
    /// Keyblade named in two slots keeps the first; an empty or bad current slot becomes the first filled one; levels
    /// are clamped into each Keyblade's range.
    pub fn from_keys(slots: &[Option<&str>], current: usize, levels: &[(&str, u8)]) -> (Keyblades, Vec<String>) {
        let mut k = Keyblades::default();
        let mut notes = Vec::new();
        for (slot, key) in slots.iter().take(SLOTS).enumerate() {
            let Some(key) = key.filter(|k| !k.is_empty()) else { continue };
            match KeybladeId::from_key(key) {
                Some(id) if k.slot_of(id).is_none() => k.slots[slot] = Some(id),
                Some(_) => notes.push(format!("Keyblade {key} is in two slots: slot {slot} is left empty")),
                None => notes.push(format!("Keyblade slot {slot}: \"{key}\" is not a row of sheets/port/sora-keyblades")),
            }
        }
        for &(key, level) in levels {
            match KeybladeId::from_key(key) {
                Some(id) => {
                    k.set_level(id, level);
                }
                None => notes.push(format!("Keyblade level of \"{key}\": not a row of sheets/port/sora-keyblades")),
            }
        }
        k.current = if k.slot(current).is_some() { current as u8 } else { k.slots.iter().position(Option::is_some).unwrap_or(0) as u8 };
        (k, notes)
    }
}

/// One Keyblade switch (state WEAPONCHANGE 32's outcome): what the host shows. The core has already spawned the old
/// Keyblade's disappear effect and the new one's appear effect on the weapon socket (`R_wep_skt`); the host draws the
/// new weapon (P5) after `appear_visible_delay` and plays KH3's Attack-slot text change (`setMainCommand(10)`,
/// `ATTACK_TEXT_CHANGE`, labels `weapon` / `weapon2`, 60 frames; docs/sora-ui-spec.md 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeybladeSwitch {
    pub from: Option<KeybladeId>,
    pub to: KeybladeId,
    pub from_slot: u8,
    pub to_slot: u8,
}

impl KeybladeSwitch {
    /// The old Keyblade's `m_DisappearEffect` (sheet `kb.<KEY>.fx.disappear.effect`).
    pub fn disappear_effect(&self) -> Option<&'static str> {
        self.from.and_then(|id| id.effect_name("fx.disappear.effect"))
    }

    /// The new Keyblade's `m_AppearEffect` (sheet `kb.<KEY>.fx.appear.effect`).
    pub fn appear_effect(&self) -> Option<&'static str> {
        self.to.effect_name("fx.appear.effect")
    }

    /// How long the new mesh stays hidden after the switch (`m_AppearVisibleDelayTime`, s).
    pub fn appear_visible_delay(&self) -> f32 {
        self.to.number("fx.appear.visible_delay")
    }
}

/// THE SWITCH COMMAND (KH3's request-interface slots 234 / 235, 0x141446590 / 0x141446640): D-pad right (`forward`) /
/// left with the command cursor on Attack (command/mod.rs). The weapon restriction (none in the port), the slot search
/// (`next_slot`: none with one Keyblade), a Keyblade in that slot, then the request: state WEAPONCHANGE (32,
/// states/weapon_change.rs), which makes the switch and carries combo and form rules. Returns whether it was queued.
pub(crate) fn request_switch(state: &crate::states::StateData, cx: &mut crate::pawn::Ctx, forward: bool) -> bool {
    let Some(slot) = cx.pawn.status.keyblades().next_slot(forward) else { return false };
    crate::states::weapon_change::try_weapon_change(state, cx, slot)
}

// ------------------------------------------------------------------ (P2 hitfx) the weapons' own hit effects

/// (P2 hitfx) Where a hit window takes its hit effects from (KH3's chooser 0x140628960 reads the `m_HitEffects` of the
/// TresAtkColl0 component the window's primitive belongs to; docs/sora-combat-spec.md 8.6):
/// - `Entry`: the entry of that component for the window's effect group (sheet `sora-keyblades` `hit.<bp>.<group>`);
/// - `NoEntry`: the component has no entry for the group: no effect and no entry sound (KH3's rule);
/// - `Default`: a window that does not come from a weapon / the pawn (a shot made in code, a team shot, an XS shape): the
///   port's former choice (the Kingdom Key's table) stays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitSource {
    Default,
    Entry(&'static SoraKeybladeRow),
    NoEntry,
}

/// The pawn's own component (`p_ex001_PawnBase` TresAtkColl0: DM_ARIEL_DIVE, DM_STITCH_DIVE, FUSION_ROCKET, AN_LHAND).
pub const PAWN_COMPONENT: &str = "p_ex001_PawnBase";

/// The `m_HitEffects` entry `group` of the weapon blueprint `blueprint` (`w_so052_Wep`, any case).
pub fn hit_entry(blueprint: &str, group: &str) -> Option<&'static SoraKeybladeRow> {
    SORA_KEYBLADES.iter().find(|r| r.kind == "hit" && r.blueprint.eq_ignore_ascii_case(blueprint) && r.group.eq_ignore_ascii_case(group))
}

/// The blueprint name of a unit string (`RWeapon@RIGHT_HAND:R_wep_skt=/Game/Blueprints/Player/p_ex001/w_so052_Wep`) or of
/// a path (`/Game/Blueprints/Player/p_ex001/w_so030_Wep`): `w_so052_Wep`.
pub fn blueprint_of(unit: &str) -> &str {
    let tail = unit.rsplit(['=', '/']).next().unwrap_or(unit);
    tail.split('.').next().unwrap_or(tail)
}

/// The weapon model of a blueprint (`w_so052_Wep` -> `w_so052`): the sound sets' and the effect folders' key.
pub fn model_of(blueprint: &str) -> &str {
    blueprint.get(..7).filter(|m| m.starts_with("w_so")).unwrap_or(blueprint)
}

/// THE WEAPON COMPONENTS a melee hit can come from, in the order the port tries them: the held form's weapon units (its
/// right-hand unit first; `Form::weapon_units` at its grow level), else the held Keyblade's own blueprint (sheet
/// `kb.<KEY>.blueprint`), else the Kingdom Key's.
pub fn attack_weapons(form_units: &[&'static str], held: Option<KeybladeId>) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = form_units.iter().map(|u| blueprint_of(u)).filter(|b| b.starts_with("w_so")).collect();
    if out.is_empty() {
        out.push(blueprint_of(held.unwrap_or(KeybladeId::KINGDOM_KEY).row().blueprint));
    }
    out
}

/// THE HIT SOURCE of a weapon window (its notify's begin, combat/hit.rs): the component is Sora's pawn for the pawn's own
/// shape groups (`pawn_shape`), else the first of `weapons` (the hand weapon); with an override group (the notify's
/// `m_bOverrideHitEffect` + `m_HitEffectGrpName`) the first of them, the pawn after them, that has an entry of that group.
pub fn hit_source(weapons: &[&'static str], pawn_shape: bool, override_group: Option<&str>) -> HitSource {
    let found = match override_group {
        Some(group) => weapons.iter().copied().chain(std::iter::once(PAWN_COMPONENT)).find_map(|bp| hit_entry(bp, group)),
        None if pawn_shape => hit_entry(PAWN_COMPONENT, "None"),
        None => weapons.first().and_then(|bp| hit_entry(bp, "None")),
    };
    found.map_or(HitSource::NoEntry, HitSource::Entry)
}

/// (P2 hitfx) Every effect name a Keyblade's own data spawns (its `kb.<KEY>.fx.*` assets and the hit effects of every
/// weapon component it equips: `hit.*` rows with `keyblade` = its key), for a host's effect-name filter: KH3 names some
/// of them after another model (Dead of Night `vw_so120_*`, Oblivion's map hit `vw_so100_wht0`, Oathkeeper `vw_ex090_*`).
pub fn effect_names(id: KeybladeId) -> Vec<&'static str> {
    let key = id.key();
    let mut out: Vec<&'static str> = Vec::new();
    for r in SORA_KEYBLADES.iter().filter(|r| r.keyblade == key && matches!(r.kind, "fx" | "hit")) {
        for asset in std::iter::once(r.asset).chain(r.effects.iter().copied()).chain([r.direct, r.map]) {
            if let Some(name) = effect_name_of(asset) {
                if !out.contains(&name) {
                    out.push(name);
                }
            }
        }
    }
    out
}

/// The asset name an effect spawn takes (`/Game/Effects/wep/w_so030/par/vw_so030_hit` -> `vw_so030_hit`); None for "-".
fn effect_name_of(asset: &'static str) -> Option<&'static str> {
    (asset != "-" && !asset.is_empty()).then(|| asset.rsplit('/').next().unwrap_or(asset))
}

impl HitSource {
    /// The chooser's effect: on a map hit with `m_bChangeMapHit` the map effect, else the attribute's
    /// (`m_PawnHitEffectPerAttr[attribute]`, attribute < 8), else `m_PawnHitEffect`. None: no effect.
    pub fn effect(&self, attribute: usize, map: bool) -> Option<&'static str> {
        let HitSource::Entry(e) = self else { return None };
        if map && e.change_map {
            return effect_name_of(e.map);
        }
        e.effects.get(attribute).copied().and_then(effect_name_of).or_else(|| effect_name_of(e.asset))
    }

    /// `m_DirectHitEffect`: spawned too on a phys shape's hit that is not a map hit.
    pub fn direct(&self) -> Option<&'static str> {
        match self {
            HitSource::Entry(e) => effect_name_of(e.direct),
            _ => None,
        }
    }

    /// `m_fHitEffectRandomOffsetRadius` of the component.
    pub fn random_offset(&self) -> f32 {
        match self {
            HitSource::Entry(e) => e.number,
            _ => 0.0,
        }
    }

    /// The entry's own hit sound (map SE with `m_bChangeSEMapHit` on a map hit, else the attribute's, else the entry's):
    /// None = the window's own sound plays. (No Sora weapon carries one: the sheet's `sounds` are all "-".)
    pub fn sound(&self, attribute: usize, map: bool) -> Option<&'static str> {
        let HitSource::Entry(e) = self else { return None };
        if map && e.change_se_map {
            return e.sounds.get(9).copied().and_then(effect_name_of);
        }
        e.sounds.get(1 + attribute).copied().and_then(effect_name_of).or_else(|| e.sounds.first().copied().and_then(effect_name_of))
    }
}

/// (weapons P2, the coordinator's decision 2026-11-03) THE COMPOSED NOTE for a Keyblade ability that has nothing to act on
/// in Elden Ring, added after KH3's help text where that text would mislead: Harvester ("Pick up one extra cooking
/// ingredient each time you gather": Elden Ring has gathering, and it gives no extra) and Chef Extraordinaire ("Earn an
/// Excellent rating more easily when cooking": Elden Ring's crafting is learned from "Cookbooks"). No cooking exists in
/// the port, so both have no effect; every other Keyblade ability works.
pub const NO_EFFECT_NOTE: &str = "(no effect in this world)";

/// [`NO_EFFECT_NOTE`] for the abilities it belongs to, else `None`.
pub fn help_note(kind: ETresAbilityKind) -> Option<&'static str> {
    matches!(kind, ETresAbilityKind::Harvest | ETresAbilityKind::CharismaChef).then_some(NO_EFFECT_NOTE)
}

/// The old Keyblade's disappear effect and the new one's appear effect on the weapon socket (`R_wep_skt`) for a switch
/// made while the weapon is out (the equipment change's SetVisible with effects).
pub(crate) fn spawn_switch_effects(cx: &mut crate::pawn::Ctx, switch: &KeybladeSwitch) {
    let at = crate::host::Attach::Sora { bone: Some("R_wep_skt"), offset: glam::Vec3::ZERO };
    for name in [switch.disappear_effect(), switch.appear_effect()].into_iter().flatten() {
        cx.spawn_effect(crate::host::EffectSpawn { at, ..crate::host::EffectSpawn::at_sora(crate::host::EffectId(crate::notify::notify_asset_id(name)), name) });
    }
}
