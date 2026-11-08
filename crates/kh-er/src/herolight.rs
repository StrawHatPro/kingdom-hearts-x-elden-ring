//! (HEROLIGHT-KB, 2026-11-05) THE HERO LIGHT'S EFFECTS ON THE LEADER'S WEAPON. The user: "the light effect for the hero light in
//! dark places needs to adapt to all the keyblades" and "for the hero light, it should just be whoever is party leader, not
//! dependent on Sora". State: build/sora_tmp/weapons/HEROLIGHT-KB/STATE.md.
//!
//! Once a step (sora.rs, the IMPL_ER44 block) the host says who leads and with what weapon (Sora: kh-core's `attack_model`, the
//! hand Keyblade or a form's hand unit; Donald / Goofy: their equipped staff / shield) and whether that weapon is lit (the
//! darkness gate closed, the weapon fully drawn). The weapon's row of `sheets/port/sora-herolight.json` (kh-er-render
//! herolight.rs) gives its own appear effect's steady glow and sparkles, the socket they start at and the light's tint and
//! line; kh-er-render's [`Driver`] decides the starts and stops (a change of weapon or leader ends the old glow and sparkle at
//! once). The effects go into the step's effect queue with this host's own handles (above the party ranges: neither
//! `EffectHost::stop_sora` nor `stop_party` touches them; this host ends them itself), attached to Sora's socket or to the
//! friend's (`Attach::Party`).
//!
//! Hand-off (PARTY-WEAPONS owns the weapon draw / put-away gate): the carrier's weapon is held out by its one rule,
//! `kh_core::weapon_gate::hold(carrier, light_wanted)` (sora.rs: Sora's core hold and kh-party's `set_weapon_hold` for the
//! friends; partydraw.rs: the light at a friend leader only while that hold is on).

use glam::Vec3;
use kh_core::{Attach, EffectHandle, EffectId, EffectSpawn, PartyMember};
use kh_er_render::herolight::{Act, Driver, HeroWeapon};
use kh_host::party::PartyChara;
use kh_host::seam::{EffectCommand, EffectQueue};

use crate::erlog;

/// This host's own effect handles (above the party ranges `0x2000_0000 .. 0x3000_0000`, below the unlock burst `0x7E00_0000`).
const HANDLE_BASE: u32 = 0x7D00_0000;
const HANDLE_SPAN: u32 = 0x0010_0000;

/// The carrier of the step: who leads and the model of the weapon in his hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Carrier<'a> {
    pub who: PartyChara,
    pub model: &'a str,
    /// (HEROLIGHT-KB final pass) the character socket this weapon is on when it is not its row's (the LEFT-hand Keyblade of a
    /// dual-wield form: Double Form's Oblivion on L_wep_skt); None = the row's
    pub socket: Option<&'static str>,
}

#[derive(Default)]
pub struct HeroLight {
    driver: Driver,
    glow: Option<EffectHandle>,
    spark: Option<EffectHandle>,
    next: u32,
    /// the weapons whose two effect copies the effect side has been given
    defined: Vec<&'static str>,
    told: String,
    /// (HEROLIGHT-KB final pass) a second weapon of the same carrier (the left hand of a dual-wield form): its glow and sparkles
    /// only; the light, its tint and its place stay the first weapon's (`set_carrier` is not told)
    secondary: bool,
}

fn attach(who: PartyChara, socket: &'static str) -> Attach {
    match who {
        PartyChara::Sora => Attach::Sora { bone: Some(socket), offset: Vec3::ZERO },
        PartyChara::Donald => Attach::Party { member: PartyMember::DONALD, bone: Some(socket), offset: Vec3::ZERO },
        PartyChara::Goofy => Attach::Party { member: PartyMember::GOOFY, bone: Some(socket), offset: Vec3::ZERO },
    }
}

fn chara_of(id: u8) -> PartyChara {
    match id {
        1 => PartyChara::Donald,
        2 => PartyChara::Goofy,
        _ => PartyChara::Sora,
    }
}

impl HeroLight {
    /// (HEROLIGHT-KB final pass) the left-hand weapon's: its glow and sparkles only (see `secondary`).
    pub fn secondary() -> HeroLight {
        HeroLight { secondary: true, ..HeroLight::default() }
    }

    fn handle(&mut self) -> EffectHandle {
        self.next = (self.next + 1) % HANDLE_SPAN;
        // (the secondary weapon's handles in the next span: never one of the first's)
        EffectHandle(HANDLE_BASE + if self.secondary { HANDLE_SPAN } else { 0 } + self.next)
    }

