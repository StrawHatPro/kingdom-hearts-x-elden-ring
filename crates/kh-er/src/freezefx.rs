//! (FREEZE-FX, THE USER 2026-11-07: "make sure frozen effects are applied to anything that gets frozen by blizzard") KH3'S
//! FROZEN LOOK ON EVERY ELDEN RING ENEMY THAT IS FROZEN, the Elden Ring host's half of the one shared route (kh-core
//! `combat::freeze_look`). UNTESTED IN GAME (agents never launch Elden Ring): the rule is unit-tested offline, the look is
//! pictured by the effects harness (`--freeze-fx`).
//!
//! FROZEN = what holds the body now, whoever froze it, generic for any body (no table of enemies):
//! - KH3's FREEZE bad status of any hit (Sora's Blizzard rows, Goofy's, a Keyblade's or a form's FREEZE rows, Donald's Freeze
//!   and Blizzard lines: every KH3 attack row whose bad status is FREEZE): kh-react's Hold in its Frozen phase (puppet.rs
//!   `frozen`), exactly the row's time, ended early by a breaking hit, a death or a release; bosses included whenever their
//!   gates let the freeze land;
//! - Donald's Frozen status (kh-party's status book, donstatus.rs `frozen_ids`): its own length, its Shatter.
//!
//! Once a step ([`FreezeFx::step`], sora.rs after the step's own effects) the list goes to the shared route: the ice row
//! (`vs_abn_frz0` + `vs_abn_frz_in`) on the body for exactly as long as it is frozen, the break row (`vs_abn_frz3` +
//! `vs_abn_frz_out0`, its SE se00002_116) the step it ends; on its feet, scaled to its own half height; its smoke and shards
//! starting on ITS OWN BONES: the model read from the game's files and posed from the game's memory (zdraw.rs `bones_of`,
//! Zettaflare's reader), else points on its capsule (sheet kh3-freeze-look `capsule.*`). The effects carry this host's own
//! handles (above the party ranges: neither `stop_sora` nor `stop_party` touches them). A body gone from the list (despawned,
//! unloaded) loses its look at once, no break.

use kh_core::combat::freeze_look::{capsule_bones, half_height_of, shell_ice, Change, FreezeLook, Frozen, Victim, P};
use kh_core::{EffectHandle, TargetId, Targets};
use kh_host::seam::EffectQueue;

use crate::erlog;
use crate::targets::ErTargets;

/// This host's own effect handles (above the party ranges, below the hero light's `0x7D00_0000`).
const HANDLE_BASE: u32 = 0x7C00_0000;
const HANDLE_SPAN: u32 = 0x0010_0000;

/// The frozen enemies' looks (one per body).
#[derive(Default)]
pub struct FreezeFx {
    look: FreezeLook,
    next: u32,
    /// The bodies whose route of bones was told (its own model / its capsule), for the log.
    told: Vec<(u32, bool)>,
    /// (ice shell) the bodies whose freeze ended: (target, seconds it was frozen, seconds since the break)
    breaking: Vec<(u32, f32, f32)>,
    /// (ice shell) the bodies whose shell route was told (drawn from its own model / KH3's particles alone)
    shell_told: Vec<(u32, bool)>,
}

/// (ice shell) The sheet's look for the renderer.
fn shell_look() -> kh_er_render::iceshell::ShellLook {
    kh_er_render::iceshell::ShellLook { tile_uv: P.shell_tile_uv, tile_cm: P.shell_tile_cm, facing_share: P.shell_facing_share, emissive: P.shell_emissive, light_wrap: P.shell_light_wrap, lift_cm: P.shell_lift_cm, colour_slope: P.shell_colour_slope, colour_base: P.shell_colour_base }
}

impl FreezeFx {
    /// The bodies frozen now: KH3's Hold (any FREEZE row) and Donald's Frozen.
    pub fn frozen_ids(targets: &ErTargets) -> Vec<TargetId> {
        let mut ids = targets.puppets.frozen();
        ids.extend(crate::donstatus::frozen_ids());
        ids.sort_by_key(|t| t.0);
        ids.dedup();
        ids
    }

