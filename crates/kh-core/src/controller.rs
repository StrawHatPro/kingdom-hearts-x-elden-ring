//! The controller's input update (`TresPlayerControllerSora`, the Sora input update
//! 0x1413F1AB0): docs/sora-movement-spec.md revision 2, sections 2.1 - 2.4. It polls the stick
//! and the buttons and calls the request interface (`TryRun`, `JumpRequest` ...: the functions
//! live with the state they request, `states/run.rs`, `states/jump.rs`).
//!
//! OWNERSHIP: shared, one call per request. The ORDER of the calls is the game's (spec 2.4) and
//! decides which request wins a frame: an implementer who adds a request (attack, guard, magic
//! shortcut ...) adds its call at the place its spec gives, citing the call's address, and
//! nothing else here. Buttons other than jump come from `input.pad` (input.rs).

use crate::pawn::Ctx;
use crate::params::SoraParams;
use crate::sora::{wrap_degrees, Input};
use crate::input::PadButton;
use crate::states::{floating, dangle, hopping, pole_turn, rail_slide, slope_slide, swaying, wall_run, risk_dodge, super_slide, wall_kick, air_slide, attack, damage, dodge_roll, glide, guard, jump, run, StateData};

/// Stock `UPlayerInput::MassageAxisInput` (0x1445AF120) with DefaultInput.ini's dead zone 0.25,
/// exponent 1, sensitivity 1: per axis, no clamp (2.1).
fn dead_zone(p: &SoraParams, v: f32) -> f32 {
    let dz = p.input_axis_dead_zone;
    if v > 0.0 {
        (v - dz).max(0.0) / (1.0 - dz)
    } else {
        -(-v - dz).max(0.0) / (1.0 - dz)
    }
}

/// Phase U of the frame (after the state tick): the HUD command window's decide on the Attack
/// row (0x141622513 -> `CanExecuteCommand` -> executor 0x14141B470 -> `TryAttack(current
/// target, 0)`; combat strand A 1.1). The command menu itself is the UI implementer's: until it
/// exists the cursor is taken to be on the Attack row and the row enabled.
pub(crate) fn ui_attack_request(state: &mut StateData, s: &mut Ctx, input: &Input, dt: f32) {
    // (party stream D) The friend gauge (slot 900), the friends' rows and offers; while an FR
    // state runs the command menu and the situation queue wait (combat spec 11.5).
    if crate::combat::friend::tick(state, s, dt) {
        return;
    }
    // (party stream D3) A unison rolled on a player-controlled friend leader's combo
    // (FriendState::roll_leader_unison): Sora starts it as his attack while the latch lasts.
    if s.pawn.combat.friend.leader_unison > 0.0 {
        s.pawn.combat.friend.leader_unison -= dt;
        if s.pawn.combat.friend.leader_unison <= 0.0 {
            s.pawn.combat.friend.unison = 0;
        } else if state.attack_mut().is_none() {
            let target = s.pawn.combat.lockon.current;
            crate::states::attack::try_attack(state, s, target);
        }
    }
    // The HUD part's tick first: the situation-command timers, L2 and Triangle (0x141630AF0,
    // 0x14162BAF0, 0x1416220D0; combat spec 11.4, 11.5).
    crate::combat::situation::tick(state, s, input, dt);
    // (magic implementer) The command system owns the UI phase: the menu's state machine reads
    // the pad; with the cursor on Attack the decide press is `ExecuteCommand(27)` ->
    // `TryAttack(current target, 0)` as before (command/mod.rs).
    crate::command::ui_tick(state, s, input);
}

