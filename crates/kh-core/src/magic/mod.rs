//! OWNER: the magic implementer. Magic, items, shotlock (focus) and airstep of
//! docs/sora-magic-spec.md (parts A - K; the later parts govern the earlier ones), built on the
//! status rules of `status/` (MP, focus, item effects, the damage function).
//!
//! | file | what | written from |
//! |---|---|---|
//! | `tables.rs` | the five `sora-magic-*` sheets as lookup tables | sheets |
//! | `spells.rs` | magic-set units, command costs, attack rows | A 3, C 2, I 6.2 |
//! | `projectile.rs` | every spell actor and shotlock projectile, simulated in the core's step | B, G 3, I |
//! | `items.rs` | the pouch (8 slots with flags, refill), the delayed item effect | E, J 7 |
//! | `shotlock.rs` | the aim mode on the controller: locks, focus, slow motion, airstep targets | F, H 4.2 |
//! | `grand.rs` | grand-magic production: element tallies and the roll | I 7 |
//! | `../states/fire_magic.rs` | `TresPlayerState_FireMagic` (18), `CanMagic`, `TryMagic` | A |
//! | `../states/use_item.rs` | `TresPlayerState_UseItem` (111), `CanUseItem`, `TryUseItem` | E |
//! | `../states/shoot_flow.rs` | `TresPlayerState_ShootFlow` (41): King of Hearts, Ragnarok, Focus Syphon | G |
//! | `../states/athletic_flow.rs` | `TresPlayerState_AthleticFlow` (42): airstep travel and arrival | H, actions II.5 |
//! | `../command/` | the command menu, shortcuts, `ExecuteCommand` | J |
//!
//! WHAT THE HOST SEES: the hits of spell actors go through `Targets::apply_hit` like melee hits
//! (`HitRecord::attack_name` = the `MAG_*` / `SF_*` row; `HitRecord::flags` = command id |
//! shotlock kind << 8); effects and sounds are seam events; things that live in the world are
//! readable between steps through [`Sora::magic`](crate::Sora::magic): the projectiles (position,
//! class, the handle of the persistent effect each one started), the ice-rail nodes, the Aero
//! jump pads; the per-object-type time dilation is [`TimeDilation`] (`Sora::time_dilation`).

pub mod caster; // (party X2) Sora's spells cast by a friend leader
pub mod grand;
pub mod items;
pub mod projectile;
pub mod shotlock;
pub mod spells;
pub mod tables;

use glam::Vec3;

pub use items::{Pouch, PouchSlot};
pub use projectile::{Projectile, ProjectileView, StageDef};
pub use shotlock::{AimMode, AimView, AirstepPoint, LockEntry};
pub use spells::{cmd, AttackRow, Element, SpellUnit};

/// The SQEX per-object-type time dilation as the port needs it (magic spec F A6, H 4.8, J 6):
/// the game slows object TYPES, not the world. Two groups are enough for Sora alone:
///
/// - `player`: type PLAYER = Sora himself. The CORE applies it to his own step (animation,
///   movement, gravity, state timers).
/// - `world`: the other slowed types (ENEMY, GIMMICK, AVATAR, MAP, BG, FRIEND, VEHICLE = the
///   player controller, PHYSX). The HOST applies it to its enemies and level objects; the core
///   applies it to the controller's timers.
///
/// Never slowed: projectiles, prizes, the camera (except by the menu's slow option: `camera`),
/// events, the UI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeDilation {
    pub player: f32,
    pub world: f32,
    /// Only the "Command Selection: Slow" option slows the camera (mask 0x4F9E has CAMERA).
    pub camera: f32,
}

impl TimeDilation {
    pub const NONE: TimeDilation = TimeDilation { player: 1.0, world: 1.0, camera: 1.0 };
}

impl Default for TimeDilation {
    fn default() -> TimeDilation {
        TimeDilation::NONE
    }
}

/// Object-type bits of the shared time table (host/time.rs): PLAYER (Sora), and the player
/// controller (type VEHICLE, 11), whose timers the core scales.
pub const MASK_PLAYER: u32 = 1 << 1;
pub const OBJ_CONTROLLER: u32 = 11;
/// The aim mode's mask 0x4D9E without the PLAYER bit: ENEMY, GIMMICK, AVATAR, MAP, BG, FRIEND,
/// VEHICLE, PHYSX.
pub const MASK_AIM_WORLD: u32 = 0x4D9E & !MASK_PLAYER;

