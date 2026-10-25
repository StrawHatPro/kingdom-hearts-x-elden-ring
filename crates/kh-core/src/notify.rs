//! The generic notify handlers: what a notify does by itself, whatever state is running
//! (docs/sora-animation-spec.md 9; the notify classes of the generated `NotifyClass`). The states
//! get the same events first (`PlayerState::on_notifies`).
//!
//! FIRST PASS, SEAM EVENTS ONLY: a sound, voice or effect notify becomes `Ctx::play_sound` /
//! `Ctx::spawn_effect` with the name the notify carries; a notify STATE starts a looping sound
//! or a persistent effect and stops it at its End. The audio and VFX implementers refine what
//! is emitted (alias sets, surface sounds, random voice draws, attach data).
//!
//! IDS: until the audio and effect tables are generated, a seam event of a notify has the id
//! `NOTIFY_ID_BASE + index of the asset name in the generated NOTIFY_ASSETS`; a name that is not
//! an asset (a weapon sound alias "SWING1", an automatic motion sound "Run") has `NOTIFY_ID_NONE`.

use glam::Vec3;

use crate::anim::{Notifies, NotifyData, NotifyPhase, NOTIFY_ASSETS};
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, SoundHandle, SoundId, SoundPlay};
use crate::pawn::Ctx;

/// First id of the notify assets (clear of a host's debug ids and of small table indices).
pub const NOTIFY_ID_BASE: u32 = 1_000_000;
/// The id of a notify name that is not an asset path.
pub const NOTIFY_ID_NONE: u32 = u32::MAX;

/// The seam id of a name a notify carries.
pub fn notify_asset_id(name: &str) -> u32 {
    NOTIFY_ASSETS.binary_search(&name).map_or(NOTIFY_ID_NONE, |i| NOTIFY_ID_BASE + i as u32)
}

/// What a notify state started, to stop at its End.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum NotifyHandle {
    Sound(SoundHandle, f32),
    Effect(EffectHandle),
}

fn at(socket: Option<&'static str>) -> Attach {
    Attach::Sora { bone: socket.filter(|s| !s.is_empty() && *s != "None"), offset: Vec3::ZERO }
}

/// (vfx implementer 2) Whether a row of an AttachEffect notify spawns while `current` is the
/// playing animation's effect group (docs/sora-vfx-spec.md 4.2): its GroupID is that group, or
/// -1 (any). -2 ("every current group") stands on AttachDataList rows that name an ead asset or
/// the weapon's common attach data; the asset's own rows then decide: a named asset passes here,
/// the common data (the Keyblade trail, whose row has group 0) only while the current group is 0.
pub fn effect_group_passes(group_id: i32, names_asset: bool, current: i32) -> bool {
    match group_id {
        -1 => true,
        -2 => names_asset || current == 0,
        g => g == current,
    }
}

