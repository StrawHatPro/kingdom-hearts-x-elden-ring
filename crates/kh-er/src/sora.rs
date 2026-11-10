//! Sora's simulation hosted in the Elden Ring frame (`mode = sora`).
//!
//! `ChrIns_PrePhysics`: read the pad, run 0..n fixed 60 Hz steps of `kh_core::Sora` against the
//! Havok world, evaluate the pose of each step, write the latest step's position and facing to
//! the (passive, hidden) Elden Ring player. `ChrIns_PostPhysics`: re-assert the switches and the
//! position, draw the skeleton, the capsule and the facing with the game's debug drawer.
//!
//! THE CAMERA: Kingdom Hearts III's (cam.rs: kh-camera stepped after each of Sora's steps, shown
//! through the game's camera) while Sora is in control; Elden Ring's own otherwise (`cam.kh = 0`:
//! always). The camera yaw of Sora's input is the yaw of the camera that has the view.

use std::io::Write;
use std::path::{Path, PathBuf};

use eldenring::cs::PlayerIns;
use glam::{Quat, Vec2, Vec3};
use kh_anim::{Animator, BodySlot, Frame as PoseFrame, Settings, Trs};
use kh_core::states::HitOnPlayer;
use kh_core::{EventKind, Host, Input, Output, PadButton, PadButtons, PadState, Pose, Sora, Status, TargetId, SORA_PARAMS};

use crate::control::{Control, OneShot};
use crate::ermap;
use crate::inputs;
use crate::targets::{self, ErTargets};
use crate::coords::{self, Frame};
use crate::erlog;
use crate::game::{self, Draw};
use crate::pad;
use crate::world::ErWorld;

/// The body's skeleton retargeted for the overlay: (source pose, pelvis, weight).
type OwnPose = (Vec<(Vec3, Quat)>, Vec3, f32);
/// (IMPL_ER16) The component place of a pose step and the overlay on it.
type PoseOver<'a> = (Trs, &'a mut dyn FnMut(&kh_anim::Skeleton, &mut [Trs]));

const STEP: f32 = 1.0 / 60.0;
/// The body capsule kh-core sweeps (kh-core/src/world.rs), cm: drawn only.
const CAPSULE_RADIUS: f32 = 60.0;
const CAPSULE_HALF_HEIGHT: f32 = 89.0;

/// The Keyblade's appear / disappear sequence, as kh-app/src/player.rs has it (docs/sora-vfx-spec.md
/// 2.4, the Kingdom Key's values): 0.22 s after the core's `WeaponVisible` the mesh appears and
/// its `OpacityFade` ramps to 1 over 0.06 s; hiding is the mirror.
#[derive(Clone, Copy, Debug)]
struct WeaponShow {
    visible: bool,
    since: f32,
}

impl WeaponShow {
    const DELAY: f32 = 0.22;
    const RAMP: f32 = 0.06;

    fn opacity(&self) -> f32 {
        let ramp = ((self.since - Self::DELAY) / Self::RAMP).clamp(0.0, 1.0);
        if self.visible {
            ramp
        } else {
            1.0 - ramp
        }
    }
}

/// The socket R_wep_skt relative to bone R_buki (UE rotator roll -90; the weapon manifest's `attach`).
fn weapon_socket() -> Trs {
    let half = std::f32::consts::FRAC_1_SQRT_2;
    Trs { translation: Vec3::ZERO, rotation: Quat::from_xyzw(half, 0.0, 0.0, half), scale: Vec3::ONE }
}

mod lab;
pub(crate) mod kbdraw; // (P5 WEAPON DRAW) the equipped keyblade: renderer, keychain, sounds, trail, switch timing

/// (IMPL_ER23A) We hold the game's no-death switch for its player (`SoraHost::fall_switches`).
static NO_DEAD_HELD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (IMPL_ER23A) No Sora runs (the player is gone: a load, the title; the game's own death): the game's no-death switch is
/// never left on behind him (its own value is off: a retail game has no debug menu to set it).
pub fn no_dead_release() {
    use fromsoftware_shared::FromStatic;
    if NO_DEAD_HELD.swap(false, std::sync::atomic::Ordering::Relaxed) {
        // SAFETY: game thread; the SDK resolves the static by its pinned RVA.
        if let Ok(flags) = unsafe { eldenring::cs::WorldChrManDbgFlags::instance_mut() } {
            flags.player_no_dead = false;
        }
    }
}

/// (DTH) The death flow's numbers (death.rs `Rules`): `death.ko_show` / `death.fade` / `death.wait` seconds,
/// `death.scripted_hp`, `death.scripted = 0` (the old rule: HP 0 at once at a scripted death's place).
fn death_rules(control: &Control) -> crate::death::Rules {
    let d = crate::death::Rules::DEFAULT;
    crate::death::Rules {
        ko_show: control.f32("death.ko_show").filter(|v| *v >= 0.0).unwrap_or(d.ko_show),
        fade: control.f32("death.fade").filter(|v| *v >= 0.0).unwrap_or(d.fade),
        wait: control.f32("death.wait").filter(|v| *v > 0.0).unwrap_or(d.wait),
        hp: control.i64("death.scripted_hp").filter(|v| *v >= 1).map_or(d.hp, |v| v as i32),
        scripted: control.flag("death.scripted") != Some(false),
    }
}

/// (DTH) A scripted death's scene plays (the game's cutscene manager, `cut.states` as `follow_phase` reads it).
fn scene_plays(control: &Control) -> bool {
    let remo = game::remo_state();
    match control.str("cut.states") {
        Some(list) => remo.is_some_and(|s| list.split(',').any(|p| p.trim().parse::<i32>() == Ok(s))),
        None => remo.is_some_and(|s| CUTSCENE_STATES.contains(&s)),
    }
}

/// (IMPL_ER10) Why Sora follows the body (`SoraHost::follow_phase`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Follow {
    Cutscene,
    /// (IMPL_ER17) The body performs one of the game's own actions (an item picked up, a door, a ladder: the ids of the
    /// sheet rows `own.anim.*`): the game moves and animates it, Sora is seated on it and shows its pose.
    Action,
    /// (IMPL_ER57) AN ENEMY'S GRAB HOLDS THE BODY (its throw node: the player is the one thrown): the game moves and animates
    /// it with the attacker's paired animation; Sora is seated on it, shows its pose (retargeted), takes no input, and the
    /// pad is kept from both (nothing of the game's input means anything in a throw).
    Grab,
    NoGround { since: f32 },
}

/// (IMPL_ER16) What a real-time cutscene makes of Sora's drawing this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CutDraw {
    /// As in play (no cutscene, or `cut.sora = double`).
    Normal,
    /// Not drawn (the scene's cut does not show the player, or `cut.sora = hide | game`).
    Hidden,
    /// The scene's figure, retargeted: his component stands here.
    Posed(Trs),
}

/// The states of the game's cutscene manager in which a real-time cutscene plays (MEASURED: IMPL_ER10_STATE.md).
const CUTSCENE_STATES: [i32; 6] = [2, 3, 4, 5, 6, 7];

pub struct SoraHost {
    /// The test bench for enemy animations (sora/lab.rs; keys `dbg.*`, closed by real pad input).
    lab: lab::Lab,
    sora: Sora,
    out: Output,
    /// The step before `out` and its pose (the drawn frame may blend the two: `r.interp`).
    out_before: Output,
    pose_before: kh_anim::Pose,
    weapon: WeaponShow,
    /// (F-SHIP 3, INT-F) Highwind's spears in flight: (shot key, its life last seen, the free effect of its body particle).
    spears: Vec<(u16, f32, u64)>,
    /// (IMPL_ER44) The Keyblade light: Sora holds the Keyblade out while the hero light's darkness gate is closed and he leads;
    /// `keyblade_held`: for the log. (HEROLIGHT-KB) its glow and sparkles on the leader's weapon, whatever it is (herolight.rs).
    keyblade_held: bool,
    herolight: crate::herolight::HeroLight,
    /// (HEROLIGHT-KB final pass) the left-hand weapon of a dual-wield form: its own glow and sparkles
    herolight_left: crate::herolight::HeroLight,
    /// (FORMWEP-FIX) the hand Keyblade between a form's start and its transformation (kh-host formstart.rs)
    form_gate: kh_host::formstart::FormStartGate,
    /// (FREEZE-FX) KH3's frozen look on every frozen Elden Ring enemy (freezefx.rs)
    freeze_fx: crate::freezefx::FreezeFx,
    /// The Keyblade's keychain (its own Bonamik instance) and its poses of the two latest steps.
    keychain: Option<kh_anim::Keychain>,
    chain: Vec<Trs>,
    chain_before: Vec<Trs>,
    /// (INT) the drawn history of his own that is not in `out_before` / `pose_before` (interp.rs)
    int: crate::interp::Track,
    /// (INT) his place, facing and bones AS DRAWN this frame (`submit`), for the effects on him (fx.rs `draw_drawn`)
    int_drawn: std::cell::RefCell<Option<(Output, kh_host::audio::PoseTap)>>,
    weapon_shown: bool,
    clock: f32,
    /// (fx) Sora's sound and effects in this host (fx.rs): the core's audio and effect seams.
    fx: crate::fx::Fx,
    /// (prize) KH3's prize orbs from killed enemies (`prize.kh`) and the munny mirror (`munny.mirror`): prize.rs. Both OFF by default.
    prize: crate::prize::ErPrizes,
    /// (loot) Elden Ring's loot as KH3 items (`loot.kh`, `loot.gate`, `loot.log`): loot.rs. All OFF by default.
    loot: crate::loot::ErLoot,
    /// (ENG) Elden Ring's team attack limiter set to `enemy.max_attackers` (5) at once; the game's values back on stop / drop.
    attackers: crate::attackers::ErAttackers,
    /// (IMPL_ER31) Bosses give magic, abilities, slots, max HP / MP, small bosses an item (`boss.rewards`): bossreward.rs.
    boss_rewards: crate::bossreward::ErBossRewards,
    weapon_src: crate::weaponsrc::ErWeaponSources, // (P9b) the rare weapon drops of real deaths
    /// (savepoint) KH3 save points at the sites of grace in reach (`savepoint.kh`): savepoint.rs.
    savepoint: crate::savepoint::ErSavePoints,
    /// (hud) Sora's HUD and menus in this host (hud.rs): the core's UI sink, the menus' pad input.
    hud: crate::hud::HudHost,
    world: ErWorld,
    /// KH3's camera (cam.rs) and the step-exact input script (script.rs; testing aid).
    cam: crate::cam::KhCam,
    /// (IMPL_ER22) KH3's boss finish (finish.rs; `finish.kh`).
    finish: crate::finish::ErFinish,
    worldtime: crate::worldtime::ErWorldTime, // (AUDIT-A WORLDTIME) KH3's world slow / hit-stops
    script: crate::script::InScript,
    script_clock: f32,
    script_at: std::time::Instant,
    script_now: Option<crate::script::Seg>,
    held_before_r3: bool,
    animator: Option<Animator>,
    pose: kh_anim::Pose,
    reader: pad::Reader,
    accumulator: f32,
    held_before: PadButtons,
    /// The value of `in.jump` last seen and the steps the scripted jump is still held for.
    scripted_jump: (i64, u32),
    /// What we last wrote to the player (Elden Ring space).
    written: Option<Vec3>,
    trace: Option<std::io::BufWriter<std::fs::File>>,
    // Per-second numbers for the log.
    steps_in_tick: u32,
    casts_in_tick: u32,
    pushed: f32,
    pad_seen: bool,
    /// Frames of this second in which the REAL pad was touched (somebody is playing).
    pad_active: u32,
    /// The value of `sora.warp` that was last carried out (outer `None`: not looked at yet; a value
    /// that is there when Sora starts is NOT carried out: after a death the place may be gone).
    warped: Option<Option<String>>,
    /// The value of `w.scan` that was last carried out.
    scanned: Option<Option<String>>,
    proxied: Option<Option<String>>,
    fscanned: Option<Option<String>>,
    /// (IMPL_ER18) Where (KH3) the body's own action left him, the time since, whether the stick moved him since.
    action_end: Option<(Vec3, f32, bool)>,
    /// (IMPL_ER18) The dynamic body he stands on and the height of its surface under him last frame; how far it carried him.
    ride: Option<(u32, f32)>,
    rode: f32,
    rode_frames: u32,
    rode_still: u32,
    ladder_seen: i8,
    /// (LADDER2) THE LADDER JUMP (ladder.rs): the game's ladder starts on the body, the game's let-go of it, the landing
    /// waiting for the let-go (KH3 floor point, heading, seconds waited), Sora's leap (leader or AI Sora) and what his body
    /// slot shows meanwhile (KH3's own jump clips); a chest opening (chest.rs, `chest.kh`)
    ladder: crate::ladder::Watch,
    ladder_go: crate::ladder::LetGo,
    /// (LADDER2) the game's "Descend" prompt was up last frame (the log's edge)
    descend_told: bool,
    /// (TELESCOPE) a Birdseye Telescope's prompt was up last frame (the log's edge)
    telescope_told: bool,
    /// (SUMMON-POOL) a summoning pool's prompt was taken away (said once per sighting)
    pool_told: bool,
    leap_wait: Option<(Vec3, f32, f32)>,
    /// (SPIRITSPRING) the leader's jump at a spiritspring (spiritspring.rs)
    spring: crate::spiritspring::rules::Watch,
    leap: Option<crate::ladder::SoraLeap>,
    leap_show: crate::ladder::Show,
    chest: crate::chest::Chest,
    /// (CHESTFIX) the net on his own simulation (exitnet.rs `StillNet`), kh-core's closes of a dropped aim mode and its
    /// form nets as last logged
    still_net: crate::exitnet::StillNet,
    aim_closed_seen: u32,
    form_nets_seen: u32,
    /// (ANIMS) the ladder state as believed (a stale module after a let-go: -1), the net, the actions' cap
    ladder_truth: crate::exitnet::LadderTruth,
    busy_net: crate::exitnet::BusyNet,
    own_cap: crate::exitnet::OwnCap,
    ladder_fix: crate::exitnet::Fix,
    /// (ANIMS) the borrowed actions at his proportions (propfit.rs): the frame's dt, the fit, its plan, the measures
    /// (FLVER, the Tarnished's, his), the action touches something in front (a door, the fog, a chest)
    anims_dt: f32,
    own_fit: crate::propfit::Fit,
    own_plan: Option<crate::propfit::Plan>,
    own_dims: Option<(usize, crate::propfit::Dims, crate::propfit::Dims)>,
    own_surface: bool,
    /// (ANIMS) the log's edges: the friend leader's walk through an action, a borrowed pose refused by the sanity check
    #[cfg(feature = "party")]
    lead_walk_told: bool,
    #[cfg(feature = "party")]
    lead_insane_told: bool,
    worn: crate::limbs::Worn,
    /// (IMPL_ER38) the map's requests and outcomes, the game's window focus, the View guard (mapwatch.rs)
    map_watch: crate::mapwatch::MapWatch,
    work: std::time::Duration,
    // ---- the gameplay hookup (IMPL_ER4_STATE.md)
    /// Elden Ring's characters as Sora's targets.
    targets: ErTargets,
    rebased_now: bool,
    /// (WALLS load fix 2026-10-23) the game moved its body during the physics step by a jump that is not an origin shift (its own
    /// placement after a load / warp): adopted in `post_physics`, handled as a relocation at the next `pre_physics`
    relocated_post: bool,
    /// (WALLS load fix) seconds left of the guard after a release from the wait for the ground (groundwait.rs `GUARD_S`)
    release_guard: f32,
    /// (WALLS cutscene fix) the wait for the ground began at a cutscene's end, a load or a warp: its release snaps the camera
    cam_snap_due: bool,
    /// (IMPL_ER10) SORA FOLLOWS THE BODY instead of driving it (a cutscene; the wait for the ground after the game
    /// put its player somewhere else): seated on the body every frame, standing on a level floor, without input.
    follow: Option<Follow>,
    /// (ZETTA 2026-10-23) the wait for the ground after the game moved its player (groundwait.rs)
    ground_wait: crate::groundwait::Wait,
    /// This host has not run a frame yet (it starts with the wait for the ground).
    fresh: bool,
    /// (LDI) The load-in gate (loadin.rs): the party held under the loading screen, placed when it lifts; the window in
    /// which what appears appears settled (the save points).
    loadin: crate::loadin::LoadIn,
    /// Where he last stood on the ground or was seated by the game (KH3 space): the place the net brings him back to.
    anchor: Option<Vec3>,
    /// Seconds without any ground under him (the net), and the clock of its casts.
    void_for: f32,
    void_check: f32,
    /// The game's cutscene state as last logged; the body's yaw while he follows.
    remo: Option<i32>,
    follow_yaw: Option<f32>,
    cutscene: bool,
    /// (IMPL_ER16) In a real-time cutscene: what is drawn of him (the scene's figure of the player shown as Sora).
    cut_draw: CutDraw,
    cut_at: Option<std::time::Instant>,
    /// (IMPL_ER16) IN PLAY, while the game animates its hidden body with an action Sora has none for (a pick-up, a door,
    /// a lever: its event animations): the body's skeleton, retargeted, is laid over his pose (weight eased over 0.2 s).
    own_weight: f32,
    own_rig: Option<crate::figure::Rig>,
    own_over: Option<OwnPose>,
    own_anim: i32,
    own_reach: f32,
    /// (IMPL_ER17) The fall net's last put-back: where to and when.
    net_last: Option<(Vec3, std::time::Instant)>,
    own_hold: bool,
    cut_lying: Option<bool>,
    /// (CUTSCENE-ANIM) what the figure and Sora rest on (cutfit.rs parts), as last logged
    cut_rests: Option<(usize, usize)>,
    own_seen: i32,
    /// (testing aid) `dbg.kh = x,y,z` (KH3 cm; one-shot): he is put at that KH3 position (a recorded run's place).
    kh_shot: OneShot,
    /// The Tarnished's HP as we left it: a lower value at the next frame is a hit by the game.
    hp_written: Option<i32>,
    /// THE ONE-SHOT RULE: the Tarnished's own maximum HP (as the game computes it) and the raised
    /// maximum we hold him at, so that no single blow of the game's can kill the hidden body:
    /// Sora's HP alone decides death. Shares of HP are taken of his OWN maximum.
    body: Option<Body>,
    /// `dbg.travel`: whom Sora is being taken to, a hop a second.
    travel: Option<(String, f32)>,
    /// `dbg.hop`: the hop still to make (Elden Ring metres), how many, seconds to the next.
    hop: Option<(Vec3, u32, f32)>,
    /// A target whose animation / HP changes are logged for a while (`dbg.atk`, `dbg.watch`).
    watch: Option<Watch>,
    /// Sora died (the core's event) / the Tarnished's HP was set to 0 for it.
    /// (DTH) The leader is down, whoever leads (death.rs `Flow::observe`).
    died: bool,
    death_sent: bool,
    /// (DTH) ONE DEATH FOR WHOEVER LEADS (death.rs): the leader's KO, the scripted death's fade and the game's own scene,
    /// the game over; `sora_fell`: Sora's own death event of this frame (he leads); `frame_dt`: this frame's time.
    flow: crate::death::Flow<kh_host::party::PartyChara>,
    sora_fell: bool,
    frame_dt: f32,
    /// The target Elden Ring's own lock-on was put on / the one it would not take (logged once).
    er_locked: Option<TargetId>,
    lock_missing: Option<TargetId>,
    /// Elden Ring's interface this frame; its element states of the frame before (`dbg.ui`).
    ui: inputs::ErUi,
    ui_states: Option<[u8; 0x46]>,
    /// An interaction prompt of Elden Ring is on screen (Y is Elden Ring's then).
    prompt: bool,
    /// The world stands still (a tutorial popup): frames the player's animation clock has not moved.
    still: (f32, i32, u32),
    paused: bool,
    r3_before: bool,
    /// He is invulnerable to Elden Ring this frame (a dodge of his; the game's own no-damage flags).
    invulnerable: bool,
    shots: Shots,
    /// Per-second numbers for the log.
    hits_in: u32,
    invulnerable_frames: u32,
    key_frames: u32,
    /// (IMPL_ER14) the ground-combo investigation: what is counted between two attacks (`attack:` log lines).
    atk: AtkDiag,
    /// (IMPL_ER14) the Y press in progress and who has it.
    y: YPress,
    // ---- (IMPL_ER23A) guards.rs
    /// Buttons that are not his until released (a menu closed under them, a window of the game's took them).
    latch: crate::guards::PressLatch,
    /// A selection window of Elden Ring's that leaves its HUD state at gameplay; what was last read of its popup menu.
    dialog: crate::guards::DialogWatch,
    popup: Option<crate::guards::PopupState>,
    dialog_up: bool,
    /// The hidden body never dies of a fall: the window in which the game's HP losses are its fall damage; the game's
    /// own value of its no-death switch (put back when Sora stops), whether we hold it now.
    fall: crate::guards::FallGuard,
    /// (THE IN-GAME TESTER 2026-11-03) the hidden body's own fall (a warp that put it lower counts): HP the game takes with
    /// no attacker named in its window is that fall's, discarded (guards.rs `BodyFall`, `blame`)
    body_fall: crate::guards::BodyFall,
    no_dead_own: Option<bool>,
    no_dead_held: bool,
    death_anim_for: f32,
    /// Wedged off the floor: the detector and what it asked for this frame.
    stuck: crate::guards::Stuck,
    stuck_now: bool,
    /// The leg IK as last set on the animator.
    leg_ik: bool,
    /// Where and when he was last freed, how often in a row.
    stuck_last: Option<(Vec3, std::time::Instant, u32)>,
    latch_told: PadButtons,
    /// (IMPL_ER23A) fast travel only from a save point (travel.rs).
    fast_travel: crate::travel::Travel,
    /// (IMPL_ER24) Where he last STOOD and the height he left the floor at (guards.rs `Anchor`: the nets' places); the
    /// "pushing and not moving" detector; the two reports waiting for the end of the frame's steps; how many reports were
    /// written and when the last; the floor layer as last said in the log.
    stood: crate::guards::Anchor,
    blocked: crate::guards::Blocked,
    blocked_now: bool,
    floor_report: Option<(Output, Output)>,
    reports: (u32, Option<std::time::Instant>),
    floor_told: u32,
    /// (IMPL_ER57) the deep-water death (guards.rs `Drown`), the clock of its casts, the depth last said in the log (cm, in
    /// steps of a metre); an enemy's grab of the body (guards.rs `Grab`), the throw node's state last logged.
    drown: crate::guards::Drown,
    /// (DEATHPLANE) every KH character's doom by Elden Ring's death barriers (deathplane.rs; rebuilt with the host)
    barrier: crate::deathplane::Deathplane,
    water_check: f32,
    water_told: i32,
    grab: crate::guards::Grab,
    throw_seen: Option<u32>,
    /// (GRABS) KH3's caught sequence of the leader while an enemy's grab holds the body and while he gets up (grabs.rs)
    catch: Option<crate::grabs::Catch>,
    /// (GRABS) the grab's thrower: its handle bits, its MSB event entity and its map; the abduction it is, if any
    grabber: Option<(u64, u32, String)>,
    abduct: Option<crate::grabs::Abduction>,
    /// (IMPL_ER67) the body plays a pickup animation Sora runs through (for the log's edge).
    pickup_seen: bool,
    /// (ER-TWEAKS) the body opens a door / pulls a lever and the leader stays free (doorfree.rs; the log's edge).
    door_seen: bool,
    /// (ER-TWEAKS) the followed action of this frame walks the leader through (the fog: fogwalk.rs), his own walk shown;
    /// the walk, the carry's trail (KH3 cm, moved by the seat's shift while followed), the log's edge, a friend's walk drawn by it.
    walk_action: bool,
    fog_walk: crate::fogwalk::ActionWalk,
    fog_trail: Vec3,
    fog_walk_told: bool,
    #[cfg(feature = "party")]
    fog_drew: bool,
    /// (ER party S2) THE LEADER BODY (party/leader.rs): whoever leads (Sora, Donald, Goofy). S1's party glue feeds it every
    /// party step (`update` / `apply`); the body rules read `friend_lead` (None = Sora: every rule as before).
    #[cfg(feature = "party")]
    pub(crate) lead: crate::party::leader::LeaderBody,
    /// (ER party S1) THE PARTY HOST (party/mod.rs): Donald, Goofy and AI Sora, stepped in the contract's order around
    /// Sora's steps (`party.er`, default on); the core lives on across Sora hosts (loads, deaths, fast travel).
    #[cfg(feature = "party")]
    pub(crate) party: crate::party::PartySlot,
}

/// (IMPL_ER14) WHO HAS A PRESS OF Y, from its down-edge to its release (never split).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum YOwner {
    /// Elden Ring's prompt was up at the down-edge (or a menu of its had the pad): its action button.
    EldenRing,
    /// No prompt: Sora's situation command. `replayed`: a prompt came up within the window and the press was handed on.
    Sora,
    /// The KH3 save point's command (its menu opens): neither of the two sees it.
    SavePoint,
    /// (PFY) A menu of ours (the HUD: the save menu, Party Formation, the camp menu ...) had the pad at the down-edge: the
    /// press is the menu's (Party Formation's "Make Leader"), neither the save point's nor Elden Ring's nor Sora's.
    Menu,
}

impl YOwner {
    /// (PFY) WHO GETS A PRESS OF Y, at its down-edge. `menu_before`: a menu of ours (the HUD) had the pad before the save
    /// point looked at this frame's Y; `save_point`: the save point's `frame` says Y is its own; `prompt`: Elden Ring's
    /// action prompt is up; `er_menu`: a menu of Elden Ring's has the pad.
    /// THE USER'S BUG ("pressing Y on the party formation menu to change leader does nothing"): the save point's Y rule
    /// knows nothing of our menus, so with Sora still standing at the grace under the Party Formation screen it claimed
    /// every Y press and hid it from the HUD (A and B were never routed). A menu of ours that is up comes first now. The
    /// press that OPENS the save menu stays the save point's (the menu was not up before it), so the new menu never sees it.
    pub(crate) fn at_down(menu_before: bool, save_point: bool, prompt: bool, er_menu: bool) -> YOwner {
        if menu_before {
            YOwner::Menu
        } else if save_point {
            YOwner::SavePoint
        } else if prompt || er_menu {
            YOwner::EldenRing
        } else {
            YOwner::Sora
        }
    }

    /// (PFY) The press is taken out of the buttons Sora and the HUD's menus read.
    pub(crate) fn hides_y(self) -> bool {
        !matches!(self, YOwner::Sora | YOwner::Menu)
    }
}

struct YPress {
    held: bool,
    owner: YOwner,
    count: u32,
    down_at: std::time::Instant,
    /// The press was Sora's and Elden Ring's prompt came up within the window: handed to Elden Ring as a press of its own.
    replayed: bool,
    /// The press is over but still inside the window in which a prompt hands it on.
    watching: bool,
    prompt_at_down: bool,
    er_frames: u32,
    frames: u32,
}

impl Default for YPress {
    fn default() -> YPress {
        YPress { held: false, owner: YOwner::Sora, count: 0, down_at: std::time::Instant::now(), replayed: false, watching: false, prompt_at_down: false, er_frames: 0, frames: 0 }
    }
}

/// (IMPL_ER14) Counters of the `attack:` / `press:` log lines.
#[derive(Default)]
struct AtkDiag {
    /// The attack last logged (kind << 16 | combo << 8 | air), u32::MAX = none.
    last: u32,
    /// Steps since the last attack line: all, those off the floor, the floor <-> air changes.
    steps: u32,
    off_floor: u32,
    flips: u32,
    /// Presses of the attack button counted since Sora started.
    presses: u32,
    /// Per second: steps in an attack state, those of them off the floor.
    sec_steps: u32,
    sec_off: u32,
}

/// The one-shot keys of the control file (testing aids; none fires from a value present at the start).
struct Shots {
    chrs: OneShot,
    goto: OneShot,
    spawn: OneShot,
    hurt: OneShot,
    heal: OneShot,
    launch: OneShot,
    atk: OneShot,
    watch: OneShot,
    travel: OneShot,
    raw: OneShot,
    hop: OneShot,
    findslope: OneShot,
}

/// THE ONE-SHOT RULE's state: the Tarnished's own HP numbers and what we wrote.
#[derive(Clone, Copy, Debug)]
struct Body {
    /// His own base HP and maximum, as the game computes them.
    own_base: i32,
    own_max: i32,
    /// The base HP we last wrote (a different value read back = the game computed it afresh).
    written_base: i32,
    /// Frames until the maximum is read as his own (after a start or a change by the game).
    settle: u8,
    told: i32,
}

/// The log of what the game does with one target after a test attack.
struct Watch {
    id: TargetId,
    seconds: f32,
    anim: i32,
    hp: i32,
    poise: f32,
    from: Vec3,
    farthest: f32,
    highest: f32,
}

/// Where the pack and the loadout are: `kh-er.cfg` next to the DLL (`pack = ...`, `loadout = ...`),
/// the environment (`KH_SORA_PACK`, `KH_SORA_LOADOUT`), or the project layout the DLL sits in
/// (`<project>\er\mods\kh_er.dll` -> `<project>\build\sora_pack`, `<project>\sheets\port\sora-loadout-99.json`).
pub fn data_paths(root: &Path) -> (PathBuf, PathBuf) {
    let mut cfg = Control::new(root.join("mods").join("kh-er.cfg"));
    cfg.poll();
    let project = root.parent().unwrap_or(root);
    let pick = |key: &str, env: &str, default: PathBuf| {
        cfg.str(key).map(PathBuf::from).or_else(|| std::env::var_os(env).map(PathBuf::from)).unwrap_or(default)
    };
    (
        pick("pack", "KH_SORA_PACK", option_env!("KH_ER_TEST_PACK").map_or_else(|| project.join("build").join("sora_pack"), PathBuf::from)), // (P5) a TEST build's own pack (set at build time only)
        pick("loadout", "KH_SORA_LOADOUT", project.join("sheets").join("port").join("sora-loadout-99.json")),
    )
}

impl SoraHost {
    /// Sora standing where the Elden Ring player stands, facing as he does.
    pub fn start(player: &PlayerIns, root: &Path, control: &Control) -> Option<SoraHost> {
        if !crate::havok::init() {
            erlog!("sora: the Havok casts are not available: Sora is not started");
            return None;
        }
        let physics = &player.chr_ins.modules.physics;
        let feet = game::v3(&physics.position);
        let yaw = coords::yaw_kh_of_er_quat(game::quat(&physics.orientation));
        // KH3's origin is put under his feet; he starts at the height he stands at on a floor at 0.
        let frame = Frame::at(feet);
        let start = Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw };
        let mut sora = Sora::new(SORA_PARAMS, start);

        let (pack_dir, loadout_path) = data_paths(root);
        crate::boc::load(&pack_dir); // (IMPL_ER59) Sora's outfits
        crate::limbs::set_pack(&pack_dir); // (ER70) the worn outfit's skeleton for the hands of a borrowed action
        #[cfg(feature = "party")]
        crate::partydraw::set_pack(&pack_dir); // (ER party S4) the friends' weapons and props for the drawing
        #[cfg(feature = "party")]
        crate::moogle::set_paths(&pack_dir, &loadout_path); // (P8) the Moogle's pack (PACK/party/n_ex028, P8 fix) and sheet (kh-moogle.json beside the loadout)
        #[cfg(feature = "party")]
        crate::spirit_mimic::set_pack(&pack_dir); // (MIMIC SORA) his own Animator and keychain are read from Sora's pack
        crate::status::set_pack(&pack_dir); // (IMPL_ER48) the KH3 item texts
        match std::fs::read_to_string(&loadout_path).map_err(|e| e.to_string()).and_then(|text| kh_host::rig::loadout::parse(&loadout_path.to_string_lossy(), &text)) {
            Ok((mut loadout, problems)) => {
                // `sora.level = N` (1..99; read when Sora starts): the same loadout at another level.
                if let Some(level) = control.i64("sora.level").filter(|l| (1..=99).contains(l)) {
                    loadout.set_level(level as u32);
                }
                erlog!("sora: loadout {} ({} rows not understood), level {}", loadout_path.display(), problems.len(), loadout.level);
                sora = sora.with_status(Status::from_loadout(&loadout));
            }
            Err(e) => erlog!("sora: loadout {} not read ({e}): the empty placeholder status is used", loadout_path.display()),
        }
        crate::save::on_start(&mut sora, root, &loadout_path, control); // (save) `save.kh = 1`: the character's saved Sora, or a new game
        crate::prize::on_start(&mut sora, control); // (prize) a respawn keeps his munny
        let started = std::time::Instant::now();
        // (the real model is drawn now: hair, hood and coat simulate, the face plays its clips)
        // (IMPL_ER23A) KH3's leg IK (kh-anim legik.rs) runs here too: its foot sweeps go through `ErWorld` (`ik.legs = 0` off)
        let settings = Settings { leg_ik: control.flag("ik.legs") != Some(false), kine_driver: true, face: true, bonamik: true, first_update_holds_position: kh_core::anim::FIRST_UPDATE_HOLDS_POSITION };
        let animator = match kh_anim::Pack::load(&pack_dir).and_then(|pack| Animator::new(pack, settings)) {
            Ok(animator) => {
                erlog!("sora: pack {} read in {:.2} s: {} bones", pack_dir.display(), started.elapsed().as_secs_f32(), animator.skeleton().len());
                Some(animator)
            }
            Err(e) => {
                erlog!("sora: pack {} not read ({e}): no skeleton is drawn", pack_dir.display());
                None
            }
        };
        let trace = std::fs::File::create(crate::log::dir().join("sora-trace.csv")).ok().map(|file| {
            let mut w = std::io::BufWriter::new(file);
            let _ = writeln!(w, "step,kh_x,kh_y,kh_z,yaw,state,sub_step,on_floor,vel_x,vel_y,vel_z,er_x,er_y,er_z,anim,stick_x,stick_y");
            w
        });
        let keychain = match kh_anim::Keychain::load(&pack_dir.join("weapon")) {
            Ok(chain) => {
                erlog!("sora: keychain: Bonamik on the weapon skeleton ({} bones, {} driven)", chain.skeleton().len(), chain.driven_bones().len());
                Some(chain)
            }
            Err(e) => {
                erlog!("sora: keychain: {e}: the Keyblade stays rigid");
                None
            }
        };
        // The host's numbers (sheets/port/sora-er-host.json beside the loadout; the defaults are the same rows).
        let host_sheet = loadout_path.with_file_name("sora-er-host.json");
        let params = match std::fs::read_to_string(&host_sheet).map_err(|e| e.to_string()).and_then(|text| ermap::Params::from_sheet(&text)) {
            Ok((params, problems)) => {
                erlog!("sora: host numbers {} ({} rows not understood): {params:?}", host_sheet.display(), problems.len());
                params
            }
            Err(e) => {
                erlog!("sora: host numbers {} not read ({e}): the built-in defaults are used", host_sheet.display());
                ermap::Params::default()
            }
        };
        crate::graceflame::set_ids(params.grace_flame_sfx); // (IMPL_ER17)
        crate::blood::load(&host_sheet); // (IMPL_ER60) the rows `blood.*`
        crate::guardblow::load(&host_sheet); // (THE USER 2026-11-02) the rows `guard_sound.*`
        let out = sora.output();
        erlog!(
            "sora: started at Elden Ring ({:.3}, {:.3}, {:.3}), KH yaw {yaw:.1}; level {} HP {}",
            feet.x, feet.y, feet.z, sora.status().level, sora.status().hp
        );
        Some(SoraHost {
            sora,
            out_before: out.clone(),
            pose_before: kh_anim::Pose::default(),
            weapon: WeaponShow { visible: false, since: 10.0 },
            spears: Vec::new(), // (F-SHIP 3, INT-F)
            keyblade_held: false,
            herolight: crate::herolight::HeroLight::default(), // (HEROLIGHT-KB)
            herolight_left: crate::herolight::HeroLight::secondary(), // (HEROLIGHT-KB final pass)
            form_gate: kh_host::formstart::FormStartGate::default(), // (FORMWEP-FIX)
            freeze_fx: Default::default(), // (FREEZE-FX)
            keychain,
            chain: Vec::new(),
            chain_before: Vec::new(),
            int: crate::interp::Track::default(), // (INT)
            int_drawn: Default::default(),
            weapon_shown: false,
            clock: 0.0,
            out,
            prize: crate::prize::ErPrizes::start(control, params.prize), // (prize)
            loot: crate::loot::ErLoot::new(), // (loot)
            attackers: crate::attackers::ErAttackers::new(), // (ENG)
            boss_rewards: crate::bossreward::ErBossRewards::new(control), // (IMPL_ER31)
            weapon_src: crate::weaponsrc::ErWeaponSources::new(), // (P9b) this Sora's drops (the load epoch is read per kill)
            savepoint: crate::savepoint::ErSavePoints::new(&pack_dir, control, params.savepoint_sfx).with_story(&loadout_path.with_file_name("sora-er-story.json")).with_ground(params.ground, params.collision_floor_layer).with_layer(params.collision_layer) /* (WALLS) */, // (savepoint) (IMPL_ER33: the story table) (IMPL_ER52: on the ground)
            fx: crate::fx::Fx::start(&pack_dir, &loadout_path.with_file_name("sora-er-fx.json"), control), // (fx)
            hud: crate::hud::HudHost::new(&pack_dir), // (hud)
            world: ErWorld::new(frame),
            cam: crate::cam::KhCam::new(&pack_dir, &crate::log::dir(), frame),
            finish: crate::finish::ErFinish::new(crate::finish::FinishRules::DEFAULT),
            worldtime: crate::worldtime::ErWorldTime::default(), // (AUDIT-A WORLDTIME)
            script: crate::script::InScript::new(control),
            script_clock: 0.0,
            script_at: std::time::Instant::now(),
            script_now: None,
            held_before_r3: false,
            animator,
            pose: kh_anim::Pose::default(),
            reader: pad::Reader::default(),
            accumulator: 0.0,
            held_before: PadButtons::NONE,
            // (ONE-SHOT: a value that is in the file at the start is not a press)
            scripted_jump: (control.i64("in.jump").unwrap_or(0), 0),
            targets: ErTargets::new(frame, params),
            rebased_now: false,
            relocated_post: false, // (WALLS load fix)
            release_guard: 0.0,    // (WALLS load fix)
            cam_snap_due: false,   // (WALLS cutscene fix)
            follow: None,
            ground_wait: crate::groundwait::Wait::default(), // (ZETTA 2026-10-23)
            fresh: true,
            loadin: crate::loadin::LoadIn::new(), // (LDI) a new host: a load is coming in
            anchor: None,
            void_for: 0.0,
            void_check: 0.0,
            remo: None,
            follow_yaw: None,
            cutscene: false,
            cut_draw: CutDraw::Normal,
            cut_at: None,
            own_weight: 0.0,
            own_rig: None,
            own_over: None,
            own_anim: -1,
            own_reach: 1.0,
            net_last: None,
            own_hold: false,
            cut_lying: None,
            cut_rests: None, // (CUTSCENE-ANIM)
            own_seen: -1,
            kh_shot: OneShot::new("dbg.kh", control),
            lab: lab::Lab::new(control),
            hp_written: None,
            body: None,
            travel: None,
            hop: None,
            watch: None,
            died: false,
            death_sent: false,
            flow: crate::death::Flow::new(death_rules(control)), // (DTH)
            sora_fell: false,
            frame_dt: 1.0 / 60.0,
            er_locked: None,
            lock_missing: None,
            ui: inputs::ErUi::default(),
            ui_states: None,
            prompt: false,
            still: (0.0, 0, 0),
            paused: false,
            r3_before: false,
            invulnerable: false,
            shots: Shots { chrs: OneShot::new("dbg.chrs", control), goto: OneShot::new("dbg.goto", control), spawn: OneShot::new("dbg.spawn", control), hurt: OneShot::new("dbg.hurt", control), heal: OneShot::new("dbg.heal", control), launch: OneShot::new("dbg.launch", control), atk: OneShot::new("dbg.atk", control), watch: OneShot::new("dbg.watch", control), travel: OneShot::new("dbg.travel", control), raw: OneShot::new("dbg.raw", control), hop: OneShot::new("dbg.hop", control), findslope: OneShot::new("dbg.findslope", control) },
            hits_in: 0,
            invulnerable_frames: 0,
            key_frames: 0,
            atk: AtkDiag { last: u32::MAX, ..AtkDiag::default() },
            y: YPress::default(),
            latch: Default::default(),
            dialog: Default::default(),
            popup: None,
            dialog_up: false,
            fall: Default::default(),
            body_fall: Default::default(),
            no_dead_own: None,
            no_dead_held: false,
            death_anim_for: 0.0,
            stuck: Default::default(),
            stuck_now: false,
            leg_ik: control.flag("ik.legs") != Some(false),
            stuck_last: None,
            latch_told: PadButtons::NONE,
            fast_travel: Default::default(),
            stood: Default::default(),
            blocked: Default::default(),
            blocked_now: false,
            floor_report: None,
            reports: (0, None),
            floor_told: u32::MAX,
            written: None,
            trace,
            steps_in_tick: 0,
            casts_in_tick: 0,
            pushed: 0.0,
            pad_seen: false,
            pad_active: 0,
            warped: None,
            scanned: None,
            proxied: None,
            fscanned: None,
            action_end: None,
            ride: None,
            rode: 0.0,
            rode_frames: 0,
            rode_still: 0,
            ladder_seen: -1,
            ladder: Default::default(),
            ladder_go: Default::default(),
            descend_told: false,
            telescope_told: false,
            pool_told: false,
            leap_wait: None,
            spring: Default::default(),
            leap: None,
            leap_show: crate::ladder::Show::Own,
            chest: Default::default(),
            still_net: Default::default(),
            aim_closed_seen: 0,
            form_nets_seen: 0,
            ladder_truth: Default::default(),
            busy_net: Default::default(),
            own_cap: Default::default(),
            ladder_fix: crate::exitnet::Fix::None,
            anims_dt: 1.0 / 60.0,
            own_fit: Default::default(),
            own_plan: None,
            own_dims: None,
            own_surface: false,
            #[cfg(feature = "party")]
            lead_walk_told: false,
            #[cfg(feature = "party")]
            lead_insane_told: false,
            worn: Default::default(),
            map_watch: Default::default(),
            work: std::time::Duration::ZERO,
            drown: Default::default(),
            barrier: Default::default(), // (DEATHPLANE)
            water_check: 0.0,
            water_told: 0,
            grab: Default::default(),
            throw_seen: None,
            catch: None, // (GRABS)
            grabber: None,
            abduct: None,
            pickup_seen: false,
            door_seen: false,                       // (ER-TWEAKS)
            walk_action: false,                     // (ER-TWEAKS)
            fog_walk: Default::default(),           // (ER-TWEAKS)
            fog_trail: Vec3::ZERO,                  // (ER-TWEAKS)
            fog_walk_told: false,                   // (ER-TWEAKS)
            #[cfg(feature = "party")]
            fog_drew: false,                        // (ER-TWEAKS)
            #[cfg(feature = "party")]
            lead: { let mut lead = crate::party::leader::LeaderBody::default(); lead.set_pack(&pack_dir); lead }, // (ER party S2)
            #[cfg(feature = "party")]
            party: crate::party::PartySlot::start(&pack_dir, loadout_path.parent().unwrap_or(Path::new(".")), control), // (ER party S1)
        })
    }

    /// (IMPL_ER10) The game moves the body itself (a cutscene): the passive switches are relaxed meanwhile (frame.rs).
    pub fn game_owns_body(&self) -> bool {
        matches!(self.follow, Some(Follow::Cutscene | Follow::Action | Follow::Grab))
    }

    /// (IMPL_ER13) Sora stands on a floor as of his latest step / Elden Ring's action prompt is on screen.
    /// (IMPL_ER17) The body performs an action of the game's in which the game moves it (`own.move.*`).
    pub fn body_action(&self) -> bool {
        self.follow == Some(Follow::Action) || self.ladder_go.letting_go()
    }

    /// (LADDER2) The game is letting its body go off its ladder (ladder.rs `LetGo`): the body keeps the game's own switches
    /// meanwhile (its movement, action and gravity), so that its graph can drop it (frame.rs).
    pub fn body_on_ladder(&self) -> bool {
        self.ladder_go.letting_go()
    }

    pub fn on_floor(&self) -> bool {
        self.lead_on_floor() // (ER party S2) the body's leader (Sora: his own step, as before)
    }

    /// (ER party S2) A FRIEND LEADS (Donald / Goofy): the hidden body follows HIM (party/leader.rs). None = Sora leads, the
    /// party is off or not compiled: every body rule reads Sora's own `Output` exactly as before.
    #[cfg(feature = "party")]
    fn friend_lead(&self) -> Option<crate::guards::FriendLead> {
        self.lead.friend_lead()
    }
    #[cfg(not(feature = "party"))]
    fn friend_lead(&self) -> Option<crate::guards::FriendLead> {
        None
    }

    /// (ER party S2) An order of the body code for the friend leader (carried out at the party's next step).
    #[cfg(feature = "party")]
    fn lead_order(&mut self, order: crate::guards::LeadOrder) {
        // (LOADFALL, the in-game tester 2026-11-03: a load after Goofy died mid-hop wrote the body to his place of before the
        // load and the leader fell for ever) A SEAT IS TAKEN AT ONCE, not at the party's next step (none runs while a new host
        // waits for the ground): the friend leader is on the body's place before anything reads or writes his place
        if let crate::guards::LeadOrder::Follow(at, yaw) = order {
            if let Some(rt) = self.party.runtime_on() {
                self.lead.seat_now(rt, &mut self.sora, &self.out, at, yaw);
            }
        }
        self.lead.order(order);
    }
    #[cfg(not(feature = "party"))]
    fn lead_order(&mut self, _order: crate::guards::LeadOrder) {}

    /// (ER party S2) The body's leader: where he stands (feet, KH3), his yaw, his floor, alive (Sora: his own as before).
    fn lead_pos(&self) -> Vec3 {
        self.friend_lead().map_or(self.out.position, |f| f.position)
    }
    fn lead_yaw(&self) -> f32 {
        self.friend_lead().map_or(self.out.yaw, |f| f.yaw)
    }
    fn lead_on_floor(&self) -> bool {
        self.friend_lead().map_or(self.out.on_floor, |f| f.on_floor)
    }
    fn lead_alive(&self) -> bool {
        self.friend_lead().map_or(self.sora.status().hp > 0, |f| f.alive)
    }

    /// (HUD-REVIVE-COMBAT, THE USER 2026-11-07: "if an enemy is actively fighting the party leader or any party member, the whole
    /// party should still be in combat. Combat only ends when no enemy is targetting the party") THE PARTY'S MARKS ON THE TARGETS,
    /// once a frame after the enemies were read (telegraph.rs): every party place besides the leader's (the members who are up,
    /// a friend leader, AI Sora) and the hostiles whose own Elden Ring AI targets a party member's body (party/aggro.rs
    /// `aims_now`: the game's AI target matched to the member there; Mimic Sora is no party member and does not count). Read by
    /// `ErTargets::engaged_count`, the one count behind every battle flag. `combat.aims = 0`: the aim sign is not used.
    fn party_marks(&mut self, control: &Control) {
        self.targets.aims_rule = control.flag("combat.aims") != Some(false);
        #[cfg(feature = "party")]
        if let Some(core) = self.party.core() {
            let rt = &core.rt;
            let mut at: Vec<Vec3> = rt.current.iter().filter(|v| !v.ko).map(|v| v.position).collect();
            if rt.leader.is_some() && rt.lead.sora_active && self.sora.status().hp > 0 {
                at.push(self.out.position); // AI Sora
            }
            let aims = crate::party::aggro::aims_now(rt, &self.targets, self.out.position).into_iter().filter(|(_, who)| who.is_some_and(|w| w.chara().is_some())).map(|(id, _)| id).collect();
            self.targets.set_party_marks(at, aims);
            return;
        }
        // Sora alone: the enemies whose AI targets his body (the place within 3 m of his feet: aggro.aim_near's default)
        let lead = self.lead_pos();
        let aims = crate::telegraph::aims().into_iter().filter(|(id, aim)| self.targets.get(*id).is_some_and(|e| (e.pos + coords::dir_er_to_kh(*aim - e.er_pos) - lead).length() <= 300.0)).map(|(id, _)| id).collect();
        self.targets.set_party_marks(Vec::new(), aims);
    }
    /// (DTH) WHO LEADS the body this frame (Sora when he does, the party is off or not compiled).
    #[cfg(feature = "party")]
    fn lead_chara(&self) -> kh_host::party::PartyChara {
        if self.friend_lead().is_some() { self.lead.chara() } else { kh_host::party::PartyChara::Sora }
    }
    #[cfg(not(feature = "party"))]
    fn lead_chara(&self) -> kh_host::party::PartyChara {
        kh_host::party::PartyChara::Sora
    }

    /// (DTH) The game's no-death switch is the death flow's (a scripted death until its scene): frame.rs keeps it held.
    pub fn death_holds_body(&self) -> bool {
        self.flow.holds_no_dead()
    }

    /// (DTH) The death handed to the game was a scripted one (no reload of the last save after it).
    pub fn scripted_sent(&self) -> bool {
        self.flow.sent() && self.flow.kind() == Some(crate::death::Kind::Scripted)
    }

    /// (DTH) THE DEAD PATH'S PART OF THE DEATH FLOW (frame.rs, PRE_PHYSICS, once a frame while the game owns its player): the
    /// scripted death's black until the game's scene plays, the no-death switch held until then, the game's own death given
    /// if nothing came of it (`death.wait`).
    pub fn death_dead_frame(&mut self, player: &mut PlayerIns, control: &Control, dt: f32) {
        use fromsoftware_shared::FromStatic;
        let order = self.flow.dead_step(dt, scene_plays(control));
        crate::death::plate(order.fade);
        // SAFETY: game thread; the SDK resolves the static by its pinned RVA (the exe-version gate holds).
        if let Ok(flags) = unsafe { eldenring::cs::WorldChrManDbgFlags::instance_mut() } {
            if order.hold_no_dead {
                if !flags.player_no_dead {
                    flags.player_no_dead = true;
                }
                self.no_dead_held = true;
                NO_DEAD_HELD.store(true, std::sync::atomic::Ordering::Relaxed);
            } else if order.body == crate::death::Body::Kill && std::mem::take(&mut self.no_dead_held) {
                flags.player_no_dead = false;
                NO_DEAD_HELD.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }
        if order.body == crate::death::Body::Kill {
            player.chr_ins.modules.data.as_mut().hp = 0;
            crate::hud::hold_banner(); // (its YOU DIED text held away, as after the Retry's death)
            erlog!("death: THE SCRIPTED DEATH GAVE NOTHING in {:.0} s (no scene of the game's): the Tarnished's HP set to 0, Elden Ring's own death follows under the black (death.wait)", self.flow.rules.wait);
        }
    }

    pub fn prompt_up(&self) -> bool {
        self.prompt
    }

    /// (IMPL_ER10) Sora's death was handed to the game (the Tarnished's HP written to 0).
    pub fn death_sent(&self) -> bool {
        self.death_sent
    }

    /// (IMPL_ER10) WHO HAS THE BODY. Sora normally drives the hidden body. He FOLLOWS it instead
    /// - while a real-time cutscene plays (`Follow::Cutscene`): the game moves and animates its player, Sora is
    ///   drawn where it is, facing as it faces, standing (a level floor under him), without input;
    /// - after the game put its player somewhere else (a warp: the tutorial's death, a cutscene's end, a start)
    ///   UNTIL THERE IS GROUND UNDER HIM (`Follow::NoGround`: a cast down from his feet hits within 3 m): the
    ///   collision of the new place may not be loaded yet, and he must never fall through it.
    ///   THE NET: in the air with no ground at all within 300 m below for 1.5 s -> he is put back where he last
    ///   stood (or was seated by the game) and waits there for the ground.
    fn follow_phase(&mut self, player: &mut PlayerIns, control: &Control, now: Vec3, relocated: bool, dt: f32) {
        let height = Sora::origin_height_on_floor(&SORA_PARAMS);
        let remo = game::remo_state();
        if remo != self.remo {
            erlog!("cutscene: the game's cutscene manager: state {:?} -> {:?}", self.remo, remo);
            self.remo = remo;
        }
        // THE RULE (MEASURED, IMPL_ER10_STATE.md): a cutscene plays while the manager's state is one of `cut.states`.
        let playing = match control.str("cut.states") {
            Some(list) => remo.is_some_and(|s| list.split(',').any(|p| p.trim().parse::<i32>() == Ok(s))),
            None => remo.is_some_and(|s| CUTSCENE_STATES.contains(&s)),
        };
        let cutscene = playing && control.flag("cut.follow") != Some(false);
        if cutscene != self.cutscene {
            erlog!("cutscene: {}", if cutscene { "A REAL-TIME CUTSCENE PLAYS: Sora follows the body, his HUD is hidden, the pad is the game's" } else { "the cutscene is over" });
            self.cutscene = cutscene;
            self.prize.cutscene(cutscene, &mut self.sora); // (prize, IMPL_ER20) no orbs on the screen in a cutscene
            if !cutscene {
                crate::remo::forget(); // (IMPL_ER12) the scene's models are gone with it
            }
        }
        let before = self.follow;
        let fresh = std::mem::take(&mut self.fresh);
        // (IMPL_ER17) the body plays one of the game's own actions (`own.anim.*`; the key `own.anims` replaces the rows, `own.retarget = 0` switches it off)
        let action = {
            let ta = &player.chr_ins.modules.time_act;
            let anim = ta.anim_queue[(ta.read_idx as usize) % 10].anim_id;
            let within = |key: &str, rows: &[(i32, i32)]| match control.str(key).map(ermap::own_ranges) {
                Some(ranges) => ranges.iter().any(|(a, b)| (*a..=*b).contains(&anim)),
                None => rows.iter().any(|(a, b)| (*a..=*b).contains(&anim)),
            };
            let allowed = !cutscene && control.flag("own.retarget") != Some(false) && self.lead_alive() /* (ER party S2) */ && !(relocated || fresh);
            // (an action in place: the body is held where Sora stands as always, he only takes no input and shows its pose)
            // (IMPL_ER67, the user: "Sora should also still be able to run around while doing the pickup animation for items (not
            // chests though) just elden ring") THE GAME'S PICKUP IS NOT HIS: while the body plays a pickup animation (`pickup.anims`,
            // default 60070-60079; MEASURED: 60071, 1.83 s, "Pillage remains" / gathering) Sora is neither held nor shown in its
            // pose: he keeps the pad and runs on, the body is written to where he is as always. The item is not affected: MEASURED
            // in the user's log (111.609 s Y to the game, 111.694 the loot gate's swap + "item: obtained", 111.718 the body's
            // 60071 starts) the game gives the item at the action's start, before the animation. `pickup.run = 0`: as before.
            crate::chest::learn_from_game(); // (ER-TWEAKS follow-up) the chests of the game's own object-action table (once)
            let pickup = control.flag("pickup.run") != Some(false) && crate::guards::pickup_anim(anim, control.str("pickup.anims"));
            if pickup != self.pickup_seen {
                self.pickup_seen = pickup;
                if pickup {
                    erlog!("pickup: the body plays the game's pickup {anim}: SORA RUNS ON (no hold, no pose of the body's; the item was given at the press; Sora {:?}) (pickup.run = 0: held as before)", self.out.state);
                }
            }
            // (ER-TWEAKS, the user 2026-11-06: "when Y is pressed the door opens but the party leader isn't locked into any
            // animation regardless of door type") A DOOR / LEVER IS NOT THE LEADER'S EITHER (doorfree.rs: every ObjActParam door
            // and lever animation): no hold, no seat on the body, no pose of the body's; the game's ObjAct opens the door and
            // kicks its event / flag from the press as before (the body plays its animation, hidden). `door.free = 0`: as before.
            let door = control.flag("door.free") != Some(false) && crate::doorfree::free_anim(anim, control.str("door.anims"));
            if door != self.door_seen {
                self.door_seen = door;
                if door {
                    erlog!("door: (ER-TWEAKS) the body plays the game's door / lever {anim}: THE LEADER ({}) STAYS FREE (no hold, no follow, no pose of the body's; the game opens the door with its own ObjAct and event) (door.free = 0: as before)", self.lead_chara_name());
                }
            }
            let pickup = pickup || door;
            self.own_hold = allowed && !pickup && within("own.anims", &self.targets.sheet.own_anims);
            // (IMPL_ER18) a row of the held kind wins over the follow rows (the row `own.move.unlisted` is a whole range)
            // (LADDER2) THE GAME'S LADDER (ladder.rs): Y at a ladder's "Climb" prompt reached the game, which starts its own
            // ladder on the body (its approach 28999, then state 0 StartBottom). Nobody climbs: the start is read once (the
            // module's top and its heading: the landing), the game is asked to let its body go (`ladder_body`), and the leader
            // leaps up onto the landing with the party after him (`leap_launch`, the leap in the steps). The ladder animations
            // without the state (`own.ladder_anims`, default 28000-28999) count too: the approach, the let-go's fall.
            let ladder_allowed = !cutscene && self.lead_alive() /* (ER party S2) */ && !(relocated || fresh);
            let ladder_anim = match control.str("own.ladder_anims").map(ermap::own_ranges) {
                Some(ranges) => ranges.iter().any(|(a, b)| (*a..=*b).contains(&anim)),
                None => (crate::ladder::DEFAULT_ANIMS.0..=crate::ladder::DEFAULT_ANIMS.1).contains(&anim),
            };
            // (ANIMS) THE LADDER STATE AS BELIEVED (exitnet.rs): a module that still says "ladder" while the body plays none of
            // the ladder animations (the let-go's W_LadderDrop left the graph's ladder states behind the module's back) is
            // over: believed -1, the module repaired below; a death, a load or a warp forgets every ladder state of ours
            let (state, ladder_fix) = self.ladder_truth.frame(crate::exitnet::Seen { state: ladder_state(player), ladder_anim, lead_on_floor: self.lead_on_floor() }, dt);
            if !self.lead_alive() || relocated || fresh {
                if self.ladder_go.active || self.leap_wait.is_some() || self.leap.is_some() {
                    erlog!("ladder: {} during a ladder jump: every ladder state of ours is forgotten (the let-go, the leap)", if self.lead_alive() { "a load or a warp" } else { "THE LEADER IS DOWN" });
                }
                self.ladder_go.end();
                self.leap_wait = None;
                self.leap = None;
                self.leap_show = crate::ladder::Show::Own;
            }
            let started = self.ladder.frame(state, ladder_anim, ladder_allowed, dt);
            // (ANIMS) THE SAFETY NET: our side busy with a ladder while the body has been in plain locomotion for 1 s: cleared
            let body_plain = state < 0 && !ladder_anim && !within("own.anims", &self.targets.sheet.own_anims) && !within("own.moves", &self.targets.sheet.own_moves);
            if self.busy_net.frame(self.ladder_go.active || self.leap_wait.is_some(), body_plain, dt) {
                erlog!("!!! ladder: OUR SIDE WAS STILL ON A LADDER (the let-go {}, a leap waiting {}) while the body has walked in plain animations ({anim}) for {:.1} s: CLEARED (the net, #{})", self.ladder_go.active, self.leap_wait.is_some(), crate::exitnet::BusyNet::NET_S, self.busy_net.fired);
                self.ladder_go.end();
                self.leap_wait = None;
            }
            self.ladder_fix = ladder_fix;
            if started {
                self.ladder_start(player, state, anim);
            }
            // (IMPL_ER58) A CHEST (chest.rs; `chest.kh = 0`: as before, the body's chest animation is no action of ours): Sora
            // follows the body (the game places and turns it at the chest), and his simulation opens KH3's treasure box
            // (CHESTFIX, the user 2026-10-27: "Sora should play his chest animation for ALL chests" / "It also plays again when
            // he picks up the item from the chest") every chest OPENING of the user's logs (chest.rs `DEFAULT_LIST`: 60080,
            // 60110); not the item's pickup from an open chest (60090 under "Search treasure chest"): one opening per chest
            let chest_listed = match control.str("chest.anims").map(ermap::own_ranges) {
                Some(ranges) => ranges.iter().any(|(a, b)| (*a..=*b).contains(&anim)),
                None => crate::chest::default_anim(anim),
            };
            crate::loot::note_chest_anim(chest_listed); // (PU) the gate tells a chest's item by it (any leader)
            let chest_allowed = !cutscene && control.flag("chest.kh") != Some(false) && self.lead_alive() /* (ER party S2) */ && !(relocated || fresh);
            // (CHESTFIX, the user 2026-10-27: "the chest opens AFTER he taps it with the keyblade") NO HOLD OF THE BODY: the game's
            // lid runs on the chest's own time (the CHESTFIX build froze the body: the lid opened all the same, and the frozen
            // body stopped the game's world update flag: Sora paused, the game's camera behind the chest; the user's log
            // 367.144 - 368.595). KH3's state is asked for at once and is OpenTreasureS by default (its tap at 0.47 s, the
            // earliest KH3 has: chest.rs `large`); `chest.delay` (s) and `chest.size = l` as before
            let body_len = ta.anim_queue[(ta.read_idx as usize) % 10].anim_length;
            let large = crate::chest::large(control.str("chest.size"), body_len);
            let friend_chest = self.friend_lead().is_some();
            let delay = control.f32("chest.delay").unwrap_or(0.0).clamp(0.0, 10.0);
            let (on_chest, chest_edge) = self.chest.frame(anim, chest_listed, chest_allowed, delay, dt);
            match chest_edge {
                crate::chest::Edge::Begin { anim } => erlog!("chest: THE BODY OPENS A CHEST (animation {anim}, {body_len:.2} s): {} (chest.kh = 0: off)", if friend_chest { format!("the leader ({}) is seated on the body, facing the chest, and plays NO chest animation (his own idle)", self.lead_chara_name()) } else { "Sora FOLLOWS it (seated on the body, facing the chest) and plays KH3's own treasure-box opening".to_string() }),
                // (CHESTFIX, the user 2026-10-27: "Goofy and Donald should have no chest animation") a friend leading: no KH3
                // clip and no retargeted body animation (own_frame); he stands facing the chest while the game opens it
                crate::chest::Edge::Ask if friend_chest => erlog!("chest: the leader ({}) opens it with no chest animation: he stands facing the chest while the game opens it (the item comes through the gate as always)", self.lead_chara_name()),
                crate::chest::Edge::Ask => {
                    let asked = self.sora.open_treasure_box(large);
                    erlog!("chest: KH3's TresPlayerState_OpenTreasureBox {} ({}; {delay:.2} s after the body's begin; the body is not held: the game's lid keeps its own time)", if asked { "ASKED FOR" } else { "NOT ASKED (Sora is dead)" }, if large { "large: OpenTreasureL, 2.97 s, the box opens at 2.0 s" } else { "small: OpenTreasureS, 0.97 s, the box opens at 0.5 s" });
                }
                crate::chest::Edge::End { anim, seconds, asked } => erlog!("chest: the body's chest animation {anim} is over after {seconds:.2} s ({}; Sora's state now {:?}): the body is handed back as after a door", if friend_chest { format!("the leader {} played none", self.lead_chara_name()) } else if asked { "KH3's opening was played".to_string() } else { "KH3's opening WAS NOT asked for".to_string() }, self.out.state),
                crate::chest::Edge::None => {}
            }
            // (LADDER2) the game's ladder animations are never followed (the leap has the leader)
            if on_chest || ladder_anim {
                self.own_hold = false;
            }
            // (IMPL_ER18, UNTESTED, OFF unless the key is set) `own.unlisted = 60000-60999`: an id of that range WITHOUT a row of its
            // own is followed like an `own.move.*` row (single doors, levers, chests are expected there; a held row always wins).
            let unlisted = !pickup && control.str("own.unlisted").map(ermap::own_ranges).is_some_and(|r| r.iter().any(|(a, b)| (*a..=*b).contains(&anim)));
            // (ANIMS) the actions' cap (exitnet.rs `OwnCap`, `own.max_s` default 20 s): no door / fog follow holds the pad for ever
            // (ER-TWEAKS) a door / lever (`pickup` includes it) is never followed, whatever row or key names it
            let moved_by_row = within("own.moves", &self.targets.sheet.own_moves);
            let moves = allowed && !pickup && !self.own_hold && !ladder_anim && (moved_by_row || unlisted);
            let (moves, spent) = self.own_cap.frame(anim, moves, control.f32("own.max_s").unwrap_or(crate::exitnet::OwnCap::DEFAULT_MAX_S), dt);
            if spent {
                erlog!("!!! own: the body has played the action {anim} for more than {:.0} s (own.max_s): no longer followed (Sora has the body) until it changes", control.f32("own.max_s").unwrap_or(crate::exitnet::OwnCap::DEFAULT_MAX_S));
            }
            // (ER-TWEAKS, the user 2026-11-06: "when entering fog gate, just make the party leader play their walking animation
            // thru the fog gate instead of using the elden ring animation") an `own.move.*` row (the fog) is still followed (the
            // game carries the body through: its fog flag, the boss's start), but the leader shows his own walk (fogwalk.rs)
            self.walk_action = moves && moved_by_row && control.flag("fog.walk") != Some(false);
            self.own_surface = (on_chest || moves) && !self.walk_action; // (ANIMS) the hands on something in front (propfit.rs `Kind::Surface`)
            on_chest || moves
        };
        // (ANIMS) the game's ladder module after a stale ladder (exitnet.rs): repaired once, a stuck let-go nudged once
        self.ladder_repair(player, control);
        // (LADDER2) the game's let-go of its body, then the leap (the leader's, the party's after him)
        if self.ladder_body(player, dt) == crate::ladder::LetStep::Drop {
            self.landing_refresh(player);
        }
        self.leap_launch(dt);
        // (IMPL_ER57) an enemy's grab wins over the body's own actions (not over a cutscene)
        let grabbed = self.grab_phase(player, control, cutscene, dt);
        if grabbed {
            self.own_hold = false;
        }
        if cutscene {
            self.follow = Some(Follow::Cutscene);
        } else if grabbed {
            self.follow = Some(Follow::Grab);
        } else if action {
            self.follow = Some(Follow::Action);
        } else if matches!(before, Some(Follow::Cutscene | Follow::Action | Follow::Grab)) || relocated || fresh {
            self.follow = Some(Follow::NoGround { since: 0.0 });
            // (ZETTA 2026-10-23) a cutscene's end, a warp, a load: the strict wait (groundwait.rs); an action or a grab: as before
            self.ground_wait = crate::groundwait::Wait::start(before == Some(Follow::Cutscene) || relocated || fresh);
            self.cam_snap_due = crate::groundwait::camera_snap_due(before == Some(Follow::Cutscene), relocated, fresh); // (WALLS cutscene fix)
            if before == Some(Follow::Action) && self.friend_lead().is_none() /* (ER party S2) Sora's own invariant */ {
                self.action_end = Some((self.out.position, 0.0, false)); // (IMPL_ER18) the invariant below starts here
            }
        }
        // (IMPL_ER57) the game's deep-water death, for Sora (before the net: the sea is no pit to be put back from)
        if self.follow.is_none() && self.water_phase(control, dt) {
            return;
        }
        let body_yaw = coords::yaw_kh_of_er_dir(game::quat(&player.chr_ins.modules.physics.orientation) * Vec3::NEG_Z);
        self.anims_dt = dt;
        // (WALLS load fix 2026-10-23) THE GUARD AFTER A RELEASE (groundwait.rs `GUARD_S`): given back at once by the fast rule, the
        // leader who leaves the floor with no ground at all below him in the next seconds (the place still streaming in, an origin
        // shift the release did not see) goes back into the wait, held where he is, instead of falling. Key `load.guard = 0`: not.
        if self.release_guard > 0.0 {
            self.release_guard = (self.release_guard - dt).max(0.0);
            if self.follow.is_none() && self.lead_alive() && !self.lead_on_floor() && control.flag("load.guard") != Some(false) {
                let p = self.lead_pos();
                if kh_core::World::line_trace(&self.world, p + Vec3::Z * 20.0, p - Vec3::Z * (height + 300.0)).is_none() {
                    erlog!("sora: (WALLS) {:.2} s after the release the leader left the floor with no ground below (still streaming in): back to the wait for the ground, held where he is (no fall)", crate::groundwait::GUARD_S - self.release_guard);
                    self.release_guard = 0.0;
                    self.follow = Some(Follow::NoGround { since: 0.0 });
                    self.ground_wait = crate::groundwait::Wait::start(false);
                }
            }
        }
        // ---- the net (IMPL_ER24: guards.rs `Anchor` and `net_verdict`; `net.safe = 0` = the rule as it was)
        if self.follow.is_none() && control.flag("follow.net") != Some(false) {
            let safe = control.flag("net.safe") != Some(false);
            // (ER party S2) the net on the LEADER (Sora: his own step, as before)
            let friend = self.friend_lead();
            let (lead_floor, lead_at) = (self.lead_on_floor(), self.lead_pos());
            self.stood.frame(lead_floor, lead_at);
            if lead_floor {
                // (where he has STOOD for a moment: a landing on an edge he slides off again is no place to bring him back to)
                self.anchor = if safe { self.stood.at.or(self.anchor) } else { Some(lead_at) };
                self.void_for = 0.0;
            } else {
                self.void_check += dt;
                if self.void_check >= 0.5 {
                    self.void_check = 0.0;
                    let p = lead_at;
                    // any ground under him: a line, and (IMPL_ER24) a sphere of his body's width: a line goes through the holes
                    // and cracks of a plank floor he is wedged on
                    let wide = friend.map_or(SORA_PARAMS.capsule_radius, |f| f.capsule.0);
                    // (NETDROP) the nearest 30 m first, then the whole 300 m: a floor close under him counts even where the one long
                    // cast does not see it (the user's log 2026-11-08: put back over lower floors 3 - 8 m down)
                    let ground = kh_core::World::line_trace(&self.world, p + Vec3::Z * 50.0, p - Vec3::Z * 3_000.0).is_some()
                        || kh_core::World::line_trace(&self.world, p + Vec3::Z * 50.0, p - Vec3::Z * 30_000.0).is_some()
                        || safe && kh_core::World::sweep_capsule(&self.world, p + Vec3::Z * (wide + 50.0), p - Vec3::Z * 30_000.0, wide, wide).is_some();
                    // (IMPL_ER57) WATER UNDER HIM IS GROUND for the net: his casts go through the sea (layer 0x56 does not see it),
                    // and the net put him back from a jump into the sea before he reached it (the user's log, 111.2 s); in the
                    // water the deep-water rule decides (`water_phase`), as the game's own does
                    // (DEATHPLANE) water under him is ground for the net while either water rule is on: the barrier rule (the sea's kill plane
                    // lies about 2 m under its surface, measured in game 2026-11-03) needs him to reach the water, not a put-back from the cliff
                    let water_rule = control.flag("water.death") != Some(false) || control.flag("barrier.death") != Some(false);
                    let ground = ground || water_rule && self.water_below();
                    self.void_for = if ground { 0.0 } else { self.void_for + 0.5 };
                    let dropped = if safe { self.stood.dropped(p.z) } else { f32::MAX };
                    if let (true, Some(anchor)) = (self.lead_alive() /* (ER party S2) */ && !self.drown.under(), self.anchor) {
                        // (IMPL_ER17) THE ANCHOR ITSELF IS IN THE VOID (he was put back to this very place a moment ago and fell out
                        // again: IMPL_ER14 saw that loop every 9.5 s for ever): there is no place of his to go back to, so the fall
                        // is a death, as the game's own kill plane makes it: the game-over menu (Continue = the last save).
                        let again = self.net_last.is_some_and(|(at, when)| (at - anchor).length() < 100.0 && when.elapsed().as_secs_f32() < 30.0);
                        // (DEATHPLANE) a leader doomed by one of the game's death barriers falls into the void to his death, no put-back
                        match crate::deathplane::net(crate::guards::net_verdict(self.void_for, dropped, again), self.barrier.void_is_death(crate::deathplane::Who::Leader)) {
                            crate::guards::NetVerdict::Nothing => {
                                if self.void_for >= 1.5 && self.void_for < 2.0 {
                                    erlog!("net: no ground at all under him for {:.1} s at kh ({:.0}, {:.0}, {:.1}) but he is only {:.0} cm below where he last had a floor: NOT A FALL (wedged, hovering or gliding): nothing is done (the stuck rule frees a wedged body)", self.void_for, p.x, p.y, p.z, dropped);
                                }
                            }
                            crate::guards::NetVerdict::Death => {
                                erlog!("sora: NO GROUND AT ALL again {:.1} s after he was put back to the same place, and he is {:.0} cm below it: THE PLACE HE WAS PUT BACK TO HAS NO GROUND EITHER: the fall is his death (the game-over menu)", self.net_last.map_or(0.0, |(_, when)| when.elapsed().as_secs_f32()), dropped);
                                self.net_last = None;
                                self.anchor = None;
                                self.stood = Default::default();
                                self.void_for = 0.0;
                                // (KILLPLANE) doomed by one of the game's barriers: the barrier's own death, instant and past every ability
                                let doomed = self.barrier.void_is_death(crate::deathplane::Who::Leader);
                                self.barrier.leader_killed |= doomed;
                                if friend.is_some() {
                                    // (ER party S2) the friend leader's fall is his KO (the game over is the leader's)
                                    #[cfg(feature = "party")]
                                    if doomed {
                                        if let Some(rt) = self.party.runtime_on() {
                                            if let Some(id) = rt.leader {
                                                crate::deathplane::knock_out(&mut rt.party, id);
                                            }
                                        }
                                        return;
                                    }
                                    self.lead_order(crate::guards::LeadOrder::Die);
                                    return;
                                }
                                self.invulnerable = false;
                                if doomed {
                                    let feet = self.out.position;
                                    crate::deathplane::kill_core(&mut self.sora, feet);
                                    return;
                                }
                                let from = self.out.position + Vec3::Z * 100.0;
                                self.sora.take_hit(HitOnPlayer { direction: Vec3::NEG_Z, ..HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 1.0e6, TargetId::NONE, from) });
                                return;
                            }
                            crate::guards::NetVerdict::PutBack => {
                                // (IMPL_ER24) only to a place that is still supported and free (or, after a fall of 30 m, all the same:
                                // the wait for the ground holds him there on a level floor)
                                let verified = self.place_ok(anchor);
                                if safe && verified.is_none() && dropped < 3000.0 {
                                    if self.void_for < 2.0 {
                                        erlog!("net: no ground at all under him for {:.1} s, {:.0} cm below his last floor, but where he last stood, kh ({:.0}, {:.0}, {:.1}), is NOT VERIFIED (no walkable ground under it or his body overlaps something there): not put back yet (he is at 30 m down)", self.void_for, dropped, anchor.x, anchor.y, anchor.z);
                                    }
                                } else {
                                    self.net_last = Some((anchor, std::time::Instant::now()));
                                    erlog!(
                                        "sora: NO GROUND AT ALL under him for {:.1} s (he fell out of the world, {:.0} cm below his last floor): PUT BACK where he last stood ({:.0} cm above where he is), to wait there for the ground | from kh ({:.0}, {:.0}, {:.1}) to kh ({:.0}, {:.0}, {:.1}), the place {}",
                                        self.void_for, dropped, anchor.z - p.z, p.x, p.y, p.z, anchor.x, anchor.y, anchor.z,
                                        verified.map_or("NOT verified".to_string(), |dz| format!("verified: walkable ground {dz:+.1} cm from its feet, his body free there")),
                                    );
                                    self.void_for = 0.0;
                                    if let Some(f) = friend {
                                        // (ER party S2) the friend leader is put back (the body with him; Sora stays where he is)
                                        self.lead_order(crate::guards::LeadOrder::Follow(anchor, f.yaw));
                                        self.stood.seat(anchor);
                                        self.write_at(player, anchor, f.yaw);
                                        self.follow = Some(Follow::NoGround { since: 0.0 });
                                        self.ground_wait = crate::groundwait::Wait::start(true); // (ZETTA 2026-10-23)
                                        return;
                                    }
                                    self.sora.teleport(Pose { position: anchor, yaw: self.out.yaw });
                                    self.out.position = anchor;
                                    self.out_before.position = anchor;
                                    self.stood.seat(anchor);
                                    self.write(player);
                                    self.follow = Some(Follow::NoGround { since: 0.0 });
                                    self.ground_wait = crate::groundwait::Wait::start(true); // (ZETTA 2026-10-23)
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
        let Some(follow) = self.follow else { return };
        if before.is_none() {
            erlog!("sora: HE FOLLOWS THE BODY ({}): no simulation of his own moves it", match follow { Follow::Action => "the game's own action of the body", Follow::Grab => "an enemy's grab", Follow::Cutscene => "a cutscene", Follow::NoGround { .. } => "the game put its player here: waiting for the ground" });
            let friend = self.friend_lead(); // (ER party S2) a friend leading is stopped where he is (Sora is left alone)
            if let Some(f) = friend {
                self.lead_order(crate::guards::LeadOrder::Follow(f.position, body_yaw));
            } else if self.sora.status().hp > 0 {
                self.sora.teleport(Pose { position: self.out.position, yaw: body_yaw });
            }
            self.targets.puppets.release_all("Sora follows the body");
        }
        // seated on the body
        let shift = now - self.feet_er();
        // (ER-TWEAKS) the carry's trail for the fog walk (fogwalk.rs): what the game moved the body since the last frame of the
        // same action (the first frame's seat is a placement, not a step)
        if follow == Follow::Action && before == Some(Follow::Action) {
            self.fog_trail += coords::dir_er_to_kh(shift);
        }
        // (ER party S2) A FRIEND LEADING IS SEATED ON THE BODY: he is put there (held every party step), the KH3 frame stays
        // (Sora and the other friend stay where they are); Sora leading: the frame moves under him as before, and the members
        // who do not lead are moved back by as much (party/leader.rs `seated`)
        let seat_at = match self.friend_lead() {
            Some(f) => {
                let at = f.position + coords::dir_er_to_kh(shift);
                self.lead_order(crate::guards::LeadOrder::Follow(at, body_yaw));
                at
            }
            None => {
                if shift.length() > 1e-4 {
                    self.world.frame.rebase(shift);
                    #[cfg(feature = "party")]
                    self.lead.seated(shift);
                    if shift.length() > 0.5 {
                        self.targets.rebased();
                        self.fx.cut_trails(); // (IMPL_ER75) he was moved in the world: the trail's history is of the old place
                    }
                }
                self.out.position
            }
        };
        self.written = None;
        self.anchor = Some(seat_at);
        self.stood.seat(seat_at); // (IMPL_ER24)
        self.follow_yaw = Some(body_yaw);
        self.world.flat.set(None);
        if let Follow::NoGround { since } = follow {
            let p = seat_at;
            let under = kh_core::World::line_trace(&self.world, p + Vec3::Z * 20.0, p - Vec3::Z * (height + 300.0)).map(|h| h.position.z);
            let ground = under.is_some();
            // (WALLS load fix 2026-10-23) THE GAME'S OWN WORD for the fast rule (groundwait.rs `frame_with`): its body stands on solid ground
            let body_ground = {
                let ph = &player.chr_ins.modules.physics;
                (ph.standing_on_solid_ground || ph.touching_solid_ground) && !ph.is_falling && control.flag("load.fast") != Some(false)
            };
            // (WALLS load fix 2026-10-23) UNBURIED: held with no ground under him for a moment while a walkable floor is just ABOVE
            // his feet (the user's log: the body left 2.3 m under the floor by a load; every layer saw nothing below it): the body
            // is put on the lowest such floor (within 4 m), and the seat follows it next frame. Key `load.unbury = 0`: not.
            if !ground && self.ground_wait.since >= 0.5 && self.ground_wait.since_rebase >= 0.25 && control.flag("load.unbury") != Some(false) {
                let lift = [100.0f32, 200.0, 300.0, 400.0].into_iter().find_map(|up| {
                    kh_core::World::line_trace(&self.world, p + Vec3::Z * up, p + Vec3::Z * 1.0).filter(|h| h.impact_normal.z > 0.7).map(|h| h.position)
                });
                if let Some(floor) = lift {
                    let to = self.world.frame.pos_kh_to_er(Vec3::new(p.x, p.y, floor.z + 1.0));
                    erlog!("sora: (WALLS) NO GROUND UNDER the held leader but a floor {:.0} cm ABOVE his feet (he was left under it): put ON it at kh ({:.0}, {:.0}, {:.0})", floor.z - p.z, p.x, p.y, floor.z);
                    player.chr_ins.modules.physics.as_mut().position = game::hp(to);
                }
            }
            // (ZETTA 2026-10-23, the user's Malenia log: taken at 0.00 s after her phase cutscene, the origin shifted 2 s later,
            // no ground, put back where there was none either, let go blind after 8 s: his death) THE STRICT WAIT
            // (groundwait.rs): held where the game put him until real ground has answered for a moment, settled after the
            // cutscene / warp and after the last shift of the origin; never let go blind within the wait
            let _ = control;
            match self.ground_wait.frame_with(dt, self.rebased_now, ground, body_ground) {
                crate::groundwait::Verdict::Release => {
                    erlog!("sora: there is ground under him after {since:.2} s (confirmed for {:.2} s, {:.2} s after the last shift of the origin{}): SORA HAS THE BODY AGAIN", self.ground_wait.ground_for, self.ground_wait.since_rebase, if body_ground { "; the game's body stands: the fast rule" } else { "" });
                    // (WALLS load fix) FEET ON THE FLOOR: a seat a little under the floor our cast sees (the body's place a few cm in
                    // it) is lifted onto it before he moves (Sora leading; a friend leader lands by his own step)
                    if let (Some(floor), None) = (under, self.friend_lead()) {
                        let dz = floor - (p.z - height);
                        if dz > 1.0 && dz <= 30.0 {
                            let to = p + Vec3::Z * dz;
                            erlog!("sora: (WALLS) his feet were {dz:.1} cm under the floor at the release: put on it");
                            self.sora.teleport(Pose { position: to, yaw: self.out.yaw });
                            self.out.position = to;
                            self.out_before.position = to;
                        }
                    }
                    self.release_guard = crate::groundwait::GUARD_S;
                    // (WALLS cutscene fix) the body's place as read this frame stands for our last write: a placement the game makes in
                    // THIS physics step (a load, a cutscene's end or skip) is then seen in `post_physics` and adopted, not written over
                    self.written = Some(now);
                    // (WALLS cutscene fix) after a cutscene, a load or a warp: KH3's camera behind the leader at once, full weight
                    if std::mem::take(&mut self.cam_snap_due) && control.flag("cam.snap_back") != Some(false) {
                        self.cam.snap_back();
                    }
                    self.follow = None;
                    self.follow_yaw = None;
                    return;
                }
                crate::groundwait::Verdict::GiveUp => {
                    erlog!("sora: NO GROUND WAS FOUND under the place the game put him for {since:.0} s (its collision does not answer his casts): he is let go all the same");
                    self.written = Some(now); // (WALLS cutscene fix)
                    if std::mem::take(&mut self.cam_snap_due) && control.flag("cam.snap_back") != Some(false) {
                        self.cam.snap_back();
                    }
                    self.follow = None;
                    self.follow_yaw = None;
                    return;
                }
                crate::groundwait::Verdict::Hold => {
                    if self.ground_wait.tell() {
                        erlog!("sora: still held where the game put him after {since:.0} s (no fall): {}", if ground { "ground answers, it settles" } else { "no ground answers his casts yet (the place is still loading)" });
                    }
                }
            }
            self.follow = Some(Follow::NoGround { since: since + dt });
        }
        // (a level floor at his feet: he stands whatever the collision says)
        // (ER party S2) not for a friend leading: the flat floor is every cast's (AI Sora's too); his seat is held instead
        self.world.flat.set(if self.friend_lead().is_some() { None } else { Some(self.out.position.z - height) });
    }

    /// (IMPL_ER18) A FLOOR THAT MOVES CARRIES HIM (lifts). While he stands (his own floor flag) on a body owned by a DYNAMIC map
    /// object (owner type 6, MEASURED: `CSWorldGeomDynamicIns`: doors, lifts, fog) the height of that body's surface under his
    /// feet is read once a frame; when the same body's surface has moved up or down since the last frame (less than 0.5 m: not
    /// an origin shift, not another floor) the KH3 frame moves with it: in his own space he stands still on the platform, so he
    /// neither sinks into a rising floor nor drops and lands again and again on a sinking one. Vertical only (what Elden Ring's
    /// lifts do). UNTESTED: `ride = 1` switches it ON. Also (the log): every change of the body's ladder state.
    /// (ANIMS) ONCE A FRAME in `follow_phase`, after the ladder's belief (exitnet.rs `LadderTruth`): a STALE ladder module (it
    /// still says "ladder" while the body plays no ladder animation: the let-go's W_LadderDrop) gets its state -1 and its
    /// ladder handle emptied, once per episode (`ladder.repair = 0`: not written; the belief stays); a let-go whose ladder
    /// animation never ended while the leader stands gets the body's `W_Idle` once (`ladder.nudge = 0`: not).
    fn ladder_repair(&mut self, player: &mut PlayerIns, control: &Control) {
        let fix = std::mem::replace(&mut self.ladder_fix, crate::exitnet::Fix::None);
        let anim = crate::chranim::now(&player.chr_ins).id;
        match fix {
            crate::exitnet::Fix::Repair => {
                let was = ladder_state(player);
                let m = &player.chr_ins.modules.ladder;
                let (handle, flags) = (format!("{}", m.ladder_handle), m.flags);
                let write = control.flag("ladder.repair") != Some(false);
                if write {
                    free_ladder_module(player);
                }
                erlog!("!!! ladder: THE GAME'S LADDER MODULE IS STALE: state {was} {} for {:.1} s while the body plays no ladder animation ({anim}; handle {handle}, flags {flags:#x}): the ladder is OVER for us{} (#{})", crate::ladder::state_name(was), crate::exitnet::LadderTruth::STALE_S, if write { "; the module's state set to -1 and its handle emptied, so the game offers its prompts again" } else { " (ladder.repair = 0: the module is not written)" }, self.ladder_truth.repairs);
            }
            crate::exitnet::Fix::Nudge => {
                let fired = control.flag("ladder.nudge") != Some(false) && crate::chranim::fire(&mut player.chr_ins, "W_Idle");
                erlog!("!!! ladder: the body has played the ladder animation {anim} with no ladder state for {:.1} s while the leader stands: its graph asked for W_Idle ({})", crate::exitnet::LadderTruth::STUCK_S, if fired { "fired" } else { "NOT fired" });
            }
            crate::exitnet::Fix::None => {}
        }
    }

    /// (LADDER2) THE GAME STARTED ITS LADDER ON THE BODY (Y at its "Climb" prompt): THE LANDING read from the game's ladder
    /// module (its top: where its Tarnished stands when the climb up is over; its heading: the ladder's own, bottom to top
    /// across) and the let-go asked (`ladder_body`). A start from the top (StartTop: the climb down is not offered) or a
    /// module without a usable ladder: the body is let go, no leap.
    fn ladder_start(&mut self, player: &PlayerIns, state: i8, anim: i32) {
        let l = &player.chr_ins.modules.ladder;
        let (top_er, bottom_er) = (Vec3::new(l.top.0, l.top.1, l.top.2), Vec3::new(l.bottom.0, l.bottom.1, l.bottom.2));
        let (top, bottom) = (self.world.frame.pos_er_to_kh(top_er), self.world.frame.pos_er_to_kh(bottom_er));
        let physics = &player.chr_ins.modules.physics;
        let feet = self.world.frame.pos_er_to_kh(game::v3(&physics.position));
        let body_yaw = coords::yaw_kh_of_er_dir(game::quat(&physics.orientation) * Vec3::NEG_Z);
        let up = crate::ladder::from_bottom(state, feet, bottom, top);
        if up {
            self.ladder_go.start();
        } else {
            self.ladder_go.start_quiet();
        }
        let how = if state >= 0 { format!("the game's ladder state {state} {}", crate::ladder::state_name(state)) } else { "its approach, before the game's ladder state".to_string() };
        match crate::ladder::landing(bottom, top, body_yaw).filter(|_| up) {
            Some((to, facing)) => {
                self.leap_wait = Some((to, facing, 0.0));
                erlog!("ladder: (LADDER2) Y AT THE LADDER ({how}; animation {anim}): THE LEADER ({}) LEAPS UP IT, the party after him. The game's ladder: kh ({:.0}, {:.0}, {:.0}) to ({:.0}, {:.0}, {:.0}) (ER top ({:.3}, {:.3}, {:.3})); THE LANDING is its top, facing {facing:.0} (its heading; the body faced {body_yaw:.0}); the game is asked to let its body go", self.lead_chara_name(), bottom.x, bottom.y, bottom.z, top.x, top.y, top.z, top_er.x, top_er.y, top_er.z);
            }
            None => erlog!("ladder: (LADDER2) the game started its ladder ({how}; animation {anim}) {}: NO LEAP (a ladder state is let go)", if up { "BUT ITS MODULE HOLDS NO USABLE LADDER (top / bottom)" } else { "NOT AT ITS FOOT (from the top: the climb down is not offered, walk off the edge; or the module holds another ladder)" }),
        }
    }

    /// (LADDER2) ONCE A FRAME in `follow_phase`: THE GAME'S LET-GO OF ITS BODY (ladder.rs `LetGo`): `W_LadderDrop` as soon as
    /// its ladder module has a state, `W_LadderFallStart` if it kept the body, at the last the module written free and its
    /// graph asked for idle; after the let-go its graph is asked for idle while it still plays a ladder animation (its fall:
    /// no greyed prompts), and the let-go ends when the body is out of its ladder animations.
    fn ladder_body(&mut self, player: &mut PlayerIns, dt: f32) -> crate::ladder::LetStep {
        let state = if self.ladder_truth.stale() { -1 } else { ladder_state(player) }; // (ANIMS) as believed
        let anim = crate::chranim::now(&player.chr_ins).id;
        let ladder_anim = (crate::ladder::DEFAULT_ANIMS.0..=crate::ladder::DEFAULT_ANIMS.1).contains(&anim);
        let step = self.ladder_go.frame(state, dt);
        match step {
            crate::ladder::LetStep::Drop => {
                let fired = crate::chranim::fire(&mut player.chr_ins, "W_LadderDrop");
                erlog!("ladder: (LADDER2) the game's ladder state {state} {}: its body is asked to let go (W_LadderDrop {})", crate::ladder::state_name(state), if fired { "fired" } else { "NOT FIRED: the event function is not there" });
            }
            crate::ladder::LetStep::Retry => {
                let fired = crate::chranim::fire(&mut player.chr_ins, "W_LadderFallStart");
                erlog!("ladder: the body is still on its ladder {:.1} s after W_LadderDrop (state {state}): W_LadderFallStart asked ({})", crate::ladder::LetGo::RETRY_S, if fired { "fired" } else { "THE EVENT FUNCTION IS NOT THERE" });
            }
            crate::ladder::LetStep::LetGo => erlog!("ladder: the game LET ITS BODY GO after {:.2} s (ladder state -1, animation {anim}): the leader has it", self.ladder_go.for_s),
            crate::ladder::LetStep::Force => {
                free_ladder_module(player);
                let fired = crate::chranim::fire(&mut player.chr_ins, "W_Idle");
                erlog!("!!! ladder: the game did not let its body go in {:.1} s (state {state} {}, animation {anim}): its ladder module is written free and its graph asked for W_Idle ({}); the leader has it", self.ladder_go.for_s, crate::ladder::state_name(state), if fired { "fired" } else { "NOT fired" });
            }
            crate::ladder::LetStep::Wait => {}
        }
        if self.ladder_go.idle_due(ladder_anim, dt) {
            let fired = crate::chranim::fire(&mut player.chr_ins, "W_Idle");
            erlog!("ladder: the body let go still plays its ladder animation {anim}: its graph asked for W_Idle ({})", if fired { "fired" } else { "NOT fired" });
        }
        if self.ladder_go.over(ladder_anim) {
            self.ladder_go.end();
            // (MAP-GATE, the user 2026-11-08: "using a ladder disables anything to do with the map") what the cut-short ladder
            // leaves in the game's own map gate is said, and an action-button press still waiting (the ladder's "Climb" is
            // carried out by now) is ended as the game ends one it carried out (mapgate.rs)
            let gate = crate::mapgate::read(player);
            let ended = gate.action_press_waiting == Some(true) && crate::mapgate::consume_action_press();
            erlog!("ladder: (MAP-GATE) the let-go is over: the game's own map gate: {}{}", gate.text(), if ended { "; the action-button press that waited was ENDED" } else { "" });
        }
        step
    }

    /// (LADDER2) The landing read again when the game's ladder state comes (the start may have been its approach): the
    /// module then holds this ladder for sure.
    fn landing_refresh(&mut self, player: &PlayerIns) {
        let Some((to, facing, waited)) = self.leap_wait else { return };
        let l = &player.chr_ins.modules.ladder;
        let (top, bottom) = (self.world.frame.pos_er_to_kh(Vec3::new(l.top.0, l.top.1, l.top.2)), self.world.frame.pos_er_to_kh(Vec3::new(l.bottom.0, l.bottom.1, l.bottom.2)));
        let body_yaw = coords::yaw_kh_of_er_dir(game::quat(&player.chr_ins.modules.physics.orientation) * Vec3::NEG_Z);
        if let Some((now, heading)) = crate::ladder::landing(bottom, top, body_yaw) {
            if now.distance(to) > 1.0 || (heading - facing).abs() > 0.5 {
                erlog!("ladder: (LADDER2) the landing read again at the game's ladder state: kh ({:.0}, {:.0}, {:.0}) facing {heading:.0} (was ({:.0}, {:.0}, {:.0}) facing {facing:.0})", now.x, now.y, now.z, to.x, to.y, to.z);
            }
            self.leap_wait = Some((now, heading, waited));
        }
    }

    /// (LADDER2) THE LEAP STARTS once the game let its body go (or `LAUNCH_S` after the start at the latest): the leader's
    /// (Sora: his leap on his kh-core place; a friend: his pawn's ladder jump, `LeadOrder::LadderJump`) and the party's
    /// right after him (party/glue.rs `ladder_jump`: the AI friends and AI Sora beside the landing).
    fn leap_launch(&mut self, dt: f32) {
        let Some((to, facing, waited)) = self.leap_wait.as_mut() else { return };
        *waited += dt;
        if !self.ladder_go.body_free() && *waited < crate::ladder::LAUNCH_S {
            return;
        }
        let (to, facing, waited) = (*to, *facing, *waited);
        self.leap_wait = None;
        let from = self.lead_pos();
        if self.friend_lead().is_some() {
            self.lead_order(crate::guards::LeadOrder::LadderJump { to, facing });
        } else {
            let h = Sora::origin_height_on_floor(&SORA_PARAMS);
            self.leap = Some(crate::ladder::SoraLeap::new(kh_core::leap::Leap::new(self.out.position, self.out.yaw, to + Vec3::Z * h, facing, 0.0)));
        }
        #[cfg(feature = "party")]
        if let Some(core) = self.party.core_mut() {
            core.ladder_jump(to, facing);
        }
        erlog!("ladder: (LADDER2) THE LEAP: the leader from kh ({:.0}, {:.0}, {:.0}) up {:.0} cm onto ({:.0}, {:.0}, {:.0}) facing {facing:.0} ({waited:.2} s after the start; {})", from.x, from.y, from.z, to.z - from.z, to.x, to.y, to.z, if self.ladder_go.body_free() { "the game let its body go" } else { "THE GAME HAS NOT LET ITS BODY GO YET: it follows when it does" });
    }

    /// (SPIRITSPRING) ONE STEP OF THE SPIRITSPRING WATCH (spiritspring.rs), before the leader's step: the lift asked of his core
    /// for this step (Sora: `Sora::spring_lift`; a friend: `LeadOrder::SpringLift`), the spring's launch sound when his core
    /// takes it, and the allies' jump after him when he lands higher (party/glue.rs `spring_catch_up`).
    fn spring_step(&mut self, here: Option<usize>, on_foot: bool, control: &Control, dt: f32) {
        use crate::spiritspring::{self as spring, rules};
        let friend = self.friend_lead();
        let (feet, vz) = match friend {
            Some(f) => (f.position, f.velocity.z),
            None => (self.out.position - Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS), self.out.velocity.z),
        };
        let who = self.lead_chara();
        let leader = rules::LeaderIn { on_foot, on_floor: self.lead_on_floor(), feet, vz, who: who as u8 };
        let step = self.spring.step(leader, here, spring::launch_cm, dt);
        let log = control.flag("spring.log") != Some(false);
        if let Some(height) = step.lift {
            if friend.is_some() {
                self.lead_order(crate::guards::LeadOrder::SpringLift(height));
            } else {
                self.sora.spring_lift(height);
            }
        }
        if let Some(r) = step.confirmed {
            let played = crate::ersound::spring_launch(self.world.frame.pos_kh_to_er(r.takeoff));
            if log {
                erlog!("spring: {} JUMPS AT THE SPIRITSPRING {}: his jump rises {:.0} cm above the take-off (Torrent's: JumpHeight + 7 m), launch sound c{} {}", who.name(), spring::describe(r.spring), r.height, crate::ersound::TORRENT_SPRING_LAUNCH, if played { "played by the game" } else { "NOT PLAYED (the game's sound play is not available)" });
            }
        }
        if let Some((r, end)) = step.ended {
            if log && (r.confirmed || end != rules::End::Refused) {
                erlog!("spring: {}'s spiritspring lift ended ({end:?}) {:.2} s after the take-off, {:.0} cm above it (aim {:.0})", who.name(), r.t, feet.z - r.takeoff.z, r.height);
            }
        }
        if let Some((r, landing)) = step.landed {
            let follow = rules::allies_follow(&r, landing);
            #[cfg(feature = "party")]
            if follow {
                let facing = self.lead_yaw();
                if let Some(core) = self.party.core_mut() {
                    core.spring_catch_up(landing, facing, r.takeoff, rules::ALLY_RADIUS_CM);
                }
            }
            if log {
                let reach = if follow { format!(": the party members within {:.0} m of the take-off jump after him (the ladder jump's leap), the others by their catch-up / warp", rules::ALLY_RADIUS_CM / 100.0) } else { String::new() };
                erlog!("spring: {} landed {:.0} cm above the spiritspring take-off{reach}", who.name(), landing.z - r.takeoff.z);
            }
        }
    }

    /// (LADDER2) AFTER A STEP OF SORA'S LEAP (the leader's or AI Sora's): his take-off plays Torrent's wind (ersound.rs, the
    /// game's own sound at his feet); his landing puts his kh-core state there, facing the landing's heading.
    fn leap_after_step(&mut self, out: &Output) {
        let Some(l) = self.leap.as_mut() else { return };
        if std::mem::take(&mut l.leap.took_off) {
            let feet = out.position - Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
            let played = crate::ersound::ladder_jump(self.world.frame.pos_kh_to_er(feet));
            erlog!("ladder: (LADDER2) Sora leaves the floor: Torrent's wind (c{}) {}", crate::ersound::TORRENT_WIND_JUMP, if played { "played by the game" } else { "NOT PLAYED (the game's sound play is not available)" });
        }
        if l.leap.done() {
            let (to, facing) = (l.leap.to, l.leap.facing);
            self.leap = None;
            self.leap_show = crate::ladder::Show::Own;
            self.sora.teleport(Pose { position: to, yaw: facing });
            erlog!("ladder: (LADDER2) Sora landed on the ladder's top at kh ({:.0}, {:.0}, {:.0}) facing {facing:.0}", to.x, to.y, to.z);
        }
    }

    /// The leader's name (the log).
    fn lead_chara_name(&self) -> &'static str {
        #[cfg(feature = "party")]
        return self.lead.chara().name();
        #[cfg(not(feature = "party"))]
        "Sora"
    }

    fn ride_phase(&mut self, player: &PlayerIns, control: &Control) {
        let ladder = ladder_state(player);
        if ladder != self.ladder_seen {
            let ta = &player.chr_ins.modules.time_act;
            erlog!("own: the body's LADDER STATE {} -> {ladder} {} (animation {}; Sora {:?}, following {}; the body at height {:.2})", self.ladder_seen, crate::ladder::state_name(ladder), ta.anim_queue[(ta.read_idx as usize) % 10].anim_id, self.out.state, self.follow.is_some(), game::v3(&player.chr_ins.modules.physics.position).y);
            self.ladder_seen = ladder;
        }
        if self.follow.is_some() || !self.lead_on_floor() /* (ER party S2) the leader */ || control.flag("ride") == Some(false) { // ON (the user rode the Stranded Graveyard lift with it, build 4B6C2CFEF72E; without it the lift went through him); `ride = 0` switches it off
            self.ride = None;
            return;
        }
        let feet = self.feet_er();
        let mut hits = Vec::new();
        crate::havok::cast_sphere(feet + Vec3::Y * 0.6, Vec3::NEG_Y * 1.2, 0.1, self.world.filter, false, &mut hits);
        hits.retain(|h| h.fraction.is_finite() && h.normal.y > 0.7);
        hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
        let Some(floor) = hits.first().filter(|h| h.owner == 6) else {
            self.ride = None;
            return;
        };
        if let Some((body, y)) = self.ride {
            let dy = floor.position.y - y;
            if body == floor.body && dy.abs() > 0.0005 && dy.abs() < 0.5 {
                // (ER party S2) a friend leading rides it himself (the frame stays: Sora and the other friend are not lifted);
                // Sora leading: the frame moves as before and the members who do not lead are moved back by as much
                if self.friend_lead().is_some() {
                    self.lead_order(crate::guards::LeadOrder::Nudge(coords::dir_er_to_kh(Vec3::Y * dy)));
                } else {
                    self.world.frame.rebase(Vec3::Y * dy);
                    // (ER party S2) the members standing on the same moving body ride it with him (no counter-move for them)
                    #[cfg(feature = "party")]
                    {
                        let riders: Vec<u8> = match self.party.core() {
                            Some(core) => core
                                .rt
                                .current
                                .iter()
                                .filter(|v| Some(v.id) != core.rt.leader && v.on_ground)
                                .filter(|v| {
                                    let mut under = Vec::new();
                                    crate::havok::cast_sphere(self.world.frame.pos_kh_to_er(v.position) + Vec3::Y * 0.6, Vec3::NEG_Y * 1.2, 0.1, self.world.filter, false, &mut under);
                                    under.retain(|h| h.fraction.is_finite() && h.normal.y > 0.7);
                                    under.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
                                    under.first().is_some_and(|h| h.body == floor.body)
                                })
                                .map(|v| v.id.0)
                                .collect(),
                            None => Vec::new(),
                        };
                        self.lead.seated_riding(Vec3::Y * dy, &riders);
                    }
                }
                if self.rode == 0.0 {
                    erlog!("ride: THE FLOOR UNDER HIM MOVES ({}; body {}): he is carried with it", crate::wallscan::owner_text(floor.owner_ptr), floor.body);
                }
                self.rode += dy;
                self.rode_frames += 1;
            } else if self.rode_frames > 0 && dy.abs() <= 0.0005 {
                self.rode_still += 1;
                if self.rode_still > 30 {
                    erlog!("ride: the floor stands again: he was carried {:+.2} m in {} frame(s)", self.rode, self.rode_frames);
                    (self.rode, self.rode_frames, self.rode_still) = (0.0, 0, 0);
                }
            }
        }
        self.ride = Some((floor.body, floor.position.y));
    }
    fn feet_er(&self) -> Vec3 {
        self.world.frame.pos_kh_to_er(self.lead_pos()) // (ER party S2) the body's leader's feet (Sora's as before)
    }

    /// (IMPL_ER14, the log only) THE STATES' GROUND PROBE as kh-core/src/states/common.rs makes it (a sphere of 50 from
    /// 55 above the actor origin, straight down 800; PreDecide calls a hit within 30 "on the ground"), said in words.
    fn ground_probe(&self) -> String {
        let p = &SORA_PARAMS;
        let actor = self.out.position;
        let start = actor + Vec3::Z * p.probe_start_height;
        let limit = p.probe_max_slope.to_radians().cos();
        let hits = kh_core::World::sweep_capsule_all(&self.world, start, start - Vec3::Z * 800.0, p.probe_sphere_radius, p.probe_sphere_radius);
        let mut steep = Vec::new();
        for hit in &hits {
            if hit.start_penetrating {
                return format!("STARTS INSIDE the ground by {:.1} cm (normal z {:.2}) => AIR", hit.penetration_depth, hit.impact_normal.z);
            }
            let distance = actor.z - hit.position.z;
            let slope = hit.impact_normal.z.clamp(-1.0, 1.0).acos().to_degrees();
            if hit.impact_normal.z > limit {
                return format!("floor {distance:+.1} cm below his origin, slope {slope:.0} deg{} => {}", if steep.is_empty() { String::new() } else { format!(" (after {} steeper contact(s): {})", steep.len(), steep.join(", ")) }, if distance <= 30.0 { "GROUND" } else { "AIR" });
            }
            steep.push(format!("{slope:.0} deg at {distance:+.0}"));
        }
        format!("no walkable floor within 800 ({} steep contact(s){}) => AIR", steep.len(), if steep.is_empty() { String::new() } else { format!(": {}", steep.join(", ")) })
    }

    /// `ChrIns_PrePhysics`.
    pub fn pre_physics(&mut self, player: &mut PlayerIns, dt: f32, control: &Control) {
        let started = std::time::Instant::now();
        self.frame_dt = dt.clamp(0.0, 0.25); // (DTH) the death flow's clock
        crate::mimic_music::sora_frame(self.died); // (MIMIC-MUSIC) Sora's frames run; the leader down (death.rs latch) ends the fight's music
        // (experiments) `w.filter = 0x...`: the filter word the casts are made with; `w.reject =
        // 0x..,0x..`: filter words of bodies that do not block him.
        let hex = |s: &str| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
        self.leg_ik = control.flag("ik.legs") != Some(false); // (IMPL_ER23A)
        self.world.filter = control.str("w.filter").and_then(hex).unwrap_or(crate::havok::filter_of_layer(self.targets.sheet.collision_layer)); // (WALLS) the Tarnished's own layer (sheet collision.layer = 2)
        *self.world.reject.borrow_mut() = control.str("w.reject").map(|s| s.split(',').filter_map(hex).collect()).unwrap_or_default();
        // (IMPL_ER18) the widest cast: the sheet's `collision.radius` (`w.radius = cm` overrides; 60 = KH3's own capsule)
        self.world.radius_cap.set(control.f32("w.radius").filter(|r| *r >= 5.0).unwrap_or(self.targets.sheet.collision_radius));
        // (IMPL_ER23A) the adapter's answers to Elden Ring's geometry (sweep.rs `Options`): `w.edge = 0` the separating direction
        // as the surface normal of an edge again, `w.mtd = 0` the deepest overlap alone, `w.gap = cm` (0 = off; sheet `collision.gap`)
        // (IMPL_ER24) `w.floor = hex layer` (0 = none; sheet `collision.floor_layer`): the FLOORS of the map's other collision mesh
        // are taken from a second cast with that layer; `w.foot = cm` (0 = off; sheet `collision.foot`): the foot rule;
        // `w.lift = cm`: vertical lines start that much higher; `w.clamp = 0`: a push out of an overlap is not cut short.
        let floor_layer = control.str("w.floor").and_then(hex).unwrap_or(self.targets.sheet.collision_floor_layer) & 0x7f;
        self.world.floor_filter.set(if floor_layer == 0 || floor_layer == self.world.filter & 0x7f { 0 } else { (self.world.filter & !0xFF) | floor_layer });
        if floor_layer != self.floor_told {
            self.floor_told = floor_layer;
            if floor_layer == 0 {
                erlog!("world: (WALLS) the KH characters' casts (the leader's and the friends') use layer {:#x}{}; no second floor layer (w.floor = 0)", self.world.filter & 0xFF, if self.world.filter == crate::havok::FILTER_PLAYER { " = THE TARNISHED'S OWN (his character proxy's word, read in the game's code): what blocks him blocks them, the invisible walls (layer 0x35) included" } else { " (w.filter: NOT the Tarnished's own layer 2)" });
            } else {
                erlog!("world: FLOORS ALSO FROM LAYER {floor_layer:#x} (the map's other collision mesh, the one characters stand on; walls of it are not taken); his own layer {:#x}; foot rule radius {} cm (w.floor = 0 / w.foot = 0 switch them off)", self.world.filter & 0xFF, control.f32("w.foot").unwrap_or(self.targets.sheet.collision_foot));
            }
        }
        self.world.opts.set(crate::sweep::Options {
            edge_faces: control.flag("w.edge") != Some(false),
            combine_overlaps: control.flag("w.mtd") != Some(false),
            gap: control.f32("w.gap").filter(|g| (0.0..=30.0).contains(g)).unwrap_or(self.targets.sheet.collision_gap),
            floor_layer: floor_layer != 0,
            foot: control.f32("w.foot").filter(|g| (0.0..=60.0).contains(g)).unwrap_or(self.targets.sheet.collision_foot),
            lift: control.f32("w.lift").filter(|g| (0.0..=20.0).contains(g)).unwrap_or(crate::sweep::Options::DEFAULT.lift),
            clamp: control.flag("w.clamp").unwrap_or(crate::sweep::Options::DEFAULT.clamp),
        });
        // The game moved the player since our last write: a small move is a push we overrule; a
        // large one is the physics origin shifting (or a warp by the game), which Sora follows.
        let now = game::v3(&player.chr_ins.modules.physics.position);
        let mut relocated = false;
        let mut origin_moved = false; // (THE IN-GAME TESTER 2026-11-03) the body's own fall watch moves with the origin
        if let Some(written) = self.written {
            let moved = now - written;
            if moved.length() > 4.0 {
                self.world.frame.rebase(moved);
                self.rebased_now = true;
                self.targets.rebased();
                relocated = !game::origin_shift(moved);
                origin_moved = !relocated;
                erlog!("sora: the player was moved by ({:.3}, {:.3}, {:.3}): {}; the KH3 frame is rebased", moved.x, moved.y, moved.z, if relocated { "THE GAME PUT ITS PLAYER SOMEWHERE ELSE (a warp): Sora waits there for the ground" } else { "the physics origin shifted" });
            } else {
                self.pushed = self.pushed.max(moved.length());
            }
        }

        if let Some(text) = self.kh_shot.fired(control) {
            let v: Vec<f32> = text.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            if let [x, y, z] = v[..] {
                let to = Vec3::new(x, y, z);
                erlog!("dbg: Sora is put at KH3 ({x:.0}, {y:.0}, {z:.0}) (dbg.kh; he was at ({:.0}, {:.0}, {:.0}))", self.out.position.x, self.out.position.y, self.out.position.z);
                self.sora.teleport(Pose { position: to, yaw: self.out.yaw });
                self.out.position = to;
                self.out_before.position = to;
                self.write(player);
            }
        }
        // (WALLS load fix) the game's own placement adopted after the last physics step: a relocation (the wait for the ground,
        // the load-in gate), but not the strict one: the game put its body on its ground, so ground at once is enough
        let placed = std::mem::take(&mut self.relocated_post);
        let relocated = relocated || placed;
        self.follow_phase(player, control, now, relocated, dt); // (IMPL_ER10) cutscenes, the wait for the ground, the net
        if placed && matches!(self.follow, Some(Follow::NoGround { .. })) {
            self.ground_wait.strict = false;
        }
        #[cfg(feature = "party")]
        if self.follow.is_none() {
            // (GRABS) a friend leader getting up after a grab (KH3's CatchEnd, partydraw.rs) stays where the throw let him go
            match self.friend_lead().filter(|_| crate::grabs::friend_caught(self.lead.chara())) {
                Some(f) => self.lead_order(crate::guards::LeadOrder::Follow(f.position, f.yaw)),
                None => self.lead.release(), // (ER party S2) the body leads the friend leader no more: his seat is let go
            }
        }
        self.ride_phase(player, control); // (IMPL_ER18) a floor that moves (a lift) carries him

        // (testing aid) `sora.warp = x,y,z`: once per change of the value Sora is put at that Elden
        // Ring position (the KH3 frame is moved; his own state is untouched, he lands by himself).
        let warp = control.str("sora.warp").map(str::to_string);
        if self.warped.is_none() {
            self.warped = Some(warp.clone());
        }
        if Some(&warp) != self.warped.as_ref() {
            if let Some(text) = warp.as_deref() {
                // (a leading + : relative to where he is; the physics origin shifts under absolute values)
                let relative = text.trim_start().starts_with('+');
                let v: Vec<f32> = text.trim_start().trim_start_matches('+').split(',').filter_map(|p| p.trim().parse().ok()).collect();
                if let [x, y, z] = v[..] {
                    let shift = if relative { Vec3::new(x, y, z) } else { Vec3::new(x, y, z) - self.feet_er() };
                    self.world.frame.rebase(shift);
                    self.targets.rebased();
                    self.targets.puppets.release_all("Sora was warped: the KH3 frame moved under the enemies");
                    erlog!("sora: warped {} Elden Ring ({x:.3}, {y:.3}, {z:.3})", if relative { "by" } else { "to" });
                }
            }
            self.warped = Some(warp);
        }
        // (LDI) THE LOAD-IN GATE (loadin.rs): under the loading screen (the game's in-game step not called) the party is held;
        // when it lifts and the leader stands, the friends are placed in that frame; the window after it (the game's fade-in)
        let loadin = if control.flag("loadin") != Some(false) {
            let settled = !matches!(self.follow, Some(Follow::NoGround { .. }));
            self.loadin.settle = control.f32("loadin.settle").filter(|s| s.is_finite()).map_or(crate::loadin::LoadIn::SETTLE, |s| s.clamp(0.0, 10.0));
            let out = self.loadin.frame(crate::loadin::In { relocated, world_step: crate::pause::hooked().then(crate::pause::available), settled, dt });
            if let (Some(line), true) = (out.line.as_ref(), control.flag("loadin.log") != Some(false)) {
                erlog!("{line}");
            }
            out
        } else {
            crate::loadin::Out::default()
        };
        // (ER party S1) the party's frame: party.er, a load / warp / cutscene (hidden, then put beside the leader), the body's
        // own actions (the catch-up after a fog wall), the AI's camera
        #[cfg(feature = "party")]
        {
            let hidden = match self.follow {
                // (LDI) the loading screen: held (parked, not stepped, not drawn), placed beside the leader when it lifts
                _ if loadin.hold && control.flag("loadin.hold") != Some(false) => Some("the loading screen (placed when it lifts)"),
                _ if self.cutscene => Some("a real-time cutscene"),
                Some(Follow::Cutscene) => Some("a real-time cutscene"),
                Some(Follow::NoGround { .. }) => Some("the wait for the ground after a load or a warp"),
                _ => None,
            };
            let action = self.follow == Some(Follow::Action);
            self.party.frame(player, control, &self.world.frame, crate::party::BodyFrame { relocated, hidden, body_action: action }, dt, self.clock as f64);
        }

        // ---- Elden Ring's interface: a menu has the input; the world stands still (a tutorial popup)
        let ui = inputs::read_ui(control.flag("hud.hide") == Some(true));
        let ui = crate::hud::er_ui(ui, control); // (hud) the game's own state while `hud.er = 0` holds its HUD hidden
        crate::loot::note_er_menu(ui.menu); // (PU) a window of the game's (a merchant's): the gate reads a purchase by it
        // (IMPL_ER10) a real-time cutscene: as a menu of the game's (no input to Sora, the whole pad is the game's: its
        // own skip prompt works), and Sora's HUD is hidden
        let ui = if self.cutscene { inputs::ErUi { menu: true, cutscene: true, ..ui } } else { ui };
        let ui = if self.savepoint.holds_pad() { inputs::ErUi { menu: true, ..ui } } else { ui }; // (IMPL_ER36) a driven story rest: Melina's talk has the pad, Sora gets none
        // (IMPL_ER17) the body's own action: the pad is the game's meanwhile; Sora's HUD stays
        let er_menu_own = ui.menu; // (ERUI) a menu / window / cutscene of the game's own, before the action's rule below
        let ui = if self.follow == Some(Follow::Action) { inputs::ErUi { menu: true, ..ui } } else { ui };
        // (IMPL_ER13) A WINDOW OF ELDEN RING'S STANDS ITS WORLD STILL (a tutorial window, any modal the interface state
        // does not tell us about): THE CAUSE OF "every prompt was greyed out": such a window is closed with the Menu
        // button, which was Sora's (the Kingdom Hearts menu opened instead), so with a pad it could never be closed,
        // and Sora ran around a standing world in which nothing of the game answers. Now: the game's own world flag
        // (pause.rs: the in-game step computes it) off for 6 frames while no menu of ours holds it = a menu of the
        // game's: the whole pad is Elden Ring's, Sora gets no input and stands. `in.er_modal = 0` switches the rule off.
        let ui = {
            static STILL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let standing = crate::pause::world_runs() == Some(false) && !crate::pause::holding() && !crate::hud::menu_open() && !crate::hud::gameover_holds();
            let frames = if standing { STILL.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1 } else { STILL.store(0, std::sync::atomic::Ordering::Relaxed); 0 };
            if frames == 6 {
                erlog!("ui: ELDEN RING'S WORLD STANDS STILL AND NO MENU OF OURS HOLDS IT (a window of the game's is up: a tutorial, a message): the whole pad is Elden Ring's until it runs again (interface state said menu {})", ui.menu);
            }
            if frames >= 6 && control.flag("in.er_modal") != Some(false) { inputs::ErUi { menu: true, ..ui } } else { ui }
        };
        // (IMPL_ER23A, the user: "Sora attacks when pressing A when making a selection in elden ring dialogue") A SELECTION WINDOW OF
        // ELDEN RING'S THAT LEAVES ITS HUD STATE AT GAMEPLAY (an NPC's talk list, a choice box): the user's log shows 23 s of
        // talk with hud_state 3 = "Sora has the pad": our mask hid A from the game and every A was his attack. While the game's
        // popup menu has a top job (guards.rs `DialogWatch`) the whole pad is Elden Ring's, as for any menu of its.
        // `in.er_dialog = 0` switches the rule off; `log.popup = 0` silences the lines.
        let ui = {
            let popup = inputs::popup_state();
            if popup != self.popup {
                if control.flag("log.popup") != Some(false) {
                    let text = |p: Option<crate::guards::PopupState>| p.map_or("none".to_string(), |p| format!("top job {:#x}, talk id {}", p.top_job, p.talk_id));
                    erlog!("ui: Elden Ring's popup menu: {} -> {} (hud_state {}, prompt {}, Sora {:?})", text(self.popup), text(popup), ui.hud_state, if self.prompt { "up" } else { "not up" }, self.out.state);
                }
                self.popup = popup;
            }
            let gave_up = self.dialog.given_up;
            let up = popup.is_some_and(|p| self.dialog.frame(p, 1.0 / 60.0)) && control.flag("in.er_dialog") != Some(false);
            if self.dialog.given_up && !gave_up {
                erlog!("!!! ui: Elden Ring's popup menu has had a top job for {:.0} s: THAT IS NO SELECTION WINDOW: the rule is off for this session (Sora has the pad again)", crate::guards::DialogWatch::LIMIT);
            }
            if up != self.dialog_up {
                self.dialog_up = up;
                erlog!("ui: {}", if up { "A SELECTION WINDOW OF ELDEN RING'S IS UP (its popup menu has a top job while the HUD state says gameplay): the whole pad is Elden Ring's, Sora gets none" } else { "Elden Ring's selection window is gone: Sora has the pad again (buttons still held are not his until released)" });
            }
            if up { inputs::ErUi { menu: true, ..ui } } else { ui }
        };
        if ui != self.ui {
            erlog!("ui: Elden Ring's hud_state {} -> {}: {}", self.ui.hud_state, ui.hud_state, if ui.menu { "a menu / cutscene has the input: Sora gets none, Elden Ring sees the whole pad" } else { "gameplay: Sora has the pad" });
            self.ui = ui;
        }
        // (ERUI) A BORROWED INTERACTION (the body's own action: Follow::Action, own_hold): Start = KH3's Pause screen, the game
        // sees no Start / View (hud.rs `borrowed_pad`); a window of the game's own besides it keeps the pad as before
        let borrowed_pad = {
            let borrowed = (self.follow == Some(Follow::Action) || self.own_hold) && control.flag("in.action_pause") != Some(false) && control.on("hud.kh") && !ui.cutscene; // (NO-ER-PAUSE) Start is always the KH menu's (`in.kh_menu` retired)
            let standing = crate::pause::world_runs() == Some(false) && !crate::pause::holding() && !crate::hud::menu_open();
            crate::hud::borrowed_pad(borrowed, er_menu_own || self.dialog_up || standing, crate::hud::menu_open())
        };
        crate::hud::set_borrowed(borrowed_pad.on);
        // (IMPL_ER23A) fast travel only from the map a save point opened (`travel.savepoint_only = 0`: the game's own rules)
        self.fast_travel.frame(control.flag("travel.savepoint_only") != Some(false), ui.menu && !ui.cutscene, relocated, 1.0 / 60.0);
        self.watch_ui(control);
        let paused = self.world_paused(player);
        let paused = paused || crate::hud::menu_open(); // (hud) a menu of ours holds the pause: no steps
        self.fx.frame(player, control, dt, paused, &self.world.frame); // (fx) surface, listener, switches, pause
        // (AUDIT-A CAMS) KH3's dark screen of every finish (0x14140DF80: the master light x 0.5 over 1 s, back over 1 s): the
        // late pass multiplies the lit scene by it (kh-er-render darkscreen.rs); `r.dark_screen = 0`: off
        kh_er_render::darkscreen::frame(self.sora.combat().form.dark_screen && control.flag("r.dark_screen") != Some(false), if paused { 0.0 } else { dt });
        if self.cutscene && control.flag("cut.savepoint") != Some(false) {
            self.fx.cutscene_step(&self.out, &self.sora, dt); // (IMPL_ER33) the save points' effects live through a cutscene
        }

        // ---- the targets of this frame, the testing keys, and what the game did to its player
        self.host_keys(control);
        let feet_now = self.feet_er();
        // THE HIT EVENTS ARE PER FRAME: what the steps of this frame deliver is in `targets.events`
        // until the next frame's simulation starts (`hit_events`).
        self.targets.events.clear();
        self.targets.refresh(player, self.world.frame, feet_now, dt);
        self.targets.sora = self.lead_pos(); // (GF-INGAME) the leader's: Sora out of the party stood where he was left, and "nearest" was read from there
        // (IMPL_ER32) breakable map objects near him and near his spells; the next bullet at one (`break.kh`)
        if !paused {
            let actors: Vec<(Vec3, Vec3)> = self.sora.magic().projectiles.iter().map(|p| (p.position, p.velocity)).collect();
            crate::breakables::frame(&mut self.targets, control, &actors, dt);
        }
        // (IMPL_ER10) the scripted death's place (sheet rows death.scripted.*): the tutorial boss is here
        {
            let rule = self.targets.params.scripted_death;
            let block = player.current_block_id;
            crate::hud::set_scripted_death(block.area() as i32 == rule[1] && block.block() as i32 == rule[2] && self.targets.list.iter().any(|e| e.npc_id == rule[0]));
        }
        self.targets.bullets.tick(dt);
        // The enemy puppet (puppet.rs; `puppet = 0` switches it off).
        self.targets.puppets.off = control.flag("puppet") == Some(false);
        self.targets.puppets.ai_bits = control.i64("pup.ai").map_or(3, |v| v as u32);
        self.targets.puppets.draw = control.on("pup.draw");
        self.targets.puppets.rules = self.targets.params.anim;
        // (IMPL_ER21) `react.guard = 0`: bosses as before this pass (bossguard.rs)
        self.targets.puppets.guard = Some(if control.flag("react.guard") == Some(false) { crate::bossguard::GuardRules::OFF } else { self.targets.params.guard });
        // (REVENGE) KH3's revenge and hit-stun of the bodies on Elden Ring's boss bar (bossrevenge.rs; off with `react.guard = 0`)
        self.targets.puppets.boss = Some(if control.flag("react.guard") == Some(false) { crate::bossrevenge::BossRules::OFF } else { self.targets.params.boss });
        self.targets.puppets.bosses = crate::erhud::boss_bar_handles();
        self.targets.puppets.no_anim = control.flag("pup.anim") == Some(false);
        // (IMPL_ER45) `poise.kh = 0`: every hit on an ordinary enemy reacts, as before (poise.rs)
        self.targets.puppets.poise = Some(if control.flag("poise.kh") == Some(false) { crate::poise::PoiseRules::OFF } else { self.targets.params.poise });
        self.targets.cast_filter = self.world.filter;
        self.rebased_now = false;
        self.watch_step(dt);
        self.debug_keys(control);
        self.lab_step(control, dt);
        self.fall.frame(self.lead_on_floor(), self.lead_pos().z, dt); // (IMPL_ER23A) the fall guard's window (ER party S2: the leader's)
        {
            // (THE IN-GAME TESTER 2026-11-03) the hidden body's own drops, a warp that put it lower among them (guards.rs)
            let ph = &player.chr_ins.modules.physics;
            if self.body_fall.frame(ph.standing_on_solid_ground || ph.touching_solid_ground, now, origin_moved, dt) {
                erlog!("fall: THE HIDDEN BODY came down {:.1} m (a fall of its own or a warp that put it lower; the leader {}): for {:.2} s what the game takes off it with nobody named is that fall's, never an enemy's", self.body_fall.depth, if self.lead_on_floor() { "stands" } else { "is off the floor" }, crate::guards::FALL_WINDOW);
            }
        }
        self.body_watch(player, control, dt);
        crate::status::frame(player, control, dt); // (IMPL_ER48) Elden Ring's ailments on the body: ticks, clearing
        // (GF-SENSOR) what the enemies are doing, before the hits are read
        crate::telegraph::frame(control, player, &self.targets, self.lead_pos(), dt);
        self.party_marks(control); // (HUD-REVIVE-COMBAT) the party's places and the enemies whose AI targets one of them
        crate::stance::frame(&mut self.targets, dt);
        crate::aipause::frame(&self.targets, dt); // (GF-POLISH)
        crate::rebound::frame(&mut self.targets, dt); // (THE USER 2026-11-02) swings bounced off Goofy's shield
        if crate::sensorlab::frame(control, player, &mut self.targets, dt) {
            self.sora.status_request(kh_core::StatusRequest::FullRecover);
        }
        self.damage_in(player, control);
        crate::nocombat::frame(control); // (NOCOMBAT) Elden Ring's own "no attacking here" state -> every KH character's rule, before any steps
        // (THE USER 2026-11-02: "Give Sora the ability to 'Summon' from the command menu, but he uses Elden Ring's spirit
        // ashes") the Summon command's Elden Ring side (spirit.rs): his ashes, the bell, the spirit, its timer and leaving;
        // the view the core reads in this frame's steps. Battle = the KH HUD's rule (the leader's battle flag, or the forced one).
        {
            #[cfg(feature = "party")]
            let in_battle = crate::conv::party_scene(self.party.core(), &self.out, &self.sora).2;
            #[cfg(not(feature = "party"))]
            let in_battle = self.sora.actions().battle;
            let in_battle = in_battle || self.sora.actions().battle_forced;
            let (leader_kh, leader_yaw, leads) = (self.lead_pos(), self.lead_yaw(), self.friend_lead().is_none());
            let (feet_kh, yaw, resting) = (self.out.position, self.out.yaw, self.savepoint.resting());
            crate::spirit::frame(crate::spirit::SpiritFrame { control, sora: &mut self.sora, player, frame: self.world.frame, feet_kh, yaw, leader_kh, leader_yaw, leads, in_battle, resting, paused, dt });
        }
        // (MIMIC-BOSS) THE MIMIC TEAR BOSSES (mimic_boss.rs): the fight's state, the solo fight at the fog gate (the friends out,
        // a spirit from before the gate dismissed; back on a win or outside the arena), the copy of the leader made when the
        // game enables the boss's body
        #[cfg(feature = "party")]
        {
            let (leader_kh, leader_alive, now) = (self.lead_pos(), self.lead_alive() && !self.died, self.clock as f64);
            let rt = self.party.core_mut().map(|c| &mut c.rt);
            let reseat = crate::mimic_boss::frame(crate::mimic_boss::BossFrame { control, sora: &mut self.sora, rt, targets: &self.targets, frame: self.world.frame, leader_kh, leader_alive, now });
            crate::arena_walls::frame(&self.world, leader_kh); // (NOKRON-WALLS) the fight's invisible walls into Sora's world
            if reseat {
                if let Some(core) = self.party.core_mut() {
                    core.reseat_soon();
                }
            }
        }

        // ---- the camera: KH3's while Sora is in control (alive, gameplay, the world runs)
        // (IMPL_ER13) a menu of OURS (or the game-over hold) freezes the view; Elden Ring's own menus / map / grace /
        // cutscenes get its camera back as before
        let own_menu = (crate::hud::menu_open() || crate::hud::gameover_holds()) && !ui.cutscene;
        self.cam.frame(control, (!ui.menu || self.follow == Some(Follow::Action)) && !paused, own_menu, crate::cam::CamSora { out: &self.out, before: &self.out_before, sora: &self.sora }, self.world.frame);

        // Input: the pad through XInput, the keyboard path beside it; `in.*` of the control file
        // replaces the stick (unattended tests).
        let device = self.reader.read();
        self.pad_seen = device.is_some();
        if device.is_some() {
            inputs::real_pad_seen(); // (IMPL_ER8) a real pad is connected: the hook invents no slot
        }
        self.script.poll(control);
        // (IMPL_ER8, testing aid) dbg.bossbar = <NpcName id>: the game's boss bar shown for the player's own character
        crate::erhud::test_boss_bar(control.i64("dbg.bossbar").filter(|v| *v > 0).map(|v| (v as i32, player.chr_ins.field_ins_handle)));
        inputs::set_vpad(control.on("in.vpad")); // (IMPL_ER8, testing aid) a virtual pad in slot 0 while no real one is connected
        if device.is_some_and(|p| p.left.length() > 0.25 || p.right.length() > 0.25 || p.buttons != PadButtons::NONE) {
            self.pad_active += 1;
            self.lab_close("real pad input");
            self.script.cancel("real pad input");
        }
        let mut right_stick = device.map(|p| p.right).unwrap_or(Vec2::ZERO);
        // (CAMP-OPTIONS follow-ups) THE MOUSE AS KH3 PC READS IT: as the right stick (eroptions.rs `mouse_stick`), for every
        // camera and aim mode the right stick drives (cam.rs: field, lock-on, shotlock aim, the forms' cameras; ShooterMode's
        // aim below); taken every frame (none kept for later), given only where the right stick reaches them (below)
        let mut mouse_look = crate::eroptions::mouse_stick(crate::dinput_mask::take_mouse_motion(), dt);
        if let (Some(x), Some(y)) = (control.f32("in.rx"), control.f32("in.ry")) {
            right_stick = Vec2::new(x, y);
        }
        let mut stick = device.map(|p| p.left).unwrap_or(Vec2::ZERO);
        let mut held = device.map(|p| p.buttons).unwrap_or(PadButtons::NONE);
        // (IMPL_ER38) View on the real pad (mapwatch.rs: each press in the field is a map request), and THE GAME'S WINDOW NOT
        // IN FRONT: Elden Ring takes no pad input then (the user's lost map, mapwatch.rs), so neither does Sora (`in.focus = 0`: he does)
        let view_raw = device.is_some_and(|p| p.buttons.has(PadButton::SpecialLeft));
        let pad_y_raw = device.is_some_and(|p| p.buttons.has(PadButton::FaceTop)); // (KBD-Y) the physical pad's own Y
        if !self.map_watch.pad_gate(control, dt) {
            (stick, right_stick, held) = (Vec2::ZERO, Vec2::ZERO, PadButtons::NONE);
            mouse_look = Vec2::ZERO; // (CAMP-OPTIONS)
        }
        if control.flag("in.keys") != Some(false) {
            let (key_stick, key_buttons) = inputs::keyboard();
            if key_stick != Vec2::ZERO {
                stick = key_stick;
            }
            for button in key_buttons.iter() {
                held.set(button, true);
            }
            // (NO-ER-PAUSE) the keyboard's Esc is Start: the KH menu (the camp menu, the Pause screen in battle) opens and
            // closes on it as on the pad's Start
            if inputs::esc_menu() {
                held.set(PadButton::SpecialRight, true);
            }
            if key_stick != Vec2::ZERO || key_buttons != PadButtons::NONE {
                self.key_frames += 1;
            }
        }
        if let (Some(x), Some(y)) = (control.f32("in.x"), control.f32("in.y")) {
            stick = Vec2::new(x, y);
        }
        // (testing aid) THE INPUT SCRIPT STANDS FOR THE PAD ITSELF (IMPL_ER8: moved up from below the ownership rules, so
        // that LB + Start, Y under a prompt, View and an Elden Ring menu's buttons take the path a held pad takes): it
        // replaces sticks and buttons while it runs, one segment step per 1/60 s of REAL time (the game's own delta
        // stands at 0 while `menu.pause` holds its speed at 0), and the XInput hook shows the same pad to Elden Ring
        // (before its mask). `in.script_menus = 1`: it goes on while an Elden Ring menu has the input.
        let real = self.script_at.elapsed().as_secs_f32();
        self.script_at = std::time::Instant::now();
        self.script_clock = (self.script_clock + real).min(0.25);
        while self.script_clock >= STEP {
            self.script_clock -= STEP;
            self.script_now = self.script.next();
        }
        if ui.menu && !control.on("in.script_menus") {
            self.script.cancel("an Elden Ring menu has the input");
        }
        if let (Some(seg), true) = (self.script_now, self.script.active()) {
            (stick, right_stick, held) = (seg.left, seg.right, seg.buttons);
            inputs::set_virtual(seg.left, seg.right, seg.buttons);
        }
        // (GF-INGAME) the test pilot (`test.auto`, a test key)
        if let Some(seg) = crate::autopad::step(control, self.lead_pos(), self.cam.input_yaw().unwrap_or(self.out.yaw), &self.targets, dt) {
            (stick, right_stick, held) = (seg.left, seg.right, seg.buttons);
            inputs::set_virtual(seg.left, seg.right, seg.buttons);
        }
        let raw_held = held; // (IMPL_ER23A) what is physically down, before any ownership rule
        // INPUT OWNERSHIP: View is Elden Ring's (its map); while one of its prompts is on screen so is Y; while a menu of its
        // has the input Sora gets nothing.
        // BUTTON TABLE (user decision 2026-10-12): View (select) = Elden Ring's map, always. Menu (Start) = the Kingdom Hearts
        // menu, ALWAYS: (NO-ER-PAUSE, THE USER 2026-11-09: "remove all access to the elden ring pause menu, esc should make KH
        // menu appear on keyboard. Elden Ring pause menu should be impossible to ever show up") Start is hidden from Elden Ring
        // and goes to the core, with or without LB (LB + Start opened Elden Ring's menu before), whatever `in.kh_menu` says; the
        // keyboard's Esc is Start too (`inputs::esc_menu`, where the keyboard is read), hidden from the game's keyboard as Start is from its pad
        // (dinput_mask.rs). The game's top menu itself is refused where the game opens it (nopause.rs).
        held.set(PadButton::SpecialLeft, false);
        inputs::run_key_script();
        let r3 = held.iter().any(|b| b == PadButton::RightThumb);
        // (savepoint) the save points of the sites of grace in reach; Y at a discovered one is the KH3 save menu's
        let y_raw = held.iter().any(|b| b == PadButton::FaceTop);
        let menu_before = crate::hud::menu_open(); // (PFY) a menu of ours had the pad before the save point looks at Y
        // (CV3 round 4) the save point reads THE LEADER (on the floor, fighting, alive), never a frozen out-of-party Sora
        #[cfg(feature = "party")]
        let sp_floor = {
            let lf = crate::conv::leader_facts_of(self.party.core(), &self.out, &self.sora);
            self.savepoint.set_leader(lf.battle, lf.alive);
            lf.on_floor
        };
        #[cfg(not(feature = "party"))]
        let sp_floor = self.out.on_floor;
        self.savepoint.load_in(control, loadin.window, self.feet_er()); // (LDI) what appears in the load-in window appears settled
        let sp_y = self.savepoint.frame(control, player, &self.targets, self.world.frame, &mut self.sora, crate::savepoint::SpFrame { on_floor: sp_floor, prompt: self.prompt, ui, y_held: y_raw, lb_held: held.iter().any(|b| b == PadButton::LeftShoulder) /* (IMPL_ER26) LB + Y */, dt: real });
        // (savepoint, IMPL_ER26) Respawn Enemies asked the game for its world reset: no orb stays on the field (their munny is given, as when a cutscene starts)
        if self.savepoint.take_world_reset() && !self.cutscene {
            self.prize.cutscene(true, &mut self.sora);
            self.prize.cutscene(false, &mut self.sora);
        }
        self.prize.frame(control, player, &mut self.sora, dt, self.savepoint.resting()); // (prize) the wallet: a kill's worth, `munny.mirror`
        self.loot.frame(control, &mut self.sora, &self.targets.hostiles_by_distance().iter().map(|(_, e)| e.npc_param).collect::<Vec<_>>()); // (loot) enemy types examined / taken over, the gate's dues
        self.attackers.frame(control, &self.targets.hostiles_by_distance().iter().map(|(d, e)| (e.npc_id, e.think_param, *d, e.in_attack)).collect::<Vec<_>>(), dt); // (ENG) up to 5 attackers at once
        // (IMPL_ER14, the user: "I was having trouble interacting with Y") Y IS ROUTED PER PRESS. At the down-edge:
        // the save point's command when it is offered; Elden Ring's when its action prompt is up (or a menu of its has
        // the pad); else Sora's. The owner keeps the WHOLE press, whatever the prompt does meanwhile (a prompt that
        // goes away under a held Y no longer gives Sora the tail of the press as a press of his own; a prompt that
        // comes up under a held Y of Sora's is not answered by its tail). ONE EXCEPTION, the window `in.y_window`
        // (ms, default 100): a prompt that comes up that soon after the down-edge of a press Sora got (he walked into
        // reach as the button went down) gets a press of its own, made for Elden Ring through the pad hook (120 ms),
        // also when the button is already up again. `in.y_press = 0`: the rule before (by the prompt, frame by frame).
        let per_press = control.flag("in.y_press") != Some(false);
        let window = std::time::Duration::from_millis(control.i64("in.y_window").map_or(100, |v| v.clamp(0, 500)) as u64);
        // (CV3) TALK PRIORITY: a party member's talk prompt (Donald at the crone) or a party conversation running wins Y
        // over Elden Ring's own action prompt (the crone's talk); Y then goes the party talk's way (unlock::note_pad)
        let er_prompt = crate::conv::er_prompt_may_take_y(self.prompt);
        // (CAMP-OPTIONS) the game's own action input is the game's while its prompt is open to it (no KH prompt, no menu of
        // ours): its keyboard action key (dinput_mask.rs) and, with a remapped pad, its own Y button (the mask below)
        let er_action_open = er_prompt && !sp_y && !menu_before;
        crate::dinput_mask::set_er_action_open(er_action_open);
        if self.prompt && !er_prompt && y_raw && !self.y.held {
            erlog!("y: Elden Ring's action prompt is up, but a party member's talk is offered (or his conversation runs): Y is the party talk's (CV3)");
        }
        let y_to_er = if per_press {
            let y = &mut self.y;
            if y_raw && !y.held {
                y.count += 1;
                y.down_at = std::time::Instant::now();
                y.owner = YOwner::at_down(menu_before, sp_y, er_prompt, ui.menu); // (PFY) (CV3: er_prompt)
                // (KBD-Y) the keyboard's Y (F) at Elden Ring's prompt: made for the game as its action (its pad Y through the pad
                // hook when it reads a pad, else its own E key), the press the prompt shows
                let keyboard_y = control.flag("in.keys") != Some(false) && inputs::keyboard().1.has(PadButton::FaceTop);
                if crate::dinput_mask::keyboard_y_for_er(y.owner == YOwner::EldenRing, keyboard_y, pad_y_raw) {
                    let how = inputs::press_for_er(inputs::XI_Y, crate::dinput_mask::DIK_ER_ACTION as u16);
                    erlog!("y: press #{}: the KEYBOARD's Y for Elden Ring's prompt: made for the game as its action ({}) (KBD-Y)", y.count, how.unwrap_or("NOT made: no route open"));
                }
                (y.replayed, y.watching, y.prompt_at_down, y.er_frames, y.frames) = (false, y.owner == YOwner::Sora, self.prompt, 0, 0);
                if control.flag("log.y") != Some(false) {
                    let visible: Vec<String> = self.ui_states.as_ref().map(|s| s.iter().enumerate().filter(|(_, v)| **v & 2 != 0).map(|(k, _)| k.to_string()).collect()).unwrap_or_default();
                    erlog!(
                        "y: press #{} DOWN -> {} | Elden Ring's prompt {} (interface elements visible: [{}]; hud_state {}{}), save point command {}, Sora {:?} on_floor {} | Elden Ring reads the pad through our hook: {}",
                        y.count,
                        match y.owner { YOwner::SavePoint => "THE SAVE POINT (KH3's save menu)", YOwner::EldenRing => "ELDEN RING (its action button)", YOwner::Sora => "SORA (situation command)", YOwner::Menu => "OUR MENU (the HUD's open menu, e.g. Party Formation's Make Leader)" },
                        if self.prompt { "UP" } else { "not up" }, visible.join(" "), ui.hud_state, if ui.menu { ", a menu of its has the pad" } else { "" },
                        if sp_y { "offered" } else { "not offered" }, self.out.state, self.out.on_floor, inputs::er_reads_pad(),
                    );
                }
            }
            // (the window: a prompt that comes up just after the down-edge of a press Sora got)
            if y.watching && y.down_at.elapsed() > window {
                y.watching = false;
            }
            if y.watching && er_prompt && !sp_y && y.owner == YOwner::Sora {
                y.watching = false;
                y.replayed = true;
                y.owner = YOwner::EldenRing;
                let how = inputs::press_for_er(inputs::XI_Y, crate::dinput_mask::DIK_ER_ACTION as u16); // (KBD-Y) also keyboard-only
                erlog!("y: press #{}: Elden Ring's prompt came up {} ms after the down-edge (inside the {} ms window): THE PRESS IS HANDED ON to Elden Ring as a press of its own ({}); Sora had the down-edge", y.count, y.down_at.elapsed().as_millis(), window.as_millis(), how.unwrap_or("NO WAY: the game reads no pad"));
            }
            if y_raw {
                y.frames += 1;
                y.er_frames += u32::from(y.owner == YOwner::EldenRing);
            }
            if !y_raw && y.held && control.flag("log.y") != Some(false) {
                erlog!("y: press #{} UP after {} ms ({} frame(s)): it was {}'s for the whole press{}; Elden Ring's prompt was {} at the down-edge and is {} now", y.count, y.down_at.elapsed().as_millis(), y.frames, match y.owner { YOwner::SavePoint => "the save point", YOwner::EldenRing => "Elden Ring", YOwner::Sora => "Sora", YOwner::Menu => "our menu" }, if y.replayed { " from the hand-over on" } else { "" }, if y.prompt_at_down { "up" } else { "not up" }, if self.prompt { "up" } else { "not up" });
            }
            y.held = y_raw;
            if y_raw && y.owner.hides_y() {
                held.set(PadButton::FaceTop, false);
            }
            y_raw && y.owner == YOwner::EldenRing
        } else {
            if er_prompt && !menu_before {
                held.set(PadButton::FaceTop, false); // (PFY) not while a menu of ours has the pad
            }
            er_prompt && !sp_y && !menu_before // (CV3: er_prompt)
        };
        // (IMPL_ER10) in a cutscene with `cutscene.menu` the pad still reaches OUR menus (Start = KH3's cutscene pause menu)
        let cutscene_menu = ui.cutscene && control.on("cutscene.menu") && control.on("hud.kh");
        // (CV3) A DRIVEN STORY REST WHOSE TALK IS A KH BUBBLE (Melina's talks: CV2 shows her lines and choices as bubbles):
        // the bubble gets the pad (A, the D-pad, B); Sora still gets none (the body rests). Before, the forced "menu" of
        // the story rest kept every key from the bubble: SV's run2 at 262 s, her Accept / Refuse never answerable.
        crate::conv::set_story_rest(self.savepoint.holds_pad());
        let story_bubble = crate::conv::story_bubble_keys();
        if ui.menu && !cutscene_menu && !story_bubble {
            let (stick_was, held_was) = (stick, held); // (ERUI)
            stick = Vec2::ZERO;
            right_stick = Vec2::ZERO;
            held = PadButtons::NONE;
            if borrowed_pad.on {
                // (ERUI) a borrowed interaction: our HUD gets Start (KH3's Pause), and the whole pad while that Pause is up
                held = PadButtons(held_was.0 & borrowed_pad.hud_buttons);
                if borrowed_pad.hud_nav {
                    stick = stick_was;
                }
            }
        }
        let before_hud = held;
        self.hud.input(control, &mut stick, &mut right_stick, &mut held); // (hud) the menus' pad input; Sora gets none while one is up
        if story_bubble {
            stick = Vec2::ZERO;
            right_stick = Vec2::ZERO;
            held = PadButtons::NONE;
        }
        let ours = before_hud != held || crate::hud::menu_open() || crate::hud::gameover_holds(); // (IMPL_ER23A) a menu of ours has the pad
        // (CAMP-OPTIONS) no mouse look under a menu (ours or the game's), a story bubble, the boss finish, a held body, a script
        if ui.menu || story_bubble || ours || crate::conv::holds_sticks() || self.finish.locked() || self.follow.is_some() || self.own_hold || self.leap_wait.is_some() || self.script.active() {
            mouse_look = Vec2::ZERO;
        }
        if self.finish.locked() {
            // (IMPL_ER22) KH3's boss finish holds the input (bDisableMove, bDisableCameraMove)
            stick = Vec2::ZERO;
            right_stick = Vec2::ZERO;
            held = PadButtons::NONE;
        }
        // (LADDER2) also while the game lets its body go at a ladder: the leader stands for his leap (no step away from it)
        if self.follow.is_some() || self.own_hold || self.leap_wait.is_some() || (self.catch.is_some_and(|c| c.holds()) && self.friend_lead().is_none()) /* (GRABS) Sora leading, caught or getting up */ {
            // (IMPL_ER10) he follows the body: no input (IMPL_ER17: nor while the body performs an action in place)
            stick = Vec2::ZERO;
            right_stick = Vec2::ZERO;
            held = PadButtons::NONE;
        }
        // (IMPL_ER23A, the user: "he jumps when you press B to exit save point screen") PRESS OWNERSHIP FOR EVERY BUTTON: a
        // button that is down while somebody else has the pad (a menu of ours, a menu / selection window of Elden Ring's,
        // the game's own action of the body, the boss finish, a standing world) is not Sora's until it has been released:
        // the press that closes a menu, picks a line or ends a hand-over never reaches the simulation, on no frame
        // (guards.rs `PressLatch`). `in.latch = 0`: as before.
        if control.flag("in.latch") != Some(false) {
            let foreign = ui.menu || ours || paused || self.finish.locked() || self.follow.is_some() || self.own_hold;
            held = self.latch.filter(raw_held, held, foreign);
            let kept = self.latch.blocked();
            if !foreign && kept != self.latch_told && !kept.is_empty() && control.flag("log.latch") != Some(false) {
                erlog!("pad: {:?} held while somebody else had the pad: NOT SORA'S UNTIL RELEASED (no jump / attack from the press that closed a menu)", kept.iter().collect::<Vec<_>>());
            }
            if !foreign {
                self.latch_told = kept;
            }
        }
        // What Elden Ring does not see of the pad (inputs.rs; renewed every frame, lapses by itself).
        let own = control.flag("in.own") != Some(false) && control.flag("hud.hide") != Some(true);
        let mask = if own && cutscene_menu { crate::hud::cutscene_mask() } else if !own || ui.menu { 0 } else { u32::from((inputs::XI_GAMEPLAY | inputs::XI_START) & !(if y_to_er || (er_action_open && crate::remap_er::pad_moves(PadButton::FaceTop)) { inputs::XI_Y } else { 0 })) | inputs::MASK_LEFT_STICK | inputs::MASK_TRIGGERS | if self.cam.has_view() { inputs::MASK_RIGHT_STICK } else { 0 } };
        let mask = if borrowed_pad.on && own { mask | borrowed_pad.er_mask } else { mask }; // (ERUI) no Start / View of the game's in a borrowed interaction (nothing under our Pause)
        inputs::set_mask(if mask != 0 { mask | crate::hud::extra_mask() } else { mask }); // (hud) + Start, View, right stick while a menu of ours is up
        if self.savepoint.masks_pad() {
            inputs::set_mask(0xFFFF | inputs::MASK_LEFT_STICK | inputs::MASK_TRIGGERS | inputs::MASK_RIGHT_STICK); // (IMPL_ER36) under the cover only our presses reach the game's menu
        }
        if crate::talkkh::masks_game_pad() {
            inputs::set_mask(0xFFFF | inputs::MASK_LEFT_STICK | inputs::MASK_TRIGGERS | inputs::MASK_RIGHT_STICK); // (CV2) a KH conversation of Elden Ring's talk: the pad is the bubble's
        }
        // (IMPL_ER38) the map: each request and its outcome (`map:` lines), the View guard (after every mask of this frame)
        {
            let ta = &player.chr_ins.modules.time_act;
            let facts = crate::mapwatch::Facts {
                menu_ours: crate::hud::menu_open() || crate::hud::gameover_holds() || crate::hud::extra_mask() != 0,
                cover: self.savepoint.masks_pad(),
                hud_state: ui.hud_state,
                body_anim: ta.anim_queue[(ta.read_idx as usize) % 10].anim_id,
                ladder: ladder_state(player),
                world_still: crate::pause::world_runs() == Some(false),
                cutscene: ui.cutscene || self.cutscene, // (MAP-GATE)
                leader_alive: self.lead_alive(),
                ..Default::default()
            };
            self.map_watch.frame(control, view_raw, if mask != 0 { mask | crate::hud::extra_mask() } else { mask }, facts, player, dt);
        }
        if control.on("dbg.padprobe") {
            inputs::probe_log(); // (IMPL_ER8, testing aid) what Elden Ring would read of the pad right now
        }
        let jump_mark = control.i64("in.jump").unwrap_or(0);
        if jump_mark != self.scripted_jump.0 {
            self.scripted_jump = (jump_mark, control.i64("in.jump_hold").unwrap_or(20).max(1) as u32);
        }
        let er_camera_yaw = game::camera().map(|c| coords::yaw_kh_of_er_dir(c.forward)).unwrap_or(self.out.yaw);

        // (the world stands still: so does he)
        // (IMPL_ER22) KH3's boss finish, once a frame: the deaths the game confirmed, what ends a sequence
        self.finish.frame(
            control,
            &mut self.targets,
            crate::finish::Scene { sora_dead: self.death_sent || self.sora.status().hp <= 0 || self.flow.down().is_some() /* (DTH) the leader down, whoever leads */, cutscene: ui.cutscene || self.cutscene || self.follow.is_some(), menu: ui.menu || paused },
            1.0 / dt.max(1.0e-3),
        );
        // (while a sequence runs the steps are counted on the WALL CLOCK: the task's delta may be scaled by the game speed we wrote)
        let dt_steps = self.finish.accumulator_dt(dt);
        // (INT) paused: the accumulator is HELD (no steps, the frame stays where it was between the steps; 0 drew the step
        // before: everything popped back a step when a menu opened, with `r.interp` on)
        self.accumulator = if paused { self.accumulator.min(STEP * 0.999) } else { (self.accumulator + dt_steps).min(0.25) };
        // (SPIRITSPRING) the spring the body stands in (never while it rides Torrent: the game's own spring jump), once a frame
        let spring_on = control.flag("spring.on") != Some(false) && !ui.menu && !ui.cutscene && !self.cutscene && !paused;
        let spring_riding = crate::spiritspring::game::riding(player);
        let spring_here = if spring_on && !spring_riding { crate::spiritspring::game::here(player) } else { None };
        let casts_before = self.world.casts.get();
        while self.accumulator >= STEP {
            self.accumulator -= STEP;
            let mut held_now = held;
            // The controller steers by the camera as the PREVIOUS step left it (camera spec 1.4).
            let camera_yaw = self.cam.input_yaw().unwrap_or(er_camera_yaw);
            // (IMPL_ER18, testing aid) `in.dir = yaw`: the stick pushed fully toward that KH3 world yaw, whatever the camera does
            let (stick, camera_yaw) = match control.f32("in.dir") {
                Some(yaw) if stick.length() < 0.3 && !self.ui.menu => { let a = (yaw - camera_yaw).to_radians(); (Vec2::new(a.sin(), a.cos()), camera_yaw) }
                _ => (stick, camera_yaw),
            };
            if self.scripted_jump.1 > 0 {
                self.scripted_jump.1 -= 1;
                held_now.set(PadButton::JUMP, true);
            }
            let pad = PadState::from_held(held_now, self.held_before);
            let attack_pressed = held_now.has(PadButton::FaceBottom) && !self.held_before.has(PadButton::FaceBottom);
            self.held_before = held_now;
            let input = Input { move_stick: stick, camera_yaw, view: self.cam.core_view(), ..Input::default() }.with_pad(pad);
            // (IMPL_ER14) ONE LINE PER PRESS OF THE ATTACK BUTTON: what he is doing when it comes down, and the floor
            // under him as the attack's own choice will see it (the states' ground probe, repeated here).
            if attack_pressed && control.flag("log.attack") != Some(false) {
                self.atk.presses += 1;
                let probe = self.ground_probe();
                erlog!(
                    "press: #{} A down before step {} | state {:?}/{} move {:?} on_floor {} z {:.0} vel z {:.0} | ground probe: {}",
                    self.atk.presses, self.out.step + 1, self.out.state, self.out.sub_step, self.out.movement_state, self.out.on_floor, self.out.position.z, self.out.velocity.z, probe,
                );
            }
            // (IMPL_ER22) KH3's boss finish: the script's step on the real step; `wdt` = the world's step (1/60 outside a sequence)
            let rate = self.finish.step(STEP);
            let world = self.worldtime.step(STEP, rate, &self.sora); // (AUDIT-A WORLDTIME) + the forms' hit-stops (global) and WorldSlow (enemies)
            let rate = world.global; // (AUDIT-A WORLDTIME)
            let wdt = STEP * rate;
            // (PARTY-AI, AUDIT-A WORLDTIME) KH3's [FRIEND] (the shotlock aim's x0.4, SetTypeRates 0x4D9E): every AI party member's
            // step on wdt x [FRIEND], the player's character on wdt (kh-party Party::member_rate)
            #[cfg(feature = "party")]
            if let Some(c) = self.party.core_mut() {
                c.rt.party.set_friend_rate(world.friend_type);
            }
            self.targets.puppets.time_slow = 1.0 - world.enemy(); // (AUDIT-A WORLDTIME) the enemies' reactions at global x [ENEMY]
            if self.finish.locked() {
                held_now = PadButtons::NONE;
            }
            self.targets.step = self.out.step + 1;
            self.targets.sora = self.lead_pos(); // (GF-INGAME)
            self.targets.sora_on_floor = self.out.on_floor; // (IMPL_ER14)
            // The enemies in a KH3 reaction move one fixed step BEFORE Sora's step, so that his
            // targeting, combo continuation and air-combo height matching see them where they are.
            self.targets.step_puppets();
            // (ER party S1) contract ER.0 step 1: the leader routing (the formation, the pad's frame to a leading friend);
            // Sora's input is the pad's while he leads, AI Sora's while a friend does
            // (ER party S7) a friend leading: the player's refused Y on a team command is held for Sora's executor (0.5 s)
            #[cfg(feature = "party")]
            let input = crate::party::team::hold_y(&self.party, &self.sora, input, wdt, control);
            #[cfg(feature = "party")]
            let input = self.party.route(&mut self.sora, &input, wdt, &self.world, &mut self.targets, self.clock as f64);
            // (ER party S1) step 2: Sora's step with the party's targets (his allies, heals on friends, the team seam); not
            // stepped while he is out of the active party
            // (LADDER2) SORA'S LEAP UP A LADDER (the leader, or AI Sora after a friend leader; ladder.rs `SoraLeap`): his kh-core
            // state is HELD (not stepped), his place is the leap's (kh-core leap.rs), his figure KH3's own jump clips
            // (SPIRITSPRING) the leader on foot at a spiritspring: his jump's lift for this step (spiritspring.rs)
            let on_foot = spring_on && !spring_riding && self.follow.is_none() && !self.own_hold && self.leap.is_none() && self.leap_wait.is_none() && self.lead_alive();
            self.spring_step(spring_here, on_foot, control, wdt);
            let leaping = self.leap.is_some();
            // (F-ARROWGUN line 3, F-HUD, INT-F) the right stick for the gun forms' ShooterMode aim (Input has none): the
            // pad's while Sora leads (cam.rs keeps the orbit still while ShooterMode's camera is up)
            #[cfg(feature = "party")]
            let look = if self.friend_lead().is_some() { Vec2::ZERO } else { right_stick + mouse_look /* (CAMP-OPTIONS) the mouse aims too */ };
            #[cfg(not(feature = "party"))]
            let look = right_stick + mouse_look; // (CAMP-OPTIONS) the mouse aims too
            self.sora.set_shooter_look(look);
            let leap_out = match self.leap.as_mut().map(|l| l.step(wdt)) {
                Some((s, show)) => {
                    self.leap_show = show;
                    let air = matches!(s.phase, kh_core::leap::Phase::Rise | kh_core::leap::Phase::Fall);
                    Some(Output { events: Vec::new(), position: s.position, yaw: s.yaw, velocity: s.velocity, capsule_center: s.position + (self.out.capsule_center - self.out.position), on_floor: !air, ..self.out.clone() })
                }
                None => None,
            };
            #[cfg(feature = "party")]
            let out = match leap_out {
                Some(o) => o,
                None => self.party.step_sora(&mut self.sora, wdt, &input, crate::party::soratargets::SoraSeams { world: &self.world, targets: &mut self.targets, fx: &mut self.fx, ui: &mut self.hud.keep }, &self.out),
            };
            #[cfg(not(feature = "party"))]
            let out = match leap_out {
                Some(o) => o,
                None => {
                    let mut host = Host { world: &self.world, targets: &mut self.targets, audio: &mut self.fx.audio, effects: &mut self.fx.effects, ui: &mut self.hud.keep /* (hud) */ };
                    self.sora.step_hosted(wdt, &input, &mut host)
                }
            };
            self.leap_after_step(&out);
            // (CHESTFIX, the user 2026-10-27: "he stopped moving and even swapping to him from the party menu I couldn't move
            // as him") kh-core closed a shotlock aim mode that a dropped state had left open (shotlock.rs `close_dropped` at a
            // teleport, the net in its update): said, as every net
            let closed = self.sora.aim().stale_cleared;
            if closed != self.aim_closed_seen {
                self.aim_closed_seen = closed;
                erlog!("!!! sora: A SHOTLOCK AIM MODE LEFT OPEN BY A DROPPED STATE (a warp or a teleport during ShootFlow) WAS CLOSED by kh-core (#{closed}): before, it refused every movement, jump, attack and command of his for ever ({})", if self.friend_lead().is_some() { "AI Sora" } else { "Sora leading" });
            }
            // (CHESTFIX, the user 2026-10-27: "Sora was also stuck in a pseudo-second form after transforming into it as an
            // ally") kh-core's form net (style.rs `tick`): a wear form / form abilities that no form held were given back
            let nets = self.sora.combat().form.nets;
            if nets != self.form_nets_seen {
                self.form_nets_seen = nets;
                erlog!("!!! sora: A FORM'S LOOK AND ABILITIES WITH NO FORM HELD (a form whose end was skipped) WERE GIVEN BACK by kh-core (#{nets}; {}): wear form {} now", if self.friend_lead().is_some() { "AI Sora" } else { "Sora leading" }, self.sora.combat().form.wear_form);
            }
            // (CHESTFIX) THE NET ON HIS OWN SIMULATION (exitnet.rs `StillNet`): Idle with the run stick for 2 s, on the floor,
            // not moving, nothing of ours or the game's holding him -> a fresh Sora where he stands (his status, magic, menu
            // kept), whoever leads. `sora.still_net = 0`: off
            {
                let aim_mode = self.sora.aim().mode;
                let free = self.follow.is_none() && !self.own_hold && self.leap.is_none() && self.leap_wait.is_none() && !self.finish.locked() && self.sora.status().hp > 0 && !matches!(aim_mode, 1 | 4) && control.flag("sora.still_net") != Some(false);
                let moved = Vec2::new(out.position.x - self.out.position.x, out.position.y - self.out.position.y).length();
                if self.still_net.step(out.state == kh_core::StateId::Idle, input.move_stick.length(), moved, out.on_floor, free, STEP) {
                    erlog!("!!! sora: HIS SIMULATION REFUSED THE STICK for {:.1} s ({}: Idle/{} at kh ({:.0}, {:.0}, {:.0}), the stick {:.2}, aim mode {aim_mode}, nothing holding him): A FRESH SORA WHERE HE STANDS (the net, #{})", crate::exitnet::StillNet::NET_S, if self.friend_lead().is_some() { "AI Sora behind the leader" } else { "Sora leading" }, out.sub_step, out.position.x, out.position.y, out.position.z, input.move_stick.length(), self.still_net.fired);
                    self.sora.teleport(Pose { position: out.position, yaw: out.yaw });
                }
            }
            // (ER party S1) steps 3 - 5: the party's step (AI, Party::step, Team::step), the poses, the leader's lock, the
            // trail and the fallbacks, the bodies' want list; then S2's leader body (its view of the leader, its orders)
            #[cfg(feature = "party")]
            {
                let lock_on = self.hud.keep.snapshot.lock_on;
                // (ER party S2) the follow rule's level floor (`world.flat`: Sora seated on the body) is HIS: the party's casts
                // see the game's world (else the friends below him fall through it)
                let flat = self.world.flat.take();
                // (ER party S1) Sora's look-at point for the friends' head look-at: his "t_camera" bone (else his head) of the
                // last drawn pose, as the test level takes it
                let look = self.animator.as_ref().and_then(|a| {
                    let s = a.skeleton();
                    let i = s.index_of("t_camera").or_else(|| s.index_of("atama"))?;
                    self.pose.component.get(i).map(|t| component_to_world(&self.out).transform_point(t.translation))
                });
                self.party.after_sora_step(&mut self.sora, crate::party::AfterSora { out: &out, dt: wdt, world: &self.world, targets: &mut self.targets, fx: &mut self.fx, lock_on, look, leaping: self.leap.is_some(), now: self.clock as f64 });
                // (MIMIC SORA, THE USER 2026-11-03: "build mimic Sora") the Mimic Tear's Sora: his AI (AI Sora's), his step, his pose
                // (spirit_mimic.rs), right after the party's step, against the same world and enemies
                crate::spirit_mimic::step(crate::spirit_mimic::MimicStep { sora: &self.sora, world: &self.world, targets: &mut self.targets, fx: &mut self.fx, rt: self.party.core().map(|c| &c.rt), camera: self.party.core().map(|c| &c.camera), lock_on, dt: wdt * world.friend_type /* (PARTY-AI) Mimic Sora is an AI: KH3's [FRIEND] */, now: self.clock as f64 });
                // (MIMIC-BOSS) a Mimic Tear boss's copy (and its summon): its hostile AI, its step against the party, its hits on
                // the leader through the party's damage path; an enemy of the world: KH3's [ENEMY] time rate
                let camera = self.party.core().map(|c| c.camera);
                crate::mimic_boss::step(crate::mimic_boss::BossStep { sora: &mut self.sora, rt: self.party.core_mut().map(|c| &mut c.rt), camera, world: &self.world, fx: &mut self.fx, dt: wdt * world.enemy_type, now: self.clock as f64 });
                self.world.flat.set(flat);
                match self.party.runtime_on() {
                    Some(rt) => {
                        let dodge = crate::party::leader::sora_dodge_frames(self.sora.actions().reaction);
                        self.lead.update(rt, &out, self.sora.status(), dodge);
                        let seat = self.lead.apply(rt, &mut self.sora);
                        if let Some(core) = self.party.core_mut() {
                            core.counter_move(seat);
                        }
                    }
                    None => self.lead.clear(),
                }
                // (LADDER2) AI SORA JUMPS THE LADDER right after the friend leader (party/glue.rs `ladder_jumps`: his place
                // beside the landing, his turn); every party member's take-off: Torrent's wind (ersound.rs, at his feet)
                if let Some(j) = self.party.core().and_then(|c| c.feed.sora_leap) {
                    if self.friend_lead().is_some() && self.leap.is_none() && self.sora.status().hp > 0 {
                        let h = Sora::origin_height_on_floor(&SORA_PARAMS);
                        self.leap = Some(crate::ladder::SoraLeap::new(kh_core::leap::Leap::new(out.position, out.yaw, j.to + Vec3::Z * h, j.facing, j.delay)));
                    }
                }
                let sounds = self.party.core().map(|c| c.feed.ladder_sounds.clone()).unwrap_or_default();
                for at in sounds {
                    crate::ersound::ladder_jump(self.world.frame.pos_kh_to_er(at));
                }
            }
            // (IMPL_ER23A) wedged off the floor (guards.rs `Stuck`): resolved after the steps of this frame (LADDER2: not in a
            // leap, which is in the air on purpose)
            if self.stuck.step(out.on_floor || leaping, out.position, out.velocity) && control.flag("stuck") != Some(false) {
                self.stuck_now = true;
            }
            // (IMPL_ER24) the reports (falllog.rs), written after the steps of this frame: he left the floor without a jump
            // (walking, standing, rolling, guarding, hit); he pushes on the floor and does not move
            if self.out.on_floor && !out.on_floor && out.velocity.z <= 1.0 && self.follow.is_none() && matches!(format!("{:?}", out.state).as_str(), "Idle" | "Run" | "Guard" | "Dodgeroll" | "Damage" | "Jump") {
                self.floor_report = Some((self.out.clone(), out.clone()));
            }
            if self.blocked.step(out.on_floor || leaping, out.position, out.velocity) {
                self.blocked_now = true;
            }
            // (IMPL_ER24) the leg IK's floor: a foot reaches at most `ik.drop` cm (0 = no limit) below what he stands on
            let drop = control.f32("ik.drop").unwrap_or(25.0);
            self.world.ik_floor.set((out.on_floor && drop > 0.0).then(|| (out.position.z - Sora::origin_height_on_floor(&SORA_PARAMS), drop)));
            // (IMPL_ER16) while the scene's figure is shown as him his pose is stepped once a frame in post_physics
            if !matches!(self.cut_draw, CutDraw::Posed(_)) {
                self.pose_before = std::mem::take(&mut self.pose);
                self.step_pose(&out);
            }
            if self.finish.take_collect() {
                // (IMPL_ER22) KH3's AllPrizeCollection when the white-out is full: every orb flies to him
                let count = self.prize.collect_all();
                erlog!("finish: ALL PRIZES fly to Sora ({count} orb(s) on the field)");
            }
            self.prize.after_step(control, wdt, &self.world, &mut self.sora, out.capsule_center, Vec3::new(out.yaw.to_radians().cos(), out.yaw.to_radians().sin(), 0.0)); // (prize) the orbs' step, before the effect side's
            self.savepoint.after_step(control, wdt, &mut self.sora, out.capsule_center); // (savepoint) the recover rule, the effects
            // (SP2) the save point's entry sound, played this step through the effect side's audio queue (whoever leads)
            for play in self.savepoint.take_sounds() {
                static SP_SOUND: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                let n = SP_SOUND.fetch_add(1, std::sync::atomic::Ordering::Relaxed) & 0xFFFF;
                self.fx.audio.0.push(kh_host::seam::AudioCommand::Play(kh_core::SoundHandle(0x7E01_0000 | n), play));
            }
            // (THE USER 2026-11-02: "...while they are guarding plays the 'guard' sound effect instead of hit and they don't get
            // bloody from it") a blow Sora's guard took this step (leader or AI Sora), then every guarded blow's guard sound
            // (guardblow.rs: his row; the friends' were noted by the party's step above), through the same audio queue
            crate::guardblow::sora_step(&out.events, out.capsule_center);
            for play in crate::guardblow::take_sounds() {
                static GUARD_SOUND: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                let n = GUARD_SOUND.fetch_add(1, std::sync::atomic::Ordering::Relaxed) & 0xFFFF;
                self.fx.audio.0.push(kh_host::seam::AudioCommand::Play(kh_core::SoundHandle(0x7E02_0000 | n), play));
            }
            self.fx.effect_dt = wdt; // (IMPL_ER75) the effects (the trail's history and fade) on the world's clock, as his step
            #[cfg(feature = "party")]
            self.fx.set_party(self.party.core().map(|c| (&c.feed, c.visible()))); // (ER party S5) the party's bones, spell actors, shown or not
            // (FX5) Sora out of the party (and not found fighting Margit): every effect of his ends (nothing of an absent
            // Sora is drawn; `submit` already draws neither his body nor his Keyblade)
            #[cfg(feature = "party")]
            self.fx.sora_absent(self.party.core().is_some_and(|c| c.sora_skipped()) && !crate::unlock::sora_staged());
            self.fx.after_step(&out, self.animator.as_ref(), &self.pose, &self.targets, &self.sora); // (fx) the step's sounds and effects
            // KH3's camera updates after the pawn, in the same fixed step (kh-app/src/player.rs).
            let recentre = held_now.has(PadButton::RightThumb) && !self.held_before_r3;
            self.held_before_r3 = held_now.has(PadButton::RightThumb);
            {
                // (IMPL_ER22) the finish's cut, its shake, the world's rate for our cameras
                let shake = self.finish.take_shake();
                let rules = self.finish.rules;
                self.cam.set_finish(crate::cam::FinishCam { shot: self.finish.shot(), enemy: self.finish.enemy(), player_height: rules.player_height, collision: rules.collision, locked: self.finish.locked(), rate, shake });
            }
            // (ER party S7) the party's side of the camera: a friend leader's field / lock-on target, the team attacks' cameras
            #[cfg(feature = "party")]
            let mut party_cam = crate::party::team::camera(&mut self.party);
            #[cfg(feature = "party")]
            let party_cam: Option<&mut dyn crate::cam::PartyCamera> = party_cam.as_mut().map(|c| c as &mut dyn crate::cam::PartyCamera);
            #[cfg(not(feature = "party"))]
            let party_cam: Option<&mut dyn crate::cam::PartyCamera> = None;
            self.cam.step(STEP, crate::cam::StepInput { right: right_stick, left: stick, recentre, mouse: mouse_look /* (CAMP-OPTIONS) a stick: every step of the frame */ }, crate::cam::CamSora { out: &out, before: &self.out, sora: &self.sora }, &self.targets, self.fx.take_shakes(), party_cam);
            self.worldtime.request(self.cam.take_form_game_speed()); // (AUDIT-A WORLDTIME) tate_finish's SetGlobalGameSpeed, from the next step
            if self.pose_before.component.is_empty() {
                self.pose_before = self.pose.clone();
            }
            // (IMPL_ER18) THE INVARIANT "NOT PUSHED BACK": the game moved its body in an action of its own (`own.move.*`: a door, a
            // fog wall, a ladder) and handed it back; for one second after that Sora must stay where the body ended unless the
            // stick moves him. More than 1 m away without input = his own collision threw him out of the place the game
            // put its body (what the user saw at opened doors): said loudly, once.
            if let Some((end, elapsed, input)) = self.action_end {
                let (elapsed, input) = (elapsed + STEP, input || stick.length() > 0.3);
                let away = (out.position - end).length();
                if !input && away > 100.0 {
                    erlog!("!!! PUSHED BACK: {elapsed:.2} s after the game's own action ended and without any stick input Sora is {:.2} m from where the game left its body (by ({:.0}, {:.0}, {:.0}) cm KH3): HIS COLLISION DOES NOT ACCEPT THE PLACE THE GAME MOVED THE BODY TO", away / 100.0, out.position.x - end.x, out.position.y - end.y, out.position.z - end.z);
                    self.action_end = None;
                } else if elapsed >= 1.0 || input {
                    erlog!("own: {elapsed:.2} s after the body's action he is {:.2} m from where the game left the body ({}): not pushed back", away / 100.0, if input { "the stick moves him" } else { "no input" });
                    self.action_end = None;
                } else {
                    self.action_end = Some((end, elapsed, input));
                }
            }
            // (IMPL_ER48) Cure / status-clearing items clear Elden Ring's ailments; (ER party S2) Sora's own body is the player's
            // while he leads, his ally body while a friend leads (S3 cures it)
            #[cfg(feature = "party")]
            crate::status::on_events_of(&out.events, self.sora.status(), kh_host::party::PartyChara::Sora, self.lead.chara());
            #[cfg(not(feature = "party"))]
            crate::status::on_events(&out.events, self.sora.status());
            out.events.iter().for_each(|e| crate::spirit::note_event(&e.kind)); // (THE USER 2026-11-02: "Summon") the core's Summon events
            // The Keyblade's show / hide sequence (the core's events) and its keychain.
            for event in &out.events {
                match event.kind {
                    EventKind::WeaponVisible { visible: true, instant: true } => self.weapon = WeaponShow { visible: true, since: self.weapon.since.max(WeaponShow::DELAY + WeaponShow::RAMP) + if self.weapon.visible { 0.0 } else { 10.0 } },
                    EventKind::WeaponVisible { visible, .. } if visible != self.weapon.visible => self.weapon = WeaponShow { visible, since: 0.0 },
                    // ---- what the core made of the hits on him, and his own targeting (the log is the evidence)
                    EventKind::PlayerHit { hit, hp_lost, hp_after } => erlog!("hit: Sora <- {:?} from target {} for {:.1}: lost {hp_lost}, HP {hp_after}/{}", hit.kind, hit.causer.0, hit.amount, self.sora.status().max_hp()),
                    EventKind::PlayerGuarded { hit, success, critical } => erlog!("hit: Sora GUARDED {:?} from target {} ({:.1} would have been taken): no HP lost{}{}", hit.kind, hit.causer.0, hit.amount, if success { "; counter window open" } else { "" }, if critical { " (just guard)" } else { "" }),
                    EventKind::PlayerGuard { hit, victim, attacker } => erlog!("hit: Sora's guard met {:?} from target {}: he {victim:?}, the attacker {attacker:?}", hit.kind, hit.causer.0),
                    EventKind::PlayerDodged { hit } => erlog!("hit: Sora DODGED {:?} from target {} (the hit does not exist)", hit.kind, hit.causer.0),
                    // (ER party S2) AI Sora down while a friend leads is no game over: the leader's KO is (after_steps)
                    EventKind::PlayerDied { causer } if self.friend_lead().is_some() => erlog!("hit: AI SORA IS DOWN (by target {}): a friend leads, the body lives (the leader's KO is the game over)", causer.0),
                    EventKind::PlayerDied { causer } => {
                        erlog!("hit: SORA DIED (by target {}): the leader is down (DTH: his death takes the one death path of every leader)", causer.0);
                        self.sora_fell = true; // (DTH) taken by the death flow (`death_observe`)
                    }
                    EventKind::CameraLock { target } => erlog!("lock: Sora's lock-on camera target is {}", if target == TargetId::NONE { "none".to_string() } else { format!("target {}", target.0) }),
                    // (AUDIT-C) a face play the core asks for (Rage Form's end: ANTI_01 / ANTI_02, EndStyle 0x14141CA20)
                    EventKind::FaceAnim { row } => {
                        let played = self.animator.as_mut().is_some_and(|a| a.play_face_row(Some(row), "NORMAL_AUTO", [true, true], -1.0, false));
                        erlog!("face: {row} on his face{}", if played { "" } else { ": NO SUCH FACE ROW IN THE PACK (face/face_extra.json)" });
                    }
                    EventKind::BattleMode { on } => erlog!("battle: Sora's battle flag {}{}", if on { "ON" } else { "off" }, if on { format!(" ({})", self.targets.engaged_report()) } else { String::new() }), // (BM) who engaged
                    _ => {}
                }
            }
            kbdraw::frame(control, self.sora.status_mut()); // (P5) test.keyblade / test.kb_form (test profile only)
            if let Some(forced) = control.flag("sora.weapon") {
                if forced != self.weapon.visible {
                    self.weapon = WeaponShow { visible: forced, since: 0.0 };
                }
            }
            // (IMPL_ER44) THE KEYBLADE LIGHT (the user: "When the hero light should activate, make Sora pull out the keyblade
            // and have the keyblade sparkle. It is this effect that creates the light for the hero light"): while the hero
            // light's darkness gate is closed (r.hero_light 1 / 2, hero.keyblade 1) the core holds the Keyblade out (the
            // game's Draw with its appear effect; put away the same way when the gate opens, not in battle); KH3's sparkle
            // effect is started at the blade's tip at an interval while it is drawn; the light itself is placed on the
            // blade by the renderer (pointlight.rs).
            {
                // (the user, 2026-11-03: "keyblade-hero light should never activate in round table hold") the renderer's gate
                // is held off at once in the Hold's map (pointlight.rs `set_hero_banned`): no light, no Keyblade drawn for it
                #[cfg(feature = "party")]
                kh_er_render::pointlight::set_hero_banned(crate::moogle::in_roundtable_hold());
                let wanted = kh_er_render::pointlight::keyblade_wanted();
                // (HEROLIGHT-KB, the user 2026-11-05: "for the hero light, it should just be whoever is party leader, not dependent on
                // Sora") WHO CARRIES THE HERO LIGHT: the party leader, whoever he is (leader.rs `light_carrier`); an AI Sora does not
                // draw his Keyblade for it; a friend leader's weapon carries it (S4 places the light at it: partydraw.rs `lead.light`)
                #[cfg(feature = "party")]
                let leader = {
                    let leader = crate::party::leader::light_carrier(self.lead.chara());
                    self.lead.light = Some(leader);
                    leader
                };
                #[cfg(not(feature = "party"))]
                let leader = kh_host::party::PartyChara::Sora;
                let sora_holds = wanted && leader == kh_host::party::PartyChara::Sora;
                if sora_holds != self.keyblade_held {
                    self.keyblade_held = sora_holds;
                    erlog!("hero light: the Keyblade light is {} (the darkness gate{})", if sora_holds { "ON: Sora draws the Keyblade, it sparkles and lights the dark" } else { "off for Sora: his Keyblade follows its combat rule" }, if sora_holds { " closed" } else if wanted { " closed, but a friend leads: the light is on his weapon" } else { " open, or r.hero_light / hero.keyblade = 0" });
                }
                // (PARTY-WEAPONS) the one hold rule of every character (kh_core::weapon_gate::hold: the carrier only, never in a
                // no-combat place); a friend leader's staff / shield is held by the same rule (kh-party weapon_draw)
                self.sora.hold_weapon(kh_core::weapon_gate::hold(leader == kh_host::party::PartyChara::Sora, wanted));
                #[cfg(feature = "party")]
                if let Some(core) = self.party.core_mut() {
                    for (chara, id) in core.rt.members.clone() {
                        core.rt.party.set_weapon_hold(id, kh_core::weapon_gate::hold(chara == leader, wanted));
                    }
                }
                // (IMPL_ER53, the user: "the glow on the keyblade pulses instead of being constant while in the dark, it must
                // be constant") a STEADY glow (the carrier weapon's own appear effect's glow emitters held) for as long as the
                // light is on, and only its sparkles restarted at the interval; no particle light of the effect (the hero light
                // is the light). (HEROLIGHT-KB) Of THE CARRIER'S WEAPON (sheets/port/sora-herolight.json): Sora's hand Keyblade or
                // his form's hand unit (kh-core `attack_model`), fully drawn (not inside a D-pad switch); a friend leader's
                // staff / shield while it is in his hand and he is drawn. A switch or a leader swap ends the old ones at once.
                let carrier: (Option<String>, bool) = if leader == kh_host::party::PartyChara::Sora {
                    (Some(kbdraw::sound_model()), self.weapon.visible && self.weapon.opacity() >= 1.0 && kbdraw::visibility() >= 1.0)
                } else {
                    #[cfg(feature = "party")]
                    let friend = self.party.core().filter(|c| c.visible()).and_then(|c| {
                        let t = c.feed.taps.iter().find(|t| crate::fx::party::chara_of_member(t.member) == Some(leader))?;
                        let id = kh_party::MemberId(t.member);
                        Some((Some(t.weapon.clone()), !c.rt.party.weapon_hidden(id) && c.rt.party.weapon_ground(id).is_none() && c.rt.party.weapon_view(id).is_some_and(|w| w.opacity >= 1.0) /* (PARTY-WEAPONS) fully drawn, as the Keyblade */))
                    });
                    // (PARTY-WEAPONS) Clippy without the party feature: no literal None unwrapped
                    #[cfg(feature = "party")]
                    let friend = friend.unwrap_or((None, false));
                    #[cfg(not(feature = "party"))]
                    let friend = (None, false);
                    friend
                };
                let lit = wanted && carrier.1;
                self.herolight.step(STEP, lit, carrier.0.as_deref().map(|model| crate::herolight::Carrier { who: leader, model, socket: None }), &mut self.fx.effects);
                // (HEROLIGHT-KB final pass, the user: "all of them have graphical errors") A DUAL-WIELD FORM's LEFT-HAND weapon gets its
                // own glow and sparkles too (Double Form's Oblivion, the Double Arrowguns' left gun, ...): the form's LEFT_HAND unit
                // (`LWeapon@LEFT_HAND:L_wep1_skt=.../w_so023_Wep`) on its own socket; the light stays the right hand's
                let left: Option<(&'static str, &'static str)> = if leader == kh_host::party::PartyChara::Sora {
                    self.sora.combat().form.weapon_units().into_iter().find_map(|u| {
                        let socket = u.split_once("@LEFT_HAND:")?.1.split('=').next().filter(|s| !s.is_empty() && *s != "None")?;
                        Some((kh_core::status::keyblade::model_of(kh_core::status::keyblade::blueprint_of(u)), socket))
                    })
                } else {
                    None
                };
                self.herolight_left.step(STEP, lit && left.is_some(), left.map(|(model, socket)| crate::herolight::Carrier { who: leader, model, socket: Some(socket) }), &mut self.fx.effects);
            }
            // (FREEZE-FX) KH3's frozen look on every frozen enemy, for exactly its freeze; the break at its end (freezefx.rs)
            self.freeze_fx.step(&self.targets, self.world.frame, STEP, &mut self.fx.effects);
            self.weapon.since += STEP;
            self.step_chain(&out);
            self.clock += STEP;
            if let Some(trace) = self.trace.as_mut() {
                let er = self.world.frame.pos_kh_to_er(out.position);
                let _ = writeln!(
                    trace,
                    "{},{:.2},{:.2},{:.2},{:.2},{:?},{},{},{:.1},{:.1},{:.1},{:.4},{:.4},{:.4},{:?},{:.3},{:.3}",
                    out.step, out.position.x, out.position.y, out.position.z, out.yaw, out.state, out.sub_step, out.on_floor as u8,
                    out.velocity.x, out.velocity.y, out.velocity.z, er.x, er.y, er.z, out.anim.name, stick.x, stick.y
                );
            }
            // (IMPL_ER13) ONE LINE PER ATTACK (a new kind / combo count): what was chosen and what the choice saw
            {
                self.atk.steps += 1;
                if !out.on_floor {
                    self.atk.off_floor += 1;
                }
                if out.on_floor != self.out.on_floor {
                    self.atk.flips += 1;
                }
                if self.sora.attack().is_some() {
                    self.atk.sec_steps += 1;
                    self.atk.sec_off += u32::from(!out.on_floor);
                }
                let now = self.sora.attack().map(|a| (a.kind as u32) << 16 | (a.combo as u32 & 0xFF) << 8 | u32::from(a.air));
                if std::mem::replace(&mut self.atk.last, now.unwrap_or(u32::MAX)) != now.unwrap_or(u32::MAX) && control.flag("log.attack") != Some(false) {
                    if let Some(a) = self.sora.attack() {
                        let t = self.targets.get(a.target);
                        let before = &self.out;
                        let rise = kh_core::combat::tables::P.decide_rise_dz;
                        erlog!(
                            "attack: step {} kind {:#04x} combo {} {}{} | chosen from: state {:?} move {:?} on_floor {} z {:.0} vel z {:.0} | since the attack before: {} steps, {} off the floor, {} floor changes | target {} {} | dist {:.0} dz {:.0} => target_high {} (above {:.0})",
                            out.step, a.kind, a.combo, if a.air { "AIR" } else { "GROUND" }, if a.finish { " FINISHER" } else { "" },
                            before.state, before.movement_state, before.on_floor, before.position.z, before.velocity.z,
                            self.atk.steps, self.atk.off_floor, self.atk.flips,
                            a.target.0,
                            t.map_or("none".to_string(), |e| format!(
                                "c{:04} feet {:+.0} from Sora's z, height {:.0}, aim point {:.0} above its feet, its velocity z {:.0} (lead {:+.0}), {}, terrain's share {:+.0}{}{}",
                                e.npc_id, e.pos.z - before.position.z, e.half_height * 2.0, self.targets.aim_height(e), e.velocity.z, e.velocity.z * 8.0 / 30.0,
                                if e.grounded { "on the ground" } else { "NOT on the ground" }, kh_core::Targets::terrain_dz(&self.targets, e.id),
                                if e.airborne { ", IN THE AIR (our puppet)" } else { "" },
                                if self.targets.puppets.count() > 0 { format!(", puppets flying {}", self.targets.puppets.count()) } else { String::new() },
                            )),
                            a.dist, a.dz, a.dz > rise, rise,
                        );
                        (self.atk.steps, self.atk.off_floor, self.atk.flips) = (0, 0, 0);
                    }
                }
            }
            self.out_before = std::mem::replace(&mut self.out, out);
            // (INT) the drawn history of this step: the chandelier pawn's clip time, the team props
            self.int.step(self.sora.xs_chandelier_pawn().map(|p| p.time));
            #[cfg(feature = "party")]
            if let Some(core) = self.party.core() {
                crate::interp::record_props(crate::party::team::props(&core.rt).iter().map(|p| (p.model, p.index, p.trs())));
            }
            self.steps_in_tick += 1;
            held = held_now;
        }
        self.spear_fx(); // (F-SHIP 3, INT-F)
        if std::mem::take(&mut self.stuck_now) && self.follow.is_none() && self.sora.status().hp > 0 && self.friend_lead().is_none() /* (ER party S2) the net and anchor are the leader's */ {
            self.unstick(player, control.flag("stuck.safe") != Some(false));
        }
        // (IMPL_ER24) the reports
        if let Some((before, now)) = self.floor_report.take() {
            if control.flag("log.floor") != Some(false) && self.report_due() {
                crate::falllog::left_floor(&self.world, &before, &now);
            }
        }
        if std::mem::take(&mut self.blocked_now) && self.follow.is_none() && control.flag("log.blocked") != Some(false) && self.report_due() {
            crate::falllog::blocked(&self.world, &self.out);
        }
        // EXP (user decision): an enemy Sora brought to 0 gives him EXP by the one rule of the sheet.
        if let Some(v) = control.f32("exp.rate").filter(|v| *v >= 0.0) {
            self.targets.params.exp_per_max_hp = v;
        }
        // (IMPL_ER22) the kill of a boss whose finish runs (or waits for its confirmation) is held until the white-out
        // is full: its KH3 prize burst and EXP come after the slow motion
        let mut kills = self.finish.release_kills();
        for kill in std::mem::take(&mut self.targets.kills) {
            if self.finish.holds(kill.0) {
                self.finish.held_kills.push(kill);
            } else if self.finish.expects(kill.0) {
                self.finish.note_expected(kill.0);
                self.finish.held_kills.push(kill);
            } else {
                kills.push(kill);
            }
        }
        for (id, max_hp, at) in kills {
            let npc_param = self.targets.get(id).map_or(0, |e| e.npc_param); // (prize)
            self.prize.on_kill(control, &mut self.sora, id.0, npc_param, max_hp, at); // (prize) `prize.kh = 1`: it bursts into KH3 prizes
            self.loot.on_kill(control, &mut self.sora, self.prize.loot_host(), crate::loot::Kill { target: id.0, npc_param, max_hp, at }); // (loot) `loot.kh = 1`: its item lot as KH3 prize boxes
            // (P9b) a REAL death of a drop kind (no load asked since this Sora started, the leader alive, not counted already): KH3's rare weapon drop as a prize box
            let dead = self.death_sent || self.sora.status().hp <= 0;
            self.weapon_src.on_kill(control, &mut self.sora, self.prize.loot_host(), crate::weaponsrc::Kill { target: id.0, npc_param, at, dead });
            let exp = self.targets.params.exp_of(max_hp);
            if exp > 0 {
                // (ER party S6, P2) a friend leading: the award with him as player 0 (his Zero EXP), the shared total
                #[cfg(feature = "party")]
                let party = crate::partyui::give_exp(control, exp, 1.0, Some(at));
                #[cfg(not(feature = "party"))]
                let party = false;
                if !party {
                    self.sora.give_exp(exp, 1.0, Some(at));
                }
            }
            erlog!("combat: target {} (max HP {max_hp}) was KILLED by Sora: base EXP {exp} given (level {}, total EXP {})", id.0, self.sora.status().level, self.sora.status().exp);
        }
        // (IMPL_ER31) boss rewards: the defeat flags, the boss finish's confirmed boss-bar deaths; given when nothing else has the screen
        {
            let deaths = std::mem::take(&mut self.finish.boss_deaths);
            let busy = self.finish.busy() || ui.cutscene || ui.menu || self.cutscene || self.follow.is_some() || paused || self.death_sent || self.sora.status().hp <= 0 || self.flow.down().is_some() /* (DTH) */;
            self.boss_rewards.frame(control, &mut self.sora, player.current_block_id.into(), &deaths, busy, dt);
        }
        self.casts_in_tick += self.world.casts.get() - casts_before;
        // (INT) a discontinuity of the steps is never blended; THE VIEW OF THIS FRAME is posted BEFORE the HUD and the bubbles
        // project their world marks (they project through it: interp.rs `drawn_camera`; before, through the previous
        // frame's camera, a frame behind the picture)
        self.int_snap_check(control);
        self.cam.post(dt, crate::interp::alpha(control, self.accumulator), control);
        self.death_observe(); // (DTH) the leader down, whoever leads: before the HUD (its game over) and `after_steps` (the body)
        let down = self.flow.gameover(); // (DTH) KH3's game over is the down leader's (never AI Sora's, never a scripted death)
        self.hud.frame(&mut self.sora, crate::hud::HudStep { out: &self.out, targets: &self.targets, origin: self.world.frame.origin }, control, ui, down); // (hud) one HUD frame from the latest step
        {
            // (CV1) THE CONVERSATION SYSTEM (conv.rs): the participants, the bubble, the locks, the camera's shots
            #[cfg(feature = "party")]
            let (leader, members, party_battle, leader_name) = crate::conv::party_scene(self.party.core(), &self.out, &self.sora);
            #[cfg(not(feature = "party"))]
            let (leader, members, party_battle, leader_name) = (crate::conv::sora_pose(&self.out), Vec::new(), self.sora.actions().battle, "Sora".to_string());
            let (drawn_leader, drawn_members) = self.int_drawn_feet(control); // (INT) the bubbles' faces where the bodies are drawn
            // (CV3) the leader's fight: party_scene counts Sora's battle flag only while he is in the active party (out of
            // it his flag is frozen: every conversation ended "by combat" in the user's session with Goofy leading)
            let scene = crate::conv::Scene { frame: self.world.frame, dt, leader, leader_name, members, combat: party_battle, hidden: (ui.menu && !self.savepoint.holds_pad()) /* (CV3) a driven story rest: her talk is our bubble, it runs */ || ui.cutscene || self.cutscene, drawn_leader, drawn_members };
            crate::conv::frame(control, &scene);
            crate::talkkh::frame(control, crate::talkkh::TalkCtx { cutscene: ui.cutscene || self.cutscene, in_battle: scene.combat, er_menu: self.popup.is_some_and(|p| p.top_job != 0), story_talk: self.savepoint.holds_pad() /* (CV3) */, leader_name: scene.leader_name.clone() /* (CV3) */, dt }); // (CV2) Elden Ring's character conversations as KH bubbles (`talk.kh`)
            crate::shopkh::frame(control, &self.sora, ui.cutscene || self.cutscene, dt); // (SHOP) the KH shop for the merchants' windows
            crate::boc::talk_frame(control, ui.cutscene || self.cutscene, dt); // (THE USER 2026-11-02) Boc's "Alter garments" -> the KH Outfits screen
            crate::spirittune::talk_frame(control, &self.sora, ui.cutscene || self.cutscene, dt); // (THE USER 2026-11-02) Roderika's "Spirit Tuning" -> the KH Spirit Tuning screen
            crate::forgekh::talk_frame(control, ui.cutscene || self.cutscene, dt); // (P7 SMITHS, the user 2026-11-03) any smith's reinforcement menu -> KH3's Keyblade Forge; his other options trimmed
            crate::gestureprompt::frame(control, crate::gestureprompt::FrameIn { hidden: ui.menu || ui.cutscene || self.cutscene }); // (EMOTE-PROMPTS) a known gesture's quest event as a Y command
            #[cfg(feature = "party")]
            crate::moogle::frame(control, &crate::moogle::FrameIn { dt, frame: self.world.frame, hidden: ui.cutscene || self.cutscene, in_battle: scene.combat }); // (P8) the Synthesis Moogle at Roundtable Hold
        }
        crate::save::choice_tick(&mut self.sora); // (IMPL_ER11) the opening's choices were made
        crate::save::tick(&self.sora, control); // (save)
        // The view of this frame for the game's camera step (which runs after this phase).
        self.worldtime.post(&self.targets, control); // (AUDIT-A WORLDTIME) the enemies' animation at WorldSlow's rate
        self.finish.form_rate = self.worldtime.form_rate(); // (AUDIT-A WORLDTIME) the hit-stops on Elden Ring's game speed too
        self.finish.post(); // (IMPL_ER22) Elden Ring's game speed for this frame (put back exactly when no sequence runs), the white plate
        // (INT) the view of this frame was posted before the HUD (above)
        self.after_steps(player, control, r3, mask != 0);
        if let (Some(yaw), None) = (self.follow_yaw, self.friend_lead()) { // (ER party S2) Sora's own yaw only when he leads
            self.out.yaw = yaw;
        }
        // (LADDER2) at a ladder the body is not moved until the game has let it go (ladder.rs `LetGo`)
        if self.follow.is_some() || !self.ladder_go.body_free() {
            self.written = None;
        } else {
            self.write(player);
        }
        // (ER party S3) THE AI ALLIES' INVISIBLE ELDEN RING BODIES (allybody.rs): made, placed at the friends (S1's want
        // list of the latest party step), topped up; their hits made final and queued for the party's next step
        #[cfg(feature = "party")]
        {
            let wants = self.party.core().map(|c| c.feed.wants.clone());
            // (ER party S8) the staged friends' bodies too (a not yet unlocked friend fighting a boss: its AI targets him)
            let wants = wants.map(|mut w| {
                w.extend(crate::unlock::stage_wants());
                w.extend(crate::spirit_mimic::want()); // (MIMIC SORA) his body: the pool's fourth key
                w
            });
            let (leader_feet, leader_yaw) = (self.lead_pos(), self.lead_yaw());
            crate::allybody::pre_physics(&mut crate::allybody::Ctx { player, frame: self.world.frame, targets: &mut self.targets, control, leader_feet, leader_yaw, dt, paused, wants });
            crate::mimic_boss::write_bodies(self.world.frame); // (MIMIC-BOSS) the Mimic Tear boss's body (and its Mimic's) at their copies
            let (sora_hp, sora_max) = (self.sora.status().hp, self.sora.status().max_hp());
            let params = self.targets.params;
            if let Some(core) = self.party.core_mut() {
                crate::allybody::deliver(core, sora_hp, sora_max, &params);
            }
            self.seat_caught_allies(); // (GRABS) the AI friends a throw holds: seated on their bodies
            // (ER party I, X3G) Goofy's Over Here!: the enemies in its reach are given a hit from his Elden Ring body
            crate::party::taunt::frame(self.party.core(), player.chr_ins.field_ins_handle, &mut self.targets, control, dt);
            // (DON-PROG) Donald's aggro (Low Profile / Hide Behind / Distraction / Taunt Pass / Out of Sight): the hate route, never a hit moved
            crate::party::donaggro::hate(self.party.core(), player.chr_ins.field_ins_handle, &mut self.targets, control, Some(self.sora.output().position), dt);
            // (AGGRO, wired by GF-THREAT) THE DIRECTOR: whom each engaged enemy attacks. Without this call it never ran (its log held no line)
            let sora_pos = self.sora.output().position;
            crate::party::aggro::frame(self.party.core_mut(), player.chr_ins.field_ins_handle, &mut self.targets, control, sora_pos, dt);
        }
        self.barrier_frame(control, dt, paused); // (DEATHPLANE) Elden Ring's death barriers, for every KH character
        self.work += started.elapsed();
    }

    /// The control file's switches of the hookup (read every frame; all have defaults).
    fn host_keys(&mut self, control: &Control) {
        let t = &mut self.targets;
        t.bystanders = control.flag("tgt.bystanders") != Some(false);
        t.npcs = control.flag("hit.npcs") != Some(false); // (IMPL_ER51) friendly NPCs are hit by his attacks (never targeted); 0 = bystanders as before
        t.passive_rule = control.flag("target.passive") != Some(false); // (BM) passive wildlife (NpcParam threatLv 0) starts no battle on its own; 0 = as before
        t.disguised_ids = crate::targets::disguised_ids(control.str("hit.disguised")); // (IMPL_ER54) characters in a disguise asset (Boc's bush): hit across it
        // (IMPL_ER23A) `tgt.hover = 0`: no capsule lift for bodies that hover above their position node; `tgt.hover_min = cm`
        t.hover = control.flag("tgt.hover") != Some(false);
        // (IMPL_ER57) `target.visible_only = 0`: undrawn characters are targets as before; `target.drawgroup = 0`: the render switch alone
        t.visible_only = control.flag("target.visible_only") != Some(false);
        t.drawgroups = control.flag("target.drawgroup") != Some(false);
        t.params.hover_min = control.f32("tgt.hover_min").filter(|v| *v >= 0.0).unwrap_or(t.sheet.hover_min);
        // (testing aid) `tgt.range = metres`: how far characters are read.
        if let Some(v) = control.f32("tgt.range").filter(|v| *v > 1.0) {
            t.params.range = v * coords::CM_PER_M;
        }
        // (IMPL_ER14, testing aids: the A / B of the ground-combo fixes) `tgt.point_max = cm`, `tgt.terrain_slope = degrees`
        t.params.point_max_height = control.f32("tgt.point_max").filter(|v| *v > 0.0).unwrap_or(t.sheet.point_max_height);
        t.params.terrain_slope = control.f32("tgt.terrain_slope").filter(|v| (0.0..=60.0).contains(v)).unwrap_or(t.sheet.terrain_slope);
        // (IMPL_ER61, the user: "seriously nerf shotlock" / "increase MP cost of magic") rows damage.shotlock.* and
        // magic.mp_cost_scale; `balance.shotlock = 0` / `balance.mp = 0` give KH3's own numbers
        t.params.shotlock = if control.flag("balance.shotlock") == Some(false) { crate::ermap::ShotlockRules::OFF } else { t.sheet.shotlock };
        // (DF, 2026-10-15) THE DIFFICULTY (Critical by default, kept in Sora's save): on Critical KH3's own halved MP stands
        // in for the 1.5 nerf (row magic.mp_cost_scale.critical = 1; `difficulty.mp_stack = 1`: both, as before)
        let level = self.sora.status().game_level;
        let mp_cost_scale = if control.flag("balance.mp") == Some(false) {
            1.0
        } else if level.is_critical() && !control.on("difficulty.mp_stack") {
            t.sheet.mp_cost_scale_critical
        } else {
            t.sheet.mp_cost_scale
        };
        self.sora.status_mut().mp_cost_scale = mp_cost_scale;
        let t = &mut self.targets;
        if let Some(v) = control.f32("dmg.out").filter(|v| *v > 0.0) {
            t.params.out_scale = v;
        }
        if let Some(v) = control.f32("dmg.in").filter(|v| *v >= 0.0) {
            t.params.in_scale = v;
        }
        // (DF) the difficulty on Elden Ring's hits: KH3's enemy-damage factor x the base-HP ratio (Critical 4, Proud 1.5);
        // `difficulty.in = 0`: off (Standard's 1), a number: that factor
        t.params.diff_in = match control.f32("difficulty.in") {
            Some(0.0) => 1.0,
            Some(v) if v > 0.0 => v,
            _ => crate::ermap::difficulty_in(level),
        };
        if let Some(v) = control.f32("dmg.poise").filter(|v| *v >= 0.0) {
            t.params.poise_scale = v;
        }
        if let Some(v) = control.f32("dmg.body_hp").filter(|v| *v >= 1.0) {
            t.params.body_hp = v;
        }
        if let Some(v) = control.i64("react.landing").filter(|v| (0..=255).contains(v)) {
            t.params.landing_level = v as u8;
        }
        // `dmg.mode = hp`: the old direct HP write; anything else: the game's own pipeline (bullet.rs).
        let mode = if control.str("dmg.mode") == Some("hp") { crate::damage::Mode::Hp } else { crate::damage::Mode::Bullet };
        t.route = crate::damage::Route { mode, fallback: control.flag("dmg.fallback") != Some(false), landing_damage: control.i64("dmg.landing").unwrap_or(0).clamp(0, 60000) as u16, anim: control.i64("dmg.anim").unwrap_or(0) as i32, anim_big: control.i64("dmg.anim_big").unwrap_or(0) as i32, credit: control.flag("dmg.credit") != Some(false), off: control.on("dmg.off") };
        t.shape = crate::bullet::Shape::from_control(control);
        t.bullets.log_rows = control.on("blt.log");
        t.diag = control.on("blt.diag");
        crate::bullet::DUMP.store(control.on("blt.dump"), std::sync::atomic::Ordering::Relaxed);
        if mode == crate::damage::Mode::Bullet && !t.bullets.picked() {
            t.bullets.pick(control);
            if !t.bullets.available() {
                erlog!("damage: the game's own pipeline is NOT available: Sora's hits are written to the targets' HP directly (the stopgap)");
            }
        }
    }

    /// The watched target (testing aid): every change of its animation, HP and poise is logged.
    fn watch_step(&mut self, dt: f32) {
        let Some(w) = self.watch.as_mut() else { return };
        w.seconds += dt;
        let Some(e) = self.targets.get(w.id) else {
            erlog!("watch: target {} is gone after {:.2} s", w.id.0, w.seconds);
            self.watch = None;
            return;
        };
        let moved = (e.er_pos - w.from) * Vec3::new(1.0, 0.0, 1.0);
        w.farthest = w.farthest.max(moved.length());
        w.highest = w.highest.max(e.er_pos.y - w.from.y);
        if e.anim != w.anim || e.hp != w.hp || (e.poise - w.poise).abs() > 0.5 {
            erlog!("watch: {:.2} s target {} (c{:04}): animation {} -> {}, hp {} -> {}, poise {:.0} -> {:.0}/{:.0}; moved {:.2} m, height {:+.2} m{}", w.seconds, w.id.0, e.npc_id, w.anim, e.anim, w.hp, e.hp, w.poise, e.poise, e.poise_max, moved.length(), e.er_pos.y - w.from.y, if e.alive { "" } else { " DEAD" });
            (w.anim, w.hp, w.poise) = (e.anim, e.hp, e.poise);
        }
        if w.seconds > 6.0 {
            erlog!("watch: target {} over: animation {} hp {}; farthest {:.2} m, highest {:+.2} m", w.id.0, e.anim, e.hp, w.farthest, w.highest);
            self.watch = None;
        }
    }

    fn watch_start(&mut self, id: TargetId) {
        if let Some(e) = self.targets.get(id) {
            self.watch = Some(Watch { id, seconds: 0.0, anim: e.anim, hp: e.hp, poise: e.poise, from: e.er_pos, farthest: 0.0, highest: 0.0 });
        }
    }

    /// The Tarnished goes back to his own maximum HP (Sora stops; the payload is replaced).
    pub fn stop(&mut self, player: &mut PlayerIns) {
        self.worldtime.stop(); // (AUDIT-A WORLDTIME) every slowed body given back
        self.finish.stop(); // (IMPL_ER22) a running boss finish ends: Elden Ring's game speed is put back
        self.loot.restore("Sora stops"); // (loot) the enemy types get their own item lots back
        self.attackers.restore("Sora stops"); // (ENG) the enemies' own TeamAttackEffectivity
        crate::spirit::stop("Sora stops"); // (THE USER 2026-11-02: "Summon") the spirit goes with him (the game's remove-now)
        self.prize.stop(&mut self.sora); // (prize) the orbs and their effects go; munny still in orbs is his (IMPL_ER20), before the file is written
        crate::save::on_stop(&self.sora); // (save)
        self.savepoint.stop(); // (savepoint) the save points go, the graces get their own drawing back
        crate::guardblow::release_body_sounds(&mut player.chr_ins, "Sora stops"); // (THE USER 2026-11-02) the body's sound switch back if we held it
        if let Some(b) = self.body.take() {
            let data = player.chr_ins.modules.data.as_mut();
            if data.base_hp == b.written_base && b.written_base != b.own_base {
                data.base_hp = b.own_base;
                data.hp = data.hp.min(b.own_max.max(1));
                erlog!("death: the Tarnished's base HP put back to his own {}", b.own_base);
            }
        }
        if std::mem::take(&mut self.no_dead_held) {
            use fromsoftware_shared::FromStatic;
            // SAFETY: game thread.
            if let Ok(flags) = unsafe { eldenring::cs::WorldChrManDbgFlags::instance_mut() } {
                flags.player_no_dead = false;
                NO_DEAD_HELD.store(false, std::sync::atomic::Ordering::Relaxed);
                erlog!("fall: the game's no-death switch put back to {} ({} HP of fall damage were discarded this time)", flags.player_no_dead, self.fall.discarded);
            }
        }
        self.fast_travel.stop(); // (IMPL_ER23A) the game's own fast-travel flag back
        self.lab_restore();
        erlog!("bullets: {}", self.targets.stats_line());
        self.targets.puppets.release_all("Sora stopped");
        self.targets.bullets.restore_all();
        crate::donstatus::restore_flattened("Sora stopped"); // (DON-MAGIC gravity) no character is left flat
    }

    /// `dbg.ui = 1`: every change of Elden Ring's interface elements is logged (to find which one
    /// is the interaction prompt); `in.prompt_ui = N`: element N visible = a prompt is on screen.
    fn watch_ui(&mut self, control: &Control) {
        let now = inputs::ui_states();
        if control.on("dbg.ui") {
            if let (Some(a), Some(b)) = (self.ui_states.as_ref(), now.as_ref()) {
                let changes: Vec<String> = (0..a.len()).filter(|k| a[*k] != b[*k]).map(|k| format!("{k}: {} -> {}", a[k], b[k])).collect();
                if !changes.is_empty() {
                    erlog!("ui: elements (index: created | visible << 1) {}", changes.join(", "));
                }
            }
        }
        // (MEASURED, IMPL_ER4_STATE.md run 1: element 19 is the action-button prompt, "Y: Rest at site of grace")
        self.prompt = match (Some(control.i64("in.prompt_ui").unwrap_or(19)), now.as_ref()) {
            (Some(k), Some(states)) if (0..states.len() as i64).contains(&k) => states[k as usize] & 2 != 0,
            _ => false,
        };
        // (RC) what the game's action-button manager shows (its text: KH3's green reaction command, hud.rs); with
        // `prompt.er_hide` 1 its own box's fields are cleared after the read, so the manager's state also counts as "up"
        let hide = control.flag("prompt.er_hide") != Some(false); // (coordinator 2026-10-22) ON by default: the user saw it work with no lost interactions
        let manager_up = crate::erprompt::read(hide);
        if hide && manager_up {
            self.prompt = true;
        }
        // (LADDER2) GOING DOWN A LADDER IS NOT OFFERED (the user: "Disable the prompt and functionality for going down the
        // ladder, as the player can just fall down it"): the game's "Descend" prompt is no prompt for us (no KH3 command shown,
        // Y never reaches the game for it: the mask keeps Y from it), whoever leads
        let descend = crate::erprompt::shown().is_some_and(|s| crate::ladder::is_descend_prompt(s.text_id, s.text.as_deref()));
        if descend && self.prompt {
            self.prompt = false;
            if !self.descend_told {
                erlog!("ladder: (LADDER2) the game offers its ladder's climb down (\"Descend\"): NOT OFFERED (no command, Y is not the game's): walk off the edge to go down");
            }
        }
        self.descend_told = descend;
        // (TELESCOPE, the user 2026-10-27: "disable using eagle eye telescopes entirely, give no prompt for them") the second
        // line under telescope.rs (their rows' region is nothing): a telescope prompt the manager still shows (a reload of
        // the params before the next check) is no prompt for us: no KH3 command, Y never reaches the game for it
        let telescope = crate::erprompt::shown().is_some_and(|s| crate::telescope::hides_prompt(control, s.text_id, s.text.as_deref()));
        if telescope && self.prompt {
            self.prompt = false;
            if !self.telescope_told {
                erlog!("telescope: (TELESCOPE) the game offers a Birdseye Telescope: NOT OFFERED (no command, Y is not the game's; telescope.er = 1 gives it back)");
            }
        }
        self.telescope_told = telescope;
        // (SUMMON-POOL, the user 2026-11-10: "Disable summoning pools") the second line under summonpool.rs: a pool's prompt
        // (its ActionButtonParam row) the manager still shows is no prompt for us: no KH3 command, Y never reaches the game for it
        let pool = crate::erprompt::shown().filter(|s| crate::summonpool::hides_prompt(control, s.row)).map(|s| s.row);
        if let Some(row) = pool.filter(|_| self.prompt) {
            self.prompt = false;
            if !self.pool_told {
                erlog!("{}", crate::summonpool::told_line(row as u32));
            }
        }
        self.pool_told = pool.is_some();
        crate::erprompt::set_er_up(self.prompt);
        self.ui_states = now;
    }

    /// A tutorial popup stops Elden Ring's world while our tasks keep running: the player's
    /// animation clock stands still. Sora is paused while a menu has the input AND that clock
    /// has not moved for six frames.
    fn world_paused(&mut self, player: &PlayerIns) -> bool {
        let ta = &player.chr_ins.modules.time_act;
        let anim = &ta.anim_queue[(ta.read_idx as usize) % 10];
        if (anim.play_time, anim.anim_id) == (self.still.0, self.still.1) {
            self.still.2 = self.still.2.saturating_add(1);
        } else {
            self.still = (anim.play_time, anim.anim_id, 0);
        }
        let paused = self.ui.menu && self.still.2 >= 6;
        if paused != self.paused {
            erlog!("ui: Elden Ring's world {}", if paused { "STANDS STILL (a popup): Sora is paused" } else { "runs again: Sora goes on" });
            self.paused = paused;
        }
        paused
    }

    /// A character by the text of a testing key: a target id, `cNNNN` (the nearest living one of that
    /// character id) or anything else = the nearest living hostile.
    fn pick(&self, text: &str) -> Option<targets::Enemy> {
        let text = text.trim();
        // (an INACTIVE character's position is not in the current physics frame: sending Sora there
        // put him 80 m under the world and the game killed its player; measured, run 1)
        if let Ok(id) = text.parse::<u32>() {
            return self.targets.get(TargetId(id)).filter(|e| e.active).cloned();
        }
        if let Some(npc) = text.strip_prefix('c').and_then(|n| n.parse::<i32>().ok()) {
            let mut all: Vec<&targets::Enemy> = self.targets.list.iter().filter(|e| e.npc_id == npc && e.alive && e.active).collect();
            all.sort_by(|a, b| (a.pos - self.out.position).length().total_cmp(&(b.pos - self.out.position).length()));
            return all.first().map(|e| (*e).clone());
        }
        self.targets.hostiles_by_distance().first().map(|(_, e)| (*e).clone())
    }

    /// The testing keys (one-shots: a change of the value fires; documented in IMPL_ER4_STATE.md).
    fn debug_keys(&mut self, control: &Control) {
        if self.shots.chrs.fired(control).is_some() {
            self.targets.dump();
        }
        // `dbg.goto = nearest | <target id>`: Sora is put 3 m in front of that character.
        if let Some(text) = self.shots.goto.fired(control) {
            let wanted = self.pick(&text);
            match wanted {
                Some(e) => {
                    // At most 30 m a time (a longer jump of the player made the game kill him: run 1),
                    // and onto ground found by a cast down (a place under the terrain is a fall out of the world).
                    let feet = self.feet_er();
                    let away = (feet - e.er_pos) * Vec3::new(1.0, 0.0, 1.0);
                    let wanted = e.er_pos + away.normalize_or(Vec3::X) * (3.0 + e.radius / coords::CM_PER_M);
                    let hop = ((wanted - feet) * Vec3::new(1.0, 0.0, 1.0)).clamp_length_max(30.0);
                    let over = Vec3::new(feet.x + hop.x, feet.y.max(e.er_pos.y) + 40.0, feet.z + hop.z);
                    let mut hits = Vec::new();
                    crate::havok::cast_sphere(over, Vec3::NEG_Y * 120.0, 0.2, self.world.filter, false, &mut hits);
                    hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
                    match hits.first() {
                        Some(ground) => {
                            let to = Vec3::new(over.x, ground.position.y + 0.3, over.z);
                            self.world.frame.rebase(to - feet);
                            self.targets.rebased();
                    self.targets.puppets.release_all("Sora was warped: the KH3 frame moved under the enemies");
                            erlog!("dbg: Sora put at ({:.2}, {:.2}, {:.2}), {:.1} m from target {} (c{:04})", to.x, to.y, to.z, (to - e.er_pos).length(), e.id.0, e.npc_id);
                        }
                        None => erlog!("dbg: goto {text}: no ground found under the place {:.1} m toward target {}: not moved", hop.length(), e.id.0),
                    }
                }
                None => erlog!("dbg: goto {text}: no such target near him"),
            }
        }
        // `dbg.spawn = nearest | <target id>`: the game's debug character creator makes a copy of
        // that character 4 m in front of Sora.
        if let Some(text) = self.shots.spawn.fired(control) {
            let like = self.pick(&text);
            match like {
                Some(e) => {
                    let at = self.feet_er() + coords::facing_er(self.out.yaw) * 4.0 + Vec3::Y * 0.3;
                    let asked = targets::debug_spawn(&e, at);
                    erlog!("dbg: spawn of a copy of target {} (c{:04}, NpcParam {}, think {}) at ({:.2}, {:.2}, {:.2}): {}", e.id.0, e.npc_id, e.npc_param, e.think_param, at.x, at.y, at.z, if asked { "asked of the game's debug creator" } else { "no debug creator" });
                }
                None => erlog!("dbg: spawn {text}: no such target near him"),
            }
        }
        // `dbg.hurt = N[,kind]`: a hit of N on Sora out of nowhere (kind: ETresDamageKind, 1 = flinch).
        if let Some(text) = self.shots.hurt.fired(control) {
            let mut parts = text.split(',').map(|p| p.trim().parse::<f32>().ok());
            if let Some(Some(amount)) = parts.next() {
                let kind = parts.next().flatten().and_then(|k| kh_core::enums::ETresDamageKind::from_value(k as i32)).unwrap_or(kh_core::enums::ETresDamageKind::Small);
                let from = self.out.position + coords::dir_er_to_kh(coords::facing_er(self.out.yaw)) * 1.5;
                self.sora.take_hit(HitOnPlayer { direction: (self.out.position - from).normalize_or_zero(), ..HitOnPlayer::new(kind, amount, TargetId::NONE, from) });
                erlog!("dbg: a {kind:?} hit of {amount} on Sora (dbg.hurt)");
            }
        }
        // `dbg.launch = nearest | <id> | cNNNN [,kind]` (with `puppet = 1`): a reaction of that
        // ETresDamageKind (default 8 LAUNCH) is asked of the character without any hit (for timing screenshots).
        if let Some(text) = self.shots.launch.fired(control) {
            let mut parts = text.split(',');
            let who = self.pick(parts.next().unwrap_or(""));
            let kind = parts.next().and_then(|k| k.trim().parse::<i32>().ok()).unwrap_or(8);
            match who {
                Some(e) => {
                    let direction = ((e.pos - self.out.position) * Vec3::new(1.0, 1.0, 0.0)).normalize_or(Vec3::X);
                    let asked = kh_core::enums::ETresDamageKind::from_value(kind).map(|k| self.targets.ask_reaction(e.id, k, direction));
                    erlog!("dbg: reaction kind {kind} asked of target {} (c{:04}): {asked:?}", e.id.0, e.npc_id);
                    self.watch_start(e.id);
                }
                None => erlog!("dbg: launch {text}: no such target near him"),
            }
        }
        // `dbg.atk = who,level[,phys[,poise[,down]]]`: ONE BULLET of ours at that character with that
        // Elden Ring dmgLevel, attack value (default 1) and poise damage (default 200), from Sora's
        // side (or from above with a fifth value); what the game does with it is logged (`watch:`).
        if let Some(text) = self.shots.atk.fired(control) {
            let parts: Vec<&str> = text.split(',').collect();
            let who = self.pick(parts.first().copied().unwrap_or(""));
            let num = |k: usize, d: f32| parts.get(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(d);
            match who {
                Some(e) => {
                    let attack = crate::bullet::Attack { phys: num(2, 1.0).clamp(0.0, 60000.0) as u16, poise: num(3, 200.0), stamina: 0, level: num(1, 1.0).clamp(0.0, 255.0) as u8, ..crate::bullet::Attack::PLAIN };
                    self.targets.bullets.pick(control);
                    match self.targets.test_attack(e.id, attack, parts.len() < 5, None) {
                        Ok(what) => erlog!("dbg: {attack:?} at target {} (c{:04}, {:?}, puppet_ok {}): {what}", e.id.0, e.npc_id, e.weight, e.puppet_ok),
                        Err(why) => erlog!("dbg: atk {text}: {why}"),
                    }
                    self.watch_start(e.id);
                }
                None => erlog!("dbg: atk {text}: no such target near him"),
            }
        }
        if let Some(text) = self.shots.raw.fired(control) {
            let parts: Vec<&str> = text.split(',').collect();
            match (self.pick(parts.first().copied().unwrap_or("")), parts.get(1).and_then(|v| v.trim().parse::<u32>().ok())) {
                (Some(e), Some(bullet)) => {
                    erlog!("dbg: raw bullet {bullet} at target {} (c{:04}): {:?}", e.id.0, e.npc_id, self.targets.test_raw(e.id, bullet));
                    self.watch_start(e.id);
                }
                _ => erlog!("dbg: raw {text}: who,bulletId"),
            }
        }
        if let Some(text) = self.shots.watch.fired(control) {
            match self.pick(&text) {
                Some(e) => self.watch_start(e.id),
                None => erlog!("dbg: watch {text}: no such target near him"),
            }
        }
        // `dbg.travel = cNNNN | <id> | nearest` (testing aid): Sora is taken to that character by hops
        // of at most 30 m, one a second (a longer jump of the player made the game kill him), until
        // he is within 6 m; taking the key away stops it.
        if let Some(text) = self.shots.travel.fired(control) {
            self.travel = Some((text, 0.0));
        }
        if control.str("dbg.travel").is_none() {
            self.travel = None;
        }
        if let Some((who, wait)) = self.travel.as_mut() {
            *wait -= 1.0 / 60.0;
            if *wait <= 0.0 {
                *wait = 1.0;
                let who = who.clone();
                // (travel may head for a character that is not active yet: only the DIRECTION of its
                // place is used, the hop lands on ground found by a cast; it becomes active on the way)
                let far = who.trim().strip_prefix('c').and_then(|n| n.parse::<i32>().ok()).and_then(|npc| {
                    let mut all: Vec<&targets::Enemy> = self.targets.list.iter().filter(|e| e.npc_id == npc && e.alive).collect();
                    all.sort_by(|a, b| (a.pos - self.out.position).length().total_cmp(&(b.pos - self.out.position).length()));
                    all.first().map(|e| (*e).clone())
                });
                match self.pick(&who).or(far) {
                    Some(e) => {
                        let feet = self.feet_er();
                        let gap = ((e.er_pos - feet) * Vec3::new(1.0, 0.0, 1.0)).length();
                        if gap < 6.0 + e.radius / coords::CM_PER_M {
                            erlog!("dbg: travel: arrived, {gap:.1} m from target {} (c{:04})", e.id.0, e.npc_id);
                            self.travel = None;
                        } else {
                            let wanted = e.er_pos + ((feet - e.er_pos) * Vec3::new(1.0, 0.0, 1.0)).normalize_or(Vec3::X) * (3.5 + e.radius / coords::CM_PER_M);
                            let hop = ((wanted - feet) * Vec3::new(1.0, 0.0, 1.0)).clamp_length_max(30.0);
                            let over = Vec3::new(feet.x + hop.x, feet.y.max(e.er_pos.y) + 40.0, feet.z + hop.z);
                            let mut hits = Vec::new();
                            crate::havok::cast_sphere(over, Vec3::NEG_Y * 160.0, 0.2, self.world.filter, false, &mut hits);
                            hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
                            match hits.first() {
                                Some(ground) => {
                                    let to = Vec3::new(over.x, ground.position.y + 0.3, over.z);
                                    self.world.frame.rebase(to - feet);
                                    self.targets.rebased();
                    self.targets.puppets.release_all("Sora was warped: the KH3 frame moved under the enemies");
                                    erlog!("dbg: travel: hop to ({:.1}, {:.1}, {:.1}), {:.1} m left to target {} (c{:04})", to.x, to.y, to.z, (to - e.er_pos).length(), e.id.0, e.npc_id);
                                }
                                None => {
                                    erlog!("dbg: travel: no ground under the next hop: stopped");
                                    self.travel = None;
                                }
                            }
                        }
                    }
                    None => {
                        erlog!("dbg: travel {who}: no such active character in range: stopped");
                        self.travel = None;
                    }
                }
            }
        }
        // `dbg.hop = dx,dz[,n]` (testing aid): Sora is moved by (dx, dz) metres (Elden Ring axes: +z = KH3 yaw 0,
        // +x = yaw 90) n times, a hop every 0.6 s, each onto ground found by a cast down; at most 28 m a hop.
        if let Some(text) = self.shots.hop.fired(control) {
            let v: Vec<f32> = text.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            if v.len() >= 2 {
                self.hop = Some((Vec3::new(v[0], 0.0, v[1]).clamp_length_max(28.0), v.get(2).map_or(1, |n| (*n as u32).clamp(1, 200)), 0.0));
            }
        }
        #[cfg(feature = "party")]
        let unlock_hops = crate::unlock::hop_active(); // (ER party S8) `unlock.goto` hops with this mover
        #[cfg(not(feature = "party"))]
        let unlock_hops = false;
        if control.str("dbg.hop").is_none() && !unlock_hops {
            self.hop = None;
        }
        #[cfg(feature = "party")]
        if let Some((delta, n)) = crate::unlock::take_hop() {
            self.hop = Some((delta.clamp_length_max(28.0), n.clamp(1, 400), 0.0)); // (ER party S8)
        }
        if let Some((delta, left, wait)) = self.hop {
            let wait = wait - 1.0 / 60.0;
            if wait > 0.0 {
                self.hop = Some((delta, left, wait));
            } else {
                let feet = self.feet_er();
                let over = Vec3::new(feet.x + delta.x, feet.y + 40.0, feet.z + delta.z);
                let mut hits = Vec::new();
                crate::havok::cast_sphere(over, Vec3::NEG_Y * 160.0, 0.2, self.world.filter, false, &mut hits);
                hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
                match hits.first() {
                    Some(ground) => {
                        let to = Vec3::new(over.x, ground.position.y + 0.3, over.z);
                        self.world.frame.rebase(to - feet);
                        self.targets.rebased();
                        self.targets.puppets.release_all("Sora was warped: the KH3 frame moved under the enemies");
                        erlog!("dbg: hop to ({:.1}, {:.1}, {:.1}), {} left", to.x, to.y, to.z, left - 1);
                        self.hop = (left > 1).then_some((delta, left - 1, 0.6));
                    }
                    None => {
                        erlog!("dbg: hop: no ground under the next hop: stopped");
                        self.hop = None;
                    }
                }
            }
        }
        // (IMPL_ER14, testing aid) `dbg.findslope = lo,hi[,metres]`: the nearest ground whose slope is between lo and hi
        // degrees (casts down on rings around him, the four neighbours 1.5 m away must be in the range too): Sora is put
        // there; the line says which way is downhill (Elden Ring axes and KH3 yaw).
        if let Some(text) = self.shots.findslope.fired(control) {
            let v: Vec<f32> = text.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            if v.len() >= 2 {
                let (lo, hi, reach) = (v[0], v[1], v.get(2).copied().unwrap_or(40.0).clamp(4.0, 60.0));
                let feet = self.feet_er();
                let filter = self.world.filter;
                let ground = |x: f32, z: f32| -> Option<(Vec3, f32)> {
                    let mut hits = Vec::new();
                    crate::havok::cast_sphere(Vec3::new(x, feet.y + 30.0, z), Vec3::NEG_Y * 80.0, 0.1, filter, false, &mut hits);
                    hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
                    hits.first().map(|h| (h.position, h.normal.normalize_or(Vec3::Y).y.clamp(-1.0, 1.0).acos().to_degrees()))
                };
                let mut found = None;
                let mut casts = 0;
                'rings: for ring in 1..=(reach / 2.0) as u32 {
                    let r = ring as f32 * 2.0;
                    let n = (ring * 6).max(8);
                    for k in 0..n {
                        let a = k as f32 / n as f32 * std::f32::consts::TAU;
                        let (x, z) = (feet.x + r * a.cos(), feet.z + r * a.sin());
                        casts += 1;
                        let Some((at, slope)) = ground(x, z) else { continue };
                        if slope < lo || slope > hi || casts > 3000 {
                            continue;
                        }
                        let around = [(1.5, 0.0), (-1.5, 0.0), (0.0, 1.5), (0.0, -1.5)].map(|(dx, dz)| ground(x + dx, z + dz));
                        casts += 4;
                        if around.iter().all(|g| g.is_some_and(|(p, s)| s >= lo && s <= hi && (p.y - at.y).abs() < 2.0)) {
                            // downhill: from the higher neighbours to the lower
                            let h = |k: usize| around[k].map_or(at.y, |g| g.0.y);
                            let down = Vec3::new(h(1) - h(0), 0.0, h(3) - h(2)).normalize_or(Vec3::X);
                            found = Some((at, slope, down));
                            break 'rings;
                        }
                    }
                }
                match found {
                    Some((at, slope, down)) => {
                        let to = at + Vec3::Y * 0.3;
                        self.world.frame.rebase(to - feet);
                        self.targets.rebased();
                        self.targets.puppets.release_all("Sora was warped: the KH3 frame moved under the enemies");
                        // (a fourth value: 1 = he is turned to face uphill, -1 = downhill)
                        if let Some(face) = v.get(3).copied().filter(|f| *f != 0.0) {
                            let yaw = coords::yaw_kh_of_er_dir(if face > 0.0 { -down } else { down });
                            self.sora.teleport(Pose { position: self.out.position, yaw });
                            self.out.yaw = yaw;
                            self.out_before.yaw = yaw;
                        }
                        erlog!("dbg: findslope: ground of {slope:.0} deg found {:.1} m away after {casts} casts: Sora put at ({:.2}, {:.2}, {:.2}); DOWNHILL is Elden Ring ({:.2}, {:.2}) = KH3 yaw {:.0}", ((to - feet) * Vec3::new(1.0, 0.0, 1.0)).length(), to.x, to.y, to.z, down.x, down.z, coords::yaw_kh_of_er_dir(down));
                    }
                    None => erlog!("dbg: findslope: no ground of {lo:.0} .. {hi:.0} deg within {reach:.0} m ({casts} casts)"),
                }
            }
        }
        if self.shots.heal.fired(control).is_some() {
            self.sora.status_request(kh_core::StatusRequest::FullRecover);
            erlog!("dbg: Sora fully recovered (dbg.heal)");
        }
    }

    /// DAMAGE IN (item 3): the game took HP off its own player since we last wrote it = an enemy's
    /// attack (or damage over time) reached the passive body. The loss is put back (in
    /// `after_steps`) and handed to Sora as a hit of the same share of HIS maximum; KH3's own
    /// rules decide what it does (guard, Second Chance, flinch or blow, death).
    fn damage_in(&mut self, player: &PlayerIns, control: &Control) {
        // (the maximum is HIS OWN, not the raised one we hold the body at: the one-shot rule)
        let (hp, max) = (player.chr_ins.modules.data.hp, self.body.filter(|b| b.settle == 0).map_or(player.chr_ins.modules.data.max_hp, |b| b.own_max));
        let Some(written) = self.hp_written else { return };
        // (HP exactly at the game's current maximum is the game clamping it to a maximum it computed
        // afresh, never a hit: a hit leaves less than the maximum)
        if hp >= player.chr_ins.modules.data.max_hp && hp < written {
            self.hp_written = Some(hp);
            return;
        }
        if self.death_sent || hp >= written || hp < 0 || max <= 0 || !self.lead_alive() /* (ER party S2) the body's leader */ {
            return;
        }
        let lost = written - hp;
        crate::sensorlab::on_hit(&player.chr_ins, lost, "the leader"); // (GF-SENSOR)
        // (GRABS) THE RAYA LUCARIA ABDUCTOR'S BLOWS: in Elden Ring they are its way of carrying its player off (the death,
        // then the respawn at Volcano Manor its event set); here nothing reaches the leader and the warp at the release
        // carries the party (grabs.rs). No hit, no death, no game over; the body is topped up as always.
        if self.grab.on && self.abduct.is_some() {
            erlog!("hit: (GRABS) the Abductor's blow took {lost} HP off the body: not the leader's (its grab carries the party to Volcano Manor)");
            self.hp_written = Some(hp);
            return;
        }
        // (IMPL_ER23A, the user: "I've had Sora die from fall damage because the invisible elden ring character died instead of
        // him") THE GAME'S FALL DAMAGE IS NOT SORA'S: KH3 has none. What the game takes off its body while he is on a drop of
        // 4 m or more, or within 0.75 s after he landed from one (the body lands by itself a few frames after him), is
        // discarded: no hit on Sora, the body is topped up as always. (`fall.guard = 0`: as before.)
        let lead_floor = self.lead_on_floor(); // (ER party S2) the body's leader
        if control.flag("fall.guard") != Some(false) && self.fall.in_window(lead_floor) {
            self.fall.discarded += lost as i64;
            erlog!(
                "fall: the game took {lost} HP off its body ({hp} left of {}) {}: THE GAME'S FALL DAMAGE, discarded: Sora takes none{}",
                player.chr_ins.modules.data.max_hp,
                if lead_floor { "right after his landing from a long drop".to_string() } else { format!("while he is {:.1} m below the floor he left", self.fall.drop() / 100.0) },
                if hp <= 1 { " (A LETHAL FALL for the game: its no-death switch, held by us, kept the body alive)" } else { "" },
            );
            self.hp_written = Some(hp);
            return;
        }
        let params = self.targets.params;
        // (IMPL_ER48) a STATUS TICK (poison, scarlet rot running on the body: status.rs) is taken without a hit reaction, at
        // what the tick would have been at his own maximum
        let class = crate::status::loss(lost, max, player.chr_ins.modules.data.max_hp, control);
        // (IMPL_ER50, the user: "bleed or frostbite should care about Sora's maximum HP % not Elden Ring's") a PROC: its
        // percent of the maximum is taken of HIS maximum, the rest by the share rule; a hit (it comes with the attack)
        use crate::status_rules::{self as sr, Loss};
        let (tick, taken) = match class { Loss::Tick { own_loss, .. } => (true, own_loss), Loss::Proc { pct, flat, .. } => (false, sr::proc_own_loss(pct, flat, max)), Loss::Hit => (false, lost) };
        // WHO: the handle the game left in `last_hit_by`, or (GF-SENSOR R4) the attack sensor's reading where that names
        // nobody (telegraph.rs).
        // (THE IN-GAME TESTER 2026-11-03, run 2: the test aids put the hidden body 16.7 m lower, the game took 1325 of its HP
        // as a fall with nobody named, and "the nearest hostile", an idle soldier, was blamed for a Blow on Sora; a 21 m
        // put-down later knocked the friend leader out) A GUESS NEVER NAMES AN ENEMY (guards.rs `blame`): with nobody named,
        // the body on (or just down from) a fall of its own is the game's fall damage, KH3 has none: discarded whether the
        // leader fell or not; else a hostile within 8 m in (or just out of) an attack animation is the evidence of a bullet or
        // a bite; else it is the environment's (a fire floor, a trap): his damage with no causer.
        let read = if tick { None } else { crate::telegraph::hit_read(self.lead_pos(), crate::game::v3(&player.chr_ins.modules.physics.position)) };
        let by_handle = if tick { None } else { self.targets.id_of(&player.chr_ins.last_hit_by).or(read.map(|r| r.attacker)).and_then(|id| self.targets.get(id)).cloned() };
        // (GRABS) a blow inside an enemy's grab names nobody (the user's log 2026-11-06: "NOBODY NAMED ... THE ENVIRONMENT"):
        // it is the thrower's (the throw node's other party, named by `grab_kh`)
        let by_handle = by_handle.or_else(|| if self.grab.on && !tick { self.grabber.as_ref().and_then(|(bits, ..)| self.targets.list.iter().find(|e| game::handle_bits(&e.handle) == *bits)).cloned() } else { None });
        let attacking = if tick || by_handle.is_some() { None } else { self.targets.attacking_near(self.lead_pos(), 800.0).cloned() };
        let blame = crate::guards::blame(by_handle.is_some(), tick, control.flag("fall.guard") != Some(false) && self.body_fall.in_window(), attacking.is_some());
        if blame == crate::guards::Blame::Fall {
            self.fall.discarded += lost as i64;
            erlog!(
                "fall: the game took {lost} HP off its body ({hp} left of {}) with nobody named (last_hit_by {:#x}) while THE BODY ITSELF came down {:.1} m (a fall of its own or a warp that put it lower; the leader {}): THE GAME'S FALL DAMAGE, discarded: no enemy is blamed, the leader takes none{}",
                player.chr_ins.modules.data.max_hp,
                crate::game::handle_bits(&player.chr_ins.last_hit_by),
                self.body_fall.depth,
                if lead_floor { "stands" } else { "is off the floor" },
                if hp <= 1 { " (A LETHAL FALL for the game: its no-death switch, held by us, kept the body alive)" } else { "" },
            );
            self.hp_written = Some(hp);
            return;
        }
        // (DON-MAGIC glacier) GLACIER'S WALLS: an Elden Ring bullet cannot be stopped in flight safely, so a hit (not a status tick)
        // whose attacker's line to the leader passes through Donald's ice wall (the attacker unknown, as a bullet often is: every
        // hostile's line within 60 m) is taken by the ice: no hit, the body topped up as always (the fall guard's route).
        // UNTESTED IN GAME.
        if !matches!(class, Loss::Tick { .. }) {
            let by = self.targets.id_of(&player.chr_ins.last_hit_by).and_then(|id| self.targets.get(id)).map(|e| e.pos);
            let hostiles: Vec<Vec3> = self.targets.hostiles_by_distance().into_iter().filter(|(d, _)| *d <= 6000.0).map(|(_, e)| e.pos).collect();
            if crate::donstatus::wall_shields(self.lead_pos(), by, &hostiles) {
                erlog!("hit: Donald's Glacier wall stands between the attacker and the leader: the ice takes the hit ({lost} HP), no damage");
                self.hp_written = Some(hp);
                return;
            }
        }
        let is_proc = matches!(class, Loss::Proc { .. });
        // (ER party S2) a share of the LEADER'S maximum (Sora's own as before)
        let friend = self.friend_lead();
        let lead_max = friend.map_or(self.sora.status().max_hp(), |f| f.share_max.round().max(1.0) as i32); // (GF-STATS) a friend leader: the sheet's reference x his Defense factor (balance.rs)
        // (DF) a status tick without the difficulty factor (max-HP-proportional), a hit with it
        let amount = match class { Loss::Proc { pct, flat, .. } => sr::proc_amount(&params, pct, flat, max, lead_max), Loss::Tick { .. } => params.damage_in_tick(taken, max, lead_max), _ => params.damage_in(taken, max, lead_max) };
        // (the attacker as decided above, by `blame`)
        let guessed = by_handle.is_none();
        let attacker = by_handle.or(attacking);
        let (causer, from, yaw, radius) = match &attacker {
            Some(e) => (e.id, e.pos, e.yaw, e.radius),
            None => (TargetId::NONE, self.lead_pos() + coords::dir_er_to_kh(coords::facing_er(self.lead_yaw())) * 1.0, self.lead_yaw() + 180.0, 40.0), // (ER party S2)
        };
        let direction = ((self.lead_pos() - from) * Vec3::new(1.0, 1.0, 0.0)).normalize_or_zero(); // (ER party S2) the leader's
        // (IMPL_ER48, the user: "status effects like poison should not make Sora flinch, burn should though") a status tick
        // has no reaction; any other loss below the minimum share (burning) is a small hit (`status.burn_flinch = 0`: as before)
        let kind = if tick { None } else { params.kind_in(taken, max).or_else(|| (is_proc || control.flag("status.burn_flinch") != Some(false)).then_some(kh_core::enums::ETresDamageKind::Small)) };
        // (GRABS) INSIDE AN ENEMY'S GRAB the blows land on HP WITHOUT A REACTION: KH3's caught state holds the pawn (the
        // user's log: each blow of the throw threw Sora into a Blow reaction, Damage/4, while he was seated on the body)
        let kind = if self.grab.on && control.flag("grab.kh") != Some(false) { None } else { kind };
        // (DON-MAGIC) Sanctuary's short invincibility and Aero Barrier's cut on Sora (a friend leader's and the allies' are
        // kh-party's Party::receive_hit). (A Blind enemy's hit that reached him lands: Blind moves its aim, donstatus.rs.)
        if friend.is_none() && crate::donstatus::sora_invincible() {
            erlog!("hit: Sora is invincible (Donald's Sanctuary): no damage");
            self.hp_written = Some(hp);
            return;
        }
        let amount = if friend.is_none() { amount * crate::donstatus::sora_damage_mult() } else { amount };
        // (GF-ARTS, R14) Goofy's Interpose absorbs a hit on Sora: the body is topped up, he takes nothing
        #[cfg(feature = "party")]
        if friend.is_none() {
            let at = self.lead_pos();
            let attacker = (causer != TargetId::NONE).then_some(causer);
            if self.party.runtime_on().is_some_and(|rt| rt.party.cover_sora(at, amount, from, attacker)) {
                erlog!("hit: Goofy's Interpose took it for Sora: no damage");
                self.hp_written = Some(hp);
                return;
            }
        }
        // (damage over time: reaction type 1 = the HP rule without a reaction)
        let hit = HitOnPlayer { direction, causer_yaw: yaw, causer_radius: radius, reaction_type: if kind.is_some() { 2 } else { 1 }, ..HitOnPlayer::new(kind.unwrap_or(kh_core::enums::ETresDamageKind::Small), amount, causer, from) };
        let mut guarded = false; // (ER party I) the friend leader's kit guarded / dodged it
        match friend {
            // (ER party S2) a friend leading: the same hit as kh-party's record (his own reaction rules; his defender's cap)
            // (ER party I, X3G's request) applied AT ONCE, so the body code knows what became of it: a hit his kit guards
            // (Goofy's shield / Perfect Guard, Donald's Magic Block) or dodges costs him nothing and plays his guard, not a
            // hit reaction (kh-party); the body is topped up as always
            Some(f) => {
                let mut record = crate::guards::friend_hit(&f, kind, amount, causer, from, yaw, radius);
                record.attack_info = read.map_or(kh_core::host::AttackInfo { attacker: (causer != TargetId::NONE).then_some(causer), ..kh_core::host::AttackInfo::NONE }, |r| r.info); // (GF-SENSOR)
                #[cfg(feature = "party")]
                let result = self.party.runtime_on().and_then(|rt| crate::party::leader::LeaderBody::hit_now(rt, record));
                #[cfg(not(feature = "party"))]
                let result: Option<kh_core::host::HitResult> = None;
                match result {
                    Some(r) => guarded = r.guarded || !r.accepted,
                    None => self.lead_order(crate::guards::LeadOrder::Hit(Box::new(record))),
                }
            }
            None => self.sora.take_hit(hit),
        }
        if let Some(e) = &attacker {
            self.targets.note_attack(e.id);
        }
        self.hits_in += 1;
        erlog!(
            "hit: Elden Ring took {lost} of {max} HP off its player ({}) -> {} is hit for {amount:.1} ({}){}",
            match &attacker {
                Some(e) if guessed => format!("last_hit_by {:#x} is no character we know: target {} c{:04} is in its attack animation {:.1} m away (the evidence of a bullet or a bite)", crate::game::handle_bits(&player.chr_ins.last_hit_by), e.id.0, e.npc_id, (e.pos - self.lead_pos()).length() / 100.0),
                Some(e) => format!("last hit by target {} c{:04}", e.id.0, e.npc_id),
                None if tick => "a status tick, no attacker".to_string(),
                None => format!("NOBODY NAMED (last_hit_by {:#x}) and no hostile attacking within 8 m: THE ENVIRONMENT or the body's own state ({blame:?}), no enemy blamed", crate::game::handle_bits(&player.chr_ins.last_hit_by)),
            },
            if friend.is_some() { "THE LEADER (a friend)" } else { "Sora" }, // (ER party S2)
            match kind { Some(k) => format!("{k:?}"), None if tick => "a STATUS TICK, no reaction".to_string(), None => "damage over time, no reaction".to_string() },
            if guarded { "; HIS GUARD TOOK IT (ER party I): 0 HP, no hit reaction" } else if hp == 0 { "; THE TARNISHED IS AT 0: Elden Ring kills its player whatever Sora's rules say" } else if lost >= max { "; a blow of his whole own HP: the raised maximum kept the body alive, Sora's rules decide" } else { "" },
        );
    }

    /// After the steps: the Tarnished's HP (topped up, or 0 when Sora died), his invulnerability,
    /// Elden Ring's lock-on on Sora's locked target, the camera reset.
    /// (DTH) THE LEADER DOWN, WHOEVER LEADS (death.rs): Sora's own death event or his HP when he leads, the friend leader's
    /// KO when one leads (AI Sora's KO is no death of the leader's); latched once per host with its kind (a scripted death's
    /// place: sheet rows `death.scripted.*`).
    fn death_observe(&mut self) {
        let who = self.lead_chara();
        let alive = crate::death::leader_alive(self.friend_lead().map(|f| f.alive), self.sora.status().hp, self.sora_fell);
        self.sora_fell = false;
        // (KILLPLANE) a leader killed by a barrier dies the party's normal death, never the scripted 1-HP spare
        if let Some((who, kind)) = self.flow.observe(who, alive, crate::hud::scripted_death() && !self.barrier.leader_killed) {
            self.died = true;
            match kind {
                crate::death::Kind::Scripted => erlog!(
                    "death: THE LEADER {} IS DOWN AT A SCRIPTED DEATH'S PLACE: his own KO for {:.1} s, a fade to black ({:.1} s), then the body's HP {} with the game's no-death switch held: the game's own scene follows (no death of the game's: no death sound, no YOU DIED)",
                    who.name(), self.flow.rules.ko_show, self.flow.rules.fade, self.flow.rules.hp
                ),
                crate::death::Kind::Normal => erlog!("death: THE LEADER {} IS DOWN: KH3's game over (gameover.kh), else Elden Ring's own death and respawn", who.name()),
            }
        }
    }

    fn after_steps(&mut self, player: &mut PlayerIns, control: &Control, r3: bool, masked: bool) {
        // (DTH) the death flow's order for the body of this frame (the leader's death was observed before the HUD)
        let order = self.flow.live_step(self.frame_dt, crate::hud::gameover_holds());
        crate::death::plate(order.fade);
        // (prize) the leader's death is handed to the game in this frame: with the mirror its counter is emptied first
        if order.body == crate::death::Body::Kill && !self.death_sent {
            self.prize.dead_frame(control, player, &mut self.sora, 0.0);
        }
        // ---- HP: Elden Ring's death follows Sora's
        {
            // (ER party S2) the leader's (Sora's as before)
            let (sora_hp, sora_max) = self.friend_lead().map_or((self.sora.status().hp, self.sora.status().max_hp()), |f| (f.hp, f.max_hp));
            let data = player.chr_ins.modules.data.as_mut();
            // THE ONE-SHOT RULE: the body's BASE HP is held at `damage.in.body_hp` times his own (the
            // game computes the maximum from it every frame: a write of the maximum itself is undone
            // within the frame, measured), and he is kept full: a blow worth his whole own HP no
            // longer kills the hidden body. Shares of HP are taken of his OWN maximum.
            if !self.death_sent && data.max_hp > 0 {
                let k = if control.on("dmg.mirror") { 1.0 } else { self.targets.params.body_hp.max(1.0) };
                match self.body.as_mut() {
                    None => self.body = Some(Body { own_base: data.base_hp, own_max: data.max_hp, written_base: data.base_hp, settle: 4, told: 0 }),
                    Some(b) if data.base_hp != b.written_base => {
                        erlog!("death: the game set the Tarnished's base HP to {} (ours was {}, his own {}): taken as his own from now", data.base_hp, b.written_base, b.own_base);
                        *b = Body { own_base: data.base_hp, own_max: data.max_hp, written_base: data.base_hp, settle: 4, told: 0 };
                    }
                    Some(b) if b.settle > 0 => {
                        b.settle -= 1;
                        if b.settle == 0 {
                            b.own_max = data.max_hp;
                        }
                    }
                    Some(b) => {
                        let want = ((b.own_base as f32 * k).round() as i32).clamp(b.own_base, 9_999_999);
                        if data.base_hp != want {
                            data.base_hp = want;
                            b.written_base = want;
                        }
                        if b.told != data.max_hp && data.max_hp != b.own_max {
                            erlog!("death: the Tarnished's base HP is held at {want} ({k} x his own {}): the game makes his maximum {} (his own {}): no single blow kills the hidden body, Sora's HP decides", b.own_base, data.max_hp, b.own_max);
                            b.told = data.max_hp;
                        }
                    }
                }
            }
            // (DTH) the flow's order: HP 0 (a normal death; with `gameover.kh` only once the KH3 game-over menu is answered),
            // or THE SCRIPTED EVENT'S HP (1) with the no-death switch held: the game's own scene, no death of the game's
            let who = self.flow.down().map_or("Sora", |w| w.name());
            if order.body == crate::death::Body::Kill && !self.death_sent {
                data.hp = 0;
                self.death_sent = true;
                erlog!("death: the Tarnished's HP set to 0 (the leader {who} is down, his HP {sora_hp})");
            } else if let (crate::death::Body::Spare(hp), false) = (order.body, self.death_sent) {
                if data.hp > 0 {
                    data.hp = hp; // (a body the game had killed already is left dead: its own death goes on)
                }
                self.death_sent = true;
                erlog!("death: THE SCRIPTED DEATH IS HANDED TO THE GAME under black: the Tarnished's HP set to {} (its event waits for HP {hp} with the boss alive), the no-death switch held: no death of the game's; its own scene follows", data.hp);
            } else if !self.death_sent && data.hp > 0 {
                // TOPPED UP by default (a guarded or dodged blow must never kill the hidden body);
                // `dmg.mirror = 1` shows Sora's share in Elden Ring's own HP bar instead.
                let want = if control.on("dmg.mirror") { ermap::mirror_hp(sora_hp, sora_max, data.max_hp) } else { data.max_hp };
                if data.hp != want && want > 0 {
                    data.hp = want;
                }
                self.hp_written = Some(data.hp);
            } else if !self.death_sent {
                self.hp_written = Some(data.hp);
            }
        }
        // ---- his dodge frames (reaction types 6 - 9 and 11: invincible / ghost) are Elden Ring's no-damage frames
        let reaction = self.sora.actions().reaction;
        // (ER party S2) a friend leading: his own refusal of hits (Die / Warp / a team attack's FR state)
        let dodge = self.friend_lead().map_or(matches!(reaction, 6 | 7 | 8 | 9 | 11), |f| f.invulnerable);
        // (ER party S7) Sora leading and held by a team attack (one of his FR states; KH3's team attacks are invincible):
        // Elden Ring's no-damage frames too (`party.team_invuln = 0`: off)
        #[cfg(feature = "party")]
        let dodge = dodge || (self.friend_lead().is_none() && self.party.core().is_some() && control.flag("party.team_invuln") != Some(false) && crate::party::team::sora_in_team_attack(self.out.state));
        self.invulnerable = dodge || control.on("dmg.invuln") || crate::hud::gameover_holds() /* (hud) */ || order.body == crate::death::Body::Hold /* (DTH) the leader down, nothing handed over yet */;
        if self.invulnerable {
            self.invulnerable_frames += 1;
        }
        self.assert_flags(player, control);

        // ---- Elden Ring's own lock-on follows Sora's
        let lock = self.sora.combat().lockon;
        // (IMPL_ER23A, the user: "Elden Ring's lock on icon is still showing when locked onto an enemy") THE MIRROR IS ONLY FOR
        // ELDEN RING'S OWN CAMERA: nothing else of ours reads the game's lock (the gauge and the marker read Sora's target, the
        // hidden body is turned by our writes). While KH3's camera has the view the game's lock-on is left alone, so its
        // reticle is never made (it was drawn where the game's own unused camera projects the enemy: beside it). `lock.mirror
        // = 1`: mirrored always, as before; `lock.mirror = 0`: never.
        let mirror = match control.str("lock.mirror").map(str::trim) { Some("1") => true, Some("0") => false, _ => !self.cam.has_view() };
        let wanted = (mirror && lock.locked && lock.current != TargetId::NONE && control.flag("cam.lock") != Some(false)).then_some(lock.current);
        match wanted.and_then(|id| self.targets.get(id).map(|e| (id, e.handle, e.npc_id))) {
            Some((id, handle, npc)) => {
                if targets::er_lock_on(&handle) {
                    if self.er_locked != Some(id) {
                        erlog!("lock: Elden Ring's lock-on put on Sora's target {} (c{npc:04})", id.0);
                    }
                    self.er_locked = Some(id);
                    self.lock_missing = None;
                } else {
                    if self.er_locked.take().is_some() {
                        targets::er_unlock();
                    }
                    if self.lock_missing != Some(id) {
                        erlog!("lock: Sora's target {} (c{npc:04}) is not among Elden Ring's own lock-on candidates: its camera does not follow this one", id.0);
                        self.lock_missing = Some(id);
                    }
                }
            }
            None => {
                self.lock_missing = None;
                if self.er_locked.take().is_some() {
                    targets::er_unlock();
                    erlog!("lock: Elden Ring's lock-on released");
                }
            }
        }
        // R3 is hidden from Elden Ring: without a lock it is its camera reset, asked for here.
        if r3 && !self.r3_before && masked && wanted.is_none() && !self.cam.has_view() {
            use fromsoftware_shared::FromStatic;
            // SAFETY: game thread; the pointer is checked.
            if let Ok(wcm) = unsafe { eldenring::cs::WorldChrMan::instance_mut() } {
                if let Some(mut cam) = crate::game::chr_cam(wcm) /* (AGGRO) at its place for the running version (2.7.2.0 moved it) */ {
                    {
                        // SAFETY: the game's follow camera object, alive with the world.
                        unsafe { cam.as_mut().request_camera_reset = true };
                    }
                }
            }
        }
        self.r3_before = r3;
    }

    /// The game's own no-damage switches on the passive body, while Sora cannot be hit (written
    /// in both phases: the game may reset them). `dmg.iflag = hit | inv` picks one of the two.
    fn assert_flags(&mut self, player: &mut PlayerIns, control: &Control) {
        self.fall_switches(player, control);
        let which = control.str("dmg.iflag").unwrap_or("both");
        let chr = &mut player.chr_ins;
        if which != "inv" && chr.debug_flags.disabled_hit() != self.invulnerable {
            chr.debug_flags.set_disabled_hit(self.invulnerable);
        }
        if which != "hit" && chr.chr_flags1c5.is_invincible() != self.invulnerable {
            chr.chr_flags1c5.set_is_invincible(self.invulnerable);
        }
        // (THE USER 2026-11-02: the guard sound INSTEAD of the hit's) the game's own sounds of the body held off while the
        // leader's guard takes a blow (guardblow.rs; `guard_sound.quiet = 0`: left alone)
        let guard_up = self.friend_lead().is_none() && self.sora.actions().guard_parts;
        crate::guardblow::hold_body_sounds(chr, self.lead_chara(), guard_up, control.flag("guard_sound.quiet") != Some(false));
    }

    /// (IMPL_ER23A) THE HIDDEN BODY NEVER DIES OF A FALL THAT SORA SURVIVES. MEASURED in the user's log: the body died twice
    /// with its fall timer held at 0 and 5220 HP: the game measures the HEIGHT fallen and kills at the body's landing (17002).
    /// The height's field is not known, so the death itself is refused: WHILE SORA IS IN CONTROL AND ALIVE the game's own
    /// "player cannot die" switch is held (SDK `WorldChrManDbgFlags::player_no_dead`: HP 1 instead of death; the same block's
    /// `all_no_attack` is the lab's "calm" and works), and the material module's "no fall damage here" is written every phase.
    /// What the game then takes off the body is read as always (`damage_in`): inside the fall window it is discarded,
    /// outside it it is a blow of the game's on Sora (a lethal one = a blow of his whole HP: his rules decide). THE RULE:
    /// - a landing never hurts or kills, whatever the height;
    /// - a pit without ground is Sora's own net (no ground at all under him: put back, the second time his death);
    /// - every other death of the game's (drowning, an instant kill, a kill volume he stands in) reaches him as a blow of
    ///   his whole HP. The switch is let go the moment Sora dies (his death is written as the body's HP 0), in cutscenes,
    ///   and when Sora stops. `fall.guard = 0`: nothing of this.
    fn fall_switches(&mut self, player: &mut PlayerIns, control: &Control) {
        use fromsoftware_shared::FromStatic;
        let want = control.flag("fall.guard") != Some(false) && !self.died && !self.death_sent && self.lead_alive() /* (ER party S2) */ && self.follow != Some(Follow::Cutscene);
        // (DTH) a scripted death holds it from the leader's KO until the game's scene: the body stays at the event's HP
        let want = want || self.flow.holds_no_dead();
        // (DEATHPLANE, in game 2026-11-03: Sora died of the sea's kill plane, the switch was let go with his death and the game's
        // own kill plane killed the body 0.4 s later, under KH3's game over: Elden Ring's death and a reload instead of the
        // menu) THE LEADER IS DOWN AND HIS DEATH NOT HANDED OVER YET: the body is ours until the menu is answered (Body::Hold)
        let want = want || (control.flag("fall.guard") != Some(false) && self.flow.down().is_some() && !self.death_sent);
        // SAFETY: game thread; the SDK resolves the static by its pinned RVA (the exe-version gate holds).
        if let Ok(flags) = unsafe { eldenring::cs::WorldChrManDbgFlags::instance_mut() } {
            if want {
                if self.no_dead_own.is_none() {
                    self.no_dead_own = Some(flags.player_no_dead);
                    erlog!("fall: the game's own no-death switch for its player is HELD while Sora is in control (it was {}): a fall cannot kill the hidden body; Sora's HP decides his death", flags.player_no_dead);
                }
                if !flags.player_no_dead {
                    flags.player_no_dead = true;
                }
                self.no_dead_held = true;
                NO_DEAD_HELD.store(true, std::sync::atomic::Ordering::Relaxed);
            } else if std::mem::take(&mut self.no_dead_held) {
                flags.player_no_dead = false;
                NO_DEAD_HELD.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }
        if want && game::plausible(crate::word!(player.chr_ins.modules.material)) && !player.chr_ins.modules.material.disable_fall_damage {
            player.chr_ins.modules.material.disable_fall_damage = true;
        }
    }

    /// (IMPL_ER23A) The body plays one of the game's death animations although it lives (the no-death switch kept it): said once.
    fn body_watch(&mut self, player: &PlayerIns, control: &Control, dt: f32) {
        let ta = &player.chr_ins.modules.time_act;
        let anim = ta.anim_queue[(ta.read_idx as usize) % 10].anim_id;
        let dying = (17000..19000).contains(&anim) && player.chr_ins.modules.data.hp > 0 && self.no_dead_held && control.flag("fall.guard") != Some(false);
        let before = self.death_anim_for;
        self.death_anim_for = if dying { before + dt } else { 0.0 };
        if before < 1.0 && self.death_anim_for >= 1.0 {
            erlog!("!!! fall: THE BODY PLAYS THE GAME'S DEATH ANIMATION {anim} BUT LIVES (HP {}): the no-death switch refused a death of the game's; if the game does not get up by itself, fall.guard = 0 gives its deaths back", player.chr_ins.modules.data.hp);
        }
    }

    /// (IMPL_ER57, the user: "Sora doesn't get grabbed properly by enemy grab attacks") AN ENEMY'S GRAB. CAUSE: a grab of the
    /// game's lands on the hidden body (it stands where Sora stands) and the game puts the body into its throw: the body's
    /// throw node goes to "the thrown one" and the attacker's paired animation moves and animates it; but Sora went on
    /// driving the body, so every frame it was written back to where he stood: the enemy grabbed air and its blow at the end
    /// reached Sora as a plain hit. NOW: while the body's throw node (SDK `CSChrThrowModule::throw_node`, `throw_state`) is in
    /// a grabbed state (`grab.states`, default 4 InThrowTarget, 6 DeathTarget: the SDK's names) SORA FOLLOWS THE BODY
    /// (`Follow::Grab`: the game moves it, its switches are a cutscene's; he is seated on it and shows its pose retargeted,
    /// as on a ladder; no input, the pad kept from both; no net, no stuck rule); when the node leaves those states he gets
    /// the body back where the game left it (after the wait for the ground). At most `grab.max` seconds (15) in one piece.
    /// `grab.follow = 0`: as before. Every change of the node's state is logged (`grab:`; `log.grab = 0` silences it).
    fn grab_phase(&mut self, player: &PlayerIns, control: &Control, cutscene: bool, dt: f32) -> bool {
        let (state, by) = throw_state(player);
        let ta = &player.chr_ins.modules.time_act;
        let anim = ta.anim_queue[(ta.read_idx as usize) % 10].anim_id;
        let attacker = || match by.and_then(|bits| self.targets.list.iter().find(|e| game::handle_bits(&e.handle) == bits)) {
            Some(e) => format!("target {} c{:04} (animation {})", e.id.0, e.npc_id, e.anim),
            None => format!("handle {:#x} (no character of the target list)", by.unwrap_or(0)),
        };
        if state != self.throw_seen && control.flag("log.grab") != Some(false) {
            erlog!("grab: the body's throw node: state {:?} -> {:?} (body animation {anim}, the other party {}; Sora {:?}, HP {})", self.throw_seen, state, attacker(), self.out.state, self.sora.status().hp);
        }
        self.throw_seen = state;
        let states: Vec<u32> = match control.str("grab.states") {
            Some(list) => list.split(',').filter_map(|s| s.trim().parse().ok()).collect(),
            None => crate::guards::GRAB_STATES.to_vec(),
        };
        let wanted = control.flag("grab.follow") != Some(false) && !cutscene && self.lead_alive() /* (ER party S2) */ && !self.death_sent && state.is_some_and(|s| states.contains(&s));
        let max = control.f32("grab.max").filter(|v| *v > 0.5).unwrap_or(15.0);
        match self.grab.frame(wanted, max, dt) {
            crate::guards::GrabEdge::On => erlog!(
                "grab: AN ENEMY GRABS THE BODY (throw state {}, by {}, body animation {anim}): Sora FOLLOWS the body through the throw (seated on it, its pose retargeted onto him, no input, no net / stuck rule); the blow it ends with reaches him through the body's HP as any hit (grab no. {})",
                state.unwrap_or(0), attacker(), self.grab.count
            ),
            crate::guards::GrabEdge::Off => erlog!(
                "grab: THE GRAB IS OVER after {:.1} s (throw state now {:?}, body animation {anim}, Sora HP {}): the body is handed back to Sora where the game left it (he waits for ground under him)",
                self.grab.seconds, state, self.sora.status().hp
            ),
            crate::guards::GrabEdge::GaveUp => erlog!("!!! grab: the throw node has said \"grabbed\" (state {:?}) for more than {max:.0} s: TAKEN AS STALE, Sora has the body again (grab.max = s; grab.states = 4,6)", state),
            crate::guards::GrabEdge::None => {}
        }
        self.grab_kh(control, by, dt); // (GRABS)
        self.grab.on
    }

    /// (GRABS) THE AI FRIENDS A THROW HOLDS (party/allybody_live.rs leaves their bodies to the throw): each is seated on his
    /// body as the game moves it (Donald / Goofy: his pawn put there, his movement stopped; AI Sora while a friend leads:
    /// put there when he is more than 30 cm off). The party's caught gate (kh-partyhost caught.rs) gets every AI-driven
    /// character's word each frame: no action while caught or getting up (his tree stopped, a spell cancelled), KH3's caught
    /// motion on a friend's pawn (CatchStart / CatchLoop / CatchEnd), AI Sora's on his core (`step_pose`), and the guards
    /// against a stale hold. The same rule as the leader's: the throw node's, whoever is held.
    #[cfg(feature = "party")]
    fn seat_caught_allies(&mut self) {
        use kh_host::party::PartyChara;
        let leader = if self.friend_lead().is_some() { self.lead.chara() } else { PartyChara::Sora };
        // the party's caught gate (kh-partyhost caught.rs): the word for every AI-driven character, every frame (no action while
        // caught or getting up, KH3's caught motion on a friend's pawn, the stale-hold guards)
        if let Some(core) = self.party.core_mut() {
            for c in [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy] {
                core.rt.set_caught(c, c != leader && crate::grabs::ally_held(c));
            }
        }
        for a in crate::grabs::allies() {
            if a.chara == leader {
                continue;
            }
            if a.chara == PartyChara::Sora {
                if self.sora.status().hp > 0 && (self.sora.output().position - a.feet).length() > 30.0 {
                    self.sora.teleport(Pose { position: a.feet, yaw: a.yaw });
                }
                continue;
            }
            if let Some(core) = self.party.core_mut() {
                if let Some(m) = core.rt.id(a.chara).and_then(|id| core.rt.party.member_mut(id)) {
                    m.cmc.location = a.feet;
                    m.cmc.stop_movement();
                    m.yaw = a.yaw;
                }
            }
        }
    }

    /// (GRABS) THE GRAB, KH3'S WAY (grabs.rs), once a frame after `grab_phase`'s rule: the thrower is named (its MSB event
    /// entity and map, from the throw node's other party); the caught sequence is stepped (KH3's `CatchStart`, `CatchLoop`
    /// while the throw holds the body, `CatchEnd` at the release: Sora's here, a friend leader's by partydraw.rs from
    /// `grabs::leader_held`); and the ONE Raya Lucaria Abductor Virgin's grab ends in the game's own warp to Volcano Manor.
    /// `grab.kh = 0`: the body's pose retargeted as before (IMPL_ER57); `grab.abduct = 0`: no warp.
    fn grab_kh(&mut self, control: &Control, by: Option<u64>, dt: f32) {
        let held = self.grab.on;
        let kh = control.flag("grab.kh") != Some(false);
        crate::grabs::set_leader_held(held && kh && self.friend_lead().is_some());
        if held && self.grabber.is_none() {
            let named = by.and_then(|bits| self.targets.list.iter().find(|e| game::handle_bits(&e.handle) == bits)).and_then(|e| crate::targets::find_chr(&e.handle).map(|chr| (game::handle_bits(&e.handle), chr.event_entity_id, crate::zphase::map_of(chr), e.npc_id)));
            if let Some((bits, entity, map, npc)) = named {
                self.abduct = if control.flag("grab.abduct") != Some(false) { crate::grabs::abduction(&map, entity) } else { None };
                erlog!(
                    "grab: (GRABS) the thrower is c{npc:04}, MSB entity {entity} in {map}{}",
                    if self.abduct.is_some() { ": THE RAYA LUCARIA ABDUCTOR VIRGIN: its grab takes the party to Volcano Manor (its blows cost nothing, no death; at the release the game's own warp to m16_00_00_00 point 16002080, flags 16000540 and 14009300)" } else { ": an ordinary grab (KH3's caught sequence, its blows through the party's damage path without a reaction)" }
                );
                self.grabber = Some((bits, entity, map));
            }
        }
        if crate::grabs::abduct_step(held) == crate::grabs::AbductStep::Warp {
            self.grabber = None;
            if let Some(a) = self.abduct.take() {
                abduct_warp(&a);
            }
        }
        // KH3's caught sequence on Sora: leading, or (a friend leading) AI Sora whose own body a throw holds
        let sora_held = kh && if self.friend_lead().is_none() { held } else { crate::grabs::ally_held(kh_host::party::PartyChara::Sora) };
        if sora_held && self.catch.is_none() {
            self.catch = Some(crate::grabs::Catch::begin());
            erlog!("grab: (GRABS) Sora is CAUGHT: KH3's TresPlayerState_Catch sequence (CatchStart, CatchLoop while held, CatchEnd at the release), seated on the body the throw moves");
        }
        if let Some(c) = self.catch.as_mut() {
            c.step(sora_held, dt, sora_catch_lengths());
            if c.done() {
                erlog!("grab: (GRABS) Sora is up again after CatchEnd (held {:.1} s): his own pose and input", c.held_for);
                self.catch = None;
            }
        }
    }

    /// (IMPL_ER57, the user: "Sora is not dying when falling into the ocean") THE GAME'S DEEP-WATER DEATH, FOR SORA. CAUSE
    /// (the user's log, 108 - 111 s): he jumped off the coast into the sea; his casts (layer 0x56, chosen because it does
    /// not see water: IMPL_ER3) went through the sea surface, he fell 110 m through the water, and the net, seeing no
    /// ground at all, PUT HIM BACK on the cliff. The game's own kill of its player under water (MEASURED in IMPL_ER3: about
    /// 0.6 s after the body went under) no longer happens since IMPL_ER23A holds the game's "player cannot die" switch
    /// while Sora is in control (the body's HP never moved in that log: the kill is refused before any HP is taken).
    /// NOW: every 0.1 s three casts straight down through his place (the water layers 0x58 / 0x5a and his solid layers,
    /// guards.rs `water_surfaces` / `depth_under`) give the water surface over his feet; `water.depth` cm (150: his head)
    /// or deeper for `water.time` s (0.6, the measured delay) is his death, the way the net's death is made (a blow of
    /// 1e6: KH3's death and its game-over menu). A wade is not deep enough; a cave under a lake has the lake's bed between.
    /// `water.death = 0`: as before. Log lines `water:`.
    /// (DEATHPLANE) RETIRED BY DEFAULT (control.rs DEFAULTS `water.death = 0`): the game's own barriers kill in the sea
    /// (deathplane.rs); the key brings this back for debugging.
    fn water_phase(&mut self, control: &Control, dt: f32) -> bool {
        if control.flag("water.death") == Some(false) || !self.lead_alive() /* (ER party S2) */ || self.died || self.death_sent {
            self.drown = Default::default();
            self.water_told = 0;
            return false;
        }
        self.water_check += dt;
        if self.water_check < 0.1 {
            return false;
        }
        let step = std::mem::replace(&mut self.water_check, 0.0).min(0.5);
        let feet = self.feet_er();
        let solid_filters = [self.world.filter, self.world.floor_filter.get(), WATER_SOLID_OLD[0], WATER_SOLID_OLD[1]]; // (WALLS) the old solid layers too
        let water_a = flat_heights(feet + Vec3::Y * 50.0, 51.0, crate::havok::FILTER_CAMERA, true);
        // (nothing of the water layers above his feet: no water over him; the other casts are spared)
        let depth = if water_a.iter().any(|h| *h > feet.y) {
            let water_b = flat_heights(feet + Vec3::Y * 50.0, 51.0, WATER_LAYER_B, true);
            let solid: Vec<f32> = solid_filters.iter().filter(|f| **f != 0).flat_map(|f| flat_heights(feet + Vec3::Y * 50.0, 51.0, *f, false)).collect();
            let surfaces = crate::guards::water_surfaces(&water_a, &water_b, &solid);
            crate::guards::depth_under(feet.y, &surfaces, &solid).map(|m| m * coords::CM_PER_M)
        } else {
            None
        };
        let min = control.f32("water.depth").filter(|v| v.is_finite() && *v > 0.0).unwrap_or(150.0);
        let min = self.friend_lead().map_or(min, |f| crate::guards::water_depth_for(min, f.capsule)); // (ER party S2) at the leader's height
        let need = control.f32("water.time").filter(|v| v.is_finite() && *v >= 0.1).unwrap_or(crate::guards::Drown::NEED);
        let told = match depth { Some(d) if d >= min => 2, Some(d) if d >= 30.0 => 1, _ => 0 };
        if told != self.water_told {
            let p = self.lead_pos(); // (ER party S2)
            erlog!("water: {} at kh ({:.0}, {:.0}, {:.0}) (Sora {:?}, on the floor {})", match (told, depth) {
                (2, Some(d)) => format!("HE IS UNDER WATER: the surface is {d:.0} cm above his feet (water.depth {min:.0}): {need:.1} s of this is his death, as the game's own deep water kills its player"),
                (1, Some(d)) => format!("he wades: the water surface is {d:.0} cm above his feet (deadly from {min:.0} cm)"),
                _ => "out of the water".to_string(),
            }, p.x, p.y, p.z, self.out.state, self.out.on_floor);
            self.water_told = told;
        }
        if self.drown.frame(depth, min, need, step) {
            erlog!("sora: HE DROWNS: {need:.1} s with the water surface {:.0} cm or more above his feet (the deepest {:.0} cm): the game's deep water kills its player there: SORA'S DEATH (the game-over menu); water.death = 0: not", min, self.drown.deepest);
            self.net_last = None;
            self.void_for = 0.0;
            if self.friend_lead().is_some() {
                self.lead_order(crate::guards::LeadOrder::Die); // (ER party S2) the friend leader drowns: his KO (U10: the leader)
                return true;
            }
            let from = self.out.position + Vec3::Z * 100.0;
            self.invulnerable = false;
            self.sora.take_hit(HitOnPlayer { direction: Vec3::NEG_Z, ..HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 1.0e6, TargetId::NONE, from) });
            return true;
        }
        false
    }

    /// (IMPL_ER57) A WATER SURFACE BELOW HIM within 1 km (both water layers meet it, his solid layers do not): for the net,
    /// the sea under a cliff is not a void.
    fn water_below(&self) -> bool {
        let feet = self.feet_er();
        let water_a = flat_heights(feet, 1000.0, crate::havok::FILTER_CAMERA, true);
        if water_a.is_empty() {
            return false;
        }
        let water_b = flat_heights(feet, 1000.0, WATER_LAYER_B, true);
        let solid: Vec<f32> = [self.world.filter, self.world.floor_filter.get(), WATER_SOLID_OLD[0], WATER_SOLID_OLD[1]].iter() /* (WALLS) */.filter(|f| **f != 0).flat_map(|f| flat_heights(feet, 1000.0, *f, false)).collect();
        !crate::guards::water_surfaces(&water_a, &water_b, &solid).is_empty()
    }

    /// (DEATHPLANE, THE USER 2026-11-03: "KH characters should die from Elden Ring's death barriers (like the ocean)") ONCE A
    /// FRAME, at the end of the live pre-physics: the game's own contact query (deathplane.rs) at every KH character's place,
    /// the shared rule (`Deathplane::judge`), and each one's own death: the leader (Sora: the blow of his whole HP, KH3's
    /// knockdown, the game over; a friend leader: his KO), the AI friends (their KO), AI Sora (a friend leads) and Mimic Sora
    /// (the blow on his core). The no-death switch, the fall guard and the hit cap stand nowhere on this path: it never goes
    /// through the body's HP. `barrier.death = 0`: off.
    fn barrier_frame(&mut self, control: &Control, dt: f32, paused: bool) {
        use crate::deathplane::{Facts, Who};
        if paused {
            return;
        }
        if control.flag("barrier.death") == Some(false) || !crate::deathplane::ready() {
            self.barrier = Default::default();
            return;
        }
        let log = control.flag("log.barrier") != Some(false);
        // (test aids, test profile only; inert unless written, one-shots on a change) where the barriers are; a friend put
        if let Some(text) = crate::deathplane::aid_fired(&mut self.barrier.aids[0], control.str("test.barrier_probe")) {
            self.barrier_probe(&text);
        }
        if let Some(text) = crate::deathplane::aid_fired(&mut self.barrier.aids[1], control.str("test.barrier_push")) {
            self.barrier_push(&text);
        }
        let world_ok = self.follow != Some(Follow::Cutscene) && !self.death_sent;
        // ---- the leader (the body's leader: Sora, or the friend who leads)
        let (radius, top) = self.friend_lead().map_or((SORA_PARAMS.capsule_radius, crate::guards::SORA_TOP_CM), |f| (f.capsule.0, f.capsule.1 + f.capsule.2));
        let alive = self.lead_alive() && !self.died && !self.death_sent;
        let facts = Facts { feet_kh: self.lead_pos(), radius, top, alive, on_floor: self.lead_on_floor(), test: world_ok && self.follow.is_none() };
        if self.barrier_one(Who::Leader, facts, log, dt).is_some() {
            self.net_last = None;
            self.void_for = 0.0;
            self.barrier.leader_killed = true; // (KILLPLANE)
            if self.friend_lead().is_some() {
                // a friend leader: his KO (the party's Die state, past his guard); the game over is the death flow's
                #[cfg(feature = "party")]
                if let Some(rt) = self.party.runtime_on() {
                    if let Some(id) = rt.leader {
                        crate::deathplane::knock_out(&mut rt.party, id);
                    }
                }
            } else {
                self.invulnerable = false;
                let feet = self.out.position;
                crate::deathplane::kill_core(&mut self.sora, feet); // (KILLPLANE) instant, past every ability
            }
        }
        let judged = [vec![Who::Leader], self.barrier_allies(world_ok, log, dt)].concat();
        self.barrier.keep_only(&judged);
    }

    /// (DEATHPLANE) ONE CHARACTER: the game's query along his move of this frame (when `facts.test`), the shared rule, the
    /// log. Returns the death to give him now.
    fn barrier_one(&mut self, who: crate::deathplane::Who, facts: crate::deathplane::Facts, log: bool, dt: f32) -> Option<crate::deathplane::Barrier> {
        use crate::deathplane::Barrier;
        let feet = self.world.frame.pos_kh_to_er(facts.feet_kh);
        let found = if facts.test {
            let (a, b) = self.barrier.doom(who).sweep(feet);
            crate::deathplane::touches(a, b, facts.radius / coords::CM_PER_M, facts.top / coords::CM_PER_M)
        } else {
            self.barrier.doom(who).pause();
            Vec::new()
        };
        let hit: Vec<Barrier> = found.iter().filter_map(|t| t.barrier).collect();
        let (began, give) = self.barrier.judge(who, &hit, facts.alive, facts.on_floor, dt);
        if log {
            let name = who.name(self.lead_chara());
            for b in &began {
                let at = found.iter().find(|t| t.barrier == Some(*b)).map_or(feet, |t| t.at);
                erlog!("barrier: {name} TOUCHED {} of the game's at Elden Ring ({:.2}, {:.2}, {:.2}), his feet at y {:.2}: {}", b.name(), at.x, at.y, at.z, feet.y, match b {
                    Barrier::Kill => "HIS DEATH (given until he is down)",
                    Barrier::LethalFall => "his death at his landing (no fall damage: the game's lethal-fall zone)",
                    Barrier::FallCamera => "no death of its own; the void below is his death (no put-back)",
                });
            }
            if give == Some(Barrier::LethalFall) {
                erlog!("barrier: {name} LANDED after a lethal-fall zone: HIS DEATH");
            }
        }
        give
    }

    /// (DEATHPLANE) The AI friends, AI Sora (a friend leads) and Mimic Sora: the same query at each one's KH place, each
    /// one's own KO. Returns whom it judged.
    #[cfg(feature = "party")]
    fn barrier_allies(&mut self, world_ok: bool, log: bool, dt: f32) -> Vec<crate::deathplane::Who> {
        use crate::deathplane::{Facts, Who};
        use kh_host::party::PartyChara;
        let mut judged = Vec::new();
        if !world_ok {
            return judged;
        }
        let (sr, st) = (SORA_PARAMS.capsule_radius, crate::guards::SORA_TOP_CM);
        // AI Sora: the host's own Sora core while a friend leads
        if self.friend_lead().is_some() {
            judged.push(Who::Ally(PartyChara::Sora));
            let out = self.sora.output();
            let facts = Facts { feet_kh: out.position, radius: sr, top: st, alive: self.sora.status().hp > 0, on_floor: out.on_floor, test: true };
            if self.barrier_one(Who::Ally(PartyChara::Sora), facts, log, dt).is_some() {
                crate::deathplane::kill_core(&mut self.sora, out.position); // (KILLPLANE) instant, past every ability
            }
        }
        // Donald / Goofy (the friends who do not lead)
        let list: Vec<(PartyChara, kh_party::MemberId, Facts)> = match self.party.runtime_on() {
            Some(rt) => {
                let leader = rt.leader;
                rt.members
                    .iter()
                    .filter(|(c, id)| Some(*id) != leader && *c != PartyChara::Sora)
                    .filter_map(|(c, id)| rt.view(*c).map(|v| (*c, *id, Facts { feet_kh: v.position, radius: v.capsule.0, top: 2.0 * v.capsule.1, alive: !v.ko, on_floor: v.on_ground, test: true })))
                    .collect()
            }
            None => Vec::new(),
        };
        for (c, id, facts) in list {
            judged.push(Who::Ally(c));
            if self.barrier_one(Who::Ally(c), facts, log, dt).is_some() {
                let downed = self.party.runtime_on().is_some_and(|rt| crate::deathplane::knock_out(&mut rt.party, id));
                if downed && log {
                    erlog!("barrier: {} IS KNOCKED OUT by the game's death barrier (the party's KO)", c.name());
                }
            }
        }
        // Mimic Sora
        if let Some((feet_kh, on_floor, alive)) = crate::spirit_mimic::barrier_view() {
            judged.push(Who::Mimic);
            let facts = Facts { feet_kh, radius: sr, top: st, alive, on_floor, test: true };
            if self.barrier_one(Who::Mimic, facts, log, dt).is_some() && crate::spirit_mimic::barrier_down() && log {
                erlog!("barrier: Mimic Sora is given the barrier's blow (he leaves as Died when down)");
            }
        }
        judged
    }
    #[cfg(not(feature = "party"))]
    fn barrier_allies(&mut self, _world_ok: bool, _log: bool, _dt: f32) -> Vec<crate::deathplane::Who> {
        Vec::new()
    }

    /// (DEATHPLANE test aid `test.barrier_probe = metres`) The game's special collision around the leader, nearest first per
    /// kind, with the `sora.warp` that puts him 6 m above it (he falls in by himself).
    fn barrier_probe(&self, text: &str) {
        let feet = self.feet_er();
        let reach = text.trim().parse::<f32>().unwrap_or(40.0);
        let cols = crate::deathplane::probe(feet, reach);
        erlog!("barrier: test.barrier_probe {reach:.0} m around Elden Ring ({:.2}, {:.2}, {:.2}): {} column(s) meet the game's special collision", feet.x, feet.y, feet.z, cols.len());
        for kind in crate::deathplane::KIND_SPECIAL.0..=crate::deathplane::KIND_SPECIAL.1 {
            let near = cols.iter().flat_map(|(top, ts)| ts.iter().filter(|t| t.kind == kind).map(move |t| (*top, t.at))).min_by(|a, b| (a.1 - feet).length().total_cmp(&(b.1 - feet).length()));
            if let Some((top, at)) = near {
                let to = Vec3::new(top.x, at.y + 6.0, top.z) - feet;
                erlog!("barrier: probe: kind {kind}{} nearest at ({:.2}, {:.2}, {:.2}), {:.1} m away: `sora.warp = +{:.2},{:.2},{:.2}` puts him 6 m above it", crate::deathplane::Barrier::of_kind(kind).map_or(String::new(), |b| format!(" ({})", b.name())), at.x, at.y, at.z, (at - feet).length(), to.x, to.y, to.z);
            }
        }
    }

    /// (DEATHPLANE test aid `test.barrier_push = donald|goofy|sora,x,y,z` or `..,+dx,+dy,+dz`) An AI friend put at an Elden Ring
    /// place (relative: to the leader's feet), KH3's own fall from there.
    #[cfg(feature = "party")]
    fn barrier_push(&mut self, text: &str) {
        use kh_host::party::PartyChara;
        let Some((who, at)) = crate::deathplane::push_of(text, self.feet_er()) else {
            erlog!("barrier: test.barrier_push = {text}: give who,x,y,z or who,+dx,+dy,+dz (Elden Ring metres)");
            return;
        };
        let chara = match who.as_str() {
            "donald" => PartyChara::Donald,
            "goofy" => PartyChara::Goofy,
            "sora" => PartyChara::Sora,
            _ => {
                erlog!("barrier: test.barrier_push: {who} is not donald / goofy / sora");
                return;
            }
        };
        let kh = self.world.frame.pos_er_to_kh(at);
        if chara == PartyChara::Sora && self.friend_lead().is_some() {
            let yaw = self.sora.output().yaw;
            self.sora.teleport(Pose { position: kh, yaw });
            erlog!("barrier: test.barrier_push: AI Sora put at Elden Ring ({:.2}, {:.2}, {:.2})", at.x, at.y, at.z);
            return;
        }
        let leader = self.party.runtime_on().and_then(|rt| rt.leader);
        let done = self.party.runtime_on().and_then(|rt| rt.id(chara).filter(|id| Some(*id) != leader).and_then(|id| rt.party.member_mut(id))).map(|m| {
            m.cmc.location = kh;
            m.cmc.stop_movement();
        });
        erlog!("barrier: test.barrier_push: {} {}", chara.name(), if done.is_some() { format!("put at Elden Ring ({:.2}, {:.2}, {:.2}): he falls from there", at.x, at.y, at.z) } else { "is no AI friend in the party now: not put".to_string() });
    }
    #[cfg(not(feature = "party"))]
    fn barrier_push(&mut self, text: &str) {
        erlog!("barrier: test.barrier_push = {text}: no party in this build");
    }

    /// (IMPL_ER24) At most one report a second and 300 a session.
    fn report_due(&mut self) -> bool {
        if self.reports.0 >= 300 || self.reports.1.is_some_and(|t| t.elapsed().as_secs_f32() < 1.0) {
            return false;
        }
        self.reports = (self.reports.0 + 1, Some(std::time::Instant::now()));
        true
    }

    /// (IMPL_ER24) A PLACE HE MAY BE PUT AT: walkable ground within 60 cm under it (how far from its feet: the answer) and
    /// his body free there: no sweep of it (3 cm up, 2 cm to each side, down) starts in an overlap.
    fn place_ok(&self, position: Vec3) -> Option<f32> {
        use kh_core::World;
        let p = &SORA_PARAMS;
        // (ER party S2) the leader's capsule (Sora's as before)
        let (centre, (r, hh)) = match self.friend_lead() {
            Some(f) => (position + Vec3::Z * f.capsule.2, (f.capsule.0, f.capsule.1)),
            None => (position + (self.out.capsule_center - self.out.position), (p.capsule_radius, p.capsule_half_height)),
        };
        for way in [Vec3::Z * 3.0, Vec3::X * 2.0, Vec3::NEG_X * 2.0, Vec3::Y * 2.0, Vec3::NEG_Y * 2.0] {
            if self.world.sweep_capsule(centre, centre + way, r, hh).is_some_and(|h| h.start_penetrating) {
                return None;
            }
        }
        let hit = self.world.sweep_capsule(centre + Vec3::Z * 3.0, centre - Vec3::Z * 60.0, r, hh)?;
        (!hit.start_penetrating && hit.impact_normal.z >= p.floor_walkable_z).then(|| hit.position.z - (position.z - Sora::origin_height_on_floor(p)))
    }

    /// (IMPL_ER24) A free, supported spot beside him at about his feet's height (40 and 70 cm away, eight ways), reached
    /// through free air at chest height: where the stuck rule may put him when there is no ground under his centre.
    fn spot_beside(&self, p: Vec3) -> Option<Vec3> {
        use kh_core::World;
        let height = Sora::origin_height_on_floor(&SORA_PARAMS);
        for reach in [40.0, 70.0] {
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let q = p + Vec3::new(a.cos(), a.sin(), 0.0) * reach;
                if self.world.sweep_capsule(p + Vec3::Z * 90.0, q + Vec3::Z * 90.0, 15.0, 15.0).is_some() {
                    continue;
                }
                let Some(hit) = self.world.sweep_capsule(q + Vec3::Z * 60.0, q - Vec3::Z * 60.0, 15.0, 15.0) else { continue };
                if hit.start_penetrating || hit.impact_normal.z < SORA_PARAMS.floor_walkable_z || (hit.position.z - (p.z - height)).abs() > crate::guards::STUCK_NEAR {
                    continue;
                }
                let to = Vec3::new(q.x, q.y, hit.position.z + height);
                if self.place_ok(to).is_some() {
                    return Some(to);
                }
            }
        }
        None
    }

    /// (IMPL_ER23A) HE IS WEDGED (guards.rs `Stuck`: off the floor, falling speed, no movement for 0.4 s). What is under him is
    /// looked at (his own body's sweep, a line, a sphere of 15 cm under his centre) and written to the log.
    /// (IMPL_ER24, `stuck.safe = 0` = as it was) He is put ONLY at a place that is verified (`place_ok`: walkable ground under
    /// it, his body free there): onto the ground under his centre or a free spot beside him, both within 30 cm of his feet's
    /// height; back to where he last stood when that is within 1 m of his height, and from the third time at one place
    /// whatever its height (he is inside something: no small move frees him). Never down through what his body rests on:
    /// the ground under his centre is looked for no deeper than 30 cm. Every time: one line with both places.
    fn unstick(&mut self, player: &mut PlayerIns, safe: bool) {
        use crate::guards::Unstick;
        use kh_core::World;
        let p = self.out.position;
        let height = Sora::origin_height_on_floor(&SORA_PARAMS);
        let centre = self.out.capsule_center;
        let body = self.world.sweep_capsule(centre, centre - Vec3::Z * 30.0, SORA_PARAMS.capsule_radius, SORA_PARAMS.capsule_half_height);
        let under = self.world.sweep_capsule(p + Vec3::Z * 60.0, p - Vec3::Z * 60.0, 15.0, 15.0);
        let line = self.world.line_trace(p + Vec3::Z * 50.0, p - Vec3::Z * 60.0);
        let say = |h: Option<kh_core::Hit>| h.map_or("nothing".to_string(), |h| format!("{:+.1} cm from his feet at ({:+.0}, {:+.0}), normal ({:.2}, {:.2}, {:.2}) surface ({:.2}, {:.2}, {:.2}){} body {}", h.position.z - (p.z - height), h.position.x - p.x, h.position.y - p.y, h.normal.x, h.normal.y, h.normal.z, h.impact_normal.x, h.impact_normal.y, h.impact_normal.z, if h.start_penetrating { format!(" IN OVERLAP {:.1} cm", h.penetration_depth) } else { String::new() }, h.component));
        let again = self.stuck_last.filter(|(at, when, _)| (*at - p).length() < 100.0 && when.elapsed().as_secs_f32() < 10.0).map_or(0, |(_, _, n)| n);
        let ground = under.filter(|h| !h.start_penetrating && h.impact_normal.z >= SORA_PARAMS.floor_walkable_z && (h.position.z - (p.z - height)).abs() <= 30.0).map(|h| Vec3::new(p.x, p.y, h.position.z + height));
        let near = self.anchor.filter(|a| (*a - p).length() < 800.0);
        let to = if safe {
            let ground = ground.filter(|g| self.place_ok(*g).is_some());
            let beside = if ground.is_none() && again < 2 { self.spot_beside(p) } else { None };
            let anchor = near.filter(|a| self.place_ok(*a).is_some());
            match crate::guards::unstick_choice(again, ground.map(|g| g.z - p.z), beside.map(|g| g.z - p.z), anchor.map(|a| a.z - p.z)) {
                Unstick::Centre => ground.map(|g| (g, "ONTO THE WALKABLE GROUND UNDER HIS CENTRE (verified)")),
                Unstick::Beside => beside.map(|g| (g, "ONTO A FREE SPOT BESIDE HIM AT HIS HEIGHT (verified)")),
                Unstick::Anchor => anchor.map(|a| (a, "BACK TO WHERE HE LAST STOOD (verified)")),
                Unstick::Leave => None,
            }
        } else {
            match (again, ground, near) {
                (0, Some(g), _) => Some((g, "ONTO THE WALKABLE GROUND UNDER HIS CENTRE")),
                (0..=2, _, Some(a)) => Some((a, "BACK TO WHERE HE LAST STOOD")),
                _ => None,
            }
        };
        erlog!(
            "!!! stuck: Sora is WEDGED at kh ({:.0}, {:.0}, {:.1}) = Elden Ring ({:.2}, {:.2}, {:.2}): off the floor, speed z {:.0}, not moving (state {:?}; time no. {} here) | his body's sweep down: {} | a line under his centre: {} | a 15 cm sphere under his centre: {} => {}",
            p.x, p.y, p.z, self.feet_er().x, self.feet_er().y, self.feet_er().z, self.out.velocity.z, self.out.state, again + 1, say(body), say(line), say(under),
            to.map_or("LEFT AS HE IS (no verified place within reach: under his centre / beside him within 30 cm of his height, where he last stood within 1 m of it or at the third time; or freed four times here already)".to_string(), |(to, how)| format!("PUT {how}: from kh ({:.0}, {:.0}, {:.1}) to kh ({:.0}, {:.0}, {:.1}) = moved ({:+.0}, {:+.0}, {:+.1})", p.x, p.y, p.z, to.x, to.y, to.z, to.x - p.x, to.y - p.y, to.z - p.z)),
        );
        self.stuck_last = Some((p, std::time::Instant::now(), again + 1));
        if let Some((to, _)) = to {
            self.sora.teleport(Pose { position: to, yaw: self.out.yaw });
            self.out.position = to;
            self.out_before.position = to;
            self.write(player);
        }
    }

    /// THE TARNISHED IS DEAD OR DYING (Sora's death, or the game's own: deep water, a fall): the
    /// game owns its player; Sora goes on being simulated without input (his Die state plays) and
    /// drawn, nothing is written to the player. He starts again, full, when the game respawns him.
    pub fn prize_dead(&mut self, player: &mut PlayerIns, control: &Control, dt: f32) {
        self.prize.dead_frame(control, player, &mut self.sora, dt); // (prize)
    }

    pub fn dead_phase(&mut self, dt: f32, control: &Control, pre: bool) {
        self.finish.abort("Sora is dead"); // (IMPL_ER22) nothing of a boss finish outlives him (the game speed is put back)
        if !pre {
            self.submit(control);
            #[cfg(feature = "party")]
            self.party_draw(control); // (ER party S4) the friends handed to the renderer
            self.int_draw_fx(control); // (fx) the effects' draw list of the frame (INT: between the steps, on him as drawn)
            return;
        }
        inputs::set_mask(0);
        self.written = None;
        self.hp_written = None;
        self.cam.frame(control, false, false, crate::cam::CamSora { out: &self.out, before: &self.out_before, sora: &self.sora }, self.world.frame);
        self.cam.post(dt, 1.0, control);
        self.accumulator = (self.accumulator + dt).min(0.25);
        while self.accumulator >= STEP {
            self.accumulator -= STEP;
            let pad = PadState::from_held(PadButtons::NONE, self.held_before);
            self.held_before = PadButtons::NONE;
            let input = Input { move_stick: Vec2::ZERO, camera_yaw: self.out.yaw, ..Input::default() }.with_pad(pad);
            self.targets.list.clear();
            self.sora.set_shooter_look(Vec2::ZERO); // (F-ARROWGUN line 3, F-HUD, INT-F) no aim stick while down
            // (DTH) THE WHOLE PARTY GOES ON BEING STEPPED (no input, no targets): the leader's own KO plays, whoever he is
            // (before: only Sora's core was stepped here: a friend leader froze in the pose of his last live step)
            #[cfg(feature = "party")]
            let out = if self.party.core().is_some() {
                let input = self.party.route(&mut self.sora, &input, STEP, &self.world, &mut self.targets, self.clock as f64);
                let out = self.party.step_sora(&mut self.sora, STEP, &input, crate::party::soratargets::SoraSeams { world: &self.world, targets: &mut self.targets, fx: &mut self.fx, ui: &mut self.hud.keep }, &self.out);
                self.party.after_sora_step(&mut self.sora, crate::party::AfterSora { out: &out, dt: STEP, world: &self.world, targets: &mut self.targets, fx: &mut self.fx, lock_on: TargetId::NONE, look: None, leaping: false, now: self.clock as f64 });
                out
            } else {
                self.sora.set_aura(0.0, 0.0); // (DON-KIT) no party: no Donald's aura
                let mut host = Host { world: &self.world, targets: &mut self.targets, audio: &mut self.fx.audio, effects: &mut self.fx.effects, ui: &mut self.hud.keep /* (hud) */ };
                self.sora.step_hosted(STEP, &input, &mut host)
            };
            #[cfg(not(feature = "party"))]
            let out = {
                let mut host = Host { world: &self.world, targets: &mut self.targets, audio: &mut self.fx.audio, effects: &mut self.fx.effects, ui: &mut self.hud.keep /* (hud) */ };
                self.sora.step_hosted(STEP, &input, &mut host)
            };
            self.pose_before = std::mem::take(&mut self.pose);
            self.step_pose(&out);
            self.fx.effect_dt = STEP; // (IMPL_ER75) this path steps him at the fixed step
            self.herolight.end(&mut self.fx.effects); // (HEROLIGHT-KB) down: the hero light's glow and sparkle end at once
            self.herolight_left.end(&mut self.fx.effects); // (HEROLIGHT-KB final pass) and the left hand's
            self.fx.after_step(&out, self.animator.as_ref(), &self.pose, &self.targets, &self.sora); // (fx) the step's sounds and effects
            self.weapon.since += STEP;
            self.step_chain(&out);
            self.clock += STEP;
            self.out_before = std::mem::replace(&mut self.out, out);
        }
    }

    /// The pose of one step (as `kh-app`'s `step_pose`).
    fn step_pose(&mut self, out: &Output) {
        // (IMPL_ER16) the game's own action of the body (a pick-up, a door), laid over his pose
        if let (Some((source, pelvis, weight)), Some(retarget)) = (self.own_over.clone(), self.own_rig.as_ref().map(|r| r.retarget.clone())) {
            let to_world = component_to_world(out);
            // (IMPL_ER17) his arms are shorter: each hand is brought to where the body's hand is (the prop), as far as his arm reaches
            let reach = self.own_reach;
            // (ANIMS) his feet too when the plan planted them (no sliding, the crouch to an item on the ground)
            let feet = self.own_plan.is_some_and(|p| p.limbs[2].is_some());
            let limbs: &[[usize; 3]] = if feet { &[crate::figure::ARMS[0], crate::figure::ARMS[1], crate::figure::LEGS[0], crate::figure::LEGS[1]] } else { &crate::figure::ARMS };
            // (ER70) the worn outfit's own arms and legs (limbs.rs): solved on its skeleton
            let worn = match self.animator.as_ref() {
                Some(a) => self.worn.current(a.skeleton()),
                None => None,
            };
            // (ANIMS) the plan (propfit.rs): its contacts are the limbs' places
            let plan = self.own_plan;
            // (ANIMS) the drawn pose's limits (propfit.rs `limits`): a broken one draws his own pose
            let check_limits = plan.is_some();
            let mut overlay = |skeleton: &kh_anim::Skeleton, local: &mut [Trs]| {
                let own: Vec<Trs> = if check_limits { local.to_vec() } else { Vec::new() };
                retarget.apply(skeleton, &source, pelvis, &to_world, weight, local);
                if reach > 0.0 {
                    for (k, rows) in limbs.iter().copied().enumerate() {
                        let goal = plan.and_then(|p| p.limbs[k]).unwrap_or(source[rows[2]].0);
                        let to = to_world.rotation.conjugate() * (goal - to_world.translation);
                        match &worn {
                            Some(r) => {
                                crate::limbs::reach_outfit(r, local, k, to, weight * reach);
                            }
                            None => {
                                if let (Some(upper), Some(lower), Some(hand)) = (retarget.target_bone(rows[0]), retarget.target_bone(rows[1]), retarget.target_bone(rows[2])) {
                                    kh_anim::retarget::reach_arm(skeleton, upper, lower, hand, to, weight * reach, local);
                                }
                            }
                        }
                    }
                }
                if check_limits && own.len() == local.len() {
                    let (before, after) = (kh_anim::pose::component_space(skeleton, &own), kh_anim::pose::component_space(skeleton, local));
                    if let Err(why) = crate::propfit::limits(skeleton, &before, &after) {
                        static TOLD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                        if TOLD.fetch_add(1, std::sync::atomic::Ordering::Relaxed).is_multiple_of(600) {
                            erlog!("!!! own: the borrowed pose on Sora breaks a limit ({why}): his own pose is drawn");
                        }
                        local.copy_from_slice(&own);
                    }
                }
            };
            self.step_pose_over(out, STEP, Some((to_world, &mut overlay)));
            return;
        }
        self.step_pose_over(out, STEP, None);
    }

    /// (IMPL_ER16) `step_pose` with another component place and an overlay on the body's pose (the cutscene's figure).
    fn step_pose_over(&mut self, out: &Output, dt: f32, over: Option<PoseOver<'_>>) {
        // (ER69) the outfit's body size: the collision capsule
        let scale = crate::boc::body_scale();
        if (self.sora.params().capsule_half_height - kh_core::params::SORA_PARAMS.capsule_half_height * scale).abs() > 0.01 {
            self.sora.set_body_scale(scale);
        }
        // (ER-TWEAKS) Sora leading through the fog (fogwalk.rs): KH3's `WalkLoopN` (an outfit's own of that name when it has
        // one), its time by the distance the game carried him, in place of his held idle (the leap's clip route below)
        let fog_show = (self.fog_walk.on && self.friend_lead().is_none()).then(|| {
            let walk = kh_core::AnimId::WalkLoopN;
            crate::ladder::Show::Clip { anim: walk, time: crate::fogwalk::clip_time(self.fog_walk.distance, crate::fogwalk::sora_stride(), walk.data().length), looping: true, play: 0x0E00_0000 + (self.fog_walk.walks & 0xFFFF) }
        });
        let Some(animator) = self.animator.as_mut() else { return };
        // (IMPL_ER59) the worn outfit's own clips (boc.rs): sampled instead of his of the same names
        if let Some(dir) = crate::boc::take_clip_dir() {
            // (ER79) the clips of `boc.<outfit>.use_kh3` stay KH3 Sora's (retargeted by the renderer)
            let names = match &dir {
                Some((dir, kh3)) => animator.set_own_clips_except(Some(dir), kh3),
                None => animator.set_own_clips(None),
            };
            crate::erlog!("boc: {} clip(s) of the outfit's own in play{}", names.len(), if names.is_empty() { String::new() } else { format!(" ({} ...)", names.iter().take(8).cloned().collect::<Vec<_>>().join(" ")) });
        }
        // (IMPL_ER23A) the leg IK: his own poses and the body's actions in play; never the cutscene figure's pose (it lies, sits, is carried)
        // (LADDER2) in his ladder leap KH3's own jump clips replace his held state's (no leg IK: he is in the air)
        // (GRABS) an enemy's grab: KH3's caught sequence in his body slot (grabs.rs; the leap's route), before the leap's clips
        let show = self.catch.and_then(|c| c.shown(sora_catch_lengths())).and_then(catch_show).unwrap_or(self.leap_show);
        let show = match (show, fog_show) {
            (crate::ladder::Show::Own, Some(walk)) => walk, // (ER-TWEAKS) the grab's and the leap's clips win
            (show, _) => show,
        };
        animator.set_leg_ik(self.leg_ik && !self.cutscene && show == crate::ladder::Show::Own);
        let (shown, ticked) = match show {
            crate::ladder::Show::Own => (out.anim, out.anim_ticked),
            crate::ladder::Show::Clip { anim, time, looping, play } => {
                let data = anim.data();
                let r = kh_core::AnimRequest { name: anim, play_id: 0x4C00_0000 + play, time, active: true, weight: 1.0, looping, blend_in: 0.1, axis: 0.0, axis_y: 0.0 /* (F-LOCO) */, root_motion: data.root_motion_enabled, notify_window: (0.0, 0.0), locomotion: None, length: data.length };
                (r, r)
            }
        };
        let sequences = ticked.name.sequences();
        let (samples, time, blend_with) = match shown.locomotion {
            Some(l) => (l.run.sample_weights(shown.axis), l.run_time, Some((l.walk.sequences()[0], l.walk_time, 1.0 - l.run_weight))),
            None => {
                let [(first, first_weight, first_time), (second, second_weight, second_time)] = shown.body_samples();
                if second_weight > 0.0 && second_time != first_time {
                    ([(first, 1.0), (first, 0.0)], first_time, Some((second, second_time, second_weight)))
                } else {
                    ([(first, first_weight), (second, second_weight)], first_time, None)
                }
            }
        };
        let body = BodySlot { samples, blend_with, notify_sequences: &sequences, time, weight: shown.weight, active: shown.active, notify_window: ticked.notify_window };
        self.pose = match over {
            Some((component_to_world, overlay)) => animator.step_with(&PoseFrame { dt, body, component_to_world }, &self.world, Some(overlay)).clone(),
            None => animator.step(&PoseFrame { dt, body, component_to_world: component_to_world(out) }, &self.world).clone(),
        };
        // (ER83) the outfit's hand contact IK leaves the outfit's own clips alone (their share of the pose)
        #[cfg(windows)]
        kh_er_render::handik::set_kh3_share(1.0 - animator.own_share());
    }

    /// The keychain of one step (as kh-app's `step_pose`): the weapon's component is the socket on
    /// bone R_buki; while the weapon is hidden the chain is frozen, and reset when it appears.
    fn step_chain(&mut self, out: &Output) {
        kbdraw::equip(self.sora.status()); // (P5) the equipped keyblade to the renderer, its switch timing and effects
        // (FORMWEP-FIX) once a step: a form just started and its transformation not begun yet keeps the hand Keyblade drawn
        self.form_gate.step(&self.sora.combat().form, out.state);
        let visible = self.weapon.opacity() > 0.0;
        let (Some(chain), Some(animator)) = (self.keychain.as_mut(), self.animator.as_ref()) else { return };
        let Some(hand) = animator.skeleton().index_of("R_buki") else { return };
        // (KEYCHAIN-DUAL) the Keyblades his form holds, each with its own keychain (Double Form's two, Light / Dark Form's), on
        // the hands they are drawn in (an outfit's, as his hand's keychain below); `submit` draws them (`apply_form_chains`);
        // the clip his weapons play (each weapon's own clip of it runs its Bonamik states: the hand's too, below)
        let clip = {
            let shown = self.weapon.opacity() * (1.0 - self.own_weight).clamp(0.0, 1.0) * kbdraw::visibility() > 0.0;
            #[cfg(windows)]
            let effect = crate::boc::effect_pose(animator.skeleton(), &self.pose);
            #[cfg(windows)]
            let component = effect.as_ref().map_or(&self.pose.component, |p| &p.component);
            #[cfg(not(windows))]
            let component = &self.pose.component;
            let kb = kbdraw::StepParts { sora: &self.sora, out, skeleton: animator.skeleton(), component, to_world: component_to_world(out), shown, test_aid: true, start_pending: self.form_gate.on() };
            kbdraw::step_form_chains(STEP, &kb, Some(animator));
            kb.clip()
        };
        let input = kbdraw::ChainIn { animator: Some(animator), clip }; // (KEYCHAIN-DUAL) his routed clip states, the weapon's own
        if visible {
            let Some(his_hand) = self.pose.component.get(hand).copied() else { return };
            // (ER79) with an outfit on, the Keyblade is drawn in the OUTFIT'S hand (kh-er-render outfit.rs weapon_to_world): its
            // keychain hangs from there, not from his hand (whose turn differs: the chain stood off the handle, the user's 53.jpg)
            #[cfg(windows)]
            let hand_pose = crate::boc::effect_pose(animator.skeleton(), &self.pose).and_then(|p| p.component.get(hand).copied()).unwrap_or(his_hand);
            #[cfg(not(windows))]
            let hand_pose = his_hand;
            let hand_pose = &hand_pose;
            let appeared = !self.weapon_shown;
            if appeared {
                chain.reset();
            }
            let weapon_to_world = weapon_socket().then(hand_pose).then(&component_to_world(out));
            // (P5) another keyblade in his hand: ITS keychain (its own skeleton and Bonamik asset), else the Kingdom Key's
            let now = kbdraw::step_chain(STEP, &weapon_to_world, &input, appeared).unwrap_or_else(|| kbdraw::step_keychain(chain, kh_er_render::keyblade::KINGDOM_KEY, STEP, None, &weapon_to_world, &input).to_vec());
            self.chain_before = std::mem::replace(&mut self.chain, now);
            if appeared || self.chain_before.len() != self.chain.len() {
                self.chain_before = self.chain.clone();
            }
        }
        self.weapon_shown = visible;
    }

    /// (IMPL_ER16) ONCE A FRAME (PostPhysics): in a real-time cutscene the scene's own figure of the player (figure.rs:
    /// found and held hidden in every phase) gives Sora his pose: its bones are read, retargeted onto his skeleton
    /// (kh-anim retarget) as an overlay of his animator's step, so his helper bones, hair and clothes follow it.
    /// `cut.sora` = retarget (default) | hide (the figure hidden, Sora not drawn) | game (the game's figure shown, Sora
    /// not drawn) | double (as before IMPL_ER16: both).
    fn cut_frame(&mut self, control: &Control) {
        let before = self.cut_draw;
        let mode = control.str("cut.sora").unwrap_or("retarget");
        let now = std::time::Instant::now();
        let dt = self.cut_at.replace(now).map_or(STEP, |t| (now - t).as_secs_f32().clamp(0.001, 0.1));
        // (ER party S2, U6) a friend leading stands in for the Tarnished; AI Sora is not drawn
        #[cfg(feature = "party")]
        let friend_cut = self.friend_lead().is_some() && self.cutscene && mode == "retarget";
        #[cfg(feature = "party")]
        if !self.cutscene && std::mem::take(&mut self.lead.cut_on) {
            self.lead.over = None;
            self.lead.hidden = false;
        }
        #[cfg(not(feature = "party"))]
        let friend_cut = false;
        self.cut_draw = if !self.cutscene || mode == "double" {
            CutDraw::Normal
        } else if mode != "retarget" {
            CutDraw::Hidden
        } else if friend_cut {
            #[cfg(feature = "party")]
            {
                self.lead.cut_on = true;
                self.lead_cut(control.flag("cut.fit") != Some(false)); // (CUTSCENE-ANIM) `cut.fit`
            }
            CutDraw::Hidden
        } else {
            let posed = match (crate::figure::CUT.lock(), self.animator.as_ref()) {
                (Ok(mut guard), Some(animator)) => guard.as_mut().and_then(|cut| cut.posed(animator.skeleton(), &self.world.frame).and_then(|p| cut.retarget().cloned().map(|r| (p, r)))),
                _ => None,
            };
            match posed {
                Some((posed, retarget)) => {
                    // (IMPL_ER17) his face: eyes closed while the figure lies (pelvis to head nearer the floor than upright), his own otherwise
                    let lying = (posed.source[5].0 - posed.source[0].0).normalize_or_zero().z < 0.5;
                    if control.flag("cut.face") != Some(false) && self.cut_lying != Some(lying) {
                        self.cut_lying = Some(lying);
                        if let Some(animator) = self.animator.as_mut() {
                            if lying { animator.play_face(Some("NO_1"), "NORMAL_AUTO", 0.3) } else { animator.play_face(None, "NORMAL_AUTO", 0.3) }
                        }
                        erlog!("cutfig: his face: {}", if lying { "EYES CLOSED (the figure lies)" } else { "his own (the figure is up)" });
                    }
                    let out = self.out.clone();
                    let to_world = posed.component_to_world;
                    // (CUTSCENE-ANIM) AT HIS PROPORTIONS ON THE SCENE'S FLOOR (cutfit.rs; `cut.fit = 0`: the rule before): measured
                    // on the skeleton that is DRAWN (the worn outfit's: KH1's shorter legs rest on the floor, not his), hands and
                    // feet on what the figure's touch
                    let worn = match (self.animator.as_ref(), control.flag("cut.fit") != Some(false)) {
                        (Some(a), true) => self.worn.current(a.skeleton()),
                        _ => None,
                    };
                    let fit_on = control.flag("cut.fit") != Some(false);
                    let sora_body = self.animator.as_ref().and_then(|a| crate::cutfit::body_of_skeleton(a.skeleton(), crate::cutfit::SORA_DEPTH));
                    let fitted = std::cell::Cell::new(None);
                    let mut overlay = |skeleton: &kh_anim::Skeleton, local: &mut [Trs]| {
                        let done = match (fit_on, posed.body.as_ref(), worn.as_deref(), sora_body.as_ref()) {
                            (true, Some(s), Some(w), _) => crate::cutfit::apply_outfit(&retarget, skeleton, &posed.source, &posed.source, (s, w), &to_world, local),
                            (true, Some(s), None, Some(t)) => crate::cutfit::apply_skeleton(&retarget, skeleton, &posed.source, &posed.source, (s, t), &to_world, local),
                            _ => None,
                        };
                        if done.is_none() {
                            retarget.apply(skeleton, &posed.source, posed.pelvis, &to_world, 1.0, local);
                        }
                        fitted.set(done);
                    };
                    self.step_pose_over(&out, dt, Some((to_world, &mut overlay)));
                    if fitted.get().map(|f| f.rests_on) != self.cut_rests {
                        self.cut_rests = fitted.get().map(|f| f.rests_on);
                        match fitted.get() {
                            Some(f) => erlog!("cutfig: (CUTSCENE-ANIM) the figure rests on its {} (floor z {:.0}), Sora{} on his {} (lifted {:+.1} cm from the figure's pelvis height)", crate::cutfit::part_name(f.rests_on.0), f.floor, if worn.is_some() { " (the worn outfit)" } else { "" }, crate::cutfit::part_name(f.rests_on.1), f.lift),
                            None => erlog!("cutfig: (CUTSCENE-ANIM) Sora placed as before (`cut.fit = 0` or the figure's measures not readable)"),
                        }
                    }
                    CutDraw::Posed(to_world)
                }
                None => CutDraw::Hidden,
            }
        };
        if !self.cutscene && self.cut_lying.take() == Some(true) {
            if let Some(animator) = self.animator.as_mut() {
                animator.play_face(None, "NORMAL_AUTO", 0.3);
            }
        }
        if std::mem::discriminant(&before) != std::mem::discriminant(&self.cut_draw) {
            erlog!("cutfig: Sora's drawing: {}", match self.cut_draw { CutDraw::Normal => "as in play", CutDraw::Hidden => "NOT DRAWN (the cut shows no figure of the player)", CutDraw::Posed(_) => "THE SCENE'S FIGURE, RETARGETED ONTO HIS SKELETON" });
        }
    }

    /// (IMPL_ER16) ONCE A FRAME (PostPhysics), IN PLAY: while the hidden body plays one of the game's EVENT ANIMATIONS
    /// (ids 60000 .. 69999: item pick-up, doors, levers, the grace; `own.anims = lo-hi`; `own.retarget = 0` switches it
    /// off) its skeleton (the real player's model: the same layout as the cutscene figure's, MEASURED) is read and kept
    /// for the next steps' overlay, eased in and out over 0.2 s.
    fn own_frame(&mut self, player: &PlayerIns, control: &Control) {
        let ta = &player.chr_ins.modules.time_act;
        let anim = ta.anim_queue[(ta.read_idx as usize) % 10].anim_id;
        // (IMPL_ER33) while the body attunes to / rests at a site of grace he is held (no input) but does NOT take its pose: he neither kneels nor sits (`savepoint.sit_pose = 1`)
        // (GRABS) a grab is no longer the body's pose retargeted: KH3's caught sequence (grabs.rs; `grab.kh = 0`: as before)
        let grab_owned = self.follow == Some(Follow::Grab) && control.flag("grab.kh") == Some(false);
        let owned = matches!(self.follow, Some(Follow::Action)) || grab_owned || (self.own_hold && !(self.savepoint.hides_grace_pose() && crate::story::grace_animation(anim)));
        // (ER party S2) a friend leading takes the body's pose; Sora keeps his own
        // (CHESTFIX, the user 2026-10-27: "Goofy and Donald should have no chest animation") NOT the body's chest opening
        // (U13's retarget is gone): seated facing the chest, he shows his own pose (his idle) while the game opens it
        #[cfg(feature = "party")]
        let held_by_body = owned && !(self.follow == Some(Follow::Action) && self.chest.on());
        let owned = owned && self.friend_lead().is_none();
        // (IMPL_ER58) a chest shown with KH3's own animation: no Elden Ring pose laid over him
        let owned = owned && !(self.follow == Some(Follow::Action) && self.chest.on());
        // (ER-TWEAKS) THE FOG WALK (fogwalk.rs): carried through a walking action (the fog), WHOEVER LEADS shows his own walk
        // and no pose of the Tarnished's (Sora: his `WalkLoopN` in `step_pose_over`; a friend: his walk below)
        let walking = self.follow == Some(Follow::Action) && self.walk_action;
        self.fog_walk.frame(walking, self.fog_trail, self.anims_dt);
        if walking != self.fog_walk_told {
            self.fog_walk_told = walking;
            erlog!("own: (ER-TWEAKS) {}", if walking { format!("THE GAME CARRIES THE LEADER ({}) THROUGH ITS ACTION (animation {anim}: the fog): HIS OWN WALK is shown, not the Tarnished's (fog.walk = 0: the old pose)", self.lead_chara_name()) } else { format!("the walk through the action is over ({:.0} cm walked)", self.fog_walk.distance) });
        }
        let owned = owned && !walking;
        #[cfg(feature = "party")]
        let held_by_body = held_by_body && !walking;
        // (IMPL_ER17, `log.own = 1`) every change of the body's animation, for finding the ids of the game's own actions
        // (IMPL_ER18: ON unless `log.own = 0`: a session's log then names the ids of actions nobody measured yet)
        if control.flag("log.own") != Some(false) && anim != self.own_seen {
            let slot = &ta.anim_queue[(ta.read_idx as usize) % 10];
            erlog!("own: body animation {} -> {anim} (length {:.2} s; prompt {}, Sora {:?})", self.own_seen, slot.anim_length, if self.prompt { "up" } else { "not up" }, self.out.state);
        }
        self.own_seen = anim;
        if owned != (self.own_anim != -1) {
            erlog!("own: {}", if owned { format!("THE GAME ANIMATES ITS BODY (animation {anim}): Sora shows what it does (retargeted), eased in") } else { format!("the body's action (animation {}) is over: Sora's own pose again", self.own_anim) });
            self.own_anim = if owned { anim } else { -1 };
        }
        self.own_reach = control.f32("own.reach").unwrap_or(1.0).clamp(0.0, 1.0);
        let blend = control.f32("own.blend").unwrap_or(0.2);
        let step = 1.0 / (blend.max(0.02) * 60.0);
        #[cfg(feature = "party")]
        self.lead_frame(player, control, held_by_body, anim, step); // (ER party S2) the leader's pose of the body
        // (ER-TWEAKS) a friend leading through the fog: HIS OWN WALK at full weight from the first frame (party/leader.rs
        // `walk_pose`), its phase by the carry; given back (None) once eased out
        #[cfg(feature = "party")]
        {
            let fog_lead = self.friend_lead().is_some() && self.lead.active() && !self.cutscene && self.fog_walk.weight > 0.0;
            if fog_lead {
                self.lead.walk_draw = Some((self.lead.chara(), self.fog_walk.distance, self.fog_walk.speed, self.fog_walk.weight));
                self.fog_drew = true;
            } else if std::mem::take(&mut self.fog_drew) {
                self.lead.walk_draw = None;
            }
        }
        self.own_weight = if owned { (self.own_weight + step).min(1.0) } else { (self.own_weight - step).max(0.0) };
        self.own_over = None;
        self.own_plan = None;
        if self.own_weight <= 0.0 {
            self.own_fit.reset(); // (ANIMS) the next action starts afresh
            return;
        }
        let Some(animator) = self.animator.as_ref() else { return };
        let item = crate::probe::word(crate::word!(player.chr_ins.chr_model_ins) + 0x10).unwrap_or(0);
        let flver = crate::probe::word(item + 0x68).unwrap_or(0);
        if item == 0 || flver == 0 {
            return;
        }
        if self.own_rig.as_ref().is_none_or(|r| r.flver != flver) {
            match crate::figure::Rig::build(item, animator.skeleton()) {
                Ok(rig) => self.own_rig = Some(rig),
                Err(e) => {
                    erlog!("own: the body's skeleton is not usable ({e})");
                    self.own_weight = 0.0;
                    return;
                }
            }
        }
        let Some(rig) = self.own_rig.as_ref() else { return };
        let Some(er) = rig.read(item) else { return };
        let mut source: Vec<(Vec3, Quat)> = er.iter().map(|(p, q)| (self.world.frame.pos_er_to_kh(*p), crate::figure::rotation_kh(*q))).collect();
        let ratio = rig.retarget.ratio;
        // (his own floor: where he stands)
        let floor = self.out.position.z - Sora::origin_height_on_floor(&SORA_PARAMS);
        // (ANIMS) AT HIS PROPORTIONS (propfit.rs; `own.fit = 0`: the rule before): the pelvis, the standoff, his contacts;
        // the plan is laid on in `step_pose`
        // (ANIMS) a plan the sanity check refuses is not used: the retarget as before
        let (plan, pelvis) = match self.anims_plan(control, item, flver, &mut source, None, Vec3::new(self.out.position.x, self.out.position.y, floor)).filter(|p| p.sane) {
            Some(plan) => (Some(plan), plan.pelvis),
            None => (None, { let h = source[0].0.z - floor; source[0].0 + Vec3::Z * (h * ratio - h) }), // (`Retarget::pelvis_place`)
        };
        self.own_plan = plan;
        // (smooth step of the ease)
        let w = self.own_weight;
        self.own_over = Some((source, pelvis, w * w * (3.0 - 2.0 * w)));
    }


    /// (ANIMS) What the body's action touches (propfit.rs `Kind`).
    fn anims_kind(&self) -> crate::propfit::Kind {
        if self.own_surface {
            crate::propfit::Kind::Surface
        } else {
            crate::propfit::Kind::Free
        }
    }

    /// (ANIMS) ONE FRAME OF THE FIT for whoever leads (`chara` None: Sora, his fit; a friend: the leader's): the body's pose
    /// (`source`, every `ROWS` row, KH3 world), the measures (the Tarnished's from the body's FLVER bind pose, his from his
    /// skeleton), the plan. None: `own.fit = 0` or the measures are not readable (the rule before stays).
    /// `root`: his feet's place on his floor (its z is the floor's height).
    fn anims_plan(&mut self, control: &Control, item: usize, flver: usize, source: &mut [(Vec3, Quat)], chara: Option<kh_host::party::PartyChara>, root: Vec3) -> Option<crate::propfit::Plan> {
        let floor = root.z;
        if control.flag("own.fit") == Some(false) || source.len() != crate::figure::ROWS.len() {
            return None;
        }
        let kind = self.anims_kind();
        let dt = self.anims_dt;
        // the Tarnished's measures (per FLVER), his
        if self.own_dims.is_none_or(|(f, _, _)| f != flver) {
            let s = crate::figure::Rig::source_binds(item).ok().and_then(|b| crate::propfit::Dims::of_binds(&b));
            let t = self.animator.as_ref().and_then(|a| crate::propfit::Dims::of_skeleton(a.skeleton())); // (Sora's; a friend's are read below)
            match (s, t) {
                (Some(s), Some(t)) => {
                    erlog!("own: the measures for the borrowed actions: the Tarnished's pelvis {:.0} cm, arms {:.0}, legs {:.0}; Sora's pelvis {:.0}, arms {:.0}, legs {:.0}", s.pelvis_h, s.limb[0], s.limb[2], t.pelvis_h, t.limb[0], t.limb[2]);
                    self.own_dims = Some((flver, s, t));
                }
                _ => {
                    erlog!("own: the measures for the borrowed actions are not readable: the retarget as before");
                    return None;
                }
            }
        }
        let (_, s, t) = self.own_dims?;
        match chara {
            None => {
                let ratio = self.own_rig.as_ref().map_or(1.0, |r| r.retarget.ratio);
                Some(self.own_fit.plan(source, crate::propfit::Measures { s: &s, t: &t, ratio, prof: &crate::propfit::Profile::SORA }, root, floor, kind, dt))
            }
            #[cfg(feature = "party")]
            Some(c) => {
                let skeleton = self.lead.skeleton(c)?;
                let t = crate::propfit::Dims::of_skeleton(&skeleton)?;
                Some(self.lead.fit.plan(source, crate::propfit::Measures { s: &s, t: &t, ratio: 1.0, prof: &crate::propfit::Profile::of(c) }, root, floor, kind, dt))
            }
            #[cfg(not(feature = "party"))]
            Some(_) => None,
        }
    }

    /// (ER party S2) ONCE A FRAME (own_frame), while the party host runs: THE LEADER'S FIGURE OF THE BODY. A friend leading:
    /// while the body performs the game's own action (`held`: a door, a grab, a pick-up held in place, a chest: U13) its
    /// pose, retargeted onto HIS skeleton, is laid over his (party/leader.rs `PoseOver`, eased as Sora's `own_weight`; his
    /// hands brought to the body's as far as his limbs reach).
    #[cfg(feature = "party")]
    fn lead_frame(&mut self, player: &PlayerIns, control: &Control, held: bool, anim: i32, step: f32) {
        use crate::party::leader::PoseOver;
        if !self.lead.active() || self.cutscene {
            return;
        }
        let friend = self.friend_lead();
        let was = self.lead.over_weight;
        let w = if friend.is_some() && held { (was + step).min(1.0) } else { (was - step).max(0.0) };
        self.lead.over_weight = w;
        if (w > 0.0) != (was > 0.0) {
            erlog!("own: {}", if w > 0.0 { format!("THE GAME ANIMATES ITS BODY (animation {anim}): the leader ({}) shows what it does (retargeted onto his skeleton), eased in", self.lead.chara().name()) } else { "the body's action is over: the leader's own pose again".to_string() });
        }
        if friend.is_none() || w <= 0.0 {
            self.lead.over = None;
            return;
        }
        let item = crate::probe::word(crate::word!(player.chr_ins.chr_model_ins) + 0x10).unwrap_or(0);
        let flver = crate::probe::word(item + 0x68).unwrap_or(0);
        if item == 0 || flver == 0 {
            self.lead.over = None;
            return;
        }
        // the rig: on the friend leader's skeleton
        let chara = self.lead.chara();
        if self.lead.rig.as_ref().is_none_or(|(f, c, _)| *f != flver || *c != chara) && self.lead.rig_refused != Some((flver, chara)) {
            let built = self.lead.skeleton(chara).ok_or_else(|| "no skeleton".to_string()).and_then(|s| crate::figure::Rig::build_for(item, &s));
            match built {
                Ok(rig) => {
                    erlog!("own: the body's skeleton is read for {} ({} of {} rows; his legs {:.2} of its)", chara.name(), rig.keep.as_ref().map_or(crate::figure::ROWS.len(), |k| k.len()), crate::figure::ROWS.len(), rig.retarget.ratio);
                    self.lead.rig = Some((flver, chara, rig));
                }
                Err(e) => {
                    erlog!("own: the body's skeleton is not usable for {} ({e}): the leader shows his own pose", chara.name());
                    self.lead.rig_refused = Some((flver, chara));
                }
            }
        }
        let Some((_, _, rig)) = self.lead.rig.as_ref() else {
            self.lead.over = None;
            return;
        };
        let Some(er) = rig.read(item) else {
            self.lead.over = None;
            return;
        };
        let source: Vec<(Vec3, Quat)> = er.iter().map(|(p, q)| (self.world.frame.pos_er_to_kh(*p), crate::figure::rotation_kh(*q))).collect();
        let (retarget, keep) = (rig.retarget.clone(), rig.keep.clone().unwrap_or_else(|| (0..crate::figure::ROWS.len()).collect()));
        // (ANIMS) THE WALK THROUGH (propfit.rs `WalkThrough`; `own.walk = 0`: off): the game walks the friend leader through a
        // borrowed action (the fog, a door's steps): HIS OWN KH3 WALK is drawn (partydraw: `walk_pose`), not the Tarnished's
        let walk_ok = control.flag("own.walk") != Some(false);
        self.lead.walk_draw = match friend {
            Some(fl) => {
                let weight = self.lead.walk.frame(walk_ok && held, fl.position, self.anims_dt);
                if self.lead.walk.on != self.lead_walk_told {
                    self.lead_walk_told = self.lead.walk.on;
                    erlog!("own: {}", if self.lead.walk.on { format!("THE GAME WALKS THE LEADER ({}) THROUGH ITS ACTION (animation {anim}, {:.0} cm/s): his own KH3 walk, not the Tarnished's", self.lead.chara().name(), self.lead.walk.speed) } else { "the leader's walk through the action is over".to_string() });
                }
                (weight > 0.0).then(|| (self.lead.chara(), self.lead.walk.distance, self.lead.walk.speed, weight))
            }
            None => {
                self.lead.walk = Default::default();
                None
            }
        };
        let Some(f) = friend.filter(|_| w > 0.0) else {
            self.lead.over = None;
            self.lead.fit.reset(); // (ANIMS) the next action starts afresh
            return;
        };
        // his own floor: where he stands
        let floor = f.position.z;
        // (ANIMS) AT HIS PROPORTIONS (propfit.rs; `own.fit = 0`: the rule before): the pelvis at his size, the standoff from
        // the door, his feet planted, his posture kept in part
        let mut source = source;
        let flver = crate::probe::word(item + 0x68).unwrap_or(0);
        let plan = self.anims_plan(control, item, flver, &mut source, Some(self.lead.chara()), Vec3::new(f.position.x, f.position.y, floor));
        // (ANIMS) THE POSE SANITY (propfit.rs `sanity`): a plan that would fold or over-stretch him is not shown: his own pose
        if plan.is_some_and(|p| !p.sane) {
            if !self.lead_insane_told {
                self.lead_insane_told = true;
                erlog!("!!! own: the borrowed pose (animation {anim}) would fold or over-stretch {}: his own pose is shown instead (propfit sanity)", self.lead.chara().name());
            }
            self.lead.over = None;
            return;
        }
        self.lead_insane_told = false;
        let pelvis = plan.map_or_else(|| retarget.pelvis_place(source[0].0, Vec3::Z, floor), |p| p.pelvis);
        self.lead.over = Some(PoseOver { chara: self.lead.chara(), retarget, keep, source, pelvis, weight: w * w * (3.0 - 2.0 * w), place: None, limbs: 2, reach: self.own_reach, hide_weapon: true, plan, cut: None /* (CUTSCENE-ANIM) */ });
    }

    /// (ER party S2, U6) A REAL-TIME CUTSCENE WITH A FRIEND LEADING: the scene's figure of the player retargeted onto HIS
    /// skeleton, in its place (party/leader.rs `PoseOver` with the figure's place); AI Sora is not drawn (S1 hides the
    /// others). False: no figure of the player in this cut (the leader is not drawn either: `LeaderBody::hidden`).
    #[cfg(feature = "party")]
    fn lead_cut(&mut self, fit: bool) -> bool {
        use crate::party::leader::PoseOver;
        let chara = self.lead.chara();
        let Some(skeleton) = self.lead.skeleton(chara) else { return false };
        // (CUTSCENE-ANIM) his measures for the fit on the scene's floor (cutfit.rs); `cut.fit = 0`: none (the rule before)
        let cut_body = crate::cutfit::body_of_skeleton(&skeleton, crate::cutfit::depth_of(chara)).filter(|_| fit);
        let posed = match crate::figure::CUT.lock() {
            Ok(mut guard) => guard.as_mut().and_then(|cut| cut.posed_party(&skeleton, &self.world.frame)),
            Err(_) => None,
        };
        match posed {
            Some((posed, retarget, keep)) => {
                self.lead.over = Some(PoseOver { chara, retarget, keep, source: posed.source, pelvis: posed.pelvis, weight: 1.0, place: Some(posed.component_to_world), limbs: 0, reach: 0.0, hide_weapon: true, plan: None, cut: posed.body.zip(cut_body) /* (CUTSCENE-ANIM) */ });
                self.lead.hidden = false;
                true
            }
            None => {
                self.lead.over = None;
                self.lead.hidden = true;
                false
            }
        }
    }

    /// (ER party S4) The party's drawing of this frame (partydraw.rs): at the same place between the steps as Sora's.
    #[cfg(feature = "party")]
    fn party_draw(&mut self, control: &Control) {
        let alpha = crate::interp::alpha(control, self.accumulator); // (INT) `r.interp` default on
        crate::partydraw::frame(&mut self.party, &mut self.lead, control, alpha, STEP, self.cutscene);
    }

    /// (F-SHIP INT 3, INT-F) HIGHWIND'S SPEARS IN FLIGHT: each `HL_SPEAR` shot of the core (combat/forms/ship.rs: the rain of
    /// the finisher and the dive; the core moves them and spawns their stick / hit effects itself) carries TresProjHL_Spear's
    /// body particle `vw_so092_fns_blt0` at its shape's centre, once a frame after the steps; gone with its shot. A shot whose
    /// life went UP is a new spear of the same key (a later rain).
    fn spear_fx(&mut self) {
        const BODY: &str = "vw_so092_fns_blt0";
        // (AUDIT-B W-FIN) + the shot's yaw: 0 in flight (the rain spawns them at yaw 0), the hit bone's turn once stuck (ShotStick)
        let now: Vec<(u16, f32, Vec3, f32)> = self.sora.combat().shots.iter().filter(|s| s.name == "HL_SPEAR").map(|s| (s.window.key.1, s.life, s.centre(), s.yaw)).collect();
        if now.is_empty() && self.spears.is_empty() {
            return;
        }
        let mut kept: Vec<(u16, f32, u64)> = Vec::with_capacity(now.len());
        let mut old = std::mem::take(&mut self.spears);
        crate::fx::with_effects(|e| {
            for (key, life, at, yaw) in &now {
                let id = match old.iter().position(|o| o.0 == *key && o.1 >= *life && e.free_alive(o.2)) {
                    Some(i) => {
                        let id = old.swap_remove(i).2;
                        e.free_move(id, *at, *yaw, 1.0);
                        Some(id)
                    }
                    None => e.free_spawn(BODY, *at, *yaw, 1.0),
                };
                if let Some(id) = id {
                    kept.push((*key, *life, id));
                }
            }
            for (_, _, id) in &old {
                e.free_stop(*id, false);
            }
        });
        self.spears = kept;
    }

    /// What the renderer draws this frame: the latest step, or with `r.interp` the blend of the
    /// two latest steps by how far the frame is between them (as the test app draws).
    fn submit(&self, control: &Control) {
        *self.int_drawn.borrow_mut() = None; // (INT) set below when he is drawn between the steps
        let Some(camera) = game::camera() else { return };
        // (NF) KH3's camera fade (kh-er-render camfade.rs): where the camera looks; `r.camfade = 0` draws everyone whole
        kh_er_render::camfade::set_look(self.cam.look_point());
        kh_er_render::camfade::set_enabled(control.flag("r.camfade"));
        kh_er_render::camfade::set_friends_look_fade(control.flag("r.camfade_friends") == Some(true)); // (RSF round 18) default off: friends as Sora
        // (IMPL_ER59, weapons P5b) Boc's talk seen; the outfit drawn; the wear form shown (boc::wear_form_shown -> kh_er_render::set_wear_form)
        // (SHOT-B line 2, INT-F) while a shotlock holds a weapon form, that form's wear form (equipment vt+0x5F8: Frozen Crescents
        // Grace, Flag Rampage Brave), back to the held state's own when it returns to 0
        let shot_form = self.sora.magic().shot_weapon_form;
        let wear = if shot_form != 0 { kh_core::magic::shotlocks::vacuum::wear_form(shot_form).unwrap_or(self.sora.combat().form.wear_form) } else { self.sora.combat().form.wear_form };
        crate::boc::frame(control, self.popup.map(|p| p.talk_id), wear);
        let Some(animator) = self.animator.as_ref() else { return };
        if self.pose.component.is_empty() {
            return;
        }
        let cam = kh_er_render::Camera { right: camera.right, up: camera.up, forward: camera.forward, position: camera.position, fov: camera.fov, aspect: camera.aspect, near: camera.near, far: camera.far };
        match self.cut_draw {
            CutDraw::Normal => {}
            // (IMPL_ER16) the scene's cut does not show the player: nothing of his is drawn
            CutDraw::Hidden => {
                kh_er_render::submit(kh_er_render::FrameInput { camera: cam, origin: self.world.frame.origin, clock: self.clock, body: None, weapon: None, forms: Vec::new() });
                return;
            }
            // (IMPL_ER16) the scene's figure shown as him: the pose of this frame, no weapon (his own state has none out)
            CutDraw::Posed(to_world) => {
                kh_er_render::submit(kh_er_render::FrameInput { camera: cam, origin: self.world.frame.origin, clock: self.clock, body: Some(kh_er_render::Body { component_to_world: to_world, local: self.pose.local.clone(), component: self.pose.component.clone() }), weapon: None, forms: Vec::new() });
                return;
            }
        }
        // (ER party S8) SORA OUT OF THE PARTY (not unlocked yet, or slotted out) IS NOT DRAWN, unless he is found fighting
        // Margit (unlock/host.rs: his stage)
        #[cfg(feature = "party")]
        if self.party.core().is_some_and(|c| c.sora_skipped()) && !crate::unlock::sora_staged() {
            kh_er_render::submit(kh_er_render::FrameInput { camera: cam, origin: self.world.frame.origin, clock: self.clock, body: None, weapon: None, forms: Vec::new() });
            return;
        }
        // (INT) `r.interp` default ON: his pose, transform and keychain between the two latest steps (interp.rs `sora_drawn`)
        let alpha = crate::interp::alpha(control, self.accumulator);
        let (pose, mut to_world, chain) = crate::interp::sora_drawn(&self.out_before, &self.out, &self.pose_before, &self.pose, &self.chain_before, &self.chain, alpha);
        let feet = to_world.translation;
        // (F-LOCO, INT-F) the body tilted by the actor's pitch about the capsule centre (both as drawn between the two steps)
        {
            let jumped = crate::interp::discontinuous(&self.out_before, &self.out) || alpha >= 1.0;
            let (pitch, centre) = if jumped { (self.out.pitch, self.out.capsule_center - self.out.position) } else { (self.out_before.pitch + (self.out.pitch - self.out_before.pitch) * alpha, (self.out_before.capsule_center - self.out_before.position).lerp(self.out.capsule_center - self.out.position, alpha)) };
            let yaw = crate::interp::yaw_drawn(&self.out_before, &self.out, alpha);
            pitch_about(&mut to_world, yaw, pitch, feet + centre);
        }
        // (INT) his drawn bones for the effects on him: the effects' own bones (the worn outfit's: boc.rs), his drawn place
        if alpha < 1.0 {
            let mut out_d = self.out.clone();
            (out_d.position, out_d.yaw) = (feet, crate::interp::yaw_drawn(&self.out_before, &self.out, alpha));
            let mut tap = kh_host::audio::PoseTap::default();
            let effect_pose = crate::boc::effect_pose(animator.skeleton(), &pose);
            tap.record(self.out.step, &out_d, animator.skeleton(), effect_pose.as_ref().unwrap_or(&pose), &to_world);
            tap.samples.clear();
            *self.int_drawn.borrow_mut() = Some((out_d, tap));
        }
        to_world.translation += coords::dir_er_to_kh(render_offset(control));
        // (ER party S7) Sora pitched on Goofy Bombardier's link pawn's socket (`Team::sora_mesh_pitch`, D5 handoff)
        #[cfg(feature = "party")]
        crate::party::team::pitch_sora(&mut to_world, self.party.core().and_then(|c| c.rt.team.sora_mesh_pitch()));
        // (IMPL_ER17) no Keyblade in his hand while he performs the body's action (it goes and comes with the blend)
        let opacity = self.weapon.opacity() * (1.0 - self.own_weight).clamp(0.0, 1.0) * kbdraw::visibility(); // (P5) hidden for the switch's appear delay
        let weapon = animator.skeleton().index_of("R_buki").and_then(|hand| pose.component.get(hand)).filter(|_| opacity > 0.0).map(|hand| kh_er_render::Weapon { to_world: weapon_socket().then(hand).then(&to_world), component: chain, opacity });
        // (ER90, XSI request 1) THE XS MOVES' FORM WEAPONS (kh-app xsi_weapons.rs, the same rule): while Giant Stamp, Chandelier
        // or Myriad plays, its form weapons on their hand bones and the Kingdom Key hidden meanwhile; `r.xsi_weapons = 0`: the
        // Kingdom Key as before
        let forms = match self.sora.attack().map(|a| a.kind) {
            Some(kind) if control.flag("r.xsi_weapons") != Some(false) => {
                use kh_core::abilities::xs;
                let name = match kind {
                    xs::finishers::GIANT_STAMP => "giant_stamp",
                    xs::chandelier::CHANDELIER => "chandelier",
                    xs::myriad::MYRIAD => "myriad",
                    _ => "",
                };
                if name.is_empty() {
                    Vec::new()
                } else {
                    // (XW) each form weapon posed by its own clip at the time of his clip of the move
                    let sequences = self.out.anim.name.sequences();
                    // (INT) at his clip's DRAWN time (between the steps, as his body)
                    let anim_time = crate::interp::clip_time(self.out_before.anim.name == self.out.anim.name, self.out_before.anim.time, self.out.anim.time, alpha);
                    let time = kh_er_render::xsiweapon::clip_time(name, sequences.first().copied(), anim_time);
                    kh_er_render::xsiweapon::place(name, &|b| animator.skeleton().index_of(b), &pose.component, &to_world, &weapon_socket(), time)
                }
            }
            _ => Vec::new(),
        };
        // (XW-b) Chandelier's chandelier (KH3's StylePawn w_so088) where the core says it stands, its clip at its time
        let mut forms = forms;
        if control.flag("r.xsi_weapons") != Some(false) {
            if let Some(p) = self.sora.xs_chandelier_pawn() {
                forms.extend(kh_er_render::xsiweapon::pawn_form("chandelier", p.ground + coords::dir_er_to_kh(render_offset(control)), p.yaw, self.int.chandelier_time(p.time, alpha) /* (INT) */));
            }
        }
        let skeleton = animator.skeleton();
        let index_of = |b: &str| skeleton.index_of(b);
        let hand_socket = weapon_socket();
        // (AUDIT-B W-DATA) a blend-space row of the held form's set: the weapons play their row of its LOGICAL name (KH3's weapon anim
        // instance; Twin Yo-Yos' MagicFire -> 4321, RunLoopB -> 4017), not the sample's sequence they have no clip of
        let weapon_anim = kh_core::combat::forms::weapon_logical(self.sora.combat().form.cmd, self.out.anim.name).or_else(|| self.out.anim.name.sequences().first().copied());
        let form_pose = kh_er_render::keyblade::FormPose { index_of: &index_of, component: &pose.component, to_world: &to_world, anim: weapon_anim, time: self.out.anim.time, hand_socket: &hand_socket };
        forms.extend(kbdraw::form_parts(&form_pose)); // (P5) test.kb_form
        let mut hand_hidden = !forms.is_empty();
        // (INT-F) THE FORM WEAPONS AND THE FINISHER PAWNS (P3 line 4, F-SHIELD 5, F-HAMMER 1-2, F-SHIP 1-2, F-STAFF 1, F-NANO 1-2,
        // SHOT-A 1, SHOT-B 1, SHOT-C 1): of the keyblade in his hand, in place of it
        let hand_kb = kbdraw::hand();
        let combat = self.sora.combat();
        let form = &combat.form;
        let magic = self.sora.magic();
        if magic.shot_weapon_form != 0 {
            // a shotlock's weapon form (hex, as the pack names forms: Flag Rampage "0F", Stellar Inception "18"); Atomic
            // Deluge's own equip set first
            let id = format!("{:02X}", magic.shot_weapon_form);
            if magic.shot_weapon_units.is_empty() {
                forms.extend(kh_er_render::keyblade::draw_form_weapon_parts(&hand_kb, &id, "", false, &form_pose));
            } else {
                forms.extend(kbdraw::unit_parts(&hand_kb, &id, magic.shot_weapon_units, &form_pose));
            }
            hand_hidden = true;
        } else if let Some(row) = form.row().filter(|r| r.weapon_form_value > 1 && !hand_hidden && form.equip != kh_core::combat::form::FormEquip::Keyblade /* (AUDIT-B W-CORE) */ && !self.form_gate.on() /* (FORMWEP-FIX) */) {
            // a form's weapons by its grow level; the hand keyblade is hidden when the pack knows the form's weapons (a form the
            // pack lacks keeps the keyblade)
            // (AUDIT-B W-CORE) exactly the units KH3's equipment holds (kh-core `FormEquip`): the Keyblade until StyleChange's
            // equip, then the transform units ALONE (w_so051 brings the hammer in itself), the form's own units from its End
            let grow = form.grow.max(1).to_string();
            let style_change = self.out.state == kh_core::StateId::Stylechange;
            let transform = form.equip == kh_core::combat::form::FormEquip::Transform;
            if !kh_er_render::keyblade::equipped_parts(&hand_kb, row.weapon_form, &grow, transform).is_empty() {
                if opacity > 0.0 || style_change {
                    forms.extend(kh_er_render::keyblade::draw_equipped_parts(&hand_kb, row.weapon_form, &grow, transform, &form_pose));
                }
                hand_hidden = true;
            }
        }
        // (AUDIT-A) (AUDIT-A PLACEMENT) the parts off their sockets: a weapon clip's ChangeAttachParent window hangs the part on
        // that bone of his (kh-core placement::part_parent: the launchers' FINISH_BZ, the staff's A416, Nano Arms' clips); a
        // part KH3 threw (ThrowWeapon: the raid's shield / pan, the flag) is drawn where its flight is (between the steps)
        let (seq_now, time_now) = (self.out.anim.name.sequences().first().copied().unwrap_or(""), self.out.anim.time);
        for model in forms.iter().filter_map(|f| f.model.strip_prefix(kh_er_render::keyblade::PREFIX).map(str::to_string)).collect::<Vec<_>>() {
            // (AUDIT-B W-HANDOVER) or the weapon clip's own window whoever equips it (a shotlock's equip set: placement::clip_parent)
            if let Some(b) = kh_core::combat::forms::placement::part_parent(form.cmd, &model, seq_now, time_now).or_else(|| kh_core::combat::forms::placement::clip_parent(&model, seq_now, time_now)).and_then(|name| {
                let (bone, loc, rot) = kh_core::combat::forms::placement::socket(name).unwrap_or((name, Vec3::ZERO, Vec3::ZERO));
                let socket = kh_anim::Trs { translation: loc, rotation: kh_anim::blend::Rotator { pitch: rot.x, yaw: rot.y, roll: rot.z }.quat(), scale: Vec3::ONE };
                index_of(bone).and_then(|i| pose.component.get(i)).map(|b| socket.then(b))
            }) {
                kh_er_render::keyblade::part_on_bone(&mut forms, &model, &b, &to_world);
            }
        }
        for t in &combat.thrown {
            let at = kh_anim::Trs { translation: t.position - t.velocity * (1.0 - alpha) * STEP + coords::dir_er_to_kh(render_offset(control)), rotation: t.rotation, scale: Vec3::ONE };
            kh_er_render::keyblade::part_thrown(&mut forms, t.model, at, t.clip().0, t.clip_time());
        }
        // the finisher / accompany pawns of the running finish, each with its own clip
        if let Some(row) = form.row() {
            let finish = self.sora.finish();
            let off = coords::dir_er_to_kh(render_offset(control));
            let yaw_d = crate::interp::yaw_drawn(&self.out_before, &self.out, alpha);
            let anim_time = crate::interp::clip_time(self.out_before.anim.name == self.out.anim.name, self.out_before.anim.time, self.out.anim.time, alpha);
            let wf = row.weapon_form;
            let pawn = |model: &str, ground: Vec3, yaw: f32, clip: Option<&str>, time: f32| kh_er_render::keyblade::form_pawn(&hand_kb, wf, model, ground, yaw, clip, time);
            // F-SHIELD: Pegasus with its Chariot / the Frying Pan, at Sora
            // (AUDIT-A SHIELD) placed by KH3's links (Sora rides the Chariot behind Pegasus), M_B425 as its heaviest sample
            for p in kh_core::combat::forms::shield::finisher_pawns_at(finish, form.cmd, &self.out.anim) {
                let (at, yaw) = kh_core::combat::forms::shield::place_on(feet + off, yaw_d, &p);
                forms.extend(pawn(p.model, at, yaw, p.clip, anim_time));
            }
            // F-HAMMER: Clock Drill's Clock where it was spawned
            for p in kh_core::combat::forms::hammer::finisher_pawns(finish, form.cmd) {
                forms.extend(pawn(p.model, p.at + off, p.yaw, Some(p.clip), anim_time));
            }
            // F-STAFF: Mirage Staff's tower
            if let Some(t) = kh_core::combat::forms::staff::tower(finish, combat) {
                forms.extend(pawn(t.model, t.position + off, t.yaw, Some(t.clip), t.time));
            }
            // F-SHIP: Storm Flag's Kraken, at Sora on its M_F420's clock
            for p in kh_core::combat::forms::ship::finisher_pawns(finish) {
                forms.extend(pawn(p.model, feet + off, yaw_d, p.clip, finish.map_or(0.0, |f| f.root_len)));
            }
            // F-NANO: FINISH_GM's gate (Sora and his weapons hidden meanwhile: below)
            if combat.nano.hide_sora {
                forms.clear();
                hand_hidden = true;
            }
            if let Some(g) = combat.nano.gate {
                forms.extend(pawn(g.model, g.position + off, g.yaw, Some(g.clip), g.time));
            }
        }
        // (AUDIT-B F-NANO, INT line 3) the flying Rocket Fists (TresProjGM_RocketPunch: TresCharMesh0 = w_so173, its AnimBP has
        // only the FullBody slot and nothing plays it: the reference pose), along their flight (yaw and pitch); they outlive
        // the form. Their effects (ead_so170_com0 group 1620) come through the effect seam.
        for r in combat.nano.rocket_views().filter(|r| r.stage == 1) {
            let off = coords::dir_er_to_kh(render_offset(control));
            let yaw = r.forward.y.atan2(r.forward.x);
            let pitch = r.forward.z.clamp(-1.0, 1.0).asin();
            if let Some(mut m) = kh_er_render::keyblade::model_at("w_so173", r.position + off, yaw.to_degrees()) {
                m.to_world.rotation = glam::Quat::from_rotation_z(yaw) * glam::Quat::from_rotation_y(-pitch) * glam::Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2);
                forms.push(m);
            }
        }
        kbdraw::apply_form_chains(&mut forms, alpha); // (KEYCHAIN-DUAL) each Keyblade part with its own keychain (`step_chain`)
        let weapon = if hand_hidden { None } else { weapon };
        // (F-NANO 1, SHOT-C rush, INT-F) his body hidden while FINISH_GM's gate holds him or Noise Flux dashes
        let body_hidden = combat.nano.hide_sora || magic.shot_body_hidden;
        kh_er_render::submit(kh_er_render::FrameInput {
            camera: kh_er_render::Camera { right: camera.right, up: camera.up, forward: camera.forward, position: camera.position, fov: camera.fov, aspect: camera.aspect, near: camera.near, far: camera.far },
            origin: self.world.frame.origin,
            clock: self.clock - (1.0 - alpha) * STEP, // (INT) the drawn time (between the steps, as his pose)
            body: (!body_hidden).then_some(kh_er_render::Body { component_to_world: to_world, local: pose.local, component: pose.component }),
            weapon,
            forms,
        });
    }

    /// (INT) A DISCONTINUITY of his place between the two latest steps (a teleport, a reseat, the stuck fix, a load, a warp):
    /// never blended: he, the camera and the effects are drawn at the latest step until the next one (the friends and the
    /// props by their own jumps: `PosedMember::posed`, `interp::Props`). Before the view of the frame is posted.
    fn int_snap_check(&mut self, control: &Control) {
        let on = crate::interp::on(control);
        if self.int.told != Some(on) {
            self.int.told = Some(on);
            erlog!("interp: {}", if on { "r.interp ON (default): Sora, the camera, the friends, the effects, the team props, the carried enemies and the HUD's world marks are drawn between the two latest fixed steps (r.interp = 0: the latest step)" } else { "r.interp = 0: everything of ours is drawn at the latest fixed step" });
        }
        if crate::interp::discontinuous(&self.out_before, &self.out) {
            if self.int.snaps < 20 || self.int.snaps.is_multiple_of(100) {
                erlog!("interp: a jump of {:.0} cm / {:.0} deg between two steps (teleport / reseat / warp / a turn set at once): drawn at the latest step, not blended (snap {})", self.out_before.position.distance(self.out.position), ((self.out.yaw - self.out_before.yaw + 540.0).rem_euclid(360.0) - 180.0).abs(), self.int.snaps + 1);
            }
            self.out_before = self.out.clone();
            self.pose_before = self.pose.clone();
            self.chain_before = self.chain.clone();
            self.int.snap();
            self.cam.snap();
            self.fx.snap();
        }
    }

    /// (INT) The effects' draw of the frame between the steps, those on him on his sockets AS DRAWN (`submit`'s pose).
    fn int_draw_fx(&mut self, control: &Control) {
        let alpha = crate::interp::alpha(control, self.accumulator);
        let drawn = self.int_drawn.borrow_mut().take();
        let targets = &self.targets;
        let target = |id: kh_core::TargetId| targets.get(id).map(|e| e.pos + Vec3::Z * e.half_height);
        let scene = drawn.as_ref().map(|(out, tap)| kh_host::vfx::Scene { out, sora: &self.sora, tap, target: &target, ground: None, eye: Vec3::ZERO });
        self.fx.draw_drawn(&self.world.frame, alpha, scene.as_ref());
    }

    /// (INT) The leader's and the members' feet as DRAWN this frame (conv.rs: the bubbles' faces; the conversation's camera
    /// keeps the steps' places).
    fn int_drawn_feet(&self, control: &Control) -> (Option<Vec3>, Vec<(kh_host::party::PartyChara, Vec3)>) {
        let alpha = crate::interp::alpha(control, self.accumulator);
        let sora = if !crate::interp::discontinuous(&self.out_before, &self.out) { self.out_before.position.lerp(self.out.position, alpha) } else { self.out.position };
        #[cfg(feature = "party")]
        if let Some(core) = self.party.core() {
            use kh_host::party::PartyChara;
            let feet = |c: PartyChara| if c == PartyChara::Sora { Some(sora) } else { core.poser.pose(kh_partyhost::poser::draw_id(c), alpha).map(|p| p.component_to_world.translation) };
            let leader = self.friend_lead().map(|_| self.lead.chara()).unwrap_or(PartyChara::Sora);
            let members = core.rt.current.iter().filter_map(|v| core.rt.chara_of_id(v.id)).filter(|c| *c != leader).filter_map(|c| Some((c, feet(c)?))).collect();
            return (feet(leader), members);
        }
        (Some(sora), Vec::new())
    }

    /// Puts the Elden Ring player where the latest step left Sora.
    fn write(&mut self, player: &mut PlayerIns) {
        // (ER party S2) the body's leader (Sora: his latest step, as before)
        let (at, yaw) = (self.lead_pos(), self.lead_yaw());
        self.write_at(player, at, yaw);
    }

    /// (ER party S2) Puts the Elden Ring player at these feet (KH3) and yaw.
    fn write_at(&mut self, player: &mut PlayerIns, at: Vec3, yaw: f32) {
        let feet = self.world.frame.pos_kh_to_er(at);
        let q = coords::quat_er_from_yaw(yaw);
        let physics = player.chr_ins.modules.physics.as_mut();
        physics.position = game::hp(feet);
        physics.orientation = game::quaternion(q);
        physics.interpolated_orientation = game::quaternion(q);
        self.written = Some(feet);
    }

    /// `ChrIns_PostPhysics`.
    pub fn post_physics(&mut self, player: &mut PlayerIns, control: &Control, tick: bool) {
        if self.follow.is_none() && self.ladder_go.body_free() {
            // (WALLS load fix 2026-10-23, the user's log kh-er_load_issues.log 155.37 s) THE GAME'S OWN PLACEMENT IS NEVER
            // OVERWRITTEN: the wait for the ground let the leader go on the very frame the game put its body at the loaded place;
            // this write put the body back on the stale seat ("position jumped by (73.161, 47.996, 501.686)"), the origin shift
            // that followed left it 2.3 m under the floor, and no cast of any layer found ground there again (falls, 30 s holds).
            // A jump of more than 4 m in the physics step is the game's: the KH3 frame follows it (as `pre_physics` does between
            // frames) and, unless it is an origin shift, the next step waits for the ground there.
            let now = game::v3(&player.chr_ins.modules.physics.position);
            if let Some(written) = self.written {
                let moved = now - written;
                if let Some(shift) = crate::groundwait::game_jump(written, now) {
                    self.world.frame.rebase(moved);
                    self.rebased_now = true;
                    self.targets.rebased();
                    self.relocated_post |= !shift;
                    erlog!("sora: (WALLS) the game moved its body by ({:.3}, {:.3}, {:.3}) in the physics step: {}; the KH3 frame follows it (not written over)", moved.x, moved.y, moved.z, if shift { "the physics origin shifted" } else { "ITS OWN PLACEMENT (a load, a warp): the leader waits for the ground THERE" });
                }
            }
            self.write(player);
        }
        #[cfg(feature = "party")]
        crate::allybody::post_physics(self.world.frame); // (ER party S3) the ally bodies written again after the physics
        #[cfg(feature = "party")]
        crate::mimic_boss::write_bodies(self.world.frame); // (MIMIC-BOSS) the Mimic Tear boss's body again after the physics
        self.assert_flags(player, control);
        self.targets.puppets.reassert_at(crate::interp::alpha(control, self.accumulator)); // (INT) the enemies we carry, between the steps as he is

        if control.flag("draw.sora") != Some(false) {
            self.draw();
        }
        #[cfg(feature = "party")]
        self.party.draw(control, &self.world.frame); // (ER party S1, testing aid `party.draw = 1`) the friends' capsules, the trail
        // (measuring aid) A green pole above his origin, drawn by the game itself: a world-fixed
        // mark to compare our own drawing against while the camera turns.
        // With r.dx / r.dz (metres) the pole and the drawn model stand that far beside him: the
        // camera orbits the player, so a mark away from the pivot crosses the screen when it turns.
        if control.on("draw.pole") {
            if let Some(mut draw) = Draw::get() {
                let base = self.feet_er() + render_offset(control);
                draw.colour([0.0, 1.0, 0.0, 1.0]);
                for k in 0..25 {
                    let side = Vec3::new((k % 5) as f32 - 2.0, 0.0, (k / 5) as f32 - 2.0) * 0.006;
                    draw.line(base + side + Vec3::Y * 1.9, base + side + Vec3::Y * 4.5);
                }
            }
        }
        self.cut_frame(control); // (IMPL_ER16) a cutscene's figure of the player, shown as him
        self.own_frame(player, control); // (IMPL_ER16) the game's own actions of the body, shown on him
        {
            // (IMPL_ER60) the blood on him and the Keyblade (this frame's hits, his HP, the body's wetness, the save point)
            let s = self.sora.status();
            crate::blood::frame(control, player, &self.targets, s.hp, s.max_hp(), self.clock, tick);
            // (ER party S5) the friends' blood by the same rules (the save point washes every member and his weapon)
            #[cfg(feature = "party")]
            if let Some(core) = self.party.core() {
                let members: Vec<_> = core.rt.current.iter().map(|v| (kh_partyhost::runtime::party_chara(v.chara), v.chara, v.hp, v.hp_max, v.position)).collect();
                crate::blood::party_frame(control, &self.targets, &members, self.clock, tick);
            }
        }
        // The real model (kh-er-render draws it in the game's frame).
        self.submit(control);
        #[cfg(feature = "party")]
        self.party_draw(control); // (ER party S4) the friends handed to the renderer
        // (CUTSCENE-ANIM) in a real-time cutscene Mimic Sora is hidden as the AI friends are (partydraw U6): he is no figure of the
        // scene's and would stand idle in its shots; `cut.mimic = 1`: drawn as in play
        #[cfg(feature = "party")]
        if self.cutscene && control.flag("cut.mimic") != Some(true) {
            kh_er_render::mimic::clear_mimic();
        } else {
            crate::spirit_mimic::draw(control, crate::interp::alpha(control, self.accumulator)); // (MIMIC SORA) to the renderer, the Mimic Tear's look
        }
        self.int_draw_fx(control); // (fx) the effects' draw list of the frame (INT: between the steps, on him as drawn)
        if tick {
            if let Some(trace) = self.trace.as_mut() {
                let _ = trace.flush();
            }
            let o = &self.out;
            let er = self.feet_er();
            erlog!(
                "sora: step {} state {:?}/{} on_floor {} kh ({:.1}, {:.1}, {:.1}) yaw {:.1} vel ({:.0}, {:.0}, {:.0}) anim {:?} | er ({:.3}, {:.3}, {:.3}) | {} steps, {} casts (+ {} on the floor layer, {} contacts taken), {:.2} ms/frame, pushed {:.3} m, pad {}",
                o.step, o.state, o.sub_step, o.on_floor, o.position.x, o.position.y, o.position.z, o.yaw, o.velocity.x, o.velocity.y, o.velocity.z, o.anim.name,
                er.x, er.y, er.z, self.steps_in_tick, self.casts_in_tick, self.world.casts2.replace(0), self.world.taken2.replace(0), self.work.as_secs_f32() * 1000.0 / 60.0, self.pushed,
                if self.pad_seen { "connected" } else { "none" },
            );
            if self.pad_active > 0 {
                erlog!("pad: REAL PAD INPUT in {} frame(s) of this second (somebody is playing)", self.pad_active);
                self.pad_active = 0;
            }
            {
                let status = self.sora.status();
                let lock = self.sora.combat().lockon;
                let hostiles = self.targets.hostiles_by_distance();
                // SAFETY: the player's game data, alive with the player.
                let runes = unsafe { player.player_game_data.as_ref() }.rune_count;
                erlog!(
                    "combat: Sora HP {}/{} MP {} focus {} | targets {} read, {} hostile alive (nearest {}), engaged {} | lock {} (current {}) | hits on him {} | no-damage frames {} | keyboard frames {} | Tarnished HP {}/{} runes {runes} | Elden Ring saw the pad through our hook {} time(s) of {} call(s), prompt {}, menu {}, paused {}",
                    status.hp, status.max_hp(), status.mp, status.focus, self.targets.list.len(), hostiles.len(),
                    hostiles.first().map_or("none".to_string(), |(d, e)| format!("target {} c{:04} at {:.1} m hp {}/{}", e.id.0, e.npc_id, d / coords::CM_PER_M, e.hp, e.hp_max)),
                    kh_core::Targets::engaged_count(&self.targets),
                    if lock.locked { "LOCKED" } else { "soft" }, lock.current.0, self.hits_in, self.invulnerable_frames, self.key_frames,
                    player.chr_ins.modules.data.hp, player.chr_ins.modules.data.max_hp,
                    inputs::HOOK_CALLS.swap(0, std::sync::atomic::Ordering::Relaxed), inputs::ALL_CALLS.swap(0, std::sync::atomic::Ordering::Relaxed), self.prompt, self.ui.menu, self.paused,
                );

                // (prize) (savepoint) their once-a-second lines
                if let Some(line) = self.prize.summary(control, &self.sora, runes) {
                    erlog!("{line}");
                }
                if self.loot.active() {
                    erlog!("{}", self.loot.summary()); // (loot)
                }
                if let Some(line) = self.savepoint.summary(er) {
                    erlog!("{line}");
                }
                let t = &mut self.targets;
                erlog!(
                    "damage: route {} | bullets spawned {} refused {} | landed {} missed {} | rows {} | puppets flying {} (taken so far {}) | hit events this frame {}",
                    if t.route.mode == crate::damage::Mode::Bullet && t.bullets.available() { "BULLET (the game's pipeline)" } else { "HP WRITE (stopgap)" },
                    t.bullets.spawned, t.bullets.failed, t.landed, t.missed, t.bullets.describe(), t.puppets.count(), t.puppets.launched, t.events.len(),
                );
                self.hits_in = 0;
                self.invulnerable_frames = 0;
                self.key_frames = 0;
            }
            // (discovery; a value that is in the file when Sora starts is NOT carried out, only a
            // change is: a hot reload once ran a scan under a player that way)
            // `w.scan = n`: a small sphere is cast straight down through his place (from 3 m above
            // his feet, 15 m) with every layer number 0..127 in the filter word's low byte; the
            // layers are listed by what they hit first (height, owner type).
            // `w.proxy = n`: words that look like a filter word (0x020000LL, as the camera's
            // 0x02000058) in the player's two character proxies (CSChrPhysicsModule +0x98 / +0xa0,
            // private in the SDK) and one pointer level below; every read is checked first.
            let proxy = control.str("w.proxy").map(str::to_string);
            let proxy_now = self.proxied.is_some() && Some(&proxy) != self.proxied.as_ref() && proxy.is_some();
            self.proxied = Some(proxy);
            let scan = control.str("w.scan").map(str::to_string);
            let scan_now = self.scanned.is_some() && Some(&scan) != self.scanned.as_ref() && scan.is_some();
            self.scanned = Some(scan);
            // (REMOVED: the scan of the proxies' memory HUNG the game, twice, reproducibly: one thread spinning,
            // window not responding. Do not read the Havok character proxies' memory from a frame task.)
            let _ = proxy_now;
            // (IMPL_ER18) `w.fscan = n[,len[,radius]]`: what every layer meets first ahead of him (wallscan.rs); `w.hits = 1`: the
            // non-floor bodies his own casts met this second
            let fscan = control.str("w.fscan").map(str::to_string);
            let fscan_now = self.fscanned.is_some() && Some(&fscan) != self.fscanned.as_ref() && fscan.is_some();
            if fscan_now {
                let v: Vec<f32> = fscan.as_deref().unwrap_or("").split(',').filter_map(|p| p.trim().parse().ok()).collect();
                let dir = coords::quat_er_from_yaw(self.out.yaw) * Vec3::NEG_Z;
                crate::wallscan::forward_scan(self.feet_er() + Vec3::Y * v.get(3).copied().unwrap_or(1.0), dir, v.get(1).copied().unwrap_or(4.0), v.get(2).copied().unwrap_or(0.15), self.world.filter);
            }
            if fscan_now && control.str("w.grid").is_some() {
                // w.grid = yaw (KH3 degrees; 999 = his own), len, half, step, height, radius
                let g: Vec<f32> = control.str("w.grid").unwrap_or("").split(',').filter_map(|p| p.trim().parse().ok()).collect();
                let yaw = g.first().copied().filter(|y| *y < 900.0).unwrap_or(self.out.yaw);
                let dir = coords::quat_er_from_yaw(yaw) * Vec3::NEG_Z;
                crate::wallscan::grid(self.feet_er() + Vec3::Y * g.get(4).copied().unwrap_or(1.0), dir, g.get(1).copied().unwrap_or(4.0), g.get(2).copied().unwrap_or(2.0), g.get(3).copied().unwrap_or(0.1), g.get(5).copied().unwrap_or(0.02), self.world.filter);
                let physics = &player.chr_ins.modules.physics;
                erlog!("grid: the game's own player: hit_radius {:.3} hit_height {:.3} chr_hit_radius {:.3} chr_hit_height {:.3}; Sora's capsule radius {:.1} cm half height {:.1} cm", physics.hit_radius, physics.hit_height, physics.chr_hit_radius, physics.chr_hit_height, SORA_PARAMS.capsule_radius, SORA_PARAMS.capsule_half_height);
            }
            self.fscanned = Some(fscan);
            self.world.log_walls.set(control.on("w.hits"));
            crate::wallscan::log(&std::mem::take(&mut *self.world.walls.borrow_mut()), self.feet_er());
            if scan_now {
                let from = self.feet_er() + Vec3::Y * 3.0;
                let mut groups: std::collections::BTreeMap<String, Vec<u32>> = std::collections::BTreeMap::new();
                let mut hits = Vec::new();
                for layer in 0..128u32 {
                    crate::havok::cast_sphere(from, Vec3::NEG_Y * 15.0, 0.05, (self.world.filter & !0xFF) | layer, false, &mut hits);
                    hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
                    let what = match hits.first() {
                        Some(h) => format!("y {:+.2} owner {} ({} contacts, lowest y {:+.2})", h.position.y - self.feet_er().y, h.owner, hits.len(), hits.last().map_or(0.0, |l| l.position.y - self.feet_er().y)),
                        None => "nothing".to_string(),
                    };
                    groups.entry(what).or_default().push(layer);
                }
                erlog!("scan: layers by what a cast down through his place hits first (heights relative to his feet):");
                for (what, layers) in groups {
                    erlog!("scan:   {what}: {}", layers.iter().map(|l| format!("{l:#x}")).collect::<Vec<_>>().join(" "));
                }
            }
            if control.on("w.log") {
                let seen = std::mem::take(&mut *self.world.seen.borrow_mut());
                let physics = &player.chr_ins.modules.physics;
                erlog!(
                    "world: cast filter {:#x}; the game's own ground under the player: hit_material {} (override {}), solid ground {}; contacts by owner type / tag words 0x28 0x2c 0x30 0x38: {}",
                    self.world.filter,
                    physics.material_info.hit_material,
                    player.chr_ins.hit_material_override,
                    physics.standing_on_solid_ground,
                    seen.iter().map(|((o, t), n)| format!("{o}/{:x} {:x} {:x} {:x}: {n}", t[0], t[1], t[2], t[3])).collect::<Vec<_>>().join(", ")
                );
            } else {
                self.world.seen.borrow_mut().clear();
            }
            if self.atk.sec_steps > 0 {
                erlog!("attack: this second {} step(s) in an attack state, {} of them off the floor", self.atk.sec_steps, self.atk.sec_off);
                (self.atk.sec_steps, self.atk.sec_off) = (0, 0);
            }
            self.cam.tick();
            self.steps_in_tick = 0;
            self.casts_in_tick = 0;
            self.pushed = 0.0;
            self.work = std::time::Duration::ZERO;
        }
    }

    /// Sora as debug lines: every bone to its parent, the collision capsule, the facing arrow.
    fn draw(&self) {
        let Some(mut draw) = Draw::get() else { return };
        let frame = &self.world.frame;
        let o = &self.out;
        if let Some(animator) = self.animator.as_ref() {
            let to_world = component_to_world(o);
            let world: Vec<Vec3> = self.pose.component.iter().map(|t| frame.pos_kh_to_er(to_world.transform_point(t.translation))).collect();
            draw.colour(game::WHITE);
            for (i, bone) in animator.skeleton().bones.iter().enumerate() {
                if let (Some(parent), Some(&a)) = (bone.parent, world.get(i)) {
                    if let Some(&b) = world.get(parent) {
                        if a.is_finite() && b.is_finite() && (a - b).length() < 2.0 {
                            draw.line(a, b);
                        }
                    }
                }
            }
        }
        // The capsule (radius 60, half height 89 about `capsule_center`), green on a floor, orange in the air.
        draw.colour(if o.on_floor { game::GREEN } else { game::ORANGE });
        draw.capsule(frame.pos_kh_to_er(o.capsule_center), CAPSULE_RADIUS / coords::CM_PER_M, CAPSULE_HALF_HEIGHT / coords::CM_PER_M);
        // The facing: an arrow at chest height.
        let chest = frame.pos_kh_to_er(o.position + Vec3::Z * 110.0);
        let facing = coords::facing_er(o.yaw);
        let tip = chest + facing * 1.0;
        let side = facing.cross(Vec3::Y) * 0.15;
        draw.colour(game::CYAN);
        draw.line(chest, tip);
        draw.line(tip, tip - facing * 0.3 + side);
        draw.line(tip, tip - facing * 0.3 - side);
    }
}

/// (measuring aid) `r.dx`, `r.dz`: the drawn model stands this far (m, Elden Ring axes) beside the player.
/// (ANIMS, LADDER2) The player's ladder module written free: its state -1 (as off a ladder) and its handle emptied, so the
/// game offers its prompts again (exitnet.rs's repair; the let-go's last resort).
fn free_ladder_module(player: &mut PlayerIns) {
    let m = player.chr_ins.modules.ladder.as_mut();
    // SAFETY: two plain fields of the player's live ladder module (game thread; game::player_mut checked the modules): the
    // state byte (-1 = none) and the handle's selector (u32::MAX = empty)
    unsafe {
        std::ptr::write(&mut m.state as *mut _ as *mut i8, -1);
        std::ptr::write(&mut m.ladder_handle as *mut _ as *mut u32, u32::MAX);
    }
}

/// (IMPL_ER18) The body's ladder state as a number (the SDK's `LadderState`, read as its byte: -1 = not on a ladder).
fn ladder_state(player: &PlayerIns) -> i8 {
    // SAFETY: one byte of a live module of the player (game::player_mut checked the modules).
    unsafe { std::ptr::read(&player.chr_ins.modules.ladder.state as *const _ as *const i8) }
}

/// (GRABS) KH3 Sora's caught clips' lengths (his AnimSet rows CatchStart / CatchLoop / CatchEnd in the pack).
fn sora_catch_lengths() -> crate::grabs::Lengths {
    crate::grabs::sora_lengths()
}

/// (GRABS) The caught sequence's clip in Sora's body slot (the ladder leap's route: `Show::Clip`).
fn catch_show(s: crate::grabs::Shown) -> Option<crate::ladder::Show> {
    crate::grabs::catch_request(s).map(|r| crate::ladder::Show::Clip { anim: r.name, time: r.time, looping: r.looping, play: 0x0100_0000 + s.play })
}

/// (GRABS) THE ABDUCTION'S WARP: the two flags the Abductor's m14 events set, then what the game's own EMEVD `WarpPlayer`
/// writes into GameMan (the same three fields) for its destination. No death: no rune is
/// dropped, no game over; the load that follows makes the party anew round the leader at the arrival point.
fn abduct_warp(a: &crate::grabs::Abduction) -> bool {
    use fromsoftware_shared::FromStatic;
    let flags: Vec<String> = a.flags.iter().map(|f| format!("{f}{}", if crate::loot::set_event_flag(*f) { "" } else { " (NOT WRITTEN)" })).collect();
    // SAFETY: game thread; the singleton lives as long as the game; three plain fields of it (the SDK's layout)
    let Ok(gm) = (unsafe { eldenring::cs::GameMan::instance_mut() }) else {
        erlog!("!!! grab: (GRABS) the Abductor's warp: no GameMan, NOT warped (flags {})", flags.join(", "));
        return false;
    };
    gm.initial_area_entity_id = a.point;
    gm.move_map_target = eldenring::cs::BlockId::from_parts(a.area, a.block, a.region, a.index);
    gm.warp_requested = true;
    erlog!("grab: (GRABS) THE ABDUCTOR LET GO: flags {} set; THE GAME'S WARP to m{:02}_{:02}_{:02}_{:02} point {} asked (Volcano Manor; no death, no rune lost)", flags.join(", "), a.area, a.block, a.region, a.index, a.point);
    true
}

/// (IMPL_ER57) The body's throw node: its state (SDK `ThrowNodeState` as a number; `None`: the module or node is not
/// readable) and the handle bits of the other party of the paired animation (`CSPairAnimNode::counter_party`). Every read
/// goes through `probe` (a bad pointer gives "not read", never a fault).
fn throw_state(player: &PlayerIns) -> (Option<u32>, Option<u64>) {
    use eldenring::cs::{CSChrThrowModule, CSPairAnimNode, CSThrowNode};
    let module = crate::word!(player.chr_ins.modules.throw);
    if !game::plausible(module) {
        return (None, None);
    }
    let Some(node) = crate::probe::word(module + std::mem::offset_of!(CSChrThrowModule, throw_node)).filter(|n| game::plausible(*n)) else { return (None, None) };
    let state = crate::probe::u32_at(node + std::mem::offset_of!(CSThrowNode, throw_state));
    let by = crate::probe::word(node + std::mem::offset_of!(CSThrowNode, pair_anim_node) + std::mem::offset_of!(CSPairAnimNode, counter_party)).map(|w| w as u64);
    (state, by)
}

/// (IMPL_ER57) A second layer that meets the sea surface (MEASURED, IMPL_ER3_STATE.md: 0x57 0x58 0x5a 0x5b 0x66 0x68).
const WATER_LAYER_B: u32 = (crate::havok::FILTER_CAMERA & !0xFF) | 0x5a;

/// (WALLS) The water rule's solid layers from before the KH characters took the Tarnished's layer (0x56 and the old floor layer 0x5c,
/// MEASURED in IMPL_ER3 / ER57 not to meet the sea surface): kept among the solids, so a body only they met (layers 0x3b 0x40 0x45
/// 0x4a: both water layers meet them, the Tarnished's layer 2 does not) is never taken for a water surface.
const WATER_SOLID_OLD: [u32; 2] = [crate::havok::FILTER_SORA, (crate::havok::FILTER_SORA & !0xFF) | 0x5c];

/// (IMPL_ER57) The heights (Elden Ring Y, m) of what a 5 cm sphere meets straight down from `from` over `down` m with one
/// filter word (all contacts; with `flat`, only surfaces facing up).
fn flat_heights(from: Vec3, down: f32, filter: u32, flat: bool) -> Vec<f32> {
    let mut hits = Vec::new();
    crate::havok::cast_sphere(from, Vec3::NEG_Y * down, 0.05, filter, false, &mut hits);
    hits.iter().filter(|h| h.position.is_finite() && (!flat || h.normal.y > 0.9)).map(|h| h.position.y).collect()
}

fn render_offset(control: &Control) -> Vec3 {
    Vec3::new(control.f32("r.dx").unwrap_or(0.0), 0.0, control.f32("r.dz").unwrap_or(0.0))
}

/// The mesh component in the KH3 world: at the actor origin, turned by the yaw minus 90 degrees; (F-LOCO, INT-F) tilted by
/// the actor's pitch about the capsule centre (kh-host rig.rs, the effects' rule too).
fn component_to_world(out: &Output) -> Trs {
    kh_host::rig::component_to_world(out)
}

/// (F-LOCO, INT-F) kh-host rig.rs `pitch_about`.
fn pitch_about(t: &mut Trs, yaw: f32, pitch: f32, centre: Vec3) {
    kh_host::rig::pitch_about(t, yaw, pitch, centre);
}
