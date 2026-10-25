//! Making the Elden Ring player a passive body: the switches that stop the game from moving him
//! (pad, gravity, falling, root motion, sliding), the ones that hide him, and putting everything
//! back. Which switches are needed was found by experiment (state note, "Passive player").

use eldenring::cs::{ChrIns, PlayerIns}; // (ER party S3) ChrIns: `apply_chr`

use crate::control::Control;

/// One set of switches; `None` = leave the game's value alone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Switches {
    /// `ChrDebugFlags.disabled_movement`.
    pub dbg_no_move: Option<bool>,
    /// `ChrDebugFlags.disabled_secondary_actions`.
    pub dbg_no_action: Option<bool>,
    /// `ChrDebugFlags.disabled_hit` (ignore incoming damage).
    pub dbg_no_hit: Option<bool>,
    /// `ChrInsFlags1c4.no_gravity`.
    pub c4_no_gravity: Option<bool>,
    /// `CSChrPhysicsModule.gravity_disabled`.
    pub grav_disabled: Option<bool>,
    /// `CSChrPhysicsModule.gravity_multiplier`.
    pub grav_mult: Option<f32>,
    /// `CSChrPhysicsModule.motion_multiplier` (root motion scale).
    pub motion_mult: Option<f32>,
    /// `CSChrFallModule.disable_fall_motion`.
    pub fall_no_motion: Option<bool>,
    /// Hold `CSChrFallModule.fall_timer` at 0.
    pub fall_timer_zero: Option<bool>,
    /// `ChrPhysicsSlideInfo.enabled` (true = the game may slide him down slopes).
    pub slide: Option<bool>,
    /// `ChrCtrlFlags.disable_map_collision`.
    pub ctrl_no_map_hit: Option<bool>,
    /// `ChrInsFlags1c5.enable_render`.
    pub render: Option<bool>,
    /// `ChrIns.base_transparency`.
    pub transparency: Option<f32>,
    /// Bit 0 of every part model's `disp_flags1` (true = shown).
    pub parts: Option<bool>,
}

impl Switches {
    /// The keys `p.<name>` of the control file.
    pub fn from_control(c: &Control) -> Self {
        Self {
            dbg_no_move: c.flag("p.dbg_no_move"),
            dbg_no_action: c.flag("p.dbg_no_action"),
            dbg_no_hit: c.flag("p.dbg_no_hit"),
            c4_no_gravity: c.flag("p.c4_no_gravity"),
            grav_disabled: c.flag("p.grav_disabled"),
            grav_mult: c.f32("p.grav_mult"),
            motion_mult: c.f32("p.motion_mult"),
            fall_no_motion: c.flag("p.fall_no_motion"),
            fall_timer_zero: c.flag("p.fall_timer_zero"),
            slide: c.flag("p.slide"),
            ctrl_no_map_hit: c.flag("p.ctrl_no_map_hit"),
            render: c.flag("p.render"),
            transparency: c.f32("p.transparency"),
            parts: c.flag("p.parts"),
        }
    }

    /// THE SET FOUND BY EXPERIMENT (state note): what `mode = sora` applies.
    pub fn passive(hidden: bool) -> Self {
        Self {
            dbg_no_move: Some(true),
            dbg_no_action: Some(true),
            dbg_no_hit: None,
            c4_no_gravity: None,
            grav_disabled: Some(true),
            grav_mult: None,
            motion_mult: Some(0.0),
            fall_no_motion: Some(true),
            fall_timer_zero: Some(true),
            slide: Some(false),
            ctrl_no_map_hit: None,
            render: if hidden { Some(false) } else { None },
            transparency: None,
            parts: None,
        }
    }

    /// Values set here override `self`.
    pub fn overlaid(mut self, over: &Switches) -> Self {
        macro_rules! take {
            ($($f:ident),*) => { $( if over.$f.is_some() { self.$f = over.$f; } )* };
        }
        take!(dbg_no_move, dbg_no_action, dbg_no_hit, c4_no_gravity, grav_disabled, grav_mult, motion_mult,
              fall_no_motion, fall_timer_zero, slide, ctrl_no_map_hit, render, transparency, parts);
        self
    }
}