/// The Sora input update, 0x1413F1AB0 (spec 2.3, 2.4).
pub(crate) fn controller_update(state: &mut StateData, s: &mut Ctx, input: &Input, dt: f32) {
    let p = *s.params();
    let stick = if input.move_stick.is_finite() { input.move_stick } else { glam::Vec2::ZERO };
    // V = (X = LeftY, Y = LeftX, 0); mag not clamped (controller +0x854).
    let (forward, right) = (dead_zone(&p, stick.y), dead_zone(&p, stick.x));
    let magnitude = (forward * forward + right * right).sqrt();
    let (previous_magnitude, previous_yaw) = (s.stick_magnitude, s.stick_yaw);
    s.stick_magnitude = magnitude;
    s.stick_yaw = wrap_degrees(input.camera_yaw + right.atan2(forward).to_degrees());
    // (combat implementer, 2026-10-12) The stick flick (controller+0x878 bit 5 -> request interface
    // slot 37 0x141440570; combat spec 9.9.2): in the stun state each flick shortens the stun.
    crate::combat::badstat::stick_flick(state, previous_magnitude, magnitude, previous_yaw, s.stick_yaw);
    // Combat strand D 7.3 (0x1413F241F - 0x1413F2468): the RB / RT tap timers and `UpdateLockon`
    // run before every request of the frame, so an attack pressed on the frame an enemy comes
    // into range already has it as its target.
    crate::combat::targeting::update(s, dt);
    // The formchange gauge's tick (controller slot 829; combat strand F 10.2).
    crate::combat::style::tick(state, s, dt);
    // (XSI) The XS abilities' per-step part (abilities/xs: Sonic Dash's airtime, the Ancient Light offer).
    crate::abilities::xs::tick(state, s, input, dt);
    // The pawn's cool-downs (pawn tick 0x141449CD0; strand A 1.3).
    {
        let combat = &mut s.pawn.combat;
        combat.command_cooldown = (combat.command_cooldown - dt).max(0.0);
        combat.air_slide_cooldown = (combat.air_slide_cooldown - dt).max(0.0);
        combat.air_guard_cooldown = (combat.air_guard_cooldown - dt).max(0.0);
        // controller+0x8F4 (0x1413F80F0) and pawn+0x638 (0x1405BC470): combat spec 9.6.
        combat.reaction_block = (combat.reaction_block - dt).max(0.0);
        combat.no_damage_timer = (combat.no_damage_timer - dt).max(0.0);
    }
    // The action states' part of the same ticks (actions.rs): the availability bits return, the
    // battle flag is mirrored from the host (controller slot 630), the counter window counts down.
    crate::actions::pawn_tick(s, dt);
    // (magic implementer) The command timers (controller slot 635), then slot 800: the shotlock
    // aim mode (magic spec F A1). While it is open the whole normal dispatch below is skipped;
    // only the airstep state still takes the jump press (it aborts the flight).
    crate::command::controller_tick(s, dt);
    let real_dt = s.real_dt;
    // (party stream D) An FR state takes the stick and the presses for its link pawn (Trinity
    // Guard 0x141377AC0); no other request is made while it runs.
    {
        let a = s.stick_yaw.to_radians();
        let stick = if magnitude < p.input_stick_active_min { glam::Vec3::ZERO } else { glam::Vec3::new(a.cos(), a.sin(), 0.0) * magnitude.min(1.0) };
        if crate::combat::friend::fr_input(state, s, input, stick) {
            return;
        }
    }
    if crate::magic::shotlock::update(state, s, input, dt, real_dt) {
        if input.jump_pressed {
            crate::states::athletic_flow::jump_request(state);
        }
        return;
    }
    // L1 held makes the face buttons shortcuts (0x1413F25E8: the jump press is dropped), and
    // the HUD masks the cancel press while a sub-list is open.
    let jump_pressed = input.jump_pressed && !input.pad.held.has(PadButton::LeftShoulder) && !s.command.masks_jump();
    // The stick's yaw for the requests below (the camera's yaw when the stick is centred).
    let stick_yaw = wrap_degrees(input.camera_yaw + right.atan2(forward).to_degrees());
    // Actions spec 2.1 (0x1413F1AB0), the GUARD button, before the jump button: flow trigger
    // (`TryFlow`, slot 72: tier 2) -> `TryAirSlide` (slot 69) -> stick inactive: `TryGuard` (slot
    // 55), else `TryDodge` (slot 58). With the stick active the button means dodge only: a
    // refused dodge does not fall back to guard. A request that is taken returns: the stick is
    // not forwarded on that frame.
    // (magic implementer: with L1 held the face buttons are shortcuts; the controller drops the
    // guard, triangle and jump presses: 0x1413F25E8, combat strand A 1.2.)
    if input.pad.pressed.has(PadButton::GUARD) && !input.pad.held.has(PadButton::LeftShoulder) {
        // `TryFlow(1, 0)` (slot 72): the Superslide out of a flow jump / the wall kick's latch.
        if super_slide::try_flow(state, s) {
            return;
        }
        // (XSI) SONIC DASH (XS 2.2): with a lock 4 - 15 m away and the stick toward it, the press is the dash.
        if crate::abilities::xs::sonic::try_sonic_dash(state, s, stick_yaw, magnitude) {
            return;
        }
        if air_slide::try_air_slide(state, s, stick_yaw, magnitude) {
            return;
        }
        if magnitude < p.input_stick_active_min {
            if guard::try_guard(state, s) {
                return;
            }
        } else if dodge_roll::try_dodge(state, s, stick_yaw, magnitude) {
            return;
        }
    }
    // (The NORMAL attack press is not issued here: the HUD command menu makes the request in
    // the UI tick, after the state tick: `ui_attack_request` below; combat strand A 1.1.)
    // 2.4, in this order (change D4):
    // 1. jump key just pressed -> JumpRequest(0) (0x1413F280B; result ignored, no buffering);
    // 2. else, key down -> JumpHeld() (0x1413F283F): not on the press frame.
    if jump_pressed {
        // Actions spec 2.1 / II.1: `TryRiskDodge(yaw, mag, target, type)` (slot 61 0x14143FF10)
        // first: out of a Reflect whose risk-dodge window is open.
        if risk_dodge::try_risk_dodge(state, s) {
            return;
        }
        // Strand A 1.2 (0x1413F27F5): the jump press during a combo is the launch attack
        // (`TryAttack(target, 1)`), tried before `JumpRequest`.
        let target = s.combat.lockon.current;
        if attack::try_launch(state, s, target) {
            return;
        }
        jump::jump_request(state, s);
    } else if input.jump_held {
        // Actions spec 2.1: the key is down and was not pressed this frame: `TryGlide` (slot 67
        // 0x14143C5B0) first; a glide returns from the input update.
        if glide::try_glide(state, s, stick_yaw, magnitude) {
            return;
        }
        jump::jump_held(state);
    }
    // 3. stick active (!(mag < 0.1), 0x1413F206D): MoveInput, then TryRun.
    if !(magnitude < p.input_stick_active_min) {
        // moveYaw = cameraYaw + atan2(LeftX, LeftY) with the camera's roll 0 (2.3). The camera
        // yaw is the previous frame's (the host passes it so). The engine's polynomial Atan2 is
        // within 1e-4 degrees of this one (change E7).
        let yaw = wrap_degrees(input.camera_yaw + right.atan2(forward).to_degrees());
        // MoveInput (slot 35 0x141440600): only when the current state is JUMP (any sub-step).
        jump::move_input(state, yaw, magnitude);
        // ... AIR_SLIDE (0x1414406A7): the stick flag, yaw and magnitude of the exit test.
        air_slide::move_input(state, yaw, magnitude);
        // ... SUPER_SLIDE (0x141440A15: the stick steers) and WALL_KICK (0x141440D03).
        super_slide::move_input(state, yaw);
        wall_kick::move_input(state, yaw, magnitude);
        // ... and DAMAGE: the stick is active this frame (0x141440B9E; combat strand E 9.5).
        damage::move_input(state);
        // ... and the traversal states of tier 2 (actions spec II.4 - II.6: WALLRUN 0x1414408ED,
        // POLE_TURN 0x141440D7B, RAIL_SLIDE, DANGLE 0x1414406D0 (only above 0.5), SLOPESLIDE,
        // SWAYING, HOPPING).
        wall_run::move_input(state, yaw);
        pole_turn::move_input(state, yaw);
        rail_slide::move_input(state, yaw);
        dangle::move_input(state, yaw, magnitude);
        slope_slide::move_input(state, yaw);
        swaying::move_input(state, yaw);
        hopping::move_input(state, yaw, magnitude);
        // ... and FLOATING (0x141441099).
        floating::move_input(state, yaw, magnitude);
        run::try_run(state, s, yaw, magnitude);
    }
}
