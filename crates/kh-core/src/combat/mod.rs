//! OWNER: the combat implementer. Melee combat with the Kingdom Key in base form:
//! docs/sora-combat-spec.md (first pass) with the second-pass strands that replace parts of it
//! (`build/sora_tmp/combat/P2_C_section.md` hits, `P2_D_section.md` targeting; they govern where
//! they contradict the first pass). Sheets `sheets/port/sora-combat-*` -> `tables.rs`.
//!
//! | file | what | written from |
//! |---|---|---|
//! | `tables.rs` | the generated tables and constants | sheets |
//! | `decide.rs` | the combo decision (`Decide` 0x14134ACA0), MaxCombo, Finishers | spec 4 |
//! | `targeting.rs` | the controller's `UpdateLockon`: soft target, pin, RB lock, RT switch | strand D |
//! | `hit.rs` | hit windows, sweep, hit record, damage call, effect / sound events | strand C |
//! | `../states/attack.rs` | `TresPlayerState_Attack` (id 12), `CanAttack`, `TryAttack` | spec 3, 5, 6 |
//!
//! NOT implemented (each is marked TODO with its strand where it would go): Sora being hit
//! (strand E), Second Form and the situation commands (strand F), launch attacks' request
//! (the jump button during a combo, spec 3.4: the decision rule is in `decide.rs`), guard /
//! risk-dodge counters, clashes and wall hits, the chase attack, swimming.

pub mod badstat;
pub mod camera;
pub mod classify;
pub mod finish;
pub mod decide;
pub mod form;
pub mod freeze_look; // (FREEZE-FX) KH3's frozen look on any victim
/// (weapons P3) The form packages' hooks into the form framework (form.rs).
pub mod forms;
/// (party stream D) The friend gauge, team-command offers, unison roll.
pub mod friend;
pub mod guard;
pub mod hit;
/// (F-SHAPES) The CONVEX attack shape (a hull under the shape's transform) and a window's several shapes.
pub mod hull;
pub mod push;
pub mod situation;
/// (MIMIC-STANCE) A KH character's stance break (one route for Sora's core and kh-party's pawns).
pub mod stance;
pub mod style;
pub mod tables;
pub mod targeting;

pub use decide::{decide, ComboAbilities, DecideIn, Decision};
pub use hit::{AttackCtx, Shot, Window};
pub use targeting::{CursorKind, Lockon};

use glam::Vec3;