/// The game's values as they were before we touched anything.
#[derive(Clone, Debug)]
pub struct Original {
    dbg_no_move: bool,
    dbg_no_action: bool,
    dbg_no_hit: bool,
    /// `ChrInsFlags1c5.is_invincible` (Sora's dodge frames set it: sora.rs).
    invincible: bool,
    c4_no_gravity: bool,
    grav_disabled: bool,
    grav_mult: f32,
    motion_mult: f32,
    fall_no_motion: bool,
    slide: bool,
    ctrl_no_map_hit: bool,
    render: bool,
    transparency: f32,
    /// (slot, flags) of the part models, and of the body model as slot `usize::MAX`.
    parts: Vec<(usize, u32)>,
}

impl Original {
    pub fn capture(player: &PlayerIns) -> Self {
        let chr = &player.chr_ins;
        let physics = &chr.modules.physics;
        let mut parts = Vec::new();
        if let Some(asm) = player.chr_asm_model_ins.as_ref() {
            for (slot, part) in asm.parts_model_ins.iter().enumerate() {
                if let Some(part) = part {
                    parts.push((slot, part.model_disp_entity.disp_flags1));
                }
            }
        }
        parts.push((usize::MAX, chr.chr_model_ins.model_ins.model_disp_entity.disp_flags1));
        Self {
            dbg_no_move: chr.debug_flags.disabled_movement(),
            dbg_no_action: chr.debug_flags.disabled_secondary_actions(),
            dbg_no_hit: chr.debug_flags.disabled_hit(),
            invincible: chr.chr_flags1c5.is_invincible(),
            c4_no_gravity: chr.chr_flags1c4.no_gravity(),
            grav_disabled: physics.gravity_disabled,
            grav_mult: physics.gravity_multiplier,
            motion_mult: physics.motion_multiplier,
            fall_no_motion: chr.modules.fall.disable_fall_motion,
            slide: physics.slide_info.enabled,
            ctrl_no_map_hit: chr.chr_ctrl.flags.disable_map_collision(),
            render: chr.chr_flags1c5.enable_render(),
            transparency: chr.base_transparency,
            parts,
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "dbg_no_move {} dbg_no_action {} dbg_no_hit {} c4_no_gravity {} grav_disabled {} grav_mult {} motion_mult {} fall_no_motion {} slide {} ctrl_no_map_hit {} render {} transparency {} parts {:x?}",
            self.dbg_no_move, self.dbg_no_action, self.dbg_no_hit, self.c4_no_gravity, self.grav_disabled, self.grav_mult,
            self.motion_mult, self.fall_no_motion, self.slide, self.ctrl_no_map_hit, self.render, self.transparency, self.parts
        )
    }

    /// The switches that put every value back.
    pub fn as_switches(&self) -> Switches {
        Switches {
            dbg_no_move: Some(self.dbg_no_move),
            dbg_no_action: Some(self.dbg_no_action),
            dbg_no_hit: Some(self.dbg_no_hit),
            c4_no_gravity: Some(self.c4_no_gravity),
            grav_disabled: Some(self.grav_disabled),
            grav_mult: Some(self.grav_mult),
            motion_mult: Some(self.motion_mult),
            fall_no_motion: Some(self.fall_no_motion),
            fall_timer_zero: None,
            slide: Some(self.slide),
            ctrl_no_map_hit: Some(self.ctrl_no_map_hit),
            render: Some(self.render),
            transparency: Some(self.transparency),
            parts: None,
        }
    }