    /// One host step. `lit`: the light is on (the darkness gate closed, not banned) and the carrier's weapon fully drawn.
    pub fn step(&mut self, dt: f32, lit: bool, carrier: Option<Carrier<'_>>, effects: &mut EffectQueue) {
        let row: Option<(PartyChara, &'static HeroWeapon)> = carrier.and_then(|c| kh_er_render::herolight::weapon(c.model).map(|w| (c.who, w)));
        let line = match (carrier, row) {
            (Some(c), None) => format!("{:?}'s {} is not in the table: no glow", c.who, c.model),
            (Some(c), Some((_, w))) => format!("{:?} carries it on {} ({}: glow {:?}, {} sparkle emitter(s), at {})", c.who, w.model, w.effect, w.glow, w.spark.len(), w.socket),
            (None, _) => "nobody carries it".to_string(),
        };
        if lit && line != self.told {
            erlog!("hero light: (HEROLIGHT-KB) {line}");
            self.told = line;
        }
        // the weapon's two copies of its appear effect, given to the effect side once (retried until it is there)
        if let Some((_, w)) = row {
            if !self.defined.contains(&w.model.as_str()) && crate::fx::with_effects(|e| {
                for (name, base, keep, hold) in w.variants() {
                    let keep: Vec<&str> = keep.iter().map(String::as_str).collect();
                    e.define_variant(name, base, &keep, hold);
                }
            }) {
                self.defined.push(w.model.as_str());
            }
        }
        if !self.secondary {
            kh_er_render::herolight::set_carrier(row.filter(|_| lit).map(|(_, w)| w.model.as_str()));
        }
        let socket_override = carrier.and_then(|c| c.socket);
        let s = kh_er_render::pointlight::settings();
        let acts = self.driver.step(dt, lit, row.map(|(who, w)| (who as u8, w)), &s);
        let who = self.driver.on().map(|(id, _)| chara_of(id));
        for act in acts {
            match act {
                Act::Start { name, socket, glow } => {
                    let socket = socket_override.unwrap_or(socket);
                    let Some(who) = who else { continue };
                    let handle = self.handle();
                    effects.0.push(EffectCommand::Spawn(handle, EffectSpawn { at: attach(who, socket), persistent: glow, ..EffectSpawn::at_sora(EffectId(kh_core::notify::notify_asset_id(name)), name) }));
                    if glow {
                        self.glow = Some(handle);
                    } else {
                        self.spark = Some(handle);
                    }
                }
                Act::Stop { glow, immediate } => {
                    if let Some(handle) = if glow { self.glow.take() } else { self.spark.take() } {
                        effects.0.push(EffectCommand::Stop(handle, immediate));
                    }
                }
            }
        }
    }

    /// Everything of the light ends at once (a path that does not step it: the light is off there).
    pub fn end(&mut self, effects: &mut EffectQueue) {
        for handle in [self.glow.take(), self.spark.take()].into_iter().flatten() {
            effects.0.push(EffectCommand::Stop(handle, true));
        }
        self.driver = Driver::default();
        if !self.secondary {
            kh_er_render::herolight::set_carrier(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawned(q: &EffectQueue) -> Vec<(&'static str, bool, Attach)> {
        q.0.iter().filter_map(|c| match c {
            EffectCommand::Spawn(_, s) => Some((s.name, s.persistent, s.at)),
            _ => None,
        }).collect()
    }

    /// The leader carries it: Sora's Keyblade, then a switch, then Donald's staff on a leader swap: each time the old glow and
    /// sparkle are stopped at once and exactly one new glow starts, on the new carrier's socket.
    #[test]
    fn the_light_moves_with_the_leader_and_the_weapon_without_doubles() {
        let dt = 1.0 / 60.0;
        let mut h = HeroLight::default();
        let mut q = EffectQueue::default();
        h.step(dt, true, Some(Carrier { who: PartyChara::Sora, model: "w_so010", socket: None }), &mut q);
        assert_eq!(spawned(&q).iter().filter(|s| s.1).map(|s| s.0).collect::<Vec<_>>(), ["VW_SO010_APP0#hl_glow"]);
        let kk_glow = h.glow.unwrap();
        q.0.clear();
        // the D-pad switch to Oblivion (DLC)
        h.step(dt, true, Some(Carrier { who: PartyChara::Sora, model: "w_so210", socket: None }), &mut q);
        assert!(q.0.contains(&EffectCommand::Stop(kk_glow, true)), "{:?}", q.0);
        assert_eq!(spawned(&q).iter().filter(|s| s.1).map(|s| s.0).collect::<Vec<_>>(), ["vw_so210_app0#hl_glow"]);
        q.0.clear();
        // Goofy becomes the leader: on his shield, at his socket
        h.step(dt, true, Some(Carrier { who: PartyChara::Goofy, model: "w_gu070", socket: None }), &mut q);
        let glow = spawned(&q).into_iter().find(|s| s.1).unwrap();
        assert_eq!(glow.0, "vw_gof_app00#hl_glow");
        assert_eq!(glow.2, Attach::Party { member: PartyMember::GOOFY, bone: Some("R_wep"), offset: Vec3::ZERO });
        q.0.clear();
        // daylight: the glow stops softly, nothing new
        h.step(dt, false, Some(Carrier { who: PartyChara::Goofy, model: "w_gu070", socket: None }), &mut q);
        assert!(matches!(q.0.as_slice(), [EffectCommand::Stop(_, false)]), "{:?}", q.0);
        assert!(h.glow.is_none());
    }
}
