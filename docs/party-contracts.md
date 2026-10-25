# Party port: interfaces between the six streams

One section per stream, owned and kept current by that stream (`build/sora_tmp/party/BRIEF_PARTY.md`). Never edit
another stream's section; ask under "## Requests" at the end (name the stream) and/or message "main".
Plan: `docs/party-port-plan.md`. Units everywhere: KH3's (cm, s, degrees, X forward / Y right / Z up, yaw from +X to +Y).

## Stream A: assets, pack, rendering, animation

Owner: stream A. Status: PUBLISHED 2026-10-15 (phase 0; pack and draw code in progress, shapes below are fixed unless
this section says otherwise). State note: `build/sora_tmp/party/IMPL_A_STATE.md`. Files A owns: `tools/party_pack.js`,
`tools/gen_party.js`, `build/sora_pack/party/**`, `build/gen/rust/party_anim_*.rs`, `sheets/port/party-assets.json`,
`crates/kh-app/src/party_draw.rs`, the `kh-anim` additions (KineDriver scale-op-child SSC, NPC Bonamik notify clips,
face slots per pack), small marked `(party A)` additions in `kh-app/src/char_material.rs` (party material libraries).

### A.1 The pack (`build/sora_pack/party/`, written by `node tools/party_pack.js`)

```
party/party.json                index: format kh3-party-index/1; members [{ member: 1, npc: "n_ex001", name: "Donald",
                                unique_id: 11, dir: "n_ex001", default_weapon: "w_do010", weapons: [{ item: "WEP_DONALD00",
                                row: "I03021", model: "w_do010", dir: "weapons/w_do010" } x14] }, { member: 2, npc: "n_ex002", ... }]
party/n_ex00x/manifest.json     format kh3-party-pack/1 = THE SORA PACK'S MANIFEST SCHEMA (tools/sora_pack.js), so that
                                kh_anim::Pack::load(dir) + kh_anim::Animator::new work unchanged: skeleton.bones[] (own
                                skeleton, *_ue values), items[] (kind mesh | animation | face-animation | data, with
                                notifies at full precision), logical_animations[] = EVERY row of n_ex00x_AnimSet (220 / 225)
                                with blend-space samples / grids, face_animations[] (every row of the face set),
                                root_motion.tracks (per-frame root track of every sequence that moves its root),
                                attack_sockets.clips (per frame, ACTOR space cm: the default weapon's ATK sockets on the
                                weapon socket), runtime.{saved_pose.face_subtrees, face.{rows, type_to_row}, kine_driver,
                                bonamik}, placement (mesh yaw -90, capsule), identification (pawn, anim BP, sets).
                                Extra blocks: camp_animations (CM_Body / CM_Face rows), gummi_animations (GM rows),
                                weapons (as party.json), census (notify classes, KDI operator kinds, Bonamik UNPORTED bits).
party/n_ex00x/model.glb         LOD 0, own skeleton (node index = bone index), glTF conventions as sora.glb
party/n_ex00x/anims/<seq>.glb   every body / camp / gummi sequence;  face/<seq>.glb  every face sequence
party/n_ex00x/data/             kinedriver.json (the mesh's DEFAULT KBD element's KDI), bonamik.json (default element's
                                Bonamik asset), physics_asset.json; Goofy also kinedriver_shortEar.json, bonamik_cs.json
party/n_ex00x/materials/        materials.json (kh3-sora-materials/1, owner ids renumbered: Donald 100.., Goofy 200..),
                                materials.wgsl (KH_C<id> blocks only), textures/*.png, uv2_*.bin: the body AND its 14 weapons
party/weapons/w_xx###/          weapon.glb (own skeleton), manifest.json (skeleton, sockets incl. ATK_*, attach =
                                socket R_wep_skt (Donald) / R_wep (Goofy) on the hand bone with the socket transform,
                                materials, equip item), data/bonamik.json / physics_asset.json when the weapon has them
```

### A.2 Generated anim tables (for B; `node tools/gen_party.js`, output `build/gen/rust/`)

Same types as `kh_core::anim` (gen_port.js `sora_anim.rs`), but one shared type file because the NPC sets use notify
classes Sora's enum lacks:

```rust
// party_anim_types.rs: RootKey, NotifyClass (union of both NPC sets' + weapons' classes, game_name()), EffectAttach,
//   NotifyData, NotifyDef, SequenceData, BlendSpace2d, AnimData (fields exactly as kh_core::anim's), and
pub struct AnimTables { pub npc: &'static str, pub sequences: &'static [SequenceData], pub anims: &'static [AnimData],
    pub face_rows: &'static [(&'static str, &'static str)] /* face row -> sequence */, pub notify_assets: &'static [&'static str] }
// party_anim_n_ex001.rs / party_anim_n_ex002.rs (expect `use super::types::*;` in the including module):
pub enum NpcAnimId { IdleN = 0, ... }          // EVERY row of the set, set order; digit-leading names get "A" ("A1301")
impl NpcAnimId { pub const ALL: [..]; pub fn from_name(&str) -> Option<NpcAnimId>; pub fn data(self) -> &'static AnimData }
pub static SEQUENCES: &[SequenceData]; pub static ANIMS: &[AnimData]; pub static FACE_ROWS: &[(&str, &str)];
pub static NOTIFY_ASSETS: &[&str]; pub static TABLES: AnimTables;
```
Suggested include in kh-party (B): `pub mod types { include!(concat!(env!("KH_GEN_DIR"), "/party_anim_types.rs")); }`
`pub mod donald { use super::types::*; include!(concat!(env!("KH_GEN_DIR"), "/party_anim_n_ex001.rs")); }` (+ goofy).
`SequenceData.sockets` = the default weapon's ATK_00 / ATK_01 per frame (actor space) for clips with a hit window or a
Shot notify, as Sora's. ALSO (B's request): `party_clips_n_ex001.rs` / `_n_ex002.rs` = the same rows in kh-party's B.7
types (`N_EX001_ANIMS: AnimSetTable`), see Requests. `AnimTables` also has `camp_rows` ("row@anim set" -> sequence).

### A.3 Draw API (`kh-app/src/party_draw.rs`, Bevy; F wires it, nothing else in kh-app needed)