    pub fn restore(&self, player: &mut PlayerIns) {
        apply(player, &self.as_switches());
        player.chr_ins.chr_flags1c5.set_is_invincible(self.invincible);
        if let Some(asm) = player.chr_asm_model_ins.as_mut() {
            for &(slot, flags) in &self.parts {
                if slot != usize::MAX {
                    if let Some(part) = asm.parts_model_ins.get_mut(slot).and_then(|p| p.as_mut()) {
                        part.model_disp_entity.disp_flags1 = flags;
                    }
                }
            }
        }
        if let Some(&(_, flags)) = self.parts.iter().find(|(slot, _)| *slot == usize::MAX) {
            player.chr_ins.chr_model_ins.model_ins.model_disp_entity.disp_flags1 = flags;
        }
    }
}

/// Writes the switches; returns how many of the values the game had different (i.e. had reset
/// since we last wrote them, when called every frame).
pub fn apply(player: &mut PlayerIns, s: &Switches) -> u32 {
    // (ER party S3) every switch but the part models is on ChrIns: `apply_chr` makes the same writes in the same order
    // and counts them the same way; the part models stay here (the player's equipment)
    let mut changed = apply_chr(&mut player.chr_ins, s);
    if let Some(shown) = s.parts {
        let bit = shown as u32;
        if let Some(asm) = player.chr_asm_model_ins.as_mut() {
            for part in asm.parts_model_ins.iter_mut().flatten() {
                let flags = &mut part.model_disp_entity.disp_flags1;
                if *flags & 1 != bit {
                    changed += 1;
                    *flags = *flags & !1 | bit;
                }
            }
        }
        let flags = &mut player.chr_ins.chr_model_ins.model_ins.model_disp_entity.disp_flags1;
        if *flags & 1 != bit {
            changed += 1;
            *flags = *flags & !1 | bit;
        }
    }
    changed
}

/// (ER party S3) The switches on any character (`Switches::parts` aside: a part model belongs to the player's
/// equipment): the player through `apply`, the AI allies' bodies (allybody.rs) directly. Returns the count of values
/// the game had different.
pub fn apply_chr(chr: &mut ChrIns, s: &Switches) -> u32 {
    let mut changed = 0;
    macro_rules! set {
        ($get:expr, $set:expr, $want:expr) => {
            if let Some(want) = $want {
                if $get != want {
                    changed += 1;
                    $set(want);
                }
            }
        };
    }
    set!(chr.debug_flags.disabled_movement(), |v| chr.debug_flags.set_disabled_movement(v), s.dbg_no_move);
    set!(chr.debug_flags.disabled_secondary_actions(), |v| chr.debug_flags.set_disabled_secondary_actions(v), s.dbg_no_action);
    set!(chr.debug_flags.disabled_hit(), |v| chr.debug_flags.set_disabled_hit(v), s.dbg_no_hit);
    set!(chr.chr_flags1c4.no_gravity(), |v| chr.chr_flags1c4.set_no_gravity(v), s.c4_no_gravity);
    set!(chr.chr_flags1c5.enable_render(), |v| chr.chr_flags1c5.set_enable_render(v), s.render);
    set!(chr.base_transparency, |v| chr.base_transparency = v, s.transparency);
    set!(chr.chr_ctrl.flags.disable_map_collision(), |v| chr.chr_ctrl.flags.set_disable_map_collision(v), s.ctrl_no_map_hit);
    {
        let fall = chr.modules.fall.as_mut();
        set!(fall.disable_fall_motion, |v| fall.disable_fall_motion = v, s.fall_no_motion);
        if s.fall_timer_zero == Some(true) && fall.fall_timer != 0.0 {
            fall.fall_timer = 0.0;
        }
    }
    {
        let physics = chr.modules.physics.as_mut();
        set!(physics.gravity_disabled, |v| physics.gravity_disabled = v, s.grav_disabled);
        set!(physics.gravity_multiplier, |v| physics.gravity_multiplier = v, s.grav_mult);
        set!(physics.motion_multiplier, |v| physics.motion_multiplier = v, s.motion_mult);
        set!(physics.slide_info.enabled, |v| physics.slide_info.enabled = v, s.slide);
    }
    changed
}