/// The pawn's and the controller's melee fields (the running Attack state has its own).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PawnCombat {
    /// pawn+0xE10: the last attack kind played.
    pub last_kind: u8,
    /// pawn+0xE14: finishers played in this combo.
    pub finish_count: i32,
    /// pawn+0xE48 bit 5, "finish end": the combo is over, no further attack request is taken.
    /// Cleared with the rest of the combo memory when a chain ends (pawn slot 981, strand A 6.3).
    pub finish_end: bool,
    /// pawn+0xE34: the command cool-down (`MyCommandCoolDownTime` 0.6 s after a whiffed air attack
    /// without a follow-up); +0xE38 / +0xE3C: the air-slide (0.6 s) and air-guard (0.3333 s)
    /// cool-downs of the same case (for the actions implementer's gates).
    pub command_cooldown: f32,
    pub air_slide_cooldown: f32,
    pub air_guard_cooldown: f32,
    /// The running attack landed a hit (state+0x13C bit 2 / +0x234 bit 13; set by the on-hit
    /// handler in hit.rs, read by the state).
    pub swing_hit: bool,
    /// What an ending attack hands the next attack of the chain: its counters +0x12C / +0x138
    /// (strand A 6.3, 6.4).
    pub pass_on: (i32, i32),
    /// pawn+0xE50 / +0xE54: the launch stocks (Slash Upper, Air Roll Beat).
    pub stocks: (i32, i32),
    /// pawn+0xE1C / +0xE20: the hit counters an ended attack leaves (`End`).
    pub carry: (i32, i32),
    /// request interface +0x9C8: the combo count of the latest request.
    pub combo: i32,
    /// pawn+0xE24 / +0xE28: hits landed, "a hit landed" (strand C 8.5.1).
    pub hit_count: i32,
    pub hit_flag: bool,
    /// (F-POLISH) The latest contacts of `hit_count` with their victims (combat/hit.rs `Contact`), for the states' on-contact
    /// handlers.
    pub contacts: Vec<hit::Contact>,
    /// pawn+0x6C8: the last impact point.
    pub last_impact: Vec3,
    /// controller+0xB40 ...: the formchange gauge (style.rs; strand F 10.2).
    pub style: style::Style,
    /// The controller's lock-on state.
    pub lockon: Lockon,
    /// The active hit windows (the keyblade's attack-collision component).
    pub windows: Vec<Window>,
    /// What the running Attack state's pre-processor gives an activation.
    pub attack_ctx: AttackCtx,
    /// The latest attack kinds played, oldest first (at most 16): for tests and the debug text.
    pub history: Vec<u8>,
    /// Hits on Sora the host delivered since the last step (`Sora::take_hit`).
    pub incoming: Vec<crate::states::HitOnPlayer>,
    // ---- second combat implementer (2026-10-11)
    /// The hook with the actions implementer's guard / dodge states (guard.rs).
    pub guard: guard::GuardSeam,
    /// The situation-command queue (situation.rs; strand F 11).
    pub situation: situation::Queue,
    /// Second Form's bookkeeping (form.rs; strand F 10.3 - 10.7).
    pub form: form::Form,
    /// controller+0x8F4: no flinch while > 0 (0.1 s after a command was executed; strand E 9.6).
    pub reaction_block: f32,
    /// The recoil record of the running attack (Attack state +0x264 bit 0, +0x268, +0x270),
    /// written by the on-contact handler (hit.rs), consumed by the state's Tick (slot 129).
    pub recoil: Option<(crate::host::ReactionResult, crate::host::TargetId, Vec3)>,
    /// The per-object-type time dilation (host/time.rs).
    pub time: crate::host::time::TimeTable,
    /// pawn+0x638: no HP loss while > 0 (1.0 s after the auto-revive; strand E 9.6, 9.8).
    pub no_damage_timer: f32,
    /// (SHOT-C) pawn+0xE44: INVINCIBLE while > 0 (setter pawn vt+0x1E48 0x141112510; vt+0x1E50 0x14110E840 = "> 0" is
    /// the first test of the player's IsInvincible vt+0x2060 0x141435A90, which ShouldTakeDamage 0x1405BA540 asks; counted
    /// down by the pawn tick 0x141449CD0, clamped at 0). Set to 1.0 at the end of shotlocks 40 / 41 / 42 and Double Form.
    pub invincible_timer: f32,
    /// (KILLPLANE) A KILL PLANE'S DEATH asked by the host (`Sora::kill_unconditional`): where it came from. Taken at the
    /// start of the next step's hits, before them: HP 0 and the Die state, whatever would save him (`damage::kill_unconditional`).
    pub kill_plane: Option<Vec3>,
    /// (KILLPLANE) He lies in a kill plane's death: while in the Die state his HP stays 0 (no Cure, Recover or item lifts it);
    /// let go once the host stood him up (out of the Die state with HP again: a respawn).
    pub kill_plane_held: bool,
    /// Wall hits, clashes and body-push ticks so far (for tests and the debug text).
    pub wall_hits: u32,
    pub clashes: u32,
    pub pushed: Vec3,
    /// The engine's xorshift128 is a process global; here a per-pawn stream (the bad-status roll
    /// of strand C 8.2.5 step 18; never drawn for rates of 0 or 100).
    pub rng: u32,
    /// The map-hit-reflect window of the active primitives is open (prim+0x987 bit 1:
    /// `TresAnimNotifyState_AttackEnableMapHitReflect`; spec 8.2.5 step 9, 8.5.1).
    pub map_reflect: bool,
    /// Swings repelled by the level so far (for tests and the debug text).
    pub map_recoils: u32,
    /// Final Blow (REVENGE_EX; spec 5.6.4): the pawn a guard counter hit (Attack state +0x278
    /// `m_pRevengePawn`), "chase armed" (state+0x234 bit 19), the flag an ending attack hands
    /// the next one of the chain, and the chases started so far.
    pub revenge: crate::host::TargetId,
    pub chase_armed: bool,
    pub chase_next: bool,
    pub chases: u32,
    /// The bad status on him (badstat.rs; spec 9.9): pawn+0x85D / +0x860.
    pub bad: badstat::BadStatus,
    /// A shot the running attack asked for (the dive's "DOWNIMPACT"): spawned by the state's tick.
    pub pending_shot: Option<&'static str>,
    /// The attack shots alive (hit.rs `Shot`: actors of `p_ex001_ProjectileSet` that an attack
    /// spawned; spec 8.8).
    pub shots: Vec<hit::Shot>,
    /// Shots spawned so far (for tests and the debug text).
    pub shots_spawned: u32,
    /// (AUDIT-A SHIELD) Pegasus's +0xC48 (TresStylePawnSHPegasus): the enemies its ride has flown at, a TArray every search
    /// result joins and a search that finds nobody empties (FUN_1413FC260 / Move tick 0x141409CE0); here because KH3's list
    /// has no bound (the ride's state stays `Copy`).
    pub shield_visited: Vec<crate::host::TargetId>,
    // ---- fourth combat pass (2026-10-12)
    /// The attack camera (controller+0xAB0 / +0xAD8; camera.rs, spec 7.12.1).
    pub attack_cam: camera::AttackCam,
    /// controller+0x920: the next attack of the combo is the sneeze (set by `PlayAttack`'s tail
    /// one step before the finisher while the SNEEZE status lasts, cleared when the chain ends;
    /// spec 9.9.5). The HUD shows the Attack command as "Sneeze" while it is set.
    pub sneeze_next: bool,
    // ---- party stream D (2026-10-15)
    /// The friend gauge, the unison request and the friends' rows (friend.rs).
    pub friend: friend::FriendState,
    // ---- (XSI, 2026-10-16) the XS abilities (abilities/xs)
    pub xs: crate::abilities::xs::XsState,
    // ---- (weapons F-ARROWGUN) the gun formchanges' bullets (combat/forms/arrowgun)
    pub arrowgun: forms::arrowgun::Bullets,
    // ---- (F-STAFF) Mirage Staff's mirror images and projectiles (combat/forms/staff)
    pub staff: forms::staff::StaffState,
    // ---- (F-NANO) Nano Arms' Rocket Fists and the finisher's gate (combat/forms/nano)
    pub nano: forms::nano::NanoState,
    // ---- (F-LIGHTDARK) Light / Dark / Double Form: the moving projectiles (Joint Struggle) and their keys
    pub lightdark: forms::lightdark::LightDarkState,
    // ---- (F-SHAPES) where the windows made of several shapes / CONVEX hulls left them last tick (combat/hull.rs)
    pub placed: hull::Placed,
    // ---- (AUDIT-C) the form clips' open BpEvent windows and the Ultimate Finish's Blue Hour particle (states/bp_event.rs)
    pub bp_events: crate::states::bp_event::BpEvents,
    // ---- (AUDIT-A PLACEMENT) the form parts KH3 has thrown (weapon state 5 FreeMove: the raid's shield / pan, the flag)
    pub thrown: Vec<forms::placement::ThrownWeapon>,
}
