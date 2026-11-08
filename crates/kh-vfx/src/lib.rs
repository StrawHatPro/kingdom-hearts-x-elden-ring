//! kh-vfx: Sora's visual effects, engine-free.
//!
//! The game's effects are UE4 Cascade particle systems with Square Enix modules that turn each
//! emitter into a node of a hierarchy (docs/sora-vfx-spec.md). This crate evaluates them from
//! the effect pack's JSON (`build/sora_pack/vfx/effects/*.json`) in KH3's own conventions
//! (cm, seconds, degrees, X forward, Y right, Z up, left-handed) at the host's fixed step, and
//! hands the host draw lists: quads, ribbons and mesh instances with a material id, a colour
//! and UVs. It draws nothing itself and knows no engine.
//!
//! ```text
//! let mut world = World::new();
//! world.register(EffectDef::parse(&json_text)?);          // once per effect
//! let id = world.spawn("vs_fir_hit0", Xform::at(point), SpawnOptions::default());
//! per fixed step:   world.set_transform(id, xform);  world.step(1.0 / 60.0);
//! per drawn frame:  world.draw(alpha, &camera, &mut draw_list);
//! ```
//!
//! What is evaluated, what is approximated and what is skipped, module by module:
//! `sheets/port/sora-vfx-modules.json` and `build/sora_tmp/vfx/RENDER_COVERAGE.md`. A module the
//! evaluator does not know is skipped and logged once (`World::take_log`), never a panic.

pub mod def;
pub mod dist;
pub mod draw;
pub mod taildecal; // (VFX-RING) TypeDataSQEX_TailDecal
pub mod math;
pub mod tailmodel; // (VFX-RING M) TypeDataSQEX_TailModelMarch
pub mod tailnoise; // (VFX-RING) SQEX_TailNoise
pub mod rng;
pub mod ring; // (VFX-RING)
pub mod dynshape; // (VFX-RING S) TypeDataSQEX_DynamicShape
pub mod param_dist; // (VFX-RING)
pub mod screen; // (VFXR)
pub mod vfxring_more; // (VFX-RING)
pub mod vfxr_modules; // (VFXR)
pub mod sim;

pub use def::{base_name, EffectDef, EmitterDef, Kind};
pub use def::{RayHit, RayWorld, RaycastDef}; // (AUDIT-B W-HANDOVER) the SQEX_Raycast world seam
pub use dist::Dist;
pub use draw::{Blend, Camera, Decal, DrawList, Light, MeshInstance, Quad, Ribbon, RibbonPoint, TintRange}; // (DON-MAGIC tint) TintRange
pub use draw::{group_order, DrawGroup}; // (VFXR sort)
pub use draw::sub_image_code; // (ZA)
pub use def::{AnimChange, SkelAnim, SkeletalDef}; // (VFX-2)
pub use draw::SkelInstance; // (VFX-2)
pub use dynshape::{DynShapeDef, RibbonExact}; // (VFX-RING S)
pub use def::SortMode; // (VFXR sort)
pub use math::{euler, rotator, Xform};
pub use rng::Rng;
pub use sim::{lens_turns, set_depth_offset_toward_camera, set_lens_rule, perlin, EffectId, EmitterProbe, ShakeRequest, SpawnOptions, World};
pub use sim::BeamAsk; // (AUDIT-B W-VFX)
pub use def::{BeamDef, BeamPoint}; // (AUDIT-B W-VFX)
pub use def::BeamNoiseDef; // (AUDIT-B W-VFX2)
pub use sim::recolor; // (DON-MAGIC tint)
pub use screen::{CharParam, ScreenFx, ScreenKind}; // (VFXR)