/// `SetTypeRates(CH_PLAYER, 0x4D9E, ...)` with the player type and the rest set apart (the
/// fire puts only the player back to 1.0: F A6). One table for every slow motion of the port
/// (host/time.rs, shared with the combat implementer): a reset here also ends a world slow.
pub(crate) fn set_aim_dilation(cx: &mut crate::pawn::Ctx, player: f32, world: f32) {
    cx.set_time_dilation(MASK_PLAYER, player);
    cx.set_time_dilation(MASK_AIM_WORLD, world);
}

/// The view of the last drawn frame, for the shotlock reticle test (F A4.2): the game reads the
/// local player's last view-projection matrix and view rectangle. A host fills
/// [`crate::Input::view`] each step; `None` = no camera (nothing can be locked).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// The camera's position and its unit axes in the world (core axes).
    pub position: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    /// tan(horizontal FOV / 2) and tan(vertical FOV / 2).
    pub tan_half_x: f32,
    pub tan_half_y: f32,
    /// The view rectangle in pixels: (x0, y0, x1, y1) (the letter-boxed 16:9 area).
    pub rect: [i32; 4],
}

impl View {
    /// A view from a camera position, its yaw / pitch (degrees, the core's conventions: yaw
    /// about Z from +X toward +Y, pitch up positive), the horizontal field of view in degrees
    /// and the view rectangle's size in pixels.
    pub fn from_camera(position: Vec3, yaw: f32, pitch: f32, fov_horizontal: f32, width: i32, height: i32) -> View {
        let (sy, cy) = yaw.to_radians().sin_cos();
        let (sp, cp) = pitch.to_radians().sin_cos();
        let forward = Vec3::new(cy * cp, sy * cp, sp);
        let right = Vec3::new(-sy, cy, 0.0);
        let up = right.cross(forward) * -1.0;
        let tan_half_x = (fov_horizontal.to_radians() * 0.5).tan();
        let aspect = if height > 0 { width as f32 / height as f32 } else { 16.0 / 9.0 };
        View { position, forward, right, up, tan_half_x, tan_half_y: tan_half_x / aspect, rect: [0, 0, width, height] }
    }

    /// `Project` (0x140ECF670, F A4.2): the pixel position of a world point and its depth
    /// (`z` <= 0: behind the camera).
    pub fn project(&self, p: Vec3) -> Vec3 {
        let d = p - self.position;
        let w = d.dot(self.forward);
        let wi = 1.0 / if w.abs() <= 1.0e-8 { 1.0e-6 } else { w };
        let ndc_x = d.dot(self.right) / self.tan_half_x * wi;
        let ndc_y = d.dot(self.up) / self.tan_half_y * wi;
        let [x0, y0, x1, y1] = self.rect;
        let x = (ndc_x + 1.0) * (x1 - x0) as f32 * 0.5 + x0 as f32;
        let y = (1.0 - ndc_y) * (y1 - y0) as f32 * 0.5 + y0 as f32;
        Vec3::new(x, y, if w <= 0.0 { 0.0 } else { w })
    }

    /// Centre of the rectangle (integer halves, as the game computes them) and the scale of
    /// the 1280 x 720 reference (`min(w / 1280, h / 720)`).
    pub fn centre_and_scale(&self) -> (f32, f32, f32) {
        let [x0, y0, x1, y1] = self.rect;
        let (w, h) = (x1 - x0, y1 - y0);
        ((x0 + w / 2) as f32, (y0 + h / 2) as f32, (w as f32 * 0.000_781_25).min(h as f32 * 0.001_388_888_9))
    }
}

/// A node of a Blizzard ice rail (part I 4): a floor point that lives 3 s, linked to the node
/// laid before it. A host with rail riding builds a rail from consecutive nodes; others draw them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailNode {
    pub id: u32,
    /// The node before it in the same rail (0 = the first).
    pub previous: u32,
    pub position: Vec3,
    /// Unit vector from the previous node (the bolt's direction for the first).
    pub tangent: Vec3,
    /// Seconds left.
    pub life: f32,
}

/// The residue of an Aero (`TresProj_MagAeroCylinder`, part I 2.3): a disc of radius 100 that,
/// for 6 s, throws Sora up once with super-jump power.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AeroPad {
    pub id: u32,
    pub position: Vec3,
    pub life: f32,
}

/// An item effect waiting on its target (pawn+0x7E8: `{delay, item}`; E I4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueuedItemEffect {
    pub delay: f32,
    pub item: crate::status::ItemKind,
}