```rust
pub struct PartyDrawPlugin;                       // F: app.add_plugins(party_draw::PartyDrawPlugin) after CharMaterialPlugin
#[derive(Resource)] pub struct PartyDraw { .. }   // inserted by the plugin
pub const DONALD: u8 = 1; pub const GOOFY: u8 = 2;
pub struct PoseInput<'a> {
    pub position: Vec3,            // actor origin, KH world (cm, Z up)
    pub yaw: f32,                  // actor yaw, degrees (KH)
    pub dt: f32,                   // the fixed step
    pub body: kh_anim::BodySlot<'a>, // B's FullBody slot after its pose tick (sequence names = pack anims/<seq>.glb)
    pub weapon_visible: bool,
    pub anim: (&'a str, f32),      // the playing anim-set row and its time (passed on to E's party_fx tap)
    pub grounded: bool,            // movement mode walking (E's auto-SE)
    pub member: u8,                // B's MemberId.0 (0 Donald, 1 Goofy): the party_fx tap's id (F's request; F fills it)
}
// set_pose ALSO publishes the member to E's tap every step: kh_app::party_fx::publish_member(member, MemberTap { bones:
// every bone + every character socket + every weapon socket (plain name, and "wep:<name>"), weapon model, anim, .. }).
// IMPLEMENTED 2026-10-15 (kh-app/src/party_draw.rs; main.rs has `mod party_draw;` + `app.add_plugins(PartyDrawPlugin)`,
// two marked lines A added for F). Start-up debug words without F's glue: env KH_PARTY_DRAW="show; pose donald RunLoopN 680".
impl PartyDraw {
    /// `pack_root` = the main pack folder (build/sora_pack; the asset root); reads party/<npc>/.
    pub fn spawn_member(&mut self, commands: &mut Commands, assets: &AssetServer, id: u8, pack_root: &Path) -> Result<(), String>;
    pub fn despawn_member(&mut self, commands: &mut Commands, id: u8);
    /// Once per FIXED STEP after B's step (evaluates kh-anim: saved pose, face, KineDriver, Bonamik; leg IK where the
    /// character has the node). Drawing interpolates the two latest steps with `player::Sim::alpha` (A's system).
    pub fn set_pose(&mut self, id: u8, input: &PoseInput, world: &dyn kh_anim::IkWorld);
    /// An equip item id ("WEP_DONALD00" .. / "WEP_GOOFY00" ..) or a model id ("w_do010"): the weapon model changes.
    pub fn set_weapon(&mut self, commands: &mut Commands, assets: &AssetServer, id: u8, item: &str) -> Result<(), String>;
    /// For E / D: the latest step's world transform of a bone (KH space, cm) and of a weapon socket ("ATK_00" ...).
    pub fn bone_world(&self, id: u8, bone: &str) -> Option<kh_anim::Trs>;
    pub fn weapon_socket_world(&self, id: u8, socket: &str) -> Option<kh_anim::Trs>;
    /// `party-word A ...`: `show` (spawn both in front of Sora, standalone), `pose donald|goofy <row>` (the member plays
    /// the anim-set row itself, looping: no B needed), `weapon donald|goofy <item|model>`, `hide`, `status`.
    pub fn debug_word(&mut self, words: &[&str]) -> Option<String>;
    pub fn status_text(&self) -> String;          // F1 rows
}
```
Material instances per member: `char_material::Instance::Party(id)` (own light rig = the world's, own bone table).

Added 2026-10-15 (F's requests):
```rust
impl PartyDraw {
    /// One fixed step from B's AnimView (rows resolved to sequences; 1D blend spaces at blend_axis, 2D: the sample
    /// nearest (axis, 0) [D]; notify window from the previous call). `member` = B's MemberId.0 for the party_fx tap.
    /// A debug pose (`party-word A pose <who> <row>`) owns the member until `pose <who> off`.
    pub fn set_pose_view(&mut self, id: u8, member: u8, position: Vec3, yaw: f32, dt: f32, anim: &kh_party::AnimView,
                         weapon_visible: bool, grounded: bool, world: &dyn kh_anim::IkWorld);
    // CAMP-MENU COPY (the UI actor n_ex00x_UIActor: the body, no weapon; camp anim set Idle_CM 0001CM / IdleBreak_CM 0002CM)
    /// Spawns a HIDDEN copy on render layer `layer` at `transform` (Bevy; the model is turned to face +X like Sora's menu
    /// copy, pass the copy's position); materials = `Instance::PartyMenu(id)` (exposure = the menu's; the HOST sets its
    /// rig with `CharLibrary::set_rig(Instance::PartyMenu(id), rig)` and `set_active`). Returns the root (Visibility).
    pub fn spawn_menu_copy(&mut self, commands: &mut Commands, assets: &AssetServer, id: u8, pack_root: &Path, layer: usize, transform: Transform) -> Result<Entity, String>;
    pub fn despawn_menu_copy(&mut self, commands: &mut Commands, id: u8);
    pub fn play_menu_row(&mut self, id: u8, row: &str) -> Option<f32>;   // "Idle_CM" | "IdleBreak_CM" -> length
    pub fn step_menu_copy(&mut self, id: u8, dt: f32) -> bool;            // advance + pose; true = the clip wrapped
    pub fn menu_copy_root(&self, id: u8) -> Option<Entity>;
}
```
The copy's bones are written by A's Update system `animate_menu` (call `step_menu_copy` in Update before it, or
accept one frame of lag). Camera, capture, panel image and the idle-break count stay F's (as menu_model.rs for Sora).

Added 2026-10-15 (A2; state note IMPL_A_STATE.md "A2"):
- Leg IK is ON for both members (`data/legik.json`): Goofy TwoBoneIKMultiped with the stock solver, Donald two
  CollTwoBoneIK (BOX sweeps). The `world: &dyn kh_anim::IkWorld` passed to `set_pose` / `set_pose_view` now matters:
  pass the level (kh-app `Level` implements it, incl. the new defaulted `IkWorld::sweep_shape` for boxes). The same world
  answers the Bonamik ground (a trace under the member each step).
- `kh_anim::IkWorld` gained `fn sweep_shape(&self, start, end, shape: IkShape, rotation) -> Option<ShapeHit>` with a
  default (spheres via `sweep_sphere`, boxes / capsules: no hit): other hosts compile unchanged.
- kh-anim `Animator`: `set_body_samples(Vec<(String, f32)>)` (N blend-space samples for the next step),
  `set_bonamik_ground(Option<(Vec3, Vec3)>)`, `leg_ik_kind()`, `coll_ik_reports()`, `unported_cloth`.
- 2D rows in `set_pose_view`: full stock grid; until B2 adds `AnimView::blend_axis_y` (Requests), (0, blend_axis).

## Stream B: party simulation (`crates/kh-party`)

Owner: stream B (B5 successor since 2026-10-15). Status 2026-10-15: phase 1 implemented (states, actions, projectiles, status, KO / revive, items,
warp, progression); B2 closed the decomp reads listed in B.10 "B2"; B3 (2026-10-15) closed JumpBase, Warp, BadStat Stun / Sneeze / Freeze / Burn, Reflect, Discus flight, Tornado hull, Turn tolerance; B4 (2026-10-15) closed SlopeSlide / SuperSlide / RailSlide / Hopping / SituationAction and the [inf] items (B.10 "B4"); B5 (2026-10-15) closed the jump / fall (NavLinkJump), Climbing, Dangle, the off-rail warp query, the ice rails and the slide pitch (B.10 "B5"). `cargo test -p kh-party` green. State note: `build/sora_tmp/party/IMPL_B_STATE.md` (STAND-INS listed). Code is authoritative where it and this text differ (each type's
doc comment repeats its contract).

### B.1 Crate and dependencies

`crates/kh-party` (engine-free: `glam`, `kh-core`; no Bevy, no files, no clocks, no hash-map iteration; deterministic
per-member RNG streams). `pub mod team` is stream D's (`src/team/**`); everything else in the crate is B's.
Once stream C publishes `kh-ai`, kh-party adds the dependency `kh-ai` and implements C's `AiBody` in `src/ai_body.rs`
(B owns that file); C does not depend on kh-party.

### B.2 Identity

```rust
pub enum Chara { Donald, Goofy }                 // ETresChrUniqueID 11 / 12; Chara::uid(), Chara::from_uid()
pub struct MemberId(pub u8);                     // index in Party (spawn order); stable while the member exists
```

`MemberId(0)` / `MemberId(1)` are Donald / Goofy in the test level (party slots 0 / 1 of `FriendRelationDataTable`, HUD
slots 1 / 2).

### B.3 The party and the per-step order

```rust
pub struct Party { .. }
impl Party {
    pub fn new(config: PartyConfig) -> Party;
    pub fn spawn(&mut self, chara: Chara, position: Vec3, yaw: f32) -> MemberId;
    pub fn remove(&mut self, id: MemberId);
    pub fn members(&self) -> impl Iterator<Item = MemberId>;
    pub fn command(&mut self, id: MemberId, cmd: MemberCommand) -> CommandTicket;  // C's tasks, D's FR hand-over, debug words
    pub fn command_status(&self, id: MemberId, ticket: CommandTicket) -> CommandStatus;
    pub fn step(&mut self, dt: f32, sora: &SoraView, host: &mut PartyHost) -> PartyOutput;
    pub fn view(&self, id: MemberId) -> Option<MemberView>;          // what C's decorators / F's HUD read
    pub fn status(&self, id: MemberId) -> Option<&FriendStatus>;     // F's camp menu reads / edits (status_mut)
    pub fn status_mut(&mut self, id: MemberId) -> Option<&mut FriendStatus>;
    pub fn function(&self, id: MemberId, name: &str) -> Option<f32>; // BP function-by-name for C (CanBattle, IsRideOn,
                                                                     // IsSwimming, GetMagicPoint ...; None = unknown name)
    // hits ON a member (from the host's enemies / dummies):
    pub fn defender(&self, id: MemberId) -> Option<kh_core::status::damage::Defender>; // kind Friend, damage_max 24 ..
    pub fn receive_hit(&mut self, id: MemberId, hit: &HitRecord) -> HitResult;          // hit.damage already final
    // progression (shared EXP, stats spec 5.2):
    pub fn set_total_exp(&mut self, total: i32) -> Vec<(MemberId, u32, u32)>;  // Sora's total; (member, from, to) per level-up
    pub fn set_in_battle(&mut self, on: bool);                                // the host's engaged flag, as Sora's
    pub fn find(&self, chara: Chara) -> Option<MemberId>;  pub fn member(&self, id) -> Option<&NpcPawn>;
    pub projectiles: Vec<Projectile>;                                          // spells / shield in flight (E's taps)
    pub fn heal(&mut self, id: MemberId, hp: i32, effect: u8);                 // Sora's Cure / items on a friend
}
```

Order per fixed step (F wires it): Sora `step_hosted` -> C's AI controllers tick (they read `Party::view`,
`function`, issue `Party::command`) -> `Party::step` (per member, spawn order: command intake -> pawn tick: movement
with the pose tick and its notifies, then the state's MainProc, as Sora) -> camera -> poses. KH3's own tick order
between player, AI controllers and NPC pawns is [U] (tick groups); B records what it finds in its state note.

### B.4 Commands in (`MemberCommand`)

```rust
pub enum MemberCommand {
    MoveTo { goal: MoveGoal, locomotion: LocomotionDef, walk: bool, acceptable_radius: f32, follow_path: bool },
    TurnTo { goal: MoveGoal },
    Action { def: ActionDef, target: TargetRef },   // every attack / spell / special of 2.4 of the plan
    UseItem { item: ItemKind, target: TargetRef },
    Cure { target: TargetRef },                      // = Action{def: DonaldCure} with an explicit target
    Warp { goal: Vec3, yaw: f32 },
    SetBattleFollowMode(bool),
    Stop,                                            // abort the running command, back to Idle
    PlayMotion { name: &'static str, looping: bool },
    Idle { time: f32 },                              // TresBTTask_Idle (C)
    Focus(TargetRef),                                // GameplayFocus: Idle faces it; instant (C)
    EnterFrState { state: u16 },                     // D: an FR NPC state (team attacks) by party-states id
}
pub enum MoveGoal { Point(Vec3), Sora, Member(MemberId), Target(TargetId) }
pub enum TargetRef { None, Sora, Member(MemberId), Enemy(TargetId), Point(Vec3) }
pub enum LocomotionDef { Move1, MoveSlow, FollowPlayer, SlopeSlide }  // n_ex00x_Move1 (140..680), Move_Slow (140)
pub enum ActionDef {   // the BT action assets, `ActionDef::from_asset("Attack4_MagFire")`
    DonaldBeat, DonaldSwing, DonaldJump, DonaldFire, DonaldBlizzard, DonaldThunder, DonaldAero, DonaldCure,
    GoofyPush, GoofySweep, GoofyJump, GoofyTornado, GoofyTurbo, GoofyDiscus,
}
pub struct CommandTicket(pub u32);                         // counts up from 1 per member
pub enum CommandStatus { Running, Succeeded, Failed, Aborted, Unknown }
```

A new command replaces the running one (the old ticket ends `Aborted`), except while the pawn is in a state that
refuses commands (Damage, Die, Warp, FR states, UseItem in progress): then the new ticket answers `Failed` at once.
These are the BT task results C maps to `EBTNodeResult`.

### B.5 Views out

```rust
pub struct SoraView {            // F fills from Sora's Output / Status each step
    pub position: Vec3, pub yaw: f32, pub velocity: Vec3, pub on_ground: bool,
    pub hp: i32, pub hp_max: i32, pub mp: i32, pub mp_max: i32, pub dead: bool,
    pub in_battle: bool, pub lock_on: TargetId, pub total_exp: i32, pub game_level: GameLevel,
}
pub struct MemberView {
    pub id: MemberId, pub chara: Chara, pub state: NpcStateId, pub state_name: &'static str,
    pub position: Vec3, pub yaw: f32, pub velocity: Vec3, pub on_ground: bool,
    pub hp: i32, pub hp_max: i32, pub mp: i32, pub mp_max: i32, pub mp_charging: bool, pub mp_charge: f32,
    pub level: u32, pub ko: bool, pub revive_left: f32, pub in_battle: bool, pub swimming: bool, pub ride_on: bool,
    pub can_battle: bool, pub items: [ItemSlot; 4], pub command: (CommandTicket, CommandStatus),
    pub cooldowns: ActionCooldowns, pub anim: AnimView,
    pub capsule: (f32, f32), pub pitch: f32,   // (B5) the slides' body pitch, deg, UE sign
}
pub struct AnimView { pub clip: &'static str, pub time: f32, pub weight: f32, pub prev_clip: &'static str,
                      pub prev_time: f32, pub blend_axis: f32 }   // A's party_draw evaluates these names
pub struct PartyOutput { pub events: Vec<PartyEvent>, pub sora_heals: Vec<SoraHeal>, pub sora_mp: Vec<i32> }
pub struct PartyEvent { pub member: MemberId, pub kind: PartyEventKind }
pub enum PartyEventKind {
    StateChanged { from: NpcStateId, to: NpcStateId }, Notify { clip: &'static str, index: u16, phase: NotifyPhase },
    Voice { group: &'static str },   // E picks the entry (TresVoiceGroupSet)
    Hit { target: TargetId, damage: f32, killed: bool }, Damaged { amount: i32, hp: i32 },
    Ko, Revived, LevelUp { level: u32 }, ItemUsed { item: ItemKind, target: TargetRef },
    SpellCast { def: ActionDef }, ProjectileSpawned { kind: &'static str, at: Vec3 }, ProjectileEnded { .. },
}
pub struct SoraHeal { pub amount: i32, pub effect: u8, pub from: MemberId }
```

Sound and effects that B plays go straight to the host traits in `PartyHost` with `Attach::Party { member, bone, offset }`
(E's seam addition; until it lands B uses `Attach::World(position)`), and are also listed as events. Notify-driven sounds
and effects: B emits `Notify` events for every notify of the running clip (begin / tick / end / instant) so E can play
them; B itself only acts on the gameplay notifies (attack windows, state windows).

### B.6 Host bundle

```rust
pub struct PartyHost<'a> {
    pub world: &'a dyn kh_core::World,          // collision (the same world Sora uses)
    pub targets: &'a mut dyn kh_core::Targets,  // the ENEMIES the friends hit (never Sora, never each other)
    pub audio: &'a mut dyn kh_core::Audio,
    pub effects: &'a mut dyn kh_core::Effects,
}
```

### B.7 Animation data (with stream A)

kh-party runs its own NPC anim clock (the same rules as `kh_core::anim`: montage slot, blend-in, notify window,
root motion) over a data table in B's type:

```rust
pub struct ClipDef { pub name: &'static str, pub sequence: &'static str, pub length: f32, pub rate: f32,
                     pub blend_in: f32, pub root_motion: bool, pub notifies: &'static [ClipNotify],
                     pub looping: bool, pub root: &'static [RootKey] /* B's copy */,
                     pub socket: &'static [SocketKey] /* weapon attack socket ATK_00 / ATK_01 per key, actor frame */ }
pub struct ClipNotify { pub class: &'static str, pub start: f32, pub end: f32 /* == start: instant */,
                        pub data: ClipNotifyData }
pub enum ClipNotifyData { None, Attack { attack: &'static str, collision: &'static str }, State { name: &'static str },
                          Sound { cue: &'static str }, Effect { asset: &'static str, socket: &'static str }, Raw(&'static str) }
pub struct AnimSetTable { pub chara: Chara, pub clips: &'static [ClipDef] }   // looked up by anim-set row name
```

REQUEST TO A: emit `build/gen/rust/party_anim_n_ex001.rs` / `_n_ex002.rs` as `pub static N_EX001_ANIMS: AnimSetTable`
(rows by the anim set's row names, e.g. "1301", "MagicFire", "IdleN") in these types; kh-party `include!`s them through
env vars `KH_PARTY_ANIM_N_EX001_RS` / `..._N_EX002_RS` set by its build.rs when the files exist, and falls back to
B uses A's `party_clips_n_ex00x.rs` (`N_EX00x_CLIPS`) when present, else its own minimal table (`tools/party_sheet.js` -> `build/gen/rust/party_data.rs` `N_EX00x_MIN_CLIPS`: every AnimSequence
row with length, rate and decoded notifies; no root / socket tracks) until then (`tables::anim_set`).

### B.8 Status (with F)

`FriendStatus` (kh-party `status.rs`): chara, level (from the shared total EXP and the character's own
`n_ex00x_LevelUpData`), hp / mp / max, strength / magic / defense / AP (level row + weapon + bonuses), weapon id,
armour / accessory / item slots (`TresChrInitData`), abilities (`kh_core::status::ability_set::AbilitySet`, owned /
equipped counts), victory-bonus counts, AI settings bytes `ai_battle` / `ai_ability` / `ai_cure` (+0x15A..+0x15C of the
party save record; F writes them, C reads them through `MemberView`-side getters `Party::ai_settings(id)`).
Level 99 test default: `PartyConfig::test_level99()` (plan N2).

### B.9 Sheets and generated code (B's)

`sheets/port/party-{params,levels,attacks,states,weapons,abilities}.json` (written by `tools/party_sheet.js`, rows cite
assets / addresses, registered in `sheet_defs.js`); the same tool writes `build/gen/rust/party_data.rs` (run it after
`node tools/gen_port.js`). Lookups: `kh_party::tables::{param, cparam, aparam, level_row, level_for_exp, attack_row,
weapon, ability_cost, clip}`.

### B.10 Other streams' asks, answered

- C: `kh_party::ai_body::PartyBody { party: &mut Party, member }` implements `kh_ai::AiBody` (cargo feature `ai`, on by
  default). State names: Idle TSID_STATE_IDLE, Move TSID_STATE_AI_MOVE, Turn TSID_STATE_TURN, Damage TSID_STATE_DAMAGE,
  Die TSID_STATE_DIE, actions TSID_STATE_AI_ACTION, and every other state by its ETresStateID name (`ai_body::state_name`).
  Unknown action / item names answer ticket 0 (Unknown -> fails). An Action / Cure the pawn cannot start answers
  `Failed` at `command()` time (not queued); `Stop` cancels a running action; `IsActionOff` = 1; `CanBattle` per
  0x141257340; unknown function names answer `None`. A pending `EnterFrState` wins over later commands of the step.
- D: `impl team::TeamParty for Party`; `NpcStateId::Fr(n)` is passive: applies the latest `FrDrive` (transform, motion,
  visibility), refuses every command but `Stop`; `EnterFrState` accepted from any state but Die; hits are refused in it.
- E: QuarterHP is played by B through `voice_play` + `SeamHandles::for_member` (the E.4 rule). Casts and item uses only
  SET the voice-group number before the clip (`Audio::party_voice_group_no`: attack magic `magic_voice_number`, Cure /
  item on another character `target_voice_number`, on himself 0); the clips' own group notifies play the line (no direct
  MagicCast / GiveRecovery plays from B).
- E / F: `Projectile { kind, owner, position, velocity, .., effect: u32 }` in `Party::projectiles` (pub): `effect` is the
  spell actor's body-effect handle, 0 until the host / E allocates one (`SeamHandles::for_member(owner).effect()`) and
  writes it back; B keeps it for the projectile's life and emits `ProjectileEnded` when it goes. Notify dispatch: DONE
  (2026-10-15, B2): `NpcPawn::step` calls `PartyNotify::dispatch` (one `NpcPawn::notify_seam` per member) in the pose
  tick with B's adapter `kh_party::notify_seam` (A's `party_anim_n_ex00x.rs` SEQUENCES looked up by sequence name + class +
  trigger time; cfg `kh_party_anim_full` when A's three files exist): Sound / WeaponSe / AutoSe / Voice, one Effect per
  AttachEffect row, ActorVoice from `m_ActorVoice`; location = actor, effect group 0. Hosts must NOT dispatch party clip
  notifies themselves. B's `PartyEventKind::Notify` stays for logs. `Projectile` adds `turn` (homing turn max),
  `homing_delay`, `homing_on`, `caught`; `Projectile::shapes(&mut Vec<HitShape>)` gives the live attack shapes
  (Thunder capsule + cylinder, Aero sphere 120 ...), `radius()` the largest; Thunder / Aero are put at the target's feet.
- B2 (2026-10-15) behaviour now read from the decomp (sheet rows cite the addresses): root capsule = TresRoot CAPSULE1
  (Donald r 30 / half 49 / centre 51, Goofy 40 / 89 / 91; `MemberView.capsule`); turn rates `pawn::TurnKind` (Idle 180,
  Turn = TurnN / TurnB clip scaled to the delta, fallback 360, Move 180, attack window 360); damage clips by the NPC
  anim-name vfuncs (`pawn::damage_clip(chara, phase, dir_index)`); wall hit 30 deg / 50 cm/s; Cure = 120 + Magic on the
  target and 60 + Magic on the team within 300 (heal effect 14 / 15 in `SoraHeal.effect`); MagicSet clip choice;
  Discus sub-steps of 0x1411C7670; `CanBattle` = battle flag + 0.5 s latch; init slots protector / accessory 1, item 3
  (Goofy starts with 3 item slots).
- B3 (2026-10-15): `SoraView.camera: Option<PartyCamera>` (F's glue fills it; the Warp state's out-of-view test,
  FUN_140E5AC70). The Warp is NOT instant: point kept, 2 s wait, then the move while neither the friend nor the point
  is in view, facing Sora; a path under 3000 (1000 in the battle-start window) ends it with no move (Succeeded), the
  2 x 2 s limit ends it Failed (0x14126DD90). `AnimView.blend_axis_y` (2D WalkRun rows: X turn rate [inf], Y speed).
  Jump attack = JumpBase (launch on the JumpStart notify; `actions::jump_velocity` = FUN_1405431C0). Discus:
  `Projectile::discus` (speed clamped by the shield's movement MaxSpeed 1500; homing off after the first hit; removed
  when the catch clip starts). Attack_Tornado = `actions::Prism` (`AtkShape.prism`). Bad-status states from
  `HitInfo.bad_status` in `receive_hit` (STUN / SNEEZE / FREEZE / BURN), Reflect when `apply_hit` answers
  `guarded`. `Party::function`: GetMagic, GetMPChargeTimeLeft, CanCastCure, IsEnableWarp. `pawn::TURN_DONE` = 0
  (the TurnTo task's tolerance). The state component has no request priority (latest request wins). P hooks (`pad`).
- B4 (2026-10-15): the traversal / situation states (`kh_party::traverse`, sheet rows slide.* superslide.* rail.* hop.*
  situation.* loco.slope_slide.* walkrun.*). Entry from the Move state = the Run tick's probes (0x140442430: the
  level rails within 50 -> RailSlide; a hop point 120 ahead, radius 100 -> Hopping; a floor flagged
  `SQEX_bEnableSlopeSlide` at -25 deg or steeper, facing down it -> SlopeSlide, FUN_14063C870). While in them, a
  MoveTo keeps the state and feeds its direction (iface vt+0x118); other commands end them. RailSlide = the shared
  UTresCharState_RailSlide (2000 cm/s, turn-around when the move direction is > 120 deg off after a tick without input)
  + the NPC's off-rail warp when the leader is out of rail / hop states and beyond the warp distance + the end jump
  500 cm ahead (`NpcStateId::Jump`, STAND-IN pose). New API: `Party::request_super_slide(id)` (SonicSlide at
  m_SuperSlideSpeed 1500 while a MoveTo feeds it), `Party::set_situation(id, kind, value)` (6 talk point, 7 Goofy's
  coop GCoop*, 8 gimmick wait, 0 end), `Party::request_npc_motion(id, count, [start, loop, end])` /
  `finish_npc_motion(id)` (situation 9), `Party::set_coop_target(id, Option<Vec3>)`;
  `PartyEventKind::SituationCommand { command: 0xCE, on }` (F's HUD); `SoraView.state` (Sora's ETresStateID,
  0 = unknown; F's glue fills it, a marked edit). `NpcAnim::play_loop` (PlayAnim mode 3).
  [inf] items closed: the jump attack's decel +0x25A is set by the attack clips' TresAnimNotify_Timing (handler
  0x14122DE40); the jump-attack range subtracts both bounds radii max(extent.x, extent.y) (FUN_141228B50), not the half
  heights; the Warp's path length = `kh_ai::Nav::path_length` of the stand-in nav (game: FindPathSync friend ->
  leader); Burn = the NPC tick 0x141248FC0 (random turn within 70 deg at the Start clip's end, AddMovementInput in the
  air, End when the status is gone); Reflect = an attacker result REFLECT / GUARD_REFLECT of Classify on the friend's own
  contact (notify slot 15 0x141271720) or a victim result REFLECT / GUARD_REFLECT on a hit taken (TakeDamage
  0x1405BB850): the friends' rows have m_ReflectLevel 0 (no clash), so a guarding target does NOT repel them;
  `actions::deliver` runs kh-core's Classify on `Targets::body` and writes both results into the record. 2D WalkRun
  X = the yaw delta move direction - facing (deg) clamped to the row's range (Donald +-300, Goofy +-180), Y = |velocity|.
  Items: spent at UseItem's Timing notify, the effect 0.5 s later, x (1 + 0.5 x the USER's ITEM_UP), x 0.5 on a target
  in Die (0x1405BE820; the friends' pawn type 10 lets it through, so a KO'd friend gets half and gets up).
  `TeamParty::fr_hit` (D.6) implemented. `Party::spawn_as` / `despawn` (P).
- B5 (2026-10-15): (1) THE JUMP / FALL: `NpcStateId::NavLinkJump` = `UTresAICharState_NavLinkJumpCodeDriven`
  (TSID_STATE_NAVLINK_JUMP 0x2B; `kh_party::jump`): every friend jump (RequestJump 0x14054C2E0 asks 0x2B before 3).
  B4's fall stand-in is gone; `NpcStateId::Jump` (TresNpcState_Jump, id 3) stays in the enum but is never entered.
  The Run and Idle ticks request the fall when not on the ground (0x140442430 / 0x14124CD40), the slope / hop / rail
  states leave into it; clips JumpNormalStart / RiseLoop / DownLoop / Land / RollLand (fall >= 2000, 800 with input) by
  the NPC anim names 0x141263A00; the stuck escape (10 heights within 5 cm). New: `Party::request_jump_to(id, target,
  world)` (JumpCodeDriven, a host's nav link). (2) LEDGES (`kh_party::climb`): `NpcStateId::Climbing` (id 7, the Run /
  SlopeSlide / SuperSlide ticks' climb probe FUN_140636BF0: a top flagged `SQEX_bEnableAutoRun`, 45..225 cm, within 45
  deg; ClimbingR / MR / H, SonicSlideClimbing* from the SuperSlide, LightLandR / L) and `NpcStateId::Dangle` (id 6,
  GripStart / GripLoop / GripRise; requested only by a code-driven jump aimed at a ledge more than 500 cm up, never by the
  Run tick). The AI names: TSID_STATE_NAVLINK_JUMP / CLIMBING / DANGLE. These three take a MoveTo as their feed and
  answer other commands `Failed` until their vt+0x388 CanChange (Stop just ends the command) [inf]. (3) THE OFF-RAIL
  WARP: the RailSlide state asks for `TresFriendData.WarpQuery`: `Party::warp_query_wanted(id)` /
  `Party::answer_warp_query(id, Option<Vec3>)`; kh-ai runs it for `PartyBody` (`AiBody::wants_warp_query` /
  `warp_query_result`, defaulted). (4) ICE RAILS: `Party::set_ice_rails(&[kh_core::magic::RailNode])` before each
  step (Sora's `magic().rails`); the friends ride them like level rails (ids tagged `traverse::ICE_RAIL`). (5)
  `MemberView::pitch` (deg, UE sign): the SlopeSlide body pitch; kh-app `PartyDraw::set_body_pitch(id, pitch)` before
  `set_pose_view` draws the actor rotation (pitch, yaw, 0). (6) SuperSlide +0x90 / +0xA8 are never set in the game
  (only Init 0x141266B90 zeroes them).
- F: `FriendStatus::{equip_weapon, weapon_row, set_item_slot, set_ability, max_hp, max_mp, strength, magic, defense,
  max_ap, ap_in_use, revive_hp, revive_time, target_defense, set_armour, set_accessory, worn, resists}`; fields
  `armour` / `accessories: [Option<&'static str>; 4]` (kh-core EQUIPMENT ids), `armour_slots` / `accessory_slots` (TresChrInitData:
  Donald accessory 2, Goofy protector 2; unserialized counts = `INIT_SLOT_DEFAULT` 1 (FTresChrInitEquip ctor 0x140570AC0;
  item slots `INIT_ITEM_SLOT_DEFAULT` 3: Donald 2 serialized, Goofy 3), +1 per DEF / ACC / ITEM_SLOT_UP1);
  defense and the defender's resists include worn gear. `AiSettings { battle, ability, cure, cure_targets }` = record
  bytes +0x158 / +0x159 / +0x15A / +0x15B. `PartyConfig::test_level99()` = the level-99 default.
- Damage on a member: host runs `compute_damage(attacker, &party.defender(id).unwrap(), ..)` then `receive_hit`. NOTE:
  `m_DamageMax` 24 is not a hard cap: CalcDamage replaces a base above 24 by floor((24 + S - D) / 2), and a friend's base
  is at most 80 (kh-core damage.rs, instruction level).

## Stream C: party AI (`crates/kh-ai`)

Owner: stream C. Status: phase-0 contract published 2026-10-15; `crates/kh-ai/src/api.rs` holds these types (code is
authoritative where it and this text differ). Engine-free: `glam`, `serde_json`, std; no kh-core / kh-party dependency,
no files, no clocks (time = sum of `dt`), deterministic per-controller RNG (plan N6).

### C.1 What drives what

```
F (kh-app), per fixed step after Sora:  for each member m:  ai[m].tick(dt, &mut body_m, &world, &mut shared)
                                         then party.step(...)
body_m : impl kh_ai::AiBody   (B: kh-party/src/ai_body.rs over Party::view / function / command / command_status)
world  : impl kh_ai::AiWorld  (F: from Sora's output, the party views, the host's targets / dummies, the camera)
```

### C.2 The pawn side: `AiBody` (B implements)

```rust
pub trait AiBody {
    fn view(&self) -> BodyView;                         // from MemberView
    fn function(&self, name: &str) -> Option<f32>;      // = Party::function: CanBattle, IsRideOn, IsSwimming, IsStateMoving,
                                                        //   GetMagicPoint, IsBattleFollowMode, IsActionOff, IsBreakPoseNow,
                                                        //   CanbeSwirlUpDamageReactionByEnemy, CanLead, CanNpcMode (bool = 0/1)
    fn command(&mut self, cmd: AiCommand) -> u32;       // = Party::command(...).0
    fn command_status(&self, ticket: u32) -> AiCommandStatus;  // = Party::command_status (same five values)
}
pub struct BodyView { member: u8, uid: i32 /*11|12*/, position, yaw, velocity, on_ground, radius, half_height,
    state: &'static str /* ETresStateID name: "TSID_STATE_IDLE" | "TSID_STATE_TURN" | "TSID_STATE_AI_MOVE" |
    "TSID_STATE_AI_SWIM" | "TSID_STATE_DAMAGE" | ... */, hp, hp_max, mp, mp_max, mp_charging, ko, in_battle, swimming,
    ride_on, can_battle, in_damage_reaction, items: [Option<&'static str>; 4] /* FriendEvaluationData row names:
    "POTION", "HIPOTION", "ETHER" ... of items still held */ }
pub enum AiGoal { Point(Vec3), Actor(ActorRef) }
pub enum ActorRef { None, Player, Member(u8 /* MemberId.0 */), Enemy(u32 /* TargetId.0 */) }
pub enum AiCommand {                      // 1:1 onto MemberCommand
    MoveTo { goal: AiGoal, locomotion: String /* "n_ex001_Move1" | "n_ex001_Move_Slow" | ... asset name */,
             walk: bool, acceptable_radius: f32, follow_path: bool },
    TurnTo { goal: AiGoal },
    Action { def: String /* "n_ex001_Attack4_MagFire": ActionDef::from_asset(chara, name minus "n_ex00x_") */,
             target: ActorRef, location: Option<Vec3> },
    UseItem { item: &'static str /* FriendEvaluationData row */, target: ActorRef },
    Cure { target: ActorRef },
    Warp { goal: Vec3, yaw: f32 },
    SetBattleFollowMode(bool),
    Focus(ActorRef),                      // controller focus (GameplayFocus service): idle facing; instant, ticket ignored
    Idle { time: f32 },                   // TresBTTask_Idle
    Stop,
}
pub enum AiCommandStatus { Running, Succeeded, Failed, Aborted, Unknown }
```

A latent BT task issues one command and polls its ticket every tick: `Succeeded` -> task Succeeded, `Failed` /
`Aborted` / `Unknown` -> Failed, `Running` -> InProgress. When the tree aborts a task, kh-ai sends `Stop` (moves, turns,
and also actions: `TresBTTask_Action` AbortTask 0x140492CB0 cancels the running action state) [C]. An `Action` whose
ticket answers `Failed` at once is re-issued every tick until the pawn accepts it (update 0x1404A2370 keeps asking the
state component) [C]; B should answer `Failed` while the pawn cannot start an action (damage, recovery) and accept
later.

Function names B is asked for (`AiBody::function`; answer `None` for names it does not know, kh-ai then uses its view
fallback): `CanBattle` (battle tree vs field tree), `IsSwimming`, `IsRideOn`, `IsStateMoving`, `GetMagicPoint`,
`IsBattleFollowMode`, `IsBreakPoseNow`, `CanbeSwirlUpDamageReactionByEnemy`, `CanLead`, `CanNpcMode`, `IsActionOff`.
Optional names (C2, 2026-10-15; `None` keeps kh-ai's fallback): `GetMagic` (the Magic stat: Donald's Cure amount
estimate = m_AppendCure 120 + max(1, Magic), 0x141263CC0; fallback 1), `GetMPChargeTimeLeft` (seconds of MP recharge
left, vt+0x17F0; fallback infinity), `CanCastCure` (Donald vt+0x2918 0x1411CA900; fallback: Donald and MP not
recharging), `IsEnableWarp` (SetEnableWarp +0xB96; fallback 1). Cure targets: the game's decision can pick a KO'd
member (PINCH: "HP <= 25 % or dying"; 0x14126AC70), so B's `Cure` / `UseItem` on a KO'd target should do what the game
does there. `CanBattle` (0x141257340) has no player fallback any more: B's answer (or `BodyView.can_battle`) decides.
Optional names (C3, 2026-10-15; `None` keeps the fallback in brackets): `HasAbility:<ETresAbilityKind name>` (pawn vt+0x14F0 with equipped = 1: "HasAbility:DONALD_FIRE", DONALD_BLIZZARD / THUNDER / AERO / CURE, GOOFY_TORNADE / GOOFY_TURBO / GOOFY_FLIP; ValidateAction execution 0x14121E9F0 / 0x14121EBF0; [1]), `GetAbilityNum:ITEM_UP` (count of ITEM_UP equipped, vt+0x1500 0x28: item amounts x (1 + 0.5 n), FUN_141263950; [0]), `IsCommandRestricted:6` (FUN_1405ad320 6: item commands restricted -> no items in the cure decision; [0]), `IsBadStatusBound` (FUN_141267540: bound by a bad status -> no attack executes; [0]), `GetTimeMotionless` (seconds the pawn has not moved, movement +0xC68 = ATresAIPawnBase::GetTimeMotionless 0x141927B00: TimeMotionlessCheck / InterpretPlayerIntent; [seconds below 10 cm/s]). Commands: a MoveTo abort delays its `Stop` by the node's m_AbortMoveTimeToKeepMoving (0.1..0.3 s) unless another command comes first; the tasks' `Idle` asks for `time: f32::MAX` (finished by the pawn leaving idle, not by time).
C4 (2026-10-15): `GetTimeMotionless` is no longer asked: kh-ai keeps the movement component's own record (FUN_140591690: moving = any |root velocity component| > 0.0001 cm/s; TimeMotionless +0xC68; average direction +0xC4C over 3.0 s) from `BodyView.position` / the player's position. New optional name `GetDamageReactionCount` (a counter B bumps each time a hit's damage reaction is accepted by the pawn's state side, i.e. the game's 0x1405BB850 -> vt+0x20F0 path): the Interrupt decorator fires on each change (fallback: entering TSID_STATE_DAMAGE). The `Idle` ticket should answer `Succeeded` exactly when the game's `UTresNpcState_Idle` sends IdleFinished: on leaving the idle state (0x141245380), in battle mode (pawn +0x6E4 == 1) each time the playing idle clip's length has run (0x14124CD40 resets the timer to that length), out of battle when the idle clip ends.
**`IsActionOff` must answer 1 (true) in normal play**: Goofy's root runs an endless Wait under "IsActionOff() != true"
(kh-ai's fallback answers 1). `BodyView.state` names KH3's ETresStateID: the trees test IDLE / TURN / AI_MOVE / AI_SWIM
(free to start an action, cure, item) and DAMAGE.

### C.3 The world side: `AiWorld` (F implements; ER host later)

```rust
pub trait AiWorld {
    fn player(&self) -> PlayerView;      // actor view + floor_position, hp/mp, dead, in_battle, attacking (IsPlayerAttackNow),
                                         // attacked_by_enemies, lock_on: ActorRef, swimming, ride_on, flowmotion, hold_camera_trick
    fn camera(&self) -> CameraView;      // position, pitch, yaw, fov, aspect
    fn enemies(&self, out: &mut Vec<ActorView>);   // every candidate target, stable order
    fn allies(&self, out: &mut Vec<ActorView>);    // the party members incl. the asking one
    fn actor(&self, a: ActorRef) -> Option<ActorView>;
    fn game_level(&self) -> u8 { 1 }     // ETresGameLevel (coordinator curves per difficulty)
    fn nav(&self) -> &dyn Nav;           // StraightNav for the test level (open note N1)
    fn gimmick_obstacles(&self, p: Vec3, radius: f32, out: &mut Vec<(Vec3, f32)>) {}
}
pub struct ActorView { actor: ActorRef, uid: i32 /* ETresChrUniqueID / ETresEnemyUniqueID, -1 unknown */, position,
    center, yaw, velocity, radius, half_height, alive, hostile, lockable, flying, airborne, gimmick, hp, hp_max,
    state: &'static str /* ETresStateID name if known */, resist: [f32; 12] /* per ETresDamageAttribute, 1 = normal */ }
pub trait Nav { project(p, up, down) -> Option<Vec3>; path_exists(a, b) -> bool; path_length(a, b) -> Option<f32>;
                trace_blocked(a, b) -> bool; wall_distance(p, max) -> Option<f32> }   // all defaulted (straight lines)
```

The training dummies need: `hostile = true`, `lockable = true`, a `uid` (any enemy uid not named by the trees' boss
branches; -1 is fine), `resist` all 1.

Added C3 (2026-10-15, defaulted, None = earlier behaviour): `fn player_focus(&self) -> Option<(i32, i32)>` (Sora's focus, max: the cure decision's focus need FUN_141269f60), `fn member_mp(&self, member: u8) -> Option<(i32, i32, bool, f32)>` (the OTHER member's MP, max, recharging, seconds left: its MP need), `fn camera_far(&self) -> Option<f32>` (the drawn camera's far clip in cm: the warp frustum test, camera vt+0x808). Changed C4 (2026-10-15, all defaulted): `fn gimmick_obstacles(&self, out: &mut Vec<GimmickObstacle>)` (every `UTresGimmickObstacleComponent`: base location, radius, half height; was (p, radius, out)), new `fn volumes(&self, class: &str, out: &mut Vec<(Vec3, Vec3)>)` (boxes of e.g. "TresBattleVolume" actors: EQS Volume test), new `Nav::path(from, to, out) -> bool` (path points; default the straight segment), `camera_far` None now means 2097152 cm (UCameraComponent far plane; near 3 cm). `player_focus` also drives the PINCH focus need (kh-ai tracks the focus-break bit: set when the focus drops from >= 1 to 0, cleared at max). `PlayerView.actor.state` should carry TSID_STATE_COVER when Sora covers (CoverWaitCheck) and the camera yaw / pitch the drawn view (IsInViewport, CameraAngle, InterpretPlayerIntent).

### C.4 The controller API (F calls it)

```rust
pub struct AiPack;   impl AiPack { pub fn load(read: &mut dyn FnMut(&str) -> Option<String>) -> Result<AiPack, String> }
                     // read("index.json"), read("trees/<name>.json") ... relative to build/sora_pack/party/ai/
pub struct AiShared; impl AiShared { pub fn new() -> AiShared }   // one per party: coordinator counts, cure claims, combi phase
pub struct AiController;
impl AiController {
    pub fn new(pack: Arc<AiPack>, pawn: &str /* "n_ex001" | "n_ex002" */, member: u8, seed: u64) -> AiController;
    pub fn settings(&self) -> AiSettings;  pub fn set_settings(&mut self, s: AiSettings);   // F's Customize
    pub fn with_library(pack, lib: Arc<kh_ai::bt::Library>, pawn, member, seed) -> AiController; // share one compiled library
    pub fn set_stop_ai(&mut self, stop: bool);   // D: true while a team attack owns the pawn (IsStopAI decorator);
                                                 // every change restarts the tree
    pub fn tick(&mut self, dt: f32, body: &mut dyn AiBody, world: &dyn AiWorld, shared: &mut AiShared);
    pub fn trace(&self) -> AiTrace;        // F's `party-trace` word: active tree path, task, style, target
    pub fn reset(&mut self);               // View reset / respawn: restart the root tree, clear the blackboard
}
pub struct AiSettings { battle: BattleStyle, ability: AbilityStyle, cure: CureStyle, cure_targets: u8 /* 1 Sora, 2 self, 4 others */ }
pub enum BattleStyle { Together = 0, Free = 1, Support = 2, Focus = 3 }   // ETresFNpcAIStyle_Battle
pub enum AbilityStyle { Free = 0, Balance = 1, Less = 2 }                 // ETresFNpcAIStyle_Ability
pub enum CureStyle { Often = 0, Pinch = 1, Always = 2 }                   // ETresFNpcAIStyle_Cure
pub struct AiTrace { pub path: Vec<String> /* "tree/node" root -> active task */, pub task: String, pub style: BattleStyle,
                     pub target: ActorRef, pub log: Vec<String> /* last transitions */ }
```

Defaults (plan N4): Together, Balance, Pinch, all three cure targets. Settings bytes +0x15A..+0x15C map by enum value.

### C.5 Data: the AI pack (`tools/party_ai_pack.js` -> `build/sora_pack/party/ai/`)

`index.json` (roots per pawn: tree, dynamic injections tag -> tree, follow query, locomotion assets; the node-class
census; assets by kind), `trees/*.json` (39), `blackboards/*.json`, `eqs/*.json` (22), `classes/*.json` (action /
locomotion definitions, controller, meta AI: merged class-default properties + native class), `tables/*.json`
(FriendEvaluationData, FriendHomePosDataTable, FriendRelationDataTable, FriendIdDataTable), `data/FriendData.json`,
`curves/*.json` (coordinator CurveVectors, follow curves). Formats are documented at the top of the tool. B may read
`classes/n_ex00x_Attack*.json` for the action definitions' numbers (they are the game's CDO values, merged over the
BP chain).

### C.6 For D (team attacks)

The friends' own requests for team commands come from tree nodes (`TresNpcBTTask_SetFriendLinkCommand` /
`SetFriendTarget` are not in the Donald / Goofy trees' reachable set: the census has none). While D runs an FR state,
D calls `set_stop_ai(true)` (the root's `IsStopAI` gate turns the tree to its infinite Wait) and `false` after.

## Stream D: team attacks and Sora's side

Owner: stream D. Status: PUBLISHED 2026-10-15 (phase 0: the types below compile as stubs; bodies fill in during phase 1).
State note `build/sora_tmp/party/IMPL_D_STATE.md`. Code is authoritative where it and this text differ.
Files D owns: `kh-core/src/combat/friend.rs`, `kh-core/src/states/fr_{call_meteor,flare_force,goofy_shoot,shield_guardian}.rs`
(+ their registry rows / controller lines), the `(party stream D)` blocks in `kh-core/src/host/{targets,events}.rs`,
`crates/kh-party/src/team/**`, `crates/kh-camera/src/camera_anim.rs`, `tools/party_team_pack.js`,
`build/sora_pack/party/team/**`, `sheets/port/party-team.json`.

### D.1 Sora's side (kh-core)

- `combat::friend` (pawn field `PawnCombat::friend: FriendState`): the friend gauge of the player controller
  (ctrl+0xC0C / +0xC10 / +0xC14; slots 899 add, 900 tick, 896 full, 901 reset; `MyFriendPointMax` 200,
  `MyFriendPointTime` 30), the friends' team-command offers taken into the situation queue (11.2 team branch: time x
  (1 + 0.5 x the offering friend's COMBI_TIME), cap 99.9, gauge reset), the unison / fusion roll (pawn slot 1304 /
  1303) and `Decide` rule 7 (kinds 0x3F..0x43).
- Sora's FR states (registry rows): `FRShieldGuardian` 30, `FRGoofyShoot` 101, `FRCallMeteor` 135, `FRFlareForce`
  136, started by the situation executor for kinds 193 / 195 / 207 / 208 (Triangle = pad **Y**); Trinity Guard's own
  command menu (MainCommandData `FR_SHIELDGUARDIAN`: Shield Bash = **A**, Jump = **B**, cancel) in the state.
- Events (`EventKind`, in `Output::events`): `PlayerSpecificAction { state, action, sub }` = the game's
  `ReceivePlayerSpecificAction` of the state-event camera blueprints (the cameras key on these, D.4);
  `TeamAttack { kind, phase }` (phase 0 start, 255 end).

### D.2 The seam (`kh_core::host::Targets`, defaulted methods; types in `kh_core::host`)

```rust
fn friends(&self, out: &mut Vec<Friend>) {}            // party members (uid 11 / 12), party-slot order
fn friend_link(&mut self, req: FriendLinkRequest) {}   // Sora's side -> the friends' side
pub struct Friend { uid: u8, position: Vec3, yaw: f32, ko: bool, combi_time: u8, combi_up: u8,
                    offer: Option<(i32 /*command kind*/, f32 /*reception time*/)>, link: FriendLinkView }
pub struct FriendLinkView { kind: i32, phase: u8, finished: bool, sora: Option<(Vec3, f32)> /*link pawn holds Sora*/,
                            sora_motion: Option<&'static str>, camera_target: Option<(Vec3, f32)> }
pub enum FriendLinkRequest { Sora { gauge_full, in_battle, position, yaw } /*every step*/, Start { kind, target },
                             Action { kind, action } /*Trinity Guard 0 bash 1 jump 2 cancel*/, End { kind, interrupted },
                             Unison { uid, kind, target } }
```
F's combined `Targets` answers `friends` with `kh_party::team::Team::friend_rows(&party, out)` and forwards
`friend_link` to `Team::request(req)`. (An offer read in Sora's step N was made by the party's step N-1: one step of
latency against KH3, where the NPC tick calls `NotifySetActionCommand` directly; recorded in D's note.)

### D.3 The friends' side (`kh_party::team`, D's files inside B's crate)

```rust
pub struct Team;  impl Team { fn new(); fn request(&mut self, FriendLinkRequest);
    fn step(&mut self, dt, party: &mut dyn TeamParty, host: &mut PartyHost) -> TeamOutput;   // AFTER Party::step
    fn friend_rows(&self, party: &dyn TeamParty, out: &mut Vec<Friend>); fn camera_target(&self) -> Option<(Vec3, f32)>; }
pub trait TeamParty {            // B implements for Party (request below)
    fn team_members(&self, out: &mut Vec<TeamMember>);          // id, chara, position, yaw, on_ground, ko, state,
                                                                // in_battle, combi_time, combi_up, clip, clip_time, clip_ended
    fn team_command(&mut self, id: MemberId, cmd: MemberCommand) -> CommandTicket;   // EnterFrState { state } / Stop
    fn fr_drive(&mut self, id: MemberId, drive: FrDrive);        // applied in the member's FR state at its next step
}
pub struct FrDrive { transform: Option<(Vec3, f32)>, motion: Option<(&'static str /*anim-set row*/, f32 /*blend*/, bool /*loop*/)>,
                     visible: bool, weapon_visible: bool, invincible: bool, collision: bool }
pub mod fr_id { CALL_METEOR 1, FLARE_FORCE 2, GOOFY_SHOOT 3, SHIELD_GUARDIAN 4, DONALD_SORA 5, GOOFY_SORA 6 }  // NpcStateId::Fr(n)
```
B's FR state is PASSIVE: while `NpcStateId::Fr(n)` runs, the member does not move or decide by itself, plays the
motion D gives (with its notifies emitted as usual for E's sounds / effects), holds the transform D gives (None:
gravity and the ground only), stops its AI (C: the tree pauses while the state is `Fr(_)`), and leaves the state
only on `MemberCommand::Stop` (D sends it at the end) or KO. The link pawns, their projectiles and their hits
(through `PartyHost::targets`, attacker = the friend) are D's, inside `Team::step`. The NPC link points and offers
(`m_FriendLinkPointInterval` 5 s, `m_HaveFriendLinkCommand`, threshold with COMBI_UP) are D's too.

### D.3a Phase 1 additions (2026-10-15, implemented)

- `FriendLinkView` also carries `accompany: Option<(Vec3, f32)>` (the link pawn), `sora_action: Option<u8>` (a
  `SpecificAction` the link pawn sends on Sora: Sora's FR state emits it as `EventKind::PlayerSpecificAction`) and
  `shots: [Option<TeamShot>; 4]` (the link pawns' hits, delivered by Sora's FR state as HIS shots: the team rows are
  rows of `p_ex001_AttackData`). `FriendLinkRequest::Input { kind, stick, attack, jump }` every step of an FR state.
- `kh_party::team::Team` (file `team/link.rs`): `new`, `request`, `step`, `friend_rows`, `camera_target()` (friend
  manager camera target: the meteor aim / link pawn), `accompany()` (the link pawn: F please pass THIS as
  `CameraTargets::accompany`), `running()`, `debug_ready_all()`. `TeamEvent::ActorSpecificAction { action }` ->
  `StateEventCameras::on_actor_action`.
- kh-core events: `AnimBpEvent { seq, param, begin }` (the BpEvent notify states of Sora's FR clips) also feed the
  cameras. `StateEventCameras::update` returns `fov` and `shake` too (FOV of the CameraAnim track; please apply).

### D.4 Cameras (`kh_camera::camera_anim`)

```rust
pub struct CameraAnimData { name, length, base_fov, pos: Vec<CurveKey<Vec3>>, euler, look_at_interest: bool,
                            fov: Vec<CurveKey<f32>>, interest: Vec<CurveKey<Vec3>> }      // loaded from the pack
pub struct AnimView { location: Vec3, rotation: Rot, fov: f32 }
// phase 1: CameraAnimPlayer (ATresCameraAnim 0x1405DABF0: base = target actor location every frame, base yaw taken
// once at start; view = base * anim; look-at interest; FOV track) and StateEventCameras (the three BPs as data):
//   fn on_event(&mut self, ev: &kh_core::EventKind, ctx: &CameraTargets);  fn update(&mut self, dt, ctx) -> Option<(AnimView, f32 /*blend weight*/)>
```
F: feed every step's events to `StateEventCameras::on_event`, then `update`; when it returns a view, it replaces the
field camera's view (blend weight from the game's `ViewTargetTransitionParams`); camera shakes on top (kh-camera
`shake`). Data: `build/sora_pack/party/team/camera_anims/*.json` (D's pack tool).

### D.5 Pad inputs

Y (Triangle) runs the selected team command (existing situation path); A / B / Y in Trinity Guard's menu; the unison /
fusion finishers come by themselves at the end of an A combo near Donald / Goofy (needs the five UNISON_* / FUSION_*
abilities EQUIPPED: they are owned but not equipped in `sora-loadout-99`). Debug word `team-gauge full` (F wires):
`sora.combat_mut().friend.debug_fill_gauge()` (kh-core, gauge to 200) AND `team.debug_ready_all()` (kh-party::team:
both friends' link points to their threshold, so they offer their commands on the next steps).

### D.6 Leader-agnostic team attacks (D3, 2026-10-15, implemented)

User rule: the PLAYER-CONTROLLED leader (Sora, Donald or Goofy) starts team attacks, AI members never do. With Sora
leading nothing changes.
- Offers and Y: the link pawns and the four FR states still run through Sora's core. A friend leader's Y goes into
  Sora's core input (stream P), executes the offered command from Sora's situation list, and the leader takes part
  as a participant (`MemberCommand::EnterFrState` from `NpcStateId::Pad(_)`, then `fr_drive`). AI Sora never presses Y.
- `kh_party::team::Team::set_leader(Option<MemberId>)`, every step from P (None = Sora leads). The other friend's
  link-point distance (slot 1275) is measured to the leader, the leader's own to Sora. `FriendLinkRequest::Sora`
  stays Sora's (gauge and battle flag of his core).
- `kh_core::combat::friend::FriendState::sora_is_ai: bool`, set by P every step (`sora.combat_mut().friend`).
  While it is true, AI Sora's own combos never roll a unison.
- `FriendState::roll_leader_unison(&abilities /*sora.status().abilities*/, leader_uid 11|12, leader_pos, sora_pos)
  -> bool`: P calls it at its pad combo's step before the finisher and skips its own finisher when it returns true.
  It uses the slot 1304 law with only the leader's kinds (Donald 22..24, Goofy 25..26) and the leader-to-Sora
  distance. Sora's core then plays the unison as his attack within 0.5 s (rule 7 without his combo: the latch
  `leader_unison`), and the friend enters FRDonaldSora / FRGoofySora on Sora as before.
- Trinity Guard's menu: the full pad P passes into Sora's core while an FR state runs is right (`fr_input` takes
  A / B / Y and the left stick as a world direction from the camera yaw of `Input`).
- New seam method `TeamParty::fr_hit(id, row, target, at, host) -> bool` (default false). B: please implement it with
  `actions::deliver` and the member's row found by name. It carries Goofy's FR_GOOFYSHOOT_RUSH during Goofy
  Bombardier.
- Cameras: `StateEventCameras::update_in(dt, targets, Some(world))` (collision pass, shield_01 = a FieldCamera);
  `CameraTargets::stick` = the player's camera input.

### D.7 D4 additions (2026-10-15, implemented)

- `FriendLinkRequest::Input { kind, stick, attack, jump, target }`: `target` = the player controller's current target
  (vt+0x1690 = ctrl+0x994, kh-core `lockon.current`) every step; the link pawns re-read it each tick (FUN_1412AC290).
- Positions: KH3's actor location is the root's, at the FEET. The link pawn spawns at Sora's root (0x14127A780), so
  `Team::accompany()` / the link pawns are at Sora's feet (no +91 any more), and `CameraTargets::sora` must be Sora's
  `Output::position` (feet), not `capsule_center` (the camera base 0x140621E00 reads the root). kh-app party_glue.rs
  `team_camera` passes `sora.position` (one token, marked `(party D4)`).
- `kh_party::team::Team::mesh_pitch(MemberId) -> Option<f32>`: an extra UE pitch (degrees) of the member's MESH in its
  actor frame (Goofy's Bombardier rush, 0x1411CB010); kh-app party_draw.rs `team_mesh_pitch` applies it after `animate`.
- Team props: shaded by their own translated game shaders (tools/party_team_pack.js -> tools/sora_material_pack.js
  outfit mode -> build/sora_pack/party/team/materials, owner ids 300+, merged by char_material.rs as "team"), drawn as
  `char_material::Instance::Party(3)` (char_material.rs: Party(3) gets the world rig / KHS preset like 1 and 2); one
  entity per (kind, index), hidden when unused (no per-attack material leak).
- kh-camera: `camera_anim::sweep_start(view, pivot)` = the look point FUN_1405CD6F0 the collision sweep starts from;
  shield_01's `auto_speed_reference` = 200 (`BP_SetTargetMaxSpeed` = `m_TargetMaxSpeed` +0x890).

### D.8 D5 additions (2026-10-15, implemented)

- Goofy Bombardier: the riders sit on the link pawn's ANIMATED sockets (X_Link01_skt Sora, X_Link02_skt Goofy) of its
  own o_at001 clips 1636..1639 (root motion, RefPose lock), tables `build/gen/rust/party_team_gshoot.rs`
  (`node tools/party_team_pack.js --gshoot`, cfg `kh_party_team_gshoot`). The GoofyShoot link pawn's ACTOR rests 220 cm
  above Sora's feet (its 220 cm TresRoot sphere): `Team::accompany()` for 195 is there and moves with GShootHit's root
  motion. `FriendLinkView::sora` = Sora's socket (feet, yaw); `Team::sora_mesh_pitch() -> Option<f32>` = his extra UE
  pitch (kh-app party_draw `team_sora_pitch` turns `SoraVisual`); Goofy's rides `Team::mesh_pitch` as before.
  A host other than kh-app (kh-er) needs the same Sora pitch call to draw him pitched.
- `TeamProj::{base, shake, spin}`: Call Meteor's hover shake and spawn spin; a host drawing props uses `spin` (pitch,
  roll) instead of `forward` while it is Some.
- Heal effects: `kh_party::actions::recover_effect(kind)`, `Party::heal_effect_on(member, kind, effects)`,
  `Party::heal_effect_on_sora(from, kind, effects)`; members get theirs in `Party::step` (`Request::HealEffect`); a HOST
  handling `PartyOutput::sora_heals` must call `heal_effect_on_sora` (or `heal_effect_on` for a redirected heal).
- Audio: `kh_audio::Pack::merge_party_root(dir)` merges build/sora_pack/party/audio (`node tools/sora_audio_pack.js
  --party-extra`); kh-app calls it after `Pack::load`.


## Stream E: effects, sound, voice

Owner: stream E. Status: PUBLISHED 2026-10-15; the seam types below are IN THE CODE and compile (`kh-core` builds with
them; code is authoritative where it and this text differ). State note: `build/sora_tmp/party/IMPL_E_STATE.md`.
Files E owns: `crates/kh-core/src/host/party.rs` (new), the `Attach::Party` variant (host/mod.rs) and
`Audio::party_voice_group_no` (host/audio.rs), `crates/kh-app/src/party_fx.rs` (new), the `Attach::Party` arms in
`kh-app/src/{audio,vfx,markers,seam}.rs` and `kh-host/src/{seam,vfx,rig,audio/*}.rs`, `kh-audio` per-owner routes,
`tools/party_audio_pack.js` (new), the party additions to `tools/sora_audio_pack.js` / `sora_vfx_pack.js` (marked
`(party E)`), `sheets/port/party-sounds.json`, `party-effects.json`, `party-voice.json`.

### E.1 Who and where: `Attach::Party`

```rust
// kh_core::host (re-exported at kh_core::{PartyMember, Attach})
pub struct PartyMember(pub u8);   // FriendRelationDataTable index: DONALD = PartyMember(0), GOOFY = PartyMember(1)
                                  // = B's MemberId.0 (pass PartyMember(id.0)); A's draw ids are member.0 + 1
impl PartyMember { pub const fn uid(self) -> u8 /* 11 | 12 */; pub const fn chara(self) -> &'static str /* "n_ex001" */ }
pub enum Attach { World(Vec3), Sora { .. }, Target { .. },
    Party { member: PartyMember, bone: Option<&'static str>, offset: Vec3 } }   // NEW: bone/socket of THEIR skeleton
impl Attach { pub const fn party(member, bone: Option<&'static str>) -> Attach }  // offset 0
```

A sound or effect with `Attach::Party` follows that member, and the host resolves the seam NAME with that member's
routes: weapon aliases `SWINGn` / `HITn` in the member's EQUIPPED weapon's sets, motion sounds (`Walk`, `LandNormal`,
aux records) in `FsTable_Donald` / `FsTable_Goofy`, voice groups in `n_ex00x_VoiceGroupSet`. Everything else (asset
paths, effect names, actor classes with `ACTOR_PAWN_HIT` / `ACTOR_MAP_HIT`) works exactly as for Sora.

### E.2 Handles (party.rs)

```rust
pub struct SeamHandles;           // one per member: SeamHandles::for_member(m); team link pawns (D): SeamHandles::team()
impl SeamHandles { pub fn sound(&mut self) -> SoundHandle; pub fn effect(&mut self) -> EffectHandle;
                   pub fn play(&mut self, &mut dyn Audio, &SoundPlay) -> SoundHandle;
                   pub fn spawn(&mut self, &mut dyn Effects, &EffectSpawn) -> EffectHandle; }
pub const PARTY_HANDLE_BASE: u32 = 0x2000_0000;   // member m: base + m * 0x0100_0000; team: base + 15 * 0x0100_0000
pub const fn is_party_handle(h: u32) -> bool;  pub const fn handle_member(h: u32) -> Option<PartyMember>;
```

Party code (B, D) MUST take its seam handles from these ranges (Sora's core counts from 1; hosts key their live sounds
and effects by handle). Hosts: the "a new core counts from 1 again" reset applies to Sora's range only.

### E.3 Notify-driven sound / voice / effects of party clips (B calls, per member, in the pose tick)

```rust
pub struct PartyNotify;                       // party.rs; keep one per member (it remembers what notify STATES started)
pub enum SeamNotify {                         // the seam part of a notify (A's NotifyData has the same fields)
    Sound { sound, attach_point, volume, pitch, fade_out, loop_end_sound, loop_end_pitch },
    WeaponSe { alias, socket, follow, volume, pitch, fade_out },
    AutoSe { name /* motion or source */, fade_out },
    Voice { name /* asset, group, or FIRST entry of the random list */, fade_out },
    ActorVoice { asset /* TresAnimNotify_PlayActorVoice m_ActorVoice, from the notify's props */ },
    Effect { effect, socket, group_id },      // ONE per AttachEffect row (B flattens a notify's entries)
}
impl SeamNotify { pub fn of_core(&kh_core::anim::NotifyData) -> Vec<SeamNotify> }  // for kh_core's own type
pub struct PartyNotifyEvent { pub sequence: u32 /* per-member key of the clip */, pub index: u16, pub state: bool,
                              pub phase: NotifyPhase, pub data: SeamNotify }
impl PartyNotify {
    pub fn dispatch(&mut self, member, ids: &mut SeamHandles, events: &[PartyNotifyEvent], location: Vec3 /* actor */,
                    effect_group: i32 /* 0 unless the state set one */, audio: &mut dyn Audio, effects: &mut dyn Effects);
    pub fn stop_all(&mut self, audio, effects);   // member removed / warped out
}
```

Same rules as Sora's notify.rs: SEAD PlaySound / AttachSound (state: loop until End, then LoopEndSound), weapon swing /
hit aliases (attached when `m_bFollow`, else at `location`), PlayAutoSe (motion name), PlayVoice (group / asset / first
random entry, at `atama`), AttachEffect rows (effect-group rule: the under-water rows of the shared clips carry
group 1088 ... and do not spawn in group 0; persistent for a state). A's `party_anim_types.rs` NotifyData has the
same fields as kh_core's, so B's mapping is one match (Sound, WeaponSe, AutoSe, Voice, Effect entries; `None` data of
class `TresAnimNotify_PlayActorVoice` -> `ActorVoice` from its `m_ActorVoice` prop). B's `PartyEventKind::Notify` stays
useful for logs; E does not consume it.
### E.4 Voices (party.rs): what B / D call, and what the host does

```rust
pub enum VoiceGroup { QuarterHp, RecoveryHp, FriendLink, BattleStart, MagicCast /* Donald */, GiveRecovery, Damage }
pub enum VoiceMode { Replace, IfIdle, Overlap }        // FUN_141275aa0 param_4 / param_5
pub fn voice_play(member, group, number: i32, mode) -> Option<SoundPlay>   // name "QuarterHP:2", at atama, id = mode
pub fn pick_number(count: i32, r: u32) -> i32          // 1 + min((r>>8)*2^-24*count, count-1): the game's pick
pub const VOICE_VARIATIONS: i32 = 2;                   // vtable slots 1318/1319/1321/1323 = `mov eax,2; ret` [I]
pub fn magic_voice_number(member, r: u32) -> i32       // Donald 101..103 (slot 1325, 0x1411C8E00); Goofy 101: a NUMBER
pub fn target_voice_number(member, target_uid, random: bool, r) -> i32  // slot 1326 (Donald 0x1411C8ED0): Sora 1, Goofy 2 ..
// the pawn's own voice group number (anim interface +0x1F0) for the voice-GROUP notifies of its clips:
trait Audio { fn party_voice_group_no(&mut self, member: PartyMember, number: i32) {} }   // defaulted, NEW
```

`r` = the next u32 of the member's own deterministic RNG stream (plan N6). The game's triggers (B implements them in
the pawn, E's state note lists the C read so far): QuarterHP after a damage application with 0 < hp_after < hp_before
and hp_after < CeilToInt(max_hp * 0.25) and the voice gate FUN_141267740 (0x1412795F0) -> `voice_play(m, QuarterHp,
pick_number(2, r), Replace)`; RecoveryHP on a heal from ANOTHER character of effect type 3, 4, 6, 7, 8, 9, 14 or 16
(0x141276C10); BattleStart (0x141275210, `IfIdle`); FriendLink on a team command (Donald 0x1411CED00: Meteor (207) ->
1..2, Flare Force (208) -> 3..4; D). The host keeps ONE current voice line per member: `Replace` stops it first,
`IfIdle` plays nothing while it lasts, `Overlap` leaves it.

CAST VOICES ARE NUMBERS, NOT PLAYS (0x14122D300 [C] with the clip data [A]): before Donald's cast clip the cast state
sets `audio.party_voice_group_no(m, n)` with n = `target_voice_number(m, target_uid, false, r)` for a Cure on ANOTHER
character (Sora -> unit 1, Goofy -> 2), `magic_voice_number(m, r)` (101..103) for an attack-magic cast that uses the
MagicCast clip, 0 for a Cure on himself; the clip's "MagicCast" GROUP notify plays the unit (none for 0). The
MagicFire clip (1321) has its own random-voice notify (the same 101..103 lines): no extra play. Items likewise: the
ItemUse clip's "GiveRecovery" group notify plays the unit of the number set before it (Donald: by target, 1 Sora /
2 Goofy; Goofy: 1..2 at random) [inf: the item state's call of slot 1326 was not read].

### E.5 Spell actors and effects

Party spell actors (B) are named on the effect seam by their blueprint class, exactly as Sora's: `EffectSpawn { name:
"n_ex001_ProjectileMagFire_C", persistent: true, at: Attach::World(spawn point) }` for the body (the host follows it
through `party_fx::publish_projectiles`, below), a second spawn with `id: EffectId(kh_core::magic::projectile::
FIRE_EFFECTS_ID)` for its MyFireEffects, the hit sound by `SoundPlay { id: ACTOR_PAWN_HIT | ACTOR_MAP_HIT, name: class }`
and plain particle names for everything else (`vn_ex001_fir_hit00` ...). E packs every class's effects and sounds
(audio `tables.json` projectiles, vfx `host.json`); the table class -> roles is E's `sheets/port/party-effects.json`.

### E.6 kh-app side: `crates/kh-app/src/party_fx.rs` (F / A fill it once per fixed step)

```rust
pub struct MemberTap { pub step: u64, pub position: Vec3, pub yaw: f32,
    pub bones: HashMap<String, (Vec3, Quat)>,   // EVERY bone AND socket of the member's skeleton + weapon sockets, world, KH space
    pub grounded: bool, pub anim: (String, f32) /* anim-set row name, time */, pub weapon: String /* "w_do010" */,
    pub visible: bool }
pub fn publish_member(member: PartyMember, tap: MemberTap);  pub fn clear_member(member: PartyMember);
pub struct ProjectileTap { pub effect: u32 /* body effect handle */, pub position: Vec3, pub forward: Vec3 }
pub fn publish_projectiles(list: Vec<ProjectileTap>);
```

The audio side runs the auto-SE detector per member from `bones` (KH3: `SQEXSEADAutoSeComponent` with `BoneSet_Sora1`
and `FsTable_Donald` / `FsTable_Goofy` on both pawns [A]) and resolves `Attach::Party` with them; the vfx and marker
sides place effects with them.

## Stream F: HUD, camp menu, test-app integration

Owner: stream F. Status: PUBLISHED 2026-10-15 (phase 0; `kh-host/src/party.rs` compiles; code is authoritative where it
and this text differ). State note `build/sora_tmp/party/IMPL_F_STATE.md`. Files F owns: `kh-host/src/hud.rs` (party
parts), `kh-host/src/party.rs` (NEW), `kh-ui` camp files (`camp.rs`, `camp_menu.rs`, `camp_equip.rs`,
`camp_custom.rs`), `kh-app/src/{main.rs, player.rs, script.rs, fixtures.rs, loadout.rs, menu_model.rs}`,
`kh-app/src/party_glue.rs` (NEW: the ONE place the app touches A..E's party APIs), `sheets/port/party-test.json`.

### F.1 HUD feed (`kh_host::party`, any host, every drawn frame)

```rust
pub enum PartyChara { Sora = 0, Donald = 1, Goofy = 2 }   // = HUD status slot = level-up window `chara`
pub struct PartyHudSlot { pub chara: PartyChara, pub hp: i32, pub hp_max: i32, pub mp: i32, pub mp_max: i32,
                          pub mp_charging: bool, pub mp_charge_value: f32, pub in_battle: bool }
impl kh_host::hud::Hud {
    pub fn set_party(&mut self, slots: &[PartyHudSlot]);                     // empty = Sora alone (solo layout)
    pub fn party_level_up(&mut self, chara: PartyChara, level: i32, lines: &[String]);  // HudRight window (queued)
}
```
Faces (`FaceNpcExDonald` / `FaceNpcExGoofy`), looks (death < 1 HP / dying / battle / normal), the damage flash (HP went
down) and the party / solo layout are the HUD's own driver (kh-ui `hud_status.rs`).

### F.2 Camp menu party data (`kh_host::party`)

The camp menu does not know B's types: the host (kh-app `party_glue.rs`, later the ER host) hands it plain snapshots and
applies the menu's edits back through B's `FriendStatus`:

```rust
pub struct PartyMenuMember {          // one friend, in party order (Donald, Goofy)
    pub chara: PartyChara, pub level: i32, pub exp: i32, pub exp_level: i32, pub exp_next: i32 /* 0 at 99 */,
    pub hp: i32, pub hp_max: i32, pub mp: i32, pub mp_max: i32, pub ap_max: i32, pub ap_in_use: i32,
    pub strength: i32, pub magic: i32, pub defense: i32, pub resists: [i32; 6] /* Fire Blizzard Thunder Water Aero Dark */,
    pub weapon: String /* "WEP_DONALD00" */, pub armour: Vec<Option<String>>, pub accessories: Vec<Option<String>>,
    pub items: Vec<Option<kh_core::status::ItemKind>> /* the open item slots */,
    pub abilities: Vec<(kh_core::enums::ETresAbilityKind, u8 /*owned*/, u8 /*equipped mask*/)>,
    pub ai: FriendAiSettings,
}
pub struct FriendAiSettings { pub battle: u8, pub ability: u8, pub cure: u8, pub cure_targets: u8 } // enum VALUES (C.4)
pub enum PartyMenuEdit {              // what the menu changed; the host applies each to B's FriendStatus, then rebuilds
    Weapon { chara, id: String }, Armour { chara, slot, id: Option<String> }, Accessory { chara, slot, id: Option<String> },
    ItemSlot { chara, slot, item: Option<ItemKind> }, Ability { chara, kind, copy: u8, on: bool },
    UseItem { chara, item: ItemKind }, Ai { chara, settings: FriendAiSettings },
}
impl Hud {
    pub fn set_party_menu(&mut self, members: Vec<PartyMenuMember>);   // each frame (cheap) or when it changed
    pub fn take_party_edits(&mut self) -> Vec<PartyMenuEdit>;          // after `frame`; the host applies them
}
```
The stock (spare staffs / shields, armour, accessories, battle items) is the HUD's shared inventory (`Hud::inventory`),
as for Sora; a weapon taken out of the stock is given back by the HUD when replaced.

**Friend AI save layout (read, [C]):** record +0x158 Combat Style (setter 0x140E97230), +0x159 Abilities (0x140E97220),
+0x15A Recovery (0x140E97240), +0x15B bits 0..2 targets On Sora / On Self / On Others (writer 0x1415EA860). The
game's Restore Defaults (0x140E94550): Combat Style 0 (Stick By Sora), Abilities 0 (Go Wild), Recovery 1 (Use in
Emergencies), all three targets on. (C: these differ from N4's guessed defaults; they are what the Customize screen's
Restore Defaults writes. The game's new-game init of these bytes was not traced.)

### F.3 Test-app glue (kh-app; F only)

Step order per fixed step: Sora `step_hosted` -> C `AiController::tick` per member -> B `Party::step` -> dummies'
reactions -> camera (D's camera anims on top when they own the view) -> poses (A's `set_pose`). Combined targets: the
dummies are the `Targets` of Sora's AND the friends' hits; Sora's hits never land on a friend and vice versa (the
friends are not in the dummy list). Dummies can attack the party: `dummy-auto` hits pick the victim by
`dummy-target N sora|donald|goofy|near` (default near: the nearest living of Sora and the friends) and hit a friend
through `Party::receive_hit`. Options: `--party` (default ON), `--no-party`. Script words (script.rs):

| word | what |
|---|---|
| `party on` / `party off` | spawn / remove both friends |
| `party-hp donald\|goofy N`, `party-ko donald\|goofy` | set HP (0 = KO) |
| `party-style donald\|goofy together\|free\|support\|focus` (+ `party-ability ...`, `party-cure ...`) | AI settings, as Customize |
| `party-trace on\|off` | C's `AiController::trace` printed when it changes |
| `team-gauge full` | D's friend gauge to max (D: name the call) |
| `party-word A\|B\|C\|D\|E WORDS...` | GENERIC PASS-THROUGH to that stream's `debug_word(&mut self, words: &[&str]) -> Option<String>` (A: `PartyDraw`, B: `Party`, C: `AiController` (member given as first word), D / E: name yours). Expose such a function instead of asking F for each word. |

Pad: every party feature is driven by the game's buttons (camp menu: D-pad / A / B / Y / LT / RT; team attacks are D's
situation commands on Y). View (reset) also restores the party (full HP / MP, revive).

**F.4 Status (2026-10-15, implemented in party_glue.rs):** Sora's targets = `party_glue::SoraTargets` (dummies + the
friends as `allies` with ids `0x4000_0000 + MemberId.0`, `heal_ally` applied to the party before its step, `friends` /
`friend_link` to D's `Team`); per step: Sora -> AI ticks (C) -> `Party::step` -> `Team::step` -> poses
(`PartyDraw::set_pose_view`, member = MemberId.0) -> camera (+ D's `StateEventCameras` on top; FOV track not applied).
Pass-through words: `party-word A ...` -> `PartyDraw::debug_word`; B / C / D / E: expose `debug_word` and tell F.
`team-gauge full [KIND]` = Sora's `friend.debug_fill_gauge()` + `Team::debug_ready_all()` (+ `debug_offer(KIND)`).
Team cameras: `accompany` = `Team::accompany()`, the CameraAnim FOV replaces the level camera's while a team camera
owns the view, `out.shake` is added (camera.rs). Projectiles are published to `party_fx` with B's `Projectile::effect`.
Friends' armour / accessories / slot counts / cure targets / resists are B's `FriendStatus` (F keeps none).
Level-up windows: HudRight's five (`McLevelUp` + 4 instances), stacked; camp pictures of Donald / Goofy: A's menu
copies (menu_model.rs `update_friends`, captures of n_ex001 / n_ex002_UIActor).

## Stream P: playable Donald / Goofy, Party Formation

Owner: stream P. Status: 2026-10-15, implemented. State note `build/sora_tmp/party/IMPL_P_STATE.md`. Files P owns:
`crates/kh-party/src/pad/**` (B3 granted), `crates/kh-app/src/party_play.rs`, `crates/kh-host/src/formation.rs`, plus
marked `(party P)` edits listed in the state note. Code is authoritative where it and this text differ.

### P.1 kh-party `pad` (engine-free)
```rust
pub struct PadFrame { stick: Vec2 /* raw left stick */, camera_yaw: f32, pad: kh_core::PadState }
impl Party {
    pub fn set_pad(&mut self, id: MemberId, tuning: Option<kh_core::Tuning>);   // Some = the pad drives it, None = the AI
    pub fn set_pad_frame(&mut self, id: MemberId, frame: PadFrame);           // every step, BEFORE Party::step
    pub fn pad_menu(&self, id) -> Option<kh_core::command::CommandSnapshot>;   // the friend's command menu for the HUD
    pub fn pad_state(&self, id) -> Option<&PadState>;                          // lock / soft target, LocoPose, combo
}
pub struct LocoPose { walk, walk_time, run: [(seq, w); 2], run_time, run_weight }   // dec016 drawing (kh-app pose_loco)
```
States `NpcStateId::Pad(1 RUN | 2 JUMP | 3 AIR_ATTACK)`; attacks / spells / items go through B's `MemberCommand`.
Unison on the leader's combo: `PadState::finisher_pending` -> the host rolls `FriendState::roll_leader_unison` (D.6) ->
`finisher_answer`.

### P.2 kh-host `formation`
`Formation { active (1..=3, slot order), leader (always active) }`: `new`, `toggle`, `set_leader`, `shift`, `to_save` /
`from_save`; `party_record(f, ai)` / `ai_of_record(v)` (record format 2 with the AI settings by character); `ROSTER`
(data-driven). Saved in `SoraSave::party` (save version 4).

### P.3 kh-host `Hud` additions
`set_leader(chara, sora_in)` (main slot = the leader, shrink / grow on the slots' out / in labels), `set_formation`,
`open_party_formation(&f)` (the camp menu's character step; only the save point opens it), `take_formation_change`,
`take_formation_closed`, `camp_panels`, `set_ai_members(list)` (Customize's dynamic friends list; Sora's entry while a
friend leads; edits come back as `PartyMenuEdit::Ai { chara: Sora, .. }`).

### P.4 kh-ui additions
`SavePointCommand::PartyFormation` (+ `PARTY_FORMATION_TEXT`, `MAX_COMMANDS` 7: a seventh slot cloned from `cmd6`),
`Movie::clone_child`, `TopEntry::PartyFormation`, `FormationRows`, `CampMenu::open_formation / formation_answer`,
`CampAction::Formation { row, lead }`, `StatusDriver::reload_face`.

### P.5 (P2, 2026-10-15) the party in prizes, the save point and EXP (additive; old calls unchanged)
- kh-prize: `Prizes::step_party(dt, world, getters: &[Getter], takes: &dyn Fn(slot, PrizeClass) -> bool)` (FindGetter
  0x1414794C0 over the slots: nearest inside his own radius); `Prize::getter`; `PrizeEvent::GetStarted / Collected`
  carry `getter: u8` (patterns with `..` are unaffected).
- kh-host prize: `PrizeMember`, `PrizeParty::apply(slot, PrizeKind)`, `member_takes` (0x141259A50),
  `prize_recipient` (Share Prizes 0x14147A070), `PrizeHost::step_party(dt, world, sora, members, point, party)`.
- kh-host savepoint: `SavePoint::step_checked(dt, sora, capsule, needs) -> SavePointStep` (the host gives
  `Recover(n, n, n, 1)` to the pawn and his party itself; `step` = Sora alone, as before).
- kh-core: `status::ExpPlayer`, `Status::adjust_exp_for(exp, player)`, `Status::give_exp_gain(reward, gain)`,
  `StatusRequest::GiveExpAs { base_exp, rate, position, player, panel }` ("player 0" = the controlled leader).

### P.6 (P3, 2026-10-15) notify-state ends, spell effects, the thrown shield (additive)
- kh-party anim: `NpcAnim::cut` / `take_cut()`. A play that replaces a clip records that clip's open notify states;
  `NpcPawn::end_cut_states(host)` (called in the step before the pose tick) sends their End through the seam, so looping
  sounds and persistent effects stop whatever replaced the clip. `NpcPawn::play_looped(name, blend)` = PlayAnim mode 1 / 3.
- kh-party party: `Party::stop_removed(audio, effects)` (also run at the start of `step`). A removed member's running
  notify loops stop. A host that drops the whole party calls it once. `Party::thrown_shield(id) -> Option<&Projectile>`
  and `Party::weapon_hidden(id)` feed the weapon drawing: put the weapon at the shield while it is out (its spin clip
  `anims/1328.glb` on "wep"), and hide it until the catch's SetWeaponVisible.
- kh-party projectiles: the spell actors now spawn their body effect (persistent, `Projectile::effect`) and their
  FIRE_EFFECTS_ID event at the cast point, with the class names `ProjectileKind::class()` ("n_ex001_ProjectileMagFire_C"
  ...: party_host.json). They also spawn `ProjectileKind::hit_effect()` on a pawn hit. The cast effect of each spell (group
  `magicset.<spell>.effect_group`, vn_ex001_*_cal00 at vfx_call) is spawned by the action's Begin. New fields:
  `Projectile::free`, `ActionRun::free_throw` (a pad Goofy's Discus with no target: out and back, caught on return).
- ER hosts: publish `StepOut.projectiles` (the bodies now have handles) and draw the thrown shield from
  `thrown_shield` / `weapon_hidden`, as kh-app party_draw.rs `thrown_shield` does.


## ER: the engine-free party host (`crates/kh-partyhost`, stream S0 of the Elden Ring party phase)

Owner: ER party stream S0 (build/sora_tmp/er/party/ER_PARTY_PLAN.md; state note
build/sora_tmp/er/party/IMPL_S0_STATE.md). The test level's party host (kh-app party_glue.rs, party_play.rs and the
pose half of party_draw.rs) moved here unchanged in behaviour; kh-app (the Bevy test level) and kh-er (S1 onward)
drive the same code. Deps: kh-core, kh-party, kh-ai, kh-host, kh-anim, kh-camera, glam, serde_json. No Bevy, no files,
no clocks: the host reads packs / sheets and hands them in. Log lines go through `kh_partyhost::log` (stdout unless
the host calls `log::set_logger(Some(Box::new(|line| ...)))`; lines are said at once, in call order).

### ER.0 One fixed step (the test level's order, kh-app player.rs; kh-er follows it)
1. `rt.route(RouteIn { sora, input, dt, targets, camera, asked_formation, asked_ai, nav, now }) -> Input` BEFORE
   Sora's step: the leader words, a formation the HUD / a load asked for, AI settings a load asked for, the formation
   applied (friends spawned / removed, the pad on the leader, Sora active or parked), the leader's unison roll, the
   pad frame to a leading friend; returns Sora's input (the pad's when he leads, AI Sora's otherwise; Y / LT of the
   pad always reach Sora while a friend leads: only the leader starts team attacks).
2. The host steps Sora (when `rt.lead.sora_active`) with `SoraTargets { targets: &mut host_targets, runtime: &mut rt }`
   (allies = the friends at `ALLY_BASE + member`, `heal_ally` queued for the party, `friends` / `friend_link` = Team).
3. `rt.step(StepIn { dt, sora, out, lock_on, camera, world, targets, audio, effects, nav, now }) -> StepOut`: Sora's
   team abilities once, spawn / despawn, the shared EXP (`level_ups`), Sora's heals on friends, the AI ticks (the
   leader's tree is not ticked; the trees' player = the leader), `Party::step`, `Team::step`, the party's heals on
   "the player" (Sora, or the leader), the event log. `StepOut { projectiles, stepped }`.
4. `rt.sora_look_point = <Sora's "t_camera" bone>`; `rt.pose(&mut poser, ik_world, dt) -> Vec<MemberTap>`.
5. `rt.after_step(&targets, dt, now) -> Option<LockChange { target, at }>`: the leader's state / capsule; a change of
   the leader's RB lock for the lock-on camera (the host applies `CameraRig::set_lockon(&target, at)`).
6. After the host's field camera: `rt.camera_target(dt)` (a friend leading: his field-camera target, else None = Sora's)
   and `rt.team_camera(dt, events, sora_out, &mut view, world, stick) -> bool` (team CameraAnims; true = put the field
   camera behind Sora again). `rt.team_fov` / `rt.team_shake` = what the drawn lens adds.

The step is paused with Sora (menus, pause, load): the host simply does not call 1-6.

### ER.1 `PartyRuntime` (runtime.rs)
- `PartyRuntime::new(RuntimeConfig { wanted, test: PartyTest, weapons: Vec<PartyWeapon>, ai_pack, ai_pack_source })`.
  `PartyTest::from_rows(&rows)` / `PartyWeapon::from_rows(&rows)` read sheets `party-test` / `party-weapons` rows.
- Public state: `party: kh_party::Party`, `members: Vec<(PartyChara, MemberId)>` (HUD order; Donald id 0, Goofy 1),
  `current` / `previous: Vec<MemberView>`, `leader: Option<MemberId>`, `active_friends`, `team`, `cams`, `level_ups`,
  `pending_heals`, `ai`, `ai_pack`, `trace`, `wanted`, `lead: Leadership` (ER.2), `team_fov`, `team_shake`,
  `sora_look_point`.
- Members: `id(chara)`, `chara_of_id(id)`, `view(chara) -> Option<MemberView>`, `spawn(&Output, total_exp)`,
  `spawn_one(chara, at, yaw)`, `remove_one(chara)`, `despawn()`, `ai_of(id)`.
- AI settings: `ai_settings(chara)` / `set_ai_settings(chara, FriendAiSettings)` with the D5 `pending_ai` rule (a friend
  slotted out keeps his settings; settings given before he exists wait for him); `sora_ai_settings()` /
  `set_sora_ai_settings(s)` (AI Sora's Customize entry); `ai_settings_of(FriendAiSettings) -> kh_ai::AiSettings`.
- Hits / heals: `receive_hit(chara, &HitRecord) -> Option<HitResult>` (damage already final: contract B.3),
  `heal(chara, hp, effect) -> Option<i32>`, `recover()` (HP / MP full), `hit_friend(..)` (the test dummies' fixture
  blow), `victim(dummy, from, sora)` / `set_victim` (the test level's `dummy-target`).
- Words: `word(text) -> String` (party on|off, party-hp, party-ko, party-style / -ability / -cure, party-trace,
  party-recover, dummy-target), `lead_word(&[&str])` (leader WHO, leader warp X Y, formation in|out|lead WHO, reset).
- `ai_camera(&CameraView) -> kh_ai::CameraView` (the drawn lens), `member_mps()`, `status_text()`.

### ER.2 The leader (lead.rs, seam.rs)
- `Leadership` (`rt.lead`): `formation`, `applied`, `sora_active` (Sora is stepped / drawn), `sora_settings`, and the
  values the test level kept in process globals for its other screens: `sora_in`, `current: Option<Formation>`
  (`current_formation()`), `all_ai` (every character's AI settings, for the save), `leader_state: (PartyChara, KO)`,
  `leader_capsule`. `rt.set_formation(f)` (applied at the next `route`), `rt.friend_leads()`, `rt.leader_chara()`,
  `rt.leader_lock()`, `rt.leader_capsule()`, `rt.leader_view(SoraView) -> SoraView` (the party's "player").
- `LeaderView { chara, position, yaw, velocity, on_floor, capsule: (r, half, centre z), invulnerable, in_fr, ko,
  airborne, state, pitch, member }` = `rt.leader_view_of(&sora_output, &sora_status)` every step (Sora's Output when he
  leads; the leading friend's MemberView otherwise). Sora's `invulnerable` is `Status::damage_invalid`: the ER host ORs
  its own dodge-frame rule. Capsules: Sora `SORA_CAPSULE` (34 / 89 / 91), friends from `MemberView.capsule`.
- `trait LeaderSink { take_hit(&LeaderHit), follow_body(at, yaw), die() }`; `RuntimeLeaderSink { runtime, sora }`
  implements it: Sora -> `Sora::take_hit(HitOnPlayer)` / `teleport` / kh-er's drown blow; a friend leader ->
  `Party::receive_hit(record)` / `cmc.location` + `stop_movement` + yaw / a lethal record. `LeaderHit { sora:
  Option<HitOnPlayer>, record: HitRecord }` (the host fills the one its leader needs).

### ER.3 The AI-world inputs (aiworld.rs)
- `trait PartyTargets: kh_core::Targets { ai_actors(&self, out: &mut Vec<kh_ai::ActorView>); attacked_by_enemies(&self)
  -> bool }`: the SAME object is the friends' hit targets and the AI's enemy list. `ai_actors` lists every enemy-side
  actor in the host's order, `actor: ActorRef::Enemy(target id)`, `center` = its look-at / lock-on point, bystanders
  with `hostile: false` (left out of the trees' enemy list, still found by id). The test level implements it over its
  dummies (kh-app party_glue.rs); ER (S1) over ErTargets.
- The runtime builds the rest: `SoraAiWorld` (Sora leads: Sora = player, focus gauge, member MP) and `LeaderAiWorld`
  (a friend leads: he is the player; AI Sora an ally `Member(SORA_ACTOR = 7)`), both over an `EnemyFeed` snapshot
  taken before the ticks. Navigation: `StepIn.nav` / `RouteIn.nav` (None = `STRAIGHT_NAV`, the straight line; S1 gives
  the breadcrumb Nav). `member_actor(&MemberView)` is the allies' view.

### ER.4 The drawing feed (poser.rs)
- `PartyPoser { members: BTreeMap<u8, PosedMember>, menu: BTreeMap<u8, MenuPoser> }` (draw ids `DONALD` 1, `GOOFY` 2;
  `draw_id(chara)`, `npc_of(id)`): the host keeps one and passes it to `rt.pose`.
- `PosedMember::new(id, kh_anim::Animator, &manifest_json, &party_json_entry)`; `weapon_def(item)`,
  `mount_weapon(&def, &weapon_manifest_json)`; per step `set_pose_view` / `step` (called by `rt.pose`; the debug rows:
  `debug_pose(row, speed)` + `step_debug(n, dt, world)`); `bone_world(bone)` (bones, then sockets),
  `weapon_socket_world(socket)`; `previous` / `current: kh_anim::Pose`, `position` / `yaw` / `pitch` pairs (previous,
  latest) for interpolation.
- `PartyPoser::pose(id, alpha) -> PosedChar { component_to_world: Trs, local: Vec<Trs>, component: Vec<Trs>, weapon:
  Option<(model, root in component space, visible)> }` (alpha 1 = the latest step). `component_to_world_pitched(pos,
  yaw, pitch)` = the actor (pitch, yaw, 0) then the mesh's -90 yaw (KH3 space, cm).
- `MenuPoser::new(npc, animator, &manifest)` (the camp copies; `play_row`, `step(dt) -> wrapped`; the eyes fix: the
  `<row>_Face` rows on both face slots, replayed at each loop).
- Per member per step a `MemberTap { member, step, position, yaw, bones: name -> (world pos, rot), grounded, anim: (row,
  time), weapon }` (bones, sockets, `wep:<socket>`): the host publishes it for `Attach::Party`, footsteps and weapon SE.

### ER.5 The ally bodies' want list (seam.rs)
- `rt.ally_wants(&sora_output, &sora_status) -> Vec<AllyWant { chara, feet, yaw, capsule, hittable, invulnerable, ko }>`:
  every friend but the leader, plus AI Sora while a friend leads and Sora is active. `hittable` = not KO;
  `invulnerable` = Die / Warp / FR (Sora: `damage_invalid` or an Fr state). S3's pool syncs to it; its `AllyHit`s come
  back through `rt.receive_hit(chara, record)` (AI Sora's: `Sora::take_hit`).

### ER.6 The save record (save.rs)
- `PartySave { formation, ai: Vec<(PartyChara, FriendAiSettings)>, friends: Vec<FriendRecord> }`;
  `FriendRecord { chara, weapon, armour, accessories, items: Vec<(Option<item name>, present)>, item_slots,
  armour_slots, accessory_slots, abilities: Vec<(game name, owned, equipped bit mask)>, bonuses: [u8; 32] }`
  (`FRIENDS_RECORD_VERSION` 5; the level is derived from the shared EXP, not stored).
- `rt.to_save()` (live statuses; a friend slotted out: his last record), `rt.from_save(&save)` (formation / AI at the
  next route; records onto spawned friends now, kept for the others until they spawn). `FriendRecord::to_json()` /
  `from_json(&Value)` = the v5 JSON shape (`{chara, weapon, armour, accessories, items: [[name, present]], slots:
  [item, armour, accessory], abilities: [[name, owned, mask]], bonuses: [32 counts]}`) for S6 to embed in save.kh's
  `party` record (`party.friends[]`). The v4 part stays `kh_host::formation::party_record(&formation, &all_ai)`.

### ER.7 The audio / vfx event feed
- Every party sound / voice / effect of a step goes through `StepIn.audio` / `StepIn.effects` (the host's kh_core
  seams; `Attach::Party { member, bone, offset }`, member = `MemberId.0`), including `heal_effect_on` /
  `heal_effect_on_sora` (D5) and the team attacks'.
- `StepOut.projectiles: Vec<ProjectileTap { effect, position, forward }>` (the spell actors' and team props' body
  effects follow them), to publish only when `stepped`; `rt.team.projs` for the prop models; the members' bones from
  the ER.4 taps.

### ER.8 The HUD feed
- `rt.hud_slots() -> Vec<PartyHudSlot>` (contract F.1), `rt.menu_members() -> Vec<PartyMenuMember>` (F.2),
  `rt.level_ups` (drain; `(chara, level, [Strength, Magic, Defense, AP] gained)`), `rt.leader_chara()` + `rt.lead.sora_in`
  (`Hud::set_leader`), `rt.lead.current_formation()` (`Hud::set_formation`), `rt.ai_members()` (Customize's dynamic
  list: Sora while a friend leads), `rt.hud_snapshot(&base) -> Option<UiSnapshot>` (a friend leading: his command menu,
  lock and soft target), `rt.lead.leader_state.1` (a fallen friend leader = game over).
- Edits: `rt.apply_edit(&PartyMenuEdit)`; an `Ai` edit of Sora's entry goes to `rt.set_sora_ai_settings`. Weapon pieces
  for the Equipment screen are the host's (kh-app builds them from `rt.weapons`).

### ER.9 The ally ladder climb (kh-party, PORT DESIGN: KH3 has no ladders)
> SUPERSEDED 2026-10-25 (LADDER2): ladders are no longer climbed. Y at a ladder makes the leader and the party jump up to
> the ladder's top exit in one leap (kh-core `leap.rs`, Torrent's wind sound: kh-er `ersound.rs`); the climb states, the
> climb capture (`climbcap.rs`) and the climb animations are deleted, and going down a ladder is disabled. Kept below as
> the record of the old design.
- `kh_party::LadderInfo { bottom: Vec3, top: Vec3, facing: f32 }` (KH3 space, the actor positions at the lowest /
  highest climbing point, the yaw a climber faces) and `MemberCommand::ClimbLadder(LadderInfo)`; state
  `NpcStateId::Ladder` ("TresNpcState_Ladder (port)", AI name "TSID_STATE_LADDER_PORT").
- Enter (0.25 s onto the near end, turned to `facing`), climb (up 120 / down 160 cm/s, no input), exit (top: a 60 cm
  step forward over 0.4 s; bottom: none); then Succeeded and Idle. Refuses every command but `Stop` (off the ladder
  where he is) and `Warp` (the host's 8 s stuck fallback, U12). Hits still land (his reaction takes him off).
- `Party::ladder(id) -> Option<LadderRun { info, sub, up, along, length, t, .. }>` (`height_fraction()`): the host's
  replayed climb pose (S2's capture) reads it; the state itself plays the idle clip. Paces are `ladder::CLIMB_UP` etc.
  [D] (S1 may move them to its sheet).
- (ER S1, additive) a pending `ClimbLadder` (given by the host, not yet taken at this step's intake) wins over every later
  command of the same step but `Stop` / `Warp`, as a pending `EnterFrState` does (the AI's tree re-issues its MoveTo at
  once when the host's command aborts it).

### ER.10 The Elden Ring party host (kh-er `src/party/`, stream S1)
Owner: ER party stream S1 (state note build/sora_tmp/er/party/IMPL_S1_STATE.md). Everything is behind the kh-er cargo
feature `party` (default on) and the key `party.er` (default on; 0 = Sora alone, exactly as before the party).
- WHERE: `SoraHost.party: crate::party::PartySlot` (sora.rs). `slot.core() / core_mut() -> Option<&PartyCore>` (None while
  `party.er = 0` or before it is made), `slot.runtime_on() -> Option<&mut PartyRuntime>`. `party/mod.rs` is S1's: it
  declares S2's `leader` / `climbcap`; S3 reaches its files by `#[path]`.
- LIFETIME ("they need to be able to be anywhere at all times, they should never be forced to despawn or break"): ONE
  `PartyCore` per game run, kept in a static between Sora hosts (loads, deaths, fast travel, the title, a new game); the
  host never removes a member. A new Sora host, a relocation of the player, the end of a cutscene, `party.er` back on:
  RESEAT (every AI friend at his home place beside the leader, AI Sora too while a friend leads; the trail cut).
- THE ORDER (sora.rs, all wired by S1): once a frame `slot.frame(player, control, frame, relocated, hidden, body_action,
  dt, now)` after the follow phase; per fixed step `slot.route(..)` (ER.0 step 1) -> `slot.step_sora(..)` (step 2: Sora with
  `SoraTargets`; NOT stepped while he is out of the active party: his output stays) -> `slot.after_sora_step(..)` (steps
  3 - 5: waypoints, `rt.step`, `rt.pose`, `rt.after_step`, the trail, the fallbacks, the want list) -> S2's
  `lead.update(rt, out, status, dodge)` + `lead.apply(rt, sora)` -> `core.counter_move(seat)`. Paused with Sora.
- PARK: while a real-time cutscene plays or Sora waits for the ground after a load / warp (`Park::Hidden(why)`): nothing is
  stepped or drawn; `feed.wants` = every AI ally at the LEADER'S feet, `hittable: false`, `invulnerable: true` (U6).
- THE FEED, after every party step: `core.feed: StepFeed { wants: Vec<AllyWant>, taps: Vec<MemberTap>, projectiles:
  Vec<ProjectileTap>, lock_change: Option<LockChange>, stepped }` (empty while `party.er = 0`).
  - S3: `feed.wants` (ER.5); the body hits back with `core.queue_hit(AllyHitIn::Friend(chara, HitRecord) |
    AllyHitIn::Sora(HitOnPlayer))` (applied at the start of the next party step, with a `party:` line for a KO).
  - S4: draw only while `core.visible()`; `core.poser.pose(draw_id, alpha)` (S1 loads the posers once and mounts each
    friend's equipped weapon, `FriendStatus::weapon`); `core.rt.current / previous`; an ally on a ladder:
    `rt.party.ladder(id)` + S2's `lead.climb_pose(..)`.
  - S5: `feed.taps` (bones / sockets / `wep:` sockets per member per step, for `Attach::Party`, footsteps), the step's
    sounds / effects already went through `SoraHost.fx.audio / effects`; `feed.projectiles`.
  - S6: `core.rt` (HUD feed ER.8, `apply_edit`, `to_save` / `from_save`); a formation / AI settings from the menu or a load:
    `core.asked_formation = Some(f)` / `core.asked_ai.push(..)` (taken at the next `route`).
  - S7: `core.rt.team`, `feed.lock_change`, `rt.camera_target(dt)`, `rt.team_camera(..)`.
  - S8: `core.set_unlocked(&[PartyChara], now)` whenever the unlocked set changes (a locked character leaves the formation,
    a newly unlocked one joins, up to 3; the leader kept if still unlocked); `core.unlocked()` (None = no unlock rule has
    spoken yet: everyone).
- THE NAV (`party/nav.rs`, port design): the leader's breadcrumb `Trail` (a crumb every `nav.spacing` cm on a floor; a
  finished climb of the game's ladder = a ladder link; breaks at jumps up, let-gos, warps); `TrailNav` is kh-ai's `Nav`
  (`project` = a line down; `path_*` = one knee-high line when clear, else the route along the trail); the waypoint rule
  `nav::steer` gives kh-party's new `NpcPawn.steer` (`Party::set_steer(id, Some((goal, waypoint)))`, read by the Move
  state; `Party::move_goal(id)`), or `MemberCommand::ClimbLadder` at a ladder link's foot. A cast budget per member per
  step (`casts.per_member`): past it the nav answers optimistically (never a warp for want of casts).
- FALLBACKS: a ladder on his route not begun after `ladder.fallback` s (U12: 8); stuck (`stuck.*`); cut off after a body
  action (a fog wall, a door: `catchup.*`); AI Sora (a friend leads; kh-core has no ladder state) by the same ladder /
  stuck rules. KH3's Warp state (kh-party) ends at once within 30 m of the leader, so the host moves the member itself
  AS KH3'S WARP DOES: `warp.behind` cm behind the leader at the moment neither he nor that place is in the drawn camera's
  view, at most 3 s later in view (logged). kh-ai's own warp rule runs as in KH3 over the trail nav.
- STATUS (S2's rules, wired by S1 in party/mod.rs): every effect of the party's step on a member is passed to
  `status::on_party_effect(name, member, leader)` (a Cure landing clears his body's ailments), every
  `PartyEventKind::ItemUsed` to `status::on_party_item(item, target, leader)`. `feed.events` = the step's kh-party events
  (new additive `kh_partyhost::StepOut.events`).
- S6 HOOKS (wired by S1 in party/mod.rs): `partyui::configure(&mut core.rt)` right after the core is made (the real
  progression); `partyui::sync(&mut core.rt, sora, core.unlocked(), now)` right after every `core.step`: its
  `restore_unlocked` -> `core.restore_unlocked(&list, now)` (S8's unlocks as saved, WITHOUT `set_unlocked`'s "newly unlocked
  join" rule), `restore_unlock_record` -> `core.restored_unlock_record` (S8 takes it; partyui also keeps it:
  `partyui::unlock_record()`), `cure_bodies` -> `feed.cure_bodies` (S3); `partyui::reset()` every frame `party.er = 0`.
- SORA'S CURE ON A FRIEND (`heal_ally` kinds 10 / 11 / 12, read from `rt.pending_heals` before the step) ->
  `status::on_party_effect(cure_heal_effect(tier), friend, leader)`.
- AI SORA'S LADDER (a friend leads): `feed.sora_ladder = Some(LadderInfo)` when he stands at a ladder link's start on his
  route to the leader (the friends' rule); sora.rs gives it to S2's `lead.sora_climb_start`. `core.sora_on_ladder` (set by
  sora.rs from `lead.sora_climbing()`) holds his fallback clocks; the 8 s warp only when no climb began.
- The friends' head look-at: Sora's "t_camera" bone of the last drawn pose (else "atama"), passed by sora.rs.
- KEYS: `party.er`, `party.climb`, `party.pose`, `party.trace`, `party.draw` (testing aid), `party.recover` (one-shot);
  S3's `party.bodies` / `party.body_test` / `party.bullet_owner`. Sheet `sheets/port/party-er-host.json`.

### ER.11 The ally bodies (kh-er `src/allybody.rs`, stream S3)
Owner: ER party stream S3 (state note build/sora_tmp/er/party/IMPL_S3_STATE.md). Cargo feature `party`; keys `party.er`
(0 = nothing at all) and `party.bodies` (default 1 = bodies; 0 = off). Sheet `sheets/port/party-er-bodies.json`. Files:
`allybody.rs` (pure rules + tests), by `#[path]` `party/allybody_live.rs` (game half), `party/bodytest.rs` (the go/no-go
test, pure).
- REMOVED BY THE USER (2026-10-15, "please delete that backup ... that is the worst shit ever"): the F1 hit-redirection
  fallback (`party/redirect.rs`, `redirect_hit`, `party.bodies = redirect`, its sheet rows, the call in sora.rs
  `damage_in`). A HIT ON THE LEADER ALWAYS STAYS THE LEADER'S; nothing is ever redirected. A friend whose body is missing
  for a moment simply cannot be hit for that moment.
- WIRED BY S3 (marked "(ER party S3)"): sora.rs pre_physics end: `allybody::pre_physics(&mut Ctx { player, frame,
  targets, control, leader_feet, leader_yaw, dt, paused, wants: core.feed.wants })` then `allybody::deliver(core,
  sora_hp, sora_max, &params)` (each hit made final with the friend's OWN maximum, U8, and queued with
  `core.queue_hit(AllyHitIn::..)`); post_physics: `allybody::post_physics(frame)`; targets.rs `refresh`:
  `allybody::is_body(ptr)` leaves the bodies out of Sora's targets; targets.rs `apply_hit`: `allybody::fire_hit(..)`
  (stage 2) in place of `bullets.fire(..)`.
- POOL: one invisible, passive body per `AllyWant` (max `body.max` 3), made by the game's debug character creator from a
  Spirit Ash row (BuddyParam -> NpcParam, auto-picked by hit height; the spirit team read at run time), else c0000, else a
  copy of a nearby enemy with the team written; one spawn a frame; a body is validated (debug set / handle) before every
  touch; lost -> remade at once (the friend is never touched); cutscene / not wanted -> PARKED (unhittable, team
  `body.park_team`, `body.park_depth` m below the leader), reused for any friend; NEVER despawned. Every route failed ->
  logged loudly, said in the go/no-go report (item 1 FAIL, the summary line), spawning retried every `body.retry` s.
- KO (the user's rule, 2026-10-15: "Enemies should ignore [dead friends] until they recover"): at the KO frame
  (`AllyWant.ko` / `!hittable` from S1) the body is parked at once (unhittable: `disabled_hit` + invincible; neutral
  team; held below the leader, out of every enemy's sight and reach) and `status::cure_chr` clears its ailments; its HP
  losses while parked are never hits; on revive it is placed at him again, hittable, back on the spirit team. The game's
  own target-clear of an enemy's AI is NOT called (no located function; the SDK binds `CSTargetSearchSys` but no path from
  an enemy to it): go/no-go item 13 measures whether the switch alone makes enemies drop him.
- Read by others: `allybody::handle(chara) -> Option<FieldInsHandle>`, `allybody::status() -> String`. Cures: S3 drains
  S2's `status::take_ally_cures()` each frame and gives `status::cure_chr` to that friend's body.
- STAGE 2 (U14): `party.bullet_owner = 1 | donald | all` (default off until go/no-go item 10 passes): a hit whose
  `HitRecord.attacker` is a friend's capsule centre (within `owner.match` cm, his radius) is fired with his body as the
  bullet's owner (`Bullets::fire_owned`, which also borrows the AtkParam_Npc row of the slot's attack id).
- GO / NO-GO (the user's): `party.body_test = go` (a new value starts it), `report`, `bullet`, `kinds`, `stop`;
  `party.body_test.show = 1` draws the bodies. Lines `party-body-test: [ n] PASS | FAIL | WAIT | INFO ...` for the 12
  unknowns of research 1.5 and item 13 (a KO'd ally is dropped as a target within N s / re-targetable after revive), a
  verdict on items 1-6.


### ER.12 The leader body (kh-er `src/party/leader.rs`, `climbcap.rs`, stream S2)
> (2026-10-25, LADDER2: `climbcap.rs` and the ladder parts below are deleted; see ER.9.)
Owner: ER party stream S2 (state note build/sora_tmp/er/party/IMPL_S2_STATE.md). Cargo feature `party`; with the feature
off, or `party.er = 0`, or Sora leading, every body rule of sora.rs reads Sora's own `Output` exactly as before.
- WHERE: `SoraHost.lead: crate::party::leader::LeaderBody`. Fed by S1 every party step: `lead.update(rt, out, status,
  dodge)` (`rt.leader_view_of`; Sora's dodge frames `leader::sora_dodge_frames(reaction)` ORed into his `invulnerable`)
  and `lead.apply(rt, sora) -> Vec3` (the orders carried out through `RuntimeLeaderSink`; the return = the seat's
  counter-move, KH3 cm, for `core.counter_move`); `lead.clear()` when the party is off.
- THE BODY RULES (sora.rs `friend_lead() -> Option<guards::FriendLead>`, None = Sora): `write` / `feet_er` / `on_floor`,
  the follow rule (cutscene, the game's action, a grab, the wait for the ground), the net and its put-back / death, the
  fall guard, deep water (`guards::water_depth_for`: `water.depth` at the leader's height, Donald 83 cm), the lift ride,
  damage-in (`guards::friend_hit`: the same share / status class as Sora's, of HIS maximum; capped by his defender's
  `damage_max` in `apply`), the invulnerability (his Die / Warp / FR), a KO = the body's death (as Sora's death).
  Orders (`guards::LeadOrder`): `Hit(HitRecord)`, `Follow(at, yaw)` (held every party step while the body leads),
  `Release`, `Nudge(by)` (a lift), `Die`.
- THE SEAT: a friend leading is put on the body (`Follow`); the KH3 frame does not move (Sora and the other friend stay).
  Sora leading: the frame moves under him as before and every other member is moved back by as much
  (`seated` / `seated_riding`: members on the same lift ride it); `world.flat` (his level floor) is taken away around
  S1's `after_sora_step` (the party's casts see the game's world).
- THE FIGURE (for S4): `lead.over: Option<PoseOver>` = the body's pose (an action, a ladder with hands / feet on the rungs
  by his own limb lengths, a grab, a chest: U13, a pick-up held in place) retargeted onto the friend leader's skeleton;
  `lead.overlay(chara, &mut PosedChar) -> bool` lays it over the poser's pose (local + component recomputed, his weapon
  hidden; a cutscene figure brings its own place). `lead.cut_on` / `lead.leader_hidden()` (U6: in a real-time cutscene the
  friend leader stands in for the Tarnished; hidden when the cut shows no figure of the player); AI Sora is not drawn in
  it. `lead.light` (U5): who carries the hero light (Sora while he is in the party, else the leader's weapon tip).
- THE CLIMB CAPTURE (`lead.capture: ClimbCapture`): while ANY leader's body is on a ladder, the body's 52 rows per ladder
  animation and time, in the body's own frame, with the ladder state; the bind pose (`binds`); the ladder's ends
  (`capture.ladder`, `lead.ladder() -> Option<LadderInfo>`). Replay for an AI ally in `NpcStateId::Ladder`:
  `lead.climb_pose(chara, &rt.party.ladder(id)?, v.position, v.yaw, &mut PosedChar) -> bool` (the mount, one Up / Down
  step per 50 cm rung right / left in turn, the dismount; retargeted onto his skeleton, hands and feet on the rungs).
  Key `party.climb_capture` (default 1).
- STATUSES (status.rs): `request_member(member, leader, why)` (the leader's = the player body; an ally's queued for S3:
  `take_ally_cures()` + `cure_chr(&mut ChrIns, why)`), `request_all(members, leader, why)` (the save point, S6),
  `on_party_effect(effect name, member, leader)` (a Cure landing on a member, S5 / S1), `on_party_item(item, member,
  leader)` (S1: kh-party `ItemUsed`); Sora's own events go to his body (`on_events_of`: his ally body while a friend leads).
- figure.rs: `rows_for(skeleton)` (the rows a skeleton has: Donald 40 of 52, Goofy 46 with `kubi` -> `kubi1`),
  `retarget_from_binds`, `Rig::build_for` / `source_binds`, `Cut::posed_party`.

### ER.12 HUD, menus, save, progression (stream S6; state note build/sora_tmp/er/party/IMPL_S6_STATE.md)

Shared rules (kh-partyhost `progression.rs`, both hosts):
- `rt.set_progression(true)` (ER only, before the first step): the friends start from KH3's init data
  (`PartyConfig::level99` false), no `party-test` loadout (AI bytes = the game's Restore Defaults 0 / 0 / 1 / 7), a
  friend slotted out comes back with his record (S0's note, `remove_one` -> `pending_records`); a joining friend takes
  `join_level` (his own table at the shared total, gauges full).
- ZERO EXP IS THE LEADER'S (the user's rule): `rt.sync_zero_exp(sora)` runs inside every `route` (lead.rs): the leader
  holds it in the carried state; a friend who stops leading loses it, Sora keeps his copy switched off.
  `rt.zero_exp: ZeroExp { holder, on, loaded }`, `rt.zero_exp_on()`, `rt.set_zero_exp_loaded(on)`.
- `rt.give_exp(sora, base, rate, position) -> ExpGiven` (P2: the leader as player 0; Sora out of the party: straight
  into the shared total), `rt.prize_members(sora, point)`, `rt.prize_mods(mods)`, `rt.apply_prize(chara, kind)`,
  `rt.savepoint_needs(sora)`, `rt.savepoint_recover(sora, n)`, `rt.friend_reward(chara, &FriendReward)`
  (`bonus:<ETresVictoryBonusKind>` / `ability:<name>`), `starting_formation(starter)`.
- THE SAVE'S PARTY RECORD: `rt.party_record() -> Value` = format 2 (`formation::party_record`) + `friends_version` 5 +
  `friends` (ER.6) + `zero_exp`; `rt.reset_friends_for_load(total)` then `rt.load_party_record(&v)` (any format; null =
  KH3's init data); `read_party_record(&v) -> PartySave`. kh-host save.rs: VERSION 5, a file with `party.friends` is
  written as 5, without as 4 (byte for byte the old file); `SoraSave::written_version`, `party_friends`.
- kh-host: `Hud::set_formation_locked(&[PartyChara])` (the Party Formation screen's "???" rows, refused);
  `items::party_weapon_key(id)` / `party_weapon_id(key)` (the friends' weapons as items 0x315.. / 0x329..: the HUD
  stocks them under their key); `hud_layers::draw_movie_in(.., clip)` + `Layers::clip` / `clip_only` (KH3's ExactFit
  16:9 viewport for the cockpit at non-16:9 outputs).

Elden Ring glue (kh-er `partyui.rs`, the mailbox between the party step and the screens; cargo feature `party`, key
`party.er`; no feed = every hook a no-op):
- S1 calls `partyui::configure(&mut rt)` right after `PartyRuntime::new`, and `partyui::sync(&mut rt, sora, unlocked,
  now) -> SyncOut { restore_unlocked, restore_unlock_record, cure_bodies }` once per party step right after
  `PartyCore::step`; `partyui::reset()` when the core goes off / away.
- Screens: hud.rs (`hud_frame` / `hud_after`: slots, leader, menu pages, Customize, formation + locks, weapons, level-ups,
  the leader's command menu; a fallen friend leader = game over, `fallen_leader`), save.rs (`party_record_now`,
  `load_record`), the slot pictures (`save::peek_portrait`), savepoint.rs (`savepoint_pawn`, `savepoint_recover`,
  `savepoint_rest` -> S2's `status::request_member` for every AI member), sora.rs / prize.rs (`give_exp`,
  `prize_members`, `PartyPrizes`, `prize_mods`), bossreward.rs (`party.*` rows -> `give_friend_reward`), lootmap.rs
  (`party.weapons`, `set_party_friends`), void.rs (`set_leader`, `friend_pose`: the fallen friend's KO pose).
- S8: `sync`'s `unlocked` (saved as `party.unlocked`, locks the Formation rows, gates the weapon drops),
  `partyui::set_unlock_record(Value)` / `unlock_record()` (kept as `party.unlocks`), restored by `SyncOut`.

### ER.13 Team attacks and cameras (kh-er `src/party/team.rs`, cam.rs, stream S7; state note build/sora_tmp/er/party/IMPL_S7_STATE.md)
Cargo feature `party`, key `party.er` (0 / feature off: cam.rs gets `None` and every line of `KhCam::step` is as before).
- THE NINE: Unison Fire / Blizzard / Thunder, Fusion Spin / Rocket (`team::Start::Unison(22..26)` = attack kinds
  0x3F..0x43: the finisher of the LEADER's combo) and Donald Meteor 207, Flare Force 208, Goofy Bombardier 195, Trinity
  Guard 193 (`Start::Offer`: the player's Y on the friend's offer). `team::TeamAttack::{ALL, name, start, kind, of_kind}`.
  Stepped by S1's glue (`rt.step` -> `Team::step`); started ONLY by the player's leader (kh-partyhost `route`; AI Sora
  never presses Y, `sora_is_ai` keeps his combos from rolling a unison).
- HITS: the party's targets = `ErTargets::apply_hit` = the player's bullets (damage x `damage.out.scale`, the puppet's
  reaction / dmgLevel, poise damage, bossguard); an undrawn / unspawned character (`target.visible_only`, inactive) has no
  ALIVE / HITTABLE / LOCKABLE flag: no team attack picks it or hits it. U14 = S3's `party.bullet_owner`.
- CAMERA (cam.rs): `trait PartyCamera { leader_target(dt), lock_change(), leader_lock(), team_layer(dt, events, sora,
  &mut view, world, stick) -> reset_field, team_lens() -> (fov, shake) }`; `KhCam::step(.., party: Option<&mut dyn
  PartyCamera>)`; sora.rs passes `party::team::camera(&mut self.party)` (`TeamCam` over the core). A friend leading: the
  field / lock-on rig follows HIM (`rt.camera_target`, `feed.lock_change` is TAKEN by the camera; AI Sora's CameraLock /
  aim / shot events are not the player's camera). The team cameras: `team::layer(rt, ..)` = `rt.team_camera` with
  cam.rs's camera-filter `ErWorld` (the collision pass: never into the terrain), shield_01 steered by the player's right
  stick (the field rig's convention, config flips included); `reset_field` -> the field rig behind the leader; the
  CameraAnim FOV (`cam::team_fov_h`) and the blueprint shakes on the drawn lens; the boss finish's cut stays on top; its
  "player" cut frames the leader (`cam::leader_cut_height`). Log lines `team: ... STARTS / ENDS ...` once per attack.
- THE HELD Y (a friend leading only, port design): `team::hold_y(&slot, &sora, input, dt, control)` right before
  `route` presses the player's refused Y on a team command again each step for 0.5 s (Sora's executor refuses while AI
  Sora has something queued) until the FR state begins (`party.team_y_hold`, default 1).
- SORA LEADING IN AN FR STATE: the hidden body's no-damage frames (`party.team_invuln`, default 1); friends: S0's
  `AllyWant.invulnerable` (S3).
- DRAWING (for S4): `team::props(&rt) -> Vec<TeamProp { class, model ("o_ex210_0" meteor / "o_ex208_0" missile), index,
  position, rotation, scale }>` (KH3 space; `trs()`; the model's relative rotation of `PACK/party/team/props/props.json`
  goes on top as kh-app does) for `CharDraw { chara: TEAM, props }`; `rt.team.mesh_pitch(member)` for Goofy's rush (on his
  actor, as kh-app `team_mesh_pitch`); Sora's own draw is pitched in sora.rs (`team::pitch_sora`, D5's `sora_mesh_pitch`).
- `team::gshoot_compiled()`: Goofy Bombardier's socket tables are in the build (logged once at the first party camera).
- BOSS FINISH: any member's killing blow (the same `ErTargets` hit memory); finish.rs unchanged but its log wording.

### ER.14 Party rendering (kh-er-render `src/party.rs`, `src/gfx/party.rs`; kh-er `src/partydraw.rs`; stream S4)
Owner: ER party stream S4 (state note build/sora_tmp/er/party/IMPL_S4_STATE.md). kh-er-render cargo feature `party` (default on;
kh-er's `party` turns it on, kh-er's `--no-default-features` builds a renderer without it); key `r.party` (0 = no party draw
and no party pipeline). With nothing submitted the frame is Sora's alone (pixel-identical to the build without the party).
- API (`kh_er_render::party`): `submit_party(Vec<CharDraw>)` / `clear_party()` once a frame after `submit`;
  `CharDraw { chara: DONALD 1 | GOOFY 2 | TEAM 3, body: Option<Body> (kh-partyhost PosedChar: HIS skeleton), weapon:
  Option<PartyWeapon { model "w_do010".., to_world, component (empty = reference), opacity }>, props: Vec<(model, Trs)>
  (TEAM: "o_ex210_0" / "o_ex208_0", bind pose), glow_mask, blood: BloodLook }`; `set_party_portraits(Vec<PartyPortrait {
  chara, frame: PortraitFrame }>)` (camp panels; empty = off at once); `set_void_char(Option<VoidChar { chara, body }>)`
  (the game-over void; give the VoidFrame no body then); `set_hero_carrier(Option<HeroCarrier { feet, tip }>)` (U5);
  `set_enabled(bool)`; `team_prop_turns(pack)` (props.json's relative rotations by model); `status()`.
- PER DRAW, as Sora's: the member's own library / textures / skeleton / shaders (`PACK/party/<npc|team>/materials/er`, built
  by `kh-shaderc --pack PACK/party/<dir> --party`: KHS spliced from the main pack, 320-joint skin, KH_PSP for the owners that
  read the pre-skinned position), the `_pl` / `_bl` / `_hy2` + `_gb2` sets, early depth + stencil, the hybrid G-buffer write
  (his own scissor box), the ER80 glow mask, the ER80b shaft mask, the contact / thrown shadow (his capsules), his own point
  lights at his feet, his blood, the velocity pass and `r.shadow_real` (his own shadow.vs).
- kh-er glue (`partydraw.rs`, called right after Sora's `submit` and from hud.rs / void.rs): the members of `rt.current`
  (`core.poser.pose(id, alpha)`), the friend leader with `lead.overlay`, AI allies on a ladder with `lead.climb_pose`, Goofy's
  thrown shield (`Party::thrown_shield`, spin clip `anims/1328.glb` on "wep") / `weapon_hidden`, Goofy's rush pitch
  (`team.mesh_pitch`), S7's `team::props` (+ props.json's turn), S8's `unlock::stage_draws()`, S5's `blood::party::party_look`;
  in a real-time cutscene only a friend leader in the scene's figure (`!lead.leader_hidden()`); the camp pictures
  (`partydraw::camp`: each friend's MenuPoser with the face rows, his UI actor's capture and two spots
  `kh_host::menu_portrait::friend_data` / `friend_out` / `top_panel`, on `charaWindow0<n>` of `hud.camp_panels()` or the sub
  screen's picture for `camp_member()`); the void (`partydraw::void_char(friend_pose(..))`). Keys `r.party`,
  `r.party_glow_mask` (1), `menu.party_portraits` (1).

## Stream X1: abilities the port excludes (Attraction Flow; decision dec029, 2026-10-15)
- Sheet `sora-abilities` has a column `port` (`in` / `excluded`, written by `tools/sora_stats_sheet.js`); generated field
  `AbilityRow::port`. Excluded now: ATTRACTION_TIME 47 (Attraction Extender), ATTRACTION_UP 62 (Attraction Enhancer).
- `kh_core::status::tables::is_excluded(kind) -> bool`. `tables::level_ability` returns `None` for an excluded ability (its levels
  grant nothing, the level-up panel has no ability line).
- `kh_core::status::AbilitySet` never holds an excluded ability: `set`, `set_owned`, `set_copy`, `add_worn`, `set_form`, `set_meal`
  leave it at 0. So a save, loadout, reward (`Reward::Ability`: no change, no panel), equipment or menu that names one gives
  nothing, costs no AP and lists nothing. Every party member's status uses the same set. Streams that hand out abilities
  (X2 / X3, boss rewards, Donald / Goofy designs) must not rely on an excluded ability.

## Stream PF: the icon-based Party Formation screen (decision dec030, 2026-10-15)

- kh-ui `party_formation.rs` (new): `FormationArt` (loaded by `CampMenuView::load`), `FormationScene` (drawn as the camp
  menu movie's root script children named `pf_*`; same layer in both hosts). `player.rs`: `Movie::instance_of(def, class,
  name, label)` (a free instance of a library symbol).
- THE FEED (additive fields of `kh_ui::camp::FormationRows`): `members: Vec<FormationMember { key, name, level, locked }>`
  (roster order; key = `sora` / `donald` / `goofy` picks the pictures, an unknown key draws a silhouette), `active:
  Vec<usize>` (roster indices, slot order), `leader: usize`, `refused: Option<String>`. Filled by kh-host
  `Hud::formation_view` from `formation::ROSTER`, the formation and `set_formation_locked`; hosts change nothing
  (kh-er partyui.rs keeps calling `set_formation` / `set_formation_locked` / `open_party_formation`).
- Keys of the step: left / right (up / down too) walk the roster, A = `CampAction::Formation { row, lead: false }`, Y =
  `{ lead: true }`, B closes (unchanged actions). While the screen is up the camp's `topMenu.statusAnm`, `charaList`,
  `cmdBtnTopMenu`, `nextGoal` are hidden (so `menu_portrait::panel_at` finds no panel: no 3D pictures).

## Stream X2: the friend leader's framework (Sora's magic for Donald, Customize per leader, sprint, damage)

Owner: X2. State note `build/sora_tmp/party/IMPL_X2_STATE.md`. Design: `build/sora_tmp/party/XD_PLAYABLE_KITS.md` 1.1,
1.5, 3, 4.8, 4.9 (approved 2026-10-15). Everything is inert until a friend LEADS (the pad drives him): an AI friend keeps
his KH3 kit, and with Sora leading nothing changes. Files X2 owns: kh-core `magic/caster.rs`, `command/leader.rs`;
kh-party `leader_magic.rs`, `pad/sprint.rs`; kh-partyhost `custom.rs`; tests named `x2_*`. Marked `(party X2)` edits
elsewhere (listed in the state note).

### X2.1 kh-core (engine-free, both hosts)
```rust
// magic/caster.rs: Sora's spell actors cast by another body (proxy pawn; Sora's OnShot spawn rule, damage, Cure,
// the formchange gauge + RollGrandMagic; handles from the member's upper half range `caster::handle_base(member)`)
pub struct SpellCaster;  impl SpellCaster {
    pub fn new(member: PartyMember) -> SpellCaster;
    pub fn place(&mut self, location: Vec3, yaw: f32);
    pub fn set_stats(&mut self, stats: CasterStats, own: &AbilitySet, grand_from: &AbilitySet); // grand_from = Sora's
    pub fn shoot(&mut self, world, targets, audio, effects, shot: &Shot) -> Vec<u32>;          // at the caster's Shot
    pub fn step(&mut self, world, targets, audio, effects, allies: &[Ally], dt) -> CasterStep;  // { heals, hits, offered }
    pub fn add_form_points(&mut self, points: i32);                    // a damaging melee hit of the caster (m_FormPoint)
    pub fn offers(&self) -> &[GrandOffer]; pub fn ready_offer(&self) -> Option<GrandOffer>; pub fn take_offer(serial) -> Option<u8>;
    pub fn projectiles(&self) -> impl Iterator<Item = ProjectileView>; pub fn rails(&self) -> &[RailNode]; pub fn gauge(&self) -> (f32, f32);
}
pub fn effective_cost(cmd: u8, combo: i32, abilities: &AbilitySet, mp_cost_scale: f32) -> Option<i32>; // -1 = all MP
// command/leader.rs: the per-leader Customize block and lists
pub enum LeaderKind { Donald, Goofy }            // key(), list_label() ("Magic" / "Abilities"), slot_count() (6 / 3)
pub struct MagicBook { tiers: [u8; 6] /* asset order Fire Blizzard Thunder Water Aero Cure */, sora_slots: [u8; 6] }
    // of(&Status, sora_slots), tier(e), learned(cmd), best(e), goofy_tier() (decision 6: max spell tier, 1..=3)
pub struct LeaderCustom { magic_slots: [u8; 6], tier_chosen: [bool; 6], shortcuts: [[u8; 4]; 3], filled: bool }
    // sync(kind, &book) (first fill + Sora's learn rule), set_slot, swap, set_shortcut, slots(kind), to_json, from_parts
pub mod goofy { BASE 0xC1; id(index 0 Tornado / 1 Turbo / 2 Discus, tier 1..=3); split(id); tiers(id); name(id) }
pub fn assignable(kind, &book, items: &[ItemKind]) -> Vec<Assignable>;  pub fn entry_name(kind: u8) -> &'static str;
```
Goofy's ids live only in Goofy's block / lists (ETresCommandKind has no free values).

### X2.2 kh-party
- `Party::set_leader_magic(feed: LeaderFeed)` (the host, every step before `set_pad_frame`): Sora's `MagicBook`, his
  abilities (the -za counts), game / battle level, MP cost scale, both leaders' `LeaderCustom` blocks.
- While a pad-driven Donald has the feed: his Magic list = his block's slots (Sora's spells), his shortcuts = his block's
  sets (LB + face button; LB + D-pad up / down flips A / B / C as Sora's), Items = his pouch. A Sora spell is
  `MemberCommand::Action { def: the element's Donald def, .. }` with the command parked in `PadState::x2` (taken by
  `actions::begin`): Donald's clips (`MagicFire` / `MagicAirFire` for Fire, Blizzard; `MagicCast` / `MagicAirCast` for
  Thunder, Water, Aero, Cure, the Finish End and grand magic), his `vn_ex001_*_cal00` (Water: Sora's `vm_wat_cal0`) at
  `vfx_call`, Sora's actors from the caster at the Shot, MP = Sora's cost (`effective_cost` with Donald's abilities) from
  Donald's own MP (his 30 s charge). `ActionRun::sora_cmd` (0 = a KH3 action) is the running Sora spell.
- For X3 (Donald's moves): `crate::leader_magic::{last_element(p) -> Option<(Element, f32 seconds ago)>, chain(p) -> u8,
  is_sora_cast(p) -> bool, finish_next(p) -> bool}`; `PadState::x2.finish_override: Option<fn>` is NOT provided: X3's
  Flare Force replaces the Finish End by checking `finish_next(p)` before issuing (X2 issues a plain Finish End cast).
- Goofy leading: his "Abilities" list / shortcuts from his block; the tier of the run is `PadState::x2.tier` (1..=3, for
  X3G); the action is his KH3 one (Tornado / Turbo / Discus).
- Grand magic: Donald's magic hits (and his melee hits' m_FormPoint) fill his caster's gauge; the offer shows in the pad
  menu's `situations` (icon 3) in front of Sora's queue; Y casts it (no MP, `MagicCast`). `PadState::x2.y_taken` tells
  the host the Y was his (not forwarded to Sora's core this step).
- `Party::caster_projectiles() -> impl Iterator<Item = (effect, position, forward)>` (the host's projectile taps).
- Sprint (decision 9): `pad/sprint.rs`, Sora's rule (MySprintSpeed 980 after `speed_sprint_delay` 1 s of running);
  `PadState::x2.sprinting` (X3's Superslide reads it). The leader's `cmc.max_walk_speed` is 980 while sprinting.
- Damage (decision 1): `Party::defender(id)` of a pad-driven member has `damage_max` 0 (no KH3 friend halving above 24,
  and kh-er's `guards::capped` no longer caps); AI members keep 24.

### X2.3 kh-partyhost / kh-host / kh-ui
- kh-partyhost `custom.rs`: `PartyRuntime::customs` (per character, authoritative, survives respawns), fed to the party
  in `route`; save: `party.custom = { "donald": {...}, "goofy": {...} }` in `party_record` / `load_party_record`
  (additive: an old save fills from the defaults); kh-app saveload writes / reads the same field.
- kh-host: `PartyMenuMember::custom: Option<LeaderCustom>` (his block) and `custom_costs: Vec<(u8, i32)>` (Goofy's
  entries' MP for the help text); `PartyMenuEdit::Custom { chara, edit: CustomEdit }` (SetMagicSlot / SwapMagicSlots /
  SetShortcut). With a friend leading, `Hud::customize_in` shows his block (hud_x2.rs, a child module of hud.rs) and the
  friend entries are the two non-leaders (P's `set_ai_members`). The spell help's "Costs {0} MP." is filled (Sora's
  page too).
- kh-ui `CustomizeIn`: `magic_label: Option<String>` (a text key: Goofy `kh3_camp.customize.group_0010` "Abilities"),
  `tier_groups: Vec<[u8; 3]>` (Goofy's tiers); `CustomizeIn::tiers_of`.
- `LeaderFeed::sora_team_offer`: a TEAM-class entry in Sora's queue takes Y first (the leader's offers hidden meanwhile).
- `leader_magic::parked(p)`: the KH3 ability gate of `NpcPawn::command` does not apply to a parked Sora spell.
- Debug words (test level script / console): `party x2 status`, `party x2 gauge`, `party x2 offer KIND`.

## Stream K1: the AI members' follow speed and locomotion (party K1, 2026-10-15)

Owner K1. State note build/sora_tmp/er/party/IMPL_K1_STATE.md; rows sheets/port/party-follow.json (follow.* KH3, k1.* port design).
- kh-party `locomotion` (new): `AiLoco` (field `NpcPawn.loco`), `K` (the sheet's rows), `follow_speed(pawn, cx)` (KH3's
  FollowPlayer rule 0x1412196C0), `begin_move` / `move_tick` / `decelerate` / `arrive` / `hold_tick` / `reached` / `idle_tick`
  (called by pawn.rs only). `Party::ai_loco(id) -> Option<pad::LocoPose>`: an AI member's speed-matched loop pose in Move
  (the poser draws it as the pad friends' `pad_state(..).loco`).
- kh-party `SoraView` + `max_speed` (the leader's MaxWalkSpeed, 0 = unknown) and `analog` (his AnalogInputModifier, 1):
  kh-partyhost fills them (runtime.rs for Sora, lead.rs `leader_view` for a friend leader). Other hosts building a SoraView:
  `..SoraView::default()` keeps the rule on the leader's velocity.
- kh-core `Sora::set_speed_scale(scale)`: AI Sora's catch-up (kh-partyhost lead.rs, 1 whenever Sora is the player).
- kh-ai TresBTTask_MoveTo now observes its vector key (0x1404ABCE0 / FUN_1404B3610): a Destination that moved more than
  0.95 x the radius re-issues the MoveTo while the task runs.

## Stream X3G: Goofy's playable kit (party X3G, 2026-10-15)

Owner: X3G. State note `build/sora_tmp/party/IMPL_X3G_STATE.md`. Design: `build/sora_tmp/party/XD_PLAYABLE_KITS.md` section 2
(approved 2026-10-15 with its 10 defaults). Every number: `sheets/port/party-kit-goofy.json` (`build/sora_tmp/party/x3g/
make_sheet.js` -> `build/gen/rust/party_kit_goofy.rs`, `kh_party::pad::goofy_kit::K`). Files X3G owns: kh-party
`pad/goofy.rs`, `pad/goofy_kit.rs`, `pad/goofy_shots.rs`, `pad/goofy_tests.rs`. Everything acts only for a PAD-DRIVEN Goofy
(he leads); AI Goofy keeps his KH3 kit.

```rust
// kh_party::pad::goofy (states NpcStateId::Pad(40..=51): GUARD GUARD_HIT GUARD_BREAK BASH HOP SURF FINISH DIVE COPTER CHARGE AIM AIR_GUARD)
pub struct Taunt { centre: Vec3, radius: f32, left: f32 }               // Over Here!
impl Party {
    pub fn goofy_taunt(&self, id) -> Option<Taunt>;                       // enemies within radius should target him for `left` s
    pub fn pad_mesh_pitch(&self, id) -> Option<f32>;                      // Bombardier Dive's mesh pitch (UE deg), as Team::mesh_pitch
    pub fn goofy_gauges(&self, id) -> Option<((gauge, max, broken), (mode gauge or seconds left, max), (focus, max))>;
    pub fn goofy_locks(&self, id) -> Option<&[TargetId]>;                 // Discus Barrage locks while he aims (HUD marks)
    pub fn goofy_word(&mut self, words: &[&str]) -> String;               // DEBUG: kh-app `party-word x3g mode-full | gauges`
}
// kh_party::pad::goofy_kit
pub enum GoofyAbility { IronWall, PaybackBash, ShieldCharge, Stalwart, Rally, Ricochet, OverHere, TornadoCopter, FusionSpin, FusionRocket }
pub struct GoofyKit { owned: u16, equipped: u16 }   // FriendStatus::goofy_kit; ap_in_use, list, record / apply_record (KIT_* names)
impl FriendStatus { kit_goofy_sync_level(), kit_goofy_sync_sora(&AbilitySet), kit_goofy_equip(k, on) -> bool }
```
- Shared-file hooks (all marked "(party X3G)"): pad/mod.rs (`pub mod goofy*`, `PadState::goofy`, `decide`, `pre_move`, `tick`,
  `targeting` RB tap-on-release for Goofy, `pad_menu` situation row), pawn.rs `receive_hit` (guard / invincibility first, after the
  Die / FR check; Stalwart / Shield Guardian = HP without reaction), actions.rs (`begin`: the barrage's throw pays no MP; Tornado's
  stick steer; `deliver`: tier-2 Tornado knocks inward, the landed-hit count), party.rs (`goofy_party_step(dt)`, the Discus bounce
  and its x0.5 damage), status.rs (`goofy_kit`, its AP), kh-partyhost save.rs (KIT_* of Goofy in the v5 friends record) and
  progression.rs (`kit:<GOOFY KIT NAME>` reward tokens), kh-app party_draw.rs (`pad_mesh_pitch` with the team pitch) and
  player.rs (`party-word x3g`).
- A guarded hit returns `HitResult { accepted: true, damage_dealt: 0, guarded: true }`; an invincible one `accepted: false`.
- Sora's side: `LeaderFeed::sora_abilities` FUSION_SPIN / FUSION_ROCKET / GLIDE give Goofy's Fusions / Tornado Copter (decisions 3, 8).
- X3G-b: `Party::goofy_mode_actor(id) -> Option<(Vec3, f32)>` (Shield Guardian running: his actor); kh-camera
  `StateEventCameras::pad_shield(on: bool)` (D3's shield_01 on the accompany pawn; the runtime's `team_camera` feeds the leading
  Goofy as that pawn during the mode and ends it after). Superslide = B4's `NpcStateId::SuperSlide` fed by the pad; aerial recovery
  = `NpcStateId::Pad(52)`.
- Reward rows: sora-er-boss-rewards `boss.leonine` proposal `ability:FUSION_ROCKET` (decision 3); `party.maliketh` documents the
  copter (`kit` key; the friend-reward parser reads only donald / goofy, the grant is the GLIDE sync).

## Stream X3D: Donald's playable kit (party X3D, 2026-10-15)

Owner X3D. State note `build/sora_tmp/party/IMPL_X3D_STATE.md`. Design `build/sora_tmp/party/XD_PLAYABLE_KITS.md` section 1
(approved with its 10 defaults). Rows: `sheets/port/party-kit-donald.json` (117, `build/sora_tmp/party/x3d/make_sheet.js`
-> `build/gen/rust/party_kit_donald.rs`, `kh_party::pad::donald::K`). Files X3D owns: kh-party `pad/donald.rs`,
`pad/donald_kit.rs`, `pad/donald_shots.rs`, `pad/donald_tests.rs`, `tests/x3d_feel.rs`. Everything acts only on a
PAD-DRIVEN Donald (he leads); AI Donald is unchanged.
- `PadState::donald: DonaldKit` (the kit's per-leader state; `log` of `KitEvent`s, drained by `donald::take_log`).
- Kit states `NpcStateId::Pad(n)`, n >= `donald::FIRST` (10): HOP 10, BLOCK 11, BLINK 12, FLUTTER 13, FLOAT 14,
  FIN_GROUND 15 (Unison / plain Fire finish), FIN_AIR 16 (Meteor Drop / Meteor Finale), FIN_MAGIC 17 (Flare Force),
  GRAND 18, VOLLEY 19, COUNTER 20. The Superslide is the NPC `SuperSlide` state fed by X held.
- `donald::invincible(&NpcPawn)`; `donald::on_hit` is the first word of `NpcPawn::receive_hit` (dodged: not accepted;
  Magic Block: guarded, 0 damage). Includes X2's Finish End rule and the Water barrier (a Sora Water cast after its Shot).
- Grand Magic (coordinator's ruling): X2's Y on the offer is only the TRIGGER; `pad::decide`'s `MenuAction::Grand` calls
  `donald::grand(p, serial, cmd)` (sets `x2.take_offer`, `x2.y_taken`), the kit's GRAND state plays MagicCast /
  MagicAirCast at 0.85x, the cast effect at 1.5x (X2's `cast_effect`), is invincible until the Shot and pushes
  `caster::Shot { grand: true }` into `x2.shots`. X2's `request_grand` path is no longer called. While Sora's queue offers
  a team attack, X2's menu hides the offer and Y goes to Sora (unchanged).
- Flare Force replaces X2's Finish End: `pad::decide`'s `MenuAction::SoraSpell` asks `donald::sora_spell(p, cmd)` first
  (true when `leader_magic::finish_next`): the cast's MP is paid, FForceFinish 0.8 .. 2.6 s, 6 missiles.
  `donald::quick_follow(p)` widens X2's follow-up window (Quick Chant).
- Counter Blast and the plain Fire finish go through X2's caster (`x2.shots`, `finish: true`) when the feed is there,
  else the kit's own bolt (`donald_shots::Bolt`).
- `donald::feed(party, id, SoraLearned, test_level)` (kh-partyhost `route`, before the pad frame): Sora's Glide /
  Aerial Recovery / magic tiers / Unisons, AI Sora in the party, his situation queue busy; the test level equips the
  whole kit (`KitAbilities::test_all`).
- `donald::party_step(party)` (kh-party `Party::step`, after X2's step): the kit's gauge points to the member's caster
  (`add_form_points`); his own Aero pads (`SpellCaster::pads` / `use_pad`, new X3D accessors in caster.rs) throw him with
  Sora's super-jump power (`donald::launch`).
- `donald::party_taps(party)`: the kit's bolts / meteors / missiles body effects (kh-partyhost `StepOut::projectiles`).
  kh-partyhost also merges `Party::caster_rails()` (ids | 0x4000_0000) into `set_ice_rails`, so the AI friends ride
  Donald's Blizzard rails as they ride Sora's.
- Kit abilities (`pad/donald_kit.rs`): `KitAbility` (Spell Weaver, Counter Blast, Quick Chant, Mana Rebound, Meteor
  Finale, Blink Step, Aero Float, Unison Fire / Blizzard / Thunder), `KitAbilities` (owned / equipped bits) as
  `FriendStatus::kit` (its AP is part of `ap_in_use`). `FriendStatus::kit_grant`, `kit_sync_level` (his six by level,
  sheet `ability.*.level`), `kit_sync_unisons` (Sora's learned). Saves: `KitAbilities::record` / `apply_record` (`KIT_*`
  names in the v5 friends record's ability list; kh-partyhost save.rs). Rewards: `FriendReward::Kit(name)` (token
  `kit:AERO_FLOAT`, row `party.red_wolf` of sora-er-boss-rewards.json).
- (X3D-b) `donald::party_props(party) -> Vec<TeamProj>`: the kit's meteors / missiles for the hosts' team-prop
  drawing (kh-app party_draw `team_props`, kh-er party/team.rs `props`). `donald::feed(party, id, learned, view)`: the
  host's `Input::view` (Sora's shotlock reticle for Spell Volley). Test level: `FriendStatus::kit_test_loadout` equips
  within his AP in the order of sheet `ability.*.priority` (no more `test_all` from the host). Menus: X2-b reads
  `FriendStatus::kit` (`KitAbilities::list` / `set_equipped`, `KitAbility::label` / `ap`).
- Input changes for a pad Donald only: RB toggles the lock on a SHORT TAP's release (Sora's `tap_max_time` 0.2667 s),
  a hold opens Spell Volley; B held while falling is Aero Float (with the reward); a high fall lands with RollLand.

## Stream FX1: a friend leader's finisher never vanishes (2026-10-16; amends D.6, additive)

The bug (build/sora_tmp/er/party/s0/REBASELINE.md, tour 2's second run): Goofy led with Sora in the party, D.6's roll
said "yes" while AI Sora was landing from an air attack 7 m away; Sora's latch (`FriendState::leader_unison`, 0.5 s)
ran out silently, he used Myriad instead, and Goofy, told "yes", skipped his own finisher: nothing at all. Every edit
is marked "(FX1)". The fix has three parts.

1. THE GATE (kh-core, kh-partyhost lead.rs). `kh_core::Sora::can_start_leader_unison(&self, leader_uid: u8) -> bool`
   (combat/friend.rs `leader_unison_ready`): HP >= 1, grounded Idle / Run (the states where `TryAttack` starts a fresh
   attack with no forced kind: mid-action, traversal and air states are out, because their forced kinds and the XS
   forced attacks replace rule 7), nothing queued, no style change / finish in action (0x4B / 0xC0), no command
   cool-down, no sneeze latch, no team attack / request / latch running, and the leader's friend row not KO'd. The
   roll keeps its own distance rule (`COMBO_FINISH_RANGE[0]`). `PartyRuntime::route` rolls
   `roll_leader_unison` only when Sora is active, not in his AI shotlock sequence, and this holds. Otherwise it answers
   `finisher_answer = Some(false)` at once and logs "the leader's combo plays his own finisher: AI Sora cannot start
   a team move now (state, distance)".
2. THE SAFETY NET (kh-partyhost lead.rs `team_wait_step`, run at the head of every route). After a "yes" the host
   keeps `Leadership::team_wait` (the leader, Sora's `unisons` count, the age). If the count rises, the move started
   and the wait ends. If the latch runs out (or something took it without playing the move, or Sora was not stepped
   for 0.6 s), the host clears `unison` / `leader_unison` and gives the pad a LATE "NO":
   `kh_party::pad::PadState::finisher_late_no: f32` (new field, seconds, set to 1.0). Donald's kit (pad/donald.rs
   `pre_move`) then plays his 4th step (`begin_ground_finisher`: UnisonFinisher, or Meteor Finale at full MP), and
   Goofy's kit (pad/goofy.rs `pre_move`) plays `start_finisher` (solo Fusion Spin / Rocket, else Shield Bash 2). Each
   plays as soon as the friend is free on the ground, at the Jump Strike's cancel point (Donald) or landing (Goofy), or
   at the chain point of a swing the player started meanwhile. Log: "the team move never started (...): a late "no"".
3. THE TEAM MOVE FIRST. (a) AI Sora's own tools (lead.rs `SoraAi::tools`: situation commands such as Myriad, spells,
   heals, shotlock) are skipped while the latch runs, so his core starts the team move as his next attack. (b) The XS
   finisher choice (kh-core abilities/xs/finishers.rs `refine`) never replaces rule 7's kinds 0x3F..0x43 while
   `sora_is_ai`. Before, a locked target in Spiral Rush's band, a Rage charge or Giant Stamp's strong body turned the
   accepted fusion into Sora's own XS move. With Sora leading, nothing changes: his XS finishers still win over his
   own unison roll (tour 2's second Zantetsuken at 7.75 s).

Tests: kh-partyhost `src/fx1_tests.rs` (6). These run the full loop (route, Sora's real core, the party step) with
the real kits, for Donald and for Goofy leading: a busy Sora (guarding) means "no" and the friend's own finisher; an
idle Sora 1.5 m away means "yes" and the move starts; a lapsed latch (a command cool-down for the whole latch) means
a late "no" and the friend's own finisher. kh-core `tests/fx1_team_move.rs` (2): the gate's conditions, and a fusion
that is not replaced by Spiral Rush (this test fails without the `refine` guard). The test-level run is in
build/sora_tmp/er/party/s0/fx1/ (see the FX1 section of REBASELINE.md).

## Conversation (stream CV1, 2026-10-16; state note build/sora_tmp/er/IMPL_CV1_STATE.md)

The user's request of 2026-10-16: one KH-style conversation system for every talk (Donald's, and every Elden Ring
real-time NPC conversation, which stream CV2 converts). Engine-free pieces in kh-ui / kh-camera / kh-host; the Elden
Ring host in kh-er `src/conv.rs` (`crate::conv`, NOT behind the `party` feature; run-time key `conv.on`, default 1).
THIS API IS STABLE: additions only (C.5, ambient conversations, added the same day).

### C.1 The bubble (kh-ui `bubble.rs`, `kh_ui::bubble::SpeechBubble`; kh-host `Hud` layer "SpeechBubble", the 15th of `hud_layers::LAYERS`)
- KH2's rounded bubble with a tail, today's window colour (measured), the speaker's name in yellow, KH3's Keyblade
  (`MessageTex01`) bobbing at the bottom middle when a line waits for A, KH3's glove cursor (`DialogYubi`) on choices.
- SIZE FROM THE CONTENT: width = the longest wrapped line between `MIN_W` 140 and `MAX_W` 860 stage px (wrapped beyond),
  height from the lines; eased between lines. One long subtitle line = one bubble (it wraps and grows taller).
- `say(name, text, reveal_cps, sounds)`, `choose(name, prompt, options, start, cancel, sounds)`, `set_speaker(Option<Face>)`
  (the tail's target, every frame), `set_faces(&[Face])` (kept off), `set_safe(Rect)`, `set_cue(Option<bool>)`, `key`,
  `tick`, `close`, `clear`, `take_events() -> Vec<BubbleEvent>` (Opened, Revealed, Advanced, Moved(i), Chosen(i), Closed).
- `Face { x, y, r }` = a face on the 1920 x 1080 stage (KH3's ExactFit 16:9 viewport: on 21:9 the stage is the middle).
- Pad (the HUD's frame, both hosts): A = show the rest / go on / take the option; B = the choice's cancel option;
  D-pad / left stick = the options. Through `Hud`: `bubble_speaker`, `bubble_faces`, `bubble_safe`, `bubble_open`,
  `bubble_drawn`, `bubble_text`, `bubble_clear`; `hud-cmd conv demo | end` (test level / HUD harness demonstration).

### C.2 The runner (kh-host `conv.rs`, `kh_host::conv`)
- `Conversation::new(actors, steps)` (a script) / `Conversation::open(actors)` + `push(step)` (open-ended: out of steps it
  waits, the host ends it). `step(&mut dyn ConvView, &ConvInput)` once a frame; `take_events()`.
- `Actor { id: ActorId (u32, host-chosen), name, role: Leader | Party | Npc }`.
- `Step`: `Line(Line)`, `Choice(Choice)`, `Wait(secs)`, `Direct { directive, wait }`, `Camera(ShotCue)`, `Label(name)`,
  `Goto(name)`, `End`.
- `Line { speaker, listener, text ({leader} = the leader's name), voice: Option<Voice { key, seconds }>, anim, face,
  reveal_cps (None KH's 45/s, Some(0.0) all at once), auto_after (None = waits for A; Some(s) = goes on s seconds after its
  voice ended / it was all shown), shot: Option<ShotKind> }`; `Line::new(speaker, text)`.
- `Choice { speaker, prompt, options: Vec<ChoiceOption { text, branch: Next | Goto(label) | End }>, start, cancel,
  external }`. `external = true`: the pick is the host's (`ConvEvent::Chosen { index, option }`), branches ignored, the
  host pushes what follows.
- `Directive`: `Turn { actor, face: Actor(id) | Yaw(deg) | Point([x,y,z]) }`, `MoveTo { actor, to, run }`,
  `Play { actor, anim }`, `Release { actor }` (he may move until `Hold`), `Hold { actor }`. With `wait`, the runner waits
  for `directive_done(actor)` (or 8 s).
- `ConvInput { dt, combat, voice_playing: Option<bool> }`. `CombatPolicy::End` (default) | `Suspend` (comes back 1.5 s
  after the fight, on the line it was on).
- `ConvEvent`: `Started`, `Line { index, speaker, listener, voice, anim, face }` (play them), `StopVoice { speaker }` (A was
  pressed while the voice played: stop it; the conversation goes on), `LineDone`, `ChoiceUp`, `Chosen { index, option }`,
  `Direct(Directive)` (perform it), `Shot(ShotCue)`, `Suspended`, `Resumed`, `Ended(Script | Combat | Host(why))`.
- THE LOCK: `holds(actor)` = he may not move, attack or be moved by AI (every participant, for the whole conversation,
  except while released); `directed()` = the directive being performed; `speaker()` / `listener()`.

### C.3 The camera (kh-camera `conv.rs`, `kh_camera::conv`)
- `ShotKind`: `OverShoulder`, `TwoShot`, `CloseUp`, `PushIn`; `ShotCue { kind, subject, other, cut }`; `ShotDirector`
  (one shot per line: a two-shot first, a cut to each new speaker, reaction close-ups, push-ins on long lines);
  `ConvCamera` (framing from the subjects' heads, the 180-degree side fixed at the start, the collision pass
  `camera_anim::collide` against the host's world, blend in 0.6 s / out 0.8 s, glides 0.45 s).

### C.4 The Elden Ring host (kh-er `src/conv.rs`, `crate::conv`; for CV2's talk adapter and S8's Donald)
- `begin(Begin) -> Result<ConvId, String>`: `Begin { owner: &'static str, participants: Vec<Participant>, steps,
  open_ended, policy, camera: bool, hold_party: bool }`; refused while another conversation runs, with `conv.on = 0`, or
  without the HUD's bubble.
- `Participant { actor: Actor, body: Body, height_cm: Option<f32> }`; `Body::Leader` (the party's leader: Sora or a friend
  leading), `Body::Member(PartyChara)`, `Body::Chr(FieldInsHandle)`, `Body::Entity(u32)` (an event entity id),
  `Body::Placed` (the owner gives the pose every frame: `place(id, actor, Pose { feet_kh, yaw, height_cm })`).
- `push(id, Step)`, `take_events(id) -> Vec<ConvEvent>` (the owner's copy of every event; read it every frame),
  `directive_done(id, actor)`, `voice_playing(id, Option<bool>)`, `end(id, why)`, `running() -> Option<(ConvId, &'static str)>`.
- `holds_chr(FieldInsHandle) -> bool` / `holds_entity(u32) -> bool`: the NPC is in a running conversation and held (CV2
  holds the game's NPC while true). The leader's pad (sticks and buttons, the HUD's A / B / D-pad excepted) and the
  party's AI (stopped, standing, looking at the speaker) are held by `crate::conv` itself.
- The camera: the shots go to KH3's camera (cam.rs) while KH3's camera has the view; collision with the game's world
  (the camera filter); handed back to the field camera at the end.
- Logs: `conv:` lines (begin / every line / choices / directives / shots / the end and why).

### C.5 Ambient conversations (proximity lines, hostile taunts, battle lines; CV1, 2026-10-16, for CV2)
- `Begin::ambient(owner, participants, steps)` = `Begin { hold_leader: false, hold_party: false, camera: false, .. }` (any
  Begin with `hold_leader: false` is ambient). AMBIENT MEANS: nobody is held (`holds*` false), the pad stays the player's,
  no conversation camera and no shots, no A (the bubble is passive: no key, no Keyblade cue), a fight does NOT end it,
  every line goes on by itself: `auto_after` seconds (default `kh_host::conv::AMBIENT_AUTO` = 1.2) after its voice ended
  (`voice_playing(id, Some(false))`, else `Voice::seconds`) and its text is all shown; a `Choice` step is skipped (its
  cancel option, else the first, is reported as `Chosen`). Events, `push`, `end`, `take_events` as C.4. Runner side:
  `Conversation::ambient = true`; bubble side: `SpeechBubble::set_passive` / `ConvView::conv_passive` (defaulted).
- PRIORITY (one bubble on screen): a HELD conversation wins.
  - held `begin` while an ambient one runs: the ambient one ENDS (`Ended(Host("a held conversation began"))`) and
    the ambient queue is dropped (each: `Ended(Host("dropped: ..."))`); the held one starts.
  - ambient `begin` while a held one runs: REFUSED (`Err`): the owner drops the line.
  - ambient `begin` while an ambient one runs: QUEUED (`Ok(id)`; `push(id, ..)` adds to it), started when the running one
    ends; at most `crate::conv::QUEUE` (3; the oldest is dropped) and dropped after `QUEUE_WAIT` (6 s) of waiting.
  - held `begin` while a held one runs: refused (as C.4).

### C.6 The free conversation (stream CV3, 2026-10-17; supersedes the leader's lock and the camera of C.2 - C.4)
The user: "remove the cinematic camera from talking and allow the player to move the camera and their character, though A
is reserved for moving dialogue along while in conversation unless in combat". State note:
build/sora_tmp/party/IMPL_CV3_STATE.md.
- Runner (`kh_host::conv::Conversation`): `holds(actor)` is NEVER true for the `Role::Leader` actor. `takes_a()`: A is
  the bubble's (held, not over, not suspended, not hands-free). `in_choice()`: a choice takes keys (A, D-pad, B).
  `hands_free()`: a fight under `CombatPolicy::HandsFree` (THE NEW DEFAULT; `End` / `Suspend` stay): the bubble is passive
  (no key, no Keyblade cue), its key events are ignored, each line goes on by itself `combat_auto` s (default
  `COMBAT_AUTO` = 1.0) after its voice / text (its own `auto_after` first), a choice is shown passive and waits for the
  fight's end, then takes keys again. `walk_away: Option<f32>` (default `Some(WALK_AWAY)` = 5.0 m) with
  `ConvInput::apart` (m from the leader to the nearest other participant): farther ends it, the bubble pops out,
  `Ended(EndReason::WalkedAway)` (never an ambient one). `ConvInput` gained `apart` (use `..Default::default()`).
- Hud (`kh_host::hud::Hud::bubble_dpad_only(bool)`): the bubble's options take the D-pad's presses only (the stick is
  the player's movement); default false (the test level keeps the stick).
- Elden Ring host (`crate::conv`): `note_pad` applies `PadRule` (`pad_rule`): `A` (out of a fight: A is the bubble's
  and nothing else), `Choice { b }` (also the D-pad and, `conv.b_cancel`, B), `Free` (a fight: A attacks), `All`
  (`conv.free = 0`: CV1's lock). The sticks are never taken in free mode. `conv.camera` is OFF by default. The party is
  held only while the leader is within `conv.hold_near` (3.5 m) of the others. `Begin::new` defaults to HandsFree.
- TALK PRIORITY: `offer_talk(owner, up)` (a party member's talk prompt is up: Donald's, any later one) /
  `party_talk_offered()`; sora.rs's Y routing asks `er_prompt_may_take_y(prompt)` (= `er_prompt_wins`): Elden Ring's own
  action prompt gets Y only while no party talk is offered and no held conversation of a party owner runs.
- THE LEADER'S FIGHT: `leader_combat(sora_in_party, sora_battle, members_battle)`: Sora's battle flag counts only while
  he is in the active party (`party_scene` returns it so). The unlock host clears the frozen flag of Sora out of the
  party and not staged (`stage_sora_step`).
- Talk adapter (CV2): a talk the leader walked away from is DISMISSED (`talkkh::rules::Dismissal`): no held conversation
  for it again while it goes on, its lines ambient (`AmbientBook::step_with(.., dismissed, ..)`), its pending choice
  answered with its cancel option; forgotten when its talk is over.
- Keys: `conv.free` (1), `conv.camera` (0), `conv.walk_away` (5.0; 0 = never), `conv.combat_auto` (1.0),
  `conv.hold_near` (3.5), `conv.b_cancel` (1).

## Requests

(one line per request: date, from -> to, what)
- 2026-10-15 B -> A: emit the party anim tables in kh-party's `ClipDef` / `AnimSetTable` types (B.7).
  - 2026-10-15 A -> B: DONE as `build/gen/rust/party_clips_n_ex001.rs` / `party_clips_n_ex002.rs` (`node tools/gen_party.js`):
    `pub static N_EX001_CLIPS: &[ClipDef]` + `pub static N_EX001_ANIMS: AnimSetTable` (Goofy N_EX002_*), EVERY row
    (216 / 222), your exact field set (expects ClipDef, ClipNotify, ClipNotifyData, RootKey, SocketKey, AnimSetTable,
    Chara in scope, as party_data.rs); notifies sorted by trigger time (with TriggerTimeOffset), data mapped as your
    party_sheet.js (Attack / State / Raw), root tracks where bEnableRootMotion, `socket` = ATK_01 (Donald's staff
    collision, w_do_WepBase TresAtkColl0) / ATK_00 (Goofy's shield, w_gu_WepBase) per frame of the default weapon.
    NOTE the file names: `party_anim_n_ex00x.rs` are A.2's full tables in OTHER types (they need `party_anim_types.rs`):
    please point your build.rs cfg / include at `party_clips_n_ex00x.rs`. Compile-checked against a copy of your types
    (kh-anim/tests/party_pack.rs `clip_tables_for_kh_party`).
- 2026-10-15 B -> E: `Attach::Party { member: u8, bone: Option<&'static str>, offset: Vec3 }` in `kh_core::host::Attach`
  (plan stream E); kh-party passes `MemberId.0` as `member`.
  - 2026-10-15 E -> B: DONE in kh-core (E.1), as `member: kh_core::PartyMember` (= `PartyMember(id.0)`). Please take seam
    handles from `party::SeamHandles::for_member` (E.2), call `PartyNotify::dispatch` in the pose tick (E.3), and play the
    state voices with `party::voice_play` + `pick_number` (E.4).
- 2026-10-15 E -> A: `bone_world` please also answers SOCKETS of the member skeleton (`atama`, `R_wep_skt`, vfx sockets);
  and keep `kh_core::anim::NotifyDef` as the notify type if at all possible (else tell E the path of your type).
  - 2026-10-15 A -> E: sockets DONE (`bone_world` answers bones then the character's sockets; `weapon_socket_world` the
    weapon's; `set_pose` publishes all of them to party_fx every step). The NotifyDef cannot be kh_core's (its
    NotifyClass enum is generated from Sora's classes only; the friends' sets add 38-class union): the type is
    `NotifyDef` of `build/gen/rust/party_anim_types.rs` with kh_core's EXACT field set and NotifyData variants
    (include it as `mod types { include!(...party_anim_types.rs) }`; per character `party_anim_n_ex00x.rs`
    `SEQUENCES[i].notifies`). B's runtime tables (`party_clips_*`) carry class name + start / end + B's data.
- 2026-10-15 E -> F: once per fixed step, `party_fx::publish_member` (A's bone / socket transforms, B's view: position,
  yaw, on_ground, anim row + time, weapon model) and `party_fx::publish_projectiles` (B's spell actors) (E.6).
- 2026-10-15 E -> D: link pawns / team projectiles take handles from `SeamHandles::team()`; FR voices through
  `party::voice_play(m, VoiceGroup::FriendLink, n, Replace)` (Donald: Meteor 1..2, Flare Force 3..4; Goofy: E's note).
- 2026-10-15 F -> B: `FriendStatus` edits for the camp menu (F.2): `equip_weapon(id) -> Result`, `set_armour(slot,
  Option<id>)`, `set_accessory(slot, Option<id>)`, `set_item_slot(slot, Option<ItemKind>)`, `set_ability(kind, copy, on)`
  (or `abilities_mut()` + `rebuild()`), getters for every `PartyMenuMember` field (incl. resists, exp_level / exp_next,
  open slot counts), and the friends' weapon rows (I03021.. / I03041..: Str / Mag / abilities) for the HUD's Equipment
  preview (a `pub fn weapon_row(id) -> Option<..>` is enough). AI bytes: please store FOUR (F.2: +0x158 battle, +0x159
  ability, +0x15A cure, +0x15B targets), not three.
- 2026-10-15 F -> C: the game's Restore Defaults is Together / Free(Go Wild) / Pinch / all targets (F.2), not
  Together / Balance / Pinch; F passes whatever the menu holds through `set_settings`.
- 2026-10-15 D -> B: implement `kh_party::team::TeamParty` for `Party` (D.3: `team_members`, `team_command`, `fr_drive`)
  and a PASSIVE `NpcStateId::Fr(n)` state that applies the latest `FrDrive` (transform hold, motion by anim-set row,
  visibility, weapon, invincible, collision) and leaves only on `Stop` / KO; `EnterFrState` accepted from any state but
  Die. `team/mod.rs` is D's file: please keep `pub mod team;`.
- 2026-10-15 D -> F: own a `kh_party::team::Team` next to the `Party`; per step after `Party::step`: relay Sora's
  `Targets::friend_link` requests to `Team::request`, call `Team::step(dt, &mut party, &mut party_host)`, answer
  `Targets::friends` from `Team::friend_rows`; feed the step's events to `kh_camera::camera_anim::StateEventCameras`
  and use its view while it returns one (D.4). Debug word `team-gauge full` (D will name the kh-core call).
- 2026-10-15 D -> C: pause the member's tree while its state is `NpcStateId::Fr(_)` (team attack running).
- 2026-10-15 F -> A: please add `PartyDraw::set_pose_view(&mut self, id: u8, position: Vec3, yaw: f32, dt: f32, anim: &kh_party::AnimView, weapon_visible: bool, grounded: bool, world: &dyn IkWorld)` (or the same with plain fields: row, time, weight, prev_row, prev_time, blend_axis): the anim-set ROW names of B's AnimView resolved to sequences / blend-space samples the way `debug_pose` does, the notify window from the previous call. F calls it once per fixed step per member after `Party::step` (party_glue.rs) and drops its capsule stand-ins when `has_member` is true. Spawning: F calls `spawn_member` / `despawn_member` from its party on/off.
- 2026-10-15 E -> B: cast / item voices are voice-group NUMBERS (`party_voice_group_no` before the clip; E.4 "CAST VOICES ARE NUMBERS"), not direct plays: kh-party actions.rs 155/160 (MagicCast via `queue_voice`) and 252-253 (GiveRecovery) would double the clips' own notify voices. Use `target_voice_number` (new, slot 1326) / `magic_voice_number` as numbers; the notify dispatch is `PartyNotify::dispatch` with `SeamNotify` (E.3, changed: neutral notify data, one `Effect` per row).
- 2026-10-15 F -> A (and E): `set_pose` publishes to `party_fx` under `PartyMember(draw id)` = 1 Donald / 2 Goofy, but B's `Attach::Party` uses `PartyMember(MemberId.0)` = 0 Donald / 1 Goofy (spawn order): the taps never match. Please publish under the B member id (e.g. a `member: u8` field in `PoseInput`, F fills it with `MemberId.0`); F treats A's publish as the authoritative one (F does not publish members itself).
  - 2026-10-15 A -> F: DONE: `PoseInput.member: u8` (B's MemberId.0) is what publish_member uses; party_glue.rs must fill
    it (kh-app does not build until it does). Also DONE: `PartyDraw::set_pose_view(id, member, position, yaw, dt,
    &kh_party::AnimView, weapon_visible, grounded, world)` (rows resolved like debug_pose, 1D blend spaces at blend_axis,
    notify window from the previous call; it ends any debug pose of the member).
  - 2026-10-15 A -> F (menu request): DONE: `spawn_menu_copy` / `step_menu_copy` / `play_menu_row` / `menu_copy_root` +
    `Instance::PartyMenu(id)` (section A.3 "Added"). Note: with --no-party your glue despawns A's debug members; the
    debug words now work with --party (a debug pose owns the member until `pose <who> off`).
- 2026-10-15 F -> B (for E): the projectiles need their body-effect handle (`Projectile::effect: u32`, the seam handle of the effect spawned with the actor) so F can call `party_fx::publish_projectiles`; F calls it once that field exists (position, normalised velocity as forward).
- 2026-10-15 D -> F (answer): `team-gauge full` = `sora.combat_mut().friend.debug_fill_gauge()` + `team.debug_ready_all()`
  (section D.5). Also please equip UNISON_FIRE / _BLIZZARD / _THUNDER / FUSION_SPIN / _ROCKET in the level-99 test loadout.
- 2026-10-15 B -> C: B's swim answers are false on land for both members (`BodyView.swimming` false; `function` IsSwimming / IsUnderWater / IsUnderwater / IsInWater / IsDiving / IsSwimSurface / IsWaterSurface = 0; test `nobody_swims_on_dry_land`). Any OTHER name answers None: if the underwater branch is gated on a name B does not know, treat None as false or tell B the name. `TSID_STATE_AI_SWIM` is never B's state.
- 2026-10-15 D -> F: in `party_glue.rs::team_camera` pass `accompany: self.team.accompany()` (not `camera_target()`),
  apply `out.view.fov` and `out.shake` (kh_camera::shake offset) to the drawn camera, and forward
  `TeamEvent::ActorSpecificAction` to `on_actor_action` (D.3a).
- 2026-10-15 D -> F: for scripted runs of each team attack: `team-gauge full KIND` (KIND 207 Meteor, 208 Flare Force,
  195 Bombardier, 193 Trinity Guard) = the two calls above + `glue.team.debug_offer(KIND)` (the owning friend offers it
  at the next step, in or out of battle). Then `tap y` (after 0.7 s: appear 0.2 + start delay 0.5) runs it.
- 2026-10-15 A2 -> B2: `AnimView` has one blend-space axis; the friends' sets have 2D `BlendSpace` rows (manifest
  `logical_animations[].blend_space.axis_y`). Please add `pub blend_axis_y: f32` to `kh_party::AnimView` (the second
  input of the playing row in the asset's own units, 0 for 1D rows / sequences; same timing as `blend_axis`). NOTE the
  axes of the 2D rows: `WalkRunN/W/B` X = -300..300 (Donald) / -180..180 (Goofy) [the turn input], Y = 140..680 = the
  SPEED (Move1's range); pawn.rs:552 now puts the speed into `blend_axis`, i.e. on X. For a 2D row please put X in
  `blend_axis` and Y in `blend_axis_y`. A2's `set_pose_view` evaluates the full stock 2D grid (all samples, bilinear
  over the grid points) at (blend_axis, blend_axis_y) as soon as the field exists (until then, reading pawn.rs:552, a
  2D row is evaluated at (X 0, Y blend_axis): straight walk / run by speed, no turn lean).
- 2026-10-15 B -> C: `CanBattle` is answered by kh-party (`Party::function` / `AiBody::function` and `BodyView.can_battle`),
  per the game's rule 0x141257340 (exec 0x1427E8F00 -> vt+0x2880): true only while the member's battle flag is on AND the
  battle has lasted more than 0.5 s (latch +0xCA8, cleared when the flag goes off); out of battle it is false (KO is not
  part of the rule). Also done for C's relay: `IsActionOff` = 1; `BodyView.state` = ETresStateID names for every state;
  an Action / Cure that cannot start (wrong character, MP charging / not payable, refusing state) answers `Failed` at
  `command()` time and is not queued; `Stop` cancels a running action (gravity / hover overrides cleared).
- 2026-10-15 D -> B: an `EnterFrState` command must not be replaced by an AI command issued after it but before your
  intake (C's tree issues MoveTo / Action every few frames: the unison partner and Goofy in Trinity Guard missed their
  FR state until D re-sent the command every step). Suggest: a pending EnterFrState wins over any later non-`Stop`
  command; also emit nothing from the AI while `Fr(_)` (C: `set_stop_ai(true)` on entering Fr, false on leaving).
  - 2026-10-15 B -> D: DONE: a pending `EnterFrState` (sent this step, not yet taken) wins: any later command of the
    same step except another `EnterFrState` / `Stop` answers `Failed` at once (the FR request outranks the AI's in the
    state request priority [inf]); once in `Fr(n)` every command but `Stop` is refused as before. No re-send needed.
- 2026-10-15 P -> B3 (stream P: playable Donald / Goofy, state note `build/sora_tmp/party/IMPL_P_STATE.md`): (1) P owns
  `crates/kh-party/src/pad/**` (`pub mod pad;` in lib.rs, as D's team/); (2) `NpcPawn.pad: Option<crate::pad::PadState>`
  (None = AI, nothing changes); (3) `NpcStateId::Pad(u8)` (class "TresNpcState_Pad (port)", accepts commands); (4) in
  `NpcPawn::step` after the command intake `if self.pad.is_some() { crate::pad::pre_move(self, cx, host); }` and in
  `main_proc` first `if self.pad.is_some() && crate::pad::owns(self) { return crate::pad::tick(self, cx, host); }`.
  Actions / spells / items keep going through `MemberCommand::Action / UseItem / Cure`.
- 2026-10-15 B3 -> P: DONE. `pub mod pad;` (stub `src/pad/mod.rs`: `PadState` (Clone, Debug, Default, PartialEq),
  `pre_move(&mut NpcPawn, &StepCtx, &mut PartyHost)`, `tick(&mut NpcPawn, &StepCtx, &mut PartyHost)`, `owns(&NpcPawn) -> bool`
  = false; P owns `src/pad/**` and replaces it); `NpcPawn.pad: Option<PadState>` (None in `new`); `NpcStateId::Pad(u8)`
  ("TresNpcState_Pad (port)", accepts commands; kh-ai name "TSID_STATE_PAD"); `pre_move` right after the command intake of
  `NpcPawn::step`; `main_proc` first line `if pad && owns -> pad::tick`, and an arm `NpcStateId::Pad(_) => pad::tick`.
  All marked `// (party P)`.
- 2026-10-15 C3 -> B: please answer the C.2 C3 names: `HasAbility:<kind>` (equipped abilities; DONALD_* / GOOFY_TORNADE / GOOFY_TURBO / GOOFY_FLIP gate the attacks once B answers), `GetAbilityNum:ITEM_UP`, `IsCommandRestricted:6`, `IsBadStatusBound`, `GetTimeMotionless` (seconds since the pawn last moved, the movement component's rule if B has it). Unknown names keep answering None.
- 2026-10-15 C3 -> F: for the final AiWorld pass please implement `player_focus`, `member_mp` (the other member's MP from the party view), `camera_far` (or leave None = infinite), and keep `camera()` = the drawn view; Sora's state name TSID_STATE_COVER if the port ever has cover.
  - 2026-10-15 B3 -> P: DONE (relayed): `pub mod pad;` + stub, `NpcPawn.pad`, `NpcStateId::Pad(u8)` (TSID_STATE_PAD), the two
    hook lines, all marked `// (party P)`. P owns `src/pad/**`.
- 2026-10-15 P -> B3: `notify_seam.rs`: `mod full` -> `pub(crate) mod full` (pad/ reads A's SequenceData root tracks of the
  RunStartN samples 0007 / L / R / BL / BR; no second include of the 1.3 MB tables).
- 2026-10-15 B3 -> P: DONE: `notify_seam.rs` `pub(crate) mod full` (cfg kh_party_anim_full); nothing else changed.
- 2026-10-15 B3 -> C3: DONE: `Party::function` answers `HasAbility:<kind>` (equipped count > 0), `GetAbilityNum:<kind>`
  (equipped count), `IsCommandRestricted:<n>` (0: the system restriction flags FUN_140E8E870 are never set in the port),
  `IsBadStatusBound` (in a BadStat state or SNEEZE on him, FUN_141267540 [inf on FUN_1405ACFE0]), `GetTimeMotionless`
  (seconds under 10 cm/s [inf], movement +0xC68). The delayed `Stop` after a MoveTo abort and `Idle { time: f32::MAX }`
  need no change (Stop cancels any state that accepts commands; Idle MAX runs until replaced / aborted by a hit).
  Also B3: CanCastCure / GetMagic / GetMPChargeTimeLeft / IsEnableWarp (C2); Cure on a KO'd friend heals half and revives.
  - B3 -> P: DONE (`pub(crate) mod full`).
- 2026-10-15 P -> B3: `Party::spawn_as(chara, id: MemberId, position, yaw) -> MemberId` (member at index id.0, Vec padded with
  None, same seed rule as `spawn`): the save point's Party Formation removes / re-adds friends at run time and E's
  `PartyMember(id.0)` / A's party_fx keys need Donald = 0 / Goofy = 1 always.
  - B4 -> P: DONE (2026-10-15): `Party::spawn_as(chara, id, position, yaw)` (pads with None, replaces the slot, `spawn`'s seed
    for that id); `Party::remove(id)` / `Party::despawn(id)` leave a None slot (no id moves). Test `spawn_as_keeps_ids`.
  - B4 -> P: DONE (`spawn_as`); P uses it in kh-app party_glue (spawn, spawn_one).
- 2026-10-15 P -> D3 (user rule: only the player-controlled leader starts a team attack): with a friend leading, P passes the
  pad's Y / LT into Sora's core (Sora = AI Sora, his situation queue / FR states run as now) and AI Sora never presses Y.
  Asks: (1) confirm executing through Sora's core is right and offers come with a friend leader (P feeds kh-party a
  leader-relative SoraView; should FriendLinkRequest::Sora's range checks use the leader?); (2) Unison / Fusion on the
  LEADER's combo: a call like `Team::try_leader_unison(leader, partner Sora) -> bool` P can make at the pad combo's
  finisher; (3) Trinity Guard's menu: P passes the whole pad to Sora while an FR state runs - confirm.
- D3 -> P (answer, 2026-10-15; the details are in D.6): (1) Yes, run it through Sora's core, and offers keep coming
  while a friend leads. Call `Team::set_leader(Some(id))` each step: the other friend's link-point distance then
  measures to the leader, the leader's own to Sora. Keep `FriendLinkRequest::Sora` as it is (Sora's gauge and
  battle flag). Set `sora.combat_mut().friend.sora_is_ai = true` while a friend leads, so AI Sora's own combos
  never roll a unison. (2) Use `sora.combat_mut().friend.roll_leader_unison(&sora.status().abilities.clone(),
  leader_uid, leader_pos, sora_pos)` at your combo step before the finisher. Skip your finisher when it returns
  true: AI Sora plays the unison within 0.5 s, and the friend enters FRDonaldSora / FRGoofySora on him through
  `team_command`. Please keep the leader in that FR state until `MemberCommand::Stop`. (3) Yes, pass the full pad
  (A / B / Y and the left stick with the camera yaw) into Sora's core while an FR state runs.
- D3 -> B3: please implement `TeamParty::fr_hit` for Party (D.6): `actions::deliver` of the member's own row by
  name (Goofy `FR_GOOFYSHOOT_RUSH`, n_ex002). Until then the rush lands no hits; its EXP blast still hits.
  - B4 -> D3: DONE (2026-10-15): `impl TeamParty for Party` has `fr_hit(id, row, target, at, host)`. It looks up the member's
    own row with `tables::attack_row(chara, row)` and delivers it with `actions::deliver`, which now returns the host's
    `accepted` (part 0, the direction from the member to `at`). An unknown row or member answers false. Test
    `fr_hit_delivers_the_members_own_row`.
- 2026-10-15 C4 -> B: (1) please answer `GetDamageReactionCount` (count of accepted damage reactions, C.2 C4) — the Interrupt branch (BT_n_Common_OnDamage_01) restarts on each one; (2) make the `Idle` ticket succeed at the game's IdleFinished moments (C.2 C4: leaving idle; in battle every idle clip length; out of battle at the clip end) — today `Idle { time: f32::MAX }` never succeeds, so the friends' Idle task waits for an abort; (3) `GetTimeMotionless` is no longer read (kh-ai computes the game's rule); (4) preflight E-L0-KEY: `party-states` has a duplicate key `Jump`.
- 2026-10-15 C4 -> F: AiWorld C.3 C4: `gimmick_obstacles` / `volumes` defaulted to none (the test level has neither); `camera_far` None = 2097152 cm is right for the test-level camera; feed `player_focus` (Sora's focus, max) so the PINCH focus need works.
  - B4 -> C4: DONE (2026-10-15).
    (1) `Party::function("GetDamageReactionCount")` returns `NpcPawn::damage_reactions`. It goes up by one each time kh-react
        accepts a hit's damage reaction (`HitOutcome::Started` in `receive_hit`).
    (2) The `Idle` ticket succeeds on IdleFinished: when the pawn leaves Idle (also before an abort by a hit or KO); in
        battle once per idle-clip length; out of battle when the idle clip ends or wraps.
    (3) `GetTimeMotionless` is still answered but no longer needed.
    (4) The duplicate `Jump` row in party-states is gone: the B4 note is merged into the original row.
    Test `idle_finished_and_the_damage_reaction_count`.
- 2026-10-15 D4 -> P (notice): party_glue.rs `team_camera`: `CameraTargets { sora: (sora.position, ..) }` (was
  `capsule_center`): KH3's camera base is Sora's root = his feet (0x140621E00) [C]; one token, marked `(party D4)` (D.7).
- 2026-10-15 D4 -> A (notice): char_material.rs: `Instance::Party(3)` = the team props (world rig + KHS preset added to the
  two setup loops, 2 tokens + doc line, marked `(party D4)`); party_draw.rs: the "(party D3) team props" section now applies
  the props' translated game materials (`lib.apply(.., "o_ex208_0" / "o_ex210_0", Instance::Party(3), ..)`) and keeps one
  entity per prop; new system `team_mesh_pitch` after `animate` (marked `(party D4)`).
- 2026-10-15 D4 -> A (notice): tools/sora_material_pack.js `vec()`: an operand without a swizzle (an indexable temp
  `x0[r0.x + 0]`, e.g. a dither matrix) now fails that shader cleanly (`bad(...)`) instead of crashing the build (1 line,
  marked `(party D4)`); shaders that translated before are unchanged.
- 2026-10-15 B5 -> C (notice + additions): (1) the friends' jumps and falls are now `TSID_STATE_NAVLINK_JUMP` (0x2B,
  `UTresAICharState_NavLinkJumpCodeDriven`, as in the game: RequestJump 0x14054C2E0 takes 0x2B before 3), not
  `TSID_STATE_JUMP`; also new body states `TSID_STATE_CLIMBING` / `TSID_STATE_DANGLE`. Your warp rule (controller.rs
  `warp_wanted`) lists JUMP and notes "NAVLINK_JUMP only in phases not modelled": with B5 a falling friend reports
  NAVLINK_JUMP, so please model those phases if the warp should see a falling friend. (2) kh-ai API additions, defaulted
  (nothing else changed): `AiBody::wants_warp_query(&self) -> bool { false }` and `AiBody::warp_query_result(&mut self,
  Option<Vec3>) {}`; `AiController::tick` calls the new private `state_warp_query` (runs `FriendData.WarpQuery` /
  EQSQuery_NpcWarp with the usual querier, no params, best item) before `warp_decision`; test
  `crates/kh-ai/tests/b5_state_warp_query.rs` (new file). All marked `(party B5)`.
- 2026-10-15 B5 -> A (notice, marked edits in party_draw.rs): `Member.pitch: (f32, f32)` (both literals),
  `PartyDraw::set_body_pitch(id, pitch)` (call before `set_pose_view` / `set_pose`), `component_to_world_pitched` used by
  `step_member` (IK / sockets / party_fx), `bone_world`, `weapon_socket_world`, `feed_bones`, and `animate` draws the
  actor rotation (pitch, yaw, 0) (`ue_rotator` + `rot_to_bevy`); pitch 0 = the old yaw-only path unchanged.
- 2026-10-15 B5 -> P / F (notice, two marked lines in party_glue.rs): `self.party.set_ice_rails(&sora.magic().rails)`
  right before `self.party.step` (the friends ride Sora's Blizzard ice rails), and `draw.set_body_pitch(id, v.pitch)` in
  the pose loop before the pad-loco branch / `set_pose_view`.
- 2026-10-15 B5 -> coordinator (relay to the kh-core owner, finding only): `UTresCharState_Dangle` tick 0x14150F220 picks
  the climb / let-go rule by pawn vt+0x1A20 = (vt+0x1A18() - 1) < 2, i.e. m_CharInputType PAD / DEBUG_PAD (vt+0x1A18
  0x1405AA1E0: an AI controller answers its +0x4E0 m_CharInputType, a player controller answers 1): an input-driven pawn
  climbs under 45 deg and lets go from 135 (stays between); TRES_CHAR_INPUT_AI climbs under 90, else lets go. kh-core's
  Sora port (states/dangle.rs `dangle_climb_angle` 90) uses the AI rule; Sora is player-controlled (type 1).
- 2026-10-15 B5 -> coordinator (spec note): docs/sora-actions-spec.md II.6 says the obstacle probe 0x140634E60 "returns false at once unless the pawn is a TresPlayerPawnBase"; the asm (0x140634EEB / 0x140634F26 -> 0x140636B76) shows any other pawn takes FUN_140636BF0 instead (the NPC climb probe, ungated; kh-party `climb::climb_probe`). True as written for Sora.
- 2026-10-15 I1 (final pass) -> C / coordinator: B5's warp relay DONE in kh-ai controller.rs `warp_wanted` (marked `(party I1)`):
  FUN_141258790's switch passes NAVLINK_JUMP unless the jump state's byte +0xA9 is set (only the rail leave 0x141252210 sets it):
  the body answers `function("IsRailLeaveJump")` (kh-party: NavLinkJump with `trav.jump.steer`; None = 0); CLIMBING / DANGLE never
  warp; BURN with BadStatBurn +0xAC not modelled. Test crates/kh-ai/tests/i1_warp_states.rs.
- 2026-10-15 I1 -> coordinator: B5's Dangle relay DONE in kh-core states/dangle.rs (pad rule 45 / 135, STAY between; rows
  dangle.climb_angle_pad / dangle.release_angle_pad); B5's spec note DONE (docs/sora-actions-spec.md, obstacle probe 0x140634E60).
- 2026-10-15 I1 -> B / P / F / A (notices, kh-party API): `NpcPawn::command` now applies the game's request gates (RequestAction
  0x140547B40 / CanStart 0x140518070 = `actions::viable_in`, TurnTo FUN_140530C80, Warp ungated) instead of B5's "refused until
  CanChange" rule (kept for UseItem / PlayMotion / Idle): an AI member refuses an Action / Cure / TurnTo outside Idle / Move (and a
  second action while one runs); pad-driven members (P) are exempt. `climb::ledge_probe` is the NPC attempt 3 of FUN_140638900
  (sphere 1.5 x capsule radius on the axis, `climb::ledge_probe_radius`); `climb::dangle_tick(pw, cx, world)` (new world argument);
  `climb::ledge_supports`; `DangleRun.local` / `.based` (moving ledges); `jump::in_water` / `jump::request_swim` (answers false: the
  swim locomotion is not ported); `NpcPawn.ignore_overlap_force`; `actions::viable_swim`; team/link.rs `SORA_CENTER_BONE_Z` 102.338.
- 2026-10-15 I1 -> all hosts (notice, kh-core API, additive): `World::component_motion(component) -> Option<ComponentMotion>`
  (defaulted None) + `kh_core::ComponentMotion { location, rotation, scale, movable }`: a host with moving geometry answers it and
  the Dangle state (Sora's and the friends') follows Movable ledges (+0xA4, the base, the support test FUN_14064A090).
- 2026-10-15 I1 -> D / F (notice, kh-camera): `camera_anim::frame_target` / `Framing` = ATresCameraTarget vt+0x8F8 0x1406223B0; the
  state-event cameras' compose uses it before the base distance (`sweep_start` stays the fallback).
- 2026-10-15 I1 -> A / D (notice, tools/sora_material_pack.js): indexable temps (`x#[...]` as WGSL arrays) and `bfi` translate; the
  meteor (o_ex210 ymt_mtr_lit_00o) mask is translated; the friends' materials.wgsl are byte-identical before / after.
- 2026-10-15 I2 notice (additive): kh-party `MemberView.focus: TargetRef` (the AI focus, for the head look-at); kh-anim
  `Pack.look_at`, `Animator::set_look_focus(Option<Vec3>)` / `look_at()`, module `lookat`; kh-app `PartyDraw::set_look_focus`,
  `PartyGlue::{ai_camera, member_mps, sora_look_point}`, `AppAiWorld.mps` / `LeaderAiWorld.mps` (member_mp); the party's effects
  in PACK/party/vfx (kh-app vfx.rs `party_vfx_dir` / `pack_path`, vfx_material.rs shader tables), never PACK/vfx.
- 2026-10-15 ER S0 -> all hosts / S1-S7 (notice): the test level's party host is `crates/kh-partyhost` (section ER); kh-app
  party_glue.rs / party_play.rs / party_draw.rs only delegate (`PartyGlue` derefs to `PartyRuntime`; `PartyPlay` is gone:
  `party.lead`). kh-er should drive `PartyRuntime` (S1) and never copy party_glue / party_play code.
- 2026-10-15 ER S1 -> all (notice, additive, kh-party): `NpcPawn.steer: Option<(goal, waypoint)>` (None everywhere but kh-er:
  the Move state unchanged), `Party::set_steer` / `Party::move_goal`, `pawn::STEER_GOAL_SLACK`; a pending `ClimbLadder` wins
  over later commands of the step but Stop / Warp (ER.9). kh-partyhost: `StepOut.events`. Test crates/kh-party/tests/er_s1_steer.rs.
  `Party` Debug dumps change (the new field), as with S0's `ladder`.
- 2026-10-15 ER S1 -> S2: `lead.update` / `lead.apply` + `core.counter_move(seat)` are called by S1 in sora.rs right after
  `party.after_sora_step` (do not add a second call); party/mod.rs declares `leader` / `climbcap`. S1 observes the body's
  ladder itself for the trail's ladder links (`LadderObs`: the feet where the climb began and ended, the yaw on it);
  `lead.ladder()` is not needed by S1. Your two requests are done: Cures / items -> `status::on_party_effect` /
  `on_party_item` (party/mod.rs), AI Sora's ladder / stuck warp fallback (glue.rs `sora_fallback`).
- 2026-10-15 ER S1 -> S3: `feed.wants` is filled on every party step, also parked (at the leader, unhittable); empty only
  while `party.er = 0`; mod.rs declares none of your files.
- 2026-10-15 ER S1 -> S4: draw while `core.visible()` (false in a cutscene / the ground wait / `party.er = 0`); S1 mounts the
  equipped weapon on each `core.poser` member. A reseat sets `rt.previous = rt.current` (no slide from the old place).
- 2026-10-15 ER S1 -> S6: formations from the menu / a load go to `core.asked_formation` (and AI settings to `core.asked_ai`):
  taken at the next `route`. The core lives on across Sora hosts: when a DIFFERENT character's save is loaded (the title,
  a new game) your save path must hand the runtime that save's record (`rt.from_save`, or a fresh progression) - S1 does
  not know which save is loaded.
- 2026-10-15 ER S1 -> S8: `core.set_unlocked(&list, now)` on every change of the unlocked set (until the first call everyone
  is available); the starter as leader: `core.asked_formation`.
- 2026-10-15 ER S0 -> B / C / P / all matching on kh-party enums (notice, additive): `MemberCommand::ClimbLadder(LadderInfo)` and
  `NpcStateId::Ladder` (port design, kh-party src/ladder.rs, `Party::ladder`); an exhaustive `match` on either needs the arm
  (kh-party ai_body `state_name` has it). `NpcPawn.ladder` is a new field (Debug dumps of `Party` change accordingly).
- 2026-10-15 ER S3 -> S1 (notice): the ally bodies read `core.feed.wants` and queue `AllyHitIn` themselves (sora.rs, marked
  "(ER party S3)", ER.11): nothing for S1 to call. Please keep `feed.wants` set on every party step (also while parked) and
  keep `party/mod.rs` from declaring `bodytest` / `allybody_live` (S3 declares them by `#[path]`).
- 2026-10-15 ER S3 -> S2: (F1 part withdrawn: see the next S3 -> S2 line) S3 drains your
  `take_ally_cures()` every frame and calls `cure_chr` on the friend's body (the `#[allow(dead_code)]` can go).
- 2026-10-15 ER S3 -> S6 / S7: the save point's "cure every member" and a team attack's invulnerability need nothing from
  you beyond S2's ally-cure queue / `AllyWant.invulnerable`; `allybody::handle(chara)` gives a friend's body if needed.
- 2026-10-15 ER S2 -> S4: draw the friend LEADER with `lead.overlay(chara, &mut posed)` after `core.poser.pose(..)` (also
  while the party is parked in a real-time cutscene when `lead.cut_on && !lead.leader_hidden()`); an AI ally whose
  `rt.party.ladder(id)` is Some with `lead.climb_pose(chara, &run, v.position, v.yaw, &mut posed)`; the hero light on the
  leader's weapon tip while `kh_er_render::pointlight::keyblade_wanted()` and `lead.light != Some(Sora)` (U5).
- 2026-10-15 ER S2 -> S6: a friend leader's KO is handed to the game as Sora's death (`died` -> the body's HP 0); the KH3
  game-over menu (`hud::gameover_holds`) is driven by Sora's HP today: please open it for the leader's KO
  (`rt.lead.leader_state.1`) and revive the leader at the respawn. The save point: `status::request_all(active, leader,
  why)` cures every member's body.
- 2026-10-15 ER S2 -> S1 / S5: a Cure or an item landing on a member: `status::on_party_effect(name, member, leader)` /
  `status::on_party_item(item, member, leader)` (Sora's own Cure on the friends comes through `heal_ally`: route those
  heals' effect kinds too).
- 2026-10-15 ER S2 -> S1 (done by S2, marked): sora.rs takes `world.flat` (Sora's level floor while he follows the body)
  away around `party.after_sora_step` and puts it back after: the friends' casts never stand on it. Your `route` still
  sees it (its casts only steer).
- 2026-10-15 ER S2 -> S7: the camera target is the leader's: `lead.view()` (or `rt.camera_target`); on a ladder Sora's seat
  lags the body (`ladder.cam_tau`), a friend leader's too (his figure is placed on the body by the retarget).
- 2026-10-15 ER S6 -> S1 (two lines in party/mod.rs, without them S6 is inert): in `create()` right after
  `PartyCore::new(..)`: `crate::partyui::configure(&mut core.rt);` (the real progression: KH3's init data, not the
  level-99 test loadout); in `after_sora_step()` right after `core.step(..)`:
  `let s6 = crate::partyui::sync(&mut core.rt, sora, core.unlocked(), now);` then hand `s6.restore_unlocked` /
  `s6.restore_unlock_record` to S8's restore (WITHOUT `set_unlocked`'s "newly unlocked join" rule: a load must keep the
  saved formation) and, when the core is switched off or dropped, `crate::partyui::reset()`. Formation asks go through
  `rt.set_formation` (the same as `asked_formation`).
- 2026-10-15 ER S6 -> S8: who is unlocked = the `unlocked` you give S1's core (S6 saves it as `party.unlocked`, locks the
  Party Formation rows of the others, drops the friends' weapons only for unlocked friends); anything else you need kept
  (the starter, scenes seen): `partyui::set_unlock_record(json)`, handed back at a load in `SyncOut`. New game:
  `kh_partyhost::starting_formation(starter)` (the starter alone, leading; the shared total 0 = level 1).
- 2026-10-15 ER S6 -> S4: the game over's fallen friend leader is posed (kh-er void.rs `friend_pose(scene, dt) ->
  (chara, component_to_world, kh_anim::Pose)`, his own skeleton from `PACK/party/n_ex00x`, KO clip BadStatStunLoop,
  feet 30 cm up): please add `kh_er_render::set_void_char(..)` (or a `VoidFrame` party body) and call it at the marked
  line in void.rs `frame` (`let _friend = friend_pose(..)`); until then Sora's copy is drawn. The camp menu's friend
  panels (portrait.rs) need your `set_party_portraits` the same way.
- 2026-10-15 ER S6 -> S5: blood on every member is cleared at the save point by your per-member blood (status.rs
  `at_save_point()` is the leader's rest; S6's `partyui::savepoint_rest` runs at the same moments).
- 2026-10-15 ER S3 -> S2 (notice, user decision): THE F1 CALL IN sora.rs `damage_in` IS GONE (the hit-redirection fallback
  was deleted by the user: "please delete that backup ... that is the worst shit ever"). A hit on the leader's body always
  stays the leader's; `allybody::redirect_hit` no longer exists. Nothing to keep in `damage_in` for S3.
- 2026-10-15 ER S3 -> S1 (notice): the user's KO rule ("Enemies should ignore [dead friends] until they recover") rests on
  `AllyWant.ko` / `hittable` of `feed.wants` the same step the friend goes down and gets up: please keep that timing.
- 2026-10-15 ER S2 (follow-up, the user's rules) -> S1: (1) B on a ladder with a friend leading = his JUMP OFF
  (`LeadOrder::JumpOff(away yaw)` -> `leader::ladder_jump_off(&mut NpcPawn, yaw)`: his pad's `Pad(JUMP)` launched at once
  from the ladder at Sora's power, no ground, no level floor; sora.rs keeps feeding Sora's jump-off pad frame (B held,
  stick away 0.4 s), which your `route` already hands to the leader). (2) AI SORA CLIMBS behind the leader: sora.rs holds
  his kh-core step (not stepped) and moves him by kh-party's own ladder run (`climbcap::begin_run` / `advance_run`, the same
  paces), his figure = the leader's captured climb on his skeleton, released by a teleport at the top / bottom. It begins
  by itself at the leader's finished climb (`lead.link`; AI Sora at its foot / head, the leader past it; key `party.climb`
  0 = never). If your trail finds the ladder for him first, call `lead.sora_climb_start(info, at, yaw,
  sora.output().position)`. Your 8 s ladder fallback / reseat may still warp him: a teleport during his climb ends it.
- 2026-10-15 ER S7 -> S4: the team props: `crate::party::team::props(&core.rt)` (class, model `o_ex210_0` / `o_ex208_0`,
  index, KH3-space actor `trs()`; put props.json's `relative_rotation` on top and convert with the frame) for your TEAM
  `CharDraw.props`, drawn while `core.visible()`; Goofy in his Bombardier rush: his actor turned by
  `core.rt.team.mesh_pitch(member)` (kh-partyhost `component_to_world_pitched(pos, yaw, pitch)`), as kh-app's
  `team_mesh_pitch`. Sora's own pitch is done (sora.rs submit). The friends in their FR states need nothing more: their
  views carry the link pawns' transforms and `FrDrive` visibility (`MemberView`).
- 2026-10-15 ER S7 -> S5: the team attacks' sounds / effects already go through `StepIn.audio / effects` (D's
  `SeamHandles::team()`), the meteors' / missiles' body effects follow `feed.projectiles`: please keep publishing them only
  when `feed.stepped`. Nothing else is asked of you for S7.
- 2026-10-15 ER S7 -> S1 (notice): cam.rs TAKES `core.feed.lock_change` (the lock-on camera applies it once). S7 owns
  nothing in glue.rs; `party/mod.rs` got one marked line (`pub mod team;`).
- 2026-10-15 ER S5 (notice, additive): THE PARTY'S SOUND / EFFECTS IN kh-er (fx.rs + new fx_party.rs). sora.rs calls
  `fx.set_party(core.map(|c| (&c.feed, c.visible())))` before `fx.after_step` (one marked line). kh-host `EffectHost`:
  `load_party_root(pack)` / `preload_party()` (PACK/party/vfx as a SECOND ROOT: its effects for names the main pack lacks,
  party_host.json + team_host.json classes; its own materials / meshes named `party:<name>` = `kh_host::vfx::
  PARTY_NAMESPACE`), `set_party(members, projectiles)`, `Anchor::Party`, `party_loops()`, `stop_party()`, `has_effect()`;
  the "new core" check and Sora's animation-change end now only look at handles below `PARTY_HANDLE_BASE` (before: the
  next Sora spawn after a party spawn ended every effect alive). kh-er-render vfx.rs: a `party:` name is made from
  PACK/party/vfx's own shading table and DXIL (`kh-shaderc --vfx --pack build/sora_pack/party` -> PACK/party/vfx/er), never
  the main table (cache keys of the party root carry the prefix). kh-host `HostAudio::step_with(step, before_run)`
  (the members' motion sounds before the engine step). kh-audio `Engine::actor_hit_se` + kh-host session: a party
  handle's ACTOR_PAWN_HIT plays only the class's hit SE. kh-party party.rs: the spell actors now PLAY their pawn-hit SE
  (`SoundPlay { id: ACTOR_PAWN_HIT, name: class }`, contract E.5; it was never sent): the S0 trace reference gains those
  sound calls at each party projectile pawn hit.
- 2026-10-15 ER S5 -> S4: (1) the friends' blood: `crate::blood::party::party_look(PartyChara) -> Option<BloodLook>` (body =
  him, blade = his weapon; the save point washes every member, S6's request) for your `*_bl` block per friend; (2) the
  renderer's cargo feature `party` is yours to declare (S5 declared it for a moment: it switched on your hud.rs code before
  lib.rs had `mod party`, so it was taken out again; S5's renderer code needs no feature: it is inert without `party:` names).
- 2026-10-15 ER S5 -> S3: please expose the ally body's ground material (`hit_material` of the body's physics module, when
  the body stands on something: `Option<i32>`); fx_party's footsteps use `FxParams::member_surface(own, leader)` and take the
  leader's surface until then.
- 2026-10-15 ER S5 -> S8 (notice): unlock/host.rs, two marked lines: the celebration voice and Donald's talk voice keep
  `voice_play`'s `Attach::Party` (a World attach resolved "BattleStart:1" with Sora's routes); a staged friend's sounds /
  effects resolve from `unlock::stage_taps()` (fx.rs `set_party`); the burst's handle 0x7E00_0000 no longer triggers
  Sora's "new core" rule.
- 2026-10-15 ER S4 -> S6 / S2 / S7 / S8 / S5 (notice, ER.14): the friends are drawn (partydraw.rs): S2 overlay / climb_pose, S6 void.rs marked line (`partydraw::void_char`) and the camp panels (hud.rs marked line after `portrait::frame`), S7 props + mesh pitch, S8 stage draws, S5 party_look. The renderer feature `party` is S4's (kh-er `party` turns on `kh-er-render/party`).
- 2026-10-15 ER S4 -> S6: Sora's OWN camp picture is still placed on `charaWindow01` (portrait.rs `placement` = PANELS[0]); with a friend leading the panels order may put Sora elsewhere (kh-app menu_model.rs `sora_top` rule): please place him by `hud.camp_panels()` as kh-app does.
- 2026-10-15 ER S4 -> S5: the hero light's body shadow in the light pass (plight2, `er.caps`) is Sora's capsules only; the friends throw no shadow from it (their contact / thrown sun shadow and their own point lights are drawn).
- 2026-10-15 X2 -> X3D (notice): GRAND MAGIC'S Y IS X2'S (pad/menu.rs `MenuAction::Grand` -> `leader_magic::request_grand`:
  the -za spell under Donald's `MagicCast` at 0.85x, cast effect 1.5x, no MP, invincible until the shot via
  `leader_magic::invincible`, the offer taken off the caster). Please do not start a second Y path for it; if your
  `GRAND` pad state should own the cast, say so here and X2 removes the menu path. Y PRIORITY (implemented): a TEAM-class
  entry in Sora's queue (a team attack the leader may start) comes first: Donald's offers are hidden and Y is forwarded to
  Sora; otherwise his ready offer takes Y (`PadState::x2.y_taken`, the host does not forward it). Donald's own attack rows
  (n_ex001_AttackDataTable) carry m_FormPoint 0 [A], so his melee feeds the gauge nothing; your Unison / FR rows'
  form points through `SpellCaster::add_form_points` are fine. Shots you push to `PadState::x2.shots` are released by the
  caster at the party step (feed present).
- 2026-10-15 X2 -> K1 (notice): a friend leader's `cmc.max_walk_speed` is MySprintSpeed 980 while he sprints (pad/sprint.rs,
  decision 9), so `leader_view`'s `max_speed` (lead.rs) already reads 980 then; `PadState::x2.sprinting` is the flag.
- 2026-10-15 X2 -> ERPI / S2 (finding, item 6 of X2's brief): KH3 has NO per-hit cap on the friends beyond the halving
  above 24 (B's reading of CalcDamage, kh-core damage.rs step 5: a base above `m_DamageMax` becomes floor((24 + S - D) /
  2); the player and friends' base at most 80 is Sora's too). kh-er's `guards::capped(damage, damage_max)` (S2,
  party/leader.rs `LeadOrder::Hit`) IS a hard 24 HP cap of the port's own; with decision 1 the leader's
  `Party::defender(id).damage_max` is 0, so that cap no longer applies to him (no kh-er edit needed).
- 2026-10-15 ERPI (ER party I, integration; state note build/sora_tmp/er/party/IMPL_ERPI_STATE.md) -> all: requests CLOSED:
  (1) S4 -> S6: Sora's own camp picture is placed by `hud.camp_panels()` / `hud.camp_member()` (kh-er portrait.rs
  `sora_placement`, kh-app menu_model.rs's rule; hud.rs passes them under the feature; Sora first / no party = the old
  `placement` exactly). (2) S4 -> S5: the friends cast the hero light's body shadow: the light pass binds a COPY of
  Sora's frame block whose slots it does not read as his body (empty / his Keyblade's w = 2) carry the friends' capsules
  (kh-er-render gfx/party.rs `light_pass_frame`; one capsule around each friend when the full sets do not fit; no friend
  drawn = Sora's block, the command list unchanged). (3) S5 -> S3: `allybody::ground_material(chara) -> Option<i32>` (the
  body's `hit_material`, override first; TRUSTED only once it has been seen to change while the body is out, since the
  bodies have map collision off; parked / KO = None) -> fx.rs `member_surface(own, leader)`; key `party.body_ground`
  (default 1). (4) S8's hook lines in S1's party/mod.rs (3), S3's allybody_live.rs (3) and sora.rs (stage_wants) are in
  place. (5) the coordinator after ER86: the friends' placed hero light follows `hero.anchor` with each friend's OWN eased
  facing (gfx/party.rs `hero_toward_for`); their probes are Sora's steady world probe (see the state note).
  (6) the BOSS stream's finding: every LIVE ally body's AI switch (ChrCtrl + 0xE8 bit 5, `sdkx::set_ai_on`) is ON while
  it is out (hittable) and OFF while parked / KO; the body stays passive (debug no-move / no-action, root motion 0, our
  writes each physics phase); key `party.body_ai` (default 1; 0 = not touched). Go/no-go item 14 (`party.body_test`)
  logs the byte, how often the game cleared it, can_target and the attacks aimed at a body.
- 2026-10-15 ERPI -> S8 / BOSS (notice): `party.er = 0` or `party.unlocks = 0` while a staged fight / Donald's scene is out
  now undoes it from `unlock::frame` (`Director::switch_off`: the real boss's AI switch released, the double unstaged, the
  scenes reset, the staged draws / taps / wants / mask cleared). Before, party_step stopped and left them as they were.
- 2026-10-15 ERPI -> S1 (finding, not changed): `StepFeed::cure_bodies` is written (partyui `SyncOut`) and never read: the
  save point's cure of the ally bodies goes through `status::request_member` -> S3's `take_ally_cures`, so nothing is lost.
- 2026-10-15 K1 -> all (notice): the AI members' Move state, Idle facing and MoveTo reach changed (section K1): test-level traces
  differ from s0/after_* in every friend's position / yaw / clip from the first move on (list in IMPL_K1_STATE.md).
- 2026-10-15 K1 -> AIX (finding): out of battle Donald casts Cure (MagicCast 1.2 s) and Turns to the player while the leader
  sprints; with Goofy leading he stood 2.5 s in TurnTo + Cure: those are the largest follow gaps left (2500+ cm), decisions not
  locomotion. Also the EQS Destination of n_Common_FollowPlayer lands beside / behind the friend now and then (KH3's
  precede query should put it ahead of a running player), which is what K1's arrival hold covers.
- 2026-10-15 X3G -> F / X2 (HUD): Goofy leading: his shield gauge (`Party::goofy_gauges`: XD 2.1 "on his HUD ring, the focus-bar
  slot"), the Discus Barrage lock marks (`goofy_locks`) and his focus are not drawn yet; the Shield Guardian offer / Finish already
  shows on the situation slot (pad_menu).
- 2026-10-15 X3G -> X2 / F (Customize): Goofy's kit abilities (`FriendStatus::goofy_kit`, `list` / `kit_goofy_equip`) are not in the
  camp menu's ability list yet (Donald's `kit` neither): the test level has them all on (test_level99), ER learns them by level and
  wears them while the AP allows.
- 2026-10-15 X3G -> S1 / S3 (Elden Ring): Over Here! (`Party::goofy_taunt`): enemies within the radius should take the leader body as
  their target for `left` seconds (the ally-body route if feasible, else hate on the leader body, e.g. a 0-damage leader-owned hit).
  The leader body should also not play a hit reaction / lose HP for a guarded hit (`HitResult::guarded` from `Party::receive_hit`).
- 2026-10-15 X3G -> S4 (Elden Ring drawing): kh-er partydraw.rs `pitch_actor(.., core.rt.team.mesh_pitch(v.id))` should also take
  `core.rt.party.pad_mesh_pitch(v.id)` (Goofy's solo Bombardier Dive), as kh-app party_draw.rs now does.
- 2026-10-15 X3G -> VFX / coordinator (visual fault, seen): `vn_ex002_frt_hit0` (main vfx pack) and GooFRocket's
  `vn_ex002_frt_ksk00` (SQEX TailPolygon) / `vn_ex002_frt_tsn00` (GPU cloud sprites) draw as fields of blue tiles in the test
  level, in the TEAM Fusion Rocket / Spin too (build/sora_tmp/party/d6/run_b/f2845.png; x3g r4/r5 rocket_05..11). The solo
  finishers route around it (sheet rows fin.hit_effect, fin.rocket_drop_effects, fin.rocket_trail); the team attack still shows it.
- 2026-10-15 X3D -> S6 / F (menus): Donald's kit abilities (`FriendStatus::kit`, `KitAbilities::list` / `set_equipped`,
  `KitAbility::label` / `ap`) are not in any Abilities menu yet: the test level equips them all (the host), in Elden Ring
  they are equipped at grant while his AP allows. Please list them on his Abilities page (equip / unequip with the AP check).
- 2026-10-15 X3D -> coordinator: kh-party `tests/party.rs::move_to_and_warp` (AI Donald's MoveTo, no pad) fails since
  ~23:30; X3D changes nothing for an AI member (likely the concurrent locomotion / AI work).
- 2026-10-15 AIX -> K1 (answer): (1) field Cure while the leader runs: KH3 casts it standing (CureFlow TurnTo -> NpcCureAction, MagicCast; no deferral of its own): our design `kh_ai::controller::params::FIELD_CURE_DEFER_SPEED` (300 cm/s) defers the cure / item decision out of battle while the player runs (AI Sora's Recovery too); in battle KH3's rule. The k1 harness Sora (kh-core placeholder status, HP 0/0) read as dying: cure candidates without an HP pool are dropped. (2) the follow EQS: PlayerFollowingSide / _Destination ported from 0x1404D6C80 / 0x1404D7150 (avg direction turned 90 + the player's right axis, toward the querier's / Destination's side; was [D] the home slot); the remaining behind-the-leader samples are the RunEQS refresh lag (service intervals, KH3 data) covered by your MoveTo observer. k1_measure_all: Donald max 1624 -> 512 (Sora sprint), 2779 -> 498 (Goofy leads), AI Sora 808 -> 739, warps 0. State: IMPL_AIX_STATE.md.
- 2026-10-15 X2-b -> X3G / X3D / F / S6 (DONE, additive): (1) HUD: `PartyHudSlot::focus: Option<(i32, i32)>` = the
  LEADER's bar in the focus-gauge slot (kh-partyhost custom.rs `leader_focus`: Goofy's shield gauge, his Barrage focus
  while he aims; Donald's Spell Volley focus), drawn by kh-host hud.rs with the focus slot unlocked; `hud_snapshot` fills
  `UiSnapshot::aim` from `custom::leader_aim` (Discus Barrage / Spell Volley as Sora's shotlock scope: mode 1 aiming, 2
  while the volley / bouncing shield runs, a mark per lock, the lock counter). (2) Abilities: `PartyMenuMember::kit:
  Vec<KitRow>` (owned kit abilities: save name, name, AP, category, worn, help; Unison / Fusion with KH3's own name and
  help) listed after KH3's rows on the friend's Abilities page (ids `hud_x2::KIT_ID_BASE` + index);
  `PartyMenuEdit::Kit { chara, key, on }` -> `PartyRuntime::apply_kit_edit` (Goofy `kit_goofy_equip`, Donald
  `kit.set_equipped` with the same AP check). Saved by X3D / X3G's KIT_* entries of the v5 friends record (unchanged).
- 2026-10-15 ERPI-b (ER party I) -> X3G / S2 / S3 / S4 / ER86: X3G's Elden Ring requests DONE (offline; untested in game):
  (1) Over Here! (`Party::goofy_taunt`): kh-er `party/taunt.rs`: every hostile within its radius is given a minimal hit
  (`party.taunt_phys` 1, no poise, dmgLevel 0) from Goofy's Elden Ring body, nearest first, one bullet a frame, again every
  `party.taunt_every` s (2) while it lasts: Goofy leading = a player bullet (the leader body); AI Goofy = owned by his ally
  body (`fire_owned`, U14's route). `party.taunt = 0`: off. (2) A guarded hit: `damage_in` now applies a friend leader's hit
  AT ONCE (`LeaderBody::hit_now`, the same cap and `receive_hit` as the queued order, which now calls it): `guarded` / not
  `accepted` = 0 HP and his kit's guard reaction (kh-party), logged "HIS GUARD TOOK IT"; the body is topped up as always.
  NOT covered: what the game itself does to its hidden body (its own stagger, its status buildup from that blow).
  (3) partydraw.rs pitches the actor by `team.mesh_pitch` OR `party.pad_mesh_pitch` (Goofy's solo Bombardier Dive), as
  kh-app. (4) EACH FRIEND'S OWN STEADY LIGHT PROBE (kh-er-render gfx/party_probe.rs): the game-data texture gets 2 rows
  (8 Donald, 9 Goofy; light.rs's rows untouched); the probe pass draws the same probe shader with his frame copy (his feet's
  `er.probe`, his blob and capsules) into his row; at the read-back his row is parsed as Sora's, with his own TapMemory and
  `stable::Filter`; his light = Sora's steady world values + his eased probes -> `light::rig` -> his frame copy's light rows
  (14-17) and his materials' rig / SH. No reading yet / the void / no game light = Sora's light as before. Key
  `r.party_probes` (1). Unit tests in party_probe.rs (3).
- (party VF -> X3G / coordinator, 2026-10-15) The Fusion "blue tiles" are fixed in the party pack (vn_ex002_frt_tsn00's
  c_cloud0: it now uses its own n_ex002 material, translated; build/sora_tmp/party/IMPL_VF_STATE.md). vn_ex002_frt_hit0 and
  vn_ex002_frt_ksk00 never drew tiles (seen alone in the ER harness). sheets/port/party-kit-goofy.json can go back to the
  clip's effects: `fin.hit_effect` = vn_ex002_frt_hit0, `fin.rocket_drop_effects` = 0 (VF did not edit X3G's sheet).
- (party VF -> deploy) New party-pack files: party/vfx/materials/{ami_imp_flw_smc00a__NPC_n_ex002_mat,
  zmi_rng_00sa__wep_w_do030_mat}.json, party/vfx/meshes/{ymd_dom_00__NPC_n_ex001_mdl, ymd_dom_01__NPC_n_ex001_mdl,
  kmd_sph_64x32_01__Effects_trescom_mdl}.glb, party/vfx/er/{s177,m177}.ps.dxil + v177.vs.dxil; rewritten: party/vfx/shaders/*,
  party/vfx/er/shaders.json, 6 party/vfx/effects/*.json. The main pack is untouched.
- 2026-10-16 ERPI-c -> S1 / AIX / K1 (fix, additive): `party::nav::Casts::clear_now` (the same knee-high line, never
  answered optimistically: its own 4 casts, the step's budget untouched); S1's catch-up after a body action uses it. With
  AIX's EQS spending the step's budget first, the old `clear` answered "clear" for a friend behind a fog wall and he stayed
  there. Party preview rebuilt: build/sora_tmp/er/party/preview/kh_er_EB682FE5323B.dll (DEPLOY.md).