/// Every event of this pose tick, in dispatch order.
pub(crate) fn dispatch(cx: &mut Ctx, notifies: &Notifies) {
    // (vfx implementer 2) THE EFFECT-GROUP RULE (docs/sora-vfx-spec.md 4.2, 7.4; attach component
    // `Set` 0x141770D30, spawn lambda 0x141762070 [C]). A play makes its effect group the only
    // current group; a row of an AttachEffect notify spawns when its GroupID is that group or -1
    // (the rows of the other keyblades' forms carry their form's group, 1823 .. 3410, and so never
    // spawn with the plain Keyblade, whose plays pass 0).
    let play = cx.pawn.anim.play_id;
    if play != cx.pawn.effect_group_play {
        cx.pawn.effect_group_play = play;
        cx.pawn.effect_group = cx.pawn.next_effect_group.take().unwrap_or(0);
    }
    let group = cx.pawn.effect_group;
    for event in &notifies.events {
        let def = event.def();
        let key = (event.sequence, event.index);
        match event.phase {
            NotifyPhase::Tick => {}
            NotifyPhase::End => {
                // Everything the state's Begin started.
                let mut k = 0;
                while k < cx.pawn.notify_handles.len() {
                    if (cx.pawn.notify_handles[k].0, cx.pawn.notify_handles[k].1) == key {
                        match cx.pawn.notify_handles.remove(k).2 {
                            NotifyHandle::Sound(handle, fade_out) => {
                                cx.stop_sound(handle, fade_out);
                                // (audio implementer) `SQEXSEADAnimNotifyState_AttachSound` End
                                // (0x1458162E0): after the Stop / FadeOut, `LoopEndSound` if set,
                                // at the same attach point (audio spec 5.1).
                                if let NotifyData::Sound { loop_end_sound: Some(end), attach_point, volume, loop_end_pitch, .. } = def.data {
                                    cx.play_sound(SoundPlay { id: SoundId(notify_asset_id(end)), name: end, at: at(attach_point), volume, pitch: loop_end_pitch, looping: false, spatial: true });
                                }
                            }
                            NotifyHandle::Effect(handle) => cx.stop_effect(handle, false),
                        }
                    } else {
                        k += 1;
                    }
                }
            }
            NotifyPhase::Instant | NotifyPhase::Begin => {
                let state = def.state;
                let sound = |cx: &mut Ctx, name: &'static str, socket: Option<&'static str>, volume: f32, pitch: f32, fade_out: f32| {
                    if name.is_empty() {
                        return;
                    }
                    let handle = cx.play_sound(SoundPlay { id: SoundId(notify_asset_id(name)), name, at: at(socket), volume, pitch, looping: state, spatial: true });
                    if state {
                        cx.pawn.notify_handles.push((key.0, key.1, NotifyHandle::Sound(handle, fade_out)));
                    }
                };
                match def.data {
                    NotifyData::Sound { sound: name, attach_point, volume, pitch, fade_out, .. } => sound(cx, name, attach_point, volume, pitch, fade_out),
                    // The alias (SWING1.., HIT1..) of the equipped weapon's sound set.
                    // (audio implementer) 0x1405615F0 (audio spec 5.1): attached at `m_SocketName`
                    // when `m_bFollow` (no socket = the mesh), else AT THE MESH LOCATION of this
                    // moment (a fixed point: it does not follow him).
                    NotifyData::WeaponSe { alias, socket, follow, volume, pitch, fade_out, .. } => {
                        if follow {
                            sound(cx, alias, socket, volume, pitch, fade_out)
                        } else if !alias.is_empty() {
                            let handle = cx.play_sound(SoundPlay { id: SoundId(notify_asset_id(alias)), name: alias, at: Attach::World(cx.pawn.cmc.location), volume, pitch, looping: state, spatial: true });
                            if state {
                                cx.pawn.notify_handles.push((key.0, key.1, NotifyHandle::Sound(handle, fade_out)));
                            }
                        }
                    }
                    // TODO(audio): the surface-dependent choice; the motion's name is passed.
                    NotifyData::AutoSe { motion, source, fade_out } => sound(cx, motion.or(source).unwrap_or(""), None, 1.0, 1.0, fade_out),
                    // TODO(audio): the weighted random draw; the group, the single asset or the
                    // FIRST entry of the random list is passed.
                    NotifyData::Voice { group, asset, random, fade_out } => sound(cx, asset.or(group).or(random.first().map(|r| r.0)).unwrap_or(""), Some("atama"), 1.0, 1.0, fade_out),
                    NotifyData::Effect { entries } => {
                        for entry in entries {
                            // (vfx implementer 2) the group rule. -2 on an AttachDataList row asks
                            // for "every current group" of the ead asset it names: such a row
                            // passes here and the asset's own rows decide (the Keyblade trail's
                            // row has group 0: only while the current group is 0, spec 6).
                            if !effect_group_passes(entry.group_id, entry.effect.is_some(), group) {
                                continue;
                            }
                            // (vfx implementer) An entry of common attach data on the weapon
                            // (bUseCommonAttachData, ATTACH_TO_WEAPON at R_wep_skt) of a state
                            // notify is the Keyblade trail: the weapon's designated data
                            // ead_key_trail0 -> common effect "key_trail0" = VW_SO010_LOC0
                            // (docs/sora-vfx-spec.md 6). The host samples the blade sockets
                            // vfx_top / vfx_bottom while it runs and fades it out at its stop.
                            // Other entries without an asset (area effects) emit nothing.
                            let effect = match entry.effect {
                                Some(effect) => effect,
                                None if state && entry.common && entry.attach_type.ends_with("ATTACH_TO_WEAPON") => "key_trail0",
                                None => continue,
                            };
                            let handle = cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(effect)), name: effect, at: at(Some(entry.socket)), rotation: Vec3::ZERO, scale: 1.0, persistent: state });
                            if state {
                                cx.pawn.notify_handles.push((key.0, key.1, NotifyHandle::Effect(handle)));
                            }
                        }
                    }
                    _ => {
                        // `TresAnimNotify_SetWeaponVisible`: the one instance serializes no
                        // property; the class default is not read [?]: "visible" is assumed.
                        if def.class.game_name() == "TresAnimNotify_SetWeaponVisible" {
                            let visible = !def.props.iter().any(|(_, value)| *value == "false");
                            cx.set_weapon_visible(visible);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::effect_group_passes;

    /// (vfx implementer 2) The effect-group rule with the groups of the generated clips.
    #[test]
    fn rows_spawn_only_for_the_playing_animations_group() {
        // The plain Keyblade plays with group 0: its own rows and the "any group" rows spawn ...
        assert!(effect_group_passes(0, true, 0));
        assert!(effect_group_passes(-1, true, 0));
        // ... the rows of the other keyblades' forms (1900 Classic Tone, 2100 Oathkeeper ...) do not.
        for form_group in [1900, 2100, 2200, 2360, 3400, 1605, 2156] {
            assert!(!effect_group_passes(form_group, true, 0), "{form_group}");
        }
        // A play with its own group (WallKick 0x139) takes the rows of that group and drops group 0.
        assert!(effect_group_passes(0x139, true, 0x139));
        assert!(!effect_group_passes(0, true, 0x139));
        assert!(effect_group_passes(-1, true, 0x139));
        // The Keyblade trail (common attach data, request -2, its row group 0): only in group 0.
        assert!(effect_group_passes(-2, false, 0));
        assert!(!effect_group_passes(-2, false, 0x139));
        assert!(effect_group_passes(-2, true, 0x139));
    }
}