/// What the magic side keeps on the pawn and the controller. Plain data, part of the save state.
#[derive(Clone, Debug, PartialEq)]
pub struct PawnMagic {
    /// pawn+0xE00: the pending magic / item command written by `ExecCommand`.
    pub pending_cmd: u8,
    /// pawn+0x10E4: the combo keep timer ("formchange carry timer", never armed in base form).
    pub combo_keep: f32,
    /// pawn+0xE60 bit 17: the one-frame latch after a command that armed the command lock
    /// (combat strand A 1.3): set in the UI phase, cleared by the next pawn tick.
    pub command_latch: bool,
    /// The spell actors and shotlock projectiles alive (the projectile manager).
    pub projectiles: Vec<Projectile>,
    next_object: u32,
    pub rails: Vec<RailNode>,
    pub pads: Vec<AeroPad>,
    /// The battle pouch (J 7). Filled from the loadout's items on first use.
    pub pouch: Pouch,
    pub item_effects: Vec<QueuedItemEffect>,
    /// The controller's aim mode (F).
    pub aim: AimMode,
    /// The grand-magic bookkeeping (I 7).
    pub grand: grand::Grand,
    /// The story switches of the player state byte +0x95B0: bit 4 "ShootFlow", bit 5 airstep
    /// (H 4.1). A host sets them; the level-99 test configuration has both.
    pub shotlock_unlocked: bool,
    pub airstep_unlocked: bool,
    /// The level's airstep points (`TresShootFlowActor`s, H 4.2.1): a host affordance.
    pub airstep_points: Vec<AirstepPoint>,
    /// The port's deterministic random stream (bad-status rolls, the grand-magic roll).
    pub rng: u32,
    /// What the running ShootFlow state keeps outside itself (lists are not `Copy`): the
    /// projectiles of the volley, by id.
    pub volley: Vec<u32>,
    /// Hits on Sora that were refused because he is invincible (for tests and the debug text).
    pub hits_refused: u32,
}

impl Default for PawnMagic {
    fn default() -> PawnMagic {
        PawnMagic {
            pending_cmd: 0,
            combo_keep: 0.0,
            command_latch: false,
            projectiles: Vec::new(),
            next_object: 0,
            rails: Vec::new(),
            pads: Vec::new(),
            pouch: Pouch::default(),
            item_effects: Vec::new(),
            aim: AimMode::default(),
            grand: grand::Grand::default(),
            shotlock_unlocked: true,
            airstep_unlocked: true,
            airstep_points: Vec::new(),
            rng: 0x2545_F491,
            volley: Vec::new(),
            hits_refused: 0,
        }
    }
}

impl PawnMagic {
    /// A new id for a projectile, a rail node or a pad (counting up from 1).
    pub fn next_id(&mut self) -> u32 {
        self.next_object += 1;
        self.next_object
    }

    /// xorshift32; the value's upper 24 bits.
    pub fn next_random(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x >> 8
    }

    /// The projectiles as a host draws them.
    pub fn projectile_views(&self) -> impl Iterator<Item = ProjectileView> + '_ {
        self.projectiles.iter().filter(|p| !p.dead).map(Projectile::view)
    }

    pub fn projectile(&self, id: u32) -> Option<&Projectile> {
        self.projectiles.iter().find(|p| p.id == id)
    }
}

/// The Aero residue's launcher (`TresChrLauncherComponent`, tick 0x140F60730; part I 2.3): a
/// cylinder of radius 100 and height 50 centred 20 below the pad. The first tick Sora's root is
/// inside it he is thrown up with a super jump at his own yaw (`RequestJump(yaw, type 2)`: the
/// actions implementer's `super_jump_request`), once: the pad then ends. Enemies are not launched.
pub(crate) fn pads_tick(state: &mut crate::states::StateData, cx: &mut crate::pawn::Ctx) {
    if cx.pawn.magic.pads.is_empty() || cx.pawn.status.hp < 1 {
        return;
    }
    // The launch gate (slot 216): any current state except the cinematic / finish ones; nothing
    // blocking queued. Of the port's states: not while dying, casting the pad's own spell, or in
    // a shotlock / airstep.
    use crate::states::StateData as S;
    if matches!(state, S::Die(_) | S::FireMagic(_) | S::ShootFlow(_) | S::AthleticFlow(_) | S::Damage(_)) || !cx.pawn.queue.nothing_queued() {
        return;
    }
    let s = tables::Sheet::Proj;
    let (scale, offset) = (s.vec3("aero_cylinder.launcher_scale"), s.vec3("aero_cylinder.launcher_offset"));
    let (radius, half) = (100.0 * scale.x, 50.0 * scale.z);
    let root = cx.pawn.cmc.location;
    let hit = cx.pawn.magic.pads.iter().position(|pad| {
        let d = root - (pad.position + offset);
        d.truncate().length() <= radius && d.z.abs() <= half + 30.0
    });
    if let Some(k) = hit {
        cx.pawn.magic.pads.remove(k);
        let yaw = cx.pawn.yaw;
        crate::states::jump::super_jump_request(cx, yaw, Vec3::ZERO);
    }
}