    /// ONCE A STEP (sora.rs, after the step's own effects): the frozen list to the shared route; the bodies' bones to the
    /// effect side.
    pub fn step(&mut self, targets: &ErTargets, frame: crate::coords::Frame, dt: f32, fx: &mut EffectQueue) {
        let ids = Self::frozen_ids(targets);
        if ids.is_empty() && self.look.on().next().is_none() && self.breaking.is_empty() {
            return;
        }
        let mut list = Vec::with_capacity(ids.len());
        let mut bones = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(info) = targets.info(id) else { continue };
            let own = targets.list.iter().find(|e| e.id == id).and_then(|e| crate::zdraw::bones_of(&e.handle, frame, P.bones_max.max(1) as usize));
            // sized to the body: its capsule, or its own bones' reach (a winged or long body)
            let hh = own.as_ref().map_or(info.half_height, |b| half_height_of(info.half_height, &b.iter().map(|x| x.1).collect::<Vec<_>>()));
            list.push(Frozen::target(id, info.center, info.position, hh));
            if !self.told.contains(&(id.0, own.is_some())) {
                self.told.retain(|t| t.0 != id.0);
                self.told.push((id.0, own.is_some()));
                erlog!("freeze-fx: target {}: KH3's frozen look on {}", id.0, if own.is_some() { "its own bones (its model read from the game's files, posed from the game)" } else { "its capsule (its model is not read yet or cannot be)" });
            }
            let points = own.unwrap_or_else(|| capsule_bones(info.center, info.radius, info.half_height));
            bones.push((id, points.into_iter().map(|(n, p)| (n, (p, glam::Quat::IDENTITY))).collect()));
        }
        // a body gone from the game's list: its look ends at once (no break: nothing is left to break)
        let gone: Vec<Victim> = self.look.on().map(|v| v.0).filter(|v| matches!(v, Victim::Target(t) if targets.info(*t).is_none())).collect();
        for v in gone {
            self.look.forget(v, fx);
        }
        // (ice shell) the frozen time of the bodies whose freeze ends this step, before the look forgets them
        let was_on: Vec<(u32, f32)> = self.look.on().filter_map(|(v, t)| if let Victim::Target(id) = v { Some((id.0, t)) } else { None }).collect();
        let next = &mut self.next;
        let mut handle = || {
            *next = (*next + 1) % HANDLE_SPAN;
            EffectHandle(HANDLE_BASE + *next)
        };
        for c in self.look.step(&list, dt, &mut handle, fx) {
            match c {
                Change::Froze(Victim::Target(t)) => {
                    let f = list.iter().find(|f| f.victim == Victim::Target(t));
                    erlog!("freeze-fx: target {} FROZEN: KH3's ice (ead_com_abn0 0xC1C: vs_abn_frz0 + vs_abn_frz_in) on it, size x{:.2}", t.0, f.map_or(1.0, |f| f.scale));
                }
                Change::Broke(Victim::Target(t)) => {
                    erlog!("freeze-fx: target {}: the freeze ENDED: its ice stopped, KH3's break (0xC1F: vs_abn_frz3 + vs_abn_frz_out0)", t.0);
                    let frozen_for = was_on.iter().find(|w| w.0 == t.0).map_or(0.0, |w| w.1);
                    self.breaking.retain(|b| b.0 != t.0);
                    self.breaking.push((t.0, frozen_for, 0.0));
                }
                _ => {}
            }
        }
        self.told.retain(|t| self.look.is_on(Victim::Target(TargetId(t.0))));
        let _ = crate::fx::with_effects(|e| e.set_target_bones(bones));
        self.shells(targets, frame, dt);
    }

    /// (ice shell) THE ENCASING of every frozen body (and of a broken one for the 0.1 s its ice takes to go): its own posed
    /// model handed to the renderer (kh-er-render iceshell.rs) with KH3's "Ice" amount; a body whose model cannot be read keeps
    /// KH3's particle look alone (logged).
    fn shells(&mut self, targets: &ErTargets, frame: crate::coords::Frame, dt: f32) {
        for b in &mut self.breaking {
            b.2 += dt.max(0.0);
        }
        self.breaking.retain(|b| b.2 < P.shell_ice_out_s);
        let mut wanted: Vec<(u32, f32)> = self.look.on().filter_map(|(v, t)| if let Victim::Target(id) = v { Some((id.0, shell_ice(t, None))) } else { None }).collect();
        wanted.extend(self.breaking.iter().map(|b| (b.0, shell_ice(b.1, Some(b.2)))));
        let mut bodies = Vec::with_capacity(wanted.len());
        for (id, ice) in wanted {
            let soup = targets.list.iter().find(|e| e.id.0 == id).and_then(|e| crate::zdraw::shell_of(&e.handle, frame, P.shell_max_triangles.max(1) as usize));
            if !self.shell_told.contains(&(id, soup.is_some())) {
                self.shell_told.retain(|t| t.0 != id);
                self.shell_told.push((id, soup.is_some()));
                erlog!("freeze-fx: target {id}: {}", if soup.is_some() { "its ICE SHELL drawn from its own model (KH3's ice over its posed FLVER)" } else { "NO ice shell (its model is not read yet or cannot be): KH3's particle look alone (decal, smoke, shards on its capsule or bones)" });
            }
            if let Some(soup) = soup {
                bodies.push((id, ice, soup));
            }
        }
        self.shell_told.retain(|t| self.look.is_on(Victim::Target(TargetId(t.0))) || self.breaking.iter().any(|b| b.0 == t.0));
        kh_er_render::iceshell::set_frame(bodies, shell_look());
    }
}
